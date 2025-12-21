//! 热路径优化器
//!
//! 识别频繁执行的代码块序列，并进行优化

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use vm_core::GuestAddr;

/// 代码块执行记录
#[derive(Debug, Clone)]
struct BlockExecution {
    /// 块地址
    pc: GuestAddr,
    /// 执行时间戳
    timestamp: Instant,
    /// 执行次数（在路径中）
    count: u32,
}

/// 执行路径
#[derive(Debug, Clone)]
pub struct ExecutionPath {
    /// 路径中的块序列
    blocks: Vec<GuestAddr>,
    /// 路径执行次数
    execution_count: u64,
    /// 最后执行时间
    last_execution: Instant,
    /// 路径总执行时间（微秒）
    total_time_us: u64,
    /// 是否为热路径
    is_hot: bool,
}

impl ExecutionPath {
    fn new(blocks: Vec<GuestAddr>) -> Self {
        Self {
            blocks,
            execution_count: 0,
            last_execution: Instant::now(),
            total_time_us: 0,
            is_hot: false,
        }
    }

    /// 记录路径执行
    fn record_execution(&mut self, time_us: u64) {
        self.execution_count += 1;
        self.last_execution = Instant::now();
        self.total_time_us += time_us;
        
        // 判断是否为热路径（执行次数 > 100 或总时间 > 10ms）
        self.is_hot = self.execution_count > 100 || self.total_time_us > 10_000;
    }

    /// 计算平均执行时间
    fn avg_time_us(&self) -> f64 {
        if self.execution_count == 0 {
            0.0
        } else {
            self.total_time_us as f64 / self.execution_count as f64
        }
    }
}

/// 热路径优化器
pub struct HotPathOptimizer {
    /// 当前执行路径（滑动窗口）
    current_path: VecDeque<BlockExecution>,
    /// 路径窗口大小
    path_window_size: usize,
    /// 识别的热路径
    hot_paths: HashMap<Vec<GuestAddr>, ExecutionPath>,
    /// 块到路径的映射（用于快速查找）
    block_to_paths: HashMap<GuestAddr, Vec<Vec<GuestAddr>>>,
    /// 热路径阈值
    hot_threshold: u64,
    /// 统计信息
    stats: HotPathStats,
}

/// 热路径统计信息
#[derive(Debug, Clone, Default)]
pub struct HotPathStats {
    /// 识别的热路径数量
    pub hot_path_count: usize,
    /// 路径执行总次数
    pub total_path_executions: u64,
    /// 热路径执行次数
    pub hot_path_executions: u64,
}

impl HotPathOptimizer {
    /// 创建新的热路径优化器
    pub fn new(path_window_size: usize, hot_threshold: u64) -> Self {
        Self {
            current_path: VecDeque::with_capacity(path_window_size),
            path_window_size,
            hot_paths: HashMap::new(),
            block_to_paths: HashMap::new(),
            hot_threshold,
            stats: HotPathStats::default(),
        }
    }

    /// 记录块执行
    pub fn record_block_execution(&mut self, pc: GuestAddr) {
        let execution = BlockExecution {
            pc,
            timestamp: Instant::now(),
            count: 1,
        };

        // 添加到当前路径
        self.current_path.push_back(execution);

        // 如果路径窗口已满，移除最旧的
        if self.current_path.len() > self.path_window_size {
            self.current_path.pop_front();
        }

        // 如果路径窗口已满，识别路径
        if self.current_path.len() == self.path_window_size {
            self.identify_path();
        }
    }

    /// 识别执行路径
    fn identify_path(&mut self) {
        // 提取路径中的块序列
        let path: Vec<GuestAddr> = self.current_path.iter().map(|e| e.pc).collect();

        // 更新路径统计
        let path_entry = self.hot_paths.entry(path.clone()).or_insert_with(|| {
            ExecutionPath::new(path.clone())
        });

        // 计算路径执行时间（简化：使用当前时间）
        let time_us = 100; // 简化处理，实际应该测量真实时间
        path_entry.record_execution(time_us);

        // 更新块到路径的映射
        for &block_pc in &path {
            self.block_to_paths
                .entry(block_pc)
                .or_insert_with(Vec::new)
                .push(path.clone());
        }

        // 更新统计信息
        self.stats.total_path_executions += 1;
        if path_entry.is_hot {
            self.stats.hot_path_executions += 1;
        }
    }

    /// 获取热路径列表
    pub fn get_hot_paths(&self, limit: usize) -> Vec<ExecutionPath> {
        let mut hot_paths: Vec<_> = self
            .hot_paths
            .values()
            .filter(|p| p.is_hot)
            .cloned()
            .collect();

        hot_paths.sort_by(|a, b| {
            b.execution_count.cmp(&a.execution_count)
        });

        hot_paths.truncate(limit);
        hot_paths
    }

    /// 检查块是否在热路径中
    pub fn is_block_in_hot_path(&self, pc: GuestAddr) -> bool {
        if let Some(paths) = self.block_to_paths.get(&pc) {
            paths.iter().any(|path| {
                self.hot_paths.get(path).map(|p| p.is_hot).unwrap_or(false)
            })
        } else {
            false
        }
    }

    /// 获取块的热路径
    pub fn get_block_hot_paths(&self, pc: GuestAddr) -> Vec<ExecutionPath> {
        if let Some(paths) = self.block_to_paths.get(&pc) {
            paths
                .iter()
                .filter_map(|path| {
                    self.hot_paths.get(path).filter(|p| p.is_hot).cloned()
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 预取热路径中的下一个块
    pub fn prefetch_next_blocks(&self, current_pc: GuestAddr, limit: usize) -> Vec<GuestAddr> {
        let mut prefetch_list = Vec::new();

        if let Some(paths) = self.block_to_paths.get(&current_pc) {
            for path in paths {
                if let Some(path_info) = self.hot_paths.get(path) {
                    if path_info.is_hot {
                        // 找到当前块在路径中的位置
                        if let Some(pos) = path.iter().position(|&pc| pc == current_pc) {
                            // 预取路径中的下一个块
                            if pos + 1 < path.len() {
                                prefetch_list.push(path[pos + 1]);
                            }
                        }
                    }
                }
            }
        }

        // 去重并限制数量
        prefetch_list.sort();
        prefetch_list.dedup();
        prefetch_list.truncate(limit);

        prefetch_list
    }

    /// 获取统计信息
    pub fn stats(&self) -> &HotPathStats {
        &self.stats
    }

    /// 更新热路径阈值
    pub fn set_hot_threshold(&mut self, threshold: u64) {
        self.hot_threshold = threshold;
        
        // 重新计算热路径状态
        for path in self.hot_paths.values_mut() {
            path.is_hot = path.execution_count > threshold || path.total_time_us > 10_000;
        }
        
        self.stats.hot_path_count = self.hot_paths.values().filter(|p| p.is_hot).count();
    }
}

impl Default for HotPathOptimizer {
    fn default() -> Self {
        Self::new(10, 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hot_path_detection() {
        let mut optimizer = HotPathOptimizer::new(5, 3);

        // 执行一个路径多次
        let path = vec![
            GuestAddr(0x1000),
            GuestAddr(0x2000),
            GuestAddr(0x3000),
            GuestAddr(0x4000),
            GuestAddr(0x5000),
        ];

        for _ in 0..5 {
            for &pc in &path {
                optimizer.record_block_execution(pc);
            }
        }

        let hot_paths = optimizer.get_hot_paths(10);
        assert!(!hot_paths.is_empty());
    }

    #[test]
    fn test_prefetch_next_blocks() {
        let mut optimizer = HotPathOptimizer::new(5, 3);

        let path = vec![
            GuestAddr(0x1000),
            GuestAddr(0x2000),
            GuestAddr(0x3000),
        ];

        for _ in 0..5 {
            for &pc in &path {
                optimizer.record_block_execution(pc);
            }
        }

        let next_blocks = optimizer.prefetch_next_blocks(GuestAddr(0x1000), 10);
        assert!(next_blocks.contains(&GuestAddr(0x2000)));
    }
}

