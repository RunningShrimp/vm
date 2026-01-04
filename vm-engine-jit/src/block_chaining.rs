//! 基本块链接优化
//!
//! 将多个连续的基本块链接在一起，减少间接跳转开销。
//!
//! ## 功能
//!
//! - 基本块链接：识别可以链接的连续块
//! - 跳转优化：将直接跳转转换为块链接
//! - 热路径识别：识别执行频率高的路径优先链接
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use vm_engine_jit::block_chaining::BlockChainer;
//! use vm_ir::IRBlock;
//!
//! let mut chainer = BlockChainer::new();
//!
//! let block1 = IRBlock { /* ... */ };
//! let block2 = IRBlock { /* ... */ };
//!
//! // 链接块
//! chainer.analyze_block(&block1);
//! chainer.analyze_block(&block2);
//! chainer.build_chains();
//!
//! // 获取链接后的块序列
//! let chain = chainer.get_chain(block1.start_pc);
//! ```

use crate::compiler_backend::CompilerError;
use std::collections::{HashMap, HashSet};
use vm_core::GuestAddr;
use vm_ir::{IRBlock, Terminator};

/// 链接关系
#[derive(Debug, Clone)]
pub struct ChainLink {
    /// 源块地址
    pub from: GuestAddr,
    /// 目标块地址
    pub to: GuestAddr,
    /// 链接类型
    pub link_type: ChainType,
    /// 执行频率（用于热路径识别）
    pub frequency: u32,
    /// 是否已优化
    pub optimized: bool,
}

/// 链接类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainType {
    /// 直接跳转（无条件）
    Direct,
    /// 条件分支
    Conditional,
    /// 函数调用
    Call,
    /// 函数返回
    Return,
}

/// 块链
#[derive(Debug, Clone)]
pub struct BlockChain {
    /// 起始块地址
    pub start: GuestAddr,
    /// 链接的块序列
    pub blocks: Vec<GuestAddr>,
    /// 总执行频率
    pub frequency: u32,
}

/// 基本块链接器
pub struct BlockChainer {
    /// 所有链接关系
    links: HashMap<(GuestAddr, GuestAddr), ChainLink>,
    /// 块链（起始地址 -> 链）
    chains: HashMap<GuestAddr, BlockChain>,
    /// 块频率统计
    block_frequencies: HashMap<GuestAddr, u32>,
    /// 最大链长度
    max_chain_length: usize,
    /// 是否启用热路径优化
    enable_hot_path: bool,
}

impl BlockChainer {
    /// 创建新的块链接器
    pub fn new() -> Self {
        Self {
            links: HashMap::new(),
            chains: HashMap::new(),
            block_frequencies: HashMap::new(),
            max_chain_length: 16,
            enable_hot_path: true,
        }
    }

    /// 创建带配置的块链接器
    pub fn with_config(max_chain_length: usize, enable_hot_path: bool) -> Self {
        Self {
            links: HashMap::new(),
            chains: HashMap::new(),
            block_frequencies: HashMap::new(),
            max_chain_length,
            enable_hot_path,
        }
    }

    /// 分析基本块并记录潜在的链接关系
    pub fn analyze_block(&mut self, block: &IRBlock) -> Result<(), CompilerError> {
        let block_addr = block.start_pc;

        // 更新块频率
        *self.block_frequencies.entry(block_addr).or_insert(0) += 1;

        // 分析终结符以识别链接关系
        match &block.term {
            Terminator::Jmp { target } => {
                // 直接跳转 - 可以链接
                self.add_link(block_addr, *target, ChainType::Direct);
            }
            Terminator::CondJmp {
                cond: _,
                target_true,
                target_false,
            } => {
                // 条件分支 - 两个分支都可能链接
                self.add_link(block_addr, *target_true, ChainType::Conditional);
                self.add_link(block_addr, *target_false, ChainType::Conditional);
            }
            Terminator::Call { target, ret_pc: _ } => {
                // 函数调用 - 可以链接到被调用函数
                self.add_link(block_addr, *target, ChainType::Call);
            }
            Terminator::Ret => {
                // 函数返回 - 不创建链接
            }
            Terminator::JmpReg { .. } => {
                // 间接跳转 - 无法静态分析，不创建链接
            }
            Terminator::Fault { .. } | Terminator::Interrupt { .. } => {
                // 异常终止 - 不创建链接
            }
        }

        Ok(())
    }

    /// 添加链接关系
    fn add_link(&mut self, from: GuestAddr, to: GuestAddr, link_type: ChainType) {
        let key = (from, to);
        let entry = self.links.entry(key).or_insert(ChainLink {
            from,
            to,
            link_type,
            frequency: 0,
            optimized: false,
        });
        entry.frequency += 1;
    }

    /// 构建块链
    pub fn build_chains(&mut self) {
        // 清空现有链
        self.chains.clear();

        // 获取所有块起始地址
        let mut start_blocks: Vec<GuestAddr> = self.links.keys().map(|(from, _)| *from).collect();

        // 按频率排序（热路径优先）
        if self.enable_hot_path {
            start_blocks.sort_by(|a, b| {
                let freq_a = self.block_frequencies.get(a).copied().unwrap_or(0);
                let freq_b = self.block_frequencies.get(b).copied().unwrap_or(0);
                freq_b.cmp(&freq_a)
            });
        }

        // 为每个起始块构建链
        for start in start_blocks {
            if self.chains.contains_key(&start) {
                continue; // 已经作为链的一部分
            }

            let chain = self.build_chain_from(start);
            if chain.blocks.len() > 1 {
                // 只保存长度大于1的链
                self.chains.insert(start, chain);
            }
        }
    }

    /// 从指定块开始构建链
    fn build_chain_from(&self, start: GuestAddr) -> BlockChain {
        let mut blocks = Vec::new();
        let mut current = start;
        let mut visited = HashSet::new();
        let mut total_frequency = 0;

        while blocks.len() < self.max_chain_length {
            if visited.contains(&current) {
                break; // 避免循环
            }

            visited.insert(current);
            blocks.push(current);

            // 累加频率
            total_frequency += self.block_frequencies.get(&current).copied().unwrap_or(0);

            // 查找下一个块
            let next = self.find_best_next_block(current);
            match next {
                Some(addr) => current = addr,
                None => break,
            }
        }

        BlockChain {
            start,
            blocks,
            frequency: total_frequency,
        }
    }

    /// 查找最佳的下一个块
    fn find_best_next_block(&self, current: GuestAddr) -> Option<GuestAddr> {
        let mut candidates: Vec<(GuestAddr, ChainType, u32)> = self
            .links
            .iter()
            .filter(|((from, _), _)| *from == current)
            .map(|((_, to), link)| (*to, link.link_type, link.frequency))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // 优先选择直接跳转
        candidates.sort_by(|a, b| {
            // 首先按链接类型排序（Direct优先）
            if a.1 == ChainType::Direct && b.1 != ChainType::Direct {
                return std::cmp::Ordering::Less;
            }
            if a.1 != ChainType::Direct && b.1 == ChainType::Direct {
                return std::cmp::Ordering::Greater;
            }

            // 然后按频率排序
            b.2.cmp(&a.2)
        });

        candidates.first().map(|(addr, _, _)| *addr)
    }

    /// 获取从指定块开始的链
    pub fn get_chain(&self, start: GuestAddr) -> Option<&BlockChain> {
        self.chains.get(&start)
    }

    /// 获取所有链
    pub fn all_chains(&self) -> impl Iterator<Item = &BlockChain> {
        self.chains.values()
    }

    /// 获取链接信息
    pub fn get_link(&self, from: GuestAddr, to: GuestAddr) -> Option<&ChainLink> {
        self.links.get(&(from, to))
    }

    /// 清除所有链接和链
    pub fn clear(&mut self) {
        self.links.clear();
        self.chains.clear();
        self.block_frequencies.clear();
    }

    /// 获取统计信息
    pub fn stats(&self) -> BlockChainerStats {
        BlockChainerStats {
            total_links: self.links.len(),
            total_chains: self.chains.len(),
            total_blocks: self.block_frequencies.len(),
            avg_chain_length: if self.chains.is_empty() {
                0.0
            } else {
                let total: usize = self.chains.values().map(|c| c.blocks.len()).sum();
                total as f64 / self.chains.len() as f64
            },
            max_chain_length: self.max_chain_length,
        }
    }
}

impl Default for BlockChainer {
    fn default() -> Self {
        Self::new()
    }
}

/// 块链接器统计信息
#[derive(Debug, Clone)]
pub struct BlockChainerStats {
    /// 总链接数
    pub total_links: usize,
    /// 总链数
    pub total_chains: usize,
    /// 总块数
    pub total_blocks: usize,
    /// 平均链长度
    pub avg_chain_length: f64,
    /// 最大链长度配置
    pub max_chain_length: usize,
}

/// 向后兼容的别名
pub type ChainingStats = BlockChainerStats;

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::IRBuilder;

    #[test]
    fn test_chainer_creation() {
        let chainer = BlockChainer::new();
        assert_eq!(chainer.stats().total_links, 0);
        assert_eq!(chainer.stats().total_chains, 0);
    }

    #[test]
    fn test_analyze_jump() {
        let mut chainer = BlockChainer::new();

        let mut builder1 = IRBuilder::new(GuestAddr(0x1000));
        builder1.set_term(Terminator::Jmp {
            target: GuestAddr(0x2000),
        });
        let block1 = builder1.build();

        chainer.analyze_block(&block1).unwrap();

        assert_eq!(chainer.stats().total_links, 1);
        assert!(
            chainer
                .get_link(GuestAddr(0x1000), GuestAddr(0x2000))
                .is_some()
        );
    }

    #[test]
    fn test_analyze_conditional() {
        let mut chainer = BlockChainer::new();

        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.set_term(Terminator::CondJmp {
            cond: 1,
            target_true: GuestAddr(0x2000),
            target_false: GuestAddr(0x3000),
        });
        let block = builder.build();

        chainer.analyze_block(&block).unwrap();

        assert_eq!(chainer.stats().total_links, 2);
        assert!(
            chainer
                .get_link(GuestAddr(0x1000), GuestAddr(0x2000))
                .is_some()
        );
        assert!(
            chainer
                .get_link(GuestAddr(0x1000), GuestAddr(0x3000))
                .is_some()
        );
    }

    #[test]
    fn test_build_chains() {
        let mut chainer = BlockChainer::new();

        // 创建三个块：block1 -> block2 -> block3
        let mut builder1 = IRBuilder::new(GuestAddr(0x1000));
        builder1.set_term(Terminator::Jmp {
            target: GuestAddr(0x2000),
        });

        let mut builder2 = IRBuilder::new(GuestAddr(0x2000));
        builder2.set_term(Terminator::Jmp {
            target: GuestAddr(0x3000),
        });

        let mut builder3 = IRBuilder::new(GuestAddr(0x3000));
        builder3.set_term(Terminator::Ret);

        let block1 = builder1.build();
        let block2 = builder2.build();
        let block3 = builder3.build();

        chainer.analyze_block(&block1).unwrap();
        chainer.analyze_block(&block2).unwrap();
        chainer.analyze_block(&block3).unwrap();

        chainer.build_chains();

        // 应该有一个从0x1000开始的链
        let chain = chainer.get_chain(GuestAddr(0x1000));
        assert!(chain.is_some());
        let chain = chain.unwrap();
        assert_eq!(chain.start, GuestAddr(0x1000));
        assert!(chain.blocks.len() >= 2);
    }

    #[test]
    fn test_max_chain_length() {
        let chainer = BlockChainer::with_config(2, true);
        assert_eq!(chainer.stats().max_chain_length, 2);
    }

    #[test]
    fn test_clear() {
        let mut chainer = BlockChainer::new();

        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.set_term(Terminator::Jmp {
            target: GuestAddr(0x2000),
        });
        let block = builder.build();

        chainer.analyze_block(&block).unwrap();
        chainer.build_chains();

        assert!(chainer.stats().total_links > 0);

        chainer.clear();
        assert_eq!(chainer.stats().total_links, 0);
        assert_eq!(chainer.stats().total_chains, 0);
    }

    #[test]
    fn test_stats() {
        let mut chainer = BlockChainer::new();

        let mut builder1 = IRBuilder::new(GuestAddr(0x1000));
        builder1.set_term(Terminator::Jmp {
            target: GuestAddr(0x2000),
        });
        let block1 = builder1.build();

        chainer.analyze_block(&block1).unwrap();

        let stats = chainer.stats();
        assert_eq!(stats.total_links, 1);
        assert_eq!(stats.total_blocks, 1);
    }
}
