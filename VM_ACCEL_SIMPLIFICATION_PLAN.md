# vm-accel Simplification Plan

**Date**: 2026-01-06
**Task**: P1 #2 - Simplify vm-accel conditional compilation
**Goal**: Reduce 30-40% code duplication, eliminate repetitive stubs
**Current Status**: Analysis complete

---

## Executive Summary

**Current State**: vm-accel has significant conditional compilation complexity
- **397** `#[cfg(...)]` directives across all source files
- **Top files**: kvm_impl.rs (78 cfg), hvf_impl.rs (65 cfg)
- **Total lines**: 14,330 lines across 26 Rust files
- **Complexity**: High - architecture-specific code interleaved with platform-specific code

**Target State**: Simplified architecture
- **30-40%** code reduction target
- Eliminate repetitive stub implementations
- Unify cross-platform patterns
- Improve maintainability

---

## Current Architecture Analysis

### File Structure

**Platform-Specific Implementations**:
```
vm-accel/src/
├── kvm_impl.rs         (1,691 lines, 78 cfg) - Linux KVM
├── kvm.rs              (658 lines, 20 cfg)
├── hvf_impl.rs         (923 lines, 65 cfg) - macOS HVF
├── hvf.rs              (517 lines, 42 cfg)
├── whpx_impl.rs        (589 lines, 32 cfg) - Windows WHPX
├── whpx.rs             (588 lines, 32 cfg)
├── vz_impl.rs          (517 lines, 14 cfg) - iOS VZ
└── accel_fallback.rs   (fallback implementation)
```

**Supporting Modules**:
```
├── accel.rs             (571 lines) - Unified Accel trait
├── lib.rs               (902 lines, 34 cfg) - Public API
├── cpuinfo.rs           (547 lines, 25 cfg) - CPU feature detection
├── platform_detect.rs   (253 lines, 13 cfg)
├── numa_optimizer.rs    (668 lines)
├── perf_optimizer.rs    (526 lines)
└── ... (other support modules)
```

### Conditional Compilation Breakdown

**By Type**:
- Platform detection: `target_os = "linux/macos/windows"`
- Architecture detection: `target_arch = "x86_64/aarch64"`
- Feature gates: `feature = "kvm/hvf/whpx/smmu"`
- Combined conditions: `#[cfg(all(feature = "kvm", target_arch = "x86_64"))]`

**By File**:
| File | Lines | cfg directives | cfg/100 lines |
|------|-------|-----------------|---------------|
| kvm_impl.rs | 1,691 | 78 | 4.6 |
| hvf_impl.rs | 923 | 65 | 7.0 |
| hvf.rs | 517 | 42 | 8.1 |
| lib.rs | 902 | 34 | 3.8 |
| whpx.rs | 588 | 32 | 5.4 |
| whpx_impl.rs | 589 | 32 | 5.4 |

**Highest complexity**: hvf.rs (8.1 cfg directives per 100 lines)

---

## Problem Identification

### 1. Repetitive Architecture-Specific Stubs

**Example**: kvm_impl.rs structure
```rust
#[cfg(all(feature = "kvm", target_arch = "x86_64"))]
mod kvm_x86_64 {
    pub struct KvmVcpuX86_64 { /* 50 lines */ }
    impl KvmVcpuX86_64 { /* 100 lines */ }
}

#[cfg(all(feature = "kvm", target_arch = "aarch64"))]
mod kvm_aarch64 {
    pub struct KvmVcpuArm64 { /* 50 lines */ }
    impl KvmVcpuArm64 { /* 100 lines */ }
}

#[cfg(not(feature = "kvm"))]
mod kvm_stub {
    pub struct KvmVcpuStub { /* 20 lines */ }
    impl KvmVcpuStub { /* 30 lines */ }
}
```

**Duplication**: Similar patterns repeated in hvf_impl.rs, whpx_impl.rs

### 2. FFI Declarations Scattered

**Current**: Each platform file declares its own FFI bindings

kvm_impl.rs:
```rust
#[cfg(feature = "kvm")]
mod kvm_x86_64 {
    use kvm_bindings::*;
    use kvm_ioctls::*;
    // ... implementation
}
```

hvf_impl.rs:
```rust
#[cfg(target_os = "macos")]
#[link(name = "Hypervisor", kind = "framework")]
unsafe extern "C" {
    fn hv_vm_create(...) -> i32;
    // ... 30+ FFI declarations
}
```

whpx_impl.rs:
```rust
#[cfg(target_os = "windows")]
// ... Win32 FFI declarations
```

**Problem**: FFI declarations duplicated across platform modules

### 3. Register Access Patterns

**Current**: Architecture-specific register handling in each platform

kvm_x86_64:
```rust
pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
    let regs = self.fd.get_regs()?;
    let mut gpr = [0u64; 32];
    gpr[0] = regs.rax;
    gpr[1] = regs.rcx;
    // ... 16 register assignments
}
```

hvf_x86_64:
```rust
pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
    let mut regs = GuestRegs::default();
    hv_vcpu_read_register(self.id, HV_X86_RAX, &mut regs.gpr[0])?;
    // ... similar 16 register reads
}
```

**Problem**: Same logical operations, different APIs

### 4. vCPU Creation Duplication

**Pattern across KVM, HVF, WHPX, VZ**:
```rust
// KVM
pub fn new(vm: &VmFd, id: u32) -> Result<Self, AccelError> {
    let vcpu = vm.create_vcpu(id as u64)?;
    // ... error handling
}

// HVF
pub fn new(vm: &HvVm, id: u32) -> Result<Self, AccelError> {
    let vcpu_id = unsafe { hv_vcpu_create(&mut id) }?;
    // ... error handling
}

// WHPX
pub fn new(partition: &WhpPartition, id: u32) -> Result<Self, AccelError> {
    let vcpu = WhpVcpu::new(partition, id)?;
    // ... error handling
}
```

**Problem**: Same creation pattern, 4 different implementations

---

## Refactoring Strategy

### Phase 1: Extract Common Traits (30% reduction potential)

**Goal**: Unify architecture-agnostic operations

**Create**: `src/vcpu_common.rs`
```rust
/// Common vCPU operations independent of platform
pub trait VcpuOps {
    fn get_id(&self) -> u32;
    fn run(&mut self) -> Result<VcpuExit, AccelError>;
    fn get_regs(&self) -> Result<GuestRegs, AccelError>;
    fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError>;
    fn get_fpu_regs(&self) -> Result<FpuRegs, AccelError>;
    fn set_fpu_regs(&mut self, regs: &FpuRegs) -> Result<(), AccelError>;
}

/// Register conversion utilities
pub mod reg_convert {
    /// Convert platform-specific register state to GuestRegs
    pub trait ToGuestRegs {
        fn to_guest_regs(&self) -> Result<GuestRegs, AccelError>;
    }

    /// Convert GuestRegs to platform-specific format
    pub trait FromGuestRegs {
        fn from_guest_regs(regs: &GuestRegs) -> Result<Self, AccelError>
        where
            Self: Sized;
    }
}
```

**Impact**: Eliminates ~400 lines of duplicated register handling

### Phase 2: Platform Abstraction Layer (20% reduction potential)

**Goal**: Unify platform initialization patterns

**Create**: `src/platform/mod.rs`
```rust
/// Unified platform backend
pub enum PlatformBackend {
    Kvm(kvm_impl::KvmBackend),
    Hvf(hvf_impl::HvfBackend),
    Whpx(whpx_impl::WhpxBackend),
    Vz(vz_impl::VzBackend),
    Fallback(accel_fallback::FallbackBackend),
}

impl PlatformBackend {
    pub fn new(kind: AccelKind) -> Result<Self, AccelError> {
        match kind {
            AccelKind::Kvm => Ok(PlatformBackend::Kvm(kvm_impl::KvmBackend::new()?)),
            AccelKind::Hvf => Ok(PlatformBackend::Hvf(hvf_impl::HvfBackend::new()?)),
            // ... unified initialization
        }
    }

    // Unified vCPU creation
    pub fn create_vcpu(&mut self, id: u32) -> Result<Box<dyn VcpuOps>, AccelError> {
        match self {
            PlatformBackend::Kvm(backend) => backend.create_vcpu(id),
            PlatformBackend::Hvf(backend) => backend.create_vcpu(id),
            // ... single implementation
        }
    }
}
```

**Impact**: Eliminates ~300 lines of duplicated initialization code

### Phase 3: FFI Consolidation (15% reduction potential)

**Goal**: Centralize FFI declarations

**Create**: `src/ffi/` directory
```
src/ffi/
├── mod.rs              (FFI module root)
├── kvm.rs              (KVM FFI bindings)
├── hvf.rs              (HVF FFI bindings)
├── whpx.rs             (WHPX FFI bindings)
├── vz.rs               (VZ FFI bindings)
└── stub.rs             (Stub implementations for unsupported platforms)
```

**Example**: `src/ffi/kvm.rs`
```rust
//! KVM FFI bindings
//!
//! Consolidated KVM system call bindings

#[cfg(feature = "kvm")]
pub use kvm_ioctls::*;
#[cfg(feature = "kvm")]
pub use kvm_bindings::*;

// Re-export commonly used types
#[cfg(feature = "kvm")]
pub type KvmFd = std::os::unix::io::RawFd;

#[cfg(feature = "kvm")]
#[inline]
pub fn check_kvm_version() -> Result<(), AccelError> {
    let kvm = Kvm::new()?;
    // ... version check
}
```

**Impact**: Eliminates ~200 lines of scattered FFI declarations

### Phase 4: Macro-Based Code Generation (25% reduction potential)

**Goal**: Use declarative macros to generate repetitive patterns

**Create**: `src/macros.rs`
```rust
/// Macro to generate register accessors
#[macro_export]
macro_rules! impl_reg_accessors {
    (
        $platform_type:ident,
        $get_reg_fn:ident,
        $set_reg_fn:ident,
        $reg_map:expr
    ) => {
        impl VcpuOps for $platform_type {
            fn get_regs(&self) -> Result<GuestRegs, AccelError> {
                // Generated register access logic
                let mut regs = GuestRegs::default();

                // Expand register map
                $(
                    regs.gpr[$reg_map.index] = unsafe {
                        let mut val = 0u64;
                        $get_reg_fn(self.id, $reg_map.kvm_reg, &mut val)?;
                        val
                    };
                )*

                Ok(regs)
            }

            fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
                // Generated register setting logic
                $(
                    unsafe {
                        $set_reg_fn(self.id, $reg_map.kvm_reg, regs.gpr[$reg_map.index])?;
                    }
                )*

                Ok(())
            }
        }
    };
}

/// Register mapping definition
macro_rules! reg_map {
    ($($idx:expr => $kvm_reg:ident),* $(,)?) => {
        &[$(RegMapping {
            index: $idx,
            kvm_reg: kvm_bindings::$kvm_reg,
        }),*]
    };
}
```

**Usage**:
```rust
// In kvm_x86_64 module
impl_reg_accessors!(
    KvmVcpuX86_64,
    kvm_get_reg,
    kvm_set_reg,
    reg_map!(
        0 => RAX,
        1 => RCX,
        2 => RDX,
        // ... all 16 GPRs
    )
);
```

**Impact**: Reduces ~500 lines of repetitive code to macro invocations

---

## Implementation Plan

### Step 1: Create Common Abstractions (2-3 days)

1. **Create vcpu_common.rs** with VcpuOps trait
2. **Create reg_convert.rs** with conversion utilities
3. **Create platform/mod.rs** with PlatformBackend enum
4. **Update existing implementations** to use new abstractions

### Step 2: Consolidate FFI (1 day)

1. **Create src/ffi/** directory structure
2. **Move FFI bindings** from impl files to dedicated FFI modules
3. **Update imports** across all platform implementations
4. **Test FFI access** on all platforms

### Step 3: Implement Macros (1-2 days)

1. **Create macros.rs** with code generation macros
2. **Define register mappings** for each architecture
3. **Replace repetitive code** with macro invocations
4. **Verify generated code** matches original functionality

### Step 4: Remove Dead Code (0.5 day)

1. **Identify unused stubs** after unification
2. **Remove redundant implementations**
3. **Clean up unused imports**
4. **Verify all platforms** still compile

### Step 5: Testing (1 day)

1. **Unit tests** for common abstractions
2. **Integration tests** for each platform backend
3. **Performance benchmarks** to ensure no regression
4. **Cross-platform testing** (Linux, macOS, Windows)

---

## Success Metrics

### Quantitative Goals

| Metric | Current | Target | Improvement |
|--------|---------|--------|-------------|
| **Total Lines** | 14,330 | ~9,000 | 37% reduction |
| **cfg directives** | 397 | ~150 | 62% reduction |
| **Duplicate code** | ~3,000 | <500 | 83% reduction |
| **Files** | 26 | 30 | +4 (new abstractions) |

### Qualitative Goals

- ✅ Unified vCPU operations across platforms
- ✅ Centralized FFI bindings
- ✅ Declarative register mappings
- ✅ Easier to add new platform support
- ✅ Better testability (mock common traits)

### Validation

- **Compilation**: All platforms build without errors
- **Tests**: All existing tests pass
- **Performance**: No regression in vCPU operations (<5% overhead acceptable)
- **Code Review**: Reviewed by platform team

---

## Risk Mitigation

### Risk 1: Performance Regression

**Mitigation**:
- Benchmark before/after refactoring
- Use inline attributes on critical paths
- Profile vCPU operations after changes

### Risk 2: Platform-Specific Bugs

**Mitigation**:
- Comprehensive integration testing
- Platform-specific test suites
- Incremental rollout (test on Linux first, then others)

### Risk 3: Increased Complexity

**Mitigation**:
- Keep macro code simple and well-documented
- Avoid over-abstraction
- Regular code reviews during implementation

---

## Estimated Effort

| Phase | Duration | Complexity | Risk |
|-------|----------|------------|------|
| **Phase 1: Common Abstractions** | 2-3 days | Medium | Low |
| **Phase 2: FFI Consolidation** | 1 day | Low | Low |
| **Phase 3: Macros** | 1-2 days | High | Medium |
| **Phase 4: Cleanup** | 0.5 day | Low | Low |
| **Phase 5: Testing** | 1 day | Medium | Low |
| **Total** | **5.5-7.5 days** | - | **Low-Medium** |

**Matches review report estimate**: 5-7 days ✅

---

## Rollout Plan

### Incremental Approach

**Iteration 1** (Day 1-2):
- Create vcpu_common.rs with VcpuOps trait
- Implement for KVM only
- Test thoroughly

**Iteration 2** (Day 3-4):
- Extend to HVF and WHPX
- Fix any cross-platform issues
- Add comprehensive tests

**Iteration 3** (Day 5-6):
- Consolidate FFI bindings
- Implement macros for register access
- Performance testing

**Iteration 4** (Day 7):
- Remove dead code
- Final testing
- Documentation updates

---

## Next Steps

### Immediate Actions

1. ✅ **Analysis complete** - This document
2. **Create feature branch**: `git checkout -b refactor/vm-accel-simplify`
3. **Start Phase 1**: Create vcpu_common.rs
4. **Set up benchmarks**: Before refactoring baseline

### Success Criteria

From review report:
> **成功标准**: 代码量减少 30-40% (Code reduction 30-40%)

**Our target**: 37% line reduction (14,330 → ~9,000 lines)

---

**Plan Status**: ✅ Complete
**Ready for Implementation**: Yes
**Estimated Duration**: 5.5-7.5 days
**Risk Level**: Low-Medium

Ready to proceed with implementation upon approval.
