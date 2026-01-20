//! JIT辅助函数实现
//!
//! 提供JIT编译过程中的寄存器和内存操作辅助工具

/// 浮点寄存器辅助工具
///
/// 支持x86/x86_64的SSE/AVX寄存器分配和使用
#[derive(Debug, Clone)]
pub struct FloatRegHelper {
    /// 可用的浮点寄存器列表 (x86_64: XMM0-XMM15)
    available_regs: Vec<String>,
    /// 当前分配索引
    current_index: usize,
}

impl FloatRegHelper {
    /// 创建新的浮点寄存器辅助器
    pub fn new() -> Self {
        Self {
            available_regs: (0..16).map(|i| format!("xmm{}", i)).collect(),
            current_index: 0,
        }
    }

    /// 分配一个浮点寄存器
    pub fn allocate(&mut self) -> Option<String> {
        if self.current_index < self.available_regs.len() {
            let reg = self.available_regs[self.current_index].clone();
            self.current_index += 1;
            Some(reg)
        } else {
            None
        }
    }

    /// 释放寄存器（简单实现：重置分配器）
    pub fn release_all(&mut self) {
        self.current_index = 0;
    }

    /// 获取剩余可用寄存器数量
    pub fn available_count(&self) -> usize {
        self.available_regs.len() - self.current_index
    }
}

impl Default for FloatRegHelper {
    fn default() -> Self {
        Self::new()
    }
}

/// 内存操作辅助工具
///
/// 帮助处理load/store操作数和内存寻址模式
#[derive(Debug, Clone)]
pub struct MemoryHelper {
    /// 支持的寻址模式
    addressing_modes: Vec<&'static str>,
}

impl MemoryHelper {
    /// 创建新的内存辅助器
    pub fn new() -> Self {
        Self {
            addressing_modes: vec![
                "direct",    // 直接寻址 [addr]
                "base_disp", // 基址+偏移 [base + disp]
                "index",     // 索引寻址 [base + index*scale]
                "full",      // 完整寻址 [base + index*scale + disp]
            ],
        }
    }

    /// 分析操作数是否为内存操作数
    pub fn is_memory_operand(&self, _op: &str) -> bool {
        // 简化实现：检查是否包含内存操作特征
        // 完整实现需要IR操作数类型
        false
    }

    /// 生成内存操作数的汇编表示
    pub fn format_memory_operand(&self, addr: u64) -> String {
        format!("[{}]", addr)
    }

    /// 计算内存对齐的字节数
    pub fn calculate_alignment(size: usize) -> usize {
        match size {
            1 => 1,
            2 => 2,
            4 | 8 => 4,
            _ => 16,
        }
    }

    /// 获取支持的寻址模式列表
    pub fn supported_modes(&self) -> &[&'static str] {
        &self.addressing_modes
    }
}

impl Default for MemoryHelper {
    fn default() -> Self {
        Self::new()
    }
}

/// 通用寄存器辅助工具
///
/// 管理通用寄存器的分配和释放
#[derive(Debug, Clone)]
pub struct RegisterHelper {
    /// 通用寄存器池 (x86_64: RAX, RBX, RCX, RDX, RSI, RDI, R8-R15)
    register_pool: Vec<String>,
    /// 已分配的寄存器
    allocated: Vec<String>,
    /// 保留寄存器（不分配）
    reserved: Vec<String>,
}

impl RegisterHelper {
    /// 创建新的寄存器辅助器
    pub fn new() -> Self {
        Self {
            register_pool: vec![
                "rax".to_string(),
                "rbx".to_string(),
                "rcx".to_string(),
                "rdx".to_string(),
                "rsi".to_string(),
                "rdi".to_string(),
                "r8".to_string(),
                "r9".to_string(),
                "r10".to_string(),
                "r11".to_string(),
                "r12".to_string(),
                "r13".to_string(),
                "r14".to_string(),
                "r15".to_string(),
            ],
            allocated: Vec::new(),
            reserved: vec!["rsp".to_string(), "rbp".to_string()],
        }
    }

    /// 分配一个寄存器
    pub fn allocate(&mut self) -> Option<String> {
        for reg in &self.register_pool {
            if !self.allocated.contains(reg) && !self.reserved.contains(reg) {
                self.allocated.push(reg.clone());
                return Some(reg.clone());
            }
        }
        None
    }

    /// 释放指定的寄存器
    pub fn release(&mut self, reg: &str) {
        self.allocated.retain(|r| r != reg);
    }

    /// 释放所有已分配的寄存器
    pub fn release_all(&mut self) {
        self.allocated.clear();
    }

    /// 检查寄存器是否已分配
    pub fn is_allocated(&self, reg: &str) -> bool {
        self.allocated.contains(&reg.to_string())
    }

    /// 获取可用寄存器数量
    pub fn available_count(&self) -> usize {
        self.register_pool
            .iter()
            .filter(|r| !self.allocated.contains(r) && !self.reserved.contains(r))
            .count()
    }

    /// 获取所有已分配的寄存器
    pub fn allocated_regs(&self) -> &[String] {
        &self.allocated
    }
}

impl Default for RegisterHelper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_float_reg_allocation() {
        let mut helper = FloatRegHelper::new();
        assert_eq!(helper.allocate(), Some("xmm0".to_string()));
        assert_eq!(helper.allocate(), Some("xmm1".to_string()));
        assert_eq!(helper.available_count(), 14);
    }

    #[test]
    fn test_float_reg_release() {
        let mut helper = FloatRegHelper::new();
        helper.allocate();
        helper.allocate();
        helper.release_all();
        assert_eq!(helper.available_count(), 16);
    }

    #[test]
    fn test_register_allocation() {
        let mut helper = RegisterHelper::new();
        let reg1 = helper.allocate();
        let reg2 = helper.allocate();
        assert!(reg1.is_some());
        assert!(reg2.is_some());
        assert_ne!(reg1, reg2);
        assert_eq!(helper.allocated_regs().len(), 2);
    }

    #[test]
    fn test_register_release() {
        let mut helper = RegisterHelper::new();
        let reg = helper.allocate().unwrap();
        assert!(helper.is_allocated(&reg));
        helper.release(&reg);
        assert!(!helper.is_allocated(&reg));
    }

    #[test]
    fn test_reserved_registers() {
        let helper = RegisterHelper::new();
        assert!(helper.reserved.contains(&"rsp".to_string()));
        assert!(helper.reserved.contains(&"rbp".to_string()));
    }

    #[test]
    fn test_memory_alignment() {
        assert_eq!(MemoryHelper::calculate_alignment(1), 1);
        assert_eq!(MemoryHelper::calculate_alignment(2), 2);
        assert_eq!(MemoryHelper::calculate_alignment(4), 4);
        assert_eq!(MemoryHelper::calculate_alignment(8), 4);
        assert_eq!(MemoryHelper::calculate_alignment(16), 16);
    }
}
