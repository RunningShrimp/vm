//! 使用前端代码生成器生成ARM64前端代码的示例

use vm_codegen::{
    CodegenConfig, FrontendCodeGenerator, 
    create_instruction_spec, create_instruction_set
};

fn main() {
    // 创建ARM64指令集
    let arm64_instructions = vec![
        // ADD/SUB (immediate)
        create_instruction_spec(
            "ADD_IMM",
            "Add immediate",
            0x1F000000,
            0x11000000,
            r#"let sf = (insn >> 31) & 1;
                let shift = (insn >> 22) & 3;
                let imm12 = (insn >> 10) & 0xFFF;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                let mut imm = imm12 as i64;
                if shift == 1 { imm <<= 12; }

                b.push(IROp::AddImm { dst: rd, src: rn, imm });"#
        ),
        // MOVZ
        create_instruction_spec(
            "MOVZ",
            "Move wide with zero",
            0x7F800000,
            0x52800000,
            r#"let hw = (insn >> 21) & 3;
                let imm16 = (insn >> 5) & 0xFFFF;
                let rd = insn & 0x1F;
                let val = (imm16 as u64) << (hw * 16);
                b.push(IROp::MovImm { dst: rd, imm: val });"#
        ),
        // LDR (Unsigned Immediate)
        create_instruction_spec(
            "LDR_UNSIGNED",
            "Load register (unsigned immediate)",
            0x3F000000,
            0x39000000,
            r#"let size_bits = (insn >> 30) & 0x3;
                let imm12 = (insn >> 10) & 0xFFF;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let size = match size_bits {
                    0 => 1, 1 => 2, 2 => 4, 3 => 8, _ => 4
                };
                let offset = (imm12 as i64) * (size as i64);

                b.push(IROp::Load {
                    dst: rt,
                    base: rn,
                    offset,
                    size,
                    flags: MemFlags::default()
                });"#
        ),
        // B (Unconditional Branch)
        create_instruction_spec(
            "B",
            "Branch unconditionally",
            0xFC000000,
            0x14000000,
            r#"let imm26 = insn & 0x03FFFFFF;
                let offset = ((imm26 << 6) as i32 >> 6) as i64 * 4;
                let target = b.pc().wrapping_add(offset as u64);
                b.set_term(Terminator::Jmp { target });"#
        ),
        // RET
        create_instruction_spec(
            "RET",
            "Return from subroutine",
            0xFFFFFC1F,
            0xD65F0000,
            r#"b.set_term(Terminator::Ret);"#
        ),
        // Logical (shifted register) AND/ORR/EOR
        create_instruction_spec(
            "LOGICAL_REG",
            "Logical operations (shifted register)",
            0x1F000000,
            0x0A000000,
            r#"let op = (insn >> 29) & 3;
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
                b.push(irop);"#
        ),
    ];

    let instruction_set = create_instruction_set("ARM64", arm64_instructions);

    // 创建代码生成器配置
    let config = CodegenConfig {
        target_arch: "aarch64".to_string(),
        isa_version: "8.0".to_string(),
        optimization_level: 2,
        enable_debug: true,
    };

    // 创建前端代码生成器
    let generator = FrontendCodeGenerator::new(config, "ARM64", 4, false);

    // 生成前端代码
    let generated_code = generator.generate_frontend_code(&instruction_set, true);

    // 输出生成的代码
    println!("{}", generated_code);

    // 保存到文件
    std::fs::write("arm64_frontend_generated.rs", generated_code)
        .expect("Failed to write generated ARM64 frontend code");
}