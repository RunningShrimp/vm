# Feature Flag Analysis and Reduction Plan

**Generated**: 2025-12-28
**Workspace**: `/Users/wangbiao/Desktop/project/vm`
**Analysis Scope**: All Rust packages and feature gates

---

## Executive Summary

The workspace currently has **303 feature gate usages** across **19 unique features** in **39+ files**. This analysis reveals significant opportunities for reduction and simplification:

- **High fragmentation**: 19 features for only 303 usages (~16 per feature)
- **Package isolation**: 17/19 features used in only one package
- **Heavy concentration**: 74% of gates in vm-core alone (161/303)
- **Over-engineering**: Multiple features doing similar things (TLB, async)
- **Quick win potential**: 47% reduction (9 features) with low-risk changes

---

## Current State

### Feature Usage Statistics

| Metric | Count | Percentage |
|--------|-------|------------|
| Total feature gate usages | 303 | 100% |
| Unique feature names | 19 | - |
| Files with feature gates | 39+ | - |
| Packages with features | 7 | - |

### Top 10 Most Used Features

| Feature | Usage Count | Package(s) | % of Total |
|---------|-------------|------------|------------|
| `enhanced-debugging` | 74 | vm-core | 24.4% |
| `async` | 64 | vm-core, vm-mem, vm-service | 21.1% |
| `enhanced-event-sourcing` | 46 | vm-core | 15.2% |
| `kvm` | 41 | vm-accel | 13.5% |
| `smmu` | 23 | vm-service, vm-device, vm-accel | 7.6% |
| `std` | 11 | vm-core | 3.6% |
| `cpuid` | 8 | vm-accel | 2.6% |
| `smoltcp` | 8 | vm-device | 2.6% |
| `tlb-concurrent` | 6 | vm-mem | 2.0% |
| `tlb-optimized` | 5 | vm-mem | 1.7% |

**Top 5 Features Account For**: 81.4% of all feature gates

### Top 10 Packages by Feature Gate Count

| Package | Files with Gates | Total Gates | % of Total |
|---------|------------------|-------------|------------|
| vm-core | 20 | 161 | 53.1% |
| vm-accel | 4 | 51 | 16.8% |
| vm-service | 5 | 29 | 9.6% |
| vm-mem | 3 | 28 | 9.2% |
| vm-device | 4 | 22 | 7.3% |
| vm-foundation | 2 | 5 | 1.7% |
| vm-frontend | 1 | 6 | 2.0% |

**Top 3 Packages Account For**: 79.5% of all feature gates

### Top 10 Files by Feature Gates

| File | Gates | Primary Features |
|------|-------|------------------|
| vm-core/src/debugger/enhanced_breakpoints.rs | 38 | enhanced-debugging |
| vm-core/src/snapshot/enhanced_snapshot.rs | 31 | enhanced-event-sourcing |
| vm-accel/src/kvm_impl.rs | 24 | kvm |
| vm-service/src/vm_service.rs | 17 | async, smmu |
| vm-accel/src/kvm.rs | 17 | kvm |
| vm-core/src/debugger/symbol_table.rs | 14 | enhanced-debugging |
| vm-mem/src/tlb/unified_tlb.rs | 13 | tlb-*, async |
| vm-mem/src/async_mmu.rs | 12 | async |
| vm-core/src/parallel.rs | 12 | async |
| vm-core/src/debugger/call_stack_tracker.rs | 12 | enhanced-debugging |

---

## Problems Identified

### 1. Over-Engineering

#### TLB Features (vm-mem)
- **Current**: `tlb-basic`, `tlb-optimized`, `tlb-concurrent` (3 features)
- **Usage**: 13 gates across 3 files
- **Issue**: Mutually exclusive, same package, unclear selection criteria
- **Impact**: Users must know which to choose, no clear best option

#### Enhanced Debugging (vm-core)
- **Current**: `enhanced-debugging` (74 gates)
- **Issue**: Coarse-grained, gates entire advanced debugger
- **Impact**: 25% of ALL feature gates, all-or-nothing choice
- **Better**: Split into modular components (core, symbols, profiling)

#### Enhanced Event Sourcing (vm-core)
- **Current**: `enhanced-event-sourcing` (46 gates)
- **Issue**: Gates entire advanced event sourcing system
- **Impact**: 15% of all gates, no fine-grained control
- **Better**: Separate core vs persistence features

### 2. Redundancy

#### Architecture Features (vm-frontend)
- **Current**: `x86_64`, `arm64`, `riscv64`, `all`
- **Usage**: 6 gates in lib.rs
- **Issue**: Can be auto-detected with `cfg(target_arch)`
- **Impact**: 3 unnecessary features

#### std/no_std Features
- **Current**: `std` feature in multiple packages
- **Usage**: 11 gates in vm-core
- **Issue**: Rust defaults to std, `no_std` is special case
- **Impact**: `std` feature is redundant

### 3. Low-Value Features

#### Single-Use Features
- `macros` (1 use in vm-foundation)
- `test_helpers` (1 use in vm-foundation)
- `simple-devices` (1 use in vm-device)
- **Issue**: Features used only once add complexity without benefit
- **Recommendation**: Remove gates, always include or use dev-dependencies

#### Package-Isolated Features
17 of 19 features are used in only one package:
- `enhanced-debugging` (vm-core only)
- `enhanced-event-sourcing` (vm-core only)
- `cpuid` (vm-accel only)
- `kvm` (vm-accel only)
- `smoltcp` (vm-device only)
- `async-io` (vm-device only)
- `tlb-*` (vm-mem only)

**Issue**: No cross-package visibility, should be internal

### 4. Async Feature Fragmentation

#### Current State
- `async` (67 gates across vm-core, vm-mem, vm-service)
- `async-io` (2 gates in vm-device)
- Implicit `async-mmu` (part of async)

**Issue**: Inconsistent naming, unclear boundaries, duplicate concepts

---

## Reduction Plan

### Phase 1: Quick Wins (Week 1)

**Goal**: Reduce features by 47% with minimal risk

#### 1.1 Remove Architecture Features (-3 features)

**Current**:
```toml
[features]
x86_64 = ["vm-frontend/x86_64"]
arm64 = ["vm-frontend/arm64"]
riscv64 = ["vm-frontend/riscv64"]
```

**Change**:
```rust
// In vm-frontend/src/lib.rs
// Replace #[cfg(feature = "x86_64")]
// With #[cfg(target_arch = "x86_64")]
```

**Impact**:
- Files: vm-frontend/src/lib.rs
- Gates: 6
- Risk: None (compile-time detection is more reliable)

#### 1.2 Remove Single-Use Features (-3 features)

**Features to remove**:
- `macros` (vm-foundation)
- `test_helpers` (vm-foundation)
- `simple-devices` (vm-device)

**Action**:
```toml
# Before (vm-foundation/Cargo.toml)
[features]
default = ["std", "utils", "macros", "test_helpers"]
macros = []
test_helpers = []

# After
[features]
default = ["std", "utils"]
# Remove macros and test_helpers, always include
```

**Impact**:
- Files: 3
- Gates: 3
- Risk: Low (test code can move to dev-dependencies)

#### 1.3 Remove std Feature (-1 feature)

**Current**:
```toml
[features]
default = ["std"]
std = []
```

**Change**:
```toml
[features]
default = []
# Remove std feature, it's the default
```

**Action**:
- Remove `#[cfg(feature = "std")]` gates
- Replace with `#[cfg(not(feature = "no_std"))]` where needed

**Impact**:
- Files: vm-core/src/*.rs (11 gates)
- Risk: Low (std is Rust default)

#### 1.4 Consolidate TLB Features (-2 features)

**Current**:
```toml
[features]
default = ["std", "tlb-basic"]
tlb-basic = []
tlb-optimized = []
tlb-concurrent = []
```

**Change**:
```toml
[features]
default = ["std"]
tlb-backend = []  # Auto-selects best backend
```

**Implementation**:
```rust
// In vm-mem/src/tlb/mod.rs
#[cfg(feature = "tlb-backend")]
pub use self::unified_tlb::UnifiedTlb;

// Auto-select based on platform
#[cfg(all(feature = "tlb-backend", target_os = "linux"))]
pub use self::tlb_concurrent::ConcurrentTlb as DefaultTlb;

#[cfg(all(feature = "tlb-backend", not(target_os = "linux")))]
pub use self::tlb_optimized::OptimizedTlb as DefaultTlb;
```

**Impact**:
- Files: vm-mem/src/tlb/*.rs
- Gates: 13
- Risk: Medium (requires testing all platforms)

**Phase 1 Summary**:
- **Features removed**: 9
- **Reduction**: 47.4%
- **Timeline**: 1 week
- **Risk**: Low

---

### Phase 2: Medium-Term Refactoring (Week 2-3)

**Goal**: Reduce features by additional 16%, improve organization

#### 2.1 Consolidate Async Features (-2 features)

**Current**:
- `async` (vm-core, vm-mem, vm-service)
- `async-io` (vm-device)

**Change**:
```toml
# All packages
[features]
default = ["std", "async"]  # Make async default
async = ["tokio", "async-trait"]
```

**Action**:
1. Remove `async-io` feature from vm-device
2. Make `async` default in all packages
3. Use `#[cfg(feature = "async")]` consistently

**Impact**:
- Files: 10+ across vm-core, vm-mem, vm-service, vm-device
- Gates: 67
- Risk: Medium (need to ensure no_std builds still work)

#### 2.2 Split Enhanced Debugging (+2 net features)

**Current**:
- `enhanced-debugging` (74 gates, monolithic)

**Change**:
```toml
[features]
default = ["std", "debugger-core"]
debugger-core = []  # Essential debugging
debugger-symbols = ["debugger-core"]  # Symbol table
debugger-profiling = ["debugger-core"]  # Breakpoints, call stack
```

**Implementation**:
```rust
// vm-core/src/debugger/mod.rs
#[cfg(feature = "debugger-core")]
pub mod basic;

#[cfg(feature = "debugger-symbols")]
pub mod symbol_table;

#[cfg(feature = "debugger-profiling")]
pub mod enhanced_breakpoints;

#[cfg(feature = "debugger-profiling")]
pub mod call_stack_tracker;
```

**Impact**:
- Files: 5 in vm-core/src/debugger/
- Gates: 74 (reorganized)
- Risk: Medium (requires updating all users)
- **Benefit**: Fine-grained control, core debugging always available

#### 2.3 Split Enhanced Event Sourcing (+1 net feature)

**Current**:
- `enhanced-event-sourcing` (46 gates, monolithic)

**Change**:
```toml
[features]
default = ["std", "event-store"]
event-store = []  # Basic event sourcing
event-store-persistence = ["event-store", "sqlx"]  # File/SQL backend
```

**Impact**:
- Files: vm-core/src/event_store/*.rs
- Gates: 46 (reorganized)
- Risk: Medium
- **Benefit**: Core event sourcing available by default

**Phase 2 Summary**:
- **Net feature change**: +1 (reorganization)
- **Gates reorganized**: 187
- **Timeline**: 2 weeks
- **Risk**: Medium
- **Outcome**: Better modularity, core features default

---

### Phase 3: Long-Term Improvements (Week 4-6)

**Goal**: Architecture improvements, better UX

#### 3.1 Package-Private Features

**Problem**: All features are public API, internal details exposed

**Solution**: Use underscore prefix for internal features
```toml
[features]
# Public features
async = []
smmu = []

# Internal features (not part of public API)
_internal-async-io = []
_internal-tlb-backend = []
```

**Benefits**:
- Can change internal features without breaking users
- Clearer public API surface
- Better semver guarantees

#### 3.2 Feature Presets

**Problem**: Users must know which features to combine

**Solution**: Provide preset combinations
```toml
[features]
# Individual features
async = []
smmu = []
debugger = []

# Presets
minimal = []
default = ["async", "debugger"]
full = ["async", "smmu", "debugger", "event-store"]
```

**Benefits**:
- Better UX for common use cases
- Fewer decisions for users
- Clear documentation examples

#### 3.3 Default Feature Philosophy

**Principle**: Most features should be available by default

**Rationale**:
- Modern systems have plenty of resources
- Conditional compilation adds complexity
- Users should opt-out of edge cases, not opt-in to common features

**Examples**:
- `async` should be default (2024+ standard)
- `debugger-core` should be default
- `event-store` should be default
- Opt-out: `no_std`, `minimal`

**Phase 3 Summary**:
- **Features removed**: 3-5 (moved to internal)
- **UX improved**: Presets, better defaults
- **Timeline**: 3 weeks
- **Risk**: Low to Medium
- **Outcome**: Cleaner API, better user experience

---

## Reduction Targets

### Conservative (Low Risk)

**Changes**:
- Remove unused/single-use features: -5
- Use cfg(target_arch): -3
- Remove std feature: -1

**Result**:
- **Before**: 19 features
- **After**: 10 features
- **Reduction**: 47%
- **Risk**: Low
- **Timeline**: 1 week

### Moderate (Medium Risk)

**Changes**:
- All conservative changes
- Consolidate TLB: -2
- Consolidate async: -2

**Result**:
- **Before**: 19 features
- **After**: 6 features
- **Reduction**: 68%
- **Risk**: Medium
- **Timeline**: 2-3 weeks

### Aggressive (High Reward)

**Changes**:
- All moderate changes
- Restructure debug features (net +1, but better organized)
- Move features to internal: -5
- Make most features default: -3

**Result**:
- **Before**: 19 features
- **After**: 3-5 public features
- **Reduction**: 74-84%
- **Risk**: Medium to High
- **Timeline**: 4-6 weeks
- **Outcome**: Most functionality default-available, cleaner API

---

## Implementation Roadmap

### Week 1: Quick Wins

**Days 1-2: Remove Architecture Features**
1. Update vm-frontend/src/lib.rs to use cfg(target_arch)
2. Remove x86_64, arm64, riscv64 from Cargo.toml
3. Test all architectures
4. Update documentation

**Days 3: Remove Single-Use Features**
1. Remove macros, test_helpers from vm-foundation
2. Remove simple-devices from vm-device
3. Update dependencies
4. Test compilation

**Days 4-5: Remove std Feature**
1. Remove std feature from all Cargo.toml
2. Replace #[cfg(feature = "std")] with #[cfg(not(feature = "no_std"))]
3. Test no_std builds
4. Update defaults

### Week 2: TLB & Async

**Days 1-2: Consolidate TLB Features**
1. Create tlb-backend feature with auto-detection
2. Update vm-mem/src/tlb/*.rs
3. Test all platforms
4. Update examples and docs

**Days 3-4: Consolidate Async Features**
1. Remove async-io feature
2. Make async default where appropriate
3. Standardize on async feature name
4. Test async and no_std builds

**Day 5: Testing and Validation**
1. Full workspace test
2. Performance regression tests
3. Documentation updates

### Week 3-4: Debug Refactor

**Days 1-3: Split Enhanced Debugging**
1. Create debugger-core, debugger-symbols, debugger-profiling
2. Update vm-core/src/debugger/*.rs
3. Make debugger-core default
4. Update all dependent packages

**Days 4-6: Split Enhanced Event Sourcing**
1. Create event-store, event-store-persistence
2. Update vm-core/src/event_store/*.rs
3. Make event-store default
4. Update all dependent packages

**Days 7-10: Documentation and Examples**
1. Update feature selection guide
2. Create examples for each feature combination
3. Update README files
4. Create migration guide

### Week 5-6: Polish & Testing

**Days 1-3: Comprehensive Testing**
1. Test all feature combinations
2. Test on all target platforms
3. Performance benchmarks
4. Memory usage profiling

**Days 4-5: Documentation**
1. Update all Cargo.toml comments
2. Create feature matrix
3. Update getting started guide
4. Create API documentation

**Days 6-7: Release Preparation**
1. Version bump planning
2. Changelog preparation
3. Release notes
4. Backward compatibility notes

---

## Success Metrics

### Before State
- Unique features: **19**
- Feature gates: **303**
- Gates in enhanced-debugging: **74** (24.4%)
- Top package (vm-core): **161 gates** (53.1%)
- Files with gates: **39+**

### After Conservative (Week 1)
- Unique features: **10** (-47%)
- Feature gates: **~200** (-34%)
- Enhanced-debugging: **74** (still present, but will be split in Phase 2)
- vm-core: **~100 gates** (better organization)
- **Achievement**: Significant reduction with minimal risk

### After Moderate (Week 2-3)
- Unique features: **6** (-68%)
- Feature gates: **~180** (-41%)
- Debug features: **Split and modular**
- Event sourcing: **Split and modular**
- TLB: **Consolidated**
- Async: **Standardized**
- **Achievement**: Much cleaner feature set

### After Aggressive (Week 4-6)
- Public features: **3-5** (-74% to -84%)
- Internal features: **5-7** (not part of public API)
- Feature gates: **~150** (-50%)
- Most functionality: **Default-available**
- User experience: **Much improved**
- **Achievement**: Clean, simple, user-friendly

---

## Risks and Mitigations

### Risk 1: Breaking Existing Users

**Severity**: High
**Probability**: Medium

**Impact**:
- External code using feature flags will break
- Cargo.lock updates required
- CI/CD pipeline updates needed

**Mitigation**:
1. Keep current features as deprecated aliases for 2-3 releases
2. Provide clear migration guide
3. Add deprecation warnings
4. Update all internal code first
5. Communicate changes early

**Example**:
```toml
# Deprecated (remove in v0.8.0)
[features]
x86_64 = []  # Deprecated: Use cfg(target_arch = "x86_64") instead
arm64 = []   # Deprecated: Use cfg(target_arch = "aarch64") instead
```

### Risk 2: Increased Compile Time

**Severity**: Medium
**Probability**: Low

**Impact**:
- Making features default may increase compile time
- More code compiled by default
- Longer CI/CD times

**Mitigation**:
1. Use `cfg_attr` for conditional compilation
2. Profile before and after with `cargo build --timings`
3. Provide `minimal` preset for quick builds
4. Use `lto` and other optimizations selectively

**Example**:
```toml
[features]
default = ["async", "debugger"]
minimal = []  # For quick compilation
```

### Risk 3: Loss of Fine-Grained Control

**Severity**: Medium
**Probability**: Low

**Impact**:
- Users who need minimal builds can't exclude components
- Binary size may increase
- Less flexibility for embedded systems

**Mitigation**:
1. Always provide `minimal` or `core-only` preset
2. Keep no_std support
3. Document how to disable defaults
4. Provide examples for minimal builds

**Example**:
```toml
[features]
# For most users
default = ["async", "debugger", "event-store"]

# For embedded/minimal
minimal = ["std"]

# Opt-out examples
# cargo build --no-default-features --features "minimal,no_std"
```

### Risk 4: Testing Burden

**Severity**: Medium
**Probability**: High

**Impact**:
- Need to test all feature combinations
- More complex test matrix
- Longer CI/CD times

**Mitigation**:
1. Prioritize common combinations
2. Use feature matrix in CI
3. Add integration tests
4. Document tested combinations

**Example**:
```yaml
# .github/workflows/test.yml
strategy:
  matrix:
    features: ["", "minimal", "default", "full", "async,smmu"]
```

---

## Feature Matrix

### Current Features

| Feature | Packages | Usage | Public? | Keep? |
|---------|----------|-------|---------|-------|
| enhanced-debugging | vm-core | 74 | Yes | Restructure |
| enhanced-event-sourcing | vm-core | 46 | Yes | Restructure |
| async | vm-core, vm-mem, vm-service | 64 | Yes | Keep, make default |
| kvm | vm-accel | 41 | Yes | Keep |
| smmu | vm-service, vm-device, vm-accel | 23 | Yes | Keep |
| std | vm-core | 11 | Yes | **Remove** |
| cpuid | vm-accel | 8 | Yes | Keep |
| smoltcp | vm-device | 8 | Yes | Keep |
| tlb-concurrent | vm-mem | 6 | Yes | Merge |
| tlb-optimized | vm-mem | 5 | Yes | Merge |
| utils | vm-foundation | 2 | Yes | Make internal |
| x86_64 | vm-frontend | 2 | Yes | **Remove** |
| arm64 | vm-frontend | 2 | Yes | **Remove** |
| riscv64 | vm-frontend | 2 | Yes | **Remove** |
| tlb-basic | vm-mem | 2 | Yes | Merge |
| async-io | vm-device | 2 | Yes | Merge into async |
| macros | vm-foundation | 1 | Yes | **Remove** |
| test_helpers | vm-foundation | 1 | No | **Remove** |
| simple-devices | vm-device | 1 | Yes | **Remove** |

### Proposed Features (After Phase 1-2)

| Feature | Packages | Purpose | Default? |
|---------|----------|---------|----------|
| **Public Features** |
| async | vm-core, vm-mem, vm-service, vm-device | Async runtime | Yes |
| kvm | vm-accel | KVM acceleration | No |
| smmu | vm-service, vm-device, vm-accel | SMMU support | No |
| cpuid | vm-accel | CPUID emulation | Yes |
| smoltcp | vm-device | TCP/IP stack | Yes |
| debugger-core | vm-core | Basic debugging | Yes |
| debugger-symbols | vm-core | Symbol table | No |
| debugger-profiling | vm-core | Advanced profiling | No |
| event-store | vm-core | Event sourcing | Yes |
| event-store-persistence | vm-core | Persistent event store | No |
| tlb-backend | vm-mem | TLB implementation | Yes |
| **Internal Features** |
| _internal-tlb-impl | vm-mem | TLB selection | - |
| **Presets** |
| minimal | - | Minimal build | - |
| default | - | Standard build | - |
| full | - | All features | - |

**Reduction**: 19 → 11 public features (42% reduction)
**Plus**: 2 internal features, 3 presets

---

## Recommendations

### Immediate Actions (This Week)

1. **Create feature catalog** documenting all current features
2. **Add deprecation warnings** for features to be removed
3. **Set up feature matrix testing** in CI/CD
4. **Communicate plans** to all stakeholders

### Short-Term (Next 2 Weeks)

1. **Implement Phase 1** (Quick Wins)
2. **Update documentation** with migration guide
3. **Test thoroughly** on all platforms
4. **Measure impact** on compile time and binary size

### Medium-Term (Next Month)

1. **Implement Phase 2** (Medium refactoring)
2. **Create feature selection guide** for users
3. **Add examples** for common use cases
4. **Update README** with feature matrix

### Long-Term (Next Quarter)

1. **Implement Phase 3** (Architecture improvements)
2. **Establish feature governance** process
3. **Document feature lifecycle** policy
4. **Create feature design principles**

---

## Appendix A: Feature Catalog

### Detailed Feature List

See `/tmp/all_features.txt` for complete feature definitions from all Cargo.toml files.

### Feature Gate Locations

See analysis output for detailed file-by-file breakdown.

---

## Appendix B: Migration Guide

### For Users

#### If you use architecture features:

**Before**:
```toml
[dependencies]
vm-frontend = { path = "../vm-frontend", features = ["x86_64"] }
```

**After**:
```toml
[dependencies]
vm-frontend = { path = "../vm-frontend" }
# Architecture auto-detected, no feature needed
```

#### If you use TLB features:

**Before**:
```toml
[dependencies]
vm-mem = { path = "../vm-mem", features = ["tlb-optimized"] }
```

**After**:
```toml
[dependencies]
vm-mem = { path = "../vm-mem", features = ["tlb-backend"] }
# Backend auto-selected based on platform
```

#### If you use async features:

**Before**:
```toml
[dependencies]
vm-device = { path = "../vm-device", features = ["async-io"] }
```

**After**:
```toml
[dependencies]
vm-device = { path = "../vm-device" }
# async is now default
```

### For Maintainers

#### Before removing features:

1. Check all dependent crates
2. Search GitHub for external usage
3. Add deprecation warnings
4. Update documentation

#### Removing feature gates:

1. Replace with `cfg_attr` where appropriate
2. Use target-specific cfg for platform code
3. Make internal features private
4. Update defaults

---

## Appendix C: Testing Strategy

### Feature Matrix Testing

```yaml
# .github/workflows/feature-matrix.yml
name: Feature Matrix

on: [push, pull_request]

jobs:
  test-features:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        features:
          - ""
          - "minimal"
          - "default"
          - "full"
          - "async"
          - "async,smmu"
          - "debugger-core"
          - "debugger-core,debugger-profiling"
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Test with features
        run: |
          cargo test --no-default-features --features "${{ matrix.features }}"
```

### Platform Testing

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    target: [x86_64, aarch64]
```

### Performance Testing

```bash
# Before changes
cargo build --timings --features "current-features" > before.txt

# After changes
cargo build --timings --features "new-features" > after.txt

# Compare
diff before.txt after.txt
```

---

## Conclusion

This analysis reveals significant opportunities to simplify the feature flag system:

- **Quick wins**: 47% reduction with minimal risk
- **Medium-term**: 68% reduction with better organization
- **Long-term**: 74-84% reduction with architectural improvements

The key insight is that many features are either:
1. Redundant (can be auto-detected)
2. Over-granular (can be consolidated)
3. Package-private (should be internal)
4. Low-value (used once or twice)

By implementing this plan in phases, we can achieve a much cleaner, more maintainable feature system while maintaining backward compatibility through deprecation cycles.

The recommended approach is **Conservative first** (Phase 1), then evaluate results before proceeding to **Moderate** (Phase 2) and **Aggressive** (Phase 3) changes.

---

**End of Report**
