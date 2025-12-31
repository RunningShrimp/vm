# 指令前端架构

## 目录

- [指令前端概述](#指令前端概述)
- [多架构支持](#多架构支持)
- [RISC-V前端](#risc-v前端)
- [ARM64前端](#arm64前端)
- [解码器接口](#解码器接口)
- [扩展性设计](#扩展性设计)

---

## 指令前端概述

### 职责

指令前端负责将Guest二进制指令解码为与架构无关的中间表示（IR），供执行引擎使用。

### 架构设计

```
┌─────────────────────────────────────────────────────────┐
│                   vm-frontend                           │
│                                                         │
│  ┌────────────┐  ┌────────────┐  ┌─────────────┐     │
│  │ RISC-V     │  │   ARM64    │  │   x86-64    │     │
│  │ Decoder    │  │  Decoder   │  │  Decoder    │     │
│  └─────┬──────┘  └─────┬──────┘  └─────┬───────┘     │
│        │               │               │             │
│        └───────────────┴───────────────┘             │
│                        ↓                             │
│              统一的Decoder trait                     │
│                        ↓                             │
│                 IR指令和基本块                        │
└─────────────────────────────────────────────────────────┘
```

---

## 多架构支持

### 架构列表

| 架构 | 状态 | 支持指令集 | 扩展支持 |
|------|------|-----------|---------|
| RISC-V 64 | ✅ 完整 | RV64I | M/A/F/D/C |
| ARM64 | ✅ 基础 | AArch64 | SIMD/Crypto |
| x86-64 | 🚧 开发中 | x86-64 | MMX/SSE/AVX |

### 解码器特性

```rust
pub trait Decoder {
    type Instruction;
    type Block;

    /// 解码单条指令
    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Instruction>;

    /// 解码基本块
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Block>;
}
```

---

## RISC-V前端

### RISC-V指令格式

```
R-type:    ┌─────┬──────┬──────┬─────┬─────┬──────┐
           │funct7│ rs2  │ rs1  │funct3│ rd  │opcode│
           │  7b  │  5b  │  5b  │ 3b  │ 5b  │  7b  │
           └─────┴──────┴──────┴─────┴─────┴──────┘

I-type:    ┌───────────────┬──────┬─────┬──────┐
           │    imm[11:0]  │ rs1  │funct3│ rd  │opcode│
           │      12b       │  5b  │ 3b  │ 5b  │  7b  │
           └───────────────┴──────┴─────┴──────┘

S-type:    ┌─────┬──────┬──────┬─────┬──────┐
           │imm[4:0]│ rs2  │ rs1  │funct3│imm[11:5]│opcode│
           │  5b  │  5b  │  5b  │ 3b  │   7b  │  7b  │
           └─────┴──────┴──────┴─────┴──────┴──────┘

B-type:    ┌─────────┬──────┬──────┬─────┬──────┐
           │imm[12│10:5]│ rs2  │ rs1  │funct3│imm[4:1│11]│opcode│
           │  1b│6b  │  5b  │  5b  │ 3b  │  4b│1b│  7b  │
           └─────────┴──────┴──────┴─────┴──────┴──────┘

U-type:    ┌───────────────────────┬─────┬──────┐
           │       imm[31:12]       │ rd  │opcode│
           │          20b           │ 5b  │  7b  │
           └───────────────────────┴─────┴──────┘
```

### RISC-V解码器实现

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
        // 1. 读取指令字
        let insn_word = mmu.fetch_insn(pc)? as u32;

        // 2. 提取字段
        let opcode = (insn_word & 0x7F) as u8;
        let rd = ((insn_word >> 7) & 0x1F) as usize;
        let rs1 = ((insn_word >> 15) & 0x1F) as usize;
        let rs2 = ((insn_word >> 20) & 0x1F) as usize;
        let funct3 = ((insn_word >> 12) & 0x7) as u8;
        let funct7 = ((insn_word >> 25) & 0x7F) as u8;

        // 3. 根据opcode解码
        let insn = match opcode {
            0x33 => {
                // R-type
                match (funct3, funct7) {
                    (0b000, 0b0000000) => RiscvInstruction::ADD { rd, rs1, rs2 },
                    (0b000, 0b0100000) => RiscvInstruction::SUB { rd, rs1, rs2 },
                    (0b001, 0b0000000) => RiscvInstruction::SLL { rd, rs1, rs2 },
                    (0b101, 0b0000000) => RiscvInstruction::SRL { rd, rs1, rs2 },
                    (0b101, 0b0100000) => RiscvInstruction::SRA { rd, rs1, rs2 },
                    // ...
                    _ => return Err(VmError::Execution(
                        ExecutionError::Fault(Fault::InvalidOpcode {
                            pc, opcode: insn_word
                        })
                    )),
                }
            }
            0x13 => {
                // I-type
                let imm = ((insn_word >> 20) as i32) as i64;
                match funct3 {
                    0b000 => RiscvInstruction::ADDI { rd, rs1, imm },
                    0b001 => RiscvInstruction::SLLI { rd, rs1, shamt: (rs2 & 0x1F) as u8 },
                    0b101 => RiscvInstruction::SRLI { rd, rs1, shamt: (rs2 & 0x1F) as u8 },
                    0b110 => RiscvInstruction::ANDI { rd, rs1, imm },
                    0b111 => RiscvInstruction::ORI { rd, rs1, imm },
                    // ...
                    _ => return Err(VmError::Execution(
                        ExecutionError::Fault(Fault::InvalidOpcode {
                            pc, opcode: insn_word
                        })
                    )),
                }
            }
            0x03 => {
                // Load
                let imm = ((insn_word >> 20) as i32) as i64;
                match funct3 {
                    0b000 => RiscvInstruction::LB { rd, rs1, imm },
                    0b001 => RiscvInstruction::LH { rd, rs1, imm },
                    0b010 => RiscvInstruction::LW { rd, rs1, imm },
                    0b011 => RiscvInstruction::LD { rd, rs1, imm },
                    0b100 => RiscvInstruction::LBU { rd, rs1, imm },
                    0b101 => RiscvInstruction::LHU { rd, rs1, imm },
                    0b110 => RiscvInstruction::LWU { rd, rs1, imm },
                    _ => return Err(VmError::Execution(
                        ExecutionError::Fault(Fault::InvalidOpcode {
                            pc, opcode: insn_word
                        })
                    )),
                }
            }
            0x23 => {
                // Store
                let imm = ((insn_word >> 25) & 0x7F) as i64
                        | (((insn_word >> 7) & 0x1F) as i64) << 5;
                match funct3 {
                    0b000 => RiscvInstruction::SB { rs1, rs2, imm },
                    0b001 => RiscvInstruction::SH { rs1, rs2, imm },
                    0b010 => RiscvInstruction::SW { rs1, rs2, imm },
                    0b011 => RiscvInstruction::SD { rs1, rs2, imm },
                    _ => return Err(VmError::Execution(
                        ExecutionError::Fault(Fault::InvalidOpcode {
                            pc, opcode: insn_word
                        })
                    )),
                }
            }
            0x63 => {
                // Branch
                let imm = ((insn_word >> 31) & 1) as i64 << 12
                        | ((insn_word >> 25) & 0x3F) as i64 << 5
                        | ((insn_word >> 8) & 0xF) as i64 << 1
                        | (((insn_word >> 7) & 1) as i64) << 11;
                let imm = (imm << 51) >> 51;  // 符号扩展
                match funct3 {
                    0b000 => RiscvInstruction::BEQ { rs1, rs2, imm },
                    0b001 => RiscvInstruction::BNE { rs1, rs2, imm },
                    0b100 => RiscvInstruction::BLT { rs1, rs2, imm },
                    0b101 => RiscvInstruction::BGE { rs1, rs2, imm },
                    0b110 => RiscvInstruction::BLTU { rs1, rs2, imm },
                    0b111 => RiscvInstruction::BGEU { rs1, rs2, imm },
                    _ => return Err(VmError::Execution(
                        ExecutionError::Fault(Fault::InvalidOpcode {
                            pc, opcode: insn_word
                        })
                    )),
                }
            }
            0x6F => {
                // JAL
                let imm = ((insn_word >> 31) & 1) as i64 << 20
                        | ((insn_word >> 21) & 0x3FF) as i64 << 1
                        | ((insn_word >> 20) & 1) as i64 << 11
                        | (((insn_word >> 12) & 0xFF) as i64) << 12;
                let imm = (imm << 43) >> 43;  // 符号扩展
                RiscvInstruction::JAL { rd, imm }
            }
            0x67 => {
                // JALR
                let imm = ((insn_word >> 20) as i32) as i64;
                RiscvInstruction::JALR { rd, rs1, imm }
            }
            0x73 => {
                // System
                match (funct3, funct7) {
                    (0b000, 0b0000000) => RiscvInstruction::ECALL,
                    (0b001, 0b0000000) => RiscvInstruction::EBREAK,
                    (0b000, 0b0011000) => RiscvInstruction::SRET,
                    (0b000, 0b0001000) => RiscvInstruction::MRET,
                    _ => return Err(VmError::Execution(
                        ExecutionError::Fault(Fault::InvalidOpcode {
                            pc, opcode: insn_word
                        })
                    )),
                }
            }
            _ => return Err(VmError::Execution(
                ExecutionError::Fault(Fault::InvalidOpcode {
                    pc, opcode: insn_word
                })
            )),
        };

        Ok(insn)
    }

    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Block>
    {
        let mut block = IRBlock::new(pc);

        loop {
            let insn = self.decode_insn(mmu, pc)?;

            // 转换为IR
            let ir_insn = self.riscv_to_ir(&insn)?;
            block.push(ir_insn);

            // 更新PC
            pc = pc + 4;

            // 检查是否是终止指令
            if self.is_terminator(&insn) {
                break;
            }
        }

        Ok(block)
    }
}

impl RiscvDecoder {
    fn riscv_to_ir(&self, insn: &RiscvInstruction)
        -> Result<IRInstruction, VmError>
    {
        match insn {
            RiscvInstruction::ADD { rd, rs1, rs2 } => {
                Ok(IRInstruction::Add {
                    dst: Reg::X(*rd),
                    src1: Reg::X(*rs1),
                    src2: Operand::Reg(Reg::X(*rs2)),
                })
            }
            RiscvInstruction::LW { rd, rs1, imm } => {
                Ok(IRInstruction::Load {
                    dst: Reg::X(*rd),
                    addr: MemOperand::BaseDisp {
                        base: Reg::X(*rs1),
                        disp: *imm,
                    },
                    size: 4,
                })
            }
            // ...
            _ => todo\!(),
        }
    }
}
```

---

## ARM64前端

### ARM64指令格式

```
Base instruction: 32 bits
┌─────────────────────────────────────────────────────┐
│           31:25              24:21    20:16  15:0   │
│            │                  │        │      │     │
│           op                 Rm       Rn    Rd/imm │
└─────────────────────────────────────────────────────┘
```

### ARM64解码器

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

        // 提取主要opcode字段
        let op0 = (insn_word >> 25) & 0xF;
        let op1 = (insn_word >> 20) & 0x1F;
        let op2 = (insn_word >> 16) & 0xF;

        match (op0, op1, op2) {
            // 数据处理 - 立即数
            (0b1 << 3, _, _) => {
                let rd = (insn_word & 0x1F) as usize;
                let rn = ((insn_word >> 5) & 0x1F) as usize;
                
                match (insn_word >> 23) & 0x3 {
                    0b00 => Ok(Arm64Instruction::ADDImm { rd, rn, imm12 }),
                    0b10 => Ok(Arm64Instruction::SUBImm { rd, rn, imm12 }),
                    // ...
                    _ => todo\!(),
                }
            }
            // 分支条件
            (0b0101010, _, _) => {
                let condition = ((insn_word >> 0) & 0xF) as u8;
                let offset = ((insn_word >> 5) & 0x7FFFF) as i64;
                Ok(Arm64Instruction::BCond { condition, offset })
            }
            // 无条件分支
            (0b000101, _, _) => {
                let offset = ((insn_word >> 0) & 0x3FFFFFF) as i64;
                Ok(Arm64Instruction::B { offset })
            }
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

---

## 解码器接口

### 统一的Decoder trait

```rust
pub trait Decoder {
    type Instruction;
    type Block;

    /// 解码单条指令
    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Instruction>;

    /// 解码基本块
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Block>;
}
```

### 解码器工厂

```rust
pub trait DecoderFactory {
    fn create_riscv() -> Box<dyn Decoder<Instruction = RiscvInstruction, Block = IRBlock>>;
    fn create_arm64() -> Box<dyn Decoder<Instruction = Arm64Instruction, Block = IRBlock>>;
    fn create_x86() -> Box<dyn Decoder<Instruction = X86Instruction, Block = IRBlock>>;
}
```

---

## 扩展性设计

### 添加新架构

```rust
// 1. 定义指令类型
pub struct PowerPCInstruction {
    opcode: u8,
    fields: PowerPCFields,
}

// 2. 实现Decoder
impl Decoder for PowerPCDecoder {
    type Instruction = PowerPCInstruction;
    type Block = IRBlock;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Instruction>
    {
        // 实现PowerPC解码逻辑
    }
}

// 3. 注册到工厂
impl DecoderFactory for PowerPCFactory {
    fn create_powerpc() -> Box<dyn Decoder> {
        Box::new(PowerPCDecoder::new())
    }
}
```

### 添加指令扩展

```rust
// RISC-V M扩展（乘除法）
impl RiscvDecoder {
    fn decode_m_extension(&self, insn_word: u32) -> Option<RiscvInstruction> {
        let rd = ((insn_word >> 7) & 0x1F) as usize;
        let rs1 = ((insn_word >> 15) & 0x1F) as usize;
        let rs2 = ((insn_word >> 20) & 0x1F) as usize;
        let funct3 = ((insn_word >> 12) & 0x7) as u8;

        match funct3 {
            0b000 => Some(RiscvInstruction::MUL { rd, rs1, rs2 }),
            0b001 => Some(RiscvInstruction::MULH { rd, rs1, rs2 }),
            0b100 => Some(RiscvInstruction::DIV { rd, rs1, rs2 }),
            0b101 => Some(RiscvInstruction::DIVU { rd, rs1, rs2 }),
            0b110 => Some(RiscvInstruction::REM { rd, rs1, rs2 }),
            0b111 => Some(RiscvInstruction::REMU { rd, rs1, rs2 }),
            _ => None,
        }
    }
}
```

---

**文档版本**: 1.0
**最后更新**: 2025-12-31
**作者**: VM开发团队
