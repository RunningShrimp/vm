# vm-monitor

**VM项目性能监控系统**

[![Rust](https://img.shields.io/badge/rust-2024%20Edition-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## 概述

`vm-monitor` 是VM项目的性能监控系统，提供实时性能指标收集、分析、告警和可视化功能。它帮助开发者和运维人员监控VM的运行状态，及时发现和诊断性能问题。

## 🎯 核心功能

- **实时指标收集**: CPU、内存、I/O、网络等性能指标
- **性能分析**: 热点检测、瓶颈分析、趋势预测
- **告警系统**: 可配置的阈值告警和异常检测
- **数据导出**: 支持Prometheus、JSON、CSV等格式
- **可视化仪表板**: 内置Web仪表板和Grafana集成
- **历史数据**: 时序数据存储和查询

## 📦 主要组件

### 1. 性能指标收集

```rust
use vm_monitor::MetricsCollector;

let collector = MetricsCollector::new()?;

// 启动收集器
collector.start()?;

// 获取当前指标
let metrics = collector.get_current_metrics()?;
println!("CPU使用率: {}%", metrics.cpu_usage);
println!("内存使用: {}/{}", metrics.memory_used, metrics.memory_total);
```

**支持的指标类型**:
- CPU使用率、vCPU调度延迟
- 内存使用、换页率
- 磁盘I/O、IOPS、延迟
- 网络吞吐、丢包率
- JIT编译统计
- 缓存命中率

### 2. 告警系统

```rust
use vm_monitor::{AlertManager, AlertRule, AlertThreshold};

let alert_mgr = AlertManager::new();

// 配置CPU告警
let cpu_rule = AlertRule {
    name: "High CPU Usage".to_string(),
    metric: "cpu_usage".to_string(),
    condition: AlertThreshold::GreaterThan(80.0),
    duration_secs: 60,
};

alert_mgr.add_rule(cpu_rule)?;

// 检查告警
let alerts = alert_mgr.check_alerts(&metrics)?;
for alert in alerts {
    println!("ALERT: {} - {}", alert.name, alert.message);
}
```

### 3. 数据导出

```rust
use vm_monitor::MetricsExporter;

// 导出为Prometheus格式
let exporter = MetricsExporter::prometheus();
let prometheus_text = exporter.export(&metrics)?;

// 导出为JSON
let exporter = MetricsExporter::json();
let json_data = exporter.export(&metrics)?;
```

## 🚀 使用场景

### 场景1: 实时监控VM性能

```bash
# 启动监控服务器
vm-monitor --port 9090

# 访问Web仪表板
http://localhost:9090

# 或使用Prometheus采集
curl http://localhost:9090/metrics
```

### 场景2: 配置告警规则

```toml
# config/monitoring.toml
[alerts.cpu]
threshold = 80.0
duration = 60
action = "email"

[alerts.memory]
threshold = 90.0
duration = 30
action = "slack"
```

## 📝 API概览

```rust
pub struct MetricsCollector {
    // 指标收集器
}

pub struct AlertManager {
    // 告警管理器
}

pub struct MetricsExporter {
    // 指标导出器
}
```

## 🔧 依赖关系

```toml
[dependencies]
vm-core = { path = "../vm-core" }
serde = { workspace = true }
```

## 📚 相关文档

- [vm-core](../vm-core/README.md) - 核心VM功能
- [MASTER_DOCUMENTATION_INDEX](../MASTER_DOCUMENTATION_INDEX.md)

## 📝 许可证

MIT License - 详见 [LICENSE](../LICENSE)

---

**包版本**: workspace v0.1.0
**最后更新**: 2026-01-07
