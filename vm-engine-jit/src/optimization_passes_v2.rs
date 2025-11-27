//! 性能优化 Pass 模块 v2
//!
//! 实现多个优化通道：常量折叠、强度削弱、死代码消除等

use vm_ir::{IROp, RegId};
use std::collections::HashMap;

/// 优化 Pass 配置
#[derive(Debug, Clone)]
pub struct OptimizationPassConfig {
    /// 启用常量折叠
    pub enable_constant_folding: bool,
    /// 启用强度削弱
    pub enable_strength_reduction: bool,
    /// 启用死代码消除
    pub enable_dce: bool,
    /// 启用公共子表达式消除
    pub enable_cse: bool,
    /// 启用循环不变式提升
    pub enable_licm: bool,
    /// 优化级别 (0-3)
    pub opt_level: u32,
}

impl Default for OptimizationPassConfig {
    fn default() -> Self {
        Self {
            enable_constant_folding: true,
            enable_strength_reduction: true,
            enable_dce: true,
            enable_cse: true,
            enable_licm: false,
            opt_level: 2,
        }
    }
}

/// 常量值表示
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConstantValue {
    I64(i64),
    U64(u64),
    F64(f64),
    Unknown,
}

/// 常量折叠优化
pub struct ConstantFolder {
    /// 常量值缓存
    values: HashMap<RegId, ConstantValue>,
}

impl ConstantFolder {
    /// 创建新的常量文件夹
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// 尝试折叠操作
    pub fn fold(&mut self, op: &IROp) -> Option<IROp> {
        match op {
            IROp::Add { dst, src1, src2 } => {
                if let (Some(ConstantValue::I64(v1)), Some(ConstantValue::I64(v2))) =
                    (self.values.get(src1), self.values.get(src2))
                {
                    let result = v1 + v2;
                    self.values.insert(*dst, ConstantValue::I64(result));
                    Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    })
                } else {
                    None
                }
            }
            IROp::Sub { dst, src1, src2 } => {
                if let (Some(ConstantValue::I64(v1)), Some(ConstantValue::I64(v2))) =
                    (self.values.get(src1), self.values.get(src2))
                {
                    let result = v1 - v2;
                    self.values.insert(*dst, ConstantValue::I64(result));
                    Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    })
                } else {
                    None
                }
            }
            IROp::Mul { dst, src1, src2 } => {
                if let (Some(ConstantValue::I64(v1)), Some(ConstantValue::I64(v2))) =
                    (self.values.get(src1), self.values.get(src2))
                {
                    let result = v1 * v2;
                    self.values.insert(*dst, ConstantValue::I64(result));
                    Some(IROp::MovImm {
                        dst: *dst,
                        imm: result as u64,
                    })
                } else {
                    None
                }
            }
            IROp::MovImm { dst, imm } => {
                self.values.insert(*dst, ConstantValue::U64(*imm));
                None
            }
            _ => None,
        }
    }

    /// 查询常量值
    pub fn get_constant(&self, reg: RegId) -> Option<ConstantValue> {
        self.values.get(&reg).copied()
    }
}

/// 强度削弱优化
pub struct StrengthReducer;

impl StrengthReducer {
    /// 检查是否为 2 的幂
    fn is_power_of_two(n: u64) -> bool {
        n > 0 && (n & (n - 1)) == 0
    }

    /// 尝试削弱操作
    pub fn reduce(op: &IROp, constant_map: &HashMap<RegId, u64>) -> Option<IROp> {
        match op {
            // 乘以 2 的幂 -> 左移
            IROp::Mul { dst, src1, src2 } => {
                if let Some(&imm) = constant_map.get(src2) {
                    if Self::is_power_of_two(imm) {
                        let shift = imm.trailing_zeros();
                        return Some(IROp::Sll {
                            dst: *dst,
                            src: *src1,
                            imm: shift,
                        });
                    }
                }
                None
            }
            // 除以 2 的幂 -> 右移
            IROp::Div { dst, src1, src2 } => {
                if let Some(&imm) = constant_map.get(src2) {
                    if Self::is_power_of_two(imm) {
                        let shift = imm.trailing_zeros();
                        return Some(IROp::Srl {
                            dst: *dst,
                            src: *src1,
                            imm: shift,
                        });
                    }
                }
                None
            }
            _ => None,
        }
    }
}

/// 公共子表达式消除 (CSE)
pub struct CommonSubexpressionEliminator {
    /// 表达式签名到目标寄存器的映射
    expr_map: HashMap<String, RegId>,
}

impl CommonSubexpressionEliminator {
    /// 创建新的 CSE
    pub fn new() -> Self {
        Self {
            expr_map: HashMap::new(),
        }
    }

    /// 获取表达式签名
    fn get_signature(op: &IROp) -> Option<String> {
        match op {
            IROp::Add { src1, src2, .. } => {
                Some(format!("add:{:?}:{:?}", src1, src2))
            }
            IROp::Sub { src1, src2, .. } => {
                Some(format!("sub:{:?}:{:?}", src1, src2))
            }
            IROp::Mul { src1, src2, .. } => {
                Some(format!("mul:{:?}:{:?}", src1, src2))
            }
            _ => None,
        }
    }

    /// 尝试消除公共子表达式
    pub fn try_eliminate(&mut self, op: &IROp) -> Option<RegId> {
        if let Some(sig) = Self::get_signature(op) {
            if let Some(&dst) = self.expr_map.get(&sig) {
                return Some(dst);
            }

            if let Some(dst) = Self::get_dest(op) {
                self.expr_map.insert(sig, dst);
            }
        }
        None
    }

    fn get_dest(op: &IROp) -> Option<RegId> {
        match op {
            IROp::Add { dst, .. }
            | IROp::Sub { dst, .. }
            | IROp::Mul { dst, .. } => Some(*dst),
            _ => None,
        }
    }
}

/// 循环不变式提升 (LICM)
pub struct LoopInvariantCodeMotion;

impl LoopInvariantCodeMotion {
    /// 检查操作是否为循环不变量
    pub fn is_loop_invariant(op: &IROp, loop_regs: &[RegId]) -> bool {
        Self::uses_loop_var(op, loop_regs) == false && Self::has_no_side_effects(op)
    }

    /// 检查是否使用循环变量
    fn uses_loop_var(op: &IROp, regs: &[RegId]) -> bool {
        match op {
            IROp::Add { src1, src2, .. }
            | IROp::Sub { src1, src2, .. }
            | IROp::Mul { src1, src2, .. } => {
                regs.contains(src1) || regs.contains(src2)
            }
            _ => false,
        }
    }

    /// 检查是否有副作用
    fn has_no_side_effects(op: &IROp) -> bool {
        !matches!(op, IROp::Store { .. } | IROp::AtomicRMW { .. })
    }
}

/// 优化管道
pub struct OptimizationPipeline {
    config: OptimizationPassConfig,
}

impl OptimizationPipeline {
    /// 创建新的优化管道
    pub fn new(config: OptimizationPassConfig) -> Self {
        Self { config }
    }

    /// 执行优化
    pub fn optimize(&self, ops: &[IROp]) -> Vec<IROp> {
        let mut optimized = ops.to_vec();

        if self.config.enable_constant_folding {
            optimized = self.apply_constant_folding(optimized);
        }

        if self.config.enable_strength_reduction {
            optimized = self.apply_strength_reduction(optimized);
        }

        if self.config.enable_cse {
            optimized = self.apply_cse(optimized);
        }

        optimized
    }

    fn apply_constant_folding(&self, ops: Vec<IROp>) -> Vec<IROp> {
        let mut folder = ConstantFolder::new();
        let mut result = Vec::new();

        for op in ops {
            if let Some(folded) = folder.fold(&op) {
                result.push(folded);
            } else {
                result.push(op);
            }
        }

        result
    }

    fn apply_strength_reduction(&self, ops: Vec<IROp>) -> Vec<IROp> {
        let mut constants = HashMap::new();

        // 首先收集常数
        for op in &ops {
            if let IROp::MovImm { dst, imm } = op {
                constants.insert(*dst, *imm);
            }
        }

        let mut result = Vec::new();
        for op in ops {
            if let Some(reduced) = StrengthReducer::reduce(&op, &constants) {
                result.push(reduced);
            } else {
                result.push(op);
            }
        }

        result
    }

    fn apply_cse(&self, ops: Vec<IROp>) -> Vec<IROp> {
        let mut cse = CommonSubexpressionEliminator::new();
        let mut result = Vec::new();

        for op in ops {
            if let Some(prev_dst) = cse.try_eliminate(&op) {
                // 重用之前的结果
                if let Some(dst) = CommonSubexpressionEliminator::get_dest(&op) {
                    result.push(IROp::MovImm {
                        dst,
                        imm: prev_dst.0 as u64,
                    });
                } else {
                    result.push(op);
                }
            } else {
                result.push(op);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_folder() {
        let mut folder = ConstantFolder::new();
        let val = ConstantValue::I64(42);
        folder.values.insert(RegId(0), val);

        assert_eq!(folder.get_constant(RegId(0)), Some(val));
    }

    #[test]
    fn test_strength_reduction() {
        assert!(StrengthReducer::is_power_of_two(8));
        assert!(StrengthReducer::is_power_of_two(16));
        assert!(!StrengthReducer::is_power_of_two(7));
    }

    #[test]
    fn test_optimization_pipeline_config() {
        let config = OptimizationPassConfig::default();
        assert!(config.enable_constant_folding);
        assert_eq!(config.opt_level, 2);
    }
}
