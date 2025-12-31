use vm_mem::PhysicalMemory;
use vm_core::mmu_traits::MemoryAccess;

#[test]
fn test_concurrent_bulk_read() {
    use std::sync::Arc;
    use std::thread;

    let mem = Arc::new(PhysicalMemory::new(1024 * 1024 * 1024, false));
    let addr = vm_core::GuestAddr(0x1000);

    let mut handles = vec![];

    for _ in 0..10 {
        let mem_clone = Arc::clone(&mem);
        let handle = thread::spawn(move || {
            let mut buffer = vec![0u8; 256];
            for _ in 0..10000 {
                mem_clone.read_bulk(addr, &mut buffer).unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Concurrent bulk reads passed!");
}
