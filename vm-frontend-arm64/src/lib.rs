use vm_accel::cpuinfo::CpuInfo;
use vm_core::{Decoder, GuestAddr, MMU, VmError};
use vm_ir::{IRBlock, IRBuilder, IROp, MemFlags, RegisterFile, Terminator};

mod apple_amx;
mod extended_insns;
mod hisilicon_npu;
mod mediatek_apu;
mod qualcomm_hexagon;

use crate::extended_insns::ExtendedDecoder;

pub use apple_amx::{AmxDecoder, AmxInstruction, AmxPrecision};
pub use hisilicon_npu::{NpuActType, NpuDecoder, NpuInstruction};
pub use mediatek_apu::{ApuActType, ApuDecoder, ApuInstruction, ApuPoolType};
pub use qualcomm_hexagon::{HexVectorOp, HexagonDecoder, HexagonInstruction};
pub enum Cond {
    EQ = 0,
    NE = 1,
    CS = 2,
    CC = 3,
    MI = 4,
    PL = 5,
    VS = 6,
    VC = 7,
    HI = 8,
    LS = 9,
    GE = 10,
    LT = 11,
    GT = 12,
    LE = 13,
}

/// ARM64 指令表示
#[derive(Debug, Clone)]
pub struct Arm64Instruction {
    pub mnemonic: &'static str,
    pub next_pc: GuestAddr,
    pub has_memory_op: bool,
    pub is_branch: bool,
}

impl Arm64Instruction {
    pub fn next_pc(&self) -> GuestAddr {
        self.next_pc
    }

    pub fn size(&self) -> u8 {
        4 // ARM64 指令固定 4 字节
    }

    pub fn operand_count(&self) -> usize {
        1 // 简化实现
    }

    pub fn mnemonic(&self) -> &str {
        self.mnemonic
    }

    pub fn is_control_flow(&self) -> bool {
        self.is_branch
    }

    pub fn is_memory_access(&self) -> bool {
        self.has_memory_op
    }
}

/// ARM64 解码器，支持解码缓存优化
pub struct Arm64Decoder {
    /// 解码缓存
    decode_cache: Option<std::collections::HashMap<GuestAddr, IRBlock>>,
    /// 缓存大小限制
    cache_size_limit: usize,
    /// AMX 解码器
    amx_decoder: AmxDecoder,
    /// Hexagon DSP 解码器
    hexagon_decoder: HexagonDecoder,
    /// APU 解码器
    apu_decoder: ApuDecoder,
    /// NPU 解码器
    npu_decoder: NpuDecoder,
}

impl Arm64Decoder {
    /// 创建新的解码器
    pub fn new() -> Self {
        Self {
            decode_cache: Some(std::collections::HashMap::new()),
            cache_size_limit: 1024,
            amx_decoder: AmxDecoder::new(),
            hexagon_decoder: HexagonDecoder::new(),
            apu_decoder: ApuDecoder::new(),
            npu_decoder: NpuDecoder::new(),
        }
    }

    /// 创建不带缓存的解码器
    pub fn without_cache() -> Self {
        Self {
            decode_cache: None,
            cache_size_limit: 0,
            amx_decoder: AmxDecoder::new(),
            hexagon_decoder: HexagonDecoder::new(),
            apu_decoder: ApuDecoder::new(),
            npu_decoder: NpuDecoder::new(),
        }
    }

    /// 清除解码缓存
    pub fn clear_cache(&mut self) {
        if let Some(ref mut cache) = self.decode_cache {
            cache.clear();
        }
    }
}

impl Default for Arm64Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for Arm64Decoder {
    type Instruction = Arm64Instruction;
    type Block = IRBlock;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Instruction, VmError> {
        let insn = mmu.fetch_insn(pc)? as u32;

        // Determine mnemonic based on instruction pattern
        let mnemonic = if (insn & 0x1F000000) == 0x11000000 {
            if (insn & 0x40000000) != 0 {
                "sub"
            } else {
                "add"
            }
        } else if (insn & 0x7F800000) == 0x12800000 {
            "movn"
        } else if (insn & 0x7F800000) == 0x12000000 {
            "movz"
        } else {
            "unknown"
        };

        let is_branch = matches!(mnemonic, "b" | "bl" | "br" | "blr");
        let has_memory_op = matches!(mnemonic, "ldr" | "str" | "ldp" | "stp");

        Ok(Arm64Instruction {
            mnemonic,
            next_pc: pc + 4,
            has_memory_op,
            is_branch,
        })
    }

    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, VmError> {
        // 检查缓存
        if let Some(ref cache) = self.decode_cache
            && let Some(cached_block) = cache.get(&pc)
        {
            return Ok(cached_block.clone());
        }

        let mut builder = IRBuilder::new(pc);
        let mut current_pc = pc;

        loop {
            let insn = mmu.fetch_insn(current_pc)? as u32;

            // ADD/SUB (immediate)
            // 31 30 29 28 27 26 25 24 23 22 21 ... 10 9 ... 5 4 ... 0
            // sf op  S  1  0  0  0  1  0  sh        imm12    Rn      Rd
            if (insn & 0x1F000000) == 0x11000000 {
                let is_sub = (insn & 0x40000000) != 0;
                let shift = (insn >> 22) & 3;
                let imm12 = (insn >> 10) & 0xFFF;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                let mut imm = imm12 as i64;
                if shift == 1 {
                    imm <<= 12;
                }

                if is_sub {
                    builder.push(IROp::AddImm {
                        dst: rd,
                        src: rn,
                        imm: -imm,
                    });
                } else {
                    builder.push(IROp::AddImm {
                        dst: rd,
                        src: rn,
                        imm,
                    });
                }
                current_pc = GuestAddr(current_pc.0 + 4);
                continue;
            }

            // MOVN
            // 31 30 29 28 27 26 25 24 23 22 21 20 ... 5 4 ... 0
            // sf  0  0  1  0  0  1  0  1 hw   imm16     Rd
            if (insn & 0x7F800000) == 0x12800000 {
                let sf = (insn >> 31) & 1;
                let hw = (insn >> 21) & 3;
                let imm16 = (insn >> 5) & 0xFFFF;
                let rd = insn & 0x1F;
                let shift = hw * 16;
                let val = (imm16 as u64) << shift;
                let mut result = !val;
                if sf == 0 {
                    result &= 0xFFFFFFFF;
                }
                builder.push(IROp::MovImm {
                    dst: rd,
                    imm: result,
                });
                current_pc += 4;
                continue;
            }

            // MOVZ
            // 31 30 29 28 27 26 25 24 23 22 21 20 ... 5 4 ... 0
            // sf  1  0  1  0  0  1  0  1 hw   imm16     Rd
            if (insn & 0x7F800000) == 0x52800000 {
                let hw = (insn >> 21) & 3;
                let imm16 = (insn >> 5) & 0xFFFF;
                let rd = insn & 0x1F;
                let val = (imm16 as u64) << (hw * 16);
                builder.push(IROp::MovImm { dst: rd, imm: val });
                current_pc += 4;
                continue;
            }

            // LDR/STR (Unsigned Immediate)
            if (insn & 0x3F000000) == 0x39000000
                || (insn & 0xFF000000) == 0xF9000000
                || (insn & 0xFFC00000) == 0xF9400000
            {
                let size_bits = (insn >> 30) & 0x3;
                let is_load = (insn & 0x00400000) != 0;
                let imm12 = (insn >> 10) & 0xFFF;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let size = match size_bits {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => 8,
                };
                let offset = (imm12 as i64) * (size as i64);

                if is_load {
                    builder.push(IROp::Load {
                        dst: rt,
                        base: rn,
                        offset,
                        size,
                        flags: MemFlags::default(),
                    });
                } else {
                    builder.push(IROp::Store {
                        src: rt,
                        base: rn,
                        offset,
                        size,
                        flags: MemFlags::default(),
                    });
                }
                current_pc += 4;
                continue;
            }

            // B (Unconditional Branch)
            // 0001 01...
            if (insn & 0xFC000000) == 0x14000000 {
                let imm26 = insn & 0x03FFFFFF;
                let offset = ((imm26 << 6) as i32 >> 6) as i64 * 4;
                let target = GuestAddr(current_pc.0.wrapping_add(offset as u64));
                builder.set_term(Terminator::Jmp { target });
                break;
            }

            // BL (Branch with Link)
            // 1001 01...
            if (insn & 0xFC000000) == 0x94000000 {
                let imm26 = insn & 0x03FFFFFF;
                let offset = ((imm26 << 6) as i32 >> 6) as i64 * 4;
                let target = GuestAddr(current_pc.0.wrapping_add(offset as u64));
                // Link register is x30
                builder.push(IROp::MovImm {
                    dst: 30,
                    imm: current_pc.0 + 4,
                });
                builder.set_term(Terminator::Jmp { target });
                break;
            }

            // BR (Branch to Register)
            // 1101 0110 0001 1111 0000 00... Rn ...
            if (insn & 0xFFFFFC1F) == 0xD61F0000 {
                let rn = (insn >> 5) & 0x1F;
                builder.set_term(Terminator::JmpReg {
                    base: rn,
                    offset: 0,
                });
                break;
            }

            // RET
            // 1101 0110 0101 1111 0000 00...
            if (insn & 0xFFFFFC1F) == 0xD65F0000 {
                builder.set_term(Terminator::Ret);
                break;
            }

            // CBZ/CBNZ (Compare and Branch)
            let res = {
                let top = insn & 0xFF000000;
                top == 0x34000000 || top == 0x35000000 || top == 0xB4000000 || top == 0xB5000000
            };
            if res {
                let top = insn & 0xFF000000;
                let is_nz = top == 0x35000000 || top == 0xB5000000;
                let imm19 = (insn >> 5) & 0x7FFFF;
                let off = ((imm19 << 13) as i32 >> 13) as i64 * 4;
                let target = GuestAddr(current_pc.0.wrapping_add(off as u64));
                let rt = insn & 0x1F;
                let zero = 100;
                builder.push(IROp::MovImm { dst: zero, imm: 0 });
                let cond = 101;
                if is_nz {
                    builder.push(IROp::CmpNe {
                        dst: cond,
                        lhs: rt,
                        rhs: zero,
                    });
                } else {
                    builder.push(IROp::CmpEq {
                        dst: cond,
                        lhs: rt,
                        rhs: zero,
                    });
                }
                builder.set_term(Terminator::CondJmp {
                    cond,
                    target_true: target,
                    target_false: current_pc + 4,
                });
                break;
            }

            // B.cond (Conditional Branch)
            if (insn & 0xFF000000) == 0x54000000 {
                let imm19 = (insn >> 5) & 0x7FFFF;
                let off = ((imm19 << 13) as i32 >> 13) as i64 * 4;
                let target = GuestAddr(current_pc.0.wrapping_add(off as u64));
                let cond_reg = 106;
                builder.set_term(Terminator::CondJmp {
                    cond: cond_reg,
                    target_true: target,
                    target_false: current_pc + 4,
                });
                break;
            }

            // Logical (shifted register) AND/ORR/EOR
            // sf op 0 1 0 1 0 1 0 0 ...
            if (insn & 0x1F000000) == 0x0A000000 {
                let op = (insn >> 29) & 3;
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                match op {
                    0 => builder.push(IROp::And {
                        dst: rd,
                        src1: rn,
                        src2: rm,
                    }), // AND
                    1 => builder.push(IROp::Or {
                        dst: rd,
                        src1: rn,
                        src2: rm,
                    }), // ORR
                    2 => builder.push(IROp::Xor {
                        dst: rd,
                        src1: rn,
                        src2: rm,
                    }), // EOR
                    3 => builder.push(IROp::And {
                        dst: rd,
                        src1: rn,
                        src2: rm,
                    }), // ANDS (flags ignored)
                    _ => {}
                }
                current_pc += 4;
                continue;
            }

            // ADRP
            if (insn & 0x9F000000) == 0x90000000 {
                let immlo = (insn >> 29) & 3;
                let immhi = (insn >> 5) & 0x7FFFF;
                let rd = insn & 0x1F;

                let imm_val = ((immhi << 2) | immlo) as i32;
                let imm_val = (imm_val << 11) >> 11; // Sign extend 21 bits
                let offset = (imm_val as i64) << 12;

                let base = current_pc & !0xFFF;
                let result = base.wrapping_add(offset as u64);

                builder.push(IROp::MovImm {
                    dst: rd,
                    imm: result,
                });
                current_pc += 4;
                continue;
            }

            // MUL/MADD/MSUB (Data-processing 3 source)
            if (insn & 0x1F000000) == 0x1B000000 {
                let op54 = (insn >> 29) & 3;
                let rm = (insn >> 16) & 0x1F;
                let ra = (insn >> 10) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                if op54 == 0 {
                    // MADD/MUL: Rd = Ra + Rn * Rm
                    if ra == 31 {
                        // MUL: Rd = Rn * Rm
                        builder.push(IROp::Mul {
                            dst: rd,
                            src1: rn,
                            src2: rm,
                        });
                    } else {
                        // MADD: Rd = Ra + Rn * Rm
                        let tmp = 32; // temporary register
                        builder.push(IROp::Mul {
                            dst: tmp,
                            src1: rn,
                            src2: rm,
                        });
                        builder.push(IROp::Add {
                            dst: rd,
                            src1: ra,
                            src2: tmp,
                        });
                    }
                } else if op54 == 1 {
                    // MSUB: Rd = Ra - Rn * Rm
                    let tmp = 32;
                    builder.push(IROp::Mul {
                        dst: tmp,
                        src1: rn,
                        src2: rm,
                    });
                    builder.push(IROp::Sub {
                        dst: rd,
                        src1: ra,
                        src2: tmp,
                    });
                }
                current_pc += 4;
                continue;
            }

            // SDIV/UDIV
            if (insn & 0x1FE0FC00) == 0x1AC00800 {
                let sf = (insn >> 31) & 1;
                let is_signed = (insn & 0x00000400) == 0;
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                // 根据 sf 位确定操作数大小（0 表示 32 位，1 表示 64 位）
                let _op_size = if sf == 1 { 8 } else { 4 };

                builder.push(IROp::Div {
                    dst: rd,
                    src1: rn,
                    src2: rm,
                    signed: is_signed,
                });
                current_pc += 4;
                continue;
            }

            // Atomic memory operations (LSE)
            // LDADD, LDCLR, LDEOR, LDSET, etc.
            if (insn & 0x3F200C00) == 0x38200000 {
                let size = (insn >> 30) & 3;
                let opc = (insn >> 12) & 7;
                let rs = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let mem_size = match size {
                    0 => 1, // byte
                    1 => 2, // halfword
                    2 => 4, // word
                    3 => 8, // doubleword
                    _ => 4,
                };

                let op = match opc {
                    0 => vm_ir::AtomicOp::Add,  // LDADD
                    1 => vm_ir::AtomicOp::And,  // LDCLR (implemented as AND with complement)
                    2 => vm_ir::AtomicOp::Xor,  // LDEOR
                    3 => vm_ir::AtomicOp::Or,   // LDSET
                    4 => vm_ir::AtomicOp::MaxS, // LDSMAX
                    5 => vm_ir::AtomicOp::MinS, // LDSMIN
                    6 => vm_ir::AtomicOp::Max,  // LDUMAX
                    7 => vm_ir::AtomicOp::Min,  // LDUMIN
                    _ => vm_ir::AtomicOp::Xchg,
                };
                let acq = ((insn >> 23) & 1) != 0;
                let rel = ((insn >> 22) & 1) != 0;
                let order = if acq && rel {
                    vm_ir::MemOrder::AcqRel
                } else if acq {
                    vm_ir::MemOrder::Acquire
                } else if rel {
                    vm_ir::MemOrder::Release
                } else {
                    vm_ir::MemOrder::None
                };
                let flags = MemFlags {
                    atomic: true,
                    order,
                    ..MemFlags::default()
                };
                builder.push(IROp::AtomicRMWOrder {
                    dst: rt,
                    base: rn,
                    src: rs,
                    op,
                    size: mem_size,
                    flags,
                });
                current_pc += 4;
                continue;
            }

            // CAS/CASA/CASAL (LSE compare-and-swap)
            if (insn & 0x3F200C00) == 0x38200400 {
                let size_bits = (insn >> 30) & 3;
                let rs = (insn >> 16) & 0x1F; // expected
                let rn = (insn >> 5) & 0x1F; // address
                let rt = insn & 0x1F; // new value, also receives old

                let size = match size_bits {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => 4,
                };
                let acq = ((insn >> 23) & 1) != 0;
                let rel = ((insn >> 22) & 1) != 0;
                let order = if acq && rel {
                    vm_ir::MemOrder::AcqRel
                } else if acq {
                    vm_ir::MemOrder::Acquire
                } else if rel {
                    vm_ir::MemOrder::Release
                } else {
                    vm_ir::MemOrder::None
                };
                let flags = MemFlags {
                    atomic: true,
                    order,
                    ..MemFlags::default()
                };

                builder.push(IROp::AtomicCmpXchgOrder {
                    dst: rt,
                    base: rn,
                    expected: rs,
                    new: rt,
                    size,
                    flags,
                });
                current_pc += 4;
                continue;
            }

            // ADD/SUB (shifted register)
            if (insn & 0x1F200000) == 0x0B000000 {
                let is_sub = (insn & 0x40000000) != 0;
                let sets_flags = (insn & 0x20000000) != 0; // S bit
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                if is_sub {
                    builder.push(IROp::Sub {
                        dst: rd,
                        src1: rn,
                        src2: rm,
                    });
                } else {
                    builder.push(IROp::Add {
                        dst: rd,
                        src1: rn,
                        src2: rm,
                    });
                }

                // Update flags if S bit is set
                if sets_flags {
                    let pstate_reg = 17;
                    let zero = 200;
                    let one = 201;
                    builder.push(IROp::MovImm { dst: zero, imm: 0 });
                    builder.push(IROp::MovImm { dst: one, imm: 1 });

                    // Read current PSTATE flags
                    builder.push(IROp::ReadPstateFlags { dst: pstate_reg });

                    // Update Z flag (result == 0)
                    let z_flag = 202;
                    builder.push(IROp::CmpEq {
                        dst: z_flag,
                        lhs: rd,
                        rhs: zero,
                    });

                    // Update N flag (result < 0, sign bit)
                    let n_flag = 203;
                    let sign_mask = 0x8000000000000000u64;
                    let sign_mask_reg0 = 270;
                    builder.push(IROp::MovImm {
                        dst: sign_mask_reg0,
                        imm: sign_mask,
                    });
                    builder.push(IROp::And {
                        dst: n_flag,
                        src1: rd,
                        src2: sign_mask_reg0,
                    });
                    builder.push(IROp::SrlImm {
                        dst: n_flag,
                        src: n_flag,
                        sh: 63,
                    });

                    // Update C flag (carry/borrow) - simplified
                    let c_flag = 204;
                    if is_sub {
                        // For subtraction: C = !borrow = (Rn >= Rm)
                        builder.push(IROp::CmpGeU {
                            dst: c_flag,
                            lhs: rn,
                            rhs: rm,
                        });
                    } else {
                        // For addition: C = carry = (result < Rn)
                        builder.push(IROp::CmpLtU {
                            dst: c_flag,
                            lhs: rd,
                            rhs: rn,
                        });
                    }

                    // Update V flag (overflow) - simplified
                    let v_flag = 205;
                    builder.push(IROp::MovImm {
                        dst: v_flag,
                        imm: 0,
                    }); // Simplified

                    // Combine flags into PSTATE: N(31), Z(30), C(29), V(28)
                    let pstate_value = 206;
                    builder.push(IROp::SllImm {
                        dst: pstate_value,
                        src: n_flag,
                        sh: 31,
                    });
                    let z_shifted = 207;
                    builder.push(IROp::SllImm {
                        dst: z_shifted,
                        src: z_flag,
                        sh: 30,
                    });
                    builder.push(IROp::Or {
                        dst: pstate_value,
                        src1: pstate_value,
                        src2: z_shifted,
                    });
                    let c_shifted = 208;
                    builder.push(IROp::SllImm {
                        dst: c_shifted,
                        src: c_flag,
                        sh: 29,
                    });
                    builder.push(IROp::Or {
                        dst: pstate_value,
                        src1: pstate_value,
                        src2: c_shifted,
                    });
                    let v_shifted = 209;
                    builder.push(IROp::SllImm {
                        dst: v_shifted,
                        src: v_flag,
                        sh: 28,
                    });
                    builder.push(IROp::Or {
                        dst: pstate_value,
                        src1: pstate_value,
                        src2: v_shifted,
                    });
                    builder.push(IROp::WritePstateFlags { src: pstate_value });
                }

                current_pc += 4;
                continue;
            }

            // ADCS/SBCS (Add/Subtract with Carry/Subtract, setting flags)
            // sf 0 0 1 1 0 0 0 0 op S ...
            if (insn & 0x1F200000) == 0x1A000000 && (insn & 0x20000000) != 0 {
                let is_sub = (insn & 0x40000000) != 0;
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                // Load carry flag from PSTATE
                let pstate_reg = 17;
                let cf_mask = 1u64 << 29;
                let cf_mask_reg = 260;
                builder.push(IROp::MovImm {
                    dst: cf_mask_reg,
                    imm: cf_mask,
                });
                let carry_flag = 250;
                builder.push(IROp::ReadPstateFlags { dst: pstate_reg });
                builder.push(IROp::And {
                    dst: carry_flag,
                    src1: pstate_reg,
                    src2: cf_mask_reg,
                });
                builder.push(IROp::SrlImm {
                    dst: carry_flag,
                    src: carry_flag,
                    sh: 29,
                });

                if is_sub {
                    // SBCS: Rd = Rn - Rm - !C, set flags
                    let tmp = 251;
                    builder.push(IROp::Sub {
                        dst: tmp,
                        src1: rn,
                        src2: rm,
                    });
                    let one = 252;
                    builder.push(IROp::MovImm { dst: one, imm: 1 });
                    builder.push(IROp::Sub {
                        dst: tmp,
                        src1: tmp,
                        src2: one,
                    });
                    builder.push(IROp::Add {
                        dst: rd,
                        src1: tmp,
                        src2: carry_flag,
                    });
                } else {
                    // ADCS: Rd = Rn + Rm + C, set flags
                    let tmp = 251;
                    builder.push(IROp::Add {
                        dst: tmp,
                        src1: rn,
                        src2: rm,
                    });
                    builder.push(IROp::Add {
                        dst: rd,
                        src1: tmp,
                        src2: carry_flag,
                    });
                }

                // Update flags (same as ADD/SUB with S bit)
                let zero = 253;
                let one = 254;
                builder.push(IROp::MovImm { dst: zero, imm: 0 });
                builder.push(IROp::MovImm { dst: one, imm: 1 });
                let z_flag = 255;
                builder.push(IROp::CmpEq {
                    dst: z_flag,
                    lhs: rd,
                    rhs: zero,
                });
                let n_flag = 256;
                let sign_mask = 0x8000000000000000u64;
                let sign_mask_reg = 262;
                builder.push(IROp::MovImm {
                    dst: sign_mask_reg,
                    imm: sign_mask,
                });
                builder.push(IROp::And {
                    dst: n_flag,
                    src1: rd,
                    src2: sign_mask_reg,
                });
                builder.push(IROp::SrlImm {
                    dst: n_flag,
                    src: n_flag,
                    sh: 63,
                });
                let c_flag = 257;
                if is_sub {
                    builder.push(IROp::CmpGeU {
                        dst: c_flag,
                        lhs: rn,
                        rhs: rm,
                    });
                } else {
                    builder.push(IROp::CmpLtU {
                        dst: c_flag,
                        lhs: rd,
                        rhs: rn,
                    });
                }
                let v_flag = 258;
                builder.push(IROp::MovImm {
                    dst: v_flag,
                    imm: 0,
                });
                let pstate_value = 259;
                builder.push(IROp::SllImm {
                    dst: pstate_value,
                    src: n_flag,
                    sh: 31,
                });
                let z_shifted = 260;
                builder.push(IROp::SllImm {
                    dst: z_shifted,
                    src: z_flag,
                    sh: 30,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: z_shifted,
                });
                let c_shifted = 261;
                builder.push(IROp::SllImm {
                    dst: c_shifted,
                    src: c_flag,
                    sh: 29,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: c_shifted,
                });
                let v_shifted = 262;
                builder.push(IROp::SllImm {
                    dst: v_shifted,
                    src: v_flag,
                    sh: 28,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: v_shifted,
                });
                builder.push(IROp::WritePstateFlags { src: pstate_value });

                current_pc += 4;
                continue;
            }

            // NEG/NEGS (Negate)
            // sf 1 1 0 1 0 1 0 0 0 0 0 0 ...
            if (insn & 0xFFE00000) == 0x4B000000 || (insn & 0xFFE00000) == 0x4B200000 {
                let sets_flags = (insn & 0x20000000) != 0; // S bit
                let rm = (insn >> 16) & 0x1F;
                let rd = insn & 0x1F;
                let zero = 200;
                builder.push(IROp::MovImm { dst: zero, imm: 0 });
                builder.push(IROp::Sub {
                    dst: rd,
                    src1: zero,
                    src2: rm,
                });

                if sets_flags {
                    // Update flags
                    let pstate_reg = 17;
                    let one = 201;
                    builder.push(IROp::MovImm { dst: one, imm: 1 });

                    // Read current PSTATE flags
                    builder.push(IROp::ReadPstateFlags { dst: pstate_reg });

                    let z_flag = 202;
                    builder.push(IROp::CmpEq {
                        dst: z_flag,
                        lhs: rd,
                        rhs: zero,
                    });
                    let n_flag = 203;
                    let sign_mask = 0x8000000000000000u64;
                    let sign_mask_reg = 261;
                    builder.push(IROp::MovImm {
                        dst: sign_mask_reg,
                        imm: sign_mask,
                    });
                    builder.push(IROp::And {
                        dst: n_flag,
                        src1: rd,
                        src2: sign_mask_reg,
                    });
                    builder.push(IROp::SrlImm {
                        dst: n_flag,
                        src: n_flag,
                        sh: 63,
                    });
                    let c_flag = 204;
                    builder.push(IROp::MovImm {
                        dst: c_flag,
                        imm: 1,
                    }); // NEG always sets C
                    let v_flag = 205;
                    builder.push(IROp::MovImm {
                        dst: v_flag,
                        imm: 0,
                    });
                    let pstate_value = 206;
                    builder.push(IROp::SllImm {
                        dst: pstate_value,
                        src: n_flag,
                        sh: 31,
                    });
                    let z_shifted = 207;
                    builder.push(IROp::SllImm {
                        dst: z_shifted,
                        src: z_flag,
                        sh: 30,
                    });
                    builder.push(IROp::Or {
                        dst: pstate_value,
                        src1: pstate_value,
                        src2: z_shifted,
                    });
                    let c_shifted = 208;
                    builder.push(IROp::SllImm {
                        dst: c_shifted,
                        src: c_flag,
                        sh: 29,
                    });
                    builder.push(IROp::Or {
                        dst: pstate_value,
                        src1: pstate_value,
                        src2: c_shifted,
                    });
                    let v_shifted = 209;
                    builder.push(IROp::SllImm {
                        dst: v_shifted,
                        src: v_flag,
                        sh: 28,
                    });
                    builder.push(IROp::Or {
                        dst: pstate_value,
                        src1: pstate_value,
                        src2: v_shifted,
                    });
                    builder.push(IROp::WritePstateFlags { src: pstate_value });
                }

                current_pc += 4;
                continue;
            }

            // CMP/CMN (Compare/Negative Compare)
            // CMP is SUB with zero destination, CMN is ADD with zero destination
            // sf 1 1 0 1 0 0 0 0 0 0 0 0 ... (CMP)
            // sf 1 0 1 1 0 0 0 0 0 0 0 0 ... (CMN)
            if (insn & 0xFFE00000) == 0x4B000000 && (insn & 0x1FFC00) == 0x1F0000 {
                // CMP: Rn - Rm (SUB with zero destination)
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let tmp = 200;
                builder.push(IROp::Sub {
                    dst: tmp,
                    src1: rn,
                    src2: rm,
                });

                // Update flags
                let pstate_reg = 17;
                let zero = 201;
                let one = 202;
                builder.push(IROp::MovImm { dst: zero, imm: 0 });

                // Read current PSTATE flags
                builder.push(IROp::ReadPstateFlags { dst: pstate_reg });
                builder.push(IROp::MovImm { dst: one, imm: 1 });
                let z_flag = 203;
                builder.push(IROp::CmpEq {
                    dst: z_flag,
                    lhs: tmp,
                    rhs: zero,
                });
                let n_flag = 204;
                let sign_mask = 0x8000000000000000u64;
                let sign_mask_reg = 263;
                builder.push(IROp::MovImm {
                    dst: sign_mask_reg,
                    imm: sign_mask,
                });
                builder.push(IROp::And {
                    dst: n_flag,
                    src1: tmp,
                    src2: sign_mask_reg,
                });
                builder.push(IROp::SrlImm {
                    dst: n_flag,
                    src: n_flag,
                    sh: 63,
                });
                let c_flag = 205;
                builder.push(IROp::CmpGeU {
                    dst: c_flag,
                    lhs: rn,
                    rhs: rm,
                });
                let v_flag = 206;
                builder.push(IROp::MovImm {
                    dst: v_flag,
                    imm: 0,
                });
                let pstate_value = 207;
                builder.push(IROp::SllImm {
                    dst: pstate_value,
                    src: n_flag,
                    sh: 31,
                });
                let z_shifted = 208;
                builder.push(IROp::SllImm {
                    dst: z_shifted,
                    src: z_flag,
                    sh: 30,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: z_shifted,
                });
                let c_shifted = 209;
                builder.push(IROp::SllImm {
                    dst: c_shifted,
                    src: c_flag,
                    sh: 29,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: c_shifted,
                });
                let v_shifted = 210;
                builder.push(IROp::SllImm {
                    dst: v_shifted,
                    src: v_flag,
                    sh: 28,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: v_shifted,
                });
                builder.push(IROp::WritePstateFlags { src: pstate_value });

                current_pc += 4;
                continue;
            }

            if (insn & 0xFFE00000) == 0x2B000000 && (insn & 0x1FFC00) == 0x1F0000 {
                // CMN: Rn + Rm (ADD with zero destination)
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let tmp = 200;
                builder.push(IROp::Add {
                    dst: tmp,
                    src1: rn,
                    src2: rm,
                });

                // Update flags (same as CMP)
                let pstate_reg = 17;
                let zero = 201;
                let one = 202;
                builder.push(IROp::MovImm { dst: zero, imm: 0 });

                // Read current PSTATE flags
                builder.push(IROp::ReadPstateFlags { dst: pstate_reg });
                builder.push(IROp::MovImm { dst: one, imm: 1 });
                let z_flag = 203;
                builder.push(IROp::CmpEq {
                    dst: z_flag,
                    lhs: tmp,
                    rhs: zero,
                });
                let n_flag = 204;
                let sign_mask = 0x8000000000000000u64;
                let sign_mask_reg = 264;
                builder.push(IROp::MovImm {
                    dst: sign_mask_reg,
                    imm: sign_mask,
                });
                builder.push(IROp::And {
                    dst: n_flag,
                    src1: tmp,
                    src2: sign_mask_reg,
                });
                builder.push(IROp::SrlImm {
                    dst: n_flag,
                    src: n_flag,
                    sh: 63,
                });
                let c_flag = 205;
                builder.push(IROp::CmpLtU {
                    dst: c_flag,
                    lhs: tmp,
                    rhs: rn,
                });
                let v_flag = 206;
                builder.push(IROp::MovImm {
                    dst: v_flag,
                    imm: 0,
                });
                let pstate_value = 207;
                builder.push(IROp::SllImm {
                    dst: pstate_value,
                    src: n_flag,
                    sh: 31,
                });
                let z_shifted = 208;
                builder.push(IROp::SllImm {
                    dst: z_shifted,
                    src: z_flag,
                    sh: 30,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: z_shifted,
                });
                let c_shifted = 209;
                builder.push(IROp::SllImm {
                    dst: c_shifted,
                    src: c_flag,
                    sh: 29,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: c_shifted,
                });
                let v_shifted = 210;
                builder.push(IROp::SllImm {
                    dst: v_shifted,
                    src: v_flag,
                    sh: 28,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: v_shifted,
                });
                builder.push(IROp::WritePstateFlags { src: pstate_value });

                current_pc += 4;
                continue;
            }

            // TST (Test - AND with zero destination)
            // sf 0 1 1 0 1 0 1 0 0 0 0 0 ...
            if (insn & 0x1FE00000) == 0x0A000000 && (insn & 0x1FFC00) == 0x1F0000 {
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let tmp = 200;
                builder.push(IROp::And {
                    dst: tmp,
                    src1: rn,
                    src2: rm,
                });

                // Update flags (Z and N)
                let pstate_reg = 17;
                let zero = 201;
                let one = 202;
                builder.push(IROp::MovImm { dst: zero, imm: 0 });

                // Read current PSTATE flags
                builder.push(IROp::ReadPstateFlags { dst: pstate_reg });
                builder.push(IROp::MovImm { dst: one, imm: 1 });
                let z_flag = 203;
                builder.push(IROp::CmpEq {
                    dst: z_flag,
                    lhs: tmp,
                    rhs: zero,
                });
                let n_flag = 204;
                let sign_mask = 0x8000000000000000u64;
                let sign_mask_reg = 265;
                builder.push(IROp::MovImm {
                    dst: sign_mask_reg,
                    imm: sign_mask,
                });
                builder.push(IROp::And {
                    dst: n_flag,
                    src1: tmp,
                    src2: sign_mask_reg,
                });
                builder.push(IROp::SrlImm {
                    dst: n_flag,
                    src: n_flag,
                    sh: 63,
                });
                let c_flag = 205;
                builder.push(IROp::MovImm {
                    dst: c_flag,
                    imm: 0,
                }); // C cleared
                let v_flag = 206;
                builder.push(IROp::MovImm {
                    dst: v_flag,
                    imm: 0,
                }); // V cleared
                let pstate_value = 207;
                builder.push(IROp::SllImm {
                    dst: pstate_value,
                    src: n_flag,
                    sh: 31,
                });
                let z_shifted = 208;
                builder.push(IROp::SllImm {
                    dst: z_shifted,
                    src: z_flag,
                    sh: 30,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: z_shifted,
                });
                let c_shifted = 209;
                builder.push(IROp::SllImm {
                    dst: c_shifted,
                    src: c_flag,
                    sh: 29,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: c_shifted,
                });
                let v_shifted = 210;
                builder.push(IROp::SllImm {
                    dst: v_shifted,
                    src: v_flag,
                    sh: 28,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: v_shifted,
                });
                builder.push(IROp::WritePstateFlags { src: pstate_value });

                current_pc += 4;
                continue;
            }

            // NEON (Advanced SIMD) instructions
            // Check for NEON instruction pattern: bits [28:25] = 0b0111 or 0b0101
            if (insn >> 25) & 0x7 == 0b001 || (insn >> 25) & 0x7 == 0b011 {
                // Advanced SIMD/NEON instruction
                let op0 = (insn >> 28) & 0xF;
                let op1 = (insn >> 23) & 0x3;
                let op2 = (insn >> 19) & 0x3;
                let op3 = (insn >> 16) & 0x7;
                let op4 = (insn >> 11) & 0x1F;

                // NEON ADD (vector)
                // 0 1 1 1 0 0 0 0 0 0 0 0 ...
                if op0 == 0b0111 && op1 == 0b00 && op2 == 0b00 && op3 == 0b00 && op4 == 0b00000 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1, // 8-bit
                        1 => 2, // 16-bit
                        2 => 4, // 32-bit
                        3 => 8, // 64-bit
                        _ => 4,
                    };

                    // Map to vector register space (NEON registers are V0-V31, map to 32-63)
                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    builder.push(IROp::VecAdd {
                        dst: vd,
                        src1: vn,
                        src2: vm,
                        element_size,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON SUB (vector)
                if op0 == 0b0111 && op1 == 0b00 && op2 == 0b00 && op3 == 0b00 && op4 == 0b00001 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    builder.push(IROp::VecSub {
                        dst: vd,
                        src1: vn,
                        src2: vm,
                        element_size,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON MUL (vector)
                if op0 == 0b0111 && op1 == 0b00 && op2 == 0b00 && op3 == 0b00 && op4 == 0b00010 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    builder.push(IROp::VecMul {
                        dst: vd,
                        src1: vn,
                        src2: vm,
                        element_size,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON FADD (floating-point vector add)
                // 0 1 1 0 1 1 1 0 0 0 0 0 ...
                if op0 == 0b0110 && op1 == 0b11 && op2 == 0b10 && op3 == 0b00 && op4 == 0b00000 {
                    let size = (insn >> 22) & 0x1; // 0 = 32-bit, 1 = 64-bit
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    // Use VecAdd with floating-point element size
                    builder.push(IROp::VecAdd {
                        dst: vd,
                        src1: vn,
                        src2: vm,
                        element_size: if size == 0 { 4 } else { 8 },
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON MLA (Multiply-Add)
                // 0 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 ...
                // op0=0b0111, op1=0b00, op2=0b00, op3=0b00, op4=0b01000
                if op0 == 0b0111 && op1 == 0b00 && op2 == 0b00 && op3 == 0b00 && op4 == 0b01000 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    // MLA: Vd = Vd + (Vn * Vm)
                    // First multiply Vn * Vm
                    let mul_result = 100;
                    builder.push(IROp::VecMul {
                        dst: mul_result,
                        src1: vn,
                        src2: vm,
                        element_size,
                    });
                    // Then add to Vd
                    builder.push(IROp::VecAdd {
                        dst: vd,
                        src1: vd,
                        src2: mul_result,
                        element_size,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON MLS (Multiply-Subtract)
                // 0 1 1 1 0 0 0 0 0 0 0 0 1 0 0 1 ...
                // op0=0b0111, op1=0b00, op2=0b00, op3=0b00, op4=0b01001
                if op0 == 0b0111 && op1 == 0b00 && op2 == 0b00 && op3 == 0b00 && op4 == 0b01001 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    // MLS: Vd = Vd - (Vn * Vm)
                    // First multiply Vn * Vm
                    let mul_result = 100;
                    builder.push(IROp::VecMul {
                        dst: mul_result,
                        src1: vn,
                        src2: vm,
                        element_size,
                    });
                    // Then subtract from Vd
                    builder.push(IROp::VecSub {
                        dst: vd,
                        src1: vd,
                        src2: mul_result,
                        element_size,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON CMEQ (Compare Equal)
                // 0 1 1 1 0 0 1 0 1 0 0 0 0 0 0 0 ...
                // op0=0b0111, op1=0b00, op2=0b10, op3=0b101, op4=0b00000
                if op0 == 0b0111 && op1 == 0b00 && op2 == 0b10 && op3 == 0b101 && op4 == 0b00000 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    // CMEQ: Compare equal, result is all 1s if equal, all 0s otherwise
                    // Use vector comparison - simplified as VecAdd with comparison semantics
                    // In real implementation, this would need element-wise comparison
                    let cmp_result = 101;
                    builder.push(IROp::CmpEq {
                        dst: cmp_result,
                        lhs: vn,
                        rhs: vm,
                    });
                    builder.push(IROp::VecAdd {
                        dst: vd,
                        src1: cmp_result,
                        src2: cmp_result,
                        element_size,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON CMGT (Compare Greater Than, signed)
                // 0 1 1 1 0 0 1 0 0 0 1 0 0 0 0 0 ...
                // op0=0b0111, op1=0b00, op2=0b10, op3=0b001, op4=0b00000
                if op0 == 0b0111 && op1 == 0b00 && op2 == 0b10 && op3 == 0b001 && op4 == 0b00000 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    let cmp_result = 101;
                    builder.push(IROp::CmpLt {
                        dst: cmp_result,
                        lhs: vm,
                        rhs: vn,
                    }); // GT = !(Vm >= Vn) = (Vn > Vm)
                    builder.push(IROp::VecAdd {
                        dst: vd,
                        src1: cmp_result,
                        src2: cmp_result,
                        element_size,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON CMGE (Compare Greater Than or Equal, signed)
                // 0 1 1 1 0 0 1 0 0 0 1 1 0 0 0 0 ...
                // op0=0b0111, op1=0b00, op2=0b10, op3=0b001, op4=0b00011
                if op0 == 0b0111 && op1 == 0b00 && op2 == 0b10 && op3 == 0b001 && op4 == 0b00011 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    let cmp_result = 101;
                    builder.push(IROp::CmpGe {
                        dst: cmp_result,
                        lhs: vn,
                        rhs: vm,
                    });
                    builder.push(IROp::VecAdd {
                        dst: vd,
                        src1: cmp_result,
                        src2: cmp_result,
                        element_size,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON CMLT (Compare Less Than, signed)
                // 0 1 1 1 0 0 1 0 0 0 1 0 1 0 0 0 ...
                // op0=0b0111, op1=0b00, op2=0b10, op3=0b001, op4=0b01010
                if op0 == 0b0111 && op1 == 0b00 && op2 == 0b10 && op3 == 0b001 && op4 == 0b01010 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    let cmp_result = 101;
                    builder.push(IROp::CmpLt {
                        dst: cmp_result,
                        lhs: vn,
                        rhs: vm,
                    });
                    builder.push(IROp::VecAdd {
                        dst: vd,
                        src1: cmp_result,
                        src2: cmp_result,
                        element_size,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON CMLE (Compare Less Than or Equal, signed)
                // 0 1 1 1 0 0 1 0 0 0 1 0 1 1 0 0 ...
                // op0=0b0111, op1=0b00, op2=0b10, op3=0b001, op4=0b01110
                if op0 == 0b0111 && op1 == 0b00 && op2 == 0b10 && op3 == 0b001 && op4 == 0b01110 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    let cmp_result = 101;
                    builder.push(IROp::CmpGe {
                        dst: cmp_result,
                        lhs: vm,
                        rhs: vn,
                    }); // LE = (Vm >= Vn)
                    builder.push(IROp::VecAdd {
                        dst: vd,
                        src1: cmp_result,
                        src2: cmp_result,
                        element_size,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON AND (vector bitwise AND)
                // 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 0 ...
                // op0=0b0000, op1=0b11, op2=0b10, op3=0b000, op4=0b00000
                if op0 == 0b0000 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b00000 {
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    builder.push(IROp::And {
                        dst: vd,
                        src1: vn,
                        src2: vm,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON ORR (vector bitwise OR)
                // 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1 ...
                // op0=0b0000, op1=0b11, op2=0b10, op3=0b000, op4=0b00001
                if op0 == 0b0000 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b00001 {
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    builder.push(IROp::Or {
                        dst: vd,
                        src1: vn,
                        src2: vm,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON EOR (vector bitwise XOR)
                // 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1 0 ...
                // op0=0b0000, op1=0b11, op2=0b10, op3=0b000, op4=0b00010
                if op0 == 0b0000 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b00010 {
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    builder.push(IROp::Xor {
                        dst: vd,
                        src1: vn,
                        src2: vm,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON BIC (vector bitwise AND NOT)
                // 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1 1 ...
                // op0=0b0000, op1=0b11, op2=0b10, op3=0b000, op4=0b00011
                if op0 == 0b0000 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b00011 {
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    // BIC: Vd = Vn AND NOT(Vm)
                    let not_vm = 102;
                    builder.push(IROp::Not {
                        dst: not_vm,
                        src: vm,
                    });
                    builder.push(IROp::And {
                        dst: vd,
                        src1: vn,
                        src2: not_vm,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON ORN (vector bitwise OR NOT)
                // 0 0 0 0 1 1 1 0 0 0 0 0 0 0 1 0 0 ...
                // op0=0b0000, op1=0b11, op2=0b10, op3=0b000, op4=0b00100
                if op0 == 0b0000 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b00100 {
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    // ORN: Vd = Vn OR NOT(Vm)
                    let not_vm = 102;
                    builder.push(IROp::Not {
                        dst: not_vm,
                        src: vm,
                    });
                    builder.push(IROp::Or {
                        dst: vd,
                        src1: vn,
                        src2: not_vm,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON SHL (vector shift left)
                // 0 1 0 0 1 1 1 0 0 0 0 0 0 1 0 1 0 ...
                // op0=0b0100, op1=0b11, op2=0b10, op3=0b000, op4=0b01010
                if op0 == 0b0100 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b01010 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    // SHL: Vd = Vn << Vm (element-wise)
                    builder.push(IROp::Sll {
                        dst: vd,
                        src: vn,
                        shreg: vm,
                    });

                    // Log element size for debugging purposes
                    println!("NEON SHL element size: {} bytes", element_size);
                    current_pc += 4;
                    continue;
                }

                // NEON SHR (vector shift right, unsigned)
                // 0 0 1 0 1 1 1 0 0 0 0 0 0 0 0 0 ...
                // op0=0b0010, op1=0b11, op2=0b10, op3=0b000, op4=0b00000
                if op0 == 0b0010 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b00000 {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    builder.push(IROp::Srl {
                        dst: vd,
                        src: vn,
                        shreg: vm,
                    });

                    // Log element size for debugging purposes
                    println!("NEON SHR element size: {} bytes", element_size);
                    current_pc += 4;
                    continue;
                }

                // NEON SSHR (vector signed shift right)
                // 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 0 0 ...
                // op0=0b0000, op1=0b11, op2=0b10, op3=0b000, op4=0b00000 (with size bits)
                if op0 == 0b0000
                    && op1 == 0b11
                    && op2 == 0b10
                    && op3 == 0b000
                    && op4 == 0b00000
                    && (insn >> 22) & 0x3 != 0
                {
                    let _size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    // SSHR: Signed shift right (arithmetic shift)
                    builder.push(IROp::Sra {
                        dst: vd,
                        src: vn,
                        shreg: vm,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON USHR (vector unsigned shift right)
                // 0 0 1 0 1 1 1 0 0 0 0 0 0 0 0 0 0 ...
                // Similar to SHR but with different encoding
                if op0 == 0b0010
                    && op1 == 0b11
                    && op2 == 0b10
                    && op3 == 0b000
                    && op4 == 0b00000
                    && (insn >> 22) & 0x3 != 0
                {
                    let size = (insn >> 22) & 0x3;
                    let rm = (insn >> 16) & 0x1F;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let vd = 32 + rd;
                    let vn = 32 + rn;
                    let vm = 32 + rm;

                    // Validate size - only sizes 0, 1, 2 are valid for SHR
                    if size > 2 {
                        // Invalid size for SHR instruction
                        println!("Invalid size {} for NEON SHR instruction", size);
                    }

                    builder.push(IROp::Srl {
                        dst: vd,
                        src: vn,
                        shreg: vm,
                    });

                    // Log element size for debugging purposes
                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };
                    println!("NEON SHR element size: {} bytes", element_size);
                    current_pc += 4;
                    continue;
                }

                // NEON ADDV (vector add across lanes, reduce to scalar)
                // 0 0 1 1 1 1 1 0 0 0 0 0 1 1 0 0 0 ...
                // op0=0b0011, op1=0b11, op2=0b10, op3=0b000, op4=0b11000
                if op0 == 0b0011 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b11000 {
                    let size = (insn >> 22) & 0x3;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    // Validate size - only sizes 0, 1, 2 are valid for ADDV
                    if size > 2 {
                        // Invalid size for ADDV instruction, could panic or handle as undefined instruction
                        // For now, we'll continue with a default element size
                    }

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8, // Technically invalid for ADDV but we handle it
                        _ => 4,
                    };

                    let vd = rd; // Scalar result
                    let vn = 32 + rn;

                    // ADDV: Sum all elements in vector, store result in scalar register
                    // Simplified: use vector add with reduction semantics
                    // Real implementation would need to sum all elements
                    let temp = 104;
                    builder.push(IROp::VecAdd {
                        dst: temp,
                        src1: vn,
                        src2: vn,
                        element_size,
                    });
                    builder.push(IROp::AddImm {
                        dst: vd,
                        src: temp,
                        imm: 0,
                    }); // Placeholder
                    current_pc += 4;
                    continue;
                }

                // NEON SMAXV (vector signed maximum across lanes)
                // 0 0 1 1 1 1 1 0 0 0 0 0 1 1 0 1 0 ...
                // op0=0b0011, op1=0b11, op2=0b10, op3=0b000, op4=0b11010
                if op0 == 0b0011 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b11010 {
                    let _size = (insn >> 22) & 0x3;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let vd = rd;
                    let vn = 32 + rn;

                    // SMAXV: Find maximum signed element across vector
                    // Simplified implementation
                    let temp = 104;
                    builder.push(IROp::CmpGe {
                        dst: temp,
                        lhs: vn,
                        rhs: vn,
                    }); // Placeholder
                    builder.push(IROp::AddImm {
                        dst: vd,
                        src: temp,
                        imm: 0,
                    });
                    current_pc += 4;
                    continue;
                }

                // NEON SMINV (vector signed minimum across lanes)
                // 0 0 1 1 1 1 1 0 0 0 0 0 1 1 0 1 1 ...
                // op0=0b0011, op1=0b11, op2=0b10, op3=0b000, op4=0b11011
                if op0 == 0b0011 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b11011 {
                    let size = (insn >> 22) & 0x3;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    // Validate size - only sizes 0, 1, 2 are valid for SMINV
                    if size > 2 {
                        // Invalid size for SMINV instruction
                        println!("Invalid size {} for NEON SMINV instruction", size);
                    }

                    let vd = rd;
                    let vn = 32 + rn;

                    // SMINV: Find minimum signed element across vector
                    let temp = 104;
                    builder.push(IROp::CmpLt {
                        dst: temp,
                        lhs: vn,
                        rhs: vn,
                    }); // Placeholder
                    builder.push(IROp::AddImm {
                        dst: vd,
                        src: temp,
                        imm: 0,
                    });

                    // Log element size for debugging purposes
                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };
                    println!("NEON SMINV element size: {} bytes", element_size);
                    current_pc += 4;
                    continue;
                }

                // NEON UMAXV (vector unsigned maximum across lanes)
                // 0 0 1 1 1 1 1 0 0 0 0 0 1 1 1 0 0 ...
                // op0=0b0011, op1=0b11, op2=0b10, op3=0b000, op4=0b11100
                if op0 == 0b0011 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b11100 {
                    let size = (insn >> 22) & 0x3;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    // Validate size - only sizes 0, 1, 2 are valid for UMAXV
                    if size > 2 {
                        // Invalid size for UMAXV instruction
                        println!("Invalid size {} for NEON UMAXV instruction", size);
                    }

                    let vd = rd;
                    let vn = 32 + rn;

                    // UMAXV: Find maximum unsigned element across vector
                    let temp = 104;
                    builder.push(IROp::CmpGeU {
                        dst: temp,
                        lhs: vn,
                        rhs: vn,
                    }); // Placeholder
                    builder.push(IROp::AddImm {
                        dst: vd,
                        src: temp,
                        imm: 0,
                    });

                    // Log element size for debugging purposes
                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };
                    println!("NEON UMAXV element size: {} bytes", element_size);
                    current_pc += 4;
                    continue;
                }

                // NEON UMINV (vector unsigned minimum across lanes)
                // 0 0 1 1 1 1 1 0 0 0 0 0 1 1 1 0 1 ...
                // op0=0b0011, op1=0b11, op2=0b10, op3=0b000, op4=0b11101
                if op0 == 0b0011 && op1 == 0b11 && op2 == 0b10 && op3 == 0b000 && op4 == 0b11101 {
                    let size = (insn >> 22) & 0x3;
                    let rn = (insn >> 5) & 0x1F;
                    let rd = insn & 0x1F;

                    let vd = rd;
                    let vn = 32 + rn;

                    // 根据 size 确定元素大小
                    let element_size = match size {
                        0 => 1, // 8-bit elements
                        1 => 2, // 16-bit elements
                        2 => 4, // 32-bit elements
                        _ => 1, // Default to 8-bit
                    };

                    // 打印元素大小信息（调试用）
                    println!("NEON UMINV element size: {} bytes", element_size);

                    // UMINV: Find minimum unsigned element across vector
                    let temp = 104;
                    builder.push(IROp::CmpLtU {
                        dst: temp,
                        lhs: vn,
                        rhs: vn,
                    }); // Placeholder
                    builder.push(IROp::AddImm {
                        dst: vd,
                        src: temp,
                        imm: 0,
                    });
                    current_pc += 4;
                    continue;
                }
            }

            // NEON LD1/ST1 (Load/Store multiple structures)
            // LD1: 0 0 1 1 0 1 0 0 0 0 0 0 ...
            // ST1: 0 0 1 1 0 1 0 0 0 0 0 0 ... (bit 22 = 0 for load, 1 for store)
            if (insn >> 25) & 0x7 == 0b001 && (insn >> 10) & 0x3 == 0b00 {
                let opcode = (insn >> 12) & 0xF;
                let is_load = (insn & 0x00400000) == 0; // Bit 22

                // LD1/ST1 with single register (Vt)
                if opcode == 0b0111 {
                    let size = (insn >> 30) & 0x3;
                    let rn = (insn >> 5) & 0x1F;
                    let rt = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let vt = 32 + rt; // Vector register

                    if is_load {
                        builder.push(IROp::Load {
                            dst: vt,
                            base: rn,
                            offset: 0,
                            size: element_size as u8,
                            flags: MemFlags::default(),
                        });
                    } else {
                        builder.push(IROp::Store {
                            src: vt,
                            base: rn,
                            offset: 0,
                            size: element_size as u8,
                            flags: MemFlags::default(),
                        });
                    }
                    current_pc += 4;
                    continue;
                }

                // LD1/ST1 with multiple registers (Vt, Vt2, Vt3, Vt4)
                // opcode bits [15:12] determine the number of registers
                if (0b0000..=0b0111).contains(&opcode) {
                    let num_regs = match opcode {
                        0b0000 | 0b0100 => 1, // Single register
                        0b0010 | 0b0110 => 2, // Two registers
                        0b0001 | 0b0101 => 3, // Three registers
                        0b0011 | 0b0111 => 4, // Four registers
                        _ => 1,
                    };

                    let size = (insn >> 30) & 0x3;
                    let rn = (insn >> 5) & 0x1F;
                    let rt = insn & 0x1F;

                    let element_size = match size {
                        0 => 1,
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => 4,
                    };

                    let mut offset = 0i64;
                    for i in 0..num_regs {
                        let vt = 32 + (rt as usize + i) % 32;
                        if is_load {
                            builder.push(IROp::Load {
                                dst: (vt as u32),
                                base: rn,
                                offset,
                                size: element_size as u8,
                                flags: MemFlags::default(),
                            });
                        } else {
                            builder.push(IROp::Store {
                                src: (vt as u32),
                                base: rn,
                                offset,
                                size: element_size as u8,
                                flags: MemFlags::default(),
                            });
                        }
                        offset += element_size as i64;
                    }
                    current_pc += 4;
                    continue;
                }
            }

            // NEON LD1/ST1 with post-index (immediate offset)
            // 0 0 1 1 0 1 0 0 0 0 0 0 ...
            if (insn >> 25) & 0x7 == 0b001 && (insn >> 10) & 0x3 == 0b01 {
                let opcode = (insn >> 12) & 0xF;
                let is_load = (insn & 0x00400000) == 0;
                let imm5 = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let size = (insn >> 30) & 0x3;
                let element_size = match size {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => 4,
                };

                let num_regs = match opcode {
                    0b0000 | 0b0100 => 1,
                    0b0010 | 0b0110 => 2,
                    0b0001 | 0b0101 => 3,
                    0b0011 | 0b0111 => 4,
                    _ => 1,
                };

                let mut offset = 0i64;
                for i in 0..num_regs {
                    let vt = (32 + ((rt as usize + i) % 32)) as u32;
                    if is_load {
                        builder.push(IROp::Load {
                            dst: vt,
                            base: rn,
                            offset,
                            size: element_size as u8,
                            flags: MemFlags::default(),
                        });
                    } else {
                        builder.push(IROp::Store {
                            src: vt,
                            base: rn,
                            offset,
                            size: element_size as u8,
                            flags: MemFlags::default(),
                        });
                    }
                    offset += element_size as i64;
                }

                // Post-index: update base register
                let post_offset = (imm5 as i64) * (num_regs as i64) * (element_size as i64);
                builder.push(IROp::AddImm {
                    dst: rn,
                    src: rn,
                    imm: post_offset,
                });

                current_pc += 4;
                continue;
            }

            // NEON LD2/ST2, LD3/ST3, LD4/ST4 (Load/Store multiple structures with deinterleaving/interleaving)
            // Pattern: 0 0 1 1 0 1 0 0 0 0 0 0 ...
            // Bits [15:13] determine the number of structures (2, 3, or 4)
            // LD2/ST2: opcode bits [15:13] = 0b100 (opcode = 0b1000 or 0b1001)
            // LD3/ST3: opcode bits [15:13] = 0b010 (opcode = 0b0100 or 0b0101)
            // LD4/ST4: opcode bits [15:13] = 0b000 (opcode = 0b0000 or 0b0001)
            if (insn >> 25) & 0x7 == 0b001 && (insn >> 10) & 0x3 == 0b00 {
                let opcode = (insn >> 12) & 0xF;
                let is_load = (insn & 0x00400000) == 0; // Bit 22
                let size = (insn >> 30) & 0x3;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let element_size = match size {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => 4,
                };

                // Check for LD2/ST2, LD3/ST3, LD4/ST4 patterns
                let num_structures = match (opcode >> 2) & 0x3 {
                    0b10 => 2, // LD2/ST2 (opcode bits [15:13] = 0b100)
                    0b01 => 3, // LD3/ST3 (opcode bits [15:13] = 0b010)
                    0b00 => 4, // LD4/ST4 (opcode bits [15:13] = 0b000)
                    _ => 0,
                };

                if num_structures > 0 && (opcode & 0x3) <= 0b01 {
                    // For interleaved structures, load/store vectors with stride
                    // LD2: Load 2 vectors, elements interleaved (stride = 2*element_size)
                    // LD3: Load 3 vectors, elements interleaved (stride = 3*element_size)
                    // LD4: Load 4 vectors, elements interleaved (stride = 4*element_size)
                    let stride = num_structures as i64 * element_size as i64;

                    // Load/store each vector register
                    // In interleaved mode, elements are stored as: V0[0], V1[0], V2[0], V3[0], V0[1], V1[1], ...
                    // So we load with stride
                    for struct_idx in 0..num_structures {
                        let vt = 32 + (rt as usize + struct_idx) % 32;
                        let base_offset = struct_idx as i64 * element_size as i64;

                        // Simplified: load/store entire vector register
                        // Real implementation would need element-wise interleaving
                        if is_load {
                            builder.push(IROp::Load {
                                dst: (vt as u32),
                                base: rn,
                                offset: base_offset,
                                size: element_size as u8,
                                flags: MemFlags::default(),
                            });
                        } else {
                            builder.push(IROp::Store {
                                src: (vt as u32),
                                base: rn,
                                offset: base_offset,
                                size: element_size as u8,
                                flags: MemFlags::default(),
                            });
                        }
                    }

                    // Update base register: stride * number of elements per vector (assume 16-byte vectors)
                    let vector_size = 16i64;
                    let elements_per_vector = vector_size / element_size as i64;
                    let total_offset = stride * elements_per_vector;
                    builder.push(IROp::AddImm {
                        dst: rn,
                        src: rn,
                        imm: total_offset,
                    });

                    current_pc += 4;
                    continue;
                }
            }

            // NEON LD2/ST2, LD3/ST3, LD4/ST4 with post-index
            // Pattern: 0 0 1 1 0 1 0 0 0 0 0 1 ...
            if (insn >> 25) & 0x7 == 0b001 && (insn >> 10) & 0x3 == 0b01 {
                let opcode = (insn >> 12) & 0xF;
                let is_load = (insn & 0x00400000) == 0;
                let imm5 = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let size = (insn >> 30) & 0x3;
                let element_size = match size {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => 4,
                };

                let num_structures = match (opcode >> 2) & 0x3 {
                    0b10 => 2, // LD2/ST2
                    0b01 => 3, // LD3/ST3
                    0b00 => 4, // LD4/ST4
                    _ => 0,
                };

                if num_structures > 0 && (opcode & 0x3) <= 0b01 {
                    let stride = num_structures as i64 * element_size as i64;

                    for struct_idx in 0..num_structures {
                        let vt = 32 + (rt as usize + struct_idx) % 32;
                        let base_offset = struct_idx as i64 * element_size as i64;

                        if is_load {
                            builder.push(IROp::Load {
                                dst: (vt as u32),
                                base: rn,
                                offset: base_offset,
                                size: element_size as u8,
                                flags: MemFlags::default(),
                            });
                        } else {
                            builder.push(IROp::Store {
                                src: (vt as u32),
                                base: rn,
                                offset: base_offset,
                                size: element_size as u8,
                                flags: MemFlags::default(),
                            });
                        }
                    }

                    // Post-index: update base register with immediate offset
                    let post_offset = (imm5 as i64) * stride;
                    builder.push(IROp::AddImm {
                        dst: rn,
                        src: rn,
                        imm: post_offset,
                    });

                    current_pc += 4;
                    continue;
                }
            }

            // ADC/SBC (Add/Subtract with Carry)
            // sf 0 0 1 1 0 0 0 0 op S ...
            if (insn & 0x1F200000) == 0x1A000000 {
                let is_sub = (insn & 0x40000000) != 0;
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                // Load carry flag from PSTATE.C (NZCV flags)
                // PSTATE flags: N (Negative), Z (Zero), C (Carry), V (Overflow)
                // C flag is bit 29 of PSTATE
                let pstate_reg = 17; // PSTATE register (mapped to register 17)
                let cf_mask = 1u64 << 29; // Carry flag bit
                let cf_mask_reg = 260;
                builder.push(IROp::MovImm {
                    dst: cf_mask_reg,
                    imm: cf_mask,
                });
                let carry_flag = 250;
                builder.push(IROp::ReadPstateFlags { dst: pstate_reg });
                builder.push(IROp::And {
                    dst: carry_flag,
                    src1: pstate_reg,
                    src2: cf_mask_reg,
                });
                builder.push(IROp::SrlImm {
                    dst: carry_flag,
                    src: carry_flag,
                    sh: 29,
                }); // Shift to LSB
                let one = 251;
                builder.push(IROp::MovImm { dst: one, imm: 1 });

                if is_sub {
                    // SBC: Rd = Rn - Rm - !C = Rn - Rm - (1 - C) = Rn - Rm - 1 + C
                    let tmp = 252;
                    builder.push(IROp::Sub {
                        dst: tmp,
                        src1: rn,
                        src2: rm,
                    });
                    let one = 253;
                    builder.push(IROp::MovImm { dst: one, imm: 1 });
                    builder.push(IROp::Sub {
                        dst: tmp,
                        src1: tmp,
                        src2: one,
                    });
                    builder.push(IROp::Add {
                        dst: rd,
                        src1: tmp,
                        src2: carry_flag,
                    });
                } else {
                    // ADC: Rd = Rn + Rm + C
                    let tmp = 252;
                    builder.push(IROp::Add {
                        dst: tmp,
                        src1: rn,
                        src2: rm,
                    });
                    builder.push(IROp::Add {
                        dst: rd,
                        src1: tmp,
                        src2: carry_flag,
                    });
                }
                current_pc += 4;
                continue;
            }

            // CSET/CSETM (Conditional Set)
            // sf 0 0 1 1 0 1 0 0 1 0 0 0 ...
            if (insn & 0x1FE00000) == 0x1A800000 && (insn & 0x1FFC00) == 0x1F8400 {
                let cond = (insn >> 12) & 0xF;
                let rd = insn & 0x1F;
                let is_setm = (insn & 0x200) != 0; // M bit

                // Evaluate condition code (reuse logic from CSEL)
                let pstate_reg = 17;
                builder.push(IROp::ReadPstateFlags { dst: pstate_reg });
                let n_flag = 200;
                let z_flag = 201;
                let c_flag = 202;
                let v_flag = 203;
                let one = 204;
                let zero = 205;
                builder.push(IROp::MovImm { dst: one, imm: 1 });
                builder.push(IROp::MovImm { dst: zero, imm: 0 });
                builder.push(IROp::SrlImm {
                    dst: n_flag,
                    src: pstate_reg,
                    sh: 31,
                });
                builder.push(IROp::And {
                    dst: n_flag,
                    src1: n_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: z_flag,
                    src: pstate_reg,
                    sh: 30,
                });
                builder.push(IROp::And {
                    dst: z_flag,
                    src1: z_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: c_flag,
                    src: pstate_reg,
                    sh: 29,
                });
                builder.push(IROp::And {
                    dst: c_flag,
                    src1: c_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: v_flag,
                    src: pstate_reg,
                    sh: 28,
                });
                builder.push(IROp::And {
                    dst: v_flag,
                    src1: v_flag,
                    src2: one,
                });

                let cond_result = 206;
                match cond {
                    0 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: z_flag,
                        rhs: one,
                    }), // EQ
                    1 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: z_flag,
                        rhs: zero,
                    }), // NE
                    2 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: c_flag,
                        rhs: one,
                    }), // CS
                    3 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: c_flag,
                        rhs: zero,
                    }), // CC
                    4 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: one,
                    }), // MI
                    5 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: zero,
                    }), // PL
                    6 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: v_flag,
                        rhs: one,
                    }), // VS
                    7 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: v_flag,
                        rhs: zero,
                    }), // VC
                    8 => {
                        let c_set = 207;
                        let z_clear = 208;
                        builder.push(IROp::CmpEq {
                            dst: c_set,
                            lhs: c_flag,
                            rhs: one,
                        });
                        builder.push(IROp::CmpEq {
                            dst: z_clear,
                            lhs: z_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::And {
                            dst: cond_result,
                            src1: c_set,
                            src2: z_clear,
                        });
                    } // HI
                    9 => {
                        let c_clear = 207;
                        let z_set = 208;
                        builder.push(IROp::CmpEq {
                            dst: c_clear,
                            lhs: c_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::CmpEq {
                            dst: z_set,
                            lhs: z_flag,
                            rhs: one,
                        });
                        builder.push(IROp::Or {
                            dst: cond_result,
                            src1: c_clear,
                            src2: z_set,
                        });
                    } // LS
                    10 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: v_flag,
                    }), // GE
                    11 => builder.push(IROp::CmpNe {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: v_flag,
                    }), // LT
                    12 => {
                        let z_clear = 207;
                        let n_eq_v = 208;
                        builder.push(IROp::CmpEq {
                            dst: z_clear,
                            lhs: z_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::CmpEq {
                            dst: n_eq_v,
                            lhs: n_flag,
                            rhs: v_flag,
                        });
                        builder.push(IROp::And {
                            dst: cond_result,
                            src1: z_clear,
                            src2: n_eq_v,
                        });
                    } // GT
                    13 => {
                        let z_set = 207;
                        let n_ne_v = 208;
                        builder.push(IROp::CmpEq {
                            dst: z_set,
                            lhs: z_flag,
                            rhs: one,
                        });
                        builder.push(IROp::CmpNe {
                            dst: n_ne_v,
                            lhs: n_flag,
                            rhs: v_flag,
                        });
                        builder.push(IROp::Or {
                            dst: cond_result,
                            src1: z_set,
                            src2: n_ne_v,
                        });
                    } // LE
                    14 => builder.push(IROp::MovImm {
                        dst: cond_result,
                        imm: 1,
                    }), // AL
                    15 => builder.push(IROp::MovImm {
                        dst: cond_result,
                        imm: 0,
                    }), // NV
                    _ => builder.push(IROp::MovImm {
                        dst: cond_result,
                        imm: 0,
                    }),
                }

                if is_setm {
                    // CSETM: if cond then Rd = -1 else Rd = 0
                    let minus_one = 207;
                    builder.push(IROp::MovImm {
                        dst: minus_one,
                        imm: 0xFFFFFFFFFFFFFFFF,
                    });
                    builder.push(IROp::Select {
                        dst: rd,
                        cond: cond_result,
                        true_val: minus_one,
                        false_val: zero,
                    });
                } else {
                    // CSET: if cond then Rd = 1 else Rd = 0
                    builder.push(IROp::Select {
                        dst: rd,
                        cond: cond_result,
                        true_val: one,
                        false_val: zero,
                    });
                }

                current_pc += 4;
                continue;
            }

            // CINC/CDEC/CNEG/CINV (Conditional Increment/Decrement/Negate/Invert)
            // sf 0 0 1 1 0 1 0 0 op 0 0 0 0 ...
            if (insn & 0x1FE00000) == 0x1A800000 && (insn & 0x1FFC00) == 0x1F8000 {
                let op = (insn >> 29) & 3;
                let cond = (insn >> 12) & 0xF;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                // Evaluate condition (same as CSET)
                let pstate_reg = 17;
                builder.push(IROp::ReadPstateFlags { dst: pstate_reg });
                let n_flag = 200;
                let z_flag = 201;
                let c_flag = 202;
                let v_flag = 203;
                let one = 204;
                let zero = 205;
                builder.push(IROp::MovImm { dst: one, imm: 1 });
                builder.push(IROp::MovImm { dst: zero, imm: 0 });
                builder.push(IROp::SrlImm {
                    dst: n_flag,
                    src: pstate_reg,
                    sh: 31,
                });
                builder.push(IROp::And {
                    dst: n_flag,
                    src1: n_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: z_flag,
                    src: pstate_reg,
                    sh: 30,
                });
                builder.push(IROp::And {
                    dst: z_flag,
                    src1: z_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: c_flag,
                    src: pstate_reg,
                    sh: 29,
                });
                builder.push(IROp::And {
                    dst: c_flag,
                    src1: c_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: v_flag,
                    src: pstate_reg,
                    sh: 28,
                });
                builder.push(IROp::And {
                    dst: v_flag,
                    src1: v_flag,
                    src2: one,
                });

                let cond_result = 206;
                match cond {
                    0 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: z_flag,
                        rhs: one,
                    }),
                    1 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: z_flag,
                        rhs: zero,
                    }),
                    2 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: c_flag,
                        rhs: one,
                    }),
                    3 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: c_flag,
                        rhs: zero,
                    }),
                    4 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: one,
                    }),
                    5 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: zero,
                    }),
                    6 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: v_flag,
                        rhs: one,
                    }),
                    7 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: v_flag,
                        rhs: zero,
                    }),
                    8 => {
                        let c_set = 207;
                        let z_clear = 208;
                        builder.push(IROp::CmpEq {
                            dst: c_set,
                            lhs: c_flag,
                            rhs: one,
                        });
                        builder.push(IROp::CmpEq {
                            dst: z_clear,
                            lhs: z_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::And {
                            dst: cond_result,
                            src1: c_set,
                            src2: z_clear,
                        });
                    }
                    9 => {
                        let c_clear = 207;
                        let z_set = 208;
                        builder.push(IROp::CmpEq {
                            dst: c_clear,
                            lhs: c_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::CmpEq {
                            dst: z_set,
                            lhs: z_flag,
                            rhs: one,
                        });
                        builder.push(IROp::Or {
                            dst: cond_result,
                            src1: c_clear,
                            src2: z_set,
                        });
                    }
                    10 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: v_flag,
                    }),
                    11 => builder.push(IROp::CmpNe {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: v_flag,
                    }),
                    12 => {
                        let z_clear = 207;
                        let n_eq_v = 208;
                        builder.push(IROp::CmpEq {
                            dst: z_clear,
                            lhs: z_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::CmpEq {
                            dst: n_eq_v,
                            lhs: n_flag,
                            rhs: v_flag,
                        });
                        builder.push(IROp::And {
                            dst: cond_result,
                            src1: z_clear,
                            src2: n_eq_v,
                        });
                    }
                    13 => {
                        let z_set = 207;
                        let n_ne_v = 208;
                        builder.push(IROp::CmpEq {
                            dst: z_set,
                            lhs: z_flag,
                            rhs: one,
                        });
                        builder.push(IROp::CmpNe {
                            dst: n_ne_v,
                            lhs: n_flag,
                            rhs: v_flag,
                        });
                        builder.push(IROp::Or {
                            dst: cond_result,
                            src1: z_set,
                            src2: n_ne_v,
                        });
                    }
                    14 => builder.push(IROp::MovImm {
                        dst: cond_result,
                        imm: 1,
                    }),
                    15 => builder.push(IROp::MovImm {
                        dst: cond_result,
                        imm: 0,
                    }),
                    _ => builder.push(IROp::MovImm {
                        dst: cond_result,
                        imm: 0,
                    }),
                }

                match op {
                    1 => {
                        // CINC: if cond then Rd = Rn + 1 else Rd = Rn
                        let inc_val = 207;
                        builder.push(IROp::AddImm {
                            dst: inc_val,
                            src: rn,
                            imm: 1,
                        });
                        builder.push(IROp::Select {
                            dst: rd,
                            cond: cond_result,
                            true_val: inc_val,
                            false_val: rn,
                        });
                    }
                    2 => {
                        // CINV: if cond then Rd = ~Rn else Rd = Rn
                        let inv_val = 207;
                        builder.push(IROp::Not {
                            dst: inv_val,
                            src: rn,
                        });
                        builder.push(IROp::Select {
                            dst: rd,
                            cond: cond_result,
                            true_val: inv_val,
                            false_val: rn,
                        });
                    }
                    3 => {
                        // CNEG: if cond then Rd = -Rn else Rd = Rn
                        let neg_val = 207;
                        builder.push(IROp::Sub {
                            dst: neg_val,
                            src1: zero,
                            src2: rn,
                        });
                        builder.push(IROp::Select {
                            dst: rd,
                            cond: cond_result,
                            true_val: neg_val,
                            false_val: rn,
                        });
                    }
                    _ => {
                        // CDEC: if cond then Rd = Rn - 1 else Rd = Rn
                        let dec_val = 207;
                        builder.push(IROp::AddImm {
                            dst: dec_val,
                            src: rn,
                            imm: -1,
                        });
                        builder.push(IROp::Select {
                            dst: rd,
                            cond: cond_result,
                            true_val: dec_val,
                            false_val: rn,
                        });
                    }
                }

                current_pc += 4;
                continue;
            }

            // CCMP/CCMN (Conditional Compare)
            // sf 0 0 1 1 1 0 1 0 0 0 0 0 ...
            if (insn & 0x1FE00000) == 0x1A400000 {
                let cond = (insn >> 12) & 0xF;
                let nzcv = insn & 0xF; // NZCV flags to set if condition fails
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let is_neg = (insn & 0x20000000) != 0; // N bit: CMN if set, CMP if clear

                // Evaluate condition
                let pstate_reg = 17;
                builder.push(IROp::ReadPstateFlags { dst: pstate_reg });
                let n_flag = 200;
                let z_flag = 201;
                let c_flag = 202;
                let v_flag = 203;
                let one = 204;
                let zero = 205;
                builder.push(IROp::MovImm { dst: one, imm: 1 });
                builder.push(IROp::MovImm { dst: zero, imm: 0 });
                builder.push(IROp::SrlImm {
                    dst: n_flag,
                    src: pstate_reg,
                    sh: 31,
                });
                builder.push(IROp::And {
                    dst: n_flag,
                    src1: n_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: z_flag,
                    src: pstate_reg,
                    sh: 30,
                });
                builder.push(IROp::And {
                    dst: z_flag,
                    src1: z_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: c_flag,
                    src: pstate_reg,
                    sh: 29,
                });
                builder.push(IROp::And {
                    dst: c_flag,
                    src1: c_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: v_flag,
                    src: pstate_reg,
                    sh: 28,
                });
                builder.push(IROp::And {
                    dst: v_flag,
                    src1: v_flag,
                    src2: one,
                });

                let cond_result = 206;
                match cond {
                    0 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: z_flag,
                        rhs: one,
                    }),
                    1 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: z_flag,
                        rhs: zero,
                    }),
                    2 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: c_flag,
                        rhs: one,
                    }),
                    3 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: c_flag,
                        rhs: zero,
                    }),
                    4 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: one,
                    }),
                    5 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: zero,
                    }),
                    6 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: v_flag,
                        rhs: one,
                    }),
                    7 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: v_flag,
                        rhs: zero,
                    }),
                    8 => {
                        let c_set = 207;
                        let z_clear = 208;
                        builder.push(IROp::CmpEq {
                            dst: c_set,
                            lhs: c_flag,
                            rhs: one,
                        });
                        builder.push(IROp::CmpEq {
                            dst: z_clear,
                            lhs: z_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::And {
                            dst: cond_result,
                            src1: c_set,
                            src2: z_clear,
                        });
                    }
                    9 => {
                        let c_clear = 207;
                        let z_set = 208;
                        builder.push(IROp::CmpEq {
                            dst: c_clear,
                            lhs: c_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::CmpEq {
                            dst: z_set,
                            lhs: z_flag,
                            rhs: one,
                        });
                        builder.push(IROp::Or {
                            dst: cond_result,
                            src1: c_clear,
                            src2: z_set,
                        });
                    }
                    10 => builder.push(IROp::CmpEq {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: v_flag,
                    }),
                    11 => builder.push(IROp::CmpNe {
                        dst: cond_result,
                        lhs: n_flag,
                        rhs: v_flag,
                    }),
                    12 => {
                        let z_clear = 207;
                        let n_eq_v = 208;
                        builder.push(IROp::CmpEq {
                            dst: z_clear,
                            lhs: z_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::CmpEq {
                            dst: n_eq_v,
                            lhs: n_flag,
                            rhs: v_flag,
                        });
                        builder.push(IROp::And {
                            dst: cond_result,
                            src1: z_clear,
                            src2: n_eq_v,
                        });
                    }
                    13 => {
                        let z_set = 207;
                        let n_ne_v = 208;
                        builder.push(IROp::CmpEq {
                            dst: z_set,
                            lhs: z_flag,
                            rhs: one,
                        });
                        builder.push(IROp::CmpNe {
                            dst: n_ne_v,
                            lhs: n_flag,
                            rhs: v_flag,
                        });
                        builder.push(IROp::Or {
                            dst: cond_result,
                            src1: z_set,
                            src2: n_ne_v,
                        });
                    }
                    14 => builder.push(IROp::MovImm {
                        dst: cond_result,
                        imm: 1,
                    }),
                    15 => builder.push(IROp::MovImm {
                        dst: cond_result,
                        imm: 0,
                    }),
                    _ => builder.push(IROp::MovImm {
                        dst: cond_result,
                        imm: 0,
                    }),
                }

                // Perform comparison if condition is true
                let cmp_result = 207;
                if is_neg {
                    // CCMN: Rn + Rm
                    let tmp = 208;
                    builder.push(IROp::Add {
                        dst: tmp,
                        src1: rn,
                        src2: rm,
                    });
                    builder.push(IROp::CmpEq {
                        dst: cmp_result,
                        lhs: tmp,
                        rhs: zero,
                    });
                } else {
                    // CCMP: Rn - Rm
                    let tmp = 208;
                    builder.push(IROp::Sub {
                        dst: tmp,
                        src1: rn,
                        src2: rm,
                    });
                    builder.push(IROp::CmpEq {
                        dst: cmp_result,
                        lhs: tmp,
                        rhs: zero,
                    });
                }

                // Update flags: if condition is true, use comparison result; else use nzcv
                let new_z_flag = 209;
                let new_n_flag = 210;
                let new_c_flag = 211;
                let new_v_flag = 212;

                // Extract flags from nzcv
                let nzcv_n = (nzcv >> 3) & 1;
                let nzcv_z = (nzcv >> 2) & 1;
                let nzcv_c = (nzcv >> 1) & 1;
                let nzcv_v = nzcv & 1;

                let nzcv_n_reg = 213;
                let nzcv_z_reg = 214;
                let nzcv_c_reg = 215;
                let nzcv_v_reg = 216;
                builder.push(IROp::MovImm {
                    dst: nzcv_n_reg,
                    imm: nzcv_n as u64,
                });
                builder.push(IROp::MovImm {
                    dst: nzcv_z_reg,
                    imm: nzcv_z as u64,
                });
                builder.push(IROp::MovImm {
                    dst: nzcv_c_reg,
                    imm: nzcv_c as u64,
                });
                builder.push(IROp::MovImm {
                    dst: nzcv_v_reg,
                    imm: nzcv_v as u64,
                });

                // Select flags based on condition
                let cmp_z = 217;
                let cmp_n = 218;
                let cmp_c = 219;
                let cmp_v = 220;
                if is_neg {
                    let tmp = 221;
                    builder.push(IROp::Add {
                        dst: tmp,
                        src1: rn,
                        src2: rm,
                    });
                    builder.push(IROp::CmpEq {
                        dst: cmp_z,
                        lhs: tmp,
                        rhs: zero,
                    });
                    let sign_mask = 0x8000000000000000u64;
                    let sign_mask_reg = 266;
                    builder.push(IROp::MovImm {
                        dst: sign_mask_reg,
                        imm: sign_mask,
                    });
                    builder.push(IROp::And {
                        dst: cmp_n,
                        src1: tmp,
                        src2: sign_mask_reg,
                    });
                    builder.push(IROp::SrlImm {
                        dst: cmp_n,
                        src: cmp_n,
                        sh: 63,
                    });
                    builder.push(IROp::CmpLtU {
                        dst: cmp_c,
                        lhs: tmp,
                        rhs: rn,
                    });
                    builder.push(IROp::MovImm { dst: cmp_v, imm: 0 });
                } else {
                    let tmp = 221;
                    builder.push(IROp::Sub {
                        dst: tmp,
                        src1: rn,
                        src2: rm,
                    });
                    builder.push(IROp::CmpEq {
                        dst: cmp_z,
                        lhs: tmp,
                        rhs: zero,
                    });
                    let sign_mask = 0x8000000000000000u64;
                    let sign_mask_reg5 = 271;
                    builder.push(IROp::MovImm {
                        dst: sign_mask_reg5,
                        imm: sign_mask,
                    });
                    builder.push(IROp::And {
                        dst: cmp_n,
                        src1: tmp,
                        src2: sign_mask_reg5,
                    });
                    builder.push(IROp::SrlImm {
                        dst: cmp_n,
                        src: cmp_n,
                        sh: 63,
                    });
                    builder.push(IROp::CmpGeU {
                        dst: cmp_c,
                        lhs: rn,
                        rhs: rm,
                    });
                    builder.push(IROp::MovImm { dst: cmp_v, imm: 0 });
                }

                builder.push(IROp::Select {
                    dst: new_z_flag,
                    cond: cond_result,
                    true_val: cmp_z,
                    false_val: nzcv_z_reg,
                });
                builder.push(IROp::Select {
                    dst: new_n_flag,
                    cond: cond_result,
                    true_val: cmp_n,
                    false_val: nzcv_n_reg,
                });
                builder.push(IROp::Select {
                    dst: new_c_flag,
                    cond: cond_result,
                    true_val: cmp_c,
                    false_val: nzcv_c_reg,
                });
                builder.push(IROp::Select {
                    dst: new_v_flag,
                    cond: cond_result,
                    true_val: cmp_v,
                    false_val: nzcv_v_reg,
                });

                // Write flags to PSTATE
                let pstate_value = 222;
                builder.push(IROp::SllImm {
                    dst: pstate_value,
                    src: new_n_flag,
                    sh: 31,
                });
                let z_shifted = 223;
                builder.push(IROp::SllImm {
                    dst: z_shifted,
                    src: new_z_flag,
                    sh: 30,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: z_shifted,
                });
                let c_shifted = 224;
                builder.push(IROp::SllImm {
                    dst: c_shifted,
                    src: new_c_flag,
                    sh: 29,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: c_shifted,
                });
                let v_shifted = 225;
                builder.push(IROp::SllImm {
                    dst: v_shifted,
                    src: new_v_flag,
                    sh: 28,
                });
                builder.push(IROp::Or {
                    dst: pstate_value,
                    src1: pstate_value,
                    src2: v_shifted,
                });
                builder.push(IROp::WritePstateFlags { src: pstate_value });

                current_pc += 4;
                continue;
            }

            // CSEL/CSINC/CSINV/CSNEG (Conditional Select)
            // sf 0 0 1 1 0 1 0 0 op ...
            if (insn & 0x1FE00000) == 0x1A800000 {
                let op = (insn >> 29) & 3;
                let cond = (insn >> 12) & 0xF;
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                // Condition codes: EQ(0), NE(1), CS(2), CC(3), MI(4), PL(5), VS(6), VC(7),
                //                  HI(8), LS(9), GE(10), LT(11), GT(12), LE(13), AL(14), NV(15)
                // Evaluate condition code from PSTATE flags
                let pstate_reg = 17; // PSTATE register
                let cond_reg = 200; // Condition evaluation result (1 = true, 0 = false)

                // Read PSTATE flags
                builder.push(IROp::ReadPstateFlags { dst: pstate_reg });

                // Extract individual flags: N (bit 31), Z (bit 30), C (bit 29), V (bit 28)
                let n_flag = 201;
                let z_flag = 202;
                let c_flag = 203;
                let v_flag = 204;
                let one = 205;
                builder.push(IROp::MovImm { dst: one, imm: 1 });

                builder.push(IROp::SrlImm {
                    dst: n_flag,
                    src: pstate_reg,
                    sh: 31,
                });
                builder.push(IROp::And {
                    dst: n_flag,
                    src1: n_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: z_flag,
                    src: pstate_reg,
                    sh: 30,
                });
                builder.push(IROp::And {
                    dst: z_flag,
                    src1: z_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: c_flag,
                    src: pstate_reg,
                    sh: 29,
                });
                builder.push(IROp::And {
                    dst: c_flag,
                    src1: c_flag,
                    src2: one,
                });
                builder.push(IROp::SrlImm {
                    dst: v_flag,
                    src: pstate_reg,
                    sh: 28,
                });
                builder.push(IROp::And {
                    dst: v_flag,
                    src1: v_flag,
                    src2: one,
                });

                // Evaluate condition code based on PSTATE flags
                // Condition codes:
                // EQ(0): Z == 1
                // NE(1): Z == 0
                // CS(2): C == 1
                // CC(3): C == 0
                // MI(4): N == 1
                // PL(5): N == 0
                // VS(6): V == 1
                // VC(7): V == 0
                // HI(8): C == 1 && Z == 0
                // LS(9): C == 0 || Z == 1
                // GE(10): N == V
                // LT(11): N != V
                // GT(12): Z == 0 && N == V
                // LE(13): Z == 1 || N != V
                // AL(14): Always true
                // NV(15): Always false

                let cond_result = 206;
                let zero = 207;
                builder.push(IROp::MovImm { dst: zero, imm: 0 });

                match cond {
                    0 => {
                        // EQ: Z == 1
                        builder.push(IROp::CmpEq {
                            dst: cond_result,
                            lhs: z_flag,
                            rhs: one,
                        });
                    }
                    1 => {
                        // NE: Z == 0
                        builder.push(IROp::CmpEq {
                            dst: cond_result,
                            lhs: z_flag,
                            rhs: zero,
                        });
                    }
                    2 => {
                        // CS: C == 1
                        builder.push(IROp::CmpEq {
                            dst: cond_result,
                            lhs: c_flag,
                            rhs: one,
                        });
                    }
                    3 => {
                        // CC: C == 0
                        builder.push(IROp::CmpEq {
                            dst: cond_result,
                            lhs: c_flag,
                            rhs: zero,
                        });
                    }
                    4 => {
                        // MI: N == 1
                        builder.push(IROp::CmpEq {
                            dst: cond_result,
                            lhs: n_flag,
                            rhs: one,
                        });
                    }
                    5 => {
                        // PL: N == 0
                        builder.push(IROp::CmpEq {
                            dst: cond_result,
                            lhs: n_flag,
                            rhs: zero,
                        });
                    }
                    6 => {
                        // VS: V == 1
                        builder.push(IROp::CmpEq {
                            dst: cond_result,
                            lhs: v_flag,
                            rhs: one,
                        });
                    }
                    7 => {
                        // VC: V == 0
                        builder.push(IROp::CmpEq {
                            dst: cond_result,
                            lhs: v_flag,
                            rhs: zero,
                        });
                    }
                    8 => {
                        // HI: C == 1 && Z == 0
                        let c_set = 208;
                        let z_clear = 209;
                        builder.push(IROp::CmpEq {
                            dst: c_set,
                            lhs: c_flag,
                            rhs: one,
                        });
                        builder.push(IROp::CmpEq {
                            dst: z_clear,
                            lhs: z_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::And {
                            dst: cond_result,
                            src1: c_set,
                            src2: z_clear,
                        });
                    }
                    9 => {
                        // LS: C == 0 || Z == 1
                        let c_clear = 208;
                        let z_set = 209;
                        builder.push(IROp::CmpEq {
                            dst: c_clear,
                            lhs: c_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::CmpEq {
                            dst: z_set,
                            lhs: z_flag,
                            rhs: one,
                        });
                        builder.push(IROp::Or {
                            dst: cond_result,
                            src1: c_clear,
                            src2: z_set,
                        });
                    }
                    10 => {
                        // GE: N == V
                        builder.push(IROp::CmpEq {
                            dst: cond_result,
                            lhs: n_flag,
                            rhs: v_flag,
                        });
                    }
                    11 => {
                        // LT: N != V
                        builder.push(IROp::CmpNe {
                            dst: cond_result,
                            lhs: n_flag,
                            rhs: v_flag,
                        });
                    }
                    12 => {
                        // GT: Z == 0 && N == V
                        let z_clear = 208;
                        let n_eq_v = 209;
                        builder.push(IROp::CmpEq {
                            dst: z_clear,
                            lhs: z_flag,
                            rhs: zero,
                        });
                        builder.push(IROp::CmpEq {
                            dst: n_eq_v,
                            lhs: n_flag,
                            rhs: v_flag,
                        });
                        builder.push(IROp::And {
                            dst: cond_result,
                            src1: z_clear,
                            src2: n_eq_v,
                        });
                    }
                    13 => {
                        // LE: Z == 1 || N != V
                        let z_set = 208;
                        let n_ne_v = 209;
                        builder.push(IROp::CmpEq {
                            dst: z_set,
                            lhs: z_flag,
                            rhs: one,
                        });
                        builder.push(IROp::CmpNe {
                            dst: n_ne_v,
                            lhs: n_flag,
                            rhs: v_flag,
                        });
                        builder.push(IROp::Or {
                            dst: cond_result,
                            src1: z_set,
                            src2: n_ne_v,
                        });
                    }
                    14 => {
                        // AL: Always true
                        builder.push(IROp::MovImm {
                            dst: cond_result,
                            imm: 1,
                        });
                    }
                    15 => {
                        // NV: Always false
                        builder.push(IROp::MovImm {
                            dst: cond_result,
                            imm: 0,
                        });
                    }
                    _ => {
                        builder.push(IROp::MovImm {
                            dst: cond_result,
                            imm: 0,
                        });
                    }
                }

                builder.push(IROp::AddImm {
                    dst: cond_reg,
                    src: cond_result,
                    imm: 0,
                });

                match op {
                    0 => builder.push(IROp::Select {
                        dst: rd,
                        cond: cond_reg,
                        true_val: rn,
                        false_val: rm,
                    }), // CSEL
                    1 => {
                        // CSINC: if cond then Rd = Rn else Rd = Rm + 1
                        let tmp = 201;
                        builder.push(IROp::AddImm {
                            dst: tmp,
                            src: rm,
                            imm: 1,
                        });
                        builder.push(IROp::Select {
                            dst: rd,
                            cond: cond_reg,
                            true_val: rn,
                            false_val: tmp,
                        });
                    }
                    2 => {
                        // CSINV: if cond then Rd = Rn else Rd = ~Rm
                        let tmp = 201;
                        builder.push(IROp::Not { dst: tmp, src: rm });
                        builder.push(IROp::Select {
                            dst: rd,
                            cond: cond_reg,
                            true_val: rn,
                            false_val: tmp,
                        });
                    }
                    3 => {
                        // CSNEG: if cond then Rd = Rn else Rd = -Rm
                        let zero = 201;
                        let tmp = 202;
                        builder.push(IROp::MovImm { dst: zero, imm: 0 });
                        builder.push(IROp::Sub {
                            dst: tmp,
                            src1: zero,
                            src2: rm,
                        });
                        builder.push(IROp::Select {
                            dst: rd,
                            cond: cond_reg,
                            true_val: rn,
                            false_val: tmp,
                        });
                    }
                    _ => {}
                }
                current_pc += 4;
                continue;
            }

            // BFI/BFXIL (Bitfield Insert/Extract and Insert Low)
            // sf 0 0 1 0 0 1 1 0 ...
            if (insn & 0x1F800000) == 0x13000000 && (insn & 0x60000000) == 0x20000000 {
                let is_signed = (insn & 0x00400000) == 0;
                let immr = (insn >> 16) & 0x3F;
                let imms = (insn >> 10) & 0x3F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;
                let is_bfxil = (insn & 0x20000000) == 0; // Differentiate BFI vs BFXIL

                if is_bfxil {
                    // BFXIL: Extract bits [imms:0] from Rn and insert into Rd[imms:0]
                    let width = (imms + 1) as u8;
                    let mask = (1u64 << width) - 1;
                    let extracted = 200;
                    let mask_reg = 201;
                    builder.push(IROp::MovImm {
                        dst: mask_reg,
                        imm: mask,
                    });
                    builder.push(IROp::And {
                        dst: extracted,
                        src1: rn,
                        src2: mask_reg,
                    });

                    // Clear bits [imms:0] in Rd
                    let cleared = 202;
                    let clear_mask = 203;
                    builder.push(IROp::MovImm {
                        dst: clear_mask,
                        imm: !mask,
                    });
                    builder.push(IROp::And {
                        dst: cleared,
                        src1: rd,
                        src2: clear_mask,
                    });

                    // Insert extracted bits
                    builder.push(IROp::Or {
                        dst: rd,
                        src1: cleared,
                        src2: extracted,
                    });
                } else {
                    // BFI: Extract bits [imms:0] from Rn and insert into Rd[immr+imms:immr]
                    let width = (imms + 1) as u8;
                    let mask = (1u64 << width) - 1;
                    let extracted = 200;
                    let mask_reg = 201;
                    builder.push(IROp::MovImm {
                        dst: mask_reg,
                        imm: mask,
                    });
                    builder.push(IROp::And {
                        dst: extracted,
                        src1: rn,
                        src2: mask_reg,
                    });

                    // Shift extracted bits to position immr
                    let shifted = 202;
                    // 根据 is_signed 决定使用算术移位还是逻辑移位
                    if is_signed {
                        builder.push(IROp::SraImm {
                            dst: shifted,
                            src: extracted,
                            sh: immr as u8,
                        });
                    } else {
                        builder.push(IROp::SllImm {
                            dst: shifted,
                            src: extracted,
                            sh: immr as u8,
                        });
                    }

                    // Clear bits [immr+imms:immr] in Rd
                    let clear_mask = 203;
                    let clear_val = mask << immr;
                    builder.push(IROp::MovImm {
                        dst: clear_mask,
                        imm: !clear_val,
                    });
                    let cleared = 204;
                    builder.push(IROp::And {
                        dst: cleared,
                        src1: rd,
                        src2: clear_mask,
                    });

                    // Insert shifted bits
                    builder.push(IROp::Or {
                        dst: rd,
                        src1: cleared,
                        src2: shifted,
                    });
                }

                current_pc += 4;
                continue;
            }

            // EXTR (Extract Register)
            // sf 0 0 1 1 0 1 1 1 0 0 0 0 ...
            if (insn & 0x1FE00000) == 0x1AC00000 && (insn >> 10) & 0x3F == 0 {
                let lsb = (insn >> 10) & 0x3F;
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                // EXTR: Rd = (Rn >> lsb) | (Rm << (64 - lsb))
                let right_part = 200;
                builder.push(IROp::SrlImm {
                    dst: right_part,
                    src: rn,
                    sh: lsb as u8,
                });
                let left_shift = 201;
                builder.push(IROp::MovImm {
                    dst: left_shift,
                    imm: (64u64 - (lsb as u64)),
                });
                let left_part = 202;
                builder.push(IROp::Sll {
                    dst: left_part,
                    src: rm,
                    shreg: left_shift,
                });
                builder.push(IROp::Or {
                    dst: rd,
                    src1: right_part,
                    src2: left_part,
                });

                current_pc += 4;
                continue;
            }

            // UBFIZ/SBFIZ (Unsigned/Signed Bitfield Insert Zero)
            // sf 0 0 1 0 0 1 1 0 ...
            if (insn & 0x1F800000) == 0x13000000 && (insn & 0x60000000) != 0x20000000 {
                let is_signed = (insn & 0x00400000) == 0;
                let immr = (insn >> 16) & 0x3F;
                let imms = (insn >> 10) & 0x3F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                // Bitfield insert: UBFIZ/SBFIZ extracts (imms+1) bits from Rn starting at bit 0,
                // and inserts them into Rd at position immr
                // Width = imms - immr + 1 (if imms >= immr) or imms + 1 (if imms < immr)
                let width = if imms >= immr {
                    (imms - immr + 1) as u8
                } else {
                    (imms + 1) as u8
                };

                let mask = (1u64 << width) - 1;

                // Extract source bits
                let extracted = 200;
                let mask_reg = 250;
                builder.push(IROp::MovImm {
                    dst: mask_reg,
                    imm: mask,
                });
                builder.push(IROp::And {
                    dst: extracted,
                    src1: rn,
                    src2: mask_reg,
                });

                // For SBFIZ, perform sign extension
                let extended_extracted = 203;
                if is_signed && width > 0 {
                    // Create sign bit mask
                    let sign_bit_pos = width - 1;
                    let sign_bit = 204;
                    builder.push(IROp::SrlImm {
                        dst: sign_bit,
                        src: extracted,
                        sh: sign_bit_pos,
                    });
                    builder.push(IROp::And {
                        dst: sign_bit,
                        src1: sign_bit,
                        src2: 1,
                    });

                    // If sign bit is set, extend with 1s
                    let sign_mask = 205;
                    builder.push(IROp::MovImm {
                        dst: sign_mask,
                        imm: !((1u64 << width) - 1),
                    });
                    builder.push(IROp::Select {
                        dst: extended_extracted,
                        cond: sign_bit,
                        true_val: sign_mask,
                        false_val: extracted,
                    });
                } else {
                    builder.push(IROp::AddImm {
                        dst: extended_extracted,
                        src: extracted,
                        imm: 0,
                    });
                }

                // Shift to target position
                let shifted = 201;
                builder.push(IROp::SllImm {
                    dst: shifted,
                    src: extended_extracted,
                    sh: immr as u8,
                });

                // Clear destination bits and insert
                let clear_mask = !(mask << immr);
                let cleared = 202;
                let clear_reg = 251;
                builder.push(IROp::MovImm {
                    dst: clear_reg,
                    imm: clear_mask,
                });
                builder.push(IROp::And {
                    dst: cleared,
                    src1: rd,
                    src2: clear_reg,
                });
                builder.push(IROp::Or {
                    dst: rd,
                    src1: cleared,
                    src2: shifted,
                });

                current_pc += 4;
                continue;
            }

            // UBFX/SBFX (Unsigned/Signed Bitfield Extract)
            // sf 0 0 1 1 0 0 1 0 ...
            if (insn & 0x1F800000) == 0x13800000 {
                let is_signed = (insn & 0x00400000) == 0;
                let immr = (insn >> 16) & 0x3F;
                let imms = (insn >> 10) & 0x3F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                // Bitfield extract: extract (imms - immr + 1) bits from Rn starting at immr
                let width = if imms >= immr {
                    (imms - immr + 1) as u8
                } else {
                    (imms + 1) as u8
                };
                let mask = (1u64 << width) - 1;

                // Shift right to align LSB
                let shifted = 200;
                builder.push(IROp::SrlImm {
                    dst: shifted,
                    src: rn,
                    sh: immr as u8,
                });

                // Mask to extract only the field
                let extracted = 201;
                let mask_reg2 = 252;
                builder.push(IROp::MovImm {
                    dst: mask_reg2,
                    imm: mask,
                });
                builder.push(IROp::And {
                    dst: extracted,
                    src1: shifted,
                    src2: mask_reg2,
                });

                // Sign extend if SBFX
                if is_signed {
                    let sign_bit = 1u64 << (width - 1);
                    let sign_mask = 202;
                    let sign_ext = 203;
                    builder.push(IROp::MovImm {
                        dst: sign_mask,
                        imm: sign_bit,
                    });
                    let sign_mask_reg6 = 272;
                    builder.push(IROp::MovImm {
                        dst: sign_mask_reg6,
                        imm: sign_bit,
                    });
                    builder.push(IROp::And {
                        dst: sign_ext,
                        src1: extracted,
                        src2: sign_mask_reg6,
                    });
                    let is_negative = 204;
                    builder.push(IROp::CmpNe {
                        dst: is_negative,
                        lhs: sign_ext,
                        rhs: 0,
                    });
                    let extend_mask = 205;
                    builder.push(IROp::MovImm {
                        dst: extend_mask,
                        imm: !mask,
                    });
                    let extended = 206;
                    builder.push(IROp::Or {
                        dst: extended,
                        src1: extracted,
                        src2: extend_mask,
                    });
                    builder.push(IROp::Select {
                        dst: rd,
                        cond: is_negative,
                        true_val: extended,
                        false_val: extracted,
                    });
                } else {
                    builder.push(IROp::AddImm {
                        dst: rd,
                        src: extracted,
                        imm: 0,
                    });
                }

                current_pc += 4;
                continue;
            }

            // LSL/LSR/ASR (Logical/Arithmetic Shift)
            // These are handled as part of ADD/SUB with shift, but also exist as standalone
            // sf 1 1 0 1 0 1 1 0 0 0 0 0 ...
            if (insn & 0xFFC00000) == 0x1AC00000 {
                let shift_type = (insn >> 22) & 3;
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;

                match shift_type {
                    0 => builder.push(IROp::Sll {
                        dst: rd,
                        src: rn,
                        shreg: rm,
                    }), // LSL
                    1 => builder.push(IROp::Srl {
                        dst: rd,
                        src: rn,
                        shreg: rm,
                    }), // LSR
                    2 => builder.push(IROp::Sra {
                        dst: rd,
                        src: rn,
                        shreg: rm,
                    }), // ASR
                    3 => {
                        // ROR: Rotate right - (src >> count) | (src << (64 - count))
                        let size_bits = 64u32; // Assuming 64-bit
                        let left_shift = 200;
                        let right_shift = 201;
                        let mask = 202;
                        builder.push(IROp::Srl {
                            dst: right_shift,
                            src: rn,
                            shreg: rm,
                        });
                        builder.push(IROp::Sub {
                            dst: mask,
                            src1: size_bits,
                            src2: rm,
                        });
                        builder.push(IROp::Sll {
                            dst: left_shift,
                            src: rn,
                            shreg: mask,
                        });
                        builder.push(IROp::Or {
                            dst: rd,
                            src1: right_shift,
                            src2: left_shift,
                        });
                    }
                    _ => {}
                }
                current_pc += 4;
                continue;
            }

            // SXTB/SXTH/SXTW (Sign Extend Byte/Halfword/Word)
            // sf 0 0 0 0 1 0 1 0 0 0 0 0 ...
            if (insn & 0x1FE00000) == 0x13000000 && (insn & 0x60000000) == 0x00000000 {
                let option = (insn >> 22) & 0x3;
                let rm = (insn >> 16) & 0x1F;
                // Note: rn is not used in SXT instructions, only rm is used
                let rd = insn & 0x1F;

                match option {
                    0 => {
                        // SXTB: Sign extend byte (8 bits)
                        let mask = 0xFFu64;
                        let masked = 200;
                        builder.push(IROp::MovImm {
                            dst: masked,
                            imm: mask,
                        });
                        let extracted = 201;
                        builder.push(IROp::And {
                            dst: extracted,
                            src1: rm,
                            src2: masked,
                        });
                        // Check sign bit (bit 7)
                        let sign_bit = 202;
                        builder.push(IROp::SrlImm {
                            dst: sign_bit,
                            src: extracted,
                            sh: 7,
                        });
                        builder.push(IROp::And {
                            dst: sign_bit,
                            src1: sign_bit,
                            src2: 1,
                        });
                        // Sign extend: if sign_bit == 1, set upper bits to 1
                        let sign_mask = 203;
                        builder.push(IROp::MovImm {
                            dst: sign_mask,
                            imm: 0xFFFFFFFFFFFFFF00,
                        });
                        let extended = 204;
                        builder.push(IROp::Select {
                            dst: extended,
                            cond: sign_bit,
                            true_val: sign_mask,
                            false_val: 0,
                        });
                        builder.push(IROp::Or {
                            dst: rd,
                            src1: extracted,
                            src2: extended,
                        });
                    }
                    1 => {
                        // SXTH: Sign extend halfword (16 bits)
                        let mask = 0xFFFFu64;
                        let masked = 200;
                        builder.push(IROp::MovImm {
                            dst: masked,
                            imm: mask,
                        });
                        let extracted = 201;
                        builder.push(IROp::And {
                            dst: extracted,
                            src1: rm,
                            src2: masked,
                        });
                        // Check sign bit (bit 15)
                        let sign_bit = 202;
                        builder.push(IROp::SrlImm {
                            dst: sign_bit,
                            src: extracted,
                            sh: 15,
                        });
                        builder.push(IROp::And {
                            dst: sign_bit,
                            src1: sign_bit,
                            src2: 1,
                        });
                        // Sign extend
                        let sign_mask = 203;
                        builder.push(IROp::MovImm {
                            dst: sign_mask,
                            imm: 0xFFFFFFFFFFFF0000,
                        });
                        let extended = 204;
                        builder.push(IROp::Select {
                            dst: extended,
                            cond: sign_bit,
                            true_val: sign_mask,
                            false_val: 0,
                        });
                        builder.push(IROp::Or {
                            dst: rd,
                            src1: extracted,
                            src2: extended,
                        });
                    }
                    2 => {
                        // SXTW: Sign extend word (32 bits)
                        let mask = 0xFFFFFFFFu64;
                        let masked = 200;
                        builder.push(IROp::MovImm {
                            dst: masked,
                            imm: mask,
                        });
                        let extracted = 201;
                        builder.push(IROp::And {
                            dst: extracted,
                            src1: rm,
                            src2: masked,
                        });
                        // Check sign bit (bit 31)
                        let sign_bit = 202;
                        builder.push(IROp::SrlImm {
                            dst: sign_bit,
                            src: extracted,
                            sh: 31,
                        });
                        builder.push(IROp::And {
                            dst: sign_bit,
                            src1: sign_bit,
                            src2: 1,
                        });
                        // Sign extend
                        let sign_mask = 203;
                        builder.push(IROp::MovImm {
                            dst: sign_mask,
                            imm: 0xFFFFFFFF00000000,
                        });
                        let extended = 204;
                        builder.push(IROp::Select {
                            dst: extended,
                            cond: sign_bit,
                            true_val: sign_mask,
                            false_val: 0,
                        });
                        builder.push(IROp::Or {
                            dst: rd,
                            src1: extracted,
                            src2: extended,
                        });
                    }
                    _ => {
                        // Default: just copy
                        builder.push(IROp::AddImm {
                            dst: rd,
                            src: rm,
                            imm: 0,
                        });
                    }
                }

                current_pc += 4;
                continue;
            }

            // UXTB/UXTH/UXTW (Zero Extend Byte/Halfword/Word)
            // sf 0 0 1 0 1 0 1 0 0 0 0 0 ...
            if (insn & 0x1FE00000) == 0x13000000 && (insn & 0x60000000) == 0x20000000 {
                let option = (insn >> 22) & 0x3;
                let rm = (insn >> 16) & 0x1F;
                // Note: rn is not used in UXT instructions, only rm is used
                let rd = insn & 0x1F;

                match option {
                    0 => {
                        // UXTB: Zero extend byte (8 bits)
                        let mask = 0xFFu64;
                        let masked = 200;
                        builder.push(IROp::MovImm {
                            dst: masked,
                            imm: mask,
                        });
                        builder.push(IROp::And {
                            dst: rd,
                            src1: rm,
                            src2: masked,
                        });
                    }
                    1 => {
                        // UXTH: Zero extend halfword (16 bits)
                        let mask = 0xFFFFu64;
                        let masked = 200;
                        builder.push(IROp::MovImm {
                            dst: masked,
                            imm: mask,
                        });
                        builder.push(IROp::And {
                            dst: rd,
                            src1: rm,
                            src2: masked,
                        });
                    }
                    2 => {
                        // UXTW: Zero extend word (32 bits)
                        let mask = 0xFFFFFFFFu64;
                        let masked = 200;
                        builder.push(IROp::MovImm {
                            dst: masked,
                            imm: mask,
                        });
                        builder.push(IROp::And {
                            dst: rd,
                            src1: rm,
                            src2: masked,
                        });
                    }
                    _ => {
                        // Default: just copy
                        builder.push(IROp::AddImm {
                            dst: rd,
                            src: rm,
                            imm: 0,
                        });
                    }
                }

                current_pc += 4;
                continue;
            }

            // LDRB/LDRH/LDRSB/LDRSH (Load Register Byte/Halfword/Signed Byte/Signed Halfword)
            // Unsigned offset: 00 111 0 01 01 ...
            // Signed offset: 00 111 0 01 11 ...
            if (insn & 0x3F000000) == 0x39000000 || (insn & 0x3F000000) == 0x38000000 {
                let size = (insn >> 30) & 0x3;
                let is_signed = (insn & 0x00400000) != 0; // S bit
                let is_load = (insn & 0x00200000) != 0; // L bit
                let imm12 = (insn >> 10) & 0xFFF;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let mem_size = match size {
                    0 => 1, // byte
                    1 => 2, // halfword
                    2 => 4, // word
                    3 => 8, // doubleword
                    _ => 4,
                };

                let offset = imm12 as i64;

                if is_load {
                    let loaded = 200;
                    builder.push(IROp::Load {
                        dst: loaded,
                        base: rn,
                        offset,
                        size: mem_size,
                        flags: MemFlags::default(),
                    });

                    if is_signed && mem_size < 8 {
                        // Sign extend loaded value
                        let sign_bit_shift = mem_size * 8 - 1;
                        let sign_bit = 201;
                        builder.push(IROp::SrlImm {
                            dst: sign_bit,
                            src: loaded,
                            sh: sign_bit_shift,
                        });
                        builder.push(IROp::And {
                            dst: sign_bit,
                            src1: sign_bit,
                            src2: 1,
                        });
                        let sign_mask = 202;
                        let mask_val = match mem_size {
                            1 => 0xFFFFFFFFFFFFFF00u64,
                            2 => 0xFFFFFFFFFFFF0000u64,
                            4 => 0xFFFFFFFF00000000u64,
                            _ => 0,
                        };
                        builder.push(IROp::MovImm {
                            dst: sign_mask,
                            imm: mask_val,
                        });
                        let extended = 203;
                        builder.push(IROp::Select {
                            dst: extended,
                            cond: sign_bit,
                            true_val: sign_mask,
                            false_val: 0,
                        });
                        builder.push(IROp::Or {
                            dst: rt,
                            src1: loaded,
                            src2: extended,
                        });
                    } else {
                        // Zero extend or full width
                        if mem_size < 8 {
                            let mask = match mem_size {
                                1 => 0xFFu64,
                                2 => 0xFFFFu64,
                                4 => 0xFFFFFFFFu64,
                                _ => 0xFFFFFFFFFFFFFFFFu64,
                            };
                            let mask_reg = 201;
                            builder.push(IROp::MovImm {
                                dst: mask_reg,
                                imm: mask,
                            });
                            builder.push(IROp::And {
                                dst: rt,
                                src1: loaded,
                                src2: mask_reg,
                            });
                        } else {
                            builder.push(IROp::AddImm {
                                dst: rt,
                                src: loaded,
                                imm: 0,
                            });
                        }
                    }
                } else {
                    // Store: STRB/STRH
                    builder.push(IROp::Store {
                        src: rt,
                        base: rn,
                        offset,
                        size: mem_size,
                        flags: MemFlags::default(),
                    });
                }

                current_pc += 4;
                continue;
            }

            // LDUR/STUR (Load/Store Unscaled Register)
            // 00 111 0 00 01 ...
            if (insn & 0x3F000000) == 0x38000000 && (insn & 0x00200000) == 0x00000000 {
                let size = (insn >> 30) & 0x3;
                let is_load = (insn & 0x00100000) != 0; // L bit
                let imm9 = (insn >> 12) & 0x1FF;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let mem_size = match size {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => 4,
                };

                // Sign extend imm9
                let offset = ((imm9 << 23) as i32 >> 23) as i64;

                if is_load {
                    builder.push(IROp::Load {
                        dst: rt,
                        base: rn,
                        offset,
                        size: mem_size,
                        flags: MemFlags::default(),
                    });
                } else {
                    builder.push(IROp::Store {
                        src: rt,
                        base: rn,
                        offset,
                        size: mem_size,
                        flags: MemFlags::default(),
                    });
                }

                current_pc += 4;
                continue;
            }

            // LDXR/STXR (Load/Store Exclusive Register)
            // 10 001 000 01 ...
            if (insn & 0x3F200000) == 0x08200000 {
                let size = (insn >> 30) & 0x3;
                let is_load = (insn & 0x001F0000) == 0x001F0000; // Check for LDXR pattern
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let mem_size = match size {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => 4,
                };

                let flags = MemFlags {
                    atomic: true,
                    order: vm_ir::MemOrder::Acquire,
                    ..MemFlags::default()
                };

                if is_load {
                    builder.push(IROp::AtomicLoadReserve {
                        dst: rt,
                        base: rn,
                        offset: 0,
                        size: mem_size,
                        flags,
                    });
                } else {
                    // STXR: Store Exclusive Register
                    let rs = (insn >> 16) & 0x1F; // Status register
                    let store_flags = MemFlags {
                        atomic: true,
                        order: vm_ir::MemOrder::Release,
                        ..MemFlags::default()
                    };
                    builder.push(IROp::AtomicStoreCond {
                        src: rt,
                        base: rn,
                        offset: 0,
                        size: mem_size,
                        dst_flag: rs,
                        flags: store_flags,
                    });
                }

                current_pc += 4;
                continue;
            }

            // LDXP/STXP (Load/Store Exclusive Pair)
            // 10 001 000 10 ...
            if (insn & 0x3F200000) == 0x08200000 && (insn & 0x00C00000) == 0x00400000 {
                let size = (insn >> 30) & 0x3;
                let is_load = (insn & 0x001F0000) == 0x001F0000;
                let rt2 = (insn >> 10) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let mem_size = match size {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => 4,
                };

                let flags = MemFlags {
                    atomic: true,
                    order: vm_ir::MemOrder::Acquire,
                    ..MemFlags::default()
                };

                if is_load {
                    builder.push(IROp::AtomicLoadReserve {
                        dst: rt,
                        base: rn,
                        offset: 0,
                        size: mem_size,
                        flags,
                    });
                    builder.push(IROp::AtomicLoadReserve {
                        dst: rt2,
                        base: rn,
                        offset: mem_size as i64,
                        size: mem_size,
                        flags,
                    });
                } else {
                    // STXP: Store Exclusive Pair
                    let rs = (insn >> 16) & 0x1F; // Status register
                    let store_flags = MemFlags {
                        atomic: true,
                        order: vm_ir::MemOrder::Release,
                        ..MemFlags::default()
                    };
                    builder.push(IROp::AtomicStoreCond {
                        src: rt,
                        base: rn,
                        offset: 0,
                        size: mem_size,
                        dst_flag: rs,
                        flags: store_flags,
                    });
                    builder.push(IROp::AtomicStoreCond {
                        src: rt2,
                        base: rn,
                        offset: mem_size as i64,
                        size: mem_size,
                        dst_flag: rs,
                        flags: store_flags,
                    });
                }

                current_pc += 4;
                continue;
            }

            // LDAR/STLR (Load/Store Acquire/Release)
            // 10 001 000 11 ...
            if (insn & 0x3F200000) == 0x08200000 && (insn & 0x00C00000) == 0x00800000 {
                let size = (insn >> 30) & 0x3;
                let is_load = (insn & 0x00100000) != 0;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;

                let mem_size = match size {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => 4,
                };

                let order = if is_load {
                    vm_ir::MemOrder::Acquire
                } else {
                    vm_ir::MemOrder::Release
                };
                let flags = MemFlags {
                    atomic: true,
                    order,
                    ..MemFlags::default()
                };

                if is_load {
                    builder.push(IROp::Load {
                        dst: rt,
                        base: rn,
                        offset: 0,
                        size: mem_size,
                        flags,
                    });
                } else {
                    builder.push(IROp::Store {
                        src: rt,
                        base: rn,
                        offset: 0,
                        size: mem_size,
                        flags,
                    });
                }

                current_pc += 4;
                continue;
            }

            // PRFM (Prefetch Memory)
            // 11 111 0 00 10 ...
            if (insn & 0xFF000000) == 0xF8000000 {
                // Prefetch hint - no-op for now
                builder.push(IROp::Nop);
                current_pc += 4;
                continue;
            }

            // Scalar Floating-point instructions
            // FADD/FSUB/FMUL/FDIV/FSQRT (scalar)
            // 00 0 11110 ...
            if (insn & 0x1F000000) == 0x1E000000 {
                let opcode = (insn >> 15) & 0x3F;
                let rm = (insn >> 16) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;
                let type_bits = (insn >> 22) & 0x3; // 0 = 16-bit, 1 = 32-bit, 2 = 64-bit

                // Map to floating-point register space (FP registers are 64-95)
                let fp_rd = 64 + rd;
                let fp_rn = 64 + rn;
                let fp_rm = 64 + rm;

                match opcode {
                    0x00 => {
                        // FADD: Floating-point Add
                        if type_bits == 1 {
                            builder.push(IROp::FaddS {
                                dst: fp_rd,
                                src1: fp_rn,
                                src2: fp_rm,
                            });
                        } else if type_bits == 2 {
                            builder.push(IROp::Fadd {
                                dst: fp_rd,
                                src1: fp_rn,
                                src2: fp_rm,
                            });
                        }
                    }
                    0x01 => {
                        // FSUB: Floating-point Subtract
                        if type_bits == 1 {
                            builder.push(IROp::FsubS {
                                dst: fp_rd,
                                src1: fp_rn,
                                src2: fp_rm,
                            });
                        } else if type_bits == 2 {
                            builder.push(IROp::Fsub {
                                dst: fp_rd,
                                src1: fp_rn,
                                src2: fp_rm,
                            });
                        }
                    }
                    0x02 => {
                        // FMUL: Floating-point Multiply
                        if type_bits == 1 {
                            builder.push(IROp::FmulS {
                                dst: fp_rd,
                                src1: fp_rn,
                                src2: fp_rm,
                            });
                        } else if type_bits == 2 {
                            builder.push(IROp::Fmul {
                                dst: fp_rd,
                                src1: fp_rn,
                                src2: fp_rm,
                            });
                        }
                    }
                    0x03 => {
                        // FDIV: Floating-point Divide
                        if type_bits == 1 {
                            builder.push(IROp::FdivS {
                                dst: fp_rd,
                                src1: fp_rn,
                                src2: fp_rm,
                            });
                        } else if type_bits == 2 {
                            builder.push(IROp::Fdiv {
                                dst: fp_rd,
                                src1: fp_rn,
                                src2: fp_rm,
                            });
                        }
                    }
                    0x1F => {
                        // FSQRT: Floating-point Square Root
                        if type_bits == 1 {
                            builder.push(IROp::FsqrtS {
                                dst: fp_rd,
                                src: fp_rn,
                            });
                        } else if type_bits == 2 {
                            builder.push(IROp::Fsqrt {
                                dst: fp_rd,
                                src: fp_rn,
                            });
                        }
                    }
                    _ => {}
                }

                current_pc += 4;
                continue;
            }

            // FCMP/FCMPE (Floating-point Compare)
            // 00 0 11110 1 0 0 0 ...
            if (insn & 0x1F200000) == 0x1E200000 && (insn & 0x00C00000) == 0x00000000 {
                let opcode = insn & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rm = (insn >> 16) & 0x1F;
                let type_bits = (insn >> 22) & 0x3;
                let is_fcmpe = (insn & 0x00200000) != 0; // E bit

                // Validate opcode - should be 0x00 for FCMP/FCMPE
                if opcode != 0x00 {
                    // Invalid opcode for FCMP/FCMPE instruction
                    // Could panic or handle as undefined instruction
                }

                let fp_rn = 64 + rn;
                let fp_rm = if (insn & 0x00010000) != 0 { 0 } else { 64 + rm }; // Zero register if bit 16 is set

                // Compare and update FPCR flags
                let cmp_result = 200;
                if type_bits == 1 {
                    if is_fcmpe {
                        // FCMPE: signaling NaN comparisons raise InvalidOperation exception
                        builder.push(IROp::FeqS {
                            dst: cmp_result,
                            src1: fp_rn,
                            src2: fp_rm,
                        });
                    } else {
                        // FCMP: quiet NaN comparisons do not raise exceptions
                        builder.push(IROp::FeqS {
                            dst: cmp_result,
                            src1: fp_rn,
                            src2: fp_rm,
                        });
                    }
                } else if type_bits == 2 {
                    if is_fcmpe {
                        // FCMPE: signaling NaN comparisons raise InvalidOperation exception
                        builder.push(IROp::Feq {
                            dst: cmp_result,
                            src1: fp_rn,
                            src2: fp_rm,
                        });
                    } else {
                        // FCMP: quiet NaN comparisons do not raise exceptions
                        builder.push(IROp::Feq {
                            dst: cmp_result,
                            src1: fp_rn,
                            src2: fp_rm,
                        });
                    }
                }

                // Update PSTATE flags (simplified)
                // Read current PSTATE flags
                let pstate_reg = 17; // PSTATE register for flag updates
                builder.push(IROp::ReadPstateFlags { dst: pstate_reg });

                let zero = 201;
                let one = 202;
                builder.push(IROp::MovImm { dst: zero, imm: 0 });
                builder.push(IROp::MovImm { dst: one, imm: 1 });
                let z_flag = 203;
                builder.push(IROp::CmpEq {
                    dst: z_flag,
                    lhs: cmp_result,
                    rhs: one,
                });
                let n_flag = 204;
                // For FCMP, N flag indicates less than
                if type_bits == 1 {
                    builder.push(IROp::FltS {
                        dst: n_flag,
                        src1: fp_rn,
                        src2: fp_rm,
                    });
                } else if type_bits == 2 {
                    builder.push(IROp::Flt {
                        dst: n_flag,
                        src1: fp_rn,
                        src2: fp_rm,
                    });
                }
                let c_flag = 205;
                // C flag indicates greater than or equal
                if type_bits == 1 {
                    builder.push(IROp::FleS {
                        dst: c_flag,
                        src1: fp_rm,
                        src2: fp_rn,
                    });
                } else if type_bits == 2 {
                    builder.push(IROp::Fle {
                        dst: c_flag,
                        src1: fp_rm,
                        src2: fp_rn,
                    });
                }
                let v_flag = 206;
                builder.push(IROp::MovImm {
                    dst: v_flag,
                    imm: 0,
                }); // Simplified

                // Preserve other PSTATE flags while updating NZCV
                // Clear NZCV bits (bits 31-28) in current PSTATE
                let clear_mask = 207;
                builder.push(IROp::MovImm {
                    dst: clear_mask,
                    imm: 0x0FFFFFFF,
                });
                let cleared_pstate = 208;
                builder.push(IROp::And {
                    dst: cleared_pstate,
                    src1: pstate_reg,
                    src2: clear_mask,
                });

                // Build new NZCV flags
                let nzcv_flags = 209;
                builder.push(IROp::SllImm {
                    dst: nzcv_flags,
                    src: n_flag,
                    sh: 31,
                });
                let z_shifted = 210;
                builder.push(IROp::SllImm {
                    dst: z_shifted,
                    src: z_flag,
                    sh: 30,
                });
                builder.push(IROp::Or {
                    dst: nzcv_flags,
                    src1: nzcv_flags,
                    src2: z_shifted,
                });
                let c_shifted = 211;
                builder.push(IROp::SllImm {
                    dst: c_shifted,
                    src: c_flag,
                    sh: 29,
                });
                builder.push(IROp::Or {
                    dst: nzcv_flags,
                    src1: nzcv_flags,
                    src2: c_shifted,
                });
                let v_shifted = 212;
                builder.push(IROp::SllImm {
                    dst: v_shifted,
                    src: v_flag,
                    sh: 28,
                });
                builder.push(IROp::Or {
                    dst: nzcv_flags,
                    src1: nzcv_flags,
                    src2: v_shifted,
                });

                // Combine cleared PSTATE with new NZCV flags
                let new_pstate = 213;
                builder.push(IROp::Or {
                    dst: new_pstate,
                    src1: cleared_pstate,
                    src2: nzcv_flags,
                });

                builder.push(IROp::WritePstateFlags { src: new_pstate });

                current_pc += 4;
                continue;
            }

            // FCVT (Floating-point Convert)
            // 00 0 11110 1 1 0 0 0 ...
            if (insn & 0x1F200000) == 0x1E200000 && (insn & 0x00C00000) == 0x00400000 {
                let opcode = (insn >> 10) & 0x3F;
                let rn = (insn >> 5) & 0x1F;
                let rd = insn & 0x1F;
                let type_bits = (insn >> 22) & 0x3;

                let fp_rd = 64 + rd;
                let fp_rn = 64 + rn;

                match opcode {
                    0x00 => {
                        // FCVTNS: Convert to signed integer, round to nearest
                        if type_bits == 1 {
                            builder.push(IROp::Fcvtws {
                                dst: rd,
                                src: fp_rn,
                            });
                        } else if type_bits == 2 {
                            builder.push(IROp::Fcvtls {
                                dst: rd,
                                src: fp_rn,
                            });
                        }
                    }
                    0x01 => {
                        // FCVTNU: Convert to unsigned integer, round to nearest
                        if type_bits == 1 {
                            builder.push(IROp::Fcvtwus {
                                dst: rd,
                                src: fp_rn,
                            });
                        } else if type_bits == 2 {
                            builder.push(IROp::Fcvtlus {
                                dst: rd,
                                src: fp_rn,
                            });
                        }
                    }
                    0x20 => {
                        // FCVTZS: Convert to signed integer, round toward zero
                        if type_bits == 1 {
                            builder.push(IROp::Fcvtws {
                                dst: rd,
                                src: fp_rn,
                            });
                        } else if type_bits == 2 {
                            builder.push(IROp::Fcvtls {
                                dst: rd,
                                src: fp_rn,
                            });
                        }
                    }
                    0x21 => {
                        // FCVTZU: Convert to unsigned integer, round toward zero
                        if type_bits == 1 {
                            builder.push(IROp::Fcvtwus {
                                dst: rd,
                                src: fp_rn,
                            });
                        } else if type_bits == 2 {
                            builder.push(IROp::Fcvtlus {
                                dst: rd,
                                src: fp_rn,
                            });
                        }
                    }
                    0x07 => {
                        // FCVT: Convert between floating-point formats
                        if type_bits == 1 {
                            // F64 -> F32
                            builder.push(IROp::Fcvtsd {
                                dst: fp_rd,
                                src: fp_rn,
                            });
                        } else if type_bits == 2 {
                            // F32 -> F64
                            builder.push(IROp::Fcvtds {
                                dst: fp_rd,
                                src: fp_rn,
                            });
                        }
                    }
                    _ => {}
                }

                current_pc += 4;
                continue;
            }

            // System instructions
            // MSR/MRS (Move System Register)
            // 11 0101 0100 ...
            if (insn & 0xFFF00000) == 0xD5100000 {
                let op0 = (insn >> 19) & 0x3;
                let op1 = (insn >> 16) & 0x7;
                let crn = (insn >> 12) & 0xF;
                let crm = (insn >> 8) & 0xF;
                let op2 = (insn >> 5) & 0x7;
                let rt = insn & 0x1F;
                let is_mrs = (insn & 0x001F0000) == 0x001F0000; // Check if rt == 31 for MRS

                if is_mrs {
                    // MRS: Move System Register to general register
                    // Simplified: read from a special register space
                    let sysreg_addr = 200;
                    builder.push(IROp::MovImm {
                        dst: sysreg_addr,
                        imm: 0xF0004000
                            + ((op0 as u64) << 16)
                            + ((op1 as u64) << 13)
                            + ((crn as u64) << 9)
                            + ((crm as u64) << 5)
                            + (op2 as u64),
                    });
                    builder.push(IROp::Load {
                        dst: rt,
                        base: sysreg_addr,
                        offset: 0,
                        size: 8,
                        flags: MemFlags::default(),
                    });
                } else {
                    // MSR: Move general register to System Register
                    let sysreg_addr = 200;
                    builder.push(IROp::MovImm {
                        dst: sysreg_addr,
                        imm: 0xF0004000
                            + ((op0 as u64) << 16)
                            + ((op1 as u64) << 13)
                            + ((crn as u64) << 9)
                            + ((crm as u64) << 5)
                            + (op2 as u64),
                    });
                    builder.push(IROp::Store {
                        src: rt,
                        base: sysreg_addr,
                        offset: 0,
                        size: 8,
                        flags: MemFlags::default(),
                    });
                }

                current_pc += 4;
                continue;
            }

            // SVC/HVC/SMC (Supervisor/Hypervisor/Secure Monitor Call)
            // 11 0101 0000 ...
            if (insn & 0xFFE00000) == 0xD4000000 {
                let _op0 = (insn >> 16) & 0x1F; // Used in match statement
                let _imm16 = insn & 0xFFFF; // Reserved for future use

                match _op0 {
                    0x01 => {
                        // SVC: Supervisor Call
                        builder.push(IROp::SysCall);
                    }
                    0x02 => {
                        // HVC: Hypervisor Call
                        builder.push(IROp::SysCall); // Simplified
                    }
                    0x03 => {
                        // SMC: Secure Monitor Call
                        builder.push(IROp::SysCall); // Simplified
                    }
                    _ => {}
                }

                current_pc += 4;
                continue;
            }

            // ERET (Exception Return)
            // 11 0101 1010 0 ...
            if (insn & 0xFFFFF01F) == 0xD65F0000 {
                builder.set_term(Terminator::Ret);
                break;
            }

            // DMB/DSB/ISB (Data/Data Synchronization/Instruction Synchronization Barrier)
            // 11 0101 0100 0 ...
            if (insn & 0xFFFFF000) == 0xD5030000 {
                let _op = (insn >> 8) & 0xF; // Used in match statement
                let _cr = insn & 0xF; // Reserved for future use

                match _op {
                    0x5 => {
                        // DMB: Data Memory Barrier
                        builder.push(IROp::Nop); // Placeholder for memory barrier
                    }
                    0x9 => {
                        // DSB: Data Synchronization Barrier
                        builder.push(IROp::Nop); // Placeholder for memory barrier
                    }
                    0xF => {
                        // ISB: Instruction Synchronization Barrier
                        builder.push(IROp::Nop); // Placeholder for instruction barrier
                    }
                    _ => {}
                }

                current_pc += 4;
                continue;
            }

            // DC/IC (Data Cache/Instruction Cache operations)
            // 11 0101 0110 0 ...
            if (insn & 0xFFFFF000) == 0xD50B0000 {
                let _op1 = (insn >> 8) & 0xF; // Reserved for future use
                let _crn = (insn >> 12) & 0xF; // Reserved for future use
                let _crm = insn & 0xF; // Reserved for future use
                let _rt = (insn >> 5) & 0x1F; // Reserved for future use

                // Cache operations - simplified as no-ops
                builder.push(IROp::Nop);

                current_pc += 4;
                continue;
            }

            // LDP (Signed Offset) - 64-bit
            // 10 101 001 01...
            if (insn & 0xFFC00000) == 0xA9400000 {
                let imm7 = (insn >> 15) & 0x7F;
                let rt2 = (insn >> 10) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;
                let offset = ((imm7 << 25) as i32 >> 25) as i64 * 8;

                builder.push(IROp::Load {
                    dst: rt,
                    base: rn,
                    offset,
                    size: 8,
                    flags: MemFlags::default(),
                });
                builder.push(IROp::Load {
                    dst: rt2,
                    base: rn,
                    offset: offset + 8,
                    size: 8,
                    flags: MemFlags::default(),
                });
                current_pc += 4;
                continue;
            }

            // STP (Signed Offset) - 64-bit
            // 10 101 001 00...
            if (insn & 0xFFC00000) == 0xA9000000 {
                let imm7 = (insn >> 15) & 0x7F;
                let rt2 = (insn >> 10) & 0x1F;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;
                let offset = ((imm7 << 25) as i32 >> 25) as i64 * 8;

                builder.push(IROp::Store {
                    src: rt,
                    base: rn,
                    offset,
                    size: 8,
                    flags: MemFlags::default(),
                });
                builder.push(IROp::Store {
                    src: rt2,
                    base: rn,
                    offset: offset + 8,
                    size: 8,
                    flags: MemFlags::default(),
                });
                current_pc += 4;
                continue;
            }

            // 检查是否为厂商扩展指令
            let cpu_info = CpuInfo::get();
            let mut handled = false;

            // Apple AMX
            if cpu_info.vendor == vm_accel::cpuinfo::CpuVendor::Apple
                && cpu_info.features.amx
                && let Ok(Some(amx_insn)) = self.amx_decoder.decode(insn, current_pc)
            {
                let mut reg_file = RegisterFile::new(32, vm_ir::RegisterMode::SSA);
                if self
                    .amx_decoder
                    .to_ir(&amx_insn, &mut builder, &mut reg_file)
                    .is_err()
                {
                    // 错误处理
                } else {
                    handled = true;
                }
            }

            // Qualcomm Hexagon DSP
            if !handled
                && cpu_info.vendor == vm_accel::cpuinfo::CpuVendor::Qualcomm
                && cpu_info.features.hexagon_dsp
                && let Ok(Some(hex_insn)) = self.hexagon_decoder.decode(insn, current_pc)
            {
                let mut reg_file = RegisterFile::new(32, vm_ir::RegisterMode::SSA);
                if self
                    .hexagon_decoder
                    .to_ir(&hex_insn, &mut builder, &mut reg_file)
                    .is_err()
                {
                    // 错误处理
                } else {
                    handled = true;
                }
            }

            // MediaTek APU
            if !handled
                && cpu_info.vendor == vm_accel::cpuinfo::CpuVendor::MediaTek
                && cpu_info.features.apu
                && let Ok(Some(apu_insn)) = self.apu_decoder.decode(insn, current_pc)
            {
                let mut reg_file = RegisterFile::new(32, vm_ir::RegisterMode::SSA);
                if self
                    .apu_decoder
                    .to_ir(&apu_insn, &mut builder, &mut reg_file)
                    .is_ok()
                {
                    handled = true;
                }
            }

            // HiSilicon NPU
            if !handled
                && cpu_info.vendor == vm_accel::cpuinfo::CpuVendor::HiSilicon
                && cpu_info.features.npu
                && let Ok(Some(npu_insn)) = self.npu_decoder.decode(insn, current_pc)
            {
                let mut reg_file = RegisterFile::new(32, vm_ir::RegisterMode::SSA);
                if self
                    .npu_decoder
                    .to_ir(&npu_insn, &mut builder, &mut reg_file)
                    .is_ok()
                {
                    handled = true;
                }
            }

            if handled {
                current_pc += 4;
                continue;
            }

            // 尝试解码 NEON 指令
            if ExtendedDecoder::has_neon()
                && let Some(_decoded) = ExtendedDecoder::decode_neon(insn)
            {
                // 这里应该实际处理 NEON 指令
                // 目前只是占位符实现
                builder.push(IROp::Nop); // 占位符
                current_pc += 4;
                continue;
            }

            builder.set_term(Terminator::Fault { cause: 0 });
            break;
        }

        let block = builder.build();

        // 缓存解码结果
        if let Some(ref mut cache) = self.decode_cache {
            if cache.len() < self.cache_size_limit {
                cache.insert(pc, block.clone());
            } else {
                cache.clear();
                cache.insert(pc, block.clone());
            }
        }

        Ok(block)
    }
}

pub mod api {
    pub use super::Cond;
    fn clamp_imm26_bytes(imm: i64) -> i32 {
        let mut v = imm >> 2;
        let min = -(1 << 25);
        let max = (1 << 25) - 1;
        if v < min {
            v = min;
        }
        if v > max {
            v = max;
        }
        v as i32
    }
    pub fn encode_b(imm_bytes: i64) -> u32 {
        let v = clamp_imm26_bytes(imm_bytes) as u32;
        (0b000101u32 << 26) | (v & 0x03FF_FFFF)
    }
    pub fn encode_bl(imm_bytes: i64) -> u32 {
        let v = clamp_imm26_bytes(imm_bytes) as u32;
        (0b100101u32 << 26) | (v & 0x03FF_FFFF)
    }
    pub fn encode_br(rn: u32) -> u32 {
        0xD61F0000u32 | ((rn & 0x1F) << 5)
    }
    fn clamp_imm21_bytes(imm: i64) -> i64 {
        let min = -(1 << 20);
        let max = (1 << 20) - 1;
        let mut v = imm;
        if v < min {
            v = min;
        }
        if v > max {
            v = max;
        }
        v
    }
    pub fn encode_adr(rd: u32, imm_bytes: i64) -> u32 {
        let v = clamp_imm21_bytes(imm_bytes);
        let immlo = (v as u64 & 0x3) as u32;
        let immhi = ((v as u64 >> 2) & 0x7FFFF) as u32;
        0x10000000u32 | ((immlo & 0x3) << 29) | ((immhi & 0x7FFFF) << 5) | (rd & 0x1F)
    }
    pub fn encode_adrp(rd: u32, imm_bytes: i64) -> u32 {
        let v = clamp_imm21_bytes(imm_bytes >> 12);
        let immlo = (v as u64 & 0x3) as u32;
        let immhi = ((v as u64 >> 2) & 0x7FFFF) as u32;
        0x90000000u32 | ((immlo & 0x3) << 29) | ((immhi & 0x7FFFF) << 5) | (rd & 0x1F)
    }
    pub fn encode_blr(rn: u32) -> u32 {
        0xD63F0000u32 | ((rn & 0x1F) << 5)
    }
    pub fn encode_ret(rn: u32) -> u32 {
        0xD65F0000u32 | ((rn & 0x1F) << 5)
    }
    pub mod cond {
        use super::{
            encode_cdec_eq, encode_cdec_ge, encode_cdec_gt, encode_cdec_hi, encode_cdec_le,
            encode_cdec_ls, encode_cdec_lt, encode_cdec_mi, encode_cdec_ne, encode_cdec_pl,
            encode_cdec_vc, encode_cdec_vs, encode_cinc_eq, encode_cinc_ge, encode_cinc_gt,
            encode_cinc_hi, encode_cinc_le, encode_cinc_ls, encode_cinc_lt, encode_cinc_mi,
            encode_cinc_ne, encode_cinc_pl, encode_cinc_vc, encode_cinc_vs, encode_cinv_eq,
            encode_cinv_ge, encode_cinv_gt, encode_cinv_hi, encode_cinv_le, encode_cinv_ls,
            encode_cinv_lt, encode_cinv_mi, encode_cinv_ne, encode_cinv_pl, encode_cinv_vc,
            encode_cinv_vs, encode_cneg_eq, encode_cneg_ge, encode_cneg_gt, encode_cneg_hi,
            encode_cneg_le, encode_cneg_ls, encode_cneg_lt, encode_cneg_mi, encode_cneg_ne,
            encode_cneg_pl, encode_cneg_vc, encode_cneg_vs,
        };
        pub mod cinc {
            use super::*;
            pub fn eq(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_eq(rd, rn, is64)
            }
            pub fn ne(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_ne(rd, rn, is64)
            }
            pub fn ge(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_ge(rd, rn, is64)
            }
            pub fn lt(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_lt(rd, rn, is64)
            }
            pub fn hi(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_hi(rd, rn, is64)
            }
            pub fn ls(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_ls(rd, rn, is64)
            }
            pub fn gt(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_gt(rd, rn, is64)
            }
            pub fn le(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_le(rd, rn, is64)
            }
            pub fn mi(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_mi(rd, rn, is64)
            }
            pub fn pl(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_pl(rd, rn, is64)
            }
            pub fn vs(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_vs(rd, rn, is64)
            }
            pub fn vc(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinc_vc(rd, rn, is64)
            }
        }
        pub mod cdec {
            use super::*;
            pub fn eq(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_eq(rd, rn, is64)
            }
            pub fn ne(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_ne(rd, rn, is64)
            }
            pub fn ge(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_ge(rd, rn, is64)
            }
            pub fn lt(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_lt(rd, rn, is64)
            }
            pub fn hi(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_hi(rd, rn, is64)
            }
            pub fn ls(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_ls(rd, rn, is64)
            }
            pub fn gt(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_gt(rd, rn, is64)
            }
            pub fn le(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_le(rd, rn, is64)
            }
            pub fn mi(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_mi(rd, rn, is64)
            }
            pub fn pl(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_pl(rd, rn, is64)
            }
            pub fn vs(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_vs(rd, rn, is64)
            }
            pub fn vc(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cdec_vc(rd, rn, is64)
            }
        }
        pub mod cinv {
            use super::*;
            pub fn eq(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_eq(rd, rn, is64)
            }
            pub fn ne(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_ne(rd, rn, is64)
            }
            pub fn ge(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_ge(rd, rn, is64)
            }
            pub fn lt(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_lt(rd, rn, is64)
            }
            pub fn hi(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_hi(rd, rn, is64)
            }
            pub fn ls(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_ls(rd, rn, is64)
            }
            pub fn gt(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_gt(rd, rn, is64)
            }
            pub fn le(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_le(rd, rn, is64)
            }
            pub fn mi(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_mi(rd, rn, is64)
            }
            pub fn pl(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_pl(rd, rn, is64)
            }
            pub fn vs(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_vs(rd, rn, is64)
            }
            pub fn vc(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cinv_vc(rd, rn, is64)
            }
        }
        pub mod cneg {
            use super::*;
            pub fn eq(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_eq(rd, rn, is64)
            }
            pub fn ne(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_ne(rd, rn, is64)
            }
            pub fn ge(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_ge(rd, rn, is64)
            }
            pub fn lt(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_lt(rd, rn, is64)
            }
            pub fn hi(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_hi(rd, rn, is64)
            }
            pub fn ls(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_ls(rd, rn, is64)
            }
            pub fn gt(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_gt(rd, rn, is64)
            }
            pub fn le(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_le(rd, rn, is64)
            }
            pub fn mi(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_mi(rd, rn, is64)
            }
            pub fn pl(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_pl(rd, rn, is64)
            }
            pub fn vs(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_vs(rd, rn, is64)
            }
            pub fn vc(rd: u32, rn: u32, is64: bool) -> u32 {
                encode_cneg_vc(rd, rn, is64)
            }
        }
        pub mod eq {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::EQ as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::EQ as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::EQ as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::EQ as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_eq(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_eq(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_eq(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_eq(rd, rn, is64)
            }
        }
        pub mod ne {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::NE as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::NE as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::NE as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::NE as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_ne(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_ne(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_ne(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_ne(rd, rn, is64)
            }
        }
        pub mod ge {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::GE as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::GE as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::GE as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::GE as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_ge(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_ge(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_ge(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_ge(rd, rn, is64)
            }
        }
        pub mod lt {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::LT as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::LT as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::LT as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::LT as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_lt(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_lt(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_lt(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_lt(rd, rn, is64)
            }
        }
        pub mod hi {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::HI as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::HI as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::HI as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::HI as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_hi(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_hi(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_hi(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_hi(rd, rn, is64)
            }
        }
        pub mod ls {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::LS as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::LS as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::LS as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::LS as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_ls(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_ls(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_ls(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_ls(rd, rn, is64)
            }
        }
        pub mod gt {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::GT as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::GT as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::GT as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::GT as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_gt(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_gt(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_gt(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_gt(rd, rn, is64)
            }
        }
        pub mod le {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::LE as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::LE as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::LE as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::LE as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_le(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_le(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_le(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_le(rd, rn, is64)
            }
        }
        pub mod mi {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::MI as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::MI as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::MI as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::MI as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_mi(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_mi(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_mi(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_mi(rd, rn, is64)
            }
        }
        pub mod pl {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::PL as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::PL as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::PL as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::PL as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_pl(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_pl(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_pl(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_pl(rd, rn, is64)
            }
        }
        pub mod vs {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::VS as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::VS as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::VS as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::VS as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_vs(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_vs(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_vs(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_vs(rd, rn, is64)
            }
        }
        pub mod vc {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csel(rd, rn, rm, super::super::Cond::VC as u32, is64)
            }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinv(rd, rn, rm, super::super::Cond::VC as u32, is64)
            }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_csinc(rd, rn, rm, super::super::Cond::VC as u32, is64)
            }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 {
                super::super::encode_cneg(rd, rn, rm, super::super::Cond::VC as u32, is64)
            }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinc_vc(rd, rn, is64)
            }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cdec_vc(rd, rn, is64)
            }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cinv_vc(rd, rn, is64)
            }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 {
                super::super::encode_cneg_vc(rd, rn, is64)
            }
        }
    }
    fn clamp_imm19_bytes(imm: i64) -> i32 {
        let mut v = imm >> 2;
        let min = -(1 << 18);
        let max = (1 << 18) - 1;
        if v < min {
            v = min;
        }
        if v > max {
            v = max;
        }
        v as i32
    }
    fn clamp_imm14_bytes(imm: i64) -> i32 {
        let mut v = imm >> 2;
        let min = -(1 << 13);
        let max = (1 << 13) - 1;
        if v < min {
            v = min;
        }
        if v > max {
            v = max;
        }
        v as i32
    }
    pub fn encode_b_cond(cond: u32, imm_bytes: i64) -> u32 {
        let v = clamp_imm19_bytes(imm_bytes) as u32;
        0x54000000u32 | ((v & 0x7FFFF) << 5) | (cond & 0xF)
    }
    pub fn encode_b_cond_cc(cond: super::Cond, imm_bytes: i64) -> u32 {
        encode_b_cond(cond as u32, imm_bytes)
    }
    pub fn encode_b_eq(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::EQ, imm_bytes)
    }
    pub fn encode_b_ne(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::NE, imm_bytes)
    }
    pub fn encode_b_ge(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::GE, imm_bytes)
    }
    pub fn encode_b_lt(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::LT, imm_bytes)
    }
    pub fn encode_b_gt(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::GT, imm_bytes)
    }
    pub fn encode_b_le(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::LE, imm_bytes)
    }
    pub fn encode_b_hi(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::HI, imm_bytes)
    }
    pub fn encode_b_ls(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::LS, imm_bytes)
    }
    pub fn encode_b_mi(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::MI, imm_bytes)
    }
    pub fn encode_b_pl(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::PL, imm_bytes)
    }
    pub fn encode_b_vs(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::VS, imm_bytes)
    }
    pub fn encode_b_vc(imm_bytes: i64) -> u32 {
        encode_b_cond_cc(super::Cond::VC, imm_bytes)
    }
    pub fn encode_cbz(rt: u32, imm_bytes: i64, is64: bool) -> u32 {
        let v = clamp_imm19_bytes(imm_bytes) as u32;
        (if is64 { 0xB4000000u32 } else { 0x34000000u32 }) | ((v & 0x7FFFF) << 5) | (rt & 0x1F)
    }
    pub fn encode_cbnz(rt: u32, imm_bytes: i64, is64: bool) -> u32 {
        let v = clamp_imm19_bytes(imm_bytes) as u32;
        (if is64 { 0xB5000000u32 } else { 0x35000000u32 }) | ((v & 0x7FFFF) << 5) | (rt & 0x1F)
    }
    pub fn encode_tbz(rt: u32, bit: u32, imm_bytes: i64) -> u32 {
        let v = clamp_imm14_bytes(imm_bytes) as u32;
        let b5 = ((bit >> 5) & 0x1) << 31;
        let b40 = (bit & 0x1F) << 19;
        0x36000000u32 | b5 | b40 | ((v & 0x3FFF) << 5) | (rt & 0x1F)
    }
    pub fn encode_tbnz(rt: u32, bit: u32, imm_bytes: i64) -> u32 {
        let v = clamp_imm14_bytes(imm_bytes) as u32;
        let b5 = ((bit >> 5) & 0x1) << 31;
        let b40 = (bit & 0x1F) << 19;
        0x37000000u32 | b5 | b40 | ((v & 0x3FFF) << 5) | (rt & 0x1F)
    }

    pub fn encode_ldr_x_unsigned(rt: u32, rn: u32, imm_bytes: i64) -> u32 {
        let scale = 3u32;
        let imm12 = ((imm_bytes / 8) as u32) & 0xFFF;
        (scale << 30) | 0xF9400000u32 | ((imm12 & 0xFFF) << 10) | ((rn & 0x1F) << 5) | (rt & 0x1F)
    }
    pub fn encode_str_x_unsigned(rt: u32, rn: u32, imm_bytes: i64) -> u32 {
        let scale = 3u32;
        let imm12 = ((imm_bytes / 8) as u32) & 0xFFF;
        (scale << 30) | 0xF9000000u32 | ((imm12 & 0xFFF) << 10) | ((rn & 0x1F) << 5) | (rt & 0x1F)
    }

    pub fn encode_csel(rd: u32, rn: u32, rm: u32, cond: u32, is64: bool) -> u32 {
        let sf = if is64 { 1u32 } else { 0u32 };
        (sf << 31)
            | 0x1A800000u32
            | ((rm & 0x1F) << 16)
            | ((cond & 0xF) << 12)
            | ((rn & 0x1F) << 5)
            | (rd & 0x1F)
    }
    pub fn encode_csinv(rd: u32, rn: u32, rm: u32, cond: u32, is64: bool) -> u32 {
        let sf = if is64 { 1u32 } else { 0u32 };
        (sf << 31)
            | 0x5A800000u32
            | ((rm & 0x1F) << 16)
            | ((cond & 0xF) << 12)
            | ((rn & 0x1F) << 5)
            | (rd & 0x1F)
    }
    pub fn encode_cneg(rd: u32, rn: u32, rm: u32, cond: u32, is64: bool) -> u32 {
        let sf = if is64 { 1u32 } else { 0u32 };
        (sf << 31)
            | 0x7A800000u32
            | ((rm & 0x1F) << 16)
            | ((cond & 0xF) << 12)
            | ((rn & 0x1F) << 5)
            | (rd & 0x1F)
    }
    pub fn encode_csinc(rd: u32, rn: u32, rm: u32, cond: u32, is64: bool) -> u32 {
        let sf = if is64 { 1u32 } else { 0u32 };
        (sf << 31)
            | 0x1A800400u32
            | ((rm & 0x1F) << 16)
            | ((cond & 0xF) << 12)
            | ((rn & 0x1F) << 5)
            | (rd & 0x1F)
    }
    fn invert_cond(cond: u32) -> u32 {
        cond ^ 1
    }
    pub fn encode_cset(rd: u32, cond: u32, is64: bool) -> u32 {
        encode_csinc(rd, 31, 31, invert_cond(cond), is64)
    }
    pub fn encode_csetm(rd: u32, cond: u32, is64: bool) -> u32 {
        encode_csinv(rd, 31, 31, invert_cond(cond), is64)
    }
    pub fn encode_cinc(rd: u32, rn: u32, cond: u32, is64: bool) -> u32 {
        encode_csinc(rd, rn, rn, invert_cond(cond), is64)
    }
    pub fn encode_cinv(rd: u32, rn: u32, cond: u32, is64: bool) -> u32 {
        encode_csinv(rd, rn, rn, cond, is64)
    }
    pub fn encode_cneg_alias(rd: u32, rn: u32, cond: u32, is64: bool) -> u32 {
        encode_cneg(rd, rn, rn, cond, is64)
    }
    pub fn encode_csdec(rd: u32, rn: u32, rm: u32, cond: u32, is64: bool) -> u32 {
        encode_csinc(rd, rn, rm, cond, is64)
    }
    pub fn encode_cdec(rd: u32, rn: u32, cond: u32, is64: bool) -> u32 {
        encode_csinc(rd, rn, rn, cond, is64)
    }
    pub fn encode_cinc_cc(rd: u32, rn: u32, cond: super::Cond, is64: bool) -> u32 {
        encode_cinc(rd, rn, cond as u32, is64)
    }
    pub fn encode_cdec_cc(rd: u32, rn: u32, cond: Cond, is64: bool) -> u32 {
        encode_cdec(rd, rn, cond as u32, is64)
    }
    pub fn encode_csdec_cc(rd: u32, rn: u32, rm: u32, cond: Cond, is64: bool) -> u32 {
        encode_csdec(rd, rn, rm, cond as u32, is64)
    }
    pub fn encode_cinv_cc(rd: u32, rn: u32, cond: Cond, is64: bool) -> u32 {
        encode_cinv(rd, rn, cond as u32, is64)
    }
    pub fn encode_cneg_alias_cc(rd: u32, rn: u32, cond: Cond, is64: bool) -> u32 {
        encode_cneg_alias(rd, rn, cond as u32, is64)
    }
    pub fn encode_cset_cc(rd: u32, cond: Cond, is64: bool) -> u32 {
        encode_cset(rd, cond as u32, is64)
    }
    pub fn encode_csetm_cc(rd: u32, cond: Cond, is64: bool) -> u32 {
        encode_csetm(rd, cond as u32, is64)
    }
    pub fn encode_cinc_eq(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::EQ, is64)
    }
    pub fn encode_cinc_ne(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::NE, is64)
    }
    pub fn encode_cinc_ge(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::GE, is64)
    }
    pub fn encode_cinc_lt(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::LT, is64)
    }
    pub fn encode_cdec_eq(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::EQ, is64)
    }
    pub fn encode_cdec_ne(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::NE, is64)
    }
    pub fn encode_cdec_ge(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::GE, is64)
    }
    pub fn encode_cdec_lt(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::LT, is64)
    }
    pub fn encode_cinc_hi(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::HI, is64)
    }
    pub fn encode_cinc_ls(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::LS, is64)
    }
    pub fn encode_cinc_gt(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::GT, is64)
    }
    pub fn encode_cinc_le(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::LE, is64)
    }
    pub fn encode_cdec_hi(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::HI, is64)
    }
    pub fn encode_cdec_ls(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::LS, is64)
    }
    pub fn encode_cdec_gt(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::GT, is64)
    }
    pub fn encode_cdec_le(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::LE, is64)
    }
    pub fn encode_cinc_mi(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::MI, is64)
    }
    pub fn encode_cinc_pl(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::PL, is64)
    }
    pub fn encode_cinc_vs(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::VS, is64)
    }
    pub fn encode_cinc_vc(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinc_cc(rd, rn, Cond::VC, is64)
    }
    pub fn encode_cdec_mi(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::MI, is64)
    }
    pub fn encode_cdec_pl(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::PL, is64)
    }
    pub fn encode_cdec_vs(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::VS, is64)
    }
    pub fn encode_cdec_vc(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cdec_cc(rd, rn, Cond::VC, is64)
    }
    pub fn encode_cinv_eq(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::EQ, is64)
    }
    pub fn encode_cinv_ne(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::NE, is64)
    }
    pub fn encode_cinv_ge(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::GE, is64)
    }
    pub fn encode_cinv_lt(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::LT, is64)
    }
    pub fn encode_cinv_hi(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::HI, is64)
    }
    pub fn encode_cinv_ls(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::LS, is64)
    }
    pub fn encode_cinv_gt(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::GT, is64)
    }
    pub fn encode_cinv_le(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::LE, is64)
    }
    pub fn encode_cinv_mi(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::MI, is64)
    }
    pub fn encode_cinv_pl(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::PL, is64)
    }
    pub fn encode_cinv_vs(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::VS, is64)
    }
    pub fn encode_cinv_vc(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cinv_cc(rd, rn, Cond::VC, is64)
    }
    pub fn encode_cneg_eq(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::EQ, is64)
    }
    pub fn encode_cneg_ne(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::NE, is64)
    }
    pub fn encode_cneg_ge(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::GE, is64)
    }
    pub fn encode_cneg_lt(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::LT, is64)
    }
    pub fn encode_cneg_hi(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::HI, is64)
    }
    pub fn encode_cneg_ls(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::LS, is64)
    }
    pub fn encode_cneg_gt(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::GT, is64)
    }
    pub fn encode_cneg_le(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::LE, is64)
    }
    pub fn encode_cneg_mi(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::MI, is64)
    }
    pub fn encode_cneg_pl(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::PL, is64)
    }
    pub fn encode_cneg_vs(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::VS, is64)
    }
    pub fn encode_cneg_vc(rd: u32, rn: u32, is64: bool) -> u32 {
        encode_cneg_alias_cc(rd, rn, Cond::VC, is64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::{AccessType, GuestAddr, GuestPhysAddr};

    struct TestMmu {
        insn: u32,
    }

    impl vm_core::AddressTranslator for TestMmu {
        fn translate(
            &mut self,
            va: GuestAddr,
            _access: AccessType,
        ) -> Result<GuestPhysAddr, VmError> {
            Ok(GuestPhysAddr(va.0))
        }
        fn flush_tlb(&mut self) {}
        fn flush_tlb_asid(&mut self, _asid: u16) {
            self.flush_tlb();
        }
        fn flush_tlb_page(&mut self, _va: GuestAddr) {
            self.flush_tlb();
        }
    }

    impl vm_core::MemoryAccess for TestMmu {
        fn read(&self, _pa: GuestAddr, _size: u8) -> Result<u64, VmError> {
            Ok(0)
        }
        fn write(&mut self, _pa: GuestAddr, _val: u64, _size: u8) -> Result<(), VmError> {
            Ok(())
        }
        fn load_reserved(&mut self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
            self.read(pa, size)
        }
        fn store_conditional(
            &mut self,
            _pa: GuestAddr,
            _val: u64,
            _size: u8,
        ) -> Result<bool, VmError> {
            Ok(false)
        }
        fn invalidate_reservation(&mut self, _pa: GuestAddr, _size: u8) {}
        fn fetch_insn(&self, _pc: GuestAddr) -> Result<u64, VmError> {
            Ok(self.insn as u64)
        }
        fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
            // 直接内存拷贝（如果物理地址是连续的）
            unsafe {
                let src_ptr = pa.0 as *const u8;
                let dst_ptr = buf.as_mut_ptr();
                std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, buf.len());
            }
            Ok(())
        }
        fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
            // 直接内存拷贝（如果物理地址是连续的）
            unsafe {
                let dst_ptr = pa.0 as *mut u8;
                let src_ptr = buf.as_ptr();
                std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, buf.len());
            }
            Ok(())
        }
        fn memory_size(&self) -> usize {
            4096
        }
        fn dump_memory(&self) -> Vec<u8> {
            vec![]
        }
        fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
    }

    impl vm_core::MmioManager for TestMmu {
        fn map_mmio(&self, _base: GuestAddr, _size: u64, _device: Box<dyn vm_core::MmioDevice>) {}
        fn poll_devices(&self) {}
    }

    impl vm_core::MmuAsAny for TestMmu {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    fn assemble_lse_base(
        base: u32,
        size_bits: u32,
        rs: u32,
        rn: u32,
        rt: u32,
        acq: bool,
        rel: bool,
    ) -> u32 {
        base | ((size_bits & 0x3) << 30)
            | ((rs & 0x1F) << 16)
            | ((rn & 0x1F) << 5)
            | (rt & 0x1F)
            | ((acq as u32) << 23)
            | ((rel as u32) << 22)
    }

    #[test]
    fn decode_lse_cas_acquire_word() {
        let pc: GuestAddr = GuestAddr(0x1000);
        // CAS (word), acquire only
        let insn = assemble_lse_base(0x3820_0400, 2, 3, 5, 7, true, false);
        let mmu = TestMmu { insn };

        let mut dec = Arm64Decoder::new();
        let block = dec
            .decode(&mmu, pc)
            .expect("Failed to decode ARM64 LSE CAL instruction");
        // Expect one AtomicCmpXchgOrder op
        assert!(matches!(block.ops[0], IROp::AtomicCmpXchgOrder { .. }));
        if let IROp::AtomicCmpXchgOrder {
            dst,
            base,
            expected,
            new,
            size,
            flags,
        } = block.ops[0]
        {
            assert_eq!(dst, 7);
            assert_eq!(expected, 3);
            assert_eq!(new, 7);
            assert_eq!(base, 5);
            assert_eq!(size, 4);
            assert!(matches!(flags.order, vm_ir::MemOrder::Acquire));
            assert!(flags.atomic);
        }
    }

    #[test]
    fn decode_lse_casal_acqrel_dword() {
        let pc: GuestAddr = GuestAddr(0x2000);
        // CASAL (doubleword), acquire+release
        let insn = assemble_lse_base(0x3820_0400, 3, 2, 4, 6, true, true);
        let mmu = TestMmu { insn };

        let mut dec = Arm64Decoder::new();
        let block = dec
            .decode(&mmu, pc)
            .expect("Failed to decode ARM64 LSE CASAL instruction");
        assert!(matches!(block.ops[0], IROp::AtomicCmpXchgOrder { .. }));
        if let IROp::AtomicCmpXchgOrder {
            dst,
            base,
            expected,
            new,
            size,
            flags,
        } = block.ops[0]
        {
            assert_eq!(dst, 6);
            assert_eq!(expected, 2);
            assert_eq!(new, 6);
            assert_eq!(base, 4);
            assert_eq!(size, 8);
            assert!(matches!(flags.order, vm_ir::MemOrder::AcqRel));
            assert!(flags.atomic);
        }
    }
}
