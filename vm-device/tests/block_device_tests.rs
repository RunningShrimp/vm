//! Comprehensive tests for block device operations
//!
//! Target coverage areas:
//! - Read operations (single sector, multiple sectors, various alignments)
//! - Write operations (single sector, multiple sectors, various alignments)
//! - Flush operations
//! - Error handling (out of bounds, read-only device, I/O errors)

use vm_device::block::*;
use vm_device::block_service::*;
use vm_core::{GuestAddr, MMU, VmError, CoreError};

/// Mock MMU implementation for testing
struct MockMmu {
    memory: Vec<u8>,
    read_fail_at: Option<GuestAddr>,
    write_fail_at: Option<GuestAddr>,
}

impl MockMmu {
    fn new(size: usize) -> Self {
        Self {
            memory: vec![0u8; size],
            read_fail_at: None,
            write_fail_at: None,
        }
    }

    fn with_read_failure(mut self, addr: GuestAddr) -> Self {
        self.read_fail_at = Some(addr);
        self
    }

    fn with_write_failure(mut self, addr: GuestAddr) -> Self {
        self.write_fail_at = Some(addr);
        self
    }

    fn set_data(&mut self, offset: usize, data: &[u8]) {
        if offset + data.len() <= self.memory.len() {
            self.memory[offset..offset + data.len()].copy_from_slice(data);
        }
    }

    fn get_data(&self, offset: usize, len: usize) -> Vec<u8> {
        if offset + len <= self.memory.len() {
            self.memory[offset..offset + len].to_vec()
        } else {
            vec![0u8; len]
        }
    }
}

impl MMU for MockMmu {
    fn read(&self, addr: vm_core::GuestAddr, size: u8) -> Result<u64, VmError> {
        if let Some(fail_addr) = self.read_fail_at {
            if addr == fail_addr {
                return Err(VmError::Core(CoreError::Internal {
                    message: "Mock read failure".to_string(),
                    module: "test".to_string(),
                }));
            }
        }

        let offset = addr.0 as usize;
        if offset + size as usize > self.memory.len() {
            return Err(VmError::Core(CoreError::Internal {
                message: "Out of bounds".to_string(),
                module: "test".to_string(),
            }));
        }

        match size {
            1 => Ok(self.memory[offset] as u64),
            2 => Ok(u16::from_le_bytes([
                self.memory[offset],
                self.memory[offset + 1],
            ]) as u64),
            4 => Ok(u32::from_le_bytes([
                self.memory[offset],
                self.memory[offset + 1],
                self.memory[offset + 2],
                self.memory[offset + 3],
            ]) as u64),
            8 => Ok(u64::from_le_bytes([
                self.memory[offset],
                self.memory[offset + 1],
                self.memory[offset + 2],
                self.memory[offset + 3],
                self.memory[offset + 4],
                self.memory[offset + 5],
                self.memory[offset + 6],
                self.memory[offset + 7],
            ])),
            _ => Err(VmError::Core(CoreError::Internal {
                message: "Invalid size".to_string(),
                module: "test".to_string(),
            })),
        }
    }

    fn write(&mut self, addr: vm_core::GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        if let Some(fail_addr) = self.write_fail_at {
            if addr == fail_addr {
                return Err(VmError::Core(CoreError::Internal {
                    message: "Mock write failure".to_string(),
                    module: "test".to_string(),
                }));
            }
        }

        let offset = addr.0 as usize;
        if offset + size as usize > self.memory.len() {
            return Err(VmError::Core(CoreError::Internal {
                message: "Out of bounds".to_string(),
                module: "test".to_string(),
            }));
        }

        let bytes = val.to_le_bytes();
        self.memory[offset..offset + size as usize].copy_from_slice(&bytes[..size as usize]);
        Ok(())
    }

    fn read_bulk(&self, addr: vm_core::GuestAddr, data: &mut [u8]) -> Result<(), VmError> {
        if let Some(fail_addr) = self.read_fail_at {
            if addr == fail_addr {
                return Err(VmError::Core(CoreError::Internal {
                    message: "Mock bulk read failure".to_string(),
                    module: "test".to_string(),
                }));
            }
        }

        let offset = addr.0 as usize;
        if offset + data.len() > self.memory.len() {
            return Err(VmError::Core(CoreError::Internal {
                message: "Bulk read out of bounds".to_string(),
                module: "test".to_string(),
            }));
        }

        data.copy_from_slice(&self.memory[offset..offset + data.len()]);
        Ok(())
    }

    fn write_bulk(&mut self, addr: vm_core::GuestAddr, data: &[u8]) -> Result<(), VmError> {
        if let Some(fail_addr) = self.write_fail_at {
            if addr == fail_addr {
                return Err(VmError::Core(CoreError::Internal {
                    message: "Mock bulk write failure".to_string(),
                    module: "test".to_string(),
                }));
            }
        }

        let offset = addr.0 as usize;
        if offset + data.len() > self.memory.len() {
            return Err(VmError::Core(CoreError::Internal {
                message: "Bulk write out of bounds".to_string(),
                module: "test".to_string(),
            }));
        }

        self.memory[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    fn read_u16(&self, addr: u64) -> Result<u16, VmError> {
        self.read(GuestAddr(addr), 2).map(|v| v as u16)
    }

    fn read_u32(&self, addr: u64) -> Result<u32, VmError> {
        self.read(GuestAddr(addr), 4).map(|v| v as u32)
    }

    fn read_u64(&self, addr: u64) -> Result<u64, VmError> {
        self.read(GuestAddr(addr), 8)
    }

    fn write_u16(&mut self, addr: u64, val: u16) -> Result<(), VmError> {
        self.write(GuestAddr(addr), val as u64, 2)
    }

    fn write_u32(&mut self, addr: u64, val: u32) -> Result<(), VmError> {
        self.write(GuestAddr(addr), val as u64, 4)
    }

    fn flush(&mut self) -> Result<(), VmError> {
        Ok(())
    }
}

#[cfg(test)]
mod block_device_creation_tests {
    use super::*;

    #[test]
    fn test_virtio_block_default() {
        let block = VirtioBlock::default();
        assert_eq!(block.capacity, 0);
        assert_eq!(block.sector_size, 512);
        assert_eq!(block.read_only, false);
    }

    #[test]
    fn test_virtio_block_new() {
        let block = VirtioBlock::new(1024, 512, false);
        assert_eq!(block.capacity, 1024);
        assert_eq!(block.sector_size, 512);
        assert_eq!(block.read_only, false);
    }

    #[test]
    fn test_virtio_block_read_only() {
        let block = VirtioBlock::new(1024, 512, true);
        assert_eq!(block.capacity, 1024);
        assert_eq!(block.sector_size, 512);
        assert_eq!(block.read_only, true);
    }

    #[test]
    fn test_block_request_type_from_u32() {
        assert_eq!(
            BlockRequestType::from_u32(0),
            Some(BlockRequestType::In)
        );
        assert_eq!(
            BlockRequestType::from_u32(1),
            Some(BlockRequestType::Out)
        );
        assert_eq!(
            BlockRequestType::from_u32(4),
            Some(BlockRequestType::Flush)
        );
        assert_eq!(
            BlockRequestType::from_u32(8),
            Some(BlockRequestType::GetId)
        );
        assert_eq!(BlockRequestType::from_u32(999), None);
    }

    #[test]
    fn test_block_status_values() {
        assert_eq!(BlockStatus::Ok as u8, 0);
        assert_eq!(BlockStatus::IoErr as u8, 1);
        assert_eq!(BlockStatus::Unsupported as u8, 2);
    }

    #[test]
    fn test_virtio_event_values() {
        assert_eq!(VirtioEvent::Notify as u8, 0);
        assert_eq!(VirtioEvent::Wake as u8, 16);
        assert_eq!(VirtioEvent::IndexMatch as u8, 32);
    }

    #[test]
    fn test_block_request_header_size() {
        let header = BlockRequestHeader {
            req_type: 0,
            reserved: 0,
            sector: 0,
        };
        assert_eq!(std::mem::size_of::<BlockRequestHeader>(), 16);
    }
}

#[cfg(test)]
mod block_device_service_tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = BlockDeviceService::new(1024, 512, false);
        assert_eq!(service.capacity(), 1024);
        assert_eq!(service.sector_size(), 512);
        assert!(!service.is_read_only());
    }

    #[test]
    fn test_service_features_default() {
        let service = BlockDeviceService::new(1024, 512, false);
        let features = service.get_features();
        assert!(features & (1 << 6) != 0); // VIRTIO_BLK_F_BLK_SIZE
        assert!(features & (1 << 9) != 0); // VIRTIO_BLK_F_FLUSH
        assert!(features & (1 << 5) == 0); // VIRTIO_BLK_F_RO (not set)
    }

    #[test]
    fn test_service_features_read_only() {
        let service = BlockDeviceService::new(1024, 512, true);
        let features = service.get_features();
        assert!(features & (1 << 5) != 0); // VIRTIO_BLK_F_RO
    }

    #[test]
    fn test_process_read_request_valid() {
        let service = BlockDeviceService::new(1024, 512, false);
        let mut mmu = MockMmu::new(0x10000);

        // Setup request in MMU
        let req_addr = GuestAddr(0x1000);
        let data_addr = GuestAddr(0x2000);
        let status_addr = GuestAddr(0x3000);

        mmu.set_data(0x1000, &[0u8; 16]); // Request header (type=0, sector=0)
        mmu.set_data(0x3000, &[0u8; 1]);  // Status

        let status = service.process_request(
            &mut mmu,
            req_addr,
            data_addr,
            512,
            status_addr,
        );

        // Should fail because there's no file backing
        assert_eq!(status, BlockStatus::IoErr);
    }

    #[test]
    fn test_process_read_request_out_of_bounds() {
        let service = BlockDeviceService::new(1024, 512, false);
        let mut mmu = MockMmu::new(0x10000);

        let req_addr = GuestAddr(0x1000);
        let data_addr = GuestAddr(0x2000);
        let status_addr = GuestAddr(0x3000);

        // Request sector 2000 (beyond capacity of 1024)
        mmu.set_data(0x1000, &[
            0u8, 0u8, 0u8, 0u8,   // type = 0 (read)
            0u8, 0u8, 0u8, 0u8,   // reserved
            0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0x07, // sector = 2048
        ]);
        mmu.set_data(0x3000, &[0u8; 1]);

        let status = service.process_request(
            &mut mmu,
            req_addr,
            data_addr,
            512,
            status_addr,
        );

        assert_eq!(status, BlockStatus::IoErr);
    }

    #[test]
    fn test_process_write_request_read_only() {
        let service = BlockDeviceService::new(1024, 512, true);
        let mut mmu = MockMmu::new(0x10000);

        let req_addr = GuestAddr(0x1000);
        let data_addr = GuestAddr(0x2000);
        let status_addr = GuestAddr(0x3000);

        // Write request (type=1)
        mmu.set_data(0x1000, &[
            1u8, 0u8, 0u8, 0u8,   // type = 1 (write)
            0u8, 0u8, 0u8, 0u8,   // reserved
            0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, // sector = 0
        ]);
        mmu.set_data(0x3000, &[0u8; 1]);

        let status = service.process_request(
            &mut mmu,
            req_addr,
            data_addr,
            512,
            status_addr,
        );

        assert_eq!(status, BlockStatus::IoErr);
    }

    #[test]
    fn test_process_flush_request() {
        let service = BlockDeviceService::new(1024, 512, false);

        // Flush doesn't need MMU for data access
        let status = service.handle_flush_request();
        assert_eq!(status, BlockStatus::Ok);
    }

    #[test]
    fn test_process_flush_request_read_only() {
        let service = BlockDeviceService::new(1024, 512, true);

        // Flush on read-only device should succeed (no-op)
        let status = service.handle_flush_request();
        assert_eq!(status, BlockStatus::Ok);
    }

    #[test]
    fn test_process_unsupported_request_type() {
        let service = BlockDeviceService::new(1024, 512, false);
        let mut mmu = MockMmu::new(0x10000);

        let req_addr = GuestAddr(0x1000);
        let data_addr = GuestAddr(0x2000);
        let status_addr = GuestAddr(0x3000);

        // Invalid request type (99)
        mmu.set_data(0x1000, &[
            99u8, 0u8, 0u8, 0u8,  // type = 99 (invalid)
            0u8, 0u8, 0u8, 0u8,   // reserved
            0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, // sector = 0
        ]);
        mmu.set_data(0x3000, &[0u8; 1]);

        let status = service.process_request(
            &mut mmu,
            req_addr,
            data_addr,
            512,
            status_addr,
        );

        assert_eq!(status, BlockStatus::Unsupported);
    }

    #[test]
    fn test_process_request_mmu_read_failure() {
        let service = BlockDeviceService::new(1024, 512, false);
        let mut mmu = MockMmu::new(0x10000).with_read_failure(GuestAddr(0x1000));

        let req_addr = GuestAddr(0x1000);
        let data_addr = GuestAddr(0x2000);
        let status_addr = GuestAddr(0x3000);

        let status = service.process_request(
            &mut mmu,
            req_addr,
            data_addr,
            512,
            status_addr,
        );

        assert_eq!(status, BlockStatus::IoErr);
    }

    #[test]
    fn test_process_request_mmu_write_failure() {
        let service = BlockDeviceService::new(1024, 512, false);
        let mut mmu = MockMmu::new(0x10000).with_write_failure(GuestAddr(0x3000));

        let req_addr = GuestAddr(0x1000);
        let data_addr = GuestAddr(0x2000);
        let status_addr = GuestAddr(0x3000);

        mmu.set_data(0x1000, &[0u8; 16]); // Valid request

        let status = service.process_request(
            &mut mmu,
            req_addr,
            data_addr,
            512,
            status_addr,
        );

        // Should fail when trying to write status
        assert_eq!(status, BlockStatus::IoErr);
    }
}

#[cfg(test)]
mod mmio_register_tests {
    use super::*;

    #[test]
    fn test_mmio_default() {
        let mmio = VirtioBlockMmio::default();
        assert_eq!(mmio.selected_queue, 0);
        assert_eq!(mmio.queue_size, 128);
        assert_eq!(mmio.device_status, 0);
        assert_eq!(mmio.used_idx, 0);
    }

    #[test]
    fn test_mmio_new() {
        let mmio = VirtioBlockMmio::new();
        assert_eq!(mmio.selected_queue, 0);
        assert_eq!(mmio.queue_size, 128);
    }

    #[test]
    fn test_mmio_new_with_capacity() {
        let mmio = VirtioBlockMmio::new_with_capacity(1024);
        assert_eq!(mmio.selected_queue, 0);
        assert_eq!(mmio.queue_size, 128);
    }

    #[test]
    fn test_mmio_read_selected_queue() {
        let mut mmio = VirtioBlockMmio::new();
        mmio.selected_queue = 42;

        let val = mmio_read(&mmio, 0x00, 4);
        assert_eq!(val, 42);
    }

    #[test]
    fn test_mmio_read_queue_size() {
        let mut mmio = VirtioBlockMmio::new();
        mmio.queue_size = 256;

        let val = mmio_read(&mmio, 0x04, 4);
        assert_eq!(val, 256);
    }

    #[test]
    fn test_mmio_read_desc_addr() {
        let mut mmio = VirtioBlockMmio::new();
        mmio.desc_addr = GuestAddr(0x1000);

        let val = mmio_read(&mmio, 0x08, 8);
        assert_eq!(val, 0x1000);
    }

    #[test]
    fn test_mmio_read_avail_addr() {
        let mut mmio = VirtioBlockMmio::new();
        mmio.avail_addr = GuestAddr(0x2000);

        let val = mmio_read(&mmio, 0x10, 8);
        assert_eq!(val, 0x2000);
    }

    #[test]
    fn test_mmio_read_used_addr() {
        let mut mmio = VirtioBlockMmio::new();
        mmio.used_addr = GuestAddr(0x3000);

        let val = mmio_read(&mmio, 0x18, 8);
        assert_eq!(val, 0x3000);
    }

    #[test]
    fn test_mmio_read_device_status() {
        let mut mmio = VirtioBlockMmio::new();
        mmio.device_status = 0xFF;

        let val = mmio_read(&mmio, 0x20, 4);
        assert_eq!(val, 0xFF);
    }

    #[test]
    fn test_mmio_read_driver_features() {
        let mut mmio = VirtioBlockMmio::new();
        mmio.driver_features = 0x12345678;

        let val = mmio_read(&mmio, 0x24, 4);
        assert_eq!(val, 0x12345678);
    }

    #[test]
    fn test_mmio_read_interrupt_status() {
        let mut mmio = VirtioBlockMmio::new();
        mmio.interrupt_status = 0x01;

        let val = mmio_read(&mmio, 0x28, 4);
        assert_eq!(val, 0x01);
    }

    #[test]
    fn test_mmio_read_used_idx() {
        let mut mmio = VirtioBlockMmio::new();
        mmio.used_idx = 100;

        let val = mmio_read(&mmio, 0x30, 4);
        assert_eq!(val, 100);
    }

    #[test]
    fn test_mmio_read_cause_evt() {
        let mut mmio = VirtioBlockMmio::new();
        mmio.cause_evt = 0x123456789ABCDEF0;

        let val = mmio_read(&mmio, 0x38, 8);
        assert_eq!(val, 0x123456789ABCDEF0);
    }

    #[test]
    fn test_mmio_read_undefined_offset() {
        let mmio = VirtioBlockMmio::new();
        let val = mmio_read(&mmio, 0xFF, 4);
        assert_eq!(val, 0);
    }

    #[test]
    fn test_mmio_write_selected_queue() {
        let mut mmio = VirtioBlockMmio::new();
        mmio_write(&mut mmio, 0x00, 42, 4);
        assert_eq!(mmio.selected_queue, 42);
    }

    #[test]
    fn test_mmio_write_queue_size() {
        let mut mmio = VirtioBlockMmio::new();
        mmio_write(&mut mmio, 0x04, 256, 4);
        assert_eq!(mmio.queue_size, 256);
    }

    #[test]
    fn test_mmio_write_desc_addr() {
        let mut mmio = VirtioBlockMmio::new();
        mmio_write(&mut mmio, 0x08, 0x1000, 8);
        assert_eq!(mmio.desc_addr, GuestAddr(0x1000));
    }

    #[test]
    fn test_mmio_write_avail_addr() {
        let mut mmio = VirtioBlockMmio::new();
        mmio_write(&mut mmio, 0x10, 0x2000, 8);
        assert_eq!(mmio.avail_addr, GuestAddr(0x2000));
    }

    #[test]
    fn test_mmio_write_used_addr() {
        let mut mmio = VirtioBlockMmio::new();
        mmio_write(&mut mmio, 0x18, 0x3000, 8);
        assert_eq!(mmio.used_addr, GuestAddr(0x3000));
    }

    #[test]
    fn test_mmio_write_device_status() {
        let mut mmio = VirtioBlockMmio::new();
        mmio_write(&mut mmio, 0x20, 0xFF, 4);
        assert_eq!(mmio.device_status, 0xFF);
    }

    #[test]
    fn test_mmio_write_driver_features() {
        let mut mmio = VirtioBlockMmio::new();
        mmio_write(&mut mmio, 0x24, 0x12345678, 4);
        assert_eq!(mmio.driver_features, 0x12345678);
    }

    #[test]
    fn test_mmio_write_interrupt_status() {
        let mut mmio = VirtioBlockMmio::new();
        mmio_write(&mut mmio, 0x28, 0x01, 4);
        assert_eq!(mmio.interrupt_status, 0x01);
    }

    #[test]
    fn test_mmio_write_used_idx() {
        let mut mmio = VirtioBlockMmio::new();
        mmio_write(&mut mmio, 0x30, 100, 4);
        assert_eq!(mmio.used_idx, 100);
    }

    #[test]
    fn test_mmio_write_cause_evt() {
        let mut mmio = VirtioBlockMmio::new();
        mmio_write(&mut mmio, 0x38, 0x123456789ABCDEF0, 8);
        assert_eq!(mmio.cause_evt, 0x123456789ABCDEF0);
    }

    #[test]
    fn test_mmio_write_undefined_offset() {
        let mut mmio = VirtioBlockMmio::new();
        let original_selected_queue = mmio.selected_queue;

        // Writing to undefined offset should not change anything
        mmio_write(&mut mmio, 0xFF, 42, 4);
        assert_eq!(mmio.selected_queue, original_selected_queue);
    }
}
