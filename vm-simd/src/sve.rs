//! ARM SVE (Scalable Vector Extension) 指令实现
//!
//! 包括谓词寄存器、向量加载/存储、向量算术、归约等

/// SVE 谓词寄存器 (P0-P15)
/// 每个谓词寄存器可以控制可变数量的向量元素
#[derive(Debug, Clone, Copy)]
pub struct SvePredicate {
    /// 谓词位掩码 (最多支持 256 位，实际长度由 VL 决定)
    bits: u64, // 简化实现，实际应该支持可变长度
    length: usize, // 有效长度（以位为单位）
}

impl SvePredicate {
    pub fn new(length: usize) -> Self {
        Self {
            bits: 0,
            length: length.min(64), // 简化实现限制为64位
        }
    }

    /// PTRUE: 设置所有位为真
    pub fn ptrue(length: usize) -> Self {
        let len = length.min(64);
        Self {
            bits: (1u64 << len) - 1,
            length: len,
        }
    }

    /// PFALSE: 设置所有位为假
    pub fn pfalse(length: usize) -> Self {
        Self {
            bits: 0,
            length: length.min(64),
        }
    }

    /// PAND: 谓词逻辑与
    pub fn pand(&self, other: &Self) -> Self {
        Self {
            bits: self.bits & other.bits,
            length: self.length.max(other.length),
        }
    }

    /// PORR: 谓词逻辑或
    pub fn porr(&self, other: &Self) -> Self {
        Self {
            bits: self.bits | other.bits,
            length: self.length.max(other.length),
        }
    }

    /// 检查第 i 位是否设置
    pub fn test(&self, i: usize) -> bool {
        if i >= self.length {
            return false;
        }
        (self.bits >> i) & 1 != 0
    }

    /// 设置第 i 位
    pub fn set_bit(&mut self, i: usize, value: bool) {
        if i < self.length {
            if value {
                self.bits |= 1 << i;
            } else {
                self.bits &= !(1 << i);
            }
        }
    }

    pub fn get_bits(&self) -> u64 {
        self.bits
    }

    pub fn get_length(&self) -> usize {
        self.length
    }
}

/// SVE 向量寄存器 (V0-V31)
/// 使用可变长度向量，这里简化为固定长度数组
pub type SveVector = Vec<u8>;

/// SVE 向量长度寄存器 (VL)
/// 存储当前向量长度（以字节为单位）
pub struct VectorLength {
    vl: usize,
}

impl VectorLength {
    pub fn new(vl: usize) -> Self {
        Self { vl }
    }

    pub fn get(&self) -> usize {
        self.vl
    }

    pub fn set(&mut self, vl: usize) {
        self.vl = vl;
    }
}

/// SVE 向量类型寄存器 (VTYPE)
/// 存储向量类型配置信息
#[derive(Debug, Clone, Copy)]
pub struct VectorType {
    /// SEW: Standard Element Width (元素宽度，以位为单位)
    sew: u8, // 8, 16, 32, 64
    /// LMUL: Vector Register Grouping Multiplier (向量寄存器分组倍数)
    lmul: u8, // 1, 2, 4, 8, 1/2, 1/4, 1/8
    /// TA: Tail Agnostic (尾部不可知)
    #[allow(dead_code)]
    tail_agnostic: bool,
    /// MA: Mask Agnostic (掩码不可知)
    #[allow(dead_code)]
    mask_agnostic: bool,
}

impl VectorType {
    pub fn new(sew: u8, lmul: u8) -> Self {
        Self {
            sew,
            lmul,
            tail_agnostic: false,
            mask_agnostic: false,
        }
    }

    pub fn get_sew(&self) -> u8 {
        self.sew
    }

    pub fn get_lmul(&self) -> u8 {
        self.lmul
    }

    pub fn get_vlmax(&self) -> usize {
        // VLMAX = (VLEN / SEW) * LMUL
        // 简化实现，假设 VLEN = 128
        (128 / self.sew as usize) * self.lmul as usize
    }
}

/// SVE 特性检测
/// 读取 ID_AA64PFR0_EL1 寄存器检查 SVE 支持
#[cfg(target_arch = "aarch64")]
pub fn detect_sve() -> bool {
    unsafe {
        // 读取 ID_AA64PFR0_EL1 寄存器
        // 位 [35:32] 表示 SVE 支持
        let id_aa64pfr0_el1: u64;
        std::arch::asm!(
            "mrs {}, ID_AA64PFR0_EL1",
            out(reg) id_aa64pfr0_el1,
        );

        // 检查 SVE 字段 (bits [35:32])
        let sve_field = (id_aa64pfr0_el1 >> 32) & 0xF;
        sve_field != 0
    }
}

#[cfg(not(target_arch = "aarch64"))]
pub fn detect_sve() -> bool {
    false
}

/// SVE 向量加载字节 (LD1B)
/// 从内存加载字节向量
///
/// # Safety
///
/// 调用者必须确保 `base` 指向至少 `offset + vl.get()` 字节的有效内存。
pub unsafe fn ld1b(
    base: *const u8,
    offset: isize,
    pred: &SvePredicate,
    vl: &VectorLength,
) -> SveVector {
    let mut result = Vec::with_capacity(vl.get());
    for i in 0..vl.get() {
        if pred.test(i) {
            unsafe {
                result.push(*base.add(offset as usize + i));
            }
        } else {
            result.push(0); // 未激活的元素设为0
        }
    }
    result
}

/// SVE 向量加载半字 (LD1H)
///
/// # Safety
///
/// 调用者必须确保 `base` 指向至少 `(offset / 2) + (vl.get() / 2)` 个半字的有效内存。
pub unsafe fn ld1h(
    base: *const u16,
    offset: isize,
    pred: &SvePredicate,
    vl: &VectorLength,
) -> Vec<u16> {
    let mut result = Vec::with_capacity(vl.get() / 2);
    for i in 0..(vl.get() / 2) {
        if pred.test(i) {
            unsafe {
                result.push(*base.add((offset as usize / 2) + i));
            }
        } else {
            result.push(0);
        }
    }
    result
}

/// SVE 向量加载字 (LD1W)
///
/// # Safety
///
/// 调用者必须确保 `base` 指向至少 `(offset / 4) + (vl.get() / 4)` 个字的有效内存。
pub unsafe fn ld1w(
    base: *const u32,
    offset: isize,
    pred: &SvePredicate,
    vl: &VectorLength,
) -> Vec<u32> {
    let mut result = Vec::with_capacity(vl.get() / 4);
    for i in 0..(vl.get() / 4) {
        if pred.test(i) {
            unsafe {
                result.push(*base.add((offset as usize / 4) + i));
            }
        } else {
            result.push(0);
        }
    }
    result
}

/// SVE 向量加载双字 (LD1D)
///
/// # Safety
///
/// 调用者必须确保 `base` 指向至少 `(offset / 8) + (vl.get() / 8)` 个双字的有效内存。
pub unsafe fn ld1d(
    base: *const u64,
    offset: isize,
    pred: &SvePredicate,
    vl: &VectorLength,
) -> Vec<u64> {
    let mut result = Vec::with_capacity(vl.get() / 8);
    for i in 0..(vl.get() / 8) {
        if pred.test(i) {
            unsafe {
                result.push(*base.add((offset as usize / 8) + i));
            }
        } else {
            result.push(0);
        }
    }
    result
}

/// SVE 向量加法 (支持谓词掩码)
pub fn sve_add(a: &[u32], b: &[u32], pred: &SvePredicate) -> Vec<u32> {
    let len = a.len().min(b.len());
    let mut result = Vec::with_capacity(len);

    for i in 0..len {
        if pred.test(i) {
            result.push(a[i].wrapping_add(b[i]));
        } else {
            result.push(a[i]); // 未激活的元素保持原值
        }
    }
    result
}

/// SVE 向量减法 (支持谓词掩码)
pub fn sve_sub(a: &[u32], b: &[u32], pred: &SvePredicate) -> Vec<u32> {
    let len = a.len().min(b.len());
    let mut result = Vec::with_capacity(len);

    for i in 0..len {
        if pred.test(i) {
            result.push(a[i].wrapping_sub(b[i]));
        } else {
            result.push(a[i]);
        }
    }
    result
}

/// SVE 向量乘法 (支持谓词掩码)
pub fn sve_mul(a: &[u32], b: &[u32], pred: &SvePredicate) -> Vec<u32> {
    let len = a.len().min(b.len());
    let mut result = Vec::with_capacity(len);

    for i in 0..len {
        if pred.test(i) {
            result.push(a[i].wrapping_mul(b[i]));
        } else {
            result.push(a[i]);
        }
    }
    result
}

/// SVE 向量归约加法 (ADDV)
pub fn sve_addv(a: &[u32], pred: &SvePredicate) -> u32 {
    let mut sum = 0u32;
    for (i, &value) in a.iter().enumerate() {
        if pred.test(i) {
            sum = sum.wrapping_add(value);
        }
    }
    sum
}

/// SVE 向量归约最大值 (SMAXV)
pub fn sve_smaxv(a: &[i32], pred: &SvePredicate) -> i32 {
    let mut max_val = i32::MIN;
    let mut found = false;

    for (i, &value) in a.iter().enumerate() {
        if pred.test(i) && (!found || value > max_val) {
            max_val = value;
            found = true;
        }
    }

    if found {
        max_val
    } else {
        0 // 如果没有激活的元素，返回0
    }
}

/// SVE 向量归约最小值 (SMINV)
pub fn sve_sminv(a: &[i32], pred: &SvePredicate) -> i32 {
    let mut min_val = i32::MAX;
    let mut found = false;

    for (i, &val) in a.iter().enumerate() {
        if pred.test(i) && (!found || val < min_val) {
            min_val = val;
            found = true;
        }
    }

    if found { min_val } else { 0 }
}

/// SVE 向量压缩
/// 根据谓词压缩向量，只保留激活的元素
pub fn sve_compress(a: &[u32], pred: &SvePredicate) -> Vec<u32> {
    let mut result = Vec::new();
    for (i, &val) in a.iter().enumerate() {
        if pred.test(i) {
            result.push(val);
        }
    }
    result
}

/// SVE 向量解压缩
/// 根据谓词解压缩向量，将压缩的元素展开
pub fn sve_decompress(compressed: &[u32], pred: &SvePredicate, original_len: usize) -> Vec<u32> {
    let mut result = vec![0u32; original_len];
    let mut comp_idx = 0;

    for (i, val) in result.iter_mut().enumerate().take(original_len) {
        if pred.test(i) && comp_idx < compressed.len() {
            *val = compressed[comp_idx];
            comp_idx += 1;
        }
    }
    result
}
