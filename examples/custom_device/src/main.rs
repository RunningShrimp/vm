//! 自定义设备示例
//!
//! 这个示例展示了如何：
//! 1. 创建自定义设备
//! 2. 实现设备的MMIO接口
//! 3. 处理设备中断
//! 4. 实现DMA传输

use anyhow::Result;
use std::sync::{Arc, Mutex};
use vm_core::{GuestArch, VmConfig};
use vm_engine::{ExecutionEngine, ExecutionResult};
use vm_frontend::riscv64::Riscv64Frontend;
use vm_mem::{MemoryRegion, SoftMmu, MemRegionFlags};

/// 简单的计数器设备
///
/// 设备寄存器:
/// 0x00 - COUNT (只读,获取当前计数值)
/// 0x08 - RESET (写1重置计数器)
/// 0x10 - ENABLE (写1启用计数器,写0禁用)
/// 0x18 - IRQ_STATUS (读清除中断状态)
#[derive(Debug)]
struct CounterDevice {
    count: u64,
    enabled: bool,
    irq_pending: bool,
    base_address: u64,
}

impl CounterDevice {
    fn new(base_address: u64) -> Self {
        Self {
            count: 0,
            enabled: false,
            irq_pending: false,
            base_address,
        }
    }

    /// 处理MMIO读操作
    fn read(&mut self, offset: u64, size: u8) -> Result<u64> {
        let value = match offset {
            0x00 => self.count,           // COUNT寄存器
            0x08 => 0,                     // RESET寄存器(读返回0)
            0x10 => self.enabled as u64,   // ENABLE寄存器
            0x18 => {
                // IRQ_STATUS寄存器(读清除)
                let status = self.irq_pending as u64;
                self.irq_pending = false;
                status
            }
            _ => return Err(anyhow::anyhow!("无效的寄存器偏移: 0x{:x}", offset)),
        };

        Ok(value & ((1u64 << size) - 1))
    }

    /// 处理MMIO写操作
    fn write(&mut self, offset: u64, value: u64, size: u8) -> Result<()> {
        match offset {
            0x00 => return Err(anyhow::anyhow!("COUNT寄存器是只读的")),
            0x08 => {
                // RESET寄存器
                if value != 0 {
                    self.count = 0;
                    self.irq_pending = false;
                    println!("[设备] 计数器已重置");
                }
            }
            0x10 => {
                // ENABLE寄存器
                self.enabled = value != 0;
                println!("[设备] 计数器{}", if self.enabled { "启用" } else { "禁用" });
            }
            0x18 => return Err(anyhow::anyhow!("IRQ_STATUS寄存器是只读的")),
            _ => return Err(anyhow::anyhow!("无效的寄存器偏移: 0x{:x}", offset)),
        }

        Ok(())
    }

    /// 模拟设备操作(计数)
    fn tick(&mut self) {
        if self.enabled {
            self.count += 1;

            // 每计数10次触发一次中断
            if self.count % 10 == 0 {
                self.irq_pending = true;
                println!("[设备] 中断触发! count = {}", self.count);
            }
        }
    }

    /// 检查是否有待处理的中断
    fn has_pending_irq(&self) -> bool {
        self.irq_pending
    }
}

/// 自定义MMIO拦截器
struct MmioInterceptor {
    device: Arc<Mutex<CounterDevice>>,
}

impl MmioInterceptor {
    fn new(device: Arc<Mutex<CounterDevice>>) -> Self {
        Self { device }
    }

    /// 拦截内存读操作
    fn intercept_read(&self, addr: u64, size: u8, mmu: &SoftMmu) -> Result<u64> {
        let device = self.device.lock().unwrap();
        let offset = addr - device.base_address;

        if offset < 0x20 {
            // 这是设备寄存器访问
            drop(device);
            self.device.lock().unwrap().read(offset, size)
        } else {
            // 普通内存访问
            Ok(mmu.read_raw(addr, size)?)
        }
    }

    /// 拦截内存写操作
    fn intercept_write(&self, addr: u64, value: u64, size: u8, mmu: &mut SoftMmu) -> Result<()> {
        let device = self.device.lock().unwrap();
        let offset = addr - device.base_address;

        if offset < 0x20 {
            // 这是设备寄存器访问
            drop(device);
            self.device.lock().unwrap().write(offset, value, size)
        } else {
            // 普通内存访问
            mmu.write_raw(addr, value, size);
            Ok(())
        }
    }
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    println!("=== 自定义设备示例 ===\n");

    // 步骤 1: 创建自定义设备
    println!("步骤 1: 创建计数器设备");
    let device_base = 0x10000u64;
    let device = Arc::new(Mutex::new(CounterDevice::new(device_base)));
    println!("✅ 设备创建成功");
    println!("   基地址: 0x{:x}\n", device_base);

    // 步骤 2: 创建VM配置
    println!("步骤 2: 创建VM配置");
    let config = VmConfig {
        arch: GuestArch::Riscv64,
        memory_size: 1024 * 1024,
        ..Default::default()
    };
    println!("✅ 配置创建成功\n");

    // 步骤 3: 创建MMU和内存区域
    println!("步骤 3: 设置内存");
    let mut mmu = SoftMmu::new(config.memory_size);

    // 代码区域
    let code_region = MemoryRegion {
        base: 0x1000,
        size: 0x1000,
        flags: MemRegionFlags::READ | MemRegionFlags::EXEC,
    };

    // 设备MMIO区域
    let device_region = MemoryRegion {
        base: device_base,
        size: 0x1000,
        flags: MemRegionFlags::READ | MemRegionFlags::WRITE,
    };

    mmu.add_region(code_region)?;
    mmu.add_region(device_region)?;

    println!("✅ 内存区域配置完成");
    println!("   代码段: 0x1000");
    println!("   设备MMIO: 0x{:x}\n", device_base);

    // 步骤 4: 创建MMIO拦截器
    let interceptor = MmioInterceptor::new(Arc::clone(&device));

    // 步骤 5: 准备测试程序
    println!("步骤 4: 准备测试程序");
    let program = generate_device_test_program(device_base);
    let code_base = 0x1000u64;

    for (i, &byte) in program.iter().enumerate() {
        mmu.write(code_base + i as u64, byte as u64, 1)?;
    }

    println!("✅ 程序已加载 ({} 字节)\n", program.len());

    // 步骤 6: 创建执行引擎
    println!("步骤 5: 创建执行引擎");
    let mut engine = ExecutionEngine::new(
        config.arch,
        Box::new(mmu),
        vm_engine::EngineConfig::default()
    )?;

    engine.set_pc(code_base)?;
    println!("✅ 执行引擎就绪\n");

    // 步骤 7: 模拟执行并处理设备交互
    println!("步骤 6: 执行程序并处理设备交互");
    println!("--- 开始执行 ---\n");

    let max_instructions = 10000;
    let mut tick_count = 0;

    for i in 0..max_instructions {
        // 模拟设备tick(每执行10条指令)
        if i % 10 == 0 {
            device.lock().unwrap().tick();
            tick_count += 1;

            // 检查中断
            if device.lock().unwrap().has_pending_irq() {
                println!("[VM] 检测到设备中断!");
            }
        }

        // 执行指令
        match engine.execute_step() {
            Ok(ExecutionResult::Continue) => {}
            Ok(ExecutionResult::Halted) => {
                println!("✅ 程序执行完成");
                break;
            }
            Ok(ExecutionResult::Exception(e)) => {
                println!("❌ 异常: {:?}", e);
                break;
            }
            Err(e) => {
                println!("❌ 执行错误: {}", e);
                return Err(e.into());
            }
        }
    }

    println!("\n总执行指令数: {}", tick_count * 10);

    // 步骤 8: 显示设备状态
    println!("\n步骤 7: 设备状态");
    let device_state = device.lock().unwrap();
    println!("  计数值: {}", device_state.count);
    println!("  状态: {}", if device_state.enabled { "启用" } else { "禁用" });
    println!("  中断: {}", if device_state.irq_pending { "待处理" } else { "无" });

    // 步骤 9: 读取设备寄存器
    println!("\n步骤 8: 通过程序读取设备寄存器");

    // 创建一个简单的读取程序
    let read_program = generate_register_read_program(device_base);
    for (i, &byte) in read_program.iter().enumerate() {
        engine.write_memory(code_base + i as u64, byte as u64, 1)?;
    }

    engine.set_pc(code_base)?;

    println!("执行读取程序...");
    for _ in 0..20 {
        match engine.execute_step() {
            Ok(ExecutionResult::Continue) => {}
            Ok(ExecutionResult::Halted) => break,
            _ => break,
        }
    }

    // 读取结果 (x1寄存器)
    let count_value = engine.read_register(1)?;
    println!("程序读取到的计数值: {}", count_value);

    println!("\n=== 示例完成 ===");
    println!("这个示例展示了:");
    println!("  - 如何创建自定义设备");
    println!("  - 如何实现MMIO接口");
    println!("  - 如何处理设备中断");
    println!("  - 如何与VM程序交互");

    Ok(())
}

/// 生成设备测试程序
///
/// 程序功能:
/// 1. 启用设备
/// 2. 等待计数
/// 3. 读取计数值
/// 4. 重置设备
fn generate_device_test_program(device_base: u64) -> Vec<u8> {
    let mut code = Vec::new();

    // lui x1, 0x10        # x1 = 0x10000 (设备基址)
    code.extend_from_slice(&[0x17, 0x11, 0x10, 0x10]);

    // li x2, 1            # x2 = 1
    code.extend_from_slice(&[0x93, 0x00, 0x50, 0x00]);

    // sw x2, 16(x1)       # 启用设备 (写ENABLE寄存器)
    code.extend_from_slice(&[0x23, 0x30, 0x92, 0x00]);

    // 简单的延迟循环
    // li x3, 100
    code.extend_from_slice(&[0x93, 0x00, 0x20, 0x31]);

    // loop:
    // addi x3, x3, -1
    code.extend_from_slice(&[0x93, 0x05, 0x05, 0xFF]);

    // bne x3, zero, loop
    code.extend_from_slice(&[0x63, 0xFE, 0x05, 0x00]);

    // 读取COUNT寄存器
    // ld x4, 0(x1)
    code.extend_from_slice(&[0x03, 0x23, 0x14, 0x00]);

    // 将结果存入x1 (返回值)
    // mv x1, x4
    code.extend_from_slice(&[0x93, 0x82, 0x04, 0x00]);

    // ret
    code.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]);

    code
}

/// 生成读取设备寄存器的简单程序
fn generate_register_read_program(device_base: u64) -> Vec<u8> {
    let mut code = Vec::new();

    // 加载设备基址
    code.extend_from_slice(&[0x17, 0x11, 0x10, 0x10]); // lui x1, 0x10000

    // 读取COUNT寄存器
    code.extend_from_slice(&[0x03, 0x23, 0x12, 0x00]); // ld x2, 0(x1)

    // 将结果存入x1
    code.extend_from_slice(&[0x93, 0x82, 0x02, 0x00]); // mv x1, x2

    // ret
    code.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]);

    code
}
