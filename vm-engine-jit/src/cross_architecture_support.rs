//! 跨架构编译支持模块
//!
//! 提供不同目标架构的 ISA 配置、型号选择和平台特定优化

use std::str::FromStr;

/// 目标架构类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetArch {
    /// x86-64 (Intel/AMD 64 位)
    X86_64,
    /// ARM 64 位
    Aarch64,
    /// RISC-V 64 位
    Riscv64,
    /// WebAssembly
    Wasm32,
    /// MIPS 64 位
    Mips64,
}

impl FromStr for TargetArch {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "x86_64" | "x86-64" | "amd64" => Ok(TargetArch::X86_64),
            "aarch64" | "arm64" | "armv8" => Ok(TargetArch::Aarch64),
            "riscv64" | "riscv" => Ok(TargetArch::Riscv64),
            "wasm32" => Ok(TargetArch::Wasm32),
            "mips64" => Ok(TargetArch::Mips64),
            _ => Err(format!("Unknown architecture: {}", s)),
        }
    }
}

impl std::fmt::Display for TargetArch {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TargetArch::X86_64 => write!(f, "x86_64"),
            TargetArch::Aarch64 => write!(f, "aarch64"),
            TargetArch::Riscv64 => write!(f, "riscv64"),
            TargetArch::Wasm32 => write!(f, "wasm32"),
            TargetArch::Mips64 => write!(f, "mips64"),
        }
    }
}

/// 目标操作系统
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetOS {
    /// Linux
    Linux,
    /// Windows
    Windows,
    /// macOS
    Darwin,
    /// 裸机（无操作系统）
    None,
    /// RTOS
    RealTime,
}

impl FromStr for TargetOS {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "linux" => Ok(TargetOS::Linux),
            "windows" | "win32" => Ok(TargetOS::Windows),
            "darwin" | "macos" => Ok(TargetOS::Darwin),
            "none" | "bare" => Ok(TargetOS::None),
            "rtos" | "realtime" => Ok(TargetOS::RealTime),
            _ => Err(format!("Unknown OS: {}", s)),
        }
    }
}

impl std::fmt::Display for TargetOS {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TargetOS::Linux => write!(f, "linux"),
            TargetOS::Windows => write!(f, "windows"),
            TargetOS::Darwin => write!(f, "darwin"),
            TargetOS::None => write!(f, "none"),
            TargetOS::RealTime => write!(f, "rtos"),
        }
    }
}

/// 目标系统三元组 (Triple)
/// 
/// 格式: arch-vendor-os 或 arch-os
#[derive(Debug, Clone)]
pub struct TargetTriple {
    /// 架构
    pub arch: TargetArch,
    /// 厂商（可选，例如 "unknown"）
    pub vendor: Option<String>,
    /// 操作系统
    pub os: TargetOS,
}

impl TargetTriple {
    /// 创建新的目标三元组
    pub fn new(arch: TargetArch, os: TargetOS) -> Self {
        Self {
            arch,
            vendor: None,
            os,
        }
    }

    /// 使用厂商创建目标三元组
    pub fn with_vendor(arch: TargetArch, vendor: String, os: TargetOS) -> Self {
        Self {
            arch,
            vendor: Some(vendor),
            os,
        }
    }

    /// 解析三元组字符串
    pub fn from_str(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split('-').collect();

        match parts.len() {
            2 => {
                let arch = parts[0].parse::<TargetArch>()?;
                let os = parts[1].parse::<TargetOS>()?;
                Ok(TargetTriple::new(arch, os))
            }
            3 => {
                let arch = parts[0].parse::<TargetArch>()?;
                let vendor = parts[1].to_string();
                let os = parts[2].parse::<TargetOS>()?;
                Ok(TargetTriple::with_vendor(arch, vendor, os))
            }
            _ => Err(format!(
                "Invalid triple format: {}. Expected 'arch-os' or 'arch-vendor-os'",
                s
            )),
        }
    }

    /// 转换为 Cranelift 三元组字符串
    pub fn to_cranelift_triple(&self) -> String {
        if let Some(ref vendor) = self.vendor {
            format!("{}-{}-{}", self.arch, vendor, self.os)
        } else {
            format!("{}-unknown-{}", self.arch, self.os)
        }
    }

    /// 获取 ISA 配置字符串
    /// 
    /// 用于 Cranelift 初始化
    pub fn get_isa_config(&self) -> String {
        match self.arch {
            TargetArch::X86_64 => {
                // x86-64 特定配置
                "has_avx=true,has_avx2=true,has_bmi1=true,opt_level=speed".to_string()
            }
            TargetArch::Aarch64 => {
                // ARM64 特定配置
                "has_neon=true,opt_level=speed".to_string()
            }
            TargetArch::Riscv64 => {
                // RISC-V 特定配置
                "opt_level=speed".to_string()
            }
            TargetArch::Wasm32 => {
                // WebAssembly 特定配置
                "opt_level=speed".to_string()
            }
            TargetArch::Mips64 => {
                // MIPS 特定配置
                "opt_level=speed".to_string()
            }
        }
    }
}

impl std::fmt::Display for TargetTriple {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.to_cranelift_triple())
    }
}

impl FromStr for TargetTriple {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TargetTriple::from_str(s)
    }
}

/// CPU 特性标志
#[derive(Debug, Clone)]
pub struct CpuFeatures {
    /// SIMD 扩展
    pub simd: bool,
    /// 高级向量扩展 (AVX)
    pub avx: bool,
    /// AVX-512
    pub avx512: bool,
    /// NEON (ARM)
    pub neon: bool,
    /// SVE (ARM)
    pub sve: bool,
    /// 原子操作支持
    pub atomic: bool,
    /// 浮点支持
    pub float: bool,
    /// 乘法指令
    pub mul: bool,
    /// 除法指令
    pub div: bool,
}

impl CpuFeatures {
    /// 为给定架构获取默认特性
    pub fn for_arch(arch: TargetArch) -> Self {
        match arch {
            TargetArch::X86_64 => Self {
                simd: true,
                avx: true,
                avx512: false,
                neon: false,
                sve: false,
                atomic: true,
                float: true,
                mul: true,
                div: true,
            },
            TargetArch::Aarch64 => Self {
                simd: true,
                avx: false,
                avx512: false,
                neon: true,
                sve: false,
                atomic: true,
                float: true,
                mul: true,
                div: true,
            },
            TargetArch::Riscv64 => Self {
                simd: false,
                avx: false,
                avx512: false,
                neon: false,
                sve: false,
                atomic: true,
                float: true,
                mul: true,
                div: true,
            },
            TargetArch::Wasm32 => Self {
                simd: true,
                avx: false,
                avx512: false,
                neon: false,
                sve: false,
                atomic: true,
                float: true,
                mul: true,
                div: true,
            },
            TargetArch::Mips64 => Self {
                simd: false,
                avx: false,
                avx512: false,
                neon: false,
                sve: false,
                atomic: true,
                float: true,
                mul: true,
                div: true,
            },
        }
    }
}

impl Default for CpuFeatures {
    fn default() -> Self {
        Self::for_arch(TargetArch::X86_64)
    }
}

/// 架构特定的优化选项
#[derive(Debug, Clone)]
pub struct ArchOptimizations {
    /// 使用特殊的移动优化
    pub prefer_moves: bool,
    /// 使用分支预测优化
    pub branch_prediction: bool,
    /// 使用缓存友好的代码布局
    pub cache_friendly: bool,
    /// 使用指令并行化
    pub instruction_level_parallelism: bool,
    /// 对齐要求
    pub alignment_requirement: usize,
}

impl ArchOptimizations {
    /// 获取架构特定的优化
    pub fn for_arch(arch: TargetArch) -> Self {
        match arch {
            TargetArch::X86_64 => Self {
                prefer_moves: true,
                branch_prediction: true,
                cache_friendly: true,
                instruction_level_parallelism: true,
                alignment_requirement: 16,
            },
            TargetArch::Aarch64 => Self {
                prefer_moves: true,
                branch_prediction: true,
                cache_friendly: true,
                instruction_level_parallelism: true,
                alignment_requirement: 8,
            },
            TargetArch::Riscv64 => Self {
                prefer_moves: false,
                branch_prediction: false,
                cache_friendly: false,
                instruction_level_parallelism: false,
                alignment_requirement: 4,
            },
            _ => Self {
                prefer_moves: false,
                branch_prediction: false,
                cache_friendly: false,
                instruction_level_parallelism: false,
                alignment_requirement: 4,
            },
        }
    }
}

impl Default for ArchOptimizations {
    fn default() -> Self {
        Self::for_arch(TargetArch::X86_64)
    }
}

/// 目标配置
#[derive(Debug, Clone)]
pub struct TargetConfig {
    /// 目标三元组
    pub triple: TargetTriple,
    /// CPU 特性
    pub features: CpuFeatures,
    /// 架构优化
    pub optimizations: ArchOptimizations,
}

impl TargetConfig {
    /// 创建新的目标配置
    pub fn new(triple: TargetTriple) -> Self {
        let features = CpuFeatures::for_arch(triple.arch);
        let optimizations = ArchOptimizations::for_arch(triple.arch);

        Self {
            triple,
            features,
            optimizations,
        }
    }

    /// 为原生平台创建配置
    pub fn native() -> Result<Self, String> {
        let arch = match std::env::consts::ARCH {
            "x86_64" => TargetArch::X86_64,
            "aarch64" => TargetArch::Aarch64,
            "riscv64" => TargetArch::Riscv64,
            "wasm32" => TargetArch::Wasm32,
            "mips64" => TargetArch::Mips64,
            other => return Err(format!("Unsupported architecture: {}", other)),
        };

        let os = match std::env::consts::OS {
            "linux" => TargetOS::Linux,
            "windows" => TargetOS::Windows,
            "macos" => TargetOS::Darwin,
            "none" => TargetOS::None,
            other => return Err(format!("Unsupported OS: {}", other)),
        };

        Ok(Self::new(TargetTriple::new(arch, os)))
    }

    /// 启用特定特性
    pub fn enable_feature(&mut self, feature: &str) {
        match feature {
            "simd" => self.features.simd = true,
            "avx" => self.features.avx = true,
            "avx512" => self.features.avx512 = true,
            "neon" => self.features.neon = true,
            "sve" => self.features.sve = true,
            "atomic" => self.features.atomic = true,
            "float" => self.features.float = true,
            "mul" => self.features.mul = true,
            "div" => self.features.div = true,
            _ => {}
        }
    }

    /// 禁用特定特性
    pub fn disable_feature(&mut self, feature: &str) {
        match feature {
            "simd" => self.features.simd = false,
            "avx" => self.features.avx = false,
            "avx512" => self.features.avx512 = false,
            "neon" => self.features.neon = false,
            "sve" => self.features.sve = false,
            "atomic" => self.features.atomic = false,
            "float" => self.features.float = false,
            "mul" => self.features.mul = false,
            "div" => self.features.div = false,
            _ => {}
        }
    }
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self::new(TargetTriple::new(TargetArch::X86_64, TargetOS::Linux))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_arch_parsing() {
        assert_eq!("x86_64".parse::<TargetArch>().unwrap(), TargetArch::X86_64);
        assert_eq!("aarch64".parse::<TargetArch>().unwrap(), TargetArch::Aarch64);
        assert_eq!("riscv64".parse::<TargetArch>().unwrap(), TargetArch::Riscv64);
    }

    #[test]
    fn test_target_os_parsing() {
        assert_eq!("linux".parse::<TargetOS>().unwrap(), TargetOS::Linux);
        assert_eq!("windows".parse::<TargetOS>().unwrap(), TargetOS::Windows);
        assert_eq!("darwin".parse::<TargetOS>().unwrap(), TargetOS::Darwin);
    }

    #[test]
    fn test_target_triple_parsing() {
        let triple = TargetTriple::from_str("x86_64-linux").unwrap();
        assert_eq!(triple.arch, TargetArch::X86_64);
        assert_eq!(triple.os, TargetOS::Linux);
        assert_eq!(triple.vendor, None);
    }

    #[test]
    fn test_target_triple_with_vendor() {
        let triple = TargetTriple::from_str("x86_64-unknown-linux").unwrap();
        assert_eq!(triple.arch, TargetArch::X86_64);
        assert_eq!(triple.os, TargetOS::Linux);
        assert_eq!(triple.vendor, Some("unknown".to_string()));
    }

    #[test]
    fn test_cpu_features() {
        let features = CpuFeatures::for_arch(TargetArch::X86_64);
        assert!(features.avx);
        assert!(features.atomic);

        let features = CpuFeatures::for_arch(TargetArch::Riscv64);
        assert!(!features.avx);
        assert!(features.atomic);
    }

    #[test]
    fn test_target_config_creation() {
        let config = TargetConfig::default();
        assert_eq!(config.triple.arch, TargetArch::X86_64);
    }

    #[test]
    fn test_enable_disable_features() {
        let mut config = TargetConfig::default();
        assert!(config.features.avx);
        
        config.disable_feature("avx");
        assert!(!config.features.avx);
        
        config.enable_feature("avx");
        assert!(config.features.avx);
    }
}
