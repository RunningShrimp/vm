use std::fmt;
use vm_encoder::Architecture;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceArch {
    X86_64,
    ARM64,
    RISCV64,
}

impl From<SourceArch> for Architecture {
    fn from(source: SourceArch) -> Self {
        match source {
            SourceArch::X86_64 => Architecture::X86_64,
            SourceArch::ARM64 => Architecture::ARM64,
            SourceArch::RISCV64 => Architecture::RISCV64,
        }
    }
}

impl fmt::Display for SourceArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceArch::X86_64 => write!(f, "x86_64"),
            SourceArch::ARM64 => write!(f, "arm64"),
            SourceArch::RISCV64 => write!(f, "riscv64"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetArch {
    X86_64,
    ARM64,
    RISCV64,
}

impl From<TargetArch> for Architecture {
    fn from(target: TargetArch) -> Self {
        match target {
            TargetArch::X86_64 => Architecture::X86_64,
            TargetArch::ARM64 => Architecture::ARM64,
            TargetArch::RISCV64 => Architecture::RISCV64,
        }
    }
}

impl fmt::Display for TargetArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetArch::X86_64 => write!(f, "x86_64"),
            TargetArch::ARM64 => write!(f, "arm64"),
            TargetArch::RISCV64 => write!(f, "riscv64"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("字符串错误: {0}")]
    StringError(String),
    #[error("不支持的IR操作: {op}")]
    UnsupportedOperation { op: String },
    #[error("立即数过大: {imm}")]
    ImmediateTooLarge { imm: i64 },
    #[error("无效的偏移量: {offset}")]
    InvalidOffset { offset: i64 },
    #[error("寄存器映射失败: {reason}")]
    RegisterMappingFailed { reason: String },
    #[error("寄存器分配失败: {0}")]
    RegisterAllocationFailed(String),
    #[error("编码错误: {message}")]
    EncodingError { message: String },
    #[error("不支持的架构转换: {source:?} -> {target:?}")]
    UnsupportedArchitecturePair {
        source: Architecture,
        target: Architecture,
    },
}

impl From<String> for TranslationError {
    fn from(s: String) -> Self {
        TranslationError::StringError(s)
    }
}
