//! RISC-V Vector 编码器测试

#[cfg(test)]
mod tests {
    use super::super::encoder::{ArchEncoder, Riscv64Encoder};
    use super::super::Architecture;
    use vm_core::GuestAddr;
    use vm_ir::{IROp, RegId};

    #[test]
    fn test_riscv64_encoder_architecture() {
        let encoder = Riscv64Encoder;
        assert_eq!(encoder.architecture(), Architecture::RISCV64);
    }

    #[test]
    fn test_encode_vec_add() {
        let encoder = Riscv64Encoder;
        let op = IROp::VecAdd {
            dst: 1,
            src1: 2,
            src2: 3,
            element_size: 4,
        };

        let result = encoder.encode_op(&op, GuestAddr(0x1000));
        assert!(result.is_ok());

        let instructions = result.unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].length, 4);
        assert!(instructions[0].mnemonic.contains("vadd.vv"));
    }

    #[test]
    fn test_encode_vec_sub() {
        let encoder = Riscv64Encoder;
        let op = IROp::VecSub {
            dst: 1,
            src1: 2,
            src2: 3,
            element_size: 4,
        };

        let result = encoder.encode_op(&op, GuestAddr(0x1000));
        assert!(result.is_ok());

        let instructions = result.unwrap();
        assert_eq!(instructions.len(), 1);
        assert!(instructions[0].mnemonic.contains("vsub.vv"));
    }

    #[test]
    fn test_encode_vec_mul() {
        let encoder = Riscv64Encoder;
        let op = IROp::VecMul {
            dst: 1,
            src1: 2,
            src2: 3,
            element_size: 4,
        };

        let result = encoder.encode_op(&op, GuestAddr(0x1000));
        assert!(result.is_ok());

        let instructions = result.unwrap();
        assert_eq!(instructions.len(), 1);
        assert!(instructions[0].mnemonic.contains("vmul.vv"));
    }

    #[test]
    fn test_encode_vec_addsat() {
        let encoder = Riscv64Encoder;
        let op = IROp::VecAddSat {
            dst: 1,
            src1: 2,
            src2: 3,
            element_size: 4,
            signed: true,
        };

        let result = encoder.encode_op(&op, GuestAddr(0x1000));
        assert!(result.is_ok());

        let instructions = result.unwrap();
        assert_eq!(instructions.len(), 1);
        // 检查助记符包含饱和加法相关信息
        assert!(
            instructions[0].mnemonic.contains("vsadd") || instructions[0].mnemonic.contains("vsaddu")
        );
    }

    #[test]
    fn test_encode_vec128_add() {
        let encoder = Riscv64Encoder;
        let op = IROp::Vec128Add {
            dst_lo: 1,
            dst_hi: 2,
            src1_lo: 3,
            src1_hi: 4,
            src2_lo: 5,
            src2_hi: 6,
            element_size: 4,
            signed: false,
        };

        let result = encoder.encode_op(&op, GuestAddr(0x1000));
        assert!(result.is_ok());

        let instructions = result.unwrap();
        // 128位向量操作应该生成2条指令
        assert!(instructions.len() >= 2);
    }

    #[test]
    fn test_encode_vec256_add() {
        let encoder = Riscv64Encoder;
        let op = IROp::Vec256Add {
            dst0: 1,
            dst1: 2,
            dst2: 3,
            dst3: 4,
            src10: 5,
            src11: 6,
            src12: 7,
            src13: 8,
            src20: 9,
            src21: 10,
            src22: 11,
            src23: 12,
            element_size: 4,
            signed: false,
        };

        let result = encoder.encode_op(&op, GuestAddr(0x1000));
        assert!(result.is_ok());

        let instructions = result.unwrap();
        // 256位向量操作应该生成4条指令
        assert!(instructions.len() >= 4);
    }

    #[test]
    fn test_vector_instruction_encoding_format() {
        let encoder = Riscv64Encoder;
        let op = IROp::VecAdd {
            dst: 1,
            src1: 2,
            src2: 3,
            element_size: 4,
        };

        let result = encoder.encode_op(&op, GuestAddr(0x1000));
        assert!(result.is_ok());

        let instructions = result.unwrap();
        let insn_bytes = &instructions[0].bytes;

        // 检查指令长度
        assert_eq!(insn_bytes.len(), 4);

        // 检查 opcode (最低7位应该是 0x57)
        let opcode = insn_bytes[0] & 0x7f;
        assert_eq!(opcode, 0x57);

        // 检查 funct3 (位12-14应该是 0b001)
        let funct3 = (u32::from_le_bytes([
            insn_bytes[0],
            insn_bytes[1],
            insn_bytes[2],
            insn_bytes[3],
        ]) >> 12)
            & 0x7;
        assert_eq!(funct3, 0b001);

        // 检查 funct6 (位26-31应该是 0b000000 for VADD)
        let word = u32::from_le_bytes([
            insn_bytes[0],
            insn_bytes[1],
            insn_bytes[2],
            insn_bytes[3],
        ]);
        let funct6 = (word >> 26) & 0x3f;
        assert_eq!(funct6, 0b000000);
    }
}

