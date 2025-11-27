//! 虚拟机运行时控制
//!
//! 支持暂停、恢复、安全关闭等操作

use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::sync::mpsc::{channel, Sender, Receiver};

/// 运行时控制命令
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCommand {
    /// 暂停虚拟机
    Pause,
    /// 恢复虚拟机
    Resume,
    /// 安全关闭虚拟机
    Shutdown,
    /// 强制停止虚拟机
    Stop,
    /// 重置虚拟机
    Reset,
    /// 保存快照
    SaveSnapshot,
    /// 恢复快照
    LoadSnapshot,
}

/// 运行时状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    /// 未启动
    Stopped,
    /// 正在运行
    Running,
    /// 已暂停
    Paused,
    /// 正在关闭
    ShuttingDown,
}

/// 运行时控制器
pub struct RuntimeController {
    /// 当前状态
    state: Arc<Mutex<RuntimeState>>,
    /// 命令发送器
    cmd_tx: Sender<RuntimeCommand>,
    /// 命令接收器
    cmd_rx: Arc<Mutex<Receiver<RuntimeCommand>>>,
    /// 运行标志
    running: Arc<AtomicBool>,
    /// 暂停标志
    paused: Arc<AtomicBool>,
}

impl RuntimeController {
    /// 创建新的运行时控制器
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = channel();
        
        Self {
            state: Arc::new(Mutex::new(RuntimeState::Stopped)),
            cmd_tx,
            cmd_rx: Arc::new(Mutex::new(cmd_rx)),
            running: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 获取当前状态
    pub fn state(&self) -> RuntimeState {
        *self.state.lock().unwrap()
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// 检查是否已暂停
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    /// 发送命令
    pub fn send_command(&self, cmd: RuntimeCommand) -> Result<(), String> {
        self.cmd_tx.send(cmd)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    /// 暂停虚拟机
    pub fn pause(&self) -> Result<(), String> {
        let current_state = self.state();
        if current_state != RuntimeState::Running {
            return Err(format!("Cannot pause VM in state {:?}", current_state));
        }

        self.send_command(RuntimeCommand::Pause)?;
        log::info!("Pause command sent");
        Ok(())
    }

    /// 恢复虚拟机
    pub fn resume(&self) -> Result<(), String> {
        let current_state = self.state();
        if current_state != RuntimeState::Paused {
            return Err(format!("Cannot resume VM in state {:?}", current_state));
        }

        self.send_command(RuntimeCommand::Resume)?;
        log::info!("Resume command sent");
        Ok(())
    }

    /// 安全关闭虚拟机
    pub fn shutdown(&self) -> Result<(), String> {
        self.send_command(RuntimeCommand::Shutdown)?;
        log::info!("Shutdown command sent");
        Ok(())
    }

    /// 强制停止虚拟机
    pub fn stop(&self) -> Result<(), String> {
        self.send_command(RuntimeCommand::Stop)?;
        log::info!("Stop command sent");
        Ok(())
    }

    /// 重置虚拟机
    pub fn reset(&self) -> Result<(), String> {
        self.send_command(RuntimeCommand::Reset)?;
        log::info!("Reset command sent");
        Ok(())
    }

    /// 处理命令（在主循环中调用）
    pub fn process_commands(&self) -> Option<RuntimeCommand> {
        let rx = self.cmd_rx.lock().unwrap();
        rx.try_recv().ok()
    }

    /// 轮询事件：将命令转化为事件并执行状态更新
    pub fn poll_events(&self) -> Option<RuntimeEvent> {
        if let Some(cmd) = self.process_commands() {
            match cmd {
                RuntimeCommand::Pause => {
                    self.update_state(RuntimeState::Paused);
                    Some(RuntimeEvent::Paused)
                }
                RuntimeCommand::Resume => {
                    self.update_state(RuntimeState::Running);
                    Some(RuntimeEvent::Resumed)
                }
                RuntimeCommand::Shutdown => {
                    self.update_state(RuntimeState::ShuttingDown);
                    Some(RuntimeEvent::ShuttingDown)
                }
                RuntimeCommand::Stop => {
                    self.update_state(RuntimeState::Stopped);
                    Some(RuntimeEvent::Stopped)
                }
                RuntimeCommand::Reset => {
                    self.update_state(RuntimeState::Running);
                    Some(RuntimeEvent::Reset)
                }
                RuntimeCommand::SaveSnapshot => Some(RuntimeEvent::SnapshotSaved("default".into())),
                RuntimeCommand::LoadSnapshot => Some(RuntimeEvent::SnapshotLoaded("default".into())),
            }
        } else {
            None
        }
    }

    /// 更新状态
    pub fn update_state(&self, new_state: RuntimeState) {
        let mut state = self.state.lock().unwrap();
        log::info!("State transition: {:?} -> {:?}", *state, new_state);
        *state = new_state;

        // 更新标志
        match new_state {
            RuntimeState::Running => {
                self.running.store(true, Ordering::Release);
                self.paused.store(false, Ordering::Release);
            }
            RuntimeState::Paused => {
                self.paused.store(true, Ordering::Release);
            }
            RuntimeState::Stopped | RuntimeState::ShuttingDown => {
                self.running.store(false, Ordering::Release);
                self.paused.store(false, Ordering::Release);
            }
        }
    }

    /// 启动虚拟机
    pub fn start(&self) -> Result<(), String> {
        let current_state = self.state();
        if current_state != RuntimeState::Stopped {
            return Err(format!("Cannot start VM in state {:?}", current_state));
        }

        self.update_state(RuntimeState::Running);
        log::info!("VM started");
        Ok(())
    }

    /// 获取运行标志的克隆（用于传递给其他线程）
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// 获取暂停标志的克隆（用于传递给其他线程）
    pub fn paused_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.paused)
    }
}

impl Default for RuntimeController {
    fn default() -> Self {
        Self::new()
    }
}

/// 运行时事件
#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    /// 虚拟机已启动
    Started,
    /// 虚拟机已暂停
    Paused,
    /// 虚拟机已恢复
    Resumed,
    /// 虚拟机正在关闭
    ShuttingDown,
    /// 虚拟机已停止
    Stopped,
    /// 虚拟机已重置
    Reset,
    /// 快照已保存
    SnapshotSaved(String),
    /// 快照已加载
    SnapshotLoaded(String),
    /// 错误
    Error(String),
}

/// 事件监听器
pub trait RuntimeEventListener: Send {
    /// 处理事件
    fn on_event(&mut self, event: RuntimeEvent);
}

/// 简单的日志事件监听器
pub struct LogEventListener;

impl RuntimeEventListener for LogEventListener {
    fn on_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Started => log::info!("VM started"),
            RuntimeEvent::Paused => log::info!("VM paused"),
            RuntimeEvent::Resumed => log::info!("VM resumed"),
            RuntimeEvent::ShuttingDown => log::info!("VM shutting down"),
            RuntimeEvent::Stopped => log::info!("VM stopped"),
            RuntimeEvent::Reset => log::info!("VM reset"),
            RuntimeEvent::SnapshotSaved(name) => log::info!("Snapshot saved: {}", name),
            RuntimeEvent::SnapshotLoaded(name) => log::info!("Snapshot loaded: {}", name),
            RuntimeEvent::Error(msg) => log::error!("Runtime error: {}", msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_controller() {
        let controller = RuntimeController::new();
        assert_eq!(controller.state(), RuntimeState::Stopped);
        assert!(!controller.is_running());
        assert!(!controller.is_paused());

        controller.start().unwrap();
        assert_eq!(controller.state(), RuntimeState::Running);
        assert!(controller.is_running());

        controller.pause().unwrap();
        let cmd = controller.process_commands().unwrap();
        assert_eq!(cmd, RuntimeCommand::Pause);
    }

    #[test]
    fn test_state_transitions() {
        let controller = RuntimeController::new();
        
        assert!(controller.pause().is_err()); // 不能从 Stopped 暂停
        
        controller.start().unwrap();
        assert!(controller.pause().is_ok());
        
        controller.update_state(RuntimeState::Paused);
        assert!(controller.resume().is_ok());
    }
}
