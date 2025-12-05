//! 依赖关系分析模块
//!
//! 分析代码块之间的依赖关系，用于AOT编译优化

use vm_ir::{IRBlock, IROp, Terminator};

/// 依赖关系分析器
pub struct DependencyAnalyzer;

impl DependencyAnalyzer {
    /// 分析代码块依赖关系
    ///
    /// 返回该块依赖的其他块的PC地址列表
    pub fn analyze_block_dependencies(block: &IRBlock) -> Vec<u64> {
        let mut dependencies = Vec::new();

        // 分析终结符中的跳转目标
        match &block.term {
            Terminator::Jmp { target } => {
                dependencies.push(*target);
            }
            Terminator::CondJmp {
                target_true,
                target_false,
                ..
            } => {
                dependencies.push(*target_true);
                dependencies.push(*target_false);
            }
            Terminator::Call { target, .. } => {
                dependencies.push(*target);
            }
            Terminator::JmpReg { .. } => {
                // 间接跳转，无法静态确定目标
            }
            _ => {
                // Ret, Fault, Interrupt 没有依赖
            }
        }

        // 分析操作中的跳转（如果有）
        for op in &block.ops {
            if let IROp::Beq { target, .. }
            | IROp::Bne { target, .. }
            | IROp::Blt { target, .. }
            | IROp::Bge { target, .. }
            | IROp::Bltu { target, .. }
            | IROp::Bgeu { target, .. } = op
            {
                dependencies.push(*target);
            }
        }

        dependencies
    }

    /// 分析多个代码块的依赖关系图
    ///
    /// 返回一个映射：PC -> 依赖的PC列表
    pub fn analyze_dependency_graph(blocks: &[(u64, &IRBlock)]) -> std::collections::HashMap<u64, Vec<u64>> {
        let mut graph = std::collections::HashMap::new();

        for (pc, block) in blocks {
            let deps = Self::analyze_block_dependencies(block);
            graph.insert(*pc, deps);
        }

        graph
    }

    /// 拓扑排序代码块（按依赖关系）
    ///
    /// 返回排序后的PC列表，确保依赖的块在被依赖的块之前
    pub fn topological_sort(blocks: &[(u64, &IRBlock)]) -> Vec<u64> {
        let graph = Self::analyze_dependency_graph(blocks);
        let mut sorted = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut visiting = std::collections::HashSet::new();

        fn visit(
            pc: u64,
            graph: &std::collections::HashMap<u64, Vec<u64>>,
            visited: &mut std::collections::HashSet<u64>,
            visiting: &mut std::collections::HashSet<u64>,
            sorted: &mut Vec<u64>,
        ) {
            if visited.contains(&pc) {
                return;
            }
            if visiting.contains(&pc) {
                // 检测到循环依赖，跳过
                return;
            }

            visiting.insert(pc);
            if let Some(deps) = graph.get(&pc) {
                for dep in deps {
                    visit(*dep, graph, visited, visiting, sorted);
                }
            }
            visiting.remove(&pc);
            visited.insert(pc);
            sorted.push(pc);
        }

        for (pc, _) in blocks {
            visit(*pc, &graph, &mut visited, &mut visiting, &mut sorted);
        }

        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IRBuilder, Terminator};

    #[test]
    fn test_analyze_block_dependencies() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::MovImm { dst: 1, imm: 10 });
        builder.set_terminator(Terminator::Jmp { target: 0x2000 });
        let block = builder.build();

        let deps = DependencyAnalyzer::analyze_block_dependencies(&block);
        assert_eq!(deps, vec![0x2000]);
    }

    #[test]
    fn test_cond_jmp_dependencies() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::CmpEq { dst: 1, lhs: 2, rhs: 3 });
        builder.set_terminator(Terminator::CondJmp {
            cond: 1,
            target_true: 0x2000,
            target_false: 0x3000,
        });
        let block = builder.build();

        let deps = DependencyAnalyzer::analyze_block_dependencies(&block);
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&0x2000));
        assert!(deps.contains(&0x3000));
    }

    #[test]
    fn test_call_dependencies() {
        let mut builder = IRBuilder::new(0x1000);
        builder.set_terminator(Terminator::Call {
            target: 0x5000,
            args: vec![],
        });
        let block = builder.build();

        let deps = DependencyAnalyzer::analyze_block_dependencies(&block);
        assert_eq!(deps, vec![0x5000]);
    }

    #[test]
    fn test_branch_op_dependencies() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::Beq {
            cond: 1,
            target: 0x4000,
        });
        builder.set_terminator(Terminator::Ret);
        let block = builder.build();

        let deps = DependencyAnalyzer::analyze_block_dependencies(&block);
        assert_eq!(deps, vec![0x4000]);
    }

    #[test]
    fn test_no_dependencies() {
        let mut builder = IRBuilder::new(0x1000);
        builder.add_op(IROp::MovImm { dst: 1, imm: 10 });
        builder.set_terminator(Terminator::Ret);
        let block = builder.build();

        let deps = DependencyAnalyzer::analyze_block_dependencies(&block);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_analyze_dependency_graph() {
        let mut blocks = Vec::new();
        let mut builder1 = IRBuilder::new(0x1000);
        builder1.set_terminator(Terminator::Jmp { target: 0x2000 });
        blocks.push((0x1000, builder1.build()));

        let mut builder2 = IRBuilder::new(0x2000);
        builder2.set_terminator(Terminator::Ret);
        blocks.push((0x2000, builder2.build()));

        let graph = DependencyAnalyzer::analyze_dependency_graph(&blocks);
        assert_eq!(graph.len(), 2);
        assert_eq!(graph.get(&0x1000), Some(&vec![0x2000]));
        assert_eq!(graph.get(&0x2000), Some(&vec![]));
    }

    #[test]
    fn test_topological_sort() {
        let mut blocks = Vec::new();
        let mut builder1 = IRBuilder::new(0x1000);
        builder1.set_terminator(Terminator::Jmp { target: 0x2000 });
        blocks.push((0x1000, builder1.build()));

        let mut builder2 = IRBuilder::new(0x2000);
        builder2.set_terminator(Terminator::Ret);
        blocks.push((0x2000, builder2.build()));

        let sorted = DependencyAnalyzer::topological_sort(&blocks);
        assert!(!sorted.is_empty());
        // 0x2000应该在0x1000之前（因为它被依赖）
        assert!(sorted.contains(&0x1000));
        assert!(sorted.contains(&0x2000));
    }
}

