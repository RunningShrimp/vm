//! GC 对象表示

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use super::error::{GCError, GCResult};

/// 对象类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectType {
    /// 数组
    Array,
    /// 结构体
    Struct,
    /// 字符串
    String,
    /// 函数
    Function,
    /// 闭包
    Closure,
    /// 其他
    Other,
}

/// 对象头
#[derive(Debug)]
pub struct ObjectHeader {
    /// 对象类型
    pub obj_type: AtomicU8,
    /// 对象大小（字节）
    pub size: AtomicU64,
    /// 标记位（用于 GC 标记）
    pub mark_bit: AtomicU8,
    /// 年龄（用于分代 GC）
    pub age: AtomicU8,
}

impl Clone for ObjectHeader {
    fn clone(&self) -> Self {
        Self {
            obj_type: AtomicU8::new(self.obj_type.load(Ordering::Acquire)),
            size: AtomicU64::new(self.size.load(Ordering::Acquire)),
            mark_bit: AtomicU8::new(self.mark_bit.load(Ordering::Acquire)),
            age: AtomicU8::new(self.age.load(Ordering::Acquire)),
        }
    }
}

impl ObjectHeader {
    /// 创建新的对象头
    pub fn new(obj_type: ObjectType, size: usize) -> Self {
        Self {
            obj_type: AtomicU8::new(obj_type as u8),
            size: AtomicU64::new(size as u64),
            mark_bit: AtomicU8::new(0),
            age: AtomicU8::new(0),
        }
    }

    /// 获取对象类型
    pub fn get_type(&self) -> ObjectType {
        match self.obj_type.load(Ordering::Acquire) {
            0 => ObjectType::Array,
            1 => ObjectType::Struct,
            2 => ObjectType::String,
            3 => ObjectType::Function,
            4 => ObjectType::Closure,
            _ => ObjectType::Other,
        }
    }

    /// 获取对象大小
    pub fn get_size(&self) -> usize {
        self.size.load(Ordering::Acquire) as usize
    }

    /// 获取标记位
    pub fn is_marked(&self) -> bool {
        self.mark_bit.load(Ordering::Acquire) != 0
    }

    /// 设置标记位
    pub fn set_mark(&self, marked: bool) {
        self.mark_bit.store(marked as u8, Ordering::Release);
    }

    /// 获取对象年龄
    pub fn get_age(&self) -> u8 {
        self.age.load(Ordering::Acquire)
    }

    /// 增加对象年龄
    pub fn increment_age(&self) {
        self.age.fetch_add(1, Ordering::Release);
    }

    /// 重置标记位和年龄
    pub fn reset(&self) {
        self.mark_bit.store(0, Ordering::Release);
        self.age.store(0, Ordering::Release);
    }
}

/// GC 对象指针
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GCObjectPtr {
    /// 地址
    addr: u64,
    /// 版本（用于检测悬空指针）
    version: u32,
}

impl GCObjectPtr {
    /// 创建空指针
    pub fn null() -> Self {
        Self {
            addr: 0,
            version: 0,
        }
    }

    /// 创建新的对象指针
    pub fn new(addr: u64, version: u32) -> Self {
        Self { addr, version }
    }

    /// 检查是否为空指针
    pub fn is_null(&self) -> bool {
        self.addr == 0
    }

    /// 获取地址
    pub fn addr(&self) -> u64 {
        self.addr
    }

    /// 获取版本
    pub fn version(&self) -> u32 {
        self.version
    }
}

/// GC 对象引用
#[derive(Debug)]
pub struct GCObjectRef<'a> {
    /// 对象指针
    ptr: GCObjectPtr,
    /// 生命周期标记
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> GCObjectRef<'a> {
    /// 创建新的对象引用
    pub fn new(ptr: GCObjectPtr) -> Self {
        Self {
            ptr,
            _phantom: std::marker::PhantomData,
        }
    }

    /// 获取对象指针
    pub fn ptr(&self) -> GCObjectPtr {
        self.ptr
    }

    /// 读取对象数据
    pub fn read<T>(&self, offset: usize) -> GCResult<T> {
        if self.ptr.is_null() {
            return Err(GCError::InvalidPointer(self.ptr.addr()));
        }

        unsafe {
            let base_ptr = self.ptr.addr() as *const u8;
            let data_ptr = base_ptr.add(offset + std::mem::size_of::<ObjectHeader>()) as *const T;
            Ok(data_ptr.read_unaligned())
        }
    }

    /// 写入对象数据
    pub fn write<T>(&mut self, offset: usize, value: T) -> GCResult<()> {
        if self.ptr.is_null() {
            return Err(GCError::InvalidPointer(self.ptr.addr()));
        }

        unsafe {
            let base_ptr = self.ptr.addr() as *mut u8;
            let data_ptr = base_ptr.add(offset + std::mem::size_of::<ObjectHeader>()) as *mut T;
            data_ptr.write_unaligned(value);
        }
        Ok(())
    }
}

/// GC 对象
#[derive(Clone)]
pub struct GCObject {
    /// 对象头
    header: ObjectHeader,
    /// 对象数据
    data: Vec<u8>,
}

impl GCObject {
    /// 创建新的 GC 对象
    pub fn new(obj_type: ObjectType, size: usize) -> Self {
        let header = ObjectHeader::new(obj_type, size);
        let data = vec![0u8; size];

        Self { header, data }
    }

    /// 获取对象头
    pub fn header(&self) -> &ObjectHeader {
        &self.header
    }

    /// 获取可变对象头
    pub fn header_mut(&mut self) -> &mut ObjectHeader {
        &mut self.header
    }

    /// 获取对象数据
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// 获取可变对象数据
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// 获取对象指针
    pub fn as_ptr(&self) -> GCObjectPtr {
        GCObjectPtr::new(self as *const Self as u64, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_header() {
        let header = ObjectHeader::new(ObjectType::Array, 1024);
        assert_eq!(header.get_type(), ObjectType::Array);
        assert_eq!(header.get_size(), 1024);
        assert!(!header.is_marked());
        assert_eq!(header.get_age(), 0);
    }

    #[test]
    fn test_mark_bit() {
        let header = ObjectHeader::new(ObjectType::Struct, 512);
        assert!(!header.is_marked());
        header.set_mark(true);
        assert!(header.is_marked());
        header.set_mark(false);
        assert!(!header.is_marked());
    }

    #[test]
    fn test_age() {
        let header = ObjectHeader::new(ObjectType::String, 256);
        assert_eq!(header.get_age(), 0);
        header.increment_age();
        assert_eq!(header.get_age(), 1);
        header.increment_age();
        assert_eq!(header.get_age(), 2);
    }

    #[test]
    fn test_gc_object_ptr() {
        let ptr = GCObjectPtr::new(0x1000, 1);
        assert!(!ptr.is_null());
        assert_eq!(ptr.addr(), 0x1000);
        assert_eq!(ptr.version(), 1);

        let null_ptr = GCObjectPtr::null();
        assert!(null_ptr.is_null());
    }

    #[test]
    fn test_gc_object() {
        let obj = GCObject::new(ObjectType::Array, 1024);
        assert_eq!(obj.header().get_type(), ObjectType::Array);
        assert_eq!(obj.header().get_size(), 1024);
        assert_eq!(obj.data().len(), 1024);
    }
}
