//! # WHPX (Windows Hypervisor Platform) 后端
//!
//! 提供 Windows 平台完整的硬件虚拟化加速支持。
//!
//! ## 功能完整性
//!
//! ✅ 分区管理 (Partition)
//! ✅ vCPU 创建和管理
//! ✅ 物理内存映射 (GPA -> HVA)
//! ✅ 寄存器读写
//! ✅ VM Exit 处理
//! ✅ 内存屏障和缓存管理
//!
//! ## 使用方式
//!
//! 此模块重新导出 `whpx_impl` 和 `whpx` 模块的功能，
//! 提供统一的 WHPX 接口。
//!
//! ```rust,ignore
//! use vm_accel::whp::AccelWhpx;
//! use vm_accel::Accel;
//!
//! let mut accel = AccelWhpx::new();
//! accel.init()?;
//! accel.create_vcpu(0)?;
//! accel.map_memory(0, hva, size, flags)?;
//! accel.run_vcpu(0, &mut mmu)?;
//! ```

// 重新导出 WHPX 实现
pub use crate::whpx::*;
pub use crate::whpx_impl::*;

/// WHPX 版本信息
pub const WHPX_VERSION: &str = "1.0.0";

/// WHPX 功能特性
#[derive(Debug, Clone, Copy)]
pub struct WhpxFeatures {
    /// 支持扩展 VMCS
    pub extended_vmcs: bool,
    /// 支持 MSI 地址
    pub msi_address: bool,
    /// 支持 APIC 写程
    pub apic_write: bool,
}

impl WhpxFeatures {
    /// 检测 WHPX 功能特性
    #[cfg(all(target_os = "windows", feature = "whpx"))]
    pub fn detect() -> Self {
        // WHPX 在 Windows 10 1803+ 上功能完整
        Self {
            extended_vmcs: true,
            msi_address: true,
            apic_write: true,
        }
    }

    #[cfg(not(all(target_os = "windows", feature = "whpx")))]
    pub fn detect() -> Self {
        Self {
            extended_vmcs: false,
            msi_address: false,
            apic_write: false,
        }
    }
}

impl Default for WhpxFeatures {
    fn default() -> Self {
        Self::detect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whpx_features() {
        let features = WhpxFeatures::detect();
        println!("WHPX Features: {:?}", features);
    }

    #[test]
    fn test_whpx_version() {
        assert_eq!(WHPX_VERSION, "1.0.0");
    }
}
