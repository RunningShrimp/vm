use std::path::PathBuf;
use std::process;

use log::{error, info};
use vm_core::{ExecMode, GuestArch, VmConfig};
use vm_device::hw_detect::HardwareDetector;
use vm_osal::{host_arch, host_os};
use vm_service::VmService;

struct CliArgs {
    kernel: Option<PathBuf>,
    disk: Option<PathBuf>,
    memory: usize,
    vcpus: u32,
    exec_mode: ExecMode,
    enable_accel: bool,
    debug: bool,
    detect_hw: bool,
    gpu_backend: Option<String>,
    jit_min_threshold: Option<u64>,
    jit_max_threshold: Option<u64>,
    jit_sample_window: Option<usize>,
    jit_compile_weight: Option<f64>,
    jit_benefit_weight: Option<f64>,
    jit_share_pool: Option<bool>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            kernel: None,
            disk: None,
            memory: 128 * 1024 * 1024, // 128MB
            vcpus: 1,
            exec_mode: ExecMode::Interpreter,
            enable_accel: false,
            debug: false,
            detect_hw: false,
            gpu_backend: None,
            jit_min_threshold: None,
            jit_max_threshold: None,
            jit_sample_window: None,
            jit_compile_weight: None,
            jit_benefit_weight: None,
            jit_share_pool: None,
        }
    }
}

fn parse_args() -> CliArgs {
    let mut args = CliArgs::default();
    let mut iter = std::env::args().skip(1);

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--kernel" | "-k" => {
                if let Some(path) = iter.next() {
                    args.kernel = Some(PathBuf::from(path));
                }
            }
            "--disk" | "-d" => {
                if let Some(path) = iter.next() {
                    args.disk = Some(PathBuf::from(path));
                }
            }
            "--memory" | "-m" => {
                if let Some(mem_str) = iter.next() {
                    args.memory = parse_memory_size(&mem_str);
                }
            }
            "--vcpus" | "-c" => {
                if let Some(vcpu_str) = iter.next() {
                    args.vcpus = vcpu_str.parse().unwrap_or(1);
                }
            }
            "--jit" => {
                args.exec_mode = ExecMode::JIT;
            }
            "--hybrid" => {
                args.exec_mode = ExecMode::HardwareAssisted;
            }
            "--accel" => {
                args.enable_accel = true;
                args.exec_mode = ExecMode::HardwareAssisted;
            }
            "--debug" => {
                args.debug = true;
            }
            "--detect-hw" => {
                args.detect_hw = true;
            }
            "--gpu-backend" => {
                if let Some(name) = iter.next() {
                    args.gpu_backend = Some(name);
                }
            }
            "--jit-min-threshold" => {
                if let Some(v) = iter.next() {
                    args.jit_min_threshold = v.parse().ok();
                }
            }
            "--jit-max-threshold" => {
                if let Some(v) = iter.next() {
                    args.jit_max_threshold = v.parse().ok();
                }
            }
            "--jit-sample-window" => {
                if let Some(v) = iter.next() {
                    args.jit_sample_window = v.parse().ok();
                }
            }
            "--jit-compile-weight" => {
                if let Some(v) = iter.next() {
                    args.jit_compile_weight = v.parse().ok();
                }
            }
            "--jit-benefit-weight" => {
                if let Some(v) = iter.next() {
                    args.jit_benefit_weight = v.parse().ok();
                }
            }
            "--jit-share-pool" => {
                if let Some(v) = iter.next() {
                    args.jit_share_pool = Some(!(v == "0" || v.eq_ignore_ascii_case("false")));
                }
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", arg);
                print_usage();
                std::process::exit(1);
            }
        }
    }

    args
}

fn parse_memory_size(s: &str) -> usize {
    let s = s.trim().to_uppercase();
    let (num_str, multiplier) = if s.ends_with("G") || s.ends_with("GB") {
        (
            s.trim_end_matches("GB").trim_end_matches("G"),
            1024 * 1024 * 1024,
        )
    } else if s.ends_with("M") || s.ends_with("MB") {
        (s.trim_end_matches("MB").trim_end_matches("M"), 1024 * 1024)
    } else if s.ends_with("K") || s.ends_with("KB") {
        (s.trim_end_matches("KB").trim_end_matches("K"), 1024)
    } else {
        (s.as_str(), 1)
    };

    num_str.parse::<usize>().unwrap_or(128) * multiplier
}

fn print_usage() {
    println!("RISC-V Virtual Machine");
    println!();
    println!("USAGE:");
    println!("    vm-cli [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -k, --kernel <PATH>      Kernel image path");
    println!("    -d, --disk <PATH>        Disk image path");
    println!("    -m, --memory <SIZE>      Memory size (e.g., 256M, 1G) [default: 128M]");
    println!("    -c, --vcpus <NUM>        Number of vCPUs [default: 1]");
    println!("    --jit                    Use JIT compilation");
    println!("    --hybrid                 Use hybrid mode (interpreter + JIT)");
    println!("    --accel                  Use hardware acceleration");
    println!("    --debug                  Enable debug output");
    println!("    --detect-hw              Detect and display hardware information");
    println!("    --gpu-backend <NAME>     Select GPU backend (e.g., WGPU, Passthrough)");
    println!("    --jit-min-threshold <N>  JIT hot-min threshold");
    println!("    --jit-max-threshold <N>  JIT hot-max threshold");
    println!("    --jit-sample-window <N>  JIT sampling window size");
    println!("    --jit-compile-weight <F> Weight for compile time cost");
    println!("    --jit-benefit-weight <F> Weight for execution benefit");
    println!("    --jit-share-pool <BOOL>  Enable shared code pool [true/false]");
    println!("    -h, --help               Print this help message");
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().filter_or("RUST_LOG", "info")).init();
    if std::env::var("VM_TRACING").ok().as_deref() == Some("1") {
        let _ = tracing_subscriber::fmt::try_init();
    }
    let args = parse_args();

    if args.detect_hw {
        let summary = HardwareDetector::detect().await;
        HardwareDetector::print_summary(&summary);
        return;
    }

    info!("=== RISC-V64 Virtual Machine (Refactored) ===");
    info!("Host: {} / {}", host_os(), host_arch());
    info!("Memory: {} MB", args.memory / (1024 * 1024));
    info!("vCPUs: {}", args.vcpus);
    info!("Execution Mode: {:?}", args.exec_mode);

    let mut config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: args.memory,
        vcpu_count: args.vcpus as usize,
        exec_mode: args.exec_mode,
        ..Default::default()
    };

    // 注意：VmConfig结构体中没有virtio字段，这里我们直接设置kernel_path
    // 磁盘映像的设置可能需要通过其他方式进行，具体取决于VM的实现

    if let Some(kernel) = &args.kernel {
        config.kernel_path = Some(kernel.to_string_lossy().to_string());
    }

    let mut service = match VmService::new(config, args.gpu_backend).await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to initialize VM Service: {}", e);
            process::exit(1);
        }
    };
    if let Err(e) = service.configure_tlb_from_env() {
        error!("Failed to configure TLB from environment: {}", e);
        // 非致命错误，继续执行
    }

    if let (Some(min), Some(max)) = (args.jit_min_threshold, args.jit_max_threshold) {
        service.set_hot_config_vals(
            min as u32,
            max as u32,
            args.jit_sample_window.map(|x| x as u32),
            args.jit_compile_weight.map(|x| x as f32),
            args.jit_benefit_weight.map(|x| x as f32),
        );
    }
    if let Some(enable) = args.jit_share_pool {
        service.set_shared_pool(enable);
    }

    if let Some(kernel_path) = &args.kernel {
        let kernel_path_str = match kernel_path.to_str() {
            Some(value) => value,
            None => {
                error!("Kernel path is not valid UTF-8");
                process::exit(1);
            }
        };
        if let Err(e) = service.load_kernel(kernel_path_str, 0x8000_0000) {
            error!("Failed to load kernel: {}", e);
            process::exit(1);
        }
        if let Err(e) = service.run_async(0x8000_0000).await {
            error!("Runtime error: {}", e);
            process::exit(1);
        }
    } else {
        info!("No kernel specified, running test program...");
        let code_base = 0x1000;
        if let Err(e) = service.load_test_program(code_base) {
            error!("Failed to load test program: {}", e);
            process::exit(1);
        }
        if let Err(e) = service.run_async(code_base).await {
            error!("Runtime error: {}", e);
            process::exit(1);
        }

        // Check results
        info!("Test Results:");
        info!("  x1 = {} (expected: 10)", service.get_reg(1));
        info!("  x2 = {} (expected: 20)", service.get_reg(2));
        info!("  x3 = {} (expected: 30)", service.get_reg(3));
        info!("  x6 = {} (expected: 2)", service.get_reg(6));
    }

    info!("Execution finished.");
}
