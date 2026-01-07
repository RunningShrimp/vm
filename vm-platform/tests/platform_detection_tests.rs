//! Platform Detection Tests
//!
//! Comprehensive tests for platform detection functionality including:
//! - OS detection (host_os)
//! - Architecture detection (host_arch)
//! - Platform info gathering (PlatformInfo)
//! - Platform-specific paths (PlatformPaths)
//! - Platform features detection (PlatformFeatures)

use vm_platform::{PlatformFeatures, PlatformInfo, PlatformPaths, host_arch, host_os};

// ============================================================================
// OS Detection Tests
// ============================================================================

#[test]
fn test_host_os_returns_value() {
    let os = host_os();
    // OS should be one of the supported values
    let valid_os_values = vec![
        "linux",
        "macos",
        "windows",
        "android",
        "ios",
        "harmonyos",
        "unknown",
    ];
    assert!(
        valid_os_values.contains(&os),
        "host_os returned invalid value: {}",
        os
    );
}

#[test]
fn test_host_os_is_consistent() {
    let os1 = host_os();
    let os2 = host_os();
    assert_eq!(os1, os2, "host_os should return consistent results");
}

#[test]
fn test_host_os_not_empty() {
    let os = host_os();
    assert!(!os.is_empty(), "host_os should not return empty string");
}

// ============================================================================
// Architecture Detection Tests
// ============================================================================

#[test]
fn test_host_arch_returns_value() {
    let arch = host_arch();
    // Architecture should be one of the supported values
    let valid_arch_values = vec!["x86_64", "aarch64", "riscv64", "unknown"];
    assert!(
        valid_arch_values.contains(&arch),
        "host_arch returned invalid value: {}",
        arch
    );
}

#[test]
fn test_host_arch_is_consistent() {
    let arch1 = host_arch();
    let arch2 = host_arch();
    assert_eq!(arch1, arch2, "host_arch should return consistent results");
}

#[test]
fn test_host_arch_not_empty() {
    let arch = host_arch();
    assert!(!arch.is_empty(), "host_arch should not return empty string");
}

#[test]
fn test_host_arch_and_os_combination() {
    let os = host_os();
    let arch = host_arch();
    // Certain combinations should be valid
    // For example, aarch64 should be on macos, linux, or android
    // x86_64 should be on linux, macos, or windows
    // We just verify both return values without panicking
    assert!(!os.is_empty() && !arch.is_empty());
}

// ============================================================================
// PlatformInfo Tests
// ============================================================================

#[test]
fn test_platform_info_get() {
    let info = PlatformInfo::get();
    assert!(!info.os.is_empty(), "PlatformInfo os should not be empty");
    assert!(
        !info.arch.is_empty(),
        "PlatformInfo arch should not be empty"
    );
}

#[test]
fn test_platform_info_os_version() {
    let info = PlatformInfo::get();
    // os_version is optional, so we just verify it doesn't panic
    let _ = &info.os_version;
}

#[test]
fn test_platform_info_cpu_count() {
    let info = PlatformInfo::get();
    assert!(
        info.cpu_count > 0,
        "PlatformInfo cpu_count should be greater than 0, got: {}",
        info.cpu_count
    );
}

#[test]
fn test_platform_info_total_memory() {
    let info = PlatformInfo::get();
    assert!(
        info.total_memory > 0,
        "PlatformInfo total_memory should be greater than 0, got: {}",
        info.total_memory
    );
}

#[test]
fn test_platform_info_is_consistent() {
    let info1 = PlatformInfo::get();
    let info2 = PlatformInfo::get();
    assert_eq!(info1.os, info2.os, "OS should be consistent");
    assert_eq!(info1.arch, info2.arch, "Arch should be consistent");
    assert_eq!(
        info1.cpu_count, info2.cpu_count,
        "CPU count should be consistent"
    );
    assert_eq!(
        info1.total_memory, info2.total_memory,
        "Total memory should be consistent"
    );
}

#[test]
fn test_platform_info_matches_host_functions() {
    let info = PlatformInfo::get();
    let os = host_os();
    let arch = host_arch();
    assert_eq!(info.os, os, "PlatformInfo os should match host_os()");
    assert_eq!(
        info.arch, arch,
        "PlatformInfo arch should match host_arch()"
    );
}

#[test]
fn test_platform_info_debug_trait() {
    let info = PlatformInfo::get();
    let debug_str = format!("{:?}", info);
    assert!(
        debug_str.contains("PlatformInfo"),
        "Debug output should contain struct name"
    );
}

// ============================================================================
// PlatformPaths Tests
// ============================================================================

#[test]
fn test_platform_paths_get() {
    let paths = PlatformPaths::get();
    // All paths should be non-empty
    assert!(
        !paths.config_dir.is_empty(),
        "config_dir should not be empty"
    );
    assert!(!paths.data_dir.is_empty(), "data_dir should not be empty");
    assert!(!paths.cache_dir.is_empty(), "cache_dir should not be empty");
}

#[test]
fn test_platform_paths_config_dir() {
    let paths = PlatformPaths::get();
    // Config directory should be an absolute path or a valid relative path
    assert!(
        paths.config_dir.len() > 0,
        "config_dir should have length > 0"
    );
}

#[test]
fn test_platform_paths_data_dir() {
    let paths = PlatformPaths::get();
    // Data directory should be an absolute path or a valid relative path
    assert!(paths.data_dir.len() > 0, "data_dir should have length > 0");
}

#[test]
fn test_platform_paths_cache_dir() {
    let paths = PlatformPaths::get();
    // Cache directory should be an absolute path or a valid relative path
    assert!(
        paths.cache_dir.len() > 0,
        "cache_dir should have length > 0"
    );
}

#[test]
fn test_platform_paths_are_consistent() {
    let paths1 = PlatformPaths::get();
    let paths2 = PlatformPaths::get();
    assert_eq!(
        paths1.config_dir, paths2.config_dir,
        "config_dir should be consistent"
    );
    assert_eq!(
        paths1.data_dir, paths2.data_dir,
        "data_dir should be consistent"
    );
    assert_eq!(
        paths1.cache_dir, paths2.cache_dir,
        "cache_dir should be consistent"
    );
}

#[test]
fn test_platform_paths_debug_trait() {
    let paths = PlatformPaths::get();
    let debug_str = format!("{:?}", paths);
    assert!(
        debug_str.contains("PlatformPaths"),
        "Debug output should contain struct name"
    );
}

// ============================================================================
// PlatformFeatures Tests
// ============================================================================

#[test]
fn test_platform_features_detect() {
    let features = PlatformFeatures::detect();
    // Should not panic and should return a valid struct
    let _ = features.hardware_virtualization;
    let _ = features.gpu_acceleration;
    let _ = features.network_passthrough;
}

#[test]
fn test_platform_features_hardware_virtualization_is_bool() {
    let features = PlatformFeatures::detect();
    // Just verify it's a boolean value (either true or false is valid)
    let _ = features.hardware_virtualization;
}

#[test]
fn test_platform_features_gpu_acceleration_is_bool() {
    let features = PlatformFeatures::detect();
    // Just verify it's a boolean value
    let _ = features.gpu_acceleration;
}

#[test]
fn test_platform_features_network_passthrough_is_bool() {
    let features = PlatformFeatures::detect();
    // Just verify it's a boolean value
    let _ = features.network_passthrough;
}

#[test]
fn test_platform_features_is_consistent() {
    let features1 = PlatformFeatures::detect();
    let features2 = PlatformFeatures::detect();
    // Features detection should be consistent within the same run
    assert_eq!(
        features1.hardware_virtualization, features2.hardware_virtualization,
        "hardware_virtualization should be consistent"
    );
    assert_eq!(
        features1.gpu_acceleration, features2.gpu_acceleration,
        "gpu_acceleration should be consistent"
    );
    assert_eq!(
        features1.network_passthrough, features2.network_passthrough,
        "network_passthrough should be consistent"
    );
}

#[test]
fn test_platform_features_at_least_one_supported() {
    let features = PlatformFeatures::detect();
    // At least one feature should ideally be supported, but we don't enforce this
    // Some platforms may not support any of these features
    let _ = (
        features.hardware_virtualization,
        features.gpu_acceleration,
        features.network_passthrough,
    );
}

#[test]
fn test_platform_features_debug_trait() {
    let features = PlatformFeatures::detect();
    let debug_str = format!("{:?}", features);
    assert!(
        debug_str.contains("PlatformFeatures"),
        "Debug output should contain struct name"
    );
}

#[test]
fn test_platform_features_clone_trait() {
    let features1 = PlatformFeatures::detect();
    let features2 = features1.clone();
    assert_eq!(
        features1.hardware_virtualization, features2.hardware_virtualization,
        "Cloned features should match"
    );
    assert_eq!(
        features1.gpu_acceleration, features2.gpu_acceleration,
        "Cloned features should match"
    );
    assert_eq!(
        features1.network_passthrough, features2.network_passthrough,
        "Cloned features should match"
    );
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_platform_info_integration() {
    let info = PlatformInfo::get();
    let paths = PlatformPaths::get();
    let features = PlatformFeatures::detect();

    // All should return without panicking
    assert!(!info.os.is_empty());
    assert!(!paths.config_dir.is_empty());
    // features just needs to exist
    let _ = features;
}

#[test]
fn test_host_os_arch_and_platform_info_consistency() {
    let os = host_os();
    let arch = host_arch();
    let info = PlatformInfo::get();

    assert_eq!(os, info.os, "host_os should match PlatformInfo.os");
    assert_eq!(arch, info.arch, "host_arch should match PlatformInfo.arch");
}

#[test]
fn test_all_platform_functions_no_panic() {
    // This test ensures all public platform functions work without panicking
    let _ = host_os();
    let _ = host_arch();
    let _ = PlatformInfo::get();
    let _ = PlatformPaths::get();
    let _ = PlatformFeatures::detect();
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_multiple_platform_info_calls() {
    // Multiple calls should all succeed
    let _ = PlatformInfo::get();
    let _ = PlatformInfo::get();
    let _ = PlatformInfo::get();
    let _ = PlatformInfo::get();
}

#[test]
fn test_multiple_platform_paths_calls() {
    // Multiple calls should all succeed
    let _ = PlatformPaths::get();
    let _ = PlatformPaths::get();
    let _ = PlatformPaths::get();
    let _ = PlatformPaths::get();
}

#[test]
fn test_multiple_platform_features_calls() {
    // Multiple calls should all succeed
    let _ = PlatformFeatures::detect();
    let _ = PlatformFeatures::detect();
    let _ = PlatformFeatures::detect();
    let _ = PlatformFeatures::detect();
}

#[test]
fn test_platform_info_fields_are_accessible() {
    let info = PlatformInfo::get();
    // Just verify all fields are accessible
    let _ = info.os;
    let _ = info.os_version.clone();
    let _ = info.arch;
    let _ = info.cpu_count;
    let _ = info.total_memory;
}

#[test]
fn test_platform_paths_fields_are_accessible() {
    let paths = PlatformPaths::get();
    // Just verify all fields are accessible
    let _ = paths.config_dir.clone();
    let _ = paths.data_dir.clone();
    let _ = paths.cache_dir.clone();
}
