//! EWMA热点检测占位实现

use vm_core::GuestAddr;

#[derive(Debug, Clone)]
pub struct EwmaHotspotConfig;

impl Default for EwmaHotspotConfig {
    fn default() -> Self {
        Self
    }
}

#[derive(Debug, Clone)]
pub struct EwmaHotspotStats;

#[derive(Debug, Clone)]
pub struct HotspotStats;

#[derive(Debug)]
pub struct EwmaHotspotDetector;

impl EwmaHotspotDetector {
    pub fn new(_config: EwmaHotspotConfig) -> Self {
        Self
    }

    pub fn is_hotspot(&self, _addr: GuestAddr) -> bool {
        false
    }

    pub fn record_execution(&self, _addr: GuestAddr, _duration_us: u64) {
        // 占位实现
    }

    pub fn record_execution_with_complexity(&self, _addr: GuestAddr, _duration_us: u64, _complexity_score: f64) {
        // 占位实现
    }
}
