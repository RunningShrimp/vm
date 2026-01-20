//! # VGA 虚拟化模块
//!
//! 提供VGA显示设备的虚拟化实现

pub mod sdl_display;

pub use sdl_display::{SdlDisplayFrontend, VgaSnapshot};
