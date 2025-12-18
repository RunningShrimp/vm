//! Distributed Architecture Configuration
//!
//! This module defines the configuration for the distributed execution system.

use std::net::SocketAddr;

/// Distributed architecture configuration
#[derive(Debug, Clone)]
pub struct DistributedArchitectureConfig {
    /// Address for the VM coordinator
    pub coordinator_addr: SocketAddr,

    /// List of initial VM addresses to connect to
    pub initial_vm_addrs: Vec<SocketAddr>,

    /// Discovery service port
    pub discovery_port: u16,

    /// Communication port
    pub comm_port: u16,

    /// Load balancing strategy
    pub load_balancing_strategy: LoadBalancingStrategy,

    /// Fault tolerance configuration
    pub fault_tolerance: FaultToleranceConfig,
}

impl Default for DistributedArchitectureConfig {
    fn default() -> Self {
        DistributedArchitectureConfig {
            coordinator_addr: "127.0.0.1:8080".parse().unwrap(),
            initial_vm_addrs: Vec::new(),
            discovery_port: 8081,
            comm_port: 8082,
            load_balancing_strategy: LoadBalancingStrategy::RoundRobin,
            fault_tolerance: FaultToleranceConfig::default(),
        }
    }
}

/// Load balancing strategies
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadBalancingStrategy {
    /// Round robin task distribution
    RoundRobin,

    /// Least loaded VM first
    LeastLoaded,

    /// CPU intensive tasks go to VM with lowest CPU utilization
    CpuIntensive,

    /// Memory intensive tasks go to VM with lowest memory utilization
    MemoryIntensive,
}

/// Fault tolerance configuration
#[derive(Debug, Clone)]
pub struct FaultToleranceConfig {
    /// Maximum number of retry attempts for failed tasks
    pub max_retries: usize,

    /// Timeout for task execution
    pub task_timeout: std::time::Duration,

    /// Whether to automatically restart failed VMs
    pub auto_restart: bool,

    /// Check interval for VM health
    pub health_check_interval: std::time::Duration,
}

impl Default for FaultToleranceConfig {
    fn default() -> Self {
        FaultToleranceConfig {
            max_retries: 3,
            task_timeout: std::time::Duration::from_secs(30),
            auto_restart: false,
            health_check_interval: std::time::Duration::from_secs(5),
        }
    }
}
