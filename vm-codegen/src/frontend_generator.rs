//! 前端代码生成器
//!
//! 提供通用的前端代码生成功能，减少不同架构前端之间的重复代码。

use crate::{CodegenConfig, InstructionSet, InstructionSpec};
use vm_core::GuestAddr;
use vm_ir::IRBlock;

/// 通用指令结构
#[derive(Debug, Clone)]
pub struct GenericInstruction {
    pub mnemonic: String,
    pub next_pc: GuestAddr,
    pub has_memory_op: bool,
    pub is_branch: bool,
    pub size: u8,
    pub operand_count: usize,
}

impl GenericInstruction {
    pub fn next_pc(&self) -> GuestAddr {
        self.next_pc
    }

    pub fn size(&self) -> u8 {
        self.size
    }

    pub fn operand_count(&self) -> usize {
        self.operand_count
    }

    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }

    pub fn is_control_flow(&self) -> bool {
        self.is_branch
    }

    pub fn is_memory_access(&self) -> bool {
        self.has_memory_op
    }
}

/// 通用解码器结构
#[derive(Debug, Clone)]
pub struct GenericDecoder {
    pub decode_cache: Option<std::collections::HashMap<GuestAddr, IRBlock>>,
    pub cache_size_limit: usize,
}

impl GenericDecoder {
    pub fn new() -> Self {
        Self {
            decode_cache: Some(std::collections::HashMap::new()),
            cache_size_limit: 1024,
        }
    }

    pub fn without_cache() -> Self {
        Self {
            decode_cache: None,
            cache_size_limit: 0,
        }
    }

    pub fn clear_cache(&mut self) {
        if let Some(ref mut cache) = self.decode_cache {
            cache.clear();
        }
    }

    pub fn cache_stats(&self) -> (usize, usize) {
        if let Some(ref cache) = self.decode_cache {
            (cache.len(), self.cache_size_limit)
        } else {
            (0, 0)
        }
    }
}

impl Default for GenericDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// 前端代码生成器
pub struct FrontendCodeGenerator {
    config: CodegenConfig,
    arch_name: String,
    instruction_size: u8,
    has_compressed: bool,
}

impl FrontendCodeGenerator {
    pub fn new(
        config: CodegenConfig,
        arch_name: &str,
        instruction_size: u8,
        has_compressed: bool,
    ) -> Self {
        Self {
            config,
            arch_name: arch_name.to_string(),
            instruction_size,
            has_compressed,
        }
    }

    /// 获取代码生成器配置
    pub fn get_config(&self) -> &CodegenConfig {
        &self.config
    }

    /// 根据配置生成优化级别的代码
    pub fn generate_optimized_code(&self, instruction_set: &InstructionSet) -> String {
        let mut code = self.generate_frontend_code(instruction_set, false);

        // 根据优化级别添加额外的优化注释
        if self.config.optimization_level > 0 {
            code = format!(
                "// Optimization Level: {}\n{}",
                self.config.optimization_level, code
            );
        }

        // 如果启用了调试信息，则添加调试相关的注释
        if self.config.enable_debug {
            code = format!("// Debug Information Enabled\n{}", code);
        }

        code
    }

    /// 生成指令结构体
    pub fn generate_instruction_struct(&self) -> String {
        let arch_upper = self.arch_name.to_uppercase();
        format!(
            r#"/// {} 指令表示
#[derive(Debug, Clone)]
pub struct {}Instruction {{
    pub mnemonic: &'static str,
    pub next_pc: GuestAddr,
    pub has_memory_op: bool,
    pub is_branch: bool,
}}

impl Instruction for {}Instruction {{
    fn next_pc(&self) -> GuestAddr {{
        self.next_pc
    }}

    fn size(&self) -> u8 {{
        {} // {} 指令固定 {} 字节
    }}

    fn operand_count(&self) -> usize {{
        1 // 简化实现
    }}

    fn mnemonic(&self) -> &str {{
        self.mnemonic
    }}

    fn is_control_flow(&self) -> bool {{
        self.is_branch
    }}

    fn is_memory_access(&self) -> bool {{
        self.has_memory_op
    }}
}}
"#,
            arch_upper,
            arch_upper,
            arch_upper,
            self.instruction_size,
            arch_upper,
            self.instruction_size
        )
    }

    /// 生成解码器结构体
    pub fn generate_decoder_struct(&self, has_extensions: bool) -> String {
        let arch_upper = self.arch_name.to_uppercase();
        let mut extensions = String::new();

        if has_extensions {
            extensions = r#"    /// 扩展指令解码器
    pub extension_decoders: HashMap<String, Box<dyn ExtensionDecoder>>,"#
                .to_string();
        }

        format!(
            r#"/// {} 解码器，支持解码缓存优化
pub struct {}Decoder {{
    /// 解码缓存
    decode_cache: Option<std::collections::HashMap<GuestAddr, IRBlock>>,
    /// 缓存大小限制
    cache_size_limit: usize,{}
}}

impl {}Decoder {{
    /// 创建新的解码器
    pub fn new() -> Self {{
        Self {{
            decode_cache: Some(std::collections::HashMap::new()),
            cache_size_limit: 1024,{}
        }}
    }}

    /// 创建不带缓存的解码器（用于测试或内存受限环境）
    pub fn without_cache() -> Self {{
        Self {{
            decode_cache: None,
            cache_size_limit: 0,{}
        }}
    }}

    /// 清除解码缓存
    pub fn clear_cache(&mut self) {{
        if let Some(ref mut cache) = self.decode_cache {{
            cache.clear();
        }}
    }}

    /// 获取缓存统计信息
    pub fn cache_stats(&self) -> (usize, usize) {{
        if let Some(ref cache) = self.decode_cache {{
            (cache.len(), self.cache_size_limit)
        }} else {{
            (0, 0)
        }}
    }}
}}

impl Default for {}Decoder {{
    fn default() -> Self {{
        Self::new()
    }}
}}
"#,
            arch_upper,
            arch_upper,
            extensions,
            if has_extensions {
                "\n            extension_decoders: HashMap::new(),"
            } else {
                ""
            },
            arch_upper,
            if has_extensions {
                "\n            extension_decoders: HashMap::new(),"
            } else {
                ""
            },
            arch_upper
        )
    }

    /// 生成解码实现
    pub fn generate_decode_impl(&self, instruction_set: &InstructionSet) -> String {
        let arch_upper = self.arch_name.to_uppercase();
        let mut compressed_check = String::new();

        if self.has_compressed {
            compressed_check = r#"
        // Check for compressed instructions
        // Compressed instructions have bits [1:0] != 11
        if (insn & 0x3) != 0x3 {
            // This is a 16-bit compressed instruction
            let op = (insn >> 13) & 0x7;
            let rd_rs1 = (insn >> 7) & 0x1F;
            let _rs2 = (insn >> 2) & 0x1F;
            
            // Handle compressed instructions here
            // For now, we'll treat them as regular instructions
        }"#
            .to_string();
        }

        format!(
            r#"impl Decoder for {}Decoder {{
    type Instruction = {}Instruction;
    type Block = IRBlock;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Instruction, VmError> {{
        let insn = mmu.fetch_insn(pc)? as u32;
        let opcode = insn & 0x7f;

        // Determine mnemonic based on opcode
        let mnemonic = match opcode {{{}}}

        let is_branch = matches!(opcode, 0x63 | 0x6f | 0x67);
        let has_memory_op = matches!(opcode, 0x03 | 0x23 | 0x57); // 向量加载/存储也算内存操作

        Ok({}Instruction {{
            mnemonic,
            next_pc: pc + {},
            has_memory_op,
            is_branch,
        }})
    }}

    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, VmError> {{
        let insn = mmu.fetch_insn(pc)? as u32;
        let mut reg_file = vm_ir::RegisterFile::new(32, vm_ir::RegisterMode::SSA);
        let mut b = vm_ir::IRBuilder::new(pc);{}
        
        // 指令解码逻辑
        let opcode = insn & 0x7f;
        
        match opcode {{{}}}
        
        b.build()
    }}
}}
"#,
            arch_upper,
            arch_upper,
            self.generate_opcode_matches(instruction_set),
            arch_upper,
            self.instruction_size,
            compressed_check,
            self.generate_instruction_handlers(instruction_set)
        )
    }

    /// 生成操作码匹配
    fn generate_opcode_matches(&self, instruction_set: &InstructionSet) -> String {
        let mut matches = String::new();

        for spec in &instruction_set.instructions {
            let opcode = (spec.pattern & 0x7f) as u8;
            matches.push_str(&format!(
                "\n            0x{:02x} => \"{}\",",
                opcode, spec.mnemonic
            ));
        }

        matches
    }

    /// 生成指令处理器
    fn generate_instruction_handlers(&self, instruction_set: &InstructionSet) -> String {
        let mut handlers = String::new();

        for spec in &instruction_set.instructions {
            let opcode = (spec.pattern & 0x7f) as u8;
            handlers.push_str(&format!(
                r#"
            0x{:02x} => {{
                // {}
                {}
                b.set_term(Terminator::Jmp {{ target: pc + {} }});
            }}"#,
                opcode, spec.description, spec.handler_code, self.instruction_size
            ));
        }

        handlers
    }

    /// 生成完整的前端代码
    pub fn generate_frontend_code(
        &self,
        instruction_set: &InstructionSet,
        has_extensions: bool,
    ) -> String {
        let mut code = String::new();

        // 添加文件头
        code.push_str(&format!(
            r#"//! # vm-frontend-{} - {} 前端解码器
//!
//! 提供 {} 架构的指令解码器，将 {} 机器码转换为 VM IR。
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use vm_frontend_{}::{}Decoder;
//! use vm_core::Decoder;
//!
//! let mut decoder = {}Decoder::new();
//! let block = decoder.decode(&mmu, 0x1000)?;
//! ```

use vm_core::{{Decoder, GuestAddr, Instruction, MMU, VmError}};
use vm_ir::{{IRBlock, IRBuilder, IROp, MemFlags, RegisterFile, Terminator}};

"#,
            self.arch_name.to_lowercase(),
            self.arch_name.to_uppercase(),
            self.arch_name.to_uppercase(),
            self.arch_name.to_uppercase(),
            self.arch_name.to_lowercase(),
            self.arch_name.to_uppercase(),
            self.arch_name.to_uppercase()
        ));

        // 添加扩展解码器trait（如果需要）
        if has_extensions {
            code.push_str(
                r#"/// 扩展解码器trait
pub trait ExtensionDecoder: Send + Sync {
    fn decode(&self, insn: u32, builder: &mut IRBuilder) -> Result<bool, VmError>;
    fn name(&self) -> &str;
}

"#,
            );
        }

        // 添加指令结构体
        code.push_str(&self.generate_instruction_struct());

        // 添加解码器结构体
        code.push_str(&self.generate_decoder_struct(has_extensions));

        // 添加解码实现
        code.push_str(&self.generate_decode_impl(instruction_set));

        code
    }

    /// 保存生成的代码到文件
    pub fn save_to_file(
        &self,
        instruction_set: &InstructionSet,
        has_extensions: bool,
        filename: &str,
    ) -> Result<(), std::io::Error> {
        let code = self.generate_frontend_code(instruction_set, has_extensions);
        std::fs::write(filename, code)
    }
}

/// 创建通用指令规范的辅助函数
pub fn create_instruction_spec(
    mnemonic: &str,
    description: &str,
    mask: u32,
    pattern: u32,
    handler_code: &str,
) -> InstructionSpec {
    InstructionSpec {
        mnemonic: mnemonic.to_string(),
        description: description.to_string(),
        mask,
        pattern,
        handler_code: handler_code.to_string(),
    }
}

/// 创建通用指令集的辅助函数
pub fn create_instruction_set(name: &str, specs: Vec<InstructionSpec>) -> InstructionSet {
    let mut set = InstructionSet::new(name);
    for spec in specs {
        set.add_instruction(spec);
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontend_code_generation() {
        let config = CodegenConfig {
            target_arch: "test".to_string(),
            isa_version: "1.0".to_string(),
            optimization_level: 1,
            enable_debug: false,
        };

        let generator = FrontendCodeGenerator::new(config, "TestArch", 4, false);

        // 测试配置访问
        assert_eq!(generator.get_config().target_arch, "test");
        assert_eq!(generator.get_config().optimization_level, 1);

        let add_spec = create_instruction_spec(
            "ADD",
            "Add registers",
            0x1F000000,
            0x0B000000,
            "let rd = (insn >> 0) & 0x1F;\n                let rn = (insn >> 5) & 0x1F;\n                let rm = (insn >> 16) & 0x1F;\n                b.push(IROp::Add { dst: rd, src1: rn, src2: rm });",
        );

        let instruction_set = create_instruction_set("TestArch", vec![add_spec]);

        let code = generator.generate_frontend_code(&instruction_set, false);

        assert!(code.contains("TestArch"));
        assert!(code.contains("ADD"));
        assert!(code.contains("IROp::Add"));

        // 测试优化代码生成
        let optimized_code = generator.generate_optimized_code(&instruction_set);
        assert!(optimized_code.contains("// Optimization Level: 1"));
    }
}
