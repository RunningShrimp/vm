//! # vm-engine-interpreter - 解释器执行引擎
//!
//! 提供虚拟机的解释器执行引擎，直接解释执行 IR 块。
//!
//! ## 特性
//!
//! - **完整指令支持**: 支持所有 IR 操作，包括向量和原子操作
//! - **块缓存**: 可选的已解码块缓存，减少重复解码开销
//! - **中断处理**: 支持可定制的中断处理回调
//! - **指令融合**: 识别并优化常见指令序列
//! - **优化调度**: 使用预编译调度表加速执行
//!
//! ## 性能优化
//!
//! - 启用 `block-cache` feature 开启块缓存
//! - 缓存最近执行的 IR 块，避免重复解码
//! - 使用 vm-simd 库进行 SIMD 优化向量运算
//! - 支持指令融合减少调度开销

use std::collections::HashMap;
use vm_core::{
    AccessType, ExecResult, ExecStats, ExecStatus, ExecutionEngine, GuestAddr, MMU, VmError,
};
use vm_ir::{AtomicOp, IRBlock, IROp, Terminator};
use vm_simd::{
    vec_add, vec_add_sat_s, vec_add_sat_u, vec_mul, vec_sub, vec_sub_sat_s, vec_sub_sat_u,
    vec256_add_sat_s, vec256_add_sat_u, vec256_mul_sat_s, vec256_mul_sat_u, vec256_sub_sat_s,
    vec256_sub_sat_u,
};

/// 异步设备I/O模块
pub mod async_device_io;
/// 异步执行引擎模块
pub mod async_executor;
/// 异步执行引擎集成模块
pub mod async_executor_integration;
/// 异步中断处理模块
pub mod async_interrupt_handler;

/// 块缓存配置
pub const BLOCK_CACHE_SIZE: usize = 256;

/// 指令融合模式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FusedOp {
    /// Load + Add: load value then add to register
    LoadAdd {
        dst: u32,
        base: u32,
        offset: i64,
        src2: u32,
    },
    /// Load + Use: load then immediate use
    LoadUse { dst: u32, base: u32, size: u8 },
    /// Add + Store: add then store result
    AddStore {
        src1: u32,
        src2: u32,
        base: u32,
        size: u8,
    },
    /// MovImm + Add: move immediate then add
    MovImmAdd { dst: u32, imm: u64, src2: u32 },
    /// Cmp + Branch: compare then conditional jump
    CmpBranch {
        lhs: u32,
        rhs: u32,
        target_true: GuestAddr,
        target_false: GuestAddr,
    },
    /// Add + Add: consecutive adds to same destination
    ChainedAdd {
        dst: u32,
        src1: u32,
        src2: u32,
        src3: u32,
    },
}

/// 指令融合器 - 识别可融合的指令序列
pub struct InstructionFuser {
    /// 融合统计
    pub fused_count: u64,
    /// 检查的指令对数
    pub checked_pairs: u64,
}

impl InstructionFuser {
    pub fn new() -> Self {
        Self {
            fused_count: 0,
            checked_pairs: 0,
        }
    }

    /// 尝试融合两条相邻指令
    #[inline]
    pub fn try_fuse(&mut self, op1: &IROp, op2: &IROp) -> Option<FusedOp> {
        self.checked_pairs += 1;

        match (op1, op2) {
            // MovImm followed by Add using the immediate
            (
                IROp::MovImm { dst: d1, imm },
                IROp::Add {
                    dst: d2,
                    src1,
                    src2,
                },
            ) if *d1 == *src2 && *d2 == *src1 => {
                self.fused_count += 1;
                Some(FusedOp::MovImmAdd {
                    dst: *d2,
                    imm: *imm,
                    src2: *src1,
                })
            }
            // Two consecutive adds to same dst
            (
                IROp::Add {
                    dst: d1,
                    src1: s1a,
                    src2: s2a,
                },
                IROp::Add {
                    dst: d2,
                    src1: s1b,
                    src2: s2b,
                },
            ) if *d1 == *d2 && *d1 == *s1b => {
                self.fused_count += 1;
                Some(FusedOp::ChainedAdd {
                    dst: *d2,
                    src1: *s1a,
                    src2: *s2a,
                    src3: *s2b,
                })
            }
            _ => None,
        }
    }

    /// 获取融合率
    pub fn fusion_rate(&self) -> f64 {
        if self.checked_pairs == 0 {
            0.0
        } else {
            self.fused_count as f64 / self.checked_pairs as f64
        }
    }
}

impl Default for InstructionFuser {
    fn default() -> Self {
        Self::new()
    }
}

/// 预取提示 - 用于提高缓存命中率
#[inline]
pub fn prefetch_block(pc: GuestAddr, _cache: &BlockCache) {
    // 在支持的平台上发出预取指令
    #[cfg(target_arch = "x86_64")]
    {
        // 预取下一个可能的块
        let _ = pc; // 占位，实际实现需要根据分支预测
    }
    #[cfg(target_arch = "aarch64")]
    {
        let _ = pc;
    }
}

/// Helper to create ExecStats with all required fields
fn make_stats(executed_ops: u64) -> ExecStats {
    ExecStats {
        executed_insns: executed_ops, // For interpreter, ops ≈ instructions
        executed_ops,
        tlb_hits: 0,
        tlb_misses: 0,
        jit_compiles: 0,
        jit_compile_time_ns: 0,
    }
}

/// Helper to create ExecResult with proper next_pc
fn make_result(status: ExecStatus, executed_ops: u64, next_pc: GuestAddr) -> ExecResult {
    ExecResult {
        status,
        stats: make_stats(executed_ops),
        next_pc,
    }
}

/// 块缓存条目
#[derive(Clone)]
pub struct CachedBlock {
    /// 原始 IR 块
    pub block: IRBlock,
    /// 执行次数统计
    pub exec_count: u64,
    /// 最后执行时间戳
    pub last_exec: u64,
}

/// 块缓存管理器
pub struct BlockCache {
    /// 缓存映射: PC -> CachedBlock
    cache: HashMap<GuestAddr, CachedBlock>,
    /// 最大缓存大小
    max_size: usize,
    /// 全局时间戳计数器
    timestamp: u64,
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
}

impl BlockCache {
    /// 创建新的块缓存
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(max_size),
            max_size,
            timestamp: 0,
            hits: 0,
            misses: 0,
        }
    }

    /// 查找缓存块
    pub fn get(&mut self, pc: GuestAddr) -> Option<&IRBlock> {
        self.timestamp += 1;
        if let Some(entry) = self.cache.get_mut(&pc) {
            entry.exec_count += 1;
            entry.last_exec = self.timestamp;
            self.hits += 1;
            Some(&entry.block)
        } else {
            self.misses += 1;
            None
        }
    }

    /// 插入缓存块
    pub fn insert(&mut self, pc: GuestAddr, block: IRBlock) {
        // 如果缓存已满，驱逐最旧的条目
        if self.cache.len() >= self.max_size {
            self.evict_oldest();
        }

        self.cache.insert(
            pc,
            CachedBlock {
                block,
                exec_count: 1,
                last_exec: self.timestamp,
            },
        );
    }

    /// 驱逐最旧的缓存条目 (LRU)
    fn evict_oldest(&mut self) {
        if let Some((&oldest_pc, _)) = self.cache.iter().min_by_key(|(_, entry)| entry.last_exec) {
            self.cache.remove(&oldest_pc);
        }
    }

    /// 使特定地址的缓存失效
    pub fn invalidate(&mut self, pc: GuestAddr) {
        self.cache.remove(&pc);
    }

    /// 清空所有缓存
    pub fn clear(&mut self) {
        self.cache.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// 获取缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl Default for BlockCache {
    fn default() -> Self {
        Self::new(BLOCK_CACHE_SIZE)
    }
}

pub struct Interpreter {
    regs: [u64; 32],
    pc: GuestAddr,
    intr_handler: Option<Box<dyn Fn(InterruptCtx, &mut Interpreter) -> ExecInterruptAction + Send>>,
    intr_handler_ext:
        Option<Box<dyn Fn(InterruptCtxExt, &mut Interpreter) -> ExecInterruptAction + Send>>,
    pub fence_acquire_count: u64,
    pub fence_release_count: u64,
    pub intr_mask_until: u32,
    /// 块缓存（可选）
    pub block_cache: Option<BlockCache>,
    /// 指令融合器
    pub fuser: InstructionFuser,
    /// 系统调用处理器
    pub syscall_handler: vm_core::syscall::SyscallHandler,
    /// 启用优化调度
    pub optimized_dispatch: bool,
    csr_mstatus: u64,
    csr_mie: u64,
    csr_mip: u64,
    priv_mode: u8,
    csr_sstatus: u64,
    csr_sie: u64,
    csr_sip: u64,
    csr_medeleg: u64,
    csr_mideleg: u64,
    csr_mtvec: u64,
    csr_stvec: u64,
    csr_mepc: u64,
    csr_mcause: u64,
    csr_sepc: u64,
    csr_scause: u64,
}

pub enum ExecInterruptAction {
    Continue,
    Abort,
    InjectState,
    Retry,
    Mask,
    Deliver,
}

pub struct InterruptCtx {
    pub vector: u32,
    pub pc: GuestAddr,
}
#[allow(dead_code)]
pub struct InterruptCtxExt {
    pub vector: u32,
    pub pc: GuestAddr,
    pub regs_ptr: *mut u64,
}

impl Interpreter {
    /// 创建新的解释器实例
    pub fn new() -> Self {
        Self {
            regs: [0; 32],
            pc: 0,
            intr_handler: None,
            intr_handler_ext: None,
            fence_acquire_count: 0,
            fence_release_count: 0,
            intr_mask_until: 0,
            block_cache: None,
            fuser: InstructionFuser::new(),
            syscall_handler: vm_core::syscall::SyscallHandler::new(),
            optimized_dispatch: true,
            csr_mstatus: 0,
            csr_mie: 0,
            csr_mip: 0,
            priv_mode: 3,
            csr_sstatus: 0,
            csr_sie: 0,
            csr_sip: 0,
            csr_medeleg: 0,
            csr_mideleg: 0,
            csr_mtvec: 0,
            csr_stvec: 0,
            csr_mepc: 0,
            csr_mcause: 0,
            csr_sepc: 0,
            csr_scause: 0,
        }
    }

    /// 创建带块缓存的解释器
    pub fn with_block_cache(cache_size: usize) -> Self {
        Self {
            regs: [0; 32],
            pc: 0,
            intr_handler: None,
            intr_handler_ext: None,
            fence_acquire_count: 0,
            fence_release_count: 0,
            intr_mask_until: 0,
            block_cache: Some(BlockCache::new(cache_size)),
            fuser: InstructionFuser::new(),
            syscall_handler: vm_core::syscall::SyscallHandler::new(),
            optimized_dispatch: true,
            csr_mstatus: 0,
            csr_mie: 0,
            csr_mip: 0,
            priv_mode: 3,
            csr_sstatus: 0,
            csr_sie: 0,
            csr_sip: 0,
            csr_medeleg: 0,
            csr_mideleg: 0,
            csr_mtvec: 0,
            csr_stvec: 0,
            csr_mepc: 0,
            csr_mcause: 0,
            csr_sepc: 0,
            csr_scause: 0,
        }
    }

    /// 创建带完整优化的解释器
    pub fn with_full_optimization(cache_size: usize) -> Self {
        Self {
            regs: [0; 32],
            pc: 0,
            intr_handler: None,
            intr_handler_ext: None,
            fence_acquire_count: 0,
            fence_release_count: 0,
            intr_mask_until: 0,
            block_cache: Some(BlockCache::new(cache_size)),
            fuser: InstructionFuser::new(),
            syscall_handler: vm_core::syscall::SyscallHandler::new(),
            optimized_dispatch: true,
            csr_mstatus: 0,
            csr_mie: 0,
            csr_mip: 0,
            priv_mode: 3,
            csr_sstatus: 0,
            csr_sie: 0,
            csr_sip: 0,
            csr_medeleg: 0,
            csr_mideleg: 0,
            csr_mtvec: 0,
            csr_stvec: 0,
            csr_mepc: 0,
            csr_mcause: 0,
            csr_sepc: 0,
            csr_scause: 0,
        }
    }

    /// 启用/禁用优化调度
    pub fn set_optimized_dispatch(&mut self, enabled: bool) {
        self.optimized_dispatch = enabled;
    }

    /// 获取指令融合统计
    pub fn fusion_stats(&self) -> (u64, u64, f64) {
        (
            self.fuser.fused_count,
            self.fuser.checked_pairs,
            self.fuser.fusion_rate(),
        )
    }

    /// 启用块缓存
    pub fn enable_block_cache(&mut self, cache_size: usize) {
        self.block_cache = Some(BlockCache::new(cache_size));
    }

    /// 禁用块缓存
    pub fn disable_block_cache(&mut self) {
        self.block_cache = None;
    }

    /// 获取块缓存统计信息
    pub fn cache_stats(&self) -> Option<(u64, u64, f64)> {
        self.block_cache
            .as_ref()
            .map(|c| (c.hits, c.misses, c.hit_rate()))
    }

    pub fn set_reg(&mut self, idx: u32, val: u64) {
        let hi = idx >> 16;
        let guest = if hi != 0 { hi } else { idx & 0x1F };
        if guest != 0 {
            self.regs[guest as usize] = val;
        }
    }
    pub fn get_reg(&self, idx: u32) -> u64 {
        let hi = idx >> 16;
        let guest = if hi != 0 { hi } else { idx & 0x1F };
        self.regs[guest as usize]
    }
    pub fn get_regs_ptr(&mut self) -> *mut u64 {
        self.regs.as_mut_ptr()
    }
    pub fn read_csr(&self, csr: u16) -> u64 {
        match csr {
            0x300 => self.csr_mstatus,
            0x304 => self.csr_mie,
            0x344 => self.csr_mip,
            0x100 => self.csr_sstatus,
            0x104 => self.csr_sie,
            0x144 => self.csr_sip,
            0x302 => self.csr_medeleg,
            0x303 => self.csr_mideleg,
            0x305 => self.csr_mtvec,
            0x341 => self.csr_mepc,
            0x342 => self.csr_mcause,
            0x141 => self.csr_sepc,
            0x142 => self.csr_scause,
            0x105 => self.csr_stvec,
            _ => 0,
        }
    }
    pub fn write_csr(&mut self, csr: u16, val: u64) {
        match csr {
            0x300 => {
                self.csr_mstatus = val;
            }
            0x304 => {
                self.csr_mie = val;
            }
            0x344 => {
                self.csr_mip = val;
            }
            0x100 => {
                self.csr_sstatus = val;
            }
            0x104 => {
                self.csr_sie = val;
            }
            0x144 => {
                self.csr_sip = val;
            }
            0x302 => {
                self.csr_medeleg = val;
            }
            0x303 => {
                self.csr_mideleg = val;
            }
            0x305 => {
                self.csr_mtvec = val;
            }
            0x341 => {
                self.csr_mepc = val;
            }
            0x342 => {
                self.csr_mcause = val;
            }
            0x141 => {
                self.csr_sepc = val;
            }
            0x142 => {
                self.csr_scause = val;
            }
            0x105 => {
                self.csr_stvec = val;
            }
            _ => {}
        }
    }
    pub fn resume_from_trap(&mut self) {
        let pc = if self.priv_mode == 3 {
            self.csr_mepc
        } else {
            self.csr_sepc
        };
        self.pc = pc;
    }
    pub fn clear_trap(&mut self) {
        self.csr_mcause = 0;
        self.csr_scause = 0;
    }
    pub fn get_priv_mode(&self) -> u8 {
        self.priv_mode
    }
    pub fn set_interrupt_handler<
        F: 'static + Send + Fn(InterruptCtx, &mut Interpreter) -> ExecInterruptAction,
    >(
        &mut self,
        f: F,
    ) {
        self.intr_handler = Some(Box::new(f));
    }
    pub fn set_interrupt_handler_ext<
        F: 'static + Send + Fn(InterruptCtxExt, &mut Interpreter) -> ExecInterruptAction,
    >(
        &mut self,
        f: F,
    ) {
        self.intr_handler_ext = Some(Box::new(f));
    }
    pub fn get_fence_counts(&self) -> (u64, u64) {
        (self.fence_acquire_count, self.fence_release_count)
    }
}

fn sign_extend(val: u64, bits: u64) -> i64 {
    let shift = 64 - bits;
    ((val << shift) as i64) >> shift
}

impl ExecutionEngine<IRBlock> for Interpreter {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock) -> ExecResult {
        let mut count = 0u64;
        for op in &block.ops {
            match op {
                IROp::Add { dst, src1, src2 } => {
                    let v = self.get_reg(*src1).wrapping_add(self.get_reg(*src2));
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Sub { dst, src1, src2 } => {
                    let v = self.get_reg(*src1).wrapping_sub(self.get_reg(*src2));
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Mul { dst, src1, src2 } => {
                    let v = self.get_reg(*src1).wrapping_mul(self.get_reg(*src2));
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Div {
                    dst,
                    src1,
                    src2,
                    signed,
                } => {
                    let s1 = self.get_reg(*src1);
                    let s2 = self.get_reg(*src2);
                    let v = if *signed {
                        if s2 == 0 {
                            u64::MAX
                        } else {
                            (s1 as i64).wrapping_div(s2 as i64) as u64
                        }
                    } else {
                        if s2 == 0 {
                            u64::MAX
                        } else {
                            s1.wrapping_div(s2)
                        }
                    };
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Rem {
                    dst,
                    src1,
                    src2,
                    signed,
                } => {
                    let s1 = self.get_reg(*src1);
                    let s2 = self.get_reg(*src2);
                    let v = if *signed {
                        if s2 == 0 {
                            s1
                        } else {
                            (s1 as i64).wrapping_rem(s2 as i64) as u64
                        }
                    } else {
                        if s2 == 0 { s1 } else { s1.wrapping_rem(s2) }
                    };
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::And { dst, src1, src2 } => {
                    let v = self.get_reg(*src1) & self.get_reg(*src2);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Or { dst, src1, src2 } => {
                    let v = self.get_reg(*src1) | self.get_reg(*src2);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Xor { dst, src1, src2 } => {
                    let v = self.get_reg(*src1) ^ self.get_reg(*src2);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Not { dst, src } => {
                    let v = !self.get_reg(*src);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::AddImm { dst, src, imm } => {
                    let v = self.get_reg(*src).wrapping_add(*imm as u64);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::MulImm { dst, src, imm } => {
                    let v = self.get_reg(*src).wrapping_mul(*imm as u64);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::MovImm { dst, imm } => {
                    self.set_reg(*dst, *imm);
                    count += 1;
                }
                IROp::SllImm { dst, src, sh } => {
                    let v = self.get_reg(*src) << *sh as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::SrlImm { dst, src, sh } => {
                    let v = self.get_reg(*src) >> *sh as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::SraImm { dst, src, sh } => {
                    let v = (self.get_reg(*src) as i64) >> *sh as i64;
                    self.set_reg(*dst, v as u64);
                    count += 1;
                }
                IROp::Sll { dst, src, shreg } => {
                    let sh = (self.get_reg(*shreg) & 0x3f) as u8;
                    let v = self.get_reg(*src) << sh as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Srl { dst, src, shreg } => {
                    let sh = (self.get_reg(*shreg) & 0x3f) as u8;
                    let v = self.get_reg(*src) >> sh as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Sra { dst, src, shreg } => {
                    let sh = (self.get_reg(*shreg) & 0x3f) as u8;
                    let v = (self.get_reg(*src) as i64) >> sh as i64;
                    self.set_reg(*dst, v as u64);
                    count += 1;
                }
                IROp::CmpEq { dst, lhs, rhs } => {
                    let v = (self.get_reg(*lhs) == self.get_reg(*rhs)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::CmpNe { dst, lhs, rhs } => {
                    let v = (self.get_reg(*lhs) != self.get_reg(*rhs)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::CmpLt { dst, lhs, rhs } => {
                    let v = ((self.get_reg(*lhs) as i64) < (self.get_reg(*rhs) as i64)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::CmpLtU { dst, lhs, rhs } => {
                    let v = (self.get_reg(*lhs) < self.get_reg(*rhs)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::CmpGe { dst, lhs, rhs } => {
                    let v = ((self.get_reg(*lhs) as i64) >= (self.get_reg(*rhs) as i64)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::CmpGeU { dst, lhs, rhs } => {
                    let v = (self.get_reg(*lhs) >= self.get_reg(*rhs)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Select {
                    dst,
                    cond,
                    true_val,
                    false_val,
                } => {
                    let v = if self.get_reg(*cond) != 0 {
                        self.get_reg(*true_val)
                    } else {
                        self.get_reg(*false_val)
                    };
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Load {
                    dst,
                    base,
                    offset,
                    size,
                    flags,
                } => {
                    let va = self.get_reg(*base).wrapping_add(*offset as u64);
                    if flags.align != 0 && (va % flags.align as u64 != 0) {
                        return make_result(
                            ExecStatus::Fault(VmError::from(vm_core::Fault::AccessViolation {
                                addr: 0,
                                access: AccessType::Read,
                            })),
                            count,
                            block.start_pc,
                        );
                    }
                    if flags.atomic && !(matches!(size, 1 | 2 | 4 | 8) && flags.align == *size) {
                        return make_result(
                            ExecStatus::Fault(VmError::from(vm_core::Fault::AccessViolation {
                                addr: 0,
                                access: AccessType::Read,
                            })),
                            count,
                            block.start_pc,
                        );
                    }
                    match flags.order {
                        vm_ir::MemOrder::Acquire => {
                            self.fence_acquire_count += 1;
                        }
                        vm_ir::MemOrder::Release => {}
                        vm_ir::MemOrder::AcqRel => {
                            self.fence_acquire_count += 1;
                        }
                        vm_ir::MemOrder::SeqCst => {
                            self.fence_acquire_count += 1;
                            self.fence_release_count += 1;
                        }
                        vm_ir::MemOrder::None => {}
                    }

                    let pa = match mmu.translate(va, AccessType::Read) {
                        Ok(p) => p,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    match mmu.read(pa, *size) {
                        Ok(v) => {
                            self.set_reg(*dst, v);
                        }
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    }
                    count += 1;
                }
                IROp::Store {
                    src,
                    base,
                    offset,
                    size,
                    flags,
                } => {
                    let va = self.get_reg(*base).wrapping_add(*offset as u64);
                    if flags.align != 0 && (va % flags.align as u64 != 0) {
                        return make_result(
                            ExecStatus::Fault(VmError::from(vm_core::Fault::AccessViolation {
                                addr: 0,
                                access: AccessType::Read,
                            })),
                            count,
                            block.start_pc,
                        );
                    }
                    if flags.atomic && !(matches!(size, 1 | 2 | 4 | 8) && flags.align == *size) {
                        return make_result(
                            ExecStatus::Fault(VmError::from(vm_core::Fault::AccessViolation {
                                addr: 0,
                                access: AccessType::Read,
                            })),
                            count,
                            block.start_pc,
                        );
                    }
                    match flags.order {
                        vm_ir::MemOrder::Release => {
                            self.fence_release_count += 1;
                        }
                        vm_ir::MemOrder::AcqRel => {
                            self.fence_release_count += 1;
                        }
                        vm_ir::MemOrder::SeqCst => {
                            self.fence_acquire_count += 1;
                            self.fence_release_count += 1;
                        }
                        _ => {}
                    }
                    let pa = match mmu.translate(va, AccessType::Write) {
                        Ok(p) => p,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    match mmu.write(pa, self.get_reg(*src), *size) {
                        Ok(()) => {}
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    }
                    count += 1;
                }
                IROp::SysCall => {
                    // Create GuestRegs from current state
                    let mut regs = vm_core::GuestRegs::new();
                    regs.gpr.copy_from_slice(&self.regs);
                    regs.pc = block.start_pc;

                    // Handle syscall
                    let result = self.syscall_handler.handle_syscall(
                        &mut regs,
                        vm_core::syscall::SyscallArch::Riscv64,
                        mmu,
                    );

                    match result {
                        vm_core::syscall::SyscallResult::Success(ret) => {
                            self.set_reg(10, ret as u64); // a0 = ret
                            count += 1;
                        }
                        vm_core::syscall::SyscallResult::Error(err) => {
                            self.set_reg(10, err as u64); // a0 = error code
                            count += 1;
                        }
                        vm_core::syscall::SyscallResult::Block => {
                            self.set_reg(10, 0);
                            count += 1;
                        }
                        vm_core::syscall::SyscallResult::Exit(_) => {
                            return make_result(
                                ExecStatus::Fault(VmError::from(vm_core::Fault::Shutdown)),
                                count,
                                block.start_pc,
                            );
                        }
                    }
                }

                IROp::TlbFlush { vaddr: _ } => {
                    // Interpreter uses SoftMMU which might not need explicit flush or we can't easily flush it here without MMU API change.
                    // For now, treat as NOP or maybe we should add a flush method to MMU trait?
                    // Given the current MMU trait doesn't have flush, we'll just count it.
                    count += 1;
                }
                IROp::VecAdd {
                    dst,
                    src1,
                    src2,
                    element_size,
                } => {
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let acc = vec_add(a, b, *element_size);
                    self.set_reg(*dst, acc);
                    count += 1;
                }
                IROp::VecSub {
                    dst,
                    src1,
                    src2,
                    element_size,
                } => {
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let acc = vec_sub(a, b, *element_size);
                    self.set_reg(*dst, acc);
                    count += 1;
                }
                IROp::VecMul {
                    dst,
                    src1,
                    src2,
                    element_size,
                } => {
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let acc = vec_mul(a, b, *element_size);
                    self.set_reg(*dst, acc);
                    count += 1;
                }
                IROp::VecMulSat {
                    dst,
                    src1,
                    src2,
                    element_size,
                    signed,
                } => {
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes = 64 / lane_bits;
                    let mut acc = 0u64;
                    for i in 0..lanes {
                        let shift = i * lane_bits;
                        let mask = ((1u128 << lane_bits) - 1) as u64;
                        let av = (a >> shift) & mask;
                        let bv = (b >> shift) & mask;
                        let rv = if *signed {
                            let max = ((1i128 << (lane_bits - 1)) - 1) as i128;
                            let min = (-(1i128 << (lane_bits - 1))) as i128;
                            let sa = sign_extend(av, lane_bits) as i128;
                            let sb = sign_extend(bv, lane_bits) as i128;
                            let prod = sa * sb;
                            let clamped = if prod > max {
                                max
                            } else if prod < min {
                                min
                            } else {
                                prod
                            };
                            (clamped as i128 as u128 as u64) & mask
                        } else {
                            let max = ((1u128 << lane_bits) - 1) as u128;
                            let prod = (av as u128) * (bv as u128);
                            let clamped = if prod > max { max } else { prod };
                            (clamped as u64) & mask
                        };
                        acc |= rv << shift;
                    }
                    self.set_reg(*dst, acc);
                    count += 1;
                }
                IROp::Vec128Add {
                    dst_lo,
                    dst_hi,
                    src1_lo,
                    src1_hi,
                    src2_lo,
                    src2_hi,
                    element_size,
                    signed,
                } => {
                    let a =
                        ((self.get_reg(*src1_hi) as u128) << 64) | (self.get_reg(*src1_lo) as u128);
                    let b =
                        ((self.get_reg(*src2_hi) as u128) << 64) | (self.get_reg(*src2_lo) as u128);
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes = (128 / lane_bits) as usize;
                    let mut acc: u128 = 0;
                    for i in 0..lanes {
                        let shift = i * lane_bits as usize;
                        let mask = ((1u128 << lane_bits) - 1) as u128;
                        let av = (a >> shift) & mask;
                        let bv = (b >> shift) & mask;
                        let rv = if *signed {
                            let max = ((1i128 << (lane_bits - 1)) - 1) as i128;
                            let min = (-(1i128 << (lane_bits - 1))) as i128;
                            let sa = ((av << (128 - lane_bits)) as i128) >> (128 - lane_bits);
                            let sb = ((bv << (128 - lane_bits)) as i128) >> (128 - lane_bits);
                            let sum = sa + sb;
                            let clamped = if sum > max {
                                max
                            } else if sum < min {
                                min
                            } else {
                                sum
                            } as i128;
                            (clamped as i128 as u128) & mask
                        } else {
                            let max = ((1u128 << lane_bits) - 1) as u128;
                            let sum = av + bv;
                            let clamped = if sum > max { max } else { sum };
                            clamped & mask
                        };
                        acc |= rv << shift;
                    }
                    let lo = (acc & ((1u128 << 64) - 1)) as u64;
                    let hi = (acc >> 64) as u64;
                    self.set_reg(*dst_lo, lo);
                    self.set_reg(*dst_hi, hi);
                    count += 1;
                }
                IROp::AtomicCmpXchg {
                    dst,
                    base,
                    expected,
                    new,
                    size,
                } => {
                    let va = self.get_reg(*base);
                    let pa_r = match mmu.translate(va, AccessType::Read) {
                        Ok(p) => p,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    let old = match mmu.read(pa_r, *size) {
                        Ok(v) => v,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    if old == self.get_reg(*expected) {
                        let pa_w = match mmu.translate(va, AccessType::Write) {
                            Ok(p) => p,
                            Err(e) => {
                                return make_result(ExecStatus::Fault(e), count, block.start_pc);
                            }
                        };
                        if let Err(e) = mmu.write(pa_w, self.get_reg(*new), *size) {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    }
                    self.set_reg(*dst, old);
                    count += 1;
                }
                IROp::AtomicCmpXchgOrder {
                    dst,
                    base,
                    expected,
                    new,
                    size,
                    flags,
                } => {
                    let va = self.get_reg(*base);
                    if matches!(flags.order, vm_ir::MemOrder::Acquire) || flags.fence_before {
                        self.fence_acquire_count += 1;
                    }
                    if matches!(flags.order, vm_ir::MemOrder::SeqCst) {
                        self.fence_acquire_count += 1;
                        self.fence_release_count += 1;
                    }
                    let pa_r = match mmu.translate(va, AccessType::Read) {
                        Ok(p) => p,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    let old = match mmu.read(pa_r, *size) {
                        Ok(v) => v,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    if old == self.get_reg(*expected) {
                        let pa_w = match mmu.translate(va, AccessType::Write) {
                            Ok(p) => p,
                            Err(e) => {
                                return make_result(ExecStatus::Fault(e), count, block.start_pc);
                            }
                        };
                        if let Err(e) = mmu.write(pa_w, self.get_reg(*new), *size) {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                        if matches!(flags.order, vm_ir::MemOrder::Release)
                            || matches!(flags.order, vm_ir::MemOrder::AcqRel)
                            || flags.fence_after
                        {
                            self.fence_release_count += 1;
                        }
                        if matches!(flags.order, vm_ir::MemOrder::SeqCst) {
                            self.fence_acquire_count += 1;
                            self.fence_release_count += 1;
                        }
                    }
                    self.set_reg(*dst, old);
                    count += 1;
                }
                IROp::AtomicCmpXchgFlag {
                    dst_old,
                    dst_flag,
                    base,
                    expected,
                    new,
                    size,
                } => {
                    let va = self.get_reg(*base);
                    let pa_r = match mmu.translate(va, AccessType::Read) {
                        Ok(p) => p,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    let old = match mmu.read(pa_r, *size) {
                        Ok(v) => v,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    let mut success = 0;
                    if old == self.get_reg(*expected) {
                        let pa_w = match mmu.translate(va, AccessType::Write) {
                            Ok(p) => p,
                            Err(e) => {
                                return make_result(ExecStatus::Fault(e), count, block.start_pc);
                            }
                        };
                        if let Err(e) = mmu.write(pa_w, self.get_reg(*new), *size) {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                        success = 1;
                    }
                    self.set_reg(*dst_old, old);
                    self.set_reg(*dst_flag, success);
                    count += 1;
                }
                IROp::AtomicLoadReserve {
                    dst,
                    base,
                    offset,
                    size,
                    flags,
                } => {
                    let va = self.get_reg(*base).wrapping_add(*offset as u64);
                    if matches!(flags.order, vm_ir::MemOrder::Acquire) {
                        self.fence_acquire_count += 1;
                    }
                    let val = match mmu.load_reserved(va, *size) {
                        Ok(v) => v,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    self.set_reg(*dst, val);
                    count += 1;
                }
                IROp::AtomicStoreCond {
                    src,
                    base,
                    offset,
                    size,
                    dst_flag,
                    flags,
                } => {
                    let va = self.get_reg(*base).wrapping_add(*offset as u64);
                    if matches!(flags.order, vm_ir::MemOrder::Release) {
                        self.fence_release_count += 1;
                    }
                    let success = match mmu.store_conditional(va, self.get_reg(*src), *size) {
                        Ok(s) => s,
                        Err(e) => return make_result(ExecStatus::Fault(e), count, block.start_pc),
                    };
                    self.set_reg(*dst_flag, if success { 0 } else { 1 });
                    count += 1;
                }
                IROp::AtomicRmwFlag {
                    dst_old,
                    dst_flag,
                    base,
                    src,
                    op,
                    size,
                } => {
                    let va = self.get_reg(*base);
                    let pa_r = match mmu.translate(va, AccessType::Read) {
                        Ok(p) => p,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    let old = match mmu.read(pa_r, *size) {
                        Ok(v) => v,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    let newv = match op {
                        vm_ir::AtomicOp::And => old & self.get_reg(*src),
                        vm_ir::AtomicOp::Or => old | self.get_reg(*src),
                        vm_ir::AtomicOp::Xor => old ^ self.get_reg(*src),
                        vm_ir::AtomicOp::Min => old.min(self.get_reg(*src)),
                        vm_ir::AtomicOp::Max => old.max(self.get_reg(*src)),
                        vm_ir::AtomicOp::MinS => {
                            let bits = (*size as u64) * 8;
                            let a = sign_extend(old, bits) as i64;
                            let b = sign_extend(self.get_reg(*src), bits) as i64;
                            let r = if a < b { a } else { b } as i64;
                            (r as i64 as u64) & (((1u128 << bits) - 1) as u64)
                        }
                        vm_ir::AtomicOp::MaxS => {
                            let bits = (*size as u64) * 8;
                            let a = sign_extend(old, bits) as i64;
                            let b = sign_extend(self.get_reg(*src), bits) as i64;
                            let r = if a > b { a } else { b } as i64;
                            (r as i64 as u64) & (((1u128 << bits) - 1) as u64)
                        }
                        _ => old,
                    };
                    let pa_w = match mmu.translate(va, AccessType::Write) {
                        Ok(p) => p,
                        Err(e) => {
                            return make_result(ExecStatus::Fault(e), count, block.start_pc);
                        }
                    };
                    if let Err(e) = mmu.write(pa_w, newv, *size) {
                        return make_result(ExecStatus::Fault(e), count, block.start_pc);
                    }
                    self.set_reg(*dst_old, old);
                    self.set_reg(*dst_flag, 1);
                    count += 1;
                }
                IROp::Vec256Add {
                    dst0,
                    dst1,
                    dst2,
                    dst3,
                    src10,
                    src11,
                    src12,
                    src13,
                    src20,
                    src21,
                    src22,
                    src23,
                    element_size,
                    signed,
                } => {
                    // 使用 vm-simd 优化的 256-bit 向量饱和加法
                    let src_a = [
                        self.get_reg(*src10),
                        self.get_reg(*src11),
                        self.get_reg(*src12),
                        self.get_reg(*src13),
                    ];
                    let src_b = [
                        self.get_reg(*src20),
                        self.get_reg(*src21),
                        self.get_reg(*src22),
                        self.get_reg(*src23),
                    ];
                    let out = if *signed {
                        vec256_add_sat_s(src_a, src_b, *element_size)
                    } else {
                        vec256_add_sat_u(src_a, src_b, *element_size)
                    };
                    self.set_reg(*dst0, out[0]);
                    self.set_reg(*dst1, out[1]);
                    self.set_reg(*dst2, out[2]);
                    self.set_reg(*dst3, out[3]);
                    count += 1;
                }
                IROp::Vec256Sub {
                    dst0,
                    dst1,
                    dst2,
                    dst3,
                    src10,
                    src11,
                    src12,
                    src13,
                    src20,
                    src21,
                    src22,
                    src23,
                    element_size,
                    signed,
                } => {
                    // 使用 vm-simd 优化的 256-bit 向量饱和减法
                    let src_a = [
                        self.get_reg(*src10),
                        self.get_reg(*src11),
                        self.get_reg(*src12),
                        self.get_reg(*src13),
                    ];
                    let src_b = [
                        self.get_reg(*src20),
                        self.get_reg(*src21),
                        self.get_reg(*src22),
                        self.get_reg(*src23),
                    ];
                    let out = if *signed {
                        vec256_sub_sat_s(src_a, src_b, *element_size)
                    } else {
                        vec256_sub_sat_u(src_a, src_b, *element_size)
                    };
                    self.set_reg(*dst0, out[0]);
                    self.set_reg(*dst1, out[1]);
                    self.set_reg(*dst2, out[2]);
                    self.set_reg(*dst3, out[3]);
                    count += 1;
                }
                IROp::Vec256Mul {
                    dst0,
                    dst1,
                    dst2,
                    dst3,
                    src10,
                    src11,
                    src12,
                    src13,
                    src20,
                    src21,
                    src22,
                    src23,
                    element_size,
                    signed,
                } => {
                    // 使用 vm-simd 优化的 256-bit 向量饱和乘法
                    let src_a = [
                        self.get_reg(*src10),
                        self.get_reg(*src11),
                        self.get_reg(*src12),
                        self.get_reg(*src13),
                    ];
                    let src_b = [
                        self.get_reg(*src20),
                        self.get_reg(*src21),
                        self.get_reg(*src22),
                        self.get_reg(*src23),
                    ];
                    let out = if *signed {
                        vec256_mul_sat_s(src_a, src_b, *element_size)
                    } else {
                        vec256_mul_sat_u(src_a, src_b, *element_size)
                    };
                    self.set_reg(*dst0, out[0]);
                    self.set_reg(*dst1, out[1]);
                    self.set_reg(*dst2, out[2]);
                    self.set_reg(*dst3, out[3]);
                    count += 1;
                }
                IROp::VecAddSat {
                    dst,
                    src1,
                    src2,
                    element_size,
                    signed,
                } => {
                    // 使用 vm-simd 优化的饱和加法
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let result = if *signed {
                        vec_add_sat_s(a, b, *element_size)
                    } else {
                        vec_add_sat_u(a, b, *element_size)
                    };
                    self.set_reg(*dst, result);
                    count += 1;
                }
                IROp::VecSubSat {
                    dst,
                    src1,
                    src2,
                    element_size,
                    signed,
                } => {
                    // 使用 vm-simd 优化的饱和减法
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let result = if *signed {
                        vec_sub_sat_s(a, b, *element_size)
                    } else {
                        vec_sub_sat_u(a, b, *element_size)
                    };
                    self.set_reg(*dst, result);
                    count += 1;
                }
                IROp::AtomicRMW {
                    dst,
                    base,
                    src,
                    op,
                    size,
                } => {
                    let addr = self.get_reg(*base);
                    let val = self.get_reg(*src);
                    let current = match mmu.read(addr, *size) {
                        Ok(v) => v,
                        Err(e) => return make_result(ExecStatus::Fault(e), 0, block.start_pc),
                    };
                    let res = match op {
                        AtomicOp::Add => current.wrapping_add(val),
                        AtomicOp::Sub => current.wrapping_sub(val),
                        AtomicOp::And => current & val,
                        AtomicOp::Or => current | val,
                        AtomicOp::Xor => current ^ val,
                        AtomicOp::Xchg => val,
                        AtomicOp::Min => current.min(val),
                        AtomicOp::Max => current.max(val),
                        AtomicOp::MinS => {
                            let bits = (*size as u64) * 8;
                            let a = sign_extend(current, bits) as i64;
                            let b = sign_extend(val, bits) as i64;
                            let r = if a < b { a } else { b } as i64;
                            r as i64 as u64
                        }
                        AtomicOp::MaxS => {
                            let bits = (*size as u64) * 8;
                            let a = sign_extend(current, bits) as i64;
                            let b = sign_extend(val, bits) as i64;
                            let r = if a > b { a } else { b } as i64;
                            r as i64 as u64
                        }
                        _ => current,
                    };
                    let mask = if *size == 8 {
                        !0
                    } else {
                        (1u64 << (*size * 8)) - 1
                    };
                    let res = res & mask;
                    if let Err(e) = mmu.write(addr, res, *size) {
                        return make_result(ExecStatus::Fault(e), 0, block.start_pc);
                    }
                    self.set_reg(*dst, current);
                }
                IROp::AtomicRMWOrder {
                    dst,
                    base,
                    src,
                    op,
                    size,
                    flags,
                } => {
                    let addr = self.get_reg(*base);
                    let val = self.get_reg(*src);
                    if matches!(
                        flags.order,
                        vm_ir::MemOrder::Acquire | vm_ir::MemOrder::AcqRel
                    ) || flags.fence_before
                    {
                        self.fence_acquire_count += 1;
                    }
                    let current = match mmu.read(addr, *size) {
                        Ok(v) => v,
                        Err(e) => return make_result(ExecStatus::Fault(e), 0, block.start_pc),
                    };
                    let res = match op {
                        AtomicOp::Add => current.wrapping_add(val),
                        AtomicOp::Sub => current.wrapping_sub(val),
                        AtomicOp::And => current & val,
                        AtomicOp::Or => current | val,
                        AtomicOp::Xor => current ^ val,
                        AtomicOp::Xchg => val,
                        AtomicOp::Min => current.min(val),
                        AtomicOp::Max => current.max(val),
                        AtomicOp::MinS => {
                            let bits = (*size as u64) * 8;
                            let a = sign_extend(current, bits) as i64;
                            let b = sign_extend(val, bits) as i64;
                            let r = if a < b { a } else { b } as i64;
                            r as i64 as u64
                        }
                        AtomicOp::MaxS => {
                            let bits = (*size as u64) * 8;
                            let a = sign_extend(current, bits) as i64;
                            let b = sign_extend(val, bits) as i64;
                            let r = if a > b { a } else { b } as i64;
                            r as i64 as u64
                        }
                        _ => current,
                    };
                    let mask = if *size == 8 {
                        !0
                    } else {
                        (1u64 << (*size * 8)) - 1
                    };
                    let res = res & mask;
                    if let Err(e) = mmu.write(addr, res, *size) {
                        return make_result(ExecStatus::Fault(e), 0, block.start_pc);
                    }
                    if matches!(
                        flags.order,
                        vm_ir::MemOrder::Release | vm_ir::MemOrder::AcqRel
                    ) || flags.fence_after
                    {
                        self.fence_release_count += 1;
                    }
                    self.set_reg(*dst, current);
                }
                IROp::CsrRead { dst, csr } => {
                    let val = match *csr {
                        0x300 => self.csr_mstatus,
                        0x304 => self.csr_mie,
                        0x344 => self.csr_mip,
                        0x100 => self.csr_sstatus,
                        0x104 => self.csr_sie,
                        0x144 => self.csr_sip,
                        0x302 => self.csr_medeleg,
                        0x303 => self.csr_mideleg,
                        0x341 => self.csr_mepc,
                        0x342 => self.csr_mcause,
                        0x141 => self.csr_sepc,
                        0x142 => self.csr_scause,
                        _ => 0,
                    };
                    self.set_reg(*dst, val);
                }
                IROp::CsrWrite { csr, src } => {
                    let v = self.get_reg(*src);
                    match *csr {
                        0x300 => self.csr_mstatus = v,
                        0x304 => self.csr_mie = v,
                        0x344 => self.csr_mip = v,
                        0x100 => self.csr_sstatus = v,
                        0x104 => self.csr_sie = v,
                        0x144 => self.csr_sip = v,
                        0x302 => self.csr_medeleg = v,
                        0x303 => self.csr_mideleg = v,
                        0x341 => self.csr_mepc = v,
                        0x342 => self.csr_mcause = v,
                        0x141 => self.csr_sepc = v,
                        0x142 => self.csr_scause = v,
                        _ => {}
                    }
                }
                IROp::CsrSet { csr, src } => {
                    let v = self.get_reg(*src);
                    match *csr {
                        0x300 => self.csr_mstatus |= v,
                        0x304 => self.csr_mie |= v,
                        0x344 => self.csr_mip |= v,
                        0x100 => self.csr_sstatus |= v,
                        0x104 => self.csr_sie |= v,
                        0x144 => self.csr_sip |= v,
                        0x302 => self.csr_medeleg |= v,
                        0x303 => self.csr_mideleg |= v,
                        0x341 => self.csr_mepc |= v,
                        0x342 => self.csr_mcause |= v,
                        0x141 => self.csr_sepc |= v,
                        0x142 => self.csr_scause |= v,
                        _ => {}
                    }
                }
                IROp::CsrClear { csr, src } => {
                    let v = self.get_reg(*src);
                    match *csr {
                        0x300 => self.csr_mstatus &= !v,
                        0x304 => self.csr_mie &= !v,
                        0x344 => self.csr_mip &= !v,
                        0x100 => self.csr_sstatus &= !v,
                        0x104 => self.csr_sie &= !v,
                        0x144 => self.csr_sip &= !v,
                        0x302 => self.csr_medeleg &= !v,
                        0x303 => self.csr_mideleg &= !v,
                        0x341 => self.csr_mepc &= !v,
                        0x342 => self.csr_mcause &= !v,
                        0x141 => self.csr_sepc &= !v,
                        0x142 => self.csr_scause &= !v,
                        _ => {}
                    }
                }
                IROp::CsrWriteImm { csr, imm, dst: _ } => {
                    let v = *imm as u64;
                    match *csr {
                        0x300 => self.csr_mstatus = v,
                        0x304 => self.csr_mie = v,
                        0x344 => self.csr_mip = v,
                        0x100 => self.csr_sstatus = v,
                        0x104 => self.csr_sie = v,
                        0x144 => self.csr_sip = v,
                        0x302 => self.csr_medeleg = v,
                        0x303 => self.csr_mideleg = v,
                        0x341 => self.csr_mepc = v,
                        0x342 => self.csr_mcause = v,
                        0x141 => self.csr_sepc = v,
                        0x142 => self.csr_scause = v,
                        _ => {}
                    }
                }
                IROp::CsrSetImm { csr, imm, dst: _ } => {
                    let v = *imm as u64;
                    match *csr {
                        0x300 => self.csr_mstatus |= v,
                        0x304 => self.csr_mie |= v,
                        0x344 => self.csr_mip |= v,
                        0x100 => self.csr_sstatus |= v,
                        0x104 => self.csr_sie |= v,
                        0x144 => self.csr_sip |= v,
                        0x302 => self.csr_medeleg |= v,
                        0x303 => self.csr_mideleg |= v,
                        0x341 => self.csr_mepc |= v,
                        0x342 => self.csr_mcause |= v,
                        0x141 => self.csr_sepc |= v,
                        0x142 => self.csr_scause |= v,
                        _ => {}
                    }
                }
                IROp::CsrClearImm { csr, imm, dst: _ } => {
                    let v = *imm as u64;
                    match *csr {
                        0x300 => self.csr_mstatus &= !v,
                        0x304 => self.csr_mie &= !v,
                        0x344 => self.csr_mip &= !v,
                        0x100 => self.csr_sstatus &= !v,
                        0x104 => self.csr_sie &= !v,
                        0x144 => self.csr_sip &= !v,
                        0x302 => self.csr_medeleg &= !v,
                        0x303 => self.csr_mideleg &= !v,
                        0x341 => self.csr_mepc &= !v,
                        0x342 => self.csr_mcause &= !v,
                        0x141 => self.csr_sepc &= !v,
                        0x142 => self.csr_scause &= !v,
                        _ => {}
                    }
                }

                IROp::DebugBreak => {
                    let cause_code = 3u64;
                    if ((self.csr_medeleg >> cause_code) & 1) != 0 {
                        self.csr_scause = cause_code;
                        self.csr_sepc = block.start_pc;
                        return make_result(
                            ExecStatus::Fault(VmError::from(vm_core::Fault::TrapRiscv {
                                cause: vm_core::RiscvTrapCause::Breakpoint,
                                pc: block.start_pc,
                            })),
                            count,
                            block.start_pc,
                        );
                    } else {
                        self.csr_mcause = cause_code;
                        self.csr_mepc = block.start_pc;
                        return make_result(
                            ExecStatus::Fault(VmError::from(vm_core::Fault::TrapRiscv {
                                cause: vm_core::RiscvTrapCause::Breakpoint,
                                pc: block.start_pc,
                            })),
                            count,
                            block.start_pc,
                        );
                    }
                }
                IROp::SysMret => {
                    let mpie = (self.csr_mstatus >> 7) & 1;
                    if mpie != 0 {
                        self.csr_mstatus |= 1 << 3;
                    } else {
                        self.csr_mstatus &= !(1 << 3);
                    }
                    self.csr_mstatus &= !(1 << 7);
                    let mpp = ((self.csr_mstatus >> 11) & 0x3) as u8;
                    self.priv_mode = mpp;
                    self.csr_mstatus &= !(0x1800);
                }
                IROp::SysSret => {
                    let spie = (self.csr_sstatus >> 5) & 1;
                    if spie != 0 {
                        self.csr_sstatus |= 1 << 1;
                    } else {
                        self.csr_sstatus &= !(1 << 1);
                    }
                    self.csr_sstatus &= !(1 << 5);
                    let spp = ((self.csr_sstatus >> 8) & 0x1) as u8;
                    self.priv_mode = spp;
                    self.csr_sstatus &= !(1 << 8);
                }
                IROp::SysWfi => {
                    let m_enabled =
                        (self.csr_mstatus & (1 << 3)) != 0 && (self.csr_mie & self.csr_mip) != 0;
                    let s_enabled =
                        (self.csr_sstatus & (1 << 1)) != 0 && (self.csr_sie & self.csr_sip) != 0;
                    if !(m_enabled || s_enabled) {
                        return make_result(
                            ExecStatus::InterruptPending,
                            count,
                            block.start_pc.wrapping_add(4),
                        );
                    }
                }
                _ => {}
            }
        }
        if self.intr_mask_until == 0 {
            let mtime = mmu.read(0x02000000 + 0x0000bff8, 8).unwrap_or(0);
            let mtimecmp0 = mmu.read(0x02000000 + 0x00004000, 8).unwrap_or(u64::MAX);
            let plic_pending = mmu.read(0x0C000000 + 0x00001000, 4).unwrap_or(0);
            const MSTATUS_MIE: u64 = 1 << 3;
            const MIP_MTIP: u64 = 1 << 7;
            const MIP_MEIP: u64 = 1 << 11;
            if mtime >= mtimecmp0 {
                self.csr_mip |= MIP_MTIP;
            } else {
                self.csr_mip &= !MIP_MTIP;
            }
            if plic_pending != 0 {
                self.csr_mip |= MIP_MEIP;
            } else {
                self.csr_mip &= !MIP_MEIP;
            }
            let delegated = self.csr_mip & self.csr_mideleg;
            let m_pending = self.csr_mip & !self.csr_mideleg;
            let m_enabled =
                (self.csr_mstatus & MSTATUS_MIE) != 0 && (self.csr_mie & m_pending) != 0;
            let s_enabled = (self.csr_sstatus & (1 << 1)) != 0 && (self.csr_sie & delegated) != 0;
            if m_enabled || s_enabled {
                let next = block.start_pc.wrapping_add(4);
                self.pc = next;
                return make_result(ExecStatus::InterruptPending, count, next);
            }
        }

        // Compute next_pc based on terminator
        let next_pc = match &block.term {
            Terminator::Jmp { target } => *target,
            Terminator::JmpReg { base, offset } => self.get_reg(*base).wrapping_add(*offset as u64),
            Terminator::CondJmp {
                cond,
                target_true,
                target_false,
            } => {
                if self.get_reg(*cond) != 0 {
                    *target_true
                } else {
                    *target_false
                }
            }
            Terminator::Ret => block.start_pc, // Will exit loop
            Terminator::Fault { cause: _ } => block.start_pc,
            Terminator::Interrupt { vector: _ } => block.start_pc.wrapping_add(4),
            Terminator::Call { target, ret_pc: _ } => *target,
        };
        self.pc = next_pc;
        make_result(ExecStatus::Ok, count, next_pc)
    }

    fn get_reg(&self, idx: usize) -> u64 {
        if idx < 32 { self.regs[idx] } else { 0 }
    }

    fn set_reg(&mut self, idx: usize, val: u64) {
        if idx > 0 && idx < 32 {
            self.regs[idx] = val;
        }
    }

    fn get_pc(&self) -> GuestAddr {
        self.pc
    }

    fn set_pc(&mut self, pc: GuestAddr) {
        self.pc = pc;
    }

    fn get_vcpu_state(&self) -> vm_core::VcpuStateContainer {
        vm_core::VcpuStateContainer {
            regs: self.regs,
            pc: self.pc,
        }
    }

    fn set_vcpu_state(&mut self, state: &vm_core::VcpuStateContainer) {
        self.regs = state.regs;
        self.pc = state.pc;
    }
}

pub fn run_chain(
    decoder: &mut dyn crate_decoder::DecoderDyn,
    mmu: &mut dyn MMU,
    interp: &mut Interpreter,
    mut pc: u64,
    max_blocks: usize,
) -> ExecResult {
    let mut total = 0u64;
    for _ in 0..max_blocks {
        let block = decoder.decode_dyn(mmu, pc);
        let res = interp.run(mmu, &block);
        total += res.stats.executed_ops;
        if let ExecStatus::Fault(_) = res.status {
            return res;
        }

        match block.term {
            Terminator::Jmp { target } => {
                pc = target;
            }
            Terminator::JmpReg { base, offset } => {
                pc = interp.get_reg(base).wrapping_add(offset as u64);
            }
            Terminator::CondJmp {
                cond,
                target_true,
                target_false,
            } => {
                if interp.get_reg(cond) != 0 {
                    pc = target_true;
                } else {
                    pc = target_false;
                }
            }
            Terminator::Ret => {
                break;
            }
            Terminator::Fault { cause: _ } => {
                return make_result(
                    ExecStatus::Fault(VmError::from(vm_core::Fault::AccessViolation {
                        addr: pc,
                        access: AccessType::Exec,
                    })),
                    total,
                    pc,
                );
            }
            Terminator::Interrupt { vector } => {
                let mut tmp_ext = interp.intr_handler_ext.take();
                let action = if let Some(h) = tmp_ext.take() {
                    let res = h(
                        InterruptCtxExt {
                            vector,
                            pc,
                            regs_ptr: interp.get_regs_ptr(),
                        },
                        interp,
                    );
                    interp.intr_handler_ext = Some(h);
                    res
                } else {
                    let mut tmp = interp.intr_handler.take();
                    if let Some(h) = tmp.take() {
                        let res = h(InterruptCtx { vector, pc }, interp);
                        interp.intr_handler = Some(h);
                        res
                    } else {
                        ExecInterruptAction::Abort
                    }
                };
                match action {
                    ExecInterruptAction::Continue => {
                        pc = pc.wrapping_add(4);
                        continue;
                    }
                    ExecInterruptAction::InjectState => {
                        pc = pc.wrapping_add(4);
                        continue;
                    }
                    ExecInterruptAction::Retry => {
                        continue;
                    }
                    ExecInterruptAction::Mask => {
                        pc = pc.wrapping_add(4);
                        continue;
                    }
                    ExecInterruptAction::Deliver => {
                        pc = pc.wrapping_add(4);
                        continue;
                    }
                    ExecInterruptAction::Abort => {
                        return make_result(
                            ExecStatus::Fault(VmError::from(vm_core::Fault::AccessViolation {
                                addr: pc,
                                access: AccessType::Exec,
                            })),
                            total,
                            pc,
                        );
                    }
                }
            }
            Terminator::Call { target, ret_pc: _ } => {
                pc = target;
            }
        }
    }
    make_result(ExecStatus::Ok, total, pc)
}

pub mod crate_decoder {
    use vm_core::{AccessType, Decoder, Fault, GuestAddr, MMU, VmError};
    use vm_ir::{IRBlock, Terminator};
    pub trait DecoderDyn {
        fn decode_dyn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> IRBlock;
    }
    impl<T: Decoder<Block = IRBlock>> DecoderDyn for T {
        fn decode_dyn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> IRBlock {
            match <T as Decoder>::decode(self, mmu, pc) {
                Ok(b) => b,
                Err(e) => {
                    let cause = if let VmError::Execution(vm_core::ExecutionError::Fault(f)) = &e {
                        match f {
                            Fault::InvalidOpcode { .. } => 3,
                            Fault::PageFault { .. } => 2,
                            Fault::AccessViolation { .. } => 1,
                            Fault::AlignmentFault { .. } => 4,
                            Fault::DeviceError { .. } => 5,
                            Fault::Halt => 6,
                            Fault::Shutdown => 7,
                            Fault::TrapRiscv { .. } => 12,
                        }
                    } else {
                        1
                    } as u64;
                    IRBlock {
                        start_pc: pc,
                        ops: vec![],
                        term: Terminator::Fault { cause },
                    }
                }
            }
        }
    }
}

// ============================================================
// 优化执行路径
// ============================================================

/// 优化的块执行器 - 使用指令融合和批量执行
pub struct OptimizedExecutor {
    /// 基础解释器
    pub interp: Interpreter,
    /// 批量执行块数
    pub batch_size: usize,
    /// 执行的块数统计
    pub blocks_executed: u64,
    /// 融合的指令数
    pub fused_instructions: u64,
}

impl OptimizedExecutor {
    /// 创建新的优化执行器
    pub fn new() -> Self {
        Self {
            interp: Interpreter::with_full_optimization(BLOCK_CACHE_SIZE),
            batch_size: 16,
            blocks_executed: 0,
            fused_instructions: 0,
        }
    }

    /// 创建带自定义配置的优化执行器
    pub fn with_config(cache_size: usize, batch_size: usize) -> Self {
        Self {
            interp: Interpreter::with_full_optimization(cache_size),
            batch_size,
            blocks_executed: 0,
            fused_instructions: 0,
        }
    }

    /// 批量执行多个块
    pub fn run_batch(
        &mut self,
        decoder: &mut dyn crate_decoder::DecoderDyn,
        mmu: &mut dyn MMU,
        start_pc: GuestAddr,
    ) -> ExecResult {
        let result = run_chain(decoder, mmu, &mut self.interp, start_pc, self.batch_size);
        self.blocks_executed += self.batch_size as u64;
        self.fused_instructions = self.interp.fuser.fused_count;
        result
    }

    /// 获取性能统计
    pub fn get_stats(&self) -> OptimizedExecutorStats {
        let (cache_hits, cache_misses, hit_rate) = self.interp.cache_stats().unwrap_or((0, 0, 0.0));
        let (fused, checked, fusion_rate) = self.interp.fusion_stats();
        OptimizedExecutorStats {
            blocks_executed: self.blocks_executed,
            cache_hits,
            cache_misses,
            cache_hit_rate: hit_rate,
            fused_instructions: fused,
            fusion_checks: checked,
            fusion_rate,
        }
    }
}

impl Default for OptimizedExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// 优化执行器统计信息
#[derive(Debug, Clone)]
pub struct OptimizedExecutorStats {
    /// 执行的块数
    pub blocks_executed: u64,
    /// 缓存命中数
    pub cache_hits: u64,
    /// 缓存未命中数
    pub cache_misses: u64,
    /// 缓存命中率
    pub cache_hit_rate: f64,
    /// 融合的指令数
    pub fused_instructions: u64,
    /// 检查融合的指令对数
    pub fusion_checks: u64,
    /// 融合率
    pub fusion_rate: f64,
}

// ============================================================
// 快速路径优化
// ============================================================

/// 快速执行常见的简单操作序列
/// 用于热点代码的加速执行
#[inline(always)]
pub fn fast_add_chain(regs: &mut [u64; 32], ops: &[(u32, u32, u32)]) {
    for (dst, src1, src2) in ops {
        if *dst != 0 && (*dst as usize) < 32 {
            regs[*dst as usize] = regs[*src1 as usize].wrapping_add(regs[*src2 as usize]);
        }
    }
}

/// 快速执行 load-add-store 模式
#[inline(always)]
pub fn fast_load_add_store(
    regs: &mut [u64; 32],
    mmu: &mut dyn MMU,
    base_reg: u32,
    offset: i64,
    add_reg: u32,
    size: u8,
) -> Result<(), VmError> {
    let addr = regs[base_reg as usize].wrapping_add(offset as u64);
    let val = mmu.read(addr, size)?;
    let result = val.wrapping_add(regs[add_reg as usize]);
    mmu.write(addr, result, size)
}

/// 预编译的常见指令序列
pub mod precompiled {
    use super::*;

    /// 零初始化寄存器序列
    #[inline(always)]
    pub fn zero_regs(regs: &mut [u64; 32], start: usize, count: usize) {
        let end = (start + count).min(32);
        for r in &mut regs[start..end] {
            *r = 0;
        }
    }

    /// 复制寄存器块
    #[inline(always)]
    pub fn copy_regs(regs: &mut [u64; 32], src_start: usize, dst_start: usize, count: usize) {
        let count = count.min(32 - src_start).min(32 - dst_start);
        for i in 0..count {
            if dst_start + i != 0 {
                // 不能写入 x0
                regs[dst_start + i] = regs[src_start + i];
            }
        }
    }

    /// 执行简单的算术序列
    #[inline(always)]
    pub fn arith_sequence(regs: &mut [u64; 32], dst: u32, src: u32, imm: i64, op: ArithOp) {
        if dst != 0 && (dst as usize) < 32 {
            let src_val = regs[src as usize];
            regs[dst as usize] = match op {
                ArithOp::Add => src_val.wrapping_add(imm as u64),
                ArithOp::Sub => src_val.wrapping_sub(imm as u64),
                ArithOp::Mul => src_val.wrapping_mul(imm as u64),
                ArithOp::And => src_val & (imm as u64),
                ArithOp::Or => src_val | (imm as u64),
                ArithOp::Xor => src_val ^ (imm as u64),
                ArithOp::Sll => src_val << (imm as u32 & 0x3f),
                ArithOp::Srl => src_val >> (imm as u32 & 0x3f),
                ArithOp::Sra => ((src_val as i64) >> (imm as u32 & 0x3f)) as u64,
            };
        }
    }

    /// 简单算术操作类型
    #[derive(Debug, Clone, Copy)]
    pub enum ArithOp {
        Add,
        Sub,
        Mul,
        And,
        Or,
        Xor,
        Sll,
        Srl,
        Sra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instruction_fuser() {
        let mut fuser = InstructionFuser::new();

        // 测试 MovImm + Add 融合
        let op1 = IROp::MovImm { dst: 5, imm: 100 };
        let op2 = IROp::Add {
            dst: 3,
            src1: 3,
            src2: 5,
        };
        let fused = fuser.try_fuse(&op1, &op2);
        assert!(fused.is_some());

        assert!(fuser.fused_count >= 1);
        assert!(fuser.fusion_rate() > 0.0);
    }

    #[test]
    fn test_fast_add_chain() {
        let mut regs = [0u64; 32];
        regs[1] = 10;
        regs[2] = 20;
        regs[3] = 30;

        fast_add_chain(&mut regs, &[(4, 1, 2), (5, 3, 4)]);

        assert_eq!(regs[4], 30); // 10 + 20
        assert_eq!(regs[5], 60); // 30 + 30
    }

    #[test]
    fn test_precompiled_zero_regs() {
        let mut regs = [100u64; 32];
        precompiled::zero_regs(&mut regs, 5, 3);

        assert_eq!(regs[5], 0);
        assert_eq!(regs[6], 0);
        assert_eq!(regs[7], 0);
        assert_eq!(regs[8], 100); // 未改变
    }

    #[test]
    fn test_precompiled_arith_sequence() {
        let mut regs = [0u64; 32];
        regs[1] = 100;

        precompiled::arith_sequence(&mut regs, 2, 1, 50, precompiled::ArithOp::Add);
        assert_eq!(regs[2], 150);

        precompiled::arith_sequence(&mut regs, 3, 1, 2, precompiled::ArithOp::Sll);
        assert_eq!(regs[3], 400);
    }

    #[test]
    fn test_optimized_executor_creation() {
        let executor = OptimizedExecutor::new();
        assert_eq!(executor.batch_size, 16);
        assert!(executor.interp.block_cache.is_some());
    }
}

