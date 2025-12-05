//! AOT 构建工具
//!
//! 一个功能完整的离线工具，用于将热点 IR 块编译成 AOT 镜像文件。
//! 支持多种优化级别、代码生成模式，以及完整的编译管道。
//!
//! ## 主要功能
//!
//! - **多架构支持**: x86-64, ARM64, RISC-V64
//! - **多种代码生成模式**: 直接代码生成、LLVM 代码生成
//! - **优化管道**: 集成 PassManager，支持 O0/O1/O2 优化级别
//! - **依赖关系分析**: 自动分析代码块之间的依赖关系
//! - **并行编译**: 支持并行编译多个代码块
//! - **镜像验证**: 构建时自动验证镜像完整性
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use aot_builder::{AotBuilder, CompilationOptions, CodegenMode};
//! use vm_ir_lift::ISA;
//!
//! // 创建构建器
//! let mut builder = AotBuilder::new();
//!
//! // 添加预编译的代码块
//! builder.add_compiled_block(0x1000, vec![0x90, 0xC3], 1)?;
//!
//! // 或者从原始机器码编译
//! builder.add_raw_code_block(0x2000, &[0x48, 0x89, 0xC3], 1)?;
//!
//! // 构建并保存
//! let image = builder.build()?;
//! // image.serialize(&mut file)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod config;
mod optimizer;
mod codegen_direct;
mod codegen_llvm;
mod incremental;
mod dependency_analyzer;
mod ir_processor;

pub use config::{CompilationOptions, CompilationStats, CodegenMode};
pub use optimizer::apply_optimization_passes;
pub use incremental::{IncrementalAotBuilder, IncrementalConfig, BlockChange, BlockChangeType, ChangeStats};
pub use dependency_analyzer::DependencyAnalyzer;
pub use ir_processor::IrProcessor;

// PGO支持
pub mod pgo_integration;
pub use pgo_integration::PgoAotBuilder;

use std::collections::{HashMap, HashSet};
use std::path::Path;

// 直接导入类型
use vm_engine_jit::aot_format::{
    AotImage, CodeBlockEntry, DependencyEntry, DependencyType, RelationType, RelocationEntry,
    SymbolEntry, SymbolType,
};
use vm_ir::{IROp, IRBlock, Terminator};
use vm_ir_lift::optimizer::{OptimizationLevel, PassManager};
use vm_ir_lift::{ISA, LiftingContext, create_decoder, create_semantics};

/// AOT 构建器 - 集成 vm-ir-lift 编译管道
pub struct AotBuilder {
    /// 正在构建的 AOT 镜像
    image: AotImage,
    /// 已处理的块集合
    processed_blocks: HashMap<u64, usize>,
    /// 编译代码的累计大小
    total_code_size: usize,
    /// 编译选项
    options: CompilationOptions,
    /// 编译统计
    stats: CompilationStats,
    /// 代码去重：代码哈希 -> (PC, 代码偏移)
    code_deduplication: HashMap<u64, (u64, u32)>,
    /// 是否启用代码去重
    enable_deduplication: bool,
    /// 是否启用镜像压缩
    enable_compression: bool,
}

impl AotBuilder {
    /// 创建新的 AOT 构建器
    pub fn new() -> Self {
        Self::with_options(CompilationOptions::default())
    }

    /// 使用指定选项创建 AOT 构建器
    pub fn with_options(options: CompilationOptions) -> Self {
        Self {
            image: AotImage::new(),
            processed_blocks: HashMap::new(),
            total_code_size: 0,
            options,
            stats: CompilationStats::default(),
            code_deduplication: HashMap::new(),
            enable_deduplication: true,
            enable_compression: false,
        }
    }

    /// 启用或禁用代码去重
    pub fn set_deduplication(&mut self, enable: bool) {
        self.enable_deduplication = enable;
    }

    /// 启用或禁用镜像压缩
    pub fn set_compression(&mut self, enable: bool) {
        self.enable_compression = enable;
    }

    /// 从原始机器码编译块（通过 vm-ir-lift 管道）
    pub fn add_raw_code_block(
        &mut self,
        pc: u64,
        raw_code: &[u8],
        flags: u32,
    ) -> Result<(), String> {
        if self.processed_blocks.contains_key(&pc) {
            return Err(format!("Block at {:#x} already exists", pc));
        }

        let start_time = std::time::Instant::now();

        // 步骤 1: 解码
        let decoder = create_decoder(self.options.target_isa);
        let instructions = decoder
            .decode_sequence(raw_code, raw_code.len())
            .map_err(|e| e.to_string())?;

        self.stats.input_instructions += raw_code.len() as u64;
        self.stats.decoded_instructions += instructions.len() as u64;

        // 步骤 2: 语义提升
        let semantics = create_semantics(self.options.target_isa);
        let mut lifting_ctx = LiftingContext::new(self.options.target_isa);
        let mut ir_instructions = Vec::new();

        for instr in &instructions {
            match semantics.lift(instr, &mut lifting_ctx) {
                Ok(ir) => ir_instructions.push(ir),
                Err(e) => {
                    tracing::warn!("Failed to lift instruction: {}", e);
                }
            }
        }
        self.stats.generated_ir_instructions += ir_instructions.len() as u64;

        // 步骤 3: IR 优化
        let optimized_ir = if self.options.optimization_level > 0 {
            let opt_level = optimizer::optimization_level_to_pass_level(self.options.optimization_level);
            let pass_manager = PassManager::new(opt_level);
            
            // 执行优化 Pass
            apply_optimization_passes(&ir_instructions, &pass_manager)
        } else {
            ir_instructions
        };

        self.stats.optimized_ir_instructions += optimized_ir.len() as u64;

        // 步骤 4: 代码生成
        let compiled_code = match self.options.codegen_mode {
            CodegenMode::Direct => {
                // 直接从优化后的 IR 生成机器码
                codegen_direct::generate_direct_code(&optimized_ir, self.options.target_isa)
            }
            CodegenMode::LLVM => {
                // 通过 LLVM IR 生成
                codegen_llvm::generate_llvm_code(&optimized_ir, self.processed_blocks.len())
            }
        };

        let compile_time_us = elapsed.as_micros() as u64;
        self.stats.compilation_time_ms += elapsed.as_millis() as u64;
        self.stats.output_code_size += compiled_code.len() as u64;

        // 代码去重：检查是否有重复的代码块
        if self.enable_deduplication {
            let code_hash = self.calculate_code_hash(&compiled_code);
            if let Some(&(existing_pc, _existing_offset)) = self.code_deduplication.get(&code_hash) {
                // 发现重复代码块，共享现有代码
                tracing::info!(
                    "Deduplicated block at {:#x}: sharing code from {:#x}",
                    pc, existing_pc
                );
                self.stats.deduplicated_blocks += 1;
                self.stats.deduplicated_size += compiled_code.len() as u64;
                // 仍然需要添加代码块条目，但指向共享的代码
                if let Some(existing_entry) = self.image.code_blocks.iter().find(|b| b.guest_pc == existing_pc) {
                    let shared_offset = existing_entry.code_offset;
                    let code_size = compiled_code.len() as u32;
                    // 添加代码块条目，但使用共享的代码偏移
                    self.image.code_blocks.push(
                        vm_engine_jit::aot_format::CodeBlockEntry::new(pc, shared_offset, code_size, flags)
                    );
                    self.processed_blocks.insert(pc, compiled_code.len());
                } else {
                    // 如果找不到现有条目，正常添加
                    let offset = self.image.code_section.len() as u32;
                    self.image.add_code_block(pc, &compiled_code, flags);
                    self.code_deduplication.insert(code_hash, (pc, offset));
                    self.processed_blocks.insert(pc, compiled_code.len());
                    self.total_code_size += compiled_code.len();
                }
            } else {
                // 新代码块，添加到镜像
                let offset = self.image.code_section.len() as u32;
                self.image.add_code_block(pc, &compiled_code, flags);
                self.code_deduplication.insert(code_hash, (pc, offset));
                self.processed_blocks.insert(pc, compiled_code.len());
                self.total_code_size += compiled_code.len();
            }
        } else {
            // 未启用去重，直接添加
            self.image.add_code_block(pc, &compiled_code, flags);
            self.processed_blocks.insert(pc, compiled_code.len());
            self.total_code_size += compiled_code.len();
        }

        // 添加符号信息
        let symbol_name = format!("block_{:x}", pc);
        self.image.add_symbol(
            symbol_name,
            self.image.code_section.len() as u64 - compiled_code.len() as u64,
            compiled_code.len() as u32,
            SymbolType::BlockLabel,
        );

        tracing::info!(
            "Compiled block at {:#x}: {} → {} bytes, {} ms",
            pc,
            raw_code.len(),
            compiled_code.len(),
            elapsed.as_millis()
        );

        Ok(())
    }


    /// 添加 IR 块到 AOT 镜像（支持缓存）
    pub fn add_ir_block(
        &mut self,
        pc: u64,
        block: &IRBlock,
        optimization_level: u32,
    ) -> Result<(), String> {
        self.add_ir_block_with_cache(pc, block, optimization_level, None)
    }

    /// 添加 IR 块到 AOT 镜像（支持缓存）
    pub fn add_ir_block_with_cache(
        &mut self,
        pc: u64,
        block: &IRBlock,
        optimization_level: u32,
        cache: Option<&vm_engine_jit::AotCache>,
    ) -> Result<(), String> {
        if self.processed_blocks.contains_key(&pc) {
            return Err(format!("Block at {:#x} already processed", pc));
        }

        let start_time = std::time::Instant::now();

        // 检查缓存（如果提供）
        let target_arch = format!("{:?}", self.options.target_isa);
        if let Some(ref aot_cache) = cache {
            if let Some(cached_code) = aot_cache.lookup(block, optimization_level, &target_arch) {
                // 使用缓存的编译结果
                let elapsed = start_time.elapsed();
                let compile_time_us = elapsed.as_micros() as u64;

                // 添加到 AOT 镜像
                let code_offset = self.image.code_section.len() as u32;
                let code_size = cached_code.len() as u32;
                self.image.add_code_block(pc, &cached_code, optimization_level);

                // 分析依赖关系（仍然需要）
                let dependencies = DependencyAnalyzer::analyze_block_dependencies(block);

                // 更新代码块条目
                if let Some(block_entry) = self.image.code_blocks.last_mut() {
                    for dep_pc in &dependencies {
                        block_entry.add_dependency(*dep_pc);
                    }
                    block_entry.optimization_level = optimization_level;
                    block_entry.compile_timestamp_us = compile_time_us;
                }

                // 添加依赖关系条目
                if !dependencies.is_empty() {
                    self.image.add_dependency(
                        pc,
                        dependencies.clone(),
                        DependencyType::DirectJump,
                    );
                }

                self.processed_blocks.insert(pc, cached_code.len());
                self.total_code_size += cached_code.len();

                // 添加符号信息
                let symbol_name = format!("block_{:x}", pc);
                self.image.add_symbol(
                    symbol_name,
                    code_offset as u64,
                    code_size,
                    SymbolType::BlockLabel,
                );

                tracing::info!(
                    "Using cached compilation for block at {:#x}: {} bytes (saved {} ms)",
                    pc,
                    cached_code.len(),
                    elapsed.as_millis()
                );

                return Ok(());
            }
        }

        // 缓存未命中，执行编译
        // 分析依赖关系
        let dependencies = DependencyAnalyzer::analyze_block_dependencies(block);

        // 编译 IR 块（使用模块化的代码生成器）
        let (mut compiled_code, relocations) = match self.options.codegen_mode {
            CodegenMode::Direct => {
                // 使用codegen_direct模块的编译函数
                let code = codegen_direct::compile_ir_block_direct(
                    block,
                    self.options.target_isa,
                    optimization_level,
                )?;
                (code, Vec::new()) // 简化：暂不支持重定位
            }
            CodegenMode::LLVM => {
                let code = codegen_llvm::compile_ir_block_llvm(block, optimization_level)?;
                (code, Vec::new()) // LLVM 模式暂不支持重定位
            }
        };

        let elapsed = start_time.elapsed();
        let compile_time_us = elapsed.as_micros() as u64;

        // 保存到缓存（如果提供）
        if let Some(ref aot_cache) = cache {
            aot_cache.insert(
                block,
                compiled_code.clone(),
                optimization_level,
                &target_arch,
                compile_time_us,
            );
        }
        self.stats.compilation_time_ms += elapsed.as_millis() as u64;
        self.stats.output_code_size += compiled_code.len() as u64;
        self.stats.generated_ir_instructions += block.ops.len() as u64;
        self.stats.optimized_ir_instructions += block.ops.len() as u64;

        // 添加到 AOT 镜像
        let code_offset = self.image.code_section.len() as u32;
        let code_size = compiled_code.len() as u32;
        self.image.add_code_block(pc, &compiled_code, optimization_level);

        // 添加重定位条目
        for (offset, target) in relocations {
            let reloc_offset = code_offset + offset;
            self.image.add_relocation(reloc_offset, RelationType::BlockJump, target);
        }

        // 更新代码块条目，添加依赖关系
        if let Some(block_entry) = self.image.code_blocks.last_mut() {
            for dep_pc in &dependencies {
                block_entry.add_dependency(*dep_pc);
            }
            block_entry.optimization_level = optimization_level;
            block_entry.compile_timestamp_us = compile_time_us;
        }

        // 添加依赖关系条目
        if !dependencies.is_empty() {
            self.image.add_dependency(
                pc,
                dependencies.clone(),
                DependencyType::DirectJump, // 简化：假设都是直接跳转
            );
        }

        self.processed_blocks.insert(pc, compiled_code.len());
        self.total_code_size += compiled_code.len();

        // 添加符号信息
        let symbol_name = format!("block_{:x}", pc);
        self.image.add_symbol(
            symbol_name,
            code_offset as u64,
            code_size,
            SymbolType::BlockLabel,
        );

        tracing::info!(
            "Compiled IR block at {:#x}: {} ops → {} bytes, {} deps, {} ms",
            pc,
            block.ops.len(),
            compiled_code.len(),
            dependencies.len(),
            elapsed.as_millis()
        );

        Ok(())
    }


    /// 直接编译 IR 块（不通过 LLVM），返回代码和重定位信息
    fn compile_ir_block_direct_with_relocs(
        &self,
        block: &IRBlock,
        optimization_level: u32,
        block_pc: u64,
    ) -> Result<(Vec<u8>, Vec<(u32, u64)>), String> {
        let mut code = Vec::new();
        let mut relocations = Vec::new();

        // 编译每个 IR 操作
        for op in &block.ops {
            let op_code = self.compile_ir_op_direct(op)?;
            code.extend_from_slice(&op_code);
        }

        // 编译终结符（带重定位信息）
        let (term_code, term_relocs) = self.compile_terminator_direct_with_relocs(&block.term, block_pc, code.len() as u32)?;
        code.extend_from_slice(&term_code);
        
        // 合并重定位信息（调整偏移量）
        for (offset, target) in term_relocs {
            relocations.push((offset, target));
        }

        Ok((code, relocations))
    }

    /// 直接编译 IR 块（不通过 LLVM），兼容旧接口
    fn compile_ir_block_direct(
        &self,
        block: &IRBlock,
        optimization_level: u32,
    ) -> Result<Vec<u8>, String> {
        let (code, _) = self.compile_ir_block_direct_with_relocs(block, optimization_level, block.start_pc)?;
        Ok(code)
    }

    /// 编译单个 IR 操作（直接模式）
    fn compile_ir_op_direct(&self, op: &IROp) -> Result<Vec<u8>, String> {
        match self.options.target_isa {
            ISA::X86_64 => self.compile_ir_op_x86_64(op),
            ISA::ARM64 => self.compile_ir_op_arm64(op),
            ISA::RISCV64 => self.compile_ir_op_riscv64(op),
        }
    }

    /// 编码 x86-64 寄存器到 ModR/M 字节
    fn encode_x86_64_reg(&self, reg: RegId) -> u8 {
        codegen_direct::encode_x86_64_reg(reg)
    }

    /// 编译 IR 操作到 x86-64
    fn compile_ir_op_x86_64(&self, op: &IROp) -> Result<Vec<u8>, String> {
        let mut code = Vec::new();
        match op {
            // 算术运算
            IROp::Add { dst, src1, src2 } => {
                // mov dst, src1
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src1_reg = self.encode_x86_64_reg(*src1);
                code.push(0x48); // REX.W
                code.push(0x89); // mov
                code.push(0xC0 | (src1_reg << 3) | dst_reg);
                // add dst, src2
                let src2_reg = self.encode_x86_64_reg(*src2);
                code.push(0x48); // REX.W
                code.push(0x01); // add
                code.push(0xC0 | (src2_reg << 3) | dst_reg);
            }
            IROp::Sub { dst, src1, src2 } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src1_reg = self.encode_x86_64_reg(*src1);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src1_reg << 3) | dst_reg);
                let src2_reg = self.encode_x86_64_reg(*src2);
                code.push(0x48);
                code.push(0x29); // sub
                code.push(0xC0 | (src2_reg << 3) | dst_reg);
            }
            IROp::Mul { dst, src1, src2 } => {
                // mov dst, src1; mul src2
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src1_reg = self.encode_x86_64_reg(*src1);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src1_reg << 3) | dst_reg);
                let src2_reg = self.encode_x86_64_reg(*src2);
                code.push(0x48);
                code.push(0xF7); // mul
                code.push(0xE0 | src2_reg);
            }
            IROp::Div { dst, src1, src2, signed: true } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src1_reg = self.encode_x86_64_reg(*src1);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src1_reg << 3) | dst_reg);
                let src2_reg = self.encode_x86_64_reg(*src2);
                code.push(0x48);
                code.push(0xF7); // idiv (signed)
                code.push(0xF0 | src2_reg);
            }
            IROp::Div { dst, src1, src2, signed: false } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src1_reg = self.encode_x86_64_reg(*src1);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src1_reg << 3) | dst_reg);
                let src2_reg = self.encode_x86_64_reg(*src2);
                code.push(0x48);
                code.push(0xF7); // div (unsigned)
                code.push(0xF0 | src2_reg);
            }
            // 逻辑运算
            IROp::And { dst, src1, src2 } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src1_reg = self.encode_x86_64_reg(*src1);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src1_reg << 3) | dst_reg);
                let src2_reg = self.encode_x86_64_reg(*src2);
                code.push(0x48);
                code.push(0x21); // and
                code.push(0xC0 | (src2_reg << 3) | dst_reg);
            }
            IROp::Or { dst, src1, src2 } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src1_reg = self.encode_x86_64_reg(*src1);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src1_reg << 3) | dst_reg);
                let src2_reg = self.encode_x86_64_reg(*src2);
                code.push(0x48);
                code.push(0x09); // or
                code.push(0xC0 | (src2_reg << 3) | dst_reg);
            }
            IROp::Xor { dst, src1, src2 } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src1_reg = self.encode_x86_64_reg(*src1);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src1_reg << 3) | dst_reg);
                let src2_reg = self.encode_x86_64_reg(*src2);
                code.push(0x48);
                code.push(0x31); // xor
                code.push(0xC0 | (src2_reg << 3) | dst_reg);
            }
            IROp::Not { dst, src } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src_reg = self.encode_x86_64_reg(*src);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src_reg << 3) | dst_reg);
                code.push(0x48);
                code.push(0xF7); // not
                code.push(0xD0 | dst_reg);
            }
            // 移位运算
            IROp::Sll { dst, src, shreg } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src_reg = self.encode_x86_64_reg(*src);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src_reg << 3) | dst_reg);
                let shreg_reg = self.encode_x86_64_reg(*shreg);
                code.push(0x48);
                code.push(0xD3); // shl (shift left)
                code.push(0xE0 | dst_reg);
            }
            IROp::Srl { dst, src, shreg } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src_reg = self.encode_x86_64_reg(*src);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src_reg << 3) | dst_reg);
                code.push(0x48);
                code.push(0xD3); // shr (shift right logical)
                code.push(0xE8 | dst_reg);
            }
            IROp::Sra { dst, src, shreg } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src_reg = self.encode_x86_64_reg(*src);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src_reg << 3) | dst_reg);
                code.push(0x48);
                code.push(0xD3); // sar (shift arithmetic right)
                code.push(0xF8 | dst_reg);
            }
            // 立即数操作
            IROp::AddImm { dst, src, imm } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src_reg = self.encode_x86_64_reg(*src);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src_reg << 3) | dst_reg);
                if *imm >= -128 && *imm <= 127 {
                    // 使用短立即数形式
                    code.push(0x48);
                    code.push(0x83); // add imm8
                    code.push(0xC0 | dst_reg);
                    code.push(*imm as u8);
                } else {
                    code.push(0x48);
                    code.push(0x81); // add imm32
                    code.push(0xC0 | dst_reg);
                    code.extend_from_slice(&(*imm as i32).to_le_bytes());
                }
            }
            IROp::MovImm { dst, imm } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                code.push(0x48 | ((dst_reg >> 3) & 1) as u8); // REX.W + REX.B
                code.push(0xB8 | (dst_reg & 0x7)); // mov reg, imm64
                code.extend_from_slice(&imm.to_le_bytes());
            }
            IROp::SllImm { dst, src, sh } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src_reg = self.encode_x86_64_reg(*src);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src_reg << 3) | dst_reg);
                code.push(0x48);
                code.push(0xC1); // shl imm8
                code.push(0xE0 | dst_reg);
                code.push(*sh);
            }
            IROp::SrlImm { dst, src, sh } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src_reg = self.encode_x86_64_reg(*src);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src_reg << 3) | dst_reg);
                code.push(0x48);
                code.push(0xC1); // shr imm8
                code.push(0xE8 | dst_reg);
                code.push(*sh);
            }
            IROp::SraImm { dst, src, sh } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let src_reg = self.encode_x86_64_reg(*src);
                code.push(0x48);
                code.push(0x89); // mov
                code.push(0xC0 | (src_reg << 3) | dst_reg);
                code.push(0x48);
                code.push(0xC1); // sar imm8
                code.push(0xF8 | dst_reg);
                code.push(*sh);
            }
            // 比较操作
            IROp::CmpEq { dst, lhs, rhs } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let lhs_reg = self.encode_x86_64_reg(*lhs);
                let rhs_reg = self.encode_x86_64_reg(*rhs);
                code.push(0x48);
                code.push(0x39); // cmp
                code.push(0xC0 | (rhs_reg << 3) | lhs_reg);
                code.push(0x48);
                code.push(0x0F); // setz
                code.push(0x94); // sete
                code.push(0xC0 | dst_reg);
            }
            IROp::CmpLt { dst, lhs, rhs } => {
                let lhs_reg = self.encode_x86_64_reg(*lhs);
                let rhs_reg = self.encode_x86_64_reg(*rhs);
                code.push(0x48);
                code.push(0x39); // cmp
                code.push(0xC0 | (rhs_reg << 3) | lhs_reg);
                let dst_reg = self.encode_x86_64_reg(*dst);
                code.push(0x0F);
                code.push(0x9C); // setl
                code.push(0xC0 | dst_reg);
            }
            // 内存操作
            IROp::Load { dst, base, offset, size, .. } => {
                let dst_reg = self.encode_x86_64_reg(*dst);
                let base_reg = self.encode_x86_64_reg(*base);
                match size {
                    1 => code.push(0x0F), // movzx
                    2 => code.push(0x0F),
                    4 => code.push(0x8B), // mov
                    8 => {
                        code.push(0x48);
                        code.push(0x8B); // mov
                    }
                    _ => return Err(format!("Unsupported load size: {}", size)),
                }
                // 简化：假设 offset 在 32 位范围内
                if *offset == 0 {
                    code.push(0x00 | (base_reg << 3) | dst_reg);
                } else if *offset >= -128 && *offset <= 127 {
                    code.push(0x40 | (base_reg << 3) | dst_reg);
                    code.push(*offset as u8);
                } else {
                    code.push(0x80 | (base_reg << 3) | dst_reg);
                    code.extend_from_slice(&(*offset as i32).to_le_bytes());
                }
            }
            IROp::Store { src, base, offset, size, .. } => {
                let src_reg = self.encode_x86_64_reg(*src);
                let base_reg = self.encode_x86_64_reg(*base);
                match size {
                    1 => code.push(0x88), // mov byte
                    2 => {
                        code.push(0x66);
                        code.push(0x89);
                    } // mov word
                    4 => code.push(0x89), // mov dword
                    8 => {
                        code.push(0x48);
                        code.push(0x89); // mov qword
                    }
                    _ => return Err(format!("Unsupported store size: {}", size)),
                }
                if *offset == 0 {
                    code.push(0x00 | (src_reg << 3) | base_reg);
                } else if *offset >= -128 && *offset <= 127 {
                    code.push(0x40 | (src_reg << 3) | base_reg);
                    code.push(*offset as u8);
                } else {
                    code.push(0x80 | (src_reg << 3) | base_reg);
                    code.extend_from_slice(&(*offset as i32).to_le_bytes());
                }
            }
            IROp::Nop => {
                code.push(0x90); // nop
            }
            _ => {
                // 其他操作：生成占位符并记录警告
                tracing::warn!("Unsupported IR operation: {:?}, generating placeholder", op);
                code.push(0x90); // nop
            }
        }
        Ok(code)
    }

    /// 编码 ARM64 寄存器
    fn encode_arm64_reg(&self, reg: RegId) -> u32 {
        codegen_direct::encode_arm64_reg(reg)
    }

    /// 生成 ARM64 指令编码辅助函数
    fn arm64_add_reg(&self, dst: RegId, src1: RegId, src2: RegId) -> Vec<u8> {
        let dst = self.encode_arm64_reg(dst);
        let src1 = self.encode_arm64_reg(src1);
        let src2 = self.encode_arm64_reg(src2);
        let inst = 0x8B000000u32 | (dst << 0) | (src1 << 5) | (src2 << 16);
        inst.to_le_bytes().to_vec()
    }

    /// 编译 IR 操作到 ARM64
    fn compile_ir_op_arm64(&self, op: &IROp) -> Result<Vec<u8>, String> {
        let mut code = Vec::new();
        match op {
            IROp::Add { dst, src1, src2 } => {
                code.extend_from_slice(&self.arm64_add_reg(*dst, *src1, *src2));
            }
            IROp::Sub { dst, src1, src2 } => {
                let dst = self.encode_arm64_reg(*dst);
                let src1 = self.encode_arm64_reg(*src1);
                let src2 = self.encode_arm64_reg(*src2);
                let inst = 0xCB000000u32 | (dst << 0) | (src1 << 5) | (src2 << 16);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::Mul { dst, src1, src2 } => {
                let dst = self.encode_arm64_reg(*dst);
                let src1 = self.encode_arm64_reg(*src1);
                let src2 = self.encode_arm64_reg(*src2);
                let inst = 0x9B007C00u32 | (dst << 0) | (src1 << 5) | (src2 << 16);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::And { dst, src1, src2 } => {
                let dst = self.encode_arm64_reg(*dst);
                let src1 = self.encode_arm64_reg(*src1);
                let src2 = self.encode_arm64_reg(*src2);
                let inst = 0x8A000000u32 | (dst << 0) | (src1 << 5) | (src2 << 16);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::Or { dst, src1, src2 } => {
                let dst = self.encode_arm64_reg(*dst);
                let src1 = self.encode_arm64_reg(*src1);
                let src2 = self.encode_arm64_reg(*src2);
                let inst = 0xAA000000u32 | (dst << 0) | (src1 << 5) | (src2 << 16);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::Xor { dst, src1, src2 } => {
                let dst = self.encode_arm64_reg(*dst);
                let src1 = self.encode_arm64_reg(*src1);
                let src2 = self.encode_arm64_reg(*src2);
                let inst = 0xCA000000u32 | (dst << 0) | (src1 << 5) | (src2 << 16);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::MovImm { dst, imm } => {
                let dst = self.encode_arm64_reg(*dst);
                // 尝试使用 movz（16位立即数）
                if *imm <= 0xFFFF {
                    let imm16 = (*imm & 0xFFFF) as u16;
                    let inst = 0xD2800000u32 | (dst << 0) | ((imm16 as u32) << 5);
                    code.extend_from_slice(&inst.to_le_bytes());
                } else {
                    // 对于更大的立即数，需要多条指令（movz + movk）
                    // 简化：只处理低16位
                    let imm16 = (*imm & 0xFFFF) as u16;
                    let inst = 0xD2800000u32 | (dst << 0) | ((imm16 as u32) << 5);
                    code.extend_from_slice(&inst.to_le_bytes());
                }
            }
            IROp::AddImm { dst, src, imm } => {
                let dst = self.encode_arm64_reg(*dst);
                let src = self.encode_arm64_reg(*src);
                if *imm >= 0 && *imm <= 0xFFF {
                    let imm12 = (*imm & 0xFFF) as u32;
                    let inst = 0x91000000u32 | (dst << 0) | (src << 5) | (imm12 << 10);
                    code.extend_from_slice(&inst.to_le_bytes());
                } else {
                    // 需要多条指令处理大立即数
                    return Err(format!("Large immediate {} not yet supported", imm));
                }
            }
            IROp::SllImm { dst, src, sh } => {
                let dst = self.encode_arm64_reg(*dst);
                let src = self.encode_arm64_reg(*src);
                let shamt = (*sh & 0x3F) as u32;
                let inst = 0xD3400000u32 | (dst << 0) | (src << 5) | (shamt << 16);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::SrlImm { dst, src, sh } => {
                let dst = self.encode_arm64_reg(*dst);
                let src = self.encode_arm64_reg(*src);
                let shamt = (*sh & 0x3F) as u32;
                let inst = 0xD3500000u32 | (dst << 0) | (src << 5) | (shamt << 16);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::Load { dst, base, offset, size, .. } => {
                let dst = self.encode_arm64_reg(*dst);
                let base = self.encode_arm64_reg(*base);
                if *offset >= 0 && *offset <= 0xFFF {
                    let imm12 = (*offset & 0xFFF) as u32;
                    let size_bits = match size {
                        1 => 0b00,
                        2 => 0b01,
                        4 => 0b10,
                        8 => 0b11,
                        _ => return Err(format!("Unsupported load size: {}", size)),
                    };
                    let inst = 0xF9400000u32 | (dst << 0) | (base << 5) | (imm12 << 10) | (size_bits << 30);
                    code.extend_from_slice(&inst.to_le_bytes());
                } else {
                    return Err(format!("Large offset {} not yet supported", offset));
                }
            }
            IROp::Store { src, base, offset, size, .. } => {
                let src = self.encode_arm64_reg(*src);
                let base = self.encode_arm64_reg(*base);
                if *offset >= 0 && *offset <= 0xFFF {
                    let imm12 = (*offset & 0xFFF) as u32;
                    let size_bits = match size {
                        1 => 0b00,
                        2 => 0b01,
                        4 => 0b10,
                        8 => 0b11,
                        _ => return Err(format!("Unsupported store size: {}", size)),
                    };
                    let inst = 0xF9000000u32 | (src << 0) | (base << 5) | (imm12 << 10) | (size_bits << 30);
                    code.extend_from_slice(&inst.to_le_bytes());
                } else {
                    return Err(format!("Large offset {} not yet supported", offset));
                }
            }
            IROp::Nop => {
                code.extend_from_slice(&[0x1F, 0x20, 0x03, 0xD5]); // nop
            }
            _ => {
                tracing::warn!("Unsupported IR operation for ARM64: {:?}, generating placeholder", op);
                code.extend_from_slice(&[0x1F, 0x20, 0x03, 0xD5]); // nop
            }
        }
        Ok(code)
    }

    /// 编码 RISC-V64 寄存器
    fn encode_riscv64_reg(&self, reg: RegId) -> u32 {
        codegen_direct::encode_riscv64_reg(reg)
    }

    /// 编译 IR 操作到 RISC-V64
    fn compile_ir_op_riscv64(&self, op: &IROp) -> Result<Vec<u8>, String> {
        let mut code = Vec::new();
        match op {
            IROp::Add { dst, src1, src2 } => {
                let dst = self.encode_riscv64_reg(*dst);
                let src1 = self.encode_riscv64_reg(*src1);
                let src2 = self.encode_riscv64_reg(*src2);
                // add rd, rs1, rs2: 0x00000033 | (rd << 7) | (rs1 << 15) | (rs2 << 20)
                let inst = 0x00000033u32 | (dst << 7) | (src1 << 15) | (src2 << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::Sub { dst, src1, src2 } => {
                let dst = self.encode_riscv64_reg(*dst);
                let src1 = self.encode_riscv64_reg(*src1);
                let src2 = self.encode_riscv64_reg(*src2);
                // sub rd, rs1, rs2: 0x40000033 | (rd << 7) | (rs1 << 15) | (rs2 << 20)
                let inst = 0x40000033u32 | (dst << 7) | (src1 << 15) | (src2 << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::Mul { dst, src1, src2 } => {
                let dst = self.encode_riscv64_reg(*dst);
                let src1 = self.encode_riscv64_reg(*src1);
                let src2 = self.encode_riscv64_reg(*src2);
                // mul rd, rs1, rs2: 0x02000033 | (rd << 7) | (rs1 << 15) | (rs2 << 20)
                let inst = 0x02000033u32 | (dst << 7) | (src1 << 15) | (src2 << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::And { dst, src1, src2 } => {
                let dst = self.encode_riscv64_reg(*dst);
                let src1 = self.encode_riscv64_reg(*src1);
                let src2 = self.encode_riscv64_reg(*src2);
                // and rd, rs1, rs2: 0x00007033 | (rd << 7) | (rs1 << 15) | (rs2 << 20)
                let inst = 0x00007033u32 | (dst << 7) | (src1 << 15) | (src2 << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::Or { dst, src1, src2 } => {
                let dst = self.encode_riscv64_reg(*dst);
                let src1 = self.encode_riscv64_reg(*src1);
                let src2 = self.encode_riscv64_reg(*src2);
                // or rd, rs1, rs2: 0x00006033 | (rd << 7) | (rs1 << 15) | (rs2 << 20)
                let inst = 0x00006033u32 | (dst << 7) | (src1 << 15) | (src2 << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::Xor { dst, src1, src2 } => {
                let dst = self.encode_riscv64_reg(*dst);
                let src1 = self.encode_riscv64_reg(*src1);
                let src2 = self.encode_riscv64_reg(*src2);
                // xor rd, rs1, rs2: 0x00004033 | (rd << 7) | (rs1 << 15) | (rs2 << 20)
                let inst = 0x00004033u32 | (dst << 7) | (src1 << 15) | (src2 << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::MovImm { dst, imm } => {
                let dst = self.encode_riscv64_reg(*dst);
                // addi rd, x0, imm: 0x00000013 | (rd << 7) | (imm << 20)
                // 注意：imm 是 12 位有符号立即数
                let imm12 = (*imm as i64 & 0xFFF) as u32;
                let sign_ext = if (*imm as i64) < 0 { 0xFFFFF000 } else { 0 };
                let imm = (imm12 | sign_ext) & 0xFFFFFFFF;
                let inst = 0x00000013u32 | (dst << 7) | ((imm & 0xFFF) << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::AddImm { dst, src, imm } => {
                let dst = self.encode_riscv64_reg(*dst);
                let src = self.encode_riscv64_reg(*src);
                // addi rd, rs1, imm
                let imm12 = (*imm & 0xFFF) as u32;
                let sign_ext = if *imm < 0 { 0xFFFFF000 } else { 0 };
                let imm = (imm12 | sign_ext) & 0xFFFFFFFF;
                let inst = 0x00000013u32 | (dst << 7) | (src << 15) | ((imm & 0xFFF) << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::SllImm { dst, src, sh } => {
                let dst = self.encode_riscv64_reg(*dst);
                let src = self.encode_riscv64_reg(*src);
                let shamt = (*sh & 0x3F) as u32;
                // slli rd, rs1, shamt: 0x00001013 | (rd << 7) | (rs1 << 15) | (shamt << 20)
                let inst = 0x00001013u32 | (dst << 7) | (src << 15) | (shamt << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::SrlImm { dst, src, sh } => {
                let dst = self.encode_riscv64_reg(*dst);
                let src = self.encode_riscv64_reg(*src);
                let shamt = (*sh & 0x3F) as u32;
                // srli rd, rs1, shamt: 0x00005013 | (rd << 7) | (rs1 << 15) | (shamt << 20)
                let inst = 0x00005013u32 | (dst << 7) | (src << 15) | (shamt << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::SraImm { dst, src, sh } => {
                let dst = self.encode_riscv64_reg(*dst);
                let src = self.encode_riscv64_reg(*src);
                let shamt = (*sh & 0x3F) as u32;
                // srai rd, rs1, shamt: 0x40005013 | (rd << 7) | (rs1 << 15) | (shamt << 20)
                let inst = 0x40005013u32 | (dst << 7) | (src << 15) | (shamt << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::Load { dst, base, offset, size, .. } => {
                let dst = self.encode_riscv64_reg(*dst);
                let base = self.encode_riscv64_reg(*base);
                let imm12 = (*offset & 0xFFF) as u32;
                let sign_ext = if *offset < 0 { 0xFFFFF000 } else { 0 };
                let imm = (imm12 | sign_ext) & 0xFFFFFFFF;
                let funct3 = match size {
                    1 => 0b000, // LB
                    2 => 0b001, // LH
                    4 => 0b010, // LW
                    8 => 0b011, // LD
                    _ => return Err(format!("Unsupported load size: {}", size)),
                };
                // lw rd, imm(rs1): 0x00002003 | (rd << 7) | (funct3 << 12) | (rs1 << 15) | (imm << 20)
                let inst = 0x00002003u32 | (dst << 7) | (funct3 << 12) | (base << 15) | ((imm & 0xFFF) << 20);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::Store { src, base, offset, size, .. } => {
                let src = self.encode_riscv64_reg(*src);
                let base = self.encode_riscv64_reg(*base);
                let imm12 = (*offset & 0xFFF) as u32;
                let sign_ext = if *offset < 0 { 0xFFFFF000 } else { 0 };
                let imm = (imm12 | sign_ext) & 0xFFFFFFFF;
                let funct3 = match size {
                    1 => 0b000, // SB
                    2 => 0b001, // SH
                    4 => 0b010, // SW
                    8 => 0b011, // SD
                    _ => return Err(format!("Unsupported store size: {}", size)),
                };
                // sw rs2, imm(rs1): 0x00002023 | (imm[4:0] << 7) | (funct3 << 12) | (rs1 << 15) | (rs2 << 20) | (imm[11:5] << 25)
                let imm_low = imm & 0x1F;
                let imm_high = (imm >> 5) & 0x7F;
                let inst = 0x00002023u32 | (imm_low << 7) | (funct3 << 12) | (base << 15) | (src << 20) | (imm_high << 25);
                code.extend_from_slice(&inst.to_le_bytes());
            }
            IROp::Nop => {
                code.extend_from_slice(&[0x13, 0x00, 0x00, 0x00]); // nop (addi x0, x0, 0)
            }
            _ => {
                tracing::warn!("Unsupported IR operation for RISC-V64: {:?}, generating placeholder", op);
                code.extend_from_slice(&[0x13, 0x00, 0x00, 0x00]); // nop
            }
        }
        Ok(code)
    }

    /// 编译终结符（直接模式），返回代码和重定位信息
    fn compile_terminator_direct_with_relocs(
        &self,
        term: &Terminator,
        block_pc: u64,
        code_offset: u32,
    ) -> Result<(Vec<u8>, Vec<(u32, u64)>), String> {
        match self.options.target_isa {
            ISA::X86_64 => self.compile_terminator_x86_64_with_relocs(term, block_pc, code_offset),
            ISA::ARM64 => self.compile_terminator_arm64_with_relocs(term, block_pc, code_offset),
            ISA::RISCV64 => self.compile_terminator_riscv64_with_relocs(term, block_pc, code_offset),
        }
    }

    /// 编译终结符（直接模式），兼容旧接口
    fn compile_terminator_direct(&self, term: &Terminator) -> Result<Vec<u8>, String> {
        let (code, _) = self.compile_terminator_direct_with_relocs(term, 0, 0)?;
        Ok(code)
    }

    /// 编译终结符到 x86-64（带重定位）
    fn compile_terminator_x86_64_with_relocs(
        &self,
        term: &Terminator,
        _block_pc: u64,
        code_offset: u32,
    ) -> Result<(Vec<u8>, Vec<(u32, u64)>), String> {
        let mut code = Vec::new();
        let mut relocations = Vec::new();
        
        match term {
            Terminator::Ret => {
                code.push(0xC3); // ret
            }
            Terminator::Jmp { target } => {
                code.push(0xE9); // jmp rel32
                let offset_pos = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // 占位符
                relocations.push((offset_pos, *target));
            }
            Terminator::CondJmp { cond, target_true, target_false } => {
                let cond_reg = self.encode_x86_64_reg(*cond);
                // test cond, cond
                code.push(0x48); // REX.W
                code.push(0x85); // test
                code.push(0xC0 | (cond_reg << 3) | cond_reg);
                // jnz target_true
                code.push(0x0F);
                code.push(0x85); // jnz rel32
                let jnz_offset = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
                relocations.push((jnz_offset, *target_true));
                // jmp target_false
                code.push(0xE9); // jmp rel32
                let jmp_offset = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
                relocations.push((jmp_offset, *target_false));
            }
            Terminator::Call { target, .. } => {
                code.push(0xE8); // call rel32
                let call_offset = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
                relocations.push((call_offset, *target));
            }
            Terminator::JmpReg { base, offset } => {
                let base_reg = self.encode_x86_64_reg(*base);
                if *offset == 0 {
                    code.push(0xFF);
                    code.push(0xE0 | base_reg); // jmp reg
                } else {
                    code.push(0xFF);
                    if *offset >= -128 && *offset <= 127 {
                        code.push(0x60 | base_reg);
                        code.push(*offset as u8);
                    } else {
                        code.push(0xA0 | base_reg);
                        code.extend_from_slice(&(*offset as i32).to_le_bytes());
                    }
                }
            }
            _ => {
                code.push(0xC3); // 默认返回
            }
        }
        Ok((code, relocations))
    }

    /// 编译终结符到 ARM64（带重定位）
    fn compile_terminator_arm64_with_relocs(
        &self,
        term: &Terminator,
        _block_pc: u64,
        code_offset: u32,
    ) -> Result<(Vec<u8>, Vec<(u32, u64)>), String> {
        let mut code = Vec::new();
        let mut relocations = Vec::new();
        
        match term {
            Terminator::Ret => {
                code.extend_from_slice(&[0xC0, 0x03, 0x5F, 0xD6]); // ret
            }
            Terminator::Jmp { target } => {
                // b target (需要重定位)
                let offset_pos = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x14]); // b #0 (占位符)
                relocations.push((offset_pos, *target));
            }
            Terminator::CondJmp { cond, target_true, target_false } => {
                let cond_reg = self.encode_arm64_reg(*cond);
                // cbnz cond, target_true
                let cbnz_offset = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0xB5]); // cbnz (占位符)
                relocations.push((cbnz_offset, *target_true));
                // b target_false
                let b_offset = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x14]); // b (占位符)
                relocations.push((b_offset, *target_false));
            }
            Terminator::Call { target, .. } => {
                // bl target (需要重定位)
                let call_offset = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x94]); // bl #0 (占位符)
                relocations.push((call_offset, *target));
            }
            _ => {
                code.extend_from_slice(&[0xC0, 0x03, 0x5F, 0xD6]); // 默认返回
            }
        }
        Ok((code, relocations))
    }

    /// 编译终结符到 RISC-V64（带重定位）
    fn compile_terminator_riscv64_with_relocs(
        &self,
        term: &Terminator,
        _block_pc: u64,
        code_offset: u32,
    ) -> Result<(Vec<u8>, Vec<(u32, u64)>), String> {
        let mut code = Vec::new();
        let mut relocations = Vec::new();
        
        match term {
            Terminator::Ret => {
                code.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]); // ret
            }
            Terminator::Jmp { target } => {
                // jal x0, target (需要重定位)
                let offset_pos = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x6F, 0x00, 0x00, 0x00]); // jal x0, 0 (占位符)
                relocations.push((offset_pos, *target));
            }
            Terminator::CondJmp { cond, target_true, target_false } => {
                let cond_reg = self.encode_riscv64_reg(*cond);
                // bne cond, x0, target_true
                let bne_offset = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x63, 0x00, 0x00, 0x00]); // bne (占位符)
                relocations.push((bne_offset, *target_true));
                // jal x0, target_false
                let jal_offset = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x6F, 0x00, 0x00, 0x00]); // jal (占位符)
                relocations.push((jal_offset, *target_false));
            }
            Terminator::Call { target, .. } => {
                // jal x1, target (需要重定位)
                let call_offset = code_offset + code.len() as u32;
                code.extend_from_slice(&[0x6F, 0x00, 0x08, 0x00]); // jal x1, 0 (占位符)
                relocations.push((call_offset, *target));
            }
            _ => {
                code.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]); // 默认返回
            }
        }
        Ok((code, relocations))
    }

    /// 编译终结符到 x86-64（带重定位支持）
    fn compile_terminator_x86_64(&self, term: &Terminator) -> Result<Vec<u8>, String> {
        let mut code = Vec::new();
        match term {
            Terminator::Ret => {
                code.push(0xC3); // ret
            }
            Terminator::Jmp { target } => {
                // jmp target (需要重定位)
                code.push(0xE9); // jmp rel32
                let offset_pos = code.len();
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // 占位符
                // 注意：重定位会在 add_ir_block 中通过 analyze_block_dependencies 处理
                // 这里只是生成占位符代码
            }
            Terminator::CondJmp { cond, target_true, target_false } => {
                let cond_reg = self.encode_x86_64_reg(*cond);
                // test cond, cond
                code.push(0x48); // REX.W
                code.push(0x85); // test
                code.push(0xC0 | (cond_reg << 3) | cond_reg); // test reg, reg
                // jnz target_true
                code.push(0x0F); // jnz
                code.push(0x85); // jnz rel32
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // 占位符
                // jmp target_false
                code.push(0xE9); // jmp rel32
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // 占位符
            }
            Terminator::Call { target, .. } => {
                // call target (需要重定位)
                code.push(0xE8); // call rel32
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // 占位符
            }
            Terminator::JmpReg { base, offset } => {
                // jmp [base + offset]
                let base_reg = self.encode_x86_64_reg(*base);
                if *offset == 0 {
                    code.push(0xFF); // jmp
                    code.push(0xE0 | base_reg); // jmp reg
                } else {
                    code.push(0xFF); // jmp
                    if *offset >= -128 && *offset <= 127 {
                        code.push(0x60 | base_reg); // jmp [reg+disp8]
                        code.push(*offset as u8);
                    } else {
                        code.push(0xA0 | base_reg); // jmp [reg+disp32]
                        code.extend_from_slice(&(*offset as i32).to_le_bytes());
                    }
                }
            }
            _ => {
                code.push(0xC3); // 默认返回
            }
        }
        Ok(code)
    }

    /// 编译终结符到 ARM64
    fn compile_terminator_arm64(&self, term: &Terminator) -> Result<Vec<u8>, String> {
        let mut code = Vec::new();
        match term {
            Terminator::Ret => {
                code.extend_from_slice(&[0xC0, 0x03, 0x5F, 0xD6]); // ret
            }
            Terminator::Jmp { .. } => {
                // b target (需要重定位)
                code.extend_from_slice(&[0x00, 0x00, 0x00, 0x14]); // b #0 (占位符)
            }
            _ => {
                code.extend_from_slice(&[0xC0, 0x03, 0x5F, 0xD6]); // 默认返回
            }
        }
        Ok(code)
    }

    /// 编译终结符到 RISC-V64
    fn compile_terminator_riscv64(&self, term: &Terminator) -> Result<Vec<u8>, String> {
        let mut code = Vec::new();
        match term {
            Terminator::Ret => {
                code.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]); // ret
            }
            Terminator::Jmp { .. } => {
                // jal x0, target (需要重定位)
                code.extend_from_slice(&[0x6F, 0x00, 0x00, 0x00]); // jal x0, 0 (占位符)
            }
            _ => {
                code.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]); // 默认返回
            }
        }
        Ok(code)
    }

    /// 通过 LLVM 编译 IR 块
    fn compile_ir_block_llvm(
        &self,
        block: &IRBlock,
        optimization_level: u32,
    ) -> Result<Vec<u8>, String> {
        // 将 IRBlock 转换为 LLVM IR
        let llvm_ir = self.ir_block_to_llvm_ir(block)?;

        // 应用优化
        let opt_level = match optimization_level {
            0 => OptimizationLevel::O0,
            1 => OptimizationLevel::O1,
            _ => OptimizationLevel::O2,
        };
        let pass_manager = PassManager::new(opt_level);
        let optimized_ir = self.apply_optimization_passes(&[llvm_ir], &pass_manager);

        // 从优化的 LLVM IR 生成机器码
        self.generate_machine_code_from_llvm_ir(&optimized_ir)
    }

    /// 将 IRBlock 转换为 LLVM IR 字符串
    fn ir_block_to_llvm_ir(&self, block: &IRBlock) -> Result<String, String> {
        let mut ir_lines = Vec::new();
        let func_name = format!("block_{:x}", block.start_pc);

        // 函数签名
        ir_lines.push(format!("define i64 @{}() {{", func_name));
        ir_lines.push("entry:".to_string());

        // 转换每个 IR 操作
        for (idx, op) in block.ops.iter().enumerate() {
            let op_ir = self.ir_op_to_llvm_ir(op, idx)?;
            ir_lines.push(format!("  {}", op_ir));
        }

        // 转换终结符
        let term_ir = self.terminator_to_llvm_ir(&block.term)?;
        ir_lines.push(format!("  {}", term_ir));

        ir_lines.push("}".to_string());

        Ok(ir_lines.join("\n"))
    }

    /// 将 IR 操作转换为 LLVM IR
    fn ir_op_to_llvm_ir(&self, op: &IROp, idx: usize) -> Result<String, String> {
        match op {
            // 算术运算
            IROp::Add { dst, src1, src2 } => {
                Ok(format!("%tmp{} = add i64 %r{}, %r{}", idx, src1, src2))
            }
            IROp::Sub { dst, src1, src2 } => {
                Ok(format!("%tmp{} = sub i64 %r{}, %r{}", idx, src1, src2))
            }
            IROp::Mul { dst, src1, src2 } => {
                Ok(format!("%tmp{} = mul i64 %r{}, %r{}", idx, src1, src2))
            }
            IROp::Div { dst, src1, src2, signed: true } => {
                Ok(format!("%tmp{} = sdiv i64 %r{}, %r{}", idx, src1, src2))
            }
            IROp::Div { dst, src1, src2, signed: false } => {
                Ok(format!("%tmp{} = udiv i64 %r{}, %r{}", idx, src1, src2))
            }
            IROp::Rem { dst, src1, src2, signed: true } => {
                Ok(format!("%tmp{} = srem i64 %r{}, %r{}", idx, src1, src2))
            }
            IROp::Rem { dst, src1, src2, signed: false } => {
                Ok(format!("%tmp{} = urem i64 %r{}, %r{}", idx, src1, src2))
            }
            // 逻辑运算
            IROp::And { dst, src1, src2 } => {
                Ok(format!("%tmp{} = and i64 %r{}, %r{}", idx, src1, src2))
            }
            IROp::Or { dst, src1, src2 } => {
                Ok(format!("%tmp{} = or i64 %r{}, %r{}", idx, src1, src2))
            }
            IROp::Xor { dst, src1, src2 } => {
                Ok(format!("%tmp{} = xor i64 %r{}, %r{}", idx, src1, src2))
            }
            IROp::Not { dst, src } => {
                Ok(format!("%tmp{} = xor i64 %r{}, -1", idx, src))
            }
            // 移位运算
            IROp::Sll { dst, src, shreg } => {
                Ok(format!("%tmp{} = shl i64 %r{}, %r{}", idx, src, shreg))
            }
            IROp::Srl { dst, src, shreg } => {
                Ok(format!("%tmp{} = lshr i64 %r{}, %r{}", idx, src, shreg))
            }
            IROp::Sra { dst, src, shreg } => {
                Ok(format!("%tmp{} = ashr i64 %r{}, %r{}", idx, src, shreg))
            }
            // 立即数操作
            IROp::AddImm { dst, src, imm } => {
                Ok(format!("%tmp{} = add i64 %r{}, {}", idx, src, imm))
            }
            IROp::MulImm { dst, src, imm } => {
                Ok(format!("%tmp{} = mul i64 %r{}, {}", idx, src, imm))
            }
            IROp::MovImm { dst, imm } => {
                Ok(format!("%r{} = add i64 0, {}", dst, imm))
            }
            IROp::SllImm { dst, src, sh } => {
                Ok(format!("%tmp{} = shl i64 %r{}, {}", idx, src, sh))
            }
            IROp::SrlImm { dst, src, sh } => {
                Ok(format!("%tmp{} = lshr i64 %r{}, {}", idx, src, sh))
            }
            IROp::SraImm { dst, src, sh } => {
                Ok(format!("%tmp{} = ashr i64 %r{}, {}", idx, src, sh))
            }
            // 比较操作
            IROp::CmpEq { dst, lhs, rhs } => {
                Ok(format!("%tmp{} = icmp eq i64 %r{}, %r{}", idx, lhs, rhs))
            }
            IROp::CmpNe { dst, lhs, rhs } => {
                Ok(format!("%tmp{} = icmp ne i64 %r{}, %r{}", idx, lhs, rhs))
            }
            IROp::CmpLt { dst, lhs, rhs } => {
                Ok(format!("%tmp{} = icmp slt i64 %r{}, %r{}", idx, lhs, rhs))
            }
            IROp::CmpLtU { dst, lhs, rhs } => {
                Ok(format!("%tmp{} = icmp ult i64 %r{}, %r{}", idx, lhs, rhs))
            }
            IROp::CmpGe { dst, lhs, rhs } => {
                Ok(format!("%tmp{} = icmp sge i64 %r{}, %r{}", idx, lhs, rhs))
            }
            IROp::CmpGeU { dst, lhs, rhs } => {
                Ok(format!("%tmp{} = icmp uge i64 %r{}, %r{}", idx, lhs, rhs))
            }
            IROp::Select { dst, cond, true_val, false_val } => {
                Ok(format!("%tmp{} = select i1 %r{}, i64 %r{}, i64 %r{}", idx, cond, true_val, false_val))
            }
            // 内存操作
            IROp::Load { dst, base, offset, size, .. } => {
                let ptr_type = match size {
                    1 => "i8",
                    2 => "i16",
                    4 => "i32",
                    8 => "i64",
                    _ => return Err(format!("Unsupported load size: {}", size)),
                };
                Ok(format!("%tmp{} = load {}, i64* %r{}, align {}", idx, ptr_type, base, size))
            }
            IROp::Store { src, base, offset, size, .. } => {
                let val_type = match size {
                    1 => "i8",
                    2 => "i16",
                    4 => "i32",
                    8 => "i64",
                    _ => return Err(format!("Unsupported store size: {}", size)),
                };
                Ok(format!("store {} %r{}, i64* %r{}, align {}", val_type, src, base, size))
            }
            // 浮点运算（简化处理）
            IROp::Fadd { dst, src1, src2 } => {
                Ok(format!("%tmp{} = fadd double %r{}, %r{}", idx, src1, src2))
            }
            IROp::Fsub { dst, src1, src2 } => {
                Ok(format!("%tmp{} = fsub double %r{}, %r{}", idx, src1, src2))
            }
            IROp::Fmul { dst, src1, src2 } => {
                Ok(format!("%tmp{} = fmul double %r{}, %r{}", idx, src1, src2))
            }
            IROp::Fdiv { dst, src1, src2 } => {
                Ok(format!("%tmp{} = fdiv double %r{}, %r{}", idx, src1, src2))
            }
            IROp::Nop => Ok("; nop".to_string()),
            _ => {
                // 其他操作：生成注释
                Ok(format!("; {:?}", op))
            }
        }
    }

    /// 将终结符转换为 LLVM IR
    fn terminator_to_llvm_ir(&self, term: &Terminator) -> Result<String, String> {
        match term {
            Terminator::Ret => Ok("ret i64 0".to_string()),
            Terminator::Jmp { target } => {
                Ok(format!("br label @block_{:x}", target))
            }
            Terminator::CondJmp { cond, target_true, target_false } => {
                Ok(format!(
                    "br i1 %r{}, label @block_{:x}, label @block_{:x}",
                    cond, target_true, target_false
                ))
            }
            _ => Ok("ret i64 0".to_string()),
        }
    }

    /// 并行编译多个IR块
    pub fn compile_blocks_parallel(
        &mut self,
        blocks: &[(u64, IRBlock)],
        opt_level: u32,
    ) -> Result<(), String> {
        use rayon::prelude::*;

        // 检查重复
        for (pc, _) in blocks {
            if self.processed_blocks.contains_key(pc) {
                return Err(format!("Block at {:#x} already exists", pc));
            }
        }

        // 并行编译所有块
        let compiled: Vec<(u64, Vec<u8>, Vec<u64>)> = blocks
            .par_iter()
            .map(|(pc, block)| {
                let code = self.compile_block_internal(block, opt_level);
                let deps = DependencyAnalyzer::analyze_block_dependencies(block);
                (*pc, code, deps)
            })
            .collect();

        // 串行添加到镜像（避免并发写入）
        for (pc, code, deps) in compiled {
            let code_offset = self.image.code_section.len() as u32;
            let code_size = code.len() as u32;

            self.image.add_code_block(pc, &code, opt_level);

            // 更新代码块条目
            if let Some(block_entry) = self.image.code_blocks.last_mut() {
                for dep_pc in &deps {
                    block_entry.add_dependency(*dep_pc);
                }
                block_entry.optimization_level = opt_level;
            }

            // 添加依赖关系
            if !deps.is_empty() {
                self.image.add_dependency(pc, deps, DependencyType::DirectJump);
            }

            self.processed_blocks.insert(pc, code.len());
            self.total_code_size += code.len();

            // 添加符号
            let symbol_name = format!("block_{:x}", pc);
            self.image.add_symbol(symbol_name, code_offset as u64, code_size, SymbolType::BlockLabel);
        }

        Ok(())
    }

    /// 编译代码块，支持依赖关系感知的并行编译
    ///
    /// 根据配置决定是否启用并行编译，并考虑代码块之间的依赖关系
    pub fn compile_blocks_with_dependencies(
        &mut self,
        blocks: &[(u64, IRBlock)],
        opt_level: u32,
    ) -> Result<(), String> {
        if !self.options.enable_parallel_compilation {
            // 串行编译
            for (pc, block) in blocks {
                self.add_ir_block(*pc, block, opt_level)?;
            }
            return Ok(());
        }

        if !self.options.respect_dependencies {
            // 不考虑依赖关系的并行编译
            return self.compile_blocks_parallel(blocks, opt_level);
        }

        // 考虑依赖关系的并行编译
        self.compile_blocks_parallel_with_dependencies(blocks, opt_level)
    }

    /// 考虑依赖关系的并行编译
    ///
    /// 使用拓扑排序确保依赖的块先被编译
    fn compile_blocks_parallel_with_dependencies(
        &mut self,
        blocks: &[(u64, IRBlock)],
        opt_level: u32,
    ) -> Result<(), String> {
        use rayon::prelude::*;
        use std::sync::Mutex;

        // 检查重复
        for (pc, _) in blocks {
            if self.processed_blocks.contains_key(pc) {
                return Err(format!("Block at {:#x} already exists", pc));
            }
        }

        // 创建块映射用于依赖分析
        let block_refs: Vec<(u64, &IRBlock)> = blocks.iter()
            .map(|(pc, block)| (*pc, block))
            .collect();

        // 获取编译顺序（拓扑排序）
        let compile_order = crate::DependencyAnalyzer::topological_sort(&block_refs);

        // 创建结果收集器
        let results = Arc::new(Mutex::new(Vec::new()));

        // 并行编译（按依赖顺序分组）
        let mut compiled_blocks = Vec::new();
        let mut dependency_groups = self.group_blocks_by_dependencies(&compile_order, &block_refs);

        for group in dependency_groups {
            // 当前组内的块可以并行编译
            let group_results: Vec<(u64, Vec<u8>, Vec<u64>)> = group
                .par_iter()
                .map(|pc| {
                    let block = blocks.iter()
                        .find(|(block_pc, _)| *block_pc == *pc)
                        .ok_or_else(|| format!("Block not found for PC {:#x}", pc))
                        .unwrap()
                        .1;

                    let code = self.compile_block_internal(block, opt_level);
                    let deps = crate::DependencyAnalyzer::analyze_block_dependencies(block);
                    Ok((*pc, code, deps))
                })
                .collect::<Result<Vec<_>, String>>()?;

            compiled_blocks.extend(group_results);
        }

        // 串行添加到镜像（避免并发写入）
        for (pc, code, deps) in compiled_blocks {
            let code_offset = self.image.code_section.len() as u32;
            let code_size = code.len() as u32;

            self.image.add_code_block(pc, &code, opt_level);

            // 更新代码块条目
            if let Some(block_entry) = self.image.code_blocks.last_mut() {
                for dep_pc in &deps {
                    block_entry.add_dependency(*dep_pc);
                }
                block_entry.optimization_level = opt_level;
            }

            // 添加依赖关系
            if !deps.is_empty() {
                self.image.add_dependency(pc, deps, DependencyType::DirectJump);
            }

            self.processed_blocks.insert(pc, code.len());
            self.total_code_size += code.len();

            // 添加符号
            let symbol_name = format!("block_{:x}", pc);
            self.image.add_symbol(symbol_name, code_offset as u64, code_size, SymbolType::BlockLabel);
        }

        Ok(())
    }

    /// 将代码块按依赖关系分组
    ///
    /// 返回可以并行编译的块组列表
    fn group_blocks_by_dependencies(
        &self,
        compile_order: &[u64],
        blocks: &[(u64, &IRBlock)],
    ) -> Vec<Vec<u64>> {
        let mut groups = Vec::new();
        let mut current_group = Vec::new();
        let mut processed = std::collections::HashSet::new();

        for &pc in compile_order {
            // 检查当前块是否依赖于当前组中的任何块
            let can_add_to_current_group = !current_group.iter().any(|&group_pc| {
                // 检查pc是否依赖于group_pc
                if let Some((_, block)) = blocks.iter().find(|(block_pc, _)| *block_pc == pc) {
                    let deps = crate::DependencyAnalyzer::analyze_block_dependencies(block);
                    deps.contains(&group_pc)
                } else {
                    false
                }
            });

            if can_add_to_current_group {
                current_group.push(pc);
            } else {
                // 开始新组
                if !current_group.is_empty() {
                    groups.push(current_group);
                    current_group = Vec::new();
                }
                current_group.push(pc);
            }
        }

        if !current_group.is_empty() {
            groups.push(current_group);
        }

        groups
    }

    /// 获取并行编译线程数
    pub fn get_parallel_threads(&self) -> usize {
        if self.options.parallel_threads > 0 {
            self.options.parallel_threads
        } else {
            // 自动检测可用核心数
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .max(1)
        }
    }

    /// 编译单个 IR 块（内部方法，用于并行编译）
    fn compile_block_internal(&self, block: &IRBlock, opt_level: u32) -> Vec<u8> {
        match self.options.codegen_mode {
            CodegenMode::Direct => {
                self.compile_ir_block_direct(block, opt_level).unwrap_or_else(|e| {
                    tracing::warn!("Direct compilation failed: {}, using fallback", e);
                    vec![0x90, 0xC3] // NOP + RET
                })
            }
            CodegenMode::LLVM => {
                self.compile_ir_block_llvm(block, opt_level).unwrap_or_else(|e| {
                    tracing::warn!("LLVM compilation failed: {}, using fallback", e);
                    vec![0x90, 0xC3] // NOP + RET
                })
            }
        }
    }

    /// 添加手动编译的代码块
    pub fn add_compiled_block(&mut self, pc: u64, code: Vec<u8>, flags: u32) -> Result<(), String> {
        if self.processed_blocks.contains_key(&pc) {
            return Err(format!("Block at {:#x} already exists", pc));
        }

        self.image.add_code_block(pc, &code, flags);
        self.processed_blocks.insert(pc, code.len());
        self.total_code_size += code.len();

        // 添加符号
        let symbol_name = format!("block_{:x}", pc);
        self.image.add_symbol(
            symbol_name,
            self.image.code_section.len() as u64 - code.len() as u64,
            code.len() as u32,
            SymbolType::BlockLabel,
        );

        Ok(())
    }

    /// 添加重定位信息
    pub fn add_relocation(&mut self, offset: u32, reloc_type: RelationType, target: u64) {
        self.image.add_relocation(offset, reloc_type, target);
    }

    /// 添加符号
    pub fn add_symbol(&mut self, name: String, value: u64, size: u32, symbol_type: SymbolType) {
        self.image.add_symbol(name, value, size, symbol_type);
    }

    /// 获取已处理的块数量
    pub fn block_count(&self) -> usize {
        self.processed_blocks.len()
    }

    /// 获取总代码大小
    pub fn total_size(&self) -> usize {
        self.total_code_size
    }

    /// 获取编译统计信息
    pub fn stats(&self) -> &CompilationStats {
        &self.stats
    }

    /// 获取编译选项
    pub fn options(&self) -> &CompilationOptions {
        &self.options
    }

    /// 更新编译选项
    pub fn set_options(&mut self, options: CompilationOptions) {
        self.options = options;
    }

    /// 检查代码块是否已处理
    pub fn is_block_processed(&self, pc: u64) -> bool {
        self.processed_blocks.contains_key(&pc)
    }

    /// 获取代码块大小
    pub fn get_block_size(&self, pc: u64) -> Option<usize> {
        self.processed_blocks.get(&pc).copied()
    }

    /// 清空所有已处理的块（用于重新编译）
    pub fn clear(&mut self) {
        self.processed_blocks.clear();
        self.total_code_size = 0;
        self.image = AotImage::new();
        self.stats = CompilationStats::default();
    }

    /// 获取所有已处理的代码块地址
    pub fn processed_blocks(&self) -> Vec<u64> {
        self.processed_blocks.keys().copied().collect()
    }

    /// 合并另一个构建器的结果（用于增量编译）
    pub fn merge(&mut self, other: AotBuilder) -> Result<(), String> {
        // 检查冲突
        for pc in other.processed_blocks.keys() {
            if self.processed_blocks.contains_key(pc) {
                return Err(format!("Block at {:#x} already exists in target builder", pc));
            }
        }

        // 合并代码块
        for block_entry in other.image.code_blocks {
            let pc = block_entry.guest_pc;
            let offset = block_entry.code_offset as usize;
            let size = block_entry.code_size as usize;

            if offset + size > other.image.code_section.len() {
                return Err(format!("Invalid code block entry at {:#x}", pc));
            }

            let code = &other.image.code_section[offset..offset + size];
            self.add_compiled_block(pc, code.to_vec(), block_entry.flags)?;
        }

        // 合并符号
        for symbol in other.image.symbols {
            self.image.add_symbol(
                symbol.name,
                symbol.value,
                symbol.size,
                symbol.symbol_type,
            );
        }

        // 合并重定位
        for reloc in other.image.relocations {
            self.image.add_relocation(reloc.offset, reloc.reloc_type, reloc.target);
        }

        // 合并依赖关系
        for dep in other.image.dependencies {
            self.image.add_dependency(dep.source_pc, dep.target_pcs, dep.dependency_type);
        }

        // 合并统计
        self.stats.input_instructions += other.stats.input_instructions;
        self.stats.decoded_instructions += other.stats.decoded_instructions;
        self.stats.generated_ir_instructions += other.stats.generated_ir_instructions;
        self.stats.optimized_ir_instructions += other.stats.optimized_ir_instructions;
        self.stats.output_code_size += other.stats.output_code_size;
        self.stats.compilation_time_ms += other.stats.compilation_time_ms;

        Ok(())
    }

    /// 验证 AOT 镜像的完整性
    pub fn validate(&self) -> Result<(), String> {
        // 检查代码块数量
        if self.image.code_blocks.is_empty() {
            return Err("AOT image contains no code blocks".to_string());
        }

        // 检查代码段大小是否匹配
        let total_code_size: usize = self
            .image
            .code_blocks
            .iter()
            .map(|b| b.code_size as usize)
            .sum();
        if total_code_size != self.image.code_section.len() {
            return Err(format!(
                "Code section size mismatch: expected {}, got {}",
                total_code_size,
                self.image.code_section.len()
            ));
        }

        // 检查符号表一致性
        for symbol in &self.image.symbols {
            if symbol.value as usize > self.image.code_section.len() {
                return Err(format!(
                    "Symbol {} has invalid offset {}",
                    symbol.name, symbol.value
                ));
            }
        }

        // 检查重定位表
        for reloc in &self.image.relocations {
            if reloc.offset as usize > self.image.code_section.len() {
                return Err(format!(
                    "Relocation has invalid offset {}",
                    reloc.offset
                ));
            }
        }

        Ok(())
    }

    /// 构建 AOT 镜像
    pub fn build(self) -> Result<AotImage, String> {
        // 验证镜像
        self.validate()?;
        Ok(self.image)
    }

    /// 构建 AOT 镜像（不验证）
    pub fn build_unchecked(self) -> AotImage {
        self.image
    }

    /// 保存 AOT 镜像到文件
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        if self.enable_compression {
            self.save_to_file_compressed(path)
        } else {
            self.save_to_file_uncompressed(path)
        }
    }

    /// 保存未压缩的 AOT 镜像到文件
    fn save_to_file_uncompressed<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let mut file = std::fs::File::create(path)?;
        self.image.serialize(&mut file)?;
        Ok(())
    }

    /// 保存压缩的 AOT 镜像到文件
    fn save_to_file_compressed<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        use std::io::Write;
        use flate2::write::GzEncoder;
        use flate2::Compression;

        // 先序列化到内存
        let mut buffer = Vec::new();
        self.image.serialize(&mut buffer)?;

        // 压缩数据
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&buffer)?;
        let compressed_data = encoder.finish()?;

        // 写入文件（添加压缩标记）
        let mut file = std::fs::File::create(path)?;
        // 写入压缩标记（4字节）
        file.write_all(b"COMP")?;
        // 写入原始大小（8字节）
        file.write_all(&(buffer.len() as u64).to_le_bytes())?;
        // 写入压缩数据
        file.write_all(&compressed_data)?;

        tracing::info!(
            "Saved compressed AOT image: {} bytes -> {} bytes (compression ratio: {:.2}%)",
            buffer.len(),
            compressed_data.len(),
            (compressed_data.len() as f64 / buffer.len() as f64) * 100.0
        );

        Ok(())
    }

    /// 计算代码块的哈希值（用于去重）
    fn calculate_code_hash(&self, code: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        code.hash(&mut hasher);
        hasher.finish()
    }

    /// 检测并合并重复代码块
    pub fn deduplicate_blocks(&mut self) -> usize {
        if !self.enable_deduplication {
            return 0;
        }

        let mut deduplicated_count = 0;
        let mut code_hash_map: HashMap<u64, Vec<u64>> = HashMap::new();

        // 收集所有代码块的哈希值
        for (pc, _size) in &self.processed_blocks {
            // 获取该PC对应的代码
            if let Some(block_entry) = self.image.code_blocks.iter().find(|b| b.guest_pc == *pc) {
                let code_start = block_entry.code_offset as usize;
                let code_end = code_start + block_entry.code_size as usize;
                if code_end <= self.image.code_section.len() {
                    let code = &self.image.code_section[code_start..code_end];
                    let hash = self.calculate_code_hash(code);
                    code_hash_map.entry(hash).or_insert_with(Vec::new).push(*pc);
                }
            }
        }

        // 找出重复的代码块
        for (hash, pcs) in &code_hash_map {
            if pcs.len() > 1 {
                // 有重复，保留第一个，其他引用第一个
                let first_pc = pcs[0];
                for &duplicate_pc in &pcs[1..] {
                    if let Some(block_entry) = self.image.code_blocks.iter_mut().find(|b| b.guest_pc == duplicate_pc) {
                        // 更新代码偏移指向第一个块
                        if let Some(first_entry) = self.image.code_blocks.iter().find(|b| b.guest_pc == first_pc) {
                            block_entry.code_offset = first_entry.code_offset;
                            deduplicated_count += 1;
                        }
                    }
                }
            }
        }

        deduplicated_count
    }

    /// 打印构建统计信息
    pub fn print_stats(&self) {
        println!("=== AOT Build Statistics ===");
        println!("Processed blocks: {}", self.processed_blocks.len());
        println!("Total code size: {} bytes", self.total_code_size);
        println!(
            "Average block size: {:.1} bytes",
            if self.processed_blocks.len() > 0 {
                self.total_code_size as f64 / self.processed_blocks.len() as f64
            } else {
                0.0
            }
        );

        println!("\n=== Compilation Statistics ===");
        println!("Input instructions: {}", self.stats.input_instructions);
        println!("Decoded instructions: {}", self.stats.decoded_instructions);
        println!(
            "Generated IR instructions: {}",
            self.stats.generated_ir_instructions
        );
        println!(
            "Optimized IR instructions: {}",
            self.stats.optimized_ir_instructions
        );
        println!("Output code size: {} bytes", self.stats.output_code_size);
        println!("Compilation time: {} ms", self.stats.compilation_time_ms);
        println!(
            "Code expansion ratio: {:.2}x",
            self.stats.code_expansion_ratio()
        );
        println!(
            "Optimization reduction: {:.1}%",
            self.stats.optimization_reduction() * 100.0
        );

        println!("\nBlock Details:");
        let mut blocks: Vec<_> = self.processed_blocks.iter().collect();
        blocks.sort_by_key(|(pc, _)| *pc);

        for (pc, size) in blocks {
            println!("  {:#x}: {} bytes", pc, size);
        }
    }
}

impl Default for AotBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aot_builder_creation() {
        let builder = AotBuilder::new();
        assert_eq!(builder.block_count(), 0);
        assert_eq!(builder.total_size(), 0);
    }

    #[test]
    fn test_aot_builder_add_block() {
        let mut builder = AotBuilder::new();

        let code = vec![0x90, 0xC3]; // NOP, RET
        builder.add_compiled_block(0x1000, code.clone(), 1).unwrap();

        assert_eq!(builder.block_count(), 1);
        assert_eq!(builder.total_size(), code.len());
    }

    #[test]
    fn test_aot_builder_multiple_blocks() {
        let mut builder = AotBuilder::new();

        for i in 0..10 {
            let code = vec![0x90; 16];
            builder
                .add_compiled_block(0x1000 + i * 0x100, code.clone(), 1)
                .unwrap();
        }

        assert_eq!(builder.block_count(), 10);
    }

    #[test]
    fn test_aot_builder_duplicate_block() {
        let mut builder = AotBuilder::new();

        let code = vec![0x90, 0xC3];
        builder.add_compiled_block(0x1000, code, 1).unwrap();

        let result = builder.add_compiled_block(0x1000, vec![0x90], 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_aot_builder_build() {
        let mut builder = AotBuilder::new();

        let code = vec![0x48, 0x89, 0xC3];
        builder.add_compiled_block(0x2000, code, 1).unwrap();
        builder.add_symbol("test".to_string(), 0, 3, SymbolType::Function);

        let image = builder.build().unwrap();

        assert_eq!(image.code_blocks.len(), 1);
        assert_eq!(image.code_section.len(), 3);
        assert_eq!(image.symbols.len(), 2); // auto-generated block_2000 + manually added test
    }

    #[test]
    fn test_compilation_options() {
        let opts = CompilationOptions::default();
        assert_eq!(opts.optimization_level, 2);
        assert_eq!(opts.target_isa, ISA::X86_64);
        assert!(opts.enable_applicability_check);
    }

    #[test]
    fn test_compilation_stats() {
        let mut stats = CompilationStats::default();
        stats.input_instructions = 100;
        stats.output_code_size = 200;

        assert_eq!(stats.code_expansion_ratio(), 2.0);

        stats.generated_ir_instructions = 100;
        stats.optimized_ir_instructions = 80;
        assert_eq!(stats.optimization_reduction(), 0.2);
    }

    #[test]
    fn test_aot_builder_with_options() {
        let opts = CompilationOptions {
            optimization_level: 1,
            target_isa: ISA::X86_64,
            enable_applicability_check: false,
            codegen_mode: CodegenMode::Direct,
        };
        let builder = AotBuilder::with_options(opts.clone());
        assert_eq!(builder.options.optimization_level, 1);
    }
}

    #[test]
    fn test_parallel_compilation_with_dependencies() {
        use vm_ir::{IRBuilder, Terminator};

        let options = CompilationOptions {
            enable_parallel_compilation: true,
            respect_dependencies: true,
            parallel_threads: 2,
            ..Default::default()
        };
        let mut builder = AotBuilder::with_options(options);

        // 创建有依赖关系的代码块
        let mut block1 = IRBuilder::new(0x1000);
        block1.push(IROp::MovImm { dst: 1, imm: 10 });
        block1.set_terminator(Terminator::Jmp { target: 0x2000 });
        let block1 = block1.build();

        let mut block2 = IRBuilder::new(0x2000);
        block2.push(IROp::MovImm { dst: 2, imm: 20 });
        block2.set_terminator(Terminator::Ret);
        let block2 = block2.build();

        let blocks = vec![
            (0x1000, block1),
            (0x2000, block2),
        ];

        // 并行编译（考虑依赖关系）
        assert!(builder.compile_blocks_with_dependencies(&blocks, 1).is_ok());

        // 验证结果
        assert_eq!(builder.processed_blocks.len(), 2);
        assert!(builder.processed_blocks.contains_key(&0x1000));
        assert!(builder.processed_blocks.contains_key(&0x2000));
    }

    #[test]
    fn test_parallel_compilation_without_dependencies() {
        use vm_ir::{IRBuilder, Terminator};

        let options = CompilationOptions {
            enable_parallel_compilation: true,
            respect_dependencies: false,
            parallel_threads: 2,
            ..Default::default()
        };
        let mut builder = AotBuilder::with_options(options);

        // 创建简单代码块
        let mut block1 = IRBuilder::new(0x1000);
        block1.push(IROp::MovImm { dst: 1, imm: 10 });
        block1.set_terminator(Terminator::Ret);
        let block1 = block1.build();

        let mut block2 = IRBuilder::new(0x2000);
        block2.push(IROp::MovImm { dst: 2, imm: 20 });
        block2.set_terminator(Terminator::Ret);
        let block2 = block2.build();

        let blocks = vec![
            (0x1000, block1),
            (0x2000, block2),
        ];

        // 并行编译（不考虑依赖关系）
        assert!(builder.compile_blocks_with_dependencies(&blocks, 1).is_ok());

        // 验证结果
        assert_eq!(builder.processed_blocks.len(), 2);
    }

/// 快速生成测试 AOT 镜像
pub fn create_test_aot_image() -> AotImage {
    let mut builder = AotBuilder::new();

    // 添加几个测试块
    for i in 0..5 {
        let code = vec![0x90; 8 + i * 4];
        builder
            .add_compiled_block(0x1000 + i as u64 * 0x100, code, 1)
            .unwrap();
    }

    builder.add_symbol("main".to_string(), 0, 8, SymbolType::Function);

    builder.build().unwrap_or_else(|e| {
        eprintln!("Failed to build test image: {}, using unchecked build", e);
        builder.build_unchecked()
    })
}
