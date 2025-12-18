//! Enhanced event sourcing example
//!
//! This example demonstrates how to use the enhanced event sourcing functionality
//! with persistent storage and snapshot optimization.

use std::sync::Arc;
use vm_core::{
    EnhancedEventSourcingService, EventSourcingServiceBuilder,
    PostgresEventStoreBuilder, FileEventStoreBuilder,
    SnapshotStoreBuilder, EventSourcingConfig,
    DomainEventEnum, VmLifecycleEvent, VmConfig,
    VirtualMachineAggregate,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Enhanced Event Sourcing Example");
    println!("==============================");

    // Example 1: File-based event store with snapshots
    println!("\n1. File-based Event Store with Snapshots");
    println!("-----------------------------------------");
    
    let file_event_store = FileEventStoreBuilder::new()
        .base_dir("./example_events")
        .enable_compression(true)
        .rotation_size(10 * 1024 * 1024) // 10MB
        .build()
        .await?;

    let file_snapshot_store = SnapshotStoreBuilder::new()
        .snapshot_dir("./example_snapshots")
        .snapshot_interval(100) // Create snapshot every 100 events
        .enable_compression(true)
        .build()
        .await?;

    let file_service = EventSourcingServiceBuilder::new()
        .event_store(Arc::new(vm_core::FileEventStoreAdapter::new(file_event_store)))
        .snapshot_store(file_snapshot_store)
        .auto_snapshot(true)
        .max_events_before_snapshot(100)
        .enable_compression(true)
        .build()
        .await?;

    // Create a VM and store some events
    let vm_id = "example_vm_1";
    let config = VmConfig::default();
    
    println!("Creating VM: {}", vm_id);
    let vm_events = vec![
        DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
            vm_id: vm_id.to_string(),
            config: config.clone().into(),
            occurred_at: std::time::SystemTime::now(),
        }),
        DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: vm_id.to_string(),
            occurred_at: std::time::SystemTime::now(),
        }),
    ];

    let stored_count = file_service.store_events(vm_id, vm_events).await?;
    println!("Stored {} events for VM {}", stored_count, vm_id);

    // Load aggregate state
    let aggregate = file_service.load_aggregate(vm_id, config).await?;
    println!("Loaded aggregate state for VM {} (version: {})", vm_id, aggregate.version());

    // Create a snapshot
    let snapshot_metadata = file_service.create_snapshot_for_vm(vm_id).await?;
    println!("Created snapshot version {} for VM {}", 
        snapshot_metadata.snapshot_version, vm_id);

    // Get VM statistics
    let vm_stats = file_service.get_vm_stats(vm_id).await?;
    println!("VM Statistics:");
    println!("  Event count: {}", vm_stats.event_count);
    println!("  Last sequence: {}", vm_stats.last_sequence);
    println!("  Snapshot count: {}", vm_stats.snapshot_count);
    println!("  Latest snapshot version: {}", vm_stats.latest_snapshot_version);

    // Example 2: PostgreSQL event store (if available)
    println!("\n2. PostgreSQL Event Store");
    println!("-------------------------");
    
    match PostgresEventStoreBuilder::new()
        .connection_string("postgresql://localhost/vm_events")
        .enable_compression(true)
        .build()
        .await {
        Ok(pg_event_store) => {
            println!("PostgreSQL event store created successfully");
            
            let pg_service = EventSourcingServiceBuilder::new()
                .event_store(Arc::new(vm_core::PostgresEventStoreAdapter::new(pg_event_store)))
                .auto_snapshot(true)
                .max_events_before_snapshot(500)
                .build()
                .await?;
            
            // Store some events
            let pg_vm_id = "example_vm_2";
            let pg_events = vec![
                DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
                    vm_id: pg_vm_id.to_string(),
                    config: VmConfig::default().into(),
                    occurred_at: std::time::SystemTime::now(),
                }),
            ];
            
            let pg_stored_count = pg_service.store_events(pg_vm_id, pg_events).await?;
            println!("Stored {} events in PostgreSQL", pg_stored_count);
        }
        Err(e) => {
            println!("PostgreSQL not available: {}", e);
            println!("Skipping PostgreSQL example");
        }
    }

    // Example 3: Event replay performance
    println!("\n3. Event Replay Performance");
    println!("----------------------------");
    
    // Create a VM with many events for performance testing
    let perf_vm_id = "performance_test_vm";
    let mut perf_events = Vec::new();
    
    for i in 1..=1000 {
        perf_events.push(DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: perf_vm_id.to_string(),
            occurred_at: std::time::SystemTime::now(),
        }));
    }
    
    let start_time = std::time::Instant::now();
    file_service.store_events(perf_vm_id, perf_events).await?;
    let store_duration = start_time.elapsed();
    
    println!("Stored 1000 events in {:?}", store_duration);
    
    // Test replay performance
    let replay_start = std::time::Instant::now();
    let replayed_events = file_service.replay_events(perf_vm_id, None, None).await?;
    let replay_duration = replay_start.elapsed();
    
    println!("Replayed {} events in {:?}", replayed_events.len(), replay_duration);
    
    // Create a snapshot and test replay from snapshot
    let snapshot_start = std::time::Instant::now();
    file_service.create_snapshot_for_vm(perf_vm_id).await?;
    let snapshot_duration = snapshot_start.elapsed();
    
    println!("Created snapshot in {:?}", snapshot_duration);
    
    // Test replay from snapshot
    let snapshot_replay_start = std::time::Instant::now();
    let _aggregate_from_snapshot = file_service.restore_from_snapshot(perf_vm_id, None).await?;
    let snapshot_replay_duration = snapshot_replay_start.elapsed();
    
    println!("Replayed from snapshot in {:?}", snapshot_replay_duration);
    
    // Example 4: Event store compaction
    println!("\n4. Event Store Compaction");
    println!("--------------------------");
    
    let compact_start = std::time::Instant::now();
    let deleted_count = file_service.compact_event_store(perf_vm_id).await?;
    let compact_duration = compact_start.elapsed();
    
    println!("Compacted event store, deleted {} events in {:?}", 
        deleted_count, compact_duration);

    // Example 5: Global statistics
    println!("\n5. Global Event Sourcing Statistics");
    println!("-----------------------------------");
    
    let global_stats = file_service.get_stats().await;
    println!("Global Statistics:");
    println!("  Total events: {}", global_stats.total_events);
    println!("  Snapshot count: {}", global_stats.snapshot_count);
    println!("  Event store size: {} bytes", global_stats.event_store_size_bytes);
    println!("  Snapshot store size: {} bytes", global_stats.snapshot_store_size_bytes);
    println!("  Average events per snapshot: {:.2}", global_stats.avg_events_per_snapshot);
    println!("  Replay performance: {:.2} events/sec", global_stats.replay_performance_events_per_sec);

    println!("\nEnhanced Event Sourcing Example Complete!");
    Ok(())
}