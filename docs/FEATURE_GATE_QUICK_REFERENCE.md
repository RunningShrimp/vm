# Feature Gate Quick Reference

**Last Updated**: 2025-12-28
**Current Count**: 254 gates
**Target**: <150 gates

---

## Current Status at a Glance

```
Progress:  56.7% to target ████████████░░░░░░░░ (187/291 gates removed)
Baseline:  441 gates
Current:   254 gates
Target:    <150 gates
```

---

## Top 10 Files to Optimize

| File | Gates | Quick Win |
|------|-------|-----------|
| vm-service/src/vm_service.rs | 23 | Extract submodules |
| vm-service/src/vm_service/execution.rs | 21 | Extract JIT/coroutine |
| vm-accel/src/kvm_impl.rs | 21 | Platform split |
| vm-core/src/debugger/call_stack_tracker.rs | 12 | Single gate ✓ |
| vm-service/src/device_service.rs | 10 | Extract SMMU |
| vm-core/src/debugger/unified_debugger.rs | 10 | Consolidate |
| vm-core/src/async_event_bus.rs | 10 | Trait-based |
| vm-device/src/smmu_device.rs | 9 | Single gate ✓ |
| vm-cross-arch/src/cross_arch_runtime.rs | 9 | Extract async |
| vm-accel/src/accel.rs | 9 | Minor cleanup |

---

## Quick Win Pattern

### File with 12 identical gates → 1 gate

**Before:**
```rust
#[cfg(feature = "X")]
fn foo() {}

#[cfg(feature = "X")]
fn bar() {}

#[cfg(feature = "X")]
struct Baz {}
```

**After:**
```rust
#[cfg(feature = "X")]
mod feature_x {
    fn foo() {}
    fn bar() {}
    struct Baz {}
}
```

**Result**: 12 → 1 gate (92% reduction)

---

## Feature Gate Limits

- 0-3 gates: ✅ Excellent
- 4-7 gates: ⚠️ Acceptable
- 8-10 gates: ⚠️ Needs review
- 10+ gates: ❌ Must refactor

---

## Common Patterns

### Pattern 1: Module-Level Gating
```rust
#[cfg(feature = "performance")]
pub mod performance {
    // All performance code here
    // No internal feature gates
}
```

### Pattern 2: Trait Abstraction
```rust
pub trait ExecutionEngine {
    fn execute(&self);
}

#[cfg(feature = "jit")]
pub struct JitEngine;

#[cfg(feature = "interpreter")]
pub struct Interpreter;
```

### Pattern 3: Conditional Submodules
```rust
// Core (always available)
pub mod core;

// Optional features
#[cfg(feature = "async")]
pub mod async_ops;

#[cfg(feature = "performance")]
pub mod perf;
```

---

## When NOT to Use Feature Gates

❌ Configuration options → Use runtime config
❌ Minor optimizations → Use PGO
❌ Debug logging → Use log levels
❌ Platform code → Use `target_os`

---

## Testing Commands

```bash
# Count feature gates in a file
grep -c "#\[cfg(feature" path/to/file.rs

# Find files with many gates
grep -r "#\[cfg(feature" --include="*.rs" | \
    cut -d: -f1 | sort | uniq -c | sort -rn | \
    awk '$1 > 10'

# Test specific feature
cargo test --features "performance"

# Test all features
cargo test --all-features

# Test no features
cargo test --no-default-features
```

---

## Progress Checklist

- [x] Baseline established (441 gates)
- [x] Initial optimizations (254 gates)
- [x] Documentation created
- [x] Roadmap defined
- [ ] Week 1: Critical files (3)
- [ ] Week 2: High-priority files (7)
- [ ] Week 3: Medium-priority files (10)
- [ ] Week 4: Feature unification
- [ ] Target achieved (<150 gates)

---

## Quick Actions

### Start Optimization Now
```bash
# 1. Find biggest files
grep -r "#\[cfg(feature" --include="*.rs" | \
    cut -d: -f1 | sort | uniq -c | sort -rn | head -5

# 2. Pick one file
# 3. Extract feature-specific code to submodule
# 4. Test thoroughly
# 5. Commit changes
```

### Track Progress
```bash
# Count total gates
grep -r "#\[cfg(feature" --include="*.rs" | wc -l

# Count affected files
grep -rl "#\[cfg(feature" --include="*.rs" | wc -l

# Average per file
# (Should be decreasing over time)
```

---

## Resources

- **Full Report**: [FEATURE_GATE_PROGRESS.md](../FEATURE_GATE_PROGRESS.md)
- **Optimization Plan**: [FEATURE_GATE_OPTIMIZATION_ROADMAP.md](../FEATURE_GATE_OPTIMIZATION_ROADMAP.md)
- **Best Practices**: [docs/FEATURE_GATE_BEST_PRACTICES.md](docs/FEATURE_GATE_BEST_PRACTICES.md)

---

**Next Review**: After Week 1 completion
**Maintainer**: Architecture Team
**Status**: On Track ✓
