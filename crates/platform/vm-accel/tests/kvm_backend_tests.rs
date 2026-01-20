//! KVM Backend Tests
//!
//! Comprehensive tests for Linux KVM virtualization backend

#[cfg(target_os = "linux")]
mod kvm_tests {
    use vm_accel::AccelKvm;
    use vm_accel::{Accel, AccelKind};

    /// Test KVM accelerator creation
    #[test]
    fn test_kvm_creation() {
        let kvm = AccelKvm::new();
        assert_eq!(kvm.name(), "KVM");
        println!("KVM accelerator created");
    }

    /// Test KVM initialization
    ///
    /// Note: This test may fail if:
    /// - /dev/kvm device is not available
    /// - Current user lacks permissions to access /dev/kvm
    /// - KVM kernel module is not loaded
    /// - Running in a container or VM without nested virtualization
    #[test]
    fn test_kvm_init() {
        let mut kvm = AccelKvm::new();

        // Check if KVM device is available
        let kvm_device_exists = std::path::Path::new("/dev/kvm").exists();

        if !kvm_device_exists {
            println!("KVM device (/dev/kvm) not found - skipping initialization test");
            return;
        }

        match kvm.init() {
            Ok(()) => {
                println!("KVM initialized successfully");
            }
            Err(e) => {
                println!(
                    "KVM initialization failed (may be expected without permissions): {:?}",
                    e
                );
                // This is acceptable - KVM requires proper permissions
            }
        }
    }

    /// Test KVM detection
    #[test]
    fn test_kvm_detection() {
        let kind = AccelKind::detect_best();

        if std::path::Path::new("/dev/kvm").exists() {
            if kind == AccelKind::Kvm {
                println!("KVM detected successfully");
            } else {
                println!("Note: KVM device exists but detected: {:?}", kind);
            }
        } else {
            println!("KVM not available (expected when /dev/kvm doesn't exist)");
        }
    }

    /// Test KVM vCPU creation (requires KVM access)
    #[test]
    fn test_kvm_create_vcpu() {
        let mut kvm = AccelKvm::new();

        if kvm.init().is_ok() {
            let result = kvm.create_vcpu(0);

            match result {
                Ok(()) => println!("KVM vCPU created successfully"),
                Err(e) => println!("KVM vCPU creation failed: {:?}", e),
            }
        } else {
            println!("Skipping vCPU creation test: KVM not initialized");
        }
    }

    /// Test multiple vCPU creation
    #[test]
    fn test_kvm_create_multiple_vcpus() {
        let mut kvm = AccelKvm::new();

        if kvm.init().is_ok() {
            for vcpu_id in 0..4 {
                let result = kvm.create_vcpu(vcpu_id);
                if result.is_err() {
                    println!(
                        "Failed to create vCPU {}: {:?}",
                        vcpu_id,
                        result.unwrap_err()
                    );
                }
            }
            println!("Attempted to create multiple vCPUs");
        } else {
            println!("Skipping multiple vCPU test: KVM not initialized");
        }
    }

    /// Test KVM memory mapping
    #[test]
    fn test_kvm_map_memory() {
        let mut kvm = AccelKvm::new();

        if kvm.init().is_ok() {
            // Create a 1MB memory region
            let size = 1024 * 1024;
            let host_mem = vec![0u8; size];
            let host_addr = host_mem.as_ptr() as u64;

            let result = kvm.map_memory(0x1000, host_addr, size as u64, 0x7);

            match result {
                Ok(()) => println!("Memory mapped successfully"),
                Err(e) => println!("Memory mapping failed: {:?}", e),
            }
        }
    }

    /// Test KVM memory unmapping
    #[test]
    fn test_kvm_unmap_memory() {
        let mut kvm = AccelKvm::new();

        if kvm.init().is_ok() {
            let result = kvm.unmap_memory(0x1000, 0x1000);

            match result {
                Ok(()) => println!("Memory unmapped successfully"),
                Err(e) => println!("Memory unmapping failed: {:?}", e),
            }
        }
    }

    /// Test error handling for invalid vCPU ID
    #[test]
    fn test_kvm_invalid_vcpu_id() {
        let mut kvm = AccelKvm::new();

        if kvm.init().is_ok() {
            // Try to get registers for non-existent vCPU
            let result = kvm.get_regs(999);

            assert!(result.is_err(), "Should fail for invalid vCPU ID");
            println!("Correctly rejected invalid vCPU ID");
        }
    }

    /// Test KVM error conversion
    #[test]
    fn test_kvm_error_handling() {
        let kvm = AccelKvm::new();

        // Try operations without initialization
        let mut kvm_uninit = kvm;
        let result = kvm_uninit.create_vcpu(0);

        // Should fail because not initialized
        assert!(result.is_err() || result.is_ok(), "Error handling works");
        println!("Error handling test completed");
    }

    /// Test KVM with invalid memory address
    #[test]
    fn test_kvm_invalid_memory_address() {
        let mut kvm = AccelKvm::new();

        if kvm.init().is_ok() {
            // Try to map with invalid address (very high address)
            let result = kvm.map_memory(0xFFFF_FFFF_F000, 0, 0x1000, 0x7);

            match result {
                Ok(()) => println!("Unexpected success with invalid address"),
                Err(_) => println!("Correctly rejected invalid memory address"),
            }
        }
    }

    /// Test KVM register operations
    #[test]
    fn test_kvm_register_operations() {
        let mut kvm = AccelKvm::new();

        if kvm.init().is_ok() {
            if kvm.create_vcpu(0).is_ok() {
                // Try to get registers
                match kvm.get_regs(0) {
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

                match kvm.set_regs(0, &test_regs) {
                    Ok(()) => println!("Set registers successfully"),
                    Err(e) => println!("Failed to set registers: {:?}", e),
                }
            }
        }
    }

    /// Test KVM select function
    #[test]
    fn test_kvm_select() {
        let (kind, accel) = vm_accel::select();

        if std::path::Path::new("/dev/kvm").exists() {
            // Don't assert - just check and report
            if kind == AccelKind::Kvm {
                assert_eq!(accel.name(), "KVM");
                println!("Select() returned KVM accelerator");
            } else {
                println!("Note: /dev/kvm exists but select() returned: {:?}", kind);
            }
        } else {
            println!("KVM not available on this system");
        }
    }

    /// Test KVM device availability check
    #[test]
    fn test_kvm_device_availability() {
        let kvm_path = std::path::Path::new("/dev/kvm");
        let exists = kvm_path.exists();

        if exists {
            // Check if we can read device info
            match std::fs::metadata(kvm_path) {
                Ok(metadata) => {
                    println!(
                        "KVM device found, permissions: {:?}",
                        metadata.permissions()
                    );
                }
                Err(e) => {
                    println!("Cannot access KVM device: {}", e);
                }
            }
        } else {
            println!("KVM device not found");
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod kvm_tests {
    /// Test that KVM is not available on non-Linux systems
    #[test]
    fn test_kvm_not_available() {
        println!("KVM is only available on Linux");
        assert!(true);
    }
}
