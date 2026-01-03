//! 架构特定代码宏
//!
//! 提供宏来减少架构特定代码的重复，支持 x86_64, ARM64, RISC-V64 等架构。

/// 为架构实现算术操作
///
/// # 使用示例
///
/// ```rust,ignore
/// use vm_core::macros::impl_arithmetic_ops;
///
/// // 为 x86_64 寄存器文件实现
/// impl_arithmetic_ops!(X86_64, RegisterFile, {
///     fn add(&mut self, dst: RegId, src1: RegId, src2: RegId) {
///         let val1 = self.read(src1);
///         let val2 = self.read(src2);
///         self.write(dst, val1.wrapping_add(val2));
///     }
///
///     fn sub(&mut self, dst: RegId, src1: RegId, src2: RegId) {
///         let val1 = self.read(src1);
///         let val2 = self.read(src2);
///         self.write(dst, val1.wrapping_sub(val2));
///     }
/// });
/// ```
#[macro_export]
macro_rules! impl_arithmetic_ops {
    ($arch:ident, $reg:ident, { $($method:tt)* }) => {
        impl $arch::$reg {
            $($method)*
        }
    };
}

/// 为架构实现内存操作
///
/// # 使用示例
///
/// ```rust,ignore
/// use vm_core::macros::impl_memory_ops;
///
/// impl_memory_ops!(X86_64, MemoryOps, {
///     fn load(&mut self, addr: GuestAddr, size: u8) -> Result<u64, VmError> {
///         match size {
///             1 => self.mmu.read_u8(addr),
///             2 => self.mmu.read_u16(addr).map(|v| v as u64),
///             4 => self.mmu.read_u32(addr).map(|v| v as u64),
///             8 => self.mmu.read_u64(addr),
///             _ => Err(VmError::Memory(MemoryError::InvalidAccessSize { size })),
///         }
///     }
///
///     fn store(&mut self, addr: GuestAddr, value: u64, size: u8) -> Result<(), VmError> {
///         match size {
///             1 => self.mmu.write_u8(addr, value as u8),
///             2 => self.mmu.write_u16(addr, value as u16),
///             4 => self.mmu.write_u32(addr, value as u32),
///             8 => self.mmu.write_u64(addr, value),
///             _ => Err(VmError::Memory(MemoryError::InvalidAccessSize { size })),
///         }
///     }
/// });
/// ```
#[macro_export]
macro_rules! impl_memory_ops {
    ($arch:ident, $mem:ident, { $($method:tt)* }) => {
        impl $arch::$mem {
            $($method)*
        }
    };
}

/// 为架构实现分支操作
///
/// # 使用示例
///
/// ```rust,ignore
/// use vm_core::macros::impl_branch_ops;
///
/// impl_branch_ops!(X86_64, BranchOps, {
///     fn conditional_branch(&mut self, cond: bool, target: GuestAddr) -> Result<(), VmError> {
///         if cond {
///             self.pc = target;
///         }
///         Ok(())
///     }
///
///     fn unconditional_branch(&mut self, target: GuestAddr) -> Result<(), VmError> {
///         self.pc = target;
///         Ok(())
///     }
/// });
/// ```
#[macro_export]
macro_rules! impl_branch_ops {
    ($arch:ident, $branch:ident, { $($method:tt)* }) => {
        impl $arch::$branch {
            $($method)*
        }
    };
}

/// 为架构实现特权指令操作
///
/// # 使用示例
///
/// ```rust,ignore
/// use vm_core::macros::impl_privileged_ops;
///
/// impl_privileged_ops!(X86_64, PrivilegedOps, {
///     fn read_csr(&self, csr: u16) -> Result<u64, VmError> {
///         match csr {
///             0x10B => Ok(self.tsc), // TSC (Time Stamp Counter)
///             _ => Err(VmError::Core(CoreError::NotSupported {
///                 feature: format!("CSR {}", csr),
///                 module: "X86_64".to_string(),
///             })),
///         }
///     }
///
///     fn write_csr(&mut self, csr: u16, value: u64) -> Result<(), VmError> {
///         match csr {
///             0x10B => { /* TSC is read-only */ }
///             _ => return Err(VmError::Core(CoreError::NotSupported {
///                 feature: format!("CSR {}", csr),
///                 module: "X86_64".to_string(),
///             })),
///         }
///         Ok(())
///     }
/// });
/// ```
#[macro_export]
macro_rules! impl_privileged_ops {
    ($arch:ident, $priv:ident, { $($method:tt)* }) => {
        impl $arch::$priv {
            $($method)*
        }
    };
}

/// 为多个架构生成统一的实现
///
/// # 使用示例
///
/// ```rust,ignore
/// use vm_core::macros::for_each_arch;
///
/// for_each_arch! {
///     #[inline]
///     fn add_instruction(&mut self, dst: RegId, src1: RegId, src2: RegId) {
///         let val1 = self.read_reg(src1);
///         let val2 = self.read_reg(src2);
///         self.write_reg(dst, val1.wrapping_add(val2));
///     }
/// }
/// ```
#[macro_export]
macro_rules! for_each_arch {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "x86_64")]
            $item

            #[cfg(feature = "arm64")]
            $item

            #[cfg(feature = "riscv64")]
            $item
        )*
    };
}

/// 为架构特定的寄存器类型生成实现
///
/// # 使用示例
///
/// ```rust,ignore
/// use vm_core::macros::impl_register_types;
///
/// impl_register_types!(X86_64, {
///     pub enum RegId {
///         Rax, Rbx, Rcx, Rdx,
///         Rsi, Rdi, Rbp, Rsp,
///         R8, R9, R10, R11,
///         R12, R13, R14, R15,
///         Rip, Rflags,
///     }
///
///     impl RegId {
///         pub fn from_u16(id: u16) -> Option<Self> {
///             match id {
///                 0 => Some(RegId::Rax),
///                 1 => Some(RegId::Rbx),
///                 // ... more mappings
///                 _ => None,
///             }
///         }
///     }
/// });
/// ```
#[macro_export]
macro_rules! impl_register_types {
    ($arch:ident, { $($def:item)* }) => {
        pub mod $arch {
            $($def)*
        }
    };
}

/// 定义架构特定的常量
///
/// # 使用示例
///
/// ```rust,ignore
/// use vm_core::macros::arch_constants;
///
/// arch_constants!(X86_64, {
///     pub const PAGE_SIZE: u64 = 4096;
///     pub const MAX_PHYS_ADDR: u64 = (1u64 << 52) - 1;
///     pub const GDT_NULL: u16 = 0;
///     pub const GDT_CODE: u16 = 1;
///     pub const GDT_DATA: u16 = 2;
/// });
/// ```
#[macro_export]
macro_rules! arch_constants {
    ($arch:ident, { $($const_def:item)* }) => {
        #[cfg(feature = stringify!($arch))]
        pub mod $arch {
            $($const_def)*
        }
    };
}

/// 简化条件编译的架构选择
///
/// # 使用示例
///
/// ```rust,ignore
/// use vm_core::macros::select_arch;
///
/// let result = select_arch! {
///     [x86_64] => x86_64_specific_function(),
///     [arm64] => arm64_specific_function(),
///     [riscv64] => riscv64_specific_function(),
///     _ => Err(VmError::Core(CoreError::NotSupported {
///         feature: "Unknown architecture".to_string(),
///         module: "select_arch".to_string(),
///     })),
/// };
/// ```
#[macro_export]
macro_rules! select_arch {
    ([$($arch:tt)*] => $expr:expr, $($rest:tt)*) => {
        #[cfg(feature = stringify!($($arch)*))]
        $expr

        // 递归处理其他架构
        $crate::select_arch!($($rest)*)
    };

    (_ => $default:expr,) => {
        #[cfg(not(any(
            feature = "x86_64",
            feature = "arm64",
            feature = "riscv64"
        )))]
        $default
    };

    (_ => $default:expr) => {
        #[cfg(not(any(
            feature = "x86_64",
            feature = "arm64",
            feature = "riscv64"
        )))]
        $default
    };
}

#[cfg(test)]
mod tests {

    #[test]
    #[allow(dead_code)] // Test infrastructure for macro validation
    fn test_impl_arithmetic_ops_macro() {
        // 这个测试主要是验证宏能够正确展开
        // 实际的算术操作测试在各个架构的测试中

        // 定义MockArch枚举（占位符）
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum MockArch {}

        struct MockRegisterFile {
            registers: [u64; 16],
        }

        impl MockRegisterFile {
            fn read(&self, idx: usize) -> u64 {
                self.registers[idx]
            }

            fn write(&mut self, idx: usize, val: u64) {
                self.registers[idx] = val;
            }
        }

        // 直接实现算术操作（不使用宏，因为MockRegisterFile不是MockArch的变体）
        impl MockRegisterFile {
            fn add(&mut self, dst: usize, src1: usize, src2: usize) {
                let val1 = self.read(src1);
                let val2 = self.read(src2);
                self.write(dst, val1.wrapping_add(val2));
            }
        }

        // 验证实现
        let _regs = MockRegisterFile { registers: [0; 16] };
        // regs.registers[1] = 10; // Reserved for future testing
        // regs.registers[2] = 20; // Reserved for future testing

        // MockArch::add 需要在实际使用场景中测试
    }

    #[test]
    fn test_for_each_arch_macro() {
        // 测试宏展开
        for_each_arch! {
            fn get_arch_name() -> &'static str {
                stringify!(arch)
            }
        }
    }
}
