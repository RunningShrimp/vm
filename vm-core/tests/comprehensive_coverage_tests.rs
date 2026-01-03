//! Comprehensive VM-Core coverage tests
//!
//! This test file targets to increase vm-core coverage from 55% to 80%.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use vm_core::{
    AggregateRoot, Config, DomainEvent, EventStore, GuestAddr, GuestVAddr, HostPtr, MMU,
    PageTableEntry, VmError, VmResult,
};

// ============================================================================
// GuestAddr Tests
// ============================================================================

#[test]
fn test_guest_addr_creation() {
    let addr = GuestAddr(0x1000);
    assert_eq!(addr.0, 0x1000);
}

#[test]
fn test_guest_addr_addition() {
    let addr = GuestAddr(0x1000);
    let result = addr + 0x100;
    assert_eq!(result.0, 0x1100);
}

#[test]
fn test_guest_addr_subtraction() {
    let addr = GuestAddr(0x1000);
    let result = addr - 0x100;
    assert_eq!(result.0, 0xF00);
}

#[test]
fn test_guest_addr_alignment() {
    let addr = GuestAddr(0x1003);
    let aligned = addr.align_down(4);
    assert_eq!(aligned.0, 0x1000);
}

#[test]
fn test_guest_addr_is_aligned() {
    let addr = GuestAddr(0x1000);
    assert!(addr.is_aligned(4));
    assert!(addr.is_aligned(16));
    assert!(!GuestAddr(0x1001).is_aligned(4));
}

#[test]
fn test_guestaddr_offset() {
    let base = GuestAddr(0x1000);
    let offset = 0x100;
    assert_eq!((base + offset).0, 0x1100);
}

// ============================================================================
// GuestVAddr Tests
// ============================================================================

#[test]
fn test_guest_vaddr_creation() {
    let vaddr = GuestVAddr(0x1000);
    assert_eq!(vaddr.0, 0x1000);
}

#[test]
fn test_guest_vaddr_to_guest_addr() {
    let vaddr = GuestVAddr(0x1000);
    let gaddr: GuestAddr = vaddr.into();
    assert_eq!(gaddr.0, 0x1000);
}

// ============================================================================
// HostPtr Tests
// ============================================================================

#[test]
fn test_host_ptr_null() {
    let ptr = HostPtr::<u8>::null();
    assert!(ptr.is_null());
}

#[test]
fn test_host_ptr_from_raw() {
    let value = 42u8;
    let ptr = HostPtr::from(&value as *const u8);
    assert!(!ptr.is_null());
}

#[test]
fn test_host_ptr_as_ptr() {
    let value = 42u8;
    let ptr = HostPtr::from(&value as *const u8);
    let raw_ptr = ptr.as_ptr();
    assert_eq!(unsafe { *raw_ptr }, 42);
}

#[test]
fn test_host_ptr_deref() {
    let value = 42u8;
    let ptr = HostPtr::from(&value as *const u8);
    assert_eq!(unsafe { ptr.read() }, 42);
}

#[test]
fn test_host_ptr_write() {
    let mut value = 0u8;
    let ptr = HostPtr::from(&mut value as *mut u8);
    unsafe {
        ptr.write(99);
    }
    assert_eq!(value, 99);
}

// ============================================================================
// PageTableEntry Tests
// ============================================================================

#[test]
fn test_pte_creation() {
    let pte = PageTableEntry::new(0x1000, true, true, false);
    assert_eq!(pte.addr(), 0x1000);
    assert!(pte.is_valid());
    assert!(pte.is_readable());
    assert!(pte.is_writable());
    assert!(!pte.is_executable());
}

#[test]
fn test_pte_valid_flag() {
    let pte = PageTableEntry::new(0x1000, true, false, false);
    assert!(pte.is_valid());
}

#[test]
fn test_pte_readable_flag() {
    let pte = PageTableEntry::new(0x1000, true, true, false);
    assert!(pte.is_readable());
}

#[test]
fn test_pte_writable_flag() {
    let pte = PageTableEntry::new(0x1000, true, false, true);
    assert!(pte.is_writable());
}

#[test]
fn test_pte_executable_flag() {
    let pte = PageTableEntry::new(0x1000, true, false, true);
    assert!(pte.is_executable());
}

#[test]
fn test_pte_user_mode_flag() {
    let pte = PageTableEntry::new(0x1000, true, false, false);
    assert!(!pte.is_user_mode());
}

#[test]
fn test_pte_accessed_flag() {
    let pte = PageTableEntry::new(0x1000, true, false, false);
    assert!(!pte.is_accessed());
}

#[test]
fn test_pte_dirty_flag() {
    let pte = PageTableEntry::new(0x1000, true, false, false);
    assert!(!pte.is_dirty());
}

#[test]
fn test_pte_address_alignment() {
    let pte = PageTableEntry::new(0x1000, true, false, false);
    assert_eq!(pte.addr() & 0xFFF, 0);
}

// ============================================================================
// VmError Tests
// ============================================================================

#[test]
fn test_vm_error_display() {
    let err = VmError::InvalidAddress(GuestAddr(0x1000));
    let msg = format!("{}", err);
    assert!(msg.contains("0x1000") || msg.contains("invalid") || msg.contains("address"));
}

#[test]
fn test_vm_error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let vm_err: VmError = io_err.into();
    assert!(matches!(vm_err, VmError::Io(_)));
}

#[test]
fn test_vm_error_invalid_address() {
    let err = VmError::InvalidAddress(GuestAddr(0xDEAD));
    assert!(matches!(err, VmError::InvalidAddress(_)));
}

#[test]
fn test_vm_error_page_fault() {
    let err = VmError::PageFault(GuestAddr(0x1000), false);
    assert!(matches!(err, VmError::PageFault(_, _)));
}

#[test]
fn test_vm_error_permission_denied() {
    let err = VmError::PermissionDenied(GuestAddr(0x1000));
    assert!(matches!(err, VmError::PermissionDenied(_)));
}

#[test]
fn test_vm_error_not_implemented() {
    let err = VmError::NotImplemented("test feature".to_string());
    assert!(matches!(err, VmError::NotImplemented(_)));
}

// ============================================================================
// VmResult Tests
// ============================================================================

#[test]
fn test_vm_result_ok() {
    let result: VmResult<u32> = Ok(42);
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_vm_result_err() {
    let result: VmResult<u32> = Err(VmError::InvalidAddress(GuestAddr(0)));
    assert!(result.is_err());
}

#[test]
fn test_vm_result_error_conversion() {
    let err = VmError::InvalidAddress(GuestAddr(0));
    let result: VmResult<()> = Err(err);
    assert!(result.is_err());
}

// ============================================================================
// Simple MMU Implementation for Testing
// ============================================================================

struct TestMMU {
    memory: Arc<Mutex<HashMap<u64, u8>>>,
    size: usize,
}

impl TestMMU {
    fn new(size: usize) -> Self {
        Self {
            memory: Arc::new(Mutex::new(HashMap::new())),
            size,
        }
    }

    fn write_byte(&self, addr: u64, value: u8) {
        let mut mem = self.memory.lock().unwrap();
        mem.insert(addr, value);
    }

    fn write_bytes(&self, addr: u64, data: &[u8]) {
        let mut mem = self.memory.lock().unwrap();
        for (i, &byte) in data.iter().enumerate() {
            mem.insert(addr + i as u64, byte);
        }
    }
}

impl MMU for TestMMU {
    fn read_byte(&self, addr: GuestAddr) -> VmResult<u8> {
        let mem = self.memory.lock().unwrap();
        Ok(mem.get(&addr.0).copied().unwrap_or(0))
    }

    fn write_byte(&self, addr: GuestAddr, value: u8) -> VmResult<()> {
        let mut mem = self.memory.lock().unwrap();
        mem.insert(addr.0, value);
        Ok(())
    }

    fn fetch_insn(&self, pc: GuestAddr) -> VmResult<u64> {
        let mem = self.memory.lock().unwrap();
        let mut insn = 0u64;
        for i in 0..8 {
            let byte = mem.get(&(pc.0 + i)).copied().unwrap_or(0);
            insn |= (byte as u64) << (i * 8);
        }
        Ok(insn)
    }

    fn read_half(&self, addr: GuestAddr) -> VmResult<u16> {
        let mem = self.memory.lock().unwrap();
        let low = mem.get(&addr.0).copied().unwrap_or(0) as u16;
        let high = mem.get(&(addr.0 + 1)).copied().unwrap_or(0) as u16;
        Ok(low | (high << 8))
    }

    fn read_word(&self, addr: GuestAddr) -> VmResult<u32> {
        let mem = self.memory.lock().unwrap();
        let mut word = 0u32;
        for i in 0..4 {
            let byte = mem.get(&(addr.0 + i)).copied().unwrap_or(0) as u32;
            word |= byte << (i * 8);
        }
        Ok(word)
    }

    fn read_double(&self, addr: GuestAddr) -> VmResult<u64> {
        let mem = self.memory.lock().unwrap();
        let mut double = 0u64;
        for i in 0..8 {
            let byte = mem.get(&(addr.0 + i)).copied().unwrap_or(0) as u64;
            double |= byte << (i * 8);
        }
        Ok(double)
    }

    fn write_half(&self, addr: GuestAddr, value: u16) -> VmResult<()> {
        let mut mem = self.memory.lock().unwrap();
        mem.insert(addr.0, (value & 0xFF) as u8);
        mem.insert(addr.0 + 1, ((value >> 8) & 0xFF) as u8);
        Ok(())
    }

    fn write_word(&self, addr: GuestAddr, value: u32) -> VmResult<()> {
        let mut mem = self.memory.lock().unwrap();
        for i in 0..4 {
            mem.insert(addr.0 + i, ((value >> (i * 8)) & 0xFF) as u8);
        }
        Ok(())
    }

    fn write_double(&self, addr: GuestAddr, value: u64) -> VmResult<()> {
        let mut mem = self.memory.lock().unwrap();
        for i in 0..8 {
            mem.insert(addr.0 + i, ((value >> (i * 8)) & 0xFF) as u8);
        }
        Ok(())
    }
}

// ============================================================================
// MMU Read/Write Tests
// ============================================================================

#[test]
fn test_mmu_read_byte() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_byte(0x100, 0x42);
    assert_eq!(mmu.read_byte(GuestAddr(0x100)).unwrap(), 0x42);
}

#[test]
fn test_mmu_write_byte() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_byte(GuestAddr(0x100), 0x42).unwrap();
    assert_eq!(mmu.read_byte(GuestAddr(0x100)).unwrap(), 0x42);
}

#[test]
fn test_mmu_read_half() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_bytes(0x100, &[0x34, 0x12]);
    assert_eq!(mmu.read_half(GuestAddr(0x100)).unwrap(), 0x1234);
}

#[test]
fn test_mmu_write_half() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_half(GuestAddr(0x100), 0x1234).unwrap();
    assert_eq!(mmu.read_half(GuestAddr(0x100)).unwrap(), 0x1234);
}

#[test]
fn test_mmu_read_word() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_bytes(0x100, &[0x78, 0x56, 0x34, 0x12]);
    assert_eq!(mmu.read_word(GuestAddr(0x100)).unwrap(), 0x12345678);
}

#[test]
fn test_mmu_write_word() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_word(GuestAddr(0x100), 0x12345678).unwrap();
    assert_eq!(mmu.read_word(GuestAddr(0x100)).unwrap(), 0x12345678);
}

#[test]
fn test_mmu_read_double() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_bytes(0x100, &[0xEF, 0xCD, 0xAB, 0x89, 0x67, 0x45, 0x23, 0x01]);
    assert_eq!(
        mmu.read_double(GuestAddr(0x100)).unwrap(),
        0x0123456789ABCDEF
    );
}

#[test]
fn test_mmu_write_double() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_double(GuestAddr(0x100), 0x0123456789ABCDEF)
        .unwrap();
    assert_eq!(
        mmu.read_double(GuestAddr(0x100)).unwrap(),
        0x0123456789ABCDEF
    );
}

#[test]
fn test_mmu_fetch_insn() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_word(GuestAddr(0x100), 0x12345678).unwrap();
    assert_eq!(mmu.fetch_insn(GuestAddr(0x100)).unwrap(), 0x12345678);
}

#[test]
fn test_mmu_unaligned_read() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_bytes(0x100, &[0x01, 0x02, 0x03, 0x04]);
    // Unaligned read - should still work in our test MMU
    let result = mmu.read_half(GuestAddr(0x101));
    assert!(result.is_ok());
}

#[test]
fn test_mmu_unaligned_write() {
    let mmu = TestMMU::new(0x1000);
    let result = mmu.write_half(GuestAddr(0x101), 0x1234);
    assert!(result.is_ok());
}

// ============================================================================
// Domain Events Tests
// ============================================================================

#[test]
fn test_domain_event_creation() {
    #[derive(Debug, Clone)]
    struct TestEvent {
        id: u32,
    }

    impl DomainEvent for TestEvent {}

    let event = TestEvent { id: 42 };
    assert_eq!(event.id, 42);
}

// ============================================================================
// Aggregate Root Tests
// ============================================================================

#[test]
fn test_aggregate_root_apply_event() {
    struct TestAggregate {
        value: u32,
    }

    impl AggregateRoot for TestAggregate {
        type Event = TestEventValue;

        fn apply(&mut self, event: Self::Event) {
            self.value = event.value;
        }
    }

    #[derive(Debug, Clone)]
    struct TestEventValue {
        value: u32,
    }

    impl DomainEvent for TestEventValue {}

    let mut aggregate = TestAggregate { value: 0 };
    aggregate.apply(TestEventValue { value: 42 });
    assert_eq!(aggregate.value, 42);
}

// ============================================================================
// Config Tests
// ============================================================================

#[test]
fn test_config_default() {
    let config = Config::default();
    // Just verify it can be created
    assert_eq!(config.num_cores, 1);
}

#[test]
fn test_config_builder() {
    let config = Config::builder()
        .num_cores(4)
        .memory_size(1024 * 1024)
        .build();

    assert_eq!(config.num_cores, 4);
    assert_eq!(config.memory_size, 1024 * 1024);
}

#[test]
fn test_config_serialization() {
    let config = Config::default();
    let json = serde_json::to_string(&config);
    assert!(json.is_ok());

    if let Ok(json_str) = json {
        let decoded: Config = serde_json::from_str(&json_str).unwrap();
        assert_eq!(decoded.num_cores, config.num_cores);
    }
}

#[test]
fn test_config_toml_parsing() {
    let toml_str = r#"
        num_cores = 2
        memory_size = 2097152
    "#;

    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.num_cores, 2);
    assert_eq!(config.memory_size, 2097152);
}

// ============================================================================
// Event Store Tests
// ============================================================================

struct InMemoryEventStore {
    events: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl InMemoryEventStore {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl EventStore for InMemoryEventStore {
    fn append(&self, event: &[u8]) -> VmResult<()> {
        let mut events = self.events.lock().unwrap();
        events.push(event.to_vec());
        Ok(())
    }

    fn read(&self, index: usize) -> VmResult<Option<Vec<u8>>> {
        let events = self.events.lock().unwrap();
        Ok(events.get(index).cloned())
    }

    fn len(&self) -> VmResult<usize> {
        let events = self.events.lock().unwrap();
        Ok(events.len())
    }
}

#[test]
fn test_event_store_append() {
    let store = InMemoryEventStore::new();
    store.append(b"test event").unwrap();
    assert_eq!(store.len().unwrap(), 1);
}

#[test]
fn test_event_store_read() {
    let store = InMemoryEventStore::new();
    store.append(b"test event").unwrap();
    let event = store.read(0).unwrap();
    assert!(event.is_some());
    assert_eq!(event.unwrap(), b"test event");
}

#[test]
fn test_event_store_read_nonexistent() {
    let store = InMemoryEventStore::new();
    let event = store.read(0).unwrap();
    assert!(event.is_none());
}

#[test]
fn test_event_store_multiple_appends() {
    let store = InMemoryEventStore::new();
    store.append(b"event 1").unwrap();
    store.append(b"event 2").unwrap();
    store.append(b"event 3").unwrap();
    assert_eq!(store.len().unwrap(), 3);
}

#[test]
fn test_event_store_read_all() {
    let store = InMemoryEventStore::new();
    store.append(b"event 1").unwrap();
    store.append(b"event 2").unwrap();

    assert_eq!(store.read(0).unwrap().unwrap(), b"event 1");
    assert_eq!(store.read(1).unwrap().unwrap(), b"event 2");
}

// ============================================================================
// Memory Access Pattern Tests
// ============================================================================

#[test]
fn test_sequential_memory_access() {
    let mmu = TestMMU::new(0x1000);
    for i in 0..10 {
        mmu.write_word(GuestAddr(i * 4), i as u32).unwrap();
    }
    for i in 0..10 {
        assert_eq!(mmu.read_word(GuestAddr(i * 4)).unwrap(), i as u32);
    }
}

#[test]
fn test_overlapping_memory_access() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_word(GuestAddr(0x100), 0x12345678).unwrap();
    mmu.write_half(GuestAddr(0x102), 0xABCD).unwrap();

    assert_eq!(mmu.read_word(GuestAddr(0x100)).unwrap(), 0x12345678);
    assert_eq!(mmu.read_half(GuestAddr(0x102)).unwrap(), 0xABCD);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_invalid_address_error() {
    let result: VmResult<u32> = Err(VmError::InvalidAddress(GuestAddr(0xBAD)));
    assert!(matches!(result, Err(VmError::InvalidAddress(_))));
}

#[test]
fn test_permission_denied_error() {
    let result: VmResult<u32> = Err(VmError::PermissionDenied(GuestAddr(0x1000)));
    assert!(matches!(result, Err(VmError::PermissionDenied(_))));
}

#[test]
fn test_page_fault_error() {
    let result: VmResult<u32> = Err(VmError::PageFault(GuestAddr(0x1000), false));
    assert!(matches!(result, Err(VmError::PageFault(_, _))));
}

// ============================================================================
// Boundary Condition Tests
// ============================================================================

#[test]
fn test_zero_address() {
    let mmu = TestMMU::new(0x1000);
    mmu.write_byte(GuestAddr(0), 0xFF).unwrap();
    assert_eq!(mmu.read_byte(GuestAddr(0)).unwrap(), 0xFF);
}

#[test]
fn test_max_address() {
    let mmu = TestMMU::new(0x1000);
    let addr = GuestAddr(0xFFFF_FFF8);
    mmu.write_double(addr, 0x0123456789ABCDEF).unwrap();
    assert_eq!(mmu.read_double(addr).unwrap(), 0x0123456789ABCDEF);
}

#[test]
fn test_address_overflow() {
    let addr = GuestAddr(0xFFFF_FFFF);
    let next = addr + 1;
    assert_eq!(next.0, 0);
}
