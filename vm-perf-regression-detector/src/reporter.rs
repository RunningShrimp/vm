//! 性能回归报告生成器

use anyhow::Result;
use serde_json;
use std::collections::HashMap;
use std::fs;

use super::config::{ReportConfig, ReportFormat};
use super::detector::{RegressionResult, RegressionSeverity};

/// 性能回归报告生成器
pub struct RegressionReporter {
    config: ReportConfig,
}

impl RegressionReporter {
    /// 创建新的报告生成器
    pub fn new(config: ReportConfig) -> Self {
        Self { config }
    }

    /// 生成回归报告
    pub fn generate_report(&self, results: &[RegressionResult]) -> Result<String> {
        match self.config.format {
            ReportFormat::Text => self.generate_text_report(results),
            ReportFormat::Json => self.generate_json_report(results),
            ReportFormat::Html => self.generate_html_report(results),
            ReportFormat::Markdown => self.generate_markdown_report(results),
        }
    }

    /// 保存报告到文件
    pub fn save_report(&self, results: &[RegressionResult]) -> Result<()> {
        let report = self.generate_report(results)?;
        fs::write(&self.config.output_path, report)?;

        // 如果需要生成图表
        if self.config.generate_charts {
            self.generate_charts(results)?;
        }

        Ok(())
    }

    /// 生成文本格式报告
    fn generate_text_report(&self, results: &[RegressionResult]) -> Result<String> {
        let mut report = String::new();

        report.push_str("性能回归检测报告\n");
        report.push_str("================\n\n");

        // 统计信息
        let total_regressions = results.len();
        let critical_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Critical)
            .count();
        let major_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Major)
            .count();
        let moderate_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Moderate)
            .count();
        let minor_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Minor)
            .count();

        report.push_str(&format!("总回归数: {}\n", total_regressions));
        report.push_str(&format!("关键回归: {}\n", critical_count));
        report.push_str(&format!("严重回归: {}\n", major_count));
        report.push_str(&format!("中等回归: {}\n", moderate_count));
        report.push_str(&format!("轻微回归: {}\n\n", minor_count));

        // 详细结果
        if !results.is_empty() {
            report.push_str("回归详情:\n");
            report.push_str("----------\n");

            for result in results {
                report.push_str(&format!("指标: {}\n", result.metric_name));
                report.push_str(&format!("当前值: {:.2}\n", result.current_value));
                report.push_str(&format!("基准值: {:.2}\n", result.baseline_value));
                report.push_str(&format!("变化: {:.2}%\n", result.percentage_change));
                report.push_str(&format!("严重程度: {:?}\n", result.severity));

                if let Some(p_value) = result.p_value {
                    report.push_str(&format!("统计显著性: {:.4}\n", p_value));
                }

                report.push_str(&format!("检测算法: {}\n", result.algorithm));
                report.push_str(&format!(
                    "检测时间: {}\n",
                    result.timestamp.format("%Y-%m-%d %H:%M:%S")
                ));
                report.push_str("----------\n");
            }
        } else {
            report.push_str("未检测到性能回归。\n");
        }

        Ok(report)
    }

    /// 生成JSON格式报告
    fn generate_json_report(&self, results: &[RegressionResult]) -> Result<String> {
        let report = serde_json::to_string_pretty(results)?;
        Ok(report)
    }

    /// 生成HTML格式报告
    fn generate_html_report(&self, results: &[RegressionResult]) -> Result<String> {
        let mut report = String::new();

        report.push_str("<!DOCTYPE html>\n");
        report.push_str("<html>\n");
        report.push_str("<head>\n");
        report.push_str("    <meta charset=\"UTF-8\">\n");
        report.push_str("    <title>性能回归检测报告</title>\n");
        report.push_str("    <style>\n");
        report.push_str("        body { font-family: Arial, sans-serif; margin: 20px; }\n");
        report.push_str("        h1 { color: #333; }\n");
        report.push_str("        h2 { color: #555; }\n");
        report.push_str(
            "        table { border-collapse: collapse; width: 100%; margin-top: 20px; }\n",
        );
        report.push_str(
            "        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }\n",
        );
        report.push_str("        th { background-color: #f2f2f2; }\n");
        report.push_str("        .critical { color: #d32f2f; }\n");
        report.push_str("        .major { color: #f57c00; }\n");
        report.push_str("        .moderate { color: #ff8f00; }\n");
        report.push_str("        .minor { color: #ffc107; }\n");
        report.push_str("    </style>\n");
        report.push_str("</head>\n");
        report.push_str("<body>\n");

        report.push_str("    <h1>性能回归检测报告</h1>\n");

        // 统计信息
        let total_regressions = results.len();
        let critical_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Critical)
            .count();
        let major_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Major)
            .count();
        let moderate_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Moderate)
            .count();
        let minor_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Minor)
            .count();

        report.push_str("    <h2>统计信息</h2>\n");
        report.push_str("    <p>\n");
        report.push_str(&format!("        总回归数: {}<br>\n", total_regressions));
        report.push_str(&format!("        关键回归: {}<br>\n", critical_count));
        report.push_str(&format!("        严重回归: {}<br>\n", major_count));
        report.push_str(&format!("        中等回归: {}<br>\n", moderate_count));
        report.push_str(&format!("        轻微回归: {}\n", minor_count));
        report.push_str("    </p>\n");

        // 详细结果
        if !results.is_empty() {
            report.push_str("    <h2>回归详情</h2>\n");
            report.push_str("    <table>\n");
            report.push_str("        <tr>\n");
            report.push_str("            <th>指标</th>\n");
            report.push_str("            <th>当前值</th>\n");
            report.push_str("            <th>基准值</th>\n");
            report.push_str("            <th>变化</th>\n");
            report.push_str("            <th>严重程度</th>\n");
            report.push_str("            <th>统计显著性</th>\n");
            report.push_str("            <th>检测算法</th>\n");
            report.push_str("            <th>检测时间</th>\n");
            report.push_str("        </tr>\n");

            for result in results {
                let severity_class = match result.severity {
                    RegressionSeverity::Critical => "critical",
                    RegressionSeverity::Major => "major",
                    RegressionSeverity::Moderate => "moderate",
                    RegressionSeverity::Minor => "minor",
                    RegressionSeverity::None => "none",
                };

                report.push_str("        <tr>\n");
                report.push_str(&format!("            <td>{}</td>\n", result.metric_name));
                report.push_str(&format!(
                    "            <td>{:.2}</td>\n",
                    result.current_value
                ));
                report.push_str(&format!(
                    "            <td>{:.2}</td>\n",
                    result.baseline_value
                ));
                report.push_str(&format!(
                    "            <td>{:.2}%</td>\n",
                    result.percentage_change
                ));
                report.push_str(&format!(
                    "            <td class=\"{}\">{:?}</td>\n",
                    severity_class, result.severity
                ));

                if let Some(p_value) = result.p_value {
                    report.push_str(&format!("            <td>{:.4}</td>\n", p_value));
                } else {
                    report.push_str("            <td>-</td>\n");
                }

                report.push_str(&format!("            <td>{}</td>\n", result.algorithm));
                report.push_str(&format!(
                    "            <td>{}</td>\n",
                    result.timestamp.format("%Y-%m-%d %H:%M:%S")
                ));
                report.push_str("        </tr>\n");
            }

            report.push_str("    </table>\n");
        } else {
            report.push_str("    <p>未检测到性能回归。</p>\n");
        }

        report.push_str("</body>\n");
        report.push_str("</html>\n");

        Ok(report)
    }

    /// 生成Markdown格式报告
    fn generate_markdown_report(&self, results: &[RegressionResult]) -> Result<String> {
        let mut report = String::new();

        report.push_str("# 性能回归检测报告\n\n");

        // 统计信息
        let total_regressions = results.len();
        let critical_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Critical)
            .count();
        let major_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Major)
            .count();
        let moderate_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Moderate)
            .count();
        let minor_count = results
            .iter()
            .filter(|r| r.severity == RegressionSeverity::Minor)
            .count();

        report.push_str("## 统计信息\n\n");
        report.push_str(&format!("- 总回归数: {}\n", total_regressions));
        report.push_str(&format!("- 关键回归: {}\n", critical_count));
        report.push_str(&format!("- 严重回归: {}\n", major_count));
        report.push_str(&format!("- 中等回归: {}\n", moderate_count));
        report.push_str(&format!("- 轻微回归: {}\n\n", minor_count));

        // 详细结果
        if !results.is_empty() {
            report.push_str("## 回归详情\n\n");
            report.push_str(
                "| 指标 | 当前值 | 基准值 | 变化 | 严重程度 | 统计显著性 | 检测算法 | 检测时间 |\n",
            );
            report.push_str(
                "|------|--------|--------|------|----------|------------|----------|----------|\n",
            );

            for result in results {
                let severity_str = match result.severity {
                    RegressionSeverity::Critical => "🔴 关键",
                    RegressionSeverity::Major => "🟠 严重",
                    RegressionSeverity::Moderate => "🟡 中等",
                    RegressionSeverity::Minor => "🟢 轻微",
                    RegressionSeverity::None => "✅ 无",
                };

                let p_value_str = if let Some(p_value) = result.p_value {
                    format!("{:.4}", p_value)
                } else {
                    "-".to_string()
                };

                report.push_str(&format!(
                    "| {} | {:.2} | {:.2} | {:.2}% | {} | {} | {} | {} |\n",
                    result.metric_name,
                    result.current_value,
                    result.baseline_value,
                    result.percentage_change,
                    severity_str,
                    p_value_str,
                    result.algorithm,
                    result.timestamp.format("%Y-%m-%d %H:%M:%S")
                ));
            }
        } else {
            report.push_str("未检测到性能回归。\n");
        }

        Ok(report)
    }

    /// 生成图表
    fn generate_charts(&self, results: &[RegressionResult]) -> Result<()> {
        // 创建图表目录
        fs::create_dir_all(&self.config.charts_path)?;

        // 按严重程度分组
        let mut grouped_results: HashMap<RegressionSeverity, Vec<&RegressionResult>> =
            HashMap::new();
        for result in results {
            let entry = grouped_results.entry(result.severity.clone()).or_default();
            entry.push(result);
        }

        // 生成严重程度分布图
        self.generate_severity_chart(&grouped_results)?;

        // 生成指标变化图
        self.generate_metrics_change_chart(results)?;

        Ok(())
    }

    /// 生成严重程度分布图
    fn generate_severity_chart(
        &self,
        grouped_results: &HashMap<RegressionSeverity, Vec<&RegressionResult>>,
    ) -> Result<()> {
        use plotters::prelude::*;

        let output_path = format!("{}/severity_distribution.png", self.config.charts_path);

        let root = BitMapBackend::new(&output_path, (640, 480)).into_drawing_area();

        let data = [
            (
                "关键",
                grouped_results
                    .get(&RegressionSeverity::Critical)
                    .map_or(0, |v| v.len()) as f32,
            ),
            (
                "严重",
                grouped_results
                    .get(&RegressionSeverity::Major)
                    .map_or(0, |v| v.len()) as f32,
            ),
            (
                "中等",
                grouped_results
                    .get(&RegressionSeverity::Moderate)
                    .map_or(0, |v| v.len()) as f32,
            ),
            (
                "轻微",
                grouped_results
                    .get(&RegressionSeverity::Minor)
                    .map_or(0, |v| v.len()) as f32,
            ),
            (
                "无",
                grouped_results
                    .get(&RegressionSeverity::None)
                    .map_or(0, |v| v.len()) as f32,
            ),
        ];

        root.fill(&WHITE)?;

        // 手动计算最大计数，处理浮点数
        let max_count = data
            .iter()
            .map(|&(_, count)| count)
            .fold(
                0.0,
                |max, current| if current > max { current } else { max },
            )
            * 1.2;

        // 创建一个简单的柱状图
        let mut chart = ChartBuilder::on(&root)
            .caption("性能回归严重程度分布", ("Arial", 20))
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(
                // 使用固定的x值范围，每个严重程度类型对应一个位置
                0.0..5.0,
                0.0..max_count,
            )?;

        // 配置网格和标签
        chart
            .configure_mesh()
            .disable_x_mesh()
            .y_labels(5)
            .y_label_formatter(&|y| format!("{:.0}", y))
            // 禁用自动x标签，因为我们要手动添加
            .x_labels(0)
            .draw()?;

        // 绘制柱状图，每个柱形宽度为0.8，居中于对应x位置
        let bar_width = 0.8;
        let bar_series = data.iter().enumerate().map(|(i, &(_, count))| {
            // 计算每个柱形的x位置（0.5, 1.5, 2.5, 3.5, 4.5）
            let x = i as f64 + 0.5;
            // 使用矩形绘制柱形
            Rectangle::new(
                [(x - bar_width / 2.0, 0.0), (x + bar_width / 2.0, count)],
                BLUE.filled(),
            )
        });

        chart.draw_series(bar_series)?;

        // 手动添加x轴标签，使用简单的Text元素
        for (i, &(label, _)) in data.iter().enumerate() {
            let x = i as f64 + 0.5;
            // 使用ChartContext的draw_series方法添加文本
            chart.draw_series(vec![Text::new(
                label.to_string(),
                (x, -5.0),
                ("Arial", 12).into_font(),
            )])?;
        }

        Ok(())
    }

    /// 生成指标变化图
    fn generate_metrics_change_chart(&self, results: &[RegressionResult]) -> Result<()> {
        use plotters::prelude::*;

        // 按指标分组
        let mut grouped_metrics: HashMap<String, Vec<&RegressionResult>> = HashMap::new();
        for result in results {
            let entry = grouped_metrics
                .entry(result.metric_name.clone())
                .or_default();
            entry.push(result);
        }

        // 为每个指标生成一个图表
        for (metric_name, metric_results) in grouped_metrics {
            if metric_results.len() < 2 {
                continue;
            }

            let chart_path = format!("{}/{}_change.png", self.config.charts_path, metric_name);
            let chart_root = BitMapBackend::new(&chart_path, (640, 480)).into_drawing_area();

            let data: Vec<(f32, f32)> = metric_results
                .iter()
                .enumerate()
                .map(|(i, r)| (i as f32, r.percentage_change as f32))
                .collect();

            chart_root.fill(&WHITE)?;

            // 计算数据范围
            let min_y = data
                .iter()
                .map(|&(_, y)| y)
                .reduce(|a, b| if a < b { a } else { b })
                .unwrap_or(0.0);
            let max_y = data
                .iter()
                .map(|&(_, y)| y)
                .reduce(|a, b| if a > b { a } else { b })
                .unwrap_or(100.0);
            let range_y = max_y - min_y;
            let lower_y = if range_y > 0.0 {
                min_y - range_y * 0.1
            } else {
                min_y - 10.0
            };
            let upper_y = if range_y > 0.0 {
                max_y + range_y * 0.1
            } else {
                max_y + 10.0
            };

            // 创建图表
            let mut chart = ChartBuilder::on(&chart_root)
                .caption(format!("{} 变化趋势", metric_name), ("Arial", 16))
                .margin(5)
                .x_label_area_size(30)
                .y_label_area_size(30)
                .build_cartesian_2d(0.0..(metric_results.len() as f32 - 1.0), lower_y..upper_y)?;

            // 配置网格
            chart.configure_mesh().x_labels(5).y_labels(5).draw()?;

            // 绘制折线
            chart.draw_series(LineSeries::new(data.clone(), &RED))?;

            // 绘制数据点
            chart.draw_series(
                data.iter()
                    .map(|&(x, y)| Circle::new((x, y), 2, RED.filled())),
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_engine::jit::config::{ReportConfig, ReportFormat};
    use vm_engine::jit::detector::{RegressionResult, RegressionSeverity};

    #[test]
    fn test_text_report() -> Result<()> {
        let config = ReportConfig {
            format: ReportFormat::Text,
            output_path: "test_report.txt".to_string(),
            generate_charts: false,
            charts_path: "charts".to_string(),
        };

        let reporter = RegressionReporter::new(config);

        let results = vec![RegressionResult {
            metric_name: "execution_time".to_string(),
            current_value: 125.0,
            baseline_value: 100.0,
            percentage_change: 25.0,
            severity: RegressionSeverity::Critical,
            p_value: Some(0.01),
            algorithm: "Z-Score".to_string(),
            timestamp: chrono::Utc::now(),
        }];

        let report = reporter.generate_report(&results)?;

        assert!(report.contains("性能回归检测报告"));
        assert!(report.contains("execution_time"));
        assert!(report.contains("25.00%"));
        assert!(report.contains("关键"));

        Ok(())
    }

    #[test]
    fn test_json_report() -> Result<()> {
        let config = ReportConfig {
            format: ReportFormat::Json,
            output_path: "test_report.json".to_string(),
            generate_charts: false,
            charts_path: "charts".to_string(),
        };

        let reporter = RegressionReporter::new(config);

        let results = vec![RegressionResult {
            metric_name: "execution_time".to_string(),
            current_value: 125.0,
            baseline_value: 100.0,
            percentage_change: 25.0,
            severity: RegressionSeverity::Critical,
            p_value: Some(0.01),
            algorithm: "Z-Score".to_string(),
            timestamp: chrono::Utc::now(),
        }];

        let report = reporter.generate_report(&results)?;

        // 验证是否为有效JSON
        let parsed: Vec<RegressionResult> = serde_json::from_str(&report)?;
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].metric_name, "execution_time");

        Ok(())
    }
}
