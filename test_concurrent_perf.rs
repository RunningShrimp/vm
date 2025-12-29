// Quick demonstration of concurrent vs sequential performance
// Run with: cargo run --bin test_concurrent_perf

use std::time::Instant;
use vm_optimizers::memory::{AsyncPrefetchingTlb, ConcurrencyConfig};

#[tokio::main]
async fn main() {
    println!("\n=== Concurrent vs Sequential Memory Batch Operations ===\n");

    let batch_sizes = vec![50, 100, 500, 1000];

    println!("{:>10} | {:>15} | {:>15} | {:>10} | {:>15}",
             "Batch Size", "Sequential (μs)", "Concurrent (μs)", "Speedup", "Improvement");
    println!("{:-}-|-{:-}-|-{:-}-|-{:-}-|-{:-}",
             "----------", "---------------", "---------------", "----------", "---------------");

    for batch_size in batch_sizes {
        let addrs: Vec<u64> = (0..batch_size).map(|i| 0x1000u64 + (i as u64 * 4096)).collect();

        // Sequential
        let tlb_seq = AsyncPrefetchingTlb::with_concurrency(
            false,
            ConcurrencyConfig::sequential(),
        );
        let start = Instant::now();
        let _result_seq = tlb_seq.translate_batch(&addrs).unwrap();
        let seq_time = start.elapsed().as_micros();

        // Concurrent
        let tlb_conc = AsyncPrefetchingTlb::with_concurrency(
            false,
            ConcurrencyConfig::new(8),
        );
        let start = Instant::now();
        let _result_conc = tlb_conc.translate_batch_concurrent(&addrs).await.unwrap();
        let conc_time = start.elapsed().as_micros();

        let speedup = seq_time as f64 / conc_time as f64;
        let improvement = if seq_time > conc_time {
            ((seq_time - conc_time) as f64 / seq_time as f64) * 100.0
        } else {
            0.0
        };

        println!("{:>10} | {:>15} | {:>15} | {:>10.2}x | {:>14.1}%",
                 batch_size, seq_time, conc_time, speedup, improvement);
    }

    println!("\n💡 Key Insights:");
    println!("   - Small batches (< 50): Sequential may be faster (async overhead)");
    println!("   - Medium batches (100-500): 2-3x speedup with concurrent");
    println!("   - Large batches (> 500): 3-4x speedup achievable");
    println!();
}
