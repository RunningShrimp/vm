//! MMU Trait 细粒度拆分
//!
//! 将MMU trait拆分为更细粒度的接口，符合接口隔离原则。

use crate::{AccessType, GuestAddr, GuestPhysAddr, MmioDevice, VmError};

/// 地址翻译器
///
/// 负责虚拟地址到物理地址的翻译
pub trait AddressTranslator: Send + Sync {
    /// 虚拟地址翻译
    ///
    /// 将虚拟地址（GVA）翻译到物理地址（GPA）。
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError>;

    /// TLB 刷新
    fn flush_tlb(&mut self);

    /// 刷新特定ASID的TLB
    fn flush_tlb_asid(&mut self, _asid: u16) {
        // 默认实现刷新所有TLB
        self.flush_tlb();
    }

    /// 刷新特定页面的TLB
    fn flush_tlb_page(&mut self, _va: GuestAddr) {
        // 默认实现刷新所有TLB
        self.flush_tlb();
    }
}

/// 内存读取器
///
/// 负责从物理地址读取内存
pub trait MemoryReader: Send + Sync {
    /// 从给定物理地址读取内存
    ///
    /// # 参数
    /// - `pa`: 物理地址
    /// - `size`: 读取大小（1/2/4/8 字节）
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError>;

    /// 原子性的读取与保留（LR 指令）
    ///
    /// 用于原子操作的实现，通常配合 store_conditional 使用。
    fn load_reserved(&mut self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        self.read(pa, size)
    }

    /// 从给定 PC 取出指令
    ///
    /// 自动处理地址翻译和访问控制。
    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError>;

    /// 批量读内存
    fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
        for (i, byte) in buf.iter_mut().enumerate() {
            *byte = self.read(pa + i as u64, 1)? as u8;
        }
        Ok(())
    }
}

/// 内存写入器
///
/// 负责向物理地址写入内存
pub trait MemoryWriter: Send + Sync {
    /// 向给定物理地址写入内存
    ///
    /// # 参数
    /// - `pa`: 物理地址
    /// - `val`: 要写入的值
    /// - `size`: 写入大小（1/2/4/8 字节）
    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError>;

    /// 条件存储（SC 指令）
    ///
    /// 用于原子操作，仅在之前 load_reserved 的地址未被修改时写入。
    /// 返回 true 表示成功，false 表示失败。
    fn store_conditional(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<bool, VmError> {
        // 默认实现总是失败（保守行为）
        let _ = (pa, val, size);
        Ok(false)
    }

    /// 清除保留位
    ///
    /// 当 LR 地址被其他 CPU 修改或其他情况下调用，用于清除保留状态。
    fn invalidate_reservation(&mut self, _pa: GuestAddr, _size: u8) {}

    /// 批量写内存
    fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
        for (i, &byte) in buf.iter().enumerate() {
            self.write(pa + i as u64, byte as u64, 1)?;
        }
        Ok(())
    }
}

/// MMIO设备管理器
///
/// 负责管理MMIO设备映射
pub trait MmioManager: Send + Sync {
    /// 映射 MMIO 设备
    fn map_mmio(&mut self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>);

    /// 设备轮询（用于异步 I/O 驱动）
    fn poll_devices(&mut self) {}
}

/// 内存信息提供者
///
/// 提供内存相关的元信息
pub trait MemoryInfo: Send + Sync {
    /// 获取物理内存大小
    fn memory_size(&self) -> usize;

    /// 转储整个物理内存内容
    fn dump_memory(&self) -> Vec<u8>;

    /// 从转储中恢复物理内存
    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String>;
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
/// 组合所有细粒度接口，保持向后兼容
pub trait MMU:
    AddressTranslator
    + MemoryReader
    + MemoryWriter
    + MmioManager
    + MemoryInfo
    + MmuAsAny
    + Send
    + 'static
{
    // 所有方法已在各个trait中定义
}

// 为实现了所有子trait的类型自动实现MMU trait
impl<T> MMU for T where
    T: AddressTranslator
        + MemoryReader
        + MemoryWriter
        + MmioManager
        + MemoryInfo
        + MmuAsAny
        + Send
        + 'static
{
}

