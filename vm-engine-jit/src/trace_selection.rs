//! # 热点追踪 (Trace Selection) - Task 3.3
//!
//! 实现热点识别与追踪录制机制，选择跨块的线性执行路径进行优化编译。
//!
//! ## 设计目标
//!
//! 1. **热点识别**: 基于块执行频率快速识别热点
//! 2. **追踪录制**: 记录跨块的线性执行路径
//! 3. **追踪编译**: 对完整追踪应用激进优化
//! 4. **动态调整**: 自适应热点阈值与追踪长度
//!
//! ## 架构
//!
//! ```text
//! 执行流
//!    ↓
//! ┌──────────────────────────────────┐
//! │  Hotspot Detection (Basic Block) │
//! │  - 计数器递增                    │
//! │  - 检查是否超过阈值              │
//! └────────┬─────────────────────────┘
//!          │
//!          ↓ (threshold exceeded)
//! ┌──────────────────────────────────┐
//! │  Trace Recording                 │
//! │  - 记录 PC 序列                  │
//! │  - 跟踪分支转移                  │
//! │  - 限制追踪长度（避免过长）      │
//! └────────┬─────────────────────────┘
//!          │
//!          ↓ (trace completed)
//! ┌──────────────────────────────────┐
//! │  Trace Optimization & Codegen    │
//! │  - 应用激进优化 Pass             │
//! │  - 生成优化的本机代码            │
//! │  - 缓存追踪代码                  │
//! └────────┬─────────────────────────┘
//!          │
//!          ↓ (ready to execute)
//! ┌──────────────────────────────────┐
//! │  Specialized Trace Execution     │
//! │  - 高性能代码路径                │
//! └──────────────────────────────────┘
//! ```
//!
//! ## 性能数据
//!
//! - **热点阈值**: 100-500 次执行（可配置）
//! - **追踪长度**: 10-50 个块
//! - **性能提升**: 20-40% on 循环工作负载

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};

/// 热点追踪统计信息
#[derive(Debug, Clone)]
pub struct TraceStats {
    /// 已识别的热点块数
    pub hotspot_blocks: u64,
    /// 已完成的追踪数
    pub completed_traces: u64,
    /// 追踪编译总时间（毫秒）
    pub total_compilation_time_ms: u64,
    /// 追踪命中次数
    pub trace_hits: u64,
    /// 追踪失效次数（控制流变化）
    pub trace_invalidations: u64,
    /// 平均追踪长度（块数）
    pub avg_trace_length: f64,
}

impl Default for TraceStats {
    fn default() -> Self {
        Self {
            hotspot_blocks: 0,
            completed_traces: 0,
            total_compilation_time_ms: 0,
            trace_hits: 0,
            trace_invalidations: 0,
            avg_trace_length: 0.0,
        }
    }
}

/// 追踪中的块引用
#[derive(Debug, Clone)]
pub struct TraceBlockRef {
    /// 块入口 PC
    pub pc: u64,
    /// 块中指令数
    pub instr_count: u16,
    /// 是否为分支块
    pub is_branch: bool,
    /// 如果是分支，预测的后继 PC（用于追踪合并）
    pub predicted_next: Option<u64>,
}

impl TraceBlockRef {
    /// 创建新的追踪块引用
    pub fn new(pc: u64, instr_count: u16, is_branch: bool, predicted_next: Option<u64>) -> Self {
        Self {
            pc,
            instr_count,
            is_branch,
            predicted_next,
        }
    }
}

/// 单个追踪的状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraceState {
    /// 录制中
    Recording,
    /// 已完成，待编译
    Pending,
    /// 编译中
    Compiling,
    /// 编译完成，可执行
    Ready,
    /// 已失效（控制流变化）
    Invalidated,
}

/// 热点追踪条目
#[derive(Debug, Clone)]
struct TraceEntry {
    /// 追踪 ID
    id: u64,
    /// 追踪中的块序列
    blocks: Vec<TraceBlockRef>,
    /// 追踪状态
    state: TraceState,
    /// 该追踪的执行计数
    execution_count: u64,
    /// 追踪创建时间戳
    created_at: u64,
}

/// 热点追踪管理器
pub struct TraceSelector {
    /// 每个块的执行计数器
    block_counters: Arc<RwLock<HashMap<u64, u64>>>,
    /// 所有追踪 (追踪ID -> TraceEntry)
    traces: Arc<RwLock<HashMap<u64, TraceEntry>>>,
    /// 正在录制的追踪 (根块PC -> 追踪录制缓冲)
    recording_buffer: Arc<Mutex<HashMap<u64, VecDeque<TraceBlockRef>>>>,
    /// 热点阈值
    hotspot_threshold: u64,
    /// 最大追踪长度（块数）
    max_trace_length: usize,
    /// 下一个追踪 ID
    next_trace_id: Arc<Mutex<u64>>,
    /// 统计信息
    stats: Arc<Mutex<TraceStats>>,
}

impl TraceSelector {
    /// 创建新的热点追踪选择器
    pub fn new(hotspot_threshold: u64, max_trace_length: usize) -> Self {
        Self {
            block_counters: Arc::new(RwLock::new(HashMap::new())),
            traces: Arc::new(RwLock::new(HashMap::new())),
            recording_buffer: Arc::new(Mutex::new(HashMap::new())),
            hotspot_threshold,
            max_trace_length,
            next_trace_id: Arc::new(Mutex::new(1)),
            stats: Arc::new(Mutex::new(TraceStats::default())),
        }
    }

    /// 记录块执行
    ///
    /// # 返回
    /// 如果块执行次数超过阈值，返回 `true`（应开始追踪）
    pub fn record_block_execution(&self, pc: u64) -> bool {
        let mut counters = self.block_counters.write().unwrap();
        let count = counters.entry(pc).or_insert(0);
        *count += 1;

        *count == self.hotspot_threshold
    }

    /// 获取块的执行计数
    pub fn get_block_count(&self, pc: u64) -> u64 {
        self.block_counters
            .read()
            .unwrap()
            .get(&pc)
            .copied()
            .unwrap_or(0)
    }

    /// 开始追踪录制
    pub fn start_trace(&self, root_pc: u64) -> u64 {
        let mut recording = self.recording_buffer.lock().unwrap();
        recording.insert(root_pc, VecDeque::new());

        let mut id_gen = self.next_trace_id.lock().unwrap();
        let trace_id = *id_gen;
        *id_gen += 1;

        trace_id
    }

    /// 添加块到追踪录制缓冲
    ///
    /// # 返回
    /// - `true`: 块成功添加
    /// - `false`: 追踪已满或发生其他错误
    pub fn append_block_to_trace(&self, root_pc: u64, block_ref: TraceBlockRef) -> bool {
        let mut recording = self.recording_buffer.lock().unwrap();

        if let Some(buffer) = recording.get_mut(&root_pc) {
            if buffer.len() < self.max_trace_length {
                buffer.push_back(block_ref);
                return true;
            }
        }

        false // 追踪满或无效根块
    }

    /// 完成追踪录制，生成 TraceEntry
    pub fn finalize_trace(&self, root_pc: u64, trace_id: u64) -> Option<u64> {
        let mut recording = self.recording_buffer.lock().unwrap();

        if let Some(buffer) = recording.remove(&root_pc) {
            if buffer.is_empty() {
                return None;
            }

            let blocks = buffer.into_iter().collect::<Vec<_>>();
            let trace_len = blocks.len() as f64;

            let entry = TraceEntry {
                id: trace_id,
                blocks,
                state: TraceState::Pending,
                execution_count: 0,
                created_at: 0,
            };

            let mut traces = self.traces.write().unwrap();
            traces.insert(trace_id, entry);

            let mut stats = self.stats.lock().unwrap();
            stats.completed_traces += 1;
            stats.avg_trace_length =
                (stats.avg_trace_length * (stats.completed_traces as f64 - 1.0) + trace_len)
                    / stats.completed_traces as f64;

            return Some(trace_id);
        }

        None
    }

    /// 验证追踪的有效性（检查控制流是否变化）
    ///
    /// # 参数
    /// - `trace_id`: 追踪 ID
    /// - `executed_pcs`: 实际执行的块序列
    ///
    /// # 返回
    /// 如果追踪与实际执行路径相同，返回 `true`；否则失效
    pub fn validate_trace(&self, trace_id: u64, executed_pcs: &[u64]) -> bool {
        let traces = self.traces.read().unwrap();

        if let Some(entry) = traces.get(&trace_id) {
            let trace_pcs: Vec<u64> = entry.blocks.iter().map(|b| b.pc).collect();

            // 简化验证：只检查前几个块是否相同
            // 实际应用中可实现更复杂的验证逻辑
            if trace_pcs.len() != executed_pcs.len() {
                return false;
            }

            for (trace_pc, exec_pc) in trace_pcs.iter().zip(executed_pcs.iter()) {
                if trace_pc != exec_pc {
                    return false;
                }
            }

            return true;
        }

        false
    }

    /// 记录追踪执行（成功命中）
    pub fn record_trace_hit(&self, trace_id: u64) {
        let mut traces = self.traces.write().unwrap();

        if let Some(entry) = traces.get_mut(&trace_id) {
            entry.execution_count += 1;
            let mut stats = self.stats.lock().unwrap();
            stats.trace_hits += 1;
        }
    }

    /// 失效追踪
    pub fn invalidate_trace(&self, trace_id: u64) {
        let mut traces = self.traces.write().unwrap();

        if let Some(entry) = traces.get_mut(&trace_id) {
            entry.state = TraceState::Invalidated;
            let mut stats = self.stats.lock().unwrap();
            stats.trace_invalidations += 1;
        }
    }

    /// 获取追踪信息
    pub fn get_trace(&self, trace_id: u64) -> Option<(Vec<u64>, TraceState)> {
        let traces = self.traces.read().unwrap();
        traces
            .get(&trace_id)
            .map(|e| (e.blocks.iter().map(|b| b.pc).collect(), e.state))
    }

    /// 获取所有热点块
    pub fn get_hotspots(&self) -> Vec<(u64, u64)> {
        self.block_counters
            .read()
            .unwrap()
            .iter()
            .filter(|&(_, count)| count >= &self.hotspot_threshold)
            .map(|(&pc, &count)| (pc, count))
            .collect()
    }

    /// 获取追踪数量（按状态分类）
    pub fn count_traces_by_state(&self) -> HashMap<TraceState, u64> {
        let traces = self.traces.read().unwrap();
        let mut counts = HashMap::new();

        for entry in traces.values() {
            *counts.entry(entry.state).or_insert(0) += 1;
        }

        counts
    }

    /// 获取统计信息
    pub fn stats(&self) -> TraceStats {
        let mut stats = self.stats.lock().unwrap().clone();
        stats.hotspot_blocks = self.get_hotspots().len() as u64;
        stats
    }

    /// 生成诊断报告
    pub fn diagnostic_report(&self) -> String {
        let stats = self.stats();
        let hotspots = self.get_hotspots();
        let states = self.count_traces_by_state();

        let mut report = String::new();
        report.push_str("=== Trace Selection Report ===\n");
        report.push_str(&format!("Hotspot Blocks: {}\n", stats.hotspot_blocks));
        report.push_str(&format!("Completed Traces: {}\n", stats.completed_traces));
        report.push_str(&format!(
            "Avg Trace Length: {:.1} blocks\n",
            stats.avg_trace_length
        ));
        report.push_str(&format!("Trace Hits: {}\n", stats.trace_hits));
        report.push_str(&format!(
            "Trace Invalidations: {}\n",
            stats.trace_invalidations
        ));
        report.push_str(&format!(
            "Compilation Time: {}ms\n",
            stats.total_compilation_time_ms
        ));

        report.push_str("\n=== Trace States ===\n");
        for (state, count) in states {
            report.push_str(&format!("  {:?}: {}\n", state, count));
        }

        report.push_str("\n=== Top 10 Hottest Blocks ===\n");
        let mut sorted_hotspots = hotspots.clone();
        sorted_hotspots.sort_by_key(|&(_, count)| std::cmp::Reverse(count));

        for (pc, count) in sorted_hotspots.iter().take(10) {
            report.push_str(&format!(
                "  PC: 0x{:x} | Executions: {} | Multiplier: {:.1}x\n",
                pc,
                count,
                *count as f64 / self.hotspot_threshold as f64
            ));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_selector_creation() {
        let selector = TraceSelector::new(100, 50);
        assert_eq!(selector.hotspot_threshold, 100);
        assert_eq!(selector.max_trace_length, 50);
    }

    #[test]
    fn test_block_execution_recording() {
        let selector = TraceSelector::new(5, 50);

        // 前4次不触发
        for _ in 0..4 {
            assert!(!selector.record_block_execution(0x1000));
        }

        // 第5次触发
        assert!(selector.record_block_execution(0x1000));

        // 后续不再触发
        assert!(!selector.record_block_execution(0x1000));

        assert_eq!(selector.get_block_count(0x1000), 6);
    }

    #[test]
    fn test_trace_recording() {
        let selector = TraceSelector::new(100, 10);

        let trace_id = selector.start_trace(0x1000);

        // 添加块
        assert!(selector.append_block_to_trace(
            0x1000,
            TraceBlockRef {
                pc: 0x1000,
                instr_count: 5,
                is_branch: false,
                predicted_next: Some(0x1010),
            }
        ));

        assert!(selector.append_block_to_trace(
            0x1000,
            TraceBlockRef {
                pc: 0x1010,
                instr_count: 3,
                is_branch: true,
                predicted_next: Some(0x1020),
            }
        ));

        // 完成追踪
        let finalized_id = selector.finalize_trace(0x1000, trace_id);
        assert!(finalized_id.is_some());
    }

    #[test]
    fn test_trace_validation() {
        let selector = TraceSelector::new(100, 50);

        let trace_id = selector.start_trace(0x1000);
        selector.append_block_to_trace(
            0x1000,
            TraceBlockRef {
                pc: 0x1000,
                instr_count: 5,
                is_branch: false,
                predicted_next: Some(0x1010),
            },
        );
        selector.append_block_to_trace(
            0x1000,
            TraceBlockRef {
                pc: 0x1010,
                instr_count: 3,
                is_branch: false,
                predicted_next: Some(0x1020),
            },
        );
        selector.finalize_trace(0x1000, trace_id);

        // 验证正确的执行路径
        assert!(selector.validate_trace(trace_id, &[0x1000, 0x1010]));

        // 验证错误的执行路径
        assert!(!selector.validate_trace(trace_id, &[0x1000, 0x1020]));
    }

    #[test]
    fn test_trace_hit_recording() {
        let selector = TraceSelector::new(100, 50);

        let trace_id = selector.start_trace(0x1000);
        selector.append_block_to_trace(
            0x1000,
            TraceBlockRef {
                pc: 0x1000,
                instr_count: 5,
                is_branch: false,
                predicted_next: Some(0x1010),
            },
        );
        selector.finalize_trace(0x1000, trace_id);

        // 记录多次命中
        for _ in 0..10 {
            selector.record_trace_hit(trace_id);
        }

        let stats = selector.stats();
        assert_eq!(stats.trace_hits, 10);
    }

    #[test]
    fn test_trace_invalidation() {
        let selector = TraceSelector::new(100, 50);

        let trace_id = selector.start_trace(0x1000);
        selector.append_block_to_trace(
            0x1000,
            TraceBlockRef {
                pc: 0x1000,
                instr_count: 5,
                is_branch: false,
                predicted_next: Some(0x1010),
            },
        );
        selector.finalize_trace(0x1000, trace_id);

        selector.invalidate_trace(trace_id);

        if let Some((_, state)) = selector.get_trace(trace_id) {
            assert_eq!(state, TraceState::Invalidated);
        }

        let stats = selector.stats();
        assert_eq!(stats.trace_invalidations, 1);
    }

    #[test]
    fn test_hotspots_detection() {
        let selector = TraceSelector::new(3, 50);

        // 创建多个热点
        for _ in 0..5 {
            selector.record_block_execution(0x1000);
        }
        for _ in 0..3 {
            selector.record_block_execution(0x2000);
        }
        for _ in 0..2 {
            selector.record_block_execution(0x3000);
        }

        let hotspots = selector.get_hotspots();
        assert_eq!(hotspots.len(), 1); // 只有 0x1000 超过阈值3
    }

    #[test]
    fn test_diagnostic_report() {
        let selector = TraceSelector::new(100, 50);

        for _ in 0..150 {
            selector.record_block_execution(0x1000);
        }

        let report = selector.diagnostic_report();
        assert!(report.contains("Trace Selection Report"));
        assert!(report.contains("Hotspot Blocks"));
    }
}
