use crate::compiler_backend::{CompilerBackend, CompilerError};
use vm_ir::IRBlock;

#[derive(Default)]
pub struct DirectBackend;

impl CompilerBackend for DirectBackend {
    fn compile(&mut self, _block: &IRBlock) -> Result<crate::ExecutableBlock, CompilerError> {
        Ok(crate::ExecutableBlock {
            code: vec![0xC3],
            entry_offset: 0,
            code_size: 1,
            executable: true,
        })
    }
}
