//! Common register management for VM cross-architecture translation
//!
//! This module provides unified register management across different architectures,
//! including register mapping, allocation, and lifecycle management.

use std::collections::{HashMap, HashSet};
use thiserror::Error;
use vm_error::{Architecture, RegId};

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
        match class {
            RegisterClass::GeneralPurpose => self.general_purpose.iter().collect(),
            RegisterClass::FloatingPoint => self.floating_point.iter().collect(),
            RegisterClass::Vector => self.vector.iter().collect(),
            RegisterClass::Special => self.special.values().collect(),
            RegisterClass::Control => self.control.iter().collect(),
            RegisterClass::Status => self.status.iter().collect(),
            RegisterClass::System => self.system.iter().collect(),
            RegisterClass::Predicate => self.predicate.iter().collect(),
            RegisterClass::Application => self.application.iter().collect(),
        }
    }

    pub fn get_available_registers(&self, class: RegisterClass) -> Vec<&RegisterInfo> {
        self.get_registers_by_class(class)
            .into_iter()
            .filter(|reg| !reg.reserved)
            .collect()
    }
}

/// Register mapping strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingStrategy {
    /// Direct 1:1 mapping between registers
    Direct,
    /// Windowed mapping (e.g., RISC-V register windows)
    Windowed { window_size: u8, window_count: u8 },
    /// Stack-based mapping (e.g., x86 stack-based floating-point)
    StackBased { stack_size: u8 },
    /// Optimized mapping with register allocation
    Optimized,
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
        let candidates: Vec<RegId> = self
            .allocated
            .iter()
            .filter(|(_, info)| info.class == class)
            .map(|(reg_id, _)| *reg_id)
            .collect();

        if candidates.is_empty() {
            return Err(RegisterError::NoAvailableRegisters(class));
        }

        // Find the register with the lowest spill cost
        let reg_id = candidates
            .iter()
            .min_by(|&&a, &&b| {
                let cost_a = self
                    .allocated
                    .get(&a)
                    .map(|info| info.spill_cost)
                    .unwrap_or(0.0);
                let cost_b = self
                    .allocated
                    .get(&b)
                    .map(|info| info.spill_cost)
                    .unwrap_or(0.0);
                cost_a
                    .partial_cmp(&cost_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
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
                i,
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

        let mapped = mapper.map_register(0).unwrap();
        assert_eq!(mapped, 0);

        let mapped2 = mapper.map_register(0).unwrap();
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
}
