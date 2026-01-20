//! Linux KVM FFI bindings
//!
//! This module re-exports KVM FFI bindings from the kvm_ioctls and kvm_bindings crates,
//! providing a centralized location for all KVM-related FFI declarations.
//!
//! # Note
//!
//! Most KVM FFI is handled by the kvm-bindings and kvm-ioctls crates.
//! This module primarily re-exports their types for convenience.

#[cfg(feature = "kvm")]
pub use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};

#[cfg(feature = "kvm")]
pub use kvm_bindings;

/// KVM API version information
#[cfg(feature = "kvm")]
#[derive(Debug, Clone, Copy)]
pub struct KvmVersion {
    pub major: u32,
    pub minor: u32,
}

#[cfg(feature = "kvm")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kvm_types_available() {
        // Test that KVM types are properly re-exported
        // This is a compile-time test
        let _ = std::marker::PhantomData::<VcpuFd>;
    }
}
