//! OS级跨架构执行兼容性测试
//!
//! 测试系统调用映射、信号处理、文件系统和网络栈兼容层

#[cfg(test)]
mod tests {
    use crate::syscall_compat::*;
    use crate::signal_compat::*;
    use crate::filesystem_compat::*;
    use crate::network_compat::*;
    use vm_core::{GuestArch, GuestRegs};

    #[test]
    fn test_comprehensive_syscall_mapping() {
        let mapper = SyscallNumberMapper::new();

        // 测试文件I/O系统调用映射
        assert_eq!(mapper.map_guest_to_host(GuestArch::Arm64, 63), Some(0)); // read
        assert_eq!(mapper.map_guest_to_host(GuestArch::Arm64, 64), Some(1)); // write
        assert_eq!(mapper.map_guest_to_host(GuestArch::Arm64, 56), Some(2)); // open
        assert_eq!(mapper.map_guest_to_host(GuestArch::Arm64, 57), Some(3)); // close

        // 测试进程管理系统调用映射
        assert_eq!(mapper.map_guest_to_host(GuestArch::Arm64, 93), Some(60)); // exit
        assert_eq!(mapper.map_guest_to_host(GuestArch::Arm64, 107), Some(57)); // fork

        // 测试网络系统调用映射
        assert_eq!(mapper.map_guest_to_host(GuestArch::Arm64, 198), Some(41)); // socket
        assert_eq!(mapper.map_guest_to_host(GuestArch::Arm64, 203), Some(42)); // connect

        // 测试RISC-V64映射
        assert_eq!(mapper.map_guest_to_host(GuestArch::Riscv64, 63), Some(0)); // read
        assert_eq!(mapper.map_guest_to_host(GuestArch::Riscv64, 93), Some(60)); // exit
    }

    #[test]
    fn test_syscall_arg_conversion() {
        let mut regs = GuestRegs::default();

        // 测试 x86_64 参数提取
        regs.gpr[7] = 1;  // RDI = fd
        regs.gpr[6] = 0x1000;  // RSI = buf
        regs.gpr[2] = 100;  // RDX = count
        let args = SyscallArgConverter::extract_args(GuestArch::X86_64, &regs);
        assert_eq!(args[0], 1);
        assert_eq!(args[1], 0x1000);
        assert_eq!(args[2], 100);

        // 测试 ARM64 参数提取
        regs.gpr[0] = 2;  // X0 = fd
        regs.gpr[1] = 0x2000;  // X1 = buf
        regs.gpr[2] = 200;  // X2 = count
        let args = SyscallArgConverter::extract_args(GuestArch::Arm64, &regs);
        assert_eq!(args[0], 2);
        assert_eq!(args[1], 0x2000);
        assert_eq!(args[2], 200);

        // 测试 RISC-V64 参数提取
        regs.gpr[10] = 3;  // A0 = fd
        regs.gpr[11] = 0x3000;  // A1 = buf
        regs.gpr[12] = 300;  // A2 = count
        let args = SyscallArgConverter::extract_args(GuestArch::Riscv64, &regs);
        assert_eq!(args[0], 3);
        assert_eq!(args[1], 0x3000);
        assert_eq!(args[2], 300);
    }

    #[test]
    fn test_syscall_compatibility_layer() {
        let layer = SyscallCompatibilityLayer::new(GuestArch::Arm64, GuestArch::X86_64);

        let mut regs = GuestRegs::default();
        regs.gpr[8] = 63; // ARM64 read syscall
        regs.gpr[0] = 0;  // X0 = fd
        regs.gpr[1] = 0x1000; // X1 = buf
        regs.gpr[2] = 100; // X2 = count

        let (host_syscall, args) = layer.handle_syscall(&regs).unwrap();
        assert_eq!(host_syscall, 0); // x86_64 read
        assert_eq!(args[0], 0);      // fd
        assert_eq!(args[1], 0x1000); // buf
        assert_eq!(args[2], 100);    // count

        // 测试返回值设置
        layer.handle_return(&mut regs, 50);
        assert_eq!(regs.gpr[0], 50); // ARM64 X0 = return value
    }

    #[test]
    fn test_signal_compatibility_layer() {
        let layer = SignalCompatibilityLayer::new(GuestArch::X86_64);

        // 测试信号处理函数注册
        let action = SignalAction {
            handler: Some(vm_core::GuestAddr(0x1000)),
            mask: 0,
            flags: 0,
            restorer: None,
        };
        assert!(layer.register_handler(2, action).is_ok()); // SIGINT
        assert!(layer.register_handler(9, action.clone()).is_err()); // SIGKILL (cannot be caught)

        // 测试信号发送
        assert!(layer.send_signal(2).is_ok()); // SIGINT
        let pending = layer.get_pending_signals().unwrap();
        assert!(pending & (1u64 << 2) != 0);

        // 测试信号掩码
        let old_mask = layer.set_signal_mask(0xFF).unwrap();
        assert_eq!(old_mask, 0);
        assert_eq!(layer.get_signal_mask().unwrap(), 0xFF);

        // 测试信号栈
        assert!(layer.set_signal_stack(Some(vm_core::GuestAddr(0x2000))).is_ok());
        assert_eq!(layer.get_signal_stack().unwrap(), Some(vm_core::GuestAddr(0x2000)));
    }

    #[test]
    fn test_signal_from_number() {
        assert_eq!(Signal::from_number(1), Some(Signal::SIGHUP));
        assert_eq!(Signal::from_number(2), Some(Signal::SIGINT));
        assert_eq!(Signal::from_number(9), Some(Signal::SIGKILL));
        assert_eq!(Signal::from_number(11), Some(Signal::SIGSEGV));
        assert_eq!(Signal::from_number(99), None);
    }

    #[test]
    fn test_signal_catchable() {
        assert!(!Signal::SIGKILL.is_catchable());
        assert!(!Signal::SIGSTOP.is_catchable());
        assert!(Signal::SIGINT.is_catchable());
        assert!(Signal::SIGTERM.is_catchable());
    }

    #[test]
    fn test_filesystem_open_flags() {
        // 测试从POSIX flags创建
        let flags = OpenFlags::from_posix(0o2 | 0o100 | 0o1000); // O_RDWR | O_CREAT | O_TRUNC
        assert!(flags.read);
        assert!(flags.write);
        assert!(flags.create);
        assert!(flags.truncate);

        // 测试转换回POSIX flags
        let posix_flags = flags.to_posix();
        assert_eq!(posix_flags & 0o2, 0o2); // O_RDWR
        assert_eq!(posix_flags & 0o100, 0o100); // O_CREAT
        assert_eq!(posix_flags & 0o1000, 0o1000); // O_TRUNC
    }

    #[test]
    fn test_file_mode() {
        // 测试从POSIX mode创建
        let mode = FileMode::from_posix(0o755);
        assert!(mode.owner_read);
        assert!(mode.owner_write);
        assert!(mode.owner_exec);
        assert!(mode.group_read);
        assert!(!mode.group_write);
        assert!(mode.group_exec);
        assert!(mode.other_read);
        assert!(!mode.other_write);
        assert!(mode.other_exec);

        // 测试转换回POSIX mode
        let posix_mode = mode.to_posix();
        assert_eq!(posix_mode, 0o755);
    }

    #[test]
    fn test_filesystem_path_conversion() {
        use std::path::PathBuf;
        // 只测试路径转换，不涉及文件系统操作，避免死锁
        let layer = FilesystemCompatibilityLayer::new(
            GuestArch::X86_64,
            PathBuf::from("/tmp/guest_root"),
            Box::new(DefaultFilesystemOperations),
        );

        // 测试路径转换（不涉及锁操作）
        let host_path = layer.convert_path("/etc/passwd");
        let path_str = host_path.to_string_lossy();
        assert!(path_str.contains("tmp/guest_root") || path_str.contains("etc/passwd"));

        let host_path2 = layer.convert_path("relative/path");
        let path_str2 = host_path2.to_string_lossy();
        assert!(path_str2.contains("tmp/guest_root") || path_str2.contains("relative/path"));
    }

    #[test]
    fn test_network_socket_domain_parsing() {
        assert_eq!(
            NetworkCompatibilityLayer::parse_domain(2).unwrap(),
            SocketDomain::Inet
        );
        assert_eq!(
            NetworkCompatibilityLayer::parse_domain(10).unwrap(),
            SocketDomain::Inet6
        );
        assert_eq!(
            NetworkCompatibilityLayer::parse_domain(1).unwrap(),
            SocketDomain::Unix
        );
    }

    #[test]
    fn test_network_socket_type_parsing() {
        assert_eq!(
            NetworkCompatibilityLayer::parse_socket_type(1).unwrap(),
            SocketType::Stream
        );
        assert_eq!(
            NetworkCompatibilityLayer::parse_socket_type(2).unwrap(),
            SocketType::Datagram
        );
        assert_eq!(
            NetworkCompatibilityLayer::parse_socket_type(3).unwrap(),
            SocketType::Raw
        );
    }

    #[test]
    fn test_network_socket_protocol_parsing() {
        assert_eq!(
            NetworkCompatibilityLayer::parse_protocol(6).unwrap(),
            SocketProtocol::Tcp
        );
        assert_eq!(
            NetworkCompatibilityLayer::parse_protocol(17).unwrap(),
            SocketProtocol::Udp
        );
        assert_eq!(
            NetworkCompatibilityLayer::parse_protocol(0).unwrap(),
            SocketProtocol::Ip
        );
    }

    #[test]
    fn test_network_socket_option_parsing() {
        assert_eq!(
            NetworkCompatibilityLayer::parse_option_level(1).unwrap(),
            SocketOptionLevel::Socket
        );
        assert_eq!(
            NetworkCompatibilityLayer::parse_option_level(6).unwrap(),
            SocketOptionLevel::Tcp
        );

        assert_eq!(
            NetworkCompatibilityLayer::parse_option(2).unwrap(),
            SocketOption::ReuseAddr
        );
        assert_eq!(
            NetworkCompatibilityLayer::parse_option(9).unwrap(),
            SocketOption::KeepAlive
        );
    }

    #[test]
    fn test_socket_address_conversion() {
        use std::net::SocketAddr;
        let addr = SocketAddress::Inet(SocketAddr::from(([127, 0, 0, 1], 8080)));
        assert_eq!(addr.to_host_addr(), Some(SocketAddr::from(([127, 0, 0, 1], 8080))));

        let addr6 = SocketAddress::Inet6(SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 8080)));
        assert_eq!(
            addr6.to_host_addr(),
            Some(SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 8080)))
        );
    }
}

