//! SIMD Capabilities Detection Tool
//!
//! Detects and reports SIMD capabilities of the current platform.
//! Run with: cargo run --bin simd_capabilities

use std::process::Command;

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║        Platform SIMD Capabilities Detection Tool          ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Architecture information
    println!("📟 Architecture Information");
    println!("─────────────────────────────────────────────────────────");
    println!("Architecture: {}", std::env::consts::ARCH);
    println!("OS:           {}", std::env::consts::OS);
    println!("Family:       {}", std::env::consts::FAMILY);
    println!("Endian:       {}", if cfg!(target_endian = "little") { "Little" } else { "Big" });
    println!("Pointer:      {} bit", if cfg!(target_pointer_width = "64") { "64" } else { "32" });
    println!();

    // SIMD Features
    println!("⚡ SIMD Features");
    println!("─────────────────────────────────────────────────────────");

    #[cfg(target_arch = "aarch64")]
    {
        println!("✅ ARM64 NEON: Available");
        println!("  NEON (Advanced SIMD): Advanced SIMD instruction set");
        println!("  - Vector width: 128 bits");
        println!("  - crypto:   {} (AES, SHA1, SHA2)", if cfg!(target_feature = "aes") { "✅" } else { "❌" });
        println!("  - aes:      {} (AES encryption)", if cfg!(target_feature = "aes") { "✅" } else { "❌" });
        println!("  - crc:      {} (CRC-32)", if cfg!(target_feature = "crc") { "✅" } else { "❌" });
        println!("  - dotprod:  {} (Dot product)", if cfg!(target_feature = "dotprod") { "✅" } else { "❌" });
        println!("  - fp16:     {} (Half-precision FP)", if cfg!(target_feature = "fp16") { "✅" } else { "❌" });
        println!("  - sve:      {} (Scalable Vector Extension)", if cfg!(target_feature = "sve") { "✅" } else { "❌" });
        println!("  - sve2:     {} (SVE version 2)", if cfg!(target_feature = "sve2") { "✅" } else { "❌" });
    }

    #[cfg(target_arch = "x86_64")]
    {
        println!("✅ x86_64 SIMD: Available");
        println!("  - sse:     {} (Streaming SIMD Extensions)", if cfg!(target_feature = "sse") { "✅" } else { "❌" });
        println!("  - sse2:    {} (SSE2)", if cfg!(target_feature = "sse2") { "✅" } else { "❌" });
        println!("  - sse3:    {} (SSE3)", if cfg!(target_feature = "sse3") { "✅" } else { "❌" });
        println!("  - ssse3:   {} (Supplemental SSE3)", if cfg!(target_feature = "ssse3") { "✅" } else { "❌" });
        println!("  - sse4.1:  {} (SSE4.1)", if cfg!(target_feature = "sse4.1") { "✅" } else { "❌" });
        println!("  - sse4.2:  {} (SSE4.2)", if cfg!(target_feature = "sse4.2") { "✅" } else { "❌" });
        println!("  - avx:     {} (Advanced Vector Extensions)", if cfg!(target_feature = "avx") { "✅" } else { "❌" });
        println!("  - avx2:    {} (AVX2 - 256-bit vectors)", if cfg!(target_feature = "avx2") { "✅" } else { "❌" });
        println!("  - avx512f: {} (AVX-512 Foundation)", if cfg!(target_feature = "avx512f") { "✅" } else { "❌" });
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        println!("⚠️  Platform: Not ARM64 or x86_64");
        println!("   SIMD detection may not be comprehensive");
    }

    println!();

    // CPU Information
    println!("💻 CPU Information");
    println!("─────────────────────────────────────────────────────────");

    // macOS CPU info
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("sysctl")
            .arg("-n")
            .arg("machdep.cpu.brand_string")
            .output()
        {
            let cpu = String::from_utf8_lossy(&output.stdout);
            println!("CPU Model:  {}", cpu.trim());
        }

        if let Ok(output) = Command::new("sysctl")
            .arg("-n")
            .arg("hw.physicalcpu")
            .output()
        {
            let cores = String::from_utf8_lossy(&output.stdout);
            println!("Physical Cores: {}", cores.trim());
        }

        if let Ok(output) = Command::new("sysctl")
            .arg("-n")
            .arg("hw.logicalcpu")
            .output()
        {
            let threads = String::from_utf8_lossy(&output.stdout);
            println!("Logical Threads: {}", threads.trim());
        }

        if let Ok(output) = Command::new("sysctl")
            .arg("-n")
            .arg("hw.memsize")
            .output()
        {
            let mem = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u64>()
                .unwrap_or(0);
            println!("Total Memory:  {:.2} GB", mem as f64 / 1024.0 / 1024.0 / 1024.0 / 1024.0);
        }

        if let Ok(output) = Command::new("sysctl")
            .arg("-n")
            .arg("hw.cpufrequency")
            .output()
        {
            let freq = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u64>()
                .unwrap_or(0);
            println!("CPU Frequency: {:.2} GHz", freq as f64 / 1_000_000_000.0);
        }
    }

    // Linux CPU info
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = Command::new("sh")
            .arg("-c")
            .arg("cat /proc/cpuinfo | grep 'model name' | head -1")
            .output()
        {
            let cpu = String::from_utf8_lossy(&output.stdout);
            if let Some(model) = cpu.split(':').nth(1) {
                println!("CPU Model:  {}", model.trim());
            }
        }

        if let Ok(output) = Command::new("nproc")
            .output()
        {
            let cores = String::from_utf8_lossy(&output.stdout);
            println!("CPU Cores:  {}", cores.trim());
        }
    }

    println!();

    // Rust Compilation Information
    println!("🦀 Rust Compilation Information");
    println!("─────────────────────────────────────────────────────────");
    println!("Target:        {}", std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string()));
    println!("Opt Level:      {}", std::env::var("OPT_LEVEL").unwrap_or_else(|_| "unknown".to_string()));
    println!("Debug Assert:   {}", std::env::var("CFG_DEBUG_ASSERTIONS").unwrap_or_else(|_| "unknown".to_string()));

    if let Ok(rustc) = Command::new("rustc")
        .arg("--version")
        .output()
    {
        let version = String::from_utf8_lossy(&rustc.stdout);
        println!("Rustc Version: {}", version.trim());
    }

    println!();

    // SIMD Capability Summary
    println!("📊 SIMD Capability Summary");
    println!("─────────────────────────────────────────────────────────");

    #[cfg(target_arch = "aarch64")]
    {
        println!("ARM64 Platform:");
        println!("  ✅ 128-bit NEON vectors");
        println!("  ✅ 4 × float32 or 2 × float64 per vector");
        println!("  ✅ Comprehensive instruction set");
        println!();
        println!("Recommended optimizations:");
        println!("  • Use std::arch::aarch64 intrinsics");
        println!("  • Target 4-element f32 arrays for best throughput");
        println!("  • Leverage FMA (fused multiply-add) when possible");
    }

    #[cfg(target_arch = "x86_64")]
    {
        println!("x86_64 Platform:");
        if cfg!(target_feature = "avx2") {
            println!("  ✅ AVX2: 256-bit vectors available");
            println!("  ✅ 8 × float32 or 4 × float64 per vector");
            println!("  ✅ Best SIMD performance");
        } else if cfg!(target_feature = "avx") {
            println!("  ⚠️  AVX: 256-bit vectors (no AVX2)");
        } else if cfg!(target_feature = "sse4.2") {
            println!("  ⚠️  SSE4.2: 128-bit vectors");
        } else {
            println!("  ⚠️  Limited SIMD support");
        }
        println!();
        println!("Recommended optimizations:");
        println!("  • Use widest available vector (AVX2 > AVX > SSE)");
        println!("  • Check target_feature at compile time");
        println!("  • Provide scalar fallbacks");
    }

    println!();
    println!("════════════════════════════════════════════════════════════");
    println!("Detection complete!");
    println!("════════════════════════════════════════════════════════════");
}
