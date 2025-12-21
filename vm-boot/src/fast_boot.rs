//! 快速启动优化
//!
//! 实现虚拟机的快速启动功能

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// 启动缓存
pub struct BootCache {
    /// 缓存的内核镜像
    kernel_cache: Option<Vec<u8>>,
    /// 缓存的 initrd
    initrd_cache: Option<Vec<u8>>,
    /// 缓存的设备树
    dtb_cache: Option<Vec<u8>>,
}

impl Default for BootCache {
    fn default() -> Self {
        Self::new()
    }
}

impl BootCache {
    pub fn new() -> Self {
        Self {
            kernel_cache: None,
            initrd_cache: None,
            dtb_cache: None,
        }
    }

    /// 预加载内核
    pub fn preload_kernel<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        self.kernel_cache = Some(buffer);
        Ok(())
    }

    /// 预加载 initrd
    pub fn preload_initrd<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        self.initrd_cache = Some(buffer);
        Ok(())
    }

    /// 预加载设备树
    pub fn preload_dtb<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        self.dtb_cache = Some(buffer);
        Ok(())
    }

    /// 获取缓存的内核
    pub fn get_kernel(&self) -> Option<&[u8]> {
        self.kernel_cache.as_deref()
    }

    /// 获取缓存的 initrd
    pub fn get_initrd(&self) -> Option<&[u8]> {
        self.initrd_cache.as_deref()
    }

    /// 获取缓存的设备树
    pub fn get_dtb(&self) -> Option<&[u8]> {
        self.dtb_cache.as_deref()
    }
}

/// 快速启动优化器
pub struct FastBootOptimizer {
    /// 启动缓存
    cache: BootCache,
    /// 是否启用并行加载
    parallel_loading: bool,
    /// 是否启用内存预分配
    memory_preallocation: bool,
}

impl Default for FastBootOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl FastBootOptimizer {
    pub fn new() -> Self {
        Self {
            cache: BootCache::new(),
            parallel_loading: true,
            memory_preallocation: true,
        }
    }

    /// 启用并行加载
    pub fn enable_parallel_loading(&mut self, enabled: bool) {
        self.parallel_loading = enabled;
    }

    /// 启用内存预分配
    pub fn enable_memory_preallocation(&mut self, enabled: bool) {
        self.memory_preallocation = enabled;
    }

    /// 优化启动流程
    pub fn optimize_boot(
        &mut self,
        kernel_path: &str,
        initrd_path: Option<&str>,
    ) -> io::Result<()> {
        if self.parallel_loading {
            // 并行加载内核和 initrd
            self.parallel_load(kernel_path, initrd_path)?;
        } else {
            // 串行加载
            self.cache.preload_kernel(kernel_path)?;
            if let Some(initrd) = initrd_path {
                self.cache.preload_initrd(initrd)?;
            }
        }

        Ok(())
    }

    /// 并行加载资源
    fn parallel_load(&mut self, kernel_path: &str, initrd_path: Option<&str>) -> io::Result<()> {
        use std::thread;

        let kernel_path = kernel_path.to_string();
        let initrd_path = initrd_path.map(|s| s.to_string());

        // 启动内核加载线程
        let kernel_handle = thread::spawn(move || {
            let mut file = File::open(&kernel_path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            Ok::<Vec<u8>, io::Error>(buffer)
        });

        // 启动 initrd 加载线程
        let initrd_handle = initrd_path.map(|path| {
            thread::spawn(move || {
                let mut file = File::open(&path)?;
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)?;
                Ok::<Vec<u8>, io::Error>(buffer)
            })
        });

        // 等待加载完成
        let kernel_result = kernel_handle
            .join()
            .map_err(|_| io::Error::other("Kernel loader thread panicked"))?;
        let kernel_data = kernel_result?;
        self.cache.kernel_cache = Some(kernel_data);

        if let Some(handle) = initrd_handle {
            let initrd_result = handle
                .join()
                .map_err(|_| io::Error::other("Initrd loader thread panicked"))?;
            let initrd_data = initrd_result?;
            self.cache.initrd_cache = Some(initrd_data);
        }

        Ok(())
    }

    /// 预分配内存
    pub fn preallocate_memory(&self, size: usize) -> Vec<u8> {
        if self.memory_preallocation {
            // 预分配并初始化内存
            vec![0u8; size]
        } else {
            Vec::new()
        }
    }

    /// 获取启动缓存
    pub fn cache(&self) -> &BootCache {
        &self.cache
    }
}

/// 启动性能统计
pub struct BootPerformanceStats {
    /// 内核加载时间（毫秒）
    pub kernel_load_time: u64,
    /// initrd 加载时间（毫秒）
    pub initrd_load_time: u64,
    /// 内存初始化时间（毫秒）
    pub memory_init_time: u64,
    /// 设备初始化时间（毫秒）
    pub device_init_time: u64,
    /// 总启动时间（毫秒）
    pub total_boot_time: u64,
}

impl Default for BootPerformanceStats {
    fn default() -> Self {
        Self::new()
    }
}

impl BootPerformanceStats {
    pub fn new() -> Self {
        Self {
            kernel_load_time: 0,
            initrd_load_time: 0,
            memory_init_time: 0,
            device_init_time: 0,
            total_boot_time: 0,
        }
    }

    /// 打印统计信息
    pub fn print(&self) {
        println!("=== Boot Performance Statistics ===");
        println!("Kernel load time:     {} ms", self.kernel_load_time);
        println!("Initrd load time:     {} ms", self.initrd_load_time);
        println!("Memory init time:     {} ms", self.memory_init_time);
        println!("Device init time:     {} ms", self.device_init_time);
        println!("Total boot time:      {} ms", self.total_boot_time);
        println!("===================================");
    }
}

/// 启动性能分析器
pub struct BootProfiler {
    start_time: std::time::Instant,
    stats: BootPerformanceStats,
}

impl Default for BootProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl BootProfiler {
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            stats: BootPerformanceStats::new(),
        }
    }

    /// 记录内核加载时间
    pub fn record_kernel_load(&mut self) {
        self.stats.kernel_load_time = self.start_time.elapsed().as_millis() as u64;
    }

    /// 记录 initrd 加载时间
    pub fn record_initrd_load(&mut self) {
        self.stats.initrd_load_time = self.start_time.elapsed().as_millis() as u64;
    }

    /// 记录内存初始化时间
    pub fn record_memory_init(&mut self) {
        self.stats.memory_init_time = self.start_time.elapsed().as_millis() as u64;
    }

    /// 记录设备初始化时间
    pub fn record_device_init(&mut self) {
        self.stats.device_init_time = self.start_time.elapsed().as_millis() as u64;
    }

    /// 完成启动
    pub fn finish(&mut self) {
        self.stats.total_boot_time = self.start_time.elapsed().as_millis() as u64;
    }

    /// 获取统计信息
    pub fn stats(&self) -> &BootPerformanceStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::io::Write;

    #[test]
    fn test_boot_cache() {
        let temp_dir = env::temp_dir();
        let kernel_path = temp_dir.join("test_kernel");

        // 创建测试文件
        let mut file = File::create(&kernel_path).expect("Failed to create kernel test file");
        file.write_all(b"test kernel data")
            .expect("Failed to write kernel test data");

        let mut cache = BootCache::new();
        cache
            .preload_kernel(&kernel_path)
            .expect("Failed to preload kernel");

        assert!(cache.get_kernel().is_some());
        let cached_kernel = cache.get_kernel().expect("Kernel should be cached");
        assert_eq!(cached_kernel, b"test kernel data");

        // 清理
        std::fs::remove_file(kernel_path).ok();
    }

    #[test]
    fn test_fast_boot_optimizer() {
        let temp_dir = env::temp_dir();
        let kernel_path = temp_dir.join("test_kernel2");

        // 创建测试文件
        let mut file = File::create(&kernel_path).expect("Failed to create kernel test file");
        file.write_all(b"test kernel data")
            .expect("Failed to write kernel test data");

        let mut optimizer = FastBootOptimizer::new();
        optimizer
            .optimize_boot(
                kernel_path
                    .to_str()
                    .expect("Kernel path is not valid UTF-8"),
                None,
            )
            .expect("Optimized boot should succeed");

        assert!(optimizer.cache().get_kernel().is_some());

        // 清理
        std::fs::remove_file(kernel_path).ok();
    }
}
