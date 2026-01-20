//! 指令语义库 - 将指令转换为 LLVM IR
//!
//! 实现每种 ISA 的指令语义，生成对应的 LLVM IR 代码。

use vm_core::{CoreError, VmError};

use crate::lift::decoder::{ISA, Instruction, OperandType};
use crate::lift::{LiftResult, LiftingContext};

/// 指令语义 Trait
pub trait Semantics {
    /// 将指令抬升为 LLVM IR
    /// 返回生成的 IR 代码片段
    fn lift(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String>;

    /// 获取指令的操作类型描述
    fn describe(&self) -> String {
        "generic instruction".to_string()
    }
}

/// FLAGS 影子状态管理
#[derive(Debug, Clone)]
pub struct FlagsState {
    /// 进位标志 (Carry Flag)
    pub cf: Option<String>,
    /// 奇偶标志 (Parity Flag)
    pub pf: Option<String>,
    /// 零标志 (Zero Flag)
    pub zf: Option<String>,
    /// 符号标志 (Sign Flag)
    pub sf: Option<String>,
    /// 溢出标志 (Overflow Flag)
    pub of: Option<String>,
    /// 调整标志 (Auxiliary Flag)
    pub af: Option<String>,
}

impl Default for FlagsState {
    fn default() -> Self {
        Self::new()
    }
}

impl FlagsState {
    pub fn new() -> Self {
        Self {
            cf: None,
            pf: None,
            zf: None,
            sf: None,
            of: None,
            af: None,
        }
    }

    /// 从算术操作生成 FLAGS 更新
    pub fn from_arithmetic_op(mnemonic: &str, result_var: &str) -> Self {
        let mut flags = FlagsState::new();

        if matches!(mnemonic, "add" | "sub" | "adc" | "sbb") {
            // 算术指令更新所有 FLAGS
            flags.zf = Some(format!("icmp eq i64 {}, 0", result_var));
            flags.sf = Some(format!("lshr i64 {}, 63", result_var));
            flags.of = Some(format!(
                "llvm.ssub.with.overflow(i64 {}, i64 0).1",
                result_var
            ));
            flags.cf = Some("undef".to_string());
        }

        flags
    }

    /// 生成 FLAGS 更新的 LLVM IR
    pub fn to_ir(&self) -> Vec<String> {
        let mut ir = Vec::new();

        if let Some(cf) = &self.cf
            && cf != "undef"
        {
            ir.push(format!(r#"store i1 {}, i1* @shadow_CF"#, cf));
        }
        if let Some(zf) = &self.zf {
            ir.push(format!(r#"store i1 {}, i1* @shadow_ZF"#, zf));
        }
        if let Some(sf) = &self.sf {
            ir.push(format!(r#"store i1 {}, i1* @shadow_SF"#, sf));
        }
        if let Some(of) = &self.of {
            ir.push(format!(r#"store i1 {}, i1* @shadow_OF"#, of));
        }
        if let Some(pf) = &self.pf {
            ir.push(format!(r#"store i1 {}, i1* @shadow_PF"#, pf));
        }
        if let Some(af) = &self.af {
            ir.push(format!(r#"store i1 {}, i1* @shadow_AF"#, af));
        }

        ir
    }
}

/// x86-64 指令语义实现
pub struct X86_64Semantics {}

impl Default for X86_64Semantics {
    fn default() -> Self {
        Self::new()
    }
}

impl X86_64Semantics {
    pub fn new() -> Self {
        Self {}
    }

    fn operand_to_ir(&self, op: &OperandType) -> LiftResult<String> {
        match op {
            OperandType::Register(name) => Ok(format!("%{}", name)),
            OperandType::Immediate(val) => Ok(format!("{}", val)),
            OperandType::Memory {
                base,
                offset,
                size: _,
            } => {
                let base_str = base.as_deref().unwrap_or("RBX");
                Ok(format!("load i64, i64* (%{} + {})", base_str, offset))
            }
        }
    }

    fn lift_add(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "ADD requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%add_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = add i64 {}, {}", result_var, dst, src)];

        // 生成 FLAGS 更新
        let flags = FlagsState::from_arithmetic_op("add", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_sub(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "SUB requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%sub_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = sub i64 {}, {}", result_var, dst, src)];

        // 生成 FLAGS 更新
        let flags = FlagsState::from_arithmetic_op("sub", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_mov(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "MOV requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        Ok(format!("store i64 {}, i64* {}", src, dst))
    }

    fn lift_lea(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "LEA requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        // LEA dst, [src_addr] -> dst = src_addr
        let src = self.operand_to_ir(&instr.operands[1])?;

        Ok(format!("ptrtoint i64* {} to i64", src))
    }

    fn lift_cmp(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "CMP requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let src1 = self.operand_to_ir(&instr.operands[0])?;
        let src2 = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%cmp_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = sub i64 {}, {}", result_var, src1, src2)];

        // CMP 只更新 FLAGS，不存储结果
        let flags = FlagsState::from_arithmetic_op("cmp", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_jmp(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "JMP requires target".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let target = self.operand_to_ir(&instr.operands[0])?;

        Ok(format!("br i1 1, label %jmp_target_{}", target))
    }

    fn lift_call(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "CALL requires target".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let target = self.operand_to_ir(&instr.operands[0])?;

        // CALL 生成：
        // 1. 推送返回地址
        // 2. 跳转到目标
        Ok(format!(
            r#"call void @push_return_address()
br label %call_target_{}"#,
            target
        ))
    }

    fn lift_ret(&self, _instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        // RET 生成：
        // 1. 弹出返回地址
        // 2. 跳转到返回地址
        Ok(r#"call i64 @pop_return_address()
br label %ret_target"#
            .to_string())
    }

    fn lift_push(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "PUSH requires operand".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let value = self.operand_to_ir(&instr.operands[0])?;

        // PUSH:
        // RSP -= 8
        // [RSP] = value
        Ok(format!(
            r#"%rsp_new = sub i64 %RSP, 8
store i64 {}, i64* %rsp_new"#,
            value
        ))
    }

    fn lift_pop(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "POP requires operand".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;

        // POP:
        // value = [RSP]
        // RSP += 8
        // dst = value
        Ok(format!(
            r#"%pop_value = load i64, i64* %RSP
%rsp_new = add i64 %RSP, 8
store i64 %pop_value, i64* {}"#,
            dst
        ))
    }

    fn lift_nop(&self, _instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        Ok("call void @llvm.donothing()".to_string())
    }

    fn lift_imul(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() < 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "IMUL requires at least 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%imul_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = mul i64 {}, {}", result_var, dst, src)];

        // IMUL 设置 OF/CF
        let flags = FlagsState::from_arithmetic_op("imul", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_xor(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "XOR requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%xor_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = xor i64 {}, {}", result_var, dst, src)];

        // XOR 清除 OF/CF, 设置 ZF/SF
        let flags = FlagsState::from_arithmetic_op("xor", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_or(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "OR requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%or_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = or i64 {}, {}", result_var, dst, src)];

        // OR 清除 OF/CF
        let flags = FlagsState::from_arithmetic_op("or", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_and(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "AND requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%and_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = and i64 {}, {}", result_var, dst, src)];

        // AND 清除 OF/CF
        let flags = FlagsState::from_arithmetic_op("and", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_test(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "TEST requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let src1 = self.operand_to_ir(&instr.operands[0])?;
        let src2 = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%test_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = and i64 {}, {}", result_var, src1, src2)];

        // TEST 只修改 FLAGS，不修改操作数
        let flags = FlagsState::from_arithmetic_op("test", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_shl(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "SHL requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let val = self.operand_to_ir(&instr.operands[0])?;
        let shift = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%shl_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = shl i64 {}, {}", result_var, val, shift)];

        let flags = FlagsState::from_arithmetic_op("shl", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_shr(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "SHR requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let val = self.operand_to_ir(&instr.operands[0])?;
        let shift = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%shr_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = lshr i64 {}, {}", result_var, val, shift)];

        let flags = FlagsState::from_arithmetic_op("shr", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_div(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "DIV requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dividend = self.operand_to_ir(&instr.operands[0])?;
        let divisor = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%div_result_{}", ctx.cache_stats());

        let ir_lines = [format!(
            "{} = sdiv i64 {}, {}",
            result_var, dividend, divisor
        )];

        Ok(ir_lines.join("\n"))
    }

    fn lift_movzx(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "MOVZX requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        // Zero-extend from 8/16/32 to 64 bits and store to destination
        Ok(format!("store i64 (zext i32 {} to i64), i64* {}", src, dst))
    }

    fn lift_movsx(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "MOVSX requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        // Sign-extend from 8/16/32 to 64 bits and store to destination
        Ok(format!("store i64 (sext i32 {} to i64), i64* {}", src, dst))
    }

    fn lift_inc(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "INC requires operand".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let result_var = format!("%inc_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = add i64 {}, 1", result_var, dst)];

        let flags = FlagsState::from_arithmetic_op("add", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_dec(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "DEC requires operand".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let result_var = format!("%dec_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = sub i64 {}, 1", result_var, dst)];

        let flags = FlagsState::from_arithmetic_op("sub", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_neg(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "NEG requires operand".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let result_var = format!("%neg_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = sub i64 0, {}", result_var, dst)];

        let flags = FlagsState::from_arithmetic_op("neg", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_not(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "NOT requires operand".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let result_var = format!("%not_result_{}", ctx.cache_stats());

        let ir_lines = [format!("{} = xor i64 {}, -1", result_var, dst)];

        Ok(ir_lines.join("\n"))
    }

    fn lift_ror(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "ROR requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let val = self.operand_to_ir(&instr.operands[0])?;
        let shift = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%ror_result_{}", ctx.cache_stats());

        let ir_lines = [format!(
            "{} = call i64 @llvm.fshr.i64(i64 {}, i64 {}, i64 {})",
            result_var, val, val, shift
        )];

        Ok(ir_lines.join("\n"))
    }

    fn lift_rol(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "ROL requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let val = self.operand_to_ir(&instr.operands[0])?;
        let shift = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%rol_result_{}", ctx.cache_stats());

        let ir_lines = [format!(
            "{} = call i64 @llvm.fshl.i64(i64 {}, i64 {}, i64 {})",
            result_var, val, val, shift
        )];

        Ok(ir_lines.join("\n"))
    }

    fn lift_sar(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "SAR requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let val = self.operand_to_ir(&instr.operands[0])?;
        let shift = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%sar_result_{}", ctx.cache_stats());

        let ir_lines = [format!("{} = ashr i64 {}, {}", result_var, val, shift)];

        Ok(ir_lines.join("\n"))
    }

    fn lift_adc(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "ADD requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%adc_result_{}", ctx.cache_stats());

        // ADC 需要读取进位标志，但简化处理为加 0/1
        let mut ir_lines = vec![format!("{} = add i64 {}, {}", result_var, dst, src)];

        let flags = FlagsState::from_arithmetic_op("adc", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_sbb(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "SBB requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        let result_var = format!("%sbb_result_{}", ctx.cache_stats());

        let mut ir_lines = vec![format!("{} = sub i64 {}, {}", result_var, dst, src)];

        let flags = FlagsState::from_arithmetic_op("sbb", &result_var);
        ir_lines.extend(flags.to_ir());

        Ok(ir_lines.join("\n"))
    }

    fn lift_movq(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "MOVQ requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        Ok(format!("store i64 {}, i64* {}", src, dst))
    }

    fn lift_movsq(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "MOVSQ requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let src = self.operand_to_ir(&instr.operands[0])?;

        // MOVSQ: 移动 [RSI] -> [RDI]，RSI/RDI += 8
        Ok(format!(
            r#"%val = load i64, i64* {}
store i64 %val, i64* %RDI
%rsi_new = add i64 %RSI, 8
%rdi_new = add i64 %RDI, 8"#,
            src
        ))
    }

    fn lift_bswap(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "BSWAP requires operand".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let val = self.operand_to_ir(&instr.operands[0])?;
        let result_var = format!("%bswap_result_{}", ctx.cache_stats());

        let ir_lines = [format!(
            "{} = call i64 @llvm.bswap.i64(i64 {})",
            result_var, val
        )];

        Ok(ir_lines.join("\n"))
    }

    fn lift_clz(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "BSR/CLZ requires operand".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let val = self.operand_to_ir(&instr.operands[0])?;
        let result_var = format!("%clz_result_{}", ctx.cache_stats());

        let ir_lines = [format!(
            "{} = call i64 @llvm.ctlz.i64(i64 {}, i1 0)",
            result_var, val
        )];

        Ok(ir_lines.join("\n"))
    }

    fn lift_ctz(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "BSF/CTZ requires operand".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let val = self.operand_to_ir(&instr.operands[0])?;
        let result_var = format!("%ctz_result_{}", ctx.cache_stats());

        let ir_lines = [format!(
            "{} = call i64 @llvm.cttz.i64(i64 {}, i1 0)",
            result_var, val
        )];

        Ok(ir_lines.join("\n"))
    }

    fn lift_popcnt(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "POPCNT requires operand".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let val = self.operand_to_ir(&instr.operands[0])?;
        let result_var = format!("%popcnt_result_{}", ctx.cache_stats());

        let ir_lines = [format!(
            "{} = call i64 @llvm.ctpop.i64(i64 {})",
            result_var, val
        )];

        Ok(ir_lines.join("\n"))
    }

    fn lift_parity(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.is_empty() {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "PARITY requires operand".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let val = self.operand_to_ir(&instr.operands[0])?;
        let result_var = format!("%parity_result_{}", ctx.cache_stats());

        let count_var = format!("%count_{}", ctx.cache_stats());
        let ir_lines = [
            format!("{} = call i64 @llvm.ctpop.i64(i64 {})", count_var, val),
            format!("{} = and i64 {}, 1", result_var, count_var),
        ];

        Ok(ir_lines.join("\n"))
    }

    fn lift_cpuid(&self, _instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        // CPUID: 返回处理器信息（简化处理）
        Ok(r#"call void @cpuid_handler()
store i64 0, i64* %RAX
store i64 0, i64* %RBX
store i64 0, i64* %RCX
store i64 0, i64* %RDX"#
            .to_string())
    }

    fn lift_rdmsr(&self, _instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        // RDMSR: 读取 MSR（模型特定寄存器）
        Ok(r#"call void @rdmsr_handler()
store i64 0, i64* %RAX
store i64 0, i64* %RDX"#
            .to_string())
    }

    fn lift_wrmsr(&self, _instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        // WRMSR: 写入 MSR
        Ok(r#"call void @wrmsr_handler(i64 %RCX, i64 %RAX, i64 %RDX)"#.to_string())
    }

    fn lift_prefetch(&self, _instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        // PREFETCH: 预取缓存行
        Ok(r#"call void @llvm.prefetch(i64* %RSI, i32 0, i32 3, i32 1)"#.to_string())
    }

    fn lift_cmov(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "CMOV requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        let dst = self.operand_to_ir(&instr.operands[0])?;
        let src = self.operand_to_ir(&instr.operands[1])?;

        // 条件移动：if (condition) dst = src
        // 简化处理：根据 mnemonic 的条件代码选择
        Ok(format!(
            r#"br i1 %condition_flag, label %cmov_true_{}, label %cmov_false_{}
cmov_true_{}:
  store i64 {}, i64* {}
  br label %cmov_end_{}
cmov_false_{}:
  br label %cmov_end_{}
cmov_end_{}:"#,
            dst, dst, dst, src, dst, dst, dst, dst, dst
        ))
    }

    fn lift_cmpxchg(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        if instr.operands.len() != 2 {
            return Err(VmError::Core(CoreError::DecodeError {
                message: "CMPXCHG requires 2 operands".to_string(),
                position: None,
                module: "vm-ir".to_string(),
            }));
        }

        // CMPXCHG: 原子比较交换
        Ok(r#"call i64 @llvm.cmpxchg.i64(i64* %mem, i64 %RAX, i64 %RBX)
store i1 %cmpxchg_success, i1* @shadow_ZF"#
            .to_string())
    }
}

impl Semantics for X86_64Semantics {
    fn lift(&self, instr: &Instruction, ctx: &mut LiftingContext) -> LiftResult<String> {
        match instr.mnemonic.as_str() {
            "add" => self.lift_add(instr, ctx),
            "sub" => self.lift_sub(instr, ctx),
            "mov" => self.lift_mov(instr, ctx),
            "movq" => self.lift_movq(instr, ctx),
            "movsq" => self.lift_movsq(instr, ctx),
            "lea" => self.lift_lea(instr, ctx),
            "cmp" => self.lift_cmp(instr, ctx),
            "jmp" => self.lift_jmp(instr, ctx),
            "call" => self.lift_call(instr, ctx),
            "ret" => self.lift_ret(instr, ctx),
            "push" => self.lift_push(instr, ctx),
            "pop" => self.lift_pop(instr, ctx),
            "nop" => self.lift_nop(instr, ctx),
            "imul" | "mul" => self.lift_imul(instr, ctx),
            "div" | "idiv" => self.lift_div(instr, ctx),
            "xor" => self.lift_xor(instr, ctx),
            "or" => self.lift_or(instr, ctx),
            "and" => self.lift_and(instr, ctx),
            "test" => self.lift_test(instr, ctx),
            "shl" | "sal" => self.lift_shl(instr, ctx),
            "shr" => self.lift_shr(instr, ctx),
            "sar" => self.lift_sar(instr, ctx),
            "movzx" => self.lift_movzx(instr, ctx),
            "movsx" => self.lift_movsx(instr, ctx),
            "inc" => self.lift_inc(instr, ctx),
            "dec" => self.lift_dec(instr, ctx),
            "neg" => self.lift_neg(instr, ctx),
            "not" => self.lift_not(instr, ctx),
            "ror" => self.lift_ror(instr, ctx),
            "rol" => self.lift_rol(instr, ctx),
            "adc" => self.lift_adc(instr, ctx),
            "sbb" => self.lift_sbb(instr, ctx),
            "bswap" => self.lift_bswap(instr, ctx),
            "bsr" | "clz" => self.lift_clz(instr, ctx),
            "bsf" | "ctz" => self.lift_ctz(instr, ctx),
            "popcnt" => self.lift_popcnt(instr, ctx),
            "parity" => self.lift_parity(instr, ctx),
            "prefetch" => self.lift_prefetch(instr, ctx),
            "cmov" | "cmove" | "cmovne" | "cmovz" | "cmovnz" => self.lift_cmov(instr, ctx),
            "cmpxchg" => self.lift_cmpxchg(instr, ctx),
            "cpuid" => self.lift_cpuid(instr, ctx),
            "rdmsr" => self.lift_rdmsr(instr, ctx),
            "wrmsr" => self.lift_wrmsr(instr, ctx),
            _ => Err(VmError::Core(CoreError::DecodeError {
                message: format!("Unsupported x86-64 instruction: {}", instr.mnemonic),
                position: None,
                module: "vm-ir".to_string(),
            })),
        }
    }

    fn describe(&self) -> String {
        "x86-64 semantics".to_string()
    }
}

/// ARM64 指令语义实现（框架）
pub struct ARM64SemanticsImpl;
impl Default for ARM64SemanticsImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl ARM64SemanticsImpl {
    pub fn new() -> Self {
        Self
    }
}
impl Semantics for ARM64SemanticsImpl {
    fn lift(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        // Placeholder implementation
        Err(VmError::Core(CoreError::DecodeError {
            message: format!("Unsupported ARM64 instruction: {}", instr.mnemonic),
            position: None,
            module: "vm-ir".to_string(),
        }))
    }
}

/// RISC-V 指令语义实现（框架）
pub struct RISCV64Semantics;
pub struct RISCV64SemanticsImpl;
impl Default for RISCV64SemanticsImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl RISCV64SemanticsImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Semantics for RISCV64Semantics {
    fn lift(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        match instr.mnemonic.as_str() {
            "addi" | "add" => {
                let ir = format!(
                    "%res = add i64 %rs1, %{}",
                    if instr.mnemonic == "addi" {
                        "imm"
                    } else {
                        "rs2"
                    }
                );
                Ok(ir)
            }
            _ => Err(VmError::Core(CoreError::DecodeError {
                message: format!("Unsupported RISC-V instruction: {}", instr.mnemonic),
                position: None,
                module: "vm-ir".to_string(),
            })),
        }
    }

    fn describe(&self) -> String {
        "RISC-V 64-bit semantics".to_string()
    }
}
impl Semantics for RISCV64SemanticsImpl {
    fn lift(&self, instr: &Instruction, _ctx: &mut LiftingContext) -> LiftResult<String> {
        match instr.mnemonic.as_str() {
            "addi" | "add" => {
                let ir = format!(
                    "%res = add i64 %rs1, %{}",
                    if instr.mnemonic == "addi" {
                        "imm"
                    } else {
                        "rs2"
                    }
                );
                Ok(ir)
            }
            _ => Err(VmError::Core(CoreError::DecodeError {
                message: format!("Unsupported RISC-V instruction: {}", instr.mnemonic),
                position: None,
                module: "vm-ir".to_string(),
            })),
        }
    }

    fn describe(&self) -> String {
        "RISC-V 64-bit semantics".to_string()
    }
}

/// 创建指定 ISA 的语义库
pub fn create_semantics(isa: ISA) -> Box<dyn Semantics> {
    match isa {
        ISA::X86_64 => Box::new(X86_64Semantics::new()),
        ISA::ARM64 => Box::new(ARM64SemanticsImpl::new()),
        ISA::RISCV64 => Box::new(RISCV64SemanticsImpl::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x86_add_semantics() {
        let mut ctx = LiftingContext::new(ISA::X86_64);
        let semantics = X86_64Semantics::new();

        let instr = Instruction::new(
            "add".to_string(),
            vec![
                OperandType::Register("RAX".to_string()),
                OperandType::Register("RBX".to_string()),
            ],
            2,
        );

        let ir = semantics
            .lift(&instr, &mut ctx)
            .expect("should lift add instruction");
        assert!(ir.contains("add i64"));
        assert!(ir.contains("@shadow_ZF"));
    }

    #[test]
    fn test_x86_mov_semantics() {
        let mut ctx = LiftingContext::new(ISA::X86_64);
        let semantics = X86_64Semantics::new();

        let instr = Instruction::new(
            "mov".to_string(),
            vec![
                OperandType::Register("RAX".to_string()),
                OperandType::Register("RBX".to_string()),
            ],
            2,
        );

        let ir = semantics
            .lift(&instr, &mut ctx)
            .expect("should lift mov instruction");
        assert!(ir.contains("store i64"));
    }

    #[test]
    fn test_x86_nop_semantics() {
        let mut ctx = LiftingContext::new(ISA::X86_64);
        let semantics = X86_64Semantics::new();

        let instr = Instruction::new("nop".to_string(), vec![], 1);

        let ir = semantics
            .lift(&instr, &mut ctx)
            .expect("should lift nop instruction");
        assert!(ir.contains("donothing"));
    }

    #[test]
    fn test_x86_imul_semantics() {
        let mut ctx = LiftingContext::new(ISA::X86_64);
        let semantics = X86_64Semantics::new();

        let instr = Instruction::new(
            "imul".to_string(),
            vec![
                OperandType::Register("RAX".to_string()),
                OperandType::Register("RBX".to_string()),
            ],
            3,
        );

        let ir = semantics
            .lift(&instr, &mut ctx)
            .expect("should lift imul instruction");
        assert!(ir.contains("mul i64"));
    }

    #[test]
    fn test_x86_xor_semantics() {
        let mut ctx = LiftingContext::new(ISA::X86_64);
        let semantics = X86_64Semantics::new();

        let instr = Instruction::new(
            "xor".to_string(),
            vec![
                OperandType::Register("RAX".to_string()),
                OperandType::Register("RBX".to_string()),
            ],
            2,
        );

        let ir = semantics
            .lift(&instr, &mut ctx)
            .expect("should lift xor instruction");
        assert!(ir.contains("xor i64"));
    }

    #[test]
    fn test_x86_or_semantics() {
        let mut ctx = LiftingContext::new(ISA::X86_64);
        let semantics = X86_64Semantics::new();

        let instr = Instruction::new(
            "or".to_string(),
            vec![
                OperandType::Register("RAX".to_string()),
                OperandType::Register("RBX".to_string()),
            ],
            2,
        );

        let ir = semantics
            .lift(&instr, &mut ctx)
            .expect("should lift or instruction");
        assert!(ir.contains("or i64"));
    }

    #[test]
    fn test_x86_and_semantics() {
        let mut ctx = LiftingContext::new(ISA::X86_64);
        let semantics = X86_64Semantics::new();

        let instr = Instruction::new(
            "and".to_string(),
            vec![
                OperandType::Register("RAX".to_string()),
                OperandType::Register("RBX".to_string()),
            ],
            2,
        );

        let ir = semantics
            .lift(&instr, &mut ctx)
            .expect("should lift and instruction");
        assert!(ir.contains("and i64"));
    }

    #[test]
    fn test_x86_test_semantics() {
        let mut ctx = LiftingContext::new(ISA::X86_64);
        let semantics = X86_64Semantics::new();

        let instr = Instruction::new(
            "test".to_string(),
            vec![
                OperandType::Register("RAX".to_string()),
                OperandType::Register("RBX".to_string()),
            ],
            2,
        );

        let ir = semantics
            .lift(&instr, &mut ctx)
            .expect("should lift test instruction");
        assert!(ir.contains("and i64"));
    }

    #[test]
    fn test_x86_shl_semantics() {
        let mut ctx = LiftingContext::new(ISA::X86_64);
        let semantics = X86_64Semantics::new();

        let instr = Instruction::new(
            "shl".to_string(),
            vec![
                OperandType::Register("RAX".to_string()),
                OperandType::Immediate(1),
            ],
            2,
        );

        let ir = semantics
            .lift(&instr, &mut ctx)
            .expect("should lift shl instruction");
        assert!(ir.contains("shl i64"));
    }

    #[test]
    fn test_x86_shr_semantics() {
        let mut ctx = LiftingContext::new(ISA::X86_64);
        let semantics = X86_64Semantics::new();

        let instr = Instruction::new(
            "shr".to_string(),
            vec![
                OperandType::Register("RAX".to_string()),
                OperandType::Immediate(1),
            ],
            2,
        );

        let ir = semantics
            .lift(&instr, &mut ctx)
            .expect("should lift shr instruction");
        assert!(ir.contains("lshr i64"));
    }

    #[test]
    fn test_flags_state() {
        let flags = FlagsState::from_arithmetic_op("add", "%result");
        let ir = flags.to_ir();
        assert!(!ir.is_empty());
        assert!(ir.iter().any(|s| s.contains("@shadow_ZF")));
    }

    #[test]
    fn test_create_semantics() {
        let semantics = create_semantics(ISA::X86_64);
        assert_eq!(semantics.describe(), "x86-64 semantics");
    }
}
