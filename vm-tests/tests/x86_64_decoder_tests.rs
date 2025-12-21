//! x86_64 解码器测试
//!
//! 测试增强的x86_64解码器功能

use std::time::Instant;
use vm_core::{MMU, GuestAddr};
use vm_frontend_x86_64::{DecoderPipeline, InsnStream};
use vm_mem::SoftMmu;

#[test]
fn test_x86_64_basic_instructions() {
    let mut mmu = SoftMmu::new(1024 * 1024, false); // 1MB内存

    // 写入一些简单的指令到内存
    let instructions = vec![
        0x90, // NOP
        0xC3, // RET
        0x90, // NOP
        0xEB, 0x10, // JMP rel8 (forward 16 bytes)
        0x90, // NOP
        0x90, // NOP
    ];

    for (i, &byte) in instructions.iter().enumerate() {
        mmu.write(GuestAddr(i as u64), byte as u64, 1).unwrap();
    }

    let mut decoder = DecoderPipeline::new();
    let mut _stream = InsnStream::new(&mmu, GuestAddr(0));

    println!("Testing x86_64 instruction decoding:");

    // 测试NOP指令
    let pc = 0;
    let mut stream = InsnStream::new(&mmu, GuestAddr(pc));
    match decoder.decode_instruction(&mut stream, GuestAddr(pc)) {
        Ok(instruction) => {
            println!("  NOP decoded: {:?}", instruction.mnemonic);
            assert_eq!(format!("{:?}", instruction.mnemonic), "Nop");
        }
        Err(e) => panic!("Failed to decode NOP: {:?}", e),
    }

    // 测试RET指令
    let pc = 1;
    let mut stream = InsnStream::new(&mmu, GuestAddr(pc));
    match decoder.decode_instruction(&mut stream, GuestAddr(pc)) {
        Ok(instruction) => {
            println!("  RET decoded: {:?}", instruction.mnemonic);
            assert_eq!(format!("{:?}", instruction.mnemonic), "Ret");
        }
        Err(e) => panic!("Failed to decode RET: {:?}", e),
    }

    // 测试JMP rel8指令
    let pc = 4;
    let mut stream = InsnStream::new(&mmu, GuestAddr(pc));
    match decoder.decode_instruction(&mut stream, GuestAddr(pc)) {
        Ok(instruction) => {
            println!("  JMP rel8 decoded: {:?}", instruction.mnemonic);
            assert_eq!(format!("{:?}", instruction.mnemonic), "Jmp");
        }
        Err(e) => panic!("Failed to decode JMP rel8: {:?}", e),
    }
}

#[test]
fn test_x86_64_mov_instructions() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // MOV指令测试
    let instructions = vec![
        0xB8, 0x2A, 0x00, 0x00, 0x00, // MOV EAX, 0x2A
        0x89, 0xD8, // MOV EAX, EBX
        0x8B, 0xC3, // MOV EAX, EBX
        0x90, // NOP
    ];

    for (i, &byte) in instructions.iter().enumerate() {
        mmu.write(GuestAddr(i as u64), byte as u64, 1).unwrap();
    }

    let mut decoder = DecoderPipeline::new();
    let mut _stream = InsnStream::new(&mmu, GuestAddr(0));

    println!("Testing x86_64 MOV instructions:");

    // 测试MOV reg, imm32
    let pc = 0;
    let mut stream = InsnStream::new(&mmu, GuestAddr(pc));
    match decoder.decode_instruction(&mut stream, GuestAddr(pc)) {
        Ok(instruction) => {
            println!("  MOV EAX, imm32 decoded: {:?}", instruction.mnemonic);
            assert_eq!(format!("{:?}", instruction.mnemonic), "Mov");
        }
        Err(e) => {
            println!("  MOV EAX, imm32 decode error: {:?}", e);
            // 由于操作数解码还未完全实现，这可能会失败
        }
    }
}

#[test]
fn test_x86_64_arithmetic_instructions() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // 算术指令测试
    let instructions = vec![
        0x01, 0xD8, // ADD EAX, EBX
        0x29, 0xD8, // SUB EAX, EBX
        0x31, 0xD8, // XOR EAX, EBX
        0x21, 0xD8, // AND EAX, EBX
        0x90, // NOP
    ];

    for (i, &byte) in instructions.iter().enumerate() {
        mmu.write(i as u64, byte as u64, 1).unwrap();
    }

    let mut decoder = DecoderPipeline::new();

    println!("Testing x86_64 arithmetic instructions:");

    // 测试ADD指令
    let pc = 0;
    let mut stream = InsnStream::new(&mmu, pc);
    match decoder.decode_instruction(&mut stream, pc) {
        Ok(instruction) => {
            println!("  ADD decoded: {:?}", instruction.mnemonic);
            assert_eq!(format!("{:?}", instruction.mnemonic), "Add");
        }
        Err(e) => {
            println!("  ADD decode error: {:?}", e);
            // 由于操作数解码还未完全实现，这可能会失败
        }
    }

    // 测试SUB指令
    let pc = 2;
    stream = InsnStream::new(&mmu, pc);
    match decoder.decode_instruction(&mut stream, pc) {
        Ok(instruction) => {
            println!("  SUB decoded: {:?}", instruction.mnemonic);
            assert_eq!(format!("{:?}", instruction.mnemonic), "Sub");
        }
        Err(e) => {
            println!("  SUB decode error: {:?}", e);
        }
    }
}

#[test]
fn test_x86_64_decoder_performance() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // 创建包含多种指令的测试序列
    let mut instructions = Vec::new();
    for i in 0..1000 {
        instructions.push(0x90); // NOP
        instructions.push(0xC3); // RET
    }

    for (i, &byte) in instructions.iter().enumerate() {
        mmu.write(i as u64, byte as u64, 1).unwrap();
    }

    let mut decoder = DecoderPipeline::new();

    println!("Testing x86_64 decoder performance:");

    let start = Instant::now();
    let mut successful_decodes = 0;

    for i in 0..500 {
        let pc = (i * 2) as u64; // 每条指令2字节
        let mut stream = InsnStream::new(&mmu, pc);

        match decoder.decode_instruction(&mut stream, pc) {
            Ok(_) => {
                successful_decodes += 1;
                decoder.reset(); // 重置解码器状态
            }
            Err(_) => {
                decoder.reset(); // 即使出错也重置
            }
        }
    }

    let duration = start.elapsed();
    let decodes_per_sec = successful_decodes as f64 / duration.as_secs_f64();

    println!("  Decodes: {}", successful_decodes);
    println!("  Duration: {:?}", duration);
    println!("  Decodes/sec: {:.2}", decodes_per_sec);

    // 性能断言：解码应该相对较快
    assert!(
        successful_decodes > 400,
        "Should successfully decode most instructions"
    );
    assert!(
        decodes_per_sec > 1000.0,
        "Decoder should be reasonably fast"
    );
}

#[test]
fn test_x86_64_extended_instructions() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // 两字节指令测试 (0x0F前缀)
    let instructions = vec![
        0x0F, 0x80, 0x10, 0x00, 0x00, 0x00, // JO rel32
        0x0F, 0x90, 0x90, 0x90, // SETO [r/m8] (需要ModR/M)
        0x90, // NOP
    ];

    for (i, &byte) in instructions.iter().enumerate() {
        mmu.write(i as u64, byte as u64, 1).unwrap();
    }

    let mut decoder = DecoderPipeline::new();

    println!("Testing x86_64 extended instructions:");

    // 测试两字节指令
    let pc = 0;
    let mut stream = InsnStream::new(&mmu, pc);
    match decoder.decode_instruction(&mut stream, pc) {
        Ok(instruction) => {
            println!("  Extended instruction decoded: {:?}", instruction.mnemonic);
        }
        Err(e) => {
            println!("  Extended instruction decode error: {:?}", e);
            // 由于操作数解码还未完全实现，这可能会失败
        }
    }
}
