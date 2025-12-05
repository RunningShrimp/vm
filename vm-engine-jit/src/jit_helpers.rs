//! vm-engine-jit 通用操作助手 (Register/SIMD helper functions)
//!
//! 消除 JIT 编译器中重复的寄存器加载/存储和二元操作模式
//! 目标：减少 30% 的代码重复

use cranelift::prelude::*;

/// 寄存器操作助手
pub struct RegisterHelper;

impl RegisterHelper {
    /// 从寄存器数组中加载值（专用寄存器0返回0）
    #[inline]
    pub fn load_reg(builder: &mut FunctionBuilder, regs_ptr: Value, idx: u32) -> Value {
        if idx == 0 {
            builder.ins().iconst(types::I64, 0)
        } else {
            let offset = (idx as i32) * 8;
            builder
                .ins()
                .load(types::I64, MemFlags::trusted(), regs_ptr, offset)
        }
    }

    /// 将值存储到寄存器数组（专用寄存器0跳过写入）
    #[inline]
    pub fn store_reg(builder: &mut FunctionBuilder, regs_ptr: Value, idx: u32, val: Value) {
        if idx != 0 {
            let offset = (idx as i32) * 8;
            builder
                .ins()
                .store(MemFlags::trusted(), val, regs_ptr, offset);
        }
    }

    /// 执行二元整数操作并存储结果
    ///
    /// # 参数
    /// - `builder`: Cranelift 函数生成器
    /// - `regs_ptr`: 寄存器数组指针
    /// - `dst`: 目标寄存器索引
    /// - `src1`: 源操作数1寄存器索引
    /// - `src2`: 源操作数2寄存器索引
    /// - `op`: 操作闭包 (v1: Value, v2: Value) -> Value
    #[inline]
    pub fn binary_op<F>(
        builder: &mut FunctionBuilder,
        regs_ptr: Value,
        dst: u32,
        src1: u32,
        src2: u32,
        op: F,
    ) where
        F: FnOnce(&mut FunctionBuilder, Value, Value) -> Value,
    {
        let v1 = Self::load_reg(builder, regs_ptr, src1);
        let v2 = Self::load_reg(builder, regs_ptr, src2);
        let res = op(builder, v1, v2);
        Self::store_reg(builder, regs_ptr, dst, res);
    }

    /// 执行二元整数操作（一个立即数）并存储结果
    #[inline]
    pub fn binary_op_imm<F>(
        builder: &mut FunctionBuilder,
        regs_ptr: Value,
        dst: u32,
        src: u32,
        imm: i64,
        op: F,
    ) where
        F: FnOnce(&mut FunctionBuilder, Value, Value) -> Value,
    {
        let v = Self::load_reg(builder, regs_ptr, src);
        let imm_val = builder.ins().iconst(types::I64, imm);
        let res = op(builder, v, imm_val);
        Self::store_reg(builder, regs_ptr, dst, res);
    }

    /// 执行二元移位操作（从寄存器读取移位量）
    #[inline]
    pub fn shift_op<F>(
        builder: &mut FunctionBuilder,
        regs_ptr: Value,
        dst: u32,
        src: u32,
        shreg: u32,
        op: F,
    ) where
        F: FnOnce(&mut FunctionBuilder, Value, Value) -> Value,
    {
        let v = Self::load_reg(builder, regs_ptr, src);
        let amt = Self::load_reg(builder, regs_ptr, shreg);
        let res = op(builder, v, amt);
        Self::store_reg(builder, regs_ptr, dst, res);
    }

    /// 执行二元移位操作（立即移位量）
    #[inline]
    pub fn shift_op_imm<F>(
        builder: &mut FunctionBuilder,
        regs_ptr: Value,
        dst: u32,
        src: u32,
        sh: u32,
        op: F,
    ) where
        F: FnOnce(&mut FunctionBuilder, Value, Value) -> Value,
    {
        let v = Self::load_reg(builder, regs_ptr, src);
        let amt = builder.ins().iconst(types::I64, sh as i64);
        let res = op(builder, v, amt);
        Self::store_reg(builder, regs_ptr, dst, res);
    }

    /// 执行比较操作并存储结果（0 或 1）
    #[inline]
    pub fn compare_op<F>(
        builder: &mut FunctionBuilder,
        regs_ptr: Value,
        dst: u32,
        src1: u32,
        src2: u32,
        cmp: F,
    ) where
        F: FnOnce(&mut FunctionBuilder, Value, Value) -> Value,
    {
        let v1 = Self::load_reg(builder, regs_ptr, src1);
        let v2 = Self::load_reg(builder, regs_ptr, src2);
        let result = cmp(builder, v1, v2);
        // 比较结果转换为 i64 (0 或 1)
        let res = builder.ins().uextend(types::I64, result);
        Self::store_reg(builder, regs_ptr, dst, res);
    }

    /// 执行一元操作并存储结果
    #[inline]
    pub fn unary_op<F>(builder: &mut FunctionBuilder, regs_ptr: Value, dst: u32, src: u32, op: F)
    where
        F: FnOnce(&mut FunctionBuilder, Value) -> Value,
    {
        let v = Self::load_reg(builder, regs_ptr, src);
        let res = op(builder, v);
        Self::store_reg(builder, regs_ptr, dst, res);
    }
}

/// 浮点寄存器操作助手
pub struct FloatRegHelper;

impl FloatRegHelper {
    /// 从浮点寄存器数组中加载值
    #[inline]
    pub fn load_freg(builder: &mut FunctionBuilder, fregs_ptr: Value, idx: u32) -> Value {
        let offset = (idx as i32) * 8;
        builder
            .ins()
            .load(types::F64, MemFlags::trusted(), fregs_ptr, offset)
    }

    /// 将值存储到浮点寄存器数组
    #[inline]
    pub fn store_freg(builder: &mut FunctionBuilder, fregs_ptr: Value, idx: u32, val: Value) {
        let offset = (idx as i32) * 8;
        builder
            .ins()
            .store(MemFlags::trusted(), val, fregs_ptr, offset);
    }

    /// 执行二元浮点操作并存储结果
    #[inline]
    pub fn binary_op<F>(
        builder: &mut FunctionBuilder,
        fregs_ptr: Value,
        dst: u32,
        src1: u32,
        src2: u32,
        op: F,
    ) where
        F: FnOnce(&mut FunctionBuilder, Value, Value) -> Value,
    {
        let v1 = Self::load_freg(builder, fregs_ptr, src1);
        let v2 = Self::load_freg(builder, fregs_ptr, src2);
        let res = op(builder, v1, v2);
        Self::store_freg(builder, fregs_ptr, dst, res);
    }

    /// 执行一元浮点操作并存储结果
    #[inline]
    pub fn unary_op<F>(builder: &mut FunctionBuilder, fregs_ptr: Value, dst: u32, src: u32, op: F)
    where
        F: FnOnce(&mut FunctionBuilder, Value) -> Value,
    {
        let v = Self::load_freg(builder, fregs_ptr, src);
        let res = op(builder, v);
        Self::store_freg(builder, fregs_ptr, dst, res);
    }

    /// 执行转换操作（整数到浮点或浮点到整数）
    #[inline]
    pub fn convert_from_reg(
        builder: &mut FunctionBuilder,
        regs_ptr: Value,
        fregs_ptr: Value,
        dst_freg: u32,
        src_reg: u32,
        signed: bool,
    ) {
        let v = RegisterHelper::load_reg(builder, regs_ptr, src_reg);
        let res = if signed {
            builder.ins().fcvt_from_sint(types::F64, v)
        } else {
            builder.ins().fcvt_from_uint(types::F64, v)
        };
        Self::store_freg(builder, fregs_ptr, dst_freg, res);
    }

    /// 执行转换操作（浮点到整数）
    #[inline]
    pub fn convert_to_reg(
        builder: &mut FunctionBuilder,
        regs_ptr: Value,
        fregs_ptr: Value,
        dst_reg: u32,
        src_freg: u32,
        signed: bool,
    ) {
        let v = Self::load_freg(builder, fregs_ptr, src_freg);
        let res = if signed {
            builder.ins().fcvt_to_sint(types::I64, v)
        } else {
            builder.ins().fcvt_to_uint(types::I64, v)
        };
        RegisterHelper::store_reg(builder, regs_ptr, dst_reg, res);
    }
}

/// 内存操作助手
pub struct MemoryHelper;

impl MemoryHelper {
    /// 计算有效地址: base + offset
    #[inline]
    pub fn compute_address(builder: &mut FunctionBuilder, base: Value, offset: i64) -> Value {
        if offset == 0 {
            base
        } else {
            let offset_val = builder.ins().iconst(types::I64, offset);
            builder.ins().iadd(base, offset_val)
        }
    }

    /// 计算有效地址: base + index*scale + offset
    #[inline]
    pub fn compute_scaled_address(
        builder: &mut FunctionBuilder,
        base: Value,
        index: Value,
        scale: u8,
        offset: i64,
    ) -> Value {
        // 计算 index * scale
        let scaled = if scale == 1 {
            index
        } else {
            let scale_val = builder.ins().iconst(types::I64, scale as i64);
            builder.ins().imul(index, scale_val)
        };

        // 添加 base
        let addr = builder.ins().iadd(base, scaled);

        // 添加位移
        if offset == 0 {
            addr
        } else {
            let offset_val = builder.ins().iconst(types::I64, offset);
            builder.ins().iadd(addr, offset_val)
        }
    }

    /// 根据大小选择加载类型
    #[inline]
    pub fn load_with_size(
        builder: &mut FunctionBuilder,
        addr: Value,
        size: u8,
        flags: MemFlags,
    ) -> Value {
        match size {
            1 => builder.ins().load(types::I8, flags, addr, 0),
            2 => builder.ins().load(types::I16, flags, addr, 0),
            4 => builder.ins().load(types::I32, flags, addr, 0),
            _ => builder.ins().load(types::I64, flags, addr, 0),
        }
    }

    /// 根据大小选择存储类型
    #[inline]
    pub fn store_with_size(
        builder: &mut FunctionBuilder,
        addr: Value,
        val: Value,
        size: u8,
        flags: MemFlags,
    ) {
        let truncated = match size {
            1 => builder.ins().ireduce(types::I8, val),
            2 => builder.ins().ireduce(types::I16, val),
            4 => builder.ins().ireduce(types::I32, val),
            _ => val,
        };
        builder.ins().store(flags, truncated, addr, 0);
    }

    /// 加载并符号扩展
    #[inline]
    pub fn load_sext(
        builder: &mut FunctionBuilder,
        addr: Value,
        size: u8,
        flags: MemFlags,
    ) -> Value {
        let val = Self::load_with_size(builder, addr, size, flags);
        match size {
            1 => builder.ins().sextend(types::I64, val),
            2 => builder.ins().sextend(types::I64, val),
            4 => builder.ins().sextend(types::I64, val),
            _ => val,
        }
    }

    /// 加载并零扩展
    #[inline]
    pub fn load_zext(
        builder: &mut FunctionBuilder,
        addr: Value,
        size: u8,
        flags: MemFlags,
    ) -> Value {
        let val = Self::load_with_size(builder, addr, size, flags);
        match size {
            1 => builder.ins().uextend(types::I64, val),
            2 => builder.ins().uextend(types::I64, val),
            4 => builder.ins().uextend(types::I64, val),
            _ => val,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_helper_compilation() {
        // 这个测试只是确保代码编译正确
        // 实际的功能测试需要在完整的 JIT 环境中进行
        assert_eq!(0, 0);
    }
}
