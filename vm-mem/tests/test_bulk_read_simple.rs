use vm_core::mmu_traits::MemoryAccess;
use vm_mem::PhysicalMemory;

#[test]
fn test_bulk_read_basic() {
    let mem = PhysicalMemory::new(1024 * 1024 * 1024, false);
    let addr = vm_core::GuestAddr(0x1000);
    let mut buffer = vec![0u8; 256];

    for i in 0..10000 {
        buffer.clear();
        buffer.resize(256, 0);
        mem.read_bulk(addr, &mut buffer).unwrap();
    }
}
