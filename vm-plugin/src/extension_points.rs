//! # 插件扩展点定义
//!
//! 定义了VM系统中可扩展的功能点，插件可以通过这些扩展点来扩展系统功能。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use vm_core::VmError;

/// 扩展点管理器
pub struct ExtensionPointManager {
    /// 注册的扩展点
    extension_points: Arc<RwLock<HashMap<String, Box<dyn ExtensionPoint>>>>,
    /// 扩展点统计
    stats: Arc<RwLock<ExtensionPointStats>>,
}

impl ExtensionPointManager {
    /// 创建新的扩展点管理器
    pub fn new() -> Self {
        Self {
            extension_points: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(ExtensionPointStats::default())),
        }
    }

    /// 注册扩展点
    pub fn register_extension_point(&mut self, extension_point: Box<dyn ExtensionPoint>) -> Result<(), VmError> {
        let name = extension_point.name().to_string();
        let mut points = self.extension_points.write().unwrap();
        
        if points.contains_key(&name) {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Extension point {} already registered", name),
                current: "registered".to_string(),
                expected: "not_registered".to_string(),
            }));
        }
        
        points.insert(name.clone(), extension_point);
        
        // 更新统计
        let mut stats = self.stats.write().unwrap();
        stats.registered_points += 1;
        stats.points_by_type.insert(extension_point.extension_type(), 1);
        
        tracing::info!("Registered extension point: {}", name);
        Ok(())
    }

    /// 获取扩展点
    pub fn get_extension_point(&self, name: &str) -> Option<Box<dyn ExtensionPoint>> {
        let points = self.extension_points.read().unwrap();
        // 注意：这里需要克隆，实际实现可能需要不同的方法
        points.get(name).map(|ep| ep.clone_box())
    }

    /// 调用扩展点
    pub async fn call_extension_point(
        &self,
        name: &str,
        context: &ExtensionContext,
    ) -> Result<ExtensionResult, VmError> {
        let points = self.extension_points.read().unwrap();
        if let Some(extension_point) = points.get(name) {
            // 更新调用统计
            {
                let mut stats = self.stats.write().unwrap();
                stats.total_calls += 1;
                *stats.calls_by_point.entry(name.to_string()).or_insert(0) += 1;
            }
            
            extension_point.execute(context).await
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Extension point {} not found", name),
                current: "not_found".to_string(),
                expected: "found".to_string(),
            }))
        }
    }

    /// 列出所有扩展点
    pub fn list_extension_points(&self) -> Vec<String> {
        let points = self.extension_points.read().unwrap();
        points.keys().cloned().collect()
    }

    /// 获取扩展点统计信息
    pub fn get_stats(&self) -> ExtensionPointStats {
        self.stats.read().unwrap().clone()
    }
}

/// 扩展点统计信息
#[derive(Debug, Clone, Default)]
pub struct ExtensionPointStats {
    /// 注册的扩展点数量
    pub registered_points: usize,
    /// 总调用次数
    pub total_calls: u64,
    /// 按类型统计的扩展点数量
    pub points_by_type: HashMap<String, usize>,
    /// 按扩展点统计的调用次数
    pub calls_by_point: HashMap<String, u64>,
}

/// 扩展点trait
pub trait ExtensionPoint: Send + Sync {
    /// 扩展点名称
    fn name(&self) -> &str;
    
    /// 扩展点类型
    fn extension_type(&self) -> String;
    
    /// 扩展点描述
    fn description(&self) -> &str;
    
    /// 执行扩展点
    async fn execute(&self, context: &ExtensionContext) -> Result<ExtensionResult, VmError>;
    
    /// 获取扩展点元数据
    fn metadata(&self) -> ExtensionPointMetadata;
    
    /// 克隆扩展点（用于返回）
    fn clone_box(&self) -> Box<dyn ExtensionPoint>;
}

/// 扩展点元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionPointMetadata {
    /// 扩展点名称
    pub name: String,
    /// 扩展点类型
    pub extension_type: String,
    /// 扩展点描述
    pub description: String,
    /// 扩展点版本
    pub version: String,
    /// 扩展点作者
    pub author: String,
    /// 支持的参数类型
    pub parameter_types: Vec<String>,
    /// 返回值类型
    pub return_type: String,
    /// 是否是必需的扩展点
    pub required: bool,
    /// 扩展点优先级
    pub priority: i32,
}

/// 扩展上下文
#[derive(Debug, Clone)]
pub struct ExtensionContext {
    /// 上下文ID
    pub id: String,
    /// 上下文类型
    pub context_type: String,
    /// 参数
    pub parameters: HashMap<String, ExtensionValue>,
    /// 调用者信息
    pub caller: Option<String>,
    /// 时间戳
    pub timestamp: std::time::SystemTime,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl ExtensionContext {
    /// 创建新的扩展上下文
    pub fn new(context_type: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            context_type,
            parameters: HashMap::new(),
            caller: None,
            timestamp: std::time::SystemTime::now(),
            metadata: HashMap::new(),
        }
    }

    /// 添加参数
    pub fn add_parameter(&mut self, key: String, value: ExtensionValue) {
        self.parameters.insert(key, value);
    }

    /// 获取参数
    pub fn get_parameter(&self, key: &str) -> Option<&ExtensionValue> {
        self.parameters.get(key)
    }

    /// 设置调用者
    pub fn set_caller(&mut self, caller: String) {
        self.caller = Some(caller);
    }

    /// 添加元数据
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }
}

/// 扩展值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExtensionValue {
    /// 布尔值
    Bool(bool),
    /// 整数
    Integer(i64),
    /// 浮点数
    Float(f64),
    /// 字符串
    String(String),
    /// 字节数组
    Bytes(Vec<u8>),
    /// 数组
    Array(Vec<ExtensionValue>),
    /// 对象
    Object(HashMap<String, ExtensionValue>),
    /// 空值
    Null,
}

impl ExtensionValue {
    /// 转换为布尔值
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ExtensionValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// 转换为整数
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            ExtensionValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// 转换为浮点数
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ExtensionValue::Float(f) => Some(*f),
            ExtensionValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// 转换为字符串
    pub fn as_string(&self) -> Option<&String> {
        match self {
            ExtensionValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// 转换为字节数组
    pub fn as_bytes(&self) -> Option<&Vec<u8>> {
        match self {
            ExtensionValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// 转换为数组
    pub fn as_array(&self) -> Option<&Vec<ExtensionValue>> {
        match self {
            ExtensionValue::Array(a) => Some(a),
            _ => None,
        }
    }

    /// 转换为对象
    pub fn as_object(&self) -> Option<&HashMap<String, ExtensionValue>> {
        match self {
            ExtensionValue::Object(o) => Some(o),
            _ => None,
        }
    }
}

/// 扩展结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionResult {
    /// 是否成功
    pub success: bool,
    /// 结果数据
    pub data: Option<ExtensionValue>,
    /// 错误信息
    pub error: Option<String>,
    /// 执行时间（纳秒）
    pub execution_time_ns: u64,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl ExtensionResult {
    /// 创建成功结果
    pub fn success(data: ExtensionValue) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            execution_time_ns: 0,
            metadata: HashMap::new(),
        }
    }

    /// 创建错误结果
    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            execution_time_ns: 0,
            metadata: HashMap::new(),
        }
    }

    /// 设置执行时间
    pub fn with_execution_time(mut self, time: std::time::Duration) -> Self {
        self.execution_time_ns = time.as_nanos() as u64;
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

// ============================================================================
// JIT编译器扩展点
// ============================================================================

/// JIT编译器扩展点
pub struct JitCompilerExtensionPoint {
    metadata: ExtensionPointMetadata,
}

impl JitCompilerExtensionPoint {
    /// 创建新的JIT编译器扩展点
    pub fn new() -> Self {
        Self {
            metadata: ExtensionPointMetadata {
                name: "jit.compiler".to_string(),
                extension_type: "jit".to_string(),
                description: "JIT编译器扩展点，用于自定义编译策略和优化".to_string(),
                version: "1.0.0".to_string(),
                author: "VM Team".to_string(),
                parameter_types: vec![
                    "method_info".to_string(),
                    "compilation_context".to_string(),
                    "optimization_level".to_string(),
                ],
                return_type: "compiled_method".to_string(),
                required: false,
                priority: 100,
            },
        }
    }
}

impl ExtensionPoint for JitCompilerExtensionPoint {
    fn name(&self) -> &str {
        &self.metadata.name
    }

    fn extension_type(&self) -> String {
        self.metadata.extension_type.clone()
    }

    fn description(&self) -> &str {
        &self.metadata.description
    }

    async fn execute(&self, context: &ExtensionContext) -> Result<ExtensionResult, VmError> {
        let start_time = std::time::Instant::now();
        
        // 获取参数
        let method_info = context.get_parameter("method_info");
        let compilation_context = context.get_parameter("compilation_context");
        let optimization_level = context.get_parameter("optimization_level");
        
        // 这里应该调用实际的JIT编译逻辑
        // 简化实现，返回模拟结果
        let result = ExtensionResult::success(ExtensionValue::String(
            "compiled_method_placeholder".to_string()
        ));
        
        Ok(result.with_execution_time(start_time.elapsed()))
    }

    fn metadata(&self) -> ExtensionPointMetadata {
        self.metadata.clone()
    }

    fn clone_box(&self) -> Box<dyn ExtensionPoint> {
        Box::new(Self::new())
    }
}

/// JIT策略扩展点
pub struct JitStrategyExtensionPoint {
    metadata: ExtensionPointMetadata,
}

impl JitStrategyExtensionPoint {
    /// 创建新的JIT策略扩展点
    pub fn new() -> Self {
        Self {
            metadata: ExtensionPointMetadata {
                name: "jit.strategy".to_string(),
                extension_type: "jit".to_string(),
                description: "JIT策略扩展点，用于自定义编译决策".to_string(),
                version: "1.0.0".to_string(),
                author: "VM Team".to_string(),
                parameter_types: vec![
                    "method_info".to_string(),
                    "execution_stats".to_string(),
                ],
                return_type: "compilation_decision".to_string(),
                required: false,
                priority: 90,
            },
        }
    }
}

impl ExtensionPoint for JitStrategyExtensionPoint {
    fn name(&self) -> &str {
        &self.metadata.name
    }

    fn extension_type(&self) -> String {
        self.metadata.extension_type.clone()
    }

    fn description(&self) -> &str {
        &self.metadata.description
    }

    async fn execute(&self, context: &ExtensionContext) -> Result<ExtensionResult, VmError> {
        let start_time = std::time::Instant::now();
        
        // 获取参数
        let method_info = context.get_parameter("method_info");
        let execution_stats = context.get_parameter("execution_stats");
        
        // 这里应该实现实际的JIT策略逻辑
        // 简化实现，返回模拟决策
        let result = ExtensionResult::success(ExtensionValue::Object(HashMap::from([
            ("should_compile".to_string(), ExtensionValue::Bool(true)),
            ("optimization_level".to_string(), ExtensionValue::Integer(2)),
        ])));
        
        Ok(result.with_execution_time(start_time.elapsed()))
    }

    fn metadata(&self) -> ExtensionPointMetadata {
        self.metadata.clone()
    }

    fn clone_box(&self) -> Box<dyn ExtensionPoint> {
        Box::new(Self::new())
    }
}

// ============================================================================
// GC策略扩展点
// ============================================================================

/// GC策略扩展点
pub struct GcStrategyExtensionPoint {
    metadata: ExtensionPointMetadata,
}

impl GcStrategyExtensionPoint {
    /// 创建新的GC策略扩展点
    pub fn new() -> Self {
        Self {
            metadata: ExtensionPointMetadata {
                name: "gc.strategy".to_string(),
                extension_type: "gc".to_string(),
                description: "GC策略扩展点，用于自定义垃圾回收策略".to_string(),
                version: "1.0.0".to_string(),
                author: "VM Team".to_string(),
                parameter_types: vec![
                    "heap_info".to_string(),
                    "gc_trigger".to_string(),
                ],
                return_type: "gc_decision".to_string(),
                required: false,
                priority: 80,
            },
        }
    }
}

impl ExtensionPoint for GcStrategyExtensionPoint {
    fn name(&self) -> &str {
        &self.metadata.name
    }

    fn extension_type(&self) -> String {
        self.metadata.extension_type.clone()
    }

    fn description(&self) -> &str {
        &self.metadata.description
    }

    async fn execute(&self, context: &ExtensionContext) -> Result<ExtensionResult, VmError> {
        let start_time = std::time::Instant::now();
        
        // 获取参数
        let heap_info = context.get_parameter("heap_info");
        let gc_trigger = context.get_parameter("gc_trigger");
        
        // 这里应该实现实际的GC策略逻辑
        // 简化实现，返回模拟决策
        let result = ExtensionResult::success(ExtensionValue::Object(HashMap::from([
            ("gc_algorithm".to_string(), ExtensionValue::String("mark_sweep".to_string())),
            ("gc_threshold".to_string(), ExtensionValue::Float(0.8)),
        ])));
        
        Ok(result.with_execution_time(start_time.elapsed()))
    }

    fn metadata(&self) -> ExtensionPointMetadata {
        self.metadata.clone()
    }

    fn clone_box(&self) -> Box<dyn ExtensionPoint> {
        Box::new(Self::new())
    }
}

// ============================================================================
// 事件处理扩展点
// ============================================================================

/// 事件处理扩展点
pub struct EventHandlerExtensionPoint {
    metadata: ExtensionPointMetadata,
}

impl EventHandlerExtensionPoint {
    /// 创建新的事件处理扩展点
    pub fn new() -> Self {
        Self {
            metadata: ExtensionPointMetadata {
                name: "event.handler".to_string(),
                extension_type: "event".to_string(),
                description: "事件处理扩展点，用于自定义事件处理逻辑".to_string(),
                version: "1.0.0".to_string(),
                author: "VM Team".to_string(),
                parameter_types: vec![
                    "event".to_string(),
                    "event_context".to_string(),
                ],
                return_type: "event_result".to_string(),
                required: false,
                priority: 70,
            },
        }
    }
}

impl ExtensionPoint for EventHandlerExtensionPoint {
    fn name(&self) -> &str {
        &self.metadata.name
    }

    fn extension_type(&self) -> String {
        self.metadata.extension_type.clone()
    }

    fn description(&self) -> &str {
        &self.metadata.description
    }

    async fn execute(&self, context: &ExtensionContext) -> Result<ExtensionResult, VmError> {
        let start_time = std::time::Instant::now();
        
        // 获取参数
        let event = context.get_parameter("event");
        let event_context = context.get_parameter("event_context");
        
        // 这里应该实现实际的事件处理逻辑
        // 简化实现，返回模拟结果
        let result = ExtensionResult::success(ExtensionValue::Object(HashMap::from([
            ("handled".to_string(), ExtensionValue::Bool(true)),
            ("processed".to_string(), ExtensionValue::Bool(true)),
        ])));
        
        Ok(result.with_execution_time(start_time.elapsed()))
    }

    fn metadata(&self) -> ExtensionPointMetadata {
        self.metadata.clone()
    }

    fn clone_box(&self) -> Box<dyn ExtensionPoint> {
        Box::new(Self::new())
    }
}

// ============================================================================
// 虚拟化加速器扩展点
// ============================================================================

/// 虚拟化加速器扩展点
pub struct VirtualizationAcceleratorExtensionPoint {
    metadata: ExtensionPointMetadata,
}

impl VirtualizationAcceleratorExtensionPoint {
    /// 创建新的虚拟化加速器扩展点
    pub fn new() -> Self {
        Self {
            metadata: ExtensionPointMetadata {
                name: "virtualization.accelerator".to_string(),
                extension_type: "virtualization".to_string(),
                description: "虚拟化加速器扩展点，用于自定义虚拟化加速策略".to_string(),
                version: "1.0.0".to_string(),
                author: "VM Team".to_string(),
                parameter_types: vec![
                    "acceleration_type".to_string(),
                    "hardware_info".to_string(),
                ],
                return_type: "acceleration_config".to_string(),
                required: false,
                priority: 60,
            },
        }
    }
}

impl ExtensionPoint for VirtualizationAcceleratorExtensionPoint {
    fn name(&self) -> &str {
        &self.metadata.name
    }

    fn extension_type(&self) -> String {
        self.metadata.extension_type.clone()
    }

    fn description(&self) -> &str {
        &self.metadata.description
    }

    async fn execute(&self, context: &ExtensionContext) -> Result<ExtensionResult, VmError> {
        let start_time = std::time::Instant::now();
        
        // 获取参数
        let acceleration_type = context.get_parameter("acceleration_type");
        let hardware_info = context.get_parameter("hardware_info");
        
        // 这里应该实现实际的虚拟化加速逻辑
        // 简化实现，返回模拟配置
        let result = ExtensionResult::success(ExtensionValue::Object(HashMap::from([
            ("accelerator".to_string(), ExtensionValue::String("kvm".to_string())),
            ("enabled".to_string(), ExtensionValue::Bool(true)),
            ("features".to_string(), ExtensionValue::Array(vec![
                ExtensionValue::String("nested_virtualization".to_string()),
                ExtensionValue::String("huge_pages".to_string()),
            ])),
        ])));
        
        Ok(result.with_execution_time(start_time.elapsed()))
    }

    fn metadata(&self) -> ExtensionPointMetadata {
        self.metadata.clone()
    }

    fn clone_box(&self) -> Box<dyn ExtensionPoint> {
        Box::new(Self::new())
    }
}

// ============================================================================
// 跨架构支持扩展点
// ============================================================================

/// 跨架构支持扩展点
pub struct CrossArchitectureExtensionPoint {
    metadata: ExtensionPointMetadata,
}

impl CrossArchitectureExtensionPoint {
    /// 创建新的跨架构支持扩展点
    pub fn new() -> Self {
        Self {
            metadata: ExtensionPointMetadata {
                name: "cross.architecture".to_string(),
                extension_type: "translation".to_string(),
                description: "跨架构支持扩展点，用于自定义指令集转换".to_string(),
                version: "1.0.0".to_string(),
                author: "VM Team".to_string(),
                parameter_types: vec![
                    "source_arch".to_string(),
                    "target_arch".to_string(),
                    "instruction".to_string(),
                ],
                return_type: "translated_instruction".to_string(),
                required: false,
                priority: 50,
            },
        }
    }
}

impl ExtensionPoint for CrossArchitectureExtensionPoint {
    fn name(&self) -> &str {
        &self.metadata.name
    }

    fn extension_type(&self) -> String {
        self.metadata.extension_type.clone()
    }

    fn description(&self) -> &str {
        &self.metadata.description
    }

    async fn execute(&self, context: &ExtensionContext) -> Result<ExtensionResult, VmError> {
        let start_time = std::time::Instant::now();
        
        // 获取参数
        let source_arch = context.get_parameter("source_arch");
        let target_arch = context.get_parameter("target_arch");
        let instruction = context.get_parameter("instruction");
        
        // 这里应该实现实际的指令转换逻辑
        // 简化实现，返回模拟结果
        let result = ExtensionResult::success(ExtensionValue::Object(HashMap::from([
            ("translated".to_string(), ExtensionValue::Bool(true)),
            ("instruction_bytes".to_string(), ExtensionValue::Bytes(vec![0x90, 0x90, 0x90])),
            ("cycles".to_string(), ExtensionValue::Integer(5)),
        ])));
        
        Ok(result.with_execution_time(start_time.elapsed()))
    }

    fn metadata(&self) -> ExtensionPointMetadata {
        self.metadata.clone()
    }

    fn clone_box(&self) -> Box<dyn ExtensionPoint> {
        Box::new(Self::new())
    }
}

/// 注册所有默认扩展点
pub fn register_default_extension_points(manager: &mut ExtensionPointManager) -> Result<(), VmError> {
    // JIT编译器扩展点
    manager.register_extension_point(Box::new(JitCompilerExtensionPoint::new()))?;
    manager.register_extension_point(Box::new(JitStrategyExtensionPoint::new()))?;
    
    // GC策略扩展点
    manager.register_extension_point(Box::new(GcStrategyExtensionPoint::new()))?;
    
    // 事件处理扩展点
    manager.register_extension_point(Box::new(EventHandlerExtensionPoint::new()))?;
    
    // 虚拟化加速器扩展点
    manager.register_extension_point(Box::new(VirtualizationAcceleratorExtensionPoint::new()))?;
    
    // 跨架构支持扩展点
    manager.register_extension_point(Box::new(CrossArchitectureExtensionPoint::new()))?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extension_point_manager() {
        let mut manager = ExtensionPointManager::new();
        
        // 注册扩展点
        let jit_ep = JitCompilerExtensionPoint::new();
        manager.register_extension_point(Box::new(jit_ep)).unwrap();
        
        // 列出扩展点
        let points = manager.list_extension_points();
        assert!(points.contains(&"jit.compiler".to_string()));
        
        // 获取统计信息
        let stats = manager.get_stats();
        assert_eq!(stats.registered_points, 1);
    }

    #[tokio::test]
    async fn test_extension_point_execution() {
        let mut manager = ExtensionPointManager::new();
        
        // 注册扩展点
        let jit_ep = JitCompilerExtensionPoint::new();
        manager.register_extension_point(Box::new(jit_ep)).unwrap();
        
        // 创建上下文
        let mut context = ExtensionContext::new("jit_compilation".to_string());
        context.add_parameter("method_info".to_string(), ExtensionValue::String("test_method".to_string()));
        context.add_parameter("optimization_level".to_string(), ExtensionValue::Integer(2));
        
        // 调用扩展点
        let result = manager.call_extension_point("jit.compiler", &context).await.unwrap();
        assert!(result.success);
        
        // 检查统计信息
        let stats = manager.get_stats();
        assert_eq!(stats.total_calls, 1);
        assert_eq!(stats.calls_by_point.get("jit.compiler"), Some(&1));
    }

    #[test]
    fn test_extension_value_conversions() {
        let bool_val = ExtensionValue::Bool(true);
        assert_eq!(bool_val.as_bool(), Some(true));
        
        let int_val = ExtensionValue::Integer(42);
        assert_eq!(int_val.as_integer(), Some(42));
        assert_eq!(int_val.as_float(), Some(42.0));
        
        let str_val = ExtensionValue::String("test".to_string());
        assert_eq!(str_val.as_string(), Some(&"test".to_string()));
        
        let bytes_val = ExtensionValue::Bytes(vec![1, 2, 3]);
        assert_eq!(bytes_val.as_bytes(), Some(&vec![1, 2, 3]));
    }

    #[test]
    fn test_extension_result() {
        let success_result = ExtensionResult::success(ExtensionValue::Bool(true));
        assert!(success_result.success);
        assert!(success_result.error.is_none());
        
        let error_result = ExtensionResult::error("test error".to_string());
        assert!(!error_result.success);
        assert_eq!(error_result.error, Some("test error".to_string()));
    }
}