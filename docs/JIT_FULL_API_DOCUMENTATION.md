# JIT Full Feature API Documentation

**Version**: 1.0
**Date**: 2026-01-03
**Feature**: `jit-full` in vm-engine

---

## Table of Contents

1. [Overview](#overview)
2. [Basic Types](#basic-types)
3. [Advanced JIT Types](#advanced-jit-types)
4. [Usage Patterns](#usage-patterns)
5. [API Reference](#api-reference)
6. [Examples](#examples)

---

## Overview

The `jit-full` feature in `vm-engine` provides a unified API for both basic and advanced JIT functionality. When enabled, it re-exports all core types from `vm-engine-jit`, allowing you to import everything from a single crate.

### Activation

```toml
# Cargo.toml
[dependencies]
vm-engine = { path = "../vm-engine", features = ["jit-full"] }
```

### What's Included

- ✅ Basic JIT compilation (always available)
- ✅ Tiered compilation system
- ✅ AOT (Ahead-Of-Time) caching
- ✅ ML-guided optimization
- ✅ Block chaining optimization
- ✅ Loop optimization
- ✅ Inline caching
- ✅ Unified garbage collection
- ✅ Adaptive optimization
- ✅ CPU vendor optimizations

---

## Basic Types

These types are available even without the `jit-full` feature:

### JITCompiler

Basic just-in-time compiler for translating IR to native code.

```rust
use vm_engine::JITCompiler;

let config = JITConfig::default();
let mut jit = JITCompiler::new(config);
```

**Methods**:
- `new(config: JITConfig) -> Self` - Create a new JIT compiler
- `compile(&mut self, block: &IRBlock) -> Result<JITCode>` - Compile a basic block
- `optimize(&mut self, code: &mut JITCode)` - Optimize compiled code

### JITConfig

Configuration options for the JIT compiler.

```rust
use vm_engine::JITConfig;

let config = JITConfig {
    optimization_level: OptimizationLevel::Aggressive,
    enable_inline_cache: true,
    ..Default::default()
};
```

**Fields**:
- `optimization_level: OptimizationLevel` - Optimization aggressiveness
- `enable_inline_cache: bool` - Enable inline caching
- `enable_block_chaining: bool` - Enable basic block chaining
- `max_code_size: usize` - Maximum compiled code size

---

## Advanced JIT Types

These types require the `jit-full` feature:

### Core JIT Types

#### Jit

Advanced JIT compiler with tiered compilation and ML guidance.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::Jit;

let jit = Jit::new();
```

**Capabilities**:
- Multi-tier compilation
- Hotspot detection
- Profile-guided optimization
- Adaptive optimization strategies

#### JitContext

Execution context for the JIT compiler.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::JitContext;

let context = JitContext::new();
```

**Purpose**: Manages compilation state, code caches, and optimization metadata.

---

### Tiered Compilation

#### TieredCompiler

Multi-tier JIT compiler with different optimization levels.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::TieredCompiler;

let compiler = TieredCompiler::new()?;
```

**Tiers**:
1. **Tier 1**: Fast compilation (no optimization)
2. **Tier 2**: Basic optimization (inlining, constant propagation)
3. **Tier 3**: Advanced optimization (loop optimization, vectorization)

**Methods**:
- `new() -> Result<Self>` - Create a new tiered compiler
- `compile_at_tier(&mut self, block: &IRBlock, tier: usize) -> Result<JITCode>` - Compile at specific tier
- `promote_to_next_tier(&mut self, code: &JITCode) -> Result<()>` - Promote code to next tier

**Use Case**: Start with fast compilation, progressively optimize hot code.

---

### Compilation Caching

#### CompileCache

In-memory cache for compiled code.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::CompileCache;

let cache = CompileCache::new(1000); // 1000 entries max
```

**Methods**:
- `new(capacity: usize) -> Self` - Create cache with capacity limit
- `get(&self, key: &BlockKey) -> Option<&JITCode>` - Retrieve compiled code
- `insert(&mut self, key: BlockKey, code: JITCode)` - Cache compiled code
- `clear(&mut self)` - Clear all cached code

**Use Case**: Avoid recompiling frequently executed blocks.

---

### AOT (Ahead-Of-Time) Compilation

#### AotCache

Persistent cache for pre-compiled code.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::AotCache;

let cache = AotCache::new(AotCacheConfig::default())?;
```

**Features**:
- Persistent storage across sessions
- Pre-compiled code loading
- Cross-session code reuse
- Reduced startup time

**Methods**:
- `new(config: AotCacheConfig) -> Result<Self>` - Create AOT cache
- `load(&mut self, block: &IRBlock) -> Result<Option<JITCode>>` - Load pre-compiled code
- `store(&mut self, block: &IRBlock, code: &JITCode) -> Result<()>` - Store compiled code
- `flush(&mut self) -> Result<()>` - Persist cache to disk

#### AotFormat

Serialized format for AOT-compiled code.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::AotFormat;

let format = AotFormat:: serialize(&code)?;
```

**Purpose**: Defines binary format for storing compiled code.

#### AotLoader

Loader for AOT-compiled code from disk.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::AotLoader;

let loader = AotLoader::new("/path/to/cache")?;
let code = loader.load_block(block_key)?;
```

**Methods**:
- `new(cache_path: &str) -> Result<Self>` - Create loader with cache path
- `load_block(&self, key: BlockKey) -> Result<JITCode>` - Load specific block
- `load_all(&self) -> Result<Vec<JITCode>>` - Load all cached blocks

---

### ML-Guided Optimization

#### MLModel

Machine learning model for predicting optimal compilation strategies.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::MLModel;

let model = MLModel::new()?;
```

**Features**:
- Predict optimal optimization level
- Identify hot code paths
- Suggest inlining candidates
- Recommend compilation tier

**Methods**:
- `new() -> Result<Self>` - Load/create ML model
- `predict_optimization_level(&self, block: &IRBlock) -> OptimizationLevel` - Predict best optimization
- `predict_hotness(&self, block: &IRBlock) -> f64` - Predict execution frequency
- `update(&mut self, block: &IRBlock, metrics: &ExecutionMetrics)` - Update model with runtime data

#### EwmaHotspotDetector

Exponentially Weighted Moving Average hotspot detector.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::EwmaHotspotDetector;

let detector = EwmaHotspotDetector::new(100.0); // alpha = 100.0
```

**Purpose**: Detect frequently executed code using EWMA algorithm.

**Methods**:
- `new(alpha: f64) -> Self` - Create detector with smoothing factor
- `record_execution(&mut self, block_id: BlockId)` - Record block execution
- `is_hot(&self, block_id: BlockId) -> bool` - Check if block is hot
- `get_hotness_score(&self, block_id: BlockId) -> f64` - Get hotness score

---

### Optimization Passes

#### BlockChainer

Optimizer for linking basic blocks together.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::BlockChainer;

let chainer = BlockChainer::new();
```

**Purpose**: Reduce branch overhead by chaining sequential basic blocks.

**Methods**:
- `new() -> Self` - Create block chainer
- `chain_blocks(&mut self, blocks: Vec<&IRBlock>) -> Result<BlockChain>` - Chain blocks together
- `optimize_chain(&mut self, chain: &mut BlockChain)` - Optimize block chain

#### BlockChain

Representation of chained basic blocks.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::BlockChain;

// BlockChainer creates BlockChain instances
```

**Purpose**: Efficiently represent and execute sequential basic blocks.

#### LoopOptimizer

Optimizer for loop structures.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::LoopOptimizer;

let loopy = LoopOptimizer::new();
```

**Optimizations**:
- Loop invariant code motion
- Induction variable simplification
- Loop unrolling
- Loop vectorization hints

**Methods**:
- `new() -> Self` - Create loop optimizer
- `optimize_loop(&mut self, loop: &Loop) -> Result<Loop>` - Optimize loop structure
- `detect_loops(&self, block: &IRBlock) -> Vec<Loop>` - Detect loops in block

#### InlineCache

Cache for inline method dispatch.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::InlineCache;

let cache = InlineCache::new();
```

**Purpose**: Speed up virtual method calls by caching target addresses.

**Methods**:
- `new() -> Self` - Create inline cache
- `lookup(&mut self, call_site: CallSite, receiver: &Object) -> Option<CodePtr>` - Lookup cached target
- `update(&mut self, call_site: CallSite, receiver: &Object, target: CodePtr)` - Update cache
- `invalidate(&mut self, call_site: CallSite)` - Invalidate cache entry

---

### Garbage Collection

#### UnifiedGC

Unified garbage collector for JIT-allocated objects.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::UnifiedGC;

let gc = UnifiedGC::new()?;
```

**Features**:
- Generational collection
- Concurrent marking
- Incremental sweeping
- JIT object integration

**Methods**:
- `new() -> Result<Self>` - Create GC instance
- `allocate(&mut self, size: usize) -> Result<ObjectPtr>` - Allocate managed object
- `collect(&mut self) -> CollectionStats` - Run garbage collection
- `add_root(&mut self, root: ObjectPtr)` - Add GC root
- `remove_root(&mut self, root: ObjectPtr)` - Remove GC root

---

### Adaptive Optimization

#### AdaptiveOptimizer

Optimizer that adapts strategies based on runtime feedback.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::{AdaptiveOptimizer, AdaptiveParameters};

let params = AdaptiveParameters::default();
let optimizer = AdaptiveOptimizer::new(params);
```

**Features**:
- Dynamic optimization level adjustment
- Hotspot-based recompilation
- Profile-guided optimization
- Performance feedback integration

**Methods**:
- `new(params: AdaptiveParameters) -> Self` - Create optimizer
- `should_optimize(&self, block: &IRBlock) -> bool` - Decide if block should be optimized
- `should_recompile(&self, code: &JITCode) -> bool` - Decide if code should be recompiled
- `record_performance(&mut self, block_id: BlockId, metrics: &PerformanceMetrics)` - Record performance data

#### AdaptiveParameters

Configuration for adaptive optimization.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::AdaptiveParameters;

let params = AdaptiveParameters {
    hot_threshold: 1000,          // executions before hot
    compile_threshold: 10000,      // executions before optimization
    recompile_threshold: 100000,   // executions before recompilation
    ..Default::default()
};
```

**Fields**:
- `hot_threshold: usize` - Execution count to consider code hot
- `compile_threshold: usize` - Execution count to trigger compilation
- `recompile_threshold: usize` - Execution count to trigger recompilation
- `sample_rate: f64` - Performance sampling rate

---

### CPU Vendor Optimizations

#### VendorOptimizer

CPU vendor-specific optimizer.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::VendorOptimizer;

let optimizer = VendorOptimizer::new();
```

**Purpose**: Apply CPU-specific optimizations based on detected vendor and features.

**Methods**:
- `new() -> Self` - Create optimizer (auto-detects CPU)
- `detect_vendor() -> CpuVendor` - Detect CPU vendor
- `detect_features() -> Vec<CpuFeature>` - Detect CPU features
- `optimize_for_vendor(&self, code: &mut JITCode)` - Apply vendor-specific optimizations

#### CpuVendor

CPU vendor enumeration.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::CpuVendor;

match optimizer.detect_vendor() {
    CpuVendor::Intel => println!("Intel CPU detected"),
    CpuVendor::AMD => println!("AMD CPU detected"),
    CpuVendor::ARM => println!("ARM CPU detected"),
    CpuVendor::AppleSilicon => println!("Apple Silicon detected"),
    CpuVendor::Unknown => println!("Unknown CPU vendor"),
}
```

**Variants**:
- `Intel` - Intel x86_64 processors
- `AMD` - AMD x86_64 processors
- `ARM` - ARM processors
- `AppleSilicon` - Apple ARM-based processors (M1/M2/M3)
- `Unknown` - Unrecognized vendor

#### CpuFeature

CPU feature/capability flags.

```rust
#[cfg(feature = "jit-full")]
use vm_engine::CpuFeature;

for feature in optimizer.detect_features() {
    match feature {
        CpuFeature::AVX2 => println!("AVX2 supported"),
        CpuFeature::AVX512 => println!("AVX-512 supported"),
        CpuFeature::NEON => println!("NEON supported"),
        // ... more features
    }
}
```

**Key Features**:
- x86_64: AVX, AVX2, AVX-512, SSE4.2, BMI2, etc.
- ARM: NEON, SVE, etc.
- Apple Silicon: Apple-specific performance features

---

## Usage Patterns

### Pattern 1: Basic JIT Usage

```rust
use vm_engine::JITCompiler;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = JITConfig::default();
    let mut jit = JITCompiler::new(config);

    let block = create_ir_block();
    let code = jit.compile(&block)?;

    Ok(())
}
```

---

### Pattern 2: Tiered Compilation with Hotspot Detection

```rust
#[cfg(feature = "jit-full")]
use vm_engine::{TieredCompiler, EwmaHotspotDetector};

#[cfg(feature = "jit-full")]
fn execute_with_tiered_compilation() -> Result<(), Box<dyn std::error::Error>> {
    let mut compiler = TieredCompiler::new()?;
    let mut hotspot_detector = EwmaHotspotDetector::new(100.0);

    loop {
        let block_id = fetch_next_block();

        // Record execution
        hotspot_detector.record_execution(block_id);

        // Compile at appropriate tier
        if hotspot_detector.is_hot(block_id) {
            // Hot code: compile at tier 3
            compiler.compile_at_tier(&get_block(block_id), 3)?;
        } else if hotspot_detector.get_hotness_score(block_id) > 50.0 {
            // Warm code: compile at tier 2
            compiler.compile_at_tier(&get_block(block_id), 2)?;
        } else {
            // Cold code: compile at tier 1
            compiler.compile_at_tier(&get_block(block_id), 1)?;
        }

        // Execute compiled code
        execute_compiled_code(block_id)?;
    }
}
```

---

### Pattern 3: AOT Cache with Fallback

```rust
#[cfg(feature = "jit-full")]
use vm_engine::{AotCache, JITCompiler};

#[cfg(feature = "jit-full")]
fn execute_with_aot_cache() -> Result<(), Box<dyn std::error::Error>> {
    let mut aot_cache = AotCache::new(AotCacheConfig::default())?;
    let mut jit = JITCompiler::new(JITConfig::default());

    let block = create_ir_block();

    // Try to load from AOT cache
    let code = match aot_cache.load(&block)? {
        Some(cached_code) => {
            println!("Loaded from AOT cache");
            cached_code
        }
        None => {
            println!("Compiling and caching");
            let code = jit.compile(&block)?;
            aot_cache.store(&block, &code)?;
            code
        }
    };

    execute_code(&code)
}
```

---

### Pattern 4: ML-Guided Optimization

```rust
#[cfg(feature = "jit-full")]
use vm_engine::{MLModel, TieredCompiler, ExecutionMetrics};

#[cfg(feature = "jit-full")]
fn ml_guided_compilation() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = MLModel::new()?;
    let mut compiler = TieredCompiler::new()?;

    let block = create_ir_block();

    // Predict optimal strategy
    let opt_level = model.predict_optimization_level(&block);
    let hotness = model.predict_hotness(&block);

    println!("Predicted hotness: {:.2}", hotness);

    // Compile using ML suggestion
    let tier = match opt_level {
        OptimizationLevel::None => 1,
        OptimizationLevel::Basic => 2,
        OptimizationLevel::Aggressive => 3,
    };

    let code = compiler.compile_at_tier(&block, tier)?;

    // Execute and collect metrics
    let metrics = execute_and_measure(&code);

    // Update model with actual performance
    model.update(&block, &metrics);

    Ok(())
}
```

---

### Pattern 5: Adaptive Optimization Loop

```rust
#[cfg(feature = "jit-full")]
use vm_engine::{AdaptiveOptimizer, AdaptiveParameters};

#[cfg(feature = "jit-full")]
fn adaptive_execution() -> Result<(), Box<dyn std::error::Error>> {
    let params = AdaptiveParameters {
        hot_threshold: 1000,
        compile_threshold: 10000,
        recompile_threshold: 100000,
        ..Default::default()
    };

    let mut optimizer = AdaptiveOptimizer::new(params);

    loop {
        let block_id = fetch_next_block();
        let block = get_block(block_id);

        // Decide optimization strategy
        if optimizer.should_optimize(&block) {
            if optimizer.should_recompile(&get_compiled_code(block_id)?) {
                // Recompile with higher optimization
                recompile_with_higher_optimization(block)?;
            } else {
                // Compile for the first time
                compile_block(block)?;
            }
        }

        // Execute and record performance
        let metrics = execute_block(block_id)?;
        optimizer.record_performance(block_id, &metrics);
    }
}
```

---

### Pattern 6: CPU Vendor-Specific Optimization

```rust
#[cfg(feature = "jit-full")]
use vm_engine::VendorOptimizer;

#[cfg(feature = "jit-full")]
fn vendor_specific_optimization() -> Result<(), Box<dyn std::error::Error>> {
    let optimizer = VendorOptimizer::new();

    // Detect CPU
    let vendor = optimizer.detect_vendor();
    let features = optimizer.detect_features();

    println!("CPU Vendor: {:?}", vendor);
    println!("Features: {:?}", features);

    // Compile code
    let mut code = compile_block()?;

    // Apply vendor-specific optimizations
    optimizer.optimize_for_vendor(&mut code);

    // Example: Use AVX-512 on Intel CPUs
    if vendor == CpuVendor::Intel && features.contains(&CpuFeature::AVX512) {
        println!("Applying AVX-512 optimizations");
        // Enable AVX-512 code generation
    }

    Ok(())
}
```

---

## API Reference

### Feature Detection

#### Check if jit-full is Enabled

```rust
#[cfg(feature = "jit-full")]
fn use_advanced_features() {
    // This function only exists with jit-full
}

#[cfg(not(feature = "jit-full"))]
fn use_advanced_features() {
    panic!("This function requires the jit-full feature");
}
```

#### Conditional Compilation

```rust
use vm_engine::JITCompiler;  // Always available

#[cfg(feature = "jit-full")]
use vm_engine::TieredCompiler;  // Only with jit-full

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let jit = JITCompiler::new(Default::default());

    #[cfg(feature = "jit-full")]
    let _tiered = TieredCompiler::new()?;

    Ok(())
}
```

---

### Error Handling

All advanced JIT functions return `Result<T, E>` for proper error handling:

```rust
#[cfg(feature = "jit-full")]
use vm_engine::{TieredCompiler, JITError};

#[cfg(feature = "jit-full")]
fn compile_with_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    match TieredCompiler::new() {
        Ok(compiler) => {
            println!("Compiler created successfully");
            // Use compiler
        }
        Err(e) => {
            eprintln!("Failed to create compiler: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
```

---

## Examples

### Example 1: Complete JIT Pipeline

```rust
#[cfg(feature = "jit-full")]
use vm_engine::{
    JITCompiler, TieredCompiler, AotCache,
    MLModel, EwmaHotspotDetector, BlockChainer,
};

#[cfg(feature = "jit-full")]
fn complete_jit_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize components
    let mut jit = JITCompiler::new(Default::default());
    let tiered = TieredCompiler::new()?;
    let mut aot_cache = AotCache::new(Default::default())?;
    let ml = MLModel::new()?;
    let hotspot = EwmaHotspotDetector::new(100.0);
    let chainer = BlockChainer::new();

    // Process blocks
    for block in get_all_blocks() {
        // Check AOT cache first
        let code = match aot_cache.load(&block)? {
            Some(cached) => cached,
            None => {
                // Predict optimal strategy
                let opt_level = ml.predict_optimization_level(&block);

                // Compile with predicted optimization
                let mut code = jit.compile_with_level(&block, opt_level)?;

                // Apply block chaining
                if let Ok(chain) = chainer.chain_blocks(vec![&block]) {
                    code = chainer.optimize_chain(&mut chain)?;
                }

                // Cache for future use
                aot_cache.store(&block, &code)?;

                code
            }
        };

        // Execute
        execute_code(&code)?;

        // Update ML model
        let metrics = collect_execution_metrics();
        ml.update(&block, &metrics);
    }

    Ok(())
}
```

### Example 2: Minimal jit-full Usage

```rust
#[cfg(feature = "jit-full")]
use vm_engine::TieredCompiler;

#[cfg(feature = "jit-full")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Just use one advanced feature
    let tiered = TieredCompiler::new()?;

    let block = create_test_block();
    let code = tiered.compile_at_tier(&block, 2)?;

    execute_code(&code)?;

    Ok(())
}
```

---

## Best Practices

### 1. Feature Selection

Choose the right feature for your needs:

- **Basic JIT**: Use without any feature for minimal dependencies
- **jit-full**: Use when you need advanced optimizations
- **Conditional**: Use `#[cfg(feature = "jit-full")]` for optional features

### 2. Error Handling

Always handle errors from JIT operations:

```rust
#[cfg(feature = "jit-full")]
let tiered = TieredCompiler::new()?;  // May fail
```

### 3. Caching Strategy

Use multiple cache layers for optimal performance:

1. **Inline Cache** - Fastest, smallest scope
2. **Compile Cache** - Medium speed, session scope
3. **AOT Cache** - Slowest load, persistent across sessions

### 4. Compilation Strategy

Start with fast compilation, optimize hot code:

```rust
#[cfg(feature = "jit-full")]
use vm_engine::TieredCompiler;

// First execution: compile at tier 1 (fast)
// Subsequent executions: detect hotness
// Hot code: recompile at tier 3 (optimized)
```

### 5. ML Model Training

Train ML model with representative workloads:

```rust
#[cfg(feature = "jit-full")]
use vm_engine::MLModel;

let mut model = MLModel::new()?;

// Collect diverse training data
for block in representative_workload() {
    let metrics = execute_and_measure(&block);
    model.update(&block, &metrics);
}

// Model now provides better predictions
```

---

## Performance Considerations

### Compilation Time

| Feature | Compile Time Impact | Runtime Impact |
|---------|-------------------|----------------|
| `jit` | +10-20% | +30-50% |
| `jit-full` | +30-50% | +50-200% |

### Binary Size

| Feature | Binary Size Impact |
|---------|-------------------|
| `jit` | +200-500 KB |
| `jit-full` | +1-2 MB |

### Memory Usage

- **TieredCompiler**: ~10-50 MB (depends on cache size)
- **AotCache**: ~50-200 MB (depends on cached code)
- **MLModel**: ~5-20 MB
- **UnifiedGC**: ~10-100 MB (depends on heap size)

---

## Troubleshooting

### Issue: "unresolved import" error

**Cause**: Missing `jit-full` feature

**Solution**:
```toml
# Add to Cargo.toml
vm-engine = { path = "../vm-engine", features = ["jit-full"] }
```

### Issue: Slow compilation

**Cause**: `jit-full` includes many dependencies

**Solutions**:
- Use `cargo check` for faster verification
- Enable fewer features during development
- Use `jit` feature instead of `jit-full` if possible

### Issue: Runtime performance worse than expected

**Possible Causes**:
- Not using AOT cache for repeated executions
- Not training ML model with representative data
- Suboptimal tier selection for workload

**Solutions**:
- Enable AOT caching
- Collect training data for ML model
- Profile and tune tier thresholds

---

## Additional Resources

- **Migration Guide**: [JIT_FULL_MIGRATION_GUIDE.md](./JIT_FULL_MIGRATION_GUIDE.md)
- **Implementation Report**: [crate_merge_plan_c_report.md](../crate_merge_plan_c_report.md)
- **Example Code**: [examples/jit_full_example.rs](../examples/jit_full_example.rs)
- **Performance Baselines**: [PERFORMANCE_BASELINE.md](./PERFORMANCE_BASELINE.md)

---

*API Documentation Version: 1.0*
*Last Updated: 2026-01-03*
*Feature: jit-full*
*Status: ✅ Stable*
