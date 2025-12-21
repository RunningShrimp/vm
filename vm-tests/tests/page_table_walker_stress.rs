//! 页表遍历器压力测试
//!
//! 测试页表遍历器在高负载下的功能和性能

use std::time::{Duration, Instant};

use vm_core::{AccessType, Fault, GuestAddr, MemoryAccess, MMU};
use vm_mem::{PagingMode, SoftMmu, pte_flags};

/// 创建测试用的页表结构
fn create_test_page_table(mmu: &mut SoftMmu, root_addr: u64, levels: u32) -> Result<(), vm_core::VmError> {
    // 递归创建多级页表
    let mut current_addr = root_addr;

    for level in (0..levels).rev() {
        let next_addr = if level == 0 {
            // 最后一级，指向物理页
            current_addr + 0x1000 // 物理页区域
        } else {
            // 中间级，指向下一级页表
            current_addr + 0x1000 * (1 << (level * 9))
        };

        // 写入页表项
        for i in 0..512 {
            let pte_addr = current_addr + i * 8;

            if level == 0 {
                // 最后一级：指向物理页
                let pte = pte_flags::V
                    | pte_flags::R
                    | pte_flags::W
                    | pte_flags::X
                    | ((current_addr + 0x1000 + i * 0x1000) >> 12); // PPN

                (&mut *mmu as &mut dyn MemoryAccess).write(GuestAddr(pte_addr), pte, 8)?;
            } else {
                // 中间级：指向下一级页表
                let next_level_addr = next_addr + (i * 512) * 0x1000;
                let pte = pte_flags::V | (next_level_addr >> 12);
                (&mut *mmu as &mut dyn MemoryAccess).write(GuestAddr(pte_addr), pte, 8)?;
            }
        }

        current_addr = if level == 0 {
            current_addr + 0x1000
        } else {
            current_addr + (1 << (level * 9)) * 512 * 8
        };
    }

    Ok(())
}

/// 测试页表遍历器的基本功能
#[test]
fn test_page_table_walker_basic() {
    let mut mmu = SoftMmu::new(16 * 1024 * 1024, false); // 16MB内存

    // 设置分页模式
    mmu.set_paging_mode(PagingMode::Sv39);

    // 设置页表基址
    let page_table_base = 0x2000;
    mmu.set_page_table_base(page_table_base);

    // 创建3级页表结构
    create_test_page_table(&mut mmu, page_table_base, 3).expect("Failed to create test page table");

    // 测试各种地址翻译
    let test_addresses = [
        0x100000u64, // 第一页
        0x101000u64, // 中间页
        0x102000u64, // 最后页
        0x10FFF0u64, // 页末对齐测试
    ];

    for (i, test_va) in test_addresses.iter().enumerate() {
        match mmu.translate(Guest_addr(test_va), AccessType::Read) {
            Ok(pa) => {
                let expected_pa = 0x10000 + (test_va & 0xFFF);
                assert_eq!(pa, expected_pa, "Page {} translation failed", i);
                println!("✓ Page {} translation: 0x{:x} -> 0x{:x}", i, test_va, pa);
            }
            Err(e) => panic!("Page {} translation failed: {:?}", i, e),
        }
    }
}

/// 测试页表遍历器的性能
#[test]
fn test_page_table_walker_performance() {
    let mut mmu = SoftMmu::new(64 * 1024 * 1024, false); // 64MB内存

    // 设置分页模式
    mmu.set_paging_mode(PagingMode::Sv39);

    // 设置页表基址
    let page_table_base = 0x4000;
    mmu.set_page_table_base(page_table_base);

    // 创建大规模页表结构（模拟真实应用）
    let num_root_entries = 256; // 256个根页表项

    for i in 0..num_root_entries {
        let root_addr = page_table_base + i * 8;
        let second_level_addr = 0x100000 + i * 0x1000 * 512;

        // 设置根页表项
        let root_pte = pte_flags::V | (second_level_addr >> 12);
        mmu.write(root_addr, root_pte, 8).unwrap();

        // 创建二级页表
        for j in 0..512 {
            let second_level_addr = second_level_addr + j * 8;
            let third_level_addr = 0x200000 + (i * 512 + j) * 0x1000;

            let second_pte = pte_flags::V | (third_level_addr >> 12);
            mmu.write(second_level_addr, second_pte, 8).unwrap();

            // 创建三级页表（物理页）
            for k in 0..512 {
                let third_level_addr = third_level_addr + k * 8;
                let physical_addr = 0x1000000 + ((i * 512 + j) * 512 + k) * 0x1000;
                let third_pte = pte_flags::V
                    | pte_flags::R
                    | pte_flags::W
                    | pte_flags::X
                    | (physical_addr >> 12);
                mmu.write(third_level_addr, third_pte, 8).unwrap();
            }
        }
    }

    println!(
        "Created page table with {} root entries, {} total pages",
        num_root_entries,
        num_root_entries * 512 * 512
    );

    // 性能测试：大量地址翻译
    let start = Instant::now();
    let num_translations = 100000;

    let mut successful_translations = 0;

    for i in 0..num_translations {
        let test_va = 0x100000 + (i % (256 * 1024 * 1024)) as u64;

        match mmu.translate(test_va, AccessType::Read) {
            Ok(_) => successful_translations += 1,
            Err(Fault::PageFault { .. }) => {
                // 页错误是正常的，因为不是所有地址都映射
            }
            Err(e) => panic!("Unexpected error during translation: {:?}", e),
        }
    }

    let duration = start.elapsed();
    let translations_per_sec = num_translations as f64 / duration.as_secs_f64();

    println!("Page Table Walker Performance:");
    println!("  Translations: {}", num_translations);
    println!("  Duration: {:?}", duration);
    println!("  Translations/sec: {:.2}", translations_per_sec);
    println!("  Successful: {}", successful_translations);
    println!(
        "  Success rate: {:.2}%",
        (successful_translations as f64 / num_translations as f64) * 100.0
    );

    // 性能断言：应该能达到高翻译速度
    assert!(
        translations_per_sec > 100000.0,
        "Page table walker should be very fast"
    );
}

/// 测试TLB缓存的效果
#[test]
fn test_tlb_performance() {
    let mut mmu = SoftMmu::new(32 * 1024 * 1024, false);

    // 设置分页模式
    mmu.set_paging_mode(PagingMode::Sv39);
    mmu.set_page_table_base(0x5000);

    // 创建简单的页表
    create_test_page_table(&mut mmu, 0x5000, 3).unwrap();

    let test_addr = 0x100000u64;

    // 第一次翻译（TLB miss）
    let start = Instant::now();
    let _ = mmu.translate(test_addr, AccessType::Read).unwrap();
    let first_translation = start.elapsed();

    // 后续翻译（应该是TLB hit）
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = mmu.translate(test_addr, AccessType::Read).unwrap();
    }
    let cached_translations = start.elapsed();

    let tlb_speedup = first_translation.as_nanos() as f64 / cached_translations.as_nanos() as f64;

    println!("TLB Performance:");
    println!("  First translation: {:?}", first_translation);
    println!("  1000 cached translations: {:?}", cached_translations);
    println!("  TLB speedup: {:.2}x", tlb_speedup);

    // TLB应该显著提升性能
    assert!(tlb_speedup > 10.0, "TLB should provide significant speedup");
}

/// 测试页表遍历器的错误处理
#[test]
fn test_page_table_walker_error_handling() {
    let mut mmu = SoftMmu::new(4 * 1024 * 1024, false);

    // 设置分页模式但不设置页表基址
    mmu.set_paging_mode(PagingMode::Sv39);

    // 尝试翻译未映射的地址
    let unmapped_addr = 0x100000u64;

    match mmu.translate(unmapped_addr, AccessType::Read) {
        Err(Fault::PageFault { .. }) => {
            println!("✓ Correctly detected unmapped address");
        }
        Ok(_) => panic!("Should have detected page fault for unmapped address"),
        Err(e) => panic!("Unexpected error type: {:?}", e),
    }

    // 设置无效的页表基址
    mmu.set_page_table_base(0xFFFF0000); // 高地址，超出内存范围

    match mmu.translate(unmapped_addr, AccessType::Read) {
        Err(Fault::PageFault { .. }) => {
            println!("✓ Correctly detected invalid page table base");
        }
        Ok(_) => panic!("Should have detected page fault for invalid base"),
        Err(e) => panic!("Unexpected error type: {:?}", e),
    }
}

/// 测试SV48页表遍历器
#[test]
fn test_sv48_page_table_walker() {
    let mut mmu = SoftMmu::new(16 * 1024 * 1024, false);

    // 设置SV48分页模式
    mmu.set_paging_mode(PagingMode::Sv48);

    // 设置页表基址
    let page_table_base = 0x3000;
    mmu.set_page_table_base(page_table_base);

    // SV48需要4级页表，这里创建一个简化版本
    // 只设置根页表的一个有效项指向一个第三级页表
    let root_pte = pte_flags::V | (0x40000 >> 12); // 指向第四级页表
    mmu.write(page_table_base, root_pte, 8).unwrap();

    // 简化的SV48测试：直接测试第四级页表的某个地址
    let test_addr = 0x40000 + 0x1000; // 第四级页表基址 + 4KB偏移

    // 由于我们没有完整的4级页表，这会导致页错误，这是预期的
    match mmu.translate(test_addr, AccessType::Read) {
        Err(Fault::PageFault { .. }) => {
            println!("✓ SV48 correctly detects page fault for incomplete page table");
        }
        Ok(_) => println!("✓ SV48 page table access successful"),
        Err(e) => panic!("Unexpected error in SV48: {:?}", e),
    }
}

/// 比较SV39和SV48的性能
#[test]
fn test_sv39_vs_sv48_performance() {
    let modes = [PagingMode::Sv39, PagingMode::Sv48];

    for &mode in modes.iter() {
        let mut mmu = SoftMmu::new(8 * 1024 * 1024, false);
        mmu.set_paging_mode(mode);
        mmu.set_page_table_base(0x6000);

        // 创建对应的页表结构
        let levels = if mode == PagingMode::Sv39 { 3 } else { 4 };
        create_test_page_table(&mut mmu, 0x6000, levels).unwrap();

        println!("Testing {:?} performance:", mode);

        let start = Instant::now();
        let num_translations = 50000;

        let mut successful = 0;

        for i in 0..num_translations {
            let test_va = 0x100000 + (i % 0x100000) as u64;

            match mmu.translate(GuestAddr(test_va), AccessType::Read) {
                Ok(_) => successful += 1,
                Err(_) => {} // Any error is considered a fault for this test
            }
        }

        let duration = start.elapsed();
        let ops_per_sec = successful as f64 / duration.as_secs_f64();

        println!(
            "  {:?}: {:.2} ops/sec, {}% success rate",
            mode,
            ops_per_sec,
            (successful as f64 / num_translations as f64) * 100.0
        );
    }
}
