# RISC-V扩展集成实施计划

**日期**：2024年12月25日  
**状态**：📋 规划中  
**版本**：1.0

---

## 📊 总体目标

### 主要目标

将143个RISC-V指令特征集成到`vm-engine-jit/src/codegen.rs`，完善RISC-V代码生成和优化支持。

### 次要目标

1. **功能完整性**：支持完整的RISC-V基础指令集 + M/A/F/D/C扩展
2. **代码质量**：确保代码编译通过，没有错误或警告
3. **性能优化**：为RISC-V指令提供准确的性能特征数据
4. **可维护性**：清晰的代码结构和完整的文档

---

## 📋 实施范围

### 指令集覆盖

| 扩展 | 指令数量 | 说明 |
|--------|---------|------|
| **基础指令集 (RV64I)** | 37个 | 整数运算、分支、访存 |
| **M扩展 (RV64M)** | 16个 | 整数乘除 |
| **A扩展 (RV64A)** | 20个 | 原子字操作 |
| **F扩展 (RV64F)** | 32个 | 单精度浮点 |
| **D扩展 (RV64D)** | 38个 | 双精度浮点 |
| **总计** | **143个** | 完整的RISC-V支持 |

---

## 📋 阶段1：准备工作（预计1-2小时）

### 任务1.1：分析现有代码架构

#### 子任务
- [ ] 检查`vm-engine-jit/src/codegen.rs`中RISC-V指令特征初始化代码
- [ ] 分析`vm-ir/src/lib.rs`中的模块导出
- [ ] 确认`ExecutionUnitType`和`InstructionFeatures`结构定义
- [ ] 检查`TargetArch`枚举是否包含RISC-V支持
- [ ] 识别现有的基础指令集实现

#### 预期成果
- 完整的代码架构分析报告
- 清晰的依赖关系图
- 识别的集成点和潜在风险

### 任务1.2：设计RISC-V指令特征数据结构

#### 子任务
- [ ] 设计`InstructionFeatures`结构体字段（latency, throughput, size等）
- [ ] 确定性能特征值的合理范围
- [ ] 设计指令分组策略（按扩展、按功能）
- [ ] 设计数据初始化函数签名

#### 预期成果
- 完整的`InstructionFeatures`结构体设计文档
- 清晰的数据组织规范
- 初始化函数接口设计

---

## 📋 阶段2：实施基础指令集（预计2-3天）

### 任务2.1：实施RV64I基础指令集（37个指令）

#### 子任务
- [ ] 实现`init_riscv64i_base_features()`函数
- [ ] 添加37个基础指令的特征数据
- [ ] 确保每个指令都有合理的性能特征值
- [ ] 集成到`init_riscv64_features()`调用

#### 指令列表
**算术指令（12个）**：
- ADD（加法）：latency 1, throughput 1, size 4
- SUB（减法）：latency 1, throughput 1, size 4
- SLL（逻辑左移）：latency 1, throughput 1, size 4
- SRL（逻辑右移）：latency 1, throughput 1, size 4
- SRA（算术右移）：latency 1, throughput 1, size 4
- SLT（有符号左移）：latency 1, throughput 1, size 4
- SRT（有符号右移）：latency 1, throughput 1, size 4

**分支指令（6个）**：
- JAL（跳转并链接）：latency 0, throughput 1, size 4
- JALR（跳转并链接返回）：latency 0, throughput 1, size 4
- BEQ（分支相等）：latency 0, throughput 1, size 4
- BNE（分支不等）：latency 0, throughput 1, size 4
- BLT（分支小于）：latency 0, throughput 1, size 4
- BGE（分支大于等于）：latency 0, throughput 1, size 4

**访存指令（19个）**：
- LB（加载字节）：latency 1, throughput 1, size 4
- LH（加载半字）：latency 1, throughput 1, size 4
- LW（加载字）：latency 1, throughput 1, size 4
- LD（加载双字）：latency 1, throughput 1, size 4
- LBU（加载字节无符号）：latency 1, throughput 1, size 4
- LHU（加载半字无符号）：latency 1, throughput 1, size 4
- SB（存储字节）：latency 1, throughput 1, size 4
- SH（存储半字）：latency 1, throughput 1, size 4
- SW（存储字）：latency 1, throughput 1, size 4
- SD（存储双字）：latency 1, throughput 1, size 4

#### 预期成果
- RV64I基础指令集支持完成（37个指令）
- 所有指令都有完整的性能特征数据
- 集成到codegen.rs成功

### 任务2.2：实施M扩展（RV64M，16个指令）

#### 子任务
- [ ] 实现`init_riscv64m_features()`函数
- [ ] 添加16个M扩展乘法指令
- [ ] 为每个乘法指令设置合理的性能特征
- [ ] 集成到`init_riscv64_features()`调用

#### 指令列表
**MUL（8位乘法，6个）**：
- MUL：latency 3-5, throughput 0.2-0.5, size 4
- MULH：高位乘法：latency 3-5, throughput 0.2-0.5, size 4
- MULHSU：高位乘法立即数：latency 3-5, throughput 0.2-0.5, size 4

**MUL（16位乘法，6个）**：
- MULW：latency 3-5, throughput 0.2-0.5, size 8
- MULW：低位乘法：latency 3-5, throughput 0.2-0.5, size 8
- MULHSU：高位乘法立即数：latency 3-5, throughput 0.2-0.5, size 8
- MULWSU：高位乘法立即数：latency 3-5, throughput 0.2-0.5, size 8

**DIV（除法，4个）**：
- DIV：8位有符号除法：latency 32, throughput 0.03, size 4
- DIVU：8位无符号除法：latency 32, throughput 0.03, size 4
- DIVW：16位除法：latency 64, throughput 0.03, size 8
- REM：8位取余：latency 32, throughput 0.03, size 4
- REMW：16位取余：latency 64, throughput 0.03, size 8

#### 预期成果
- RV64M扩展支持完成（16个指令）
- 所有M扩展指令都有完整的性能特征数据
- 集成到codegen.rs成功

---

## 📋 阶段3：实施A/F/D/C扩展（预计3-5天）

### 任务3.1：实施A扩展（RV64A，20个指令）

#### 子任务
- [ ] 实现`init_riscv64a_features()`函数
- [ ] 添加20个A扩展原字操作指令
- [ ] 为每个原字操作指令设置合理的性能特征
- [ ] 集成到`init_riscv64_features()`调用

#### 指令列表（20个）
**加载保留（LR.W系列，4个）**：
- LR.W：加载保留字：latency 1, throughput 0.5, size 4
- LR.W.U：加载保留字无符号：latency 1, throughput 0.5, size 4
- LDR.W：加载保留字无符号：latency 1, throughput 0.5, size 8
- LDR：加载保留字无符号：latency 1, throughput 0.5, size 8

**加载保留（LR.D系列，4个）**：
- LR.D：加载保留字：latency 1, throughput 0.5, size 4
- LR.D.U：加载保留字无符号：latency 1, throughput 0.5, size 8
- LDR：加载保留字无符号：latency 1, throughput 0.5, size 8

**加载存储（LR系列，4个）**：
- STR.W：存储保留字：latency 1, throughput 0.5, size 4
- STR.W.U：存储保留字无符号：latency 1, throughput 0.5, size 4
- STR.D：存储保留字：latency 1, throughput 0.5, size 8
- STR：存储保留字无符号：latency 1, throughput 0.5, size 8

**原子操作（AMO系列，8个）**：
- AMOADD.W：原子加（5位地址）：latency 8-15, throughput 0.06, size 4
- AMOAND.W：原子与（5位地址）：latency 8-15, throughput 0.06, size 4
- AMOOR.W：原子或（5位地址）：latency 8-15, throughput 0.06, size 4
- AMOXOR.W：原子异或（5位地址）：latency 8-15, throughput 0.06, size 4
- AMOMIN.W.U：原子最小（无符号5位地址）：latency 8-15, throughput 0.06, size 4
- AMOMAX.W.U：原子最大（无符号5位地址）：latency 8-15, throughput 0.06, size 4

**加载存储（LR系列，4个）**：
- LDR：加载保留字：latency 1, throughput 0.5, size 8
- LDR：加载保留字无符号：latency 1, throughput 0.5, size 8
- STR：存储保留字：latency 1, throughput 0.5, size 8

#### 预期成果
- RV64A扩展支持完成（20个指令）
- 所有A扩展指令都有完整的性能特征数据
- 集成到codegen.rs成功

### 任务3.2：实施F扩展（RV64F，32个指令）

#### 子任务
- [ ] 实现`init_riscv64f_features()`函数
- [ ] 添加32个F扩展单精度浮点指令
- [ ] 为每个浮点指令设置合理的性能特征
- [ ] 集成到`init_riscv64_features()`调用

#### 指令列表（32个）
**浮点加载（FLW系列，16个）**：
- FLW：加载单精度浮点：latency 4-6, throughput 0.2, size 4
- FLW.WU：加载无符号单精度：latency 4-6, throughput 0.2, size 4
- FLW.D：加载双精度浮点：latency 4-6, throughput 0.2, size 8

**浮点存储（FSW系列，8个）**：
- FSW：存储单精度浮点：latency 4-6, throughput 0.2, size 4
- FSW.W.U：存储无符号单精度：latency 4-6, throughput 0.2, size 4
- FSW.D：存储双精度浮点：latency 4-6, throughput 0.2, size 8

**浮点运算（FM系列，4个）**：
- FADD.S：单精度加：latency 4-6, throughput 0.2, size 4
- FSUB.S：单精度减：latency 4-6, throughput 0.2, size 4
- FMUL.S：单精度乘：latency 4-6, throughput 0.05, size 4
- FDIV.S：单精度除：latency 32-64, throughput 0.03, size 4
- FSQRT.S：单精度平方根：latency 12-20, throughput 0.08, size 4

**浮点比较和转换（FM系列，4个）**：
- FEQ.S：单精度比较：latency 4-6, throughput 0.2, size 4
- FLT.S：单精度小于：latency 4-6, throughput 0.2, size 4
- FLE.S：单精度小于等于：latency 4-6, throughput 0.2, size 4

**浮点绝对值（FM系列，2个）**：
- FABS.S：单精度绝对值：latency 4-6, throughput 0.2, size 4
- FCLASS.S：单精度分类：latency 4-6, throughput 0.2, size 4

#### 预期成果
- RV64F扩展支持完成（32个指令）
- 所有F扩展指令都有完整的性能特征数据
- 集成到codegen.rs成功

### 任务3.3：实施D扩展（RV64D，38个指令）

#### 子任务
- [ ] 实现`init_riscv64d_features()`函数
- [ ] 添加38个D扩展双精度浮点指令
- [ ] 为每个双精度浮点指令设置合理的性能特征
- [ ] 集成到`init_riscv64_features()`调用

#### 指令列表（38个）
**浮点加载（FLD系列，16个）**：
- FLD：加载双精度浮点：latency 4-6, throughput 0.2, size 8
- FLD.WU：加载无符号双精度：latency 4-6, throughput 0.2, size 8
- FLD.D：加载双精度浮点：latency 4-6, throughput 0.2, size 8

**浮点存储（FSD系列，8个）**：
- FSD：存储双精度浮点：latency 4-6, throughput 0.2, size 8
- FSD.WU：存储无符号双精度：latency 4-6, throughput 0.2, size 8
- FSD.D：存储双精度浮点：latency 4-6, throughput 0.2, size 8

**浮点运算（FD系列，10个）**：
- FADD.D：双精度加：latency 4-6, throughput 0.2, size 8
- FSUB.D：双精度减：latency 4-6, throughput 0.2, size 8
- FMUL.D：双精度乘：latency 4-6, throughput 0.05, size 8
- FDIV.D：双精度除：latency 32-64, throughput 0.03, size 8
- FSQRT.D：双精度平方根：latency 12-20, throughput 0.08, size 8

**浮点比较和转换（FD系列，4个）**：
- FEQ.D：双精度比较：latency 4-6, throughput 0.2, size 8
- FLT.D：双精度小于：latency 4-6, throughput 0.2, size 8
- FLE.D：双精度小于等于：latency 4-6, throughput 0.2, size 8

**浮点绝对值（FD系列，2个）**：
- FABS.D：双精度绝对值：latency 4-6, throughput 0.2, size 8
- FCLASS.D：双精度分类：latency 4-6, throughput 0.2, size 8

**浮点转换（FCVT系列，8个）**：
- FCVT.W.S：单精度转字：latency 4-6, throughput 0.2, size 4
- FCVT.W.U.S：无符号单精度转字：latency 4-6, throughput 0.2, size 4
- FCVT.L.S：单精度转长：latency 4-6, throughput 0.2, size 8
- FCVT.L.U.S：无符号单精度转长：latency 4-6, throughput 0.2, size 8
- FCVT.W.S：单精度转双：latency 4-6, throughput 0.2, size 8
- FCVT.W.U.S：无符号单精度转双：latency 4-6, throughput 0.2, size 8

#### 预期成果
- RV64D扩展支持完成（38个指令）
- 所有D扩展指令都有完整的性能特征数据
- 集成到codegen.rs成功

### 任务3.4：实施C扩展（RV64C，27个指令）

#### 子任务
- [ ] 实现`init_rvc_features()`函数
- [ ] 添加27个C扩展压缩指令
- [ ] 为每个压缩指令设置合理的性能特征
- [ ] 集成到`init_riscv64_features()`调用

#### 指令列表（27个）
**压缩指令（12个）**：
- CLZ.W：计数前导零：latency 1-2, throughput 1, size 4
- CLO.W：计数前导一：latency 1-2, throughput 1, size 4
- CTZ.W：计数尾导零：latency 1-2, throughput 1, size 4
- CPOP.W：计数前导零个数：latency 2, throughput 1, size 4
- CPOP.W：计数尾导一：latency 2, throughput 1, size 4
- MAX：有符号最大：latency 1, throughput 0.5, size 4
- SEXT.B：符号扩展字节：latency 1-2, throughput 1, size 4
- SEXT.B：符号扩展半字：latency 1-2, throughput 1, size 4

**位操作（8个）**：
- AND：逻辑与：latency 1, throughput 1, size 4
- OR：逻辑或：latency 1, throughput 1, size 4
- XOR：逻辑异或：latency 1, throughput 1, size 4
- SLL：逻辑左移：latency 1, throughput 1, size 4
- SRL：逻辑右移：latency 1, throughput 1, size 4
- SRA：算术右移：latency 1, throughput 1, size 4

**其他（7个）**：
- REV8：字节反转：latency 1, throughput 0.5, size 4
- REV16：半字反转：latency 1, throughput 0.5, size 4
- ORC.B：字节逻辑或常数：latency 1, throughput 1, size 4
- ROR：循环右移：latency 1, throughput 0.5, size 4
- AUIPC：添加到PC高20位立即数：latency 1, throughput 1, size 4
- ADDI：加立即数：latency 1, throughput 1, size 4

#### 预期成果
- RV64C扩展支持完成（27个指令）
- 所有C扩展指令都有完整的性能特征数据
- 集成到codegen.rs成功

---

## 📋 阶段4：测试和验证（预计1天）

### 任务4.1：单元测试

#### 子任务
- [ ] 为每个指令集编写单元测试
- [ ] 测试指令特征初始化
- [ ] 测试特征数据的准确性
- [ ] 测试依赖关系

#### 子任务
- [ ] 为RV64I基础指令集编写单元测试
- [ ] 为RV64M扩展编写单元测试
- [ ] 为RV64A扩展编写单元测试
- [ ] 为RV64F扩展编写单元测试
- [ ] 为RV64D扩展编写单元测试
- [ ] 为RV64C扩展编写单元测试

#### 预期成果
- 所有扩展都有单元测试覆盖
- 测试通过率 > 90%

### 任务4.2：编译验证

#### 子任务
- [ ] 每个阶段完成后都进行编译验证
- [ ] 确保没有编译错误
- [ ] 确保没有警告

#### 预期成果
- 代码编译成功，0错误，0警告
- 所有RISC-V扩展集成完成

---

## 📋 阶段5：文档和总结（预计1天）

### 任务5.1：更新集成计划

#### 子任务
- [ ] 更新`RISCV_EXTENSION_INTEGRATION_PLAN.md`实施记录
- [ ] 创建`RISCV_EXTENSION_INTEGRATION_SUMMARY.md`总结文档
- [ ] 更新项目主进度文档

#### 预期成果
- 完整的实施文档
- 清晰的总结和后续指导

---

## 🎯 时间估算

| 阶段 | 预计时间 | 说明 |
|------|---------|------|
| 阶段1：准备 | 1-2小时 | 分析现有代码架构 |
| 阶段2：基础指令集 | 2-3天 | 实施RV64I基础指令集（37个） |
| 阶段3：A/F/D/C扩展 | 3-5天 | 实施A(20) + F(32) + D(38) + C(27) = 117个 |
| 阶段4：测试和验证 | 1天 | 单元测试和编译验证 |
| 阶段5：文档和总结 | 1天 | 更新文档和总结 |
| **总计** | **8-9天** | **143个RISC-V指令特征** |

---

## 📈 预期成果

### 功能完整性
- RV64I基础指令集：37个指令（100%）
- M扩展（RV64M）：16个指令（100%）
- A扩展（RV64A）：20个指令（100%）
- F扩展（RV64F）：32个指令（100%）
- D扩展（RV64D）：38个指令（100%）
- C扩展（RV64C）：27个指令（100%）
- **总计**：**143个指令**（100%）

### 代码质量
- 编译成功：0错误，0警告
- 测试覆盖：> 90%
- 文档完整性：完整的计划、实施和总结文档

### 性能优化
- 所有指令都有合理的性能特征数据
- 性能值基于硬件特性估算
- 支持代码生成器性能优化

---

## 🚧 风险管理

### 技术风险
1. **性能数据准确性**：指令性能特征值可能需要根据实际硬件调整
   - 缓解措施：在文档中标注这些值为估计值
   - 建议措施：在实际部署后进行性能测试和调优

2. **代码复杂性**：143个指令的特征数据可能难以维护
   - 缓解措施：使用数据驱动设计、代码生成工具
   - 建议措施：考虑使用宏或DSL来简化特征数据定义

3. **测试覆盖不足**：单元测试可能无法覆盖所有指令组合
   - 缓解措施：添加集成测试和性能基准测试
   - 建议措施：使用模糊测试和符号执行测试

### 风险缓解
1. **分阶段实施**：按照计划逐步实施，每个阶段都测试验证
2. **持续集成**：每个阶段完成后都编译验证
3. **文档更新**：保持文档与代码同步
4. **代码审查**：实施过程中进行代码审查

---

## 📝 关键设计决策

### 性能特征设计

#### 延迟（latency）
- 整数运算：1-4周期（快速）
- 乘除法：3-64周期（较慢）
- 浮点加/减：4-6周期（中等）
- 浮点乘/除：4-32, 32-64周期（较慢）
- 浮点比较：4-6周期（中等）
- 压缩指令：1-2周期（快速）
- 位操作：1周期（快速）
- 原子字操作：1-2周期（快速）
- 分支/跳转：0周期（控制流）

#### 吞吐量（throughput）
- 整数运算：1指令/周期（高）
- 乘除法：0.03-0.5指令/周期（低）
- 浮点运算：0.05-0.2指令/周期（低）
- 浮点运算：0.03-0.2指令/周期（低）
- 访存指令：1指令/周期（高）
- 原子字操作：1指令/周期（高）
- 分支/跳转：1指令/周期（高）

#### 指令大小（size）
- 整数运算：4字节（32位）
- 8位乘法：8字节（64位）
- 16位乘法：8字节（128位）
- 加载保留字：4/8字节（32/64位）
- 加载双精度：8字节（64位）
- 浮点数：4/8字节（32/64位）

#### 是否微操作（is_micro_op）
- 否：大部分指令（算术、逻辑、访存、分支）
- 是：少数指令（特殊控制、系统指令）

#### 依赖关系（dependencies）
- 大部分指令：依赖其他指令很少或为空
- 某些指令：依赖相关指令（如DIV依赖MUL）

---

## 📚 相关文档

### 输入文档
- `vm-ir/src/riscv_instruction_data.rs`：143个RISC-V指令特征数据源
- `vm-engine-jit/src/codegen.rs`：目标集成文件
- `vm-engine-jit/src/performance_optimizer.rs`：性能优化器

### 输出文档
- `RISCV_EXTENSION_INTEGRATION_PLAN.md`：本文档（实施计划）
- `RISCV_EXTENSION_INTEGRATION_SUMMARY.md`：总结文档（待创建）

---

## 🎯 下一步行动

### 立即开始
1. **开始阶段1**：分析现有代码架构
   - 检查`vm-engine-jit/src/codegen.rs`中的RISC-V指令特征初始化代码
   - 分析`ExecutionUnitType`枚举
   - 确定`InstructionFeatures`结构体

2. **创建RISC-V指令特征数据文件**
   - 创建`vm-engine-jit/src/riscv_instruction_features.rs`存储143个指令的特征数据
   - 按扩展组织数据（基础、M、A、F、D、C）
   - 为每个指令添加注释说明

### 短期目标
- 完成代码架构分析
- 创建指令特征数据文件
- 开始实施基础指令集（阶段2）

---

## 📊 成功标准

### 阶段完成标准
- [ ] 代码编译成功（0错误，0警告）
- [ ] 测试通过（> 90%）
- [ ] 文档完整
- [ ] 功能完整（143个指令）

### 总体进度
- [ ] 阶段1：准备工作完成
- [ ] 阶段2-5：A/F/D/C扩展实施中
- [ ] 阶段3-4：测试和验证待开始
- [ ] 阶段4-5：文档和总结待开始

---

**实施计划版本**：1.0  
**最后更新**：2024年12月25日  
**创建者**：AI Assistant

