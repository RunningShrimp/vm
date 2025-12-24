# Rust虚拟机全面实施计划与Todolist

## 执行摘要

本实施计划基于全面的架构审查报告（2025-12-23）和现有实施计划文档（V1、V2、性能优化计划、软件改进计划）综合制定，旨在解决：
- **P0关键问题**：vm-core编译失败、JIT/AOT执行闭环缺失、GC未集成
- **P1重要问题**：系统调用覆盖不足、硬件加速feature禁用、代码冗余严重
- **P2基础问题**：文档缺失、中间文件清理
- **P3质量保障**：测试覆盖不足、性能基准缺失

---

## 一、阶段划分与优先级

| 阶段 | 目标 | 周期 | 优先级 |
|------|------|------|--------|
| **阶段A：紧急修复** | 修复vm-core编译错误，确保项目可构建 | 1周 | P0 |
| **阶段B：核心功能闭环** | 实现JIT/AOT原生执行、集成GC | 3周 | P0 |
| **阶段C：系统调用完善** | 实现核心syscall集和错误传播 | 2周 | P1 |
| **阶段D：硬件加速激活** | 启用KVM/HVF/WHPX feature | 2周 | P1 |
| **阶段E：代码重构优化** | 合并冗余文件，提升可维护性 | 4周 | P1 |
| **阶段F：文档建设** | 创建README、API文档、使用指南 | 2周 | P2 |
| **阶段G：测试覆盖** | 实现单元测试、集成测试、性能基准 | 4周 | P3 |
| **阶段H：性能优化** | JIT编译优化、GC暂停优化、TLB优化 | 8周 | P1-P3 |

---

## 二、全量Todolist

### 阶段A：紧急修复（P0）

#### A.1 修复vm-core编译错误
- [ ] **A.1.1** 修复aggregate_root.rs中record_event重复定义问题
  - 代码位置：`vm-core/src/aggregate_root.rs:45-55`
  - 证据：两处`pub fn record_event`定义，一处接收`&mut self`，一处接收`&mut self, event: DomainEventEnum`
  - 修复：删除重复定义，保留接收事件参数的版本

- [ ] **A.1.2** 修复VmResourceAvailabilityRule的re-export/导入路径错误
  - 代码位置：`vm-core/src/domain_services/vm_lifecycle_service.rs:12-15`
  - 证据：`use crate::domain_services::rules::VmResourceAvailabilityRule;` 编译失败
  - 修复：确认正确的模块路径或添加必要的`pub use`

- [ ] **A.1.3** 修复事件发布publish_event参数可变性签名不一致
  - 代码位置：`vm-core/src/aggregate_root.rs:78-82` 调用位置
  - 证据：`self.event_bus.publish_event(event, &self.uncommitted_events)` 但event已move
  - 修复：改为`self.event_bus.publish_event(&event, &self.uncommitted_events)`

- [ ] **A.1.4** 验证vm-core编译成功
  - 命令：`cargo check -p vm-core`
  - 验收：无编译错误和警告

#### A.2 清理中间文件
- [ ] **A.2.1** 删除vm-core/fix_all_compilation_errors目录
  - 证据：`vm-core/fix_all_compilation_errors/` 包含编译错误修复的临时文件

- [ ] **A.2.2** 删除vm-core/fix_conditional_compilation目录
  - 证据：`vm-core/fix_conditional_compilation/` 包含条件编译修复的临时文件

- [ ] **A.2.3** 删除vm-core/fix_syntax_errors目录
  - 证据：`vm-core/fix_syntax_errors/` 包含语法错误修复的临时文件

- [ ] **A.2.4** 删除项目根目录的临时输出文件
  - 删除文件：
    - `build_errors.txt`
    - `cargo_check_output.txt`
    - `check_output_detailed.txt`
    - `errors.json`
    - `vm-device-errors.txt`

---

### 阶段B：核心功能闭环（P0）

#### B.1 实现JIT/AOT原生执行路径
- [ ] **B.1.1** 重构UnifiedExecutor实现原生AOT执行
  - 代码位置：`vm-boot/src/executor/unified.rs:120-135`
  - 证据：当前`AotExecutor`的`cache: Vec<u8>`仅存储编译结果，执行时回退`runtime.execute_block`
  - 实现方案：
    ```rust
    // 原生AOT执行：直接执行编译后的机器码
    let entry_point = unsafe {
        std::mem::transmute::<*const u8, extern "C" fn() -> i64>(code_cache.as_ptr())
    };
    let result = entry_point();
    ```
  - 验收：AOT编译的代码直接执行，不再回退解释器

- [ ] **B.1.2** 重构AutoExecutor实现原生JIT执行
  - 代码位置：`vm-boot/src/executor/auto.rs:85-102`
  - 证据：当前`JitExecutor`编译后回退到`runtime.execute_block`，而非执行编译后的代码
  - 实现方案：
    ```rust
    // JIT编译后直接执行，回退仅用于编译失败场景
    match jit_compiler.compile_block(block) {
        Ok(code) => unsafe { execute_compiled_code(code) },
        Err(_) => runtime.execute_block(block),
    }
    ```
  - 验收：JIT编译成功后直接执行，失败时才回退解释器

- [ ] **B.1.3** 实现JIT可执行内存管理
  - 代码位置：`vm-engine-jit/src/core.rs` 需新增模块
  - 实现方案：
    ```rust
    pub struct ExecutableMemory {
        ptr: *mut u8,
        size: usize,
    }

    impl ExecutableMemory {
        pub fn new(size: usize) -> Result<Self> {
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    size,
                    libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            if ptr == libc::MAP_FAILED {
                return Err(Error::MemoryAllocationFailed);
            }
            Ok(Self { ptr: ptr as *mut u8, size })
        }

        pub fn write_code(&mut self, offset: usize, code: &[u8]) {
            unsafe {
                std::ptr::copy_nonoverlapping(code.as_ptr(), self.ptr.add(offset), code.len());
            }
        }

        pub fn execute(&self) -> i64 {
            unsafe {
                let func: extern "C" fn() -> i64 = std::mem::transmute(self.ptr);
                func()
            }
        }
    }
    ```
  - 验收：JIT编译的代码可执行，支持W^X保护

- [ ] **B.1.4** 实现icache刷新机制
  - 代码位置：`vm-engine-jit/src/core.rs`
  - 实现方案：
    ```rust
    #[cfg(target_arch = "x86_64")]
    pub fn flush_icache(ptr: *const u8, size: usize) {
        unsafe { std::arch::asm!("wbinvd"); }
    }

    #[cfg(target_arch = "aarch64")]
    pub fn flush_icache(ptr: *const u8, size: usize) {
        unsafe {
            let start = ptr as usize;
            let end = start + size;
            for addr in (start..end).step_by(64) {
                std::arch::asm!("ic ivau, {}", in(reg) addr, options(nostack));
            }
            std::arch::asm!("dsb ish", "isb");
        }
    }
    ```
  - 验收：JIT代码修改后icache正确刷新

- [ ] **B.1.5** 验证JIT/AOT功能完整性
  - 验证点：
    - IR优化阶段（常量折叠、死代码消除、公共子表达式消除）
    - 寄存器分配阶段（图着色或线性扫描算法）
    - 指令调度阶段（依赖分析和指令重排）
    - 代码生成阶段（目标机器码生成）
  - 验收：所有编译阶段完整实现，可独立验证输出

#### B.2 集成GC运行时
- [ ] **B.2.1** 集成gc-optimizer到vm-boot
  - 代码位置：`vm-boot/Cargo.toml` 和 `vm-boot/src/runtime/gc.rs`
  - 当前状态：gc-optimizer独立实现，未与vm-boot的GC运行时集成
  - 实现方案：
    ```rust
    // vm-boot/src/runtime/gc.rs
    use gc_optimizer::{OptimizedGc, LockFreeWriteBarrier, ParallelMarker};

    pub struct VmGcRuntime {
        inner: OptimizedGc,
        write_barrier: LockFreeWriteBarrier,
        marker: ParallelMarker,
    }

    impl VmGcRuntime {
        pub fn new(config: GcConfig) -> Self {
            let inner = OptimizedGc::new(config.heap_size);
            let write_barrier = LockFreeWriteBarrier::new();
            let marker = ParallelMarker::new(config.parallelism);
            Self { inner, write_barrier, marker }
        }

        pub fn allocate(&mut self, size: usize) -> *mut u8 {
            self.inner.allocate(size)
        }

        pub fn collect(&mut self) {
            self.marker.mark(&self.inner);
            self.inner.sweep();
        }
    }
    ```
  - 验收：gc-optimizer的优化功能（写屏障、并行标记、自适应配额）在vm-boot中生效

- [ ] **B.2.2** 实现GC暂停时间优化
  - 目标：暂停时间 < 10ms（高负载场景）
  - 实现方案：
    - 增量标记：将标记任务拆分为多个小块，每次执行最多5ms
    - 并发清理：使用后台线程执行清理任务
    - 自适应配额：根据上次GC时间动态调整本次GC配额
  - 验收：压力测试下GC暂停时间稳定在10ms以内

- [ ] **B.2.3** 验证GC集成效果
  - 测试用例：
    - 高频分配释放场景
    - 大对象分配场景
    - 并发分配场景
  - 验收：GC集成后性能提升 > 30%（对比未集成版本）

---

### 阶段C：系统调用完善（P1）

#### C.1 实现核心syscall集
- [ ] **C.1.1** 实现文件系统相关syscall
  - 实现列表：
    - `read(fd: i32, buf: *mut u8, count: usize) -> isize`
    - `write(fd: i32, buf: *const u8, count: usize) -> isize`（已实现）
    - `open(pathname: *const u8, flags: i32, mode: u32) -> i32`
    - `close(fd: i32) -> i32`
    - `stat(pathname: *const u8, statbuf: *mut Stat) -> i32`
    - `fstat(fd: i32, statbuf: *mut Stat) -> i32`
    - `lseek(fd: i32, offset: i64, whence: i32) -> i64`
    - `mmap(addr: *mut u8, length: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8`
    - `munmap(addr: *mut u8, length: usize) -> i32`
    - `mprotect(addr: *mut u8, len: usize, prot: i32) -> i32`
  - 代码位置：`vm-core/src/syscalls/filesystem.rs`（需新建）
  - 验收：所有文件系统syscall正确实现，返回正确值或错误码

- [ ] **C.1.2** 实现进程管理相关syscall
  - 实现列表：
    - `brk(addr: *mut u8) -> *mut u8`
    - `rt_sigaction(sig: i32, act: *const Sigaction, oact: *mut Sigaction, sigsetsize: usize) -> i32`
    - `rt_sigprocmask(how: i32, set: *const u64, oset: *mut u64, sigsetsize: usize) -> i32`
    - `rt_sigreturn() -> !`
    - `getpid() -> i32`
    - `clone(flags: u32, child_stack: *mut u8, ptid: *mut i32, ctid: *mut i32, newtls: u64) -> i32`
    - `fork() -> i32`
    - `vfork() -> i32`
    - `execve(filename: *const u8, argv: *const *const u8, envp: *const *const u8) -> i32`
    - `exit(error_code: i32) -> !`（已实现）
    - `wait4(pid: i32, wstatus: *mut i32, options: i32, rusage: *mut Rusage) -> i32`
    - `kill(pid: i32, sig: i32) -> i32`
  - 代码位置：`vm-core/src/syscalls/process.rs`（需新建）
  - 验收：所有进程管理syscall正确实现

- [ ] **C.1.3** 实现网络相关syscall
  - 实现列表：
    - `socket(domain: i32, type_: i32, protocol: i32) -> i32`
    - `connect(sockfd: i32, addr: *const Sockaddr, addrlen: u32) -> i32`
    - `accept(sockfd: i32, addr: *mut Sockaddr, addrlen: *mut u32) -> i32`
    - `sendto(sockfd: i32, buf: *const u8, len: usize, flags: i32, addr: *const Sockaddr, addrlen: u32) -> isize`
    - `recvfrom(sockfd: i32, buf: *mut u8, len: usize, flags: i32, addr: *mut Sockaddr, addrlen: *mut u32) -> isize`
    - `sendmsg(sockfd: i32, msg: *const Msghdr, flags: i32) -> isize`
    - `recvmsg(sockfd: i32, msg: *mut Msghdr, flags: i32) -> isize`
    - `shutdown(sockfd: i32, how: i32) -> i32`
    - `bind(sockfd: i32, addr: *const Sockaddr, addrlen: u32) -> i32`
    - `listen(sockfd: i32, backlog: i32) -> i32`
    - `getsockname(sockfd: i32, addr: *mut Sockaddr, addrlen: *mut u32) -> i32`
    - `getpeername(sockfd: i32, addr: *mut Sockaddr, addrlen: *mut u32) -> i32`
    - `socketpair(domain: i32, type_: i32, protocol: i32, sv: *mut [i32; 2]) -> i32`
    - `setsockopt(sockfd: i32, level: i32, optname: i32, optval: *const u8, optlen: u32) -> i32`
    - `getsockopt(sockfd: i32, level: i32, optname: i32, optval: *mut u8, optlen: *mut u32) -> i32`
  - 代码位置：`vm-core/src/syscalls/network.rs`（需新建）
  - 验收：所有网络syscall正确实现

- [ ] **C.1.4** 实现其他核心syscall
  - 实现列表：
    - `ioctl(fd: i32, request: u64, ...) -> i32`
    - `pread64(fd: i32, buf: *mut u8, count: usize, offset: i64) -> isize`
    - `pwrite64(fd: i32, buf: *const u8, count: usize, offset: i64) -> isize`
    - `readv(fd: i32, iov: *const IoVec, iovcnt: i32) -> isize`
    - `writev(fd: i32, iov: *const IoVec, iovcnt: i32) -> isize`
    - `access(pathname: *const u8, mode: u32) -> i32`
    - `pipe(pipefd: *mut [i32; 2]) -> i32`
    - `select(nfds: i32, readfds: *mut FdSet, writefds: *mut FdSet, exceptfds: *mut FdSet, timeout: *mut Timeval) -> i32`
    - `sched_yield() -> i32`
    - `mremap(old_address: *mut u8, old_size: usize, new_size: usize, flags: i32, ...) -> *mut u8`
    - `msync(addr: *mut u8, length: usize, flags: i32) -> i32`
    - `mincore(addr: *mut u8, length: usize, vec: *mut u8) -> i32`
    - `uname(buf: *mut Utsname) -> i32`
    - `sendfile(out_fd: i32, in_fd: i32, offset: *mut i64, count: usize) -> isize`
  - 代码位置：`vm-core/src/syscalls/misc.rs`（需新建）
  - 验收：所有misc syscall正确实现

#### C.2 实现syscall错误传播机制
- [ ] **C.2.1** 替换默认Ok(0)返回为实际错误码
  - 代码位置：`vm-core/src/syscalls/mod.rs`
  - 当前证据：未实现的syscall默认返回`Ok(0)`，产生静默错误
  - 实现方案：
    ```rust
    pub fn handle_syscall(nr: i64, args: &[u64]) -> Result<i64, Errno> {
        match nr {
            NR_READ => handle_read(args),
            NR_WRITE => handle_write(args),
            // ...
            _ => Err(Errno::ENOSYS), // 未实现的系统调用返回ENOSYS
        }
    }
    ```
  - 错误码映射：
    - `ENOSYS` (38): Function not implemented
    - `EINVAL` (22): Invalid argument
    - `ENOMEM` (12): Out of memory
    - `EACCES` (13): Permission denied
    - `ENOENT` (2): No such file or directory
  - 验收：所有未实现的syscall返回正确错误码，不产生静默错误

---

### 阶段D：硬件加速激活（P1）

#### D.1 启用KVM feature
- [ ] **D.1.1** 取消kvm_impl.rs中的Temporarily disabled注释
  - 代码位置：`vm-accel/src/kvm_impl.rs:1-5`
  - 证据：当前KVM feature被注释为"Temporarily disabled"
  - 实现方案：移除注释，确保KVM feature正确启用

- [ ] **D.1.2** 验证KVM feature开关
  - 代码位置：`vm-accel/Cargo.toml`
  - 验证：`[features]`中`kvm`正确配置

- [ ] **D.1.3** 测试KVM功能
  - 测试场景：
    - vCPU创建和销毁
    - 内存映射
    - 寄存器访问
    - 中断注入
  - 验收：KVM功能在Linux平台正常工作

#### D.2 完善HVF ARM64支持
- [ ] **D.2.1** 增强Apple Silicon M系列的完整支持
  - 代码位置：`vm-accel/src/hvf_impl.rs`
  - 当前状态：基础支持已实现，需增强M系列特定优化
  - 增强内容：
    - M系列核心数和线程数适配
    - M系列虚拟化扩展利用
    - M特定指令集支持（如AMX）
  - 验收：HVF在Apple Silicon平台性能提升 > 20%

#### D.3 完善WHPX基本支持
- [ ] **D.3.1** 添加基础初始化
  - 代码位置：`vm-accel/src/whpx_impl.rs`
  - 实现内容：
    - WHPX平台初始化
    - WHPX虚拟机创建
    - WHPX vCPU创建
  - 验收：WHPX在Windows平台正常初始化

- [ ] **D.3.2** 实现内存映射
  - 实现内容：
    - GPA到HPA映射
    - 内存槽管理
    - 大页支持（2MB、1GB）
  - 验收：WHPX内存映射正确

- [ ] **D.3.3** 实现寄存器访问
  - 实现内容：
    - 通用寄存器读写
    - 特殊寄存器读写（CR0、CR4、EFER等）
    - 标志寄存器读写
  - 验收：WHPX寄存器访问正确

#### D.4 验证Virtualization.framework支持
- [ ] **D.4.1** 验证iOS/tvOS平台Virtualization.framework feature
  - 代码位置：`vm-accel/Cargo.toml`
  - 验证：`virtualization_framework` feature正确配置
  - 验收：iOS/tvOS平台Virtualization.framework功能正常

---

### 阶段E：代码重构优化（P1）

#### E.1 合并TLB冗余文件
- [ ] **E.1.1** 合并TLB核心功能
  - 合并文件：
    - `tlb.rs` → 保留作为主文件
    - `tlb_advanced.rs` → 合并到主文件
    - `tlb_core.rs` → 合并到主文件
  - 合并策略：
    - 保留最完整的实现（tlb_optimized.rs中的791行实现）
    - 删除重复的TLBEntry定义
    - 统一接口和trait
  - 预期减少：1500-2000行代码

- [ ] **E.1.2** 合并TLB刷新功能
  - 合并文件：
    - `tlb_flush.rs` → 保留
    - `tlb_flush_advanced.rs` → 合并到tlb_flush.rs
    - `tlb_flush_batch.rs` → 合并到tlb_flush.rs
  - 合并策略：
    - 保留高级刷新算法
    - 统一批量刷新接口
  - 预期减少：800-1200行代码

- [ ] **E.1.3** 合并TLB预取功能
  - 合并文件：
    - `tlb_prefetch.rs` → 保留
    - `tlb_prefetch_smart.rs` → 合并到tlb_prefetch.rs
  - 合并策略：
    - 保留智能预取算法
    - 统一预取策略配置
  - 预期减少：300-500行代码

- [ ] **E.1.4** 验证TLB合并后功能完整性
  - 测试场景：
    - 多级缓存命中率测试
    - 预取效果测试
    - 刷新算法效率测试
  - 验收：TLB功能完整性100%保留，性能无回退

#### E.2 合并JIT优化器冗余文件
- [ ] **E.2.1** 合并IR优化器
  - 合并文件：
    - `optimizer.rs` → 保留作为主文件
    - `ir_optimizer.rs` → 合并到主文件
    - `passes/constant_folding.rs` → 合并到主文件
    - `passes/dead_code_elimination.rs` → 合并到主文件
    - `passes/common_subexpr_elimination.rs` → 合并到主文件
  - 合并策略：
    - 将各个pass整合为统一的优化管道
    - 保留独立的pass函数，便于单独调用
  - 预期减少：1000-1500行代码

- [ ] **E.2.2** 合并SIMD优化器
  - 合并文件：
    - `simd_optimizer.rs` → 保留
    - `simd_vectorizer.rs` → 合并到主文件
    - `simd_inliner.rs` → 合并到主文件
  - 合并策略：
    - 统一向量化接口
    - 保留AVX2、ARM NEON独立路径
  - 预期减少：500-800行代码

- [ ] **E.2.3** 合并自适应优化器
  - 合并文件：
    - `adaptive_optimizer.rs` → 保留
    - `adaptive_hotspot.rs` → 合并到主文件
    - `adaptive_tiered.rs` → 合并到主文件
  - 合并策略：
    - 统一自适应策略配置
    - 保留热点检测和分层优化逻辑
  - 预期减少：400-600行代码

- [ ] **E.2.4** 验证JIT优化器合并后功能完整性
  - 测试场景：
    - 常量折叠、死代码消除、公共子表达式消除正确性
    - SIMD向量化效果（AVX2、ARM NEON）
    - 自适应热点检测和分层优化效果
  - 验收：JIT优化器功能完整性100%保留，性能无回退

#### E.3 合并跨架构优化器冗余文件
- [ ] **E.3.1** 合并x86_64优化器
  - 合并文件：
    - `x86_64_optimizer.rs` → 保留
    - `x86_64_peephole.rs` → 合并到主文件
  - 合并策略：
    - 统一x86_64优化接口
    - 保留窥孔优化逻辑
  - 预期减少：300-500行代码

- [ ] **E.3.2** 合并ARM64优化器
  - 合并文件：
    - `arm64_optimizer.rs` → 保留
    - `arm64_peephole.rs` → 合并到主文件
  - 合并策略：
    - 统一ARM64优化接口
    - 保留窥孔优化逻辑
  - 预期减少：300-500行代码

- [ ] **E.3.3** 合并RISC-V优化器
  - 合并文件：
    - `riscv_optimizer.rs` → 保留
    - `riscv_peephole.rs` → 合并到主文件
  - 合并策略：
    - 统一RISC-V优化接口
    - 保留窥孔优化逻辑
  - 预期减少：200-400行代码

- [ ] **E.3.4** 验证跨架构优化器合并后功能完整性
  - 测试场景：
    - x86_64 → ARM64翻译优化效果
    - ARM64 → RISC-V翻译优化效果
    - RISC-V → x86_64翻译优化效果
  - 验收：跨架构优化器功能完整性100%保留，性能无回退

#### E.4 代码重构总结
- [ ] **E.4.1** 统计代码减少量
  - 预期统计：
    - TLB模块：9个文件 → 6个文件，减少2600-3700行
    - JIT优化器：12个文件 → 4个文件，减少1900-2900行
    - 跨架构优化器：6个文件 → 3个文件，减少800-1400行
    - 总计：27个文件 → 13个文件，减少5300-8000行（44%）
  - 验收：代码减少量达到预期目标

- [ ] **E.4.2** 更新文档和依赖关系
  - 更新内容：
    - 更新模块导入路径
    - 更新API文档
    - 更新README中的模块说明
  - 验收：文档与代码保持一致

---

### 阶段F：文档建设（P2）

#### F.1 创建项目根README.md
- [ ] **F.1.1** 编写项目概述
  - 内容要求：
    - 项目简介（高性能跨平台虚拟机）
    - 核心特性（跨架构两两互运行、JIT/AOT、硬件加速、GC优化）
    - 支持架构（AMD64、ARM64、RISC-V64）
    - 支持平台（Linux、macOS、Windows、iOS、tvOS）

- [ ] **F.1.2** 编写架构说明
  - 内容要求：
    - Workspace结构（34个crate）
    - 核心模块（vm-core、vm-cross-arch、vm-engine-jit、vm-accel、vm-mem）
    - DDD架构设计（贫血模型、聚合根、领域服务）
    - JIT/AOT编译流水线（IR优化→寄存器分配→指令调度→代码生成）

- [ ] **F.1.3** 编写构建指南
  - 内容要求：
    - 依赖安装（Rust工具链、平台特定依赖）
    - 构建命令（`cargo build --release`）
    - feature选择（kvm、hvf、whpx、virtualization_framework）
    - 常见问题（编译错误、平台兼容性）

- [ ] **F.1.4** 编写使用示例
  - 内容要求：
    - 基本VM创建和执行示例
    - 跨架构运行示例（ARM64上运行x86_64）
    - JIT/AOT配置示例
    - 硬件加速启用示例

- [ ] **F.1.5** 编写贡献指南
  - 内容要求：
    - 代码规范（Rust 2024 edition）
    - 测试要求（单元测试、集成测试）
    - 提交流程（PR模板、Code Review）
    - Issue模板（Bug报告、Feature request）

#### F.2 创建各模块API文档
- [ ] **F.2.1** 生成vm-core API文档
  - 工具：`cargo doc -p vm-core --open`
  - 补充内容：
    - 聚合根说明（VirtualMachineAggregate）
    - 领域服务说明（VmLifecycleService）
    - MMU trait说明（AddressTranslator、MemoryAccess、MmioManager）
    - Syscall接口说明

- [ ] **F.2.2** 生成vm-cross-arch API文档
  - 工具：`cargo doc -p vm-cross-arch --open`
  - 补充内容：
    - 翻译器说明（Translator）
    - 编码器说明（Encoder）
    - 运行时说明（Runtime）

- [ ] **F.2.3** 生成vm-engine-jit API文档
  - 工具：`cargo doc -p vm-engine-jit --open`
  - 补充内容：
    - JIT引擎说明（JITEngine）
    - 优化器说明（IROptimizer、SIMDOptimizer、AdaptiveOptimizer）
    - 代码缓存说明（TieredCache）
    - 寄存器分配器说明（RegisterAllocator）

- [ ] **F.2.4** 生成vm-accel API文档
  - 工具：`cargo doc -p vm-accel --open`
  - 补充内容：
    - KVM实现说明（KvmAccelerator）
    - HVF实现说明（HvfAccelerator）
    - WHPX实现说明（WhpxAccelerator）
    - Virtualization.framework实现说明（VirtualizationFrameworkAccelerator）

- [ ] **F.2.5** 生成vm-mem API文档
  - 工具：`cargo doc -p vm-mem --open`
  - 补充内容：
    - TLB说明（TlbOptimized）
    - MMU说明（Mmu）
    - 内存管理器说明（MemoryManager）

#### F.3 创建跨架构使用指南
- [ ] **F.3.1** 编写x86_64 ↔ ARM64互运行指南
  - 内容要求：
    - 配置说明（架构选择、翻译器配置）
    - 指令集映射（x86_64 → ARM64、ARM64 → x86_64）
    - 寄存器映射（通用寄存器、特殊寄存器）
    - 常见问题（未支持的指令、性能优化）

- [ ] **F.3.2** 编写ARM64 ↔ RISC-V互运行指南
  - 内容要求：
    - 配置说明（架构选择、翻译器配置）
    - 指令集映射（ARM64 → RISC-V、RISC-V → ARM64）
    - 寄存器映射（通用寄存器、特殊寄存器）
    - 常见问题（未支持的指令、性能优化）

- [ ] **F.3.3** 编写x86_64 ↔ RISC-V互运行指南
  - 内容要求：
    - 配置说明（架构选择、翻译器配置）
    - 指令集映射（x86_64 → RISC-V、RISC-V → x86_64）
    - 寄存器映射（通用寄存器、特殊寄存器）
    - 常见问题（未支持的指令、性能优化）

#### F.4 创建性能调优指南
- [ ] **F.4.1** 编写JIT配置指南
  - 内容要求：
    - 编译策略选择（AOT优先、JIT编译、解释器回退）
    - 优化级别配置（O0-O3）
    - 代码缓存配置（L1/L2/L3大小、命中率阈值）
    - 性能调优建议（热点代码识别、编译开销平衡）

- [ ] **F.4.2** 编写代码缓存调优指南
  - 内容要求：
    - 三层缓存配置（L1: 256KB、L2: 2MB、L3: 64MB）
    - 命中率阈值配置（L1: >1000次、L2: >100次、L3: 全部）
    - 热点代码识别策略
    - 缓存替换策略（LRU、LFU）

- [ ] **F.4.3** 编写内存管理优化指南
  - 内容要求：
    - TLB配置（多级缓存大小、预取策略、替换算法）
    - MMU配置（大页支持：2MB、1GB）
    - NUMA优化（vCPU亲和性、内存节点选择）
    - 内存分配策略（池化、延迟分配）

---

### 阶段G：测试覆盖（P3）

#### G.1 实现单元测试
- [ ] **G.1.1** 实现vm-core单元测试
  - 测试范围：
    - 聚合根状态转换（Created → Running → Paused → Stopped）
    - 领域服务业务逻辑（VmLifecycleService验证逻辑）
    - MMU地址转换（x86_64、AArch64、RISC-V Sv39、RISC-V Sv48）
    - Syscall接口（read、write、open、close等）
  - 目标覆盖率：> 80%

- [ ] **G.1.2** 实现vm-cross-arch单元测试
  - 测试范围：
    - 指令翻译正确性（算术、逻辑、位移、内存、分支、浮点、SIMD、原子）
    - 编码器正确性（目标代码生成）
    - 运行时集成（AOT、JIT、解释器执行）
  - 目标覆盖率：> 85%

- [ ] **G.1.3** 实现vm-engine-jit单元测试
  - 测试范围：
    - IR优化（常量折叠、死代码消除、公共子表达式消除）
    - SIMD优化（AVX2、ARM NEON向量化）
    - 自适应优化（热点检测、分层优化）
    - 代码缓存（L1/L2/L3命中率、替换策略）
    - 寄存器分配（图着色、线性扫描）
  - 目标覆盖率：> 90%

- [ ] **G.1.4** 实现vm-accel单元测试
  - 测试范围：
    - KVM初始化和基本功能（vCPU创建、内存映射、寄存器访问）
    - HVF初始化和基本功能（Intel x86_64、Apple Silicon M系列）
    - WHPX初始化和基本功能（vCPU创建、内存映射、寄存器访问）
    - Virtualization.framework初始化和基本功能（iOS/tvOS）
  - 目标覆盖率：> 75%

- [ ] **G.1.5** 实现vm-mem单元测试
  - 测试范围：
    - TLB多级缓存（命中率、预取效果、替换算法）
    - MMU地址转换（四种架构页表遍历）
    - 大页支持（2MB、1GB映射）
  - 目标覆盖率：> 85%

#### G.2 实现集成测试
- [ ] **G.2.1** 实现跨架构集成测试
  - 测试场景：
    - x86_64 → ARM64完整流程（加载→翻译→执行）
    - ARM64 → RISC-V完整流程（加载→翻译→执行）
    - RISC-V → x86_64完整流程（加载→翻译→执行）
    - 反向测试（ARM64 → x86_64、RISC-V → ARM64、x86_64 → RISC-V）
  - 验收：所有跨架构测试通过，功能完整性100%

- [ ] **G.2.2** 实现JIT/AOT集成测试
  - 测试场景：
    - AOT编译和执行完整流程
    - JIT编译和执行完整流程
    - JIT/AOT混合执行流程
    - 解释器回退流程
  - 验收：所有JIT/AOT测试通过，功能完整性100%

- [ ] **G.2.3** 实现硬件加速集成测试
  - 测试场景：
    - KVM加速完整流程（Linux平台）
    - HVF加速完整流程（macOS平台）
    - WHPX加速完整流程（Windows平台）
    - Virtualization.framework加速完整流程（iOS/tvOS平台）
  - 验收：所有硬件加速测试通过，功能完整性100%

#### G.3 实现性能基准测试
- [ ] **G.3.1** 实现JIT编译性能基准
  - 测试指标：
    - JIT编译速度（指令/秒）
    - JIT编译延迟（编译单个基本块的时间）
    - JIT编译吞吐量（并发编译能力）
  - 目标：编译速度 > 100M指令/秒，编译延迟 < 1ms

- [ ] **G.3.2** 实现执行性能基准
  - 测试指标：
    - 原生执行速度（指令/秒）
    - 解释器执行速度（指令/秒）
    - JIT执行速度（指令/秒）
    - AOT执行速度（指令/秒）
  - 目标：JIT执行速度 > 原生执行速度的70%，AOT执行速度 > 原生执行速度的80%

- [ ] **G.3.3** 实现内存占用基准
  - 测试指标：
    - 基础内存占用（VM创建后的内存占用）
    - 代码缓存内存占用（L1/L2/L3缓存）
    - TLB内存占用（多级缓存）
    - 峰值内存占用（压力测试）
  - 目标：基础内存占用 < 50MB，峰值内存占用 < 500MB

- [ ] **G.3.4** 实现GC暂停时间基准
  - 测试指标：
    - GC触发频率（GC/秒）
    - GC暂停时间（标记、清理、总暂停）
    - GC吞吐量（内存回收/秒）
  - 目标：GC暂停时间 < 10ms，GC吞吐量 > 1GB/秒

- [ ] **G.3.5** 实现TLB性能基准
  - 测试指标：
    - TLB命中率（L1/L2/L3）
    - TLB访问延迟（命中/未命中）
    - TLB预取效果（预取命中率）
    - TLB替换算法效率（LRU/ARC）
  - 目标：TLB综合命中率 > 90%，L1命中率 > 30%，L2命中率 > 40%

- [ ] **G.3.6** 实现代码缓存性能基准
  - 测试指标：
    - 代码缓存命中率（L1/L2/L3）
    - 代码缓存访问延迟（命中/未命中）
    - 热点代码识别准确性（热点代码比例）
  - 目标：代码缓存综合命中率 > 90%，L1命中率 > 30%，L2命中率 > 40%

- [ ] **G.3.7** 实现NUMA性能基准
  - 测试指标：
    - vCPU亲和性效果（本地/远程内存访问延迟）
    - 内存节点分配策略效果（内存带宽利用率）
  - 目标：本地内存访问延迟 < 远程访问延迟的50%

- [ ] **G.3.8** 实现SIMD优化性能基准
  - 测试指标：
    - AVX2向量化效果（性能提升比例）
    - ARM NEON向量化效果（性能提升比例）
  - 目标：SIMD优化性能提升 > 2x

---

### 阶段H：性能优化（P1-P3）

#### H.1 JIT编译优化
- [ ] **H.1.1** 优化寄存器分配算法
  - 当前状态：基础寄存器分配器实现
  - 优化方案：
    - 实现图着色算法（Chaitin-Briggs）
    - 实现线性扫描算法（线性时间复杂度）
    - 根据代码块复杂度自适应选择算法
  - 目标：寄存器溢出率降低 > 30%

- [ ] **H.1.2** 优化指令调度
  - 当前状态：基础指令调度器实现
  - 优化方案：
    - 实现列表调度算法
    - 实现追踪调度算法
    - 根据指令依赖关系优化调度顺序
  - 目标：指令级并行度提升 > 20%

- [ ] **H.1.3** 优化编译流水线
  - 当前状态：四阶段编译流水线（IR优化→寄存器分配→指令调度→代码生成）
  - 优化方案：
    - 实现并行编译（多线程编译不同基本块）
    - 实现增量编译（仅重新编译修改的部分）
    - 实现编译缓存（复用中间结果）
  - 目标：编译速度提升 > 50%

#### H.2 GC暂停优化
- [ ] **H.2.1** 实现增量标记
  - 当前状态：gc-optimizer已实现ParallelMarker，需增量标记
  - 实现方案：
    - 将标记任务拆分为多个小块（每块最多执行5ms）
    - 在安全点调度增量标记任务
    - 记录标记进度，下次继续
  - 目标：单次暂停时间 < 5ms

- [ ] **H.2.2** 实现并发清理
  - 当前状态：gc-optimizer基础清理实现
  - 实现方案：
    - 使用后台线程执行清理任务
    - 应用启动时创建GC线程池
    - 并发清理与业务线程同步
  - 目标：清理阶段暂停时间 < 2ms

- [ ] **H.2.3** 优化自适应配额管理
  - 当前状态：gc-optimizer已实现AdaptiveQuota
  - 优化方案：
    - 根据上次GC时间动态调整本次GC配额
    - 根据内存压力动态调整GC频率
    - 根据CPU利用率动态调整GC线程数
  - 目标：GC总暂停时间降低 > 40%

#### H.3 TLB优化
- [ ] **H.3.1** 优化TLB预取策略
  - 当前状态：tlb_prefetch_smart.rs已实现智能预取
  - 优化方案：
    - 基于访问模式自适应调整预取深度（顺序访问、随机访问）
    - 基于历史命中率调整预取策略
    - 实现预取窗口动态调整
  - 目标：预取命中率提升 > 15%

- [ ] **H.3.2** 优化TLB替换算法
  - 当前状态：基础LRU实现
  - 优化方案：
    - 实现LRU/ARC混合算法（Adaptive Replacement Cache）
    - 实现Bélády算法（OPT）作为参考基准
    - 根据访问模式自适应选择替换策略
  - 目标：TLB综合命中率提升 > 10%

- [ ] **H.3.3** 优化TLB刷新策略
  - 当前状态：tlb_flush_advanced.rs已实现高级刷新
  - 优化方案：
    - 实现批量刷新（减少刷新次数）
    - 实现智能刷新（仅刷新相关条目）
    - 实现延迟刷新（合并多次刷新）
  - 目标：TLB刷新开销降低 > 30%

#### H.4 NUMA优化
- [ ] **H.4.1** 优化vCPU亲和性管理
  - 当前状态：基础NUMA支持
  - 优化方案：
    - 自动检测NUMA拓扑
    - 自动分配vCPU到不同NUMA节点
    - 实现vCPU迁移策略（负载均衡）
  - 目标：NUMA节点负载均衡度 > 80%

- [ ] **H.4.2** 优化内存分配策略
  - 当前状态：基础内存分配
  - 优化方案：
    - 本地优先分配（优先在vCPU所在NUMA节点分配内存）
    - 跨节点分配优化（减少远程内存访问）
    - 大页NUMA感知（大页分配考虑NUMA）
  - 目标：本地内存访问比例 > 70%

#### H.5 调试支持增强
- [ ] **H.5.1** 实现调试接口
  - 实现内容：
    - 断点设置（软件断点、硬件断点）
    - 单步执行（指令级、基本块级）
    - 寄存器查看和修改
    - 内存查看和修改
  - 验收：调试接口功能完整，可与GDB/LLDB集成

- [ ] **H.5.2** 实现日志和监控
  - 实现内容：
    - 结构化日志（tracing crate）
    - 日志级别控制（ERROR/WARN/INFO/DEBUG/TRACE）
    - 日志过滤（模块级别、级别级别）
    - 性能指标收集（JIT编译速度、执行速度、GC暂停时间）
    - 指标暴露（Prometheus格式）
  - 验收：日志和监控系统功能完整，可观测性强

---

## 三、代码证据映射

### P0关键问题代码证据

| 问题 | 文件位置 | 代码证据 |
|------|---------|---------|
| record_event重复定义 | vm-core/src/aggregate_root.rs:45-55 | 两处`pub fn record_event`定义 |
| VmResourceAvailabilityRule导入错误 | vm-core/src/domain_services/vm_lifecycle_service.rs:12-15 | 编译失败 |
| publish_event参数可变性不一致 | vm-core/src/aggregate_root.rs:78-82 | event已move后仍尝试引用 |
| JIT回退解释器 | vm-boot/src/executor/unified.rs:120-135 | `cache: Vec<u8>`仅存储，执行回退runtime.execute_block |
| JIT回退解释器 | vm-boot/src/executor/auto.rs:85-102 | JitExecutor编译后回退runtime.execute_block |
| GC未集成 | vm-boot/Cargo.toml | gc-optimizer未在vm-boot中依赖 |
| 缺少可执行内存管理 | vm-engine-jit/src/core.rs | 无ExecutableMemory实现 |
| 缺少icache刷新 | vm-engine-jit/src/core.rs | 无flush_icache实现 |

### P1重要问题代码证据

| 问题 | 文件位置 | 代码证据 |
|------|---------|---------|
| syscall默认返回Ok(0) | vm-core/src/syscalls/mod.rs | 未实现的syscall默认返回`Ok(0)` |
| KVM feature禁用 | vm-accel/src/kvm_impl.rs:1-5 | 注释为"Temporarily disabled" |
| TLB冗余文件 | vm-mem/src/tlb/*.rs | 9个文件，功能重叠 |
| JIT优化器冗余文件 | vm-engine-jit/src/*.rs | 12个文件，功能重叠 |
| 跨架构优化器冗余文件 | vm-cross-arch/src/optimizers/*.rs | 6个文件，功能重叠 |
| 缺少核心syscall | vm-core/src/syscalls/*.rs | 仅实现write和exit |

### P2基础问题代码证据

| 问题 | 文件位置 | 代码证据 |
|------|---------|---------|
| 缺少README.md | /Users/wangbiao/Desktop/project/vm/ | 项目根无README.md |
| 缺少API文档 | docs/ | 无API文档目录 |
| 中间文件 | vm-core/fix_*、build_errors.txt等 | 大量临时文件 |

### P3质量保障代码证据

| 问题 | 文件位置 | 代码证据 |
|------|---------|---------|
| 单元测试覆盖不足 | **/tests/*.rs | 测试文件少 |
| 集成测试覆盖不足 | tests/integration_tests.rs | 无集成测试 |
| 性能基准缺失 | benches/*.rs | 无性能基准测试 |

---

## 四、闭环验收门槛

### 阶段A验收门槛
- [ ] vm-core编译无错误和警告
- [ ] 所有中间文件已删除
- [ ] `cargo build -p vm-core`成功

### 阶段B验收门槛
- [ ] JIT/AOT编译的代码直接执行，不再回退解释器
- [ ] JIT可执行内存管理正确实现（W^X保护、icache刷新）
- [ ] GC集成到vm-boot，优化功能生效
- [ ] GC暂停时间 < 10ms（高负载场景）

### 阶段C验收门槛
- [ ] 核心syscall集正确实现（文件系统、进程管理、网络、misc）
- [ ] syscall错误传播机制正确实现（返回正确错误码，不产生静默错误）
- [ ] syscall测试覆盖 > 80%

### 阶段D验收门槛
- [ ] KVM feature正确启用，Linux平台功能正常
- [ ] HVF ARM64支持增强，Apple Silicon平台性能提升 > 20%
- [ ] WHPX基本支持完整，Windows平台功能正常
- [ ] Virtualization.framework支持正确，iOS/tvOS平台功能正常

### 阶段E验收门槛
- [ ] TLB模块：9个文件 → 6个文件，减少2600-3700行
- [ ] JIT优化器：12个文件 → 4个文件，减少1900-2900行
- [ ] 跨架构优化器：6个文件 → 3个文件，减少800-1400行
- [ ] 功能完整性100%保留，性能无回退

### 阶段F验收门槛
- [ ] README.md完整（项目概述、架构说明、构建指南、使用示例、贡献指南）
- [ ] 各模块API文档完整（vm-core、vm-cross-arch、vm-engine-jit、vm-accel、vm-mem）
- [ ] 跨架构使用指南完整（x86_64↔ARM64、ARM64↔RISC-V、x86_64↔RISC-V）
- [ ] 性能调优指南完整（JIT配置、代码缓存调优、内存管理优化）

### 阶段G验收门槛
- [ ] 单元测试覆盖率 > 80%（vm-core）、> 85%（vm-cross-arch、vm-mem）、> 90%（vm-engine-jit）、> 75%（vm-accel）
- [ ] 集成测试覆盖所有跨架构、JIT/AOT、硬件加速场景
- [ ] 性能基准测试覆盖JIT编译速度、执行性能、内存占用、GC暂停时间、TLB性能、代码缓存性能、NUMA性能、SIMD优化

### 阶段H验收门槛
- [ ] JIT编译速度提升 > 50%
- [ ] GC总暂停时间降低 > 40%，单次暂停时间 < 10ms
- [ ] TLB综合命中率提升 > 10%，预取命中率提升 > 15%
- [ ] JIT执行速度 > 原生执行速度的70%，AOT执行速度 > 原生执行速度的80%
- [ ] 调试接口功能完整，可与GDB/LLDB集成
- [ ] 日志和监控系统功能完整，可观测性强

---

## 五、实施时间表

| 阶段 | 开始时间 | 结束时间 | 周期 |
|------|---------|---------|------|
| A：紧急修复 | Week 1 | Week 1 | 1周 |
| B：核心功能闭环 | Week 2 | Week 4 | 3周 |
| C：系统调用完善 | Week 5 | Week 6 | 2周 |
| D：硬件加速激活 | Week 5 | Week 6 | 2周（与C并行） |
| E：代码重构优化 | Week 7 | Week 10 | 4周 |
| F：文档建设 | Week 7 | Week 8 | 2周（与E并行） |
| G：测试覆盖 | Week 7 | Week 10 | 4周（与E并行） |
| H：性能优化 | Week 11 | Week 18 | 8周 |

**总计：18周（约4.5个月）**

---

## 六、资源分配建议

| 角色 | 任务分配 | 工作量 |
|------|---------|--------|
| **核心开发** | 阶段A、B、C、D（P0-P1关键问题） | 10周 |
| **代码重构** | 阶段E（代码重构优化） | 4周 |
| **文档工程师** | 阶段F（文档建设） | 2周 |
| **测试工程师** | 阶段G（测试覆盖） | 4周 |
| **性能优化** | 阶段H（性能优化） | 8周（可并行） |

**建议团队规模：5-7人（2名核心开发、1名代码重构、1名文档工程师、1名测试工程师、1-2名性能优化）**

---

## 七、风险评估与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|---------|
| vm-core编译修复复杂度高 | 高 | 中 | 提前预留2周缓冲时间，必要时简化DDD模型 |
| JIT/AOT闭环实现难度大 | 高 | 中 | 分阶段实现，先实现基础功能，再优化 |
| GC集成性能回退 | 中 | 中 | 充分测试，准备回退方案 |
| 硬件加速平台兼容性问题 | 中 | 高 | 提前在所有目标平台测试 |
| 代码重构引入新Bug | 高 | 中 | 充分单元测试，代码Review |
| 性能优化效果不明显 | 中 | 中 | 基准测试驱动，持续优化 |

---

## 八、附录：现有文档映射

| 文档 | 映射到本计划 | 备注 |
|------|-------------|------|
| IMPLEMENTATION_PLAN_AND_TODOS.md | 阶段A-H | 基础框架 |
| IMPLEMENTATION_PLAN_AND_TODOS_V2.md | 代码证据映射 | 补充代码位置和证据 |
| Rust虚拟机性能优化与代码重构计划.md | 阶段E、H | 性能优化和代码重构 |
| Rust虚拟机软件改进实施计划.md | 阶段A-G | 优先级划分和任务分解 |
| 全面架构审查报告.md | 所有阶段 | 问题识别和优先级 |

---

## 九、版本历史

| 版本 | 日期 | 变更说明 |
|------|------|---------|
| V1.0 | 2025-12-23 | 初始版本，综合审查报告和现有实施计划文档制定 |

---

**本实施计划涵盖所有关键问题，确保不遗漏任何信息。**
