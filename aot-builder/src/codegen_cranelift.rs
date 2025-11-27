//! Cranelift AOT 代码生成器
//!
//! 使用 Cranelift 生成目标文件 (.o)。

use vm_ir::IRBlock;
use vm_engine_jit::cranelift_translator::CraneliftTranslator;
use cranelift::prelude::*;
use cranelift_module::{Module, Linkage};
use cranelift_object::{ObjectBuilder, ObjectModule};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use target_lexicon::Triple;
use std::path::Path;

/// Cranelift AOT 编译器
pub struct CraneliftAotCompiler {
    module: ObjectModule,
    ctx: Context,
    builder_ctx: FunctionBuilderContext,
    compiled_count: usize,
}

impl CraneliftAotCompiler {
    /// 创建新的 AOT 编译器
    pub fn new(target_triple: Option<Triple>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut flag_builder = settings::builder();
        flag_builder.enable("is_pic")?;
        flag_builder.set("opt_level", "speed")?;
        
        let isa = cranelift_native::builder()?
            .finish(settings::Flags::new(flag_builder))?;

        let builder = ObjectBuilder::new(isa, "vm_module", cranelift_module::default_libcall_names())?;
        let module = ObjectModule::new(builder);
        let ctx = module.make_context();
        let builder_ctx = FunctionBuilderContext::new();
        
        Ok(Self {
            module,
            ctx,
            builder_ctx,
            compiled_count: 0,
        })
    }

    /// 编译单个 IR 块
    pub fn compile(&mut self, block: &IRBlock) -> Result<(), Box<dyn std::error::Error>> {
        self.module.clear_context(&mut self.ctx);
        
        self.ctx.func.signature.params.clear();
        self.ctx.func.signature.returns.clear();
        
        {
            let builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_ctx);
            let mut translator = CraneliftTranslator::new(builder);
            translator.translate(block)?;
        }
        
        let func_name = format!("block_{:x}", block.start_pc);
        let func_id = self.module.declare_function(
            &func_name,
            Linkage::Export,
            &self.ctx.func.signature,
        )?;
        
        self.module.define_function(func_id, &mut self.ctx)?;
        self.compiled_count += 1;
        
        Ok(())
    }

    /// 编译多个 IR 块
    pub fn compile_batch(&mut self, blocks: &[IRBlock]) -> Result<usize, Box<dyn std::error::Error>> {
        let mut count = 0;
        for block in blocks {
            match self.compile(block) {
                Ok(()) => count += 1,
                Err(e) => {
                    eprintln!("Warning: Failed to compile block {:x}: {}", block.start_pc, e);
                }
            }
        }
        Ok(count)
    }

    /// 完成编译并生成目标文件字节
    pub fn finish(self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let product = self.module.finish();
        let bytes = product.emit()?;
        Ok(bytes)
    }

    /// 完成编译并写入文件
    pub fn write_to_file<P: AsRef<Path>>(self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = self.finish()?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// 获取编译的块数
    pub fn compiled_count(&self) -> usize {
        self.compiled_count
    }
}
            
        Ok(())
    }

    /// 编译多个 IR 块
    pub fn compile_batch(&mut self, blocks: &[IRBlock]) -> Result<usize, Box<dyn std::error::Error>> {
        let mut count = 0;
        for block in blocks {
            match self.compile(block) {
                Ok(()) => count += 1,
                Err(e) => {
                    eprintln!("Warning: Failed to compile block {:x}: {}", block.start_pc, e);
                }
            }
        }
        Ok(count)
    }

    /// 完成编译并生成目标文件字节
    pub fn finish(self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let product = self.module.finish();
        let bytes = product.emit()?;
        Ok(bytes)
    }

    /// 完成编译并写入文件
    pub fn write_to_file<P: AsRef<Path>>(self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = self.finish()?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// 获取编译的块数
    pub fn compiled_count(&self) -> usize {
        self.compiled_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::Terminator;

    #[test]
    fn test_compiler_creation() {
        let result = CraneliftAotCompiler::new(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_simple_block() {
        let block = IRBlock {
            start_pc: vm_core::GuestAddr::from(0x1000u64),
            ops: vec![],
            term: Terminator::Ret { value: None },
        };

        let mut compiler = CraneliftAotCompiler::new(None).unwrap();
        assert!(compiler.compile(&block).is_ok());
        assert_eq!(compiler.compiled_count(), 1);
    }
}
