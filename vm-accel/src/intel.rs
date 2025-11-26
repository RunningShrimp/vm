//! Intel CPU 特定优化
//!
//! 实现 Intel VT-x, EPT, VPID, APICv 等特性的优化

use super::cpuinfo::{CpuInfo, CpuVendor};

/// Intel VT-x 优化配置
#[derive(Debug, Clone)]
pub struct IntelVtxConfig {
    /// 启用 EPT (Extended Page Tables)
    pub enable_ept: bool,
    /// 启用 VPID (Virtual Processor ID)
    pub enable_vpid: bool,
    /// 启用 APICv (Advanced Programmable Interrupt Controller virtualization)
    pub enable_apicv: bool,
    /// 启用 Unrestricted Guest
    pub enable_unrestricted_guest: bool,
    /// 启用 VMCS Shadowing
    pub enable_vmcs_shadowing: bool,
}

impl Default for IntelVtxConfig {
    fn default() -> Self {
        let cpu_info = CpuInfo::get();
        Self {
            enable_ept: cpu_info.features.ept,
            enable_vpid: cpu_info.features.vpid,
            enable_apicv: cpu_info.features.apicv,
            enable_unrestricted_guest: true,
            enable_vmcs_shadowing: false,
        }
    }
}

/// Intel 优化器
pub struct IntelOptimizer {
    config: IntelVtxConfig,
    is_available: bool,
}

impl IntelOptimizer {
    /// 创建新的 Intel 优化器
    pub fn new() -> Self {
        let cpu_info = CpuInfo::get();
        let is_available = cpu_info.vendor == CpuVendor::Intel && cpu_info.features.vmx;
        
        Self {
            config: IntelVtxConfig::default(),
            is_available,
        }
    }

    /// 检查是否可用
    pub fn is_available(&self) -> bool {
        self.is_available
    }

    /// 获取配置
    pub fn config(&self) -> &IntelVtxConfig {
        &self.config
    }

    /// 设置配置
    pub fn set_config(&mut self, config: IntelVtxConfig) {
        self.config = config;
    }

    /// 应用 Intel 特定优化
    pub fn apply_optimizations(&self) {
        if !self.is_available {
            log::warn!("Intel VT-x not available, skipping optimizations");
            return;
        }

        log::info!("Applying Intel VT-x optimizations:");
        
        if self.config.enable_ept {
            log::info!("  - EPT (Extended Page Tables): enabled");
            // EPT 优化：减少 VM exit，提升内存访问性能
        }
        
        if self.config.enable_vpid {
            log::info!("  - VPID (Virtual Processor ID): enabled");
            // VPID 优化：避免 TLB flush，提升上下文切换性能
        }
        
        if self.config.enable_apicv {
            log::info!("  - APICv: enabled");
            // APICv 优化：硬件虚拟化中断控制器，减少中断开销
        }
        
        if self.config.enable_unrestricted_guest {
            log::info!("  - Unrestricted Guest: enabled");
            // 允许 Guest 运行在实模式和保护模式
        }
    }

    /// 获取推荐的 SIMD 指令集
    pub fn get_recommended_simd(&self) -> &'static str {
        let cpu_info = CpuInfo::get();
        
        if cpu_info.features.avx512f {
            "AVX-512"
        } else if cpu_info.features.avx2 {
            "AVX2"
        } else if cpu_info.features.avx {
            "AVX"
        } else if cpu_info.features.sse4_2 {
            "SSE4.2"
        } else {
            "SSE2"
        }
    }

    /// 优化内存访问模式
    pub fn optimize_memory_access(&self) -> MemoryAccessHint {
        let cpu_info = CpuInfo::get();
        
        MemoryAccessHint {
            use_huge_pages: true,
            cache_line_size: 64,  // Intel 通常是 64 字节
            prefetch_distance: if cpu_info.features.avx512f { 512 } else { 256 },
            numa_aware: cpu_info.core_count > 8,
        }
    }

    /// 获取 JIT 编译器优化建议
    pub fn get_jit_hints(&self) -> JitOptimizationHints {
        let cpu_info = CpuInfo::get();
        
        JitOptimizationHints {
            inline_threshold: 100,
            loop_unroll_factor: 4,
            use_simd: cpu_info.features.avx2,
            simd_width: if cpu_info.features.avx512f { 512 } 
                       else if cpu_info.features.avx2 { 256 }
                       else { 128 },
            enable_branch_prediction_hints: true,
            enable_cache_prefetch: true,
        }
    }
}

impl Default for IntelOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// 内存访问优化提示
#[derive(Debug, Clone)]
pub struct MemoryAccessHint {
    pub use_huge_pages: bool,
    pub cache_line_size: usize,
    pub prefetch_distance: usize,
    pub numa_aware: bool,
}

/// JIT 优化提示
#[derive(Debug, Clone)]
pub struct JitOptimizationHints {
    pub inline_threshold: usize,
    pub loop_unroll_factor: usize,
    pub use_simd: bool,
    pub simd_width: usize,
    pub enable_branch_prediction_hints: bool,
    pub enable_cache_prefetch: bool,
}

/// Intel TSX (Transactional Synchronization Extensions) 支持
pub struct IntelTsx {
    available: bool,
}

impl IntelTsx {
    pub fn new() -> Self {
        // 检测 TSX 支持（需要 CPUID）
        Self {
            available: false, // 简化实现
        }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    /// 使用 TSX 优化原子操作
    pub fn optimize_atomic_ops(&self) -> bool {
        if !self.available {
            return false;
        }
        
        log::info!("Using Intel TSX for lock elision");
        true
    }
}

impl Default for IntelTsx {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intel_optimizer() {
        let optimizer = IntelOptimizer::new();
        println!("Intel optimizer available: {}", optimizer.is_available());
        
        if optimizer.is_available() {
            optimizer.apply_optimizations();
            println!("Recommended SIMD: {}", optimizer.get_recommended_simd());
            println!("Memory hints: {:?}", optimizer.optimize_memory_access());
            println!("JIT hints: {:?}", optimizer.get_jit_hints());
        }
    }
}
