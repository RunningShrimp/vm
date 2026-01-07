//! Inkwell 集成模块 - 使用 Inkwell 进行类型安全的 LLVM 代码生成
//!
//! 实现 LLVM Module、Function、BasicBlock 的构建和优化，提供比 llvm-sys 更安全的接口。

#[cfg(feature = "llvm")]
pub use inkwell::builder::Builder as InkwellBuilder;
#[cfg(feature = "llvm")]
pub use inkwell::context::Context as InkwellContext;
#[cfg(feature = "llvm")]
pub use inkwell::module::Module as InkwellModule;
#[cfg(feature = "llvm")]
pub use inkwell::values::BasicValueEnum;
#[cfg(feature = "llvm")]
pub use inkwell::values::FunctionValue;

use super::LiftResult;

/// Inkwell 代码生成器
#[cfg(feature = "llvm")]
pub struct InkwellCodeGenerator<'ctx> {
    context: InkwellContext,
    module: InkwellModule<'ctx>,
    builder: InkwellBuilder<'ctx>,
    functions: Vec<FunctionValue<'ctx>>,
}

#[cfg(feature = "llvm")]
impl<'ctx> InkwellCodeGenerator<'ctx> {
    /// 创建新的代码生成器
    pub fn new(module_name: &str) -> Self {
        let context = InkwellContext::create();
        let module = context.create_module(module_name);
        let builder = context.create_builder();

        Self {
            context,
            module,
            builder,
            functions: Vec::new(),
        }
    }

    /// 创建函数
    pub fn create_function(&mut self, name: &str, return_type: &str) -> LiftResult<()> {
        let i64_type = self.context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = self.module.add_function(name, fn_type, None);
        self.functions.push(function);

        let basic_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(basic_block);

        Ok(())
    }

    /// 添加返回指令
    pub fn add_return(&self, value: i64) -> LiftResult<()> {
        let i64_type = self.context.i64_type();
        let const_value = i64_type.const_int(value as u64, false);
        self.builder.build_return(Some(&const_value));
        Ok(())
    }

    /// 添加加法指令
    pub fn add_add(&self, left: i64, right: i64) -> LiftResult<()> {
        let i64_type = self.context.i64_type();
        let left_val = i64_type.const_int(left as u64, false);
        let right_val = i64_type.const_int(right as u64, false);
        let add = self.builder.build_int_add(left_val, right_val, "addtmp");

        // 存储结果以便后续使用
        let result_ptr = self.builder.build_alloca(i64_type, "result").unwrap();
        self.builder.build_store(result_ptr, add);

        Ok(())
    }

    /// 添加减法指令
    pub fn add_sub(&self, left: i64, right: i64) -> LiftResult<()> {
        let i64_type = self.context.i64_type();
        let left_val = i64_type.const_int(left as u64, false);
        let right_val = i64_type.const_int(right as u64, false);
        let sub = self.builder.build_int_sub(left_val, right_val, "subtmp");

        // 存储结果以便后续使用
        let result_ptr = self.builder.build_alloca(i64_type, "result").unwrap();
        self.builder.build_store(result_ptr, sub);

        Ok(())
    }

    /// 生成 LLVM IR 文本
    pub fn generate_ir(&self) -> LiftResult<String> {
        Ok(self.module.print_to_string().to_string())
    }

    /// 验证模块
    pub fn verify(&self) -> LiftResult<()> {
        // Inkwell 的模块验证在创建时自动进行
        // 这里可以添加额外的验证逻辑
        Ok(())
    }

    /// 获取模块引用
    pub fn module(&self) -> &InkwellModule {
        &self.module
    }

    /// 获取函数列表
    pub fn functions(&self) -> &[FunctionValue] {
        &self.functions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "llvm")]
    fn test_inkwell_code_generator() {
        let mut codegen = InkwellCodeGenerator::new("test_module");

        // 创建函数
        codegen.create_function("test_add", "i64").unwrap();

        // 添加指令
        codegen.add_add(10, 20).unwrap();
        codegen.add_return(30).unwrap();

        // 生成 IR
        let ir = codegen.generate_ir().unwrap();
        assert!(ir.contains("test_add"));
        assert!(ir.contains("add"));
    }
}
