// Copyright 2022 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Wraps VfioContainer for virtio-iommu implementation

use std::sync::Arc;

use base::{AsRawDescriptor, AsRawDescriptors, RawDescriptor};
use sync::Mutex;
use vm_memory::{GuestAddress, GuestMemory};

use crate::virtio::iommu::memory_mapper::{
    Error as MemoryMapperError, MappingInfo, MemoryMapper, Permission,
};
use crate::VfioContainer;

pub struct VfioWrapper {
    container: Arc<Mutex<VfioContainer>>,
    mem: GuestMemory,
}

impl VfioWrapper {
    pub fn new(container: Arc<Mutex<VfioContainer>>, mem: GuestMemory) -> Self {
        Self { container, mem }
    }
}

impl MemoryMapper for VfioWrapper {
    fn add_map(&mut self, mut map: MappingInfo) -> Result<(), MemoryMapperError> {
        map.gpa = GuestAddress(
            self.mem
                .get_host_address_range(map.gpa, map.size as usize)
                .map_err(MemoryMapperError::GetHostAddress)? as u64,
        );
        // Safe because both guest and host address are guaranteed by
        // get_host_address_range() to be valid.
        unsafe {
            self.container.lock().vfio_dma_map(
                map.iova,
                map.size,
                map.gpa.offset(),
                (map.perm as u8 & Permission::Write as u8) != 0,
            )
        }
        .map_err(MemoryMapperError::Vfio)
    }

    fn remove_map(&mut self, iova_start: u64, size: u64) -> Result<(), MemoryMapperError> {
        iova_start
            .checked_add(size)
            .ok_or(MemoryMapperError::IntegerOverflow)?;
        self.container
            .lock()
            .vfio_dma_unmap(iova_start, size)
            .map_err(MemoryMapperError::Vfio)
    }

    fn get_mask(&self) -> Result<u64, MemoryMapperError> {
        self.container
            .lock()
            .vfio_get_iommu_page_size_mask()
            .map_err(MemoryMapperError::Vfio)
    }

    fn translate(&self, _iova: u64, _size: u64) -> Result<GuestAddress, MemoryMapperError> {
        Err(MemoryMapperError::Unimplemented)
    }
}

impl AsRawDescriptors for VfioWrapper {
    fn as_raw_descriptors(&self) -> Vec<RawDescriptor> {
        vec![self.container.lock().as_raw_descriptor()]
    }
}
