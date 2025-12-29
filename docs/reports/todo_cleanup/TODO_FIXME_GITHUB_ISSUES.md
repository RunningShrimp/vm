# TODO/FIXME Items - GitHub Issues Tracker

This document tracks all TODO and FIXME comments found in the codebase that should be converted to GitHub issues.

## Summary
- **Total TODO/FIXME found**: 132
- **In example tools**: 92 (vm-codegen/examples/todo_*.rs)
- **Actual code TODOs**: 40
- **Files affected**: 17

---

## High Priority Issues

### JIT Engine Optimizations (vm-engine-jit)

#### Issue #1: Implement Complete IR Block Fusion
**Location**: `/vm-engine-jit/src/translation_optimizer.rs:186`
**Type**: TODO
**Description**: Implement IR block-level fusion for better optimization
**Impact**: Performance optimization
**Complexity**: High

```rust
// Current state: Placeholder implementation
// TODO: 实现IR块级别的融合
```

**Action Items**:
- [ ] Design IR block fusion algorithm
- [ ] Implement block dependency analysis
- [ ] Add fusion optimization passes
- [ ] Test fusion performance improvements

---

#### Issue #2: Implement Complete x86 Code Generation
**Location**: `/vm-engine-jit/src/translation_optimizer.rs:334`
**Type**: TODO
**Description**: Complete x86 code generation implementation
**Impact**: Cross-architecture support
**Complexity**: High

**Action Items**:
- [ ] Implement full RISC-V to x86 instruction mapping
- [ ] Add x86-specific optimizations
- [ ] Test x86 code generation
- [ ] Benchmark performance

---

#### Issue #3: Implement Constant Propagation Algorithm
**Location**: `/vm-engine-jit/src/translation_optimizer.rs:341`
**Type**: TODO
**Description**: Implement complete constant propagation optimization
**Impact**: Code optimization
**Complexity**: Medium

**Action Items**:
- [ ] Design constant propagation algorithm
- [ ] Implement data flow analysis
- [ ] Add constant folding
- [ ] Test optimization effectiveness

---

#### Issue #4: Implement Dead Code Elimination
**Location**: `/vm-engine-jit/src/translation_optimizer.rs:347`
**Type**: TODO
**Description**: Implement complete dead code detection and elimination
**Impact**: Code size and performance
**Complexity**: Medium

**Action Items**:
- [ ] Implement liveness analysis
- [ ] Add dead code detection
- [ ] Implement elimination pass
- [ ] Test and validate

---

#### Issue #5: Calculate Actual Hash Values for Compilation Cache
**Location**: `/vm-engine-jit/src/domain/compilation.rs:391`
**Type**: TODO
**Description**: Replace placeholder hash with actual computation
**Impact**: Cache correctness
**Complexity**: Low

```rust
hash: 0, // TODO: 计算实际哈希值
```

**Action Items**:
- [ ] Design hash computation strategy
- [ ] Implement hashing algorithm
- [ ] Test cache invalidation
- [ ] Verify cache hits

---

### Platform Implementation (vm-platform)

#### Issue #6: Implement CPU Usage Calculation
**Location**: `/vm-platform/src/runtime.rs:123`
**Type**: TODO
**Description**: Implement actual CPU usage tracking
**Impact**: Monitoring and resource management
**Complexity**: Medium

```rust
cpu_usage_percent: 0.0, // TODO: 实现 CPU 使用率计算
```

**Action Items**:
- [ ] Use platform-specific CPU time APIs
- [ ] Calculate usage percentage
- [ ] Update periodically
- [ ] Test accuracy

---

#### Issue #7: Implement Memory Usage Tracking
**Location**: `/vm-platform/src/runtime.rs:124`
**Type**: TODO
**Description**: Implement memory usage monitoring
**Impact**: Resource management
**Complexity**: Medium

```rust
memory_used_bytes: 0, // TODO: 实现内存使用量计算
```

**Action Items**:
- [ ] Use platform-specific memory APIs
- [ ] Track allocations
- [ ] Update statistics
- [ ] Test accuracy

---

#### Issue #8: Implement Device Count Statistics
**Location**: `/vm-platform/src/runtime.rs:125`
**Type**: TODO
**Description**: Track number of active devices
**Impact**: Resource monitoring
**Complexity**: Low

```rust
device_count: 0, // TODO: 实现设备数量统计
```

**Action Items**:
- [ ] Count active devices
- [ ] Update on device add/remove
- [ ] Test accuracy

---

#### Issue #9: Implement VM Boot Logic
**Location**: `/vm-platform/src/boot.rs:97`
**Type**: TODO
**Description**: Implement actual VM startup sequence
**Impact**: Core functionality
**Complexity**: High

```rust
// TODO: 实现实际的启动逻辑
```

**Action Items**:
- [ ] Design boot sequence
- [ ] Initialize CPU state
- [ ] Load kernel
- [ ] Start execution
- [ ] Test boot process

---

#### Issue #10: Implement VM Shutdown Logic
**Location**: `/vm-platform/src/boot.rs:111`
**Type**: TODO
**Description**: Implement graceful VM shutdown
**Impact**: Core functionality
**Complexity**: High

```rust
// TODO: 实现实际的停止逻辑
```

**Action Items**:
- [ ] Design shutdown sequence
- [ ] Save state if needed
- [ ] Clean up resources
- [ ] Stop CPU
- [ ] Test shutdown

---

### GPU Passthrough (vm-platform)

#### Issue #11: Implement NVIDIA GPU Passthrough Setup
**Location**: `/vm-platform/src/gpu.rs:49`
**Type**: TODO
**Description**: Implement NVIDIA GPU preparation for passthrough
**Impact**: GPU virtualization
**Complexity**: High

**Action Items**:
- [ ] Detect NVIDIA GPUs
- [ ] Setup IOMMU
- [ ] Configure PCI passthrough
- [ ] Test with NVIDIA GPUs
- [ ] Add error handling

---

#### Issue #12: Implement NVIDIA GPU Cleanup
**Location**: `/vm-platform/src/gpu.rs:59`
**Type**: TODO
**Description**: Implement NVIDIA GPU passthrough cleanup
**Impact**: GPU virtualization
**Complexity**: Medium

**Action Items**:
- [ ] Release GPU resources
- [ ] Reset GPU state
- [ ] Handle errors
- [ ] Test cleanup

---

#### Issue #13: Implement AMD GPU Passthrough Setup
**Location**: `/vm-platform/src/gpu.rs:83`
**Type**: TODO
**Description**: Implement AMD GPU preparation for passthrough
**Impact**: GPU virtualization
**Complexity**: High

**Action Items**:
- [ ] Detect AMD GPUs
- [ ] Setup IOMMU
- [ ] Configure PCI passthrough
- [ ] Test with AMD GPUs
- [ ] Add error handling

---

#### Issue #14: Implement AMD GPU Cleanup
**Location**: `/vm-platform/src/gpu.rs:90`
**Type**: TODO
**Description**: Implement AMD GPU passthrough cleanup
**Impact**: GPU virtualization
**Complexity**: Medium

**Action Items**:
- [ ] Release GPU resources
- [ ] Reset GPU state
- [ ] Handle errors
- [ ] Test cleanup

---

### ISO Filesystem (vm-platform)

#### Issue #15: Implement ISO Mount Logic
**Location**: `/vm-platform/src/iso.rs:88`
**Type**: TODO
**Description**: Implement actual ISO filesystem mounting
**Impact**: ISO boot support
**Complexity**: Medium

```rust
// TODO: 实现实际的挂载逻辑
```

**Action Items**:
- [ ] Parse ISO 9660 format
- [ ] Mount filesystem
- [ ] Handle extensions (Joliet, Rock Ridge)
- [ ] Test mounting

---

#### Issue #16: Implement Root Directory Reading
**Location**: `/vm-platform/src/iso.rs:118`
**Type**: TODO
**Description**: Read root directory from ISO
**Impact**: ISO filesystem support
**Complexity**: Medium

```rust
// TODO: 实现实际的根目录读取逻辑
```

**Action Items**:
- [ ] Parse directory records
- [ ] Read root directory
- [ ] Return entries
- [ ] Test reading

---

#### Issue #17: Implement File Reading from ISO
**Location**: `/vm-platform/src/iso.rs:132`
**Type**: TODO
**Description**: Read files from mounted ISO
**Impact**: ISO filesystem support
**Complexity**: Medium

```rust
// TODO: 实现实际的文件读取逻辑
```

**Action Items**:
- [ ] Locate file by path
- [ ] Read file data
- [ ] Handle file attributes
- [ ] Test file reading

---

#### Issue #18: Implement Directory Listing
**Location**: `/vm-platform/src/iso.rs:143`
**Type**: TODO
**Description**: List directory contents from ISO
**Impact**: ISO filesystem support
**Complexity**: Medium

```rust
// TODO: 实现实际的目录列出逻辑
```

**Action Items**:
- [ ] Parse directory entries
- [ ] List contents
- [ ] Handle subdirectories
- [ ] Test listing

---

### SR-IOV Support (vm-platform)

#### Issue #19: Implement SR-IOV Device Scanning
**Location**: `/vm-platform/src/sriov.rs:88`
**Type**: TODO
**Description**: Scan for SR-IOV capable devices in /sys/bus/pci/devices
**Impact**: Network virtualization
**Complexity**: Medium

```rust
// TODO: 实现扫描 /sys/bus/pci/devices 中的 SR-IOV 设备
```

**Action Items**:
- [ ] Scan PCI devices
- [ ] Check SR-IOV capability
- [ ] Return device list
- [ ] Test scanning

---

#### Issue #20: Implement Virtual Function (VF) Creation
**Location**: `/vm-platform/src/sriov.rs:104`
**Type**: TODO
**Description**: Create VFs for SR-IOV devices
**Impact**: Network virtualization
**Complexity**: High

```rust
// TODO: 实现创建 VF 逻辑
```

**Action Items**:
- [ ] Implement VF creation
- [ ] Configure VF parameters
- [ ] Handle errors
- [ ] Test VF creation

---

#### Issue #21: Implement VF Deletion
**Location**: `/vm-platform/src/sriov.rs:120`
**Type**: TODO
**Description**: Delete VFs from SR-IOV devices
**Impact**: Network virtualization
**Complexity**: Medium

```rust
// TODO: 实现删除 VF 逻辑
```

**Action Items**:
- [ ] Implement VF deletion
- [ ] Clean up resources
- [ ] Handle errors
- [ ] Test VF deletion

---

## Medium Priority Issues

### Concurrency Improvements (vm-service)

#### Issue #22: Convert to tokio::sync::Mutex (First Instance)
**Location**: `/vm-service/src/vm_service.rs:321`
**Type**: TODO
**Description**: Refactor std::sync::Mutex to tokio::sync::Mutex for better async compatibility
**Impact**: Async performance
**Complexity**: Medium

```rust
// TODO: Convert Arc<Mutex<VirtualMachineState>> to Arc<tokio::sync::Mutex<VirtualMachineState>>
```

**Action Items**:
- [ ] Identify blocking operations
- [ ] Replace with tokio::sync::Mutex
- [ ] Update all lock sites
- [ ] Test async behavior

---

#### Issue #23: Convert to tokio::sync::Mutex (Second Instance)
**Location**: `/vm-service/src/vm_service.rs:353`
**Type**: TODO
**Description**: Refactor std::sync::Mutex to tokio::sync::Mutex for better async compatibility
**Impact**: Async performance
**Complexity**: Medium

**Action Items**: Same as Issue #22

---

### Lockfree Data Structures (vm-common)

#### Issue #24: Implement True Lockfree Resizing
**Location**: `/vm-common/src/lockfree/hash_table.rs:297`
**Type**: TODO
**Description**: Implement lockfree hash table expansion
**Impact**: Concurrent performance
**Complexity**: High

```rust
// TODO: 实现真正的无锁扩容
```

**Action Items**:
- [ ] Design lockfree resize algorithm
- [ ] Implement incremental migration
- [ ] Handle concurrent operations during resize
- [ ] Test under high concurrency
- [ ] Benchmark performance

---

## Low Priority Issues

### IR Module Migration (vm-ir)

#### Issue #25: Migrate IR Lifter Modules
**Location**: `/vm-ir/src/lift/semantics.rs:9`
**Type**: TODO
**Description**: Migrate or implement missing modules for IR lifting
**Impact**: IR completeness
**Complexity**: Medium

```rust
// TODO: Migrate these modules later if needed, for now we will implement stubs
```

**Action Items**:
- [ ] Identify required modules
- [ ] Implement or migrate modules
- [ ] Remove stubs
- [ ] Test IR lifting

---

#### Issue #26: Complete IR Module Implementation
**Location**: `/vm-ir/src/lift/mod.rs:50`
**Type**: TODO
**Description**: Implement missing IR modules
**Impact**: IR functionality
**Complexity**: Medium

```rust
// TODO: These modules need to be migrated or implemented in vm-ir if they are needed
```

**Action Items**:
- [ ] Determine required modules
- [ ] Implement missing functionality
- [ ] Test integration

---

### Test Fixes (vm-mem)

#### Issue #27: Fix Concurrent TLB Test Timing Issues
**Location**: `/vm-mem/src/tlb/tlb_concurrent.rs:705`
**Type**: TODO
**Description**: Fix timing-related test failures in concurrent TLB
**Impact**: Test reliability
**Complexity**: Medium

```rust
#[ignore]  // TODO: 修复并发TLB测试的时序问题
```

**Action Items**:
- [ ] Identify race conditions
- [ ] Add proper synchronization
- [ ] Fix timing dependencies
- [ ] Enable and validate test

---

#### Issue #28: Fix Sharded TLB Distribution Test
**Location**: `/vm-mem/src/tlb/tlb_concurrent.rs:755`
**Type**: TODO
**Description**: Fix counting issues in sharded TLB distribution test
**Impact**: Test accuracy
**Complexity**: Low

```rust
#[ignore]  // TODO: 修复分片TLB分布测试的计数问题
```

**Action Items**:
- [ ] Debug counting logic
- [ ] Fix counter increments
- [ ] Validate test expectations
- [ ] Enable and validate test

---

#### Issue #29: Fix Memory Pool Test Crash
**Location**: `/vm-mem/src/memory/memory_pool.rs:431`
**Type**: TODO
**Description**: Fix crash in memory pool test
**Impact**: Test stability
**Complexity**: Medium

```rust
#[ignore]  // TODO: 修复此测试的崩溃问题
```

**Action Items**:
- [ ] Identify crash cause
- [ ] Fix memory safety issue
- [ ] Add proper error handling
- [ ] Enable and validate test

---

#### Issue #30: Fix SV39 Page Table Translation Test
**Location**: `/vm-mem/src/lib.rs:1265`
**Type**: TODO
**Description**: Fix SV39 page table translation logic test
**Impact**: Memory management correctness
**Complexity**: High

```rust
#[ignore]  // TODO: 修复SV39页表翻译逻辑
```

**Action Items**:
- [ ] Debug translation logic
- [ ] Fix page walk implementation
- [ ] Validate address translation
- [ ] Enable and validate test

---

### Infrastructure (vm-common)

#### Issue #31: Create Placeholder Modules
**Location**: `/vm-common/src/lib.rs:11`
**Type**: TODO
**Description**: Create modules when needed (comment placeholder)
**Impact**: Project organization
**Complexity**: Variable

```rust
// TODO: Create these modules when needed
```

**Action Items**:
- [ ] Determine if modules are needed
- [ ] Create or remove comment
- [ ] Update documentation

---

#### Issue #32: Re-enable Conditional Compilation
**Location**: `/vm-common/src/lib.rs:63`
**Type**: TODO
**Description**: Re-enable code when dependencies are implemented
**Impact**: Feature completeness
**Complexity**: Low

```rust
/// TODO: Re-enable when required modules are implemented
```

**Action Items**:
- [ ] Implement required modules
- [ ] Remove conditional compilation
- [ ] Test re-enabled code

---

## Documentation/Meta Comments

### Foundation Macros (vm-foundation)

The following TODO/FIXME entries in `/vm-foundation/src/support_macros.rs` are **documentation/meta-comments** and should be kept as-is:

- Line 151: `/// TODO宏：标记待实现的功能` - Macro documentation
- Line 162: `/// FIXME宏：标记需要修复的代码` - Macro documentation

These are intentional macro definitions and documentation, not actual TODO items.

---

## Cleanup Strategy

### Immediate Actions (Safe to Remove)
None - all TODOs represent real work that should be tracked.

### Should Convert to Issues
All 30+ issues above should be created as GitHub issues with appropriate labels and priorities.

### Comments to Keep
- vm-foundation macro documentation (lines 151-169)

---

## Next Steps

1. **Create GitHub Issues**: Use this document to create issues in your GitHub repository
2. **Add Issue References**: Replace TODO comments with issue references like:
   ```rust
   // See: Issue #1 - Implement IR Block Fusion
   ```
3. **Track Progress**: Update this document as issues are resolved
4. **Remove Comments**: Once issues are created and referenced, remove TODO comments

---

## Statistics

- **High Priority**: 21 issues
- **Medium Priority**: 3 issues
- **Low Priority**: 6 issues
- **Documentation**: 2 entries (keep as-is)
- **Total Actionable**: 30 issues

---

## Files Requiring Cleanup

1. `/vm-engine-jit/src/translation_optimizer.rs` (5 TODOs)
2. `/vm-engine-jit/src/x86_codegen.rs` (1 TODO)
3. `/vm-engine-jit/src/domain/compilation.rs` (1 TODO)
4. `/vm-platform/src/runtime.rs` (3 TODOs)
5. `/vm-platform/src/boot.rs` (2 TODOs)
6. `/vm-platform/src/gpu.rs` (4 TODOs)
7. `/vm-platform/src/iso.rs` (4 TODOs)
8. `/vm-platform/src/sriov.rs` (3 TODOs)
9. `/vm-service/src/vm_service.rs` (2 TODOs)
10. `/vm-common/src/lockfree/hash_table.rs` (1 TODO)
11. `/vm-common/src/lib.rs` (2 TODOs)
12. `/vm-ir/src/lift/semantics.rs` (1 TODO)
13. `/vm-ir/src/lift/mod.rs` (1 TODO)
14. `/vm-mem/src/tlb/tlb_concurrent.rs` (2 TODOs)
15. `/vm-mem/src/memory/memory_pool.rs` (1 TODO)
16. `/vm-mem/src/lib.rs` (1 TODO)

---

**Generated**: 2025-12-28
**Total TODO/FIXME comments analyzed**: 132
**Actionable issues identified**: 30
**Example/tool code**: 92 comments (excluded from action items)
