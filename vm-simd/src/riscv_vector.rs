//! RISC-V Vector Extension 指令实现
//!
//! 包括向量浮点、比较、归约、掩码操作等

/// RISC-V Vector 向量长度寄存器 (VL)
/// 存储当前向量长度（以元素为单位）
#[derive(Debug, Clone, Copy)]
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

/// RISC-V Vector 向量类型寄存器 (VTYPE)
/// 存储向量类型配置信息
#[derive(Debug, Clone, Copy)]
pub struct VectorType {
    /// SEW: Standard Element Width (元素宽度，以位为单位)
    sew: u8, // 8, 16, 32, 64
    /// LMUL: Vector Register Grouping Multiplier (向量寄存器分组倍数)
    lmul: u8, // 1, 2, 4, 8, 1/2, 1/4, 1/8
    /// SEW/LMUL ratio
    #[allow(dead_code)]
    ratio: u8,
    /// TA: Tail Agnostic (尾部不可知)
    #[allow(dead_code)]
    tail_agnostic: bool,
    /// MA: Mask Agnostic (掩码不可知)
    #[allow(dead_code)]
    mask_agnostic: bool,
}

impl VectorType {
    pub fn new(sew: u8, lmul: u8) -> Self {
        let ratio = sew / lmul;
        Self {
            sew,
            lmul,
            ratio,
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

/// RISC-V Vector 向量寄存器 (V0-V31)
/// 使用可变长度向量，这里简化为固定长度数组
pub type VectorRegister = Vec<u8>;

/// RISC-V Vector 向量掩码寄存器 (v0)
/// 用于条件执行和掩码操作
#[derive(Debug, Clone)]
pub struct VectorMask {
    bits: Vec<bool>,
}

impl VectorMask {
    pub fn new(length: usize) -> Self {
        Self {
            bits: vec![false; length],
        }
    }

    pub fn from_bits(bits: Vec<bool>) -> Self {
        Self { bits }
    }

    pub fn test(&self, i: usize) -> bool {
        if i < self.bits.len() {
            self.bits[i]
        } else {
            false
        }
    }

    pub fn set_bit(&mut self, i: usize, value: bool) {
        if i < self.bits.len() {
            self.bits[i] = value;
        }
    }

    pub fn get_bits(&self) -> &[bool] {
        &self.bits
    }
}

/// RISC-V Vector 向量浮点加法 (VFADD, 128位向量)
/// 128位向量 = 4个单精度浮点数或2个双精度浮点数
pub fn vfadd_f32(a: &[f32; 4], b: &[f32; 4], mask: &VectorMask) -> [f32; 4] {
    let mut result = [0f32; 4];
    for (i, (a_val, b_val)) in a.iter().zip(b.iter()).enumerate() {
        if mask.test(i) {
            result[i] = a_val + b_val;
        } else {
            result[i] = *a_val; // 未激活的元素保持原值
        }
    }
    result
}

/// RISC-V Vector 向量浮点减法 (VFSUB, 128位向量)
pub fn vfsub_f32(a: &[f32; 4], b: &[f32; 4], mask: &VectorMask) -> [f32; 4] {
    let mut result = [0f32; 4];
    for (i, (a_val, b_val)) in a.iter().zip(b.iter()).enumerate() {
        if mask.test(i) {
            result[i] = a_val - b_val;
        } else {
            result[i] = *a_val;
        }
    }
    result
}

/// RISC-V Vector 向量浮点乘法 (VFMUL, 128位向量)
pub fn vfmul_f32(a: &[f32; 4], b: &[f32; 4], mask: &VectorMask) -> [f32; 4] {
    let mut result = [0f32; 4];
    for (i, (a_val, b_val)) in a.iter().zip(b.iter()).enumerate() {
        if mask.test(i) {
            result[i] = a_val * b_val;
        } else {
            result[i] = *a_val;
        }
    }
    result
}

/// RISC-V Vector 向量浮点除法 (VFDIV, 128位向量)
pub fn vfdiv_f32(a: &[f32; 4], b: &[f32; 4], mask: &VectorMask) -> [f32; 4] {
    let mut result = [0f32; 4];
    for (i, (a_val, b_val)) in a.iter().zip(b.iter()).enumerate() {
        if mask.test(i) {
            result[i] = a_val / b_val;
        } else {
            result[i] = *a_val;
        }
    }
    result
}

/// RISC-V Vector 向量浮点比较相等 (VFCMEQ, 128位向量)
pub fn vfcmpeq_f32(a: &[f32; 4], b: &[f32; 4], mask: &VectorMask) -> VectorMask {
    let mut result = VectorMask::new(4);
    for (i, (a_val, b_val)) in a.iter().zip(b.iter()).enumerate() {
        if mask.test(i) {
            result.set_bit(i, a_val == b_val);
        }
    }
    result
}

/// RISC-V Vector 向量浮点比较大于 (VFCMGT, 128位向量)
pub fn vfcmgt_f32(a: &[f32; 4], b: &[f32; 4], mask: &VectorMask) -> VectorMask {
    let mut result = VectorMask::new(4);
    for (i, (a_val, b_val)) in a.iter().zip(b.iter()).enumerate() {
        if mask.test(i) {
            result.set_bit(i, a_val > b_val);
        }
    }
    result
}

/// RISC-V Vector 向量浮点比较大于等于 (VFCMGE, 128位向量)
pub fn vfcmge_f32(a: &[f32; 4], b: &[f32; 4], mask: &VectorMask) -> VectorMask {
    let mut result = VectorMask::new(4);
    for (i, (a_val, b_val)) in a.iter().zip(b.iter()).enumerate() {
        if mask.test(i) {
            result.set_bit(i, a_val >= b_val);
        }
    }
    result
}

/// RISC-V Vector 向量归约求和 (VREDSUM)
pub fn vredsum_f32(a: &[f32], mask: &VectorMask) -> f32 {
    let mut sum = 0f32;
    for (i, val) in a
        .iter()
        .enumerate()
        .take(a.len().min(mask.get_bits().len()))
    {
        if mask.test(i) {
            sum += *val;
        }
    }
    sum
}

/// RISC-V Vector 向量归约最大值 (VREDMAX)
pub fn vredmax_f32(a: &[f32], mask: &VectorMask) -> f32 {
    let mut max_val = f32::NEG_INFINITY;
    let mut found = false;

    for (i, val) in a
        .iter()
        .enumerate()
        .take(a.len().min(mask.get_bits().len()))
    {
        if mask.test(i) && (!found || *val > max_val) {
            max_val = *val;
            found = true;
        }
    }

    if found { max_val } else { 0.0 }
}

/// RISC-V Vector 向量归约最小值 (VREDMIN)
pub fn vredmin_f32(a: &[f32], mask: &VectorMask) -> f32 {
    let mut min_val = f32::INFINITY;
    let mut found = false;

    for (i, val) in a
        .iter()
        .enumerate()
        .take(a.len().min(mask.get_bits().len()))
    {
        if mask.test(i) && (!found || *val < min_val) {
            min_val = *val;
            found = true;
        }
    }

    if found { min_val } else { 0.0 }
}

/// RISC-V Vector 向量比较相等 (VMSEQ)
pub fn vmseq_i32(a: &[i32], b: &[i32], mask: &VectorMask) -> VectorMask {
    let len = a.len().min(b.len()).min(mask.get_bits().len());
    let mut result = VectorMask::new(len);

    for i in 0..len {
        if mask.test(i) {
            result.set_bit(i, a[i] == b[i]);
        }
    }
    result
}

/// RISC-V Vector 向量比较不等 (VMSNE)
pub fn vmsne_i32(a: &[i32], b: &[i32], mask: &VectorMask) -> VectorMask {
    let len = a.len().min(b.len()).min(mask.get_bits().len());
    let mut result = VectorMask::new(len);

    for i in 0..len {
        if mask.test(i) {
            result.set_bit(i, a[i] != b[i]);
        }
    }
    result
}

/// RISC-V Vector 向量比较小于 (VMSLT)
pub fn vmslt_i32(a: &[i32], b: &[i32], mask: &VectorMask) -> VectorMask {
    let len = a.len().min(b.len()).min(mask.get_bits().len());
    let mut result = VectorMask::new(len);

    for i in 0..len {
        if mask.test(i) {
            result.set_bit(i, a[i] < b[i]);
        }
    }
    result
}

/// RISC-V Vector 向量比较小于等于 (VMSLE)
pub fn vmsle_i32(a: &[i32], b: &[i32], mask: &VectorMask) -> VectorMask {
    let len = a.len().min(b.len()).min(mask.get_bits().len());
    let mut result = VectorMask::new(len);

    for i in 0..len {
        if mask.test(i) {
            result.set_bit(i, a[i] <= b[i]);
        }
    }
    result
}

/// RISC-V Vector 向量掩码逻辑与 (VMAND)
pub fn vmand(a: &VectorMask, b: &VectorMask) -> VectorMask {
    let len = a.get_bits().len().min(b.get_bits().len());
    let mut result = VectorMask::new(len);

    for i in 0..len {
        result.set_bit(i, a.test(i) && b.test(i));
    }
    result
}

/// RISC-V Vector 向量掩码逻辑或 (VMOR)
pub fn vmor(a: &VectorMask, b: &VectorMask) -> VectorMask {
    let len = a.get_bits().len().min(b.get_bits().len());
    let mut result = VectorMask::new(len);

    for i in 0..len {
        result.set_bit(i, a.test(i) || b.test(i));
    }
    result
}

/// RISC-V Vector 向量掩码逻辑异或 (VMXOR)
pub fn vmxor(a: &VectorMask, b: &VectorMask) -> VectorMask {
    let len = a.get_bits().len().min(b.get_bits().len());
    let mut result = VectorMask::new(len);

    for i in 0..len {
        result.set_bit(i, a.test(i) ^ b.test(i));
    }
    result
}

/// RISC-V Vector 向量寄存器文件 (V0-V31)
/// 管理32个向量寄存器
pub struct VectorRegisterFile {
    registers: [VectorRegister; 32],
    vl: VectorLength,
    vtype: VectorType,
}

impl VectorRegisterFile {
    pub fn new() -> Self {
        use std::mem::MaybeUninit;
        let mut regs: [MaybeUninit<VectorRegister>; 32] =
            unsafe { MaybeUninit::uninit().assume_init() };
        for reg in &mut regs {
            reg.write(VectorRegister::new());
        }
        let registers: [VectorRegister; 32] = unsafe { std::mem::transmute(regs) };
        Self {
            registers,
            vl: VectorLength::new(0),
            vtype: VectorType::new(32, 1), // 默认: SEW=32, LMUL=1
        }
    }

    pub fn get_vl(&self) -> &VectorLength {
        &self.vl
    }

    pub fn get_vl_mut(&mut self) -> &mut VectorLength {
        &mut self.vl
    }

    pub fn get_vtype(&self) -> &VectorType {
        &self.vtype
    }

    pub fn get_vtype_mut(&mut self) -> &mut VectorType {
        &mut self.vtype
    }

    pub fn get_register(&self, idx: usize) -> Option<&VectorRegister> {
        if idx < 32 {
            Some(&self.registers[idx])
        } else {
            None
        }
    }

    pub fn get_register_mut(&mut self, idx: usize) -> Option<&mut VectorRegister> {
        if idx < 32 {
            Some(&mut self.registers[idx])
        } else {
            None
        }
    }

    pub fn set_register(&mut self, idx: usize, value: VectorRegister) {
        if idx < 32 {
            self.registers[idx] = value;
        }
    }
}

impl Default for VectorRegisterFile {
    fn default() -> Self {
        Self::new()
    }
}
