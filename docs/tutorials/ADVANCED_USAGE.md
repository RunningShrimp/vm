# VM高级用法指南

本指南深入介绍VM项目的高级特性和用法。

## 目录

- [JIT编译](#jit编译)
- [自定义设备](#自定义设备)
- [内存管理](#内存管理)
- [性能优化](#性能优化)
- [多线程执行](#多线程执行)
- [跨架构支持](#跨架构支持)
- [调试和剖析](#调试和剖析)
- [安全特性](#安全特性)

## JIT编译

### 启用JIT

```rust
use vm_engine::EngineConfig;

let config = EngineConfig {
    enable_jit: true,
    jit_threshold: 100,        // 执行100次后编译
    optimization_level: 2,     // 优化级别
    ..Default::default()
};

let engine = ExecutionEngine::new(arch, mmu, config)?;
```

### JIT配置选项

#### 编译阈值

```rust
// 保守策略 - 等待更长时间
EngineConfig {
    jit_threshold: 1000,
}

// 激进策略 - 快速编译
EngineConfig {
    jit_threshold: 10,
}
```

#### 优化级别

```rust
// 级别0: 无优化 - 快速编译
optimization_level: 0

// 级别1: 基础优化
optimization_level: 1

// 级别2: 标准优化 - 推荐
optimization_level: 2

// 级别3: 激进优化 - 最慢但最快
optimization_level: 3
```

### JIT缓存管理

```rust
use vm_engine::jit::CodeCache;

let mut cache = CodeCache::new(64 * 1024 * 1024); // 64MB

// 手动预编译
cache.compile_block(addr, &instructions)?;

// 查询编译状态
if let Some(block) = cache.get(addr) {
    println!("已编译, 执行次数: {}", block.execution_count());
}

// 清除缓存
cache.clear();
```

### 分层编译

```rust
use vm_engine::jit::TieredCompiler;

let mut tiered = TieredCompiler::new();

// 第一层: 快速生成无优化代码
tiered.compile_at_level(addr, 0);

// 第二层: 优化代码
if execution_count > 1000 {
    tiered.compile_at_level(addr, 2);
}
```

## 自定义设备

### 设备接口

```rust
use vm_device::{Device, DeviceBus, IoResult};

pub struct MyDevice {
    base_address: u64,
    // 设备状态
}

impl Device for MyDevice {
    fn read(&mut self, offset: u64, size: u8) -> IoResult<u64> {
        match offset {
            0x00 => Ok(self.register_a),
            0x08 => Ok(self.register_b),
            _ => Err(IoError::InvalidOffset(offset)),
        }
    }

    fn write(&mut self, offset: u64, value: u64, size: u8) -> IoResult<()> {
        match offset {
            0x00 => {
                self.register_a = value;
                Ok(())
            }
            _ => Err(IoError::InvalidOffset(offset)),
        }
    }

    fn irq(&self) -> bool {
        self.irq_pending
    }
}
```

### 注册设备

```rust
use vm_device::DeviceBus;

let mut bus = DeviceBus::new();
let device = MyDevice::new(0x10000);

bus.register_device(0x10000, 0x1000, Box::new(device))?;
```

### MMIO拦截

```rust
use vm_engine::MmioCallback;

let callback = MmioCallback::new(
    |addr: u64, size: u8| -> Result<u64> {
        // 处理读
        Ok(0)
    },
    |addr: u64, value: u64, size: u8| -> Result<()> {
        // 处理写
        Ok(())
    }
);

engine.set_mmio_callback(0x10000, 0x1000, callback)?;
```

### DMA实现

```rust
pub struct DmaDevice {
    src_addr: u64,
    dst_addr: u64,
    length: usize,
}

impl DmaDevice {
    pub fn transfer(&mut self, mmu: &mut SoftMmu) -> Result<()> {
        for i in 0..self.length {
            let value = mmu.read(self.src_addr + i as u64, 8)?;
            mmu.write(self.dst_addr + i as u64, value, 8)?;
        }
        Ok(())
    }
}
```

## 内存管理

### 内存区域

```rust
use vm_mem::{MemoryRegion, MemRegionFlags};

// 代码区域 - 只读可执行
let code_region = MemoryRegion {
    base: 0x1000,
    size: 0x1000,
    flags: MemRegionFlags::READ | MemRegionFlags::EXEC,
};

// 数据区域 - 可读可写
let data_region = MemoryRegion {
    base: 0x10000,
    size: 0x10000,
    flags: MemRegionFlags::READ | MemRegionFlags::WRITE,
};

// 设备区域 - 可读写
let device_region = MemoryRegion {
    base: 0x20000,
    size: 0x1000,
    flags: MemRegionFlags::READ | MemRegionFlags::WRITE | MemRegionFlags::IO,
};

mmu.add_region(code_region)?;
mmu.add_region(data_region)?;
mmu.add_region(device_region)?;
```

### 内存映射

```rust
// 映射物理内存到虚拟地址
mmu.map_page(
    0x1000,      // 虚拟地址
    0x5000,      // 物理地址
    MemRegionFlags::READ | MemRegionFlags::WRITE
)?;

// 取消映射
mmu.unmap_page(0x1000)?;
```

### 大页支持

```rust
use vm_mem::PageSize;

// 2MB大页
mmu.map_page(
    0x200000,
    0x400000,
    MemRegionFlags::READ | MemRegionFlags::WRITE
)?;

// 使用大页
mmu.set_page_size(PageSize::Huge2MB);
```

### NUMA优化

```rust
use vm_accel::NumaAllocator;

// 绑定到特定NUMA节点
let numa = NumaAllocator::bind_to_node(0)?;

let mut mmu = SoftMmu::with_allocator(numa);
mmu.add_region(region)?;
```

## 性能优化

### 热点检测

```rust
use vm_engine::Profiler;

let mut profiler = Profiler::new();

// 运行并收集统计
for _ in 0..10000 {
    engine.execute_step()?;
}

// 获取热点
let hotspots = profiler.hotspots();
for (addr, count) in hotspots.iter().take(10) {
    println!("0x{:x}: {} 次执行", addr, count);
}
```

### 批量执行

```rust
// 执行N条指令而不检查
engine.execute_batch(1000)?;

// 检查是否完成
if engine.is_halted() {
    println!("执行完成");
}
```

### 预编译

```rust
// AOT编译
use vm_engine::AotCompiler;

let mut aot = AotCompiler::new();

// 编译整个程序
let binary = aot.compile_program(&program)?;

// 保存到文件
aot.save_to_file("program.aot")?;

// 加载AOT代码
let engine = ExecutionEngine::load_aot("program.aot")?;
```

### 寄存器优化

```rust
// 窥孔优化
let mut optimizer = PeepholeOptimizer::new();

let optimized = optimizer.optimize(&instructions)?;
```

## 多线程执行

### 并行执行

```rust
use vm_engine::ParallelExecutor;

let executor = ParallelExecutor::new(4)?; // 4个线程

let threads = vec![
    ThreadConfig { pc: 0x1000, stack: 0x10000 },
    ThreadConfig { pc: 0x2000, stack: 0x20000 },
    ThreadConfig { pc: 0x3000, stack: 0x30000 },
    ThreadConfig { pc: 0x4000, stack: 0x40000 },
];

executor.execute_parallel(threads)?;
```

### 同步原语

```rust
use vm_core::synchronization::{Mutex, Condvar};

// 创建互斥锁
let mutex = Mutex::new(0);

// 加锁
let guard = mutex.lock();
*guard += 1;
drop(guard);

// 条件变量
let condvar = Condvar::new();
condvar.wait(&mut mutex);
```

### 原子操作

```rust
use std::sync::atomic::{AtomicU64, Ordering};

let counter = AtomicU64::new(0);

// 原子递增
counter.fetch_add(1, Ordering::SeqCst);

// 原子比较交换
counter.compare_exchange(
    old,
    new,
    Ordering::SeqCst,
    Ordering::SeqCst
)?;
```

## 跨架构支持

### 运行时转换

```rust
use vm_cross_arch::{CrossArchRuntime, GuestArch, HostArch};

// 检测主机架构
let host = HostArch::detect();

// 创建x86_64 guest on ARM64 host
let runtime = CrossArchRuntime::new(
    GuestArch::X86_64,
    HostArch::Arm64
)?;

// 执行
runtime.execute(0x1000)?;
```

### 二进制翻译

```rust
use vm_cross_arch::BinaryTranslator;

let mut translator = BinaryTranslator::new(
    GuestArch::X86_64,
    HostArch::Arm64
);

// 翻译基本块
let translated = translator.translate_block(&x86_instructions)?;

// 执行翻译后的代码
engine.execute_native(&translated)?;
```

### 热点重编译

```rust
// 动态检测架构不匹配
if engine.inefficient_arch() {
    let new_arch = find_optimal_arch()?;

    // 重新编译为更优的架构
    engine.recompile_for(new_arch)?;
}
```

## 调试和剖析

### 断点

```rust
use vm_engine::debugger::Breakpoint;

let mut debugger = engine.debugger();

// 设置断点
debugger.set_breakpoint(0x1000)?;

// 继续执行
loop {
    match engine.execute_step()? {
        ExecutionResult::Breakpoint => {
            println!("命中断点: 0x{:x}", engine.pc());
            // 检查状态
        }
        ExecutionResult::Halted => break,
        _ => {}
    }
}
```

### 单步执行

```rust
// 启用单步模式
engine.set_single_step(true);

for _ in 0..100 {
    engine.execute_step()?;

    let pc = engine.pc();
    let instr = engine.current_instruction()?;
    println!("0x{:x}: {}", pc, instr);
}
```

### 内存监视

```rust
use vm_engine::debugger::Watchpoint;

// 监视内存写入
debugger.set_watchpoint(
    0x10000,
    Watchpoint::Write
)?;

// 执行时会触发
match engine.execute_step()? {
    ExecutionResult::Watchpoint => {
        println!("内存地址 0x10000 被修改");
    }
    _ => {}
}
```

### 性能剖析

```rust
use vm_engine::Profiler;

let mut profiler = Profiler::new();

// 开始剖析
profiler.start();

// 执行程序
for _ in 0..10000 {
    engine.execute_step()?;
}

// 停止剖析
profiler.stop();

// 生成报告
let report = profiler.report();
println!("总指令数: {}", report.total_instructions);
println!("执行时间: {:?}", report.total_time);
println!("每秒指令数: {}", report.instructions_per_second());

// 按类别统计
for (category, count) in report.by_category() {
    println!("{}: {}", category, count);
}
```

### 火焰图

```rust
// 生成火焰图数据
let flame_data = profiler.flamegraph()?;

// 保存到文件
std::fs::write("flamegraph.svg", flame_data)?;
```

## 安全特性

### 内存隔离

```rust
use vm_core::Sandbox;

let sandbox = Sandbox::new();

// 沙箱执行
let result = sandbox.execute(|| {
    engine.execute()
})?;

// 沙箱外的内存访问会被阻止
```

### 系统调用过滤

```rust
use vm_core::syscall::SyscallFilter;

let filter = SyscallFilter::whitelist(&[
    SyscallNumber::Read,
    SyscallNumber::Write,
    SyscallNumber::Exit,
]);

engine.set_syscall_filter(filter)?;
```

### 资源限制

```rust
use vm_core::ResourceLimits;

let limits = ResourceLimits {
    max_instructions: 1_000_000,
    max_memory: 1024 * 1024,
    max_time: Duration::from_secs(10),
};

engine.set_resource_limits(limits)?;
```

### SELinux集成

```rust
use vm_core::selinux::SelinuxContext;

// 设置SELinux上下文
let ctx = SelinuxContext::new("system_u:object_r:vm_exec_t:s0")?;
engine.set_selinux_context(ctx)?;
```

## 高级示例

### 实现JIT编译器

```rust
use vm_engine::jit::{JitCompiler, CompiledBlock};

pub struct MyJitCompiler {
    // 编译器状态
}

impl JitCompiler for MyJitCompiler {
    fn compile_block(&mut self, instructions: &[Instruction]) -> Result<CompiledBlock> {
        let mut code = Vec::new();

        for instr in instructions {
            // 生成机器码
            self.emit_instruction(&mut code, instr)?;
        }

        Ok(CompiledBlock {
            code,
            size: code.len(),
            // ...
        })
    }
}
```

### 自定义内存管理器

```rust
use vm_mem::MemoryManager;

pub struct CustomAllocator {
    // 自定义分配逻辑
}

impl MemoryManager for CustomAllocator {
    fn allocate(&mut self, size: usize) -> Result<u64> {
        // 自定义分配
    }

    fn deallocate(&mut self, addr: u64) -> Result<()> {
        // 自定义释放
    }
}
```

### 插件系统

```rust
use vm_plugin::{Plugin, PluginManager};

pub struct MyPlugin {
    // 插件状态
}

impl Plugin for MyPlugin {
    fn name(&self) -> &str {
        "my_plugin"
    }

    fn on_instruction(&mut self, instr: &Instruction) {
        // 指令回调
    }
}

// 注册插件
let mut manager = PluginManager::new();
manager.load(Box::new(MyPlugin::new()))?;
```

## 最佳实践

### 1. 错误处理

```rust
use anyhow::Result;

fn execute_program() -> Result<()> {
    engine.execute_step()
        .map_err(|e| anyhow::anyhow!("执行失败: {}", e))?;

    Ok(())
}
```

### 2. 日志记录

```rust
use log::{debug, info, warn};

debug!("PC: 0x{:x}", engine.pc());
info!("程序执行完成");
warn!("检测到异常: {:?}", exception);
```

### 3. 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addition() {
        let mut engine = create_test_engine();
        engine.execute_test_program();
        assert_eq!(engine.read_register(1).unwrap(), 42);
    }
}
```

### 4. 文档

```rust
/// 执行程序直到完成或达到最大指令数
///
/// # Arguments
///
/// * `max_instructions` - 最大执行指令数
///
/// # Returns
///
/// 执行结果
///
/// # Examples
///
/// ```no_run
/// let result = engine.execute_until(1000)?;
/// ```
fn execute_until(&mut self, max_instructions: usize) -> Result<ExecutionResult> {
    // ...
}
```

## 相关资源

- [快速入门](./GETTING_STARTED.md)
- [RISC-V编程](./RISCV_PROGRAMMING.md)
- [API文档](https://docs.rs/vm)
- [示例代码](../../examples/)

---

深入探索VM的强大功能!
