//! 核心trait定义

use crate::{ComponentStatus, SubscriptionId, VmError};
use serde::{Deserialize, Serialize};

/// VM组件基础trait，定义生命周期管理
pub trait VmComponent {
    type Config;
    type Error;

    /// 初始化组件
    fn init(config: Self::Config) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// 启动组件
    fn start(&mut self) -> Result<(), Self::Error>;

    /// 停止组件
    fn stop(&mut self) -> Result<(), Self::Error>;

    /// 获取组件状态
    fn status(&self) -> ComponentStatus;

    /// 获取组件名称
    fn name(&self) -> &str;
}

/// 配置管理trait
pub trait Configurable {
    type Config;

    /// 更新配置
    fn update_config(&mut self, config: &Self::Config) -> Result<(), VmError>;

    /// 获取当前配置
    fn get_config(&self) -> &Self::Config;

    /// 验证配置
    fn validate_config(config: &Self::Config) -> Result<(), VmError>;
}

/// 状态观察trait
pub trait Observable {
    type State;
    type Event;

    /// 获取当前状态
    fn get_state(&self) -> &Self::State;

    /// 订阅状态变化
    #[allow(clippy::type_complexity)]
    fn subscribe(
        &mut self,
        callback: Box<dyn Fn(&Self::State, &Self::Event) + Send + Sync>,
    ) -> SubscriptionId;

    /// 取消订阅
    fn unsubscribe(&mut self, id: SubscriptionId) -> Result<(), VmError>;
}

/// 基础组件状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentState {
    pub name: String,
    pub status: ComponentStatus,
    pub start_time: Option<std::time::SystemTime>,
    pub last_error: Option<String>,
}

impl Default for ComponentState {
    fn default() -> Self {
        Self {
            name: String::new(),
            status: ComponentStatus::Uninitialized,
            start_time: None,
            last_error: None,
        }
    }
}

/// 组件管理器
pub struct ComponentManager {
    components: std::collections::HashMap<
        String,
        Box<dyn VmComponent<Config = serde_json::Value, Error = VmError>>,
    >,
}

impl Default for ComponentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentManager {
    pub fn new() -> Self {
        Self {
            components: std::collections::HashMap::new(),
        }
    }

    pub fn register_component<C>(&mut self, name: String, component: C)
    where
        C: VmComponent<Config = serde_json::Value, Error = VmError> + 'static,
    {
        self.components.insert(name, Box::new(component));
    }

    pub fn get_component(
        &self,
        name: &str,
    ) -> Option<&dyn VmComponent<Config = serde_json::Value, Error = VmError>> {
        self.components.get(name).map(|v| &**v)
    }

    pub fn get_component_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut Box<dyn VmComponent<Config = serde_json::Value, Error = VmError>>> {
        self.components.get_mut(name)
    }

    pub fn list_components(&self) -> Vec<&str> {
        self.components.keys().map(|s| s.as_str()).collect()
    }

    pub fn start_all(&mut self) -> Result<(), VmError> {
        for (name, component) in &mut self.components {
            component.start().map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to start component '{}': {:?}", name, e),
                    module: "ComponentManager".to_string(),
                })
            })?;
        }
        Ok(())
    }

    pub fn stop_all(&mut self) -> Result<(), VmError> {
        for (name, component) in &mut self.components {
            component.stop().map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to stop component '{}': {:?}", name, e),
                    module: "ComponentManager".to_string(),
                })
            })?;
        }
        Ok(())
    }
}
