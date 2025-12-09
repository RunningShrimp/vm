//! 构建脚本 - 自动生成指令解码器

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=examples/");
    println!("cargo:rerun-if-env-changed=VM_CODEGEN_GEN");

    let r#gen = std::env::var("VM_CODEGEN_GEN").unwrap_or_default();
    if r#gen == "1" {
        if let Err(e) = run_codegen_example("arm64_instructions") {
            println!("cargo:warning=Failed to generate ARM64 decoder: {}", e);
        }

        if let Err(e) = run_codegen_example("riscv_instructions") {
            println!("cargo:warning=Failed to generate RISC-V decoder: {}", e);
        }

        check_generated_code();
    } else {
        println!("cargo:warning=Skip codegen examples (set VM_CODEGEN_GEN=1 to enable)");
    }
}

fn run_codegen_example(example_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("cargo")
        .args(["run", "--example", example_name])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Code generation failed: {}", stderr).into());
    }

    Ok(())
}

fn check_generated_code() {
    // 检查生成的ARM64解码器
    let arm64_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("arm64_decoder_generated.rs");
    if arm64_path.exists() {
        println!("cargo:rerun-if-changed=arm64_decoder_generated.rs");
    }

    // 检查生成的RISC-V解码器
    let riscv_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("riscv_decoder_generated.rs");
    if riscv_path.exists() {
        println!("cargo:rerun-if-changed=riscv_decoder_generated.rs");
    }
}
