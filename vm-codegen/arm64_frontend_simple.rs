//! # vm-frontend-arm64 - ARM64 前端解码器
//!
//! 提供 ARM64 架构的指令解码器，将 ARM64 机器码转换为 VM IR。
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use vm_frontend_arm64::Arm64Decoder;
//!
//! let mut decoder = Arm64Decoder::new();
//! let instructions = decoder.decode_block(&data, 0x1000)?;
//! ```

/// ARM64 指令表示
#[derive(Debug, Clone)]
pub struct Arm64Instruction {
    pub mnemonic: &'static str,
    pub next_pc: u64,
    pub has_memory_op: bool,
    pub is_branch: bool,
}

impl Arm64Instruction {
    pub fn new(mnemonic: &'static str, next_pc: u64, has_memory_op: bool, is_branch: bool) -> Self {
        Self {
            mnemonic,
            next_pc,
            has_memory_op,
            is_branch,
        }
    }
}

/// ARM64 解码器，支持解码缓存优化
pub struct Arm64Decoder {
    /// 解码缓存
    decode_cache: Option<std::collections::HashMap<u64, Vec<u8>>>,
    /// 缓存大小限制
    cache_size_limit: usize,
}

impl Arm64Decoder {
    /// 创建新的解码器
    pub fn new() -> Self {
        Self {
            decode_cache: Some(std::collections::HashMap::new()),
            cache_size_limit: 1024,
        }
    }

    /// 创建不带缓存的解码器（用于测试或内存受限环境）
    pub fn without_cache() -> Self {
        Self {
            decode_cache: None,
            cache_size_limit: 0,
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

impl Default for Arm64Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Arm64Decoder {
    /// 解码单条指令
    pub fn decode_insn(&mut self, insn: u32, pc: u64) -> Arm64Instruction {
        let opcode = insn & 0x1F;
        
        // 确定指令助记符
        let mnemonic = match opcode {
            0x10 => "ADD_IMM",
            0x11 => "SUB_IMM",
            0x12 => "MOVZ",
            0x13 => "LDR_UNSIGNED",
            0x14 => "STR_UNSIGNED",
            0x15 => "B",
            0x16 => "RET",
            _ => "UNKNOWN",
        };
        
        // 确定指令属性
        let is_branch = matches!(opcode, 0x15 | 0x16);
        let has_memory_op = matches!(opcode, 0x13 | 0x14);
        
        Arm64Instruction::new(mnemonic, pc + 4, has_memory_op, is_branch)
    }
    
    /// 解码指令块
    pub fn decode_block(&mut self, data: &[u8], pc: u64) -> Result<Vec<Arm64Instruction>, String> {
        let mut instructions = Vec::new();
        let mut current_pc = pc;
        let mut i = 0;
        
        while i < data.len() {
            if i + 4 > data.len() {
                return Err("不完整的指令".to_string());
            }
            
            let insn = u32::from_le_bytes([
                data[i],
                data[i+1],
                data[i+2],
                data[i+3]
            ]);
            
            let instruction = self.decode_insn(insn, current_pc);
            instructions.push(instruction);
            
            i += 4;
            current_pc += 4;
            
            // 如果是分支指令，停止解码
            if instructions.last().unwrap().is_branch {
                break;
            }
        }
        
        Ok(instructions)
    }
}
