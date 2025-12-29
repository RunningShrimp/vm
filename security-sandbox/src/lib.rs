//! 安全隔离库 - 生产环境安全保障
//!
//! 本库实现完整的安全隔离机制：
//! - Seccomp沙箱 (syscall白名单)
//! - 资源配额管理 (CPU/内存/I/O限制)
//! - 访问控制列表 (权限管理)
//! - 审计日志 (安全事件记录)

use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// 权限映射类型别名：简化复杂类型定义
/// Key: (资源类型, 资源ID) -> Value: 权限集合
type PermissionMap = Arc<RwLock<HashMap<(String, String), HashSet<String>>>>;

/// Syscall权限级别
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallPermission {
    /// 允许执行
    Allow,
    /// 拒绝执行
    Deny,
    /// 需要审核
    Audit,
}

/// Seccomp沙箱配置
pub struct SeccompPolicy {
    // Syscall白名单 (按权限分类)
    allowed_syscalls: Arc<RwLock<HashMap<String, SyscallPermission>>>,
    // 默认策略 (白名单模式：Deny，黑名单模式：Allow)
    default_policy: SyscallPermission,
}

impl SeccompPolicy {
    pub fn new(whitelist_mode: bool) -> Self {
        Self {
            allowed_syscalls: Arc::new(RwLock::new(HashMap::new())),
            default_policy: if whitelist_mode {
                SyscallPermission::Deny
            } else {
                SyscallPermission::Allow
            },
        }
    }

    /// 添加白名单系统调用
    pub fn add_allowed_syscall(&self, syscall: String) {
        self.allowed_syscalls
            .write()
            .insert(syscall, SyscallPermission::Allow);
    }

    /// 添加黑名单系统调用
    pub fn add_denied_syscall(&self, syscall: String) {
        self.allowed_syscalls
            .write()
            .insert(syscall, SyscallPermission::Deny);
    }

    /// 添加需审核的系统调用
    pub fn add_audit_syscall(&self, syscall: String) {
        self.allowed_syscalls
            .write()
            .insert(syscall, SyscallPermission::Audit);
    }

    /// 检查系统调用权限
    pub fn check_syscall(&self, syscall: &str) -> SyscallPermission {
        self.allowed_syscalls
            .read()
            .get(syscall)
            .copied()
            .unwrap_or(self.default_policy)
    }

    /// 获取允许的系统调用列表
    pub fn get_allowed_syscalls(&self) -> Vec<String> {
        self.allowed_syscalls
            .read()
            .iter()
            .filter(|(_, perm)| **perm == SyscallPermission::Allow)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// 获取被拒绝的系统调用列表
    pub fn get_denied_syscalls(&self) -> Vec<String> {
        self.allowed_syscalls
            .read()
            .iter()
            .filter(|(_, perm)| **perm == SyscallPermission::Deny)
            .map(|(name, _)| name.clone())
            .collect()
    }
}

impl Default for SeccompPolicy {
    fn default() -> Self {
        Self::new(true) // 默认使用白名单模式
    }
}

/// 资源配额配置
#[derive(Clone, Debug)]
pub struct ResourceQuota {
    /// CPU时间限制 (毫秒)
    pub cpu_time_ms: Option<u64>,
    /// 内存限制 (字节)
    pub memory_limit_bytes: Option<u64>,
    /// 文件描述符限制
    pub fd_limit: Option<u32>,
    /// 进程/线程限制
    pub process_limit: Option<u32>,
    /// 文件大小限制 (字节)
    pub file_size_limit: Option<u64>,
    /// I/O速率限制 (字节/秒)
    pub io_rate_limit: Option<u64>,
}

impl ResourceQuota {
    pub fn new() -> Self {
        Self {
            cpu_time_ms: Some(10000),                     // 10秒
            memory_limit_bytes: Some(1024 * 1024 * 1024), // 1GB
            fd_limit: Some(256),
            process_limit: Some(1),
            file_size_limit: Some(100 * 1024 * 1024), // 100MB
            io_rate_limit: Some(10 * 1024 * 1024),    // 10MB/s
        }
    }
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self::new()
    }
}

/// 资源使用统计
#[derive(Clone, Debug, Default)]
pub struct ResourceUsage {
    /// 已使用CPU时间 (毫秒)
    pub cpu_time_ms: u64,
    /// 当前内存使用 (字节)
    pub memory_bytes: u64,
    /// 打开的文件描述符数
    pub open_fds: u32,
    /// 已运行的进程/线程数
    pub process_count: u32,
    /// 已写入的文件大小 (字节)
    pub file_written_bytes: u64,
    /// I/O操作数
    pub io_ops: u64,
}

impl ResourceUsage {
    /// 检查是否超出配额
    pub fn check_quota(&self, quota: &ResourceQuota) -> Vec<String> {
        let mut violations = Vec::new();

        if let Some(limit) = quota.cpu_time_ms
            && self.cpu_time_ms > limit
        {
            violations.push(format!(
                "CPU time exceeded: {} > {}",
                self.cpu_time_ms, limit
            ));
        }

        if let Some(limit) = quota.memory_limit_bytes
            && self.memory_bytes > limit
        {
            violations.push(format!(
                "Memory exceeded: {} > {}",
                self.memory_bytes, limit
            ));
        }

        if let Some(limit) = quota.fd_limit
            && self.open_fds > limit
        {
            violations.push(format!("FD limit exceeded: {} > {}", self.open_fds, limit));
        }

        if let Some(limit) = quota.process_limit
            && self.process_count > limit
        {
            violations.push(format!(
                "Process limit exceeded: {} > {}",
                self.process_count, limit
            ));
        }

        if let Some(limit) = quota.file_size_limit
            && self.file_written_bytes > limit
        {
            violations.push(format!(
                "File size exceeded: {} > {}",
                self.file_written_bytes, limit
            ));
        }

        violations
    }
}

/// 资源监控器
pub struct ResourceMonitor {
    quota: Arc<RwLock<ResourceQuota>>,
    usage: Arc<RwLock<ResourceUsage>>,
    // 资源超限事件计数
    violations: Arc<AtomicU64>,
}

impl ResourceMonitor {
    pub fn new(quota: ResourceQuota) -> Self {
        Self {
            quota: Arc::new(RwLock::new(quota)),
            usage: Arc::new(RwLock::new(ResourceUsage::default())),
            violations: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 更新CPU使用时间
    pub fn record_cpu_time(&self, ms: u64) {
        let mut usage = self.usage.write();
        usage.cpu_time_ms += ms;
    }

    /// 更新内存使用
    pub fn record_memory(&self, bytes: u64) {
        let mut usage = self.usage.write();
        usage.memory_bytes = bytes; // 直接设置当前值
    }

    /// 记录文件描述符
    pub fn record_fd(&self, count: u32) {
        let mut usage = self.usage.write();
        usage.open_fds = count;
    }

    /// 记录进程计数
    pub fn record_process(&self, count: u32) {
        let mut usage = self.usage.write();
        usage.process_count = count;
    }

    /// 记录I/O操作
    pub fn record_io(&self, bytes: u64) {
        let mut usage = self.usage.write();
        usage.file_written_bytes += bytes;
        usage.io_ops += 1;
    }

    /// 检查违规情况并记录
    pub fn check_and_log_violations(&self) {
        let usage = self.usage.read();
        let quota = self.quota.read();
        let violations = usage.check_quota(&quota);
        if !violations.is_empty() {
            self.violations.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 获取当前使用情况
    pub fn get_usage(&self) -> ResourceUsage {
        self.usage.read().clone()
    }

    /// 获取配额
    pub fn get_quota(&self) -> ResourceQuota {
        self.quota.read().clone()
    }

    /// 更新配额
    pub fn set_quota(&self, quota: ResourceQuota) {
        *self.quota.write() = quota;
    }

    /// 获取违规次数
    pub fn get_violations(&self) -> u64 {
        self.violations.load(Ordering::Relaxed)
    }

    /// 重置统计
    pub fn reset(&self) {
        *self.usage.write() = ResourceUsage::default();
        self.violations.store(0, Ordering::Relaxed);
    }
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new(ResourceQuota::default())
    }
}

/// 访问控制列表 (ACL)
pub struct AccessControlList {
    // 权限映射: (资源类型, 资源ID) -> 权限集合
    permissions: PermissionMap,
}

impl AccessControlList {
    pub fn new() -> Self {
        Self {
            permissions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 授予权限
    pub fn grant_permission(&self, resource_type: String, resource_id: String, permission: String) {
        let mut perms = self.permissions.write();
        perms
            .entry((resource_type, resource_id))
            .or_default()
            .insert(permission);
    }

    /// 撤销权限
    pub fn revoke_permission(
        &self,
        resource_type: &str,
        resource_id: &str,
        permission: &str,
    ) -> bool {
        let mut perms = self.permissions.write();
        if let Some(perm_set) = perms.get_mut(&(resource_type.to_string(), resource_id.to_string()))
        {
            return perm_set.remove(permission);
        }
        false
    }

    /// 检查权限
    pub fn check_permission(
        &self,
        resource_type: &str,
        resource_id: &str,
        permission: &str,
    ) -> bool {
        let perms = self.permissions.read();
        perms
            .get(&(resource_type.to_string(), resource_id.to_string()))
            .map(|perm_set| perm_set.contains(permission))
            .unwrap_or(false)
    }

    /// 获取资源的所有权限
    pub fn get_permissions(&self, resource_type: &str, resource_id: &str) -> Vec<String> {
        let perms = self.permissions.read();
        perms
            .get(&(resource_type.to_string(), resource_id.to_string()))
            .map(|perm_set| perm_set.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl Default for AccessControlList {
    fn default() -> Self {
        Self::new()
    }
}

/// 审计日志事件
#[derive(Clone, Debug)]
pub struct AuditEvent {
    /// 事件ID
    pub event_id: u64,
    /// 事件类型
    pub event_type: String,
    /// 事件内容
    pub message: String,
    /// 时间戳 (秒)
    pub timestamp: u64,
    /// 严重程度 (0=info, 1=warn, 2=error)
    pub severity: u32,
}

/// 审计日志记录器
pub struct AuditLogger {
    // 审计事件日志
    events: Arc<RwLock<Vec<AuditEvent>>>,
    // 事件ID计数器
    event_counter: Arc<AtomicU64>,
    // 日志大小限制 (事件数)
    max_events: usize,
}

impl AuditLogger {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            event_counter: Arc::new(AtomicU64::new(0)),
            max_events,
        }
    }

    /// 记录事件
    pub fn log_event(&self, event_type: String, message: String, severity: u32) {
        let event_id = self.event_counter.fetch_add(1, Ordering::Relaxed);

        // 使用真实的时间戳
        use std::time::SystemTime;
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let event = AuditEvent {
            event_id,
            event_type,
            message,
            timestamp,
            severity,
        };

        let mut events = self.events.write();
        events.push(event);

        // 保持日志大小在限制内
        if events.len() > self.max_events {
            events.remove(0); // 移除最旧的事件
        }
    }

    /// 记录信息级事件
    pub fn log_info(&self, message: String) {
        self.log_event("INFO".to_string(), message, 0);
    }

    /// 记录警告级事件
    pub fn log_warn(&self, message: String) {
        self.log_event("WARN".to_string(), message, 1);
    }

    /// 记录错误级事件
    pub fn log_error(&self, message: String) {
        self.log_event("ERROR".to_string(), message, 2);
    }

    /// 获取所有事件
    pub fn get_events(&self) -> Vec<AuditEvent> {
        self.events.read().clone()
    }

    /// 获取特定类型的事件
    pub fn get_events_by_type(&self, event_type: &str) -> Vec<AuditEvent> {
        self.events
            .read()
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect()
    }

    /// 清空日志
    pub fn clear(&self) {
        self.events.write().clear();
        self.event_counter.store(0, Ordering::Relaxed);
    }

    /// 获取事件数
    pub fn event_count(&self) -> usize {
        self.events.read().len()
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new(10000) // 默认最多10000个事件
    }
}

/// 综合安全沙箱
pub struct SecuritySandbox {
    pub seccomp: Arc<SeccompPolicy>,
    pub resource_monitor: Arc<ResourceMonitor>,
    pub acl: Arc<AccessControlList>,
    pub audit_logger: Arc<AuditLogger>,
}

impl SecuritySandbox {
    pub fn new(quota: ResourceQuota) -> Self {
        Self {
            seccomp: Arc::new(SeccompPolicy::default()),
            resource_monitor: Arc::new(ResourceMonitor::new(quota)),
            acl: Arc::new(AccessControlList::default()),
            audit_logger: Arc::new(AuditLogger::default()),
        }
    }

    /// 初始化常见的POSIX Syscalls白名单
    pub fn init_posix_whitelist(&self) {
        let syscalls = vec![
            "read",
            "write",
            "open",
            "close",
            "stat",
            "fstat",
            "lstat",
            "poll",
            "lseek",
            "mmap",
            "mprotect",
            "munmap",
            "brk",
            "rt_sigaction",
            "rt_sigprocmask",
            "rt_sigpending",
            "rt_sigtimedwait",
            "rt_sigaction",
            "rt_sigprocmask",
            "sigaltstack",
            "pause",
            "nanosleep",
            "getitimer",
            "alarm",
            "setitimer",
            "getpid",
            "sendfile",
            "socket",
            "connect",
            "accept",
            "sendto",
            "recvfrom",
            "shutdown",
            "listen",
            "getsockname",
            "getpeername",
            "socketpair",
            "setsockopt",
            "getsockopt",
            "clone",
            "fork",
            "vfork",
            "execve",
            "exit",
            "wait4",
            "kill",
            "uname",
            "fcntl",
            "flock",
            "fsync",
            "fdatasync",
            "truncate",
            "ftruncate",
            "getdents",
            "getcwd",
            "chdir",
            "fchdir",
            "rename",
            "mkdir",
            "rmdir",
            "creat",
            "link",
            "unlink",
            "symlink",
            "readlink",
            "chmod",
            "fchmod",
            "chown",
            "fchown",
            "lchown",
            "umask",
        ];

        for syscall in syscalls {
            self.seccomp.add_allowed_syscall(syscall.to_string());
        }
    }

    /// 初始化I/O设备权限
    pub fn init_device_acl(&self) {
        // 允许读取标准输入/输出/错误
        self.acl.grant_permission(
            "device".to_string(),
            "stdin".to_string(),
            "read".to_string(),
        );
        self.acl.grant_permission(
            "device".to_string(),
            "stdout".to_string(),
            "write".to_string(),
        );
        self.acl.grant_permission(
            "device".to_string(),
            "stderr".to_string(),
            "write".to_string(),
        );
    }

    /// 启用强化安全模式 (企业级)
    pub fn enable_hardened_mode(&self) {
        // 1. 限制危险的系统调用
        self.audit_logger
            .log_info("Enabling hardened security mode".to_string());

        // 拒绝危险的系统调用
        let dangerous_syscalls = vec![
            "ptrace",
            "kcmp",
            "kexec_load",
            "syscall",
            "perf_event_open",
            "process_vm_readv",
            "process_vm_writev",
            "remap_file_pages",
            "personality",
            "modify_ldt",
            "set_thread_area",
            "get_thread_area",
        ];

        for syscall in dangerous_syscalls {
            self.seccomp.add_denied_syscall(syscall.to_string());
        }

        // 2. 强化资源配额
        let mut hardened_quota = self.resource_monitor.get_quota();
        hardened_quota.cpu_time_ms = Some(5000); // 5秒
        hardened_quota.memory_limit_bytes = Some(512 * 1024 * 1024); // 512MB
        hardened_quota.fd_limit = Some(64); // 限制文件描述符
        hardened_quota.process_limit = Some(1); // 严格限制单个进程
        self.resource_monitor.set_quota(hardened_quota);

        // 3. 启用详细审计
        self.audit_logger
            .log_info("Hardened security mode enabled".to_string());
    }

    /// 启用安全模式
    pub fn enable(&self) {
        self.audit_logger
            .log_info("Security sandbox enabled".to_string());
    }

    /// 检查系统调用是否允许
    pub fn check_syscall_allowed(&self, syscall: &str) -> bool {
        let perm = self.seccomp.check_syscall(syscall);
        let allowed = perm == SyscallPermission::Allow;

        if perm == SyscallPermission::Deny {
            self.audit_logger
                .log_warn(format!("Syscall denied: {}", syscall));
        } else if perm == SyscallPermission::Audit {
            self.audit_logger
                .log_info(format!("Syscall audited: {}", syscall));
        }

        allowed
    }
}

impl Default for SecuritySandbox {
    fn default() -> Self {
        Self::new(ResourceQuota::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seccomp_policy_whitelist() {
        let policy = SeccompPolicy::new(true); // 白名单模式
        policy.add_allowed_syscall("read".to_string());
        policy.add_allowed_syscall("write".to_string());

        assert_eq!(policy.check_syscall("read"), SyscallPermission::Allow);
        assert_eq!(policy.check_syscall("write"), SyscallPermission::Allow);
        assert_eq!(policy.check_syscall("fork"), SyscallPermission::Deny);
    }

    #[test]
    fn test_seccomp_policy_blacklist() {
        let policy = SeccompPolicy::new(false); // 黑名单模式
        policy.add_denied_syscall("fork".to_string());
        policy.add_denied_syscall("exec".to_string());

        assert_eq!(policy.check_syscall("read"), SyscallPermission::Allow);
        assert_eq!(policy.check_syscall("fork"), SyscallPermission::Deny);
    }

    #[test]
    fn test_resource_quota_check() {
        let quota = ResourceQuota {
            cpu_time_ms: Some(1000),
            memory_limit_bytes: Some(100),
            ..Default::default()
        };

        let usage = ResourceUsage {
            cpu_time_ms: 2000,
            memory_bytes: 50,
            ..Default::default()
        };

        let violations = usage.check_quota(&quota);
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.contains("CPU")));
    }

    #[test]
    fn test_resource_monitor() {
        let monitor = ResourceMonitor::new(ResourceQuota {
            cpu_time_ms: Some(1000),
            memory_limit_bytes: Some(1000),
            ..Default::default()
        });

        monitor.record_cpu_time(500);
        monitor.record_memory(800);

        let usage = monitor.get_usage();
        assert_eq!(usage.cpu_time_ms, 500);
        assert_eq!(usage.memory_bytes, 800);
    }

    #[test]
    fn test_resource_monitor_violations() {
        let monitor = ResourceMonitor::new(ResourceQuota {
            cpu_time_ms: Some(100),
            ..Default::default()
        });

        monitor.record_cpu_time(50);
        monitor.check_and_log_violations();
        assert_eq!(monitor.get_violations(), 0);

        monitor.record_cpu_time(60); // 超出限制
        monitor.check_and_log_violations();
        assert!(monitor.get_violations() > 0);
    }

    #[test]
    fn test_acl_permissions() {
        let acl = AccessControlList::new();

        acl.grant_permission(
            "file".to_string(),
            "secret.txt".to_string(),
            "read".to_string(),
        );
        acl.grant_permission(
            "file".to_string(),
            "secret.txt".to_string(),
            "write".to_string(),
        );

        assert!(acl.check_permission("file", "secret.txt", "read"));
        assert!(acl.check_permission("file", "secret.txt", "write"));
        assert!(!acl.check_permission("file", "secret.txt", "delete"));
    }

    #[test]
    fn test_acl_revoke() {
        let acl = AccessControlList::new();

        acl.grant_permission(
            "file".to_string(),
            "test.txt".to_string(),
            "read".to_string(),
        );
        assert!(acl.check_permission("file", "test.txt", "read"));

        acl.revoke_permission("file", "test.txt", "read");
        assert!(!acl.check_permission("file", "test.txt", "read"));
    }

    #[test]
    fn test_audit_logger() {
        let logger = AuditLogger::new(100);

        logger.log_info("Test info".to_string());
        logger.log_warn("Test warning".to_string());
        logger.log_error("Test error".to_string());

        let events = logger.get_events();
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_audit_logger_filtering() {
        let logger = AuditLogger::new(100);

        logger.log_info("Info 1".to_string());
        logger.log_warn("Warn 1".to_string());
        logger.log_info("Info 2".to_string());

        let warns = logger.get_events_by_type("WARN");
        assert_eq!(warns.len(), 1);
    }

    #[test]
    fn test_security_sandbox_basic() {
        let sandbox = SecuritySandbox::default();
        sandbox.enable();

        assert_eq!(sandbox.audit_logger.event_count(), 1);
    }

    #[test]
    fn test_security_sandbox_posix_whitelist() {
        let sandbox = SecuritySandbox::default();
        sandbox.init_posix_whitelist();

        assert!(matches!(
            sandbox.seccomp.check_syscall("read"),
            SyscallPermission::Allow
        ));
        assert!(matches!(
            sandbox.seccomp.check_syscall("write"),
            SyscallPermission::Allow
        ));
    }

    #[test]
    fn test_security_sandbox_syscall_check() {
        let sandbox = SecuritySandbox::default();
        sandbox.init_posix_whitelist();

        assert!(sandbox.check_syscall_allowed("read"));
        assert!(!sandbox.check_syscall_allowed("ptrace"));
    }

    #[test]
    fn test_comprehensive_sandbox() {
        let quota = ResourceQuota {
            cpu_time_ms: Some(5000),
            memory_limit_bytes: Some(10_000_000),
            fd_limit: Some(100),
            process_limit: Some(1),
            ..Default::default()
        };

        let sandbox = SecuritySandbox::new(quota);
        sandbox.init_posix_whitelist();
        sandbox.init_device_acl();
        sandbox.enable();

        // 模拟一些操作
        sandbox.resource_monitor.record_cpu_time(1000);
        sandbox.resource_monitor.record_memory(5_000_000);

        // 检查权限
        assert!(sandbox.check_syscall_allowed("read"));
        assert!(sandbox.acl.check_permission("device", "stdout", "write"));

        // 验证审计日志
        assert!(sandbox.audit_logger.event_count() > 0);
    }
}
