// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::collections::VecDeque;
use std::io::{self, stdin};
use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context};
use base::{error, warn, Event, RawDescriptor, Terminal};
use cros_async::{EventAsync, Executor};
use data_model::DataInit;

use argh::FromArgs;
use futures::future::{AbortHandle, Abortable};
use hypervisor::ProtectionType;
use once_cell::sync::OnceCell;
use sync::Mutex;
use vm_memory::GuestMemory;
use vmm_vhost::message::{VhostUserProtocolFeatures, VhostUserVirtioFeatures};

use crate::serial_device::{SerialDevice, SerialHardware, SerialParameters, SerialType};
use crate::virtio::console::{
    handle_input, process_transmit_queue, spawn_input_thread, virtio_console_config, ConsoleError,
};
use crate::virtio::vhost::user::device::handler::{
    DeviceRequestHandler, Doorbell, VhostUserBackend,
};
use crate::virtio::{self, copy_config};

static CONSOLE_EXECUTOR: OnceCell<Executor> = OnceCell::new();

async fn run_tx_queue(
    mut queue: virtio::Queue,
    mem: GuestMemory,
    doorbell: Arc<Mutex<Doorbell>>,
    kick_evt: EventAsync,
    mut output: Box<dyn io::Write>,
) {
    loop {
        if let Err(e) = kick_evt.next_val().await {
            error!("Failed to read kick event for tx queue: {}", e);
            break;
        }
        process_transmit_queue(&mem, &doorbell, &mut queue, &mut output);
    }
}

async fn run_rx_queue(
    mut queue: virtio::Queue,
    mem: GuestMemory,
    doorbell: Arc<Mutex<Doorbell>>,
    kick_evt: EventAsync,
    in_buffer: Arc<Mutex<VecDeque<u8>>>,
    in_avail_evt: EventAsync,
) {
    loop {
        if let Err(e) = in_avail_evt.next_val().await {
            error!("Failed reading in_avail_evt: {}", e);
            break;
        }
        match handle_input(&mem, &doorbell, in_buffer.lock().deref_mut(), &mut queue) {
            Ok(()) => {}
            Err(ConsoleError::RxDescriptorsExhausted) => {
                if let Err(e) = kick_evt.next_val().await {
                    error!("Failed to read kick event for rx queue: {}", e);
                    break;
                }
            }
        }
    }
}

struct ConsoleBackend {
    input: Option<Box<dyn io::Read + Send>>,
    output: Option<Box<dyn io::Write + Send>>,
    avail_features: u64,
    acked_features: u64,
    acked_protocol_features: VhostUserProtocolFeatures,
    workers: [Option<AbortHandle>; Self::MAX_QUEUE_NUM],
}

impl SerialDevice for ConsoleBackend {
    fn new(
        protected_vm: ProtectionType,
        _evt: Event,
        input: Option<Box<dyn io::Read + Send>>,
        output: Option<Box<dyn io::Write + Send>>,
        _keep_rds: Vec<RawDescriptor>,
    ) -> ConsoleBackend {
        let avail_features = 1u64 << crate::virtio::VIRTIO_F_VERSION_1
            | virtio::base_features(protected_vm)
            | VhostUserVirtioFeatures::PROTOCOL_FEATURES.bits();
        ConsoleBackend {
            input,
            output,
            avail_features,
            acked_features: 0,
            acked_protocol_features: VhostUserProtocolFeatures::empty(),
            workers: Default::default(),
        }
    }
}

impl VhostUserBackend for ConsoleBackend {
    const MAX_QUEUE_NUM: usize = 2; /* transmit and receive queues */
    const MAX_VRING_LEN: u16 = 256;

    type Error = anyhow::Error;

    fn features(&self) -> u64 {
        self.avail_features
    }

    fn ack_features(&mut self, value: u64) -> anyhow::Result<()> {
        let unrequested_features = value & !self.avail_features;
        if unrequested_features != 0 {
            bail!("invalid features are given: {:#x}", unrequested_features);
        }

        self.acked_features |= value;

        Ok(())
    }

    fn acked_features(&self) -> u64 {
        self.acked_features
    }

    fn protocol_features(&self) -> VhostUserProtocolFeatures {
        VhostUserProtocolFeatures::CONFIG
    }

    fn ack_protocol_features(&mut self, features: u64) -> anyhow::Result<()> {
        let features = VhostUserProtocolFeatures::from_bits(features)
            .ok_or_else(|| anyhow!("invalid protocol features are given: {:#x}", features))?;
        let supported = self.protocol_features();
        self.acked_protocol_features = features & supported;
        Ok(())
    }

    fn acked_protocol_features(&self) -> u64 {
        self.acked_protocol_features.bits()
    }

    fn read_config(&self, offset: u64, data: &mut [u8]) {
        let config = virtio_console_config {
            max_nr_ports: 1.into(),
            ..Default::default()
        };
        copy_config(data, 0, config.as_slice(), offset);
    }

    fn reset(&mut self) {
        for handle in self.workers.iter_mut().filter_map(Option::take) {
            handle.abort();
        }
    }

    fn start_queue(
        &mut self,
        idx: usize,
        mut queue: virtio::Queue,
        mem: GuestMemory,
        doorbell: Arc<Mutex<Doorbell>>,
        kick_evt: Event,
    ) -> anyhow::Result<()> {
        if let Some(handle) = self.workers.get_mut(idx).and_then(Option::take) {
            warn!("Starting new queue handler without stopping old handler");
            handle.abort();
        }

        // Enable any virtqueue features that were negotiated (like VIRTIO_RING_F_EVENT_IDX).
        queue.ack_features(self.acked_features);

        // Safe because the executor is initialized in main() below.
        let ex = CONSOLE_EXECUTOR.get().expect("Executor not initialized.");

        let kick_evt =
            EventAsync::new(kick_evt.0, ex).context("Failed to create EventAsync for kick_evt")?;
        let (handle, registration) = AbortHandle::new_pair();
        match idx {
            // ReceiveQueue
            0 => {
                // See explanation in devices/src/virtio/console.rs
                // We need a multithreaded input polling because io::Read only provides
                // a blocking interface which we cannot use in an async function.
                let in_avail_evt = match Event::new() {
                    Ok(evt) => evt,
                    Err(e) => {
                        bail!("Failed creating Event: {}", e);
                    }
                };

                let input_unpacked = self
                    .input
                    .take()
                    .ok_or_else(|| anyhow!("input source unavailable"))?;
                let in_buffer = spawn_input_thread(input_unpacked, &in_avail_evt)
                    .take()
                    .ok_or_else(|| anyhow!("input channel unavailable"))?;

                // Create the async 'in' event so we can await on it.
                let in_avail_async_evt = EventAsync::new(in_avail_evt.0, ex)
                    .context("Failed to create EventAsync for in_avail_evt")?;

                ex.spawn_local(Abortable::new(
                    run_rx_queue(
                        queue,
                        mem,
                        doorbell,
                        kick_evt,
                        in_buffer,
                        in_avail_async_evt,
                    ),
                    registration,
                ))
                .detach();
            }
            // TransmitQueue
            1 => {
                // Take ownership of output writer.
                // Safe because output should always be initialized to something
                let output_unwrapped: Box<dyn io::Write + Send> = self
                    .output
                    .take()
                    .ok_or_else(|| anyhow!("no output available"))?;
                ex.spawn_local(Abortable::new(
                    run_tx_queue(queue, mem, doorbell, kick_evt, output_unwrapped),
                    registration,
                ))
                .detach();
            }
            _ => bail!("attempted to start unknown queue: {}", idx),
        }

        self.workers[idx] = Some(handle);
        Ok(())
    }

    fn stop_queue(&mut self, idx: usize) {
        if let Some(handle) = self.workers.get_mut(idx).and_then(Option::take) {
            handle.abort();
        }
    }
}

fn run_console(params: &SerialParameters, socket: &str) -> anyhow::Result<()> {
    // We need to pass an event as per Serial Device API but we don't really use it anyway.
    let evt = Event::new()?;
    // Same for keep_rds, we don't really use this.
    let mut keep_rds = Vec::new();
    let console = match params.create_serial_device::<ConsoleBackend>(
        ProtectionType::Unprotected,
        &evt,
        &mut keep_rds,
    ) {
        Ok(c) => c,
        Err(e) => bail!(e),
    };

    let handler = DeviceRequestHandler::new(console);
    let ex = Executor::new().context("Failed to create executor")?;

    let _ = CONSOLE_EXECUTOR.set(ex.clone());

    // run_until() returns an Result<Result<..>> which the ? operator lets us flatten.
    ex.run_until(handler.run(socket, &ex))?
}

#[derive(FromArgs)]
#[argh(description = "")]
struct Options {
    #[argh(option, description = "path to a socket", arg_name = "PATH")]
    socket: String,
    #[argh(option, description = "path to a file", arg_name = "OUTFILE")]
    output_file: Option<PathBuf>,
    #[argh(option, description = "path to a file", arg_name = "INFILE")]
    input_file: Option<PathBuf>,
}

/// Starts a vhost-user console device.
/// Returns an error if the given `args` is invalid or the device fails to run.
pub fn run_console_device(program_name: &str, args: &[&str]) -> anyhow::Result<()> {
    let Options {
        input_file,
        output_file,
        socket,
    } = match Options::from_args(&[program_name], args) {
        Ok(opts) => opts,
        Err(e) => {
            if e.status.is_err() {
                bail!(e.output);
            } else {
                println!("{}", e.output);
            }
            return Ok(());
        }
    };

    // Set stdin() in raw mode so we can send over individual keystrokes unbuffered
    stdin()
        .set_raw_mode()
        .context("Failed to set terminal raw mode")?;

    let type_ = match output_file {
        Some(_) => SerialType::File,
        None => SerialType::Stdout,
    };

    let params = SerialParameters {
        type_,
        hardware: SerialHardware::VirtioConsole,
        // Required only if type_ is SerialType::File or SerialType::UnixSocket
        path: output_file,
        input: input_file,
        num: 1,
        console: true,
        earlycon: false,
        // We do not support stdin-less mode
        stdin: true,
    };

    let res = run_console(&params, &socket);

    // Restore terminal capabilities back to what they were before
    stdin()
        .set_canon_mode()
        .context("Failed to restore canonical mode for terminal")?;

    if let Err(e) = res {
        bail!("error occurred: {:#}", e);
    }

    Ok(())
}
