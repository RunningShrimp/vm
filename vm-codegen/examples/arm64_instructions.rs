//! ARM64指令集定义示例
//!
//! 使用代码生成工具定义ARM64指令集。

use vm_codegen::{
    CodegenConfig, InstructionSet, InstructionSpec, instruction_set, instruction_spec,
};

fn main() {
    // 创建ARM64指令集
    let arm64_set = instruction_set!(
        "ARM64",
        // ADD/SUB (immediate)
        instruction_spec!(
            "ADD_IMM",
            "Add immediate",
            0x1F000000,
            0x11000000,
            r#"                let sf = (insn >> 31) & 1;
                let shift = (insn >> 22) & 3;
                let imm12 = (insn >> 10) & 0xFFF;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                let mut imm = imm12 as i64;
                if shift == 1 { imm <<= 12; }

                builder.push(IROp::AddImm { dst: rd, src: rn, imm });"#
        ),
        // MOVZ
        instruction_spec!(
            "MOVZ",
            "Move wide with zero",
            0x7F800000,
            0x52800000,
            r#"                let hw = (insn >> 21) & 3;
                let imm16 = (insn >> 5) & 0xFFFF;
                let rd = insn & 0x1F;
                let val = (imm16 as u64) << (hw * 16);
                builder.push(IROp::MovImm { dst: rd, imm: val });"#
        ),
        // LDR (Unsigned Immediate)
        instruction_spec!(
            "LDR_UNSIGNED",
            "Load register (unsigned immediate)",
            0x3F000000,
            0x39000000,
            r#"                let size_bits = (insn >> 30) & 0x3;
                let imm12 = (insn >> 10) & 0xFFF;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let size = match size_bits {
                    0 => 1, 1 => 2, 2 => 4, 3 => 8, _ => 4
                };
                let offset = (imm12 as i64) * (size as i64);

                builder.push(IROp::Load {
                    dst: rt,
                    base: rn,
                    offset,
                    size,
                    flags: MemFlags::default()
                });"#
        ),
        // B (Unconditional Branch)
        instruction_spec!(
            "B",
            "Branch unconditionally",
            0xFC000000,
            0x14000000,
            r#"                let imm26 = insn & 0x03FFFFFF;
                let offset = ((imm26 << 6) as i32 >> 6) as i64 * 4;
                let target = current_pc.wrapping_add(offset as u64);
                builder.set_term(Terminator::Jmp { target });
                break;"#
        ),
        // RET
        instruction_spec!(
            "RET",
            "Return from subroutine",
            0xFFFFFC1F,
            0xD65F0000,
            r#"                builder.set_term(Terminator::Ret);
                break;"#
        ),
        // Logical (shifted register) AND/ORR/EOR
        instruction_spec!(
            "LOGICAL_REG",
            "Logical operations (shifted register)",
            0x1F000000,
            0x0A000000,
            r#"                let op = (insn >> 29) & 3;
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                let irop = match op {
                    0 => IROp::And { dst: rd, src1: rn, src2: rm },
                    1 => IROp::Or { dst: rd, src1: rn, src2: rm },
                    2 => IROp::Xor { dst: rd, src1: rn, src2: rm },
                    3 => IROp::And { dst: rd, src1: rn, src2: rm }, // ANDS
                    _ => return Ok(false),
                };
                builder.push(irop);"#
        ),
        // MUL/MADD/MSUB
        instruction_spec!(
            "MUL_MADD",
            "Multiply and multiply-accumulate",
            0x1F000000,
            0x1B000000,
            r#"                let op54 = (insn >> 29) & 3;
                let rm = (insn >> 16) & 0x1F;
                let ra = (insn >> 10) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                if op54 == 0 {
                    if ra == 31 {
                        builder.push(IROp::Mul { dst: rd, src1: rn, src2: rm });
                    } else {
                        let tmp = 32;
                        builder.push(IROp::Mul { dst: tmp, src1: rn, src2: rm });
                        builder.push(IROp::Add { dst: rd, src1: ra, src2: tmp });
                    }
                } else if op54 == 1 {
                    let tmp = 32;
                    builder.push(IROp::Mul { dst: tmp, src1: rn, src2: rm });
                    builder.push(IROp::Sub { dst: rd, src1: ra, src2: tmp });
                }"#
        ),
        // SDIV/UDIV
        instruction_spec!(
            "DIV",
            "Divide operations",
            0x1FE0FC00,
            0x1AC00800,
            r#"                let sf = (insn >> 31) & 1;
                let is_signed = (insn & 0x00000400) == 0;
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                builder.push(IROp::Div {
                    dst: rd,
                    src1: rn,
                    src2: rm,
                    signed: is_signed
                });"#
        )
    );

    // 生成代码
    let config = CodegenConfig {
        target_arch: "aarch64".to_string(),
        isa_version: "8.0".to_string(),
        optimization_level: 2,
        enable_debug: true,
    };

    let generated_code = arm64_set.generate_decoder(&config);

    // 输出生成的代码
    println!("{}", generated_code);

    // 或者保存到文件
    std::fs::write("arm64_decoder_generated.rs", generated_code)
        .expect("Failed to write generated code");
}
