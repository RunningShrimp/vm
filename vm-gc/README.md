# vm-gc

VM垃圾回收Crate - 解决vm-core和vm-optimizers之间的循环依赖

## 目的

这个crate被创建用来打破以下循环依赖：

```
vm-core/src/runtime/gc.rs
  ↓ depends on
vm-optimizers/src/gc*.rs
  ↓ depends on
vm-core (循环)
```

## 架构

新的依赖结构：

```
    vm-core
       ↓
    vm-gc (本crate)
       ↑
vm-optimizers
```

## 功能

- **GC策略接口**: 定义可插拔的GC算法接口
- **GC策略**: 分代GC、增量GC、自适应GC
- **GC统计**: 性能监控和分析
- **错误处理**: 统一的错误类型

## 使用示例

```rust
use vm_gc::{GcManager, GcConfig, traits::GenerationalPolicy};

// 创建GC配置
let config = GcConfig::default();

// 创建GC管理器（需要实现GcStrategy trait）
let mut gc = GcManager::new(config, strategy);

// 触发垃圾回收
gc.collect()?;

// 检查是否需要触发GC
if gc.should_collect() {
    gc.collect()?;
}
```

## 特性

- `generational`: 启用分代垃圾回收
- `incremental`: 启用增量垃圾回收
- `adaptive`: 启用自适应GC（结合分代和增量）
- `stats`: 启用统计信息收集
- `benchmarking`: 启用基准测试支持

## 迁移计划

1. ✅ 创建vm-gc crate基础结构
2. ⏳ 从vm-optimizers迁移GC实现
3. ⏳ 从vm-core迁移GC trait定义
4. ⏳ 更新vm-core依赖
5. ⏳ 更新vm-optimizers依赖
6. ⏳ 删除循环依赖代码
7. ⏳ 验证编译和功能

## 相关文件

- vm-core/src/runtime/gc.rs (待迁移/删除)
- vm-optimizers/src/gc.rs (待迁移)
- vm-optimizers/src/gc_adaptive.rs (待迁移)
- vm-optimizers/src/gc_generational_enhanced.rs (待迁移)
- vm-optimizers/src/gc_incremental_enhanced.rs (待迁移)
