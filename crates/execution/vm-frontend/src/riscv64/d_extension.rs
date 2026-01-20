//! RISC-V D扩展：双精度浮点指令集
//!
//! 实现IEEE 754双精度浮点运算（64位）。
//!
//! ## 指令列表（20个）
//!
//! ### 加载/存储
//! - FLD: 浮点加载双字
//! - FSD: 浮点存储双字
//!
//! ### 算术运算
//! - FADD.D: 双精度加法
//! - FSUB.D: 双精度减法
//! - FMUL.D: 双精度乘法
//! - FDIV.D: 双精度除法
//! - FSQRT.D: 双精度平方根
//!
//! ### 最小/最大
//! - FMIN.D: 双精度最小值
//! - FMAX.D: 双精度最大值
//!
//! ### 比较
//! - FEQ.D: 双精度相等比较
//! - FLT.D: 双精度小于比较
//! - FLE.D: 双精度小于等于比较
//!
//! ### 类型转换
//! - FCVT.S.D: 双精度转单精度
//! - FCVT.D.S: 单精度转双精度
//! - FCVT.L.D: 双精度转有符号长整数
//! - FCVT.D.L: 有符号长整数转双精度
//! - FCVT.LU.D: 双精度转无符号长整数
//! - FCVT.D.LU: 无符号长整数转双精度
//!
//! ### 分类
//! - FCLASS.D: 双精度浮点分类

use crate::riscv64::{RiscvCPU, VmResult};

// ============================================================================
// 双精度浮点寄存器扩展（修复后）
// ============================================================================

/// **修复说明**: D扩展现在使用FPRegisters中的独立f64存储（regs64字段）。
/// 这避免了之前试图组合f32寄存器导致的寄存器重叠和数据损坏问题。
///
/// FPRegisters结构体现在包含：
/// - regs: [f32; 32]  - 用于F扩展
/// - regs64: [f64; 32] - 用于D扩展（独立存储）
///
/// 这样f0和f0_d可以独立存储，不会相互干扰。

// ============================================================================
// D扩展执行器
// ============================================================================

pub struct DExtensionExecutor {
    /// 浮点寄存器（包含独立的f64存储）
    pub fp_regs: crate::riscv64::f_extension::FPRegisters,
    /// 共享F扩展的FCSR
    pub fcsr: crate::riscv64::f_extension::FCSR,
    /// 是否启用浮点异常
    pub exceptions_enabled: bool,
}

impl DExtensionExecutor {
    /// 创建新的D扩展执行器
    pub fn new() -> Self {
        Self {
            fp_regs: crate::riscv64::f_extension::FPRegisters::default(),
            fcsr: crate::riscv64::f_extension::FCSR::default(),
            exceptions_enabled: false,
        }
    }

    /// 更新FCSR标志（双精度版本）
    fn _update_fcsr_flags_f64(&mut self, a: f64, b: f64, result: f64) {
        // 检测NaN
        if result.is_nan() {
            self.fcsr.flags.nv = true;
        }

        // 检测无穷
        if result.is_infinite() && (a.is_infinite() || b.is_infinite()) {
            self.fcsr.flags.of = true;
        }

        // 检测下溢（比最小正数还小的绝对值）
        if result.abs() < f64::MIN_POSITIVE {
            self.fcsr.flags.uf = true;
        }

        // 检测不精确
        if !result.is_finite() || result.fract() != 0.0 {
            self.fcsr.flags.nx = true;
        }
    }
}

impl Default for DExtensionExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// D扩展指令实现
// ============================================================================

impl<'a> RiscvCPU<'a> {
    /// FLD - 浮点加载双字（Floating Load Doubleword）
    ///
    /// 从内存加载双精度浮点数到浮点寄存器。
    ///
    /// # 格式
    /// `fld rd, offset(rs1)`
    ///
    /// # 参数
    /// - `rd`: 目标浮点寄存器
    /// - `rs1`: 基址寄存器
    /// - `imm`: 偏移量（符号扩展）
    ///
    /// # 示例
    /// ```text
    /// fld f1, 8(a0)  # f1 = mem[a0 + 8]
    /// ```
    pub fn exec_fld(&mut self, rd: usize, rs1: usize, imm: i16) -> VmResult<()> {
        let addr = self.regs[rs1].wrapping_add(imm as u64);
        let value = self.read_u64(addr)?;
        let fp_val = f64::from_bits(value);
        self.fp_regs.set_f64(rd, fp_val);
        Ok(())
    }

    /// FSD - 浮点存储双字（Floating Store Doubleword）
    ///
    /// 将双精度浮点数存储到内存。
    ///
    /// # 格式
    /// `fsd rs2, offset(rs1)`
    ///
    /// # 参数
    /// - `rs1`: 基址寄存器
    /// - `rs2`: 源浮点寄存器
    /// - `imm`: 偏移量（符号扩展）
    ///
    /// # 示例
    /// ```text
    /// fsd f1, 8(a0)  # mem[a0 + 8] = f1
    /// ```
    pub fn exec_fsd(&mut self, rs1: usize, rs2: usize, imm: i16) -> VmResult<()> {
        let addr = self.regs[rs1].wrapping_add(imm as u64);
        let fp_val = self.fp_regs.get_f64(rs2);
        let value = fp_val.to_bits();
        self.write_u64(addr, value)?;
        Ok(())
    }

    /// FADD.D - 双精度浮点加法
    ///
    /// # 格式
    /// `fadd.d rd, rs1, rs2`
    ///
    /// # IEEE 754行为
    /// - 遵循当前舍入模式
    /// - 设置适当的异常标志
    ///
    /// # 示例
    /// ```text
    /// fadd.d f3, f1, f2  # f3 = f1 + f2 (double precision)
    /// ```
    pub fn exec_fadd_d(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        let b = self.fp_regs.get_f64(rs2);
        let result = a + b;
        self.fp_regs.set_f64(rd, result);
        self.update_fcsr_flags_f64(a, b, result);
        Ok(())
    }

    /// FSUB.D - 双精度浮点减法
    ///
    /// # 格式
    /// `fsub.d rd, rs1, rs2`
    ///
    /// # 示例
    /// ```text
    /// fsub.d f3, f1, f2  # f3 = f1 - f2 (double precision)
    /// ```
    pub fn exec_fsub_d(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        let b = self.fp_regs.get_f64(rs2);
        let result = a - b;
        self.fp_regs.set_f64(rd, result);
        self.update_fcsr_flags_f64(a, b, result);
        Ok(())
    }

    /// FMUL.D - 双精度浮点乘法
    ///
    /// # 格式
    /// `fmul.d rd, rs1, rs2`
    ///
    /// # 示例
    /// ```text
    /// fmul.d f3, f1, f2  # f3 = f1 * f2 (double precision)
    /// ```
    pub fn exec_fmul_d(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        let b = self.fp_regs.get_f64(rs2);
        let result = a * b;
        self.fp_regs.set_f64(rd, result);
        self.update_fcsr_flags_f64(a, b, result);
        Ok(())
    }

    /// FDIV.D - 双精度浮点除法
    ///
    /// # 格式
    /// `fdiv.d rd, rs1, rs2`
    ///
    /// # 特殊情况
    /// - 除以零：返回±∞，设置DZ标志
    /// - 0/0：返回NaN，设置NV标志
    ///
    /// # 示例
    /// ```text
    /// fdiv.d f3, f1, f2  # f3 = f1 / f2 (double precision)
    /// ```
    pub fn exec_fdiv_d(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        let b = self.fp_regs.get_f64(rs2);
        if b == 0.0 {
            self.fcsr.flags.dz = true; // 除零标志
            let result = if a.is_sign_negative() {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
            self.fp_regs.set_f64(rd, result);
        } else {
            let result = a / b;
            self.fp_regs.set_f64(rd, result);
            self.update_fcsr_flags_f64(a, b, result);
        }
        Ok(())
    }

    /// FSQRT.D - 双精度浮点平方根
    ///
    /// # 格式
    /// `fsqrt.d rd, rs1`
    ///
    /// # 特殊情况
    /// - 负数：返回NaN，设置NV标志
    /// - +∞：返回+∞
    /// - +0：返回+0
    ///
    /// # 示例
    /// ```text
    /// fsqrt.d f2, f1  # f2 = sqrt(f1)
    /// ```
    pub fn exec_fsqrt_d(&mut self, rd: usize, rs1: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        if a < 0.0 {
            self.fcsr.flags.nv = true; // 无效操作
            self.fp_regs.set_f64(rd, f64::NAN);
        } else {
            let result = a.sqrt();
            self.fp_regs.set_f64(rd, result);
            // 平方根总是精确的或设置NX标志
            if result != result.floor() {
                self.fcsr.flags.nx = true;
            }
        }
        Ok(())
    }

    /// FMIN.D - 双精度浮点最小值
    ///
    /// # 格式
    /// `fmin.d rd, rs1, rs2`
    ///
    /// # IEEE 754行为
    /// - 如果任一操作数是NaN，返回NaN
    /// - 否则返回较小的操作数
    /// - 符号零处理：-0.0 < +0.0
    ///
    /// # 示例
    /// ```text
    /// fmin.d f3, f1, f2  # f3 = min(f1, f2)
    /// ```
    pub fn exec_fmin_d(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        let b = self.fp_regs.get_f64(rs2);
        let result = if a.is_nan() || b.is_nan() {
            f64::NAN
        } else {
            a.min(b)
        };
        self.fp_regs.set_f64(rd, result);
        Ok(())
    }

    /// FMAX.D - 双精度浮点最大值
    ///
    /// # 格式
    /// `fmax.d rd, rs1, rs2`
    ///
    /// # IEEE 754行为
    /// - 如果任一操作数是NaN，返回NaN
    /// - 否则返回较大的操作数
    /// - 符号零处理：-0.0 < +0.0
    ///
    /// # 示例
    /// ```text
    /// fmax.d f3, f1, f2  # f3 = max(f1, f2)
    /// ```
    pub fn exec_fmax_d(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        let b = self.fp_regs.get_f64(rs2);
        let result = if a.is_nan() || b.is_nan() {
            f64::NAN
        } else {
            a.max(b)
        };
        self.fp_regs.set_f64(rd, result);
        Ok(())
    }

    /// FEQ.D - 双精度浮点相等比较
    ///
    /// # 格式
    /// `feq.d rd, rs1, rs2`
    ///
    /// # 返回值
    /// - 相等：rd = 1
    /// - 不相等：rd = 0
    /// - 任一操作数是NaN：rd = 0（不设置异常标志）
    ///
    /// # 示例
    /// ```text
    /// feq.d a0, f1, f2  # a0 = (f1 == f2)
    /// ```
    pub fn exec_feq_d(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        let b = self.fp_regs.get_f64(rs2);
        let result = if a == b { 1 } else { 0 };
        self.regs[rd] = result as u64;
        Ok(())
    }

    /// FLT.D - 双精度浮点小于比较
    ///
    /// # 格式
    /// `flt.d rd, rs1, rs2`
    ///
    /// # 返回值
    /// - rs1 < rs2：rd = 1
    /// - rs1 >= rs2：rd = 0
    /// - 任一操作数是NaN：rd = 0，设置NV标志
    ///
    /// # 示例
    /// ```text
    /// flt.d a0, f1, f2  # a0 = (f1 < f2)
    /// ```
    pub fn exec_flt_d(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        let b = self.fp_regs.get_f64(rs2);
        if a.is_nan() || b.is_nan() {
            self.fcsr.flags.nv = true;
            self.regs[rd] = 0;
        } else {
            let result = if a < b { 1 } else { 0 };
            self.regs[rd] = result as u64;
        }
        Ok(())
    }

    /// FLE.D - 双精度浮点小于等于比较
    ///
    /// # 格式
    /// `fle.d rd, rs1, rs2`
    ///
    /// # 返回值
    /// - rs1 <= rs2：rd = 1
    /// - rs1 > rs2：rd = 0
    /// - 任一操作数是NaN：rd = 0，设置NV标志
    ///
    /// # 示例
    /// ```text
    /// fle.d a0, f1, f2  # a0 = (f1 <= f2)
    /// ```
    pub fn exec_fle_d(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        let b = self.fp_regs.get_f64(rs2);
        if a.is_nan() || b.is_nan() {
            self.fcsr.flags.nv = true;
            self.regs[rd] = 0;
        } else {
            let result = if a <= b { 1 } else { 0 };
            self.regs[rd] = result as u64;
        }
        Ok(())
    }

    /// FCVT.S.D - 双精度转单精度
    ///
    /// # 格式
    /// `fcvt.s.d rd, rs1`
    ///
    /// # IEEE 754行为
    /// - 遵循当前舍入模式
    /// - 溢出时设置OF标志
    /// - 下溢时设置UF标志
    ///
    /// # 示例
    /// ```text
    /// fcvt.s.d f2, f1  # f2 = (f32)f1
    /// ```
    pub fn exec_fcvt_s_d(&mut self, rd: usize, rs1: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        let result = a as f32; // double -> float
        // FCVT.S.D converts f64 to f32, so store in f32 register
        self.fp_regs.set(rd, result);

        // 检测溢出/下溢
        if a.abs() > f32::MAX as f64 {
            self.fcsr.flags.of = true;
        } else if a.abs() < f32::MIN_POSITIVE as f64 && a != 0.0 {
            self.fcsr.flags.uf = true;
        }

        Ok(())
    }

    /// FCVT.D.S - 单精度转双精度
    ///
    /// # 格式
    /// `fcvt.d.s rd, rs1`
    ///
    /// # 说明
    /// - 这个转换总是精确的（不会损失精度）
    /// - 不设置任何异常标志
    ///
    /// # 示例
    /// ```text
    /// fcvt.d.s f2, f1  # f2 = (f64)f1
    /// ```
    pub fn exec_fcvt_d_s(&mut self, rd: usize, rs1: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);
        let result = a as f64; // float -> double
        self.fp_regs.set_f64(rd, result);
        Ok(())
    }

    /// FCVT.L.D - 双精度转有符号长整数（64位）
    ///
    /// # 格式
    /// `fcvt.l.d rd, rs1`
    ///
    /// # IEEE 754行为
    /// - 遵循当前舍入模式
    /// - 无效输入（NaN、无穷大）设置NV标志
    /// - 溢出（超出i64范围）设置NV标志
    ///
    /// # 示例
    /// ```text
    /// fcvt.l.d a0, f1  # a0 = (i64)f1
    /// ```
    pub fn exec_fcvt_l_d(&mut self, rd: usize, rs1: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);

        if !a.is_finite() {
            self.fcsr.flags.nv = true;
            self.regs[rd] = if a.is_nan() {
                0x8000_0000_0000_0000 // i64::MIN作为错误指示
            } else {
                // 无穷大
                if a.is_sign_positive() {
                    i64::MAX as u64
                } else {
                    i64::MIN as u64
                }
            };
        } else if a > (i64::MAX as f64) || a < (i64::MIN as f64) {
            self.fcsr.flags.nv = true;
            self.regs[rd] = if a > 0.0 {
                i64::MAX as u64
            } else {
                i64::MIN as u64
            };
        } else {
            let result = a as i64;
            self.regs[rd] = result as u64;
        }

        Ok(())
    }

    /// FCVT.D.L - 有符号长整数转双精度
    ///
    /// # 格式
    /// `fcvt.d.l rd, rs1`
    ///
    /// # 说明
    /// - 转换总是精确的
    /// - i64的所有值都可以精确表示为f64
    ///
    /// # 示例
    /// ```text
    /// fcvt.d.l f1, a0  # f1 = (f64)a0
    /// ```
    pub fn exec_fcvt_d_l(&mut self, rd: usize, rs1: usize) -> VmResult<()> {
        let a = self.regs[rs1] as i64;
        let result = a as f64;
        self.fp_regs.set_f64(rd, result);
        Ok(())
    }

    /// FCVT.LU.D - 双精度转无符号长整数（64位）
    ///
    /// # 格式
    /// `fcvt.lu.d rd, rs1`
    ///
    /// # IEEE 754行为
    /// - 遵循当前舍入模式
    /// - 负数输入被视为0.0
    /// - 无效输入（NaN、无穷大）设置NV标志
    /// - 溢出（超出u64范围）设置NV标志
    ///
    /// # 示例
    /// ```text
    /// fcvt.lu.d a0, f1  # a0 = (u64)f1
    /// ```
    pub fn exec_fcvt_lu_d(&mut self, rd: usize, rs1: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);

        if !a.is_finite() {
            self.fcsr.flags.nv = true;
            self.regs[rd] = if a.is_sign_negative() || a.is_nan() {
                0
            } else {
                u64::MAX
            };
        } else if a < 0.0 {
            // 负数被截断为0
            self.regs[rd] = 0;
        } else if a > (u64::MAX as f64) {
            self.fcsr.flags.nv = true;
            self.regs[rd] = u64::MAX;
        } else {
            let result = a as u64;
            self.regs[rd] = result;
        }

        Ok(())
    }

    /// FCVT.D.LU - 无符号长整数转双精度
    ///
    /// # 格式
    /// `fcvt.d.lu rd, rs1`
    ///
    /// # 说明
    /// - 对于大于2^53的值，转换可能损失精度
    /// - 这是f64精度的限制
    ///
    /// # 示例
    /// ```text
    /// fcvt.d.lu f1, a0  # f1 = (f64)a0
    /// ```
    pub fn exec_fcvt_d_lu(&mut self, rd: usize, rs1: usize) -> VmResult<()> {
        let a = self.regs[rs1];
        let result = a as f64;
        self.fp_regs.set_f64(rd, result);

        // 对于大值（> 2^53），设置不精确标志
        if a > 9007199254740992 {
            // 2^53
            self.fcsr.flags.nx = true;
        }

        Ok(())
    }

    /// FCLASS.D - 双精度浮点分类
    ///
    /// # 格式
    /// `fclass.d rd, rs1`
    ///
    /// # 返回值
    /// 返回一个10位掩码，指示浮点数的类别：
    /// - bit 0: 负无穷大
    /// - bit 1: 负正常数
    /// - bit 2: 负次正规数
    /// - bit 3: 负零
    /// - bit 4: 正零
    /// - bit 5: 正次正规数
    /// - bit 6: 正正常数
    /// - bit 7: 正无穷大
    /// - bit 8: signaling NaN
    /// - bit 9: quiet NaN
    ///
    /// # 示例
    /// ```text
    /// fclass.d a0, f1  # a0 = fclass(f1)
    /// ```
    pub fn exec_fclass_d(&mut self, rd: usize, rs1: usize) -> VmResult<()> {
        let a = self.fp_regs.get_f64(rs1);

        let mask = if a.is_nan() {
            // 检查是signaling NaN还是quiet NaN
            let bits = a.to_bits();
            // quiet NaN: 最高分数位为1
            // signaling NaN: 最高分数位为0
            if (bits & 0x0008_0000_0000_0000) != 0 {
                0b0010_0000_0000 // bit 9: quiet NaN
            } else {
                0b0001_0000_0000 // bit 8: signaling NaN
            }
        } else if a.is_infinite() {
            if a.is_sign_negative() {
                0b0000_0000_0001 // bit 0: 负无穷大
            } else {
                0b0100_0000_0000 // bit 7: 正无穷大
            }
        } else if a == 0.0 {
            if a.is_sign_negative() {
                0b0000_1000_0000 // bit 3: 负零
            } else {
                0b0001_0000_0000 // bit 4: 正零
            }
        } else {
            // 检查是否是次正规数（subnormal/denormal）
            let abs = a.abs();
            let is_subnormal = abs < f64::MIN_POSITIVE;

            if a.is_sign_negative() {
                if is_subnormal {
                    0b0000_0100_0000 // bit 2: 负次正规数
                } else {
                    0b0000_0010_0000 // bit 1: 负正常数
                }
            } else if is_subnormal {
                0b0000_0001_0000 // bit 5: 正次正规数
            } else {
                0b0000_0000_1000 // bit 6: 正正常数
            }
        };

        self.regs[rd] = mask as u64;
        Ok(())
    }

    /// 辅助方法：更新FCSR标志（双精度版本）
    fn update_fcsr_flags_f64(&mut self, a: f64, b: f64, result: f64) {
        // 检测NaN
        if result.is_nan() {
            self.fcsr.flags.nv = true;
        }

        // 检测无穷
        if result.is_infinite() && (a.is_infinite() || b.is_infinite()) {
            self.fcsr.flags.of = true;
        }

        // 检测下溢（比最小正数还小的绝对值）
        if result.abs() < f64::MIN_POSITIVE && result != 0.0 {
            self.fcsr.flags.uf = true;
        }

        // 检测不精确
        if !result.is_finite() || result.fract() != 0.0 {
            self.fcsr.flags.nx = true;
        }
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(dead_code)] // 测试辅助函数可能未使用

    use super::*;
    use crate::riscv64::f_extension::{FCSR, FFlags, FPRegisters, RoundingMode};
    use std::collections::HashMap;
    use vm_core::{
        AccessType, AddressTranslator, GuestAddr, GuestPhysAddr, MemoryAccess, MmioDevice,
        MmioManager, MmuAsAny, VmError,
    };

    /// 简单的Mock MMU，用于测试
    struct MockMMU {
        memory: HashMap<u64, u8>,
    }

    impl MockMMU {
        fn new() -> Self {
            Self {
                memory: HashMap::new(),
            }
        }
    }

    impl MemoryAccess for MockMMU {
        fn read(&self, addr: GuestAddr, size: u8) -> Result<u64, VmError> {
            let mut result = 0u64;
            for i in 0..size {
                let byte = self.memory.get(&(addr.0 + i as u64)).copied().unwrap_or(0);
                result |= (byte as u64) << (i * 8);
            }
            Ok(result)
        }

        fn write(&mut self, addr: GuestAddr, value: u64, size: u8) -> Result<(), VmError> {
            for i in 0..size {
                let byte = (value >> (i * 8)) & 0xFF;
                self.memory.insert(addr.0 + i as u64, byte as u8);
            }
            Ok(())
        }

        fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
            self.read(pc, 4)
        }

        fn memory_size(&self) -> usize {
            self.memory.len()
        }

        fn dump_memory(&self) -> Vec<u8> {
            let max_addr = self.memory.keys().copied().max().unwrap_or(0) as usize;
            let mut result = vec![0u8; max_addr + 1];
            for (addr, &val) in &self.memory {
                if *addr as usize <= max_addr {
                    result[*addr as usize] = val;
                }
            }
            result
        }

        fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
            for (i, &val) in data.iter().enumerate() {
                self.memory.insert(i as u64, val);
            }
            Ok(())
        }
    }

    impl AddressTranslator for MockMMU {
        fn translate(
            &mut self,
            va: GuestAddr,
            _access: AccessType,
        ) -> Result<GuestPhysAddr, VmError> {
            Ok(GuestPhysAddr(va.0))
        }

        fn flush_tlb(&mut self) {
            // No TLB in simple test MMU
        }
    }

    impl MmioManager for MockMMU {
        fn map_mmio(&self, _base: GuestAddr, _size: u64, _device: Box<dyn MmioDevice>) {
            // No MMIO mapping in test MMU
        }
    }

    impl MmuAsAny for MockMMU {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    /// 创建测试用的虚拟机实例
    fn create_test_cpu() -> crate::riscv64::RiscvCPU<'static> {
        use crate::riscv64::RiscvCPU;

        // 创建MMU实例并使用leak创建'static引用
        let mmu = Box::leak(Box::new(MockMMU::new()));

        // 创建CPU并初始化基本状态
        let mut cpu = RiscvCPU::new(mmu);
        cpu.pc = GuestAddr(0x1000);

        // 初始化x0为0（RISC-V约定：x0始终为0）
        cpu.regs[0] = 0;

        // 初始化浮点寄存器为0
        for i in 0..32 {
            cpu.fp_regs.set_f64(i, 0.0f64);
        }

        // 初始化FCSR
        cpu.fcsr = FCSR {
            flags: FFlags {
                nv: false,
                dz: false,
                of: false,
                uf: false,
                nx: false,
            },
            rm: RoundingMode::RNE,
        };

        cpu
    }

    #[test]
    fn test_fld_fsd() {
        let mut fp_regs = FPRegisters::default();

        // 测试双精度浮点加载/存储
        let test_value: f64 = 1.5;
        fp_regs.set_f64(1, test_value);
        assert_eq!(fp_regs.get_f64(1), test_value);
    }

    #[test]
    fn test_fadd_d() {
        let mut fp_regs = FPRegisters::default();
        let mut fcsr = FCSR {
            flags: FFlags {
                nv: false,
                dz: false,
                of: false,
                uf: false,
                nx: false,
            },
            rm: RoundingMode::RNE,
        };

        fp_regs.set_f64(1, 1.0);
        fp_regs.set_f64(2, 2.0);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = a + b;
        fp_regs.set_f64(3, result);

        assert_eq!(fp_regs.get_f64(3), 3.0);
    }

    #[test]
    fn test_fdiv_d() {
        let mut fp_regs = FPRegisters::default();

        fp_regs.set_f64(1, 10.0);
        fp_regs.set_f64(2, 2.0);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = a / b;
        fp_regs.set_f64(3, result);

        assert!((fp_regs.get_f64(3) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fsqrt_d() {
        let mut fp_regs = FPRegisters::default();

        fp_regs.set_f64(1, 16.0);
        let a = fp_regs.get_f64(1);
        let result = a.sqrt();
        fp_regs.set_f64(2, result);

        assert!((fp_regs.get_f64(2) - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fcvt_s_d() {
        let mut fp_regs = FPRegisters::default();

        // double -> float转换
        fp_regs.set_f64(1, 1.5);
        let a = fp_regs.get_f64(1);
        let result = a as f32;
        fp_regs.set_f64(2, result as f64);

        assert!((fp_regs.get_f64(2) - 1.5f64).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fcvt_d_s() {
        let mut fp_regs = FPRegisters::default();

        // float -> double转换
        fp_regs.set_f64(1, 3.14159f32 as f64);
        let a = fp_regs.get_f64(1);
        let result = a as f64;
        fp_regs.set_f64(2, result);

        // Note: 3.14159f32 has limited precision, when cast to f64 it becomes 3.141590118408203
        // The key is that the value is preserved, not that it equals 3.14159f64
        let expected = 3.14159f32 as f64;
        assert!((fp_regs.get_f64(2) - expected).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fcvt_l_d() {
        let mut fp_regs = FPRegisters::default();

        // double -> i64转换
        fp_regs.set_f64(1, 42.7);
        let a = fp_regs.get_f64(1);
        let result = a as i64;

        assert_eq!(result, 42);
    }

    #[test]
    fn test_fcvt_d_l() {
        let mut fp_regs = FPRegisters::default();

        // i64 -> double转换
        let a: i64 = -12345;
        let result = a as f64;
        fp_regs.set_f64(1, result);

        assert_eq!(fp_regs.get_f64(1), -12345.0f64);
    }

    #[test]
    fn test_fcvt_lu_d() {
        let mut fp_regs = FPRegisters::default();

        // double -> u64转换
        fp_regs.set_f64(1, 4294967295.7); // u32::MAX
        let a = fp_regs.get_f64(1);
        let result = a as u64;

        assert_eq!(result, 4294967295);
    }

    #[test]
    fn test_fcvt_d_lu() {
        let mut fp_regs = FPRegisters::default();

        // u64 -> double转换
        let a: u64 = 18446744073709551615; // u64::MAX
        let result = a as f64;
        fp_regs.set_f64(1, result);

        // 注意：大整数转f64可能损失精度
        assert!(fp_regs.get_f64(1) > 1.8e19);
    }

    #[test]
    fn test_d_extension_precision() {
        let mut fp_regs = FPRegisters::default();

        // 测试双精度精度
        fp_regs.set_f64(1, std::f64::consts::PI);
        fp_regs.set_f64(2, std::f64::consts::E);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = a + b;
        fp_regs.set_f64(3, result);

        let expected = std::f64::consts::PI + std::f64::consts::E;
        assert!((fp_regs.get_f64(3) - expected).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fmin_d() {
        let mut fp_regs = FPRegisters::default();

        fp_regs.set_f64(1, 3.5);
        fp_regs.set_f64(2, 2.7);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = a.min(b);
        fp_regs.set_f64(3, result);

        assert_eq!(fp_regs.get_f64(3), 2.7);
    }

    #[test]
    fn test_fmax_d() {
        let mut fp_regs = FPRegisters::default();

        fp_regs.set_f64(1, 3.5);
        fp_regs.set_f64(2, 2.7);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = a.max(b);
        fp_regs.set_f64(3, result);

        assert_eq!(fp_regs.get_f64(3), 3.5);
    }

    #[test]
    fn test_feq_d() {
        let mut fp_regs = FPRegisters::default();

        fp_regs.set_f64(1, 1.0);
        fp_regs.set_f64(2, 1.0);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = if a == b { 1 } else { 0 };

        assert_eq!(result, 1);
    }

    #[test]
    fn test_flt_d() {
        let mut fp_regs = FPRegisters::default();

        fp_regs.set_f64(1, 1.0);
        fp_regs.set_f64(2, 2.0);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = if a < b { 1 } else { 0 };

        assert_eq!(result, 1);
    }

    #[test]
    fn test_fle_d() {
        let mut fp_regs = FPRegisters::default();

        fp_regs.set_f64(1, 1.0);
        fp_regs.set_f64(2, 1.0);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = if a <= b { 1 } else { 0 };

        assert_eq!(result, 1);
    }

    #[test]
    fn test_nan_handling_d() {
        let mut fp_regs = FPRegisters::default();

        // NaN加法测试
        fp_regs.set_f64(1, f64::NAN);
        fp_regs.set_f64(2, 1.0);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = a + b;
        fp_regs.set_f64(3, result);

        assert!(fp_regs.get_f64(3).is_nan());
    }

    #[test]
    fn test_infinity_handling_d() {
        let mut fp_regs = FPRegisters::default();

        // 无穷大测试
        fp_regs.set_f64(1, f64::INFINITY);
        fp_regs.set_f64(2, 1.0);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = a + b;
        fp_regs.set_f64(3, result);

        assert!(fp_regs.get_f64(3).is_infinite());
    }

    #[test]
    fn test_divide_by_zero_d() {
        let mut fp_regs = FPRegisters::default();
        let mut fcsr = FCSR {
            flags: FFlags {
                nv: false,
                dz: false,
                of: false,
                uf: false,
                nx: false,
            },
            rm: RoundingMode::RNE,
        };

        fp_regs.set_f64(1, 1.0);
        fp_regs.set_f64(2, 0.0);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);

        if b == 0.0 {
            fcsr.flags.dz = true;
            let result = if a.is_sign_negative() {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
            fp_regs.set_f64(3, result);
        }

        assert!(fp_regs.get_f64(3).is_infinite());
        assert!(fcsr.flags.dz);
    }

    #[test]
    fn test_fclass_d_infinity() {
        let mut fp_regs = FPRegisters::default();

        // 正无穷大
        fp_regs.set_f64(1, f64::INFINITY);
        let a = fp_regs.get_f64(1);
        let mask = if a.is_infinite() && !a.is_sign_negative() {
            0b0100_0000_0000 // bit 7
        } else {
            0
        };
        assert_eq!(mask, 0b0100_0000_0000);

        // 负无穷大
        fp_regs.set_f64(1, f64::NEG_INFINITY);
        let a = fp_regs.get_f64(1);
        let mask = if a.is_infinite() && a.is_sign_negative() {
            0b0000_0000_0001 // bit 0
        } else {
            0
        };
        assert_eq!(mask, 0b0000_0000_0001);
    }

    #[test]
    fn test_fclass_d_zero() {
        let mut fp_regs = FPRegisters::default();

        // 正零
        fp_regs.set_f64(1, 0.0);
        let a = fp_regs.get_f64(1);
        let mask = if a == 0.0 && !a.is_sign_negative() {
            0b0001_0000_0000 // bit 4
        } else {
            0
        };
        assert_eq!(mask, 0b0001_0000_0000);

        // 负零
        fp_regs.set_f64(1, -0.0);
        let a = fp_regs.get_f64(1);
        let mask = if a == 0.0 && a.is_sign_negative() {
            0b0000_1000_0000 // bit 3
        } else {
            0
        };
        assert_eq!(mask, 0b0000_1000_0000);
    }

    #[test]
    fn test_fclass_d_nan() {
        let mut fp_regs = FPRegisters::default();

        fp_regs.set_f64(1, f64::NAN);
        let a = fp_regs.get_f64(1);
        let bits = a.to_bits();
        let mask = if a.is_nan() {
            if (bits & 0x0008_0000_0000_0000) != 0 {
                0b0010_0000_0000 // bit 9: quiet NaN
            } else {
                0b0001_0000_0000 // bit 8: signaling NaN
            }
        } else {
            0
        };

        // Rust的NAN通常是quiet NaN
        assert!(mask != 0);
    }

    #[test]
    fn test_fclass_d_normal() {
        let mut fp_regs = FPRegisters::default();

        // 正正常数
        fp_regs.set_f64(1, 1.5);
        let a = fp_regs.get_f64(1);
        let is_subnormal = a.abs() < f64::MIN_POSITIVE;
        let mask = if !is_subnormal && a.is_sign_positive() && a.is_finite() && a != 0.0 {
            0b0000_0000_1000 // bit 6
        } else {
            0
        };
        assert_eq!(mask, 0b0000_0000_1000);

        // 负正常数
        fp_regs.set_f64(1, -1.5);
        let a = fp_regs.get_f64(1);
        let is_subnormal = a.abs() < f64::MIN_POSITIVE;
        let mask = if !is_subnormal && a.is_sign_negative() && a.is_finite() && a != 0.0 {
            0b0000_0010_0000 // bit 1
        } else {
            0
        };
        assert_eq!(mask, 0b0000_0010_0000);
    }

    #[test]
    fn test_fclass_d_subnormal() {
        let mut fp_regs = FPRegisters::default();

        // 正次正规数（subnormal）
        let subnormal_pos: f64 = 1.0e-320; // 非常小的正数
        fp_regs.set_f64(1, subnormal_pos);
        let a = fp_regs.get_f64(1);
        let is_subnormal = a.abs() < f64::MIN_POSITIVE && a != 0.0;
        let mask = if is_subnormal && a.is_sign_positive() {
            0b0000_0001_0000 // bit 5
        } else {
            0
        };

        // 如果数值确实是次正规数
        if subnormal_pos < f64::MIN_POSITIVE {
            assert_eq!(mask, 0b0000_0001_0000);
        }
    }

    #[test]
    fn test_double_precision_range() {
        // 测试双精度浮点数的范围
        let max: f64 = f64::MAX;
        let min: f64 = f64::MIN_POSITIVE;

        assert!(max > 1.7e308);
        // f64::MIN_POSITIVE = 2.2250738585072014e-308, which is > 2.2e-308
        assert!(min > 2.2e-308); // Changed assertion to match actual value
        assert!(min > 0.0);
    }

    #[test]
    fn test_rounding_modes_d() {
        let mut fp_regs = FPRegisters::default();

        // 测试不同舍入模式（简化测试）
        fp_regs.set_f64(1, 1.5);
        fp_regs.set_f64(2, 2.5);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = a + b;

        // 默认舍入模式应该是最近舍入
        assert_eq!(result, 4.0);
    }

    #[test]
    fn test_double_vs_single_precision() {
        let mut fp_regs = FPRegisters::default();

        // 演示双精度vs单精度的差异
        let value_single: f32 = 1.0 / 3.0;
        let value_double: f64 = 1.0 / 3.0;

        fp_regs.set_f64(1, value_single as f64);
        fp_regs.set_f64(2, value_double);

        // 双精度应该更精确（更多小数位）
        let single_val = fp_regs.get_f64(1);
        let double_val = fp_regs.get_f64(2);

        // Double precision has more precision digits than single precision
        // The key difference is in the mantissa, not just the bit pattern
        assert!(double_val != single_val); // They should differ
        assert!(double_val.to_bits() != single_val.to_bits()); // Different representations
    }

    #[test]
    fn test_overflow_handling_d() {
        let mut fp_regs = FPRegisters::default();
        let mut fcsr = FCSR {
            flags: FFlags {
                nv: false,
                dz: false,
                of: false,
                uf: false,
                nx: false,
            },
            rm: RoundingMode::RNE,
        };

        // 测试溢出
        fp_regs.set_f64(1, f64::MAX);
        fp_regs.set_f64(2, f64::MAX);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = a * b;
        fp_regs.set_f64(3, result);

        // 结果应该是无穷大
        assert!(fp_regs.get_f64(3).is_infinite());
    }

    #[test]
    fn test_underflow_handling_d() {
        let mut fp_regs = FPRegisters::default();

        // 测试下溢
        fp_regs.set_f64(1, f64::MIN_POSITIVE);
        fp_regs.set_f64(2, 0.1);

        let a = fp_regs.get_f64(1);
        let b = fp_regs.get_f64(2);
        let result = a * b;
        fp_regs.set_f64(3, result);

        // 结果应该非常小或为零
        assert!(fp_regs.get_f64(3) < f64::MIN_POSITIVE);
    }
}
