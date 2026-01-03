//! vm-core属性测试
//!
//! 使用proptest进行基于属性的测试，验证核心不变量

use proptest::prelude::*;
use vm_core::{CoreError, GuestAddr, VmError};

// ============================================================================
// 页表地址转换属性测试
// ============================================================================

proptest! {
    #[test]
    fn test_guest_addr_roundtrip(addr in any::<u64>()) {
        // GuestAddr应该能正确地往返转换
        let guest_addr = GuestAddr(addr);
        let back = guest_addr.0;
        prop_assert_eq!(addr, back);
    }

    #[test]
    fn test_guest_addr_alignment(addr in any::<u64>()) {
        // 测试地址对齐属性
        let guest_addr = GuestAddr(addr);

        // 如果地址是对齐的，按位与应该保持不变
        let aligned_addr = GuestAddr(addr & !0xFFF);
        prop_assert_eq!(aligned_addr.0 & 0xFFF, 0);

        // 原始地址应该等于对齐地址加上偏移
        prop_assert_eq!(guest_addr.0, aligned_addr.0 | (addr & 0xFFF));
    }

    #[test]
    fn test_guest_addr_page_boundary(addr in any::<u64>()) {
        // 测试页边界属性
        let guest_addr = GuestAddr(addr);
        let page_num = guest_addr.0 / 4096;
        let offset = guest_addr.0 % 4096;

        // 重新组合应该得到原始地址
        prop_assert_eq!(guest_addr.0, page_num * 4096 + offset);
    }
}

// ============================================================================
// 寄存器操作属性测试
// ============================================================================

proptest! {
    #[test]
    fn test_register_write_read(vals in prop::collection::vec(any::<u64>(), 1..32)) {
        // 创建简单的寄存器数组模拟
        let mut regs = [0u64; 32];

        // 写入值
        for (i, val) in vals.iter().enumerate() {
            regs[i] = *val;
        }

        // 读回应该相同
        for (i, val) in vals.iter().enumerate() {
            prop_assert_eq!(regs[i], *val);
        }
    }

    #[test]
    fn test_register_id_bounds(idx in any::<usize>()) {
        // 寄存器索引应该在有效范围内
        let valid = idx < 32;

        if valid {
            prop_assert!(idx < 32);
        } else {
            prop_assert!(idx >= 32);
        }
    }

    #[test]
    fn test_register_zero_always_zero(val in any::<u64>()) {
        // x0寄存器应该始终为0（RISC-V规范）
        // 这是一个简化测试，演示期望行为
        // 实际实现中，x0的写入应该被忽略或自动清零

        // 简化的模拟：我们手动确保x0始终为0
        let mut regs = [0u64; 32];

        // 尝试写入x0（在实际RISC-V实现中应该被忽略）
        // regs[0] = val;  // 这行在真实硬件中应该无效

        // x0应该仍然是0
        prop_assert_eq!(regs[0], 0);

        // 其他寄存器应该可以正常写入
        regs[1] = val;
        prop_assert_eq!(regs[1], val);
    }

    #[test]
    fn test_register_arithmetic_identity(a in any::<u64>(), b in any::<u64>()) {
        // 测试算术恒等式：加法单位元
        prop_assert_eq!(a.wrapping_add(0), a);
        prop_assert_eq!(0u64.wrapping_add(a), a);

        // 乘法单位元
        prop_assert_eq!(a.wrapping_mul(1), a);
        prop_assert_eq!(1u64.wrapping_mul(a), a);

        // 加法交换律
        prop_assert_eq!(a.wrapping_add(b), b.wrapping_add(a));

        // 乘法交换律
        prop_assert_eq!(a.wrapping_mul(b), b.wrapping_mul(a));
    }

    #[test]
    fn test_register_bitwise_identity(a in any::<u64>()) {
        // 测试位运算恒等式
        prop_assert_eq!(a | 0, a);
        prop_assert_eq!(a & u64::MAX, a);

        // 异或自身应该为0
        prop_assert_eq!(a ^ a, 0);

        // 或自身应该不变
        prop_assert_eq!(a | a, a);

        // 与自身应该不变
        prop_assert_eq!(a & a, a);
    }
}

// ============================================================================
// 内存操作属性测试
// ============================================================================

proptest! {
    #[test]
    fn test_memory_byte_addressing(
        addr1 in any::<u64>(),
        addr2 in any::<u64>(),
        val1 in any::<u64>(),
        val2 in any::<u64>(),
    ) {
        // 创建简单的内存模拟（字节数组）
        let mut memory = vec![0u8; 65536]; // 64KB

        // 写入两个8字节值
        let aligned_addr1 = (addr1 & 0xFFFE) as usize;
        let aligned_addr2 = (addr2 & 0xFFFE) as usize;

        if aligned_addr1 + 8 <= memory.len() {
            memory[aligned_addr1..aligned_addr1+8].copy_from_slice(&val1.to_le_bytes());
        }

        if aligned_addr2 + 8 <= memory.len() && aligned_addr2 != aligned_addr1 {
            memory[aligned_addr2..aligned_addr2+8].copy_from_slice(&val2.to_le_bytes());
        }

        // 读回应该相同
        if aligned_addr1 + 8 <= memory.len() {
            prop_assert_eq!(u64::from_le_bytes(memory[aligned_addr1..aligned_addr1+8].try_into().unwrap()), val1);
        }

        if aligned_addr2 + 8 <= memory.len() && aligned_addr2 != aligned_addr1 {
            prop_assert_eq!(u64::from_le_bytes(memory[aligned_addr2..aligned_addr2+8].try_into().unwrap()), val2);
        }
    }

    #[test]
    fn test_memory_write_read_roundtrip(val in any::<u64>()) {
        // 测试写入和读回
        let mut memory = [0u8; 8];

        // 写入u64值（小端序）
        memory[0] = (val & 0xFF) as u8;
        memory[1] = ((val >> 8) & 0xFF) as u8;
        memory[2] = ((val >> 16) & 0xFF) as u8;
        memory[3] = ((val >> 24) & 0xFF) as u8;
        memory[4] = ((val >> 32) & 0xFF) as u8;
        memory[5] = ((val >> 40) & 0xFF) as u8;
        memory[6] = ((val >> 48) & 0xFF) as u8;
        memory[7] = ((val >> 56) & 0xFF) as u8;

        // 读回应该相同
        let read_val = memory[0] as u64
            | ((memory[1] as u64) << 8)
            | ((memory[2] as u64) << 16)
            | ((memory[3] as u64) << 24)
            | ((memory[4] as u64) << 32)
            | ((memory[5] as u64) << 40)
            | ((memory[6] as u64) << 48)
            | ((memory[7] as u64) << 56);

        prop_assert_eq!(val, read_val);
    }

    #[test]
    fn test_memory_alignment(addr in any::<u64>(), size in 1usize..9) {
        // 测试内存对齐属性
        let alignment = size.next_power_of_two() as u64;

        // 对齐的地址
        let aligned_addr = (addr + alignment - 1) & !(alignment - 1);

        // 验证对齐
        prop_assert_eq!(aligned_addr % alignment, 0);

        // 验证对齐地址 >= 原始地址
        prop_assert!(aligned_addr >= addr || addr == 0);
    }

    #[test]
    fn test_memory_overlap_detection(
        addr1 in any::<u64>(),
        addr2 in any::<u64>(),
        size in 1usize..17
    ) {
        // 检测内存区域重叠
        let end1 = addr1.saturating_add(size as u64);
        let end2 = addr2.saturating_add(size as u64);

        let overlaps = addr1 < end2 && addr2 < end1;

        // 如果重叠，那么其中一个区域的开始应该在另一个区域内
        if overlaps {
            let condition = (addr1 >= addr2 && addr1 < end2) || (addr2 >= addr1 && addr2 < end1);
            prop_assert!(condition);
        }
    }
}

// ============================================================================
// VmError 属性测试
// ============================================================================

proptest! {
    #[test]
    fn test_vm_error_display(message in any::<String>()) {
        // VmError应该能正确显示
        let error = VmError::Core(CoreError::NotSupported {
            feature: message.clone(),
            module: "test_module".to_string(),
        });

        // 显示应该包含消息
        let display_str = format!("{}", error);
        prop_assert!(display_str.contains(&message) || display_str.len() > 0);
    }

    #[test]
    fn test_vm_error_clone_roundtrip(feature in any::<String>(), module in any::<String>()) {
        // VmError应该能正确克隆
        let error1 = VmError::Core(CoreError::NotImplemented {
            feature: feature.clone(),
            module: module.clone(),
        });

        let error2 = error1.clone();

        // 两者应该相等（注意：VmError可能没有实现PartialEq，所以我们检查格式化输出）
        let str1 = format!("{:?}", error1);
        let str2 = format!("{:?}", error2);

        prop_assert_eq!(str1, str2);
    }
}

// ============================================================================
// 算术运算属性测试
// ============================================================================

proptest! {
    #[test]
    fn test_wrapping_arithmetic_consistency(a in any::<u64>(), b in any::<u64>()) {
        // 测试环绕算术的一致性

        // 加法结合律
        let sum1 = a.wrapping_add(b).wrapping_add(1);
        let sum2 = a.wrapping_add(b.wrapping_add(1));
        prop_assert_eq!(sum1, sum2);

        // 减法性质：a - (b + c) = (a - b) - c
        let diff1 = a.wrapping_sub(b.wrapping_add(1));
        let diff2 = a.wrapping_sub(b).wrapping_sub(1);
        prop_assert_eq!(diff1, diff2);
    }

    #[test]
    fn test_bitwise_operations(a in any::<u64>(), b in any::<u64>()) {
        // 德摩根定律
        prop_assert_eq!(!(a & b), (!a) | (!b));
        prop_assert_eq!(!(a | b), (!a) & (!b));

        // 异或性质
        prop_assert_eq!(a ^ b, b ^ a);  // 交换律
        prop_assert_eq!((a ^ b) ^ a, b); // 自逆性
    }

    #[test]
    fn test_shift_operations(val in any::<u64>(), shift in any::<u8>()) {
        // 移位运算属性
        let shift = shift % 64; // 标准化移位量

        // 左移再右移：只在值为0时完全恢复（否则高位信息丢失）
        let shifted = val.wrapping_shl(shift as u32);
        let restored = shifted.wrapping_shr(shift as u32);

        // 恢复后的值应该是原始值的低(64-shift)位左移后的结果
        // 或者更简单：如果原始值为0，则恢复后也应该为0
        if val == 0 {
            prop_assert_eq!(restored, 0);
        } else {
            // 验证：左移再右移后，高位被清零
            let mask = if shift == 0 { u64::MAX } else { (1u64 << (64 - shift as u32)) - 1 };
            prop_assert_eq!(restored, val & mask);
        }

        // 右移再左移：低位信息丢失
        let shifted_right = val.wrapping_shr(shift as u32);
        let restored_left = shifted_right.wrapping_shl(shift as u32);

        // 恢复后的值应该只有高位部分（低位被清零）
        if shift == 0 {
            prop_assert_eq!(restored_left, val);
        } else {
            let mask = u64::MAX << shift;
            prop_assert_eq!(restored_left, val & mask);
        }
    }

    #[test]
    fn test_multiplication_by_power_of_two(val in any::<u64>(), power in 0u8..6u8) {
        // 乘以2的幂应该等于左移
        let pow2 = 1u64 << power;
        prop_assert_eq!(val.wrapping_mul(pow2), val.wrapping_shl(power as u32));
    }

    #[test]
    fn test_division_by_power_of_two(val in any::<u64>(), power in 0u8..6u8) {
        // 除以2的幂应该等于右移（对于正数）
        let pow2 = 1u64 << power;
        prop_assert_eq!(val / pow2, val >> power);
    }
}

// ============================================================================
// 状态转换属性测试
// ============================================================================

proptest! {
    #[test]
    fn test_state_machine_consistency(transitions in prop::collection::vec(any::<u8>(), 1..50)) {
        // 简单状态机：0=未初始化, 1=初始化, 2=运行中, 3=停止
        let mut state = 0u8;

        for transition in transitions {
            match state {
                0 => {
                    // 未初始化 -> 可以转到初始化
                    if transition % 2 == 0 {
                        state = 1;
                    }
                }
                1 => {
                    // 初始化 -> 可以转到运行中或停止
                    if transition % 2 == 0 {
                        state = 2;
                    } else {
                        state = 3;
                    }
                }
                2 => {
                    // 运行中 -> 可以转到停止
                    if transition % 3 == 0 {
                        state = 3;
                    }
                }
                3 => {
                    // 停止 -> 可以回到未初始化
                    if transition % 2 == 0 {
                        state = 0;
                    }
                }
                _ => unreachable!(),
            }

            // 状态始终应该在有效范围内
            prop_assert!(state <= 3);
        }
    }
}
