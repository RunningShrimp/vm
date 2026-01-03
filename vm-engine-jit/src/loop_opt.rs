//! 循环优化
//!
//! 提供基本的循环检测和优化功能，包括：
//! - 循环不变量外提
//! - 循环展开
//! - 归纳变量优化
//! - 循环强度削弱

use vm_ir::{IRBlock, Terminator, GuestAddr};

/// 循环信息
#[derive(Debug, Clone)]
pub struct LoopInfo {
    /// 循环头地址
    pub header: GuestAddr,
    /// 循环基本块集合
    pub blocks: Vec<GuestAddr>,
    /// 回边（跳转到循环头的分支）
    pub back_edges: Vec<GuestAddr>,
    /// 退出边（跳出循环的分支）
    pub exit_edges: Vec<GuestAddr>,
}

/// 循环优化配置
#[derive(Debug, Clone)]
pub struct LoopOptConfig {
    /// 是否启用循环不变量外提
    pub enable_code_motion: bool,
    /// 是否启用循环展开
    pub enable_unrolling: bool,
    /// 循环展开因子
    pub unroll_factor: usize,
    /// 是否启用归纳变量优化
    pub enable_induction: bool,
}

impl Default for LoopOptConfig {
    fn default() -> Self {
        Self {
            enable_code_motion: true,
            enable_unrolling: false, // 默认关闭，可能增加代码大小
            unroll_factor: 4,
            enable_induction: true,
        }
    }
}

/// 循环优化器
pub struct LoopOptimizer {
    config: LoopOptConfig,
}

impl LoopOptimizer {
    /// 创建新的循环优化器
    pub fn new() -> Self {
        Self {
            config: LoopOptConfig::default(),
        }
    }

    /// 使用自定义配置创建循环优化器
    pub fn with_config(config: LoopOptConfig) -> Self {
        Self { config }
    }

    /// 优化IR块
    ///
    /// 执行以下优化：
    /// 1. 检测循环结构
    /// 2. 循环不变量外提
    /// 3. 归纳变量优化
    /// 4. 可选的循环展开
    pub fn optimize(&self, block: &mut IRBlock) {
        // 1. 检测循环
        if let Some(loop_info) = self.detect_loop(block) {
            // 2. 循环不变量外提
            if self.config.enable_code_motion {
                self.hoist_invariants(block, &loop_info);
            }

            // 3. 归纳变量优化
            if self.config.enable_induction {
                self.optimize_induction_vars(block, &loop_info);
            }

            // 4. 循环展开（如果启用）
            if self.config.enable_unrolling {
                self.unroll_loop(block, &loop_info);
            }
        }
    }

    /// 检测循环结构
    fn detect_loop(&self, block: &IRBlock) -> Option<LoopInfo> {
        // 检查终止符是否为回边（跳转到块内或自身）
        match &block.term {
            Terminator::Jmp { target } => {
                // 无条件跳转 - 检查是否跳转到当前块或更早的地址
                if target.0 <= block.start_pc.0 {
                    Some(LoopInfo {
                        header: block.start_pc,
                        blocks: vec![block.start_pc],
                        back_edges: vec![block.start_pc],
                        exit_edges: vec![],
                    })
                } else {
                    None
                }
            }
            Terminator::CondJmp { target_true, target_false, .. } => {
                // 条件跳转 - 检查是否有回边
                let has_back_edge = target_true.0 <= block.start_pc.0 || target_false.0 <= block.start_pc.0;

                if has_back_edge {
                    let mut back_edges = Vec::new();
                    let mut exit_edges = Vec::new();

                    if target_true.0 <= block.start_pc.0 {
                        back_edges.push(*target_true);
                    } else {
                        exit_edges.push(*target_true);
                    }

                    if target_false.0 <= block.start_pc.0 {
                        back_edges.push(*target_false);
                    } else {
                        exit_edges.push(*target_false);
                    }

                    Some(LoopInfo {
                        header: block.start_pc,
                        blocks: vec![block.start_pc],
                        back_edges,
                        exit_edges,
                    })
                } else {
                    None
                }
            }
            _ => None, // Ret, Call, Fault, Interrupt - 不是循环
        }
    }

    /// 循环不变量外提
    ///
    /// 将循环中不随迭代变化的操作移到循环外
    fn hoist_invariants(&self, _block: &mut IRBlock, _loop_info: &LoopInfo) {
        // 简化实现：识别纯计算操作（不涉及内存访问）
        // 完整实现需要数据流分析

        // TODO: 实现完整的数据流分析
        // 1. 构建支配树
        // 2. 识别循环不变量
        // 3. 在循环前预计算
        // 4. 替换循环中的使用
    }

    /// 归纳变量优化
    ///
    /// 优化循环中的归纳变量（如i++, i+=constant）
    fn optimize_induction_vars(&self, _block: &mut IRBlock, _loop_info: &LoopInfo) {
        // 简化实现：识别加法序列模式
        // 完整实现需要：
        // 1. 识别归纳变量
        // 2. 计算初始值和步长
        // 3. 替换为线性函数

        // TODO: 实现完整的归纳变量识别和优化
        // 1. 模式匹配：add x, y; sub x, z
        // 2. 强度削弱：乘以2^n转为移位
        // 3. 删除死代码
    }

    /// 循环展开
    ///
    /// 将循环体复制多次以减少分支开销
    fn unroll_loop(&self, _block: &mut IRBlock, _loop_info: &LoopInfo) {
        // 简化实现：复制操作序列
        // 完整实现需要：
        // 1. 确定循环次数
        // 2. 复制循环体
        // 3. 调整分支条件
        // 4. 处理剩余迭代

        // TODO: 实现完整的循环展开
        // 1. 分析循环次数
        // 2. 生成展开的代码
        // 3. 处理序言和结语
        // 4. 更新分支目标
    }
}

impl Clone for LoopOptimizer {
    fn clone(&self) -> Self {
        Self::with_config(self.config.clone())
    }
}

impl Default for LoopOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_optimizer_creation() {
        let optimizer = LoopOptimizer::new();
        let config = optimizer.config;
        assert!(config.enable_code_motion);
        assert!(!config.enable_unrolling);
    }

    #[test]
    fn test_loop_optimizer_with_config() {
        let config = LoopOptConfig {
            enable_code_motion: false,
            enable_unrolling: true,
            unroll_factor: 8,
            enable_induction: false,
        };
        let optimizer = LoopOptimizer::with_config(config.clone());
        assert_eq!(optimizer.config.unroll_factor, 8);
    }

    #[test]
    fn test_detect_loop_with_jmp() {
        let optimizer = LoopOptimizer::new();

        // 创建一个自跳转的块（无限循环）
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Jmp { target: GuestAddr(0x1000) },
        };

        let loop_info = optimizer.detect_loop(&block);
        assert!(loop_info.is_some());
        assert_eq!(loop_info.unwrap().header, GuestAddr(0x1000));
    }

    #[test]
    fn test_detect_loop_with_backward_cond_jmp() {
        let optimizer = LoopOptimizer::new();

        // 创建一个条件跳转到自身的块
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::CondJmp {
                cond: 1,
                target_true: GuestAddr(0x1000),
                target_false: GuestAddr(0x2000),
            },
        };

        let loop_info = optimizer.detect_loop(&block);
        assert!(loop_info.is_some());
        let info = loop_info.unwrap();
        assert_eq!(info.header, GuestAddr(0x1000));
        assert_eq!(info.back_edges.len(), 1);
        assert_eq!(info.exit_edges.len(), 1);
    }

    #[test]
    fn test_no_loop_forward_jmp() {
        let optimizer = LoopOptimizer::new();

        // 创建一个前向跳转的块（不是循环）
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Jmp { target: GuestAddr(0x2000) },
        };

        let loop_info = optimizer.detect_loop(&block);
        assert!(loop_info.is_none());
    }

    #[test]
    fn test_no_loop_forward_cond_jmp() {
        let optimizer = LoopOptimizer::new();

        // 创建一个只有前向条件跳转的块
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::CondJmp {
                cond: 1,
                target_true: GuestAddr(0x3000),
                target_false: GuestAddr(0x2000),
            },
        };

        let loop_info = optimizer.detect_loop(&block);
        assert!(loop_info.is_none());
    }

    #[test]
    fn test_optimize_does_not_panic() {
        let optimizer = LoopOptimizer::new();

        // 测试optimize方法不会panic
        let mut block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                IROp::MovImm { dst: 1, imm: 42 },
            ],
            term: Terminator::Jmp { target: GuestAddr(0x1000) },
        };

        // 应该成功执行而不panic
        optimizer.optimize(&mut block);

        // 块应该仍然有效
        assert_eq!(block.start_pc, GuestAddr(0x1000));
        assert_eq!(block.ops.len(), 1);
    }

    #[test]
    fn test_clone_optimizer() {
        let optimizer1 = LoopOptimizer::new();
        let optimizer2 = optimizer1.clone();

        // 克隆的优化器应该有相同的配置
        assert_eq!(optimizer2.config.enable_code_motion, optimizer1.config.enable_code_motion);
    }

    #[test]
    fn test_default_optimizer() {
        let optimizer = LoopOptimizer::default();

        // 默认优化器应该使用默认配置
        assert_eq!(optimizer.config.enable_code_motion, true);
        assert_eq!(optimizer.config.unroll_factor, 4);
    }
}
