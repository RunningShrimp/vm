//! 系统调用兼容层
//!
//! 提供跨架构系统调用兼容性，包括：
//! - 系统调用号映射（不同架构的系统调用号可能不同）
//! - 参数转换（寄存器布局差异）
//! - 返回值处理

use std::collections::HashMap;
use vm_core::{GuestArch, GuestRegs, VmError};
use tracing::{debug, trace};

/// 系统调用号映射表
/// 
/// 不同架构的系统调用号可能不同，例如：
/// - x86_64: read = 0, write = 1
/// - ARM64: read = 63, write = 64
/// - RISC-V64: read = 63, write = 64
pub struct SyscallNumberMapper {
    /// 从 guest 架构到 host 架构的系统调用号映射
    guest_to_host: HashMap<(GuestArch, u64), u64>,
    /// 从 host 架构到 guest 架构的系统调用号映射
    host_to_guest: HashMap<(GuestArch, u64), u64>,
}

impl SyscallNumberMapper {
    /// 创建新的系统调用号映射器
    pub fn new() -> Self {
        let mut mapper = Self {
            guest_to_host: HashMap::new(),
            host_to_guest: HashMap::new(),
        };
        
        // 初始化常用系统调用的映射
        mapper.init_common_syscalls();
        mapper
    }

    /// 初始化常用系统调用映射
    fn init_common_syscalls(&mut self) {
        // Linux 系统调用号定义（基于内核版本 5.x）
        // x86_64
        let x86_64_syscalls = vec![
            // 文件I/O
            (0, "read"),
            (1, "write"),
            (2, "open"),
            (3, "close"),
            (4, "stat"),
            (5, "fstat"),
            (6, "lstat"),
            (8, "lseek"),
            (9, "mmap"),
            (10, "mprotect"),
            (11, "munmap"),
            (12, "brk"),
            (72, "fcntl"),
            (73, "flock"),
            (74, "fsync"),
            (75, "fdatasync"),
            (76, "truncate"),
            (77, "ftruncate"),
            (78, "getdents"),
            (79, "getcwd"),
            (80, "chdir"),
            (81, "fchdir"),
            (82, "rename"),
            (83, "mkdir"),
            (84, "rmdir"),
            (85, "creat"),
            (86, "link"),
            (87, "unlink"),
            (88, "symlink"),
            (89, "readlink"),
            (90, "chmod"),
            (91, "fchmod"),
            (92, "chown"),
            (93, "fchown"),
            (94, "lchown"),
            (95, "umask"),
            // 进程管理
            (56, "clone"),
            (57, "fork"),
            (58, "vfork"),
            (59, "execve"),
            (60, "exit"),
            (61, "wait4"),
            (62, "kill"),
            (39, "getpid"),
            (231, "exit_group"),
            // 信号处理
            (13, "rt_sigaction"),
            (14, "rt_sigprocmask"),
            (15, "rt_sigpending"),
            (131, "sigaltstack"),
            (34, "pause"),
            // 定时器
            (35, "nanosleep"),
            (36, "getitimer"),
            (37, "alarm"),
            (38, "setitimer"),
            // 网络
            (41, "socket"),
            (42, "connect"),
            (43, "accept"),
            (44, "sendto"),
            (45, "recvfrom"),
            (48, "shutdown"),
            (50, "listen"),
            (51, "getsockname"),
            (52, "getpeername"),
            (53, "socketpair"),
            (54, "setsockopt"),
            (55, "getsockopt"),
            // 其他
            (63, "uname"),
            (40, "sendfile"),
            (7, "poll"),
        ];

        // ARM64 (使用 x86_64 作为参考)
        let arm64_syscalls = vec![
            // 文件I/O
            (63, "read"),
            (64, "write"),
            (56, "open"),
            (57, "close"),
            (106, "stat"),
            (80, "fstat"),
            (6, "lstat"),
            (62, "lseek"),
            (222, "mmap"),
            (226, "mprotect"),
            (215, "munmap"),
            (214, "brk"),
            (25, "fcntl"),
            (32, "flock"),
            (82, "fsync"),
            (83, "fdatasync"),
            (45, "truncate"),
            (46, "ftruncate"),
            (61, "getdents"),
            (17, "getcwd"),
            (49, "chdir"),
            (50, "fchdir"),
            (38, "rename"),
            (34, "mkdir"),
            (40, "rmdir"),
            (8, "creat"),
            (9, "link"),
            (10, "unlink"),
            (11, "symlink"),
            (12, "readlink"),
            (15, "chmod"),
            (16, "fchmod"),
            (92, "chown"),
            (93, "fchown"),
            (94, "lchown"),
            (60, "umask"),
            // 进程管理
            (220, "clone"),
            (107, "fork"),
            (58, "vfork"),
            (221, "execve"),
            (93, "exit"),
            (260, "wait4"),
            (129, "kill"),
            (172, "getpid"),
            (94, "exit_group"),
            // 信号处理
            (134, "rt_sigaction"),
            (135, "rt_sigprocmask"),
            (136, "rt_sigpending"),
            (132, "sigaltstack"),
            (133, "pause"),
            // 定时器
            (101, "nanosleep"),
            (102, "getitimer"),
            (103, "alarm"),
            (104, "setitimer"),
            // 网络
            (198, "socket"),
            (203, "connect"),
            (202, "accept"),
            (206, "sendto"),
            (207, "recvfrom"),
            (210, "shutdown"),
            (201, "listen"),
            (204, "getsockname"),
            (205, "getpeername"),
            (199, "socketpair"),
            (208, "setsockopt"),
            (209, "getsockopt"),
            // 其他
            (160, "uname"),
            (71, "sendfile"),
            (17, "poll"),
        ];

        // RISC-V64
        let riscv64_syscalls = vec![
            // 文件I/O
            (63, "read"),
            (64, "write"),
            (56, "open"),
            (57, "close"),
            (106, "stat"),
            (80, "fstat"),
            (1039, "lstat"),
            (62, "lseek"),
            (222, "mmap"),
            (226, "mprotect"),
            (215, "munmap"),
            (214, "brk"),
            (25, "fcntl"),
            (32, "flock"),
            (82, "fsync"),
            (83, "fdatasync"),
            (45, "truncate"),
            (46, "ftruncate"),
            (61, "getdents"),
            (17, "getcwd"),
            (49, "chdir"),
            (50, "fchdir"),
            (38, "rename"),
            (34, "mkdir"),
            (40, "rmdir"),
            (8, "creat"),
            (9, "link"),
            (10, "unlink"),
            (11, "symlink"),
            (12, "readlink"),
            (15, "chmod"),
            (16, "fchmod"),
            (92, "chown"),
            (93, "fchown"),
            (94, "lchown"),
            (60, "umask"),
            // 进程管理
            (220, "clone"),
            (107, "fork"),
            (58, "vfork"),
            (221, "execve"),
            (93, "exit"),
            (260, "wait4"),
            (129, "kill"),
            (172, "getpid"),
            (94, "exit_group"),
            // 信号处理
            (134, "rt_sigaction"),
            (135, "rt_sigprocmask"),
            (136, "rt_sigpending"),
            (132, "sigaltstack"),
            (133, "pause"),
            // 定时器
            (101, "nanosleep"),
            (102, "getitimer"),
            (103, "alarm"),
            (104, "setitimer"),
            // 网络
            (198, "socket"),
            (203, "connect"),
            (202, "accept"),
            (206, "sendto"),
            (207, "recvfrom"),
            (210, "shutdown"),
            (201, "listen"),
            (204, "getsockname"),
            (205, "getpeername"),
            (199, "socketpair"),
            (208, "setsockopt"),
            (209, "getsockopt"),
            // 其他
            (160, "uname"),
            (71, "sendfile"),
            (7, "poll"),
        ];

        // 建立映射关系（以 x86_64 为参考）
        for (x86_num, name) in &x86_64_syscalls {
            // ARM64 映射
            if let Some((arm64_num, _)) = arm64_syscalls.iter().find(|(_, n)| n == name) {
                mapper.guest_to_host.insert((GuestArch::Arm64, *arm64_num), *x86_num);
                mapper.host_to_guest.insert((GuestArch::Arm64, *x86_num), *arm64_num);
            }
            
            // RISC-V64 映射
            if let Some((riscv_num, _)) = riscv64_syscalls.iter().find(|(_, n)| n == name) {
                mapper.guest_to_host.insert((GuestArch::Riscv64, *riscv_num), *x86_num);
                mapper.host_to_guest.insert((GuestArch::Riscv64, *x86_num), *riscv_num);
            }
        }

        debug!("Initialized syscall mapper with {} mappings", mapper.guest_to_host.len());
    }

    /// 将 guest 架构的系统调用号转换为 host 架构的系统调用号
    pub fn map_guest_to_host(&self, guest_arch: GuestArch, guest_syscall: u64) -> Option<u64> {
        self.guest_to_host.get(&(guest_arch, guest_syscall)).copied()
    }

    /// 将 host 架构的系统调用号转换为 guest 架构的系统调用号
    pub fn map_host_to_guest(&self, guest_arch: GuestArch, host_syscall: u64) -> Option<u64> {
        self.host_to_guest.get(&(guest_arch, host_syscall)).copied()
    }

    /// 直接映射（如果架构相同，直接返回）
    pub fn map(&self, guest_arch: GuestArch, host_arch: GuestArch, syscall: u64) -> u64 {
        if guest_arch == host_arch {
            return syscall;
        }
        
        self.map_guest_to_host(guest_arch, syscall).unwrap_or(syscall)
    }
}

impl Default for SyscallNumberMapper {
    fn default() -> Self {
        Self::new()
    }
}

/// 系统调用参数转换器
/// 
/// 不同架构的系统调用参数传递方式不同：
/// - x86_64: RDI, RSI, RDX, R10, R8, R9
/// - ARM64: X0-X5
/// - RISC-V64: A0-A5
pub struct SyscallArgConverter;

impl SyscallArgConverter {
    /// 从 guest 寄存器提取系统调用参数
    pub fn extract_args(guest_arch: GuestArch, regs: &GuestRegs) -> [u64; 6] {
        match guest_arch {
            GuestArch::X86_64 => {
                [
                    regs.gpr[7],  // RDI
                    regs.gpr[6],  // RSI
                    regs.gpr[2],  // RDX
                    regs.gpr[10], // R10
                    regs.gpr[8],  // R8
                    regs.gpr[9],  // R9
                ]
            }
            GuestArch::Arm64 => {
                [
                    regs.gpr[0], // X0
                    regs.gpr[1], // X1
                    regs.gpr[2], // X2
                    regs.gpr[3], // X3
                    regs.gpr[4], // X4
                    regs.gpr[5], // X5
                ]
            }
            GuestArch::Riscv64 => {
                [
                    regs.gpr[10], // A0
                    regs.gpr[11], // A1
                    regs.gpr[12], // A2
                    regs.gpr[13], // A3
                    regs.gpr[14], // A4
                    regs.gpr[15], // A5
                ]
            }
        }
    }

    /// 从 guest 寄存器提取系统调用号
    pub fn extract_syscall_number(guest_arch: GuestArch, regs: &GuestRegs) -> u64 {
        match guest_arch {
            GuestArch::X86_64 => regs.gpr[0], // RAX
            GuestArch::Arm64 => regs.gpr[8],  // X8
            GuestArch::Riscv64 => regs.gpr[17], // A7
        }
    }

    /// 将返回值写入 guest 寄存器
    pub fn set_return_value(guest_arch: GuestArch, regs: &mut GuestRegs, ret: i64) {
        match guest_arch {
            GuestArch::X86_64 => {
                regs.gpr[0] = ret as u64; // RAX
            }
            GuestArch::Arm64 => {
                regs.gpr[0] = ret as u64; // X0
            }
            GuestArch::Riscv64 => {
                regs.gpr[10] = ret as u64; // A0
            }
        }
    }
}

/// 系统调用兼容层
pub struct SyscallCompatibilityLayer {
    /// 系统调用号映射器
    mapper: SyscallNumberMapper,
    /// Guest 架构
    guest_arch: GuestArch,
    /// Host 架构（用于执行）
    host_arch: GuestArch,
}

impl SyscallCompatibilityLayer {
    /// 创建新的系统调用兼容层
    pub fn new(guest_arch: GuestArch, host_arch: GuestArch) -> Self {
        Self {
            mapper: SyscallNumberMapper::new(),
            guest_arch,
            host_arch,
        }
    }

    /// 处理系统调用（转换参数和系统调用号）
    pub fn handle_syscall(
        &self,
        regs: &GuestRegs,
    ) -> Result<(u64, [u64; 6]), VmError> {
        // 1. 提取 guest 架构的系统调用号和参数
        let guest_syscall = SyscallArgConverter::extract_syscall_number(self.guest_arch, regs);
        let guest_args = SyscallArgConverter::extract_args(self.guest_arch, regs);

        trace!(
            "Guest syscall: {} (arch: {:?}) with args: {:?}",
            guest_syscall,
            self.guest_arch,
            guest_args
        );

        // 2. 映射系统调用号到 host 架构
        let host_syscall = if self.guest_arch == self.host_arch {
            guest_syscall
        } else {
            self.mapper
                .map_guest_to_host(self.guest_arch, guest_syscall)
                .ok_or_else(|| {
                    VmError::Core(vm_core::CoreError::NotSupported {
                        feature: format!(
                            "Syscall {} not mapped for {:?}",
                            guest_syscall, self.guest_arch
                        ),
                        module: "SyscallCompatibilityLayer".to_string(),
                    })
                })?
        };

        debug!(
            "Mapped syscall {} -> {} (guest: {:?}, host: {:?})",
            guest_syscall, host_syscall, self.guest_arch, self.host_arch
        );

        Ok((host_syscall, guest_args))
    }

    /// 处理系统调用返回值
    pub fn handle_return(&self, regs: &mut GuestRegs, ret: i64) {
        SyscallArgConverter::set_return_value(self.guest_arch, regs, ret);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_mapping() {
        let mapper = SyscallNumberMapper::new();

        // ARM64 read (63) -> x86_64 read (0)
        let mapped = mapper.map_guest_to_host(GuestArch::Arm64, 63);
        assert_eq!(mapped, Some(0));

        // RISC-V64 write (64) -> x86_64 write (1)
        let mapped = mapper.map_guest_to_host(GuestArch::Riscv64, 64);
        assert_eq!(mapped, Some(1));
    }

    #[test]
    fn test_arg_extraction() {
        let mut regs = GuestRegs::default();
        
        // x86_64: RDI=1, RSI=2, RDX=3
        regs.gpr[7] = 1;  // RDI
        regs.gpr[6] = 2;  // RSI
        regs.gpr[2] = 3;  // RDX
        
        let args = SyscallArgConverter::extract_args(GuestArch::X86_64, &regs);
        assert_eq!(args[0], 1);
        assert_eq!(args[1], 2);
        assert_eq!(args[2], 3);

        // ARM64: X0=10, X1=20
        regs.gpr[0] = 10;
        regs.gpr[1] = 20;
        let args = SyscallArgConverter::extract_args(GuestArch::Arm64, &regs);
        assert_eq!(args[0], 10);
        assert_eq!(args[1], 20);
    }

    #[test]
    fn test_syscall_compatibility_layer() {
        let mut regs = GuestRegs::default();
        regs.gpr[8] = 63; // ARM64 read syscall
        regs.gpr[0] = 0;  // X0 = fd
        regs.gpr[1] = 0x1000; // X1 = buf
        regs.gpr[2] = 100; // X2 = count

        let layer = SyscallCompatibilityLayer::new(GuestArch::Arm64, GuestArch::X86_64);
        let (host_syscall, args) = layer.handle_syscall(&regs).unwrap();

        assert_eq!(host_syscall, 0); // x86_64 read
        assert_eq!(args[0], 0);      // fd
        assert_eq!(args[1], 0x1000); // buf
        assert_eq!(args[2], 100);    // count
    }
}

