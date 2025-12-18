//! LLVM功能检测模块
//!
//! 提供运行时LLVM功能检测和自适应配置功能。

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::llvm_version::{LlvmVersion, LlvmVersionDetector};

/// LLVM功能检测结果
#[derive(Debug, Clone)]
pub struct LlvmFeatures {
    /// 检测到的LLVM版本
    pub version: Option<LlvmVersion>,
    /// 可用的功能列表
    pub available_features: HashMap<String, FeatureStatus>,
    /// 支持的目标架构
    pub supported_targets: Vec<String>,
    /// 支持的优化级别
    pub optimization_levels: Vec<String>,
    /// JIT编译器支持
    pub jit_support: bool,
    /// 解释器支持
    pub interpreter_support: bool,
    /// MC层支持
    pub mc_support: bool,
    /// 目标代码生成支持
    pub target_codegen_support: bool,
}

/// 功能状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureStatus {
    /// 功能可用且完全支持
    Available,
    /// 功能可用但有部分限制
    Partial,
    /// 功能不可用
    Unavailable,
    /// 未知状态
    Unknown,
}

impl FeatureStatus {
    /// 检查功能是否可用
    pub fn is_available(&self) -> bool {
        matches!(self, FeatureStatus::Available | FeatureStatus::Partial)
    }
}

/// LLVM功能检测器
pub struct LlvmFeatureDetector;

impl LlvmFeatureDetector {
    /// 检测所有LLVM功能
    pub fn detect_all() -> LlvmFeatures {
        static FEATURES: OnceLock<LlvmFeatures> = OnceLock::new();
        FEATURES
            .get_or_init(|| {
                let mut features = LlvmFeatures::new();
                Self::detect_version(&mut features);
                Self::detect_core_features(&mut features);
                Self::detect_target_support(&mut features);
                Self::detect_optimization_support(&mut features);
                Self::detect_jit_support(&mut features);
                Self::detect_interpreter_support(&mut features);
                Self::detect_mc_support(&mut features);
                Self::detect_target_codegen_support(&mut features);
                Self::detect_advanced_features(&mut features);
                features
            })
            .clone()
    }

    /// 检测LLVM版本
    fn detect_version(features: &mut LlvmFeatures) {
        features.version = LlvmVersionDetector::detect().ok();
    }

    /// 检测核心功能
    fn detect_core_features(features: &mut LlvmFeatures) {
        let core_features = vec![
            ("ir_builder", "IR构建器"),
            ("pass_manager", "Pass管理器"),
            ("analysis", "分析Pass"),
            ("transforms", "变换Pass"),
            ("code_gen", "代码生成"),
            ("linker", "链接器"),
            ("target", "目标描述"),
        ];

        for (feature_id, _desc) in core_features {
            let status = Self::check_feature_availability(feature_id);
            features.available_features.insert(feature_id.to_string(), status);
        }
    }

    /// 检测目标支持
    fn detect_target_support(features: &mut LlvmFeatures) {
        // 尝试获取支持的目标列表
        if let Ok(output) = std::process::Command::new("llc")
            .arg("--version")
            .output()
        {
            if output.status.success() {
                // 从版本信息推断支持的目标
                let version_output = String::from_utf8_lossy(&output.stdout);
                
                // 常见目标架构
                let common_targets = vec![
                    "x86_64",
                    "x86",
                    "arm64",
                    "aarch64",
                    "arm",
                    "riscv64",
                    "riscv32",
                    "mips64",
                    "mips",
                    "powerpc64",
                    "powerpc",
                ];

                for target in common_targets {
                    if Self::check_target_support(target) {
                        features.supported_targets.push(target.to_string());
                    }
                }
            }
        }
    }

    /// 检测优化支持
    fn detect_optimization_support(features: &mut LlvmFeatures) {
        let optimization_levels = vec!["O0", "O1", "O2", "O3", "Os", "Oz"];
        
        for level in optimization_levels {
            if Self::check_optimization_level(level) {
                features.optimization_levels.push(level.to_string());
            }
        }
    }

    /// 检测JIT支持
    fn detect_jit_support(features: &mut LlvmFeatures) {
        features.jit_support = Self::check_jit_support();
    }

    /// 检测解释器支持
    fn detect_interpreter_support(features: &mut LlvmFeatures) {
        features.interpreter_support = Self::check_interpreter_support();
    }

    /// 检测MC层支持
    fn detect_mc_support(features: &mut LlvmFeatures) {
        features.mc_support = Self::check_mc_support();
    }

    /// 检测目标代码生成支持
    fn detect_target_codegen_support(features: &mut LlvmFeatures) {
        features.target_codegen_support = Self::check_target_codegen_support();
    }

    /// 检测高级功能
    fn detect_advanced_features(features: &mut LlvmFeatures) {
        let advanced_features = vec![
            ("lto", "链接时优化"),
            ("thin_lto", " ThinLTO"),
            ("pgo", "配置引导优化"),
            ("sample_pgo", "采样PGO"),
            ("instrumentation", "插桩"),
            ("sanitizers", "清理器"),
            ("coverage", "代码覆盖率"),
            ("profile", "性能分析"),
            ("debug_info", "调试信息"),
            ("parallel_code_gen", "并行代码生成"),
            ("vectorization", "向量化"),
            ("loop_optimization", "循环优化"),
            ("interprocedural_optimization", "过程间优化"),
        ];

        for (feature_id, _desc) in advanced_features {
            let status = Self::check_advanced_feature(feature_id);
            features.available_features.insert(feature_id.to_string(), status);
        }
    }

    /// 检查单个功能可用性
    fn check_feature_availability(feature_id: &str) -> FeatureStatus {
        #[cfg(feature = "llvm")]
        {
            // 在有LLVM支持的情况下，进行实际检测
            match feature_id {
                "ir_builder" => Self::check_ir_builder_support(),
                "pass_manager" => Self::check_pass_manager_support(),
                "analysis" => Self::check_analysis_support(),
                "transforms" => Self::check_transforms_support(),
                "code_gen" => Self::check_code_gen_support(),
                "linker" => Self::check_linker_support(),
                "target" => Self::check_target_support_available(),
                _ => FeatureStatus::Unknown,
            }
        }
        #[cfg(not(feature = "llvm"))]
        {
            // 在没有LLVM支持的情况下，返回不可用
            FeatureStatus::Unavailable
        }
    }

    #[cfg(feature = "llvm")]
    fn check_ir_builder_support() -> FeatureStatus {
        // 检查IR构建器API是否可用
        use llvm_sys::core::*;
        use llvm_sys::ir_builder::*;
        
        unsafe {
            let context = LLVMContextCreate();
            if context.is_null() {
                return FeatureStatus::Unavailable;
            }
            
            let builder = LLVMCreateBuilderInContext(context);
            let available = !builder.is_null();
            
            LLVMDisposeBuilder(builder);
            LLVMContextDispose(context);
            
            if available {
                FeatureStatus::Available
            } else {
                FeatureStatus::Unavailable
            }
        }
    }

    #[cfg(feature = "llvm")]
    fn check_pass_manager_support() -> FeatureStatus {
        use llvm_sys::core::*;
        use llvm_sys::passes::*;
        
        unsafe {
            let context = LLVMContextCreate();
            if context.is_null() {
                return FeatureStatus::Unavailable;
            }
            
            let module = LLVMModuleCreateWithNameInContext(b"test\0".as_ptr() as *const _, context);
            if module.is_null() {
                LLVMContextDispose(context);
                return FeatureStatus::Unavailable;
            }
            
            let pass_manager = LLVMCreatePassManager();
            let available = !pass_manager.is_null();
            
            LLVMDisposePassManager(pass_manager);
            LLVMDisposeModule(module);
            LLVMContextDispose(context);
            
            if available {
                FeatureStatus::Available
            } else {
                FeatureStatus::Unavailable
            }
        }
    }

    #[cfg(feature = "llvm")]
    fn check_analysis_support() -> FeatureStatus {
        // 检查分析Pass是否可用
        Self::check_command_availability("opt")
    }

    #[cfg(feature = "llvm")]
    fn check_transforms_support() -> FeatureStatus {
        // 检查变换Pass是否可用
        Self::check_command_availability("opt")
    }

    #[cfg(feature = "llvm")]
    fn check_code_gen_support() -> FeatureStatus {
        Self::check_command_availability("llc")
    }

    #[cfg(feature = "llvm")]
    fn check_linker_support() -> FeatureStatus {
        Self::check_command_availability("llvm-link")
    }

    #[cfg(feature = "llvm")]
    fn check_target_support_available() -> FeatureStatus {
        Self::check_command_availability("llc")
    }

    /// 检查目标架构支持
    fn check_target_support(target: &str) -> bool {
        if let Ok(output) = std::process::Command::new("llc")
            .args(&["--version", &format!("-mtriple={}", target)])
            .output()
        {
            output.status.success()
        } else {
            false
        }
    }

    /// 检查优化级别支持
    fn check_optimization_level(level: &str) -> bool {
        if let Ok(output) = std::process::Command::new("opt")
            .args(&[&format!("-{}", level), "--version"])
            .output()
        {
            output.status.success()
        } else {
            false
        }
    }

    /// 检查JIT支持
    fn check_jit_support() -> bool {
        #[cfg(feature = "llvm")]
        {
            use llvm_sys::execution_engine::*;
            use llvm_sys::core::*;
            
            unsafe {
                let context = LLVMContextCreate();
                if context.is_null() {
                    return false;
                }
                
                let module = LLVMModuleCreateWithNameInContext(b"test\0".as_ptr() as *const _, context);
                if module.is_null() {
                    LLVMContextDispose(context);
                    return false;
                }
                
                let mut engine = std::ptr::null_mut();
                let mut error = std::ptr::null_mut();
                let result = LLVMAcceptExecutionEngineForModule(
                    &mut engine,
                    module,
                    &mut error,
                );
                
                let supported = result == 0 && !engine.is_null();
                
                if !engine.is_null() {
                    LLVMDisposeExecutionEngine(engine);
                } else {
                    LLVMDisposeModule(module);
                }
                LLVMContextDispose(context);
                
                supported
            }
        }
        #[cfg(not(feature = "llvm"))]
        {
            false
        }
    }

    /// 检查解释器支持
    fn check_interpreter_support() -> bool {
        #[cfg(feature = "llvm")]
        {
            use llvm_sys::execution_engine::*;
            use llvm_sys::core::*;
            
            unsafe {
                let context = LLVMContextCreate();
                if context.is_null() {
                    return false;
                }
                
                let module = LLVMModuleCreateWithNameInContext(b"test\0".as_ptr() as *const _, context);
                if module.is_null() {
                    LLVMContextDispose(context);
                    return false;
                }
                
                let mut engine = std::ptr::null_mut();
                let mut error = std::ptr::null_mut();
                let result = LLVMCreateInterpreterForModule(
                    &mut engine,
                    module,
                    &mut error,
                );
                
                let supported = result == 0 && !engine.is_null();
                
                if !engine.is_null() {
                    LLVMDisposeExecutionEngine(engine);
                } else {
                    LLVMDisposeModule(module);
                }
                LLVMContextDispose(context);
                
                supported
            }
        }
        #[cfg(not(feature = "llvm"))]
        {
            false
        }
    }

    /// 检查MC层支持
    fn check_mc_support() -> bool {
        Self::check_command_availability("llvm-mc")
    }

    /// 检查目标代码生成支持
    fn check_target_codegen_support() -> bool {
        Self::check_command_availability("llc")
    }

    /// 检查高级功能
    fn check_advanced_feature(feature_id: &str) -> FeatureStatus {
        match feature_id {
            "lto" => Self::check_lto_support(),
            "thin_lto" => Self::check_thin_lto_support(),
            "pgo" => Self::check_pgo_support(),
            "sample_pgo" => Self::check_sample_pgo_support(),
            "instrumentation" => Self::check_instrumentation_support(),
            "sanitizers" => Self::check_sanitizers_support(),
            "coverage" => Self::check_coverage_support(),
            "profile" => Self::check_profile_support(),
            "debug_info" => Self::check_debug_info_support(),
            "parallel_code_gen" => Self::check_parallel_code_gen_support(),
            "vectorization" => Self::check_vectorization_support(),
            "loop_optimization" => Self::check_loop_optimization_support(),
            "interprocedural_optimization" => Self::check_interprocedural_optimization_support(),
            _ => FeatureStatus::Unknown,
        }
    }

    fn check_lto_support() -> FeatureStatus {
        if Self::check_command_availability("llvm-lto") {
            FeatureStatus::Available
        } else if Self::check_command_availability("opt") {
            // 检查opt是否支持LTO相关选项
            if let Ok(output) = std::process::Command::new("opt")
                .args(&["--help"])
                .output()
            {
                let help_text = String::from_utf8_lossy(&output.stdout);
                if help_text.contains("lto") || help_text.contains("LTO") {
                    FeatureStatus::Partial
                } else {
                    FeatureStatus::Unavailable
                }
            } else {
                FeatureStatus::Unavailable
            }
        } else {
            FeatureStatus::Unavailable
        }
    }

    fn check_thin_lto_support() -> FeatureStatus {
        if let Ok(output) = std::process::Command::new("opt")
            .args(&["--help"])
            .output()
        {
            let help_text = String::from_utf8_lossy(&output.stdout);
            if help_text.contains("thinlto") || help_text.contains("thin-lto") {
                FeatureStatus::Available
            } else {
                FeatureStatus::Unavailable
            }
        } else {
            FeatureStatus::Unavailable
        }
    }

    fn check_pgo_support() -> FeatureStatus {
        if let Ok(output) = std::process::Command::new("opt")
            .args(&["--help"])
            .output()
        {
            let help_text = String::from_utf8_lossy(&output.stdout);
            if help_text.contains("pgo") || help_text.contains("profile") {
                FeatureStatus::Available
            } else {
                FeatureStatus::Unavailable
            }
        } else {
            FeatureStatus::Unavailable
        }
    }

    fn check_sample_pgo_support() -> FeatureStatus {
        if let Ok(output) = std::process::Command::new("opt")
            .args(&["--help"])
            .output()
        {
            let help_text = String::from_utf8_lossy(&output.stdout);
            if help_text.contains("sample-profile") {
                FeatureStatus::Available
            } else {
                FeatureStatus::Unavailable
            }
        } else {
            FeatureStatus::Unavailable
        }
    }

    fn check_instrumentation_support() -> FeatureStatus {
        Self::check_command_availability("llvm-profdata")
    }

    fn check_sanitizers_support() -> FeatureStatus {
        // 检查是否支持AddressSanitizer等
        if let Ok(output) = std::process::Command::new("clang")
            .args(&["--help"])
            .output()
        {
            let help_text = String::from_utf8_lossy(&output.stdout);
            if help_text.contains("sanitize") {
                FeatureStatus::Available
            } else {
                FeatureStatus::Unavailable
            }
        } else {
            FeatureStatus::Unavailable
        }
    }

    fn check_coverage_support() -> FeatureStatus {
        if Self::check_command_availability("llvm-cov") {
            FeatureStatus::Available
        } else if Self::check_command_availability("clang") {
            // 检查clang是否支持覆盖率
            if let Ok(output) = std::process::Command::new("clang")
                .args(&["--help"])
                .output()
            {
                let help_text = String::from_utf8_lossy(&output.stdout);
                if help_text.contains("coverage") {
                    FeatureStatus::Partial
                } else {
                    FeatureStatus::Unavailable
                }
            } else {
                FeatureStatus::Unavailable
            }
        } else {
            FeatureStatus::Unavailable
        }
    }

    fn check_profile_support() -> FeatureStatus {
        Self::check_command_availability("llvm-profdata")
    }

    fn check_debug_info_support() -> FeatureStatus {
        if Self::check_command_availability("dsymutil") || cfg!(target_os = "linux") {
            FeatureStatus::Available
        } else {
            FeatureStatus::Partial
        }
    }

    fn check_parallel_code_gen_support() -> FeatureStatus {
        if let Ok(output) = std::process::Command::new("llc")
            .args(&["--help"])
            .output()
        {
            let help_text = String::from_utf8_lossy(&output.stdout);
            if help_text.contains("threads") || help_text.contains("parallel") {
                FeatureStatus::Available
            } else {
                FeatureStatus::Unavailable
            }
        } else {
            FeatureStatus::Unavailable
        }
    }

    fn check_vectorization_support() -> FeatureStatus {
        if let Ok(output) = std::process::Command::new("opt")
            .args(&["--help"])
            .output()
        {
            let help_text = String::from_utf8_lossy(&output.stdout);
            if help_text.contains("vectorize") || help_text.contains("loop-vectorize") {
                FeatureStatus::Available
            } else {
                FeatureStatus::Unavailable
            }
        } else {
            FeatureStatus::Unavailable
        }
    }

    fn check_loop_optimization_support() -> FeatureStatus {
        if let Ok(output) = std::process::Command::new("opt")
            .args(&["--help"])
            .output()
        {
            let help_text = String::from_utf8_lossy(&output.stdout);
            if help_text.contains("loop") {
                FeatureStatus::Available
            } else {
                FeatureStatus::Unavailable
            }
        } else {
            FeatureStatus::Unavailable
        }
    }

    fn check_interprocedural_optimization_support() -> FeatureStatus {
        if let Ok(output) = std::process::Command::new("opt")
            .args(&["--help"])
            .output()
        {
            let help_text = String::from_utf8_lossy(&output.stdout);
            if help_text.contains("interprocedural") || help_text.contains("ipo") {
                FeatureStatus::Available
            } else {
                FeatureStatus::Unavailable
            }
        } else {
            FeatureStatus::Unavailable
        }
    }

    /// 检查命令可用性
    fn check_command_availability(command: &str) -> bool {
        std::process::Command::new(command)
            .arg("--version")
            .output()
            .is_ok()
    }
}

impl LlvmFeatures {
    /// 创建新的功能检测结果
    fn new() -> Self {
        Self {
            version: None,
            available_features: HashMap::new(),
            supported_targets: Vec::new(),
            optimization_levels: Vec::new(),
            jit_support: false,
            interpreter_support: false,
            mc_support: false,
            target_codegen_support: false,
        }
    }

    /// 检查特定功能是否可用
    pub fn has_feature(&self, feature_id: &str) -> bool {
        self.available_features
            .get(feature_id)
            .map(|status| status.is_available())
            .unwrap_or(false)
    }

    /// 获取功能状态
    pub fn feature_status(&self, feature_id: &str) -> FeatureStatus {
        self.available_features
            .get(feature_id)
            .cloned()
            .unwrap_or(FeatureStatus::Unknown)
    }

    /// 检查目标架构是否支持
    pub fn supports_target(&self, target: &str) -> bool {
        self.supported_targets.contains(&target.to_string())
    }

    /// 检查优化级别是否支持
    pub fn supports_optimization_level(&self, level: &str) -> bool {
        self.optimization_levels.contains(&level.to_string())
    }

    /// 生成功能报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("# LLVM功能检测报告\n\n");
        
        // 版本信息
        if let Some(version) = &self.version {
            report.push_str(&format!("**LLVM版本**: {}\n", version));
            report.push_str(&format!("**兼容性**: {}\n", 
                if version.is_compatible() { "✅ 兼容" } else { "❌ 不兼容" }));
        } else {
            report.push_str("**LLVM版本**: ❌ 未检测到\n");
        }
        
        // 核心功能
        report.push_str("\n## 核心功能\n\n");
        let core_features = [
            ("ir_builder", "IR构建器"),
            ("pass_manager", "Pass管理器"),
            ("analysis", "分析Pass"),
            ("transforms", "变换Pass"),
            ("code_gen", "代码生成"),
            ("linker", "链接器"),
            ("target", "目标描述"),
        ];
        
        for (feature_id, desc) in core_features {
            let status = self.feature_status(feature_id);
            let icon = match status {
                FeatureStatus::Available => "✅",
                FeatureStatus::Partial => "⚠️",
                FeatureStatus::Unavailable => "❌",
                FeatureStatus::Unknown => "❓",
            };
            report.push_str(&format!("- {} {} ({})\n", icon, desc, feature_id));
        }
        
        // 高级功能
        report.push_str("\n## 高级功能\n\n");
        let advanced_features = [
            ("lto", "链接时优化"),
            ("thin_lto", "ThinLTO"),
            ("pgo", "配置引导优化"),
            ("sample_pgo", "采样PGO"),
            ("instrumentation", "插桩"),
            ("sanitizers", "清理器"),
            ("coverage", "代码覆盖率"),
            ("profile", "性能分析"),
            ("debug_info", "调试信息"),
            ("parallel_code_gen", "并行代码生成"),
            ("vectorization", "向量化"),
            ("loop_optimization", "循环优化"),
            ("interprocedural_optimization", "过程间优化"),
        ];
        
        for (feature_id, desc) in advanced_features {
            let status = self.feature_status(feature_id);
            let icon = match status {
                FeatureStatus::Available => "✅",
                FeatureStatus::Partial => "⚠️",
                FeatureStatus::Unavailable => "❌",
                FeatureStatus::Unknown => "❓",
            };
            report.push_str(&format!("- {} {} ({})\n", icon, desc, feature_id));
        }
        
        // 支持的目标
        if !self.supported_targets.is_empty() {
            report.push_str("\n## 支持的目标架构\n\n");
            for target in &self.supported_targets {
                report.push_str(&format!("- ✅ {}\n", target));
            }
        }
        
        // 优化级别
        if !self.optimization_levels.is_empty() {
            report.push_str("\n## 支持的优化级别\n\n");
            for level in &self.optimization_levels {
                report.push_str(&format!("- ✅ {}\n", level));
            }
        }
        
        // 特殊支持
        report.push_str("\n## 特殊支持\n\n");
        report.push_str(&format!("- **JIT编译器**: {}\n", 
            if self.jit_support { "✅ 支持" } else { "❌ 不支持" }));
        report.push_str(&format!("- **解释器**: {}\n", 
            if self.interpreter_support { "✅ 支持" } else { "❌ 不支持" }));
        report.push_str(&format!("- **MC层**: {}\n", 
            if self.mc_support { "✅ 支持" } else { "❌ 不支持" }));
        report.push_str(&format!("- **目标代码生成**: {}\n", 
            if self.target_codegen_support { "✅ 支持" } else { "❌ 不支持" }));
        
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_detection() {
        let features = LlvmFeatureDetector::detect_all();
        
        // 基本检查
        assert!(!features.available_features.is_empty());
        
        // 检查核心功能
        let core_features = ["ir_builder", "pass_manager", "analysis", "transforms"];
        for feature in core_features {
            println!("Feature {}: {:?}", feature, features.feature_status(feature));
        }
    }

    #[test]
    fn test_feature_status() {
        assert!(FeatureStatus::Available.is_available());
        assert!(FeatureStatus::Partial.is_available());
        assert!(!FeatureStatus::Unavailable.is_available());
        assert!(!FeatureStatus::Unknown.is_available());
    }

    #[test]
    fn test_report_generation() {
        let features = LlvmFeatureDetector::detect_all();
        let report = features.generate_report();
        
        assert!(report.contains("LLVM功能检测报告"));
        assert!(report.contains("核心功能"));
        assert!(report.contains("高级功能"));
    }
}