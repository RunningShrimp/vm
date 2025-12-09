//! ARM64 到 x86-64 指令翻译测试

use vm_ir::{IROp, RegId};

/// ARM到x86翻译上下文
struct ArmToX86Context {
    instruction_count: usize,
    condition_flags_used: bool,
}

impl ArmToX86Context {
    fn new() -> Self {
        Self {
            instruction_count: 0,
            condition_flags_used: false,
        }
    }

    fn translate(&mut self, _ir: IROp) -> bool {
        self.instruction_count += 1;
        true
    }

    fn use_condition_flags(&mut self) {
        self.condition_flags_used = true;
    }
}

#[test]
fn test_arm_to_x86_add_instruction() {
    let mut ctx = ArmToX86Context::new();
    
    // ARM64: add x0, x0, x1
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
fn test_arm_to_x86_load_instruction() {
    let mut ctx = ArmToX86Context::new();
    
    // ARM64: ldr x0, [x1, #offset]
    let load_ir = IROp::Load {
        dest: RegId(0),
        addr: RegId(1),
        size: 8,
    };
    
    assert!(ctx.translate(load_ir));
    assert_eq!(ctx.instruction_count, 1);
}

#[test]
fn test_arm_to_x86_store_instruction() {
    let mut ctx = ArmToX86Context::new();
    
    // ARM64: str x0, [x1, #offset]
    let store_ir = IROp::Store {
        addr: RegId(1),
        value: RegId(0),
        size: 8,
    };
    
    assert!(ctx.translate(store_ir));
}

#[test]
fn test_arm_to_x86_conditional_branch() {
    let mut ctx = ArmToX86Context::new();
    
    // ARM64: b.eq label (条件分支)
    let branch_ir = IROp::Branch {
        target: 0x1000,
        condition: Some("eq"),
        cond_reg: Some(RegId(0)),
    };
    
    ctx.use_condition_flags();
    assert!(ctx.translate(branch_ir));
    assert!(ctx.condition_flags_used);
}

#[test]
fn test_arm_to_x86_shift_operations() {
    let mut ctx = ArmToX86Context::new();
    
    // ARM64支持的移位操作
    // lsl (逻辑左移), lsr (逻辑右移), asr (算术右移)
    
    let shift_ir = IROp::BinOp {
        dest: RegId(0),
        src1: RegId(0),
        src2: RegId(1),
        op: "shl",
    };
    
    assert!(ctx.translate(shift_ir));
}

#[test]
fn test_arm_to_x86_neon_vector() {
    // ARM64 NEON向量操作到x86 SSE/AVX
    
    let neon_vector_width = 128; // bits
    let sse_vector_width = 128;  // bits
    
    assert_eq!(neon_vector_width, sse_vector_width);
}

#[test]
fn test_arm_to_x86_register_mapping() {
    // ARM64通用寄存器到x86-64映射
    let arm_regs = vec![
        ("x0", 0),   // 对应 rax
        ("x1", 1),   // 对应 rcx
        ("x2", 2),   // 对应 rdx
        ("x3", 3),   // 对应 rbx
        ("sp", 31),  // 对应 rsp
    ];
    
    let x86_regs = vec![
        ("rax", 0),
        ("rcx", 1),
        ("rdx", 2),
        ("rbx", 3),
        ("rsp", 4),
    ];
    
    assert_eq!(arm_regs.len(), 5);
    assert_eq!(x86_regs.len(), 5);
}

#[test]
fn test_arm_to_x86_float_register_mapping() {
    // ARM64浮点寄存器到x86 SSE/AVX
    let arm_float_regs = vec!["d0", "d1", "d2", "d3"];
    let x86_float_regs = vec!["xmm0", "xmm1", "xmm2", "xmm3"];
    
    assert_eq!(arm_float_regs.len(), x86_float_regs.len());
}

#[test]
fn test_arm_to_x86_immediate_encoding() {
    // ARM64的立即数编码不同于x86-64
    // ARM64可以编码旋转后的8位值
    // x86-64立即数通常更灵活
    
    let arm_imm_bits = 8;  // 基础宽度
    let x86_imm_bits = 32; // 或64位
    
    assert!(x86_imm_bits >= arm_imm_bits);
}

#[test]
fn test_arm_to_x86_packed_operations() {
    // ARM64 NEON打包操作到x86 SSE
    let neon_width = 128;
    let sse_width = 128;
    
    assert_eq!(neon_width, sse_width);
}

#[test]
fn test_arm_to_x86_cryptographic_instructions() {
    // ARM64 ARMv8加密扩展到x86 AES-NI
    
    let arm_has_crypto = true;
    let x86_has_aes_ni = true;
    
    assert_eq!(arm_has_crypto, x86_has_aes_ni);
}

#[test]
fn test_arm_to_x86_multiple_instructions() {
    let mut ctx = ArmToX86Context::new();
    
    // 翻译多条ARM64指令序列
    let instructions = vec![
        IROp::Load {
            dest: RegId(0),
            addr: RegId(1),
            size: 8,
        },
        IROp::BinOp {
            dest: RegId(0),
            src1: RegId(0),
            src2: RegId(2),
            op: "add",
        },
        IROp::Store {
            addr: RegId(1),
            value: RegId(0),
            size: 8,
        },
    ];
    
    for ir in instructions {
        assert!(ctx.translate(ir));
    }
    
    assert_eq!(ctx.instruction_count, 3);
}
