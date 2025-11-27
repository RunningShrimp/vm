//! 多块编译支持模块
//!
//! 提供对多基本块和控制流的支持

use vm_ir::{IRBlock, Terminator, GuestAddr};
use std::collections::{HashMap, VecDeque};

/// 基本块信息
#[derive(Debug, Clone)]
pub struct BasicBlockInfo {
    pub pc: GuestAddr,
    pub successors: Vec<GuestAddr>,
    pub predecessors: Vec<GuestAddr>,
    pub block: IRBlock,
}

/// 控制流图 (CFG)
pub struct ControlFlowGraph {
    blocks: HashMap<GuestAddr, BasicBlockInfo>,
    entry: Option<GuestAddr>,
}

impl ControlFlowGraph {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            entry: None,
        }
    }

    /// 添加基本块
    pub fn add_block(&mut self, block: IRBlock) {
        let pc = block.start_pc;
        
        // 提取后继块
        let successors = Self::extract_successors(&block.term);
        
        self.blocks.insert(
            pc,
            BasicBlockInfo {
                pc,
                successors: successors.clone(),
                predecessors: Vec::new(),
                block,
            },
        );

        // 如果这是第一个块，设为入口
        if self.entry.is_none() {
            self.entry = Some(pc);
        }

        // 更新后继块的前驱信息
        for succ_pc in successors {
            if let Some(succ_info) = self.blocks.get_mut(&succ_pc) {
                if !succ_info.predecessors.contains(&pc) {
                    succ_info.predecessors.push(pc);
                }
            }
        }
    }

    /// 获取基本块
    pub fn get_block(&self, pc: GuestAddr) -> Option<&IRBlock> {
        self.blocks.get(&pc).map(|info| &info.block)
    }

    /// 拓扑排序块
    pub fn topological_order(&self) -> Vec<GuestAddr> {
        let mut visited = std::collections::HashSet::new();
        let mut order = Vec::new();
        
        if let Some(entry) = self.entry {
            self.dfs(entry, &mut visited, &mut order);
        }
        
        order
    }

    fn dfs(&self, pc: GuestAddr, visited: &mut std::collections::HashSet<GuestAddr>, order: &mut Vec<GuestAddr>) {
        if visited.contains(&pc) {
            return;
        }
        visited.insert(pc);

        if let Some(info) = self.blocks.get(&pc) {
            for &succ in &info.successors {
                self.dfs(succ, visited, order);
            }
        }

        order.push(pc);
    }

    /// 提取终结符的后继块地址
    fn extract_successors(term: &Terminator) -> Vec<GuestAddr> {
        match term {
            Terminator::Ret { .. } => Vec::new(),
            Terminator::Jmp { target } => vec![*target],
            Terminator::CondJmp {
                target_true,
                target_false,
                ..
            } => vec![*target_true, *target_false],
            Terminator::Call { ret_pc, .. } => vec![*ret_pc],
            Terminator::Fault { .. } | Terminator::Interrupt { .. } => Vec::new(),
            Terminator::JmpReg { .. } => Vec::new(), // 间接跳转，无法静态分析
        }
    }

    /// 获取块数量
    pub fn num_blocks(&self) -> usize {
        self.blocks.len()
    }

    /// 获取入口块
    pub fn entry(&self) -> Option<GuestAddr> {
        self.entry
    }

    /// 获取块的后继
    pub fn successors(&self, pc: GuestAddr) -> Option<&[GuestAddr]> {
        self.blocks.get(&pc).map(|info| info.successors.as_slice())
    }
}

/// Dominance 树构建
pub struct DominanceAnalysis {
    dominators: HashMap<GuestAddr, Vec<GuestAddr>>,
}

impl DominanceAnalysis {
    pub fn new(cfg: &ControlFlowGraph) -> Self {
        let mut da = Self {
            dominators: HashMap::new(),
        };
        da.compute(cfg);
        da
    }

    fn compute(&mut self, cfg: &ControlFlowGraph) {
        let all_nodes: Vec<_> = cfg.blocks.keys().cloned().collect();
        let entry = match cfg.entry() {
            Some(e) => e,
            None => return,
        };

        // 初始化支配集
        for &node in &all_nodes {
            if node == entry {
                self.dominators.insert(node, vec![node]);
            } else {
                self.dominators.insert(node, all_nodes.clone());
            }
        }

        // 不动点迭代
        let mut changed = true;
        while changed {
            changed = false;

            for &node in &all_nodes {
                if node == entry {
                    continue;
                }

                let preds: Vec<_> = cfg
                    .blocks
                    .get(&node)
                    .map(|info| info.predecessors.clone())
                    .unwrap_or_default();

                if preds.is_empty() {
                    continue;
                }

                // 计算前驱的支配集交集
                let mut new_dom = self.dominators[&preds[0]].clone();
                for &pred in &preds[1..] {
                    new_dom.retain(|x| self.dominators[&pred].contains(x));
                }
                new_dom.push(node);

                if new_dom != self.dominators[&node] {
                    self.dominators.insert(node, new_dom);
                    changed = true;
                }
            }
        }
    }

    pub fn dominates(&self, a: GuestAddr, b: GuestAddr) -> bool {
        self.dominators
            .get(&b)
            .map(|doms| doms.contains(&a))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::Terminator;

    #[test]
    fn test_cfg_construction() {
        let mut cfg = ControlFlowGraph::new();
        
        let block1 = IRBlock {
            start_pc: GuestAddr::from(0u64),
            ops: vec![],
            term: Terminator::Jmp {
                target: GuestAddr::from(0x100u64),
            },
        };
        
        let block2 = IRBlock {
            start_pc: GuestAddr::from(0x100u64),
            ops: vec![],
            term: Terminator::Ret { value: None },
        };

        cfg.add_block(block1);
        cfg.add_block(block2);

        assert_eq!(cfg.num_blocks(), 2);
        assert_eq!(cfg.entry(), Some(GuestAddr::from(0u64)));
    }

    #[test]
    fn test_topological_sort() {
        let mut cfg = ControlFlowGraph::new();
        
        let block1 = IRBlock {
            start_pc: GuestAddr::from(0u64),
            ops: vec![],
            term: Terminator::Jmp {
                target: GuestAddr::from(0x100u64),
            },
        };
        
        let block2 = IRBlock {
            start_pc: GuestAddr::from(0x100u64),
            ops: vec![],
            term: Terminator::Ret { value: None },
        };

        cfg.add_block(block1);
        cfg.add_block(block2);

        let order = cfg.topological_order();
        assert_eq!(order.len(), 2);
    }
}
