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
pub trait TlbManager: Send + Sync {
    /// 查询地址对应的 TLB 条目。
    fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry>;

    /// 更新或插入 TLB 条目。
    fn update(&mut self, entry: TlbEntry);

    /// 清空 TLB。
    fn flush(&mut self);

    /// 清除特定 ASID 的条目。
    fn flush_asid(&mut self, asid: u16);

    /// 获取统计信息（可选实现）
    /// 
    /// 返回TLB的统计信息，如命中率、查找次数等。
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
pub trait PageTableWalker: Send + Sync {
    /// 遍历页表，将虚拟地址映射到物理地址，并返回 PTE 标志。
    /// 需要通过依赖注入访问 MMU 进行物理内存读取。
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
