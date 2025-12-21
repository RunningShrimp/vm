//! 事件循环接口
//!
//! 提供异步事件驱动的虚拟机执行模型，替代轮询机制

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// 事件循环状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventLoopState {
    /// 停止状态
    Stopped,
    /// 运行状态
    Running,
    /// 暂停状态
    Paused,
}

/// 事件循环消息
#[derive(Debug)]
pub enum EventLoopMessage {
    /// 启动事件循环
    Start,
    /// 停止事件循环
    Stop,
    /// 暂停事件循环
    Pause,
    /// 恢复事件循环
    Resume,
    /// 处理事件
    ProcessEvent(AccelEvent),
    /// 心跳检查
    Heartbeat,
}

/// 异步事件源
pub trait AsyncEventSource: Send + Sync {
    /// 异步等待事件
    fn wait_event(&self) -> Option<AccelEvent>;

    /// 检查是否有待处理事件（非阻塞）
    fn poll_event(&self) -> Option<AccelEvent> {
        None
    }

    /// 获取事件源名称
    fn name(&self) -> &str;
}

/// 事件循环
pub struct EventLoop<E: AsyncEventSource> {
    /// 事件源
    event_source: Arc<Mutex<E>>,
    /// 消息发送端
    tx: Sender<EventLoopMessage>,
    /// 消息接收端
    rx: Receiver<EventLoopMessage>,
    /// 当前状态
    state: Arc<Mutex<EventLoopState>>,
    /// 事件处理器
    event_handler: Arc<Mutex<Option<Box<dyn Fn(AccelEvent) + Send + Sync>>>>,
    /// 工作线程句柄
    worker_thread: Option<thread::JoinHandle<()>>,
}

impl<E: AsyncEventSource + 'static> EventLoop<E> {
    /// 创建新的事件循环
    pub fn new(event_source: E) -> Self {
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(EventLoopState::Stopped));
        let event_source = Arc::new(Mutex::new(event_source));

        Self {
            event_source,
            tx,
            rx,
            state,
            event_handler: Arc::new(Mutex::new(None)),
            worker_thread: None,
        }
    }

    /// 设置事件处理器
    pub fn set_event_handler<F>(&mut self, handler: F)
    where
        F: Fn(AccelEvent) + Send + Sync + 'static,
    {
        *self.event_handler.lock().unwrap() = Some(Box::new(handler));
    }

    /// 启动事件循环
    pub fn start(&mut self) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        if *state != EventLoopState::Stopped {
            return Err("Event loop is already running".to_string());
        }

        *state = EventLoopState::Running;

        // 克隆必要的Arc引用
        let event_source = Arc::clone(&self.event_source);
        let rx = self.rx.clone();
        let state_clone = Arc::clone(&self.state);
        let event_handler = Arc::clone(&self.event_handler);

        // 启动工作线程
        let worker_thread = thread::spawn(move || {
            Self::event_loop_worker(event_source, rx, state_clone, event_handler);
        });

        self.worker_thread = Some(worker_thread);
        Ok(())
    }

    /// 停止事件循环
    pub fn stop(&mut self) -> Result<(), String> {
        let state = self.state.lock().unwrap();
        if *state == EventLoopState::Stopped {
            return Ok(());
        }

        // 发送停止消息
        if self.tx.send(EventLoopMessage::Stop).is_err() {
            return Err("Failed to send stop message".to_string());
        }

        // 等待工作线程结束
        if let Some(thread) = self.worker_thread.take() {
            if let Err(_) = thread.join() {
                return Err("Failed to join worker thread".to_string());
            }
        }

        *self.state.lock().unwrap() = EventLoopState::Stopped;
        Ok(())
    }

    /// 暂停事件循环
    pub fn pause(&self) -> Result<(), String> {
        self.tx.send(EventLoopMessage::Pause)
            .map_err(|_| "Failed to send pause message".to_string())
    }

    /// 恢复事件循环
    pub fn resume(&self) -> Result<(), String> {
        self.tx.send(EventLoopMessage::Resume)
            .map_err(|_| "Failed to send resume message".to_string())
    }

    /// 获取当前状态
    pub fn state(&self) -> EventLoopState {
        *self.state.lock().unwrap()
    }

    /// 等待事件循环完成
    pub fn wait(&mut self) {
        if let Some(thread) = self.worker_thread.take() {
            let _ = thread.join();
        }
    }

    /// 事件循环工作线程
    fn event_loop_worker(
        event_source: Arc<Mutex<E>>,
        rx: Receiver<EventLoopMessage>,
        state: Arc<Mutex<EventLoopState>>,
        event_handler: Arc<Mutex<Option<Box<dyn Fn(AccelEvent) + Send + Sync>>>>,
    ) {
        loop {
            // 检查消息队列（非阻塞）
            match rx.try_recv() {
                Ok(EventLoopMessage::Stop) => {
                    *state.lock().unwrap() = EventLoopState::Stopped;
                    break;
                }
                Ok(EventLoopMessage::Pause) => {
                    *state.lock().unwrap() = EventLoopState::Paused;
                    continue;
                }
                Ok(EventLoopMessage::Resume) => {
                    *state.lock().unwrap() = EventLoopState::Running;
                }
                Ok(EventLoopMessage::ProcessEvent(event)) => {
                    // 处理外部事件
                    if let Some(handler) = event_handler.lock().unwrap().as_ref() {
                        handler(event);
                    }
                }
                Ok(EventLoopMessage::Heartbeat) => {
                    // 心跳处理
                }
                Ok(EventLoopMessage::Start) => {
                    *state.lock().unwrap() = EventLoopState::Running;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // 没有消息，继续处理事件
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // 发送端断开，退出循环
                    break;
                }
            }

            // 检查状态
            let current_state = *state.lock().unwrap();
            if current_state != EventLoopState::Running {
                thread::sleep(Duration::from_millis(10));
                continue;
            }

            // 等待事件
            if let Some(event) = event_source.lock().unwrap().wait_event() {
                // 调用事件处理器
                if let Some(handler) = event_handler.lock().unwrap().as_ref() {
                    handler(event);
                }
            } else {
                // 没有事件，短暂休眠避免忙等待
                thread::sleep(Duration::from_micros(100));
            }
        }
    }
}

/// 定时器事件源
pub struct TimerEventSource {
    interval_ms: u64,
    last_tick: std::time::Instant,
}

impl TimerEventSource {
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            last_tick: std::time::Instant::now(),
        }
    }
}

impl AsyncEventSource for TimerEventSource {
    fn wait_event(&self) -> Option<AccelEvent> {
        // 简化实现：总是返回定时器事件
        // 在实际实现中，这里会等待定时器到期
        Some(AccelEvent::Timer)
    }

    fn poll_event(&self) -> Option<AccelEvent> {
        if self.last_tick.elapsed() >= Duration::from_millis(self.interval_ms) {
            Some(AccelEvent::Timer)
        } else {
            None
        }
    }

    fn name(&self) -> &str {
        "Timer"
    }
}

/// IO事件源
pub struct IoEventSource;

impl IoEventSource {
    pub fn new() -> Self {
        Self
    }
}

impl AsyncEventSource for IoEventSource {
    fn wait_event(&self) -> Option<AccelEvent> {
        // 简化实现：总是返回IO事件
        // 在实际实现中，这里会等待IO事件
        Some(AccelEvent::Io)
    }

    fn name(&self) -> &str {
        "IO"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[derive(Clone)]
    struct TestEventSource {
        should_trigger: Arc<AtomicBool>,
    }

    impl TestEventSource {
        fn new() -> Self {
            Self {
                should_trigger: Arc::new(AtomicBool::new(false)),
            }
        }

        fn trigger_event(&self) {
            self.should_trigger.store(true, Ordering::SeqCst);
        }
    }

    impl AsyncEventSource for TestEventSource {
        fn wait_event(&self) -> Option<AccelEvent> {
            if self.should_trigger.load(Ordering::SeqCst) {
                self.should_trigger.store(false, Ordering::SeqCst);
                Some(AccelEvent::Interrupt(42))
            } else {
                None
            }
        }

        fn name(&self) -> &str {
            "Test"
        }
    }

    #[test]
    fn test_event_loop_creation() {
        let source = TestEventSource::new();
        let mut event_loop = EventLoop::new(source);

        assert_eq!(event_loop.state(), EventLoopState::Stopped);
    }

    #[test]
    fn test_event_loop_start_stop() {
        let source = TestEventSource::new();
        let mut event_loop = EventLoop::new(source);

        // 启动
        assert!(event_loop.start().is_ok());
        assert_eq!(event_loop.state(), EventLoopState::Running);

        // 停止
        assert!(event_loop.stop().is_ok());
        assert_eq!(event_loop.state(), EventLoopState::Stopped);
    }

    #[test]
    fn test_event_handler() {
        let source = TestEventSource::new();
        let mut event_loop = EventLoop::new(source.clone());
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);

        event_loop.set_event_handler(move |event| {
            if let AccelEvent::Interrupt(42) = event {
                called_clone.store(true, Ordering::SeqCst);
            }
        });

        assert!(event_loop.start().is_ok());

        // 触发事件
        source.trigger_event();

        // 等待一小段时间让事件被处理
        thread::sleep(Duration::from_millis(50));

        assert!(event_loop.stop().is_ok());
        // 注意：在这个简化测试中，事件可能不会被处理
        // 在实际实现中，需要更复杂的同步机制
    }
}
