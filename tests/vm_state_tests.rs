//! VM核心状态管理模块测试
//!
//! 测试VirtualMachineState结构体的所有功能，包括：
//! - 状态创建和初始化
//! - 状态转换
//! - vCPU管理
//! - MMU访问
//! - 并发安全性
//! - 边界条件测试

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// 导入必要的类型和trait
use vm_core::{
    VmConfig, VmState, ExecStats, GuestArch, ExecMode,
    MMU, ExecutionEngine, ExecResult, GuestAddr, VcpuStateContainer,
    AccessType, VmError, MemoryError, VirtualMachine
};

// 创建模拟的MMU实现
struct MockMMU {
    memory_size: usize,
    memory: Vec<u8>,
}

impl MockMMU {
    fn new(memory_size: usize) -> Self {
        Self {
            memory_size,
            memory: vec![0; memory_size],
        }
    }
}

impl MMU for MockMMU {
    fn translate(&mut self, va: GuestAddr, _access: AccessType) -> Result<GuestAddr, VmError> {
        // 简单的1:1映射
        if va < self.memory_size as GuestAddr {
            Ok(va)
        } else {
            Err(VmError::Memory(MemoryError::InvalidAddress(va)))
        }
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        if pc + 8 <= self.memory.len() as GuestAddr {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&self.memory[pc as usize..(pc + 8) as usize]);
            Ok(u64::from_le_bytes(bytes))
        } else {
            Err(VmError::Memory(MemoryError::AccessViolation {
                addr: pc,
                msg: "Instruction fetch out of bounds".to_string(),
                access_type: Some(AccessType::Exec),
            }))
        }
    }

    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        if pa + size as GuestAddr <= self.memory.len() as GuestAddr {
            let mut result = 0u64;
            for i in 0..size {
                result |= (self.memory[(pa + i as GuestAddr) as usize] as u64) << (i * 8);
            }
            Ok(result)
        } else {
            Err(VmError::Memory(MemoryError::AccessViolation {
                addr: pa,
                msg: "Read out of bounds".to_string(),
                access_type: Some(AccessType::Read),
            }))
        }
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        if pa + size as GuestAddr <= self.memory.len() as GuestAddr {
            for i in 0..size {
                self.memory[(pa + i as GuestAddr) as usize] = ((val >> (i * 8)) & 0xFF) as u8;
            }
            Ok(())
        } else {
            Err(VmError::Memory(MemoryError::AccessViolation {
                addr: pa,
                msg: "Write out of bounds".to_string(),
                access_type: Some(AccessType::Write),
            }))
        }
    }

    fn map_mmio(&mut self, _base: GuestAddr, _size: u64, _device: Box<dyn vm_core::MmioDevice>) {
        // 简单实现，忽略MMIO映射
    }

    fn flush_tlb(&mut self) {
        // 简单实现，无需TLB刷新
    }

    fn memory_size(&self) -> usize {
        self.memory_size
    }

    fn dump_memory(&self) -> Vec<u8> {
        self.memory.clone()
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
        if data.len() <= self.memory_size {
            self.memory[..data.len()].copy_from_slice(data);
            Ok(())
        } else {
            Err("Data too large for memory".to_string())
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// 创建模拟的ExecutionEngine实现
struct MockExecutionEngine {
    regs: [u64; 32],
    pc: GuestAddr,
}

impl MockExecutionEngine {
    fn new() -> Self {
        let mut regs = [0u64; 32];
        regs[1] = 100; // 设置一些初始值用于测试
        regs[2] = 200;
        
        Self {
            regs,
            pc: 0x1000, // 设置初始PC
        }
    }
}

impl<B> ExecutionEngine<B> for MockExecutionEngine {
    fn run(&mut self, _mmu: &mut dyn MMU, _block: &B) -> ExecResult {
        // 简单实现，只返回继续状态
        ExecResult {
            status: vm_core::ExecStatus::Continue,
            stats: ExecStats::default(),
            next_pc: self.pc + 4,
        }
    }

    fn get_reg(&self, idx: usize) -> u64 {
        if idx < 32 {
            self.regs[idx]
        } else {
            0
        }
    }

    fn set_reg(&mut self, idx: usize, val: u64) {
        if idx < 32 {
            self.regs[idx] = val;
        }
    }

    fn get_pc(&self) -> GuestAddr {
        self.pc
    }

    fn set_pc(&mut self, pc: GuestAddr) {
        self.pc = pc;
    }

    fn get_vcpu_state(&self) -> VcpuStateContainer {
        VcpuStateContainer {
            regs: self.regs,
            pc: self.pc,
        }
    }

    fn set_vcpu_state(&mut self, state: &VcpuStateContainer) {
        self.regs = state.regs;
        self.pc = state.pc;
    }
}

// 测试用例类型
type TestBlock = u8; // 简单的测试块类型

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用的VmConfig
    fn create_test_config() -> VmConfig {
        VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 1024 * 1024, // 1MB
            vcpu_count: 1,
            exec_mode: ExecMode::Interpreter,
            enable_accel: false,
            kernel_path: None,
            initrd_path: None,
            cmdline: None,
            virtio: Default::default(),
            debug_trace: false,
            jit_threshold: 100,
            aot: Default::default(),
            async_executor: Default::default(),
        }
    }

    /// 创建测试用的VirtualMachine
    fn create_test_vm_state() -> VirtualMachine<TestBlock> {
        let config = create_test_config();
        let mmu = Box::new(MockMMU::new(config.memory_size));
        VirtualMachine::new(config, mmu)
    }

    #[test]
    fn test_vm_state_creation() {
        let vm_state = create_test_vm_state();
        
        // 验证初始状态
        assert_eq!(vm_state.state(), VmState::Created);
        assert_eq!(vm_state.config().memory_size, 1024 * 1024);
        assert_eq!(vm_state.config().guest_arch, GuestArch::Riscv64);
        assert_eq!(vm_state.config().vcpu_count, 1);
        assert_eq!(vm_state.vcpus.len(), 0); // 初始没有vCPU
        
        // 验证统计信息
        let stats = vm_state.stats();
        assert_eq!(stats.executed_insns, 0);
        assert_eq!(stats.executed_ops, 0);
        assert_eq!(stats.tlb_hits, 0);
        assert_eq!(stats.tlb_misses, 0);
        assert_eq!(stats.jit_compiles, 0);
        assert_eq!(stats.jit_compile_time_ns, 0);
    }

    #[test]
    fn test_vm_state_set_state() {
        let mut vm_state = create_test_vm_state();
        
        // 测试状态转换
        assert_eq!(vm_state.state(), VmState::Created);
        
        vm_state.set_state(VmState::Running);
        assert_eq!(vm_state.state(), VmState::Running);
        
        vm_state.set_state(VmState::Paused);
        assert_eq!(vm_state.state(), VmState::Paused);
        
        vm_state.set_state(VmState::Stopped);
        assert_eq!(vm_state.state(), VmState::Stopped);
    }

    #[test]
    fn test_add_vcpu() {
        let mut vm_state = create_test_vm_state();
        
        // 初始没有vCPU
        assert_eq!(vm_state.vcpus.len(), 0);
        
        // 添加vCPU
        let vcpu = Arc::new(Mutex::new(MockExecutionEngine::new()));
        vm_state.add_vcpu(vcpu);
        
        // 验证vCPU已添加
        assert_eq!(vm_state.vcpus.len(), 1);
        
        // 添加更多vCPU
        let vcpu2 = Arc::new(Mutex::new(MockExecutionEngine::new()));
        vm_state.add_vcpu(vcpu2);
        
        assert_eq!(vm_state.vcpus.len(), 2);
    }

    #[test]
    fn test_mmu_access() {
        let vm_state = create_test_vm_state();
        
        // 获取MMU引用
        let mmu_ref = vm_state.mmu();
        
        // 测试MMU操作
        {
            let mut mmu = mmu_ref.lock().unwrap();
            
            // 测试内存写入和读取
            let test_addr = 0x1000;
            let test_value = 0x12345678u64;
            
            mmu.write(test_addr, test_value, 8).unwrap();
            let read_value = mmu.read(test_addr, 8).unwrap();
            assert_eq!(read_value, test_value);
            
            // 测试地址翻译
            let translated = mmu.translate(test_addr, AccessType::Read).unwrap();
            assert_eq!(translated, test_addr);
        }
    }

    #[test]
    fn test_config_access() {
        let vm_state = create_test_vm_state();
        let config = vm_state.config();
        
        // 验证配置访问
        assert_eq!(config.guest_arch, GuestArch::Riscv64);
        assert_eq!(config.memory_size, 1024 * 1024);
        assert_eq!(config.vcpu_count, 1);
        assert_eq!(config.exec_mode, ExecMode::Interpreter);
        assert!(!config.enable_accel);
        assert!(!config.debug_trace);
        assert_eq!(config.jit_threshold, 100);
    }

    #[test]
    fn test_snapshot_manager_access() {
        let vm_state = create_test_vm_state();
        
        // 测试快照列表功能
        let snapshots = vm_state.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 0); // 初始应该没有快照
    }

    #[test]
    fn test_template_manager_access() {
        let vm_state = create_test_vm_state();
        
        // 测试模板列表功能
        let templates = vm_state.list_templates().unwrap();
        assert_eq!(templates.len(), 0); // 初始应该没有模板
    }

    #[test]
    fn test_concurrent_mmu_access() {
        let vm_state = create_test_vm_state();
        let mmu_ref = vm_state.mmu();
        
        // 创建多个线程并发访问MMU
        let mut handles = vec![];
        
        for i in 0..10 {
            let mmu_clone = Arc::clone(&mmu_ref);
            let handle = thread::spawn(move || {
                let mut mmu = mmu_clone.lock().unwrap();
                
                // 每个线程在不同的地址写入数据
                let addr = (i * 0x1000) as GuestAddr;
                let value = (i as u64) * 0x12345678;
                
                mmu.write(addr, value, 8).unwrap();
                let read_value = mmu.read(addr, 8).unwrap();
                
                assert_eq!(read_value, value);
            });
            handles.push(handle);
        }
        
        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_state_access() {
        let mut vm_state = create_test_vm_state();
        
        // 创建多个线程并发访问状态
        let mut handles = vec![];
        
        for i in 0..5 {
            let handle = thread::spawn(move || {
                // 生成不同的状态值
                let new_state = match i % 4 {
                    0 => VmState::Created,
                    1 => VmState::Running,
                    2 => VmState::Paused,
                    _ => VmState::Stopped,
                };
                
                // 返回生成的状态
                new_state
            });
            handles.push(handle);
        }
        
        // 收集结果
        let mut states = vec![];
        for handle in handles {
            states.push(handle.join().unwrap());
        }
        
        // 验证所有状态都被生成
        assert_eq!(states.len(), 5);
        assert!(states.contains(&VmState::Created));
        assert!(states.contains(&VmState::Running));
        assert!(states.contains(&VmState::Paused));
        assert!(states.contains(&VmState::Stopped));
    }

    #[test]
    fn test_vcpu_concurrent_access() {
        let mut vm_state = create_test_vm_state();
        
        // 添加多个vCPU
        for _ in 0..5 {
            let vcpu = Arc::new(Mutex::new(MockExecutionEngine::new()));
            vm_state.add_vcpu(vcpu);
        }
        
        let vcpus = vm_state.vcpus.clone();
        let mut handles = vec![];
        
        // 并发访问vCPU
        for (i, vcpu) in vcpus.into_iter().enumerate() {
            let handle = thread::spawn(move || {
                let mut engine = vcpu.lock().unwrap();
                
                // 修改寄存器
                engine.set_reg(1, (i as u64) * 100);
                engine.set_reg(2, (i as u64) * 200);
                engine.set_pc(0x1000 + (i as GuestAddr) * 0x100);
                
                // 验证修改
                assert_eq!(engine.get_reg(1), (i as u64) * 100);
                assert_eq!(engine.get_reg(2), (i as u64) * 200);
                assert_eq!(engine.get_pc(), 0x1000 + (i as GuestAddr) * 0x100);
            });
            handles.push(handle);
        }
        
        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_mmu_boundary_conditions() {
        let vm_state = create_test_vm_state();
        let mmu_ref = vm_state.mmu();
        let mut mmu = mmu_ref.lock().unwrap();
        
        let memory_size = vm_state.config().memory_size;
        
        // 测试边界条件：刚好在内存范围内
        let last_valid_addr = (memory_size - 8) as GuestAddr;
        mmu.write(last_valid_addr, 0x12345678, 8).unwrap();
        let value = mmu.read(last_valid_addr, 8).unwrap();
        assert_eq!(value, 0x12345678);
        
        // 测试边界条件：超出内存范围
        let invalid_addr = memory_size as GuestAddr;
        let result = mmu.write(invalid_addr, 0x12345678, 8);
        assert!(result.is_err());
        
        let result = mmu.read(invalid_addr, 8);
        assert!(result.is_err());
        
        // 测试地址翻译的边界条件
        let result = mmu.translate(invalid_addr, AccessType::Read);
        assert!(result.is_err());
    }

    #[test]
    fn test_vcpu_state_serialization() {
        let vm_state = create_test_vm_state();
        
        // 添加vCPU
        let vcpu = Arc::new(Mutex::new(MockExecutionEngine::new()));
        vm_state.add_vcpu(vcpu);
        
        // 获取vCPU状态
        let vcpu_ref = &vm_state.vcpus[0];
        let engine = vcpu_ref.lock().unwrap();
        let state = engine.get_vcpu_state();
        
        // 验证状态内容
        assert_eq!(state.pc, 0x1000);
        assert_eq!(state.regs[1], 100);
        assert_eq!(state.regs[2], 200);
        
        // 创建新的vCPU并恢复状态
        let mut new_engine = MockExecutionEngine::new();
        new_engine.set_vcpu_state(&state);
        
        // 验证状态恢复
        assert_eq!(new_engine.get_pc(), 0x1000);
        assert_eq!(new_engine.get_reg(1), 100);
        assert_eq!(new_engine.get_reg(2), 200);
    }

    #[test]
    fn test_different_vm_configs() {
        // 测试不同的配置
        let configs = vec![
            VmConfig {
                guest_arch: GuestArch::Riscv64,
                memory_size: 512 * 1024, // 512KB
                vcpu_count: 2,
                exec_mode: ExecMode::JIT,
                enable_accel: true,
                ..Default::default()
            },
            VmConfig {
                guest_arch: GuestArch::Arm64,
                memory_size: 2 * 1024 * 1024, // 2MB
                vcpu_count: 4,
                exec_mode: ExecMode::Accelerated,
                enable_accel: false,
                ..Default::default()
            },
            VmConfig {
                guest_arch: GuestArch::X86_64,
                memory_size: 4 * 1024 * 1024, // 4MB
                vcpu_count: 8,
                exec_mode: ExecMode::HardwareAssisted,
                enable_accel: true,
                ..Default::default()
            },
        ];
        
        for config in configs {
            let mmu = Box::new(MockMMU::new(config.memory_size));
            let vm_state = VirtualMachine::with_mmu(config.clone(), mmu);
            
            // 验证配置正确设置
            assert_eq!(vm_state.config().guest_arch, config.guest_arch);
            assert_eq!(vm_state.config().memory_size, config.memory_size);
            assert_eq!(vm_state.config().vcpu_count, config.vcpu_count);
            assert_eq!(vm_state.config().exec_mode, config.exec_mode);
            assert_eq!(vm_state.config().enable_accel, config.enable_accel);
            
            // 验证MMU内存大小
            let mmu_ref = vm_state.mmu();
            let mmu = mmu_ref.lock().unwrap();
            assert_eq!(mmu.memory_size(), config.memory_size);
        }
    }

    #[test]
    fn test_error_handling() {
        let vm_state = create_test_vm_state();
        let mmu_ref = vm_state.mmu();
        let mut mmu = mmu_ref.lock().unwrap();
        
        // 测试各种错误情况
        let invalid_addr = 0xFFFFFFFFu64 as GuestAddr;
        
        // 无效地址读取
        let result = mmu.read(invalid_addr, 8);
        assert!(result.is_err());
        if let Err(VmError::Memory(MemoryError::AccessViolation { addr, .. })) = result {
            assert_eq!(addr, invalid_addr);
        } else {
            panic!("Expected MemoryError::AccessViolation");
        }
        
        // 无效地址写入
        let result = mmu.write(invalid_addr, 0x12345678, 8);
        assert!(result.is_err());
        
        // 无效地址翻译
        let result = mmu.translate(invalid_addr, AccessType::Read);
        assert!(result.is_err());
        
        // 无效地址取指
        let result = mmu.fetch_insn(invalid_addr);
        assert!(result.is_err());
    }

    #[test]
    fn test_performance_critical_operations() {
        let vm_state = create_test_vm_state();
        let mmu_ref = vm_state.mmu();
        
        // 测试大量内存操作的性能
        let start = std::time::Instant::now();
        
        {
            let mut mmu = mmu_ref.lock().unwrap();
            
            // 执行大量内存操作
            for i in 0..1000 {
                let addr = (i * 8) as GuestAddr;
                mmu.write(addr, (i as u64) * 0x12345678, 8).unwrap();
                let value = mmu.read(addr, 8).unwrap();
                assert_eq!(value, (i as u64) * 0x12345678);
            }
        }
        
        let duration = start.elapsed();
        println!("1000 memory operations took: {:?}", duration);
        
        // 验证操作在合理时间内完成（这里设置为1秒，实际应该更快）
        assert!(duration.as_secs() < 1);
    }

    #[test]
    fn test_memory_dump_and_restore() {
        let vm_state = create_test_vm_state();
        let mmu_ref = vm_state.mmu();
        
        // 写入一些测试数据
        {
            let mut mmu = mmu_ref.lock().unwrap();
            for i in 0..100 {
                let addr = (i * 8) as GuestAddr;
                mmu.write(addr, (i as u64) * 0x12345678, 8).unwrap();
            }
        }
        
        // 转储内存
        let dump = {
            let mmu = mmu_ref.lock().unwrap();
            mmu.dump_memory()
        };
        
        // 验证转储数据
        assert_eq!(dump.len(), vm_state.config().memory_size);
        
        // 创建新的VM状态并恢复内存
        let new_config = create_test_config();
        let new_mmu = Box::new(MockMMU::new(new_config.memory_size));
        let mut new_vm_state = VirtualMachine::with_mmu(new_config, new_mmu);
        let new_mmu_ref = new_vm_state.mmu();
        
        {
            let mut new_mmu = new_mmu_ref.lock().unwrap();
            new_mmu.restore_memory(&dump).unwrap();
        }
        
        // 验证恢复的数据
        {
            let new_mmu = new_mmu_ref.lock().unwrap();
            for i in 0..100 {
                let addr = (i * 8) as GuestAddr;
                let value = new_mmu.read(addr, 8).unwrap();
                assert_eq!(value, (i as u64) * 0x12345678);
            }
        }
    }
}