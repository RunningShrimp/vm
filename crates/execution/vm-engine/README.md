# vm-engine

Unified execution engine providing high-performance instruction interpretation and JIT compilation with tiered execution, hot-spot detection, and advanced optimization frameworks.

## Overview

`vm-engine` is the execution heart of the Rust VM project, implementing a hybrid execution strategy combining fast interpretation with aggressive JIT compilation. It features tiered execution, intelligent hot-spot detection, and comprehensive optimization frameworks including machine learning guidance.

## Architecture

### Hybrid Execution Model

```
┌─────────────────────────────────────────────────┐
│           vm-engine (Unified Execution)          │
├─────────────────────────────────────────────────┤
│                                                  │
│  ┌──────────────┐        ┌──────────────┐       │
│  │ Interpreter  │───────▶│  Hot-spot    │       │
│  │              │        │  Detector    │       │
│  │ • Fast exec  │        │              │       │
│  │ • Low overhead│       │ • Frequency  │       │
│  └──────────────┘        │ • Edge count │       │
│                          └──────┬───────┘       │
│                                 │                │
│                          ┌──────▼───────┐       │
│                          │ Tiered JIT   │       │
│                          │              │       │
│                          │ • Tier 1:    │       │
│                          │   Quick compile│     │
│                          │ • Tier 2:    │       │
│                          │   Optimized  │       │
│                          └──────┬───────┘       │
│                                 │                │
│                          ┌──────▼───────┐       │
│                          │ Optimizations│       │
│                          │              │       │
│                          │ • SIMD       │       │
│                          │ • Inline     │       │
│                          │ • ML-guided  │       │
│                          └──────────────┘       │
└─────────────────────────────────────────────────┘
```

## Key Components

### 1. Interpreter (`src/interpreter/`)
Fast, lightweight instruction interpreter for cold code and startup.

**Features**:
- Low-overhead dispatch
- Efficient instruction decoding
- Fast path for common operations
- Hot-spot detection hooks

**Files**:
- `mod.rs`: Main interpreter interface
- `async_executor.rs`: Async execution support

### 2. JIT Compiler (`src/jit/`)
Multi-tier JIT compilation with Cranelift backend.

**Components**:

#### Core JIT (`src/jit/`)
- **`core.rs`**: Main JIT compiler interface
- **`codegen.rs`**: Code generation with Cranelift
- **`optimizer.rs`**: IR optimization passes
- **`tiered_compiler.rs`**: Multi-tier compilation strategy
- **`hotspot_detector.rs`**: Hot-spot detection algorithms

#### Caching (`src/jit/cache/`)
- **`manager.rs`**: Code cache management
- **`code_cache.rs`**: LRU cache for compiled code
- **`tiered_cache.rs`**: Multi-level caching strategy
- **`translation_cache.rs`**: Translation result caching

#### Optimization (`src/jit/`)
- **`block_chaining.rs`**: Basic block chaining
- **`branch_target_cache.rs`**: Branch target prediction
- **`instruction_scheduler.rs`**: Instruction scheduling
- **`register_allocator.rs`**: Register allocation
- **`parallel.rs`**: Parallel compilation

### 3. Advanced JIT (`src/jit_advanced/`)
Experimental and advanced optimization features.

**Features**:
- Adaptive optimization
- AOT (Ahead-Of-Time) compilation
- ML-guided optimization decisions
- SIMD optimization
- Loop optimizations
- Graph-coloring register allocator

**Components**:
- `adaptive_optimizer.rs`: Adaptive optimization strategies
- `aot_integration.rs`: AOT compilation support
- `ml_guided_jit.rs`: Machine learning guidance
- `simd_integration.rs`: SIMD vectorization
- `loop_opt.rs`: Loop optimization passes
- `parallel_compiler.rs`: Parallel compilation framework

### 4. Executor (`src/executor/`)
Async and distributed execution frameworks.

**Components**:
- `mod.rs`: Executor traits and interfaces
- `distributed/`: Distributed execution
  - `coordinator.rs`: Multi-node coordination
  - `scheduler.rs`: Task scheduling
  - `discovery.rs`: Node discovery

### 5. Hot-spot Detection
Intelligent detection of frequently executed code.

**Algorithms**:
- Frequency-based detection
- Edge counting in CFG
- EWMA (Exponentially Weighted Moving Average)
- ML model-based prediction

**Configuration**:
```rust
pub struct HotspotConfig {
    pub threshold: usize,        // Execution count threshold
    pub sample_interval: u64,    // Sampling interval
    pub enable_ewma: bool,       // Enable EWMA smoothing
    pub enable_ml: bool,         // Enable ML prediction
}
```

## Features

### Default Features
- **`std`**: Standard library support
- **`interpreter`**: Interpreter execution engine

### Optional Features
- **`jit`**: JIT compilation (always compiled-in, feature controls optimizations)
- **`jit-full`**: Full JIT with vm-engine-jit integration
- **`async`**: Async executor support
- **`executor`**: Async execution framework
- **`debug`**: Debugging support
- **`all-engines`**: Enable both interpreter and JIT
- **`experimental`**: Experimental features (executor, async batch)

## Usage

### Basic Interpreter Execution

```rust
use vm_engine::interpreter::Interpreter;
use vm_core::{VirtualMachineAggregate, VmConfig};

let vm = VirtualMachineAggregate::new(VmConfig::default())?;
let mut interpreter = Interpreter::new(vm)?;

// Execute instructions
interpreter.run()?;

// Check execution stats
let stats = interpreter.stats();
println!("Instructions: {}", stats.instruction_count);
```

### JIT Compilation

```rust
use vm_engine::jit::JitCompiler;
use vm_ir::IRBlock;

let mut jit = JitCompiler::new()?;

// Compile IR block
let block = IRBlock::new(/* ... */);
let compiled = jit.compile_block(&block)?;

// Execute compiled code
compiled.execute()?;
```

### Tiered Compilation

```rust
use vm_engine::jit::TieredCompiler;

let mut tiered = TieredCompiler::new()?;

// Tier 1: Quick compilation for cold code
let fast_code = tiered.compile_tier1(block)?;

// Later: Tier 2: Optimized compilation for hot code
if tiered.is_hotspot(block) {
    let optimized = tiered.compile_tier2(block)?;
    tiered.replace_code(block, optimized)?;
}
```

### Hot-spot Detection

```rust
use vm_engine::jit::HotspotDetector;

let detector = HotspotDetector::new()?;

// Monitor execution
detector.record_execution(block_id);

// Check if hot-spot
if detector.is_hotspot(block_id) {
    println!("Block {:?} is hot!", block_id);
}
```

### Async Execution

```rust
use vm_engine::executor::AsyncExecutor;
use vm_core::async_executor::AsyncTask;

#[tokio::main]
async fn main() {
    let executor = AsyncExecutor::new()?;

    // Execute async
    let result = executor.execute_async(async {
        // Async execution logic
        Ok(())
    }).await?;
}
```

## Performance Optimization

### 1. Tiered Compilation
- **Tier 1**: Fast compilation (10-100ms)
  - Basic optimization
  - Quick startup
  - Good for cold code

- **Tier 2**: Optimized compilation (100-1000ms)
  - Aggressive optimization
  - Better performance
  - Used for hot-spots

### 2. Hot-spot Detection
- Frequency threshold: Configurable (default: 1000 executions)
- Sampling interval: Balance overhead vs accuracy
- EWMA smoothing: Reduce noise in detection

### 3. Code Caching
- LRU cache: Compiled code cache
- Multi-level cache: L1 (hot), L2 (warm), L3 (compiled)
- Cache size: Configurable (default: 10,000 blocks)

### 4. SIMD Optimization
- Auto-vectorization: Detect vectorizable loops
- Platform-specific: x86 SSE/AVX, ARM NEON
- Fallback: Scalar code for unsupported platforms

### 5. ML-Guided Optimization
- Random forest model: Predict optimization benefit
- A/B testing: Compare optimization strategies
- Adaptive tuning: Adjust parameters based on feedback

## Benchmarks

### Running Benchmarks

```bash
# JIT compilation benchmark
cargo bench -p vm-engine --bench jit_compilation_bench

# TLB lookup benchmark
cargo bench -p vm-engine --bench tlb_lookup_bench

# Cross-architecture translation
cargo bench -p vm-engine --bench cross_arch_translation_bench

# Cache performance
cargo bench -p vm-engine --bench p1_2_cache_bench

# Profile-guided optimization
cargo bench -p vm-engine --bench pgo_performance_bench

# Async batch execution
cargo bench -p vm-engine --bench async_batch_bench
```

### Performance Tips

1. **Enable tiered compilation**: Best cold/warm/hot code balance
2. **Tune hot-spot threshold**: Lower for more aggressive JIT
3. **Increase cache size**: For larger working sets
4. **Enable SIMD**: For vectorizable workloads
5. **Use ML guidance**: For adaptive optimization

## Configuration

### JIT Configuration

```rust
use vm_engine::jit::JitConfig;

let config = JitConfig {
    tier1_enabled: true,
    tier2_threshold: 1000,
    cache_size: 10_000,
    enable_simd: true,
    enable_ml: true,
    opt_level: OptimizationLevel::Aggressive,
};

let jit = JitCompiler::with_config(config)?;
```

### Hot-spot Configuration

```rust
use vm_engine::jit::HotspotConfig;

let config = HotspotConfig {
    threshold: 1000,              // Execution count
    sample_interval: 100,         // Instructions per sample
    enable_ewma: true,            // Enable EWMA
    ewma_alpha: 0.1,              // EWMA smoothing factor
    enable_ml: true,              // Enable ML prediction
};
```

## Architecture Diagram

```
┌───────────────────────────────────────────────────────────┐
│                     vm-engine                              │
├───────────────────────────────────────────────────────────┤
│                                                            │
│  ┌──────────────┐    ┌──────────────┐    ┌─────────────┐ │
│  │ Interpreter  │───▶│ Hot-spot     │───▶│  Tiered JIT │ │
│  │              │    │ Detector     │    │             │ │
│  │ Fast exec    │    │              │    │ • Tier 1    │ │
│  │ Low overhead │    └──────────────┘    │ • Tier 2    │ │
│  └──────────────┘                        └──────┬──────┘ │
│                                                   │        │
│                                        ┌──────────▼─────┐│
│                                        │  Optimizations ││
│                                        │                ││
│                                        │ • SIMD         ││
│                                        │ • Inline cache ││
│                                        │ • ML-guided    ││
│                                        │ • Loop opt     ││
│                                        └────────────────┘│
│                                                            │
│  ┌──────────────────────────────────────────────────┐   │
│  │            Cache Management                       │   │
│  │  • Code cache (LRU)                               │   │
│  │  • Translation cache                              │   │
│  │  • Tiered cache (L1/L2/L3)                       │   │
│  └──────────────────────────────────────────────────┘   │
│                                                            │
│  ┌──────────────────────────────────────────────────┐   │
│  │            Async/Distributed                      │   │
│  │  • Async executor                                 │   │
│  │  • Distributed coordinator                        │   │
│  │  • Task scheduler                                 │   │
│  └──────────────────────────────────────────────────┘   │
└───────────────────────────────────────────────────────────┘
```

## Design Patterns

### 1. Tiered Compilation
Balance compilation speed vs execution performance.

### 2. Hot-spot Detection
Identify frequently executed code for optimization investment.

### 3. Adaptive Optimization
Adjust optimization strategies based on runtime feedback.

### 4. Cache Management
Multi-level caching for different data access patterns.

### 5. Async Execution
Non-blocking execution for I/O-bound operations.

## Advanced Features

### Machine Learning Guidance

```rust
use vm_engine::jit::ml_guided_jit::MlGuidedJit;

let ml_jit = MlGuidedJit::new()?;

// Get optimization recommendation
let strategy = ml_jit.recommend_optimization(block)?;

// Apply recommendation
match strategy {
    OptimizationStrategy::Aggressive => {
        jit.compile_aggressive(block)?;
    }
    OptimizationStrategy::Conservative => {
        jit.compile_basic(block)?;
    }
}
```

### Parallel Compilation

```rust
use vm_engine::jit_advanced::parallel_compiler::ParallelCompiler;

let compiler = ParallelCompiler::new(num_threads)?;

// Compile blocks in parallel
let results = compiler.compile_parallel(blocks)?;
```

### SIMD Optimization

```rust
use vm_engine::jit::simd_integration::SimdOptimizer;

let simd_opt = SimdOptimizer::new()?;

// Optimize for SIMD
let optimized = simd_opt.optimize_vectorizable(block)?;
```

## Related Crates

- **vm-core**: Domain models and business logic
- **vm-ir**: Intermediate representation
- **vm-mem**: Memory management
- **vm-engine-jit**: Extended JIT functionality
- **vm-accel**: Hardware acceleration (KVM, HVF, WHPX)

## Performance Characteristics

### Interpreter
- **Startup**: < 1ms
- **Overhead**: 2-5x native
- **Memory**: Low (~1MB)
- **Best for**: Cold code, short-lived VMs

### JIT Tier 1
- **Compilation**: 10-100ms per block
- **Overhead**: 1.2-2x native
- **Memory**: Medium (~10MB)
- **Best for**: Warm code

### JIT Tier 2
- **Compilation**: 100-1000ms per block
- **Overhead**: 1.05-1.2x native
- **Memory**: High (~50MB)
- **Best for**: Hot code, long-running VMs

## Testing

```bash
# Run all tests
cargo test -p vm-engine

# Run interpreter tests
cargo test -p vm-engine --lib interpreter

# Run JIT tests
cargo test -p vm-engine --lib jit

# Run with coverage
cargo tarpaulin -p vm-engine --out Html
```

## License

[Your License Here]

## Contributing

Contributions welcome! Please:
- Ensure all tests pass
- Follow Rust style guidelines
- Add benchmarks for performance changes
- Document new optimizations

## See Also

- [Cranelift Documentation](https://docs.rs/cranelift/)
- [JIT Compilation Techniques](https://en.wikipedia.org/wiki/Just-in-time_compilation)
- [Hot-spot Detection](https://en.wikipedia.org/wiki/Hot_spot_(computer_science))
