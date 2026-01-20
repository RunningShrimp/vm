// Re-export decoder_factory module contents for external use
pub use decoder_factory::*;
pub use service::*;

pub mod acpi;
pub mod apic;
pub mod bios;
pub mod decoder_factory;
pub mod efi;
pub mod execution;
pub mod kernel_loader;
pub mod lifecycle;
pub mod mode_trans;
pub mod pci;
pub mod performance;
pub mod pic;
pub mod pit;
pub mod realmode;
pub mod service;
pub mod snapshot_manager;
pub mod vesa;
pub mod vga;
pub mod x86_boot;
pub mod x86_boot_exec;
pub mod x86_boot_setup;

#[cfg(feature = "smmu")]
pub mod smmu;
