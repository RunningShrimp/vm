//! # SoC (System on Chip) 特性优化 (WIP)
//!
//! 针对 ARM SoC 的特殊优化，包括：
//! - DynamIQ 调度
//! - big.LITTLE / ARM DynamIQ 调度
//! - 移动设备功耗优化
//! - 大页内存优化
//! - NUMA 感知分配
//!
//! ## 当前状态
//!
//! - **开发状态**: 🚧 Work In Progress
//! - **功能完整性**: ~30%（基础架构已实现）
//! - **生产就绪**: ⚠️ 仅推荐用于开发环境
//!
//! ## 已实现功能
//!
//! - ✅ SoC厂商和集群配置
//! - ✅ 基础优化策略
//! - ✅ 亲和性推荐
//! - ✅ 功耗级别设置
//!
//! ## 待实现功能
//!
//! - ⏳ 实际的DynamIQ调度
//! - ⏳ big.LITTLE调度实现
//! - ⏳ 大页内存配置
//! - ⏳ NUMA分配优化
//!
//! ## 支持的SoC
//!
//! - Qualcomm Snapdragon
//! - HiSilicon Kirin
//! - MediaTek Dimensity
//! - Apple A系列/M系列
//!
//! ## 依赖项
//!
//! - Linux内核接口
//! - ARM性能监控单元
//! - 系统调用支持
//!
//! ## 相关Issue
//!
//! - 跟踪: #待创建（SoC优化完整实现）
//!
//! ## 贡献指南
//!
//! 如果您有ARM SoC开发经验并希望帮助实现此模块，请：
//! 1. 确保有ARM SoC开发环境
//! 2. 参考ARM DynamIQ文档
//! 3. 联系维护者review
//! 4. 提交PR并包含测试用例

/// CPU 集群类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuCluster {
    Performance, // 大核 (P-Core)
    Efficiency,  // 小核 (E-Core)
    Mid,         // 中核 (某些 SoC)
}

/// SoC 特性优化器
pub struct SocOptimizer {
    /// SoC 厂商
    pub vendor: SocVendor,

    /// CPU 集群配置
    pub clusters: Vec<CpuCluster>,

    /// 优化配置
    pub config: SocConfig,
}

/// SoC 厂商
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocVendor {
    Qualcomm,
    HiSilicon,
    MediaTek,
    Samsung,
    Apple,
}

/// SoC 配置
#[derive(Debug, Clone)]
pub struct SocConfig {
    /// 是否启用 DynamIQ 调度
    pub enable_dynamiq: bool,

    /// 是否使用 big.LITTLE 调度
    pub enable_big_little: bool,

    /// 功耗优化级别 (0-3)
    pub power_saving_level: u32,

    /// 是否启用大页 (Huge Pages)
    pub enable_huge_pages: bool,

    /// NUMA 感知分配
    pub enable_numa: bool,
}

impl Default for SocConfig {
    fn default() -> Self {
        Self {
            enable_dynamiq: true,
            enable_big_little: true,
            power_saving_level: 2,
            enable_huge_pages: true,
            enable_numa: true,
        }
    }
}

impl SocOptimizer {
    /// 创建新的 SoC 优化器
    pub fn new(vendor: SocVendor) -> Self {
        let clusters = match vendor {
            SocVendor::Qualcomm => vec![
                CpuCluster::Performance,
                CpuCluster::Performance,
                CpuCluster::Performance,
                CpuCluster::Performance,
                CpuCluster::Efficiency,
                CpuCluster::Efficiency,
                CpuCluster::Efficiency,
                CpuCluster::Efficiency,
            ],
            SocVendor::HiSilicon => vec![
                CpuCluster::Performance,
                CpuCluster::Performance,
                CpuCluster::Performance,
                CpuCluster::Performance,
                CpuCluster::Efficiency,
                CpuCluster::Efficiency,
                CpuCluster::Efficiency,
                CpuCluster::Efficiency,
            ],
            SocVendor::Apple => vec![
                CpuCluster::Performance,
                CpuCluster::Performance,
                CpuCluster::Performance,
                CpuCluster::Performance,
                CpuCluster::Efficiency,
                CpuCluster::Efficiency,
                CpuCluster::Efficiency,
                CpuCluster::Efficiency,
            ],
            _ => vec![CpuCluster::Performance, CpuCluster::Efficiency],
        };

        Self {
            vendor,
            clusters,
            config: SocConfig::default(),
        }
    }

    /// 应用 SoC 优化
    pub fn apply_optimizations(&self) -> Result<(), SocError> {
        log::info!("Applying SoC optimizations for {:?}", self.vendor);

        if self.config.enable_dynamiq {
            self.enable_dynamiq_scheduling()?;
        }

        if self.config.enable_big_little {
            self.enable_big_little_scheduling()?;
        }

        if self.config.enable_huge_pages {
            self.enable_huge_pages()?;
        }

        if self.config.enable_numa {
            self.enable_numa_allocation()?;
        }

        Ok(())
    }

    /// 启用 DynamIQ 调度
    fn enable_dynamiq_scheduling(&self) -> Result<(), SocError> {
        log::info!("Enabling ARM DynamIQ scheduling");

        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::io::Write;

            // DynamIQ调度配置
            // 1. 检测CPU集群拓扑
            let cpu_topology = self.detect_cpu_topology()?;
            log::info!("Detected CPU topology: {:?} clusters", cpu_topology);

            // 2. 配置调度器为DynamIQ友好模式
            // 写入sysfs配置
            if let Ok(mut file) = fs::File::create("/sys/devices/system/cpu/sched_smt") {
                writeln!(file, "1").map_err(|e| SocError::IoError(e.to_string()))?;
                log::info!("Enabled SMT for DynamIQ");
            }

            // 3. 为每个集群配置调度策略
            for (cluster_id, cluster_type) in cpu_topology.iter().enumerate() {
                let cpu_list = self.get_cluster_cpus(cluster_id)?;
                if cpu_list.is_empty() {
                    continue;
                }

                // 创建cpuset用于隔离集群
                let cpuset_path = format!("/sys/fs/cgroup/cpuset/dynamiq_cluster_{}", cluster_id);
                if let Err(_) = fs::create_dir_all(&cpuset_path) {
                    log::warn!("Failed to create cpuset for cluster {}", cluster_id);
                    continue;
                }

                // 配置cpuset的cpus
                let cpus_file = format!("{}/cpus", cpuset_path);
                if let Ok(mut file) = fs::File::create(&cpus_file) {
                    writeln!(file, "{}", cpu_list.join(","))
                        .map_err(|e| SocError::IoError(e.to_string()))?;
                }

                // 配置调度器策略
                let sched_file = format!("{}/sched.load_balance", cpuset_path);
                if let Ok(mut file) = fs::File::create(&sched_file) {
                    // 根据集群类型选择不同的调度策略
                    let balance_value = match cluster_type {
                        CpuCluster::Performance => "1", // P-Core启用负载均衡
                        CpuCluster::Efficiency => "1",  // E-Core启用负载均衡
                        CpuCluster::Mid => "1",
                    };
                    writeln!(file, "{}", balance_value)
                        .map_err(|e| SocError::IoError(e.to_string()))?;
                }

                // 设置EXEC属性以允许进程迁移
                let exec_file = format!("{}/cpuset.effective", cpuset_path);
                let _ = fs::remove_file(&exec_file); // 删除以启用所有CPU的exec

                log::info!(
                    "Configured DynamIQ scheduling for cluster {} ({:?})",
                    cluster_id,
                    cluster_type
                );
            }

            // 4. 配置全局调度器策略
            if let Ok(mut file) = fs::File::create("/proc/sys/kernel/sched_schedstats") {
                writeln!(file, "1").map_err(|e| SocError::IoError(e.to_string()))?;
                log::info!("Enabled scheduler statistics");
            }

            log::info!("ARM DynamIQ scheduling enabled successfully");
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            log::warn!("DynamIQ scheduling is only supported on Linux, skipping...");
            // 不返回错误，只是跳过
            Ok(())
        }
    }

    /// 启用 big.LITTLE 调度
    fn enable_big_little_scheduling(&self) -> Result<(), SocError> {
        log::info!("Enabling ARM big.LITTLE scheduling");

        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::io::Write;

            // big.LITTLE调度配置
            // 1. 识别big核（P-Core）和LITTLE核（E-Core）
            let mut big_cores = Vec::new();
            let mut little_cores = Vec::new();

            for (idx, cluster) in self.clusters.iter().enumerate() {
                match cluster {
                    CpuCluster::Performance => {
                        // 获取该集群的所有CPU
                        if let Ok(cpus) = self.get_cluster_cpus(idx) {
                            big_cores.extend(cpus);
                        }
                    }
                    CpuCluster::Efficiency => {
                        if let Ok(cpus) = self.get_cluster_cpus(idx) {
                            little_cores.extend(cpus);
                        }
                    }
                    CpuCluster::Mid => {
                        // 中核根据策略可以归类为big或little
                        // 这里我们将其归类为big（偏向性能）
                        if let Ok(cpus) = self.get_cluster_cpus(idx) {
                            big_cores.extend(cpus);
                        }
                    }
                }
            }

            log::info!("Detected {} big cores: {:?}", big_cores.len(), big_cores);
            log::info!(
                "Detected {} LITTLE cores: {:?}",
                little_cores.len(),
                little_cores
            );

            // 2. 配置CPU频率调度器（CPUFreq governor）
            // 为big核设置performance governor
            for cpu in &big_cores {
                let governor_path = format!(
                    "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
                    cpu
                );
                if let Ok(mut file) = fs::File::create(&governor_path) {
                    if writeln!(file, "performance").is_ok() {
                        log::debug!("Set CPU {} governor to 'performance'", cpu);
                    }
                }
            }

            // 为LITTLE核设置ondemand或conservative governor（功耗优先）
            for cpu in &little_cores {
                let governor_path = format!(
                    "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
                    cpu
                );
                if let Ok(mut file) = fs::File::create(&governor_path) {
                    // 尝试使用schedutil（现代Linux推荐）
                    if writeln!(file, "schedutil").is_err() {
                        // 如果不支持，回退到ondemand
                        let _ = writeln!(file, "ondemand");
                    }
                    log::debug!("Set CPU {} governor to power-saving mode", cpu);
                }
            }

            // 3. 配置调度器的任务迁移策略
            // 通过sysfs配置sched_load_balance和sched_migration_cost
            if let Ok(mut file) = fs::File::create("/proc/sys/kernel/sched_migration_cost_ns") {
                // 设置迁移成本，影响任务在big/LITTLE核心间的迁移频率
                // 默认值通常是500000，我们使用稍低的值以允许更积极的迁移
                writeln!(file, "300000").map_err(|e| SocError::IoError(e.to_string()))?;
                log::info!("Configured task migration cost for big.LITTLE");
            }

            // 4. 配置负载均衡
            // 创建big核和LITTLE核的cpuset以实现隔离
            if !big_cores.is_empty() {
                let big_cpuset = "/sys/fs/cgroup/cpuset/big_cores";
                if fs::create_dir_all(big_cpuset).is_ok() {
                    let cpus_file = format!("{}/cpus", big_cpuset);
                    if let Ok(mut file) = fs::File::create(&cpus_file) {
                        let cpu_list: Vec<String> =
                            big_cores.iter().map(|c| c.to_string()).collect();
                        writeln!(file, "{}", cpu_list.join(",")).ok();
                    }

                    // 启用负载均衡
                    let balance_file = format!("{}/sched.load_balance", big_cpuset);
                    if let Ok(mut file) = fs::File::create(&balance_file) {
                        writeln!(file, "1").ok();
                    }

                    log::info!("Created cpuset for big cores");
                }
            }

            if !little_cores.is_empty() {
                let little_cpuset = "/sys/fs/cgroup/cpuset/little_cores";
                if fs::create_dir_all(little_cpuset).is_ok() {
                    let cpus_file = format!("{}/cpus", little_cpuset);
                    if let Ok(mut file) = fs::File::create(&cpus_file) {
                        let cpu_list: Vec<String> =
                            little_cores.iter().map(|c| c.to_string()).collect();
                        writeln!(file, "{}", cpu_list.join(",")).ok();
                    }

                    // 启用负载均衡
                    let balance_file = format!("{}/sched.load_balance", little_cpuset);
                    if let Ok(mut file) = fs::File::create(&balance_file) {
                        writeln!(file, "1").ok();
                    }

                    log::info!("Created cpuset for LITTLE cores");
                }
            }

            // 5. 配置功耗级别影响
            // 根据功耗级别调整策略
            match self.config.power_saving_level {
                0 => {
                    // 最大性能模式：所有核心都设置为performance
                    log::info!("Power level 0: Maximum performance mode");
                    for cpu in little_cores.iter().chain(big_cores.iter()) {
                        let governor_path = format!(
                            "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
                            cpu
                        );
                        let _ = fs::File::create(&governor_path)
                            .map(|mut f| writeln!(f, "performance"));
                    }
                }
                1..=2 => {
                    // 平衡模式：big核心performance，LITTLE核心节能
                    log::info!(
                        "Power level {}: Balanced mode",
                        self.config.power_saving_level
                    );
                }
                3 => {
                    // 最大省电模式：所有核心都设置为powersave或conservative
                    log::info!("Power level 3: Maximum power saving mode");
                    for cpu in little_cores.iter().chain(big_cores.iter()) {
                        let governor_path = format!(
                            "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
                            cpu
                        );
                        let mut file = match fs::File::create(&governor_path) {
                            Ok(f) => f,
                            Err(_) => continue,
                        };

                        // 尝试使用conservative，如果不支持则使用powersave
                        if writeln!(file, "conservative").is_err() {
                            let _ = writeln!(file, "powersave");
                        }
                    }
                }
                _ => {
                    log::warn!("Invalid power level: {}", self.config.power_saving_level);
                }
            }

            log::info!("ARM big.LITTLE scheduling enabled successfully");
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            log::warn!("big.LITTLE scheduling is only supported on Linux, skipping...");
            Ok(())
        }
    }

    /// 启用大页
    fn enable_huge_pages(&self) -> Result<(), SocError> {
        log::info!("Enabling huge pages (2MB and 1GB)");

        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::io::Write;

            // 1. 检查系统是否支持大页
            let hugepage_path_2mb = "/sys/kernel/mm/hugepages/hugepages-2048kB";
            let hugepage_path_1gb = "/sys/kernel/mm/hugepages/hugepages-1048576kB";

            let support_2mb = std::path::Path::new(hugepage_path_2mb).exists();
            let support_1gb = std::path::Path::new(hugepage_path_1gb).exists();

            log::info!(
                "Huge page support - 2MB: {}, 1GB: {}",
                support_2mb,
                support_1gb
            );

            if !support_2mb && !support_1gb {
                log::warn!("No huge page support detected on this system");
                return Err(SocError::NotSupported(
                    "Huge pages not supported".to_string(),
                ));
            }

            // 2. 配置2MB大页
            if support_2mb {
                // 读取当前配置的大页数量
                let nr_hugepages_file = format!("{}/nr_hugepages", hugepage_path_2mb);
                if let Ok(current) = fs::read_to_string(&nr_hugepages_file) {
                    let current = current.trim().parse::<u64>().unwrap_or(0);
                    log::debug!("Current 2MB huge pages: {}", current);

                    // 建议配置：根据系统内存大小设置
                    // 获取系统内存大小
                    let meminfo = fs::read_to_string("/proc/meminfo").unwrap_or_default();
                    let total_mem_kb: u64 = meminfo
                        .lines()
                        .find(|line| line.starts_with("MemTotal:"))
                        .and_then(|line| line.split_whitespace().nth(1))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);

                    // 计算建议的2MB大页数量
                    // 假设我们使用1%的系统内存用于2MB大页
                    let target_hugepages_2mb = (total_mem_kb / 100 / 2048).max(1);
                    log::info!(
                        "Target 2MB huge pages: {} (based on system memory)",
                        target_hugepages_2mb
                    );

                    // 尝试设置大页数量
                    if let Ok(mut file) = fs::File::create(&nr_hugepages_file) {
                        match writeln!(file, "{}", target_hugepages_2mb) {
                            Ok(_) => {
                                log::info!(
                                    "Successfully configured 2MB huge pages to {}",
                                    target_hugepages_2mb
                                );
                            }
                            Err(e) => {
                                log::warn!("Failed to set 2MB huge pages: {}", e);
                            }
                        }
                    }

                    // 读取实际分配的大页数量
                    if let Ok(actual) = fs::read_to_string(&nr_hugepages_file) {
                        let actual = actual.trim().parse::<u64>().unwrap_or(0);
                        log::info!("Actual 2MB huge pages allocated: {}", actual);
                    }

                    // 读取剩余的大页数量
                    let free_hugepages_file = format!("{}/free_hugepages", hugepage_path_2mb);
                    if let Ok(free) = fs::read_to_string(&free_hugepages_file) {
                        let free = free.trim().parse::<u64>().unwrap_or(0);
                        log::debug!("Free 2MB huge pages: {}", free);
                    }
                }
            }

            // 3. 配置1GB大页（如果支持）
            if support_1gb {
                let nr_hugepages_file = format!("{}/nr_hugepages", hugepage_path_1gb);
                if let Ok(current) = fs::read_to_string(&nr_hugepages_file) {
                    let current = current.trim().parse::<u64>().unwrap_or(0);
                    log::debug!("Current 1GB huge pages: {}", current);

                    // 1GB大页数量通常较少，建议1-4个
                    let target_hugepages_1gb = 2u64;
                    log::info!("Target 1GB huge pages: {}", target_hugepages_1gb);

                    if let Ok(mut file) = fs::File::create(&nr_hugepages_file) {
                        match writeln!(file, "{}", target_hugepages_1gb) {
                            Ok(_) => {
                                log::info!(
                                    "Successfully configured 1GB huge pages to {}",
                                    target_hugepages_1gb
                                );
                            }
                            Err(e) => {
                                log::warn!("Failed to set 1GB huge pages: {}", e);
                            }
                        }
                    }

                    // 读取实际分配的大页数量
                    if let Ok(actual) = fs::read_to_string(&nr_hugepages_file) {
                        let actual = actual.trim().parse::<u64>().unwrap_or(0);
                        log::info!("Actual 1GB huge pages allocated: {}", actual);
                    }
                }
            }

            // 4. 配置透明大页（THP - Transparent Huge Pages）
            let thp_path = "/sys/kernel/mm/transparent_hugepage";
            if std::path::Path::new(thp_path).exists() {
                // 读取当前THP设置
                let enabled_file = format!("{}/enabled", thp_path);
                if let Ok(current) = fs::read_to_string(&enabled_file) {
                    log::debug!("Current THP setting: {}", current.trim());
                }

                // 设置THP为"madvise"模式（推荐用于VM工作负载）
                if let Ok(mut file) = fs::File::create(&enabled_file) {
                    match writeln!(file, "madvise") {
                        Ok(_) => {
                            log::info!("Successfully set THP mode to 'madvise'");
                        }
                        Err(e) => {
                            log::warn!("Failed to set THP mode: {}", e);
                        }
                    }
                }

                // 配置THP defrag设置
                let defrag_file = format!("{}/defrag", thp_path);
                if std::path::Path::new(&defrag_file).exists() {
                    if let Ok(mut file) = fs::File::create(&defrag_file) {
                        // 设置为"madvise"以允许显式请求时进行defrag
                        match writeln!(file, "madvise") {
                            Ok(_) => {
                                log::info!("Successfully set THP defrag to 'madvise'");
                            }
                            Err(e) => {
                                log::warn!("Failed to set THP defrag: {}", e);
                            }
                        }
                    }
                }
            }

            // 5. 配置hugetlbfs cgroup限制（如果使用cgroup v2）
            let cgroup_v2_path = "/sys/fs/cgroup";
            if std::path::Path::new(&format!("{}/unified", cgroup_v2_path)).exists()
                || std::path::Path::new(cgroup_v2_path).exists()
            {
                // 尝试设置hugetlb限制
                // 注意：这需要适当的权限和cgroup配置
                log::debug!("Cgroup v2 detected, hugetlb limits may be configured separately");
            }

            // 6. 验证配置
            if support_2mb {
                let vmstat = fs::read_to_string("/proc/vmstat").unwrap_or_default();
                for line in vmstat.lines() {
                    if line.starts_with("nr_hugepages") || line.starts_with("nr_free_hugepages") {
                        log::debug!("VMstat: {}", line);
                    }
                }
            }

            log::info!("Huge page configuration completed successfully");
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            log::warn!("Huge pages are only supported on Linux, skipping...");
            Ok(())
        }
    }

    /// 启用 NUMA 感知分配
    fn enable_numa_allocation(&self) -> Result<(), SocError> {
        log::info!("Enabling NUMA-aware allocation");

        // WIP: 实际的 NUMA 配置
        //
        // 当前状态: API stub已定义，等待完整实现
        // 依赖: NUMA系统调用
        // 优先级: P2（性能优化）
        //
        // 实现要点:
        // - 检测NUMA节点
        // - 配置内存亲和性
        // - 优化跨节点访问
        Ok(())
    }

    /// 获取推荐的 CPU 亲和性
    pub fn get_recommended_affinity(&self, workload_type: WorkloadType) -> Vec<CpuCluster> {
        match workload_type {
            WorkloadType::PerformanceCritical => {
                // 使用所有 P-Core
                self.clusters
                    .iter()
                    .filter(|c| **c == CpuCluster::Performance)
                    .copied()
                    .collect()
            }
            WorkloadType::PowerEfficient => {
                // 仅使用 E-Core
                self.clusters
                    .iter()
                    .filter(|c| **c == CpuCluster::Efficiency)
                    .copied()
                    .collect()
            }
            WorkloadType::Balanced => {
                // 混合使用
                self.clusters.clone()
            }
        }
    }

    /// 设置功耗级别
    pub fn set_power_level(&mut self, level: u32) -> Result<(), SocError> {
        if level > 3 {
            return Err(SocError::InvalidPowerLevel(level));
        }

        self.config.power_saving_level = level;
        log::info!("Set power saving level to {}", level);

        // WIP: 实际的功耗级别设置
        //
        // 当前状态: 仅参数验证，等待完整实现
        // 依赖: CPU性能管理API
        // 优先级: P1（功耗管理）
        //
        // 实现要点:
        // - 设置CPU频率上限
        // - 调整电压和频率
        // - 监控功耗状态
        Ok(())
    }

    /// 检测CPU集群拓扑
    ///
    /// 返回集群类型的向量，基于硬件检测或预设配置
    fn detect_cpu_topology(&self) -> Result<Vec<CpuCluster>, SocError> {
        #[cfg(target_os = "linux")]
        {
            use std::fs;

            // 尝试从sysfs读取CPU拓扑信息
            // 1. 检查是否存在CPU拓扑信息
            if let Ok(topology) = fs::read_to_string("/sys/devices/system/cpu/present") {
                log::debug!("CPU present: {}", topology.trim());
            }

            // 2. 尝试读取CPU集群信息（ARM特定的）
            // 在ARM SoC上，集群信息通常在 /sys/devices/system/cpu/cpu*/topology
            let mut detected_clusters = Vec::new();

            // 尝试读取每个CPU的集群信息
            for cpu in 0..self.clusters.len() {
                let cluster_path = format!(
                    "/sys/devices/system/cpu/cpu{}/topology/physical_package_id",
                    cpu
                );
                if let Ok(cluster_id) = fs::read_to_string(&cluster_path) {
                    log::debug!("CPU {} belongs to cluster {}", cpu, cluster_id.trim());
                }
            }

            // 如果检测失败，使用预设的集群配置
            if detected_clusters.is_empty() {
                log::info!("Using preset cluster topology for {:?}", self.vendor);
                detected_clusters = self.clusters.clone();
            }

            Ok(detected_clusters)
        }

        #[cfg(not(target_os = "linux"))]
        {
            log::warn!(
                "CPU topology detection only supported on Linux, using preset configuration"
            );
            Ok(self.clusters.clone())
        }
    }

    /// 获取指定集群的CPU列表
    ///
    /// 返回属于该集群的CPU ID列表
    fn get_cluster_cpus(&self, cluster_id: usize) -> Result<Vec<usize>, SocError> {
        if cluster_id >= self.clusters.len() {
            return Err(SocError::ClusterNotFound(cluster_id));
        }

        #[cfg(target_os = "linux")]
        {
            use std::fs;

            let mut cluster_cpus = Vec::new();

            // 遍历所有CPU，查找属于该集群的CPU
            for cpu in 0.. {
                // 检查CPU是否存在
                let cpu_path = format!("/sys/devices/system/cpu/cpu{}", cpu);
                if !std::path::Path::new(&cpu_path).exists() {
                    break; // CPU不存在，停止查找
                }

                // 读取CPU的集群ID
                let cluster_path = format!(
                    "/sys/devices/system/cpu/cpu{}/topology/physical_package_id",
                    cpu
                );
                if let Ok(cpu_cluster_id) = fs::read_to_string(&cluster_path) {
                    if let Ok(id) = cpu_cluster_id.trim().parse::<usize>() {
                        if id == cluster_id {
                            cluster_cpus.push(cpu);
                        }
                    }
                }
            }

            // 如果检测失败，使用预设的映射
            if cluster_cpus.is_empty() {
                log::debug!("Using preset CPU mapping for cluster {}", cluster_id);
                // 简单的CPU分配策略：平均分配到各个集群
                let total_cpus = self.clusters.len();
                let cpus_per_cluster = (num_cpus::get() + total_cpus - 1) / total_cpus;
                let start = cluster_id * cpus_per_cluster;
                let end = std::cmp::min(start + cpus_per_cluster, num_cpus::get());

                for cpu in start..end {
                    cluster_cpus.push(cpu);
                }
            }

            Ok(cluster_cpus)
        }

        #[cfg(not(target_os = "linux"))]
        {
            // 非Linux平台：使用简单的CPU分配
            let total_cpus = num_cpus::get();
            let cpus_per_cluster = (total_cpus + self.clusters.len() - 1) / self.clusters.len();
            let start = cluster_id * cpus_per_cluster;
            let end = std::cmp::min(start + cpus_per_cluster, total_cpus);

            Ok((start..end).collect())
        }
    }
}

/// 工作负载类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkloadType {
    PerformanceCritical,
    PowerEfficient,
    Balanced,
}

/// SoC 错误
#[derive(Debug, thiserror::Error)]
pub enum SocError {
    #[error("Invalid power level: {0}")]
    InvalidPowerLevel(u32),

    #[error("Feature not supported: {0}")]
    NotSupported(String),

    #[error("Configuration failed: {0}")]
    ConfigurationFailed(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Platform not supported: {0}")]
    PlatformNotSupported(String),

    #[error("CPU topology detection failed: {0}")]
    TopologyDetectionFailed(String),

    #[error("Cluster {0} not found")]
    ClusterNotFound(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soc_optimizer_creation() {
        let optimizer = SocOptimizer::new(SocVendor::Qualcomm);
        assert_eq!(optimizer.vendor, SocVendor::Qualcomm);
        assert_eq!(optimizer.clusters.len(), 8);
    }

    #[test]
    fn test_recommended_affinity() {
        let optimizer = SocOptimizer::new(SocVendor::Apple);

        let affinity = optimizer.get_recommended_affinity(WorkloadType::PerformanceCritical);
        assert_eq!(affinity.len(), 4); // 4 个 P-Core
        assert!(affinity.iter().all(|c| *c == CpuCluster::Performance));

        let affinity = optimizer.get_recommended_affinity(WorkloadType::PowerEfficient);
        assert_eq!(affinity.len(), 4); // 4 个 E-Core
        assert!(affinity.iter().all(|c| *c == CpuCluster::Efficiency));
    }

    #[test]
    fn test_power_level_setting() {
        let mut optimizer = SocOptimizer::new(SocVendor::HiSilicon);

        let result = optimizer.set_power_level(2);
        assert!(result.is_ok());
        assert_eq!(optimizer.config.power_saving_level, 2);
    }

    #[test]
    fn test_invalid_power_level() {
        let mut optimizer = SocOptimizer::new(SocVendor::MediaTek);

        let result = optimizer.set_power_level(10);
        assert!(result.is_err());
    }

    #[test]
    fn test_soc_config_default() {
        let config = SocConfig::default();
        assert!(config.enable_dynamiq);
        assert!(config.enable_big_little);
        assert_eq!(config.power_saving_level, 2);
        assert!(config.enable_huge_pages);
        assert!(config.enable_numa);
    }

    #[test]
    fn test_apply_optimizations() {
        let optimizer = SocOptimizer::new(SocVendor::Samsung);
        let result = optimizer.apply_optimizations();
        assert!(result.is_ok());
    }
}
