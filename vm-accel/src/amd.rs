//! AMD CPU 特定优化
//!
//! 实现 AMD-V (SVM), NPT, AVIC 等特性的优化

use super::cpuinfo::{CpuInfo, CpuVendor};

/// AMD-V (SVM) 优化配置
#[derive(Debug, Clone)]
pub struct AmdSvmConfig {
    /// 启用 NPT (Nested Page Tables)
    pub enable_npt: bool,
    /// 启用 AVIC (Advanced Virtual Interrupt Controller)
    pub enable_avic: bool,
    /// 启用 Decode Assists
    pub enable_decode_assists: bool,
    /// 启用 Flush by ASID
    pub enable_flush_by_asid: bool,
    /// 启用 VMCB Clean Bits
    pub enable_vmcb_clean: bool,
}

impl Default for AmdSvmConfig {
    fn default() -> Self {
        let cpu_info = CpuInfo::get();
        Self {
            enable_npt: cpu_info.features.npt,
            enable_avic: cpu_info.features.avic,
            enable_decode_assists: true,
            enable_flush_by_asid: true,
            enable_vmcb_clean: true,
        }
    }
}

/// AMD 优化器
pub struct AmdOptimizer {
    config: AmdSvmConfig,
    is_available: bool,
    generation: AmdGeneration,
}

/// AMD CPU 代数
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmdGeneration {
    Unknown,
    Zen,      // Ryzen 1000
    ZenPlus,  // Ryzen 2000
    Zen2,     // Ryzen 3000
    Zen3,     // Ryzen 5000
    Zen4,     // Ryzen 7000
    Zen5,     // Ryzen 9000
}

impl AmdOptimizer {
    /// 创建新的 AMD 优化器
    pub fn new() -> Self {
        let cpu_info = CpuInfo::get();
        let is_available = cpu_info.vendor == CpuVendor::AMD && cpu_info.features.svm;
        let generation = Self::detect_generation(&cpu_info.model_name);
        
        Self {
            config: AmdSvmConfig::default(),
            is_available,
            generation,
        }
    }

    /// 检测 AMD CPU 代数
    fn detect_generation(model_name: &str) -> AmdGeneration {
        let lower = model_name.to_lowercase();
        
        if lower.contains("ryzen 9000") || lower.contains("zen 5") {
            AmdGeneration::Zen5
        } else if lower.contains("ryzen 7000") || lower.contains("zen 4") {
            AmdGeneration::Zen4
        } else if lower.contains("ryzen 5000") || lower.contains("zen 3") {
            AmdGeneration::Zen3
        } else if lower.contains("ryzen 3000") || lower.contains("zen 2") {
            AmdGeneration::Zen2
        } else if lower.contains("ryzen 2000") || lower.contains("zen+") {
            AmdGeneration::ZenPlus
        } else if lower.contains("ryzen 1000") || lower.contains("zen") {
            AmdGeneration::Zen
        } else {
            AmdGeneration::Unknown
        }
    }

    /// 检查是否可用
    pub fn is_available(&self) -> bool {
        self.is_available
    }

    /// 获取配置
    pub fn config(&self) -> &AmdSvmConfig {
        &self.config
    }

    /// 设置配置
    pub fn set_config(&mut self, config: AmdSvmConfig) {
        self.config = config;
    }

    /// 获取 CPU 代数
    pub fn generation(&self) -> AmdGeneration {
        self.generation
    }

    /// 应用 AMD 特定优化
    pub fn apply_optimizations(&self) {
        if !self.is_available {
            log::warn!("AMD-V (SVM) not available, skipping optimizations");
            return;
        }

        log::info!("Applying AMD-V (SVM) optimizations:");
        log::info!("  - CPU Generation: {:?}", self.generation);
        
        if self.config.enable_npt {
            log::info!("  - NPT (Nested Page Tables): enabled");
            // NPT 优化：二级地址转换，减少 VM exit
        }
        
        if self.config.enable_avic {
            log::info!("  - AVIC (Advanced Virtual Interrupt Controller): enabled");
            // AVIC 优化：硬件虚拟化中断，减少中断处理开销
        }
        
        if self.config.enable_decode_assists {
            log::info!("  - Decode Assists: enabled");
            // 硬件辅助指令解码
        }
        
        if self.config.enable_flush_by_asid {
            log::info!("  - Flush by ASID: enabled");
            // 按 ASID 刷新 TLB，避免全局 flush
        }
        
        if self.config.enable_vmcb_clean {
            log::info!("  - VMCB Clean Bits: enabled");
            // 减少 VMCB 状态同步开销
        }
    }

    /// 获取推荐的 SIMD 指令集
    pub fn get_recommended_simd(&self) -> &'static str {
        let cpu_info = CpuInfo::get();
        
        // Zen 4+ 支持 AVX-512
        if matches!(self.generation, AmdGeneration::Zen4 | AmdGeneration::Zen5) && cpu_info.features.avx512f {
            "AVX-512"
        } else if cpu_info.features.avx2 {
            "AVX2"
        } else if cpu_info.features.avx {
            "AVX"
        } else {
            "SSE4.2"
        }
    }

    /// 优化内存访问模式
    pub fn optimize_memory_access(&self) -> MemoryAccessHint {
        let cpu_info = CpuInfo::get();
        
        MemoryAccessHint {
            use_huge_pages: true,
            cache_line_size: 64,  // AMD 也是 64 字节
            prefetch_distance: match self.generation {
                AmdGeneration::Zen4 | AmdGeneration::Zen5 => 512,
                AmdGeneration::Zen3 => 384,
                _ => 256,
            },
            numa_aware: cpu_info.core_count > 8,
            // AMD Infinity Fabric 优化
            prefer_local_memory: true,
        }
    }

    /// 获取 JIT 编译器优化建议
    pub fn get_jit_hints(&self) -> JitOptimizationHints {
        let cpu_info = CpuInfo::get();
        
        JitOptimizationHints {
            inline_threshold: 120,  // AMD 分支预测较好，可以更激进
            loop_unroll_factor: match self.generation {
                AmdGeneration::Zen4 | AmdGeneration::Zen5 => 8,
                AmdGeneration::Zen3 => 6,
                _ => 4,
            },
            use_simd: cpu_info.features.avx2,
            simd_width: if cpu_info.features.avx512f { 512 } 
                       else if cpu_info.features.avx2 { 256 }
                       else { 128 },
            enable_branch_prediction_hints: true,
            enable_cache_prefetch: true,
            // AMD 特有优化
            prefer_micro_op_cache: !matches!(self.generation, AmdGeneration::Unknown | AmdGeneration::Zen | AmdGeneration::ZenPlus),
        }
    }

    /// 获取 CCX (Core Complex) 优化建议
    pub fn get_ccx_optimization(&self) -> CcxOptimization {
        match self.generation {
            AmdGeneration::Zen5 => CcxOptimization {
                cores_per_ccx: 16,  // Zen 5 每个 CCX 16 核心
                shared_l3_size_mb: 32,
                prefer_same_ccx_scheduling: true,
            },
            AmdGeneration::Zen4 => CcxOptimization {
                cores_per_ccx: 8,
                shared_l3_size_mb: 32,
                prefer_same_ccx_scheduling: true,
            },
            AmdGeneration::Zen3 => CcxOptimization {
                cores_per_ccx: 8,
                shared_l3_size_mb: 32,
                prefer_same_ccx_scheduling: true,
            },
            AmdGeneration::Zen2 => CcxOptimization {
                cores_per_ccx: 4,
                shared_l3_size_mb: 16,
                prefer_same_ccx_scheduling: true,
            },
            _ => CcxOptimization {
                cores_per_ccx: 4,
                shared_l3_size_mb: 8,
                prefer_same_ccx_scheduling: false,
            },
        }
    }
}

impl Default for AmdOptimizer {
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
    pub prefer_local_memory: bool,
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
    pub prefer_micro_op_cache: bool,
}

/// CCX (Core Complex) 优化配置
#[derive(Debug, Clone)]
pub struct CcxOptimization {
    pub cores_per_ccx: usize,
    pub shared_l3_size_mb: usize,
    pub prefer_same_ccx_scheduling: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amd_optimizer() {
        let optimizer = AmdOptimizer::new();
        println!("AMD optimizer available: {}", optimizer.is_available());
        
        if optimizer.is_available() {
            optimizer.apply_optimizations();
            println!("CPU Generation: {:?}", optimizer.generation());
            println!("Recommended SIMD: {}", optimizer.get_recommended_simd());
            println!("Memory hints: {:?}", optimizer.optimize_memory_access());
            println!("JIT hints: {:?}", optimizer.get_jit_hints());
            println!("CCX optimization: {:?}", optimizer.get_ccx_optimization());
        }
    }
}
