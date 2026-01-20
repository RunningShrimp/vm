//! IR 指令表示（用于解码器）
//!
//! 这个模块定义了与 vm-ir::IROp 不同的 IR 指令格式，
//! 主要用于架构解码器返回简化后的指令表示。

use crate::RegId;

/// 二元操作符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Sub,
}

/// IR 指令表示（简化版，用于解码器）
#[derive(Debug, Clone)]
pub enum IRInstruction {
    /// 分支指令
    Branch {
        target: Operand,
        link: bool,
    },
    /// 条件分支指令
    CondBranch {
        condition: Operand,
        target: Operand,
        link: bool,
    },
    /// 二元操作
    BinaryOp {
        op: BinaryOperator,
        dest: RegId,
        src1: Operand,
        src2: Operand,
    },
    /// 加载指令
    Load {
        dest: RegId,
        addr: Operand,
        size: u8,
    },
    /// 存储指令
    Store {
        value: Operand,
        addr: Operand,
        size: u8,
    },
    /// 无操作
    Nop,
}

/// 操作数
#[derive(Debug, Clone)]
pub enum Operand {
    /// 寄存器
    Reg(RegId),
    /// 立即数
    Imm64(u64),
}
