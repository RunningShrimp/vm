//! Domain layer interfaces to reduce coupling between VM services.

use crate::{AccessType, Fault, GuestAddr, GuestPhysAddr, VmResult};

/// 可插拔的 TLB 管理器接口。
/// 
/// 标识: 服务接口
pub trait TlbManager: Send + Sync {
    /// 查询地址对应的 TLB 条目。
    fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry>;

    /// 更新或插入 TLB 条目。
    fn update(&mut self, entry: TlbEntry);

    /// 清空 TLB。
    fn flush(&mut self);

    /// 清除特定 ASID 的条目。
    fn flush_asid(&mut self, asid: u16);
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
    fn walk(&mut self, addr: GuestAddr, access: AccessType, asid: u16) -> Result<(GuestPhysAddr, u64), Fault>;
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
