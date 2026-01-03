//! CPU 特性检测模块
//!
//! 提供运行时 CPU 特性检测功能，用于动态选择最优的 SIMD 代码路径
//!
//! 支持的特性：
//! - x86_64: AVX-512, AVX2, SSE4.2
//! - ARM64: NEON, SVE

use std::fmt;
use std::sync::OnceLock;

/// CPU 特性集合
#[derive(Debug, Clone, Copy)]
pub struct CPUFeatures {
    /// AVX-512 支持 (x86_64)
    pub avx512f: bool,
    /// AVX-512 VL 支持 (x86_64)
    pub avx512vl: bool,
    /// AVX-512 BW 支持 (x86_64)
    pub avx512bw: bool,
    /// AVX2 支持 (x86_64)
    pub avx2: bool,
    /// SSE4.2 支持 (x86_64)
    pub sse42: bool,
    /// NEON 支持 (ARM64)
    pub neon: bool,
    /// SVE 支持 (ARM64)
    pub sve: bool,
    /// SVE2 支持 (ARM64)
    pub sve2: bool,
}

impl CPUFeatures {
    /// 获取系统 CPU 特性（单例）
    pub fn get() -> &'static CPUFeatures {
        INSTANCE.get_or_init(|| {
            #[cfg(target_arch = "x86_64")]
            return detect_x86_features();

            #[cfg(target_arch = "aarch64")]
            return detect_arm_features();

            #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
            return CPUFeatures::none();
        })
    }

    /// 创建一个空的 CPU 特性集合
    pub const fn none() -> Self {
        Self {
            avx512f: false,
            avx512vl: false,
            avx512bw: false,
            avx2: false,
            sse42: false,
            neon: false,
            sve: false,
            sve2: false,
        }
    }

    /// 检查是否支持 AVX-512
    pub fn has_avx512(&self) -> bool {
        self.avx512f
    }

    /// 检查是否支持 AVX2
    pub fn has_avx2(&self) -> bool {
        self.avx2
    }

    /// 检查是否支持 NEON
    pub fn has_neon(&self) -> bool {
        self.neon
    }

    /// 获取推荐的 SIMD 向量宽度（字节）
    pub fn vector_width_bytes(&self) -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            if self.has_avx512() {
                return 64; // AVX-512: 512 bits = 64 bytes
            } else if self.has_avx2() {
                return 32; // AVX2: 256 bits = 32 bytes
            } else if self.sse42 {
                return 16; // SSE: 128 bits = 16 bytes
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if self.sve {
                // SVE 支持可变向量长度，从 128 到 2048 bits
                // 这里返回保守值 256 bits = 32 bytes
                return 32;
            } else if self.has_neon() {
                return 16; // NEON: 128 bits = 16 bytes
            }
        }

        8 // 默认：64位寄存器
    }

    /// 获取推荐的 SIMD 向量宽度（元素数量，假设 u64）
    pub fn vector_width_elements_u64(&self) -> usize {
        self.vector_width_bytes() / 8
    }
}

impl fmt::Display for CPUFeatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut features = vec![];

        #[cfg(target_arch = "x86_64")]
        {
            if self.has_avx512() {
                features.push("AVX-512");
            }
            if self.avx2 {
                features.push("AVX2");
            }
            if self.sse42 {
                features.push("SSE4.2");
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if self.neon {
                features.push("NEON");
            }
            if self.sve {
                features.push("SVE");
            }
            if self.sve2 {
                features.push("SVE2");
            }
        }

        if features.is_empty() {
            write!(f, "无SIMD支持")
        } else {
            write!(f, "{}", features.join(", "))
        }
    }
}

/// 全局 CPU 特性单例
static INSTANCE: OnceLock<CPUFeatures> = OnceLock::new();

#[cfg(target_arch = "x86_64")]
mod x86 {
    use std::arch::x86_64::*;
    use std::arch::x86_64::{__cpuid, __cpuid_count, CpuidResult};

    /// 执行 CPUID 指令
    #[inline(always)]
    unsafe fn cpuid(leaf: u32) -> CpuidResult {
        /// # Safety
        ///
        /// 调用者必须确保：
        /// - CPUID 指令在当前 CPU 架构上可用（x86_64）
        /// - `leaf` 值是有效的 CPUID leaf 编号
        /// - 传入的四个可变引用指向有效的、独立的位置
        /// - 调用期间不会发生 CPU 热迁移或特性变化
        ///
        /// # 维护者必须确保：
        /// - 仅在 x86_64 架构上调用此函数（由 `#[cfg(target_arch = "x86_64")]` 保证）
        /// - `__cpuid_count` 是标准库的 intrinsic，保证安全调用
        /// - 每个寄存器变量都是独立的，避免别名问题
        /// - 修改时验证 CPUID leaf 值的有效性
        unsafe {
            let mut eax = 0u32;
            let mut ebx = 0u32;
            let mut ecx = 0u32;
            let mut edx = 0u32;
            __cpuid_count(leaf, 0, &mut eax, &mut ebx, &mut ecx, &mut edx);
            CpuidResult { eax, ebx, ecx, edx }
        }
    }

    /// 检测 x86_64 CPU 特性
    pub(super) fn detect_x86_features() -> super::CPUFeatures {
        /// # Safety
        ///
        /// 调用者必须确保：
        /// - 仅在 x86_64 架构上运行（由 `#[cfg(target_arch = "x86_64")]` 保证）
        /// - CPUID leaf 1 和 7 在所有 x86_64 CPU 上都有效
        /// - 检测过程中 CPU 特性不会改变
        ///
        /// # 维护者必须确保：
        /// - `cpuid` 函数的正确性已通过其自身的安全文档保证
        /// - CPUID 返回的寄存器值按 Intel/AMD 规范正确解析
        /// - 特性依赖关系正确（如 AVX-512 需要 AVX2）
        /// - 修改时验证位偏移量和特性依赖的正确性
        unsafe {
            let result1 = cpuid(1);
            let result7 = cpuid(7);

            // 检查 SSE4.2 (bit 20 in ECX)
            let sse42 = (result1.ecx & (1 << 20)) != 0;

            // 检查 AVX (bit 28 in ECX)
            let avx = (result1.ecx & (1 << 28)) != 0;

            // 检查 AVX2 (bit 5 in EBX of leaf 7)
            let avx2 = avx && ((result7.ebx & (1 << 5)) != 0);

            // 检查 AVX-512F (bit 16 in EBX of leaf 7)
            let avx512f = avx2 && ((result7.ebx & (1 << 16)) != 0);

            // 检查 AVX-512VL (bit 31 in EBX of leaf 7)
            let avx512vl = avx512f && ((result7.ebx & (1 << 31)) != 0);

            // 检查 AVX-512BW (bit 30 in EBX of leaf 7)
            let avx512bw = avx512f && ((result7.ebx & (1 << 30)) != 0);

            super::CPUFeatures {
                avx512f,
                avx512vl,
                avx512bw,
                avx2,
                sse42,
                neon: false,
                sve: false,
                sve2: false,
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
use x86::detect_x86_features;

#[cfg(target_arch = "aarch64")]
mod arm {
    /// 检测 ARM64 CPU 特性
    pub(super) fn detect_arm_features() -> super::CPUFeatures {
        // 在 ARM64 上，NEON 总是可用的
        // SVE 和 SVE2 需要特殊检测，这里简化处理
        super::CPUFeatures {
            avx512f: false,
            avx512vl: false,
            avx512bw: false,
            avx2: false,
            sse42: false,
            neon: true, // ARM64 总是支持 NEON
            sve: false, // 需要额外的系统调用检测
            sve2: false,
        }
    }
}

#[cfg(target_arch = "aarch64")]
use arm::detect_arm_features;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_features_get() {
        let features = CPUFeatures::get();
        println!("检测到的CPU特性: {}", features);

        // 验证至少有一个特性被检测到（除了默认的 none）
        #[cfg(target_arch = "x86_64")]
        assert!(
            features.sse42 || features.avx2 || features.avx512f,
            "x86_64 应该至少支持 SSE4.2"
        );

        #[cfg(target_arch = "aarch64")]
        assert!(features.neon, "ARM64 应该支持 NEON");
    }

    #[test]
    fn test_vector_width() {
        let features = CPUFeatures::get();
        let width_bytes = features.vector_width_bytes();
        let width_elements = features.vector_width_elements_u64();

        println!(
            "向量宽度: {} bytes ({} u64元素)",
            width_bytes, width_elements
        );

        // 验证向量宽度合理
        assert!(width_bytes >= 8, "向量宽度至少应该是8字节");
        assert!(width_bytes <= 64, "向量宽度不应该超过64字节");
        assert!(width_elements >= 1, "至少能容纳1个u64元素");
    }

    #[test]
    fn test_feature_flags() {
        let features = CPUFeatures::get();

        #[cfg(target_arch = "x86_64")]
        {
            // 验证特性依赖关系
            if features.avx512f {
                assert!(features.avx2, "AVX-512 需要 AVX2");
            }
            if features.avx2 {
                assert!(features.sse42, "AVX2 需要 SSE4.2");
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            // ARM64 总是支持 NEON
            assert!(features.neon, "ARM64 必须支持 NEON");
        }
    }

    #[test]
    fn test_display() {
        let features = CPUFeatures::get();
        let display = format!("{}", features);
        println!("CPU特性: {}", display);

        // 验证显示不为空
        assert!(!display.is_empty(), "CPU特性显示不应为空");
    }

    #[test]
    fn test_none() {
        let features = CPUFeatures::none();
        assert!(!features.avx512f);
        assert!(!features.avx2);
        assert!(!features.neon);

        // none() 的向量宽度应该是 8 字节
        assert_eq!(features.vector_width_bytes(), 8);
    }
}
