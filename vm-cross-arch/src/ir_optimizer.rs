//! IR优化模块
//!
//! 实现各种IR级别的优化，包括常量折叠、死代码消除、公共子表达式消除等

use std::collections::{HashMap, HashSet};
use vm_ir::{IROp, RegId};

/// IR优化器
pub struct IROptimizer {
    /// 优化统计信息
    stats: OptimizationStats,
    /// 常量值映射
    constant_values: HashMap<RegId, i64>,
    /// 活跃寄存器集合
    live_registers: HashSet<RegId>,
    /// 已计算的子表达式
    computed_expressions: HashMap<SubExpression, RegId>,
}

/// 优化统计信息
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    /// 常量折叠次数
    pub constant_folds: usize,
    /// 死代码消除次数
    pub dead_code_eliminations: usize,
    /// 公共子表达式消除次数
    pub common_subexpression_eliminations: usize,
    /// 代数简化次数
    pub algebraic_simplifications: usize,
    /// 强度削弱次数
    pub strength_reductions: usize,
}

/// 子表达式表示
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum SubExpression {
    /// 二元操作
    Binary {
        op: BinaryOp,
        left: Operand,
        right: Operand,
    },
    /// 一元操作
    Unary { op: UnaryOp, operand: Operand },
    /// 内存加载
    Load { base: RegId, offset: i64, size: u8 },
}

/// 二元操作类型
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// 一元操作类型
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// 操作数表示
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Operand {
    /// 寄存器
    Register(RegId),
    /// 常量
    Constant(i64),
}

impl IROptimizer {
    /// 创建新的IR优化器
    pub fn new() -> Self {
        Self {
            stats: OptimizationStats::default(),
            constant_values: HashMap::new(),
            live_registers: HashSet::new(),
            computed_expressions: HashMap::new(),
        }
    }

    /// 优化IR操作序列
    pub fn optimize(&mut self, ops: &[IROp]) -> Vec<IROp> {
        // 重置状态
        self.constant_values.clear();
        self.live_registers.clear();
        self.computed_expressions.clear();

        // 第一遍：常量传播和折叠
        let ops1 = self.constant_propagation_and_folding(ops);

        // 第二遍：死代码消除
        let ops2 = self.dead_code_elimination(&ops1);

        // 第三遍：公共子表达式消除
        let ops3 = self.common_subexpression_elimination(&ops2);

        // 第四遍：代数简化和强度削弱
        let ops4 = self.algebraic_simplification(&ops3);

        // 第五遍：窥孔优化

        self.peephole_optimization(&ops4)
    }

    /// 常量传播和折叠
    fn constant_propagation_and_folding(&mut self, ops: &[IROp]) -> Vec<IROp> {
        let mut optimized_ops = Vec::with_capacity(ops.len());

        for op in ops {
            match self.try_constant_fold(op) {
                Some(folded_op) => {
                    optimized_ops.push(folded_op);
                    self.stats.constant_folds += 1;
                }
                None => {
                    // 无法折叠，保留原操作
                    optimized_ops.push(op.clone());
                }
            }
        }

        optimized_ops
    }

    /// 尝试常量折叠
    fn try_constant_fold(&mut self, op: &IROp) -> Option<IROp> {
        match op {
            IROp::MovImm { dst, imm } => {
                // 记录常量值
                self.constant_values.insert(*dst, *imm as i64);
                None // 常量操作本身不需要折叠
            }
            IROp::Add { dst, src1, src2 } => {
                // 尝试折叠加法
                if let (Some(val1), Some(val2)) = (
                    self.get_constant_value(*src1),
                    self.get_constant_value(*src2),
                ) {
                    let result = val1.wrapping_add(val2);
                    self.constant_values.insert(*dst, result);
                    self.stats.constant_folds += 1;
                    return Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    });
                }
                None
            }
            IROp::Sub { dst, src1, src2 } => {
                // 尝试折叠减法
                if let (Some(val1), Some(val2)) = (
                    self.get_constant_value(*src1),
                    self.get_constant_value(*src2),
                ) {
                    let result = val1.wrapping_sub(val2);
                    self.constant_values.insert(*dst, result);
                    self.stats.constant_folds += 1;
                    return Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    });
                }
                None
            }
            IROp::Mul { dst, src1, src2 } => {
                // 尝试折叠乘法
                if let (Some(val1), Some(val2)) = (
                    self.get_constant_value(*src1),
                    self.get_constant_value(*src2),
                ) {
                    let result = val1.wrapping_mul(val2);
                    self.constant_values.insert(*dst, result);
                    self.stats.constant_folds += 1;
                    return Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    });
                }
                None
            }
            IROp::Div {
                dst,
                src1,
                src2,
                signed: _,
            } => {
                // 尝试折叠除法
                if let (Some(val1), Some(val2)) = (
                    self.get_constant_value(*src1),
                    self.get_constant_value(*src2),
                ) && val2 != 0
                {
                    let result = val1.wrapping_div(val2);
                    self.constant_values.insert(*dst, result);
                    self.stats.constant_folds += 1;
                    return Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    });
                }
                None
            }
            IROp::Rem {
                dst,
                src1,
                src2,
                signed,
            } => {
                // 尝试折叠取模
                if let (Some(val1), Some(val2)) = (
                    self.get_constant_value(*src1),
                    self.get_constant_value(*src2),
                ) && val2 != 0
                {
                    let result = if *signed {
                        val1.wrapping_rem_euclid(val2)
                    } else {
                        val1.wrapping_rem(val2)
                    };
                    self.constant_values.insert(*dst, result);
                    self.stats.constant_folds += 1;
                    return Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    });
                }
                None
            }
            IROp::And { dst, src1, src2 } => {
                // 尝试折叠按位与
                if let (Some(val1), Some(val2)) = (
                    self.get_constant_value(*src1),
                    self.get_constant_value(*src2),
                ) {
                    let result = val1 & val2;
                    self.constant_values.insert(*dst, result);
                    self.stats.constant_folds += 1;
                    return Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    });
                }
                None
            }
            IROp::Or { dst, src1, src2 } => {
                // 尝试折叠按位或
                if let (Some(val1), Some(val2)) = (
                    self.get_constant_value(*src1),
                    self.get_constant_value(*src2),
                ) {
                    let result = val1 | val2;
                    self.constant_values.insert(*dst, result);
                    self.stats.constant_folds += 1;
                    return Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    });
                }
                None
            }
            IROp::Xor { dst, src1, src2 } => {
                // 尝试折叠按位异或
                if let (Some(val1), Some(val2)) = (
                    self.get_constant_value(*src1),
                    self.get_constant_value(*src2),
                ) {
                    let result = val1 ^ val2;
                    self.constant_values.insert(*dst, result);
                    self.stats.constant_folds += 1;
                    return Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    });
                }
                None
            }
            IROp::Sll { dst, src, shreg } => {
                // 尝试折叠左移
                if let (Some(val1), Some(val2)) = (
                    self.get_constant_value(*src),
                    self.get_constant_value(*shreg),
                ) {
                    let shift = val2 as u32 % 64; // 确保移位位数在有效范围内
                    let result = val1.wrapping_shl(shift);
                    self.constant_values.insert(*dst, result);
                    self.stats.constant_folds += 1;
                    return Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    });
                }
                None
            }
            IROp::Srl { dst, src, shreg } => {
                // 尝试折叠右移
                if let (Some(val1), Some(val2)) = (
                    self.get_constant_value(*src),
                    self.get_constant_value(*shreg),
                ) {
                    let shift = val2 as u32 % 64; // 确保移位位数在有效范围内
                    let result = val1.wrapping_shr(shift);
                    self.constant_values.insert(*dst, result);
                    self.stats.constant_folds += 1;
                    return Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    });
                }
                None
            }
            _ => None,
        }
    }

    /// 死代码消除
    fn dead_code_elimination(&mut self, ops: &[IROp]) -> Vec<IROp> {
        // 分析活跃寄存器
        self.analyze_live_registers(ops);

        let mut optimized_ops = Vec::with_capacity(ops.len());

        // 反向遍历，更容易识别死代码
        for op in ops.iter().rev() {
            if self.is_useful_operation(op) {
                optimized_ops.push(op.clone());
            } else {
                self.stats.dead_code_eliminations += 1;
            }
        }

        // 反转回正确顺序
        optimized_ops.reverse();
        optimized_ops
    }

    /// 分析活跃寄存器
    fn analyze_live_registers(&mut self, ops: &[IROp]) {
        self.live_registers.clear();

        // 从后向前分析
        for op in ops.iter().rev() {
            match op {
                IROp::Add { dst, src1, src2 }
                | IROp::Sub { dst, src1, src2 }
                | IROp::Mul { dst, src1, src2 }
                | IROp::And { dst, src1, src2 }
                | IROp::Or { dst, src1, src2 }
                | IROp::Xor { dst, src1, src2 } => {
                    // 使用源寄存器
                    self.live_registers.insert(*src1);
                    self.live_registers.insert(*src2);
                    // 定义目标寄存器（如果之前不是活跃的，则可能是死代码）
                    if !self.live_registers.contains(dst) {
                        // 这个寄存器可能不是活跃的
                    }
                }
                IROp::Div {
                    dst,
                    src1,
                    src2,
                    signed: _,
                }
                | IROp::Rem {
                    dst,
                    src1,
                    src2,
                    signed: _,
                } => {
                    // 使用源寄存器
                    self.live_registers.insert(*src1);
                    self.live_registers.insert(*src2);
                    // 定义目标寄存器（如果之前不是活跃的，则可能是死代码）
                    if !self.live_registers.contains(dst) {
                        // 这个寄存器可能不是活跃的
                    }
                }
                IROp::Sll { dst, src, shreg }
                | IROp::Srl { dst, src, shreg }
                | IROp::Sra { dst, src, shreg } => {
                    // 使用源寄存器
                    self.live_registers.insert(*src);
                    self.live_registers.insert(*shreg);
                    // 定义目标寄存器（如果之前不是活跃的，则可能是死代码）
                    if !self.live_registers.contains(dst) {
                        // 这个寄存器可能不是活跃的
                    }
                }
                IROp::Load { dst, base, .. } => {
                    // 使用源寄存器
                    self.live_registers.insert(*base);
                    // 定义目标寄存器
                    if !self.live_registers.contains(dst) {
                        // 这个寄存器可能不是活跃的
                    }
                }
                IROp::Store { src, .. } => {
                    // 使用源寄存器
                    self.live_registers.insert(*src);
                }
                IROp::MovImm { dst, .. } => {
                    // 定义目标寄存器
                    if !self.live_registers.contains(dst) {
                        // 这个寄存器可能不是活跃的
                    }
                }
                _ => {}
            }
        }
    }

    /// 判断操作是否有用
    fn is_useful_operation(&self, op: &IROp) -> bool {
        match op {
            IROp::Sll { dst, src, shreg } | IROp::Srl { dst, src, shreg } => {
                // 如果目标寄存器或源寄存器是活跃的，则操作有用
                self.live_registers.contains(dst)
                    || self.live_registers.contains(src)
                    || self.live_registers.contains(shreg)
            }
            IROp::Add {
                dst,
                src1: _,
                src2: _,
            }
            | IROp::Sub {
                dst,
                src1: _,
                src2: _,
            }
            | IROp::Mul {
                dst,
                src1: _,
                src2: _,
            }
            | IROp::Div {
                dst,
                src1: _,
                src2: _,
                signed: _,
            }
            | IROp::Rem {
                dst,
                src1: _,
                src2: _,
                signed: _,
            }
            | IROp::And {
                dst,
                src1: _,
                src2: _,
            }
            | IROp::Or {
                dst,
                src1: _,
                src2: _,
            }
            | IROp::Xor {
                dst,
                src1: _,
                src2: _,
            } => {
                // 如果目标寄存器是活跃的，则操作有用
                self.live_registers.contains(dst)
            }
            IROp::Load { dst, base, .. } => {
                // 如果目标寄存器或源寄存器是活跃的，则操作有用
                self.live_registers.contains(dst) || self.live_registers.contains(base)
            }
            IROp::Store { src: _, .. } => {
                // 存储操作总是有用的（有副作用）
                true
            }
            IROp::MovImm { dst, .. } => {
                // 如果目标寄存器是活跃的，则操作有用
                self.live_registers.contains(dst)
            }
            _ => true, // 其他操作（如跳转）总是有用的
        }
    }

    /// 公共子表达式消除
    fn common_subexpression_elimination(&mut self, ops: &[IROp]) -> Vec<IROp> {
        let mut optimized_ops = Vec::with_capacity(ops.len());

        for op in ops {
            match self.try_cse(op) {
                Some(cse_op) => {
                    optimized_ops.push(cse_op);
                    self.stats.common_subexpression_eliminations += 1;
                }
                None => {
                    // 无法消除，保留原操作
                    optimized_ops.push(op.clone());
                }
            }
        }

        optimized_ops
    }

    /// 尝试公共子表达式消除
    fn try_cse(&mut self, op: &IROp) -> Option<IROp> {
        match op {
            IROp::Add { dst, src1, src2 } => {
                let expr = SubExpression::Binary {
                    op: BinaryOp::Add,
                    left: Operand::Register(*src1),
                    right: Operand::Register(*src2),
                };

                if let Some(existing_reg) = self.computed_expressions.get(&expr) {
                    // 使用已计算的结果
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *existing_reg,
                    });
                } else {
                    // 记录新计算的表达式
                    self.computed_expressions.insert(expr, *dst);
                }
            }
            IROp::Mul { dst, src1, src2 } => {
                let expr = SubExpression::Binary {
                    op: BinaryOp::Mul,
                    left: Operand::Register(*src1),
                    right: Operand::Register(*src2),
                };

                if let Some(existing_reg) = self.computed_expressions.get(&expr) {
                    // 使用已计算的结果
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *existing_reg,
                    });
                } else {
                    // 记录新计算的表达式
                    self.computed_expressions.insert(expr, *dst);
                }
            }
            IROp::Load {
                dst,
                base,
                offset,
                size,
                ..
            } => {
                let expr = SubExpression::Load {
                    base: *base,
                    offset: *offset,
                    size: *size,
                };

                if let Some(existing_reg) = self.computed_expressions.get(&expr) {
                    // 使用已加载的值
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *existing_reg,
                    });
                } else {
                    // 记录新加载的表达式
                    self.computed_expressions.insert(expr, *dst);
                }
            }
            _ => {}
        }

        None
    }

    /// 代数简化和强度削弱
    fn algebraic_simplification(&mut self, ops: &[IROp]) -> Vec<IROp> {
        let mut optimized_ops = Vec::with_capacity(ops.len());

        for op in ops {
            match self.try_algebraic_simplification(op) {
                Some(simplified_op) => {
                    optimized_ops.push(simplified_op);
                    self.stats.algebraic_simplifications += 1;
                }
                None => {
                    // 无法简化，保留原操作
                    optimized_ops.push(op.clone());
                }
            }
        }

        optimized_ops
    }

    /// 尝试代数简化和强度削弱
    fn try_algebraic_simplification(&mut self, op: &IROp) -> Option<IROp> {
        match op {
            IROp::Mul { dst, src1, src2 } => {
                // 乘以0
                if let Some(val) = self.get_constant_value(*src2)
                    && val == 0
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::MovImm { dst: *dst, imm: 0 });
                }
                if let Some(val) = self.get_constant_value(*src1)
                    && val == 0
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::MovImm { dst: *dst, imm: 0 });
                }

                // 乘以1
                if let Some(val) = self.get_constant_value(*src2)
                    && val == 1
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *src1,
                    });
                }
                if let Some(val) = self.get_constant_value(*src1)
                    && val == 1
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *src2,
                    });
                }

                // 乘以2的幂（转换为移位）
                if let Some(val) = self.get_constant_value(*src2)
                    && let Some(_shift) = self.is_power_of_two(val)
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Sll {
                        dst: *dst,
                        src: *src1,
                        shreg: *src2,
                    });
                }
                if let Some(val) = self.get_constant_value(*src1)
                    && let Some(_shift) = self.is_power_of_two(val)
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Sll {
                        dst: *dst,
                        src: *src2,
                        shreg: *src1,
                    });
                }
            }
            IROp::Div {
                dst, src1, src2, ..
            } => {
                // 除以1
                if let Some(val) = self.get_constant_value(*src2)
                    && val == 1
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *src1,
                    });
                }

                // 除以2的幂（转换为移位）
                if let Some(val) = self.get_constant_value(*src2)
                    && let Some(_shift) = self.is_power_of_two(val)
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Sra {
                        dst: *dst,
                        src: *src1,
                        shreg: *src2,
                    });
                }
            }
            IROp::Add { dst, src1, src2 } => {
                // 加0
                if let Some(val) = self.get_constant_value(*src2)
                    && val == 0
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *src1,
                    });
                }
                if let Some(val) = self.get_constant_value(*src1)
                    && val == 0
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *src2,
                    });
                }
            }
            IROp::Sub { dst, src1, src2 } => {
                // 减0
                if let Some(val) = self.get_constant_value(*src2)
                    && val == 0
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *src1,
                    });
                }
            }
            IROp::And { dst, src1, src2 } => {
                // 与0
                if let Some(val) = self.get_constant_value(*src2)
                    && val == 0
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::MovImm { dst: *dst, imm: 0 });
                }
                if let Some(val) = self.get_constant_value(*src1)
                    && val == 0
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::MovImm { dst: *dst, imm: 0 });
                }

                // 与全1（取决于操作数宽度）
                if let Some(val) = self.get_constant_value(*src2)
                    && (val == -1 || val == 0xFF || val == 0xFFFF || val == 0xFFFFFFFF)
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *src1,
                    });
                }
            }
            IROp::Or { dst, src1, src2 } => {
                // 或0
                if let Some(val) = self.get_constant_value(*src2)
                    && val == 0
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *src1,
                    });
                }
                if let Some(val) = self.get_constant_value(*src1)
                    && val == 0
                {
                    self.stats.strength_reductions += 1;
                    return Some(IROp::Mov {
                        dst: *dst,
                        src: *src2,
                    });
                }
            }
            _ => {}
        }

        None
    }

    /// 窥孔优化
    fn peephole_optimization(&mut self, ops: &[IROp]) -> Vec<IROp> {
        let mut optimized_ops = Vec::with_capacity(ops.len());
        let mut i = 0;

        while i < ops.len() {
            // 尝试匹配和优化指令模式
            if let Some((optimized_pattern, pattern_len)) = self.match_peephole_pattern(&ops[i..]) {
                optimized_ops.extend(optimized_pattern);
                i += pattern_len;
            } else {
                optimized_ops.push(ops[i].clone());
                i += 1;
            }
        }

        optimized_ops
    }

    /// 匹配窥孔优化模式
    fn match_peephole_pattern(&self, ops: &[IROp]) -> Option<(Vec<IROp>, usize)> {
        if ops.len() < 2 {
            return None;
        }

        // 模式1：Mov -> Mov（冗余移动）
        if let (
            IROp::Mov {
                dst: dst1,
                src: src1,
            },
            IROp::Mov {
                dst: dst2,
                src: src2,
            },
        ) = (&ops[0], &ops[1])
            && dst1 == src2
        {
            // mov r1, r2; mov r2, r1 -> 可以优化
            return Some((
                vec![
                    IROp::Mov {
                        dst: *dst1,
                        src: *src1,
                    },
                    IROp::Mov {
                        dst: *dst2,
                        src: *src1,
                    },
                ],
                2,
            ));
        }

        // 模式2：MovImm -> Mov（常量传播）
        if let (
            IROp::MovImm {
                dst: dst1,
                imm: value,
            },
            IROp::Mov {
                dst: dst2,
                src: src2,
            },
        ) = (&ops[0], &ops[1])
            && dst1 == src2
        {
            // movimm r1, 42; mov r2, r1 -> movimm r2, 42
            return Some((
                vec![
                    IROp::MovImm {
                        dst: *dst1,
                        imm: *value,
                    },
                    IROp::MovImm {
                        dst: *dst2,
                        imm: *value,
                    },
                ],
                2,
            ));
        }

        // 模式3：Add -> Sub（加法后立即减去相同值）
        if let (
            IROp::Add {
                dst: dst1,
                src1,
                src2,
            },
            IROp::Sub {
                dst: dst2,
                src1: sub_src1,
                src2: sub_src2,
            },
        ) = (&ops[0], &ops[1])
            && dst1 == sub_src1
            && src2 == sub_src2
        {
            // add r1, r2, 5; sub r3, r1, 5 -> mov r3, r1
            return Some((
                vec![
                    IROp::Add {
                        dst: *dst1,
                        src1: *src1,
                        src2: *src2,
                    },
                    IROp::Mov {
                        dst: *dst2,
                        src: *src1,
                    },
                ],
                2,
            ));
        }

        None
    }

    /// 获取寄存器的常量值
    fn get_constant_value(&self, reg: RegId) -> Option<i64> {
        self.constant_values.get(&reg).copied()
    }

    /// 检查是否是2的幂
    fn is_power_of_two(&self, value: i64) -> Option<u8> {
        if value > 0 && (value & (value - 1)) == 0 {
            // 计算log2
            let mut shift = 0;
            let mut v = value;
            while v > 1 {
                v >>= 1;
                shift += 1;
            }
            Some(shift)
        } else {
            None
        }
    }

    /// 获取优化统计信息
    pub fn get_stats(&self) -> &OptimizationStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = OptimizationStats::default();
    }
}

impl Default for IROptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_folding() {
        let mut optimizer = IROptimizer::new();

        let ops = vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::MovImm { dst: 2, imm: 20 },
            IROp::Add {
                dst: 3,
                src1: 1,
                src2: 2,
            },
        ];

        let optimized = optimizer.optimize(&ops);

        // 应该折叠为常量
        assert!(
            optimized
                .iter()
                .any(|op| matches!(op, IROp::MovImm { dst: 3, imm: 30 }))
        );

        let stats = optimizer.get_stats();
        assert!(stats.constant_folds > 0);
    }

    #[test]
    fn test_dead_code_elimination() {
        let mut optimizer = IROptimizer::new();

        let ops = vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::MovImm { dst: 2, imm: 20 },
            IROp::Add {
                dst: 3,
                src1: 1,
                src2: 2,
            }, // r3 = r1 + r2
            IROp::MovImm { dst: 4, imm: 30 }, // r4未使用，应该被消除
        ];

        let optimized = optimizer.optimize(&ops);

        // r4的定义应该被消除
        assert!(
            !optimized
                .iter()
                .any(|op| matches!(op, IROp::MovImm { dst: 4, .. }))
        );

        let stats = optimizer.get_stats();
        assert!(stats.dead_code_eliminations > 0);
    }

    #[test]
    fn test_common_subexpression_elimination() {
        let mut optimizer = IROptimizer::new();

        let ops = vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::MovImm { dst: 2, imm: 20 },
            IROp::Add {
                dst: 3,
                src1: 1,
                src2: 2,
            }, // r3 = r1 + r2
            IROp::Add {
                dst: 4,
                src1: 1,
                src2: 2,
            }, // r4 = r1 + r2 (重复)
        ];

        let optimized = optimizer.optimize(&ops);

        // 第二个加法应该被替换为mov
        assert!(
            optimized
                .iter()
                .any(|op| matches!(op, IROp::Mov { dst: 4, src: 3 }))
        );

        let stats = optimizer.get_stats();
        assert!(stats.common_subexpression_eliminations > 0);
    }

    #[test]
    fn test_strength_reduction() {
        let mut optimizer = IROptimizer::new();

        let ops = vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::Mul {
                dst: 2,
                src1: 1,
                src2: 8,
            }, // 乘以8，应该转换为移位
        ];

        let optimized = optimizer.optimize(&ops);

        // 乘法应该被转换为移位
        assert!(optimized.iter().any(|op| matches!(
            op,
            IROp::SllImm {
                dst: 2,
                src: 1,
                sh: 3
            }
        )));

        let stats = optimizer.get_stats();
        assert!(stats.strength_reductions > 0);
    }
}
