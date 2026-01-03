//! Domain layer interfaces to reduce coupling between VM services.

use crate::{AccessType, GuestAddr, GuestPhysAddr, MMU, VmError, VmResult};

/// 可插拔的 TLB 管理器接口。
///
/// 标识: 服务接口
///
/// 此trait为所有TLB实现提供统一接口，包括：
/// - MultiLevelTlb (vm-mem): 多级TLB，适用于高性能场景
/// - ConcurrentTlbManager (vm-mem): 并发TLB，适用于高并发场景
/// - AsyncTlbAdapter (vm-core): 异步TLB，适用于异步执行场景
///
/// # 使用场景
/// - 地址转换缓存：缓存虚拟地址到物理地址的转换结果
/// - 多级缓存：L1/L2/L3多级TLB优化
/// - ASID支持：区分不同地址空间的TLB条目
/// - 性能监控：TLB命中率统计和优化
///
/// # TLB策略
/// - LRU（Least Recently Used）：最近最少使用淘汰
/// - PLRU（Pseudo-LRU）：伪LRU，硬件友好
/// - Random：随机淘汰，简单实现
///
/// # 示例
/// ```ignore
/// let mut tlb = MultiLevelTlb::new(config);
/// if let Some(entry) = tlb.lookup(addr, asid, AccessType::Read) {
///     // TLB命中，直接使用
/// } else {
///     // TLB未命中，进行页表遍历
///     let pa = page_table_walk(addr, access, mmu)?;
///     tlb.update(TlbEntry { guest_addr: addr, phys_addr: pa, ... });
/// }
/// ```
pub trait TlbManager: Send + Sync {
    /// 查询地址对应的 TLB 条目。
    ///
    /// 从TLB中查找指定地址的转换结果。
    /// 匹配条件包括地址和ASID（地址空间标识符）。
    ///
    /// # 参数
    /// - `addr`: 要查询的虚拟地址
    /// - `asid`: 地址空间标识符
    /// - `access`: 访问类型
    ///
    /// # 返回
    /// TLB条目（如果命中），否则返回None
    fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry>;

    /// 更新或插入 TLB 条目。
    ///
    /// 将新的地址转换结果插入TLB。
    /// 如果TLB已满，根据淘汰策略移除旧条目。
    ///
    /// # 参数
    /// - `entry`: 要插入的TLB条目
    ///
    /// # 注意
    /// - 如果键已存在，会更新旧条目
    /// - 插入操作会更新LRU顺序（如果使用LRU策略）
    fn update(&mut self, entry: TlbEntry);

    /// 清空 TLB。
    ///
    /// 移除所有TLB条目。
    /// 通常在页表变更、进程切换或ASID切换时调用。
    ///
    /// # 注意
    /// 清空后，所有地址转换都需要重新进行页表遍历。
    fn flush(&mut self);

    /// 清除特定 ASID 的条目
    ///
    /// 移除指定ASID的所有TLB条目。
    /// 用于精确的TLB失效，减少不必要的重新转换。
    ///
    /// # 参数
    /// - `asid`: 要清除的地址空间标识符
    ///
    /// # 注意
    /// 这是ASID TLB的优化，可以只失效特定进程的TLB。
    fn flush_asid(&mut self, asid: u16);

    /// 清除特定页面的条目
    ///
    /// 移除包含指定虚拟地址的TLB条目。
    /// 用于精确的TLB失效。
    ///
    /// # 参数
    /// - `_va`: 虚拟地址
    ///
    /// # 注意
    /// 默认实现清空所有TLB条目。
    /// 支持精确失效的实现可以覆盖此方法。
    fn flush_page(&mut self, _va: GuestAddr) {
        self.flush();
    }

    /// 获取统计信息（可选实现）
    ///
    /// 返回TLB的统计信息，如命中率、查找次数等。
    /// 用于性能监控和优化。
    ///
    /// # 返回
    /// TLB统计信息（如果支持），否则返回None
    ///
    /// # 注意
    /// 默认实现返回None，表示不支持统计。
    fn get_stats(&self) -> Option<TlbStats> {
        None
    }
}

/// TLB统计信息
#[derive(Debug, Clone)]
pub struct TlbStats {
    /// 总查找次数
    pub total_lookups: u64,
    /// 命中次数
    pub hits: u64,
    /// 缺失次数
    pub misses: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 当前条目数
    pub current_entries: usize,
    /// 容量
    pub capacity: usize,
}

/// 代表一条 TLB 条目。
///
/// 标识: 数据模型
#[derive(Clone, Copy, Debug)]
pub struct TlbEntry {
    pub guest_addr: GuestAddr,
    pub phys_addr: GuestPhysAddr,
    pub flags: u64,
    pub asid: u16,
}

/// 页表遍历器接口，支持分页硬件或软件实现。
///
/// 标识: 服务接口
///
/// 负责遍历多级页表，将虚拟地址转换为物理地址。
/// 这是虚拟机内存管理的核心功能之一。
///
/// # 使用场景
/// - 虚拟地址转换：Guest OS虚拟地址到宿主物理地址
/// - 页表遍历：多级页表（如4级页表）的递归查找
/// - 权限检查：验证访问权限和属性
/// - TLB填充：页表遍历结果填充到TLB
///
/// # 页表结构
/// 不同架构有不同的页表结构：
/// - x86-64: 4级页表（PML4 -> PDP -> PD -> PT）
/// - ARM64: 4级页表（L0 -> L1 -> L2 -> L3）
/// - RISC-V: 3级或4级页表
///
/// # 示例
/// ```ignore
/// let mut walker = PageTableWalker::new(config);
/// let (pa, flags) = walker.walk(va, AccessType::Read, asid, &mut mmu)?;
/// if flags & PAGE_EXECUTE == 0 {
///     return Err(VmError::PermissionDenied);
/// }
/// ```
pub trait PageTableWalker: Send + Sync {
    /// 遍历页表，将虚拟地址映射到物理地址，并返回 PTE 标志。
    /// 需要通过依赖注入访问 MMU 进行物理内存读取。
    ///
    /// # 参数
    /// - `addr`: 要翻译的虚拟地址
    /// - `access`: 访问类型（读/写/执行）
    /// - `asid`: 地址空间标识符
    /// - `mmu`: MMU引用，用于读取页表
    ///
    /// # 返回
    /// 物理地址和页表项标志的元组
    ///
    /// # 错误
    /// - 页错误：页表项不存在或权限不足
    /// - 对齐错误：地址未对齐
    /// - 访问错误：物理内存访问失败
    fn walk(
        &mut self,
        addr: GuestAddr,
        access: AccessType,
        asid: u16,
        mmu: &mut dyn MMU,
    ) -> Result<(GuestPhysAddr, u64), VmError>;
}

/// 执行管理器接口，负责驱动 vCPU 的执行流。
///
/// 标识: 服务接口
pub trait ExecutionManager<B>: Send {
    /// 运行一次基本块或执行上下文。
    fn run(&mut self, block: &B) -> VmResult<()>;

    /// 查询下一条要执行的 PC。
    fn next_pc(&self) -> GuestAddr;

    /// 设置要执行的 PC。
    fn set_pc(&mut self, pc: GuestAddr);
}

/// 缓存管理接口
///
/// 标识: 服务接口
///
/// 提供统一的缓存管理接口，支持多种缓存实现（LRU、LFU、FIFO等）。
/// 具体实现应在基础设施层（如 vm-engine、vm-mem）。
///
/// # 类型参数
/// - `K`: 缓存键类型
/// - `V`: 缓存值类型
///
/// # 使用场景
/// - JIT 编译缓存：缓存编译后的代码块
/// - 翻译缓存：缓存跨架构翻译结果
/// - 优化缓存：缓存优化后的 IR
pub trait CacheManager<K, V>: Send + Sync {
    /// 获取缓存值
    ///
    /// # 参数
    /// - `key`: 缓存键
    ///
    /// # 返回
    /// 缓存值（如果存在），否则返回 None
    fn get(&self, key: &K) -> Option<V>;

    /// 插入缓存值
    ///
    /// # 参数
    /// - `key`: 缓存键
    /// - `value`: 缓存值
    fn put(&mut self, key: K, value: V);

    /// 移除缓存值
    ///
    /// # 参数
    /// - `key`: 要移除的缓存键
    fn evict(&mut self, key: &K);

    /// 清空缓存
    fn clear(&mut self);

    /// 获取缓存统计
    ///
    /// # 返回
    /// 缓存统计信息
    fn stats(&self) -> CacheStats;
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存缺失次数
    pub misses: u64,
    /// 当前缓存条目数
    pub size: usize,
    /// 缓存容量
    pub capacity: usize,
}

/// 优化策略接口
///
/// 标识: 服务接口
///
/// 提供统一的优化策略接口，支持多种优化实现。
/// 具体实现应在基础设施层（如 vm-engine、vm-optimizers）。
///
/// # 使用场景
/// - IR 优化：对中间表示进行优化
/// - 代码优化：对生成的代码进行优化
/// - 跨架构优化：针对特定架构的优化
pub trait OptimizationStrategy: Send + Sync {
    /// 优化 IR 块
    ///
    /// # 参数
    /// - `ir`: 要优化的 IR 块
    ///
    /// # 返回
    /// 优化后的 IR 块
    ///
    /// # 注意
    /// 此方法应使用 `vm_ir::IRBlock`，但为避免循环依赖，
    /// 具体类型应在基础设施层定义。
    fn optimize_ir(&self, ir: &[u8]) -> VmResult<Vec<u8>>;

    /// 获取优化级别
    ///
    /// # 返回
    /// 优化级别 (0-3)
    fn optimization_level(&self) -> u32;

    /// 是否支持特定优化
    ///
    /// # 参数
    /// - `opt_type`: 优化类型
    ///
    /// # 返回
    /// 是否支持该优化
    fn supports_optimization(&self, opt_type: OptimizationType) -> bool;
}

/// 优化类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptimizationType {
    /// 常量折叠
    ConstantFolding,
    /// 死代码消除
    DeadCodeElimination,
    /// 指令合并
    InstructionCombining,
    /// 循环优化
    LoopOptimization,
    /// 寄存器分配优化
    RegisterAllocation,
    /// 指令调度
    InstructionScheduling,
}

/// 寄存器分配器接口
///
/// 标识: 服务接口
///
/// 提供统一的寄存器分配接口，支持多种分配算法（图着色、线性扫描等）。
/// 具体实现应在基础设施层（如 vm-engine）。
///
/// # 使用场景
/// - JIT 编译：为生成的代码分配寄存器
/// - 跨架构翻译：将源架构寄存器映射到目标架构
pub trait RegisterAllocator: Send + Sync {
    /// 分配寄存器
    ///
    /// # 参数
    /// - `ir`: IR 块（序列化形式，避免循环依赖）
    ///
    /// # 返回
    /// 寄存器映射结果（序列化形式）
    ///
    /// # 注意
    /// 为避免循环依赖，使用序列化形式传递数据。
    /// 具体类型应在基础设施层定义。
    fn allocate(&mut self, ir: &[u8]) -> VmResult<Vec<u8>>;

    /// 获取分配统计
    ///
    /// # 返回
    /// 寄存器分配统计信息
    fn stats(&self) -> RegisterAllocationStats;
}

/// 寄存器分配统计信息
#[derive(Debug, Clone)]
pub struct RegisterAllocationStats {
    /// 总分配次数
    pub total_allocations: usize,
    /// 溢出次数
    pub spills: usize,
    /// 使用的物理寄存器数
    pub physical_regs_used: usize,
    /// 虚拟寄存器数
    pub virtual_regs: usize,
}
