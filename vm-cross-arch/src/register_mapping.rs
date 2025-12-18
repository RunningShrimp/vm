//! 寄存器映射模块
//!
//! 处理不同架构之间的寄存器映射关系

use super::Architecture;
use vm_ir::RegId;

/// 寄存器映射器
///
/// 负责在不同架构之间映射寄存器编号
pub struct RegisterMapper {
    /// 寄存器映射表：源寄存器ID -> 目标寄存器ID
    mapping: Vec<Option<RegId>>,
    /// 临时寄存器池（用于复杂操作）
    temp_regs: Vec<RegId>,
    next_temp: usize,
}

impl RegisterMapper {
    /// 创建新的寄存器映射器
    pub fn new(source_arch: Architecture, target_arch: Architecture) -> Self {
        let source_regs = source_arch.register_count();
        let target_regs = target_arch.register_count();

        // 初始化映射表
        let mut mapping = vec![None; source_regs];

        // 建立基本映射关系
        for i in 0..source_regs.min(target_regs) {
            mapping[i] = Some(i as RegId);
        }

        // 初始化临时寄存器池（使用高编号寄存器）
        let temp_start = target_regs.min(16);
        let temp_count = target_regs.saturating_sub(temp_start);
        let temp_regs: Vec<RegId> = (temp_start..temp_start + temp_count)
            .map(|i| i as RegId)
            .collect();

        Self {
            mapping,
            temp_regs,
            next_temp: 0,
        }
    }

    /// 映射源寄存器到目标寄存器
    pub fn map_register(&self, source_reg: RegId) -> RegId {
        let source_idx = source_reg as usize;
        if source_idx < self.mapping.len() {
            self.mapping[source_idx].unwrap_or(0)
        } else {
            // 超出范围的寄存器映射到0
            0
        }
    }

    /// 分配临时寄存器
    pub fn allocate_temp(&mut self) -> Option<RegId> {
        if self.next_temp < self.temp_regs.len() {
            let reg = self.temp_regs[self.next_temp];
            self.next_temp += 1;
            Some(reg)
        } else {
            None
        }
    }

    /// 释放临时寄存器
    pub fn release_temp(&mut self, _reg: RegId) {
        // 简化实现：不跟踪释放，在转换结束时重置
    }

    /// 重置临时寄存器分配器
    pub fn reset_temps(&mut self) {
        self.next_temp = 0;
    }

    /// 设置自定义寄存器映射
    pub fn set_mapping(&mut self, source_reg: RegId, target_reg: RegId) {
        let source_idx = source_reg as usize;
        if source_idx < self.mapping.len() {
            self.mapping[source_idx] = Some(target_reg);
        }
    }
}

/// 寄存器映射配置
#[derive(Debug, Clone)]
pub struct RegisterMapping {
    /// 源架构
    pub source_arch: Architecture,
    /// 目标架构
    pub target_arch: Architecture,
    /// 映射规则：源寄存器 -> 目标寄存器
    pub mappings: Vec<(RegId, RegId)>,
}

impl RegisterMapping {
    /// 创建默认映射
    pub fn default(source_arch: Architecture, target_arch: Architecture) -> Self {
        let count = source_arch
            .register_count()
            .min(target_arch.register_count());
        let mappings: Vec<(RegId, RegId)> = (0..count).map(|i| (i as RegId, i as RegId)).collect();

        Self {
            source_arch,
            target_arch,
            mappings,
        }
    }

    /// 应用映射到寄存器ID
    pub fn apply(&self, source_reg: RegId) -> RegId {
        self.mappings
            .iter()
            .find(|(src, _)| *src == source_reg)
            .map(|(_, dst)| *dst)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_mapper() {
        let mapper = RegisterMapper::new(Architecture::X86_64, Architecture::ARM64);
        assert_eq!(mapper.map_register(0), 0);
        assert_eq!(mapper.map_register(1), 1);
    }

    #[test]
    fn test_temp_allocation() {
        let mut mapper = RegisterMapper::new(Architecture::X86_64, Architecture::ARM64);
        let temp1 = mapper.allocate_temp();
        assert!(temp1.is_some());
    }
}
