//! Common register management for VM cross-architecture translation
//!
//! This module provides unified register management across different architectures,
//! including register mapping, allocation, and lifecycle management.

use std::collections::{HashMap, HashSet};

use thiserror::Error;
use vm_core::VmError;
use vm_core::error::CoreError;

use crate::encoding::{Architecture, RegId};

/// Errors that can occur during register management
#[derive(Error, Debug, Clone, PartialEq)]
pub enum RegisterError {
    #[error("Invalid register ID: {0}")]
    InvalidRegister(RegId),
    #[error("Register {0} not found in register set")]
    RegisterNotFound(RegId),
    #[error("Register {0} is already allocated")]
    RegisterAlreadyAllocated(RegId),
    #[error("No available registers for class {0:?}")]
    NoAvailableRegisters(RegisterClass),
    #[error("Invalid register mapping from {0} to {1}")]
    InvalidMapping(RegId, RegId),
    #[error("Register class {0:?} not supported by architecture {1:?}")]
    UnsupportedRegisterClass(RegisterClass, Architecture),
    #[error("Register conflict: {0}")]
    RegisterConflict(String),
}

impl From<RegisterError> for VmError {
    fn from(err: RegisterError) -> Self {
        match err {
            RegisterError::InvalidRegister(reg_id) => VmError::Core(CoreError::InvalidParameter {
                name: "register_id".to_string(),
                value: reg_id.to_string(),
                message: "Invalid register ID".to_string(),
            }),
            RegisterError::RegisterNotFound(reg_id) => VmError::Core(CoreError::InvalidParameter {
                name: "register_id".to_string(),
                value: reg_id.to_string(),
                message: "Register not found in register set".to_string(),
            }),
            RegisterError::RegisterAlreadyAllocated(reg_id) => {
                VmError::Core(CoreError::InvalidState {
                    message: format!("Register {} is already allocated", reg_id),
                    current: "allocated".to_string(),
                    expected: "free".to_string(),
                })
            }
            RegisterError::NoAvailableRegisters(class) => {
                VmError::Core(CoreError::ResourceExhausted {
                    resource: format!("registers in class {:?}", class),
                    current: 0,
                    limit: 0,
                })
            }
            RegisterError::InvalidMapping(from, to) => VmError::Core(CoreError::InvalidParameter {
                name: "register_mapping".to_string(),
                value: format!("{} -> {}", from, to),
                message: "Invalid register mapping".to_string(),
            }),
            RegisterError::UnsupportedRegisterClass(class, arch) => {
                VmError::Core(CoreError::NotSupported {
                    feature: format!("Register class {:?} for architecture {:?}", class, arch),
                    module: "vm-cross-arch-support::register".to_string(),
                })
            }
            RegisterError::RegisterConflict(msg) => VmError::Core(CoreError::Internal {
                message: format!("Register conflict: {}", msg),
                module: "vm-cross-arch-support::register".to_string(),
            }),
        }
    }
}

/// Register classes for categorizing registers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegisterClass {
    GeneralPurpose,
    FloatingPoint,
    Vector,
    Special,
    Control,
    Status,
    System,
    Predicate,
    Application,
}

/// Register types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterType {
    Integer { width: u8 },
    Float { width: u8 },
    Vector { width: u8, lanes: u8 },
    Special,
}

impl Default for RegisterType {
    fn default() -> Self {
        Self::Integer { width: 64 }
    }
}

/// Register information
#[derive(Debug, Clone)]
pub struct RegisterInfo {
    pub id: RegId,
    pub name: String,
    pub class: RegisterClass,
    pub reg_type: RegisterType,
    pub caller_saved: bool,
    pub callee_saved: bool,
    pub volatile: bool,
    pub reserved: bool,
    pub aliases: Vec<RegId>,
    pub overlapping: Vec<RegId>,
}

impl RegisterInfo {
    pub fn new(
        id: RegId,
        name: impl Into<String>,
        class: RegisterClass,
        reg_type: RegisterType,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            class,
            reg_type,
            caller_saved: false,
            callee_saved: false,
            volatile: false,
            reserved: false,
            aliases: Vec::new(),
            overlapping: Vec::new(),
        }
    }

    pub fn with_caller_saved(mut self) -> Self {
        self.caller_saved = true;
        self
    }

    pub fn with_callee_saved(mut self) -> Self {
        self.callee_saved = true;
        self
    }

    pub fn with_volatile(mut self) -> Self {
        self.volatile = true;
        self
    }

    pub fn with_reserved(mut self) -> Self {
        self.reserved = true;
        self
    }

    pub fn with_alias(mut self, alias: RegId) -> Self {
        self.aliases.push(alias);
        self
    }

    pub fn with_overlapping(mut self, overlapping: RegId) -> Self {
        self.overlapping.push(overlapping);
        self
    }
}

/// Register set for an architecture
#[derive(Debug, Clone)]
pub struct RegisterSet {
    pub architecture: Architecture,
    pub general_purpose: Vec<RegisterInfo>,
    pub floating_point: Vec<RegisterInfo>,
    pub vector: Vec<RegisterInfo>,
    pub special: HashMap<String, RegisterInfo>,
    pub control: Vec<RegisterInfo>,
    pub status: Vec<RegisterInfo>,
    pub system: Vec<RegisterInfo>,
    pub predicate: Vec<RegisterInfo>,
    pub application: Vec<RegisterInfo>,
}

impl RegisterSet {
    pub fn new(architecture: Architecture) -> Self {
        Self {
            architecture,
            general_purpose: Vec::new(),
            floating_point: Vec::new(),
            vector: Vec::new(),
            special: HashMap::new(),
            control: Vec::new(),
            status: Vec::new(),
            system: Vec::new(),
            predicate: Vec::new(),
            application: Vec::new(),
        }
    }

    pub fn add_register(&mut self, info: RegisterInfo) {
        match info.class {
            RegisterClass::GeneralPurpose => self.general_purpose.push(info),
            RegisterClass::FloatingPoint => self.floating_point.push(info),
            RegisterClass::Vector => self.vector.push(info),
            RegisterClass::Special => {
                self.special.insert(info.name.clone(), info);
            }
            RegisterClass::Control => self.control.push(info),
            RegisterClass::Status => self.status.push(info),
            RegisterClass::System => self.system.push(info),
            RegisterClass::Predicate => self.predicate.push(info),
            RegisterClass::Application => self.application.push(info),
        }
    }

    pub fn get_register(&self, id: RegId) -> Option<&RegisterInfo> {
        self.general_purpose
            .iter()
            .chain(self.floating_point.iter())
            .chain(self.vector.iter())
            .chain(self.special.values())
            .chain(self.control.iter())
            .chain(self.status.iter())
            .chain(self.system.iter())
            .chain(self.predicate.iter())
            .chain(self.application.iter())
            .find(|reg| reg.id == id)
    }

    pub fn get_register_by_name(&self, name: &str) -> Option<&RegisterInfo> {
        self.general_purpose
            .iter()
            .chain(self.floating_point.iter())
            .chain(self.vector.iter())
            .chain(self.special.values())
            .chain(self.control.iter())
            .chain(self.status.iter())
            .chain(self.system.iter())
            .chain(self.predicate.iter())
            .chain(self.application.iter())
            .find(|reg| reg.name == name)
    }

    pub fn get_registers_by_class(&self, class: RegisterClass) -> Vec<&RegisterInfo> {
        // Collect with better performance by estimating capacity
        let capacity = match class {
            RegisterClass::GeneralPurpose => self.general_purpose.len(),
            RegisterClass::FloatingPoint => self.floating_point.len(),
            RegisterClass::Vector => self.vector.len(),
            RegisterClass::Special => self.special.len(),
            RegisterClass::Control => self.control.len(),
            RegisterClass::Status => self.status.len(),
            RegisterClass::System => self.system.len(),
            RegisterClass::Predicate => self.predicate.len(),
            RegisterClass::Application => self.application.len(),
        };

        let mut result = Vec::with_capacity(capacity);

        match class {
            RegisterClass::GeneralPurpose => result.extend(self.general_purpose.iter()),
            RegisterClass::FloatingPoint => result.extend(self.floating_point.iter()),
            RegisterClass::Vector => result.extend(self.vector.iter()),
            RegisterClass::Special => result.extend(self.special.values()),
            RegisterClass::Control => result.extend(self.control.iter()),
            RegisterClass::Status => result.extend(self.status.iter()),
            RegisterClass::System => result.extend(self.system.iter()),
            RegisterClass::Predicate => result.extend(self.predicate.iter()),
            RegisterClass::Application => result.extend(self.application.iter()),
        }

        result
    }

    pub fn get_available_registers(&self, class: RegisterClass) -> Vec<&RegisterInfo> {
        self.get_registers_by_class(class)
            .into_iter()
            .filter(|reg| !reg.reserved)
            .collect()
    }

    /// Create a RegisterSet with virtual registers for IR translation
    pub fn with_virtual_registers(arch: Architecture, virtual_count: usize) -> Self {
        let mut set = Self::new(arch);

        // Add virtual registers (for intermediate representation)
        for i in 0..virtual_count {
            let reg_id = RegId(i as u16);
            let info = RegisterInfo::new(
                reg_id,
                format!("v{}", i),
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            );
            set.add_register(info);
        }

        set
    }
}

/// Register mapping strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MappingStrategy {
    #[default]
    Direct, // Direct 1:1 mapping between registers (most common)
    /// Windowed mapping (e.g., RISC-V register windows)
    Windowed { window_size: u8, window_count: u8 },
    /// Stack-based mapping (e.g., x86 stack-based floating-point)
    StackBased { stack_size: u8 },
    /// Optimized mapping with register allocation
    Optimized,
    /// Virtual register mapping for IR
    Virtual,
    /// Custom mapping function
    Custom,
}

/// Register mapper for cross-architecture register conversion
#[derive(Debug)]
pub struct RegisterMapper {
    source_set: RegisterSet,
    target_set: RegisterSet,
    mapping_strategy: MappingStrategy,
    allocation_cache: HashMap<RegId, RegId>,
    reverse_cache: HashMap<RegId, RegId>,
    allocated_registers: HashSet<RegId>,
    reserved_registers: HashSet<RegId>,
}

impl RegisterMapper {
    pub fn new(
        source_set: RegisterSet,
        target_set: RegisterSet,
        mapping_strategy: MappingStrategy,
    ) -> Self {
        Self {
            source_set,
            target_set,
            mapping_strategy,
            allocation_cache: HashMap::new(),
            reverse_cache: HashMap::new(),
            allocated_registers: HashSet::new(),
            reserved_registers: HashSet::new(),
        }
    }

    /// Map a source register to a target register
    pub fn map_register(&mut self, source_reg: RegId) -> Result<RegId, RegisterError> {
        // Check if we already have a mapping
        if let Some(&mapped) = self.allocation_cache.get(&source_reg) {
            return Ok(mapped);
        }

        // Get source register info
        let source_info = self
            .source_set
            .get_register(source_reg)
            .ok_or(RegisterError::RegisterNotFound(source_reg))?;

        // Find a suitable target register
        let target_reg = self.find_target_register(source_info)?;

        // Allocate the register
        self.allocate_register(source_reg, target_reg)?;

        Ok(target_reg)
    }

    /// Reverse map a target register back to source register
    pub fn reverse_map(&self, target_reg: RegId) -> Option<RegId> {
        self.reverse_cache.get(&target_reg).copied()
    }

    /// Check if a register is allocated
    pub fn is_allocated(&self, reg: RegId) -> bool {
        self.allocated_registers.contains(&reg)
    }

    /// Reserve a register (prevent allocation)
    pub fn reserve_register(&mut self, reg: RegId) -> Result<(), RegisterError> {
        if self.allocated_registers.contains(&reg) {
            return Err(RegisterError::RegisterAlreadyAllocated(reg));
        }
        self.reserved_registers.insert(reg);
        Ok(())
    }

    /// Release a reservation
    pub fn release_reservation(&mut self, reg: RegId) {
        self.reserved_registers.remove(&reg);
    }

    /// Free a register allocation
    pub fn free_register(&mut self, reg: RegId) -> Result<(), RegisterError> {
        if let Some(&source_reg) = self.reverse_cache.get(&reg) {
            self.allocation_cache.remove(&source_reg);
            self.reverse_cache.remove(&reg);
            self.allocated_registers.remove(&reg);
            Ok(())
        } else {
            Err(RegisterError::InvalidRegister(reg))
        }
    }

    /// Free all register allocations
    pub fn free_all(&mut self) {
        self.allocation_cache.clear();
        self.reverse_cache.clear();
        self.allocated_registers.clear();
    }

    /// Get mapping statistics
    pub fn get_stats(&self) -> MappingStats {
        MappingStats {
            total_mappings: self.allocation_cache.len(),
            allocated_registers: self.allocated_registers.len(),
            reserved_registers: self.reserved_registers.len(),
            strategy: self.mapping_strategy,
        }
    }

    /// Find a suitable target register for the source register
    fn find_target_register(&self, source_info: &RegisterInfo) -> Result<RegId, RegisterError> {
        let target_candidates = self.target_set.get_available_registers(source_info.class);

        if target_candidates.is_empty() {
            return Err(RegisterError::NoAvailableRegisters(source_info.class));
        }

        // Try to find a register with similar properties
        for &candidate in target_candidates.iter() {
            if !self.allocated_registers.contains(&candidate.id)
                && !self.reserved_registers.contains(&candidate.id)
                && self.is_compatible(source_info, candidate)
            {
                return Ok(candidate.id);
            }
        }

        // If no compatible register found, take any available register
        for &candidate in target_candidates.iter() {
            if !self.allocated_registers.contains(&candidate.id)
                && !self.reserved_registers.contains(&candidate.id)
            {
                return Ok(candidate.id);
            }
        }

        Err(RegisterError::NoAvailableRegisters(source_info.class))
    }

    /// Check if source and target registers are compatible
    fn is_compatible(&self, source: &RegisterInfo, target: &RegisterInfo) -> bool {
        // Check register type compatibility
        match (&source.reg_type, &target.reg_type) {
            (RegisterType::Integer { width: sw }, RegisterType::Integer { width: tw }) => {
                // Target register should be at least as wide as source
                tw >= sw
            }
            (RegisterType::Float { width: sw }, RegisterType::Float { width: tw }) => tw >= sw,
            (
                RegisterType::Vector {
                    width: sw,
                    lanes: sl,
                },
                RegisterType::Vector {
                    width: tw,
                    lanes: tl,
                },
            ) => tw >= sw && tl >= sl,
            (RegisterType::Special, RegisterType::Special) => true,
            _ => false,
        }
    }

    /// Allocate a register mapping
    fn allocate_register(
        &mut self,
        source_reg: RegId,
        target_reg: RegId,
    ) -> Result<(), RegisterError> {
        if self.allocated_registers.contains(&target_reg) {
            return Err(RegisterError::RegisterAlreadyAllocated(target_reg));
        }

        self.allocation_cache.insert(source_reg, target_reg);
        self.reverse_cache.insert(target_reg, source_reg);
        self.allocated_registers.insert(target_reg);

        Ok(())
    }
}

/// Mapping statistics
#[derive(Debug, Clone)]
pub struct MappingStats {
    pub total_mappings: usize,
    pub allocated_registers: usize,
    pub reserved_registers: usize,
    pub strategy: MappingStrategy,
}

/// Register allocator for optimized register allocation
#[derive(Debug)]
pub struct RegisterAllocator {
    register_set: RegisterSet,
    allocated: HashMap<RegId, AllocationInfo>,
    free_lists: HashMap<RegisterClass, Vec<RegId>>,
    allocation_order: HashMap<RegisterClass, Vec<RegId>>,
}

#[derive(Debug, Clone)]
struct AllocationInfo {
    _reg_id: RegId,
    class: RegisterClass,
    last_used: u64,
    spill_cost: f32,
}

impl RegisterAllocator {
    pub fn new(register_set: RegisterSet) -> Self {
        let mut allocator = Self {
            register_set,
            allocated: HashMap::new(),
            free_lists: HashMap::new(),
            allocation_order: HashMap::new(),
        };

        allocator.initialize_free_lists();
        allocator
    }

    /// Allocate a register of the specified class
    pub fn allocate(&mut self, class: RegisterClass) -> Result<RegId, RegisterError> {
        let free_list =
            self.free_lists
                .get_mut(&class)
                .ok_or(RegisterError::UnsupportedRegisterClass(
                    class,
                    self.register_set.architecture,
                ))?;

        if let Some(reg_id) = free_list.pop() {
            let info = AllocationInfo {
                _reg_id: reg_id,
                class,
                last_used: 0,
                spill_cost: 0.0,
            };
            self.allocated.insert(reg_id, info);
            Ok(reg_id)
        } else {
            // Try to spill a register
            self.spill_register(class)
        }
    }

    /// Free a register
    pub fn free(&mut self, reg_id: RegId) -> Result<(), RegisterError> {
        if let Some(info) = self.allocated.remove(&reg_id) {
            let free_list = self.free_lists.get_mut(&info.class).ok_or(
                RegisterError::UnsupportedRegisterClass(info.class, self.register_set.architecture),
            )?;
            free_list.push(reg_id);
            Ok(())
        } else {
            Err(RegisterError::InvalidRegister(reg_id))
        }
    }

    /// Mark a register as used
    pub fn mark_used(&mut self, reg_id: RegId, timestamp: u64) {
        if let Some(info) = self.allocated.get_mut(&reg_id) {
            info.last_used = timestamp;
        }
    }

    /// Set spill cost for a register
    pub fn set_spill_cost(&mut self, reg_id: RegId, cost: f32) {
        if let Some(info) = self.allocated.get_mut(&reg_id) {
            info.spill_cost = cost;
        }
    }

    /// Initialize free lists with available registers
    fn initialize_free_lists(&mut self) {
        for class in [
            RegisterClass::GeneralPurpose,
            RegisterClass::FloatingPoint,
            RegisterClass::Vector,
            RegisterClass::Control,
            RegisterClass::Status,
            RegisterClass::System,
            RegisterClass::Predicate,
            RegisterClass::Application,
        ] {
            let registers = self.register_set.get_available_registers(class);
            let free_list: Vec<RegId> = registers.iter().map(|reg| reg.id).collect();
            self.free_lists.insert(class, free_list);

            // Set allocation order based on register preferences
            let allocation_order: Vec<RegId> = registers
                .iter()
                .map(|reg| {
                    // Prefer caller-saved registers first
                    if reg.caller_saved {
                        reg.id
                    } else {
                        // Put callee-saved registers at the end
                        reg.id
                    }
                })
                .collect();
            self.allocation_order.insert(class, allocation_order);
        }
    }

    /// Spill a register to make room
    fn spill_register(&mut self, class: RegisterClass) -> Result<RegId, RegisterError> {
        let candidates: Vec<_> = self
            .allocated
            .iter()
            .filter(|(_, info)| info.class == class)
            .collect();

        if candidates.is_empty() {
            return Err(RegisterError::NoAvailableRegisters(class));
        }

        // Find the register with the lowest spill cost
        let (&reg_id, _) = candidates
            .into_iter()
            .min_by(|(_, a), (_, b)| {
                a.spill_cost
                    .partial_cmp(&b.spill_cost)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        let _info = self.allocated.remove(&reg_id).unwrap();
        let free_list = self.free_lists.get_mut(&class).unwrap();
        free_list.push(reg_id);

        Ok(reg_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_register_set() -> RegisterSet {
        let mut set = RegisterSet::new(Architecture::X86_64);

        // Add some general purpose registers
        for i in 0..16 {
            let info = RegisterInfo::new(
                RegId(i),
                format!("r{}", i),
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            )
            .with_caller_saved();
            set.add_register(info);
        }

        set
    }

    #[test]
    fn test_register_mapping() {
        let source_set = create_test_register_set();
        let target_set = create_test_register_set();
        let mut mapper = RegisterMapper::new(source_set, target_set, MappingStrategy::Direct);

        let mapped = mapper.map_register(RegId(0)).unwrap();
        assert_eq!(mapped, RegId(0));

        let mapped2 = mapper.map_register(RegId(0)).unwrap();
        assert_eq!(mapped, mapped2); // Should return the same mapping
    }

    #[test]
    fn test_register_allocation() {
        let register_set = create_test_register_set();
        let mut allocator = RegisterAllocator::new(register_set);

        let reg1 = allocator.allocate(RegisterClass::GeneralPurpose).unwrap();
        let reg2 = allocator.allocate(RegisterClass::GeneralPurpose).unwrap();

        assert_ne!(reg1, reg2);

        allocator.free(reg1).unwrap();
        let reg3 = allocator.allocate(RegisterClass::GeneralPurpose).unwrap();
        assert_eq!(reg1, reg3);
    }

    #[test]
    fn test_register_error_to_vm_error_conversion() {
        use vm_core::VmError;

        // Test InvalidRegister conversion
        let err = RegisterError::InvalidRegister(RegId(99));
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test RegisterNotFound conversion
        let err = RegisterError::RegisterNotFound(RegId(99));
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test RegisterAlreadyAllocated conversion
        let err = RegisterError::RegisterAlreadyAllocated(RegId(5));
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test NoAvailableRegisters conversion
        let err = RegisterError::NoAvailableRegisters(RegisterClass::GeneralPurpose);
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test InvalidMapping conversion
        let err = RegisterError::InvalidMapping(RegId(1), RegId(2));
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test UnsupportedRegisterClass conversion
        let err =
            RegisterError::UnsupportedRegisterClass(RegisterClass::Vector, Architecture::X86_64);
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test RegisterConflict conversion
        let err = RegisterError::RegisterConflict("register already in use".to_string());
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));
    }

    // ========== New Comprehensive Tests for Coverage Enhancement ==========

    #[test]
    fn test_register_info_builder_methods() {
        // Test with_caller_saved
        let info = RegisterInfo::new(
            RegId(0),
            "rax",
            RegisterClass::GeneralPurpose,
            RegisterType::Integer { width: 64 },
        )
        .with_caller_saved();
        assert!(info.caller_saved);
        assert!(!info.callee_saved);

        // Test with_callee_saved
        let info = RegisterInfo::new(
            RegId(1),
            "rbx",
            RegisterClass::GeneralPurpose,
            RegisterType::Integer { width: 64 },
        )
        .with_callee_saved();
        assert!(info.callee_saved);
        assert!(!info.caller_saved);

        // Test with_volatile
        let info = RegisterInfo::new(
            RegId(2),
            "rcx",
            RegisterClass::GeneralPurpose,
            RegisterType::Integer { width: 64 },
        )
        .with_volatile();
        assert!(info.volatile);

        // Test with_reserved
        let info = RegisterInfo::new(
            RegId(3),
            "rsp",
            RegisterClass::GeneralPurpose,
            RegisterType::Integer { width: 64 },
        )
        .with_reserved();
        assert!(info.reserved);

        // Test with_alias
        let info = RegisterInfo::new(
            RegId(4),
            "r4",
            RegisterClass::GeneralPurpose,
            RegisterType::Integer { width: 64 },
        )
        .with_alias(RegId(5));
        assert_eq!(info.aliases, vec![RegId(5)]);

        // Test with_overlapping
        let info = RegisterInfo::new(
            RegId(6),
            "r6",
            RegisterClass::GeneralPurpose,
            RegisterType::Integer { width: 64 },
        )
        .with_overlapping(RegId(7));
        assert_eq!(info.overlapping, vec![RegId(7)]);
    }

    #[test]
    fn test_register_set_all_classes() {
        let mut set = RegisterSet::new(Architecture::ARM64);

        // Add registers of all classes
        set.add_register(
            RegisterInfo::new(
                RegId(0),
                "x0",
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            )
            .with_caller_saved(),
        );

        set.add_register(
            RegisterInfo::new(
                RegId(1),
                "d0",
                RegisterClass::FloatingPoint,
                RegisterType::Float { width: 64 },
            )
            .with_caller_saved(),
        );

        set.add_register(
            RegisterInfo::new(
                RegId(2),
                "v0",
                RegisterClass::Vector,
                RegisterType::Vector {
                    width: 128,
                    lanes: 4,
                },
            )
            .with_caller_saved(),
        );

        set.add_register(
            RegisterInfo::new(
                RegId(3),
                "pc",
                RegisterClass::Special,
                RegisterType::Special,
            )
            .with_reserved(),
        );

        set.add_register(
            RegisterInfo::new(
                RegId(4),
                "cpsr",
                RegisterClass::Control,
                RegisterType::Special,
            )
            .with_reserved(),
        );

        set.add_register(
            RegisterInfo::new(
                RegId(5),
                "nzcv",
                RegisterClass::Status,
                RegisterType::Special,
            )
            .with_reserved(),
        );

        // Verify all registers are added
        assert_eq!(set.general_purpose.len(), 1);
        assert_eq!(set.floating_point.len(), 1);
        assert_eq!(set.vector.len(), 1);
        assert_eq!(set.special.len(), 1);
        assert_eq!(set.control.len(), 1);
        assert_eq!(set.status.len(), 1);
    }

    #[test]
    fn test_get_register_by_name() {
        let mut set = RegisterSet::new(Architecture::X86_64);

        set.add_register(
            RegisterInfo::new(
                RegId(0),
                "rax",
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            )
            .with_caller_saved(),
        );

        set.add_register(
            RegisterInfo::new(
                RegId(1),
                "rbx",
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            )
            .with_callee_saved(),
        );

        // Test finding by name
        let rax = set.get_register_by_name("rax");
        assert!(rax.is_some());
        assert_eq!(rax.unwrap().id, RegId(0));

        let rbx = set.get_register_by_name("rbx");
        assert!(rbx.is_some());
        assert_eq!(rbx.unwrap().id, RegId(1));

        // Test not found
        let rcx = set.get_register_by_name("rcx");
        assert!(rcx.is_none());
    }

    #[test]
    fn test_get_registers_by_class() {
        let mut set = RegisterSet::new(Architecture::X86_64);

        for i in 0..8 {
            set.add_register(
                RegisterInfo::new(
                    RegId(i),
                    format!("r{}", i),
                    RegisterClass::GeneralPurpose,
                    RegisterType::Integer { width: 64 },
                )
                .with_caller_saved(),
            );
        }

        for i in 8..16 {
            set.add_register(
                RegisterInfo::new(
                    RegId(i),
                    format!("r{}", i),
                    RegisterClass::FloatingPoint,
                    RegisterType::Float { width: 64 },
                )
                .with_caller_saved(),
            );
        }

        // Test getting GP registers
        let gp_regs = set.get_registers_by_class(RegisterClass::GeneralPurpose);
        assert_eq!(gp_regs.len(), 8);

        // Test getting FP registers
        let fp_regs = set.get_registers_by_class(RegisterClass::FloatingPoint);
        assert_eq!(fp_regs.len(), 8);

        // Test empty class
        let vec_regs = set.get_registers_by_class(RegisterClass::Vector);
        assert_eq!(vec_regs.len(), 0);
    }

    #[test]
    fn test_get_available_registers_filters_reserved() {
        let mut set = RegisterSet::new(Architecture::X86_64);

        set.add_register(
            RegisterInfo::new(
                RegId(0),
                "r0",
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            )
            .with_caller_saved(),
        );

        set.add_register(
            RegisterInfo::new(
                RegId(1),
                "r1",
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            )
            .with_reserved(),
        );

        set.add_register(
            RegisterInfo::new(
                RegId(2),
                "r2",
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            )
            .with_caller_saved(),
        );

        let available = set.get_available_registers(RegisterClass::GeneralPurpose);
        assert_eq!(available.len(), 2); // r0 and r2, not r1 (reserved)
    }

    #[test]
    fn test_with_virtual_registers() {
        let set = RegisterSet::with_virtual_registers(Architecture::RISCV64, 10);

        // Should have 10 virtual registers
        let vregs = set.get_registers_by_class(RegisterClass::GeneralPurpose);
        assert_eq!(vregs.len(), 10);

        // Verify naming pattern
        for (i, reg) in vregs.iter().enumerate() {
            assert_eq!(reg.name, format!("v{}", i));
            assert_eq!(reg.id, RegId(i as u16));
        }
    }

    #[test]
    fn test_register_mapper_reservation() {
        let source_set = create_test_register_set();
        let target_set = create_test_register_set();
        let mut mapper = RegisterMapper::new(source_set, target_set, MappingStrategy::Direct);

        // Reserve a register
        mapper.reserve_register(RegId(5)).unwrap();

        // Try to allocate the reserved register
        let result = mapper.map_register(RegId(5));
        // Should succeed but map to a different register since 5 is reserved
        assert!(result.is_ok());
        assert_ne!(result.unwrap(), RegId(5));

        // Release reservation
        mapper.release_reservation(RegId(5));

        // Now should be able to allocate
        let result = mapper.map_register(RegId(5));
        assert!(result.is_ok());
    }

    #[test]
    fn test_register_mapper_reserve_already_allocated() {
        let source_set = create_test_register_set();
        let target_set = create_test_register_set();
        let mut mapper = RegisterMapper::new(source_set, target_set, MappingStrategy::Direct);

        // Allocate a register
        let _ = mapper.map_register(RegId(0)).unwrap();

        // Try to reserve it - should fail
        let result = mapper.reserve_register(RegId(0));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegisterError::RegisterAlreadyAllocated(_)
        ));
    }

    #[test]
    fn test_register_mapper_free_register() {
        let source_set = create_test_register_set();
        let target_set = create_test_register_set();
        let mut mapper = RegisterMapper::new(source_set, target_set, MappingStrategy::Direct);

        // Allocate a register
        let mapped = mapper.map_register(RegId(0)).unwrap();
        assert!(mapper.is_allocated(mapped));

        // Free it
        mapper.free_register(mapped).unwrap();
        assert!(!mapper.is_allocated(mapped));

        // Try to free again - should fail
        let result = mapper.free_register(mapped);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegisterError::InvalidRegister(_)
        ));
    }

    #[test]
    fn test_register_mapper_free_all() {
        let source_set = create_test_register_set();
        let target_set = create_test_register_set();
        let mut mapper = RegisterMapper::new(source_set, target_set, MappingStrategy::Direct);

        // Allocate multiple registers
        let r1 = mapper.map_register(RegId(0)).unwrap();
        let r2 = mapper.map_register(RegId(1)).unwrap();
        let r3 = mapper.map_register(RegId(2)).unwrap();

        assert!(mapper.is_allocated(r1));
        assert!(mapper.is_allocated(r2));
        assert!(mapper.is_allocated(r3));

        // Free all
        mapper.free_all();

        assert!(!mapper.is_allocated(r1));
        assert!(!mapper.is_allocated(r2));
        assert!(!mapper.is_allocated(r3));
    }

    #[test]
    fn test_register_mapper_reverse_map() {
        let source_set = create_test_register_set();
        let target_set = create_test_register_set();
        let mut mapper = RegisterMapper::new(source_set, target_set, MappingStrategy::Direct);

        // Map a register
        let source_reg = RegId(5);
        let target_reg = mapper.map_register(source_reg).unwrap();

        // Reverse map
        let reversed = mapper.reverse_map(target_reg);
        assert_eq!(reversed, Some(source_reg));

        // Try reverse mapping non-existent register
        let non_existent = mapper.reverse_map(RegId(99));
        assert_eq!(non_existent, None);
    }

    #[test]
    fn test_register_mapper_get_stats() {
        let source_set = create_test_register_set();
        let target_set = create_test_register_set();
        let mut mapper = RegisterMapper::new(source_set, target_set, MappingStrategy::Direct);

        // Allocate some registers
        let _ = mapper.map_register(RegId(0)).unwrap();
        let _ = mapper.map_register(RegId(1)).unwrap();
        mapper.reserve_register(RegId(2)).unwrap();

        // Get stats
        let stats = mapper.get_stats();
        assert_eq!(stats.total_mappings, 2);
        assert_eq!(stats.allocated_registers, 2);
        assert_eq!(stats.reserved_registers, 1);
        assert_eq!(stats.strategy, MappingStrategy::Direct);
    }

    #[test]
    fn test_register_mapper_map_nonexistent_register() {
        let source_set = create_test_register_set();
        let target_set = create_test_register_set();
        let mut mapper = RegisterMapper::new(source_set, target_set, MappingStrategy::Direct);

        // Try to map a register that doesn't exist
        let result = mapper.map_register(RegId(999));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegisterError::RegisterNotFound(_)
        ));
    }

    #[test]
    fn test_register_allocator_spill_mechanism() {
        let mut set = RegisterSet::new(Architecture::X86_64);

        // Add only one register
        set.add_register(
            RegisterInfo::new(
                RegId(0),
                "r0",
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            )
            .with_caller_saved(),
        );

        let mut allocator = RegisterAllocator::new(set);

        // Allocate the only register
        let reg1 = allocator.allocate(RegisterClass::GeneralPurpose).unwrap();

        // Try to allocate another - should trigger spill and succeed
        let reg2 = allocator.allocate(RegisterClass::GeneralPurpose);
        assert!(reg2.is_ok());

        // The spilled register should be the same as the first one
        assert_eq!(reg1, reg2.unwrap());
    }

    #[test]
    fn test_register_allocator_unsupported_class() {
        let set = RegisterSet::new(Architecture::X86_64);
        let mut allocator = RegisterAllocator::new(set);

        // Try to allocate from an unsupported class (no free list initialized)
        // The allocator should create a free list for the class if registers exist
        // But if no registers exist, it should return an error

        // Let's test with System registers which are likely to be empty
        let result = allocator.allocate(RegisterClass::System);
        // Since we have no system registers, this should fail
        assert!(result.is_err());
    }

    #[test]
    fn test_register_allocator_free_nonexistent() {
        let set = create_test_register_set();
        let mut allocator = RegisterAllocator::new(set);

        // Try to free a register that was never allocated
        let result = allocator.free(RegId(999));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegisterError::InvalidRegister(_)
        ));
    }

    #[test]
    fn test_register_allocator_mark_used() {
        let set = create_test_register_set();
        let mut allocator = RegisterAllocator::new(set);

        // Allocate a register
        let reg = allocator.allocate(RegisterClass::GeneralPurpose).unwrap();

        // Mark as used
        allocator.mark_used(reg, 12345);

        // Free it
        allocator.free(reg).unwrap();

        // Mark used again after reallocation
        let reg2 = allocator.allocate(RegisterClass::GeneralPurpose).unwrap();
        allocator.mark_used(reg2, 67890);
    }

    #[test]
    fn test_register_allocator_set_spill_cost() {
        let set = create_test_register_set();
        let mut allocator = RegisterAllocator::new(set);

        // Allocate registers
        let reg1 = allocator.allocate(RegisterClass::GeneralPurpose).unwrap();
        let reg2 = allocator.allocate(RegisterClass::GeneralPurpose).unwrap();

        // Set spill costs
        allocator.set_spill_cost(reg1, 100.0);
        allocator.set_spill_cost(reg2, 1.0);
    }

    #[test]
    fn test_register_allocator_spill() {
        let mut set = RegisterSet::new(Architecture::X86_64);

        // Add only one register
        set.add_register(
            RegisterInfo::new(
                RegId(0),
                "r0",
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            )
            .with_caller_saved(),
        );

        let mut allocator = RegisterAllocator::new(set);

        // Allocate the only register
        let _ = allocator.allocate(RegisterClass::GeneralPurpose).unwrap();

        // Try to allocate another - should trigger spill
        let result = allocator.allocate(RegisterClass::GeneralPurpose);

        // Should succeed after spilling
        assert!(result.is_ok());
    }

    #[test]
    fn test_mapping_strategy_windowed() {
        let strategy = MappingStrategy::Windowed {
            window_size: 8,
            window_count: 4,
        };

        assert!(matches!(
            strategy,
            MappingStrategy::Windowed {
                window_size: 8,
                window_count: 4
            }
        ));
    }

    #[test]
    fn test_mapping_strategy_stack_based() {
        let strategy = MappingStrategy::StackBased { stack_size: 16 };

        assert!(matches!(
            strategy,
            MappingStrategy::StackBased { stack_size: 16 }
        ));
    }

    #[test]
    fn test_register_type_default() {
        let reg_type: RegisterType = Default::default();
        assert!(matches!(reg_type, RegisterType::Integer { width: 64 }));
    }

    #[test]
    fn test_register_error_partial_eq() {
        let err1 = RegisterError::InvalidRegister(RegId(5));
        let err2 = RegisterError::InvalidRegister(RegId(5));
        let err3 = RegisterError::InvalidRegister(RegId(6));

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_register_mapper_strategies() {
        let source_set = create_test_register_set();
        let target_set = create_test_register_set();

        // Test Direct strategy
        let mapper1 = RegisterMapper::new(
            source_set.clone(),
            target_set.clone(),
            MappingStrategy::Direct,
        );
        assert_eq!(mapper1.mapping_strategy, MappingStrategy::Direct);

        // Test Optimized strategy
        let mapper2 = RegisterMapper::new(
            source_set.clone(),
            target_set.clone(),
            MappingStrategy::Optimized,
        );
        assert_eq!(mapper2.mapping_strategy, MappingStrategy::Optimized);

        // Test Virtual strategy
        let mapper3 = RegisterMapper::new(
            source_set.clone(),
            target_set.clone(),
            MappingStrategy::Virtual,
        );
        assert_eq!(mapper3.mapping_strategy, MappingStrategy::Virtual);

        // Test Custom strategy
        let mapper4 = RegisterMapper::new(source_set, target_set, MappingStrategy::Custom);
        assert_eq!(mapper4.mapping_strategy, MappingStrategy::Custom);
    }

    #[test]
    fn test_register_set_multiple_architectures() {
        // Test X86_64
        let x86_set = RegisterSet::new(Architecture::X86_64);
        assert_eq!(x86_set.architecture, Architecture::X86_64);

        // Test ARM64
        let arm_set = RegisterSet::new(Architecture::ARM64);
        assert_eq!(arm_set.architecture, Architecture::ARM64);

        // Test RISC-V
        let riscv_set = RegisterSet::new(Architecture::RISCV64);
        assert_eq!(riscv_set.architecture, Architecture::RISCV64);
    }

    #[test]
    fn test_register_compatibility() {
        let source_set = create_test_register_set();
        let target_set = create_test_register_set();
        let mapper = RegisterMapper::new(source_set, target_set, MappingStrategy::Direct);

        // Test compatible registers (both Integer 64-bit)
        let source_info = RegisterInfo::new(
            RegId(0),
            "r0",
            RegisterClass::GeneralPurpose,
            RegisterType::Integer { width: 64 },
        );

        let target_info_64 = RegisterInfo::new(
            RegId(0),
            "r0",
            RegisterClass::GeneralPurpose,
            RegisterType::Integer { width: 64 },
        );

        let target_info_32 = RegisterInfo::new(
            RegId(1),
            "r1",
            RegisterClass::GeneralPurpose,
            RegisterType::Integer { width: 32 },
        );

        assert!(mapper.is_compatible(&source_info, &target_info_64));
        assert!(!mapper.is_compatible(&source_info, &target_info_32));
    }

    #[test]
    fn test_register_info_clone() {
        let info1 = RegisterInfo::new(
            RegId(0),
            "rax",
            RegisterClass::GeneralPurpose,
            RegisterType::Integer { width: 64 },
        )
        .with_caller_saved()
        .with_alias(RegId(1));

        let info2 = info1.clone();

        assert_eq!(info1.id, info2.id);
        assert_eq!(info1.name, info2.name);
        assert_eq!(info1.class, info2.class);
        assert_eq!(info1.caller_saved, info2.caller_saved);
        assert_eq!(info1.aliases, info2.aliases);
    }

    #[test]
    fn test_register_class_copy() {
        let class1 = RegisterClass::GeneralPurpose;
        let class2 = class1;

        assert_eq!(class1, class2);
    }

    #[test]
    fn test_register_type_copy() {
        let type1 = RegisterType::Integer { width: 64 };
        let type2 = type1;

        assert_eq!(type1, type2);
    }
}
