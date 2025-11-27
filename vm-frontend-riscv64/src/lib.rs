use vm_core::{Decoder, MMU, GuestAddr, Fault};
use vm_ir::{IRBlock, IROp, Terminator, MemFlags};

pub struct RiscvDecoder;

fn sext21(x: u32) -> i64 { if ((x >> 20) & 1) != 0 { (x as i64) | (!0i64 << 21) } else { x as i64 } }

impl Decoder for RiscvDecoder {
    type Block = IRBlock;
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, Fault> {
        let insn = mmu.fetch_insn(pc)? as u32;
        let mut reg_file = vm_ir::RegisterFile::new(32, vm_ir::RegisterMode::SSA);
        let opcode = insn & 0x7f;
        let rd = ((insn >> 7) & 0x1f) as u32;
        let funct3 = ((insn >> 12) & 0x7) as u32;
        let rs1 = ((insn >> 15) & 0x1f) as u32;
        let rs2 = ((insn >> 20) & 0x1f) as u32;
        let mut b = vm_ir::IRBuilder::new(pc);
        match opcode {
            0x37 => {
                let imm = ((insn & 0xfffff000) as i32) as i64;
                let imm = imm; // upper placed
                let dst = reg_file.write(rd as usize);
                b.push(IROp::AddImm { dst, src: reg_file.read(0), imm });
                b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
            }
            0x13 => {
                let imm = ((insn as i32) >> 20) as i64;
                match funct3 {
                    0 => { let dst = reg_file.write(rd as usize); b.push(IROp::AddImm { dst, src: reg_file.read(rs1 as usize), imm }); }
                    1 => { let dst = reg_file.write(rd as usize); b.push(IROp::SllImm { dst, src: reg_file.read(rs1 as usize), sh: (imm as u64 & 0x3f) as u8 }); }
                    5 => {
                        let funct7 = (insn >> 25) & 0x7f;
                        let dst = reg_file.write(rd as usize);
                        if funct7 == 0x00 { b.push(IROp::SrlImm { dst, src: reg_file.read(rs1 as usize), sh: (imm as u64 & 0x3f) as u8 }) }
                        else if funct7 == 0x20 { b.push(IROp::SraImm { dst, src: reg_file.read(rs1 as usize), sh: (imm as u64 & 0x3f) as u8 }) }
                    }
                    _ => {}
                }
                b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
            }
            0x33 => {
                let funct7 = (insn >> 25) & 0x7f;
                match (funct3, funct7) {
                    (0, 0x00) => { let dst = reg_file.write(rd as usize); b.push(IROp::Add { dst, src1: reg_file.read(rs1 as usize), src2: reg_file.read(rs2 as usize) }) },
                    (0, 0x20) => { let dst = reg_file.write(rd as usize); b.push(IROp::Sub { dst, src1: reg_file.read(rs1 as usize), src2: reg_file.read(rs2 as usize) }) },
                    (0, 0x01) => { let dst = reg_file.write(rd as usize); b.push(IROp::Mul { dst, src1: reg_file.read(rs1 as usize), src2: reg_file.read(rs2 as usize) }) }, // MUL
                    (4, 0x01) => { let dst = reg_file.write(rd as usize); b.push(IROp::Div { dst, src1: reg_file.read(rs1 as usize), src2: reg_file.read(rs2 as usize), signed: true }) }, // DIV
                    (5, 0x01) => { let dst = reg_file.write(rd as usize); b.push(IROp::Div { dst, src1: reg_file.read(rs1 as usize), src2: reg_file.read(rs2 as usize), signed: false }) }, // DIVU
                    (6, 0x01) => { let dst = reg_file.write(rd as usize); b.push(IROp::Rem { dst, src1: reg_file.read(rs1 as usize), src2: reg_file.read(rs2 as usize), signed: true }) }, // REM
                    (7, 0x01) => { let dst = reg_file.write(rd as usize); b.push(IROp::Rem { dst, src1: reg_file.read(rs1 as usize), src2: reg_file.read(rs2 as usize), signed: false }) }, // REMU
                    (1, 0x00) => { let dst = reg_file.write(rd as usize); b.push(IROp::Sll { dst, src: reg_file.read(rs1 as usize), shreg: reg_file.read(rs2 as usize) }) },
                    (5, 0x00) => { let dst = reg_file.write(rd as usize); b.push(IROp::Srl { dst, src: reg_file.read(rs1 as usize), shreg: reg_file.read(rs2 as usize) }) },
                    (5, 0x20) => { let dst = reg_file.write(rd as usize); b.push(IROp::Sra { dst, src: reg_file.read(rs1 as usize), shreg: reg_file.read(rs2 as usize) }) },
                    (7, 0x00) => { let dst = reg_file.write(rd as usize); b.push(IROp::And { dst, src1: reg_file.read(rs1 as usize), src2: reg_file.read(rs2 as usize) }) },
                    (6, 0x00) => { let dst = reg_file.write(rd as usize); b.push(IROp::Or { dst, src1: reg_file.read(rs1 as usize), src2: reg_file.read(rs2 as usize) }) },
                    (4, 0x00) => { let dst = reg_file.write(rd as usize); b.push(IROp::Xor { dst, src1: reg_file.read(rs1 as usize), src2: reg_file.read(rs2 as usize) }) },
                    _ => {}
                }
                b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
            }
            0x17 => {
                let imm = ((insn & 0xfffff000) as i32) as i64;
                let dst = reg_file.write(rd as usize);
                b.push(IROp::MovImm { dst, imm: pc });
                b.push(IROp::AddImm { dst, src: dst, imm });
                b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
            }
            0x03 => {
                let imm = ((insn as i32) >> 20) as i64;
                match funct3 {
                    0x0 => { let dst = reg_file.write(rd as usize); b.push(IROp::Load { dst, base: reg_file.read(rs1 as usize), size: 1, offset: imm, flags: MemFlags::default() }); },
                    0x1 => { let dst = reg_file.write(rd as usize); b.push(IROp::Load { dst, base: reg_file.read(rs1 as usize), size: 2, offset: imm, flags: MemFlags::default() }); },
                    0x2 => { let dst = reg_file.write(rd as usize); b.push(IROp::Load { dst, base: reg_file.read(rs1 as usize), size: 4, offset: imm, flags: MemFlags::default() }); },
                    0x3 => { let dst = reg_file.write(rd as usize); b.push(IROp::Load { dst, base: reg_file.read(rs1 as usize), size: 8, offset: imm, flags: MemFlags::default() }); },
                    _ => {}
                }
                b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
            }
            0x23 => {
                let imm = (((insn >> 7) & 0x1f) | (((insn >> 25) & 0x7f) << 5)) as i32;
                let imm = ((imm as i32) << 20 >> 20) as i64;
                match funct3 {
                    0x0 => b.push(IROp::Store { src: reg_file.read(rs2 as usize), base: reg_file.read(rs1 as usize), size: 1, offset: imm, flags: MemFlags::default() }),
                    0x1 => b.push(IROp::Store { src: reg_file.read(rs2 as usize), base: reg_file.read(rs1 as usize), size: 2, offset: imm, flags: MemFlags::default() }),
                    0x2 => b.push(IROp::Store { src: reg_file.read(rs2 as usize), base: reg_file.read(rs1 as usize), size: 4, offset: imm, flags: MemFlags::default() }),
                    0x3 => b.push(IROp::Store { src: reg_file.read(rs2 as usize), base: reg_file.read(rs1 as usize), size: 8, offset: imm, flags: MemFlags::default() }),
                    _ => {}
                }
                b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
            }
            0x63 => {
                let imm = ((((insn >> 31) & 0x1) << 12)
                    | (((insn >> 7) & 0x1) << 11)
                    | (((insn >> 25) & 0x3f) << 5)
                    | (((insn >> 8) & 0xf) << 1)) as i32;
                let imm = ((imm as i32) << 19 >> 19) as i64;
                let target = ((pc as i64).wrapping_add(imm)) as u64;
                match funct3 {
                    0x0 => { b.push(IROp::CmpEq { dst: 31, lhs: reg_file.read(rs1 as usize), rhs: reg_file.read(rs2 as usize) }); b.set_term(Terminator::CondJmp { cond: 31, target_true: target, target_false: pc.wrapping_add(4) }); }
                    0x1 => { b.push(IROp::CmpNe { dst: 31, lhs: reg_file.read(rs1 as usize), rhs: reg_file.read(rs2 as usize) }); b.set_term(Terminator::CondJmp { cond: 31, target_true: target, target_false: pc.wrapping_add(4) }); }
                    0x4 => { b.push(IROp::CmpLt { dst: 31, lhs: reg_file.read(rs1 as usize), rhs: reg_file.read(rs2 as usize) }); b.set_term(Terminator::CondJmp { cond: 31, target_true: target, target_false: pc.wrapping_add(4) }); }
                    0x5 => { b.push(IROp::CmpGe { dst: 31, lhs: reg_file.read(rs1 as usize), rhs: reg_file.read(rs2 as usize) }); b.set_term(Terminator::CondJmp { cond: 31, target_true: target, target_false: pc.wrapping_add(4) }); }
                    0x6 => { b.push(IROp::CmpLtU { dst: 31, lhs: reg_file.read(rs1 as usize), rhs: reg_file.read(rs2 as usize) }); b.set_term(Terminator::CondJmp { cond: 31, target_true: target, target_false: pc.wrapping_add(4) }); }
                    0x7 => { b.push(IROp::CmpGeU { dst: 31, lhs: reg_file.read(rs1 as usize), rhs: reg_file.read(rs2 as usize) }); b.set_term(Terminator::CondJmp { cond: 31, target_true: target, target_false: pc.wrapping_add(4) }); }
                    _ => { b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) }); }
                }
            }
            0x6f => {
                let i = insn;
                let raw = (((i >> 31) & 0x1) << 20) | (((i >> 21) & 0x3ff) << 1) | (((i >> 20) & 0x1) << 11) | (((i >> 12) & 0xff) << 12);
                let imm = sext21(raw);
                let dst = reg_file.write(rd as usize);
                b.push(IROp::MovImm { dst, imm: pc.wrapping_add(4) });
                b.set_term(Terminator::Jmp { target: ((pc as i64).wrapping_add(imm)) as u64 });
            }
            0x2f => {
                // AMO (Atomic Memory Operations)
                let funct5 = (insn >> 27) & 0x1f;
                let aq = (insn >> 26) & 0x1;
                let rl = (insn >> 25) & 0x1;
                let mut flags = MemFlags::default();
                flags.atomic = true;
                if aq != 0 { flags.fence_before = true; flags.order = vm_ir::MemOrder::Acquire; }
                if rl != 0 { flags.fence_after = true; flags.order = vm_ir::MemOrder::Release; }
                if aq != 0 && rl != 0 { flags.order = vm_ir::MemOrder::AcqRel; }
                
                let size = match funct3 {
                    2 => 4, // .W
                    3 => 8, // .D
                    _ => 4,
                };
                
                match funct5 {
                    0x02 => { // LR (Load Reserved)
                        let dst = reg_file.write(rd as usize);
                        let mut lr_flags = flags;
                        b.push(IROp::AtomicLoadReserve { dst, base: reg_file.read(rs1 as usize), offset: 0, size, flags: lr_flags });
                    }
                    0x03 => { // SC (Store Conditional)
                        let dst_flag = reg_file.write(rd as usize);
                        b.push(IROp::AtomicStoreCond { src: reg_file.read(rs2 as usize), base: reg_file.read(rs1 as usize), offset: 0, size, dst_flag, flags });
                    }
                    0x01 => { // AMOSWAP
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder { dst, base: reg_file.read(rs1 as usize), src: reg_file.read(rs2 as usize), op: vm_ir::AtomicOp::Xchg, size, flags });
                    }
                    0x00 => { // AMOADD
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder { dst, base: reg_file.read(rs1 as usize), src: reg_file.read(rs2 as usize), op: vm_ir::AtomicOp::Add, size, flags });
                    }
                    0x04 => { // AMOXOR
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder { dst, base: reg_file.read(rs1 as usize), src: reg_file.read(rs2 as usize), op: vm_ir::AtomicOp::Xor, size, flags });
                    }
                    0x0c => { // AMOAND
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder { dst, base: reg_file.read(rs1 as usize), src: reg_file.read(rs2 as usize), op: vm_ir::AtomicOp::And, size, flags });
                    }
                    0x08 => { // AMOOR
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder { dst, base: reg_file.read(rs1 as usize), src: reg_file.read(rs2 as usize), op: vm_ir::AtomicOp::Or, size, flags });
                    }
                    0x10 => { // AMOMIN
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder { dst, base: reg_file.read(rs1 as usize), src: reg_file.read(rs2 as usize), op: vm_ir::AtomicOp::MinS, size, flags });
                    }
                    0x14 => { // AMOMAX
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder { dst, base: reg_file.read(rs1 as usize), src: reg_file.read(rs2 as usize), op: vm_ir::AtomicOp::MaxS, size, flags });
                    }
                    0x18 => { // AMOMINU
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder { dst, base: reg_file.read(rs1 as usize), src: reg_file.read(rs2 as usize), op: vm_ir::AtomicOp::Min, size, flags });
                    }
                    0x1c => { // AMOMAXU
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder { dst, base: reg_file.read(rs1 as usize), src: reg_file.read(rs2 as usize), op: vm_ir::AtomicOp::Max, size, flags });
                    }
                    _ => {}
                }
                b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
            }
            0x67 => {
                let imm = ((insn as i32) >> 20) as i64;
                let dst = reg_file.write(rd as usize);
                b.push(IROp::MovImm { dst, imm: pc.wrapping_add(4) });
                
                let t29 = reg_file.write(29);
                b.push(IROp::AddImm { dst: t29, src: reg_file.read(rs1 as usize), imm });
                
                let t28 = reg_file.write(28);
                b.push(IROp::MovImm { dst: t28, imm: !1u64 });
                
                let t29_new = reg_file.write(29);
                b.push(IROp::And { dst: t29_new, src1: t29, src2: t28 });
                
                b.set_term(Terminator::JmpReg { base: t29_new, offset: 0 });
            }
            0x73 => {
                let csr = ((insn >> 20) & 0xFFF) as u16;
                match funct3 {
                    0x1 => { // CSRRW
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrWrite { csr, src: reg_file.read(rs1 as usize) });
                        b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
                    }
                    0x2 => { // CSRRS
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrSet { csr, src: reg_file.read(rs1 as usize) });
                        b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
                    }
                    0x3 => { // CSRRC
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrClear { csr, src: reg_file.read(rs1 as usize) });
                        b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
                    }
                    0x5 => { // CSRRWI
                        let zimm = ((insn >> 15) & 0x1f) as u32;
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrWriteImm { csr, imm: zimm, dst });
                        b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
                    }
                    0x6 => { // CSRRSI
                        let zimm = ((insn >> 15) & 0x1f) as u32;
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrSetImm { csr, imm: zimm, dst });
                        b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
                    }
                    0x7 => { // CSRRCI
                        let zimm = ((insn >> 15) & 0x1f) as u32;
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrClearImm { csr, imm: zimm, dst });
                        b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) });
                    }
                    0x0 => {
                        if insn == 0x00000073 { b.push(IROp::SysCall); b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) }); }
                        else if insn == 0x00100073 { b.push(IROp::DebugBreak); b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) }); }
                        else if insn == 0x30200073 { b.push(IROp::SysMret); b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) }); }
                        else if insn == 0x10200073 { b.push(IROp::SysSret); b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) }); }
                        else if insn == 0x10500073 { b.push(IROp::SysWfi); b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) }); }
                        else { b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) }); }
                    }
                    _ => { b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) }); }
                }
            }
            _ => { b.set_term(Terminator::Jmp { target: pc.wrapping_add(4) }); }
        }
        Ok(b.build())
    }
}

pub fn encode_jal(rd: u32, imm: i32) -> u32 {
    let min = (-(1 << 20)) << 1;
    let max = (((1 << 20) - 1) << 1) as i32;
    let mut v = imm;
    if (v & 1) != 0 { v &= !1; }
    if v < min { v = min; }
    if v > max { v = max; }
    let u = v as u32;
    let b20 = ((u >> 20) & 0x1) << 31;
    let b10_1 = ((u >> 1) & 0x3ff) << 21;
    let b11 = ((u >> 11) & 0x1) << 20;
    let b19_12 = ((u >> 12) & 0xff) << 12;
    b20 | b10_1 | b11 | b19_12 | (rd << 7) | 0x6f
}

fn clamp_i_imm(mut imm: i32) -> i32 {
    let min = -(1 << 11) - 1;
    let max = (1 << 11) - 1;
    if imm < min { imm = min; }
    if imm > max { imm = max; }
    imm
}

pub fn encode_jalr(rd: u32, rs1: u32, imm: i32) -> u32 {
    let v = clamp_i_imm(imm) as u32;
    (v << 20) | (rs1 << 15) | (0 << 12) | (rd << 7) | 0x67
}

pub fn encode_jalr_with_align(rd: u32, rs1: u32, imm: i32, align_even: bool) -> u32 {
    let mut v = clamp_i_imm(imm);
    if align_even { v &= !1; }
    ((v as u32) << 20) | (rs1 << 15) | (0 << 12) | (rd << 7) | 0x67
}

pub fn encode_auipc(rd: u32, upper: u32) -> u32 {
    ((upper & 0xfffff) << 12) | (rd << 7) | 0x17
}

pub mod api {
    pub use super::{
        encode_jal,
        encode_jalr,
        encode_jalr_with_align,
        encode_auipc,
        encode_branch,
        encode_beq,
        encode_bne,
        encode_blt,
        encode_bge,
        encode_bltu,
        encode_bgeu,
        encode_add,
        encode_sub,
        encode_addi,
        encode_lw,
        encode_sw,
    };
}

fn clamp_b_imm(mut imm: i32) -> i32 {
    let min = (-(1 << 12)) << 1;
    let max = (((1 << 12) - 1) << 1) as i32;
    if (imm & 1) != 0 { imm &= !1; }
    if imm < min { imm = min; }
    if imm > max { imm = max; }
    imm
}

pub fn encode_branch(funct3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let v = clamp_b_imm(imm) as u32;
    let b12 = ((v >> 12) & 0x1) << 31;
    let b11 = ((v >> 11) & 0x1) << 7;
    let b10_5 = ((v >> 5) & 0x3f) << 25;
    let b4_1 = ((v >> 1) & 0xf) << 8;
    b12 | b10_5 | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | b4_1 | b11 | 0x63
}

pub fn encode_beq(rs1: u32, rs2: u32, imm: i32) -> u32 { encode_branch(0x0, rs1, rs2, imm) }
pub fn encode_bne(rs1: u32, rs2: u32, imm: i32) -> u32 { encode_branch(0x1, rs1, rs2, imm) }
pub fn encode_blt(rs1: u32, rs2: u32, imm: i32) -> u32 { encode_branch(0x4, rs1, rs2, imm) }
pub fn encode_bge(rs1: u32, rs2: u32, imm: i32) -> u32 { encode_branch(0x5, rs1, rs2, imm) }
pub fn encode_bltu(rs1: u32, rs2: u32, imm: i32) -> u32 { encode_branch(0x6, rs1, rs2, imm) }
pub fn encode_bgeu(rs1: u32, rs2: u32, imm: i32) -> u32 { encode_branch(0x7, rs1, rs2, imm) }

pub fn encode_r_type(opcode: u32, funct3: u32, funct7: u32, rd: u32, rs1: u32, rs2: u32) -> u32 {
    (funct7 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

pub fn encode_i_type(opcode: u32, funct3: u32, rd: u32, rs1: u32, imm: i32) -> u32 {
    let imm = clamp_i_imm(imm) as u32;
    (imm << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

pub fn encode_s_type(opcode: u32, funct3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm = clamp_i_imm(imm) as u32;
    let imm11_5 = (imm >> 5) & 0x7f;
    let imm4_0 = imm & 0x1f;
    (imm11_5 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (imm4_0 << 7) | opcode
}

pub fn encode_add(rd: u32, rs1: u32, rs2: u32) -> u32 { encode_r_type(0x33, 0x0, 0x00, rd, rs1, rs2) }
pub fn encode_sub(rd: u32, rs1: u32, rs2: u32) -> u32 { encode_r_type(0x33, 0x0, 0x20, rd, rs1, rs2) }
pub fn encode_addi(rd: u32, rs1: u32, imm: i32) -> u32 { encode_i_type(0x13, 0x0, rd, rs1, imm) }
pub fn encode_lw(rd: u32, rs1: u32, imm: i32) -> u32 { encode_i_type(0x03, 0x2, rd, rs1, imm) }
pub fn encode_sw(rs1: u32, rs2: u32, imm: i32) -> u32 { encode_s_type(0x23, 0x2, rs1, rs2, imm) }
