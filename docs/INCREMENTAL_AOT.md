# 增量AOT编译实现文档

## 概述

增量AOT编译允许只重新编译变更的代码块，而不是完全重建整个AOT镜像，从而显著减少编译时间。

## 实现原理

### 变更检测

1. **哈希计算**：为每个代码块计算哈希值（基于IR块内容）
2. **变更比较**：将新代码块的哈希值与现有镜像中的哈希值比较
3. **变更分类**：
   - `Added`: 新增的代码块
   - `Modified`: 修改的代码块（哈希值不同）
   - `Removed`: 删除的代码块（存在于旧镜像但不在新代码中）
   - `Unchanged`: 未变更的代码块（哈希值相同）

### 增量编译流程

1. **加载现有镜像**：从文件加载现有的AOT镜像
2. **计算哈希值**：为现有镜像中的所有代码块计算哈希值
3. **检测变更**：比较新旧代码块，识别变更
4. **选择性编译**：
   - 新增/修改的代码块：重新编译
   - 未变更的代码块：从现有镜像复制（如果启用`preserve_unchanged`）
   - 删除的代码块：从镜像中移除
5. **合并镜像**：将新编译的代码块合并到现有镜像

## 使用方法

### 基本使用

```rust
use aot_builder::{IncrementalAotBuilder, IncrementalConfig};
use vm_ir::IRBlock;

// 创建增量编译配置
let config = IncrementalConfig {
    enabled: true,
    detect_changes: true,
    preserve_unchanged: true,
};

// 创建增量AOT构建器
let mut incremental = IncrementalAotBuilder::new(config);

// 加载现有镜像（如果存在）
if std::path::Path::new("existing.aot").exists() {
    incremental.load_existing_image("existing.aot")?;
}

// 准备要编译的代码块
let blocks = vec![
    (0x1000, ir_block_1),
    (0x2000, ir_block_2),
    // ...
];

// 增量编译
incremental.incremental_compile(&blocks)?;

// 保存更新后的镜像
incremental.build_and_save("updated.aot")?;

// 或者更新现有镜像
incremental.update_existing_image("existing.aot")?;
```

### 变更检测

```rust
// 检测变更
let changes = incremental.detect_changes(&blocks);

for change in &changes {
    match change.change_type {
        BlockChangeType::Added => {
            println!("New block at {:#x}", change.pc);
        }
        BlockChangeType::Modified => {
            println!("Modified block at {:#x}", change.pc);
        }
        BlockChangeType::Removed => {
            println!("Removed block at {:#x}", change.pc);
        }
        BlockChangeType::Unchanged => {
            println!("Unchanged block at {:#x}", change.pc);
        }
    }
}
```

### 获取变更统计

```rust
let stats = incremental.change_stats();
println!("Added: {}, Modified: {}, Removed: {}, Unchanged: {}",
    stats.added,
    stats.modified,
    stats.removed,
    stats.unchanged);

if stats.has_changes() {
    println!("Total changes: {}", stats.total_changes());
}
```

## 配置选项

### IncrementalConfig

```rust
pub struct IncrementalConfig {
    /// 是否启用增量编译
    pub enabled: bool,
    /// 是否检测代码块变更（通过哈希）
    pub detect_changes: bool,
    /// 是否保留未变更的代码块
    pub preserve_unchanged: bool,
}
```

### 配置示例

```rust
// 完全启用增量编译
let config = IncrementalConfig {
    enabled: true,
    detect_changes: true,
    preserve_unchanged: true,
};

// 禁用变更检测（所有块都重新编译）
let config = IncrementalConfig {
    enabled: true,
    detect_changes: false,
    preserve_unchanged: false,
};

// 禁用增量编译（完全重建）
let config = IncrementalConfig {
    enabled: false,
    detect_changes: false,
    preserve_unchanged: false,
};
```

## 技术细节

### 哈希算法

使用`DefaultHasher`计算代码块哈希值：

1. **IR块哈希**：基于`start_pc`、操作数量和操作类型
2. **代码哈希**：基于编译后的机器码字节

### 变更检测精度

- **精确检测**：通过比较哈希值检测变更
- **误报率**：极低（哈希冲突概率极低）
- **性能**：哈希计算开销很小

### 镜像更新策略

1. **完全重建**：构建新镜像，包含所有代码块
2. **增量更新**：修改现有镜像，只更新变更的代码块

## 性能影响

### 编译时间节省

- **无变更**：几乎不花费编译时间（只复制现有代码块）
- **少量变更**：只编译变更的代码块，显著减少编译时间
- **大量变更**：接近完全重建的时间

### 预期效果

假设有1000个代码块，其中10个变更：
- **完全重建**：编译1000个代码块
- **增量编译**：编译10个代码块 + 复制990个代码块
- **时间节省**：约99%（假设编译时间与代码块数量成正比）

## 限制和注意事项

1. **依赖关系**：如果代码块A依赖代码块B，B变更时A也需要重新编译
2. **重定位**：代码块地址变更可能需要更新重定位表
3. **符号表**：新增/删除代码块需要更新符号表
4. **镜像完整性**：更新后需要验证镜像完整性

## 未来改进

1. **依赖分析**：自动检测并重新编译依赖的代码块
2. **并行编译**：并行编译多个变更的代码块
3. **增量验证**：增量验证镜像完整性
4. **压缩优化**：增量更新时优化镜像布局

## 相关模块

- `aot-builder/src/lib.rs`: AOT构建器主模块
- `vm-engine-jit/src/aot_format.rs`: AOT镜像格式定义
- `vm-engine-jit/src/aot_loader.rs`: AOT镜像加载器


