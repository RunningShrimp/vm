#![no_main]
//! Memory Access Fuzzing Target
//!
//! This fuzzing target tests the robustness of the memory access system.
//! It feeds random addresses and data to memory operations and verifies that:
//! 1. Memory operations never crash
//! 2. Out-of-bounds access is properly rejected
//! 3. Memory state remains consistent
//! 4. No memory corruption occurs

use libfuzzer_sys::fuzz_target;
use std::sync::Arc;
use std::collections::HashMap;

/// Simple memory pool implementation for fuzzing
///
/// This is a simplified memory pool that's designed to be robust against
/// arbitrary inputs while maintaining basic memory semantics.
struct FuzzMemoryPool {
    size: usize,
    data: Vec<u8>,
    allocated: bool,
}

impl FuzzMemoryPool {
    fn new(size: usize) -> Self {
        Self {
            size,
            data: vec![0u8; size],
            allocated: true,
        }
    }

    /// Read from memory
    fn read(&self, addr: u64, buf: &mut [u8]) -> Result<(), MemoryError> {
        let addr = addr as usize;

        // Check bounds
        if addr.checked_add(buf.len()).is_none() {
            return Err(MemoryError::AddressOverflow);
        }

        let end = addr.checked_add(buf.len()).ok_or(MemoryError::AddressOverflow)?;

        if end > self.size {
            return Err(MemoryError::OutOfBounds {
                addr: addr as u64,
                size: buf.len() as u64,
                max: self.size as u64,
            });
        }

        // Copy data
        buf.copy_from_slice(&self.data[addr..end]);
        Ok(())
    }

    /// Write to memory
    fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), MemoryError> {
        let addr = addr as usize;

        // Check bounds
        if addr.checked_add(data.len()).is_none() {
            return Err(MemoryError::AddressOverflow);
        }

        let end = addr.checked_add(data.len()).ok_or(MemoryError::AddressOverflow)?;

        if end > self.size {
            return Err(MemoryError::OutOfBounds {
                addr: addr as u64,
                size: data.len() as u64,
                max: self.size as u64,
            });
        }

        // Copy data
        self.data[addr..end].copy_from_slice(data);
        Ok(())
    }

    /// Check if address is aligned
    fn is_aligned(&self, addr: u64, alignment: u64) -> bool {
        if !alignment.is_power_of_two() {
            return false;
        }
        addr % alignment == 0
    }

    /// Get memory size
    fn size(&self) -> u64 {
        self.size as u64
    }
}

/// Memory operation for fuzzing
#[derive(Debug, Clone, Copy)]
enum MemOp {
    Read { addr: u64, size: usize },
    Write { addr: u64, size: usize },
    ReadWrite { addr: u64, size: usize },
}

/// Memory error types
#[derive(Debug, Clone, PartialEq)]
enum MemoryError {
    OutOfBounds { addr: u64, size: u64, max: u64 },
    AddressOverflow,
    UnalignedAccess { addr: u64, alignment: u64 },
    NullPointer,
}

/// Memory access state tracker
///
/// Tracks memory operations and verifies consistency
struct MemoryTracker {
    writes: HashMap<u64, u8>,
    operations: Vec<MemOp>,
}

impl MemoryTracker {
    fn new() -> Self {
        Self {
            writes: HashMap::new(),
            operations: Vec::new(),
        }
    }

    fn record_write(&mut self, addr: u64, data: &[u8]) {
        for (i, &byte) in data.iter().enumerate() {
            self.writes.insert(addr + i as u64, byte);
        }
    }

    fn record_operation(&mut self, op: MemOp) {
        self.operations.push(op);
    }

    fn verify_read_consistency(&self, addr: u64, data: &[u8]) -> bool {
        for (i, &byte) in data.iter().enumerate() {
            let expected_addr = addr + i as u64;
            if let Some(&expected_byte) = self.writes.get(&expected_addr) {
                if byte != expected_byte {
                    // Memory corruption detected
                    return false;
                }
            } else if byte != 0 {
                // Uninitialized memory should be zero
                return false;
            }
        }
        true
    }
}

/// Validate memory operation
fn validate_operation(op: &MemOp, pool_size: u64) -> Result<(), MemoryError> {
    match op {
        MemOp::Read { addr, size } | MemOp::Write { addr, size } | MemOp::ReadWrite { addr, size } => {
            // Check for overflow
            let end = addr.checked_add(*size as u64).ok_or(MemoryError::AddressOverflow)?;

            // Check bounds
            if end > pool_size {
                return Err(MemoryError::OutOfBounds {
                    addr: *addr,
                    size: *size as u64,
                    max: pool_size,
                });
            }

            // Check alignment (optional, for strict alignment requirements)
            // if *size > 1 && *addr % (size as u64) != 0 {
            //     return Err(MemoryError::UnalignedAccess {
            //         addr: *addr,
            //         alignment: *size as u64,
            //     });
            // }

            Ok(())
        }
    }
}

/// Fuzz target function
///
/// This function is called by libfuzzer with random byte sequences.
/// It parses the input as a sequence of memory operations and executes them.
fuzz_target!(|data: &[u8]| {
    // Create memory pool (1MB for fuzzing)
    let mut pool = FuzzMemoryPool::new(1024 * 1024);
    let mut tracker = MemoryTracker::new();

    // Parse input as memory operations
    let operations = parse_operations(data);

    // Execute operations
    for op in operations {
        tracker.record_operation(op);

        match op {
            MemOp::Read { addr, size } => {
                // Validate operation
                let validation = validate_operation(&op, pool.size());

                match validation {
                    Ok(_) => {
                        // Perform read
                        let mut buffer = vec![0u8; size];
                        let result = pool.read(addr, &mut buffer);

                        // Read should succeed if validation passed
                        if result.is_ok() {
                            // Verify consistency
                            if !tracker.verify_read_consistency(addr, &buffer) {
                                // Memory corruption detected
                                eprintln!("Memory corruption detected at addr 0x{:x}", addr);
                                eprintln!("Buffer: {:?}", buffer);
                                panic!("Memory read returned inconsistent data");
                            }
                        }
                    }
                    Err(_) => {
                        // Operation should fail
                        let mut buffer = vec![0u8; size];
                        let result = pool.read(addr, &mut buffer);
                        if result.is_ok() {
                            eprintln!("Read should have failed for addr 0x{:x}, size {}", addr, size);
                            panic!("Invalid read operation succeeded");
                        }
                    }
                }
            }

            MemOp::Write { addr, size } => {
                // Validate operation
                let validation = validate_operation(&op, pool.size());

                // Create data to write
                let write_data = create_pattern_data(addr, size);

                match validation {
                    Ok(_) => {
                        // Perform write
                        let result = pool.write(addr, &write_data);

                        // Write should succeed if validation passed
                        if result.is_ok() {
                            // Track write
                            tracker.record_write(addr, &write_data);
                        } else {
                            eprintln!("Valid write failed: {:?}", result);
                            panic!("Valid write operation failed");
                        }
                    }
                    Err(_) => {
                        // Operation should fail
                        let result = pool.write(addr, &write_data);
                        if result.is_ok() {
                            eprintln!("Write should have failed for addr 0x{:x}, size {}", addr, size);
                            panic!("Invalid write operation succeeded");
                        }
                    }
                }
            }

            MemOp::ReadWrite { addr, size } => {
                // Validate operation
                let validation = validate_operation(&op, pool.size());

                match validation {
                    Ok(_) => {
                        // Perform read-modify-write
                        let mut buffer = vec![0u8; size];
                        if pool.read(addr, &mut buffer).is_ok() {
                            // Modify and write back
                            for byte in buffer.iter_mut() {
                                *byte = byte.wrapping_add(1);
                            }

                            if pool.write(addr, &buffer).is_ok() {
                                tracker.record_write(addr, &buffer);
                            }
                        }
                    }
                    Err(_) => {
                        // Should fail
                        let mut buffer = vec![0u8; size];
                        if pool.read(addr, &mut buffer).is_ok() {
                            panic!("Invalid read-modify-write read succeeded");
                        }
                    }
                }
            }
        }
    }

    // Final consistency check
    // Verify that all tracked writes are still valid
    for (&addr, &expected_byte) in tracker.writes.iter() {
        let mut buffer = [0u8; 1];
        if pool.read(addr, &mut buffer).is_ok() {
            if buffer[0] != expected_byte {
                eprintln!("Final consistency check failed at addr 0x{:x}", addr);
                eprintln!("Expected: 0x{:02x}, Got: 0x{:02x}", expected_byte, buffer[0]);
                panic!("Memory state corrupted after operations");
            }
        }
    }
});

/// Parse byte sequence into memory operations
fn parse_operations(data: &[u8]) -> Vec<MemOp> {
    let mut operations = Vec::new();

    // Each operation is encoded as:
    // [op_type: 1 byte][addr: 8 bytes][size: 2 bytes]
    let chunk_size = 11; // 1 + 8 + 2

    for chunk in data.chunks(chunk_size) {
        if chunk.len() < chunk_size {
            break;
        }

        let op_type = chunk[0];
        let addr = u64::from_le_bytes([
            chunk[1], chunk[2], chunk[3], chunk[4],
            chunk[5], chunk[6], chunk[7], chunk[8],
        ]);
        let size = u16::from_le_bytes([chunk[9], chunk[10]]) as usize;

        // Limit size to prevent excessive allocations
        let size = size.min(4096);

        let op = match op_type % 3 {
            0 => MemOp::Read { addr, size },
            1 => MemOp::Write { addr, size },
            _ => MemOp::ReadWrite { addr, size },
        };

        operations.push(op);
    }

    // Limit total operations to prevent timeouts
    operations.truncate(1000);
    operations
}

/// Create patterned data for writes
fn create_pattern_data(addr: u64, size: usize) -> Vec<u8> {
    (0..size)
        .map(|i| ((addr.wrapping_add(i as u64)) % 256) as u8)
        .collect()
}
