#[derive(Default, Clone)]
pub struct LoopOptimizer {}

impl LoopOptimizer {
    pub fn optimize(&mut self, _block: &mut vm_ir::IRBlock) {
        // stub: mutate block for optimization
        let _ = _block;
    }
}
