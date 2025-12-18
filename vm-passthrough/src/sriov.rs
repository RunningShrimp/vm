//! SR-IOV 虚拟函数直通
//!
//! 实现 SR-IOV (Single Root I/O Virtualization) 虚拟函数管理和直通,
//! 支持高性能的 I/O 直通和隔离。
//!
//! # 主要特性
//! - 虚拟函数（VF）生命周期管理
//! - VF 资源分配和隔离
//! - MAC 地址和 VLAN 管理
//! - QoS 和带宽限制
//! - 直通性能监控

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// PCIe 虚拟函数标识
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VfId {
    /// PF Bus:Device.Function
    pub pf_bdf: u16,
    /// VF 索引
    pub vf_index: u16,
}

impl VfId {
    /// 创建 VF ID
    pub fn new(pf_bdf: u16, vf_index: u16) -> Self {
        Self { pf_bdf, vf_index }
    }

    /// 获取 VF Bus Device Function
    pub fn get_vf_bdf(&self) -> u16 {
        let base = self.pf_bdf & 0xfff8; // 清除 Function 部分
        base + (self.vf_index & 0xff)
    }
}

/// VF 状态
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfState {
    /// 未初始化
    Uninitialized,
    /// 已初始化
    Initialized,
    /// 已启用
    Enabled,
    /// 已禁用
    Disabled,
    /// 故障
    Faulty,
}

/// VF MAC 地址管理
#[derive(Clone, Debug)]
pub struct VfMacConfig {
    /// MAC 地址
    pub mac_addr: [u8; 6],
    /// 是否为管理员设置的 MAC
    pub admin_set: bool,
}

impl VfMacConfig {
    /// 创建 MAC 配置
    pub fn new(mac_addr: [u8; 6]) -> Self {
        Self {
            mac_addr,
            admin_set: false,
        }
    }

    /// MAC 地址为字符串
    pub fn to_string(&self) -> String {
        format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.mac_addr[0],
            self.mac_addr[1],
            self.mac_addr[2],
            self.mac_addr[3],
            self.mac_addr[4],
            self.mac_addr[5]
        )
    }
}

/// VLAN 配置
#[derive(Clone, Debug)]
pub struct VlanConfig {
    /// VLAN ID（0-4095）
    pub vlan_id: u16,
    /// VLAN 优先级（0-7）
    pub priority: u8,
    /// 是否为 QinQ
    pub qinq: bool,
}

impl VlanConfig {
    /// 创建 VLAN 配置
    pub fn new(vlan_id: u16, priority: u8) -> Self {
        Self {
            vlan_id,
            priority,
            qinq: false,
        }
    }

    /// 检查是否有效
    pub fn is_valid(&self) -> bool {
        self.vlan_id <= 4095 && self.priority <= 7
    }
}

/// QoS 配置
#[derive(Clone, Copy, Debug)]
pub struct QosConfig {
    /// 入站带宽限制（Mbps）
    pub ingress_bandwidth_mbps: u32,
    /// 出站带宽限制（Mbps）
    pub egress_bandwidth_mbps: u32,
    /// 最小保证带宽（Mbps）
    pub min_bandwidth_mbps: u32,
}

impl QosConfig {
    /// 创建 QoS 配置
    pub fn new(ingress: u32, egress: u32, min_bw: u32) -> Self {
        Self {
            ingress_bandwidth_mbps: ingress,
            egress_bandwidth_mbps: egress,
            min_bandwidth_mbps: min_bw,
        }
    }

    /// 检查是否有效
    pub fn is_valid(&self) -> bool {
        self.min_bandwidth_mbps <= self.ingress_bandwidth_mbps
            && self.min_bandwidth_mbps <= self.egress_bandwidth_mbps
    }
}

/// VF 配置
#[derive(Clone)]
pub struct VfConfig {
    /// VF ID
    pub vf_id: VfId,
    /// 状态
    pub state: VfState,
    /// MAC 地址配置
    pub mac_config: VfMacConfig,
    /// VLAN 配置
    pub vlan_config: Option<VlanConfig>,
    /// QoS 配置
    pub qos_config: Option<QosConfig>,
    /// 分配的内存大小（字节）
    pub memory_size: u64,
    /// 分配的中断数
    pub interrupt_count: u32,
}

impl VfConfig {
    /// 创建 VF 配置
    pub fn new(vf_id: VfId, mac_addr: [u8; 6]) -> Self {
        Self {
            vf_id,
            state: VfState::Uninitialized,
            mac_config: VfMacConfig::new(mac_addr),
            vlan_config: None,
            qos_config: None,
            memory_size: 0,
            interrupt_count: 0,
        }
    }

    /// 设置 VLAN 配置
    pub fn set_vlan(&mut self, vlan_config: VlanConfig) -> bool {
        if !vlan_config.is_valid() {
            return false;
        }
        self.vlan_config = Some(vlan_config);
        true
    }

    /// 设置 QoS 配置
    pub fn set_qos(&mut self, qos_config: QosConfig) -> bool {
        if !qos_config.is_valid() {
            return false;
        }
        self.qos_config = Some(qos_config);
        true
    }

    /// 分配资源
    pub fn allocate_resources(&mut self, memory: u64, interrupts: u32) {
        self.memory_size = memory;
        self.interrupt_count = interrupts;
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        format!(
            "VfConfig(BDF={:04x}): state={:?}, MAC={}, memory={} KB, interrupts={}",
            self.vf_id.get_vf_bdf(),
            self.state,
            self.mac_config.to_string(),
            self.memory_size / 1024,
            self.interrupt_count
        )
    }
}

/// VF 性能统计
#[derive(Clone, Copy, Debug, Default)]
pub struct VfPerfStats {
    /// 处理的包数
    pub packets_processed: u64,
    /// 处理的字节数
    pub bytes_processed: u64,
    /// 丢弃的包数
    pub packets_dropped: u64,
    /// 总延迟（纳秒）
    pub total_latency_ns: u64,
}

impl VfPerfStats {
    /// 平均延迟（微秒）
    pub fn avg_latency_us(&self) -> f64 {
        if self.packets_processed == 0 {
            return 0.0;
        }
        self.total_latency_ns as f64 / (self.packets_processed as f64 * 1000.0)
    }

    /// 吞吐量（Mbps）
    pub fn throughput_mbps(&self) -> f64 {
        if self.total_latency_ns == 0 {
            return 0.0;
        }
        (self.bytes_processed as f64 * 8.0 / self.total_latency_ns as f64) * 1000.0
    }

    /// 丢包率（%）
    pub fn drop_rate(&self) -> f64 {
        let total = self.packets_processed + self.packets_dropped;
        if total == 0 {
            return 0.0;
        }
        (self.packets_dropped as f64 / total as f64) * 100.0
    }
}

/// SR-IOV VF 管理器
pub struct SriovVfManager {
    /// VF 配置映射
    vf_configs: Arc<RwLock<HashMap<VfId, VfConfig>>>,
    /// VF 性能统计
    vf_stats: Arc<RwLock<HashMap<VfId, VfPerfStats>>>,
    /// 最大 VF 数
    max_vfs: usize,
    /// 已创建的 VF 数
    created_vfs: Arc<Mutex<usize>>,
}

impl SriovVfManager {
    /// 创建 SR-IOV VF 管理器
    pub fn new(max_vfs: usize) -> Self {
        Self {
            vf_configs: Arc::new(RwLock::new(HashMap::new())),
            vf_stats: Arc::new(RwLock::new(HashMap::new())),
            max_vfs,
            created_vfs: Arc::new(Mutex::new(0)),
        }
    }

    /// 创建 VF
    pub fn create_vf(&self, vf_id: VfId, mac_addr: [u8; 6]) -> bool {
        let mut configs = self.vf_configs.write().unwrap();

        // 检查是否已存在
        if configs.contains_key(&vf_id) {
            return false;
        }

        // 检查是否超过最大数
        let mut created = self.created_vfs.lock().unwrap();
        if *created >= self.max_vfs {
            return false;
        }

        let mut config = VfConfig::new(vf_id, mac_addr);
        config.state = VfState::Initialized;

        configs.insert(vf_id, config);
        *created += 1;

        // 初始化性能统计
        let mut stats = self.vf_stats.write().unwrap();
        stats.insert(vf_id, VfPerfStats::default());

        true
    }

    /// 销毁 VF
    pub fn destroy_vf(&self, vf_id: VfId) -> bool {
        let mut configs = self.vf_configs.write().unwrap();
        let mut stats = self.vf_stats.write().unwrap();

        if configs.remove(&vf_id).is_some() {
            stats.remove(&vf_id);
            let mut created = self.created_vfs.lock().unwrap();
            if *created > 0 {
                *created -= 1;
            }
            return true;
        }

        false
    }

    /// 启用 VF
    pub fn enable_vf(&self, vf_id: VfId) -> bool {
        let mut configs = self.vf_configs.write().unwrap();
        if let Some(config) = configs.get_mut(&vf_id)
            && (config.state == VfState::Initialized || config.state == VfState::Disabled)
        {
            config.state = VfState::Enabled;
            return true;
        }
        false
    }

    /// 禁用 VF
    pub fn disable_vf(&self, vf_id: VfId) -> bool {
        let mut configs = self.vf_configs.write().unwrap();
        if let Some(config) = configs.get_mut(&vf_id)
            && config.state == VfState::Enabled
        {
            config.state = VfState::Disabled;
            return true;
        }
        false
    }

    /// 获取 VF 配置
    pub fn get_vf_config(&self, vf_id: VfId) -> Option<VfConfig> {
        let configs = self.vf_configs.read().unwrap();
        configs.get(&vf_id).cloned()
    }

    /// 设置 VLAN
    pub fn set_vlan(&self, vf_id: VfId, vlan_config: VlanConfig) -> bool {
        let mut configs = self.vf_configs.write().unwrap();
        if let Some(config) = configs.get_mut(&vf_id) {
            return config.set_vlan(vlan_config);
        }
        false
    }

    /// 设置 QoS
    pub fn set_qos(&self, vf_id: VfId, qos_config: QosConfig) -> bool {
        let mut configs = self.vf_configs.write().unwrap();
        if let Some(config) = configs.get_mut(&vf_id) {
            return config.set_qos(qos_config);
        }
        false
    }

    /// 分配资源
    pub fn allocate_resources(&self, vf_id: VfId, memory: u64, interrupts: u32) -> bool {
        let mut configs = self.vf_configs.write().unwrap();
        if let Some(config) = configs.get_mut(&vf_id) {
            config.allocate_resources(memory, interrupts);
            return true;
        }
        false
    }

    /// 更新性能统计
    pub fn update_stats(
        &self,
        vf_id: VfId,
        packets: u64,
        bytes: u64,
        dropped: u64,
        latency_ns: u64,
    ) -> bool {
        let mut stats = self.vf_stats.write().unwrap();
        if let Some(stat) = stats.get_mut(&vf_id) {
            stat.packets_processed += packets;
            stat.bytes_processed += bytes;
            stat.packets_dropped += dropped;
            stat.total_latency_ns += latency_ns;
            return true;
        }
        false
    }

    /// 获取 VF 统计
    pub fn get_stats(&self, vf_id: VfId) -> Option<VfPerfStats> {
        let stats = self.vf_stats.read().unwrap();
        stats.get(&vf_id).copied()
    }

    /// 获取所有 VF
    pub fn list_vfs(&self) -> Vec<VfId> {
        let configs = self.vf_configs.read().unwrap();
        configs.keys().copied().collect()
    }

    /// 获取已创建的 VF 数
    pub fn vf_count(&self) -> usize {
        *self.created_vfs.lock().unwrap()
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        let configs = self.vf_configs.read().unwrap();
        let stats = self.vf_stats.read().unwrap();

        let mut report = format!(
            "SriovVfManager: max_vfs={}, created={}\n",
            self.max_vfs,
            self.vf_count()
        );

        for (vf_id, config) in configs.iter() {
            report.push_str(&format!("  {}\n", config.diagnostic_report()));

            if let Some(vf_stats) = stats.get(vf_id) {
                report.push_str(&format!(
                    "    stats: throughput={:.1} Mbps, latency={:.2} us, drop_rate={:.2}%\n",
                    vf_stats.throughput_mbps(),
                    vf_stats.avg_latency_us(),
                    vf_stats.drop_rate()
                ));
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vf_id_creation() {
        let vf_id = VfId::new(0x0001, 0);
        assert_eq!(vf_id.pf_bdf, 0x0001);
        assert_eq!(vf_id.vf_index, 0);
    }

    #[test]
    fn test_vf_id_get_bdf() {
        let vf_id = VfId::new(0x0008, 3);
        let bdf = vf_id.get_vf_bdf();
        // Base (0x0008 & 0xfff8) = 0x0008, plus vf_index 3 = 0x000b
        assert_eq!(bdf, 0x000b);
    }

    #[test]
    fn test_vf_mac_config() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let config = VfMacConfig::new(mac);
        assert_eq!(config.mac_addr, mac);
        assert!(!config.admin_set);

        let mac_str = config.to_string();
        assert_eq!(mac_str, "52:54:00:12:34:56");
    }

    #[test]
    fn test_vlan_config_valid() {
        let vlan = VlanConfig::new(100, 3);
        assert!(vlan.is_valid());

        let invalid_vlan = VlanConfig::new(5000, 3);
        assert!(!invalid_vlan.is_valid());

        let invalid_priority = VlanConfig::new(100, 8);
        assert!(!invalid_priority.is_valid());
    }

    #[test]
    fn test_qos_config_valid() {
        let qos = QosConfig::new(1000, 1000, 500);
        assert!(qos.is_valid());

        let invalid_qos = QosConfig::new(1000, 1000, 1500);
        assert!(!invalid_qos.is_valid());
    }

    #[test]
    fn test_vf_config_creation() {
        let vf_id = VfId::new(0x0001, 0);
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let config = VfConfig::new(vf_id, mac);

        assert_eq!(config.state, VfState::Uninitialized);
        assert_eq!(config.mac_config.mac_addr, mac);
        assert_eq!(config.memory_size, 0);
    }

    #[test]
    fn test_vf_config_set_vlan() {
        let vf_id = VfId::new(0x0001, 0);
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let mut config = VfConfig::new(vf_id, mac);

        let vlan = VlanConfig::new(100, 3);
        assert!(config.set_vlan(vlan.clone()));
        assert!(config.vlan_config.is_some());
    }

    #[test]
    fn test_vf_config_set_qos() {
        let vf_id = VfId::new(0x0001, 0);
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let mut config = VfConfig::new(vf_id, mac);

        let qos = QosConfig::new(1000, 1000, 500);
        assert!(config.set_qos(qos));
        assert!(config.qos_config.is_some());
    }

    #[test]
    fn test_vf_perf_stats() {
        let mut stats = VfPerfStats::default();
        stats.packets_processed = 1000;
        stats.bytes_processed = 1_000_000;
        stats.total_latency_ns = 1_000_000_000;

        assert!(stats.avg_latency_us() > 999.0 && stats.avg_latency_us() < 1001.0);
    }

    #[test]
    fn test_vf_perf_stats_drop_rate() {
        let mut stats = VfPerfStats::default();
        stats.packets_processed = 900;
        stats.packets_dropped = 100;

        let drop_rate = stats.drop_rate();
        assert!(drop_rate > 9.9 && drop_rate < 10.1); // ~10%
    }

    #[test]
    fn test_sriov_vf_manager_create() {
        let manager = SriovVfManager::new(8);

        let vf_id = VfId::new(0x0001, 0);
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];

        assert!(manager.create_vf(vf_id, mac));
        assert_eq!(manager.vf_count(), 1);

        // 不能创建相同的 VF
        assert!(!manager.create_vf(vf_id, mac));
        assert_eq!(manager.vf_count(), 1);
    }

    #[test]
    fn test_sriov_vf_manager_destroy() {
        let manager = SriovVfManager::new(8);

        let vf_id = VfId::new(0x0001, 0);
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];

        manager.create_vf(vf_id, mac);
        assert_eq!(manager.vf_count(), 1);

        assert!(manager.destroy_vf(vf_id));
        assert_eq!(manager.vf_count(), 0);
    }

    #[test]
    fn test_sriov_vf_manager_enable_disable() {
        let manager = SriovVfManager::new(8);

        let vf_id = VfId::new(0x0001, 0);
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];

        manager.create_vf(vf_id, mac);

        assert!(manager.enable_vf(vf_id));
        let config = manager.get_vf_config(vf_id).unwrap();
        assert_eq!(config.state, VfState::Enabled);

        assert!(manager.disable_vf(vf_id));
        let config = manager.get_vf_config(vf_id).unwrap();
        assert_eq!(config.state, VfState::Disabled);
    }

    #[test]
    fn test_sriov_vf_manager_resources() {
        let manager = SriovVfManager::new(8);

        let vf_id = VfId::new(0x0001, 0);
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];

        manager.create_vf(vf_id, mac);
        assert!(manager.allocate_resources(vf_id, 1024 * 1024, 8));

        let config = manager.get_vf_config(vf_id).unwrap();
        assert_eq!(config.memory_size, 1024 * 1024);
        assert_eq!(config.interrupt_count, 8);
    }

    #[test]
    fn test_sriov_vf_manager_stats() {
        let manager = SriovVfManager::new(8);

        let vf_id = VfId::new(0x0001, 0);
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];

        manager.create_vf(vf_id, mac);
        manager.update_stats(vf_id, 1000, 1_000_000, 10, 1_000_000_000);

        let stats = manager.get_stats(vf_id).unwrap();
        assert_eq!(stats.packets_processed, 1000);
        assert_eq!(stats.bytes_processed, 1_000_000);
    }

    #[test]
    fn test_sriov_vf_manager_max_vfs() {
        let manager = SriovVfManager::new(2);

        let vf_id1 = VfId::new(0x0001, 0);
        let vf_id2 = VfId::new(0x0001, 1);
        let vf_id3 = VfId::new(0x0001, 2);

        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];

        assert!(manager.create_vf(vf_id1, mac));
        assert!(manager.create_vf(vf_id2, mac));
        assert!(!manager.create_vf(vf_id3, mac)); // 超过最大数
    }
}
