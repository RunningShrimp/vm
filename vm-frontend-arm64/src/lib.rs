use vm_core::{Decoder, MMU, GuestAddr, Fault};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator, MemFlags};
pub enum Cond { EQ=0, NE=1, CS=2, CC=3, MI=4, PL=5, VS=6, VC=7, HI=8, LS=9, GE=10, LT=11, GT=12, LE=13 }

pub struct Arm64Decoder;

impl Decoder for Arm64Decoder {
    type Block = IRBlock;
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, Fault> {
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
                if shift == 1 { imm <<= 12; }
                
                if is_sub {
                    builder.push(IROp::AddImm { dst: rd, src: rn, imm: -imm });
                } else {
                    builder.push(IROp::AddImm { dst: rd, src: rn, imm });
                }
                current_pc += 4;
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
                if sf == 0 { result &= 0xFFFFFFFF; }
                builder.push(IROp::MovImm { dst: rd, imm: result });
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
            // 31 30 29 28 27 26 25 24 23 22 21 ... 10 9 ... 5 4 ... 0
            // size 1 1  1  0  0  1  0  1  0  imm12    Rn      Rt
            // Mask: 1011 1001 00... -> B90...
            // Check bits 22-29: 11100101 (LDR) or 11100100 (STR)
            // Actually bit 22 is 0 for unsigned offset? No.
            // LDR (imm unsigned): 1x11 1001 01...
            // STR (imm unsigned): 1x11 1001 00...
            if (insn & 0xBFA00000) == 0xB9000000 {
                let size_bit = (insn >> 30) & 1;
                let is_load = (insn & 0x00400000) != 0;
                let imm12 = (insn >> 10) & 0xFFF;
                let rn = (insn >> 5) & 0x1F;
                let rt = insn & 0x1F;
                
                let size = if size_bit == 1 { 8 } else { 4 };
                let offset = (imm12 as i64) * (size as i64);
                
                if is_load {
                    builder.push(IROp::Load { dst: rt, base: rn, offset, size, flags: MemFlags::default() });
                } else {
                    builder.push(IROp::Store { src: rt, base: rn, offset, size, flags: MemFlags::default() });
                }
                current_pc += 4;
                continue;
            }
            
            // B (Unconditional Branch)
            // 0001 01...
            if (insn & 0xFC000000) == 0x14000000 {
                let imm26 = insn & 0x03FFFFFF;
                let offset = ((imm26 << 6) as i32 >> 6) as i64 * 4;
                let target = current_pc.wrapping_add(offset as u64);
                builder.set_term(Terminator::Jmp { target });
                break;
            }

            // BL (Branch with Link)
            // 1001 01...
            if (insn & 0xFC000000) == 0x94000000 {
                let imm26 = insn & 0x03FFFFFF;
                let offset = ((imm26 << 6) as i32 >> 6) as i64 * 4;
                let target = current_pc.wrapping_add(offset as u64);
                // Link register is x30
                builder.push(IROp::MovImm { dst: 30, imm: (current_pc + 4) as u64 });
                builder.set_term(Terminator::Jmp { target });
                break;
            }

            // BR (Branch to Register)
            // 1101 0110 0001 1111 0000 00... Rn ...
            if (insn & 0xFFFFFC1F) == 0xD61F0000 {
                let rn = (insn >> 5) & 0x1F;
                builder.set_term(Terminator::JmpReg { base: rn, offset: 0 });
                break;
            }
            
            // RET
            // 1101 0110 0101 1111 0000 00...
            if (insn & 0xFFFFFC1F) == 0xD65F0000 {
                builder.set_term(Terminator::Ret);
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
                    0 => builder.push(IROp::And { dst: rd, src1: rn, src2: rm }), // AND
                    1 => builder.push(IROp::Or { dst: rd, src1: rn, src2: rm }),  // ORR
                    2 => builder.push(IROp::Xor { dst: rd, src1: rn, src2: rm }), // EOR
                    3 => builder.push(IROp::And { dst: rd, src1: rn, src2: rm }), // ANDS (flags ignored)
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
                
                builder.push(IROp::MovImm { dst: rd, imm: result });
                current_pc += 4;
                continue;
            }

            // ADD/SUB (shifted register)
            if (insn & 0x1F200000) == 0x0B000000 {
                 let is_sub = (insn & 0x40000000) != 0;
                 let rm = (insn >> 16) & 0x1F;
                 let rn = (insn >> 5) & 0x1F;
                 let rd = insn & 0x1F;
                 
                 if is_sub {
                     builder.push(IROp::Sub { dst: rd, src1: rn, src2: rm });
                 } else {
                     builder.push(IROp::Add { dst: rd, src1: rn, src2: rm });
                 }
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
                
                builder.push(IROp::Load { dst: rt, base: rn, offset, size: 8, flags: MemFlags::default() });
                builder.push(IROp::Load { dst: rt2, base: rn, offset: offset + 8, size: 8, flags: MemFlags::default() });
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
                
                builder.push(IROp::Store { src: rt, base: rn, offset, size: 8, flags: MemFlags::default() });
                builder.push(IROp::Store { src: rt2, base: rn, offset: offset + 8, size: 8, flags: MemFlags::default() });
                current_pc += 4;
                continue;
            }
            
            builder.set_term(Terminator::Fault { cause: 0 });
            break;
        }
        Ok(builder.build())
    }
}

pub mod api {
    pub use super::Cond;
    fn clamp_imm26_bytes(imm: i64) -> i32 {
        let mut v = imm >> 2;
        let min = -(1 << 25);
        let max = (1 << 25) - 1;
        if v < min { v = min; }
        if v > max { v = max; }
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
        if v < min { v = min; }
        if v > max { v = max; }
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
    pub fn encode_blr(rn: u32) -> u32 { 0xD63F0000u32 | ((rn & 0x1F) << 5) }
    pub fn encode_ret(rn: u32) -> u32 { 0xD65F0000u32 | ((rn & 0x1F) << 5) }
    pub mod cond {
        use super::{
            encode_cinc_eq, encode_cinc_ne, encode_cinc_ge, encode_cinc_lt, encode_cinc_hi, encode_cinc_ls, encode_cinc_gt, encode_cinc_le, encode_cinc_mi, encode_cinc_pl, encode_cinc_vs, encode_cinc_vc,
            encode_cdec_eq, encode_cdec_ne, encode_cdec_ge, encode_cdec_lt, encode_cdec_hi, encode_cdec_ls, encode_cdec_gt, encode_cdec_le, encode_cdec_mi, encode_cdec_pl, encode_cdec_vs, encode_cdec_vc,
            encode_cinv_eq, encode_cinv_ne, encode_cinv_ge, encode_cinv_lt, encode_cinv_hi, encode_cinv_ls, encode_cinv_gt, encode_cinv_le, encode_cinv_mi, encode_cinv_pl, encode_cinv_vs, encode_cinv_vc,
            encode_cneg_eq, encode_cneg_ne, encode_cneg_ge, encode_cneg_lt, encode_cneg_hi, encode_cneg_ls, encode_cneg_gt, encode_cneg_le, encode_cneg_mi, encode_cneg_pl, encode_cneg_vs, encode_cneg_vc,
        };
        pub mod cinc {
            use super::*;
            pub fn eq(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_eq(rd, rn, is64) }
            pub fn ne(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_ne(rd, rn, is64) }
            pub fn ge(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_ge(rd, rn, is64) }
            pub fn lt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_lt(rd, rn, is64) }
            pub fn hi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_hi(rd, rn, is64) }
            pub fn ls(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_ls(rd, rn, is64) }
            pub fn gt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_gt(rd, rn, is64) }
            pub fn le(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_le(rd, rn, is64) }
            pub fn mi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_mi(rd, rn, is64) }
            pub fn pl(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_pl(rd, rn, is64) }
            pub fn vs(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_vs(rd, rn, is64) }
            pub fn vc(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_vc(rd, rn, is64) }
        }
        pub mod cdec {
            use super::*;
            pub fn eq(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_eq(rd, rn, is64) }
            pub fn ne(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_ne(rd, rn, is64) }
            pub fn ge(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_ge(rd, rn, is64) }
            pub fn lt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_lt(rd, rn, is64) }
            pub fn hi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_hi(rd, rn, is64) }
            pub fn ls(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_ls(rd, rn, is64) }
            pub fn gt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_gt(rd, rn, is64) }
            pub fn le(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_le(rd, rn, is64) }
            pub fn mi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_mi(rd, rn, is64) }
            pub fn pl(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_pl(rd, rn, is64) }
            pub fn vs(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_vs(rd, rn, is64) }
            pub fn vc(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_vc(rd, rn, is64) }
        }
        pub mod cinv {
            use super::*;
            pub fn eq(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_eq(rd, rn, is64) }
            pub fn ne(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_ne(rd, rn, is64) }
            pub fn ge(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_ge(rd, rn, is64) }
            pub fn lt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_lt(rd, rn, is64) }
            pub fn hi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_hi(rd, rn, is64) }
            pub fn ls(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_ls(rd, rn, is64) }
            pub fn gt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_gt(rd, rn, is64) }
            pub fn le(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_le(rd, rn, is64) }
            pub fn mi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_mi(rd, rn, is64) }
            pub fn pl(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_pl(rd, rn, is64) }
            pub fn vs(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_vs(rd, rn, is64) }
            pub fn vc(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_vc(rd, rn, is64) }
        }
        pub mod cneg {
            use super::*;
            pub fn eq(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_eq(rd, rn, is64) }
            pub fn ne(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_ne(rd, rn, is64) }
            pub fn ge(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_ge(rd, rn, is64) }
            pub fn lt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_lt(rd, rn, is64) }
            pub fn hi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_hi(rd, rn, is64) }
            pub fn ls(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_ls(rd, rn, is64) }
            pub fn gt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_gt(rd, rn, is64) }
            pub fn le(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_le(rd, rn, is64) }
            pub fn mi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_mi(rd, rn, is64) }
            pub fn pl(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_pl(rd, rn, is64) }
            pub fn vs(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_vs(rd, rn, is64) }
            pub fn vc(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_vc(rd, rn, is64) }
        }
        pub mod eq {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::EQ as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::EQ as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::EQ as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::EQ as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_eq(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_eq(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_eq(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_eq(rd, rn, is64) }
        }
        pub mod ne {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::NE as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::NE as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::NE as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::NE as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_ne(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_ne(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_ne(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_ne(rd, rn, is64) }
        }
        pub mod ge {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::GE as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::GE as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::GE as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::GE as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_ge(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_ge(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_ge(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_ge(rd, rn, is64) }
        }
        pub mod lt {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::LT as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::LT as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::LT as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::LT as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_lt(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_lt(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_lt(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_lt(rd, rn, is64) }
        }
        pub mod hi {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::HI as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::HI as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::HI as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::HI as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_hi(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_hi(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_hi(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_hi(rd, rn, is64) }
        }
        pub mod ls {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::LS as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::LS as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::LS as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::LS as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_ls(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_ls(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_ls(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_ls(rd, rn, is64) }
        }
        pub mod gt {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::GT as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::GT as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::GT as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::GT as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_gt(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_gt(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_gt(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_gt(rd, rn, is64) }
        }
        pub mod le {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::LE as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::LE as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::LE as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::LE as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_le(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_le(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_le(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_le(rd, rn, is64) }
        }
        pub mod mi {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::MI as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::MI as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::MI as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::MI as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_mi(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_mi(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_mi(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_mi(rd, rn, is64) }
        }
        pub mod pl {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::PL as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::PL as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::PL as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::PL as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_pl(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_pl(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_pl(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_pl(rd, rn, is64) }
        }
        pub mod vs {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::VS as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::VS as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::VS as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::VS as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_vs(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_vs(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_vs(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_vs(rd, rn, is64) }
        }
        pub mod vc {
            pub fn csel(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csel(rd, rn, rm, super::super::Cond::VC as u32, is64) }
            pub fn csinv(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinv(rd, rn, rm, super::super::Cond::VC as u32, is64) }
            pub fn csinc(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_csinc(rd, rn, rm, super::super::Cond::VC as u32, is64) }
            pub fn cneg(rd: u32, rn: u32, rm: u32, is64: bool) -> u32 { super::super::encode_cneg(rd, rn, rm, super::super::Cond::VC as u32, is64) }
            pub fn cinc(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinc_vc(rd, rn, is64) }
            pub fn cdec(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cdec_vc(rd, rn, is64) }
            pub fn cinv(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cinv_vc(rd, rn, is64) }
            pub fn cneg_alias(rd: u32, rn: u32, is64: bool) -> u32 { super::super::encode_cneg_vc(rd, rn, is64) }
        }
    }
    fn clamp_imm19_bytes(imm: i64) -> i32 {
        let mut v = imm >> 2;
        let min = -(1 << 18);
        let max = (1 << 18) - 1;
        if v < min { v = min; }
        if v > max { v = max; }
        v as i32
    }
    fn clamp_imm14_bytes(imm: i64) -> i32 {
        let mut v = imm >> 2;
        let min = -(1 << 13);
        let max = (1 << 13) - 1;
        if v < min { v = min; }
        if v > max { v = max; }
        v as i32
    }
    pub fn encode_b_cond(cond: u32, imm_bytes: i64) -> u32 {
        let v = clamp_imm19_bytes(imm_bytes) as u32;
        0x54000000u32 | ((v & 0x7FFFF) << 5) | (cond & 0xF)
    }
    pub fn encode_b_cond_cc(cond: super::Cond, imm_bytes: i64) -> u32 { encode_b_cond(cond as u32, imm_bytes) }
    pub fn encode_b_eq(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::EQ, imm_bytes) }
    pub fn encode_b_ne(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::NE, imm_bytes) }
    pub fn encode_b_ge(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::GE, imm_bytes) }
    pub fn encode_b_lt(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::LT, imm_bytes) }
    pub fn encode_b_gt(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::GT, imm_bytes) }
    pub fn encode_b_le(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::LE, imm_bytes) }
    pub fn encode_b_hi(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::HI, imm_bytes) }
    pub fn encode_b_ls(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::LS, imm_bytes) }
    pub fn encode_b_mi(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::MI, imm_bytes) }
    pub fn encode_b_pl(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::PL, imm_bytes) }
    pub fn encode_b_vs(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::VS, imm_bytes) }
    pub fn encode_b_vc(imm_bytes: i64) -> u32 { encode_b_cond_cc(super::Cond::VC, imm_bytes) }
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

    pub fn encode_csel(rd: u32, rn: u32, rm: u32, cond: u32, is64: bool) -> u32 {
        let sf = if is64 { 1u32 } else { 0u32 };
        (sf << 31) | 0x1A800000u32 | ((rm & 0x1F) << 16) | ((cond & 0xF) << 12) | ((rn & 0x1F) << 5) | (rd & 0x1F)
    }
    pub fn encode_csinv(rd: u32, rn: u32, rm: u32, cond: u32, is64: bool) -> u32 {
        let sf = if is64 { 1u32 } else { 0u32 };
        (sf << 31) | 0x5A800000u32 | ((rm & 0x1F) << 16) | ((cond & 0xF) << 12) | ((rn & 0x1F) << 5) | (rd & 0x1F)
    }
    pub fn encode_cneg(rd: u32, rn: u32, rm: u32, cond: u32, is64: bool) -> u32 {
        let sf = if is64 { 1u32 } else { 0u32 };
        (sf << 31) | 0x7A800000u32 | ((rm & 0x1F) << 16) | ((cond & 0xF) << 12) | ((rn & 0x1F) << 5) | (rd & 0x1F)
    }
    pub fn encode_csinc(rd: u32, rn: u32, rm: u32, cond: u32, is64: bool) -> u32 {
        let sf = if is64 { 1u32 } else { 0u32 };
        (sf << 31) | 0x1A800400u32 | ((rm & 0x1F) << 16) | ((cond & 0xF) << 12) | ((rn & 0x1F) << 5) | (rd & 0x1F)
    }
    fn invert_cond(cond: u32) -> u32 { cond ^ 1 }
    pub fn encode_cset(rd: u32, cond: u32, is64: bool) -> u32 { encode_csinc(rd, 31, 31, invert_cond(cond), is64) }
    pub fn encode_csetm(rd: u32, cond: u32, is64: bool) -> u32 { encode_csinv(rd, 31, 31, invert_cond(cond), is64) }
    pub fn encode_cinc(rd: u32, rn: u32, cond: u32, is64: bool) -> u32 { encode_csinc(rd, rn, rn, invert_cond(cond), is64) }
    pub fn encode_cinv(rd: u32, rn: u32, cond: u32, is64: bool) -> u32 { encode_csinv(rd, rn, rn, cond, is64) }
    pub fn encode_cneg_alias(rd: u32, rn: u32, cond: u32, is64: bool) -> u32 { encode_cneg(rd, rn, rn, cond, is64) }
    pub fn encode_csdec(rd: u32, rn: u32, rm: u32, cond: u32, is64: bool) -> u32 { encode_csinc(rd, rn, rm, cond, is64) }
    pub fn encode_cdec(rd: u32, rn: u32, cond: u32, is64: bool) -> u32 { encode_csinc(rd, rn, rn, cond, is64) }
    pub fn encode_cinc_cc(rd: u32, rn: u32, cond: super::Cond, is64: bool) -> u32 { encode_cinc(rd, rn, cond as u32, is64) }
    pub fn encode_cdec_cc(rd: u32, rn: u32, cond: Cond, is64: bool) -> u32 { encode_cdec(rd, rn, cond as u32, is64) }
    pub fn encode_csdec_cc(rd: u32, rn: u32, rm: u32, cond: Cond, is64: bool) -> u32 { encode_csdec(rd, rn, rm, cond as u32, is64) }
    pub fn encode_cinv_cc(rd: u32, rn: u32, cond: Cond, is64: bool) -> u32 { encode_cinv(rd, rn, cond as u32, is64) }
    pub fn encode_cneg_alias_cc(rd: u32, rn: u32, cond: Cond, is64: bool) -> u32 { encode_cneg_alias(rd, rn, cond as u32, is64) }
    pub fn encode_cset_cc(rd: u32, cond: Cond, is64: bool) -> u32 { encode_cset(rd, cond as u32, is64) }
    pub fn encode_csetm_cc(rd: u32, cond: Cond, is64: bool) -> u32 { encode_csetm(rd, cond as u32, is64) }
    pub fn encode_cinc_eq(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::EQ, is64) }
    pub fn encode_cinc_ne(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::NE, is64) }
    pub fn encode_cinc_ge(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::GE, is64) }
    pub fn encode_cinc_lt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::LT, is64) }
    pub fn encode_cdec_eq(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::EQ, is64) }
    pub fn encode_cdec_ne(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::NE, is64) }
    pub fn encode_cdec_ge(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::GE, is64) }
    pub fn encode_cdec_lt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::LT, is64) }
    pub fn encode_cinc_hi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::HI, is64) }
    pub fn encode_cinc_ls(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::LS, is64) }
    pub fn encode_cinc_gt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::GT, is64) }
    pub fn encode_cinc_le(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::LE, is64) }
    pub fn encode_cdec_hi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::HI, is64) }
    pub fn encode_cdec_ls(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::LS, is64) }
    pub fn encode_cdec_gt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::GT, is64) }
    pub fn encode_cdec_le(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::LE, is64) }
    pub fn encode_cinc_mi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::MI, is64) }
    pub fn encode_cinc_pl(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::PL, is64) }
    pub fn encode_cinc_vs(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::VS, is64) }
    pub fn encode_cinc_vc(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinc_cc(rd, rn, Cond::VC, is64) }
    pub fn encode_cdec_mi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::MI, is64) }
    pub fn encode_cdec_pl(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::PL, is64) }
    pub fn encode_cdec_vs(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::VS, is64) }
    pub fn encode_cdec_vc(rd: u32, rn: u32, is64: bool) -> u32 { encode_cdec_cc(rd, rn, Cond::VC, is64) }
    pub fn encode_cinv_eq(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::EQ, is64) }
    pub fn encode_cinv_ne(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::NE, is64) }
    pub fn encode_cinv_ge(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::GE, is64) }
    pub fn encode_cinv_lt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::LT, is64) }
    pub fn encode_cinv_hi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::HI, is64) }
    pub fn encode_cinv_ls(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::LS, is64) }
    pub fn encode_cinv_gt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::GT, is64) }
    pub fn encode_cinv_le(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::LE, is64) }
    pub fn encode_cinv_mi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::MI, is64) }
    pub fn encode_cinv_pl(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::PL, is64) }
    pub fn encode_cinv_vs(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::VS, is64) }
    pub fn encode_cinv_vc(rd: u32, rn: u32, is64: bool) -> u32 { encode_cinv_cc(rd, rn, Cond::VC, is64) }
    pub fn encode_cneg_eq(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::EQ, is64) }
    pub fn encode_cneg_ne(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::NE, is64) }
    pub fn encode_cneg_ge(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::GE, is64) }
    pub fn encode_cneg_lt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::LT, is64) }
    pub fn encode_cneg_hi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::HI, is64) }
    pub fn encode_cneg_ls(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::LS, is64) }
    pub fn encode_cneg_gt(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::GT, is64) }
    pub fn encode_cneg_le(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::LE, is64) }
    pub fn encode_cneg_mi(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::MI, is64) }
    pub fn encode_cneg_pl(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::PL, is64) }
    pub fn encode_cneg_vs(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::VS, is64) }
    pub fn encode_cneg_vc(rd: u32, rn: u32, is64: bool) -> u32 { encode_cneg_alias_cc(rd, rn, Cond::VC, is64) }
}
