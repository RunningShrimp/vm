# Technical Debt Analysis - Session 1
## Ralph Loop Iteration 1

**Date:** 2026-01-07
**Focus:** Clean technical debt - small functions and TODO cleanup

---

## Summary

This report documents the technical debt found in the VM codebase, categorized by severity and type.

---

## 1. TODO/FIXME/HACK Analysis

### High Priority (Blocking Features)

#### vm-passthrough/src/rocm.rs
**Lines 528-531:** Missing ROCm GPU information retrieval
```rust
free_memory_mb: self.total_memory_mb, // TODO: 获取实际可用内存
multiprocessor_count: 0,              // TODO: 获取实际CU数量
clock_rate_khz: 0,                    // TODO: 获取实际时钟频率
l2_cache_size: 0,                     // TODO: 获取L2缓存
```
**Impact:** GPU resource reporting is inaccurate, affects performance monitoring
**Action:** Query ROCm API for actual device properties

#### vm-passthrough/src/rocm.rs
**Lines 564-570:** Unimplemented ROCm operations
```rust
// TODO: 实现device到host的复制
// TODO: 实现HIPRTC编译
```
**Impact:** Core GPU passthrough functionality incomplete
**Action:** Implement D2H memory copy and HIPRTC kernel compilation

#### vm-passthrough/src/cuda.rs
**Lines 1128-1129, 1247:** Missing CUDA metadata tracking
```rust
num_params: 0, // TODO: Parse from source
shared_memory_size: 0, // TODO: Parse from source
bytes_transferred: 0, // TODO: Track actual memory transfers
```
**Impact:** Incomplete CUDA execution profiling
**Action:** Parse PTX source for kernel metadata

### Medium Priority (Test Infrastructure)

#### vm-engine-jit/src/cranelift_backend.rs
**Lines 453, 539, 692:** Multiple ignored tests
```rust
#[ignore = "TODO: Fix empty compiled code issue - requires debugging Cranelift compilation flow"]
```
**Impact:** JIT compilation quality issues not caught by tests
**Action:** Debug Cranelift compilation pipeline and fix test infrastructure

#### vm-core/src/error.rs
**Lines 1175-1177, 1412-1414:** Broken error conversion tests
```rust
// TODO: Fix these tests - From trait not working as expected
```
**Impact:** Error handling correctness not verified
**Action:** Fix From trait implementations or update tests

#### vm-mem/src/memory/numa_allocator.rs
**Line 931:** Commented out NUMA GC test
```rust
// TODO: Fix test - investigate why local_allocs is 0
```
**Impact:** NUMA memory management correctness uncertain
**Action:** Debug NUMA allocator behavior

#### vm-core/src/domain_services/target_optimization_service.rs
**Lines 1239, 1293, 1327:** Missing configuration fields
```rust
// TODO: Fix - config doesn't have max_unroll_factor field
```
**Impact:** Loop unrolling optimization not validated
**Action:** Add missing config fields or update assertions

### Low Priority (Documentation)

#### vm-ir/src/lift/llvm_version.rs
**Line 232:** Environment variable detection comment
```rust
// 检查LLVM_SYS_XXX_PREFIX环境变量
```
**Status:** Actually implemented, just Chinese comment
**Action:** Translate to English or improve documentation

---

## 2. Small File Analysis (<10 lines)

### Module Re-exports (Acceptable - Keep)
These files serve as organizational re-exports:
- `vm-device/src/gpu_manager/wgpu_backend.rs` (5 lines) - GPU backend re-export
- `vm-engine/src/jit/register_allocator_adapter/mod.rs` (10 lines) - Module organization
- `vm-engine/src/jit/optimizer_strategy/mod.rs` - Strategy pattern module
- `vm-engine/src/jit/cache/mod.rs` - Cache module organization
- `vm-engine/src/jit/aot/mod.rs` - AOT module organization
- `vm-frontend/src/translation/mod.rs` - Translation module
- `vm-mem/src/domain_services/mod.rs` - Domain services module

**Recommendation:** ✅ Keep - These are proper DDD module boundaries

### Stub Implementations (Need Expansion)
These are placeholder implementations that need work:

#### vm-engine-jit/src/jit_helpers.rs (6 lines)
```rust
//! JIT辅助函数占位实现

pub struct FloatRegHelper;
pub struct MemoryHelper;
pub struct RegisterHelper;
```
**Status:** Empty structs - no implementation
**Required:**
- FloatRegHelper: Floating-point register allocation helpers
- MemoryHelper: Memory operand handling utilities
- RegisterHelper: General register management utilities

#### vm-device/src/gpu_manager/passthrough.rs
**Status:** GPU passthrough stub
**Required:** Full GPU device passthrough implementation

#### vm-device/src/gpu_manager/mdev.rs
**Status:** Mediated device framework stub
**Required:** mdev device management for GPU virtualization

#### vm-device/src/virgl.rs
**Status:** VirGL virtualization stub
**Required:** VirGL rendering backend for 3D virtualization

#### vm-device/src/npu_engine.rs
**Status:** NPU acceleration stub
**Required:** Neural Processing Unit support

---

## 3. Action Plan

### Immediate Actions (Iteration 1)

1. **Implement vm-engine-jit/src/jit_helpers.rs**
   - Add FloatRegHelper for x86/x86_64 SSE/AVX register handling
   - Add MemoryHelper for load/store operand encoding
   - Add RegisterHelper for general-purpose register allocation

2. **Fix Critical ROCm TODOs in vm-passthrough/src/rocm.rs**
   - Implement get_memory_info() for actual free memory
   - Implement get_cu_count() for compute unit count
   - Implement get_clock_rate() for GPU frequency
   - Implement get_l2_cache_size() for cache info

3. **Fix CUDA Metadata Tracking in vm-passthrough/src/cuda.rs**
   - Parse PTX source for parameter count
   - Extract shared memory size from kernel metadata
   - Track memory transfer sizes in execution results

### Medium Priority (Iteration 2-3)

4. **Fix JIT Test Infrastructure**
   - Debug Cranelift empty code generation
   - Re-enable ignored tests
   - Add compilation verification

5. **Fix Error Handling Tests**
   - Fix From trait implementations
   - Re-enable error conversion tests

6. **Fix NUMA Allocator Test**
   - Debug local_allocs tracking
   - Re-enable GC integration test

### Longer Term (Iteration 4+)

7. **Expand GPU Virtualization Stubs**
   - Implement full GPU passthrough
   - Add mdev framework support
   - Implement VirGL backend
   - Add NPU acceleration

---

## 4. Package Structure Review

### Current Package Organization
The codebase has excellent DDD-aligned package structure:
- **vm-core**: Domain models and services
- **vm-engine**: Execution engine (interpreter/JIT)
- **vm-engine-jit**: JIT compilation infrastructure
- **vm-accel**: Hardware acceleration (KVM/HVF/VZ)
- **vm-device**: Device emulation
- **vm-mem**: Memory management
- **vm-ir**: Intermediate representation
- **vm-cross-arch-support**: Cross-architecture translation

**Assessment:** ✅ Well-organized, clear separation of concerns

**Potential Consolidations:**
- Consider merging `vm-engine` and `vm-engine-jit` if JIT is always used
- Consider extracting `vm-passthrough` into separate crate if it grows

**Overall:** No urgent changes needed - structure is production-ready

---

## 5. Next Steps

1. ✅ Complete technical debt inventory
2. 🔄 Start implementing jit_helpers.rs (in progress)
3. ⏳ Fix ROCm GPU information retrieval
4. ⏳ Fix CUDA metadata tracking
5. ⏳ Address test infrastructure issues
6. ⏳ Expand stub implementations

---

## Statistics

- **Total TODOs found:** 94 files (mostly documentation/markdown)
- **Source files with TODOs:** ~15 files
- **Critical TODOs:** 8
- **Stub implementations:** 5
- **Files requiring action:** ~20

**Estimated effort:** 3-4 iterations to resolve critical technical debt
