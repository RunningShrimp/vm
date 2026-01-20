//! MMU Trait 细粒度拆分
//!
//! 将MMU trait拆分为更细粒度的接口，符合接口隔离原则。

use crate::{AccessType, GuestAddr, GuestPhysAddr, MmioDevice, VmError};

/// 地址翻译器
///
/// 负责虚拟地址到物理地址的翻译。这是虚拟机内存管理的核心功能，
/// 通过页表遍历和TLB缓存实现高效的地址转换。
///
/// # 使用场景
/// - 虚拟内存管理：Guest OS虚拟地址到宿主物理地址的映射
/// - 页表遍历：多级页表的递归查找
/// - TLB管理：地址转换结果缓存和失效
/// - 权限检查：访问权限验证和异常处理
///
/// # 示例
/// ```ignore
/// let mut translator = SoftMmu::new(config);
/// let pa = translator.translate(GuestAddr(0x1000), AccessType::Read)?;
/// ```
pub trait AddressTranslator: Send + Sync {
    /// 虚拟地址翻译
    ///
    /// 将虚拟地址（GVA）翻译到物理地址（GPA）。
    /// 首先检查TLB缓存，如果未命中则遍历页表。
    ///
    /// # 参数
    /// - `va`: 要翻译的虚拟地址
    /// - `access`: 访问类型（读/写/执行）
    ///
    /// # 返回
    /// 翻译后的物理地址
    ///
    /// # 错误
    /// - 页错误：地址无效或权限不足
    /// - 对齐错误：地址未对齐
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError>;

    /// TLB 刷新
    ///
    /// 清空所有TLB条目，下次访问需要重新翻译。
    /// 通常在页表变更或进程切换时调用。
    fn flush_tlb(&mut self);

    /// 刷新特定ASID的TLB
    ///
    /// 清空指定地址空间标识符的所有TLB条目。
    /// 用于支持ASID的TLB实现，可以只失效特定进程的TLB。
    ///
    /// # 参数
    /// - `_asid`: 地址空间标识符
    ///
    /// # 注意
    /// 默认实现清空所有TLB条目。
    fn flush_tlb_asid(&mut self, _asid: u16) {
        self.flush_tlb();
    }

    /// 刷新特定页面的TLB
    ///
    /// 清空包含指定虚拟地址的TLB条目。
    /// 用于精确的TLB失效，减少不必要的重新翻译。
    ///
    /// # 参数
    /// - `_va`: 虚拟地址
    ///
    /// # 注意
    /// 默认实现清空所有TLB条目。
    fn flush_tlb_page(&mut self, _va: GuestAddr) {
        self.flush_tlb();
    }
}

/// 内存访问接口
///
/// 统一管理内存的读取、写入和信息查询。提供原子操作、批量操作和指令获取等
/// 高级内存访问功能，支持跨架构的内存访问模式。
///
/// # 使用场景
/// - 指令执行：读取和写入Guest内存
/// - 原子操作：LL/SC（Load-Linked/Store-Conditional）指令支持
/// - 批量操作：内存拷贝、DMA模拟等
/// - 调试器：内存检查和修改
/// - 快照：内存转储和恢复
///
/// # 示例
/// ```ignore
/// let value = memory.read(GuestAddr(0x1000), 8)?;
/// memory.write(GuestAddr(0x1000), 0x42, 4)?;
/// ```
pub trait MemoryAccess: Send + Sync {
    /// 从给定物理地址读取内存
    ///
    /// 按指定大小读取内存数据，支持1/2/4/8字节。
    /// 自动处理大小端转换（如需要）。
    ///
    /// # 参数
    /// - `pa`: 物理地址
    /// - `size`: 读取大小（1/2/4/8 字节）
    ///
    /// # 返回
    /// 读取的数据值
    ///
    /// # 错误
    /// - 访问越界：地址超出物理内存范围
    /// - 未对齐访问：地址未按size对齐
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError>;

    /// 向给定物理地址写入内存
    ///
    /// 按指定大小写入内存数据，支持1/2/4/8字节。
    /// 自动处理大小端转换（如需要）。
    ///
    /// # 参数
    /// - `pa`: 物理地址
    /// - `val`: 要写入的值
    /// - `size`: 写入大小（1/2/4/8 字节）
    ///
    /// # 返回
    /// 写入成功返回Ok(())，失败返回错误
    ///
    /// # 错误
    /// - 访问越界：地址超出物理内存范围
    /// - 未对齐访问：地址未按size对齐
    /// - 只读访问：尝试写入只读内存
    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError>;

    /// 原子性的读取与保留（LR 指令）
    ///
    /// 用于实现Load-Linked/Store-Conditional（LL/SC）原子操作原语。
    /// 这是RISC-V、ARM等架构实现原子操作的基础。
    ///
    /// # 参数
    /// - `pa`: 物理地址
    /// - `size`: 读取大小
    ///
    /// # 返回
    /// 读取的数据值
    ///
    /// # 注意
    /// 默认实现调用read()，不支持真正的LL/SC语义。
    /// 实现LL/SC需要跟踪保留状态。
    fn load_reserved(&mut self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        self.read(pa, size)
    }

    /// 条件存储（SC 指令）
    ///
    /// 用于实现Load-Linked/Store-Conditional（LL/SC）原子操作原语。
    /// 只有在地址未被修改时才写入成功。
    ///
    /// # 参数
    /// - `pa`: 物理地址
    /// - `val`: 要写入的值
    /// - `size`: 写入大小
    ///
    /// # 返回
    /// 写入成功返回true，失败返回false
    ///
    /// # 注意
    /// 默认实现总是返回false，不支持真正的LL/SC语义。
    fn store_conditional(&mut self, _pa: GuestAddr, _val: u64, _size: u8) -> Result<bool, VmError> {
        Ok(false)
    }

    /// 清除保留位
    ///
    /// 清除LL/SC操作的保留状态，强制后续SC失败。
    /// 通常在执行任何可能修改内存的操作时调用。
    ///
    /// # 参数
    /// - `_pa`: 可选的地址，清除特定地址的保留
    /// - `_size`: 可选的大小
    fn invalidate_reservation(&mut self, _pa: GuestAddr, _size: u8) {}

    /// 从给定 PC 取出指令
    ///
    /// 从指定地址取指令，用于指令获取阶段。
    /// 支持指令缓存和对齐要求。
    ///
    /// # 参数
    /// - `pc`: 程序计数器
    ///
    /// # 返回
    /// 取出的指令字（机器码）
    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError>;

    /// 批量读内存
    ///
    /// 高效地批量读取连续内存区域。
    /// 直接内存拷贝，比逐字节读取快得多。
    ///
    /// # 参数
    /// - `pa`: 起始物理地址
    /// - `buf`: 输出缓冲区
    ///
    /// # 返回
    /// 读取成功返回Ok(())，失败返回错误
    ///
    /// # 性能
    /// 使用memcpy实现，适合大块内存拷贝。
    fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
        unsafe {
            let src_ptr = pa.0 as *const u8;
            let dst_ptr = buf.as_mut_ptr();
            std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, buf.len());
        }
        Ok(())
    }

    /// 批量写内存
    ///
    /// 高效地批量写入连续内存区域。
    /// 直接内存拷贝，比逐字节写入快得多。
    ///
    /// # 参数
    /// - `pa`: 起始物理地址
    /// - `buf`: 输入缓冲区
    ///
    /// # 返回
    /// 写入成功返回Ok(())，失败返回错误
    ///
    /// # 性能
    /// 使用memcpy实现，适合大块内存拷贝。
    fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
        unsafe {
            let dst_ptr = pa.0 as *mut u8;
            let src_ptr = buf.as_ptr();
            std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, buf.len());
        }
        Ok(())
    }

    /// 获取物理内存大小
    ///
    /// # 返回
    /// 物理内存总大小（字节）
    fn memory_size(&self) -> usize;

    /// 转储整个物理内存内容
    ///
    /// 用于快照和调试功能。
    ///
    /// # 返回
    /// 物理内存的完整副本
    fn dump_memory(&self) -> Vec<u8>;

    /// 从转储中恢复物理内存
    ///
    /// 用于快照恢复和测试功能。
    ///
    /// # 参数
    /// - `data`: 内存转储数据
    ///
    /// # 返回
    /// 恢复成功返回Ok(())，失败返回错误信息
    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String>;
}

/// MMIO设备管理器
///
/// 负责管理MMIO设备映射，将MMIO设备映射到虚拟机地址空间。
/// 当CPU访问MMIO区域时，将访问转发到对应的设备处理。
///
/// # 使用场景
/// - 外设模拟：UART、PCI设备、网络控制器等
/// - 中断处理：设备中断注入和路由
/// - 热插拔：设备动态添加和移除
/// - 调试：设备状态监控和诊断
///
/// # 示例
/// ```ignore
/// let uart = Box::new(UartDevice::new());
/// mmio_manager.map_mmio(GuestAddr(0x1000_0000), 0x1000, uart);
/// ```
pub trait MmioManager: Send + Sync {
    /// 映射 MMIO 设备
    ///
    /// 将MMIO设备映射到指定的虚拟机地址范围。
    /// 访问该范围的内存操作将被转发到设备。
    ///
    /// # 参数
    /// - `base`: 映射基地址
    /// - `size`: 映射大小（字节）
    /// - `device`: MMIO设备实例
    ///
    /// # 注意
    /// - 映射范围不能重叠
    /// - 设备的所有权将被转移
    /// - 设备必须实现MmioDevice trait
    fn map_mmio(&self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>);

    /// 设备轮询（用于异步 I/O 驱动）
    ///
    /// 轮询所有映射的设备，检查是否有挂起的I/O操作。
    /// 用于支持异步I/O和中断模拟。
    ///
    /// # 注意
    /// 默认实现不执行任何操作。
    /// 实现可以覆盖此方法以提供轮询功能。
    fn poll_devices(&self) {}
}

/// 类型转换支持
///
/// 用于向下转型到具体实现类型
pub trait MmuAsAny: Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// 统一的MMU trait
///
/// 组合所有必要接口，提供完整的内存管理单元功能。
/// MMU（Memory Management Unit）是虚拟机的核心组件之一。
///
/// # 使用场景
/// - 虚拟内存管理：Guest OS的虚拟地址空间管理
/// - 地址翻译：虚拟地址到物理地址的转换
/// - 内存保护：访问权限检查和异常处理
/// - 设备映射：MMIO设备到虚拟地址空间的映射
///
/// # 示例
/// ```ignore
/// let mmu = SoftMmu::new(config);
/// mmu.map_mmio(GuestAddr(0x1000_0000), 0x1000, uart_device);
/// let pa = mmu.translate(GuestAddr(0x8000_0000), AccessType::Read)?;
/// let value = mmu.read(pa, 4)?;
/// ```
///
/// # 注意
/// 此trait组合了AddressTranslator、MemoryAccess、MmioManager和MmuAsAny。
/// 实现此trait的类型需要实现所有这些子trait。
pub trait MMU: AddressTranslator + MemoryAccess + MmioManager + MmuAsAny + Send + 'static {
    // 所有方法已在各个trait中定义
}

// 为实现了所有子trait的类型自动实现MMU trait
impl<T> MMU for T where T: AddressTranslator + MemoryAccess + MmioManager + MmuAsAny + Send + 'static
{}
