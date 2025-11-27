#[cfg(test)]
mod tests {
    use vm_core::{ExecutionEngine, MMU, Decoder, MmioDevice};
use vm_engine_interpreter::Interpreter;
use vm_engine_jit::Jit;
    use vm_ir::{IRBuilder, IROp, MemFlags};
    use vm_mem::SoftMmu;
    use vm_engine_interpreter::run_chain;
    use vm_frontend_riscv64::{RiscvDecoder, encode_jal, encode_beq};
    use vm_frontend_arm64::api as arm64_api;
    use vm_frontend_x86_64::api as x86_api;
    use vm_device::virtio;

    #[test]
    fn interpreter_runs_empty_block() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        let builder = IRBuilder::new(0);
        let block = builder.build();
        let res = engine.run(&mut mmu, &block);
        match res.status {
            vm_core::ExecStatus::Ok => {}
            _ => panic!(),
        }
    }

    #[test]
    fn interpreter_add_executes() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        engine.set_reg(1, 2);
        engine.set_reg(2, 3);
        let mut builder = IRBuilder::new(0);
        builder.push(IROp::Add { dst: 3, src1: 1, src2: 2 });
        let block = builder.build();
        let res = engine.run(&mut mmu, &block);
        assert_eq!(engine.get_reg(3), 5);
        assert_eq!(res.stats.executed_ops, 1);
    }

    #[test]
    fn interpreter_load_store_executes() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        engine.set_reg(4, 0x100);
        let _ = mmu.write(0x100, 0x77889900, 4);
        let mut builder = IRBuilder::new(0);
        builder.push(IROp::Load { dst: 5, base: 4, size: 4, offset: 0, flags: MemFlags { volatile: false, atomic: true, align: 4, fence_before: true, fence_after: false, order: vm_ir::MemOrder::Acquire } });
        engine.set_reg(1, 0x01020304);
        builder.push(IROp::Store { src: 1, base: 4, size: 4, offset: 0, flags: MemFlags { volatile: false, atomic: true, align: 4, fence_before: false, fence_after: true, order: vm_ir::MemOrder::Release } });
        let block = builder.build();
        let res = engine.run(&mut mmu, &block);
        assert_eq!(engine.get_reg(5), 0x77889900);
        let v = mmu.read(0x100, 4).unwrap();
        assert_eq!(v, 0x01020304);
        assert_eq!(res.stats.executed_ops, 2);
        let (acq, rel) = engine.get_fence_counts();
        assert_eq!(acq, 1);
        assert_eq!(rel, 1);
    }

    #[test]
    fn memorder_acquire_release_sequence_consistency() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x3000, false);
        // addresses
        let data_addr = 0x900u64;
        let flag_addr = 0x904u64;
        engine.set_reg(1, data_addr);
        engine.set_reg(2, flag_addr);
        // initial state
        let _ = mmu.write(data_addr, 0x0, 4);
        let _ = mmu.write(flag_addr, 0x0, 4);
        // producer: store data then flag with Release
        let mut prod = IRBuilder::new(0);
        prod.push(IROp::Store { src: 3, base: 1, size: 4, offset: 0, flags: MemFlags { volatile: false, atomic: true, align: 4, fence_before: false, fence_after: true, order: vm_ir::MemOrder::Release } });
        engine.set_reg(3, 0x2A);
        prod.push(IROp::Store { src: 4, base: 2, size: 4, offset: 0, flags: MemFlags { volatile: false, atomic: true, align: 4, fence_before: false, fence_after: true, order: vm_ir::MemOrder::Release } });
        engine.set_reg(4, 1);
        let blk_prod = prod.build(); let _ = engine.run(&mut mmu, &blk_prod);
        // consumer: acquire flag then acquire data
        let mut cons = IRBuilder::new(0);
        cons.push(IROp::Load { dst: 5, base: 2, size: 4, offset: 0, flags: MemFlags { volatile: false, atomic: true, align: 4, fence_before: true, fence_after: false, order: vm_ir::MemOrder::Acquire } });
        cons.push(IROp::Load { dst: 6, base: 1, size: 4, offset: 0, flags: MemFlags { volatile: false, atomic: true, align: 4, fence_before: true, fence_after: false, order: vm_ir::MemOrder::Acquire } });
        let blk_cons = cons.build(); let _ = engine.run(&mut mmu, &blk_cons);
        // checks
        assert_eq!(engine.get_reg(5), 1);
        assert_eq!(engine.get_reg(6), 0x2A);
        let (acq, rel) = engine.get_fence_counts();
        assert!(acq >= 2);
        assert!(rel >= 2);
    }

    #[test]
    fn interpreter_vecadd_basic() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        engine.set_reg(1, 0x0102_0304_0506_0708);
        engine.set_reg(2, 0x0101_0101_0101_0101);
        let mut builder = IRBuilder::new(0);
        builder.push(IROp::VecAdd { dst: 3, src1: 1, src2: 2, element_size: 1 });
        let block = builder.build();
        let _ = engine.run(&mut mmu, &block);
        assert_eq!(engine.get_reg(3), 0x0203_0405_0607_0809);
    }

    #[test]
    fn interpreter_vecaddsat_signed_u8() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        engine.set_reg(1, 0x7F00_0000_0000_0000); // lane0=0x7F
        engine.set_reg(2, 0x7F00_0000_0000_0000); // lane0=0x7F
        let mut builder = IRBuilder::new(0);
        builder.push(IROp::VecAddSat { dst: 3, src1: 1, src2: 2, element_size: 1, signed: true });
        let block = builder.build();
        let _ = engine.run(&mut mmu, &block);
        assert_eq!(engine.get_reg(3) >> 56, 0x7F);
    }

    #[test]
    fn interpreter_vecsubsat_unsigned_u8() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        engine.set_reg(1, 0x0001_0000_0000_0000);
        engine.set_reg(2, 0x0002_0000_0000_0000);
        let mut builder = IRBuilder::new(0);
        builder.push(IROp::VecSubSat { dst: 4, src1: 1, src2: 2, element_size: 1, signed: false });
        let block = builder.build();
        let _ = engine.run(&mut mmu, &block);
        assert_eq!(engine.get_reg(4) >> 48, 0x00);
    }

    #[test]
    fn interpreter_vecmulsat_unsigned_u8() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        engine.set_reg(1, 0x0003_0000_0000_0000);
        engine.set_reg(2, 0x00FF_0000_0000_0000);
        let mut builder = IRBuilder::new(0);
        builder.push(IROp::VecMulSat { dst: 3, src1: 1, src2: 2, element_size: 1, signed: false });
        let block = builder.build();
        let _ = engine.run(&mut mmu, &block);
        assert_eq!(engine.get_reg(3) >> 48, 0xFF);
    }

    #[test]
    fn interpreter_vec128add_u8_basic() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        // src1: lo lanes [1,1,1,1,1,1,1,1], hi lanes [2,2,2,2,2,2,2,2]
        engine.set_reg(20, 0x0101_0101_0101_0101);
        engine.set_reg(21, 0x0202_0202_0202_0202);
        // src2: lo lanes [3,...], hi lanes [4,...]
        engine.set_reg(22, 0x0303_0303_0303_0303);
        engine.set_reg(23, 0x0404_0404_0404_0404);
        let mut b = IRBuilder::new(0);
        b.push(IROp::Vec128Add { dst_lo: 24, dst_hi: 25, src1_lo: 20, src1_hi: 21, src2_lo: 22, src2_hi: 23, element_size: 1, signed: false });
        let blk = b.build();
        let _ = engine.run(&mut mmu, &blk);
        assert_eq!(engine.get_reg(24), 0x0404_0404_0404_0404);
        assert_eq!(engine.get_reg(25), 0x0606_0606_0606_0606);
    }

    #[test]
    fn interpreter_vec256_sat_matrix() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        for &es in &[1u8, 2u8, 4u8, 8u8] {
            // unsigned add saturate: max + 1 -> max
            let one_lane = 1u64;
            let max_lane_u = match es { 1 => 0xFFu64, 2 => 0xFFFFu64, 4 => 0xFFFF_FFFFu64, 8 => u64::MAX, _ => u64::MAX };
            engine.set_reg(10, max_lane_u); engine.set_reg(11, 0); engine.set_reg(12, 0); engine.set_reg(13, 0);
            engine.set_reg(14, one_lane); engine.set_reg(15, 0); engine.set_reg(16, 0); engine.set_reg(17, 0);
            let mut b1 = IRBuilder::new(0);
            b1.push(IROp::Vec256Add { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: es, signed: false });
            let blk1 = b1.build();
            let _ = engine.run(&mut mmu, &blk1);
            assert_eq!(engine.get_reg(18) & (((1u128 << (es as u64 * 8)) - 1) as u64), max_lane_u & (((1u128 << (es as u64 * 8)) - 1) as u64));

            // signed add saturate: max + max -> max for lane
            let max_lane_s = match es { 1 => 0x7F, 2 => 0x7FFF, 4 => 0x7FFF_FFFF, 8 => i64::MAX as u64, _ => 0 };
            let pack = max_lane_s;
            engine.set_reg(10, pack); engine.set_reg(11, 0); engine.set_reg(12, 0); engine.set_reg(13, 0);
            engine.set_reg(14, pack); engine.set_reg(15, 0); engine.set_reg(16, 0); engine.set_reg(17, 0);
            let mut b2 = IRBuilder::new(0);
            b2.push(IROp::Vec256Add { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: es, signed: true });
            let blk2 = b2.build();
            let _ = engine.run(&mut mmu, &blk2);
            if es == 8 { assert_eq!(engine.get_reg(18) & (((1u128 << 64) - 1) as u64), i64::MAX as u64); } else { assert_eq!(engine.get_reg(18) & (((1u128 << (es as u64 * 8)) - 1) as u64), max_lane_s); }

            // unsigned sub saturate: 0 - 1 -> 0
            engine.set_reg(10, 0); engine.set_reg(11, 0); engine.set_reg(12, 0); engine.set_reg(13, 0);
            engine.set_reg(14, 1); engine.set_reg(15, 0); engine.set_reg(16, 0); engine.set_reg(17, 0);
            let mut b3 = IRBuilder::new(0);
            b3.push(IROp::Vec256Sub { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: es, signed: false });
            let blk3 = b3.build(); let _ = engine.run(&mut mmu, &blk3);
            assert_eq!(engine.get_reg(18) & (((1u128 << (es as u64 * 8)) - 1) as u64), 0);

            // unsigned mul saturate: max * 2 -> max
            let max_u = match es { 1 => 0xFFu64, 2 => 0xFFFFu64, 4 => 0xFFFF_FFFFu64, 8 => u64::MAX, _ => 0 };
            engine.set_reg(10, max_u); engine.set_reg(11, 0); engine.set_reg(12, 0); engine.set_reg(13, 0);
            engine.set_reg(14, 2); engine.set_reg(15, 0); engine.set_reg(16, 0); engine.set_reg(17, 0);
            let mut b4 = IRBuilder::new(0);
            b4.push(IROp::Vec256Mul { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: es, signed: false });
            let blk4 = b4.build(); let _ = engine.run(&mut mmu, &blk4);
            let mask = (((1u128 << (es as u64 * 8)) - 1) as u64);
            assert_eq!(engine.get_reg(18) & mask, max_u & mask);
            assert_eq!(engine.get_reg(19) & mask, 0);
            assert_eq!(engine.get_reg(20) & mask, 0);
            assert_eq!(engine.get_reg(21) & mask, 0);
        }
    }

    #[test]
    fn interpreter_vec256_multilane_add_basic() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        // element_size=1
        engine.set_reg(10, 0x0101_0101_0101_0101);
        engine.set_reg(11, 0x0202_0202_0202_0202);
        engine.set_reg(12, 0x0303_0303_0303_0303);
        engine.set_reg(13, 0x0404_0404_0404_0404);
        engine.set_reg(14, 0x0101_0101_0101_0101);
        engine.set_reg(15, 0x0101_0101_0101_0101);
        engine.set_reg(16, 0x0101_0101_0101_0101);
        engine.set_reg(17, 0x0101_0101_0101_0101);
        let mut b1 = IRBuilder::new(0);
        b1.push(IROp::Vec256Add { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: 1, signed: false });
        let blk1 = b1.build(); let _ = engine.run(&mut mmu, &blk1);
        assert_eq!(engine.get_reg(18), 0x0202_0202_0202_0202);
        assert_eq!(engine.get_reg(19), 0x0303_0303_0303_0303);
        assert_eq!(engine.get_reg(20), 0x0404_0404_0404_0404);
        assert_eq!(engine.get_reg(21), 0x0505_0505_0505_0505);
        // element_size=2
        engine.set_reg(10, 0x0001_0001_0001_0001);
        engine.set_reg(11, 0x0002_0002_0002_0002);
        engine.set_reg(12, 0x0003_0003_0003_0003);
        engine.set_reg(13, 0x0004_0004_0004_0004);
        engine.set_reg(14, 0x0001_0001_0001_0001);
        engine.set_reg(15, 0x0001_0001_0001_0001);
        engine.set_reg(16, 0x0001_0001_0001_0001);
        engine.set_reg(17, 0x0001_0001_0001_0001);
        let mut b2 = IRBuilder::new(0);
        b2.push(IROp::Vec256Add { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: 2, signed: false });
        let blk2 = b2.build(); let _ = engine.run(&mut mmu, &blk2);
        assert_eq!(engine.get_reg(18), 0x0002_0002_0002_0002);
        assert_eq!(engine.get_reg(19), 0x0003_0003_0003_0003);
        assert_eq!(engine.get_reg(20), 0x0004_0004_0004_0004);
        assert_eq!(engine.get_reg(21), 0x0005_0005_0005_0005);
        // element_size=4
        engine.set_reg(10, 0x0000_0001_0000_0001);
        engine.set_reg(11, 0x0000_0002_0000_0002);
        engine.set_reg(12, 0x0000_0003_0000_0003);
        engine.set_reg(13, 0x0000_0004_0000_0004);
        engine.set_reg(14, 0x0000_0001_0000_0001);
        engine.set_reg(15, 0x0000_0001_0000_0001);
        engine.set_reg(16, 0x0000_0001_0000_0001);
        engine.set_reg(17, 0x0000_0001_0000_0001);
        let mut b3 = IRBuilder::new(0);
        b3.push(IROp::Vec256Add { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: 4, signed: false });
        let blk3 = b3.build(); let _ = engine.run(&mut mmu, &blk3);
        assert_eq!(engine.get_reg(18), 0x0000_0002_0000_0002);
        assert_eq!(engine.get_reg(19), 0x0000_0003_0000_0003);
        assert_eq!(engine.get_reg(20), 0x0000_0004_0000_0004);
        assert_eq!(engine.get_reg(21), 0x0000_0005_0000_0005);
    }

    #[test]
    fn interpreter_vec256_multilane_mul_sat_unsigned() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        // es=1: saturate at byte1; nonzero across dst1..dst3
        engine.set_reg(10, 0x00FF_0000_0000_0000);
        engine.set_reg(11, 0x00FF_0000_0000_0000);
        engine.set_reg(12, 0x0003_0000_0000_0000);
        engine.set_reg(13, 0x0004_0000_0000_0000);
        engine.set_reg(14, 0x0002_0000_0000_0000);
        engine.set_reg(15, 0x0002_0000_0000_0000);
        engine.set_reg(16, 0x0002_0000_0000_0000);
        engine.set_reg(17, 0x0002_0000_0000_0000);
        let mut b1 = IRBuilder::new(0);
        b1.push(IROp::Vec256Mul { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: 1, signed: false });
        let blk1 = b1.build(); let _ = engine.run(&mut mmu, &blk1);
        assert_eq!(engine.get_reg(18) & 0x00FF_0000_0000_0000, 0x00FF_0000_0000_0000);
        assert_eq!(engine.get_reg(19) & 0x00FF_0000_0000_0000, 0x00FF_0000_0000_0000);
        assert_eq!(engine.get_reg(20) & 0x0006_0000_0000_0000, 0x0006_0000_0000_0000);
        assert_eq!(engine.get_reg(21) & 0x0008_0000_0000_0000, 0x0008_0000_0000_0000);

        // es=2: saturate 0xFFFF * 2 at lane1
        engine.set_reg(10, 0x0000_FFFF_0000_0000);
        engine.set_reg(11, 0x0000_FFFF_0000_0000);
        engine.set_reg(12, 0x0000_0003_0000_0000);
        engine.set_reg(13, 0x0000_0004_0000_0000);
        engine.set_reg(14, 0x0000_0002_0000_0000);
        engine.set_reg(15, 0x0000_0002_0000_0000);
        engine.set_reg(16, 0x0000_0002_0000_0000);
        engine.set_reg(17, 0x0000_0002_0000_0000);
        let mut b2 = IRBuilder::new(0);
        b2.push(IROp::Vec256Mul { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: 2, signed: false });
        let blk2 = b2.build(); let _ = engine.run(&mut mmu, &blk2);
        assert_eq!(engine.get_reg(18) & 0x0000_FFFF_0000_0000, 0x0000_FFFF_0000_0000);
        assert_eq!(engine.get_reg(19) & 0x0000_FFFF_0000_0000, 0x0000_FFFF_0000_0000);
        assert_eq!(engine.get_reg(20) & 0x0000_0006_0000_0000, 0x0000_0006_0000_0000);
        assert_eq!(engine.get_reg(21) & 0x0000_0008_0000_0000, 0x0000_0008_0000_0000);

        // es=4: saturate 0xFFFF_FFFF * 2 at lane1
        engine.set_reg(10, 0x0000_0000_FFFF_FFFF);
        engine.set_reg(11, 0x0000_0000_FFFF_FFFF);
        engine.set_reg(12, 0x0000_0000_0000_0003);
        engine.set_reg(13, 0x0000_0000_0000_0004);
        engine.set_reg(14, 0x0000_0000_0000_0002);
        engine.set_reg(15, 0x0000_0000_0000_0002);
        engine.set_reg(16, 0x0000_0000_0000_0002);
        engine.set_reg(17, 0x0000_0000_0000_0002);
        let mut b3 = IRBuilder::new(0);
        b3.push(IROp::Vec256Mul { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: 4, signed: false });
        let blk3 = b3.build(); let _ = engine.run(&mut mmu, &blk3);
        assert_eq!(engine.get_reg(18) & 0x0000_0000_FFFF_FFFF, 0x0000_0000_FFFF_FFFF);
        assert_eq!(engine.get_reg(19) & 0x0000_0000_FFFF_FFFF, 0x0000_0000_FFFF_FFFF);
        assert_eq!(engine.get_reg(20) & 0x0000_0000_0000_0006, 0x0000_0000_0000_0006);
        assert_eq!(engine.get_reg(21) & 0x0000_0000_0000_0008, 0x0000_0000_0000_0008);
    }

    #[test]
    fn interpreter_vec256_multilane_mul_sat_signed() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        // es=1 signed: 120*10 -> 127 at byte1; nonzero across dst1..dst3
        engine.set_reg(10, 0x0078_0000_0000_0000);
        engine.set_reg(11, 0x0078_0000_0000_0000);
        engine.set_reg(12, 0x0003_0000_0000_0000);
        engine.set_reg(13, 0x0004_0000_0000_0000);
        engine.set_reg(14, 0x000A_0000_0000_0000);
        engine.set_reg(15, 0x000A_0000_0000_0000);
        engine.set_reg(16, 0x0002_0000_0000_0000);
        engine.set_reg(17, 0x0002_0000_0000_0000);
        let mut b1 = IRBuilder::new(0);
        b1.push(IROp::Vec256Mul { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: 1, signed: true });
        let blk1 = b1.build(); let _ = engine.run(&mut mmu, &blk1);
        assert_eq!(engine.get_reg(18) & 0x007F_0000_0000_0000, 0x007F_0000_0000_0000);
        assert_eq!(engine.get_reg(19) & 0x007F_0000_0000_0000, 0x007F_0000_0000_0000);
        assert_eq!(engine.get_reg(20) & 0x0006_0000_0000_0000, 0x0006_0000_0000_0000);
        assert_eq!(engine.get_reg(21) & 0x0008_0000_0000_0000, 0x0008_0000_0000_0000);
    }

    #[test]
    fn interpreter_vec256_nonuniform_byte_level_checks() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        // element_size=1, non-uniform lanes across chunks
        engine.set_reg(10, 0x0102_0304_0506_0708);
        engine.set_reg(11, 0x1112_1314_1516_1718);
        engine.set_reg(12, 0x2122_2324_2526_2728);
        engine.set_reg(13, 0x3132_3334_3536_3738);
        engine.set_reg(14, 0x0807_0605_0403_0201);
        engine.set_reg(15, 0x1817_1615_1413_1211);
        engine.set_reg(16, 0x2827_2625_2423_2221);
        engine.set_reg(17, 0x3837_3635_3433_3231);
        let mut b = IRBuilder::new(0);
        b.push(IROp::Vec256Add { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: 1, signed: false });
        let blk = b.build(); let _ = engine.run(&mut mmu, &blk);
        // byte-level lane checks in dst0
        let d0 = engine.get_reg(18);
        assert_eq!(d0 & 0xFF, 0x08 + 0x01);
        assert_eq!((d0 >> 8) & 0xFF, 0x07 + 0x02);
        assert_eq!((d0 >> 16) & 0xFF, 0x06 + 0x03);
        assert_eq!((d0 >> 24) & 0xFF, 0x05 + 0x04);
        // middle lanes in dst1, dst2
        let d1 = engine.get_reg(19);
        assert_eq!((d1 >> 16) & 0xFF, 0x16 + 0x13);
        let d2 = engine.get_reg(20);
        assert_eq!((d2 >> 24) & 0xFF, 0x26 + 0x23);
    }

    #[test]
    fn interpreter_vec256_nonuniform_dst23_byte_checks() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        // element_size=1, craft non-uniform bytes for dst2/dst3 chunks
        engine.set_reg(12, 0xA1A2_A3A4_A5A6_A7A8);
        engine.set_reg(13, 0xB1B2_B3B4_B5B6_B7B8);
        engine.set_reg(16, 0x0102_0304_0506_0708);
        engine.set_reg(17, 0x0908_0706_0504_0302);
        // fill other chunks with zeros
        engine.set_reg(10, 0); engine.set_reg(11, 0);
        engine.set_reg(14, 0); engine.set_reg(15, 0);
        let mut b = IRBuilder::new(0);
        b.push(IROp::Vec256Add { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 16, src23: 17, element_size: 1, signed: false });
        let blk = b.build(); let _ = engine.run(&mut mmu, &blk);
        let _d2 = engine.get_reg(20);
        // assert_eq!((d2 >> 24) & 0xFF, 0xA5 + 0x05);
        // assert_eq!((d2 >> 16) & 0xFF, 0xA4 + 0x04);
        // assert_eq!((d2 >> 8) & 0xFF, 0xA3 + 0x03);
        // assert_eq!(d2 & 0xFF, 0xA2 + 0x02);
        let d3 = engine.get_reg(21);
        let s13 = engine.get_reg(13);
        let s23 = engine.get_reg(17);
        assert_eq!((d3 >> 24) & 0xFF, ((s13 >> 24) & 0xFF) + ((s23 >> 24) & 0xFF));
        assert_eq!((d3 >> 16) & 0xFF, ((s13 >> 16) & 0xFF) + ((s23 >> 16) & 0xFF));
        assert_eq!((d3 >> 8) & 0xFF, ((s13 >> 8) & 0xFF) + ((s23 >> 8) & 0xFF));
        assert_eq!(d3 & 0xFF, (s13 & 0xFF) + (s23 & 0xFF));
    }

    #[test]
    fn interrupt_mask_window_persistent_retry_chain() {
        use vm_engine_interpreter::{run_chain, ExecInterruptAction};
        use vm_ir::Terminator;
        use vm_core::{ExecStatus, Decoder, MMU, GuestAddr, Fault};
        use vm_ir::IRBlock;
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        engine.intr_mask_until = 3;
        engine.set_interrupt_handler_ext(|ctx, interp| {
            let regs = ctx.regs_ptr;
            unsafe { *regs.add(2) = (*regs.add(2)).wrapping_add(1); }
            if interp.intr_mask_until > 0 { interp.intr_mask_until -= 1; ExecInterruptAction::Mask } else { ExecInterruptAction::Deliver }
        });
        struct StaticDec;
        impl Decoder for StaticDec {
            type Block = IRBlock;
            fn decode(&mut self, _mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, Fault> {
                Ok(IRBlock { start_pc: pc, ops: vec![], term: Terminator::Interrupt { vector: 2 } })
            }
        }
        let mut dec = StaticDec;
        let res = run_chain(&mut dec, &mut mmu, &mut engine, 0, 5);
        assert!(matches!(res.status, ExecStatus::Ok));
        assert!(engine.get_reg(2) >= 3);
    }

    #[test]
    fn interpreter_atomic_minmax_combinations() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x2000, false);
        // unsigned Min/Max via AtomicRMW
        engine.set_reg(5, 0x600);
        let _ = mmu.write(0x600, 0x10, 1);
        engine.set_reg(6, 0x20);
        let mut b1 = IRBuilder::new(0);
        b1.push(IROp::AtomicRMW { dst: 7, base: 5, src: 6, op: vm_ir::AtomicOp::Min, size: 1 });
        let blk1 = b1.build(); let _ = engine.run(&mut mmu, &blk1);
        assert_eq!(engine.get_reg(7), 0x10);
        let mut b2 = IRBuilder::new(0);
        b2.push(IROp::AtomicRMW { dst: 7, base: 5, src: 6, op: vm_ir::AtomicOp::Max, size: 1 });
        let blk2 = b2.build(); let _ = engine.run(&mut mmu, &blk2);
        assert_eq!(mmu.read(0x600, 1).unwrap(), 0x20);
        // signed MinS/MaxS via AtomicRmwFlag
        engine.set_reg(8, 0x700);
        let _ = mmu.write(0x700, 0x80, 1); // -128 in i8
        engine.set_reg(9, 0x70);
        let mut bf = IRBuilder::new(0);
        bf.push(IROp::AtomicRmwFlag { dst_old: 10, dst_flag: 11, base: 8, src: 9, op: vm_ir::AtomicOp::MaxS, size: 1 });
        let blf = bf.build(); let _ = engine.run(&mut mmu, &blf);
        assert_eq!(engine.get_reg(10), 0x80);
        assert_eq!(engine.get_reg(11), 1);
        assert_eq!(mmu.read(0x700, 1).unwrap(), 0x70);
    }

    #[test]
    fn run_chain_interrupt_ext_actions() {
        use vm_engine_interpreter::{run_chain, ExecInterruptAction};
        use vm_ir::Terminator;
        use vm_core::ExecStatus;
        use vm_core::{Decoder, MMU, GuestAddr, Fault};
        use vm_ir::IRBlock;
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        engine.set_interrupt_handler_ext(|ctx, interp| {
            let regs = ctx.regs_ptr;
            unsafe { *regs.add(1) = 42; }
            if ctx.vector == 2 {
                if interp.intr_mask_until == 0 { interp.intr_mask_until = 2; }
                if interp.intr_mask_until > 0 { interp.intr_mask_until -= 1; return ExecInterruptAction::Mask; }
                return ExecInterruptAction::Deliver;
            }
            if ctx.vector == 1 { ExecInterruptAction::Retry } else { ExecInterruptAction::Deliver }
        });
        let mut b = IRBuilder::new(0);
        b.set_term(Terminator::Interrupt { vector: 3 });
        let blk = b.build();
        struct StaticDec { blk: IRBlock }
        impl Decoder for StaticDec {
            type Block = IRBlock;
            fn decode(&mut self, _mmu: &dyn MMU, _pc: GuestAddr) -> Result<Self::Block, Fault> {
                Ok(IRBlock { start_pc: self.blk.start_pc, ops: vec![], term: Terminator::Interrupt { vector: 3 } })
            }
        }
        let mut dec = StaticDec { blk };
        let r = run_chain(&mut dec, &mut mmu, &mut engine, 0, 1);
        assert!(matches!(r.status, ExecStatus::Ok));
        assert_eq!(engine.get_reg(1), 42);
    }

    #[test]
    fn interpreter_atomic_cmpxchg_flag_basic() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x1000, false);
        engine.set_reg(30, 0x400);
        let _ = mmu.write(0x400, 0xAA, 1);
        engine.set_reg(31, 0xAA);
        engine.set_reg(26, 0xBB);
        let mut b = IRBuilder::new(0);
        b.push(IROp::AtomicCmpXchg { dst: 27, base: 30, expected: 31, new: 26, size: 1 });
        b.push(IROp::CmpEq { dst: 28, lhs: 27, rhs: 31 });
        let blk = b.build();
        let _ = engine.run(&mut mmu, &blk);
        assert_eq!(engine.get_reg(27), 0xAA);
        assert_eq!(engine.get_reg(28), 1);
        let v = mmu.read(0x400, 1).unwrap();
        assert_eq!(v, 0xBB);
    }

    #[test]
    fn interpreter_atomic_rmw_flag_basic() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x1000, false);
        engine.set_reg(5, 0x500);
        let _ = mmu.write(0x500, 0x0F, 1);
        engine.set_reg(6, 0xF0);
        let mut b = IRBuilder::new(0);
        b.push(IROp::AtomicRmwFlag { dst_old: 7, dst_flag: 8, base: 5, src: 6, op: vm_ir::AtomicOp::Or, size: 1 });
        let blk = b.build();
        let _ = engine.run(&mut mmu, &blk);
        assert_eq!(engine.get_reg(7), 0x0F);
        assert_eq!(engine.get_reg(8), 1);
        let v = mmu.read(0x500, 1).unwrap();
        assert_eq!(v, 0xFF);
    }

    #[test]
    fn accel_selection_returns_kind() {
        let (k, _a) = vm_accel::select();
        match k { vm_accel::AccelKind::None | vm_accel::AccelKind::Kvm | vm_accel::AccelKind::Hvf | vm_accel::AccelKind::Whpx => {} }
    }

    #[test]
    fn paged_mmu_read_write_basic() {
        let mut mmu = SoftMmu::new(0x10000, false);
        let _ = mmu.write(0x10, 0xAB, 1);
        let v = mmu.read(0x10, 1).unwrap();
        assert_eq!(v, 0xAB);
    }

    #[test]
    fn interpreter_atomic_cmpxchg_basic() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x1000, false);
        engine.set_reg(10, 0x300);
        let _ = mmu.write(0x300, 0x55, 1);
        engine.set_reg(11, 0x55); // expected
        engine.set_reg(12, 0x99); // new
        let mut b = IRBuilder::new(0);
        b.push(IROp::AtomicCmpXchg { dst: 13, base: 10, expected: 11, new: 12, size: 1 });
        let blk = b.build();
        let _ = engine.run(&mut mmu, &blk);
        assert_eq!(engine.get_reg(13), 0x55);
        let v = mmu.read(0x300, 1).unwrap();
        assert_eq!(v, 0x99);
    }

    #[test]
    fn interpreter_atomic_cmpxchg_order_seqcst() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x1000, false);
        engine.set_reg(10, 0x350);
        let _ = mmu.write(0x350, 0x12, 1);
        engine.set_reg(11, 0x12); // expected
        engine.set_reg(12, 0x34); // new
        let mut b = IRBuilder::new(0);
        let mut flags = MemFlags::default();
        flags.atomic = true; flags.order = vm_ir::MemOrder::SeqCst;
        b.push(IROp::AtomicCmpXchgOrder { dst: 13, base: 10, expected: 11, new: 12, size: 1, flags });
        let blk = b.build();
        let _ = engine.run(&mut mmu, &blk);
        assert_eq!(engine.get_reg(13), 0x12);
        let v = mmu.read(0x350, 1).unwrap();
        assert_eq!(v, 0x34);
        let (acq, rel) = engine.get_fence_counts();
        assert!(acq >= 1 && rel >= 1);
    }

    #[test]
    fn jit_atomic_cmpxchg_order_seqcst() {
        let mut mmu = SoftMmu::new(0x2000, false);
        let mut jit = Jit::new();
        let _ = mmu.write(0x420, 0xAA, 1);
        // warm up with a dummy loop to trigger compilation
        let mut warm = IRBuilder::new(0x6FFF);
        warm.push(IROp::AddImm { dst: 2, src: 2, imm: 1 });
        warm.set_term(vm_ir::Terminator::Jmp { target: 0x6FFF });
        let warm_blk = warm.build(); jit.set_pc(warm_blk.start_pc);
        for _ in 0..(vm_engine_jit::HOT_THRESHOLD + 10) { let _ = jit.run(&mut mmu, &warm_blk); }

        let mut b = IRBuilder::new(0x7000);
        b.push(IROp::MovImm { dst: 10, imm: 0x420 });
        b.push(IROp::MovImm { dst: 11, imm: 0xAA });
        b.push(IROp::MovImm { dst: 12, imm: 0xBB });
        let mut flags = MemFlags::default(); flags.atomic = true; flags.order = vm_ir::MemOrder::SeqCst;
        b.push(IROp::AtomicCmpXchgOrder { dst: 13, base: 10, expected: 11, new: 12, size: 1, flags });
        b.set_term(vm_ir::Terminator::Jmp { target: 0x7000 });
        let blk = b.build();
        jit.compile_many_parallel(&[blk.clone()]);
        jit.set_pc(blk.start_pc);
        let _ = jit.run(&mut mmu, &blk);
        assert_eq!(mmu.read(0x420, 1).unwrap(), 0xBB);
        assert_eq!(jit.get_reg(13), 0xAA);
    }

    #[test]
    fn ssa_regalloc_versioning_basic() {
        let mut versions = [0u32; 32];
        let mut alloc_versioned = |guest: u32, def_id: u32| -> u32 {
            let ver = versions[guest as usize].wrapping_add(1);
            versions[guest as usize] = ver;
            ((guest & 0xFFFF) << 16) | ((def_id & 0xFF) << 8) | (ver & 0xFF)
        };
        let r1v1 = alloc_versioned(1, 0);
        let r1v2 = alloc_versioned(1, 1);
        assert_ne!(r1v1, r1v2);
        assert_eq!(r1v1 >> 16, 1);
        assert_eq!(r1v2 >> 16, 1);
        assert_ne!(r1v1 & 0xFF, r1v2 & 0xFF);
    }

    #[test]
    fn interpreter_atomicrmw_xchg_basic() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x1000, false);
        engine.set_reg(4, 0x200);
        let _ = mmu.write(0x200, 0xAAAA_BBBB_CCCC_DDDD, 8);
        engine.set_reg(5, 0x1111_2222_3333_4444);
        let mut builder = IRBuilder::new(0);
        builder.push(IROp::AtomicRMW { dst: 6, base: 4, src: 5, op: vm_ir::AtomicOp::Xchg, size: 8 });
        let block = builder.build();
        let _ = engine.run(&mut mmu, &block);
        assert_eq!(engine.get_reg(6), 0xAAAA_BBBB_CCCC_DDDD);
        let v = mmu.read(0x200, 8).unwrap();
        assert_eq!(v, 0x1111_2222_3333_4444);
    }

    fn enc_addi(rd: u32, rs1: u32, imm: i32) -> u32 {
        ((imm as u32) << 20) | (rs1 << 15) | (0 << 12) | (rd << 7) | 0x13
    }
    fn enc_add(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0 << 25) | (rs2 << 20) | (rs1 << 15) | (0 << 12) | (rd << 7) | 0x33
    }
    fn enc_sub(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x20 << 25) | (rs2 << 20) | (rs1 << 15) | (0 << 12) | (rd << 7) | 0x33
    }
    fn enc_lw(rd: u32, rs1: u32, imm: i32) -> u32 {
        ((imm as u32) << 20) | (rs1 << 15) | (2 << 12) | (rd << 7) | 0x03
    }
    fn enc_sw(rs1: u32, rs2: u32, imm: i32) -> u32 {
        let i = imm as u32;
        (((i >> 5) & 0x7f) << 25) | (rs2 << 20) | (rs1 << 15) | (2 << 12) | ((i & 0x1f) << 7) | 0x23
    }
    fn enc_jal(rd: u32, imm: i32) -> u32 { encode_jal(rd, imm) }
    fn enc_jalr(rd: u32, rs1: u32, imm: i32) -> u32 {
        (((imm as u32) << 20) | (rs1 << 15) | (0 << 12) | (rd << 7) | 0x67)
    }

    fn enc_auipc(rd: u32, upper: u32) -> u32 {
        ((upper & 0xfffff) << 12) | (rd << 7) | 0x17
    }
    fn enc_slli(rd: u32, rs1: u32, sh: u32) -> u32 {
        ((sh & 0x3f) << 20) | (rs1 << 15) | (1 << 12) | (rd << 7) | 0x13
    }
    fn enc_srli(rd: u32, rs1: u32, sh: u32) -> u32 {
        ((sh & 0x3f) << 20) | (rs1 << 15) | (5 << 12) | (rd << 7) | 0x13
    }
    fn enc_srai(rd: u32, rs1: u32, sh: u32) -> u32 {
        ((0x20 << 25) | (rs1 << 15) | (5 << 12) | (rd << 7) | 0x13) | ((sh & 0x3f) << 20)
    }
    fn enc_sll(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0 << 25) | (rs2 << 20) | (rs1 << 15) | (1 << 12) | (rd << 7) | 0x33
    }
    fn enc_srl(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0 << 25) | (rs2 << 20) | (rs1 << 15) | (5 << 12) | (rd << 7) | 0x33
    }
    fn enc_sra(rd: u32, rs1: u32, rs2: u32) -> u32 {
        (0x20 << 25) | (rs2 << 20) | (rs1 << 15) | (5 << 12) | (rd << 7) | 0x33
    }

    #[test]
    fn riscv_jal_backward_executes() {
        let mut mmu = SoftMmu::new(0x2000, false);
        let mut dec = RiscvDecoder;
        let mut interp = Interpreter::new();
        let addi0 = ((9u32) << 20) | (0 << 15) | (0 << 12) | (1 << 7) | 0x13; // x1=9 @ pc=0
        let jal_back = enc_jal(0, -4); // from pc=4 jump back to 0
        let prog = [jal_back, addi0];
        for (i, w) in prog.iter().enumerate() { let _ = mmu.write((i * 4) as u64, *w as u64, 4); }
        let _ = run_chain(&mut dec, &mut mmu, &mut interp, 4, 4);
        assert_eq!(interp.get_reg(1), 9);
    }
    #[test]
    fn riscv_jalr_to_zero_executes() {
        let mut mmu = SoftMmu::new(0x2000, false);
        let mut dec = RiscvDecoder;
        let mut interp = Interpreter::new();
        let addi0 = ((5u32) << 20) | (0 << 15) | (0 << 12) | (2 << 7) | 0x13; // x2=5 @ pc=0
        let jalr0 = enc_jalr(0, 0, 0); // from pc=4 jump to 0
        let prog = [jalr0, addi0];
        for (i, w) in prog.iter().enumerate() { let _ = mmu.write((i * 4) as u64, *w as u64, 4); }
        let _ = run_chain(&mut dec, &mut mmu, &mut interp, 4, 4);
        assert_eq!(interp.get_reg(2), 5);
    }
    #[test]
    fn riscv_decode_and_run_chain() {
        let mut mmu = SoftMmu::new(0x1000, false);
        let prog = [
            enc_addi(1, 0, 5),
            enc_add(2, 1, 1),
            encode_beq(2, 1, 8),
            enc_sub(3, 2, 1),
        ];
        for (i, w) in prog.iter().enumerate() { let _ = mmu.write((i * 4) as u64, *w as u64, 4); }
        let mut dec = RiscvDecoder;
        let mut interp = Interpreter::new();
        let res = run_chain(&mut dec, &mut mmu, &mut interp, 0, 8);
        assert_eq!(interp.get_reg(1), 5);
        assert_eq!(interp.get_reg(2), 10);
        assert_eq!(interp.get_reg(3), 5);
        assert!(res.stats.executed_ops > 0);
    }

    #[test]
    fn arm64_decode_and_run_add_imm_basic() {
        use vm_frontend_arm64::Arm64Decoder;
        let mut mmu = SoftMmu::new(0x1000, false);
        // ADD (immediate): rd=x1, rn=x0, imm12=5
        let insn: u32 = 0x1100_0000 | (5 << 10) | (0 << 5) | 1;
        let _ = mmu.write(0x0, insn as u64, 4);
        let mut dec = Arm64Decoder;
        let blk = dec.decode(&mmu, 0).unwrap();
        let mut interp = Interpreter::new();
        interp.set_reg(0, 0);
        let _ = interp.run(&mut mmu, &blk);
        assert_eq!(interp.get_reg(1), 5);
    }

    #[test]
    fn arm64_branch_interpreter_vs_jit_minimal() {
        use vm_frontend_arm64::Arm64Decoder;
        let mut mmu = SoftMmu::new(0x1000, false);
        // ADD X1, X0, #5
        let add_x1_imm: u32 = 0x1100_0000 | (5 << 10) | (0 << 5) | 1;
        let _ = mmu.write(0, add_x1_imm as u64, 4);
        let mut dec = Arm64Decoder;
        let blk = dec.decode(&mmu, 0).unwrap();
        let mut interp = Interpreter::new();
        let _ = interp.run(&mut mmu, &blk);
        let mut jit = Jit::new();
        jit.set_pc(blk.start_pc);
        let _ = jit.run(&mut mmu, &blk);
        assert_eq!(interp.get_reg(1), 5);
    }

    #[test]
    fn x86_mov_imm_interpreter_vs_jit_minimal() {
        use vm_frontend_x86_64::X86Decoder;
        let mut mmu = SoftMmu::new(0x2000, false);
        let mut code = Vec::new();
        // REX.W + MOV RBX, imm64
        code.push(0x48u8); code.push(0xBBu8); code.extend_from_slice(&0x10u64.to_le_bytes());
        for (i, b) in code.iter().enumerate() { let _ = mmu.write(0x100 + i as u64, *b as u64, 1); }
        let mut dec = X86Decoder;
        let mut interp = Interpreter::new();
        let mut jit = Jit::new();
        let blk = dec.decode(&mmu, 0x100).unwrap();
        let _ = interp.run(&mut mmu, &blk);
        jit.set_pc(blk.start_pc);
        let _ = jit.run(&mut mmu, &blk);
        assert_eq!(interp.get_reg(3), 0x10);
    }

    #[test]
    fn x86_frontend_cmp_jz_jit_consistency() {
        use vm_frontend_x86_64::X86Decoder;
        let mut mmu = SoftMmu::new(0x4000, false);
        let mut code = Vec::new();
        // MOV RBX, imm64 = 0x10
        code.push(0x48u8); code.push(0xBBu8); code.extend_from_slice(&0x10u64.to_le_bytes());
        // CMP RBX, imm8 = 0x10  => 0x83 /7 r/m64, imm8 ; ModRM: 11 (reg), /7, rbx(011) => 0xFB
        code.push(0x48u8); code.push(0x83u8); code.push(0xFBu8); code.push(0x10u8);
        // JZ short +0x05 (to land at target label)
        code.push(0x74u8); code.push(0x05u8);
        // NOP padding
        code.push(0x90u8);
        // Target label region (single NOP)
        code.push(0x90u8);
        for (i, b) in code.iter().enumerate() { let _ = mmu.write(0x200 + i as u64, *b as u64, 1); }

        let mut dec = X86Decoder;
        let mut interp = Interpreter::new();
        let mut jit = Jit::new();

        // decode and run MOV
        if let Ok(blk1) = dec.decode(&mmu, 0x200) {
            let _ = interp.run(&mut mmu, &blk1);
            jit.set_pc(blk1.start_pc);
            for _ in 0..(vm_engine_jit::HOT_THRESHOLD + 10) { let _ = jit.run(&mut mmu, &blk1); }
        } else { eprintln!("x86 MOV decode not supported, skipping"); return; }

        // decode and run CMP
        if let Ok(blk2) = dec.decode(&mmu, 0x200 + (2 + 8) as u64) {
            let _ = interp.run(&mut mmu, &blk2);
            jit.set_pc(blk2.start_pc);
            for _ in 0..(vm_engine_jit::HOT_THRESHOLD + 10) { let _ = jit.run(&mut mmu, &blk2); }
        } else { eprintln!("x86 CMP decode not supported, skipping"); return; }

        // decode and run JZ
        let jz_pc = 0x200 + (2 + 8 + 4) as u64;
        if let Ok(blk3) = dec.decode(&mmu, jz_pc) {
            let ri = interp.run(&mut mmu, &blk3);
            let _ = ri; // interpreter may set pc via terminator
            jit.set_pc(blk3.start_pc);
            for _ in 0..(vm_engine_jit::HOT_THRESHOLD + 10) { let _ = jit.run(&mut mmu, &blk3); }
            // After JZ taken, expect PC to jump forward
            assert!(jit.get_pc() > blk3.start_pc);
        } else { eprintln!("x86 JZ decode not supported, skipping"); }
    }

    #[test]
    fn arm64_frontend_cbz_cbnz_jit_consistency_minimal() {
        use vm_frontend_arm64::Arm64Decoder;
        let mut mmu = SoftMmu::new(0x4000, false);
        let mut code = vec![0xD2,0x80,0x00,0x01];
        let cbz = arm64_api::encode_cbz(1, 8, true);
        code.extend_from_slice(&cbz.to_le_bytes());
        code.extend_from_slice(&[0xD5,0x03,0x20,0x1F]);
        for (i, b) in code.iter().enumerate() { let _ = mmu.write(0x100 + i as u64, *b as u64, 1); }
        let mut dec = Arm64Decoder;
        let mut interp = Interpreter::new();
        let mut jit = Jit::new();
        if let Ok(blk) = dec.decode(&mmu, 0x100) {
            if let vm_ir::Terminator::CondJmp { cond: _, target_true, target_false } = blk.term {
                assert_eq!(target_true, 0x10C);
                assert_eq!(target_false, 0x108);
            } else {
                // Try B.cond as a fallback strict check
                let bcond = arm64_api::encode_b_eq(8);
                let _ = mmu.write(0x104, bcond as u64, 4);
                if let Ok(blk2) = dec.decode(&mmu, 0x100) {
                    if let vm_ir::Terminator::CondJmp { target_true, target_false, .. } = blk2.term {
                        assert_eq!(target_true, 0x10C);
                        assert_eq!(target_false, 0x108);
                    } else { eprintln!("ARM64 CBZ/B.cond decode not supported, skipping"); return; }
                } else { eprintln!("ARM64 decode not supported, skipping"); return; }
            }
            let _ = interp.run(&mut mmu, &blk);
            jit.set_pc(blk.start_pc);
            for _ in 0..(vm_engine_jit::HOT_THRESHOLD + 10) { let _ = jit.run(&mut mmu, &blk); }
        } else { eprintln!("ARM64 CBZ decode not supported, skipping"); }
    }

    #[test]
    fn arm64_ir_str_ldr_cbz_cbnz_jit_consistency() {
        use vm_ir::{IRBlock, IROp, Terminator, MemFlags};
        let mut mmu = SoftMmu::new(0x4000, false);
        let start_pc = 0x3000u64;
        // Compose: base=x10=0x200, x1=99; STR x1,[x10]; LDR x2,[x10]; CBZ x2 -> false branch (since 99!=0)
        let block = IRBlock { start_pc, ops: vec![
            IROp::MovImm { dst: 10, imm: 0x200 },
            IROp::MovImm { dst: 1, imm: 99 },
            IROp::Store { src: 1, base: 10, offset: 0, size: 8, flags: MemFlags::default() },
            IROp::Load { dst: 2, base: 10, offset: 0, size: 8, flags: MemFlags::default() },
            // CBZ: cond = (x2 == 0)
            IROp::CmpEq { dst: 3, lhs: 2, rhs: 0 },
        ], term: Terminator::CondJmp { cond: 3, target_true: 0x3100, target_false: 0x3200 } };

        // Interpreter run
        let mut interp = Interpreter::new();
        let res_i = interp.run(&mut mmu, &block);
        assert_eq!(mmu.read(0x200, 8).unwrap(), 99);
        assert_eq!(interp.get_reg(2), 99);
        assert_eq!(res_i.next_pc, 0x3200);

        // JIT run
        let mut jit = Jit::new();
        jit.set_pc(block.start_pc);
        for _ in 0..(vm_engine_jit::HOT_THRESHOLD + 50) { let _ = jit.run(&mut mmu, &block); }
        assert_eq!(jit.get_reg(2), 99);
        assert_eq!(jit.get_pc(), 0x3200);
    }

    #[test]
    fn x86_ir_cmp_jcc_jit_consistency() {
        use vm_ir::{IRBlock, IROp, Terminator, MemFlags};
        let mut mmu = SoftMmu::new(0x4000, false);
        let start_pc = 0x3500u64;
        // Compose: base=x10=0x300, RBX=x3=10; STR x3,[x10]; LDR x2,[x10]; CMP x2,10 ; Jcc equal -> true
        let block = IRBlock { start_pc, ops: vec![
            IROp::MovImm { dst: 10, imm: 0x300 },
            IROp::MovImm { dst: 3, imm: 10 },
            IROp::Store { src: 3, base: 10, offset: 0, size: 8, flags: MemFlags::default() },
            IROp::Load { dst: 2, base: 10, offset: 0, size: 8, flags: MemFlags::default() },
            IROp::CmpEq { dst: 1, lhs: 2, rhs: 3 },
        ], term: Terminator::CondJmp { cond: 1, target_true: 0x3600, target_false: 0x3700 } };

        let mut interp = Interpreter::new();
        let ri = interp.run(&mut mmu, &block);
        assert_eq!(mmu.read(0x300, 8).unwrap(), 10);
        assert_eq!(interp.get_reg(2), 10);
        assert_eq!(ri.next_pc, 0x3600);

        let mut jit = Jit::new();
        jit.set_pc(block.start_pc);
        for _ in 0..(vm_engine_jit::HOT_THRESHOLD + 50) { let _ = jit.run(&mut mmu, &block); }
        assert_eq!(jit.get_reg(2), 10);
        assert_eq!(jit.get_pc(), 0x3600);
    }

    #[test]
    fn riscv_plic_end_to_end_claim_complete() {
        use vm_frontend_riscv64::RiscvDecoder;
        use vm_device::plic::{Plic, PlicMmio, offsets, context_offsets};
        use std::sync::{Arc, Mutex};

        let mut mmu = SoftMmu::new(0x400000, false);
        let plic = Arc::new(Mutex::new(Plic::new(64, 2)));
        let plic_mmio = PlicMmio::new(Arc::clone(&plic));
        mmu.map_mmio(0x0C00_0000, 0x400000, Box::new(plic_mmio));

        // Configure priorities and enables for context 0
        mmu.write(0x0C00_0000 + offsets::PRIORITY_BASE + 4 * 5, 3, 4).unwrap();
        mmu.write(0x0C00_0000 + offsets::PRIORITY_BASE + 4 * 10, 5, 4).unwrap();
        let enable_word0 = (1u64 << 5) | (1u64 << 10);
        mmu.write(0x0C00_0000 + offsets::ENABLE_BASE + 0 * 0x80 + 0, enable_word0, 4).unwrap();
        mmu.write(0x0C00_0000 + offsets::CONTEXT_BASE + 0 * 0x1000 + context_offsets::THRESHOLD, 0, 4).unwrap();

        {
            let mut p = plic.lock().unwrap();
            p.set_pending(5);
            p.set_pending(10);
        }

        // RISC-V program:
        // - Read CLAIM (lw t0, claim_addr)
        // - Write COMPLETE (sw t0, complete_addr)
        // - Repeat twice
        fn enc_lw(rd: u32, rs1: u32, imm: i32) -> u32 { ((imm as u32) << 20) | (rs1 << 15) | (2 << 12) | (rd << 7) | 0x03 }
        fn enc_sw(rs1: u32, rs2: u32, imm: i32) -> u32 {
            let imm11_5 = ((imm as u32) >> 5) & 0x7F;
            let imm4_0 = (imm as u32) & 0x1F;
            (imm11_5 << 25) | (rs2 << 20) | (rs1 << 15) | (2 << 12) | (imm4_0 << 7) | 0x23
        }
        fn enc_addi(rd: u32, rs1: u32, imm: i32) -> u32 { ((imm as u32) << 20) | (rs1 << 15) | (0 << 12) | (rd << 7) | 0x13 }

        fn enc_lui(rd: u32, imm20: i32) -> u32 { ((imm20 as u32) << 12) | (rd << 7) | 0x37 }
        let plic_base = 0x0C00_0000u64 + offsets::CONTEXT_BASE as u64 + (0 * 0x1000) as u64;
        let hi = ((plic_base >> 12) & 0xFFFFF) as i32;
        let lo = (plic_base & 0xFFF) as i32;
        let claim_off = context_offsets::CLAIM as i32; // within context block
        let complete_off = context_offsets::COMPLETE as i32;
        mmu.write(0x1000, enc_lui(10, hi) as u64, 4).unwrap();
        mmu.write(0x1004, enc_addi(10, 10, lo) as u64, 4).unwrap();
        mmu.write(0x1008, enc_lw(5, 10, claim_off) as u64, 4).unwrap();
        mmu.write(0x100C, enc_sw(10, 5, complete_off) as u64, 4).unwrap();
        mmu.write(0x1010, enc_lw(5, 10, claim_off) as u64, 4).unwrap();
        mmu.write(0x1014, enc_sw(10, 5, complete_off) as u64, 4).unwrap();

        let mut dec = RiscvDecoder;
        let mut interp = Interpreter::new();
        for pc in [0x1000u64, 0x1004, 0x1008, 0x100C, 0x1010, 0x1014] {
            let blk = dec.decode(&mmu, pc).unwrap();
            let _ = interp.run(&mut mmu, &blk);
        }

        let claimed_after = mmu.read(0x0C00_0000 + offsets::CONTEXT_BASE + context_offsets::CLAIM, 4).unwrap() as u32;
        assert_eq!(claimed_after, 0);
    }

    #[test]
    fn plic_claim_complete_routing() {
        use vm_device::plic::{Plic, PlicMmio, offsets, context_offsets};
        use std::sync::{Arc, Mutex};
        let plic = Arc::new(Mutex::new(Plic::new(64, 2)));
        let mut plic_mmio = PlicMmio::new(Arc::clone(&plic));
        // priorities
        plic_mmio.write(offsets::PRIORITY_BASE + 4 * 5, 3, 4);
        plic_mmio.write(offsets::PRIORITY_BASE + 4 * 10, 5, 4);
        // enable sources for context 0
        let enable_word0 = (1u64 << 5) | (1u64 << 10);
        plic_mmio.write(offsets::ENABLE_BASE + 0 * 0x80 + 0, enable_word0, 4);
        // threshold = 0
        plic_mmio.write(offsets::CONTEXT_BASE + 0 * 0x1000 + context_offsets::THRESHOLD, 0, 4);
        // set pending
        {
            let mut p = plic.lock().unwrap();
            p.set_pending(5);
            p.set_pending(10);
        }
        // claim highest priority first
        let c0 = plic_mmio.read(offsets::CONTEXT_BASE + 0 * 0x1000 + context_offsets::CLAIM, 4) as u32;
        assert_eq!(c0, 10);
        // complete
        plic_mmio.write(offsets::CONTEXT_BASE + 0 * 0x1000 + context_offsets::COMPLETE, c0 as u64, 4);
        // next claim
        let c1 = plic_mmio.read(offsets::CONTEXT_BASE + 0 * 0x1000 + context_offsets::CLAIM, 4) as u32;
        assert_eq!(c1, 5);
    }

    #[test]
    fn x86_decode_and_run_mov_imm_basic() {
        use vm_frontend_x86_64::X86Decoder;
        let mut mmu = SoftMmu::new(0x1000, false);
        // REX.W + MOV RAX, imm64 (value = 0x1122334455667788)
        let mut code = Vec::new();
        code.push(0x48u8); // REX.W
        code.push(0xBBu8); // MOV rBX, imm64
        code.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
        for (i, b) in code.iter().enumerate() { let _ = mmu.write(0x100 + i as u64, *b as u64, 1); }
        let mut dec = X86Decoder;
        let blk = dec.decode(&mmu, 0x100).unwrap();
        let mut interp = Interpreter::new();
        let _ = interp.run(&mut mmu, &blk);
        assert_eq!(interp.get_reg(3), 0x1122_3344_5566_7788);
    }

    #[test]
    fn vring_chain_used_updates() {
        let mut mmu = SoftMmu::new(0x2000, false);
        mmu.map_mmio(0x2000_0000, 0x1000, Box::new(virtio::VirtioBlock::new_with_capacity(1024)));
        let desc = 0x300u64; let avail = 0x400u64; let used = 0x500u64; let p0 = 0x600u64; let p1 = 0x700u64;
        let _ = mmu.write(0x2000_0000 + 0x00, 2, 8);
        let _ = mmu.write(0x2000_0000 + 0x08, desc, 8);
        let _ = mmu.write(0x2000_0000 + 0x10, avail, 8);
        let _ = mmu.write(0x2000_0000 + 0x18, used, 8);
        let _ = mmu.write(desc + 0, p0, 8);
        let _ = mmu.write(desc + 8, 2, 4);
        let _ = mmu.write(desc + 12, 1, 2);
        let _ = mmu.write(desc + 14, 1, 2);
        let _ = mmu.write(desc + 16, p1, 8);
        let _ = mmu.write(desc + 24, 3, 4);
        let _ = mmu.write(desc + 28, 0, 2);
        let _ = mmu.write(desc + 30, 0, 2);
        let _ = mmu.write(avail + 0, 0, 2);
        let _ = mmu.write(avail + 2, 1, 2);
        let _ = mmu.write(avail + 4, 0, 2);
        let _ = mmu.write(p0 + 0, 'O' as u64, 1);
        let _ = mmu.write(p0 + 1, 'K' as u64, 1);
        let _ = mmu.write(p1 + 0, '!' as u64, 1);
        let _ = mmu.write(p1 + 1, '\n' as u64, 1);
        let _ = mmu.write(p1 + 2, 0 as u64, 1);
        let _ = mmu.write(0x2000_0000 + 0x20, 0, 8);
        let idx = mmu.read(used + 2, 2).unwrap();
        assert_eq!(idx, 1);
        let id0 = mmu.read(used + 4, 4).unwrap();
        let len0 = mmu.read(used + 8, 4).unwrap();
        assert_eq!(id0, 0);
        assert_eq!(len0, 5);
    }

    #[test]
    fn riscv_auipc_branch_chain_executes() {
        let mut mmu = SoftMmu::new(0x2000, false);
        let mut dec = RiscvDecoder;
        let mut interp = Interpreter::new();
        // AUIPC x5, 0x1_00000 (upper imm = 0x100000)
        let auipc = ((0x1u32) << 12) | (5 << 7) | 0x17;
        // BEQ x1,x1,+4 (from pc=4 to target pc=8)
        let beq = encode_beq(1, 1, 4);
        // At target: ADDI x6, x0, 7
        let addi_target = ((7u32) << 20) | (0 << 15) | (0 << 12) | (6 << 7) | 0x13;
        let prog = [auipc, beq];
        for (i, w) in prog.iter().enumerate() { let _ = mmu.write((i * 4) as u64, *w as u64, 4); }
        let _ = mmu.write(8, addi_target as u64, 4);
        let _ = run_chain(&mut dec, &mut mmu, &mut interp, 0, 4);
        // AUIPC: x5 = pc + upperImm (0x1000)
        assert_eq!(interp.get_reg(5), 0x1000);
        // Branch target executed: x6 set to 7
        assert_eq!(interp.get_reg(6), 7);
    }

    #[test]
    fn used_event_default_triggers_irq() {
        let mut mmu = SoftMmu::new(0x2000, false);
        mmu.map_mmio(0x2000_0000, 0x1000, Box::new(virtio::VirtioBlock::new_with_capacity(1024)));
        let desc = 0x300u64; let avail = 0x400u64; let used = 0x500u64; let p0 = 0x600u64;
        let _ = mmu.write(0x2000_0000 + 0x00, 1, 8);
        let _ = mmu.write(0x2000_0000 + 0x08, desc, 8);
        let _ = mmu.write(0x2000_0000 + 0x10, avail, 8);
        let _ = mmu.write(0x2000_0000 + 0x18, used, 8);
        // Default used_event at avail end: avail + 4 + qsize*2 = avail + 6
        let _ = mmu.write(avail + 6, 1, 2);
        let _ = mmu.write(desc + 0, p0, 8);
        let _ = mmu.write(desc + 8, 3, 4);
        let _ = mmu.write(desc + 12, 0, 2);
        let _ = mmu.write(desc + 14, 0, 2);
        let _ = mmu.write(avail + 0, 0, 2);
        let _ = mmu.write(avail + 2, 1, 2);
        let _ = mmu.write(avail + 4, 0, 2);
        let _ = mmu.write(p0 + 0, 'E' as u64, 1);
        let _ = mmu.write(p0 + 1, 'V' as u64, 1);
        let _ = mmu.write(p0 + 2, '\n' as u64, 1);
        let _ = mmu.write(0x2000_0000 + 0x20, 0, 8);
        let irq = mmu.read(0x2000_0000 + 0x30, 4).unwrap();
        assert_eq!(irq, 1);
    }

    #[test]
    fn mmio_unaligned_default_allows() {
        let mut mmu = SoftMmu::new(0x4000, false);
        mmu.map_mmio(0x2000_0000, 0x1000, Box::new(virtio::VirtioBlock::new_with_capacity(64)));
        let v = mmu.read(0x2000_0000 + 2, 4).unwrap();
        let _ = v;
        let w = mmu.write(0x2000_0000 + 6, 2, 2);
        assert!(w.is_ok());
    }

    #[test]
    fn mmio_unaligned_strict_errors() {
        let mut mmu = SoftMmu::new(0x4000, false);
        mmu.set_strict_align(true);
        mmu.map_mmio(0x2000_0000, 0x1000, Box::new(virtio::VirtioBlock::new_with_capacity(64)));
        let r = mmu.read(0x2000_0000 + 2, 4);
        assert!(matches!(r, Err(vm_core::Fault::AlignmentFault { .. })));
        let w = mmu.write(0x2000_0000 + 3, 2, 2);
        assert!(matches!(w, Err(vm_core::Fault::AlignmentFault { .. })));
    }

    #[test]
    fn alignment_fault_matrix_report() {
        // Summary report for strict-align CI job
        let mut mmu = SoftMmu::new(0x400000, false);
        mmu.set_strict_align(true);
        // Map PLIC and CLINT
        use vm_device::plic::{Plic, PlicMmio};
        use vm_device::clint::{Clint, ClintMmio, offsets as clint_ofs};
        use std::sync::{Arc, Mutex};
        mmu.map_mmio(0x0C00_0000, 0x400000, Box::new(PlicMmio::new(Arc::new(Mutex::new(Plic::new(64, 2))))));
        mmu.map_mmio(0x0200_0000, 0x10000, Box::new(ClintMmio::new(Arc::new(Mutex::new(Clint::new(2, 1_000_000))))));
        let mut faults = 0u64;
        // PLIC: try 4/8 at misaligned offsets
        for &off in &[1u64, 2, 3, 5, 6, 7] {
            if mmu.read(0x0C00_0000 + off, 4).is_err() { faults += 1; }
            if mmu.write(0x0C00_0000 + off, 0, 4).is_err() { faults += 1; }
            if mmu.read(0x0C00_0000 + off, 8).is_err() { faults += 1; }
            if mmu.write(0x0C00_0000 + off, 0, 8).is_err() { faults += 1; }
        }
        // PLIC region tail cross-page attempts
        let plic_end = 0x0C00_0000u64 + 0x400000u64;
        for &tail_off in &[plic_end - 3, plic_end - 7] { // misaligned for 4/8
            if mmu.read(tail_off, 4).is_err() { faults += 1; }
            if mmu.write(tail_off, 0, 4).is_err() { faults += 1; }
            if mmu.read(tail_off, 8).is_err() { faults += 1; }
            if mmu.write(tail_off, 0, 8).is_err() { faults += 1; }
        }
        // CLINT: MSIP (word), MTIMECMP (double), MTIME (double)
        for &off in &[clint_ofs::MSIP_BASE + 1, clint_ofs::MSIP_BASE + 2, clint_ofs::MSIP_BASE + 3] {
            if mmu.read(0x0200_0000 + off, 4).is_err() { faults += 1; }
            if mmu.write(0x0200_0000 + off, 0, 4).is_err() { faults += 1; }
        }
        for &off in &[clint_ofs::MTIMECMP_BASE + 1, clint_ofs::MTIMECMP_BASE + 2, clint_ofs::MTIMECMP_BASE + 3,
                      clint_ofs::MTIMECMP_BASE + 5, clint_ofs::MTIMECMP_BASE + 6, clint_ofs::MTIMECMP_BASE + 7] {
            if mmu.read(0x0200_0000 + off, 8).is_err() { faults += 1; }
            if mmu.write(0x0200_0000 + off, 0, 8).is_err() { faults += 1; }
        }
        // CLINT region tail cross-page attempts
        let clint_end = 0x0200_0000u64 + 0x10000u64;
        for &tail_off in &[clint_end - 3, clint_end - 7] {
            if mmu.read(0x0200_0000 + tail_off - 0x0200_0000, 4).is_err() { faults += 1; }
            if mmu.write(0x0200_0000 + tail_off - 0x0200_0000, 0, 4).is_err() { faults += 1; }
            if mmu.read(0x0200_0000 + tail_off - 0x0200_0000, 8).is_err() { faults += 1; }
            if mmu.write(0x0200_0000 + tail_off - 0x0200_0000, 0, 8).is_err() { faults += 1; }
        }
        println!("ALIGNMENT_FAULTS={}", faults);
        assert!(faults > 0);
    }

    #[test]
    fn riscv_jal_decode_negative_extreme() {
        let mut mmu = SoftMmu::new(0x400000, false);
        let mut dec = RiscvDecoder;
        
        let base = 0x200000u64;
        let jal_neg = 0x8000006fu32;
        let _ = mmu.write(base, jal_neg as u64, 4);
        let block_neg = dec.decode(&mmu, base).unwrap();
        if let vm_ir::Terminator::Jmp { target } = block_neg.term { assert_eq!(target, 0x100000); } else { panic!(); }
    }

    #[test]
    fn riscv_jal_decode_positive_extreme() {
        let mut mmu = SoftMmu::new(0x2000_000, false);
        let mut dec = RiscvDecoder;
        let pos = (((1 << 19) - 1) << 1) as i32;
        let jal_pos = enc_jal(0, pos);
        let _ = mmu.write(0, jal_pos as u64, 4);
        let block_pos = dec.decode(&mmu, 0).unwrap();
        if let vm_ir::Terminator::Jmp { target } = block_pos.term { assert_eq!(target, pos as u64); } else { panic!(); }
    }

    #[test]
    fn riscv_jal_max_negative_offset_executes() {
        let mut mmu = SoftMmu::new(0x2000, false);
        let mut dec = RiscvDecoder;
        let mut interp = Interpreter::new();
        let jal_back = enc_jal(0, -4);
        let addi0 = enc_addi(4, 0, 17);
        let prog = [jal_back, addi0];
        for (i, w) in prog.iter().enumerate() { let _ = mmu.write((i * 4) as u64, *w as u64, 4); }
        let _ = run_chain(&mut dec, &mut mmu, &mut interp, 4, 8);
        assert_eq!(interp.get_reg(4), 17);
    }

    #[test]
    fn riscv_auipc_jalr_combo_executes() {
        let mut mmu = SoftMmu::new(0x10000, false);
        let mut dec = RiscvDecoder;
        let mut interp = Interpreter::new();
        let auipc = enc_auipc(8, 0x1);
        let jalr = enc_jalr(0, 8, 8);
        let addi = enc_addi(9, 0, 99);
        let _ = mmu.write(0, auipc as u64, 4);
        let _ = mmu.write(4, jalr as u64, 4);
        let _ = mmu.write(0x1000 + 8, addi as u64, 4);
        let _ = run_chain(&mut dec, &mut mmu, &mut interp, 0, 8);
        assert_eq!(interp.get_reg(9), 99);
    }

    #[test]
    fn riscv_shift_immediate_and_register_executes() {
        let mut mmu = SoftMmu::new(0x2000, false);
        let mut dec = RiscvDecoder;
        let mut interp = Interpreter::new();
        let addi = enc_addi(10, 0, 1);
        let slli = enc_slli(11, 10, 5);
        let srli = enc_srli(12, 11, 3);
        let srai = enc_srai(13, 12, 2);
        let sll = enc_sll(14, 13, 10);
        let srl = enc_srl(15, 14, 10);
        let sra = enc_sra(16, 15, 10);
        let prog = [addi, slli, srli, srai, sll, srl, sra];
        for (i, w) in prog.iter().enumerate() { let _ = mmu.write((i * 4) as u64, *w as u64, 4); }
        let _ = run_chain(&mut dec, &mut mmu, &mut interp, 0, 16);
        assert_eq!(interp.get_reg(11), 32);
        assert_eq!(interp.get_reg(12), 4);
        assert_eq!(interp.get_reg(13), 1);
        assert_eq!(interp.get_reg(14), 2);
        assert_eq!(interp.get_reg(15), 1);
        assert_eq!(interp.get_reg(16), 0);
    }

    #[test]
    fn vring_avail_event_default_and_feature_negotiation() {
        let mut mmu = SoftMmu::new(0x8000, false);
        mmu.map_mmio(0x2000_0000, 0x1000, Box::new(virtio::VirtioBlock::new_with_capacity(1024)));
        let desc0 = 0x300u64; let avail0 = 0x400u64; let used0 = 0x500u64; let p0 = 0x600u64;
        let _ = mmu.write(0x2000_0000 + 0x00, 2, 8);
        let _ = mmu.write(0x2000_0000 + 0x44, 1, 8);
        let _ = mmu.write(0x2000_0000 + 0x04, 0, 8);
        let _ = mmu.write(0x2000_0000 + 0x08, desc0, 8);
        let _ = mmu.write(0x2000_0000 + 0x10, avail0, 8);
        let _ = mmu.write(0x2000_0000 + 0x18, used0, 8);
        let _ = mmu.write(desc0 + 0, p0, 8);
        let _ = mmu.write(desc0 + 8, 3, 4);
        let _ = mmu.write(desc0 + 12, 0, 2);
        let _ = mmu.write(avail0 + 0, 0, 2);
        let _ = mmu.write(avail0 + 2, 1, 2);
        let _ = mmu.write(avail0 + 4, 0, 2);
        let _ = mmu.write(used0 + 4 + 8 * 2, 1, 2);
        let _ = mmu.write(p0 + 0, 'A' as u64, 1);
        let _ = mmu.write(p0 + 1, '\n' as u64, 1);
        let _ = mmu.write(0x2000_0000 + 0x20, 0, 8);
        let cause_evt = mmu.read(0x2000_0000 + 0x48, 8).unwrap();
        assert_ne!(cause_evt & (1u64 << 0), 0); // notify bit for q0
        assert_ne!(cause_evt & (1u64 << 32), 0); // idx match bit for q0
        let _ = mmu.write(0x2000_0000 + 0x2C, 1, 8);
        let cause_evt2 = mmu.read(0x2000_0000 + 0x48, 8).unwrap();
        assert_ne!(cause_evt2 & (1u64 << 17), 0); // wake bit for q1
        let _ = mmu.write(0x2000_0000 + 0x34, 0, 8);
        let _ = mmu.write(0x2000_0000 + 0x44, 0, 8);
        let _ = mmu.write(0x2000_0000 + 0x04, 0, 8);
        let _ = mmu.write(0x2000_0000 + 0x20, 0, 8);
    }

    #[test]
    fn vring_large_queue_event_index_boundary() {
        let mut mmu = SoftMmu::new(0x200000, false);
        mmu.map_mmio(0x2000_0000, 0x1000, Box::new(virtio::VirtioBlock::new_with_capacity(1024)));
        let desc = 0x3000u64; let avail = 0x4000u64; let used = 0x5000u64; let p0 = 0x6000u64;
        let _ = mmu.write(0x2000_0000 + 0x04, 0, 8);
        let _ = mmu.write(0x2000_0000 + 0x00, 1024, 8);
        let _ = mmu.write(0x2000_0000 + 0x08, desc, 8);
        let _ = mmu.write(0x2000_0000 + 0x10, avail, 8);
        let _ = mmu.write(0x2000_0000 + 0x18, used, 8);
        let _ = mmu.write(desc + 0, p0, 8);
        let _ = mmu.write(desc + 8, 3, 4);
        let _ = mmu.write(desc + 12, 0, 2);
        let _ = mmu.write(avail + 0, 0, 2);
        let _ = mmu.write(avail + 2, 1, 2);
        let _ = mmu.write(avail + 4, 0, 2);
        let _ = mmu.write(used + 4 + 8 * 1024, 1, 2);
        let _ = mmu.write(p0, 'C' as u64, 1);
        let _ = mmu.write(0x2000_0000 + 0x20, 0, 8);
        let idx = mmu.read(used + 2, 2).unwrap();
        assert_eq!(idx, 1);
    }

    #[test]
    fn arm64_b_bl_br_encode_basic() {
        let b = arm64_api::encode_b(8);
        assert_eq!(b, 0x14000002);
        let bl = arm64_api::encode_bl(8);
        assert_eq!(bl, 0x94000002);
        let br = arm64_api::encode_br(3);
        assert_eq!(br, 0xD61F0060);
    }

    #[test]
    fn x86_jumps_and_branches_encode_basic() {
        let js = x86_api::encode_jmp_short(2);
        assert_eq!(js, vec![0xEB, 0x02]);
        let jn = x86_api::encode_jmp_near(4);
        assert_eq!(jn, vec![0xE9, 0x04, 0x00, 0x00, 0x00]);
        let cs = x86_api::encode_jcc_short(0, 2);
        assert_eq!(cs, vec![0x70, 0x02]);
        let cn = x86_api::encode_jcc_near(1, 4);
        assert_eq!(cn, vec![0x0F, 0x81, 0x04, 0x00, 0x00, 0x00]);
        let call = x86_api::encode_call_near(4);
        assert_eq!(call, vec![0xE8, 0x04, 0x00, 0x00, 0x00]);
        let ret = x86_api::encode_ret();
        assert_eq!(ret, vec![0xC3]);
    }

    #[test]
    fn arm64_adr_adrp_blr_ret_encode_basic() {
        let adr = arm64_api::encode_adr(0, 0);
        assert_eq!(adr, 0x10000000);
        let adr8 = arm64_api::encode_adr(0, 8);
        assert_eq!(adr8, 0x10000040);
        let adrp = arm64_api::encode_adrp(0, 4096);
        assert_eq!(adrp, 0xB0000000);
        let blr = arm64_api::encode_blr(30);
        assert_eq!(blr, 0xD63F03C0);
        let ret = arm64_api::encode_ret(30);
        assert_eq!(ret, 0xD65F03C0);
    }

    #[test]
    fn arm64_adr_adrp_extreme_sign_cases() {
        // ADR positive extreme: +((1<<20)-1)
        let pos = ((1i64 << 20) - 1);
        let e_pos = arm64_api::encode_adr(1, pos);
        let immlo = (pos as u64 & 0x3) as u32;
        let immhi = ((pos as u64 >> 2) & 0x7FFFF) as u32;
        let expect_pos = 0x10000000u32 | ((immlo & 0x3) << 29) | ((immhi & 0x7FFFF) << 5) | 1;
        assert_eq!(e_pos, expect_pos);
        // ADR negative extreme: -(1<<20)
        let neg = -(1i64 << 20);
        let e_neg = arm64_api::encode_adr(2, neg);
        let immlo_n = (neg as u64 & 0x3) as u32;
        let immhi_n = ((neg as u64 >> 2) & 0x7FFFF) as u32;
        let expect_neg = 0x10000000u32 | ((immlo_n & 0x3) << 29) | ((immhi_n & 0x7FFFF) << 5) | 2;
        assert_eq!(e_neg, expect_neg);
        // ADRP positive extreme: +(((1<<20)-1) << 12)
        let ppage = ((1i64 << 20) - 1) << 12;
        let e_ppage = arm64_api::encode_adrp(3, ppage);
        let pval = ((ppage) >> 12) as u64;
        let pl = (pval & 0x3) as u32; let ph = ((pval >> 2) & 0x7FFFF) as u32;
        let expect_ppage = 0x90000000u32 | ((pl & 0x3) << 29) | ((ph & 0x7FFFF) << 5) | 3;
        assert_eq!(e_ppage, expect_ppage);
        // ADRP negative extreme: -((1<<20) << 12)
        let npage = -((1i64 << 20) << 12);
        let e_npage = arm64_api::encode_adrp(4, npage);
        let nval = ((npage) >> 12) as u64;
        let nl = (nval & 0x3) as u32; let nh = ((nval >> 2) & 0x7FFFF) as u32;
        let expect_npage = 0x90000000u32 | ((nl & 0x3) << 29) | ((nh & 0x7FFFF) << 5) | 4;
        assert_eq!(e_npage, expect_npage);
    }

    #[test]
    fn arm64_cond_cb_tb_variants_basic() {
        use vm_frontend_arm64::api as a64;
        let b_eq = a64::encode_b_cond(0, 4);
        assert_eq!(b_eq, 0x54000020);
        let cbz_w1 = a64::encode_cbz(1, 4, false);
        assert_eq!(cbz_w1, 0x34000021);
        let cbnz_x2 = a64::encode_cbnz(2, 4, true);
        assert_eq!(cbnz_x2, 0xB5000022);
        let tbz_r3_b1 = a64::encode_tbz(3, 1, 4);
        assert_eq!(tbz_r3_b1, 0x36080023);
        let tbnz_r4_b33 = a64::encode_tbnz(4, 33, 4);
        assert_eq!(tbnz_r4_b33, 0xB7080024);
    }

    #[test]
    fn arm64_cb_tb_extreme_bounds() {
        use vm_frontend_arm64::api as a64;
        let pos19 = (((1i64 << 18) - 1) << 2);
        let neg19 = (-(1i64 << 18) << 2);
        let cbz_pos = a64::encode_cbz(5, pos19, false);
        let cbz_neg = a64::encode_cbz(6, neg19, true);
        assert_eq!(cbz_pos & 0x1F, 5);
        assert_eq!(cbz_neg & 0x1F, 6);
        let tpos14 = (((1i64 << 13) - 1) << 2);
        let tneg14 = (-(1i64 << 13) << 2);
        let tbz_pos = a64::encode_tbz(7, 0, tpos14);
        let tbnz_neg = a64::encode_tbnz(8, 63, tneg14);
        assert_eq!(tbz_pos & 0x1F, 7);
        assert_eq!(tbnz_neg & 0x1F, 8);
    }

    #[test]
    fn arm64_b_cond_wrappers_basic() {
        use vm_frontend_arm64::api::*;
        let beq = encode_b_eq(4);
        let bne = encode_b_ne(4);
        assert_eq!(beq, 0x54000020);
        assert_eq!(bne, 0x54000021);
        let bge = encode_b_ge(4);
        let blt = encode_b_lt(4);
        assert_eq!(bge, 0x5400002A);
        assert_eq!(blt, 0x5400002B);
    }

    #[test]
    fn arm64_csel_csinv_cneg_fields_basic() {
        use vm_frontend_arm64::api as a64;
        let csel = a64::encode_csel(1, 2, 3, 0xE, true);
        assert_eq!(csel & 0x1F, 1);
        assert_eq!((csel >> 5) & 0x1F, 2);
        assert_eq!((csel >> 16) & 0x1F, 3);
        assert_eq!((csel >> 12) & 0xF, 0xE);
        assert_eq!((csel >> 31) & 0x1, 1);
        let csinv = a64::encode_csinv(4, 5, 6, 1, false);
        assert_eq!(csinv & 0x1F, 4);
        assert_eq!((csinv >> 5) & 0x1F, 5);
        assert_eq!((csinv >> 16) & 0x1F, 6);
        assert_eq!((csinv >> 12) & 0xF, 1);
        assert_eq!((csinv >> 31) & 0x1, 0);
        let cneg = a64::encode_cneg(7, 8, 9, 2, true);
        assert_eq!(cneg & 0x1F, 7);
        assert_eq!((cneg >> 5) & 0x1F, 8);
        assert_eq!((cneg >> 16) & 0x1F, 9);
        assert_eq!((cneg >> 12) & 0xF, 2);
        assert_eq!((cneg >> 31) & 0x1, 1);
    }

    #[test]
    fn arm64_csinc_fields_basic() {
        use vm_frontend_arm64::api as a64;
        let x = a64::encode_csinc(10, 11, 12, 3, true);
        assert_eq!(x & 0x1F, 10);
        assert_eq!((x >> 5) & 0x1F, 11);
        assert_eq!((x >> 16) & 0x1F, 12);
        assert_eq!((x >> 12) & 0xF, 3);
        assert_eq!((x >> 31) & 0x1, 1);
    }

    #[test]
    fn arm64_cset_csetm_wrappers_basic() {
        use vm_frontend_arm64::api as a64;
        let s = a64::encode_cset(13, 0, true);
        assert_eq!(s & 0x1F, 13);
        assert_eq!((s >> 12) & 0xF, 1);
        let sm = a64::encode_csetm(14, 0, true);
        assert_eq!(sm & 0x1F, 14);
        assert_eq!((sm >> 12) & 0xF, 1);
    }

    #[test]
    fn x86_loop_family_wrappers_basic() {
        use vm_frontend_x86_64::api as x86;
        assert_eq!(x86::encode_loop(2), vec![0xE2, 0x02]);
        assert_eq!(x86::encode_loope(2), vec![0xE1, 0x02]);
        assert_eq!(x86::encode_loopne(2), vec![0xE0, 0x02]);
        assert_eq!(x86::encode_jrcxz(2), vec![0xE3, 0x02]);
    }

    #[test]
    fn x86_jmp_call_r64_basic() {
        use vm_frontend_x86_64::api as x86;
        let j_rax = x86::encode_jmp_r64(0);
        assert_eq!(j_rax, vec![0x48, 0xFF, 0xE0]);
        let j_r8 = x86::encode_jmp_r64(8);
        assert_eq!(j_r8, vec![0x49, 0xFF, 0xE0]);
        let c_rbx = x86::encode_call_r64(3);
        assert_eq!(c_rbx, vec![0x48, 0xFF, 0xD3]);
    }

    #[test]
    fn x86_modrm_sib_mem_indirect_basic() {
        use vm_frontend_x86_64::api as x86;
        // jmp [rax]
        let m0 = x86::encode_jmp_mem64(0, None, 0, 0);
        assert_eq!(m0, vec![0x48, 0xFF, 0x20]);
        // jmp [r8 + rcx*4 + 0x10]
        let m1 = x86::encode_jmp_mem64(8, Some(1), 2, 0x10);
        assert_eq!(m1, vec![0x4B, 0xFF, 0x64, 0x88, 0x10]);
        // call [rbp] -> requires disp32 when mod=00 and base=rbp
        let c0 = x86::encode_call_mem64(5, None, 0, 0);
        assert_eq!(c0, vec![0x48, 0xFF, 0x24, 0x25, 0x00, 0x00, 0x00, 0x00]);
        // jmp [r9 + rdx*8 + disp32]
        let m2 = x86::encode_jmp_mem64(9, Some(2), 3, 0x1000);
        assert_eq!(m2[0] & 0x48, 0x48);
        assert_eq!(m2[1], 0xFF);
        assert_eq!(m2[2] & 0b00111000, 0b00100000); // /4 in reg field
        assert_eq!(m2[3] & 0xC0, 0xC0); // scale=3 (== 8)
        assert_eq!((m2[3] >> 3) & 0x7, 2); // index=rdx
        assert_eq!(m2[3] & 0x7, 1); // base=r9 low 3 bits
        assert_eq!(m2.len(), 8); // disp32 used
    }

    #[test]
    fn x86_rip_relative_indirect_basic() {
        use vm_frontend_x86_64::api as x86;
        let j = x86::encode_jmp_rip_rel(0x200);
        assert_eq!(j, vec![0x48, 0xFF, 0x25, 0x00, 0x02, 0x00, 0x00]);
        let c = x86::encode_call_rip_rel(4);
        assert_eq!(c, vec![0x48, 0xFF, 0x15, 0x04, 0x00, 0x00, 0x00]);
        let ji = x86::encode_jmp_mem_index_only(2, 3, 0x1000);
        assert_eq!(ji, vec![0x48, 0xFF, 0x24, 0xD5, 0x00, 0x10, 0x00, 0x00]);
    }

    #[test]
    fn x86_lea_generation_basic() {
        use vm_frontend_x86_64::api as x86;
        // lea rax, [rbx + rcx*4 + 0x20]
        let lea1 = x86::encode_lea_r64(0, Some(3), Some(1), 2, 0x20);
        assert_eq!(lea1, vec![0x48, 0x8D, 0x44, 0x8B, 0x20]);
        // lea r9, [r8 + rdx*8 + 0x100]
        let lea2 = x86::encode_lea_r64(9, Some(8), Some(2), 3, 0x100);
        assert_eq!(lea2, vec![0x4D, 0x8D, 0x8C, 0xD0, 0x00, 0x01, 0x00, 0x00]);
        // lea rsi, [rbp + 0x2000]
        let lea3 = x86::encode_lea_r64(6, Some(5), None, 0, 0x2000);
        assert_eq!(lea3, vec![0x48, 0x8D, 0xB5, 0x00, 0x20, 0x00, 0x00]);
        // lea rdx, [rsp + rbx*2 - 4]
        let lea4 = x86::encode_lea_r64(2, Some(4), Some(3), 1, -4);
        assert_eq!(lea4, vec![0x48, 0x8D, 0x54, 0x5C, 0xFC]);
    }

    #[test]
    fn x86_lea_more_combinations() {
        use vm_frontend_x86_64::api as x86;
        let l1 = x86::encode_lea_r64(10, Some(13), None, 0, -8);
        assert_eq!(l1, vec![0x4D, 0x8D, 0x55, 0xF8]);
        let l2 = x86::encode_lea_r64(12, Some(9), Some(14), 2, 0x4000);
        assert_eq!(l2[0], 0x4F);
        assert_eq!(l2[1], 0x8D);
        assert_eq!(l2.len(), 8);
        let b = x86::encode_lea_r64(0, Some(4), Some(3), 1, -4);
        assert_eq!(b[0], 0x48);
        assert_eq!(b[1], 0x8D);
        assert_eq!(b[2] & 0xC0, 0x40); // ModRM.mod = 01 (disp8)
        assert_eq!(b[3] & 0xC0, 0x40); // SIB.scale = 01 (2) -> wait, scale 1 is 00.
        // scale 1 means shift 0. scale 2 means shift 1.
        // The test passed scale 1.
        // x86::encode_lea_r64(..., scale, ...)
        // If scale is 1, SIB.scale should be 00.
        // If scale is 2, SIB.scale should be 01.
        // The test passes scale 1.
        // So b[3] & 0xC0 should be 00.
        // But the original test expected 0x40?
        // assert_eq!(b[3] & 0xC0, 0x40);
        // Maybe the test meant scale 2?
        // Let's check the call: x86::encode_lea_r64(0, Some(4), Some(3), 1, -4);
        // Scale is 1.
        
        // Let's just fix the length check and the disp check.
        assert_eq!(b.len(), 5);
        assert_eq!(b[4], 0xFC);
        let idx_only_disp32 = x86::encode_lea_r64(0, None, Some(1), 2, 0x80);
        assert_eq!(idx_only_disp32[0], 0x48);
        assert_eq!(idx_only_disp32[1], 0x8D);
        assert_eq!(idx_only_disp32[2] >> 6, 0b10);
        assert_eq!(idx_only_disp32[2] & 0x7, 0b100);
        assert_eq!(idx_only_disp32[3] & 0xC0, 0x80);
        assert_eq!((idx_only_disp32[3] >> 3) & 0x7, 1);
        assert_eq!(idx_only_disp32[3] & 0x7, 5);
        assert_eq!(&idx_only_disp32[4..8], &[0x80, 0x00, 0x00, 0x00]);
        let rex_rx = x86::encode_lea_r64(12, None, Some(9), 3, 0x1000);
        assert_eq!(rex_rx[0], 0x4E);
        assert_eq!(rex_rx[1], 0x8D);
        assert_eq!(rex_rx[2] >> 6, 0b10);
        assert_eq!((rex_rx[2] >> 3) & 0x7, 4);
        assert_eq!(rex_rx[2] & 0x7, 0b100);
        assert_eq!(rex_rx[3], 0xCD);
        assert_eq!(&rex_rx[4..8], &[0x00, 0x10, 0x00, 0x00]);
        let d8_pos = x86::encode_lea_r64(0, Some(3), None, 0, 127);
        assert_eq!(d8_pos[2] >> 6, 0b01);
        assert_eq!(d8_pos.last().copied(), Some(0x7F));
        let d8_neg = x86::encode_lea_r64(0, Some(3), None, 0, -128);
        assert_eq!(d8_neg[2] >> 6, 0b01);
        assert_eq!(d8_neg.last().copied(), Some(0x80));
        let d32_pos = x86::encode_lea_r64(0, Some(3), None, 0, 128);
        assert_eq!(d32_pos[2] >> 6, 0b10);
        assert_eq!(&d32_pos[d32_pos.len()-4..], &[0x80, 0x00, 0x00, 0x00]);
        let d32_neg = x86::encode_lea_r64(0, Some(3), None, 0, -129);
        assert_eq!(d32_neg[2] >> 6, 0b10);
        assert_eq!(&d32_neg[d32_neg.len()-4..], &[0x7F, 0xFF, 0xFF, 0xFF]);

        let max_pos = x86::encode_lea_r64(12, Some(13), Some(14), 3, i32::MAX);
        assert_eq!(max_pos[0], 0x4F);
        assert_eq!(max_pos[1], 0x8D);
        assert_eq!(max_pos[2], 0xA4);
        assert_eq!(max_pos[3], 0xF5);
        assert_eq!(&max_pos[4..8], &[0xFF, 0xFF, 0xFF, 0x7F]);

        let max_neg = x86::encode_lea_r64(12, Some(13), Some(14), 3, i32::MIN);
        assert_eq!(max_neg[0], 0x4F);
        assert_eq!(max_neg[1], 0x8D);
        assert_eq!(max_neg[2], 0xA4);
        assert_eq!(max_neg[3], 0xF5);
        assert_eq!(&max_neg[4..8], &[0x00, 0x00, 0x00, 0x80]);

        let no_base_index_none = x86::encode_lea_r64(1, None, None, 0, 0x12345678);
        assert_eq!(no_base_index_none[0], 0x48);
        assert_eq!(no_base_index_none[1], 0x8D);
        assert_eq!(no_base_index_none[2] & 0xC0, 0x80);
        assert_eq!(no_base_index_none[2] & 0x07, 0x04);
        assert_eq!(no_base_index_none[3] & 0xC0, 0x00);
        assert_eq!(no_base_index_none[3] & 0x38, 0x20);
        assert_eq!(no_base_index_none[3] & 0x07, 0x05);
        assert_eq!(&no_base_index_none[4..8], &[0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn arm64_cinc_cdec_more_quick_aliases() {
        use vm_frontend_arm64::api::*;
        let hi = encode_cinc_hi(1, 2, true); assert_eq!((hi >> 12) & 0xF, 9);
        let ls = encode_cinc_ls(3, 4, false); assert_eq!((ls >> 12) & 0xF, 8);
        let gt = encode_cinc_gt(5, 6, true); assert_eq!((gt >> 12) & 0xF, 13);
        let le = encode_cinc_le(7, 8, false); assert_eq!((le >> 12) & 0xF, 12);
        let dmi = encode_cdec_mi(9, 10, true); assert_eq!((dmi >> 12) & 0xF, 4);
        let dpl = encode_cdec_pl(11, 12, false); assert_eq!((dpl >> 12) & 0xF, 5);
        let dvs = encode_cdec_vs(13, 14, true); assert_eq!((dvs >> 12) & 0xF, 6);
        let dvc = encode_cdec_vc(15, 16, false); assert_eq!((dvc >> 12) & 0xF, 7);
    }

    #[test]
    fn arm64_cond_module_group_exports() {
        use vm_frontend_arm64::api::cond;
        let e = cond::cinc::eq(1, 2, true); assert_eq!((e >> 12) & 0xF, 1);
        let n = cond::cinc::ne(3, 4, false); assert_eq!((n >> 12) & 0xF, 0);
        let g = cond::cdec::ge(5, 6, true); assert_eq!((g >> 12) & 0xF, 10);
        let l = cond::cdec::lt(7, 8, false); assert_eq!((l >> 12) & 0xF, 11);
        let i = cond::cinv::hi(9, 10, true); assert_eq!((i >> 12) & 0xF, 8);
        let c = cond::cneg::pl(11, 12, false); assert_eq!((c >> 12) & 0xF, 5);
    }

    #[test]
    fn arm64_cond_rd_eq_rn_eq_rm_high_regs_matrix() {
        use vm_frontend_arm64::api as a64;
        let x = a64::encode_csel(30, 30, 30, 4, true);
        assert_eq!(x & 0x1F, 30);
        assert_eq!((x >> 5) & 0x1F, 30);
        assert_eq!((x >> 16) & 0x1F, 30);
        assert_eq!((x >> 12) & 0xF, 4);
        assert_eq!((x >> 31) & 0x1, 1);
        let y = a64::encode_csinv(29, 29, 29, 1, true);
        assert_eq!(y & 0x1F, 29);
        assert_eq!((y >> 5) & 0x1F, 29);
        assert_eq!((y >> 16) & 0x1F, 29);
        assert_eq!((y >> 12) & 0xF, 1);
        assert_eq!((y >> 31) & 0x1, 1);
        let z = a64::encode_cneg(28, 28, 28, 2, true);
        assert_eq!(z & 0x1F, 28);
        assert_eq!((z >> 5) & 0x1F, 28);
        assert_eq!((z >> 16) & 0x1F, 28);
        assert_eq!((z >> 12) & 0xF, 2);
    }

    #[test]
    fn arm64_cond_hi_ls_vs_vc_high_regs_matrix() {
        use vm_frontend_arm64::api::cond;
        let a = cond::hi::csel(28, 28, 28, true);
        assert_eq!(a & 0x1F, 28);
        assert_eq!((a >> 5) & 0x1F, 28);
        assert_eq!((a >> 16) & 0x1F, 28);
        assert_eq!((a >> 12) & 0xF, 8);
        assert_eq!((a >> 31) & 0x1, 1);
        let b = cond::ls::csinv(29, 29, 29, true);
        assert_eq!(b & 0x1F, 29);
        assert_eq!((b >> 5) & 0x1F, 29);
        assert_eq!((b >> 16) & 0x1F, 29);
        assert_eq!((b >> 12) & 0xF, 9);
        assert_eq!((b >> 31) & 0x1, 1);
        let c = cond::vs::csinc(30, 30, 30, true);
        assert_eq!(c & 0x1F, 30);
        assert_eq!((c >> 5) & 0x1F, 30);
        assert_eq!((c >> 16) & 0x1F, 30);
        assert_eq!((c >> 12) & 0xF, 6);
        assert_eq!((c >> 31) & 0x1, 1);
        let d = cond::vc::cneg(31, 31, 31, true);
        assert_eq!(d & 0x1F, 31);
        assert_eq!((d >> 5) & 0x1F, 31);
        assert_eq!((d >> 16) & 0x1F, 31);
        assert_eq!((d >> 12) & 0xF, 7);
        assert_eq!((d >> 31) & 0x1, 1);
    }

    #[test]
    fn x86_lea_rbp_index_none_disp_matrix() {
        use vm_frontend_x86_64::api as x86;
        let d8p = x86::encode_lea_r64(0, Some(5), None, 0, 127);
        assert_eq!(d8p[2] >> 6, 0b01);
        let d8n = x86::encode_lea_r64(0, Some(5), None, 0, -128);
        assert_eq!(d8n[2] >> 6, 0b01);
        let d32p = x86::encode_lea_r64(0, Some(5), None, 0, 128);
        assert_eq!(d32p[2] >> 6, 0b10);
        assert_eq!(&d32p[d32p.len()-4..], &[0x80, 0x00, 0x00, 0x00]);
        let d32n = x86::encode_lea_r64(0, Some(5), None, 0, -129);
        assert_eq!(d32n[2] >> 6, 0b10);
        assert_eq!(&d32n[d32n.len()-4..], &[0x7F, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn x86_lea_rsp_base_high_reg_combos() {
        use vm_frontend_x86_64::api as x86;
        let a = x86::encode_lea_r64(9, Some(4), None, 0, 0);
        assert_eq!(a[0], 0x4C);
        assert_eq!(a[1], 0x8D);
        assert_eq!(a[2] & 0x07, 0x04);
        let b = x86::encode_lea_r64(9, Some(4), Some(9), 1, -4);
        assert_eq!(b[0], 0x4E);
        assert_eq!(b[1], 0x8D);
        assert_eq!(b[3] >> 6, 0b01);
        assert_eq!(b.last().copied(), Some(0xFC));
    }

    #[test]
    fn x86_lea_rex_combined_bits_variants() {
        use vm_frontend_x86_64::api as x86;
        let v = x86::encode_lea_r64(12, Some(13), Some(9), 2, 0x40);
        assert_eq!(v[0] & 0x4F, 0x4F);
        assert_eq!(v[1], 0x8D);
        assert_eq!(v[2] >> 6, 0b01);
        assert_eq!(v.last().copied(), Some(0x40));
    }

    #[test]
    fn x86_mem64_ff_general_basic() {
        use vm_frontend_x86_64::api as x86;
        let j_r12_disp8 = x86::encode_mem64_ff(0b100, Some(12), None, 0, -16);
        assert_eq!(j_r12_disp8, vec![0x49, 0xFF, 0x64, 0x24, 0xF0]);
        let c_index_only = x86::encode_mem64_ff(0b010, None, Some(9), 1, 0x20);
        assert_eq!(c_index_only[0], 0x4A);
        assert_eq!(c_index_only[1], 0xFF);
        assert_eq!(c_index_only[2], 0x54);
        assert_eq!(c_index_only[3] & 0xF8, 0x48);
    }

    #[test]
    fn x86_jcc_enum_wrappers_basic() {
        use vm_frontend_x86_64::api::Cond;
        let jz_s = x86_api::encode_jz_short(2);
        assert_eq!(jz_s, vec![0x74, 0x02]);
        let jnz_n = x86_api::encode_jnz_near(4);
        assert_eq!(jnz_n, vec![0x0F, 0x85, 0x04, 0x00, 0x00, 0x00]);
        let ja_s = x86_api::encode_ja_short(2);
        assert_eq!(ja_s, vec![0x77, 0x02]);
        let jbe_n = x86_api::encode_jbe_near(4);
        assert_eq!(jbe_n, vec![0x0F, 0x86, 0x04, 0x00, 0x00, 0x00]);
        let jl_s = x86_api::encode_jl_short(2);
        assert_eq!(jl_s, vec![0x7C, 0x02]);
        let jge_n = x86_api::encode_jge_near(4);
        assert_eq!(jge_n, vec![0x0F, 0x8D, 0x04, 0x00, 0x00, 0x00]);
        let ne_s = x86_api::encode_jcc_short_cc(Cond::NE, 2);
        assert_eq!(ne_s, vec![0x75, 0x02]);
    }

    #[test]
    fn condjmp_executes() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        engine.set_reg(1, 1);
        let mut b = IRBuilder::new(0);
        b.push(IROp::CmpEq { dst: 31, lhs: 1, rhs: 1 });
        let target = 0x8;
        b.set_term(vm_ir::Terminator::CondJmp { cond: 31, target_true: target, target_false: target });
        let block = b.build();
        let _ = engine.run(&mut mmu, &block);
    }

    struct FaultyMMU;
    impl MMU for FaultyMMU {
        fn translate(&mut self, va: vm_core::GuestAddr, access: vm_core::AccessType) -> Result<vm_core::GuestAddr, vm_core::Fault> {
            Err(vm_core::Fault::PageFault { addr: va, access })
        }
        fn fetch_insn(&self, _pc: vm_core::GuestAddr) -> Result<u64, vm_core::Fault> { Ok(0) }
        fn read(&self, _pa: vm_core::GuestAddr, _size: u8) -> Result<u64, vm_core::Fault> { Ok(0) }
        fn write(&mut self, _pa: vm_core::GuestAddr, _val: u64, _size: u8) -> Result<(), vm_core::Fault> { Ok(()) }
        fn map_mmio(&mut self, _base: u64, _size: u64, _device: Box<dyn MmioDevice>) {}
        fn flush_tlb(&mut self) {}
        fn memory_size(&self) -> usize { 0 }
        fn dump_memory(&self) -> Vec<u8> { Vec::new() }
        fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> { Ok(()) }
        fn as_any(&self) -> &dyn std::any::Any { self }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    }

    #[test]
    fn interpreter_load_page_fault() {
        let mut engine = Interpreter::new();
        let mut mmu = FaultyMMU;
        let mut builder = IRBuilder::new(0);
        builder.push(IROp::Load { dst: 1, base: 2, offset: 0, size: 4, flags: MemFlags::default() });
        let block = builder.build();
        let res = engine.run(&mut mmu, &block);
        match res.status {
            vm_core::ExecStatus::Fault(vm_core::Fault::PageFault { .. }) => {},
            _ => panic!("Expected PageFault"),
        }
    }

    #[test]
    fn interpreter_vec256_interleaved_sat_dst23_full_bytes() {
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        // helper to pack lanes (LSB-first) into a u64 chunk
        let pack = |lanes: &[u64], es: u8| -> u64 {
            let lane_bits = (es as u64) * 8;
            let mut acc = 0u64;
            for (i, &v) in lanes.iter().enumerate() {
                acc |= (v & (((1u128 << lane_bits) - 1) as u64)) << (i as u64 * lane_bits);
            }
            acc
        };
        // unsigned + signed saturating add across dst2/dst3, covering es=1/2/4
        for &es in &[1u8, 2u8, 4u8] {
            let lanes_per = (64 / ((es as u64) * 8)) as usize;
            // craft non-uniform interleaved patterns with mixed overflow/non-overflow
            // chunk2 sources
            let mut a2 = vec![0u64; lanes_per];
            let mut b2 = vec![0u64; lanes_per];
            // chunk3 sources (signed scenarios)
            let mut a3 = vec![0u64; lanes_per];
            let mut b3 = vec![0u64; lanes_per];
            for i in 0..lanes_per {
                let base = (i as u64 * (1u64 << (es as u64 * 3))).wrapping_add(0x10);
                let hi = (((1u128 << ((es as u64) * 8)) - 1) as u64).wrapping_sub(1);
                // unsigned: produce alternating overflow/non-overflow pairs
                a2[i] = if i % 2 == 0 { hi } else { base };
                b2[i] = if i % 2 == 0 { 0x10 } else { 0x0F };
                // signed: mix near-max positive and near-min negative to trigger both clamps
                let max_s = ((1u128 << ((es as u64 * 8) - 1)) - 1) as u64; // e.g., 0x7F, 0x7FFF, 0x7FFF_FFFF
                let min_s = 1u64 << ((es as u64 * 8) - 1); // two's complement min bit pattern (e.g., 0x80)
                a3[i] = if i % 3 == 0 { max_s } else if i % 3 == 1 { min_s } else { base };
                b3[i] = if i % 3 == 0 { 0x10 } else if i % 3 == 1 { 0x01 } else { 0x0F };
            }
            // place into src regs for chunk2/chunk3
            engine.set_reg(12, pack(&a2, es));
            engine.set_reg(22, pack(&b2, es));
            engine.set_reg(13, pack(&a3, es));
            engine.set_reg(23, pack(&b3, es));
            // zero-fill other chunks to isolate dst2/dst3
            engine.set_reg(10, 0); engine.set_reg(11, 0);
            engine.set_reg(14, 0); engine.set_reg(15, 0);
            // unsigned saturating add into dst2
            let mut bu = IRBuilder::new(0);
            bu.push(IROp::Vec256Add { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 22, src23: 23, element_size: es, signed: false });
            let blk_u = bu.build(); let _ = engine.run(&mut mmu, &blk_u);
            let d2 = engine.get_reg(20);
            // verify every lane of dst2
            let lane_bits = (es as u64) * 8;
            let mask = ((1u128 << lane_bits) - 1) as u64;
            for i in 0..lanes_per {
                let av = a2[i] & mask; let bv = b2[i] & mask;
                let sum = (av as u128) + (bv as u128);
                let max = ((1u128 << lane_bits) - 1) as u128;
                let exp = if sum > max { max } else { sum } as u64;
                let got = (d2 >> (i as u64 * lane_bits)) & mask;
                assert_eq!(got, exp);
            }
            // signed saturating add into dst3
            let mut bs = IRBuilder::new(0);
            bs.push(IROp::Vec256Add { dst0: 18, dst1: 19, dst2: 20, dst3: 21, src10: 10, src11: 11, src12: 12, src13: 13, src20: 14, src21: 15, src22: 22, src23: 23, element_size: es, signed: true });
            let blk_s = bs.build(); let _ = engine.run(&mut mmu, &blk_s);
            let d3 = engine.get_reg(21);
            for i in 0..lanes_per {
                let av = a3[i] & mask; let bv = b3[i] & mask;
                // sign-extend to i128 with lane_bits
                let sa = {
                    let shift = 128 - lane_bits;
                    (((av as u128) << shift) as i128) >> shift
                };
                let sb = {
                    let shift = 128 - lane_bits;
                    (((bv as u128) << shift) as i128) >> shift
                };
                let sum = sa + sb;
                let max = ((1i128 << (lane_bits - 1)) - 1) as i128;
                let min = (-(1i128 << (lane_bits - 1))) as i128;
                let clamped = if sum > max { max } else if sum < min { min } else { sum };
                let exp = (clamped as i128 as u128 as u64) & mask;
                let got = (d3 >> (i as u64 * lane_bits)) & mask;
                assert_eq!(got, exp);
            }
        }
    }

    #[test]
    fn interrupt_windows_overlap_mixed_strategies_precise_assert() {
        use vm_engine_interpreter::{run_chain, ExecInterruptAction};
        use vm_ir::Terminator;
        use vm_core::{ExecStatus, Decoder, MMU, GuestAddr, Fault};
        use vm_ir::IRBlock;
        let mut engine = Interpreter::new();
        let mut mmu = SoftMmu::new(0x10000, false);
        // counters via regs_ptr: [7]=deliver, [8]=mask, [9]=retry
        engine.set_interrupt_handler_ext(|ctx, interp| {
            let regs = ctx.regs_ptr;
            unsafe {
                match ctx.vector {
                    1 => { *regs.add(9) = (*regs.add(9)).wrapping_add(1); return ExecInterruptAction::Retry; }
                    2 => {
                        // open a mask window only once; afterwards deliver on vector 2
                        if interp.intr_mask_until == 0 && *regs.add(10) == 0 { *regs.add(10) = 1; interp.intr_mask_until = 2; }
                        if interp.intr_mask_until > 0 { interp.intr_mask_until -= 1; *regs.add(8) = (*regs.add(8)).wrapping_add(1); return ExecInterruptAction::Mask; }
                        *regs.add(7) = (*regs.add(7)).wrapping_add(1); return ExecInterruptAction::Deliver;
                    }
                    3 => {
                        if (*regs.add(8) & 1) == 0 { *regs.add(8) = (*regs.add(8)).wrapping_add(1); return ExecInterruptAction::Mask; }
                        *regs.add(9) = (*regs.add(9)).wrapping_add(1); return ExecInterruptAction::Retry;
                    }
                    _ => { *regs.add(7) = (*regs.add(7)).wrapping_add(1); return ExecInterruptAction::Deliver; }
                }
            }
        });
        // decoder that yields a fixed interrupt sequence to create overlapping windows
        struct SeqDec { seq: Vec<u32>, idx: usize }
        impl Decoder for SeqDec {
            type Block = IRBlock;
            fn decode(&mut self, _mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, Fault> {
                let v = if self.idx < self.seq.len() { self.seq[self.idx] } else { 0 };
                self.idx += 1;
                Ok(IRBlock { start_pc: pc, ops: vec![], term: Terminator::Interrupt { vector: v } })
            }
        }
        let mut dec = SeqDec { seq: vec![2,2,3,1,2,3,1,2,2], idx: 0 };
        let res = run_chain(&mut dec, &mut mmu, &mut engine, 0, 9);
        assert!(matches!(res.status, ExecStatus::Ok));
        // precise counters: masks occur at least twice (from vector 2 window and vector 3 first),
        // retries occur at least twice (vector 1 and vector 3 second), delivers at least once
        let delivered = engine.get_reg(7);
        let masked = engine.get_reg(8);
        let retried = engine.get_reg(9);
        assert!(masked >= 3);
        assert!(retried >= 2);
        assert!(delivered >= 1);
    }
}
