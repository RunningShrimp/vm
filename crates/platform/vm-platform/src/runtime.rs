//! 虚拟机运行时服务
//!
//! 提供虚拟机运行时环境管理、GC和系统服务
//! 从 vm-boot/runtime.rs 和 runtime_service.rs 迁移而来

use vm_core::VmError;

/// 运行时命令
#[derive(Debug, Clone)]
pub enum RuntimeCommand {
    /// 暂停虚拟机
    Pause,
    /// 继续虚拟机
    Resume,
    /// 获取状态
    Status,
    /// 重置虚拟机
    Reset,
    /// 自定义命令
    CustomCommand(String),
}

/// 运行时事件
#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    /// 虚拟机已暂停
    Paused,
    /// 虚拟机已继续
    Resumed,
    /// 设备热插拔
    Hotplug(String),
    /// 设备热移除
    Hotremove(String),
    /// 错误发生
    Error(String),
}

/// 运行时状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeState {
    Stopped,
    Running,
    Paused,
}

/// 运行时控制器特征
pub trait Runtime: Send + Sync {
    /// 执行运行时命令
    fn execute_command(&mut self, cmd: RuntimeCommand) -> Result<RuntimeEvent, VmError>;

    /// 获取运行时状态
    fn get_state(&self) -> RuntimeState;

    /// 获取运行时统计信息
    fn get_stats(&mut self) -> RuntimeStats;
}

/// 运行时统计信息
#[derive(Debug, Clone)]
pub struct RuntimeStats {
    /// 运行时间（秒）
    pub uptime_seconds: u64,
    /// CPU 使用率（百分比）
    pub cpu_usage_percent: f64,
    /// 内存使用量（字节）
    pub memory_used_bytes: u64,
    /// 设备数量
    pub device_count: usize,
}

/// 简化的运行时控制器实现
pub struct SimpleRuntimeController {
    state: RuntimeState,
    uptime_start: std::time::Instant,
    #[allow(dead_code)]
    last_cpu_time: Option<(u64, u64)>, // (total, idle)
    device_count: usize,
}

impl SimpleRuntimeController {
    /// 创建新的运行时控制器
    pub fn new() -> Self {
        Self {
            state: RuntimeState::Stopped,
            uptime_start: std::time::Instant::now(),
            last_cpu_time: None,
            device_count: 0,
        }
    }

    /// 设置设备数量
    pub fn set_device_count(&mut self, count: usize) {
        self.device_count = count;
    }

    /// 获取系统 CPU 使用率
    #[cfg(target_os = "linux")]
    fn get_cpu_usage(&mut self) -> f64 {
        use std::fs;

        // 读取 /proc/stat 获取 CPU 时间
        if let Ok(content) = fs::read_to_string("/proc/stat") {
            let first_line = content.lines().next();
            if let Some(line) = first_line {
                let parts: Vec<u64> = line
                    .split_whitespace()
                    .skip(1) // 跳过 "cpu" 前缀
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if parts.len() >= 4 {
                    let user = parts[0];
                    let nice = parts[1];
                    let system = parts[2];
                    let idle = parts[3];

                    let total = user + nice + system + idle;

                    if let Some((last_total, last_idle)) = self.last_cpu_time {
                        let total_delta = total.saturating_sub(last_total);
                        let idle_delta = idle.saturating_sub(last_idle);

                        self.last_cpu_time = Some((total, idle));

                        if total_delta > 0 {
                            return 100.0 * (1.0 - (idle_delta as f64 / total_delta as f64));
                        }
                    } else {
                        self.last_cpu_time = Some((total, idle));
                    }
                }
            }
        }

        0.0
    }

    /// 获取系统 CPU 使用率（非 Linux 平台）
    #[cfg(not(target_os = "linux"))]
    fn get_cpu_usage(&mut self) -> f64 {
        // 其他平台返回模拟值
        0.0
    }

    /// 获取内存使用量（字节）
    #[cfg(target_os = "linux")]
    fn get_memory_usage(&self) -> u64 {
        use std::fs;

        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            let mut total_memory = 0u64;
            let mut free_memory = 0u64;
            let mut buffers = 0u64;
            let mut cached = 0u64;

            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    total_memory = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                } else if line.starts_with("MemFree:") {
                    free_memory = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                } else if line.starts_with("Buffers:") {
                    buffers = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                } else if line.starts_with("Cached:") {
                    cached = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                }
            }

            if total_memory > 0 {
                let used = total_memory.saturating_sub(free_memory + buffers + cached);
                return used * 1024; // 转换为字节
            }
        }

        0
    }

    /// 获取内存使用量（字节）（非 Linux 平台）
    #[cfg(not(target_os = "linux"))]
    fn get_memory_usage(&self) -> u64 {
        // 其他平台返回模拟值
        0
    }

    /// 执行运行时命令
    pub fn execute_command(&mut self, cmd: RuntimeCommand) -> Result<RuntimeEvent, VmError> {
        log::info!("Executing runtime command: {:?}", cmd);

        let event = match cmd {
            RuntimeCommand::Pause => {
                self.state = RuntimeState::Paused;
                RuntimeEvent::Paused
            }
            RuntimeCommand::Resume => {
                self.state = RuntimeState::Running;
                RuntimeEvent::Resumed
            }
            RuntimeCommand::Status => RuntimeEvent::Hotplug("Status".to_string()),
            RuntimeCommand::Reset => {
                self.state = RuntimeState::Stopped;
                self.uptime_start = std::time::Instant::now();
                RuntimeEvent::Error("Reset".to_string())
            }
            RuntimeCommand::CustomCommand(s) => {
                RuntimeEvent::Error(format!("Custom command: {}", s))
            }
        };

        Ok(event)
    }

    /// 获取运行时状态
    pub fn get_state(&self) -> RuntimeState {
        self.state.clone()
    }

    /// 获取运行时统计信息
    pub fn get_stats(&mut self) -> RuntimeStats {
        let elapsed = self.uptime_start.elapsed().as_secs();
        RuntimeStats {
            uptime_seconds: elapsed,
            cpu_usage_percent: self.get_cpu_usage(),
            memory_used_bytes: self.get_memory_usage(),
            device_count: self.device_count,
        }
    }
}

impl Default for SimpleRuntimeController {
    fn default() -> Self {
        Self::new()
    }
}
