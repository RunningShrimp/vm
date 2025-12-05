//! MediaTek APU (AI Processing Unit) 指令解码器

use vm_core::{GuestAddr, VmError};
use vm_ir::{IRBuilder, IROp, RegId, RegisterFile};

/// APU 指令类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApuInstruction {
    ApuLoad {
        dst: RegId,
        base: RegId,
        offset: i64,
    },
    ApuStore {
        src: RegId,
        base: RegId,
        offset: i64,
    },
    ApuConv {
        dst: RegId,
        src: RegId,
        kernel: RegId,
        kernel_size: u8,
    },
    ApuPool {
        dst: RegId,
        src: RegId,
        pool_type: ApuPoolType,
    },
    ApuAct {
        dst: RegId,
        src: RegId,
        act_type: ApuActType,
    },
    ApuGemm {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        m: u16,
        n: u16,
        k: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApuPoolType {
    Max,
    Avg,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApuActType {
    Relu,
    Sigmoid,
    Tanh,
}

pub struct ApuDecoder;

impl ApuDecoder {
    pub fn new() -> Self {
        Self
    }

    pub fn decode(&self, insn: u32, _pc: GuestAddr) -> Result<Option<ApuInstruction>, VmError> {
        if (insn >> 28) & 0xF == 0xB {
            let opcode = (insn >> 24) & 0xF;
            let dst = (insn >> 16) & 0x1F;
            let src1 = (insn >> 8) & 0x1F;
            let src2 = insn & 0x1F;

            match opcode {
                0x0 => Ok(Some(ApuInstruction::ApuLoad {
                    dst: dst as RegId,
                    base: src1 as RegId,
                    offset: src2 as i64,
                })),
                0x1 => Ok(Some(ApuInstruction::ApuStore {
                    src: dst as RegId,
                    base: src1 as RegId,
                    offset: src2 as i64,
                })),
                0x2 => Ok(Some(ApuInstruction::ApuConv {
                    dst: dst as RegId,
                    src: src1 as RegId,
                    kernel: src2 as RegId,
                    kernel_size: ((insn >> 4) & 0xF) as u8,
                })),
                0x3 => Ok(Some(ApuInstruction::ApuPool {
                    dst: dst as RegId,
                    src: src1 as RegId,
                    pool_type: if (insn >> 4) & 1 == 0 {
                        ApuPoolType::Max
                    } else {
                        ApuPoolType::Avg
                    },
                })),
                0x4 => Ok(Some(ApuInstruction::ApuAct {
                    dst: dst as RegId,
                    src: src1 as RegId,
                    act_type: match (insn >> 4) & 0x3 {
                        0 => ApuActType::Relu,
                        1 => ApuActType::Sigmoid,
                        2 => ApuActType::Tanh,
                        _ => ApuActType::Relu,
                    },
                })),
                0x5 => Ok(Some(ApuInstruction::ApuGemm {
                    dst: dst as RegId,
                    src1: src1 as RegId,
                    src2: src2 as RegId,
                    m: ((insn >> 12) & 0xFF) as u16,
                    n: ((insn >> 4) & 0xFF) as u16,
                    k: 0, // 从其他字段获取
                })),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub fn to_ir(
        &self,
        insn: &ApuInstruction,
        builder: &mut IRBuilder,
        reg_file: &mut RegisterFile,
    ) -> Result<(), VmError> {
        match insn {
            ApuInstruction::ApuLoad { dst, base, offset } => {
                let dst_reg = reg_file.write(*dst as usize);
                builder.push(IROp::VendorLoad {
                    dst: dst_reg,
                    base: *base,
                    offset: *offset,
                    vendor: "mediatek_apu".to_string(),
                    tile_id: 0,
                });
            }
            ApuInstruction::ApuStore { src, base, offset } => {
                builder.push(IROp::VendorStore {
                    src: reg_file.read(*src as usize),
                    base: *base,
                    offset: *offset,
                    vendor: "mediatek_apu".to_string(),
                    tile_id: 0,
                });
            }
            ApuInstruction::ApuConv {
                dst,
                src,
                kernel,
                kernel_size,
            } => {
                let dst_reg = reg_file.write(*dst as usize);
                builder.push(IROp::VendorMatrixOp {
                    dst: dst_reg,
                    op: "conv".to_string(),
                    tile_c: 0,
                    tile_a: (*src as u8),
                    tile_b: (*kernel as u8),
                    precision: format!("kernel_size={}", kernel_size),
                });
            }
            ApuInstruction::ApuPool {
                dst,
                src,
                pool_type,
            } => {
                let dst_reg = reg_file.write(*dst as usize);
                let op = match pool_type {
                    ApuPoolType::Max => "pool_max",
                    ApuPoolType::Avg => "pool_avg",
                };
                builder.push(IROp::VendorVectorOp {
                    dst: dst_reg,
                    op: op.to_string(),
                    src1: reg_file.read(*src as usize),
                    src2: 0,
                    vendor: "mediatek_apu".to_string(),
                });
            }
            ApuInstruction::ApuAct { dst, src, act_type } => {
                let dst_reg = reg_file.write(*dst as usize);
                let op = match act_type {
                    ApuActType::Relu => "relu",
                    ApuActType::Sigmoid => "sigmoid",
                    ApuActType::Tanh => "tanh",
                };
                builder.push(IROp::VendorVectorOp {
                    dst: dst_reg,
                    op: op.to_string(),
                    src1: reg_file.read(*src as usize),
                    src2: 0,
                    vendor: "mediatek_apu".to_string(),
                });
            }
            ApuInstruction::ApuGemm {
                dst,
                src1,
                src2,
                m,
                n,
                k: _,
            } => {
                let dst_reg = reg_file.write(*dst as usize);
                builder.push(IROp::VendorMatrixOp {
                    dst: dst_reg,
                    op: format!("gemm_m{}_n{}", m, n),
                    tile_c: 0,
                    tile_a: (*src1 as u8),
                    tile_b: (*src2 as u8),
                    precision: "fp32".to_string(),
                });
            }
        }
        Ok(())
    }
}

impl Default for ApuDecoder {
    fn default() -> Self {
        Self::new()
    }
}

