//! Simple performance test for memory read operations
//!
//! This directly measures the performance of different-sized reads

use std::time::Instant;

fn main() {
    let mem_size = 16 * 1024 * 1024; // 16MB
    let mem = vm_mem::PhysicalMemory::new(mem_size, false);

    // Initialize memory
    println!("Initializing 16MB memory...");
    for i in 0..(1024 * 1024 / 8) {
        mem.write_u64(i * 8, 0xABCD1234567890EF).unwrap();
    }

    let iterations = 1000;
    let test_size = 16384; // Number of reads per iteration (128KB total)

    println!(
        "\nRunning {} iterations of {} reads each...\n",
        iterations, test_size
    );

    // Test u8 reads
    let start = Instant::now();
    let mut sum_u8 = 0u64;
    for _ in 0..iterations {
        for i in 0..test_size {
            sum_u8 = sum_u8.wrapping_add(mem.read_u8(i).unwrap() as u64);
        }
    }
    let elapsed_u8 = start.elapsed();
    let total_bytes_u8 = iterations * test_size;
    println!(
        "u8 reads:  {:>10.2} ms  ({:>8.2} ns/read  {:>8.2} MB/s)",
        elapsed_u8.as_micros() as f64 / 1000.0,
        elapsed_u8.as_nanos() as f64 / (total_bytes_u8 as f64),
        (total_bytes_u8 as f64) / (elapsed_u8.as_secs_f64() * 1024.0 * 1024.0)
    );

    // Test u16 reads
    let start = Instant::now();
    let mut sum_u16 = 0u64;
    for _ in 0..iterations {
        for i in 0..(test_size / 2) {
            sum_u16 = sum_u16.wrapping_add(mem.read_u16(i * 2).unwrap() as u64);
        }
    }
    let elapsed_u16 = start.elapsed();
    let total_bytes_u16 = iterations * (test_size / 2) * 2;
    println!(
        "u16 reads: {:>10.2} ms  ({:>8.2} ns/read  {:>8.2} MB/s)",
        elapsed_u16.as_micros() as f64 / 1000.0,
        elapsed_u16.as_nanos() as f64 / ((test_size / 2) as f64),
        (total_bytes_u16 as f64) / (elapsed_u16.as_secs_f64() * 1024.0 * 1024.0)
    );

    // Test u32 reads
    let start = Instant::now();
    let mut sum_u32 = 0u64;
    for _ in 0..iterations {
        for i in 0..(test_size / 4) {
            sum_u32 = sum_u32.wrapping_add(mem.read_u32(i * 4).unwrap() as u64);
        }
    }
    let elapsed_u32 = start.elapsed();
    let total_bytes_u32 = iterations * (test_size / 4) * 4;
    println!(
        "u32 reads: {:>10.2} ms  ({:>8.2} ns/read  {:>8.2} MB/s)",
        elapsed_u32.as_micros() as f64 / 1000.0,
        elapsed_u32.as_nanos() as f64 / ((test_size / 4) as f64),
        (total_bytes_u32 as f64) / (elapsed_u32.as_secs_f64() * 1024.0 * 1024.0)
    );

    // Test u64 reads
    let start = Instant::now();
    let mut sum_u64 = 0u64;
    for _ in 0..iterations {
        for i in 0..(test_size / 8) {
            sum_u64 = sum_u64.wrapping_add(mem.read_u64(i * 8).unwrap());
        }
    }
    let elapsed_u64 = start.elapsed();
    let total_bytes_u64 = iterations * (test_size / 8) * 8;
    println!(
        "u64 reads: {:>10.2} ms  ({:>8.2} ns/read  {:>8.2} MB/s)",
        elapsed_u64.as_micros() as f64 / 1000.0,
        elapsed_u64.as_nanos() as f64 / ((test_size / 8) as f64),
        (total_bytes_u64 as f64) / (elapsed_u64.as_secs_f64() * 1024.0 * 1024.0)
    );

    println!("\nChecksums (prevent optimization):");
    println!("  u8:  {}", sum_u8);
    println!("  u16: {}", sum_u16);
    println!("  u32: {}", sum_u32);
    println!("  u64: {}", sum_u64);

    println!("\nPerformance Summary (speedup vs u8):");
    println!(
        "  u16: {:.2}x",
        (elapsed_u8.as_nanos() as f64 / (test_size as f64))
            / (elapsed_u16.as_nanos() as f64 / ((test_size / 2) as f64))
    );
    println!(
        "  u32: {:.2}x",
        (elapsed_u8.as_nanos() as f64 / (test_size as f64))
            / (elapsed_u32.as_nanos() as f64 / ((test_size / 4) as f64))
    );
    println!(
        "  u64: {:.2}x",
        (elapsed_u8.as_nanos() as f64 / (test_size as f64))
            / (elapsed_u64.as_nanos() as f64 / ((test_size / 8) as f64))
    );

    println!("\nPer-byte efficiency (should be similar for all):");
    println!(
        "  u8:  {:>8.2} ns/byte",
        elapsed_u8.as_nanos() as f64 / total_bytes_u8 as f64
    );
    println!(
        "  u16: {:>8.2} ns/byte",
        elapsed_u16.as_nanos() as f64 / total_bytes_u16 as f64
    );
    println!(
        "  u32: {:>8.2} ns/byte",
        elapsed_u32.as_nanos() as f64 / total_bytes_u32 as f64
    );
    println!(
        "  u64: {:>8.2} ns/byte",
        elapsed_u64.as_nanos() as f64 / total_bytes_u64 as f64
    );
}
