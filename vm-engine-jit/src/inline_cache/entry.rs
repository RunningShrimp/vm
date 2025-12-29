use vm_core::GuestAddr;

/// 缓存条目
#[derive(Debug, Clone)]
pub enum CacheEntry {
    /// 单态缓存
    Monomorphic(MonomorphicCache),
    /// 多态缓存
    Polymorphic(PolymorphicCache),
}

/// 单态缓存
#[derive(Debug, Clone)]
pub struct MonomorphicCache {
    /// 接收者对象
    pub receiver: u64,
    /// 目标代码指针
    pub code_ptr: GuestAddr,
    /// 命中计数
    pub hit_count: u64,
    /// 未命中计数
    pub miss_count: u64,
    /// 最后访问时间
    pub last_access: std::time::Instant,
    /// 创建时间
    pub created_at: std::time::Instant,
}

impl MonomorphicCache {
    pub fn new(receiver: u64, code_ptr: GuestAddr) -> Self {
        let now = std::time::Instant::now();
        Self {
            receiver,
            code_ptr,
            hit_count: 0,
            miss_count: 0,
            last_access: now,
            created_at: now,
        }
    }

    pub fn is_expired(&self, timeout_ms: u64) -> bool {
        self.last_access.elapsed().as_millis() > timeout_ms as u128
    }
}

/// 多态缓存
#[derive(Debug, Clone)]
pub struct PolymorphicCache {
    /// 缓存条目
    pub entries: Vec<PolymorphicEntry>,
    /// 最后访问时间
    pub last_access: std::time::Instant,
    /// 创建时间
    pub created_at: std::time::Instant,
}

impl PolymorphicCache {
    pub fn new(receiver1: u64, code_ptr1: GuestAddr, receiver2: u64, code_ptr2: GuestAddr) -> Self {
        let now = std::time::Instant::now();
        Self {
            entries: vec![
                PolymorphicEntry::new(receiver1, code_ptr1),
                PolymorphicEntry::new(receiver2, code_ptr2),
            ],
            last_access: now,
            created_at: now,
        }
    }

    pub fn new_with_entries(entries: Vec<PolymorphicEntry>) -> Self {
        let now = std::time::Instant::now();
        Self {
            entries,
            last_access: now,
            created_at: now,
        }
    }

    pub fn find_entry(&self, receiver: u64) -> Option<&PolymorphicEntry> {
        self.entries.iter().find(|e| e.receiver == receiver)
    }

    pub fn is_expired(&self, timeout_ms: u64) -> bool {
        self.last_access.elapsed().as_millis() > timeout_ms as u128
    }
}

/// 多态缓存条目
#[derive(Debug, Clone)]
pub struct PolymorphicEntry {
    /// 接收者对象
    pub receiver: u64,
    /// 目标代码指针
    pub code_ptr: GuestAddr,
    /// 命中计数
    pub hit_count: u64,
    /// 未命中计数
    pub miss_count: u64,
    /// 最后访问时间
    pub last_access: std::time::Instant,
}

impl PolymorphicEntry {
    pub fn new(receiver: u64, code_ptr: GuestAddr) -> Self {
        Self {
            receiver,
            code_ptr,
            hit_count: 0,
            miss_count: 0,
            last_access: std::time::Instant::now(),
        }
    }
}
