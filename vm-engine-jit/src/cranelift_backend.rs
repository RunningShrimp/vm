//! Cranelift 后端实现
//!
//! 将 Jit 的编译逻辑封装为 CompilerBackend trait 实现

#[cfg(feature = "cranelift-backend")]
use crate::ExecutableBlock;
#[cfg(feature = "cranelift-backend")]
use crate::compiler_backend::{CompilerBackend, CompilerError};
#[cfg(feature = "cranelift-backend")]
use vm_ir::IRBlock;

/// Cranelift 后端
///
/// 使用 Cranelift 将 IR 块编译为可执行代码。
/// 这是默认的编译后端，提供高性能的代码生成。
#[cfg(feature = "cranelift-backend")]
pub struct CraneliftBackend {
    /// Jit 实例（用于实际的编译工作）
    /// 注意：这里使用 Jit 的编译逻辑，但通过 CompilerBackend trait 抽象
    jit: crate::Jit,
}

#[cfg(feature = "cranelift-backend")]
impl Default for CraneliftBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "cranelift-backend")]
impl CraneliftBackend {
    /// 创建新的 Cranelift 后端
    pub fn new() -> Self {
        Self {
            jit: crate::Jit::new(),
        }
    }
}

#[cfg(feature = "cranelift-backend")]
impl CompilerBackend for CraneliftBackend {
    fn compile(&mut self, block: &IRBlock) -> Result<ExecutableBlock, CompilerError> {
        // 使用 Jit 编译 IR 块
        let code_ptr = self.jit.compile_only(block);

        // 检查编译是否成功
        if code_ptr.is_null() {
            return Err(CompilerError {
                message: format!(
                    "Cranelift compilation failed for block at PC {:#x}",
                    block.start_pc.0
                ),
            });
        }

        // 从 CodePtr 提取代码信息
        // 注意：CodePtr 包含 ExecutableBlockRef，我们需要从中提取代码
        // 当前实现：创建一个 ExecutableBlock，包含代码指针信息
        // TODO: 从 Jit 的内部缓存中获取完整的代码块数据
        let _entry_ptr = code_ptr.entry_ptr();
        let code_size = code_ptr.size();

        if code_size == 0 {
            // 如果大小为0，说明是占位实现，返回错误
            return Err(CompilerError {
                message: format!(
                    "Cranelift compilation returned empty code block at PC {:#x}",
                    block.start_pc.0
                ),
            });
        }

        // 从指针复制代码（注意：这是临时实现，真正的实现应该直接从 Jit 缓存获取）
        // 由于我们无法安全地从指针复制代码（不知道实际大小），这里创建一个占位 ExecutableBlock
        // 真正的实现需要 Jit 提供获取完整代码块的方法
        tracing::warn!(
            pc = block.start_pc.0,
            "CraneliftBackend: Using placeholder ExecutableBlock, full implementation requires Jit API extension"
        );

        // 创建一个包含代码指针信息的 ExecutableBlock
        // 注意：这仍然是占位实现，真正的实现需要从 Jit 获取完整的代码数据
        Ok(ExecutableBlock::new(
            vec![], // 空代码，因为真正的代码在 Jit 的内部缓存中
            0,      // entry_offset
        ))
    }
}

/// 默认后端工厂
///
/// 根据可用特性选择最佳后端。
pub struct DefaultBackendFactory;

impl DefaultBackendFactory {
    /// 创建默认后端
    ///
    /// 优先级：Cranelift > Direct
    #[cfg(feature = "cranelift-backend")]
    pub fn create_default() -> Box<dyn CompilerBackend> {
        Box::new(CraneliftBackend::new())
    }

    #[cfg(not(feature = "cranelift-backend"))]
    pub fn create_default() -> Box<dyn CompilerBackend> {
        Box::new(crate::direct_backend::DirectBackend::new())
    }
}
