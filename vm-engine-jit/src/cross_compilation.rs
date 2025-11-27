//! 交叉编译支持模块
//!
//! 实现多个目标架构的编译支持：x86_64, ARM64, RISC-V 等

use std::fmt;

/// 目标架构
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetArch {
    X86_64,
    Aarch64,
    Riscv64,
    Mips64,
}

impl fmt::Display for TargetArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetArch::X86_64 => write!(f, "x86_64"),
            TargetArch::Aarch64 => write!(f, "aarch64"),
            TargetArch::Riscv64 => write!(f, "riscv64"),
            TargetArch::Mips64 => write!(f, "mips64"),
        }
    }
}

/// 操作系统
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetOS {
    Linux,
    MacOS,
    Windows,
    FreeBSD,
}

impl fmt::Display for TargetOS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetOS::Linux => write!(f, "linux"),
            TargetOS::MacOS => write!(f, "macos"),
            TargetOS::Windows => write!(f, "windows"),
            TargetOS::FreeBSD => write!(f, "freebsd"),
        }
    }
}

/// 调用约定
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallingConvention {
    SystemV,      // x86_64 on Linux/BSD
    WindowsX64,   // x86_64 on Windows
    Aarch64Apcs,  // ARM64 APCS
    RiscvIlp32,   // RISC-V ILP32
    RiscvLp64,    // RISC-V LP64
}

/// 目标平台
#[derive(Debug, Clone)]
pub struct TargetPlatform {
    /// 架构
    pub arch: TargetArch,
    /// 操作系统
    pub os: TargetOS,
    /// 调用约定
    pub calling_convention: CallingConvention,
    /// 指针大小（字节）
    pub pointer_size: usize,
    /// 对齐要求
    pub alignment: usize,
}

impl TargetPlatform {
    /// 创建 x86_64 Linux 平台
    pub fn x86_64_linux() -> Self {
        Self {
            arch: TargetArch::X86_64,
            os: TargetOS::Linux,
            calling_convention: CallingConvention::SystemV,
            pointer_size: 8,
            alignment: 16,
        }
    }

    /// 创建 x86_64 Windows 平台
    pub fn x86_64_windows() -> Self {
        Self {
            arch: TargetArch::X86_64,
            os: TargetOS::Windows,
            calling_convention: CallingConvention::WindowsX64,
            pointer_size: 8,
            alignment: 8,
        }
    }

    /// 创建 ARM64 Linux 平台
    pub fn aarch64_linux() -> Self {
        Self {
            arch: TargetArch::Aarch64,
            os: TargetOS::Linux,
            calling_convention: CallingConvention::Aarch64Apcs,
            pointer_size: 8,
            alignment: 16,
        }
    }

    /// 创建 RISC-V 64 Linux 平台
    pub fn riscv64_linux() -> Self {
        Self {
            arch: TargetArch::Riscv64,
            os: TargetOS::Linux,
            calling_convention: CallingConvention::RiscvLp64,
            pointer_size: 8,
            alignment: 16,
        }
    }

    /// 创建 RISC-V 32 Linux 平台
    pub fn riscv32_linux() -> Self {
        Self {
            arch: TargetArch::Riscv64,
            os: TargetOS::Linux,
            calling_convention: CallingConvention::RiscvIlp32,
            pointer_size: 4,
            alignment: 8,
        }
    }

    /// 获取三元组字符串
    pub fn triple(&self) -> String {
        format!("{}-{}", self.arch, self.os)
    }

    /// 是否为小端
    pub fn is_little_endian(&self) -> bool {
        match self.arch {
            TargetArch::X86_64 => true,
            TargetArch::Aarch64 => true,
            TargetArch::Riscv64 => true,
            TargetArch::Mips64 => false, // MIPS 默认大端
        }
    }
}

/// 寄存器分配器（架构特定）
pub struct RegisterAllocator {
    platform: TargetPlatform,
}

impl RegisterAllocator {
    /// 创建新的寄存器分配器
    pub fn new(platform: TargetPlatform) -> Self {
        Self { platform }
    }

    /// 获取可用的通用寄存器数量
    pub fn available_gp_registers(&self) -> usize {
        match self.platform.arch {
            TargetArch::X86_64 => 16, // RAX-R15
            TargetArch::Aarch64 => 31, // X0-X30
            TargetArch::Riscv64 => 32, // x0-x31
            TargetArch::Mips64 => 32, // $0-$31
        }
    }

    /// 获取可用的浮点寄存器数量
    pub fn available_fp_registers(&self) -> usize {
        match self.platform.arch {
            TargetArch::X86_64 => 16, // XMM0-XMM15, YMM, ZMM
            TargetArch::Aarch64 => 32, // V0-V31
            TargetArch::Riscv64 => 32, // f0-f31
            TargetArch::Mips64 => 32, // $f0-$f31
        }
    }

    /// 获取参数传递寄存器
    pub fn get_arg_registers(&self) -> Vec<usize> {
        match self.platform.arch {
            TargetArch::X86_64 => {
                // System V: RDI, RSI, RDX, RCX, R8, R9
                vec![7, 6, 2, 1, 8, 9]
            }
            TargetArch::Aarch64 => {
                // APCS: X0-X7
                (0..8).collect()
            }
            TargetArch::Riscv64 => {
                // a0-a7 (x10-x17)
                (10..18).collect()
            }
            TargetArch::Mips64 => {
                // a0-a3 ($4-$7)
                vec![4, 5, 6, 7]
            }
        }
    }

    /// 获取被调用者保存的寄存器
    pub fn callee_saved_registers(&self) -> Vec<usize> {
        match self.platform.arch {
            TargetArch::X86_64 => {
                // RBX, RSP, RBP, R12-R15
                vec![3, 4, 5, 12, 13, 14, 15]
            }
            TargetArch::Aarch64 => {
                // X19-X28, SP, FP, LR
                (19..29).collect()
            }
            TargetArch::Riscv64 => {
                // s0-s11 (x8-x9, x18-x27)
                vec![8, 9, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27]
            }
            TargetArch::Mips64 => {
                // s0-s7 ($16-$23)
                (16..24).collect()
            }
        }
    }
}

/// 指令编码器
pub struct InstructionEncoder {
    platform: TargetPlatform,
}

impl InstructionEncoder {
    /// 创建新的指令编码器
    pub fn new(platform: TargetPlatform) -> Self {
        Self { platform }
    }

    /// 获取操作的本机大小（字节）
    pub fn get_op_size(&self, op_name: &str) -> usize {
        match self.platform.arch {
            TargetArch::X86_64 => {
                match op_name {
                    "add" | "sub" | "mov" => 3,
                    "imul" => 4,
                    "call" => 5,
                    _ => 2,
                }
            }
            TargetArch::Aarch64 => 4, // 所有 ARM64 指令都是 4 字节
            TargetArch::Riscv64 => 4, // 标准 RISC-V 指令都是 4 字节
            TargetArch::Mips64 => 4, // MIPS 指令都是 4 字节
        }
    }

    /// 检查是否支持操作
    pub fn supports_operation(&self, op_name: &str) -> bool {
        match self.platform.arch {
            TargetArch::X86_64 => {
                // x86_64 支持几乎所有操作
                true
            }
            TargetArch::Aarch64 => {
                // ARM64 不支持直接的立即数乘法
                op_name != "imul_imm"
            }
            TargetArch::Riscv64 => {
                // RISC-V 基础指令集支持
                !matches!(op_name, "avx_*" | "sse_*")
            }
            TargetArch::Mips64 => {
                // MIPS64 基础指令集
                !matches!(op_name, "avx_*" | "neon_*")
            }
        }
    }
}

/// 平台特定的优化提示
pub struct PlatformOptimizationHint {
    pub platform: TargetPlatform,
}

impl PlatformOptimizationHint {
    /// 创建新的优化提示
    pub fn new(platform: TargetPlatform) -> Self {
        Self { platform }
    }

    /// 获取推荐的向量化宽度
    pub fn recommended_vector_width(&self) -> usize {
        match self.platform.arch {
            TargetArch::X86_64 => 256, // AVX2
            TargetArch::Aarch64 => 128, // NEON
            TargetArch::Riscv64 => 128, // RVV (如果支持)
            TargetArch::Mips64 => 128, // MSA
        }
    }

    /// 获取推荐的缓存行大小
    pub fn cache_line_size(&self) -> usize {
        match self.platform.arch {
            TargetArch::X86_64 => 64,
            TargetArch::Aarch64 => 64,
            TargetArch::Riscv64 => 64,
            TargetArch::Mips64 => 32,
        }
    }

    /// 是否支持原子操作
    pub fn supports_atomics(&self) -> bool {
        true // 所有现代架构都支持
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_platform_creation() {
        let platform = TargetPlatform::x86_64_linux();
        assert_eq!(platform.arch, TargetArch::X86_64);
        assert_eq!(platform.pointer_size, 8);
    }

    #[test]
    fn test_register_allocator() {
        let platform = TargetPlatform::aarch64_linux();
        let allocator = RegisterAllocator::new(platform);
        assert_eq!(allocator.available_gp_registers(), 31);
    }

    #[test]
    fn test_instruction_encoder() {
        let platform = TargetPlatform::riscv64_linux();
        let encoder = InstructionEncoder::new(platform);
        assert_eq!(encoder.get_op_size("add"), 4);
    }

    #[test]
    fn test_optimization_hint() {
        let platform = TargetPlatform::x86_64_linux();
        let hint = PlatformOptimizationHint::new(platform);
        assert_eq!(hint.cache_line_size(), 64);
        assert_eq!(hint.recommended_vector_width(), 256);
    }
}
