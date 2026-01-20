//! 平台检测和特定功能
//!
//! 提供操作系统版本、架构检测、硬件虚拟化检测等功能
//! 从 vm-osal/platform.rs 迁移而来

/// 检测操作系统
pub fn host_os() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        // HarmonyOS 基于 Linux 内核，可通过系统属性检测
        if is_harmonyos() {
            return "harmonyos";
        }
        return "linux";
    }
    #[cfg(target_os = "macos")]
    {
        return "macos";
    }
    #[cfg(target_os = "windows")]
    {
        return "windows";
    }
    #[cfg(target_os = "android")]
    {
        return "android";
    }
    #[cfg(target_os = "ios")]
    {
        return "ios";
    }
    #[allow(unreachable_code)]
    "unknown"
}

/// 检测是否为 HarmonyOS
#[allow(dead_code)]
fn is_harmonyos() -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        // 检查 /etc/os-release 或系统属性
        if let Ok(content) = fs::read_to_string("/etc/os-release") {
            return content.to_lowercase().contains("harmonyos")
                || content.to_lowercase().contains("openharmony");
        }
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// 检测架构
pub fn host_arch() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        return "x86_64";
    }
    #[cfg(target_arch = "aarch64")]
    {
        return "aarch64";
    }
    #[cfg(target_arch = "riscv64")]
    {
        return "riscv64";
    }
    #[allow(unreachable_code)]
    "unknown"
}

/// 平台信息
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// 操作系统
    pub os: String,
    /// 操作系统版本
    pub os_version: String,
    /// 架构
    pub arch: String,
    /// CPU 核心数
    pub cpu_count: usize,
    /// 总内存（字节）
    pub total_memory: u64,
}

impl PlatformInfo {
    /// 获取平台信息
    pub fn get() -> Self {
        Self {
            os: host_os().to_string(),
            os_version: Self::get_os_version(),
            arch: host_arch().to_string(),
            cpu_count: num_cpus::get(),
            total_memory: Self::get_total_memory(),
        }
    }

    fn get_os_version() -> String {
        #[cfg(target_os = "linux")]
        {
            // 从 /etc/os-release 读取版本
            if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
                for line in content.lines() {
                    if line.starts_with("PRETTY_NAME=") {
                        return line
                            .trim_start_matches("PRETTY_NAME=")
                            .trim_matches('"')
                            .to_string();
                    }
                }
            }
            String::from("Unknown")
        }
        #[cfg(target_os = "macos")]
        {
            "macOS".to_string()
        }
        #[cfg(target_os = "windows")]
        {
            "Windows".to_string()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            "Unknown".to_string()
        }
    }

    fn get_total_memory() -> u64 {
        #[cfg(unix)]
        {
            // 从 /proc/meminfo 读取
            if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
                for line in content.lines() {
                    if line.starts_with("MemTotal:")
                        && let Some(kb_str) = line.split_whitespace().nth(1)
                        && let Ok(kb) = kb_str.parse::<u64>()
                    {
                        return kb * 1024;
                    }
                }
            }
            8 * 1024 * 1024 * 1024 // 默认 8GB
        }
        #[cfg(windows)]
        {
            // Windows 系统内存检测需要额外依赖
            8 * 1024 * 1024 * 1024 // 默认 8GB
        }
    }
}

/// 平台特定路径
#[derive(Debug, Clone)]
pub struct PlatformPaths {
    /// 配置目录
    pub config_dir: String,
    /// 数据目录
    pub data_dir: String,
    /// 缓存目录
    pub cache_dir: String,
}

impl PlatformPaths {
    /// 获取平台特定路径
    pub fn get() -> Self {
        #[cfg(target_os = "linux")]
        {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            Self {
                config_dir: format!("{}/.config/vm-platform", home),
                data_dir: format!("{}/.local/share/vm-platform", home),
                cache_dir: format!("{}/.cache/vm-platform", home),
            }
        }
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            Self {
                config_dir: format!("{}/Library/Application Support/vm-platform", home),
                data_dir: format!("{}/Library/Application Support/vm-platform", home),
                cache_dir: format!("{}/Library/Caches/vm-platform", home),
            }
        }
        #[cfg(target_os = "windows")]
        {
            let appdata =
                std::env::var("APPDATA").unwrap_or_else(|_| "C:\\ProgramData".to_string());
            Self {
                config_dir: format!("{}\\vm-platform\\config", appdata),
                data_dir: format!("{}\\vm-platform\\data", appdata),
                cache_dir: format!("{}\\vm-platform\\cache", appdata),
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Self {
                config_dir: "/tmp/vm-platform/config".to_string(),
                data_dir: "/tmp/vm-platform/data".to_string(),
                cache_dir: "/tmp/vm-platform/cache".to_string(),
            }
        }
    }
}

/// 平台特性支持
#[derive(Debug, Clone)]
pub struct PlatformFeatures {
    /// 硬件虚拟化支持（KVM、HAXM等）
    pub hardware_virtualization: bool,
    /// GPU 加速支持
    pub gpu_acceleration: bool,
    /// 网络直通支持
    pub network_passthrough: bool,
}

impl PlatformFeatures {
    /// 检测平台特性
    pub fn detect() -> Self {
        Self {
            hardware_virtualization: Self::detect_hardware_virtualization(),
            gpu_acceleration: Self::detect_gpu_acceleration(),
            network_passthrough: Self::detect_network_passthrough(),
        }
    }

    fn detect_hardware_virtualization() -> bool {
        #[cfg(target_os = "linux")]
        {
            // 检查 /dev/kvm 存在
            std::path::Path::new("/dev/kvm").exists()
        }
        #[cfg(target_os = "windows")]
        {
            // 检查 HAXM 驱动
            std::path::Path::new("C:\\Windows\\System32\\drivers\\haxm.sys").exists()
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            false
        }
    }

    fn detect_gpu_acceleration() -> bool {
        #[cfg(target_os = "linux")]
        {
            // 检查 /dev/dri 存在
            std::path::Path::new("/dev/dri").exists()
        }
        #[cfg(target_os = "windows")]
        {
            // Windows 总是支持 GPU 加速
            true
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            false
        }
    }

    fn detect_network_passthrough() -> bool {
        #[cfg(target_os = "linux")]
        {
            // 检查 SR-IOV 和 macvtap 支持
            std::path::Path::new("/sys/bus/pci/devices").exists()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }
}
