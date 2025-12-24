use vm_ir::{IRBlock, IROp, RegId, Terminator};
use vm_core::GuestAddr;
use crate::common::OptimizationStats;

pub struct SimdOptimizer {
    config: SimdOptimizerConfig,
    stats: OptimizationStats,
    reg_counter: RegId,
}

#[derive(Debug, Clone)]
pub struct SimdOptimizerConfig {
    pub enable_avx2: bool,
    pub enable_avx512: bool,
    pub enable_neon: bool,
    pub enable_sse: bool,
    pub min_vector_width: u8,
    pub max_unroll_factor: u8,
    pub enable_fma: bool,
    pub enable_masked_operations: bool,
}

impl Default for SimdOptimizerConfig {
    fn default() -> Self {
        Self {
            enable_avx2: true,
            enable_avx512: true,
            enable_neon: true,
            enable_sse: true,
            min_vector_width: 4,
            max_unroll_factor: 4,
            enable_fma: true,
            enable_masked_operations: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimdAnalysis {
    pub vectorizable_loops: Vec<LoopInfo>,
    pub reducible_patterns: Vec<ReductionPattern>,
    pub horizontal_ops: Vec<HorizontalOp>,
    pub maskable_ops: Vec<MaskableOp>,
}

#[derive(Debug, Clone)]
pub struct LoopInfo {
    pub start_idx: usize,
    pub end_idx: usize,
    pub induction_var: RegId,
    pub stride: i64,
    pub body_ops: Vec<IROp>,
    pub trip_count: Option<u64>,
    pub is_vectorizable: bool,
}

#[derive(Debug, Clone)]
pub struct ReductionPattern {
    pub reduction_var: RegId,
    pub accumulator: RegId,
    pub pattern_type: ReductionType,
    pub start_idx: usize,
    pub end_idx: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReductionType {
    Sum,
    Product,
    Min,
    Max,
    Dot,
}

#[derive(Debug, Clone)]
pub struct HorizontalOp {
    pub op: IROp,
    pub idx: usize,
    pub can_vectorize: bool,
}

#[derive(Debug, Clone)]
pub struct MaskableOp {
    pub op: IROp,
    pub idx: usize,
    pub condition_reg: RegId,
}

impl SimdOptimizer {
    pub fn new(config: SimdOptimizerConfig) -> Self {
        Self {
            config,
            stats: OptimizationStats::default(),
            reg_counter: 1000,
        }
    }

    pub fn optimize_block(&mut self, block: &IRBlock) -> IRBlock {
        let mut optimized_block = block.clone();
        
        if self.config.enable_avx2 || self.config.enable_avx512 || self.config.enable_neon {
            optimized_block = self.vectorize_scalar_ops(&optimized_block);
        }
        
        if self.config.enable_fma {
            optimized_block = self.optimize_fma_fusion(&optimized_block);
        }
        
        if self.config.enable_masked_operations && (self.config.enable_avx512 || self.config.enable_neon) {
            optimized_block = self.optimize_masked_ops(&optimized_block);
        }
        
        optimized_block = self.optimize_load_store(&optimized_block);
        optimized_block = self.optimize_horizontal_ops(&optimized_block);
        
        self.stats.blocks_optimized += 1;
        optimized_block
    }

    fn vectorize_scalar_ops(&mut self, block: &IRBlock) -> IRBlock {
        let mut ops = block.ops.clone();
        let mut i = 0;
        
        while i < ops.len() {
            if let Some((vector_ops, consume_count)) = self.try_vectorize_sequence(&ops[i..]) {
                let simd_count = vector_ops.len();
                ops.drain(i..i + consume_count);
                for (pos, op) in vector_ops.into_iter().enumerate() {
                    ops.insert(i + pos, op);
                }
                self.stats.ops_vectorized += consume_count as u64;
                self.stats.simd_ops_generated += simd_count as u64;
            }
            i += 1;
        }
        
        IRBlock {
            start_pc: block.start_pc,
            ops,
            term: block.term.clone(),
        }
    }

    fn try_vectorize_sequence(&mut self, ops: &[IROp]) -> Option<(Vec<IROp>, usize)> {
        if ops.len() < 4 {
            return None;
        }

        let pattern = self.detect_pattern(ops)?;
        
        match pattern {
            VectorizationPattern::SequentialLoadComputeStore { count } => {
                self.generate_simd_ops_for_pattern(ops, count)
            }
            VectorizationPattern::Reduction { op_type, count } => {
                self.generate_simd_reduction(ops, op_type, count)
            }
            VectorizationPattern::BroadcastLoadCompute { count } => {
                self.generate_broadcast_ops(ops, count)
            }
            VectorizationPattern::ElementWiseBinary { op, count } => {
                self.generate_element_wise_simd(ops, op, count)
            }
        }
    }

    fn detect_pattern(&self, ops: &[IROp]) -> Option<VectorizationPattern> {
        let first = &ops[0];
        
        match first {
            IROp::Load { .. } => {
                if ops.len() >= 4 {
                    if self.is_sequential_load_compute_store(ops) {
                        let count = ops.len().min(8);
                        return Some(VectorizationPattern::SequentialLoadComputeStore { count });
                    }
                }
            }
            IROp::MovImm { .. } => {
                if self.is_broadcast_load_compute(ops) {
                    let count = ops.len().min(8);
                    return Some(VectorizationPattern::BroadcastLoadCompute { count });
                }
            }
            _ => {
                if let Some(op) = self.is_element_wise_binary_pattern(ops) {
                    let count = ops.len().min(8);
                    return Some(VectorizationPattern::ElementWiseBinary { op, count });
                }
                
                if let Some(op_type) = self.is_reduction_pattern(ops) {
                    let count = ops.len();
                    return Some(VectorizationPattern::Reduction { op_type, count });
                }
            }
        }
        
        None
    }

    fn is_sequential_load_compute_store(&self, ops: &[IROp]) -> bool {
        if ops.len() < 4 {
            return false;
        }
        
        let mut has_load = false;
        let mut has_compute = false;
        let mut has_store = false;
        
        for op in ops.iter() {
            match op {
                IROp::Load { .. } => has_load = true,
                IROp::Add { .. } | IROp::Sub { .. } | IROp::Mul { .. } => has_compute = true,
                IROp::Store { .. } => has_store = true,
                _ => {}
            }
        }
        
        has_load && has_compute && has_store
    }

    fn is_broadcast_load_compute(&self, ops: &[IROp]) -> bool {
        if ops.len() < 4 {
            return false;
        }
        
        ops.iter().all(|op| {
            matches!(op, IROp::MovImm { .. } | IROp::Add { .. } | IROp::Mul { .. })
        })
    }

    fn is_element_wise_binary_pattern(&self, ops: &[IROp]) -> Option<IROp> {
        if ops.len() < 2 {
            return None;
        }
        
        let first_op = &ops[0];
        if !matches!(first_op, IROp::Add { .. } | IROp::Sub { .. } | IROp::Mul { .. } | IROp::And { .. } | IROp::Or { .. } | IROp::Xor { .. }) {
            return None;
        }
        
        let op_type = first_op.clone();
        
        for op in ops.iter() {
            if std::mem::discriminant(op) != std::mem::discriminant(&op_type) {
                return None;
            }
        }
        
        Some(op_type.clone())
    }

    fn is_reduction_pattern(&self, ops: &[IROp]) -> Option<ReductionType> {
        if ops.len() < 2 {
            return None;
        }
        
        let first_op = &ops[0];
        match first_op {
            IROp::Add { dst, src1, .. } => {
                let mut reduction_type = ReductionType::Sum;
                
                for op in ops.iter() {
                    if let IROp::Add { dst: d, src1: s1, .. } = op {
                        if d != src1 {
                            reduction_type = ReductionType::Sum;
                        }
                    } else {
                        return None;
                    }
                }
                
                Some(reduction_type)
            }
            IROp::Mul { .. } => Some(ReductionType::Product),
            IROp::Fmin { .. } => Some(ReductionType::Min),
            IROp::Fmax { .. } => Some(ReductionType::Max),
            _ => None,
        }
    }

    fn generate_simd_ops_for_pattern(&mut self, ops: &[IROp], count: usize) -> Option<(Vec<IROp>, usize)> {
        let mut simd_ops = Vec::new();
        let vector_width = self.determine_vector_width(count);
        
        let dst_vec = self.alloc_reg();
        let src1_vec = self.alloc_reg();
        let src2_vec = self.alloc_reg();
        
        simd_ops.push(IROp::MovImm { dst: dst_vec, imm: vector_width as u64 });
        
        for chunk in ops.chunks(vector_width) {
            for op in chunk {
                match op {
                    IROp::Load { dst, base, offset, size, flags } => {
                        simd_ops.push(IROp::Load {
                            dst: self.alloc_reg(),
                            base: *base,
                            offset: *offset,
                            size: *size * vector_width as u8,
                            flags: *flags,
                        });
                    }
                    IROp::Store { src, base, offset, size, flags } => {
                        simd_ops.push(IROp::Store {
                            src: self.alloc_reg(),
                            base: *base,
                            offset: *offset,
                            size: *size * vector_width as u8,
                            flags: *flags,
                        });
                    }
                    IROp::Add { dst, src1, src2 } => {
                        simd_ops.push(IROp::VecAdd {
                            dst: dst_vec,
                            src1: *src1,
                            src2: *src2,
                            element_size: 4,
                        });
                    }
                    IROp::Sub { dst, src1, src2 } => {
                        simd_ops.push(IROp::VecSub {
                            dst: dst_vec,
                            src1: *src1,
                            src2: *src2,
                            element_size: 4,
                        });
                    }
                    IROp::Mul { dst, src1, src2 } => {
                        simd_ops.push(IROp::VecMul {
                            dst: dst_vec,
                            src1: *src1,
                            src2: *src2,
                            element_size: 4,
                        });
                    }
                    _ => simd_ops.push(op.clone()),
                }
            }
        }
        
        Some((simd_ops, ops.len()))
    }

    fn generate_simd_reduction(&mut self, ops: &[IROp], op_type: ReductionType, count: usize) -> Option<(Vec<IROp>, usize)> {
        let mut simd_ops = Vec::new();
        let vector_width = self.determine_vector_width(count);
        
        let accumulator = self.alloc_reg();
        
        match op_type {
            ReductionType::Sum => {
                simd_ops.push(IROp::Vec128Add {
                    dst_lo: self.alloc_reg(),
                    dst_hi: self.alloc_reg(),
                    src1_lo: self.alloc_reg(),
                    src1_hi: self.alloc_reg(),
                    src2_lo: self.alloc_reg(),
                    src2_hi: self.alloc_reg(),
                    element_size: 4,
                    signed: false,
                });
            }
            ReductionType::Product => {
                simd_ops.push(IROp::Vec256Mul {
                    dst0: self.alloc_reg(),
                    dst1: self.alloc_reg(),
                    dst2: self.alloc_reg(),
                    dst3: self.alloc_reg(),
                    src10: self.alloc_reg(),
                    src11: self.alloc_reg(),
                    src12: self.alloc_reg(),
                    src13: self.alloc_reg(),
                    src20: self.alloc_reg(),
                    src21: self.alloc_reg(),
                    src22: self.alloc_reg(),
                    src23: self.alloc_reg(),
                    element_size: 4,
                    signed: false,
                });
            }
            _ => {
                for op in ops {
                    simd_ops.push(op.clone());
                }
            }
        }
        
        Some((simd_ops, ops.len()))
    }

    fn generate_broadcast_ops(&mut self, ops: &[IROp], count: usize) -> Option<(Vec<IROp>, usize)> {
        let mut simd_ops = Vec::new();
        
        if let Some(IROp::MovImm { imm, .. }) = ops.first() {
            let broadcast_reg = self.alloc_reg();
            simd_ops.push(IROp::MovImm { dst: broadcast_reg, imm: *imm });
            
            let vector_width = self.determine_vector_width(count);
            
            for i in 0..vector_width {
                simd_ops.push(IROp::MovImm {
                    dst: self.alloc_reg(),
                    imm: *imm,
                });
            }
        }
        
        Some((simd_ops, ops.len()))
    }

    fn generate_element_wise_simd(&mut self, ops: &[IROp], op: IROp, count: usize) -> Option<(Vec<IROp>, usize)> {
        let mut simd_ops = Vec::new();
        let vector_width = self.determine_vector_width(count);
        
        match op {
            IROp::Add { .. } => {
                simd_ops.push(IROp::VecAdd {
                    dst: self.alloc_reg(),
                    src1: self.alloc_reg(),
                    src2: self.alloc_reg(),
                    element_size: 4,
                });
            }
            IROp::Sub { .. } => {
                simd_ops.push(IROp::VecSub {
                    dst: self.alloc_reg(),
                    src1: self.alloc_reg(),
                    src2: self.alloc_reg(),
                    element_size: 4,
                });
            }
            IROp::Mul { .. } => {
                simd_ops.push(IROp::VecMul {
                    dst: self.alloc_reg(),
                    src1: self.alloc_reg(),
                    src2: self.alloc_reg(),
                    element_size: 4,
                });
            }
            _ => {
                for op in ops {
                    simd_ops.push(op.clone());
                }
            }
        }
        
        Some((simd_ops, ops.len()))
    }

    fn optimize_fma_fusion(&mut self, block: &IRBlock) -> IRBlock {
        let mut ops = block.ops.clone();
        let mut i = 0;
        
        while i + 2 < ops.len() {
            if let Some(fma_op) = self.try_fuse_fma(&ops[i..=i+2]) {
                ops[i] = fma_op;
                ops.remove(i + 1);
                ops.remove(i + 1);
                self.stats.fma_fusions += 1;
            } else {
                i += 1;
            }
        }
        
        IRBlock {
            start_pc: block.start_pc,
            ops,
            term: block.term.clone(),
        }
    }

    fn try_fuse_fma(&self, ops: &[IROp]) -> Option<IROp> {
        match (&ops[0], &ops[1], &ops[2]) {
            (IROp::Mul { dst: mul_dst, src1, src2 }, IROp::Add { dst: add_dst, src1: add_src1, src2: add_src2 }, _) => {
                if add_dst == mul_dst || add_src1 == mul_dst || add_src2 == mul_dst {
                    let acc = if add_src1 == mul_dst { add_src2 } else { add_src1 };
                    Some(IROp::Fmadd {
                        dst: *add_dst,
                        src1: *src1,
                        src2: *src2,
                        src3: *acc,
                    })
                } else {
                    None
                }
            }
            (IROp::Mul { dst: mul_dst, src1, src2 }, IROp::Sub { dst: sub_dst, src1: sub_src1, src2: sub_src2 }, _) => {
                if sub_dst == mul_dst || sub_src1 == mul_dst {
                    let acc = if sub_src1 == mul_dst { sub_src2 } else { sub_src1 };
                    Some(IROp::Fmsub {
                        dst: *sub_dst,
                        src1: *src1,
                        src2: *src2,
                        src3: *acc,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn optimize_masked_ops(&mut self, block: &IRBlock) -> IRBlock {
        let mut ops = block.ops.clone();
        let mut i = 0;
        
        while i + 1 < ops.len() {
            if let IROp::CmpEq { dst: cond, lhs, rhs } = &ops[i] {
                if self.can_mask_following_op(&ops[i + 1]) {
                    let masked_op = self.create_masked_op(&ops[i + 1], *cond);
                    ops[i + 1] = masked_op;
                    ops.remove(i);
                    self.stats.masked_ops += 1;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        
        IRBlock {
            start_pc: block.start_pc,
            ops,
            term: block.term.clone(),
        }
    }

    fn can_mask_following_op(&self, op: &IROp) -> bool {
        matches!(op, IROp::Load { .. } | IROp::Store { .. } | IROp::Add { .. } | IROp::Mul { .. })
    }

    fn create_masked_op(&self, op: &IROp, mask_reg: RegId) -> IROp {
        match op {
            IROp::Load { dst, base, offset, size, flags } => {
                IROp::VendorLoad {
                    dst: *dst,
                    base: *base,
                    offset: *offset,
                    vendor: "masked_load".to_string(),
                    tile_id: mask_reg as u8,
                }
            }
            IROp::Store { src, base, offset, size, flags } => {
                IROp::VendorStore {
                    src: *src,
                    base: *base,
                    offset: *offset,
                    vendor: "masked_store".to_string(),
                    tile_id: mask_reg as u8,
                }
            }
            _ => op.clone(),
        }
    }

    fn optimize_load_store(&mut self, block: &IRBlock) -> IRBlock {
        let mut ops = block.ops.clone();
        let mut i = 0;
        
        while i + 3 < ops.len() {
            if self.is_consecutive_load_pattern(&ops[i..=i+3]) {
                let vec_load = self.create_vector_load(&ops[i..=i+3]);
                ops[i] = vec_load;
                ops.remove(i + 1);
                ops.remove(i + 1);
                ops.remove(i + 1);
                self.stats.load_store_vectorized += 1;
            } else if self.is_consecutive_store_pattern(&ops[i..=i+3]) {
                let vec_store = self.create_vector_store(&ops[i..=i+3]);
                ops[i] = vec_store;
                ops.remove(i + 1);
                ops.remove(i + 1);
                ops.remove(i + 1);
                self.stats.load_store_vectorized += 1;
            } else {
                i += 1;
            }
        }
        
        IRBlock {
            start_pc: block.start_pc,
            ops,
            term: block.term.clone(),
        }
    }

    fn is_consecutive_load_pattern(&self, ops: &[IROp]) -> bool {
        if ops.len() != 4 {
            return false;
        }
        
        ops.iter().all(|op| matches!(op, IROp::Load { .. }))
    }

    fn is_consecutive_store_pattern(&self, ops: &[IROp]) -> bool {
        if ops.len() != 4 {
            return false;
        }
        
        ops.iter().all(|op| matches!(op, IROp::Store { .. }))
    }

    fn create_vector_load(&mut self, ops: &[IROp]) -> IROp {
        if let Some(IROp::Load { base, offset, size, flags, .. }) = ops.first() {
            IROp::Load {
                dst: self.alloc_reg(),
                base: *base,
                offset: *offset,
                size: *size * 4,
                flags: *flags,
            }
        } else {
            ops[0].clone()
        }
    }

    fn create_vector_store(&mut self, ops: &[IROp]) -> IROp {
        if let Some(IROp::Store { base, offset, size, flags, .. }) = ops.first() {
            IROp::Store {
                src: self.alloc_reg(),
                base: *base,
                offset: *offset,
                size: *size * 4,
                flags: *flags,
            }
        } else {
            ops[0].clone()
        }
    }

    fn optimize_horizontal_ops(&mut self, block: &IRBlock) -> IRBlock {
        let mut ops = block.ops.clone();
        let mut i = 0;
        
        while i + 3 < ops.len() {
            if self.is_horizontal_reduction(&ops[i..=i+3]) {
                let horizontal_op = self.create_horizontal_op(&ops[i..=i+3]);
                ops[i] = horizontal_op;
                ops.remove(i + 1);
                ops.remove(i + 1);
                ops.remove(i + 1);
                self.stats.horizontal_ops_optimized += 1;
            } else {
                i += 1;
            }
        }
        
        IRBlock {
            start_pc: block.start_pc,
            ops,
            term: block.term.clone(),
        }
    }

    fn is_horizontal_reduction(&self, ops: &[IROp]) -> bool {
        if ops.len() != 4 {
            return false;
        }
        
        let mut same_op = true;
        let op_disc = std::mem::discriminant(&ops[0]);
        
        for op in ops.iter() {
            if std::mem::discriminant(op) != op_disc {
                same_op = false;
                break;
            }
        }
        
        same_op && matches!(&ops[0], IROp::Add { .. } | IROp::Fadd { .. })
    }

    fn create_horizontal_op(&mut self, ops: &[IROp]) -> IROp {
        if let Some(IROp::Add { dst, src1, src2 }) = ops.first() {
            IROp::Vec128Add {
                dst_lo: *dst,
                dst_hi: self.alloc_reg(),
                src1_lo: *src1,
                src1_hi: self.alloc_reg(),
                src2_lo: *src2,
                src2_hi: self.alloc_reg(),
                element_size: 4,
                signed: false,
            }
        } else {
            ops[0].clone()
        }
    }

    fn determine_vector_width(&self, count: usize) -> usize {
        let min_width = self.config.min_vector_width as usize;
        
        if self.config.enable_avx512 && count >= 16 {
            16
        } else if self.config.enable_avx2 && count >= 8 {
            8
        } else if self.config.enable_neon && count >= 4 {
            4
        } else if self.config.enable_sse && count >= 4 {
            4
        } else {
            min_width.min(count)
        }
    }

    fn alloc_reg(&mut self) -> RegId {
        let reg = self.reg_counter;
        self.reg_counter += 1;
        reg
    }

    pub fn analyze_block(&mut self, block: &IRBlock) -> SimdAnalysis {
        let vectorizable_loops = Vec::new();
        let mut reducible_patterns = Vec::new();
        let horizontal_ops = Vec::new();
        let maskable_ops = Vec::new();
        
        let mut i = 0;
        while i < block.ops.len() {
            if let Some(pattern) = self.detect_pattern(&block.ops[i..]) {
                match pattern {
                    VectorizationPattern::Reduction { op_type, count } => {
                        reducible_patterns.push(ReductionPattern {
                            reduction_var: self.alloc_reg(),
                            accumulator: self.alloc_reg(),
                            pattern_type: op_type,
                            start_idx: i,
                            end_idx: i + count,
                        });
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        
        SimdAnalysis {
            vectorizable_loops,
            reducible_patterns,
            horizontal_ops,
            maskable_ops,
        }
    }

    pub fn get_stats(&self) -> &OptimizationStats {
        &self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = OptimizationStats::default();
    }
}

#[derive(Debug, Clone, PartialEq)]
enum VectorizationPattern {
    SequentialLoadComputeStore { count: usize },
    Reduction { op_type: ReductionType, count: usize },
    BroadcastLoadCompute { count: usize },
    ElementWiseBinary { op: IROp, count: usize },
}

pub struct DefaultSIMDOptimizer {
    inner: SimdOptimizer,
}

impl DefaultSIMDOptimizer {
    pub fn new() -> Self {
        let config = SimdOptimizerConfig::default();
        Self {
            inner: SimdOptimizer::new(config),
        }
    }

    pub fn optimize(&mut self, block: &IRBlock) -> IRBlock {
        self.inner.optimize_block(block)
    }

    pub fn analyze(&mut self, block: &IRBlock) -> SimdAnalysis {
        self.inner.analyze_block(block)
    }

    pub fn get_stats(&self) -> &OptimizationStats {
        self.inner.get_stats()
    }
}

impl Default for DefaultSIMDOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::GuestAddr;

    #[test]
    fn test_simd_optimizer_creation() {
        let config = SimdOptimizerConfig::default();
        let optimizer = SimdOptimizer::new(config);
        assert!(optimizer.config.enable_avx2);
    }

    #[test]
    fn test_empty_block_optimization() {
        let config = SimdOptimizerConfig::default();
        let mut optimizer = SimdOptimizer::new(config);
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![],
            term: vm_ir::Terminator::Ret,
        };
        let optimized = optimizer.optimize_block(&block);
        assert_eq!(optimized.ops.len(), 0);
    }

    #[test]
    fn test_vector_width_determination() {
        let config = SimdOptimizerConfig::default();
        let optimizer = SimdOptimizer::new(config);
        
        assert_eq!(optimizer.determine_vector_width(16), 16);
        assert_eq!(optimizer.determine_vector_width(8), 8);
        assert_eq!(optimizer.determine_vector_width(4), 4);
    }
}
