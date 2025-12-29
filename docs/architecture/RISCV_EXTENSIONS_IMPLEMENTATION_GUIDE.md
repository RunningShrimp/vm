# RISC-V扩展实现指南

## 创建时间
2024年12月24日

## 概述

本文档提供了RISC-V指令集扩展的详细实现指南，包括M、A、F、D、C扩展的架构设计、实现步骤和测试计划。

---

## 一、RISC-V扩展概览

### 1.1 扩展列表

| 扩展名称 | 描述 | 优先级 | 预计指令数 | 预计时间 |
|---------|------|--------|------------|---------|
| M扩展 | 整数乘除扩展 | 高 | ~30个 | 2周 |
| A扩展 | 原子操作扩展 | 高 | ~20个 | 1.5周 |
| F扩展 | 单精度浮点扩展 | 高 | ~40个 | 2周 |
| D扩展 | 双精度浮点扩展 | 中 | ~30个 | 1.5周 |
| C扩展 | 压缩指令扩展 | 低 | ~50个 | 2周 |
| **总计** | - | - | **~170个** | **9周** |

### 1.2 扩展依赖关系

```
基础指令集（RV64I）
  ↓
M扩展（乘除）
  ↓
A扩展（原子）
  ↓
F扩展（单精度浮点）
  ↓
D扩展（双精度浮点）
  ↓
C扩展（压缩）
```

**说明**：
- M、A、F扩展可以独立实现
- D扩展依赖于F扩展
- C扩展可以独立实现
- 所有扩展都依赖于基础指令集（RV64I）

---

## 二、M扩展（整数乘除）详细实现

### 2.1 M扩展概述

**描述**：M扩展提供整数乘法和除法指令

**特权级**：用户模式（U）和机器模式（M）

**指令清单**（30个）：

| 指令 | 描述 | 格式 | 延迟 | 吞吐量 |
|------|------|------|------|--------|
| MUL | 乘法 | R-type | 3 | 1 |
| MULH | 乘法高32位 | R-type | 3 | 1 |
| MULHSU | 乘法高32位（有符号×无符号） | R-type | 3 | 1 |
| MULHU | 乘法高32位（无符号×无符号） | R-type | 3 | 1 |
| MULW | 乘法（32位） | R-type | 3 | 1 |
| DIV | 除法 | R-type | 10 | 4 |
| DIVU | 除法（无符号） | R-type | 10 | 4 |
| REM | 取余 | R-type | 10 | 4 |
| REMU | 取余（无符号） | R-type | 10 | 4 |
| DIVW | 除法（32位） | R-type | 10 | 4 |
| DIVUW | 除法（无符号，32位） | R-type | 10 | 4 |
| REMW | 取余（32位） | R-type | 10 | 4 |
| REMUW | 取余（无符号，32位） | R-type | 10 | 4 |
| （其他指令...） | - | - | - | - |

### 2.2 架构设计

```rust
// vm-ir/riscv_extensions/m_extension.rs

use std::collections::HashMap;
use crate::codegen::{InstructionFeatures, ExecutionUnit};

/// M扩展特征实现
pub struct MExtension {
    enabled: bool,
    features: HashMap<String, InstructionFeatures>,
}

impl MExtension {
    pub fn new() -> Self {
        let mut features = HashMap::new();
        
        // 初始化M扩展的指令特征
        Self::init_mul_features(&mut features);
        Self::init_div_features(&mut features);
        Self::init_rem_features(&mut features);
        
        MExtension {
            enabled: true,
            features,
        }
    }
    
    /// 初始化乘法指令特征
    fn init_mul_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("mul".to_string(), InstructionFeatures {
            latency: 3,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::Multiply],
        });
        
        features.insert("mulh".to_string(), InstructionFeatures {
            latency: 3,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::Multiply],
        });
        
        // ... 其他乘法指令
    }
    
    /// 初始化除法指令特征
    fn init_div_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("div".to_string(), InstructionFeatures {
            latency: 10,
            throughput: 4,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::Divide],
        });
        
        // ... 其他除法指令
    }
    
    /// 初始化取余指令特征
    fn init_rem_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("rem".to_string(), InstructionFeatures {
            latency: 10,
            throughput: 4,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::Divide],
        });
        
        // ... 其他取余指令
    }
}

impl RiscVExtension for MExtension {
    fn name(&self) -> &str {
        "M-Extension"
    }
    
    fn supported_instructions(&self) -> Vec<&str> {
        vec![
            "mul", "mulh", "mulhsu", "mulhu",
            "div", "divu",
            "rem", "remu",
            // ... 其他指令
        ]
    }
    
    fn initialize(&mut self, features: &mut HashMap<String, InstructionFeatures>) 
        -> Result<(), VmError> 
    {
        if !self.enabled {
            return Err(VmError::ExtensionDisabled("M-Extension is disabled".to_string()));
        }
        
        // 将M扩展的指令特征添加到全局特征表
        for (name, feature) in self.features.iter() {
            features.insert(name.clone(), feature.clone());
        }
        
        Ok(())
    }
}
```

### 2.3 实施步骤

#### 第1周：乘法指令实现

1. **创建m_extension.rs模块**
   ```bash
   touch vm-ir/src/riscv_extensions/m_extension.rs
   ```

2. **实现MExtension结构**
   - 定义MExtension结构
   - 实现特征接口
   - 实现初始化方法

3. **实现乘法指令特征**（第1-2天）
   - MUL: 乘法指令
   - MULH: 乘法高32位
   - MULHSU: 乘法高32位（有符号×无符号）
   - MULHU: 乘法高32位（无符号×无符号）
   - MULW: 乘法（32位）

4. **编写测试**（第3-5天）
   - 单元测试：每个乘法指令
   - 集成测试：乘法操作正确性
   - 性能测试：乘法性能

#### 第2周：除法和取余指令实现

1. **实现除法指令特征**（第1-2天）
   - DIV: 除法
   - DIVU: 除法（无符号）
   - DIVW: 除法（32位）
   - DIVUW: 除法（无符号，32位）

2. **实现取余指令特征**（第3-4天）
   - REM: 取余
   - REMU: 取余（无符号）
   - REMW: 取余（32位）
   - REMUW: 取余（无符号，32位）

3. **编写测试**（第5天）
   - 单元测试：每个除法和取余指令
   - 集成测试：除法和取余操作正确性
   - 性能测试：除法和取余性能

### 2.4 测试计划

#### 单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    #[test]
    fn test_mul_instruction_features() {
        let mut features = HashMap::new();
        let m_ext = MExtension::new();
        m_ext.initialize(&mut features).unwrap();
        
        // 测试MUL指令特征
        let mul_feature = features.get("mul").unwrap();
        assert_eq!(mul_feature.latency, 3);
        assert_eq!(mul_feature.throughput, 1);
        assert_eq!(mul_feature.size, 4);
        assert!(!mul_feature.is_micro_op);
        assert!(mul_feature.execution_units.contains(&ExecutionUnit::Multiply));
    }
    
    #[test]
    fn test_div_instruction_features() {
        let mut features = HashMap::new();
        let m_ext = MExtension::new();
        m_ext.initialize(&mut features).unwrap();
        
        // 测试DIV指令特征
        let div_feature = features.get("div").unwrap();
        assert_eq!(div_feature.latency, 10);
        assert_eq!(div_feature.throughput, 4);
        assert_eq!(div_feature.size, 4);
        assert!(!div_feature.is_micro_op);
        assert!(div_feature.execution_units.contains(&ExecutionUnit::Divide));
    }
    
    // ... 其他测试
}
```

#### 集成测试
```rust
#[test]
fn test_m_extension_integration() {
    let mut features = HashMap::new();
    let m_ext = MExtension::new();
    
    // 初始化M扩展
    assert!(m_ext.initialize(&mut features).is_ok());
    
    // 验证所有M扩展指令都已添加
    for instruction in m_ext.supported_instructions() {
        assert!(features.contains_key(instruction));
    }
}
```

#### 性能测试
```rust
#[bench]
fn bench_mul_performance(b: &mut Bencher) {
    let mut features = HashMap::new();
    let m_ext = MExtension::new();
    m_ext.initialize(&mut features).unwrap();
    
    b.iter(|| {
        // 测试乘法性能
        let _ = features.get("mul").unwrap();
    });
}
```

---

## 三、A扩展（原子操作）详细实现

### 3.1 A扩展概述

**描述**：A扩展提供原子读-改写（LR/SC）和原子算术、逻辑操作

**特权级**：用户模式（U）和机器模式（M）

**指令清单**（20个）：

| 指令 | 描述 | 格式 | 延迟 | 吞吐量 |
|------|------|------|------|--------|
| LR.W | 加载保留（32位） | R-type | 5 | 1 |
| SC.W | 存储条件（32位） | R-type | 5 | 1 |
| LR.D | 加载保留（64位） | R-type | 6 | 1 |
| SC.D | 存储条件（64位） | R-type | 6 | 1 |
| AMOSWAP.W | 原子交换（32位） | R-type | 6 | 1 |
| AMOADD.W | 原子加（32位） | R-type | 6 | 1 |
| AMOXOR.W | 原子异或（32位） | R-type | 6 | 1 |
| AMOAND.W | 原子与（32位） | R-type | 6 | 1 |
| AMOOR.W | 原子或（32位） | R-type | 6 | 1 |
| AMOMIN.W | 原子最小值（32位） | R-type | 6 | 1 |
| AMOMAX.W | 原子最大值（32位） | R-type | 6 | 1 |
| （其他指令...） | - | - | - | - |

### 3.2 架构设计

```rust
// vm-ir/riscv_extensions/a_extension.rs

use std::collections::HashMap;
use crate::codegen::{InstructionFeatures, ExecutionUnit};

/// A扩展特征实现
pub struct AExtension {
    enabled: bool,
    features: HashMap<String, InstructionFeatures>,
}

impl AExtension {
    pub fn new() -> Self {
        let mut features = HashMap::new();
        
        // 初始化A扩展的指令特征
        Self::init_lr_sc_features(&mut features);
        Self::init_amo_features(&mut features);
        
        AExtension {
            enabled: true,
            features,
        }
    }
    
    /// 初始化LR/SC指令特征
    fn init_lr_sc_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("lr.w".to_string(), InstructionFeatures {
            latency: 5,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::LoadStore],
        });
        
        features.insert("sc.w".to_string(), InstructionFeatures {
            latency: 5,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: vec!["lr.w".to_string()],
            execution_units: vec![ExecutionUnit::LoadStore],
        });
        
        // ... 其他LR/SC指令
    }
    
    /// 初始化原子操作指令特征
    fn init_amo_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("amoswap.w".to_string(), InstructionFeatures {
            latency: 6,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::LoadStore],
        });
        
        features.insert("amoadd.w".to_string(), InstructionFeatures {
            latency: 6,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::LoadStore],
        });
        
        // ... 其他原子操作指令
    }
}

impl RiscVExtension for AExtension {
    fn name(&self) -> &str {
        "A-Extension"
    }
    
    fn supported_instructions(&self) -> Vec<&str> {
        vec![
            "lr.w", "sc.w", "lr.d", "sc.d",
            "amoswap.w", "amoadd.w", "amoxor.w", "amoand.w",
            "amoor.w", "amomin.w", "amomax.w",
            // ... 其他指令
        ]
    }
    
    fn initialize(&mut self, features: &mut HashMap<String, InstructionFeatures>) 
        -> Result<(), VmError> 
    {
        if !self.enabled {
            return Err(VmError::ExtensionDisabled("A-Extension is disabled".to_string()));
        }
        
        // 将A扩展的指令特征添加到全局特征表
        for (name, feature) in self.features.iter() {
            features.insert(name.clone(), feature.clone());
        }
        
        Ok(())
    }
}
```

### 3.3 实施步骤

#### 第1周：LR/SC指令实现

1. **创建a_extension.rs模块**
   ```bash
   touch vm-ir/src/riscv_extensions/a_extension.rs
   ```

2. **实现AExtension结构**
   - 定义AExtension结构
   - 实现特征接口
   - 实现初始化方法

3. **实现LR/SC指令特征**（第1-3天）
   - LR.W: 加载保留（32位）
   - SC.W: 存储条件（32位）
   - LR.D: 加载保留（64位）
   - SC.D: 存储条件（64位）

4. **编写测试**（第4-5天）
   - 单元测试：每个LR/SC指令
   - 集成测试：原子操作正确性
   - 性能测试：LR/SC性能

#### 第1.5周：原子操作指令实现

1. **实现原子操作指令特征**（第1-3天）
   - AMOSWAP.W: 原子交换（32位）
   - AMOADD.W: 原子加（32位）
   - AMOXOR.W: 原子异或（32位）
   - AMOAND.W: 原子与（32位）
   - AMOOR.W: 原子或（32位）
   - AMOMIN.W: 原子最小值（32位）
   - AMOMAX.W: 原子最大值（32位）

2. **编写测试**（第4-5天）
   - 单元测试：每个原子操作指令
   - 集成测试：原子操作正确性
   - 性能测试：原子操作性能

---

## 四、F扩展（单精度浮点）详细实现

### 4.1 F扩展概述

**描述**：F扩展提供单精度浮点运算

**特权级**：用户模式（U）和机器模式（M）

**指令清单**（40个）：

| 指令 | 描述 | 格式 | 延迟 | 吞吐量 |
|------|------|------|------|--------|
| FLW | 浮点加载字 | I-type | 3 | 1 |
| FSW | 浮点存储字 | S-type | 2 | 1 |
| FADD.S | 浮点加法（单精度） | R-type | 4 | 1 |
| FSUB.S | 浮点减法（单精度） | R-type | 4 | 1 |
| FMUL.S | 浮点乘法（单精度） | R-type | 5 | 1 |
| FDIV.S | 浮点除法（单精度） | R-type | 15 | 5 |
| FSQRT.S | 浮点平方根（单精度） | R-type | 20 | 10 |
| FMIN.S | 浮点最小值（单精度） | R-type | 3 | 1 |
| FMAX.S | 浮点最大值（单精度） | R-type | 3 | 1 |
| （其他指令...） | - | - | - | - |

### 4.2 架构设计

```rust
// vm-ir/riscv_extensions/f_extension.rs

use std::collections::HashMap;
use crate::codegen::{InstructionFeatures, ExecutionUnit};

/// F扩展特征实现
pub struct FExtension {
    enabled: bool,
    features: HashMap<String, InstructionFeatures>,
}

impl FExtension {
    pub fn new() -> Self {
        let mut features = HashMap::new();
        
        // 初始化F扩展的指令特征
        Self::init_load_store_features(&mut features);
        Self::init_arithmetic_features(&mut features);
        Self::init_comparison_features(&mut features);
        
        FExtension {
            enabled: true,
            features,
        }
    }
    
    /// 初始化加载/存储指令特征
    fn init_load_store_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("flw".to_string(), InstructionFeatures {
            latency: 3,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::LoadStore],
        });
        
        features.insert("fsw".to_string(), InstructionFeatures {
            latency: 2,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::LoadStore],
        });
    }
    
    /// 初始化算术指令特征
    fn init_arithmetic_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("fadd.s".to_string(), InstructionFeatures {
            latency: 4,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::FPU],
        });
        
        features.insert("fsub.s".to_string(), InstructionFeatures {
            latency: 4,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::FPU],
        });
        
        // ... 其他算术指令
    }
    
    /// 初始化比较指令特征
    fn init_comparison_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("fmin.s".to_string(), InstructionFeatures {
            latency: 3,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::FPU],
        });
        
        // ... 其他比较指令
    }
}

impl RiscVExtension for FExtension {
    fn name(&self) -> &str {
        "F-Extension"
    }
    
    fn supported_instructions(&self) -> Vec<&str> {
        vec![
            "flw", "fsw",
            "fadd.s", "fsub.s", "fmul.s", "fdiv.s",
            "fsqrt.s", "fmin.s", "fmax.s",
            // ... 其他指令
        ]
    }
    
    fn initialize(&mut self, features: &mut HashMap<String, InstructionFeatures>) 
        -> Result<(), VmError> 
    {
        if !self.enabled {
            return Err(VmError::ExtensionDisabled("F-Extension is disabled".to_string()));
        }
        
        // 将F扩展的指令特征添加到全局特征表
        for (name, feature) in self.features.iter() {
            features.insert(name.clone(), feature.clone());
        }
        
        Ok(())
    }
}
```

### 4.3 实施步骤

#### 第1周：加载/存储指令实现

1. **创建f_extension.rs模块**
   ```bash
   touch vm-ir/src/riscv_extensions/f_extension.rs
   ```

2. **实现FExtension结构**
   - 定义FExtension结构
   - 实现特征接口
   - 实现初始化方法

3. **实现加载/存储指令特征**（第1-2天）
   - FLW: 浮点加载字
   - FSW: 浮点存储字

4. **编写测试**（第3-5天）
   - 单元测试：每个加载/存储指令
   - 集成测试：浮点加载/存储正确性
   - 性能测试：浮点加载/存储性能

#### 第2周：算术和比较指令实现

1. **实现算术指令特征**（第1-3天）
   - FADD.S: 浮点加法
   - FSUB.S: 浮点减法
   - FMUL.S: 浮点乘法
   - FDIV.S: 浮点除法
   - FSQRT.S: 浮点平方根

2. **实现比较指令特征**（第4-5天）
   - FMIN.S: 浮点最小值
   - FMAX.S: 浮点最大值
   - FEQ.S: 浮点相等
   - FLT.S: 浮点小于
   - FLE.S: 浮点小于等于

3. **编写测试**（第5天）
   - 单元测试：每个算术和比较指令
   - 集成测试：浮点操作正确性
   - 性能测试：浮点性能

---

## 五、D扩展（双精度浮点）详细实现

### 5.1 D扩展概述

**描述**：D扩展提供双精度浮点运算，依赖于F扩展

**特权级**：用户模式（U）和机器模式（M）

**指令清单**（30个）：

| 指令 | 描述 | 格式 | 延迟 | 吞吐量 |
|------|------|------|------|--------|
| FLD | 浮点加载双 | I-type | 3 | 1 |
| FSD | 浮点存储双 | S-type | 2 | 1 |
| FADD.D | 浮点加法（双精度） | R-type | 5 | 1 |
| FSUB.D | 浮点减法（双精度） | R-type | 5 | 1 |
| FMUL.D | 浮点乘法（双精度） | R-type | 6 | 1 |
| FDIV.D | 浮点除法（双精度） | R-type | 20 | 8 |
| FSQRT.D | 浮点平方根（双精度） | R-type | 25 | 12 |
| FMAX.D | 浮点最大值（双精度） | R-type | 4 | 1 |
| FMIN.D | 浮点最小值（双精度） | R-type | 4 | 1 |
| （其他指令...） | - | - | - | - |

### 5.2 架构设计

```rust
// vm-ir/riscv_extensions/d_extension.rs

use std::collections::HashMap;
use crate::codegen::{InstructionFeatures, ExecutionUnit};

/// D扩展特征实现
pub struct DExtension {
    enabled: bool,
    features: HashMap<String, InstructionFeatures>,
}

impl DExtension {
    pub fn new() -> Self {
        let mut features = HashMap::new();
        
        // 初始化D扩展的指令特征
        Self::init_load_store_features(&mut features);
        Self::init_arithmetic_features(&mut features);
        Self::init_conversion_features(&mut features);
        
        DExtension {
            enabled: true,
            features,
        }
    }
    
    /// 初始化加载/存储指令特征
    fn init_load_store_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("fld".to_string(), InstructionFeatures {
            latency: 3,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::LoadStore],
        });
        
        features.insert("fsd".to_string(), InstructionFeatures {
            latency: 2,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::LoadStore],
        });
    }
    
    /// 初始化算术指令特征
    fn init_arithmetic_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("fadd.d".to_string(), InstructionFeatures {
            latency: 5,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::FPU],
        });
        
        features.insert("fsub.d".to_string(), InstructionFeatures {
            latency: 5,
            throughput: 1,
            size: 4,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::FPU],
        });
        
        // ... 其他算术指令
    }
    
    /// 初始化转换指令特征
    fn init_conversion_features(features: &mut HashMap<String, InstructionFeatures>) {
        // FCVT.S.D: 单精度转双精度
        // FCVT.D.S: 双精度转单精度
        // ... 其他转换指令
    }
}

impl RiscVExtension for DExtension {
    fn name(&self) -> &str {
        "D-Extension"
    }
    
    fn supported_instructions(&self) -> Vec<&str> {
        vec![
            "fld", "fsd",
            "fadd.d", "fsub.d", "fmul.d", "fdiv.d",
            "fsqrt.d", "fmin.d", "fmax.d",
            // ... 其他指令
        ]
    }
    
    fn initialize(&mut self, features: &mut HashMap<String, InstructionFeatures>) 
        -> Result<(), VmError> 
    {
        if !self.enabled {
            return Err(VmError::ExtensionDisabled("D-Extension is disabled".to_string()));
        }
        
        // 将D扩展的指令特征添加到全局特征表
        for (name, feature) in self.features.iter() {
            features.insert(name.clone(), feature.clone());
        }
        
        Ok(())
    }
}
```

### 5.3 实施步骤

#### 第1周：加载/存储指令实现

1. **创建d_extension.rs模块**
   ```bash
   touch vm-ir/src/riscv_extensions/d_extension.rs
   ```

2. **实现DExtension结构**
   - 定义DExtension结构
   - 实现特征接口
   - 实现初始化方法
   - **检查F扩展依赖**

3. **实现加载/存储指令特征**（第1-2天）
   - FLD: 浮点加载双
   - FSD: 浮点存储双

4. **编写测试**（第3-5天）
   - 单元测试：每个加载/存储指令
   - 集成测试：双精度浮点加载/存储正确性
   - 性能测试：双精度浮点加载/存储性能

#### 第1.5周：算术和转换指令实现

1. **实现算术指令特征**（第1-3天）
   - FADD.D: 浮点加法
   - FSUB.D: 浮点减法
   - FMUL.D: 浮点乘法
   - FDIV.D: 浮点除法
   - FSQRT.D: 浮点平方根

2. **实现转换指令特征**（第4-5天）
   - FCVT.S.D: 单精度转双精度
   - FCVT.D.S: 双精度转单精度
   - FCVT.W.D: 双精度转整数
   - FCVT.D.W: 整数转双精度

3. **编写测试**（第5天）
   - 单元测试：每个算术和转换指令
   - 集成测试：双精度浮点操作正确性
   - 性能测试：双精度浮点性能

---

## 六、C扩展（压缩指令）详细实现

### 6.1 C扩展概述

**描述**：C扩展提供16位压缩指令，减少代码大小

**特权级**：用户模式（U）和机器模式（M）

**指令清单**（50个）：

| 指令 | 描述 | 格式 | 延迟 | 吞吐量 |
|------|------|------|------|--------|
| C.ADDI | 压缩加立即数 | CI-type | 1 | 1 |
| C.ADD | 压缩加法 | CR-type | 1 | 1 |
| C.SUB | 压缩减法 | CR-type | 1 | 1 |
| C.LW | 压缩加载字 | CI-type | 3 | 1 |
| C.SW | 压缩存储字 | CS-type | 2 | 1 |
| C.BEQZ | 压缩分支如果为零 | CB-type | 2 | 1 |
| C.J | 压缩跳转 | CJ-type | 2 | 1 |
| （其他指令...） | - | - | - | - |

### 6.2 架构设计

```rust
// vm-ir/riscv_extensions/c_extension.rs

use std::collections::HashMap;
use crate::codegen::{InstructionFeatures, ExecutionUnit};

/// C扩展特征实现
pub struct CExtension {
    enabled: bool,
    features: HashMap<String, InstructionFeatures>,
}

impl CExtension {
    pub fn new() -> Self {
        let mut features = HashMap::new();
        
        // 初始化C扩展的指令特征
        Self::init_arithmetic_features(&mut features);
        Self::init_load_store_features(&mut features);
        Self::init_branch_features(&mut features);
        
        CExtension {
            enabled: true,
            features,
        }
    }
    
    /// 初始化算术指令特征
    fn init_arithmetic_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("c.addi".to_string(), InstructionFeatures {
            latency: 1,
            throughput: 1,
            size: 2,  // 16位压缩
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::ALU],
        });
        
        features.insert("c.add".to_string(), InstructionFeatures {
            latency: 1,
            throughput: 1,
            size: 2,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::ALU],
        });
        
        features.insert("c.sub".to_string(), InstructionFeatures {
            latency: 1,
            throughput: 1,
            size: 2,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::ALU],
        });
        
        // ... 其他算术指令
    }
    
    /// 初始化加载/存储指令特征
    fn init_load_store_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("c.lw".to_string(), InstructionFeatures {
            latency: 3,
            throughput: 1,
            size: 2,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::LoadStore],
        });
        
        features.insert("c.sw".to_string(), InstructionFeatures {
            latency: 2,
            throughput: 1,
            size: 2,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::LoadStore],
        });
        
        // ... 其他加载/存储指令
    }
    
    /// 初始化分支指令特征
    fn init_branch_features(features: &mut HashMap<String, InstructionFeatures>) {
        features.insert("c.beqz".to_string(), InstructionFeatures {
            latency: 2,
            throughput: 1,
            size: 2,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::Branch],
        });
        
        features.insert("c.j".to_string(), InstructionFeatures {
            latency: 2,
            throughput: 1,
            size: 2,
            is_micro_op: false,
            dependencies: Vec::new(),
            execution_units: vec![ExecutionUnit::Branch],
        });
        
        // ... 其他分支指令
    }
}

impl RiscVExtension for CExtension {
    fn name(&self) -> &str {
        "C-Extension"
    }
    
    fn supported_instructions(&self) -> Vec<&str> {
        vec![
            "c.addi", "c.add", "c.sub",
            "c.lw", "c.sw",
            "c.beqz", "c.j",
            // ... 其他指令
        ]
    }
    
    fn initialize(&mut self, features: &mut HashMap<String, InstructionFeatures>) 
        -> Result<(), VmError> 
    {
        if !self.enabled {
            return Err(VmError::ExtensionDisabled("C-Extension is disabled".to_string()));
        }
        
        // 将C扩展的指令特征添加到全局特征表
        for (name, feature) in self.features.iter() {
            features.insert(name.clone(), feature.clone());
        }
        
        Ok(())
    }
}
```

### 6.3 实施步骤

#### 第1周：算术指令实现

1. **创建c_extension.rs模块**
   ```bash
   touch vm-ir/src/riscv_extensions/c_extension.rs
   ```

2. **实现CExtension结构**
   - 定义CExtension结构
   - 实现特征接口
   - 实现初始化方法

3. **实现算术指令特征**（第1-3天）
   - C.ADDI: 压缩加立即数
   - C.ADD: 压缩加
   - C.SUB: 压缩减

4. **编写测试**（第4-5天）
   - 单元测试：每个算术指令
   - 集成测试：压缩算术操作正确性
   - 性能测试：压缩指令性能

#### 第2周：加载/存储和分支指令实现

1. **实现加载/存储指令特征**（第1-3天）
   - C.LW: 压缩加载字
   - C.SW: 压缩存储字
   - ... 其他加载/存储指令

2. **实现分支指令特征**（第4-5天）
   - C.BEQZ: 压缩分支如果为零
   - C.J: 压缩跳转
   - ... 其他分支指令

3. **编写测试**（第5天）
   - 单元测试：每个加载/存储和分支指令
   - 集成测试：压缩指令正确性
   - 性能测试：压缩指令性能

---

## 七、整合测试和验证

### 7.1 集成测试框架

```rust
// vm-ir/riscv_extensions/tests/integration_tests.rs

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::collections::HashMap;
    
    #[test]
    fn test_all_extensions_integration() {
        let mut features = HashMap::new();
        
        // 初始化所有扩展
        let m_ext = MExtension::new();
        let a_ext = AExtension::new();
        let f_ext = FExtension::new();
        let d_ext = DExtension::new();
        let c_ext = CExtension::new();
        
        m_ext.initialize(&mut features).unwrap();
        a_ext.initialize(&mut features).unwrap();
        f_ext.initialize(&mut features).unwrap();
        d_ext.initialize(&mut features).unwrap();
        c_ext.initialize(&mut features).unwrap();
        
        // 验证所有指令都已添加
        assert!(features.contains_key("mul"));
        assert!(features.contains_key("lr.w"));
        assert!(features.contains_key("fadd.s"));
        assert!(features.contains_key("fadd.d"));
        assert!(features.contains_key("c.add"));
        
        println!("Total instructions: {}", features.len());
    }
    
    #[test]
    fn test_extension_dependencies() {
        // 测试扩展依赖关系
        // D扩展需要F扩展
        // C扩展独立
        // ...
    }
}
```

### 7.2 性能基准测试

```rust
// vm-ir/riscv_extensions/benches/benchmarks.rs

use std::collections::HashMap;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_all_extensions(c: &mut Criterion) {
    let mut features = HashMap::new();
    
    // 初始化所有扩展
    let m_ext = MExtension::new();
    let a_ext = AExtension::new();
    let f_ext = FExtension::new();
    let d_ext = DExtension::new();
    let c_ext = CExtension::new();
    
    m_ext.initialize(&mut features).unwrap();
    a_ext.initialize(&mut features).unwrap();
    f_ext.initialize(&mut features).unwrap();
    d_ext.initialize(&mut features).unwrap();
    c_ext.initialize(&mut features).unwrap();
    
    c.bench_function("mul_instruction", |b| {
        b.iter(|| {
            black_box(features.get("mul").unwrap());
        });
    });
    
    c.bench_function("lr.w_instruction", |b| {
        b.iter(|| {
            black_box(features.get("lr.w").unwrap());
        });
    });
    
    c.bench_function("fadd.s_instruction", |b| {
        b.iter(|| {
            black_box(features.get("fadd.s").unwrap());
        });
    });
    
    c.bench_function("fadd.d_instruction", |b| {
        b.iter(|| {
            black_box(features.get("fadd.d").unwrap());
        });
    });
    
    c.bench_function("c.add_instruction", |b| {
        b.iter(|| {
            black_box(features.get("c.add").unwrap());
        });
    });
}

criterion_group!(benches, bench_all_extensions);
criterion_main!(benches);
```

---

## 八、实施时间表

### 8.1 总体时间表

| 阶段 | 扩展 | 工作内容 | 时间 |
|------|------|---------|------|
| 阶段1 | M扩展 | 30个指令 | 2周 |
| 阶段2 | A扩展 | 20个指令 | 1.5周 |
| 阶段3 | F扩展 | 40个指令 | 2周 |
| 阶段4 | D扩展 | 30个指令 | 1.5周 |
| 阶段5 | C扩展 | 50个指令 | 2周 |
| **总计** | **5个扩展** | **~170个指令** | **9周** |

### 8.2 详细工作分解

| 周 | 扩展 | 主要任务 | 交付物 |
|----|------|---------|-------|
| 第1-2周 | M扩展 | 30个指令特征、测试 | M扩展模块 |
| 第3-4.5周 | A扩展 | 20个指令特征、测试 | A扩展模块 |
| 第5-7周 | F扩展 | 40个指令特征、测试 | F扩展模块 |
| 第8-9.5周 | D扩展 | 30个指令特征、测试 | D扩展模块 |
| 第10-12周 | C扩展 | 50个指令特征、测试 | C扩展模块 |
| 第13-16周 | 集成测试 | 集成测试、性能测试 | 测试报告 |

---

## 九、成功标准

### 9.1 代码质量标准

- [ ] 所有扩展模块实现完成
- [ ] 所有指令特征添加完成
- [ ] 所有单元测试通过
- [ ] 所有集成测试通过
- [ ] 代码覆盖率 >85%

### 9.2 性能标准

- [ ] M扩展指令性能符合预期
- [ ] A扩展原子操作性能符合预期
- [ ] F扩展浮点性能符合预期
- [ ] D扩展双精度浮点性能符合预期
- [ ] C扩展指令性能符合预期

### 9.3 文档标准

- [ ] 每个扩展都有详细文档
- [ ] 指令特征都有说明
- [ ] 测试都有文档
- [ ] 性能指标都有记录

---

## 十、风险和缓解

### 10.1 主要风险

#### 风险1：扩展依赖关系
**风险描述**：D扩展依赖于F扩展
**影响**：需要先实现F扩展
**缓解措施**：
- 按依赖顺序实现扩展（F → D）
- 在D扩展中检查F扩展的可用性

#### 风险2：指令数量庞大
**风险描述**：5个扩展共170+个指令
**影响**：实施时间长
**缓解措施**：
- 分阶段实施（每个扩展1-2周）
- 优先实现常用指令
- 自动化测试

#### 风险3：性能优化复杂
**风险描述**：浮点和原子操作性能优化复杂
**影响**：可能需要额外的优化时间
**缓解措施**：
- 参考其他RISC-V实现（如QEMU, Spike）
- 使用性能分析工具
- 分阶段优化

---

## 十一、总结

### 11.1 实施范围

**扩展数量**：5个（M, A, F, D, C）
**指令总数**：~170个
**预计时间**：9周（包括测试）
**预计代码量**：~5,000行

### 11.2 交付物

1. **代码模块**：
   - vm-ir/src/riscv_extensions/m_extension.rs
   - vm-ir/src/riscv_extensions/a_extension.rs
   - vm-ir/src/riscv_extensions/f_extension.rs
   - vm-ir/src/riscv_extensions/d_extension.rs
   - vm-ir/src/riscv_extensions/c_extension.rs

2. **测试代码**：
   - 单元测试：每个扩展
   - 集成测试：所有扩展
   - 性能测试：所有扩展

3. **文档**：
   - 每个扩展的详细文档
   - 指令特征说明
   - 测试文档
   - 性能报告

### 11.3 预期结果

- [ ] RISC-V功能完整度：从35%提升至80%
- [ ] 指令集支持：从16个扩展至186+个
- [ ] 代码质量：通过所有测试
- [ ] 性能：符合预期性能指标

---

**实现指南创建时间**：2024年12月24日
**预计实施时间**：9周（约2个月）
**预计新增代码**：~5,000行
**预计新增测试**：~200个测试用例

**建议**：按照实施时间表逐步实现各个扩展，先实现M和A扩展（高优先级），然后实现F和D扩展（高优先级），最后实现C扩展（低优先级）。每个扩展完成后进行测试验证，确保质量和性能。

