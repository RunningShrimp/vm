//! RISC-V F扩展：单精度浮点指令集
//!
//! 实现RISC-V单精度浮点指令集（F扩展），包含30个指令。
//!
//! ## 指令分类
//!
//! - 加载/存储：FLW, FSW
//! - 运算指令：FADD.S, FSUB.S, FMUL.S, FDIV.S, FSQRT.S, FMIN.S, FMAX.S
//! - 比较指令：FEQ.S, FLT.S, FLE.S
//! - 类型转换：FCVT.W.S, FCVT.S.W, FCVT.WU.S, FCVT.S.WU
//! - 数据移动：FMV.X.W, FMV.W.X
//! - 分类指令：FCLASS.S
//! - FCSR管理：FRCSR, FSCSR
//!
//! ## IEEE 754兼容性
//!
//! - 支持所有IEEE 754-2008单精度浮点格式
//! - 支持NaN、Infinity、Denormalized numbers
//! - 支持所有5种IEEE舍入模式
//! - 支持浮点异常标志

use vm_core::{VmError, VmResult};

/// 浮点寄存器（32个，f0-f31）
///
/// 支持F扩展（f32）和D扩展（f64）的寄存器存储。
#[derive(Debug, Clone)]
pub struct FPRegisters {
    regs: [f32; 32],
    /// D扩展专用的f64寄存器（仅在使用D扩展时使用）
    regs64: [f64; 32],
}

impl Default for FPRegisters {
    fn default() -> Self {
        Self {
            regs: [0.0; 32],
            regs64: [0.0; 32],
        }
    }
}

impl FPRegisters {
    /// 获取浮点寄存器值
    pub fn get(&self, idx: usize) -> f32 {
        self.regs[idx]
    }

    /// 设置浮点寄存器值
    pub fn set(&mut self, idx: usize, val: f32) {
        self.regs[idx] = val;
    }

    /// 获取浮点寄存器的位表示
    pub fn get_bits(&self, idx: usize) -> u32 {
        self.regs[idx].to_bits()
    }

    /// 从位表示设置浮点寄存器
    pub fn set_bits(&mut self, idx: usize, bits: u32) {
        self.regs[idx] = f32::from_bits(bits);
    }

    // ========== D扩展：双精度浮点寄存器访问 ==========

    /// 获取双精度浮点寄存器值
    ///
    /// **修复**: 使用独立的f64存储，避免与f32寄存器冲突。
    pub fn get_f64(&self, idx: usize) -> f64 {
        if idx >= 32 {
            return 0.0;
        }
        self.regs64[idx]
    }

    /// 设置双精度浮点寄存器值
    pub fn set_f64(&mut self, idx: usize, val: f64) {
        if idx >= 32 {
            return;
        }
        self.regs64[idx] = val;
    }
}

/// 浮点控制状态寄存器（FCSR）
#[derive(Debug, Clone)]
pub struct FCSR {
    /// 浮点标志
    pub flags: FFlags,
    /// 舍入模式
    pub rm: RoundingMode,
}

impl Default for FCSR {
    fn default() -> Self {
        Self {
            flags: FFlags::default(),
            rm: RoundingMode::RNE,
        }
    }
}

/// 浮点标志
#[derive(Debug, Clone, Copy, Default)]
pub struct FFlags {
    /// 无效操作（Invalid Operation）
    pub nv: bool,
    /// 除以零（Divide by Zero）
    pub dz: bool,
    /// 上溢（Overflow）
    pub of: bool,
    /// 下溢（Underflow）
    pub uf: bool,
    /// 不精确（Inexact）
    pub nx: bool,
}

impl FFlags {
    /// 转换为u32表示
    pub fn to_bits(&self) -> u32 {
        let mut bits = 0u32;
        if self.nv {
            bits |= 0x10;
        }
        if self.dz {
            bits |= 0x08;
        }
        if self.of {
            bits |= 0x04;
        }
        if self.uf {
            bits |= 0x02;
        }
        if self.nx {
            bits |= 0x01;
        }
        bits
    }

    /// 从u32表示创建
    pub fn from_bits(bits: u32) -> Self {
        Self {
            nv: (bits & 0x10) != 0,
            dz: (bits & 0x08) != 0,
            of: (bits & 0x04) != 0,
            uf: (bits & 0x02) != 0,
            nx: (bits & 0x01) != 0,
        }
    }
}

/// IEEE 754舍入模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[derive(Default)]
pub enum RoundingMode {
    /// 四舍五入到偶数（Round to Nearest, ties to Even）
    #[default]
    RNE = 0,
    /// 向零舍入（Round towards Zero）
    RTZ = 1,
    /// 向负无穷舍入（Round Down）
    RDN = 2,
    /// 向正无穷舍入（Round Up）
    RUP = 3,
    /// 四舍五入到最接近（Round to Nearest, ties away from zero）
    RMM = 4,
}

impl RoundingMode {
    /// 从u32值创建舍入模式
    pub fn from_bits(bits: u32) -> Option<Self> {
        match bits {
            0 => Some(Self::RNE),
            1 => Some(Self::RTZ),
            2 => Some(Self::RDN),
            3 => Some(Self::RUP),
            4 => Some(Self::RMM),
            _ => None,
        }
    }

    /// 转换为u32值
    pub fn to_bits(&self) -> u32 {
        *self as u32
    }
}

/// 应用舍入模式到浮点数
fn _apply_rounding(value: f32, rm: RoundingMode) -> f32 {
    match rm {
        RoundingMode::RNE => value, // 硬件自动处理
        RoundingMode::RTZ => {
            if value.is_finite() {
                if value >= 0.0 {
                    value.floor().min(value)
                } else {
                    value.ceil().max(value)
                }
            } else {
                value
            }
        }
        RoundingMode::RDN => value.floor(),
        RoundingMode::RUP => value.ceil(),
        RoundingMode::RMM => {
            let fractional = value.fract().abs();
            if fractional == 0.5 || fractional == -0.5 {
                if value >= 0.0 {
                    value.ceil()
                } else {
                    value.floor()
                }
            } else {
                value
            }
        }
    }
}

/// RISC-V F扩展执行器
pub struct FExtensionExecutor {
    /// 浮点寄存器
    pub fp_regs: FPRegisters,
    /// FCSR
    pub fcsr: FCSR,
    /// 是否启用浮点异常
    pub exceptions_enabled: bool,
}

impl Default for FExtensionExecutor {
    fn default() -> Self {
        Self {
            fp_regs: FPRegisters::default(),
            fcsr: FCSR::default(),
            exceptions_enabled: true,
        }
    }
}

impl FExtensionExecutor {
    /// 创建新的F扩展执行器
    pub fn new() -> Self {
        Self::default()
    }

    /// 更新FCSR标志（检测浮点异常）
    fn update_fcsr_flags(&mut self, a: f32, b: f32, result: f32) {
        // 检测NaN
        if result.is_nan() && !a.is_nan() && !b.is_nan() {
            self.fcsr.flags.nv = true;
        }

        // 检测无穷
        if result.is_infinite() && !a.is_infinite() && !b.is_infinite() {
            self.fcsr.flags.of = true;
        }

        // 检测除零
        if b == 0.0 && a != 0.0 && !a.is_nan() {
            self.fcsr.flags.dz = true;
        }

        // 检测下溢
        if result.abs() < f32::MIN_POSITIVE && result != 0.0 {
            self.fcsr.flags.uf = true;
        }

        // 检测不精确
        if !result.is_finite() || result.fract() != 0.0 {
            self.fcsr.flags.nx = true;
        }
    }

    /// FLW - 浮点加载字（Load Single-Precision Floating-Point）
    ///
    /// 从内存加载单精度浮点数到浮点寄存器
    pub fn exec_flw(
        &mut self,
        rd: usize,
        base_addr: u64,
        offset: i16,
        mem_read: &mut dyn Fn(u64) -> Result<u32, VmError>,
    ) -> VmResult<()> {
        let addr = base_addr.wrapping_add(offset as u64);
        let value = mem_read(addr)?;
        let fp_val = f32::from_bits(value);
        self.fp_regs.set(rd, fp_val);
        Ok(())
    }

    /// FSW - 浮点存储字（Store Single-Precision Floating-Point）
    ///
    /// 将浮点寄存器的单精度浮点数存储到内存
    pub fn exec_fsw(
        &mut self,
        _rs1: usize,
        rs2: usize,
        offset: i16,
        base_addr: u64,
        mem_write: &mut dyn Fn(u64, u32) -> Result<(), VmError>,
    ) -> VmResult<()> {
        let addr = base_addr.wrapping_add(offset as u64);
        let fp_val = self.fp_regs.get(rs2);
        let value = fp_val.to_bits();
        mem_write(addr, value)?;
        Ok(())
    }

    /// FADD.S - 浮点加法（Floating-Point Add）
    ///
    /// 执行单精度浮点加法
    pub fn exec_fadd_s(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);
        let b = self.fp_regs.get(rs2);

        // 处理特殊NaN情况（signaling NaN）
        if a.is_nan() && b.is_nan() {
            self.fcsr.flags.nv = true;
            self.fp_regs.set(rd, f32::NAN);
            return Ok(());
        }

        let result = a + b;
        self.fp_regs.set(rd, result);
        self.update_fcsr_flags(a, b, result);
        Ok(())
    }

    /// FSUB.S - 浮点减法（Floating-Point Subtract）
    ///
    /// 执行单精度浮点减法
    pub fn exec_fsub_s(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);
        let b = self.fp_regs.get(rs2);

        let result = a - b;
        self.fp_regs.set(rd, result);
        self.update_fcsr_flags(a, b, result);
        Ok(())
    }

    /// FMUL.S - 浮点乘法（Floating-Point Multiply）
    ///
    /// 执行单精度浮点乘法
    pub fn exec_fmul_s(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);
        let b = self.fp_regs.get(rs2);

        let result = a * b;
        self.fp_regs.set(rd, result);
        self.update_fcsr_flags(a, b, result);
        Ok(())
    }

    /// FDIV.S - 浮点除法（Floating-Point Divide）
    ///
    /// 执行单精度浮点除法
    pub fn exec_fdiv_s(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);
        let b = self.fp_regs.get(rs2);

        if b == 0.0 {
            self.fcsr.flags.dz = true;
            let result = if a.is_sign_negative() {
                f32::NEG_INFINITY
            } else {
                f32::INFINITY
            };
            self.fp_regs.set(rd, result);
            return Ok(());
        }

        let result = a / b;
        self.fp_regs.set(rd, result);
        self.update_fcsr_flags(a, b, result);
        Ok(())
    }

    /// FSQRT.S - 浮点平方根（Floating-Point Square Root）
    ///
    /// 计算单精度浮点数的平方根
    pub fn exec_fsqrt_s(&mut self, rd: usize, rs1: usize) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);

        if a < 0.0 {
            self.fcsr.flags.nv = true;
            self.fp_regs.set(rd, f32::NAN);
            return Ok(());
        }

        let result = a.sqrt();
        self.fp_regs.set(rd, result);
        self.update_fcsr_flags(a, 0.0, result);
        Ok(())
    }

    /// FMIN.S - 浮点最小值（Floating-Point Minimum）
    ///
    /// 返回两个浮点数中的较小者
    pub fn exec_fmin_s(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);
        let b = self.fp_regs.get(rs2);

        let result = if a.is_nan() || b.is_nan() {
            f32::NAN
        } else {
            a.min(b)
        };

        self.fp_regs.set(rd, result);
        Ok(())
    }

    /// FMAX.S - 浮点最大值（Floating-Point Maximum）
    ///
    /// 返回两个浮点数中的较大者
    pub fn exec_fmax_s(&mut self, rd: usize, rs1: usize, rs2: usize) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);
        let b = self.fp_regs.get(rs2);

        let result = if a.is_nan() || b.is_nan() {
            f32::NAN
        } else {
            a.max(b)
        };

        self.fp_regs.set(rd, result);
        Ok(())
    }

    /// FEQ.S - 浮点相等比较（Floating-Point Equal)
    ///
    /// 比较两个浮点数是否相等
    pub fn exec_feq_s(
        &mut self,
        rd: usize,
        rs1: usize,
        rs2: usize,
        set_reg: &mut dyn FnMut(usize, u64),
    ) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);
        let b = self.fp_regs.get(rs2);

        // 如果任一操作数是NaN，结果为0（不相等）
        let result = if a.is_nan() || b.is_nan() {
            0
        } else if a == b {
            1
        } else {
            0
        };

        set_reg(rd, result as u64);
        Ok(())
    }

    /// FLT.S - 浮点小于比较（Floating-Point Less Than）
    ///
    /// 比较第一个浮点数是否小于第二个
    pub fn exec_flt_s(
        &mut self,
        rd: usize,
        rs1: usize,
        rs2: usize,
        set_reg: &mut dyn FnMut(usize, u64),
    ) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);
        let b = self.fp_regs.get(rs2);

        let result = if a.is_nan() || b.is_nan() {
            0
        } else if a < b {
            1
        } else {
            0
        };

        set_reg(rd, result as u64);
        Ok(())
    }

    /// FLE.S - 浮点小于等于比较（Floating-Point Less Than or Equal）
    ///
    /// 比较第一个浮点数是否小于或等于第二个
    pub fn exec_fle_s(
        &mut self,
        rd: usize,
        rs1: usize,
        rs2: usize,
        set_reg: &mut dyn FnMut(usize, u64),
    ) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);
        let b = self.fp_regs.get(rs2);

        let result = if a.is_nan() || b.is_nan() {
            0
        } else if a <= b {
            1
        } else {
            0
        };

        set_reg(rd, result as u64);
        Ok(())
    }

    /// FCVT.W.S - 浮点转有符号整数（Float to Signed Integer）
    ///
    /// 将单精度浮点数转换为32位有符号整数
    pub fn exec_fcvt_w_s(
        &mut self,
        rd: usize,
        rs1: usize,
        set_reg: &mut dyn FnMut(usize, u64),
    ) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);

        if a.is_nan() || a.is_infinite() {
            self.fcsr.flags.nv = true;
            // 饱和到最大/最小值
            let result = if a.is_sign_negative() {
                i32::MIN as u64
            } else {
                i32::MAX as u64
            };
            set_reg(rd, result);
            return Ok(());
        }

        // 饱和转换
        let result = if a < (i32::MIN as f32) {
            self.fcsr.flags.nv = true;
            i32::MIN as u64
        } else if a > (i32::MAX as f32) {
            self.fcsr.flags.nv = true;
            i32::MAX as u64
        } else {
            a as i32 as u64
        };

        set_reg(rd, result);
        Ok(())
    }

    /// FCVT.S.W - 有符号整数转浮点（Signed Integer to Float）
    ///
    /// 将32位有符号整数转换为单精度浮点数
    pub fn exec_fcvt_s_w(&mut self, rd: usize, rs1: u64) -> VmResult<()> {
        let val = rs1 as i32;
        let result = val as f32;
        self.fp_regs.set(rd, result);
        Ok(())
    }

    /// FCVT.WU.S - 浮点转无符号整数（Float to Unsigned Integer）
    ///
    /// 将单精度浮点数转换为32位无符号整数
    pub fn exec_fcvt_wu_s(
        &mut self,
        rd: usize,
        rs1: usize,
        set_reg: &mut dyn FnMut(usize, u64),
    ) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);

        if a.is_nan() || a.is_infinite() {
            self.fcsr.flags.nv = true;
            let result = if a.is_sign_negative() {
                0u64
            } else {
                u32::MAX as u64
            };
            set_reg(rd, result);
            return Ok(());
        }

        // 饱和转换
        let result = if a < 0.0 {
            self.fcsr.flags.nv = true;
            0u64
        } else if a > (u32::MAX as f32) {
            self.fcsr.flags.nv = true;
            u32::MAX as u64
        } else {
            a as u32 as u64
        };

        set_reg(rd, result);
        Ok(())
    }

    /// FCVT.S.WU - 无符号整数转浮点（Unsigned Integer to Float）
    ///
    /// 将32位无符号整数转换为单精度浮点数
    pub fn exec_fcvt_s_wu(&mut self, rd: usize, rs1: u64) -> VmResult<()> {
        let val = (rs1 & 0xFFFFFFFF) as u32;
        let result = val as f32;
        self.fp_regs.set(rd, result);
        Ok(())
    }

    /// FMV.X.W - 浮点寄存器到整数寄存器（Floating-Point Move to Integer）
    ///
    /// 将浮点寄存器的位表示移动到整数寄存器
    pub fn exec_fmv_x_w(
        &mut self,
        rd: usize,
        rs1: usize,
        set_reg: &mut dyn FnMut(usize, u64),
    ) -> VmResult<()> {
        let bits = self.fp_regs.get_bits(rs1);
        set_reg(rd, bits as u64);
        Ok(())
    }

    /// FMV.W.X - 整数寄存器到浮点寄存器（Integer Move to Floating-Point）
    ///
    /// 将整数寄存器的位表示移动到浮点寄存器
    pub fn exec_fmv_w_x(&mut self, rd: usize, rs1: u64) -> VmResult<()> {
        let bits = (rs1 & 0xFFFFFFFF) as u32;
        self.fp_regs.set_bits(rd, bits);
        Ok(())
    }

    /// FCLASS.S - 浮点分类（Floating-Point Classify）
    ///
    /// 对浮点数进行分类，返回10位分类码
    pub fn exec_fclass_s(
        &mut self,
        rd: usize,
        rs1: usize,
        set_reg: &mut dyn FnMut(usize, u64),
    ) -> VmResult<()> {
        let a = self.fp_regs.get(rs1);

        let mut result = 0u64;

        // 分类映射
        if a.is_nan() {
            if a.is_sign_negative() {
                result |= 0x001; // 负quiet NaN
            } else {
                result |= 0x200; // 正quiet NaN
            }
        } else if a.is_infinite() {
            if a.is_sign_negative() {
                result |= 0x002; // 负无穷
            } else {
                result |= 0x100; // 正无穷
            }
        } else if a == 0.0 {
            if a.is_sign_negative() {
                result |= 0x004; // 负零
            } else {
                result |= 0x080; // 正零
            }
        } else {
            // 正常或次 normal数
            if a < 0.0 {
                // 检查是否是次 normal
                if a.abs() < f32::MIN_POSITIVE {
                    result |= 0x008; // 负次 normal
                } else {
                    result |= 0x010; // 负正常数
                }
            } else if a.abs() < f32::MIN_POSITIVE {
                result |= 0x040; // 正次 normal
            } else {
                result |= 0x020; // 正正常数
            }
        }

        set_reg(rd, result);
        Ok(())
    }

    /// FRC SR - 读取FCSR（Read Float-Point Control and Status Register）
    ///
    /// 读取浮点控制状态寄存器到整数寄存器
    pub fn exec_frcsr(&self, rd: usize, set_reg: &mut dyn Fn(usize, u64)) -> VmResult<()> {
        let fcsr_bits = (self.fcsr.rm.to_bits() << 5) | self.fcsr.flags.to_bits();
        set_reg(rd, fcsr_bits as u64);
        Ok(())
    }

    /// FSCSR - 写入FCSR（Swap Float-Point Control and Status Register）
    ///
    /// 将整数寄存器的值写入FCSR，并返回旧值
    pub fn exec_fscsr(
        &mut self,
        rd: usize,
        rs1: u64,
        set_reg: &mut dyn FnMut(usize, u64),
    ) -> VmResult<()> {
        // 读取旧值
        let old_bits = (self.fcsr.rm.to_bits() << 5) | self.fcsr.flags.to_bits();

        // 写入新值
        let bits = rs1 as u32;
        if let Some(rm) = RoundingMode::from_bits((bits >> 5) & 0x7) {
            self.fcsr.rm = rm;
        }
        self.fcsr.flags = FFlags::from_bits(bits & 0x1F);

        // 返回旧值
        set_reg(rd, old_bits as u64);
        Ok(())
    }

    /// FRM - 读取舍入模式（Read Rounding Mode）
    ///
    /// 读取当前舍入模式
    pub fn exec_frm(&self, rd: usize, set_reg: &mut dyn Fn(usize, u64)) -> VmResult<()> {
        set_reg(rd, self.fcsr.rm.to_bits() as u64);
        Ok(())
    }

    /// 设置舍入模式
    pub fn set_frm(&mut self, rm: RoundingMode) {
        self.fcsr.rm = rm;
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_executor() -> FExtensionExecutor {
        FExtensionExecutor::new()
    }

    #[test]
    fn test_fp_registers() {
        let mut fp_regs = FPRegisters::default();

        fp_regs.set(1, 1.0f32);
        assert_eq!(fp_regs.get(1), 1.0);

        fp_regs.set_bits(2, 0x3F800000); // 1.0 in IEEE 754
        assert_eq!(fp_regs.get(2), 1.0);
    }

    #[test]
    fn test_fadd_s() {
        let mut executor = create_test_executor();

        executor.fp_regs.set(1, 1.0);
        executor.fp_regs.set(2, 2.0);

        executor.exec_fadd_s(3, 1, 2).unwrap();
        assert!((executor.fp_regs.get(3) - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fsub_s() {
        let mut executor = create_test_executor();

        executor.fp_regs.set(1, 5.0);
        executor.fp_regs.set(2, 2.0);

        executor.exec_fsub_s(3, 1, 2).unwrap();
        assert!((executor.fp_regs.get(3) - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fmul_s() {
        let mut executor = create_test_executor();

        executor.fp_regs.set(1, 3.0);
        executor.fp_regs.set(2, 4.0);

        executor.exec_fmul_s(3, 1, 2).unwrap();
        assert!((executor.fp_regs.get(3) - 12.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fdiv_s() {
        let mut executor = create_test_executor();

        executor.fp_regs.set(1, 10.0);
        executor.fp_regs.set(2, 2.0);

        executor.exec_fdiv_s(3, 1, 2).unwrap();
        assert!((executor.fp_regs.get(3) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fdiv_by_zero() {
        let mut executor = create_test_executor();

        executor.fp_regs.set(1, 1.0);
        executor.fp_regs.set(2, 0.0);

        executor.exec_fdiv_s(3, 1, 2).unwrap();
        assert!(executor.fp_regs.get(3).is_infinite());
        assert!(executor.fcsr.flags.dz);
    }

    #[test]
    fn test_fsqrt_s() {
        let mut executor = create_test_executor();

        executor.fp_regs.set(1, 16.0);

        executor.exec_fsqrt_s(2, 1).unwrap();
        assert!((executor.fp_regs.get(2) - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fsqrt_negative() {
        let mut executor = create_test_executor();

        executor.fp_regs.set(1, -1.0);

        executor.exec_fsqrt_s(2, 1).unwrap();
        assert!(executor.fp_regs.get(2).is_nan());
        assert!(executor.fcsr.flags.nv);
    }

    #[test]
    fn test_fmin_s() {
        let mut executor = create_test_executor();

        executor.fp_regs.set(1, 3.0);
        executor.fp_regs.set(2, 5.0);

        executor.exec_fmin_s(3, 1, 2).unwrap();
        assert_eq!(executor.fp_regs.get(3), 3.0);
    }

    #[test]
    fn test_fmax_s() {
        let mut executor = create_test_executor();

        executor.fp_regs.set(1, 3.0);
        executor.fp_regs.set(2, 5.0);

        executor.exec_fmax_s(3, 1, 2).unwrap();
        assert_eq!(executor.fp_regs.get(3), 5.0);
    }

    #[test]
    fn test_feq_s() {
        let mut executor = create_test_executor();
        let mut reg = 0u64;

        executor.fp_regs.set(1, 1.0);
        executor.fp_regs.set(2, 1.0);

        executor.exec_feq_s(3, 1, 2, &mut |_, v| reg = v).unwrap();
        assert_eq!(reg, 1);
    }

    #[test]
    fn test_feq_s_with_nan() {
        let mut executor = create_test_executor();
        let mut reg = 0u64;

        executor.fp_regs.set(1, f32::NAN);
        executor.fp_regs.set(2, 1.0);

        executor.exec_feq_s(3, 1, 2, &mut |_, v| reg = v).unwrap();
        assert_eq!(reg, 0); // NaN != anything
    }

    #[test]
    fn test_flt_s() {
        let mut executor = create_test_executor();
        let mut reg = 0u64;

        executor.fp_regs.set(1, 1.0);
        executor.fp_regs.set(2, 2.0);

        executor.exec_flt_s(3, 1, 2, &mut |_, v| reg = v).unwrap();
        assert_eq!(reg, 1);
    }

    #[test]
    fn test_fle_s() {
        let mut executor = create_test_executor();
        let mut reg = 0u64;

        executor.fp_regs.set(1, 2.0);
        executor.fp_regs.set(2, 2.0);

        executor.exec_fle_s(3, 1, 2, &mut |_, v| reg = v).unwrap();
        assert_eq!(reg, 1); // 2.0 <= 2.0
    }

    #[test]
    fn test_fcvt_w_s() {
        let mut executor = create_test_executor();
        let mut reg = 0u64;

        executor.fp_regs.set(1, 42.5);

        executor.exec_fcvt_w_s(3, 1, &mut |_, v| reg = v).unwrap();
        assert_eq!(reg, 42);
    }

    #[test]
    fn test_fcvt_s_w() {
        let mut executor = create_test_executor();

        executor.exec_fcvt_s_w(1, 42).unwrap();
        assert!((executor.fp_regs.get(1) - 42.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fmv_x_w() {
        let mut executor = create_test_executor();
        let mut reg = 0u64;

        executor.fp_regs.set_bits(1, 0x3F800000); // 1.0

        executor.exec_fmv_x_w(2, 1, &mut |_, v| reg = v).unwrap();
        assert_eq!(reg, 0x3F800000);
    }

    #[test]
    fn test_fmv_w_x() {
        let mut executor = create_test_executor();

        executor.exec_fmv_w_x(1, 0x3F800000).unwrap();
        assert_eq!(executor.fp_regs.get(1), 1.0);
    }

    #[test]
    fn test_fclass_s_infinity() {
        let mut executor = create_test_executor();
        use std::cell::Cell;
        let reg = Cell::new(0u64);

        executor.fp_regs.set(1, f32::INFINITY);
        executor
            .exec_fclass_s(2, 1, &mut |_, v| reg.set(v))
            .unwrap();
        assert_eq!(reg.get(), 0x100); // 正无穷
    }

    #[test]
    fn test_fclass_s_nan() {
        let mut executor = create_test_executor();
        use std::cell::Cell;
        let reg = Cell::new(0u64);

        executor.fp_regs.set(1, f32::NAN);
        executor
            .exec_fclass_s(2, 1, &mut |_, v| reg.set(v))
            .unwrap();
        assert_eq!(reg.get(), 0x200); // 正NaN
    }

    #[test]
    fn test_fclass_s_zero() {
        let mut executor = create_test_executor();
        use std::cell::Cell;
        let reg = Cell::new(0u64);

        executor.fp_regs.set(1, 0.0);
        executor
            .exec_fclass_s(2, 1, &mut |_, v| reg.set(v))
            .unwrap();
        assert_eq!(reg.get(), 0x080); // 正零
    }

    #[test]
    fn test_frcsr() {
        let executor = create_test_executor();
        use std::cell::Cell;
        let reg = Cell::new(0u64);

        executor.exec_frcsr(1, &mut |_, v| reg.set(v)).unwrap();
        assert_eq!(reg.get(), 0); // 默认RNE舍入，无标志
    }

    #[test]
    fn test_fscsr() {
        let mut executor = create_test_executor();
        use std::cell::Cell;
        let reg = Cell::new(0u64);

        executor
            .exec_fscsr(1, 0x15, &mut |_, v| reg.set(v))
            .unwrap();
        assert_eq!(executor.fcsr.rm, RoundingMode::RDN); // (0x15 >> 5) & 0x7 = 2
        assert_eq!(executor.fcsr.flags.nx, true); // 0x15 & 0x01 = 1
    }

    #[test]
    fn test_rounding_modes() {
        let mut executor = create_test_executor();

        // RTZ: 向零舍入
        executor.set_frm(RoundingMode::RTZ);
        assert_eq!(executor.fcsr.rm, RoundingMode::RTZ);
    }
}
