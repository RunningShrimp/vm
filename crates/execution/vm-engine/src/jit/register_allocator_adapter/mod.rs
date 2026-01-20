//! 寄存器分配器适配器模块（基础设施层）
//!
//! 提供寄存器分配器的基础设施层实现。
//! 这是基础设施层的实现，具体的技术细节应在此模块中。

pub mod adapter;

pub use crate::jit::register_allocator::AllocationStrategy;
pub use adapter::RegisterAllocatorAdapter;
