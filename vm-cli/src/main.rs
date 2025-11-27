use vm_core::{VirtualMachine, VmConfig, GuestArch, ExecMode, Decoder, ExecutionEngine};
use vm_ir::IRBlock;
use vm_mem::SoftMmu;
use vm_frontend_riscv64::RiscvDecoder;
use vm_engine_interpreter::Interpreter;
use vm_osal::{host_os, host_arch};
use vm_device::block::{VirtioBlock, VirtioBlockMmio};
use vm_device::clint::{Clint, ClintMmio};
use vm_device::plic::{Plic, PlicMmio};
use vm_device::hw_detect::HardwareDetector;
use vm_device::gpu_virt::GpuManager;
use vm_device::virtio_ai::{VirtioAi, VirtioAiMmio};

use tokio;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;

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
                args.exec_mode = ExecMode::Jit;
            }
            "--hybrid" => {
                args.exec_mode = ExecMode::Hybrid;
            }
            "--accel" => {
                args.enable_accel = true;
                args.exec_mode = ExecMode::Accelerated;
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
        (s.trim_end_matches("GB").trim_end_matches("G"), 1024 * 1024 * 1024)
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
    println!("    -h, --help               Print this help message");
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().filter_or("RUST_LOG", "info")).init();
    let args = parse_args();

    let mut gpu_manager = GpuManager::new();
    if let Some(backend_name) = &args.gpu_backend {
        if let Err(e) = gpu_manager.select_backend_by_name(backend_name) {
            eprintln!("Failed to select GPU backend: {}", e);
        }
    } else {
        gpu_manager.auto_select_backend();
    }

    

    if let Err(e) = gpu_manager.init_selected_backend() {
        eprintln!("Failed to initialize GPU backend: {}", e);
    }

    if args.detect_hw {
        let summary = HardwareDetector::detect().await;
        HardwareDetector::print_summary(&summary);

        

        
        return;
    }

    log::info!("=== RISC-V64 Virtual Machine ===");
    log::info!("Host: {} / {}", host_os(), host_arch());
    log::info!("Memory: {} MB", args.memory / (1024 * 1024));
    log::info!("vCPUs: {}", args.vcpus);
    log::info!("Execution Mode: {:?}", args.exec_mode);

    // 创建 VM 配置
    let mut config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: args.memory,
        vcpu_count: args.vcpus,
        exec_mode: args.exec_mode,
        enable_accel: args.enable_accel,
        debug_trace: args.debug,
        ..Default::default()
    };

    // 配置磁盘
    if let Some(disk_path) = &args.disk {
        config.virtio.block_image = Some(disk_path.to_string_lossy().to_string());
        println!("Disk: {}", disk_path.display());
    }

    // 配置内核
    if let Some(kernel_path) = &args.kernel {
        config.kernel_path = Some(kernel_path.to_string_lossy().to_string());
        println!("Kernel: {}", kernel_path.display());
    }

    println!();

    // 创建 MMU
    let mmu = SoftMmu::new(config.memory_size, false);
    let mut vm: VirtualMachine<IRBlock> = VirtualMachine::with_mmu(config.clone(), Box::new(mmu));
    let mmu_arc = vm.mmu();

    // 初始化 CLINT (时钟中断控制器)
    let clint = Arc::new(Mutex::new(Clint::new(args.vcpus as usize, 10_000_000))); // 10MHz
    let clint_mmio = ClintMmio::new(Arc::clone(&clint));

    // 初始化 PLIC (平台级中断控制器)
    let plic = Arc::new(Mutex::new(Plic::new(127, args.vcpus as usize * 2))); // 127 sources, 2 contexts per hart
    let plic_mmio = PlicMmio::new(Arc::clone(&plic));

    // 初始化 VirtIO AI
    let virtio_ai = VirtioAiMmio::new(VirtioAi::new());

    // 映射设备到 MMIO 区域
    {
        let mut mmu = mmu_arc.lock().unwrap();
        
        // CLINT @ 0x0200_0000 (16KB)
        mmu.map_mmio(0x0200_0000, 0x10000, Box::new(clint_mmio));
        
        // PLIC @ 0x0C00_0000 (64MB)
        mmu.map_mmio(0x0C00_0000, 0x4000000, Box::new(plic_mmio));

        // VirtIO Block @ 0x1000_0000 (4KB)
        if let Some(disk_path) = &args.disk {
            match VirtioBlock::open(disk_path, false) {
                Ok(block_dev) => {
                    let block_mmio = VirtioBlockMmio::new(block_dev);
                    mmu.map_mmio(0x1000_0000, 0x1000, Box::new(block_mmio));
                    mmu.map_mmio(0x1000_1000, 0x1000, Box::new(virtio_ai));
                    println!("VirtIO Block device initialized at 0x1000_0000");
                }
                Err(e) => {
                    eprintln!("Failed to open disk image: {}", e);
                }
            }
        }
    }

    // 加载内核
    if let Some(kernel_path) = &args.kernel {
        match vm.load_kernel_file(kernel_path.to_str().unwrap(), 0x8000_0000) {
            Ok(_) => println!("Kernel loaded at 0x8000_0000"),
            Err(e) => {
                eprintln!("Failed to load kernel: {}", e);
                return;
            }
        }
    } else {
        // 如果没有指定内核，运行一个简单的测试程序
        println!("No kernel specified, running test program...");
        run_test_program(&mmu_arc);
        return;
    }

    // 启动 VM
    match vm.start() {
        Ok(_) => println!("VM started successfully"),
        Err(e) => {
            eprintln!("Failed to start VM: {}", e);
            return;
        }
    }

    // 主执行循环
    let mut decoder = RiscvDecoder;
    let mut interp = Interpreter::new();
    interp.set_reg(0, 0); // x0 = 0
    
    let mut pc = 0x8000_0000u64; // RISC-V kernel entry point
    let max_steps = 1_000_000;

    println!("\nStarting execution from PC={:#x}...\n", pc);

    for step in 0..max_steps {
        let mut mmu = mmu_arc.lock().unwrap();
        
        match decoder.decode(mmu.as_ref(), pc) {
            Ok(block) => {
                let res = interp.run(mmu.as_mut(), &block);
                
                if args.debug && step % 1000 == 0 {
                    println!("[Step {}] PC={:#x}", step, pc);
                }
                
                // 更新 PC
                match &block.term {
                    vm_ir::Terminator::Jmp { target } => {
                        if *target == pc {
                            println!("\n[Step {}] PC={:#x}: HALT (infinite loop)", step, pc);
                            break;
                        }
                        pc = *target;
                    }
                    vm_ir::Terminator::CondJmp { cond, target_true, target_false } => {
                        if interp.get_reg(*cond) != 0 {
                            pc = *target_true;
                        } else {
                            pc = *target_false;
                        }
                    }
                    vm_ir::Terminator::JmpReg { base, offset } => {
                        let base_val = interp.get_reg(*base);
                        pc = (base_val as i64 + offset) as u64;
                    }
                    vm_ir::Terminator::Ret => {
                        println!("\n[Step {}] PC={:#x}: RET", step, pc);
                        break;
                    }
                    vm_ir::Terminator::Fault { cause } => {
                        println!("\n[Step {}] PC={:#x}: FAULT (cause={})", step, pc, cause);
                        break;
                    }
                    _ => pc += 4,
                }
            }
            Err(e) => {
                eprintln!("Decode error at {:#x}: {:?}", pc, e);
                break;
            }
        }
    }
    
    println!("\n=== Execution Complete ===");
    println!("Total steps: {}", std::cmp::min(max_steps, 1_000_000));
}

fn run_test_program(mmu_arc: &Arc<Mutex<Box<dyn vm_core::MMU>>>) {
    use vm_frontend_riscv64::api::*;
    
    let code_base: u64 = 0x1000;
    let data_base: u64 = 0x100;
    
    let code = vec![
        encode_addi(1, 0, 10),          // li x1, 10
        encode_addi(2, 0, 20),          // li x2, 20
        encode_add(3, 1, 2),            // add x3, x1, x2
        encode_addi(10, 0, data_base as i32), // li x10, 0x100
        encode_sw(10, 3, 0),            // sw x3, 0(x10)
        encode_lw(4, 10, 0),            // lw x4, 0(x10)
        encode_beq(3, 4, 8),            // beq x3, x4, +8
        encode_addi(5, 0, 1),           // li x5, 1 (skipped)
        encode_addi(6, 0, 2),           // li x6, 2
        encode_jal(0, 0),               // j . (halt)
    ];

    {
        let mut mmu = mmu_arc.lock().unwrap();
        for (i, insn) in code.iter().enumerate() {
            mmu.write(code_base + (i as u64 * 4), *insn as u64, 4).unwrap();
        }
    }

    println!("Test program loaded at {:#x}", code_base);
    println!("Starting execution...\n");

    let mut decoder = RiscvDecoder;
    let mut interp = Interpreter::new();
    interp.set_reg(0, 0);
    
    let mut pc = code_base;
    
    for step in 0..100 {
        let mut mmu = mmu_arc.lock().unwrap();
        match decoder.decode(mmu.as_ref(), pc) {
            Ok(block) => {
                let _res = interp.run(mmu.as_mut(), &block);
                
                match &block.term {
                    vm_ir::Terminator::Jmp { target } => {
                        if *target == pc {
                            println!("\n[Step {}] PC={:#x}: HALT", step, pc);
                            break;
                        }
                        pc = *target;
                    }
                    vm_ir::Terminator::CondJmp { cond, target_true, target_false } => {
                        if interp.get_reg(*cond) != 0 {
                            pc = *target_true;
                        } else {
                            pc = *target_false;
                        }
                    }
                    _ => pc += 4,
                }
            }
            Err(e) => {
                println!("Decode error at {:#x}: {:?}", pc, e);
                break;
            }
        }
    }
    
    println!("\n=== Test Complete ===");
    println!("Register dump:");
    println!("  x1 = {} (expected: 10)", interp.get_reg(1));
    println!("  x2 = {} (expected: 20)", interp.get_reg(2));
    println!("  x3 = {} (expected: 30)", interp.get_reg(3));
    println!("  x4 = {} (expected: 30)", interp.get_reg(4));
    println!("  x5 = {} (expected: 0)", interp.get_reg(5));
    println!("  x6 = {} (expected: 2)", interp.get_reg(6));
}
