use vm_core::{GuestAddr, VmError};

/// x86 扩展指令集解码器
pub struct ExtendedDecoder {
    cpu_features: Vec<String>,
    is_big_endian: bool,
}

/// x86 扩展指令表示
#[derive(Debug, Clone)]
pub struct X86Instruction {
    pub mnemonic: &'static str,
    pub operands: Vec<String>,
    pub length: u8,
    pub has_modrm: bool,
    pub has_imm: bool,
    pub is_privileged: bool,
}

impl X86Instruction {
    /// 获取操作数列表
    pub fn get_operands(&self) -> &[String] {
        &self.operands
    }

    /// 检查是否有指定的操作数
    pub fn has_operand(&self, operand: &str) -> bool {
        self.operands.contains(&operand.to_string())
    }
}

impl X86Instruction {
    /// 创建新的扩展指令
    pub fn new(mnemonic: &'static str, operands: Vec<String>, length: u8) -> Self {
        // 分析指令特征
        let has_modrm = Self::analyze_modrm(mnemonic, &operands);
        let has_imm = Self::analyze_immediate(mnemonic, &operands);
        let is_privileged = Self::analyze_privileged(mnemonic);

        Self {
            mnemonic,
            operands,
            length,
            has_modrm,
            has_imm,
            is_privileged,
        }
    }

    /// 分析是否包含ModRM字节
    fn analyze_modrm(mnemonic: &str, operands: &[String]) -> bool {
        // 简化分析：大多数算术和内存操作指令都有ModRM
        matches!(
            mnemonic,
            "add" | "sub" | "mov" | "cmp" | "test" | "and" | "or" | "xor"
        ) && operands
            .iter()
            .any(|op| op.contains('[') || op.contains('+'))
    }

    /// 分析是否包含立即数
    fn analyze_immediate(_mnemonic: &str, operands: &[String]) -> bool {
        // 检查操作数是否包含立即数（以数字或0x开头）
        operands
            .iter()
            .any(|op| op.starts_with("0x") || op.chars().next().is_some_and(|c| c.is_ascii_digit()))
    }

    /// 分析是否为特权指令
    fn analyze_privileged(mnemonic: &str) -> bool {
        // 特权指令列表
        matches!(
            mnemonic,
            "lgdt" | "lidt" | "ltr" | "lldt" | "rdmsr" | "wrmsr" | "cli" | "sti"
        )
    }
}

impl ExtendedDecoder {
    pub fn new() -> Self {
        let mut decoder = Self {
            cpu_features: vec![
                "sse".to_string(),
                "sse2".to_string(),
                "sse3".to_string(),
                "sse4.1".to_string(),
                "sse4.2".to_string(),
                "avx".to_string(),
                "avx2".to_string(),
            ],
            is_big_endian: false,
        };

        // 根据编译目标自动配置
        decoder.set_endian(cfg!(target_endian = "big"));
        decoder.set_cpu_features_by_list(decoder.cpu_features.clone());

        decoder
    }

    pub fn reset(&mut self) {
        // 重置为初始状态
        self.cpu_features = vec![
            "sse".to_string(),
            "sse2".to_string(),
            "sse3".to_string(),
            "sse4.1".to_string(),
            "sse4.2".to_string(),
            "avx".to_string(),
            "avx2".to_string(),
        ];
    }

    pub fn set_endian(&mut self, is_big_endian: bool) {
        self.is_big_endian = is_big_endian;
    }

    pub fn set_cpu_features(&mut self) {
        // 基于CPU信息设置支持的特性
        // 在实际实现中，这里会查询CPU特性并更新支持列表
        // 目前简化实现，保留默认特性
        // TODO: 实现真正的CPU特性检测
        self.cpu_features = vec!["sse".to_string(), "sse2".to_string(), "avx".to_string()];
    }

    /// 通过特性列表设置支持的CPU特性
    pub fn set_cpu_features_by_list(&mut self, features: Vec<String>) {
        self.cpu_features = features;
    }

    pub fn supports_feature(&self, feature: &str) -> bool {
        self.cpu_features.contains(&feature.to_string())
    }

    /// 检查是否支持特定特性（别名方法）
    pub fn supports(&self, feature: &str) -> bool {
        self.supports_feature(feature)
    }

    /// 检查是否是大端序
    pub fn is_big_endian(&self) -> bool {
        self.is_big_endian
    }

    pub fn get_supported_extensions(&self) -> Vec<String> {
        self.cpu_features.clone()
    }

    pub fn decode_extended(&self, bytes: &[u8], _pc: GuestAddr) -> Result<X86Instruction, VmError> {
        // 简化实现，返回一个基本的扩展指令
        // 在实际实现中，这里会根据字节码解析具体的扩展指令
        let (mnemonic, operands) = if !bytes.is_empty() {
            match bytes[0] {
                0x0F => {
                    if bytes.len() >= 2 {
                        match bytes[1] {
                            0x38 => ("sse4.1_insn", vec!["xmm0".to_string(), "xmm1".to_string()]),
                            0x3A => ("sse4.2_insn", vec!["xmm0".to_string(), "imm8".to_string()]),
                            _ => ("two_byte_insn", vec!["reg".to_string(), "mem".to_string()]),
                        }
                    } else {
                        ("extended_insn", vec!["unknown".to_string()])
                    }
                }
                0xC4 => (
                    "vex3_insn",
                    vec!["xmm0".to_string(), "xmm1".to_string(), "xmm2".to_string()],
                ),
                0xC5 => ("vex2_insn", vec!["xmm0".to_string(), "xmm1".to_string()]),
                _ => ("extended_insn", vec!["unknown".to_string()]),
            }
        } else {
            ("invalid_insn", vec![])
        };

        // 使用构造函数来创建指令，它会自动分析特征
        let instruction = X86Instruction::new(mnemonic, operands, bytes.len() as u8);

        // 验证指令结构完整性
        if instruction.has_operand("invalid") {
            log::warn!("Instruction contains invalid operand");
        }

        Ok(instruction)
    }
}

impl Default for ExtendedDecoder {
    fn default() -> Self {
        Self::new()
    }
}
