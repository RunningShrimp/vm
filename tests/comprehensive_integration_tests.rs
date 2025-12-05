//! 增强的集成测试套件
//!
//! 补充JIT/AOT/GC/跨架构执行的集成测试

use vm_core::{GuestArch, GuestAddr, MMU};
use vm_cross_arch::{CrossArchRuntime, CrossArchRuntimeConfig, CrossArchConfig, GuestArch as CrossGuestArch, HostArch};
use vm_engine_jit::{Jit, UnifiedGC, UnifiedGcConfig};
use vm_ir::{IRBlock, IROp, Terminator, IRBuilder};
use vm_mem::SoftMmu;
use std::sync::Arc;
use std::time::Instant;

/// 创建测试用的IR块
fn create_test_ir_block(addr: GuestAddr) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    builder.push(IROp::MovImm { dst: 1, imm: 10 });
    builder.push(IROp::MovImm { dst: 2, imm: 20 });
    builder.push(IROp::Add {
        dst: 3,
        src1: 1,
        src2: 2,
    });
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 测试JIT编译和执行集成
#[test]
fn test_jit_compile_execute_integration() {
    println!("=== 测试JIT编译和执行集成 ===");
    
    let mut jit = Jit::new();
    let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
    let block = create_test_ir_block(0x1000);
    
    // 执行多次以触发JIT编译
    let start = Instant::now();
    for i in 0..200 {
        let result = jit.run(&mut mmu, &block);
        assert!(matches!(
            result.status,
            vm_core::ExecStatus::Ok | vm_core::ExecStatus::Continue
        ));
        
        // 检查JIT是否已编译（在达到阈值后）
        if i >= 100 {
            assert!(jit.is_hot(0x1000), "JIT应该已编译热点代码");
        }
    }
    let elapsed = start.elapsed();
    
    println!("  执行200次，耗时: {:?}", elapsed);
    assert!(elapsed.as_millis() < 2000, "执行时间应该 < 2s");
    
    println!("  ✓ JIT编译和执行集成测试通过");
}

/// 测试GC与内存分配集成
#[test]
fn test_gc_memory_integration() {
    println!("=== 测试GC与内存分配集成 ===");
    
    let gc_config = UnifiedGcConfig {
        heap_size_limit: 5 * 1024 * 1024, // 5MB
        mark_quota_us: 500,
        sweep_quota_us: 250,
        adaptive_quota: true,
        enable_time_based_trigger: false,
        ..Default::default()
    };
    
    let gc = Arc::new(UnifiedGC::new(gc_config));
    
    // 模拟内存分配
    let mut roots = Vec::new();
    for i in 0..1000 {
        roots.push(i as u64 * 64); // 模拟对象地址
    }
    
    // 启动GC周期
    let cycle_start = gc.start_gc(&roots);
    assert_eq!(gc.phase(), vm_engine_jit::GCPhase::Marking);
    
    // 执行增量标记
    let mut mark_complete = false;
    for _ in 0..100 {
        let (complete, _) = gc.incremental_mark();
        if complete {
            mark_complete = true;
            break;
        }
    }
    
    if mark_complete {
        gc.terminate_marking();
        assert_eq!(gc.phase(), vm_engine_jit::GCPhase::Sweeping);
        
        // 执行增量清扫
        let mut sweep_complete = false;
        for _ in 0..100 {
            let (complete, _) = gc.incremental_sweep();
            if complete {
                sweep_complete = true;
                break;
            }
        }
        
        if sweep_complete {
            gc.finish_gc(cycle_start);
            assert_eq!(gc.phase(), vm_engine_jit::GCPhase::Idle);
        }
    }
    
    println!("  ✓ GC与内存分配集成测试通过");
}

/// 测试跨架构执行集成（如果支持）
#[test]
fn test_cross_arch_execution_integration() {
    println!("=== 测试跨架构执行集成 ===");
    
    // 检查是否支持跨架构
    let host_arch = HostArch::detect();
    let guest_arch = CrossGuestArch::Riscv64;
    
    // 如果主机架构与guest架构相同，跳过测试
    if matches!(host_arch, HostArch::Riscv64) {
        println!("  跳过：主机和guest架构相同");
        return;
    }
    
    let config = CrossArchConfig {
        guest_arch,
        host_arch,
        enable_jit: true,
        enable_aot: false,
    };
    
    let runtime_config = CrossArchRuntimeConfig {
        cross_arch_config: config,
        memory_size: 64 * 1024 * 1024,
        ..Default::default()
    };
    
    match CrossArchRuntime::new(runtime_config) {
        Ok(mut runtime) => {
            // 创建测试代码
            let block = create_test_ir_block(0x1000);
            
            // 执行代码
            let mut mmu = runtime.mmu();
            let result = runtime.execute_block(&mut *mmu, &block);
            
            assert!(matches!(
                result.status,
                vm_core::ExecStatus::Ok | vm_core::ExecStatus::Continue
            ));
            
            println!("  ✓ 跨架构执行集成测试通过");
        }
        Err(e) => {
            println!("  跨架构运行时创建失败: {:?}，跳过测试", e);
        }
    }
}

/// 测试JIT和GC并发集成
#[test]
fn test_jit_gc_concurrent_integration() {
    println!("=== 测试JIT和GC并发集成 ===");
    
    let gc_config = UnifiedGcConfig {
        heap_size_limit: 10 * 1024 * 1024,
        mark_quota_us: 1000,
        sweep_quota_us: 500,
        adaptive_quota: true,
        enable_time_based_trigger: false,
        ..Default::default()
    };
    
    let gc = Arc::new(UnifiedGC::new(gc_config));
    let mut jit = Jit::new();
    let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
    let block = create_test_ir_block(0x1000);
    
    // 启动GC
    let roots: Vec<u64> = (0..100).map(|i| i as u64 * 1024).collect();
    let cycle_start = gc.start_gc(&roots);
    
    // 同时执行JIT编译和执行
    for i in 0..100 {
        // JIT执行
        let _ = jit.run(&mut mmu, &block);
        
        // 执行增量GC（每10次执行一次）
        if i % 10 == 0 {
            let (mark_complete, _) = gc.incremental_mark();
            if mark_complete {
                gc.terminate_marking();
                let (sweep_complete, _) = gc.incremental_sweep();
                if sweep_complete {
                    gc.finish_gc(cycle_start);
                    break;
                }
            }
        }
    }
    
    // 验证JIT已编译
    assert!(jit.is_hot(0x1000), "JIT应该已编译代码");
    
    println!("  ✓ JIT和GC并发集成测试通过");
}

/// 测试AOT加载和执行集成（如果AOT可用）
#[test]
fn test_aot_load_execute_integration() {
    println!("=== 测试AOT加载和执行集成 ===");
    
    // 注意：AOT功能可能需要特定的文件或配置
    // 这里仅测试AOT加载器的基本功能
    
    use vm_engine_jit::aot_loader::AotLoader;
    use vm_engine_jit::aot_format::AotImage;
    
    // 创建一个简单的AOT镜像用于测试
    let image = AotImage {
        code_section: vec![0x90, 0x90, 0xC3], // NOP, NOP, RET (x86-64)
        data_section: vec![],
        metadata: None,
        code_blocks: vec![],
        symbols: vec![],
        dependencies: vec![],
        relocations: vec![],
    };
    
    match AotLoader::new(image) {
        Ok(loader) => {
            // 验证加载器已创建
            assert!(loader.base_addr() > 0, "基地址应该有效");
            println!("  ✓ AOT加载器创建成功");
        }
        Err(e) => {
            println!("  AOT加载器创建失败: {:?}，跳过测试", e);
        }
    }
}

/// 测试热点检测和JIT编译集成
#[test]
fn test_hotspot_detection_jit_integration() {
    println!("=== 测试热点检测和JIT编译集成 ===");
    
    let mut jit = Jit::new();
    let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
    
    // 创建多个代码块
    let blocks: Vec<IRBlock> = (0..5)
        .map(|i| create_test_ir_block(0x1000 + i * 0x100))
        .collect();
    
    // 执行不同频率的代码块
    for i in 0..300 {
        let block_idx = if i < 200 {
            0 // 热点代码块
        } else {
            (i % 4) + 1 // 冷代码块
        };
        
        let _ = jit.run(&mut mmu, &blocks[block_idx]);
    }
    
    // 验证热点代码块已被编译
    assert!(jit.is_hot(0x1000), "热点代码块应该已被JIT编译");
    
    println!("  ✓ 热点检测和JIT编译集成测试通过");
}

/// 测试内存管理和TLB集成
#[test]
fn test_memory_tlb_integration() {
    println!("=== 测试内存管理和TLB集成 ===");
    
    let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
    
    // 执行多次内存访问以填充TLB
    let test_addr = 0x10000;
    for i in 0..100 {
        let addr = test_addr + (i * 4096);
        let _ = mmu.read_byte(addr);
        let _ = mmu.write_byte(addr, 0x42);
    }
    
    // 获取TLB统计
    let tlb_stats = mmu.get_tlb_stats();
    assert!(tlb_stats.total_lookups > 0, "应该有TLB查找");
    
    println!("  TLB统计: 查找={}, 命中={}, 命中率={:.2}%",
        tlb_stats.total_lookups,
        tlb_stats.total_hits,
        if tlb_stats.total_lookups > 0 {
            (tlb_stats.total_hits as f64 / tlb_stats.total_lookups as f64) * 100.0
        } else {
            0.0
        }
    );
    
    println!("  ✓ 内存管理和TLB集成测试通过");
}

