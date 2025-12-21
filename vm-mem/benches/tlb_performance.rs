//! TLB 性能基准测试

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vm_core::{AccessType, GuestAddr, TlbEntry, GuestPhysAddr};
use vm_mem::tlb::tlb_manager::{StandardTlbManager, TlbManager};
use vm_mem::tlb::tlb_optimized::{MultiLevelTlb, AdaptiveReplacementPolicy};

fn bench_tlb_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_lookup");
    
    group.bench_function("standard_tlb_hit", |b| {
        let mut tlb = StandardTlbManager::new(64);
        
        // 预填充TLB
        for i in 0..64 {
            let entry = TlbEntry {
                guest_addr: GuestAddr(i * 4096),
                phys_addr: GuestPhysAddr(i * 4096 + 0x100000),
                flags: 0x3,
                asid: 0,
            };
            tlb.update(entry);
        }
        
        b.iter(|| {
            for i in 0..64 {
                black_box(tlb.lookup(GuestAddr(i * 4096), 0, AccessType::Read));
            }
        });
    });
    
    group.bench_function("standard_tlb_miss", |b| {
        let mut tlb = StandardTlbManager::new(64);
        
        b.iter(|| {
            for i in 0..64 {
                black_box(tlb.lookup(GuestAddr(i * 4096), 0, AccessType::Read));
            }
        });
    });
    
    group.finish();
}

fn bench_tlb_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_insert");
    
    group.bench_function("standard_tlb_insert", |b| {
        let mut tlb = StandardTlbManager::new(64);
        
        b.iter(|| {
            for i in 0..64 {
                let entry = TlbEntry {
                    guest_addr: GuestAddr(i * 4096),
                    phys_addr: GuestPhysAddr(i * 4096 + 0x100000),
                    flags: 0x3,
                    asid: 0,
                };
                black_box(tlb.update(entry));
            }
        });
    });
    
    group.finish();
}

fn bench_multilevel_tlb(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_multilevel");
    
    // 注释掉多级TLB基准测试，因为需要配置对象
    // group.bench_function("multilevel_tlb_lookup", |b| {
    //     // 需要 MultiLevelTlbConfig 来创建 MultiLevelTlb
    //     // 暂时跳过，因为配置结构可能未公开
    // });
    
    group.finish();
}

fn bench_tlb_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_flush");
    
    group.bench_function("flush_all", |b| {
        let mut tlb = StandardTlbManager::new(64);
        
        // 预填充TLB
        for i in 0..64 {
            let entry = TlbEntry {
                guest_addr: GuestAddr(i * 4096),
                phys_addr: GuestPhysAddr(i * 4096 + 0x100000),
                flags: 0x3,
                asid: 0,
            };
            tlb.update(entry);
        }
        
        b.iter(|| {
            tlb.flush();
        });
    });
    
    group.bench_function("flush_asid", |b| {
        let mut tlb = StandardTlbManager::new(64);
        
        // 预填充TLB（不同ASID）
        for i in 0..64 {
            let entry = TlbEntry {
                guest_addr: GuestAddr(i * 4096),
                phys_addr: GuestPhysAddr(i * 4096 + 0x100000),
                flags: 0x3,
                asid: (i % 4) as u16,
            };
            tlb.update(entry);
        }
        
        b.iter(|| {
            tlb.flush_asid(0);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_tlb_lookup,
    bench_tlb_insert,
    bench_multilevel_tlb,
    bench_tlb_flush
);
criterion_main!(benches);

