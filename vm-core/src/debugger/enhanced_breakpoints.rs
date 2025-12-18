//! Enhanced breakpoint management system
//!
//! This module provides comprehensive breakpoint management including
//! execution, read/write, hardware, and conditional breakpoints.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use crate::{GuestAddr, VmError, VmResult};

/// Builder for creating breakpoints
#[derive(Debug, Clone)]
pub struct BreakpointBuilder {
    pub address: Option<GuestAddr>,
    pub breakpoint_type: Option<BreakpointType>,
    pub condition: Option<BreakpointCondition>,
    pub enabled: bool,
}

impl Default for BreakpointBuilder {
    fn default() -> Self {
        Self {
            address: None,
            breakpoint_type: None,
            condition: None,
            enabled: true,
        }
    }
}

/// Condition for triggering a breakpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakpointCondition {
    pub expression: String,
    pub enabled: bool,
}

/// Enhanced breakpoint types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BreakpointType {
    /// Execution breakpoint (software breakpoint)
    Execution,
    /// Read watchpoint (triggers on memory read)
    Read,
    /// Write watchpoint (triggers on memory write)
    Write,
    /// Read/Write watchpoint (triggers on both read and write)
    ReadWrite,
    /// Hardware breakpoint (CPU debug register)
    Hardware,
    /// Data breakpoint (triggers on specific data value)
    Data,
    /// Access breakpoint (triggers on memory access regardless of type)
    Access,
    /// Exception breakpoint (triggers on specific exception)
    Exception,
    /// Function entry breakpoint
    FunctionEntry,
    /// Function exit breakpoint
    FunctionExit,
}

/// Breakpoint condition evaluation
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BreakpointCondition {
    /// Always trigger (no condition)
    Always,
    /// Trigger when register equals value
    RegisterEquals { register: String, value: u64 },
    /// Trigger when register not equals value
    RegisterNotEquals { register: String, value: u64 },
    /// Trigger when memory location equals value
    MemoryEquals { address: GuestAddr, value: u64 },
    /// Trigger when memory location not equals value
    MemoryNotEquals { address: GuestAddr, value: u64 },
    /// Trigger when expression evaluates to true
    Expression { expression: String },
    /// Trigger after N hits
    HitCount { count: u64 },
    /// Trigger on specific thread ID
    ThreadId { thread_id: u32 },
    /// Complex condition (AND of multiple conditions)
    And { conditions: Vec<BreakpointCondition> },
    /// Complex condition (OR of multiple conditions)
    Or { conditions: Vec<BreakpointCondition> },
}

/// Enhanced breakpoint information
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breakpoint {
    /// Unique breakpoint ID
    pub id: u64,
    /// Breakpoint address
    pub address: GuestAddr,
    /// Breakpoint type
    pub breakpoint_type: BreakpointType,
    /// Breakpoint condition
    pub condition: BreakpointCondition,
    /// Whether breakpoint is enabled
    pub enabled: bool,
    /// Number of times breakpoint has been hit
    pub hit_count: u64,
    /// Maximum number of hits before auto-disable (0 = unlimited)
    pub max_hits: u64,
    /// Thread ID this breakpoint applies to (0 = all threads)
    pub thread_id: u32,
    /// Size of memory region for watchpoints
    pub size: usize,
    /// Original instruction bytes (for execution breakpoints)
    pub original_bytes: Vec<u8>,
    /// Breakpoint creation timestamp
    pub created_at: std::time::SystemTime,
    /// Last hit timestamp
    pub last_hit_at: Option<std::time::SystemTime>,
    /// Breakpoint group ID (for managing related breakpoints)
    pub group_id: Option<u64>,
    /// User-defined description
    pub description: String,
    /// Temporary breakpoint (auto-delete on first hit)
    pub temporary: bool,
    /// Architecture-specific data
    pub arch_specific: HashMap<String, String>,
}

impl crate::debugger::breakpoint::Breakpoint {
    /// Create a new execution breakpoint
    pub fn new_execution(
        id: u64,
        address: GuestAddr,
        original_bytes: Vec<u8>,
    ) -> Self {
        Self {
            id,
            address,
            breakpoint_type: BreakpointType::Execution,
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            condition: BreakpointCondition::Always,
            enabled: true,
            hit_count: 0,
            max_hits: 0,
            thread_id: 0,
            size: 0,
            original_bytes,
            created_at: std::time::SystemTime::now(),
            last_hit_at: None,
            group_id: None,
            description: String::new(),
            temporary: false,
            arch_specific: HashMap::new(),
        }
    }

    /// Create a new read watchpoint
    pub fn new_read_watchpoint(
        id: u64,
        address: GuestAddr,
        size: usize,
    ) -> Self {
        Self {
            id,
            address,
            breakpoint_type: BreakpointType::Read,
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            condition: BreakpointCondition::Always,
            enabled: true,
            hit_count: 0,
            max_hits: 0,
            thread_id: 0,
            size,
            original_bytes: Vec::new(),
            created_at: std::time::SystemTime::now(),
            last_hit_at: None,
            group_id: None,
            description: String::new(),
            temporary: false,
            arch_specific: HashMap::new(),
        }
    }

    /// Create a new write watchpoint
    pub fn new_write_watchpoint(
        id: u64,
        address: GuestAddr,
        size: usize,
    ) -> Self {
        Self {
            id,
            address,
            breakpoint_type: BreakpointType::Write,
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            condition: BreakpointCondition::Always,
            enabled: true,
            hit_count: 0,
            max_hits: 0,
            thread_id: 0,
            size,
            original_bytes: Vec::new(),
            created_at: std::time::SystemTime::now(),
            last_hit_at: None,
            group_id: None,
            description: String::new(),
            temporary: false,
            arch_specific: HashMap::new(),
        }
    }

    /// Create a new conditional breakpoint
    pub fn new_conditional(
        id: u64,
        address: GuestAddr,
        condition: BreakpointCondition,
        description: String,
    ) -> Self {
        Self {
            id,
            address,
            breakpoint_type: BreakpointType::Execution,
            condition,
            enabled: true,
            hit_count: 0,
            max_hits: 0,
            thread_id: 0,
            size: 0,
            original_bytes: Vec::new(),
            created_at: std::time::SystemTime::now(),
            last_hit_at: None,
            group_id: None,
            description,
            temporary: false,
            arch_specific: HashMap::new(),
        }
    }

    /// Check if breakpoint should trigger given current state
    pub fn should_trigger(
        &self,
        registers: &HashMap<String, u64>,
        memory: &HashMap<GuestAddr, u64>,
        thread_id: u32,
    ) -> bool {
        if !self.enabled {
            return false;
        }

        // Check thread ID condition
        if self.thread_id != 0 && self.thread_id != thread_id {
            return false;
        }

        // Check max hits condition
        if self.max_hits > 0 && self.hit_count >= self.max_hits {
            return false;
        }

        // Evaluate condition
        match &self.condition {
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::Always => true,
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::RegisterEquals { register, value } => {
                registers.get(register).map_or(0, |&v| v) == *value
            }
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::RegisterNotEquals { register, value } => {
                registers.get(register).map_or(0, |&v| v) != *value
            }
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::MemoryEquals { address, value } => {
                memory.get(address).map_or(0, |&v| v) == *value
            }
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::MemoryNotEquals { address, value } => {
                memory.get(address).map_or(0, |&v| v) != *value
            }
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::Expression { .. } => {
                // In a real implementation, this would evaluate the expression
                // For now, return true as a placeholder
                true
            }
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::HitCount { count } => self.hit_count >= *count,
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::ThreadId { thread_id: tid } => thread_id == *tid,
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::And { conditions } => {
                conditions.iter().all(|c| {
                    // Create a temporary breakpoint with this condition to check
                    let temp_bp = Breakpoint {
                        condition: c.clone(),
                        ..self.clone()
                    };
                    temp_bp.should_trigger(registers, memory, thread_id)
                })
            }
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::Or { conditions } => {
                conditions.iter().any(|c| {
                    // Create a temporary breakpoint with this condition to check
                    let temp_bp = Breakpoint {
                        condition: c.clone(),
                        ..self.clone()
                    };
                    temp_bp.should_trigger(registers, memory, thread_id)
                })
            }
        }
    }

    /// Record a breakpoint hit
    pub fn record_hit(&mut self) {
        self.hit_count += 1;
        self.last_hit_at = Some(std::time::SystemTime::now());
    }

    /// Check if breakpoint is temporary and should be removed
    pub fn should_remove(&self) -> bool {
        self.temporary && self.hit_count > 0
    }
}

/// Breakpoint group for managing related breakpoints
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakpointGroup {
    /// Group ID
    pub id: u64,
    /// Group name
    pub name: String,
    /// Breakpoint IDs in this group
    pub breakpoint_ids: HashSet<u64>,
    /// Group description
    pub description: String,
    /// Whether group is enabled
    pub enabled: bool,
    /// Group creation timestamp
    pub created_at: std::time::SystemTime,
}

impl crate::debugger::breakpoint::BreakpointType {
    /// Create a new breakpoint group
    pub fn new(id: u64, name: String, description: String) -> Self {
        Self {
            id,
            name,
            description,
            breakpoint_ids: HashSet::new(),
            enabled: true,
            created_at: std::time::SystemTime::now(),
        }
    }

    /// Add a breakpoint to the group
    pub fn add_breakpoint(&mut self, breakpoint_id: u64) {
        self.breakpoint_ids.insert(breakpoint_id);
    }

    /// Remove a breakpoint from the group
    pub fn remove_breakpoint(&mut self, breakpoint_id: u64) {
        self.breakpoint_ids.remove(&breakpoint_id);
    }

    /// Check if group contains a breakpoint
    pub fn contains(&self, breakpoint_id: u64) -> bool {
        self.breakpoint_ids.contains(&breakpoint_id)
    }
}

/// Breakpoint manager configuration
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakpointConfig {
    /// Maximum number of breakpoints
    pub max_breakpoints: usize,
    /// Enable hardware breakpoints
    pub enable_hardware_breakpoints: bool,
    /// Enable watchpoints
    pub enable_watchpoints: bool,
    /// Maximum number of watchpoints
    pub max_watchpoints: usize,
    /// Enable conditional breakpoints
    pub enable_conditional_breakpoints: bool,
    /// Enable breakpoint groups
    pub enable_breakpoint_groups: bool,
    /// Enable breakpoint statistics
    pub enable_statistics: bool,
    /// Auto-disable breakpoints after hit count
    pub auto_disable_after_hit_count: bool,
    /// Default hit count limit
    pub default_hit_count_limit: u64,
}

#[cfg(feature = "enhanced-debugging")]
impl Default for BreakpointConfig {
    fn default() -> Self {
        Self {
            max_breakpoints: 1000,
            enable_hardware_breakpoints: true,
            enable_watchpoints: true,
            max_watchpoints: 100,
            enable_conditional_breakpoints: true,
            enable_breakpoint_groups: true,
            enable_statistics: true,
            auto_disable_after_hit_count: false,
            default_hit_count_limit: 0,
        }
    }
}

/// Enhanced breakpoint manager
#[cfg(feature = "enhanced-debugging")]
pub struct BreakpointManager {
    /// Breakpoints by ID
    breakpoints: Arc<RwLock<HashMap<u64, Breakpoint>>>,
    /// Breakpoints by address for fast lookup
    address_index: Arc<RwLock<HashMap<GuestAddr, Vec<u64>>>>,
    /// Breakpoints by type
    type_index: Arc<RwLock<HashMap<BreakpointType, Vec<u64>>>>,
    /// Breakpoint groups
    groups: Arc<RwLock<HashMap<u64, BreakpointGroup>>>,
    /// Next breakpoint ID
    next_id: Arc<RwLock<u64>>,
    /// Next group ID
    next_group_id: Arc<RwLock<u64>>,
}

#[cfg(feature = "enhanced-debugging")]
impl BreakpointManager {
    /// Create a new breakpoint manager
    pub fn new() -> Self {
        Self {
            breakpoints: Arc::new(RwLock::new(HashMap::new())),
            address_index: Arc::new(RwLock::new(HashMap::new())),
            type_index: Arc::new(RwLock::new(HashMap::new())),
            groups: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(1)),
            next_group_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Add a new breakpoint
    pub fn add_breakpoint(&self, mut breakpoint: Breakpoint) -> VmResult<u64> {
        // Assign ID if not set
        if breakpoint.id == 0 {
            let mut next_id = self.next_id.write().unwrap();
            breakpoint.id = *next_id;
            *next_id += 1;
        }

        let id = breakpoint.id;

        // Add to main storage
        let mut breakpoints = self.breakpoints.write().unwrap();
        breakpoints.insert(id, breakpoint.clone());

        // Update address index
        let mut address_index = self.address_index.write().unwrap();
        address_index.entry(breakpoint.address)
            .or_insert_with(Vec::new)
            .push(id);

        // Update type index
        let mut type_index = self.type_index.write().unwrap();
        type_index.entry(breakpoint.breakpoint_type)
            .or_insert_with(Vec::new)
            .push(id);

        // Add to group if specified
        if let Some(group_id) = breakpoint.group_id {
            let mut groups = self.groups.write().unwrap();
            if let Some(group) = groups.get_mut(&group_id) {
                group.add_breakpoint(id);
            }
        }

        Ok(id)
    }

    /// Remove a breakpoint
    pub fn remove_breakpoint(&self, id: u64) -> VmResult<Breakpoint> {
        let mut breakpoints = self.breakpoints.write().unwrap();
        let breakpoint = breakpoints.remove(&id)
            .ok_or_else(|| VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Breakpoint {} not found", id),
                current: "N/A".to_string(),
                expected: format!("Breakpoint {} to exist", id),
            }))?;

        // Update address index
        let mut address_index = self.address_index.write().unwrap();
        if let Some(bp_ids) = address_index.get_mut(&breakpoint.address) {
            bp_ids.retain(|&bp_id| bp_id != id);
            if bp_ids.is_empty() {
                address_index.remove(&breakpoint.address);
            }
        }

        // Update type index
        let mut type_index = self.type_index.write().unwrap();
        if let Some(bp_ids) = type_index.get_mut(&breakpoint.breakpoint_type) {
            bp_ids.retain(|&bp_id| bp_id != id);
            if bp_ids.is_empty() {
                type_index.remove(&breakpoint.breakpoint_type);
            }
        }

        // Remove from group
        if let Some(group_id) = breakpoint.group_id {
            let mut groups = self.groups.write().unwrap();
            if let Some(group) = groups.get_mut(&group_id) {
                group.remove_breakpoint(id);
            }
        }

        Ok(breakpoint)
    }

    /// Get a breakpoint by ID
    pub fn get_breakpoint(&self, id: u64) -> VmResult<Breakpoint> {
        let breakpoints = self.breakpoints.read().unwrap();
        breakpoints.get(&id)
            .cloned()
            .ok_or_else(|| VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Breakpoint {} not found", id),
                current: "N/A".to_string(),
                expected: format!("Breakpoint {} to exist", id),
            }))
    }

    /// Get all breakpoints at a specific address
    pub fn get_breakpoints_at(&self, address: GuestAddr) -> Vec<Breakpoint> {
        let breakpoints = self.breakpoints.read().unwrap();
        let address_index = self.address_index.read().unwrap();
        
        if let Some(bp_ids) = address_index.get(&address) {
            bp_ids.iter()
                .filter_map(|&id| breakpoints.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all breakpoints of a specific type
    pub fn get_breakpoints_by_type(&self, breakpoint_type: BreakpointType) -> Vec<Breakpoint> {
        let breakpoints = self.breakpoints.read().unwrap();
        let type_index = self.type_index.read().unwrap();
        
        if let Some(bp_ids) = type_index.get(&breakpoint_type) {
            bp_ids.iter()
                .filter_map(|&id| breakpoints.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all breakpoints
    pub fn get_all_breakpoints(&self) -> Vec<Breakpoint> {
        let breakpoints = self.breakpoints.read().unwrap();
        breakpoints.values().cloned().collect()
    }

    /// Enable or disable a breakpoint
    pub fn set_breakpoint_enabled(&self, id: u64, enabled: bool) -> VmResult<()> {
        let mut breakpoints = self.breakpoints.write().unwrap();
        if let Some(breakpoint) = breakpoints.get_mut(&id) {
            breakpoint.enabled = enabled;
            Ok(())
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Breakpoint {} not found", id),
                current: "N/A".to_string(),
                expected: format!("Breakpoint {} to exist", id),
            }))
        }
    }

    /// Create a new breakpoint group
    pub fn create_group(&self, name: String, description: String) -> VmResult<u64> {
        let mut next_group_id = self.next_group_id.write().unwrap();
        let group_id = *next_group_id;
        *next_group_id += 1;

        let group = BreakpointGroup::new(group_id, name, description);
        
        let mut groups = self.groups.write().unwrap();
        groups.insert(group_id, group);

        Ok(group_id)
    }

    /// Add a breakpoint to a group
    pub fn add_breakpoint_to_group(&self, breakpoint_id: u64, group_id: u64) -> VmResult<()> {
        // Update breakpoint's group ID
        {
            let mut breakpoints = self.breakpoints.write().unwrap();
            if let Some(breakpoint) = breakpoints.get_mut(&breakpoint_id) {
                breakpoint.group_id = Some(group_id);
            } else {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: format!("Breakpoint {} not found", breakpoint_id),
                    current: "N/A".to_string(),
                    expected: format!("Breakpoint {} to exist", breakpoint_id),
                }));
            }
        }

        // Update group
        let mut groups = self.groups.write().unwrap();
        if let Some(group) = groups.get_mut(&group_id) {
            group.add_breakpoint(breakpoint_id);
        } else {
            return Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Group {} not found", group_id),
                current: "N/A".to_string(),
                expected: format!("Group {} to exist", group_id),
            }));
        }

        Ok(())
    }

    /// Remove a breakpoint from a group
    pub fn remove_breakpoint_from_group(&self, breakpoint_id: u64, group_id: u64) -> VmResult<()> {
        // Update breakpoint's group ID
        {
            let mut breakpoints = self.breakpoints.write().unwrap();
            if let Some(breakpoint) = breakpoints.get_mut(&breakpoint_id) {
                breakpoint.group_id = None;
            } else {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: format!("Breakpoint {} not found", breakpoint_id),
                    current: "N/A".to_string(),
                    expected: format!("Breakpoint {} to exist", breakpoint_id),
                }));
            }
        }

        // Update group
        let mut groups = self.groups.write().unwrap();
        if let Some(group) = groups.get_mut(&group_id) {
            group.remove_breakpoint(breakpoint_id);
        } else {
            return Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Group {} not found", group_id),
                current: "N/A".to_string(),
                expected: format!("Group {} to exist", group_id),
            }));
        }

        Ok(())
    }

    /// Enable or disable a breakpoint group
    pub fn set_group_enabled(&self, group_id: u64, enabled: bool) -> VmResult<()> {
        let mut groups = self.groups.write().unwrap();
        if let Some(group) = groups.get_mut(&group_id) {
            group.enabled = enabled;

            // Enable/disable all breakpoints in the group
            let mut breakpoints = self.breakpoints.write().unwrap();
            for &breakpoint_id in &group.breakpoint_ids {
                if let Some(breakpoint) = breakpoints.get_mut(&breakpoint_id) {
                    breakpoint.enabled = enabled;
                }
            }

            Ok(())
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Group {} not found", group_id),
                current: "N/A".to_string(),
                expected: format!("Group {} to exist", group_id),
            }))
        }
    }

    /// Check for breakpoints that should trigger at the current execution state
    pub fn check_breakpoints(
        &self,
        address: GuestAddr,
        access_type: BreakpointType,
        registers: &HashMap<String, u64>,
        memory: &HashMap<GuestAddr, u64>,
        thread_id: u32,
    ) -> Vec<Breakpoint> {
        let mut triggered_breakpoints = Vec::new();

        // Check execution breakpoints at this address
        for bp in self.get_breakpoints_at(address) {
            if bp.breakpoint_type == BreakpointType::Execution || 
               bp.breakpoint_type == access_type {
                if bp.should_trigger(registers, memory, thread_id) {
                    triggered_breakpoints.push(bp);
                }
            }
        }

        // Check watchpoints for memory access
        if access_type == BreakpointType::Read || 
           access_type == BreakpointType::Write || 
           access_type == BreakpointType::ReadWrite {
            
            let breakpoints = self.get_breakpoints_by_type(access_type);
            for bp in breakpoints {
                // Check if address is within watchpoint range
                if address >= bp.address && address < bp.address + bp.size as u64 {
                    if bp.should_trigger(registers, memory, thread_id) {
                        triggered_breakpoints.push(bp);
                    }
                }
            }
        }

        triggered_breakpoints
    }

    /// Get breakpoint statistics
    pub fn get_statistics(&self) -> BreakpointStatistics {
        let breakpoints = self.breakpoints.read().unwrap();
        let groups = self.groups.read().unwrap();

        let total_breakpoints = breakpoints.len();
        let enabled_breakpoints = breakpoints.values()
            .filter(|bp| bp.enabled)
            .count();
        
        let mut type_counts = HashMap::new();
        for bp in breakpoints.values() {
            *type_counts.entry(bp.breakpoint_type).or_insert(0) += 1;
        }

        let total_groups = groups.len();
        let enabled_groups = groups.values()
            .filter(|g| g.enabled)
            .count();

        BreakpointStatistics {
            total_breakpoints,
            enabled_breakpoints,
            type_counts,
            total_groups,
            enabled_groups,
        }
    }
}

/// Breakpoint statistics
#[cfg(feature = "enhanced-debugging")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakpointStatistics {
    /// Total number of breakpoints
    pub total_breakpoints: usize,
    /// Number of enabled breakpoints
    pub enabled_breakpoints: usize,
    /// Breakpoints by type
    pub type_counts: HashMap<BreakpointType, usize>,
    /// Total number of breakpoint groups
    pub total_groups: usize,
    /// Number of enabled breakpoint groups
    pub enabled_groups: usize,
}

impl Default for crate::debugger::breakpoint::BreakpointManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breakpoint_creation() {
        let bp = Breakpoint::new_execution(
            1,
            0x1000,
            vec![0x90, 0x90], // NOP instructions
        );

        assert_eq!(bp.id, 1);
        assert_eq!(bp.address, 0x1000);
        assert_eq!(bp.breakpoint_type, BreakpointType::Execution);
        assert_eq!(bp.enabled, true);
        assert_eq!(bp.hit_count, 0);
    }

    #[test]
    fn test_breakpoint_conditions() {
        let mut registers = HashMap::new();
        registers.insert("rax".to_string(), 0x1234);
        registers.insert("rbx".to_string(), 0x5678);

        let mut memory = HashMap::new();
        memory.insert(0x2000, 0xABCDEF);

        // Test register condition
        let bp_reg_eq = Breakpoint::new_conditional(
            2,
            0x1000,
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::RegisterEquals {
                register: "rax".to_string(),
                value: 0x1234,
            },
            "Break when rax = 0x1234".to_string(),
        );

        assert!(bp_reg_eq.should_trigger(&registers, &memory, 0));

        // Test memory condition
        let bp_mem_eq = Breakpoint::new_conditional(
            3,
            0x1000,
#[cfg(feature = "enhanced-debugging")]
#[cfg(feature = "enhanced-debugging")]
            BreakpointCondition::MemoryEquals {
                address: 0x2000,
                value: 0xABCDEF,
            },
            "Break when memory[0x2000] = 0xABCDEF".to_string(),
        );

        assert!(bp_mem_eq.should_trigger(&registers, &memory, 0));
    }

    #[test]
    fn test_breakpoint_manager() {
        let manager = BreakpointManager::new();

        // Add breakpoints
        let bp1 = Breakpoint::new_execution(1, 0x1000, vec![0x90]);
        let bp2 = Breakpoint::new_read_watchpoint(2, 0x2000, 4);
        
        let id1 = manager.add_breakpoint(bp1).unwrap();
        let id2 = manager.add_breakpoint(bp2).unwrap();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        // Check breakpoints at address
        let bps_at_1000 = manager.get_breakpoints_at(0x1000);
        assert_eq!(bps_at_1000.len(), 1);
        assert_eq!(bps_at_1000[0].breakpoint_type, BreakpointType::Execution);

        // Check breakpoints by type
        let exec_bps = manager.get_breakpoints_by_type(BreakpointType::Execution);
        assert_eq!(exec_bps.len(), 1);

        let read_bps = manager.get_breakpoints_by_type(BreakpointType::Read);
        assert_eq!(read_bps.len(), 1);

        // Remove breakpoint
        let removed_bp = manager.remove_breakpoint(id1).unwrap();
        assert_eq!(removed_bp.id, 1);

        let bps_at_1000_after = manager.get_breakpoints_at(0x1000);
        assert_eq!(bps_at_1000_after.len(), 0);
    }

    #[test]
    fn test_breakpoint_groups() {
        let manager = BreakpointManager::new();

        // Create group
        let group_id = manager.create_group(
            "Test Group".to_string(),
            "Group for testing".to_string(),
        ).unwrap();

        // Add breakpoints to group
        let bp1 = Breakpoint::new_execution(1, 0x1000, vec![0x90]);
        let bp2 = Breakpoint::new_execution(2, 0x2000, vec![0x90]);
        
        let id1 = manager.add_breakpoint(bp1).unwrap();
        let id2 = manager.add_breakpoint(bp2).unwrap();

        manager.add_breakpoint_to_group(id1, group_id).unwrap();
        manager.add_breakpoint_to_group(id2, group_id).unwrap();

        // Disable group
        manager.set_group_enabled(group_id, false).unwrap();

        // Check breakpoints are disabled
        let bp1_retrieved = manager.get_breakpoint(id1).unwrap();
        let bp2_retrieved = manager.get_breakpoint(id2).unwrap();
        
        assert!(!bp1_retrieved.enabled);
        assert!(!bp2_retrieved.enabled);
    }
}
