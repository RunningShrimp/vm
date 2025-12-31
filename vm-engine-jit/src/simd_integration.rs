//! SIMD指令扩展集成
//!
//! 将vm-simd库的功能集成到JIT编译器中，提供高性能的向量运算支持。

use vm_core::{VmError, GuestAddr};
use vm_ir::{IROp, IROp::*};
use cranelift::prelude::*;
use cranelift_codegen::ir::{FuncRef, Function, InstBuilder};
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
enum SimdOperation {
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
enum VectorSize {
    Scalar64,
    Vec128,
    Vec256,
    Vec512,
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
        fregs_ptr: Value,
        vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        match op {
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
    fn compile_vec_bitop(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src1: u32,
        src2: u32,
        regs_ptr: Value,
        vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        Ok(None)
    }

    /// 编译向量一元运算
    fn compile_vec_unop(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src: u32,
        regs_ptr: Value,
        vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        Ok(None)
    }

    /// 编译向量移位运算
    fn compile_vec_shift(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src: u32,
        shift: u8,
        element_size: u8,
        regs_ptr: Value,
        vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        Ok(None)
    }

    /// 编译浮点向量二元运算
    fn compile_vec_float_binop(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src1: u32,
        src2: u32,
        fregs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        Ok(None)
    }

    /// 编译FMA操作
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
        Ok(None)
    }

    /// 编译向量比较二元操作
    fn compile_vec_cmp_binop(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src1: u32,
        src2: u32,
        element_size: u8,
        regs_ptr: Value,
        vec_regs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        Ok(None)
    }

  
    /// 编译向量浮点一元操作
    fn compile_vec_float_unop(
        &mut self,
        builder: &mut FunctionBuilder,
        operation: SimdOperation,
        dst: u32,
        src: u32,
        fregs_ptr: Value,
    ) -> Result<Option<Value>, VmError> {
        Ok(None)
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
