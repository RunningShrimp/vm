//! 高级 JIT 优化模块
//!
//! 实现 Block Chaining, Inline Caching, Trace Selection

use vm_core::GuestAddr;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

/// 块链接 Patch 信息
#[derive(Debug, Clone)]
pub struct ChainPatch {
    /// 源块出口地址
    pub from_addr: GuestAddr,
    /// 目标块入口地址
    pub to_addr: GuestAddr,
    /// Patch 位置（相对于编译代码的字节偏移）
    pub patch_offset: usize,
    /// Patch 前的原始指令（用于回滚）
    pub original_code: Vec<u8>,
}

/// 块链接管理器
pub struct BlockChainer {
    /// 待处理的 Patch 列表
    patches: Arc<RwLock<Vec<ChainPatch>>>,
    /// 块到其入口地址的映射
    block_entries: Arc<RwLock<HashMap<GuestAddr, usize>>>,
}

impl BlockChainer {
    pub fn new() -> Self {
        Self {
            patches: Arc::new(RwLock::new(Vec::new())),
            block_entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册块入口
    pub fn register_block_entry(&self, block_addr: GuestAddr, entry_offset: usize) {
        self.block_entries.write().insert(block_addr, entry_offset);
    }

    /// 创建块链接 Patch
    pub fn create_chain_patch(
        &self,
        from_addr: GuestAddr,
        to_addr: GuestAddr,
        patch_offset: usize,
    ) -> Result<(), String> {
        let block_entries = self.block_entries.read();
        if !block_entries.contains_key(&to_addr) {
            return Err(format!("Target block {:?} not registered", to_addr));
        }

        drop(block_entries);

        let patch = ChainPatch {
            from_addr,
            to_addr,
            patch_offset,
            original_code: Vec::new(), // 实际应该保存原始代码
        };

        self.patches.write().push(patch);
        Ok(())
    }

    /// 获取所有 Patch
    pub fn get_patches(&self) -> Vec<ChainPatch> {
        self.patches.read().clone()
    }
}

impl Default for BlockChainer {
    fn default() -> Self {
        Self::new()
    }
}

/// 内联缓存条目
#[derive(Debug, Clone)]
pub struct InlineCacheEntry {
    /// 缓存的目标地址
    pub target_addr: GuestAddr,
    /// 缓存命中次数
    pub hit_count: u32,
    /// 是否已使用
    pub used: bool,
}

/// 内联缓存 (单态 + 多态)
pub struct InlineCache {
    /// 缓存条目 (多态缓存最多保持 4 个条目)
    entries: Vec<InlineCacheEntry>,
    /// 最大缓存条目数
    max_entries: usize,
}

impl InlineCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries),
            max_entries,
        }
    }

    /// 添加或更新缓存条目
    pub fn record_target(&mut self, target_addr: GuestAddr) {
        // 检查是否已存在
        if let Some(entry) = self.entries.iter_mut().find(|e| e.target_addr == target_addr) {
            entry.hit_count += 1;
            entry.used = true;
            return;
        }

        // 添加新条目
        if self.entries.len() < self.max_entries {
            self.entries.push(InlineCacheEntry {
                target_addr,
                hit_count: 1,
                used: true,
            });
        } else {
            // 替换最冷的条目
            if let Some(coldest) = self
                .entries
                .iter_mut()
                .min_by_key(|e| e.hit_count)
            {
                coldest.target_addr = target_addr;
                coldest.hit_count = 1;
                coldest.used = true;
            }
        }
    }

    /// 获取缓存命中率
    pub fn hit_rate(&self) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }
        let total_hits: u32 = self.entries.iter().map(|e| e.hit_count).sum();
        let used_count = self.entries.iter().filter(|e| e.used).count();
        if used_count == 0 {
            0.0
        } else {
            total_hits as f64 / used_count as f64
        }
    }

    /// 获取单态缓存状态
    pub fn is_monomorphic(&self) -> bool {
        self.entries.len() <= 1
    }
}

/// 热点 Trace 信息
#[derive(Debug, Clone)]
pub struct HotTrace {
    /// Trace 入口 PC
    pub entry_pc: GuestAddr,
    /// Trace 中的所有指令 PC 序列
    pub pcs: Vec<GuestAddr>,
    /// 热度计数
    pub hotness: u32,
    /// 是否已编译
    pub compiled: bool,
}

/// 热点追踪器
pub struct TraceSelector {
    /// 块计数器
    block_counters: Arc<RwLock<HashMap<GuestAddr, u32>>>,
    /// 热点 Trace 库
    traces: Arc<RwLock<Vec<HotTrace>>>,
    /// 热点阈值
    hotness_threshold: u32,
    /// 当前正在录制的 Trace
    recording_trace: Arc<RwLock<Option<Vec<GuestAddr>>>>,
}

impl TraceSelector {
    pub fn new(hotness_threshold: u32) -> Self {
        Self {
            block_counters: Arc::new(RwLock::new(HashMap::new())),
            traces: Arc::new(RwLock::new(Vec::new())),
            hotness_threshold,
            recording_trace: Arc::new(RwLock::new(None)),
        }
    }

    /// 记录块执行
    pub fn record_block_execution(&self, block_addr: GuestAddr) {
        let mut counters = self.block_counters.write();
        let count = counters.entry(block_addr).or_insert(0);
        *count += 1;

        // 如果热度达到阈值，启动 Trace 录制
        if *count == self.hotness_threshold {
            let mut recording = self.recording_trace.write();
            if recording.is_none() {
                *recording = Some(vec![block_addr]);
            }
        }
    }

    /// 向当前 Trace 添加块
    pub fn append_to_trace(&self, block_addr: GuestAddr) {
        let mut recording = self.recording_trace.write();
        if let Some(trace) = recording.as_mut() {
            trace.push(block_addr);
        }
    }

    /// 完成 Trace 录制
    pub fn finish_trace(&self) -> Option<HotTrace> {
        let mut recording = self.recording_trace.write();
        if let Some(pcs) = recording.take() {
            if !pcs.is_empty() {
                let entry_pc = pcs[0];
                let trace = HotTrace {
                    entry_pc,
                    pcs,
                    hotness: self.hotness_threshold,
                    compiled: false,
                };

                self.traces.write().push(trace.clone());
                return Some(trace);
            }
        }
        None
    }

    /// 获取所有热点 Trace
    pub fn get_traces(&self) -> Vec<HotTrace> {
        self.traces.read().clone()
    }
}

impl Default for TraceSelector {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_chaining() {
        let chainer = BlockChainer::new();
        chainer.register_block_entry(0x1000, 0);
        chainer.register_block_entry(0x2000, 100);

        assert!(chainer.create_chain_patch(0x1000, 0x2000, 50).is_ok());

        let patches = chainer.get_patches();
        assert_eq!(patches.len(), 1);
    }

    #[test]
    fn test_inline_cache() {
        let mut ic = InlineCache::new(4);

        ic.record_target(0x1000);
        ic.record_target(0x1000);
        ic.record_target(0x2000);

        assert!(!ic.is_monomorphic());
        assert!(ic.hit_rate() > 0.0);
    }

    #[test]
    fn test_trace_selector() {
        let selector = TraceSelector::new(3);

        selector.record_block_execution(0x1000);
        selector.record_block_execution(0x1000);
        selector.record_block_execution(0x1000);

        // 应该启动录制
        selector.append_to_trace(0x2000);
        let trace = selector.finish_trace();

        assert!(trace.is_some());
    }
}
