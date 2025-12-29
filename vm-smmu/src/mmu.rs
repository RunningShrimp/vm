// SMMU设备核心实现
//
// 实现ARM SMMUv3的核心设备功能，包括：
// - 流表管理
// - 上下文描述符管理
// - 多级页表遍历
// - SMMU设备管理

use super::error::{SmmuError, SmmuResult};
use super::{AccessType, PageSize, TLB_ENTRY_MAX};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 流表项（STE）
#[derive(Debug, Clone)]
pub struct StreamTableEntry {
    /// Stream ID
    pub stream_id: u16,
    /// 上下文描述符索引
    pub cd_index: u64,
    /// 流表配置标志
    pub config: u64,
    /// 基础配置
    pub base: u64,
}

/// 上下文描述符（CD）
#[derive(Debug, Clone)]
pub struct ContextDescriptor {
    /// STAGE 1页表指针
    pub s1_ttbr: u64,
    /// STAGE 2页表指针
    pub s2_ttbr: u64,
    /// 转换表指针
    pub ttbr: u64,
    /// 地址空间大小标志
    pub asid_size: u8,
    /// 页表大小标志
    pub granule: u8,
    /// 共享配置
    pub sh_cfg: u64,
    /// 偏移标志
    pub epd: u64,
    /// 允许读写执行标志
    pub perms: u64,
}

/// 页表描述符
#[derive(Debug, Clone)]
pub struct PageTableDescriptor {
    /// 物理地址（4096字节对齐）
    pub pa: u64,
    /// 有效标志
    pub valid: bool,
    /// 允许标志
    pub perms: u8,
    /// 访问标志
    pub attrs: u8,
    /// 连续块大小
    pub cont_hint: u8,
}

/// SMMU设备配置
#[derive(Debug, Clone)]
pub struct SmmuConfig {
    /// Stream表最大条目数
    pub max_stream_entries: usize,
    /// 页表层级
    pub num_stages: usize,
    /// TLB条目数
    pub num_tlb_entries: usize,
    /// MSI支持
    pub msi_enabled: bool,
    /// GERROR支持
    pub gerror_enabled: bool,
    /// 地址空间大小
    pub address_size: usize,
    /// 默认页大小
    pub default_page_size: PageSize,
}

impl Default for SmmuConfig {
    fn default() -> Self {
        Self {
            max_stream_entries: 256,
            num_stages: 2,
            num_tlb_entries: TLB_ENTRY_MAX,
            msi_enabled: true,
            gerror_enabled: true,
            address_size: 48,
            default_page_size: PageSize::Size4KB,
        }
    }
}

/// SMMU统计信息
#[derive(Debug, Clone)]
pub struct SmmuStats {
    /// 总地址转换次数
    pub total_translations: u64,
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 总命令数
    pub total_commands: u64,
    /// 中断次数
    pub interrupts: u64,
    /// MSI消息数
    pub msi_messages: u64,
}

impl Default for SmmuStats {
    fn default() -> Self {
        Self::new()
    }
}

impl SmmuStats {
    pub fn new() -> Self {
        Self {
            total_translations: 0,
            hits: 0,
            misses: 0,
            hit_rate: 0.0,
            total_commands: 0,
            interrupts: 0,
            msi_messages: 0,
        }
    }

    pub fn update_translation(&mut self, hit: bool) {
        self.total_translations += 1;
        if hit {
            self.hits += 1;
        } else {
            self.misses += 1;
        }

        self.hit_rate = if self.total_translations > 0 {
            self.hits as f64 / self.total_translations as f64
        } else {
            0.0
        };
    }
}

impl std::fmt::Display for SmmuStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "SMMU统计信息")?;
        writeln!(f, "  总转换次数: {}", self.total_translations)?;
        writeln!(f, "  命中次数: {}", self.hits)?;
        writeln!(f, "  未命中次数: {}", self.misses)?;
        writeln!(f, "  命中率: {:.2}%", self.hit_rate * 100.0)?;
        writeln!(f, "  总命令数: {}", self.total_commands)?;
        writeln!(f, "  中断次数: {}", self.interrupts)?;
        writeln!(f, "  MSI消息数: {}", self.msi_messages)
    }
}

/// SMMU设备
pub struct SmmuDevice {
    /// 设备ID
    pub device_id: u32,
    /// 物理基地址
    pub base_address: u64,
    /// 配置
    pub config: SmmuConfig,
    /// 流表
    pub stream_table: Arc<RwLock<HashMap<u16, StreamTableEntry>>>,
    /// 上下文描述符表
    pub context_descriptors: Arc<RwLock<HashMap<u64, ContextDescriptor>>>,
    /// 统计信息
    pub stats: Arc<RwLock<SmmuStats>>,
}

impl SmmuDevice {
    /// 创建新的SMMU设备
    ///
    /// # 参数
    /// - `device_id`: 设备ID
    /// - `base_address`: 物理基地址
    /// - `config`: 设备配置
    ///
    /// # 示例
    /// ```ignore
    /// let config = SmmuConfig::default();
    /// let smmu = SmmuDevice::new(0x1, 0x0, config)?;
    /// ```
    pub fn new(device_id: u32, base_address: u64, config: SmmuConfig) -> Self {
        Self {
            device_id,
            base_address,
            config,
            stream_table: Arc::new(RwLock::new(HashMap::new())),
            context_descriptors: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(SmmuStats::new())),
        }
    }

    /// Helper: Lock stats for read operations
    fn lock_stats_read(&self) -> std::sync::RwLockReadGuard<'_, SmmuStats> {
        self.stats.read().expect("Failed to lock stats for reading")
    }

    /// Helper: Lock stats for write operations
    fn lock_stats_write(&self) -> std::sync::RwLockWriteGuard<'_, SmmuStats> {
        self.stats
            .write()
            .expect("Failed to lock stats for writing")
    }

    /// Helper: Lock stream_table for read operations
    fn lock_stream_table_read(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, HashMap<u16, StreamTableEntry>> {
        self.stream_table
            .read()
            .expect("Failed to lock stream_table for reading")
    }

    /// Helper: Lock stream_table for write operations
    fn lock_stream_table_write(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, HashMap<u16, StreamTableEntry>> {
        self.stream_table
            .write()
            .expect("Failed to lock stream_table for writing")
    }

    /// Helper: Lock context_descriptors for read operations
    fn lock_context_descriptors_read(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, HashMap<u64, ContextDescriptor>> {
        self.context_descriptors
            .read()
            .expect("Failed to lock context_descriptors for reading")
    }

    /// Helper: Lock context_descriptors for write operations
    fn lock_context_descriptors_write(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, HashMap<u64, ContextDescriptor>> {
        self.context_descriptors
            .write()
            .expect("Failed to lock context_descriptors for writing")
    }

    /// 地址转换
    ///
    /// # 参数
    /// - `stream_id`: Stream ID
    /// - `va`: 虚拟地址
    /// - `access_type`: 访问类型
    ///
    /// # 返回
    /// - `Ok(pa)`: 物理地址
    /// - `Err(err)`: 错误
    pub fn translate_address(
        &self,
        stream_id: u16,
        va: u64,
        access_type: AccessType,
    ) -> SmmuResult<u64> {
        // 第一步：查找流表（Stream Table Lookup）
        let ste = self.lookup_stream_table(stream_id)?;

        // 第二步：检查访问权限（Permission Check）
        self.check_access_permission(stream_id, access_type)?;

        // 第三步：多级页表遍历（Page Table Walk）
        let pa = self.page_table_walk(ste.cd_index, va)?;

        // 第四步：更新统计
        {
            let mut stats = self.lock_stats_write();
            stats.update_translation(true);
        }

        Ok(pa)
    }

    /// 查找流表
    fn lookup_stream_table(&self, stream_id: u16) -> SmmuResult<StreamTableEntry> {
        let stream_table = self.lock_stream_table_read();

        if let Some(ste) = stream_table.get(&stream_id) {
            Ok(ste.clone())
        } else {
            Err(SmmuError::ConfigError(format!(
                "Stream ID {} not found in stream table",
                stream_id
            )))
        }
    }

    /// 检查访问权限
    fn check_access_permission(&self, _stream_id: u16, access_type: AccessType) -> SmmuResult<()> {
        // 简化的权限检查：假设所有权限都允许
        // 实际实现应该根据上下文描述符（CD）检查

        match access_type {
            AccessType::Read => Ok(()),
            AccessType::Write => Ok(()),
            AccessType::Execute => Ok(()),
            AccessType::Atomic => Ok(()),
        }
    }

    /// 页表遍历
    fn page_table_walk(&self, cd_index: u64, va: u64) -> SmmuResult<u64> {
        // 简化的页表遍历：假设物理地址 = 虚拟地址
        // 实际实现应该执行多级页表遍历

        // 获取上下文描述符
        let _cd = {
            let cds = self.lock_context_descriptors_read();
            cds.get(&cd_index)
                .ok_or_else(|| {
                    SmmuError::ConfigError(format!("Context descriptor {} not found", cd_index))
                })?
                .clone()
        };

        // 简化：直接返回虚拟地址作为物理地址
        // 实际实现应该：CD → S1 → S2 → Translation Table → PA
        Ok(va)
    }

    /// 创建上下文描述符
    ///
    /// # 参数
    /// - `stream_id`: Stream ID
    /// - `cd`: 上下文描述符
    ///
    /// # 示例
    /// ```ignore
    /// let cd = ContextDescriptor {
    ///     s1_ttbr: 0x1000,
    ///     s2_ttbr: 0x2000,
    ///     ttbr: 0x3000,
    ///     asid_size: 48,
    ///     granule: 12,
    ///     sh_cfg: 0,
    ///     epd: 0,
    ///     perms: AccessPermission::ReadWriteExecute as u64,
    /// };
    /// smmu.create_context_descriptor(0x100, cd)?;
    /// ```
    pub fn create_context_descriptor(
        &self,
        stream_id: u16,
        cd: ContextDescriptor,
    ) -> SmmuResult<()> {
        let stream_table = self.lock_stream_table_write();
        let mut cds = self.lock_context_descriptors_write();

        // 检查Stream ID是否已存在
        if !stream_table.contains_key(&stream_id) {
            return Err(SmmuError::InvalidParameter(format!(
                "Stream ID {} not found in stream table",
                stream_id
            )));
        }

        // 插入上下文描述符
        let cd_index = stream_table
            .get(&stream_id)
            .ok_or_else(|| SmmuError::Internal(format!("Stream ID {} disappeared", stream_id)))?
            .cd_index;
        cds.insert(cd_index, cd);

        Ok(())
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> SmmuStats {
        let stats = self.lock_stats_read();
        stats.clone()
    }

    /// 重置统计
    pub fn reset_stats(&self) {
        let mut stats = self.lock_stats_write();
        *stats = SmmuStats::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smmu_config_default() {
        let config = SmmuConfig::default();
        assert_eq!(config.max_stream_entries, 256);
        assert_eq!(config.num_stages, 2);
        assert_eq!(config.num_tlb_entries, 256);
    }

    #[test]
    fn test_smmu_creation() {
        let config = SmmuConfig::default();
        let smmu = SmmuDevice::new(0x1, 0x0, config);
        assert_eq!(smmu.device_id, 0x1);
        assert_eq!(smmu.base_address, 0x0);
    }

    #[test]
    fn test_smmu_stats() {
        let config = SmmuConfig::default();
        let smmu = SmmuDevice::new(0x1, 0x0, config);

        let stats = smmu.get_stats();
        assert_eq!(stats.total_translations, 0);
        assert_eq!(stats.hit_rate, 0.0);
    }

    #[test]
    fn test_smmu_stats_display() {
        let config = SmmuConfig::default();
        let smmu = SmmuDevice::new(0x1, 0x0, config);

        let stats = smmu.get_stats();
        let display = format!("{}", stats);
        assert!(display.contains("SMMU统计信息"));
        assert!(display.contains("总转换次数"));
    }
}
