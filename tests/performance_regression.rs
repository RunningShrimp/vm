//! 性能回归测试
//!
//! 检测性能退化，确保新代码不会显著降低性能

use vm_cross_arch::UnifiedExecutor;
use vm_core::{GuestArch, MMU};
use std::time::Instant;
use std::collections::HashMap;

/// 性能阈值配置
#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    /// 平均执行时间阈值（微秒）
    pub avg_execution_time_us: u64,
    /// 最大执行时间阈值（微秒）
    pub max_execution_time_us: u64,
    /// JIT编译时间阈值（微秒）
    pub jit_compile_time_us: u64,
    /// GC暂停时间阈值（微秒）
    pub gc_pause_time_us: u64,
    /// TLB命中率阈值（百分比）
    pub tlb_hit_rate_percent: f64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            avg_execution_time_us: 1000,  // 1ms
            max_execution_time_us: 5000,  // 5ms
            jit_compile_time_us: 10000,    // 10ms
            gc_pause_time_us: 1000,       // 1ms
            tlb_hit_rate_percent: 90.0,   // 90%
        }
    }
}

/// 性能基准线
#[derive(Debug, Clone)]
pub struct PerformanceBaseline {
    /// 基准名称
    pub name: String,
    /// 阈值配置
    pub thresholds: PerformanceThresholds,
    /// 历史性能数据
    pub historical_data: Vec<PerformanceSnapshot>,
}

/// 性能快照
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    /// 时间戳
    pub timestamp: u64,
    /// 平均执行时间（微秒）
    pub avg_execution_time_us: u64,
    /// 最大执行时间（微秒）
    pub max_execution_time_us: u64,
    /// JIT编译时间（微秒）
    pub jit_compile_time_us: u64,
    /// GC暂停时间（微秒）
    pub gc_pause_time_us: u64,
    /// TLB命中率（百分比）
    pub tlb_hit_rate_percent: f64,
}

impl PerformanceBaseline {
    /// 创建新的性能基准线
    pub fn new(name: String, thresholds: PerformanceThresholds) -> Self {
        Self {
            name,
            thresholds,
            historical_data: Vec::new(),
        }
    }

    /// 记录性能快照
    pub fn record_snapshot(&mut self, snapshot: PerformanceSnapshot) {
        self.historical_data.push(snapshot);
        // 保留最近1000个快照
        if self.historical_data.len() > 1000 {
            self.historical_data.remove(0);
        }
    }

    /// 检查性能是否退化
    pub fn check_regression(&self, snapshot: &PerformanceSnapshot) -> Vec<String> {
        let mut regressions = Vec::new();

        if snapshot.avg_execution_time_us > self.thresholds.avg_execution_time_us {
            regressions.push(format!(
                "平均执行时间 {}us 超过阈值 {}us",
                snapshot.avg_execution_time_us,
                self.thresholds.avg_execution_time_us
            ));
        }

        if snapshot.max_execution_time_us > self.thresholds.max_execution_time_us {
            regressions.push(format!(
                "最大执行时间 {}us 超过阈值 {}us",
                snapshot.max_execution_time_us,
                self.thresholds.max_execution_time_us
            ));
        }

        if snapshot.jit_compile_time_us > self.thresholds.jit_compile_time_us {
            regressions.push(format!(
                "JIT编译时间 {}us 超过阈值 {}us",
                snapshot.jit_compile_time_us,
                self.thresholds.jit_compile_time_us
            ));
        }

        if snapshot.gc_pause_time_us > self.thresholds.gc_pause_time_us {
            regressions.push(format!(
                "GC暂停时间 {}us 超过阈值 {}us",
                snapshot.gc_pause_time_us,
                self.thresholds.gc_pause_time_us
            ));
        }

        if snapshot.tlb_hit_rate_percent < self.thresholds.tlb_hit_rate_percent {
            regressions.push(format!(
                "TLB命中率 {:.2}% 低于阈值 {:.2}%",
                snapshot.tlb_hit_rate_percent,
                self.thresholds.tlb_hit_rate_percent
            ));
        }

        regressions
    }
}

/// 测试执行性能
#[test]
fn test_execution_performance() {
    let thresholds = PerformanceThresholds::default();
    let mut baseline = PerformanceBaseline::new("execution".to_string(), thresholds.clone());
    
    let mut executor = UnifiedExecutor::auto_create(GuestArch::X86_64, 128 * 1024 * 1024)
        .expect("创建执行器失败");
    
    let code_base: u64 = 0x1000;
    let test_code = create_performance_test_code();
    
    // 加载代码
    for (i, byte) in test_code.iter().enumerate() {
        executor.mmu_mut().write(code_base + i as u64, *byte as u64, 1)
            .expect("写入内存失败");
    }
    
    // 预热
    for _ in 0..100 {
        executor.execute(code_base).expect("执行失败");
    }
    
    // 性能测试
    let iterations = 1000;
    let mut times = Vec::with_capacity(iterations);
    
    for _ in 0..iterations {
        let start = Instant::now();
        executor.execute(code_base).expect("执行失败");
        let elapsed = start.elapsed();
        times.push(elapsed.as_micros() as u64);
    }
    
    // 计算统计信息
    let avg_time = times.iter().sum::<u64>() / iterations as u64;
    let max_time = *times.iter().max().unwrap();
    
    let snapshot = PerformanceSnapshot {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        avg_execution_time_us: avg_time,
        max_execution_time_us: max_time,
        jit_compile_time_us: 0,
        gc_pause_time_us: 0,
        tlb_hit_rate_percent: 95.0,
    };

    baseline.record_snapshot(snapshot.clone());
    
    let regressions = baseline.check_regression(&snapshot);
    assert!(regressions.is_empty(), "性能回归: {:?}", regressions);
}

/// 测试JIT编译性能
#[test]
fn test_jit_compile_performance() {
    use vm_engine::jit::Jit;
    use vm_ir::{IRBlock, IROp, Terminator};
    use vm_mem::SoftMmu;
    
    let thresholds = PerformanceThresholds::default();
    let mut baseline = PerformanceBaseline::new("jit_compile".to_string(), thresholds.clone());
    
    let mut jit = Jit::new();
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    
    // 创建测试IR块
    let block = IRBlock {
        start_pc: 0x1000,
        ops: vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::MovImm { dst: 2, imm: 20 },
            IROp::Add { dst: 3, src1: 1, src2: 2 },
            IROp::Sub { dst: 4, src1: 3, src2: 1 },
            IROp::Mul { dst: 5, src1: 2, src2: 2 },
        ],
        term: Terminator::Ret,
    };
    
    // 测试编译时间
    let start = Instant::now();
    
    // 触发编译（通过多次执行）
    for _ in 0..150 {  // 超过HOT_THRESHOLD (100)
        let _ = jit.run(&mut mmu, &block);
    }
    
    let elapsed = start.elapsed();
    let compile_time_us = elapsed.as_micros() as u64;
    
    let snapshot = PerformanceSnapshot {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        avg_execution_time_us: 0,
        max_execution_time_us: 0,
        jit_compile_time_us: compile_time_us,
        gc_pause_time_us: 0,
        tlb_hit_rate_percent: 95.0,
    };

    baseline.record_snapshot(snapshot.clone());
    
    let regressions = baseline.check_regression(&snapshot);
    assert!(regressions.is_empty(), "性能回归: {:?}", regressions);
}

/// 测试GC性能
#[test]
fn test_gc_performance() {
    use vm_engine::jit::{UnifiedGC, UnifiedGcConfig};
    
    let thresholds = PerformanceThresholds::default();
    let mut baseline = PerformanceBaseline::new("gc".to_string(), thresholds.clone());
    
    let config = UnifiedGcConfig {
        heap_size_limit: 10 * 1024 * 1024, // 10MB
        mark_quota_us: 1000,
        sweep_quota_us: 500,
        adaptive_quota: true,
        ..Default::default()
    };
    
    let gc = UnifiedGC::new(config);
    
    // 模拟对象分配
    let roots: Vec<u64> = (0..1000).map(|i| i as u64 * 1024).collect();
    
    // 执行GC周期并测量暂停时间
    let mut pause_times = Vec::new();
    
    for _ in 0..10 {
        let cycle_start = gc.start_gc(&roots);
        
        // 增量标记
        loop {
            let (complete, _) = gc.incremental_mark();
            if complete {
                break;
            }
        }
        
        gc.terminate_marking();
        
        // 增量清扫
        loop {
            let (complete, _) = gc.incremental_sweep();
            if complete {
                break;
            }
        }
        
        gc.finish_gc(cycle_start);
        
        let stats = gc.stats();
        pause_times.push(stats.get_last_pause_us());
    }
    
    let avg_pause = pause_times.iter().sum::<u64>() / pause_times.len() as u64;
    let max_pause = *pause_times.iter().max().unwrap();
    
    let snapshot = PerformanceSnapshot {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        avg_execution_time_us: 0,
        max_execution_time_us: 0,
        jit_compile_time_us: 0,
        gc_pause_time_us: avg_pause,
        tlb_hit_rate_percent: 95.0,
    };

    baseline.record_snapshot(snapshot.clone());
    
    let regressions = baseline.check_regression(&snapshot);
    assert!(regressions.is_empty(), "性能回归: {:?}", regressions);
}

/// 测试寄存器分配器性能
#[test]
fn test_register_allocator_performance() {
    use vm_engine::jit::register_allocator::{LinearScanAllocator, GraphColoringAllocator, RegisterAllocatorTrait};
    use vm_ir::{IROp, IRBuilder, Terminator};
    
    let thresholds = PerformanceThresholds::default();
    let mut baseline = PerformanceBaseline::new("register_allocator".to_string(), thresholds.clone());
    
    // 创建测试IR块
    let mut builder = IRBuilder::new(0x1000);
    for i in 0..50 {
        builder.push(IROp::MovImm {
            dst: i as u32,
            imm: i as u64,
        });
        if i > 0 {
            builder.push(IROp::Add {
                dst: (i + 50) as u32,
                src1: (i - 1) as u32,
                src2: i as u32,
            });
        }
    }
    builder.set_term(Terminator::Ret);
    let block = builder.build();
    
    // 测试线性扫描分配器
    let start = Instant::now();
    let mut linear_allocator = LinearScanAllocator::new();
    linear_allocator.analyze_lifetimes(&block.ops);
    let _allocations = linear_allocator.allocate_registers(&block.ops);
    let linear_time = start.elapsed().as_micros() as u64;
    
    // 测试图着色分配器
    let start = Instant::now();
    let mut graph_allocator = GraphColoringAllocator::new();
    graph_allocator.analyze_lifetimes(&block.ops);
    let _allocations = graph_allocator.allocate_registers(&block.ops);
    let graph_time = start.elapsed().as_micros() as u64;
    
    println!("  线性扫描: {}us, 图着色: {}us", linear_time, graph_time);
    
    // 验证性能阈值
    assert!(linear_time < 10000, "线性扫描应该 < 10ms");
    assert!(graph_time < 50000, "图着色应该 < 50ms");
}

/// 测试IR工具性能
#[test]
fn test_ir_utils_performance() {
    use vm_engine::jit::ir_utils::IrAnalyzer;
    use vm_ir::IROp;
    
    // 创建大量IR操作
    let ops: Vec<IROp> = (0..1000)
        .map(|i| {
            if i % 2 == 0 {
                IROp::Add {
                    dst: (i % 32) as u32,
                    src1: ((i + 1) % 32) as u32,
                    src2: ((i + 2) % 32) as u32,
                }
            } else {
                IROp::Load {
                    dst: (i % 32) as u32,
                    base: ((i + 1) % 32) as u32,
                    offset: i as i64,
                    size: 8,
                    flags: Default::default(),
                }
            }
        })
        .collect();
    
    let start = Instant::now();
    for op in &ops {
        let _ = IrAnalyzer::collect_read_regs(op);
        let _ = IrAnalyzer::collect_written_regs(op);
        let _ = IrAnalyzer::is_memory_access(op);
        let _ = IrAnalyzer::is_branch(op);
    }
    let elapsed = start.elapsed().as_micros() as u64;
    
    println!("  IR工具处理1000个操作耗时: {}us", elapsed);
    assert!(elapsed < 10000, "IR工具处理应该 < 10ms");
}

/// 测试GC模块性能
#[test]
fn test_gc_module_performance() {
    use vm_engine::jit::gc_marker::GcMarker;
    use vm_engine::jit::gc_sweeper::GcSweeper;
    use vm_engine::jit::unified_gc::{LockFreeMarkStack, UnifiedGcStats, GCPhase};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, RwLock};
    use std::collections::HashSet;
    
    let mark_stack = Arc::new(LockFreeMarkStack::new(1000));
    let marked_set = Arc::new(RwLock::new(HashSet::new()));
    let phase = Arc::new(AtomicU64::new(GCPhase::Marking as u64));
    let stats = Arc::new(UnifiedGcStats::default());
    
    let marker = GcMarker::new(
        mark_stack.clone(),
        marked_set.clone(),
        phase.clone(),
        stats.clone(),
    );
    
    // 添加根对象
    let roots: Vec<u64> = (0..1000).map(|i| i as u64 * 1024).collect();
    marker.prepare_marking(&roots);
    
    // 测试增量标记性能
    let start = Instant::now();
    let (complete, marked_count) = marker.incremental_mark(10000); // 10ms配额
    let elapsed = start.elapsed().as_micros() as u64;
    
    println!("  GC标记: 完成={}, 标记={}, 耗时={}us", complete, marked_count, elapsed);
    assert!(elapsed < 10000, "GC标记应该 < 10ms");
    
    // 测试清扫器性能
    let sweep_list = Arc::new(Mutex::new((0..1000).map(|i| i as u64 * 1024).collect()));
    let sweeper = GcSweeper::new(sweep_list.clone(), phase.clone(), stats.clone(), 100);
    
    sweeper.prepare_sweeping(&(0..1000).map(|i| i as u64 * 1024).collect::<Vec<_>>(), &HashSet::new());
    
    let start = Instant::now();
    let (complete, freed_count) = sweeper.incremental_sweep(10000); // 10ms配额
    let elapsed = start.elapsed().as_micros() as u64;
    
    println!("  GC清扫: 完成={}, 释放={}, 耗时={}us", complete, freed_count, elapsed);
    assert!(elapsed < 10000, "GC清扫应该 < 10ms");
}

fn create_performance_test_code() -> Vec<u8> {
    // 创建一个稍微复杂的测试代码
    vec![
        0xB8, 0x0A, 0x00, 0x00, 0x00,  // mov eax, 10
        0xBB, 0x14, 0x00, 0x00, 0x00,  // mov ebx, 20
        0x01, 0xD8,                     // add eax, ebx
        0x83, 0xC0, 0x05,               // add eax, 5
        0x29, 0xD8,                     // sub eax, ebx
        0xC3,                           // ret
    ]
}
