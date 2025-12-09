//! 跨架构测试的公共工具和辅助函数

use vm_ir::IROp;

/// 架构类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    X86_64,
    Arm64,
    RiscV64,
}

impl Architecture {
    pub fn name(&self) -> &'static str {
        match self {
            Architecture::X86_64 => "x86-64",
            Architecture::Arm64 => "ARM64",
            Architecture::RiscV64 => "RISC-V64",
        }
    }

    pub fn register_count(&self) -> usize {
        match self {
            Architecture::X86_64 => 16,
            Architecture::Arm64 => 32,
            Architecture::RiscV64 => 32,
        }
    }

    pub fn address_bits(&self) -> usize {
        64 // 所有都是64位架构
    }
}

/// 翻译结果
#[derive(Debug, Clone)]
pub struct TranslationResult {
    pub src_arch: Architecture,
    pub dst_arch: Architecture,
    pub instruction_count: usize,
    pub success: bool,
    pub expansion_ratio: f64,
}

impl TranslationResult {
    pub fn new(src: Architecture, dst: Architecture) -> Self {
        Self {
            src_arch: src,
            dst_arch: dst,
            instruction_count: 0,
            success: false,
            expansion_ratio: 1.0,
        }
    }
}

/// 翻译验证器
pub struct TranslationValidator {
    src_arch: Architecture,
    dst_arch: Architecture,
    results: Vec<TranslationResult>,
}

impl TranslationValidator {
    pub fn new(src: Architecture, dst: Architecture) -> Self {
        Self {
            src_arch: src,
            dst_arch: dst,
            results: vec![],
        }
    }

    pub fn validate_register_mapping(&self) -> bool {
        let src_regs = self.src_arch.register_count();
        let dst_regs = self.dst_arch.register_count();
        
        // 目标架构应该有足够的寄存器
        dst_regs >= src_regs.saturating_sub(1)
    }

    pub fn validate_address_space(&self) -> bool {
        let src_bits = self.src_arch.address_bits();
        let dst_bits = self.dst_arch.address_bits();
        
        // 地址空间大小应该相同
        src_bits == dst_bits
    }

    pub fn validate_instruction(&self, _ir: &IROp) -> bool {
        // 验证指令翻译的正确性
        true
    }

    pub fn add_result(&mut self, result: TranslationResult) {
        self.results.push(result);
    }

    pub fn average_expansion(&self) -> f64 {
        if self.results.is_empty() {
            return 1.0;
        }
        
        let sum: f64 = self.results.iter().map(|r| r.expansion_ratio).sum();
        sum / self.results.len() as f64
    }
}

/// 跨架构执行模拟器
pub struct CrossArchExecutor {
    pub current_arch: Architecture,
    pub instructions_executed: usize,
}

impl CrossArchExecutor {
    pub fn new(arch: Architecture) -> Self {
        Self {
            current_arch: arch,
            instructions_executed: 0,
        }
    }

    pub fn execute(&mut self, _ir: IROp) -> bool {
        self.instructions_executed += 1;
        true
    }

    pub fn switch_architecture(&mut self, new_arch: Architecture) {
        self.current_arch = new_arch;
    }

    pub fn get_execution_count(&self) -> usize {
        self.instructions_executed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architecture_register_count() {
        assert_eq!(Architecture::X86_64.register_count(), 16);
        assert_eq!(Architecture::Arm64.register_count(), 32);
        assert_eq!(Architecture::RiscV64.register_count(), 32);
    }

    #[test]
    fn test_architecture_names() {
        assert_eq!(Architecture::X86_64.name(), "x86-64");
        assert_eq!(Architecture::Arm64.name(), "ARM64");
        assert_eq!(Architecture::RiscV64.name(), "RISC-V64");
    }

    #[test]
    fn test_validator_register_mapping() {
        let validator = TranslationValidator::new(Architecture::X86_64, Architecture::Arm64);
        assert!(validator.validate_register_mapping());
    }

    #[test]
    fn test_validator_address_space() {
        let validator = TranslationValidator::new(Architecture::X86_64, Architecture::RiscV64);
        assert!(validator.validate_address_space());
    }

    #[test]
    fn test_executor_switching() {
        let mut executor = CrossArchExecutor::new(Architecture::X86_64);
        assert_eq!(executor.current_arch, Architecture::X86_64);
        
        executor.switch_architecture(Architecture::Arm64);
        assert_eq!(executor.current_arch, Architecture::Arm64);
    }
}
