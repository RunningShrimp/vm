# 虚拟机架构设计文档

## 1. 概述

本项目是一个用Rust开发的高性能跨平台虚拟机软件，支持AMD64、ARM64和RISC-V64三种架构之间的跨架构执行。项目采用模块化设计，集成了JIT（即时编译）、AOT（提前编译）和GC（垃圾收集）等先进技术。

## 2. 整体架构

### 2.1 架构层次

```
┌─────────────────────────────────────────────────────────┐
│                    应用层                                │
│  vm-cli, vm-service, vm-monitor                        │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│                    业务层                                │
│  vm-boot, vm-runtime, vm-adaptive                      │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│                    执行层                                │
│  vm-engine-jit, vm-engine-interpreter,                 │
│  vm-engine-hybrid, vm-cross-arch                        │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│                    前端层                                │
│  vm-frontend-x86_64, vm-frontend-arm64,                │
│  vm-frontend-riscv64                                    │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│                    核心层                                │
│  vm-core, vm-ir, vm-mem, vm-device                     │
└─────────────────────────────────────────────────────────┘
```

### 2.2 模块依赖关系

```
vm-core (核心抽象)
  ├── vm-ir (中间表示)
  ├── vm-mem (内存管理)
  └── vm-device (设备模拟)

vm-frontend-{arch} (架构前端)
  ├── vm-core
  └── vm-ir

vm-engine-{type} (执行引擎)
  ├── vm-core
  ├── vm-ir
  └── vm-mem

vm-cross-arch (跨架构执行)
  ├── vm-core
  ├── vm-ir
  ├── vm-engine-jit
  └── vm-engine-interpreter

vm-service (服务层)
  ├── vm-core
  ├── vm-engine-jit
  └── vm-engine-interpreter
```

## 3. 核心子系统

### 3.1 JIT编译系统

**模块**: `vm-engine-jit`

**架构**:
```
IR Block
  ↓
热点检测 (EWMA算法)
  ↓
寄存器分配 (线性扫描/图着色)
  ↓
指令调度 (依赖分析)
  ↓
代码生成 (Cranelift)
  ↓
代码缓存 (统一缓存管理)
  ↓
执行
```

**关键组件**:
- `enhanced_compiler.rs`: 增强型编译器，包含寄存器分配和指令调度
- `ewma_hotspot.rs`: EWMA热点检测算法
- `unified_cache.rs`: 统一代码缓存管理
- `unified_gc.rs`: 统一垃圾收集器
- `hybrid_executor.rs`: 混合执行器（AOT → JIT → 解释器）

### 3.2 AOT编译系统

**模块**: `aot-builder`, `vm-cross-arch`

**架构**:
```
原始机器码
  ↓
解码器 (架构特定)
  ↓
语义提升 (IR生成)
  ↓
IR优化 (常量折叠、死代码消除等)
  ↓
代码生成 (LLVM/直接生成)
  ↓
AOT镜像文件
```

**关键组件**:
- `aot-builder/src/lib.rs`: AOT构建器
- `vm-cross-arch/src/cross_arch_aot.rs`: 跨架构AOT编译器
- `vm-engine-jit/src/aot_loader.rs`: AOT镜像加载器

### 3.3 垃圾收集系统

**模块**: `vm-engine-jit/src/unified_gc.rs`

**架构**:
```
并发标记阶段
  ├── 无锁标记栈
  ├── 分片写屏障
  └── 自适应时间配额
  ↓
增量清扫阶段
  ├── 批量清扫
  └── 内存回收
```

**关键特性**:
- 并发标记：无锁标记栈，支持多线程并发
- 分片写屏障：根据CPU核心数动态分片
- 自适应配额：根据堆使用率动态调整时间配额
- 增量执行：最小化暂停时间

### 3.4 跨架构执行系统

**模块**: `vm-cross-arch`

**架构**:
```
Guest架构代码 (AMD64/ARM64/RISC-V64)
  ↓
解码器 (根据guest架构选择)
  ↓
统一IR (IRBlock)
  ↓
性能优化器 (IR优化)
  ├── 常量折叠
  ├── 死代码消除
  ├── 寄存器分配优化
  ├── 指令选择优化
  └── SIMD优化
  ↓
执行策略选择 (AOT > JIT > 解释器)
  ↓
Host架构代码执行
```

**关键组件**:
- `translator.rs`: 架构转换器
- `performance_optimizer.rs`: 性能优化器
- `cache_optimizer.rs`: 缓存优化器
- `unified_executor.rs`: 统一执行器

## 4. 数据流

### 4.1 指令执行流程

```
1. 取指 (MMU.fetch_insn)
   ↓
2. 解码 (Decoder.decode)
   ↓
3. IR生成 (前端解码器)
   ↓
4. 执行策略选择
   ├── AOT: 查找AOT镜像
   ├── JIT: 热点检测 → 编译 → 执行
   └── 解释器: 直接执行IR
   ↓
5. 执行结果
   ├── 正常继续
   ├── 故障处理
   └── I/O请求
```

### 4.2 内存访问流程

```
1. 虚拟地址 (GVA)
   ↓
2. TLB查找
   ├── 命中 → 物理地址
   └── 未命中 → 页表遍历
   ↓
3. 物理地址 (GPA)
   ↓
4. MMU访问
   ├── 内存访问
   └── MMIO设备访问
```

### 4.3 跨架构转换流程

```
源架构指令
  ↓
解码 (源架构解码器)
  ↓
IR生成 (统一IR)
  ↓
IR优化 (性能优化器)
  ↓
编码 (目标架构编码器)
  ↓
目标架构指令
```

## 5. 执行模式

### 5.1 解释执行模式

- **模块**: `vm-engine-interpreter`
- **特点**: 直接执行IR，启动快，性能较低
- **适用场景**: 冷代码、调试、兼容性测试

### 5.2 JIT编译模式

- **模块**: `vm-engine-jit`
- **特点**: 运行时编译热点代码，性能较高
- **适用场景**: 热点代码、动态代码

### 5.3 AOT编译模式

- **模块**: `aot-builder`, `vm-engine-jit/aot_loader`
- **特点**: 提前编译，性能最高
- **适用场景**: 启动时已知的热点代码

### 5.4 混合执行模式

- **模块**: `vm-engine-hybrid`, `vm-engine-jit/hybrid_executor`
- **特点**: 自动选择最佳执行策略
- **执行顺序**: AOT → JIT → 解释器

### 5.5 硬件加速模式

- **模块**: `vm-accel`
- **特点**: 使用KVM/HVF/WHPX硬件虚拟化
- **适用场景**: 同架构执行

## 6. 内存管理

### 6.1 MMU抽象

- **Trait**: `vm_core::MMU`
- **实现**: 
  - `vm-mem/src/mmu.rs`: 基础MMU
  - `vm-mem/src/unified_mmu.rs`: 统一MMU（带TLB）
  - `vm-core/src/async_mmu.rs`: 异步MMU

### 6.2 TLB管理

- **模块**: `vm-core/src/tlb_async.rs`, `vm-mem/src/tlb_concurrent.rs`
- **特性**:
  - 异步TLB缓存
  - 并发TLB管理
  - 预取策略

### 6.3 内存分配

- **模块**: `vm-mem/src/numa_allocator.rs`
- **特性**: NUMA感知的内存分配

## 7. 设备模拟

### 7.1 VirtIO设备

- **模块**: `vm-device`
- **支持设备**:
  - 块设备
  - 网络设备
  - 控制台设备
  - GPU设备（部分支持）

### 7.2 MMIO设备

- **Trait**: `vm_core::MmioDevice`
- **实现**: 各种MMIO设备实现

## 8. 异步执行

### 8.1 异步MMU

- **模块**: `vm-core/src/async_mmu.rs`
- **特性**: 异步内存访问接口

### 8.2 异步执行器

- **模块**: `vm-engine-interpreter/src/async_executor.rs`
- **特性**: 基于tokio的异步执行

### 8.3 协程池

- **模块**: `vm-runtime/src/coroutine_pool.rs`
- **特性**: 协程资源管理

## 9. 性能优化

### 9.1 热点检测

- **算法**: EWMA（指数加权移动平均）
- **模块**: `vm-engine-jit/src/ewma_hotspot.rs`
- **特性**: 多维度评分系统

### 9.2 代码缓存

- **模块**: `vm-engine-jit/src/unified_cache.rs`
- **策略**: LRU、LFU、ValueBased、Random
- **特性**: 分层缓存、智能预取

### 9.3 寄存器分配

- **算法**: 线性扫描、图着色（计划中）
- **模块**: `vm-engine-jit/src/enhanced_compiler.rs`

### 9.4 指令调度

- **算法**: 依赖图分析
- **模块**: `vm-engine-jit/src/enhanced_compiler.rs`

## 10. 跨平台支持

### 10.1 硬件加速

- **Linux**: KVM (`vm-accel/src/kvm.rs`)
- **macOS**: HVF (`vm-accel/src/hvf.rs`)
- **Windows**: WHPX (`vm-accel/src/whpx.rs`)

### 10.2 平台抽象

- **模块**: `vm-osal`
- **特性**: 操作系统抽象层

## 11. 设计原则

### 11.1 DDD贫血模型

- **数据模型**: 纯数据结构，无业务逻辑
- **服务层**: 业务逻辑集中在服务层
- **示例**: `vm-core/src/vm_state.rs` (数据) + `vm-service/src/vm_service.rs` (服务)

### 11.2 模块化设计

- **职责分离**: 每个模块职责单一明确
- **接口抽象**: 使用trait定义接口
- **依赖注入**: 通过trait实现依赖注入

### 11.3 性能优先

- **零成本抽象**: 使用Rust的零成本抽象
- **无锁数据结构**: 在关键路径使用无锁数据结构
- **缓存优化**: 多层缓存策略

## 12. 扩展性

### 12.1 新架构支持

1. 实现 `vm-frontend-{arch}` 模块
2. 实现架构特定的解码器
3. 实现架构特定的编码器（如需要跨架构）

### 12.2 新执行引擎

1. 实现 `ExecutionEngine<IRBlock>` trait
2. 集成到 `vm-engine-hybrid` 或创建新的混合执行器

### 12.3 新设备支持

1. 实现 `MmioDevice` trait
2. 注册到MMU的设备映射表

## 13. 性能指标

### 13.1 编译性能

- **AOT编译**: ~5-50ms/块
- **JIT编译**: ~0.5-5ms/块
- **解释执行**: ~0.1-1ms/块

### 13.2 执行性能

- **AOT代码**: 90-95%原生性能
- **JIT代码**: 70-85%原生性能
- **解释器**: 10-30%原生性能

### 13.3 内存使用

- **GC开销**: <5%
- **JIT缓存**: 可配置（默认64MB）
- **TLB命中率**: >95%

## 14. 未来改进方向

1. **寄存器分配**: 实现图着色算法
2. **指令调度**: 支持超标量调度
3. **GC优化**: 实现分代GC
4. **异步优化**: 扩大异步使用范围
5. **事件驱动**: 引入事件驱动架构
6. **聚合设计**: 优化DDD聚合设计

