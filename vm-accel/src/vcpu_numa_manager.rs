//! Integrated vCPU and NUMA Manager
//!
//! Combines vCPU affinity management with NUMA memory allocation for optimal performance

use crate::vcpu_affinity::{
    CPUTopology, NUMAAwareAllocator, VCPUAffinityManager, VCPUThreadConfig,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Combined vCPU and NUMA manager
///
/// This manager coordinates vCPU placement with NUMA memory allocation
/// to ensure optimal memory locality and performance.
pub struct VcpuNumaManager {
    /// NUMA topology information
    numa_topology: NumaTopology,
    /// vCPU to CPU affinity mapping
    vcpu_affinity: HashMap<u32, u32>,
    /// vCPU affinity manager
    affinity_manager: Arc<VCPUAffinityManager>,
    /// NUMA-aware memory allocator
    memory_allocator: Arc<RwLock<NUMAAwareAllocator>>,
    /// Per-node CPU mapping
    node_cpus: HashMap<u32, Vec<u32>>,
}

/// NUMA topology representation
#[derive(Clone, Debug)]
pub struct NumaTopology {
    /// Number of NUMA nodes
    num_nodes: u32,
    /// CPUs belonging to each NUMA node
    node_cpus: HashMap<u32, Vec<u32>>,
    /// Memory available per node (bytes)
    node_memory: HashMap<u32, u64>,
}

impl NumaTopology {
    /// Detect system NUMA topology
    pub fn detect() -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        {
            // Try to detect actual NUMA topology on Linux
            let mut node_cpus = HashMap::new();
            let mut node_memory = HashMap::new();

            // For simplicity, use a default 2-node topology if detection fails
            // In production, this would read from /sys/devices/system/node/
            if let Ok(entries) = std::fs::read_dir("/sys/devices/system/node") {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("node") {
                        if let Some(node_num) = name_str
                            .strip_prefix("node")
                            .and_then(|s| s.parse::<u32>().ok())
                        {
                            // Read CPUs for this node
                            let cpu_path = entry.path().join("cpulist");
                            if let Ok(cpus_str) = std::fs::read_to_string(&cpu_path) {
                                let cpus: Vec<u32> = cpus_str
                                    .trim()
                                    .split(',')
                                    .filter_map(|s| s.parse().ok())
                                    .collect();
                                node_cpus.insert(node_num, cpus);
                            }

                            // Read memory size for this node
                            let mem_path = entry.path().join("meminfo");
                            if let Ok(meminfo) = std::fs::read_to_string(&mem_path) {
                                // Parse meminfo to get total memory
                                for line in meminfo.lines() {
                                    if line.contains("MemTotal:") {
                                        let parts: Vec<&str> = line.split_whitespace().collect();
                                        if parts.len() >= 3 {
                                            if let Ok(kb) = parts[2].parse::<u64>() {
                                                node_memory.insert(node_num, kb * 1024);
                                            }
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !node_cpus.is_empty() {
                return Ok(Self {
                    num_nodes: node_cpus.len() as u32,
                    node_cpus,
                    node_memory,
                });
            }
        }

        // Fallback: default 2-node topology for non-Linux or detection failure
        let mut node_cpus = HashMap::new();
        let mut node_memory = HashMap::new();

        node_cpus.insert(0, vec![0, 1, 2, 3]);
        node_cpus.insert(1, vec![4, 5, 6, 7]);
        node_memory.insert(0, 8 * 1024 * 1024 * 1024); // 8GB
        node_memory.insert(1, 8 * 1024 * 1024 * 1024); // 8GB

        Ok(Self {
            num_nodes: 2,
            node_cpus,
            node_memory,
        })
    }

    /// Get CPUs belonging to a specific NUMA node
    pub fn get_node_cpus(&self, node_id: u32) -> Result<&Vec<u32>, String> {
        self.node_cpus
            .get(&node_id)
            .ok_or_else(|| format!("Invalid NUMA node: {}", node_id))
    }

    /// Get number of NUMA nodes
    pub fn num_nodes(&self) -> u32 {
        self.num_nodes
    }

    /// Get memory size for a specific node
    pub fn get_node_memory(&self, node_id: u32) -> Option<u64> {
        self.node_memory.get(&node_id).copied()
    }
}

impl VcpuNumaManager {
    /// Create a new integrated vCPU/NUMA manager
    pub fn new() -> Result<Self, String> {
        let numa_topology = NumaTopology::detect()?;
        let topology = Arc::new(CPUTopology::detect());
        let affinity_manager = Arc::new(VCPUAffinityManager::new_with_topology(topology));

        // Calculate memory per node from NUMA topology
        let memory_per_node = numa_topology
            .get_node_memory(0)
            .unwrap_or(8 * 1024 * 1024 * 1024);
        let memory_allocator = Arc::new(RwLock::new(NUMAAwareAllocator::new(
            numa_topology.num_nodes() as usize,
            memory_per_node,
        )));

        // Build node_cpus map
        let mut node_cpus = HashMap::new();
        for (&node, cpus) in numa_topology.node_cpus.iter() {
            node_cpus.insert(node, cpus.clone());
        }

        Ok(Self {
            numa_topology,
            vcpu_affinity: HashMap::new(),
            affinity_manager,
            memory_allocator,
            node_cpus,
        })
    }

    /// Bind vCPU to local NUMA node
    ///
    /// This method pins a vCPU to a CPU on the specified NUMA node,
    /// ensuring that memory accesses are local to that node.
    pub fn bind_vcpu_to_numa_node(&mut self, vcpu_id: u32, numa_node: u32) -> Result<(), String> {
        // Find CPUs on this NUMA node
        let cpus = self.numa_topology.get_node_cpus(numa_node)?;
        let cpus_count = cpus.len();
        let first_cpu = cpus.first().copied();
        if let Some(cpu_id) = first_cpu {
            self.vcpu_affinity.insert(vcpu_id, cpu_id);

            // Update the affinity manager configuration
            let mut config = VCPUThreadConfig::new(vcpu_id as usize, cpus_count);
            config.set_affinity(&[cpu_id as usize]);
            config.set_numa_node(numa_node as usize);
            self.affinity_manager.set_vcpu_config(config)?;

            // Set CPU affinity
            self.set_vcpu_affinity(vcpu_id, cpu_id)?;

            Ok(())
        } else {
            Err(format!("No CPUs available on NUMA node {}", numa_node))
        }
    }

    /// Set vCPU affinity to a specific CPU
    fn set_vcpu_affinity(&mut self, _vcpu_id: u32, _cpu_id: u32) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        unsafe {
            // Use sched_setaffinity to pin the thread
            let cpu_set = libc::cpu_set_t {
                bits: [
                    1u64 << (cpu_id as u64),
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ],
            };

            let ret = libc::sched_setaffinity(
                0, // current thread
                std::mem::size_of::<libc::cpu_set_t>(),
                &cpu_set as *const libc::cpu_set_t,
            );

            if ret != 0 {
                return Err(format!(
                    "Failed to set CPU affinity: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On non-Linux, just record the affinity
            // Actual pinning would need platform-specific code
        }

        Ok(())
    }

    /// Allocate memory on specific NUMA node
    ///
    /// This allocates memory on the specified NUMA node for optimal locality
    /// with vCPUs pinned to that node.
    pub fn allocate_numa_memory(&mut self, numa_node: u32, size: u64) -> Result<*mut u8, String> {
        let allocator = self
            .memory_allocator
            .write()
            .map_err(|e| format!("Failed to acquire memory allocator lock: {}", e))?;

        let addr = allocator.alloc_from_node(numa_node as usize, size)?;

        // Convert numeric address to pointer (simplified)
        // In production, this would use actual memory allocation
        Ok(addr as *mut u8)
    }

    /// Allocate memory local to a vCPU
    ///
    /// Automatically determines the NUMA node for a vCPU and allocates
    /// memory on that node for optimal performance.
    pub fn allocate_vcpu_local_memory(
        &mut self,
        vcpu_id: u32,
        size: u64,
    ) -> Result<*mut u8, String> {
        // Find which CPU this vCPU is pinned to
        let cpu_id = self
            .vcpu_affinity
            .get(&vcpu_id)
            .ok_or_else(|| format!("vCPU {} not bound to any CPU", vcpu_id))?;

        // Find the NUMA node for this CPU
        let numa_node = self.find_cpu_node(*cpu_id)?;

        // Allocate memory on that node
        self.allocate_numa_memory(numa_node, size)
    }

    /// Find which NUMA node a CPU belongs to
    fn find_cpu_node(&self, cpu_id: u32) -> Result<u32, String> {
        for (&node, cpus) in self.node_cpus.iter() {
            if cpus.contains(&cpu_id) {
                return Ok(node);
            }
        }
        Err(format!("CPU {} not found in any NUMA node", cpu_id))
    }

    /// Get the NUMA node for a vCPU
    pub fn get_vcpu_numa_node(&self, vcpu_id: u32) -> Result<u32, String> {
        let cpu_id = self
            .vcpu_affinity
            .get(&vcpu_id)
            .ok_or_else(|| format!("vCPU {} not bound to any CPU", vcpu_id))?;

        self.find_cpu_node(*cpu_id)
    }

    /// Configure multiple vCPUs with automatic NUMA placement
    ///
    /// Distributes vCPUs across available NUMA nodes for load balancing.
    pub fn configure_vcpus(&mut self, vcpu_count: u32) -> Result<(), String> {
        let num_nodes = self.numa_topology.num_nodes();

        for vcpu_id in 0..vcpu_count {
            // Round-robin placement across NUMA nodes
            let numa_node = vcpu_id % num_nodes;
            self.bind_vcpu_to_numa_node(vcpu_id, numa_node)?;
        }

        Ok(())
    }

    /// Get affinity manager reference
    pub fn affinity_manager(&self) -> &Arc<VCPUAffinityManager> {
        &self.affinity_manager
    }

    /// Get memory allocator reference
    pub fn memory_allocator(&self) -> &Arc<RwLock<NUMAAwareAllocator>> {
        &self.memory_allocator
    }

    /// Get NUMA topology reference
    pub fn numa_topology(&self) -> &NumaTopology {
        &self.numa_topology
    }

    /// Generate diagnostic report
    pub fn diagnostic_report(&self) -> String {
        let mut report = "=== vCPU/NUMA Integration Report ===\n\n".to_string();

        report.push_str(&format!(
            "NUMA Nodes: {}\n\n",
            self.numa_topology.num_nodes()
        ));

        // Report vCPU bindings
        report.push_str("vCPU Bindings:\n");
        if self.vcpu_affinity.is_empty() {
            report.push_str("  No vCPUs configured\n");
        } else {
            for (vcpu_id, cpu_id) in self.vcpu_affinity.iter() {
                if let Ok(node) = self.find_cpu_node(*cpu_id) {
                    report.push_str(&format!(
                        "  vCPU {} -> CPU {} (Node {})\n",
                        vcpu_id, cpu_id, node
                    ));
                }
            }
        }

        report.push('\n');

        // Report memory usage
        if let Ok(allocator) = self.memory_allocator.read() {
            let alloc_report = allocator.diagnostic_report();
            report.push_str(&alloc_report);
        }

        report.push('\n');

        // Report affinity manager status
        report.push_str(&self.affinity_manager.diagnostic_report());

        report
    }
}

impl Default for VcpuNumaManager {
    fn default() -> Self {
        Self::new().expect("Failed to create VcpuNumaManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numa_topology_detection() {
        let topology = NumaTopology::detect().expect("Should detect topology");
        assert!(topology.num_nodes() > 0);

        // Check that we can get CPUs for each node
        for node in 0..topology.num_nodes() {
            let cpus = topology.get_node_cpus(node);
            if cpus.is_ok() {
                assert!(!cpus.unwrap().is_empty());
            }
        }
    }

    #[test]
    fn test_vcpu_numa_manager_creation() {
        let manager = VcpuNumaManager::new();
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert!(manager.numa_topology.num_nodes() > 0);
    }

    #[test]
    fn test_bind_vcpu_to_numa_node() {
        let mut manager = VcpuNumaManager::new().expect("Should create manager");

        let result = manager.bind_vcpu_to_numa_node(0, 0);
        assert!(result.is_ok());

        // Check that vCPU was bound
        assert!(manager.vcpu_affinity.contains_key(&0));

        // Check that we can query the NUMA node
        let node = manager.get_vcpu_numa_node(0);
        assert!(node.is_ok());
        assert_eq!(node.unwrap(), 0);
    }

    #[test]
    fn test_configure_multiple_vcpus() {
        let mut manager = VcpuNumaManager::new().expect("Should create manager");

        let result = manager.configure_vcpus(4);
        assert!(result.is_ok());

        // Check that all vCPUs were configured
        assert_eq!(manager.vcpu_affinity.len(), 4);

        // Check that vCPUs are distributed across nodes
        for vcpu_id in 0..4 {
            assert!(manager.vcpu_affinity.contains_key(&vcpu_id));
            let node = manager.get_vcpu_numa_node(vcpu_id);
            assert!(node.is_ok());
        }
    }

    #[test]
    fn test_allocate_numa_memory() {
        let mut manager = VcpuNumaManager::new().expect("Should create manager");

        let result = manager.allocate_numa_memory(0, 1024 * 1024); // 1MB
        assert!(result.is_ok());

        let ptr = result.unwrap();
        // Note: Simplified implementation returns address as a number, may be 0
        assert_eq!(ptr as usize, 0); // Simplified: returns node_id * 1GB
    }

    #[test]
    fn test_allocate_vcpu_local_memory() {
        let mut manager = VcpuNumaManager::new().expect("Should create manager");

        // First bind a vCPU
        manager
            .bind_vcpu_to_numa_node(0, 0)
            .expect("Should bind vCPU");

        // Then allocate memory local to that vCPU
        let result = manager.allocate_vcpu_local_memory(0, 1024 * 1024); // 1MB
        assert!(result.is_ok());

        let ptr = result.unwrap();
        // Note: Simplified implementation returns address as a number
        assert_eq!(ptr as usize, 0); // Node 0 returns 0
    }

    #[test]
    fn test_diagnostic_report() {
        let mut manager = VcpuNumaManager::new().expect("Should create manager");

        // Configure some vCPUs
        manager.configure_vcpus(2).expect("Should configure vCPUs");

        // Generate report
        let report = manager.diagnostic_report();

        assert!(report.contains("vCPU/NUMA Integration Report"));
        assert!(report.contains("vCPU Bindings"));
    }

    #[test]
    fn test_vcpu_not_bound_error() {
        let mut manager = VcpuNumaManager::new().expect("Should create manager");

        // Try to allocate memory for an unbound vCPU
        let result = manager.allocate_vcpu_local_memory(99, 1024);
        assert!(result.is_err());

        // Try to get NUMA node for an unbound vCPU
        let result = manager.get_vcpu_numa_node(99);
        assert!(result.is_err());
    }
}
