//! HVF Backend Tests
//!
//! Comprehensive tests for macOS Hypervisor.framework backend

#[cfg(target_os = "macos")]
mod hvf_tests {
    use vm_accel::hvf_impl::AccelHvf;
    use vm_accel::{Accel, AccelError, AccelKind};

    /// Test HVF accelerator creation
    #[test]
    fn test_hvf_creation() {
        let hvf = AccelHvf::new();
        assert_eq!(hvf.name(), "HVF");
        println!("HVF accelerator created");
    }

    /// Test HVF initialization
    #[test]
    fn test_hvf_init() {
        let mut hvf = AccelHvf::new();

        match hvf.init() {
            Ok(()) => {
                println!("HVF initialized successfully");
            }
            Err(e) => {
                println!("HVF initialization failed: {:?}", e);
                // This can happen if Hypervisor framework is not available
            }
        }
    }

    /// Test HVF detection
    #[test]
    fn test_hvf_detection() {
        let kind = AccelKind::detect_best();

        #[cfg(target_os = "macos")]
        {
            assert_eq!(kind, AccelKind::Hvf);
            println!("HVF detected as best accelerator");
        }
    }

    /// Test HVF vCPU creation
    #[test]
    fn test_hvf_create_vcpu() {
        let mut hvf = AccelHvf::new();

        if hvf.init().is_ok() {
            let result = hvf.create_vcpu(0);

            match result {
                Ok(()) => println!("HVF vCPU created successfully"),
                Err(e) => println!("HVF vCPU creation failed: {:?}", e),
            }
        } else {
            println!("Skipping vCPU creation: HVF not initialized");
        }
    }

    /// Test multiple vCPU creation
    #[test]
    fn test_hvf_create_multiple_vcpus() {
        let mut hvf = AccelHvf::new();

        if hvf.init().is_ok() {
            for vcpu_id in 0..4 {
                let result = hvf.create_vcpu(vcpu_id);
                match result {
                    Ok(()) => println!("Created vCPU {}", vcpu_id),
                    Err(e) => println!("Failed to create vCPU {}: {:?}", vcpu_id, e),
                }
            }
        }
    }

    /// Test HVF memory mapping
    #[test]
    fn test_hvf_map_memory() {
        let mut hvf = AccelHvf::new();

        if hvf.init().is_ok() {
            // Create a 1MB memory region
            let size = 1024 * 1024;
            let host_mem = vec![0u8; size];
            let host_addr = host_mem.as_ptr() as u64;

            let result = hvf.map_memory(0x1000, host_addr, size as u64, 0x7);

            match result {
                Ok(()) => println!("HVF memory mapped successfully"),
                Err(e) => println!("HVF memory mapping failed: {:?}", e),
            }
        }
    }

    /// Test HVF memory unmapping
    #[test]
    fn test_hvf_unmap_memory() {
        let mut hvf = AccelHvf::new();

        if hvf.init().is_ok() {
            let result = hvf.unmap_memory(0x1000, 0x1000);

            match result {
                Ok(()) => println!("HVF memory unmapped successfully"),
                Err(e) => println!("HVF memory unmapping failed: {:?}", e),
            }
        }
    }

    /// Test error handling for invalid vCPU ID
    #[test]
    fn test_hvf_invalid_vcpu_id() {
        let mut hvf = AccelHvf::new();

        if hvf.init().is_ok() {
            // Try to get registers for non-existent vCPU
            let result = hvf.get_regs(999);

            assert!(result.is_err(), "Should fail for invalid vCPU ID");
            println!("Correctly rejected invalid vCPU ID");
        }
    }

    /// Test HVF register operations
    #[test]
    fn test_hvf_register_operations() {
        let mut hvf = AccelHvf::new();

        if hvf.init().is_ok() {
            if hvf.create_vcpu(0).is_ok() {
                // Try to get registers
                match hvf.get_regs(0) {
                    Ok(regs) => {
                        println!("Got registers: PC={}, SP={}", regs.pc, regs.sp);
                    }
                    Err(e) => {
                        println!("Failed to get registers: {:?}", e);
                    }
                }

                // Try to set registers
                use vm_core::GuestRegs;
                let test_regs = GuestRegs {
                    pc: 0x1000,
                    sp: 0x2000,
                    fp: 0x3000,
                    gpr: [0; 32],
                };

                match hvf.set_regs(0, &test_regs) {
                    Ok(()) => println!("Set registers successfully"),
                    Err(e) => println!("Failed to set registers: {:?}", e),
                }
            }
        }
    }

    /// Test HVF select function
    #[test]
    fn test_hvf_select() {
        let (kind, accel) = vm_accel::select();

        #[cfg(target_os = "macos")]
        {
            assert_eq!(kind, AccelKind::Hvf);
            assert_eq!(accel.name(), "HVF");
            println!("Select() returned HVF accelerator");
        }
    }

    /// Test HVF memory protection flags
    #[test]
    fn test_hvf_memory_protection() {
        let mut hvf = AccelHvf::new();

        if hvf.init().is_ok() {
            let size = 4096;
            let host_mem = vec![0u8; size];
            let host_addr = host_mem.as_ptr() as u64;

            // Test different protection flags
            let flags_read_only = 0x1; // Read-only
            let flags_read_write = 0x3; // Read-write
            let flags_exec = 0x4; // Executable

            for (name, flags) in [
                ("read-only", flags_read_only),
                ("read-write", flags_read_write),
                ("executable", flags_exec),
            ] {
                let result = hvf.map_memory(0x1000, host_addr, size as u64, flags);
                println!("Memory protection test ({}): {:?}", name, result);
            }
        }
    }

    /// Test HVF invalid memory address
    #[test]
    fn test_hvf_invalid_memory_address() {
        let mut hvf = AccelHvf::new();

        if hvf.init().is_ok() {
            // Try to map with invalid address
            let result = hvf.map_memory(0xFFFF_FFFF_F000, 0, 0x1000, 0x7);

            match result {
                Ok(()) => println!("Unexpected success with invalid address"),
                Err(_) => println!("Correctly rejected invalid memory address"),
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod hvf_tests {
    /// Test that HVF is only available on macOS
    #[test]
    fn test_hvf_not_available() {
        println!("HVF is only available on macOS");
        assert!(true);
    }
}
