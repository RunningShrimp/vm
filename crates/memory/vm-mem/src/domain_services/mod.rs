//! 领域服务模块
//!
//! 包含内存管理领域的各种服务，如地址转换服务。

pub mod address_translation;

pub use address_translation::AddressTranslationDomainService;
