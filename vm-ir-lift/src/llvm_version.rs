//! LLVM版本检测和兼容性模块
//!
//! 提供LLVM版本检测、兼容性验证和自适应配置功能。

use std::ffi::CStr;
use std::fmt;
use std::str::FromStr;

#[cfg(feature = "llvm")]
use llvm_sys::{core::LLVMGetVersion, support::LLVMGetVersionString};

/// LLVM版本信息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlvmVersion {
    /// 主版本号
    pub major: u32,
    /// 次版本号
    pub minor: u32,
    /// 补丁版本号
    pub patch: u32,
    /// 版本字符串
    pub version_string: String,
}

impl LlvmVersion {
    /// 创建新的LLVM版本
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        let version_string = format!("{}.{}.{}", major, minor, patch);
        Self {
            major,
            minor,
            patch,
            version_string,
        }
    }

    /// 从版本字符串解析
    pub fn from_string(version_str: &str) -> Result<Self, VersionParseError> {
        let parts: Vec<&str> = version_str.trim().split('.').collect();
        if parts.len() < 2 {
            return Err(VersionParseError::InvalidFormat(version_str.to_string()));
        }

        let major = parts[0].parse()
            .map_err(|_| VersionParseError::InvalidNumber(parts[0].to_string()))?;
        let minor = parts[1].parse()
            .map_err(|_| VersionParseError::InvalidNumber(parts[1].to_string()))?;
        let patch = if parts.len() > 2 {
            parts[2].parse()
                .map_err(|_| VersionParseError::InvalidNumber(parts[2].to_string()))?
        } else {
            0
        };

        Ok(Self {
            major,
            minor,
            patch,
            version_string: version_str.to_string(),
        })
    }

    /// 检查版本是否兼容
    pub fn is_compatible(&self) -> bool {
        // 支持LLVM 15.x - 18.x版本
        match self.major {
            15 => true,
            16 => true,
            17 => true,
            18 => true,
            _ => false,
        }
    }

    /// 获取兼容性级别
    pub fn compatibility_level(&self) -> CompatibilityLevel {
        match self.major {
            18 => CompatibilityLevel::Full,
            17 => CompatibilityLevel::Full,
            16 => CompatibilityLevel::Good,
            15 => CompatibilityLevel::Basic,
            _ => CompatibilityLevel::Unsupported,
        }
    }

    /// 获取推荐的功能标志
    pub fn recommended_features(&self) -> Vec<&'static str> {
        match self.major {
            18 => vec![
                "llvm-18-features",
                "modern-ir-builder",
                "pass-manager",
                "jit-compiler",
            ],
            17 => vec![
                "llvm-17-features",
                "modern-ir-builder",
                "pass-manager",
                "jit-compiler",
            ],
            16 => vec![
                "llvm-16-features",
                "ir-builder",
                "basic-pass-manager",
            ],
            15 => vec![
                "llvm-15-features",
                "legacy-ir-builder",
                "basic-pass-manager",
            ],
            _ => vec![],
        }
    }

    /// 获取已知的兼容性问题
    pub fn known_issues(&self) -> Vec<&'static str> {
        match self.major {
            15 => vec![
                "某些优化Pass可能不可用",
                "JIT编译器功能有限",
                "API变化可能导致编译警告",
            ],
            16 => vec![
                "部分新功能可能不可用",
                "某些Pass行为可能有所不同",
            ],
            17 => vec![
                "实验性功能可能不稳定",
            ],
            18 => vec![],
            _ => vec![
                "版本不受支持，可能存在严重兼容性问题",
            ],
        }
    }
}

impl fmt::Display for LlvmVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.version_string)
    }
}

/// 版本兼容性级别
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityLevel {
    /// 完全兼容 - 支持所有功能
    Full,
    /// 良好兼容 - 支持大部分功能
    Good,
    /// 基本兼容 - 支持核心功能
    Basic,
    /// 不兼容 - 不支持
    Unsupported,
}

impl fmt::Display for CompatibilityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompatibilityLevel::Full => write!(f, "完全兼容"),
            CompatibilityLevel::Good => write!(f, "良好兼容"),
            CompatibilityLevel::Basic => write!(f, "基本兼容"),
            CompatibilityLevel::Unsupported => write!(f, "不兼容"),
        }
    }
}

/// 版本解析错误
#[derive(Debug, Clone)]
pub enum VersionParseError {
    /// 无效的格式
    InvalidFormat(String),
    /// 无效的数字
    InvalidNumber(String),
}

impl fmt::Display for VersionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionParseError::InvalidFormat(s) => write!(f, "无效的版本格式: {}", s),
            VersionParseError::InvalidNumber(s) => write!(f, "无效的版本号: {}", s),
        }
    }
}

impl std::error::Error for VersionParseError {}

/// LLVM版本检测器
pub struct LlvmVersionDetector;

impl LlvmVersionDetector {
    /// 检测当前安装的LLVM版本
    #[cfg(feature = "llvm")]
    pub fn detect() -> Result<LlvmVersion, LlvmDetectionError> {
        // 首先尝试从llvm-sys获取版本
        let mut major = 0;
        let mut minor = 0;
        let mut patch = 0;

        unsafe {
            LLVMGetVersion(&mut major, &mut minor, &mut patch);
        }

        if major > 0 {
            let version = LlvmVersion::new(major as u32, minor as u32, patch as u32);
            return Ok(version);
        }

        // 如果上述方法失败，尝试获取版本字符串
        let version_str = unsafe {
            let c_str = LLVMGetVersionString();
            CStr::from_ptr(c_str).to_string_lossy().to_string()
        };

        if !version_str.is_empty() {
            // 尝试从字符串解析版本
            LlvmVersion::from_string(&version_str)
                .map_err(|_| LlvmDetectionError::ParseError(version_str.clone()))
        } else {
            Err(LlvmDetectionError::NotAvailable)
        }
    }

    /// 在没有LLVM功能时返回默认版本
    #[cfg(not(feature = "llvm"))]
    pub fn detect() -> Result<LlvmVersion, LlvmDetectionError> {
        Err(LlvmDetectionError::NotCompiled)
    }

    /// 从环境变量检测LLVM版本
    pub fn detect_from_env() -> Result<LlvmVersion, LlvmDetectionError> {
        // 检查LLVM_SYS_XXX_PREFIX环境变量
        for (key, value) in std::env::vars() {
            if key.starts_with("LLVM_SYS_") && key.ends_with("_PREFIX") {
                tracing::debug!("发现LLVM环境变量: {} = {}", key, value);
                
                // 尝试从路径推断版本
                if let Some(version) = Self::extract_version_from_path(&value) {
                    return Ok(version);
                }
            }
        }

        // 尝试运行llvm-config命令
        if let Ok(output) = std::process::Command::new("llvm-config")
            .arg("--version")
            .output()
        {
            if output.status.success() {
                let version_str = String::from_utf8_lossy(&output.stdout);
                if let Ok(version) = LlvmVersion::from_string(&version_str) {
                    return Ok(version);
                }
            }
        }

        Err(LlvmDetectionError::NotAvailable)
    }

    /// 从路径提取版本信息
    fn extract_version_from_path(path: &str) -> Option<LlvmVersion> {
        // 从路径中提取版本号，如 /usr/lib/llvm-18 -> 18
        let re = regex::Regex::new(r"llvm-(\d+)").ok()?;
        if let Some(captures) = re.captures(path) {
            if let Some(version_match) = captures.get(1) {
                if let Ok(major) = version_match.as_str().parse::<u32>() {
                    return Some(LlvmVersion::new(major, 0, 0));
                }
            }
        }
        None
    }

    /// 验证LLVM安装
    pub fn verify_installation() -> LlvmVerificationResult {
        let mut result = LlvmVerificationResult::new();

        // 检查版本
        match Self::detect() {
            Ok(version) => {
                result.version = Some(version.clone());
                result.version_compatible = version.is_compatible();
                result.compatibility_level = version.compatibility_level();
            }
            Err(e) => {
                result.errors.push(format!("版本检测失败: {}", e));
                return result;
            }
        }

        // 检查关键组件
        let components = vec![
            ("llvm-config", "LLVM配置工具"),
            ("llc", "LLVM静态编译器"),
            ("opt", "LLVM优化器"),
            ("clang", "C/C++前端"),
        ];

        for (cmd, desc) in components {
            if std::process::Command::new(cmd)
                .arg("--version")
                .output()
                .is_ok()
            {
                result.available_components.push((cmd.to_string(), desc.to_string()));
            } else {
                result.missing_components.push((cmd.to_string(), desc.to_string()));
            }
        }

        // 检查库文件
        if let Ok(version) = &result.version {
            if let Some(prefix) = std::env::var(&format!("LLVM_SYS_{}_PREFIX", version.major)).ok() {
                let lib_path = format!("{}/lib", prefix);
                if std::path::Path::new(&lib_path).exists() {
                    result.library_path = Some(lib_path);
                } else {
                    result.errors.push(format!("库路径不存在: {}", lib_path));
                }
            }
        }

        result
    }
}

/// LLVM检测错误
#[derive(Debug, Clone)]
pub enum LlvmDetectionError {
    /// LLVM不可用
    NotAvailable,
    /// 未编译LLVM支持
    NotCompiled,
    /// 解析错误
    ParseError(String),
    /// 其他错误
    Other(String),
}

impl fmt::Display for LlvmDetectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlvmDetectionError::NotAvailable => write!(f, "LLVM不可用"),
            LlvmDetectionError::NotCompiled => write!(f, "未编译LLVM支持"),
            LlvmDetectionError::ParseError(s) => write!(f, "版本解析错误: {}", s),
            LlvmDetectionError::Other(s) => write!(f, "其他错误: {}", s),
        }
    }
}

impl std::error::Error for LlvmDetectionError {}

/// LLVM验证结果
#[derive(Debug, Clone)]
pub struct LlvmVerificationResult {
    /// 检测到的版本
    pub version: Option<LlvmVersion>,
    /// 版本是否兼容
    pub version_compatible: bool,
    /// 兼容性级别
    pub compatibility_level: CompatibilityLevel,
    /// 可用组件
    pub available_components: Vec<(String, String)>,
    /// 缺失组件
    pub missing_components: Vec<(String, String)>,
    /// 库路径
    pub library_path: Option<String>,
    /// 错误列表
    pub errors: Vec<String>,
}

impl LlvmVerificationResult {
    /// 创建新的验证结果
    pub fn new() -> Self {
        Self {
            version: None,
            version_compatible: false,
            compatibility_level: CompatibilityLevel::Unsupported,
            available_components: Vec::new(),
            missing_components: Vec::new(),
            library_path: None,
            errors: Vec::new(),
        }
    }

    /// 检查是否通过验证
    pub fn is_valid(&self) -> bool {
        self.version_compatible && self.errors.is_empty()
    }

    /// 生成报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("# LLVM验证报告\n\n");
        
        if let Some(version) = &self.version {
            report.push_str(&format!("**检测到的版本**: {}\n", version));
            report.push_str(&format!("**兼容性级别**: {}\n", self.compatibility_level));
            report.push_str(&format!("**版本兼容**: {}\n", 
                if self.version_compatible { "✅ 是" } else { "❌ 否" }));
        } else {
            report.push_str("**版本检测**: ❌ 失败\n");
        }
        
        if !self.available_components.is_empty() {
            report.push_str("\n**可用组件**:\n");
            for (cmd, desc) in &self.available_components {
                report.push_str(&format!("- ✅ {} ({})\n", cmd, desc));
            }
        }
        
        if !self.missing_components.is_empty() {
            report.push_str("\n**缺失组件**:\n");
            for (cmd, desc) in &self.missing_components {
                report.push_str(&format!("- ❌ {} ({})\n", cmd, desc));
            }
        }
        
        if let Some(lib_path) = &self.library_path {
            report.push_str(&format!("\n**库路径**: {}\n", lib_path));
        }
        
        if !self.errors.is_empty() {
            report.push_str("\n**错误**:\n");
            for error in &self.errors {
                report.push_str(&format!("- ❌ {}\n", error));
            }
        }
        
        report.push_str(&format!("\n**总体状态**: {}\n", 
            if self.is_valid() { "✅ 通过" } else { "❌ 失败" }));
        
        report
    }
}

/// LLVM配置管理器
pub struct LlvmConfigManager;

impl LlvmConfigManager {
    /// 自动配置LLVM环境
    pub fn auto_configure() -> Result<LlvmVersion, LlvmDetectionError> {
        // 首先尝试检测当前环境
        if let Ok(version) = LlvmVersionDetector::detect() {
            if version.is_compatible() {
                tracing::info!("检测到兼容的LLVM版本: {}", version);
                return Ok(version);
            }
        }

        // 尝试从环境变量检测
        if let Ok(version) = LlvmVersionDetector::detect_from_env() {
            if version.is_compatible() {
                tracing::info!("从环境变量检测到兼容的LLVM版本: {}", version);
                return Ok(version);
            }
        }

        Err(LlvmDetectionError::NotAvailable)
    }

    /// 获取推荐的编译配置
    pub fn get_recommended_config(version: &LlvmVersion) -> LlvmCompileConfig {
        LlvmCompileConfig {
            version: version.clone(),
            optimization_level: Self::recommended_optimization_level(version),
            enabled_features: version.recommended_features(),
            target_triple: Self::detect_target_triple(),
            use_lto: version.major >= 16,
            use_debug_info: cfg!(debug_assertions),
        }
    }

    /// 获取推荐的优化级别
    fn recommended_optimization_level(version: &LlvmVersion) -> &'static str {
        match version.major {
            18 => "O3",
            17 => "O3",
            16 => "O2",
            15 => "O2",
            _ => "O1",
        }
    }

    /// 检测目标三元组
    fn detect_target_triple() -> String {
        std::process::Command::new("llvm-config")
            .arg("--host-target")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| {
                // 回退到默认值
                if cfg!(target_os = "macos") {
                    "x86_64-apple-macosx".to_string()
                } else if cfg!(target_os = "linux") {
                    "x86_64-unknown-linux-gnu".to_string()
                } else if cfg!(target_os = "windows") {
                    "x86_64-pc-windows-msvc".to_string()
                } else {
                    "x86_64-unknown-none".to_string()
                }
            })
    }
}

/// LLVM编译配置
#[derive(Debug, Clone)]
pub struct LlvmCompileConfig {
    /// LLVM版本
    pub version: LlvmVersion,
    /// 优化级别
    pub optimization_level: &'static str,
    /// 启用的功能
    pub enabled_features: Vec<&'static str>,
    /// 目标三元组
    pub target_triple: String,
    /// 是否使用LTO
    pub use_lto: bool,
    /// 是否使用调试信息
    pub use_debug_info: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        let version = LlvmVersion::from_string("18.1.0").unwrap();
        assert_eq!(version.major, 18);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
        assert!(version.is_compatible());
        assert_eq!(version.compatibility_level(), CompatibilityLevel::Full);
    }

    #[test]
    fn test_version_compatibility() {
        assert!(LlvmVersion::new(18, 0, 0).is_compatible());
        assert!(LlvmVersion::new(17, 0, 0).is_compatible());
        assert!(LlvmVersion::new(16, 0, 0).is_compatible());
        assert!(LlvmVersion::new(15, 0, 0).is_compatible());
        assert!(!LlvmVersion::new(14, 0, 0).is_compatible());
        assert!(!LlvmVersion::new(19, 0, 0).is_compatible());
    }

    #[test]
    fn test_compatibility_level() {
        assert_eq!(LlvmVersion::new(18, 0, 0).compatibility_level(), CompatibilityLevel::Full);
        assert_eq!(LlvmVersion::new(17, 0, 0).compatibility_level(), CompatibilityLevel::Full);
        assert_eq!(LlvmVersion::new(16, 0, 0).compatibility_level(), CompatibilityLevel::Good);
        assert_eq!(LlvmVersion::new(15, 0, 0).compatibility_level(), CompatibilityLevel::Basic);
        assert_eq!(LlvmVersion::new(14, 0, 0).compatibility_level(), CompatibilityLevel::Unsupported);
    }

    #[test]
    fn test_recommended_features() {
        let v18 = LlvmVersion::new(18, 0, 0);
        let features = v18.recommended_features();
        assert!(features.contains(&"llvm-18-features"));
        assert!(features.contains(&"jit-compiler"));

        let v15 = LlvmVersion::new(15, 0, 0);
        let features = v15.recommended_features();
        assert!(features.contains(&"llvm-15-features"));
        assert!(features.contains(&"legacy-ir-builder"));
    }

    #[test]
    fn test_known_issues() {
        let v18 = LlvmVersion::new(18, 0, 0);
        assert!(v18.known_issues().is_empty());

        let v15 = LlvmVersion::new(15, 0, 0);
        let issues = v15.known_issues();
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.contains(&"JIT")));
    }
}