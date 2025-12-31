#!/bin/bash
# Quick performance test for memory reads

set -e

echo "=== Memory Read Performance Test ==="
echo ""

# Build the test binary
cd /Users/wangbiao/Desktop/project/vm
cargo build --release --example simple_memory_test 2>&1 | grep -E "(Compiling|Finished|error)" || true

# Run if it exists
if [ -f target/release/examples/simple_memory_test ]; then
    echo "Running memory read performance test..."
    target/release/examples/simple_memory_test
else
    echo "Creating and running a quick inline test..."

    # Create a temporary Rust file
    cat > /tmp/test_mem_perf.rs << 'EOF'
use std::time::{Duration, Instant};

fn main() {
    let mem_size = 16 * 1024 * 1024; // 16MB
    let mem = vm_mem::PhysicalMemory::new(mem_size, false);

    // Initialize memory
    for i in 0..(1024 * 1024 / 8) {
        mem.write_u64(i * 8, 0xABCD1234567890EF).unwrap();
    }

    let iterations = 1000;

    // Test u8 reads
    let start = Instant::now();
    let mut sum_u8 = 0u8;
    for _ in 0..iterations {
        for i in 0..16384 {
            sum_u8 = sum_u8.wrapping_add(mem.read_u8(i).unwrap());
        }
    }
    let elapsed_u8 = start.elapsed();
    println!("u8 reads:  {:>8.2} ms (sum: {})", elapsed_u8.as_micros() as f64 / 1000.0, sum_u8);

    // Test u16 reads
    let start = Instant::now();
    let mut sum_u16 = 0u16;
    for _ in 0..iterations {
        for i in 0..8192 {
            sum_u16 = sum_u16.wrapping_add(mem.read_u16(i * 2).unwrap());
        }
    }
    let elapsed_u16 = start.elapsed();
    println!("u16 reads: {:>8.2} ms (sum: {})", elapsed_u16.as_micros() as f64 / 1000.0, sum_u16);

    // Test u32 reads
    let start = Instant::now();
    let mut sum_u32 = 0u32;
    for _ in 0..iterations {
        for i in 0..4096 {
            sum_u32 = sum_u32.wrapping_add(mem.read_u32(i * 4).unwrap());
        }
    }
    let elapsed_u32 = start.elapsed();
    println!("u32 reads: {:>8.2} ms (sum: {})", elapsed_u32.as_micros() as f64 / 1000.0, sum_u32);

    // Test u64 reads
    let start = Instant::now();
    let mut sum_u64 = 0u64;
    for _ in 0..iterations {
        for i in 0..2048 {
            sum_u64 = sum_u64.wrapping_add(mem.read_u64(i * 8).unwrap());
        }
    }
    let elapsed_u64 = start.elapsed();
    println!("u64 reads: {:>8.2} ms (sum: {})", elapsed_u64.as_micros() as f64 / 1000.0, sum_u64);

    println!("");
    println!("Performance Summary (relative to u8):");
    println!("  u16: {:.2}x", elapsed_u8.as_nanos() as f64 / elapsed_u16.as_nanos() as f64);
    println!("  u32: {:.2}x", elapsed_u8.as_nanos() as f64 / elapsed_u32.as_nanos() as f64);
    println!("  u64: {:.2}x", elapsed_u8.as_nanos() as f64 / elapsed_u64.as_nanos() as f64);
}
EOF

    # Compile and run
    rustc --edition 2021 -O --cfg test /tmp/test_mem_perf.rs \
        -L target/release/deps \
        --extern vm_mem=target/release/libvm_mem.rlib \
        -o /tmp/test_mem_perf 2>&1 | head -20 || true

    if [ -f /tmp/test_mem_perf ]; then
        /tmp/test_mem_perf
    else
        echo "Failed to compile test, using direct benchmark..."
        cargo bench --bench memory_read_bench 2>&1 | tail -50
    fi
fi
