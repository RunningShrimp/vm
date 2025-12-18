//! 编译时间预测器
//!
//! 实现了基于机器学习的编译时间预测，帮助优化编译决策。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use vm_ir::IRBlock;

/// 预测模型类型
#[derive(Debug, Clone, PartialEq)]
pub enum PredictionModel {
    /// 线性回归模型
    LinearRegression,
    /// 决策树模型
    DecisionTree,
    /// 神经网络模型
    NeuralNetwork,
    /// 集成模型
    Ensemble,
}

/// 编译时间预测配置
#[derive(Debug, Clone)]
pub struct CompilationPredictorConfig {
    /// 启用预测
    pub enabled: bool,
    /// 预测模型类型
    pub model_type: PredictionModel,
    /// 训练数据窗口大小
    pub training_window_size: usize,
    /// 最小训练样本数
    pub min_training_samples: usize,
    /// 预测精度阈值
    pub accuracy_threshold: f64,
    /// 启用在线学习
    pub enable_online_learning: bool,
    /// 模型更新间隔
    pub model_update_interval: usize,
}

impl Default for CompilationPredictorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model_type: PredictionModel::LinearRegression,
            training_window_size: 1000,
            min_training_samples: 50,
            accuracy_threshold: 0.8,
            enable_online_learning: true,
            model_update_interval: 100,
        }
    }
}

/// 编译特征
#[derive(Debug, Clone)]
pub struct CompilationFeatures {
    /// IR块大小（字节）
    pub block_size: usize,
    /// 指令数量
    pub instruction_count: usize,
    /// 基本块数量
    pub basic_block_count: usize,
    /// 循环数量
    pub loop_count: usize,
    /// 分支数量
    pub branch_count: usize,
    /// 内存访问次数
    pub memory_access_count: usize,
    /// 寄存器压力
    pub register_pressure: f64,
    /// 代码复杂度
    pub complexity_score: f64,
    /// 优化级别
    pub optimization_level: u8,
    /// 启用SIMD
    pub simd_enabled: bool,
    /// 启用并行编译
    pub parallel_compilation: bool,
    /// 目标架构
    pub target_arch: String,
}

/// 编译时间记录
#[derive(Debug, Clone)]
pub struct CompilationRecord {
    /// 编译特征
    pub features: CompilationFeatures,
    /// 实际编译时间（纳秒）
    pub actual_time_ns: u64,
    /// 预测时间（纳秒）
    pub predicted_time_ns: Option<u64>,
    /// 预测误差
    pub prediction_error: Option<f64>,
    /// 记录时间戳
    pub timestamp: std::time::Instant,
}

/// 线性回归模型
#[derive(Debug)]
struct LinearRegressionModel {
    /// 特征权重
    weights: Vec<f64>,
    /// 偏置
    bias: f64,
    /// 学习率
    learning_rate: f64,
    /// 训练样本数
    sample_count: usize,
}

impl LinearRegressionModel {
    /// 创建新的线性回归模型
    fn new(feature_count: usize) -> Self {
        Self {
            weights: vec![0.0; feature_count],
            bias: 0.0,
            learning_rate: 0.01,
            sample_count: 0,
        }
    }
    
    /// 预测编译时间
    fn predict(&self, features: &[f64]) -> f64 {
        let mut prediction = self.bias;
        for (weight, feature) in self.weights.iter().zip(features.iter()) {
            prediction += weight * feature;
        }
        prediction
    }
    
    /// 训练模型
    fn train(&mut self, features: &[f64], target: f64) {
        let prediction = self.predict(features);
        let error = prediction - target;
        
        // 更新权重
        for (weight, feature) in self.weights.iter_mut().zip(features.iter()) {
            *weight -= self.learning_rate * error * feature;
        }
        
        // 更新偏置
        self.bias -= self.learning_rate * error;
        self.sample_count += 1;
        
        // 动态调整学习率
        if self.sample_count % 100 == 0 {
            self.learning_rate *= 0.99; // 衰减学习率
        }
    }
    
    /// 获取模型准确度
    fn accuracy(&self, records: &[CompilationRecord]) -> f64 {
        if records.is_empty() {
            return 0.0;
        }
        
        let mut correct_predictions = 0;
        for record in records {
            if let Some(predicted) = record.predicted_time_ns {
                let error = (predicted as f64 - record.actual_time_ns as f64).abs();
                let relative_error = error / record.actual_time_ns as f64;
                if relative_error < 0.2 { // 20%误差内认为正确
                    correct_predictions += 1;
                }
            }
        }
        
        correct_predictions as f64 / records.len() as f64
    }
}

/// 编译时间预测器
pub struct CompilationTimePredictor {
    /// 配置
    config: CompilationPredictorConfig,
    /// 线性回归模型
    linear_model: Arc<Mutex<LinearRegressionModel>>,
    /// 编译历史记录
    compilation_history: Arc<Mutex<VecDeque<CompilationRecord>>>,
    /// 特征提取器
    feature_extractor: Arc<Mutex<FeatureExtractor>>,
    /// 预测统计
    prediction_stats: Arc<Mutex<PredictionStats>>,
}

/// 预测统计
#[derive(Debug, Clone, Default)]
pub struct PredictionStats {
    /// 总预测次数
    pub total_predictions: u64,
    /// 准确预测次数
    pub accurate_predictions: u64,
    /// 平均预测误差
    pub avg_error: f64,
    /// 最大预测误差
    pub max_error: f64,
    /// 最小预测误差
    pub min_error: f64,
}

/// 特征提取器
#[derive(Debug)]
struct FeatureExtractor;

impl FeatureExtractor {
    /// 提取编译特征
    fn extract_features(&self, block: &IRBlock, config: &CompilationPredictorConfig) -> Vec<f64> {
        let mut features = Vec::new();
        
        // 基本特征
        features.push(block.ops.len() as f64); // 指令数量
        features.push(block.ops.iter().map(|op| op.op.to_string().len()).sum::<usize>() as f64); // 代码复杂度
        features.push(self.count_basic_blocks(block) as f64); // 基本块数量
        features.push(self.count_loops(block) as f64); // 循环数量
        features.push(self.count_branches(block) as f64); // 分支数量
        features.push(self.count_memory_accesses(block) as f64); // 内存访问次数
        features.push(self.calculate_register_pressure(block)); // 寄存器压力
        features.push(self.calculate_complexity_score(block)); // 复杂度分数
        
        // 配置特征
        features.push(config.optimization_level as f64);
        features.push(if config.enable_online_learning { 1.0 } else { 0.0 });
        
        features
    }
    
    /// 计算基本块数量
    fn count_basic_blocks(&self, block: &IRBlock) -> usize {
        let mut count = 0;
        let mut has_branch = false;
        
        for op in &block.ops {
            match &op.op {
                vm_ir::IROp::Beq { .. } |
                vm_ir::IROp::Bne { .. } |
                vm_ir::IROp::Blt { .. } |
                vm_ir::IROp::Bge { .. } |
                vm_ir::IROp::Jmp { .. } => {
                    if !has_branch {
                        count += 1;
                        has_branch = true;
                    }
                }
                _ => {
                    has_branch = false;
                }
            }
        }
        
        count + 1 // 最后一个基本块
    }
    
    /// 计算循环数量
    fn count_loops(&self, block: &IRBlock) -> usize {
        let mut loop_count = 0;
        
        for i in 0..block.ops.len() {
            if let vm_ir::IROp::Blt { .. } | vm_ir::IROp::Bge { .. } = &block.ops[i].op {
                // 简化的循环检测：向后跳转
                if let Some(target) = self.get_branch_target(&block.ops[i]) {
                    if target < i {
                        loop_count += 1;
                    }
                }
            }
        }
        
        loop_count
    }
    
    /// 计算分支数量
    fn count_branches(&self, block: &IRBlock) -> usize {
        block.ops.iter().filter(|op| {
            matches!(&op.op, 
                    vm_ir::IROp::Beq { .. } |
                    vm_ir::IROp::Bne { .. } |
                    vm_ir::IROp::Blt { .. } |
                    vm_ir::IROp::Bge { .. } |
                    vm_ir::IROp::Jmp { .. })
        }).count()
    }
    
    /// 计算内存访问次数
    fn count_memory_accesses(&self, block: &IRBlock) -> usize {
        block.ops.iter().filter(|op| {
            matches!(&op.op, 
                    vm_ir::IROp::Load { .. } |
                    vm_ir::IROp::Store { .. })
        }).count()
    }
    
    /// 计算寄存器压力
    fn calculate_register_pressure(&self, block: &IRBlock) -> f64 {
        let mut register_usage = HashMap::new();
        let mut max_pressure = 0.0;
        
        for op in &block.ops {
            let used_registers = self.get_used_registers(&op.op);
            for reg in used_registers {
                let count = register_usage.entry(reg).or_insert(0);
                *count += 1;
                max_pressure = max_pressure.max(*count as f64);
            }
        }
        
        max_pressure
    }
    
    /// 计算复杂度分数
    fn calculate_complexity_score(&self, block: &IRBlock) -> f64 {
        let mut score = 0.0;
        
        for op in &block.ops {
            match &op.op {
                vm_ir::IROp::Mul { .. } | vm_ir::IROp::Div { .. } => score += 3.0,
                vm_ir::IROp::Add { .. } | vm_ir::IROp::Sub { .. } => score += 1.0,
                vm_ir::IROp::Load { .. } | vm_ir::IROp::Store { .. } => score += 2.0,
                vm_ir::IROp::Beq { .. } | vm_ir::IROp::Bne { .. } => score += 2.5,
                _ => score += 1.0,
            }
        }
        
        score / block.ops.len() as f64
    }
    
    /// 获取使用的寄存器
    fn get_used_registers(&self, op: &vm_ir::IROp) -> Vec<u32> {
        match op {
            vm_ir::IROp::Add { dst, src1, src2 } => vec![*dst, *src1, *src2],
            vm_ir::IROp::Sub { dst, src1, src2 } => vec![*dst, *src1, *src2],
            vm_ir::IROp::Mul { dst, src1, src2 } => vec![*dst, *src1, *src2],
            vm_ir::IROp::Div { dst, src1, src2, .. } => vec![*dst, *src1, *src2],
            vm_ir::IROp::Load { dst, base, .. } => vec![*dst, *base],
            vm_ir::IROp::Store { src, base, .. } => vec![*src, *base],
            vm_ir::IROp::Mov { dst, src } => vec![*dst, *src],
            vm_ir::IROp::MovImm { dst, .. } => vec![*dst],
            _ => Vec::new(),
        }
    }
    
    /// 获取分支目标
    fn get_branch_target(&self, op: &crate::compiler::CompiledIROp) -> Option<usize> {
        match &op.op {
            vm_ir::IROp::Beq { target, .. } |
            vm_ir::IROp::Bne { target, .. } |
            vm_ir::IROp::Blt { target, .. } |
            vm_ir::IROp::Bge { target, .. } |
            vm_ir::IROp::Jmp { target } => Some(*target as usize),
            _ => None,
        }
    }
}

impl CompilationTimePredictor {
    /// 创建新的编译时间预测器
    pub fn new(config: CompilationPredictorConfig) -> Self {
        let feature_count = 10; // 特征数量
        let linear_model = LinearRegressionModel::new(feature_count);
        
        Self {
            config,
            linear_model: Arc::new(Mutex::new(linear_model)),
            compilation_history: Arc::new(Mutex::new(VecDeque::new())),
            feature_extractor: Arc::new(Mutex::new(FeatureExtractor)),
            prediction_stats: Arc::new(Mutex::new(PredictionStats::default())),
        }
    }
    
    /// 预测编译时间
    pub fn predict_compilation_time(&self, block: &IRBlock) -> Option<u64> {
        if !self.config.enabled {
            return None;
        }
        
        // 提取特征
        let features = {
            let extractor = self.feature_extractor.lock().unwrap();
            extractor.extract_features(block, &self.config)
        };
        
        // 使用模型预测
        let prediction = {
            let model = self.linear_model.lock().unwrap();
            model.predict(&features)
        };
        
        Some(prediction as u64)
    }
    
    /// 记录编译结果
    pub fn record_compilation(&self, block: &IRBlock, actual_time_ns: u64) {
        // 提取特征
        let features = {
            let extractor = self.feature_extractor.lock().unwrap();
            CompilationFeatures {
                block_size: block.ops.len(),
                instruction_count: block.ops.len(),
                basic_block_count: extractor.count_basic_blocks(block),
                loop_count: extractor.count_loops(block),
                branch_count: extractor.count_branches(block),
                memory_access_count: extractor.count_memory_accesses(block),
                register_pressure: extractor.calculate_register_pressure(block),
                complexity_score: extractor.calculate_complexity_score(block),
                optimization_level: self.config.optimization_level,
                simd_enabled: false, // 从配置获取
                parallel_compilation: false, // 从配置获取
                target_arch: "x86_64".to_string(), // 从配置获取
            }
        };
        
        // 预测时间
        let feature_vector = {
            let extractor = self.feature_extractor.lock().unwrap();
            extractor.extract_features(block, &self.config)
        };
        
        let predicted_time_ns = {
            let model = self.linear_model.lock().unwrap();
            Some(model.predict(&feature_vector) as u64)
        };
        
        // 计算预测误差
        let prediction_error = predicted_time_ns
            .map(|pred| (pred as f64 - actual_time_ns as f64).abs() / actual_time_ns as f64);
        
        // 创建记录
        let record = CompilationRecord {
            features,
            actual_time_ns,
            predicted_time_ns,
            prediction_error,
            timestamp: std::time::Instant::now(),
        };
        
        // 添加到历史记录
        let mut history = self.compilation_history.lock().unwrap();
        history.push_back(record);
        
        // 保持历史窗口大小
        while history.len() > self.config.training_window_size {
            history.pop_front();
        }
        
        // 更新预测统计
        self.update_prediction_stats(prediction_error);
        
        // 在线学习
        if self.config.enable_online_learning && history.len() >= self.config.min_training_samples {
            self.update_model(&feature_vector, actual_time_ns as f64);
        }
    }
    
    /// 更新预测统计
    fn update_prediction_stats(&self, prediction_error: Option<f64>) {
        if let Some(error) = prediction_error {
            let mut stats = self.prediction_stats.lock().unwrap();
            stats.total_predictions += 1;
            
            if error < 0.2 { // 20%误差内认为准确
                stats.accurate_predictions += 1;
            }
            
            // 更新误差统计
            let count = stats.total_predictions as f64;
            stats.avg_error = (stats.avg_error * (count - 1.0) + error) / count;
            stats.max_error = stats.max_error.max(error);
            stats.min_error = if stats.min_error == 0.0 {
                error
            } else {
                stats.min_error.min(error)
            };
        }
    }
    
    /// 更新模型
    fn update_model(&self, features: &[f64], target: f64) {
        let mut model = self.linear_model.lock().unwrap();
        model.train(features, target);
    }
    
    /// 获取预测统计
    pub fn prediction_stats(&self) -> PredictionStats {
        self.prediction_stats.lock().unwrap().clone()
    }
    
    /// 获取模型准确度
    pub fn model_accuracy(&self) -> f64 {
        let history = self.compilation_history.lock().unwrap();
        if history.len() < self.config.min_training_samples {
            return 0.0;
        }
        
        let model = self.linear_model.lock().unwrap();
        model.accuracy(&history)
    }
    
    /// 重置预测器
    pub fn reset(&self) {
        let feature_count = 10;
        *self.linear_model.lock().unwrap() = LinearRegressionModel::new(feature_count);
        self.compilation_history.lock().unwrap().clear();
        *self.prediction_stats.lock().unwrap() = PredictionStats::default();
    }
}