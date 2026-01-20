# vm-debug

**VM项目调试支持系统**

[![Rust](https://img.shields.io/badge/rust-2024%20Edition-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## 概述

`vm-debug` 是VM项目的完整调试支持系统，提供GDB远程调试、性能分析、结构化日志、快照调试等专业级调试功能。它集成了多种调试工具和接口，使开发者能够高效地调试虚拟机运行时的各种问题。

## 🎯 核心功能

- **GDB远程调试**: 完整的GDB协议实现，支持断点、单步执行、变量查看
- **性能分析器**: 热点检测、调用跟踪、内存分析
- **结构化日志**: 分层日志系统，支持不同级别的调试信息
- **调试代理**: 统一的调试接口，支持多种调试前端
- **快照调试**: 虚拟机状态快照和回溯调试
- **条件断点**: 基于表达式和条件的断点设置

## 📦 主要组件

### 1. GDB远程调试 (GDB Remote Debugging)

完整的GDB协议实现，支持标准GDB客户端：

```rust
use vm_debug::{DebuggerConfig, GdbStub};

// 配置GDB存根
let config = DebuggerConfig {
    enable_gdb_stub: true,
    gdb_port: 1234,
    ..Default::default()
};

// 创建GDB存根
let gdb_stub = GdbStub::new(config)?;

// 启动GDB服务器
gdb_stub.start()?;

// 在GDB客户端中连接
// $ gdb vmlinux
// (gdb) target remote :1234
// (gdb) continue
```

**支持的GDB命令**:
- 断点设置: `break`, `hb`, `watch`
- 执行控制: `continue`, `step`, `stepi`, `next`, `nexti`
- 变量查看: `print`, `x`, `info registers`
- 内存操作: `x`, `info mem`

### 2. 性能分析器 (Profiler)

强大的性能分析和热点检测：

```rust
use vm_debug::Profiler;

let profiler = Profiler::new()?;

// 开始性能分析
profiler.start()?;

// ... 运行VM ...

// 停止分析并获取结果
let report = profiler.stop()?;

println!("Hot functions:");
for func in report.hot_functions {
    println!("  {}: {} samples ({}%)",
        func.name,
        func.sample_count,
        func.percentage
    );
}
```

**分析功能**:
- 热点检测 (Hotspot Detection)
- 调用图分析 (Call Graph Analysis)
- 内存访问分析 (Memory Access Analysis)
- 指令级分析 (Instruction-level Profiling)

### 3. 结构化日志 (Structured Logging)

分层日志系统：

```rust
use vm_debug::{Logger, LogLevel};

// 初始化日志系统
let logger = Logger::new(LogLevel::Debug);

// 记录不同级别的日志
logger.error("Failed to allocate memory");
logger.warn("High memory usage detected");
logger.info("VM started successfully");
logger.debug("Instruction executed at 0x{:x}", pc);
logger.trace("Register state: {:?}", regs);

// 查询日志
let logs = logger.get_logs(LogLevel::Debug)?;
for log in logs {
    println!("[{}] {}", log.level, log.message);
}
```

**日志级别**:
- `Error` - 错误信息
- `Warn` - 警告信息
- `Info` - 一般信息
- `Debug` - 调试信息
- `Trace` - 详细跟踪

### 4. 快照调试 (Snapshot Debugging)

虚拟机状态快照和回溯：

```rust
use vm_debug::SnapshotManager;

let snapshots = SnapshotManager::new();

// 创建快照
let snapshot_id = snapshots.create_snapshot(&vm_state)?;

// 回滚到快照
let restored_state = snapshots.restore_snapshot(snapshot_id)?;

// 列出所有快照
for snapshot in snapshots.list_snapshots() {
    println!("Snapshot {}: {}", snapshot.id, snapshot.timestamp);
}

// 删除快照
snapshots.delete_snapshot(snapshot_id)?;
```

### 5. 条件断点 (Conditional Breakpoints)

基于表达式的智能断点：

```rust
use vm_debug::{BreakpointManager, BreakpointCondition};

let bp_manager = BreakpointManager::new();

// 设置条件断点
bp_manager.set_breakpoint(
    0x1000,                           // 地址
    BreakpointCondition::RegisterEq { // 条件
        reg: "rax".to_string(),
        value: 0,
    }
)?;

// 设置内存访问断点
bp_manager.set_watchpoint(
    0x2000,
    BreakpointCondition::MemoryWrite,
)?;
```

## 🔧 依赖关系

```toml
[dependencies]
vm-core = { path = "../vm-core" }      # 核心VM类型
serde = { workspace = true }           # 序列化
```

## 🚀 使用场景

### 场景1: 使用GDB调试VM启动

```bash
# 1. 启动VM并启用GDB存根
vm-cli --kernel vmlinux --enable-gdb --gdb-port 1234

# 2. 在另一个终端启动GDB
$ gdb vmlinux
(gdb) target remote :1234
(gdb) break start_kernel
(gdb) continue
(gdb) info registers
(gdb) x/10i $pc
```

### 场景2: 性能分析热点检测

```rust
use vm_debug::Profiler;

let profiler = Profiler::new()?;

profiler.start()?;
run_vm_for_some_time();
let report = profiler.stop()?;

// 输出热点函数
println!("=== Hot Functions ===");
for (i, func) in report.hot_functions.iter().take(10).enumerate() {
    println!("{}. {}: {}%", i + 1, func.name, func.percentage);
}
```

### 场景3: 内存访问分析

```rust
use vm_debug::MemoryAnalyzer;

let analyzer = MemoryAnalyzer::new()?;

// 记录内存访问
analyzer.record_access(pc, addr, size, access_type)?;

// 生成分析报告
let report = analyzer.analyze()?;

println!("Most accessed addresses:");
for addr in report.most_accessed {
    println!("  0x{:x}: {} accesses", addr.address, addr.count);
}
```

## 📝 API概览

### 调试器配置

```rust
pub struct DebuggerConfig {
    pub enable_gdb_stub: bool,
    pub gdb_port: u16,
    pub enable_profiler: bool,
    pub profiling_sample_interval_us: u64,
    pub enable_logging: bool,
    pub log_level: LogLevel,
    pub enable_snapshot_debugging: bool,
    pub snapshot_interval_instructions: u64,
}
```

### 主要组件

- **`GdbStub`**: GDB远程调试服务器
- **`Profiler`**: 性能分析器
- **`Logger`**: 结构化日志系统
- **`SnapshotManager`**: 快照管理器
- **`BreakpointManager`**: 断点管理器

## 🎨 设计特点

### 1. 非侵入式

调试功能对VM性能影响最小：

```rust
#[cfg(debug_assertions)]
fn debug_trace(...) {
    // 仅在debug模式编译
}
```

### 2. 可组合

调试功能可以独立启用或禁用：

```rust
let config = DebuggerConfig {
    enable_gdb_stub: true,
    enable_profiler: false,  // 不启用性能分析
    enable_logging: true,
    ..
};
```

### 3. 零开销

未启用的调试功能在编译时完全移除：

```rust
if config.enable_profiler {
    profiler.sample();
}
// 编译器会优化掉整个if块
```

## 📚 相关文档

- [vm-core](../vm-core/README.md) - 核心VM功能
- [vm-engine](../vm-engine/README.md) - 执行引擎
- [vm-cli](../vm-cli/README.md) - 命令行工具（`--debug`选项）
- [MASTER_DOCUMENTATION_INDEX](../MASTER_DOCUMENTATION_INDEX.md) - 完整文档索引

## 🔨 开发指南

### 添加新的调试功能

1. 在`vm-debug/src/lib.rs`中定义新功能
2. 更新`DebuggerConfig`添加配置选项
3. 实现调试逻辑
4. 添加文档和测试
5. 更新本README

### 集成GDB命令

1. 在GdbStub中添加命令处理器
2. 实现命令逻辑
3. 更新GDB协议文档
4. 测试命令功能

## 🧪 测试

```bash
# 运行vm-debug测试
cargo test --package vm-debug

# 测试GDB协议
cargo test --package vm-debug test_gdb_protocol

# 测试性能分析器
cargo test --package vm-debug test_profiler
```

## ⚠️ 注意事项

1. **性能影响**: 调试功能可能显著降低VM性能
2. **内存使用**: 性能分析和日志记录会消耗额外内存
3. **GDB兼容性**: 某些高级GDB功能可能不支持
4. **线程安全**: 调试器在多线程环境下需要额外注意

## 📊 性能影响

| 调试功能 | 性能影响 | 内存影响 |
|---------|----------|----------|
| GDB存根 | 5-10% | +1MB |
| 性能分析器 | 10-20% | +10-50MB |
| 日志记录 | 5-15% | +5-20MB |
| 快照调试 | 最小 | +100MB/snapshot |

## 🔗 调试工具集成

### GDB集成

```bash
# 标准GDB工作流
gdb vmlinux
(gdb) target remote :1234
(gdb) load
(gdb) break main
(gdb) continue
```

### LLDB集成 (macOS/iOS)

```bash
lldb vmlinux
(lldb) gdb-remote 1234
(lldb) b main
(lldb) c
```

### VS Code集成

`.vscode/launch.json`:
```json
{
    "type": "gdb",
    "request": "attach",
    "name": "Attach to VM",
    "executable": "vmlinux",
    "target": ":1234",
    "remote": true
}
```

## 🤝 贡献指南

如果您想改进vm-debug：

1. 确保新功能支持GDB/LLDB标准
2. 添加完整的测试用例
3. 更新文档和示例
4. 考虑性能影响
5. 保持向后兼容

## 📝 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](../LICENSE) 文件

---

**包版本**: workspace v0.1.0
**Rust版本**: 2024 Edition
**最后更新**: 2026-01-07
