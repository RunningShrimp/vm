//! 寄存器映射优化器
//!
//! 提供跨架构寄存器映射的快速查找表，降低寄存器映射开销。
//!
//! ## 性能目标
//!
//! - 寄存器映射开销降低 50%+
//! - 使用哈希表或数组索引实现 O(1) 查找
//! - 缓存常见映射以减少重复计算
//!
//! ## 架构支持
//!
//! - x86_64 ↔ ARM64
//! - x86_64 ↔ RISC-V
//! - ARM64 ↔ RISC-V
//! - 任意架构组合

use std::collections::HashMap;

/// 寄存器类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegisterType {
    /// 通用寄存器
    GPR(u8),
    /// 浮点寄存器
    FPR(u8),
    /// 向量寄存器
    Vector(u8),
    /// 特殊寄存器 (PC, SP, FP等)
    Special(SpecialReg),
}

/// 特殊寄存器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialReg {
    PC,
    SP,
    FP,
    RA, // Return Address
    FLAGS,
}

/// 架构类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchType {
    X86_64,
    ARM64,
    RiscV64,
}

/// 寄存器映射键（公共API以支持get_hot_registers）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegMapKey {
    src_arch: ArchType,
    dst_arch: ArchType,
    src_reg: RegisterType,
}

/// 寄存器映射器
///
/// 使用三层缓存策略：
/// 1. L1: 最近使用的映射 (LRU，32条目)
/// 2. L2: 架构间映射表 (完全缓存)
/// 3. L3: 动态计算的映射
///
/// ## 优化特性
///
/// - 热寄存器跟踪：记录频繁使用的寄存器
/// - 自适应预热：基于热路径预加载映射
/// - 批量优化：使用并行处理大型批量映射
pub struct RegisterMapper {
    /// L1 缓存：最近使用的映射
    l1_cache: lru::LruCache<RegMapKey, RegisterType>,
    /// L2 缓存：架构间映射表
    l2_cache: HashMap<RegMapKey, RegisterType>,
    /// 缓存命中统计
    stats: MapperStats,
    /// 热寄存器访问频率计数器
    hot_registers: HashMap<RegMapKey, u64>,
    /// 自适应L1缓存大小
    adaptive_l1_size: bool,
    /// L1缓存当前容量
    l1_capacity: usize,
}

/// 映射器统计信息
#[derive(Debug, Clone, Default)]
pub struct MapperStats {
    pub l1_hits: u64,
    pub l2_hits: u64,
    pub l3_computations: u64,
    pub total_lookups: u64,
}

/// 热寄存器统计信息
#[derive(Debug, Clone, Default)]
pub struct HotRegisterStats {
    /// 唯一寄存器数量
    pub unique_registers: u64,
    /// 总访问次数
    pub total_accesses: u64,
    /// 平均访问次数
    pub avg_accesses: u64,
    /// 最大访问次数
    pub max_accesses: u64,
    /// 集中度比率（前10%寄存器的访问占比）
    pub concentration_ratio: f64,
}

impl MapperStats {
    /// 计算总缓存命中率
    pub fn hit_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            return 0.0;
        }
        let hits = self.l1_hits + self.l2_hits;
        hits as f64 / self.total_lookups as f64
    }
}

impl RegisterMapper {
    /// 创建新的寄存器映射器
    pub fn new() -> Self {
        let l1_capacity = 32;
        let mut mapper = Self {
            l1_cache: lru::LruCache::new(l1_capacity),
            l2_cache: HashMap::new(),
            stats: MapperStats::default(),
            hot_registers: HashMap::new(),
            adaptive_l1_size: false,
            l1_capacity,
        };

        // 预填充 L2 缓存：常见架构映射
        mapper.prefill_x86_64_to_arm64();
        mapper.prefill_x86_64_to_riscv();
        mapper.prefill_arm64_to_riscv();

        mapper
    }

    /// 创建启用自适应优化的寄存器映射器
    pub fn with_adaptive_optimization() -> Self {
        let l1_capacity = 32;
        let mut mapper = Self {
            l1_cache: lru::LruCache::new(l1_capacity),
            l2_cache: HashMap::new(),
            stats: MapperStats::default(),
            hot_registers: HashMap::new(),
            adaptive_l1_size: true,
            l1_capacity,
        };

        // 预填充 L2 缓存：常见架构映射
        mapper.prefill_x86_64_to_arm64();
        mapper.prefill_x86_64_to_riscv();
        mapper.prefill_arm64_to_riscv();

        mapper
    }

    /// 映射寄存器从源架构到目标架构
    ///
    /// # 参数
    ///
    /// - `src_arch`: 源架构
    /// - `dst_arch`: 目标架构
    /// - `src_reg`: 源寄存器
    ///
    /// # 返回值
    ///
    /// 返回目标架构的对应寄存器
    pub fn map_register(
        &mut self,
        src_arch: ArchType,
        dst_arch: ArchType,
        src_reg: RegisterType,
    ) -> RegisterType {
        self.stats.total_lookups += 1;

        // 同架构：直接返回
        if src_arch == dst_arch {
            return src_reg;
        }

        let key = RegMapKey {
            src_arch,
            dst_arch,
            src_reg,
        };

        // 跟踪热寄存器访问频率
        *self.hot_registers.entry(key).or_insert(0) += 1;

        // L1 缓存查找
        if let Some(&cached) = self.l1_cache.get(&key) {
            self.stats.l1_hits += 1;
            return cached;
        }

        // L2 缓存查找
        if let Some(&cached) = self.l2_cache.get(&key) {
            self.stats.l2_hits += 1;
            // 提升到 L1
            self.l1_cache.put(key, cached);
            return cached;
        }

        // L3：动态计算
        self.stats.l3_computations += 1;
        let dst_reg = self.compute_mapping(src_arch, dst_arch, src_reg);

        // 缓存结果
        self.l2_cache.insert(key, dst_reg);
        self.l1_cache.put(key, dst_reg);

        dst_reg
    }

    /// 批量映射寄存器
    ///
    /// 对于寄存器列表，使用批量接口以减少缓存查找开销
    pub fn map_registers_batch(
        &mut self,
        src_arch: ArchType,
        dst_arch: ArchType,
        src_regs: &[RegisterType],
    ) -> Vec<RegisterType> {
        // 对于小批量，使用串行处理
        if src_regs.len() < 100 {
            return src_regs
                .iter()
                .map(|&reg| self.map_register(src_arch, dst_arch, reg))
                .collect();
        }

        // 对于大批量，使用优化的并行处理
        self.map_registers_batch_parallel(src_arch, dst_arch, src_regs)
    }

    /// 批量映射寄存器（并行优化版本）
    ///
    /// 使用rayon并行处理大批量寄存器映射（100+个寄存器）
    #[allow(unexpected_cfgs)]
    #[cfg(feature = "parallel")]
    fn map_registers_batch_parallel(
        &mut self,
        src_arch: ArchType,
        dst_arch: ArchType,
        src_regs: &[RegisterType],
    ) -> Vec<RegisterType> {
        use rayon::prelude::*;

        // 对于并行处理，我们需要clone缓存来避免锁竞争
        // 这是一个简化的实现，实际中可能需要更复杂的同步策略
        src_regs
            .par_iter()
            .map(|&reg| {
                // 并行映射每个寄存器
                // 注意：这会导致热寄存器跟踪不准确，但在大批量场景下可以接受
                let key = RegMapKey {
                    src_arch,
                    dst_arch,
                    src_reg: reg,
                };

                // 优先从L2缓存查找（避免锁竞争）
                if let Some(&cached) = self.l2_cache.get(&key) {
                    return cached;
                }

                // 回退到计算
                self.compute_mapping(src_arch, dst_arch, reg)
            })
            .collect()
    }

    /// 批量映射寄存器（串行版本，作为默认实现）
    #[allow(unexpected_cfgs)]
    #[cfg(not(feature = "parallel"))]
    fn map_registers_batch_parallel(
        &mut self,
        src_arch: ArchType,
        dst_arch: ArchType,
        src_regs: &[RegisterType],
    ) -> Vec<RegisterType> {
        src_regs
            .iter()
            .map(|&reg| self.map_register(src_arch, dst_arch, reg))
            .collect()
    }

    /// 自适应预热：基于热寄存器预加载L1缓存
    ///
    /// 分析当前热寄存器使用模式，并将最频繁的映射预加载到L1缓存
    ///
    /// # 参数
    ///
    /// - `top_n`: 预热的热寄存器数量（默认为L1容量的80%）
    pub fn adaptive_warmup(&mut self, top_n: Option<usize>) {
        if !self.adaptive_l1_size {
            return;
        }

        // 确定预热的寄存器数量
        let n = top_n.unwrap_or((self.l1_capacity * 4) / 5);

        // 按访问频率排序热寄存器
        let mut hot_list: Vec<_> = self.hot_registers.iter().collect();
        hot_list.sort_by(|a, b| b.1.cmp(a.1));

        // 取前N个最热的寄存器
        for (key, _count) in hot_list.iter().take(n) {
            // 如果映射在L2缓存中，提升到L1
            if let Some(&dst_reg) = self.l2_cache.get(key) {
                self.l1_cache.put(**key, dst_reg);
            }
        }
    }

    /// 获取热寄存器列表（按访问频率排序）
    ///
    /// 返回访问频率最高的寄存器及其访问次数
    pub fn get_hot_registers(&self, limit: Option<usize>) -> Vec<(RegMapKey, u64)> {
        let mut hot_list: Vec<_> = self.hot_registers.iter().map(|(k, v)| (*k, *v)).collect();
        hot_list.sort_by(|a, b| b.1.cmp(&a.1));

        if let Some(limit) = limit {
            hot_list.truncate(limit);
        }

        hot_list
    }

    /// 优化缓存大小
    ///
    /// 基于热寄存器分布自适应调整L1缓存大小
    pub fn optimize_cache_size(&mut self) {
        if !self.adaptive_l1_size {
            return;
        }

        // 分析热寄存器分布
        let hot_regs = self.get_hot_registers(Some(100));
        if hot_regs.is_empty() {
            return;
        }

        // 计算访问频率的方差，以确定是否需要调整缓存大小
        let total_accesses: u64 = hot_regs.iter().map(|(_, count)| count).sum();
        let avg_accesses = total_accesses / hot_regs.len() as u64;

        let variance: f64 = hot_regs
            .iter()
            .map(|(_, count)| {
                let diff = *count as f64 - avg_accesses as f64;
                diff * diff
            })
            .sum::<f64>()
            / hot_regs.len() as f64;

        let std_dev = variance.sqrt();

        // 如果标准差很大，说明访问集中，可以增大L1缓存
        // 如果标准差很小，说明访问分散，可以减小L1缓存
        let new_capacity = if std_dev > avg_accesses as f64 * 0.5 {
            // 访问集中，增大缓存
            (self.l1_capacity * 3 / 2).min(128) // 最多128
        } else if std_dev < avg_accesses as f64 * 0.2 {
            // 访问分散，减小缓存
            (self.l1_capacity * 2 / 3).max(16) // 最少16
        } else {
            self.l1_capacity // 保持不变
        };

        if new_capacity != self.l1_capacity {
            self.l1_capacity = new_capacity;
            // 重建L1缓存
            let _old_cache =
                std::mem::replace(&mut self.l1_cache, lru::LruCache::new(new_capacity));

            // 将最热的条目迁移到新缓存
            for (key, _) in hot_regs.iter().take(new_capacity) {
                if let Some(dst_reg) = self.l2_cache.get(key) {
                    self.l1_cache.put(*key, *dst_reg);
                }
            }
        }
    }

    /// 获取热寄存器统计信息
    pub fn hot_register_stats(&self) -> HotRegisterStats {
        let hot_list = self.get_hot_registers(None);

        if hot_list.is_empty() {
            return HotRegisterStats::default();
        }

        let total_accesses: u64 = hot_list.iter().map(|(_, count)| count).sum();
        let unique_registers = hot_list.len() as u64;
        let avg_accesses = total_accesses / unique_registers;
        let max_accesses = hot_list[0].1;

        // 计算前10%寄存器的访问占比
        let top_10_percent_count = (unique_registers as f64 * 0.1).ceil() as usize;
        let top_10_percent_accesses: u64 = hot_list
            .iter()
            .take(top_10_percent_count.max(1))
            .map(|(_, count)| count)
            .sum();

        let concentration_ratio = if total_accesses > 0 {
            top_10_percent_accesses as f64 / total_accesses as f64
        } else {
            0.0
        };

        HotRegisterStats {
            unique_registers,
            total_accesses,
            avg_accesses,
            max_accesses,
            concentration_ratio,
        }
    }

    /// 计算寄存器映射（L3 路径）
    fn compute_mapping(
        &self,
        src_arch: ArchType,
        dst_arch: ArchType,
        src_reg: RegisterType,
    ) -> RegisterType {
        match (src_arch, dst_arch, src_reg) {
            // x86_64 → ARM64
            (ArchType::X86_64, ArchType::ARM64, RegisterType::GPR(n)) => {
                // x86_64: RAX(0), RCX(1), RDX(2), RBX(3), RSP(4), RBP(5), RSI(6), RDI(7), R8-R15
                // ARM64: X0-X30
                let arm64_reg = match n {
                    0 => 0,  // RAX → X0
                    1 => 1,  // RCX → X1
                    2 => 2,  // RDX → X2
                    3 => 3,  // RBX → X3
                    4 => 29, // RSP → SP (X29)
                    5 => 30, // RBP → FP (X30 in AArch64, but
                    // traditionally X29)
                    6 => 4,                                    // RSI → X4
                    7 => 5,                                    // RDI → X5
                    n if (8..=15).contains(&n) => 6 + (n - 8), // R8-R15 → X6-X13
                    _ => n,                                    // 直接映射
                };
                RegisterType::GPR(arm64_reg)
            }
            (ArchType::X86_64, ArchType::ARM64, RegisterType::Special(SpecialReg::PC)) => {
                RegisterType::Special(SpecialReg::PC)
            }
            (ArchType::X86_64, ArchType::ARM64, RegisterType::Special(SpecialReg::SP)) => {
                RegisterType::Special(SpecialReg::SP)
            }
            (ArchType::X86_64, ArchType::ARM64, RegisterType::Special(SpecialReg::FP)) => {
                RegisterType::Special(SpecialReg::FP)
            }

            // x86_64 → RISC-V
            (ArchType::X86_64, ArchType::RiscV64, RegisterType::GPR(n)) => {
                // RISC-V: x0-x31, x0=zero, x1=ra, x2=sp, x8=fp
                let rv_reg = match n {
                    0 => 10,                                    // RAX → a0 (x10)
                    1 => 11,                                    // RCX → a1 (x11)
                    2 => 12,                                    // RDX → a2 (x12)
                    3 => 13,                                    // RBX → a3 (x13)
                    4 => 2,                                     // RSP → sp (x2)
                    5 => 8,                                     // RBP → fp (x8)
                    6 => 14,                                    // RSI → a4 (x14)
                    7 => 15,                                    // RDI → a5 (x15)
                    n if (8..=15).contains(&n) => 16 + (n - 8), // R8-R15 → a6-a13 (x16-x23)
                    _ => n,
                };
                RegisterType::GPR(rv_reg)
            }
            (ArchType::X86_64, ArchType::RiscV64, RegisterType::Special(SpecialReg::PC)) => {
                RegisterType::Special(SpecialReg::PC)
            }
            (ArchType::X86_64, ArchType::RiscV64, RegisterType::Special(SpecialReg::SP)) => {
                RegisterType::Special(SpecialReg::SP)
            }

            // ARM64 → x86_64
            (ArchType::ARM64, ArchType::X86_64, RegisterType::GPR(n)) => {
                let x86_reg = match n {
                    0 => 0,                                    // X0 → RAX
                    1 => 1,                                    // X1 → RCX
                    2 => 2,                                    // X2 → RDX
                    3 => 3,                                    // X3 → RBX
                    29 => 4,                                   // SP → RSP
                    30 => 5,                                   // FP → RBP
                    4 => 6,                                    // X4 → RSI
                    5 => 7,                                    // X5 → RDI
                    n if (6..=13).contains(&n) => 8 + (n - 6), // X6-X13 → R8-R15
                    _ => n,
                };
                RegisterType::GPR(x86_reg)
            }
            (ArchType::ARM64, ArchType::X86_64, RegisterType::Special(SpecialReg::PC)) => {
                RegisterType::Special(SpecialReg::PC)
            }
            (ArchType::ARM64, ArchType::X86_64, RegisterType::Special(SpecialReg::SP)) => {
                RegisterType::Special(SpecialReg::SP)
            }

            // ARM64 → RISC-V
            (ArchType::ARM64, ArchType::RiscV64, RegisterType::GPR(n)) => {
                let rv_reg = match n {
                    0 => 10,                                    // X0 → a0
                    1 => 11,                                    // X1 → a1
                    2 => 12,                                    // X2 → a2
                    3 => 13,                                    // X3 → a3
                    29 => 2,                                    // SP → sp
                    30 => 8,                                    // FP → fp
                    n if (4..=18).contains(&n) => 14 + (n - 4), // X4-X18 → a4-a18
                    _ => n,
                };
                RegisterType::GPR(rv_reg)
            }

            // RISC-V → x86_64
            (ArchType::RiscV64, ArchType::X86_64, RegisterType::GPR(n)) => {
                let x86_reg = match n {
                    10 => 0,                                     // a0 → RAX
                    11 => 1,                                     // a1 → RCX
                    12 => 2,                                     // a2 → RDX
                    13 => 3,                                     // a3 → RBX
                    2 => 4,                                      // sp → RSP
                    8 => 5,                                      // fp → RBP
                    14 => 6,                                     // a4 → RSI
                    15 => 7,                                     // a5 → RDI
                    n if (16..=23).contains(&n) => 8 + (n - 16), // a6-a13 → R8-R15
                    _ => n,
                };
                RegisterType::GPR(x86_reg)
            }

            // RISC-V → ARM64
            (ArchType::RiscV64, ArchType::ARM64, RegisterType::GPR(n)) => {
                let arm64_reg = match n {
                    10 => 0,                                     // a0 → X0
                    11 => 1,                                     // a1 → X1
                    12 => 2,                                     // a2 → X2
                    13 => 3,                                     // a3 → X3
                    2 => 29,                                     // sp → SP
                    8 => 30,                                     // fp → FP
                    n if (14..=23).contains(&n) => 4 + (n - 14), // a4-a13 → X4-X13
                    _ => n,
                };
                RegisterType::GPR(arm64_reg)
            }

            // 默认：直接映射
            (_, _, reg) => reg,
        }
    }

    /// 预填充 x86_64 → ARM64 映射
    fn prefill_x86_64_to_arm64(&mut self) {
        // 通用寄存器
        for i in 0..16 {
            let key = RegMapKey {
                src_arch: ArchType::X86_64,
                dst_arch: ArchType::ARM64,
                src_reg: RegisterType::GPR(i),
            };
            let dst_reg =
                self.compute_mapping(ArchType::X86_64, ArchType::ARM64, RegisterType::GPR(i));
            self.l2_cache.insert(key, dst_reg);
        }

        // 特殊寄存器
        for &special in &[SpecialReg::PC, SpecialReg::SP, SpecialReg::FP] {
            let key = RegMapKey {
                src_arch: ArchType::X86_64,
                dst_arch: ArchType::ARM64,
                src_reg: RegisterType::Special(special),
            };
            let dst_reg = self.compute_mapping(
                ArchType::X86_64,
                ArchType::ARM64,
                RegisterType::Special(special),
            );
            self.l2_cache.insert(key, dst_reg);
        }
    }

    /// 预填充 x86_64 → RISC-V 映射
    fn prefill_x86_64_to_riscv(&mut self) {
        for i in 0..16 {
            let key = RegMapKey {
                src_arch: ArchType::X86_64,
                dst_arch: ArchType::RiscV64,
                src_reg: RegisterType::GPR(i),
            };
            let dst_reg =
                self.compute_mapping(ArchType::X86_64, ArchType::RiscV64, RegisterType::GPR(i));
            self.l2_cache.insert(key, dst_reg);
        }

        for &special in &[SpecialReg::PC, SpecialReg::SP, SpecialReg::FP] {
            let key = RegMapKey {
                src_arch: ArchType::X86_64,
                dst_arch: ArchType::RiscV64,
                src_reg: RegisterType::Special(special),
            };
            let dst_reg = self.compute_mapping(
                ArchType::X86_64,
                ArchType::RiscV64,
                RegisterType::Special(special),
            );
            self.l2_cache.insert(key, dst_reg);
        }
    }

    /// 预填充 ARM64 → RISC-V 映射
    fn prefill_arm64_to_riscv(&mut self) {
        for i in 0..19 {
            let key = RegMapKey {
                src_arch: ArchType::ARM64,
                dst_arch: ArchType::RiscV64,
                src_reg: RegisterType::GPR(i),
            };
            let dst_reg =
                self.compute_mapping(ArchType::ARM64, ArchType::RiscV64, RegisterType::GPR(i));
            self.l2_cache.insert(key, dst_reg);
        }

        for &special in &[SpecialReg::PC, SpecialReg::SP, SpecialReg::FP] {
            let key = RegMapKey {
                src_arch: ArchType::ARM64,
                dst_arch: ArchType::RiscV64,
                src_reg: RegisterType::Special(special),
            };
            let dst_reg = self.compute_mapping(
                ArchType::ARM64,
                ArchType::RiscV64,
                RegisterType::Special(special),
            );
            self.l2_cache.insert(key, dst_reg);
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> &MapperStats {
        &self.stats
    }

    /// 清除缓存
    pub fn clear_cache(&mut self) {
        self.l1_cache.clear();
        // L2 缓存保留预填充的映射
    }
}

impl Default for RegisterMapper {
    fn default() -> Self {
        Self::new()
    }
}

// 简化的 LRU 缓存实现
mod lru {
    use std::collections::{HashMap, VecDeque};

    pub struct LruCache<K, V> {
        capacity: usize,
        entries: HashMap<K, V>,
        order: VecDeque<K>,
    }

    impl<K: Clone + std::hash::Hash + Eq, V> LruCache<K, V> {
        pub fn new(capacity: usize) -> Self {
            Self {
                capacity,
                entries: HashMap::new(),
                order: VecDeque::with_capacity(capacity),
            }
        }

        pub fn get(&mut self, key: &K) -> Option<&V> {
            if self.entries.contains_key(key) {
                // 移到末尾（最近使用）
                self.order.retain(|k| k != key);
                self.order.push_back(key.clone());
                self.entries.get(key)
            } else {
                None
            }
        }

        pub fn put(&mut self, key: K, value: V) {
            // 移除旧条目（如果存在）
            self.order.retain(|k| k != &key);

            // 检查容量
            if self.order.len() >= self.capacity
                && let Some(old_key) = self.order.pop_front()
            {
                self.entries.remove(&old_key);
            }

            // 插入新条目
            self.entries.insert(key.clone(), value);
            self.order.push_back(key);
        }

        pub fn clear(&mut self) {
            self.entries.clear();
            self.order.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_mapper_creation() {
        let mapper = RegisterMapper::new();
        let stats = mapper.stats();
        assert_eq!(stats.total_lookups, 0);
    }

    #[test]
    fn test_x86_64_to_arm64_gpr_mapping() {
        let mut mapper = RegisterMapper::new();

        // RAX → X0
        let mapped = mapper.map_register(ArchType::X86_64, ArchType::ARM64, RegisterType::GPR(0));
        assert_eq!(mapped, RegisterType::GPR(0));

        // RSP → SP
        let mapped = mapper.map_register(ArchType::X86_64, ArchType::ARM64, RegisterType::GPR(4));
        assert_eq!(mapped, RegisterType::GPR(29));
    }

    #[test]
    fn test_special_register_mapping() {
        let mut mapper = RegisterMapper::new();

        let mapped = mapper.map_register(
            ArchType::X86_64,
            ArchType::ARM64,
            RegisterType::Special(SpecialReg::PC),
        );
        assert_eq!(mapped, RegisterType::Special(SpecialReg::PC));
    }

    #[test]
    fn test_batch_mapping() {
        let mut mapper = RegisterMapper::new();
        let src_regs = vec![
            RegisterType::GPR(0),
            RegisterType::GPR(1),
            RegisterType::GPR(2),
        ];

        let dst_regs = mapper.map_registers_batch(ArchType::X86_64, ArchType::ARM64, &src_regs);

        assert_eq!(dst_regs.len(), 3);
        assert_eq!(dst_regs[0], RegisterType::GPR(0));
        assert_eq!(dst_regs[1], RegisterType::GPR(1));
        assert_eq!(dst_regs[2], RegisterType::GPR(2));
    }

    #[test]
    fn test_cache_hit_rate() {
        let mut mapper = RegisterMapper::new();

        // 第一次查找：缓存未命中
        mapper.map_register(ArchType::X86_64, ArchType::ARM64, RegisterType::GPR(0));

        // 第二次查找：L1 缓存命中
        mapper.map_register(ArchType::X86_64, ArchType::ARM64, RegisterType::GPR(0));

        let stats = mapper.stats();
        assert_eq!(stats.total_lookups, 2);
        assert_eq!(stats.l1_hits, 1);
        assert!(stats.hit_rate() > 0.0);
    }

    #[test]
    fn test_same_arch_mapping() {
        let mut mapper = RegisterMapper::new();

        let mapped = mapper.map_register(ArchType::X86_64, ArchType::X86_64, RegisterType::GPR(5));

        // 同架构应该直接返回
        assert_eq!(mapped, RegisterType::GPR(5));
    }

    #[test]
    fn test_cache_clear() {
        let mut mapper = RegisterMapper::new();

        mapper.map_register(ArchType::X86_64, ArchType::ARM64, RegisterType::GPR(0));

        let stats_before = mapper.stats().l1_hits;

        mapper.clear_cache();

        mapper.map_register(ArchType::X86_64, ArchType::ARM64, RegisterType::GPR(0));

        // L1 缓存应该被清除，但 L2 仍命中
        let stats_after = mapper.stats();
        assert_eq!(stats_after.l1_hits, stats_before);
        assert!(stats_after.l2_hits > 0);
    }

    #[test]
    fn test_adaptive_warmup() {
        let mut mapper = RegisterMapper::with_adaptive_optimization();

        // 生成一些热寄存器访问
        for _ in 0..10 {
            mapper.map_register(ArchType::X86_64, ArchType::ARM64, RegisterType::GPR(0));
        }
        for _ in 0..5 {
            mapper.map_register(ArchType::X86_64, ArchType::ARM64, RegisterType::GPR(1));
        }

        // 执行自适应预热
        mapper.adaptive_warmup(Some(2));

        // 验证热寄存器统计
        let hot_regs = mapper.get_hot_registers(Some(5));
        assert!(!hot_regs.is_empty());
        assert!(hot_regs[0].1 >= 10); // 最热的寄存器应该有至少10次访问
    }

    #[test]
    fn test_hot_register_stats() {
        let mut mapper = RegisterMapper::with_adaptive_optimization();

        // 生成一些访问
        for i in 0..5 {
            for _ in 0..(i + 1) {
                mapper.map_register(ArchType::X86_64, ArchType::ARM64, RegisterType::GPR(i));
            }
        }

        let stats = mapper.hot_register_stats();
        assert!(stats.unique_registers > 0);
        assert!(stats.total_accesses > 0);
        assert!(stats.max_accesses >= 5); // 寄存器0应该有5次访问
        assert!(stats.concentration_ratio > 0.0);
    }

    #[test]
    fn test_adaptive_optimization_mapper_creation() {
        let mapper = RegisterMapper::with_adaptive_optimization();
        let stats = mapper.stats();
        assert_eq!(stats.total_lookups, 0);
        // 自适应优化版本创建成功即可
        assert!(mapper.stats().total_lookups == 0);
    }

    #[test]
    fn test_large_batch_mapping() {
        let mut mapper = RegisterMapper::new();

        // 创建大量寄存器（超过100个）
        let src_regs: Vec<RegisterType> = (0..150).map(RegisterType::GPR).collect();

        let dst_regs = mapper.map_registers_batch(ArchType::X86_64, ArchType::ARM64, &src_regs);

        assert_eq!(dst_regs.len(), 150);
        // 验证一些映射
        assert_eq!(dst_regs[0], RegisterType::GPR(0)); // RAX → X0
        assert_eq!(dst_regs[4], RegisterType::GPR(29)); // RSP → SP
    }
}
