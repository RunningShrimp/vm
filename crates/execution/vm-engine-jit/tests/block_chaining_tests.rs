//! Block Chaining Tests
//!
//! Comprehensive tests for the BlockChainer component including:
//! - Basic block chain creation and management
//! - Direct jump optimization
//! - Conditional branch handling
//! - Hot path identification
//! - Chain validation and execution tracking

use vm_core::GuestAddr;
use vm_engine_jit::block_chaining::{BlockChainer, BlockChainerStats, ChainType};
use vm_ir::{IRBlock, IRBuilder, Terminator};

/// Helper function to create a simple test block with a direct jump
fn create_direct_jump_block(start: u64, target: u64) -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(start));
    builder.set_term(Terminator::Jmp {
        target: GuestAddr(target),
    });
    builder.build()
}

/// Helper function to create a conditional jump block
fn create_conditional_block(start: u64, target_true: u64, target_false: u64) -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(start));
    builder.set_term(Terminator::CondJmp {
        cond: 1,
        target_true: GuestAddr(target_true),
        target_false: GuestAddr(target_false),
    });
    builder.build()
}

/// Helper function to create a return block
fn create_return_block(start: u64) -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(start));
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// Helper function to create a call block
fn create_call_block(start: u64, target: u64, ret_pc: u64) -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(start));
    builder.set_term(Terminator::Call {
        target: GuestAddr(target),
        ret_pc: GuestAddr(ret_pc),
    });
    builder.build()
}

#[test]
fn test_chainer_creation() {
    let chainer = BlockChainer::new();
    let stats = chainer.stats();

    assert_eq!(stats.total_links, 0);
    assert_eq!(stats.total_chains, 0);
    assert_eq!(stats.total_blocks, 0);
    assert_eq!(stats.avg_chain_length, 0.0);
}

#[test]
fn test_chainer_with_config() {
    let chainer = BlockChainer::with_config(8, false);
    let stats = chainer.stats();

    assert_eq!(stats.max_chain_length, 8);
    assert_eq!(stats.total_links, 0);
    assert_eq!(stats.total_chains, 0);
}

#[test]
fn test_analyze_direct_jump() {
    let mut chainer = BlockChainer::new();

    let block1 = create_direct_jump_block(0x1000, 0x2000);
    chainer.analyze_block(&block1).unwrap();

    let stats = chainer.stats();
    assert_eq!(stats.total_links, 1);
    assert_eq!(stats.total_blocks, 1);

    // Check the link was created
    let link = chainer.get_link(GuestAddr(0x1000), GuestAddr(0x2000));
    assert!(link.is_some());
    let link = link.unwrap();
    assert_eq!(link.from, GuestAddr(0x1000));
    assert_eq!(link.to, GuestAddr(0x2000));
    assert_eq!(link.link_type, ChainType::Direct);
    assert_eq!(link.frequency, 1);
    assert!(!link.optimized);
}

#[test]
fn test_analyze_conditional_jump() {
    let mut chainer = BlockChainer::new();

    let block1 = create_conditional_block(0x1000, 0x2000, 0x3000);
    chainer.analyze_block(&block1).unwrap();

    let stats = chainer.stats();
    assert_eq!(stats.total_links, 2);
    assert_eq!(stats.total_blocks, 1);

    // Check both links were created
    let link1 = chainer.get_link(GuestAddr(0x1000), GuestAddr(0x2000));
    assert!(link1.is_some());
    assert_eq!(link1.unwrap().link_type, ChainType::Conditional);

    let link2 = chainer.get_link(GuestAddr(0x1000), GuestAddr(0x3000));
    assert!(link2.is_some());
    assert_eq!(link2.unwrap().link_type, ChainType::Conditional);
}

#[test]
fn test_analyze_return() {
    let mut chainer = BlockChainer::new();

    let block1 = create_return_block(0x1000);
    chainer.analyze_block(&block1).unwrap();

    let stats = chainer.stats();
    assert_eq!(stats.total_links, 0); // No links should be created for return
    assert_eq!(stats.total_blocks, 1);
}

#[test]
fn test_analyze_call() {
    let mut chainer = BlockChainer::new();

    let block1 = create_call_block(0x1000, 0x2000, 0x1008);
    chainer.analyze_block(&block1).unwrap();

    let stats = chainer.stats();
    assert_eq!(stats.total_links, 1);
    assert_eq!(stats.total_blocks, 1);

    let link = chainer.get_link(GuestAddr(0x1000), GuestAddr(0x2000));
    assert!(link.is_some());
    assert_eq!(link.unwrap().link_type, ChainType::Call);
}

#[test]
fn test_build_simple_chain() {
    let mut chainer = BlockChainer::new();

    // Create a simple chain: block1 -> block2 -> block3
    let block1 = create_direct_jump_block(0x1000, 0x2000);
    let block2 = create_direct_jump_block(0x2000, 0x3000);
    let block3 = create_return_block(0x3000);

    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block2).unwrap();
    chainer.analyze_block(&block3).unwrap();

    chainer.build_chains();

    let stats = chainer.stats();
    assert!(stats.total_chains >= 1); // At least one chain should be created

    // Check the chain
    let chain = chainer.get_chain(GuestAddr(0x1000));
    assert!(chain.is_some());
    let chain = chain.unwrap();
    assert_eq!(chain.start, GuestAddr(0x1000));
    assert!(chain.blocks.len() >= 2); // Should have at least block1 and block2
    assert!(chain.blocks.contains(&GuestAddr(0x1000)));
    assert!(chain.blocks.contains(&GuestAddr(0x2000)));
}

#[test]
fn test_build_chain_with_conditional() {
    let mut chainer = BlockChainer::new();

    // Create: block1 -> (block2 | block3) -> block4
    let block1 = create_conditional_block(0x1000, 0x2000, 0x3000);
    let block2 = create_direct_jump_block(0x2000, 0x4000);
    let block3 = create_direct_jump_block(0x3000, 0x4000);
    let block4 = create_return_block(0x4000);

    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block2).unwrap();
    chainer.analyze_block(&block3).unwrap();
    chainer.analyze_block(&block4).unwrap();

    chainer.build_chains();

    // Should have chains from 0x1000
    let chain = chainer.get_chain(GuestAddr(0x1000));
    assert!(chain.is_some());

    // Should prefer direct jumps in the chain
    let chain = chain.unwrap();
    assert!(chain.blocks.len() >= 2);
}

#[test]
fn test_max_chain_length() {
    let chainer = BlockChainer::with_config(2, true);
    let mut chainer = chainer;

    // Create a chain longer than max_length
    let block1 = create_direct_jump_block(0x1000, 0x2000);
    let block2 = create_direct_jump_block(0x2000, 0x3000);
    let block3 = create_direct_jump_block(0x3000, 0x4000);
    let block4 = create_return_block(0x4000);

    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block2).unwrap();
    chainer.analyze_block(&block3).unwrap();
    chainer.analyze_block(&block4).unwrap();

    chainer.build_chains();

    let chain = chainer.get_chain(GuestAddr(0x1000));
    assert!(chain.is_some());
    let chain = chain.unwrap();
    // Chain should be limited to max_chain_length
    assert!(chain.blocks.len() <= 2);
}

#[test]
fn test_hot_path_optimization() {
    let mut chainer = BlockChainer::with_config(16, true);

    // Create multiple chains with different frequencies
    let block1 = create_direct_jump_block(0x1000, 0x2000);
    let block2 = create_direct_jump_block(0x2000, 0x3000);
    let block3 = create_return_block(0x3000);

    // Analyze block1 multiple times to simulate hot path
    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block1).unwrap();

    chainer.analyze_block(&block2).unwrap();
    chainer.analyze_block(&block3).unwrap();

    chainer.build_chains();

    // Hot path (0x1000) should be prioritized
    let chain = chainer.get_chain(GuestAddr(0x1000));
    assert!(chain.is_some());
}

#[test]
fn test_chain_cycle_detection() {
    let mut chainer = BlockChainer::new();

    // Create a cycle: block1 -> block2 -> block1
    let block1 = create_direct_jump_block(0x1000, 0x2000);
    let block2 = create_direct_jump_block(0x2000, 0x1000);

    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block2).unwrap();

    chainer.build_chains();

    let chain = chainer.get_chain(GuestAddr(0x1000));
    assert!(chain.is_some());

    // Chain should stop when it detects a cycle
    let chain = chain.unwrap();
    // Should not contain duplicate blocks
    let mut unique_blocks = std::collections::HashSet::new();
    for block in &chain.blocks {
        assert!(
            unique_blocks.insert(*block),
            "Cycle detected: block appears twice"
        );
    }
}

#[test]
fn test_all_chains_iterator() {
    let mut chainer = BlockChainer::new();

    let block1 = create_direct_jump_block(0x1000, 0x2000);
    let block2 = create_direct_jump_block(0x2000, 0x3000);
    let block3 = create_return_block(0x3000);

    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block2).unwrap();
    chainer.analyze_block(&block3).unwrap();

    chainer.build_chains();

    let chains: Vec<_> = chainer.all_chains().collect();
    assert!(!chains.is_empty());
}

#[test]
fn test_clear() {
    let mut chainer = BlockChainer::new();

    let block1 = create_direct_jump_block(0x1000, 0x2000);
    chainer.analyze_block(&block1).unwrap();
    chainer.build_chains();

    assert!(chainer.stats().total_links > 0);
    assert!(chainer.stats().total_chains > 0);

    chainer.clear();

    assert_eq!(chainer.stats().total_links, 0);
    assert_eq!(chainer.stats().total_chains, 0);
    assert_eq!(chainer.stats().total_blocks, 0);
}

#[test]
fn test_link_frequency_tracking() {
    let mut chainer = BlockChainer::new();

    let block1 = create_direct_jump_block(0x1000, 0x2000);

    // Analyze the same block multiple times
    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block1).unwrap();

    let link = chainer.get_link(GuestAddr(0x1000), GuestAddr(0x2000));
    assert!(link.is_some());
    assert_eq!(link.unwrap().frequency, 3);
}

#[test]
fn test_stats_average_chain_length() {
    let mut chainer = BlockChainer::new();

    let block1 = create_direct_jump_block(0x1000, 0x2000);
    let block2 = create_direct_jump_block(0x2000, 0x3000);
    let block3 = create_return_block(0x3000);

    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block2).unwrap();
    chainer.analyze_block(&block3).unwrap();

    chainer.build_chains();

    let stats = chainer.stats();
    assert!(stats.avg_chain_length > 0.0);
}

#[test]
fn test_complex_chain_network() {
    let mut chainer = BlockChainer::new();

    // Create a complex network:
    // 0x1000 -> 0x2000
    // 0x1000 -> 0x3000 (conditional)
    // 0x2000 -> 0x4000
    // 0x3000 -> 0x4000
    // 0x4000 -> ret

    let block1 = create_conditional_block(0x1000, 0x2000, 0x3000);
    let block2 = create_direct_jump_block(0x2000, 0x4000);
    let block3 = create_direct_jump_block(0x3000, 0x4000);
    let block4 = create_return_block(0x4000);

    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block2).unwrap();
    chainer.analyze_block(&block3).unwrap();
    chainer.analyze_block(&block4).unwrap();

    chainer.build_chains();

    // Should have at least one chain
    assert!(chainer.stats().total_chains >= 1);

    // All blocks should be tracked
    assert_eq!(chainer.stats().total_blocks, 4);
}

#[test]
fn test_indirect_jump_no_link() {
    let mut chainer = BlockChainer::new();

    // Indirect jumps should not create links
    let mut builder = IRBuilder::new(GuestAddr(0x1000));
    builder.set_term(Terminator::JmpReg { base: 1, offset: 0 });
    let block1 = builder.build();

    chainer.analyze_block(&block1).unwrap();

    assert_eq!(chainer.stats().total_links, 0);
}

#[test]
fn test_call_link_creation() {
    let mut chainer = BlockChainer::new();

    let block1 = create_call_block(0x1000, 0x5000, 0x1008);

    chainer.analyze_block(&block1).unwrap();

    let link = chainer.get_link(GuestAddr(0x1000), GuestAddr(0x5000));
    assert!(link.is_some());
    assert_eq!(link.unwrap().link_type, ChainType::Call);
}

#[test]
fn test_multiple_analyze_calls() {
    let mut chainer = BlockChainer::new();

    let block1 = create_direct_jump_block(0x1000, 0x2000);

    // Multiple analyzes should increase frequency
    for _ in 0..5 {
        chainer.analyze_block(&block1).unwrap();
    }

    let link = chainer.get_link(GuestAddr(0x1000), GuestAddr(0x2000));
    assert!(link.is_some());
    assert_eq!(link.unwrap().frequency, 5);

    // But total_blocks should still be 1 (unique blocks)
    assert_eq!(chainer.stats().total_blocks, 1);
}

#[test]
fn test_chain_with_mixed_terminators() {
    let mut chainer = BlockChainer::new();

    // Mix different terminator types
    let block1 = create_direct_jump_block(0x1000, 0x2000);
    let block2 = create_conditional_block(0x2000, 0x3000, 0x4000);
    let block3 = create_return_block(0x3000);
    let block4 = create_return_block(0x4000);

    chainer.analyze_block(&block1).unwrap();
    chainer.analyze_block(&block2).unwrap();
    chainer.analyze_block(&block3).unwrap();
    chainer.analyze_block(&block4).unwrap();

    chainer.build_chains();

    // Should create chains even with mixed terminators
    assert!(chainer.stats().total_chains > 0);
}

#[test]
fn test_backward_compatibility_aliases() {
    // Test that type aliases work
    let chainer = BlockChainer::new();
    let _stats: BlockChainerStats = chainer.stats();

    // Should compile without errors
    assert!(true);
}
