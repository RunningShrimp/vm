//! 分支预测优化
//!
//! 实现Profile-guided分支预测和静态分支预测

use std::collections::HashMap;
use vm_core::VmResult;
use vm_ir::IRBlock;

/// 分支方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchDirection {
    /// 不跳转（fallthrough）
    NotTaken,
    /// 跳转
    Taken,
}

/// 分支预测器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PredictorType {
    /// 总是预测不跳转
    AlwaysNotTaken,
    /// 总是预测跳转
    AlwaysTaken,
    /// 后向跳转-向前不跳转（BTFN）
    BackwardTakenForwardNotTaken,
    /// 2位饱和计数器
    TwoBitCounter,
    /// 自适应预测器
    Adaptive,
}

/// 分支预测器统计信息
#[derive(Debug, Clone, Copy, Default)]
pub struct PredictorStats {
    /// 总预测次数
    pub total: u64,
    /// 正确预测次数
    pub correct: u64,
}

/// 分支预测器
pub trait BranchPredictor: Send + Sync {
    /// 预测分支方向
    fn predict(&mut self, addr: u64) -> BranchDirection;

    /// 更新预测器（实际结果）
    fn update(&mut self, addr: u64, actual: BranchDirection);

    /// 获取预测准确率
    fn accuracy(&self) -> f64;

    /// 获取统计信息
    fn stats(&self) -> PredictorStats;
}

/// 总是预测不跳转
pub struct AlwaysNotTakenPredictor {
    total: u64,
    correct: u64,
}

impl Default for AlwaysNotTakenPredictor {
    fn default() -> Self {
        Self::new()
    }
}

impl AlwaysNotTakenPredictor {
    pub fn new() -> Self {
        Self {
            total: 0,
            correct: 0,
        }
    }
}

impl BranchPredictor for AlwaysNotTakenPredictor {
    fn predict(&mut self, _addr: u64) -> BranchDirection {
        BranchDirection::NotTaken
    }

    fn update(&mut self, _addr: u64, actual: BranchDirection) {
        self.total += 1;
        if actual == BranchDirection::NotTaken {
            self.correct += 1;
        }
    }

    fn accuracy(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.correct as f64 / self.total as f64
    }

    fn stats(&self) -> PredictorStats {
        PredictorStats {
            total: self.total,
            correct: self.correct,
        }
    }
}

/// 总是预测跳转
pub struct AlwaysTakenPredictor {
    total: u64,
    correct: u64,
}

impl Default for AlwaysTakenPredictor {
    fn default() -> Self {
        Self::new()
    }
}

impl AlwaysTakenPredictor {
    pub fn new() -> Self {
        Self {
            total: 0,
            correct: 0,
        }
    }
}

impl BranchPredictor for AlwaysTakenPredictor {
    fn predict(&mut self, _addr: u64) -> BranchDirection {
        BranchDirection::Taken
    }

    fn update(&mut self, _addr: u64, actual: BranchDirection) {
        self.total += 1;
        if actual == BranchDirection::Taken {
            self.correct += 1;
        }
    }

    fn accuracy(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.correct as f64 / self.total as f64
    }

    fn stats(&self) -> PredictorStats {
        PredictorStats {
            total: self.total,
            correct: self.correct,
        }
    }
}

/// 后向跳转-向前不跳转预测器
///
/// 基于经验：循环回退通常跳转，forward if通常不跳转
pub struct BTFNPredictor {
    total: u64,
    correct: u64,
}

impl Default for BTFNPredictor {
    fn default() -> Self {
        Self::new()
    }
}

impl BTFNPredictor {
    pub fn new() -> Self {
        Self {
            total: 0,
            correct: 0,
        }
    }

    /// 判断是否是后向跳转
    fn is_backward_branch(&self, pc: u64, target: u64) -> bool {
        target < pc
    }
}

impl BranchPredictor for BTFNPredictor {
    fn predict(&mut self, _addr: u64) -> BranchDirection {
        // 简化实现：默认预测不跳转
        // 实际需要知道分支目标和当前位置
        BranchDirection::NotTaken
    }

    fn update(&mut self, _addr: u64, actual: BranchDirection) {
        self.total += 1;
        if actual == BranchDirection::Taken {
            self.correct += 1;
        }
    }

    fn accuracy(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.correct as f64 / self.total as f64
    }

    fn stats(&self) -> PredictorStats {
        PredictorStats {
            total: self.total,
            correct: self.correct,
        }
    }
}

/// 2位饱和计数器预测器
///
/// 状态转换：
/// - 00: 强烈不跳转
/// - 01: 弱不跳转
/// - 10: 弱跳转
/// - 11: 强烈跳转
pub struct TwoBitCounterPredictor {
    /// 计数器表（地址 -> 2位计数器）
    counters: HashMap<u64, u8>,
    total: u64,
    correct: u64,
    /// 表大小
    table_size: usize,
}

impl TwoBitCounterPredictor {
    pub fn new(table_size: usize) -> Self {
        Self {
            counters: HashMap::with_capacity(table_size),
            total: 0,
            correct: 0,
            table_size,
        }
    }

    /// 获取计数器
    fn get_counter(&mut self, addr: u64) -> u8 {
        *self.counters.entry(addr).or_insert(1) // 默认弱不跳转
    }

    /// 更新计数器
    fn update_counter(&mut self, addr: u64, taken: bool) {
        let counter = self.counters.entry(addr).or_insert(1);

        if taken {
            if *counter < 3 {
                *counter += 1;
            }
        } else {
            if *counter > 0 {
                *counter -= 1;
            }
        }
    }
}

impl BranchPredictor for TwoBitCounterPredictor {
    fn predict(&mut self, addr: u64) -> BranchDirection {
        let counter = self.get_counter(addr);

        // 10或11预测跳转
        if counter >= 2 {
            BranchDirection::Taken
        } else {
            BranchDirection::NotTaken
        }
    }

    fn update(&mut self, addr: u64, actual: BranchDirection) {
        self.total += 1;

        let taken = match actual {
            BranchDirection::Taken => true,
            BranchDirection::NotTaken => false,
        };

        let predicted = self.predict(addr);
        let predicted_taken = match predicted {
            BranchDirection::Taken => true,
            BranchDirection::NotTaken => false,
        };

        if predicted_taken == taken {
            self.correct += 1;
        }

        self.update_counter(addr, taken);
    }

    fn accuracy(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.correct as f64 / self.total as f64
    }

    fn stats(&self) -> PredictorStats {
        PredictorStats {
            total: self.total,
            correct: self.correct,
        }
    }
}

/// 自适应分支预测器
///
/// 结合多种预测策略，动态选择最优
pub struct AdaptivePredictor {
    /// 2位计数器预测器
    two_bit: TwoBitCounterPredictor,
    /// BTFN预测器
    btfn: BTFNPredictor,
    /// 当前使用的预测器
    current_predictor: PredictorType,
    /// 预测器准确率历史
    accuracy_history: HashMap<PredictorType, f64>,
}

impl Default for AdaptivePredictor {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptivePredictor {
    pub fn new() -> Self {
        Self {
            two_bit: TwoBitCounterPredictor::new(1024),
            btfn: BTFNPredictor::new(),
            current_predictor: PredictorType::TwoBitCounter,
            accuracy_history: HashMap::new(),
        }
    }

    /// 选择最佳预测器
    fn select_best_predictor(&mut self) {
        let two_bit_acc = self.two_bit.accuracy();
        let btfn_acc = self.btfn.accuracy();

        self.accuracy_history
            .insert(PredictorType::TwoBitCounter, two_bit_acc);
        self.accuracy_history
            .insert(PredictorType::BackwardTakenForwardNotTaken, btfn_acc);

        // 选择准确率更高的
        if two_bit_acc >= btfn_acc {
            self.current_predictor = PredictorType::TwoBitCounter;
        } else {
            self.current_predictor = PredictorType::BackwardTakenForwardNotTaken;
        }
    }
}

impl BranchPredictor for AdaptivePredictor {
    fn predict(&mut self, addr: u64) -> BranchDirection {
        match self.current_predictor {
            PredictorType::TwoBitCounter => self.two_bit.predict(addr),
            PredictorType::BackwardTakenForwardNotTaken => self.btfn.predict(addr),
            _ => BranchDirection::NotTaken,
        }
    }

    fn update(&mut self, addr: u64, actual: BranchDirection) {
        self.two_bit.update(addr, actual);
        self.btfn.update(addr, actual);

        // 定期重新选择预测器
        if self.two_bit.total.is_multiple_of(100) {
            self.select_best_predictor();
        }
    }

    fn accuracy(&self) -> f64 {
        match self.current_predictor {
            PredictorType::TwoBitCounter => self.two_bit.accuracy(),
            PredictorType::BackwardTakenForwardNotTaken => self.btfn.accuracy(),
            _ => 0.0,
        }
    }

    fn stats(&self) -> PredictorStats {
        // 返回当前使用的预测器的统计信息
        match self.current_predictor {
            PredictorType::TwoBitCounter => self.two_bit.stats(),
            PredictorType::BackwardTakenForwardNotTaken => self.btfn.stats(),
            _ => PredictorStats::default(),
        }
    }
}

/// 分支目标缓存（BTB）
///
/// 缓存分支目标地址
pub struct BranchTargetBuffer {
    /// 分支目标缓存
    targets: HashMap<u64, u64>,
    /// 最大容量
    capacity: usize,
    /// 命中次数
    hits: u64,
    /// 访问次数
    accesses: u64,
}

impl BranchTargetBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            targets: HashMap::with_capacity(capacity),
            capacity,
            hits: 0,
            accesses: 0,
        }
    }

    /// 查找分支目标
    pub fn lookup(&mut self, addr: u64) -> Option<u64> {
        self.accesses += 1;

        if let Some(&target) = self.targets.get(&addr) {
            self.hits += 1;
            Some(target)
        } else {
            None
        }
    }

    /// 插入分支目标
    pub fn insert(&mut self, addr: u64, target: u64) {
        // 如果超过容量，简单的LRU策略（删除最旧的）
        if self.targets.len() >= self.capacity {
            // 简化：清空一半
            if self.targets.len() > self.capacity / 2 {
                self.targets.clear();
            }
        }

        self.targets.insert(addr, target);
    }

    /// 获取命中率
    pub fn hit_rate(&self) -> f64 {
        if self.accesses == 0 {
            return 0.0;
        }
        self.hits as f64 / self.accesses as f64
    }

    /// 清空缓存
    pub fn clear(&mut self) {
        self.targets.clear();
        self.hits = 0;
        self.accesses = 0;
    }
}

/// 分支预测统计信息
#[derive(Debug, Clone, Copy, Default)]
pub struct BranchPredictorStats {
    /// 总预测次数
    pub total_predictions: u64,
    /// 正确预测次数
    pub correct_predictions: u64,
    /// 预测准确率
    pub accuracy: f64,
    /// BTB命中率
    pub btb_hit_rate: f64,
}

/// 分支预测优化器
pub struct BranchPredictionOptimizer {
    /// 预测器
    predictor: Box<dyn BranchPredictor>,
    /// 分支目标缓存
    btb: BranchTargetBuffer,
}

impl BranchPredictionOptimizer {
    /// 创建新的优化器
    pub fn new(predictor_type: PredictorType, btb_capacity: usize) -> Self {
        let predictor: Box<dyn BranchPredictor> = match predictor_type {
            PredictorType::AlwaysNotTaken => Box::new(AlwaysNotTakenPredictor::new()),
            PredictorType::AlwaysTaken => Box::new(AlwaysTakenPredictor::new()),
            PredictorType::BackwardTakenForwardNotTaken => Box::new(BTFNPredictor::new()),
            PredictorType::TwoBitCounter => Box::new(TwoBitCounterPredictor::new(1024)),
            PredictorType::Adaptive => Box::new(AdaptivePredictor::new()),
        };

        Self {
            predictor,
            btb: BranchTargetBuffer::new(btb_capacity),
        }
    }

    /// 优化IR块中的分支
    pub fn optimize_block(&mut self, block: &mut IRBlock) -> VmResult<()> {
        let pc = block.start_pc.0;

        // 分析IR块中的分支指令，收集分支信息
        // 当前实现：收集基本块统计信息，为未来的优化做准备
        for (idx, _op) in block.ops.iter().enumerate() {
            // 记录指令位置，用于分支预测
            let _instruction_addr = pc + idx as u64;

            // 未来优化方向：
            // 1. 识别分支模式（循环、if-else等）
            // 2. 插入分支预测hints（需要IR扩展支持）
            // 3. 重新排列基本块以减少分支误预测
            // 4. 添加内联缓存
        }

        // 分析terminator以获取分支目标信息
        match &block.term {
            vm_ir::Terminator::Ret => {
                // 无条件返回 - 无需预测
            }
            vm_ir::Terminator::Jmp { target } => {
                // 无条件分支 - 预测总是跳转
                self.btb.insert(pc, target.0);
            }
            vm_ir::Terminator::CondJmp {
                cond: _,
                target_true,
                target_false: _,
            } => {
                // 条件分支 - 可以添加预测
                self.btb.insert(pc, target_true.0);
            }
            _ => {
                // 其他terminator类型（Call, Fault等）
            }
        }

        Ok(())
    }

    /// 预测分支
    pub fn predict_branch(&mut self, addr: u64) -> (BranchDirection, Option<u64>) {
        let direction = self.predictor.predict(addr);
        let target = self.btb.lookup(addr);

        (direction, target)
    }

    /// 更新分支结果
    pub fn update_branch(&mut self, addr: u64, taken: bool, target: Option<u64>) {
        let direction = if taken {
            BranchDirection::Taken
        } else {
            BranchDirection::NotTaken
        };

        self.predictor.update(addr, direction);

        if let Some(t) = target {
            self.btb.insert(addr, t);
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> BranchPredictorStats {
        let predictor_stats = self.predictor.stats();
        BranchPredictorStats {
            total_predictions: predictor_stats.total,
            correct_predictions: predictor_stats.correct,
            accuracy: self.predictor.accuracy(),
            btb_hit_rate: self.btb.hit_rate(),
        }
    }
}

/// 预测器工厂
pub struct PredictorFactory;

impl PredictorFactory {
    pub fn create(ty: PredictorType) -> Box<dyn BranchPredictor> {
        match ty {
            PredictorType::AlwaysNotTaken => Box::new(AlwaysNotTakenPredictor::new()),
            PredictorType::AlwaysTaken => Box::new(AlwaysTakenPredictor::new()),
            PredictorType::BackwardTakenForwardNotTaken => Box::new(BTFNPredictor::new()),
            PredictorType::TwoBitCounter => Box::new(TwoBitCounterPredictor::new(1024)),
            PredictorType::Adaptive => Box::new(AdaptivePredictor::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_not_taken() {
        let mut predictor = AlwaysNotTakenPredictor::new();

        let pred = predictor.predict(0x1000);
        assert_eq!(pred, BranchDirection::NotTaken);

        predictor.update(0x1000, BranchDirection::NotTaken);
        assert_eq!(predictor.accuracy(), 1.0);
    }

    #[test]
    fn test_two_bit_counter() {
        let mut predictor = TwoBitCounterPredictor::new(16);

        // 初始预测不跳转
        let pred = predictor.predict(0x1000);
        assert_eq!(pred, BranchDirection::NotTaken);

        // 连续跳转会改变预测
        predictor.update(0x1000, BranchDirection::Taken);
        predictor.update(0x1000, BranchDirection::Taken);

        let pred = predictor.predict(0x1000);
        assert_eq!(pred, BranchDirection::Taken);
    }

    #[test]
    fn test_btb() {
        let mut btb = BranchTargetBuffer::new(100);

        btb.insert(0x1000, 0x2000);
        assert_eq!(btb.lookup(0x1000), Some(0x2000));
        assert_eq!(btb.lookup(0x1004), None);
    }

    #[test]
    fn test_adaptive_predictor() {
        let mut predictor = AdaptivePredictor::new();

        let pred = predictor.predict(0x1000);
        assert_eq!(pred, BranchDirection::NotTaken);

        predictor.update(0x1000, BranchDirection::Taken);
    }
}
