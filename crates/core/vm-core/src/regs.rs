//! Guest 寄存器定义

use serde::{Deserialize, Serialize};

/// Guest 通用寄存器
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_regs() {
        let regs = GuestRegs::new();
        assert_eq!(regs.pc, 0);
        assert_eq!(regs.sp, 0);
        assert_eq!(regs.fp, 0);
        assert_eq!(regs.gpr, [0; 32]);
    }

    #[test]
    fn test_default_regs() {
        let regs = GuestRegs::default();
        assert_eq!(regs.pc, 0);
        assert_eq!(regs.gpr.len(), 32);
    }

    #[test]
    fn test_reset() {
        let mut regs = GuestRegs {
            pc: 0x1000,
            sp: 0x2000,
            fp: 0x3000,
            gpr: [1; 32],
        };

        regs.reset();

        assert_eq!(regs.pc, 0);
        assert_eq!(regs.sp, 0);
        assert_eq!(regs.fp, 0);
        assert_eq!(regs.gpr, [0; 32]);
    }

    #[test]
    fn test_custom_regs() {
        let mut regs = GuestRegs::new();
        regs.pc = 0x1000;
        regs.sp = 0x2000;
        regs.gpr[0] = 42;
        regs.gpr[31] = 99;

        assert_eq!(regs.pc, 0x1000);
        assert_eq!(regs.sp, 0x2000);
        assert_eq!(regs.gpr[0], 42);
        assert_eq!(regs.gpr[31], 99);
    }

    #[test]
    fn test_serialize() {
        let regs = GuestRegs {
            pc: 0x1000,
            sp: 0x2000,
            fp: 0x3000,
            gpr: [
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27, 28, 29, 30, 31, 32,
            ],
        };

        let serialized = serde_json::to_string(&regs).unwrap();
        assert!(serialized.contains("pc"));
        assert!(serialized.contains("4096")); // 0x1000
    }

    #[test]
    fn test_deserialize() {
        let json = r#"{"pc":4096,"sp":8192,"fp":12288,"gpr":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32]}"#;

        let regs: GuestRegs = serde_json::from_str(json).unwrap();
        assert_eq!(regs.pc, 4096);
        assert_eq!(regs.sp, 8192);
        assert_eq!(regs.fp, 12288);
        assert_eq!(regs.gpr[0], 1);
        assert_eq!(regs.gpr[31], 32);
    }
}
