//! 共享类型定义
//!
//! 定义在多个GC实现中共享的通用类型

/// 对象指针（简化的内存地址）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectPtr(pub usize);

impl ObjectPtr {
    /// 创建空指针
    pub fn null() -> Self {
        Self(0)
    }

    /// 是否为空
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// 获取地址
    pub fn addr(&self) -> usize {
        self.0
    }
}

/// 对象元数据
#[derive(Debug, Clone)]
pub struct ObjectMetadata {
    /// 对象大小
    pub size: usize,
    /// 对象年龄（经历过多少次GC）
    pub age: u8,
    /// 是否已标记
    pub marked: bool,
}

impl ObjectMetadata {
    /// 创建新的元数据
    pub fn new(size: usize) -> Self {
        Self {
            size,
            age: 0,
            marked: false,
        }
    }

    /// 增加年龄
    pub fn increment_age(&mut self) {
        self.age = self.age.saturating_add(1);
    }

    /// 重置标记
    pub fn reset_mark(&mut self) {
        self.marked = false;
    }
}

impl Default for ObjectMetadata {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_ptr() {
        let ptr = ObjectPtr(0x1000);
        assert!(!ptr.is_null());
        assert_eq!(ptr.addr(), 0x1000);

        let null_ptr = ObjectPtr::null();
        assert!(null_ptr.is_null());
    }

    #[test]
    fn test_object_metadata() {
        let mut meta = ObjectMetadata::new(1024);
        assert_eq!(meta.size, 1024);
        assert_eq!(meta.age, 0);
        assert!(!meta.marked);

        meta.increment_age();
        assert_eq!(meta.age, 1);

        meta.marked = true;
        meta.reset_mark();
        assert!(!meta.marked);
    }
}
