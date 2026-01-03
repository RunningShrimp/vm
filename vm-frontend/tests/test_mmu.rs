//! Test MMU Helper Module
//!
//! Provides a simple MMU implementation for testing decoders

use std::collections::HashMap;

use vm_core::{
    AccessType, AddressTranslator, GuestAddr, GuestPhysAddr, MemoryAccess, MmioDevice, MmioManager,
    MmuAsAny, VmError,
};

/// Simple MMU implementation for testing
pub struct TestMMU {
    pub memory: HashMap<u64, u8>,
}

impl TestMMU {
    pub fn new() -> Self {
        Self {
            memory: HashMap::new(),
        }
    }

    pub fn set_insn(&mut self, addr: u64, insn: u32) {
        let bytes = insn.to_le_bytes();
        for (i, &byte) in bytes.iter().enumerate() {
            self.memory.insert(addr + i as u64, byte);
        }
    }

    pub fn set_insn16(&mut self, addr: u64, insn: u16) {
        let bytes = insn.to_le_bytes();
        for (i, &byte) in bytes.iter().enumerate() {
            self.memory.insert(addr + i as u64, byte);
        }
    }

    pub fn get_insn(&self, addr: u64) -> u32 {
        let b0 = self.memory.get(&addr).copied().unwrap_or(0);
        let b1 = self.memory.get(&(addr + 1)).copied().unwrap_or(0);
        let b2 = self.memory.get(&(addr + 2)).copied().unwrap_or(0);
        let b3 = self.memory.get(&(addr + 3)).copied().unwrap_or(0);
        u32::from_le_bytes([b0, b1, b2, b3])
    }
}

impl MemoryAccess for TestMMU {
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        match size {
            1 => Ok(self.memory.get(&pa.0).copied().unwrap_or(0) as u64),
            2 => {
                let b0 = self.memory.get(&pa.0).copied().unwrap_or(0) as u64;
                let b1 = self.memory.get(&(pa.0 + 1)).copied().unwrap_or(0) as u64;
                Ok(b0 | (b1 << 8))
            }
            4 => {
                let b0 = self.memory.get(&pa.0).copied().unwrap_or(0) as u64;
                let b1 = self.memory.get(&(pa.0 + 1)).copied().unwrap_or(0) as u64;
                let b2 = self.memory.get(&(pa.0 + 2)).copied().unwrap_or(0) as u64;
                let b3 = self.memory.get(&(pa.0 + 3)).copied().unwrap_or(0) as u64;
                Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
            }
            8 => {
                let mut result = 0u64;
                for i in 0..8 {
                    result |=
                        (self.memory.get(&(pa.0 + i)).copied().unwrap_or(0) as u64) << (i * 8);
                }
                Ok(result)
            }
            _ => Err(VmError::Memory(vm_core::MemoryError::MmuLockFailed {
                message: "Invalid read size".to_string(),
            })),
        }
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        match size {
            1 => {
                self.memory.insert(pa.0, val as u8);
                Ok(())
            }
            2 => {
                self.memory.insert(pa.0, (val & 0xFF) as u8);
                self.memory.insert(pa.0 + 1, ((val >> 8) & 0xFF) as u8);
                Ok(())
            }
            4 => {
                for i in 0..4 {
                    self.memory
                        .insert(pa.0 + i, ((val >> (i * 8)) & 0xFF) as u8);
                }
                Ok(())
            }
            8 => {
                for i in 0..8 {
                    self.memory
                        .insert(pa.0 + i, ((val >> (i * 8)) & 0xFF) as u8);
                }
                Ok(())
            }
            _ => Err(VmError::Memory(vm_core::MemoryError::MmuLockFailed {
                message: "Invalid write size".to_string(),
            })),
        }
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        self.read(pc, 4)
    }

    fn memory_size(&self) -> usize {
        self.memory.len()
    }

    fn dump_memory(&self) -> Vec<u8> {
        let max_addr = self.memory.keys().copied().max().unwrap_or(0) as usize;
        let mut result = vec![0u8; max_addr + 1];
        for (addr, &val) in &self.memory {
            if *addr as usize <= max_addr {
                result[*addr as usize] = val;
            }
        }
        result
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
        for (i, &val) in data.iter().enumerate() {
            self.memory.insert(i as u64, val);
        }
        Ok(())
    }
}

impl AddressTranslator for TestMMU {
    fn translate(&mut self, va: GuestAddr, _access: AccessType) -> Result<GuestPhysAddr, VmError> {
        Ok(GuestPhysAddr(va.0))
    }

    fn flush_tlb(&mut self) {
        // No TLB in simple test MMU
    }
}

impl MmioManager for TestMMU {
    fn map_mmio(&self, _base: GuestAddr, _size: u64, _device: Box<dyn MmioDevice>) {
        // No MMIO mapping in test MMU
    }
}

impl MmuAsAny for TestMMU {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// Blanket impl will provide MMU automatically
// No need to manually implement it
