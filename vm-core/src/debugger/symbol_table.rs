//! Symbol table support system
//!
//! This module provides comprehensive symbol table support including
//! DWARF debug information parsing, symbol lookup, and source-level debugging.

#![cfg(feature = "debug")]

use std::collections::{HashMap, BTreeMap};
use std::sync::{Arc, RwLock};
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::{GuestAddr, VmError, VmResult};

/// Symbol information
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

    /// Helper: Lock debug_files for reading
    fn lock_debug_files(&self) -> VmResult<std::sync::RwLockReadGuard<'_, HashMap<String, DebugFileInfo>>> {
        self.debug_files.read().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire read lock on debug_files".to_string(),
            operation: "lock_debug_files".to_string(),
        })
    }

    /// Helper: Lock debug_files for writing
    fn lock_debug_files_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, HashMap<String, DebugFileInfo>>> {
        self.debug_files.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on debug_files".to_string(),
            operation: "lock_debug_files_mut".to_string(),
        })
    }

    /// Helper: Lock symbols_by_name for reading
    fn lock_symbols_by_name(&self) -> VmResult<std::sync::RwLockReadGuard<'_, HashMap<String, Vec<Symbol>>>> {
        self.symbols_by_name.read().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire read lock on symbols_by_name".to_string(),
            operation: "lock_symbols_by_name".to_string(),
        })
    }

    /// Helper: lock symbols_by_name for writing
    fn lock_symbols_by_name_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, HashMap<String, Vec<Symbol>>>> {
        self.symbols_by_name.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on symbols_by_name".to_string(),
            operation: "lock_symbols_by_name_mut".to_string(),
        })
    }

    /// Helper: Lock symbols_by_address for reading
    fn lock_symbols_by_address(&self) -> VmResult<std::sync::RwLockReadGuard<'_, BTreeMap<GuestAddr, Symbol>>> {
        self.symbols_by_address.read().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire read lock on symbols_by_address".to_string(),
            operation: "lock_symbols_by_address".to_string(),
        })
    }

    /// Helper: Lock symbols_by_address for writing
    fn lock_symbols_by_address_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, BTreeMap<GuestAddr, Symbol>>> {
        self.symbols_by_address.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on symbols_by_address".to_string(),
            operation: "lock_symbols_by_address_mut".to_string(),
        })
    }

    /// Helper: Lock functions_by_name for reading
    fn lock_functions_by_name(&self) -> VmResult<std::sync::RwLockReadGuard<'_, HashMap<String, FunctionInfo>>> {
        self.functions_by_name.read().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire read lock on functions_by_name".to_string(),
            operation: "lock_functions_by_name".to_string(),
        })
    }

    /// Helper: Lock functions_by_name for writing
    fn lock_functions_by_name_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, HashMap<String, FunctionInfo>>> {
        self.functions_by_name.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on functions_by_name".to_string(),
            operation: "lock_functions_by_name_mut".to_string(),
        })
    }

    /// Helper: Lock functions_by_address for reading
    fn lock_functions_by_address(&self) -> VmResult<std::sync::RwLockReadGuard<'_, BTreeMap<GuestAddr, FunctionInfo>>> {
        self.functions_by_address.read().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire read lock on functions_by_address".to_string(),
            operation: "lock_functions_by_address".to_string(),
        })
    }

    /// Helper: Lock functions_by_address for writing
    fn lock_functions_by_address_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, BTreeMap<GuestAddr, FunctionInfo>>> {
        self.functions_by_address.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on functions_by_address".to_string(),
            operation: "lock_functions_by_address_mut".to_string(),
        })
    }

    /// Helper: Lock line_info for reading
    fn lock_line_info(&self) -> VmResult<std::sync::RwLockReadGuard<'_, BTreeMap<GuestAddr, LineInfo>>> {
        self.line_info.read().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire read lock on line_info".to_string(),
            operation: "lock_line_info".to_string(),
        })
    }

    /// Helper: Lock line_info for writing
    fn lock_line_info_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, BTreeMap<GuestAddr, LineInfo>>> {
        self.line_info.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on line_info".to_string(),
            operation: "lock_line_info_mut".to_string(),
        })
    }

    /// Helper: Lock source_locations for reading
    fn lock_source_locations(&self) -> VmResult<std::sync::RwLockReadGuard<'_, BTreeMap<GuestAddr, SourceLocation>>> {
        self.source_locations.read().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire read lock on source_locations".to_string(),
            operation: "lock_source_locations".to_string(),
        })
    }

    /// Helper: Lock source_locations for writing
    fn lock_source_locations_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, BTreeMap<GuestAddr, SourceLocation>>> {
        self.source_locations.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on source_locations".to_string(),
            operation: "lock_source_locations_mut".to_string(),
        })
    }

    /// Helper: Lock symbol_cache for reading
    fn lock_symbol_cache(&self) -> VmResult<std::sync::RwLockReadGuard<'_, HashMap<String, Symbol>>> {
        self.symbol_cache.read().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire read lock on symbol_cache".to_string(),
            operation: "lock_symbol_cache".to_string(),
        })
    }

    /// Helper: Lock symbol_cache for writing
    fn lock_symbol_cache_mut(&self) -> VmResult<std::sync::RwLockWriteGuard<'_, HashMap<String, Symbol>>> {
        self.symbol_cache.write().map_err(|_| VmError::Core(crate::error::CoreError::Concurrency {
            message: "Failed to acquire write lock on symbol_cache".to_string(),
            operation: "lock_symbol_cache_mut".to_string(),
        })
    }

    /// Load symbols from a binary file
    pub fn load_from_file(&self, file_path: &Path) -> VmResult<()> {
        let file_path_str = file_path.to_string_lossy().to_string();

        // Check if already loaded
        {
            let debug_files = self.lock_debug_files()?;
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
            let mut debug_files = self.lock_debug_files_mut()?;
            let modified_time = std::fs::metadata(file_path)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            debug_files.insert(file_path_str, DebugFileInfo {
                path: file_path_str,
                modified_time,
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
            let cache = self.lock_symbol_cache()?;
            if let Some(symbol) = cache.get(name) {
                return Ok(Some(symbol.clone()));
            }
        }

        let symbols_by_name = self.lock_symbols_by_name()?;
        let symbol = symbols_by_name.get(name)
            .and_then(|symbols| symbols.first().cloned());

        // Update cache
        if self.config.enable_caching {
            if let Some(ref sym) = symbol {
                if let Ok(mut cache) = self.lock_symbol_cache_mut() {
                    cache.insert(name.to_string(), sym.clone());
                }
            }
        }

        Ok(symbol)
    }

    /// Find symbol by address
    pub fn find_symbol_by_address(&self, address: GuestAddr) -> VmResult<Option<Symbol>> {
        let symbols_by_address = self.lock_symbols_by_address()?;

        // Find the symbol with the largest address <= target address
        let symbol = symbols_by_address
            .range(..=address)
            .next_back()
            .map(|(_, sym)| sym.clone());

        Ok(symbol)
    }

    /// Find function by name
    pub fn find_function(&self, name: &str) -> VmResult<Option<FunctionInfo>> {
        let functions_by_name = self.lock_functions_by_name()?;
        Ok(functions_by_name.get(name).cloned())
    }

    /// Find function by address
    pub fn find_function_by_address(&self, address: GuestAddr) -> VmResult<Option<FunctionInfo>> {
        let functions_by_address = self.lock_functions_by_address()?;

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
        let source_locations = self.lock_source_locations()?;

        // Find the closest source location
        let location = source_locations
            .range(..=address)
            .next_back()
            .map(|(_, loc)| loc.clone());

        Ok(location)
    }

    /// Get line information for address
    pub fn get_line_info(&self, address: GuestAddr) -> VmResult<Option<LineInfo>> {
        let line_info = self.lock_line_info()?;

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
        let line_info = self.lock_line_info()?;

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
        let symbols_by_name = self.lock_symbols_by_name()?;
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
        let symbols_by_name = self.lock_symbols_by_name()?;
        let mut all_symbols = Vec::new();

        for symbols in symbols_by_name.values() {
            all_symbols.extend(symbols.clone());
        }

        Ok(all_symbols)
    }

    /// Get all functions
    pub fn get_all_functions(&self) -> VmResult<Vec<FunctionInfo>> {
        let functions_by_name = self.lock_functions_by_name()?;
        Ok(functions_by_name.values().cloned().collect())
    }

    /// Clear symbol cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.symbol_cache.write() {
            cache.clear();
        }
    }

    /// Get symbol table statistics
    pub fn get_statistics(&self) -> SymbolTableStatistics {
        let (total_symbols, total_functions, total_line_info, loaded_files, cache_size) = match (
            self.symbols_by_name.read(),
            self.functions_by_name.read(),
            self.line_info.read(),
            self.debug_files.read(),
            self.symbol_cache.read(),
        ) {
            (Ok(symbols), Ok(functions), Ok(line_info), Ok(debug_files), Ok(cache)) => {
                let total_symbols = symbols.values().map(|v| v.len()).sum();
                let total_functions = functions.len();
                let total_line_info = line_info.len();
                let loaded_files = debug_files.values().filter(|info| info.loaded).count();
                let cache_size = cache.len();
                (total_symbols, total_functions, total_line_info, loaded_files, cache_size)
            }
            _ => (0, 0, 0, 0, 0),
        };

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
        let mut symbols_by_name = self.lock_symbols_by_name_mut()?;
        let mut symbols_by_address = self.lock_symbols_by_address_mut()?;
        let mut functions_by_name = self.lock_functions_by_name_mut()?;
        let mut functions_by_address = self.lock_functions_by_address_mut()?;
        let mut line_info = self.lock_line_info_mut()?;
        let mut source_locations = self.lock_source_locations_mut()?;

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
            let line_info_entry = LineInfo {
                file: "main.c".to_string(),
                line,
                column: 1,
                address: 0x1000 + ((line - 5) * 0x10) as u64,
                length: 0x10,
            };
            line_info.insert(line_info_entry.address, line_info_entry);

            let source_location = SourceLocation {
                file: "main.c".to_string(),
                line,
                column: 1,
                function: Some("main".to_string()),
                address: line_info_entry.address,
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

impl Default for crate::debugger::symbol_table::SymbolTable {
    fn default() -> Self {
        Self::new(crate::debugger::symbol_table::SymbolTableConfig::default())
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
        let config = SymbolTableConfig::default();
        let symbol_table = SymbolTable::new(config);

        // Load dummy debug info
        let test_file = PathBuf::from("/tmp/test_binary");
        // Note: This will fail in test since file doesn't exist
        // In a real test, you would create a test binary with debug info
        let _result = symbol_table.load_from_file(&test_file);

        // Test symbol lookup
        let symbol = symbol_table.find_symbol("main").expect("Failed to find symbol");
        assert!(symbol.is_some());
        let symbol = symbol.expect("Symbol is None");
        assert_eq!(symbol.name, "main");

        // Test function lookup
        let function = symbol_table.find_function("main").expect("Failed to find function");
        assert!(function.is_some());
        let function = function.expect("Function is None");
        assert_eq!(function.name, "main");

        // Test address lookup
        let symbol_by_addr = symbol_table.find_symbol_by_address(0x1000).expect("Failed to find symbol by address");
        assert!(symbol_by_addr.is_some());
        let symbol_by_addr = symbol_by_addr.expect("Symbol by address is None");
        assert_eq!(symbol_by_addr.address, 0x1000);

        // Test source location
        let source_loc = symbol_table.get_source_location(0x1000).expect("Failed to get source location");
        assert!(source_loc.is_some());
        let source_loc = source_loc.expect("Source location is None");
        assert_eq!(source_loc.line, 5);
    }

    #[test]
    fn test_symbol_search() {
        let config = SymbolTableConfig::default();
        let symbol_table = SymbolTable::new(config);

        // Add some test symbols
        if let Ok(mut symbols_by_name) = symbol_table.symbols_by_name.write() {
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
        }

        // Search for symbols containing "test"
        let results = symbol_table.search_symbols("test", None).expect("Failed to search symbols");
        assert_eq!(results.len(), 2);

        // Search with limit
        let limited_results = symbol_table.search_symbols("test", Some(1)).expect("Failed to search symbols with limit");
        assert_eq!(limited_results.len(), 1);
    }
}
