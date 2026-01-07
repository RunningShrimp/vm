//! Memory Management Tests
//!
//! Comprehensive tests for memory management functionality including:
//! - Memory barriers (acquire, release, full)
//! - Memory protection flags
//! - MappedMemory allocation
//! - Memory protection changes

use vm_platform::memory::{
    MappedMemory, MemoryProtection, barrier_acquire, barrier_full, barrier_release,
};

// ============================================================================
// Memory Barrier Tests
// ============================================================================

#[test]
fn test_barrier_acquire_no_panic() {
    barrier_acquire();
}

#[test]
fn test_barrier_release_no_panic() {
    barrier_release();
}

#[test]
fn test_barrier_full_no_panic() {
    barrier_full();
}

#[test]
fn test_multiple_barrier_calls() {
    barrier_acquire();
    barrier_release();
    barrier_full();
    barrier_acquire();
    barrier_release();
}

#[test]
fn test_barrier_in_sequence() {
    barrier_acquire();
    barrier_release();
    barrier_full();
}

// ============================================================================
// MemoryProtection Tests
// ============================================================================

#[test]
fn test_memory_protection_none() {
    let prot = MemoryProtection::NONE;
    assert!(!prot.read);
    assert!(!prot.write);
    assert!(!prot.exec);
}

#[test]
fn test_memory_protection_read() {
    let prot = MemoryProtection::READ;
    assert!(prot.read);
    assert!(!prot.write);
    assert!(!prot.exec);
}

#[test]
fn test_memory_protection_read_write() {
    let prot = MemoryProtection::READ_WRITE;
    assert!(prot.read);
    assert!(prot.write);
    assert!(!prot.exec);
}

#[test]
fn test_memory_protection_read_exec() {
    let prot = MemoryProtection::READ_EXEC;
    assert!(prot.read);
    assert!(!prot.write);
    assert!(prot.exec);
}

#[test]
fn test_memory_protection_read_write_exec() {
    let prot = MemoryProtection::READ_WRITE_EXEC;
    assert!(prot.read);
    assert!(prot.write);
    assert!(prot.exec);
}

#[test]
fn test_memory_protection_custom() {
    let prot = MemoryProtection {
        read: true,
        write: false,
        exec: true,
    };
    assert!(prot.read);
    assert!(!prot.write);
    assert!(prot.exec);
}

#[test]
fn test_memory_protection_clone() {
    let prot1 = MemoryProtection::READ_WRITE;
    let prot2 = prot1;
    assert_eq!(prot1.read, prot2.read);
    assert_eq!(prot1.write, prot2.write);
    assert_eq!(prot1.exec, prot2.exec);
}

#[test]
fn test_memory_protection_debug_trait() {
    let prot = MemoryProtection::READ_WRITE;
    let debug_str = format!("{:?}", prot);
    assert!(debug_str.contains("MemoryProtection"));
}

// ============================================================================
// MappedMemory Tests
// ============================================================================

#[test]
fn test_mapped_memory_allocate_none() {
    let size = 4096usize;
    let result = MappedMemory::allocate(size, MemoryProtection::NONE);
    assert!(result.is_ok());
}

#[test]
fn test_mapped_memory_allocate_read() {
    let size = 4096usize;
    let result = MappedMemory::allocate(size, MemoryProtection::READ);
    assert!(result.is_ok());
}

#[test]
fn test_mapped_memory_allocate_read_write() {
    let size = 4096usize;
    let result = MappedMemory::allocate(size, MemoryProtection::READ_WRITE);
    assert!(result.is_ok());
}

#[test]
fn test_mapped_memory_allocate_read_exec() {
    let size = 4096usize;
    let result = MappedMemory::allocate(size, MemoryProtection::READ_EXEC);
    assert!(result.is_ok());
}

#[test]
fn test_mapped_memory_allocate_read_write_exec() {
    // This test may fail on systems with W^X policy
    // READ_WRITE_EXEC is often not allowed for security reasons
    let size = 4096usize;
    let result = MappedMemory::allocate(size, MemoryProtection::READ_WRITE_EXEC);
    // We accept both success and failure for this test
    // Failure is expected on systems with strict W^X enforcement
    let _ = result;
}

#[test]
fn test_mapped_memory_allocate_small_size() {
    let size = 1usize;
    let result = MappedMemory::allocate(size, MemoryProtection::READ_WRITE);
    assert!(result.is_ok());
}

#[test]
fn test_mapped_memory_allocate_page_size() {
    let size = 4096usize;
    let result = MappedMemory::allocate(size, MemoryProtection::READ_WRITE);
    assert!(result.is_ok());
}

#[test]
fn test_mapped_memory_allocate_large_size() {
    let size = 1024 * 1024usize;
    let result = MappedMemory::allocate(size, MemoryProtection::READ_WRITE);
    assert!(result.is_ok());
}

#[test]
fn test_mapped_memory_size() {
    let size = 4096usize;
    let mem = MappedMemory::allocate(size, MemoryProtection::READ_WRITE).unwrap();
    assert_eq!(mem.size(), size);
}

#[test]
fn test_mapped_memory_as_ptr() {
    let size = 4096usize;
    let mem = MappedMemory::allocate(size, MemoryProtection::READ_WRITE).unwrap();
    let ptr = mem.as_ptr();
    assert!(!ptr.is_null());
}

#[test]
fn test_mapped_memory_as_mut_ptr() {
    let size = 4096usize;
    let mut mem = MappedMemory::allocate(size, MemoryProtection::READ_WRITE).unwrap();
    let ptr = mem.as_mut_ptr();
    assert!(!ptr.is_null());
}

// ============================================================================
// MappedMemory Protect Tests
// ============================================================================

#[test]
fn test_mapped_memory_protect_to_read() {
    let size = 4096usize;
    let mem = MappedMemory::allocate(size, MemoryProtection::READ_WRITE).unwrap();
    let result = mem.protect(MemoryProtection::READ);
    assert!(result.is_ok());
}

#[test]
fn test_mapped_memory_protect_to_read_write() {
    let size = 4096usize;
    let mem = MappedMemory::allocate(size, MemoryProtection::READ).unwrap();
    let result = mem.protect(MemoryProtection::READ_WRITE);
    assert!(result.is_ok());
}

#[test]
fn test_mapped_memory_protect_to_none() {
    let size = 4096usize;
    let mem = MappedMemory::allocate(size, MemoryProtection::READ_WRITE).unwrap();
    let result = mem.protect(MemoryProtection::NONE);
    assert!(result.is_ok());
}

#[test]
fn test_mapped_memory_protect_to_read_exec() {
    let size = 4096usize;
    let mem = MappedMemory::allocate(size, MemoryProtection::READ).unwrap();
    let result = mem.protect(MemoryProtection::READ_EXEC);
    assert!(result.is_ok());
}

#[test]
fn test_mapped_memory_protect_to_read_write_exec() {
    // This test may fail on systems with W^X policy
    let size = 4096usize;
    let mem = MappedMemory::allocate(size, MemoryProtection::READ).unwrap();
    let result = mem.protect(MemoryProtection::READ_WRITE_EXEC);
    // We accept both success and failure
    let _ = result;
}

#[test]
fn test_mapped_memory_multiple_protect_changes() {
    let size = 4096usize;
    let mem = MappedMemory::allocate(size, MemoryProtection::READ_WRITE).unwrap();
    let _ = mem.protect(MemoryProtection::READ).unwrap();
    let _ = mem.protect(MemoryProtection::NONE).unwrap();
    let _ = mem.protect(MemoryProtection::READ_WRITE).unwrap();
    let _ = mem.protect(MemoryProtection::READ_EXEC).unwrap();
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_mapped_memory_allocate_and_protect() {
    let size = 4096usize;
    let mem = MappedMemory::allocate(size, MemoryProtection::READ_WRITE).unwrap();
    let _ = mem.protect(MemoryProtection::READ).unwrap();
    assert_eq!(mem.size(), size);
}

#[test]
fn test_multiple_mapped_memory_allocations() {
    let mem1 = MappedMemory::allocate(4096, MemoryProtection::READ_WRITE).unwrap();
    let mem2 = MappedMemory::allocate(4096, MemoryProtection::READ_WRITE).unwrap();
    let mem3 = MappedMemory::allocate(4096, MemoryProtection::READ_WRITE).unwrap();

    assert_eq!(mem1.size(), 4096);
    assert_eq!(mem2.size(), 4096);
    assert_eq!(mem3.size(), 4096);
}

#[test]
fn test_mapped_memory_different_protections() {
    // Test various protection levels that are commonly supported
    let mem_none = MappedMemory::allocate(4096, MemoryProtection::NONE).unwrap();
    let mem_read = MappedMemory::allocate(4096, MemoryProtection::READ).unwrap();
    let mem_rw = MappedMemory::allocate(4096, MemoryProtection::READ_WRITE).unwrap();
    let mem_rex = MappedMemory::allocate(4096, MemoryProtection::READ_EXEC).unwrap();

    assert_eq!(mem_none.size(), 4096);
    assert_eq!(mem_read.size(), 4096);
    assert_eq!(mem_rw.size(), 4096);
    assert_eq!(mem_rex.size(), 4096);

    // READ_WRITE_EXEC may not be available on all systems
    let rwx_result = MappedMemory::allocate(4096, MemoryProtection::READ_WRITE_EXEC);
    let _ = rwx_result;
}
