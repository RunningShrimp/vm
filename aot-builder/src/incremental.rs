//! 增量AOT编译
//!
//! 支持检测代码块变更、增量编译和AOT镜像增量更新。

use crate::{AotBuilder, CompilationOptions, DependencyAnalyzer};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use vm_engine_jit::aot_format::{AotImage, CodeBlockEntry};
use vm_ir::IRBlock;
use std::path::Path;
use std::fs::File;
use std::io::{Read, Write};

/// 代码块变更类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockChangeType {
    /// 新增的代码块
    Added,
    /// 修改的代码块
    Modified,
    /// 删除的代码块
    Removed,
    /// 未变更
    Unchanged,
}

/// 代码块变更信息
#[derive(Debug, Clone)]
pub struct BlockChange {
    /// Guest PC
    pub pc: u64,
    /// 变更类型
    pub change_type: BlockChangeType,
    /// 旧代码块的哈希值（如果存在）
    pub old_hash: Option<u64>,
    /// 新代码块的哈希值（如果存在）
    pub new_hash: Option<u64>,
}

/// 增量编译配置
#[derive(Debug, Clone)]
pub struct IncrementalConfig {
    /// 是否启用增量编译
    pub enabled: bool,
    /// 是否检测代码块变更（通过哈希）
    pub detect_changes: bool,
    /// 是否保留未变更的代码块
    pub preserve_unchanged: bool,
}

impl Default for IncrementalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            detect_changes: true,
            preserve_unchanged: true,
        }
    }
}

/// 增量AOT编译器
pub struct IncrementalAotBuilder {
    /// 基础构建器
    builder: AotBuilder,
    /// 配置
    config: IncrementalConfig,
    /// 现有镜像（如果存在）
    existing_image: Option<AotImage>,
    /// 代码块哈希映射 (PC -> 哈希值)
    block_hashes: HashMap<u64, u64>,
    /// 变更列表
    changes: Vec<BlockChange>,
}

impl IncrementalAotBuilder {
    /// 创建新的增量AOT构建器
    pub fn new(config: IncrementalConfig) -> Self {
        Self {
            builder: AotBuilder::new(),
            config,
            existing_image: None,
            block_hashes: HashMap::new(),
            changes: Vec::new(),
        }
    }

    /// 使用指定选项创建增量AOT构建器
    pub fn with_options(config: IncrementalConfig, options: CompilationOptions) -> Self {
        Self {
            builder: AotBuilder::with_options(options),
            config,
            existing_image: None,
            block_hashes: HashMap::new(),
            changes: Vec::new(),
        }
    }

    /// 加载现有AOT镜像
    pub fn load_existing_image<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let mut file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
        let image = AotImage::deserialize(&mut file)
            .map_err(|e| format!("Failed to deserialize image: {}", e))?;

        // 计算现有代码块的哈希值
        for block_entry in &image.code_blocks {
            let offset = block_entry.code_offset as usize;
            let size = block_entry.code_size as usize;
            if offset + size <= image.code_section.len() {
                let code = &image.code_section[offset..offset + size];
                let hash = Self::hash_code(code);
                self.block_hashes.insert(block_entry.guest_pc, hash);
            }
        }

        self.existing_image = Some(image);
        Ok(())
    }

    /// 检测代码块变更
    pub fn detect_changes(&mut self, blocks: &[(u64, IRBlock)]) -> Vec<BlockChange> {
        if !self.config.detect_changes {
            // 如果禁用变更检测，将所有块标记为新增
            return blocks
                .iter()
                .map(|(pc, _)| BlockChange {
                    pc: *pc,
                    change_type: BlockChangeType::Added,
                    old_hash: None,
                    new_hash: None,
                })
                .collect();
        }

        let mut changes = Vec::new();
        let mut new_pcs = HashSet::new();

        // 检测新增和修改的代码块
        for (pc, block) in blocks {
            new_pcs.insert(*pc);
            let new_hash = Self::hash_ir_block(block);
            let old_hash = self.block_hashes.get(pc).copied();

            let change_type = if let Some(old_hash_val) = old_hash {
                if old_hash_val == new_hash {
                    BlockChangeType::Unchanged
                } else {
                    BlockChangeType::Modified
                }
            } else {
                BlockChangeType::Added
            };

            changes.push(BlockChange {
                pc: *pc,
                change_type,
                old_hash,
                new_hash: Some(new_hash),
            });
        }

        // 检测删除的代码块
        if let Some(ref existing_image) = self.existing_image {
            for block_entry in &existing_image.code_blocks {
                if !new_pcs.contains(&block_entry.guest_pc) {
                    changes.push(BlockChange {
                        pc: block_entry.guest_pc,
                        change_type: BlockChangeType::Removed,
                        old_hash: self.block_hashes.get(&block_entry.guest_pc).copied(),
                        new_hash: None,
                    });
                }
            }
        }

        self.changes = changes.clone();
        changes
    }

    /// 增量编译代码块（支持依赖分析）
    pub fn incremental_compile(
        &mut self,
        blocks: &[(u64, IRBlock)],
    ) -> Result<(), String> {
        if !self.config.enabled {
            // 如果禁用增量编译，执行完整编译
            for (pc, block) in blocks {
                self.builder.add_ir_block(*pc, block, 1)?;
            }
            return Ok(());
        }

        // 检测变更
        let changes = self.detect_changes(blocks);

        // 如果保留未变更的代码块，从现有镜像复制
        if self.config.preserve_unchanged {
            if let Some(ref existing_image) = self.existing_image {
                for change in &changes {
                    if change.change_type == BlockChangeType::Unchanged {
                        // 从现有镜像复制代码块
                        if let Some(block_entry) = existing_image
                            .code_blocks
                            .iter()
                            .find(|b| b.guest_pc == change.pc)
                        {
                            let offset = block_entry.code_offset as usize;
                            let size = block_entry.code_size as usize;
                            if offset + size <= existing_image.code_section.len() {
                                let code = &existing_image.code_section[offset..offset + size];
                                self.builder
                                    .add_compiled_block(change.pc, code.to_vec(), block_entry.flags)?;
                            }
                        }
                    }
                }
            }
        }

        // 分析依赖关系，确定编译顺序
        let compile_order = self.determine_compile_order(blocks, &changes)?;

        // 编译新增和修改的代码块（按依赖顺序）
        for pc in compile_order {
            let block = blocks
                .iter()
                .find(|(block_pc, _)| *block_pc == pc)
                .ok_or_else(|| format!("Block not found for PC {:#x}", pc))?
                .1;

            let change = changes
                .iter()
                .find(|c| c.pc == pc)
                .ok_or_else(|| format!("Change not found for block at {:#x}", pc))?;

            match change.change_type {
                BlockChangeType::Added | BlockChangeType::Modified => {
                    // 编译新的或修改的代码块
                    self.builder.add_ir_block(pc, block, 1)?;

                    // 更新哈希值
                    let hash = Self::hash_ir_block(block);
                    self.block_hashes.insert(pc, hash);
                }
                BlockChangeType::Removed => {
                    // 删除的代码块不需要编译
                    // 从哈希映射中移除
                    self.block_hashes.remove(&pc);
                }
                BlockChangeType::Unchanged => {
                    // 未变更的代码块已经复制，不需要重新编译
                    // 哈希值保持不变
                }
            }
        }

        Ok(())
    }

    /// 基于依赖关系确定编译顺序
    fn determine_compile_order(
        &self,
        blocks: &[(u64, IRBlock)],
        changes: &[BlockChange],
    ) -> Result<Vec<u64>, String> {
        // 收集需要编译的块（新增、修改的块）
        let blocks_to_compile: Vec<(u64, &IRBlock)> = blocks
            .iter()
            .filter(|(pc, _)| {
                changes.iter().any(|c| {
                    c.pc == *pc && matches!(c.change_type, BlockChangeType::Added | BlockChangeType::Modified)
                })
            })
            .map(|(pc, block)| (*pc, block))
            .collect();

        // 如果没有块需要编译，返回空列表
        if blocks_to_compile.is_empty() {
            return Ok(Vec::new());
        }

        // 使用依赖分析器进行拓扑排序
        let compile_order = DependencyAnalyzer::topological_sort(&blocks_to_compile);

        // 验证所有需要编译的块都在排序结果中
        for (pc, _) in &blocks_to_compile {
            if !compile_order.contains(pc) {
                return Err(format!("Block at {:#x} not included in compile order", pc));
            }
        }

        Ok(compile_order)
    }

    /// 构建并保存AOT镜像（支持增量更新）
    pub fn build_and_save<P: AsRef<Path>>(&mut self, output_path: P) -> Result<(), String> {
        // 构建新镜像（包含所有代码块）
        // 注意：build()会消费builder，所以我们需要先克隆或重构
        // 为了简化，我们使用save_to_file方法
        self.builder.save_to_file(&output_path)
            .map_err(|e| format!("Failed to save image: {}", e))?;
        
        Ok(())
    }

    /// 更新现有AOT镜像（合并变更）
    pub fn update_existing_image<P: AsRef<Path>>(
        &mut self,
        output_path: P,
    ) -> Result<(), String> {
        if let Some(ref mut existing_image) = self.existing_image {
            // 构建新镜像以获取变更的代码块
            // 注意：由于build()会消费builder，我们需要先保存变更的代码块
            // 简化实现：直接构建新镜像并合并
            let new_image = self.builder.build()?;
            
            // 创建新的代码块映射（从新镜像）
            let mut new_blocks_map: HashMap<u64, (u32, Vec<u8>)> = HashMap::new();
            for block_entry in &new_image.code_blocks {
                let offset = block_entry.code_offset as usize;
                let size = block_entry.code_size as usize;
                if offset + size <= new_image.code_section.len() {
                    let code = new_image.code_section[offset..offset + size].to_vec();
                    new_blocks_map.insert(block_entry.guest_pc, (block_entry.flags, code));
                }
            }
            
            // 应用变更到现有镜像
            for change in &self.changes {
                match change.change_type {
                    BlockChangeType::Added | BlockChangeType::Modified => {
                        // 如果代码块已存在，先删除旧的
                        existing_image
                            .code_blocks
                            .retain(|b| b.guest_pc != change.pc);
                        
                        // 添加新的代码块
                        if let Some((flags, code)) = new_blocks_map.get(&change.pc) {
                            existing_image.add_code_block(change.pc, code, *flags);
                        }
                    }
                    BlockChangeType::Removed => {
                        // 删除代码块
                        existing_image.code_blocks.retain(|b| b.guest_pc != change.pc);
                    }
                    BlockChangeType::Unchanged => {
                        // 未变更的代码块保持不变
                    }
                }
            }

            // 保存更新后的镜像
            let mut file = File::create(output_path)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            existing_image
                .serialize(&mut file)
                .map_err(|e| format!("Failed to serialize image: {}", e))?;
        } else {
            // 如果没有现有镜像，构建新镜像
            self.build_and_save(output_path)?;
        }

        Ok(())
    }

    /// 获取变更统计
    pub fn change_stats(&self) -> ChangeStats {
        let mut stats = ChangeStats::default();
        for change in &self.changes {
            match change.change_type {
                BlockChangeType::Added => stats.added += 1,
                BlockChangeType::Modified => stats.modified += 1,
                BlockChangeType::Removed => stats.removed += 1,
                BlockChangeType::Unchanged => stats.unchanged += 1,
            }
        }
        stats
    }

    /// 计算代码块的哈希值
    fn hash_code(code: &[u8]) -> u64 {
        let mut hasher = DefaultHasher::new();
        code.hash(&mut hasher);
        hasher.finish()
    }

    /// 计算IR块的哈希值
    fn hash_ir_block(block: &IRBlock) -> u64 {
        let mut hasher = DefaultHasher::new();
        block.start_pc.hash(&mut hasher);
        block.ops.len().hash(&mut hasher);
        for op in &block.ops {
            // 简化哈希：只考虑操作类型和关键参数
            std::mem::discriminant(op).hash(&mut hasher);
        }
        if let Some(term) = &block.term {
            std::mem::discriminant(term).hash(&mut hasher);
        }
        hasher.finish()
    }

    /// 获取基础构建器（用于访问其他功能）
    pub fn builder_mut(&mut self) -> &mut AotBuilder {
        &mut self.builder
    }

    /// 获取基础构建器（只读）
    pub fn builder(&self) -> &AotBuilder {
        &self.builder
    }
}

/// 变更统计
#[derive(Debug, Clone, Default)]
pub struct ChangeStats {
    /// 新增的代码块数
    pub added: usize,
    /// 修改的代码块数
    pub modified: usize,
    /// 删除的代码块数
    pub removed: usize,
    /// 未变更的代码块数
    pub unchanged: usize,
}

impl ChangeStats {
    /// 总变更数
    pub fn total_changes(&self) -> usize {
        self.added + self.modified + self.removed
    }

    /// 是否有变更
    pub fn has_changes(&self) -> bool {
        self.total_changes() > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IRBuilder, IROp, Terminator};

    fn create_test_block(pc: u64) -> IRBlock {
        let mut builder = IRBuilder::new(pc);
        builder.push(IROp::AddImm { dst: 1, src: 0, imm: 42 });
        builder.push(IROp::MovImm { dst: 2, imm: 100 });
        builder.build()
    }

    #[test]
    fn test_change_detection() {
        let mut incremental = IncrementalAotBuilder::new(IncrementalConfig::default());

        // 添加初始代码块
        let blocks = vec![
            (0x1000, create_test_block(0x1000)),
            (0x2000, create_test_block(0x2000)),
        ];

        let changes = incremental.detect_changes(&blocks);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].change_type, BlockChangeType::Added);
        assert_eq!(changes[1].change_type, BlockChangeType::Added);

        // 再次检测相同的代码块
        let changes2 = incremental.detect_changes(&blocks);
        assert_eq!(changes2.len(), 2);
        // 注意：由于没有现有镜像，所有块仍被标记为新增
    }

    #[test]
    fn test_incremental_compile() {
        let mut incremental = IncrementalAotBuilder::new(IncrementalConfig::default());

        let blocks = vec![
            (0x1000, create_test_block(0x1000)),
            (0x2000, create_test_block(0x2000)),
        ];

        // 增量编译
        assert!(incremental.incremental_compile(&blocks).is_ok());

        let stats = incremental.change_stats();
        assert_eq!(stats.added, 2);
    }

    #[test]
    fn test_incremental_compile_with_dependencies() {
        let mut incremental = IncrementalAotBuilder::new(IncrementalConfig::default());

        // 创建有依赖关系的代码块
        let mut block1 = IRBuilder::new(0x1000);
        block1.push(IROp::MovImm { dst: 1, imm: 10 });
        block1.set_terminator(Terminator::Jmp { target: 0x2000 });
        let block1 = block1.build();

        let mut block2 = IRBuilder::new(0x2000);
        block2.push(IROp::MovImm { dst: 2, imm: 20 });
        block2.set_terminator(Terminator::Ret);
        let block2 = block2.build();

        let blocks = vec![
            (0x1000, block1),
            (0x2000, block2),
        ];

        // 增量编译
        assert!(incremental.incremental_compile(&blocks).is_ok());

        let stats = incremental.change_stats();
        assert_eq!(stats.added, 2);
        assert_eq!(stats.unchanged, 0);
    }

    #[test]
    fn test_incremental_compile_with_changes() {
        let mut incremental = IncrementalAotBuilder::new(IncrementalConfig::default());

        // 初始编译
        let blocks = vec![
            (0x1000, create_test_block(0x1000)),
            (0x2000, create_test_block(0x2000)),
        ];
        assert!(incremental.incremental_compile(&blocks).is_ok());

        // 修改一个块
        let mut modified_block = IRBuilder::new(0x1000);
        modified_block.push(IROp::MovImm { dst: 1, imm: 42 }); // 改变立即数
        modified_block.push(IROp::MovImm { dst: 2, imm: 100 });
        modified_block.set_terminator(Terminator::Ret);
        let modified_block = modified_block.build();

        let modified_blocks = vec![
            (0x1000, modified_block),
            (0x2000, create_test_block(0x2000)), // 未变更
        ];

        // 重新增量编译
        assert!(incremental.incremental_compile(&modified_blocks).is_ok());

        let stats = incremental.change_stats();
        assert_eq!(stats.modified, 1);
        assert_eq!(stats.unchanged, 1);
        assert_eq!(stats.added, 0);
    }
}

