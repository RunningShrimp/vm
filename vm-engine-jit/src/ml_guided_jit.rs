pub enum CompilationDecision {
    Skip,
    FastJit,
    OptimizedJit,
    Aot,
    Compile,
    Interpret,
}

#[derive(Default)]
pub struct MLGuidedCompiler {}

impl MLGuidedCompiler {
    pub fn new() -> Self {
        Self {}
    }

    pub fn enhance_features_with_pgo(
        &mut self,
        _pc: vm_core::GuestAddr,
        features: &mut Vec<f64>,
        _pgo: &crate::pgo::ProfileData,
    ) {
        // stub: potentially augment features
        let _ = features;
    }

    pub fn predict_decision(
        &self,
        _pc: vm_core::GuestAddr,
        _features: &[f64],
    ) -> CompilationDecision {
        // default to Skip for conservative behavior
        CompilationDecision::Skip
    }
}
