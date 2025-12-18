//! IPC Handlers - Inter-Process Communication between Frontend and Backend
//!
//! Defines all available commands that the frontend can invoke via Tauri's IPC.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmInstance {
    pub id: String,
    pub name: String,
    pub state: VmState,
    pub cpu_count: u32,
    pub memory_mb: u32,
    pub display_mode: DisplayMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VmState {
    Stopped,
    Running,
    Paused,
    Suspended,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DisplayMode {
    GUI,
    Terminal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VmConfig {
    pub id: String,
    pub name: String,
    pub cpu_count: u32,
    pub memory_mb: u32,
    pub disk_gb: u32,
    pub display_mode: DisplayMode,
    pub os_type: OsType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OsType {
    Ubuntu,
    Debian,
    Windows,
    CentOS,
    Other,
}

#[derive(Debug, Clone, Serialize)]
pub struct VmMetrics {
    pub id: String,
    pub cpu_usage: f32,
    pub memory_usage_mb: u32,
    pub disk_io_read_mb_s: f32,
    pub disk_io_write_mb_s: f32,
    pub network_rx_mb_s: f32,
    pub network_tx_mb_s: f32,
    pub uptime_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct TerminalOutput {
    pub id: String,
    pub data: String,
    pub timestamp: u64,
}

// IPC command handlers are defined in separate modules
pub use super::config;
pub use super::monitoring;
pub use super::vm_controller;
