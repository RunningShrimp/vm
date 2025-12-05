//! IR优化相关功能

use vm_ir_lift::llvm_integration::LLVMPassExecutor;
use vm_ir_lift::optimizer::{PassManager, OptimizationLevel};

/// 应用优化 Pass
pub fn apply_optimization_passes(
    ir_instructions: &[String],
    pass_manager: &PassManager,
) -> Vec<String> {
    // 将 IR 指令合并为单个字符串
    let ir_text = ir_instructions.join("\n");

    // 创建 Pass 执行器
    let mut executor = LLVMPassExecutor::new();

    // 根据 PassManager 配置添加 Pass
    for pass in pass_manager.passes() {
        match pass {
            vm_ir_lift::optimizer::LLVMPass::ConstantFolding => {
                executor.add_pass("constant-folding".to_string());
            }
            vm_ir_lift::optimizer::LLVMPass::DeadCodeElimination => {
                executor.add_pass("dead-code-elimination".to_string());
            }
            vm_ir_lift::optimizer::LLVMPass::InstructionCombining => {
                executor.add_pass("instruction-combining".to_string());
            }
            vm_ir_lift::optimizer::LLVMPass::CFGSimplification => {
                executor.add_pass("cfg-simplification".to_string());
            }
            _ => {
                // 其他 Pass 暂不支持，跳过
                tracing::debug!("Pass {:?} not yet implemented, skipping", pass);
            }
        }
    }

    // 执行优化
    match executor.run(&ir_text) {
        Ok((optimized_ir, _stats)) => {
            // 将优化后的 IR 文本分割回指令列表
            optimized_ir
                .lines()
                .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with(';'))
                .map(|s| s.to_string())
                .collect()
        }
        Err(e) => {
            tracing::warn!("Optimization failed: {}, using original IR", e);
            ir_instructions.to_vec()
        }
    }
}

/// 将优化级别转换为PassManager的优化级别
pub fn optimization_level_to_pass_level(level: u32) -> OptimizationLevel {
    match level {
        0 => OptimizationLevel::O0,
        1 => OptimizationLevel::O1,
        _ => OptimizationLevel::O2,
    }
}


