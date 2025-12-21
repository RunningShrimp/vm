pub struct LinearRegressionModel {
    pub weight: f64,
}

impl LinearRegressionModel {
    pub fn with_optimized_weights(w: f64) -> Self {
        Self { weight: w }
    }
}

#[derive(Default)]
pub struct OnlineLearner {}

use std::time::Duration;
impl OnlineLearner {
    // Accept construction parameters for compatibility with callers
    pub fn new(_model: Box<LinearRegressionModel>, _window: usize, _interval: Duration) -> Self {
        Self {}
    }
    pub fn add_sample(
        &mut self,
        _features: Vec<f64>,
        _decision: crate::ml_guided_jit::CompilationDecision,
        _perf: f64,
    ) {
    }
}

#[derive(Default)]
pub struct PerformanceValidator {}

impl PerformanceValidator {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get_performance_report(&self) -> crate::ml_model::PerformanceReport {
        crate::ml_model::PerformanceReport { score: 0.0 }
    }
}

#[derive(Clone, Debug)]
pub struct PerformanceReport {
    pub score: f64,
}

pub struct FeatureExtractor;
impl FeatureExtractor {
    pub fn extract_from_ir_block(_block: &vm_ir::IRBlock) -> Vec<f64> {
        vec![]
    }
}
