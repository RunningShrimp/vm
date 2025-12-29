//! Comprehensive tests for VirtIO device emulation
//!
//! Target coverage areas:
//! - Basic queue operations (create, pop, add_used)
//! - Descriptor chain operations
//! - Batch operations for performance
//! - Error handling (circular references, chain too long)

use vm_core::{CoreError, GuestAddr, MMU, VmError};
use vm_device::virtio::*;

/// Mock MMU with configurable queue memory layout
struct QueueMmu {
    desc_table: Vec<u8>,
    avail_ring: Vec<u8>,
    used_ring: Vec<u8>,
    data_buffer: Vec<u8>,
    fail_reads: bool,
    fail_writes: bool,
}

impl QueueMmu {
    /// Create a new QueueMmu with specified queue size
    fn new(queue_size: u16) -> Self {
        // Each descriptor is 16 bytes
        let desc_size = queue_size as usize * 16;
        // Available ring: flags(2) + idx(2) + ring[queue_size](2) + used_event(2)
        let avail_size = 4 + queue_size as usize * 2 + 2;
        // Used ring: flags(2) + idx(2) + used[queue_size](8) + avail_event(2)
        let used_size = 4 + queue_size as usize * 8 + 2;

        Self {
            desc_table: vec![0u8; desc_size],
            avail_ring: vec![0u8; avail_size],
            used_ring: vec![0u8; used_size],
            data_buffer: vec![0u8; 0x1000],
            fail_reads: false,
            fail_writes: false,
        }
    }

    /// Setup a descriptor in the descriptor table
    fn set_descriptor(&mut self, index: u16, addr: u64, len: u32, flags: u16, next: Option<u16>) {
        let offset = index as usize * 16;
        let addr_bytes = addr.to_le_bytes();
        let len_bytes = len.to_le_bytes();
        let flags_bytes = flags.to_le_bytes();
        let next_bytes = next.unwrap_or(0).to_le_bytes();

        self.desc_table[offset..offset + 8].copy_from_slice(&addr_bytes);
        self.desc_table[offset + 8..offset + 12].copy_from_slice(&len_bytes);
        self.desc_table[offset + 12..offset + 14].copy_from_slice(&flags_bytes);
        self.desc_table[offset + 14..offset + 16].copy_from_slice(&next_bytes);
    }

    /// Set available ring index
    fn set_avail_idx(&mut self, idx: u16) {
        let bytes = idx.to_le_bytes();
        self.avail_ring[2..4].copy_from_slice(&bytes);
    }

    /// Add a descriptor index to the available ring
    fn set_avail_desc(&mut self, ring_index: u16, desc_index: u16) {
        let offset = 4 + ring_index as usize * 2;
        let bytes = desc_index.to_le_bytes();
        self.avail_ring[offset..offset + 2].copy_from_slice(&bytes);
    }

    /// Get used ring index
    fn get_used_idx(&self) -> u16 {
        u16::from_le_bytes([self.used_ring[2], self.used_ring[3]])
    }

    /// Read used element
    fn get_used_elem(&self, ring_index: u16, queue_size: u16) -> (u32, u32) {
        let offset = 4 + (ring_index % queue_size) as usize * 8;
        let id = u32::from_le_bytes([
            self.used_ring[offset],
            self.used_ring[offset + 1],
            self.used_ring[offset + 2],
            self.used_ring[offset + 3],
        ]);
        let len = u32::from_le_bytes([
            self.used_ring[offset + 4],
            self.used_ring[offset + 5],
            self.used_ring[offset + 6],
            self.used_ring[offset + 7],
        ]);
        (id, len)
    }

    /// Set data in buffer
    fn set_data(&mut self, offset: usize, data: &[u8]) {
        if offset + data.len() <= self.data_buffer.len() {
            self.data_buffer[offset..offset + data.len()].copy_from_slice(data);
        }
    }

    /// Get data from buffer
    fn get_data(&self, offset: usize, len: usize) -> Vec<u8> {
        if offset + len <= self.data_buffer.len() {
            self.data_buffer[offset..offset + len].to_vec()
        } else {
            vec![0u8; len]
        }
    }

    fn with_read_fail(mut self) -> Self {
        self.fail_reads = true;
        self
    }

    fn with_write_fail(mut self) -> Self {
        self.fail_writes = true;
        self
    }
}

impl MMU for QueueMmu {
    fn read(&self, addr: GuestAddr, size: u8) -> Result<u64, VmError> {
        if self.fail_reads {
            return Err(VmError::Core(CoreError::Internal {
                message: "Mock read failure".to_string(),
                module: "test".to_string(),
            }));
        }

        let addr = addr.0 as usize;

        // Descriptor table at 0x1000
        if addr >= 0x1000 && addr < 0x1000 + self.desc_table.len() {
            let offset = addr - 0x1000;
            return read_bytes(&self.desc_table, offset, size);
        }

        // Available ring at 0x2000
        if addr >= 0x2000 && addr < 0x2000 + self.avail_ring.len() {
            let offset = addr - 0x2000;
            return read_bytes(&self.avail_ring, offset, size);
        }

        // Used ring at 0x3000
        if addr >= 0x3000 && addr < 0x3000 + self.used_ring.len() {
            let offset = addr - 0x3000;
            return read_bytes(&self.used_ring, offset, size);
        }

        // Data buffer at 0x4000
        if addr >= 0x4000 && addr < 0x4000 + self.data_buffer.len() {
            let offset = addr - 0x4000;
            return read_bytes(&self.data_buffer, offset, size);
        }

        Err(VmError::Core(CoreError::Internal {
            message: "MMU read out of bounds".to_string(),
            module: "test".to_string(),
        }))
    }

    fn write(&mut self, addr: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        if self.fail_writes {
            return Err(VmError::Core(CoreError::Internal {
                message: "Mock write failure".to_string(),
                module: "test".to_string(),
            }));
        }

        let addr = addr.0 as usize;
        let bytes = val.to_le_bytes();

        // Used ring at 0x3000 (writable)
        if addr >= 0x3000 && addr < 0x3000 + self.used_ring.len() {
            let offset = addr - 0x3000;
            if offset + size as usize <= self.used_ring.len() {
                self.used_ring[offset..offset + size as usize]
                    .copy_from_slice(&bytes[..size as usize]);
                return Ok(());
            }
        }

        // Data buffer at 0x4000 (writable)
        if addr >= 0x4000 && addr < 0x4000 + self.data_buffer.len() {
            let offset = addr - 0x4000;
            if offset + size as usize <= self.data_buffer.len() {
                self.data_buffer[offset..offset + size as usize]
                    .copy_from_slice(&bytes[..size as usize]);
                return Ok(());
            }
        }

        Err(VmError::Core(CoreError::Internal {
            message: "MMU write out of bounds".to_string(),
            module: "test".to_string(),
        }))
    }

    fn read_bulk(&self, addr: GuestAddr, data: &mut [u8]) -> Result<(), VmError> {
        if self.fail_reads {
            return Err(VmError::Core(CoreError::Internal {
                message: "Bulk read failure".to_string(),
                module: "test".to_string(),
            }));
        }

        let addr = addr.0 as usize;

        // Data buffer at 0x4000
        if addr >= 0x4000 && addr + data.len() <= 0x4000 + self.data_buffer.len() {
            let offset = addr - 0x4000;
            data.copy_from_slice(&self.data_buffer[offset..offset + data.len()]);
            return Ok(());
        }

        Err(VmError::Core(CoreError::Internal {
            message: "Bulk read out of bounds".to_string(),
            module: "test".to_string(),
        }))
    }

    fn write_bulk(&mut self, addr: GuestAddr, data: &[u8]) -> Result<(), VmError> {
        if self.fail_writes {
            return Err(VmError::Core(CoreError::Internal {
                message: "Bulk write failure".to_string(),
                module: "test".to_string(),
            }));
        }

        let addr = addr.0 as usize;

        // Data buffer at 0x4000
        if addr >= 0x4000 && addr + data.len() <= 0x4000 + self.data_buffer.len() {
            let offset = addr - 0x4000;
            self.data_buffer[offset..offset + data.len()].copy_from_slice(data);
            return Ok(());
        }

        Err(VmError::Core(CoreError::Internal {
            message: "Bulk write out of bounds".to_string(),
            module: "test".to_string(),
        }))
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

fn read_bytes(data: &[u8], offset: usize, size: u8) -> Result<u64, VmError> {
    if offset + size as usize > data.len() {
        return Err(VmError::Core(CoreError::Internal {
            message: "Read out of bounds".to_string(),
            module: "test".to_string(),
        }));
    }

    match size {
        1 => Ok(data[offset] as u64),
        2 => Ok(u16::from_le_bytes([data[offset], data[offset + 1]]) as u64),
        4 => Ok(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as u64),
        8 => Ok(u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ])),
        _ => Err(VmError::Core(CoreError::Internal {
            message: "Invalid read size".to_string(),
            module: "test".to_string(),
        })),
    }
}

#[cfg(test)]
mod queue_tests {
    use super::*;

    #[test]
    fn test_queue_creation() {
        let queue = Queue::new(256);
        assert_eq!(queue.size, 256);
        assert_eq!(queue.desc_addr, 0);
        assert_eq!(queue.avail_addr, 0);
        assert_eq!(queue.used_addr, 0);
        assert_eq!(queue.last_avail_idx, 0);
    }

    #[test]
    fn test_queue_pop_empty() {
        let mut mmu = QueueMmu::new(16);
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        // No descriptors available
        let result = queue.pop(&mmu);
        assert!(result.is_none());
    }

    #[test]
    fn test_queue_pop_single_descriptor() {
        let mut mmu = QueueMmu::new(16);
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        // Setup a single descriptor
        mmu.set_descriptor(0, 0x4000, 512, 0, None);
        mmu.set_avail_idx(1);
        mmu.set_avail_desc(0, 0);

        // Pop the descriptor
        let chain = queue.pop(&mmu).unwrap();
        assert_eq!(chain.head_index, 0);
        assert_eq!(chain.descs.len(), 1);
        assert_eq!(chain.descs[0].addr, 0x4000);
        assert_eq!(chain.descs[0].len, 512);
        assert_eq!(chain.descs[0].flags, 0);
        assert_eq!(chain.descs[0].next, None);
    }

    #[test]
    fn test_queue_pop_descriptor_chain() {
        let mut mmu = QueueMmu::new(16);
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        // Setup a chain: desc 0 -> desc 1 -> desc 2
        mmu.set_descriptor(0, 0x4000, 256, 1, Some(1)); // VRING_DESC_F_NEXT = 1
        mmu.set_descriptor(1, 0x4100, 256, 1, Some(2));
        mmu.set_descriptor(2, 0x4200, 512, 0, None);
        mmu.set_avail_idx(1);
        mmu.set_avail_desc(0, 0);

        // Pop the chain
        let chain = queue.pop(&mmu).unwrap();
        assert_eq!(chain.head_index, 0);
        assert_eq!(chain.descs.len(), 3);
        assert_eq!(chain.descs[0].addr, 0x4000);
        assert_eq!(chain.descs[1].addr, 0x4100);
        assert_eq!(chain.descs[2].addr, 0x4200);
    }

    #[test]
    fn test_queue_pop_batch_empty() {
        let mut mmu = QueueMmu::new(16);
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        let batch = queue.pop_batch(&mmu, 5);
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn test_queue_pop_batch_multiple() {
        let mut mmu = QueueMmu::new(16);
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        // Setup 3 descriptors
        for i in 0..3 {
            mmu.set_descriptor(i, 0x4000 + i as u64 * 512, 512, 0, None);
        }
        mmu.set_avail_idx(3);
        mmu.set_avail_desc(0, 0);
        mmu.set_avail_desc(1, 1);
        mmu.set_avail_desc(2, 2);

        // Pop up to 5, but only 3 available
        let batch = queue.pop_batch(&mmu, 5);
        assert_eq!(batch.len(), 3);
        assert_eq!(batch[0].head_index, 0);
        assert_eq!(batch[1].head_index, 1);
        assert_eq!(batch[2].head_index, 2);
    }

    #[test]
    fn test_queue_add_used() {
        let mut mmu = QueueMmu::new(16);
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        // Add a used descriptor
        queue.add_used(&mut mmu, 5, 512);

        // Check used ring
        assert_eq!(mmu.get_used_idx(), 1);
        let (id, len) = mmu.get_used_elem(0, 16);
        assert_eq!(id, 5);
        assert_eq!(len, 512);
    }

    #[test]
    fn test_queue_add_used_batch() {
        let mut mmu = QueueMmu::new(16);
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        // Add multiple used descriptors
        let entries = vec![(1, 256), (2, 512), (3, 1024)];
        queue.add_used_batch(&mut mmu, &entries);

        // Check used ring
        assert_eq!(mmu.get_used_idx(), 3);
        let (id0, len0) = mmu.get_used_elem(0, 16);
        assert_eq!(id0, 1);
        assert_eq!(len0, 256);

        let (id1, len1) = mmu.get_used_elem(1, 16);
        assert_eq!(id1, 2);
        assert_eq!(len1, 512);

        let (id2, len2) = mmu.get_used_elem(2, 16);
        assert_eq!(id2, 3);
        assert_eq!(len2, 1024);
    }

    #[test]
    fn test_queue_add_used_batch_empty() {
        let mut mmu = QueueMmu::new(16);
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        let original_idx = mmu.get_used_idx();

        // Add empty batch
        queue.add_used_batch(&mut mmu, &[]);

        // Should not change
        assert_eq!(mmu.get_used_idx(), original_idx);
    }

    #[test]
    fn test_queue_signal_used() {
        let mut mmu = QueueMmu::new(16);
        let queue = Queue::new(16);

        // signal_used is a no-op in current implementation
        queue.signal_used(&mut mmu);
        // Should not panic
    }
}

#[cfg(test)]
mod descriptor_chain_tests {
    use super::*;

    #[test]
    fn test_desc_chain_new() {
        let mmu = QueueMmu::new(16);
        let chain = DescChain::new(&mmu, 0x1000, 0);

        // With empty MMU, should return empty chain
        assert_eq!(chain.head_index, 0);
        assert_eq!(chain.descs.len(), 0);
    }

    #[test]
    fn test_desc_chain_try_new_single() {
        let mut mmu = QueueMmu::new(16);
        mmu.set_descriptor(0, 0x4000, 512, 0, None);

        let chain = DescChain::try_new(&mmu, 0x1000, 0).unwrap();
        assert_eq!(chain.head_index, 0);
        assert_eq!(chain.descs.len(), 1);
        assert_eq!(chain.descs[0].addr, 0x4000);
        assert_eq!(chain.descs[0].len, 512);
    }

    #[test]
    fn test_desc_chain_try_new_circular_reference() {
        let mut mmu = QueueMmu::new(16);

        // Create circular reference: 0 -> 1 -> 0
        mmu.set_descriptor(0, 0x4000, 512, 1, Some(1));
        mmu.set_descriptor(1, 0x4100, 512, 1, Some(0));

        let result = DescChain::try_new(&mmu, 0x1000, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_desc_chain_try_new_too_long() {
        let mut mmu = QueueMmu::new(256);

        // Create a chain that's too long (>256 descriptors)
        for i in 0..260u16 {
            let next = if i < 259 { Some(i + 1) } else { None };
            mmu.set_descriptor(i, 0x4000 + i as u64 * 16, 16, 1, next);
        }

        let result = DescChain::try_new(&mmu, 0x1000, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_desc_from_memory() {
        let mut mmu = QueueMmu::new(16);
        mmu.set_descriptor(5, 0x123456789ABCDEF0, 0x1000, 0x1234, Some(42));

        let desc = Desc::from_memory(&mmu, 0x1000, 5);
        assert_eq!(desc.addr, 0x123456789ABCDEF0);
        assert_eq!(desc.len, 0x1000);
        assert_eq!(desc.flags, 0x1234);
        assert_eq!(desc.next, Some(42));
    }

    #[test]
    fn test_desc_from_memory_no_next() {
        let mut mmu = QueueMmu::new(16);
        mmu.set_descriptor(5, 0x4000, 512, 0, None);

        let desc = Desc::from_memory(&mmu, 0x1000, 5);
        assert_eq!(desc.addr, 0x4000);
        assert_eq!(desc.len, 512);
        assert_eq!(desc.flags, 0);
        assert_eq!(desc.next, None);
    }

    #[test]
    fn test_desc_from_memory_with_next_flag() {
        let mut mmu = QueueMmu::new(16);

        // Set NEXT flag (bit 0)
        mmu.set_descriptor(5, 0x4000, 512, 0x01, Some(10));

        let desc = Desc::from_memory(&mmu, 0x1000, 5);
        assert_eq!(desc.next, Some(10));
    }

    #[test]
    fn test_desc_from_memory_without_next_flag() {
        let mut mmu = QueueMmu::new(16);

        // Don't set NEXT flag
        mmu.set_descriptor(5, 0x4000, 512, 0x02, Some(10)); // WRITE flag only

        let desc = Desc::from_memory(&mmu, 0x1000, 5);
        // NEXT flag not set, so next should be None even if value is present
        assert_eq!(desc.next, None);
    }

    #[test]
    fn test_desc_from_memory_mmu_failure() {
        let mmu = QueueMmu::new(16).with_read_fail();

        let desc = Desc::from_memory(&mmu, 0x1000, 0);
        // Should return default values on MMU failure
        assert_eq!(desc.addr, 0);
        assert_eq!(desc.len, 0);
        assert_eq!(desc.flags, 0);
        assert_eq!(desc.next, None);
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_queue_pop_mmu_read_failure() {
        let mmu = QueueMmu::new(16).with_read_fail();
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        let result = queue.pop(&mmu);
        assert!(result.is_none());
    }

    #[test]
    fn test_queue_add_used_mmu_write_failure() {
        let mmu = QueueMmu::new(16).with_write_fail();
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        // Should not panic on write failure
        queue.add_used(&mut mmu, 0, 512);
    }

    #[test]
    fn test_queue_pop_batch_mmu_read_failure() {
        let mmu = QueueMmu::new(16).with_read_fail();
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        let batch = queue.pop_batch(&mmu, 5);
        assert_eq!(batch.len(), 0);
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_queue_max_size() {
        let queue = Queue::new(u16::MAX);
        assert_eq!(queue.size, u16::MAX);
    }

    #[test]
    fn test_queue_wraparound() {
        let mut mmu = QueueMmu::new(16);
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;
        queue.last_avail_idx = u16::MAX - 5;

        // Setup descriptor
        mmu.set_descriptor(0, 0x4000, 512, 0, None);
        mmu.set_avail_idx(u16::MAX - 4);
        mmu.set_avail_desc((u16::MAX - 4) % 16, 0);

        let chain = queue.pop(&mmu).unwrap();
        assert_eq!(chain.head_index, 0);
        // Should wrap around correctly
        assert_eq!(queue.last_avail_idx, u16::MAX - 3);
    }

    #[test]
    fn test_empty_descriptor_chain() {
        let mmu = QueueMmu::new(16);
        let chain = DescChain::new(&mmu, 0x1000, 0);

        // Empty chain should have no descriptors
        assert_eq!(chain.descs.len(), 0);
    }

    #[test]
    fn test_queue_batch_limit() {
        let mut mmu = QueueMmu::new(16);
        let mut queue = Queue::new(16);
        queue.desc_addr = 0x1000;
        queue.avail_addr = 0x2000;
        queue.used_addr = 0x3000;

        // Setup more descriptors than we'll request
        for i in 0..10 {
            mmu.set_descriptor(i, 0x4000 + i as u64 * 512, 512, 0, None);
        }
        mmu.set_avail_idx(10);
        for i in 0..10 {
            mmu.set_avail_desc(i, i);
        }

        // Request only 5
        let batch = queue.pop_batch(&mmu, 5);
        assert_eq!(batch.len(), 5);
    }
}
