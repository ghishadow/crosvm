// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cmp::{max, min};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::u32;

use base::{error, pagesize, AsRawDescriptor, Event, PollToken, RawDescriptor, Tube, WaitContext};
use hypervisor::{Datamatch, MemSlot};

use resources::{Alloc, MmioType, SystemAllocator};

use vfio_sys::*;
use vm_control::{
    VmIrqRequest, VmIrqResponse, VmMemoryRequest, VmMemoryResponse, VmRequest, VmResponse,
};

use crate::pci::msix::{
    MsixConfig, BITS_PER_PBA_ENTRY, MSIX_PBA_ENTRIES_MODULO, MSIX_TABLE_ENTRIES_MODULO,
};

use crate::pci::pci_device::{Error as PciDeviceError, PciDevice};
use crate::pci::{
    PciAddress, PciBarConfiguration, PciBarPrefetchable, PciBarRegionType, PciClassCode,
    PciInterruptPin,
};

use crate::vfio::{VfioDevice, VfioIrqType, VfioPciConfig};

const PCI_VENDOR_ID: u32 = 0x0;
const INTEL_VENDOR_ID: u16 = 0x8086;
const PCI_COMMAND: u32 = 0x4;
const PCI_COMMAND_MEMORY: u8 = 0x2;
const PCI_BASE_CLASS_CODE: u32 = 0x0B;
const PCI_INTERRUPT_NUM: u32 = 0x3C;
const PCI_INTERRUPT_PIN: u32 = 0x3D;

const PCI_CAPABILITY_LIST: u32 = 0x34;
const PCI_CAP_ID_MSI: u8 = 0x05;
const PCI_CAP_ID_MSIX: u8 = 0x11;

// MSI registers
const PCI_MSI_NEXT_POINTER: u32 = 0x1; // Next cap pointer
const PCI_MSI_FLAGS: u32 = 0x2; // Message Control
const PCI_MSI_FLAGS_ENABLE: u16 = 0x0001; // MSI feature enabled
const PCI_MSI_FLAGS_64BIT: u16 = 0x0080; // 64-bit addresses allowed
const PCI_MSI_FLAGS_MASKBIT: u16 = 0x0100; // Per-vector masking capable
const PCI_MSI_ADDRESS_LO: u32 = 0x4; // MSI address lower 32 bits
const PCI_MSI_ADDRESS_HI: u32 = 0x8; // MSI address upper 32 bits (if 64 bit allowed)
const PCI_MSI_DATA_32: u32 = 0x8; // 16 bits of data for 32-bit message address
const PCI_MSI_DATA_64: u32 = 0xC; // 16 bits of date for 64-bit message address

// MSI length
const MSI_LENGTH_32BIT_WITHOUT_MASK: u32 = 0xA;
const MSI_LENGTH_32BIT_WITH_MASK: u32 = 0x14;
const MSI_LENGTH_64BIT_WITHOUT_MASK: u32 = 0xE;
const MSI_LENGTH_64BIT_WITH_MASK: u32 = 0x18;

enum VfioMsiChange {
    Disable,
    Enable,
}

struct VfioMsiCap {
    offset: u32,
    is_64bit: bool,
    mask_cap: bool,
    ctl: u16,
    address: u64,
    data: u16,
    vm_socket_irq: Tube,
    irqfd: Option<Event>,
    gsi: Option<u32>,
}

impl VfioMsiCap {
    fn new(config: &VfioPciConfig, msi_cap_start: u32, vm_socket_irq: Tube) -> Self {
        let msi_ctl: u16 = config.read_config(msi_cap_start + PCI_MSI_FLAGS);

        VfioMsiCap {
            offset: msi_cap_start,
            is_64bit: (msi_ctl & PCI_MSI_FLAGS_64BIT) != 0,
            mask_cap: (msi_ctl & PCI_MSI_FLAGS_MASKBIT) != 0,
            ctl: 0,
            address: 0,
            data: 0,
            vm_socket_irq,
            irqfd: None,
            gsi: None,
        }
    }

    fn is_msi_reg(&self, index: u64, len: usize) -> bool {
        let msi_len = match (self.is_64bit, self.mask_cap) {
            (true, true) => MSI_LENGTH_64BIT_WITH_MASK,
            (true, false) => MSI_LENGTH_64BIT_WITHOUT_MASK,
            (false, true) => MSI_LENGTH_32BIT_WITH_MASK,
            (false, false) => MSI_LENGTH_32BIT_WITHOUT_MASK,
        };

        index >= self.offset as u64
            && index + len as u64 <= (self.offset + msi_len) as u64
            && len as u32 <= msi_len
    }

    fn write_msi_reg(&mut self, index: u64, data: &[u8]) -> Option<VfioMsiChange> {
        let len = data.len();
        let offset = index as u32 - self.offset;
        let mut ret: Option<VfioMsiChange> = None;
        let old_address = self.address;
        let old_data = self.data;

        // write msi ctl
        if len == 2 && offset == PCI_MSI_FLAGS {
            let was_enabled = self.is_msi_enabled();
            let value: [u8; 2] = [data[0], data[1]];
            self.ctl = u16::from_le_bytes(value);
            let is_enabled = self.is_msi_enabled();
            if !was_enabled && is_enabled {
                self.enable();
                ret = Some(VfioMsiChange::Enable);
            } else if was_enabled && !is_enabled {
                ret = Some(VfioMsiChange::Disable)
            }
        } else if len == 4 && offset == PCI_MSI_ADDRESS_LO && !self.is_64bit {
            //write 32 bit message address
            let value: [u8; 8] = [data[0], data[1], data[2], data[3], 0, 0, 0, 0];
            self.address = u64::from_le_bytes(value);
        } else if len == 4 && offset == PCI_MSI_ADDRESS_LO && self.is_64bit {
            // write 64 bit message address low part
            let value: [u8; 8] = [data[0], data[1], data[2], data[3], 0, 0, 0, 0];
            self.address &= !0xffffffff;
            self.address |= u64::from_le_bytes(value);
        } else if len == 4 && offset == PCI_MSI_ADDRESS_HI && self.is_64bit {
            //write 64 bit message address high part
            let value: [u8; 8] = [0, 0, 0, 0, data[0], data[1], data[2], data[3]];
            self.address &= 0xffffffff;
            self.address |= u64::from_le_bytes(value);
        } else if len == 8 && offset == PCI_MSI_ADDRESS_LO && self.is_64bit {
            // write 64 bit message address
            let value: [u8; 8] = [
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ];
            self.address = u64::from_le_bytes(value);
        } else if len == 2
            && ((offset == PCI_MSI_DATA_32 && !self.is_64bit)
                || (offset == PCI_MSI_DATA_64 && self.is_64bit))
        {
            // write message data
            let value: [u8; 2] = [data[0], data[1]];
            self.data = u16::from_le_bytes(value);
        }

        if self.is_msi_enabled() && (old_address != self.address || old_data != self.data) {
            self.add_msi_route();
        }

        ret
    }

    fn is_msi_enabled(&self) -> bool {
        self.ctl & PCI_MSI_FLAGS_ENABLE == PCI_MSI_FLAGS_ENABLE
    }

    fn add_msi_route(&self) {
        let gsi = match self.gsi {
            Some(g) => g,
            None => {
                error!("Add msi route but gsi is none");
                return;
            }
        };
        if let Err(e) = self.vm_socket_irq.send(&VmIrqRequest::AddMsiRoute {
            gsi,
            msi_address: self.address,
            msi_data: self.data.into(),
        }) {
            error!("failed to send AddMsiRoute request at {:?}", e);
            return;
        }
        match self.vm_socket_irq.recv() {
            Ok(VmIrqResponse::Err(e)) => error!("failed to call AddMsiRoute request {:?}", e),
            Ok(_) => {}
            Err(e) => error!("failed to receive AddMsiRoute response {:?}", e),
        }
    }

    fn allocate_one_msi(&mut self) {
        let irqfd = match self.irqfd.take() {
            Some(e) => e,
            None => match Event::new() {
                Ok(e) => e,
                Err(e) => {
                    error!("failed to create event: {:?}", e);
                    return;
                }
            },
        };

        let request = VmIrqRequest::AllocateOneMsi { irqfd };
        let request_result = self.vm_socket_irq.send(&request);

        // Stash the irqfd in self immediately because we used take above.
        self.irqfd = match request {
            VmIrqRequest::AllocateOneMsi { irqfd } => Some(irqfd),
            _ => unreachable!(),
        };

        if let Err(e) = request_result {
            error!("failed to send AllocateOneMsi request: {:?}", e);
            return;
        }

        match self.vm_socket_irq.recv() {
            Ok(VmIrqResponse::AllocateOneMsi { gsi }) => self.gsi = Some(gsi),
            _ => error!("failed to receive AllocateOneMsi Response"),
        }
    }

    fn enable(&mut self) {
        if self.gsi.is_none() || self.irqfd.is_none() {
            self.allocate_one_msi();
        }

        self.add_msi_route();
    }

    fn get_msi_irqfd(&self) -> Option<&Event> {
        self.irqfd.as_ref()
    }

    fn destroy(&mut self) {
        if let Some(gsi) = self.gsi {
            if let Some(irqfd) = self.irqfd.take() {
                let request = VmIrqRequest::ReleaseOneIrq { gsi, irqfd };
                if self.vm_socket_irq.send(&request).is_ok() {
                    let _ = self.vm_socket_irq.recv::<VmIrqResponse>();
                }
            }
        }
    }
}

// MSI-X registers in MSI-X capability
const PCI_MSIX_FLAGS: u32 = 0x02; // Message Control
const PCI_MSIX_FLAGS_QSIZE: u16 = 0x07FF; // Table size
const PCI_MSIX_TABLE: u32 = 0x04; // Table offset
const PCI_MSIX_TABLE_BIR: u32 = 0x07; // BAR index
const PCI_MSIX_TABLE_OFFSET: u32 = 0xFFFFFFF8; // Offset into specified BAR
const PCI_MSIX_PBA: u32 = 0x08; // Pending bit Array offset
const PCI_MSIX_PBA_BIR: u32 = 0x07; // BAR index
const PCI_MSIX_PBA_OFFSET: u32 = 0xFFFFFFF8; // Offset into specified BAR

struct VfioMsixCap {
    config: MsixConfig,
    offset: u32,
    table_size: u16,
    table_pci_bar: u32,
    table_offset: u64,
    table_size_bytes: u64,
    pba_pci_bar: u32,
    pba_offset: u64,
    pba_size_bytes: u64,
}

impl VfioMsixCap {
    fn new(config: &VfioPciConfig, msix_cap_start: u32, vm_socket_irq: Tube) -> Self {
        let msix_ctl: u16 = config.read_config(msix_cap_start + PCI_MSIX_FLAGS);
        let table: u32 = config.read_config(msix_cap_start + PCI_MSIX_TABLE);
        let table_pci_bar = table & PCI_MSIX_TABLE_BIR;
        let table_offset = (table & PCI_MSIX_TABLE_OFFSET) as u64;
        let pba: u32 = config.read_config(msix_cap_start + PCI_MSIX_PBA);
        let pba_pci_bar = pba & PCI_MSIX_PBA_BIR;
        let pba_offset = (pba & PCI_MSIX_PBA_OFFSET) as u64;

        let mut table_size = (msix_ctl & PCI_MSIX_FLAGS_QSIZE) as u64 + 1;
        if table_pci_bar == pba_pci_bar
            && pba_offset > table_offset
            && (table_offset + table_size * MSIX_TABLE_ENTRIES_MODULO) > pba_offset
        {
            table_size = (pba_offset - table_offset) / MSIX_TABLE_ENTRIES_MODULO;
        }

        let table_size_bytes = table_size * MSIX_TABLE_ENTRIES_MODULO;
        let pba_size_bytes = ((table_size + BITS_PER_PBA_ENTRY as u64 - 1)
            / BITS_PER_PBA_ENTRY as u64)
            * MSIX_PBA_ENTRIES_MODULO;

        VfioMsixCap {
            config: MsixConfig::new(table_size as u16, vm_socket_irq),
            offset: msix_cap_start,
            table_size: table_size as u16,
            table_pci_bar,
            table_offset,
            table_size_bytes,
            pba_pci_bar,
            pba_offset,
            pba_size_bytes,
        }
    }

    // only msix control register is writable and need special handle in pci r/w
    fn is_msix_control_reg(&self, offset: u32, size: u32) -> bool {
        let control_start = self.offset + PCI_MSIX_FLAGS;
        let control_end = control_start + 2;

        offset < control_end && offset + size > control_start
    }

    fn read_msix_control(&self, data: &mut u32) {
        *data = self.config.read_msix_capability(*data);
    }

    fn write_msix_control(&mut self, data: &[u8]) -> Option<VfioMsiChange> {
        let old_enabled = self.config.enabled();
        let old_masked = self.config.masked();

        self.config
            .write_msix_capability(PCI_MSIX_FLAGS.into(), data);

        let new_enabled = self.config.enabled();
        let new_masked = self.config.masked();
        if (!old_enabled && new_enabled) || (new_enabled && old_masked && !new_masked) {
            Some(VfioMsiChange::Enable)
        } else if (old_enabled && !new_enabled) || (new_enabled && !old_masked && new_masked) {
            Some(VfioMsiChange::Disable)
        } else {
            None
        }
    }

    fn is_msix_table(&self, bar_index: u32, offset: u64) -> bool {
        bar_index == self.table_pci_bar
            && offset >= self.table_offset
            && offset < self.table_offset + self.table_size_bytes
    }

    fn get_msix_table(&self, bar_index: u32) -> Option<(u64, u64)> {
        if bar_index == self.table_pci_bar {
            Some((self.table_offset, self.table_size_bytes))
        } else {
            None
        }
    }

    fn read_table(&self, offset: u64, data: &mut [u8]) {
        let offset = offset - self.table_offset;
        self.config.read_msix_table(offset, data);
    }

    fn write_table(&mut self, offset: u64, data: &[u8]) {
        let offset = offset - self.table_offset;
        self.config.write_msix_table(offset, data);
    }

    fn is_msix_pba(&self, bar_index: u32, offset: u64) -> bool {
        bar_index == self.pba_pci_bar
            && offset >= self.pba_offset
            && offset < self.pba_offset + self.pba_size_bytes
    }

    fn get_msix_pba(&self, bar_index: u32) -> Option<(u64, u64)> {
        if bar_index == self.pba_pci_bar {
            Some((self.pba_offset, self.pba_size_bytes))
        } else {
            None
        }
    }

    fn read_pba(&self, offset: u64, data: &mut [u8]) {
        let offset = offset - self.pba_offset;
        self.config.read_pba_entries(offset, data);
    }

    fn write_pba(&mut self, offset: u64, data: &[u8]) {
        let offset = offset - self.pba_offset;
        self.config.write_pba_entries(offset, data);
    }

    fn get_msix_irqfds(&self) -> Option<Vec<&Event>> {
        let mut irqfds = Vec::new();

        for i in 0..self.table_size {
            let irqfd = self.config.get_irqfd(i as usize);
            if let Some(fd) = irqfd {
                irqfds.push(fd);
            } else {
                return None;
            }
        }

        Some(irqfds)
    }

    fn destroy(&mut self) {
        self.config.destroy()
    }
}

struct VfioMsixAllocator {
    // memory regions unoccupied by MSIX registers
    // stores sets of (start, end) tuples, where `end` is the address of the
    // last byte in the region
    regions: BTreeSet<(u64, u64)>,
}

impl VfioMsixAllocator {
    // Creates a new `VfioMsixAllocator` for managing a range of MSIX addresses.
    // Can return `Err` if `base` + `size` overflows a u64.
    //
    // * `base` - The starting address of the range to manage.
    // * `size` - The size of the address range in bytes.
    fn new(base: u64, size: u64) -> Result<Self, PciDeviceError> {
        if size == 0 {
            return Err(PciDeviceError::MsixAllocatorSizeZero);
        }
        let end = base
            .checked_add(size - 1)
            .ok_or(PciDeviceError::MsixAllocatorOverflow { base, size })?;
        let mut regions = BTreeSet::new();
        regions.insert((base, end));
        Ok(VfioMsixAllocator { regions })
    }

    // Allocates a range of addresses from the managed region with a required location.
    // Returns a new range of addresses excluding the required range.
    fn allocate_at(&mut self, start: u64, size: u64) -> Result<(), PciDeviceError> {
        if size == 0 {
            return Err(PciDeviceError::MsixAllocatorSizeZero);
        }
        let end = start
            .checked_add(size - 1)
            .ok_or(PciDeviceError::MsixAllocatorOutOfSpace)?;
        while let Some(slot) = self
            .regions
            .iter()
            .find(|range| (start <= range.1 && end >= range.0))
            .cloned()
        {
            self.regions.remove(&slot);
            if slot.0 < start {
                self.regions.insert((slot.0, start - 1));
            }
            if slot.1 > end {
                self.regions.insert((end + 1, slot.1));
            }
        }
        Ok(())
    }
}

struct VfioPciWorker {
    vm_socket: Tube,
    name: String,
}

impl VfioPciWorker {
    fn run(&mut self, req_irq_evt: Event, kill_evt: Event) {
        #[derive(PollToken)]
        enum Token {
            ReqIrq,
            Kill,
        }

        let wait_ctx: WaitContext<Token> = match WaitContext::build_with(&[
            (&req_irq_evt, Token::ReqIrq),
            (&kill_evt, Token::Kill),
        ]) {
            Ok(pc) => pc,
            Err(e) => {
                error!(
                    "{} failed creating vfio WaitContext: {}",
                    self.name.clone(),
                    e
                );
                return;
            }
        };

        'wait: loop {
            let events = match wait_ctx.wait() {
                Ok(v) => v,
                Err(e) => {
                    error!("{} failed polling vfio events: {}", self.name.clone(), e);
                    break;
                }
            };

            for event in events.iter().filter(|e| e.is_readable) {
                match event.token {
                    Token::ReqIrq => {
                        let mut sysfs_path = PathBuf::new();
                        sysfs_path.push("/sys/bus/pci/devices/");
                        sysfs_path.push(self.name.clone());
                        let request = VmRequest::VfioCommand {
                            vfio_path: sysfs_path,
                            add: false,
                        };
                        if self.vm_socket.send(&request).is_ok() {
                            if let Err(e) = self.vm_socket.recv::<VmResponse>() {
                                error!("{} failed to remove vfio_device: {}", self.name.clone(), e);
                            } else {
                                break 'wait;
                            }
                        }
                    }
                    Token::Kill => break 'wait,
                }
            }
        }
    }
}

enum DeviceData {
    IntelGfxData { opregion_index: u32 },
}

/// Implements the Vfio Pci device, then a pci device is added into vm
pub struct VfioPciDevice {
    device: Arc<VfioDevice>,
    config: VfioPciConfig,
    hotplug_bus_number: Option<u8>, // hot plug device has bus number specified at device creation.
    pci_address: Option<PciAddress>,
    interrupt_evt: Option<Event>,
    interrupt_resample_evt: Option<Event>,
    mmio_regions: Vec<PciBarConfiguration>,
    io_regions: Vec<PciBarConfiguration>,
    msi_cap: Option<VfioMsiCap>,
    msix_cap: Option<VfioMsixCap>,
    irq_type: Option<VfioIrqType>,
    vm_socket_mem: Tube,
    device_data: Option<DeviceData>,
    kill_evt: Option<Event>,
    worker_thread: Option<thread::JoinHandle<VfioPciWorker>>,
    vm_socket_vm: Option<Tube>,

    mmap_slots: Vec<MemSlot>,
    remap: bool,
}

impl VfioPciDevice {
    /// Constructs a new Vfio Pci device for the give Vfio device
    pub fn new(
        device: VfioDevice,
        hotplug_bus_number: Option<u8>,
        vfio_device_socket_msi: Tube,
        vfio_device_socket_msix: Tube,
        vfio_device_socket_mem: Tube,
        vfio_device_socket_vm: Option<Tube>,
    ) -> Self {
        let dev = Arc::new(device);
        let config = VfioPciConfig::new(Arc::clone(&dev));
        let mut msi_socket = Some(vfio_device_socket_msi);
        let mut msix_socket = Some(vfio_device_socket_msix);
        let mut msi_cap: Option<VfioMsiCap> = None;
        let mut msix_cap: Option<VfioMsixCap> = None;

        let mut cap_next: u32 = config.read_config::<u8>(PCI_CAPABILITY_LIST).into();
        while cap_next != 0 {
            let cap_id: u8 = config.read_config(cap_next);
            if cap_id == PCI_CAP_ID_MSI {
                if let Some(msi_socket) = msi_socket.take() {
                    msi_cap = Some(VfioMsiCap::new(&config, cap_next, msi_socket));
                }
            } else if cap_id == PCI_CAP_ID_MSIX {
                if let Some(msix_socket) = msix_socket.take() {
                    msix_cap = Some(VfioMsixCap::new(&config, cap_next, msix_socket));
                }
            }
            let offset = cap_next + PCI_MSI_NEXT_POINTER;
            cap_next = config.read_config::<u8>(offset).into();
        }

        let vendor_id: u16 = config.read_config(PCI_VENDOR_ID);
        let class_code: u8 = config.read_config(PCI_BASE_CLASS_CODE);

        let is_intel_gfx = vendor_id == INTEL_VENDOR_ID
            && class_code == PciClassCode::DisplayController.get_register_value();
        let device_data = if is_intel_gfx {
            Some(DeviceData::IntelGfxData {
                opregion_index: u32::max_value(),
            })
        } else {
            None
        };

        VfioPciDevice {
            device: dev,
            config,
            hotplug_bus_number,
            pci_address: None,
            interrupt_evt: None,
            interrupt_resample_evt: None,
            mmio_regions: Vec::new(),
            io_regions: Vec::new(),
            msi_cap,
            msix_cap,
            irq_type: None,
            vm_socket_mem: vfio_device_socket_mem,
            device_data,
            kill_evt: None,
            worker_thread: None,
            vm_socket_vm: vfio_device_socket_vm,
            mmap_slots: Vec::new(),
            remap: true,
        }
    }

    fn is_intel_gfx(&self) -> bool {
        let mut ret = false;

        if let Some(device_data) = &self.device_data {
            match *device_data {
                DeviceData::IntelGfxData { .. } => ret = true,
            }
        }

        ret
    }

    fn find_region(&self, addr: u64) -> Option<PciBarConfiguration> {
        for mmio_info in self.mmio_regions.iter() {
            if addr >= mmio_info.address() && addr < mmio_info.address() + mmio_info.size() {
                return Some(*mmio_info);
            }
        }

        None
    }

    fn enable_intx(&mut self) {
        if self.interrupt_evt.is_none() || self.interrupt_resample_evt.is_none() {
            return;
        }

        if let Some(ref interrupt_evt) = self.interrupt_evt {
            if let Err(e) = self
                .device
                .irq_enable(&[interrupt_evt], VFIO_PCI_INTX_IRQ_INDEX)
            {
                error!("{} Intx enable failed: {}", self.debug_label(), e);
                return;
            }
            if let Some(ref irq_resample_evt) = self.interrupt_resample_evt {
                if let Err(e) = self.device.irq_mask(VFIO_PCI_INTX_IRQ_INDEX) {
                    error!("{} Intx mask failed: {}", self.debug_label(), e);
                    self.disable_intx();
                    return;
                }
                if let Err(e) = self
                    .device
                    .resample_virq_enable(irq_resample_evt, VFIO_PCI_INTX_IRQ_INDEX)
                {
                    error!("{} resample enable failed: {}", self.debug_label(), e);
                    self.disable_intx();
                    return;
                }
                if let Err(e) = self.device.irq_unmask(VFIO_PCI_INTX_IRQ_INDEX) {
                    error!("{} Intx unmask failed: {}", self.debug_label(), e);
                    self.disable_intx();
                    return;
                }
            }
        }

        self.irq_type = Some(VfioIrqType::Intx);
    }

    fn disable_intx(&mut self) {
        if let Err(e) = self.device.irq_disable(VFIO_PCI_INTX_IRQ_INDEX) {
            error!("{} Intx disable failed: {}", self.debug_label(), e);
        }
        self.irq_type = None;
    }

    fn disable_irqs(&mut self) {
        match self.irq_type {
            Some(VfioIrqType::Msi) => self.disable_msi(),
            Some(VfioIrqType::Msix) => self.disable_msix(),
            _ => (),
        }

        // Above disable_msi() or disable_msix() will enable intx again.
        // so disable_intx here again.
        if let Some(VfioIrqType::Intx) = self.irq_type {
            self.disable_intx();
        }
    }

    fn enable_msi(&mut self) {
        self.disable_irqs();

        let irqfd = match &self.msi_cap {
            Some(cap) => {
                if let Some(fd) = cap.get_msi_irqfd() {
                    fd
                } else {
                    self.enable_intx();
                    return;
                }
            }
            None => {
                self.enable_intx();
                return;
            }
        };

        if let Err(e) = self.device.irq_enable(&[irqfd], VFIO_PCI_MSI_IRQ_INDEX) {
            error!("{} failed to enable msi: {}", self.debug_label(), e);
            self.enable_intx();
            return;
        }

        self.irq_type = Some(VfioIrqType::Msi);
    }

    fn disable_msi(&mut self) {
        if let Err(e) = self.device.irq_disable(VFIO_PCI_MSI_IRQ_INDEX) {
            error!("{} failed to disable msi: {}", self.debug_label(), e);
            return;
        }
        self.irq_type = None;

        self.enable_intx();
    }

    fn enable_msix(&mut self) {
        self.disable_irqs();

        let irqfds = match &self.msix_cap {
            Some(cap) => cap.get_msix_irqfds(),
            None => return,
        };

        if let Some(descriptors) = irqfds {
            if let Err(e) = self
                .device
                .irq_enable(&descriptors, VFIO_PCI_MSIX_IRQ_INDEX)
            {
                error!("{} failed to enable msix: {}", self.debug_label(), e);
                self.enable_intx();
                return;
            }
        } else {
            self.enable_intx();
            return;
        }

        self.irq_type = Some(VfioIrqType::Msix);
    }

    fn disable_msix(&mut self) {
        if let Err(e) = self.device.irq_disable(VFIO_PCI_MSIX_IRQ_INDEX) {
            error!("{} failed to disable msix: {}", self.debug_label(), e);
            return;
        }
        self.irq_type = None;

        self.enable_intx();
    }

    fn add_bar_mmap_msix(
        &self,
        bar_index: u32,
        bar_mmaps: Vec<vfio_region_sparse_mmap_area>,
    ) -> Vec<vfio_region_sparse_mmap_area> {
        let msix_cap = &self.msix_cap.as_ref().unwrap();
        let mut msix_mmaps: Vec<(u64, u64)> = Vec::new();

        if let Some(t) = msix_cap.get_msix_table(bar_index) {
            msix_mmaps.push(t);
        }
        if let Some(p) = msix_cap.get_msix_pba(bar_index) {
            msix_mmaps.push(p);
        }

        if msix_mmaps.is_empty() {
            return bar_mmaps;
        }

        let mut mmaps: Vec<vfio_region_sparse_mmap_area> = Vec::with_capacity(bar_mmaps.len());
        let pgmask = (pagesize() as u64) - 1;

        for mmap in bar_mmaps.iter() {
            let mmap_offset = mmap.offset as u64;
            let mmap_size = mmap.size as u64;
            let mut to_mmap = match VfioMsixAllocator::new(mmap_offset, mmap_size) {
                Ok(a) => a,
                Err(e) => {
                    error!("{} add_bar_mmap_msix failed: {}", self.debug_label(), e);
                    mmaps.clear();
                    return mmaps;
                }
            };

            // table/pba offsets are qword-aligned - align to page size
            for &(msix_offset, msix_size) in msix_mmaps.iter() {
                if msix_offset >= mmap_offset && msix_offset < mmap_offset + mmap_size {
                    let begin = max(msix_offset, mmap_offset) & !pgmask;
                    let end =
                        (min(msix_offset + msix_size, mmap_offset + mmap_size) + pgmask) & !pgmask;
                    if end > begin {
                        if let Err(e) = to_mmap.allocate_at(begin, end - begin) {
                            error!("add_bar_mmap_msix failed: {}", e);
                        }
                    }
                }
            }

            for mmap in to_mmap.regions {
                mmaps.push(vfio_region_sparse_mmap_area {
                    offset: mmap.0,
                    size: mmap.1 - mmap.0 + 1,
                });
            }
        }

        mmaps
    }

    fn add_bar_mmap(&self, index: u32, bar_addr: u64) -> Vec<MemSlot> {
        let mut mmaps_slots: Vec<MemSlot> = Vec::new();
        if self.device.get_region_flags(index) & VFIO_REGION_INFO_FLAG_MMAP != 0 {
            // the bar storing msix table and pba couldn't mmap.
            // these bars should be trapped, so that msix could be emulated.
            let mut mmaps = self.device.get_region_mmap(index);

            if self.msix_cap.is_some() {
                mmaps = self.add_bar_mmap_msix(index, mmaps);
            }
            if mmaps.is_empty() {
                return mmaps_slots;
            }

            for mmap in mmaps.iter() {
                let mmap_offset = mmap.offset;
                let mmap_size = mmap.size;
                let guest_map_start = bar_addr + mmap_offset;
                let region_offset = self.device.get_region_offset(index);
                let offset = region_offset + mmap_offset;
                let descriptor = match self.device.device_file().try_clone() {
                    Ok(device_file) => device_file.into(),
                    Err(_) => break,
                };
                if self
                    .vm_socket_mem
                    .send(&VmMemoryRequest::RegisterMmapMemory {
                        descriptor,
                        size: mmap_size as usize,
                        offset,
                        gpa: guest_map_start,
                        read_only: false,
                    })
                    .is_err()
                {
                    break;
                }

                let response: VmMemoryResponse = match self.vm_socket_mem.recv() {
                    Ok(res) => res,
                    Err(_) => break,
                };
                match response {
                    VmMemoryResponse::RegisterMemory { pfn: _, slot } => {
                        mmaps_slots.push(slot as MemSlot);
                    }
                    _ => break,
                }
            }
        }

        mmaps_slots
    }

    fn remove_bar_mmap(&self, mmap_slot: &MemSlot) {
        if self
            .vm_socket_mem
            .send(&VmMemoryRequest::UnregisterMemory(*mmap_slot as u32))
            .is_err()
        {
            error!("failed to send UnregisterMemory request");
            return;
        }
        if self.vm_socket_mem.recv::<VmMemoryResponse>().is_err() {
            error!("failed to receive UnregisterMemory response");
        }
    }

    fn disable_bars_mmap(&mut self) {
        for mmap_slot in self.mmap_slots.iter() {
            self.remove_bar_mmap(mmap_slot);
        }
        self.mmap_slots.clear();
    }

    fn enable_bars_mmap(&mut self) {
        self.disable_bars_mmap();

        for mmio_info in self.mmio_regions.iter() {
            if mmio_info.address() != 0 {
                let mut mmap_slots =
                    self.add_bar_mmap(mmio_info.bar_index() as u32, mmio_info.address());
                self.mmap_slots.append(&mut mmap_slots);
            }
        }

        self.remap = false;
    }

    fn close(&mut self) {
        if let Some(msi) = self.msi_cap.as_mut() {
            msi.destroy();
        }
        if let Some(msix) = self.msix_cap.as_mut() {
            msix.destroy();
        }
        self.disable_bars_mmap();
        self.device.close();
    }

    fn start_work_thread(&mut self) {
        let vm_socket = match self.vm_socket_vm.take() {
            Some(socket) => socket,
            None => return,
        };

        let req_evt = match Event::new() {
            Ok(evt) => {
                if let Err(e) = self.device.irq_enable(&[&evt], VFIO_PCI_REQ_IRQ_INDEX) {
                    error!("{} enable req_irq failed: {}", self.debug_label(), e);
                    return;
                }
                evt
            }
            Err(_) => return,
        };

        let (self_kill_evt, kill_evt) = match Event::new().and_then(|e| Ok((e.try_clone()?, e))) {
            Ok(v) => v,
            Err(e) => {
                error!(
                    "{} failed creating kill Event pair: {}",
                    self.debug_label(),
                    e
                );
                return;
            }
        };
        self.kill_evt = Some(self_kill_evt);

        let name = self.device.device_name().to_string();
        let worker_result = thread::Builder::new()
            .name("vfio_pci".to_string())
            .spawn(move || {
                let mut worker = VfioPciWorker { vm_socket, name };
                worker.run(req_evt, kill_evt);
                worker
            });

        match worker_result {
            Err(e) => {
                error!(
                    "{} failed to spawn vfio_pci worker: {}",
                    self.debug_label(),
                    e
                );
            }
            Ok(join_handle) => {
                self.worker_thread = Some(join_handle);
            }
        }
    }
}

impl PciDevice for VfioPciDevice {
    fn debug_label(&self) -> String {
        format!("vfio {} device", self.device.device_name())
    }

    fn allocate_address(
        &mut self,
        resources: &mut SystemAllocator,
    ) -> Result<PciAddress, PciDeviceError> {
        if self.pci_address.is_none() {
            let mut address = PciAddress::from_string(self.device.device_name());
            if let Some(bus_num) = self.hotplug_bus_number {
                // Caller specify pcie bus number for hotplug device
                address.bus = bus_num;
                // devfn should be 0, otherwise pcie root port couldn't detect it
                address.dev = 0;
                address.func = 0;
            }

            if resources.reserve_pci(
                Alloc::PciBar {
                    bus: address.bus,
                    dev: address.dev,
                    func: address.func,
                    bar: 0,
                },
                self.debug_label(),
            ) {
                self.pci_address = Some(address);
            }
        }
        self.pci_address.ok_or(PciDeviceError::PciAllocationFailed)
    }

    fn keep_rds(&self) -> Vec<RawDescriptor> {
        let mut rds = self.device.keep_rds();
        if let Some(ref interrupt_evt) = self.interrupt_evt {
            rds.push(interrupt_evt.as_raw_descriptor());
        }
        if let Some(ref interrupt_resample_evt) = self.interrupt_resample_evt {
            rds.push(interrupt_resample_evt.as_raw_descriptor());
        }
        rds.push(self.vm_socket_mem.as_raw_descriptor());
        if let Some(msi_cap) = &self.msi_cap {
            rds.push(msi_cap.vm_socket_irq.as_raw_descriptor());
        }
        if let Some(msix_cap) = &self.msix_cap {
            rds.push(msix_cap.config.as_raw_descriptor());
        }
        rds
    }

    fn assign_irq(
        &mut self,
        irq_evt: &Event,
        irq_resample_evt: &Event,
        _irq_num: Option<u32>,
    ) -> Option<(u32, PciInterruptPin)> {
        // Is INTx configured?
        let pin = match self.config.read_config::<u8>(PCI_INTERRUPT_PIN) {
            1 => Some(PciInterruptPin::IntA),
            2 => Some(PciInterruptPin::IntB),
            3 => Some(PciInterruptPin::IntC),
            4 => Some(PciInterruptPin::IntD),
            _ => None,
        }?;

        // Keep event/resample event references.
        self.interrupt_evt = Some(irq_evt.try_clone().ok()?);
        self.interrupt_resample_evt = Some(irq_resample_evt.try_clone().ok()?);

        // enable INTX
        self.enable_intx();

        // TODO: replace sysfs/irq value parsing with vfio interface
        //       reporting host allocated interrupt number and type.
        let mut path = PathBuf::from("/sys/bus/pci/devices");
        path.push(self.device.device_name());
        path.push("irq");
        let gsi = fs::read_to_string(path)
            .map(|v| v.trim().parse::<u32>().unwrap_or(0))
            .unwrap_or(0);

        self.config.write_config(gsi as u8, PCI_INTERRUPT_NUM);

        Some((gsi, pin))
    }

    fn allocate_io_bars(
        &mut self,
        resources: &mut SystemAllocator,
    ) -> Result<Vec<(u64, u64)>, PciDeviceError> {
        let mut ranges = Vec::new();
        let mut i = VFIO_PCI_BAR0_REGION_INDEX;
        let address = self
            .pci_address
            .expect("allocate_address must be called prior to allocate_io_bars");

        while i <= VFIO_PCI_ROM_REGION_INDEX {
            let mut low: u32 = 0xffffffff;
            let offset: u32;
            if i == VFIO_PCI_ROM_REGION_INDEX {
                offset = 0x30;
            } else {
                offset = 0x10 + i * 4;
            }
            self.config.write_config(low, offset);
            low = self.config.read_config(offset);

            let low_flag = low & 0xf;
            let is_64bit = low_flag & 0x4 == 0x4;
            let prefetchable = low_flag & 0x8 == 0x8;
            if (low_flag & 0x1 == 0 || i == VFIO_PCI_ROM_REGION_INDEX) && low != 0 {
                let mut upper: u32 = 0xffffffff;
                if is_64bit {
                    self.config.write_config(upper, offset + 4);
                    upper = self.config.read_config(offset + 4);
                }

                low &= 0xffff_fff0;
                let mut size: u64 = u64::from(upper);
                size <<= 32;
                size |= u64::from(low);
                size = !size + 1;
                let mmio_type = match is_64bit {
                    false => MmioType::Low,
                    true => MmioType::High,
                };
                let mut bar_addr: u64 = 0;
                // Don't allocate mmio for hotplug device, OS will allocate it from
                // its parent's bridge window.
                if self.hotplug_bus_number.is_none() {
                    bar_addr = resources
                        .mmio_allocator(mmio_type)
                        .allocate_with_align(
                            size,
                            Alloc::PciBar {
                                bus: address.bus,
                                dev: address.dev,
                                func: address.func,
                                bar: i as u8,
                            },
                            "vfio_bar".to_string(),
                            size,
                        )
                        .map_err(|e| PciDeviceError::IoAllocationFailed(size, e))?;
                    ranges.push((bar_addr, size));
                }
                self.mmio_regions.push(
                    PciBarConfiguration::new(
                        i as usize,
                        size,
                        if is_64bit {
                            PciBarRegionType::Memory64BitRegion
                        } else {
                            PciBarRegionType::Memory32BitRegion
                        },
                        if prefetchable {
                            PciBarPrefetchable::Prefetchable
                        } else {
                            PciBarPrefetchable::NotPrefetchable
                        },
                    )
                    .set_address(bar_addr),
                );

                low = bar_addr as u32;
                low |= low_flag;
                self.config.write_config(low, offset);
                if is_64bit {
                    upper = (bar_addr >> 32) as u32;
                    self.config.write_config(upper, offset + 4);
                }
            } else if low_flag & 0x1 == 0x1 {
                let size = !(low & 0xffff_fffc) + 1;
                self.io_regions.push(PciBarConfiguration::new(
                    i as usize,
                    u64::from(size),
                    PciBarRegionType::IoRegion,
                    PciBarPrefetchable::NotPrefetchable,
                ));
            }

            if is_64bit {
                i += 2;
            } else {
                i += 1;
            }
        }

        // Quirk, enable igd memory for guest vga arbitrate, otherwise kernel vga arbitrate
        // driver doesn't claim this vga device, then xorg couldn't boot up.
        if self.is_intel_gfx() {
            let mut cmd = self.config.read_config::<u8>(PCI_COMMAND);
            cmd |= PCI_COMMAND_MEMORY;
            self.config.write_config(cmd, PCI_COMMAND);
        }

        Ok(ranges)
    }

    fn allocate_device_bars(
        &mut self,
        resources: &mut SystemAllocator,
    ) -> Result<Vec<(u64, u64)>, PciDeviceError> {
        let mut ranges = Vec::new();

        if !self.is_intel_gfx() {
            return Ok(ranges);
        }

        // Make intel gfx's opregion as mmio bar, and allocate a gpa for it
        // then write this gpa into pci cfg register
        if let Some((index, size)) = self.device.get_cap_type_info(
            VFIO_REGION_TYPE_PCI_VENDOR_TYPE | (INTEL_VENDOR_ID as u32),
            VFIO_REGION_SUBTYPE_INTEL_IGD_OPREGION,
        ) {
            let address = self
                .pci_address
                .expect("allocate_address must be called prior to allocate_device_bars");
            let bar_addr = resources
                .mmio_allocator(MmioType::Low)
                .allocate(
                    size,
                    Alloc::PciBar {
                        bus: address.bus,
                        dev: address.dev,
                        func: address.func,
                        bar: (index * 4) as u8,
                    },
                    "vfio_bar".to_string(),
                )
                .map_err(|e| PciDeviceError::IoAllocationFailed(size, e))?;
            ranges.push((bar_addr, size));
            self.device_data = Some(DeviceData::IntelGfxData {
                opregion_index: index,
            });

            self.mmio_regions.push(
                PciBarConfiguration::new(
                    index as usize,
                    size,
                    PciBarRegionType::Memory32BitRegion,
                    PciBarPrefetchable::NotPrefetchable,
                )
                .set_address(bar_addr),
            );
            self.config.write_config(bar_addr as u32, 0xFC);
        }

        Ok(ranges)
    }

    fn get_bar_configuration(&self, bar_num: usize) -> Option<PciBarConfiguration> {
        for region in self.mmio_regions.iter().chain(self.io_regions.iter()) {
            if region.bar_index() == bar_num {
                let command: u8 = self.config.read_config(PCI_COMMAND);
                if (region.is_memory() && (command & PCI_COMMAND_MEMORY == 0)) || region.is_io() {
                    return None;
                } else {
                    return Some(*region);
                }
            }
        }

        None
    }

    fn register_device_capabilities(&mut self) -> Result<(), PciDeviceError> {
        Ok(())
    }

    fn ioevents(&self) -> Vec<(&Event, u64, Datamatch)> {
        Vec::new()
    }

    fn read_config_register(&self, reg_idx: usize) -> u32 {
        let reg: u32 = (reg_idx * 4) as u32;

        let mut config: u32 = self.config.read_config(reg);

        // Ignore IO bar
        if (0x10..=0x24).contains(&reg) {
            let bar_idx = (reg as usize - 0x10) / 4;
            if let Some(bar) = self.get_bar_configuration(bar_idx) {
                if bar.is_io() {
                    config = 0;
                }
            }
        } else if let Some(msix_cap) = &self.msix_cap {
            if msix_cap.is_msix_control_reg(reg, 4) {
                msix_cap.read_msix_control(&mut config);
            }
        }

        // Quirk for intel graphic, set stolen memory size to 0 in pci_cfg[0x51]
        if self.is_intel_gfx() && reg == 0x50 {
            config &= 0xffff00ff;
        }

        config
    }

    fn write_config_register(&mut self, reg_idx: usize, offset: u64, data: &[u8]) {
        let start = (reg_idx * 4) as u64 + offset;

        // When guest write config register at the first time, start worker thread
        if self.worker_thread.is_none() && self.vm_socket_vm.is_some() {
            self.start_work_thread();
        };

        let mut msi_change: Option<VfioMsiChange> = None;
        if let Some(msi_cap) = self.msi_cap.as_mut() {
            if msi_cap.is_msi_reg(start, data.len()) {
                msi_change = msi_cap.write_msi_reg(start, data);
            }
        }

        match msi_change {
            Some(VfioMsiChange::Enable) => self.enable_msi(),
            Some(VfioMsiChange::Disable) => self.disable_msi(),
            None => (),
        }

        msi_change = None;
        if let Some(msix_cap) = self.msix_cap.as_mut() {
            if msix_cap.is_msix_control_reg(start as u32, data.len() as u32) {
                msi_change = msix_cap.write_msix_control(data);
            }
        }
        match msi_change {
            Some(VfioMsiChange::Enable) => self.enable_msix(),
            Some(VfioMsiChange::Disable) => self.disable_msix(),
            None => (),
        }

        self.device
            .region_write(VFIO_PCI_CONFIG_REGION_INDEX, data, start);

        // if guest enable memory access, then enable bar mappable once
        if start == PCI_COMMAND as u64
            && data.len() == 2
            && data[0] & PCI_COMMAND_MEMORY == PCI_COMMAND_MEMORY
            && self.remap
        {
            self.enable_bars_mmap();
        } else if (0x10..=0x24).contains(&start) && data.len() == 4 {
            let bar_idx = (start as u32 - 0x10) / 4;
            let value: [u8; 4] = [data[0], data[1], data[2], data[3]];
            let val = u32::from_le_bytes(value);
            if val != 0xFFFFFFFF || val != 0 {
                let mut mmio_clone = self.mmio_regions.clone();
                let mut modify = false;
                for region in mmio_clone.iter_mut() {
                    if region.bar_index() == bar_idx as usize {
                        let old_addr = region.address();
                        let new_addr = val & 0xFFFFFFF0;
                        if !region.is_64bit_memory() && (old_addr as u32) != new_addr {
                            // Change 32bit bar address
                            *region = region.set_address(u64::from(new_addr));
                            self.remap = true;
                            modify = true;
                        } else if region.is_64bit_memory() && (old_addr as u32) != new_addr {
                            // Change 64bit bar low address
                            *region =
                                region.set_address(u64::from(new_addr) | ((old_addr >> 32) << 32));
                            self.remap = true;
                            modify = true;
                        }
                        break;
                    } else if region.is_64bit_memory()
                        && ((bar_idx % 2) == 1)
                        && (region.bar_index() + 1 == bar_idx as usize)
                    {
                        // Change 64bit bar high address
                        let old_addr = region.address();
                        if val != (old_addr >> 32) as u32 {
                            let mut new_addr = (u64::from(val)) << 32;
                            new_addr |= old_addr & 0xFFFFFFFF;
                            *region = region.set_address(new_addr);
                            self.remap = true;
                            modify = true;
                        }
                        break;
                    }
                }

                if modify {
                    self.mmio_regions.clear();
                    self.mmio_regions.append(&mut mmio_clone);
                    // if bar is changed under memory enabled, mmap the
                    // new bar immediately.
                    let cmd = self.config.read_config::<u8>(PCI_COMMAND);
                    if cmd & PCI_COMMAND_MEMORY == PCI_COMMAND_MEMORY {
                        self.enable_bars_mmap();
                    }
                }
            }
        }
    }

    fn read_bar(&mut self, addr: u64, data: &mut [u8]) {
        if let Some(mmio_info) = self.find_region(addr) {
            let offset = addr - mmio_info.address();
            let bar_index = mmio_info.bar_index() as u32;
            if let Some(msix_cap) = &self.msix_cap {
                if msix_cap.is_msix_table(bar_index, offset) {
                    msix_cap.read_table(offset, data);
                    return;
                } else if msix_cap.is_msix_pba(bar_index, offset) {
                    msix_cap.read_pba(offset, data);
                    return;
                }
            }
            self.device.region_read(bar_index, data, offset);
        }
    }

    fn write_bar(&mut self, addr: u64, data: &[u8]) {
        if let Some(mmio_info) = self.find_region(addr) {
            // Ignore igd opregion's write
            if let Some(device_data) = &self.device_data {
                match *device_data {
                    DeviceData::IntelGfxData { opregion_index } => {
                        if opregion_index == mmio_info.bar_index() as u32 {
                            return;
                        }
                    }
                }
            }

            let offset = addr - mmio_info.address();
            let bar_index = mmio_info.bar_index() as u32;

            if let Some(msix_cap) = self.msix_cap.as_mut() {
                if msix_cap.is_msix_table(bar_index, offset) {
                    msix_cap.write_table(offset, data);
                    return;
                } else if msix_cap.is_msix_pba(bar_index, offset) {
                    msix_cap.write_pba(offset, data);
                    return;
                }
            }

            self.device.region_write(bar_index, data, offset);
        }
    }

    fn destroy_device(&mut self) {
        self.close();
    }
}

impl Drop for VfioPciDevice {
    fn drop(&mut self) {
        if let Some(kill_evt) = self.kill_evt.take() {
            let _ = kill_evt.write(1);
        }

        if let Some(worker_thread) = self.worker_thread.take() {
            let _ = worker_thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::VfioMsixAllocator;

    #[test]
    fn no_overlap() {
        // regions [32, 95]
        let mut memory = VfioMsixAllocator::new(32, 64).unwrap();
        memory.allocate_at(0, 16).unwrap();
        memory.allocate_at(100, 16).unwrap();

        let mut iter = memory.regions.iter();
        assert_eq!(iter.next(), Some(&(32, 95)));
    }

    #[test]
    fn full_overlap() {
        // regions [32, 95]
        let mut memory = VfioMsixAllocator::new(32, 64).unwrap();
        // regions [32, 47], [64, 95]
        memory.allocate_at(48, 16).unwrap();
        // regions [64, 95]
        memory.allocate_at(32, 16).unwrap();

        let mut iter = memory.regions.iter();
        assert_eq!(iter.next(), Some(&(64, 95)));
    }

    #[test]
    fn partial_overlap_one() {
        // regions [32, 95]
        let mut memory = VfioMsixAllocator::new(32, 64).unwrap();
        // regions [32, 47], [64, 95]
        memory.allocate_at(48, 16).unwrap();
        // regions [32, 39], [64, 95]
        memory.allocate_at(40, 16).unwrap();

        let mut iter = memory.regions.iter();
        assert_eq!(iter.next(), Some(&(32, 39)));
        assert_eq!(iter.next(), Some(&(64, 95)));
    }

    #[test]
    fn partial_overlap_two() {
        // regions [32, 95]
        let mut memory = VfioMsixAllocator::new(32, 64).unwrap();
        // regions [32, 47], [64, 95]
        memory.allocate_at(48, 16).unwrap();
        // regions [32, 39], [72, 95]
        memory.allocate_at(40, 32).unwrap();

        let mut iter = memory.regions.iter();
        assert_eq!(iter.next(), Some(&(32, 39)));
        assert_eq!(iter.next(), Some(&(72, 95)));
    }

    #[test]
    fn partial_overlap_three() {
        // regions [32, 95]
        let mut memory = VfioMsixAllocator::new(32, 64).unwrap();
        // regions [32, 39], [48, 95]
        memory.allocate_at(40, 8).unwrap();
        // regions [32, 39], [48, 63], [72, 95]
        memory.allocate_at(64, 8).unwrap();
        // regions [32, 35], [76, 95]
        memory.allocate_at(36, 40).unwrap();

        let mut iter = memory.regions.iter();
        assert_eq!(iter.next(), Some(&(32, 35)));
        assert_eq!(iter.next(), Some(&(76, 95)));
    }
}
