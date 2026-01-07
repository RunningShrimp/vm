//! vm-boot核心功能测试
//!
//! 测试启动配置、启动方式、运行时控制等核心功能

use vm_boot::{BootConfig, BootMethod};
use vm_core::GuestAddr;

#[cfg(test)]
mod boot_config_tests {
    use super::*;

    // Test 1: BootConfig默认配置
    #[test]
    fn test_boot_config_default() {
        let config = BootConfig::default();

        assert_eq!(config.method, BootMethod::Direct);
        assert!(config.kernel.is_none());
        assert!(config.cmdline.is_none());
        assert!(config.initrd.is_none());
        assert!(config.firmware.is_none());
        assert!(config.iso.is_none());
        assert_eq!(config.kernel_load_addr, GuestAddr(0x80000000));
        assert_eq!(config.initrd_load_addr, GuestAddr(0x84000000));
    }

    // Test 2: 创建Direct Boot配置
    #[test]
    fn test_boot_config_direct() {
        let config = BootConfig::new(BootMethod::Direct);

        assert_eq!(config.method, BootMethod::Direct);
        assert_eq!(config.kernel_load_addr, GuestAddr(0x80000000));
    }

    // Test 3: 创建UEFI Boot配置
    #[test]
    fn test_boot_config_uefi() {
        let config = BootConfig::new(BootMethod::Uefi);

        assert_eq!(config.method, BootMethod::Uefi);
        assert_eq!(config.kernel_load_addr, GuestAddr(0x80000000));
    }

    // Test 4: 创建BIOS Boot配置
    #[test]
    fn test_boot_config_bios() {
        let config = BootConfig::new(BootMethod::Bios);

        assert_eq!(config.method, BootMethod::Bios);
        assert_eq!(config.kernel_load_addr, GuestAddr(0x80000000));
    }

    // Test 5: 创建ISO Boot配置
    #[test]
    fn test_boot_config_iso() {
        let config = BootConfig::new(BootMethod::Iso);

        assert_eq!(config.method, BootMethod::Iso);
    }

    // Test 6: Builder模式 - with_kernel
    #[test]
    fn test_boot_config_builder_kernel() {
        let config = BootConfig::new(BootMethod::Direct).with_kernel("/path/to/kernel");

        assert_eq!(config.kernel, Some("/path/to/kernel".to_string()));
    }

    // Test 7: Builder模式 - with_cmdline
    #[test]
    fn test_boot_config_builder_cmdline() {
        let config = BootConfig::new(BootMethod::Direct).with_cmdline("console=ttyS0");

        assert_eq!(config.cmdline, Some("console=ttyS0".to_string()));
    }

    // Test 8: Builder模式 - with_initrd
    #[test]
    fn test_boot_config_builder_initrd() {
        let config = BootConfig::new(BootMethod::Direct).with_initrd("/path/to/initrd");

        assert_eq!(config.initrd, Some("/path/to/initrd".to_string()));
    }

    // Test 9: Builder模式 - with_firmware
    #[test]
    fn test_boot_config_builder_firmware() {
        let config = BootConfig::new(BootMethod::Uefi).with_firmware("/path/to/uefi firmware");

        assert_eq!(config.firmware, Some("/path/to/uefi firmware".to_string()));
    }

    // Test 10: Builder模式 - with_iso
    #[test]
    fn test_boot_config_builder_iso() {
        let config = BootConfig::new(BootMethod::Iso).with_iso("/path/to/image.iso");

        assert_eq!(config.iso, Some("/path/to/image.iso".to_string()));
    }

    // Test 11: 完整的Direct Boot配置
    #[test]
    fn test_boot_config_complete_direct() {
        let config = BootConfig::new(BootMethod::Direct)
            .with_kernel("/boot/vmlinuz")
            .with_cmdline("root=/dev/sda1")
            .with_initrd("/boot/initramfs");

        assert_eq!(config.method, BootMethod::Direct);
        assert_eq!(config.kernel, Some("/boot/vmlinuz".to_string()));
        assert_eq!(config.cmdline, Some("root=/dev/sda1".to_string()));
        assert_eq!(config.initrd, Some("/boot/initramfs".to_string()));
    }

    // Test 12: 完整的UEFI Boot配置
    #[test]
    fn test_boot_config_complete_uefi() {
        let config = BootConfig::new(BootMethod::Uefi)
            .with_firmware("/usr/share/edk2/x64/OVMF.fd")
            .with_iso("/path/to/ubuntu.iso");

        assert_eq!(config.method, BootMethod::Uefi);
        assert_eq!(
            config.firmware,
            Some("/usr/share/edk2/x64/OVMF.fd".to_string())
        );
        assert_eq!(config.iso, Some("/path/to/ubuntu.iso".to_string()));
    }

    // Test 13: BootMethod枚举比较
    #[test]
    fn test_boot_method_equality() {
        assert_eq!(BootMethod::Direct, BootMethod::Direct);
        assert_ne!(BootMethod::Direct, BootMethod::Uefi);
        assert_ne!(BootMethod::Uefi, BootMethod::Bios);
    }

    // Test 14: BootMethod克隆
    #[test]
    fn test_boot_method_clone() {
        let method1 = BootMethod::Direct;
        let method2 = method1;

        assert_eq!(method1, method2);
    }

    // Test 15: BootConfig克隆
    #[test]
    fn test_boot_config_clone() {
        let config1 = BootConfig::new(BootMethod::Direct)
            .with_kernel("/boot/kernel")
            .with_cmdline("console=ttyS0");

        let config2 = config1.clone();

        assert_eq!(config1.method, config2.method);
        assert_eq!(config1.kernel, config2.kernel);
        assert_eq!(config1.cmdline, config2.cmdline);
    }

    // Test 16: 自定义加载地址
    #[test]
    fn test_boot_config_custom_addresses() {
        let mut config = BootConfig::new(BootMethod::Direct);

        config.kernel_load_addr = GuestAddr(0x40000000);
        config.initrd_load_addr = GuestAddr(0x44000000);

        assert_eq!(config.kernel_load_addr, GuestAddr(0x40000000));
        assert_eq!(config.initrd_load_addr, GuestAddr(0x44000000));
    }

    // Test 17: BootConfig Debug trait
    #[test]
    fn test_boot_config_debug() {
        let config = BootConfig::new(BootMethod::Direct).with_kernel("/boot/kernel");

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("Direct"));
        assert!(debug_str.contains("/boot/kernel"));
    }

    // Test 18: 不同架构的默认加载地址
    #[test]
    fn test_boot_config_default_architecture() {
        let config = BootConfig::default();

        // RISC-V和ARM64的默认地址
        assert_eq!(config.kernel_load_addr, GuestAddr(0x80000000));
        assert_eq!(config.initrd_load_addr, GuestAddr(0x84000000));
    }

    // Test 19: Builder链式调用
    #[test]
    fn test_boot_config_builder_chain() {
        let config = BootConfig::new(BootMethod::Direct)
            .with_kernel("/kernel")
            .with_cmdline("console=ttyS0 root=/dev/sda1")
            .with_initrd("/initrd")
            .with_firmware("/firmware");

        assert_eq!(config.kernel, Some("/kernel".to_string()));
        assert_eq!(
            config.cmdline,
            Some("console=ttyS0 root=/dev/sda1".to_string())
        );
        assert_eq!(config.initrd, Some("/initrd".to_string()));
        assert_eq!(config.firmware, Some("/firmware".to_string()));
    }

    // Test 20: BootConfig部分配置
    #[test]
    fn test_boot_config_partial() {
        let config = BootConfig::new(BootMethod::Iso).with_iso("/path/to/image.iso");

        assert_eq!(config.method, BootMethod::Iso);
        assert_eq!(config.iso, Some("/path/to/image.iso".to_string()));
        assert!(config.kernel.is_none()); // 未设置kernel
        assert!(config.firmware.is_none()); // 未设置firmware
    }
}
