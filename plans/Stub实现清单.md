# Stub 实现清单

## 概述

本文档记录项目中所有 stub（桩代码）和未完成实现，按模块和优先级组织。stub 是临时占位代码，需要替换为完整实现。

## Stub 分类

- 🔴 **P0 - 紧急**（阻塞核心功能）
- 🟠 **P1 - 高优先级**（影响功能完整性）
- 🟡 **P2 - 中优先级**（影响性能或体验）
- 🟢 **P3 - 低优先级**（边缘功能）

## 目录

1. [执行引擎](#执行引擎)
2. [指令解码](#指令解码)
3. [内存管理](#内存管理)
4. [JIT 编译器](#jit-编译器)
5. [设备模拟](#设备模拟)
6. [跨架构翻译](#跨架构翻译)
7. [硬件加速](#硬件加速)
8. [垃圾回收](#垃圾回收)

---

## 执行引擎

### P1 - 高优先级

#### Stub E001：统一执行器中的缓存加载（vm-core）

**文件**：`crates/core/vm-core/src/unified_executor.rs:253`

**当前代码**：
```rust
// TODO: 实际实现需要从缓存加载并执行编译后的代码
```

**描述**：
- 统一执行器缺少从代码缓存加载编译后代码的实现
- 影响执行效率和性能

**完整实现要求**：
- [ ] 从 JIT 代码缓存加载编译后的代码
- [ ] 验证代码有效性
- [ ] 执行编译后的代码
- [ ] 处理缓存未命中（回退到解释器）
- [ ] 添加性能监控

**预计工作量**：2-3 天
**依赖**：JIT 代码缓存实现
**负责人**：待分配

---

## 指令解码

### P1 - 高优先级

#### Stub I001：x86_64 16 位寻址模式（vm-service）

**文件**：`crates/runtime/vm-service/src/vm_service/realmode.rs:4862`

**当前代码**：
```rust
// TODO: Implement all 16-bit addressing modes
```

**描述**：
- x86_64 实模式缺少完整的 16 位寻址模式实现
- 影响 DOS/BIOS 程序兼容性

**完整实现要求**：
- [ ] 实现所有 16 位寻址模式：
  - [BX+SI+disp8
  - [BX+SI+disp16
  - [BX+DI+disp8
  - [BX+DI+disp16
  - [BP+SI+disp8
  - [BP+SI+disp16
  - [BP+DI+disp8
  - [BP+DI+disp16
  - [SI+disp8
  - [SI+disp16
  - [DI+disp8
  - [DI+disp16
  - [BP+disp8
  - [BP+disp16
  - [BX+disp8
  - [BX+disp16
- [ ] 添加单元测试
- [ ] 集成测试
- [ ] 性能验证

**预计工作量**：5-7 天
**依赖**：无
**负责人**：待分配

#### Stub I002：内存 AND 操作（vm-service）

**文件**：`crates/runtime/vm-service/src/vm_service/realmode.rs`（多处）

**当前代码**（多个位置）：
```rust
// TODO: Implement memory AND operation
```

**出现位置**：
- 行 3619：AND AL, [addr]
- 行 3656：AND AX, [addr]
- 行 3693：AND EAX, [addr]
- 行 3730：AND RAX, [addr]

**描述**：
- x86_64 实模式缺少内存 AND 操作实现
- 影响位操作程序

**完整实现要求**：
- [ ] 实现 AND AL, [addr]
- [ ] 实现 AND AX, [addr]
- [ ] 实现 AND EAX, [addr]
- [ ] 实现 AND RAX, [addr]
- [ ] 处理所有内存寻址模式
- [ ] 添加标志位更新（SF, ZF, PF, CF, OF）
- [ ] 添加单元测试
- [ ] 集成测试

**预计工作量**：2-3 天
**依赖**：I001（寻址模式）
**负责人**：待分配

#### Stub I003：内存 INC/DEC 操作（vm-service）

**文件**：`crates/runtime/vm-service/src/vm_service/realmode.rs`

**当前代码**：
```rust
// TODO: Implement actual INC for memory/registers
// TODO: Implement actual DEC for memory/registers
```

**位置**：
- 行 3794：INC
- 行 3800：DEC

**描述**：
- x86_64 实模式缺少内存 INC/DEC 操作实现
- 影响计数器操作

**完整实现要求**：
- [ ] 实现 INC [mem]
- [ ] 实现 DEC [mem]
- [ ] 处理所有内存寻址模式
- [ ] 添加标志位更新（SF, ZF, PF, AF, OF）
- [ ] 添加单元测试
- [ ] 集成测试

**预计工作量**：1-2 天
**依赖**：I001
**负责人**：待分配

#### Stub I004：栈操作（vm-service）

**文件**：`crates/runtime/vm-service/src/vm_service/realmode.rs`

**当前代码**：
```rust
// TODO: Push return address to stack
// TODO: Implement stack push
```

**位置**：
- 行 3911：PUSH 返回地址
- 行 4021：栈 PUSH 操作

**描述**：
- 栈操作不完整
- 影响 CALL/RET 和函数调用

**完整实现要求**：
- [ ] 实现 PUSH [mem]
- [ ] 实现 POP [mem]
- [ ] 实现 PUSHF/POPF（标志寄存器）
- [ ] 实现栈指针管理（SP/ESP/RSP）
- [ ] 处理栈溢出检查
- [ ] 添加单元测试
- [ ] 集成测试

**预计工作量**：2-3 天
**依赖**：I001
**负责人**：待分配

#### Stub I005：符号扩展/零扩展移动（vm-service）

**文件**：`crates/runtime/vm-service/src/vm_service/realmode.rs`

**当前代码**：
```rust
// TODO: Implement actual sign-extended move
// TODO: Implement actual zero-extended move
// TODO: Implement actual zero-extended move
```

**位置**：
- 行 5795：MOVSX（符号扩展）
- 行 5809：MOVZX（零扩展）
- 行 5820：MOVZX（零扩展）

**描述**：
- 缺少符号扩展和零扩展移动指令实现
- 影响数据类型转换

**完整实现要求**：
- [ ] 实现 MOVSX（符号扩展）
  - [ ] MOVSX r8, r/m8
  - [ ] MOVSX r16, r/m8
  - [ ] MOVSX r32, r/m8
  - [ ] MOVSX r64, r/m8
- [ ] 实现 MOVZX（零扩展）
  - [ ] MOVZX r8, r/m8
  - [ ] MOVZX r16, r/m8
  - [ ] MOVZX r32, r/m8
- [ ] 添加单元测试
- [ ] 集成测试

**预计工作量**：1-2 天
**依赖**：无
**负责人**：待分配

---

## 内存管理

### P2 - 中优先级

#### Stub M001：GPU 设备检测（vm-core）

**文件**：
- `crates/core/vm-core/src/gpu/device.rs:260`
- `crates/core/vm-core/src/gpu/device.rs:273`

**当前代码**：
```rust
// TODO: 通过 vm-passthrough crate 实现GPU设备检测
// TODO: 通过 vm-passthrough crate 实现GPU设备检测
```

**描述**：
- GPU 设备检测依赖 vm-passthrough crate，但当前使用 stub
- 影响 GPU 直通功能

**完整实现要求**：
- [ ] 集成 vm-passthrough crate
- [ ] 实现 GPU 设备枚举
- [ ] 实现设备能力检测
  - [ ] NVIDIA GPU
  - [ ] AMD GPU
  - [ ] Intel GPU
- [ ] 实现设备隔离和直通
- [ ] 添加单元测试
- [ ] 添加集成测试

**预计工作量**：5-7 天
**依赖**：vm-passthrough crate 实现
**负责人**：待分配

#### Stub M002：事件订阅（vm-core）

**文件**：`crates/core/vm-core/src/domain_services/persistent_event_bus.rs:121`

**当前代码**：
```rust
// TODO: Implement handler subscription
```

**描述**：
- 持久化事件总线缺少处理器订阅实现
- 影响事件驱动架构

**完整实现要求**：
- [ ] 实现事件处理器注册
- [ ] 实现事件处理器注销
- [ ] 实现事件过滤
- [ ] 实现事件优先级
- [ ] 实现异步事件分发
- [ ] 添加单元测试
- [ ] 添加集成测试

**预计工作量**：2-3 天
**依赖**：无
**负责人**：待分配

---

## JIT 编译器

### P1 - 高优先级

#### Stub J001：LLVM 后端（vm-engine-jit）

**文件**：`crates/execution/vm-engine-jit/Cargo.toml:18`

**当前状态**：
```toml
# vm-engine-interpreter = { path = "../vm-engine-interpreter" }  # TODO: missing crate
```

**描述**：
- LLVM 后端被注释掉，crate 缺失
- 影响 JIT 后端选择和性能

**完整实现要求**：
- [ ] 创建 vm-engine-jit-llvm crate（如果需要）
- [ ] 或移除 LLVM 相关配置（如果不需要）
- [ ] 更新文档说明后端选择
- [ ] 如需实现：
  - [ ] 集成 LLVM FFI
  - [ ] 实现 IR 到 LLVM IR 转换
  - [ ] 实现代码生成
  - [ ] 实现优化 passes
  - [ ] 添加单元测试
  - [ ] 添加集成测试

**预计工作量**：4-6 周（如需实现）/ 1 天（如需移除）
**依赖**：无
**负责人**：待分配
**决策点**：确定是否需要 LLVM 后端

---

## 设备模拟

### P2 - 中优先级

#### Stub D001：PCI 设备枚举（vm-device）

**状态**：待识别

**描述**：
- 可能存在 PCI 设备枚举的 stub 实现
- 影响 PCIe 设备热插拔和配置

**完整实现要求**：
- [ ] 识别现有 stub 实现
- [ ] 实现完整的 PCI 配置空间访问
- [ ] 实现 PCI 设备树管理
- [ ] 实现 PCI 设备热插拔
- [ ] 添加单元测试
- [ ] 添加集成测试

**预计工作量**：5-7 天
**依赖**：无
**负责人**：待分配

---

## 跨架构翻译

### P2 - 中优先级

#### Stub T001：ARM64 ↔ RISC-V64 翻译

**状态**：部分实现

**描述**：
- ARM64 ↔ RISC-V64 翻译部分实现
- 影响跨架构功能完整性

**完整实现要求**：
- [ ] 实现 ARM64 → RISC-V64 翻译
  - [ ] 指令集映射
  - [ ] 寄存器映射
  - [ ] 内存访问优化
  - [ ] 特殊指令处理
- [ ] 实现 RISC-V64 → ARM64 翻译
  - [ ] 指令集映射
  - [ ] 寄存器映射
  - [ ] 内存访问优化
  - [ ] 特殊指令处理
- [ ] 性能优化
  - [ ] 翻译缓存
  - [ ] 模式匹配
  - [ ] 寄存器映射缓存
- [ ] 添加单元测试
- [ ] 添加集成测试
- [ ] 性能基准测试

**预计工作量**：2-3 周
**依赖**：FI001（指令集实现）
**负责人**：待分配

---

## 硬件加速

### P1 - 高优先级

#### Stub H001：HVF 完整实现（vm-accel）

**状态**：部分实现

**描述**：
- macOS ARM64 HVF 实现不完整
- 影响 macOS 虚拟化性能

**完整实现要求**：
- [ ] 完善 HVF API 集成
  - [ ] 虚拟 CPU 创建和管理
  - [ ] 内存映射
  - [ ] 中断注入
  - [ ] 寄存器读写
- [ ] 实现 HVF 特定优化
  - [ ] Apple Silicon 优化
  - [ ] ARM64 特定指令加速
- [ ] 添加单元测试
- [ ] 添加集成测试
- [ ] 性能基准测试
- [ ] 文档完善

**预计工作量**：2-3 周
**依赖**：无
**负责人**：待分配

#### Stub H002：WHP 完整实现（vm-accel）

**状态**：部分实现

**描述**：
- Windows Hyper-V API (WHPX) 实现不完整
- 影响 Windows 虚拟化性能

**完整实现要求**：
- [ ] 完善 WHPX API 集成
  - [ ] 虚拟处理器创建和管理
  - [ ] 虚拟内存管理
  - [ ] 中断注入
  - [ ] 寄存器访问
- [ ] 实现 WHPX 特定优化
  - [ ] Windows 特定指令加速
  - [ ] NUMA 支持
- [ ] 添加单元测试
  - [ ] Windows 10/11 测试
  - [ ] 不同架构测试
- [ ] 添加集成测试
- [ ] 性能基准测试
- [ ] 文档完善

**预计工作量**：2-3 周
**依赖**：无
**负责人**：待分配

---

## 垃圾回收

### P2 - 中优先级

#### Stub G001：GC 写屏障实现

**状态**：部分实现

**描述**：
- 垃圾回收的写屏障实现不完整
- 影响并发 GC 的正确性和性能

**完整实现要求**：
- [ ] 完善写屏障类型
  - [ ] 饱和写屏障
  - [ ] 增量写屏障
  - [ ] 卡片写屏障
- [ ] 实现写屏障生成
  - [ ] 编译时注入
  - [ ] 运行时插入
- [ ] 性能优化
  - [ ] 屏障消除
  - [ ] 屏障合并
- [ ] 添加单元测试
  - [ ] 屏障正确性测试
  - [ ] 并发正确性测试（loom）
- [ ] 添加集成测试
- [ ] 性能基准测试
- [ ] 文档完善

**预计工作量**：1-2 周
**依赖**：无
**负责人**：待分配

#### Stub G002：GC 指针跟踪

**状态**：未实现

**描述**：
- GC 与 JIT 的指针跟踪不完整
- 影响 GC 的准确性和性能

**完整实现要求**：
- [ ] 设计 GC-JIT 集成接口
- [ ] 实现指针映射表
- [ ] 实现栈根扫描
- [ ] 实现寄存器根扫描
- [ ] 实现全局根扫描
- [ ] 性能优化
  - [ ] 快速根标记
  - [ ] 增量更新
- [ ] 添加单元测试
- [ ] 添加集成测试
- [ ] 性能基准测试
- [ ] 文档完善

**预计工作量**：2-3 周
**依赖**：JIT 集成
**负责人**：待分配

---

## Stub 实现总结

### 优先级分布

| 优先级 | 数量 | 预计总工作量 |
|--------|------|--------------|
| P0 - 紧急 | 0 | 0 天 |
| P1 - 高优先级 | 7 | 15-25 天 |
| P2 - 中优先级 | 7 | 25-40 天 |
| P3 - 低优先级 | 0 | 0 天 |
| **总计** | **14** | **40-65 天** |

### 类别分布

| 类别 | 数量 | 主要 Stub |
|------|------|-----------|
| 执行引擎 | 1 | 统一执行器缓存加载 |
| 指令解码 | 5 | 16 位寻址, 内存 AND/INC/DEC, 栈操作, 扩展移动 |
| 内存管理 | 2 | GPU 设备检测, 事件订阅 |
| JIT 编译器 | 1 | LLVM 后端 |
| 设备模拟 | 1 | PCI 设备枚举 |
| 跨架构翻译 | 1 | ARM64↔RISC-V64 翻译 |
| 硬件加速 | 2 | HVF, WHP 完整实现 |
| 垃圾回收 | 2 | GC 写屏障, GC 指针跟踪 |

### 实施路线图

| 季度 | P1 | P2 | P3 | 重点 |
|------|-----|-----|-----|------|
| Q1 | E001, I001, I002, I003, I004, I005, J001 | M001, M002 | - | 完成核心执行引擎和指令解码 stub |
| Q2 | H001, H002 | T001, G001, G002 | - | 完成硬件加速和 GC stub |
| Q3 | - | D001 | - | 完成设备模拟和跨架构翻译 |

## 实施流程

### Stub 识别
- 新 stub 发现时立即记录到此文档
- 分析 stub 的完整性和影响
- 评估修复优先级和复杂度

### Stub 跟踪
- 定期审查 stub 列表（每月）
- 更新 stub 状态（待开始、进行中、已完成）
- 记录实际工作量

### Stub 实施
- 按优先级顺序实施 stub
- P0 stub 立即处理
- P1 stub 按计划处理
- P2/P3 stub 根据资源情况处理

### Stub 验证
- 完整实现后进行验证
- 单元测试覆盖
- 集成测试验证
- 性能基准测试
- 代码审查

## 附录

### A. Stub 编号规则
- EXXX：执行引擎（Execution）
- IXXX：指令解码（Instruction Decoding）
- MXXX：内存管理（Memory Management）
- JXXX：JIT 编译器（JIT Compiler）
- DXXX：设备模拟（Device Emulation）
- TXXX：跨架构翻译（Translation）
- HXXX：硬件加速（Hardware Acceleration）
- GXXX：垃圾回收（Garbage Collection）

### B. 参考资料
- [Stub Definition](https://en.wikipedia.org/wiki/Method_stub)
- [Test-Driven Development](https://en.wikipedia.org/wiki/Test-driven_development)
- [Test Doubles](https://martinfowler.com/bliki/TestDouble.html)

---

**文档版本**：1.0  
**创建日期**：2026-01-20  
**最后更新**：2026-01-20  
**负责人**：待分配  
**下次审查**：2026-02-20
