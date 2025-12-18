//! 前端代码生成器完整示例
//!
//! 这个示例展示如何使用前端代码生成器生成ARM64和RISC-V前端代码，
//! 减少不同架构前端之间的重复代码。

use std::collections::HashMap;
use std::fs;

// 指令规范
#[derive(Debug, Clone)]
struct InstructionSpec {
    mnemonic: String,
    description: String,
    mask: u32,
    pattern: u32,
    handler_code: String,
}

// 指令集
struct InstructionSet {
    name: String,
    instructions: Vec<InstructionSpec>,
}

impl InstructionSet {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            instructions: Vec::new(),
        }
    }

    fn add_instruction(&mut self, spec: InstructionSpec) {
        self.instructions.push(spec);
    }
}

// 前端代码生成器
struct FrontendCodeGenerator {
    arch_name: String,
    instruction_size: u8,
    has_compressed: bool,
}

impl FrontendCodeGenerator {
    fn new(arch_name: &str, instruction_size: u8, has_compressed: bool) -> Self {
        Self {
            arch_name: arch_name.to_string(),
            instruction_size,
            has_compressed,
        }
    }

    /// 生成指令结构体
    fn generate_instruction_struct(&self) -> String {
        let arch_upper = self.arch_name.to_uppercase();
        format!(
            r#"/// {} 指令表示
#[derive(Debug, Clone)]
pub struct {}Instruction {{
    pub mnemonic: &'static str,
    pub next_pc: u64,
    pub has_memory_op: bool,
    pub is_branch: bool,
}}

impl {}Instruction {{
    pub fn new(mnemonic: &'static str, next_pc: u64, has_memory_op: bool, is_branch: bool) -> Self {{
        Self {{
            mnemonic,
            next_pc,
            has_memory_op,
            is_branch,
        }}
    }}
}}
"#,
            arch_upper, arch_upper, arch_upper
        )
    }

    /// 生成解码器结构体
    fn generate_decoder_struct(&self) -> String {
        let arch_upper = self.arch_name.to_uppercase();
        format!(
            r#"/// {} 解码器，支持解码缓存优化
pub struct {}Decoder {{
    /// 解码缓存
    decode_cache: Option<std::collections::HashMap<u64, Vec<u8>>>,
    /// 缓存大小限制
    cache_size_limit: usize,
}}

impl {}Decoder {{
    /// 创建新的解码器
    pub fn new() -> Self {{
        Self {{
            decode_cache: Some(std::collections::HashMap::new()),
            cache_size_limit: 1024,
        }}
    }}

    /// 创建不带缓存的解码器（用于测试或内存受限环境）
    pub fn without_cache() -> Self {{
        Self {{
            decode_cache: None,
            cache_size_limit: 0,
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
            arch_upper, arch_upper, arch_upper, arch_upper
        )
    }

    /// 生成解码实现
    fn generate_decode_impl(&self, instruction_set: &InstructionSet) -> String {
        let arch_upper = self.arch_name.to_uppercase();
        let mut compressed_check = String::new();
        
        if self.has_compressed {
            compressed_check = format!(
                r#"
        // 检查压缩指令
        // 压缩指令的bits [1:0] != 11
        if (insn & 0x3) != 0x3 {{
            // 这是一个16位压缩指令
            let op = (insn >> 13) & 0x7;
            let rd_rs1 = (insn >> 7) & 0x1F;
            let _rs2 = (insn >> 2) & 0x1F;
            
            // 处理压缩指令
            // 目前，我们将其视为常规指令
        }}"#
            );
        }

        let mut opcode_matches = String::new();
        let mut instruction_handlers = String::new();
        
        for spec in &instruction_set.instructions {
            let opcode = (spec.pattern & 0x7f) as u8;
            opcode_matches.push_str(&format!("\n            0x{:02x} => \"{}\",", opcode, spec.mnemonic));
            
            instruction_handlers.push_str(&format!(
                r#"
            0x{:02x} => {{
                // {}
                {}
                instructions.push({}Instruction::new(\"{}\", current_pc + {}, false, false));
            }}"#,
                opcode,
                spec.description,
                spec.handler_code,
                arch_upper,
                spec.mnemonic,
                self.instruction_size
            ));
        }

        format!(
            r#"impl {}Decoder {{
    /// 解码单条指令
    pub fn decode_insn(&mut self, insn: u32, pc: u64) -> {}Instruction {{
        let opcode = insn & 0x7f;
        
        // 确定指令助记符
        let mnemonic = match opcode {{{}}}

        // 确定指令属性
        let is_branch = matches!(opcode, 0x63 | 0x6f | 0x67);
        let has_memory_op = matches!(opcode, 0x03 | 0x23 | 0x57); // 向量加载/存储也算内存操作
        
        {}Instruction::new(mnemonic, pc + {}, has_memory_op, is_branch)
    }}
    
    /// 解码指令块
    pub fn decode_block(&mut self, data: &[u8], pc: u64) -> Result<Vec<{}Instruction>, String> {{
        let mut instructions = Vec::new();
        let mut current_pc = pc;
        let mut i = 0;
        
        while i < data.len() {{
            if i + {} > data.len() {{
                return Err("不完整的指令".to_string());
            }}
            
            let insn = u32::from_le_bytes([
                data[i],
                data[i+1],
                data[i+2],
                data[i+3]
            ]);
            
            let instruction = self.decode_insn(insn, current_pc);
            instructions.push(instruction);
            
            i += {};
            current_pc += {};
            
            // 如果是分支指令，停止解码
            if instructions.last().unwrap().is_branch {{
                break;
            }}
        }}
        
        Ok(instructions)
    }}
}}
"#,
            arch_upper,
            arch_upper,
            opcode_matches,
            arch_upper,
            self.instruction_size,
            arch_upper,
            self.instruction_size,
            self.instruction_size,
            self.instruction_size
        )
    }

    /// 生成完整的前端代码
    fn generate_frontend_code(&self, instruction_set: &InstructionSet) -> String {
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
//!
//! let mut decoder = {}Decoder::new();
//! let instructions = decoder.decode_block(&data, 0x1000)?;
//! ```

use std::collections::HashMap;

"#,
            self.arch_name.to_lowercase(),
            self.arch_name.to_uppercase(),
            self.arch_name.to_uppercase(),
            self.arch_name.to_uppercase(),
            self.arch_name.to_lowercase(),
            self.arch_name.to_uppercase(),
            self.arch_name.to_uppercase()
        ));
        
        // 添加指令结构体
        code.push_str(&self.generate_instruction_struct());
        
        // 添加解码器结构体
        code.push_str(&self.generate_decoder_struct());
        
        // 添加解码实现
        code.push_str(&self.generate_decode_impl(instruction_set));
        
        code
    }

    /// 保存生成的代码到文件
    fn save_to_file(&self, instruction_set: &InstructionSet, filename: &str) -> Result<(), std::io::Error> {
        let code = self.generate_frontend_code(instruction_set);
        fs::write(filename, code)
    }
}

fn create_arm64_instruction_set() -> InstructionSet {
    let mut arm64_set = InstructionSet::new("ARM64");
    
    // 添加ARM64指令
    arm64_set.add_instruction(InstructionSpec {
        mnemonic: "ADD_IMM".to_string(),
        description: "Add immediate".to_string(),
        mask: 0x1F000000,
        pattern: 0x11000000,
        handler_code: r#"let sf = (insn >> 31) & 1;
                let imm12 = (insn >> 10) & 0xFFF;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;
                // 处理ADD_IMM指令"#.to_string(),
    });
    
    arm64_set.add_instruction(InstructionSpec {
        mnemonic: "MOVZ".to_string(),
        description: "Move wide with zero".to_string(),
        mask: 0x7F800000,
        pattern: 0x52800000,
        handler_code: r#"let hw = (insn >> 21) & 3;
                let imm16 = (insn >> 5) & 0xFFFF;
                let rd = insn & 0x1F;
                // 处理MOVZ指令"#.to_string(),
    });
    
    arm64_set.add_instruction(InstructionSpec {
        mnemonic: "B".to_string(),
        description: "Branch unconditionally".to_string(),
        mask: 0xFC000000,
        pattern: 0x14000000,
        handler_code: r#"let imm26 = insn & 0x03FFFFFF;
                let offset = ((imm26 << 6) as i32 >> 6) as i64 * 4;
                // 处理B指令"#.to_string(),
    });
    
    arm64_set.add_instruction(InstructionSpec {
        mnemonic: "RET".to_string(),
        description: "Return from subroutine".to_string(),
        mask: 0xFFFFFC1F,
        pattern: 0xD65F0000,
        handler_code: r#"// 处理RET指令"#.to_string(),
    });
    
    arm64_set
}

fn create_riscv_instruction_set() -> InstructionSet {
    let mut riscv_set = InstructionSet::new("RISC-V");
    
    // 添加RISC-V指令
    riscv_set.add_instruction(InstructionSpec {
        mnemonic: "LUI".to_string(),
        description: "Load upper immediate".to_string(),
        mask: 0x7F,
        pattern: 0x37,
        handler_code: r#"let imm = ((insn & 0xfffff000) as i32) as i64;
                let rd = ((insn >> 7) & 0x1f) as u32;
                // 处理LUI指令"#.to_string(),
    });
    
    riscv_set.add_instruction(InstructionSpec {
        mnemonic: "ADDI".to_string(),
        description: "Add immediate".to_string(),
        mask: 0x707F,
        pattern: 0x13,
        handler_code: r#"let imm = ((insn as i32) >> 20) as i64;
                let rs1 = ((insn >> 15) & 0x1f) as u32;
                let rd = ((insn >> 7) & 0x1f) as u32;
                // 处理ADDI指令"#.to_string(),
    });
    
    riscv_set.add_instruction(InstructionSpec {
        mnemonic: "JAL".to_string(),
        description: "Jump and link".to_string(),
        mask: 0x7F,
        pattern: 0x6F,
        handler_code: r#"let rd = ((insn >> 7) & 0x1f) as u32;
                let i = insn;
                let imm = (((i >> 31) & 0x1) << 20)
                    | (((i >> 21) & 0x3ff) << 1)
                    | (((i >> 20) & 0x1) << 11)
                    | (((i >> 12) & 0xff) << 12);
                // 处理JAL指令"#.to_string(),
    });
    
    riscv_set.add_instruction(InstructionSpec {
        mnemonic: "ECALL".to_string(),
        description: "Environment call".to_string(),
        mask: 0x707F,
        pattern: 0x73,
        handler_code: r#"// 处理ECALL指令"#.to_string(),
    });
    
    riscv_set
}

fn main() {
    // 创建ARM64前端代码生成器
    let arm64_generator = FrontendCodeGenerator::new("ARM64", 4, false);
    let arm64_instruction_set = create_arm64_instruction_set();
    
    // 生成ARM64前端代码
    if let Err(e) = arm64_generator.save_to_file(&arm64_instruction_set, "arm64_frontend_generated.rs") {
        eprintln!("Failed to generate ARM64 frontend: {}", e);
    } else {
        println!("Successfully generated ARM64 frontend code");
    }
    
    // 创建RISC-V前端代码生成器
    let riscv_generator = FrontendCodeGenerator::new("RISC-V", 4, true);
    let riscv_instruction_set = create_riscv_instruction_set();
    
    // 生成RISC-V前端代码
    if let Err(e) = riscv_generator.save_to_file(&riscv_instruction_set, "riscv_frontend_generated.rs") {
        eprintln!("Failed to generate RISC-V frontend: {}", e);
    } else {
        println!("Successfully generated RISC-V frontend code");
    }
    
    // 比较生成的代码，展示重复代码的减少
    println!("\n=== 代码生成完成 ===");
    println!("生成的ARM64前端代码: arm64_frontend_generated.rs");
    println!("生成的RISC-V前端代码: riscv_frontend_generated.rs");
    println!("\n=== 代码重复分析 ===");
    println!("1. 指令结构体: 两个架构使用相同的结构模式");
    println!("2. 解码器结构体: 两个架构使用相同的缓存机制");
    println!("3. 解码实现: 两个架构使用相同的解码流程");
    println!("4. 错误处理: 两个架构使用相同的错误处理方式");
    println!("\n通过使用前端代码生成器，我们成功地:");
    println!("- 减少了重复代码");
    println!("- 提高了代码一致性");
    println!("- 简化了维护工作");
    println!("- 支持轻松添加新架构");
}