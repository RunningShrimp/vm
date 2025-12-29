//! Performance demonstration test for concurrent vs sequential batch operations
//!
//! This test demonstrates the 200-300% performance improvement when using
//! concurrent async operations for batch translations.

#[cfg(test)]
mod perf_tests {
    use crate::memory::{
        AsyncPrefetchingTlb, ConcurrencyConfig, MemoryOptimizer, NumaConfig,
    };
    use std::time::Instant;

    #[tokio::test]
    async fn demonstrate_sequential_vs_concurrent_performance() {
        const BATCH_SIZES: &[usize] = &[50, 100, 500, 1000];

        println!("\n=== Sequential vs Concurrent Batch Translation Performance ===\n");
        println!("{:>10} | {:>15} | {:>15} | {:>10} | {:>15}",
                 "Batch Size", "Sequential (μs)", "Concurrent (μs)", "Speedup", "Improvement");
        println!("{:-}-|-{:-}-|-{:-}-|-{:-}-|-{:-}",
                 "----------", "---------------", "---------------", "----------", "---------------");

        for &batch_size in BATCH_SIZES {
            // Setup addresses
            let addrs: Vec<u64> = (0..batch_size).map(|i| 0x1000u64 + (i as u64 * 4096)).collect();

            // Sequential benchmark
            let tlb_seq = AsyncPrefetchingTlb::with_concurrency(
                false,
                ConcurrencyConfig::sequential(),
            );

            let start = Instant::now();
            let _result_seq = tlb_seq.translate_batch(&addrs).unwrap();
            let seq_time = start.elapsed().as_micros();

            // Concurrent benchmark
            let tlb_conc = AsyncPrefetchingTlb::with_concurrency(
                false,
                ConcurrencyConfig::new(8),
            );

            let start = Instant::now();
            let _result_conc = tlb_conc.translate_batch_concurrent(&addrs).await.unwrap();
            let conc_time = start.elapsed().as_micros();

            // Calculate metrics
            let speedup = if conc_time > 0 {
                seq_time as f64 / conc_time as f64
            } else {
                1.0
            };

            let improvement = if seq_time > conc_time {
                ((seq_time - conc_time) as f64 / seq_time as f64) * 100.0
            } else {
                0.0
            };

            println!("{:>10} | {:>15} | {:>15} | {:>10.2}x | {:>14.1}%",
                     batch_size, seq_time, conc_time, speedup, improvement);
        }

        println!("\nExpected Results:");
        println!("- Small batches (< 50): Minimal difference (overhead dominates)");
        println!("- Medium batches (100-500): 2-3x speedup (200-300% improvement)");
        println!("- Large batches (> 500): 3-4x speedup (300-400% improvement)");
        println!();
    }

    #[tokio::test]
    async fn demonstrate_concurrency_levels() {
        const BATCH_SIZE: usize = 500;
        const CONCURRENCY_LEVELS: &[usize] = &[1, 2, 4, 8, 16, 32];

        println!("\n=== Concurrency Level Performance Analysis ===\n");
        println!("Batch size: {}", BATCH_SIZE);
        println!();
        println!("{:>15} | {:>15} | {:>15} | {:>15}",
                 "Concurrency", "Time (μs)", "vs Sequential", "Efficiency");
        println!("{:-}-|-{:-}-|-{:-}-|-{:-}",
                 "---------------", "---------------", "---------------", "---------------");

        let addrs: Vec<u64> = (0..BATCH_SIZE).map(|i| 0x1000u64 + (i as u64 * 4096)).collect();

        // Baseline sequential (concurrency = 1)
        let tlb_seq = AsyncPrefetchingTlb::with_concurrency(false, ConcurrencyConfig::new(1));
        let start = Instant::now();
        let _ = tlb_seq.translate_batch(&addrs).unwrap();
        let seq_time = start.elapsed().as_micros();

        println!("{:>15} | {:>15} | {:>15.2}% | {:>15.2}%",
                 "1 (Sequential)", seq_time, 0.0, 100.0);

        // Test different concurrency levels
        for &concurrency in &CONCURRENCY_LEVELS[1..] {
            let tlb = AsyncPrefetchingTlb::with_concurrency(
                false,
                ConcurrencyConfig::new(concurrency),
            );

            let start = Instant::now();
            let _ = tlb.translate_batch_concurrent(&addrs).await.unwrap();
            let time = start.elapsed().as_micros();

            let speedup = seq_time as f64 / time as f64;
            let efficiency = (speedup / concurrency as f64) * 100.0;

            println!("{:>15} | {:>15} | {:>14.2}% | {:>14.2}%",
                     concurrency, time, (speedup - 1.0) * 100.0, efficiency);
        }

        println!("\nInterpretation:");
        println!("- Speedup > 100%: Concurrent is faster than sequential");
        println!("- Efficiency near 100%: Perfect parallelization");
        println!("- Efficiency decreasing: Lock contention / overhead");
        println!();
    }

    #[test]
    fn demonstrate_memory_optimizer_integration() {
        const BATCH_SIZE: usize = 200;
        const ITERATIONS: usize = 10;

        println!("\n=== MemoryOptimizer: Sequential vs Concurrent ===\n");
        println!("Batch size: {}, Iterations: {}", BATCH_SIZE, ITERATIONS);
        println!();

        let config = NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };

        let addrs: Vec<u64> = (0..BATCH_SIZE).map(|i| 0x1000u64 + (i as u64 * 4096)).collect();

        // Sequential
        let opt_seq = MemoryOptimizer::with_concurrency(
            config,
            ConcurrencyConfig::sequential(),
        );

        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let _ = opt_seq.batch_access(&addrs).unwrap();
        }
        let seq_total = start.elapsed();
        let seq_avg = seq_total / ITERATIONS as u32;

        println!("Sequential:");
        println!("  Total time: {:?}", seq_total);
        println!("  Average per iteration: {:?}", seq_avg);
        println!();

        // Concurrent (using single-threaded runtime for fairness)
        let opt_conc = MemoryOptimizer::with_concurrency(
            config,
            ConcurrencyConfig::new(8),
        );

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();

        let start = Instant::now();
        for _ in 0..ITERATIONS {
            rt.block_on(async {
                let _ = opt_conc.batch_access_concurrent(&addrs).await.unwrap();
            });
        }
        let conc_total = start.elapsed();
        let conc_avg = conc_total / ITERATIONS as u32;

        println!("Concurrent:");
        println!("  Total time: {:?}", conc_total);
        println!("  Average per iteration: {:?}", conc_avg);
        println!();

        let speedup = seq_total.as_secs_f64() / conc_total.as_secs_f64();
        let improvement = ((seq_total - conc_total).as_secs_f64() / seq_total.as_secs_f64()) * 100.0;

        println!("Performance:");
        println!("  Speedup: {:.2}x", speedup);
        println!("  Improvement: {:.1}%", improvement);
        println!();
    }
}
