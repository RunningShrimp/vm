# vm-osal

**VM项目操作系统抽象层**

[![Rust](https://img.shields.io/badge/rust-2024%20Edition-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## 概述

`vm-osal` (Operating System Abstraction Layer) 是VM项目的操作系统抽象层，提供跨平台的系统级抽象，包括内存屏障、线程管理、信号处理、平台检测等功能。它屏蔽了不同操作系统之间的差异，使VM核心代码可以在Linux、macOS、Windows、Android、iOS、HarmonyOS等平台上无缝运行。

## 🎯 核心功能

- **内存屏障**: 跨平台的内存屏障和原子操作
- **平台检测**: 运行时操作系统和架构检测
- **线程管理**: 线程亲和性、线程优先级
- **内存映射**: 跨平台的内存映射和管理
- **信号处理**: 统一的信号处理接口
- **系统调用**: 抽象的系统调用接口

## 📦 主要组件

### 1. 内存屏障 (Memory Barriers)

提供跨平台的内存屏障操作：

```rust
use vm_osal::{barrier_acquire, barrier_release, barrier_full};

// 获取屏障（读操作）
barrier_acquire();

// 释放屏障（写操作）
barrier_release();

// 完全屏障（读写操作）
barrier_full();
```

**实现细节**:
- 使用Rust的`std::sync::atomic::fence`
- 保证跨平台的内存顺序语义
- 零成本抽象（编译为原生CPU指令）

### 2. 平台检测 (Platform Detection)

运行时检测主机操作系统和架构：

```rust
use vm_osal::{host_os, host_arch};

// 检测操作系统
let os = host_os();
match os {
    "linux" => println!("Running on Linux"),
    "macos" => println!("Running on macOS"),
    "windows" => println!("Running on Windows"),
    "harmonyos" => println!("Running on HarmonyOS"),
    "android" => println!("Running on Android"),
    "ios" => println!("Running on iOS"),
    _ => println!("Unknown OS"),
}

// 检测CPU架构
let arch = host_arch();
match arch {
    "x86_64" => println!("x86_64 architecture"),
    "aarch64" => println!("ARM64 architecture"),
    "riscv64" => println!("RISC-V 64-bit"),
    _ => println!("Unknown architecture"),
}
```

**支持的操作系统**:
- ✅ Linux (包括HarmonyOS)
- ✅ macOS
- ✅ Windows
- ✅ Android
- ✅ iOS/tvOS

**支持的架构**:
- ✅ x86_64
- ✅ ARM64 (aarch64)
- ✅ RISC-V 64-bit

### 3. 线程管理 (Thread Management)

提供跨平台的线程管理功能：

```rust
use vm_osal::{set_thread_affinity, set_thread_priority};

// 设置线程亲和性（绑定到特定CPU核心）
set_thread_affinity(thread_id, &[0, 1])?;

// 设置线程优先级
set_thread_priority(thread_id, ThreadPriority::High)?;
```

**线程优先级**:
- `Low` - 低优先级
- `Normal` - 正常优先级（默认）
- `High` - 高优先级
- `Realtime` - 实时优先级（需要特权）

### 4. 内存映射 (Memory Mapping)

跨平台的内存映射和管理：

```rust
use vm_osal::{mmap, mprotect, munmap};

// 映射匿名内存
let ptr = mmap(
    None,
    4096,
    ProtectionFlags::READ | ProtectionFlags::WRITE,
    MapFlags::PRIVATE | MapFlags::ANONYMOUS,
    -1,
    0,
)?;

// 修改内存保护
mprotect(ptr, 4096, ProtectionFlags::READ)?;

// 解除映射
munmap(ptr, 4096)?;
```

**保护标志**:
- `READ` - 可读
- `WRITE` - 可写
- `EXEC` - 可执行

**映射标志**:
- `SHARED` - 共享映射
- `PRIVATE` - 私有映射（写时复制）
- `ANONYMOUS` - 匿名映射（不关联文件）
- `FIXED` - 固定地址映射

## 🔧 依赖关系

vm-osal是无依赖的纯Rust实现，仅使用标准库：

```toml
[dependencies]
# 无外部依赖
```

## 🚀 使用场景

### 场景1: 跨平台VM启动

```rust
use vm_osal::{host_os, host_arch};

pub fn detect_platform_capabilities() -> PlatformCaps {
    let os = host_os();
    let arch = host_arch();

    PlatformCaps {
        os: os.to_string(),
        arch: arch.to_string(),
        has_kvm: os == "linux",
        has_hvf: os == "macos" || os == "ios",
        has_whpx: os == "windows",
        supports_accel: matches!(arch, "x86_64" | "aarch64"),
    }
}
```

### 场景2: 内存屏障保证

```rust
use vm_osal::{barrier_acquire, barrier_release};

pub struct SharedBuffer {
    data: Vec<u8>,
    ready: AtomicBool,
}

impl SharedBuffer {
    pub fn write(&mut self, data: &[u8]) {
        self.data.copy_from_slice(data);
        barrier_release(); // 确保写入完成
        self.ready.store(true, Ordering::Release);
    }

    pub fn read(&self) -> Vec<u8> {
        while !self.ready.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        barrier_acquire(); // 确保读取到最新数据
        self.data.clone()
    }
}
```

### 场景3: 线程亲和性优化

```rust
use vm_osal::set_thread_affinity;

pub fn optimize_vcpu_threads(num_vcpus: u32) -> Result<(), vm_core::VmError> {
    for vcpu_id in 0..num_vcpus {
        let thread = get_vcpu_thread(vcpu_id)?;

        // 绑定vCPU线程到物理CPU核心
        let core_id = (vcpu_id as usize) % num_cpus::get();
        set_thread_affinity(thread.id(), &[core_id])?;
    }
    Ok(())
}
```

## 📝 API概览

### 平台检测函数

```rust
/// 获取主机操作系统
pub fn host_os() -> &'static str;

/// 获取主机CPU架构
pub fn host_arch() -> &'static str;

/// 检测是否为HarmonyOS
pub fn is_harmonyos() -> bool;
```

### 内存屏障函数

```rust
/// 获取屏障（读操作）
pub fn barrier_acquire();

/// 释放屏障（写操作）
pub fn barrier_release();

/// 完全屏障
pub fn barrier_full();
```

### 线程管理函数

```rust
/// 设置线程亲和性
pub fn set_thread_affinity(thread_id: ThreadId, cores: &[usize]) -> Result<(), Error>;

/// 设置线程优先级
pub fn set_thread_priority(thread_id: ThreadId, priority: ThreadPriority) -> Result<(), Error>;
```

### 内存映射函数

```rust
/// 映射内存区域
pub fn mmap(
    addr: Option<usize>,
    size: usize,
    prot: ProtectionFlags,
    flags: MapFlags,
    fd: i32,
    offset: i64,
) -> Result<*mut u8, Error>;

/// 解除内存映射
pub fn munmap(ptr: *mut u8, size: usize) -> Result<(), Error>;

/// 修改内存保护
pub fn mprotect(ptr: *mut u8, size: usize, prot: ProtectionFlags) -> Result<(), Error>;
```

## 🎨 设计特点

### 1. 零成本抽象

所有抽象都编译为原生系统调用或CPU指令：

```rust
// 编译为mfence指令
barrier_full();

// 编译为sched_setaffinity系统调用
set_thread_affinity(id, cores)?;
```

### 2. 编译时平台选择

使用`cfg`属性在编译时选择正确的实现：

```rust
#[cfg(target_os = "linux")]
fn platform_specific_impl() {
    // Linux特定实现
}

#[cfg(target_os = "macos")]
fn platform_specific_impl() {
    // macOS特定实现
}
```

### 3. 类型安全

利用Rust的类型系统确保正确使用：

```rust
pub struct ProtectionFlags { /* ... */ }

impl ProtectionFlags {
    pub const READ: Self = Self { bits: 1 };
    pub const WRITE: Self = Self { bits: 2 };
    pub const EXEC: Self = Self { bits: 4 };
}
```

## 📚 相关文档

- [vm-core](../vm-core/README.md) - 核心VM功能
- [vm-accel](../vm-accel/README.md) - 硬件加速（使用OSAL进行平台检测）
- [vm-engine](../vm-engine/README.md) - 执行引擎
- [MASTER_DOCUMENTATION_INDEX](../MASTER_DOCUMENTATION_INDEX.md) - 完整文档索引

## 🔨 开发指南

### 添加新平台支持

1. 在`host_os()`函数中添加新的`cfg`分支
2. 实现平台特定的内存映射和线程管理
3. 添加平台测试
4. 更新本README

### 添加新架构支持

1. 在`host_arch()`函数中添加检测逻辑
2. 确保内存屏障正确实现
3. 测试原子操作语义
4. 更新文档

## ⚠️ 注意事项

1. **内存屏障**: 正确使用内存屏障对多线程程序至关重要
2. **线程亲和性**: 需要适当的权限才能设置线程亲和性
3. **内存映射**: 使用后务必解除映射，避免内存泄漏
4. **平台差异**: 某些功能在不同平台上的行为可能不同

## 🧪 测试

```bash
# 运行vm-osal测试
cargo test --package vm-osal

# 测试特定平台功能
cargo test --package vm-osal test_platform_detection
cargo test --package vm-osal test_memory_barriers
```

## 📊 性能特性

| 操作 | 性能 | 说明 |
|------|------|------|
| 内存屏障 | < 5ns | 原子CPU指令 |
| 平台检测 | < 1μs | 编译时常量 |
| 线程亲和性 | ~10μs | 系统调用 |
| 内存映射 | ~100μs | 系统调用 |

## 🤝 贡献指南

如果您想改进vm-osal：

1. 确保新功能支持所有主要平台
2. 添加平台特定测试
3. 使用`cfg`属性进行条件编译
4. 保持零成本抽象原则
5. 更新文档和示例

## 📝 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](../LICENSE) 文件

---

**包版本**: workspace v0.1.0
**Rust版本**: 2024 Edition
**最后更新**: 2026-01-07
