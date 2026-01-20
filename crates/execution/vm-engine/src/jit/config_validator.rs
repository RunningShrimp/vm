//! JIT引擎配置验证
//!
//! 提供JIT引擎配置参数的验证功能，确保配置的正确性和合理性

// 条件导入 num_cpus
cfg_if::cfg_if! {
    if #[cfg(all(target_os = "linux", feature = "std"))] {
        use num_cpus;

        fn get_cpu_count() -> usize {
            num_cpus::get()
        }
    } else {
        fn get_cpu_count() -> usize {
            1 // 默认单线程
        }
    }
}

use std::collections::HashMap;
use std::fmt::{self, Display};
use vm_core::VmError;

/// 配置验证结果
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// 是否验证通过
    pub is_valid: bool,
    /// 验证错误列表
    pub errors: Vec<ValidationError>,
    /// 验证警告列表
    pub warnings: Vec<ValidationWarning>,
    /// 验证信息列表
    pub infos: Vec<ValidationInfo>,
}

impl ValidationResult {
    /// 创建成功的验证结果
    pub fn success() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            infos: Vec::new(),
        }
    }

    /// 创建失败的验证结果
    pub fn failure(errors: Vec<ValidationError>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
            infos: Vec::new(),
        }
    }

    /// 添加错误
    pub fn add_error(&mut self, error: ValidationError) {
        self.is_valid = false;
        self.errors.push(error);
    }

    /// 添加警告
    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    /// 添加信息
    pub fn add_info(&mut self, info: ValidationInfo) {
        self.infos.push(info);
    }

    /// 合并另一个验证结果
    pub fn merge(&mut self, other: ValidationResult) {
        if !other.is_valid {
            self.is_valid = false;
        }
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
        self.infos.extend(other.infos);
    }

    /// 获取错误数量
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// 获取警告数量
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    /// 获取信息数量
    pub fn info_count(&self) -> usize {
        self.infos.len()
    }

    /// 生成格式化的验证报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("# JIT引擎配置验证报告\n\n");
        
        // 总体状态
        if self.is_valid {
            report.push_str("## 验证状态: ✅ 通过\n\n");
        } else {
            report.push_str("## 验证状态: ❌ 失败\n\n");
        }
        
        // 错误
        if !self.errors.is_empty() {
            report.push_str("## 错误\n\n");
            for (i, error) in self.errors.iter().enumerate() {
                report.push_str(&format!("{}. {}\n", i + 1, error));
            }
            report.push_str("\n");
        }
        
        // 警告
        if !self.warnings.is_empty() {
            report.push_str("## 警告\n\n");
            for (i, warning) in self.warnings.iter().enumerate() {
                report.push_str(&format!("{}. {}\n", i + 1, warning));
            }
            report.push_str("\n");
        }
        
        // 信息
        if !self.infos.is_empty() {
            report.push_str("## 信息\n\n");
            for (i, info) in self.infos.iter().enumerate() {
                report.push_str(&format!("{}. {}\n", i + 1, info));
            }
            report.push_str("\n");
        }
        
        // 摘要
        report.push_str("## 摘要\n\n");
        report.push_str(&format!("- 错误: {}\n", self.error_count()));
        report.push_str(&format!("- 警告: {}\n", self.warning_count()));
        report.push_str(&format!("- 信息: {}\n", self.info_count()));
        
        report
    }
}

/// 验证错误
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// 错误代码
    pub code: String,
    /// 错误消息
    pub message: String,
    /// 错误字段路径
    pub field_path: String,
    /// 错误严重程度
    pub severity: ErrorSeverity,
    /// 建议修复方案
    pub suggestion: Option<String>,
}

impl Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {} (字段: {})", 
            self.code, 
            self.severity, 
            self.message,
            self.field_path
        )?;
        
        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n  建议: {}", suggestion)?;
        }
        
        Ok(())
    }
}

/// 错误严重程度
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// 致命错误
    Fatal,
    /// 错误
    Error,
    /// 警告
    Warning,
}

impl Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Fatal => write!(f, "致命"),
            ErrorSeverity::Error => write!(f, "错误"),
            ErrorSeverity::Warning => write!(f, "警告"),
        }
    }
}

/// 验证警告
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// 警告代码
    pub code: String,
    /// 警告消息
    pub message: String,
    /// 警告字段路径
    pub field_path: String,
    /// 建议优化方案
    pub suggestion: Option<String>,
}

impl Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", 
            self.code, 
            self.message,
            self.field_path
        )?;
        
        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n  建议: {}", suggestion)?;
        }
        
        Ok(())
    }
}

/// 验证信息
#[derive(Debug, Clone)]
pub struct ValidationInfo {
    /// 信息代码
    pub code: String,
    /// 信息消息
    pub message: String,
    /// 信息字段路径
    pub field_path: String,
}

impl Display for ValidationInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", 
            self.code, 
            self.message,
            self.field_path
        )
    }
}

/// JIT引擎配置
#[derive(Debug, Clone)]
pub struct JITEngineConfig {
    /// 代码缓存大小限制（字节）
    pub cache_size_limit: usize,
    /// 热点检测阈值
    pub hotspot_threshold: u64,
    /// 冷点检测阈值
    pub cold_threshold: u64,
    /// 启用自适应优化
    pub enable_adaptive_optimization: bool,
    /// 启用SIMD优化
    pub enable_simd_optimization: bool,
    /// 启用多线程编译
    pub enable_multithreaded_compilation: bool,
    /// 编译线程数
    pub compilation_threads: usize,
    /// 优化级别
    pub optimization_level: OptimizationLevel,
    /// 寄存器分配器类型
    pub register_allocator_type: RegisterAllocatorType,
    /// 指令调度器类型
    pub instruction_scheduler_type: InstructionSchedulerType,
    /// 代码生成器类型
    pub code_generator_type: CodeGeneratorType,
    /// 启用调试模式
    pub enable_debug_mode: bool,
    /// 启用性能分析
    pub enable_profiling: bool,
    /// 启用详细日志
    pub enable_verbose_logging: bool,
    /// 自定义配置参数
    pub custom_parameters: HashMap<String, String>,
}

/// 优化级别
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// 无优化
    O0,
    /// 基本优化
    O1,
    /// 标准优化
    O2,
    /// 高级优化
    O3,
}

/// 寄存器分配器类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterAllocatorType {
    /// 线性扫描分配器
    LinearScan,
    /// 图着色分配器
    GraphColoring,
    /// 迭代合并分配器
    IterativeCoalescing,
}

/// 指令调度器类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionSchedulerType {
    /// 列表调度器
    List,
    /// 轨迹调度器
    Trace,
    /// 区域调度器
    Region,
}

/// 代码生成器类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeGeneratorType {
    /// x86-64代码生成器
    X86_64,
    /// ARM64代码生成器
    ARM64,
    /// RISC-V代码生成器
    RISCV64,
}

impl Default for JITEngineConfig {
    fn default() -> Self {
        Self {
            cache_size_limit: 64 * 1024 * 1024, // 64MB
            hotspot_threshold: 100,
            cold_threshold: 10,
            enable_adaptive_optimization: true,
            enable_simd_optimization: true,
            enable_multithreaded_compilation: true,
            compilation_threads: get_cpu_count(),
            optimization_level: OptimizationLevel::O2,
            register_allocator_type: RegisterAllocatorType::LinearScan,
            instruction_scheduler_type: InstructionSchedulerType::List,
            code_generator_type: CodeGeneratorType::X86_64,
            enable_debug_mode: false,
            enable_profiling: false,
            enable_verbose_logging: false,
            custom_parameters: HashMap::new(),
        }
    }
}

/// JIT引擎配置验证器
pub struct JITConfigValidator {
    /// 验证规则
    validation_rules: Vec<Box<dyn ValidationRule>>,
}

impl JITConfigValidator {
    /// 创建新的配置验证器
    pub fn new() -> Self {
        let mut validator = Self {
            validation_rules: Vec::new(),
        };
        
        // 添加默认验证规则
        validator.add_default_rules();
        validator
    }

    /// 添加默认验证规则
    fn add_default_rules(&mut self) {
        // 缓存大小验证
        self.validation_rules.push(Box::new(CacheSizeValidationRule));
        
        // 阈值验证
        self.validation_rules.push(Box::new(ThresholdValidationRule));
        
        // 线程数验证
        self.validation_rules.push(Box::new(ThreadCountValidationRule));
        
        // 优化级别验证
        self.validation_rules.push(Box::new(OptimizationLevelValidationRule));
        
        // 组件类型验证
        self.validation_rules.push(Box::new(ComponentTypeValidationRule));
        
        // 布尔值验证
        self.validation_rules.push(Box::new(BooleanValidationRule));
        
        // 自定义参数验证
        self.validation_rules.push(Box::new(CustomParameterValidationRule));
    }

    /// 添加验证规则
    pub fn add_rule(&mut self, rule: Box<dyn ValidationRule>) {
        self.validation_rules.push(rule);
    }

    /// 验证配置
    pub fn validate(&self, config: &JITEngineConfig) -> ValidationResult {
        let mut result = ValidationResult::success();
        
        for rule in &self.validation_rules {
            let rule_result = rule.validate(config);
            result.merge(rule_result);
        }
        
        result
    }

    /// 验证配置并返回错误（如果有）
    pub fn validate_with_error(&self, config: &JITEngineConfig) -> Result<(), Vec<ValidationError>> {
        let result = self.validate(config);
        if result.is_valid {
            Ok(())
        } else {
            Err(result.errors)
        }
    }
}

impl Default for JITConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// 验证规则接口
pub trait ValidationRule: Send + Sync {
    /// 验证配置
    fn validate(&self, config: &JITEngineConfig) -> ValidationResult;
    
    /// 获取规则名称
    fn name(&self) -> &str;
    
    /// 获取规则描述
    fn description(&self) -> &str;
}

/// 缓存大小验证规则
pub struct CacheSizeValidationRule;

impl ValidationRule for CacheSizeValidationRule {
    fn validate(&self, config: &JITEngineConfig) -> ValidationResult {
        let mut result = ValidationResult::success();
        
        // 检查最小缓存大小
        if config.cache_size_limit < 1024 * 1024 { // 1MB
            result.add_error(ValidationError {
                code: "CACHE_SIZE_TOO_SMALL".to_string(),
                message: format!("缓存大小 {} 字节太小，最小值为 1MB", config.cache_size_limit),
                field_path: "cache_size_limit".to_string(),
                severity: ErrorSeverity::Error,
                suggestion: Some("建议将缓存大小设置为至少 1MB".to_string()),
            });
        }
        
        // 检查最大缓存大小
        if config.cache_size_limit > 1024 * 1024 * 1024 { // 1GB
            result.add_warning(ValidationWarning {
                code: "CACHE_SIZE_TOO_LARGE".to_string(),
                message: format!("缓存大小 {} 字节可能过大，可能导致内存压力", config.cache_size_limit),
                field_path: "cache_size_limit".to_string(),
                suggestion: Some("建议考虑将缓存大小限制在 512MB 以内".to_string()),
            });
        }
        
        // 检查缓存大小是否为2的幂
        if !config.cache_size_limit.is_power_of_two() {
            result.add_warning(ValidationWarning {
                code: "CACHE_SIZE_NOT_POWER_OF_TWO".to_string(),
                message: "缓存大小不是2的幂，可能影响性能".to_string(),
                field_path: "cache_size_limit".to_string(),
                suggestion: Some("建议将缓存大小设置为2的幂，如 64MB、128MB、256MB".to_string()),
            });
        }
        
        result
    }
    
    fn name(&self) -> &str {
        "CacheSizeValidationRule"
    }
    
    fn description(&self) -> &str {
        "验证代码缓存大小设置"
    }
}

/// 阈值验证规则
pub struct ThresholdValidationRule;

impl ValidationRule for ThresholdValidationRule {
    fn validate(&self, config: &JITEngineConfig) -> ValidationResult {
        let mut result = ValidationResult::success();
        
        // 检查热点阈值
        if config.hotspot_threshold == 0 {
            result.add_error(ValidationError {
                code: "HOTSPOT_THRESHOLD_ZERO".to_string(),
                message: "热点阈值不能为0".to_string(),
                field_path: "hotspot_threshold".to_string(),
                severity: ErrorSeverity::Error,
                suggestion: Some("建议将热点阈值设置为至少10".to_string()),
            });
        }
        
        // 检查冷点阈值
        if config.cold_threshold == 0 {
            result.add_error(ValidationError {
                code: "COLD_THRESHOLD_ZERO".to_string(),
                message: "冷点阈值不能为0".to_string(),
                field_path: "cold_threshold".to_string(),
                severity: ErrorSeverity::Error,
                suggestion: Some("建议将冷点阈值设置为至少1".to_string()),
            });
        }
        
        // 检查阈值关系
        if config.hotspot_threshold <= config.cold_threshold {
            result.add_error(ValidationError {
                code: "INVALID_THRESHOLD_RELATION".to_string(),
                message: "热点阈值必须大于冷点阈值".to_string(),
                field_path: "hotspot_threshold, cold_threshold".to_string(),
                severity: ErrorSeverity::Error,
                suggestion: Some("建议将热点阈值设置为冷点阈值的5-10倍".to_string()),
            });
        }
        
        // 检查热点阈值是否过大
        if config.hotspot_threshold > 10000 {
            result.add_warning(ValidationWarning {
                code: "HOTSPOT_THRESHOLD_TOO_LARGE".to_string(),
                message: "热点阈值过大，可能导致优化不及时".to_string(),
                field_path: "hotspot_threshold".to_string(),
                suggestion: Some("建议将热点阈值设置在100-1000之间".to_string()),
            });
        }
        
        result
    }
    
    fn name(&self) -> &str {
        "ThresholdValidationRule"
    }
    
    fn description(&self) -> &str {
        "验证热点和冷点阈值设置"
    }
}

/// 线程数验证规则
pub struct ThreadCountValidationRule;

impl ValidationRule for ThreadCountValidationRule {
    fn validate(&self, config: &JITEngineConfig) -> ValidationResult {
        let mut result = ValidationResult::success();
        
        // 检查线程数是否为0
        if config.compilation_threads == 0 {
            result.add_error(ValidationError {
                code: "THREAD_COUNT_ZERO".to_string(),
                message: "编译线程数不能为0".to_string(),
                field_path: "compilation_threads".to_string(),
                severity: ErrorSeverity::Error,
                suggestion: Some("建议将编译线程数设置为至少1".to_string()),
            });
        }
        
        // 检查线程数是否过多
        let max_threads = get_cpu_count() * 4;
        if config.compilation_threads > max_threads {
            result.add_warning(ValidationWarning {
                code: "THREAD_COUNT_TOO_MANY".to_string(),
                message: format!("编译线程数 {} 过多，可能导致系统资源竞争", config.compilation_threads),
                field_path: "compilation_threads".to_string(),
                suggestion: Some(format!("建议将编译线程数限制在CPU核心数的2倍以内，最大不超过 {}", max_threads)),
            });
        }
        
        // 检查多线程设置一致性
        if config.enable_multithreaded_compilation && config.compilation_threads == 1 {
            result.add_warning(ValidationWarning {
                code: "SINGLE_THREAD_MULTITHREADING_ENABLED".to_string(),
                message: "启用了多线程编译但线程数为1".to_string(),
                field_path: "enable_multithreaded_compilation, compilation_threads".to_string(),
                suggestion: Some("建议增加编译线程数或禁用多线程编译".to_string()),
            });
        }
        
        if !config.enable_multithreaded_compilation && config.compilation_threads > 1 {
            result.add_warning(ValidationWarning {
                code: "MULTI_THREAD_MULTITHREADING_DISABLED".to_string(),
                message: "禁用了多线程编译但线程数大于1".to_string(),
                field_path: "enable_multithreaded_compilation, compilation_threads".to_string(),
                suggestion: Some("建议启用多线程编译或减少线程数到1".to_string()),
            });
        }
        
        result
    }
    
    fn name(&self) -> &str {
        "ThreadCountValidationRule"
    }
    
    fn description(&self) -> &str {
        "验证编译线程数设置"
    }
}

/// 优化级别验证规则
pub struct OptimizationLevelValidationRule;

impl ValidationRule for OptimizationLevelValidationRule {
    fn validate(&self, config: &JITEngineConfig) -> ValidationResult {
        let mut result = ValidationResult::success();
        
        match config.optimization_level {
            OptimizationLevel::O0 => {
                result.add_info(ValidationInfo {
                    code: "OPTIMIZATION_DISABLED".to_string(),
                    message: "优化已禁用，可能影响性能".to_string(),
                    field_path: "optimization_level".to_string(),
                });
            }
            OptimizationLevel::O3 => {
                result.add_warning(ValidationWarning {
                    code: "HIGH_OPTIMIZATION_LEVEL".to_string(),
                    message: "使用高级优化可能导致编译时间增加".to_string(),
                    field_path: "optimization_level".to_string(),
                    suggestion: Some("如果编译时间过长，考虑使用O2优化级别".to_string()),
                });
            }
            _ => {}
        }
        
        result
    }
    
    fn name(&self) -> &str {
        "OptimizationLevelValidationRule"
    }
    
    fn description(&self) -> &str {
        "验证优化级别设置"
    }
}

/// 组件类型验证规则
pub struct ComponentTypeValidationRule;

impl ValidationRule for ComponentTypeValidationRule {
    fn validate(&self, config: &JITEngineConfig) -> ValidationResult {
        let mut result = ValidationResult::success();
        
        // 检查组件类型兼容性
        match config.code_generator_type {
            CodeGeneratorType::ARM64 => {
                if config.register_allocator_type != RegisterAllocatorType::LinearScan {
                    result.add_warning(ValidationWarning {
                        code: "ARM64_ALLOCATOR_MISMATCH".to_string(),
                        message: "ARM64架构推荐使用线性扫描寄存器分配器".to_string(),
                        field_path: "register_allocator_type".to_string(),
                        suggestion: Some("建议将寄存器分配器类型设置为LinearScan".to_string()),
                    });
                }
            }
            CodeGeneratorType::RISCV64 => {
                if config.instruction_scheduler_type != InstructionSchedulerType::List {
                    result.add_warning(ValidationWarning {
                        code: "RISCV64_SCHEDULER_MISMATCH".to_string(),
                        message: "RISC-V架构推荐使用列表指令调度器".to_string(),
                        field_path: "instruction_scheduler_type".to_string(),
                        suggestion: Some("建议将指令调度器类型设置为List".to_string()),
                    });
                }
            }
            _ => {}
        }
        
        result
    }
    
    fn name(&self) -> &str {
        "ComponentTypeValidationRule"
    }
    
    fn description(&self) -> &str {
        "验证组件类型兼容性"
    }
}

/// 布尔值验证规则
pub struct BooleanValidationRule;

impl ValidationRule for BooleanValidationRule {
    fn validate(&self, config: &JITEngineConfig) -> ValidationResult {
        let mut result = ValidationResult::success();
        
        // 检查调试模式与性能分析的兼容性
        if config.enable_debug_mode && config.enable_profiling {
            result.add_warning(ValidationWarning {
                code: "DEBUG_PROFILING_CONFLICT".to_string(),
                message: "同时启用调试模式和性能分析可能影响性能".to_string(),
                field_path: "enable_debug_mode, enable_profiling".to_string(),
                suggestion: Some("建议在开发时启用调试模式，在生产环境启用性能分析".to_string()),
            });
        }
        
        // 检查详细日志设置
        if config.enable_verbose_logging && !config.enable_debug_mode {
            result.add_info(ValidationInfo {
                code: "VERBOSE_LOGGING_WITHOUT_DEBUG".to_string(),
                message: "启用详细日志但未启用调试模式".to_string(),
                field_path: "enable_verbose_logging".to_string(),
            });
        }
        
        result
    }
    
    fn name(&self) -> &str {
        "BooleanValidationRule"
    }
    
    fn description(&self) -> &str {
        "验证布尔值配置的合理性"
    }
}

/// 自定义参数验证规则
pub struct CustomParameterValidationRule;

impl ValidationRule for CustomParameterValidationRule {
    fn validate(&self, config: &JITEngineConfig) -> ValidationResult {
        let mut result = ValidationResult::success();
        
        for (key, value) in &config.custom_parameters {
            // 检查参数名格式
            if !key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                result.add_warning(ValidationWarning {
                    code: "INVALID_PARAMETER_NAME".to_string(),
                    message: format!("自定义参数名 '{}' 包含无效字符", key),
                    field_path: format!("custom_parameters.{}", key),
                    suggestion: Some("参数名应只包含字母、数字、下划线和连字符".to_string()),
                });
            }
            
            // 检查参数值长度
            if value.len() > 256 {
                result.add_warning(ValidationWarning {
                    code: "PARAMETER_VALUE_TOO_LONG".to_string(),
                    message: format!("自定义参数 '{}' 的值过长", key),
                    field_path: format!("custom_parameters.{}", key),
                    suggestion: Some("建议将参数值限制在256字符以内".to_string()),
                });
            }
        }
        
        // 检查自定义参数数量
        if config.custom_parameters.len() > 50 {
            result.add_warning(ValidationWarning {
                code: "TOO_MANY_CUSTOM_PARAMETERS".to_string(),
                message: format!("自定义参数数量 {} 过多", config.custom_parameters.len()),
                field_path: "custom_parameters".to_string(),
                suggestion: Some("建议将自定义参数数量限制在50以内".to_string()),
            });
        }
        
        result
    }
    
    fn name(&self) -> &str {
        "CustomParameterValidationRule"
    }
    
    fn description(&self) -> &str {
        "验证自定义参数设置"
    }
}

/// 配置验证器工厂
pub struct ConfigValidatorFactory;

impl ConfigValidatorFactory {
    /// 创建默认配置验证器
    pub fn create_default() -> JITConfigValidator {
        JITConfigValidator::new()
    }
    
    /// 创建严格配置验证器
    pub fn create_strict() -> JITConfigValidator {
        let mut validator = JITConfigValidator::new();
        
        // 添加更严格的验证规则
        validator.add_rule(Box::new(StrictValidationRule));
        
        validator
    }
    
    /// 创建性能优化配置验证器
    pub fn create_performance_focused() -> JITConfigValidator {
        let mut validator = JITConfigValidator::new();
        
        // 添加性能相关的验证规则
        validator.add_rule(Box::new(PerformanceValidationRule));
        
        validator
    }
}

/// 严格验证规则
pub struct StrictValidationRule;

impl ValidationRule for StrictValidationRule {
    fn validate(&self, config: &JITEngineConfig) -> ValidationResult {
        let mut result = ValidationResult::success();
        
        // 严格模式下的额外检查
        if config.cache_size_limit < 16 * 1024 * 1024 { // 16MB
            result.add_error(ValidationError {
                code: "STRICT_CACHE_SIZE_TOO_SMALL".to_string(),
                message: "严格模式下缓存大小至少需要16MB".to_string(),
                field_path: "cache_size_limit".to_string(),
                severity: ErrorSeverity::Error,
                suggestion: Some("建议将缓存大小设置为至少16MB".to_string()),
            });
        }
        
        if config.hotspot_threshold < 50 {
            result.add_error(ValidationError {
                code: "STRICT_HOTSPOT_THRESHOLD_TOO_LOW".to_string(),
                message: "严格模式下热点阈值至少需要50".to_string(),
                field_path: "hotspot_threshold".to_string(),
                severity: ErrorSeverity::Error,
                suggestion: Some("建议将热点阈值设置为至少50".to_string()),
            });
        }
        
        result
    }
    
    fn name(&self) -> &str {
        "StrictValidationRule"
    }
    
    fn description(&self) -> &str {
        "严格模式下的额外验证规则"
    }
}

/// 性能验证规则
pub struct PerformanceValidationRule;

impl ValidationRule for PerformanceValidationRule {
    fn validate(&self, config: &JITEngineConfig) -> ValidationResult {
        let mut result = ValidationResult::success();
        
        // 性能优化相关的检查
        if !config.enable_simd_optimization {
            result.add_warning(ValidationWarning {
                code: "SIMD_DISABLED_PERFORMANCE_IMPACT".to_string(),
                message: "禁用SIMD优化可能显著影响性能".to_string(),
                field_path: "enable_simd_optimization".to_string(),
                suggestion: Some("建议启用SIMD优化以提高性能".to_string()),
            });
        }
        
        if !config.enable_adaptive_optimization {
            result.add_warning(ValidationWarning {
                code: "ADAPTIVE_OPTIMIZATION_DISABLED_PERFORMANCE_IMPACT".to_string(),
                message: "禁用自适应优化可能影响长期性能".to_string(),
                field_path: "enable_adaptive_optimization".to_string(),
                suggestion: Some("建议启用自适应优化以提高长期性能".to_string()),
            });
        }
        
        if config.optimization_level == OptimizationLevel::O0 {
            result.add_warning(ValidationWarning {
                code: "NO_OPTIMIZATION_PERFORMANCE_IMPACT".to_string(),
                message: "禁用优化将严重影响性能".to_string(),
                field_path: "optimization_level".to_string(),
                suggestion: Some("建议至少启用O1优化级别".to_string()),
            });
        }
        
        result
    }
    
    fn name(&self) -> &str {
        "PerformanceValidationRule"
    }
    
    fn description(&self) -> &str {
        "性能优化相关的验证规则"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let config = JITEngineConfig::default();
        let validator = JITConfigValidator::new();
        let result = validator.validate(&config);
        
        assert!(result.is_valid);
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_invalid_cache_size() {
        let mut config = JITEngineConfig::default();
        config.cache_size_limit = 512 * 1024; // 512KB
        
        let validator = JITConfigValidator::new();
        let result = validator.validate(&config);
        
        assert!(!result.is_valid);
        assert!(result.error_count() > 0);
    }

    #[test]
    fn test_invalid_thresholds() {
        let mut config = JITEngineConfig::default();
        config.hotspot_threshold = 10;
        config.cold_threshold = 20; // 热点阈值小于冷点阈值
        
        let validator = JITConfigValidator::new();
        let result = validator.validate(&config);
        
        assert!(!result.is_valid);
        assert!(result.error_count() > 0);
    }

    #[test]
    fn test_validation_report_generation() {
        let mut config = JITEngineConfig::default();
        config.cache_size_limit = 512 * 1024; // 512KB
        
        let validator = JITConfigValidator::new();
        let result = validator.validate(&config);
        let report = result.generate_report();
        
        assert!(report.contains("JIT引擎配置验证报告"));
        assert!(report.contains("验证状态: ❌ 失败"));
        assert!(report.contains("错误"));
    }
}