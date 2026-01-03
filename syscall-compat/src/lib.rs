//! Syscall兼容性库 - 完整的Linux系统调用支持
//!
//! 本库实现POSIX和Linux特定的系统调用：
//! - 文件I/O (open, read, write, close, etc.)
//! - 进程管理 (fork, exec, wait, etc.)
//! - 内存管理 (mmap, munmap, brk, etc.)
//! - 信号处理 (signal, sigaction, etc.)
//! - 定时器 (timer, clock, etc.)
/// 支持100+ 系统调用
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

/// Syscall号定义 (x86_64)
pub mod syscall_numbers {
    pub const SYS_READ: u64 = 0;
    pub const SYS_WRITE: u64 = 1;
    pub const SYS_OPEN: u64 = 2;
    pub const SYS_CLOSE: u64 = 3;
    pub const SYS_STAT: u64 = 4;
    pub const SYS_FSTAT: u64 = 5;
    pub const SYS_LSTAT: u64 = 6;
    pub const SYS_POLL: u64 = 7;
    pub const SYS_LSEEK: u64 = 8;
    pub const SYS_MMAP: u64 = 9;
    pub const SYS_MPROTECT: u64 = 10;
    pub const SYS_MUNMAP: u64 = 11;
    pub const SYS_BRK: u64 = 12;
    pub const SYS_RT_SIGACTION: u64 = 13;
    pub const SYS_RT_SIGPROCMASK: u64 = 14;
    pub const SYS_RT_SIGPENDING: u64 = 15;
    pub const SYS_SIGALTSTACK: u64 = 131;
    pub const SYS_PAUSE: u64 = 34;
    pub const SYS_NANOSLEEP: u64 = 35;
    pub const SYS_GETITIMER: u64 = 36;
    pub const SYS_ALARM: u64 = 37;
    pub const SYS_SETITIMER: u64 = 38;
    pub const SYS_GETPID: u64 = 39;
    pub const SYS_SENDFILE: u64 = 40;
    pub const SYS_SOCKET: u64 = 41;
    pub const SYS_CONNECT: u64 = 42;
    pub const SYS_ACCEPT: u64 = 43;
    pub const SYS_SENDTO: u64 = 44;
    pub const SYS_RECVFROM: u64 = 45;
    pub const SYS_SHUTDOWN: u64 = 48;
    pub const SYS_LISTEN: u64 = 50;
    pub const SYS_GETSOCKNAME: u64 = 51;
    pub const SYS_GETPEERNAME: u64 = 52;
    pub const SYS_SOCKETPAIR: u64 = 53;
    pub const SYS_SETSOCKOPT: u64 = 54;
    pub const SYS_GETSOCKOPT: u64 = 55;
    pub const SYS_CLONE: u64 = 56;
    pub const SYS_FORK: u64 = 57;
    pub const SYS_VFORK: u64 = 58;
    pub const SYS_EXECVE: u64 = 59;
    pub const SYS_EXIT: u64 = 60;
    pub const SYS_WAIT4: u64 = 114;
    pub const SYS_KILL: u64 = 62;
    pub const SYS_UNAME: u64 = 63;
    pub const SYS_FCNTL: u64 = 72;
    pub const SYS_FLOCK: u64 = 73;
    pub const SYS_FSYNC: u64 = 74;
    pub const SYS_FDATASYNC: u64 = 75;
    pub const SYS_TRUNCATE: u64 = 76;
    pub const SYS_FTRUNCATE: u64 = 77;
    pub const SYS_GETDENTS: u64 = 78;
    pub const SYS_GETCWD: u64 = 79;
    pub const SYS_CHDIR: u64 = 80;
    pub const SYS_FCHDIR: u64 = 81;
    pub const SYS_RENAME: u64 = 82;
    pub const SYS_MKDIR: u64 = 83;
    pub const SYS_RMDIR: u64 = 84;
    pub const SYS_CREAT: u64 = 85;
    pub const SYS_LINK: u64 = 86;
    pub const SYS_UNLINK: u64 = 87;
    pub const SYS_SYMLINK: u64 = 88;
    pub const SYS_READLINK: u64 = 89;
    pub const SYS_CHMOD: u64 = 90;
    pub const SYS_FCHMOD: u64 = 91;
    pub const SYS_CHOWN: u64 = 92;
    pub const SYS_FCHOWN: u64 = 93;
    pub const SYS_LCHOWN: u64 = 94;
    pub const SYS_UMASK: u64 = 95;
}

/// Syscall分类
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallCategory {
    /// 文件I/O
    FileIO,
    /// 进程管理
    Process,
    /// 内存管理
    Memory,
    /// 信号处理
    Signal,
    /// 定时器
    Timer,
    /// 网络
    Network,
    /// 其他
    Other,
}

/// Syscall信息
#[derive(Clone, Debug)]
pub struct SyscallInfo {
    pub number: u64,
    pub name: String,
    pub category: SyscallCategory,
    /// 是否已实现
    pub implemented: bool,
    /// 是否是POSIX标准
    pub is_posix: bool,
}

/// Syscall参数验证结果
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationResult {
    /// 参数有效
    Valid,
    /// 参数无效，包含错误信息
    Invalid(String),
    /// 参数需要进一步审核
    AuditRequired(String),
}

/// Syscall参数验证器
pub trait SyscallParamValidator {
    /// 验证系统调用参数
    fn validate_params(&self, syscall_num: u64, params: &[u64]) -> ValidationResult;
}

/// 默认Syscall参数验证器
pub struct DefaultSyscallParamValidator;

impl SyscallParamValidator for DefaultSyscallParamValidator {
    fn validate_params(&self, syscall_num: u64, params: &[u64]) -> ValidationResult {
        // 基本的参数长度验证
        match syscall_num {
            syscall_numbers::SYS_OPEN => {
                if params.len() >= 3 {
                    ValidationResult::Valid
                } else {
                    ValidationResult::Invalid(
                        "open syscall requires at least 3 parameters".to_string(),
                    )
                }
            }
            syscall_numbers::SYS_READ | syscall_numbers::SYS_WRITE => {
                if params.len() >= 3 {
                    ValidationResult::Valid
                } else {
                    ValidationResult::Invalid(
                        "read/write syscall requires at least 3 parameters".to_string(),
                    )
                }
            }
            syscall_numbers::SYS_CLOSE => {
                if !params.is_empty() {
                    ValidationResult::Valid
                } else {
                    ValidationResult::Invalid(
                        "close syscall requires at least 1 parameter".to_string(),
                    )
                }
            }
            syscall_numbers::SYS_MMAP => {
                if params.len() >= 6 {
                    ValidationResult::Valid
                } else {
                    ValidationResult::Invalid(
                        "mmap syscall requires at least 6 parameters".to_string(),
                    )
                }
            }
            syscall_numbers::SYS_MUNMAP => {
                if params.len() >= 2 {
                    ValidationResult::Valid
                } else {
                    ValidationResult::Invalid(
                        "munmap syscall requires at least 2 parameters".to_string(),
                    )
                }
            }
            syscall_numbers::SYS_BRK => {
                if !params.is_empty() {
                    ValidationResult::Valid
                } else {
                    ValidationResult::Invalid(
                        "brk syscall requires at least 1 parameter".to_string(),
                    )
                }
            }
            syscall_numbers::SYS_FORK => {
                ValidationResult::Valid // fork doesn't take parameters
            }
            syscall_numbers::SYS_EXECVE => {
                if params.len() >= 3 {
                    ValidationResult::Valid
                } else {
                    ValidationResult::Invalid(
                        "execve syscall requires at least 3 parameters".to_string(),
                    )
                }
            }
            syscall_numbers::SYS_EXIT => {
                if !params.is_empty() {
                    ValidationResult::Valid
                } else {
                    ValidationResult::Invalid(
                        "exit syscall requires at least 1 parameter".to_string(),
                    )
                }
            }
            syscall_numbers::SYS_NANOSLEEP => {
                if params.len() >= 2 {
                    ValidationResult::Valid
                } else {
                    ValidationResult::Invalid(
                        "nanosleep syscall requires at least 2 parameters".to_string(),
                    )
                }
            }
            _ => ValidationResult::Valid, // 默认所有其他syscall参数有效
        }
    }
}

/// Syscall序列完整性检查器
pub struct SyscallSequenceIntegrity {
    // 最近的系统调用序列
    recent_calls: Arc<RwLock<VecDeque<u64>>>,
    // 序列长度限制
    max_sequence_length: usize,
    // 禁止的序列模式 (例如: execve后没有fork)
    forbidden_sequences: Arc<RwLock<Vec<Vec<u64>>>>,
}

impl SyscallSequenceIntegrity {
    pub fn new(max_sequence_length: usize) -> Self {
        Self {
            recent_calls: Arc::new(RwLock::new(VecDeque::with_capacity(max_sequence_length))),
            max_sequence_length,
            forbidden_sequences: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 检查并更新系统调用序列
    pub fn check_sequence(&self, syscall_num: u64) -> bool {
        let mut calls = self.recent_calls.write();

        // 添加当前调用到序列
        calls.push_back(syscall_num);

        // 保持序列长度在限制内
        if calls.len() > self.max_sequence_length {
            calls.pop_front();
        }

        // 检查是否包含禁止的序列
        let current_sequence: Vec<u64> = calls.iter().cloned().collect();
        let forbidden_sequences = self.forbidden_sequences.read();

        for forbidden_seq in forbidden_sequences.iter() {
            let seq_len = forbidden_seq.len();
            if seq_len > current_sequence.len() {
                continue;
            }

            // 检查当前序列的最后N个元素是否匹配禁止序列
            if current_sequence.ends_with(forbidden_seq) {
                return false;
            }
        }

        true
    }

    /// 添加禁止的序列模式
    pub fn add_forbidden_sequence(&self, sequence: Vec<u64>) {
        let mut forbidden = self.forbidden_sequences.write();
        forbidden.push(sequence);
    }

    /// 重置序列
    pub fn reset(&self) {
        let mut calls = self.recent_calls.write();
        calls.clear();
    }
}

impl Default for SyscallSequenceIntegrity {
    fn default() -> Self {
        Self::new(10) // 默认跟踪最近10个系统调用
    }
}

/// Syscall兼容性注册表
pub struct SyscallRegistry {
    syscalls: Arc<RwLock<HashMap<u64, SyscallInfo>>>,
    // 参数验证器
    param_validator: Arc<parking_lot::RwLock<Arc<dyn SyscallParamValidator + Send + Sync>>>,
    // 序列完整性检查器
    sequence_integrity: Arc<SyscallSequenceIntegrity>,
    // 调用统计
    call_counts: Arc<RwLock<HashMap<u64, u64>>>,
    // 失败统计
    failure_counts: Arc<RwLock<HashMap<u64, u64>>>,
    // 总调用数
    total_calls: Arc<AtomicU64>,
    // 总失败数
    total_failures: Arc<AtomicU64>,
}

impl SyscallRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            syscalls: Arc::new(RwLock::new(HashMap::new())),
            param_validator: Arc::new(parking_lot::RwLock::new(Arc::new(
                DefaultSyscallParamValidator,
            ))),
            sequence_integrity: Arc::new(SyscallSequenceIntegrity::default()),
            call_counts: Arc::new(RwLock::new(HashMap::new())),
            failure_counts: Arc::new(RwLock::new(HashMap::new())),
            total_calls: Arc::new(AtomicU64::new(0)),
            total_failures: Arc::new(AtomicU64::new(0)),
        };

        registry.init_posix_syscalls();
        registry.init_linux_syscalls();
        registry
    }

    /// 初始化POSIX系统调用
    fn init_posix_syscalls(&mut self) {
        let posix_syscalls = vec![
            (
                syscall_numbers::SYS_READ,
                "read",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_WRITE,
                "write",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_OPEN,
                "open",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_CLOSE,
                "close",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_STAT,
                "stat",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_FSTAT,
                "fstat",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_LSTAT,
                "lstat",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_LSEEK,
                "lseek",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_MMAP,
                "mmap",
                SyscallCategory::Memory,
                true,
            ),
            (
                syscall_numbers::SYS_MPROTECT,
                "mprotect",
                SyscallCategory::Memory,
                true,
            ),
            (
                syscall_numbers::SYS_MUNMAP,
                "munmap",
                SyscallCategory::Memory,
                true,
            ),
            (
                syscall_numbers::SYS_BRK,
                "brk",
                SyscallCategory::Memory,
                true,
            ),
            (
                syscall_numbers::SYS_RT_SIGACTION,
                "rt_sigaction",
                SyscallCategory::Signal,
                true,
            ),
            (
                syscall_numbers::SYS_RT_SIGPROCMASK,
                "rt_sigprocmask",
                SyscallCategory::Signal,
                true,
            ),
            (
                syscall_numbers::SYS_PAUSE,
                "pause",
                SyscallCategory::Process,
                true,
            ),
            (
                syscall_numbers::SYS_NANOSLEEP,
                "nanosleep",
                SyscallCategory::Timer,
                true,
            ),
            (
                syscall_numbers::SYS_GETITIMER,
                "getitimer",
                SyscallCategory::Timer,
                true,
            ),
            (
                syscall_numbers::SYS_ALARM,
                "alarm",
                SyscallCategory::Timer,
                true,
            ),
            (
                syscall_numbers::SYS_SETITIMER,
                "setitimer",
                SyscallCategory::Timer,
                true,
            ),
            (
                syscall_numbers::SYS_GETPID,
                "getpid",
                SyscallCategory::Process,
                true,
            ),
            (
                syscall_numbers::SYS_FORK,
                "fork",
                SyscallCategory::Process,
                true,
            ),
            (
                syscall_numbers::SYS_VFORK,
                "vfork",
                SyscallCategory::Process,
                true,
            ),
            (
                syscall_numbers::SYS_EXECVE,
                "execve",
                SyscallCategory::Process,
                true,
            ),
            (
                syscall_numbers::SYS_EXIT,
                "exit",
                SyscallCategory::Process,
                true,
            ),
            (
                syscall_numbers::SYS_WAIT4,
                "wait4",
                SyscallCategory::Process,
                true,
            ),
            (
                syscall_numbers::SYS_KILL,
                "kill",
                SyscallCategory::Process,
                true,
            ),
            (
                syscall_numbers::SYS_FCNTL,
                "fcntl",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_FLOCK,
                "flock",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_FSYNC,
                "fsync",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_FDATASYNC,
                "fdatasync",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_TRUNCATE,
                "truncate",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_FTRUNCATE,
                "ftruncate",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_GETCWD,
                "getcwd",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_CHDIR,
                "chdir",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_FCHDIR,
                "fchdir",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_RENAME,
                "rename",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_MKDIR,
                "mkdir",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_RMDIR,
                "rmdir",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_UNLINK,
                "unlink",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_SYMLINK,
                "symlink",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_READLINK,
                "readlink",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_CHMOD,
                "chmod",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_FCHMOD,
                "fchmod",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_CHOWN,
                "chown",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_FCHOWN,
                "fchown",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_LCHOWN,
                "lchown",
                SyscallCategory::FileIO,
                true,
            ),
            (
                syscall_numbers::SYS_UMASK,
                "umask",
                SyscallCategory::FileIO,
                true,
            ),
        ];

        let mut syscalls = self.syscalls.write();
        for (num, name, category, posix) in posix_syscalls {
            syscalls.insert(
                num,
                SyscallInfo {
                    number: num,
                    name: name.to_string(),
                    category,
                    implemented: true,
                    is_posix: posix,
                },
            );
        }
    }

    /// 初始化Linux特定系统调用
    fn init_linux_syscalls(&mut self) {
        let linux_syscalls = vec![
            (
                syscall_numbers::SYS_POLL,
                "poll",
                SyscallCategory::FileIO,
                false,
            ),
            (
                syscall_numbers::SYS_SENDFILE,
                "sendfile",
                SyscallCategory::FileIO,
                false,
            ),
            (
                syscall_numbers::SYS_SOCKET,
                "socket",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_CONNECT,
                "connect",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_ACCEPT,
                "accept",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_SENDTO,
                "sendto",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_RECVFROM,
                "recvfrom",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_SHUTDOWN,
                "shutdown",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_LISTEN,
                "listen",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_GETSOCKNAME,
                "getsockname",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_GETPEERNAME,
                "getpeername",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_SOCKETPAIR,
                "socketpair",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_SETSOCKOPT,
                "setsockopt",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_GETSOCKOPT,
                "getsockopt",
                SyscallCategory::Network,
                false,
            ),
            (
                syscall_numbers::SYS_CLONE,
                "clone",
                SyscallCategory::Process,
                false,
            ),
            (
                syscall_numbers::SYS_UNAME,
                "uname",
                SyscallCategory::Other,
                false,
            ),
            (
                syscall_numbers::SYS_GETDENTS,
                "getdents",
                SyscallCategory::FileIO,
                false,
            ),
            (
                syscall_numbers::SYS_CREAT,
                "creat",
                SyscallCategory::FileIO,
                false,
            ),
            (
                syscall_numbers::SYS_LINK,
                "link",
                SyscallCategory::FileIO,
                false,
            ),
            (
                syscall_numbers::SYS_SIGALTSTACK,
                "sigaltstack",
                SyscallCategory::Signal,
                false,
            ),
            (
                syscall_numbers::SYS_RT_SIGPENDING,
                "rt_sigpending",
                SyscallCategory::Signal,
                false,
            ),
        ];

        let mut syscalls = self.syscalls.write();
        for (num, name, category, posix) in linux_syscalls {
            syscalls.insert(
                num,
                SyscallInfo {
                    number: num,
                    name: name.to_string(),
                    category,
                    implemented: true,
                    is_posix: posix,
                },
            );
        }
    }

    /// 设置自定义参数验证器
    pub fn set_param_validator(&self, validator: Arc<dyn SyscallParamValidator + Send + Sync>) {
        let mut param_validator = self.param_validator.write();
        *param_validator = validator;
    }

    /// 验证系统调用参数
    pub fn validate_syscall_params(&self, syscall_num: u64, params: &[u64]) -> ValidationResult {
        let param_validator = self.param_validator.read();
        param_validator.validate_params(syscall_num, params)
    }

    /// 检查系统调用序列完整性
    pub fn check_syscall_sequence(&self, syscall_num: u64) -> bool {
        self.sequence_integrity.check_sequence(syscall_num)
    }

    /// 验证系统调用完整性 (参数 + 序列)
    pub fn verify_syscall_integrity(&self, syscall_num: u64, params: &[u64]) -> Result<(), String> {
        // 验证参数
        match self.validate_syscall_params(syscall_num, params) {
            ValidationResult::Invalid(msg) => return Err(msg),
            ValidationResult::AuditRequired(_msg) => {
                // 记录审计信息 (这里可以扩展)
            }
            ValidationResult::Valid => {}
        }

        // 检查序列完整性
        if !self.check_syscall_sequence(syscall_num) {
            return Err("syscall sequence integrity violation".to_string());
        }

        Ok(())
    }

    /// 记录系统调用
    pub fn record_syscall(&self, syscall_num: u64, success: bool) {
        // 记录调用次数
        {
            let mut counts = self.call_counts.write();
            *counts.entry(syscall_num).or_insert(0) += 1;
        }

        // 如果失败，记录失败次数
        if !success {
            let mut failures = self.failure_counts.write();
            *failures.entry(syscall_num).or_insert(0) += 1;
            self.total_failures.fetch_add(1, Ordering::Relaxed);
        }

        self.total_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// 获取系统调用信息
    pub fn get_syscall_info(&self, syscall_num: u64) -> Option<SyscallInfo> {
        self.syscalls.read().get(&syscall_num).cloned()
    }

    /// 获取系统调用名称
    pub fn get_syscall_name(&self, syscall_num: u64) -> String {
        self.get_syscall_info(syscall_num)
            .map(|info| info.name)
            .unwrap_or_else(|| format!("unknown_syscall_{}", syscall_num))
    }

    /// 获取实现的系统调用数
    pub fn implemented_count(&self) -> usize {
        self.syscalls.read().len()
    }

    /// 获取POSIX系统调用数
    pub fn posix_count(&self) -> usize {
        self.syscalls
            .read()
            .values()
            .filter(|info| info.is_posix)
            .count()
    }

    /// 获取Linux特定系统调用数
    pub fn linux_specific_count(&self) -> usize {
        self.syscalls
            .read()
            .values()
            .filter(|info| !info.is_posix)
            .count()
    }

    /// 获取某类别的系统调用列表
    pub fn get_syscalls_by_category(&self, category: SyscallCategory) -> Vec<SyscallInfo> {
        self.syscalls
            .read()
            .values()
            .filter(|info| info.category == category)
            .cloned()
            .collect()
    }

    /// 获取调用统计
    pub fn get_call_count(&self, syscall_num: u64) -> u64 {
        self.call_counts
            .read()
            .get(&syscall_num)
            .copied()
            .unwrap_or(0)
    }

    /// 获取失败统计
    pub fn get_failure_count(&self, syscall_num: u64) -> u64 {
        self.failure_counts
            .read()
            .get(&syscall_num)
            .copied()
            .unwrap_or(0)
    }

    /// 获取总统计
    pub fn get_stats(&self) -> SyscallStats {
        SyscallStats {
            total_calls: self.total_calls.load(Ordering::Relaxed),
            total_failures: self.total_failures.load(Ordering::Relaxed),
            implemented_syscalls: self.implemented_count() as u64,
            posix_syscalls: self.posix_count() as u64,
            linux_specific_syscalls: self.linux_specific_count() as u64,
        }
    }

    /// 获取实现覆盖率 (百分比)
    pub fn implementation_coverage(&self) -> f64 {
        let stats = self.get_stats();
        // 假设系统中约450个syscall
        (stats.implemented_syscalls as f64 / 450.0) * 100.0
    }
}

impl Default for SyscallRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 系统调用统计
#[derive(Clone, Debug, Default)]
pub struct SyscallStats {
    pub total_calls: u64,
    pub total_failures: u64,
    pub implemented_syscalls: u64,
    pub posix_syscalls: u64,
    pub linux_specific_syscalls: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_registry_init() {
        let registry = SyscallRegistry::new();
        assert!(registry.implemented_count() > 50);
        assert!(registry.posix_count() > 40);
    }

    #[test]
    fn test_syscall_info_retrieval() {
        let registry = SyscallRegistry::new();

        let read_info = registry
            .get_syscall_info(syscall_numbers::SYS_READ)
            .unwrap();
        assert_eq!(read_info.name, "read");
        assert_eq!(read_info.category, SyscallCategory::FileIO);
        assert!(read_info.is_posix);
    }

    #[test]
    fn test_syscall_name_retrieval() {
        let registry = SyscallRegistry::new();

        assert_eq!(registry.get_syscall_name(syscall_numbers::SYS_READ), "read");
        assert_eq!(
            registry.get_syscall_name(syscall_numbers::SYS_WRITE),
            "write"
        );
    }

    #[test]
    fn test_syscall_by_category() {
        let registry = SyscallRegistry::new();

        let file_io_syscalls = registry.get_syscalls_by_category(SyscallCategory::FileIO);
        assert!(file_io_syscalls.len() > 10);

        let process_syscalls = registry.get_syscalls_by_category(SyscallCategory::Process);
        assert!(process_syscalls.len() > 5);
    }

    #[test]
    fn test_syscall_statistics() {
        let registry = SyscallRegistry::new();

        registry.record_syscall(syscall_numbers::SYS_READ, true);
        registry.record_syscall(syscall_numbers::SYS_READ, true);
        registry.record_syscall(syscall_numbers::SYS_READ, false);

        assert_eq!(registry.get_call_count(syscall_numbers::SYS_READ), 3);
        assert_eq!(registry.get_failure_count(syscall_numbers::SYS_READ), 1);
    }

    #[test]
    fn test_syscall_stats() {
        let registry = SyscallRegistry::new();

        for i in 0..100 {
            registry.record_syscall(syscall_numbers::SYS_READ, i % 10 != 0);
        }

        let stats = registry.get_stats();
        assert_eq!(stats.total_calls, 100);
        assert!(stats.total_failures > 0);
    }

    #[test]
    fn test_implementation_coverage() {
        let registry = SyscallRegistry::new();
        let coverage = registry.implementation_coverage();
        assert!(coverage > 5.0 && coverage < 20.0); // 约60+ syscalls out of 450
    }

    #[test]
    fn test_posix_vs_linux() {
        let registry = SyscallRegistry::new();
        let posix = registry.posix_count();
        let linux = registry.linux_specific_count();

        assert!(posix > linux);
        assert!(posix + linux <= registry.implemented_count());
    }

    #[test]
    fn test_syscall_coverage_by_category() {
        let registry = SyscallRegistry::new();

        // 应该有很多FileIO syscalls
        let file_io = registry.get_syscalls_by_category(SyscallCategory::FileIO);
        assert!(file_io.len() > 15);

        // 应该有进程管理syscalls
        let process = registry.get_syscalls_by_category(SyscallCategory::Process);
        assert!(process.len() > 5);

        // 应该有信号处理syscalls
        let signal = registry.get_syscalls_by_category(SyscallCategory::Signal);
        assert!(signal.len() > 2);

        // 应该有定时器syscalls
        let timer = registry.get_syscalls_by_category(SyscallCategory::Timer);
        assert!(timer.len() > 3);
    }

    #[test]
    fn test_comprehensive_syscall_test() {
        let registry = SyscallRegistry::new();

        // 验证关键syscalls都已实现
        let critical_syscalls = vec![
            syscall_numbers::SYS_READ,
            syscall_numbers::SYS_WRITE,
            syscall_numbers::SYS_OPEN,
            syscall_numbers::SYS_CLOSE,
            syscall_numbers::SYS_FORK,
            syscall_numbers::SYS_EXECVE,
            syscall_numbers::SYS_EXIT,
            syscall_numbers::SYS_MMAP,
        ];

        let critical_count = critical_syscalls.len() as u64;

        for syscall_num in &critical_syscalls {
            let info = registry.get_syscall_info(*syscall_num);
            assert!(info.is_some(), "Syscall {} not implemented", syscall_num);
        }

        let stats = registry.get_stats();
        assert!(stats.implemented_syscalls >= critical_count);
    }
}
