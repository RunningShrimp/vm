//! 代码生成工具库
//!
//! 提供通用的指令解码、IR生成和代码生成工具，减少不同架构前端的重复代码。

use std::collections::HashMap;
use vm_core::{Decoder, GuestAddr, MMU, VmError};
use vm_ir::{IRBlock, IRBuilder, Terminator};

/// 模式处理器类型别名
type PatternHandler<F> = Box<dyn Fn(&mut IRBuilder, u64, &F) -> Result<(), VmError> + Send + Sync>;

/// 指令工厂类型别名
type InstructionFactory<I> = Box<dyn Fn(&str, GuestAddr, bool, bool) -> I + Send + Sync>;

// 导出前端生成器
pub mod frontend_generator;
pub use frontend_generator::{
    FrontendCodeGenerator, GenericInstruction, create_instruction_set, create_instruction_spec,
};

/// 指令字段提取器
pub trait FieldExtractor {
    /// 提取位字段
    fn extract_field(&self, insn: u64, start: u32, width: u32) -> u64;
    /// 符号扩展字段
    fn sign_extend(&self, value: u64, width: u32) -> i64;
}

/// 标准字段提取器实现
pub struct StandardFieldExtractor;

impl FieldExtractor for StandardFieldExtractor {
    fn extract_field(&self, insn: u64, start: u32, width: u32) -> u64 {
        (insn >> start) & ((1u64 << width) - 1)
    }

    fn sign_extend(&self, value: u64, width: u32) -> i64 {
        let shift = 64 - width;
        ((value as i64) << shift) >> shift
    }
}

/// 指令模式匹配器
pub struct PatternMatcher<F: FieldExtractor> {
    patterns: HashMap<u64, PatternHandler<F>>,
    extractor: F,
}

impl<F: FieldExtractor> PatternMatcher<F> {
    pub fn new(extractor: F) -> Self {
        Self {
            patterns: HashMap::new(),
            extractor,
        }
    }

    /// 注册指令模式
    pub fn register_pattern<P>(&mut self, mask: u64, pattern: u64, handler: P)
    where
        P: Fn(&mut IRBuilder, u64, &F) -> Result<(), VmError> + Send + Sync + 'static,
    {
        let key = (mask << 32) | pattern;
        self.patterns.insert(key, Box::new(handler));
    }

    /// 匹配并处理指令
    pub fn match_and_handle(&self, builder: &mut IRBuilder, insn: u64) -> Result<bool, VmError> {
        for (&key, handler) in &self.patterns {
            let mask = key >> 32;
            let pattern = key & 0xFFFFFFFF;

            if (insn & mask) == pattern {
                handler(builder, insn, &self.extractor)?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// 通用指令解码器
pub struct GenericDecoder<F: FieldExtractor, I> {
    matcher: PatternMatcher<F>,
    instruction_factory: InstructionFactory<I>,
}

impl<F: FieldExtractor + 'static, I> GenericDecoder<F, I> {
    pub fn new(extractor: F, instruction_factory: InstructionFactory<I>) -> Self {
        Self {
            matcher: PatternMatcher::new(extractor),
            instruction_factory,
        }
    }

    pub fn matcher_mut(&mut self) -> &mut PatternMatcher<F> {
        &mut self.matcher
    }
}

impl<F: FieldExtractor + 'static + Send, I> Decoder for GenericDecoder<F, I> {
    type Instruction = I;
    type Block = IRBlock;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Instruction, VmError> {
        let _insn = mmu.fetch_insn(pc)?;

        // 这里需要实现指令识别逻辑
        // 为了简化，我们返回一个基本的指令
        Ok((self.instruction_factory)(
            "unknown",
            GuestAddr(pc.0 + 4), // 假设4字节指令
            false,
            false,
        ))
    }

    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, VmError> {
        let mut builder = IRBuilder::new(pc);
        let mut current_pc = pc;

        loop {
            let insn = mmu.fetch_insn(current_pc)?;

            if self.matcher.match_and_handle(&mut builder, insn)? {
                // 匹配成功，指令已处理
                current_pc += 4; // 假设4字节指令
                continue;
            } else {
                // 未匹配的指令
                builder.set_term(Terminator::Fault { cause: 0 });
                break;
            }
        }

        Ok(builder.build())
    }
}

/// 代码生成器配置
#[derive(Debug, Clone)]
pub struct CodegenConfig {
    /// 目标架构
    pub target_arch: String,
    /// 指令集版本
    pub isa_version: String,
    /// 优化级别
    pub optimization_level: u32,
    /// 启用调试信息
    pub enable_debug: bool,
}

impl Default for CodegenConfig {
    fn default() -> Self {
        Self {
            target_arch: "unknown".to_string(),
            isa_version: "1.0".to_string(),
            optimization_level: 1,
            enable_debug: false,
        }
    }
}

/// 代码生成器
pub struct CodeGenerator {
    config: CodegenConfig,
    generated_code: Vec<String>,
}

impl CodeGenerator {
    pub fn new(config: CodegenConfig) -> Self {
        Self {
            config,
            generated_code: Vec::new(),
        }
    }

    /// 生成指令解码函数
    pub fn generate_decoder_function(&mut self, arch_name: &str, instructions: &[InstructionSpec]) {
        let mut code = format!("impl Decoder for {}Decoder {{\n", arch_name);
        code.push_str("    type Instruction = Arm64Instruction;\n");
        code.push_str("    type Block = IRBlock;\n\n");

        code.push_str("    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, VmError> {\n");
        code.push_str("        let mut builder = IRBuilder::new(pc);\n");
        code.push_str("        let mut current_pc = pc;\n\n");
        code.push_str("        loop {\n");
        code.push_str("            let insn = mmu.fetch_insn(current_pc)? as u32;\n\n");

        for spec in instructions {
            code.push_str(&format!(
                "            // {}: {}\n",
                spec.mnemonic, spec.description
            ));
            code.push_str(&format!(
                "            if (insn & 0x{:08x}) == 0x{:08x} {{\n",
                spec.mask, spec.pattern
            ));
            code.push_str(&spec.handler_code);
            code.push_str("                current_pc += 4;\n");
            code.push_str("                continue;\n");
            code.push_str("            }\n\n");
        }

        code.push_str("            builder.set_term(Terminator::Fault { cause: 0 });\n");
        code.push_str("            break;\n");
        code.push_str("        }\n");
        code.push_str("        Ok(builder.build())\n");
        code.push_str("    }\n");
        code.push_str("}\n");

        self.generated_code.push(code);
    }

    /// 获取生成的代码
    pub fn get_generated_code(&self) -> &[String] {
        &self.generated_code
    }

    /// 获取代码生成器配置
    pub fn get_config(&self) -> &CodegenConfig {
        &self.config
    }

    /// 生成完整文件
    pub fn generate_file(&self, filename: &str) -> String {
        let mut content = format!("//! Auto-generated {} decoder\n\n", filename);
        content.push_str("use vm_core::{Decoder, Instruction, MMU, GuestAddr, VmError};\n");
        content.push_str("use vm_ir::{IRBlock, IRBuilder, IROp, Terminator, MemFlags};\n\n");

        for code in &self.generated_code {
            content.push_str(code);
            content.push('\n');
        }

        content
    }
}

/// 指令规范
#[derive(Debug, Clone)]
pub struct InstructionSpec {
    /// 指令助记符
    pub mnemonic: String,
    /// 指令描述
    pub description: String,
    /// 位掩码
    pub mask: u32,
    /// 位模式
    pub pattern: u32,
    /// 处理代码
    pub handler_code: String,
}

/// 指令集定义
pub struct InstructionSet {
    pub name: String,
    pub instructions: Vec<InstructionSpec>,
}

impl InstructionSet {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            instructions: Vec::new(),
        }
    }

    pub fn add_instruction(&mut self, spec: InstructionSpec) {
        self.instructions.push(spec);
    }

    /// 生成解码器代码
    pub fn generate_decoder(&self, config: &CodegenConfig) -> String {
        let mut generator = CodeGenerator::new(config.clone());
        generator.generate_decoder_function(&self.name, &self.instructions);
        generator.generate_file(&format!("{}_decoder.rs", self.name.to_lowercase()))
    }
}

/// 宏辅助函数
/// 创建指令规范
#[macro_export]
macro_rules! instruction_spec {
    ($mnemonic:expr, $desc:expr, $mask:expr, $pattern:expr, $handler:expr) => {
        InstructionSpec {
            mnemonic: $mnemonic.to_string(),
            description: $desc.to_string(),
            mask: $mask,
            pattern: $pattern,
            handler_code: $handler.to_string(),
        }
    };
}

/// 创建指令集
#[macro_export]
macro_rules! instruction_set {
    ($name:expr, $($spec:expr),* $(,)?) => {
        {
            let mut set = InstructionSet::new($name);
            $(
                set.add_instruction($spec);
            )*
            set
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_extractor() {
        let extractor = StandardFieldExtractor;

        // 测试字段提取
        let insn = 0x12345678u64;
        assert_eq!(extractor.extract_field(insn, 0, 8), 0x78);
        assert_eq!(extractor.extract_field(insn, 8, 8), 0x56);

        // 测试符号扩展
        assert_eq!(extractor.sign_extend(0x7F, 8), 127);
        assert_eq!(extractor.sign_extend(0x80, 8), -128);
    }

    #[test]
    fn test_pattern_matcher() {
        let extractor = StandardFieldExtractor;
        let mut matcher = PatternMatcher::new(extractor);

        // 注册一个简单的模式
        matcher.register_pattern(
            0xFC000000, // 6位opcode掩码
            0x14000000, // B指令opcode
            |builder, insn, extractor| {
                let imm26 = extractor.extract_field(insn, 0, 26) as i32;
                let offset = ((imm26 << 6) >> 6) * 4;
                let target = builder.pc().wrapping_add(offset as u64);
                builder.set_term(Terminator::Jmp { target });
                Ok(())
            },
        );

        let mut builder = IRBuilder::new(vm_core::GuestAddr(0x1000));

        // 测试匹配
        let b_insn = 0x14000000; // B指令
        assert!(matcher.match_and_handle(&mut builder, b_insn).unwrap());

        // 测试不匹配
        let other_insn = 0x94000000; // BL指令
        assert!(!matcher.match_and_handle(&mut builder, other_insn).unwrap());
    }

    #[test]
    fn test_instruction_set() {
        let mut set = InstructionSet::new("TestISA");

        let add_spec = instruction_spec!(
            "ADD",
            "Add immediate",
            0x1F000000,
            0x11000000,
            "                let rd = (insn >> 0) & 0x1F;\n                let rn = (insn >> 5) & 0x1F;\n                let imm = (insn >> 10) & 0xFFF;\n                builder.push(IROp::AddImm { dst: rd, src: rn, imm: imm as i64 });"
        );

        set.add_instruction(add_spec);

        assert_eq!(set.name, "TestISA");
        assert_eq!(set.instructions.len(), 1);
        assert_eq!(set.instructions[0].mnemonic, "ADD");
    }

    #[test]
    fn test_code_generation() {
        let config = CodegenConfig {
            target_arch: "test".to_string(),
            isa_version: "1.0".to_string(),
            optimization_level: 1,
            enable_debug: false,
        };

        let mut set = InstructionSet::new("TestArch");
        let add_spec = instruction_spec!(
            "ADD",
            "Add registers",
            0x1F000000,
            0x0B000000,
            "                let rd = (insn >> 0) & 0x1F;\n                let rn = (insn >> 5) & 0x1F;\n                let rm = (insn >> 16) & 0x1F;\n                builder.push(IROp::Add { dst: rd, src1: rn, src2: rm });"
        );
        set.add_instruction(add_spec);

        let code = set.generate_decoder(&config);
        assert!(code.contains("TestArch"));
        assert!(code.contains("ADD"));
        assert!(code.contains("IROp::Add"));
    }
}
