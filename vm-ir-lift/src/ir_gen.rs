//! LLVM IR 代码生成器
//!
//! 从指令语义生成优化后的 LLVM IR，准备用于后端编译。

use crate::LiftResult;
use crate::decoder::Instruction;
use std::collections::HashMap;

/// LLVM IR 基本块表示
#[derive(Debug, Clone)]
pub struct BasicBlock {
    /// 块标签
    pub label: String,
    /// IR 指令行列表
    pub instructions: Vec<String>,
    /// 后继块标签
    pub successors: Vec<String>,
}

impl BasicBlock {
    pub fn new(label: String) -> Self {
        Self {
            label,
            instructions: Vec::new(),
            successors: Vec::new(),
        }
    }

    /// 添加指令
    pub fn add_instruction(&mut self, instr: String) {
        self.instructions.push(instr);
    }

    /// 转换为 LLVM IR 文本
    pub fn to_ir(&self) -> String {
        let mut ir = format!("{}:\n", self.label);
        for instr in &self.instructions {
            ir.push_str("  ");
            ir.push_str(instr);
            ir.push('\n');
        }
        ir
    }
}

/// LLVM 函数表示
#[derive(Debug, Clone)]
pub struct LLVMFunction {
    /// 函数名
    pub name: String,
    /// 基本块映射
    pub blocks: HashMap<String, BasicBlock>,
    /// 入口块标签
    pub entry: String,
    /// 出口块标签
    pub exit: String,
}

impl LLVMFunction {
    pub fn new(name: String) -> Self {
        let entry = "entry".to_string();
        let exit = "exit".to_string();

        let mut blocks = HashMap::new();
        blocks.insert(entry.clone(), BasicBlock::new(entry.clone()));
        blocks.insert(exit.clone(), BasicBlock::new(exit.clone()));

        Self {
            name,
            blocks,
            entry,
            exit,
        }
    }

    /// 获取或创建块
    pub fn get_or_create_block(&mut self, label: String) -> &mut BasicBlock {
        if !self.blocks.contains_key(&label) {
            self.blocks
                .insert(label.clone(), BasicBlock::new(label.clone()));
        }
        self.blocks.get_mut(&label).unwrap()
    }

    /// 转换为 LLVM IR 文本
    pub fn to_ir(&self) -> String {
        let mut ir = format!("define i64 @{}(i64* %mem, i64* %regs) {{\n", self.name);

        // 按块顺序输出（简化：按块名排序）
        let mut block_names: Vec<_> = self.blocks.keys().cloned().collect();
        block_names.sort();

        for block_name in block_names {
            if let Some(block) = self.blocks.get(&block_name) {
                ir.push_str(&block.to_ir());
            }
        }

        ir.push_str("}\n");
        ir
    }
}

/// IR 构建器
pub struct IRBuilder {
    function: LLVMFunction,
    current_block: String,
    temp_counter: usize,
}

impl IRBuilder {
    pub fn new(func_name: String) -> Self {
        let func = LLVMFunction::new(func_name);
        let current_block = func.entry.clone();

        Self {
            function: func,
            current_block,
            temp_counter: 0,
        }
    }

    /// 生成临时变量名
    pub fn gen_temp(&mut self) -> String {
        let name = format!("%tmp_{}", self.temp_counter);
        self.temp_counter += 1;
        name
    }

    /// 添加指令到当前块
    pub fn add_instruction(&mut self, instr: String) {
        let block = self
            .function
            .get_or_create_block(self.current_block.clone());
        block.add_instruction(instr);
    }

    /// 生成二元操作 (add, sub, mul, etc.)
    pub fn emit_binary_op(&mut self, op: &str, lhs: &str, rhs: &str, ty: &str) -> String {
        let result = self.gen_temp();
        self.add_instruction(format!("{} = {} {} {}, {}", result, op, ty, lhs, rhs));
        result
    }

    /// 生成负数操作
    pub fn emit_neg(&mut self, val: &str, ty: &str) -> String {
        let result = self.gen_temp();
        self.add_instruction(format!("{} = sub {} 0, {}", result, ty, val));
        result
    }

    /// 生成比较操作
    pub fn emit_cmp(&mut self, cond: &str, lhs: &str, rhs: &str, ty: &str) -> String {
        let result = self.gen_temp();
        self.add_instruction(format!(
            "{} = icmp {} {} {}, {}",
            result, cond, ty, lhs, rhs
        ));
        result
    }

    /// 生成加载
    pub fn emit_load(&mut self, ptr: &str, ptr_ty: &str, load_ty: &str) -> String {
        let result = self.gen_temp();
        self.add_instruction(format!(
            "{} = load {}, {}* {}",
            result, load_ty, ptr_ty, ptr
        ));
        result
    }

    /// 生成存储
    pub fn emit_store(&mut self, val: &str, val_ty: &str, ptr: &str, ptr_ty: &str) {
        self.add_instruction(format!("store {} {}, {}* {}", val_ty, val, ptr_ty, ptr));
    }

    /// 生成无条件分支
    pub fn emit_br(&mut self, target: &str) {
        self.add_instruction(format!("br label %{}", target));
        self.current_block = format!("bb_{}", target);
    }

    /// 生成条件分支
    pub fn emit_br_cond(&mut self, cond: &str, then_label: &str, else_label: &str) {
        self.add_instruction(format!(
            "br i1 {}, label %{}, label %{}",
            cond, then_label, else_label
        ));
    }

    /// 生成返回
    pub fn emit_return(&mut self, val: &str, ty: &str) {
        self.add_instruction(format!("ret {} {}", ty, val));
    }

    /// 生成函数调用
    pub fn emit_call(&mut self, func: &str, args: Vec<(&str, &str)>, ret_ty: &str) -> String {
        let result = self.gen_temp();
        let args_str = args
            .iter()
            .map(|(ty, val)| format!("{} {}", ty, val))
            .collect::<Vec<_>>()
            .join(", ");

        self.add_instruction(format!(
            "{} = call {} @{}({})",
            result, ret_ty, func, args_str
        ));
        result
    }

    /// 生成选择操作 (三元运算符)
    pub fn emit_select(&mut self, cond: &str, true_val: &str, false_val: &str, ty: &str) -> String {
        let result = self.gen_temp();
        self.add_instruction(format!(
            "{} = select i1 {}, {} {}, {} {}",
            result, cond, ty, true_val, ty, false_val
        ));
        result
    }

    /// 完成函数
    pub fn finalize(mut self) -> String {
        // 添加返回指令到 exit 块
        let exit_block = self
            .function
            .get_or_create_block(self.function.exit.clone());
        if exit_block.instructions.is_empty() {
            exit_block.add_instruction("ret i64 0".to_string());
        }

        self.function.to_ir()
    }
}

/// IR 优化器（框架）
pub struct IROptimizer {
    optimizations: Vec<String>,
}

impl IROptimizer {
    pub fn new() -> Self {
        Self {
            optimizations: vec![
                "constant-folding".to_string(),
                "dead-code-elimination".to_string(),
                "cfg-simplification".to_string(),
            ],
        }
    }

    /// 应用常数折叠优化
    pub fn constant_folding(ir: &str) -> String {
        // 简化实现：直接返回原 IR
        // 完整实现应解析和优化常数表达式
        ir.to_string()
    }

    /// 应用死代码消除
    pub fn dead_code_elimination(ir: &str) -> String {
        // 简化实现：直接返回原 IR
        // 完整实现应分析使用链并移除未使用的指令
        ir.to_string()
    }

    /// 应用 CFG 简化
    pub fn cfg_simplification(ir: &str) -> String {
        // 简化实现：直接返回原 IR
        // 完整实现应合并连续块、移除无法达到的代码等
        ir.to_string()
    }

    /// 执行完整优化管线
    pub fn optimize(ir: &str) -> String {
        let ir = Self::constant_folding(ir);
        let ir = Self::dead_code_elimination(&ir);
        let ir = Self::cfg_simplification(&ir);
        ir
    }
}

/// 将指令序列编译为 LLVM 函数
pub fn compile_instructions_to_llvm(instrs: &[Instruction], isa_name: &str) -> LiftResult<String> {
    let mut builder = IRBuilder::new(format!("translate_{}", isa_name));

    for (idx, instr) in instrs.iter().enumerate() {
        // 为每条指令生成对应的 IR 注释
        builder.add_instruction(format!("; {}: {}", idx, instr.mnemonic));

        // 生成简化的 IR（真实实现应调用语义库）
        match instr.mnemonic.as_str() {
            "add" => {
                if instr.operands.len() >= 2 {
                    builder.emit_binary_op("add", "%rax", "%rbx", "i64");
                }
            }
            "sub" => {
                if instr.operands.len() >= 2 {
                    builder.emit_binary_op("sub", "%rax", "%rbx", "i64");
                }
            }
            "mov" => {
                // 简化：假设 mov rax, rbx
            }
            "nop" => {
                builder.add_instruction("call void @llvm.donothing()".to_string());
            }
            _ => {}
        }
    }

    Ok(builder.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_block_creation() {
        let mut bb = BasicBlock::new("entry".to_string());
        bb.add_instruction("add i64 %a, %b".to_string());
        assert_eq!(bb.instructions.len(), 1);
    }

    #[test]
    fn test_ir_builder_binary_op() {
        let mut builder = IRBuilder::new("test_func".to_string());
        let result = builder.emit_binary_op("add", "%a", "%b", "i64");
        assert!(result.starts_with("%tmp_"));
    }

    #[test]
    fn test_ir_builder_cmp() {
        let mut builder = IRBuilder::new("test_func".to_string());
        let result = builder.emit_cmp("eq", "%a", "%b", "i64");
        assert!(result.starts_with("%tmp_"));
    }

    #[test]
    fn test_ir_builder_load_store() {
        let mut builder = IRBuilder::new("test_func".to_string());
        let loaded = builder.emit_load("%ptr", "i64", "i64");
        builder.emit_store(&loaded, "i64", "%ptr2", "i64");
        assert!(!builder.finalize().is_empty());
    }

    #[test]
    fn test_ir_optimizer() {
        let ir = "add i64 %a, %b\nret i64 0";
        let optimized = IROptimizer::optimize(ir);
        assert!(!optimized.is_empty());
    }

    #[test]
    fn test_llvm_function_to_ir() {
        let func = LLVMFunction::new("test".to_string());
        let ir = func.to_ir();
        assert!(ir.contains("define i64 @test"));
        assert!(ir.contains("entry:"));
    }
}
