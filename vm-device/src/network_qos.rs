use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use vm_core::VmResult;

/// Network Quality of Service (QoS) implementation
///
/// This module provides comprehensive QoS functionality including:
/// - Traffic shaping and policing
/// - Queue management and scheduling
/// - Priority-based packet handling
/// - Bandwidth allocation and limiting
/// - Congestion control
/// - QoS statistics and monitoring
///
/// QoS configuration
#[derive(Debug, Clone)]
pub struct NetworkQosConfig {
    /// Maximum bandwidth (Mbps)
    pub max_bandwidth: u32,
    /// Guaranteed bandwidth (Mbps)
    pub guaranteed_bandwidth: u32,
    /// Burst size (bytes)
    pub burst_size: u32,
    /// Peak rate (Mbps)
    pub peak_rate: u32,
    /// Average rate (Mbps)
    pub average_rate: u32,
    /// Number of traffic classes
    pub num_traffic_classes: u8,
    /// Queue configuration per traffic class
    pub queue_configs: Vec<TrafficClassConfig>,
    /// Scheduler type
    pub scheduler_type: QosSchedulerType,
    /// Congestion control algorithm
    pub congestion_control: CongestionControlType,
    /// QoS monitoring enabled
    pub monitoring_enabled: bool,
}

impl Default for NetworkQosConfig {
    fn default() -> Self {
        Self {
            max_bandwidth: 10000,       // 10 Gbps
            guaranteed_bandwidth: 1000, // 1 Gbps
            burst_size: 65536,          // 64 KB
            peak_rate: 10000,           // 10 Gbps
            average_rate: 5000,         // 5 Gbps
            num_traffic_classes: 8,
            queue_configs: (0..8).map(TrafficClassConfig::default_for_tc).collect(),
            scheduler_type: QosSchedulerType::WeightedFairQueueing,
            congestion_control: CongestionControlType::Red,
            monitoring_enabled: true,
        }
    }
}

/// Traffic class configuration
#[derive(Debug, Clone)]
pub struct TrafficClassConfig {
    /// Traffic class ID
    pub tc_id: u8,
    /// Traffic class priority (0-7, higher is more important)
    pub priority: u8,
    /// Minimum guaranteed bandwidth (Mbps)
    pub min_bandwidth: u32,
    /// Maximum allowed bandwidth (Mbps)
    pub max_bandwidth: u32,
    /// Queue weight (for WFQ scheduler)
    pub weight: u16,
    /// Queue depth (packets)
    pub queue_depth: u16,
    /// Drop policy
    pub drop_policy: DropPolicy,
    /// Traffic shaping enabled
    pub shaping_enabled: bool,
    /// Policing enabled
    pub policing_enabled: bool,
}

impl TrafficClassConfig {
    /// Create default configuration for traffic class
    pub fn default_for_tc(tc_id: u8) -> Self {
        Self {
            tc_id,
            priority: 7 - tc_id, // Higher TC gets higher priority
            min_bandwidth: match tc_id {
                0 => 1000, // Highest priority gets guaranteed bandwidth
                1 => 500,
                2 => 200,
                3 => 100,
                _ => 0,
            },
            max_bandwidth: match tc_id {
                0 => 10000,
                1 => 5000,
                2 => 2000,
                3 => 1000,
                _ => 500,
            },
            weight: match tc_id {
                0 => 100,
                1 => 50,
                2 => 25,
                3 => 12,
                _ => 5,
            },
            queue_depth: match tc_id {
                0 => 1024,
                1 => 512,
                2 => 256,
                3 => 128,
                _ => 64,
            },
            drop_policy: DropPolicy::TailDrop,
            shaping_enabled: tc_id < 4,
            policing_enabled: tc_id < 4,
        }
    }
}

/// QoS scheduler type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QosSchedulerType {
    /// First In First Out
    Fifo,
    /// Priority Queuing
    PriorityQueuing,
    /// Weighted Fair Queueing
    WeightedFairQueueing,
    /// Deficit Round Robin
    DeficitRoundRobin,
    /// Hierarchical Fair Service Curve
    HierarchicalFairServiceCurve,
}

/// Congestion control type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CongestionControlType {
    /// No congestion control
    None,
    /// Random Early Detection
    Red,
    /// Weighted Random Early Detection
    Wred,
    /// Explicit Congestion Notification
    Ecn,
    /// Tail Drop
    TailDrop,
}

/// Drop policy
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropPolicy {
    /// Drop from tail
    TailDrop,
    /// Drop from head
    HeadDrop,
    /// Random drop
    RandomDrop,
}

/// QoS packet metadata
#[derive(Debug, Clone)]
pub struct QosPacket {
    /// Packet data
    pub data: Vec<u8>,
    /// Packet size (bytes)
    pub size: u16,
    /// Traffic class
    pub traffic_class: u8,
    /// Priority
    pub priority: u8,
    /// Timestamp
    pub timestamp: Instant,
    /// Packet ID
    pub packet_id: u64,
    /// Source address
    pub src_addr: [u8; 6],
    /// Destination address
    pub dst_addr: [u8; 6],
    /// VLAN ID
    pub vlan_id: u16,
    /// DSCP value
    pub dscp: u8,
}

impl QosPacket {
    /// Create a new QoS packet
    pub fn new(data: Vec<u8>, traffic_class: u8, priority: u8) -> Self {
        Self {
            size: data.len() as u16,
            data,
            traffic_class,
            priority,
            timestamp: Instant::now(),
            packet_id: 0,
            src_addr: [0; 6],
            dst_addr: [0; 6],
            vlan_id: 0,
            dscp: 0,
        }
    }

    /// Set packet ID
    pub fn with_packet_id(mut self, packet_id: u64) -> Self {
        self.packet_id = packet_id;
        self
    }

    /// Set source address
    pub fn with_src_addr(mut self, src_addr: [u8; 6]) -> Self {
        self.src_addr = src_addr;
        self
    }

    /// Set destination address
    pub fn with_dst_addr(mut self, dst_addr: [u8; 6]) -> Self {
        self.dst_addr = dst_addr;
        self
    }

    /// Set VLAN ID
    pub fn with_vlan_id(mut self, vlan_id: u16) -> Self {
        self.vlan_id = vlan_id;
        self
    }

    /// Set DSCP
    pub fn with_dscp(mut self, dscp: u8) -> Self {
        self.dscp = dscp;
        self
    }
}

/// QoS queue
pub struct QosQueue {
    /// Queue configuration
    config: TrafficClassConfig,
    /// Packet queue
    packets: Arc<Mutex<VecDeque<QosPacket>>>,
    /// Queue statistics
    stats: Arc<Mutex<QosQueueStats>>,
    /// Current queue depth
    current_depth: AtomicU16,
    /// Queue is enabled
    enabled: AtomicBool,
    /// Last dequeue time
    last_dequeue_time: Arc<Mutex<Instant>>,
    /// Deficit counter (for DRR scheduler)
    deficit_counter: AtomicU32,
    /// Quantum value (for DRR scheduler)
    quantum: u32,
}

/// QoS queue statistics
#[derive(Debug, Default)]
pub struct QosQueueStats {
    /// Total packets enqueued
    pub packets_enqueued: AtomicU64,
    /// Total packets dequeued
    pub packets_dequeued: AtomicU64,
    /// Total bytes enqueued
    pub bytes_enqueued: AtomicU64,
    /// Total bytes dequeued
    pub bytes_dequeued: AtomicU64,
    /// Packets dropped
    pub packets_dropped: AtomicU64,
    /// Bytes dropped
    pub bytes_dropped: AtomicU64,
    /// Queue overflows
    pub queue_overflows: AtomicU64,
    /// Average queue depth
    pub avg_queue_depth: AtomicU32,
    /// Maximum queue depth
    pub max_queue_depth: AtomicU16,
    /// Queue utilization percentage
    pub utilization: AtomicU32,
}

impl Clone for QosQueueStats {
    fn clone(&self) -> Self {
        Self {
            packets_enqueued: AtomicU64::new(self.packets_enqueued.load(Ordering::Relaxed)),
            packets_dequeued: AtomicU64::new(self.packets_dequeued.load(Ordering::Relaxed)),
            bytes_enqueued: AtomicU64::new(self.bytes_enqueued.load(Ordering::Relaxed)),
            bytes_dequeued: AtomicU64::new(self.bytes_dequeued.load(Ordering::Relaxed)),
            packets_dropped: AtomicU64::new(self.packets_dropped.load(Ordering::Relaxed)),
            bytes_dropped: AtomicU64::new(self.bytes_dropped.load(Ordering::Relaxed)),
            queue_overflows: AtomicU64::new(self.queue_overflows.load(Ordering::Relaxed)),
            avg_queue_depth: AtomicU32::new(self.avg_queue_depth.load(Ordering::Relaxed)),
            max_queue_depth: AtomicU16::new(self.max_queue_depth.load(Ordering::Relaxed)),
            utilization: AtomicU32::new(self.utilization.load(Ordering::Relaxed)),
        }
    }
}

impl QosQueue {
    /// Create a new QoS queue
    pub fn new(config: TrafficClassConfig) -> Self {
        Self {
            quantum: config.weight as u32 * 1518, // Default MTU * weight
            config,
            packets: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(Mutex::new(QosQueueStats::default())),
            current_depth: AtomicU16::new(0),
            enabled: AtomicBool::new(true),
            last_dequeue_time: Arc::new(Mutex::new(Instant::now())),
            deficit_counter: AtomicU32::new(0),
        }
    }

    /// Enqueue a packet
    pub fn enqueue(&self, packet: QosPacket) -> VmResult<bool> {
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(false);
        }

        let mut packets = self.packets.lock().unwrap();
        let current_depth = self.current_depth.load(Ordering::Acquire);

        // Check if queue is full
        if current_depth >= self.config.queue_depth {
            // Apply drop policy
            let should_drop = match self.config.drop_policy {
                DropPolicy::TailDrop => true,
                DropPolicy::HeadDrop => {
                    if packets.pop_front().is_some() {
                        self.current_depth.fetch_sub(1, Ordering::Relaxed);
                        false
                    } else {
                        true
                    }
                }
                DropPolicy::RandomDrop => {
                    // Simple random drop with 50% probability
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = DefaultHasher::new();
                    packet.packet_id.hash(&mut hasher);
                    hasher.finish().is_multiple_of(2)
                }
            };

            if should_drop {
                self.update_drop_stats(&packet);
                return Ok(false);
            }
        }

        // Enqueue packet
        packets.push_back(packet.clone());
        self.current_depth.fetch_add(1, Ordering::Relaxed);

        // Update statistics
        let stats = self.stats.lock().unwrap();
        stats.packets_enqueued.fetch_add(1, Ordering::Relaxed);
        stats
            .bytes_enqueued
            .fetch_add(packet.size as u64, Ordering::Relaxed);

        let new_depth = self.current_depth.load(Ordering::Acquire);
        if new_depth > stats.max_queue_depth.load(Ordering::Relaxed) {
            stats.max_queue_depth.store(new_depth, Ordering::Relaxed);
        }

        Ok(true)
    }

    /// Dequeue a packet
    pub fn dequeue(&self) -> VmResult<Option<QosPacket>> {
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(None);
        }

        let mut packets = self.packets.lock().unwrap();

        if let Some(packet) = packets.pop_front() {
            self.current_depth.fetch_sub(1, Ordering::Relaxed);

            // Update statistics
            let stats = self.stats.lock().unwrap();
            stats.packets_dequeued.fetch_add(1, Ordering::Relaxed);
            stats
                .bytes_dequeued
                .fetch_add(packet.size as u64, Ordering::Relaxed);

            // Update last dequeue time
            *self.last_dequeue_time.lock().unwrap() = Instant::now();

            Ok(Some(packet))
        } else {
            Ok(None)
        }
    }

    /// Get current queue depth
    pub fn depth(&self) -> u16 {
        self.current_depth.load(Ordering::Acquire)
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.depth() == 0
    }

    /// Enable/disable queue
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }

    /// Check if queue is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Get queue configuration
    pub fn config(&self) -> &TrafficClassConfig {
        &self.config
    }

    /// Get queue statistics
    pub fn stats(&self) -> QosQueueStats {
        self.stats.lock().unwrap().clone()
    }

    /// Reset queue statistics
    pub fn reset_stats(&self) {
        *self.stats.lock().unwrap() = QosQueueStats::default();
    }

    /// Update drop statistics
    fn update_drop_stats(&self, packet: &QosPacket) {
        let stats = self.stats.lock().unwrap();
        stats.packets_dropped.fetch_add(1, Ordering::Relaxed);
        stats
            .bytes_dropped
            .fetch_add(packet.size as u64, Ordering::Relaxed);
        stats.queue_overflows.fetch_add(1, Ordering::Relaxed);
    }

    /// Get deficit counter (for DRR scheduler)
    pub fn deficit_counter(&self) -> u32 {
        self.deficit_counter.load(Ordering::Acquire)
    }

    /// Add to deficit counter (for DRR scheduler)
    pub fn add_deficit(&self, amount: u32) {
        self.deficit_counter.fetch_add(amount, Ordering::Relaxed);
    }

    /// Subtract from deficit counter (for DRR scheduler)
    pub fn sub_deficit(&self, amount: u32) {
        self.deficit_counter.fetch_sub(amount, Ordering::Relaxed);
    }

    /// Get quantum value (for DRR scheduler)
    pub fn quantum(&self) -> u32 {
        self.quantum
    }
}

/// Traffic shaper
pub struct TrafficShaper {
    /// Configuration
    config: TrafficShaperConfig,
    /// Token bucket
    token_bucket: Arc<Mutex<TokenBucket>>,
    /// Statistics
    stats: Arc<Mutex<TrafficShaperStats>>,
}

/// Traffic shaper configuration
#[derive(Debug, Clone)]
pub struct TrafficShaperConfig {
    /// Committed information rate (bytes per second)
    pub cir: u64,
    /// Committed burst size (bytes)
    pub cbs: u64,
    /// Peak information rate (bytes per second)
    pub pir: u64,
    /// Peak burst size (bytes)
    pub pbs: u64,
    /// Shaper enabled
    pub enabled: bool,
}

impl Default for TrafficShaperConfig {
    fn default() -> Self {
        Self {
            cir: 125_000_000,   // 1 Gbps
            cbs: 1_000_000,     // 1 MB
            pir: 1_250_000_000, // 10 Gbps
            pbs: 10_000_000,    // 10 MB
            enabled: true,
        }
    }
}

/// Token bucket implementation
#[derive(Debug)]
struct TokenBucket {
    /// Current token count
    tokens: u64,
    /// Maximum token count
    max_tokens: u64,
    /// Last update time
    last_update: Instant,
}

/// Traffic shaper statistics
#[derive(Debug, Default)]
pub struct TrafficShaperStats {
    /// Total packets processed
    pub packets_processed: AtomicU64,
    /// Total bytes processed
    pub bytes_processed: AtomicU64,
    /// Packets conformed
    pub packets_conformed: AtomicU64,
    /// Bytes conformed
    pub bytes_conformed: AtomicU64,
    /// Packets exceeded
    pub packets_exceeded: AtomicU64,
    /// Bytes exceeded
    pub bytes_exceeded: AtomicU64,
    /// Packets dropped
    pub packets_dropped: AtomicU64,
    /// Bytes dropped
    pub bytes_dropped: AtomicU64,
}

impl Clone for TrafficShaperStats {
    fn clone(&self) -> Self {
        Self {
            packets_processed: AtomicU64::new(self.packets_processed.load(Ordering::Relaxed)),
            bytes_processed: AtomicU64::new(self.bytes_processed.load(Ordering::Relaxed)),
            packets_conformed: AtomicU64::new(self.packets_conformed.load(Ordering::Relaxed)),
            bytes_conformed: AtomicU64::new(self.bytes_conformed.load(Ordering::Relaxed)),
            packets_exceeded: AtomicU64::new(self.packets_exceeded.load(Ordering::Relaxed)),
            bytes_exceeded: AtomicU64::new(self.bytes_exceeded.load(Ordering::Relaxed)),
            packets_dropped: AtomicU64::new(self.packets_dropped.load(Ordering::Relaxed)),
            bytes_dropped: AtomicU64::new(self.bytes_dropped.load(Ordering::Relaxed)),
        }
    }
}

impl TrafficShaper {
    /// Create a new traffic shaper
    pub fn new(config: TrafficShaperConfig) -> Self {
        Self {
            token_bucket: Arc::new(Mutex::new(TokenBucket {
                tokens: config.cbs,
                max_tokens: config.cbs,
                last_update: Instant::now(),
            })),
            config,
            stats: Arc::new(Mutex::new(TrafficShaperStats::default())),
        }
    }

    /// Process a packet
    pub fn process_packet(&self, packet: &QosPacket) -> VmResult<TrafficShaperResult> {
        if !self.config.enabled {
            return Ok(TrafficShaperResult::Conform);
        }

        let mut token_bucket = self.token_bucket.lock().unwrap();
        let stats = self.stats.lock().unwrap();

        // Refill tokens
        self.refill_tokens(&mut token_bucket);

        let packet_size = packet.size as u64;
        stats.packets_processed.fetch_add(1, Ordering::Relaxed);
        stats
            .bytes_processed
            .fetch_add(packet_size, Ordering::Relaxed);

        // Check if packet conforms to committed rate
        if token_bucket.tokens >= packet_size {
            token_bucket.tokens -= packet_size;
            stats.packets_conformed.fetch_add(1, Ordering::Relaxed);
            stats
                .bytes_conformed
                .fetch_add(packet_size, Ordering::Relaxed);
            Ok(TrafficShaperResult::Conform)
        } else {
            stats.packets_exceeded.fetch_add(1, Ordering::Relaxed);
            stats
                .bytes_exceeded
                .fetch_add(packet_size, Ordering::Relaxed);
            Ok(TrafficShaperResult::Exceed)
        }
    }

    /// Refill tokens in the bucket
    fn refill_tokens(&self, token_bucket: &mut TokenBucket) {
        let now = Instant::now();
        let elapsed = now.duration_since(token_bucket.last_update);
        let elapsed_secs = elapsed.as_secs_f64();

        // Calculate tokens to add based on CIR
        let tokens_to_add = (self.config.cir as f64 * elapsed_secs) as u64;

        token_bucket.tokens = (token_bucket.tokens + tokens_to_add).min(token_bucket.max_tokens);
        token_bucket.last_update = now;
    }

    /// Get shaper statistics
    pub fn stats(&self) -> TrafficShaperStats {
        self.stats.lock().unwrap().clone()
    }

    /// Reset shaper statistics
    pub fn reset_stats(&self) {
        *self.stats.lock().unwrap() = TrafficShaperStats::default();
    }

    /// Get configuration
    pub fn config(&self) -> &TrafficShaperConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: TrafficShaperConfig) {
        // Save cbs value before moving config
        let cbs = config.cbs;
        self.config = config;
        let mut token_bucket = self.token_bucket.lock().unwrap();
        token_bucket.max_tokens = cbs;
        token_bucket.tokens = token_bucket.tokens.min(cbs);
    }
}

/// Traffic shaper result
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrafficShaperResult {
    /// Packet conforms to the rate limit
    Conform,
    /// Packet exceeds the rate limit
    Exceed,
}

/// Network QoS manager
pub struct NetworkQosManager {
    /// QoS configuration
    config: NetworkQosConfig,
    /// QoS queues
    queues: HashMap<u8, Arc<QosQueue>>,
    /// Traffic shapers
    shapers: HashMap<u8, Arc<TrafficShaper>>,
    /// QoS statistics
    stats: Arc<Mutex<NetworkQosStats>>,
    /// Scheduler
    scheduler: Box<dyn QosScheduler>,
    /// Congestion controller
    congestion_controller: Box<dyn CongestionController>,
    /// Packet ID counter
    next_packet_id: AtomicU64,
    /// QoS enabled
    enabled: AtomicBool,
}

/// Network QoS statistics
#[derive(Debug, Default)]
pub struct NetworkQosStats {
    /// Total packets processed
    pub total_packets: AtomicU64,
    /// Total bytes processed
    pub total_bytes: AtomicU64,
    /// Packets dropped
    pub dropped_packets: AtomicU64,
    /// Bytes dropped
    pub dropped_bytes: AtomicU64,
    /// Average latency (microseconds)
    pub avg_latency: AtomicU32,
    /// Maximum latency (microseconds)
    pub max_latency: AtomicU32,
    /// Bandwidth utilization (percentage)
    pub bandwidth_utilization: AtomicU32,
    /// Queue statistics per traffic class
    pub queue_stats: HashMap<u8, QosQueueStats>,
    /// Shaper statistics per traffic class
    pub shaper_stats: HashMap<u8, TrafficShaperStats>,
}

impl Clone for NetworkQosStats {
    fn clone(&self) -> Self {
        Self {
            total_packets: AtomicU64::new(self.total_packets.load(Ordering::Relaxed)),
            total_bytes: AtomicU64::new(self.total_bytes.load(Ordering::Relaxed)),
            dropped_packets: AtomicU64::new(self.dropped_packets.load(Ordering::Relaxed)),
            dropped_bytes: AtomicU64::new(self.dropped_bytes.load(Ordering::Relaxed)),
            avg_latency: AtomicU32::new(self.avg_latency.load(Ordering::Relaxed)),
            max_latency: AtomicU32::new(self.max_latency.load(Ordering::Relaxed)),
            bandwidth_utilization: AtomicU32::new(
                self.bandwidth_utilization.load(Ordering::Relaxed),
            ),
            queue_stats: self.queue_stats.clone(),
            shaper_stats: self.shaper_stats.clone(),
        }
    }
}

impl NetworkQosManager {
    /// Create a new network QoS manager
    pub fn new(config: NetworkQosConfig) -> Self {
        let mut queues = HashMap::new();
        let mut shapers = HashMap::new();

        // Create queues and shapers for each traffic class
        for tc_config in &config.queue_configs {
            let queue = Arc::new(QosQueue::new(tc_config.clone()));
            queues.insert(tc_config.tc_id, queue);

            let shaper_config = TrafficShaperConfig {
                cir: (tc_config.min_bandwidth as u64 * 1_000_000) / 8, // Convert Mbps to bytes per second
                cbs: config.burst_size as u64,
                pir: (tc_config.max_bandwidth as u64 * 1_000_000) / 8,
                pbs: (config.burst_size as u64) * 2,
                enabled: tc_config.shaping_enabled,
            };
            let shaper = Arc::new(TrafficShaper::new(shaper_config));
            shapers.insert(tc_config.tc_id, shaper);
        }

        // Create scheduler based on configuration
        let scheduler: Box<dyn QosScheduler> = match config.scheduler_type {
            QosSchedulerType::Fifo => Box::new(FifoScheduler::new()),
            QosSchedulerType::PriorityQueuing => Box::new(PriorityScheduler::new()),
            QosSchedulerType::WeightedFairQueueing => Box::new(WfqScheduler::new()),
            QosSchedulerType::DeficitRoundRobin => Box::new(DrrScheduler::new()),
            QosSchedulerType::HierarchicalFairServiceCurve => Box::new(HfscScheduler::new()),
        };

        // Create congestion controller based on configuration
        let congestion_controller: Box<dyn CongestionController> = match config.congestion_control {
            CongestionControlType::None => Box::new(NoCongestionControl::new()),
            CongestionControlType::Red => Box::new(RedController::new()),
            CongestionControlType::Wred => Box::new(WredController::new()),
            CongestionControlType::Ecn => Box::new(EcnController::new()),
            CongestionControlType::TailDrop => Box::new(TailDropController::new()),
        };

        Self {
            config,
            queues,
            shapers,
            stats: Arc::new(Mutex::new(NetworkQosStats::default())),
            scheduler,
            congestion_controller,
            next_packet_id: AtomicU64::new(1),
            enabled: AtomicBool::new(true),
        }
    }

    /// Enqueue a packet
    pub fn enqueue_packet(&self, mut packet: QosPacket) -> VmResult<bool> {
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(false);
        }

        // Assign packet ID
        packet.packet_id = self.next_packet_id.fetch_add(1, Ordering::Relaxed);

        // Get queue for traffic class
        let queue = self.queues.get(&packet.traffic_class).ok_or_else(|| {
            vm_core::error::VmError::Core(vm_core::error::CoreError::InvalidParameter {
                name: "traffic_class".to_string(),
                value: packet.traffic_class.to_string(),
                message: format!("Invalid traffic class: {}", packet.traffic_class),
            })
        })?;

        // Apply traffic shaping if enabled
        if let Some(shaper) = self.shapers.get(&packet.traffic_class) {
            match shaper.process_packet(&packet)? {
                TrafficShaperResult::Conform => {
                    // Packet conforms, continue processing
                }
                TrafficShaperResult::Exceed => {
                    // Packet exceeds rate limit, drop it
                    self.update_drop_stats(&packet);
                    return Ok(false);
                }
            }
        }

        // Apply congestion control
        if self.congestion_controller.should_drop(&packet, queue)? {
            self.update_drop_stats(&packet);
            return Ok(false);
        }

        // Save packet size before enqueuing
        let packet_size = packet.size;

        // Enqueue packet
        let success = queue.enqueue(packet)?;

        // Update statistics
        if success {
            let stats = self.stats.lock().unwrap();
            stats.total_packets.fetch_add(1, Ordering::Relaxed);
            stats
                .total_bytes
                .fetch_add(packet_size as u64, Ordering::Relaxed);
        }

        Ok(success)
    }

    /// Dequeue a packet
    pub fn dequeue_packet(&self) -> VmResult<Option<QosPacket>> {
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(None);
        }

        // Use scheduler to select queue
        let selected_queue = self.scheduler.select_queue(&self.queues)?;

        if let Some((_tc_id, queue)) = selected_queue {
            // Dequeue packet from selected queue
            if let Some(packet) = queue.dequeue()? {
                // Update statistics
                let stats = self.stats.lock().unwrap();
                stats.total_packets.fetch_sub(1, Ordering::Relaxed);
                stats
                    .total_bytes
                    .fetch_sub(packet.size as u64, Ordering::Relaxed);

                // Calculate latency
                let latency = packet.timestamp.elapsed().as_micros() as u32;
                stats.avg_latency.store(
                    (stats.avg_latency.load(Ordering::Relaxed) + latency) / 2,
                    Ordering::Relaxed,
                );
                if latency > stats.max_latency.load(Ordering::Relaxed) {
                    stats.max_latency.store(latency, Ordering::Relaxed);
                }

                return Ok(Some(packet));
            }
        }

        Ok(None)
    }

    /// Get queue for traffic class
    pub fn get_queue(&self, tc_id: u8) -> Option<&Arc<QosQueue>> {
        self.queues.get(&tc_id)
    }

    /// Get shaper for traffic class
    pub fn get_shaper(&self, tc_id: u8) -> Option<&Arc<TrafficShaper>> {
        self.shapers.get(&tc_id)
    }

    /// Get QoS statistics
    pub fn get_stats(&self) -> NetworkQosStats {
        let mut stats = self.stats.lock().unwrap().clone();

        // Update queue statistics
        for (tc_id, queue) in &self.queues {
            stats.queue_stats.insert(*tc_id, queue.stats());
        }

        // Update shaper statistics
        for (tc_id, shaper) in &self.shapers {
            stats.shaper_stats.insert(*tc_id, shaper.stats());
        }

        stats
    }

    /// Reset QoS statistics
    pub fn reset_stats(&self) {
        *self.stats.lock().unwrap() = NetworkQosStats::default();

        for queue in self.queues.values() {
            queue.reset_stats();
        }

        for shaper in self.shapers.values() {
            shaper.reset_stats();
        }
    }

    /// Enable/disable QoS
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }

    /// Check if QoS is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Get configuration
    pub fn config(&self) -> &NetworkQosConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: NetworkQosConfig) -> VmResult<()> {
        self.config = config;

        // Update queue configurations
        for tc_config in &self.config.queue_configs {
            if self.queues.contains_key(&tc_config.tc_id) {
                // Note: In a real implementation, we would update the queue configuration
                // This is a simplified version that doesn't support runtime config changes
            }
        }

        Ok(())
    }

    /// Update drop statistics
    fn update_drop_stats(&self, packet: &QosPacket) {
        let stats = self.stats.lock().unwrap();
        stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
        stats
            .dropped_bytes
            .fetch_add(packet.size as u64, Ordering::Relaxed);
    }
}

/// QoS scheduler trait
pub trait QosScheduler: Send + Sync {
    /// Select a queue for dequeue
    fn select_queue<'a>(
        &self,
        queues: &'a HashMap<u8, Arc<QosQueue>>,
    ) -> VmResult<Option<(u8, &'a Arc<QosQueue>)>>;
}

/// FIFO scheduler implementation
pub struct FifoScheduler;

impl Default for FifoScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl FifoScheduler {
    pub fn new() -> Self {
        Self
    }
}

impl QosScheduler for FifoScheduler {
    fn select_queue<'a>(
        &self,
        queues: &'a HashMap<u8, Arc<QosQueue>>,
    ) -> VmResult<Option<(u8, &'a Arc<QosQueue>)>> {
        // Simple round-robin through queues
        for (tc_id, queue) in queues {
            if queue.is_enabled() && !queue.is_empty() {
                return Ok(Some((*tc_id, queue)));
            }
        }
        Ok(None)
    }
}

/// Priority scheduler implementation
pub struct PriorityScheduler;

impl Default for PriorityScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl PriorityScheduler {
    pub fn new() -> Self {
        Self
    }
}

impl QosScheduler for PriorityScheduler {
    fn select_queue<'a>(
        &self,
        queues: &'a HashMap<u8, Arc<QosQueue>>,
    ) -> VmResult<Option<(u8, &'a Arc<QosQueue>)>> {
        // Select queue with highest priority that has packets
        let mut selected_tc: Option<(u8, &'a Arc<QosQueue>)> = None;

        for (tc_id, queue) in queues {
            if queue.is_enabled() && !queue.is_empty() {
                match selected_tc {
                    None => {
                        selected_tc = Some((*tc_id, queue));
                    }
                    Some((current_tc, _)) => {
                        if queue.config().priority
                            > queues.get(&current_tc).unwrap().config().priority
                        {
                            selected_tc = Some((*tc_id, queue));
                        }
                    }
                }
            }
        }

        Ok(selected_tc)
    }
}

/// Weighted Fair Queueing scheduler implementation
pub struct WfqScheduler;

impl Default for WfqScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl WfqScheduler {
    pub fn new() -> Self {
        Self
    }
}

impl QosScheduler for WfqScheduler {
    fn select_queue<'a>(
        &self,
        queues: &'a HashMap<u8, Arc<QosQueue>>,
    ) -> VmResult<Option<(u8, &'a Arc<QosQueue>)>> {
        // Simplified WFQ - select queue based on weight and queue depth
        let mut selected_tc: Option<(u8, &'a Arc<QosQueue>)> = None;
        let mut max_score = 0.0;

        for (tc_id, queue) in queues {
            if queue.is_enabled() && !queue.is_empty() {
                let weight = queue.config().weight as f32;
                let depth = queue.depth() as f32;
                let score = weight * depth;

                if score > max_score {
                    max_score = score;
                    selected_tc = Some((*tc_id, queue));
                }
            }
        }

        Ok(selected_tc)
    }
}

/// Deficit Round Robin scheduler implementation
pub struct DrrScheduler;

impl Default for DrrScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl DrrScheduler {
    pub fn new() -> Self {
        Self
    }
}

impl QosScheduler for DrrScheduler {
    fn select_queue<'a>(
        &self,
        queues: &'a HashMap<u8, Arc<QosQueue>>,
    ) -> VmResult<Option<(u8, &'a Arc<QosQueue>)>> {
        // DRR implementation
        for (tc_id, queue) in queues {
            if queue.is_enabled() && !queue.is_empty() {
                // Add quantum to deficit counter
                queue.add_deficit(queue.quantum());

                // Check if we have enough deficit to send a packet
                if let Some(packet) = queue.packets.lock().unwrap().front()
                    && queue.deficit_counter() >= packet.size as u32
                {
                    return Ok(Some((*tc_id, queue)));
                }
            }
        }
        Ok(None)
    }
}

/// Hierarchical Fair Service Curve scheduler implementation
pub struct HfscScheduler;

impl Default for HfscScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl HfscScheduler {
    pub fn new() -> Self {
        Self
    }
}

impl QosScheduler for HfscScheduler {
    fn select_queue<'a>(
        &self,
        queues: &'a HashMap<u8, Arc<QosQueue>>,
    ) -> VmResult<Option<(u8, &'a Arc<QosQueue>)>> {
        // Simplified HFSC - similar to WFQ for this implementation
        let mut selected_tc: Option<(u8, &'a Arc<QosQueue>)> = None;
        let mut max_score = 0.0;

        for (tc_id, queue) in queues {
            if queue.is_enabled() && !queue.is_empty() {
                let weight = queue.config().weight as f32;
                let depth = queue.depth() as f32;
                let score = weight * depth;

                if score > max_score {
                    max_score = score;
                    selected_tc = Some((*tc_id, queue));
                }
            }
        }

        Ok(selected_tc)
    }
}

/// Congestion controller trait
pub trait CongestionController: Send + Sync {
    /// Determine if a packet should be dropped
    fn should_drop(&self, packet: &QosPacket, queue: &QosQueue) -> VmResult<bool>;
}

/// No congestion control implementation
pub struct NoCongestionControl;

impl Default for NoCongestionControl {
    fn default() -> Self {
        Self::new()
    }
}

impl NoCongestionControl {
    pub fn new() -> Self {
        Self
    }
}

impl CongestionController for NoCongestionControl {
    fn should_drop(&self, _packet: &QosPacket, _queue: &QosQueue) -> VmResult<bool> {
        Ok(false)
    }
}

/// RED (Random Early Detection) controller implementation
pub struct RedController {
    min_threshold: u16,
    max_threshold: u16,
    max_probability: f32,
}

impl Default for RedController {
    fn default() -> Self {
        Self::new()
    }
}

impl RedController {
    pub fn new() -> Self {
        Self {
            min_threshold: 10,
            max_threshold: 100,
            max_probability: 0.1,
        }
    }
}

impl CongestionController for RedController {
    fn should_drop(&self, _packet: &QosPacket, queue: &QosQueue) -> VmResult<bool> {
        let depth = queue.depth();

        if depth < self.min_threshold {
            Ok(false)
        } else if depth >= self.max_threshold {
            Ok(true)
        } else {
            // Calculate drop probability
            let probability = self.max_probability
                * ((depth - self.min_threshold) as f32
                    / (self.max_threshold - self.min_threshold) as f32);

            // Random drop
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            std::time::SystemTime::now().hash(&mut hasher);
            let random = (hasher.finish() % 100) as f32 / 100.0;

            Ok(random < probability)
        }
    }
}

/// WRED (Weighted Random Early Detection) controller implementation
pub struct WredController {
    red_controllers: HashMap<u8, RedController>,
}

impl Default for WredController {
    fn default() -> Self {
        Self::new()
    }
}

impl WredController {
    pub fn new() -> Self {
        let mut red_controllers = HashMap::new();

        // Create RED controllers for different traffic classes
        for tc in 0..8 {
            let (min_threshold, max_threshold, max_probability) = match tc {
                0 => (5, 50, 0.05),   // High priority - less aggressive
                1 => (10, 100, 0.1),  // Medium-high priority
                2 => (20, 150, 0.15), // Medium priority
                3 => (30, 200, 0.2),  // Medium-low priority
                _ => (40, 250, 0.25), // Low priority - more aggressive
            };

            red_controllers.insert(
                tc,
                RedController {
                    min_threshold,
                    max_threshold,
                    max_probability,
                },
            );
        }

        Self { red_controllers }
    }
}

impl CongestionController for WredController {
    fn should_drop(&self, packet: &QosPacket, queue: &QosQueue) -> VmResult<bool> {
        if let Some(red_controller) = self.red_controllers.get(&packet.traffic_class) {
            red_controller.should_drop(packet, queue)
        } else {
            Ok(false)
        }
    }
}

/// ECN (Explicit Congestion Notification) controller implementation
pub struct EcnController {
    red_controller: RedController,
}

impl Default for EcnController {
    fn default() -> Self {
        Self::new()
    }
}

impl EcnController {
    pub fn new() -> Self {
        Self {
            red_controller: RedController::new(),
        }
    }
}

impl CongestionController for EcnController {
    fn should_drop(&self, packet: &QosPacket, queue: &QosQueue) -> VmResult<bool> {
        // ECN marks packets instead of dropping them when possible
        // For simplicity, we'll use RED logic but assume ECN-capable packets are marked
        self.red_controller.should_drop(packet, queue)
    }
}

/// Tail Drop controller implementation
pub struct TailDropController;

impl Default for TailDropController {
    fn default() -> Self {
        Self::new()
    }
}

impl TailDropController {
    pub fn new() -> Self {
        Self
    }
}

impl CongestionController for TailDropController {
    fn should_drop(&self, _packet: &QosPacket, _queue: &QosQueue) -> VmResult<bool> {
        // Tail drop - only drop when queue is full
        // This is handled at the queue level, so we always return false here
        Ok(false)
    }
}
