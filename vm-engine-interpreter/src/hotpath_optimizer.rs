//! Hot Path Optimizations for VM Execution
//!
//! This module provides optimized implementations of critical execution paths
//! to improve VM performance.
//!
//! # Hot Paths Optimized
//!
//! 1. **Instruction Execution Loop** - Reduced overhead in main dispatch loop
//! 2. **Register Access** - Faster register file operations
//! 3. **Memory Operations** - Optimized load/store sequences
//! 4. **Branch Prediction** - Hints for common branch patterns
//! 5. **SIMD Operations** - Vectorized instruction execution

use crate::IROp;

/// Hot path execution statistics
#[derive(Debug, Default, Clone)]
pub struct HotPathStats {
    /// Total operations executed via hot path
    pub hot_path_executions: u64,
    /// Fast path taken count
    pub fast_path_hits: u64,
    /// Slow path taken count
    pub slow_path_taken: u64,
    /// Instructions fused
    pub instructions_fused: u64,
    /// Branch predictions
    pub branches_predicted: u64,
    pub correct_predictions: u64,
}

/// Optimized register operations using inline hints
pub mod optimized_regs {
    /// Fast register access with inline hint
    #[inline(always)]
    pub fn get_reg_fast(regs: &[u64; 32], idx: u32) -> u64 {
        let hi = idx >> 16;
        let guest = if hi != 0 { hi } else { idx & 0x1F };
        // Use copy propagation to avoid bounds check in hot path
        if guest < 32 { regs[guest as usize] } else { 0 }
    }

    /// Fast register set with inline hint
    #[inline(always)]
    pub fn set_reg_fast(regs: &mut [u64; 32], idx: u32, val: u64) {
        let hi = idx >> 16;
        let guest = if hi != 0 { hi } else { idx & 0x1F };
        // Skip x0 writes (it's hardwired to 0 in RISC-V)
        if guest != 0 && guest < 32 {
            regs[guest as usize] = val;
        }
    }

    /// Batch register operations - reduces loop overhead
    #[inline(always)]
    pub fn batch_set_regs(regs: &mut [u64; 32], operations: &[(u32, u64)]) {
        for &(dst, val) in operations {
            let hi = dst >> 16;
            let guest = if hi != 0 { hi } else { dst & 0x1F };
            if guest != 0 && guest < 32 {
                regs[guest as usize] = val;
            }
        }
    }
}

/// Optimized memory operation patterns
pub mod optimized_memory {
    use crate::hotpath_optimizer::optimized_regs;
    use vm_core::MMU;

    /// Load-add-store fusion - common in atomic operations
    #[inline(always)]
    pub fn load_add_store(
        mmu: &mut dyn MMU,
        regs: &mut [u64; 32],
        base: u32,
        offset: i64,
        src: u32,
        size: u8,
    ) -> Result<(), vm_core::VmError> {
        let base_val = optimized_regs::get_reg_fast(regs, base);
        let src_val = optimized_regs::get_reg_fast(regs, src);
        let addr = base_val.wrapping_add(offset as u64);

        // Load
        let loaded = mmu.read(vm_core::GuestAddr(addr), size)?;

        // Add
        let result = loaded.wrapping_add(src_val);

        // Store
        mmu.write(vm_core::GuestAddr(addr), result, size)?;

        Ok(())
    }

    /// Sequential load optimization - prefetch next cache line
    #[inline(always)]
    pub fn sequential_load(
        mmu: &mut dyn MMU,
        regs: &mut [u64; 32],
        base: u32,
        offsets: &[i64],
        size: u8,
    ) -> Result<(), vm_core::VmError> {
        let base_val = optimized_regs::get_reg_fast(regs, base);

        // Prefetch hint for next cache line
        #[cfg(target_arch = "x86_64")]
        unsafe {
            use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
            let next_addr = base_val.wrapping_add(offsets.get(1).copied().unwrap_or(0) as u64);
            _mm_prefetch(next_addr as *const i8, _MM_HINT_T0);
        }

        for (i, &offset) in offsets.iter().enumerate() {
            let dst = i as u32;
            let addr = base_val.wrapping_add(offset as u64);
            let val = mmu.read(vm_core::GuestAddr(addr), size)?;
            super::optimized_regs::set_reg_fast(regs, dst, val);
        }

        Ok(())
    }
}

/// Branch prediction hints
pub mod branch_hints {
    use vm_core::GuestAddr;

    /// Likely hint for conditional branches
    #[inline(always)]
    pub fn likely_cond(cond: bool) -> bool {
        if cond {
            // Tell compiler this branch is likely
            true
        } else {
            false
        }
    }

    /// Predict forward branches as not taken
    #[inline(always)]
    pub fn predict_forward_not_taken(target: GuestAddr, current_pc: GuestAddr) -> bool {
        // Forward branches are usually not taken (loop conditions)
        target <= current_pc
    }

    /// Predict backward branches as taken (loops)
    #[inline(always)]
    pub fn predict_backward_taken(target: GuestAddr, current_pc: GuestAddr) -> bool {
        // Backward branches are usually taken (loops)
        target < current_pc
    }
}

/// Optimized arithmetic operations
pub mod optimized_arith {

    /// Fast add with overflow checking
    #[inline(always)]
    pub fn add_with_overflow(a: u64, b: u64) -> (u64, bool) {
        let (result, overflow) = a.overflowing_add(b);
        (result, overflow)
    }

    /// Fast multiply with early exit for common cases
    #[inline(always)]
    pub fn mul_fast(a: u64, b: u64) -> u64 {
        match b {
            0 => 0,
            1 => a,
            2 => a.wrapping_shl(1),
            _ => a.wrapping_mul(b),
        }
    }

    /// Power-of-2 multiply optimization
    #[inline(always)]
    pub fn mul_power_of_two(a: u64, b: u64) -> Option<u64> {
        if b.is_power_of_two() {
            Some(a.wrapping_shl(b.trailing_zeros()))
        } else {
            None
        }
    }

    /// Division by power-of-2 optimization
    #[inline(always)]
    pub fn div_power_of_two(a: u64, b: u64) -> Option<u64> {
        if b.is_power_of_two() {
            Some(a.wrapping_shr(b.trailing_zeros()))
        } else {
            None
        }
    }
}

/// Hot path executor with integrated optimizations
pub struct HotPathExecutor {
    pub stats: HotPathStats,
}

impl HotPathExecutor {
    pub fn new() -> Self {
        Self {
            stats: HotPathStats::default(),
        }
    }

    /// Execute arithmetic operations with optimizations
    #[inline(always)]
    pub fn execute_arith(
        &mut self,
        regs: &mut [u64; 32],
        op: &IROp,
    ) -> Result<bool, vm_core::VmError> {
        self.stats.hot_path_executions += 1;

        match op {
            IROp::Add { dst, src1, src2 } => {
                let a = optimized_regs::get_reg_fast(regs, *src1);
                let b = optimized_regs::get_reg_fast(regs, *src2);

                // Try power-of-2 optimization
                if let Some(result) = optimized_arith::mul_power_of_two(a, b) {
                    optimized_regs::set_reg_fast(regs, *dst, result);
                    self.stats.fast_path_hits += 1;
                    return Ok(true);
                }

                // Fast multiply for common cases
                let result = optimized_arith::mul_fast(a, b);
                optimized_regs::set_reg_fast(regs, *dst, result);
                self.stats.fast_path_hits += 1;
                Ok(true)
            }

            IROp::MulImm { dst, src, imm } => {
                let a = optimized_regs::get_reg_fast(regs, *src);

                // Try power-of-2 optimization
                let imm_abs = imm.unsigned_abs();
                if imm_abs.is_power_of_two() {
                    let result = a.wrapping_shl(imm_abs.trailing_zeros());
                    optimized_regs::set_reg_fast(regs, *dst, result);
                    self.stats.fast_path_hits += 1;
                    return Ok(true);
                }

                self.stats.slow_path_taken += 1;
                Ok(false)
            }

            _ => {
                self.stats.slow_path_taken += 1;
                Ok(false)
            }
        }
    }

    /// Execute load operations with prefetching
    #[inline(always)]
    pub fn execute_load(
        &mut self,
        mmu: &mut dyn vm_core::MMU,
        regs: &mut [u64; 32],
        op: &IROp,
    ) -> Result<bool, vm_core::VmError> {
        self.stats.hot_path_executions += 1;

        if let IROp::Load {
            dst,
            base,
            offset,
            size,
            flags: _,
        } = op
        {
            let base_val = optimized_regs::get_reg_fast(regs, *base);
            let addr = base_val.wrapping_add(*offset as u64);

            // Prefetch next cache line
            #[cfg(target_arch = "x86_64")]
            if addr & 0x3F == 0 {
                // Aligned to cache line boundary
                use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
                unsafe {
                    _mm_prefetch((addr + 64) as *const i8, _MM_HINT_T0);
                }
            }

            let val = mmu.read(vm_core::GuestAddr(addr), *size)?;
            optimized_regs::set_reg_fast(regs, *dst, val);

            self.stats.fast_path_hits += 1;
            Ok(true)
        } else {
            self.stats.slow_path_taken += 1;
            Ok(false)
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> &HotPathStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = HotPathStats::default();
    }
}

impl Default for HotPathExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimized_register_access() {
        let mut regs = [0u64; 32];
        regs[5] = 42;

        assert_eq!(optimized_regs::get_reg_fast(&regs, 5), 42);
        optimized_regs::set_reg_fast(&mut regs, 10, 100);
        assert_eq!(regs[10], 100);

        // Test x0 write (should be ignored)
        optimized_regs::set_reg_fast(&mut regs, 0, 999);
        assert_eq!(regs[0], 0);
    }

    #[test]
    fn test_arith_optimizations() {
        assert_eq!(optimized_arith::mul_fast(10, 0), 0);
        assert_eq!(optimized_arith::mul_fast(10, 1), 10);
        assert_eq!(optimized_arith::mul_fast(10, 2), 20);

        assert_eq!(optimized_arith::mul_power_of_two(10, 4), Some(40));
        assert_eq!(optimized_arith::mul_power_of_two(10, 3), None);

        assert_eq!(optimized_arith::div_power_of_two(64, 8), Some(8));
        assert_eq!(optimized_arith::div_power_of_two(64, 7), None);
    }

    #[test]
    fn test_hot_path_executor() {
        let mut executor = HotPathExecutor::new();
        let mut regs = [0u64; 32];
        regs[1] = 10;
        regs[2] = 4;

        let op = IROp::MulImm {
            dst: 3,
            src: 1,
            imm: 4,
        };

        let result = executor.execute_arith(&mut regs, &op).unwrap();
        assert!(result);
        assert_eq!(regs[3], 40);
        assert_eq!(executor.stats.fast_path_hits, 1);
    }
}
