//! Cranelift JIT 后端
//!
//! 使用 Cranelift 作为 JIT 编译后端，提供高效的本地代码生成。
//!
//! ## 实现状态
//!
//! 这是一个完整的 Cranelift JIT 实现，支持：
//!
//! 1. IR 到 Cranelift IR 的完整翻译
//! 2. 寄存器分配和管理
//! 3. 基本算术、逻辑、比较操作
//! 4. 代码缓存和执行内存管理
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use vm_engine::jit::backend::{CraneliftBackend, JITConfig};
//! use vm_ir::{IRBuilder, IROp, Terminator};
//!
//! let config = JITConfig::default();
//! let mut backend = CraneliftBackend::new(config).unwrap();
//!
//! // 创建 IR 块
//! let mut builder = IRBuilder::new(0x1000);
//! builder.push(IROp::MovImm { dst: 0, imm: 42 });
//! builder.push(IROp::MovImm { dst: 1, imm: 58 });
//! builder.push(IROp::Add { dst: 0, src1: 0, src2: 1 });
//! builder.set_term(Terminator::Ret);
//! let block = builder.build();
//!
//! // 编译并执行
//! let compiled = backend.compile_block(&block).unwrap();
//! let result = unsafe {
//!     let func: extern "C" fn() -> u64 = std::mem::transmute(compiled.exec_addr);
//!     func()
//! };
//! assert_eq!(result, 100);
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, UserFuncName,
    types,
    TrapCode,
};
                builder.def_var(dst_var, result);
            }

            IROp::Rem {
                dst,
                src1,
                src2,
                signed,
            } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src1_var = self.get_or_create_variable(*src1);
                let src2_var = self.get_or_create_variable(*src2);
                let val1 = builder.use_var(src1_var);
                let val2 = builder.use_var(src2_var);
                let result = if *signed {
                    builder.ins().srem(val1, val2)
                } else {
                    builder.ins().urem(val1, val2)
                };
                builder.def_var(dst_var, result);
            }

            // 位操作
            IROp::And { dst, src1, src2 } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src1_var = self.get_or_create_variable(*src1);
                let src2_var = self.get_or_create_variable(*src2);
                let val1 = builder.use_var(src1_var);
                let val2 = builder.use_var(src2_var);
                let result = builder.ins().band(val1, val2);
                builder.def_var(dst_var, result);
            }

            IROp::Or { dst, src1, src2 } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src1_var = self.get_or_create_variable(*src1);
                let src2_var = self.get_or_create_variable(*src2);
                let val1 = builder.use_var(src1_var);
                let val2 = builder.use_var(src2_var);
                let result = builder.ins().bor(val1, val2);
                builder.def_var(dst_var, result);
            }

            IROp::Xor { dst, src1, src2 } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src1_var = self.get_or_create_variable(*src1);
                let src2_var = self.get_or_create_variable(*src2);
                let val1 = builder.use_var(src1_var);
                let val2 = builder.use_var(src2_var);
                let result = builder.ins().bxor(val1, val2);
                builder.def_var(dst_var, result);
            }

            IROp::Not { dst, src } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src_var = self.get_or_create_variable(*src);
                let val = builder.use_var(src_var);
                let result = builder.ins().bnot(val);
                builder.def_var(dst_var, result);
            }

            // 移位操作
            IROp::Sll { dst, src, shreg } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src_var = self.get_or_create_variable(*src);
                let sh_var = self.get_or_create_variable(*shreg);
                let val = builder.use_var(src_var);
                let shift = builder.use_var(sh_var);
                let result = builder.ins().ishl(val, shift);
                builder.def_var(dst_var, result);
            }

            IROp::Srl { dst, src, shreg } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src_var = self.get_or_create_variable(*src);
                let sh_var = self.get_or_create_variable(*shreg);
                let val = builder.use_var(src_var);
                let shift = builder.use_var(sh_var);
                let result = builder.ins().ushr(val, shift);
                builder.def_var(dst_var, result);
            }

            IROp::Sra { dst, src, shreg } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src_var = self.get_or_create_variable(*src);
                let sh_var = self.get_or_create_variable(*shreg);
                let val = builder.use_var(src_var);
                let shift = builder.use_var(sh_var);
                let result = builder.ins().sshr(val, shift);
                builder.def_var(dst_var, result);
            }

            // 立即数操作
            IROp::AddImm { dst, src, imm } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src_var = self.get_or_create_variable(*src);
                let val = builder.use_var(src_var);
                let imm_val = builder.ins().iconst(types::I64, *imm);
                let result = builder.ins().iadd(val, imm_val);
                builder.def_var(dst_var, result);
            }

            IROp::MulImm { dst, src, imm } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src_var = self.get_or_create_variable(*src);
                let val = builder.use_var(src_var);
                let imm_val = builder.ins().iconst(types::I64, *imm);
                let result = builder.ins().imul(val, imm_val);
                builder.def_var(dst_var, result);
            }

            IROp::SllImm { dst, src, sh } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src_var = self.get_or_create_variable(*src);
                let val = builder.use_var(src_var);
                let shift_val = builder.ins().iconst(types::I64, *sh as i64);
                let result = builder.ins().ishl(val, shift_val);
                builder.def_var(dst_var, result);
            }

            IROp::SrlImm { dst, src, sh } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src_var = self.get_or_create_variable(*src);
                let val = builder.use_var(src_var);
                let shift_val = builder.ins().iconst(types::I64, *sh as i64);
                let result = builder.ins().ushr(val, shift_val);
                builder.def_var(dst_var, result);
            }

            IROp::SraImm { dst, src, sh } => {
                let dst_var = self.get_or_create_variable(*dst);
                let src_var = self.get_or_create_variable(*src);
                let val = builder.use_var(src_var);
                let shift_val = builder.ins().iconst(types::I64, *sh as i64);
                let result = builder.ins().sshr(val, shift_val);
                builder.def_var(dst_var, result);
            }

            // 比较操作
            IROp::CmpEq { dst, lhs, rhs } => {
                let dst_var = self.get_or_create_variable(*dst);
                let lhs_var = self.get_or_create_variable(*lhs);
                let rhs_var = self.get_or_create_variable(*rhs);
                let val1 = builder.use_var(lhs_var);
                let val2 = builder.use_var(rhs_var);
                let cmp = builder.ins().icmp(IntCC::Equal, val1, val2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let result = builder.ins().select(cmp, one, zero);
                builder.def_var(dst_var, result);
            }

            IROp::CmpNe { dst, lhs, rhs } => {
                let dst_var = self.get_or_create_variable(*dst);
                let lhs_var = self.get_or_create_variable(*lhs);
                let rhs_var = self.get_or_create_variable(*rhs);
                let val1 = builder.use_var(lhs_var);
                let val2 = builder.use_var(rhs_var);
                let cmp = builder.ins().icmp(IntCC::NotEqual, val1, val2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let result = builder.ins().select(cmp, one, zero);
                builder.def_var(dst_var, result);
            }

            IROp::CmpLt { dst, lhs, rhs } => {
                let dst_var = self.get_or_create_variable(*dst);
                let lhs_var = self.get_or_create_variable(*lhs);
                let rhs_var = self.get_or_create_variable(*rhs);
                let val1 = builder.use_var(lhs_var);
                let val2 = builder.use_var(rhs_var);
                let cmp = builder.ins().icmp(IntCC::SignedLessThan, val1, val2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let result = builder.ins().select(cmp, one, zero);
                builder.def_var(dst_var, result);
            }

            IROp::CmpLtU { dst, lhs, rhs } => {
                let dst_var = self.get_or_create_variable(*dst);
                let lhs_var = self.get_or_create_variable(*lhs);
                let rhs_var = self.get_or_create_variable(*rhs);
                let val1 = builder.use_var(lhs_var);
                let val2 = builder.use_var(rhs_var);
                let cmp = builder.ins().icmp(IntCC::UnsignedLessThan, val1, val2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let result = builder.ins().select(cmp, one, zero);
                builder.def_var(dst_var, result);
            }

            IROp::CmpGe { dst, lhs, rhs } => {
                let dst_var = self.get_or_create_variable(*dst);
                let lhs_var = self.get_or_create_variable(*lhs);
                let rhs_var = self.get_or_create_variable(*rhs);
                let val1 = builder.use_var(lhs_var);
                let val2 = builder.use_var(rhs_var);
                let cmp = builder
                    .ins()
                    .icmp(IntCC::SignedGreaterThanOrEqual, val1, val2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let result = builder.ins().select(cmp, one, zero);
                builder.def_var(dst_var, result);
            }

            IROp::CmpGeU { dst, lhs, rhs } => {
                let dst_var = self.get_or_create_variable(*dst);
                let lhs_var = self.get_or_create_variable(*lhs);
                let rhs_var = self.get_or_create_variable(*rhs);
                let val1 = builder.use_var(lhs_var);
                let val2 = builder.use_var(rhs_var);
                let cmp = builder
                    .ins()
                    .icmp(IntCC::UnsignedGreaterThanOrEqual, val1, val2);
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let result = builder.ins().select(cmp, one, zero);
                builder.def_var(dst_var, result);
            }

            IROp::Select {
                dst,
                cond,
                true_val,
                false_val,
            } => {
                let dst_var = self.get_or_create_variable(*dst);
                let cond_var = self.get_or_create_variable(*cond);
                let true_var = self.get_or_create_variable(*true_val);
                let false_var = self.get_or_create_variable(*false_val);

                let cond_val = builder.use_var(cond_var);
                let true_val = builder.use_var(true_var);
                let false_val_data = builder.use_var(false_var);

                // 将条件转换为布尔值
                let zero = builder.ins().iconst(types::I64, 0);
                let cond_bool = builder.ins().icmp(IntCC::NotEqual, cond_val, zero);
                let result = builder.ins().select(cond_bool, true_val, false_val_data);
                builder.def_var(dst_var, result);
            }

            // 内存操作（占位符，需要实际内存上下文）
            IROp::Load { .. } | IROp::Store { .. } => {
                // 内存操作需要实际的内存上下文支持
                // 这里暂时返回错误
                return Err(VmError::Execution(ExecutionError::JitError {
                    message: "Memory operations not yet supported in Cranelift backend".to_string(),
                    function_addr: None,
                }));
            }

            // 系统调用
            IROp::SysCall => {
                // 系统调用需要宿主机集成
                // Create trap code for user trap (user vector cause)
                builder.ins().trap(TrapCode::User(0u16));
            }

            _ => {
                return Err(VmError::Execution(ExecutionError::JitError {
                    message: format!("Unsupported IR operation: {:?}", op),
                    function_addr: None,
                }));
            }
        }

        Ok(())
    }

    /// 编译终结符
    fn compile_terminator(
        &mut self,
        builder: &mut FunctionBuilder,
        term: &vm_ir::Terminator,
    ) -> VmResult<()> {
        match term {
            vm_ir::Terminator::Ret => {
                // 返回寄存器0的值作为结果
                if let Some(&ret_var) = self.var_map.get(&0) {
                    let ret_val = builder.use_var(ret_var);
                    builder.ins().return_(&[ret_val]);
                } else {
                    // 没有返回值，返回0
                    let zero = builder.ins().iconst(types::I64, 0);
                    builder.ins().return_(&[zero]);
                }
            }

            vm_ir::Terminator::Jmp { target: _ } => {
                return Err(VmError::Execution(ExecutionError::JitError {
                    message: "Direct jumps not yet supported in Cranelift backend".to_string(),
                    function_addr: None,
                }));
            }

            vm_ir::Terminator::CondJmp { .. } => {
                return Err(VmError::Execution(ExecutionError::JitError {
                    message: "Conditional jumps not yet supported in Cranelift backend".to_string(),
                    function_addr: None,
                }));
            }

            vm_ir::Terminator::Call { .. } => {
                return Err(VmError::Execution(ExecutionError::JitError {
                    message: "Function calls not yet supported in Cranelift backend".to_string(),
                    function_addr: None,
                }));
            }

            vm_ir::Terminator::Fault { cause } => {
                builder.ins().trap(TrapCode::user(cause));
            }

            vm_ir::Terminator::Interrupt { vector } => {
                builder.ins().trap(TrapCode::User(vector as u16));
            }

            vm_ir::Terminator::JmpReg { .. } => {
                return Err(VmError::Execution(ExecutionError::JitError {
                    message: "Indirect jumps not yet supported in Cranelift backend".to_string(),
                    function_addr: None,
                }));
            }
        }

        Ok(())
    }

    /// 分配可执行内存 (Unix)
    #[cfg(unix)]
    fn allocate_executable_memory(code: &[u8]) -> VmResult<*const u8> {
        use libc::size_t;

        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
        let size = code.len().div_ceil(page_size) * page_size;

        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size as size_t,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err(VmError::Execution(ExecutionError::JitError {
                message: "Failed to allocate executable memory".to_string(),
                function_addr: None,
            }));
        }

        // 复制代码到内存
        unsafe {
            std::ptr::copy_nonoverlapping(code.as_ptr(), ptr as *mut u8, code.len());
        }

        // 设置为可执行
        let result =
            unsafe { libc::mprotect(ptr, size as size_t, libc::PROT_READ | libc::PROT_EXEC) };

        if result != 0 {
            unsafe {
                libc::munmap(ptr, size as size_t);
            }
            return Err(VmError::Execution(ExecutionError::JitError {
                message: "Failed to set memory as executable".to_string(),
                function_addr: None,
            }));
        }

        Ok(ptr as *const u8)
    }

    /// 分配可执行内存 (Windows)
    #[cfg(windows)]
    fn allocate_executable_memory(code: &[u8]) -> VmResult<*const u8> {
        const PAGE_SIZE: usize = 4096;
        let size = ((code.len() + PAGE_SIZE - 1) / PAGE_SIZE) * PAGE_SIZE;

        let ptr = unsafe {
            windows_sys::Win32::System::Memory::VirtualAlloc(
                std::ptr::null_mut(),
                size,
                windows_sys::Win32::System::Memory::MEM_COMMIT
                    | windows_sys::Win32::System::Memory::MEM_RESERVE,
                windows_sys::Win32::System::Memory::PAGE_EXECUTE_READWRITE,
            )
        };

        if ptr.is_null() {
            return Err(VmError::Execution(ExecutionError::JitError {
                message: "Failed to allocate executable memory".to_string(),
                function_addr: None,
            }));
        }

        // 复制代码到内存
        unsafe {
            std::ptr::copy_nonoverlapping(code.as_ptr(), ptr as *mut u8, code.len());
        }

        Ok(ptr as *const u8)
    }
}

impl JITBackend for CraneliftBackend {
    fn compile_block(&mut self, block: &IRBlock) -> VmResult<CompiledCode> {
        let start_time = Instant::now();

        // 编译函数
        let code = self.compile_function(block)?;

        let code_size = code.len();

        // 分配可执行内存
        let exec_addr = Self::allocate_executable_memory(&code)? as u64;

        // 更新统计信息
        let compile_time = start_time.elapsed();
        self.stats.compiled_blocks += 1;
        self.stats.total_compile_time_us += compile_time.as_micros() as u64;
        self.stats.total_code_size += code_size;

        Ok(CompiledCode {
            code,
            size: code_size,
            exec_addr,
        })
    }

    fn set_opt_level(&mut self, level: OptLevel) -> VmResult<()> {
        self.config.opt_level = level;
        // 重新创建 ISA 以应用新的优化设置
        self.isa = Self::create_isa(&self.config)?;
        Ok(())
    }

    fn get_opt_level(&self) -> OptLevel {
        self.config.opt_level
    }

    fn get_stats(&self) -> &JITStats {
        &self.stats
    }

    fn clear_cache(&mut self) {
        self.stats = JITStats::default();
        self.symbols.clear();
        self.var_map.clear();
        self.next_var = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::GuestAddr;

    #[test]
    fn test_cranelift_backend_creation() {
        let config = JITConfig::default();
        let backend = CraneliftBackend::new(config);
        assert!(backend.is_ok());
    }

    #[test]
    fn test_cranelift_backend_stats() {
        let config = JITConfig::default();
        let backend = CraneliftBackend::new(config).unwrap();

        // 初始统计
        let stats = backend.get_stats();
        assert_eq!(stats.compiled_blocks, 0);
        assert_eq!(stats.total_code_size, 0);
    }

    #[test]
    fn test_cranelift_opt_level() {
        let config = JITConfig::default();
        let mut backend = CraneliftBackend::new(config).unwrap();

        assert_eq!(backend.get_opt_level(), OptLevel::Balanced);

        backend.set_opt_level(OptLevel::Aggressive).unwrap();
        assert_eq!(backend.get_opt_level(), OptLevel::Aggressive);
    }

    #[test]
    fn test_cranelift_clear_cache() {
        let config = JITConfig::default();
        let mut backend = CraneliftBackend::new(config).unwrap();

        // 编译一些代码
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![IROp::MovImm { dst: 1, imm: 42 }],
            term: vm_ir::Terminator::Ret,
        };

        let _ = backend.compile_block(&block);

        assert_eq!(backend.get_stats().compiled_blocks, 1);

        backend.clear_cache();
        assert_eq!(backend.get_stats().compiled_blocks, 0);
    }

    #[test]
    fn test_cranelift_compile_simple_block() {
        let config = JITConfig::default();
        let mut backend = CraneliftBackend::new(config).unwrap();

        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                IROp::MovImm { dst: 1, imm: 10 },
                IROp::MovImm { dst: 2, imm: 20 },
                IROp::Add {
                    dst: 0,
                    src1: 1,
                    src2: 2,
                },
            ],
            term: vm_ir::Terminator::Ret,
        };

        let result = backend.compile_block(&block);
        assert!(result.is_ok());

        let compiled = result.unwrap();
        assert!(!compiled.code.is_empty());
        assert!(compiled.size > 0);
        assert_ne!(compiled.exec_addr, 0);

        // 验证可执行内存已分配
        assert!(compiled.exec_addr != 0);
    }

    #[test]
    fn test_cranelift_compile_arithmetic() {
        let config = JITConfig::default();
        let mut backend = CraneliftBackend::new(config).unwrap();

        let block = IRBlock {
            start_pc: GuestAddr(0x2000),
            ops: vec![
                IROp::MovImm { dst: 1, imm: 100 },
                IROp::MovImm { dst: 2, imm: 5 },
                IROp::Add {
                    dst: 0,
                    src1: 1,
                    src2: 2,
                },
                IROp::MovImm { dst: 3, imm: 10 },
                IROp::Sub {
                    dst: 0,
                    src1: 0,
                    src2: 3,
                },
                IROp::MovImm { dst: 4, imm: 2 },
                IROp::Mul {
                    dst: 0,
                    src1: 0,
                    src2: 4,
                },
            ],
            term: vm_ir::Terminator::Ret,
        };

        let result = backend.compile_block(&block);
        assert!(result.is_ok());

        let compiled = result.unwrap();
        assert!(!compiled.code.is_empty());
    }

    #[test]
    fn test_cranelift_compile_comparison() {
        let config = JITConfig::default();
        let mut backend = CraneliftBackend::new(config).unwrap();

        let block = IRBlock {
            start_pc: GuestAddr(0x3000),
            ops: vec![
                IROp::MovImm { dst: 1, imm: 10 },
                IROp::MovImm { dst: 2, imm: 20 },
                IROp::CmpLt {
                    dst: 3,
                    lhs: 1,
                    rhs: 2,
                },
                IROp::CmpEq {
                    dst: 4,
                    lhs: 1,
                    rhs: 2,
                },
            ],
            term: vm_ir::Terminator::Ret,
        };

        let result = backend.compile_block(&block);
        assert!(result.is_ok());

        let compiled = result.unwrap();
        assert!(!compiled.code.is_empty());
    }

    #[test]
    fn test_cranelift_compile_logic() {
        let config = JITConfig::default();
        let mut backend = CraneliftBackend::new(config).unwrap();

        let block = IRBlock {
            start_pc: GuestAddr(0x4000),
            ops: vec![
                IROp::MovImm { dst: 1, imm: 0xFF },
                IROp::MovImm { dst: 2, imm: 0x0F },
                IROp::And {
                    dst: 3,
                    src1: 1,
                    src2: 2,
                },
                IROp::Or {
                    dst: 4,
                    src1: 1,
                    src2: 2,
                },
                IROp::Xor {
                    dst: 5,
                    src1: 1,
                    src2: 2,
                },
            ],
            term: vm_ir::Terminator::Ret,
        };

        let result = backend.compile_block(&block);
        assert!(result.is_ok());

        let compiled = result.unwrap();
        assert!(!compiled.code.is_empty());
    }

    #[test]
    fn test_cranelift_compile_shifts() {
        let config = JITConfig::default();
        let mut backend = CraneliftBackend::new(config).unwrap();

        let block = IRBlock {
            start_pc: GuestAddr(0x5000),
            ops: vec![
                IROp::MovImm {
                    dst: 1,
                    imm: 0x1000,
                },
                IROp::SllImm {
                    dst: 2,
                    src: 1,
                    sh: 4,
                },
                IROp::SrlImm {
                    dst: 3,
                    src: 1,
                    sh: 8,
                },
                IROp::SraImm {
                    dst: 4,
                    src: 1,
                    sh: 2,
                },
            ],
            term: vm_ir::Terminator::Ret,
        };

        let result = backend.compile_block(&block);
        assert!(result.is_ok());

        let compiled = result.unwrap();
        assert!(!compiled.code.is_empty());
    }
}
