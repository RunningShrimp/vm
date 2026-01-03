//! RISC-V 架构特定优化
//!
//! 针对压缩指令处理、CSR 访问和向量扩展进行优化。
//!
//! ## 性能目标
//!
//! - 压缩指令 (RVC) 解码性能提升 30%+
//! - CSR 访问优化
//! - 向量扩展 (RVV) 支持
//! - 使用查找表加速指令分类

use std::collections::HashMap;

/// RISC-V 指令格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiscVFormat {
    R,   // Register
    I,   // Immediate
    S,   // Store
    B,   // Branch
    U,   // Upper immediate
    J,   // Jump
    CR,  // Compressed Register (RVC)
    CI,  // Compressed Immediate (RVC)
    CSS, // Compressed Store (RVC)
    CIW, // Compressed Wide Immediate (RVC)
    CL,  // Compressed Load (RVC)
    CS,  // Compressed Store (RVC)
    CB,  // Compressed Branch (RVC)
    CJ,  // Compressed Jump (RVC)
    V,   // Vector (RVV)
}

/// RISC-V 压缩指令类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressedInsnType {
    // C.ADD, C.MV, C.ADDI, etc.
    Arithmetic,
    // C.LWSP, C.LW, C.SWSP, C.SW
    LoadStore,
    // C.BEQZ, C.BNEZ
    Branch,
    // C.J, C.JAL, C.JR, C.JALR
    Jump,
    // C.LI, C.LUI
    Immediate,
    // C.SLLI, C.SRLI, C.SRAI
    Shift,
}

/// RISC-V CSR 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsrRegister {
    // Machine CSRs
    Mstatus,
    Misa,
    Medeleg,
    Mideleg,
    Mie,
    Mtvec,
    Mscratch,
    Mepc,
    Mcause,
    Mtval,
    Mip,
    // Supervisor CSRs
    Sstatus,
    Sedeleg,
    Sideleg,
    Sie,
    Stvec,
    Sscratch,
    Sepc,
    Scause,
    Stval,
    Sip,
    Satp,
    // User CSRs
    Ustatus,
    Uie,
    Utvec,
    Uscratch,
    Uepc,
    Ucause,
    Utval,
    Uip,
    // Floating-point CSRs
    Fflags,
    Frm,
    Fcsr,
    // Custom
    Custom(u16),
}

/// 向量指令类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorInsnType {
    // Vector arithmetic
    Vadd,
    Vsub,
    Vmul,
    Vdiv,
    Vrem,
    // Vector logical
    Vand,
    Vor,
    Vxor,
    // Vector shift
    Vsll,
    Vsrl,
    Vsra,
    // Vector compare
    Vmseq,
    Vmsne,
    Vmsltu,
    Vmslt,
    Vmsleu,
    Vmsle,
    // Vector load/store
    Vle,
    Vse,
    Vlm,
    Vsm,
    // Vector conversion
    Vfcvt,
    // Vector configuration
    Vsetvli,
    Vsetvl,
}

/// RISC-V 优化解码器
pub struct RiscvOptimizer {
    /// 压缩指令查找表
    compressed_table: HashMap<u16, CompressedInsnType>,
    /// CSR 寄存器表
    csr_table: HashMap<u16, CsrRegister>,
    /// 向量指令表
    vector_table: HashMap<u32, VectorInsnType>,
    /// 指令格式表
    format_table: HashMap<u8, RiscVFormat>,
}

impl RiscvOptimizer {
    /// 创建优化的 RISC-V 解码器
    pub fn new() -> Self {
        let mut optimizer = Self {
            compressed_table: HashMap::new(),
            csr_table: HashMap::new(),
            vector_table: HashMap::new(),
            format_table: HashMap::new(),
        };

        optimizer.build_compressed_table();
        optimizer.build_csr_table();
        optimizer.build_vector_table();
        optimizer.build_format_table();

        optimizer
    }

    /// 构建压缩指令表
    fn build_compressed_table(&mut self) {
        // RVC 指令的 bits [1:0] 必须不是 11
        // 使用 opcode (bits [15:13]) 进行快速查找

        // C.ADDI4SPN (opcode 000)
        for funct3 in 0..=7u16 {
            self.compressed_table
                .insert(funct3 << 13, CompressedInsnType::Immediate);
        }

        // C.LW (opcode 010)
        self.compressed_table
            .insert(0x4000, CompressedInsnType::LoadStore);
        self.compressed_table
            .insert(0x4002, CompressedInsnType::LoadStore);
        self.compressed_table
            .insert(0x4004, CompressedInsnType::LoadStore);
        self.compressed_table
            .insert(0x4006, CompressedInsnType::LoadStore);

        // C.SW (opcode 110)
        self.compressed_table
            .insert(0xC000, CompressedInsnType::LoadStore);
        self.compressed_table
            .insert(0xC002, CompressedInsnType::LoadStore);
        self.compressed_table
            .insert(0xC004, CompressedInsnType::LoadStore);
        self.compressed_table
            .insert(0xC006, CompressedInsnType::LoadStore);

        // C.ADDI (opcode 001)
        self.compressed_table
            .insert(0x1000, CompressedInsnType::Immediate);
        self.compressed_table
            .insert(0x1002, CompressedInsnType::Immediate);
        self.compressed_table
            .insert(0x1004, CompressedInsnType::Immediate);
        self.compressed_table
            .insert(0x1006, CompressedInsnType::Immediate);

        // C.LI (opcode 010 with rd=0)
        self.compressed_table
            .insert(0x4001, CompressedInsnType::Immediate);
        self.compressed_table
            .insert(0x4003, CompressedInsnType::Immediate);
        self.compressed_table
            .insert(0x4005, CompressedInsnType::Immediate);

        // C.LUI (opcode 011)
        self.compressed_table
            .insert(0x6001, CompressedInsnType::Immediate);
        self.compressed_table
            .insert(0x6003, CompressedInsnType::Immediate);
        self.compressed_table
            .insert(0x6005, CompressedInsnType::Immediate);

        // C.SRLI (opcode 100)
        self.compressed_table
            .insert(0x8001, CompressedInsnType::Shift);
        self.compressed_table
            .insert(0x8003, CompressedInsnType::Shift);
        self.compressed_table
            .insert(0x8005, CompressedInsnType::Shift);

        // C.BEQZ (opcode 110)
        self.compressed_table
            .insert(0xC001, CompressedInsnType::Branch);
        self.compressed_table
            .insert(0xC003, CompressedInsnType::Branch);
        self.compressed_table
            .insert(0xC005, CompressedInsnType::Branch);
        self.compressed_table
            .insert(0xC007, CompressedInsnType::Branch);

        // C.BNEZ (opcode 111)
        self.compressed_table
            .insert(0xE001, CompressedInsnType::Branch);
        self.compressed_table
            .insert(0xE003, CompressedInsnType::Branch);
        self.compressed_table
            .insert(0xE005, CompressedInsnType::Branch);
        self.compressed_table
            .insert(0xE007, CompressedInsnType::Branch);
    }

    /// 构建 CSR 寄存器表
    fn build_csr_table(&mut self) {
        // Machine CSRs (0x300 - 0x3FF)
        self.csr_table.insert(0x300, CsrRegister::Mstatus);
        self.csr_table.insert(0x301, CsrRegister::Misa);
        self.csr_table.insert(0x302, CsrRegister::Medeleg);
        self.csr_table.insert(0x303, CsrRegister::Mideleg);
        self.csr_table.insert(0x304, CsrRegister::Mie);
        self.csr_table.insert(0x305, CsrRegister::Mtvec);
        self.csr_table.insert(0x340, CsrRegister::Mscratch);
        self.csr_table.insert(0x341, CsrRegister::Mepc);
        self.csr_table.insert(0x342, CsrRegister::Mcause);
        self.csr_table.insert(0x343, CsrRegister::Mtval);
        self.csr_table.insert(0x344, CsrRegister::Mip);

        // Supervisor CSRs (0x100 - 0x1FF)
        self.csr_table.insert(0x100, CsrRegister::Sstatus);
        self.csr_table.insert(0x102, CsrRegister::Sedeleg);
        self.csr_table.insert(0x103, CsrRegister::Sideleg);
        self.csr_table.insert(0x104, CsrRegister::Sie);
        self.csr_table.insert(0x105, CsrRegister::Stvec);
        self.csr_table.insert(0x140, CsrRegister::Sscratch);
        self.csr_table.insert(0x141, CsrRegister::Sepc);
        self.csr_table.insert(0x142, CsrRegister::Scause);
        self.csr_table.insert(0x143, CsrRegister::Stval);
        self.csr_table.insert(0x144, CsrRegister::Sip);
        self.csr_table.insert(0x180, CsrRegister::Satp);

        // User CSRs (0x000 - 0x0FF)
        self.csr_table.insert(0x000, CsrRegister::Ustatus);
        self.csr_table.insert(0x004, CsrRegister::Uie);
        self.csr_table.insert(0x005, CsrRegister::Utvec);
        self.csr_table.insert(0x040, CsrRegister::Uscratch);
        self.csr_table.insert(0x041, CsrRegister::Uepc);
        self.csr_table.insert(0x042, CsrRegister::Ucause);
        self.csr_table.insert(0x043, CsrRegister::Utval);
        self.csr_table.insert(0x044, CsrRegister::Uip);

        // Floating-point CSRs (0x001 - 0x003)
        self.csr_table.insert(0x001, CsrRegister::Fflags);
        self.csr_table.insert(0x002, CsrRegister::Frm);
        self.csr_table.insert(0x003, CsrRegister::Fcsr);
    }

    /// 构建向量指令表
    fn build_vector_table(&mut self) {
        // Vector configuration
        self.vector_table
            .insert(0x00000007, VectorInsnType::Vsetvli);
        self.vector_table.insert(0x80000007, VectorInsnType::Vsetvl);

        // Vector integer arithmetic
        for funct6 in 0..=0x3Fu32 {
            self.vector_table.insert(
                (funct6 << 26) | 0x00000057,
                match funct6 {
                    0b000010 => VectorInsnType::Vadd,
                    0b000110 => VectorInsnType::Vsub,
                    0b000101 => VectorInsnType::Vmul,
                    0b000001 => VectorInsnType::Vdiv,
                    0b000011 => VectorInsnType::Vrem,
                    0b000111 => VectorInsnType::Vand,
                    0b001111 => VectorInsnType::Vor,
                    0b001011 => VectorInsnType::Vxor,
                    _ => VectorInsnType::Vadd, // 默认
                },
            );
        }

        // Vector shift
        self.vector_table.insert(0xA0000057, VectorInsnType::Vsll);
        self.vector_table.insert(0xC0000057, VectorInsnType::Vsrl);
        self.vector_table.insert(0xE0000057, VectorInsnType::Vsra);

        // Vector compare
        self.vector_table.insert(0x60000057, VectorInsnType::Vmseq);
        self.vector_table.insert(0x70000057, VectorInsnType::Vmsne);
        self.vector_table.insert(0x60040057, VectorInsnType::Vmsltu);
        self.vector_table.insert(0x60050057, VectorInsnType::Vmslt);
        self.vector_table.insert(0x60060057, VectorInsnType::Vmsleu);
        self.vector_table.insert(0x60070057, VectorInsnType::Vmsle);

        // Vector load/store
        self.vector_table.insert(0x00000007, VectorInsnType::Vle); // VLE8
        self.vector_table.insert(0x00002007, VectorInsnType::Vle); // VLE16
        self.vector_table.insert(0x00004007, VectorInsnType::Vle); // VLE32
        self.vector_table.insert(0x00006007, VectorInsnType::Vle); // VLE64

        self.vector_table.insert(0x00001007, VectorInsnType::Vse); // VSE8
        self.vector_table.insert(0x00003007, VectorInsnType::Vse); // VSE16
        self.vector_table.insert(0x00005007, VectorInsnType::Vse); // VSE32
        self.vector_table.insert(0x00007007, VectorInsnType::Vse); // VSE64
    }

    /// 构建指令格式表
    fn build_format_table(&mut self) {
        // Standard instruction formats (bits [6:2] = opcode)
        self.format_table.insert(0x13, RiscVFormat::I); // OP-IMM
        self.format_table.insert(0x33, RiscVFormat::R); // OP
        self.format_table.insert(0x03, RiscVFormat::I); // LOAD
        self.format_table.insert(0x23, RiscVFormat::S); // STORE
        self.format_table.insert(0x63, RiscVFormat::B); // BRANCH
        self.format_table.insert(0x37, RiscVFormat::U); // LUI
        self.format_table.insert(0x17, RiscVFormat::U); // AUIPC
        self.format_table.insert(0x6F, RiscVFormat::J); // JAL
        self.format_table.insert(0x67, RiscVFormat::I); // JALR
        self.format_table.insert(0x73, RiscVFormat::I); // SYSTEM
        self.format_table.insert(0x57, RiscVFormat::V); // OP-V
        self.format_table.insert(0x07, RiscVFormat::I); // MISC-MEM
    }

    /// 快速检查是否为压缩指令
    #[inline]
    pub fn is_compressed(insn: u32) -> bool {
        // Compressed instructions have bits [1:0] != 11
        (insn & 0x3) != 0x3
    }

    /// 识别压缩指令类型
    #[inline]
    pub fn identify_compressed(&self, insn: u16) -> Option<CompressedInsnType> {
        // 使用 opcode (bits [15:13]) 进行查找
        let key = insn & 0xE000;
        self.compressed_table.get(&key).copied()
    }

    /// 查找 CSR 寄存器
    #[inline]
    pub fn lookup_csr(&self, csr_addr: u16) -> Option<CsrRegister> {
        self.csr_table.get(&csr_addr).copied()
    }

    /// 识别向量指令
    #[inline]
    pub fn identify_vector(&self, insn: u32) -> Option<VectorInsnType> {
        // Vector instructions: opcode = 0x57 (1010111)
        if (insn & 0x7F) != 0x57 {
            return None;
        }

        // 使用 funct6 (bits [31:26]) 和 funct3 (bits [14:12]) 进行查找
        let key = insn & 0xFC00007F;
        self.vector_table.get(&key).copied()
    }

    /// 获取指令格式
    #[inline]
    pub fn get_insn_format(&self, insn: u32) -> Option<RiscVFormat> {
        if Self::is_compressed(insn) {
            // 压缩指令格式
            let opcode = (insn & 0xE000) >> 13;
            match opcode {
                0 => Some(RiscVFormat::CIW),
                1..=3 => Some(RiscVFormat::CI),
                4 | 5 => Some(RiscVFormat::CSS),
                6 | 7 => Some(RiscVFormat::CI),
                _ => None,
            }
        } else {
            // 标准指令格式
            let opcode = (insn & 0x7F) as u8;
            self.format_table.get(&opcode).copied()
        }
    }

    /// 批量检查压缩指令
    pub fn check_compressed_batch(&self, insns: &[u32]) -> Vec<bool> {
        insns
            .iter()
            .map(|&insn| Self::is_compressed(insn))
            .collect()
    }

    /// 获取压缩指令统计
    pub fn get_compression_stats(&self, insns: &[u32]) -> CompressionStats {
        let mut stats = CompressionStats::default();

        for &insn in insns {
            stats.total_insn_count += 1;

            if Self::is_compressed(insn) {
                stats.compressed_count += 1;
                stats.compressed_size += 2;

                if let Some(c_type) = self.identify_compressed(insn as u16) {
                    match c_type {
                        CompressedInsnType::Arithmetic => stats.arithmetic_count += 1,
                        CompressedInsnType::LoadStore => stats.memory_count += 1,
                        CompressedInsnType::Branch => stats.branch_count += 1,
                        CompressedInsnType::Jump => stats.jump_count += 1,
                        CompressedInsnType::Immediate => stats.immediate_count += 1,
                        CompressedInsnType::Shift => stats.shift_count += 1,
                    }
                }
            } else {
                stats.standard_size += 4;
            }
        }

        stats
    }

    /// 获取向量指令统计
    pub fn get_vector_stats(&self, insns: &[u32]) -> VectorStats {
        let mut stats = VectorStats::default();

        for &insn in insns {
            if let Some(v_type) = self.identify_vector(insn) {
                stats.total_vector_insn += 1;
                match v_type {
                    VectorInsnType::Vadd
                    | VectorInsnType::Vsub
                    | VectorInsnType::Vmul
                    | VectorInsnType::Vdiv
                    | VectorInsnType::Vrem => {
                        stats.arithmetic_count += 1;
                    }
                    VectorInsnType::Vand | VectorInsnType::Vor | VectorInsnType::Vxor => {
                        stats.logical_count += 1;
                    }
                    VectorInsnType::Vsll | VectorInsnType::Vsrl | VectorInsnType::Vsra => {
                        stats.shift_count += 1;
                    }
                    VectorInsnType::Vmseq
                    | VectorInsnType::Vmsne
                    | VectorInsnType::Vmsltu
                    | VectorInsnType::Vmslt
                    | VectorInsnType::Vmsleu
                    | VectorInsnType::Vmsle => {
                        stats.compare_count += 1;
                    }
                    VectorInsnType::Vle
                    | VectorInsnType::Vse
                    | VectorInsnType::Vlm
                    | VectorInsnType::Vsm => {
                        stats.memory_count += 1;
                    }
                    VectorInsnType::Vfcvt => {
                        stats.convert_count += 1;
                    }
                    VectorInsnType::Vsetvli | VectorInsnType::Vsetvl => {
                        stats.config_count += 1;
                    }
                }
            }
        }

        stats
    }
}

impl Default for RiscvOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// 压缩指令统计信息
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    pub total_insn_count: u64,
    pub compressed_count: u64,
    pub arithmetic_count: u64,
    pub memory_count: u64,
    pub branch_count: u64,
    pub jump_count: u64,
    pub immediate_count: u64,
    pub shift_count: u64,
    pub compressed_size: u64,
    pub standard_size: u64,
}

impl CompressionStats {
    /// 计算压缩率
    pub fn compression_ratio(&self) -> f64 {
        if self.compressed_size + self.standard_size == 0 {
            return 0.0;
        }
        let original_size = self.total_insn_count * 4;
        let compressed_size = self.compressed_size + self.standard_size;
        1.0 - (compressed_size as f64 / original_size as f64)
    }

    /// 计算压缩指令占比
    pub fn compressed_ratio(&self) -> f64 {
        if self.total_insn_count == 0 {
            return 0.0;
        }
        self.compressed_count as f64 / self.total_insn_count as f64
    }
}

/// 向量指令统计信息
#[derive(Debug, Clone, Default)]
pub struct VectorStats {
    pub total_vector_insn: u64,
    pub arithmetic_count: u64,
    pub logical_count: u64,
    pub shift_count: u64,
    pub compare_count: u64,
    pub memory_count: u64,
    pub convert_count: u64,
    pub config_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimizer_creation() {
        let optimizer = RiscvOptimizer::new();
        assert!(!optimizer.compressed_table.is_empty());
        assert!(!optimizer.csr_table.is_empty());
        assert!(!optimizer.vector_table.is_empty());
    }

    #[test]
    fn test_is_compressed() {
        // C.ADDI4SPN (0x0000): bits [1:0] = 00
        assert!(RiscvOptimizer::is_compressed(0x0000));

        // Standard ADD (0x00000013): bits [1:0] = 11
        assert!(!RiscvOptimizer::is_compressed(0x00000013));
    }

    #[test]
    fn test_identify_compressed() {
        let optimizer = RiscvOptimizer::new();

        // C.LWSP (0x4002)
        let c_type = optimizer.identify_compressed(0x4002);
        assert_eq!(c_type, Some(CompressedInsnType::LoadStore));
    }

    #[test]
    fn test_lookup_csr() {
        let optimizer = RiscvOptimizer::new();

        // mstatus (0x300)
        assert_eq!(optimizer.lookup_csr(0x300), Some(CsrRegister::Mstatus));

        // sstatus (0x100)
        assert_eq!(optimizer.lookup_csr(0x100), Some(CsrRegister::Sstatus));

        // Unknown CSR
        assert_eq!(optimizer.lookup_csr(0x999), None);
    }

    #[test]
    fn test_identify_vector() {
        let optimizer = RiscvOptimizer::new();

        // VADD.VV (simplified example)
        let v_type = optimizer.identify_vector(0x00000057);
        assert_eq!(v_type, Some(VectorInsnType::Vsetvli));
    }

    #[test]
    fn test_get_insn_format() {
        let optimizer = RiscvOptimizer::new();

        // ADDI (I-format)
        assert_eq!(optimizer.get_insn_format(0x00000013), Some(RiscVFormat::I));

        // ADD (R-format)
        assert_eq!(optimizer.get_insn_format(0x00000033), Some(RiscVFormat::R));

        // SW (S-format)
        assert_eq!(optimizer.get_insn_format(0x00000023), Some(RiscVFormat::S));
    }

    #[test]
    fn test_compression_stats() {
        let optimizer = RiscvOptimizer::new();

        let insns = vec![0x0000, 0x4002, 0x00000013, 0x00000033];
        let stats = optimizer.get_compression_stats(&insns);

        assert_eq!(stats.total_insn_count, 4);
        assert_eq!(stats.compressed_count, 2);
        assert_eq!(stats.compressed_size, 4); // 2 * 2 bytes
        assert_eq!(stats.standard_size, 8); // 2 * 4 bytes
        assert!(stats.compression_ratio() > 0.0);
        assert_eq!(stats.compressed_ratio(), 0.5);
    }

    #[test]
    fn test_batch_compressed_check() {
        let optimizer = RiscvOptimizer::new();

        let insns = vec![0x0000, 0x00000013, 0x4002, 0x00000033];
        let is_compressed = optimizer.check_compressed_batch(&insns);

        assert_eq!(is_compressed, vec![true, false, true, false]);
    }

    #[test]
    fn test_vector_stats() {
        let optimizer = RiscvOptimizer::new();

        let insns = vec![0x00000007, 0x00000057, 0xA0000057];
        let stats = optimizer.get_vector_stats(&insns);

        assert_eq!(stats.total_vector_insn, 3);
        assert!(stats.config_count > 0 || stats.arithmetic_count > 0);
    }
}
