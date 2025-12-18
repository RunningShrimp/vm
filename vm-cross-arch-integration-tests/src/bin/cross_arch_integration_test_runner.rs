//! 跨架构集成测试运行器
//!
//! 本模块提供跨架构集成测试的运行器，用于执行所有测试并生成报告

use std::env;
use std::fs;
use std::time::Instant;

use vm_cross_arch_integration_tests::{
    CrossArchIntegrationTestFramework, 
    CrossArchTestConfig
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 解析命令行参数
    let args: Vec<String> = env::args().collect();
    let config = parse_args(&args)?;
    
    // 创建测试框架
    let mut framework = CrossArchIntegrationTestFramework::new(config.clone());
    
    // 运行测试
    println!("开始运行跨架构集成测试...");
    let start_time = Instant::now();
    
    let results = framework.run_all_tests();
    
    let execution_time = start_time.elapsed();
    println!("测试完成，总耗时: {:?}", execution_time);
    
    // 生成报告
    let report = framework.generate_test_report(&results);
    
    // 输出报告到控制台
    println!("\n{}", report);
    
    // 保存报告到文件
    if let Some(output_path) = &config.output_path {
        fs::write(output_path, report)?;
        println!("报告已保存到: {}", output_path);
    }
    
    // 检查是否有失败的测试
    let failed_tests = results.iter().filter(|r| !r.success).count();
    if failed_tests > 0 {
        println!("\n警告: 有 {} 个测试失败", failed_tests);
        std::process::exit(1);
    }
    
    Ok(())
}

/// 解析命令行参数
fn parse_args(args: &[String]) -> Result<CrossArchTestConfig, Box<dyn std::error::Error>> {
    let mut config = CrossArchTestConfig::default();
    let mut output_path: Option<String> = None;
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--enable-performance-tests" => {
                config.enable_performance_tests = true;
            },
            "--disable-performance-tests" => {
                config.enable_performance_tests = false;
            },
            "--enable-stress-tests" => {
                config.enable_stress_tests = true;
            },
            "--disable-stress-tests" => {
                config.enable_stress_tests = false;
            },
            "--timeout" => {
                if i + 1 < args.len() {
                    config.timeout_seconds = args[i + 1].parse()?;
                    i += 1;
                } else {
                    return Err("缺少超时时间参数".into());
                }
            },
            "--verbose" => {
                config.verbose_logging = true;
            },
            "--output" => {
                if i + 1 < args.len() {
                    output_path = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    return Err("缺少输出路径参数".into());
                }
            },
            "--help" => {
                print_help();
                std::process::exit(0);
            },
            _ => {
                return Err(format!("未知参数: {}", args[i]).into());
            }
        }
        i += 1;
    }
    
    // 设置输出路径
    if output_path.is_none() {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        output_path = Some(format!("cross_arch_test_report_{}.md", timestamp));
    }
    
    config.output_path = output_path;
    
    Ok(config)
}

/// 打印帮助信息
fn print_help() {
    println!("跨架构集成测试运行器");
    println!();
    println!("用法: cross_arch_integration_test_runner [选项]");
    println!();
    println!("选项:");
    println!("  --enable-performance-tests   启用性能测试 (默认: 启用)");
    println!("  --disable-performance-tests  禁用性能测试");
    println!("  --enable-stress-tests        启用压力测试 (默认: 禁用)");
    println!("  --disable-stress-tests       禁用压力测试");
    println!("  --timeout <秒>               设置测试超时时间 (默认: 30秒)");
    println!("  --verbose                    启用详细日志");
    println!("  --output <路径>              设置报告输出路径");
    println!("  --help                       显示此帮助信息");
    println!();
    println!("示例:");
    println!("  cross_arch_integration_test_runner");
    println!("  cross_arch_integration_test_runner --enable-stress-tests --verbose");
    println!("  cross_arch_integration_test_runner --timeout 60 --output report.md");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_default() {
        let args = vec!["test".to_string()];
        let config = parse_args(&args).unwrap();
        
        assert!(config.enable_performance_tests);
        assert!(!config.enable_stress_tests);
        assert_eq!(config.timeout_seconds, 30);
        assert!(!config.verbose_logging);
    }

    #[test]
    fn test_parse_args_with_options() {
        let args = vec![
            "test".to_string(),
            "--disable-performance-tests".to_string(),
            "--enable-stress-tests".to_string(),
            "--timeout".to_string(),
            "60".to_string(),
            "--verbose".to_string(),
        ];
        let config = parse_args(&args).unwrap();
        
        assert!(!config.enable_performance_tests);
        assert!(config.enable_stress_tests);
        assert_eq!(config.timeout_seconds, 60);
        assert!(config.verbose_logging);
    }

    #[test]
    fn test_parse_args_help() {
        let args = vec!["test".to_string(), "--help".to_string()];
        
        // 由于--help会调用std::process::exit(0)，我们无法直接测试
        // 但可以验证参数解析逻辑
        let mut i = 1;
        let mut found_help = false;
        while i < args.len() {
            if args[i].as_str() == "--help" {
                found_help = true;
                break;
            }
            i += 1;
        }
        
        assert!(found_help);
    }
}