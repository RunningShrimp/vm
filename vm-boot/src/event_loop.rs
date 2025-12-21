//! VM Boot事件循环接口
//!
//! 提供异步事件驱动的VM引导过程

use crate::runtime::{RuntimeController, RuntimeEvent, RuntimeEventListener};
use vm_accel::event::{AccelEvent, AccelEventSource};
use vm_accel::event_loop::{AsyncEventSource, EventLoop, EventLoopMessage, EventLoopState};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// VM运行时事件源
pub struct VmRuntimeEventSource<L: RuntimeEventListener> {
    controller: Arc<Mutex<RuntimeController>>,
    listener: Arc<Mutex<L>>,
    accel_source: Option<Box<dyn AsyncEventSource>>,
}

impl<L: RuntimeEventListener + Send + Sync> VmRuntimeEventSource<L> {
    pub fn new(
        controller: RuntimeController,
        listener: L,
        accel_source: Option<Box<dyn AsyncEventSource>>,
    ) -> Self {
        Self {
            controller: Arc::new(Mutex::new(controller)),
            listener: Arc::new(Mutex::new(listener)),
            accel_source,
        }
    }
}

impl<L: RuntimeEventListener + Send + Sync> AsyncEventSource for VmRuntimeEventSource<L> {
    fn wait_event(&self) -> Option<AccelEvent> {
        // 检查运行时事件
        if let Ok(mut controller) = self.controller.lock() {
            if let Some(event) = controller.poll_events() {
                // 将运行时事件转换为加速事件
                match event {
                    RuntimeEvent::Timer => Some(AccelEvent::Timer),
                    RuntimeEvent::Io => Some(AccelEvent::Io),
                    RuntimeEvent::Interrupt(id) => Some(AccelEvent::Interrupt(id)),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn poll_event(&self) -> Option<AccelEvent> {
        // 非阻塞版本
        self.wait_event()
    }

    fn name(&self) -> &str {
        "VM Runtime"
    }
}

/// VM引导事件循环
pub struct VmBootEventLoop<L: RuntimeEventListener + Send + Sync + 'static> {
    event_loop: EventLoop<VmRuntimeEventSource<L>>,
    runtime_controller: Arc<Mutex<RuntimeController>>,
}

impl<L: RuntimeEventListener + Send + Sync + 'static> VmBootEventLoop<L> {
    /// 创建VM引导事件循环
    pub fn new(
        controller: RuntimeController,
        listener: L,
        accel_source: Option<Box<dyn AsyncEventSource>>,
    ) -> Self {
        let runtime_source = VmRuntimeEventSource::new(
            controller,
            listener,
            accel_source,
        );

        let event_loop = EventLoop::new(runtime_source);
        let runtime_controller = Arc::new(Mutex::new(RuntimeController::default()));

        Self {
            event_loop,
            runtime_controller,
        }
    }

    /// 设置运行时事件处理器
    pub fn set_runtime_event_handler<F>(&mut self, handler: F)
    where
        F: Fn(RuntimeEvent) + Send + Sync + 'static,
    {
        let controller = Arc::clone(&self.runtime_controller);
        self.event_loop.set_event_handler(move |accel_event| {
            // 将加速事件转换为运行时事件
            let runtime_event = match accel_event {
                AccelEvent::Timer => RuntimeEvent::Timer,
                AccelEvent::Io => RuntimeEvent::Io,
                AccelEvent::Interrupt(id) => RuntimeEvent::Interrupt(id),
            };

            handler(runtime_event);

            // 更新运行时控制器状态
            if let Ok(mut ctrl) = controller.lock() {
                // 处理事件后的逻辑
                // 例如：检查是否需要重新调度、更新状态等
            }
        });
    }

    /// 启动引导过程
    pub fn start_boot(&mut self) -> Result<(), String> {
        self.event_loop.start()
    }

    /// 停止引导过程
    pub fn stop_boot(&mut self) -> Result<(), String> {
        self.event_loop.stop()
    }

    /// 获取当前状态
    pub fn state(&self) -> EventLoopState {
        self.event_loop.state()
    }

    /// 等待引导完成
    pub fn wait_for_completion(&mut self) {
        self.event_loop.wait();
    }

    /// 发送消息到事件循环
    pub fn send_message(&self, message: EventLoopMessage) -> Result<(), String> {
        // 注意：这里需要访问内部的Sender，但EventLoop没有公开它
        // 在实际实现中，可能需要修改EventLoop设计
        Err("Message sending not implemented".to_string())
    }
}

/// 引导阶段事件
#[derive(Debug, Clone)]
pub enum BootEvent {
    /// 初始化开始
    InitStart,
    /// 初始化完成
    InitComplete,
    /// 设备发现开始
    DeviceDiscoveryStart,
    /// 设备发现完成
    DeviceDiscoveryComplete,
    /// 内核加载开始
    KernelLoadStart,
    /// 内核加载完成
    KernelLoadComplete,
    /// 引导完成
    BootComplete,
    /// 引导失败
    BootFailed(String),
}

/// 引导事件监听器
pub trait BootEventListener: Send + Sync {
    /// 处理引导事件
    fn on_boot_event(&mut self, event: BootEvent);

    /// 检查引导是否完成
    fn is_boot_complete(&self) -> bool;
}

/// 标准引导事件监听器实现
pub struct StandardBootListener {
    boot_complete: Arc<Mutex<bool>>,
    events: Arc<Mutex<Vec<BootEvent>>>,
}

impl StandardBootListener {
    pub fn new() -> Self {
        Self {
            boot_complete: Arc::new(Mutex::new(false)),
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_events(&self) -> Vec<BootEvent> {
        self.events.lock().unwrap().clone()
    }

    pub fn is_complete(&self) -> bool {
        *self.boot_complete.lock().unwrap()
    }
}

impl BootEventListener for StandardBootListener {
    fn on_boot_event(&mut self, event: BootEvent) {
        self.events.lock().unwrap().push(event.clone());

        match event {
            BootEvent::BootComplete => {
                *self.boot_complete.lock().unwrap() = true;
            }
            BootEvent::BootFailed(_) => {
                *self.boot_complete.lock().unwrap() = true;
            }
            _ => {}
        }
    }

    fn is_boot_complete(&self) -> bool {
        self.is_complete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::RuntimeController;

    #[test]
    fn test_boot_event_loop_creation() {
        let controller = RuntimeController::default();
        let listener = StandardBootListener::new();
        let event_loop = VmBootEventLoop::new(controller, listener, None);

        assert_eq!(event_loop.state(), EventLoopState::Stopped);
    }

    #[test]
    fn test_boot_listener() {
        let mut listener = StandardBootListener::new();

        assert!(!listener.is_boot_complete());

        listener.on_boot_event(BootEvent::InitStart);
        listener.on_boot_event(BootEvent::BootComplete);

        assert!(listener.is_boot_complete());

        let events = listener.get_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], BootEvent::InitStart));
        assert!(matches!(events[1], BootEvent::BootComplete));
    }
}
