//! Domain Events Module
//!
//! This module contains domain events that are published by domain services
//! to notify other parts of the system about important state changes.
//! This module re-exports the domain events from the main domain_events module
//! and adds additional events specific to domain services.

use std::sync::{Arc, Mutex, MutexGuard};
use std::collections::VecDeque;
use std::time::SystemTime;

/// Memory access type for page table operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessType {
    /// Read access
    Read,
    /// Write access
    Write,
    /// Execute access
    Execute,
    /// Atomic operation
    Atomic,
}

/// Domain Event trait
///
/// All domain events must implement this trait to be compatible with the event bus.
pub trait DomainEvent: Send + Sync {
    /// Get the event type identifier
    fn event_type(&self) -> &'static str;

    /// Get when the event occurred
    fn occurred_at(&self) -> SystemTime;
}

/// Translation events for cross-architecture translation
#[derive(Debug, Clone)]
pub enum TranslationEvent {
    /// Translation strategy was selected
    StrategySelected {
        source_arch: String,
        target_arch: String,
        strategy: String,
        occurred_at: SystemTime,
    },
    /// Architecture compatibility was validated
    CompatibilityValidated {
        source_arch: String,
        target_arch: String,
        compatibility_level: String,
        occurred_at: SystemTime,
    },
    /// Translation was planned
    TranslationPlanned {
        source_arch: String,
        target_arch: String,
        block_count: usize,
        occurred_at: SystemTime,
    },
    /// Instruction encoding was validated
    InstructionEncodingValidated {
        instruction: String,
        is_valid: bool,
        occurred_at: SystemTime,
    },
    /// Register mapping was completed
    RegisterMappingCompleted {
        function_name: String,
        mappings_count: usize,
        occurred_at: SystemTime,
    },
    /// Pipeline orchestration was completed
    PipelineOrchestrationCompleted {
        pipeline_stages: usize,
        occurred_at: SystemTime,
    },
}

impl DomainEvent for TranslationEvent {
    fn event_type(&self) -> &'static str {
        match self {
            TranslationEvent::StrategySelected { .. } => "translation.strategy_selected",
            TranslationEvent::CompatibilityValidated { .. } => "translation.compatibility_validated",
            TranslationEvent::TranslationPlanned { .. } => "translation.translation_planned",
            TranslationEvent::InstructionEncodingValidated { .. } => "translation.instruction_encoding_validated",
            TranslationEvent::RegisterMappingCompleted { .. } => "translation.register_mapping_completed",
            TranslationEvent::PipelineOrchestrationCompleted { .. } => "translation.pipeline_orchestration_completed",
        }
    }

    fn occurred_at(&self) -> SystemTime {
        match self {
            TranslationEvent::StrategySelected { occurred_at, .. } => *occurred_at,
            TranslationEvent::CompatibilityValidated { occurred_at, .. } => *occurred_at,
            TranslationEvent::TranslationPlanned { occurred_at, .. } => *occurred_at,
            TranslationEvent::InstructionEncodingValidated { occurred_at, .. } => *occurred_at,
            TranslationEvent::RegisterMappingCompleted { occurred_at, .. } => *occurred_at,
            TranslationEvent::PipelineOrchestrationCompleted { occurred_at, .. } => *occurred_at,
        }
    }
}

/// Optimization pipeline events
#[derive(Debug, Clone)]
pub enum OptimizationEvent {
    /// Optimization pipeline configuration was created
    PipelineConfigCreated {
        source_arch: String,
        target_arch: String,
        optimization_level: u8,
        stages_count: usize,
        occurred_at: SystemTime,
    },
    /// Optimization stage was completed
    StageCompleted {
        stage_name: String,
        execution_time_ms: u64,
        memory_usage_mb: f32,
        success: bool,
        occurred_at: SystemTime,
    },
    /// Optimization pipeline was completed
    PipelineCompleted {
        success: bool,
        total_time_ms: u64,
        stages_completed: usize,
        peak_memory_usage_mb: f32,
        occurred_at: SystemTime,
    },
    /// Hotspots were detected
    HotspotsDetected {
        count: usize,
        threshold: u64,
        occurred_at: SystemTime,
    },
    /// Optimization strategy was selected
    StrategySelected {
        strategy: String,
        hotspot_count: usize,
        resource_utilization: crate::domain_services::adaptive_optimization_service::ResourceUtilization,
        occurred_at: SystemTime,
    },
    /// Resource constraint violation detected
    ResourceConstraintViolation {
        violated_resources: Vec<String>,
        occurred_at: SystemTime,
    },
    /// Resource was allocated
    ResourceAllocated {
        resource_type: String,
        requested_amount: u64,
        allocated_amount: u64,
        success: bool,
        occurred_at: SystemTime,
    },
    /// Resource was released
    ResourceReleased {
        resource_type: String,
        released_amount: u64,
        allocation_id: String,
        occurred_at: SystemTime,
    },
    /// Performance threshold was updated
    PerformanceThresholdUpdated {
        resource_type: String,
        new_min_performance: f64,
        new_max_latency: std::time::Duration,
        occurred_at: SystemTime,
    },
    /// Cache hit occurred
    CacheHit {
        tier: String,
        key: u64,
        size: usize,
        occurred_at: SystemTime,
    },
    /// Cache miss occurred
    CacheMiss {
        key: u64,
        occurred_at: SystemTime,
    },
    /// Cache entry was put
    CachePut {
        tier: String,
        key: u64,
        size: usize,
        occurred_at: SystemTime,
    },
    /// Cache entry was evicted
    CacheEviction {
        tier: String,
        key: u64,
        size: usize,
        occurred_at: SystemTime,
    },
    /// Cache entry was promoted
    CachePromotion {
        from_tier: String,
        to_tier: String,
        key: u64,
        occurred_at: SystemTime,
    },
    /// Cache was resized
    CacheResized {
        tier: String,
        old_capacity: usize,
        new_capacity: usize,
        occurred_at: SystemTime,
    },
    /// Cache prefetch was performed
    CachePrefetch {
        base_key: u64,
        prefetched_keys: Vec<u64>,
        pattern: String,
        confidence: f64,
        occurred_at: SystemTime,
    },
    /// Target optimization was completed
    TargetOptimizationCompleted {
        target_arch: String,
        optimization_level: String,
        performance_improvement: f64,
        size_change: f64,
        optimizations_applied: usize,
        occurred_at: SystemTime,
    },
    /// Optimization effectiveness was monitored
    OptimizationEffectivenessMonitored {
        target_arch: String,
        overall_effectiveness: f64,
        roi: f64,
        occurred_at: SystemTime,
    },
    /// Performance bottleneck analysis was completed
    PerformanceBottleneckAnalysisCompleted {
        target_arch: String,
        bottlenecks_found: usize,
        overall_impact_score: f64,
        occurred_at: SystemTime,
    },
    /// Optimization recommendations were generated
    OptimizationRecommendationsGenerated {
        target_arch: String,
        recommendations_count: usize,
        total_estimated_improvement: f64,
        occurred_at: SystemTime,
    },
    /// Optimization plan was created
    OptimizationPlanCreated {
        target_arch: String,
        phases_count: usize,
        expected_improvement: f64,
        occurred_at: SystemTime,
    },
    /// Optimization execution was completed
    OptimizationExecutionCompleted {
        target_arch: String,
        success: bool,
        actual_improvement: f64,
        occurred_at: SystemTime,
    },
    /// Register allocation was completed
    RegisterAllocationCompleted {
        function_name: String,
        registers_used: usize,
        spill_count: usize,
        occurred_at: SystemTime,
    },
}

impl DomainEvent for OptimizationEvent {
    fn event_type(&self) -> &'static str {
        match self {
            OptimizationEvent::PipelineConfigCreated { .. } => "optimization.pipeline_config_created",
            OptimizationEvent::StageCompleted { .. } => "optimization.stage_completed",
            OptimizationEvent::PipelineCompleted { .. } => "optimization.pipeline_completed",
            OptimizationEvent::HotspotsDetected { .. } => "optimization.hotspots_detected",
            OptimizationEvent::StrategySelected { .. } => "optimization.strategy_selected",
            OptimizationEvent::ResourceConstraintViolation { .. } => "optimization.resource_constraint_violation",
            OptimizationEvent::ResourceAllocated { .. } => "optimization.resource_allocated",
            OptimizationEvent::ResourceReleased { .. } => "optimization.resource_released",
            OptimizationEvent::PerformanceThresholdUpdated { .. } => "optimization.performance_threshold_updated",
            OptimizationEvent::CacheHit { .. } => "optimization.cache_hit",
            OptimizationEvent::CacheMiss { .. } => "optimization.cache_miss",
            OptimizationEvent::CachePut { .. } => "optimization.cache_put",
            OptimizationEvent::CacheEviction { .. } => "optimization.cache_eviction",
            OptimizationEvent::CachePromotion { .. } => "optimization.cache_promotion",
            OptimizationEvent::CacheResized { .. } => "optimization.cache_resized",
            OptimizationEvent::CachePrefetch { .. } => "optimization.cache_prefetch",
            OptimizationEvent::TargetOptimizationCompleted { .. } => "optimization.target_optimization_completed",
            OptimizationEvent::OptimizationEffectivenessMonitored { .. } => "optimization.effectiveness_monitored",
            OptimizationEvent::PerformanceBottleneckAnalysisCompleted { .. } => "optimization.bottleneck_analysis_completed",
            OptimizationEvent::OptimizationRecommendationsGenerated { .. } => "optimization.recommendations_generated",
            OptimizationEvent::OptimizationPlanCreated { .. } => "optimization.plan_created",
            OptimizationEvent::OptimizationExecutionCompleted { .. } => "optimization.execution_completed",
            OptimizationEvent::RegisterAllocationCompleted { .. } => "optimization.register_allocation_completed",
        }
    }

    fn occurred_at(&self) -> SystemTime {
        match self {
            OptimizationEvent::PipelineConfigCreated { occurred_at, .. } => *occurred_at,
            OptimizationEvent::StageCompleted { occurred_at, .. } => *occurred_at,
            OptimizationEvent::PipelineCompleted { occurred_at, .. } => *occurred_at,
            OptimizationEvent::HotspotsDetected { occurred_at, .. } => *occurred_at,
            OptimizationEvent::StrategySelected { occurred_at, .. } => *occurred_at,
            OptimizationEvent::ResourceConstraintViolation { occurred_at, .. } => *occurred_at,
            OptimizationEvent::ResourceAllocated { occurred_at, .. } => *occurred_at,
            OptimizationEvent::ResourceReleased { occurred_at, .. } => *occurred_at,
            OptimizationEvent::PerformanceThresholdUpdated { occurred_at, .. } => *occurred_at,
            OptimizationEvent::CacheHit { occurred_at, .. } => *occurred_at,
            OptimizationEvent::CacheMiss { occurred_at, .. } => *occurred_at,
            OptimizationEvent::CachePut { occurred_at, .. } => *occurred_at,
            OptimizationEvent::CacheEviction { occurred_at, .. } => *occurred_at,
            OptimizationEvent::CachePromotion { occurred_at, .. } => *occurred_at,
            OptimizationEvent::CacheResized { occurred_at, .. } => *occurred_at,
            OptimizationEvent::CachePrefetch { occurred_at, .. } => *occurred_at,
            OptimizationEvent::TargetOptimizationCompleted { occurred_at, .. } => *occurred_at,
            OptimizationEvent::OptimizationEffectivenessMonitored { occurred_at, .. } => *occurred_at,
            OptimizationEvent::PerformanceBottleneckAnalysisCompleted { occurred_at, .. } => *occurred_at,
            OptimizationEvent::OptimizationRecommendationsGenerated { occurred_at, .. } => *occurred_at,
            OptimizationEvent::OptimizationPlanCreated { occurred_at, .. } => *occurred_at,
            OptimizationEvent::OptimizationExecutionCompleted { occurred_at, .. } => *occurred_at,
            OptimizationEvent::RegisterAllocationCompleted { occurred_at, .. } => *occurred_at,
        }
    }
}

/// TLB events for translation lookaside buffer management
#[derive(Debug, Clone)]
pub enum TlbEvent {
    /// TLB entry was inserted
    EntryInserted {
        level: super::tlb_management_service::TlbLevel,
        va: u64,
        asid: u16,
    },
    /// TLB entry was evicted
    EntryEvicted {
        level: super::tlb_management_service::TlbLevel,
        va: u64,
        asid: u16,
    },
    /// TLB entry was flushed
    EntryFlushed {
        level: super::tlb_management_service::TlbLevel,
        va: u64,
        asid: u16,
    },
    /// All TLB entries were flushed
    FlushAll {
        level: super::tlb_management_service::TlbLevel,
    },
    /// TLB entries were flushed by ASID
    FlushAsid {
        asid: u16,
    },
    /// TLB entries were flushed by range
    FlushRange {
        start_va: u64,
        end_va: u64,
    },
    /// TLB entries were invalidated by physical address
    InvalidatePa {
        pa: u64,
    },
}

impl DomainEvent for TlbEvent {
    fn event_type(&self) -> &'static str {
        match self {
            TlbEvent::EntryInserted { .. } => "tlb.entry_inserted",
            TlbEvent::EntryEvicted { .. } => "tlb.entry_evicted",
            TlbEvent::EntryFlushed { .. } => "tlb.entry_flushed",
            TlbEvent::FlushAll { .. } => "tlb.flush_all",
            TlbEvent::FlushAsid { .. } => "tlb.flush_asid",
            TlbEvent::FlushRange { .. } => "tlb.flush_range",
            TlbEvent::InvalidatePa { .. } => "tlb.invalidate_pa",
        }
    }

    fn occurred_at(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Page table events for page table walking
#[derive(Debug, Clone)]
pub enum PageTableEvent {
    /// Page fault occurred
    PageFault {
        va: u64,
        access_type: AccessType,
    },
    /// Access violation occurred
    AccessViolation {
        va: u64,
        access_type: AccessType,
    },
    /// Invalid entry encountered
    InvalidEntry {
        va: u64,
    },
    /// Cache entry was invalidated
    CacheInvalidated {
        va: u64,
    },
    /// Cache was flushed
    CacheFlushed {
        count: usize,
    },
}

impl DomainEvent for PageTableEvent {
    fn event_type(&self) -> &'static str {
        match self {
            PageTableEvent::PageFault { .. } => "page_table.page_fault",
            PageTableEvent::AccessViolation { .. } => "page_table.access_violation",
            PageTableEvent::InvalidEntry { .. } => "page_table.invalid_entry",
            PageTableEvent::CacheInvalidated { .. } => "page_table.cache_invalidated",
            PageTableEvent::CacheFlushed { .. } => "page_table.cache_flushed",
        }
    }

    fn occurred_at(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Execution events for execution management
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    /// Execution context was created
    ContextCreated {
        id: u64,
        pc: u64,
        priority: super::execution_manager_service::ExecutionPriority,
    },
    /// Execution context was deleted
    ContextDeleted {
        id: u64,
        final_state: super::execution_manager_service::ExecutionState,
        execution_time: std::time::Duration,
        instructions_executed: u64,
    },
    /// Execution context was scheduled
    ContextScheduled {
        id: u64,
        priority: super::execution_manager_service::ExecutionPriority,
    },
    /// Execution context was started
    ContextStarted {
        id: u64,
    },
    /// Execution context was completed
    ContextCompleted {
        id: u64,
        execution_time: std::time::Duration,
        instructions_executed: u64,
    },
    /// Execution context failed
    ContextFailed {
        id: u64,
        error: String,
    },
    /// Execution context was paused
    ContextPaused {
        id: u64,
    },
    /// Execution context was resumed
    ContextResumed {
        id: u64,
    },
}

impl DomainEvent for ExecutionEvent {
    fn event_type(&self) -> &'static str {
        match self {
            ExecutionEvent::ContextCreated { .. } => "execution.context_created",
            ExecutionEvent::ContextDeleted { .. } => "execution.context_deleted",
            ExecutionEvent::ContextScheduled { .. } => "execution.context_scheduled",
            ExecutionEvent::ContextStarted { .. } => "execution.context_started",
            ExecutionEvent::ContextCompleted { .. } => "execution.context_completed",
            ExecutionEvent::ContextFailed { .. } => "execution.context_failed",
            ExecutionEvent::ContextPaused { .. } => "execution.context_paused",
            ExecutionEvent::ContextResumed { .. } => "execution.context_resumed",
        }
    }

    fn occurred_at(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// VM lifecycle events
#[derive(Debug, Clone)]
pub enum VmLifecycleEvent {
    /// VM was created
    VmCreated {
        vm_id: String,
        config_snapshot: VmConfigSnapshot,
    },
    /// VM was started
    VmStarted {
        vm_id: String,
    },
    /// VM was paused
    VmPaused {
        vm_id: String,
    },
    /// VM was resumed
    VmResumed {
        vm_id: String,
    },
    /// VM was stopped
    VmStopped {
        vm_id: String,
        reason: String,
    },
    /// VM was destroyed
    VmDestroyed {
        vm_id: String,
    },
    /// VM state transition occurred
    StateTransition {
        vm_id: String,
        from_state: String,
        to_state: String,
    },
    /// VM state changed (alias for StateTransition)
    VmStateChanged {
        vm_id: String,
        to: String,
    },
}

impl DomainEvent for VmLifecycleEvent {
    fn event_type(&self) -> &'static str {
        match self {
            VmLifecycleEvent::VmCreated { .. } => "vm_lifecycle.created",
            VmLifecycleEvent::VmStarted { .. } => "vm_lifecycle.started",
            VmLifecycleEvent::VmPaused { .. } => "vm_lifecycle.paused",
            VmLifecycleEvent::VmResumed { .. } => "vm_lifecycle.resumed",
            VmLifecycleEvent::VmStopped { .. } => "vm_lifecycle.stopped",
            VmLifecycleEvent::VmDestroyed { .. } => "vm_lifecycle.destroyed",
            VmLifecycleEvent::StateTransition { .. } => "vm_lifecycle.state_transition",
            VmLifecycleEvent::VmStateChanged { .. } => "vm_lifecycle.state_changed",
        }
    }

    fn occurred_at(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// VM configuration snapshot
#[derive(Debug, Clone)]
pub struct VmConfigSnapshot {
    /// Guest architecture
    pub guest_arch: String,
    /// Memory size in bytes
    pub memory_size: u64,
    /// VCPU count
    pub vcpu_count: u32,
    /// Execution mode
    pub exec_mode: String,
    /// Kernel path (if any)
    pub kernel_path: Option<String>,
    /// Snapshot timestamp
    pub timestamp: SystemTime,
}

impl From<&crate::VmConfig> for VmConfigSnapshot {
    fn from(config: &crate::VmConfig) -> Self {
        Self {
            guest_arch: config.guest_arch.name().to_string(),
            memory_size: config.memory_size as u64,
            vcpu_count: config.vcpu_count as u32,
            exec_mode: format!("{:?}", config.exec_mode),
            kernel_path: config.kernel_path.clone(),
            timestamp: SystemTime::now(),
        }
    }
}

/// Extended domain event enumeration that includes all domain service specific events
#[derive(Debug, Clone)]
pub enum DomainEventEnum {
    /// Translation events
    Translation(TranslationEvent),
    /// Optimization events
    Optimization(OptimizationEvent),
    /// TLB events
    Tlb(TlbEvent),
    /// Page table events
    PageTable(PageTableEvent),
    /// Execution events
    Execution(ExecutionEvent),
    /// VM lifecycle events
    VmLifecycle(VmLifecycleEvent),
}

impl DomainEvent for DomainEventEnum {
    fn event_type(&self) -> &'static str {
        match self {
            DomainEventEnum::Translation(e) => e.event_type(),
            DomainEventEnum::Optimization(e) => e.event_type(),
            DomainEventEnum::Tlb(e) => e.event_type(),
            DomainEventEnum::PageTable(e) => e.event_type(),
            DomainEventEnum::Execution(e) => e.event_type(),
            DomainEventEnum::VmLifecycle(e) => e.event_type(),
        }
    }

    fn occurred_at(&self) -> SystemTime {
        match self {
            DomainEventEnum::Translation(e) => e.occurred_at(),
            DomainEventEnum::Optimization(e) => e.occurred_at(),
            DomainEventEnum::Tlb(e) => e.occurred_at(),
            DomainEventEnum::PageTable(e) => e.occurred_at(),
            DomainEventEnum::Execution(e) => e.occurred_at(),
            DomainEventEnum::VmLifecycle(e) => e.occurred_at(),
        }
    }
}

impl From<TranslationEvent> for DomainEventEnum {
    fn from(event: TranslationEvent) -> Self {
        DomainEventEnum::Translation(event)
    }
}

impl From<OptimizationEvent> for DomainEventEnum {
    fn from(event: OptimizationEvent) -> Self {
        DomainEventEnum::Optimization(event)
    }
}

impl From<ExecutionEvent> for DomainEventEnum {
    fn from(event: ExecutionEvent) -> Self {
        DomainEventEnum::Execution(event)
    }
}

impl From<PageTableEvent> for DomainEventEnum {
    fn from(event: PageTableEvent) -> Self {
        DomainEventEnum::PageTable(event)
    }
}

impl From<TlbEvent> for DomainEventEnum {
    fn from(event: TlbEvent) -> Self {
        DomainEventEnum::Tlb(event)
    }
}

impl From<VmLifecycleEvent> for DomainEventEnum {
    fn from(event: VmLifecycleEvent) -> Self {
        DomainEventEnum::VmLifecycle(event)
    }
}

impl DomainEventEnum {
    /// Get the event name
    pub fn name(&self) -> &'static str {
        self.event_type()
    }
    
    /// Get the event timestamp as duration since epoch
    pub fn timestamp(&self) -> u64 {
        self.occurred_at()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Trait for domain event handlers
pub trait DomainEventHandler: Send + Sync {
    /// Handle a domain event
    fn handle(&self, event: &DomainEventEnum);
}

/// Domain event bus for publishing and subscribing to events
pub trait DomainEventBus: Send + Sync {
    /// Publish a domain event
    fn publish(&self, event: DomainEventEnum);
    
    /// Subscribe to domain events
    fn subscribe(&self, handler: Arc<dyn DomainEventHandler>);
}

/// In-memory implementation of domain event bus
pub struct InMemoryDomainEventBus {
    handlers: Arc<Mutex<Vec<Arc<dyn DomainEventHandler>>>>,
    events: Arc<Mutex<VecDeque<DomainEventEnum>>>,
    max_events: usize,
}

impl std::fmt::Debug for InMemoryDomainEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryDomainEventBus")
            .field("max_events", &self.max_events)
            .finish()
    }
}

impl InMemoryDomainEventBus {
    /// Create a new in-memory event bus
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(Mutex::new(Vec::new())),
            events: Arc::new(Mutex::new(VecDeque::new())),
            max_events: 1000,
        }
    }

    /// Create a new in-memory event bus with custom max events
    pub fn with_max_events(max_events: usize) -> Self {
        Self {
            handlers: Arc::new(Mutex::new(Vec::new())),
            events: Arc::new(Mutex::new(VecDeque::new())),
            max_events,
        }
    }

    /// Helper: Lock events mutex
    fn lock_events(&self) -> MutexGuard<'_, VecDeque<DomainEventEnum>> {
        self.events.lock().unwrap_or_else(|e| {
            panic!("Mutex lock failed for events: {}", e);
        })
    }

    /// Helper: Lock handlers mutex
    fn lock_handlers(&self) -> MutexGuard<'_, Vec<Arc<dyn DomainEventHandler>>> {
        self.handlers.lock().unwrap_or_else(|e| {
            panic!("Mutex lock failed for handlers: {}", e);
        })
    }

    /// Get all published events
    pub fn get_events(&self) -> Vec<DomainEventEnum> {
        let events = self.lock_events();
        events.iter().cloned().collect()
    }

    /// Clear all events
    pub fn clear_events(&self) {
        let mut events = self.lock_events();
        events.clear();
    }

    /// Get the number of handlers
    pub fn handler_count(&self) -> usize {
        let handlers = self.lock_handlers();
        handlers.len()
    }
}

impl Default for InMemoryDomainEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainEventBus for InMemoryDomainEventBus {
    fn publish(&self, event: DomainEventEnum) {
        // Store the event
        {
            let mut events = self.lock_events();
            events.push_back(event.clone());

            // Remove old events if we exceed the maximum
            while events.len() > self.max_events {
                events.pop_front();
            }
        }

        // Notify all handlers
        let handlers = self.lock_handlers();
        for handler in handlers.iter() {
            handler.handle(&event);
        }
    }

    fn subscribe(&self, handler: Arc<dyn DomainEventHandler>) {
        let mut handlers = self.lock_handlers();
        handlers.push(handler);
    }
}

/// Mock event bus for testing
#[derive(Debug)]
pub struct MockDomainEventBus {
    events: Arc<Mutex<Vec<DomainEventEnum>>>,
}

impl MockDomainEventBus {
    /// Create a new mock event bus
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Helper: Lock events mutex
    fn lock_events(&self) -> MutexGuard<'_, Vec<DomainEventEnum>> {
        self.events.lock().unwrap_or_else(|e| {
            panic!("Mutex lock failed for events: {}", e);
        })
    }

    /// Get all published events
    pub fn published_events(&self) -> Vec<DomainEventEnum> {
        let events = self.lock_events();
        events.clone()
    }

    /// Clear all events
    pub fn clear(&self) {
        let mut events = self.lock_events();
        events.clear();
    }

    /// Get the number of published events
    pub fn event_count(&self) -> usize {
        let events = self.lock_events();
        events.len()
    }
}

impl Default for MockDomainEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainEventBus for MockDomainEventBus {
    fn publish(&self, event: DomainEventEnum) {
        let mut events = self.lock_events();
        events.push(event);
    }

    fn subscribe(&self, _handler: Arc<dyn DomainEventHandler>) {
        // Mock implementation doesn't actually handle subscriptions
    }
}

/// Simple event handler that collects events for testing
#[derive(Debug)]
pub struct CollectingEventHandler {
    events: Arc<Mutex<Vec<DomainEventEnum>>>,
}

impl CollectingEventHandler {
    /// Create a new collecting event handler
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Helper: Lock events mutex
    fn lock_events(&self) -> MutexGuard<'_, Vec<DomainEventEnum>> {
        self.events.lock().unwrap_or_else(|e| {
            panic!("Mutex lock failed for events: {}", e);
        })
    }

    /// Get all collected events
    pub fn get_events(&self) -> Vec<DomainEventEnum> {
        let events = self.lock_events();
        events.clone()
    }

    /// Clear all collected events
    pub fn clear(&self) {
        let mut events = self.lock_events();
        events.clear();
    }

    /// Get the number of collected events
    pub fn event_count(&self) -> usize {
        let events = self.lock_events();
        events.len()
    }
}

impl Default for CollectingEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainEventHandler for CollectingEventHandler {
    fn handle(&self, event: &DomainEventEnum) {
        let mut events = self.lock_events();
        events.push(event.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_event_bus() {
        let event_bus = InMemoryDomainEventBus::new();
        let handler = Arc::new(CollectingEventHandler::new());

        event_bus.subscribe(handler.clone());

        let event = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: "test-vm".to_string(),
        });

        event_bus.publish(event.clone());

        // Check that the event was stored
        let events = event_bus.get_events();
        assert_eq!(events.len(), 1);

        // Check that the handler received the event
        let handler_events = handler.get_events();
        assert_eq!(handler_events.len(), 1);
    }
    
    #[test]
    fn test_mock_event_bus() {
        let event_bus = MockDomainEventBus::new();

        let event = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: "test-vm".to_string(),
        });

        event_bus.publish(event.clone());

        let events = event_bus.published_events();
        assert_eq!(events.len(), 1);

        assert_eq!(event_bus.event_count(), 1);

        event_bus.clear();
        assert_eq!(event_bus.event_count(), 0);
    }
    
    #[test]
    fn test_collecting_event_handler() {
        let handler = CollectingEventHandler::new();

        let event1 = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: "test-vm-1".to_string(),
        });

        let event2 = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStopped {
            vm_id: "test-vm-2".to_string(),
            reason: "test".to_string(),
        });

        handler.handle(&event1);
        handler.handle(&event2);

        let events = handler.get_events();
        assert_eq!(events.len(), 2);

        assert_eq!(handler.event_count(), 2);

        handler.clear();
        assert_eq!(handler.event_count(), 0);
    }
    
    #[test]
    fn test_domain_event_properties() {
        let event = DomainEventEnum::Translation(TranslationEvent::StrategySelected {
            source_arch: "x86_64".to_string(),
            target_arch: "arm64".to_string(),
            strategy: "Optimized".to_string(),
            occurred_at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(12345),
        });
        
        assert_eq!(event.name(), "translation.strategy_selected");
        assert_eq!(event.timestamp(), 12345);
    }
    
    #[test]
    fn test_in_memory_event_bus_max_events() {
        let event_bus = InMemoryDomainEventBus::with_max_events(2);

        let event1 = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: "test-vm-1".to_string(),
        });

        let event2 = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: "test-vm-2".to_string(),
        });

        let event3 = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: "test-vm-3".to_string(),
        });

        event_bus.publish(event1);
        event_bus.publish(event2);
        event_bus.publish(event3);

        let events = event_bus.get_events();
        assert_eq!(events.len(), 2); // Should only keep the last 2 events
    }
}