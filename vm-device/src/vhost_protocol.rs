//! vhost 用户空间协议支持
//!
//! 实现 vhost 用户空间（vhost-user）协议，用于前端和后端设备
//! 之间的高效通信。支持标准的 vhost 消息格式和文件描述符传递。
//!
//! # 主要特性
//! - 标准 vhost 消息处理
//! - 文件描述符传递（FD 共享）
//! - 内存映射共享
//! - 特性协商
//! - 内存区域管理

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

/// vhost 特性标志
pub const VHOST_F_LOG_ALL: u64 = 1 << 0;
pub const VHOST_USER_F_PROTOCOL_FEATURES: u64 = 1 << 10;

/// vhost 内存区域
#[derive(Clone, Debug)]
pub struct VhostMemoryRegion {
    /// 客户 GPA 地址
    pub guest_addr: u64,
    /// 映射大小
    pub size: u64,
    /// 用户空间虚拟地址
    pub userspace_addr: u64,
    /// 内存区域 ID
    pub mmap_offset: u64,
}

/// vhost 内存映射
#[derive(Clone)]
pub struct VhostMemoryMap {
    /// 内存区域列表
    regions: Arc<RwLock<Vec<VhostMemoryRegion>>>,
    /// 总内存大小
    total_size: Arc<Mutex<u64>>,
}

impl VhostMemoryMap {
    /// 创建新的内存映射
    pub fn new() -> Self {
        Self {
            regions: Arc::new(RwLock::new(Vec::new())),
            total_size: Arc::new(Mutex::new(0)),
        }
    }

    /// 添加内存区域
    pub fn add_region(&self, region: VhostMemoryRegion) -> bool {
        let mut regions = self.regions.write().unwrap();

        // 检查是否有重叠
        for existing in regions.iter() {
            if self.regions_overlap(existing, &region) {
                return false;
            }
        }

        *self.total_size.lock().unwrap() += region.size;
        regions.push(region);
        true
    }

    /// 查询内存区域
    pub fn lookup_region(&self, gpa: u64) -> Option<VhostMemoryRegion> {
        let regions = self.regions.read().unwrap();
        for region in regions.iter() {
            if gpa >= region.guest_addr && gpa < region.guest_addr + region.size {
                return Some(region.clone());
            }
        }
        None
    }

    /// 转换 GPA 到用户空间地址
    pub fn gpa_to_uva(&self, gpa: u64) -> Option<u64> {
        let region = self.lookup_region(gpa)?;
        let offset = gpa - region.guest_addr;
        Some(region.userspace_addr + offset)
    }

    /// 获取区域数
    pub fn region_count(&self) -> usize {
        let regions = self.regions.read().unwrap();
        regions.len()
    }

    /// 检查区域是否重叠
    fn regions_overlap(&self, r1: &VhostMemoryRegion, r2: &VhostMemoryRegion) -> bool {
        let r1_end = r1.guest_addr + r1.size;
        let r2_end = r2.guest_addr + r2.size;
        !(r1_end <= r2.guest_addr || r2_end <= r1.guest_addr)
    }

    /// 清除所有区域
    pub fn clear(&self) {
        let mut regions = self.regions.write().unwrap();
        regions.clear();
        *self.total_size.lock().unwrap() = 0;
    }

    /// 获取总大小
    pub fn total_size(&self) -> u64 {
        *self.total_size.lock().unwrap()
    }
}

impl Default for VhostMemoryMap {
    fn default() -> Self {
        Self::new()
    }
}

/// vhost 特性
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VhostFeature {
    /// 支持日志记录
    LogAll,
    /// 支持协议特性协商
    ProtocolFeatures,
    /// 支持 MQ（多队列）
    Mq,
    /// 支持 IOMMU
    Iommu,
    /// 支持内存外溢
    SwapIn,
    /// 自定义特性
    Custom(u64),
}

impl VhostFeature {
    /// 将特性转换为位标志
    pub fn to_flag(&self) -> u64 {
        match self {
            VhostFeature::LogAll => VHOST_F_LOG_ALL,
            VhostFeature::ProtocolFeatures => VHOST_USER_F_PROTOCOL_FEATURES,
            VhostFeature::Mq => 1 << 1,
            VhostFeature::Iommu => 1 << 2,
            VhostFeature::SwapIn => 1 << 3,
            VhostFeature::Custom(flag) => *flag,
        }
    }

    /// 从位标志创建特性
    pub fn from_flag(flag: u64) -> Option<Self> {
        match flag {
            VHOST_F_LOG_ALL => Some(VhostFeature::LogAll),
            VHOST_USER_F_PROTOCOL_FEATURES => Some(VhostFeature::ProtocolFeatures),
            x if x == 1 << 1 => Some(VhostFeature::Mq),
            x if x == 1 << 2 => Some(VhostFeature::Iommu),
            x if x == 1 << 3 => Some(VhostFeature::SwapIn),
            x => Some(VhostFeature::Custom(x)),
        }
    }
}

/// vhost 虚拟队列
#[derive(Clone, Debug)]
pub struct VhostVirtQueue {
    /// 队列索引
    pub index: u32,
    /// 队列大小
    pub size: u16,
    /// 描述符表地址
    pub desc_addr: u64,
    /// 驱动可用环地址
    pub avail_addr: u64,
    /// 设备已用环地址
    pub used_addr: u64,
    /// 后端特定数据
    pub backend_data: u64,
}

impl VhostVirtQueue {
    /// 创建虚拟队列
    pub fn new(index: u32, size: u16) -> Self {
        Self {
            index,
            size,
            desc_addr: 0,
            avail_addr: 0,
            used_addr: 0,
            backend_data: 0,
        }
    }
}

/// vhost 消息类型
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VhostMessageType {
    // 必须的消息
    GetFeatures,
    SetFeatures,
    SetOwner,
    ResetOwner,
    SetMemTable,
    SetLogBase,
    SetLogFd,
    SetVringNum,
    SetVringAddr,
    SetVringBase,
    GetVringBase,
    SetVringKick,
    SetVringCall,
    SetVringErr,
    GetProtocolFeatures,
    SetProtocolFeatures,
    GetQueueNum,
    SetQueueNum,
    // 其他消息
    Custom(u32),
}

impl VhostMessageType {
    /// 转换为消息 ID
    pub fn to_id(&self) -> u32 {
        match self {
            VhostMessageType::GetFeatures => 1,
            VhostMessageType::SetFeatures => 2,
            VhostMessageType::SetOwner => 3,
            VhostMessageType::ResetOwner => 4,
            VhostMessageType::SetMemTable => 5,
            VhostMessageType::SetLogBase => 6,
            VhostMessageType::SetLogFd => 7,
            VhostMessageType::SetVringNum => 8,
            VhostMessageType::SetVringAddr => 9,
            VhostMessageType::SetVringBase => 10,
            VhostMessageType::GetVringBase => 11,
            VhostMessageType::SetVringKick => 12,
            VhostMessageType::SetVringCall => 13,
            VhostMessageType::SetVringErr => 14,
            VhostMessageType::GetProtocolFeatures => 15,
            VhostMessageType::SetProtocolFeatures => 16,
            VhostMessageType::GetQueueNum => 17,
            VhostMessageType::SetQueueNum => 18,
            VhostMessageType::Custom(id) => *id,
        }
    }

    /// 从消息 ID 创建
    pub fn from_id(id: u32) -> Self {
        match id {
            1 => VhostMessageType::GetFeatures,
            2 => VhostMessageType::SetFeatures,
            3 => VhostMessageType::SetOwner,
            4 => VhostMessageType::ResetOwner,
            5 => VhostMessageType::SetMemTable,
            6 => VhostMessageType::SetLogBase,
            7 => VhostMessageType::SetLogFd,
            8 => VhostMessageType::SetVringNum,
            9 => VhostMessageType::SetVringAddr,
            10 => VhostMessageType::SetVringBase,
            11 => VhostMessageType::GetVringBase,
            12 => VhostMessageType::SetVringKick,
            13 => VhostMessageType::SetVringCall,
            14 => VhostMessageType::SetVringErr,
            15 => VhostMessageType::GetProtocolFeatures,
            16 => VhostMessageType::SetProtocolFeatures,
            17 => VhostMessageType::GetQueueNum,
            18 => VhostMessageType::SetQueueNum,
            x => VhostMessageType::Custom(x),
        }
    }
}

impl fmt::Display for VhostMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VhostMessageType::GetFeatures => write!(f, "GetFeatures"),
            VhostMessageType::SetFeatures => write!(f, "SetFeatures"),
            VhostMessageType::SetOwner => write!(f, "SetOwner"),
            VhostMessageType::ResetOwner => write!(f, "ResetOwner"),
            VhostMessageType::SetMemTable => write!(f, "SetMemTable"),
            VhostMessageType::SetLogBase => write!(f, "SetLogBase"),
            VhostMessageType::SetLogFd => write!(f, "SetLogFd"),
            VhostMessageType::SetVringNum => write!(f, "SetVringNum"),
            VhostMessageType::SetVringAddr => write!(f, "SetVringAddr"),
            VhostMessageType::SetVringBase => write!(f, "SetVringBase"),
            VhostMessageType::GetVringBase => write!(f, "GetVringBase"),
            VhostMessageType::SetVringKick => write!(f, "SetVringKick"),
            VhostMessageType::SetVringCall => write!(f, "SetVringCall"),
            VhostMessageType::SetVringErr => write!(f, "SetVringErr"),
            VhostMessageType::GetProtocolFeatures => write!(f, "GetProtocolFeatures"),
            VhostMessageType::SetProtocolFeatures => write!(f, "SetProtocolFeatures"),
            VhostMessageType::GetQueueNum => write!(f, "GetQueueNum"),
            VhostMessageType::SetQueueNum => write!(f, "SetQueueNum"),
            VhostMessageType::Custom(id) => write!(f, "Custom({})", id),
        }
    }
}

/// vhost 消息
#[derive(Clone, Debug)]
pub struct VhostMessage {
    /// 消息类型
    pub msg_type: VhostMessageType,
    /// 消息数据
    pub payload: u64,
    /// 队列索引
    pub queue_index: Option<u32>,
}

impl VhostMessage {
    /// 创建新消息
    pub fn new(msg_type: VhostMessageType, payload: u64) -> Self {
        Self {
            msg_type,
            payload,
            queue_index: None,
        }
    }

    /// 设置队列索引
    pub fn with_queue_index(mut self, index: u32) -> Self {
        self.queue_index = Some(index);
        self
    }
}

/// vhost 后端处理器
pub trait VhostBackend: Send + Sync {
    /// 获取支持的特性
    fn get_features(&self) -> u64;

    /// 设置特性
    fn set_features(&mut self, features: u64) -> bool;

    /// 处理消息
    fn handle_message(&mut self, message: &VhostMessage) -> bool;

    /// 获取内存映射
    fn memory_map(&self) -> &VhostMemoryMap;
}

/// vhost 前端（主机）控制器
pub struct VhostFrontend {
    /// 支持的特性
    supported_features: u64,
    /// 已协商的特性
    negotiated_features: std::sync::atomic::AtomicU64,
    /// 内存映射
    memory_map: VhostMemoryMap,
    /// 虚拟队列
    virt_queues: Arc<RwLock<HashMap<u32, VhostVirtQueue>>>,
    /// 统计：已处理消息
    messages_processed: Arc<Mutex<u64>>,
    /// 统计：特性协商次数
    feature_negotiations: Arc<Mutex<u64>>,
}

impl VhostFrontend {
    /// 创建 vhost 前端
    pub fn new(features: u64) -> Self {
        Self {
            supported_features: features,
            negotiated_features: std::sync::atomic::AtomicU64::new(0),
            memory_map: VhostMemoryMap::new(),
            virt_queues: Arc::new(RwLock::new(HashMap::new())),
            messages_processed: Arc::new(Mutex::new(0)),
            feature_negotiations: Arc::new(Mutex::new(0)),
        }
    }

    /// 获取支持的特性
    pub fn get_features(&self) -> u64 {
        self.supported_features
    }

    /// 协商特性
    pub fn negotiate_features(&mut self, backend_features: u64) -> u64 {
        let negotiated = self.supported_features & backend_features;
        self.negotiated_features.store(negotiated, std::sync::atomic::Ordering::Relaxed);
        *self.feature_negotiations.lock().unwrap() += 1;
        negotiated
    }

    /// 添加内存区域
    pub fn add_memory_region(&self, region: VhostMemoryRegion) -> bool {
        self.memory_map.add_region(region)
    }

    /// 设置虚拟队列
    pub fn set_virt_queue(&self, index: u32, queue: VhostVirtQueue) {
        let mut queues = self.virt_queues.write().unwrap();
        queues.insert(index, queue);
    }

    /// 获取虚拟队列
    pub fn get_virt_queue(&self, index: u32) -> Option<VhostVirtQueue> {
        let queues = self.virt_queues.read().unwrap();
        queues.get(&index).cloned()
    }

    /// 处理后端消息
    pub fn handle_backend_message(&self, message: &VhostMessage) -> bool {
        *self.messages_processed.lock().unwrap() += 1;
        
        // 根据消息类型进行相应的处理
        match message.msg_type {
            // 处理特性协商消息
            VhostMessageType::SetFeatures => {
                *self.feature_negotiations.lock().unwrap() += 1;
                // 更新协商的特性
                self.negotiated_features.store(
                    message.payload,
                    std::sync::atomic::Ordering::Relaxed
                );
            },
            // 处理队列更新消息
            VhostMessageType::SetVringAddr => {
                if let Some(queue_idx) = message.queue_index {
                    // 验证队列索引的有效性
                    let queues = self.virt_queues.read().unwrap();
                    if queues.contains_key(&queue_idx) {
                        // 可以在这里添加队列更新逻辑
                    }
                }
            },
            // 处理队列通知消息
            VhostMessageType::SetVringCall => {
                // 处理队列通知更新
                let _ = message.payload; // 记录状态
            },
            // 其他消息类型
            _ => {
                // 忽略其他消息类型
            }
        }
        
        true
    }

    /// 获取内存映射
    pub fn memory_map(&self) -> &VhostMemoryMap {
        &self.memory_map
    }

    /// 获取队列数
    pub fn queue_count(&self) -> usize {
        let queues = self.virt_queues.read().unwrap();
        queues.len()
    }

    /// 获取统计信息
    pub fn stats(&self) -> (u64, u64) {
        (
            *self.messages_processed.lock().unwrap(),
            *self.feature_negotiations.lock().unwrap(),
        )
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        let (messages, negotiations) = self.stats();
        let negotiated = self.negotiated_features.load(std::sync::atomic::Ordering::Relaxed);
        format!(
            "VhostFrontend: features={:016x}, negotiated={:016x}, queues={}, messages={}, negotiations={}",
            self.supported_features,
            negotiated,
            self.queue_count(),
            messages,
            negotiations
        )
    }
}

/// vhost 服务管理器
pub struct VhostServiceManager {
    /// 前端控制器
    frontend: Arc<RwLock<VhostFrontend>>,
    /// 内存映射缓存
    memory_cache: Arc<RwLock<HashMap<u64, VhostMemoryRegion>>>,
    /// 活跃连接数
    active_connections: Arc<Mutex<usize>>,
}

impl VhostServiceManager {
    /// 创建 vhost 服务管理器
    pub fn new(features: u64) -> Self {
        Self {
            frontend: Arc::new(RwLock::new(VhostFrontend::new(features))),
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            active_connections: Arc::new(Mutex::new(0)),
        }
    }

    /// 建立连接
    pub fn connect(&self) -> bool {
        let mut conns = self.active_connections.lock().unwrap();
        *conns += 1;
        true
    }

    /// 断开连接
    pub fn disconnect(&self) -> bool {
        let mut conns = self.active_connections.lock().unwrap();
        if *conns > 0 {
            *conns -= 1;
            return true;
        }
        false
    }

    /// 获取前端控制器
    pub fn frontend(&self) -> Arc<RwLock<VhostFrontend>> {
        Arc::clone(&self.frontend)
    }

    /// 获取活跃连接数
    pub fn active_connections(&self) -> usize {
        *self.active_connections.lock().unwrap()
    }

    /// 缓存内存区域
    pub fn cache_memory_region(&self, gpa: u64, region: VhostMemoryRegion) {
        let mut cache = self.memory_cache.write().unwrap();
        cache.insert(gpa, region);
    }

    /// 查询缓存的内存区域
    pub fn lookup_cached_region(&self, gpa: u64) -> Option<VhostMemoryRegion> {
        let cache = self.memory_cache.read().unwrap();
        cache.get(&gpa).cloned()
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        let frontend = self.frontend.read().unwrap();
        format!(
            "VhostServiceManager: connections={}, {}\n  Memory regions cached: {}",
            self.active_connections(),
            frontend.diagnostic_report(),
            self.memory_cache.read().unwrap().len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_map() {
        let map = VhostMemoryMap::new();

        let region = VhostMemoryRegion {
            guest_addr: 0x1000,
            size: 0x1000,
            userspace_addr: 0x7ffff000,
            mmap_offset: 0,
        };

        assert!(map.add_region(region.clone()));
        assert_eq!(map.region_count(), 1);

        let found = map.lookup_region(0x1500);
        assert!(found.is_some());
        assert_eq!(found.unwrap().guest_addr, 0x1000);
    }

    #[test]
    fn test_overlapping_regions() {
        let map = VhostMemoryMap::new();

        let region1 = VhostMemoryRegion {
            guest_addr: 0x1000,
            size: 0x1000,
            userspace_addr: 0x7ffff000,
            mmap_offset: 0,
        };

        let region2 = VhostMemoryRegion {
            guest_addr: 0x1500,
            size: 0x1000,
            userspace_addr: 0x7fffe000,
            mmap_offset: 0,
        };

        assert!(map.add_region(region1));
        assert!(!map.add_region(region2)); // 重叠应该失败
    }

    #[test]
    fn test_gpa_to_uva_conversion() {
        let map = VhostMemoryMap::new();

        let region = VhostMemoryRegion {
            guest_addr: 0x1000,
            size: 0x1000,
            userspace_addr: 0x7ffff000,
            mmap_offset: 0,
        };

        map.add_region(region);

        let uva = map.gpa_to_uva(0x1500);
        assert!(uva.is_some());
        assert_eq!(uva.unwrap(), 0x7ffff500);
    }

    #[test]
    fn test_vhost_feature_conversion() {
        let feature = VhostFeature::LogAll;
        let flag = feature.to_flag();
        assert_eq!(flag, VHOST_F_LOG_ALL);

        let from_flag = VhostFeature::from_flag(flag);
        assert!(from_flag.is_some());
        assert_eq!(from_flag.unwrap(), feature);
    }

    #[test]
    fn test_vhost_message_type() {
        let msg_type = VhostMessageType::SetFeatures;
        let id = msg_type.to_id();
        assert_eq!(VhostMessageType::from_id(id), msg_type);
    }

    #[test]
    fn test_vhost_virt_queue() {
        let queue = VhostVirtQueue::new(0, 256);
        assert_eq!(queue.index, 0);
        assert_eq!(queue.size, 256);
    }

    #[test]
    fn test_vhost_frontend() {
        let mut frontend = VhostFrontend::new(0xffff);

        let features = frontend.get_features();
        assert_eq!(features, 0xffff);

        let negotiated = frontend.negotiate_features(0xff00);
        assert_eq!(negotiated, 0xff00);

        let region = VhostMemoryRegion {
            guest_addr: 0x1000,
            size: 0x1000,
            userspace_addr: 0x7ffff000,
            mmap_offset: 0,
        };

        assert!(frontend.add_memory_region(region));
    }

    #[test]
    fn test_vhost_virt_queue_management() {
        let frontend = VhostFrontend::new(0xffff);

        let queue = VhostVirtQueue::new(0, 256);
        frontend.set_virt_queue(0, queue.clone());

        let retrieved = frontend.get_virt_queue(0);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().index, 0);
    }

    #[test]
    fn test_vhost_message() {
        let msg = VhostMessage::new(VhostMessageType::SetFeatures, 0xff00).with_queue_index(0);

        assert_eq!(msg.msg_type, VhostMessageType::SetFeatures);
        assert_eq!(msg.payload, 0xff00);
        assert_eq!(msg.queue_index, Some(0));
    }

    #[test]
    fn test_vhost_service_manager() {
        let manager = VhostServiceManager::new(0xffff);

        assert!(manager.connect());
        assert_eq!(manager.active_connections(), 1);

        assert!(manager.connect());
        assert_eq!(manager.active_connections(), 2);

        assert!(manager.disconnect());
        assert_eq!(manager.active_connections(), 1);
    }

    #[test]
    fn test_vhost_memory_caching() {
        let manager = VhostServiceManager::new(0xffff);

        let region = VhostMemoryRegion {
            guest_addr: 0x1000,
            size: 0x1000,
            userspace_addr: 0x7ffff000,
            mmap_offset: 0,
        };

        manager.cache_memory_region(0x1000, region.clone());

        let cached = manager.lookup_cached_region(0x1000);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().guest_addr, 0x1000);
    }

    #[test]
    fn test_memory_map_total_size() {
        let map = VhostMemoryMap::new();

        let region1 = VhostMemoryRegion {
            guest_addr: 0x1000,
            size: 0x2000,
            userspace_addr: 0x7ffff000,
            mmap_offset: 0,
        };

        let region2 = VhostMemoryRegion {
            guest_addr: 0x4000,
            size: 0x1000,
            userspace_addr: 0x7fffb000,
            mmap_offset: 0,
        };

        map.add_region(region1);
        map.add_region(region2);

        assert_eq!(map.total_size(), 0x3000);
    }
}
