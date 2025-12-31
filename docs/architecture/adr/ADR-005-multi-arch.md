# ADR-005: 多架构支持策略

## 状态
已接受 (2024-12-31)

## 上下文
VM项目需要支持多种CPU架构：
- RISC-V 64位
- ARM64 (AArch64)
- x86-64

## 设计原则

### 架构无关的IR层

```
Guest二进制
    ↓
架构特定解码器 (RISC-V/ARM/x86)
    ↓
统一IR (与架构无关)
    ↓
执行引擎 (解释器/JIT)
    ↓
Host执行
```

### 统一接口

```rust
// 所有架构解码器实现同一trait
pub trait Decoder {
    type Instruction;
    type Block;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Instruction>;
    
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Block>;
}
```

## 架构实现

### RISC-V 64位

```rust
pub struct RiscvDecoder {
    insn_cache: LruCache<GuestAddr, RiscvInstruction>,
}

impl Decoder for RiscvDecoder {
    type Instruction = RiscvInstruction;
    type Block = IRBlock;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Instruction>
    {
        let insn_word = mmu.fetch_insn(pc)? as u32;
        
        // RISC-V指令解码
        let opcode = insn_word & 0x7F;
        match opcode {
            0x33 => self.decode_r_type(insn_word),
            0x13 => self.decode_i_type(insn_word),
            0x63 => self.decode_b_type(insn_word),
            // ...
            _ => Err(VmError::Execution(
                ExecutionError::Fault(Fault::InvalidOpcode {
                    pc, opcode: insn_word
                })
            )),
        }
    }
}
```

### ARM64

```rust
pub struct Arm64Decoder {
    insn_cache: LruCache<GuestAddr, Arm64Instruction>,
}

impl Decoder for Arm64Decoder {
    type Instruction = Arm64Instruction;
    type Block = IRBlock;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Instruction>
    {
        let insn_word = mmu.fetch_insn(pc)? as u32;
        
        // ARM64指令解码
        let op0 = (insn_word >> 25) & 0xF;
        match op0 {
            0b1000 => self.decode_data_processing(insn_word),
            0b1010 => self.decode_branch(insn_word),
            0b1100 => self.decode_load_store(insn_word),
            // ...
            _ => Err(VmError::Execution(
                ExecutionError::Fault(Fault::InvalidOpcode {
                    pc, opcode: insn_word
                })
            )),
        }
    }
}
```

### x86-64

```rust
pub struct X86Decoder {
    insn_cache: LruCache<GuestAddr, X86Instruction>,
    prefix_table: HashMap<u8, PrefixHandler>,
}

impl Decoder for X86Decoder {
    type Instruction = X86Instruction;
    type Block = IRBlock;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Instruction>
    {
        // x86复杂解码（处理前缀、ModRM、SIB等）
        let mut offset = 0;
        
        // 读取前缀
        let prefixes = self.read_prefixes(mmu, pc, &mut offset)?;
        
        // 读取opcode
        let opcode = self.read_opcode(mmu, pc, &mut offset)?;
        
        // 解码指令
        match opcode {
            0x01 => self.decode_add_rm32_r32(mmu, pc, &mut offset, &prefixes),
            0x89 => self.decode_mov_rm32_r32(mmu, pc, &mut offset, &prefixes),
            // ...
            _ => Err(VmError::Execution(
                ExecutionError::Fault(Fault::InvalidOpcode {
                    pc, opcode: opcode as u32
                })
            )),
        }
    }
}
```

## IR设计

### 统一指令集

```rust
pub enum IRInstruction {
    // 算术运算
    Add { dst: Reg, src1: Reg, src2: Operand },
    Sub { dst: Reg, src1: Reg, src2: Operand },
    Mul { dst: Reg, src1: Reg, src2: Operand },
    Div { dst: Reg, src1: Reg, src2: Operand },

    // 逻辑运算
    And { dst: Reg, src1: Reg, src2: Operand },
    Or  { dst: Reg, src1: Reg, src2: Operand },
    Xor { dst: Reg, src1: Reg, src2: Operand },

    // 内存访问
    Load  { dst: Reg, addr: MemOperand, size: u8 },
    Store { src: Reg, addr: MemOperand, size: u8 },

    // 分支跳转
    Branch { cond: CondCode, target: GuestAddr },
    Jump   { target: GuestAddr },
    Call   { target: GuestAddr },
    Ret,
}

pub enum Operand {
    Reg(Reg),
    Imm(i64),
}

pub enum MemOperand {
    BaseDisp { base: Reg, disp: i64 },
    IndexScale { base: Reg, index: Reg, scale: u8 },
}
```

## 工厂模式

```rust
pub trait DecoderFactory {
    fn create(arch: GuestArch) -> Box<dyn Decoder>;
}

pub struct DecoderFactoryImpl;

impl DecoderFactory for DecoderFactoryImpl {
    fn create(arch: GuestArch) -> Box<dyn Decoder> {
        match arch {
            GuestArch::Riscv64 => Box::new(RiscvDecoder::new()),
            GuestArch::Arm64 => Box::new(Arm64Decoder::new()),
            GuestArch::X86_64 => Box::new(X86Decoder::new()),
            _ => panic!("Unsupported architecture: {:?}", arch),
        }
    }
}
```

## 扩展性

### 添加新架构

```rust
// 1. 定义解码器
pub struct PowerPCDecoder { /* ... */ }

impl Decoder for PowerPCDecoder {
    type Instruction = PowerPCInstruction;
    type Block = IRBlock;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Instruction>
    {
        // PowerPC解码逻辑
    }
}

// 2. 注册架构
pub enum GuestArch {
    Riscv64,
    Arm64,
    X86_64,
    PowerPC64,  // 新增
}

// 3. 更新工厂
impl DecoderFactory for DecoderFactoryImpl {
    fn create(arch: GuestArch) -> Box<dyn Decoder> {
        match arch {
            // ... 现有架构
            GuestArch::PowerPC64 => Box::new(PowerPCDecoder::new()),
        }
    }
}
```

## 优势

1. **代码复用**:
   - 执行引擎只需实现一次
   - 新架构只需添加解码器

2. **统一优化**:
   - IR优化pass对所有架构生效
   - JIT代码生成只需支持IR

3. **可维护性**:
   - 清晰的架构边界
   - 独立的模块开发

## 后果

### 短期
- ✅ 支持主流架构
- ✅ 清晰的抽象层
- ⚠️ IR设计需要平衡

### 长期
- ✅ 易于添加新架构
- ✅ 可扩展到更多平台
- ✅ 促进代码复用

---
**创建时间**: 2024-12-31
**作者**: VM开发团队
