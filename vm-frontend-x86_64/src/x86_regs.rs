//! x86寄存器常量定义
//!
//! 定义x86架构中常用的寄存器编号

// use vm_core::GuestAddr;

// 通用寄存器常量
pub const RAX: u32 = 0;
pub const RCX: u32 = 1;
pub const RDX: u32 = 2;
pub const RBX: u32 = 3;
pub const RSP: u32 = 4;
pub const RBP: u32 = 5;
pub const RSI: u32 = 6;
pub const RDI: u32 = 7;
pub const R8: u32 = 8;
pub const R9: u32 = 9;
pub const R10: u32 = 10;
pub const R11: u32 = 11;
pub const R12: u32 = 12;
pub const R13: u32 = 13;
pub const R14: u32 = 14;
pub const R15: u32 = 15;

// XMM寄存器常量 (从0开始)
pub const XMM0: u32 = 0;

// VEX常量
pub const FEX_W0: u32 = 0;
pub const FEX_W1: u32 = 1;
pub const XMM1: u32 = 1;
pub const XMM2: u32 = 2;
pub const XMM3: u32 = 3;
pub const XMM4: u32 = 4;
pub const XMM5: u32 = 5;
pub const XMM6: u32 = 6;
pub const XMM7: u32 = 7;

// RFLAGS寄存器
pub const RFLAGS: u32 = 16;
