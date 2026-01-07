# vm-mem

Comprehensive memory management subsystem featuring NUMA-aware allocation, unified MMU, advanced TLB optimization, SIMD acceleration, and concurrent data structures.

## Overview

`vm-mem` provides the memory management foundation for the Rust VM project, implementing high-performance memory operations with sophisticated caching, NUMA optimization, and platform-specific acceleration.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              vm-mem (Memory Management)                  │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐    ┌──────────────┐    ┌───────────┐│
│  │  Unified MMU │───▶│  TLB Hierarchy│───▶│  Physical ││
│  │              │    │              │    │  Memory   ││
│  │ • Page tables│    │ • L1/L2/L3   │    │           ││
│  │ • Addressing │    │ • Prefetch   │    │ • NUMA    ││
│  │ • Protection │    │ • Prediction │    │ • Pools    ││
│  └──────────────┘    └──────────────┘    │ • SIMD    ││
│                                         │ • Async   ││
│                                         └───────────┘│
│                                                  │     │
│  ┌──────────────────────────────────────────────┘     │
│                                                      │
│  ┌──────────────────────────────────────────────┐   │
│  │         Optimization Framework                │   │
│  │  • Unified optimization coordinator            │   │
│  │  • Adaptive algorithms                        │   │
│  │  • Performance monitoring                     │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

## Key Components

### 1. Memory Management Unit (`src/mmu.rs`)
Unified MMU implementation with comprehensive address translation.

**Features**:
- **Multi-level page tables**: Support 2/3/4-level page tables
- **Address spaces**: Separate user/supervisor spaces
- **Memory protection**: Read/write/execute permissions
- **TLB integration**: Tight TLB coupling for performance
- **NUMA-aware**: NUMA node allocation policies

**Usage**:
```rust
use vm_mem::mmu::Mmu;

let mmu = Mmu::new()?;

// Translate address
let physical = mmu.translate(virtual_address)?;

// Map page
mmu.map_page(virtual_addr, physical_addr, permissions)?;

// Unmap page
mmu.unmap_page(virtual_address)?;
```

### 2. TLB Hierarchy (`src/tlb/`)
Advanced Translation Lookaside Buffer with multi-level caching.

#### Components

**Unified TLB (`src/tlb/unified_tlb.rs`)**:
- Multi-level cache hierarchy (L1/L2/L3)
- Configurable cache sizes
- Statistics and monitoring

**Hierarchy (`src/tlb/hierarchy/`)**:
- `flush.rs`: TLB flush operations
- `unified_hierarchy.rs`: Unified multi-level TLB
- `management/`: TLB management policies

**Optimization (`src/tlb/optimization/`)**:
- `predictor.rs`: Access pattern prediction
- `prefetch.rs`: Data prefetching
- `static_preheat.rs`: Static cache warming

**Features**:
- **L1 TLB**: Small, fast (64-256 entries)
- **L2 TLB**: Medium (512-2048 entries)
- **L3 TLB**: Large (4096-16384 entries)
- **Prefetch**: Adaptive prefetch algorithms
- **Prediction**: ML-based access prediction

**Usage**:
```rust
use vm_mem::tlb::unified_tlb::UnifiedTlb;

let tlb = UnifiedTlb::new()?;

// Lookup translation (fast path)
if let Some(physical) = tlb.lookup(virtual_address) {
    return Ok(physical);
}

// TLB miss - go to MMU
let physical = mmu.translate(virtual_address)?;
tlb.insert(virtual_address, physical);
```

### 3. Physical Memory (`src/memory/`)

#### Memory Allocator (`src/memory/allocator.rs`)
- **Slab allocator**: Efficient fixed-size allocation
- **Bump allocator**: Fast temporary allocation
- **Pool allocator**: Object pooling

#### NUMA Allocation (`src/memory/numa_allocator.rs`)
- **NUMA-aware**: Bind memory to NUMA nodes
- **Policy-based**: Local, interleaved, replication
- **Performance**: Optimized for multi-socket systems

**Usage**:
```rust
use vm_mem::memory::numa_allocator::NumaAllocator;

let allocator = NumaAllocator::new()?;

// Allocate on local NUMA node
let memory = allocator.allocate_local(size)?;

// Allocate on specific node
let memory = allocator.allocate_on_node(size, node_id)?;

// Interleaved allocation
let memory = allocator.allocate_interleaved(size)?;
```

#### Memory Pool (`src/memory/memory_pool.rs`)
- Pre-allocated memory pools
- Reduce allocation overhead
- Thread-safe (concurrent access)

#### Protection (`src/memory/protection.rs`)
- Memory protection flags
- W^X policy enforcement
- Page-level permissions

### 4. SIMD Acceleration (`src/simd_memcpy.rs`)
Platform-specific SIMD memory operations.

**Features**:
- **x86_64**: SSE2, AVX, AVX2, AVX-512
- **ARM64**: NEON
- **Fallback**: Scalar implementation
- **Auto-detection**: Runtime capability detection
- **Adaptive**: Choose best implementation

**Usage**:
```rust
use vm_mem::simd_memcpy::simd_memcpy;

// Fast SIMD memory copy
unsafe {
    simd_memcpy(dst, src, len);
}
```

### 5. Async MMU (`src/async_mmu.rs`)
Async memory management for high-concurrency scenarios.

**Features**:
- Non-blocking address translation
- Async page fault handling
- Concurrent batch operations

**Usage**:
```rust
use vm_mem::async_mmu::AsyncMmu;

#[tokio::main]
async fn main() {
    let mmu = AsyncMmu::new()?;

    // Async translation
    let physical = mmu.translate_async(vaddr).await?;

    // Batch translation
    let results = mmu.translate_batch(vaddrs).await?;
}
```

### 6. Optimization Framework (`src/optimization/`)
Unified optimization coordinator for adaptive performance tuning.

**Components**:
- `unified.rs`: Unified optimization interface
- `adaptive/`: Adaptive algorithms
- Coordinator for cross-component optimization

## Features

### Default Features
- Standard library support

### Optional Features
- **`async`**: Async MMU support (tokio, async-trait)

## Performance Characteristics

### TLB Performance

| TLB Level | Size | Latency | Hit Rate |
|-----------|------|---------|----------|
| L1 | 64-256 entries | ~1 cycle | 80-95% |
| L2 | 512-2K entries | ~5 cycles | 95-99% |
| L3 | 4K-16K entries | ~20 cycles | 99-99.9% |

### Memory Allocation

| Allocator | Throughput | Latency | Fragmentation |
|-----------|------------|---------|---------------|
| Slab | High | Low | Minimal |
| Bump | Very High | Very Low | N/A |
| Pool | Medium | Medium | Low |

### SIMD Performance

| Platform | Implementation | Speedup |
|----------|---------------|---------|
| x86_64 AVX-512 | AVX-512 | 8-16x |
| x86_64 AVX2 | AVX2 | 4-8x |
| ARM64 | NEON | 4-16x |
| Fallback | Scalar | 1x |

## Benchmarks

### Running Benchmarks

```bash
# MMU translation
cargo bench -p vm-mem --bench mmu_translate

# TLB optimization
cargo bench -p vm-mem --bench tlb_optimized

# NUMA performance
cargo bench -p vm-mem --bench numa_performance

# Async MMU
cargo bench -p vm-mem --bench async_mmu_performance --features async

# Memory allocation
cargo bench -p vm-mem --bench memory_allocation

# SIMD memcpy
cargo bench -p vm-mem --bench simd_memcpy

# Concurrent MMU
cargo bench -p vm-mem --bench concurrent_mmu

# Allocator performance
cargo bench -p vm-mem --bench allocator_bench
```

### Performance Tips

1. **Enable NUMA**: For multi-socket systems
2. **Tune TLB sizes**: Match workload patterns
3. **Use SIMD**: For bulk memory operations
4. **Enable prefetch**: For predictable access patterns
5. **Use async**: For high-concurrency scenarios

## Configuration

### TLB Configuration

```rust
use vm_mem::tlb::unified_tlb::TlbConfig;

let config = TlbConfig {
    l1_size: 256,              // L1 entries
    l2_size: 2048,             // L2 entries
    l3_size: 16384,            // L3 entries
    enable_prefetch: true,
    enable_prediction: true,
    prefetch_distance: 4,      // Prefetch ahead
};

let tlb = UnifiedTlb::with_config(config)?;
```

### NUMA Configuration

```rust
use vm_mem::memory::numa_allocator::NumaPolicy;

let policy = NumaPolicy::Local {  // Use local node
    preferred_node: 0,
    fallback: true,
};

let allocator = NumaAllocator::with_policy(policy)?;
```

### SIMD Detection

```bash
# Check SIMD capabilities
cargo run --bin simd_capabilities

# Quick verification
cargo run --bin simd_quick_verify
```

## Advanced Features

### TLB Prefetching

```rust
use vm_mem::tlb::optimization::prefetch::TlbPrefetcher;

let prefetcher = TlbPrefetcher::new()?;

// Sequential access
prefetcher.prefetch_sequential(base_address, count)?;

// Strided access
prefetcher.prefetch_strided(base_address, stride, count)?;
```

### NUMA Optimization

```rust
use vm_mem::memory::numa_allocator::NumaAllocator;

// Allocate on same node as CPU
let memory = allocator.allocate_cpu_local(size, cpu_id)?;

// Interleaved for bandwidth
let memory = allocator.allocate_interleaved(size)?;

// Replicated for latency
let memory = allocator.allocate_replicated(size, nodes)?;
```

### SIMD Memory Operations

```rust
use vm_mem::simd_memcpy::{MemcpyStrategy, simd_memcpy_optimized};

// Auto-detect best strategy
let strategy = MemcpyStrategy::detect();

// Use optimized copy
unsafe {
    simd_memcpy_optimized(dst, src, len, strategy);
}
```

## Architecture Diagram

```
┌──────────────────────────────────────────────────────────┐
│                       vm-mem                             │
├──────────────────────────────────────────────────────────┤
│                                                           │
│  ┌─────────────────────────────────────────────────┐    │
│  │                Unified MMU                      │    │
│  │  • Multi-level page tables                      │    │
│  │  • Address translation                           │    │
│  │  • Memory protection                             │    │
│  │  • NUMA-aware allocation                          │    │
│  └────────────┬────────────────────────────────────┘    │
│               │                                         │
│  ┌────────────▼────────────────────────────────────┐   │
│  │            TLB Hierarchy                         │   │
│  │  ┌────────┐  ┌────────┐  ┌────────┐            │   │
│  │  │ L1 TLB │──│ L2 TLB │──│ L3 TLB │            │   │
│  │  │ 256    │  │ 2K     │  │ 16K    │            │   │
│  │  └────────┘  └────────┘  └────────┘            │   │
│  │                                           Prefetch│
│  └───────────────────────────────────────────────────┘   │
│                                                           │
│  ┌─────────────────────────────────────────────────┐    │
│  │            Physical Memory                       │    │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐      │    │
│  │  │   Slab   │  │   Bump   │  │   Pool   │      │    │
│  │  │ Allocator│  │ Allocator│  │ Allocator│      │    │
│  │  └──────────┘  └──────────┘  └──────────┘      │    │
│  │                                                  │    │
│  │  ┌──────────────┐  ┌──────────────┐            │    │
│  │  │ NUMA Alloc   │  │ SIMD memcpy  │            │    │
│  │  │ • Local      │  │ • x86 SSE/AVX│            │    │
│  │  │ • Interleave │  │ • ARM NEON   │            │    │
│  │  └──────────────┘  └──────────────┘            │    │
│  └──────────────────────────────────────────────────┘    │
│                                                           │
│  ┌─────────────────────────────────────────────────┐    │
│  │        Optimization & Monitoring                  │    │
│  │  • Performance monitoring                        │    │
│  │  • Adaptive algorithms                           │    │
│  │  • Statistics collection                          │    │
│  └──────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────┘
```

## Design Patterns

### 1. Multi-level Caching
L1/L2/L3 TLB hierarchy for performance.

### 2. Policy-Based Allocation
NUMA policies for different workloads.

### 3. Adaptive Optimization
Runtime performance tuning.

### 4. SIMD Abstraction
Platform-specific optimizations with fallback.

### 5. Async/Await
Non-blocking operations for concurrency.

## Memory Layout

### Virtual Address Space
```
+-------------------+ 0xFFFFFFFFFFFFFFFF
| Kernel Space      |
|                   |
+-------------------+ 0xC000000000000000
|                   |
| User Space        |
|                   |
+-------------------+ 0x0000000000000000
```

### Physical Memory Organization
```
+-------------------+
| NUMA Node 0      |
| - Socket 0 memory |
+-------------------+
| NUMA Node 1      |
| - Socket 1 memory |
+-------------------+
```

## Best Practices

1. **NUMA-aware**: Bind memory to local NUMA node
2. **Use SIMD**: For large memory operations
3. **TLB tuning**: Match cache sizes to workload
4. **Enable prefetch**: For predictable patterns
5. **Batch operations**: Use async for concurrency

## Testing

```bash
# Run all tests
cargo test -p vm-mem

# Run MMU tests
cargo test -p vm-mem --lib mmu

# Run TLB tests
cargo test -p vm-mem --lib tlb

# Run concurrent tests with loom
cargo test -p vm-mem --lib concurrent --features loom
```

## Related Crates

- **vm-core**: Domain models and error handling
- **vm-accel**: Hardware acceleration (uses memory management)
- **vm-engine**: Execution engine (consumes memory services)

## Platform Support

| Platform | SIMD | NUMA | Features |
|----------|------|------|----------|
| x86_64 Linux | ✅ SSE/AVX | ✅ | Full |
| ARM64 Linux | ✅ NEON | ✅ | Full |
| x86_64 macOS | ✅ SSE/AVX | ⚠️ Limited | Most |
| Windows | ✅ SSE/AVX | ❌ | Partial |

## Performance Tuning

### For High Throughput
- Use larger TLBs
- Enable SIMD
- NUMA interleaved allocation

### For Low Latency
- Use smaller L1 TLB
- NUMA local allocation
- Enable prefetch

### For High Concurrency
- Use async MMU
- Lock-free data structures
- Concurrent allocators

## License

[Your License Here]

## Contributing

Contributions welcome! Please:
- Benchmark performance changes
- Test on multiple platforms
- Add tests for new features
- Document NUMA behavior

## See Also

- [NUMA API](https://numactl.readthedocs.io/)
- [SIMD Programming](https://www.intel.com/content/www/us/en/docs/intrinsics-guide/)
- [TLB Design](https://en.wikipedia.org/wiki/Translation_lookaside_buffer)
