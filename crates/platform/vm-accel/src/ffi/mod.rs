//! FFI bindings consolidation module
//!
//! This module consolidates all Foreign Function Interface (FFI) declarations
//! for different virtualization platforms into a single location, making it
//! easier to maintain and update platform-specific bindings.
//!
//! # Structure
//!
//! - `kvm.rs` - Linux KVM FFI bindings (re-exports from kvm_ioctls)
//! - `hvf.rs` - macOS Hypervisor.framework FFI bindings
//! - `whpx.rs` - Windows Hypervisor Platform FFI bindings (re-exports from windows-rs)
//! - `vz.rs` - iOS/tvOS Virtualization.framework FFI bindings
//!
//! # Example
//!
//! ```rust,ignore
//! use vm_accel::ffi::hvf::*;
//!
//! unsafe {
//!     let result = hv_vm_create(std::ptr::null_mut());
//!     if result == HV_SUCCESS {
//!         println!("VM created successfully");
//!     }
//! }
//! ```

// KVM FFI bindings
#[cfg(target_os = "linux")]
pub mod kvm;

// HVF FFI bindings
#[cfg(target_os = "macos")]
pub mod hvf;

// WHPX FFI bindings
#[cfg(target_os = "windows")]
pub mod whpx;

// VZ FFI bindings
#[cfg(any(target_os = "ios", target_os = "tvos"))]
pub mod vz;
