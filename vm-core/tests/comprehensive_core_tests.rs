//! Comprehensive tests for vm-core module
//!
//! This test suite covers core VM types, traits, and error handling.

use std::sync::RwLock;
use vm_core::{
    AccessType, CoreError, ExecMode, ExecResult, ExecStats, ExecStatus, ExecutionError, Fault,
    GuestAddr, GuestArch, GuestPhysAddr, Instruction, MemoryError, MmioDevice, SyscallContext,
    VmConfig, VmError, VmResult, VmState,
};

// ============================================================================
// GuestAddr Tests
// ============================================================================

#[test]
fn test_guest_addr_creation() {
    let addr = GuestAddr(0x1000);
    assert_eq!(addr.0, 0x1000);
}

#[test]
fn test_guest_addr_wrapping_operations() {
    let addr = GuestAddr(0xFFFF_FFFF_FFFF_FFFF);

    // Test wrapping add
    let result = addr.wrapping_add(1);
    assert_eq!(result, GuestAddr(0));

    // Test wrapping sub
    let zero = GuestAddr(0);
    let result = zero.wrapping_sub(1);
    assert_eq!(result, GuestAddr(0xFFFF_FFFF_FFFF_FFFF));

    // Test wrapping add with addr
    let addr1 = GuestAddr(0x1000);
    let addr2 = GuestAddr(0x2000);
    let result = addr1.wrapping_add_addr(addr2);
    assert_eq!(result, GuestAddr(0x3000));

    // Test wrapping sub with addr
    let result = addr2.wrapping_sub_addr(addr1);
    assert_eq!(result, GuestAddr(0x1000));
}

#[test]
fn test_guest_addr_conversions() {
    let addr = GuestAddr(0x1000);

    // Test as_i64
    assert_eq!(addr.as_i64(), 0x1000i64);

    // Test value
    assert_eq!(addr.value(), 0x1000);
}

#[test]
fn test_guest_addr_arithmetic() {
    let addr = GuestAddr(0x1000);

    // Test addition
    let result = addr + 0x100;
    assert_eq!(result, GuestAddr(0x1100));

    // Test subtraction
    let addr1 = GuestAddr(0x2000);
    let addr2 = GuestAddr(0x1000);
    let result = addr1 - addr2;
    assert_eq!(result, 0x1000);

    // Test bitand
    let result = addr & 0xFFF;
    assert_eq!(result, 0);

    // Test rem
    let result = addr % 0x100;
    assert_eq!(result, 0);

    // Test shr
    let result = addr >> 4;
    assert_eq!(result, 0x100);
}

#[test]
fn test_guest_addr_display() {
    let addr = GuestAddr(0x1000);
    assert_eq!(format!("{}", addr), "0x1000");
}

#[test]
fn test_guest_addr_hex() {
    let addr = GuestAddr(0x1000);
    assert_eq!(format!("{:#x}", addr), "0x1000");
}

#[test]
fn test_guest_addr_ord() {
    let addr1 = GuestAddr(0x1000);
    let addr2 = GuestAddr(0x2000);
    let addr3 = GuestAddr(0x1000);

    assert!(addr1 < addr2);
    assert!(addr2 > addr1);
    assert!(addr1 <= addr3);
    assert!(addr1 == addr3);
    assert!(addr1 != addr2);
}

// ============================================================================
// GuestPhysAddr Tests
// ============================================================================

#[test]
fn test_guest_phys_addr_creation() {
    let addr = GuestPhysAddr(0x1000);
    assert_eq!(addr.0, 0x1000);
}

#[test]
fn test_guest_phys_addr_to_guest_addr() {
    let phys = GuestPhysAddr(0x1000);
    let virt = phys.to_guest_addr();
    assert_eq!(virt, GuestAddr(0x1000));
}

#[test]
fn test_guest_phys_addr_from_guest_addr() {
    let virt = GuestAddr(0x1000);
    let phys = GuestPhysAddr::from(virt);
    assert_eq!(phys, GuestPhysAddr(0x1000));
}

#[test]
fn test_guest_phys_addr_arithmetic() {
    let addr = GuestPhysAddr(0x1000);

    // Test addition
    let result = addr + 0x100;
    assert_eq!(result, GuestPhysAddr(0x1100));

    // Test shr
    let result = addr >> 4;
    assert_eq!(result, 0x100);
}

// ============================================================================
// AccessType and Fault Tests
// ============================================================================

#[test]
fn test_access_type_variants() {
    let read = AccessType::Read;
    let write = AccessType::Write;
    let execute = AccessType::Execute;
    let atomic = AccessType::Atomic;

    assert_eq!(read, AccessType::Read);
    assert_eq!(write, AccessType::Write);
    assert_eq!(execute, AccessType::Execute);
    assert_eq!(atomic, AccessType::Atomic);
}

#[test]
fn test_fault_page_fault() {
    let fault = Fault::PageFault {
        addr: GuestAddr(0x1000),
        access_type: AccessType::Read,
        is_write: false,
        is_user: true,
    };

    match fault {
        Fault::PageFault {
            addr,
            access_type,
            is_write,
            is_user,
        } => {
            assert_eq!(addr, GuestAddr(0x1000));
            assert_eq!(access_type, AccessType::Read);
            assert!(!is_write);
            assert!(is_user);
        }
        _ => panic!("Expected PageFault"),
    }
}

#[test]
fn test_fault_invalid_opcode() {
    let fault = Fault::InvalidOpcode {
        pc: GuestAddr(0x1000),
        opcode: 0xDEADBEEF,
    };

    match fault {
        Fault::InvalidOpcode { pc, opcode } => {
            assert_eq!(pc, GuestAddr(0x1000));
            assert_eq!(opcode, 0xDEADBEEF);
        }
        _ => panic!("Expected InvalidOpcode"),
    }
}

#[test]
fn test_fault_other_variants() {
    let gp = Fault::GeneralProtection;
    let seg = Fault::SegmentFault;
    let align = Fault::AlignmentFault;
    let bus = Fault::BusError;

    assert_eq!(gp, Fault::GeneralProtection);
    assert_eq!(seg, Fault::SegmentFault);
    assert_eq!(align, Fault::AlignmentFault);
    assert_eq!(bus, Fault::BusError);
}

// ============================================================================
// GuestArch Tests
// ============================================================================

#[test]
fn test_guest_arch_variants() {
    let riscv = GuestArch::Riscv64;
    let arm = GuestArch::Arm64;
    let x86 = GuestArch::X86_64;
    let powerpc = GuestArch::PowerPC64;

    assert_eq!(riscv, GuestArch::Riscv64);
    assert_eq!(arm, GuestArch::Arm64);
    assert_eq!(x86, GuestArch::X86_64);
    assert_eq!(powerpc, GuestArch::PowerPC64);
}

#[test]
fn test_guest_arch_name() {
    assert_eq!(GuestArch::Riscv64.name(), "riscv64");
    assert_eq!(GuestArch::Arm64.name(), "arm64");
    assert_eq!(GuestArch::X86_64.name(), "x86_64");
    assert_eq!(GuestArch::PowerPC64.name(), "powerpc64");
}

// ============================================================================
// VmConfig Tests
// ============================================================================

#[test]
fn test_vm_config_default() {
    let config = VmConfig::default();
    assert_eq!(config.guest_arch, GuestArch::Riscv64);
    assert_eq!(config.memory_size, 128 * 1024 * 1024);
    assert_eq!(config.vcpu_count, 1);
    assert_eq!(config.exec_mode, ExecMode::Interpreter);
    assert!(config.kernel_path.is_none());
    assert!(config.initrd_path.is_none());
}

#[test]
fn test_vm_config_custom() {
    let config = VmConfig {
        guest_arch: GuestArch::X86_64,
        memory_size: 256 * 1024 * 1024,
        vcpu_count: 4,
        exec_mode: ExecMode::JIT,
        kernel_path: Some("/path/to/kernel".to_string()),
        initrd_path: Some("/path/to/initrd".to_string()),
    };

    assert_eq!(config.guest_arch, GuestArch::X86_64);
    assert_eq!(config.memory_size, 256 * 1024 * 1024);
    assert_eq!(config.vcpu_count, 4);
    assert_eq!(config.exec_mode, ExecMode::JIT);
    assert_eq!(config.kernel_path, Some("/path/to/kernel".to_string()));
    assert_eq!(config.initrd_path, Some("/path/to/initrd".to_string()));
}

// ============================================================================
// ExecMode Tests
// ============================================================================

#[test]
fn test_exec_mode_variants() {
    let interp = ExecMode::Interpreter;
    let jit = ExecMode::JIT;
    let hw = ExecMode::HardwareAssisted;

    assert_eq!(interp, ExecMode::Interpreter);
    assert_eq!(jit, ExecMode::JIT);
    assert_eq!(hw, ExecMode::HardwareAssisted);
}

// ============================================================================
// VmState Tests
// ============================================================================

#[test]
fn test_vm_state_default() {
    let state = VmState::default();
    assert_eq!(state.pc, GuestAddr(0));
    assert!(state.memory.is_empty());
}

// ============================================================================
// Instruction Tests
// ============================================================================

#[test]
fn test_instruction_creation() {
    let insn = Instruction {
        opcode: 0x01,
        operands: vec![0x10, 0x20, 0x30],
        length: 4,
    };

    assert_eq!(insn.opcode, 0x01);
    assert_eq!(insn.operands.len(), 3);
    assert_eq!(insn.operands[0], 0x10);
    assert_eq!(insn.length, 4);
}

#[test]
fn test_instruction_clone() {
    let insn1 = Instruction {
        opcode: 0x01,
        operands: vec![0x10, 0x20],
        length: 4,
    };

    let insn2 = insn1.clone();
    assert_eq!(insn1.opcode, insn2.opcode);
    assert_eq!(insn1.operands, insn2.operands);
    assert_eq!(insn1.length, insn2.length);
}

// ============================================================================
// SyscallContext Tests
// ============================================================================

#[test]
fn test_syscall_context_default() {
    let ctx = SyscallContext::default();
    assert_eq!(ctx.syscall_no, 0);
    assert_eq!(ctx.args, [0; 6]);
    assert_eq!(ctx.ret, 0);
    assert_eq!(ctx.errno, 0);
    assert_eq!(ctx.brk_addr, GuestAddr(0));
}

#[test]
fn test_syscall_context_custom() {
    let ctx = SyscallContext {
        syscall_no: 100,
        args: [1, 2, 3, 4, 5, 6],
        ret: -1,
        errno: 2,
        brk_addr: GuestAddr(0x1000),
    };

    assert_eq!(ctx.syscall_no, 100);
    assert_eq!(ctx.args[0], 1);
    assert_eq!(ctx.args[5], 6);
    assert_eq!(ctx.ret, -1);
    assert_eq!(ctx.errno, 2);
    assert_eq!(ctx.brk_addr, GuestAddr(0x1000));
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_vm_error_from_fault() {
    let fault = Fault::PageFault {
        addr: GuestAddr(0x1000),
        access_type: AccessType::Read,
        is_write: false,
        is_user: false,
    };

    let vm_error: VmError = VmError::from(fault);
    match vm_error {
        VmError::Execution(_) => {}
        _ => panic!("Expected Execution error"),
    }
}

#[test]
fn test_vm_error_display() {
    let error = VmError::Memory(MemoryError::InvalidAddress(GuestAddr(0x1000)));
    let error_str = format!("{}", error);
    assert!(error_str.contains("0x1000"));
}

#[test]
fn test_core_error_invalid_parameter() {
    let error = CoreError::InvalidParameter {
        name: "test".to_string(),
        value: "value".to_string(),
        message: "test message".to_string(),
    };

    match error {
        CoreError::InvalidParameter {
            name,
            value,
            message,
        } => {
            assert_eq!(name, "test");
            assert_eq!(value, "value");
            assert_eq!(message, "test message");
        }
        _ => panic!("Expected InvalidParameter error"),
    }
}

#[test]
fn test_execution_error_fault_conversion() {
    let fault = Fault::AlignmentFault;
    let error: ExecutionError = ExecutionError::Fault(fault);

    match error {
        ExecutionError::Fault(f) => {
            assert_eq!(f, Fault::AlignmentFault);
        }
        _ => panic!("Expected Fault error"),
    }
}

// ============================================================================
// ExecStats and ExecResult Tests
// ============================================================================

#[test]
fn test_exec_stats_default() {
    let stats = ExecStats::default();
    assert_eq!(stats.executed_ops, 0);
    assert_eq!(stats.executed_insns, 0);
    assert_eq!(stats.mem_accesses, 0);
    assert_eq!(stats.exec_time_ns, 0);
    assert_eq!(stats.tlb_hits, 0);
    assert_eq!(stats.tlb_misses, 0);
    assert_eq!(stats.jit_compiles, 0);
    assert_eq!(stats.jit_compile_time_ns, 0);
}

#[test]
fn test_exec_stats_custom() {
    let stats = ExecStats {
        executed_ops: 100,
        executed_insns: 100,
        mem_accesses: 50,
        exec_time_ns: 1000,
        tlb_hits: 80,
        tlb_misses: 20,
        jit_compiles: 5,
        jit_compile_time_ns: 500,
    };

    assert_eq!(stats.executed_ops, 100);
    assert_eq!(stats.executed_insns, 100);
    assert_eq!(stats.mem_accesses, 50);
    assert_eq!(stats.exec_time_ns, 1000);
    assert_eq!(stats.tlb_hits, 80);
    assert_eq!(stats.tlb_misses, 20);
    assert_eq!(stats.jit_compiles, 5);
    assert_eq!(stats.jit_compile_time_ns, 500);
}

#[test]
fn test_exec_result_continue() {
    let result = ExecResult {
        status: ExecStatus::Continue,
        stats: ExecStats::default(),
        next_pc: GuestAddr(0x2000),
    };

    match result.status {
        ExecStatus::Continue => {}
        _ => panic!("Expected Continue status"),
    }
    assert_eq!(result.next_pc, GuestAddr(0x2000));
}

#[test]
fn test_exec_result_ok() {
    let result = ExecResult {
        status: ExecStatus::Ok,
        stats: ExecStats {
            executed_insns: 10,
            ..Default::default()
        },
        next_pc: GuestAddr(0x3000),
    };

    match result.status {
        ExecStatus::Ok => {}
        _ => panic!("Expected Ok status"),
    }
    assert_eq!(result.stats.executed_insns, 10);
}

#[test]
fn test_exec_result_fault() {
    let error = ExecutionError::Fault(Fault::AlignmentFault);
    let result = ExecResult {
        status: ExecStatus::Fault(error.clone()),
        stats: ExecStats::default(),
        next_pc: GuestAddr(0x1000),
    };

    match result.status {
        ExecStatus::Fault(e) => {
            assert_eq!(e, error);
        }
        _ => panic!("Expected Fault status"),
    }
}

#[test]
fn test_exec_result_io_request() {
    let result = ExecResult {
        status: ExecStatus::IoRequest,
        stats: ExecStats::default(),
        next_pc: GuestAddr(0x1000),
    };

    match result.status {
        ExecStatus::IoRequest => {}
        _ => panic!("Expected IoRequest status"),
    }
}

#[test]
fn test_exec_result_interrupt_pending() {
    let result = ExecResult {
        status: ExecStatus::InterruptPending,
        stats: ExecStats::default(),
        next_pc: GuestAddr(0x1000),
    };

    match result.status {
        ExecStatus::InterruptPending => {}
        _ => panic!("Expected InterruptPending status"),
    }
}

// ============================================================================
// MmioDevice Tests
// ============================================================================

struct TestMmioDevice {
    data: RwLock<[u8; 8]>,
}

impl TestMmioDevice {
    fn new() -> Self {
        Self {
            data: RwLock::new([0; 8]),
        }
    }
}

impl MmioDevice for TestMmioDevice {
    fn read(&self, offset: u64, size: u8) -> VmResult<u64> {
        let data = self.data.read().unwrap();
        match size {
            1 => Ok(data[offset as usize] as u64),
            2 => {
                let val = u16::from_le_bytes([data[offset as usize], data[offset as usize + 1]]);
                Ok(val as u64)
            }
            4 => {
                let val = u32::from_le_bytes([
                    data[offset as usize],
                    data[offset as usize + 1],
                    data[offset as usize + 2],
                    data[offset as usize + 3],
                ]);
                Ok(val as u64)
            }
            8 => {
                let val = u64::from_le_bytes([
                    data[offset as usize],
                    data[offset as usize + 1],
                    data[offset as usize + 2],
                    data[offset as usize + 3],
                    data[offset as usize + 4],
                    data[offset as usize + 5],
                    data[offset as usize + 6],
                    data[offset as usize + 7],
                ]);
                Ok(val)
            }
            _ => Err(VmError::Execution(ExecutionError::Fault(
                Fault::AlignmentFault,
            ))),
        }
    }

    fn write(&mut self, offset: u64, value: u64, size: u8) -> VmResult<()> {
        let mut data = self.data.write().unwrap();
        match size {
            1 => data[offset as usize] = value as u8,
            2 => {
                let bytes = (value as u16).to_le_bytes();
                data[offset as usize] = bytes[0];
                data[offset as usize + 1] = bytes[1];
            }
            4 => {
                let bytes = (value as u32).to_le_bytes();
                for i in 0..4 {
                    data[offset as usize + i] = bytes[i];
                }
            }
            8 => {
                let bytes = value.to_le_bytes();
                for i in 0..8 {
                    data[offset as usize + i] = bytes[i];
                }
            }
            _ => {
                return Err(VmError::Execution(ExecutionError::Fault(
                    Fault::AlignmentFault,
                )));
            }
        }
        Ok(())
    }
}

#[test]
fn test_mmio_device_read_write() {
    let mut device = TestMmioDevice::new();

    // Write and read u8
    device.write(0, 0xAB, 1).unwrap();
    assert_eq!(device.read(0, 1).unwrap(), 0xAB);

    // Write and read u16
    device.write(0, 0xABCD, 2).unwrap();
    assert_eq!(device.read(0, 2).unwrap(), 0xABCD);

    // Write and read u32
    device.write(0, 0xDEADBEEF, 4).unwrap();
    assert_eq!(device.read(0, 4).unwrap(), 0xDEADBEEF);

    // Write and read u64
    device.write(0, 0x123456789ABCDEF0, 8).unwrap();
    assert_eq!(device.read(0, 8).unwrap(), 0x123456789ABCDEF0);
}

#[test]
fn test_mmio_device_invalid_size() {
    let device = TestMmioDevice::new();

    // Test invalid read size
    assert!(device.read(0, 3).is_err());

    // Test invalid write size
    let mut device = TestMmioDevice::new();
    assert!(device.write(0, 0, 3).is_err());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_guest_addr_overflow() {
    let addr = GuestAddr(u64::MAX);
    let result = addr.wrapping_add(1);
    assert_eq!(result, GuestAddr(0));

    let result = addr.wrapping_add(u64::MAX);
    assert_eq!(result, GuestAddr(u64::MAX - 1));
}

#[test]
fn test_guest_addr_underflow() {
    let addr = GuestAddr(0);
    let result = addr.wrapping_sub(1);
    assert_eq!(result, GuestAddr(u64::MAX));
}

#[test]
fn test_empty_vm_state() {
    let state = VmState {
        regs: Default::default(),
        memory: vec![],
        pc: GuestAddr(0),
    };

    assert!(state.memory.is_empty());
    assert_eq!(state.pc, GuestAddr(0));
}

#[test]
fn test_large_memory_vm_state() {
    let memory = vec![0u8; 1024 * 1024]; // 1MB
    let state = VmState {
        regs: Default::default(),
        memory,
        pc: GuestAddr(0x1000),
    };

    assert_eq!(state.memory.len(), 1024 * 1024);
    assert_eq!(state.pc, GuestAddr(0x1000));
}

#[test]
fn test_vm_config_extreme_values() {
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: usize::MAX, // Extremely large
        vcpu_count: 1024,        // Many vCPUs
        exec_mode: ExecMode::JIT,
        kernel_path: None,
        initrd_path: None,
    };

    assert_eq!(config.memory_size, usize::MAX);
    assert_eq!(config.vcpu_count, 1024);
}

#[test]
fn test_instruction_with_no_operands() {
    let insn = Instruction {
        opcode: 0x90, // NOP
        operands: vec![],
        length: 1,
    };

    assert_eq!(insn.opcode, 0x90);
    assert!(insn.operands.is_empty());
    assert_eq!(insn.length, 1);
}

#[test]
fn test_instruction_with_many_operands() {
    let operands = vec![0u64; 10];
    let insn = Instruction {
        opcode: 0x01,
        operands: operands.clone(),
        length: 4 + operands.len() as usize,
    };

    assert_eq!(insn.operands.len(), 10);
    assert_eq!(insn.operands, operands);
}

#[test]
fn test_syscall_context_all_zeros() {
    let ctx = SyscallContext {
        syscall_no: 0,
        args: [0; 6],
        ret: 0,
        errno: 0,
        brk_addr: GuestAddr(0),
    };

    assert_eq!(ctx.syscall_no, 0);
    assert_eq!(ctx.args, [0; 6]);
    assert_eq!(ctx.ret, 0);
    assert_eq!(ctx.errno, 0);
    assert_eq!(ctx.brk_addr, GuestAddr(0));
}

#[test]
fn test_syscall_context_max_values() {
    let ctx = SyscallContext {
        syscall_no: u64::MAX,
        args: [u64::MAX; 6],
        ret: i64::MAX,
        errno: i64::MAX,
        brk_addr: GuestAddr(u64::MAX),
    };

    assert_eq!(ctx.syscall_no, u64::MAX);
    assert_eq!(ctx.args, [u64::MAX; 6]);
    assert_eq!(ctx.ret, i64::MAX);
    assert_eq!(ctx.errno, i64::MAX);
    assert_eq!(ctx.brk_addr, GuestAddr(u64::MAX));
}

#[test]
fn test_fault_all_variants() {
    let faults = vec![
        Fault::PageFault {
            addr: GuestAddr(0),
            access_type: AccessType::Read,
            is_write: false,
            is_user: false,
        },
        Fault::GeneralProtection,
        Fault::SegmentFault,
        Fault::AlignmentFault,
        Fault::BusError,
        Fault::InvalidOpcode {
            pc: GuestAddr(0),
            opcode: 0,
        },
    ];

    // Ensure all variants can be created and compared
    for fault in faults {
        match fault {
            Fault::PageFault { .. } => {}
            Fault::GeneralProtection => {}
            Fault::SegmentFault => {}
            Fault::AlignmentFault => {}
            Fault::BusError => {}
            Fault::InvalidOpcode { .. } => {}
        }
    }
}
