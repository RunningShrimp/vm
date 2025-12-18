//! JIT引擎基础测试
//!
//! 测试JIT引擎的基本功能

use vm_engine_jit::core::{JITEngine, JITConfig};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

#[test]
fn test_jit_config_default() {
    let config = JITConfig::default();
    
    // 验证默认配置值
    assert!(config.enable_optimization, "默认应该启用优化");
    assert_eq!(config.optimization_level, 2, "默认优化级别应该为2");
    assert!(config.enable_simd, "默认应该启用SIMD");
    assert_eq!(config.code_cache_size_limit, 64 * 1024 * 1024, "默认代码缓存大小应该为64MB");
    assert_eq!(config.hotspot_threshold, 100, "默认热点阈值应该为100");
    assert!(config.enable_adaptive_compilation, "默认应该启用自适应编译");
}

#[test]
fn test_jit_engine_creation() {
    let config = JITConfig::default();
    let engine = JITEngine::new(config);
    
    // 验证引擎创建成功
    let stats = engine.get_compilation_stats();
    assert_eq!(stats.original_insn_count, 0, "初始编译统计应该为0");
    assert_eq!(stats.optimized_insn_count, 0, "初始优化指令数应该为0");
    assert_eq!(stats.machine_insn_count, 0, "初始机器指令数应该为0");
}

#[test]
fn test_hotspot_counter() {
    let config = JITConfig::default();
    let engine = JITEngine::new(config);
    
    let pc = 0x1000;
    
    // 初始状态不应该是热点
    assert!(!engine.is_hotspot(pc), "初始状态不应该是热点");
    
    // 更新热点计数
    for _ in 0..50 {
        engine.update_hotspot_counter(pc);
    }
    
    // 50次后仍不应该是热点（阈值是100）
    assert!(!engine.is_hotspot(pc), "50次后仍不应该是热点");
    
    // 再更新50次，总共100次
    for _ in 50..100 {
        engine.update_hotspot_counter(pc);
    }
    
    // 100次后应该是热点
    assert!(engine.is_hotspot(pc), "100次后应该是热点");
}

#[test]
fn test_cache_operations() {
    let config = JITConfig::default();
    let engine = JITEngine::new(config);
    
    // 初始缓存统计
    let stats = engine.get_cache_stats();
    assert_eq!(stats.entry_count, 0, "初始缓存条目数应该为0");
    assert_eq!(stats.hits, 0, "初始命中次数应该为0");
    assert_eq!(stats.misses, 0, "初始未命中次数应该为0");
    
    // 清空缓存
    engine.clear_cache();
    let stats_after_clear = engine.get_cache_stats();
    assert_eq!(stats_after_clear.entry_count, 0, "清空后缓存条目数应该为0");
}

#[test]
fn test_ir_block_creation() {
    // 创建一个简单的IR块
    let mut builder = IRBuilder::new(0x1000);
    builder.push(IROp::MovImm { dst: 1, imm: 42 });
    builder.push(IROp::MovImm { dst: 2, imm: 24 });
    builder.push(IROp::Add { dst: 3, src1: 1, src2: 2 });
    builder.set_term(Terminator::Ret);
    let block = builder.build();
    
    // 验证IR块内容
    assert_eq!(block.start_pc, 0x1000, "起始PC应该为0x1000");
    assert_eq!(block.ops.len(), 3, "应该有3条指令");
    
    // 验证第一条指令
    match &block.ops[0] {
        IROp::MovImm { dst, imm } => {
            assert_eq!(*dst, 1, "第一条指令的目标寄存器应该为1");
            assert_eq!(*imm, 42, "第一条指令的立即数应该为42");
        }
        _ => panic!("第一条指令应该是MovImm"),
    }
    
    // 验证第二条指令
    match &block.ops[1] {
        IROp::MovImm { dst, imm } => {
            assert_eq!(*dst, 2, "第二条指令的目标寄存器应该为2");
            assert_eq!(*imm, 24, "第二条指令的立即数应该为24");
        }
        _ => panic!("第二条指令应该是MovImm"),
    }
    
    // 验证第三条指令
    match &block.ops[2] {
        IROp::Add { dst, src1, src2 } => {
            assert_eq!(*dst, 3, "第三条指令的目标寄存器应该为3");
            assert_eq!(*src1, 1, "第三条指令的第一个源寄存器应该为1");
            assert_eq!(*src2, 2, "第三条指令的第二个源寄存器应该为2");
        }
        _ => panic!("第三条指令应该是Add"),
    }
    
    // 验证终结符
    match &block.term {
        Terminator::Ret => {}, // 应该是返回
        _ => panic!("终结符应该是Ret"),
    }
}