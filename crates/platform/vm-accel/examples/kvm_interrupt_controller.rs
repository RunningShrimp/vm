//! KVM Interrupt Controller and Device Assignment Example
//!
//! This example demonstrates how to use the enhanced KVM integration with:
//! - Interrupt controller setup
//! - IRQ line management
//! - IRQ routing configuration
//! - PCI device assignment

use vm_accel::AccelKvm;
use vm_core::{PlatformError, VmError};

fn main() -> Result<(), VmError> {
    // Initialize logging
    env_logger::init();

    println!("=== KVM Interrupt Controller Example ===\n");

    // Check if KVM is available
    if !AccelKvm::is_available() {
        println!("Error: KVM is not available on this system");
        println!("Please ensure:");
        println!("  1. You are running on a Linux system");
        println!("  2. /dev/kvm exists");
        println!("  3. You have permissions to access /dev/kvm");
        return Err(VmError::Platform(PlatformError::HardwareUnavailable(
            "KVM not available".to_string(),
        )));
    }

    println!("✓ KVM is available\n");

    // Create and initialize KVM accelerator
    let mut kvm_accel = AccelKvm::new();
    kvm_accel.init()?;
    println!("✓ KVM accelerator initialized\n");

    // Create a vCPU
    kvm_accel.create_vcpu(0)?;
    println!("✓ Created vCPU 0\n");

    // Setup interrupt controller
    println!("--- Setting up interrupt controller ---");
    match kvm_accel.setup_irq_controller() {
        Ok(_) => println!("✓ Interrupt controller created successfully\n"),
        Err(e) => {
            println!("✗ Failed to create interrupt controller: {}\n", e);
            println!("This is expected if:");
            println!("  - The VM already has memory slots configured");
            println!("  - The kernel doesn't support in-kernel irqchip");
        }
    }

    // Try to set IRQ lines (only works if irqchip was created)
    println!("--- Setting IRQ lines ---");
    match kvm_accel.set_irq_line(4, true) {
        Ok(_) => println!("✓ IRQ 4 set to active (high)\n"),
        Err(e) => {
            println!("Note: Could not set IRQ line: {}\n", e);
            println!("This is expected if irqchip creation failed");
        }
    }

    match kvm_accel.set_irq_line(4, false) {
        Ok(_) => println!("✓ IRQ 4 set to inactive (low)\n"),
        Err(_) => {}
    }

    // Try IRQ routing on x86_64
    #[cfg(target_arch = "x86_64")]
    {
        println!("--- Setting up IRQ routing ---");
        match kvm_accel.setup_irq_routing(4, 0, 4) {
            Ok(_) => println!("✓ IRQ routing configured: GSI 4 -> IOAPIC pin 4\n"),
            Err(e) => {
                println!("Note: Could not setup IRQ routing: {}\n", e);
                println!("This is expected if irqchip creation failed");
            }
        }
    }

    // Example: Device assignment
    println!("--- PCI Device Assignment ---");
    println!("Attempting to assign a PCI device...");
    println!("Note: This requires:");
    println!("  1. Root access or appropriate permissions");
    println!("  2. IOMMU support in hardware and kernel");
    println!("  3. The device to not be in use by the host");
    println!();

    // This will fail without a real device, but demonstrates the API
    match kvm_accel.assign_pci_device("0000:01:00.0") {
        Ok(_) => println!("✓ PCI device 0000:01:00.0 assigned successfully\n"),
        Err(e) => {
            println!("Note: Could not assign PCI device: {}\n", e);
            println!("This is expected if:");
            println!("  - No such device exists");
            println!("  - Running without sufficient permissions");
            println!("  - Device is already in use");
            println!("  - IOMMU is not enabled");
        }
    }

    println!("=== Example Complete ===");
    println!("\nKey features demonstrated:");
    println!("  1. Interrupt controller setup (setup_irq_controller)");
    println!("  2. IRQ line management (set_irq_line)");
    println!("  3. IRQ routing configuration (setup_irq_routing) [x86_64 only]");
    println!("  4. PCI device assignment (assign_pci_device)");

    Ok(())
}
