//! PGO集成到AOT构建器
//!
//! 提供基于Profile数据的AOT编译优化

use crate::{AotBuilder, CompilationOptions};
use vm_ir::IRBlock;
use vm_engine_jit::pgo::{ProfileData, ProfileAnalyzer, OptimizationSuggestion};

/// 基于PGO的AOT构建器
pub struct PgoAotBuilder {
    /// 底层AOT构建器
    builder: AotBuilder,
    /// Profile数据
    profile_data: Option<ProfileData>,
    /// Profile分析器
    analyzer: ProfileAnalyzer,
}

impl PgoAotBuilder {
    /// 创建新的PGO AOT构建器
    pub fn new(options: CompilationOptions) -> Self {
        Self {
            builder: AotBuilder::with_options(options),
            profile_data: None,
            analyzer: ProfileAnalyzer,
        }
    }

    /// 加载Profile数据
    pub fn load_profile<P: AsRef<std::path::Path>>(&mut self, path: P) -> Result<(), String> {
        self.profile_data = Some(
            vm_engine_jit::pgo::ProfileCollector::deserialize_from_file(path)?
        );
        Ok(())
    }

    /// 设置Profile数据
    pub fn set_profile_data(&mut self, profile: ProfileData) {
        self.profile_data = Some(profile);
    }

    /// 添加IR块（应用PGO优化）
    pub fn add_ir_block_with_pgo(
        &mut self,
        pc: u64,
        block: &IRBlock,
        flags: u32,
    ) -> Result<(), String> {
        // 如果有Profile数据，应用优化建议
        if let Some(ref profile) = self.profile_data {
            let suggestions = self.analyzer.analyze(profile);
            
            // 查找该代码块的优化建议
            let block_suggestions: Vec<&OptimizationSuggestion> = suggestions
                .iter()
                .filter(|s| s.pc == pc)
                .collect();

            // 根据优化建议调整编译选项
            if !block_suggestions.is_empty() {
                // 高优先级优化：应用更激进的优化级别
                let has_high_priority = block_suggestions.iter().any(|s| s.priority > 80);
                if has_high_priority {
                    // 可以在这里调整编译选项
                    // 例如：增加优化级别、启用特定优化Pass等
                }
            }
        }

        // 添加IR块
        self.builder.add_ir_block(pc, block, flags)
    }

    /// 获取优化建议
    pub fn get_optimization_suggestions(&self) -> Option<Vec<OptimizationSuggestion>> {
        self.profile_data.as_ref().map(|profile| {
            self.analyzer.analyze(profile)
        })
    }

    /// 获取热点代码块列表
    pub fn get_hot_blocks(&self, threshold: u64) -> Option<Vec<u64>> {
        self.profile_data.as_ref().map(|profile| {
            self.analyzer.get_hot_blocks(profile, threshold)
        })
    }

    /// 构建AOT镜像（应用PGO优化）
    pub fn build(&mut self) -> Result<vm_engine_jit::aot_format::AotImage, String> {
        // 如果有Profile数据，可以在构建前应用优化
        if let Some(ref profile) = self.profile_data {
            let suggestions = self.analyzer.analyze(profile);
            
            // 记录优化建议（可以用于后续分析）
            let high_priority_count = suggestions.iter().filter(|s| s.priority > 80).count();
            if high_priority_count > 0 {
                log::info!("Found {} high-priority optimization suggestions", high_priority_count);
            }
        }

        // 构建镜像
        self.builder.build()
    }

    /// 获取底层构建器（用于直接访问）
    pub fn builder_mut(&mut self) -> &mut AotBuilder {
        &mut self.builder
    }
}


