//! ARM System Memory Management Unit (SMMU) 虚拟化支持
//!
//! SMMU 是 ARM 架构的 IOMMU，用于设备 DMA 地址转换和内存保护。
//! 本模块提供 SMMU 虚拟化功能，支持：
//! - SMMU 配置和初始化
//! - 地址转换表管理
//! - 设备流ID (StreamID) 映射
//! - TLB 管理
//! - 中断处理

use std::collections::HashMap;
use vm_core::{GuestAddr, HostAddr, VmError};
use tracing::{debug, trace};

/// SMMU 版本
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmmuVersion {
    /// SMMUv1 (已废弃)
    V1,
    /// SMMUv2
    V2,
    /// SMMUv3 (当前主流)
    V3,
}

/// SMMU 配置寄存器
#[derive(Debug, Clone)]
pub struct SmmuConfig {
    /// SMMU 版本
    pub version: SmmuVersion,
    /// 是否启用两阶段转换
    pub two_stage: bool,
    /// 是否启用嵌套转换
    pub nested: bool,
    /// 页表大小（4KB, 16KB, 64KB）
    pub page_size: u64,
    /// 地址宽度（32位或64位）
    pub address_width: u8,
}

impl Default for SmmuConfig {
    fn default() -> Self {
        Self {
            version: SmmuVersion::V3,
            two_stage: false,
            nested: false,
            page_size: 4096, // 4KB
            address_width: 48, // 48位地址
        }
    }
}

/// 设备流ID (StreamID)
///
/// StreamID 用于标识不同的设备，SMMU 使用 StreamID 来选择对应的转换表。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamId(pub u32);

/// 地址转换表条目
#[derive(Debug, Clone)]
pub struct TranslationTableEntry {
    /// 输入地址（设备地址）
    pub input_addr: GuestAddr,
    /// 输出地址（物理地址）
    pub output_addr: HostAddr,
    /// 大小
    pub size: u64,
    /// 权限标志
    pub flags: TranslationFlags,
}

/// 地址转换标志
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslationFlags {
    /// 可读
    pub read: bool,
    /// 可写
    pub write: bool,
    /// 可执行
    pub exec: bool,
    /// 特权模式
    pub privileged: bool,
    /// 全局映射
    pub global: bool,
}

impl Default for TranslationFlags {
    fn default() -> Self {
        Self {
            read: true,
            write: true,
            exec: false,
            privileged: false,
            global: false,
        }
    }
}

/// SMMU 虚拟化实例
pub struct SmmuVirtualizer {
    /// SMMU 配置
    config: SmmuConfig,
    /// StreamID 到转换表的映射
    stream_tables: HashMap<StreamId, TranslationTable>,
    /// TLB 缓存
    tlb: SmmuTlb,
    /// 是否已启用
    enabled: bool,
}

/// 地址转换表
#[derive(Debug, Clone)]
struct TranslationTable {
    /// 转换条目
    entries: Vec<TranslationTableEntry>,
    /// 表基地址（保留用于未来扩展）
    #[allow(dead_code)]
    base_addr: GuestAddr,
}

/// SMMU TLB 缓存
#[derive(Debug)]
struct SmmuTlb {
    /// TLB 条目缓存
    cache: HashMap<(StreamId, GuestAddr), HostAddr>,
    /// 最大缓存条目数
    max_entries: usize,
}

impl SmmuTlb {
    fn new(max_entries: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(max_entries),
            max_entries,
        }
    }

    /// 查找 TLB 条目
    fn lookup(&self, stream_id: StreamId, input_addr: GuestAddr) -> Option<HostAddr> {
        self.cache.get(&(stream_id, input_addr)).copied()
    }

    /// 插入 TLB 条目
    fn insert(&mut self, stream_id: StreamId, input_addr: GuestAddr, output_addr: HostAddr) {
        if self.cache.len() >= self.max_entries {
            // 简单的 FIFO 替换策略：移除第一个条目
            if let Some(key) = self.cache.keys().next().copied() {
                self.cache.remove(&key);
            }
        }
        self.cache.insert((stream_id, input_addr), output_addr);
    }

    /// 清除 TLB
    fn flush(&mut self) {
        self.cache.clear();
    }

    /// 按 StreamID 清除 TLB
    fn flush_stream(&mut self, stream_id: StreamId) {
        self.cache.retain(|(sid, _), _| *sid != stream_id);
    }

    /// 按地址范围清除 TLB
    fn flush_range(&mut self, stream_id: StreamId, start: GuestAddr, size: u64) {
        let end = GuestAddr(start.0 + size);
        self.cache.retain(|(sid, addr), _| {
            *sid != stream_id && (*addr < start || *addr >= end)
        });
    }
}

impl SmmuVirtualizer {
    /// 创建新的 SMMU 虚拟化实例
    pub fn new(config: SmmuConfig) -> Self {
        Self {
            config,
            stream_tables: HashMap::new(),
            tlb: SmmuTlb::new(1024), // 默认 1024 个 TLB 条目
            enabled: false,
        }
    }

    /// 启用 SMMU
    pub fn enable(&mut self) -> Result<(), VmError> {
        if self.enabled {
            return Ok(());
        }

        debug!("Enabling SMMU (version: {:?})", self.config.version);
        self.enabled = true;
        Ok(())
    }

    /// 禁用 SMMU
    pub fn disable(&mut self) {
        if !self.enabled {
            return;
        }

        debug!("Disabling SMMU");
        self.enabled = false;
        self.tlb.flush();
    }

    /// 配置 StreamID 的转换表
    pub fn configure_stream(&mut self, stream_id: StreamId, base_addr: GuestAddr) -> Result<(), VmError> {
        let table = TranslationTable {
            entries: Vec::new(),
            base_addr,
        };

        self.stream_tables.insert(stream_id, table);
        debug!("Configured StreamID {} with translation table at {:#x}", stream_id.0, base_addr.0);

        // 清除该 StreamID 的 TLB
        self.tlb.flush_stream(stream_id);

        Ok(())
    }

    /// 添加地址转换条目
    pub fn add_translation(
        &mut self,
        stream_id: StreamId,
        input_addr: GuestAddr,
        output_addr: HostAddr,
        size: u64,
        flags: TranslationFlags,
    ) -> Result<(), VmError> {
        if !self.enabled {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "SMMU is not enabled".to_string(),
                current: "disabled".to_string(),
                expected: "enabled".to_string(),
            }));
        }

        let table = self.stream_tables.get_mut(&stream_id).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidParameter {
                name: "stream_id".to_string(),
                value: stream_id.0.to_string(),
                message: "StreamID not configured".to_string(),
            })
        })?;

        let entry = TranslationTableEntry {
            input_addr,
            output_addr,
            size,
            flags,
        };

        table.entries.push(entry);
        trace!(
            "Added translation: StreamID {}: {:#x} -> {:#x} (size: {})",
            stream_id.0,
            input_addr.0,
            output_addr.0,
            size
        );

        // 更新 TLB
        self.tlb.insert(stream_id, input_addr, output_addr);

        Ok(())
    }

    /// 执行地址转换
    pub fn translate(&self, stream_id: StreamId, input_addr: GuestAddr) -> Result<HostAddr, VmError> {
        if !self.enabled {
            // SMMU 未启用时，直接返回输入地址（直通模式）
            return Ok(HostAddr(input_addr.0));
        }

        // 先检查 TLB
        if let Some(output_addr) = self.tlb.lookup(stream_id, input_addr) {
            trace!(
                "TLB hit: StreamID {}: {:#x} -> {:#x}",
                stream_id.0,
                input_addr.0,
                output_addr.0
            );
            return Ok(output_addr);
        }

        // TLB 未命中，查找转换表
        let table = self.stream_tables.get(&stream_id).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidParameter {
                name: "stream_id".to_string(),
                value: stream_id.0.to_string(),
                message: "StreamID not configured".to_string(),
            })
        })?;

        // 查找匹配的转换条目
        for entry in &table.entries {
            if input_addr.0 >= entry.input_addr.0
                && input_addr.0 < entry.input_addr.0 + entry.size
            {
                let offset = input_addr.0 - entry.input_addr.0;
                let output_addr = HostAddr(entry.output_addr.0 + offset);

                trace!(
                    "Translation: StreamID {}: {:#x} -> {:#x}",
                    stream_id.0,
                    input_addr.0,
                    output_addr.0
                );

                // 注意：这里应该更新 TLB，但 TLB 是 &mut，所以需要在调用者处处理
                return Ok(output_addr);
            }
        }

        // 未找到转换条目，返回错误
        Err(VmError::Core(vm_core::CoreError::InvalidParameter {
            name: "input_addr".to_string(),
            value: format!("{:#x}", input_addr.0),
            message: "Address not mapped in translation table".to_string(),
        }))
    }

    /// 清除 TLB
    pub fn flush_tlb(&mut self) {
        self.tlb.flush();
        debug!("Flushed SMMU TLB");
    }

    /// 按 StreamID 清除 TLB
    pub fn flush_tlb_stream(&mut self, stream_id: StreamId) {
        self.tlb.flush_stream(stream_id);
        debug!("Flushed SMMU TLB for StreamID {}", stream_id.0);
    }

    /// 按地址范围清除 TLB
    pub fn flush_tlb_range(&mut self, stream_id: StreamId, start: GuestAddr, size: u64) {
        self.tlb.flush_range(stream_id, start, size);
        debug!(
            "Flushed SMMU TLB range: StreamID {}: {:#x} - {:#x}",
            stream_id.0,
            start.0,
            start.0 + size
        );
    }

    /// 获取 SMMU 配置
    pub fn config(&self) -> &SmmuConfig {
        &self.config
    }

    /// 检查是否已启用
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smmu_creation() {
        let config = SmmuConfig::default();
        let smmu = SmmuVirtualizer::new(config);
        assert!(!smmu.is_enabled());
    }

    #[test]
    fn test_smmu_enable_disable() {
        let config = SmmuConfig::default();
        let mut smmu = SmmuVirtualizer::new(config);
        
        assert!(smmu.enable().is_ok());
        assert!(smmu.is_enabled());
        
        smmu.disable();
        assert!(!smmu.is_enabled());
    }

    #[test]
    fn test_stream_configuration() {
        let config = SmmuConfig::default();
        let mut smmu = SmmuVirtualizer::new(config);
        smmu.enable().unwrap();

        let stream_id = StreamId(1);
        let base_addr = GuestAddr(0x1000);
        
        assert!(smmu.configure_stream(stream_id, base_addr).is_ok());
    }

    #[test]
    fn test_address_translation() {
        let config = SmmuConfig::default();
        let mut smmu = SmmuVirtualizer::new(config);
        smmu.enable().unwrap();

        let stream_id = StreamId(1);
        let base_addr = GuestAddr(0x1000);
        smmu.configure_stream(stream_id, base_addr).unwrap();

        let input_addr = GuestAddr(0x2000);
        let output_addr = HostAddr(0x5000);
        let flags = TranslationFlags::default();

        assert!(smmu.add_translation(stream_id, input_addr, output_addr, 4096, flags).is_ok());

        // 测试转换
        let translated = smmu.translate(stream_id, GuestAddr(0x2000)).unwrap();
        assert_eq!(translated.0, 0x5000);

        // 测试偏移地址
        let translated2 = smmu.translate(stream_id, GuestAddr(0x2000 + 0x100)).unwrap();
        assert_eq!(translated2.0, 0x5000 + 0x100);
    }

    #[test]
    fn test_tlb_flush() {
        let config = SmmuConfig::default();
        let mut smmu = SmmuVirtualizer::new(config);
        smmu.enable().unwrap();

        let stream_id = StreamId(1);
        let base_addr = GuestAddr(0x1000);
        smmu.configure_stream(stream_id, base_addr).unwrap();

        smmu.flush_tlb();
        smmu.flush_tlb_stream(stream_id);
        smmu.flush_tlb_range(stream_id, GuestAddr(0x2000), 4096);
    }
}

