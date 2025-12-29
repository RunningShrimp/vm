//! JIT性能分析工具
//!
//! 本模块提供详细的JIT性能分析功能，包括性能数据收集、
//! 分析、报告生成等功能。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::fs;
use serde::{Serialize, Deserialize};
use crate::core::{JITEngine, JITConfig};
use vm_ir::IRBlock;

/// 性能分析器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerConfig {
    /// 采样间隔（毫秒）
    pub sampling_interval_ms: u64,
    /// 最大样本数量
    pub max_samples: usize,
    /// 是否启用详细分析
    pub enable_detailed_analysis: bool,
    /// 是否启用内存分析
    pub enable_memory_analysis: bool,
    /// 是否启用热点分析
    pub enable_hotspot_analysis: bool,
    /// 输出目录
    pub output_directory: String,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            sampling_interval_ms: 100,
            max_samples: 10000,
            enable_detailed_analysis: true,
            enable_memory_analysis: true,
            enable_hotspot_analysis: true,
            output_directory: "./jit_performance_analysis".to_string(),
        }
    }
}

/// 性能数据点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceDataPoint {
    /// 时间戳
    pub timestamp: Duration,
    /// 编译时间（微秒）
    pub compilation_time_us: u64,
    /// 优化时间（微秒）
    pub optimization_time_us: u64,
    /// 代码生成时间（微秒）
    pub codegen_time_us: u64,
    /// 内存使用量（字节）
    pub memory_usage_bytes: u64,
    /// 代码缓存大小（字节）
    pub cache_size_bytes: u64,
    /// 缓存命中率
    pub cache_hit_rate: f64,
    /// 活跃编译任务数
    pub active_compilations: u32,
    /// 热点代码块数量
    pub hotspot_blocks: u32,
}

/// 性能统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    /// 平均编译时间（微秒）
    pub avg_compilation_time_us: f64,
    /// 最小编译时间（微秒）
    pub min_compilation_time_us: u64,
    /// 最大编译时间（微秒）
    pub max_compilation_time_us: u64,
    /// 编译时间标准差
    pub compilation_time_stddev: f64,
    /// 平均内存使用量（字节）
    pub avg_memory_usage_bytes: f64,
    /// 峰值内存使用量（字节）
    pub peak_memory_usage_bytes: u64,
    /// 平均缓存命中率
    pub avg_cache_hit_rate: f64,
    /// 总编译次数
    pub total_compilations: u64,
    /// 总优化时间（微秒）
    pub total_optimization_time_us: u64,
    /// 总代码生成时间（微秒）
    pub total_codegen_time_us: u64,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            avg_compilation_time_us: 0.0,
            min_compilation_time_us: 0,
            max_compilation_time_us: 0,
            compilation_time_stddev: 0.0,
            avg_memory_usage_bytes: 0.0,
            peak_memory_usage_bytes: 0,
            avg_cache_hit_rate: 0.0,
            total_compilations: 0,
            total_optimization_time_us: 0,
            total_codegen_time_us: 0,
        }
    }
}

/// 热点分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotAnalysis {
    /// 热点代码块地址
    pub hotspot_addresses: Vec<u64>,
    /// 热点执行频率
    pub execution_frequencies: HashMap<u64, u64>,
    /// 热点编译时间
    pub hotspot_compilation_times: HashMap<u64, u64>,
    /// 热点优化收益
    pub hotspot_optimization_benefits: HashMap<u64, f64>,
}

impl Default for HotspotAnalysis {
    fn default() -> Self {
        Self {
            hotspot_addresses: Vec::new(),
            execution_frequencies: HashMap::new(),
            hotspot_compilation_times: HashMap::new(),
            hotspot_optimization_benefits: HashMap::new(),
        }
    }
}

/// 性能分析器
pub struct JITPerformanceProfiler {
    /// JIT引擎
    jit_engine: Arc<Mutex<JITEngine>>,
    /// 分析器配置
    config: ProfilerConfig,
    /// 性能数据点
    performance_data: Arc<Mutex<VecDeque<PerformanceDataPoint>>>,
    /// 开始时间
    start_time: Instant,
    /// 是否正在分析
    is_profiling: Arc<Mutex<bool>>,
}

impl JITPerformanceProfiler {
    /// 创建新的性能分析器
    pub fn new(jit_engine: Arc<Mutex<JITEngine>>, config: ProfilerConfig) -> Self {
        // 确保输出目录存在
        if let Err(e) = fs::create_dir_all(&config.output_directory) {
            eprintln!("警告：无法创建输出目录 {}: {}", config.output_directory, e);
        }

        Self {
            jit_engine,
            config,
            performance_data: Arc::new(Mutex::new(VecDeque::new())),
            start_time: Instant::now(),
            is_profiling: Arc::new(Mutex::new(false)),
        }
    }

    /// 辅助方法：获取is_profiling锁
    fn lock_is_profiling(&self) -> Result<std::sync::MutexGuard<'_, bool>, String> {
        self.is_profiling
            .lock()
            .map_err(|e| format!("获取is_profiling锁失败: {}", e))
    }

    /// 辅助方法：获取performance_data锁
    fn lock_performance_data(&self) -> Result<std::sync::MutexGuard<'_, VecDeque<PerformanceDataPoint>>, String> {
        self.performance_data
            .lock()
            .map_err(|e| format!("获取performance_data锁失败: {}", e))
    }

    /// 开始性能分析
    pub fn start_profiling(&self) -> Result<(), String> {
        let mut is_profiling = self.lock_is_profiling()?;
        if *is_profiling {
            return Err("性能分析已在运行中".to_string());
        }
        *is_profiling = true;
        drop(is_profiling);

        // 清空之前的数据
        {
            let mut data = self.lock_performance_data()?;
            data.clear();
        }

        // 启动采样线程
        let performance_data = self.performance_data.clone();
        let config = self.config.clone();
        let jit_engine = self.jit_engine.clone();
        let is_profiling = self.is_profiling.clone();

        std::thread::spawn(move || {
            loop {
                // 检查是否继续分析
                let should_continue = match is_profiling.lock() {
                    Ok(guard) => *guard,
                    Err(_) => break, // 锁获取失败，退出线程
                };

                if !should_continue {
                    break;
                }

                let data_point = Self::collect_performance_data(&jit_engine);

                {
                    match performance_data.lock() {
                        Ok(mut data) => {
                            data.push_back(data_point);

                            // 限制数据点数量
                            while data.len() > config.max_samples {
                                data.pop_front();
                            }
                        }
                        Err(e) => {
                            eprintln!("警告：获取performance_data锁失败: {}", e);
                        }
                    }
                }

                std::thread::sleep(Duration::from_millis(config.sampling_interval_ms));
            }
        });

        Ok(())
    }

    /// 停止性能分析
    pub fn stop_profiling(&self) -> Result<(), String> {
        let mut is_profiling = self.lock_is_profiling()?;
        if !*is_profiling {
            return Err("性能分析未在运行".to_string());
        }
        *is_profiling = false;
        Ok(())
    }

    /// 收集性能数据
    fn collect_performance_data(jit_engine: &Arc<Mutex<JITEngine>>) -> PerformanceDataPoint {
        let timestamp = Instant::now().duration_since(Instant::now() - Duration::from_secs(0));
        
        // 这里应该从实际的JIT引擎获取性能数据
        // 暂时使用模拟数据
        PerformanceDataPoint {
            timestamp,
            compilation_time_us: 100 + (rand::random::<u64>() % 500),
            optimization_time_us: 50 + (rand::random::<u64>() % 200),
            codegen_time_us: 80 + (rand::random::<u64>() % 300),
            memory_usage_bytes: 1024 * 1024 + (rand::random::<u64>() % 10 * 1024 * 1024),
            cache_size_bytes: 512 * 1024 + (rand::random::<u64>() % 5 * 1024 * 1024),
            cache_hit_rate: 0.7 + (rand::random::<f64>() * 0.3),
            active_compilations: rand::random::<u32>() % 5,
            hotspot_blocks: rand::random::<u32>() % 20,
        }
    }

    /// 获取性能统计信息
    pub fn get_performance_stats(&self) -> PerformanceStats {
        let data = match self.lock_performance_data() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("警告：获取性能数据失败: {}", e);
                return PerformanceStats::default();
            }
        };

        if data.is_empty() {
            return PerformanceStats::default();
        }

        let compilation_times: Vec<u64> = data.iter().map(|d| d.compilation_time_us).collect();
        let memory_usages: Vec<u64> = data.iter().map(|d| d.memory_usage_bytes).collect();
        let cache_hit_rates: Vec<f64> = data.iter().map(|d| d.cache_hit_rate).collect();

        let avg_compilation_time = compilation_times.iter().sum::<u64>() as f64 / compilation_times.len() as f64;
        let min_compilation_time = compilation_times.iter().copied().min().unwrap_or(0);
        let max_compilation_time = compilation_times.iter().copied().max().unwrap_or(0);

        // 计算标准差
        let variance = compilation_times.iter()
            .map(|&time| (time as f64 - avg_compilation_time).powi(2))
            .sum::<f64>() / compilation_times.len() as f64;
        let compilation_time_stddev = variance.sqrt();

        let avg_memory_usage = memory_usages.iter().sum::<u64>() as f64 / memory_usages.len() as f64;
        let peak_memory_usage = memory_usages.iter().copied().max().unwrap_or(0);
        let avg_cache_hit_rate = cache_hit_rates.iter().sum::<f64>() / cache_hit_rates.len() as f64;

        PerformanceStats {
            avg_compilation_time_us: avg_compilation_time,
            min_compilation_time_us: min_compilation_time,
            max_compilation_time_us: max_compilation_time,
            compilation_time_stddev,
            avg_memory_usage_bytes: avg_memory_usage,
            peak_memory_usage_bytes: peak_memory_usage,
            avg_cache_hit_rate,
            total_compilations: data.len() as u64,
            total_optimization_time_us: data.iter().map(|d| d.optimization_time_us).sum(),
            total_codegen_time_us: data.iter().map(|d| d.codegen_time_us).sum(),
        }
    }

    /// 执行热点分析
    pub fn analyze_hotspots(&self) -> HotspotAnalysis {
        let _data = match self.lock_performance_data() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("警告：获取性能数据失败: {}", e);
                return HotspotAnalysis::default();
            }
        };

        // 模拟热点分析
        let mut hotspot_addresses = Vec::new();
        let mut execution_frequencies = HashMap::new();
        let mut hotspot_compilation_times = HashMap::new();
        let mut hotspot_optimization_benefits = HashMap::new();

        // 生成模拟热点数据
        for i in 0..10 {
            let addr = 0x1000 + i as u64 * 0x1000;
            hotspot_addresses.push(addr);
            execution_frequencies.insert(addr, 1000 + i * 100);
            hotspot_compilation_times.insert(addr, 500 + i * 50);
            hotspot_optimization_benefits.insert(addr, 1.5 + i as f64 * 0.1);
        }

        HotspotAnalysis {
            hotspot_addresses,
            execution_frequencies,
            hotspot_compilation_times,
            hotspot_optimization_benefits,
        }
    }

    /// 生成性能报告
    pub fn generate_performance_report(&self) -> String {
        let stats = self.get_performance_stats();
        let hotspot_analysis = self.analyze_hotspots();
        
        let mut report = String::new();
        report.push_str("# JIT性能分析报告\n\n");
        
        // 基本信息
        report.push_str("## 基本信息\n");
        report.push_str(&format!("- 分析时长: {:?}\n", self.start_time.elapsed()));
        report.push_str(&format!("- 采样间隔: {} ms\n", self.config.sampling_interval_ms));
        report.push_str(&format!("- 数据点数量: {}\n\n", stats.total_compilations));
        
        // 编译性能统计
        report.push_str("## 编译性能统计\n");
        report.push_str(&format!("- 平均编译时间: {:.2} μs\n", stats.avg_compilation_time_us));
        report.push_str(&format!("- 最小编译时间: {} μs\n", stats.min_compilation_time_us));
        report.push_str(&format!("- 最大编译时间: {} μs\n", stats.max_compilation_time_us));
        report.push_str(&format!("- 编译时间标准差: {:.2} μs\n", stats.compilation_time_stddev));
        report.push_str(&format!("- 总优化时间: {} μs\n", stats.total_optimization_time_us));
        report.push_str(&format!("- 总代码生成时间: {} μs\n\n", stats.total_codegen_time_us));
        
        // 内存使用统计
        report.push_str("## 内存使用统计\n");
        report.push_str(&format!("- 平均内存使用: {:.2} MB\n", stats.avg_memory_usage_bytes / 1024.0 / 1024.0));
        report.push_str(&format!("- 峰值内存使用: {:.2} MB\n", stats.peak_memory_usage_bytes as f64 / 1024.0 / 1024.0));
        report.push_str(&format!("- 平均缓存命中率: {:.2}%\n\n", stats.avg_cache_hit_rate * 100.0));
        
        // 热点分析
        report.push_str("## 热点分析\n");
        report.push_str(&format!("- 热点代码块数量: {}\n", hotspot_analysis.hotspot_addresses.len()));

        if !hotspot_analysis.hotspot_addresses.is_empty() {
            report.push_str("\n### 热点代码块详情\n");
            for &addr in &hotspot_analysis.hotspot_addresses {
                let freq = hotspot_analysis.execution_frequencies.get(&addr).copied().unwrap_or(0);
                let comp_time = hotspot_analysis.hotspot_compilation_times.get(&addr).copied().unwrap_or(0);
                let benefit = hotspot_analysis.hotspot_optimization_benefits.get(&addr).copied().unwrap_or(0.0);

                report.push_str(&format!(
                    "- 地址 0x{:x}: 执行频率 {}, 编译时间 {} μs, 优化收益 {:.2}x\n",
                    addr, freq, comp_time, benefit
                ));
            }
        }
        
        report.push_str("\n## 性能建议\n");
        
        // 基于分析结果生成建议
        if stats.avg_cache_hit_rate < 0.8 {
            report.push_str("- 缓存命中率较低，建议增加缓存大小或优化缓存策略\n");
        }
        
        if stats.compilation_time_stddev > stats.avg_compilation_time_us * 0.5 {
            report.push_str("- 编译时间波动较大，建议检查是否存在性能瓶颈\n");
        }
        
        if stats.peak_memory_usage_bytes > stats.avg_memory_usage_bytes as u64 * 2 {
            report.push_str("- 内存使用波动较大，建议优化内存管理策略\n");
        }
        
        if !hotspot_analysis.hotspot_addresses.is_empty() {
            report.push_str("- 发现热点代码块，建议针对热点进行专门优化\n");
        }
        
        report
    }

    /// 保存性能数据
    pub fn save_performance_data(&self, filename: &str) -> Result<(), String> {
        let data = self.lock_performance_data()?;
        let json = serde_json::to_string_pretty(&*data)
            .map_err(|e| format!("序列化失败: {}", e))?;

        let filepath = format!("{}/{}", self.config.output_directory, filename);
        fs::write(&filepath, json)
            .map_err(|e| format!("写入文件失败: {}", e))?;

        Ok(())
    }

    /// 保存性能报告
    pub fn save_performance_report(&self, filename: &str) -> Result<(), String> {
        let report = self.generate_performance_report();
        let filepath = format!("{}/{}", self.config.output_directory, filename);
        fs::write(&filepath, report)
            .map_err(|e| format!("写入文件失败: {}", e))?;
        
        Ok(())
    }

    /// 加载性能数据
    pub fn load_performance_data(&self, filename: &str) -> Result<(), String> {
        let filepath = format!("{}/{}", self.config.output_directory, filename);
        let json = fs::read_to_string(&filepath)
            .map_err(|e| format!("读取文件失败: {}", e))?;

        let data: VecDeque<PerformanceDataPoint> = serde_json::from_str(&json)
            .map_err(|e| format!("反序列化失败: {}", e))?;

        {
            let mut current_data = self.lock_performance_data()?;
            *current_data = data;
        }

        Ok(())
    }

    /// 导出CSV格式数据
    pub fn export_csv(&self, filename: &str) -> Result<(), String> {
        let data = self.lock_performance_data()?;
        let filepath = format!("{}/{}", self.config.output_directory, filename);

        let mut csv_content = String::new();
        csv_content.push_str("timestamp,compilation_time_us,optimization_time_us,codegen_time_us,memory_usage_bytes,cache_size_bytes,cache_hit_rate,active_compilations,hotspot_blocks\n");

        for data_point in data.iter() {
            csv_content.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                data_point.timestamp.as_millis(),
                data_point.compilation_time_us,
                data_point.optimization_time_us,
                data_point.codegen_time_us,
                data_point.memory_usage_bytes,
                data_point.cache_size_bytes,
                data_point.cache_hit_rate,
                data_point.active_compilations,
                data_point.hotspot_blocks
            ));
        }

        fs::write(&filepath, csv_content)
            .map_err(|e| format!("写入CSV文件失败: {}", e))?;

        Ok(())
    }

    /// 实时性能监控
    pub fn start_realtime_monitoring(&self) -> Result<(), String> {
        let is_profiling = self.lock_is_profiling()?;
        if *is_profiling {
            return Err("性能分析已在运行中".to_string());
        }
        drop(is_profiling);

        let performance_data = self.performance_data.clone();
        let config = self.config.clone();
        let jit_engine = self.jit_engine.clone();

        std::thread::spawn(move || {
            loop {
                let stats = Self::calculate_realtime_stats(&performance_data);

                // 打印实时统计信息
                println!("=== JIT实时性能监控 ===");
                println!("平均编译时间: {:.2} μs", stats.avg_compilation_time_us);
                println!("内存使用: {:.2} MB", stats.avg_memory_usage_bytes / 1024.0 / 1024.0);
                println!("缓存命中率: {:.2}%", stats.avg_cache_hit_rate * 100.0);
                println!("活跃编译任务: {}", stats.total_compilations);
                println!("========================");

                std::thread::sleep(Duration::from_secs(5));
            }
        });

        self.start_profiling()
    }

    /// 计算实时统计信息
    fn calculate_realtime_stats(performance_data: &Arc<Mutex<VecDeque<PerformanceDataPoint>>>) -> PerformanceStats {
        let data = match performance_data.lock() {
            Ok(guard) => guard,
            Err(_) => {
                eprintln!("警告：获取性能数据锁失败");
                return PerformanceStats::default();
            }
        };

        if data.is_empty() {
            return PerformanceStats::default();
        }

        let recent_data: Vec<_> = data.iter().rev().take(100).collect();
        let compilation_times: Vec<u64> = recent_data.iter().map(|d| d.compilation_time_us).collect();
        let memory_usages: Vec<u64> = recent_data.iter().map(|d| d.memory_usage_bytes).collect();
        let cache_hit_rates: Vec<f64> = recent_data.iter().map(|d| d.cache_hit_rate).collect();

        let avg_compilation_time = compilation_times.iter().sum::<u64>() as f64 / compilation_times.len() as f64;
        let min_compilation_time = compilation_times.iter().copied().min().unwrap_or(0);
        let max_compilation_time = compilation_times.iter().copied().max().unwrap_or(0);

        let variance = compilation_times.iter()
            .map(|&time| (time as f64 - avg_compilation_time).powi(2))
            .sum::<f64>() / compilation_times.len() as f64;
        let compilation_time_stddev = variance.sqrt();

        let avg_memory_usage = memory_usages.iter().sum::<u64>() as f64 / memory_usages.len() as f64;
        let peak_memory_usage = memory_usages.iter().copied().max().unwrap_or(0);
        let avg_cache_hit_rate = cache_hit_rates.iter().sum::<f64>() / cache_hit_rates.len() as f64;

        PerformanceStats {
            avg_compilation_time_us: avg_compilation_time,
            min_compilation_time_us: min_compilation_time,
            max_compilation_time_us: max_compilation_time,
            compilation_time_stddev,
            avg_memory_usage_bytes: avg_memory_usage,
            peak_memory_usage_bytes: peak_memory_usage,
            avg_cache_hit_rate,
            total_compilations: data.len() as u64,
            total_optimization_time_us: recent_data.iter().map(|d| d.optimization_time_us).sum(),
            total_codegen_time_us: recent_data.iter().map(|d| d.codegen_time_us).sum(),
        }
    }
}

/// 性能回归检测器
pub struct PerformanceRegressionDetector {
    /// 基准性能数据
    baseline_stats: PerformanceStats,
    /// 回归阈值
    regression_thresholds: RegressionThresholds,
}

/// 回归阈值配置
#[derive(Debug, Clone)]
pub struct RegressionThresholds {
    /// 编译时间回归阈值（百分比）
    pub compilation_time_threshold: f64,
    /// 内存使用回归阈值（百分比）
    pub memory_usage_threshold: f64,
    /// 缓存命中率回归阈值（绝对值）
    pub cache_hit_rate_threshold: f64,
}

impl Default for RegressionThresholds {
    fn default() -> Self {
        Self {
            compilation_time_threshold: 0.1, // 10%
            memory_usage_threshold: 0.2,      // 20%
            cache_hit_rate_threshold: 0.05,   // 5%
        }
    }
}

impl PerformanceRegressionDetector {
    /// 创建新的性能回归检测器
    pub fn new(baseline_stats: PerformanceStats) -> Self {
        Self {
            baseline_stats,
            regression_thresholds: RegressionThresholds::default(),
        }
    }

    /// 检测性能回归
    pub fn detect_regressions(&self, current_stats: &PerformanceStats) -> Vec<String> {
        let mut regressions = Vec::new();

        // 检查编译时间回归
        let compilation_time_increase = (current_stats.avg_compilation_time_us - self.baseline_stats.avg_compilation_time_us) 
            / self.baseline_stats.avg_compilation_time_us;
        if compilation_time_increase > self.regression_thresholds.compilation_time_threshold {
            regressions.push(format!(
                "编译时间回归: {:.2}% (基准: {:.2} μs, 当前: {:.2} μs)",
                compilation_time_increase * 100.0,
                self.baseline_stats.avg_compilation_time_us,
                current_stats.avg_compilation_time_us
            ));
        }

        // 检查内存使用回归
        let memory_usage_increase = (current_stats.avg_memory_usage_bytes - self.baseline_stats.avg_memory_usage_bytes) 
            / self.baseline_stats.avg_memory_usage_bytes;
        if memory_usage_increase > self.regression_thresholds.memory_usage_threshold {
            regressions.push(format!(
                "内存使用回归: {:.2}% (基准: {:.2} MB, 当前: {:.2} MB)",
                memory_usage_increase * 100.0,
                self.baseline_stats.avg_memory_usage_bytes / 1024.0 / 1024.0,
                current_stats.avg_memory_usage_bytes / 1024.0 / 1024.0
            ));
        }

        // 检查缓存命中率回归
        let cache_hit_rate_decrease = self.baseline_stats.avg_cache_hit_rate - current_stats.avg_cache_hit_rate;
        if cache_hit_rate_decrease > self.regression_thresholds.cache_hit_rate_threshold {
            regressions.push(format!(
                "缓存命中率回归: {:.2}% (基准: {:.2}%, 当前: {:.2}%)",
                cache_hit_rate_decrease * 100.0,
                self.baseline_stats.avg_cache_hit_rate * 100.0,
                current_stats.avg_cache_hit_rate * 100.0
            ));
        }

        regressions
    }

    /// 设置回归阈值
    pub fn set_regression_thresholds(&mut self, thresholds: RegressionThresholds) {
        self.regression_thresholds = thresholds;
    }
}