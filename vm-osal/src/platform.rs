//! 平台特定集成
//!
//! 为不同操作系统提供原生支持

use std::path::PathBuf;

/// 平台信息
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    pub os: String,
    pub os_version: String,
    pub arch: String,
    pub cpu_count: usize,
    pub total_memory: u64,
}

impl PlatformInfo {
    /// 获取当前平台信息
    pub fn get() -> Self {
        Self {
            os: super::host_os().to_string(),
            os_version: Self::get_os_version(),
            arch: super::host_arch().to_string(),
            cpu_count: Self::get_cpu_count(),
            total_memory: Self::get_total_memory(),
        }
    }

    /// 获取操作系统版本
    #[cfg(target_os = "linux")]
    fn get_os_version() -> String {
        use std::fs;
        if let Ok(content) = fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if line.starts_with("PRETTY_NAME=") {
                    return line
                        .trim_start_matches("PRETTY_NAME=")
                        .trim_matches('"')
                        .to_string();
                }
            }
        }
        "Linux".to_string()
    }

    #[cfg(target_os = "macos")]
    fn get_os_version() -> String {
        use std::process::Command;
        if let Ok(output) = Command::new("sw_vers").arg("-productVersion").output()
            && let Ok(version) = String::from_utf8(output.stdout) {
                return format!("macOS {}", version.trim());
            }
        "macOS".to_string()
    }

    #[cfg(target_os = "windows")]
    fn get_os_version() -> String {
        "Windows".to_string()
    }

    #[cfg(target_os = "android")]
    fn get_os_version() -> String {
        use std::process::Command;
        if let Ok(output) = Command::new("getprop")
            .arg("ro.build.version.release")
            .output()
        {
            if let Ok(version) = String::from_utf8(output.stdout) {
                return format!("Android {}", version.trim());
            }
        }
        "Android".to_string()
    }

    #[cfg(target_os = "ios")]
    fn get_os_version() -> String {
        "iOS".to_string()
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows",
        target_os = "android",
        target_os = "ios"
    )))]
    fn get_os_version() -> String {
        "Unknown".to_string()
    }

    /// 获取 CPU 核心数
    fn get_cpu_count() -> usize {
        num_cpus::get()
    }

    /// 获取总内存（字节）
    #[cfg(target_os = "linux")]
    fn get_total_memory() -> u64 {
        use std::fs;
        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return kb * 1024;
                        }
                    }
                }
            }
        }
        0
    }

    #[cfg(target_os = "macos")]
    fn get_total_memory() -> u64 {
        use std::process::Command;
        if let Ok(output) = Command::new("sysctl").arg("-n").arg("hw.memsize").output()
            && let Ok(mem_str) = String::from_utf8(output.stdout)
                && let Ok(mem) = mem_str.trim().parse::<u64>() {
                    return mem;
                }
        0
    }

    #[cfg(target_os = "windows")]
    fn get_total_memory() -> u64 {
        // Windows 实现需要使用 GlobalMemoryStatusEx
        0
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    fn get_total_memory() -> u64 {
        0
    }

    /// 打印平台信息
    pub fn print(&self) {
        println!("Platform Information:");
        println!("  OS: {}", self.os);
        println!("  Version: {}", self.os_version);
        println!("  Architecture: {}", self.arch);
        println!("  CPU Cores: {}", self.cpu_count);
        println!(
            "  Total Memory: {} GB",
            self.total_memory / (1024 * 1024 * 1024)
        );
    }
}

/// 平台特定路径
pub struct PlatformPaths;

impl PlatformPaths {
    /// 获取配置目录
    pub fn config_dir() -> Option<PathBuf> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
                return Some(PathBuf::from(xdg_config).join("vm"));
            }
            if let Ok(home) = std::env::var("HOME") {
                return Some(PathBuf::from(home).join(".config/vm"));
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return Some(PathBuf::from(home).join("Library/Application Support/vm"));
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                return Some(PathBuf::from(appdata).join("vm"));
            }
        }

        #[cfg(target_os = "android")]
        {
            return Some(PathBuf::from("/data/local/tmp/vm"));
        }

        None
    }

    /// 获取数据目录
    pub fn data_dir() -> Option<PathBuf> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
                return Some(PathBuf::from(xdg_data).join("vm"));
            }
            if let Ok(home) = std::env::var("HOME") {
                return Some(PathBuf::from(home).join(".local/share/vm"));
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return Some(PathBuf::from(home).join("Library/Application Support/vm"));
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
                return Some(PathBuf::from(appdata).join("vm"));
            }
        }

        None
    }

    /// 获取缓存目录
    pub fn cache_dir() -> Option<PathBuf> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
                return Some(PathBuf::from(xdg_cache).join("vm"));
            }
            if let Ok(home) = std::env::var("HOME") {
                return Some(PathBuf::from(home).join(".cache/vm"));
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return Some(PathBuf::from(home).join("Library/Caches/vm"));
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(temp) = std::env::var("TEMP") {
                return Some(PathBuf::from(temp).join("vm"));
            }
        }

        None
    }
}

/// 平台特定功能
pub struct PlatformFeatures;

impl PlatformFeatures {
    /// 检查是否支持硬件虚拟化
    pub fn supports_hardware_virtualization() -> bool {
        #[cfg(target_os = "linux")]
        {
            use std::path::Path;
            // 检查 KVM
            return Path::new("/dev/kvm").exists();
        }

        #[cfg(target_os = "macos")]
        {
            // macOS 10.10+ 都支持 Hypervisor.framework
            true
        }

        #[cfg(target_os = "windows")]
        {
            // Windows 需要检查 Hyper-V 或 WHPX
            return false;
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            return false;
        }
    }

    /// 检查是否支持 GPU 加速
    pub fn supports_gpu_acceleration() -> bool {
        // 所有桌面平台都支持某种形式的 GPU 加速
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            true
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            return false;
        }
    }

    /// 检查是否支持网络直通
    pub fn supports_network_passthrough() -> bool {
        #[cfg(target_os = "linux")]
        {
            use std::path::Path;
            return Path::new("/dev/net/tun").exists();
        }

        #[cfg(target_os = "macos")]
        {
            // macOS 需要安装 tuntap 驱动
            false
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_info() {
        let info = PlatformInfo::get();
        info.print();
        assert!(!info.os.is_empty());
        assert!(info.cpu_count > 0);
    }

    #[test]
    fn test_platform_paths() {
        if let Some(config_dir) = PlatformPaths::config_dir() {
            println!("Config dir: {:?}", config_dir);
        }
        if let Some(data_dir) = PlatformPaths::data_dir() {
            println!("Data dir: {:?}", data_dir);
        }
    }

    #[test]
    fn test_platform_features() {
        println!(
            "Hardware virtualization: {}",
            PlatformFeatures::supports_hardware_virtualization()
        );
        println!(
            "GPU acceleration: {}",
            PlatformFeatures::supports_gpu_acceleration()
        );
        println!(
            "Network passthrough: {}",
            PlatformFeatures::supports_network_passthrough()
        );
    }
}
