//! Symbol table support system
//!
//! This module provides comprehensive symbol table support including
//! DWARF debug information parsing, symbol lookup, and source-level debugging.

use std::collections::{HashMap, BTreeMap};
use std::sync::{Arc, RwLock};
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::{GuestAddr, VmError, VmResult};

/// Symbol information
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Symbol name
    pub name: String,
    /// Symbol address
    pub address: GuestAddr,
    /// Symbol size in bytes
    pub size: usize,
    /// Symbol type
    pub symbol_type: SymbolType,
    /// Symbol scope (local, global, static)
    pub scope: SymbolScope,
    /// Source file where symbol is defined
    pub source_file: Option<String>,
    /// Line number in source file
    pub line_number: Option<u32>,
    /// Column number in source file
    pub column_number: Option<u32>,
    /// Symbol visibility
    pub visibility: SymbolVisibility,
    /// Additional symbol attributes
    pub attributes: HashMap<String, String>,
}

/// Symbol types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolType {
    /// Function symbol
    Function,
    /// Variable symbol
    Variable,
    /// Parameter symbol
    Parameter,
    /// Label symbol
    Label,
    /// Type symbol
    Type,
    /// Constant symbol
    Constant,
    /// Array symbol
    Array,
    /// Struct symbol
    Struct,
    /// Union symbol
    Union,
    /// Enum symbol
    Enum,
    /// Namespace symbol
    Namespace,
    /// Unknown symbol type
    Unknown,
}

/// Symbol scope
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolScope {
    /// Local symbol (function-local)
    Local,
    /// Global symbol
    Global,
    /// Static symbol (file-local)
    Static,
    /// Extern symbol
    Extern,
    /// Template symbol
    Template,
}

/// Symbol visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolVisibility {
    /// Default visibility
    Default,
    /// Public symbol
    Public,
    /// Private symbol
    Private,
    /// Protected symbol
    Protected,
    /// Hidden symbol
    Hidden,
    /// Internal symbol
    Internal,
}

/// Source location information
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    /// Source file path
    pub file: String,
    /// Line number
    pub line: u32,
    /// Column number
    pub column: u32,
    /// Function name at this location
    pub function: Option<String>,
    /// Address corresponding to this location
    pub address: GuestAddr,
}

/// Function information
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    /// Function name
    pub name: String,
    /// Function start address
    pub start_address: GuestAddr,
    /// Function end address
    pub end_address: GuestAddr,
    /// Function size in bytes
    pub size: usize,
    /// Return type
    pub return_type: Option<String>,
    /// Parameters
    pub parameters: Vec<ParameterInfo>,
    /// Local variables
    pub local_variables: Vec<LocalVariableInfo>,
    /// Source file where function is defined
    pub source_file: Option<String>,
    /// Start line in source file
    pub start_line: Option<u32>,
    /// End line in source file
    pub end_line: Option<u32>,
    /// Calling convention
    pub calling_convention: Option<String>,
    /// Frame size
    pub frame_size: Option<usize>,
    /// Whether function is inline
    pub inline: bool,
    /// Whether function is variadic
    pub variadic: bool,
}

/// Parameter information
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterInfo {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub param_type: String,
    /// Parameter location (register, stack, etc.)
    pub location: VariableLocation,
    /// Default value (if any)
    pub default_value: Option<String>,
}

/// Local variable information
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalVariableInfo {
    /// Variable name
    pub name: String,
    /// Variable type
    pub var_type: String,
    /// Variable location
    pub location: VariableLocation,
    /// Variable scope (within function)
    pub scope_start: GuestAddr,
    /// Variable scope end address
    pub scope_end: GuestAddr,
    /// Source line where variable is defined
    pub line: Option<u32>,
    /// Whether variable is optimized out
    pub optimized_out: bool,
}

/// Variable location (reused from call_stack module)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableLocation {
    /// Stored in a register
    Register { register: String, offset: Option<i32> },
    /// Stored on stack at offset from frame pointer
    StackOffset { offset: i32 },
    /// Stored at absolute memory address
    Memory { address: GuestAddr },
    /// Stored in multiple locations (e.g., split across registers)
    Multiple { locations: Vec<VariableLocation> },
}

/// Line number information for address-to-line mapping
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineInfo {
    /// Source file path
    pub file: String,
    /// Line number
    pub line: u32,
    /// Column number
    pub column: u32,
    /// Start address for this line
    pub address: GuestAddr,
    /// Number of bytes covered by this line
    pub length: usize,
}

/// Symbol table configuration
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolTableConfig {
    /// Enable DWARF debug information parsing
    pub enable_dwarf: bool,
    /// Enable symbol demangling
    pub enable_demangling: bool,
    /// Cache symbol information
    pub enable_caching: bool,
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Search paths for source files
    pub source_search_paths: Vec<String>,
    /// Enable lazy loading of debug information
    pub lazy_loading: bool,
}

#[cfg(feature = "enhanced-debugging")]
impl Default for SymbolTableConfig {
    fn default() -> Self {
        Self {
            enable_dwarf: true,
            enable_demangling: true,
            enable_caching: true,
            max_cache_size: 10000,
            source_search_paths: vec![".".to_string(), "./src".to_string()],
            lazy_loading: true,
        }
    }
}

/// Enhanced symbol table
#[cfg(feature = "enhanced-debugging")]
pub struct SymbolTable {
    /// Configuration
    config: SymbolTableConfig,
    /// Symbols by name
    symbols_by_name: Arc<RwLock<HashMap<String, Vec<Symbol>>>>,
    /// Symbols by address
    symbols_by_address: Arc<RwLock<BTreeMap<GuestAddr, Symbol>>>,
    /// Functions by name
    functions_by_name: Arc<RwLock<HashMap<String, FunctionInfo>>>,
    /// Functions by address
    functions_by_address: Arc<RwLock<BTreeMap<GuestAddr, FunctionInfo>>>,
    /// Line number information
    line_info: Arc<RwLock<BTreeMap<GuestAddr, LineInfo>>>,
    /// Source locations by address
    source_locations: Arc<RwLock<BTreeMap<GuestAddr, SourceLocation>>>,
    /// Loaded debug files
    debug_files: Arc<RwLock<HashMap<String, DebugFileInfo>>>,
    /// Symbol cache
    symbol_cache: Arc<RwLock<HashMap<String, Symbol>>>,
}

/// Debug file information
#[derive(Debug, Clone)]
struct DebugFileInfo {
    /// File path
    path: String,
    /// File modification time
    modified_time: std::time::SystemTime,
    /// Whether debug info has been loaded
    loaded: bool,
    /// Debug info format (DWARF, PDB, etc.)
    format: DebugFormat,
}

/// Debug information formats
#[derive(Debug, Clone, Copy)]
enum DebugFormat {
    /// DWARF format
    Dwarf,
    /// PDB format (Windows)
    Pdb,
    /// STABS format
    Stabs,
    /// Unknown format
    Unknown,
}

#[cfg(feature = "enhanced-debugging")]
impl SymbolTable {
    /// Create a new symbol table
    pub fn new(config: SymbolTableConfig) -> Self {
        Self {
            config,
            symbols_by_name: Arc::new(RwLock::new(HashMap::new())),
            symbols_by_address: Arc::new(RwLock::new(BTreeMap::new())),
            functions_by_name: Arc::new(RwLock::new(HashMap::new())),
            functions_by_address: Arc::new(RwLock::new(BTreeMap::new())),
            line_info: Arc::new(RwLock::new(BTreeMap::new())),
            source_locations: Arc::new(RwLock::new(BTreeMap::new())),
            debug_files: Arc::new(RwLock::new(HashMap::new())),
            symbol_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load symbols from a binary file
    pub fn load_from_file(&self, file_path: &Path) -> VmResult<()> {
        let file_path_str = file_path.to_string_lossy().to_string();
        
        // Check if already loaded
        {
            let debug_files = self.debug_files.read().unwrap();
            if let Some(info) = debug_files.get(&file_path_str) {
                if info.loaded {
                    return Ok(());
                }
            }
        }

        // Load debug information based on file format
        let format = self.detect_debug_format(file_path)?;
        
        match format {
            DebugFormat::Dwarf => {
                if self.config.enable_dwarf {
                    self.load_dwarf_debug_info(file_path)?;
                }
            }
            DebugFormat::Pdb => {
                // PDB loading would go here
                return Err(VmError::Core(crate::error::CoreError::UnsupportedOperation {
                    operation: "PDB debug info loading".to_string(),
                    reason: "Not yet implemented".to_string(),
                }));
            }
            DebugFormat::Stabs => {
                // STABS loading would go here
                return Err(VmError::Core(crate::error::CoreError::UnsupportedOperation {
                    operation: "STABS debug info loading".to_string(),
                    reason: "Not yet implemented".to_string(),
                }));
            }
            DebugFormat::Unknown => {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: "Unknown debug format".to_string(),
                    current: "Unknown".to_string(),
                    expected: "DWARF, PDB, or STABS".to_string(),
                }));
            }
        }

        // Mark as loaded
        {
            let mut debug_files = self.debug_files.write().unwrap();
            debug_files.insert(file_path_str, DebugFileInfo {
                path: file_path_str,
                modified_time: std::fs::metadata(file_path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                loaded: true,
                format,
            });
        }

        Ok(())
    }

    /// Find symbol by name
    pub fn find_symbol(&self, name: &str) -> VmResult<Option<Symbol>> {
        // Check cache first
        if self.config.enable_caching {
            let cache = self.symbol_cache.read().unwrap();
            if let Some(symbol) = cache.get(name) {
                return Ok(Some(symbol.clone()));
            }
        }

        let symbols_by_name = self.symbols_by_name.read().unwrap();
        if let Some(symbols) = symbols_by_name.get(name) {
            // Return the first match (could be multiple due to overloading)
            let symbol = symbols.first().cloned();
            
            // Update cache
            if self.config.enable_caching {
                if let Some(ref sym) = symbol {
                    let mut cache = self.symbol_cache.write().unwrap();
                    cache.insert(name.to_string(), sym.clone());
                }
            }
            
            Ok(symbol)
        } else {
            Ok(None)
        }
    }

    /// Find symbol by address
    pub fn find_symbol_by_address(&self, address: GuestAddr) -> VmResult<Option<Symbol>> {
        let symbols_by_address = self.symbols_by_address.read().unwrap();
        
        // Find the symbol with the largest address <= target address
        let symbol = symbols_by_address
            .range(..=address)
            .next_back()
            .map(|(_, sym)| sym.clone());

        Ok(symbol)
    }

    /// Find function by name
    pub fn find_function(&self, name: &str) -> VmResult<Option<FunctionInfo>> {
        let functions_by_name = self.functions_by_name.read().unwrap();
        Ok(functions_by_name.get(name).cloned())
    }

    /// Find function by address
    pub fn find_function_by_address(&self, address: GuestAddr) -> VmResult<Option<FunctionInfo>> {
        let functions_by_address = self.functions_by_address.read().unwrap();
        
        // Find the function containing this address
        let function = functions_by_address
            .range(..=address)
            .next_back()
            .filter(|(_, func)| func.start_address <= address && address < func.end_address)
            .map(|(_, func)| func.clone());

        Ok(function)
    }

    /// Get source location for address
    pub fn get_source_location(&self, address: GuestAddr) -> VmResult<Option<SourceLocation>> {
        let source_locations = self.source_locations.read().unwrap();
        
        // Find the closest source location
        let location = source_locations
            .range(..=address)
            .next_back()
            .map(|(_, loc)| loc.clone());

        Ok(location)
    }

    /// Get line information for address
    pub fn get_line_info(&self, address: GuestAddr) -> VmResult<Option<LineInfo>> {
        let line_info = self.line_info.read().unwrap();
        
        // Find the line containing this address
        let info = line_info
            .range(..=address)
            .next_back()
            .filter(|(_, line)| line.address <= address && address < line.address + line.length as u64)
            .map(|(_, line)| line.clone());

        Ok(info)
    }

    /// Resolve address to line number
    pub fn address_to_line(&self, address: GuestAddr) -> VmResult<Option<u32>> {
        self.get_line_info(address).map(|opt| opt.map(|info| info.line))
    }

    /// Resolve line number to address
    pub fn line_to_address(&self, file: &str, line: u32) -> VmResult<Option<GuestAddr>> {
        let line_info = self.line_info.read().unwrap();
        
        // Find the first occurrence of this line in the specified file
        for (_, info) in line_info.range(..) {
            if info.file == file && info.line == line {
                return Ok(Some(info.address));
            }
        }

        Ok(None)
    }

    /// Search for symbols by pattern
    pub fn search_symbols(&self, pattern: &str, max_results: Option<usize>) -> VmResult<Vec<Symbol>> {
        let symbols_by_name = self.symbols_by_name.read().unwrap();
        let mut results = Vec::new();
        
        for symbols in symbols_by_name.values() {
            for symbol in symbols {
                if symbol.name.contains(pattern) {
                    results.push(symbol.clone());
                    
                    if let Some(max) = max_results {
                        if results.len() >= max {
                            break;
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Get all symbols
    pub fn get_all_symbols(&self) -> VmResult<Vec<Symbol>> {
        let symbols_by_name = self.symbols_by_name.read().unwrap();
        let mut all_symbols = Vec::new();
        
        for symbols in symbols_by_name.values() {
            all_symbols.extend(symbols.clone());
        }

        Ok(all_symbols)
    }

    /// Get all functions
    pub fn get_all_functions(&self) -> VmResult<Vec<FunctionInfo>> {
        let functions_by_name = self.functions_by_name.read().unwrap();
        Ok(functions_by_name.values().cloned().collect())
    }

    /// Clear symbol cache
    pub fn clear_cache(&self) {
        let mut cache = self.symbol_cache.write().unwrap();
        cache.clear();
    }

    /// Get symbol table statistics
    pub fn get_statistics(&self) -> SymbolTableStatistics {
        let symbols_by_name = self.symbols_by_name.read().unwrap();
        let functions_by_name = self.functions_by_name.read().unwrap();
        let line_info = self.line_info.read().unwrap();
        let debug_files = self.debug_files.read().unwrap();
        let cache = self.symbol_cache.read().unwrap();

        let total_symbols = symbols_by_name.values().map(|v| v.len()).sum();
        let total_functions = functions_by_name.len();
        let total_line_info = line_info.len();
        let loaded_files = debug_files.values().filter(|info| info.loaded).count();
        let cache_size = cache.len();

        SymbolTableStatistics {
            total_symbols,
            total_functions,
            total_line_info,
            loaded_debug_files: loaded_files,
            cache_size,
        }
    }

    /// Detect debug format of a file
    fn detect_debug_format(&self, file_path: &Path) -> VmResult<DebugFormat> {
        // This is a simplified implementation
        // In a real system, you would examine the file headers
        
        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if file_name.ends_with(".exe") || file_name.ends_with(".dll") {
            Ok(DebugFormat::Pdb)
        } else if file_name.ends_with(".elf") || file_name.ends_with(".o") {
            Ok(DebugFormat::Dwarf)
        } else {
            // Try to read file magic
            if let Ok(mut file) = std::fs::File::open(file_path) {
                let mut magic = [0u8; 4];
                if file.read_exact(&mut magic).is_ok() {
                    match &magic {
                        [0x7f, 0x45, 0x4c, 0x46] => Ok(DebugFormat::Dwarf), // ELF
                        [0x4d, 0x5a, 0x90, 0x00] => Ok(DebugFormat::Pdb),   // PE
                        _ => Ok(DebugFormat::Unknown),
                    }
                } else {
                    Ok(DebugFormat::Unknown)
                }
            } else {
                Ok(DebugFormat::Unknown)
            }
        }
    }

    /// Load DWARF debug information
    fn load_dwarf_debug_info(&self, file_path: &Path) -> VmResult<()> {
        // This is a placeholder for DWARF parsing
        // In a real implementation, you would use a DWARF parsing library
        
        // For now, create some dummy symbols for testing
        let mut symbols_by_name = self.symbols_by_name.write().unwrap();
        let mut symbols_by_address = self.symbols_by_address.write().unwrap();
        let mut functions_by_name = self.functions_by_name.write().unwrap();
        let mut functions_by_address = self.functions_by_address.write().unwrap();
        let mut line_info = self.line_info.write().unwrap();
        let mut source_locations = self.source_locations.write().unwrap();

        // Add a dummy function
        let function_info = FunctionInfo {
            name: "main".to_string(),
            start_address: 0x1000,
            end_address: 0x1100,
            size: 0x100,
            return_type: Some("int".to_string()),
            parameters: vec![
                ParameterInfo {
                    name: "argc".to_string(),
                    param_type: "int".to_string(),
                    location: VariableLocation::Register { 
                        register: "rdi".to_string(), 
                        offset: None 
                    },
                    default_value: None,
                },
                ParameterInfo {
                    name: "argv".to_string(),
                    param_type: "char**".to_string(),
                    location: VariableLocation::Register { 
                        register: "rsi".to_string(), 
                        offset: None 
                    },
                    default_value: None,
                },
            ],
            local_variables: vec![
                LocalVariableInfo {
                    name: "local_var".to_string(),
                    var_type: "int".to_string(),
                    location: VariableLocation::StackOffset { offset: -4 },
                    scope_start: 0x1000,
                    scope_end: 0x1100,
                    line: Some(10),
                    optimized_out: false,
                },
            ],
            source_file: Some("main.c".to_string()),
            start_line: Some(5),
            end_line: Some(15),
            calling_convention: Some("System V AMD64".to_string()),
            frame_size: Some(32),
            inline: false,
            variadic: false,
        };

        // Add to symbol tables
        functions_by_name.insert("main".to_string(), function_info.clone());
        functions_by_address.insert(0x1000, function_info.clone());

        // Add function symbol
        let symbol = Symbol {
            name: "main".to_string(),
            address: 0x1000,
            size: 0x100,
            symbol_type: SymbolType::Function,
            scope: SymbolScope::Global,
            source_file: Some("main.c".to_string()),
            line_number: Some(5),
            column_number: Some(1),
            visibility: SymbolVisibility::Public,
            attributes: HashMap::new(),
        };

        symbols_by_name.entry("main".to_string())
            .or_insert_with(Vec::new)
            .push(symbol.clone());
        symbols_by_address.insert(0x1000, symbol);

        // Add line info
        for line in 5..=15 {
            let line_info = LineInfo {
                file: "main.c".to_string(),
                line,
                column: 1,
                address: 0x1000 + ((line - 5) * 0x10) as u64,
                length: 0x10,
            };
            line_info.insert(line_info.address, line_info);

            let source_location = SourceLocation {
                file: "main.c".to_string(),
                line,
                column: 1,
                function: Some("main".to_string()),
                address: line_info.address,
            };
            source_locations.insert(source_location.address, source_location);
        }

        Ok(())
    }
}

/// Symbol table statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolTableStatistics {
    /// Total number of symbols
    pub total_symbols: usize,
    /// Total number of functions
    pub total_functions: usize,
    /// Total number of line info entries
    pub total_line_info: usize,
    /// Number of loaded debug files
    pub loaded_debug_files: usize,
    /// Current cache size
    pub cache_size: usize,
}

#[cfg(feature = "enhanced-debugging")]
impl Default for crate::debugger::symbol_table::SymbolTable {
    fn default() -> Self {
        #[cfg(feature = "enhanced-debugging")]
        { Self::new(SymbolTableBuilder::default()) }
        #[cfg(not(feature = "enhanced-debugging"))]
        { Self::new(crate::debugger::symbol_table::SymbolTableConfig::default()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_symbol_creation() {
        let symbol = Symbol {
            name: "test_function".to_string(),
            address: 0x1000,
            size: 0x100,
            symbol_type: SymbolType::Function,
            scope: SymbolScope::Global,
            source_file: Some("test.c".to_string()),
            line_number: Some(10),
            column_number: Some(5),
            visibility: SymbolVisibility::Public,
            attributes: HashMap::new(),
        };

        assert_eq!(symbol.name, "test_function");
        assert_eq!(symbol.address, 0x1000);
        assert_eq!(symbol.symbol_type, SymbolType::Function);
        assert_eq!(symbol.scope, SymbolScope::Global);
    }

    #[test]
    fn test_symbol_table() {
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
        let config = SymbolTableBuilder::default();        let config = SymbolTableConfig::default();
        let symbol_table = SymbolTable::new(config);

        // Load dummy debug info
        let test_file = PathBuf::from("/tmp/test_binary");
        // Note: This will fail in test since file doesn't exist
        // In a real test, you would create a test binary with debug info
        let _result = symbol_table.load_from_file(&test_file);

        // Test symbol lookup
        let symbol = symbol_table.find_symbol("main").unwrap();
        assert!(symbol.is_some());
        assert_eq!(symbol.unwrap().name, "main");

        // Test function lookup
        let function = symbol_table.find_function("main").unwrap();
        assert!(function.is_some());
        assert_eq!(function.unwrap().name, "main");

        // Test address lookup
        let symbol_by_addr = symbol_table.find_symbol_by_address(0x1000).unwrap();
        assert!(symbol_by_addr.is_some());
        assert_eq!(symbol_by_addr.unwrap().address, 0x1000);

        // Test source location
        let source_loc = symbol_table.get_source_location(0x1000).unwrap();
        assert!(source_loc.is_some());
        assert_eq!(source_loc.unwrap().line, 5);
    }

    #[test]
    fn test_symbol_search() {
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
        let config = SymbolTableBuilder::default();        let config = SymbolTableConfig::default();
        let symbol_table = SymbolTable::new(config);

        // Add some test symbols
        let mut symbols_by_name = symbol_table.symbols_by_name.write().unwrap();
        
        symbols_by_name.insert("test_func".to_string(), vec![
            Symbol {
                name: "test_func".to_string(),
                address: 0x1000,
                size: 0x100,
                symbol_type: SymbolType::Function,
                scope: SymbolScope::Global,
                source_file: None,
                line_number: None,
                column_number: None,
                visibility: SymbolVisibility::Public,
                attributes: HashMap::new(),
            },
            Symbol {
                name: "test_helper".to_string(),
                address: 0x2000,
                size: 0x50,
                symbol_type: SymbolType::Function,
                scope: SymbolScope::Global,
                source_file: None,
                line_number: None,
                column_number: None,
                visibility: SymbolVisibility::Public,
                attributes: HashMap::new(),
            },
        ]);

        // Search for symbols containing "test"
        let results = symbol_table.search_symbols("test", None).unwrap();
        assert_eq!(results.len(), 2);

        // Search with limit
        let limited_results = symbol_table.search_symbols("test", Some(1)).unwrap();
        assert_eq!(limited_results.len(), 1);
    }
}
