//! 运行时服务模块 - DDD贫血模型
//!
//! 将VM引导逻辑拆分为服务层和实体层

pub mod event_handler;
pub mod runtime_controller;

// 重新导出主要类型
pub use event_handler::{EventHandlerService, FilteredEventListener, StandardEventListener};
pub use runtime_controller::{
    DeviceInfo, MemoryLayout, RuntimeControllerEntity, RuntimeControllerService, RuntimeStats,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{RuntimeEvent, RuntimeState};

    #[test]
    fn test_runtime_service_integration() {
        // 创建控制器服务
        let controller = RuntimeControllerService::new(1024 * 1024 * 1024, 64 * 1024 * 1024);

        // 创建事件处理器服务
        let event_handler: EventHandlerService<StandardEventListener> = EventHandlerService::new();

        // 注册事件监听器
        let listener = StandardEventListener::new();
        event_handler
            .register_listener("main".to_string(), listener.clone())
            .unwrap();

        // 启动运行时
        controller.set_state(RuntimeState::Running);
        event_handler.process_event(RuntimeEvent::Started).unwrap();

        // 验证状态
        assert!(matches!(controller.get_state(), RuntimeState::Running));
        assert_eq!(listener.event_count(), 1);

        // 停止运行时
        controller.set_state(RuntimeState::Stopped);
        event_handler.process_event(RuntimeEvent::Stopped).unwrap();

        assert!(matches!(controller.get_state(), RuntimeState::Stopped));
        assert_eq!(listener.event_count(), 2);
    }
}
