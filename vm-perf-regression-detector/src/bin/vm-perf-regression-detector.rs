use clap::{Arg, Command};
use std::path::PathBuf;
use vm_perf_regression_detector::{config::RegressionDetectorConfig, collector::PerformanceCollector, detector::RegressionDetector, reporter::RegressionReporter, storage::{SqlitePerformanceStorage, PerformanceStorage}};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = Command::new("vm-perf-regression-detector")
        .version("1.0.0")
        .about("Performance regression detection for VM cross-architecture translation")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .required(false),
        )
        .arg(
            Arg::new("database")
                .short('d')
                .long("database")
                .value_name("FILE")
                .help("Database file path")
                .required(false),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output report file path")
                .required(false),
        )
        .arg(
            Arg::new("format")
                .short('f')
                .long("format")
                .value_name("FORMAT")
                .help("Report format (text, json, html, markdown)")
                .required(false),
        )
        .arg(
            Arg::new("test-name")
                .short('t')
                .long("test-name")
                .value_name("NAME")
                .help("Test name for this run")
                .required(false),
        )
        .arg(
            Arg::new("source-arch")
                .short('s')
                .long("source-arch")
                .value_name("ARCH")
                .help("Source architecture")
                .required(false),
        )
        .arg(
            Arg::new("target-arch")
                .short('r')
                .long("target-arch")
                .value_name("ARCH")
                .help("Target architecture")
                .required(false),
        )
        .arg(
            Arg::new("iterations")
                .short('i')
                .long("iterations")
                .value_name("NUM")
                .help("Number of test iterations")
                .required(false),
        )
        .arg(
            Arg::new("collect-only")
                .long("collect-only")
                .help("Only collect metrics, don't detect regressions")
                .required(false),
        )
        .arg(
            Arg::new("detect-only")
                .long("detect-only")
                .help("Only detect regressions, don't collect new metrics")
                .required(false),
        )
        .arg(
            Arg::new("generate-charts")
                .long("generate-charts")
                .help("Generate charts in HTML reports")
                .required(false),
        )
        .get_matches();

    // Load configuration
    let config_path = matches.get_one::<String>("config").map(PathBuf::from);
    let config = if let Some(path) = config_path {
        RegressionDetectorConfig::from_file(&path)?
    } else {
        RegressionDetectorConfig::default()
    };

    // Override config with command line arguments
    let database_path = matches
        .get_one::<String>("database")
        .cloned()
        .unwrap_or_else(|| config.database_path.clone());
    
    let output_path = matches
        .get_one::<String>("output")
        .cloned()
        .unwrap_or_else(|| "performance_report.txt".to_string());
    
    let report_format = matches
        .get_one::<String>("format")
        .cloned()
        .unwrap_or_else(|| "text".to_string());
    
    let test_name = matches
        .get_one::<String>("test-name")
        .cloned()
        .unwrap_or_else(|| "default_test".to_string());
    
    let source_arch = matches
        .get_one::<String>("source-arch")
        .cloned()
        .unwrap_or_else(|| "x86_64".to_string());
    
    let target_arch = matches
        .get_one::<String>("target-arch")
        .cloned()
        .unwrap_or_else(|| "arm64".to_string());
    
    let iterations = matches
        .get_one::<String>("iterations")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    
    let collect_only = matches.get_flag("collect-only");
    let detect_only = matches.get_flag("detect-only");
    let generate_charts = matches.get_flag("generate-charts");

    // Initialize components
    let storage = SqlitePerformanceStorage::new(&database_path)?;
    
    // Parse arch strings to GuestArch enum
    let src_arch_enum = match source_arch.as_str() {
        "x86_64" => vm_core::GuestArch::X86_64,
        "arm64" => vm_core::GuestArch::Arm64,
        "riscv64" => vm_core::GuestArch::Riscv64,
        _ => panic!("Unsupported source architecture: {}", source_arch),
    };
    
    let dst_arch_enum = match target_arch.as_str() {
        "x86_64" => vm_core::GuestArch::X86_64,
        "arm64" => vm_core::GuestArch::Arm64,
        "riscv64" => vm_core::GuestArch::Riscv64,
        _ => panic!("Unsupported target architecture: {}", target_arch),
    };
    
    // Create test context
    let test_context = vm_perf_regression_detector::collector::TestContext {
        src_arch: src_arch_enum,
        dst_arch: dst_arch_enum,
        test_name: test_name.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        environment: vm_perf_regression_detector::collector::EnvironmentInfo {
            cpu_cores: num_cpus::get(),
            memory_mb: 128 * 1024,  // 简化处理，直接使用默认值
            os: std::env::consts::OS.to_string(),
            rustc_version: "1.70.0".to_string(),  // 简化处理，直接使用默认值
            opt_level: "3".to_string(),
        },
    };
    
    // Configure report settings
    let mut report_config = config.report_config.clone();
    report_config.generate_charts = generate_charts;
    
    // Parse and set report format
    let parsed_format = match report_format.as_str() {
        "text" => vm_perf_regression_detector::config::ReportFormat::Text,
        "json" => vm_perf_regression_detector::config::ReportFormat::Json,
        "html" => vm_perf_regression_detector::config::ReportFormat::Html,
        "markdown" => vm_perf_regression_detector::config::ReportFormat::Markdown,
        _ => panic!("Unsupported report format: {}", report_format),
    };
    report_config.format = parsed_format;
    
    let mut collector = PerformanceCollector::new(test_context);
    let detector = RegressionDetector::new(config.clone());
    let reporter = RegressionReporter::new(report_config);

    println!("VM Performance Regression Detector");
    println!("================================");
    println!("Test: {}", test_name);
    println!("Source Architecture: {}", source_arch);
    println!("Target Architecture: {}", target_arch);
    println!("Iterations: {}", iterations);
    println!("Database: {}", database_path);
    println!("Output: {}", output_path);
    println!("Format: {}", report_format);
    println!();

    if collect_only {
        println!("Collecting performance metrics...");
        
        // Simulate running performance tests
        for i in 1..=iterations {
            println!("Running iteration {} of {}", i, iterations);
            
            // Start collection
            collector.start_collection();
            
            // Simulate some work
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            // Collect metrics
            let metrics = collector.collect_metrics()?;
            
            // Store metrics
            storage.store_metrics(&metrics)?;
            
            println!("  Execution time: {:.2}ms", metrics.execution_time_us as f64 / 1000.0);
            println!("  JIT compilation time: {:.2}ms", metrics.jit_compilation_time_us as f64 / 1000.0);
            println!("  Memory usage: {} bytes", metrics.memory_usage_bytes);
            println!("  Instructions per second: {:.2}", metrics.instruction_throughput);
        }
        
        println!("Metrics collection completed.");
    } else if detect_only {
        println!("Detecting performance regressions...");
        
        // Get current metrics (latest)
        let context = collector.get_context();
        let current_metrics = storage.get_latest_metrics(&context)?
            .ok_or_else(|| anyhow::anyhow!("No current metrics found"))?;
        
        // Get historical metrics
        let historical_metrics = storage.get_history(&context, Some(20))?;
        
        if historical_metrics.len() < 3 {
            println!("Warning: Not enough historical data for reliable regression detection");
        }
        
        // Detect regressions
        let results = detector.detect_regressions(&current_metrics, &historical_metrics)?;
        
        // Generate report
        let report = reporter.generate_report(&results)?;
        
        // Save report
        std::fs::write(&output_path, report)?;
        
        println!("Regression detection completed.");
        println!("Report saved to: {}", output_path);
        
        // Print summary
        let critical_count = results.iter().filter(|r| r.severity == vm_perf_regression_detector::detector::RegressionSeverity::Critical).count();
        let high_count = results.iter().filter(|r| r.severity == vm_perf_regression_detector::detector::RegressionSeverity::Major).count();
        let medium_count = results.iter().filter(|r| r.severity == vm_perf_regression_detector::detector::RegressionSeverity::Moderate).count();
        let low_count = results.iter().filter(|r| r.severity == vm_perf_regression_detector::detector::RegressionSeverity::Minor).count();
        
        println!("Regression Summary:");
        println!("  Critical: {}", critical_count);
        println!("  High: {}", high_count);
        println!("  Medium: {}", medium_count);
        println!("  Low: {}", low_count);
    } else {
        println!("Collecting metrics and detecting regressions...");
        
        // Collect new metrics
        for i in 1..=iterations {
            println!("Running iteration {} of {}", i, iterations);
            
            // Start collection
            collector.start_collection();
            
            // Simulate some work
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            // Collect metrics
            let metrics = collector.collect_metrics()?;
            
            // Store metrics
            storage.store_metrics(&metrics)?;
            
            println!("  Execution time: {:.2}ms", metrics.execution_time_us as f64 / 1000.0);
            println!("  JIT compilation time: {:.2}ms", metrics.jit_compilation_time_us as f64 / 1000.0);
            println!("  Memory usage: {} bytes", metrics.memory_usage_bytes);
            println!("  Instructions per second: {:.2}", metrics.instruction_throughput);
        }
        
        // Get current metrics (latest)
        let context = collector.get_context();
        let current_metrics = storage.get_latest_metrics(&context)?
            .ok_or_else(|| anyhow::anyhow!("No current metrics found"))?;
        
        // Get historical metrics
        let historical_metrics = storage.get_history(&context, Some(20))?;
        
        if historical_metrics.len() < 3 {
            println!("Warning: Not enough historical data for reliable regression detection");
        }
        
        // Detect regressions
        let results = detector.detect_regressions(&current_metrics, &historical_metrics)?;
        
        // Generate report
        let report = reporter.generate_report(&results)?;
        
        // Save report
        std::fs::write(&output_path, report)?;
        
        println!("Metrics collection and regression detection completed.");
        println!("Report saved to: {}", output_path);
        
        // Print summary
        let critical_count = results.iter().filter(|r| r.severity == vm_perf_regression_detector::detector::RegressionSeverity::Critical).count();
        let high_count = results.iter().filter(|r| r.severity == vm_perf_regression_detector::detector::RegressionSeverity::Major).count();
        let medium_count = results.iter().filter(|r| r.severity == vm_perf_regression_detector::detector::RegressionSeverity::Moderate).count();
        let low_count = results.iter().filter(|r| r.severity == vm_perf_regression_detector::detector::RegressionSeverity::Minor).count();
        
        println!("Regression Summary:");
        println!("  Critical: {}", critical_count);
        println!("  High: {}", high_count);
        println!("  Medium: {}", medium_count);
        println!("  Low: {}", low_count);
    }

    Ok(())
}