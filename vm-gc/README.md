# vm-gc

Advanced garbage collection framework implementing generational GC, concurrent collection, tri-color marking, and integration with VM memory management for optimal performance and pause time control.

## Overview

`vm-gc` provides production-ready garbage collection for the Rust VM project, implementing multiple GC algorithms with configurable tradeoffs between throughput, latency, and pause times.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  vm-gc (Garbage Collection)              │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │  Generational│  │  Concurrent  │  │   Tri-color  │ │
│  │      GC      │  │     GC       │  │   Marking    │ │
│  │              │  │              │  │              │ │
│  │ • Young gen  │  │ • Concurrent │  │ • Write      │ │
│  │ • Old gen   │  │   marking   │  │   barrier   │ │
│  │ • Survivor   │  │ • Concurrent │  │ • Incremental │ │
│  │   spaces    │  │   sweeping   │  │   marking   │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │
│         │                  │                  │         │
│         └──────────────────┼──────────────────┘         │
│                            │                            │
│                  ┌─────────▼──────────┐                 │
│                  │    GC Coordinator  │                 │
│                  │                    │                 │
│                  │ • Algorithm select│                 │
│                  │ • Collection      │                 │
│                  │ • Tuning          │                 │
│                  └─────────┬──────────┘                 │
│                            │                            │
│  ┌─────────────────────────┼─────────────────────────┐ │
│  │  ┌──────────────────────▼─────────────────────┐  │ │
│  │  │         Memory Allocation                   │  │ │
│  │  │  • Nursery generation                     │  │ │
│  │  │  • Object pools                           │  │ │
│  │  │  • Large object space                    │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌─────────────────────────────────────────────┐  │ │
│  │  │            GC Integration                  │  │ │
│  │  │  • VM memory coordination                 │  │ │
│  │  │  • JIT compilation coordination           │  │ │
│  │  │  • Finalization queues                   │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌─────────────────────────────────────────────┐  │ │
│  │  │         Performance Monitoring             │  │ │
│  │  │  • Pause times                              │  │ │
│  │  │  • Throughput                              │  │ │
│  │  │  • Memory usage                             │  │ │
│  │  │  • Collection frequency                    │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## Key Components

### 1. Generational GC (`src/generational.rs`)

**Generation Structure**:
```rust
pub struct GenerationalGC {
    nursery: Vec<ObjPtr>,           // Young generation
    young: Vec<ObjPtr>,            // Young survivors
    old: Vec<ObjPtr>,              // Old generation
    survivor_spaces: [Vec<ObjPtr>; 2],  // Survivor spaces
}
```

**Usage**:
```rust
use vm_gc::generational::GenerationalGC;

let gc = GenerationalGC::new()?;

// Allocate object
let obj = gc.allocate(Object::new(size))?;

// Trigger collection
gc.collect()?;

// Get statistics
let stats = gc.statistics()?;
println!("Collections: {}", stats.collection_count);
println!("Pause time: {:?}", stats.last_pause_time);
```

**Collection Phases**:
1. **Stop-the-world**: Pause mutators
2. **Mark**: Trace live objects
3. **Sweep**: Reclaim dead objects
4. **Compact**: (optional) Defragment memory

### 2. Concurrent GC (`src/concurrent.rs`)

**Concurrent Marking**:
```rust
use vm_gc::concurrent::ConcurrentGC;

let gc = ConcurrentGC::new()?;

// Concurrent marking
gc.start_concurrent_mark()?;

// Continue execution while GC runs
execute_code()?;

// Wait for marking complete
gc.wait_for_marking()?;

// Concurrent sweeping
gc.start_concurrent_sweep()?;
```

**Benefits**:
- Lower pause times (1-5ms vs 10-50ms)
- Better for interactive workloads
- Slightly lower throughput

### 3. Adaptive GC (`src/adaptive.rs`)

**Adaptive Algorithms**:
```rust
use vm_gc::adaptive::{AdaptiveGC, GcStrategy};

let mut gc = AdaptiveGC::new()?;

// GC chooses strategy automatically
match gc.get_current_strategy() {
    GcStrategy::Generational => { /* Standard gen GC */ }
    GcStrategy::Concurrent => { /* Concurrent marking */ }
    GcStrategy::Incremental => { /* Incremental collection */ }
}
```

**Adaptation Criteria**:
- Pause time requirements
- Allocation rate
- Memory pressure
- Mutator activity

### 4. GC Statistics

**Performance Metrics**:
```rust
use vm_gc::statistics::GcStatistics;

let stats = gc.statistics()?;

println!("Collections: {}", stats.total_collections);
println!("Pause time: {:?}", stats.avg_pause_time);
println!("Throughput: {} objs/sec", stats.throughput);
println!("Memory: {} MB", stats.memory_mb);
println!("Reclaimed: {} MB", stats.memory_reclaimed_mb);
```

## GC Algorithms

### Generational GC

**Young Generation Collection** (Minor GC):
- Fast (1-5ms)
- Eden + 2 survivor spaces
- Copying collector
- Only scans young generation

**Old Generation Collection** (Major GC):
- Slower (10-50ms)
- Mark-sweep-compact
- Scans entire heap
- Compacts memory

**Object Promotion**:
```rust
if object.age >= promotion_threshold {
    promote_to_old_generation(object);
}
```

### Concurrent Mark-Sweep

**Phases**:
1. **Concurrent Mark**: Mark while mutators run
2. **Stop-the-world Final Mark**: Quick final mark
3. **Concurrent Sweep**: Sweep while mutators run
4. **Write Barrier**: Track object mutations

**Write Barrier**:
```rust
fn write_barrier(field: &mut ObjPtr, new_value: ObjPtr) {
    // Mark old value if concurrent GC is running
    if gc.is_marking_in_progress() {
        gc.mark_card(field);
    }
    *field = new_value;
}
```

## Usage Examples

### Basic GC Usage

```rust
use vm_gc::Gc;

let gc = Gc::new()?;

// Allocate objects
for i in 0..1000 {
    let obj = gc.allocate(Object::new(1024))?;
    gc.root_object(obj)?;
}

// Trigger collection
gc.collect()?;
```

### GC Configuration

```rust
use vm_gc::{Gc, GcConfig};

let config = GcConfig {
    young_gen_size: 16 * 1024 * 1024,      // 16 MB
    old_gen_size: 128 * 1024 * 1024,       // 128 MB
    promotion_threshold: 10,                // 10 collections
    concurrent_marking: true,
    target_pause_time: Duration::from_millis(5),
};

let gc = Gc::with_config(config)?;
```

### Finalization

```rust
use vm_gc::finalization::{Finalizer, FinalizationQueue};

let finalizer = Finalizer::new(|obj| {
    println!("Finalizing object");
    // Cleanup logic
});

gc.register_finalizer(obj, finalizer)?;
```

## Performance Characteristics

### Generational GC

| Metric | Young Collection | Old Collection |
|--------|-----------------|----------------|
| **Pause Time** | 1-5ms | 10-50ms |
| **Throughput** | High | Medium |
| **Memory Overhead** | 2-3x | 1.5-2x |
| **Best For** | Short-lived objects | Long-lived objects |

### Concurrent GC

| Metric | Value |
|--------|-------|
| **Pause Time** | 1-5ms |
| **Throughput** | Medium-High |
| **Memory Overhead** | 2-3x |
| **Best For** | Low-latency applications |

## Tuning Parameters

### Heap Sizing

```rust
let config = GcConfig {
    young_gen_size: 32 * 1024 * 1024,    // Larger young gen
    old_gen_size: 256 * 1024 * 1024,      // Larger old gen
    promotion_threshold: 15,               // Later promotion
};
```

### Pause Time Targets

```rust
let config = GcConfig {
    target_pause_time: Duration::from_millis(1),  // Aggressive
    max_pause_time: Duration::from_millis(10),   // Limit
};
```

### Trigger Thresholds

```rust
let config = GcConfig {
    collection_trigger_memory: 0.8,        // Trigger at 80% heap
    collection_trigger_rate: 1024 * 1024, // 1 MB/s allocation
};
```

## Best Practices

1. **Choose Right GC**: Use concurrent for low latency, generational for throughput
2. **Tune Heap Sizes**: Match application allocation patterns
3. **Monitor Statistics**: Track pause times and throughput
4. **Handle Finalizers Carefully**: They add overhead
5. **Test Real Workloads**: GC behavior varies by workload

## Testing

```bash
# Run all tests
cargo test -p vm-gc

# Test generational GC
cargo test -p vm-gc --lib generational

# Test concurrent GC
cargo test -p vm-gc --lib concurrent

# Test adaptive GC
cargo test -p vm-gc --lib adaptive
```

## Related Crates

- **vm-core**: Domain models and VM aggregates
- **vm-mem**: Memory management (GC integration)
- **vm-engine**: Execution engine (coordinates with GC)
- **vm-optimizers**: GC optimization decisions

## Platform Support

| Platform | Generational | Concurrent | Notes |
|----------|--------------|------------|-------|
| Linux | ✅ Full | ✅ Full | Best support |
| macOS | ✅ Full | ⚠️ Partial | Good |
| Windows | ✅ Full | ⚠️ Partial | Good |

## License

[Your License Here]

## Contributing

Contributions welcome! Please:
- Benchmark GC performance
- Add new GC algorithms
- Reduce pause times
- Improve throughput

## See Also

- [Generational GC Hypothesis](https://www.cs.kent.ac.uk/people/staff/pjr/gcNotes/gcNotes.html)
- [Tri-color Marking](https://www.memorymanagement.org/gcbook/chapter-4/2-tri-color-marking.html)
- [Concurrent GC](https://www.oracle.com/javase/8/docs/technotes/guides/vm/gctuning/gctuning.html)
