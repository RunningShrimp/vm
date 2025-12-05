# vm-cross-arch - 跨架构指令转换层

实现不同架构之间等价转换层，支持AMD64、ARM64、RISC-V64架构指令之间的转换。

## 🚀 新功能：自动跨架构执行

**现在支持自动检测host/guest架构并运行跨架构操作系统！**

- ✅ ARM64架构上运行AMD64操作系统
- ✅ AMD64架构上运行ARM64操作系统  
- ✅ 自动检测并配置
- ✅ 自动选择执行策略

## 功能特性

- ✅ **多架构支持**: 支持AMD64、ARM64、RISC-V64三种架构
- ✅ **双向转换**: 支持任意两种架构之间的相互转换
- ✅ **统一IR**: 基于统一的中间表示(IR)进行转换
- ✅ **寄存器映射**: 自动处理不同架构之间的寄存器映射
- ✅ **指令编码**: 将IR操作编码为目标架构的机器码

## 架构设计

```
源架构指令 (AMD64/ARM64/RISC-V64)
    ↓ (解码器)
统一IR (vm-ir::IRBlock)
    ↓ (架构编码器)
目标架构指令 (AMD64/ARM64/RISC-V64)
```

## 使用示例

### 基本转换

```rust
use vm_cross_arch::{ArchTranslator, SourceArch, TargetArch};
use vm_ir::{IRBuilder, IROp, Terminator};

// 创建转换器：从x86-64转换到ARM64
let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);

// 构建IR块
let mut builder = IRBuilder::new(0x1000);
builder.push(IROp::Add {
    dst: 0,
    src1: 1,
    src2: 2,
});
builder.set_term(Terminator::Ret);
let block = builder.build();

// 转换
let result = translator.translate_block(&block)?;

// 获取转换后的指令
for insn in result.instructions {
    println!("{}: {:?}", insn.mnemonic, insn.bytes);
}
```

### 支持的转换方向

- AMD64 → ARM64
- AMD64 → RISC-V64
- ARM64 → AMD64
- ARM64 → RISC-V64
- RISC-V64 → AMD64
- RISC-V64 → ARM64

## 支持的IR操作

### 算术运算
- `Add`, `Sub`, `Mul`, `Div`
- `AddImm`, `MulImm`

### 逻辑运算
- `And`, `Or`, `Xor`, `Not`

### 内存操作
- `Load`, `Store`

### SIMD向量操作
- `VecAdd`, `VecSub`, `VecMul` - 向量加/减/乘
- 支持不同元素大小（1/2/4/8字节）

### 浮点运算
- **双精度**: `Fadd`, `Fsub`, `Fmul`, `Fdiv`, `Fsqrt`
- **单精度**: `FaddS`, `FsubS`, `FmulS`, `FdivS`, `FsqrtS`

### 原子操作
- `AtomicRMW` - 原子读-修改-写操作（Add/Sub/And/Or/Xor/Xchg）
- `AtomicCmpXchg` - 原子比较并交换
- `AtomicLoadReserve` / `AtomicStoreCond` - RISC-V LR/SC 或 ARM64 LDXR/STXR

### 控制流
- `Terminator::Ret` - 返回
- `Terminator::Jmp` - 无条件跳转
- `Terminator::CondJmp` - 条件跳转

## 实现细节

### 寄存器映射

不同架构的寄存器数量不同：
- **x86-64**: 16个通用寄存器 (RAX, RBX, RCX, RDX, RSI, RDI, RBP, RSP, R8-R15)
- **ARM64**: 32个通用寄存器 (X0-X30, SP)
- **RISC-V64**: 32个通用寄存器 (X0-X31)

转换器会自动映射寄存器编号，确保语义正确性。

### 立即数处理

不同架构对立即数的支持范围不同：
- **x86-64**: 支持64位立即数
- **ARM64**: 12位立即数，大立即数需要MOVZ/MOVK组合
- **RISC-V64**: 12位立即数，大立即数需要LUI+ADDI组合

转换器会自动处理大立即数，生成多条指令序列。

### 跳转指令

不同架构的跳转指令编码方式不同：
- **x86-64**: 相对偏移量（32位）
- **ARM64**: 相对偏移量（26位，4字节对齐）
- **RISC-V64**: 相对偏移量（20位，2字节对齐）

转换器会计算正确的偏移量，如果超出范围会返回错误。

## 测试

运行测试：

```bash
cargo test --package vm-cross-arch
```

## 实现状态

### ✅ 已实现功能

1. **基础算术和逻辑运算**: Add, Sub, AddImm, MovImm
2. **内存操作**: Load, Store
3. **SIMD向量操作**: 
   - 基础运算: VecAdd, VecSub, VecMul（支持x86-64 SSE2、ARM64 NEON、RISC-V向量扩展）
   - 饱和运算: VecAddSat, VecSubSat（支持有符号/无符号）
4. **浮点运算**: 
   - 基础运算: 单精度和双精度的加/减/乘/除/平方根
   - 融合乘加: Fmadd, Fmsub, Fnmadd, Fnmsub（单精度和双精度）
   - 比较操作: Feq, Flt, Fle（单精度和双精度）
   - 最小/最大: Fmin, Fmax（单精度和双精度）
   - 绝对值/取反: Fabs, Fneg（单精度和双精度）
   - 类型转换: Fcvtws, Fcvtsw, Fcvtld, Fcvtdl, Fcvtsd, Fcvtds
   - 加载/存储: Fload, Fstore
5. **原子操作**: AtomicRMW, AtomicCmpXchg, LR/SC模式

### 📊 实现统计

- **总代码行数**: 3400+ 行
- **编码函数**: 100+ 个
- **支持的操作类型**: 60+ 种
- **架构支持**: x86-64, ARM64, RISC-V64（完整支持）

### 📋 未来计划

- [ ] 实现更多SIMD操作（VecMulSat、混合、置换等）
- [ ] 实现大向量操作（Vec128Add, Vec256Add等）
- [ ] 实现浮点符号操作（Fsgnj/Fsgnjn/Fsgnjx）
- [ ] 实现浮点分类操作（Fclass）
- [ ] 优化指令序列生成（减少指令数量）
- [ ] 支持指令模式识别和优化
- [ ] 实现更复杂的原子操作（Min/Max等）

## 许可证

本项目采用 MIT 或 Apache-2.0 许可证。

