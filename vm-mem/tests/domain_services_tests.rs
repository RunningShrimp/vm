//! 地址转换领域服务测试

mod address_translation_tests {
    use std::sync::{Arc, Mutex};
    use vm_core::{AccessType, Fault, VmError};
    use vm_mem::domain_services::AddressTranslationDomainService;
    use vm_mem::GuestAddr;
    use vm_mem::mmu::{MmuArch, PageWalkResult};

    fn create_test_memory() -> Arc<Mutex<Vec<u8>>> {
        Arc::new(Mutex::new(vec![0u8; 1024 * 1024]))
    }

    #[test]
    fn test_service_creation() {
        let memory = create_test_memory();
        let memory_clone = Arc::clone(&memory);
        let service = AddressTranslationDomainService::new(
            MmuArch::X86_64,
            move |addr: GuestAddr, size: usize| -> Result<Vec<u8>, VmError> {
                let mem = memory_clone.lock().unwrap();
                let start = addr.0 as usize;
                let end = (start + size).min(mem.len());
                Ok(mem[start..end].to_vec())
            }
        );
        assert_eq!(service.arch, MmuArch::X86_64);
    }

    #[test]
    fn test_service_creation_all_architectures() {
        let memory = create_test_memory();
        let memory_clone = Arc::clone(&memory);
        let read_fn = move |addr: GuestAddr, size: usize| -> Result<Vec<u8>, VmError> {
            let mem = memory_clone.lock().unwrap();
            let start = addr.0 as usize;
            let end = (start + size).min(mem.len());
            Ok(mem[start..end].to_vec())
        };

        let _ = AddressTranslationDomainService::new(MmuArch::X86_64, &read_fn);
        let _ = AddressTranslationDomainService::new(MmuArch::AArch64, &read_fn);
        let _ = AddressTranslationDomainService::new(MmuArch::RiscVSv39, &read_fn);
        let _ = AddressTranslationDomainService::new(MmuArch::RiscVSv48, &read_fn);
    }

    #[test]
    fn test_translate_x86_64_success() {
        let mut memory = vec![0u8; 1024 * 1024];
        
        let pml4e: u64 = 0x3; 
        let pdpte: u64 = 0x1000 + 0x3; 
        let pde: u64 = 0x2000 + 0x3; 
        let pte: u64 = 0x3000 + 0x3; 

        memory[0..8].copy_from_slice(&pml4e.to_le_bytes());
        memory[0x1000..0x1008].copy_from_slice(&pdpte.to_le_bytes());
        memory[0x2000..0x2008].copy_from_slice(&pde.to_le_bytes());
        memory[0x3000..0x3008].copy_from_slice(&pte.to_le_bytes());

        let memory_arc = Arc::new(Mutex::new(memory));
        let memory_clone = Arc::clone(&memory_arc);
        let service = AddressTranslationDomainService::new(
            MmuArch::X86_64,
            move |addr: GuestAddr, size: usize| -> Result<Vec<u8>, VmError> {
                let mem = memory_clone.lock().unwrap();
                let start = addr.0 as usize;
                let end = (start + size).min(mem.len());
                Ok(mem[start..end].to_vec())
            }
        );

        let result = service.translate(GuestAddr(0x1000), GuestAddr(0));
        assert!(result.is_ok(), "Translation should succeed");
        let page_walk = result.unwrap();
        assert_eq!(page_walk.gpa, GuestAddr(0x1000));
    }

    #[test]
    fn test_translate_x86_64_page_fault() {
        let memory = Arc::new(Mutex::new(vec![0u8; 1024 * 1024]));
        let memory_clone = Arc::clone(&memory);
        let service = AddressTranslationDomainService::new(
            MmuArch::X86_64,
            move |addr: GuestAddr, size: usize| -> Result<Vec<u8>, VmError> {
                let mem = memory_clone.lock().unwrap();
                let start = addr.0 as usize;
                let end = (start + size).min(mem.len());
                Ok(mem[start..end].to_vec())
            }
        );

        let result = service.translate(GuestAddr(0x1000), GuestAddr(0));
        assert!(result.is_err(), "Translation should fail with page fault");
        
        match result {
            Err(VmError::Execution(vm_core::ExecutionError::Fault(Fault::PageFault { .. }))) => {}
            _ => panic!("Expected PageFault error"),
        }
    }

    #[test]
    fn test_translate_aarch64_success() {
        let mut memory = vec![0u8; 1024 * 1024];
        
        let l0e: u64 = 0x3; 
        let l1e: u64 = 0x1000 + 0x3; 
        let l2e: u64 = 0x2000 + 0x3; 
        let l3e: u64 = 0x3000 + 0x3; 

        memory[0..8].copy_from_slice(&l0e.to_le_bytes());
        memory[0x1000..0x1008].copy_from_slice(&l1e.to_le_bytes());
        memory[0x2000..0x2008].copy_from_slice(&l2e.to_le_bytes());
        memory[0x3000..0x3008].copy_from_slice(&l3e.to_le_bytes());

        let memory_arc = Arc::new(Mutex::new(memory));
        let memory_clone = Arc::clone(&memory_arc);
        let service = AddressTranslationDomainService::new(
            MmuArch::AArch64,
            move |addr: GuestAddr, size: usize| -> Result<Vec<u8>, VmError> {
                let mem = memory_clone.lock().unwrap();
                let start = addr.0 as usize;
                let end = (start + size).min(mem.len());
                Ok(mem[start..end].to_vec())
            }
        );

        let result = service.translate(GuestAddr(0x1000), GuestAddr(0));
        assert!(result.is_ok(), "Translation should succeed");
        let page_walk = result.unwrap();
        assert_eq!(page_walk.gpa, GuestAddr(0x1000));
    }

    #[test]
    fn test_translate_aarch64_page_fault() {
        let memory = Arc::new(Mutex::new(vec![0u8; 1024 * 1024]));
        let memory_clone = Arc::clone(&memory);
        let service = AddressTranslationDomainService::new(
            MmuArch::AArch64,
            move |addr: GuestAddr, size: usize| -> Result<Vec<u8>, VmError> {
                let mem = memory_clone.lock().unwrap();
                let start = addr.0 as usize;
                let end = (start + size).min(mem.len());
                Ok(mem[start..end].to_vec())
            }
        );

        let result = service.translate(GuestAddr(0x1000), GuestAddr(0));
        assert!(result.is_err(), "Translation should fail with page fault");
        
        match result {
            Err(VmError::Execution(vm_core::ExecutionError::Fault(Fault::PageFault { .. }))) => {}
            _ => panic!("Expected PageFault error"),
        }
    }

    #[test]
    fn test_translate_riscv_sv39_success() {
        let mut memory = vec![0u8; 1024 * 1024];
        
        let l2e: u64 = 0x3; 
        let l1e: u64 = 0x1000 + 0x3; 
        let l0e: u64 = 0x2000 + 0xF; 

        memory[0..8].copy_from_slice(&l2e.to_le_bytes());
        memory[0x1000..0x1008].copy_from_slice(&l1e.to_le_bytes());
        memory[0x2000..0x2008].copy_from_slice(&l0e.to_le_bytes());

        let memory_arc = Arc::new(Mutex::new(memory));
        let memory_clone = Arc::clone(&memory_arc);
        let service = AddressTranslationDomainService::new(
            MmuArch::RiscVSv39,
            move |addr: GuestAddr, size: usize| -> Result<Vec<u8>, VmError> {
                let mem = memory_clone.lock().unwrap();
                let start = addr.0 as usize;
                let end = (start + size).min(mem.len());
                Ok(mem[start..end].to_vec())
            }
        );

        let result = service.translate(GuestAddr(0x1000), GuestAddr(0));
        assert!(result.is_ok(), "Translation should succeed");
        let page_walk = result.unwrap();
        assert_eq!(page_walk.gpa, GuestAddr(0x1000));
    }

    #[test]
    fn test_translate_riscv_sv48_success() {
        let mut memory = vec![0u8; 1024 * 1024];
        
        let l3e: u64 = 0x3; 
        let l2e: u64 = 0x1000 + 0x3; 
        let l1e: u64 = 0x2000 + 0x3; 
        let l0e: u64 = 0x3000 + 0xF; 

        memory[0..8].copy_from_slice(&l3e.to_le_bytes());
        memory[0x1000..0x1008].copy_from_slice(&l2e.to_le_bytes());
        memory[0x2000..0x2008].copy_from_slice(&l1e.to_le_bytes());
        memory[0x3000..0x3008].copy_from_slice(&l0e.to_le_bytes());

        let memory_arc = Arc::new(Mutex::new(memory));
        let memory_clone = Arc::clone(&memory_arc);
        let service = AddressTranslationDomainService::new(
            MmuArch::RiscVSv48,
            move |addr: GuestAddr, size: usize| -> Result<Vec<u8>, VmError> {
                let mem = memory_clone.lock().unwrap();
                let start = addr.0 as usize;
                let end = (start + size).min(mem.len());
                Ok(mem[start..end].to_vec())
            }
        );

        let result = service.translate(GuestAddr(0x1000), GuestAddr(0));
        assert!(result.is_ok(), "Translation should succeed");
        let page_walk = result.unwrap();
        assert_eq!(page_walk.gpa, GuestAddr(0x1000));
    }

    #[test]
    fn test_page_walk_result() {
        let result = PageWalkResult {
            gpa: GuestAddr(0x1000),
            page_size: 4096,
            flags: vm_mem::mmu::PageTableFlags::default(),
        };

        assert_eq!(result.gpa, GuestAddr(0x1000));
        assert_eq!(result.page_size, 4096);
    }

    #[test]
    fn test_memory_read_error() {
        let service = AddressTranslationDomainService::new(
            MmuArch::X86_64,
            |_addr: GuestAddr, _size: usize| -> Result<Vec<u8>, VmError> {
                Err(VmError::Memory(vm_core::MemoryError::AccessViolation {
                    addr: GuestAddr(0),
                    msg: "Access denied".to_string(),
                    access_type: Some(AccessType::Read),
                }))
            }
        );

        let result = service.translate(GuestAddr(0x1000), GuestAddr(0));
        assert!(result.is_err());
    }
}
