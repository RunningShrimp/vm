//! GC 与执行引擎集成示例
//!
//! 本模块展示如何将垃圾回收器集成到虚拟机执行引擎中。

use vm_core::gc::{GCStrategy, GenerationalGC, GCConfig, GCObjectPtr};
use vm_core::{VmError, ExecutionError};
use std::sync::{Arc, RwLock};

/// GC 集成的执行引擎
///
/// 此结构体展示了如何在执行引擎中集成 GC，包括：
/// - 对象分配时的 GC 触发
/// - 写屏障的使用
/// - GC 统计信息的收集
pub struct GCIntegratedEngine {
    /// GC 实例
    gc: Arc<RwLock<Box<dyn GCStrategy>>>,
    /// 执行统计
    stats: EngineStats,
}

/// 执行引擎统计信息
#[derive(Debug, Default)]
pub struct EngineStats {
    /// 总指令数
    pub total_instructions: u64,
    /// GC 触发次数
    pub gc_collections: u64,
    /// 对象分配次数
    pub allocations: u64,
    /// 写屏障调用次数
    pub write_barriers: u64,
}

impl GCIntegratedEngine {
    /// 创建新的 GC 集成引擎
    pub fn new() -> Result<Self, VmError> {
        let config = GCConfig::default();
        let gc = GenerationalGC::new(1024 * 1024 * 1024, config); // 1GB 堆

        Ok(Self {
            gc: Arc::new(RwLock::new(Box::new(gc))),
            stats: EngineStats::default(),
        })
    }

    /// 分配对象（带 GC 触发）
    ///
    /// 当堆使用率超过阈值时自动触发 GC
    pub fn allocate_object(&mut self, size: usize, align: usize) -> Result<GCObjectPtr, VmError> {
        // 尝试分配
        let mut gc = self.gc.write().unwrap();
        let ptr = gc.allocate(size, align);

        match ptr {
            Ok(obj_ptr) => {
                // 更新统计
                self.stats.allocations += 1;

                // 检查是否需要触发 GC
                let heap_stats = gc.get_heap_stats();
                let usage_ratio = heap_stats.used_bytes as f64 / heap_stats.total_bytes as f64;

                if usage_ratio > 0.8 {
                    // 触发 GC
                    drop(gc); // 释放锁

                    self.collect_garbage(false)?;
                } else {
                    drop(gc);
                }

                Ok(obj_ptr)
            }
            Err(e) => {
                // 分配失败，尝试 GC 后重试
                drop(gc);
                self.collect_garbage(true)?;

                let mut gc = self.gc.write().unwrap();
                gc.allocate(size, align).map_err(|e| {
                    VmError::Execution(ExecutionError::Halted {
                        reason: format!("Allocation failed even after GC: {:?}", e),
                    })
                })
            }
        }
    }

    /// 更新对象引用（写屏障）
    ///
    /// 当对象字段被更新时调用写屏障，确保 GC 正确追踪对象图
    pub fn update_object_field(
        &mut self,
        obj: GCObjectPtr,
        field_offset: usize,
        new_value: GCObjectPtr,
    ) -> Result<(), VmError> {
        let mut gc = self.gc.write().unwrap();

        // 调用写屏障
        gc.write_barrier(obj, field_offset, new_value);

        self.stats.write_barriers += 1;
        Ok(())
    }

    /// 手动触发垃圾回收
    ///
    /// # 参数
    ///
    /// * `force_full` - 是否强制执行完整 GC（true）或仅 Minor GC（false）
    pub fn collect_garbage(&mut self, force_full: bool) -> Result<(), VmError> {
        let mut gc = self.gc.write().unwrap();

        let stats = gc.collect(force_full).map_err(|e| {
            VmError::Execution(ExecutionError::Halted {
                reason: format!("GC failed: {:?}", e),
            })
        })?;

        self.stats.gc_collections += 1;

        // 记录 GC 统计
        log::info!("GC completed: collected {} objects in {:.2}ms",
            stats.objects_collected,
            stats.collection_time_ns as f64 / 1_000_000.0
        );

        Ok(())
    }

    /// 获取引擎统计信息
    pub fn get_stats(&self) -> &EngineStats {
        &self.stats
    }

    /// 获取 GC 统计信息
    pub fn get_gc_stats(&self) -> vm_core::gc::GCStats {
        let gc = self.gc.read().unwrap();
        gc.get_gc_stats()
    }

    /// 获取堆统计信息
    pub fn get_heap_stats(&self) -> vm_core::gc::GCHeapStats {
        let gc = self.gc.read().unwrap();
        gc.get_heap_stats()
    }
}

impl Default for GCIntegratedEngine {
    fn default() -> Self {
        Self::new().expect("Failed to create GC integrated engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_engine_creation() {
        let engine = GCIntegratedEngine::new();
        assert!(engine.is_ok());
    }

    #[test]
    fn test_allocate_and_collect() {
        let mut engine = GCIntegratedEngine::new().unwrap();

        // 分配一些对象
        for _ in 0..10 {
            let _ = engine.allocate_object(1024, 8);
        }

        // 手动触发 GC
        let result = engine.collect_garbage(false);
        assert!(result.is_ok());

        // 检查 GC 统计
        let stats = engine.get_stats();
        assert!(stats.gc_collections > 0);
    }
}

/// 使用示例
///
/// ```rust,no_run,ignore
/// use vm_engine::gc_integration::GCIntegratedEngine;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // 创建带 GC 的引擎
///     let mut engine = GCIntegratedEngine::new()?;
///
///     // 分配对象
///     let obj1 = engine.allocate_object(1024, 8)?;
///     let obj2 = engine.allocate_object(2048, 16)?;
///
///     // 更新对象引用
///     engine.update_object_field(obj1, 0, obj2)?;
///
///     // 执行一些指令...
///
///     // 触发 GC
///     engine.collect_garbage(false)?;
///
///     // 查看统计
///     let stats = engine.get_stats();
///     println!("Allocations: {}", stats.allocations);
///     println!("GC collections: {}", stats.gc_collections);
///
///     Ok(())
/// }
/// ```
