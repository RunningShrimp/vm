//! 性能分析工具模块
//!
//! 提供性能分析API，支持性能报告生成和性能问题识别

use crate::metrics_collector::{MetricsCollector, SystemMetrics};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// 性能分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalysis {
    /// 分析时间戳
    pub timestamp: DateTime<Utc>,
    /// 总体性能评分 (0-100)
    pub overall_score: f64,
    /// 性能瓶颈列表
    pub bottlenecks: Vec<Bottleneck>,
    /// 性能建议
    pub recommendations: Vec<Recommendation>,
    /// 详细指标分析
    pub metric_analysis: HashMap<String, MetricAnalysis>,
    /// 性能趋势
    pub trends: Vec<Trend>,
}

/// 性能瓶颈
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    /// 瓶颈类型
    pub category: BottleneckCategory,
    /// 严重程度
    pub severity: Severity,
    /// 描述
    pub description: String,
    /// 当前值
    pub current_value: f64,
    /// 阈值
    pub threshold: f64,
    /// 影响范围
    pub impact: String,
}

/// 瓶颈类别
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BottleneckCategory {
    Compilation,
    Execution,
    Memory,
    GarbageCollection,
    Cache,
    Tlb,
    Parallelization,
}

/// 严重程度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// 性能建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// 建议类型
    pub category: BottleneckCategory,
    /// 优先级
    pub priority: u8, // 1-10, 10最高
    /// 标题
    pub title: String,
    /// 描述
    pub description: String,
    /// 预期改进
    pub expected_improvement: String,
}

/// 指标分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricAnalysis {
    /// 指标名称
    pub name: String,
    /// 当前值
    pub current_value: f64,
    /// 平均值
    pub average_value: f64,
    /// 最大值
    pub max_value: f64,
    /// 最小值
    pub min_value: f64,
    /// 百分位数
    pub percentiles: Percentiles,
    /// 趋势 (正数表示上升，负数表示下降)
    pub trend: f64,
    /// 是否异常
    pub is_anomaly: bool,
}

/// 百分位数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Percentiles {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
}

/// 性能趋势
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trend {
    /// 指标名称
    pub metric_name: String,
    /// 趋势方向 (1=上升, -1=下降, 0=稳定)
    pub direction: i8,
    /// 变化百分比
    pub change_percent: f64,
    /// 时间范围（秒）
    pub time_range_secs: u64,
}

/// 性能分析器
pub struct PerformanceAnalyzer {
    metrics_collector: Arc<MetricsCollector>,
    historical_data: Arc<RwLock<Vec<SystemMetrics>>>,
}

impl PerformanceAnalyzer {
    /// 创建新的性能分析器
    pub fn new(metrics_collector: Arc<MetricsCollector>) -> Self {
        Self {
            metrics_collector,
            historical_data: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 执行性能分析
    pub async fn analyze(&self) -> PerformanceAnalysis {
        let current_metrics = self.metrics_collector.get_current_metrics().await;
        let historical = self.metrics_collector.get_historical_metrics().await;
        
        // 更新历史数据
        {
            let mut data = self.historical_data.write().await;
            data.clear();
            data.extend_from_slice(&historical);
        }
        
        // 识别瓶颈
        let bottlenecks = self.identify_bottlenecks(&current_metrics).await;
        
        // 生成建议
        let recommendations = self.generate_recommendations(&bottlenecks, &current_metrics).await;
        
        // 分析指标
        let metric_analysis = self.analyze_metrics(&current_metrics, &historical).await;
        
        // 计算趋势
        let trends = self.calculate_trends(&historical).await;
        
        // 计算总体评分
        let overall_score = self.calculate_overall_score(&current_metrics, &bottlenecks).await;
        
        PerformanceAnalysis {
            timestamp: Utc::now(),
            overall_score,
            bottlenecks,
            recommendations,
            metric_analysis,
            trends,
        }
    }

    /// 识别性能瓶颈
    async fn identify_bottlenecks(&self, metrics: &SystemMetrics) -> Vec<Bottleneck> {
        let mut bottlenecks = Vec::new();
        
        // 检查编译时间
        if metrics.jit_metrics.avg_compile_time_ns > 10_000_000.0 { // 10ms
            bottlenecks.push(Bottleneck {
                category: BottleneckCategory::Compilation,
                severity: if metrics.jit_metrics.avg_compile_time_ns > 50_000_000.0 {
                    Severity::Critical
                } else {
                    Severity::High
                },
                description: format!(
                    "平均编译时间过高: {:.2}ms",
                    metrics.jit_metrics.avg_compile_time_ns / 1_000_000.0
                ),
                current_value: metrics.jit_metrics.avg_compile_time_ns,
                threshold: 10_000_000.0,
                impact: "影响首次执行延迟和代码预取效率".to_string(),
            });
        }
        
        // 检查执行时间
        if metrics.jit_metrics.avg_execution_time_ns > 1_000_000.0 { // 1ms
            bottlenecks.push(Bottleneck {
                category: BottleneckCategory::Execution,
                severity: if metrics.jit_metrics.avg_execution_time_ns > 5_000_000.0 {
                    Severity::Critical
                } else {
                    Severity::Medium
                },
                description: format!(
                    "平均执行时间过高: {:.2}μs",
                    metrics.jit_metrics.avg_execution_time_ns / 1_000.0
                ),
                current_value: metrics.jit_metrics.avg_execution_time_ns,
                threshold: 1_000_000.0,
                impact: "影响整体吞吐量和响应时间".to_string(),
            });
        }
        
        // 检查GC时间
        if metrics.gc_metrics.avg_gc_time_ns > 50_000_000.0 { // 50ms
            bottlenecks.push(Bottleneck {
                category: BottleneckCategory::GarbageCollection,
                severity: if metrics.gc_metrics.avg_gc_time_ns > 100_000_000.0 {
                    Severity::Critical
                } else {
                    Severity::High
                },
                description: format!(
                    "平均GC时间过高: {:.2}ms",
                    metrics.gc_metrics.avg_gc_time_ns / 1_000_000.0
                ),
                current_value: metrics.gc_metrics.avg_gc_time_ns,
                threshold: 50_000_000.0,
                impact: "影响系统响应性和吞吐量".to_string(),
            });
        }
        
        // 检查GC暂停时间
        if metrics.gc_metrics.avg_pause_time_ns > 10_000_000.0 { // 10ms
            bottlenecks.push(Bottleneck {
                category: BottleneckCategory::GarbageCollection,
                severity: if metrics.gc_metrics.avg_pause_time_ns > 50_000_000.0 {
                    Severity::Critical
                } else {
                    Severity::High
                },
                description: format!(
                    "平均GC暂停时间过高: {:.2}ms",
                    metrics.gc_metrics.avg_pause_time_ns / 1_000_000.0
                ),
                current_value: metrics.gc_metrics.avg_pause_time_ns,
                threshold: 10_000_000.0,
                impact: "影响实时性和用户体验".to_string(),
            });
        }
        
        // 检查缓存命中率
        if metrics.jit_metrics.cache_hit_rate < 0.8 { // 80%
            bottlenecks.push(Bottleneck {
                category: BottleneckCategory::Cache,
                severity: if metrics.jit_metrics.cache_hit_rate < 0.5 {
                    Severity::High
                } else {
                    Severity::Medium
                },
                description: format!(
                    "缓存命中率过低: {:.2}%",
                    metrics.jit_metrics.cache_hit_rate * 100.0
                ),
                current_value: metrics.jit_metrics.cache_hit_rate,
                threshold: 0.8,
                impact: "增加编译开销，降低性能".to_string(),
            });
        }
        
        // 检查TLB命中率
        if metrics.tlb_metrics.hit_rate < 0.9 { // 90%
            bottlenecks.push(Bottleneck {
                category: BottleneckCategory::Tlb,
                severity: if metrics.tlb_metrics.hit_rate < 0.7 {
                    Severity::High
                } else {
                    Severity::Medium
                },
                description: format!(
                    "TLB命中率过低: {:.2}%",
                    metrics.tlb_metrics.hit_rate * 100.0
                ),
                current_value: metrics.tlb_metrics.hit_rate,
                threshold: 0.9,
                impact: "增加页表遍历开销".to_string(),
            });
        }
        
        bottlenecks
    }

    /// 生成性能建议
    async fn generate_recommendations(
        &self,
        bottlenecks: &[Bottleneck],
        metrics: &SystemMetrics,
    ) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();
        
        for bottleneck in bottlenecks {
            match bottleneck.category {
                BottleneckCategory::Compilation => {
                    recommendations.push(Recommendation {
                        category: BottleneckCategory::Compilation,
                        priority: match bottleneck.severity {
                            Severity::Critical => 10,
                            Severity::High => 8,
                            Severity::Medium => 6,
                            Severity::Low => 4,
                        },
                        title: "优化编译性能".to_string(),
                        description: "考虑启用异步编译、增加编译缓存大小、优化编译器配置".to_string(),
                        expected_improvement: "编译时间减少30-50%".to_string(),
                    });
                }
                BottleneckCategory::Execution => {
                    recommendations.push(Recommendation {
                        category: BottleneckCategory::Execution,
                        priority: 7,
                        title: "优化执行性能".to_string(),
                        description: "检查热点代码优化、考虑启用SIMD优化、优化寄存器分配".to_string(),
                        expected_improvement: "执行时间减少10-20%".to_string(),
                    });
                }
                BottleneckCategory::GarbageCollection => {
                    recommendations.push(Recommendation {
                        category: BottleneckCategory::GarbageCollection,
                        priority: match bottleneck.severity {
                            Severity::Critical => 9,
                            Severity::High => 7,
                            _ => 5,
                        },
                        title: "优化GC性能".to_string(),
                        description: "启用并行GC、调整GC策略、优化内存分配模式".to_string(),
                        expected_improvement: "GC暂停时间减少50%".to_string(),
                    });
                }
                BottleneckCategory::Cache => {
                    recommendations.push(Recommendation {
                        category: BottleneckCategory::Cache,
                        priority: 6,
                        title: "优化缓存策略".to_string(),
                        description: "增加缓存大小、优化缓存替换策略、启用预取机制".to_string(),
                        expected_improvement: "缓存命中率提升到90%以上".to_string(),
                    });
                }
                BottleneckCategory::Tlb => {
                    recommendations.push(Recommendation {
                        category: BottleneckCategory::Tlb,
                        priority: 5,
                        title: "优化TLB性能".to_string(),
                        description: "增加TLB大小、优化TLB替换策略、优化内存访问模式".to_string(),
                        expected_improvement: "TLB命中率提升到95%以上".to_string(),
                    });
                }
                _ => {}
            }
        }
        
        // 根据并行度建议
        if metrics.parallel_metrics.active_vcpu_count < 2 {
            recommendations.push(Recommendation {
                category: BottleneckCategory::Parallelization,
                priority: 5,
                title: "启用并行执行".to_string(),
                description: "考虑增加vCPU数量以提升并行性能".to_string(),
                expected_improvement: "吞吐量提升50-100%".to_string(),
            });
        }
        
        // 按优先级排序
        recommendations.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        recommendations
    }

    /// 分析指标
    async fn analyze_metrics(
        &self,
        current: &SystemMetrics,
        historical: &[SystemMetrics],
    ) -> HashMap<String, MetricAnalysis> {
        let mut analysis = HashMap::new();
        
        // 分析编译时间
        if historical.len() > 1 {
            let compile_times: Vec<f64> = historical
                .iter()
                .map(|m| m.jit_metrics.avg_compile_time_ns)
                .collect();
            
            let mut sorted = compile_times.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            analysis.insert("compile_time".to_string(), MetricAnalysis {
                name: "编译时间".to_string(),
                current_value: current.jit_metrics.avg_compile_time_ns,
                average_value: compile_times.iter().sum::<f64>() / compile_times.len() as f64,
                max_value: *compile_times.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(&0.0),
                min_value: *compile_times.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(&0.0),
                percentiles: Percentiles {
                    p50: self.percentile(&sorted, 0.5),
                    p95: self.percentile(&sorted, 0.95),
                    p99: self.percentile(&sorted, 0.99),
                },
                trend: self.calculate_trend_value(&compile_times),
                is_anomaly: current.jit_metrics.avg_compile_time_ns > self.percentile(&sorted, 0.95),
            });
        }
        
        // 分析执行时间
        if historical.len() > 1 {
            let exec_times: Vec<f64> = historical
                .iter()
                .map(|m| m.jit_metrics.avg_execution_time_ns)
                .collect();
            
            let mut sorted = exec_times.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            analysis.insert("execution_time".to_string(), MetricAnalysis {
                name: "执行时间".to_string(),
                current_value: current.jit_metrics.avg_execution_time_ns,
                average_value: exec_times.iter().sum::<f64>() / exec_times.len() as f64,
                max_value: *exec_times.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(&0.0),
                min_value: *exec_times.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(&0.0),
                percentiles: Percentiles {
                    p50: self.percentile(&sorted, 0.5),
                    p95: self.percentile(&sorted, 0.95),
                    p99: self.percentile(&sorted, 0.99),
                },
                trend: self.calculate_trend_value(&exec_times),
                is_anomaly: current.jit_metrics.avg_execution_time_ns > self.percentile(&sorted, 0.95),
            });
        }
        
        analysis
    }

    /// 计算趋势
    async fn calculate_trends(&self, historical: &[SystemMetrics]) -> Vec<Trend> {
        let mut trends = Vec::new();
        
        if historical.len() < 2 {
            return trends;
        }
        
        // 计算编译时间趋势
        let compile_times: Vec<f64> = historical
            .iter()
            .map(|m| m.jit_metrics.avg_compile_time_ns)
            .collect();
        
        if compile_times.len() >= 2 {
            let trend_value = self.calculate_trend_value(&compile_times);
            let change_percent = if compile_times[0] > 0.0 {
                (compile_times[compile_times.len() - 1] - compile_times[0]) / compile_times[0] * 100.0
            } else {
                0.0
            };
            
            trends.push(Trend {
                metric_name: "compile_time".to_string(),
                direction: if trend_value > 0.1 { 1 } else if trend_value < -0.1 { -1 } else { 0 },
                change_percent,
                time_range_secs: 3600, // 假设1小时
            });
        }
        
        trends
    }

    /// 计算总体评分
    async fn calculate_overall_score(
        &self,
        metrics: &SystemMetrics,
        bottlenecks: &[Bottleneck],
    ) -> f64 {
        let mut score: f64 = 100.0;
        
        // 根据瓶颈扣分
        for bottleneck in bottlenecks {
            let penalty = match bottleneck.severity {
                Severity::Critical => 20.0,
                Severity::High => 10.0,
                Severity::Medium => 5.0,
                Severity::Low => 2.0,
            };
            score -= penalty;
        }
        
        // 根据性能指标调整
        if metrics.jit_metrics.cache_hit_rate < 0.8 {
            score -= 5.0;
        }
        if metrics.tlb_metrics.hit_rate < 0.9 {
            score -= 5.0;
        }
        if metrics.gc_metrics.avg_pause_time_ns > 10_000_000.0 {
            score -= 10.0;
        }
        
        score.max(0.0_f64).min(100.0_f64)
    }

    /// 计算百分位数
    fn percentile(&self, sorted: &[f64], p: f64) -> f64 {
        if sorted.is_empty() {
            return 0.0;
        }
        let index = (sorted.len() as f64 * p) as usize;
        sorted.get(index.min(sorted.len() - 1)).copied().unwrap_or(0.0)
    }

    /// 计算趋势值（线性回归斜率）
    fn calculate_trend_value(&self, values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }
        
        let n = values.len() as f64;
        let sum_x: f64 = (0..values.len()).map(|i| i as f64).sum();
        let sum_y: f64 = values.iter().sum();
        let sum_xy: f64 = values.iter().enumerate().map(|(i, &y)| i as f64 * y).sum();
        let sum_x2: f64 = (0..values.len()).map(|i| (i as f64).powi(2)).sum();
        
        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x.powi(2));
        slope
    }

    /// 生成性能报告（JSON格式）
    pub async fn generate_report_json(&self) -> Result<String, serde_json::Error> {
        let analysis = self.analyze().await;
        serde_json::to_string_pretty(&analysis)
    }

    /// 生成性能报告（文本格式）
    pub async fn generate_report_text(&self) -> String {
        let analysis = self.analyze().await;
        
        let mut report = String::new();
        report.push_str("=== 性能分析报告 ===\n\n");
        report.push_str(&format!("生成时间: {}\n", analysis.timestamp.format("%Y-%m-%d %H:%M:%S")));
        report.push_str(&format!("总体评分: {:.1}/100\n\n", analysis.overall_score));
        
        if !analysis.bottlenecks.is_empty() {
            report.push_str("性能瓶颈:\n");
            for (i, bottleneck) in analysis.bottlenecks.iter().enumerate() {
                report.push_str(&format!(
                    "  {}. [{}] {}\n",
                    i + 1,
                    match bottleneck.severity {
                        Severity::Critical => "严重",
                        Severity::High => "高",
                        Severity::Medium => "中",
                        Severity::Low => "低",
                    },
                    bottleneck.description
                ));
            }
            report.push_str("\n");
        }
        
        if !analysis.recommendations.is_empty() {
            report.push_str("优化建议:\n");
            for (i, rec) in analysis.recommendations.iter().enumerate() {
                report.push_str(&format!(
                    "  {}. [优先级: {}] {}\n",
                    i + 1,
                    rec.priority,
                    rec.title
                ));
                report.push_str(&format!("     {}\n", rec.description));
                report.push_str(&format!("     预期改进: {}\n", rec.expected_improvement));
            }
        }
        
        report
    }
}

