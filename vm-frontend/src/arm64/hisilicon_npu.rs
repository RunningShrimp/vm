//! HiSilicon NPU (Neural Processing Unit) 指令解码器
//!
//! 华为达芬奇架构 NPU 指令

use vm_core::{GuestAddr, VmError};
use vm_ir::{IRBuilder, IROp, RegId, RegisterFile};

/// NPU 指令类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NpuInstruction {
    NpuLoad {
        dst: RegId,
        base: RegId,
        offset: i64,
    },
    NpuStore {
        src: RegId,
        base: RegId,
        offset: i64,
    },
    NpuConv {
        dst: RegId,
        src: RegId,
        kernel: RegId,
    },
    NpuFc {
        dst: RegId,
        src: RegId,
        weight: RegId,
    },
    NpuBn {
        dst: RegId,
        src: RegId,
        scale: RegId,
        bias: RegId,
    },
    NpuAct {
        dst: RegId,
        src: RegId,
        act_type: NpuActType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NpuActType {
    Relu,
    Sigmoid,
    Tanh,
}

pub struct NpuDecoder;

impl NpuDecoder {
    pub fn new() -> Self {
        Self
    }

    pub fn decode(&self, insn: u32, _pc: GuestAddr) -> Result<Option<NpuInstruction>, VmError> {
        if (insn >> 28) & 0xF == 0xC {
            let opcode = (insn >> 24) & 0xF;
            let dst = (insn >> 16) & 0x1F;
            let src1 = (insn >> 8) & 0x1F;
            let src2 = insn & 0x1F;

            match opcode {
                0x0 => Ok(Some(NpuInstruction::NpuLoad {
                    dst: dst as RegId,
                    base: src1 as RegId,
                    offset: src2 as i64,
                })),
                0x1 => Ok(Some(NpuInstruction::NpuStore {
                    src: dst as RegId,
                    base: src1 as RegId,
                    offset: src2 as i64,
                })),
                0x2 => Ok(Some(NpuInstruction::NpuConv {
                    dst: dst as RegId,
                    src: src1 as RegId,
                    kernel: src2 as RegId,
                })),
                0x3 => Ok(Some(NpuInstruction::NpuFc {
                    dst: dst as RegId,
                    src: src1 as RegId,
                    weight: src2 as RegId,
                })),
                0x4 => Ok(Some(NpuInstruction::NpuBn {
                    dst: dst as RegId,
                    src: src1 as RegId,
                    scale: src2 as RegId,
                    bias: ((insn >> 4) & 0x1F) as RegId,
                })),
                0x5 => Ok(Some(NpuInstruction::NpuAct {
                    dst: dst as RegId,
                    src: src1 as RegId,
                    act_type: match (insn >> 4) & 0x3 {
                        0 => NpuActType::Relu,
                        1 => NpuActType::Sigmoid,
                        2 => NpuActType::Tanh,
                        _ => NpuActType::Relu,
                    },
                })),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub fn to_ir(
        &self,
        insn: &NpuInstruction,
        builder: &mut IRBuilder,
        reg_file: &mut RegisterFile,
    ) -> Result<(), VmError> {
        match insn {
            NpuInstruction::NpuLoad { dst, base, offset } => {
                let dst_reg = reg_file.write(*dst as usize);
                builder.push(IROp::VendorLoad {
                    dst: dst_reg,
                    base: *base,
                    offset: *offset,
                    vendor: "hisilicon_npu".to_string(),
                    tile_id: 0,
                });
            }
            NpuInstruction::NpuStore { src, base, offset } => {
                builder.push(IROp::VendorStore {
                    src: reg_file.read(*src as usize),
                    base: *base,
                    offset: *offset,
                    vendor: "hisilicon_npu".to_string(),
                    tile_id: 0,
                });
            }
            NpuInstruction::NpuConv { dst, src, kernel } => {
                let dst_reg = reg_file.write(*dst as usize);
                builder.push(IROp::VendorMatrixOp {
                    dst: dst_reg,
                    op: "conv".to_string(),
                    tile_c: 0,
                    tile_a: (*src as u8),
                    tile_b: (*kernel as u8),
                    precision: "fp16".to_string(),
                });
            }
            NpuInstruction::NpuFc { dst, src, weight } => {
                let dst_reg = reg_file.write(*dst as usize);
                builder.push(IROp::VendorMatrixOp {
                    dst: dst_reg,
                    op: "fc".to_string(),
                    tile_c: 0,
                    tile_a: (*src as u8),
                    tile_b: (*weight as u8),
                    precision: "fp16".to_string(),
                });
            }
            NpuInstruction::NpuBn {
                dst,
                src,
                scale,
                bias,
            } => {
                let dst_reg = reg_file.write(*dst as usize);
                builder.push(IROp::VendorMatrixOp {
                    dst: dst_reg,
                    op: format!("bn_scale_{}_bias_{}", scale, bias),
                    tile_c: 0,
                    tile_a: (*src as u8),
                    tile_b: (*scale as u8),
                    precision: "fp16".to_string(),
                });
            }
            NpuInstruction::NpuAct { dst, src, act_type } => {
                let dst_reg = reg_file.write(*dst as usize);
                let op = match act_type {
                    NpuActType::Relu => "relu",
                    NpuActType::Sigmoid => "sigmoid",
                    NpuActType::Tanh => "tanh",
                };
                builder.push(IROp::VendorVectorOp {
                    dst: dst_reg,
                    op: op.to_string(),
                    src1: reg_file.read(*src as usize),
                    src2: 0,
                    vendor: "hisilicon_npu".to_string(),
                });
            }
        }
        Ok(())
    }
}

impl Default for NpuDecoder {
    fn default() -> Self {
        Self::new()
    }
}
