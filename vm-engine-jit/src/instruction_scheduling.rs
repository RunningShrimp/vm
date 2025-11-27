//! 指令调度和循环展开优化模块
//!
//! 实现指令级并行、循环展开、资源分配等优化

use std::collections::{HashMap, VecDeque};
use vm_ir::{IROp, RegId};

/// 指令依赖关系
#[derive(Debug, Clone)]
pub struct InstructionDependency {
    /// 源寄存器
    pub source_regs: Vec<RegId>,
    /// 目标寄存器
    pub dest_reg: Option<RegId>,
    /// 延迟周期数
    pub latency: u32,
}

/// 指令调度器
pub struct InstructionScheduler {
    /// 机器模型
    machine_model: MachineModel,
}

impl InstructionScheduler {
    /// 创建新调度器
    pub fn new(machine_model: MachineModel) -> Self {
        Self { machine_model }
    }

    /// 分析指令依赖
    pub fn analyze_dependencies(ops: &[IROp]) -> Vec<InstructionDependency> {
        let mut deps = Vec::new();

        for op in ops {
            let dep = InstructionDependency {
                source_regs: extract_source_regs(op),
                dest_reg: extract_dest_reg(op),
                latency: estimate_latency(op),
            };
            deps.push(dep);
        }

        deps
    }

    /// 构建依赖图并优化指令顺序
    pub fn schedule(&self, ops: &[IROp]) -> Vec<usize> {
        let deps = Self::analyze_dependencies(ops);
        let mut scheduled = Vec::new();
        let mut ready = Vec::new();
        let mut waiting: VecDeque<usize> = (0..ops.len()).collect();
        let mut reg_ready_time: HashMap<RegId, u32> = HashMap::new();
        let mut current_time = 0u32;

        // 初始化：没有依赖的指令可以立即执行
        while let Some(idx) = waiting.pop_front() {
            if deps[idx].source_regs.is_empty()
                || deps[idx]
                    .source_regs
                    .iter()
                    .all(|reg| !reg_ready_time.contains_key(reg))
            {
                ready.push(idx);
            } else {
                waiting.push_back(idx);
            }
        }

        // 调度循环
        while !ready.is_empty() || !waiting.is_empty() {
            if ready.is_empty() {
                // 时间推进到下一个就绪指令
                if let Some(idx) = waiting.pop_front() {
                    let min_ready_time = deps[idx]
                        .source_regs
                        .iter()
                        .filter_map(|reg| reg_ready_time.get(reg))
                        .max()
                        .copied()
                        .unwrap_or(0);
                    current_time = min_ready_time;
                    ready.push(idx);
                }
            } else {
                // 选择最高优先级的就绪指令
                let idx = Self::select_best_instruction(&ready, &deps);
                ready.remove(
                    ready
                        .iter()
                        .position(|&i| i == idx)
                        .unwrap(),
                );

                scheduled.push(idx);

                // 更新寄存器就绪时间
                if let Some(dest) = deps[idx].dest_reg {
                    reg_ready_time.insert(dest, current_time + deps[idx].latency);
                }

                // 检查等待队列中是否有新的就绪指令
                let mut newly_ready = Vec::new();
                for (i, &waiting_idx) in waiting.iter().enumerate() {
                    if deps[waiting_idx]
                        .source_regs
                        .iter()
                        .all(|reg| {
                            reg_ready_time
                                .get(reg)
                                .map(|&t| t <= current_time)
                                .unwrap_or(true)
                        })
                    {
                        newly_ready.push(i);
                    }
                }

                // 反向移除以保持索引有效性
                for i in newly_ready.iter().rev() {
                    let idx = waiting.remove(*i).unwrap();
                    ready.push(idx);
                }

                current_time += 1;
            }
        }

        scheduled
    }

    /// 选择最佳指令（使用启发式方法）
    fn select_best_instruction(candidates: &[usize], deps: &[InstructionDependency]) -> usize {
        candidates
            .iter()
            .max_by_key(|&&idx| {
                // 优先级：
                // 1. 高延迟的指令（关键路径）
                // 2. 有多个依赖者的指令
                deps[idx].latency
            })
            .copied()
            .unwrap_or(candidates[0])
    }
}

/// 循环展开优化器
pub struct LoopUnroller {
    /// 最大展开因子
    max_unroll_factor: u32,
}

impl LoopUnroller {
    /// 创建新的展开器
    pub fn new(max_unroll_factor: u32) -> Self {
        Self { max_unroll_factor }
    }

    /// 计算最优展开因子
    pub fn compute_unroll_factor(&self, loop_body_size: usize, loop_count: u64) -> u32 {
        // 展开因子受以下限制：
        // 1. 最大展开因子限制
        // 2. 代码膨胀限制（通常为原始大小的 3-4 倍）
        // 3. 指令缓存限制

        let max_by_size = std::cmp::min(
            self.max_unroll_factor,
            (4096 / loop_body_size) as u32, // 假设 4KB 指令缓存
        );

        // 如果循环次数很少，完全展开
        if loop_count <= 8 {
            loop_count as u32
        } else {
            max_by_size
        }
    }

    /// 检查是否值得展开
    pub fn should_unroll(&self, loop_body_size: usize, trip_count: u64) -> bool {
        // 展开的收益来自：
        // 1. 减少循环控制指令
        // 2. 增加指令级并行性
        // 3. 更好的分支预测

        // 规则：
        // - 循环体较小且执行多次 -> 展开
        // - 循环次数已知且有限 -> 展开
        loop_body_size < 32 && trip_count > 4
    }

    /// 生成展开的循环体
    pub fn unroll(&self, ops: &[IROp], factor: u32) -> Vec<IROp> {
        let mut unrolled = Vec::new();

        for _ in 0..factor {
            unrolled.extend_from_slice(ops);
        }

        unrolled
    }
}

/// 机器模型
#[derive(Debug, Clone)]
pub struct MachineModel {
    /// 指令延迟表
    pub latencies: HashMap<String, u32>,
    /// 资源冲突
    pub resources: Vec<Resource>,
}

impl MachineModel {
    /// 创建默认的现代 CPU 机器模型
    pub fn default_modern_cpu() -> Self {
        let mut latencies = HashMap::new();
        latencies.insert("add".to_string(), 1);
        latencies.insert("mul".to_string(), 3);
        latencies.insert("div".to_string(), 10);
        latencies.insert("load".to_string(), 4);
        latencies.insert("store".to_string(), 2);
        latencies.insert("fadd".to_string(), 4);
        latencies.insert("fmul".to_string(), 5);

        Self {
            latencies,
            resources: vec![
                Resource {
                    name: "ALU".to_string(),
                    count: 4,
                },
                Resource {
                    name: "FPU".to_string(),
                    count: 2,
                },
                Resource {
                    name: "MemUnit".to_string(),
                    count: 2,
                },
            ],
        }
    }
}

/// 处理单元
#[derive(Debug, Clone)]
pub struct Resource {
    /// 资源名称
    pub name: String,
    /// 可用数量
    pub count: u32,
}

// 辅助函数
fn extract_source_regs(op: &IROp) -> Vec<RegId> {
    match op {
        IROp::Add { src1, src2, .. }
        | IROp::Sub { src1, src2, .. }
        | IROp::Mul { src1, src2, .. } => vec![*src1, *src2],
        IROp::Div { src1, src2, .. } | IROp::Rem { src1, src2, .. } => vec![*src1, *src2],
        IROp::Load { src, .. } => vec![*src],
        IROp::Store { src, addr, .. } => vec![*src, *addr],
        _ => Vec::new(),
    }
}

fn extract_dest_reg(op: &IROp) -> Option<RegId> {
    match op {
        IROp::Add { dst, .. }
        | IROp::Sub { dst, .. }
        | IROp::Mul { dst, .. }
        | IROp::Div { dst, .. }
        | IROp::Rem { dst, .. }
        | IROp::Load { dst, .. } => Some(*dst),
        _ => None,
    }
}

fn estimate_latency(op: &IROp) -> u32 {
    match op {
        IROp::Add { .. } | IROp::Sub { .. } => 1,
        IROp::Mul { .. } => 3,
        IROp::Div { .. } => 10,
        IROp::Load { .. } => 4,
        IROp::Store { .. } => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_machine_model() {
        let model = MachineModel::default_modern_cpu();
        assert_eq!(model.latencies.get("add"), Some(&1));
        assert_eq!(model.latencies.get("mul"), Some(&3));
    }

    #[test]
    fn test_loop_unroll_factor() {
        let unroller = LoopUnroller::new(8);
        let factor = unroller.compute_unroll_factor(16, 100);
        assert!(factor > 0 && factor <= 8);
    }

    #[test]
    fn test_should_unroll() {
        let unroller = LoopUnroller::new(8);
        assert!(unroller.should_unroll(16, 10));
        assert!(!unroller.should_unroll(64, 2));
    }
}
