//! AOT 镜像二进制格式定义
//!
//! 定义 AOT (Ahead-Of-Time) 编译产物的磁盘和内存布局。

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

/// AOT 镜像魔数，用于标识有效的 AOT 文件
pub const AOT_MAGIC: u32 = 0x414F5401; // "AOT\x01"

/// AOT 镜像版本
pub const AOT_VERSION: u32 = 1;

/// AOT 镜像头
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AotHeader {
    /// 魔数 (0x414F5401)
    pub magic: u32,
    /// 版本号
    pub version: u32,
    /// 文件总大小
    pub file_size: u32,
    /// 代码段大小
    pub code_section_size: u32,
    /// 数据段大小
    pub data_section_size: u32,
    /// 重定位表大小
    pub reloc_table_size: u32,
    /// 符号表大小
    pub symbol_table_size: u32,
    /// 编译时间戳
    pub timestamp: u64,
    /// 目标架构 (x86_64=1, ARM64=2, RISCV64=3)
    pub target_arch: u32,
    /// 元数据段大小
    pub metadata_section_size: u32,
    /// 依赖关系表大小
    pub dependency_table_size: u32,
    /// 保留字段
    pub reserved: [u32; 4],
}

impl AotHeader {
    /// 创建新的 AOT 头
    pub fn new(
        code_size: u32,
        data_size: u32,
        reloc_size: u32,
        symbol_size: u32,
        arch: u32,
    ) -> Self {
        Self::new_with_metadata(code_size, data_size, reloc_size, symbol_size, 0, 0, arch)
    }

    /// 创建带元数据的 AOT 头
    pub fn new_with_metadata(
        code_size: u32,
        data_size: u32,
        reloc_size: u32,
        symbol_size: u32,
        metadata_size: u32,
        dependency_size: u32,
        arch: u32,
    ) -> Self {
        let file_size = std::mem::size_of::<AotHeader>() as u32
            + code_size
            + data_size
            + reloc_size
            + symbol_size
            + metadata_size
            + dependency_size;

        Self {
            magic: AOT_MAGIC,
            version: AOT_VERSION,
            file_size,
            code_section_size: code_size,
            data_section_size: data_size,
            reloc_table_size: reloc_size,
            symbol_table_size: symbol_size,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            target_arch: arch,
            metadata_section_size: metadata_size,
            dependency_table_size: dependency_size,
            reserved: [0; 4],
        }
    }

    /// 验证头的有效性
    pub fn validate(&self) -> io::Result<()> {
        if self.magic != AOT_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid AOT magic number",
            ));
        }

        if self.version != AOT_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported AOT version: {}", self.version),
            ));
        }

        Ok(())
    }

    /// 序列化头部
    pub fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.magic.to_le_bytes())?;
        writer.write_all(&self.version.to_le_bytes())?;
        writer.write_all(&self.file_size.to_le_bytes())?;
        writer.write_all(&self.code_section_size.to_le_bytes())?;
        writer.write_all(&self.data_section_size.to_le_bytes())?;
        writer.write_all(&self.reloc_table_size.to_le_bytes())?;
        writer.write_all(&self.symbol_table_size.to_le_bytes())?;
        writer.write_all(&self.timestamp.to_le_bytes())?;
        writer.write_all(&self.target_arch.to_le_bytes())?;
        writer.write_all(&self.metadata_section_size.to_le_bytes())?;
        writer.write_all(&self.dependency_table_size.to_le_bytes())?;
        for reserved in self.reserved.iter() {
            writer.write_all(&reserved.to_le_bytes())?;
        }
        Ok(())
    }

    /// 反序列化头部
    pub fn deserialize<R: Read>(reader: &mut R) -> io::Result<Self> {
        // 头部大小已增加，需要读取更多字节
        let mut buf = vec![0u8; 64]; // 新的头部大小
        reader.read_exact(&mut buf[..64])?;

        let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        let version = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        let file_size = u32::from_le_bytes(buf[8..12].try_into().unwrap());
        let code_section_size = u32::from_le_bytes(buf[12..16].try_into().unwrap());
        let data_section_size = u32::from_le_bytes(buf[16..20].try_into().unwrap());
        let reloc_table_size = u32::from_le_bytes(buf[20..24].try_into().unwrap());
        let symbol_table_size = u32::from_le_bytes(buf[24..28].try_into().unwrap());
        let timestamp = u64::from_le_bytes(buf[28..36].try_into().unwrap());
        let target_arch = u32::from_le_bytes(buf[36..40].try_into().unwrap());

        let metadata_section_size = u32::from_le_bytes(buf[40..44].try_into().unwrap());
        let dependency_table_size = u32::from_le_bytes(buf[44..48].try_into().unwrap());

        let mut reserved = [0u32; 4];
        for i in 0..4 {
            reserved[i] = u32::from_le_bytes(buf[48 + i * 4..48 + (i + 1) * 4].try_into().unwrap());
        }

        Ok(Self {
            magic,
            version,
            file_size,
            code_section_size,
            data_section_size,
            reloc_table_size,
            symbol_table_size,
            timestamp,
            target_arch,
            metadata_section_size,
            dependency_table_size,
            reserved,
        })
    }
}

/// 代码块条目：将 Guest PC 映射到编译的代码
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlockEntry {
    /// Guest 程序计数器 (起始地址)
    pub guest_pc: u64,
    /// 代码段中的偏移量
    pub code_offset: u32,
    /// 代码大小 (字节)
    pub code_size: u32,
    /// 块的标志 (热度, 优化级别等)
    pub flags: u32,
    /// 依赖的代码块PC列表（用于代码块依赖关系）
    pub dependencies: Vec<u64>,
    /// 优化级别
    pub optimization_level: u32,
    /// 编译时间戳（微秒）
    pub compile_timestamp_us: u64,
}

impl CodeBlockEntry {
    /// 创建新的代码块条目
    pub fn new(guest_pc: u64, code_offset: u32, code_size: u32, flags: u32) -> Self {
        Self {
            guest_pc,
            code_offset,
            code_size,
            flags,
            dependencies: Vec::new(),
            optimization_level: 0,
            compile_timestamp_us: 0,
        }
    }

    /// 创建带依赖关系的代码块条目
    pub fn with_dependencies(
        guest_pc: u64,
        code_offset: u32,
        code_size: u32,
        flags: u32,
        dependencies: Vec<u64>,
    ) -> Self {
        Self {
            guest_pc,
            code_offset,
            code_size,
            flags,
            dependencies,
            optimization_level: 0,
            compile_timestamp_us: 0,
        }
    }

    /// 添加依赖的代码块
    pub fn add_dependency(&mut self, dep_pc: u64) {
        if !self.dependencies.contains(&dep_pc) {
            self.dependencies.push(dep_pc);
        }
    }

    /// 序列化
    pub fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.guest_pc.to_le_bytes())?;
        writer.write_all(&self.code_offset.to_le_bytes())?;
        writer.write_all(&self.code_size.to_le_bytes())?;
        writer.write_all(&self.flags.to_le_bytes())?;
        writer.write_all(&self.optimization_level.to_le_bytes())?;
        writer.write_all(&self.compile_timestamp_us.to_le_bytes())?;

        // 序列化依赖关系
        writer.write_all(&(self.dependencies.len() as u32).to_le_bytes())?;
        for &dep in &self.dependencies {
            writer.write_all(&dep.to_le_bytes())?;
        }

        Ok(())
    }

    /// 反序列化
    pub fn deserialize<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0u8; 28];
        reader.read_exact(&mut buf)?;

        let guest_pc = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        let code_offset = u32::from_le_bytes(buf[8..12].try_into().unwrap());
        let code_size = u32::from_le_bytes(buf[12..16].try_into().unwrap());
        let flags = u32::from_le_bytes(buf[16..20].try_into().unwrap());
        let optimization_level = u32::from_le_bytes(buf[20..24].try_into().unwrap());
        let compile_timestamp_us = u64::from_le_bytes(buf[24..32].try_into().unwrap());

        // 读取依赖关系
        let mut dep_count_buf = [0u8; 4];
        reader.read_exact(&mut dep_count_buf)?;
        let dep_count = u32::from_le_bytes(dep_count_buf) as usize;

        let mut dependencies = Vec::with_capacity(dep_count);
        for _ in 0..dep_count {
            let mut dep_buf = [0u8; 8];
            reader.read_exact(&mut dep_buf)?;
            dependencies.push(u64::from_le_bytes(dep_buf));
        }

        Ok(Self {
            guest_pc,
            code_offset,
            code_size,
            flags,
            dependencies,
            optimization_level,
            compile_timestamp_us,
        })
    }
}

/// 重定位条目：指定需要修复的地址引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationEntry {
    /// 代码段中的偏移量 (需要修复的位置)
    pub offset: u32,
    /// 重定位类型
    pub reloc_type: RelationType,
    /// 目标符号索引或绝对地址
    pub target: u64,
    /// 符号名称（如果reloc_type为SymbolRef）
    pub symbol_name: Option<String>,
    /// 加数（用于重定位计算）
    pub addend: i64,
}

/// 重定位类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RelationType {
    /// 绝对 64 位地址
    Abs64 = 0,
    /// 相对 32 位偏移 (PC 相对)
    Rel32 = 1,
    /// 块间直接跳转
    BlockJump = 2,
    /// 外部函数调用
    ExtCall = 3,
    /// 符号引用（通过符号表解析）
    SymbolRef = 4,
    /// 数据段引用
    DataRef = 5,
    /// GOT (Global Offset Table) 引用
    GotRef = 6,
    /// PLT (Procedure Linkage Table) 引用
    PltRef = 7,
}

impl RelocationEntry {
    /// 创建新的重定位条目
    pub fn new(offset: u32, reloc_type: RelationType, target: u64) -> Self {
        Self {
            offset,
            reloc_type,
            target,
            symbol_name: None,
            addend: 0,
        }
    }

    /// 创建符号引用重定位条目
    pub fn new_symbol_ref(offset: u32, symbol_name: String, addend: i64) -> Self {
        Self {
            offset,
            reloc_type: RelationType::SymbolRef,
            target: 0,
            symbol_name: Some(symbol_name),
            addend,
        }
    }

    /// 创建块跳转重定位条目
    pub fn new_block_jump(offset: u32, target_block_pc: u64) -> Self {
        Self {
            offset,
            reloc_type: RelationType::BlockJump,
            target: target_block_pc,
            symbol_name: None,
            addend: 0,
        }
    }

    /// 序列化
    pub fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.offset.to_le_bytes())?;
        writer.write_all(&(self.reloc_type as u32).to_le_bytes())?;
        writer.write_all(&self.target.to_le_bytes())?;
        writer.write_all(&self.addend.to_le_bytes())?;

        // 序列化符号名称（如果存在）
        if let Some(ref name) = self.symbol_name {
            let name_bytes = name.as_bytes();
            writer.write_all(&(name_bytes.len() as u32).to_le_bytes())?;
            writer.write_all(name_bytes)?;
        } else {
            writer.write_all(&0u32.to_le_bytes())?;
        }

        Ok(())
    }

    /// 反序列化
    pub fn deserialize<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0u8; 24];
        reader.read_exact(&mut buf)?;

        let offset = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        let reloc_type_u32 = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        let reloc_type = match reloc_type_u32 {
            0 => RelationType::Abs64,
            1 => RelationType::Rel32,
            2 => RelationType::BlockJump,
            3 => RelationType::ExtCall,
            4 => RelationType::SymbolRef,
            5 => RelationType::DataRef,
            6 => RelationType::GotRef,
            7 => RelationType::PltRef,
            _ => RelationType::Abs64,
        };
        let target = u64::from_le_bytes(buf[8..16].try_into().unwrap());
        let addend = i64::from_le_bytes(buf[16..24].try_into().unwrap());

        // 读取符号名称（如果存在）
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf)?;
        let name_len = u32::from_le_bytes(len_buf) as usize;

        let symbol_name = if name_len > 0 {
            let mut name_bytes = vec![0u8; name_len];
            reader.read_exact(&mut name_bytes)?;
            Some(String::from_utf8(name_bytes).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 symbol name")
            })?)
        } else {
            None
        };

        Ok(Self {
            offset,
            reloc_type,
            target,
            symbol_name,
            addend,
        })
    }
}

/// 符号表条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolEntry {
    /// 符号名称
    pub name: String,
    /// 符号值 (地址或偏移)
    pub value: u64,
    /// 符号大小
    pub size: u32,
    /// 符号类型 (函数, 数据等)
    pub symbol_type: SymbolType,
}

/// 符号类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SymbolType {
    /// 函数
    Function = 0,
    /// 数据
    Data = 1,
    /// 块标签
    BlockLabel = 2,
}

impl SymbolEntry {
    /// 创建新的符号条目
    pub fn new(name: String, value: u64, size: u32, symbol_type: SymbolType) -> Self {
        Self {
            name,
            value,
            size,
            symbol_type,
        }
    }

    /// 序列化
    pub fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let name_bytes = self.name.as_bytes();
        writer.write_all(&(name_bytes.len() as u32).to_le_bytes())?;
        writer.write_all(name_bytes)?;
        writer.write_all(&self.value.to_le_bytes())?;
        writer.write_all(&self.size.to_le_bytes())?;
        writer.write_all(&(self.symbol_type as u32).to_le_bytes())?;
        Ok(())
    }

    /// 反序列化
    pub fn deserialize<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf)?;
        let len = u32::from_le_bytes(len_buf) as usize;

        let mut name_bytes = vec![0u8; len];
        reader.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 symbol name"))?;

        let mut buf = [0u8; 16];
        reader.read_exact(&mut buf)?;

        let value = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        let size = u32::from_le_bytes(buf[8..12].try_into().unwrap());
        let symbol_type_u32 = u32::from_le_bytes(buf[12..16].try_into().unwrap());
        let symbol_type = match symbol_type_u32 {
            0 => SymbolType::Function,
            1 => SymbolType::Data,
            2 => SymbolType::BlockLabel,
            _ => SymbolType::Function,
        };

        Ok(Self {
            name,
            value,
            size,
            symbol_type,
        })
    }
}

/// AOT 镜像元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AotMetadata {
    /// 编译选项（JSON格式）
    pub compilation_options: String,
    /// 编译器版本
    pub compiler_version: String,
    /// 优化级别
    pub optimization_level: u32,
    /// 依赖的库列表
    pub dependencies: Vec<String>,
    /// 自定义元数据（键值对）
    pub custom_metadata: std::collections::HashMap<String, String>,
}

impl Default for AotMetadata {
    fn default() -> Self {
        Self {
            compilation_options: String::new(),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
            optimization_level: 2,
            dependencies: Vec::new(),
            custom_metadata: std::collections::HashMap::new(),
        }
    }
}

/// 代码块依赖关系条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEntry {
    /// 源代码块PC
    pub source_pc: u64,
    /// 目标代码块PC列表
    pub target_pcs: Vec<u64>,
    /// 依赖类型（调用、跳转等）
    pub dependency_type: DependencyType,
}

/// 依赖类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DependencyType {
    /// 直接调用
    DirectCall = 0,
    /// 间接调用
    IndirectCall = 1,
    /// 直接跳转
    DirectJump = 2,
    /// 间接跳转
    IndirectJump = 3,
    /// 数据依赖
    DataDependency = 4,
}

/// 完整 AOT 镜像结构
#[derive(Debug, Clone)]
pub struct AotImage {
    /// 头部
    pub header: AotHeader,
    /// 代码块条目列表
    pub code_blocks: Vec<CodeBlockEntry>,
    /// 代码段 (编译后的机器码)
    pub code_section: Vec<u8>,
    /// 数据段 (常量和运行时数据)
    pub data_section: Vec<u8>,
    /// 重定位条目列表
    pub relocations: Vec<RelocationEntry>,
    /// 符号表
    pub symbols: Vec<SymbolEntry>,
    /// 元数据
    pub metadata: Option<AotMetadata>,
    /// 依赖关系表
    pub dependencies: Vec<DependencyEntry>,
}

impl AotImage {
    /// 创建新的 AOT 镜像
    pub fn new() -> Self {
        Self {
            header: AotHeader::new(0, 0, 0, 0, 1), // x86_64
            code_blocks: Vec::new(),
            code_section: Vec::new(),
            data_section: Vec::new(),
            relocations: Vec::new(),
            symbols: Vec::new(),
            metadata: None,
            dependencies: Vec::new(),
        }
    }

    /// 设置元数据
    pub fn set_metadata(&mut self, metadata: AotMetadata) {
        self.metadata = Some(metadata);
    }

    /// 添加依赖关系条目
    pub fn add_dependency(
        &mut self,
        source_pc: u64,
        target_pcs: Vec<u64>,
        dep_type: DependencyType,
    ) {
        self.dependencies.push(DependencyEntry {
            source_pc,
            target_pcs,
            dependency_type: dep_type,
        });
    }

    /// 添加代码块
    pub fn add_code_block(&mut self, guest_pc: u64, code: &[u8], flags: u32) {
        let code_offset = self.code_section.len() as u32;
        let code_size = code.len() as u32;

        self.code_blocks
            .push(CodeBlockEntry::new(guest_pc, code_offset, code_size, flags));
        self.code_section.extend_from_slice(code);
    }

    /// 添加重定位
    pub fn add_relocation(&mut self, offset: u32, reloc_type: RelationType, target: u64) {
        self.relocations
            .push(RelocationEntry::new(offset, reloc_type, target));
    }

    /// 添加符号引用重定位
    pub fn add_symbol_relocation(&mut self, offset: u32, symbol_name: String, addend: i64) {
        self.relocations
            .push(RelocationEntry::new_symbol_ref(offset, symbol_name, addend));
    }

    /// 添加符号
    pub fn add_symbol(&mut self, name: String, value: u64, size: u32, symbol_type: SymbolType) {
        self.symbols
            .push(SymbolEntry::new(name, value, size, symbol_type));
    }

    /// 序列化 AOT 镜像到文件
    pub fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        // 序列化元数据
        let metadata_bytes = if let Some(ref metadata) = self.metadata {
            serde_json::to_vec(metadata).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to serialize metadata: {}", e),
                )
            })?
        } else {
            Vec::new()
        };

        // 序列化依赖关系表
        let mut dependency_bytes = Vec::new();
        dependency_bytes.write_all(&(self.dependencies.len() as u32).to_le_bytes())?;
        for dep in &self.dependencies {
            dependency_bytes.write_all(&dep.source_pc.to_le_bytes())?;
            dependency_bytes.write_all(&(dep.target_pcs.len() as u32).to_le_bytes())?;
            for &target in &dep.target_pcs {
                dependency_bytes.write_all(&target.to_le_bytes())?;
            }
            dependency_bytes.write_all(&(dep.dependency_type as u32).to_le_bytes())?;
        }

        // 更新头部大小信息
        let mut header = self.header.clone();
        header.code_section_size = self.code_section.len() as u32;
        header.data_section_size = self.data_section.len() as u32;
        // 重定位表大小需要根据实际序列化大小计算
        let mut reloc_size = 0u32;
        for reloc in &self.relocations {
            reloc_size += 24; // 基础大小
            if let Some(ref name) = reloc.symbol_name {
                reloc_size += 4 + name.len() as u32;
            } else {
                reloc_size += 4;
            }
        }
        header.reloc_table_size = reloc_size;
        header.symbol_table_size = self
            .symbols
            .iter()
            .map(|s| 4 + s.name.len() + 16)
            .sum::<usize>() as u32;
        header.metadata_section_size = metadata_bytes.len() as u32;
        header.dependency_table_size = dependency_bytes.len() as u32;

        // 写入头部
        header.serialize(writer)?;

        // 写入代码块条目表
        writer.write_all(&(self.code_blocks.len() as u32).to_le_bytes())?;
        for block in &self.code_blocks {
            block.serialize(writer)?;
        }

        // 写入代码段
        writer.write_all(&self.code_section)?;

        // 写入数据段
        writer.write_all(&self.data_section)?;

        // 写入重定位表
        writer.write_all(&(self.relocations.len() as u32).to_le_bytes())?;
        for reloc in &self.relocations {
            reloc.serialize(writer)?;
        }

        // 写入符号表
        writer.write_all(&(self.symbols.len() as u32).to_le_bytes())?;
        for symbol in &self.symbols {
            symbol.serialize(writer)?;
        }

        // 写入元数据段
        writer.write_all(&metadata_bytes)?;

        // 写入依赖关系表
        writer.write_all(&dependency_bytes)?;

        Ok(())
    }

    /// 反序列化 AOT 镜像
    pub fn deserialize<R: Read>(reader: &mut R) -> io::Result<Self> {
        let header = AotHeader::deserialize(reader)?;
        header.validate()?;

        // 读取代码块条目表
        let mut block_count_buf = [0u8; 4];
        reader.read_exact(&mut block_count_buf)?;
        let block_count = u32::from_le_bytes(block_count_buf) as usize;

        let mut code_blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            code_blocks.push(CodeBlockEntry::deserialize(reader)?);
        }

        // 读取代码段
        let mut code_section = vec![0u8; header.code_section_size as usize];
        reader.read_exact(&mut code_section)?;

        // 读取数据段
        let mut data_section = vec![0u8; header.data_section_size as usize];
        reader.read_exact(&mut data_section)?;

        // 读取重定位表
        let mut reloc_count_buf = [0u8; 4];
        reader.read_exact(&mut reloc_count_buf)?;
        let reloc_count = u32::from_le_bytes(reloc_count_buf) as usize;

        let mut relocations = Vec::with_capacity(reloc_count);
        for _ in 0..reloc_count {
            relocations.push(RelocationEntry::deserialize(reader)?);
        }

        // 读取符号表
        let mut symbol_count_buf = [0u8; 4];
        reader.read_exact(&mut symbol_count_buf)?;
        let symbol_count = u32::from_le_bytes(symbol_count_buf) as usize;

        let mut symbols = Vec::with_capacity(symbol_count);
        for _ in 0..symbol_count {
            symbols.push(SymbolEntry::deserialize(reader)?);
        }

        // 读取元数据段
        let metadata = if header.metadata_section_size > 0 {
            let mut metadata_buf = vec![0u8; header.metadata_section_size as usize];
            reader.read_exact(&mut metadata_buf)?;
            Some(serde_json::from_slice(&metadata_buf).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to deserialize metadata: {}", e),
                )
            })?)
        } else {
            None
        };

        // 读取依赖关系表
        let mut dependencies = Vec::new();
        if header.dependency_table_size > 0 {
            let mut dep_count_buf = [0u8; 4];
            reader.read_exact(&mut dep_count_buf)?;
            let dep_count = u32::from_le_bytes(dep_count_buf) as usize;

            for _ in 0..dep_count {
                let mut source_buf = [0u8; 8];
                reader.read_exact(&mut source_buf)?;
                let source_pc = u64::from_le_bytes(source_buf);

                let mut target_count_buf = [0u8; 4];
                reader.read_exact(&mut target_count_buf)?;
                let target_count = u32::from_le_bytes(target_count_buf) as usize;

                let mut target_pcs = Vec::with_capacity(target_count);
                for _ in 0..target_count {
                    let mut target_buf = [0u8; 8];
                    reader.read_exact(&mut target_buf)?;
                    target_pcs.push(u64::from_le_bytes(target_buf));
                }

                let mut dep_type_buf = [0u8; 4];
                reader.read_exact(&mut dep_type_buf)?;
                let dep_type_u32 = u32::from_le_bytes(dep_type_buf);
                let dependency_type = match dep_type_u32 {
                    0 => DependencyType::DirectCall,
                    1 => DependencyType::IndirectCall,
                    2 => DependencyType::DirectJump,
                    3 => DependencyType::IndirectJump,
                    4 => DependencyType::DataDependency,
                    _ => DependencyType::DirectCall,
                };

                dependencies.push(DependencyEntry {
                    source_pc,
                    target_pcs,
                    dependency_type,
                });
            }
        }

        Ok(Self {
            header,
            code_blocks,
            code_section,
            data_section,
            relocations,
            symbols,
            metadata,
            dependencies,
        })
    }
}

impl Default for AotImage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aot_header_serialization() {
        let header = AotHeader::new(1024, 512, 256, 128, 1);
        let mut buf = Vec::new();
        header.serialize(&mut buf).unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let loaded = AotHeader::deserialize(&mut cursor).unwrap();

        assert_eq!(header.magic, loaded.magic);
        assert_eq!(header.version, loaded.version);
        assert_eq!(header.code_section_size, loaded.code_section_size);
    }

    #[test]
    fn test_aot_image_creation() {
        let mut image = AotImage::new();
        image.add_code_block(0x1000, &[0x90, 0xC3], 1); // NOP, RET
        image.add_symbol("main".to_string(), 0x1000, 2, SymbolType::Function);

        assert_eq!(image.code_blocks.len(), 1);
        assert_eq!(image.code_section.len(), 2);
        assert_eq!(image.symbols.len(), 1);
    }

    #[test]
    fn test_aot_image_roundtrip() {
        let mut image = AotImage::new();
        image.add_code_block(0x2000, &[0x48, 0x89, 0xC3], 1);
        image.add_relocation(10, RelationType::Abs64, 0x100000);
        image.add_symbol("block".to_string(), 0x2000, 3, SymbolType::BlockLabel);

        let mut buf = Vec::new();
        image.serialize(&mut buf).unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let loaded = AotImage::deserialize(&mut cursor).unwrap();

        assert_eq!(loaded.code_blocks.len(), image.code_blocks.len());
        assert_eq!(loaded.relocations.len(), image.relocations.len());
        assert_eq!(loaded.symbols.len(), image.symbols.len());
    }
}
