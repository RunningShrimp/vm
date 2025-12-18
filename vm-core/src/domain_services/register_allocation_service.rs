//! Register allocation domain service
//!
//! This service manages complex business logic of register allocation,
//! including live range analysis, register pressure analysis, and spill decisions.

use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use crate::domain_services::events::{DomainEventBus, DomainEventEnum, OptimizationEvent};
use crate::domain_services::rules::optimization_pipeline_rules::OptimizationPipelineBusinessRule;
use crate::error::VmError;
use crate::VmResult;
use crate::GuestArch;

/// Register allocation domain service
/// 
/// This service encapsulates the business logic for register allocation across
/// different architectures, managing register pressure, live ranges, and spill decisions.
pub struct RegisterAllocationDomainService {
    business_rules: Vec<Box<dyn OptimizationPipelineBusinessRule>>,
    event_bus: Option<Arc<dyn DomainEventBus>>,
    config: RegisterAllocationConfig,
}

impl RegisterAllocationDomainService {
    /// Create a new register allocation domain service
    pub fn new() -> Self {
        let config = RegisterAllocationConfig::default();
        
        Self {
            business_rules: Vec::new(),
            event_bus: None,
            config,
        }
    }
    
    /// Create a new register allocation domain service with custom config
    pub fn with_config(config: RegisterAllocationConfig) -> Self {
        Self {
            business_rules: Vec::new(),
            event_bus: None,
            config,
        }
    }
    
    /// Create a new register allocation domain service with custom rules
    pub fn with_rules(business_rules: Vec<Box<dyn OptimizationPipelineBusinessRule>>) -> Self {
        let config = RegisterAllocationConfig::default();
        
        Self {
            business_rules,
            event_bus: None,
            config,
        }
    }
    
    /// Set the event bus for publishing domain events
    pub fn with_event_bus(mut self, event_bus: Arc<DomainEventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }
    
    /// Analyze live ranges for a set of variables
    pub fn analyze_live_ranges(
        &self,
        variables: &[Variable],
        instructions: &[Instruction],
    ) -> VmResult<LiveRangeAnalysisResult> {
        // Validate input
        if variables.is_empty() {
            return Err(VmError::Core(crate::error::CoreError::InvalidConfig {
                field: "variables".to_string(),
                message: "Variables list cannot be empty".to_string(),
            }));
        }
        
        if instructions.is_empty() {
            return Err(VmError::Core(crate::error::CoreError::InvalidConfig {
                field: "instructions".to_string(),
                message: "Instructions list cannot be empty".to_string(),
            }));
        }
        
        // Perform live range analysis
        let live_ranges = self.compute_live_ranges(variables, instructions)?;
        
        // Build interference graph
        let interference_graph = self.build_interference_graph(&live_ranges)?;
        
        // Calculate register pressure
        let register_pressure = self.calculate_register_pressure_at_points(&live_ranges, instructions)?;
        
        let result = LiveRangeAnalysisResult {
            live_ranges,
            interference_graph,
            register_pressure,
            max_simultaneous_variables: self.find_max_simultaneous_variables(&live_ranges),
        };
        
        // Publish live range analysis event
        self.publish_optimization_event(OptimizationEvent::LiveRangeAnalysisCompleted {
            variable_count: variables.len() as u32,
            instruction_count: instructions.len() as u32,
            max_simultaneous_variables: result.max_simultaneous_variables,
        })?;
        
        Ok(result)
    }
    
    /// Allocate registers for a set of variables
    pub fn allocate_registers(
        &self,
        variables: &[Variable],
        instructions: &[Instruction],
        target_arch: GuestArch,
        allocation_strategy: RegisterAllocationStrategy,
    ) -> VmResult<RegisterAllocationResult> {
        // Validate input
        self.validate_allocation_request(variables, instructions, target_arch)?;
        
        // Analyze live ranges
        let live_range_analysis = self.analyze_live_ranges(variables, instructions)?;
        
        // Get available registers for target architecture
        let available_registers = self.get_available_registers(target_arch)?;
        
        // Perform register allocation based on strategy
        let allocation = match allocation_strategy {
            RegisterAllocationStrategy::LinearScan => {
                self.linear_scan_allocation(&live_range_analysis, &available_registers)?
            }
            RegisterAllocationStrategy::GraphColoring => {
                self.graph_coloring_allocation(&live_range_analysis, &available_registers)?
            }
            RegisterAllocationStrategy::IterativeCoalescing => {
                self.iterative_coalescing_allocation(&live_range_analysis, &available_registers)?
            }
            RegisterAllocationStrategy::PriorityBased => {
                self.priority_based_allocation(&live_range_analysis, &available_registers)?
            }
        };
        
        // Generate spill decisions if needed
        let spill_decisions = self.generate_spill_decisions(&allocation, &available_registers)?;
        
        let result = RegisterAllocationResult {
            allocation,
            spill_decisions,
            allocation_strategy,
            target_arch,
            registers_used: allocation.allocations.len() as u32,
            spills_generated: spill_decisions.len() as u32,
            allocation_quality: self.calculate_allocation_quality(&allocation, &spill_decisions),
        };
        
        // Publish register allocation event
        self.publish_optimization_event(OptimizationEvent::RegisterAllocationCompleted {
            target_arch,
            strategy: format!("{:?}", allocation_strategy),
            registers_used: result.registers_used,
            spills_generated: result.spills_generated,
            allocation_quality: result.allocation_quality,
        })?;
        
        Ok(result)
    }
    
    /// Analyze register pressure
    pub fn analyze_register_pressure(
        &self,
        variables: &[Variable],
        instructions: &[Instruction],
        target_arch: GuestArch,
    ) -> VmResult<RegisterPressureAnalysisResult> {
        // Validate input
        self.validate_allocation_request(variables, instructions, target_arch)?;
        
        // Analyze live ranges
        let live_range_analysis = self.analyze_live_ranges(variables, instructions)?;
        
        // Get available registers for target architecture
        let available_registers = self.get_available_registers(target_arch)?;
        
        // Calculate register pressure at each point
        let pressure_points = self.calculate_register_pressure_at_points(
            &live_range_analysis.live_ranges,
            instructions,
        )?;
        
        // Identify pressure hotspots
        let pressure_hotspots = self.identify_pressure_hotspots(&pressure_points, &available_registers)?;
        
        // Generate pressure reduction recommendations
        let recommendations = self.generate_pressure_reduction_recommendations(
            &pressure_hotspots,
            &available_registers,
        )?;
        
        let result = RegisterPressureAnalysisResult {
            pressure_points,
            pressure_hotspots,
            recommendations,
            max_pressure: pressure_points.iter().map(|p| p.pressure).max().unwrap_or(0),
            average_pressure: if pressure_points.is_empty() { 0.0 } else {
                pressure_points.iter().map(|p| p.pressure).sum::<f32>() / pressure_points.len() as f32
            },
            available_registers: available_registers.len() as u32,
        };
        
        // Publish register pressure analysis event
        self.publish_optimization_event(OptimizationEvent::RegisterPressureAnalysisCompleted {
            target_arch,
            max_pressure: result.max_pressure,
            average_pressure: result.average_pressure,
            available_registers: result.available_registers,
        })?;
        
        Ok(result)
    }
    
    /// Compute live ranges for variables
    fn compute_live_ranges(
        &self,
        variables: &[Variable],
        instructions: &[Instruction],
    ) -> VmResult<Vec<LiveRange>> {
        let mut live_ranges = Vec::new();
        
        for variable in variables {
            let mut start_point = None;
            let mut end_point = None;
            
            // Find first use
            for (i, instruction) in instructions.iter().enumerate() {
                if instruction.uses_variable(&variable.id) {
                    start_point = Some(i as u32);
                    break;
                }
            }
            
            // Find last use
            for (i, instruction) in instructions.iter().enumerate().rev() {
                if instruction.uses_variable(&variable.id) {
                    end_point = Some(i as u32);
                    break;
                }
            }
            
            if let (Some(start), Some(end)) = (start_point, end_point) {
                live_ranges.push(LiveRange {
                    variable_id: variable.id.clone(),
                    start,
                    end,
                    variable_type: variable.var_type.clone(),
                    weight: variable.weight,
                });
            }
        }
        
        Ok(live_ranges)
    }
    
    /// Build interference graph from live ranges
    fn build_interference_graph(&self, live_ranges: &[LiveRange]) -> VmResult<InterferenceGraph> {
        let mut graph = HashMap::new();
        
        // Initialize graph nodes
        for live_range in live_ranges {
            graph.insert(live_range.variable_id.clone(), HashSet::new());
        }
        
        // Build edges between overlapping live ranges
        for (i, range1) in live_ranges.iter().enumerate() {
            for range2 in live_ranges.iter().skip(i + 1) {
                if self.ranges_overlap(range1, range2) {
                    graph.get_mut(&range1.variable_id).unwrap()
                        .insert(range2.variable_id.clone());
                    graph.get_mut(&range2.variable_id).unwrap()
                        .insert(range1.variable_id.clone());
                }
            }
        }
        
        Ok(InterferenceGraph { graph })
    }
    
    /// Check if two live ranges overlap
    fn ranges_overlap(&self, range1: &LiveRange, range2: &LiveRange) -> bool {
        range1.start <= range2.end && range2.start <= range1.end
    }
    
    /// Calculate register pressure at each instruction point
    fn calculate_register_pressure_at_points(
        &self,
        live_ranges: &[LiveRange],
        instructions: &[Instruction],
    ) -> VmResult<Vec<PressurePoint>> {
        let mut pressure_points = Vec::new();
        
        for (i, _) in instructions.iter().enumerate() {
            let point = i as u32;
            let mut live_count = 0;
            
            for live_range in live_ranges {
                if live_range.start <= point && point <= live_range.end {
                    live_count += 1;
                }
            }
            
            pressure_points.push(PressurePoint {
                instruction_point: point,
                pressure: live_count as f32,
            });
        }
        
        Ok(pressure_points)
    }
    
    /// Find maximum number of simultaneous variables
    fn find_max_simultaneous_variables(&self, live_ranges: &[LiveRange]) -> u32 {
        let mut max_simultaneous = 0;
        
        for range1 in live_ranges {
            let mut simultaneous = 1;
            
            for range2 in live_ranges {
                if range1.variable_id != range2.variable_id && self.ranges_overlap(range1, range2) {
                    simultaneous += 1;
                }
            }
            
            max_simultaneous = max_simultaneous.max(simultaneous);
        }
        
        max_simultaneous
    }
    
    /// Get available registers for target architecture
    fn get_available_registers(&self, target_arch: GuestArch) -> VmResult<Vec<PhysicalRegister>> {
        let registers = match target_arch {
            GuestArch::X86_64 => {
                vec![
                    PhysicalRegister { name: "rax".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "rbx".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "rcx".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "rdx".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "rsi".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "rdi".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "r8".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "r9".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "r10".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "r11".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "r12".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "r13".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "r14".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "r15".to_string(), class: RegisterClass::General, caller_saved: false },
                ]
            }
            GuestArch::ARM64 => {
                vec![
                    PhysicalRegister { name: "x0".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x1".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x2".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x3".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x4".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x5".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x6".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x7".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x8".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x9".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x10".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x11".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x12".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x13".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x14".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x15".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x16".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x17".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x18".to_string(), class: RegisterClass::General, caller_saved: true },
                    PhysicalRegister { name: "x19".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "x20".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "x21".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "x22".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "x23".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "x24".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "x25".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "x26".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "x27".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "x28".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "x29".to_string(), class: RegisterClass::General, caller_saved: false },
                    PhysicalRegister { name: "x30".to_string(), class: RegisterClass::General, caller_saved: true },
                ]
            }
            GuestArch::RISCV64 => {
                vec![
                    PhysicalRegister { name: "x0".to_string(), class: RegisterClass::General, caller_saved: false }, // zero
                    PhysicalRegister { name: "x1".to_string(), class: RegisterClass::General, caller_saved: true },  // ra
                    PhysicalRegister { name: "x2".to_string(), class: RegisterClass::General, caller_saved: true },  // sp
                    PhysicalRegister { name: "x3".to_string(), class: RegisterClass::General, caller_saved: true },  // gp
                    PhysicalRegister { name: "x4".to_string(), class: RegisterClass::General, caller_saved: true },  // tp
                    PhysicalRegister { name: "x5".to_string(), class: RegisterClass::General, caller_saved: true },  // t0
                    PhysicalRegister { name: "x6".to_string(), class: RegisterClass::General, caller_saved: true },  // t1
                    PhysicalRegister { name: "x7".to_string(), class: RegisterClass::General, caller_saved: true },  // t2
                    PhysicalRegister { name: "x8".to_string(), class: RegisterClass::General, caller_saved: false }, // s0/fp
                    PhysicalRegister { name: "x9".to_string(), class: RegisterClass::General, caller_saved: false }, // s1
                    PhysicalRegister { name: "x10".to_string(), class: RegisterClass::General, caller_saved: true }, // a0
                    PhysicalRegister { name: "x11".to_string(), class: RegisterClass::General, caller_saved: true }, // a1
                    PhysicalRegister { name: "x12".to_string(), class: RegisterClass::General, caller_saved: true }, // a2
                    PhysicalRegister { name: "x13".to_string(), class: RegisterClass::General, caller_saved: true }, // a3
                    PhysicalRegister { name: "x14".to_string(), class: RegisterClass::General, caller_saved: true }, // a4
                    PhysicalRegister { name: "x15".to_string(), class: RegisterClass::General, caller_saved: true }, // a5
                    PhysicalRegister { name: "x16".to_string(), class: RegisterClass::General, caller_saved: true }, // a6
                    PhysicalRegister { name: "x17".to_string(), class: RegisterClass::General, caller_saved: true }, // a7
                    PhysicalRegister { name: "x18".to_string(), class: RegisterClass::General, caller_saved: true }, // s2
                    PhysicalRegister { name: "x19".to_string(), class: RegisterClass::General, caller_saved: false }, // s3
                    PhysicalRegister { name: "x20".to_string(), class: RegisterClass::General, caller_saved: false }, // s4
                    PhysicalRegister { name: "x21".to_string(), class: RegisterClass::General, caller_saved: false }, // s5
                    PhysicalRegister { name: "x22".to_string(), class: RegisterClass::General, caller_saved: false }, // s6
                    PhysicalRegister { name: "x23".to_string(), class: RegisterClass::General, caller_saved: false }, // s7
                    PhysicalRegister { name: "x24".to_string(), class: RegisterClass::General, caller_saved: false }, // s8
                    PhysicalRegister { name: "x25".to_string(), class: RegisterClass::General, caller_saved: false }, // s9
                    PhysicalRegister { name: "x26".to_string(), class: RegisterClass::General, caller_saved: false }, // s10
                    PhysicalRegister { name: "x27".to_string(), class: RegisterClass::General, caller_saved: false }, // s11
                    PhysicalRegister { name: "x28".to_string(), class: RegisterClass::General, caller_saved: true }, // t3
                    PhysicalRegister { name: "x29".to_string(), class: RegisterClass::General, caller_saved: true }, // t4
                    PhysicalRegister { name: "x30".to_string(), class: RegisterClass::General, caller_saved: true }, // t5
                    PhysicalRegister { name: "x31".to_string(), class: RegisterClass::General, caller_saved: true }, // t6
                ]
            }
        };
        
        Ok(registers)
    }
    
    /// Validate allocation request
    fn validate_allocation_request(
        &self,
        variables: &[Variable],
        instructions: &[Instruction],
        target_arch: GuestArch,
    ) -> VmResult<()> {
        if variables.is_empty() {
            return Err(VmError::Core(crate::error::CoreError::InvalidConfig {
                field: "variables".to_string(),
                message: "Variables list cannot be empty".to_string(),
            }));
        }
        
        if instructions.is_empty() {
            return Err(VmError::Core(crate::error::CoreError::InvalidConfig {
                field: "instructions".to_string(),
                message: "Instructions list cannot be empty".to_string(),
            }));
        }
        
        Ok(())
    }
    
    /// Linear scan register allocation
    fn linear_scan_allocation(
        &self,
        live_range_analysis: &LiveRangeAnalysisResult,
        available_registers: &[PhysicalRegister],
    ) -> VmResult<Allocation> {
        let mut allocations = HashMap::new();
        let mut active_ranges = Vec::new();
        let mut register_pool = available_registers.to_vec();
        
        // Sort live ranges by start point
        let mut sorted_ranges = live_range_analysis.live_ranges.clone();
        sorted_ranges.sort_by(|a, b| a.start.cmp(&b.start));
        
        for live_range in sorted_ranges {
            // Remove expired ranges
            active_ranges.retain(|range| range.end >= live_range.start);
            
            // Check if we have available registers
            if active_ranges.len() < register_pool.len() {
                // Allocate register
                let register_index = active_ranges.len();
                allocations.insert(live_range.variable_id.clone(), register_pool[register_index].clone());
                active_ranges.push(live_range);
            } else {
                // Need to spill - find range with furthest end
                let spill_index = active_ranges.iter()
                    .enumerate()
                    .max_by_key(|(_, range)| range.end)
                    .map(|(index, _)| index)
                    .unwrap();
                
                if active_ranges[spill_index].end > live_range.end {
                    // Spill the active range
                    let spilled_range = active_ranges.remove(spill_index);
                    allocations.remove(&spilled_range.variable_id);
                    
                    // Allocate register to current range
                    let register_index = active_ranges.len();
                    allocations.insert(live_range.variable_id.clone(), register_pool[register_index].clone());
                    active_ranges.push(live_range);
                }
                // Otherwise, current range will be spilled
            }
        }
        
        Ok(Allocation { allocations })
    }
    
    /// Graph coloring register allocation
    fn graph_coloring_allocation(
        &self,
        live_range_analysis: &LiveRangeAnalysisResult,
        available_registers: &[PhysicalRegister],
    ) -> VmResult<Allocation> {
        let mut allocations = HashMap::new();
        let mut colored_nodes = HashSet::new();
        let mut stack = Vec::new();
        let mut graph = live_range_analysis.interference_graph.graph.clone();
        
        // Simplify phase
        while !graph.is_empty() {
            // Find node with degree less than number of registers
            let mut found = false;
            
            for (node, neighbors) in &graph {
                if !colored_nodes.contains(node) && neighbors.len() < available_registers.len() {
                    stack.push(node.clone());
                    colored_nodes.insert(node.clone());
                    found = true;
                    break;
                }
            }
            
            if !found {
                // No node with degree < k, select one for potential spilling
                if let Some((node, _)) = graph.iter().next() {
                    stack.push(node.clone());
                    colored_nodes.insert(node.clone());
                }
            }
        }
        
        // Select phase
        while let Some(node) = stack.pop() {
            // Find available color (register)
            let mut used_registers = HashSet::new();
            
            if let Some(neighbors) = graph.get(&node) {
                for neighbor in neighbors {
                    if let Some(allocated_register) = allocations.get(neighbor) {
                        used_registers.insert(allocated_register.name.clone());
                    }
                }
            }
            
            // Find first available register
            for register in available_registers {
                if !used_registers.contains(&register.name) {
                    allocations.insert(node, register.clone());
                    break;
                }
            }
        }
        
        Ok(Allocation { allocations })
    }
    
    /// Iterative coalescing register allocation
    fn iterative_coalescing_allocation(
        &self,
        live_range_analysis: &LiveRangeAnalysisResult,
        available_registers: &[PhysicalRegister],
    ) -> VmResult<Allocation> {
        // Simplified implementation - in reality this would be much more complex
        // For now, fall back to graph coloring
        self.graph_coloring_allocation(live_range_analysis, available_registers)
    }
    
    /// Priority based register allocation
    fn priority_based_allocation(
        &self,
        live_range_analysis: &LiveRangeAnalysisResult,
        available_registers: &[PhysicalRegister],
    ) -> VmResult<Allocation> {
        let mut allocations = HashMap::new();
        let mut register_usage = HashMap::new();
        
        // Initialize register usage
        for register in available_registers {
            register_usage.insert(register.name.clone(), 0.0);
        }
        
        // Sort live ranges by weight (priority)
        let mut sorted_ranges = live_range_analysis.live_ranges.clone();
        sorted_ranges.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap());
        
        for live_range in sorted_ranges {
            // Find register with lowest usage
            let mut best_register = None;
            let mut min_usage = f32::INFINITY;
            
            for register in available_registers {
                let usage = register_usage.get(&register.name).unwrap();
                if usage < &min_usage {
                    min_usage = *usage;
                    best_register = Some(register);
                }
            }
            
            if let Some(register) = best_register {
                allocations.insert(live_range.variable_id.clone(), register.clone());
                
                // Update register usage based on live range length
                let usage = register_usage.get_mut(&register.name).unwrap();
                *usage += (live_range.end - live_range.start) as f32 * live_range.weight;
            }
        }
        
        Ok(Allocation { allocations })
    }
    
    /// Generate spill decisions
    fn generate_spill_decisions(
        &self,
        allocation: &Allocation,
        available_registers: &[PhysicalRegister],
    ) -> VmResult<Vec<SpillDecision>> {
        let spill_decisions = Vec::new();
        
        // This is a simplified implementation
        // In reality, this would analyze which variables to spill and when
        
        Ok(spill_decisions)
    }
    
    /// Calculate allocation quality
    fn calculate_allocation_quality(
        &self,
        allocation: &Allocation,
        spill_decisions: &[SpillDecision],
    ) -> f32 {
        let registers_used = allocation.allocations.len() as f32;
        let spills_count = spill_decisions.len() as f32;
        
        // Simple quality metric - higher is better
        // Balance between register usage and spills
        let quality = 100.0 - (spills_count * 10.0) - (registers_used * 0.5);
        quality.max(0.0)
    }
    
    /// Identify pressure hotspots
    fn identify_pressure_hotspots(
        &self,
        pressure_points: &[PressurePoint],
        available_registers: &[PhysicalRegister],
    ) -> VmResult<Vec<PressureHotspot>> {
        let mut hotspots = Vec::new();
        let threshold = available_registers.len() as f32 * 0.8; // 80% threshold
        
        for point in pressure_points {
            if point.pressure > threshold {
                hotspots.push(PressureHotspot {
                    instruction_point: point.instruction_point,
                    pressure: point.pressure,
                    threshold,
                    severity: if point.pressure > available_registers.len() as f32 {
                        HotspotSeverity::Critical
                    } else {
                        HotspotSeverity::Warning
                    },
                });
            }
        }
        
        Ok(hotspots)
    }
    
    /// Generate pressure reduction recommendations
    fn generate_pressure_reduction_recommendations(
        &self,
        hotspots: &[PressureHotspot],
        available_registers: &[PhysicalRegister],
    ) -> VmResult<Vec<PressureReductionRecommendation>> {
        let mut recommendations = Vec::new();
        
        for hotspot in hotspots {
            match hotspot.severity {
                HotspotSeverity::Warning => {
                    recommendations.push(PressureReductionRecommendation {
                        instruction_point: hotspot.instruction_point,
                        recommendation_type: RecommendationType::OptimizeLiveRanges,
                        description: "Consider optimizing variable live ranges around this point".to_string(),
                        priority: 2,
                    });
                }
                HotspotSeverity::Critical => {
                    recommendations.push(PressureReductionRecommendation {
                        instruction_point: hotspot.instruction_point,
                        recommendation_type: RecommendationType::IncreaseSpilling,
                        description: "Consider increasing register spilling around this point".to_string(),
                        priority: 1,
                    });
                }
            }
        }
        
        Ok(recommendations)
    }
    
    /// Publish optimization event
    fn publish_optimization_event(&self, event: OptimizationEvent) -> VmResult<()> {
        if let Some(event_bus) = &self.event_bus {
            let domain_event = DomainEventEnum::Optimization(event);
            event_bus.publish(domain_event)?;
        }
        Ok(())
    }
}

impl Default for RegisterAllocationDomainService {
    fn default() -> Self {
        Self::new()
    }
}

/// Register allocation configuration
#[derive(Debug, Clone)]
pub struct RegisterAllocationConfig {
    pub max_spills: u32,
    pub spill_cost_weight: f32,
    pub register_pressure_threshold: f32,
    pub prefer_caller_saved: bool,
}

impl Default for RegisterAllocationConfig {
    fn default() -> Self {
        Self {
            max_spills: 10,
            spill_cost_weight: 1.0,
            register_pressure_threshold: 0.8,
            prefer_caller_saved: true,
        }
    }
}

/// Variable
#[derive(Debug, Clone)]
pub struct Variable {
    pub id: String,
    pub var_type: String,
    pub weight: f32,
}

/// Instruction
#[derive(Debug, Clone)]
pub struct Instruction {
    pub id: u32,
    pub opcode: String,
    pub operands: Vec<String>,
}

impl Instruction {
    /// Check if instruction uses a variable
    pub fn uses_variable(&self, variable_id: &str) -> bool {
        self.operands.iter().any(|op| op.contains(variable_id))
    }
}

/// Live range
#[derive(Debug, Clone)]
pub struct LiveRange {
    pub variable_id: String,
    pub start: u32,
    pub end: u32,
    pub variable_type: String,
    pub weight: f32,
}

/// Interference graph
#[derive(Debug, Clone)]
pub struct InterferenceGraph {
    pub graph: HashMap<String, HashSet<String>>,
}

/// Pressure point
#[derive(Debug, Clone)]
pub struct PressurePoint {
    pub instruction_point: u32,
    pub pressure: f32,
}

/// Live range analysis result
#[derive(Debug, Clone)]
pub struct LiveRangeAnalysisResult {
    pub live_ranges: Vec<LiveRange>,
    pub interference_graph: InterferenceGraph,
    pub register_pressure: Vec<PressurePoint>,
    pub max_simultaneous_variables: u32,
}

/// Physical register
#[derive(Debug, Clone)]
pub struct PhysicalRegister {
    pub name: String,
    pub class: RegisterClass,
    pub caller_saved: bool,
}

/// Register class
#[derive(Debug, Clone, PartialEq)]
pub enum RegisterClass {
    General,
    FloatingPoint,
    Vector,
    Special,
}

/// Register allocation strategy
#[derive(Debug, Clone, PartialEq)]
pub enum RegisterAllocationStrategy {
    LinearScan,
    GraphColoring,
    IterativeCoalescing,
    PriorityBased,
}

/// Allocation
#[derive(Debug, Clone)]
pub struct Allocation {
    pub allocations: HashMap<String, PhysicalRegister>,
}

/// Spill decision
#[derive(Debug, Clone)]
pub struct SpillDecision {
    pub variable_id: String,
    pub spill_point: u32,
    pub spill_cost: f32,
    pub spill_location: SpillLocation,
}

/// Spill location
#[derive(Debug, Clone, PartialEq)]
pub enum SpillLocation {
    Stack,
    Memory,
    RegisterFile,
}

/// Register allocation result
#[derive(Debug, Clone)]
pub struct RegisterAllocationResult {
    pub allocation: Allocation,
    pub spill_decisions: Vec<SpillDecision>,
    pub allocation_strategy: RegisterAllocationStrategy,
    pub target_arch: GuestArch,
    pub registers_used: u32,
    pub spills_generated: u32,
    pub allocation_quality: f32,
}

/// Register pressure analysis result
#[derive(Debug, Clone)]
pub struct RegisterPressureAnalysisResult {
    pub pressure_points: Vec<PressurePoint>,
    pub pressure_hotspots: Vec<PressureHotspot>,
    pub recommendations: Vec<PressureReductionRecommendation>,
    pub max_pressure: f32,
    pub average_pressure: f32,
    pub available_registers: u32,
}

/// Pressure hotspot
#[derive(Debug, Clone)]
pub struct PressureHotspot {
    pub instruction_point: u32,
    pub pressure: f32,
    pub threshold: f32,
    pub severity: HotspotSeverity,
}

/// Hotspot severity
#[derive(Debug, Clone, PartialEq)]
pub enum HotspotSeverity {
    Warning,
    Critical,
}

/// Pressure reduction recommendation
#[derive(Debug, Clone)]
pub struct PressureReductionRecommendation {
    pub instruction_point: u32,
    pub recommendation_type: RecommendationType,
    pub description: String,
    pub priority: u8,
}

/// Recommendation type
#[derive(Debug, Clone, PartialEq)]
pub enum RecommendationType {
    OptimizeLiveRanges,
    IncreaseSpilling,
    RestructureCode,
    UseDifferentRegisterClass,
}

/// Optimization goal
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationGoal {
    MinimizeSpills,
    MinimizeRegisterUsage,
    BalanceLoad,
    PreferCallerSaved,
}

/// Register allocation optimization result
#[derive(Debug, Clone)]
pub struct RegisterAllocationOptimizationResult {
    pub original_allocation: Allocation,
    pub optimized_allocation: Allocation,
    pub original_spills: Vec<SpillDecision>,
    pub optimized_spills: Vec<SpillDecision>,
    pub original_quality: f32,
    pub optimized_quality: f32,
    pub applied_optimizations: Vec<String>,
    pub improvement_percentage: f32,
}

/// Optimization result
#[derive(Debug, Clone)]
struct OptimizationResult {
    allocation: Allocation,
    spills: Vec<SpillDecision>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_register_allocation_service_creation() {
        let service = RegisterAllocationDomainService::new();
        assert_eq!(service.config.max_spills, 10);
    }
    
    #[test]
    fn test_get_available_registers() {
        let service = RegisterAllocationDomainService::new();
        
        let x86_registers = service.get_available_registers(GuestArch::X86_64).unwrap();
        assert!(!x86_registers.is_empty());
        
        let arm_registers = service.get_available_registers(GuestArch::ARM64).unwrap();
        assert!(!arm_registers.is_empty());
        
        let riscv_registers = service.get_available_registers(GuestArch::RISCV64).unwrap();
        assert!(!riscv_registers.is_empty());
    }
    
    #[test]
    fn test_compute_live_ranges() {
        let service = RegisterAllocationDomainService::new();
        
        let variables = vec![
            Variable {
                id: "var1".to_string(),
                var_type: "int".to_string(),
                weight: 1.0,
            },
        ];
        
        let instructions = vec![
            Instruction {
                id: 1,
                opcode: "load".to_string(),
                operands: vec!["var1".to_string()],
            },
            Instruction {
                id: 2,
                opcode: "add".to_string(),
                operands: vec!["var1".to_string(), "const".to_string()],
            },
        ];
        
        let live_ranges = service.compute_live_ranges(&variables, &instructions).unwrap();
        assert_eq!(live_ranges.len(), 1);
        assert_eq!(live_ranges[0].variable_id, "var1");
    }
    
    #[test]
    fn test_ranges_overlap() {
        let service = RegisterAllocationDomainService::new();
        
        let range1 = LiveRange {
            variable_id: "var1".to_string(),
            start: 0,
            end: 5,
            variable_type: "int".to_string(),
            weight: 1.0,
        };
        
        let range2 = LiveRange {
            variable_id: "var2".to_string(),
            start: 3,
            end: 8,
            variable_type: "int".to_string(),
            weight: 1.0,
        };
        
        assert!(service.ranges_overlap(&range1, &range2));
        
        let range3 = LiveRange {
            variable_id: "var3".to_string(),
            start: 6,
            end: 10,
            variable_type: "int".to_string(),
            weight: 1.0,
        };
        
        assert!(service.ranges_overlap(&range2, &range3));
        assert!(!service.ranges_overlap(&range1, &range3));
    }
}