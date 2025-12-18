//! VM Desktop Application - Tauri Backend
//!
//! This module provides the Rust backend for the Tauri-based VM management desktop application.
//! It handles:
//! - VM lifecycle management (start, stop, pause, resume)
//! - Configuration file operations
//! - Real-time monitoring and data collection
//! - IPC communication with the frontend

pub mod config;
pub mod display;
pub mod ipc;
pub mod monitoring;
pub mod vm_controller;

pub use config::*;
pub use display::*;
pub use ipc::*;
pub use monitoring::*;
pub use vm_controller::*;

/// Application state shared across IPC handlers
pub struct AppState {
    pub vm_controller: vm_controller::VmController,
    pub monitoring: monitoring::MonitoringService,
}
