//! LLVM代码生成相关功能

use vm_ir::{IROp, IRBlock, Terminator};
use vm_ir_lift::llvm_integration::{
    LLVMCodeGenerator, LLVMContext, LLVMFunction, LLVMFunctionBuilder,
};
use vm_ir_lift::optimizer::{OptimizationLevel, PassManager};
use super::optimizer::apply_optimization_passes;

/// 通过 LLVM 生成代码
pub fn generate_llvm_code(
    ir_instructions: &[String],
    processed_blocks_count: usize,
) -> Vec<u8> {
    // 创建 LLVM 上下文和代码生成器
    let ctx = LLVMContext::new("aot_module".to_string());
    let mut codegen = LLVMCodeGenerator::new("aot_codegen".to_string());

    // 创建函数构建器
    let func_name = format!("block_{}", processed_blocks_count);
    let mut func_builder = LLVMFunctionBuilder::new(func_name.clone(), "i64".to_string());

    // 将 IR 指令转换为 LLVM IR
    for ir in ir_instructions {
        // 简化转换：将语义 IR 转换为 LLVM IR 指令
        if ir.contains("add") {
            func_builder = func_builder.add_instruction(
                "%result = add i64 %arg1, %arg2".to_string(),
            );
        } else if ir.contains("sub") {
            func_builder = func_builder.add_instruction(
                "%result = sub i64 %arg1, %arg2".to_string(),
            );
        } else if ir.contains("ret") {
            func_builder = func_builder.add_instruction("ret i64 0".to_string());
        } else {
            func_builder = func_builder.add_instruction("; ".to_string() + ir);
        }
    }

    // 如果没有返回指令，添加默认返回
    if !ir_instructions.iter().any(|ir| ir.contains("ret")) {
        func_builder = func_builder.add_instruction("ret i64 0".to_string());
    }

    // 构建函数并添加到代码生成器
    let func = func_builder.build();
    codegen.add_function(func);

    // 生成 LLVM IR（用于调试）
    match codegen.generate() {
        Ok(llvm_ir) => {
            tracing::debug!("Generated LLVM IR:\n{}", llvm_ir);
        }
        Err(e) => {
            tracing::warn!("Failed to generate LLVM IR: {}", e);
        }
    }

    // 从 LLVM IR 生成机器码
    // 注意：这里需要实际的 LLVM 后端，当前使用简化实现
    generate_machine_code_from_llvm_ir(ir_instructions)
}

/// 从 LLVM IR 生成机器码
/// 
/// 在实际实现中，这里应该调用 LLVM 的代码生成后端（如 llvm-sys 或 inkwell）
/// 当前实现是一个简化的代码生成器，用于演示和测试
fn generate_machine_code_from_llvm_ir(ir_instructions: &[String]) -> Vec<u8> {
    // 在实际生产环境中，应该：
    // 1. 使用 LLVM C API 或 Rust 绑定（如 inkwell）
    // 2. 创建 LLVM Module 和 Function
    // 3. 将 IR 字符串解析为 LLVM IR
    // 4. 调用 LLVM 的代码生成后端（如 llc）
    // 5. 返回生成的机器码
    
    // 当前简化实现：根据 IR 指令类型生成对应的机器码
    let mut code = Vec::new();

    for ir in ir_instructions {
        // 根据 IR 指令类型生成对应的机器码
        if ir.contains("add") {
            // add i64 %r1, %r2 -> x86-64: add rax, rbx
            code.extend_from_slice(&[0x48, 0x01, 0xD8]); // add rax, rbx
        } else if ir.contains("sub") {
            // sub i64 %r1, %r2 -> x86-64: sub rax, rbx
            code.extend_from_slice(&[0x48, 0x29, 0xD8]); // sub rax, rbx
        } else if ir.contains("mul") {
            // mul i64 %r1, %r2 -> x86-64: imul rax, rbx
            code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC3]); // imul rax, rbx
        } else if ir.contains("and") {
            // and i64 %r1, %r2 -> x86-64: and rax, rbx
            code.extend_from_slice(&[0x48, 0x21, 0xD8]); // and rax, rbx
        } else if ir.contains("or") {
            // or i64 %r1, %r2 -> x86-64: or rax, rbx
            code.extend_from_slice(&[0x48, 0x09, 0xD8]); // or rax, rbx
        } else if ir.contains("xor") {
            // xor i64 %r1, %r2 -> x86-64: xor rax, rbx
            code.extend_from_slice(&[0x48, 0x31, 0xD8]); // xor rax, rbx
        } else if ir.contains("shl") || ir.contains("shl i64") {
            // shl i64 %r1, %r2 -> x86-64: shl rax, cl
            code.extend_from_slice(&[0x48, 0xD3, 0xE0]); // shl rax, cl
        } else if ir.contains("lshr") || ir.contains("lshr i64") {
            // lshr i64 %r1, %r2 -> x86-64: shr rax, cl
            code.extend_from_slice(&[0x48, 0xD3, 0xE8]); // shr rax, cl
        } else if ir.contains("ashr") || ir.contains("ashr i64") {
            // ashr i64 %r1, %r2 -> x86-64: sar rax, cl
            code.extend_from_slice(&[0x48, 0xD3, 0xF8]); // sar rax, cl
        } else if ir.contains("icmp eq") {
            // icmp eq i64 %r1, %r2 -> x86-64: cmp rax, rbx; sete al
            code.extend_from_slice(&[0x48, 0x39, 0xD8]); // cmp rax, rbx
            code.extend_from_slice(&[0x0F, 0x94, 0xC0]); // sete al
        } else if ir.contains("icmp ne") {
            // icmp ne i64 %r1, %r2 -> x86-64: cmp rax, rbx; setne al
            code.extend_from_slice(&[0x48, 0x39, 0xD8]); // cmp rax, rbx
            code.extend_from_slice(&[0x0F, 0x95, 0xC0]); // setne al
        } else if ir.contains("icmp slt") {
            // icmp slt i64 %r1, %r2 -> x86-64: cmp rax, rbx; setl al
            code.extend_from_slice(&[0x48, 0x39, 0xD8]); // cmp rax, rbx
            code.extend_from_slice(&[0x0F, 0x9C, 0xC0]); // setl al
        } else if ir.contains("load") {
            // load i64, i64* %r1 -> x86-64: mov rax, [rax]
            code.extend_from_slice(&[0x48, 0x8B, 0x00]); // mov rax, [rax]
        } else if ir.contains("store") {
            // store i64 %r1, i64* %r2 -> x86-64: mov [rbx], rax
            code.extend_from_slice(&[0x48, 0x89, 0x03]); // mov [rbx], rax
        } else if ir.contains("ret") {
            code.push(0xC3); // ret
        } else if ir.contains("br label") {
            // 跳转指令：jmp target
            // 注意：这里需要重定位信息，简化实现使用相对跳转
            code.extend_from_slice(&[0xE9, 0x00, 0x00, 0x00, 0x00]); // jmp rel32 (placeholder)
        } else if ir.contains("br i1") {
            // 条件跳转：test + jcc
            code.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
            code.extend_from_slice(&[0x0F, 0x85, 0x00, 0x00, 0x00, 0x00]); // jne rel32 (placeholder)
        } else if ir.starts_with(";") || ir.trim().is_empty() {
            // 注释或空行，跳过
            continue;
        } else {
            // 其他指令：生成 NOP（占位符）
            code.push(0x90); // nop
            tracing::debug!("Unhandled LLVM IR instruction: {}, generating NOP", ir);
        }
    }

    // 确保有返回指令
    if !code.ends_with(&[0xC3]) {
        code.push(0xC3); // ret
    }

    tracing::debug!("Generated {} bytes of machine code from {} LLVM IR instructions", code.len(), ir_instructions.len());
    code
}

/// 通过 LLVM 编译 IR 块
pub fn compile_ir_block_llvm(
    block: &IRBlock,
    optimization_level: u32,
) -> Result<Vec<u8>, String> {
    // 将 IRBlock 转换为 LLVM IR
    let llvm_ir = ir_block_to_llvm_ir(block)?;

    // 应用优化
    let opt_level = match optimization_level {
        0 => OptimizationLevel::O0,
        1 => OptimizationLevel::O1,
        _ => OptimizationLevel::O2,
    };
    let pass_manager = PassManager::new(opt_level);
    let optimized_ir = apply_optimization_passes(&[llvm_ir], &pass_manager);

    // 从优化的 LLVM IR 生成机器码
    Ok(generate_machine_code_from_llvm_ir(&optimized_ir))
}

/// 将 IRBlock 转换为 LLVM IR 字符串
fn ir_block_to_llvm_ir(block: &IRBlock) -> Result<String, String> {
    let mut ir_lines = Vec::new();
    let func_name = format!("block_{:x}", block.start_pc);

    // 函数签名
    ir_lines.push(format!("define i64 @{}() {{", func_name));
    ir_lines.push("entry:".to_string());

    // 转换每个 IR 操作
    for (idx, op) in block.ops.iter().enumerate() {
        let op_ir = ir_op_to_llvm_ir(op, idx)?;
        ir_lines.push(format!("  {}", op_ir));
    }

    // 转换终结符
    let term_ir = terminator_to_llvm_ir(&block.term)?;
    ir_lines.push(format!("  {}", term_ir));

    ir_lines.push("}".to_string());

    Ok(ir_lines.join("\n"))
}

/// 将 IR 操作转换为 LLVM IR
fn ir_op_to_llvm_ir(op: &IROp, idx: usize) -> Result<String, String> {
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
fn terminator_to_llvm_ir(term: &Terminator) -> Result<String, String> {
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

