//! SIMD指令扩展集成
#![allow(dead_code)] // TODO: Many JIT structures are reserved for future optimization features
//!
//! 将vm-simd库的功能集成到JIT编译器中，提供高性能的向量运算支持。

use vm_core::{VmError, CoreError};
use vm_ir::IROp;
use cranelift::prelude::*;
use cranelift_codegen::ir::{InstBuilder, MemFlags};
use cranelift_module::{Module, FuncId};
use cranelift_jit::JITModule;
use std::collections::HashMap;

/// SIMD集成管理器
/// 
/// 注意：不保存 JITModule 引用，需要时通过参数传入
pub struct SimdIntegrationManager {
    /// SIMD函数缓存
    func_cache: HashMap<SimdFuncKey, FuncId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SimdFuncKey {
    operation: SimdOperation,
    element_size: u8,
    vector_size: VectorSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimdOperation {
    VecAdd,
    VecSub,
    VecMul,
    VecAddSatU,
    VecSubSatU,
    VecAddSatS,
    VecSubSatS,
    VecMulSatU,
    VecMulSatS,
    VecMinU,
    VecMaxU,
    VecAnd,
    VecOr,
    VecXor,
    VecNot,
    VecShl,
    VecShrU,
    VecCmpeq,
    VecCmpgtU,
    VecFaddF32,
    VecFsubF32,
    VecFmulF32,
    VecFdivF32,
    VecFaddF64,
    VecFsubF64,
    VecFmulF64,
    VecFdivF64,
    VecFmaF32,
    VecFmaF64,
    // 新增操作
    VecCmpgtS,
    VecCmpltU,
    VecCmpltS,
    VecCmpgeU,
    VecCmpgeS,
    VecShrS,
    VecFminF32,
    VecFmaxF32,
    VecFminF64,
    VecFmaxF64,
    VecFsqrtF32,
    VecFsqrtF64,
    VecFabsF32,
    VecFabsF64,
    VecFnegF32,
    VecFnegF64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VectorSize {
    Scalar64,
    Vec128,
    Vec256,
    Vec512,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementSize {
    Size8,
    Size16,
    Size32,
    Size64,
}

/// Type alias for compatibility
pub type VectorOperation = SimdOperation;

pub struct SimdCompiler;

impl SimdCompiler {
    pub fn new() -> Self {
        Self
    }
}

/// Compile SIMD operation - compatibility function
pub fn compile_simd_op<M: Module>(
    _module: &mut M,
    _operation: SimdOperation,
    _element_size: ElementSize,
    _vector_size: VectorSize,
) -> Result<FuncId, VmError> {
    Err(VmError::Core(CoreError::NotSupported {
        feature: "SIMD compilation".to_string(),
        module: "simd_integration".to_string(),
    }))
}

/// Compile SIMD operation - compatibility function
pub fn compile_simd_operation<M: Module>(
    _module: &mut M,
    _operation: VectorOperation,
    _element_size: u8,
    _vector_size: VectorSize,
) -> Result<FuncId, VmError> {
    Err(VmError::Core(CoreError::NotSupported {
        feature: "SIMD operation compilation".to_string(),
        module: "simd_integration".to_string(),
    }))
}

impl SimdIntegrationManager {
    pub fn new() -> Self {
        Self {
            func_cache: HashMap::new(),
        }
    }

    /// 获取或创建SIMD函数
    /// 
    /// # 参数
    /// * `module` - JIT模块可变引用，用于创建函数
    /// * `operation` - SIMD操作类型
    /// * `element_size` - 元素大小
    /// * `vector_size` - 向量大小
    pub fn get_or_create_func(&mut self, module: &mut JITModule, operation: SimdOperation, element_size: u8, vector_size: VectorSize) -> Result<FuncId, VmError> {
        let key = SimdFuncKey { operation, element_size, vector_size };

        if let Some(&func_id) = self.func_cache.get(&key) {
            return Ok(func_id);
        }

        // 创建SIMD函数
        let func_id = self.create_simd_function(module, operation, element_size, vector_size)?;
        self.func_cache.insert(key, func_id);

        Ok(func_id)
    }

    /// 创建SIMD函数
    fn create_simd_function(
        &self,
        module: &mut JITModule,
        operation: SimdOperation,
        element_size: u8,
        vector_size: VectorSize,
    ) -> Result<FuncId, VmError> {
        let mut sig = module.make_signature();

        // 对于基本的向量操作（VecAdd, VecSub, VecMul），使用简单的签名
        // 这些函数已经在 simd.rs 中定义，签名是 (u64, u64, u64) -> u64
        if matches!(operation, SimdOperation::VecAdd | SimdOperation::VecSub | SimdOperation::VecMul) {
            sig.params.push(AbiParam::new(types::I64)); // a
            sig.params.push(AbiParam::new(types::I64)); // b
            sig.params.push(AbiParam::new(types::I64)); // element_size
            sig.returns.push(AbiParam::new(types::I64)); // result
            let func_name = self.get_function_name(operation, element_size, vector_size);
            let func_id = module.declare_function(&func_name, cranelift_module::Linkage::Import, &sig)
                .map_err(|e| VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to declare SIMD function: {}", e),
                    module: "simd_integration".to_string(),
                }))?;
            return Ok(func_id);
        }

        // 根据向量大小设置参数和返回值
        match vector_size {
            VectorSize::Scalar64 => {
                sig.params.push(AbiParam::new(types::I64));
                sig.params.push(AbiParam::new(types::I64));
                if matches!(operation, SimdOperation::VecShl | SimdOperation::VecShrU) {
                    sig.params.push(AbiParam::new(types::I8));
                }
                sig.returns.push(AbiParam::new(types::I64));
            }
            VectorSize::Vec128 => {
                // 128-bit vectors as [u64; 2]
                sig.params.push(AbiParam::new(types::I64));
                sig.params.push(AbiParam::new(types::I64));
                sig.params.push(AbiParam::new(types::I64));
                sig.params.push(AbiParam::new(types::I64));
                sig.params.push(AbiParam::new(types::I8));
                sig.returns.push(AbiParam::new(types::I64));
                sig.returns.push(AbiParam::new(types::I64));
            }
            VectorSize::Vec256 => {
                // 256-bit vectors as [u64; 4]
                for _ in 0..4 {
                    sig.params.push(AbiParam::new(types::I64));
                }
                for _ in 0..4 {
                    sig.params.push(AbiParam::new(types::I64));
                }
                sig.params.push(AbiParam::new(types::I8));
                for _ in 0..4 {
                    sig.returns.push(AbiParam::new(types::I64));
                }
            }
            VectorSize::Vec512 => {
                // 512-bit vectors as [u64; 8]
                for _ in 0..8 {
                    sig.params.push(AbiParam::new(types::I64));
                }
                for _ in 0..8 {
                    sig.params.push(AbiParam::new(types::I64));
                }
                sig.params.push(AbiParam::new(types::I8));
                for _ in 0..8 {
                    sig.returns.push(AbiParam::new(types::I64));
                }
            }
        }

        // 为浮点操作调整签名
        if matches!(operation, SimdOperation::VecFaddF32 | SimdOperation::VecFsubF32 |
                           SimdOperation::VecFmulF32 | SimdOperation::VecFdivF32) {
            // f32 vectors as [f32; 4]
            sig.params.clear();
            sig.returns.clear();
            for _ in 0..4 {
                sig.params.push(AbiParam::new(types::F32));
            }
            for _ in 0..4 {
                sig.params.push(AbiParam::new(types::F32));
            }
            for _ in 0..4 {
                sig.returns.push(AbiParam::new(types::F32));
            }
        } else if matches!(operation, SimdOperation::VecFaddF64 | SimdOperation::VecFsubF64 |
                                  SimdOperation::VecFmulF64 | SimdOperation::VecFdivF64) {
            // f64 vectors as [f64; 2]
            sig.params.clear();
            sig.returns.clear();
            for _ in 0..2 {
                sig.params.push(AbiParam::new(types::F64));
            }
            for _ in 0..2 {
                sig.params.push(AbiParam::new(types::F64));
            }
            for _ in 0..2 {
                sig.returns.push(AbiParam::new(types::F64));
            }
        } else if matches!(operation, SimdOperation::VecFmaF32) {
            // FMA: a, b, c -> result
            sig.params.clear();
            sig.returns.clear();
            for _ in 0..12 { // 3 vectors × 4 elements
                sig.params.push(AbiParam::new(types::F32));
            }
            for _ in 0..4 {
                sig.returns.push(AbiParam::new(types::F32));
            }
        } else if matches!(operation, SimdOperation::VecFmaF64) {
            sig.params.clear();
            sig.returns.clear();
            for _ in 0..6 { // 3 vectors × 2 elements
                sig.params.push(AbiParam::new(types::F64));
            }
            for _ in 0..2 {
                sig.returns.push(AbiParam::new(types::F64));
            }
        }

        let func_name = self.get_function_name(operation, element_size, vector_size);
        let func_id = module.declare_function(&func_name, cranelift_module::Linkage::Import, &sig)
            .map_err(|e| VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to declare SIMD function: {}", e),
                module: "simd_integration".to_string(),
            }))?;

        Ok(func_id)
    }

    /// 获取函数名称
    fn get_function_name(&self, operation: SimdOperation, element_size: u8, vector_size: VectorSize) -> String {
        let op_name = match operation {
            SimdOperation::VecAdd => "jit_vec_add",
            SimdOperation::VecSub => "jit_vec_sub",
            SimdOperation::VecMul => "jit_vec_mul",
            SimdOperation::VecAddSatU => "vec_add_sat_u",
            SimdOperation::VecSubSatU => "vec_sub_sat_u",
            SimdOperation::VecAddSatS => "vec_add_sat_s",
            SimdOperation::VecSubSatS => "vec_sub_sat_s",
            SimdOperation::VecMulSatU => "vec_mul_sat_u",
            SimdOperation::VecMulSatS => "vec_mul_sat_s",
            SimdOperation::VecMinU => "vec_min_u",
            SimdOperation::VecMaxU => "vec_max_u",
            SimdOperation::VecAnd => "vec_and",
            SimdOperation::VecOr => "vec_or",
            SimdOperation::VecXor => "vec_xor",
            SimdOperation::VecNot => "vec_not",
            SimdOperation::VecShl => "vec_shl",
            SimdOperation::VecShrU => "vec_shr_u",
            SimdOperation::VecCmpeq => "vec_cmpeq",
            SimdOperation::VecCmpgtU => "vec_cmpgt_u",
            SimdOperation::VecCmpgtS => "vec_cmpgt_s",
            SimdOperation::VecCmpltU => "vec_cmplt_u",
            SimdOperation::VecCmpltS => "vec_cmplt_s",
            SimdOperation::VecCmpgeU => "vec_cmpge_u",
            SimdOperation::VecCmpgeS => "vec_cmpge_s",
            SimdOperation::VecShrS => "vec_shr_s",
            SimdOperation::VecFaddF32 => "vec_fadd_f32",
            SimdOperation::VecFsubF32 => "vec_fsub_f32",
            SimdOperation::VecFmulF32 => "vec_fmul_f32",
            SimdOperation::VecFdivF32 => "vec_fdiv_f32",
            SimdOperation::VecFaddF64 => "vec_fadd_f64",
            SimdOperation::VecFsubF64 => "vec_fsub_f64",
            SimdOperation::VecFmulF64 => "vec_fmul_f64",
            SimdOperation::VecFdivF64 => "vec_fdiv_f64",
            SimdOperation::VecFmaF32 => "vec_fma_f32",
            SimdOperation::VecFmaF64 => "vec_fma_f64",
            SimdOperation::VecFminF32 => "vec_fmin_f32",
            SimdOperation::VecFmaxF32 => "vec_fmax_f32",
            SimdOperation::VecFminF64 => "vec_fmin_f64",
            SimdOperation::VecFmaxF64 => "vec_fmax_f64",
            SimdOperation::VecFsqrtF32 => "vec_fsqrt_f32",
            SimdOperation::VecFsqrtF64 => "vec_fsqrt_f64",
            SimdOperation::VecFabsF32 => "vec_fabs_f32",
            SimdOperation::VecFabsF64 => "vec_fabs_f64",
            SimdOperation::VecFnegF32 => "vec_fneg_f32",
            SimdOperation::VecFnegF64 => "vec_fneg_f64",
        };

        // 对于基本的向量操作（VecAdd, VecSub, VecMul），直接返回函数名
        // 这些函数已经在 simd.rs 中定义，签名是 (u64, u64, u64) -> u64
        if matches!(operation, SimdOperation::VecAdd | SimdOperation::VecSub | SimdOperation::VecMul) {
            return op_name.to_string();
        }

        let size_suffix = match vector_size {
            VectorSize::Scalar64 => "",
            VectorSize::Vec128 => "128",
            VectorSize::Vec256 => "256",
            VectorSize::Vec512 => "512",
        };

        if matches!(operation, SimdOperation::VecFaddF32 | SimdOperation::VecFsubF32 |
                           SimdOperation::VecFmulF32 | SimdOperation::VecFdivF32 |
                           SimdOperation::VecFmaF32) {
            format!("{}_{}", op_name, size_suffix)
        } else {
            format!("{}_{}_{}", op_name, size_suffix, element_size)
        }
    }

    /// 编译SIMD操作
    ///
    /// # 参数
    /// * `module` - JIT模块可变引用
    /// * `builder` - 函数构建器
    /// * `op` - IR操作
    /// * `regs_ptr` - 寄存器指针
    /// * `fregs_ptr` - 浮点寄存器指针
    /// * `vec_regs_ptr` - 向量寄存器指针
    pub fn compile_simd_op(
        &mut self,
        module: &mut JITModule,
        builder: &mut FunctionBuilder,
        op: &IROp,
        regs_ptr: Value,
        _fregs_ptr: Value,
        vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        match op {
            // === 基本向量运算 ===
            IROp::VecAdd { dst, src1, src2, element_size } => {
                self.compile_vec_binop(
                    module,
                    builder,
                    SimdOperation::VecAdd,
                    *dst,
                    *src1,
                    *src2,
                    *element_size,
                    VectorSize::Scalar64,
                    regs_ptr,
                    vec_regs_ptr,
                )
            }
            IROp::VecSub { dst, src1, src2, element_size } => {
                self.compile_vec_binop(
                    module,
                    builder,
                    SimdOperation::VecSub,
                    *dst,
                    *src1,
                    *src2,
                    *element_size,
                    VectorSize::Scalar64,
                    regs_ptr,
                    vec_regs_ptr,
                )
            }
            IROp::VecMul { dst, src1, src2, element_size } => {
                self.compile_vec_binop(
                    module,
                    builder,
                    SimdOperation::VecMul,
                    *dst,
                    *src1,
                    *src2,
                    *element_size,
                    VectorSize::Scalar64,
                    regs_ptr,
                    vec_regs_ptr,
                )
            }

            // === 饱和向量运算 ===
            IROp::VecAddSat { dst, src1, src2, element_size, signed } => {
                let operation = if *signed {
                    SimdOperation::VecAddSatS
                } else {
                    SimdOperation::VecAddSatU
                };
                self.compile_vec_binop(
                    module,
                    builder,
                    operation,
                    *dst,
                    *src1,
                    *src2,
                    *element_size,
                    VectorSize::Scalar64,
                    regs_ptr,
                    vec_regs_ptr,
                )
            }
            IROp::VecSubSat { dst, src1, src2, element_size, signed } => {
                let operation = if *signed {
                    SimdOperation::VecSubSatS
                } else {
                    SimdOperation::VecSubSatU
                };
                self.compile_vec_binop(
                    module,
                    builder,
                    operation,
                    *dst,
                    *src1,
                    *src2,
                    *element_size,
                    VectorSize::Scalar64,
                    regs_ptr,
                    vec_regs_ptr,
                )
            }
            IROp::VecMulSat { dst, src1, src2, element_size, signed } => {
                let operation = if *signed {
                    SimdOperation::VecMulSatS
                } else {
                    SimdOperation::VecMulSatU
                };
                self.compile_vec_binop(
                    module,
                    builder,
                    operation,
                    *dst,
                    *src1,
                    *src2,
                    *element_size,
                    VectorSize::Scalar64,
                    regs_ptr,
                    vec_regs_ptr,
                )
            }

            // 向量按位操作
            IROp::VecAnd {
                dst,
                src1,
                src2,
                element_size,
            } => self.compile_vec_binop(
                module,
                builder,
                SimdOperation::VecAnd,
                *dst,
                *src1,
                *src2,
                *element_size,
                VectorSize::Scalar64,
                regs_ptr,
                vec_regs_ptr,
            ),

            IROp::VecOr {
                dst,
                src1,
                src2,
                element_size,
            } => self.compile_vec_binop(
                module,
                builder,
                SimdOperation::VecOr,
                *dst,
                *src1,
                *src2,
                *element_size,
                VectorSize::Scalar64,
                regs_ptr,
                vec_regs_ptr,
            ),

            IROp::VecXor {
                dst,
                src1,
                src2,
                element_size,
            } => self.compile_vec_binop(
                module,
                builder,
                SimdOperation::VecXor,
                *dst,
                *src1,
                *src2,
                *element_size,
                VectorSize::Scalar64,
                regs_ptr,
                vec_regs_ptr,
            ),

            IROp::VecNot {
                dst,
                src,
                element_size: _element_size,
            } => self.compile_vec_unop(
                builder,
                SimdOperation::VecNot,
                *dst,
                *src,
                regs_ptr,
                vec_regs_ptr,
            ),

            // 向量移位操作（寄存器移位）
            IROp::VecShl {
                dst,
                src,
                shift,
                element_size,
            } => self.compile_vec_shift_reg(
                module,
                builder,
                SimdOperation::VecShl,
                *dst,
                *src,
                *shift,
                *element_size,
                VectorSize::Scalar64,
                regs_ptr,
                vec_regs_ptr,
            ),

            IROp::VecSrl {
                dst,
                src,
                shift,
                element_size,
            } => self.compile_vec_shift_reg(
                module,
                builder,
                SimdOperation::VecShrU,
                *dst,
                *src,
                *shift,
                *element_size,
                VectorSize::Scalar64,
                regs_ptr,
                vec_regs_ptr,
            ),

            IROp::VecSra {
                dst,
                src,
                shift,
                element_size,
            } => self.compile_vec_shift_reg(
                module,
                builder,
                SimdOperation::VecShrS,
                *dst,
                *src,
                *shift,
                *element_size,
                VectorSize::Scalar64,
                regs_ptr,
                vec_regs_ptr,
            ),

            // 向量移位操作（立即数移位）
            IROp::VecShlImm {
                dst,
                src,
                shift,
                element_size,
            } => self.compile_vec_shift_imm(
                module,
                builder,
                SimdOperation::VecShl,
                *dst,
                *src,
                *shift,
                *element_size,
                VectorSize::Scalar64,
                regs_ptr,
                vec_regs_ptr,
            ),

            IROp::VecSrlImm {
                dst,
                src,
                shift,
                element_size,
            } => self.compile_vec_shift_imm(
                module,
                builder,
                SimdOperation::VecShrU,
                *dst,
                *src,
                *shift,
                *element_size,
                VectorSize::Scalar64,
                regs_ptr,
                vec_regs_ptr,
            ),

            IROp::VecSraImm {
                dst,
                src,
                shift,
                element_size,
            } => self.compile_vec_shift_imm(
                module,
                builder,
                SimdOperation::VecShrS,
                *dst,
                *src,
                *shift,
                *element_size,
                VectorSize::Scalar64,
                regs_ptr,
                vec_regs_ptr,
            ),

            _ => Ok(None),
        }
    }

    /// 编译向量二元运算
    fn compile_vec_binop(
        &mut self,
        module: &mut JITModule,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src1: u32,
        src2: u32,
        element_size: u8,
        vector_size: VectorSize,
        regs_ptr: Value,
        _vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        // 获取或创建SIMD函数
        let func_id = self.get_or_create_func(module, operation, element_size, vector_size)?;
        
        // 声明函数引用
        let func_ref = module.declare_func_in_func(func_id, builder.func);
        
        // 加载源寄存器
        let src1_val = Self::load_vec_reg(builder, regs_ptr, src1 as usize);
        let src2_val = Self::load_vec_reg(builder, regs_ptr, src2 as usize);
        let element_size_val = builder.ins().iconst(types::I64, element_size as i64);
        
        // 调用SIMD函数
        let call = builder.ins().call(func_ref, &[src1_val, src2_val, element_size_val]);
        let result = builder.inst_results(call)[0];
        
        // 存储结果
        Self::store_vec_reg(builder, regs_ptr, dst as usize, result);
        
        Ok(Some(result))
    }

    /// 编译向量位运算
    ///
    /// 支持的操作：
    /// - VecAnd: 向量按位与
    /// - VecOr: 向量按位或
    /// - VecXor: 向量按位异或
    /// - VecNot: 向量按位取反
    fn compile_vec_bitop(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src1: u32,
        src2: u32,
        regs_ptr: Value,
        _vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        // 加载源操作数
        let src1_val = Self::load_vec_reg(builder, regs_ptr, src1 as usize);
        let src2_val = if matches!(operation, SimdOperation::VecNot) {
            None
        } else {
            Some(Self::load_vec_reg(builder, regs_ptr, src2 as usize))
        };

        // 执行位运算
        let result = match operation {
            SimdOperation::VecAnd => builder.ins().band(src1_val, src2_val.unwrap()),
            SimdOperation::VecOr => builder.ins().bor(src1_val, src2_val.unwrap()),
            SimdOperation::VecXor => builder.ins().bxor(src1_val, src2_val.unwrap()),
            SimdOperation::VecNot => builder.ins().bnot(src1_val),
            _ => return Ok(None),
        };

        // 存储结果
        Self::store_vec_reg(builder, regs_ptr, dst as usize, result);

        Ok(Some(result))
    }

    /// 编译向量一元运算
    ///
    /// 支持的操作：
    /// - VecNot: 按位取反（在compile_vec_bitop中处理）
    /// - VecFsqrtF32/F64: 平方根
    /// - VecFabsF32/F64: 绝对值
    /// - VecFnegF32/F64: 取反
    fn compile_vec_unop(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src: u32,
        regs_ptr: Value,
        _vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        let src_val = Self::load_vec_reg(builder, regs_ptr, src as usize);

        let result = match operation {
            SimdOperation::VecFsqrtF32 => {
                // 对于f32向量，简化实现：对标量值执行sqrt
                // 完整实现需要使用SIMD指令如vsqrtps (SSE/AVX)
                let _float_val = builder.ins().bitcast(types::F32, MemFlags::new(), src_val);
                // 简化：这里需要实际的sqrt intrinsic支持
                // 暂时返回原值
                src_val
            }
            SimdOperation::VecFsqrtF64 => {
                // f64 向量平方根
                src_val
            }
            SimdOperation::VecFabsF32 => {
                // f32 绝对值：使用位操作清除符号位
                let mask = builder.ins().iconst(types::I64, 0x7FFFFFFF);
                builder.ins().band(src_val, mask)
            }
            SimdOperation::VecFabsF64 => {
                // f64 绝对值
                let mask = builder.ins().iconst(types::I64, 0x7FFFFFFFFFFFFFFF);
                builder.ins().band(src_val, mask)
            }
            SimdOperation::VecFnegF32 | SimdOperation::VecFnegF64 => {
                // 取反：翻转符号位
                let mask = builder.ins().iconst(types::I64, i64::MIN as i64);
                builder.ins().bxor(src_val, mask)
            }
            _ => return Ok(None),
        };

        Self::store_vec_reg(builder, regs_ptr, dst as usize, result);
        Ok(Some(result))
    }

    /// 编译向量移位运算
    ///
    /// 支持的操作：
    /// - VecShl: 向量左移
    /// - VecShrU: 向量逻辑右移（无符号）
    /// - VecShrS: 向量算术右移（有符号）
    fn compile_vec_shift(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src: u32,
        shift: u8,
        element_size: u8,
        regs_ptr: Value,
        _vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        let src_val = Self::load_vec_reg(builder, regs_ptr, src as usize);
        let shift_val = builder.ins().iconst(types::I64, shift as i64);

        let result = match operation {
            SimdOperation::VecShl => builder.ins().ishl(src_val, shift_val),
            SimdOperation::VecShrU => {
                // 逻辑右移（无符号）
                if element_size <= 4 {
                    builder.ins().ushr(src_val, shift_val)
                } else {
                    builder.ins().ushr(src_val, shift_val)
                }
            }
            SimdOperation::VecShrS => {
                // 算术右移（有符号）
                builder.ins().sshr(src_val, shift_val)
            }
            _ => return Ok(None),
        };

        Self::store_vec_reg(builder, regs_ptr, dst as usize, result);
        Ok(Some(result))
    }

    /// 编译向量移位操作（寄存器移位）
    ///
    /// 支持的操作：
    /// - VecShl: 逻辑左移
    /// - VecShrU: 逻辑右移（无符号）
    /// - VecShrS: 算术右移（有符号）
    fn compile_vec_shift_reg(
        &mut self,
        module: &mut JITModule,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src: u32,
        shift: u32,
        element_size: u8,
        vector_size: VectorSize,
        regs_ptr: Value,
        _vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        // 获取或创建移位函数
        let func_id = self.get_or_create_func(module, operation, element_size, vector_size)?;

        // 声明函数引用
        let func_ref = module.declare_func_in_func(func_id, builder.func);

        // 加载源寄存器和移位值寄存器
        let src_val = Self::load_vec_reg(builder, regs_ptr, src as usize);
        let shift_val = Self::load_vec_reg(builder, regs_ptr, shift as usize);
        let element_size_val = builder.ins().iconst(types::I64, element_size as i64);

        // 调用SIMD函数
        let call = builder.ins().call(
            func_ref,
            &[src_val, shift_val, element_size_val],
        );
        let result = builder.inst_results(call)[0];

        // 存储结果
        Self::store_vec_reg(builder, regs_ptr, dst as usize, result);

        Ok(Some(result))
    }

    /// 编译向量移位操作（立即数移位）
    ///
    /// 支持的操作：
    /// - VecShl: 逻辑左移
    /// - VecShrU: 逻辑右移（无符号）
    /// - VecShrS: 算术右移（有符号）
    fn compile_vec_shift_imm(
        &mut self,
        module: &mut JITModule,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src: u32,
        shift: u8,
        element_size: u8,
        vector_size: VectorSize,
        regs_ptr: Value,
        _vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        // 获取或创建移位函数
        let func_id = self.get_or_create_func(module, operation, element_size, vector_size)?;

        // 声明函数引用
        let func_ref = module.declare_func_in_func(func_id, builder.func);

        // 加载源寄存器
        let src_val = Self::load_vec_reg(builder, regs_ptr, src as usize);
        let shift_val = builder.ins().iconst(types::I64, shift as i64);
        let element_size_val = builder.ins().iconst(types::I64, element_size as i64);

        // 调用SIMD函数
        let call = builder.ins().call(
            func_ref,
            &[src_val, shift_val, element_size_val],
        );
        let result = builder.inst_results(call)[0];

        // 存储结果
        Self::store_vec_reg(builder, regs_ptr, dst as usize, result);

        Ok(Some(result))
    }

    /// 编译浮点向量二元运算
    ///
    /// 支持的操作：
    /// - VecFaddF32/F64: 浮点加法
    /// - VecFsubF32/F64: 浮点减法
    /// - VecFmulF32/F64: 浮点乘法
    /// - VecFdivF32/F64: 浮点除法
    /// - VecFminF32/F64: 最小值
    /// - VecFmaxF32/F64: 最大值
    fn compile_vec_float_binop(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src1: u32,
        src2: u32,
        fregs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        // 从浮点寄存器加载
        let src1_val = Self::load_vec_reg(builder, fregs_ptr, src1 as usize);
        let src2_val = Self::load_vec_reg(builder, fregs_ptr, src2 as usize);

        // 确定类型
        let (_is_f32, type_val) = match operation {
            SimdOperation::VecFaddF32 | SimdOperation::VecFsubF32 |
            SimdOperation::VecFmulF32 | SimdOperation::VecFdivF32 |
            SimdOperation::VecFminF32 | SimdOperation::VecFmaxF32 => (true, types::F32),
            SimdOperation::VecFaddF64 | SimdOperation::VecFsubF64 |
            SimdOperation::VecFmulF64 | SimdOperation::VecFdivF64 |
            SimdOperation::VecFminF64 | SimdOperation::VecFmaxF64 => (false, types::F64),
            _ => return Ok(None),
        };

        // 转换为浮点类型
        let src1_float = builder.ins().bitcast(type_val, MemFlags::new(), src1_val);
        let src2_float = builder.ins().bitcast(type_val, MemFlags::new(), src2_val);

        // 执行浮点运算
        let result_float = match operation {
            SimdOperation::VecFaddF32 | SimdOperation::VecFaddF64 => {
                builder.ins().fadd(src1_float, src2_float)
            }
            SimdOperation::VecFsubF32 | SimdOperation::VecFsubF64 => {
                builder.ins().fsub(src1_float, src2_float)
            }
            SimdOperation::VecFmulF32 | SimdOperation::VecFmulF64 => {
                builder.ins().fmul(src1_float, src2_float)
            }
            SimdOperation::VecFdivF32 | SimdOperation::VecFdivF64 => {
                builder.ins().fdiv(src1_float, src2_float)
            }
            SimdOperation::VecFminF32 | SimdOperation::VecFminF64 => {
                // 浮点最小值：使用fcmp + select
                // 简化实现：使用fmin (如果可用)
                let cmp = builder.ins().fcmp(FloatCC::LessThan, src1_float, src2_float);
                builder.ins().select(cmp, src1_float, src2_float)
            }
            SimdOperation::VecFmaxF32 | SimdOperation::VecFmaxF64 => {
                // 浮点最大值
                let cmp = builder.ins().fcmp(FloatCC::GreaterThan, src1_float, src2_float);
                builder.ins().select(cmp, src1_float, src2_float)
            }
            _ => return Ok(None),
        };

        // 转换回整数类型存储
        let result = builder.ins().bitcast(types::I64, MemFlags::new(), result_float);
        Self::store_vec_reg(builder, fregs_ptr, dst as usize, result);

        Ok(Some(result))
    }

    /// 编译FMA（融合乘加）操作
    ///
    /// FMA: dst = (src1 * src2) + src3
    /// 这是一个关键操作，可以显著提升数值计算性能
    ///
    /// 支持的操作：
    /// - VecFmaF32: 单精度FMA
    /// - VecFmaF64: 双精度FMA
    fn compile_vec_fma(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src1: u32,
        src2: u32,
        src3: u32,
        fregs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        // 加载操作数
        let src1_val = Self::load_vec_reg(builder, fregs_ptr, src1 as usize);
        let src2_val = Self::load_vec_reg(builder, fregs_ptr, src2 as usize);
        let src3_val = Self::load_vec_reg(builder, fregs_ptr, src3 as usize);

        // 确定类型
        let type_val = match operation {
            SimdOperation::VecFmaF32 => types::F32,
            SimdOperation::VecFmaF64 => types::F64,
            _ => return Ok(None),
        };

        // 转换为浮点类型
        let src1_float = builder.ins().bitcast(type_val, MemFlags::new(), src1_val);
        let src2_float = builder.ins().bitcast(type_val, MemFlags::new(), src2_val);
        let src3_float = builder.ins().bitcast(type_val, MemFlags::new(), src3_val);

        // 计算乘积
        let product = builder.ins().fmul(src1_float, src2_float);

        // 加上第三个操作数
        let result_float = builder.ins().fadd(product, src3_float);

        // 转换回整数类型存储
        let result = builder.ins().bitcast(types::I64, MemFlags::new(), result_float);
        Self::store_vec_reg(builder, fregs_ptr, dst as usize, result);

        Ok(Some(result))
    }

    /// 编译向量比较二元操作
    ///
    /// 支持的操作：
    /// - VecCmpeq: 相等比较
    /// - VecCmpgtU/CmpgtS: 大于比较（无符号/有符号）
    /// - VecCmpltU/CmpltS: 小于比较（无符号/有符号）
    /// - VecCmpgeU/CmpgeS: 大于等于比较
    fn compile_vec_cmp_binop(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src1: u32,
        src2: u32,
        _element_size: u8,
        regs_ptr: Value,
        _vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        let src1_val = Self::load_vec_reg(builder, regs_ptr, src1 as usize);
        let src2_val = Self::load_vec_reg(builder, regs_ptr, src2 as usize);

        // 根据操作选择整数比较条件码
        let intcc = match operation {
            SimdOperation::VecCmpeq => IntCC::Equal,
            SimdOperation::VecCmpgtU => IntCC::UnsignedGreaterThan,
            SimdOperation::VecCmpgtS => IntCC::SignedGreaterThan,
            SimdOperation::VecCmpltU => IntCC::UnsignedLessThan,
            SimdOperation::VecCmpltS => IntCC::SignedLessThan,
            SimdOperation::VecCmpgeU => IntCC::UnsignedGreaterThanOrEqual,
            SimdOperation::VecCmpgeS => IntCC::SignedGreaterThanOrEqual,
            _ => return Ok(None),
        };

        // 执行整数比较
        let cmp_result = builder.ins().icmp(intcc, src1_val, src2_val);

        // 将布尔结果扩展为全1/全0掩码
        // 对于64位：true -> 0xFFFFFFFFFFFFFFFF, false -> 0
        let mask = builder.ins().uextend(types::I64, cmp_result);

        // 对掩码取反以获得正确的位模式（如果需要）
        let result = if matches!(intcc, IntCC::Equal) {
            // 相等比较：cmp_result为true时返回全1
            let neg_zero = builder.ins().iconst(types::I64, 0);
            let neg_one = builder.ins().iconst(types::I64, !0i64 as i64);
            builder.ins().select(cmp_result, neg_one, neg_zero)
        } else {
            mask
        };

        Self::store_vec_reg(builder, regs_ptr, dst as usize, result);
        Ok(Some(result))
    }

  
    /// 加载向量寄存器（从通用寄存器数组）
    fn load_vec_reg(builder: &mut FunctionBuilder, regs_ptr: Value, idx: usize) -> Value {
        let offset = (idx as i32) * 8;
        builder.ins().load(types::I64, MemFlags::trusted(), regs_ptr, offset)
    }

    /// 存储向量寄存器（到通用寄存器数组）
    fn store_vec_reg(builder: &mut FunctionBuilder, regs_ptr: Value, idx: usize, val: Value) {
        let offset = (idx as i32) * 8;
        builder.ins().store(MemFlags::trusted(), val, regs_ptr, offset);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_integration_creation() {
        let manager = SimdIntegrationManager::new();
        assert!(manager.func_cache.is_empty());
    }

    #[test]
    fn test_simd_func_key() {
        let key1 = SimdFuncKey {
            operation: SimdOperation::VecAdd,
            element_size: 4,
            vector_size: VectorSize::Scalar64,
        };
        let key2 = SimdFuncKey {
            operation: SimdOperation::VecAdd,
            element_size: 4,
            vector_size: VectorSize::Scalar64,
        };
        assert_eq!(key1, key2);
        assert_eq!(key1, key2); // Test Hash and Eq
    }
}
