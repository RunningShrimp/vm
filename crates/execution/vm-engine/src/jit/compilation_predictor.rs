use crate::jit::common::OptimizationStats;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use vm_ir::IRBlock;

pub struct CompilationPredictor {
    config: CompilationPredictorConfig,
    history: VecDeque<CompilationRecord>,
    predictions: HashMap<u64, Prediction>,
    stats: OptimizationStats,
}

#[derive(Debug, Clone)]
pub struct CompilationPredictorConfig {
    pub history_size: usize,
    pub min_samples_for_prediction: usize,
    pub prediction_window: Duration,
    pub enable_ml_prediction: bool,
    pub confidence_threshold: f64,
}

impl Default for CompilationPredictorConfig {
    fn default() -> Self {
        Self {
            history_size: 1000,
            min_samples_for_prediction: 10,
            prediction_window: Duration::from_secs(60),
            enable_ml_prediction: false,
            confidence_threshold: 0.7,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompilationRecord {
    pub block_id: u64,
    pub compilation_time: Duration,
    pub code_size: usize,
    pub optimization_level: u8,
    pub timestamp: Instant,
    pub execution_count: u64,
    pub predicted: bool,
}

#[derive(Debug, Clone)]
pub struct Prediction {
    pub block_id: u64,
    pub predicted_compilation_time: Duration,
    pub confidence: f64,
    pub recommended_optimization: u8,
    pub predicted_code_size: usize,
    pub method: PredictionMethod,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PredictionMethod {
    LinearRegression,
    MovingAverage,
    NeuralNetwork,
    Heuristic,
}

#[derive(Debug, Clone)]
pub struct PredictionAccuracy {
    pub mean_absolute_error: Duration,
    pub mean_squared_error: f64,
    pub accuracy_percentage: f64,
    pub sample_count: usize,
}

impl CompilationPredictor {
    pub fn new(config: CompilationPredictorConfig) -> Self {
        Self {
            config,
            history: VecDeque::new(),
            predictions: HashMap::new(),
            stats: OptimizationStats::default(),
        }
    }

    pub fn record_compilation(
        &mut self,
        block_id: u64,
        compilation_time: Duration,
        code_size: usize,
        optimization_level: u8,
        execution_count: u64,
    ) {
        let record = CompilationRecord {
            block_id,
            compilation_time,
            code_size,
            optimization_level,
            timestamp: Instant::now(),
            execution_count,
            predicted: false,
        };

        self.history.push_back(record);

        if self.history.len() > self.config.history_size {
            self.history.pop_front();
        }

        self.update_predictions();
    }

    pub fn predict_compilation_time(
        &mut self,
        block: &IRBlock,
        execution_count: u64,
    ) -> Option<Prediction> {
        let block_id = block.start_pc.0;

        if let Some(prediction) = self.predictions.get(&block_id)
            && prediction.confidence >= self.config.confidence_threshold
        {
            return Some(prediction.clone());
        }

        let samples: Vec<&CompilationRecord> = self
            .history
            .iter()
            .filter(|r| r.block_id == block_id || r.code_size == block.ops.len())
            .collect();

        if samples.len() < self.config.min_samples_for_prediction {
            return None;
        }

        let prediction = if self.config.enable_ml_prediction {
            self.ml_predict(block_id, &samples, execution_count)
        } else {
            self.heuristic_predict(block_id, &samples, execution_count)
        };

        self.predictions.insert(block_id, prediction.clone());
        self.stats.blocks_optimized += 1;
        Some(prediction)
    }

    fn heuristic_predict(
        &self,
        block_id: u64,
        samples: &[&CompilationRecord],
        execution_count: u64,
    ) -> Prediction {
        let avg_time = self.calculate_average_time(samples);
        let avg_size = self.calculate_average_size(samples);
        let confidence = self.calculate_confidence(samples);

        let recommended_level = if execution_count > 1000 {
            3
        } else if execution_count > 100 {
            2
        } else if execution_count > 10 {
            1
        } else {
            0
        };

        Prediction {
            block_id,
            predicted_compilation_time: avg_time,
            confidence,
            recommended_optimization: recommended_level,
            predicted_code_size: avg_size,
            method: PredictionMethod::Heuristic,
        }
    }

    fn ml_predict(
        &self,
        block_id: u64,
        samples: &[&CompilationRecord],
        execution_count: u64,
    ) -> Prediction {
        let avg_time = self.calculate_average_time(samples);
        let avg_size = self.calculate_average_size(samples);

        let trend = self.calculate_trend(samples);
        let predicted_time = self.apply_trend(avg_time, trend);

        let confidence = self.calculate_confidence(samples).min(0.95);

        let recommended_level = self.predict_optimization_level(samples, execution_count);

        Prediction {
            block_id,
            predicted_compilation_time: predicted_time,
            confidence,
            recommended_optimization: recommended_level,
            predicted_code_size: avg_size,
            method: PredictionMethod::LinearRegression,
        }
    }

    fn calculate_average_time(&self, samples: &[&CompilationRecord]) -> Duration {
        let total: Duration = samples.iter().map(|r| r.compilation_time).sum();
        total / samples.len() as u32
    }

    fn calculate_average_size(&self, samples: &[&CompilationRecord]) -> usize {
        samples.iter().map(|r| r.code_size).sum::<usize>() / samples.len()
    }

    fn calculate_confidence(&self, samples: &[&CompilationRecord]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }

        let avg_time = self.calculate_average_time(samples).as_nanos() as f64;
        let variance = samples
            .iter()
            .map(|r| {
                let diff = r.compilation_time.as_nanos() as f64 - avg_time;
                diff * diff
            })
            .sum::<f64>()
            / samples.len() as f64;

        let std_dev = variance.sqrt();
        if avg_time == 0.0 {
            return 1.0;
        }

        (1.0 - (std_dev / avg_time)).clamp(0.0, 1.0)
    }

    fn calculate_trend(&self, samples: &[&CompilationRecord]) -> f64 {
        if samples.len() < 2 {
            return 0.0;
        }

        let n = samples.len() as f64;
        let sum_x = (0..samples.len()).map(|i| i as f64).sum::<f64>();
        let sum_y = samples
            .iter()
            .map(|r| r.compilation_time.as_nanos() as f64)
            .sum::<f64>();
        let sum_xy = samples
            .iter()
            .enumerate()
            .map(|(i, r)| i as f64 * r.compilation_time.as_nanos() as f64)
            .sum::<f64>();
        let sum_x2 = (0..samples.len())
            .map(|i| (i as f64) * (i as f64))
            .sum::<f64>();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
        slope / (samples.len() as f64)
    }

    fn apply_trend(&self, base_time: Duration, trend: f64) -> Duration {
        let nanos = (base_time.as_nanos() as f64 + trend) as u64;
        Duration::from_nanos(nanos)
    }

    fn predict_optimization_level(
        &self,
        samples: &[&CompilationRecord],
        execution_count: u64,
    ) -> u8 {
        let avg_level = samples
            .iter()
            .map(|r| r.optimization_level as u64)
            .sum::<u64>()
            / samples.len() as u64;

        if execution_count > 1000 {
            avg_level.min(3) as u8
        } else if execution_count > 100 {
            (avg_level + 1).min(2) as u8
        } else {
            avg_level.min(1) as u8
        }
    }

    fn update_predictions(&mut self) {
        self.predictions.clear();
    }

    pub fn get_prediction(&self, block_id: u64) -> Option<&Prediction> {
        self.predictions.get(&block_id)
    }

    pub fn calculate_accuracy(&self) -> PredictionAccuracy {
        let accurate_predictions: Vec<&CompilationRecord> =
            self.history.iter().filter(|r| r.predicted).collect();

        if accurate_predictions.is_empty() {
            return PredictionAccuracy {
                mean_absolute_error: Duration::ZERO,
                mean_squared_error: 0.0,
                accuracy_percentage: 0.0,
                sample_count: 0,
            };
        }

        let mut total_error = 0u64;
        let mut total_squared_error = 0.0;
        let mut accurate_count = 0;

        for record in &accurate_predictions {
            if let Some(prediction) = self.predictions.get(&record.block_id) {
                let error = prediction.predicted_compilation_time.as_nanos() as i64
                    - record.compilation_time.as_nanos() as i64;
                total_error += error.unsigned_abs();
                total_squared_error += (error as f64) * (error as f64);

                let relative_error =
                    error.unsigned_abs() as f64 / record.compilation_time.as_nanos().max(1) as f64;
                if relative_error < 0.2 {
                    accurate_count += 1;
                }
            }
        }

        let mean_absolute_error =
            Duration::from_nanos(total_error / accurate_predictions.len() as u64);
        let mean_squared_error = total_squared_error / accurate_predictions.len() as f64;
        let accuracy_percentage =
            (accurate_count as f64 / accurate_predictions.len() as f64) * 100.0;

        PredictionAccuracy {
            mean_absolute_error,
            mean_squared_error,
            accuracy_percentage,
            sample_count: accurate_predictions.len(),
        }
    }

    pub fn should_compile(&mut self, block: &IRBlock, execution_count: u64) -> bool {
        if let Some(prediction) = self.predict_compilation_time(block, execution_count) {
            prediction.confidence >= self.config.confidence_threshold
                && prediction.predicted_compilation_time < Duration::from_millis(100)
        } else {
            execution_count >= 10
        }
    }

    pub fn get_stats(&self) -> &OptimizationStats {
        &self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = OptimizationStats::default();
    }
}

pub struct DefaultCompilationPredictor {
    inner: CompilationPredictor,
}

impl DefaultCompilationPredictor {
    pub fn new() -> Self {
        let config = CompilationPredictorConfig::default();
        Self {
            inner: CompilationPredictor::new(config),
        }
    }

    pub fn record_compilation(
        &mut self,
        block_id: u64,
        compilation_time: Duration,
        code_size: usize,
        optimization_level: u8,
        execution_count: u64,
    ) {
        self.inner.record_compilation(
            block_id,
            compilation_time,
            code_size,
            optimization_level,
            execution_count,
        );
    }

    pub fn predict(&mut self, block: &IRBlock, execution_count: u64) -> Option<Prediction> {
        self.inner.predict_compilation_time(block, execution_count)
    }

    pub fn should_compile(&mut self, block: &IRBlock, execution_count: u64) -> bool {
        self.inner.should_compile(block, execution_count)
    }

    pub fn get_accuracy(&self) -> PredictionAccuracy {
        self.inner.calculate_accuracy()
    }
}

impl Default for DefaultCompilationPredictor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compilation_predictor_creation() {
        let config = CompilationPredictorConfig::default();
        let predictor = CompilationPredictor::new(config);
        assert_eq!(predictor.history.len(), 0);
    }

    #[test]
    fn test_record_compilation() {
        let config = CompilationPredictorConfig::default();
        let mut predictor = CompilationPredictor::new(config);

        predictor.record_compilation(1, Duration::from_millis(10), 100, 0, 5);
        assert_eq!(predictor.history.len(), 1);
    }

    #[test]
    fn test_prediction_confidence() {
        let config = CompilationPredictorConfig::default();
        let mut predictor = CompilationPredictor::new(config);

        for i in 0..15 {
            predictor.record_compilation(1, Duration::from_millis(10 + i), 100, 0, 5);
        }

        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: vm_ir::Terminator::Ret,
        };

        if let Some(prediction) = predictor.predict_compilation_time(&block, 5) {
            assert!(prediction.confidence > 0.0);
        }
    }

    #[test]
    fn test_should_compile() {
        let config = CompilationPredictorConfig::default();
        let mut predictor = CompilationPredictor::new(config);

        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: vm_ir::Terminator::Ret,
        };

        assert!(predictor.should_compile(&block, 15));
    }
}
