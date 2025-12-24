use std::collections::HashSet;
use parking_lot::RwLock;

/// 记忆集
/// 
/// 记录从老生代到新生代的引用，用于 Minor GC 的根扫描
pub struct RememberedSet {
    /// 引用集合：(老生代对象, 新生代对象)
    refs: RwLock<HashSet<(u64, u64)>>,
    /// 最大容量
    max_size: usize,
}

impl RememberedSet {
    /// 创建新的记忆集
    pub fn new(max_size: usize) -> Self {
        Self {
            refs: RwLock::new(HashSet::new()),
            max_size,
        }
    }

    /// 添加跨代引用
    pub fn add(&self, from_addr: u64, to_addr: u64) {
        let mut refs = self.refs.write();

        if refs.len() >= self.max_size {
            // 容量不足，清除最早的引用（LRU 策略简化实现）
            refs.clear();
        }

        refs.insert((from_addr, to_addr));
    }

    /// 获取所有根对象（老生代对象）
    pub fn get_roots(&self) -> Vec<u64> {
        let refs = self.refs.read();
        refs.iter().map(|(from, _)| *from).collect()
    }

    /// 获取所有引用的新生代对象
    pub fn get_young_refs(&self) -> Vec<u64> {
        let refs = self.refs.read();
        refs.iter().map(|(_, to)| *to).collect()
    }

    /// 检查是否包含特定引用
    pub fn contains(&self, from_addr: u64, to_addr: u64) -> bool {
        let refs = self.refs.read();
        refs.contains(&(from_addr, to_addr))
    }

    /// 移除引用
    pub fn remove(&self, from_addr: u64, to_addr: u64) {
        let mut refs = self.refs.write();
        refs.remove(&(from_addr, to_addr));
    }

    /// 清空记忆集
    pub fn clear(&self) {
        self.refs.write().clear();
    }

    /// 获取大小
    pub fn len(&self) -> usize {
        self.refs.read().len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.refs.read().is_empty()
    }
}
