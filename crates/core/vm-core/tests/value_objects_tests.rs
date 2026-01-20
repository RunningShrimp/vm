//! 值对象单元测试

use vm_core::{DeviceId, GuestAddr, MemorySize, PortNumber, VcpuCount, VmId};

#[test]
fn test_guest_addr_creation() {
    let addr = GuestAddr(0x1000);
    assert_eq!(addr.0, 0x1000);
}

#[test]
fn test_guest_addr_arithmetic() {
    let addr = GuestAddr(0x1000);
    let new_addr = addr + 0x100;
    assert_eq!(new_addr.0, 0x1100);
}

#[test]
fn test_memory_size_from_bytes_valid() {
    let size = MemorySize::from_bytes(1024 * 1024);
    assert!(size.is_ok());
    assert_eq!(size.unwrap().bytes(), 1024 * 1024);
}

#[test]
fn test_memory_size_from_mb_valid() {
    let size = MemorySize::from_mb(256);
    assert!(size.is_ok());
    assert_eq!(size.unwrap().as_mb(), 256);
}

#[test]
fn test_memory_size_from_gb_valid() {
    let size = MemorySize::from_gb(4);
    assert!(size.is_ok());
    assert_eq!(size.unwrap().as_gb(), 4);
}

#[test]
fn test_memory_size_zero_invalid() {
    let size = MemorySize::from_bytes(0);
    assert!(size.is_err());
}

#[test]
fn test_memory_size_page_aligned() {
    let size = MemorySize::from_bytes(4096).unwrap();
    assert!(size.is_page_aligned());

    let size = MemorySize::from_bytes(4095).unwrap();
    assert!(!size.is_page_aligned());
}

#[test]
fn test_memory_size_conversions() {
    let size = MemorySize::from_mb(1).unwrap();
    assert_eq!(size.bytes(), 1024 * 1024);
    assert_eq!(size.as_mb(), 1);
    assert_eq!(size.as_gb(), 0);
}

#[test]
fn test_vm_id_valid() {
    let id = VmId::new("vm-123".to_string());
    assert!(id.is_ok());
    assert_eq!(id.unwrap().as_str(), "vm-123");
}

#[test]
fn test_vm_id_too_short() {
    let id = VmId::new("".to_string());
    assert!(id.is_err());
}

#[test]
fn test_vm_id_too_long() {
    let id = VmId::new("a".repeat(100));
    assert!(id.is_err());
}

#[test]
fn test_vm_id_invalid_characters() {
    let id = VmId::new("vm@123".to_string());
    assert!(id.is_err());
}

#[test]
fn test_vm_id_valid_with_hyphen() {
    let id = VmId::new("vm-test-123".to_string());
    assert!(id.is_ok());
}

#[test]
fn test_vm_id_valid_with_underscore() {
    let id = VmId::new("vm_test_123".to_string());
    assert!(id.is_ok());
}

#[test]
fn test_vcpu_count_valid() {
    let count = VcpuCount::new(4);
    assert!(count.is_ok());
    assert_eq!(count.unwrap().count(), 4);
}

#[test]
fn test_vcpu_count_zero_invalid() {
    let count = VcpuCount::new(0);
    assert!(count.is_err());
}

#[test]
fn test_vcpu_count_too_large() {
    let count = VcpuCount::new(1000);
    assert!(count.is_err());
}

#[test]
fn test_vcpu_count_ord_comparison() {
    let count1 = VcpuCount::new(2).unwrap();
    let count2 = VcpuCount::new(4).unwrap();
    assert!(count2 > count1);
}

#[test]
fn test_port_number_creation() {
    let port = PortNumber::new(8080);
    assert_eq!(port.port(), 8080);
}

#[test]
fn test_port_number_is_privileged() {
    let port = PortNumber::new(80);
    assert!(port.is_privileged());

    let port = PortNumber::new(8080);
    assert!(!port.is_privileged());
}

#[test]
fn test_port_number_comparison() {
    let port1 = PortNumber::new(80);
    let port2 = PortNumber::new(443);
    assert!(port2 > port1);
}

#[test]
fn test_device_id_valid() {
    let id = DeviceId::new("uart0".to_string());
    assert!(id.is_ok());
    assert_eq!(id.unwrap().as_str(), "uart0");
}

#[test]
fn test_device_id_empty_invalid() {
    let id = DeviceId::new("".to_string());
    assert!(id.is_err());
}
