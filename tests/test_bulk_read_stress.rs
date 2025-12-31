use vm_mem::PhysicalMemory;
use vm_core::mmu_traits::MemoryAccess;

#[test]
fn test_bulk_read_stress() {
    let mem = PhysicalMemory::new(1024 * 1024 * 1024, false);
    let addr = vm_core::GuestAddr(0x1000);
    
    // 测试不同大小
    for size in [256, 1024, 4096, 16384, 65536] {
        println!("Testing size: {}", size);
        let mut buffer = vec![0u8; size];
        
        for i in 0..10000 {
            mem.read_bulk(addr, &mut buffer).unwrap();
            
            if i % 1000 == 0 {
                println!("  Completed {} iterations", i);
            }
        }
    }
}
