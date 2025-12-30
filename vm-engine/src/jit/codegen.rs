//! 代码生成模块（简化版）
//!
//! 提供基础的代码生成功能

use std::collections::HashMap;
use vm_core::{VmResult, GuestAddr};
use vm_ir::IROp;

use crate::jit::core::{JITCompilationResult, JITCompilationStats};

/// 编译后的IR操作
#[derive(Debug, Clone)]
pub struct CompiledIROp {
    /// 操作
    pub op: IROp,
    /// 寄存器分配信息
    pub register_allocation: HashMap<String, String>,
    /// 调度信息
    pub scheduling_info: SchedulingInfo,
}

/// 调度信息
#[derive(Debug, Clone, Default)]
pub struct SchedulingInfo {
    /// 调度后的位置
    pub scheduled_position: usize,
    /// 依赖的指令
    pub dependencies: Vec<usize>,
    /// 延迟
    pub latency: u8,
    /// 调度周期
    pub scheduled_cycle: u32,
}

/// 编译后的IR块
#[derive(Debug, Clone)]
pub struct CompiledIRBlock {
    /// 块ID
    pub id: u64,
    /// 操作列表
    pub ops: Vec<CompiledIROp>,
    /// 寄存器信息
    pub register_info: RegisterInfo,
}

/// 寄存器信息
#[derive(Debug, Clone, Default)]
pub struct RegisterInfo {
    /// 虚拟寄存器到物理寄存器的映射
    pub vreg_to_preg: HashMap<String, String>,
    /// 栈槽列表
    pub stack_slots: Vec<StackSlot>,
}

/// 栈槽
#[derive(Debug, Clone)]
pub struct StackSlot {
    /// 栈槽索引
    pub index: usize,
    /// 栈槽大小（字节）
    pub size: usize,
    /// 对齐要求
    pub alignment: usize,
    /// 栈槽用途
    pub purpose: StackSlotPurpose,
}

/// 栈槽用途
#[derive(Debug, Clone, Copy)]
pub enum StackSlotPurpose {
    /// 溢出槽
    Spill,
    /// 参数槽
    Arg,
    /// 局部变量槽
    Local,
}

/// 代码生成器
///
/// 将编译后的IR块转换为目标代码
pub struct CodeGenerator {
    /// 优化级别
    opt_level: u8,
    /// 生成的代码统计
    stats: CodegenStats,
}

/// 代码生成统计信息
#[derive(Debug, Clone, Default)]
pub struct CodegenStats {
    /// 生成的指令数
    pub generated_insn_count: usize,
    /// 生成的字节数
    pub generated_bytes: usize,
    /// 寄存器溢出次数
    pub spill_count: usize,
    /// 栈槽使用数量
    pub stack_slot_count: usize,
}

/// 默认代码生成器
pub type DefaultCodeGenerator = CodeGenerator;

impl CodeGenerator {
    /// 创建新的代码生成器
    pub fn new() -> Self {
        Self {
            opt_level: 0,
            stats: CodegenStats::default(),
        }
    }

    /// 使用指定优化级别创建代码生成器
    pub fn with_opt_level(level: u8) -> Self {
        Self {
            opt_level: level,
            stats: CodegenStats::default(),
        }
    }

    /// 生成代码
    ///
    /// 将编译后的IR块转换为目标代码
    pub fn generate(&mut self, block: &CompiledIRBlock) -> VmResult<JITCompilationResult> {
        use std::time::Instant;

        let start_time = Instant::now();
        let mut code = Vec::new();
        let mut insn_count = 0;

        // 为每个IR操作生成代码
        for compiled_op in &block.ops {
            let op_code = self.generate_op(compiled_op)?;
            code.extend_from_slice(&op_code);
            insn_count += 1;
        }

        // 测量代码生成时间
        let code_gen_time = start_time.elapsed();

        // 更新统计信息
        self.stats.generated_insn_count = insn_count;
        self.stats.generated_bytes = code.len();
        self.stats.spill_count = block.register_info.stack_slots.len();
        self.stats.stack_slot_count = block.register_info.stack_slots.len();

        // 创建编译统计
        let compilation_stats = JITCompilationStats {
            original_insn_count: block.ops.len(),
            optimized_insn_count: block.ops.len(),
            machine_insn_count: insn_count,
            compilation_time_ns: 0,  // 由上层设置
            optimization_time_ns: 0,  // 由上层设置
            register_allocation_time_ns: 0,  // 由上层设置
            instruction_scheduling_time_ns: 0,  // 由上层设置
            code_generation_time_ns: code_gen_time.as_nanos() as u64,
        };

        let code_size = code.len();
        Ok(JITCompilationResult {
            code,
            entry_point: GuestAddr(block.id), // 使用块ID作为入口点地址
            code_size,
            stats: compilation_stats,
        })
    }

    /// 为单个IR操作生成代码
    ///
    /// 返回操作码字节序列
    fn generate_op(&self, compiled_op: &CompiledIROp) -> VmResult<Vec<u8>> {
        let mut code = Vec::new();

        // 根据IR操作类型生成代码
        match &compiled_op.op {
            // 算术操作
            IROp::Add { dst, src1, src2 } => {
                code.extend_from_slice(&self.encode_arith(*dst, *src1, *src2, 0x00));
            }
            IROp::Sub { dst, src1, src2 } => {
                code.extend_from_slice(&self.encode_arith(*dst, *src1, *src2, 0x01));
            }
            IROp::Mul { dst, src1, src2 } => {
                code.extend_from_slice(&self.encode_arith(*dst, *src1, *src2, 0x02));
            }
            IROp::Div { dst, src1, src2, signed: _ } => {
                code.extend_from_slice(&self.encode_arith(*dst, *src1, *src2, 0x03));
            }

            // 逻辑操作
            IROp::And { dst, src1, src2 } => {
                code.extend_from_slice(&self.encode_logic(*dst, *src1, *src2, 0x10));
            }
            IROp::Or { dst, src1, src2 } => {
                code.extend_from_slice(&self.encode_logic(*dst, *src1, *src2, 0x11));
            }
            IROp::Xor { dst, src1, src2 } => {
                code.extend_from_slice(&self.encode_logic(*dst, *src1, *src2, 0x12));
            }
            IROp::Not { dst, src } => {
                code.extend_from_slice(&self.encode_unary(*dst, *src, 0x13));
            }

            // 移位操作
            IROp::SllImm { dst, src, sh } => {
                code.extend_from_slice(&self.encode_shift_imm(*dst, *src, *sh, 0x20));
            }
            IROp::SrlImm { dst, src, sh } => {
                code.extend_from_slice(&self.encode_shift_imm(*dst, *src, *sh, 0x21));
            }
            IROp::SraImm { dst, src, sh } => {
                code.extend_from_slice(&self.encode_shift_imm(*dst, *src, *sh, 0x22));
            }

            // 内存操作
            IROp::Load { dst, base, offset, size, .. } => {
                code.extend_from_slice(&self.encode_memory(*dst, *base, *offset, *size, 0x30, false));
            }
            IROp::Store { src, base, offset, size, .. } => {
                code.extend_from_slice(&self.encode_memory(*src, *base, *offset, *size, 0x31, true));
            }

            // 立即数操作
            IROp::MovImm { dst, imm } => {
                code.extend_from_slice(&self.encode_mov_imm(*dst, *imm));
            }

            // 其他操作（包括比较和跳转）
            _ => {
                // 未实现或不匹配的操作：生成NOP
                code.push(0x00);
            }
        }

        Ok(code)
    }

    /// 编码算术操作
    fn encode_arith(&self, dst: u32, src1: u32, src2: u32, opcode: u8) -> Vec<u8> {
        // 简化编码：opcode(1) + dst(4) + src1(4) + src2(4) = 13 bytes
        let mut code = Vec::with_capacity(13);
        code.push(opcode);
        code.extend_from_slice(&dst.to_le_bytes());
        code.extend_from_slice(&src1.to_le_bytes());
        code.extend_from_slice(&src2.to_le_bytes());
        code
    }

    /// 编码逻辑操作
    fn encode_logic(&self, dst: u32, src1: u32, src2: u32, opcode: u8) -> Vec<u8> {
        let mut code = Vec::with_capacity(13);
        code.push(opcode);
        code.extend_from_slice(&dst.to_le_bytes());
        code.extend_from_slice(&src1.to_le_bytes());
        code.extend_from_slice(&src2.to_le_bytes());
        code
    }

    /// 编码一元操作
    fn encode_unary(&self, dst: u32, src: u32, opcode: u8) -> Vec<u8> {
        let mut code = Vec::with_capacity(9);
        code.push(opcode);
        code.extend_from_slice(&dst.to_le_bytes());
        code.extend_from_slice(&src.to_le_bytes());
        code
    }

    /// 编码立即数移位
    fn encode_shift_imm(&self, dst: u32, src: u32, shift: u8, opcode: u8) -> Vec<u8> {
        let mut code = Vec::with_capacity(10);
        code.push(opcode);
        code.push(shift);
        code.extend_from_slice(&dst.to_le_bytes());
        code.extend_from_slice(&src.to_le_bytes());
        code
    }

    /// 编码内存操作
    fn encode_memory(&self, base: u32, addr: u32, offset: i64, size: u8, opcode: u8, _is_store: bool) -> Vec<u8> {
        let mut code = Vec::with_capacity(19);
        code.push(opcode);
        code.push(size);
        code.extend_from_slice(&base.to_le_bytes());
        code.extend_from_slice(&addr.to_le_bytes());
        code.extend_from_slice(&offset.to_le_bytes());
        code
    }

    /// 编码立即数移动
    fn encode_mov_imm(&self, dst: u32, imm: u64) -> Vec<u8> {
        let mut code = Vec::with_capacity(13);
        code.push(0x04); // MOV_IMM opcode
        code.extend_from_slice(&dst.to_le_bytes());
        code.extend_from_slice(&imm.to_le_bytes());
        code
    }

    /// 编码比较操作
    fn encode_compare(&self, dst: u32, src: u32, opcode: u8) -> Vec<u8> {
        let mut code = Vec::with_capacity(9);
        code.push(opcode);
        code.extend_from_slice(&dst.to_le_bytes());
        code.extend_from_slice(&src.to_le_bytes());
        code
    }

    /// 编码双操作数比较
    fn encode_compare_2(&self, dst: u32, src1: u32, src2: u32, opcode: u8) -> Vec<u8> {
        let mut code = Vec::with_capacity(13);
        code.push(opcode);
        code.extend_from_slice(&dst.to_le_bytes());
        code.extend_from_slice(&src1.to_le_bytes());
        code.extend_from_slice(&src2.to_le_bytes());
        code
    }

    /// 编码跳转
    ///
    /// 格式: opcode(1) + target(4) = 5 bytes
    fn encode_jump(&self, target: u32, opcode: u8) -> Vec<u8> {
        let mut code = Vec::with_capacity(5);
        code.push(opcode);
        code.extend_from_slice(&target.to_le_bytes());
        code
    }

    /// 编码条件跳转
    ///
    /// 格式: opcode(1) + src_reg(4) + target(4) = 9 bytes
    fn encode_branch(&self, src: u32, target: u32, opcode: u8) -> Vec<u8> {
        let mut code = Vec::with_capacity(9);
        code.push(opcode);
        code.extend_from_slice(&src.to_le_bytes());
        code.extend_from_slice(&target.to_le_bytes());
        code
    }

    /// 获取优化级别
    pub fn opt_level(&self) -> u8 {
        self.opt_level
    }

    /// 设置优化级别
    pub fn set_opt_level(&mut self, level: u8) {
        self.opt_level = level;
    }

    /// 获取统计信息
    pub fn stats(&self) -> &CodegenStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = CodegenStats::default();
    }
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::IRBlock;
    use vm_core::GuestAddr;

    #[test]
    fn test_codegen_creation() {
        let generator = CodeGenerator::new();
        assert_eq!(generator.opt_level(), 0);
    }

    #[test]
    fn test_codegen_with_opt_level() {
        let generator = CodeGenerator::with_opt_level(2);
        assert_eq!(generator.opt_level(), 2);
    }

    #[test]
    fn test_set_opt_level() {
        let mut generator = CodeGenerator::new();
        generator.set_opt_level(3);
        assert_eq!(generator.opt_level(), 3);
    }

    #[test]
    fn test_generate_empty_block() {
        let mut generator = CodeGenerator::new();
        let block = CompiledIRBlock {
            id: 0x1000,
            ops: vec![],
            register_info: RegisterInfo::default(),
        };

        let result = generator.generate(&block).unwrap();
        assert_eq!(result.code.len(), 0);
        assert_eq!(result.code_size, 0);
    }

    #[test]
    fn test_generate_single_nop() {
        let mut generator = CodeGenerator::new();
        let block = CompiledIRBlock {
            id: 0x1000,
            ops: vec![CompiledIROp {
                op: IROp::Nop,
                register_allocation: HashMap::new(),
                scheduling_info: SchedulingInfo::default(),
            }],
            register_info: RegisterInfo::default(),
        };

        let result = generator.generate(&block).unwrap();
        assert_eq!(result.code, vec![0x00]);
        assert_eq!(result.stats.machine_insn_count, 1);
    }

    #[test]
    fn test_generate_arith() {
        let mut generator = CodeGenerator::new();
        let block = CompiledIRBlock {
            id: 0x1000,
            ops: vec![CompiledIROp {
                op: IROp::Add {
                    dst: 1,
                    src1: 2,
                    src2: 3,
                },
                register_allocation: HashMap::new(),
                scheduling_info: SchedulingInfo::default(),
            }],
            register_info: RegisterInfo::default(),
        };

        let result = generator.generate(&block).unwrap();
        assert_eq!(result.code.len(), 13); // opcode(1) + 3 * u32(12)
        assert_eq!(result.code[0], 0x00); // ADD opcode
    }

    #[test]
    fn test_codegen_stats() {
        let mut generator = CodeGenerator::new();
        let block = CompiledIRBlock {
            id: 0x1000,
            ops: vec![
                CompiledIROp {
                    op: IROp::Nop,
                    register_allocation: HashMap::new(),
                    scheduling_info: SchedulingInfo::default(),
                },
                CompiledIROp {
                    op: IROp::MovImm {
                        dst: 0,
                        imm: 42,
                    },
                    register_allocation: HashMap::new(),
                    scheduling_info: SchedulingInfo::default(),
                },
            ],
            register_info: RegisterInfo::default(),
        };

        generator.generate(&block).unwrap();
        let stats = generator.stats();
        assert_eq!(stats.generated_insn_count, 2);
        assert_eq!(stats.generated_bytes, 14); // 1 + 13
    }
}
