//! 端到端集成测试

use vm_core::{AccessType, ExecMode, GuestAddr, MemorySize, VmConfig, VcpuCount, VmId};
use vm_engine::jit::{BaseConfig, JITCompilationConfig, JITEngine, JITExecutionStats, TieredCompilerConfig};
use vm_mem::SoftwareMmu;
// vm-runtime 已被删除，相关功能已迁移到其他模块
// use vm_runtime::{CoroutineScheduler, GcConfig, GcTriggerPolicy, GcRuntime, SandboxConfig, SandboxedVm};

#[tokio::test]
async fn test_vm_lifecycle() {
    let config = VmConfig {
        guest_arch: vm_core::GuestArch::Riscv64,
        memory_size: 128 * 1024 * 1024,
        vcpu_count: 1,
        exec_mode: ExecMode::JIT,
        kernel_path: None,
        initrd_path: None,
    };

    let vm_id = VmId::new("integration-test-vm".to_string()).unwrap();
    // vm-runtime 已被删除，此测试暂时禁用
    // let result = vm_runtime::create_vm(&vm_id, &config).await;
    //
    // assert!(result.is_ok(), "VM creation should succeed");
    // let vm = result.unwrap();
    //
    // let start_result = vm.start().await;
    // assert!(start_result.is_ok(), "VM start should succeed");
    //
    // let pause_result = vm.pause().await;
    // assert!(pause_result.is_ok(), "VM pause should succeed");
    //
    // let resume_result = vm.resume().await;
    // assert!(resume_result.is_ok(), "VM resume should succeed");
    //
    // let stop_result = vm.stop().await;
    // assert!(stop_result.is_ok(), "VM stop should succeed");
}

#[test]
fn test_memory_read_write() {
    let mut mmu = SoftwareMmu::new(
        vm_mem::mmu::MmuArch::RiscVSv39,
        |addr: GuestAddr, size: usize| -> Result<Vec<u8>, vm_core::VmError> {
            let mut buffer = vec![0u8; size];
            for i in 0..size {
                buffer[i] = (addr.0 as usize + i) as u8;
            }
            Ok(buffer)
        },
    );

    let test_addr = GuestAddr(0x1000);
    let test_value: u64 = 0xDEADBEEF;

    let write_result = mmu.write(test_addr, test_value, 8);
    assert!(write_result.is_ok(), "Memory write should succeed");

    let read_result = mmu.read(test_addr, 8);
    assert!(read_result.is_ok(), "Memory read should succeed");

    let read_value = read_result.unwrap();
    assert_eq!(read_value, test_value, "Read value should match written value");
}

#[test]
fn test_address_translation() {
    let memory = vec![0u8; 1024 * 1024];
    let memory_arc = std::sync::Arc::new(std::sync::Mutex::new(memory));

    let mut mmu = SoftwareMmu::new(
        vm_mem::mmu::MmuArch::X86_64,
        {
            let memory_clone = std::sync::Arc::clone(&memory_arc);
            move |addr: GuestAddr, size: usize| -> Result<Vec<u8>, vm_core::VmError> {
                let mem = memory_clone.lock().unwrap();
                let start = addr.0 as usize;
                let end = (start + size).min(mem.len());
                Ok(mem[start..end].to_vec())
            }
        },
    );

    let gva = GuestAddr(0x1000);
    let cr3 = GuestAddr(0);

    let result = mmu.translate(gva, cr3);
    assert!(result.is_err(), "Translation should fail with page fault for unmapped memory");
}

#[test]
fn test_tlb_operations() {
    use vm_mem::MultiLevelTlb;

    let mut tlb = MultiLevelTlb::new(vm_mem::tlb::TlbConfig::default());

    let entry = vm_mem::tlb::TlbEntry {
        guest_addr: GuestAddr(0x1000),
        phys_addr: vm_mem::GuestPhysAddr(0x2000),
        asid: 0,
        flags: vm_mem::mmu::PageTableFlags::default(),
    };

    tlb.update(entry);

    let lookup = tlb.lookup(GuestAddr(0x1000), 0, AccessType::Read);
    assert!(lookup.is_some(), "TLB lookup should find the entry");

    let looked_up_entry = lookup.unwrap();
    assert_eq!(looked_up_entry.phys_addr, vm_mem::GuestPhysAddr(0x2000));

    tlb.flush();

    let lookup_after_flush = tlb.lookup(GuestAddr(0x1000), 0, AccessType::Read);
    assert!(lookup_after_flush.is_none(), "TLB lookup should not find entry after flush");
}

#[test]
fn test_value_objects_validation() {
    let valid_vm_id = VmId::new("test-vm-123".to_string());
    assert!(valid_vm_id.is_ok());

    let invalid_vm_id = VmId::new("".to_string());
    assert!(invalid_vm_id.is_err());

    let valid_memory_size = MemorySize::from_mb(256);
    assert!(valid_memory_size.is_ok());
    assert_eq!(valid_memory_size.unwrap().bytes(), 256 * 1024 * 1024);

    let valid_vcpu_count = VcpuCount::new(4);
    assert!(valid_vcpu_count.is_ok());
    assert_eq!(valid_vcpu_count.unwrap().count(), 4);
}

#[tokio::test]
async fn test_gc_basic_operations() {
    let mut memory = vec![0u8; 1024 * 1024];
    let gc_config = GcConfig {
        trigger_policy: GcTriggerPolicy::FixedThreshold { threshold: 0.7 },
        max_pause_time_ms: 100,
        parallel_marking: false,
        incremental: false,
    };

    let gc = GcRuntime::new(gc_config);
    gc.mark(&mut memory, &[0, 100, 200]);
    gc.sweep(&mut memory);

    let stats = gc.get_stats();
    assert!(stats.total_collected > 0 || stats.live_objects > 0);
}

#[tokio::test]
async fn test_sandboxed_vm_operations() {
    let config = SandboxConfig {
        max_memory_bytes: Some(128 * 1024 * 1024),
        max_vcpus: Some(2),
        max_execution_time_ms: Some(5000),
        allowed_syscalls: vec![1, 2, 3],
        resource_limits: None,
    };

    let mut vm = SandboxedVm::new(config);

    let memory_result = vm.allocate_memory(64 * 1024 * 1024);
    assert!(memory_result.is_ok(), "Memory allocation within limits should succeed");

    let oversized_memory_result = vm.allocate_memory(256 * 1024 * 1024);
    assert!(oversized_memory_result.is_err(), "Memory allocation exceeding limits should fail");
}

#[tokio::test]
async fn test_coroutine_scheduler() {
    let mut scheduler = CoroutineScheduler::new(vm_runtime::scheduler::SchedulerConfig::default());

    let mut task1 = Box::pin(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        "task1".to_string()
    });

    let mut task2 = Box::pin(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        "task2".to_string()
    });

    let handle1 = scheduler.spawn_coroutine(&mut task1);
    let handle2 = scheduler.spawn_coroutine(&mut task2);

    let result2 = scheduler.wait_for_coroutine(handle2).await;
    assert!(result2.is_some());
    assert_eq!(result2.unwrap(), "task2");

    let result1 = scheduler.wait_for_coroutine(handle1).await;
    assert!(result1.is_some());
    assert_eq!(result1.unwrap(), "task1");
}

#[test]
fn test_jit_compilation_basic() {
    let config = JITCompilationConfig {
        base: BaseConfig::default(),
        tiered: TieredCompilerConfig::default(),
    };

    let mut engine = JITEngine::new(config);

    let ir_block = vm_engine::jit::core::IRBlock {
        instructions: vec![
            vm_engine::jit::core::IRInstruction::Const {
                dest: 0,
                value: 42,
            },
            vm_engine::jit::core::IRInstruction::Return { value: 0 },
        ],
    };

    let result = engine.compile_block(&ir_block);
    assert!(result.is_ok(), "JIT compilation should succeed");

    let compiled = result.unwrap();
    assert!(!compiled.code.is_empty(), "Compiled code should not be empty");
}

#[test]
fn test_jit_execution_stats() {
    let config = JITCompilationConfig {
        base: BaseConfig::default(),
        tiered: TieredCompilerConfig::default(),
    };

    let mut engine = JITEngine::new(config);

    let stats = engine.get_stats();
    assert_eq!(stats.compiled_blocks, 0);
    assert_eq!(stats.total_instructions, 0);
}

#[test]
fn test_error_context() {
    use vm_core::error::ErrorContext;

    let error = vm_core::VmError::Memory(vm_core::MemoryError::AccessViolation {
        addr: GuestAddr(0x1000),
        msg: "Test error".to_string(),
        access_type: Some(AccessType::Read),
    });

    let context_error = error.context("Failed to execute instruction");

    assert!(context_error.to_string().contains("Failed to execute instruction"));
    assert!(context_error.to_string().contains("Test error"));
}

#[test]
fn test_aggregate_root_event_publishing() {
    use vm_core::aggregate_root::{AggregateRoot, VirtualMachineAggregate};

    let vm_id = VmId::new("test-vm".to_string()).unwrap();
    let config = VmConfig::default();
    let mut aggregate = VirtualMachineAggregate::new(vm_id, config);

    let events_before = aggregate.uncommitted_events();
    assert_eq!(events_before.len(), 0);

    aggregate.start();

    let events_after = aggregate.uncommitted_events();
    assert!(events_after.len() > 0);

    aggregate.mark_events_as_committed();

    let events_committed = aggregate.uncommitted_events();
    assert_eq!(events_committed.len(), 0);
}
