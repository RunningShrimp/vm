//! AOT 构建工具命令行界面

use aot_builder::AotBuilder;
use std::path::Path;

fn main() {
    tracing_subscriber::fmt::init();

    println!("=== AOT Builder Tool ===");
    println!("Ahead-Of-Time Compiler for VM Optimization");
    println!();

    // 示例：构建一个测试 AOT 镜像
    let mut builder = AotBuilder::new();

    // 添加示例块
    println!("Adding test compilation units...");

    for i in 0..5 {
        let pc = (0x1000 + i * 0x200) as u64;
        let code = generate_sample_code(i);

        match builder.add_compiled_block(pc, code, 1) {
            Ok(_) => println!("  [✓] Added block at {:#x}", pc),
            Err(e) => println!("  [✗] Failed to add block: {}", e),
        }
    }

    println!();
    builder.print_stats();

    // 构建 AOT 镜像
    match builder.build() {
        Ok(image) => {
            // 保存到文件
            let output_path = "/tmp/test.aot";
            match image.serialize(&mut std::fs::File::create(output_path).unwrap()) {
                Ok(_) => println!("\n[✓] AOT image saved to: {}", output_path),
                Err(e) => println!("\n[✗] Failed to save AOT image: {}", e),
            }

            // 验证
            println!();
            println!("=== Verification ===");
            println!("Image size: {} bytes", image.code_section.len());
            println!("Code blocks: {}", image.code_blocks.len());
            println!("Symbols: {}", image.symbols.len());
            println!("Relocations: {}", image.relocations.len());
            println!("Dependencies: {}", image.dependencies.len());
        }
        Err(e) => {
            eprintln!("\n[✗] Failed to build AOT image: {}", e);
            std::process::exit(1);
        }
    }
}

/// 生成示例代码（简单的 x86-64 序列）
fn generate_sample_code(idx: usize) -> Vec<u8> {
    let mut code = Vec::new();

    // 根据索引生成不同大小的代码
    let size = 8 + idx * 4;

    for _ in 0..size {
        code.push(0x90); // NOP
    }

    code.push(0xC3); // RET

    code
}
