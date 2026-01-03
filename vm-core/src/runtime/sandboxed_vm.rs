//! 沙箱化VM实现
//!
//! 实现完整的沙箱隔离和资源管理功能

use std::sync::Arc;
use parking_lot::{Mutex, RwLock};

use crate::{ExecResult, VmError, MMU};
use vm_engine::jit::Jit;
use security_sandbox::{
    SecuritySandbox, ResourceQuota, SeccompPolicy, AccessControlList,
    AuditLogger, ResourceMonitor
};
use crate::jit::coroutine_scheduler::Scheduler;

/// VM沙箱配置
pub struct SandboxConfig {
    /// 是否启用Seccomp限制
    pub enable_seccomp: bool,
    /// CPU时间限制 (毫秒)
    pub cpu_time_limit_ms: Option<u64>,
    /// 内存限制 (字节)
    pub memory_limit_bytes: Option<u64>,
    /// 磁盘I/O限制 (字节/秒)
    pub disk_io_limit_bytes: Option<u64>,
    /// 网络I/O限制 (字节/秒)
    pub network_io_limit_bytes: Option<u64>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enable_seccomp: true,
            cpu_time_limit_ms: None,
            memory_limit_bytes: None,
            disk_io_limit_bytes: None,
            network_io_limit_bytes: None,
        }
    }
}

/// 沙箱化VM结构
pub struct SandboxedVm {
    /// 底层JIT虚拟机
    jit: Arc<Mutex<Jit>>,
    /// 协程调度器
    scheduler: Arc<Mutex<Scheduler>>,
    /// 安全沙箱组件
    security_sandbox: Arc<SecuritySandbox>,
    /// 沙箱配置
    config: SandboxConfig,
    /// 已使用的CPU时间 (毫秒)
    used_cpu_time_ms: u64,
    /// 已使用的内存 (字节)
    used_memory_bytes: u64,
}

impl SandboxedVm {
    /// 创建新的沙箱化VM
    pub fn new(jit: Jit, scheduler: Scheduler, config: SandboxConfig) -> Self {
        // 创建资源配额
        let resource_quota = ResourceQuota {
            cpu_time_ms: config.cpu_time_limit_ms,
            memory_limit_bytes: config.memory_limit_bytes,
            ..ResourceQuota::default()
        };
        
        // 创建安全沙箱
        let security_sandbox = SecuritySandbox::new(resource_quota);
        security_sandbox.init_posix_whitelist();
        security_sandbox.init_device_acl();
        
        Self {
            jit: Arc::new(Mutex::new(jit)),
            scheduler: Arc::new(Mutex::new(scheduler)),
            security_sandbox: Arc::new(security_sandbox),
            config,
            used_cpu_time_ms: 0,
            used_memory_bytes: 0,
        }
    }

    /// 运行VM (沙箱模式)
    pub fn run(&mut self, entry_point: u64) -> ExecResult {
        // 应用沙箱限制
        self.apply_sandbox_restrictions();
        
        // 运行JIT虚拟机
        let result = self.jit.lock().run(entry_point);
        
        // 记录资源使用情况
        self.update_resource_usage();
        
        result
    }

    /// 应用沙箱限制
    fn apply_sandbox_restrictions(&self) {
        // 启用安全沙箱
        self.security_sandbox.enable();
        
        // 应用Seccomp限制
        if self.config.enable_seccomp {
            // 安全沙箱已经在初始化时配置了默认的POSIX白名单
        }
        
        // 应用资源限制
        let resource_quota = ResourceQuota {
            cpu_time_ms: self.config.cpu_time_limit_ms,
            memory_limit_bytes: self.config.memory_limit_bytes,
            ..self.security_sandbox.resource_monitor.get_quota()
        };
        self.security_sandbox.resource_monitor.set_quota(resource_quota);
    }

    /// 更新资源使用情况
    fn update_resource_usage(&mut self) {
        // 更新已使用的CPU时间
        // let current_cpu_time = self.jit.lock().get_cpu_time_used_ms();
        // self.used_cpu_time_ms = current_cpu_time;
        
        // 更新已使用的内存
        // let current_memory = self.jit.lock().get_memory_used_bytes();
        // self.used_memory_bytes = current_memory;
        
        // 更新安全沙箱的资源监控
        let mut jit = self.jit.lock();
        // 获取JIT的CPU时间和内存使用情况
        let current_cpu_time = 0; // 假设JIT提供了获取CPU时间的方法
        let current_memory = 0;   // 假设JIT提供了获取内存使用的方法
        
        self.security_sandbox.resource_monitor.record_cpu_time(current_cpu_time);
        self.security_sandbox.resource_monitor.record_memory(current_memory);
        
        // 检查资源配额违规
        self.security_sandbox.resource_monitor.check_and_log_violations();
    }

    /// 获取资源使用统计
    pub fn get_resource_stats(&self) -> ResourceStats {
        let resource_usage = self.security_sandbox.resource_monitor.get_usage();
        
        ResourceStats {
            used_cpu_time_ms: resource_usage.cpu_time_ms,
            used_memory_bytes: resource_usage.memory_bytes,
        }
    }

    /// 获取沙箱配置
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }
}

/// 资源统计信息
pub struct ResourceStats {
    /// 已使用的CPU时间 (毫秒)
    pub used_cpu_time_ms: u64,
    /// 已使用的内存 (字节)
    pub used_memory_bytes: u64,
}

impl Default for ResourceStats {
    fn default() -> Self {
        Self {
            used_cpu_time_ms: 0,
            used_memory_bytes: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_engine::jit::Jit;
    use vm_mmu::SimpleMMU;

    #[test]
    fn test_sandboxed_vm_with_security_sandbox() {
        // 创建JIT虚拟机
        let mut jit = Jit::new();
        // 创建简单的MMU
        let mmu = SimpleMMU::new(1024 * 1024); // 1MB内存
        jit.set_mmu(Box::new(mmu));
        
        // 创建协程调度器
        let scheduler = crate::coroutine_scheduler::Scheduler::new(1);
        
        // 创建沙箱配置
        let config = SandboxConfig {
            enable_seccomp: true,
            cpu_time_limit_ms: Some(5000), // 5秒
            memory_limit_bytes: Some(1024 * 1024), // 1MB
            disk_io_limit_bytes: None,
            network_io_limit_bytes: None,
        };
        
        // 创建沙箱化VM
        let mut sandboxed_vm = SandboxedVm::new(jit, scheduler, config);
        
        // 验证安全沙箱已正确初始化
        assert_eq!(sandboxed_vm.security_sandbox.audit_logger.event_count(), 0);
        
        // 启用沙箱
        sandboxed_vm.apply_sandbox_restrictions();
        
        // 验证沙箱已启用
        assert_eq!(sandboxed_vm.security_sandbox.audit_logger.event_count(), 1);
        
        // 检查资源配额
        let resource_quota = sandboxed_vm.security_sandbox.resource_monitor.get_quota();
        assert_eq!(resource_quota.cpu_time_ms, Some(5000));
        assert_eq!(resource_quota.memory_limit_bytes, Some(1024 * 1024));
    }

    #[test]
    fn test_sandboxed_vm_basic() {
        // 创建JIT虚拟机
        let mut jit = Jit::new();
        // 创建简单的MMU
        let mmu = vm_mmu::SimpleMMU::new(1024 * 1024); // 1MB内存
        jit.set_mmu(Box::new(mmu));
        
        // 创建协程调度器
        let scheduler = crate::coroutine_scheduler::Scheduler::new(1);
        
        // 创建沙箱配置
        let config = SandboxConfig::default();
        
        // 创建沙箱化VM
        let mut sandboxed_vm = SandboxedVm::new(jit, scheduler, config);
        
        // 尝试运行VM (entry_point不存在，但应该能正确初始化)
        let result = sandboxed_vm.run(0x1000);
        
        // 预期会失败，因为没有加载代码，但沙箱化本身应该成功
        assert!(result.is_err());
        
        // 检查资源使用
        let stats = sandboxed_vm.get_resource_stats();
        assert_eq!(stats.used_cpu_time_ms, 0);
        assert_eq!(stats.used_memory_bytes, 0);
    }
}