//! AOT 加载器：负责加载和重定位 AOT 镜像
//!
//! 支持 mmap 加载、符号解析和运行时重定位。

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::aot_format::{
    AotImage, AotMetadata, CodeBlockEntry, DependencyEntry, DependencyType, RelationType,
    RelocationEntry, SymbolEntry,
};
use std::collections::HashSet;
use vm_core::GuestAddr;

/// AOT 加载的代码块信息
#[derive(Debug, Clone)]
pub struct AotCodeBlock {
    /// Guest PC (起始地址)
    pub guest_pc: GuestAddr,
    /// 编译代码的宿主地址
    pub host_addr: *const u8,
    /// 代码大小
    pub size: usize,
    /// 块的标志 (热度等)
    pub flags: u32,
}

/// AOT 加载器
pub struct AotLoader {
    /// 已加载的 AOT 镜像
    image: Arc<AotImage>,
    /// 加载基址 (代码段在内存中的起始地址)
    base_addr: u64,
    /// 代码段缓冲（可写，用于重定位）
    code_buffer: Arc<RwLock<Vec<u8>>>,
    /// 数据段缓冲
    data_buffer: Arc<Vec<u8>>,
    /// 已加载的代码块映射 (Guest PC -> AotCodeBlock)
    code_blocks: Arc<RwLock<HashMap<GuestAddr, AotCodeBlock>>>,
    /// 符号表 (名称 -> 宿主地址)
    symbol_table: Arc<RwLock<HashMap<String, u64>>>,
    /// 代码块依赖关系图 (源PC -> 目标PC列表)
    dependency_graph: Arc<RwLock<HashMap<GuestAddr, Vec<GuestAddr>>>>,
    /// 已解析的代码块集合（用于依赖关系加载）
    resolved_blocks: Arc<RwLock<HashSet<GuestAddr>>>,
    /// 元数据
    metadata: Option<AotMetadata>,
}

impl AotLoader {
    /// 从文件加载 AOT 镜像
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let image = AotImage::deserialize(&mut file)?;
        Self::new(image)
    }

    /// 从已加载的 AOT 镜像创建加载器
    pub fn new(image: AotImage) -> io::Result<Self> {
        let code_buffer = Arc::new(RwLock::new(image.code_section.clone()));
        let data_buffer = Arc::new(image.data_section.clone());

        // 使用代码缓冲的地址作为基地址
        let base_addr = {
            let buf = code_buffer.read();
            buf.as_ptr() as u64
        };

        let metadata = image.metadata.clone();
        let image = Arc::new(image);
        let loader = Self {
            image: image.clone(),
            base_addr,
            code_buffer,
            data_buffer,
            code_blocks: Arc::new(RwLock::new(HashMap::new())),
            symbol_table: Arc::new(RwLock::new(HashMap::new())),
            dependency_graph: Arc::new(RwLock::new(HashMap::new())),
            resolved_blocks: Arc::new(RwLock::new(HashSet::new())),
            metadata,
        };

        // 构建依赖关系图
        loader.build_dependency_graph()?;

        // 初始化符号表
        loader.build_symbol_table()?;

        // 执行重定位
        loader.perform_relocations()?;

        // 加载所有代码块（按依赖顺序）
        loader.load_code_blocks_with_dependencies()?;

        Ok(loader)
    }

    /// 构建代码块依赖关系图
    fn build_dependency_graph(&self) -> io::Result<()> {
        let mut graph = self.dependency_graph.write();

        // 从镜像的依赖关系表构建图
        for dep in &self.image.dependencies {
            graph.insert(dep.source_pc, dep.target_pcs.clone());
        }

        // 也从代码块条目的依赖关系构建
        for code_block in &self.image.code_blocks {
            if !code_block.dependencies.is_empty() {
                graph.insert(code_block.guest_pc, code_block.dependencies.clone());
            }
        }

        Ok(())
    }

    /// 构建符号表
    fn build_symbol_table(&self) -> io::Result<()> {
        let mut symbols = self.symbol_table.write();

        for symbol in &self.image.symbols {
            let addr = match symbol.symbol_type {
                crate::aot_format::SymbolType::Function
                | crate::aot_format::SymbolType::BlockLabel => {
                    // 查找对应的代码块，获取实际地址
                    if let Some(block) = self
                        .image
                        .code_blocks
                        .iter()
                        .find(|b| b.guest_pc == symbol.value)
                    {
                        self.base_addr + block.code_offset as u64
                    } else {
                        self.base_addr + symbol.value
                    }
                }
                crate::aot_format::SymbolType::Data => {
                    // 数据段地址 = 基址 + 代码段大小 + 数据偏移
                    self.base_addr + self.image.code_section.len() as u64 + symbol.value
                }
            };

            symbols.insert(symbol.name.clone(), addr);
        }

        // 为代码块创建符号（如果还没有）
        for code_block in &self.image.code_blocks {
            let symbol_name = format!("block_{:x}", code_block.guest_pc);
            if !symbols.contains_key(&symbol_name) {
                let addr = self.base_addr + code_block.code_offset as u64;
                symbols.insert(symbol_name, addr);
            }
        }

        Ok(())
    }

    /// 执行重定位
    fn perform_relocations(&self) -> io::Result<()> {
        let symbols = self.symbol_table.read();
        let mut code_buffer = self.code_buffer.write();
        let code_ptr = code_buffer.as_ptr() as u64;

        for reloc in &self.image.relocations {
            let target_addr = match reloc.reloc_type {
                RelationType::Abs64 => {
                    // 绝对地址重定位
                    if let Some(ref symbol_name) = reloc.symbol_name {
                        // 符号引用重定位
                        symbols.get(symbol_name).copied().unwrap_or(0) + reloc.addend as u64
                    } else {
                        // 直接地址重定位
                        reloc.target + reloc.addend as u64
                    }
                }
                RelationType::Rel32 => {
                    // PC 相对重定位
                    let reloc_addr = code_ptr + reloc.offset as u64;
                    let target = if let Some(ref symbol_name) = reloc.symbol_name {
                        symbols.get(symbol_name).copied().unwrap_or(0)
                    } else {
                        reloc.target
                    };
                    let offset = (target as i64 + reloc.addend) - reloc_addr as i64;
                    offset as u64
                }
                RelationType::BlockJump => {
                    // 块间直接跳转 - 查找目标块
                    let target_pc = reloc.target;
                    if let Some(&addr) = symbols.get(&format!("block_{:x}", target_pc)) {
                        addr
                    } else {
                        // 如果找不到符号，尝试直接使用目标PC
                        target_pc
                    }
                }
                RelationType::SymbolRef => {
                    // 符号引用重定位
                    if let Some(ref symbol_name) = reloc.symbol_name {
                        symbols.get(symbol_name).copied().unwrap_or(0) + reloc.addend as u64
                    } else {
                        // 回退到使用target字段
                        reloc.target + reloc.addend as u64
                    }
                }
                RelationType::DataRef => {
                    // 数据段引用
                    let data_base = self.base_addr + self.image.code_section.len() as u64;
                    data_base + reloc.target + reloc.addend as u64
                }
                RelationType::GotRef => {
                    // GOT引用（简化实现，直接使用符号地址）
                    if let Some(ref symbol_name) = reloc.symbol_name {
                        symbols.get(symbol_name).copied().unwrap_or(0)
                    } else {
                        reloc.target
                    }
                }
                RelationType::PltRef => {
                    // PLT引用（简化实现，直接使用符号地址）
                    if let Some(ref symbol_name) = reloc.symbol_name {
                        symbols.get(symbol_name).copied().unwrap_or(0)
                    } else {
                        reloc.target
                    }
                }
                RelationType::ExtCall => {
                    // 外部函数调用 - 这里保持为占位符
                    // 实际应用中应该链接到宿主库或使用回调机制
                    0
                }
            };

            // 应用重定位
            let reloc_offset = reloc.offset as usize;
            if reloc_offset + 8 <= code_buffer.len() {
                unsafe {
                    let ptr = code_buffer.as_mut_ptr().add(reloc_offset) as *mut u64;
                    match reloc.reloc_type {
                        RelationType::Abs64
                        | RelationType::BlockJump
                        | RelationType::SymbolRef
                        | RelationType::DataRef
                        | RelationType::GotRef
                        | RelationType::PltRef => {
                            // 64位绝对地址
                            *ptr = target_addr;
                        }
                        RelationType::Rel32 => {
                            // 32位相对偏移
                            let offset = target_addr as i32;
                            let ptr32 = ptr as *mut i32;
                            *ptr32 = offset;
                        }
                        RelationType::ExtCall => {
                            // 外部调用占位符
                            *ptr = target_addr;
                        }
                    }
                }
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Relocation offset {} out of bounds", reloc_offset),
                ));
            }
        }

        Ok(())
    }

    /// 加载所有代码块（按依赖顺序）
    fn load_code_blocks_with_dependencies(&self) -> io::Result<()> {
        let mut blocks = self.code_blocks.write();
        let mut resolved = self.resolved_blocks.write();
        let dependency_graph = self.dependency_graph.read();

        // 拓扑排序：先加载没有依赖的块，然后加载依赖已解析的块
        let mut to_load: Vec<GuestAddr> =
            self.image.code_blocks.iter().map(|b| b.guest_pc).collect();

        while !to_load.is_empty() {
            let mut progress = false;
            let mut loaded_this_round = Vec::new();

            for &pc in &to_load {
                // 检查依赖是否都已解析
                let deps_resolved = if let Some(deps) = dependency_graph.get(&pc) {
                    deps.iter().all(|dep_pc| resolved.contains(dep_pc))
                } else {
                    true // 没有依赖，可以直接加载
                };

                if deps_resolved {
                    // 加载这个代码块
                    if let Some(code_block) =
                        self.image.code_blocks.iter().find(|b| b.guest_pc == pc)
                    {
                        let code_buffer_guard = self.code_buffer.read();
                        let host_addr = unsafe {
                            code_buffer_guard
                                .as_ptr()
                                .add(code_block.code_offset as usize)
                        };
                        drop(code_buffer_guard); // 释放锁

                        let aot_block = AotCodeBlock {
                            guest_pc: code_block.guest_pc,
                            host_addr,
                            size: code_block.code_size as usize,
                            flags: code_block.flags,
                        };

                        blocks.insert(code_block.guest_pc, aot_block.clone());
                        resolved.insert(pc);
                        loaded_this_round.push(pc);
                        progress = true;
                    }
                }
            }

            // 移除已加载的块
            to_load.retain(|pc| !loaded_this_round.contains(pc));

            if !progress && !to_load.is_empty() {
                // 检测循环依赖或无法解析的依赖
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Circular dependency detected or unresolved dependencies for blocks: {:?}",
                        to_load
                    ),
                ));
            }
        }

        Ok(())
    }

    /// 加载单个代码块（用于延迟加载）
    pub fn load_code_block(&self, guest_pc: GuestAddr) -> io::Result<Option<AotCodeBlock>> {
        // 检查是否已加载
        {
            let blocks = self.code_blocks.read();
            if let Some(block) = blocks.get(&guest_pc) {
                return Ok(Some(block.clone()));
            }
        }

        // 查找代码块条目
        let code_block = self
            .image
            .code_blocks
            .iter()
            .find(|b| b.guest_pc == guest_pc)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Code block at {:#x} not found", guest_pc),
                )
            })?;

        // 检查依赖是否已解析
        let dependency_graph = self.dependency_graph.read();
        if let Some(deps) = dependency_graph.get(&guest_pc) {
            let resolved = self.resolved_blocks.read();
            for dep_pc in deps {
                if !resolved.contains(dep_pc) {
                    // 递归加载依赖
                    self.load_code_block(*dep_pc)?;
                }
            }
        }

        // 加载代码块
        let code_buffer_guard = self.code_buffer.read();
        let host_addr = unsafe {
            code_buffer_guard
                .as_ptr()
                .add(code_block.code_offset as usize)
        };
        drop(code_buffer_guard); // 释放锁

        let aot_block = AotCodeBlock {
            guest_pc: code_block.guest_pc,
            host_addr,
            size: code_block.code_size as usize,
            flags: code_block.flags,
        };

        {
            let mut blocks = self.code_blocks.write();
            blocks.insert(guest_pc, aot_block.clone());
        }

        {
            let mut resolved = self.resolved_blocks.write();
            resolved.insert(guest_pc);
        }

        Ok(Some(aot_block))
    }

    /// 查找 Guest PC 对应的 AOT 代码块
    pub fn lookup_block(&self, guest_pc: GuestAddr) -> Option<AotCodeBlock> {
        self.code_blocks.read().get(&guest_pc).cloned()
    }

    /// 查找符号地址
    pub fn lookup_symbol(&self, name: &str) -> Option<u64> {
        self.symbol_table.read().get(name).copied()
    }

    /// 获取代码块总数
    pub fn code_block_count(&self) -> usize {
        self.image.code_blocks.len()
    }

    /// 获取代码段大小
    pub fn code_size(&self) -> usize {
        self.image.code_section.len()
    }

    /// 获取数据段大小
    pub fn data_size(&self) -> usize {
        self.image.data_section.len()
    }

    /// 获取加载基址
    pub fn base_address(&self) -> u64 {
        self.base_addr
    }

    /// 迭代所有已加载的代码块
    pub fn iter_blocks(&self) -> Vec<AotCodeBlock> {
        self.code_blocks.read().values().cloned().collect()
    }

    /// 获取所有已加载的代码块计数
    pub fn loaded_block_count(&self) -> usize {
        self.code_blocks.read().len()
    }

    /// 获取代码块的依赖关系
    pub fn get_block_dependencies(&self, guest_pc: GuestAddr) -> Vec<GuestAddr> {
        self.dependency_graph
            .read()
            .get(&guest_pc)
            .cloned()
            .unwrap_or_default()
    }

    /// 检查代码块是否已解析
    pub fn is_block_resolved(&self, guest_pc: GuestAddr) -> bool {
        self.resolved_blocks.read().contains(&guest_pc)
    }

    /// 获取元数据
    pub fn metadata(&self) -> Option<&AotMetadata> {
        self.metadata.as_ref()
    }

    /// 解析符号（支持符号名称查找）
    pub fn resolve_symbol(&self, name: &str) -> Option<u64> {
        self.symbol_table.read().get(name).copied()
    }

    /// 获取所有符号名称
    pub fn list_symbols(&self) -> Vec<String> {
        self.symbol_table.read().keys().cloned().collect()
    }

    /// 验证代码块完整性（检查所有依赖是否已加载）
    pub fn validate_block_integrity(&self, guest_pc: GuestAddr) -> Result<(), String> {
        let dependency_graph = self.dependency_graph.read();
        let resolved = self.resolved_blocks.read();

        if let Some(deps) = dependency_graph.get(&guest_pc) {
            for dep_pc in deps {
                if !resolved.contains(dep_pc) {
                    return Err(format!("Dependency block {:#x} not resolved", dep_pc));
                }
            }
        }

        if !resolved.contains(&guest_pc) {
            return Err(format!("Block {:#x} not resolved", guest_pc));
        }

        Ok(())
    }

    /// 链接代码块（确保所有依赖已加载并重定位）
    pub fn link_code_block(&self, guest_pc: GuestAddr) -> io::Result<()> {
        // 加载代码块（如果尚未加载）
        self.load_code_block(guest_pc)?;

        // 验证完整性
        self.validate_block_integrity(guest_pc)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aot_format::{AotHeader, AotImage, CodeBlockEntry, SymbolType};

    #[test]
    fn test_aot_loader_creation() {
        let mut image = AotImage::new();
        image.add_code_block(0x1000, &[0x90, 0xC3], 1); // NOP, RET
        image.add_symbol("test_block".to_string(), 0, 2, SymbolType::BlockLabel);

        let loader = AotLoader::new(image).expect("Failed to create loader");

        assert_eq!(loader.code_block_count(), 1);
        assert_eq!(loader.code_size(), 2);
    }

    #[test]
    fn test_aot_lookup_block() {
        let mut image = AotImage::new();
        let pc = 0x2000u64;
        image.add_code_block(pc, &[0x48, 0x89, 0xC3], 1);

        let loader = AotLoader::new(image).expect("Failed to create loader");

        if let Some(block) = loader.lookup_block(pc) {
            assert_eq!(block.guest_pc, pc);
            assert_eq!(block.size, 3);
        } else {
            panic!("Block lookup failed");
        }
    }

    #[test]
    fn test_aot_symbol_lookup() {
        let mut image = AotImage::new();
        image.add_code_block(0x1000, &[0x90, 0xC3], 1);
        image.add_symbol("main".to_string(), 0, 2, SymbolType::Function);

        let loader = AotLoader::new(image).expect("Failed to create loader");

        let addr = loader.lookup_symbol("main");
        assert!(addr.is_some());
    }

    #[test]
    fn test_aot_multiple_blocks() {
        let mut image = AotImage::new();

        // 添加多个代码块
        for i in 0..10 {
            let pc = 0x1000 + i * 0x100;
            image.add_code_block(pc, &[0x90; 16], 1);
        }

        let loader = AotLoader::new(image).expect("Failed to create loader");

        assert_eq!(loader.code_block_count(), 10);
        assert_eq!(loader.loaded_block_count(), 10);

        // 验证所有块都能查找到
        for i in 0..10 {
            let pc = 0x1000 + i * 0x100;
            assert!(loader.lookup_block(pc).is_some());
        }
    }
}
