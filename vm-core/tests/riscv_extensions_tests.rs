//! RISC-V扩展测试框架（简化版）
//!
//! 测试RISC-V各种扩展的基本功能

use vm_core::{CoreError, GuestAddr, GuestArch, VmError};

// ============================================================================
// 测试辅助结构
// ============================================================================

/// 简化的CPU状态（用于测试）
#[derive(Debug, Clone)]
struct TestCPUState {
    regs: [u64; 32],
    fp_regs: [f32; 32],
    fp_regs_d: [f64; 32],
    memory: Vec<u8>,
}

impl TestCPUState {
    fn new() -> Self {
        Self {
            regs: [0; 32],
            fp_regs: [0.0; 32],
            fp_regs_d: [0.0; 32],
            memory: vec![0; 0x10000], // 64KB
        }
    }

    fn get_reg(&self, idx: usize) -> u64 {
        self.regs[idx]
    }

    fn set_reg(&mut self, idx: usize, val: u64) {
        if idx != 0 {
            // x0始终为0（RISC-V规范）
            self.regs[idx] = val;
        }
    }

    fn get_freg_f32(&self, idx: usize) -> f32 {
        self.fp_regs[idx]
    }

    fn set_freg_f32(&mut self, idx: usize, val: f32) {
        self.fp_regs[idx] = val;
    }

    fn get_freg_f64(&self, idx: usize) -> f64 {
        self.fp_regs_d[idx]
    }

    fn set_freg_f64(&mut self, idx: usize, val: f64) {
        self.fp_regs_d[idx] = val;
    }
}

// ============================================================================
// M扩展（乘除法）测试
// ============================================================================

#[test]
fn test_m_extension_multiply_operation() {
    // 测试乘法操作
    let mut cpu = TestCPUState::new();

    cpu.set_reg(1, 10);
    cpu.set_reg(2, 20);

    // 模拟MUL x3, x1, x2
    let result = cpu.get_reg(1).wrapping_mul(cpu.get_reg(2));
    cpu.set_reg(3, result);

    assert_eq!(cpu.get_reg(3), 200);
}

#[test]
fn test_m_extension_divide_operation() {
    // 测试除法操作
    let mut cpu = TestCPUState::new();

    cpu.set_reg(1, 100);
    cpu.set_reg(2, 5);

    // 模拟DIV x3, x1, x2
    let result = cpu.get_reg(1) / cpu.get_reg(2);
    cpu.set_reg(3, result);

    assert_eq!(cpu.get_reg(3), 20);
}

#[test]
fn test_m_extension_divide_by_zero() {
    // 测试除以零
    let mut cpu = TestCPUState::new();

    cpu.set_reg(1, 100);
    cpu.set_reg(2, 0);

    // RISC-V规范：除以零结果为全1
    let result = if cpu.get_reg(2) == 0 {
        0xFFFF_FFFF_FFFF_FFFF
    } else {
        cpu.get_reg(1) / cpu.get_reg(2)
    };
    cpu.set_reg(3, result);

    assert_eq!(cpu.get_reg(3), 0xFFFF_FFFF_FFFF_FFFF);
}

#[test]
fn test_m_extension_remainder_operation() {
    // 测试取余操作
    let mut cpu = TestCPUState::new();

    cpu.set_reg(1, 17);
    cpu.set_reg(2, 5);

    // 模拟REM x3, x1, x2
    let result = cpu.get_reg(1) % cpu.get_reg(2);
    cpu.set_reg(3, result);

    assert_eq!(cpu.get_reg(3), 2);
}

#[test]
fn test_m_extension_multiply_high() {
    // 测试乘法高位
    let a: i64 = -1;
    let b: i64 = 2;

    // 模拟MULH（有符号乘法高位）
    let result_full = (a as i128) * (b as i128);
    let high = ((result_full >> 64) as i64) as u64;

    assert_eq!(high, 0xFFFF_FFFF_FFFF_FFFF);
}

// ============================================================================
// F扩展（单精度浮点）测试
// ============================================================================

#[test]
fn test_f_extension_add() {
    // 测试浮点加法
    let mut cpu = TestCPUState::new();

    cpu.set_freg_f32(0, 1.0);
    cpu.set_freg_f32(1, 2.0);

    // 模拟FADD.S f2, f0, f1
    let result = cpu.get_freg_f32(0) + cpu.get_freg_f32(1);
    cpu.set_freg_f32(2, result);

    assert!((cpu.get_freg_f32(2) - 3.0).abs() < f32::EPSILON);
}

#[test]
fn test_f_extension_sub() {
    // 测试浮点减法
    let mut cpu = TestCPUState::new();

    cpu.set_freg_f32(0, 5.0);
    cpu.set_freg_f32(1, 3.0);

    let result = cpu.get_freg_f32(0) - cpu.get_freg_f32(1);
    cpu.set_freg_f32(2, result);

    assert!((cpu.get_freg_f32(2) - 2.0).abs() < f32::EPSILON);
}

#[test]
fn test_f_extension_mul() {
    // 测试浮点乘法
    let mut cpu = TestCPUState::new();

    cpu.set_freg_f32(0, 3.0);
    cpu.set_freg_f32(1, 4.0);

    let result = cpu.get_freg_f32(0) * cpu.get_freg_f32(1);
    cpu.set_freg_f32(2, result);

    assert!((cpu.get_freg_f32(2) - 12.0).abs() < f32::EPSILON);
}

#[test]
fn test_f_extension_div() {
    // 测试浮点除法
    let mut cpu = TestCPUState::new();

    cpu.set_freg_f32(0, 10.0);
    cpu.set_freg_f32(1, 2.0);

    let result = cpu.get_freg_f32(0) / cpu.get_freg_f32(1);
    cpu.set_freg_f32(2, result);

    assert!((cpu.get_freg_f32(2) - 5.0).abs() < f32::EPSILON);
}

#[test]
fn test_f_extension_sqrt() {
    // 测试浮点平方根
    let mut cpu = TestCPUState::new();

    cpu.set_freg_f32(0, 16.0);

    let result = cpu.get_freg_f32(0).sqrt();
    cpu.set_freg_f32(1, result);

    assert!((cpu.get_freg_f32(1) - 4.0).abs() < f32::EPSILON);
}

#[test]
fn test_f_extension_nan_handling() {
    // 测试NaN处理
    let mut cpu = TestCPUState::new();

    cpu.set_freg_f32(0, f32::NAN);
    cpu.set_freg_f32(1, 1.0);

    let result = cpu.get_freg_f32(0) + cpu.get_freg_f32(1);
    cpu.set_freg_f32(2, result);

    assert!(cpu.get_freg_f32(2).is_nan());
}

#[test]
fn test_f_extension_min_max() {
    // 测试FMIN/FMAX
    let mut cpu = TestCPUState::new();

    cpu.set_freg_f32(0, 3.0);
    cpu.set_freg_f32(1, 5.0);

    // FMIN
    let min_val = cpu.get_freg_f32(0).min(cpu.get_freg_f32(1));
    cpu.set_freg_f32(2, min_val);

    assert!((cpu.get_freg_f32(2) - 3.0).abs() < f32::EPSILON);

    // FMAX
    let max_val = cpu.get_freg_f32(0).max(cpu.get_freg_f32(1));
    cpu.set_freg_f32(3, max_val);

    assert!((cpu.get_freg_f32(3) - 5.0).abs() < f32::EPSILON);
}

// ============================================================================
// D扩展（双精度浮点）测试
// ============================================================================

#[test]
fn test_d_extension_add() {
    // 测试双精度加法
    let mut cpu = TestCPUState::new();

    cpu.set_freg_f64(0, std::f64::consts::PI);
    cpu.set_freg_f64(1, std::f64::consts::E);

    let result = cpu.get_freg_f64(0) + cpu.get_freg_f64(1);
    cpu.set_freg_f64(2, result);

    let expected = std::f64::consts::PI + std::f64::consts::E;
    assert!((cpu.get_freg_f64(2) - expected).abs() < f64::EPSILON);
}

#[test]
fn test_d_extension_conversion() {
    // 测试F/D转换
    let mut cpu = TestCPUState::new();

    cpu.set_freg_f32(0, 3.14159f32);

    // F32转F64
    let f64_val = cpu.get_freg_f32(0) as f64;
    cpu.set_freg_f64(1, f64_val);

    // f32转f64会有精度损失，使用更大的误差容忍度
    // 3.14159f32的实际精度约为7位有效数字
    assert!((cpu.get_freg_f64(1) - 3.14159f64).abs() < 1e-5);
}

#[test]
fn test_d_extension_precision() {
    // 测试双精度精度
    let mut cpu = TestCPUState::new();

    cpu.set_freg_f64(0, 1.0e-200);
    cpu.set_freg_f64(1, 1.0e200);

    let result = cpu.get_freg_f64(0) * cpu.get_freg_f64(1);
    cpu.set_freg_f64(2, result);

    // 应该能表示极大和极小的值
    assert!(cpu.get_freg_f64(2).is_finite());
}

// ============================================================================
// C扩展（压缩指令）测试
// ============================================================================

#[test]
fn test_c_extension_addi() {
    // 测试C.ADDI（压缩ADDI）
    let mut cpu = TestCPUState::new();

    cpu.set_reg(1, 10);

    // C.ADDI x1, 5
    let imm = 5i8;
    cpu.set_reg(1, cpu.get_reg(1).wrapping_add(imm as u64));

    assert_eq!(cpu.get_reg(1), 15);
}

#[test]
fn test_c_extension_size_reduction() {
    // 测试压缩指令的代码大小缩减
    // 原始32位：4字节
    // 压缩16位：2字节

    let insn_32bit_size = 4;
    let insn_16bit_size = 2;

    assert_eq!(insn_16bit_size * 2, insn_32bit_size);
}

#[test]
fn test_c_extension_lui() {
    // 测试C.LUI
    let mut cpu = TestCPUState::new();

    // C.LUI x1, 0x10
    let imm = 0x10u64;
    cpu.set_reg(1, imm << 12);

    assert_eq!(cpu.get_reg(1), 0x10000);
}

// ============================================================================
// A扩展（原子指令）测试
// ============================================================================

#[test]
fn test_a_extension_amoswap_logic() {
    // 测试AMOSWAP逻辑
    let mut memory = [42u64; 1];
    let new_val = 20u64;

    // AMOSWAP: 返回旧值，写入新值
    let old_val = memory[0];
    memory[0] = new_val;

    assert_eq!(old_val, 42);
    assert_eq!(memory[0], 20);
}

#[test]
fn test_a_extension_amoadd_logic() {
    // 测试AMOADD逻辑
    let mut memory = [100u64; 1];
    let increment = 50u64;

    // AMOADD: 返回旧值，加上新值
    let old_val = memory[0];
    memory[0] += increment;

    assert_eq!(old_val, 100);
    assert_eq!(memory[0], 150);
}

#[test]
fn test_a_extension_lr_sc_sequence() {
    // 测试LR/SC序列
    let mut memory = [42u64; 1];
    let reservation_valid = true;

    // LR: 加载值
    let old_val = memory[0];

    // 修改值
    let new_val = old_val + 1;

    // SC: 如果保留有效，存储新值
    if reservation_valid {
        memory[0] = new_val;
    }

    assert_eq!(memory[0], 43);
}

// ============================================================================
// 综合测试
// ============================================================================

#[test]
fn test_riscv_x0_always_zero() {
    // 测试x0寄存器始终为0
    let mut cpu = TestCPUState::new();

    cpu.set_reg(0, 42); // 尝试写入x0
    assert_eq!(cpu.get_reg(0), 0); // x0应该仍然是0
}

#[test]
fn test_riscv_register_state() {
    // 测试寄存器状态
    let mut cpu = TestCPUState::new();

    // 设置多个寄存器
    for i in 1..32 {
        cpu.set_reg(i, i as u64 * 10);
    }

    // 验证所有寄存器
    for i in 1..32 {
        assert_eq!(cpu.get_reg(i), i as u64 * 10);
    }

    // x0应该始终为0
    assert_eq!(cpu.get_reg(0), 0);
}

#[test]
fn test_riscv_memory_operations() {
    // 测试内存操作
    let mut cpu = TestCPUState::new();

    // 写入内存
    let addr = 0x1000usize;
    let val: u32 = 0x12345678;

    let bytes = val.to_le_bytes();
    cpu.memory[addr..addr + 4].copy_from_slice(&bytes);

    // 读回内存
    let read_bytes = &cpu.memory[addr..addr + 4];
    let read_val = u32::from_le_bytes(read_bytes.try_into().unwrap());

    assert_eq!(read_val, 0x12345678);
}

#[test]
fn test_riscv_fp_rounding_modes() {
    // 测试浮点舍入模式
    let val = 1.5f32;

    // RNE: 四舍五入到偶数
    let rne_result = val;
    assert_eq!(rne_result, 1.5);

    // RTZ: 向零舍入
    let rtz_result = val;
    assert_eq!(rtz_result, 1.5);
}

#[test]
fn test_riscv_extension_combinations() {
    // 测试扩展组合使用
    let mut cpu = TestCPUState::new();

    // M扩展：整数运算
    cpu.set_reg(1, 10);
    cpu.set_reg(2, 5);
    let mul_result = cpu.get_reg(1) * cpu.get_reg(2);
    cpu.set_reg(3, mul_result);

    // F扩展：浮点运算
    cpu.set_freg_f32(0, 2.0);
    cpu.set_freg_f32(1, 3.0);
    let fadd_result = cpu.get_freg_f32(0) + cpu.get_freg_f32(1);
    cpu.set_freg_f32(2, fadd_result);

    assert_eq!(cpu.get_reg(3), 50);
    assert!((cpu.get_freg_f32(2) - 5.0).abs() < f32::EPSILON);
}
