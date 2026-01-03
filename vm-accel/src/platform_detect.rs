//! Platform detection utilities for testing
//!
//! This module provides runtime platform detection and test skip conditions
//! for platform-specific functionality.

/// Platform detection result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOS,
    Windows,
    IOS,
    TvOS,
    Unknown,
}

impl Platform {
    /// Get current platform
    pub fn current() -> Self {
        #[cfg(target_os = "linux")]
        return Platform::Linux;

        #[cfg(target_os = "macos")]
        return Platform::MacOS;

        #[cfg(target_os = "windows")]
        return Platform::Windows;

        #[cfg(target_os = "ios")]
        return Platform::IOS;

        #[cfg(target_os = "tvos")]
        return Platform::TvOS;

        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows",
            target_os = "ios",
            target_os = "tvos"
        )))]
        Platform::Unknown
    }

    /// Check if this platform supports KVM
    pub fn supports_kvm(&self) -> bool {
        matches!(self, Platform::Linux)
    }

    /// Check if this platform supports HVF
    pub fn supports_hvf(&self) -> bool {
        matches!(self, Platform::MacOS)
    }

    /// Check if this platform supports WHPX
    pub fn supports_whpx(&self) -> bool {
        matches!(self, Platform::Windows)
    }

    /// Check if this platform supports Virtualization.framework
    pub fn supports_vz(&self) -> bool {
        matches!(self, Platform::IOS | Platform::TvOS)
    }

    /// Get platform name as string
    pub fn name(&self) -> &str {
        match self {
            Platform::Linux => "Linux",
            Platform::MacOS => "macOS",
            Platform::Windows => "Windows",
            Platform::IOS => "iOS",
            Platform::TvOS => "tvOS",
            Platform::Unknown => "Unknown",
        }
    }
}

/// Check if KVM device is available at runtime
pub fn is_kvm_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new("/dev/kvm").exists()
    }

    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Check if HVF is available at runtime
pub fn is_hvf_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        // HVF availability check would require actual initialization attempt
        // For now, return true on macOS - actual availability depends on entitlements
        true
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Check if WHPX is available at runtime
pub fn is_whpx_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        // WHPX availability check would require Windows API calls
        // For now, return true on Windows - actual availability depends on Hyper-V
        true
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Test skip reason generator
pub struct SkipReason {
    pub feature: &'static str,
    pub platform: Platform,
    pub required_platforms: &'static [&'static str],
    pub details: &'static str,
}

impl SkipReason {
    /// Create a new skip reason
    pub fn new(
        feature: &'static str,
        platform: Platform,
        required_platforms: &'static [&'static str],
        details: &'static str,
    ) -> Self {
        Self {
            feature,
            platform,
            required_platforms,
            details,
        }
    }

    /// Get the skip reason message
    pub fn message(&self) -> String {
        format!(
            "{} is only available on {}, current platform: {} - {}",
            self.feature,
            self.required_platforms.join("/"),
            self.platform.name(),
            self.details
        )
    }
}

/// Macro to skip test based on platform condition
#[macro_export]
macro_rules! skip_test_if_not {
    ($condition:expr, $reason:expr) => {
        if !$condition {
            println!("Test skipped: {}", $reason);
            return;
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = Platform::current();
        println!("Current platform: {:?}", platform);
        println!("Platform name: {}", platform.name());
    }

    #[test]
    fn test_platform_capabilities() {
        let platform = Platform::current();

        println!("Platform capabilities:");
        println!("  KVM support: {}", platform.supports_kvm());
        println!("  HVF support: {}", platform.supports_hvf());
        println!("  WHPX support: {}", platform.supports_whpx());
        println!("  VZ support: {}", platform.supports_vz());
    }

    #[test]
    fn test_runtime_availability_checks() {
        println!("Runtime availability:");
        println!("  KVM available: {}", is_kvm_available());
        println!("  HVF available: {}", is_hvf_available());
        println!("  WHPX available: {}", is_whpx_available());
    }

    #[test]
    fn test_skip_reason_generation() {
        let platform = Platform::current();
        let reason = SkipReason::new("KVM", platform, &["Linux"], "requires /dev/kvm device");
        println!("{}", reason.message());
    }
}
