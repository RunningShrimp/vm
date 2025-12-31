---
name: 性能问题
about: 报告性能问题或提出性能优化建议
title: '[PERF] '
labels: performance
assignees: ''
---

## 性能问题描述

清晰简洁地描述性能问题。

**问题类型**：
- [ ] 执行速度慢
- [ ] 内存占用高
- [ ] 启动时间长
- [ ] 吞吐量低
- [ ] 延迟高
- [ ] 资源泄漏
- [ ] 其他

## 环境信息

- **操作系统**: [例如 Ubuntu 22.04]
- **CPU**: [例如 Intel i7-12700K, Apple M2]
- **内存**: [例如 16GB]
- **Rust版本**: [运行 `rustc --version`]
- **VM版本**: [例如 v0.2.0]
- **编译选项**: [例如 `--release`, `--features "jit"`]
- **运行配置**: [例如 CPU核数、内存大小]

## 复现场景

描述如何复现性能问题：

1. 配置：...
2. 运行：...
3. 观察：...

```rust
// 如果适用，提供复现代码
```

## 性能指标

**当前性能**：

- 指标1：...（例如：1000ms）
- 指标2：...（例如：2GB内存）
- 指标3：...

**期望性能**：

- 指标1：...（例如：<100ms）
- 指标2：...（例如：<500MB内存）
- 指标3：...

**性能差距**：

- 指标1：差距X%
- 指标2：差距Y倍
- 指标3：...

## Benchmark数据

如果可能，请提供benchmark数据：

```bash
# 使用criterion运行benchmark
cargo bench --bench performance_test

# 或使用flamegraph分析
cargo flamegraph --bench performance_test
```

**Benchmark输出**：

```
粘贴benchmark结果
```

**火焰图**（如果适用）：
上传或链接到火焰图。

## 分析和假设

**可能的原因**：

根据您的分析，可能的原因包括：

1. 原因1：...
2. 原因2：...
3. ...

**已尝试的优化**：

描述您已经尝试的优化方法：

1. 优化1：...（结果：...）
2. 优化2：...（结果：...）

## Profiling数据

如果可能，提供profiling数据：

- [ ] CPU profile
- [ ] Memory profile
- [ ] Heap profile
- [ ] Flamegraph

**工具**：
- `cargo-flamegraph`
- `perf`
- `valgrind`
- `heaptrack`

```bash
# 示例：生成flamegraph
cargo install flamegraph
cargo flamegraph --bench your_bench
```

## 工作负载特征

描述典型的工作负载：

- **指令集**：[例如 RISC-V, x86_64]
- **程序类型**：[例如 计算密集, IO密集]
- **数据规模**：[例如 小程序(<1MB), 中等(1-100MB), 大型(>100MB)]
- **运行时长**：[例如 短期(<1s), 中期(1-60s), 长期(>60s)]

## 优先级

- [ ] P0 - 严重性能退化
- [ ] P1 - 重要性能瓶颈
- [ ] P2 - 性能改进
- [ ] P3 - 优化建议

## 相关代码

指出可能的性能瓶颈位置：

- 文件：`src/file.rs`
- 函数：`function_name`
- 行号：约第X行

## 参考和对比

**与其他实现的对比**：

- QEMU：...
- 其他VM：...

**硬件对比**：

- 原生执行：...
- 其他虚拟化方案：...

## 附加信息

- Profiling数据
- 火焰图图片
- 任何其他有助于分析性能问题的信息

## 愿意贡献

- [ ] 我愿意帮助优化性能
- [ ] 我愿意提供更多profiling数据
- [ ] 我愿意帮助审查性能优化PR
- [ ] 我有相关专业知识

感谢报告性能问题！性能是VM项目的核心关注点。
