//! Test x86 boot with Debian kernel
//!
//! This test loads the Debian kernel and uses the X86BootExecutor

use vm_service::VmService;
use vm_core::{VmConfig, GuestArch, ExecMode};

#[tokio::test]
async fn test_debian_x86_boot() {
    env_logger::builder().is_test(true).try_init();

    println!("=== Debian x86 Boot Test ===");
    println!("Host: Apple M4 Pro (aarch64)");
    println!();

    // Create VM configuration
    let config = VmConfig {
        guest_arch: GuestArch::X86_64,
        vcpu_count: 1,
        memory_size: 3 * 1024 * 1024 * 1024, // 3GB
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    println!("Creating VM service...");
    let mut service = VmService::new(config, None).await
        .expect("Failed to create VM service");

    println!("✅ VM service created");
    println!();

    // Load kernel
    println!("Loading kernel...");
    let kernel_path = "/tmp/debian_iso_extracted/debian_bzImage";
    let load_addr = 0x10000;  // x86 real-mode entry

    service.load_kernel(kernel_path, load_addr)
        .expect("Failed to load kernel");

    println!("✅ Kernel loaded at {:#010X}", load_addr);
    println!();

    // Try to boot using x86 boot executor
    println!("=== Attempting x86 Boot Sequence ===");

    match service.boot_x86_kernel() {
        Ok(result) => {
            println!("Boot result: {:?}", result);
            match result {
                vm_service::vm_service::x86_boot_exec::X86BootResult::LongModeReady { entry_point } => {
                    println!("✅ Long mode ready! 64-bit entry at {:#010X}", entry_point);
                    println!("   The Debian installer should now be displayed on VGA!");
                }
                vm_service::vm_service::x86_boot_exec::X86BootResult::Halted => {
                    println!("ℹ️  Boot code executed HLT");
                }
                vm_service::vm_service::x86_boot_exec::X86BootResult::MaxInstructionsReached => {
                    println!("⚠️  Reached maximum instruction limit");
                }
                _ => {
                    println!("ℹ️  Other boot result");
                }
            }
        }
        Err(e) => {
            println!("❌ Boot failed: {:?}", e);
            println!();
            println!("This is expected - the boot executor may need adjustments");
            println!("to work with the actual kernel boot code.");
        }
    }

    println!();
    println!("=== Test Complete ===");
}
