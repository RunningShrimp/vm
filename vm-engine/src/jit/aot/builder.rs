//! AOT Builder
//!
//! 提供 AOT 镜像构建功能

use crate::jit::aot::format::{AotImage, AotSection};
use std::io;
use vm_core::GuestAddr;
use vm_ir::lift::ISA;

/// 代码生成模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodegenMode {
    LLVM,
    Cranelift,
    SinglePass,
}

/// 编译选项
#[derive(Debug, Clone)]
pub struct CompilationOptions {
    pub optimization_level: u32,
    pub target_isa: ISA,
    pub enable_applicability_check: bool,
    pub codegen_mode: CodegenMode,
    pub enable_parallel_compilation: bool,
    pub parallel_threads: usize,
    pub respect_dependencies: bool,
}

impl Default for CompilationOptions {
    fn default() -> Self {
        Self {
            optimization_level: 2,
            target_isa: ISA::X86_64, // Default to host arch ideally
            enable_applicability_check: true,
            codegen_mode: CodegenMode::LLVM,
            enable_parallel_compilation: false,
            parallel_threads: 1,
            respect_dependencies: true,
        }
    }
}

/// AOT 构建器
pub struct AotBuilder {
    options: CompilationOptions,
    sections: Vec<AotSection>,
}

impl AotBuilder {
    pub fn new() -> Self {
        Self {
            options: CompilationOptions::default(),
            sections: Vec::new(),
        }
    }

    pub fn with_options(options: CompilationOptions) -> Self {
        Self {
            options,
            sections: Vec::new(),
        }
    }

    /// 添加已编译的代码块
    pub fn add_compiled_block(
        &mut self,
        pc: GuestAddr,
        code: Vec<u8>,
        flags: u32,
    ) -> io::Result<()> {
        self.sections.push(AotSection {
            addr: pc,
            data: code,
            flags,
        });
        Ok(())
    }

    /// 获取当前编译选项
    pub fn get_options(&self) -> &CompilationOptions {
        &self.options
    }

    /// 设置编译选项
    pub fn set_options(&mut self, options: CompilationOptions) {
        self.options = options;
    }

    /// 构建 AOT 镜像
    pub fn build(self) -> io::Result<AotImage> {
        let section_count = self.sections.len() as u32;
        let optimization_level = self.options.optimization_level;
        let target_isa = self.options.target_isa as u32;

        let image = AotImage {
            sections: self.sections,
            ..Default::default()
        };

        // 使用编译选项配置镜像
        let mut image = image;
        image.header.section_count = section_count;
        image.header.optimization_level = optimization_level;
        image.header.target_isa = target_isa;

        Ok(image)
    }
}

impl Default for AotBuilder {
    fn default() -> Self {
        Self::new()
    }
}
