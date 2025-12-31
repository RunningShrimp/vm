# VM API 文档索引

## 概述

这是VM项目API参考文档的完整索引。本文档提供了所有核心API模块的导航和快速访问。

## 核心API模块

### [vm-core API](./api/VmCore.md)

虚拟机的核心库，提供基础类型定义、Trait抽象和基础设施。

**主要内容**:
- 地址类型（GuestAddr、GuestPhysAddr、HostAddr）
- 架构支持（GuestArch）
- 执行抽象（ExecutionEngine、Decoder）
- 内存管理Trait（MMU、MemoryAccess）
- 错误类型系统
- 事件系统

**适用对象**:
- 所有VM组件开发者
- 需要理解VM基础类型的用户
- 实现自定义执行引擎的开发者

**关键类型**:
- `GuestAddr` - 虚拟地址
- `GuestPhysAddr` - 物理地址
- `VmConfig` - VM配置
- `ExecMode` - 执行模式
- `Fault` - 故障类型

---

### [vm-interface API](./api/VmInterface.md)

VM各组件的统一接口规范，遵循SOLID原则。

**主要内容**:
- 组件生命周期管理（VmComponent）
- 配置管理（Configurable）
- 状态观察（Observable）
- 执行引擎接口（ExecutionEngine）
- 内存管理接口（MemoryManager）
- 设备管理接口（DeviceManager）

**适用对象**:
- 实现VM组件的开发者
- 需要理解组件生命周期的用户
- 实现自定义设备的开发者

**关键Trait**:
- `VmComponent` - 组件基础
- `Configurable` - 配置管理
- `Observable` - 状态订阅
- `ExecutionEngine` - 执行引擎
- `MemoryManager` - 内存管理
- `Device` - 设备接口

---

### [vm-mem API](./api/VmMemory.md)

内存管理实现，包括物理内存、MMU、TLB等。

**主要内容**:
- 物理内存后端（PhysicalMemory）
- 软件MMU（SoftMmu）
- TLB优化（ITLB/DTLB）
- 页表遍历（RISC-V SV39/SV48）
- MMIO支持
- 原子操作（LR/SC）

**适用对象**:
- 实现内存管理的开发者
- 需要理解地址翻译的用户
- 性能优化开发者

**关键类型**:
- `PhysicalMemory` - 物理内存
- `SoftMmu` - 软件MMU
- `PagingMode` - 分页模式
- `PageTableBuilder` - 页表构建

**性能特点**:
- 分片RwLock设计
- TLB缓存优化
- 大页支持

---

### [vm-engine API](./api/VmEngine.md)

执行引擎实现，包括解释器和JIT编译器。

**主要内容**:
- JIT编译器（JITCompiler）
- 解释器（Interpreter）
- 异步执行上下文（AsyncExecutionContext）
- 执行控制

**适用对象**:
- 实现自定义执行引擎的开发者
- 性能优化开发者
- 需要理解执行模型的用户

**关键类型**:
- `JITCompiler` - JIT编译器
- `JITConfig` - JIT配置
- `Interpreter` - 解释器
- `ExecutorType` - 执行器类型
- `AsyncExecutionContext` - 异步上下文

**性能对比**:
- 解释器：1-5%原生性能
- JIT：50-80%原生性能
- 混合模式：自动平衡

---

### [InstructionSet API](./api/InstructionSet.md)

多架构指令前端，支持RISC-V、ARM64、x86_64。

**主要内容**:
- RISC-V64解码器
- ARM64解码器
- x86_64解码器
- 指令反汇编

**适用对象**:
- 实现新架构支持的开发者
- 需要理解指令编码的用户
- 调试工具开发者

**关键类型**:
- `RiscvDecoder` - RISC-V解码器
- `RiscvInstruction` - RISC-V指令
- `Arm64Decoder` - ARM64解码器
- `X86Decoder` - x86解码器
- `X86Instruction` - x86指令

**支持架构**:
- RISC-V64（RV64I/M/A/F/D/C）
- ARM64（AArch64）
- x86_64（AMD64）

---

### [Devices API](./api/Devices.md)

设备虚拟化实现，包括VirtIO设备、中断控制器等。

**主要内容**:
- VirtIO设备（块、网络、控制台等）
- 中断控制器（CLINT、PLIC）
- GPU虚拟化
- 零拷贝I/O
- 异步I/O

**适用对象**:
- 实现VirtIO设备的开发者
- 设备驱动开发者
- 需要理解设备模型的用户

**关键类型**:
- `VirtioBlock` - 块设备
- `VirtioNet` - 网络设备
- `CLINT` - 核心本地中断控制器
- `PLIC` - 平台级中断控制器
- `ZeroCopyIoManager` - 零拷贝I/O管理器

**设备类别**:
- 存储设备（块、SCSI）
- 网络设备（virtio-net、vhost-net）
- 输入设备（键盘、鼠标）
- GPU设备（2D/3D加速）

---

## 按功能分类

### 类型系统

- **地址类型**: [VmCore.md](./api/VmCore.md) - GuestAddr、GuestPhysAddr、HostAddr
- **配置类型**: [VmCore.md](./api/VmCore.md) - VmConfig、ExecMode
- **错误类型**: [VmCore.md](./api/VmCore.md) - VmError、Fault

### 执行相关

- **执行引擎**:
  - 接口: [VmInterface.md](./api/VmInterface.md) - ExecutionEngine trait
  - 实现: [VmEngine.md](./api/VmEngine.md) - JIT、Interpreter
- **指令解码**: [InstructionSet.md](./api/InstructionSet.md) - 多架构解码器
- **执行控制**: [VmEngine.md](./api/VmEngine.md) - 执行上下文

### 内存相关

- **内存管理**:
  - 接口: [VmInterface.md](./api/VmInterface.md) - MemoryManager trait
  - 实现: [VmMemory.md](./api/VmMemory.md) - PhysicalMemory、SoftMmu
- **地址翻译**: [VmMemory.md](./api/VmMemory.md) - TLB、页表遍历
- **MMIO**: [VmMemory.md](./api/VmMemory.md) - MMIO支持

### 设备相关

- **设备接口**: [VmInterface.md](./api/VmInterface.md) - Device trait
- **VirtIO设备**: [Devices.md](./api/Devices.md) - 块、网络、控制台等
- **中断控制**: [Devices.md](./api/Devices.md) - CLINT、PLIC
- **GPU**: [Devices.md](./api/Devices.md) - VirGL、直通

### 组件框架

- **生命周期**: [VmInterface.md](./api/VmInterface.md) - VmComponent
- **配置管理**: [VmInterface.md](./api/VmInterface.md) - Configurable
- **状态观察**: [VmInterface.md](./api/VmInterface.md) - Observable

## 按使用场景

### 快速开始

1. **了解基础类型**: [VmCore.md](./api/VmCore.md)
2. **创建VM配置**: [VmCore.md](./api/VmCore.md) - VmConfig
3. **设置内存**: [VmMemory.md](./api/VmMemory.md) - SoftMmu
4. **选择执行引擎**: [VmEngine.md](./api/VmEngine.md)

### 组件开发

1. **实现组件接口**: [VmInterface.md](./api/VmInterface.md) - VmComponent
2. **理解执行模型**: [VmCore.md](./api/VmCore.md) - ExecutionEngine
3. **实现内存管理**: [VmInterface.md](./api/VmInterface.md) - MemoryManager

### 设备开发

1. **实现设备接口**: [VmInterface.md](./api/VmInterface.md) - Device
2. **参考VirtIO**: [Devices.md](./api/Devices.md) - VirtioBlock等
3. **理解MMIO**: [VmMemory.md](./api/VmMemory.md) - MMIO支持

### 性能优化

1. **执行引擎**: [VmEngine.md](./api/VmEngine.md) - JIT配置
2. **内存优化**: [VmMemory.md](./api/VmMemory.md) - TLB、大页
3. **零拷贝I/O**: [Devices.md](./api/Devices.md) - DMA、零拷贝

### 架构扩展

1. **添加新架构**: [InstructionSet.md](./api/InstructionSet.md)
2. **实现解码器**: [VmCore.md](./api/VmCore.md) - Decoder trait
3. **指令执行**: [VmEngine.md](./api/VmEngine.md) - ExecutionEngine

## 快速参考

### 常用类型

| 类型 | 描述 | 文档 |
|------|------|------|
| `GuestAddr` | 虚拟地址 | [VmCore](./api/VmCore.md#guestaddr) |
| `GuestPhysAddr` | 物理地址 | [VmCore](./api/VmCore.md#guestphysaddr) |
| `VmConfig` | VM配置 | [VmCore](./api/VmCore.md#vmconfig) |
| `SoftMmu` | 软件MMU | [VmMemory](./api/VmMemory.md#softmmu) |
| `PhysicalMemory` | 物理内存 | [VmMemory](./api/VmMemory.md#physicalmemory) |
| `JITCompiler` | JIT编译器 | [VmEngine](./api/VmEngine.md#jitcompiler) |
| `VirtioBlock` | 块设备 | [Devices](./api/Devices.md#virtioblock-块设备) |

### 常用Trait

| Trait | 描述 | 文档 |
|------|------|------|
| `VmComponent` | 组件基础 | [VmInterface](./api/VmInterface.md#vmcomponent) |
| `ExecutionEngine` | 执行引擎 | [VmCore](./api/VmCore.md#executionengine) |
| `MemoryAccess` | 内存访问 | [VmCore](./api/VmCore.md#mmu) |
| `Decoder` | 指令解码 | [VmCore](./api/VmCore.md#decoder) |
| `Device` | 设备接口 | [VmInterface](./api/VmInterface.md#device) |

## Rust Doc

除了这些Markdown文档，还提供完整的Rust Doc：

```bash
# 生成文档
cargo doc --workspace --no-deps

# 生成并打开文档
cargo doc --workspace --no-deps --open

# 仅包含公共API
cargo doc --workspace --no-deps --document-private-items
```

生成的文档位于:
- `target/doc/vm_core/index.html` - vm-core文档
- `target/doc/vm_interface/index.html` - vm-interface文档
- `target/doc/vm_mem/index.html` - vm-mem文档
- `target/doc/vm_engine/index.html` - vm-engine文档
- `target/doc/vm_frontend/index.html` - vm-frontend文档
- `target/doc/vm_device/index.html` - vm-device文档

## 贡献指南

### 文档规范

所有公共API应该包含：
1. **模块级文档** - `//!` 注释
2. **类型文档** - `///` 注释
3. **方法文档** - 包含参数、返回值、错误
4. **示例代码** - 可运行的示例
5. **性能说明** - 时间/空间复杂度

### 示例模板

```rust
//! 模块级文档
//!
/// 类型文档
///
/// # Examples
/// ```
/// let example = Type::new();
/// ```
pub struct Type {
    /// 字段文档
    pub field: usize,
}

impl Type {
    /// 方法文档
    ///
    /// # Errors
    ///
    /// 返回错误当...
    pub fn method(&self) -> Result<()> {
        Ok(())
    }
}
```

## 相关资源

- [主README](../README.md) - 项目概述
- [架构文档](./architecture.md) - 系统架构
- [API指南](./api_guide.md) - API使用指南
- [开发指南](./development/) - 开发相关文档

## 反馈与贡献

如果你发现文档问题或有改进建议：
- 提交Issue描述问题
- 提交PR改进文档
- 参与讨论完善API设计

---

**最后更新**: 2025-12-31
**文档版本**: 1.0.0
