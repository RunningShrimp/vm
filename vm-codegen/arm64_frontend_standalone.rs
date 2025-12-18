//! # vm-frontend-arm64 - ARM64 前端解码器
//!
//! 提供 ARM64 架构的指令解码器，将 ARM64 机器码转换为 VM IR。
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use vm_frontend_arm64::ARM64Decoder;
//! use vm_core::Decoder;
//!
//! let mut decoder = ARM64Decoder::new();
//! let block = decoder.decode(&mmu, 0x1000)?;
//! ```

use std::collections::HashMap;

/// 扩展解码器trait
pub trait ExtensionDecoder: Send + Sync {
    fn decode(&self, insn: u32, builder: &mut IRBuilder) -> Result<bool, VmError>;
    fn name(&self) -> &str;
}

/// ARM64 指令表示
#[derive(Debug, Clone)]
pub struct ARM64Instruction {
    pub mnemonic: &'static str,
    pub next_pc: GuestAddr,
    pub has_memory_op: bool,
    pub is_branch: bool,
}

impl Instruction for ARM64Instruction {
    fn next_pc(&self) -> GuestAddr {
        self.next_pc
    }

    fn size(&self) -> u8 {
        4 // ARM64 指令固定 4 字节
    }

    fn operand_count(&self) -> usize {
        1 // 简化实现
    }

    fn mnemonic(&self) -> &str {
        self.mnemonic
    }

    fn is_control_flow(&self) -> bool {
        self.is_branch
    }

    fn is_memory_access(&self) -> bool {
        self.has_memory_op
    }
}
/// ARM64 解码器，支持解码缓存优化
pub struct ARM64Decoder {
    /// 解码缓存
    decode_cache: Option<std::collections::HashMap<GuestAddr, IRBlock>>,
    /// 缓存大小限制
    cache_size_limit: usize,    /// 扩展指令解码器
    pub extension_decoders: HashMap<String, Box<dyn ExtensionDecoder>>,
}

impl 
            extension_decoders: HashMap::new(),Decoder {
    /// 创建新的解码器
    pub fn new() -> Self {
        Self {
            decode_cache: Some(std::collections::HashMap::new()),
            cache_size_limit: 1024,ARM64
        }
    }

    /// 创建不带缓存的解码器（用于测试或内存受限环境）
    pub fn without_cache() -> Self {
        Self {
            decode_cache: None,
            cache_size_limit: 0,
            extension_decoders: HashMap::new(),
        }
    }

    /// 清除解码缓存
    pub fn clear_cache(&mut self) {
        if let Some(ref mut cache) = self.decode_cache {
            cache.clear();
        }
    }

    /// 获取缓存统计信息
    pub fn cache_stats(&self) -> (usize, usize) {
        if let Some(ref cache) = self.decode_cache {
            (cache.len(), self.cache_size_limit)
        } else {
            (0, 0)
        }
    }
}

impl Default for ARM64Decoder {
    fn default() -> Self {
        Self::new()
    }
}
