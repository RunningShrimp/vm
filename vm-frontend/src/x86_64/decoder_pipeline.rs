//! x86-64 解码管道 - 模块化解码架构的核心
//!
//! 提供统一的解码管道，将前缀、操作码、操作数解码串联起来

use vm_core::MMU;
use vm_core::{Fault, GuestAddr, VmError};

use super::{
    X86Instruction, X86Mnemonic, X86Operand,
    opcode_decode::{OpcodeInfo, decode_opcode},
    prefix_decode::{PrefixInfo, decode_prefixes},
};

/// 简化的指令流结构
pub struct InsnStream<'a, M: MMU + ?Sized> {
    pub mmu: &'a M,
    pub pc: GuestAddr,
    pub offset: usize,
}

impl<'a, M: MMU + ?Sized> InsnStream<'a, M> {
    pub fn new(mmu: &'a M, pc: GuestAddr) -> Self {
        Self { mmu, pc, offset: 0 }
    }

    pub fn read_u8(&mut self) -> Result<u8, VmError> {
        let addr = self.pc + self.offset as u64;
        let value = self.mmu.read(addr, 1)? as u8;
        self.offset += 1;
        Ok(value)
    }

    pub fn read_u32(&mut self) -> Result<u32, VmError> {
        let addr = self.pc + self.offset as u64;
        let value = self.mmu.read(addr, 4)? as u32;
        self.offset += 4;
        Ok(value)
    }

    pub fn read_u64(&mut self) -> Result<u64, VmError> {
        let addr = self.pc + self.offset as u64;
        let value = self.mmu.read(addr, 8)?;
        self.offset += 8;
        Ok(value)
    }

    pub fn read_u16(&mut self) -> Result<u16, VmError> {
        let addr = self.pc + self.offset as u64;
        let value = self.mmu.read(addr, 2)? as u16;
        self.offset += 2;
        Ok(value)
    }

    pub fn current_offset(&self) -> usize {
        self.offset
    }
}

/// 解码管道 - 统一的解码流程
pub struct DecoderPipeline {
    prefix_info: PrefixInfo,
    opcode_info: Option<OpcodeInfo>,
    operands: Vec<super::operand_decode::Operand>,
}

impl DecoderPipeline {
    /// 创建新的解码管道
    pub fn new() -> Self {
        Self {
            prefix_info: PrefixInfo::default(),
            opcode_info: None,
            operands: Vec::new(),
        }
    }

    /// 执行完整的解码流程
    pub fn decode_instruction<M: MMU>(
        &mut self,
        stream: &mut InsnStream<M>,
        pc: GuestAddr,
    ) -> Result<X86Instruction, VmError> {
        // 阶段1: 解析前缀
        let (prefix_info, _) = decode_prefixes(|| {
            stream
                .read_u8()
                .map_err(|_| "Failed to read byte".to_string())
        })
        .map_err(|_| VmError::from(Fault::InvalidOpcode { pc, opcode: 0 }))?;
        self.prefix_info = prefix_info;

        // 阶段2: 解析操作码
        let first_opcode = stream.read_u8()?;
        let is_two_byte = first_opcode == 0x0F;

        if is_two_byte {
            let second_opcode = stream.read_u8()?;
            match decode_opcode(second_opcode, &self.prefix_info, true) {
                Ok(Some(opcode_info)) => self.opcode_info = Some(opcode_info),
                Ok(None) => {
                    return Err(VmError::from(Fault::InvalidOpcode {
                        pc,
                        opcode: second_opcode as u32,
                    }));
                }
                Err(_) => {
                    return Err(VmError::from(Fault::InvalidOpcode {
                        pc,
                        opcode: second_opcode as u32,
                    }));
                }
            }
        } else {
            match decode_opcode(first_opcode, &self.prefix_info, false) {
                Ok(Some(opcode_info)) => self.opcode_info = Some(opcode_info),
                Ok(None) => {
                    return Err(VmError::from(Fault::InvalidOpcode {
                        pc,
                        opcode: first_opcode as u32,
                    }));
                }
                Err(_) => {
                    return Err(VmError::from(Fault::InvalidOpcode {
                        pc,
                        opcode: first_opcode as u32,
                    }));
                }
            }
        }

        // 阶段3: 解析操作数
        let opcode_info = self.opcode_info.clone(); // 克隆以避免借用问题
        if let Some(ref opcode_info) = opcode_info {
            self.decode_operands(stream, opcode_info)?;
        }

        // 阶段4: 构建指令
        self.build_instruction(pc)
    }

    /// 解析操作数
    fn decode_operands<M: MMU>(
        &mut self,
        stream: &mut InsnStream<M>,
        opcode_info: &OpcodeInfo,
    ) -> Result<(), VmError> {
        // 用于操作数解码的字节缓冲区
        let mut operand_bytes = Vec::new();

        // 如果需要ModR/M字节
        let modrm = if opcode_info.requires_modrm {
            let modrm_byte = stream.read_u8()?;
            operand_bytes.push(modrm_byte);
            Some(super::operand_decode::ModRM::from_byte(modrm_byte))
        } else {
            None
        };

        // 读取可能的立即数字节
        if opcode_info.op1_kind == super::opcode_decode::OperandKind::Imm8 {
            let imm8 = stream.read_u8()?;
            operand_bytes.push(imm8);
        } else if opcode_info.op1_kind == super::opcode_decode::OperandKind::Imm32
            || opcode_info.op1_kind == super::opcode_decode::OperandKind::Rel32
        {
            let imm32_low = stream.read_u8()?;
            let imm32 = stream.read_u32()? as u64 | ((imm32_low as u64) << 32);
            operand_bytes.push(imm32_low);
            operand_bytes.extend_from_slice(&imm32.to_le_bytes()[0..4]);
        }

        // 创建操作数解码器并传递操作码字节
        let mut operand_decoder = super::operand_decode::OperandDecoder::new_with_opcode(
            &operand_bytes,
            opcode_info.opcode_byte,
        );

        // 解码第一个操作数
        if opcode_info.op1_kind != super::opcode_decode::OperandKind::None {
            let op1 = operand_decoder
                .decode_operand(
                    opcode_info.op1_kind,
                    modrm,
                    &self.prefix_info,
                    8, // 默认操作数大小
                )
                .map_err(|_| {
                    VmError::from(Fault::InvalidOpcode {
                        pc: vm_core::GuestAddr(0),
                        opcode: opcode_info.opcode_byte as u32,
                    })
                })?;
            self.operands.push(op1);
        }

        // 解码第二个操作数
        if opcode_info.op2_kind != super::opcode_decode::OperandKind::None {
            let op2 = operand_decoder
                .decode_operand(
                    opcode_info.op2_kind,
                    modrm,
                    &self.prefix_info,
                    8, // 默认操作数大小
                )
                .map_err(|_| {
                    VmError::from(Fault::InvalidOpcode {
                        pc: vm_core::GuestAddr(0),
                        opcode: opcode_info.opcode_byte as u32,
                    })
                })?;
            self.operands.push(op2);
        }

        // 解码第三个操作数
        if opcode_info.op3_kind != super::opcode_decode::OperandKind::None {
            let op3 = operand_decoder
                .decode_operand(
                    opcode_info.op3_kind,
                    modrm,
                    &self.prefix_info,
                    8, // 默认操作数大小
                )
                .map_err(|_| {
                    VmError::from(Fault::InvalidOpcode {
                        pc: vm_core::GuestAddr(0),
                        opcode: opcode_info.opcode_byte as u32,
                    })
                })?;
            self.operands.push(op3);
        }

        Ok(())
    }

    /// 构建最终指令
    fn build_instruction(&self, pc: GuestAddr) -> Result<X86Instruction, VmError> {
        let mnemonic = if let Some(ref opcode_info) = self.opcode_info {
            opcode_info.mnemonic.parse().unwrap_or(X86Mnemonic::Nop)
        } else {
            X86Mnemonic::Nop
        };

        let op1 = self
            .operands
            .first()
            .cloned()
            .unwrap_or(super::operand_decode::Operand::None);
        let op2 = self
            .operands
            .get(1)
            .cloned()
            .unwrap_or(super::operand_decode::Operand::None);
        let op3 = self
            .operands
            .get(2)
            .cloned()
            .unwrap_or(super::operand_decode::Operand::None);

        // 转换操作数格式到X86Instruction的格式
        let x86_op1 = self.convert_operand(&op1);
        let x86_op2 = self.convert_operand(&op2);
        let x86_op3 = self.convert_operand(&op3);

        Ok(X86Instruction {
            mnemonic,
            op1: x86_op1,
            op2: x86_op2,
            op3: x86_op3,
            op_size: 32, // 默认32位操作数大小
            lock: self.prefix_info.lock,
            rep: self.prefix_info.rep,
            repne: self.prefix_info.repne,
            next_pc: pc,  // 将在translate阶段更新
            jcc_cc: None, // 将在具体指令处理中设置
        })
    }

    /// 转换操作数格式
    fn convert_operand(&self, operand: &super::operand_decode::Operand) -> X86Operand {
        match operand {
            super::operand_decode::Operand::None => X86Operand::None,
            super::operand_decode::Operand::Reg { reg, .. } => X86Operand::Reg(*reg),
            super::operand_decode::Operand::Xmm { reg } => X86Operand::Xmm(*reg),
            super::operand_decode::Operand::Memory { addr, .. } => {
                // 从MemoryOperand转换为简单格式
                match addr {
                    super::operand_decode::MemoryOperand::Direct { base, disp } => {
                        X86Operand::Mem {
                            base: Some(*base),
                            index: None,
                            scale: 1,
                            disp: *disp,
                        }
                    }
                    super::operand_decode::MemoryOperand::Indexed {
                        base,
                        index,
                        scale,
                        disp,
                    } => X86Operand::Mem {
                        base: *base,
                        index: Some(*index),
                        scale: *scale,
                        disp: *disp,
                    },
                    super::operand_decode::MemoryOperand::Rip { disp } => {
                        X86Operand::Mem {
                            base: None, // RIP通过特殊方式处理
                            index: None,
                            scale: 1,
                            disp: *disp as i64,
                        }
                    }
                }
            }
            super::operand_decode::Operand::Immediate { value, .. } => X86Operand::Imm(*value),
            super::operand_decode::Operand::Relative { offset } => X86Operand::Rel(*offset as i64),
        }
    }

    /// 重置管道状态以供下一条指令使用
    pub fn reset(&mut self) {
        self.prefix_info = PrefixInfo::default();
        self.opcode_info = None;
        self.operands.clear();
    }
}

/// 默认实现
impl Default for DecoderPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "vm-mem"))]
mod tests {
    use vm_core::MemoryAccess;
    use vm_mem::SoftMmu;

    use super::*;

    #[test]
    fn test_decoder_pipeline_basic() {
        let mut mmu = SoftMmu::new(1024 * 1024, false); // 1MB内存

        // 测试简单的NOP指令: 0x90
        let code = vec![0x90];

        // 将代码写入内存
        for (i, &byte) in code.iter().enumerate() {
            mmu.write(vm_core::GuestAddr(0x1000 + i as u64), byte as u64, 1)
                .expect("Failed to write test code to memory");
        }

        let mut stream = InsnStream::new(&mmu, vm_core::GuestAddr(0x1000));
        let mut pipeline = DecoderPipeline::new();

        let instruction = pipeline
            .decode_instruction(&mut stream, vm_core::GuestAddr(0x1000))
            .expect("Failed to decode NOP instruction");

        assert_eq!(instruction.mnemonic, X86Mnemonic::Nop);
        // 验证操作数正确解码
    }

    #[test]
    fn test_insn_stream() {
        let mut mmu = SoftMmu::new(1024, false);

        // 写入测试数据
        mmu.write(vm_core::GuestAddr(0x1000), 0x12345678, 4)
            .expect("Failed to write test data at 0x1000");
        mmu.write(vm_core::GuestAddr(0x1004), 0x9ABCDEF0, 4)
            .expect("Failed to write test data at 0x1004");
        mmu.write(vm_core::GuestAddr(0x1008), 0x90ABCDEF12345678, 8)
            .expect("Failed to write test data at 0x1008");

        let mut stream = InsnStream::new(&mmu, vm_core::GuestAddr(0x1000));

        // 测试读取不同大小的数据
        assert_eq!(
            stream.read_u8().expect("Failed to read u8 at offset 0"),
            0x78
        );
        assert_eq!(
            stream.read_u8().expect("Failed to read u8 at offset 1"),
            0x56
        );
        assert_eq!(
            stream.read_u32().expect("Failed to read u32 at offset 2"),
            0x12345678
        );
        assert_eq!(
            stream.read_u64().expect("Failed to read u64 at offset 6"),
            0x90ABCDEF12345678
        );
        assert_eq!(stream.current_offset(), 15);
    }
}
