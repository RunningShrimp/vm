//! 系统调用处理
//!
//! 实现对 Linux、Windows 和 macOS 系统调用的模拟

use crate::{GuestRegs, GuestAddr};

/// 系统调用号
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyscallNumber {
    // Linux x86-64
    LinuxRead = 0,
    LinuxWrite = 1,
    LinuxOpen = 2,
    LinuxClose = 3,
    LinuxStat = 4,
    LinuxFstat = 5,
    LinuxLstat = 6,
    LinuxPoll = 7,
    LinuxLseek = 8,
    LinuxMmap = 9,
    LinuxMprotect = 10,
    LinuxMunmap = 11,
    LinuxBrk = 12,
    LinuxExit = 60,
    LinuxExitGroup = 231,
    
    // Linux RISC-V
    RiscvRead = 1063,
    RiscvWrite = 1064,
    RiscvExit = 1093,
    RiscvExitGroup = 1094,
    RiscvBrk = 1214,
    
    // Linux AArch64
    Aarch64Read = 2063,
    Aarch64Write = 2064,
    Aarch64Exit = 2093,
    
    Unknown = 0xFFFF,
}

/// 系统调用结果
#[derive(Debug, Clone)]
pub enum SyscallResult {
    Success(i64),
    Error(i32),
    Block, // 系统调用需要阻塞
}

/// 系统调用处理器
pub struct SyscallHandler {
    // 文件描述符表
    fd_table: Vec<Option<FileDescriptor>>,
    // 内存分配信息
    brk_addr: GuestAddr,
}

/// 文件描述符
#[derive(Debug, Clone)]
pub struct FileDescriptor {
    pub fd: i32,
    pub path: String,
    pub flags: i32,
}

impl SyscallHandler {
    pub fn new() -> Self {
        let mut handler = Self {
            fd_table: Vec::new(),
            brk_addr: 0,
        };
        
        // 初始化标准文件描述符
        handler.fd_table.push(Some(FileDescriptor {
            fd: 0,
            path: "/dev/stdin".to_string(),
            flags: 0,
        }));
        handler.fd_table.push(Some(FileDescriptor {
            fd: 1,
            path: "/dev/stdout".to_string(),
            flags: 1,
        }));
        handler.fd_table.push(Some(FileDescriptor {
            fd: 2,
            path: "/dev/stderr".to_string(),
            flags: 1,
        }));
        
        handler
    }

    /// 处理系统调用
    pub fn handle_syscall(&mut self, regs: &mut GuestRegs, arch: SyscallArch) -> SyscallResult {
        let (syscall_num, args) = match arch {
            SyscallArch::X86_64 => {
                let num = regs.gpr[0] as u64; // RAX
                let args = [
                    regs.gpr[7] as u64,  // RDI
                    regs.gpr[6] as u64,  // RSI
                    regs.gpr[2] as u64,  // RDX
                    regs.gpr[10] as u64, // R10
                    regs.gpr[8] as u64,  // R8
                    regs.gpr[9] as u64,  // R9
                ];
                (num, args)
            }
            SyscallArch::Riscv64 => {
                let num = regs.gpr[17] as u64; // a7
                let args = [
                    regs.gpr[10] as u64, // a0
                    regs.gpr[11] as u64, // a1
                    regs.gpr[12] as u64, // a2
                    regs.gpr[13] as u64, // a3
                    regs.gpr[14] as u64, // a4
                    regs.gpr[15] as u64, // a5
                ];
                (num, args)
            }
            SyscallArch::Aarch64 => {
                let num = regs.gpr[8] as u64; // x8
                let args = [
                    regs.gpr[0] as u64, // x0
                    regs.gpr[1] as u64, // x1
                    regs.gpr[2] as u64, // x2
                    regs.gpr[3] as u64, // x3
                    regs.gpr[4] as u64, // x4
                    regs.gpr[5] as u64, // x5
                ];
                (num, args)
            }
        };

        self.dispatch_syscall(syscall_num, &args, arch)
    }

    /// 分发系统调用
    fn dispatch_syscall(&mut self, num: u64, args: &[u64; 6], arch: SyscallArch) -> SyscallResult {
        match arch {
            SyscallArch::X86_64 => self.handle_linux_x64_syscall(num, args),
            SyscallArch::Riscv64 => self.handle_linux_riscv_syscall(num, args),
            SyscallArch::Aarch64 => self.handle_linux_aarch64_syscall(num, args),
        }
    }

    /// 处理 Linux x86-64 系统调用
    fn handle_linux_x64_syscall(&mut self, num: u64, args: &[u64; 6]) -> SyscallResult {
        match num {
            0 => self.sys_read(args[0] as i32, args[1], args[2] as usize),
            1 => self.sys_write(args[0] as i32, args[1], args[2] as usize),
            2 => self.sys_open(args[0], args[1] as i32, args[2] as u32),
            3 => self.sys_close(args[0] as i32),
            12 => self.sys_brk(args[0]),
            60 => self.sys_exit(args[0] as i32),
            231 => self.sys_exit_group(args[0] as i32),
            _ => {
                println!("Unhandled syscall: {}", num);
                SyscallResult::Error(-38) // ENOSYS
            }
        }
    }

    /// 处理 Linux RISC-V 系统调用
    fn handle_linux_riscv_syscall(&mut self, num: u64, args: &[u64; 6]) -> SyscallResult {
        match num {
            63 => self.sys_read(args[0] as i32, args[1], args[2] as usize),
            64 => self.sys_write(args[0] as i32, args[1], args[2] as usize),
            93 => self.sys_exit(args[0] as i32),
            94 => self.sys_exit_group(args[0] as i32),
            214 => self.sys_brk(args[0]),
            _ => {
                println!("Unhandled RISC-V syscall: {}", num);
                SyscallResult::Error(-38)
            }
        }
    }

    /// 处理 Linux AArch64 系统调用
    fn handle_linux_aarch64_syscall(&mut self, num: u64, args: &[u64; 6]) -> SyscallResult {
        match num {
            63 => self.sys_read(args[0] as i32, args[1], args[2] as usize),
            64 => self.sys_write(args[0] as i32, args[1], args[2] as usize),
            93 => self.sys_exit(args[0] as i32),
            _ => {
                println!("Unhandled AArch64 syscall: {}", num);
                SyscallResult::Error(-38)
            }
        }
    }

    // 系统调用实现

    fn sys_read(&mut self, fd: i32, buf: u64, count: usize) -> SyscallResult {
        // 简化实现：只处理 stdin
        if fd == 0 {
            // 从标准输入读取（在实际实现中应该从 MMU 读取）
            println!("sys_read: fd={}, buf={:#x}, count={}", fd, buf, count);
            SyscallResult::Success(0) // 返回读取的字节数
        } else {
            SyscallResult::Error(-9) // EBADF
        }
    }

    fn sys_write(&mut self, fd: i32, buf: u64, count: usize) -> SyscallResult {
        // 简化实现：只处理 stdout 和 stderr
        if fd == 1 || fd == 2 {
            println!("sys_write: fd={}, buf={:#x}, count={}", fd, buf, count);
            // 在实际实现中应该从 MMU 读取数据并写入
            SyscallResult::Success(count as i64)
        } else {
            SyscallResult::Error(-9) // EBADF
        }
    }

    fn sys_open(&mut self, pathname: u64, flags: i32, mode: u32) -> SyscallResult {
        println!("sys_open: pathname={:#x}, flags={}, mode={}", pathname, flags, mode);
        // 在实际实现中应该从 MMU 读取路径字符串
        let fd = self.fd_table.len() as i32;
        self.fd_table.push(Some(FileDescriptor {
            fd,
            path: format!("file_{}", fd),
            flags,
        }));
        SyscallResult::Success(fd as i64)
    }

    fn sys_close(&mut self, fd: i32) -> SyscallResult {
        if fd >= 0 && (fd as usize) < self.fd_table.len() {
            self.fd_table[fd as usize] = None;
            SyscallResult::Success(0)
        } else {
            SyscallResult::Error(-9) // EBADF
        }
    }

    fn sys_brk(&mut self, addr: u64) -> SyscallResult {
        if addr == 0 {
            // 返回当前 brk 地址
            SyscallResult::Success(self.brk_addr as i64)
        } else {
            // 设置新的 brk 地址
            self.brk_addr = addr;
            SyscallResult::Success(addr as i64)
        }
    }

    fn sys_exit(&mut self, status: i32) -> SyscallResult {
        println!("sys_exit: status={}", status);
        SyscallResult::Success(0)
    }

    fn sys_exit_group(&mut self, status: i32) -> SyscallResult {
        println!("sys_exit_group: status={}", status);
        SyscallResult::Success(0)
    }
}

/// 系统调用架构
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyscallArch {
    X86_64,
    Riscv64,
    Aarch64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_handler() {
        let mut handler = SyscallHandler::new();
        let mut regs = GuestRegs::default();
        
        // 模拟 write 系统调用
        regs.gpr[0] = 1; // syscall number (write)
        regs.gpr[7] = 1; // fd (stdout)
        regs.gpr[6] = 0x1000; // buf
        regs.gpr[2] = 10; // count
        
        let result = handler.handle_syscall(&mut regs, SyscallArch::X86_64);
        match result {
            SyscallResult::Success(n) => assert_eq!(n, 10),
            _ => panic!("Expected success"),
        }
    }
}
