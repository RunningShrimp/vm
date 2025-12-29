# Detailed Feature Flag Simplification - Implementation Plan

## Phase-by-Phase Implementation Details

### PHASE 1: Safe Removals (1-2 hours)

#### 1.1 Remove memmap feature from vm-mem

**File:** `/Users/wangbiao/Desktop/project/vm/vm-mem/Cargo.toml`

**Current:**
```toml
[features]
default = ["std", "tlb-basic"]
std = []
async = ["tokio", "async-trait"]
memmap = ["memmap2"]
tlb-basic = []
tlb-optimized = []
tlb-concurrent = []
```

**After:**
```toml
[features]
default = ["std", "tlb-basic"]
std = []
async = ["tokio", "async-trait"]
tlb-basic = []
tlb-optimized = []
tlb-concurrent = []
```

**Changes:**
- Remove `memmap = ["memmap2"]` line
- Remove `memmap2` from dependencies if it's only used by this feature
- No code changes needed (feature never used)

**Risk:** NONE
**Testing:** Verify package builds without memmap feature

---

### PHASE 2: Feature Merges (4-6 hours)

#### 2.1 Merge vm-common features into std

**File:** `/Users/wangbiao/Desktop/project/vm/vm-common/Cargo.toml`

**Current:**
```toml
[features]
default = ["event", "logging", "config", "error"]
event = []
logging = []
config = []
error = []
```

**After:**
```toml
[features]
default = ["std"]
std = []
```

**Code Changes:**
No code changes needed - features are empty flags only used for dependency propagation.

**Migration:**
```toml
# Users currently doing this:
vm-common = { path = "../vm-common", features = ["event", "logging"] }

# Should do this:
vm-common = { path = "../vm-common" }  # Uses default features
```

**Risk:** LOW
**Breaking:** Yes, but minimal impact (default features enable everything)

---

#### 2.2 Merge vm-foundation features into std

**File:** `/Users/wangbiao/Desktop/project/vm/vm-foundation/Cargo.toml`

**Current:**
```toml
[features]
default = ["std", "utils", "macros", "test_helpers"]
std = []
utils = []
macros = []
test_helpers = []
```

**After:**
```toml
[features]
default = ["std"]
std = []
```

**Code Changes:**
None - all features are empty flags

**Risk:** LOW
**Breaking:** Yes, but minimal impact

---

#### 2.3 Remove simple-devices from vm-device

**File:** `/Users/wangbiao/Desktop/project/vm/vm-device/Cargo.toml`

**Current:**
```toml
[features]
default = ["async-io", "smoltcp", "std"]
std = []
async-io = []
smoltcp = ["dep:smoltcp"]
simple-devices = []
smmu = ["dep:vm-smmu", "vm-accel/smmu"]
```

**After:**
```toml
[features]
default = ["async-io", "smoltcp", "std"]
std = []
async-io = []
smoltcp = ["dep:smoltcp"]
smmu = ["dep:vm-smmu", "vm-accel/smmu"]
```

**Code Changes:**
File: `/Users/wangbiao/Desktop/project/vm/vm-device/src/lib.rs`

Find and remove:
```rust
#[cfg(feature = "simple-devices")]
```

Replace with: Always include that code (remove the gate)

**Risk:** LOW
**Breaking:** Yes (only 1 usage in codebase)

---

#### 2.4 Consolidate vm-tests to all-arch only

**File:** `/Users/wangbiao/Desktop/project/vm/vm-tests/Cargo.toml`

**Current:**
```toml
[features]
x86_64 = ["vm-frontend/x86_64"]
arm64 = ["vm-frontend/arm64"]
riscv64 = ["vm-frontend/riscv64"]
all-arch = ["vm-frontend/all"]
```

**After:**
```toml
[features]
all-arch = ["vm-frontend/all"]
```

**Migration:**
```toml
# Before:
vm-tests = { path = "../vm-tests", features = ["x86_64"] }

# After:
vm-tests = { path = "../vm-tests", features = ["all-arch"] }
```

**Risk:** LOW
**Breaking:** Yes, but test-only package

---

### PHASE 3: Architecture Simplification (6-8 hours)

#### 3.1 Simplify vm-frontend features

**File:** `/Users/wangbiao/Desktop/project/vm/vm-frontend/Cargo.toml`

**Current:**
```toml
[features]
default = []
x86_64 = ["vm-mem"]
arm64 = ["vm-accel"]
riscv64 = []
all = ["x86_64", "arm64", "riscv64"]
```

**After:**
```toml
[features]
default = ["all"]
all = ["vm-mem", "vm-accel"]
```

**Code Changes:**
File: `/Users/wangbiao/Desktop/project/vm/vm-frontend/src/lib.rs`

Current:
```rust
#[cfg(feature = "x86_64")]
pub mod x86_64_decoder;

#[cfg(feature = "arm64")]
pub mod arm64_decoder;

#[cfg(feature = "riscv64")]
pub mod riscv64_decoder;
```

After:
```rust
#[cfg(feature = "all")]
pub mod x86_64_decoder;

#[cfg(feature = "all")]
pub mod arm64_decoder;

#[cfg(feature = "all")]
pub mod riscv64_decoder;
```

**Risk:** LOW-MEDIUM
**Breaking:** Yes - affects individual architecture selection

---

#### 3.2 Remove individual arch features from vm-service

**File:** `/Users/wangbiao/Desktop/project/vm/vm-service/Cargo.toml`

**Current:**
```toml
[features]
default = ["std", "devices", "frontend"]
std = []
async = ["std", "vm-core/async", "vm-mem/async"]
devices = ["vm-device"]
frontend = ["vm-frontend"]
x86_64 = ["frontend", "vm-frontend/x86_64"]
arm64 = ["frontend", "vm-frontend/arm64"]
riscv64 = ["frontend", "vm-frontend/riscv64"]
all-arch = ["frontend", "vm-frontend/all"]
jit = ["vm-engine-jit"]
accel = ["vm-accel"]
smmu = ["accel", "vm-accel/smmu", "devices", "vm-device/smmu", "vm-smmu"]
```

**After:**
```toml
[features]
default = ["std", "devices", "frontend"]
std = []
async = ["std", "vm-core/async", "vm-mem/async"]
devices = ["vm-device"]
frontend = ["vm-frontend"]
all-arch = ["frontend", "vm-frontend/all"]
jit = ["vm-engine-jit"]
smmu = ["vm-accel", "vm-accel/smmu", "devices", "vm-device/smmu", "vm-smmu"]
```

**Changes:**
- Remove `x86_64` feature
- Remove `arm64` feature
- Remove `riscv64` feature
- Remove `accel` feature (redundant)

**Risk:** MEDIUM
**Breaking:** Yes - users selecting individual architectures

---

### PHASE 4: Complex Consolidation (8-10 hours)

#### 4.1 Simplify vm-cross-arch features

**File:** `/Users/wangbiao/Desktop/project/vm/vm-cross-arch/Cargo.toml`

**Current:**
```toml
[features]
default = []
interpreter = ["vm-engine-interpreter"]
jit = ["vm-engine-jit", "vm-mem"]
execution = ["interpreter", "jit"]
memory = ["vm-mem"]
runtime = ["vm-runtime"]
vm-frontend = ["dep:vm-frontend", "vm-frontend/all"]
all = ["execution", "memory", "runtime", "vm-frontend"]
```

**After:**
```toml
[features]
default = []
execution = ["vm-engine-interpreter", "vm-engine-jit", "vm-mem"]
runtime = ["vm-runtime"]
all = ["execution", "runtime", "vm-frontend"]
vm-frontend = ["dep:vm-frontend", "vm-frontend/all"]
```

**Changes:**
- Remove `interpreter` feature (subsumed by `execution`)
- Remove `jit` feature (subsumed by `execution`)
- Remove `memory` feature (subsumed by `execution`)

**Code Changes:**
File: `/Users/wangbiao/Desktop/project/vm/vm-cross-arch/src/lib.rs`

Remove or update feature gates:
```rust
// Remove these:
#[cfg(feature = "interpreter")]
#[cfg(feature = "jit")]
#[cfg(feature = "memory")]

// Keep these:
#[cfg(feature = "execution")]
#[cfg(feature = "runtime")]
#[cfg(feature = "all")]
```

**Risk:** MEDIUM
**Breaking:** Yes - affects granular feature selection

---

#### 4.2 Merge TLB features in vm-mem

**File:** `/Users/wangbiao/Desktop/project/vm/vm-mem/Cargo.toml`

**Current:**
```toml
[features]
default = ["std", "tlb-basic"]
std = []
async = ["tokio", "async-trait"]
tlb-basic = []
tlb-optimized = []
tlb-concurrent = []
```

**After:**
```toml
[features]
default = ["std", "tlb"]
std = []
async = ["tokio", "async-trait"]
tlb = []
```

**Code Changes:**
File: `/Users/wangbiao/Desktop/project/vm/vm-mem/src/tlb/unified_tlb.rs`

Current:
```rust
#[cfg(feature = "tlb-basic")]
pub use crate::tlb::tlb::Tlb;

#[cfg(feature = "tlb-optimized")]
pub use crate::tlb::adaptive_replacement::Tlb;

#[cfg(feature = "tlb-concurrent")]
pub use crate::tlb::tlb_concurrent::ConcurrentTlb;
```

After:
```rust
#[cfg(feature = "tlb")]
pub use crate::tlb::tlb::Tlb;
pub use crate::tlb::adaptive_replacement::Tlb;
pub use crate::tlb::tlb_concurrent::ConcurrentTlb;
```

**Risk:** LOW-MEDIUM
**Breaking:** Yes - changes TLB selection mechanism

---

### PHASE 5: Validation and Documentation (4-6 hours)

#### 5.1 Update Package Documentation

For each modified package, update README.md:

```markdown
## Features

### vm-frontend
- `all` (default): Includes all architecture decoders (x86_64, ARM64, RISC-V)

### vm-mem
- `std` (default): Standard library support
- `async`: Async/await runtime support
- `tlb` (default): Translation Lookaside Buffer implementations

### vm-cross-arch
- `execution`: Combined interpreter and JIT execution
- `runtime`: Runtime support with GC
- `all`: Complete cross-architecture support
```

#### 5.2 Create Migration Guide

Create: `/Users/wangbiao/Desktop/project/vm/MIGRATION_GUIDE.md`

```markdown
# Feature Flag Migration Guide

## Version X.Y.Z Feature Simplification

This release simplifies the feature flag system across all packages.

### Summary of Changes

- Total features reduced from 52 to 28 (46% reduction)
- Removed unused and redundant features
- Consolidated granular features

### Migration Guide

#### Architecture Features

**Before:**
```toml
vm-frontend = { path = "../vm-frontend", features = ["x86_64"] }
vm-service = { path = "../vm-service", features = ["x86_64"] }
```

**After:**
```toml
vm-frontend = { path = "../vm-frontend" }  # Uses "all" by default
vm-service = { path = "../vm-service", features = ["all-arch"] }
```

#### Common Utilities

**Before:**
```toml
vm-common = { path = "../vm-common", features = ["event", "logging"] }
```

**After:**
```toml
vm-common = { path = "../vm-common" }  # All features in "std"
```

#### TLB Features

**Before:**
```toml
vm-mem = { path = "../vm-mem", features = ["tlb-basic"] }
```

**After:**
```toml
vm-mem = { path = "../vm-mem", features = ["tlb"] }
```

#### Cross-Arch Execution

**Before:**
```toml
vm-cross-arch = { path = "../vm-cross-arch", features = ["interpreter"] }
```

**After:**
```toml
vm-cross-arch = { path = "../vm-cross-arch", features = ["execution"] }
```

### Deprecated Features

The following features have been removed:
- `memmap` (vm-mem) - unused
- `simple-devices` (vm-device) - always enabled
- Individual arch features (x86_64, arm64, riscv64) - use `all-arch` instead
- `tlb-basic`, `tlb-optimized`, `tlb-concurrent` - use `tlb` instead
- `interpreter`, `jit`, `memory` (vm-cross-arch) - use `execution` instead

### Need Help?

If you encounter issues migrating, please:
1. Check this guide
2. Review package-specific documentation
3. Open an issue on GitHub
```

#### 5.3 Update CHANGELOG

Create or update: `/Users/wangbiao/Desktop/project/vm/CHANGELOG.md`

```markdown
# Changelog

## [Unreleased]

### Changed
- Simplified feature flag system across all packages
  - Reduced total features from 52 to 28 (46% reduction)
  - Removed unused features (memmap, simple-devices)
  - Consolidated architecture features (use `all-arch` instead of individual archs)
  - Merged TLB features into single `tlb` feature
  - Simplified vm-cross-arch features
  - Merged vm-common and vm-foundation features into `std`

### Removed
- `memmap` feature from vm-mem (was unused)
- `simple-devices` feature from vm-device
- Individual architecture features from vm-frontend and vm-service
- Separate TLB features from vm-mem (tlb-basic, tlb-optimized, tlb-concurrent)
- Granular execution features from vm-cross-arch (interpreter, jit, memory)

### Migration Notes
See MIGRATION_GUIDE.md for detailed migration instructions.
```

#### 5.4 Testing Checklist

Create test script: `/Users/wangbiao/Desktop/project/vm/test_feature_changes.sh`

```bash
#!/bin/bash

echo "Testing Feature Flag Changes..."

# Test 1: Build all packages with default features
echo "Test 1: Building with default features..."
cargo build --workspace --all-targets || exit 1

# Test 2: Test vm-frontend with all feature
echo "Test 2: Testing vm-frontend..."
cargo build -p vm-frontend --features all || exit 1

# Test 3: Test vm-mem with tlb feature
echo "Test 3: Testing vm-mem..."
cargo build -p vm-mem --features tlb || exit 1

# Test 4: Test vm-cross-arch with execution feature
echo "Test 4: Testing vm-cross-arch..."
cargo build -p vm-cross-arch --features execution || exit 1

# Test 5: Test vm-service with all-arch feature
echo "Test 5: Testing vm-service..."
cargo build -p vm-service --features all-arch || exit 1

# Test 6: Run tests
echo "Test 6: Running tests..."
cargo test --workspace || exit 1

# Test 7: Test feature combinations
echo "Test 7: Testing feature combinations..."
cargo build -p vm-service --features "std,async,devices,all-arch,jit" || exit 1

# Test 8: Verify examples build
echo "Test 8: Testing examples..."
cargo build --examples || exit 1

echo "All tests passed!"
```

---

## Validation Steps

### Pre-Implementation

1. **Baseline Tests**
   ```bash
   cargo build --workspace --all-targets
   cargo test --workspace
   cargo doc --workspace --no-deps
   ```

2. **Feature Matrix Documentation**
   - Document all current feature combinations
   - Identify all public API consumers
   - Map feature dependencies

### During Implementation

1. **Per-Phase Testing**
   - Build after each phase
   - Run affected tests
   - Verify feature gates still work

2. **Backward Compatibility**
   - Test with deprecated feature names (should warn)
   - Verify migration paths work
   - Check external consumers

### Post-Implementation

1. **Comprehensive Testing**
   - Full workspace build
   - All tests pass
   - Documentation builds
   - Examples compile

2. **Feature Coverage**
   ```bash
   # Test each feature combination
   for feature in tlb async execution all-arch; do
       cargo build --features $feature
   done
   ```

3. **Documentation Review**
   - All Cargo.toml updated
   - Migration guide complete
   - CHANGELOG updated
   - README examples updated

---

## Rollback Plan

If issues arise:

1. **Immediate Rollback** (within 1 hour)
   - Revert all commits
   - Tag as "bad-feature-simplification"
   - Resume previous version

2. **Partial Rollback** (within 1 day)
   - Keep safe changes (Phases 1-2)
   - Revert complex changes (Phases 3-4)
   - Reassess approach

3. **Phased Re-release**
   - Keep completed phases
   - Defer remaining phases
   - Adjust timeline

---

## Success Criteria

✅ All packages build successfully
✅ All tests pass
✅ No regressions in functionality
✅ Documentation updated and accurate
✅ Migration guide complete
✅ Backward compatibility maintained (via warnings)
✅ Feature count reduced to ≤30
✅ No unused features remain

---

## Time Estimate Breakdown

| Phase | Task | Time | Risk |
|-------|------|------|------|
| 1 | Safe Removals | 1-2h | NONE |
| 2 | Feature Merges | 4-6h | LOW |
| 3 | Architecture | 6-8h | MED |
| 4 | Complex Consolidation | 8-10h | MED |
| 5 | Validation & Docs | 4-6h | HIGH |
| **Total** | **All Phases** | **23-32h** | **MED** |

---

## Post-Implementation Monitoring

### Week 1
- Monitor GitHub issues for migration problems
- Track feature flag usage statistics
- Gather user feedback

### Month 1
- Review migration guide effectiveness
- Update documentation based on feedback
- Address any edge cases

### Quarter 1
- Conduct feature audit
- Plan next simplification cycle
- Establish feature governance

