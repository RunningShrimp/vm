//! SMMU Device Assignment Example
//!
//! This example demonstrates how to use the SMMU-aware device assignment
//! functionality in vm-device.

use std::sync::Arc;
use vm_accel::SmmuManager;
use vm_core::GuestAddr;
use vm_device::smmu_device::SmmuDeviceManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("=== ARM SMMU Device Assignment Example ===\n");

    // Step 1: Create SMMU manager
    println!("Step 1: Creating SMMU manager...");
    let smmu_manager = Arc::new(SmmuManager::new());

    // Step 2: Initialize SMMU
    println!("Step 2: Initializing SMMU...");
    smmu_manager.init()?;
    println!("SMMU initialized successfully\n");

    // Step 3: Create SMMU device manager
    println!("Step 3: Creating SMMU device manager...");
    let device_manager = SmmuDeviceManager::new(smmu_manager.clone());
    println!("Device manager created\n");

    // Step 4: Assign devices to SMMU
    println!("Step 4: Assigning devices to SMMU...");

    // Assign a network card
    let net_bdf = "0000:01:00.0";
    let net_dma_start = 0x1000_0000;
    let net_dma_size = 0x1000_000; // 16 MB

    match device_manager.assign_device(net_bdf, net_dma_start, net_dma_size) {
        Ok(stream_id) => {
            println!(
                "✓ Network card {} assigned with stream ID 0x{:04x}",
                net_bdf, stream_id
            );
        }
        Err(e) => {
            println!("✗ Failed to assign network card: {}", e);
        }
    }

    // Assign a GPU
    let gpu_bdf = "0000:02:00.0";
    let gpu_dma_start = 0x2000_0000;
    let gpu_dma_size = 0x1000_0000; // 256 MB

    match device_manager.assign_device(gpu_bdf, gpu_dma_start, gpu_dma_size) {
        Ok(stream_id) => {
            println!(
                "✓ GPU {} assigned with stream ID 0x{:04x}",
                gpu_bdf, stream_id
            );
        }
        Err(e) => {
            println!("✗ Failed to assign GPU: {}", e);
        }
    }

    println!();

    // Step 5: List assigned devices
    println!("Step 5: Listing assigned devices...");
    let devices = device_manager.list_devices();
    println!("Total assigned devices: {}", devices.len());
    for bdf in &devices {
        if let Some(info) = device_manager.get_device_info(bdf) {
            println!(
                "  - {}: Stream ID 0x{:04x}, DMA range 0x{:x}-0x{:x}",
                bdf, info.stream_id, info.dma_range.0.0, info.dma_range.1.0
            );
        }
    }
    println!();

    // Step 6: Perform DMA address translation
    println!("Step 6: Testing DMA address translation...");
    let test_guest_addr = GuestAddr(0x1000_1000);
    let test_size = 0x1000;

    match device_manager.translate_dma_addr(net_bdf, test_guest_addr, test_size) {
        Ok(phys_addr) => {
            println!(
                "✓ DMA translation: GPA 0x{:x} -> HPA 0x{:x}",
                test_guest_addr.0, phys_addr
            );
        }
        Err(e) => {
            println!("✗ DMA translation failed: {}", e);
        }
    }
    println!();

    // Step 7: Get SMMU statistics
    println!("Step 7: Getting SMMU statistics...");
    match smmu_manager.get_stats() {
        Ok(stats) => {
            println!("{}", stats);
        }
        Err(e) => {
            println!("✗ Failed to get stats: {:?}", e);
        }
    }
    println!();

    // Step 8: Unassign a device
    println!("Step 8: Unassigning network card...");
    match device_manager.unassign_device(net_bdf) {
        Ok(()) => {
            println!("✓ Network card unassigned successfully");
        }
        Err(e) => {
            println!("✗ Failed to unassign network card: {}", e);
        }
    }
    println!();

    // Final device count
    println!("Final device count: {}", device_manager.device_count());

    println!("=== Example Complete ===");
    Ok(())
}
