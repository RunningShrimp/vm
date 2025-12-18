//! 代码生成器接口和实现
//!
//! 定义了代码生成器的抽象接口和默认实现，负责将优化后的IR块转换为目标机器码。

use std::collections::HashMap;
use vm_core::VmError;
use vm_ir::IROp;

/// 代码生成器接口
pub trait CodeGenerator: Send + Sync {
    /// 生成机器码
    fn generate(&mut self, block: &crate::compiler::CompiledIRBlock) -> Result<crate::core::JITCompilationResult, VmError>;
    
    /// 获取代码生成器名称
    fn name(&self) -> &str;
    
    /// 获取代码生成器版本
    fn version(&self) -> &str;
    
    /// 设置代码生成选项
    fn set_option(&mut self, option: &str, value: &str) -> Result<(), VmError>;
    
    /// 获取代码生成选项
    fn get_option(&self, option: &str) -> Option<String>;
    
    /// 重置代码生成器状态
    fn reset(&mut self);
    
    /// 获取代码生成统计信息
    fn get_stats(&self) -> CodeGenerationStats;
}

/// 代码生成统计信息
#[derive(Debug, Clone, Default)]
pub struct CodeGenerationStats {
    /// 原始IR指令数量
    pub original_insn_count: usize,
    /// 生成的机器码指令数量
    pub machine_insn_count: usize,
    /// 生成的机器码字节数
    pub machine_code_bytes: usize,
    /// 代码生成耗时（纳秒）
    pub generation_time_ns: u64,
    /// 代码密度（字节/指令）
    pub code_density: f64,
}

/// 目标架构
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetArch {
    /// x86-64
    X86_64,
    /// ARM64
    AArch64,
    /// RISC-V 64
    RiscV64,
}

/// 代码生成模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeGenMode {
    /// 紧凑模式（优先代码大小）
    Compact,
    /// 快速模式（优先执行速度）
    Fast,
    /// 平衡模式（平衡代码大小和速度）
    Balanced,
}

/// 默认代码生成器实现
pub struct DefaultCodeGenerator {
    /// 代码生成器名称
    name: String,
    /// 代码生成器版本
    version: String,
    /// 代码生成选项
    options: HashMap<String, String>,
    /// 目标架构
    target_arch: TargetArch,
    /// 代码生成模式
    code_gen_mode: CodeGenMode,
    /// 代码生成统计
    stats: CodeGenerationStats,
    /// 寄存器编码映射
    register_encoding: HashMap<String, u8>,
}

impl DefaultCodeGenerator {
    /// 创建新的默认代码生成器
    pub fn new() -> Self {
        let mut register_encoding = HashMap::new();
        
        // 初始化x86-64寄存器编码
        register_encoding.insert("RAX".to_string(), 0);
        register_encoding.insert("RCX".to_string(), 1);
        register_encoding.insert("RDX".to_string(), 2);
        register_encoding.insert("RBX".to_string(), 3);
        register_encoding.insert("RSP".to_string(), 4);
        register_encoding.insert("RBP".to_string(), 5);
        register_encoding.insert("RSI".to_string(), 6);
        register_encoding.insert("RDI".to_string(), 7);
        register_encoding.insert("R8".to_string(), 8);
        register_encoding.insert("R9".to_string(), 9);
        register_encoding.insert("R10".to_string(), 10);
        register_encoding.insert("R11".to_string(), 11);
        register_encoding.insert("R12".to_string(), 12);
        register_encoding.insert("R13".to_string(), 13);
        register_encoding.insert("R14".to_string(), 14);
        register_encoding.insert("R15".to_string(), 15);
        
        Self {
            name: "DefaultCodeGenerator".to_string(),
            version: "1.0.0".to_string(),
            options: HashMap::new(),
            target_arch: TargetArch::X86_64,
            code_gen_mode: CodeGenMode::Balanced,
            stats: CodeGenerationStats::default(),
            register_encoding,
        }
    }
    
    /// 生成x86-64机器码
    fn generate_x86_64(&mut self, block: &crate::compiler::CompiledIRBlock) -> Result<Vec<u8>, VmError> {
        let mut code = Vec::new();
        
        for op in &block.ops {
            let op_code = self.generate_x86_64_instruction(op)?;
            code.extend_from_slice(&op_code);
        }
        
        Ok(code)
    }
    
    /// 生成单条x86-64指令
    fn generate_x86_64_instruction(&self, op: &crate::compiler::CompiledIROp) -> Result<Vec<u8>, VmError> {
        match &op.op {
            IROp::MovImm { dst, imm } => {
                // 生成 MOV reg, imm64 指令
                let mut code = Vec::new();
                
                // 获取目标寄存器的编码
                let dst_reg = op.register_allocation.get(&format!("v{}", dst))
                    .cloned().unwrap_or_else(|| format!("R{}", dst));
                
                if let Some(&reg_encoding) = self.register_encoding.get(&dst_reg) {
                    // REX.W + B + opcode + rd
                    code.push(0x48 | (reg_encoding >> 3 & 1) as u8); // REX.W
                    code.push(0xB8 | (reg_encoding & 7) as u8); // MOV r64, imm64 opcode
                    
                    // 立即数（小端序）
                    code.extend_from_slice(&imm.to_le_bytes());
                } else if dst_reg.starts_with("stack[") {
                    // 处理栈槽
                    let slot_str = dst_reg.strip_prefix("stack[").unwrap_or("");
                    let slot_str = slot_str.strip_suffix("]").unwrap_or("");
                    if let Ok(slot) = slot_str.parse::<usize>() {
                        // 生成 MOV [RSP + offset], imm64 指令
                        code.push(0x48); // REX.W
                        code.push(0xC7); // MOV r/m64, imm32 opcode
                        code.push(0x84); // ModR/M: [RSP + disp32]
                        code.push(0x24); // SIB: [RSP]
                        
                        // 偏移量（假设每个栈槽8字节）
                        let offset = (slot * 8) as i32;
                        code.extend_from_slice(&offset.to_le_bytes());
                        
                        // 立即数（32位）
                        code.extend_from_slice(&(*imm as u32).to_le_bytes());
                    }
                } else {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: format!("Unknown register: {}", dst_reg),
                        module: "DefaultCodeGenerator".to_string(),
                    }));
                }
                
                Ok(code)
            }
            IROp::Add { dst, src1: _, src2 } => {
                // 生成 ADD dst, src 指令
                let mut code = Vec::new();
                
                // 获取目标寄存器和源寄存器的编码
                let dst_reg = op.register_allocation.get(&format!("v{}", dst))
                    .cloned().unwrap_or_else(|| format!("R{}", dst));
                let src2_reg = op.register_allocation.get(&format!("v{}", src2))
                    .cloned().unwrap_or_else(|| format!("R{}", src2));
                
                if let (Some(&dst_encoding), Some(&src_encoding)) = (
                    self.register_encoding.get(&dst_reg),
                    self.register_encoding.get(&src2_reg)
                ) {
                    // REX.W + R + opcode + ModR/M
                    code.push(0x48 | ((dst_encoding >> 3) as u8) << 2 | ((src_encoding >> 3) as u8)); // REX.W
                    code.push(0x01); // ADD r/m64, r64 opcode
                    
                    // ModR/M byte
                    let modrm = (0 << 6) | ((src_encoding & 7) << 3) | (dst_encoding & 7);
                    code.push(modrm as u8);
                } else {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: format!("Unknown registers: {}, {}", dst_reg, src2_reg),
                        module: "DefaultCodeGenerator".to_string(),
                    }));
                }
                
                Ok(code)
            }
            IROp::Sub { dst, src1: _, src2 } => {
                // 生成 SUB dst, src 指令
                let mut code = Vec::new();
                
                // 获取目标寄存器和源寄存器的编码
                let dst_reg = op.register_allocation.get(&format!("v{}", dst))
                    .cloned().unwrap_or_else(|| format!("R{}", dst));
                let src2_reg = op.register_allocation.get(&format!("v{}", src2))
                    .cloned().unwrap_or_else(|| format!("R{}", src2));
                
                if let (Some(&dst_encoding), Some(&src_encoding)) = (
                    self.register_encoding.get(&dst_reg),
                    self.register_encoding.get(&src2_reg)
                ) {
                    // REX.W + R + opcode + ModR/M
                    code.push(0x48 | ((dst_encoding >> 3) as u8) << 2 | ((src_encoding >> 3) as u8)); // REX.W
                    code.push(0x29); // SUB r/m64, r64 opcode
                    
                    // ModR/M byte
                    let modrm = (0 << 6) | ((src_encoding & 7) << 3) | (dst_encoding & 7);
                    code.push(modrm as u8);
                } else {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: format!("Unknown registers: {}, {}", dst_reg, src2_reg),
                        module: "DefaultCodeGenerator".to_string(),
                    }));
                }
                
                Ok(code)
            }
            IROp::Mul { dst, src1: _, src2 } => {
                // 生成 IMUL dst, src 指令（三操作数形式）
                let mut code = Vec::new();
                
                // 获取目标寄存器和源寄存器的编码
                let dst_reg = op.register_allocation.get(&format!("v{}", dst))
                    .cloned().unwrap_or_else(|| format!("R{}", dst));
                let src2_reg = op.register_allocation.get(&format!("v{}", src2))
                    .cloned().unwrap_or_else(|| format!("R{}", src2));
                
                if let (Some(&dst_encoding), Some(&src_encoding)) = (
                    self.register_encoding.get(&dst_reg),
                    self.register_encoding.get(&src2_reg)
                ) {
                    // REX.W + R + opcode + ModR/M
                    code.push(0x48 | ((dst_encoding >> 3) as u8) << 2 | ((src_encoding >> 3) as u8)); // REX.W
                    code.push(0x0F); // 扩展操作码
                    code.push(0xAF); // IMUL r64, r/m64 opcode
                    
                    // ModR/M byte
                    let modrm = (0 << 6) | ((dst_encoding & 7) << 3) | (src_encoding & 7);
                    code.push(modrm as u8);
                } else {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: format!("Unknown registers: {}, {}", dst_reg, src2_reg),
                        module: "DefaultCodeGenerator".to_string(),
                    }));
                }
                
                Ok(code)
            }
            IROp::Load { dst, base, offset, size, .. } => {
                // 生成 MOV dst, [base + offset] 指令
                let mut code = Vec::new();
                
                // 获取目标寄存器和基址寄存器的编码
                let dst_reg = op.register_allocation.get(&format!("v{}", dst))
                    .cloned().unwrap_or_else(|| format!("R{}", dst));
                let base_reg = op.register_allocation.get(&format!("v{}", base))
                    .cloned().unwrap_or_else(|| format!("R{}", base));
                
                if let (Some(&dst_encoding), Some(&base_encoding)) = (
                    self.register_encoding.get(&dst_reg),
                    self.register_encoding.get(&base_reg)
                ) {
                    // 根据加载大小选择指令
                    match size {
                        1 => {
                            // MOVZX r64, r/m8
                            code.push(0x48 | ((dst_encoding >> 3) as u8) << 2 | ((base_encoding >> 3) as u8)); // REX.W
                            code.push(0x0F); // 扩展操作码
                            code.push(0xB6); // MOVZX r64, r/m8 opcode
                        }
                        2 => {
                            // MOVZX r64, r/m16
                            code.push(0x48 | ((dst_encoding >> 3) as u8) << 2 | ((base_encoding >> 3) as u8)); // REX.W
                            code.push(0x0F); // 扩展操作码
                            code.push(0xB7); // MOVZX r64, r/m16 opcode
                        }
                        4 => {
                            // MOV r64, r/m32 (零扩展)
                            code.push(0x48 | ((dst_encoding >> 3) as u8) << 2 | ((base_encoding >> 3) as u8)); // REX.W
                            code.push(0x8B); // MOV r64, r/m32 opcode
                        }
                        8 => {
                            // MOV r64, r/m64
                            code.push(0x48 | ((dst_encoding >> 3) as u8) << 2 | ((base_encoding >> 3) as u8)); // REX.W
                            code.push(0x8B); // MOV r64, r/m64 opcode
                        }
                        _ => {
                            return Err(VmError::Core(vm_core::CoreError::Internal {
                                message: format!("Unsupported load size: {}", size),
                                module: "DefaultCodeGenerator".to_string(),
                            }));
                        }
                    }
                    
                    // ModR/M byte
                    let modrm = if *offset == 0 {
                        (0 << 6) | ((dst_encoding & 7) << 3) | (base_encoding & 7)
                    } else if *offset >= -128 && *offset <= 127 {
                        (1 << 6) | ((dst_encoding & 7) << 3) | (base_encoding & 7)
                    } else {
                        (2 << 6) | ((dst_encoding & 7) << 3) | (base_encoding & 7)
                    };
                    code.push(modrm as u8);
                    
                    // 如果需要，添加偏移量
                    if *offset != 0 {
                        if *offset >= -128 && *offset <= 127 {
                            code.push(*offset as u8);
                        } else {
                            code.extend_from_slice(&(*offset as i32).to_le_bytes());
                        }
                    }
                } else {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: format!("Unknown registers: {}, {}", dst_reg, base_reg),
                        module: "DefaultCodeGenerator".to_string(),
                    }));
                }
                
                Ok(code)
            }
            IROp::Store { src, base, offset, size, .. } => {
                // 生成 MOV [base + offset], src 指令
                let mut code = Vec::new();
                
                // 获取源寄存器和基址寄存器的编码
                let src_reg = op.register_allocation.get(&format!("v{}", src))
                    .cloned().unwrap_or_else(|| format!("R{}", src));
                let base_reg = op.register_allocation.get(&format!("v{}", base))
                    .cloned().unwrap_or_else(|| format!("R{}", base));
                
                if let (Some(&src_encoding), Some(&base_encoding)) = (
                    self.register_encoding.get(&src_reg),
                    self.register_encoding.get(&base_reg)
                ) {
                    // 根据存储大小选择指令
                    match size {
                        1 => {
                            // MOV r/m8, r8
                            code.push(((src_encoding >> 3) as u8) << 2 | ((base_encoding >> 3) as u8)); // REX
                            code.push(0x88); // MOV r/m8, r8 opcode
                        }
                        2 => {
                            // MOV r/m16, r16
                            code.push(0x48 | ((src_encoding >> 3) as u8) << 2 | ((base_encoding >> 3) as u8)); // REX.W
                            code.push(0x89); // MOV r/m64, r64 opcode
                        }
                        4 => {
                            // MOV r/m32, r32
                            code.push(0x48 | ((src_encoding >> 3) as u8) << 2 | ((base_encoding >> 3) as u8)); // REX.W
                            code.push(0x89); // MOV r/m64, r64 opcode
                        }
                        8 => {
                            // MOV r/m64, r64
                            code.push(0x48 | ((src_encoding >> 3) as u8) << 2 | ((base_encoding >> 3) as u8)); // REX.W
                            code.push(0x89); // MOV r/m64, r64 opcode
                        }
                        _ => {
                            return Err(VmError::Core(vm_core::CoreError::Internal {
                                message: format!("Unsupported store size: {}", size),
                                module: "DefaultCodeGenerator".to_string(),
                            }));
                        }
                    }
                    
                    // ModR/M byte
                    let modrm = if *offset == 0 {
                        (0 << 6) | ((src_encoding & 7) << 3) | (base_encoding & 7)
                    } else if *offset >= -128 && *offset <= 127 {
                        (1 << 6) | ((src_encoding & 7) << 3) | (base_encoding & 7)
                    } else {
                        (2 << 6) | ((src_encoding & 7) << 3) | (base_encoding & 7)
                    };
                    code.push(modrm as u8);
                    
                    // 如果需要，添加偏移量
                    if *offset != 0 {
                        if *offset >= -128 && *offset <= 127 {
                            code.push(*offset as u8);
                        } else {
                            code.extend_from_slice(&(*offset as i32).to_le_bytes());
                        }
                    }
                } else {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: format!("Unknown registers: {}, {}", src_reg, base_reg),
                        module: "DefaultCodeGenerator".to_string(),
                    }));
                }
                
                Ok(code)
            }
            // 其他操作类型的处理...
            _ => {
                // 默认返回NOP指令
                Ok(vec![0x90]) // NOP
            }
        }
    }
}

impl CodeGenerator for DefaultCodeGenerator {
    fn generate(&mut self, block: &crate::compiler::CompiledIRBlock) -> Result<crate::core::JITCompilationResult, VmError> {
        let start_time = std::time::Instant::now();
        
        // 根据目标架构生成机器码
        let code = match self.target_arch {
            TargetArch::X86_64 => self.generate_x86_64(block)?,
            TargetArch::AArch64 => {
                return Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "AArch64 code generation not yet implemented".to_string(),
                    module: "DefaultCodeGenerator".to_string(),
                }));
            }
            TargetArch::RiscV64 => {
                return Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "RISC-V 64 code generation not yet implemented".to_string(),
                    module: "DefaultCodeGenerator".to_string(),
                }));
            }
        };
        
        let elapsed = start_time.elapsed().as_nanos() as u64;
        
        // 更新统计信息
        self.stats.original_insn_count = block.ops.len();
        self.stats.machine_insn_count = block.ops.len(); // 简化假设
        self.stats.machine_code_bytes = code.len();
        self.stats.generation_time_ns = elapsed;
        self.stats.code_density = if self.stats.machine_insn_count > 0 {
            self.stats.machine_code_bytes as f64 / self.stats.machine_insn_count as f64
        } else {
            0.0
        };
        
        Ok(crate::core::JITCompilationResult {
            code,
            entry_point: block.start_pc,
            code_size: self.stats.machine_code_bytes,
            stats: crate::core::JITCompilationStats {
                original_insn_count: self.stats.original_insn_count,
                optimized_insn_count: block.ops.len(),
                machine_insn_count: self.stats.machine_insn_count,
                compilation_time_ns: elapsed,
                optimization_time_ns: 0, // 这个在优化阶段统计
                register_allocation_time_ns: 0, // 这个在寄存器分配阶段统计
                instruction_scheduling_time_ns: 0, // 这个在指令调度阶段统计
                code_generation_time_ns: elapsed,
            },
        })
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn version(&self) -> &str {
        &self.version
    }
    
    fn set_option(&mut self, option: &str, value: &str) -> Result<(), VmError> {
        match option {
            "target_arch" => {
                self.target_arch = match value {
                    "x86_64" => TargetArch::X86_64,
                    "aarch64" => TargetArch::AArch64,
                    "riscv64" => TargetArch::RiscV64,
                    _ => {
                        return Err(VmError::Core(vm_core::CoreError::Config {
                            message: format!("Unsupported target architecture: {}", value),
                            path: None,
                        }));
                    }
                };
            }
            "code_gen_mode" => {
                self.code_gen_mode = match value {
                    "compact" => CodeGenMode::Compact,
                    "fast" => CodeGenMode::Fast,
                    "balanced" => CodeGenMode::Balanced,
                    _ => {
                        return Err(VmError::Core(vm_core::CoreError::Config {
                            message: format!("Unsupported code generation mode: {}", value),
                            path: None,
                        }));
                    }
                };
            }
            _ => {}
        }
        
        self.options.insert(option.to_string(), value.to_string());
        Ok(())
    }
    
    fn get_option(&self, option: &str) -> Option<String> {
        self.options.get(option).cloned()
    }
    
    fn reset(&mut self) {
        self.stats = CodeGenerationStats::default();
    }
    
    fn get_stats(&self) -> CodeGenerationStats {
        self.stats.clone()
    }
}