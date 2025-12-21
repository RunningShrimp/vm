//! 信号处理兼容层
//!
//! 提供跨架构信号处理兼容性，包括：
//! - 信号号映射（不同架构的信号号可能不同）
//! - 信号处理函数注册和调用
//! - 信号掩码管理
//! - 信号栈管理

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::{GuestArch, GuestAddr, VmError};
use tracing::{debug, trace, warn};

/// 信号号定义（POSIX标准信号）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Signal {
    SIGHUP = 1,
    SIGINT = 2,
    SIGQUIT = 3,
    SIGILL = 4,
    SIGTRAP = 5,
    SIGABRT = 6,
    SIGBUS = 7,
    SIGFPE = 8,
    SIGKILL = 9,
    SIGUSR1 = 10,
    SIGSEGV = 11,
    SIGUSR2 = 12,
    SIGPIPE = 13,
    SIGALRM = 14,
    SIGTERM = 15,
    SIGSTKFLT = 16,
    SIGCHLD = 17,
    SIGCONT = 18,
    SIGSTOP = 19,
    SIGTSTP = 20,
    SIGTTIN = 21,
    SIGTTOU = 22,
    SIGURG = 23,
    SIGXCPU = 24,
    SIGXFSZ = 25,
    SIGVTALRM = 26,
    SIGPROF = 27,
    SIGWINCH = 28,
    SIGIO = 29,
    SIGPWR = 30,
    SIGSYS = 31,
}

impl Signal {
    /// 从信号号创建信号
    pub fn from_number(num: u32) -> Option<Self> {
        match num {
            1 => Some(Signal::SIGHUP),
            2 => Some(Signal::SIGINT),
            3 => Some(Signal::SIGQUIT),
            4 => Some(Signal::SIGILL),
            5 => Some(Signal::SIGTRAP),
            6 => Some(Signal::SIGABRT),
            7 => Some(Signal::SIGBUS),
            8 => Some(Signal::SIGFPE),
            9 => Some(Signal::SIGKILL),
            10 => Some(Signal::SIGUSR1),
            11 => Some(Signal::SIGSEGV),
            12 => Some(Signal::SIGUSR2),
            13 => Some(Signal::SIGPIPE),
            14 => Some(Signal::SIGALRM),
            15 => Some(Signal::SIGTERM),
            16 => Some(Signal::SIGSTKFLT),
            17 => Some(Signal::SIGCHLD),
            18 => Some(Signal::SIGCONT),
            19 => Some(Signal::SIGSTOP),
            20 => Some(Signal::SIGTSTP),
            21 => Some(Signal::SIGTTIN),
            22 => Some(Signal::SIGTTOU),
            23 => Some(Signal::SIGURG),
            24 => Some(Signal::SIGXCPU),
            25 => Some(Signal::SIGXFSZ),
            26 => Some(Signal::SIGVTALRM),
            27 => Some(Signal::SIGPROF),
            28 => Some(Signal::SIGWINCH),
            29 => Some(Signal::SIGIO),
            30 => Some(Signal::SIGPWR),
            31 => Some(Signal::SIGSYS),
            _ => None,
        }
    }

    /// 获取信号号
    pub fn number(&self) -> u32 {
        *self as u32
    }

    /// 获取信号名称
    pub fn name(&self) -> &'static str {
        match self {
            Signal::SIGHUP => "SIGHUP",
            Signal::SIGINT => "SIGINT",
            Signal::SIGQUIT => "SIGQUIT",
            Signal::SIGILL => "SIGILL",
            Signal::SIGTRAP => "SIGTRAP",
            Signal::SIGABRT => "SIGABRT",
            Signal::SIGBUS => "SIGBUS",
            Signal::SIGFPE => "SIGFPE",
            Signal::SIGKILL => "SIGKILL",
            Signal::SIGUSR1 => "SIGUSR1",
            Signal::SIGSEGV => "SIGSEGV",
            Signal::SIGUSR2 => "SIGUSR2",
            Signal::SIGPIPE => "SIGPIPE",
            Signal::SIGALRM => "SIGALRM",
            Signal::SIGTERM => "SIGTERM",
            Signal::SIGSTKFLT => "SIGSTKFLT",
            Signal::SIGCHLD => "SIGCHLD",
            Signal::SIGCONT => "SIGCONT",
            Signal::SIGSTOP => "SIGSTOP",
            Signal::SIGTSTP => "SIGTSTP",
            Signal::SIGTTIN => "SIGTTIN",
            Signal::SIGTTOU => "SIGTTOU",
            Signal::SIGURG => "SIGURG",
            Signal::SIGXCPU => "SIGXCPU",
            Signal::SIGXFSZ => "SIGXFSZ",
            Signal::SIGVTALRM => "SIGVTALRM",
            Signal::SIGPROF => "SIGPROF",
            Signal::SIGWINCH => "SIGWINCH",
            Signal::SIGIO => "SIGIO",
            Signal::SIGPWR => "SIGPWR",
            Signal::SIGSYS => "SIGSYS",
        }
    }

    /// 检查信号是否可以被捕获
    pub fn is_catchable(&self) -> bool {
        !matches!(
            self,
            Signal::SIGKILL | Signal::SIGSTOP
        )
    }

    /// 检查信号是否可以被忽略
    pub fn is_ignorable(&self) -> bool {
        !matches!(
            self,
            Signal::SIGKILL | Signal::SIGSTOP
        )
    }
}

/// 信号处理函数类型
pub type SignalHandler = Box<dyn Fn(u32, u32, u64) -> Result<(), VmError> + Send + Sync>;

/// 信号动作（sigaction结构）
#[derive(Debug, Clone)]
pub struct SignalAction {
    /// 信号处理函数地址
    pub handler: Option<GuestAddr>,
    /// 信号掩码
    pub mask: u64,
    /// 标志
    pub flags: u64,
    /// 恢复函数地址（用于信号栈）
    pub restorer: Option<GuestAddr>,
}

impl Default for SignalAction {
    fn default() -> Self {
        Self {
            handler: None,
            mask: 0,
            flags: 0,
            restorer: None,
        }
    }
}

/// 信号处理兼容层
pub struct SignalCompatibilityLayer {
    /// Guest 架构
    guest_arch: GuestArch,
    /// 信号处理函数映射
    handlers: Arc<Mutex<HashMap<u32, SignalAction>>>,
    /// 信号掩码
    signal_mask: Arc<Mutex<u64>>,
    /// 待处理的信号队列
    pending_signals: Arc<Mutex<Vec<u32>>>,
    /// 信号栈
    signal_stack: Arc<Mutex<Option<GuestAddr>>>,
}

impl SignalCompatibilityLayer {
    /// 创建新的信号处理兼容层
    pub fn new(guest_arch: GuestArch) -> Self {
        Self {
            guest_arch,
            handlers: Arc::new(Mutex::new(HashMap::new())),
            signal_mask: Arc::new(Mutex::new(0)),
            pending_signals: Arc::new(Mutex::new(Vec::new())),
            signal_stack: Arc::new(Mutex::new(None)),
        }
    }

    /// 注册信号处理函数（sigaction）
    pub fn register_handler(&self, signal: u32, action: SignalAction) -> Result<(), VmError> {
        let sig = Signal::from_number(signal).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidArgument {
                argument: "signal".to_string(),
                value: signal.to_string(),
                reason: "Invalid signal number".to_string(),
            })
        })?;

        if !sig.is_catchable() {
            return Err(VmError::Core(vm_core::CoreError::InvalidArgument {
                argument: "signal".to_string(),
                value: sig.name().to_string(),
                reason: "Signal cannot be caught".to_string(),
            }));
        }

        let mut handlers = self.handlers.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock handlers: {:?}", e),
                module: "SignalCompatibilityLayer".to_string(),
            })
        })?;

        handlers.insert(signal, action);
        debug!("Registered handler for signal {} ({})", signal, sig.name());

        Ok(())
    }

    /// 发送信号（kill）
    pub fn send_signal(&self, signal: u32) -> Result<(), VmError> {
        let sig = Signal::from_number(signal).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidArgument {
                argument: "signal".to_string(),
                value: signal.to_string(),
                reason: "Invalid signal number".to_string(),
            })
        })?;

        // 检查信号掩码
        let mask = *self.signal_mask.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock signal mask: {:?}", e),
                module: "SignalCompatibilityLayer".to_string(),
            })
        })?;

        if (mask & (1u64 << signal)) != 0 {
            trace!("Signal {} is masked, ignoring", sig.name());
            return Ok(());
        }

        // 添加到待处理队列
        let mut pending = self.pending_signals.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock pending signals: {:?}", e),
                module: "SignalCompatibilityLayer".to_string(),
            })
        })?;

        pending.push(signal);
        debug!("Signal {} ({}) queued for delivery", signal, sig.name());

        Ok(())
    }

    /// 设置信号掩码（sigprocmask）
    pub fn set_signal_mask(&self, mask: u64) -> Result<u64, VmError> {
        let mut current_mask = self.signal_mask.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock signal mask: {:?}", e),
                module: "SignalCompatibilityLayer".to_string(),
            })
        })?;

        let old_mask = *current_mask;
        *current_mask = mask;
        debug!("Signal mask updated: {:#x} -> {:#x}", old_mask, mask);

        Ok(old_mask)
    }

    /// 获取当前信号掩码
    pub fn get_signal_mask(&self) -> Result<u64, VmError> {
        let mask = self.signal_mask.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock signal mask: {:?}", e),
                module: "SignalCompatibilityLayer".to_string(),
            })
        })?;

        Ok(*mask)
    }

    /// 获取待处理的信号（sigpending）
    pub fn get_pending_signals(&self) -> Result<u64, VmError> {
        let pending = self.pending_signals.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock pending signals: {:?}", e),
                module: "SignalCompatibilityLayer".to_string(),
            })
        })?;

        let mut mask = 0u64;
        for &sig in pending.iter() {
            mask |= 1u64 << sig;
        }

        Ok(mask)
    }

    /// 设置信号栈（sigaltstack）
    pub fn set_signal_stack(&self, stack: Option<GuestAddr>) -> Result<(), VmError> {
        let mut signal_stack = self.signal_stack.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock signal stack: {:?}", e),
                module: "SignalCompatibilityLayer".to_string(),
            })
        })?;

        *signal_stack = stack;
        debug!("Signal stack set to: {:?}", stack);

        Ok(())
    }

    /// 获取信号栈
    pub fn get_signal_stack(&self) -> Result<Option<GuestAddr>, VmError> {
        let signal_stack = self.signal_stack.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock signal stack: {:?}", e),
                module: "SignalCompatibilityLayer".to_string(),
            })
        })?;

        Ok(*signal_stack)
    }

    /// 处理下一个待处理的信号
    pub fn process_next_signal(&self) -> Result<Option<(u32, SignalAction)>, VmError> {
        let mut pending = self.pending_signals.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock pending signals: {:?}", e),
                module: "SignalCompatibilityLayer".to_string(),
            })
        })?;

        if let Some(signal) = pending.pop() {
            let handlers = self.handlers.lock().map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to lock handlers: {:?}", e),
                    module: "SignalCompatibilityLayer".to_string(),
                })
            })?;

            if let Some(action) = handlers.get(&signal).cloned() {
                debug!("Processing signal {} with handler at {:?}", signal, action.handler);
                return Ok(Some((signal, action)));
            } else {
                warn!("No handler registered for signal {}", signal);
            }
        }

        Ok(None)
    }

    /// 清除所有待处理的信号
    pub fn clear_pending_signals(&self) -> Result<(), VmError> {
        let mut pending = self.pending_signals.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock pending signals: {:?}", e),
                module: "SignalCompatibilityLayer".to_string(),
            })
        })?;

        pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_from_number() {
        assert_eq!(Signal::from_number(1), Some(Signal::SIGHUP));
        assert_eq!(Signal::from_number(9), Some(Signal::SIGKILL));
        assert_eq!(Signal::from_number(99), None);
    }

    #[test]
    fn test_signal_catchable() {
        assert!(!Signal::SIGKILL.is_catchable());
        assert!(!Signal::SIGSTOP.is_catchable());
        assert!(Signal::SIGINT.is_catchable());
    }

    #[test]
    fn test_signal_handler_registration() {
        let layer = SignalCompatibilityLayer::new(GuestArch::X86_64);
        let action = SignalAction {
            handler: Some(GuestAddr(0x1000)),
            mask: 0,
            flags: 0,
            restorer: None,
        };

        assert!(layer.register_handler(2, action).is_ok()); // SIGINT
        assert!(layer.register_handler(9, action.clone()).is_err()); // SIGKILL (cannot be caught)
    }

    #[test]
    fn test_signal_sending() {
        let layer = SignalCompatibilityLayer::new(GuestArch::X86_64);
        assert!(layer.send_signal(2).is_ok()); // SIGINT
        assert!(layer.get_pending_signals().unwrap() & (1u64 << 2) != 0);
    }

    #[test]
    fn test_signal_mask() {
        let layer = SignalCompatibilityLayer::new(GuestArch::X86_64);
        let old_mask = layer.set_signal_mask(0xFF).unwrap();
        assert_eq!(old_mask, 0);
        assert_eq!(layer.get_signal_mask().unwrap(), 0xFF);
    }
}

