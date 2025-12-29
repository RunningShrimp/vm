# Feature Gate Optimization Roadmap

**Status**: Active Planning
**Timeline**: 4 weeks
**Target**: Reduce from 254 to <150 feature gates

---

## Week 1: Critical Files (65 gates → 24 gates)

### Priority 1: vm-service/src/vm_service.rs (23 → 8 gates)

**Current Structure:**
```rust
// 23 scattered gates
#[cfg(feature = "performance")]
mod performance;

#[cfg(feature = "smmu")]
use vm_accel::SmmuManager;

#[cfg(feature = "performance")]
pub async fn load_kernel_file_async(...) { ... }

#[cfg(feature = "performance")]
pub async fn create_snapshot_async(...) { ... }
```

**Proposed Refactoring:**

1. **Create vm_service/performance_mod.rs**
   - Move all performance-specific async methods
   - Single module-level gate
   - Export via trait-based API

2. **Create vm_service/smmu_mod.rs**
   - Consolidate all SMMU initialization
   - Move SMMU managers to submodule
   - Single module-level gate

3. **Simplify Main File**
   - Use conditional compilation for struct fields only
   - Move all conditional methods to submodules
   - Use trait objects for optional features

**Code Changes:**
```rust
// vm_service.rs - reduced to 8 gates
#[cfg(feature = "performance")]
pub mod performance_mod;

#[cfg(feature = "smmu")]
pub mod smmu_mod;

// Main struct - only 4 conditional fields
pub struct VmService {
    #[cfg(feature = "performance")]
    performance: performance_mod::PerformanceContext,

    #[cfg(feature = "smmu")]
    smmu_manager: Option<Arc<SmmuManager>>,
    // ... rest is feature-agnostic
}

// Conditional methods moved to submodules
```

**Expected Result**: 23 → 8 gates (65% reduction)

---

### Priority 2: vm-service/src/vm_service/execution.rs (21 → 5 gates)

**Current Structure:**
```rust
#[cfg(feature = "performance")]
mod jit_execution { /* 8 gates */ }

#[cfg(feature = "performance")]
mod async_execution { /* 6 gates */ }

pub struct ExecutionContext {
    #[cfg(feature = "performance")]
    pub jit_state: Option<...>,

    #[cfg(feature = "performance")]
    pub coroutine_state: Option<...>,
}
```

**Proposed Refactoring:**

1. **Extract JIT to Separate Compilation Unit**
   - Create vm-service/src/vm_service/execution/jit.rs
   - Move all JIT-specific code
   - Single gate at module level

2. **Extract Coroutine to Separate Module**
   - Create vm-service/src/vm_service/execution/coroutine.rs
   - Move all coroutine-specific code
   - Single gate at module level

3. **Use Execution Strategy Pattern**
   ```rust
   // Feature-agnostic core
   pub enum ExecutionStrategy {
       Interpreter,
       #[cfg(feature = "performance")]
       JIT(JitEngine),
       #[cfg(feature = "performance")]
       Coroutine(CoroutineScheduler),
   }
   ```

**Expected Result**: 21 → 5 gates (76% reduction)

---

### Priority 3: vm-accel/src/kvm_impl.rs (21 → 8 gates)

**Current Issues:**
- Platform-specific code mixed with feature-specific code
- KVM-specific optimizations scattered
- Hardware acceleration checks duplicated

**Proposed Refactoring:**

1. **Split by Platform**
   - Create platform/linux_x86_64.rs
   - Create platform/linux_arm64.rs
   - Move platform-specific implementations

2. **Consolidate Hardware Detection**
   - Single feature gate for hardware acceleration
   - Use runtime detection within feature
   - Extract hardware-specific optimizations

3. **Module Structure:**
   ```rust
   // kvm_impl.rs - main orchestration
   #[cfg(feature = "kvm")]
   pub mod kvm_impl {
       #[cfg(target_os = "linux")]
       use platform::LinuxKvmImpl;

       // Single gate for entire impl
   }
   ```

**Expected Result**: 21 → 8 gates (62% reduction)

---

## Week 2: High-Priority Files (67 gates → 19 gates)

### 4. vm-core/src/debugger/call_stack_tracker.rs (12 → 1 gate)

**Current State:**
- All 12 gates are `enhanced-debugging`
- Scattered throughout file

**Solution:**
```rust
// Entire file wrapped in single gate
#[cfg(feature = "enhanced-debugging")]
pub mod call_stack_tracker {
    // All code here, no internal gates
}
```

**Expected Result**: 12 → 1 gate (92% reduction)

---

### 5. vm-core/src/debugger/unified_debugger.rs (10 → 2 gates)

**Proposed Structure:**
```rust
// Core debugger (always available)
pub mod debugger_core { /* 0 gates */ }

// Enhanced features
#[cfg(feature = "enhanced-debugging")]
pub mod enhanced { /* 1 gate */ }

// Async support
#[cfg(feature = "async")]
pub mod async_debugger { /* 1 gate */ }
```

**Expected Result**: 10 → 2 gates (80% reduction)

---

### 6. vm-service/src/device_service.rs (10 → 3 gates)

**Strategy:**
1. Extract SMMU to dedicated submodule (9 → 1 gate)
2. Consolidate device feature gates
3. Use trait-based device registration

**Expected Result**: 10 → 3 gates (70% reduction)

---

### 7. vm-core/src/async_event_bus.rs (10 → 3 gates)

**Strategy:**
1. Core event bus (0 gates - always available)
2. Async implementation gated (1 gate)
3. Performance optimizations gated (1 gate)
4. Event sourcing integration gated (1 gate)

**Expected Result**: 10 → 3 gates (70% reduction)

---

### 8-10. Other High-Priority Files

**vm-device/src/smmu_device.rs** (9 → 1 gate)
- Move to vm-smmu crate
- Single module-level gate

**vm-cross-arch/src/cross_arch_runtime.rs** (9 → 3 gates)
- Extract async runtime module
- Consolidate hardware-specific code

**vm-accel/src/accel.rs** (9 → 6 gates)
- Already well-organized
- Minor consolidation

**Expected Result**: 67 → 19 gates (72% reduction)

---

## Week 3: Medium-Priority Files (45 gates → 10 gates)

### Target Files (8-15 gates each):

1. **vm-device/src/net.rs** (8 gates)
   - Consolidate protocol-specific code
   - Extract to submodules

2. **vm-core/src/event_store/file_event_store.rs** (8 gates)
   - All `enhanced-event-sourcing` feature
   - Single module-level gate

3. **vm-accel/src/kvm.rs** (8 gates)
   - Further consolidation possible
   - Merge related gates

4. **vm-accel/src/cpuinfo.rs** (8 gates)
   - Extract to platform-specific files
   - Use target_os instead of feature where possible

5. **vm-mem/src/lib.rs** (6 gates)
   - Consolidate memory management features

6. **vm-frontend/src/lib.rs** (6 gates)
   - Extract per-architecture to separate files

7. **vm-core/src/event_store/compatibility.rs** (6 gates)
   - Consolidate version compatibility gates

8. **vm-service/src/vm_service/kernel_loader.rs** (5 gates)
   - Extract async loading to submodule

**Expected Result**: 45 → 10 gates (78% reduction)

---

## Week 4: Feature Unification (Analysis & Implementation)

### Phase 1: Feature Dependency Analysis

**Create feature dependency matrix:**
```
┌─────────────────┬───────┬───────┬──────┬──────┐
│ Feature         │ async │ perf  │ kvm  │ smmu │
├─────────────────┼───────┼───────┼──────┼──────┤
│ async           │   ✓   │  50%  │  -   │  -   │
│ performance     │  50%  │   ✓   │ 30%  │  -   │
│ kvm             │   -   │  30%  │  ✓   │ 20%  │
│ smmu            │   -   │   -   │ 20%  │  ✓   │
└─────────────────┴───────┴───────┴──────┴──────┘
```

**Analysis Tasks:**
1. Map all feature gate occurrences
2. Identify overlapping functionality
3. Find opportunities for feature merging
4. Document feature dependencies

### Phase 2: Feature Consolidation

**Proposed Mergers:**

1. **async + performance → unified-execution**
   - 39 + 32 = 71 gates → ~35 gates
   - Rationale: Most async usage is for performance

2. **enhanced-debugging → debug (simplify name)**
   - 22 gates → ~10 gates
   - Rationale: Better feature organization

3. **kvm + hardware → accel**
   - 29 + 8 = 37 gates → ~20 gates
   - Rationale: Hardware acceleration is conceptually similar

**Expected Result**: 129 → 60 gates (53% reduction)

### Phase 3: Feature Re-evaluation

**Questions to Answer:**
1. Is `smmu` feature necessary or can it be default?
   - Currently 36 gates
   - If always-on: 0 gates

2. Can `enhanced-event-sourcing` be merged into core?
   - 15 gates
   - Evaluate usage patterns

3. Are `std` and `all` features still needed?
   - 10 + 6 = 16 gates
   - Consider removing or clarifying purpose

**Expected Result**: Additional 10-15 gates removed

---

## Week 4 Deliverables

1. **Feature Dependency Matrix** document
2. **Consolidated Feature Definitions** in Cargo.toml
3. **Migration Guide** for dependent projects
4. **Updated Documentation** explaining new features

---

## Implementation Guidelines

### General Principles

1. **Module-Level Over Method-Level**
   ```rust
   // Bad
   impl Foo {
       #[cfg(feature = "X")]
       fn method1() {}

       #[cfg(feature = "X")]
       fn method2() {}
   }

   // Good
   #[cfg(feature = "X")]
   mod foo_x {
       impl Foo {
           fn method1() {}
           fn method2() {}
       }
   }
   ```

2. **Traits Over Conditional Compilation**
   ```rust
   // Bad
   #[cfg(feature = "X")]
   fn process_x() {}
   #[cfg(feature = "Y")]
   fn process_y() {}

   // Good
   trait Processor {
       fn process(&self);
   }
   ```

3. **Runtime Configuration Over Compile-Time**
   ```rust
   // Bad
   #[cfg(feature = "optimized")]
   const THRESHOLD: usize = 100;
   #[cfg(not(feature = "optimized"))]
   const THRESHOLD: usize = 10;

   // Good
   const DEFAULT_THRESHOLD: usize = 100;
   fn get_threshold() -> usize {
       std::env::var("THRESHOLD")
           .ok()
           .and_then(|s| s.parse().ok())
           .unwrap_or(DEFAULT_THRESHOLD)
   }
   ```

### Testing Strategy

1. **Feature Matrix Testing**
   ```bash
   # Test all feature combinations
   cargo test --no-default-features
   cargo test --features "performance"
   cargo test --features "async"
   cargo test --features "performance,async"
   ```

2. **Incremental Validation**
   - Test after each file refactored
   - Ensure no behavior changes
   - Verify compile-time improvements

3. **Performance Validation**
   - Benchmark before/after each change
   - Ensure no runtime regression
   - Measure compile-time improvements

---

## Risk Mitigation

### Potential Issues

1. **Breaking Changes**
   - Risk: Feature removal may break downstream crates
   - Mitigation: Deprecate features before removing
   - Timeline: 2 release cycle deprecation period

2. **Compile-Time Regression**
   - Risk: Trait objects may increase compile time
   - Mitigation: Benchmark compile times
   - Fallback: Keep critical paths compile-time gated

3. **Runtime Performance**
   - Risk: Dynamic dispatch may slow hot paths
   - Mitigation: Profile before/after
   - Strategy: Keep hot paths compile-time optimized

### Rollback Plan

1. **Git Branches**
   - Create feature branch per file refactored
   - Keep original code accessible
   - Easy rollback if issues arise

2. **Feature Flags**
   - Introduce new features before removing old ones
   - Allow gradual migration
   - Remove old features after validation

---

## Success Metrics

### Quantitative Goals

| Metric | Current | Week 1 | Week 2 | Week 3 | Week 4 | Target |
|--------|---------|--------|--------|--------|--------|--------|
| Total Gates | 254 | 189 | 122 | 112 | <150 | <150 |
| Files >8 gates | 14 | 11 | 7 | 3 | 2 | <5 |
| Avg/File | 5.52 | 4.1 | 3.2 | 2.8 | <3.5 | <4 |
| Compile Time | 100% | 98% | 95% | 92% | <95% | <95% |

### Qualitative Goals

1. **Improved Code Organization**
   - Clear feature boundaries
   - Better module structure
   - Easier to navigate

2. **Reduced Cognitive Load**
   - Fewer feature combinations to test
   - Simpler feature dependencies
   - Clearer feature purpose

3. **Better Developer Experience**
   - Faster compilation
   - Easier to add new features
   - Simpler debugging

---

## Next Steps

### Immediate Actions (This Week)

1. **Start with vm-service/src/vm_service.rs**
   - Create feature branch
   - Implement submodule extraction
   - Test thoroughly
   - Document changes

2. **Set Up Testing Infrastructure**
   - Create feature matrix test suite
   - Set up compile-time benchmarks
   - Establish baseline metrics

3. **Document Patterns**
   - Create optimization guide
   - Share with team
   - Gather feedback

### Follow-Up Actions

1. **Weekly Reviews**
   - Track progress against metrics
   - Adjust plan as needed
   - Document lessons learned

2. **Continuous Integration**
   - Add feature gate tests to CI
   - Monitor gate count over time
   - Prevent gate proliferation

3. **Knowledge Sharing**
   - Present findings to team
   - Update contribution guidelines
   - Mentor developers on patterns

---

**Author**: Architecture Team
**Last Updated**: 2025-12-28
**Status**: Planning Complete, Ready to Execute
**Next Review**: End of Week 1 (2025-01-04)
