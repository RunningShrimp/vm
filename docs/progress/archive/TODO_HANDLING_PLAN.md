# TODO标记处理计划

## 执行时间
2024年12月24日

## 目标
根据《Rust虚拟机软件改进实施计划》任务6，处理项目中的高优先级和中优先级TODO标记。

## 当前TODO标记统计

### 扫描结果

#### 实际TODO标记（非注释、非文档）
在项目源代码中发现的TODO标记：

1. **vm-engine-jit/src/codegen.rs:770**
   ```rust
   // TODO: 实现AArch64指令特征
   ```
   - 位置：指令特征定义
   - 类型：中优先级
   - 影响：AArch64架构支持不完整

2. **vm-engine-jit/src/codegen.rs:775**
   ```rust
   // TODO: 实现RISCV64指令特征
   ```
   - 位置：指令特征定义
   - 类型：中优先级
   - 影响：RISC-V架构支持不完整

3. **vm-ir/src/lift/semantics.rs:9**
   ```rust
   // TODO: Migrate these modules later if needed, for now we will implement stubs or comment out imports if they don't exist yet
   ```
   - 位置：IR提升语义模块
   - 类型：低优先级（计划性注释）
   - 影响：模块迁移计划

#### TODO跟踪系统（vm-todo-tracker）
该项目是一个TODO管理系统，包含以下组件：
- `TodoItem`：TODO项目数据结构
- `TodoTracker`：TODO管理器
- `TodoScanner`：TODO扫描器
- 多种过滤器和统计功能

**注意**：vm-todo-tracker是一个独立的管理系统，其代码中的TODO相关术语是正常的功能实现，不是需要处理的TODO标记。

## TODO优先级分析

### 高优先级（P0）
- **无**：项目中没有发现需要立即处理的高优先级TODO

### 中优先级（P1）
1. **实现AArch64指令特征**（vm-engine-jit/src/codegen.rs:770）
   - **描述**：实现AArch64架构的指令特征数据
   - **影响**：AArch64代码生成时无法获取准确的指令特征
   - **依赖**：无
   - **预计工作量**：1-2天

2. **实现RISCV64指令特征**（vm-engine-jit/src/codegen.rs:775）
   - **描述**：实现RISC-V架构的指令特征数据
   - **影响**：RISC-V代码生成时无法获取准确的指令特征
   - **依赖**：无
   - **预计工作量**：1-2天

### 低优先级（P2）
1. **模块迁移计划**（vm-ir/src/lift/semantics.rs:9）
   - **描述**：模块迁移的规划注释
   - **影响**：无（仅为计划性注释）
   - **依赖**：无
   - **预计工作量**：无需处理

## TODO处理计划

### 阶段1：分析现有代码（第1天）

#### 1.1 研究指令特征定义
- [x] 检查`codegen.rs`中的指令特征结构
- [x] 查看现有X86_64指令特征实现
- [ ] 分析指令特征包含的字段
- [ ] 确定需要的数据源

#### 1.2 研究AArch64指令集
- [ ] 查阅AArch64指令集文档
- [ ] 识别常用AArch64指令
- [ ] 收集指令特征数据（延迟、吞吐量等）
- [ ] 确定优先级指令列表

#### 1.3 研究RISC-V指令集
- [ ] 查阅RISC-V指令集文档
- [ ] 识别常用RISC-V指令
- [ ] 收集指令特征数据
- [ ] 确定优先级指令列表

### 阶段2：实现AArch64指令特征（第2-3天）

#### 2.1 创建AArch64指令特征数据
```rust
// 在codegen.rs中添加
fn get_aarch64_instruction_features(inst: &Instruction) -> InstructionFeatures {
    match inst.opcode {
        AArch64Opcode::ADD => InstructionFeatures {
            latency: 1,
            throughput: 1,
            uops: 1,
            execution_unit: ExecutionUnit::ALU,
            // ... 其他字段
        },
        AArch64Opcode::LDR => InstructionFeatures {
            latency: 4,
            throughput: 1,
            uops: 1,
            execution_unit: ExecutionUnit::LoadStore,
            // ... 其他字段
        },
        // ... 更多指令
    }
}
```

#### 2.2 实现常用指令
- 算术指令：ADD, SUB, MUL, DIV
- 逻辑指令：AND, ORR, EOR
- 移位指令：LSL, LSR, ASR
- 加载存储：LDR, STR
- 分支指令：B, BL, BR

#### 2.3 测试验证
- [ ] 创建AArch64指令特征测试
- [ ] 验证数据准确性
- [ ] 性能测试

### 阶段3：实现RISC-V指令特征（第4-5天）

#### 3.1 创建RISC-V指令特征数据
```rust
// 在codegen.rs中添加
fn get_riscv64_instruction_features(inst: &Instruction) -> InstructionFeatures {
    match inst.opcode {
        RiscVOpcode::ADD => InstructionFeatures {
            latency: 1,
            throughput: 1,
            uops: 1,
            execution_unit: ExecutionUnit::ALU,
            // ... 其他字段
        },
        RiscVOpcode::LW => InstructionFeatures {
            latency: 4,
            throughput: 1,
            uops: 1,
            execution_unit: ExecutionUnit::LoadStore,
            // ... 其他字段
        },
        // ... 更多指令
    }
}
```

#### 3.2 实现常用指令
- 算术指令：ADD, SUB, MUL, DIV
- 逻辑指令：AND, OR, XOR
- 移位指令：SLL, SRL, SRA
- 加载存储：LW, SW
- 分支指令：JAL, JALR, BEQ

#### 3.3 测试验证
- [ ] 创建RISC-V指令特征测试
- [ ] 验证数据准确性
- [ ] 性能测试

### 阶段4：集成和文档（第6天）

#### 4.1 集成到代码生成器
- [ ] 更新代码生成器以使用新的指令特征
- [ ] 确保与现有代码兼容
- [ ] 处理边界情况

#### 4.2 文档更新
- [ ] 更新代码注释
- [ ] 添加指令特征文档
- [ ] 更新架构支持文档

#### 4.3 最终验证
- [ ] 运行完整测试套件
- [ ] 性能基准测试
- [ ] 代码审查

## 实施细节

### 指令特征数据来源

#### 1. AArch64指令特征数据
- **文档参考**：
  - ARM Architecture Reference Manual
  - Arm Cortex-A72 Software Optimization Guide
  - Arm Cortex-A53 Software Optimization Guide

- **目标架构**：
  - ARMv8-A
  - Cortex-A53/A72（常见目标）

- **优先级指令**（前50个最常用指令）：
  - 算术：ADD, SUB, AND, ORR, EOR, MOV, MVN
  - 移位：LSL, LSR, ASR, ROR
  - 乘除：MUL, MNEG, UDIV, SDIV
  - 加载存储：LDR, LDRB, LDRH, LDRSB, LDRSH, STR, STRB, STRH
  - 分支：B, BL, BR, BLR, B.EQ, B.NE
  - 条件执行：CSEL, CSINC, CSINV

#### 2. RISC-V指令特征数据
- **文档参考**：
  - RISC-V User-Level ISA
  - RISC-V Privileged Architecture
  - SiFive U74-MC Core Complex Manual

- **目标架构**：
  - RV64GC
  - SiFive U74/Ariane（常见目标）

- **优先级指令**（前50个最常用指令）：
  - 算术：ADD, SUB, AND, OR, XOR, SLL, SRL, SRA
  - 乘除：MUL, MULH, DIV, DIVU, REM, REMU
  - 加载存储：LB, LH, LW, LBU, LHU, SB, SH, SW
  - 分支：JAL, JALR, BEQ, BNE, BLT, BGE, BLTU, BGEU
  - 内存屏障：FENCE
  - CSR指令：CSRRW, CSRRS, CSRRC

### 数据结构

```rust
/// 指令特征（已在codegen.rs中定义）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InstructionFeatures {
    /// 指令延迟（时钟周期）
    pub latency: u8,
    /// 指令吞吐量（时钟周期）
    pub throughput: u8,
    /// 微操作数量
    pub uops: u8,
    /// 执行单元
    pub execution_unit: ExecutionUnit,
    /// 是否可流水线化
    pub pipelined: bool,
    /// 是否可乱序执行
    pub out_of_order: bool,
    /// 是否是内存指令
    pub is_memory: bool,
    /// 是否是分支指令
    pub is_branch: bool,
    /// 是否是原子指令
    pub is_atomic: bool,
}
```

## 预期成果

### TODO标记清理
- 高优先级TODO：0个 → 0个
- 中优先级TODO：2个 → 0个
- 低优先级TODO：1个 → 0个

### 架构支持改进
- AArch64：基础支持 → 完整指令特征支持
- RISC-V：基础支持 → 完整指令特征支持

### 代码质量提升
- 更准确的代码生成优化
- 更好的性能预测
- 更完善的架构支持

## 风险和缓解措施

### 风险1：指令特征数据不准确
**缓解措施**：
- 使用官方文档作为主要数据源
- 通过性能测试验证
- 参考开源实现（如LLVM、GCC）

### 风险2：工作量估计不准确
**缓解措施**：
- 先实现核心指令（前20个）
- 逐步扩展到全部指令
- 按优先级分阶段实施

### 风险3：架构差异
**缓解措施**：
- 支持多目标架构配置
- 提供默认值
- 标记未实现的指令

## 测试策略

### 单元测试
- 测试每个指令的特征数据
- 测试边界情况
- 测试错误处理

### 集成测试
- 测试代码生成器使用指令特征
- 测试优化器使用指令特征
- 测试性能预测

### 性能测试
- 基准测试不同架构的代码生成
- 验证性能提升
- 对比优化前后的性能

## 进度跟踪

### 里程碑
- [x] 里程碑1：完成TODO扫描和分析（第1天）
- [ ] 里程碑2：完成AArch64指令特征（第3天）
- [ ] 里程碑3：完成RISC-V指令特征（第5天）
- [ ] 里程碑4：完成集成和文档（第6天）

### 每日更新
- 第1天：分析完成
- 第2天：开始AArch64实现
- 第3天：AArch64核心指令完成
- 第4天：开始RISC-V实现
- 第5天：RISC-V核心指令完成
- 第6天：集成测试和文档

## 后续工作

### 短期（1-2周）
- 完成所有常用指令的特征
- 添加更多目标架构支持
- 完善文档和测试

### 中期（1-2个月）
- 建立指令特征数据库
- 支持动态更新特征
- 集成性能监控

### 长期（3-6个月）
- 自动化特征收集
- 机器学习辅助特征预测
- 跨架构特征映射

## 总结

当前项目中有**2个中优先级TODO**需要处理：

1. **实现AArch64指令特征**：为AArch64架构添加完整的指令特征数据
2. **实现RISC-V指令特征**：为RISC-V架构添加完整的指令特征数据

这两个TODO都涉及代码生成和性能优化，是中期计划中"完善RISC-V支持"和"增强跨架构翻译功能"的基础。

通过实施本计划，预计需要**6个工作日**，可以：
- 清除所有中优先级TODO标记
- 显著提升AArch64和RISC-V架构的代码生成质量
- 为后续的架构优化工作奠定基础

---

**报告生成时间**：2024年12月24日
**当前进度**：10%（分析阶段完成）
**预计完成时间**：6个工作日

