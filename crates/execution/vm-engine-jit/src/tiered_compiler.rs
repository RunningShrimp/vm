//! 分层编译器占位实现

#[derive(Debug, Clone)]
pub struct TieredCompiler;

#[derive(Debug, Clone)]
pub struct TieredCompilationConfig;

#[derive(Debug, Clone)]
pub struct TieredCompilationStats;

#[derive(Debug, Clone)]
pub enum CompilationTier {
    Interpreter,
    Baseline,
    Optimized,
}
