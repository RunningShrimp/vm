//! Unified Benchmark Framework for VM Project
//!
//! This module provides a comprehensive benchmarking infrastructure for all VM subsystems.
//! It includes:
//! - Unified benchmark configuration
//! - Result collection and reporting
//! - Baseline management
//! - Statistical analysis

use criterion::{BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Number of warmup iterations
    pub warmup_iters: u64,
    /// Number of measurement iterations
    pub measure_iters: u64,
    /// Measurement time per benchmark
    pub measurement_time: Duration,
    /// Warmup time per benchmark
    pub warmup_time: Duration,
    /// Sample size for statistical accuracy
    pub sample_size: usize,
    /// Output format for results
    pub output_format: OutputFormat,
    /// Whether to save baselines
    pub save_baseline: bool,
    /// Baseline directory path
    pub baseline_dir: PathBuf,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            warmup_iters: 3,
            measure_iters: 10,
            measurement_time: Duration::from_secs(5),
            warmup_time: Duration::from_secs(1),
            sample_size: 100,
            output_format: OutputFormat::Console,
            save_baseline: true,
            baseline_dir: PathBuf::from("benches/baselines"),
        }
    }
}

/// Output format for benchmark results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Console output (default)
    Console,
    /// JSON format
    Json,
    /// Markdown format
    Markdown,
    /// CSV format
    Csv,
}

/// Benchmark result metrics
#[derive(Debug, Clone)]
pub struct BenchmarkMetrics {
    /// Benchmark name
    pub name: String,
    /// Average time per iteration (nanoseconds)
    pub avg_ns: f64,
    /// Standard deviation (nanoseconds)
    pub stddev_ns: f64,
    /// Median time (nanoseconds)
    pub median_ns: f64,
    /// Min time (nanoseconds)
    pub min_ns: f64,
    /// Max time (nanoseconds)
    pub max_ns: f64,
    /// Throughput (elements/second or bytes/second)
    pub throughput: Option<f64>,
    /// Additional custom metrics
    pub custom_metrics: HashMap<String, f64>,
}

impl BenchmarkMetrics {
    /// Calculate coefficient of variation (CV)
    pub fn coefficient_of_variation(&self) -> f64 {
        if self.avg_ns > 0.0 {
            (self.stddev_ns / self.avg_ns) * 100.0
        } else {
            0.0
        }
    }

    /// Format metrics as string
    pub fn format(&self) -> String {
        let cv = self.coefficient_of_variation();
        format!(
            "Avg: {:.2} μs, StdDev: {:.2} μs, Median: {:.2} μs, CV: {:.2}%",
            self.avg_ns / 1000.0,
            self.stddev_ns / 1000.0,
            self.median_ns / 1000.0,
            cv
        )
    }
}

/// Baseline information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Baseline {
    /// Benchmark name
    pub name: String,
    /// Baseline value (e.g., instructions/second, time in ms)
    pub value: f64,
    /// Date when baseline was recorded
    pub date: String,
    /// Git commit hash
    pub commit: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl Baseline {
    /// Create a new baseline
    pub fn new(name: String, value: f64) -> Self {
        Self {
            name,
            value,
            date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            commit: Self::get_git_commit().unwrap_or_else(|| "unknown".to_string()),
            metadata: HashMap::new(),
        }
    }

    /// Get current git commit hash
    fn get_git_commit() -> Option<String> {
        std::process::Command::new("git")
            .args(&["rev-parse", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                let hash = String::from_utf8(output.stdout).ok()?;
                Some(hash.trim().to_string())
            })
    }
}

/// Benchmark baseline manager
pub struct BaselineManager {
    baselines: HashMap<String, Baseline>,
    baseline_dir: PathBuf,
}

impl BaselineManager {
    /// Load baselines from file
    pub fn load(baseline_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let baseline_file = baseline_dir.join("baselines.json");
        let mut baselines = HashMap::new();

        if baseline_file.exists() {
            let content = std::fs::read_to_string(&baseline_file)?;
            let parsed: HashMap<String, Baseline> = serde_json::from_str(&content)?;
            baselines = parsed;
        }

        Ok(Self {
            baselines,
            baseline_dir: baseline_dir.to_path_buf(),
        })
    }

    /// Save baselines to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all(&self.baseline_dir)?;

        let baseline_file = self.baseline_dir.join("baselines.json");
        let content = serde_json::to_string_pretty(&self.baselines)?;
        std::fs::write(&baseline_file, content)?;

        Ok(())
    }

    /// Get baseline for a benchmark
    pub fn get(&self, name: &str) -> Option<&Baseline> {
        self.baselines.get(name)
    }

    /// Update or add a baseline
    pub fn set(&mut self, baseline: Baseline) {
        self.baselines.insert(baseline.name.clone(), baseline);
    }

    /// Compare current result with baseline
    pub fn compare(&self, name: &str, current_value: f64) -> ComparisonResult {
        match self.get(name) {
            Some(baseline) => {
                let diff_percent = ((current_value - baseline.value) / baseline.value) * 100.0;
                let is_improvement = match name {
                    n if n.contains("time") || n.contains("latency") => diff_percent < 0.0,
                    n if n.contains("throughput") || n.contains("ips") => diff_percent > 0.0,
                    _ => false,
                };

                ComparisonResult {
                    baseline_value: baseline.value,
                    current_value,
                    diff_percent,
                    is_improvement,
                }
            }
            None => ComparisonResult {
                baseline_value: current_value,
                current_value,
                diff_percent: 0.0,
                is_improvement: false,
            },
        }
    }
}

/// Comparison result between current and baseline
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub baseline_value: f64,
    pub current_value: f64,
    pub diff_percent: f64,
    pub is_improvement: bool,
}

impl ComparisonResult {
    /// Format comparison result
    pub fn format(&self) -> String {
        let direction = if self.diff_percent > 0.0 { "+" } else { "" };
        let status = if self.is_improvement {
            "✓ Improvement"
        } else if self.diff_percent.abs() > 10.0 {
            "⚠ Regression"
        } else {
            "~ Stable"
        };

        format!(
            "{}: {} → {} ({}{}%, {})",
            status,
            Self::format_value(self.baseline_value),
            Self::format_value(self.current_value),
            direction,
            self.diff_percent.abs(),
            if self.is_improvement { "better" } else { "worse" }
        )
    }

    fn format_value(val: f64) -> String {
        if val >= 1_000_000.0 {
            format!("{:.2}M", val / 1_000_000.0)
        } else if val >= 1_000.0 {
            format!("{:.2}K", val / 1_000.0)
        } else {
            format!("{:.2}", val)
        }
    }
}

/// Configure Criterion with our settings
pub fn configure_criterion(config: &BenchmarkConfig) -> Criterion {
    Criterion::default()
        .measurement_time(config.measurement_time)
        .warm_up_time(config.warmup_time)
        .sample_size(config.sample_size)
        .noise_threshold(0.02) // 2% noise threshold
        .confidence_level(0.95)
}

/// Create a benchmark group with common settings
pub fn create_benchmark_group<'a>(
    c: &'a mut Criterion,
    name: &str,
) -> criterion::BenchmarkGroup<'a, criterion::measurement::WallTime> {
    let mut group = c.benchmark_group(name);
    group
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5))
        .sample_size(100);
    group
}

/// Run a benchmark with baseline comparison
pub fn run_benchmark_with_baseline<F>(
    name: &str,
    baseline_manager: &mut BaselineManager,
    bench_fn: F,
) -> Result<BenchmarkMetrics, Box<dyn std::error::Error>>
where
    F: Fn() -> Duration,
{
    let mut durations = Vec::new();

    // Warmup
    for _ in 0..3 {
        let _ = bench_fn();
    }

    // Measurement
    for _ in 0..10 {
        let start = Instant::now();
        let duration = bench_fn();
        durations.push(duration);
    }

    // Calculate statistics
    let mut sorted_durations = durations.clone();
    sorted_durations.sort();

    let avg_ns = durations
        .iter()
        .map(|d| d.as_nanos() as f64)
        .sum::<f64>()
        / durations.len() as f64;

    let variance = durations
        .iter()
        .map(|d| {
            let diff = d.as_nanos() as f64 - avg_ns;
            diff * diff
        })
        .sum::<f64>()
        / durations.len() as f64;

    let stddev_ns = variance.sqrt();

    let metrics = BenchmarkMetrics {
        name: name.to_string(),
        avg_ns,
        stddev_ns,
        median_ns: sorted_durations[sorted_durations.len() / 2].as_nanos() as f64,
        min_ns: sorted_durations.first().unwrap().as_nanos() as f64,
        max_ns: sorted_durations.last().unwrap().as_nanos() as f64,
        throughput: None,
        custom_metrics: HashMap::new(),
    };

    // Update baseline
    if baseline_manager.baseline_dir.exists() {
        let baseline = Baseline::new(name.to_string(), avg_ns);
        baseline_manager.set(baseline);
        baseline_manager.save()?;
    }

    Ok(metrics)
}

/// Print benchmark summary
pub fn print_summary(metrics: &[BenchmarkMetrics], baseline_manager: &BaselineManager) {
    println!("\n=== Benchmark Summary ===\n");

    for metric in metrics {
        println!("{}", metric.name);
        println!("  {}", metric.format());

        if let Some(baseline) = baseline_manager.get(&metric.name) {
            let comparison = baseline_manager.compare(&metric.name, metric.avg_ns);
            println!("  vs baseline: {}", comparison.format());
        }

        println!();
    }
}

/// Export results to various formats
pub fn export_results(
    metrics: &[BenchmarkMetrics],
    format: OutputFormat,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(metrics)?;
            std::fs::write(output_path, json)?;
        }
        OutputFormat::Markdown => {
            let mut markdown = String::from("# Benchmark Results\n\n");
            markdown.push_str("| Benchmark | Avg (μs) | StdDev (μs) | Median (μs) | CV (%) |\n");
            markdown.push_str("|-----------|----------|-------------|-------------|--------|\n");

            for metric in metrics {
                markdown.push_str(&format!(
                    "| {} | {:.2} | {:.2} | {:.2} | {:.2} |\n",
                    metric.name,
                    metric.avg_ns / 1000.0,
                    metric.stddev_ns / 1000.0,
                    metric.median_ns / 1000.0,
                    metric.coefficient_of_variation()
                ));
            }

            std::fs::write(output_path, markdown)?;
        }
        OutputFormat::Csv => {
            let mut csv = String::from("name,avg_us,stddev_us,median_us,cv_percent\n");
            for metric in metrics {
                csv.push_str(&format!(
                    "{},{:.2},{:.2},{:.2},{:.2}\n",
                    metric.name,
                    metric.avg_ns / 1000.0,
                    metric.stddev_ns / 1000.0,
                    metric.median_ns / 1000.0,
                    metric.coefficient_of_variation()
                ));
            }
            std::fs::write(output_path, csv)?;
        }
        OutputFormat::Console => {
            // Console output is handled by criterion directly
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_creation() {
        let baseline = Baseline::new("test_bench".to_string(), 1000.0);
        assert_eq!(baseline.name, "test_bench");
        assert_eq!(baseline.value, 1000.0);
    }

    #[test]
    fn test_metrics_calculation() {
        let metrics = BenchmarkMetrics {
            name: "test".to_string(),
            avg_ns: 1_000_000.0,
            stddev_ns: 100_000.0,
            median_ns: 950_000.0,
            min_ns: 800_000.0,
            max_ns: 1_200_000.0,
            throughput: None,
            custom_metrics: HashMap::new(),
        };

        assert_eq!(metrics.coefficient_of_variation(), 10.0);
    }

    #[test]
    fn test_comparison_formatting() {
        let result = ComparisonResult {
            baseline_value: 1000.0,
            current_value: 900.0,
            diff_percent: -10.0,
            is_improvement: true,
        };

        let formatted = result.format();
        assert!(formatted.contains("Improvement"));
        assert!(formatted.contains("10%"));
    }
}
