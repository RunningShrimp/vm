//! RISC-V Vector 扩展测试

#[cfg(test)]
mod tests {
    use super::super::vector::*;
    use vm_core::GuestAddr;
    use vm_ir::{IRBuilder, RegisterFile};

    #[test]
    fn test_vector_decode_vadd() {
        // VADD.VV v1, v2, v3
        // opcode=0x57, funct6=0b000000, funct3=0b001, vm=0, vs2=3, vs1=2, vd=1
        let insn: u32 = 0x57
            | (1 << 7)   // vd = 1
            | (0b001 << 12) // funct3 = 0b001
            | (2 << 15)  // vs1 = 2
            | (3 << 20)  // vs2 = 3
            | (0b000000 << 26); // funct6 = 0b000000

        assert_eq!(VectorDecoder::decode(insn), Some(VectorInstruction::Vadd));
    }

    #[test]
    fn test_vector_decode_vsub() {
        // VSUB.VV v1, v2, v3
        // funct6 = 0b000010
        let insn: u32 = 0x57
            | (1 << 7)
            | (0b001 << 12)
            | (2 << 15)
            | (3 << 20)
            | (0b000010 << 26);

        assert_eq!(VectorDecoder::decode(insn), Some(VectorInstruction::Vsub));
    }

    #[test]
    fn test_vector_decode_vmul() {
        // VMUL.VV v1, v2, v3
        // funct6 = 0b100101
        let insn: u32 = 0x57
            | (1 << 7)
            | (0b001 << 12)
            | (2 << 15)
            | (3 << 20)
            | (0b100101 << 26);

        assert_eq!(VectorDecoder::decode(insn), Some(VectorInstruction::Vmul));
    }

    #[test]
    fn test_vector_decode_vle8() {
        // VLE8.V v1, (x2)
        // funct3 = 0b000, funct6 = 0b000000
        let insn: u32 = 0x57
            | (1 << 7)   // vd = 1
            | (0b000 << 12) // funct3 = 0b000 (load)
            | (2 << 15)  // rs1 = 2
            | (0b000000 << 26);

        assert_eq!(VectorDecoder::decode(insn), Some(VectorInstruction::Vle8));
    }

    #[test]
    fn test_vector_decode_vse8() {
        // VSE8.V v1, (x2)
        // funct3 = 0b101, funct6 = 0b000000
        let insn: u32 = 0x57
            | (1 << 7)   // vs3 = 1
            | (0b101 << 12) // funct3 = 0b101 (store)
            | (2 << 15)  // rs1 = 2
            | (0b000000 << 26);

        assert_eq!(VectorDecoder::decode(insn), Some(VectorInstruction::Vse8));
    }

    #[test]
    fn test_vector_decode_vand() {
        // VAND.VV v1, v2, v3
        // funct3 = 0b010, funct6 = 0b000000
        let insn: u32 = 0x57
            | (1 << 7)
            | (0b010 << 12)
            | (2 << 15)
            | (3 << 20)
            | (0b000000 << 26);

        assert_eq!(VectorDecoder::decode(insn), Some(VectorInstruction::Vand));
    }

    #[test]
    fn test_vector_decode_vmseq() {
        // VMSEQ.VV v1, v2, v3
        // funct3 = 0b011, funct6 = 0b011000
        let insn: u32 = 0x57
            | (1 << 7)
            | (0b011 << 12)
            | (2 << 15)
            | (3 << 20)
            | (0b011000 << 26);

        assert_eq!(VectorDecoder::decode(insn), Some(VectorInstruction::Vmseq));
    }

    #[test]
    fn test_vector_decode_vredsum() {
        // VREDSUM.VS v1, v2, v3
        // funct3 = 0b100, funct6 = 0b000000
        let insn: u32 = 0x57
            | (1 << 7)
            | (0b100 << 12)
            | (2 << 15)
            | (3 << 20)
            | (0b000000 << 26);

        assert_eq!(VectorDecoder::decode(insn), Some(VectorInstruction::Vredsum));
    }

    #[test]
    fn test_vector_extract_element_size() {
        // VLE8: element_size = 1
        let insn_vle8: u32 = 0x57 | (0b000 << 12) | (0b000000 << 26);
        assert_eq!(VectorDecoder::extract_element_size(insn_vle8), 1);

        // VLE16: element_size = 2
        let insn_vle16: u32 = 0x57 | (0b000 << 12) | (0b000101 << 26);
        assert_eq!(VectorDecoder::extract_element_size(insn_vle16), 2);

        // VLE32: element_size = 4
        let insn_vle32: u32 = 0x57 | (0b000 << 12) | (0b000110 << 26);
        assert_eq!(VectorDecoder::extract_element_size(insn_vle32), 4);

        // VLE64: element_size = 8
        let insn_vle64: u32 = 0x57 | (0b000 << 12) | (0b000111 << 26);
        assert_eq!(VectorDecoder::extract_element_size(insn_vle64), 8);
    }

    // 简单的 Mock MMU 用于测试
    struct MockMMU;

    impl vm_core::AddressTranslator for MockMMU {
        fn translate(
            &mut self,
            va: vm_core::GuestAddr,
            _access: vm_core::AccessType,
        ) -> Result<vm_core::GuestPhysAddr, vm_core::VmError> {
            // 恒等映射
            Ok(vm_core::GuestPhysAddr(va.0))
        }

        fn flush_tlb(&mut self) {}
    }

    impl vm_core::MemoryAccess for MockMMU {
        fn read(&self, _pa: vm_core::GuestAddr, _size: u8) -> Result<u64, vm_core::VmError> {
            Ok(0)
        }

        fn write(
            &mut self,
            _pa: vm_core::GuestAddr,
            _val: u64,
            _size: u8,
        ) -> Result<(), vm_core::VmError> {
            Ok(())
        }

        fn fetch_insn(&self, _pc: vm_core::GuestAddr) -> Result<u64, vm_core::VmError> {
            Ok(0)
        }

        fn memory_size(&self) -> usize {
            1024
        }

        fn dump_memory(&self) -> Vec<u8> {
            vec![0; 1024]
        }

        fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
    }

    impl vm_core::MmioManager for MockMMU {
        fn map_mmio(
            &self,
            _base: vm_core::GuestAddr,
            _size: u64,
            _device: Box<dyn vm_core::MmioDevice>,
        ) {
        }
    }

    impl vm_core::MmuAsAny for MockMMU {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_vector_to_ir_vadd() {
        use vm_ir::RegisterMode;
        let mut reg_file = RegisterFile::new(32, RegisterMode::Standard);
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        let mmu = MockMMU;

        // VADD.VV v1, v2, v3
        let insn: u32 = 0x57
            | (1 << 7)
            | (0b001 << 12)
            | (2 << 15)
            | (3 << 20)
            | (0b000000 << 26);

        let result = VectorDecoder::to_ir(insn, &mut reg_file, &mut builder, &mmu, GuestAddr(0x1000));
        assert!(result.is_ok());

        let block = result.unwrap();
        assert_eq!(block.ops.len(), 1);
        // 检查是否为 VecAdd 操作
        match &block.ops[0] {
            vm_ir::IROp::VecAdd { dst, src1, src2, element_size } => {
                assert_eq!(*dst, reg_file.read(1));
                assert_eq!(*src1, reg_file.read(2));
                assert_eq!(*src2, reg_file.read(3));
                assert_eq!(*element_size, 4); // 默认32位
            }
            _ => panic!("Expected VecAdd operation"),
        }
    }
}

