//! Guest 寄存器定义

/// Guest 通用寄存器
#[derive(Debug, Clone, Default)]
pub struct GuestRegs {
    /// 程序计数器 (PC/RIP/EIP)
    pub pc: u64,
    /// 栈指针 (SP/RSP/ESP)
    pub sp: u64,
    /// 帧指针 (FP/RBP/EBP)
    pub fp: u64,
    /// 通用寄存器
    /// - x86_64: RAX, RCX, RDX, RBX, RSP, RBP, RSI, RDI, R8-R15
    /// - ARM64: X0-X31
    /// - RISC-V: X0-X31
    pub gpr: [u64; 32],
}

impl GuestRegs {
    /// 创建新的寄存器集
    pub fn new() -> Self {
        Self::default()
    }

    /// 重置所有寄存器
    pub fn reset(&mut self) {
        self.pc = 0;
        self.sp = 0;
        self.fp = 0;
        self.gpr = [0; 32];
    }
}
