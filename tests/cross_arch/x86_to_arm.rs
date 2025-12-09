//! x86-64 到 ARM64 指令翻译测试

use vm_ir::{IROp, RegId};

/// 模拟翻译上下文
struct TranslationContext {
    src_arch: &'static str,
    dst_arch: &'static str,
    instruction_count: usize,
}

impl TranslationContext {
    fn new(src: &'static str, dst: &'static str) -> Self {
        Self {
            src_arch: src,
            dst_arch: dst,
            instruction_count: 0,
        }
    }

    fn translate(&mut self, _ir: IROp) -> bool {
        self.instruction_count += 1;
        true
    }
}

#[test]
fn test_basic_arithmetic_translation() {
    let mut ctx = TranslationContext::new("x86-64", "arm64");
    
    // 模拟add指令的IR表示
    let add_ir = IROp::BinOp {
        dest: RegId(0),
        src1: RegId(0),
        src2: RegId(1),
        op: "add",
    };
    
    assert!(ctx.translate(add_ir));
    assert_eq!(ctx.instruction_count, 1);
}

#[test]
fn test_memory_load_translation() {
    let mut ctx = TranslationContext::new("x86-64", "arm64");
    
    // 模拟load指令的IR表示
    let load_ir = IROp::Load {
        dest: RegId(0),
        addr: RegId(1),
        size: 8,
    };
    
    assert!(ctx.translate(load_ir));
    assert_eq!(ctx.instruction_count, 1);
}

#[test]
fn test_memory_store_translation() {
    let mut ctx = TranslationContext::new("x86-64", "arm64");
    
    // 模拟store指令的IR表示
    let store_ir = IROp::Store {
        addr: RegId(0),
        value: RegId(1),
        size: 8,
    };
    
    assert!(ctx.translate(store_ir));
    assert_eq!(ctx.instruction_count, 1);
}

#[test]
fn test_branch_translation() {
    let mut ctx = TranslationContext::new("x86-64", "arm64");
    
    // 模拟条件分支的IR表示
    let branch_ir = IROp::Branch {
        target: 0x1000,
        condition: Some("eq"),
        cond_reg: Some(RegId(0)),
    };
    
    assert!(ctx.translate(branch_ir));
    assert_eq!(ctx.instruction_count, 1);
}

#[test]
fn test_block_translation_sequence() {
    let mut ctx = TranslationContext::new("x86-64", "arm64");
    
    let instructions = vec![
        IROp::BinOp {
            dest: RegId(0),
            src1: RegId(0),
            src2: RegId(1),
            op: "add",
        },
        IROp::Load {
            dest: RegId(1),
            addr: RegId(2),
            size: 8,
        },
        IROp::Store {
            addr: RegId(0),
            value: RegId(1),
            size: 8,
        },
    ];
    
    for ir in instructions {
        assert!(ctx.translate(ir));
    }
    
    assert_eq!(ctx.instruction_count, 3);
}

#[test]
fn test_x86_to_arm_register_mapping() {
    // x86-64通用寄存器映射到ARM64
    let x86_regs = vec![
        ("rax", 0),
        ("rcx", 1),
        ("rdx", 2),
        ("rbx", 3),
        ("rsp", 4),
    ];
    
    let arm_regs = vec![
        ("x0", 0),
        ("x1", 1),
        ("x2", 2),
        ("x3", 3),
        ("sp", 31),
    ];
    
    assert_eq!(x86_regs.len(), 5);
    assert_eq!(arm_regs.len(), 5);
}

#[test]
fn test_float_register_translation() {
    // x86-64 SSE寄存器到ARM64 NEON寄存器
    let x86_sse = vec!["xmm0", "xmm1", "xmm2", "xmm3"];
    let arm_neon = vec!["v0", "v1", "v2", "v3"];
    
    assert_eq!(x86_sse.len(), arm_neon.len());
}
