//! AOT Image Format
//!
//! 定义 AOT 镜像的文件格式和结构

use std::io::{self, Write};
use vm_core::GuestAddr;

/// AOT 镜像魔数
pub const AOT_MAGIC: u32 = 0x414F5458; // "AOTX"
/// AOT 版本
pub const AOT_VERSION: u32 = 1;

/// AOT 镜像头部
#[repr(C)]
#[derive(Clone, Debug)]
pub struct AotHeader {
    pub magic: u32,
    pub version: u32,
    pub section_count: u32,
    pub entry_point: u64,
    pub optimization_level: u32,
    pub target_isa: u32,
}

impl Default for AotHeader {
    fn default() -> Self {
        Self {
            magic: AOT_MAGIC,
            version: AOT_VERSION,
            section_count: 0,
            entry_point: 0,
            optimization_level: 2, // 默认优化级别
            target_isa: 0,         // 默认ISA
        }
    }
}

/// AOT 代码段
#[derive(Debug, Clone)]
pub struct AotSection {
    pub addr: GuestAddr,
    pub data: Vec<u8>,
    pub flags: u32,
}

/// AOT 镜像
#[derive(Debug, Clone, Default)]
pub struct AotImage {
    pub header: AotHeader,
    pub sections: Vec<AotSection>,
}

impl AotImage {
    /// 序列化镜像到 Writer
    pub fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        // Write header
        writer.write_all(&self.header.magic.to_le_bytes())?;
        writer.write_all(&self.header.version.to_le_bytes())?;
        writer.write_all(&(self.sections.len() as u32).to_le_bytes())?;
        writer.write_all(&self.header.entry_point.to_le_bytes())?;
        writer.write_all(&self.header.optimization_level.to_le_bytes())?;
        writer.write_all(&self.header.target_isa.to_le_bytes())?;

        // Write sections
        for section in &self.sections {
            writer.write_all(&section.addr.0.to_le_bytes())?;
            writer.write_all(&(section.data.len() as u32).to_le_bytes())?;
            writer.write_all(&section.flags.to_le_bytes())?;
            writer.write_all(&section.data)?;
        }

        Ok(())
    }

    /// 从 Reader 反序列化镜像
    pub fn deserialize<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let mut magic_bytes = [0u8; 4];
        reader.read_exact(&mut magic_bytes)?;
        let magic = u32::from_le_bytes(magic_bytes);

        if magic != AOT_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid AOT magic",
            ));
        }

        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);

        let mut count_bytes = [0u8; 4];
        reader.read_exact(&mut count_bytes)?;
        let section_count = u32::from_le_bytes(count_bytes);

        let mut entry_bytes = [0u8; 8];
        reader.read_exact(&mut entry_bytes)?;
        let entry_point = u64::from_le_bytes(entry_bytes);

        let mut opt_level_bytes = [0u8; 4];
        reader.read_exact(&mut opt_level_bytes)?;
        let optimization_level = u32::from_le_bytes(opt_level_bytes);

        let mut target_isa_bytes = [0u8; 4];
        reader.read_exact(&mut target_isa_bytes)?;
        let target_isa = u32::from_le_bytes(target_isa_bytes);

        let header = AotHeader {
            magic,
            version,
            section_count,
            entry_point,
            optimization_level,
            target_isa,
        };

        let mut sections = Vec::with_capacity(section_count as usize);
        for _ in 0..section_count {
            let mut addr_bytes = [0u8; 8];
            reader.read_exact(&mut addr_bytes)?;
            let addr = u64::from_le_bytes(addr_bytes);

            let mut len_bytes = [0u8; 4];
            reader.read_exact(&mut len_bytes)?;
            let len = u32::from_le_bytes(len_bytes) as usize;

            let mut flags_bytes = [0u8; 4];
            reader.read_exact(&mut flags_bytes)?;
            let flags = u32::from_le_bytes(flags_bytes);

            let mut data = vec![0u8; len];
            reader.read_exact(&mut data)?;

            sections.push(AotSection {
                addr: vm_core::GuestAddr(addr),
                data,
                flags,
            });
        }

        Ok(AotImage { header, sections })
    }
}
