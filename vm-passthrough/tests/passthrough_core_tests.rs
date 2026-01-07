//! Core Passthrough Tests
//!
//! Comprehensive tests for core passthrough functionality including:
//! - PciAddress creation and parsing
//! - PciDeviceInfo structure
//! - DeviceType variants
//! - PassthroughError types
//! - PassthroughManager basic operations

use std::str::FromStr;
use vm_passthrough::{DeviceType, PassthroughError, PassthroughManager, PciAddress, PciDeviceInfo};

// ============================================================================
// PciAddress Tests
// ============================================================================

#[test]
fn test_pci_address_new() {
    let addr = PciAddress::new(0x0000, 0x02, 0x1f, 0x0);
    assert_eq!(addr.domain, 0x0000);
    assert_eq!(addr.bus, 0x02);
    assert_eq!(addr.device, 0x1f);
    assert_eq!(addr.function, 0x0);
}

#[test]
fn test_pci_address_display() {
    let addr = PciAddress::new(0x0000, 0x02, 0x1f, 0x0);
    let display = format!("{}", addr);
    assert_eq!(display, "0000:02:1f.0");
}

#[test]
fn test_pci_address_display_full() {
    let addr = PciAddress::new(0xffff, 0xff, 0x1f, 0x7);
    let display = format!("{}", addr);
    assert_eq!(display, "ffff:ff:1f.7");
}

#[test]
fn test_pci_address_from_str_valid() {
    let addr_str = "0000:02:1f.0";
    let result = PciAddress::from_str(addr_str);
    assert!(result.is_ok());

    let addr = result.unwrap();
    assert_eq!(addr.domain, 0x0000);
    assert_eq!(addr.bus, 0x02);
    assert_eq!(addr.device, 0x1f);
    assert_eq!(addr.function, 0x0);
}

#[test]
fn test_pci_address_from_str_valid_hex() {
    let addr_str = "abcd:ef:12.3";
    let result = PciAddress::from_str(addr_str);
    assert!(result.is_ok());

    let addr = result.unwrap();
    assert_eq!(addr.domain, 0xabcd);
    assert_eq!(addr.bus, 0xef);
    assert_eq!(addr.device, 0x12);
    assert_eq!(addr.function, 0x3);
}

#[test]
fn test_pci_address_from_str_invalid_format() {
    let addr_str = "02:1f.0";
    let result = PciAddress::from_str(addr_str);
    assert!(result.is_err());
}

#[test]
fn test_pci_address_from_str_invalid_device_function() {
    let addr_str = "0000:02:1f";
    let result = PciAddress::from_str(addr_str);
    assert!(result.is_err());
}

#[test]
fn test_pci_address_from_str_invalid_hex() {
    let addr_str = "ghij:02:1f.0";
    let result = PciAddress::from_str(addr_str);
    assert!(result.is_err());
}

#[test]
fn test_pci_address_clone() {
    let addr1 = PciAddress::new(0x0000, 0x02, 0x1f, 0x0);
    let addr2 = addr1;
    assert_eq!(addr1.domain, addr2.domain);
    assert_eq!(addr1.bus, addr2.bus);
    assert_eq!(addr1.device, addr2.device);
    assert_eq!(addr1.function, addr2.function);
}

#[test]
fn test_pci_address_equality() {
    let addr1 = PciAddress::new(0x0000, 0x02, 0x1f, 0x0);
    let addr2 = PciAddress::new(0x0000, 0x02, 0x1f, 0x0);
    assert_eq!(addr1, addr2);
}

#[test]
fn test_pci_address_inequality() {
    let addr1 = PciAddress::new(0x0000, 0x02, 0x1f, 0x0);
    let addr2 = PciAddress::new(0x0000, 0x02, 0x1f, 0x1);
    assert_ne!(addr1, addr2);
}

#[test]
fn test_pci_address_debug() {
    let addr = PciAddress::new(0x0000, 0x02, 0x1f, 0x0);
    let debug_str = format!("{:?}", addr);
    assert!(debug_str.contains("PciAddress"));
}

// ============================================================================
// PciDeviceInfo Tests
// ============================================================================

#[test]
fn test_pci_device_info_creation() {
    let addr = PciAddress::new(0x0000, 0x02, 0x1f, 0x0);
    let info = PciDeviceInfo {
        address: addr,
        vendor_id: 0x10de,
        device_id: 0x2204,
        class_code: 0x030000,
        subsystem_vendor_id: 0x10de,
        subsystem_device_id: 0x2204,
        name: "NVIDIA GPU".to_string(),
    };

    assert_eq!(info.address, addr);
    assert_eq!(info.vendor_id, 0x10de);
    assert_eq!(info.device_id, 0x2204);
}

#[test]
fn test_pci_device_info_clone() {
    let addr = PciAddress::new(0x0000, 0x02, 0x1f, 0x0);
    let info1 = PciDeviceInfo {
        address: addr,
        vendor_id: 0x10de,
        device_id: 0x2204,
        class_code: 0x030000,
        subsystem_vendor_id: 0x10de,
        subsystem_device_id: 0x2204,
        name: "NVIDIA GPU".to_string(),
    };

    let info2 = info1.clone();
    assert_eq!(info1.address, info2.address);
    assert_eq!(info1.vendor_id, info2.vendor_id);
    assert_eq!(info1.name, info2.name);
}

#[test]
fn test_pci_device_info_debug() {
    let addr = PciAddress::new(0x0000, 0x02, 0x1f, 0x0);
    let info = PciDeviceInfo {
        address: addr,
        vendor_id: 0x10de,
        device_id: 0x2204,
        class_code: 0x030000,
        subsystem_vendor_id: 0x10de,
        subsystem_device_id: 0x2204,
        name: "NVIDIA GPU".to_string(),
    };

    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("PciDeviceInfo"));
}

// ============================================================================
// DeviceType Tests
// ============================================================================

#[test]
fn test_device_type_unknown() {
    let dt = DeviceType::Unknown;
    assert_eq!(dt, DeviceType::Unknown);
}

#[test]
fn test_device_type_gpu_nvidia() {
    let dt = DeviceType::GpuNvidia;
    assert_eq!(dt, DeviceType::GpuNvidia);
}

#[test]
fn test_device_type_gpu_amd() {
    let dt = DeviceType::GpuAmd;
    assert_eq!(dt, DeviceType::GpuAmd);
}

#[test]
fn test_device_type_gpu_intel() {
    let dt = DeviceType::GpuIntel;
    assert_eq!(dt, DeviceType::GpuIntel);
}

#[test]
fn test_device_type_gpu_mobile() {
    let dt = DeviceType::GpuMobile;
    assert_eq!(dt, DeviceType::GpuMobile);
}

#[test]
fn test_device_type_npu() {
    let dt = DeviceType::Npu;
    assert_eq!(dt, DeviceType::Npu);
}

#[test]
fn test_device_type_network_card() {
    let dt = DeviceType::NetworkCard;
    assert_eq!(dt, DeviceType::NetworkCard);
}

#[test]
fn test_device_type_storage_controller() {
    let dt = DeviceType::StorageController;
    assert_eq!(dt, DeviceType::StorageController);
}

#[test]
fn test_device_type_equality() {
    assert_eq!(DeviceType::GpuNvidia, DeviceType::GpuNvidia);
    assert_ne!(DeviceType::GpuNvidia, DeviceType::GpuAmd);
}

#[test]
fn test_device_type_clone() {
    let dt1 = DeviceType::GpuNvidia;
    let dt2 = dt1;
    assert_eq!(dt1, dt2);
}

#[test]
fn test_device_type_copy() {
    let dt1 = DeviceType::GpuNvidia;
    let dt2 = dt1;
    assert_eq!(dt1, dt2);
    assert_eq!(dt2, DeviceType::GpuNvidia);
}

#[test]
fn test_device_type_debug() {
    let dt = DeviceType::GpuNvidia;
    let debug_str = format!("{:?}", dt);
    assert!(debug_str.contains("GpuNvidia"));
}

// ============================================================================
// PassthroughError Tests
// ============================================================================

#[test]
fn test_passthrough_error_invalid_address() {
    let err = PassthroughError::InvalidAddress("test".to_string());
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Invalid PCI address"));
    assert!(error_msg.contains("test"));
}

#[test]
fn test_passthrough_error_device_not_found() {
    let err = PassthroughError::DeviceNotFound("gpu0".to_string());
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Device not found"));
    assert!(error_msg.contains("gpu0"));
}

#[test]
fn test_passthrough_error_device_in_use() {
    let err = PassthroughError::DeviceInUse;
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Device already in use"));
}

#[test]
fn test_passthrough_error_permission_denied() {
    let err = PassthroughError::PermissionDenied;
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Permission denied"));
}

#[test]
fn test_passthrough_error_iommu_not_enabled() {
    let err = PassthroughError::IommuNotEnabled;
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("IOMMU not enabled"));
}

#[test]
fn test_passthrough_error_driver_binding_failed() {
    let err = PassthroughError::DriverBindingFailed("vfio-pci".to_string());
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Driver binding failed"));
    assert!(error_msg.contains("vfio-pci"));
}

#[test]
fn test_passthrough_error_parse_error() {
    let err = PassthroughError::ParseError;
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Parse error"));
}

#[test]
fn test_passthrough_error_lock_poisoned() {
    let err = PassthroughError::LockPoisoned("devices".to_string());
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Lock poisoned"));
    assert!(error_msg.contains("devices"));
}

#[test]
fn test_passthrough_error_debug() {
    let err = PassthroughError::InvalidAddress("test".to_string());
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("InvalidAddress"));
}

// ============================================================================
// PassthroughManager Tests
// ============================================================================

#[test]
fn test_passthrough_manager_new() {
    let manager = PassthroughManager::new();
    let _ = manager;
}

#[test]
fn test_passthrough_manager_scan_devices() {
    let mut manager = PassthroughManager::new();
    let result = manager.scan_devices();
    let _ = result;
}

#[test]
fn test_passthrough_manager_multiple_new() {
    let _manager1 = PassthroughManager::new();
    let _manager2 = PassthroughManager::new();
    let _manager3 = PassthroughManager::new();
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_pci_address_roundtrip() {
    let addr1 = PciAddress::new(0xabcd, 0xef, 0x12, 0x3);
    let addr_str = format!("{}", addr1);
    let addr2 = PciAddress::from_str(&addr_str).unwrap();
    assert_eq!(addr1, addr2);
}

#[test]
fn test_pci_device_info_with_address() {
    let addr = PciAddress::new(0x0000, 0x01, 0x00, 0x0);
    let info = PciDeviceInfo {
        address: addr,
        vendor_id: 0x8086,
        device_id: 0x1234,
        class_code: 0x020000,
        subsystem_vendor_id: 0x8086,
        subsystem_device_id: 0x1234,
        name: "Test Device".to_string(),
    };

    assert_eq!(format!("{}", info.address), "0000:01:00.0");
}

#[test]
fn test_all_device_types() {
    let types = vec![
        DeviceType::Unknown,
        DeviceType::GpuNvidia,
        DeviceType::GpuAmd,
        DeviceType::GpuIntel,
        DeviceType::GpuMobile,
        DeviceType::Npu,
        DeviceType::NetworkCard,
        DeviceType::StorageController,
    ];

    for dt in types {
        let _ = dt;
        let _clone = dt;
    }
}

#[test]
fn test_all_error_types() {
    let errors = vec![
        PassthroughError::InvalidAddress("test".to_string()),
        PassthroughError::DeviceNotFound("dev0".to_string()),
        PassthroughError::DeviceInUse,
        PassthroughError::PermissionDenied,
        PassthroughError::IommuNotEnabled,
        PassthroughError::DriverBindingFailed("driver".to_string()),
        PassthroughError::ParseError,
        PassthroughError::LockPoisoned("lock".to_string()),
    ];

    for err in errors {
        let _ = format!("{}", err);
        let _ = format!("{:?}", err);
    }
}
