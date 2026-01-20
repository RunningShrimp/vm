# 项目快速导航

本文档帮助您快速定位项目中的代码和文档。

## 📁 目录结构总览

```
vm/
├── crates/          # 核心库（8个分类，26个模块）
├── tools/           # 用户工具（4个）
├── research/        # 研究项目（4个）
├── docs/            # 文档
├── tests/           # 测试
├── scripts/         # 脚本
├── plans/           # 规划文档
└── fixtures/        # 测试固件
```

---

## 🎯 按功能查找

### 我想... | 查看位置
---------|----------
**运行 VM** | [tools/cli/](./tools/cli/) 或 [tools/desktop/](./tools/desktop/)
**调试 VM** | [tools/debug/](./tools/debug/)
**查看执行引擎** | [crates/execution/](./crates/execution/)
**了解 JIT 实现** | [crates/execution/vm-engine-jit/](./crates/execution/vm-engine-jit/)
**管理内存** | [crates/memory/vm-mem/](./crates/memory/vm-mem/)
**添加设备** | [crates/devices/vm-device/](./crates/devices/vm-device/)
**加速虚拟化** | [crates/platform/vm-accel/](./crates/platform/vm-accel/)
**跨架构支持** | [crates/architecture/vm-cross-arch-support/](./crates/architecture/vm-cross-arch-support/)
**性能测试** | [research/perf-bench/](./research/perf-bench/)
**了解架构** | [docs/architecture/](./docs/architecture/)
**开发指南** | [docs/development/](./docs/development/)

---

## 📦 Crates 快速导航

### Core (核心组件)
- **[vm-core](./crates/core/vm-core/)** - 领域模型、事件存储
- **[vm-ir](./crates/core/vm-ir/)** - 中间表示
- **[vm-boot](./crates/core/vm-boot/)** - 启动流程

### Execution (执行引擎)
- **[vm-frontend](./crates/execution/vm-frontend/)** - 指令解码（x86_64/ARM64/RISC-V）
- **[vm-engine](./crates/execution/vm-engine/)** - 解释器执行
- **[vm-engine-jit](./crates/execution/vm-engine-jit/)** - JIT 编译器

### Memory (内存管理)
- **[vm-mem](./crates/memory/vm-mem/)** - MMU、地址空间
- **[vm-gc](./crates/memory/vm-gc/)** - 垃圾收集
- **[vm-optimizers](./crates/memory/vm-optimizers/)** - 性能优化

### Platform (平台层)
- **[vm-accel](./crates/platform/vm-accel/)** - KVM/HVF/WHP
- **[vm-platform](./crates/platform/vm-platform/)** - 平台特定代码
- **[vm-osal](./crates/platform/vm-osal/)** - 操作系统抽象

### Devices (设备)
- **[vm-device](./crates/devices/vm-device/)** - 设备框架
- **[vm-graphics](./crates/devices/vm-graphics/)** - GPU 设备
- **[vm-smmu](./crates/devices/vm-smmu/)** - IOMMU/SMMU
- **[vm-soc](./crates/devices/vm-soc/)** - 片上系统设备

### Runtime (运行时)
- **[vm-service](./crates/runtime/vm-service/)** - 服务编排
- **[vm-plugin](./crates/runtime/vm-plugin/)** - 插件系统
- **[vm-monitor](./crates/runtime/vm-monitor/)** - 监控和指标

### Compatibility (兼容性)
- **[security-sandbox](./crates/compatibility/security-sandbox/)** - 安全沙箱
- **[syscall-compat](./crates/compatibility/syscall-compat/)** - 系统调用兼容

### Architecture (架构)
- **[vm-cross-arch-support](./crates/architecture/vm-cross-arch-support/)** - 跨架构支持
- **[vm-codegen](./crates/architecture/vm-codegen/)** - 代码生成
- **[vm-build-deps](./crates/architecture/vm-build-deps/)** - 构建依赖

---

## 🛠️ Tools 快速导航

| 工具 | 用途 | 位置 |
|------|------|------|
| **vm-cli** | 命令行管理 VM | [tools/cli/](./tools/cli/) |
| **vm-desktop** | 桌面 GUI 应用 | [tools/desktop/](./tools/desktop/) |
| **vm-debug** | 调试工具 | [tools/debug/](./tools/debug/) |
| **vm-passthrough** | 设备直通 | [tools/passthrough/](./tools/passthrough/) |

---

## 🔬 Research 快速导航

| 项目 | 研究内容 | 位置 |
|------|----------|------|
| **perf-bench** | 性能基准测试 | [research/perf-bench/](./research/perf-bench/) |
| **tiered-compiler** | 分层编译器 | [research/tiered-compiler/](./research/tiered-compiler/) |
| **parallel-jit** | 并行 JIT | [research/parallel-jit/](./research/parallel-jit/) |
| **benches** | 综合基准测试 | [research/benches/](./research/benches/) |

---

## 📚 文档导航

### 用户文档
- **[用户指南](./docs/user-guides/)** - CLI 和 GUI 使用指南
- **[多平台支持](./docs/user-guides/MULTI_OS_SUPPORT.md)** - 平台兼容性

### 开发文档
- **[架构文档](./docs/architecture/ARCHITECTURE.md)** - 系统架构
- **[开发指南](./docs/development/)** - 贡献和开发流程
- **[API 文档](./docs/api/)** - 模块 API 文档

### 规划文档
- **[规划目录](./plans/)** - 功能规划和设计文档

---

## 🚀 常见任务快速入口

### 我想...

**开始使用**
```bash
# 快速启动
cargo run -p vm-cli install-debian

# 运行测试
cargo test --workspace

# 构建所有
cargo build --release
```

**添加新设备**
1. 查看 [crates/devices/vm-device/](./crates/devices/vm-device/)
2. 实现设备 trait
3. 在 [vm-service](./crates/runtime/vm-service/) 中注册

**优化性能**
1. 运行 [perf-bench](./research/perf-bench/) 评估
2. 查看 [vm-optimizers](./crates/memory/vm-optimizers/)
3. 考虑 [vm-engine-jit](./crates/execution/vm-engine-jit/) 优化

**添加新架构**
1. 参考 [vm-frontend](./crates/execution/vm-frontend/)
2. 实现 [vm-cross-arch-support](./crates/architecture/vm-cross-arch-support/)
3. 在 [vm-codegen](./crates/architecture/vm-codegen/) 中添加代码生成

**贡献代码**
1. 阅读 [CONTRIBUTING.md](./docs/development/CONTRIBUTING.md)
2. 查看 [开发指南](./docs/development/)
3. 运行测试和基准

---

## 🔍 按关键词查找

### 关键词 | 位置
--------|------
`JIT`, `compiler` | [crates/execution/vm-engine-jit/](./crates/execution/vm-engine-jit/)
`decode`, `frontend` | [crates/execution/vm-frontend/](./crates/execution/vm-frontend/)
`memory`, `MMU` | [crates/memory/vm-mem/](./crates/memory/vm-mem/)
`device`, `virtio` | [crates/devices/vm-device/](./crates/devices/vm-device/)
`KVM`, `HVF`, `WHP` | [crates/platform/vm-accel/](./crates/platform/vm-accel/)
`GPU`, `graphics` | [crates/devices/vm-graphics/](./crates/devices/vm-graphics/)
`plugin` | [crates/runtime/vm-plugin/](./crates/runtime/vm-plugin/)
`benchmark`, `perf` | [research/perf-bench/](./research/perf-bench/)
`cross-arch`, `translation` | [crates/architecture/vm-cross-arch-support/](./crates/architecture/vm-cross-arch-support/)

---

## 📞 获取帮助

1. **查看文档**: [docs/](./docs/)
2. **查看示例**: [examples/](./crates/*/vm-*/examples/)
3. **运行测试**: `cargo test -p <crate-name>`
4. **查看源码**: 浏览相应的 crate 目录

---

## 🗺️ 依赖关系图

```
vm-cli / vm-desktop (用户界面)
    ↓
vm-service (服务编排)
    ↓
├── vm-core (领域核心)
├── vm-engine / vm-engine-jit (执行)
├── vm-mem (内存)
├── vm-device (设备)
└── vm-accel (加速)
```

---

**提示**: 使用 `Ctrl+F` 或 `Cmd+F` 快速搜索本文档，或使用上面提供的按功能查找表格。
