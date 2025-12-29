//! Support utilities for VM project
//!
//! This module provides helper utilities, macros, and testing tools.

// Re-export support modules from parent
pub use crate::support_utils::*;
pub use crate::support_macros::*;
pub use crate::support_test_helpers::*;

/// VM support utilities version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// VM support utilities description
pub const DESCRIPTION: &str = "VM Support Utilities - Helper Functions, Macros, and Testing Tools";
