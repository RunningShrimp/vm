//! AOT格式占位实现

#[derive(Debug, Clone)]
pub struct AotHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub timestamp: u64,
    pub entry_point: u64,
    pub code_size: u64,
    pub data_size: u64,
}

#[derive(Debug, Clone)]
pub struct AotImage {
    pub header: AotHeader,
    pub code: Vec<u8>,
    pub data: Vec<u8>,
}

impl AotImage {
    pub fn new(header: AotHeader, code: Vec<u8>, data: Vec<u8>) -> Self {
        Self { header, code, data }
    }
}

#[derive(Debug, Clone)]
pub struct CodeBlockEntry;

#[derive(Debug, Clone)]
pub enum RelationType {
    Data,
    Code,
}

#[derive(Debug, Clone)]
pub struct RelocationEntry;

#[derive(Debug, Clone)]
pub struct SymbolEntry;

#[derive(Debug, Clone)]
pub enum SymbolType {
    Function,
    Data,
}

pub struct AotFormat;

/// AOT错误类型
#[derive(Debug, thiserror::Error)]
pub enum AotError {
    #[error("Invalid AOT configuration: {0}")]
    InvalidConfig(String),

    #[error("AOT image creation failed: {0}")]
    CreationFailed(String),

    #[error("AOT loader initialization failed: {0}")]
    LoaderInitFailed(String),

    #[error("Hybrid executor creation failed: {0}")]
    ExecutorCreationFailed(String),
}
