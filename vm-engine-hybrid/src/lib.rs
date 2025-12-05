//! Hybrid Execution Engine
//!
//! 混合执行引擎，结合解释器和JIT编译器，实现热点追踪和自适应优化

use std::collections::{HashMap, VecDeque};
use vm_core::{ExecResult, ExecStats, ExecStatus, ExecutionEngine, MMU};
use vm_engine_interpreter::Interpreter;
use vm_engine_jit::Jit;
use vm_ir::IRBlock;
use vm_ir::IROp;

/// 热点阈值：执行次数超过此值时触发JIT编译
const HOT_THRESHOLD: u64 = 100;

/// 块执行统计
#[derive(Debug, Clone, Default)]
pub struct BlockStats {
    /// 执行次数
    pub exec_count: u64,
    /// 是否已编译
    pub is_compiled: bool,
    /// 累计执行时间（微秒）
    pub total_time_us: u64,
}

/// 混合执行引擎
pub struct HybridEngine {
    /// 解释器
    interpreter: Interpreter,
    /// JIT编译器
    jit: Jit,
    /// 执行统计
    stats: HashMap<u64, BlockStats>,
    /// 总解释执行次数
    pub total_interpreted: u64,
    /// 总JIT执行次数
    pub total_jit: u64,
    /// 总编译次数
    pub total_compiled: u64,
    /// 分支预测器
    bp: BranchPredictor,
    /// 发射宽度
    issue_width: usize,
    /// 指令窗口大小
    window_size: usize,
    cycles: u64,
    issued_ops: u64,
    committed_ops: u64,
    cache: SimpleCache,
}

impl HybridEngine {
    /// 创建新的混合执行引擎
    pub fn new() -> Self {
        Self {
            interpreter: Interpreter::new(),
            jit: Jit::new(),
            stats: HashMap::new(),
            total_interpreted: 0,
            total_jit: 0,
            total_compiled: 0,
            bp: BranchPredictor::new(),
            issue_width: 4,
            window_size: 16,
            cycles: 0,
            issued_ops: 0,
            committed_ops: 0,
            cache: SimpleCache::new(64 * 1024, 64),
        }
    }

    /// 记录块执行并判断是否需要编译
    fn should_compile(&mut self, pc: u64) -> bool {
        let stats = self.stats.entry(pc).or_default();
        stats.exec_count += 1;

        // 如果已经编译过，直接返回false
        if stats.is_compiled {
            return false;
        }

        // 如果执行次数达到阈值，触发编译
        if stats.exec_count >= HOT_THRESHOLD {
            stats.is_compiled = true;
            true
        } else {
            false
        }
    }

    /// 获取执行统计
    pub fn get_stats(&self, pc: u64) -> Option<&BlockStats> {
        self.stats.get(&pc)
    }

    /// 获取所有统计信息
    pub fn get_all_stats(&self) -> &HashMap<u64, BlockStats> {
        &self.stats
    }

    /// 获取热点块（执行次数最多的前N个）
    pub fn get_hot_blocks(&self, top_n: usize) -> Vec<(u64, &BlockStats)> {
        let mut blocks: Vec<_> = self.stats.iter().map(|(pc, stats)| (*pc, stats)).collect();

        blocks.sort_by(|a, b| b.1.exec_count.cmp(&a.1.exec_count));
        blocks.truncate(top_n);
        blocks
    }

    /// 打印执行统计
    pub fn print_stats(&self) {
        println!("\n=== Hybrid Engine Statistics ===");
        println!("Total Interpreted: {}", self.total_interpreted);
        println!("Total JIT: {}", self.total_jit);
        println!("Total Compiled: {}", self.total_compiled);
        println!(
            "JIT Ratio: {:.2}%",
            if self.total_interpreted + self.total_jit > 0 {
                (self.total_jit as f64 / (self.total_interpreted + self.total_jit) as f64) * 100.0
            } else {
                0.0
            }
        );

        println!("\nTop 10 Hot Blocks:");
        for (i, (pc, stats)) in self.get_hot_blocks(10).iter().enumerate() {
            println!(
                "  {}. PC={:#x}: exec_count={}, compiled={}",
                i + 1,
                pc,
                stats.exec_count,
                stats.is_compiled
            );
        }
        let cpi = if self.committed_ops > 0 {
            self.cycles as f64 / self.committed_ops as f64
        } else {
            0.0
        };
        let issue_rate = if self.cycles > 0 {
            self.issued_ops as f64 / self.cycles as f64
        } else {
            0.0
        };
        println!(
            "Cycles: {} Committed: {} Issued: {}",
            self.cycles, self.committed_ops, self.issued_ops
        );
        println!("CPI: {:.3} IssueRate: {:.3}", cpi, issue_rate);
        println!(
            "Cache: hits={} misses={} writebacks={} sets={} ways={}",
            self.cache.hits,
            self.cache.misses,
            self.cache.writebacks,
            self.cache.sets,
            self.cache.ways
        );
    }

    /// 设置寄存器值
    pub fn set_reg_internal(&mut self, idx: u32, val: u64) {
        self.interpreter.set_reg(idx, val);
        if (idx as usize) < self.jit.regs.len() {
            self.jit.regs[idx as usize] = val;
        }
    }

    /// 获取寄存器值
    pub fn get_reg_internal(&self, idx: u32) -> u64 {
        self.interpreter.get_reg(idx)
    }

    /// 同步寄存器状态（从解释器到JIT）
    fn sync_regs_to_jit(&mut self) {
        for i in 0..32 {
            self.jit.regs[i] = self.interpreter.get_reg(i as u32);
        }
    }

    /// 同步寄存器状态（从JIT到解释器）
    fn sync_regs_from_jit(&mut self) {
        for i in 0..32 {
            self.interpreter.set_reg(i as u32, self.jit.regs[i]);
        }
    }
}

impl Default for HybridEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionEngine<vm_ir::IRBlock> for HybridEngine {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock) -> ExecResult {
        let pc = block.start_pc;

        // 检查是否需要编译
        if self.should_compile(pc) {
            self.total_compiled += 1;
            // 触发JIT编译
            self.sync_regs_to_jit();
            // 注意：这里只是标记为已编译，实际编译在JIT引擎内部完成
        }

        // 检查是否已经有编译好的代码
        let stats = self
            .stats
            .get(&pc)
            .expect("Failed to get JIT compilation stats");
        if stats.is_compiled && self.jit.is_hot(pc) {
            self.sync_regs_to_jit();
            let _ = self.jit.run(mmu, block);
            self.sync_regs_from_jit();
            self.total_jit += 1;
        } else {
            if self.can_pipeline(block) {
                self.run_pipeline(mmu, block);
            } else {
                let _ = self.interpreter.run(mmu, block);
            }
            self.total_interpreted += 1;
        }

        let next_pc = self.compute_next_pc(block);
        ExecResult {
            status: ExecStatus::Ok,
            stats: ExecStats::default(),
            next_pc,
        }
    }

    fn set_reg(&mut self, idx: usize, val: u64) {
        self.set_reg_internal(idx as u32, val);
    }

    fn get_reg(&self, idx: usize) -> u64 {
        self.get_reg_internal(idx as u32)
    }

    fn set_pc(&mut self, pc: u64) {
        self.interpreter.set_pc(pc);
        self.jit.pc = pc;
    }

    fn get_pc(&self) -> u64 {
        self.interpreter.get_pc()
    }

    fn get_vcpu_state(&self) -> vm_core::VcpuStateContainer {
        self.interpreter.get_vcpu_state()
    }

    fn set_vcpu_state(&mut self, state: &vm_core::VcpuStateContainer) {
        self.interpreter.set_vcpu_state(state);
        self.jit.set_vcpu_state(state);
    }
}

struct BranchPredictor {
    table: HashMap<u64, u8>,
}

impl BranchPredictor {
    fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }
    fn predict(&mut self, pc: u64) -> bool {
        let c = self.table.get(&pc).cloned().unwrap_or(1);
        c >= 2
    }
    fn update(&mut self, pc: u64, taken: bool) {
        let mut c = self.table.get(&pc).cloned().unwrap_or(1);
        if taken {
            if c < 3 {
                c += 1
            }
        } else {
            if c > 0 {
                c -= 1
            }
        }
        self.table.insert(pc, c);
    }
}

impl HybridEngine {
    fn can_pipeline(&self, block: &IRBlock) -> bool {
        block.ops.len() > 1
    }

    fn run_pipeline(&mut self, mmu: &mut dyn MMU, block: &IRBlock) {
        let mut ready: HashMap<u32, bool> = HashMap::new();
        for r in 0..64 {
            ready.insert(r, true);
        }
        let mut idx = 0usize;
        let mut robq: VecDeque<(Option<u32>, Option<u64>, IROp)> = VecDeque::new();
        let end = block.ops.len();
        while idx < end {
            let limit = (idx + self.window_size).min(end);
            let mut batch: Vec<&IROp> = Vec::new();
            for i in idx..limit {
                if batch.len() >= self.issue_width {
                    break;
                }
                let op = &block.ops[i];
                if self.op_ready(op, &ready) {
                    batch.push(op);
                }
            }
            for op in batch.iter() {
                let (dst, val) = self.compute_op(mmu, *op);
                robq.push_back((dst, val, (*op).clone()));
            }
            // Commit stage: commit up to issue_width results
            let mut committed = 0usize;
            while committed < self.issue_width {
                if let Some((dst, val, op)) = robq.pop_front() {
                    if let (Some(d), Some(v)) = (dst, val) {
                        self.interpreter.set_reg(d as u32, v);
                    }
                    self.mark_ready(&op, &mut ready);
                    committed += 1;
                } else {
                    break;
                }
            }
            idx = limit;
            self.cycles += 1;
            self.issued_ops += batch.len() as u64;
            self.committed_ops += committed as u64;
        }
        match &block.term {
            vm_ir::Terminator::Jmp { .. } => {}
            vm_ir::Terminator::Ret => {}
            vm_ir::Terminator::CondJmp { cond, .. } => {
                let taken = self.interpreter.get_reg(*cond as u32) != 0;
                self.bp.update(block.start_pc, taken);
            }
            _ => {}
        }
    }

    fn op_ready(&self, op: &IROp, ready: &HashMap<u32, bool>) -> bool {
        match op {
            IROp::MovImm { .. } => true,
            IROp::Add { src1, src2, .. }
            | IROp::Sub { src1, src2, .. }
            | IROp::Mul { src1, src2, .. }
            | IROp::Div { src1, src2, .. }
            | IROp::Rem { src1, src2, .. } => {
                *ready.get(src1).unwrap_or(&true) && *ready.get(src2).unwrap_or(&true)
            }
            IROp::Load { base, .. } => *ready.get(base).unwrap_or(&true),
            IROp::Store { src, base, .. } => {
                *ready.get(src).unwrap_or(&true) && *ready.get(base).unwrap_or(&true)
            }
            _ => true,
        }
    }

    fn mark_ready(&self, op: &IROp, ready: &mut HashMap<u32, bool>) {
        match op {
            IROp::MovImm { dst, .. }
            | IROp::Add { dst, .. }
            | IROp::Sub { dst, .. }
            | IROp::Mul { dst, .. }
            | IROp::Div { dst, .. }
            | IROp::Rem { dst, .. }
            | IROp::Load { dst, .. } => {
                ready.insert(*dst, true);
            }
            _ => {}
        }
    }

    fn compute_op(&mut self, mmu: &mut dyn MMU, op: &IROp) -> (Option<u32>, Option<u64>) {
        match *op {
            IROp::MovImm { dst, imm } => (Some(dst), Some(imm)),
            IROp::Add { dst, src1, src2 } => {
                let a = self.interpreter.get_reg(src1 as u32);
                let b = self.interpreter.get_reg(src2 as u32);
                (Some(dst), Some(a.wrapping_add(b)))
            }
            IROp::Sub { dst, src1, src2 } => {
                let a = self.interpreter.get_reg(src1 as u32);
                let b = self.interpreter.get_reg(src2 as u32);
                (Some(dst), Some(a.wrapping_sub(b)))
            }
            IROp::Mul { dst, src1, src2 } => {
                let a = self.interpreter.get_reg(src1 as u32);
                let b = self.interpreter.get_reg(src2 as u32);
                (Some(dst), Some(a.wrapping_mul(b)))
            }
            IROp::AddImm { dst, src, imm } => {
                let a = self.interpreter.get_reg(src as u32);
                (Some(dst), Some(a.wrapping_add(imm as u64)))
            }
            IROp::SllImm { dst, src, sh } => {
                let a = self.interpreter.get_reg(src as u32);
                (Some(dst), Some(a << sh))
            }
            IROp::SrlImm { dst, src, sh } => {
                let a = self.interpreter.get_reg(src as u32);
                (Some(dst), Some(a >> sh))
            }
            IROp::SraImm { dst, src, sh } => {
                let a = self.interpreter.get_reg(src as u32) as i64;
                (Some(dst), Some(((a >> sh) as u64)))
            }
            IROp::CmpEq { dst, lhs, rhs } => {
                let a = self.interpreter.get_reg(lhs as u32);
                let b = self.interpreter.get_reg(rhs as u32);
                (Some(dst), Some((a == b) as u64))
            }
            IROp::CmpNe { dst, lhs, rhs } => {
                let a = self.interpreter.get_reg(lhs as u32);
                let b = self.interpreter.get_reg(rhs as u32);
                (Some(dst), Some((a != b) as u64))
            }
            IROp::CmpLt { dst, lhs, rhs } => {
                let a = self.interpreter.get_reg(lhs as u32) as i64;
                let b = self.interpreter.get_reg(rhs as u32) as i64;
                (Some(dst), Some((a < b) as u64))
            }
            IROp::CmpLtU { dst, lhs, rhs } => {
                let a = self.interpreter.get_reg(lhs as u32);
                let b = self.interpreter.get_reg(rhs as u32);
                (Some(dst), Some((a < b) as u64))
            }
            IROp::CmpGe { dst, lhs, rhs } => {
                let a = self.interpreter.get_reg(lhs as u32) as i64;
                let b = self.interpreter.get_reg(rhs as u32) as i64;
                (Some(dst), Some((a >= b) as u64))
            }
            IROp::CmpGeU { dst, lhs, rhs } => {
                let a = self.interpreter.get_reg(lhs as u32);
                let b = self.interpreter.get_reg(rhs as u32);
                (Some(dst), Some((a >= b) as u64))
            }
            IROp::Select {
                dst,
                cond,
                true_val,
                false_val,
            } => {
                let c = self.interpreter.get_reg(cond as u32);
                let t = self.interpreter.get_reg(true_val as u32);
                let f = self.interpreter.get_reg(false_val as u32);
                (Some(dst), Some(if c != 0 { t } else { f }))
            }
            IROp::Load {
                dst,
                base,
                offset,
                size,
                ..
            } => {
                let basev = self.interpreter.get_reg(base as u32);
                let pa = (basev as i64 + offset) as u64;
                let val = match size {
                    1 => self.cache.read(mmu, pa, 1),
                    2 => self.cache.read(mmu, pa, 2),
                    4 => self.cache.read(mmu, pa, 4),
                    _ => self.cache.read(mmu, pa, 8),
                };
                (Some(dst), Some(val))
            }
            IROp::Store {
                src,
                base,
                offset,
                size,
                ..
            } => {
                let basev = self.interpreter.get_reg(base as u32);
                let pa = (basev as i64 + offset) as u64;
                let val = self.interpreter.get_reg(src as u32);
                let _ = self.cache.write(mmu, pa, val, size);
                (None, None)
            }
            IROp::Fadd { dst, src1, src2 } => {
                let a = f64::from_bits(self.interpreter.get_reg(src1 as u32));
                let b = f64::from_bits(self.interpreter.get_reg(src2 as u32));
                (Some(dst), Some((a + b).to_bits()))
            }
            IROp::Fsub { dst, src1, src2 } => {
                let a = f64::from_bits(self.interpreter.get_reg(src1 as u32));
                let b = f64::from_bits(self.interpreter.get_reg(src2 as u32));
                (Some(dst), Some((a - b).to_bits()))
            }
            IROp::Fmul { dst, src1, src2 } => {
                let a = f64::from_bits(self.interpreter.get_reg(src1 as u32));
                let b = f64::from_bits(self.interpreter.get_reg(src2 as u32));
                (Some(dst), Some((a * b).to_bits()))
            }
            IROp::Fdiv { dst, src1, src2 } => {
                let a = f64::from_bits(self.interpreter.get_reg(src1 as u32));
                let b = f64::from_bits(self.interpreter.get_reg(src2 as u32));
                (Some(dst), Some((a / b).to_bits()))
            }
            _ => {
                let mut b = IRBlock {
                    start_pc: 0,
                    ops: vec![op.clone()],
                    term: vm_ir::Terminator::Ret,
                };
                let _ = self.interpreter.run(mmu, &b);
                (None, None)
            }
        }
    }

    fn compute_next_pc(&mut self, block: &IRBlock) -> u64 {
        match &block.term {
            vm_ir::Terminator::Jmp { target } => *target,
            vm_ir::Terminator::CondJmp {
                cond: _,
                target_true,
                target_false,
            } => {
                let pred = self.bp.predict(block.start_pc);
                if pred { *target_true } else { *target_false }
            }
            vm_ir::Terminator::JmpReg { base, offset } => {
                let b = self.interpreter.get_reg(*base as u32);
                (b as i64 + *offset) as u64
            }
            vm_ir::Terminator::Ret => block.start_pc,
            _ => block.start_pc,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IRBuilder, IROp, Terminator};
    use vm_mem::SoftMmu;

    #[test]
    fn test_hybrid_engine_basic() {
        let mut engine = HybridEngine::new();
        let mut mmu = SoftMmu::new(1024 * 1024, false);

        // 创建一个简单的IR块
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::MovImm { dst: 1, imm: 42 });
        builder.set_term(Terminator::Jmp { target: 0x1004 });
        let block = builder.build();

        // 执行多次以触发JIT编译
        for _ in 0..HOT_THRESHOLD + 10 {
            let res = engine.run(&mut mmu, &block);
            match res.status {
                ExecStatus::Ok | ExecStatus::Continue => {}
                _ => panic!("unexpected status: {:?}", res.status),
            }
        }

        assert_eq!(engine.get_reg(1), 42);
        assert!(engine.total_compiled > 0);
    }

    #[test]
    fn test_hot_block_tracking() {
        let mut engine = HybridEngine::new();

        // 模拟执行
        for _ in 0..50 {
            let _ = engine.should_compile(0x1000);
        }
        for _ in 0..200 {
            let _ = engine.should_compile(0x2000);
        }

        let hot_blocks = engine.get_hot_blocks(2);
        assert_eq!(hot_blocks.len(), 2);
        assert_eq!(hot_blocks[0].0, 0x2000); // 最热的块
        assert_eq!(hot_blocks[1].0, 0x1000);
    }
}
struct CacheLine {
    data: Vec<u8>,
    dirty: bool,
}
struct SimpleCache {
    size: usize,
    line: usize,
    map: HashMap<u64, CacheLine>,
    set_lru: HashMap<usize, VecDeque<u64>>,
    sets: usize,
    ways: usize,
    write_back: bool,
    write_allocate: bool,
    hits: u64,
    misses: u64,
    writebacks: u64,
}

impl SimpleCache {
    fn new(size: usize, line: usize) -> Self {
        let total_lines = (size / line).max(1);
        let ways = 4usize;
        let sets = (total_lines / ways).max(1);
        Self {
            size,
            line,
            map: HashMap::new(),
            set_lru: HashMap::new(),
            sets,
            ways,
            write_back: true,
            write_allocate: true,
            hits: 0,
            misses: 0,
            writebacks: 0,
        }
    }
    fn set_index(&self, base: u64) -> usize {
        ((base / self.line as u64) % self.sets as u64) as usize
    }
    fn touch(&mut self, base: u64) {
        let set = self.set_index(base);
        let lru = self.set_lru.entry(set).or_insert_with(VecDeque::new);
        if let Some(pos) = lru.iter().position(|&x| x == base) {
            lru.remove(pos);
        }
        lru.push_front(base);
    }
    fn fetch_line(&mut self, mmu: &mut dyn MMU, base: u64) {
        let mut buf = vec![0u8; self.line];
        for i in 0..self.line {
            buf[i] = mmu.read(base + i as u64, 1).unwrap_or(0) as u8;
        }
        self.map.insert(
            base,
            CacheLine {
                data: buf,
                dirty: false,
            },
        );
        self.touch(base);
        // Per-set eviction
        let set = self.set_index(base);
        let lru = self.set_lru.entry(set).or_insert_with(VecDeque::new);
        while lru.len() > self.ways {
            if let Some(ev) = lru.pop_back() {
                if let Some(line) = self.map.remove(&ev) {
                    if self.write_back && line.dirty {
                        let mut v = 0u64;
                        for i in 0..self.line {
                            v |= (line.data[i] as u64) << ((i % 8) * 8);
                            if i % 8 == 7 {
                                let pa = ev + (i as u64 & !7u64);
                                let _ = mmu.write(pa, v, 8);
                                v = 0;
                            }
                        }
                        self.writebacks += 1;
                    }
                }
            }
        }
        let pre = base + self.line as u64;
        if !self.map.contains_key(&pre) {
            let mut pbuf = vec![0u8; self.line];
            for i in 0..self.line {
                pbuf[i] = mmu.read(pre + i as u64, 1).unwrap_or(0) as u8;
            }
            self.map.insert(
                pre,
                CacheLine {
                    data: pbuf,
                    dirty: false,
                },
            );
            self.touch(pre);
        }
    }
    fn read(&mut self, mmu: &mut dyn MMU, pa: u64, size: u8) -> u64 {
        let base = pa & !((self.line as u64) - 1);
        if !self.map.contains_key(&base) {
            self.fetch_line(mmu, base);
            self.misses += 1;
        } else {
            self.hits += 1;
        }
        self.touch(base);
        let off = (pa - base) as usize;
        let line = self.map.get(&base).unwrap();
        let mut v = 0u64;
        for i in 0..(size as usize) {
            v |= (line.data[off + i] as u64) << (i * 8);
        }
        v
    }
    fn write(
        &mut self,
        mmu: &mut dyn MMU,
        pa: u64,
        val: u64,
        size: u8,
    ) -> Result<(), vm_core::VmError> {
        let base = pa & !((self.line as u64) - 1);
        if !self.map.contains_key(&base) {
            if self.write_allocate {
                self.fetch_line(mmu, base);
                self.misses += 1;
            } else {
                self.misses += 1;
                return mmu.write(pa, val, size);
            }
        } else {
            self.hits += 1;
        }
        self.touch(base);
        let off = (pa - base) as usize;
        if let Some(line) = self.map.get_mut(&base) {
            for i in 0..(size as usize) {
                line.data[off + i] = ((val >> (i * 8)) & 0xFF) as u8;
            }
            if self.write_back {
                line.dirty = true;
            } else {
                let _ = mmu.write(pa, val, size);
            }
        }
        Ok(())
    }
}
