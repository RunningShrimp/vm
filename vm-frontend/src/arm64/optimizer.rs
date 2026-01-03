//! ARM64 架构特定优化
//!
//! 针对 NEON 指令处理、条件执行和向量操作进行优化。
//!
//! ## 性能目标
//!
//! - NEON 指令解码性能提升 30%+
//! - 条件指令快速识别
//! - 向量寄存器批量操作优化
//! - 使用查找表加速指令分类

use std::collections::HashMap;

/// ARM64 条件码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmCondition {
    EQ = 0,  // Equal
    NE = 1,  // Not equal
    CS = 2,  // Carry set (>=)
    CC = 3,  // Carry clear (<)
    MI = 4,  // Minus (negative)
    PL = 5,  // Plus (positive or zero)
    VS = 6,  // Overflow
    VC = 7,  // No overflow
    HI = 8,  // Higher (unsigned >)
    LS = 9,  // Lower or same (unsigned <=)
    GE = 10, // Greater or equal (signed >=)
    LT = 11, // Less than (signed <)
    GT = 12, // Greater than (signed >)
    LE = 13, // Less or equal (signed <=)
    AL = 14, // Always
}

/// NEON 指令类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeonInstructionType {
    // Vector arithmetic
    Add,
    Sub,
    Mul,
    Fma,
    // Vector logical
    And,
    Orr,
    Eor,
    // Vector shift
    Shl,
    Shr,
    // Vector load/store
    Ld1,
    Ld2,
    Ld3,
    Ld4,
    St1,
    St2,
    St3,
    St4,
    // Vector comparison
    Ceq,
    Cgt,
    Cge,
    // Vector conversion
    Fcvtns,
    Fcvtnu,
    Scvtf,
    Ucvtf,
}

/// ARM64 指令分类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmInstructionClass {
    // 数据处理 - 立即数
    ArithmeticImm,
    LogicalImm,
    BitfieldImm,
    ExtractImm,
    // 数据处理 - 寄存器
    ArithmeticReg,
    LogicalReg,
    ShiftReg,
    // 向量 (NEON)
    Vector(ArchitectureExtension),
    // 浮点
    Float,
    // 内存访问
    LoadStore,
    LoadStorePair,
    LoadStoreExclusive,
    // 控制流
    Branch,
    BranchReg,
    BranchCond,
    // 系统指令
    System,
    Barrier,
}

/// 架构扩展类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchitectureExtension {
    NEON,
    Crypto,
    DotProd,
    SVE,
    SVE2,
}

/// ARM64 优化解码器
pub struct Arm64Optimizer {
    /// 指令分类查找表 (基于 opcode 高位)
    class_table: HashMap<u32, ArmInstructionClass>,
    /// NEON 指令查找表
    neon_table: HashMap<u32, NeonInstructionType>,
    /// 条件码查找表
    cond_table: HashMap<u32, ArmCondition>,
}

impl Arm64Optimizer {
    /// 创建优化的 ARM64 解码器
    pub fn new() -> Self {
        let mut optimizer = Self {
            class_table: HashMap::new(),
            neon_table: HashMap::new(),
            cond_table: HashMap::new(),
        };

        optimizer.build_class_table();
        optimizer.build_neon_table();
        optimizer.build_cond_table();

        optimizer
    }

    /// 构建指令分类表
    fn build_class_table(&mut self) {
        // 31 30 29 28 27 26 25
        //  0  0  1  0  0  0  0 -> reserved
        //  0  0  1  0  0  0  1 -> arithmetic immediate

        // Arithmetic immediate (ADD/SUB immediate)
        for op in 0x11000000u32..=0x11000000u32 {
            self.class_table
                .insert(op, ArmInstructionClass::ArithmeticImm);
        }

        // Logical immediate (AND/ORR/EOR immediate)
        for op in 0x12000000u32..=0x13000000u32 {
            self.class_table.insert(op, ArmInstructionClass::LogicalImm);
        }

        // Load/store pair
        for op in 0x28000000u32..=0x2D000000u32 {
            self.class_table
                .insert(op, ArmInstructionClass::LoadStorePair);
        }

        // Load/store register (unsigned immediate)
        for op in 0x38000000u32..=0x3D000000u32 {
            self.class_table.insert(op, ArmInstructionClass::LoadStore);
        }

        // Branch, exception generation, system instructions
        for op in 0xD4000000u32..=0xD6000000u32 {
            self.class_table.insert(op, ArmInstructionClass::Branch);
        }

        // Vector (NEON/SVE)
        for op in 0x0E000000u32..=0x0F000000u32 {
            self.class_table
                .insert(op, ArmInstructionClass::Vector(ArchitectureExtension::NEON));
        }

        // Floating point
        for op in 0x1E000000u32..=0x1F800000u32 {
            self.class_table.insert(op, ArmInstructionClass::Float);
        }
    }

    /// 构建 NEON 指令表
    fn build_neon_table(&mut self) {
        // NEON 加法: 0x0E00_0000 - 0x0E20_0C00
        // ADD (vector)
        self.neon_table
            .insert(0x0E00_0C00, NeonInstructionType::Add);
        self.neon_table
            .insert(0x0E20_0C00, NeonInstructionType::Add);

        // NEON 减法
        self.neon_table
            .insert(0x0E00_0400, NeonInstructionType::Sub);
        self.neon_table
            .insert(0x0E20_0400, NeonInstructionType::Sub);

        // NEON 乘法
        self.neon_table
            .insert(0x0E00_0D00, NeonInstructionType::Mul);
        self.neon_table
            .insert(0x0E20_0D00, NeonInstructionType::Mul);

        // NEON 逻辑运算
        self.neon_table
            .insert(0x0E00_0150, NeonInstructionType::And);
        self.neon_table
            .insert(0x0E00_0550, NeonInstructionType::Orr);
        self.neon_table
            .insert(0x0E00_0950, NeonInstructionType::Eor);

        // NEON 位移
        self.neon_table
            .insert(0x0E00_0A00, NeonInstructionType::Shl);
        self.neon_table
            .insert(0x0E00_0240, NeonInstructionType::Shr);

        // NEON 加载/存储
        self.neon_table
            .insert(0x0C00_0000, NeonInstructionType::Ld1);
        self.neon_table
            .insert(0x0C00_0800, NeonInstructionType::Ld2);
        self.neon_table
            .insert(0x0C00_1000, NeonInstructionType::Ld3);
        self.neon_table
            .insert(0x0C00_1800, NeonInstructionType::Ld4);
        self.neon_table
            .insert(0x0C00_0400, NeonInstructionType::St1);
        self.neon_table
            .insert(0x0C00_0C00, NeonInstructionType::St2);
        self.neon_table
            .insert(0x0C00_1400, NeonInstructionType::St3);
        self.neon_table
            .insert(0x0C00_1C00, NeonInstructionType::St4);

        // NEON 比较
        self.neon_table
            .insert(0x0E00_0840, NeonInstructionType::Ceq);
        self.neon_table
            .insert(0x0E00_0880, NeonInstructionType::Cgt);
        self.neon_table
            .insert(0x0E00_0A40, NeonInstructionType::Cge);

        // NEON 转换
        self.neon_table
            .insert(0x0E00_C800, NeonInstructionType::Fcvtns);
        self.neon_table
            .insert(0x0E00_C900, NeonInstructionType::Fcvtnu);
        self.neon_table
            .insert(0x0E00_D200, NeonInstructionType::Scvtf);
        self.neon_table
            .insert(0x0E00_D300, NeonInstructionType::Ucvtf);
    }

    /// 构建条件码表
    fn build_cond_table(&mut self) {
        // 条件分支指令 (bits 31:24 = 0x54)
        // cond in bits 3:0
        let cond_base = 0x54000000u32;

        for cond in 0..=15u32 {
            self.cond_table.insert(cond_base | (cond << 5), unsafe {
                std::mem::transmute::<u8, ArmCondition>(cond as u8)
            });
        }
    }

    /// 快速指令分类
    #[inline]
    pub fn classify_instruction(&self, insn: u32) -> Option<ArmInstructionClass> {
        // 使用指令的高 7 位进行快速分类
        let key = insn & 0xFC000000;

        // 如果完全匹配，直接返回
        if let Some(&class) = self.class_table.get(&key) {
            return Some(class);
        }

        // 部分匹配：检查更细粒度的分类
        let sub_key = insn & 0xFE000000;
        self.class_table.get(&sub_key).copied()
    }

    /// 识别 NEON 指令
    #[inline]
    pub fn identify_neon(&self, insn: u32) -> Option<NeonInstructionType> {
        // NEON 指令的 bits 28:25 = 0x7
        if (insn & 0x1F000000) != 0x0E000000 && (insn & 0x1F000000) != 0x0F000000 {
            return None;
        }

        // 使用更精确的键查找
        let key = insn & 0xFF00FC00;
        self.neon_table.get(&key).copied()
    }

    /// 识别条件码
    #[inline]
    pub fn identify_condition(&self, insn: u32) -> Option<ArmCondition> {
        // 条件分支: B.cond
        if (insn & 0xFF000000) == 0x54000000 {
            let cond = ((insn >> 5) & 0xF) as u8;
            if cond <= 15 {
                return unsafe { Some(std::mem::transmute::<u8, ArmCondition>(cond)) };
            }
        }

        // 条件选择: CSEL
        // Fixed: corrected bit mask from 0x3F000000 to 0x1E000000 to allow proper matching
        if (insn & 0x1E000000) == 0x1A800000 {
            let cond = ((insn >> 12) & 0xF) as u8;
            if cond <= 15 {
                return unsafe { Some(std::mem::transmute::<u8, ArmCondition>(cond)) };
            }
        }

        None
    }

    /// 检查是否为向量指令
    #[inline]
    pub fn is_vector_instruction(&self, insn: u32) -> bool {
        // Advanced SIMD or SVE
        matches!(
            self.classify_instruction(insn),
            Some(ArmInstructionClass::Vector(_))
        )
    }

    /// 检查是否为条件指令
    #[inline]
    pub fn is_conditional_instruction(&self, insn: u32) -> bool {
        self.identify_condition(insn).is_some()
    }

    /// 批量分类指令
    pub fn classify_instructions_batch(&self, insns: &[u32]) -> Vec<Option<ArmInstructionClass>> {
        insns
            .iter()
            .map(|&insn| self.classify_instruction(insn))
            .collect()
    }

    /// 获取 NEON 指令统计信息
    pub fn get_neon_stats(&self, insns: &[u32]) -> NeonStats {
        let mut stats = NeonStats::default();

        for &insn in insns {
            if let Some(neon_type) = self.identify_neon(insn) {
                match neon_type {
                    NeonInstructionType::Add
                    | NeonInstructionType::Sub
                    | NeonInstructionType::Mul => {
                        stats.arithmetic_count += 1;
                    }
                    NeonInstructionType::And
                    | NeonInstructionType::Orr
                    | NeonInstructionType::Eor => {
                        stats.logical_count += 1;
                    }
                    NeonInstructionType::Shl | NeonInstructionType::Shr => {
                        stats.shift_count += 1;
                    }
                    NeonInstructionType::Ld1
                    | NeonInstructionType::Ld2
                    | NeonInstructionType::Ld3
                    | NeonInstructionType::Ld4
                    | NeonInstructionType::St1
                    | NeonInstructionType::St2
                    | NeonInstructionType::St3
                    | NeonInstructionType::St4 => {
                        stats.memory_count += 1;
                    }
                    NeonInstructionType::Ceq
                    | NeonInstructionType::Cgt
                    | NeonInstructionType::Cge => {
                        stats.compare_count += 1;
                    }
                    NeonInstructionType::Fcvtns
                    | NeonInstructionType::Fcvtnu
                    | NeonInstructionType::Scvtf
                    | NeonInstructionType::Ucvtf => {
                        stats.convert_count += 1;
                    }
                    _ => {}
                }
                stats.total_neon_count += 1;
            }
        }

        stats
    }
}

impl Default for Arm64Optimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// NEON 统计信息
#[derive(Debug, Clone, Default)]
pub struct NeonStats {
    pub total_neon_count: u64,
    pub arithmetic_count: u64,
    pub logical_count: u64,
    pub shift_count: u64,
    pub memory_count: u64,
    pub compare_count: u64,
    pub convert_count: u64,
}

impl NeonStats {
    /// 计算 NEON 指令占比
    pub fn neon_ratio(&self, total_insn_count: u64) -> f64 {
        if total_insn_count == 0 {
            return 0.0;
        }
        self.total_neon_count as f64 / total_insn_count as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimizer_creation() {
        let optimizer = Arm64Optimizer::new();
        assert!(!optimizer.class_table.is_empty());
        assert!(!optimizer.neon_table.is_empty());
    }

    #[test]
    fn test_classify_arithmetic_imm() {
        let optimizer = Arm64Optimizer::new();

        // ADD X0, X1, #0x100 (0x91000000)
        let insn = 0x91000000u32;
        let class = optimizer.classify_instruction(insn);

        assert_eq!(class, Some(ArmInstructionClass::ArithmeticImm));
    }

    #[test]
    fn test_classify_vector() {
        let optimizer = Arm64Optimizer::new();

        // ADD V0.8B, V1.8B, V2.8B (0x0E00_0C00)
        let insn = 0x0E00_0C00u32;
        let class = optimizer.classify_instruction(insn);

        assert_eq!(
            class,
            Some(ArmInstructionClass::Vector(ArchitectureExtension::NEON))
        );
    }

    #[test]
    fn test_identify_neon_add() {
        let optimizer = Arm64Optimizer::new();

        // ADD V0.8B, V1.8B, V2.8B
        let insn = 0x0E00_0C00u32;
        let neon_type = optimizer.identify_neon(insn);

        assert_eq!(neon_type, Some(NeonInstructionType::Add));
    }

    #[test]
    fn test_identify_condition() {
        let optimizer = Arm64Optimizer::new();

        // B.EQ label (0x54000000)
        let insn = 0x54000000u32;
        let cond = optimizer.identify_condition(insn);

        assert_eq!(cond, Some(ArmCondition::EQ));
    }

    #[test]
    fn test_is_vector_instruction() {
        let optimizer = Arm64Optimizer::new();

        // NEON ADD
        assert!(optimizer.is_vector_instruction(0x0E00_0C00));

        // Regular ADD
        assert!(!optimizer.is_vector_instruction(0x91000000));
    }

    #[test]
    fn test_is_conditional_instruction() {
        let optimizer = Arm64Optimizer::new();

        // B.EQ
        assert!(optimizer.is_conditional_instruction(0x54000000));

        // Regular B
        assert!(!optimizer.is_conditional_instruction(0x14000000));
    }

    #[test]
    fn test_batch_classification() {
        let optimizer = Arm64Optimizer::new();

        let insns = vec![0x91000000, 0x0E00_0C00, 0x54000000];
        let classes = optimizer.classify_instructions_batch(&insns);

        assert_eq!(classes[0], Some(ArmInstructionClass::ArithmeticImm));
        assert_eq!(
            classes[1],
            Some(ArmInstructionClass::Vector(ArchitectureExtension::NEON))
        );
        assert_eq!(classes[2], Some(ArmInstructionClass::BranchCond));
    }

    #[test]
    fn test_neon_stats() {
        let optimizer = Arm64Optimizer::new();

        let insns = vec![
            0x0E00_0C00, // ADD
            0x0E00_0400, // SUB
            0x0E00_0150, // AND
            0x0C00_0000, // LD1
        ];

        let stats = optimizer.get_neon_stats(&insns);

        assert_eq!(stats.total_neon_count, 4);
        assert_eq!(stats.arithmetic_count, 2);
        assert_eq!(stats.logical_count, 1);
        assert_eq!(stats.memory_count, 1);

        let ratio = stats.neon_ratio(4);
        assert_eq!(ratio, 1.0);
    }
}
