use std::fmt;
use vm_core::error::{CoreError, VmError};
use vm_cross_arch_support::encoding::{Architecture, EncodingError};
use vm_cross_arch_support::register::RegisterError;

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

impl TryFrom<Architecture> for SourceArch {
    type Error = ();

    fn try_from(arch: Architecture) -> Result<Self, Self::Error> {
        match arch {
            Architecture::X86_64 => Ok(SourceArch::X86_64),
            Architecture::ARM64 => Ok(SourceArch::ARM64),
            Architecture::RISCV64 => Ok(SourceArch::RISCV64),
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

impl TryFrom<Architecture> for TargetArch {
    type Error = ();

    fn try_from(arch: Architecture) -> Result<Self, Self::Error> {
        match arch {
            Architecture::X86_64 => Ok(TargetArch::X86_64),
            Architecture::ARM64 => Ok(TargetArch::ARM64),
            Architecture::RISCV64 => Ok(TargetArch::RISCV64),
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

/// 跨架构翻译错误
///
/// 定义跨架构指令翻译过程中可能发生的错误。
/// 此错误类型可以自动转换为统一的 `VmError`。
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
    #[error("不支持的架构转换")]
    UnsupportedArchitecturePair,
}

impl From<String> for TranslationError {
    fn from(s: String) -> Self {
        TranslationError::StringError(s)
    }
}

impl From<RegisterError> for TranslationError {
    fn from(e: RegisterError) -> Self {
        TranslationError::RegisterMappingFailed {
            reason: e.to_string(),
        }
    }
}

impl From<EncodingError> for TranslationError {
    fn from(e: EncodingError) -> Self {
        TranslationError::EncodingError {
            message: e.to_string(),
        }
    }
}

/// 将 `TranslationError` 转换为统一的 `VmError`
impl From<TranslationError> for VmError {
    fn from(e: TranslationError) -> Self {
        VmError::Core(CoreError::DecodeError {
            message: e.to_string(),
            position: None,
            module: "vm-cross-arch".to_string(),
        })
    }
}

/// 结果类型别名，使用 `TranslationError` 作为错误类型
///
/// 注意：此别名用于函数返回类型，与 `TranslationResult` 结构体（翻译结果）
/// 是不同的概念。
pub type TranslationOutcome<T> = Result<T, TranslationError>;
