use vm_core::{ExecutionEngine, ExecResult, ExecStatus, ExecStats, MMU, AccessType, GuestAddr};
use vm_ir::{IRBlock, IROp, Terminator, AtomicOp};

pub struct Interpreter {
    regs: [u64; 32],
    intr_handler: Option<Box<dyn Fn(InterruptCtx, &mut Interpreter) -> ExecInterruptAction>>,
    intr_handler_ext: Option<Box<dyn Fn(InterruptCtxExt, &mut Interpreter) -> ExecInterruptAction>>,
    pub fence_acquire_count: u64,
    pub fence_release_count: u64,
    pub intr_mask_until: u32,
}

pub enum ExecInterruptAction { Continue, Abort, InjectState, Retry, Mask, Deliver }

pub struct InterruptCtx { pub vector: u32, pub pc: GuestAddr }
#[allow(dead_code)]
pub struct InterruptCtxExt { pub vector: u32, pub pc: GuestAddr, pub regs_ptr: *mut u64 }

impl Interpreter {
        pub fn new() -> Self { Self { regs: [0; 32], intr_handler: None, intr_handler_ext: None, fence_acquire_count: 0, fence_release_count: 0, intr_mask_until: 0 } }

    pub fn set_reg(&mut self, idx: u32, val: u64) {
        let hi = idx >> 16;
        let guest = if hi != 0 { hi } else { idx & 0x1F };
        if guest != 0 { self.regs[guest as usize] = val; }
    }
    pub fn get_reg(&self, idx: u32) -> u64 {
        let hi = idx >> 16;
        let guest = if hi != 0 { hi } else { idx & 0x1F };
        self.regs[guest as usize]
    }
    pub fn get_regs_ptr(&mut self) -> *mut u64 {
        self.regs.as_mut_ptr()
    }
    pub fn set_interrupt_handler<F: 'static + Fn(InterruptCtx, &mut Interpreter) -> ExecInterruptAction>(&mut self, f: F) {
        self.intr_handler = Some(Box::new(f));
    }
    pub fn set_interrupt_handler_ext<F: 'static + Fn(InterruptCtxExt, &mut Interpreter) -> ExecInterruptAction>(&mut self, f: F) {
        self.intr_handler_ext = Some(Box::new(f));
    }
    pub fn get_fence_counts(&self) -> (u64, u64) { (self.fence_acquire_count, self.fence_release_count) }
}

fn sign_extend(val: u64, bits: u64) -> i64 {
    let shift = 64 - bits;
    ((val << shift) as i64) >> shift
}

impl ExecutionEngine<IRBlock> for Interpreter {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock) -> ExecResult {
        let mut count = 0u64;
        for op in &block.ops {
            match op {
                IROp::Add { dst, src1, src2 } => {
                    let v = self.get_reg(*src1).wrapping_add(self.get_reg(*src2));
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Sub { dst, src1, src2 } => {
                    let v = self.get_reg(*src1).wrapping_sub(self.get_reg(*src2));
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Mul { dst, src1, src2 } => {
                    let v = self.get_reg(*src1).wrapping_mul(self.get_reg(*src2));
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Div { dst, src1, src2, signed } => {
                    let s1 = self.get_reg(*src1);
                    let s2 = self.get_reg(*src2);
                    let v = if *signed {
                        if s2 == 0 { u64::MAX } else { (s1 as i64).wrapping_div(s2 as i64) as u64 }
                    } else {
                        if s2 == 0 { u64::MAX } else { s1.wrapping_div(s2) }
                    };
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Rem { dst, src1, src2, signed } => {
                    let s1 = self.get_reg(*src1);
                    let s2 = self.get_reg(*src2);
                    let v = if *signed {
                        if s2 == 0 { s1 } else { (s1 as i64).wrapping_rem(s2 as i64) as u64 }
                    } else {
                        if s2 == 0 { s1 } else { s1.wrapping_rem(s2) }
                    };
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::And { dst, src1, src2 } => {
                    let v = self.get_reg(*src1) & self.get_reg(*src2);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Or { dst, src1, src2 } => {
                    let v = self.get_reg(*src1) | self.get_reg(*src2);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Xor { dst, src1, src2 } => {
                    let v = self.get_reg(*src1) ^ self.get_reg(*src2);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Not { dst, src } => {
                    let v = !self.get_reg(*src);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::AddImm { dst, src, imm } => {
                    let v = self.get_reg(*src).wrapping_add(*imm as u64);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::MulImm { dst, src, imm } => {
                    let v = self.get_reg(*src).wrapping_mul(*imm as u64);
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::MovImm { dst, imm } => {
                    self.set_reg(*dst, *imm);
                    count += 1;
                }
                IROp::SllImm { dst, src, sh } => {
                    let v = self.get_reg(*src) << *sh as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::SrlImm { dst, src, sh } => {
                    let v = self.get_reg(*src) >> *sh as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::SraImm { dst, src, sh } => {
                    let v = (self.get_reg(*src) as i64) >> *sh as i64;
                    self.set_reg(*dst, v as u64);
                    count += 1;
                }
                IROp::Sll { dst, src, shreg } => {
                    let sh = (self.get_reg(*shreg) & 0x3f) as u8;
                    let v = self.get_reg(*src) << sh as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Srl { dst, src, shreg } => {
                    let sh = (self.get_reg(*shreg) & 0x3f) as u8;
                    let v = self.get_reg(*src) >> sh as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Sra { dst, src, shreg } => {
                    let sh = (self.get_reg(*shreg) & 0x3f) as u8;
                    let v = (self.get_reg(*src) as i64) >> sh as i64;
                    self.set_reg(*dst, v as u64);
                    count += 1;
                }
                IROp::CmpEq { dst, lhs, rhs } => {
                    let v = (self.get_reg(*lhs) == self.get_reg(*rhs)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::CmpNe { dst, lhs, rhs } => {
                    let v = (self.get_reg(*lhs) != self.get_reg(*rhs)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::CmpLt { dst, lhs, rhs } => {
                    let v = ((self.get_reg(*lhs) as i64) < (self.get_reg(*rhs) as i64)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::CmpLtU { dst, lhs, rhs } => {
                    let v = (self.get_reg(*lhs) < self.get_reg(*rhs)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::CmpGe { dst, lhs, rhs } => {
                    let v = ((self.get_reg(*lhs) as i64) >= (self.get_reg(*rhs) as i64)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::CmpGeU { dst, lhs, rhs } => {
                    let v = (self.get_reg(*lhs) >= self.get_reg(*rhs)) as u64;
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Select { dst, cond, true_val, false_val } => {
                    let v = if self.get_reg(*cond) != 0 { self.get_reg(*true_val) } else { self.get_reg(*false_val) };
                    self.set_reg(*dst, v);
                    count += 1;
                }
                IROp::Load { dst, base, offset, size, flags } => {
                    let va = self.get_reg(*base).wrapping_add(*offset as u64);
                    if flags.align != 0 && (va % flags.align as u64 != 0) {
                        return ExecResult { status: ExecStatus::Fault(vm_core::Fault::AccessViolation), stats: ExecStats { executed_ops: count } };
                    }
                    if flags.atomic && !(matches!(size, 1 | 2 | 4 | 8) && flags.align == *size) {
                        return ExecResult { status: ExecStatus::Fault(vm_core::Fault::AccessViolation), stats: ExecStats { executed_ops: count } };
                    }
                    match flags.order { vm_ir::MemOrder::Acquire => { self.fence_acquire_count += 1; }, vm_ir::MemOrder::Release => {}, vm_ir::MemOrder::AcqRel => { self.fence_acquire_count += 1; }, vm_ir::MemOrder::None => {} }

                    let pa = match mmu.translate(va, AccessType::Read) { Ok(p) => p, Err(e) => {
                        return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                    } };
                    match mmu.read(pa, *size) {
                        Ok(v) => { self.set_reg(*dst, v); }
                        Err(e) => { return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } }; }
                    }
                    count += 1;
                }
                IROp::Store { src, base, offset, size, flags } => {
                    let va = self.get_reg(*base).wrapping_add(*offset as u64);
                    if flags.align != 0 && (va % flags.align as u64 != 0) {
                        return ExecResult { status: ExecStatus::Fault(vm_core::Fault::AccessViolation), stats: ExecStats { executed_ops: count } };
                    }
                    if flags.atomic && !(matches!(size, 1 | 2 | 4 | 8) && flags.align == *size) {
                        return ExecResult { status: ExecStatus::Fault(vm_core::Fault::AccessViolation), stats: ExecStats { executed_ops: count } };
                    }
                    match flags.order { vm_ir::MemOrder::Release => { self.fence_release_count += 1; }, vm_ir::MemOrder::AcqRel => { self.fence_release_count += 1; }, _ => {} }
                    let pa = match mmu.translate(va, AccessType::Write) { Ok(p) => p, Err(e) => {
                        return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                    } };
                    match mmu.write(pa, self.get_reg(*src), *size) {
                        Ok(()) => {}
                        Err(e) => { return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } }; }
                    }
                    count += 1;
                }
                IROp::SysCall => {
                    return ExecResult { status: ExecStatus::Fault(vm_core::Fault::AccessViolation), stats: ExecStats { executed_ops: count } }; // TODO: Handle syscall
                }
                IROp::DebugBreak => {
                    return ExecResult { status: ExecStatus::Ok, stats: ExecStats { executed_ops: count } };
                }
                IROp::TlbFlush { vaddr: _ } => {
                    // Interpreter uses SoftMMU which might not need explicit flush or we can't easily flush it here without MMU API change.
                    // For now, treat as NOP or maybe we should add a flush method to MMU trait?
                    // Given the current MMU trait doesn't have flush, we'll just count it.
                    count += 1;
                }
                IROp::VecAdd { dst, src1, src2, element_size } => {
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes = 64 / lane_bits;
                    let mut acc = 0u64;
                    for i in 0..lanes {
                        let shift = i * lane_bits;
                        let mask = ((1u128 << lane_bits) - 1) as u64;
                        let av = (a >> shift) & mask;
                        let bv = (b >> shift) & mask;
                        let rv = av.wrapping_add(bv) & mask;
                        acc |= rv << shift;
                    }
                    self.set_reg(*dst, acc);
                    count += 1;
                }
                IROp::VecSub { dst, src1, src2, element_size } => {
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes = 64 / lane_bits;
                    let mut acc = 0u64;
                    for i in 0..lanes {
                        let shift = i * lane_bits;
                        let mask = ((1u128 << lane_bits) - 1) as u64;
                        let av = (a >> shift) & mask;
                        let bv = (b >> shift) & mask;
                        let rv = av.wrapping_sub(bv) & mask;
                        acc |= rv << shift;
                    }
                    self.set_reg(*dst, acc);
                    count += 1;
                }
                IROp::VecMul { dst, src1, src2, element_size } => {
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes = 64 / lane_bits;
                    let mut acc = 0u64;
                    for i in 0..lanes {
                        let shift = i * lane_bits;
                        let mask = ((1u128 << lane_bits) - 1) as u64;
                        let av = (a >> shift) & mask;
                        let bv = (b >> shift) & mask;
                        let rv = av.wrapping_mul(bv) & mask;
                        acc |= rv << shift;
                    }
                    self.set_reg(*dst, acc);
                    count += 1;
                }
                IROp::VecMulSat { dst, src1, src2, element_size, signed } => {
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes = 64 / lane_bits;
                    let mut acc = 0u64;
                    for i in 0..lanes {
                        let shift = i * lane_bits;
                        let mask = ((1u128 << lane_bits) - 1) as u64;
                        let av = (a >> shift) & mask;
                        let bv = (b >> shift) & mask;
                        let rv = if *signed {
                            let max = ((1i128 << (lane_bits - 1)) - 1) as i128;
                            let min = (-(1i128 << (lane_bits - 1))) as i128;
                            let sa = sign_extend(av, lane_bits) as i128;
                            let sb = sign_extend(bv, lane_bits) as i128;
                            let prod = sa * sb;
                            let clamped = if prod > max { max } else if prod < min { min } else { prod };
                            (clamped as i128 as u128 as u64) & mask
                        } else {
                            let max = ((1u128 << lane_bits) - 1) as u128;
                            let prod = (av as u128) * (bv as u128);
                            let clamped = if prod > max { max } else { prod };
                            (clamped as u64) & mask
                        };
                        acc |= rv << shift;
                    }
                    self.set_reg(*dst, acc);
                    count += 1;
                }
                IROp::Vec128Add { dst_lo, dst_hi, src1_lo, src1_hi, src2_lo, src2_hi, element_size, signed } => {
                    let a = ((self.get_reg(*src1_hi) as u128) << 64) | (self.get_reg(*src1_lo) as u128);
                    let b = ((self.get_reg(*src2_hi) as u128) << 64) | (self.get_reg(*src2_lo) as u128);
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes = (128 / lane_bits) as usize;
                    let mut acc: u128 = 0;
                    for i in 0..lanes {
                        let shift = i * lane_bits as usize;
                        let mask = ((1u128 << lane_bits) - 1) as u128;
                        let av = (a >> shift) & mask;
                        let bv = (b >> shift) & mask;
                        let rv = if *signed {
                            let max = ((1i128 << (lane_bits - 1)) - 1) as i128;
                            let min = (-(1i128 << (lane_bits - 1))) as i128;
                            let sa = ((av << (128 - lane_bits)) as i128) >> (128 - lane_bits);
                            let sb = ((bv << (128 - lane_bits)) as i128) >> (128 - lane_bits);
                            let sum = sa + sb;
                            let clamped = if sum > max { max } else if sum < min { min } else { sum } as i128;
                            (clamped as i128 as u128) & mask
                        } else {
                            let max = ((1u128 << lane_bits) - 1) as u128;
                            let sum = av + bv;
                            let clamped = if sum > max { max } else { sum };
                            clamped & mask
                        };
                        acc |= rv << shift;
                    }
                    let lo = (acc & ((1u128 << 64) - 1)) as u64;
                    let hi = (acc >> 64) as u64;
                    self.set_reg(*dst_lo, lo);
                    self.set_reg(*dst_hi, hi);
                    count += 1;
                }
                IROp::AtomicCmpXchg { dst, base, expected, new, size } => {
                    let va = self.get_reg(*base);
                    let pa_r = match mmu.translate(va, AccessType::Read) { Ok(p) => p, Err(e) => {
                        return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                    } };
                    let old = match mmu.read(pa_r, *size) { Ok(v) => v, Err(e) => {
                        return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                    } };
                    if old == self.get_reg(*expected) {
                        let pa_w = match mmu.translate(va, AccessType::Write) { Ok(p) => p, Err(e) => {
                            return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                        } };
                        if let Err(e) = mmu.write(pa_w, self.get_reg(*new), *size) {
                            return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                        }
                    }
                    self.set_reg(*dst, old);
                    count += 1;
                }
                IROp::AtomicCmpXchgFlag { dst_old, dst_flag, base, expected, new, size } => {
                    let va = self.get_reg(*base);
                    let pa_r = match mmu.translate(va, AccessType::Read) { Ok(p) => p, Err(e) => {
                        return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                    } };
                    let old = match mmu.read(pa_r, *size) { Ok(v) => v, Err(e) => {
                        return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                    } };
                    let mut success = 0;
                    if old == self.get_reg(*expected) {
                        let pa_w = match mmu.translate(va, AccessType::Write) { Ok(p) => p, Err(e) => {
                            return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                        } };
                        if let Err(e) = mmu.write(pa_w, self.get_reg(*new), *size) {
                            return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                        }
                        success = 1;
                    }
                    self.set_reg(*dst_old, old);
                    self.set_reg(*dst_flag, success);
                    count += 1;
                }
                IROp::AtomicRmwFlag { dst_old, dst_flag, base, src, op, size } => {
                    let va = self.get_reg(*base);
                    let pa_r = match mmu.translate(va, AccessType::Read) { Ok(p) => p, Err(e) => {
                        return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                    } };
                    let old = match mmu.read(pa_r, *size) { Ok(v) => v, Err(e) => {
                        return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                    } };
                    let newv = match op {
                        vm_ir::AtomicOp::And => old & self.get_reg(*src),
                        vm_ir::AtomicOp::Or => old | self.get_reg(*src),
                        vm_ir::AtomicOp::Xor => old ^ self.get_reg(*src),
                        vm_ir::AtomicOp::Min => old.min(self.get_reg(*src)),
                        vm_ir::AtomicOp::Max => old.max(self.get_reg(*src)),
                        vm_ir::AtomicOp::MinS => {
                            let bits = (*size as u64) * 8;
                            let a = sign_extend(old, bits) as i64;
                            let b = sign_extend(self.get_reg(*src), bits) as i64;
                            let r = if a < b { a } else { b } as i64;
                            (r as i64 as u64) & (((1u128 << bits) - 1) as u64)
                        }
                        vm_ir::AtomicOp::MaxS => {
                            let bits = (*size as u64) * 8;
                            let a = sign_extend(old, bits) as i64;
                            let b = sign_extend(self.get_reg(*src), bits) as i64;
                            let r = if a > b { a } else { b } as i64;
                            (r as i64 as u64) & (((1u128 << bits) - 1) as u64)
                        }
                        _ => old,
                    };
                    let pa_w = match mmu.translate(va, AccessType::Write) { Ok(p) => p, Err(e) => {
                        return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                    } };
                    if let Err(e) = mmu.write(pa_w, newv, *size) {
                        return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: count } };
                    }
                    self.set_reg(*dst_old, old);
                    self.set_reg(*dst_flag, 1);
                    count += 1;
                }
                IROp::Vec256Add { dst0, dst1, dst2, dst3, src10, src11, src12, src13, src20, src21, src22, src23, element_size, signed } => {
                    let mut out = [0u64; 4];
                    let src_a = [self.get_reg(*src10), self.get_reg(*src11), self.get_reg(*src12), self.get_reg(*src13)];
                    let src_b = [self.get_reg(*src20), self.get_reg(*src21), self.get_reg(*src22), self.get_reg(*src23)];
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes_per = (64 / lane_bits) as usize;
                    for c in 0..4 {
                        let mut acc_chunk = 0u64;
                        for i in 0..lanes_per {
                            let shift = i * lane_bits as usize;
                            let mask = ((1u128 << lane_bits) - 1) as u64;
                            let av = (src_a[c] >> shift) & mask;
                            let bv = (src_b[c] >> shift) & mask;
                            let rv = if *signed {
                                let max = ((1i128 << (lane_bits - 1)) - 1) as i128;
                                let min = (-(1i128 << (lane_bits - 1))) as i128;
                                let sa = sign_extend(av, lane_bits) as i128;
                                let sb = sign_extend(bv, lane_bits) as i128;
                                let sum = sa + sb;
                                let clamped = if sum > max { max } else if sum < min { min } else { sum };
                                (clamped as i128 as u128 as u64) & mask
                            } else {
                                let max = ((1u128 << lane_bits) - 1) as u128;
                                let sum = (av as u128) + (bv as u128);
                                let clamped = if sum > max { max } else { sum };
                                (clamped as u64) & mask
                            };
                            acc_chunk |= rv << shift;
                        }
                        out[c] = acc_chunk;
                    }
                    self.set_reg(*dst0, out[0]);
                    self.set_reg(*dst1, out[1]);
                    self.set_reg(*dst2, out[2]);
                    self.set_reg(*dst3, out[3]);
                    count += 1;
                }
                IROp::Vec256Sub { dst0, dst1, dst2, dst3, src10, src11, src12, src13, src20, src21, src22, src23, element_size, signed } => {
                    let mut out = [0u64; 4];
                    let src_a = [self.get_reg(*src10), self.get_reg(*src11), self.get_reg(*src12), self.get_reg(*src13)];
                    let src_b = [self.get_reg(*src20), self.get_reg(*src21), self.get_reg(*src22), self.get_reg(*src23)];
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes_per = (64 / lane_bits) as usize;
                    for c in 0..4 {
                        let mut acc_chunk = 0u64;
                        for i in 0..lanes_per {
                            let shift = i * lane_bits as usize;
                            let mask = ((1u128 << lane_bits) - 1) as u64;
                            let av = (src_a[c] >> shift) & mask;
                            let bv = (src_b[c] >> shift) & mask;
                            let rv = if *signed {
                                let max = ((1i128 << (lane_bits - 1)) - 1) as i128;
                                let min = (-(1i128 << (lane_bits - 1))) as i128;
                                let sa = sign_extend(av, lane_bits) as i128;
                                let sb = sign_extend(bv, lane_bits) as i128;
                                let diff = sa - sb;
                                let clamped = if diff > max { max } else if diff < min { min } else { diff };
                                (clamped as i128 as u128 as u64) & mask
                            } else {
                                let diff = (av as i128) - (bv as i128);
                                let clamped = if diff < 0 { 0u128 } else { diff as u128 };
                                (clamped as u64) & mask
                            };
                            acc_chunk |= rv << shift;
                        }
                        out[c] = acc_chunk;
                    }
                    self.set_reg(*dst0, out[0]);
                    self.set_reg(*dst1, out[1]);
                    self.set_reg(*dst2, out[2]);
                    self.set_reg(*dst3, out[3]);
                    count += 1;
                }
                IROp::Vec256Mul { dst0, dst1, dst2, dst3, src10, src11, src12, src13, src20, src21, src22, src23, element_size, signed } => {
                    let mut out = [0u64; 4];
                    let src_a = [self.get_reg(*src10), self.get_reg(*src11), self.get_reg(*src12), self.get_reg(*src13)];
                    let src_b = [self.get_reg(*src20), self.get_reg(*src21), self.get_reg(*src22), self.get_reg(*src23)];
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes_per = (64 / lane_bits) as usize;
                    for c in 0..4 {
                        let mut acc_chunk = 0u64;
                        for i in 0..lanes_per {
                            let shift = i * lane_bits as usize;
                            let mask = ((1u128 << lane_bits) - 1) as u64;
                            let av = (src_a[c] >> shift) & mask;
                            let bv = (src_b[c] >> shift) & mask;
                            let rv = if *signed {
                                let max = ((1i128 << (lane_bits - 1)) - 1) as i128;
                                let min = (-(1i128 << (lane_bits - 1))) as i128;
                                let sa = sign_extend(av, lane_bits) as i128;
                                let sb = sign_extend(bv, lane_bits) as i128;
                                let prod = sa * sb;
                                let clamped = if prod > max { max } else if prod < min { min } else { prod };
                                (clamped as i128 as u128 as u64) & mask
                            } else {
                                let max = ((1u128 << lane_bits) - 1) as u128;
                                let prod = (av as u128) * (bv as u128);
                                let clamped = if prod > max { max } else { prod };
                                (clamped as u64) & mask
                            };
                            acc_chunk |= rv << shift;
                        }
                        out[c] = acc_chunk;
                    }
                    self.set_reg(*dst0, out[0]);
                    self.set_reg(*dst1, out[1]);
                    self.set_reg(*dst2, out[2]);
                    self.set_reg(*dst3, out[3]);
                    count += 1;
                }
                IROp::VecAddSat { dst, src1, src2, element_size, signed } => {
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes = 64 / lane_bits;
                    let mut acc = 0u64;
                    for i in 0..lanes {
                        let shift = i * lane_bits;
                        let mask = ((1u128 << lane_bits) - 1) as u64;
                        let av = (a >> shift) & mask;
                        let bv = (b >> shift) & mask;
                        let rv = if *signed {
                            let max = ((1i128 << (lane_bits - 1)) - 1) as i128;
                            let min = (-(1i128 << (lane_bits - 1))) as i128;
                            let sa = sign_extend(av, lane_bits) as i128;
                            let sb = sign_extend(bv, lane_bits) as i128;
                            let sum = sa + sb;
                            let clamped = if sum > max { max } else if sum < min { min } else { sum };
                            (clamped as i128 as u128 as u64) & mask
                        } else {
                            let max = ((1u128 << lane_bits) - 1) as u128;
                            let sum = (av as u128) + (bv as u128);
                            let clamped = if sum > max { max } else { sum };
                            (clamped as u64) & mask
                        };
                        acc |= rv << shift;
                    }
                    self.set_reg(*dst, acc);
                    count += 1;
                }
                IROp::VecSubSat { dst, src1, src2, element_size, signed } => {
                    let a = self.get_reg(*src1);
                    let b = self.get_reg(*src2);
                    let es = *element_size as u64;
                    let lane_bits = (es * 8) as u64;
                    let lanes = 64 / lane_bits;
                    let mut acc = 0u64;
                    for i in 0..lanes {
                        let shift = i * lane_bits;
                        let mask = ((1u128 << lane_bits) - 1) as u64;
                        let av = (a >> shift) & mask;
                        let bv = (b >> shift) & mask;
                        let rv = if *signed {
                            let max = ((1i128 << (lane_bits - 1)) - 1) as i128;
                            let min = (-(1i128 << (lane_bits - 1))) as i128;
                            let sa = sign_extend(av, lane_bits) as i128;
                            let sb = sign_extend(bv, lane_bits) as i128;
                            let diff = sa - sb;
                            let clamped = if diff > max { max } else if diff < min { min } else { diff };
                            (clamped as i128 as u128 as u64) & mask
                        } else {
                            let diff = (av as i128) - (bv as i128);
                            let clamped = if diff < 0 { 0u128 } else { diff as u128 };
                            (clamped as u64) & mask
                        };
                        acc |= rv << shift;
                    }
                    self.set_reg(*dst, acc);
                    count += 1;
                }
                IROp::AtomicRMW { dst, base, src, op, size } => {
                    let addr = self.get_reg(*base);
                    let val = self.get_reg(*src);
                    let current = match mmu.read(addr, *size) {
                        Ok(v) => v,
                        Err(e) => return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: 0 } },
                    };
                    let res = match op {
                        AtomicOp::Add => current.wrapping_add(val),
                        AtomicOp::Sub => current.wrapping_sub(val),
                        AtomicOp::And => current & val,
                        AtomicOp::Or => current | val,
                        AtomicOp::Xor => current ^ val,
                        AtomicOp::Xchg => val,
                        AtomicOp::Min => current.min(val),
                        AtomicOp::Max => current.max(val),
                        AtomicOp::MinS => {
                            let bits = (*size as u64) * 8;
                            let a = sign_extend(current, bits) as i64;
                            let b = sign_extend(val, bits) as i64;
                            let r = if a < b { a } else { b } as i64;
                            (r as i64 as u64)
                        }
                        AtomicOp::MaxS => {
                            let bits = (*size as u64) * 8;
                            let a = sign_extend(current, bits) as i64;
                            let b = sign_extend(val, bits) as i64;
                            let r = if a > b { a } else { b } as i64;
                            (r as i64 as u64)
                        }
                        _ => current,
                    };
                    let mask = if *size == 8 { !0 } else { (1u64 << (*size * 8)) - 1 };
                    let res = res & mask;
                    if let Err(e) = mmu.write(addr, res, *size) {
                        return ExecResult { status: ExecStatus::Fault(e), stats: ExecStats { executed_ops: 0 } };
                    }
                    self.set_reg(*dst, current);
                }
                _ => {}
            }
        }
        ExecResult { status: ExecStatus::Ok, stats: ExecStats { executed_ops: count } }
    }
}

pub fn run_chain(decoder: &mut dyn crate_decoder::DecoderDyn, mmu: &mut dyn MMU, interp: &mut Interpreter, mut pc: u64, max_blocks: usize) -> ExecResult {
    let mut total = 0u64;
    for _ in 0..max_blocks {
        let block = decoder.decode_dyn(mmu, pc);
        let res = interp.run(mmu, &block);
        total += res.stats.executed_ops;
        if let ExecStatus::Fault(_) = res.status { return res; }
        
        match block.term {
            Terminator::Jmp { target } => { pc = target; }
            Terminator::JmpReg { base, offset } => {
                pc = interp.get_reg(base).wrapping_add(offset as u64);
            }
            Terminator::CondJmp { cond, target_true, target_false } => {
                if interp.get_reg(cond) != 0 { pc = target_true; } else { pc = target_false; }
            }
            Terminator::Ret => { break; }
            Terminator::Fault { cause: _ } => { return ExecResult { status: ExecStatus::Fault(vm_core::Fault::AccessViolation), stats: ExecStats { executed_ops: total } }; }
            Terminator::Interrupt { vector } => {
                let mut tmp_ext = interp.intr_handler_ext.take();
                let action = if let Some(h) = tmp_ext.take() {
                    let res = h(InterruptCtxExt { vector, pc, regs_ptr: interp.get_regs_ptr() }, interp);
                    interp.intr_handler_ext = Some(h);
                    res
                } else {
                    let mut tmp = interp.intr_handler.take();
                    if let Some(h) = tmp.take() {
                        let res = h(InterruptCtx { vector, pc }, interp);
                        interp.intr_handler = Some(h);
                        res
                    } else { ExecInterruptAction::Abort }
                };
                match action {
                    ExecInterruptAction::Continue => { pc = pc.wrapping_add(4); continue; }
                    ExecInterruptAction::InjectState => { pc = pc.wrapping_add(4); continue; }
                    ExecInterruptAction::Retry => { continue; }
                    ExecInterruptAction::Mask => { pc = pc.wrapping_add(4); continue; }
                    ExecInterruptAction::Deliver => { pc = pc.wrapping_add(4); continue; }
                    ExecInterruptAction::Abort => {
                        return ExecResult { status: ExecStatus::Fault(vm_core::Fault::AccessViolation), stats: ExecStats { executed_ops: total } };
                    }
                }
            }
            Terminator::Call { target, ret_pc: _ } => { pc = target; }
        }
    }
    ExecResult { status: ExecStatus::Ok, stats: ExecStats { executed_ops: total } }
}

pub mod crate_decoder {
    use vm_core::{Decoder, MMU, GuestAddr};
    use vm_ir::{IRBlock, Terminator};
    pub trait DecoderDyn {
        fn decode_dyn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> IRBlock;
    }
    impl<T: Decoder<Block = IRBlock>> DecoderDyn for T {
        fn decode_dyn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> IRBlock {
            match <T as Decoder>::decode(self, mmu, pc) {
                Ok(b) => b,
                Err(e) => {
                    let cause = match e { vm_core::Fault::InvalidOpcode => 3, vm_core::Fault::PageFault => 2, vm_core::Fault::AccessViolation => 1 } as u64;
                    IRBlock { start_pc: pc, ops: vec![], term: Terminator::Fault { cause } }
                }
            }
        }
    }
}
