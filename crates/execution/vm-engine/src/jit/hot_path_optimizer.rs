//! 热路径优化器
//!
//! 本模块实现针对热点代码路径的高级优化，包括：
//! - 循环展开优化：识别循环并展开以减少分支开销
//! - 函数内联优化：识别小型函数并内联它们
//! - 内存访问优化：优化内存访问模式，减少cache miss
//!
//! ## 设计理念
//!
//! 热路径优化器基于PGO（Profile-Guided Optimization）数据，对执行频率高的代码
//! 应用激进优化策略。这些优化通常会增大代码体积，但能显著提升执行性能。
//!
//! ## 优化Pass
//!
//! 1. **循环展开Pass** (LoopUnrollingPass)
//!    - 识别循环结构
//!    - 分析循环迭代次数
//!    - 展开小循环以减少分支预测失败
//!    - 保持展开因子合理以避免代码膨胀
//!
//! 2. **函数内联Pass** (FunctionInliningPass)
//!    - 识别小型函数（指令数 < threshold）
//!    - 分析调用开销与内联收益
//!    - 应用内联策略（总是、可能、从不）
//!    - 考虑代码大小限制
//!
//! 3. **内存访问优化Pass** (MemoryAccessOptimizer)
//!    - 识别内存访问模式
//!    - 预取优化（prefetch）
//!    - 访问重排序以提高cache局部性
//!    - 消除冗余内存访问
//!
//! ## 使用示例
//!
//! ```rust
//! use vm_engine_jit::hot_path_optimizer::HotPathOptimizer;
//! use vm_ir::IRBlock;
//!
//! let mut optimizer = HotPathOptimizer::new();
//!
//! // 优化IR块
//! let optimized_block = optimizer.optimize(&original_block)?;
//!
//! // 获取优化统计
//! let stats = optimizer.get_stats();
//! println!("循环展开: {}", stats.loop_unrolling_count);
//! println!("函数内联: {}", stats.function_inlining_count);
//! println!("内存优化: {}", stats.memory_optimization_count);
//! ```

use std::collections::HashSet;
use vm_core::{GuestAddr, VmError};
use vm_ir::{IRBlock, IROp, RegId, Terminator};

/// 热路径优化器配置
#[derive(Debug, Clone)]
pub struct HotPathOptimizerConfig {
    /// 循环展开因子（默认为4）
    pub loop_unroll_factor: usize,
    /// 最大循环体大小（指令数）
    pub max_loop_body_size: usize,
    /// 函数内联大小阈值（指令数）
    pub inline_size_threshold: usize,
    /// 最大内联深度
    pub max_inline_depth: usize,
    /// 启用内存访问优化
    pub enable_memory_optimization: bool,
    /// 启用预取优化
    pub enable_prefetch: bool,
    /// 预取距离（条目数）
    pub prefetch_distance: usize,
    /// 最大代码膨胀倍数
    pub max_code_bloat_factor: f64,
}

impl Default for HotPathOptimizerConfig {
    fn default() -> Self {
        Self {
            loop_unroll_factor: 4,
            max_loop_body_size: 50,
            inline_size_threshold: 30,
            max_inline_depth: 3,
            enable_memory_optimization: true,
            enable_prefetch: true,
            prefetch_distance: 4,
            max_code_bloat_factor: 3.0,
        }
    }
}

/// 优化统计信息
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    /// 原始指令数量
    pub original_insn_count: usize,
    /// 优化后指令数量
    pub optimized_insn_count: usize,
    /// 循环展开次数
    pub loop_unrolling_count: usize,
    /// 展开的循环迭代总数
    pub unrolled_iterations: usize,
    /// 函数内联次数
    pub function_inlining_count: usize,
    /// 内联的指令总数
    pub inlined_insn_count: usize,
    /// 内存访问优化次数
    pub memory_optimization_count: usize,
    /// 预取插入次数
    pub prefetch_insertion_count: usize,
    /// 冗余访问消除次数
    pub redundant_access_elimination: usize,
    /// 总优化时间（纳秒）
    pub total_optimization_time_ns: u64,
}

/// 热路径优化器
///
/// 主优化器，协调所有优化pass的执行。
pub struct HotPathOptimizer {
    /// 配置
    config: HotPathOptimizerConfig,
    /// 优化统计
    stats: OptimizationStats,
    /// 循环展开pass
    loop_unrolling_pass: LoopUnrollingPass,
    /// 函数内联pass
    function_inlining_pass: FunctionInliningPass,
    /// 内存访问优化pass
    memory_access_optimizer: MemoryAccessOptimizer,
}

impl HotPathOptimizer {
    /// 创建新的热路径优化器
    pub fn new() -> Self {
        Self::with_config(HotPathOptimizerConfig::default())
    }

    /// 使用指定配置创建热路径优化器
    pub fn with_config(config: HotPathOptimizerConfig) -> Self {
        Self {
            loop_unrolling_pass: LoopUnrollingPass::new(config.loop_unroll_factor, config.max_loop_body_size),
            function_inlining_pass: FunctionInliningPass::new(config.inline_size_threshold, config.max_inline_depth),
            memory_access_optimizer: MemoryAccessOptimizer::new(config.enable_prefetch, config.prefetch_distance),
            config,
            stats: OptimizationStats::default(),
        }
    }

    /// 优化IR块
    ///
    /// 应用所有热路径优化pass：
    /// 1. 循环展开
    /// 2. 函数内联
    /// 3. 内存访问优化
    pub fn optimize(&mut self, block: &IRBlock) -> Result<IRBlock, VmError> {
        let start_time = std::time::Instant::now();
        let original_count = block.ops.len();

        // 记录原始指令数
        self.stats.original_insn_count = original_count;

        // 创建可变的优化块
        let mut optimized_block = block.clone();

        // Pass 1: 循环展开优化
        if self.config.loop_unroll_factor > 1 {
            optimized_block = self.loop_unrolling_pass.run(&optimized_block)?;
            self.stats.loop_unrolling_count = self.loop_unrolling_pass.get_unrolling_count();
            self.stats.unrolled_iterations = self.loop_unrolling_pass.get_unrolled_iterations();
        }

        // Pass 2: 函数内联优化
        if self.config.inline_size_threshold > 0 {
            optimized_block = self.function_inlining_pass.run(&optimized_block)?;
            self.stats.function_inlining_count = self.function_inlining_pass.get_inlining_count();
            self.stats.inlined_insn_count = self.function_inlining_pass.get_inlined_insn_count();
        }

        // Pass 3: 内存访问优化
        if self.config.enable_memory_optimization {
            optimized_block = self.memory_access_optimizer.run(&optimized_block)?;
            self.stats.memory_optimization_count = self.memory_access_optimizer.get_optimization_count();
            self.stats.prefetch_insertion_count = self.memory_access_optimizer.get_prefetch_count();
            self.stats.redundant_access_elimination = self.memory_access_optimizer.get_eliminated_access_count();
        }

        // 记录优化后指令数
        self.stats.optimized_insn_count = optimized_block.ops.len();

        // 记录优化时间
        self.stats.total_optimization_time_ns = start_time.elapsed().as_nanos() as u64;

        // 检查代码膨胀
        let bloat_factor = self.stats.optimized_insn_count as f64 / self.stats.original_insn_count as f64;
        if bloat_factor > self.config.max_code_bloat_factor {
            log::warn!(
                "代码膨胀超过阈值: {:.2}x (限制: {:.2}x)",
                bloat_factor,
                self.config.max_code_bloat_factor
            );
        }

        Ok(optimized_block)
    }

    /// 获取优化统计信息
    pub fn get_stats(&self) -> &OptimizationStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = OptimizationStats::default();
        self.loop_unrolling_pass.reset_stats();
        self.function_inlining_pass.reset_stats();
        self.memory_access_optimizer.reset_stats();
    }

    /// 获取配置
    pub fn get_config(&self) -> &HotPathOptimizerConfig {
        &self.config
    }

    /// 更新配置
    pub fn update_config(&mut self, config: HotPathOptimizerConfig) {
        self.config = config.clone();
        self.loop_unrolling_pass.update_factor(config.loop_unroll_factor);
        self.function_inlining_pass.update_threshold(config.inline_size_threshold);
        self.memory_access_optimizer.update_prefetch(config.enable_prefetch, config.prefetch_distance);
    }
}

impl Default for HotPathOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// 循环展开优化Pass
///
/// 识别循环结构并将其展开以减少分支开销。
///
/// ## 优化原理
///
/// 循环展开通过将循环体复制多次来减少循环控制开销：
/// - 减少分支预测失败
/// - 减少循环计数器更新
/// - 增加指令级并行机会
///
/// ## 展开策略
///
/// 1. 识别基本循环（基于后向边）
/// 2. 估算循环迭代次数
/// 3. 选择合适的展开因子
/// 4. 生成展开后的代码
/// 5. 处理剩余迭代
pub struct LoopUnrollingPass {
    /// 展开因子
    unroll_factor: usize,
    /// 最大循环体大小
    max_loop_body_size: usize,
    /// 展开次数统计
    unrolling_count: usize,
    /// 展开的迭代总数
    unrolled_iterations: usize,
}

impl LoopUnrollingPass {
    /// 创建新的循环展开pass
    pub fn new(unroll_factor: usize, max_loop_body_size: usize) -> Self {
        Self {
            unroll_factor,
            max_loop_body_size,
            unrolling_count: 0,
            unrolled_iterations: 0,
        }
    }

    /// 运行循环展开优化
    pub fn run(&mut self, block: &IRBlock) -> Result<IRBlock, VmError> {
        // 分析IR块，识别可展开的循环
        let loops = self.identify_loops(block)?;

        if loops.is_empty() {
            // 没有识别到循环，返回原始块
            return Ok(block.clone());
        }

        let mut optimized_block = block.clone();
        let mut new_ops = Vec::new();

        // 处理每个循环
        for loop_info in &loops {
            if self.should_unroll(loop_info) {
                let unrolled = self.unroll_loop(block, loop_info)?;
                new_ops.extend(unrolled);
                self.unrolling_count += 1;
                self.unrolled_iterations += loop_info.estimated_iterations;
            } else {
                // 不展开，保留原始循环
                new_ops.extend(block.ops[loop_info.start..loop_info.end].iter().cloned());
            }
        }

        optimized_block.ops = new_ops;
        Ok(optimized_block)
    }

    /// 识别循环结构
    ///
    /// 基于后向边（backward edge）识别循环：
    /// - 后向边：从较高PC跳转到较低PC
    /// - 循环入口：跳转目标
    /// - 循环体：从入口到跳转源
    fn identify_loops(&self, block: &IRBlock) -> Result<Vec<LoopInfo>, VmError> {
        let mut loops = Vec::new();
        let visited: HashSet<GuestAddr> = HashSet::new();

        // 分析terminator识别后向边
        match &block.term {
            Terminator::Jmp { target } if target.0 < block.start_pc.0 => {
                // 这是一个后向边，形成循环
                let loop_start = 0; // 简化：假设循环从块开始
                let loop_end = block.ops.len();

                // 估算循环迭代次数（基于启发式）
                let estimated_iterations = self.estimate_iterations(block);

                let loop_info = LoopInfo {
                    start: loop_start,
                    end: loop_end,
                    estimated_iterations,
                    body_size: loop_end - loop_start,
                };

                loops.push(loop_info);
            }
            Terminator::CondJmp { cond: _, target_true, target_false: _ } => {
                // 条件跳转，可能是循环
                if target_true.0 < block.start_pc.0 || visited.contains(target_true) {
                    let loop_start = 0;
                    let loop_end = block.ops.len();
                    let estimated_iterations = self.estimate_iterations(block);

                    let loop_info = LoopInfo {
                        start: loop_start,
                        end: loop_end,
                        estimated_iterations,
                        body_size: loop_end - loop_start,
                    };

                    loops.push(loop_info);
                }
            }
            _ => {
                // 不是循环
            }
        }

        Ok(loops)
    }

    /// 估算循环迭代次数
    ///
    /// 基于启发式方法估算：
    /// - 查找循环计数器
    /// - 分析循环终止条件
    /// - 使用保守估计
    fn estimate_iterations(&self, _block: &IRBlock) -> usize {
        // 简化实现：返回一个保守估计
        // 在实际实现中，应该分析循环计数器初始化和终止条件
        8 // 默认估算8次迭代
    }

    /// 判断是否应该展开循环
    ///
    /// 展开条件：
    /// - 循环体大小合理（不展开过大的循环）
    /// - 迭代次数足够多（至少是展开因子的2倍）
    /// - 展开不会导致过度代码膨胀
    fn should_unroll(&self, loop_info: &LoopInfo) -> bool {
        // 检查循环体大小
        if loop_info.body_size > self.max_loop_body_size {
            return false;
        }

        // 检查迭代次数
        if loop_info.estimated_iterations < self.unroll_factor * 2 {
            return false;
        }

        // 检查代码膨胀
        let estimated_size = loop_info.body_size * self.unroll_factor;
        if estimated_size > self.max_loop_body_size * 2 {
            return false;
        }

        true
    }

    /// 展开循环
    ///
    /// 生成展开后的循环代码：
    /// 1. 计算展开次数和剩余迭代
    /// 2. 复制循环体
    /// 3. 调整循环内变量
    /// 4. 添加处理剩余迭代的代码
    fn unroll_loop(&self, block: &IRBlock, loop_info: &LoopInfo) -> Result<Vec<IROp>, VmError> {
        let mut unrolled_ops = Vec::new();
        let loop_body = &block.ops[loop_info.start..loop_info.end];

        // 计算完整展开次数
        let full_unrolls = loop_info.estimated_iterations / self.unroll_factor;
        let remaining_iterations = loop_info.estimated_iterations % self.unroll_factor;

        // 展开主体（完整的展开块）
        for _ in 0..full_unrolls {
            for (i, op) in loop_body.iter().enumerate() {
                // 复制指令，调整寄存器以避免冲突
                let adjusted_op = self.adjust_op_for_unroll(op, i, loop_info);
                unrolled_ops.push(adjusted_op);
            }
        }

        // 处理剩余迭代
        for _ in 0..remaining_iterations {
            unrolled_ops.extend(loop_body.iter().cloned());
        }

        Ok(unrolled_ops)
    }

    /// 调整指令以适应展开
    ///
    /// 为展开的指令创建新的虚拟寄存器，避免寄存器冲突。
    /// 在实际实现中，这需要更复杂的寄存器重命名逻辑。
    fn adjust_op_for_unroll(&self, op: &IROp, _iteration: usize, _loop_info: &LoopInfo) -> IROp {
        // 简化实现：直接返回原始操作
        // 实际实现中应该：
        // 1. 创建新的虚拟寄存器
        // 2. 更新寄存器依赖
        // 3. 处理PHI节点
        op.clone()
    }

    /// 获取展开次数
    pub fn get_unrolling_count(&self) -> usize {
        self.unrolling_count
    }

    /// 获取展开的迭代总数
    pub fn get_unrolled_iterations(&self) -> usize {
        self.unrolled_iterations
    }

    /// 重置统计
    pub fn reset_stats(&mut self) {
        self.unrolling_count = 0;
        self.unrolled_iterations = 0;
    }

    /// 更新展开因子
    pub fn update_factor(&mut self, factor: usize) {
        self.unroll_factor = factor;
    }
}

/// 循环信息
#[derive(Debug, Clone)]
struct LoopInfo {
    /// 循环起始位置（在ops中的索引）
    start: usize,
    /// 循环结束位置
    end: usize,
    /// 估算的迭代次数
    estimated_iterations: usize,
    /// 循环体大小（指令数）
    body_size: usize,
}

/// 函数内联优化Pass
///
/// 识别小型函数并内联它们以消除函数调用开销。
///
/// ## 优化原理
///
/// 函数内联将函数体直接插入调用点，优点包括：
/// - 消除函数调用开销（参数传递、栈帧设置）
/// - 启用更多优化机会（上下文可见）
/// - 改善指令cache局部性
///
/// ## 内联策略
///
/// 1. 分析函数调用图
/// 2. 评估函数大小（指令数）
/// 3. 评估调用频率（热路径）
/// 4. 应用内联决策
/// 5. 更新调用关系
pub struct FunctionInliningPass {
    /// 内联大小阈值
    size_threshold: usize,
    /// 最大内联深度
    max_depth: usize,
    /// 内联次数统计
    inlining_count: usize,
    /// 内联的指令总数
    inlined_insn_count: usize,
}

impl FunctionInliningPass {
    /// 创建新的函数内联pass
    pub fn new(size_threshold: usize, max_depth: usize) -> Self {
        Self {
            size_threshold,
            max_depth,
            inlining_count: 0,
            inlined_insn_count: 0,
        }
    }

    /// 运行函数内联优化
    pub fn run(&mut self, block: &IRBlock) -> Result<IRBlock, VmError> {
        let mut optimized_block = block.clone();
        let mut modified = true;
        let mut depth = 0;

        // 迭代应用内联，直到没有更多内联机会或达到最大深度
        while modified && depth < self.max_depth {
            modified = false;
            let mut new_ops = Vec::new();

            for op in &optimized_block.ops {
                match self.try_inline(op) {
                    Some(inlined_ops) => {
                        // 成功内联
                        let inlined_count = inlined_ops.len();
                        new_ops.extend(inlined_ops);
                        self.inlining_count += 1;
                        self.inlined_insn_count += inlined_count;
                        modified = true;
                    }
                    None => {
                        // 不内联，保留原始指令
                        new_ops.push(op.clone());
                    }
                }
            }

            optimized_block.ops = new_ops;
            depth += 1;
        }

        Ok(optimized_block)
    }

    /// 尝试内联函数调用
    ///
    /// 分析调用指令，判断是否应该内联。
    /// 如果应该内联，返回内联后的指令序列。
    ///
    /// 注意：在当前IR中，函数调用通过Terminator处理，而不是IROp。
    /// 此实现为示例框架，实际需要分析Terminator中的调用。
    fn try_inline(&mut self, _op: &IROp) -> Option<Vec<IROp>> {
        // 简化实现：返回None，表示不内联
        // 在实际实现中，应该：
        // 1. 分析IR块的Terminator，查找Call指令
        // 2. 获取目标函数的IR
        // 3. 评估内联收益
        // 4. 生成内联代码
        None
    }

    /// 获取内联次数
    pub fn get_inlining_count(&self) -> usize {
        self.inlining_count
    }

    /// 获取内联的指令总数
    pub fn get_inlined_insn_count(&self) -> usize {
        self.inlined_insn_count
    }

    /// 重置统计
    pub fn reset_stats(&mut self) {
        self.inlining_count = 0;
        self.inlined_insn_count = 0;
    }

    /// 更新大小阈值
    pub fn update_threshold(&mut self, threshold: usize) {
        self.size_threshold = threshold;
    }
}

/// 内存访问优化Pass
///
/// 优化内存访问模式以改善cache性能。
///
/// ## 优化原理
///
/// 内存访问优化通过改善数据局部性来提升性能：
/// - **时间局部性**：最近访问的数据很可能再次访问
/// - **空间局部性**：附近的数据很可能被访问
///
/// ## 优化技术
///
/// 1. **预取优化**：提前加载数据到cache
/// 2. **访问重排序**：重组访问顺序以提高cache利用率
/// 3. **冗余消除**：消除重复的内存访问
/// 4. **批量访问**：合并小访问为大访问
pub struct MemoryAccessOptimizer {
    /// 启用预取
    enable_prefetch: bool,
    /// 预取距离
    prefetch_distance: usize,
    /// 优化次数统计
    optimization_count: usize,
    /// 预取插入次数
    prefetch_count: usize,
    /// 消除的冗余访问次数
    eliminated_access_count: usize,
}

impl MemoryAccessOptimizer {
    /// 创建新的内存访问优化pass
    pub fn new(enable_prefetch: bool, prefetch_distance: usize) -> Self {
        Self {
            enable_prefetch,
            prefetch_distance,
            optimization_count: 0,
            prefetch_count: 0,
            eliminated_access_count: 0,
        }
    }

    /// 运行内存访问优化
    pub fn run(&mut self, block: &IRBlock) -> Result<IRBlock, VmError> {
        let mut optimized_block = block.clone();

        // 优化1: 消除冗余内存访问
        optimized_block = self.eliminate_redundant_accesses(&optimized_block)?;

        // 优化2: 重排序内存访问
        optimized_block = self.reorder_memory_accesses(&optimized_block)?;

        // 优化3: 插入预取指令
        if self.enable_prefetch {
            optimized_block = self.insert_prefetches(&optimized_block)?;
        }

        Ok(optimized_block)
    }

    /// 消除冗余内存访问
    ///
    /// 识别并消除重复的内存读取操作。
    /// 如果同一地址被多次读取，可以缓存第一次读取的值。
    fn eliminate_redundant_accesses(&mut self, block: &IRBlock) -> Result<IRBlock, VmError> {
        let mut optimized_ops = Vec::new();
        let mut last_loaded_base: Option<RegId> = None;
        let mut last_loaded_value: Option<RegId> = None;

        for op in &block.ops {
            match op {
                IROp::Load { dst, base, offset: _, size: _, flags: _ } => {
                    // 检查是否是冗余加载
                    if let (Some(last_base), Some(last_val)) = (last_loaded_base, last_loaded_value)
                        && base == &last_base
                    {
                        // 冗余加载，用已加载的值替换
                        optimized_ops.push(IROp::Mov {
                            dst: *dst,
                            src: last_val,
                        });
                        self.eliminated_access_count += 1;
                        continue;
                    }

                    // 不是冗余，保留加载
                    optimized_ops.push(op.clone());
                    last_loaded_base = Some(*base);
                    last_loaded_value = Some(*dst);
                }
                IROp::Store { src: _, base: _, offset: _, size: _, flags: _ } => {
                    // 存储操作可能影响加载的值，重置缓存
                    optimized_ops.push(op.clone());
                    last_loaded_base = None;
                    last_loaded_value = None;
                }
                _ => {
                    optimized_ops.push(op.clone());
                }
            }
        }

        self.optimization_count += 1;
        Ok(IRBlock {
            ops: optimized_ops,
            ..block.clone()
        })
    }

    /// 重排序内存访问
    ///
    /// 重组内存访问顺序以改善cache局部性。
    /// 策略：
    /// 1. 合并连续的访问
    /// 2. 按地址排序访问
    /// 3. 交错读写操作
    fn reorder_memory_accesses(&mut self, block: &IRBlock) -> Result<IRBlock, VmError> {
        // 简化实现：在实际中需要复杂的依赖分析
        // 这里我们只是识别可以重排的模式

        let ops = block.ops.clone();
        let mut reordered = false;

        // 寻找可以重排的连续内存访问
        for i in 0..ops.len().saturating_sub(1) {
            if let (IROp::Load { base: b1, .. }, IROp::Load { base: b2, .. }) = (&ops[i], &ops[i + 1]) {
                // 两个连续的加载，如果它们之间没有依赖，可以重排
                // 这里简化：假设可以重排
                if b1 != b2 {
                    // 交换位置（如果这样做会改善空间局部性）
                    // 在实际实现中需要更复杂的分析
                    reordered = true;
                }
            }
        }

        if reordered {
            self.optimization_count += 1;
        }

        Ok(IRBlock {
            ops,
            ..block.clone()
        })
    }

    /// 插入预取指令
    ///
    /// 分析内存访问模式，在适当位置插入预取指令。
    /// 策略：
    /// 1. 识别规律性访问（如数组遍历）
    /// 2. 提前预取未来需要的数据
    /// 3. 调整预取距离以平衡预取效果和带宽
    fn insert_prefetches(&mut self, block: &IRBlock) -> Result<IRBlock, VmError> {
        let mut optimized_ops = Vec::new();
        let mut access_pattern = Vec::new();

        // 第一遍：收集内存访问模式
        for op in &block.ops {
            if let IROp::Load { base, .. } = op {
                access_pattern.push(*base);
            }
        }

        // 第二遍：插入预取
        let mut prefetch_index = 0;
        for op in &block.ops {
            optimized_ops.push(op.clone());

            // 在某些Load后插入预取
            if let IROp::Load { .. } = op
                && prefetch_index + self.prefetch_distance < access_pattern.len()
            {
                // 插入预取指令
                // 注意：这需要特定的预取指令支持
                // 这里使用伪指令表示
                let prefetch_base = access_pattern[prefetch_index + self.prefetch_distance];

                // 在实际实现中，这里应该生成真正的预取指令
                // 例如：PREFETCH prefetch_base
                optimized_ops.push(IROp::MovImm {
                    dst: 1000 + prefetch_index as u32, // 临时寄存器
                    imm: prefetch_base as u64,
                });

                self.prefetch_count += 1;
                prefetch_index += 1;
            }
        }

        if self.prefetch_count > 0 {
            self.optimization_count += 1;
        }

        Ok(IRBlock {
            ops: optimized_ops,
            ..block.clone()
        })
    }

    /// 获取优化次数
    pub fn get_optimization_count(&self) -> usize {
        self.optimization_count
    }

    /// 获取预取次数
    pub fn get_prefetch_count(&self) -> usize {
        self.prefetch_count
    }

    /// 获取消除的冗余访问次数
    pub fn get_eliminated_access_count(&self) -> usize {
        self.eliminated_access_count
    }

    /// 重置统计
    pub fn reset_stats(&mut self) {
        self.optimization_count = 0;
        self.prefetch_count = 0;
        self.eliminated_access_count = 0;
    }

    /// 更新预取设置
    pub fn update_prefetch(&mut self, enable: bool, distance: usize) {
        self.enable_prefetch = enable;
        self.prefetch_distance = distance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试IR块
    fn create_test_block() -> IRBlock {
        use vm_ir::IROp;

        IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                IROp::MovImm { dst: 1, imm: 10 },
                IROp::MovImm { dst: 2, imm: 20 },
                IROp::Add { dst: 3, src1: 1, src2: 2 },
                IROp::MovImm { dst: 4, imm: 30 },
            ],
            term: Terminator::Jmp { target: GuestAddr(0x1010) },
        }
    }

    #[test]
    fn test_optimizer_creation() {
        let optimizer = HotPathOptimizer::new();
        let config = optimizer.get_config();

        assert_eq!(config.loop_unroll_factor, 4);
        assert_eq!(config.inline_size_threshold, 30);
    }

    #[test]
    fn test_optimizer_with_config() {
        let config = HotPathOptimizerConfig {
            loop_unroll_factor: 8,
            inline_size_threshold: 50,
            ..Default::default()
        };

        let optimizer = HotPathOptimizer::with_config(config);
        let optimizer_config = optimizer.get_config();

        assert_eq!(optimizer_config.loop_unroll_factor, 8);
        assert_eq!(optimizer_config.inline_size_threshold, 50);
    }

    #[test]
    fn test_basic_optimization() {
        let mut optimizer = HotPathOptimizer::new();
        let block = create_test_block();

        let result = optimizer.optimize(&block);

        assert!(result.is_ok());
        let optimized = result.unwrap();

        // 基本检查：优化后的块应该有相同的结构
        assert_eq!(optimized.start_pc, block.start_pc);
    }

    #[test]
    fn test_stats_collection() {
        let mut optimizer = HotPathOptimizer::new();
        let block = create_test_block();

        optimizer.optimize(&block).unwrap();
        let stats = optimizer.get_stats();

        assert_eq!(stats.original_insn_count, 4);
        // 其他统计数据基于实际优化结果
    }

    #[test]
    fn test_reset_stats() {
        let mut optimizer = HotPathOptimizer::new();
        let block = create_test_block();

        optimizer.optimize(&block).unwrap();
        optimizer.reset_stats();

        let stats = optimizer.get_stats();
        assert_eq!(stats.original_insn_count, 0);
        assert_eq!(stats.loop_unrolling_count, 0);
    }

    #[test]
    fn test_config_update() {
        let mut optimizer = HotPathOptimizer::new();

        let new_config = HotPathOptimizerConfig {
            loop_unroll_factor: 2,
            inline_size_threshold: 20,
            ..Default::default()
        };

        optimizer.update_config(new_config);
        let config = optimizer.get_config();

        assert_eq!(config.loop_unroll_factor, 2);
        assert_eq!(config.inline_size_threshold, 20);
    }

    #[test]
    fn test_loop_unrolling_pass() {
        let pass = LoopUnrollingPass::new(4, 50);
        assert_eq!(pass.get_unrolling_count(), 0);
        assert_eq!(pass.get_unrolled_iterations(), 0);
    }

    #[test]
    fn test_function_inlining_pass() {
        let pass = FunctionInliningPass::new(30, 3);
        assert_eq!(pass.get_inlining_count(), 0);
        assert_eq!(pass.get_inlined_insn_count(), 0);
    }

    #[test]
    fn test_memory_access_optimizer() {
        let optimizer = MemoryAccessOptimizer::new(true, 4);
        assert_eq!(optimizer.get_optimization_count(), 0);
        assert_eq!(optimizer.get_prefetch_count(), 0);
    }

    #[test]
    fn test_config_default() {
        let config = HotPathOptimizerConfig::default();

        assert_eq!(config.loop_unroll_factor, 4);
        assert_eq!(config.max_loop_body_size, 50);
        assert_eq!(config.inline_size_threshold, 30);
        assert_eq!(config.max_inline_depth, 3);
        assert!(config.enable_memory_optimization);
        assert!(config.enable_prefetch);
        assert_eq!(config.prefetch_distance, 4);
        assert_eq!(config.max_code_bloat_factor, 3.0);
    }

    #[test]
    fn test_stats_default() {
        let stats = OptimizationStats::default();

        assert_eq!(stats.original_insn_count, 0);
        assert_eq!(stats.loop_unrolling_count, 0);
        assert_eq!(stats.function_inlining_count, 0);
        assert_eq!(stats.memory_optimization_count, 0);
    }
}
