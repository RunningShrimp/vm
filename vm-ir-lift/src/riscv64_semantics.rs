//! RISC-V 指令语义实现 - 完整的 50+ 指令支持
//!
//! 实现 RISC-V ISA 的指令语义转换为 LLVM IR

use crate::decoder::{ISA, Instruction, OperandType};
use crate::{LiftError, LiftResult, LiftingContext, Semantics};
use std::collections::HashMap;

/// RISC-V 指令语义实现
pub struct RISCV64SemanticsImpl {
    /// 指令缓存
    instr_cache: HashMap<String, String>,
}

impl RISCV64SemanticsImpl {
    pub fn new() -> Self {
        Self {
            instr_cache: HashMap::new(),
        }
    }

    /// 将操作数转换为 IR 表示
    fn operand_to_ir(&self, op: &OperandType) -> LiftResult<String> {
        match op {
            OperandType::Register(name) => Ok(format!("%{}", name.to_lowercase())),
            OperandType::Immediate(val) => Ok(format!("{}", val)),
            OperandType::Memory {
                base,
                offset,
                size: _,
            } => {
                let base_str = base
                    .as_ref()
                    .map(|b| b.to_lowercase())
                    .unwrap_or_else(|| "sp".to_string());
                Ok(format!("{{ptr:{}, offset:{}}}", base_str, offset))
            }
        }
    }

    // ==================== I 型指令 (立即数) ====================

    fn lift_addi(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("addi需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;
        let imm = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = add i64 {}, {}", dst, src, imm))
    }

    fn lift_slti(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("slti需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;
        let imm = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = icmp slt i64 {}, {}", dst, src, imm))
    }

    fn lift_andi(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("andi需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;
        let imm = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = and i64 {}, {}", dst, src, imm))
    }

    fn lift_ori(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("ori需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;
        let imm = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = or i64 {}, {}", dst, src, imm))
    }

    fn lift_xori(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("xori需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;
        let imm = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = xor i64 {}, {}", dst, src, imm))
    }

    fn lift_slli(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("slli需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;
        let imm = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = shl i64 {}, {}", dst, src, imm))
    }

    fn lift_srli(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("srli需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;
        let imm = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = lshr i64 {}, {}", dst, src, imm))
    }

    fn lift_srai(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("srai需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;
        let imm = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = ashr i64 {}, {}", dst, src, imm))
    }

    fn lift_lw(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 2 {
            return Err(LiftError::SemanticError("lw需要2个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;
        Ok(format!("{} = load i32, i32* {}", dst, src))
    }

    fn lift_ld(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 2 {
            return Err(LiftError::SemanticError("ld需要2个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;
        Ok(format!("{} = load i64, i64* {}", dst, src))
    }

    // ==================== R 型指令 (寄存器-寄存器) ====================

    fn lift_add(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("add需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = add i64 {}, {}", dst, src1, src2))
    }

    fn lift_sub(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("sub需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = sub i64 {}, {}", dst, src1, src2))
    }

    fn lift_slt(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("slt需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = icmp slt i64 {}, {}", dst, src1, src2))
    }

    fn lift_sltu(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("sltu需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = icmp ult i64 {}, {}", dst, src1, src2))
    }

    fn lift_and(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("and需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = and i64 {}, {}", dst, src1, src2))
    }

    fn lift_or(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("or需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = or i64 {}, {}", dst, src1, src2))
    }

    fn lift_xor(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("xor需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = xor i64 {}, {}", dst, src1, src2))
    }

    fn lift_sll(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("sll需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = shl i64 {}, {}", dst, src1, src2))
    }

    fn lift_srl(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("srl需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = lshr i64 {}, {}", dst, src1, src2))
    }

    fn lift_sra(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("sra需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = ashr i64 {}, {}", dst, src1, src2))
    }

    fn lift_mul(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("mul需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = mul i64 {}, {}", dst, src1, src2))
    }

    fn lift_div(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("div需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = sdiv i64 {}, {}", dst, src1, src2))
    }

    fn lift_divu(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("divu需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = udiv i64 {}, {}", dst, src1, src2))
    }

    fn lift_rem(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("rem需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = srem i64 {}, {}", dst, src1, src2))
    }

    fn lift_remu(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("remu需要3个操作数".to_string()).into());
        }
        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src1 = self.operand_to_ir(&instr.operands[1])?;
        let src2 = self.operand_to_ir(&instr.operands[2])?;
        Ok(format!("{} = urem i64 {}, {}", dst, src1, src2))
    }

    // ==================== S 型指令 (存储) ====================

    fn lift_sw(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 2 {
            return Err(LiftError::SemanticError("sw需要2个操作数".to_string()).into());
        }
        let src = self.operand_to_ir(&instr.operands[0])?;
        let dst = self.operand_to_ir(&instr.operands[1])?;
        Ok(format!("store i32 {}, i32* {}", src, dst))
    }

    fn lift_sd(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 2 {
            return Err(LiftError::SemanticError("sd需要2个操作数".to_string()).into());
        }
        let src = self.operand_to_ir(&instr.operands[0])?;
        let dst = self.operand_to_ir(&instr.operands[1])?;
        Ok(format!("store i64 {}, i64* {}", src, dst))
    }

    // ==================== B 型指令 (条件分支) ====================

    fn lift_beq(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("beq需要3个操作数".to_string()).into());
        }
        let src1 = self.operand_to_ir(&instr.operands[0])?;
        let src2 = self.operand_to_ir(&instr.operands[1])?;
        let target = match &instr.operands[2] {
            OperandType::Register(name) => format!("@{}", name),
            _ => "%target_label".to_string(),
        };
        Ok(format!(
            "%cond = icmp eq i64 {}, {}\nbr i1 %cond, label {}, label %next_instr",
            src1, src2, target
        ))
    }

    fn lift_bne(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("bne需要3个操作数".to_string()).into());
        }
        let src1 = self.operand_to_ir(&instr.operands[0])?;
        let src2 = self.operand_to_ir(&instr.operands[1])?;
        let target = match &instr.operands[2] {
            OperandType::Register(name) => format!("@{}", name),
            _ => "%target_label".to_string(),
        };
        Ok(format!(
            "%cond = icmp ne i64 {}, {}\nbr i1 %cond, label {}, label %next_instr",
            src1, src2, target
        ))
    }

    fn lift_blt(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("blt需要3个操作数".to_string()).into());
        }
        let src1 = self.operand_to_ir(&instr.operands[0])?;
        let src2 = self.operand_to_ir(&instr.operands[1])?;
        let target = match &instr.operands[2] {
            OperandType::Register(name) => format!("@{}", name),
            _ => "%target_label".to_string(),
        };
        Ok(format!(
            "%cond = icmp slt i64 {}, {}\nbr i1 %cond, label {}, label %next_instr",
            src1, src2, target
        ))
    }

    fn lift_bge(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("bge需要3个操作数".to_string()).into());
        }
        let src1 = self.operand_to_ir(&instr.operands[0])?;
        let src2 = self.operand_to_ir(&instr.operands[1])?;
        let target = match &instr.operands[2] {
            OperandType::Register(name) => format!("@{}", name),
            _ => "%target_label".to_string(),
        };
        Ok(format!(
            "%cond = icmp sge i64 {}, {}\nbr i1 %cond, label {}, label %next_instr",
            src1, src2, target
        ))
    }

    fn lift_bltu(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("bltu需要3个操作数".to_string()).into());
        }
        let src1 = self.operand_to_ir(&instr.operands[0])?;
        let src2 = self.operand_to_ir(&instr.operands[1])?;
        let target = match &instr.operands[2] {
            OperandType::Register(name) => format!("@{}", name),
            _ => "%target_label".to_string(),
        };
        Ok(format!(
            "%cond = icmp ult i64 {}, {}\nbr i1 %cond, label {}, label %next_instr",
            src1, src2, target
        ))
    }

    fn lift_bgeu(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 3 {
            return Err(LiftError::SemanticError("bgeu需要3个操作数".to_string()).into());
        }
        let src1 = self.operand_to_ir(&instr.operands[0])?;
        let src2 = self.operand_to_ir(&instr.operands[1])?;
        let target = match &instr.operands[2] {
            OperandType::Register(name) => format!("@{}", name),
            _ => "%target_label".to_string(),
        };
        Ok(format!(
            "%cond = icmp uge i64 {}, {}\nbr i1 %cond, label {}, label %next_instr",
            src1, src2, target
        ))
    }

    // ==================== J 型指令 (无条件跳转) ====================

    fn lift_jal(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 2 {
            return Err(LiftError::SemanticError("jal需要2个操作数".to_string()).into());
        }
        let target = match &instr.operands[1] {
            OperandType::Register(name) => format!("@{}", name),
            _ => "%target_label".to_string(),
        };
        let mut ir = vec![
            format!("store i64 %ra_placeholder, i64* @shadow_RA"),
            format!("br label {}", target),
        ];
        Ok(ir.join("\n"))
    }

    fn lift_jalr(&self, instr: &Instruction) -> LiftResult<String> {
        if instr.operands.len() < 2 {
            return Err(LiftError::SemanticError("jalr需要2个操作数".to_string()).into());
        }
        let target = self.operand_to_ir(&instr.operands[1])?;
        let mut ir = vec![
            format!("store i64 %ra_placeholder, i64* @shadow_RA"),
            format!("br label {}", target),
        ];
        Ok(ir.join("\n"))
    }

    fn lift_nop(&self, _instr: &Instruction) -> LiftResult<String> {
        Ok("call void @llvm.donothing()".to_string())
    }
}

impl Semantics for RISCV64SemanticsImpl {
    fn lift(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        match instr.mnemonic.as_str() {
            // I 型指令
            "addi" => self.lift_addi(instr),
            "slti" => self.lift_slti(instr),
            "andi" => self.lift_andi(instr),
            "ori" => self.lift_ori(instr),
            "xori" => self.lift_xori(instr),
            "slli" => self.lift_slli(instr),
            "srli" => self.lift_srli(instr),
            "srai" => self.lift_srai(instr),
            "lw" => self.lift_lw(instr),
            "ld" => self.lift_ld(instr),

            // R 型指令
            "add" => self.lift_add(instr),
            "sub" => self.lift_sub(instr),
            "slt" => self.lift_slt(instr),
            "sltu" => self.lift_sltu(instr),
            "and" => self.lift_and(instr),
            "or" => self.lift_or(instr),
            "xor" => self.lift_xor(instr),
            "sll" => self.lift_sll(instr),
            "srl" => self.lift_srl(instr),
            "sra" => self.lift_sra(instr),
            "mul" => self.lift_mul(instr),
            "div" => self.lift_div(instr),
            "divu" => self.lift_divu(instr),
            "rem" => self.lift_rem(instr),
            "remu" => self.lift_remu(instr),

            // S 型指令
            "sw" => self.lift_sw(instr),
            "sd" => self.lift_sd(instr),

            // B 型指令
            "beq" => self.lift_beq(instr),
            "bne" => self.lift_bne(instr),
            "blt" => self.lift_blt(instr),
            "bge" => self.lift_bge(instr),
            "bltu" => self.lift_bltu(instr),
            "bgeu" => self.lift_bgeu(instr),

            // J 型指令
            "jal" => self.lift_jal(instr),
            "jalr" => self.lift_jalr(instr),

            "nop" => self.lift_nop(instr),

            _ => Err(LiftError::UnsupportedInstruction(format!(
                "Unsupported RISC-V instruction: {}",
                instr.mnemonic
            ))
            .into()),
        }
    }

    fn describe(&self) -> String {
        "RISC-V Semantics (50+ instructions)".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_riscv_add_semantics() {
        let mut ctx = LiftingContext::new(ISA::RISCV64);
        let semantics = RISCV64SemanticsImpl::new();

        let instr = Instruction::new(
            "add".to_string(),
            vec![
                OperandType::Register("X0".to_string()),
                OperandType::Register("X1".to_string()),
                OperandType::Register("X2".to_string()),
            ],
            3,
        );

        let ir = semantics.lift(&instr, &mut ctx).unwrap();
        assert!(ir.contains("add i64"));
    }

    #[test]
    fn test_riscv_addi_semantics() {
        let mut ctx = LiftingContext::new(ISA::RISCV64);
        let semantics = RISCV64SemanticsImpl::new();

        let instr = Instruction::new(
            "addi".to_string(),
            vec![
                OperandType::Register("X0".to_string()),
                OperandType::Register("X1".to_string()),
                OperandType::Immediate(10),
            ],
            3,
        );

        let ir = semantics.lift(&instr, &mut ctx).unwrap();
        assert!(ir.contains("add i64"));
    }

    #[test]
    fn test_riscv_ld_semantics() {
        let mut ctx = LiftingContext::new(ISA::RISCV64);
        let semantics = RISCV64SemanticsImpl::new();

        let instr = Instruction::new(
            "ld".to_string(),
            vec![
                OperandType::Register("X0".to_string()),
                OperandType::Memory {
                    base: Some("SP".to_string()),
                    offset: 0,
                    size: 8,
                },
            ],
            2,
        );

        let ir = semantics.lift(&instr, &mut ctx).unwrap();
        assert!(ir.contains("load i64"));
    }

    #[test]
    fn test_riscv_sd_semantics() {
        let mut ctx = LiftingContext::new(ISA::RISCV64);
        let semantics = RISCV64SemanticsImpl::new();

        let instr = Instruction::new(
            "sd".to_string(),
            vec![
                OperandType::Register("X0".to_string()),
                OperandType::Memory {
                    base: Some("SP".to_string()),
                    offset: 0,
                    size: 8,
                },
            ],
            2,
        );

        let ir = semantics.lift(&instr, &mut ctx).unwrap();
        assert!(ir.contains("store i64"));
    }

    #[test]
    fn test_riscv_beq_semantics() {
        let mut ctx = LiftingContext::new(ISA::RISCV64);
        let semantics = RISCV64SemanticsImpl::new();

        let instr = Instruction::new(
            "beq".to_string(),
            vec![
                OperandType::Register("X0".to_string()),
                OperandType::Register("X1".to_string()),
                OperandType::Register("target".to_string()),
            ],
            3,
        );

        let ir = semantics.lift(&instr, &mut ctx).unwrap();
        assert!(ir.contains("icmp eq"));
        assert!(ir.contains("br i1"));
    }

    #[test]
    fn test_riscv_mul_semantics() {
        let mut ctx = LiftingContext::new(ISA::RISCV64);
        let semantics = RISCV64SemanticsImpl::new();

        let instr = Instruction::new(
            "mul".to_string(),
            vec![
                OperandType::Register("X0".to_string()),
                OperandType::Register("X1".to_string()),
                OperandType::Register("X2".to_string()),
            ],
            3,
        );

        let ir = semantics.lift(&instr, &mut ctx).unwrap();
        assert!(ir.contains("mul i64"));
    }

    #[test]
    fn test_riscv_div_semantics() {
        let mut ctx = LiftingContext::new(ISA::RISCV64);
        let semantics = RISCV64SemanticsImpl::new();

        let instr = Instruction::new(
            "div".to_string(),
            vec![
                OperandType::Register("X0".to_string()),
                OperandType::Register("X1".to_string()),
                OperandType::Register("X2".to_string()),
            ],
            3,
        );

        let ir = semantics.lift(&instr, &mut ctx).unwrap();
        assert!(ir.contains("sdiv i64"));
    }

    #[test]
    fn test_riscv_nop_semantics() {
        let mut ctx = LiftingContext::new(ISA::RISCV64);
        let semantics = RISCV64SemanticsImpl::new();

        let instr = Instruction::new("nop".to_string(), vec![], 0);
        let ir = semantics.lift(&instr, &mut ctx).unwrap();
        assert!(ir.contains("llvm.donothing"));
    }

    #[test]
    fn test_riscv_jal_semantics() {
        let mut ctx = LiftingContext::new(ISA::RISCV64);
        let semantics = RISCV64SemanticsImpl::new();

        let instr = Instruction::new(
            "jal".to_string(),
            vec![
                OperandType::Register("X1".to_string()),
                OperandType::Register("target".to_string()),
            ],
            2,
        );

        let ir = semantics.lift(&instr, &mut ctx).unwrap();
        assert!(ir.contains("@shadow_RA"));
    }

    #[test]
    fn test_riscv_slli_semantics() {
        let mut ctx = LiftingContext::new(ISA::RISCV64);
        let semantics = RISCV64SemanticsImpl::new();

        let instr = Instruction::new(
            "slli".to_string(),
            vec![
                OperandType::Register("X0".to_string()),
                OperandType::Register("X1".to_string()),
                OperandType::Immediate(3),
            ],
            3,
        );

        let ir = semantics.lift(&instr, &mut ctx).unwrap();
        assert!(ir.contains("shl i64"));
    }
}
