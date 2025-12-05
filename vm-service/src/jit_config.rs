//! JIT配置管理模块

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::GuestAddr;
use vm_engine_jit::{AdaptiveThresholdConfig, AdaptiveThresholdStats, CodePtr};

/// JIT配置管理功能
pub struct JitConfigManager {
    /// 自适应快照
    adaptive_snapshot: Arc<Mutex<Option<AdaptiveThresholdStats>>>,
    /// 自适应配置
    adaptive_config: Option<AdaptiveThresholdConfig>,
    /// JIT代码池
    code_pool: Option<Arc<Mutex<HashMap<GuestAddr, CodePtr>>>>,
}

impl JitConfigManager {
    pub fn new() -> Self {
        Self {
            adaptive_snapshot: Arc::new(Mutex::new(None)),
            adaptive_config: None,
            code_pool: None,
        }
    }

    /// 获取JIT热点统计
    pub fn hot_stats(&self) -> Option<AdaptiveThresholdStats> {
        self.adaptive_snapshot
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// 设置JIT热点配置
    pub fn set_hot_config(&mut self, cfg: AdaptiveThresholdConfig) {
        self.adaptive_config = Some(cfg);
    }

    /// 设置JIT热点配置值
    pub fn set_hot_config_vals(
        &mut self,
        min: u64,
        max: u64,
        window: Option<usize>,
        compile_w: Option<f64>,
        benefit_w: Option<f64>,
    ) {
        let mut cfg = AdaptiveThresholdConfig::default();
        cfg.min_threshold = min;
        cfg.max_threshold = max;
        if let Some(w) = window {
            cfg.sample_window = w;
        }
        if let Some(w) = compile_w {
            cfg.compile_time_weight = w;
        }
        if let Some(w) = benefit_w {
            cfg.exec_benefit_weight = w;
        }
        self.adaptive_config = Some(cfg);
    }

    /// 设置共享代码池
    pub fn set_shared_pool(&mut self, enable: bool) {
        if enable {
            if self.code_pool.is_none() {
                self.code_pool = Some(Arc::new(Mutex::new(HashMap::new())));
            }
        } else {
            self.code_pool = None;
        }
    }

    /// 获取JIT热点快照
    pub fn hot_snapshot(
        &self,
    ) -> Option<(AdaptiveThresholdConfig, AdaptiveThresholdStats)> {
        let snapshot = self
            .adaptive_snapshot
            .lock()
            .ok()
            .and_then(|guard| guard.clone());
        match (self.adaptive_config.clone(), snapshot) {
            (Some(cfg), Some(stats)) => Some((cfg, stats)),
            _ => None,
        }
    }

    /// 导出JIT热点快照为JSON
    pub fn export_hot_snapshot_json(&self) -> Option<String> {
        self.hot_snapshot().map(|(cfg, stats)| {
            format!(
                "{{\"min_threshold\":{},\"max_threshold\":{},\"sample_window\":{},\"compile_time_weight\":{},\"exec_benefit_weight\":{},\"compiled_hits\":{},\"interpreted_runs\":{},\"total_compiles\":{} }}",
                cfg.min_threshold, cfg.max_threshold, cfg.sample_window,
                cfg.compile_time_weight, cfg.exec_benefit_weight,
                stats.compiled_hits, stats.interpreted_runs, stats.total_compiles,
            )
        })
    }

    /// 获取代码池引用
    pub fn code_pool(&self) -> Option<Arc<Mutex<HashMap<GuestAddr, CodePtr>>>> {
        self.code_pool.as_ref().cloned()
    }

    /// 获取自适应配置引用
    pub fn adaptive_config(&self) -> Option<AdaptiveThresholdConfig> {
        self.adaptive_config.clone()
    }

    /// 更新自适应快照
    pub fn update_snapshot(&self, stats: AdaptiveThresholdStats) {
        if let Ok(mut snapshot) = self.adaptive_snapshot.lock() {
            *snapshot = Some(stats);
        }
    }
}


