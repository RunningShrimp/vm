/// 内联缓存统计
#[derive(Debug, Clone, Default)]
pub struct InlineCacheStats {
    /// 总命中次数
    pub total_hits: u64,
    /// 总未命中次数
    pub total_misses: u64,
    /// 单态命中次数
    pub monomorphic_hits: u64,
    /// 单态未命中次数
    pub monomorphic_misses: u64,
    /// 多态命中次数
    pub polymorphic_hits: u64,
    /// 多态未命中次数
    pub polymorphic_misses: u64,
    /// 超多态转换次数（缓存条目过多）
    pub megamorphic_transitions: u64,
    /// 单态到多态转换次数
    pub monomorphic_to_polymorphic: u64,
    /// 失效次数
    pub invalidations: u64,
    /// 清空次数
    pub clears: u64,
}

impl InlineCacheStats {
    /// 计算命中率
    pub fn hit_rate(&self) -> f64 {
        if self.total_hits + self.total_misses == 0 {
            0.0
        } else {
            self.total_hits as f64 / (self.total_hits + self.total_misses) as f64
        }
    }

    /// 计算单态命中率
    pub fn monomorphic_hit_rate(&self) -> f64 {
        if self.monomorphic_hits + self.monomorphic_misses == 0 {
            0.0
        } else {
            self.monomorphic_hits as f64 / (self.monomorphic_hits + self.monomorphic_misses) as f64
        }
    }

    /// 计算多态命中率
    pub fn polymorphic_hit_rate(&self) -> f64 {
        if self.polymorphic_hits + self.polymorphic_misses == 0 {
            0.0
        } else {
            self.polymorphic_hits as f64 / (self.polymorphic_hits + self.polymorphic_misses) as f64
        }
    }

    /// 计算多态转换率
    pub fn polymorphic_transition_rate(&self) -> f64 {
        if self.monomorphic_hits + self.monomorphic_misses == 0 {
            0.0
        } else {
            self.monomorphic_to_polymorphic as f64
                / (self.monomorphic_hits + self.monomorphic_misses) as f64
        }
    }

    /// 获取总查找次数
    pub fn total_lookups(&self) -> u64 {
        self.total_hits + self.total_misses
    }

    /// 重置统计
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
