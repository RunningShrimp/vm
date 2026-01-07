//! # Real-Mode x86 Emulator
//!
//! Minimal 16-bit x86 real-mode emulation for booting bzImage kernels.
//! This handles the initial boot sequence before the kernel switches to protected/long mode.

use vm_core::{GuestAddr, MMU, VmError, VmResult};
use super::bios::BiosInt;
use super::mode_trans::{ModeTransition, X86Mode};

/// Real-mode x86 register file
#[derive(Debug, Clone, Default)]
pub struct RealModeRegs {
    /// General-purpose registers (32-bit for convenience, though real-mode only uses low 16 bits)
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esp: u32,
    pub ebp: u32,
    pub esi: u32,
    pub edi: u32,

    /// Segment registers
    pub cs: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,
    pub fs: u16,
    pub gs: u16,

    /// Instruction pointer
    pub eip: u32,

    /// Flags register
    pub eflags: u32,
}

impl RealModeRegs {
    /// Convert segment:offset to linear address (real-mode addressing)
    #[inline]
    pub fn seg_to_linear(&self, seg: u16, offset: u16) -> u32 {
        ((seg as u32) << 4) + (offset as u32)
    }

    /// Get current CS:IP linear address
    #[inline]
    pub fn get_pc_linear(&self) -> u32 {
        self.seg_to_linear(self.cs, self.eip as u16)
    }

    /// Read a byte from memory using segment:offset addressing
    pub fn read_mem_byte(&self, mmu: &mut dyn MMU, seg: u16, offset: u16) -> VmResult<u8> {
        let linear = self.seg_to_linear(seg, offset);
        let addr = GuestAddr(linear as u64);

        // Debug logging for critical address range
        if seg == 0x1000 && (0x40..=0x50).contains(&offset) {
            log::warn!("read_mem_byte: seg={:04x}, offset={:04x} -> linear={:#08x}, addr={:#010x}",
                      seg, offset, linear, addr.0);
        }

        // Read as u64, truncate to u8
        let val = mmu.read(addr, 1)?;
        Ok(val as u8)
    }

    /// Read a word from memory using segment:offset addressing
    pub fn read_mem_word(&self, mmu: &mut dyn MMU, seg: u16, offset: u16) -> VmResult<u16> {
        let linear = self.seg_to_linear(seg, offset);
        let addr = GuestAddr(linear as u64);
        let val = mmu.read(addr, 2)?;
        Ok(val as u16)
    }

    /// Read a dword from memory using segment:offset addressing
    pub fn read_mem_dword(&self, mmu: &mut dyn MMU, seg: u16, offset: u16) -> VmResult<u32> {
        let linear = self.seg_to_linear(seg, offset);
        let addr = GuestAddr(linear as u64);
        let val = mmu.read(addr, 4)?;
        Ok(val as u32)
    }

    /// Write a byte to memory using segment:offset addressing
    pub fn write_mem_byte(&self, mmu: &mut dyn MMU, seg: u16, offset: u16, val: u8) -> VmResult<()> {
        let linear = self.seg_to_linear(seg, offset);
        let addr = GuestAddr(linear as u64);
        mmu.write(addr, val as u64, 1)
    }

    /// Write a word to memory using segment:offset addressing
    pub fn write_mem_word(&self, mmu: &mut dyn MMU, seg: u16, offset: u16, val: u16) -> VmResult<()> {
        let linear = self.seg_to_linear(seg, offset);
        let addr = GuestAddr(linear as u64);
        mmu.write(addr, val as u64, 2)
    }

    /// Write a dword to memory using segment:offset addressing
    pub fn write_mem_dword(&self, mmu: &mut dyn MMU, seg: u16, offset: u16, val: u32) -> VmResult<()> {
        let linear = self.seg_to_linear(seg, offset);
        let addr = GuestAddr(linear as u64);
        mmu.write(addr, val as u64, 4)
    }
}

/// Minimal real-mode emulator
pub struct RealModeEmulator {
    /// Registers
    regs: RealModeRegs,
    /// Whether emulation is active
    active: bool,
    /// BIOS interrupt handler
    bios: BiosInt,
    /// Mode transition manager
    mode_trans: ModeTransition,
}

impl RealModeEmulator {
    /// Create new real-mode emulator
    pub fn new() -> Self {
        let mut regs = RealModeRegs::default();

        // Initialize to typical BIOS boot state
        regs.cs = 0x07C0;  // BIOS code segment
        regs.ds = 0x07C0;  // BIOS data segment
        regs.es = 0x07C0;
        regs.ss = 0x07C0;
        regs.esp = 0x7C00;  // Stack below code
        regs.eip = 0;
        regs.eflags = 0x202;  // Interrupts enabled, reserved bit set

        Self {
            regs,
            active: false,
            bios: BiosInt::new(),
            mode_trans: ModeTransition::new(),
        }
    }

    /// Check if emulator is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate real-mode emulation
    pub fn activate(&mut self) {
        self.active = true;
        log::info!("Real-mode emulation activated: CS={:04X}, IP={:08X}",
                  self.regs.cs, self.regs.eip);
    }

    /// Deactivate and switch to 64-bit mode
    pub fn switch_to_long_mode(&mut self) -> VmResult<u64> {
        if !self.active {
            return Err(VmError::Core(vm_core::CoreError::Internal {
                message: "Real-mode not active".to_string(),
                module: "realmode".to_string(),
            }));
        }

        log::info!("Switching from real-mode to long mode");
        self.active = false;

        // Return the 64-bit entry point (typically at 0x100000)
        Ok(0x100000)
    }

    /// Fetch next instruction byte
    pub fn fetch_byte(&mut self, mmu: &mut dyn MMU) -> VmResult<u8> {
        let byte = self.regs.read_mem_byte(mmu, self.regs.cs, self.regs.eip as u16)?;
        self.regs.eip += 1;
        Ok(byte)
    }

    /// Fetch next instruction word
    pub fn fetch_word(&mut self, mmu: &mut dyn MMU) -> VmResult<u16> {
        let word = self.regs.read_mem_word(mmu, self.regs.cs, self.regs.eip as u16)?;
        self.regs.eip += 2;
        Ok(word)
    }

    /// Fetch next instruction dword
    pub fn fetch_dword(&mut self, mmu: &mut dyn MMU) -> VmResult<u32> {
        let dword = self.regs.read_mem_dword(mmu, self.regs.cs, self.regs.eip as u16)?;
        self.regs.eip += 4;
        Ok(dword)
    }

    // ===== Helper Methods for Instruction Execution =====

    /// Get 8-bit register value
    fn get_reg8(&self, reg: usize) -> u8 {
        match reg {
            0 => (self.regs.eax & 0xFF) as u8,       // AL
            1 => ((self.regs.eax >> 8) & 0xFF) as u8, // AH (simplified)
            2 => (self.regs.ecx & 0xFF) as u8,       // CL
            3 => ((self.regs.ecx >> 8) & 0xFF) as u8, // CH
            4 => (self.regs.edx & 0xFF) as u8,       // DL
            5 => ((self.regs.edx >> 8) & 0xFF) as u8, // DH
            6 => (self.regs.ebx & 0xFF) as u8,       // BL
            7 => ((self.regs.ebx >> 8) & 0xFF) as u8, // BH
            _ => 0,
        }
    }

    /// Set 8-bit register value
    fn set_reg8(&mut self, reg: usize, val: u8) {
        match reg {
            0 => self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (val as u32),       // AL
            1 => self.regs.eax = (self.regs.eax & 0xFFFF00FF) | ((val as u32) << 8), // AH
            2 => self.regs.ecx = (self.regs.ecx & 0xFFFFFF00) | (val as u32),       // CL
            3 => self.regs.ecx = (self.regs.ecx & 0xFFFF00FF) | ((val as u32) << 8), // CH
            4 => self.regs.edx = (self.regs.edx & 0xFFFFFF00) | (val as u32),       // DL
            5 => self.regs.edx = (self.regs.edx & 0xFFFF00FF) | ((val as u32) << 8), // DH
            6 => self.regs.ebx = (self.regs.ebx & 0xFFFFFF00) | (val as u32),       // BL
            7 => self.regs.ebx = (self.regs.ebx & 0xFFFF00FF) | ((val as u32) << 8), // BH
            _ => {}
        }
    }

    /// Get 16-bit register value
    fn get_reg16(&self, reg: usize) -> u16 {
        match reg {
            0 => (self.regs.eax & 0xFFFF) as u16, // AX
            1 => (self.regs.ecx & 0xFFFF) as u16, // CX
            2 => (self.regs.edx & 0xFFFF) as u16, // DX
            3 => (self.regs.ebx & 0xFFFF) as u16, // BX
            4 => self.regs.esp as u16,            // SP
            5 => self.regs.ebp as u16,            // BP
            6 => self.regs.esi as u16,            // SI
            7 => self.regs.edi as u16,            // DI
            _ => 0,
        }
    }

    /// Set 16-bit register value
    fn set_reg16(&mut self, reg: usize, val: u16) {
        match reg {
            0 => self.regs.eax = (self.regs.eax & 0xFFFF0000) | (val as u32), // AX
            1 => self.regs.ecx = (self.regs.ecx & 0xFFFF0000) | (val as u32), // CX
            2 => self.regs.edx = (self.regs.edx & 0xFFFF0000) | (val as u32), // DX
            3 => self.regs.ebx = (self.regs.ebx & 0xFFFF0000) | (val as u32), // BX
            4 => self.regs.esp = (self.regs.esp & 0xFFFF0000) | (val as u32), // SP
            5 => self.regs.ebp = (self.regs.ebp & 0xFFFF0000) | (val as u32), // BP
            6 => self.regs.esi = (self.regs.esi & 0xFFFF0000) | (val as u32), // SI
            7 => self.regs.edi = (self.regs.edi & 0xFFFF0000) | (val as u32), // DI
            _ => {}
        }
    }

    /// Push 16-bit value to stack
    fn push16(&mut self, mmu: &mut dyn MMU, val: u16) -> VmResult<()> {
        self.regs.esp = self.regs.esp.wrapping_sub(2);
        self.regs.write_mem_word(mmu, self.regs.ss, self.regs.esp as u16, val)
    }

    /// Pop 16-bit value from stack
    fn pop16(&mut self, mmu: &mut dyn MMU) -> VmResult<u16> {
        let val = self.regs.read_mem_word(mmu, self.regs.ss, self.regs.esp as u16)?;
        self.regs.esp = self.regs.esp.wrapping_add(2);
        Ok(val)
    }

    /// Perform ALU operation on 8-bit value
    fn alu_op8(&mut self, reg: usize, opcode: u8, val: u8) -> VmResult<()> {
        let dst = self.get_reg8(reg);
        let result: u8 = match opcode {
            0x80 => match reg { // ADD/OR/ADC/SBB/AND/SUB/CMP/XOR
                0 | 2 => {
                    let (res, _) = dst.overflowing_add(val);
                    res
                }
                1 | 6 => dst | val,
                3 | 5 | 7 => {
                    let (res, _) = dst.overflowing_sub(val);
                    res
                }
                4 => dst & val,
                _ => dst,
            },
            0x82 => {
                let (res, _) = dst.overflowing_sub(val);
                res
            }
            _ => dst,
        };

        self.set_reg8(reg, result);

        // Update flags (simplified)
        if result == 0 {
            self.regs.eflags |= 0x0040; // ZF
        } else {
            self.regs.eflags &= !0x0040;
        }

        Ok(())
    }

    /// Perform ALU operation on 16-bit value
    fn alu_op16(&mut self, reg: usize, opcode: u8, val: u16) -> VmResult<()> {
        let dst = self.get_reg16(reg);
        let result: u16 = match opcode {
            0x81 => match reg { // ADD/OR/ADC/SBB/AND/SUB/CMP/XOR
                0 | 2 => {
                    let (res, _) = dst.overflowing_add(val);
                    res
                }
                1 | 6 => dst | val,
                3 | 5 | 7 => {
                    let (res, _) = dst.overflowing_sub(val);
                    res
                }
                4 => dst & val,
                _ => dst,
            },
            0x83 => {
                let (res, _) = dst.overflowing_sub(val);
                res
            }
            _ => dst,
        };

        if reg < 8 {
            self.set_reg16(reg, result);
        }

        // Update flags (simplified)
        if result == 0 {
            self.regs.eflags |= 0x0040; // ZF
        } else {
            self.regs.eflags &= !0x0040;
        }

        Ok(())
    }

    /// Update zero, sign, parity flags for 8-bit result
    fn update_flags_zsp8(&mut self, result: u8) {
        // Zero flag
        if result == 0 {
            self.regs.eflags |= 0x0040;
        } else {
            self.regs.eflags &= !0x0040;
        }

        // Sign flag (bit 7)
        if result & 0x80 != 0 {
            self.regs.eflags |= 0x0080;
        } else {
            self.regs.eflags &= !0x0080;
        }

        // Parity flag (even number of 1 bits)
        let parity = result.count_ones() % 2 == 0;
        if parity {
            self.regs.eflags |= 0x0004;
        } else {
            self.regs.eflags &= !0x0004;
        }
    }

    /// Update zero, sign, parity flags for 16-bit result
    fn update_flags_zsp16(&mut self, result: u16) {
        // Zero flag
        if result == 0 {
            self.regs.eflags |= 0x0040;
        } else {
            self.regs.eflags &= !0x0040;
        }

        // Sign flag (bit 15)
        if result & 0x8000 != 0 {
            self.regs.eflags |= 0x0080;
        } else {
            self.regs.eflags &= !0x0080;
        }

        // Parity flag (low 8 bits only)
        let parity = (result as u8).count_ones() % 2 == 0;
        if parity {
            self.regs.eflags |= 0x0004;
        } else {
            self.regs.eflags &= !0x0004;
        }
    }

    /// Check if condition is true for conditional jump
    fn check_cond(&self, opcode: u8) -> bool {
        let cf = (self.regs.eflags & 0x0001) != 0;
        let zf = (self.regs.eflags & 0x0040) != 0;
        let sf = (self.regs.eflags & 0x0080) != 0;
        let of = (self.regs.eflags & 0x0800) != 0;
        let pf = (self.regs.eflags & 0x0004) != 0;

        match opcode {
            0x70 => of,                        // JO
            0x71 => !of,                       // JNO
            0x72 => cf,                        // JB/JC/JNAE
            0x73 => !cf,                       // JNB/JAE/JNC
            0x74 => zf,                        // JZ/JE
            0x75 => !zf,                       // JNZ/JNE
            0x76 => cf || zf,                  // JBE/JNA
            0x77 => !cf && !zf,                // JA/JNBE
            0x78 => sf,                        // JS
            0x79 => !sf,                       // JNS
            0x7A => pf,                        // JP/JPE
            0x7B => !pf,                       // JNP/JPO
            0x7C => sf != of,                  // JL/JNGE
            0x7D => sf == of,                  // JGE/JNL
            0x7E => zf || (sf != of),          // JLE/JNG
            0x7F => !zf && (sf == of),         // JG/JNLE
            _ => false,
        }
    }

    /// Get zero flag state
    fn get_zf(&self) -> bool {
        (self.regs.eflags & 0x0040) != 0
    }

    /// Execute instruction at current CS:IP
    pub fn execute(&mut self, mmu: &mut dyn MMU) -> VmResult<RealModeStep> {
        if !self.active {
            return Ok(RealModeStep::NotActive);
        }

        // ALWAYS log for debugging
        static mut CALL_COUNT: u32 = 0;
        unsafe {
            CALL_COUNT += 1;
            // Log first 20, then around 60-75, then every 10000
            let should_log = CALL_COUNT <= 20
                || (CALL_COUNT >= 60 && CALL_COUNT <= 80)
                || CALL_COUNT % 10000 == 0;

            if should_log {
                let count = CALL_COUNT;
                let cs = self.regs.cs;
                let ip = self.regs.eip;
                log::warn!("execute() call #{}: CS:IP={:04X}:{:08X}", count, cs, ip);
            }
        }

        // Fetch opcode
        let opcode = self.fetch_byte(mmu)?;

        // Log when at critical address 0x44 (before fetch) or 0x45 (after fetch)
        if self.regs.cs == 0x1000 && (self.regs.eip == 0x45 || self.regs.eip == 0x44) {
            log::warn!("Fetched opcode {:02X} when CS:IP={:04X}:{:08X}", opcode, self.regs.cs, self.regs.eip);

            // Dump next 10 bytes from memory for debugging using RealModeRegs
            log::warn!("Dumping next 10 bytes from memory:");
            for i in 0..10 {
                let offset = self.regs.eip + i as u32;
                if let Ok(byte) = self.regs.read_mem_byte(mmu, self.regs.cs, offset as u16) {
                    log::warn!("  CS:IP={:04X}:{:08X} = {:02X}", self.regs.cs, offset, byte);
                }
            }
        }

        match opcode {
            // ===== Arithmetic Instructions (Group 1) =====
            // MUST come before other patterns since these are low opcodes

            // ADD r/m8, r8 (00 /r)
            0x00 => {
                let eip_before = self.regs.eip;
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg8(reg);

                // Perform actual addition based on addressing mode
                if mod_val == 3 {
                    // Register to register
                    let dst = self.get_reg8(rm);
                    let result = dst.wrapping_add(src);

                    self.set_reg8(rm, result);

                    // Set flags based on result
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("ADD r8[{}], r8[{}] ({:02X} + {:02X} = {:02X}) ZF={}",
                              rm, reg, dst, src, result, (self.regs.eflags & 0x40) != 0);
                } else {
                    // Memory operations - read, modify, write
                    // Calculate effective address for ALL 16-bit addressing modes
                    let mem_addr: u16 = match (mod_val, rm) {
                        // Mod 00: No displacement
                        (0, 6) => self.fetch_word(mmu)?, // [disp16]
                        (0, 0) => { // [BX+SI]
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bx.wrapping_add(si)
                        }
                        (0, 1) => { // [BX+DI]
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bx.wrapping_add(di)
                        }
                        (0, 2) => { // [BP+SI]
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bp.wrapping_add(si)
                        }
                        (0, 3) => { // [BP+DI]
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bp.wrapping_add(di)
                        }
                        (0, 4) => (self.regs.esi & 0xFFFF) as u16, // [SI]
                        (0, 5) => (self.regs.edi & 0xFFFF) as u16, // [DI]
                        (0, 7) => (self.regs.ebx & 0xFFFF) as u16, // [BX]

                        // Mod 01: 8-bit displacement
                        (1, rm) => {
                            let disp8 = self.fetch_byte(mmu)? as i8 as u16;
                            let base = match rm {
                                0 => { // [BX+SI+disp8]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bx.wrapping_add(si)
                                }
                                1 => { // [BX+DI+disp8]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bx.wrapping_add(di)
                                }
                                2 => { // [BP+SI+disp8]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bp.wrapping_add(si)
                                }
                                3 => { // [BP+DI+disp8]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bp.wrapping_add(di)
                                }
                                4 => (self.regs.esi & 0xFFFF) as u16, // [SI+disp8]
                                5 => (self.regs.edi & 0xFFFF) as u16, // [DI+disp8]
                                6 => { // [BP+disp8]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    bp.wrapping_add(disp8)
                                }
                                7 => { // [BX+disp8]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    bx.wrapping_add(disp8)
                                }
                                _ => unreachable!(),
                            };
                            base.wrapping_add(disp8)
                        }

                        // Mod 10: 16-bit displacement
                        (2, rm) => {
                            let disp16 = self.fetch_word(mmu)? as i16 as u16;
                            let base = match rm {
                                0 => { // [BX+SI+disp16]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bx.wrapping_add(si)
                                }
                                1 => { // [BX+DI+disp16]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bx.wrapping_add(di)
                                }
                                2 => { // [BP+SI+disp16]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bp.wrapping_add(si)
                                }
                                3 => { // [BP+DI+disp16]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bp.wrapping_add(di)
                                }
                                4 => (self.regs.esi & 0xFFFF) as u16, // [SI+disp16]
                                5 => (self.regs.edi & 0xFFFF) as u16, // [DI+disp16]
                                6 => { // [BP+disp16]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    bp.wrapping_add(disp16)
                                }
                                7 => { // [BX+disp16]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    bx.wrapping_add(disp16)
                                }
                                _ => unreachable!(),
                            };
                            base.wrapping_add(disp16)
                        }

                        _ => {
                            log::warn!("ADD [mem] - unsupported addressing mode mod={}, rm={}", mod_val, rm);
                            return Ok(RealModeStep::Continue);
                        }
                    };

                    // Read current value from memory
                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_add(src);

                    // Write result back to memory
                    self.regs.write_mem_byte(mmu, self.regs.ds, mem_addr, result)?;

                    // Set flags based on result
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("ADD [mem], r8[{}] addr={:04X} ({:02X} + {:02X} = {:02X}) ZF={}",
                              reg, mem_addr, dst, src, result, (self.regs.eflags & 0x40) != 0);
                }

                Ok(RealModeStep::Continue)
            }

            // ADD r/m16, r16 (01 /r)
            0x01 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let _rm = (modrm & 7) as usize;

                let src = self.get_reg16(reg);
                log::debug!("ADD r/m16, r16 (modrm={:02X}, reg={}, src={:04X})", modrm, reg, src);

                if src == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                Ok(RealModeStep::Continue)
            }

            // ADC r/m8, r8 (10 /r) - ADD with Carry
            0x10 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg8(reg);
                let carry = if (self.regs.eflags & 0x01) != 0 { 1u8 } else { 0u8 };

                // Perform ADC with carry based on addressing mode
                if mod_val == 3 {
                    // Register to register
                    let dst = self.get_reg8(rm);
                    let result = dst.wrapping_add(src).wrapping_add(carry);

                    self.set_reg8(rm, result);

                    // Set flags based on result
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("ADC r8[{}], r8[{}] ({:02X} + {:02X} + {} = {:02X}) ZF={}",
                              rm, reg, dst, src, carry, result, (self.regs.eflags & 0x40) != 0);
                } else {
                    // Memory operations - read, modify, write
                    let mem_addr: u16 = match (mod_val, rm) {
                        // Mod 00: No displacement
                        (0, 6) => self.fetch_word(mmu)?, // [disp16]
                        (0, 0) => { // [BX+SI]
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bx.wrapping_add(si)
                        }
                        (0, 1) => { // [BX+DI]
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bx.wrapping_add(di)
                        }
                        (0, 2) => { // [BP+SI]
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bp.wrapping_add(si)
                        }
                        (0, 3) => { // [BP+DI]
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bp.wrapping_add(di)
                        }
                        (0, 4) => (self.regs.esi & 0xFFFF) as u16, // [SI]
                        (0, 5) => (self.regs.edi & 0xFFFF) as u16, // [DI]
                        (0, 7) => (self.regs.ebx & 0xFFFF) as u16, // [BX]

                        // Mod 01: 8-bit displacement
                        (1, rm) => {
                            let disp8 = self.fetch_byte(mmu)? as i8 as u16;
                            let base = match rm {
                                0 => { // [BX+SI+disp8]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bx.wrapping_add(si)
                                }
                                1 => { // [BX+DI+disp8]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bx.wrapping_add(di)
                                }
                                2 => { // [BP+SI+disp8]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bp.wrapping_add(si)
                                }
                                3 => { // [BP+DI+disp8]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bp.wrapping_add(di)
                                }
                                4 => (self.regs.esi & 0xFFFF) as u16, // [SI+disp8]
                                5 => (self.regs.edi & 0xFFFF) as u16, // [DI+disp8]
                                6 => { // [BP+disp8]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    bp.wrapping_add(disp8)
                                }
                                7 => { // [BX+disp8]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    bx.wrapping_add(disp8)
                                }
                                _ => unreachable!(),
                            };
                            base.wrapping_add(disp8)
                        }

                        // Mod 10: 16-bit displacement
                        (2, rm) => {
                            let disp16 = self.fetch_word(mmu)? as i16 as u16;
                            let base = match rm {
                                0 => { // [BX+SI+disp16]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bx.wrapping_add(si)
                                }
                                1 => { // [BX+DI+disp16]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bx.wrapping_add(di)
                                }
                                2 => { // [BP+SI+disp16]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bp.wrapping_add(si)
                                }
                                3 => { // [BP+DI+disp16]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bp.wrapping_add(di)
                                }
                                4 => (self.regs.esi & 0xFFFF) as u16, // [SI+disp16]
                                5 => (self.regs.edi & 0xFFFF) as u16, // [DI+disp16]
                                6 => { // [BP+disp16]
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    bp.wrapping_add(disp16)
                                }
                                7 => { // [BX+disp16]
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    bx.wrapping_add(disp16)
                                }
                                _ => unreachable!(),
                            };
                            base.wrapping_add(disp16)
                        }

                        _ => {
                            log::warn!("ADC [mem] - unsupported addressing mode mod={}, rm={}", mod_val, rm);
                            return Ok(RealModeStep::Continue);
                        }
                    };

                    // Read current value from memory
                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_add(src).wrapping_add(carry);

                    // Write result back to memory
                    self.regs.write_mem_byte(mmu, self.regs.ds, mem_addr, result)?;

                    // Set flags based on result
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("ADC [mem], r8[{}] addr={:04X} ({:02X} + {:02X} + {} = {:02X}) ZF={}",
                              reg, mem_addr, dst, src, carry, result, (self.regs.eflags & 0x40) != 0);
                }

                Ok(RealModeStep::Continue)
            }

            // ADC r/m16, r16 (11 /r) - ADD with Carry
            0x11 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg16(reg);
                let carry = if (self.regs.eflags & 0x01) != 0 { 1u16 } else { 0u16 };

                // Perform ADC with carry
                if mod_val == 3 {
                    // Register to register
                    let dst = self.get_reg16(rm);
                    let result = dst.wrapping_add(src).wrapping_add(carry);

                    self.set_reg16(rm, result);

                    // Set flags based on result
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("ADC r16[{}], r16[{}] ({:04X} + {:04X} + {} = {:04X}) ZF={}",
                              rm, reg, dst, src, carry, result, (self.regs.eflags & 0x40) != 0);
                } else {
                    // Memory operations
                    let mem_addr: u16 = match (mod_val, rm) {
                        (0, 6) => self.fetch_word(mmu)?,
                        (0, 0) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bx.wrapping_add(si)
                        }
                        (0, 1) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bx.wrapping_add(di)
                        }
                        (0, 2) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bp.wrapping_add(si)
                        }
                        (0, 3) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bp.wrapping_add(di)
                        }
                        (0, 4) => (self.regs.esi & 0xFFFF) as u16,
                        (0, 5) => (self.regs.edi & 0xFFFF) as u16,
                        (0, 7) => (self.regs.ebx & 0xFFFF) as u16,
                        _ => {
                            log::warn!("ADC16 [mem] - unsupported addressing mode mod={}, rm={}", mod_val, rm);
                            return Ok(RealModeStep::Continue);
                        }
                    };

                    let dst = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_add(src).wrapping_add(carry);

                    self.regs.write_mem_word(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("ADC16 [mem], r16[{}] addr={:04X} ({:04X} + {:04X} + {} = {:04X}) ZF={}",
                              reg, mem_addr, dst, src, carry, result, (self.regs.eflags & 0x40) != 0);
                }

                Ok(RealModeStep::Continue)
            }

            // ADD r8, r/m8 (02 /r)
            0x02 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let _rm = (modrm & 7) as usize;

                log::debug!("ADD r8, r/m8 (modrm={:02X}, reg={})", modrm, reg);
                Ok(RealModeStep::Continue)
            }

            // ADD r16, r/m16 (03 /r)
            0x03 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let _rm = (modrm & 7) as usize;

                log::debug!("ADD r16, r/m16 (modrm={:02X}, reg={})", modrm, reg);
                Ok(RealModeStep::Continue)
            }

            // ADD AL, imm8 (04 ib)
            0x04 => {
                let imm = self.fetch_byte(mmu)?;
                let al = (self.regs.eax & 0xFF) as u8;
                let result = al.wrapping_add(imm);

                self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (result as u32);

                // Update flags
                if result == 0 {
                    self.regs.eflags |= 0x40; // ZF
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::debug!("ADD AL, imm8 (imm={:02X}, result={:02X})", imm, result);
                Ok(RealModeStep::Continue)
            }

            // ADD AX, imm16 (05 iw)
            0x05 => {
                let imm = self.fetch_word(mmu)?;
                let ax = (self.regs.eax & 0xFFFF) as u16;
                let result = ax.wrapping_add(imm);

                self.regs.eax = (self.regs.eax & 0xFFFF0000) | (result as u32);

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::debug!("ADD AX, imm16 (imm={:04X}, result={:04X})", imm, result);
                Ok(RealModeStep::Continue)
            }

            // ADC AL, imm8 (14 ib) - ADD with Carry
            0x14 => {
                let imm = self.fetch_byte(mmu)?;
                let al = (self.regs.eax & 0xFF) as u8;
                let cf = (self.regs.eflags & 0x01) != 0; // Carry flag
                let result = al.wrapping_add(imm).wrapping_add(if cf { 1 } else { 0 });

                self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (result as u32);

                // Update flags
                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::debug!("ADC AL, imm8 (imm={:02X}, cf={}, result={:02X})", imm, cf, result);
                Ok(RealModeStep::Continue)
            }

            // ADC AX, imm16 (15 iw) - ADD with Carry
            0x15 => {
                let imm = self.fetch_word(mmu)?;
                let ax = (self.regs.eax & 0xFFFF) as u16;
                let cf = (self.regs.eflags & 0x01) != 0;
                let result = ax.wrapping_add(imm).wrapping_add(if cf { 1 } else { 0 });

                self.regs.eax = (self.regs.eax & 0xFFFF0000) | (result as u32);

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::debug!("ADC AX, imm16 (imm={:04X}, cf={}, result={:04X})", imm, cf, result);
                Ok(RealModeStep::Continue)
            }

            // SUB r/m8, r8 (28 /r)
            0x28 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg8(reg);

                if mod_val == 3 {
                    // Register to register
                    let dst = self.get_reg8(rm);
                    let result = dst.wrapping_sub(src);

                    self.set_reg8(rm, result);

                    // Set flags based on result
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("SUB r8[{}], r8[{}] ({:02X} - {:02X} = {:02X}) ZF={}",
                              rm, reg, dst, src, result, (self.regs.eflags & 0x40) != 0);
                } else {
                    // Memory operations - read, modify, write
                    let mem_addr: u16 = match (mod_val, rm) {
                        (0, 6) => self.fetch_word(mmu)?,
                        (0, 0) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bx.wrapping_add(si)
                        }
                        (0, 1) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bx.wrapping_add(di)
                        }
                        (0, 2) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bp.wrapping_add(si)
                        }
                        (0, 3) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bp.wrapping_add(di)
                        }
                        (0, 4) => (self.regs.esi & 0xFFFF) as u16,
                        (0, 5) => (self.regs.edi & 0xFFFF) as u16,
                        (0, 7) => (self.regs.ebx & 0xFFFF) as u16,
                        (1, rm) => {
                            let disp8 = self.fetch_byte(mmu)? as i8 as u16;
                            let base = match rm {
                                0 => {
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bx.wrapping_add(si)
                                }
                                1 => {
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bx.wrapping_add(di)
                                }
                                2 => {
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bp.wrapping_add(si)
                                }
                                3 => {
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bp.wrapping_add(di)
                                }
                                4 => (self.regs.esi & 0xFFFF) as u16,
                                5 => (self.regs.edi & 0xFFFF) as u16,
                                6 => {
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    bp.wrapping_add(disp8)
                                }
                                7 => {
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    bx.wrapping_add(disp8)
                                }
                                _ => unreachable!(),
                            };
                            base.wrapping_add(disp8)
                        }
                        (2, rm) => {
                            let disp16 = self.fetch_word(mmu)? as i16 as u16;
                            let base = match rm {
                                0 => {
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bx.wrapping_add(si)
                                }
                                1 => {
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bx.wrapping_add(di)
                                }
                                2 => {
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    bp.wrapping_add(si)
                                }
                                3 => {
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    let di = (self.regs.edi & 0xFFFF) as u16;
                                    bp.wrapping_add(di)
                                }
                                4 => (self.regs.esi & 0xFFFF) as u16,
                                5 => (self.regs.edi & 0xFFFF) as u16,
                                6 => {
                                    let bp = (self.regs.ebp & 0xFFFF) as u16;
                                    bp.wrapping_add(disp16)
                                }
                                7 => {
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    bx.wrapping_add(disp16)
                                }
                                _ => unreachable!(),
                            };
                            base.wrapping_add(disp16)
                        }
                        _ => {
                            log::warn!("SUB [mem] - unsupported addressing mode mod={}, rm={}", mod_val, rm);
                            return Ok(RealModeStep::Continue);
                        }
                    };

                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_sub(src);

                    self.regs.write_mem_byte(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("SUB [mem], r8[{}] addr={:04X} ({:02X} - {:02X} = {:02X}) ZF={}",
                              reg, mem_addr, dst, src, result, (self.regs.eflags & 0x40) != 0);
                }

                Ok(RealModeStep::Continue)
            }

            // SUB r/m16, r16 (29 /r)
            0x29 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg16(reg);

                if mod_val == 3 {
                    let dst = self.get_reg16(rm);
                    let result = dst.wrapping_sub(src);

                    self.set_reg16(rm, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("SUB r16[{}], r16[{}] ({:04X} - {:04X} = {:04X}) ZF={}",
                              rm, reg, dst, src, result, (self.regs.eflags & 0x40) != 0);
                } else {
                    let mem_addr: u16 = match (mod_val, rm) {
                        (0, 6) => self.fetch_word(mmu)?,
                        (0, 0) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bx.wrapping_add(si)
                        }
                        (0, 1) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bx.wrapping_add(di)
                        }
                        (0, 2) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bp.wrapping_add(si)
                        }
                        (0, 3) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bp.wrapping_add(di)
                        }
                        (0, 4) => (self.regs.esi & 0xFFFF) as u16,
                        (0, 5) => (self.regs.edi & 0xFFFF) as u16,
                        (0, 7) => (self.regs.ebx & 0xFFFF) as u16,
                        _ => {
                            log::warn!("SUB16 [mem] - unsupported addressing mode mod={}, rm={}", mod_val, rm);
                            return Ok(RealModeStep::Continue);
                        }
                    };

                    let dst = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_sub(src);

                    self.regs.write_mem_word(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("SUB16 [mem], r16[{}] addr={:04X} ({:04X} - {:04X} = {:04X}) ZF={}",
                              reg, mem_addr, dst, src, result, (self.regs.eflags & 0x40) != 0);
                }

                Ok(RealModeStep::Continue)
            }

            // SUB AL, imm8 (2C ib)
            0x2C => {
                let imm = self.fetch_byte(mmu)?;
                let al = (self.regs.eax & 0xFF) as u8;
                let result = al.wrapping_sub(imm);

                self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (result as u32);

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::warn!("SUB AL, imm8 (imm={:02X}, result={:02X}) ZF={}",
                          imm, result, (self.regs.eflags & 0x40) != 0);
                Ok(RealModeStep::Continue)
            }

            // SUB AX, imm16 (2D iw)
            0x2D => {
                let imm = self.fetch_word(mmu)?;
                let ax = (self.regs.eax & 0xFFFF) as u16;
                let result = ax.wrapping_sub(imm);

                self.regs.eax = (self.regs.eax & 0xFFFF0000) | (result as u32);

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::warn!("SUB AX, imm16 (imm={:04X}, result={:04X}) ZF={}",
                          imm, result, (self.regs.eflags & 0x40) != 0);
                Ok(RealModeStep::Continue)
            }

            // CMP r/m8, r8 (3A /r) - Compare (subtract but don't store result)
            0x3A => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg8(reg);

                if mod_val == 3 {
                    let dst = self.get_reg8(rm);
                    let result = dst.wrapping_sub(src);

                    // Set flags based on result but don't store it
                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("CMP r8[{}], r8[{}] ({:02X} - {:02X} = {:02X}) ZF={}",
                              rm, reg, dst, src, result, (self.regs.eflags & 0x40) != 0);
                } else {
                    let mem_addr: u16 = match (mod_val, rm) {
                        (0, 6) => self.fetch_word(mmu)?,
                        (0, 0) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bx.wrapping_add(si)
                        }
                        (0, 1) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bx.wrapping_add(di)
                        }
                        (0, 2) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bp.wrapping_add(si)
                        }
                        (0, 3) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bp.wrapping_add(di)
                        }
                        (0, 4) => (self.regs.esi & 0xFFFF) as u16,
                        (0, 5) => (self.regs.edi & 0xFFFF) as u16,
                        (0, 7) => (self.regs.ebx & 0xFFFF) as u16,
                        _ => {
                            log::warn!("CMP [mem] - unsupported addressing mode mod={}, rm={}", mod_val, rm);
                            return Ok(RealModeStep::Continue);
                        }
                    };

                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_sub(src);

                    // Set flags but don't write back
                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("CMP [mem], r8[{}] addr={:04X} ({:02X} - {:02X} = {:02X}) ZF={}",
                              reg, mem_addr, dst, src, result, (self.regs.eflags & 0x40) != 0);
                }

                Ok(RealModeStep::Continue)
            }

            // CMP r/m16, r16 (3B /r)
            0x3B => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg16(reg);

                if mod_val == 3 {
                    let dst = self.get_reg16(rm);
                    let result = dst.wrapping_sub(src);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("CMP r16[{}], r16[{}] ({:04X} - {:04X} = {:04X}) ZF={}",
                              rm, reg, dst, src, result, (self.regs.eflags & 0x40) != 0);
                } else {
                    let mem_addr: u16 = match (mod_val, rm) {
                        (0, 6) => self.fetch_word(mmu)?,
                        (0, 0) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bx.wrapping_add(si)
                        }
                        (0, 1) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bx.wrapping_add(di)
                        }
                        (0, 2) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            bp.wrapping_add(si)
                        }
                        (0, 3) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            bp.wrapping_add(di)
                        }
                        (0, 4) => (self.regs.esi & 0xFFFF) as u16,
                        (0, 5) => (self.regs.edi & 0xFFFF) as u16,
                        (0, 7) => (self.regs.ebx & 0xFFFF) as u16,
                        _ => {
                            log::warn!("CMP16 [mem] - unsupported addressing mode mod={}, rm={}", mod_val, rm);
                            return Ok(RealModeStep::Continue);
                        }
                    };

                    let dst = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_sub(src);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::warn!("CMP16 [mem], r16[{}] addr={:04X} ({:04X} - {:04X} = {:04X}) ZF={}",
                              reg, mem_addr, dst, src, result, (self.regs.eflags & 0x40) != 0);
                }

                Ok(RealModeStep::Continue)
            }

            // AND r/m8, r8 (20 /r)
            0x20 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let rm = (modrm & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let src = self.get_reg8(reg);

                if mod_val == 3 {
                    // Register direct mode
                    let dst = self.get_reg8(rm);
                    let result = dst & src;
                    self.set_reg8(rm, result);

                    // Update zero flag
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!("AND r8[{}], r8[{}] ({:02X} & {:02X} = {:02X})", rm, reg, dst, src, result);
                } else {
                    // Memory mode - log for now
                    log::debug!("AND [mem], r8[{}] - memory operation not implemented", reg);
                    // TODO: Implement memory AND operation
                }
                Ok(RealModeStep::Continue)
            }

            // AND r/m16, r16 (21 /r)
            0x21 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let rm = (modrm & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let src = self.get_reg16(reg);

                if mod_val == 3 {
                    // Register direct mode
                    let dst = self.get_reg16(rm);
                    let result = dst & src;
                    self.set_reg16(rm, result);

                    // Update zero flag
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!("AND r16[{}], r16[{}] ({:04X} & {:04X} = {:04X})", rm, reg, dst, src, result);
                } else {
                    // Memory mode - log for now
                    log::debug!("AND [mem], r16[{}] - memory operation not implemented", reg);
                    // TODO: Implement memory AND operation
                }
                Ok(RealModeStep::Continue)
            }

            // AND r8, r/m8 (22 /r)
            0x22 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let rm = (modrm & 7) as usize;
                let mod_val = (modrm >> 6) & 3;

                if mod_val == 3 {
                    // Register direct mode
                    let src = self.get_reg8(rm);
                    let dst = self.get_reg8(reg);
                    let result = dst & src;
                    self.set_reg8(reg, result);

                    // Update zero flag
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!("AND r8[{}], r8[{}] ({:02X} & {:02X} = {:02X})", reg, rm, dst, src, result);
                } else {
                    // Memory mode - log for now
                    log::debug!("AND r8[{}], [mem] - memory operation not implemented", reg);
                    // TODO: Implement memory AND operation
                }
                Ok(RealModeStep::Continue)
            }

            // AND r16, r/m16 (23 /r)
            0x23 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let rm = (modrm & 7) as usize;
                let mod_val = (modrm >> 6) & 3;

                if mod_val == 3 {
                    // Register direct mode
                    let src = self.get_reg16(rm);
                    let dst = self.get_reg16(reg);
                    let result = dst & src;
                    self.set_reg16(reg, result);

                    // Update zero flag
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!("AND r16[{}], r16[{}] ({:04X} & {:04X} = {:04X})", reg, rm, dst, src, result);
                } else {
                    // Memory mode - log for now
                    log::debug!("AND r16[{}], [mem] - memory operation not implemented", reg);
                    // TODO: Implement memory AND operation
                }
                Ok(RealModeStep::Continue)
            }

            // AND AL, imm8 (24 ib)
            0x24 => {
                let imm = self.fetch_byte(mmu)?;
                let al = (self.regs.eax & 0xFF) as u8;
                let result = al & imm;

                self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (result as u32);

                // Update flags
                if result == 0 {
                    self.regs.eflags |= 0x40; // ZF
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::debug!("AND AL, imm8 (imm={:02X}, result={:02X})", imm, result);
                Ok(RealModeStep::Continue)
            }

            // AND AX, imm16 (25 iw)
            0x25 => {
                let imm = self.fetch_word(mmu)?;
                let ax = (self.regs.eax & 0xFFFF) as u16;
                let result = ax & imm;

                self.regs.eax = (self.regs.eax & 0xFFFF0000) | (result as u32);

                // Update flags
                if result == 0 {
                    self.regs.eflags |= 0x40; // ZF
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::debug!("AND AX, imm16 (imm={:04X}, result={:04X})", imm, result);
                Ok(RealModeStep::Continue)
            }

            // Group 5 one-byte - INC/DEC/CALL/JMP/PUSH (FE /r)
            // Similar to FF but operates on bytes instead of words
            0xFE => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let _rm = (modrm & 7) as usize;
                let mod_val = (modrm >> 6) & 3;

                log::warn!("Group 5 (FE) opcode at CS:IP={:04X}:{:08X}, reg={}, modrm={:02X}, mod={}",
                          self.regs.cs, self.regs.eip - 2, reg, modrm, mod_val);

                match reg {
                    // INC r/m8 (FE /0)
                    0 => {
                        log::debug!("  INC r/m8 (modrm={:02X}, reg={})", modrm, reg);
                        // TODO: Implement actual INC for memory/registers
                        Ok(RealModeStep::Continue)
                    }
                    // DEC r/m8 (FE /1)
                    1 => {
                        log::debug!("  DEC r/m8 (modrm={:02X}, reg={})", modrm, reg);
                        // TODO: Implement actual DEC for memory/registers
                        Ok(RealModeStep::Continue)
                    }
                    _ => {
                        log::warn!("  Reserved Group 5 (FE) operation - treating as NOP");
                        Ok(RealModeStep::Continue)
                    }
                }
            }

            // Group 5 - INC/DEC/CALL/JMP/PUSH (FF /r)
            // This is one of the most critical instruction groups for control flow
            0xFF => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize; // reg field determines operation
                let rm = (modrm & 7) as usize;
                let mod_val = (modrm >> 6) & 3; // mod field

                log::warn!("Group 5 (FF) opcode at CS:IP={:04X}:{:08X}, reg={}, modrm={:02X}, mod={}",
                          self.regs.cs, self.regs.eip - 2, reg, modrm, mod_val);

                match reg {
                    // INC r/m16 (FF /0)
                    0 => {
                        if mod_val == 3 {
                            // Register direct mode
                            let old_val = self.get_reg16(rm);
                            let new_val = old_val.wrapping_add(1);
                            self.set_reg16(rm, new_val);

                            // Update flags
                            if new_val == 0 {
                                self.regs.eflags |= 0x40; // ZF
                            } else {
                                self.regs.eflags &= !0x40;
                            }

                            log::debug!("  INC r16[{}] = {:04X} -> {:04X}", rm, old_val, new_val);
                        } else if mod_val == 0 && rm == 0 {
                            // [BX+SI] - increment memory at DS:[BX+SI]
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            let addr = bx.wrapping_add(si);
                            log::warn!("  INC [BX+SI] with BX={:04X}, SI={:04X}, addr={:04X}", bx, si, addr);
                            match self.regs.read_mem_word(mmu, self.regs.ds, addr) {
                                Ok(old_val) => {
                                    let new_val = old_val.wrapping_add(1);
                                    self.regs.write_mem_word(mmu, self.regs.ds, addr, new_val)?;
                                    log::warn!("  INC [{:04X}] = {:04X} -> {:04X}", addr, old_val, new_val);
                                }
                                Err(e) => {
                                    log::error!("  Failed to read/write memory at DS:{:04X}: {:?}", addr, e);
                                }
                            }
                        } else {
                            // Other memory addressing modes - treat as NOP for now
                            log::warn!("  INC [mem] - treating as NOP (mod={}, rm={})", mod_val, rm);
                        }
                        Ok(RealModeStep::Continue)
                    }
                    // DEC r/m16 (FF /1)
                    1 => {
                        if mod_val == 3 {
                            let old_val = self.get_reg16(rm);
                            let new_val = old_val.wrapping_sub(1);
                            self.set_reg16(rm, new_val);

                            if new_val == 0 {
                                self.regs.eflags |= 0x40;
                            } else {
                                self.regs.eflags &= !0x40;
                            }

                            log::debug!("  DEC r16[{}] = {:04X} -> {:04X}", rm, old_val, new_val);
                        } else {
                            // Memory addressing mode - treat as NOP for now
                            log::warn!("  DEC [mem] - treating as NOP (mod={}, rm={})", mod_val, rm);
                        }
                        Ok(RealModeStep::Continue)
                    }
                    // CALL r/m16 (FF /2) - NEAR call
                    2 => {
                        log::debug!("  CALL r/m16 - pushing return address and jumping");
                        // TODO: Push return address to stack
                        // For now, just continue without actual call
                        Ok(RealModeStep::Continue)
                    }
                    // CALL r/m32 (FF /3) - FAR call (not in real mode)
                    3 => {
                        log::debug!("  CALL far pointer (shouldn't occur in real mode)");
                        Ok(RealModeStep::Continue)
                    }
                    // JMP r/m16 (FF /4) - NEAR jump
                    4 => {
                        if mod_val == 3 {
                            // Register direct mode: jump to address in register
                            let target = self.get_reg16(rm) as u16;
                            log::warn!("  JMP r16[{}] = {:04X} - PERFORMING JUMP!", rm, target);
                            self.regs.eip = target as u32;
                        } else if mod_val == 0 && rm == 7 {
                            // Special case: [BX] - jump to address stored at [DS:BX]
                            let bx = self.regs.ebx & 0xFFFF;
                            log::warn!("  JMP [BX] with BX={:04X} - reading target from DS:BX", bx);
                            match self.regs.read_mem_word(mmu, self.regs.ds, bx as u16) {
                                Ok(target) => {
                                    log::warn!("  JMP [BX] -> jumping to {:04X}", target);
                                    self.regs.eip = target as u32;
                                }
                                Err(e) => {
                                    log::error!("  Failed to read jump target from [DS:BX]: {:?}", e);
                                }
                            }
                        } else {
                            log::warn!("  JMP [mem] - treating as NOP (mod={}, rm={})", mod_val, rm);
                        }
                        Ok(RealModeStep::Continue)
                    }
                    // JMP r/m32 (FF /5) - FAR jump (not in real mode)
                    5 => {
                        log::debug!("  JMP far pointer (shouldn't occur in real mode)");
                        Ok(RealModeStep::Continue)
                    }
                    // PUSH r/m16 (FF /6)
                    6 => {
                        log::debug!("  PUSH r/m16");
                        // TODO: Implement stack push
                        Ok(RealModeStep::Continue)
                    }
                    // Reserved (FF /7)
                    _ => {
                        log::warn!("  Reserved Group 5 operation - treating as NOP");
                        Ok(RealModeStep::Continue)
                    }
                }
            }

            // ===== Data Movement =====

            // NOP
            0x90 => Ok(RealModeStep::Continue),

            // MOV reg8, imm8 (B0+reg cw)
            0xB0..=0xB3 => {
                let val = self.fetch_byte(mmu)?;
                let reg = (opcode - 0xB0) as usize;
                self.set_reg8(reg, val);
                Ok(RealModeStep::Continue)
            }

            // Group 2 - rotate/shift with imm8 (C0/C1)
            // MUST come before MOV reg16 pattern (0xB8..=0xBF) because Rust match patterns
            // are evaluated in order, and 0xB8..=0xBF would incorrectly match 0xC0/0xC1
            0xC0 | 0xC1 => {
                let modrm = self.fetch_byte(mmu)?;
                let _imm = self.fetch_byte(mmu)?;
                let reg = (modrm >> 3) & 7;
                let _rm = (modrm & 7) as usize;

                // For now, just log and continue - these are complex instructions
                log::warn!("Group 2 (C0/C1) opcode {:02X} at CS:IP={:04X}:{:08X}, reg={}, modrm={:02X}",
                          opcode, self.regs.cs, self.regs.eip - 3, reg, modrm);
                Ok(RealModeStep::Continue)
            }

            // Group 2 - rotate/shift with CL count (D2 /r)
            0xD2 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = (modrm >> 3) & 7;
                let _rm = (modrm & 7) as usize;
                let cl = (self.regs.ecx & 0xFF) as u8;

                // For now, just log and skip - these are complex rotate/shift operations
                log::warn!("Group 2 (D2) opcode at CS:IP={:04X}:{:08X}, reg={}, modrm={:02X}, CL={:02X}",
                          self.regs.cs, self.regs.eip - 2, reg, modrm, cl);
                Ok(RealModeStep::Continue)
            }

            // MOV reg16, imm16 (B8+reg dw)
            0xB8..=0xBF => {
                let val = self.fetch_word(mmu)?;
                let reg = (opcode - 0xB8) as usize;
                self.set_reg16(reg, val);
                Ok(RealModeStep::Continue)
            }

            // MOV r/m8, imm8 (C6 /0 ib)
            0xC6 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let imm = self.fetch_byte(mmu)?;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                if reg == 0 { // Only MOV is valid for C6
                    if mod_val == 3 {
                        // Register direct mode
                        self.set_reg8(rm, imm);
                        log::debug!("MOV r8[{}], imm8 (imm={:02X})", rm, imm);
                    } else if mod_val == 0 && rm == 7 {
                        // [BX] addressing mode
                        let bx = self.regs.ebx & 0xFFFF;
                        match self.regs.write_mem_byte(mmu, self.regs.ds, bx as u16, imm) {
                            Ok(_) => {
                                log::debug!("MOV [BX], imm8 (BX={:04X}, imm={:02X})", bx, imm);
                            }
                            Err(e) => {
                                log::error!("Failed to write to memory at DS:{:04X}: {:?}", bx, e);
                            }
                        }
                    } else if mod_val == 0 && rm == 0 {
                        // [BX+SI] addressing mode
                        let bx = (self.regs.ebx & 0xFFFF) as u16;
                        let si = (self.regs.esi & 0xFFFF) as u16;
                        let addr = bx.wrapping_add(si);
                        match self.regs.write_mem_byte(mmu, self.regs.ds, addr, imm) {
                            Ok(_) => {
                                log::debug!("MOV [BX+SI], imm8 (addr={:04X}, imm={:02X})", addr, imm);
                            }
                            Err(e) => {
                                log::error!("Failed to write to memory at DS:{:04X}: {:?}", addr, e);
                            }
                        }
                    } else {
                        // Other memory addressing modes - for now just log
                        log::debug!("MOV [mem], imm8 (modrm={:02X}, imm={:02X}) - addressing mode not implemented", modrm, imm);
                        // TODO: Implement all 16-bit addressing modes
                    }
                } else {
                    log::warn!("Invalid C6 extension (reg={})", reg);
                }
                Ok(RealModeStep::Continue)
            }

            // MOV r/m16, imm16 (C7 /0 iw)
            0xC7 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let imm = self.fetch_word(mmu)?;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                if reg == 0 { // Only MOV is valid for C7
                    if mod_val == 3 {
                        // Register direct mode
                        self.set_reg16(rm, imm);
                        log::debug!("MOV r16[{}], imm16 (imm={:04X})", rm, imm);
                    } else if mod_val == 0 && rm == 7 {
                        // [BX] addressing mode
                        let bx = self.regs.ebx & 0xFFFF;
                        match self.regs.write_mem_word(mmu, self.regs.ds, bx as u16, imm) {
                            Ok(_) => {
                                log::debug!("MOV [BX], imm16 (BX={:04X}, imm={:04X})", bx, imm);
                            }
                            Err(e) => {
                                log::error!("Failed to write to memory at DS:{:04X}: {:?}", bx, e);
                            }
                        }
                    } else if mod_val == 0 && rm == 0 {
                        // [BX+SI] addressing mode
                        let bx = (self.regs.ebx & 0xFFFF) as u16;
                        let si = (self.regs.esi & 0xFFFF) as u16;
                        let addr = bx.wrapping_add(si);
                        match self.regs.write_mem_word(mmu, self.regs.ds, addr, imm) {
                            Ok(_) => {
                                log::debug!("MOV [BX+SI], imm16 (addr={:04X}, imm={:04X})", addr, imm);
                            }
                            Err(e) => {
                                log::error!("Failed to write to memory at DS:{:04X}: {:?}", addr, e);
                            }
                        }
                    } else {
                        // Other memory addressing modes - for now just log
                        log::debug!("MOV [mem], imm16 (modrm={:02X}, imm={:04X}) - addressing mode not implemented", modrm, imm);
                        // TODO: Implement all 16-bit addressing modes
                    }
                } else {
                    log::warn!("Invalid C7 extension (reg={})", reg);
                }
                Ok(RealModeStep::Continue)
            }

            // XCHG r/m8, r8 (86 /r) - Exchange
            0x86 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let rm = (modrm & 7) as usize;
                let mod_val = (modrm >> 6) & 3;

                if mod_val == 3 {
                    // Register-to-register exchange
                    let reg_val = self.get_reg8(reg);
                    let rm_val = self.get_reg8(rm);
                    self.set_reg8(reg, rm_val);
                    self.set_reg8(rm, reg_val);
                    log::debug!("XCHG r8[{}], r8[{}] (swapped {:02X} <-> {:02X})", reg, rm, reg_val, rm_val);
                } else {
                    // Memory exchange - log for now
                    log::debug!("XCHG [mem], r8[{}] - memory exchange not implemented", reg);
                    // TODO: Implement memory-register exchange
                }
                Ok(RealModeStep::Continue)
            }

            // XCHG r/m16, r16 (87 /r) - Exchange
            0x87 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let rm = (modrm & 7) as usize;
                let mod_val = (modrm >> 6) & 3;

                if mod_val == 3 {
                    // Register-to-register exchange
                    let reg_val = self.get_reg16(reg);
                    let rm_val = self.get_reg16(rm);
                    self.set_reg16(reg, rm_val);
                    self.set_reg16(rm, reg_val);
                    log::debug!("XCHG r16[{}], r16[{}] (swapped {:04X} <-> {:04X})", reg, rm, reg_val, rm_val);
                } else {
                    // Memory exchange - log for now
                    log::debug!("XCHG [mem], r16[{}] - memory exchange not implemented", reg);
                    // TODO: Implement memory-register exchange
                }
                Ok(RealModeStep::Continue)
            }

            // MOV r/m, r (opcode 0x88-0x8B with ModRM)
            0x88 | 0x89 | 0x8A | 0x8B => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let rm = (modrm & 7) as usize;
                let mod_val = (modrm >> 6) & 3;

                match opcode {
                    0x88 => { // MOV r/m8, r8
                        let src = self.get_reg8(reg);
                        if mod_val == 3 {
                            // Register to register
                            self.set_reg8(rm, src);
                        } else {
                            // Register to memory - simplified: just log
                            log::debug!("MOV [mem], r8 (modrm={:02X}, src={:02X})", modrm, src);
                            // TODO: Implement memory write
                        }
                    }
                    0x89 => { // MOV r/m16, r16
                        let src = self.get_reg16(reg);
                        if mod_val == 3 {
                            // Register to register
                            self.set_reg16(rm, src);
                        } else {
                            // Register to memory - simplified: just log
                            log::debug!("MOV [mem], r16 (modrm={:02X}, src={:04X})", modrm, src);
                            // TODO: Implement memory write
                        }
                    }
                    0x8A => { // MOV r8, r/m8
                        if mod_val == 3 {
                            // Register to register
                            let src = self.get_reg8(rm);
                            self.set_reg8(reg, src);
                        } else {
                            // Memory to register - simplified: just log
                            log::debug!("MOV r8, [mem] (modrm={:02X})", modrm);
                            // TODO: Implement memory read
                        }
                    }
                    0x8B => { // MOV r16, r/m16
                        if mod_val == 3 {
                            // Register to register
                            let src = self.get_reg16(rm);
                            self.set_reg16(reg, src);
                        } else {
                            // Memory to register - simplified: just log
                            log::debug!("MOV r16, [mem] (modrm={:02X})", modrm);
                            // TODO: Implement memory read
                        }
                    }
                    _ => unreachable!(),
                }
                Ok(RealModeStep::Continue)
            }

            // MOV acc, moffs (A0-A3)
            0xA0 => { // MOV AL, moffs8
                let addr = self.fetch_word(mmu)? as u32;
                let val = self.regs.read_mem_byte(mmu, self.regs.ds, addr as u16)?;
                self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (val as u32);
                Ok(RealModeStep::Continue)
            }
            0xA1 => { // MOV AX, moffs16
                let addr = self.fetch_word(mmu)? as u32;
                let val = self.regs.read_mem_word(mmu, self.regs.ds, addr as u16)?;
                self.regs.eax = (self.regs.eax & 0xFFFF0000) | (val as u32);
                Ok(RealModeStep::Continue)
            }
            0xA2 => { // MOV moffs8, AL
                let addr = self.fetch_word(mmu)? as u32;
                self.regs.write_mem_byte(mmu, self.regs.ds, addr as u16, (self.regs.eax & 0xFF) as u8)?;
                Ok(RealModeStep::Continue)
            }
            0xA3 => { // MOV moffs16, AX
                let addr = self.fetch_word(mmu)? as u32;
                self.regs.write_mem_word(mmu, self.regs.ds, addr as u16, (self.regs.eax & 0xFFFF) as u16)?;
                Ok(RealModeStep::Continue)
            }

            // MOV seg, r/m (8E)
            0x8E => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let val = self.get_reg16(reg);
                match reg {
                    0 => self.regs.es = val,
                    1 => self.regs.cs = val,
                    2 => self.regs.ss = val,
                    3 => self.regs.ds = val,
                    _ => log::warn!("Invalid segment register: {}", reg),
                }
                Ok(RealModeStep::Continue)
            }

            // MOV to control register (0F 20-23) / MOV from control register (0F 20-23)
            // These are two-byte opcodes, need special handling
            // We'll handle them in the 0x0F case below

            // LGDT (0F 01) / LIDT (0F 01) - Load GDT/IDT
            // These are also handled below in 0x0F cases

            // ===== Stack Operations =====

            // PUSH r16 (50+reg)
            0x50..=0x57 => {
                let reg = (opcode - 0x50) as usize;
                let val = self.get_reg16(reg);
                self.push16(mmu, val)?;
                Ok(RealModeStep::Continue)
            }

            // POP r16 (58+reg)
            0x58..=0x5F => {
                let reg = (opcode - 0x58) as usize;
                let val = self.pop16(mmu)?;
                self.set_reg16(reg, val);
                Ok(RealModeStep::Continue)
            }

            // PUSHA (60) - Push All Registers
            0x60 => {
                // Push registers in order: AX, CX, DX, BX, SP, BP, SI, DI
                let sp_before = self.regs.esp;
                self.push16(mmu, (self.regs.eax & 0xFFFF) as u16)?;
                self.push16(mmu, (self.regs.ecx & 0xFFFF) as u16)?;
                self.push16(mmu, (self.regs.edx & 0xFFFF) as u16)?;
                self.push16(mmu, (self.regs.ebx & 0xFFFF) as u16)?;
                self.push16(mmu, sp_before as u16)?;
                self.push16(mmu, (self.regs.ebp & 0xFFFF) as u16)?;
                self.push16(mmu, (self.regs.esi & 0xFFFF) as u16)?;
                self.push16(mmu, (self.regs.edi & 0xFFFF) as u16)?;
                log::debug!("PUSHA - all registers pushed");
                Ok(RealModeStep::Continue)
            }

            // POPA (61) - Pop All Registers
            0x61 => {
                // Pop registers in reverse order: DI, SI, BP, SP, BX, DX, CX, AX
                let di = self.pop16(mmu)? as u32;
                let si = self.pop16(mmu)? as u32;
                let bp = self.pop16(mmu)? as u32;
                let _sp = self.pop16(mmu)? as u32; // SP value is discarded
                let bx = self.pop16(mmu)? as u32;
                let dx = self.pop16(mmu)? as u32;
                let cx = self.pop16(mmu)? as u32;
                let ax = self.pop16(mmu)? as u32;

                self.regs.edi = (self.regs.edi & 0xFFFF0000) | di;
                self.regs.esi = (self.regs.esi & 0xFFFF0000) | si;
                self.regs.ebp = (self.regs.ebp & 0xFFFF0000) | bp;
                self.regs.ebx = (self.regs.ebx & 0xFFFF0000) | bx;
                self.regs.edx = (self.regs.edx & 0xFFFF0000) | dx;
                self.regs.ecx = (self.regs.ecx & 0xFFFF0000) | cx;
                self.regs.eax = (self.regs.eax & 0xFFFF0000) | ax;

                log::debug!("POPA - all registers popped");
                Ok(RealModeStep::Continue)
            }

            // PUSHF (9C)
            0x9C => {
                self.push16(mmu, self.regs.eflags as u16)?;
                Ok(RealModeStep::Continue)
            }

            // POPF (9D)
            0x9D => {
                let flags = self.pop16(mmu)?;
                self.regs.eflags = (self.regs.eflags & 0xFFFF0000) | (flags as u32);
                Ok(RealModeStep::Continue)
            }

            // PUSH imm16 (68)
            0x68 => {
                let val = self.fetch_word(mmu)?;
                self.push16(mmu, val)?;
                Ok(RealModeStep::Continue)
            }

            // PUSH imm8 (6A)
            0x6A => {
                let val = self.fetch_byte(mmu)? as i8 as i16 as u16;
                self.push16(mmu, val)?;
                Ok(RealModeStep::Continue)
            }

            // Two-byte opcodes (0x0F prefix)
            0x0F => {
                let opcode2 = self.fetch_byte(mmu)?;
                match opcode2 {
                    // MOV to control register (0F 20 /2)
                    // MOV from control register (0F 20 /0) - not needed yet
                    0x20 => {
                        let modrm = self.fetch_byte(mmu)?;
                        let reg = ((modrm >> 3) & 7) as usize;
                        let rm = (modrm & 7) as usize;

                        // For simplicity, only handle MOV from CR
                        let val = match reg {
                            0 => self.mode_trans.cr.cr0,
                            2 => self.mode_trans.cr.cr2,
                            3 => self.mode_trans.cr.cr3,
                            4 => self.mode_trans.cr.cr4,
                            _ => {
                                log::warn!("Invalid control register read: CR{}", reg);
                                0
                            }
                        };

                        // Store in register (simplified - assumes EAX)
                        self.regs.eax = val;
                        log::debug!("MOV from CR{}: EAX = {:#010X}", reg, val);
                        Ok(RealModeStep::Continue)
                    }

                    // MOV to control register (0F 22 /2)
                    0x22 => {
                        let modrm = self.fetch_byte(mmu)?;
                        let reg = ((modrm >> 3) & 7) as usize;
                        let rm = (modrm & 7) as usize;

                        // Get value from register (simplified - assumes EAX)
                        let val = self.regs.eax as u32;
                        self.mode_trans.write_control_register(reg as u8, val)?;
                        log::debug!("MOV to CR{}: EAX = {:#010X}", reg, val);

                        // Check if we need to switch modes
                        if let Some(step) = self.mode_trans.check_mode_switch(&mut self.regs, mmu)? {
                            return Ok(step);
                        }

                        Ok(RealModeStep::Continue)
                    }

                    // LGDT/LIDT (0F 01 /2 and /3)
                    0x01 => {
                        let modrm = self.fetch_byte(mmu)?;
                        let reg = (modrm >> 3) & 7;
                        let rm = (modrm & 7) as usize;
                        let mod_val = (modrm >> 6) & 3;

                        match reg {
                            2 => {
                                // LGDT - Load Global Descriptor Table
                                // Format: LGDT m - loads 6 bytes: limit (16-bit) and base (32-bit)
                                log::info!("LGDT instruction encountered (modrm={:02X}, mod={}, rm={})", 
                                          modrm, mod_val, rm);
                                
                                // Calculate effective address based on addressing mode
                                let mem_addr: u32 = if mod_val == 0 && rm == 6 {
                                    // [disp16] - 16-bit displacement follows ModRM
                                    let disp16 = self.fetch_word(mmu)? as u32;
                                    log::debug!("LGDT [disp16] - disp16={:04X}", disp16);
                                    disp16
                                } else if mod_val == 0 {
                                    // [mem] addressing - use register-based addressing
                                    // For simplicity, handle common cases
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    let addr = bx.wrapping_add(si) as u32;
                                    log::debug!("LGDT [mem] - addr={:04X}", addr);
                                    addr
                                } else if mod_val == 2 {
                                    // [mem+disp16] - displacement follows ModRM
                                    let disp16 = self.fetch_word(mmu)? as u32;
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    let addr = bx.wrapping_add(si).wrapping_add(disp16 as u16) as u32;
                                    log::debug!("LGDT [mem+disp16] - addr={:04X}", addr);
                                    addr
                                } else {
                                    log::warn!("LGDT with unsupported addressing mode (mod={}, rm={})", mod_val, rm);
                                    // For now, don't block execution
                                    return Ok(RealModeStep::Continue);
                                };
                                
                                // Read 6-byte GDT descriptor from memory:
                                // Bytes 0-1: Limit (16-bit)
                                // Bytes 2-5: Base address (32-bit)
                                let limit = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr as u16)?;
                                let base_low = self.regs.read_mem_word(mmu, self.regs.ds, (mem_addr + 2) as u16)? as u32;
                                let base_high = self.regs.read_mem_word(mmu, self.regs.ds, (mem_addr + 4) as u16)? as u32;
                                let base = (base_high << 16) | base_low;

                                log::info!("LGDT loaded: base={:#010X}, limit={:#06X} ({} entries)",
                                          base, limit, (limit + 1) / 8);

                                // Mark that GDT has been loaded by the kernel
                                self.mode_trans.mark_gdt_loaded();
                                
                                Ok(RealModeStep::Continue)
                            }
                            3 => {
                                // LIDT - Load Interrupt Descriptor Table
                                log::debug!("LIDT instruction (not implemented)");
                                Ok(RealModeStep::Continue)
                            }
                            _ => {
                                log::warn!("Unknown 0F 01 modrm reg: {}", reg);
                                Ok(RealModeStep::Continue)
                            }
                        }
                    }

                    // RDMSR (0F 32) - Read from Model-Specific Register
                    0x32 => {
                        let ecx = (self.regs.ecx & 0xFFFFFFFF) as u32;
                        log::debug!("RDMSR ECX={:#010X}", ecx);

                        // For now, only support EFER
                        if ecx == 0xC0000080 {
                            let efer = self.mode_trans.efer;
                            self.regs.eax = (efer & 0xFFFFFFFF) as u32;
                            self.regs.edx = ((efer >> 32) & 0xFFFFFFFF) as u32;
                            log::debug!("RDMSR EFER: EAX={:#010X}, EDX={:#010X}",
                                      self.regs.eax, self.regs.edx);
                        } else {
                            log::warn!("RDMSR from unsupported MSR: {:#010X}", ecx);
                            self.regs.eax = 0;
                            self.regs.edx = 0;
                        }
                        Ok(RealModeStep::Continue)
                    }

                    // WRMSR (0F 30) - Write to Model-Specific Register
                    0x30 => {
                        let ecx = (self.regs.ecx & 0xFFFFFFFF) as u32;
                        let eax = self.regs.eax as u32;
                        let edx = self.regs.edx as u32;
                        let value = ((edx as u64) << 32) | (eax as u64);

                        log::debug!("WRMSR ECX={:#010X}, value={:#018X}", ecx, value);
                        self.mode_trans.write_msr(ecx, value)?;

                        // Check if EFER.LME was set
                        if ecx == 0xC0000080 && (value & 0x100) != 0 {
                            log::info!("EFER.LME set via WRMSR - long mode enabled");
                            // Check if we need to switch modes
                            if let Some(step) = self.mode_trans.check_mode_switch(&mut self.regs, mmu)? {
                                return Ok(step);
                            }
                        }

                        Ok(RealModeStep::Continue)
                    }

                    // MOVSX r16, r/m8 (0F BE /r) - Move with Sign-Extension
                    0xBE => {
                        let modrm = self.fetch_byte(mmu)?;
                        let reg = ((modrm >> 3) & 7) as usize;
                        let _rm = (modrm & 7) as usize;

                        log::debug!("MOVSX r16, r/m8 (modrm={:02X}, reg={})", modrm, reg);
                        // TODO: Implement actual sign-extended move
                        Ok(RealModeStep::Continue)
                    }

                    // MOVSX r32, r/m8 (0F BE /r) with 0x66 prefix
                    // Handled by 0x66 prefix processing

                    // MOVZX r16, r/m8 (0F B6 /r) - Move with Zero-Extension
                    0xB6 => {
                        let modrm = self.fetch_byte(mmu)?;
                        let reg = ((modrm >> 3) & 7) as usize;
                        let _rm = (modrm & 7) as usize;

                        log::debug!("MOVZX r16, r/m8 (modrm={:02X}, reg={})", modrm, reg);
                        // TODO: Implement actual zero-extended move
                        Ok(RealModeStep::Continue)
                    }

                    // MOVZX r32, r/m16 (0F B7 /r)
                    0xB7 => {
                        let modrm = self.fetch_byte(mmu)?;
                        let reg = ((modrm >> 3) & 7) as usize;
                        let _rm = (modrm & 7) as usize;

                        log::debug!("MOVZX r32, r/m16 (modrm={:02X}, reg={})", modrm, reg);
                        // TODO: Implement actual zero-extended move
                        Ok(RealModeStep::Continue)
                    }

                    // Other two-byte opcodes can be added here
                    _ => {
                        log::warn!("Unknown two-byte opcode: 0F {:02X}", opcode2);
                        Ok(RealModeStep::Continue)
                    }
                }
            }

            // ===== Arithmetic =====

            // ADD/ADC/SUB/SBB/CMP/AND/OR/XOR reg, imm (80-83 with ModRM)
            0x80..=0x83 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let wide = (opcode == 0x81) || (opcode == 0x83);
                let sign_extend = opcode == 0x83;

                if wide {
                    let val = if sign_extend {
                        self.fetch_byte(mmu)? as i8 as i16 as u16
                    } else {
                        self.fetch_word(mmu)?
                    };
                    self.alu_op16(reg, opcode, val)?;
                } else {
                    let val = self.fetch_byte(mmu)?;
                    self.alu_op8(reg, opcode, val)?;
                }
                Ok(RealModeStep::Continue)
            }

            // INC r16 (40+reg)
            0x40..=0x47 => {
                let reg = (opcode - 0x40) as usize;
                let val = self.get_reg16(reg);
                let (result, _) = val.overflowing_add(1);
                self.set_reg16(reg, result);
                self.update_flags_zsp16(result);
                Ok(RealModeStep::Continue)
            }

            // DEC r16 (48+reg)
            0x48..=0x4F => {
                let reg = (opcode - 0x48) as usize;
                let val = self.get_reg16(reg);
                let (result, _) = val.overflowing_sub(1);
                self.set_reg16(reg, result);
                self.update_flags_zsp16(result);
                Ok(RealModeStep::Continue)
            }

            // ===== Logical =====

            // NOT r/m8 (F6) / NOT r/m16 (F7)
            0xF6 | 0xF7 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = (modrm >> 3) & 7;
                let rm = (modrm & 7) as usize;
                if reg == 2 { // NOT
                    if opcode == 0xF6 {
                        // 8-bit NOT - simplified
                        log::debug!("NOT r/m8");
                    } else {
                        // 16-bit NOT
                        let val = self.get_reg16(rm);
                        self.set_reg16(rm, !val);
                    }
                } else if reg == 3 { // NEG
                    if opcode == 0xF6 {
                        log::debug!("NEG r/m8");
                    } else {
                        let val = self.get_reg16(rm);
                        let (result, _) = (0i16).overflowing_sub(val as i16);
                        self.set_reg16(rm, result as u16);
                        self.update_flags_zsp16(result as u16);
                    }
                }
                Ok(RealModeStep::Continue)
            }

            // SHL/SAL r/m8, 1 (D0) / r/m16, 1 (D1)
            0xD0 | 0xD1 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = (modrm >> 3) & 7;
                let rm = (modrm & 7) as usize;
                if reg <= 3 { // SHL/SAL/SHR/SAR
                    if opcode == 0xD1 {
                        let mut val = self.get_reg16(rm);
                        match reg {
                            4 => { // SHL
                                val <<= 1;
                            }
                            5 => { // SHR
                                val >>= 1;
                            }
                            7 => { // SAR
                                val = (val as i16 >> 1) as u16;
                            }
                            _ => {}
                        }
                        self.set_reg16(rm, val);
                    }
                }
                Ok(RealModeStep::Continue)
            }

            // ===== Control Flow =====

            // JMP rel8
            0xEB => {
                let rel = self.fetch_byte(mmu)? as i8;
                self.regs.eip = self.regs.eip.wrapping_add(rel as i32 as u32);
                Ok(RealModeStep::Continue)
            }

            // JMP rel16/32
            0xE9 => {
                let rel = self.fetch_dword(mmu)? as i32;
                self.regs.eip = self.regs.eip.wrapping_add(rel as u32);
                Ok(RealModeStep::Continue)
            }

            // JMP far (segment:offset)
            0xEA => {
                let offset = self.fetch_word(mmu)?;
                let seg = self.fetch_word(mmu)?;
                self.regs.eip = offset as u32;
                self.regs.cs = seg;
                log::info!("FAR JMP: CS={:04X}, IP={:08X}", seg, offset);
                Ok(RealModeStep::Continue)
            }

            // Jcc rel8 (70-7F)
            0x70..=0x7F => {
                let rel = self.fetch_byte(mmu)? as i8;
                let cond_met = self.check_cond(opcode);
                // Use log::warn to ensure visibility in release builds
                if self.regs.cs == 0x1000 && self.regs.eip <= 0x50 {
                    log::warn!("J{:02X} at CS:IP={:04X}:{:08X} rel={:02X} (cond={}) - ZF={}, CF={}",
                              opcode, self.regs.cs, self.regs.eip - 2, rel, cond_met,
                              (self.regs.eflags & 0x0040) != 0,
                              (self.regs.eflags & 0x0001) != 0);
                }
                if cond_met {
                    self.regs.eip = self.regs.eip.wrapping_add(rel as i32 as u32);
                    if self.regs.cs == 0x1000 && self.regs.eip < 0x100 {
                        log::warn!("  Jump taken! New EIP={:08X}", self.regs.eip);
                    }
                }
                Ok(RealModeStep::Continue)
            }

            // JCXZ rel8 (E3)
            0xE3 => {
                let rel = self.fetch_byte(mmu)? as i8;
                if self.get_reg16(1) == 0 { // CX
                    self.regs.eip = self.regs.eip.wrapping_add(rel as i32 as u32);
                }
                Ok(RealModeStep::Continue)
            }

            // CALL rel16 (E8)
            0xE8 => {
                let rel = self.fetch_dword(mmu)? as i32;
                let ret_ip = self.regs.eip;
                self.push16(mmu, self.regs.cs)?;
                self.push16(mmu, ret_ip as u16)?;
                self.regs.eip = self.regs.eip.wrapping_add(rel as u32);
                Ok(RealModeStep::Continue)
            }

            // CALL far (9A)
            0x9A => {
                let offset = self.fetch_word(mmu)?;
                let seg = self.fetch_word(mmu)?;
                self.push16(mmu, self.regs.cs)?;
                self.push16(mmu, self.regs.eip as u16)?;
                self.regs.cs = seg;
                self.regs.eip = offset as u32;
                Ok(RealModeStep::Continue)
            }

            // RET (C3)
            0xC3 => {
                let ret_ip = self.pop16(mmu)?;
                self.regs.eip = ret_ip as u32;
                Ok(RealModeStep::Continue)
            }

            // RET imm16 (C2)
            0xC2 => {
                let disp = self.fetch_word(mmu)?;
                let ret_ip = self.pop16(mmu)?;
                self.regs.eip = ret_ip as u32;
                self.regs.esp = self.regs.esp.wrapping_add(disp as u32);
                Ok(RealModeStep::Continue)
            }

            // RET far (CB)
            0xCB => {
                let ret_ip = self.pop16(mmu)?;
                let ret_cs = self.pop16(mmu)?;
                self.regs.eip = ret_ip as u32;
                self.regs.cs = ret_cs;
                Ok(RealModeStep::Continue)
            }

            // LOOP/LOOPE/LOOPNE rel8 (E0, E1, E2)
            0xE0 | 0xE1 | 0xE2 => {
                let rel = self.fetch_byte(mmu)? as i8;
                let cx = self.get_reg16(1);
                let new_cx = cx.wrapping_sub(1);
                self.set_reg16(1, new_cx);

                let should_loop = match opcode {
                    0xE0 => new_cx != 0, // LOOPNZ
                    0xE1 => new_cx != 0 && self.get_zf(), // LOOPZ
                    0xE2 => new_cx != 0, // LOOP
                    _ => false,
                };

                if should_loop {
                    self.regs.eip = self.regs.eip.wrapping_add(rel as i32 as u32);
                }
                Ok(RealModeStep::Continue)
            }

            // INT imm8 (CD)
            0xCD => {
                let int_num = self.fetch_byte(mmu)?;
                log::info!("INT {:02X} called", int_num);

                // Try BIOS interrupt handler first
                let handled = self.bios.handle_int(int_num, &mut self.regs, mmu)?;

                if handled {
                    // Sync VGA display if needed
                    self.bios.sync_vga(mmu)?;
                } else {
                    log::warn!("INT {:02X} not implemented, continuing", int_num);
                }

                Ok(RealModeStep::Continue)
            }

            // IRET (CF)
            0xCF => {
                let ret_ip = self.pop16(mmu)?;
                let ret_cs = self.pop16(mmu)?;
                let flags = self.pop16(mmu)?;
                self.regs.eip = ret_ip as u32;
                self.regs.cs = ret_cs;
                self.regs.eflags = (self.regs.eflags & 0xFFFF0000) | (flags as u32);
                Ok(RealModeStep::Continue)
            }

            // ===== String Operations =====

            // MOVSB (A4) / MOVSW (A5)
            0xA4 | 0xA5 => {
                let is_word = opcode == 0xA5;
                let si = self.get_reg16(6); // SI
                let di = self.get_reg16(7); // DI

                if is_word {
                    let val = self.regs.read_mem_word(mmu, self.regs.ds, si)?;
                    self.regs.write_mem_word(mmu, self.regs.es, di, val)?;
                    self.set_reg16(6, si.wrapping_add(2));
                    self.set_reg16(7, di.wrapping_add(2));
                } else {
                    let val = self.regs.read_mem_byte(mmu, self.regs.ds, si)?;
                    self.regs.write_mem_byte(mmu, self.regs.es, di, val)?;
                    self.set_reg16(6, si.wrapping_add(1));
                    self.set_reg16(7, di.wrapping_add(1));
                }
                Ok(RealModeStep::Continue)
            }

            // STOSB (AA) / STOSW (AB)
            0xAA | 0xAB => {
                let is_word = opcode == 0xAB;
                let di = self.get_reg16(7); // DI

                if is_word {
                    let val = (self.regs.eax & 0xFFFF) as u16;
                    self.regs.write_mem_word(mmu, self.regs.es, di, val)?;
                    self.set_reg16(7, di.wrapping_add(2));
                } else {
                    let val = (self.regs.eax & 0xFF) as u8;
                    self.regs.write_mem_byte(mmu, self.regs.es, di, val)?;
                    self.set_reg16(7, di.wrapping_add(1));
                }
                Ok(RealModeStep::Continue)
            }

            // LODSB (AC) / LODSW (AD)
            0xAC | 0xAD => {
                let is_word = opcode == 0xAD;
                let si = self.get_reg16(6); // SI

                if is_word {
                    let val = self.regs.read_mem_word(mmu, self.regs.ds, si)?;
                    self.regs.eax = (self.regs.eax & 0xFFFF0000) | (val as u32);
                    self.set_reg16(6, si.wrapping_add(2));
                } else {
                    let val = self.regs.read_mem_byte(mmu, self.regs.ds, si)?;
                    self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (val as u32);
                    self.set_reg16(6, si.wrapping_add(1));
                }
                Ok(RealModeStep::Continue)
            }

            // CMPSB (A6) / CMPSW (A7)
            0xA6 | 0xA7 => {
                let is_word = opcode == 0xA7;
                let si = self.get_reg16(6);
                let di = self.get_reg16(7);

                if is_word {
                    let src = self.regs.read_mem_word(mmu, self.regs.ds, si)?;
                    let dst = self.regs.read_mem_word(mmu, self.regs.es, di)?;
                    let result = (dst as i16) - (src as i16);
                    self.update_flags_zsp16(result as u16);
                    self.set_reg16(6, si.wrapping_add(2));
                    self.set_reg16(7, di.wrapping_add(2));
                } else {
                    let src = self.regs.read_mem_byte(mmu, self.regs.ds, si)?;
                    let dst = self.regs.read_mem_byte(mmu, self.regs.es, di)?;
                    let result = (dst as i8) - (src as i8);
                    self.update_flags_zsp8(result as u8);
                    self.set_reg16(6, si.wrapping_add(1));
                    self.set_reg16(7, di.wrapping_add(1));
                }
                Ok(RealModeStep::Continue)
            }

            // SCASB (AE) / SCASW (AF)
            0xAE | 0xAF => {
                let is_word = opcode == 0xAF;
                let di = self.get_reg16(7);

                if is_word {
                    let val = self.regs.read_mem_word(mmu, self.regs.es, di)?;
                    let acc = (self.regs.eax & 0xFFFF) as u16;
                    let result = (acc as i16) - (val as i16);
                    self.update_flags_zsp16(result as u16);
                    self.set_reg16(7, di.wrapping_add(2));
                } else {
                    let val = self.regs.read_mem_byte(mmu, self.regs.es, di)?;
                    let acc = (self.regs.eax & 0xFF) as u8;
                    let result = (acc as i8) - (val as i8);
                    self.update_flags_zsp8(result as u8);
                    self.set_reg16(7, di.wrapping_add(1));
                }
                Ok(RealModeStep::Continue)
            }

            // REP prefix (F3) / REPE/REPZ (F3) / REPNE/REPNZ (F2)
            0xF2 | 0xF3 => {
                let next_opcode = self.fetch_byte(mmu)?;
                // For now, just execute the following instruction once
                // Full REP implementation would loop based on CX
                log::debug!("REP prefix, executing once (next opcode: {:02X})", next_opcode);
                // Push back the opcode so it gets processed
                self.regs.eip -= 1;
                Ok(RealModeStep::Continue)
            }

            // ===== Flag Control =====

            // CLC (F8), STC (F9), CLI (FA), STI (FB), CLD (FC), STD (FD)
            0xF8 => { self.regs.eflags &= !0x0001; Ok(RealModeStep::Continue) } // CLC
            0xF9 => { self.regs.eflags |= 0x0001; Ok(RealModeStep::Continue) } // STC
            0xFA => { self.regs.eflags &= !0x0200; Ok(RealModeStep::Continue) } // CLI
            0xFB => { self.regs.eflags |= 0x0200; Ok(RealModeStep::Continue) } // STI
            0xFC => { self.regs.eflags &= !0x0400; Ok(RealModeStep::Continue) } // CLD
            0xFD => { self.regs.eflags |= 0x0400; Ok(RealModeStep::Continue) } // STD

            // LAHF (9F) - Load AH from flags
            0x9F => {
                let flags_low = (self.regs.eflags & 0xFF) as u8;
                self.regs.eax = (self.regs.eax & 0xFFFF00FF) | ((flags_low as u32) << 8);
                Ok(RealModeStep::Continue)
            }

            // SAHF (9E) - Store AH into flags
            0x9E => {
                let ah = ((self.regs.eax >> 8) & 0xFF) as u8;
                self.regs.eflags = (self.regs.eflags & 0xFFFFFF00) | (ah as u32);
                Ok(RealModeStep::Continue)
            }

            // ===== Control Transfer =====

            // JCXZ, LOOP, etc. handled above

            // XCHG r/m, reg (86-87 with ModRM) or XCHG AX, reg (91-97)
            0x91..=0x97 => {
                let reg = (opcode - 0x91) as usize;
                let tmp = self.get_reg16(0); // AX
                let val = self.get_reg16(reg);
                self.set_reg16(0, val);
                self.set_reg16(reg, tmp);
                Ok(RealModeStep::Continue)
            }

            // XLAT (D7)
            0xD7 => {
                let ebx = self.regs.ebx as u16;
                let al = (self.regs.eax & 0xFF) as u8;
                let addr = ebx.wrapping_add(al as u16);
                let val = self.regs.read_mem_byte(mmu, self.regs.ds, addr)?;
                self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (val as u32);
                Ok(RealModeStep::Continue)
            }

            // LEA r16, m (8D with ModRM)
            0x8D => {
                let _modrm = self.fetch_byte(mmu)?;
                // Simplified: just skip for now
                log::debug!("LEA instruction (modrm fetched, not fully implemented)");
                Ok(RealModeStep::Continue)
            }

            // LDS/LES/LSS/LFS/LGS (C4-C5 with ModRM)
            0xC4 | 0xC5 => {
                let _modrm = self.fetch_byte(mmu)?;
                log::debug!("LDS/LES instruction (modrm fetched, not fully implemented)");
                Ok(RealModeStep::Continue)
            }

            // ===== I/O Operations =====

            // IN AL, imm8 (E4) / IN AX, imm8 (E5)
            0xE4 | 0xE5 => {
                let _port = self.fetch_byte(mmu)?;
                // For now, return 0 - real I/O will be implemented later
                if opcode == 0xE4 {
                    self.regs.eax = (self.regs.eax & 0xFFFFFF00) | 0x00;
                } else {
                    self.regs.eax = (self.regs.eax & 0xFFFF0000) | 0x0000;
                }
                Ok(RealModeStep::Continue)
            }

            // IN AL, DX (EC) / IN AX, DX (ED)
            0xEC | 0xED => {
                let _dx = self.get_reg16(2); // DX
                // For now, return 0
                if opcode == 0xEC {
                    self.regs.eax = (self.regs.eax & 0xFFFFFF00) | 0x00;
                } else {
                    self.regs.eax = (self.regs.eax & 0xFFFF0000) | 0x0000;
                }
                Ok(RealModeStep::Continue)
            }

            // OUT imm8, AL (E6) / OUT imm8, AX (E7)
            0xE6 | 0xE7 => {
                let _port = self.fetch_byte(mmu)?;
                // For now, just ignore
                log::debug!("OUT to port (ignored)");
                Ok(RealModeStep::Continue)
            }

            // OUT DX, AL (EE) / OUT DX, AX (EF)
            0xEE | 0xEF => {
                let _dx = self.get_reg16(2); // DX
                // For now, just ignore
                log::debug!("OUT to DX port (ignored)");
                Ok(RealModeStep::Continue)
            }

            // ===== Miscellaneous =====

            // CBW (98) - Convert byte to word
            0x98 => {
                let al = (self.regs.eax & 0xFF) as i8;
                self.regs.eax = (self.regs.eax & 0xFFFF0000) | (al as i16 as u16 as u32);
                Ok(RealModeStep::Continue)
            }

            // CWD (99) - Convert word to dword
            0x99 => {
                let ax = (self.regs.eax & 0xFFFF) as i16;
                self.regs.eax = ((ax as i32) as u32);
                Ok(RealModeStep::Continue)
            }

            // AAA (37) / AAS (3F) - ASCII adjust
            0x37 | 0x3F => {
                // Simplified: just continue
                log::debug!("ASCII adjust instruction");
                Ok(RealModeStep::Continue)
            }

            // AAM (D4) / AAD (D5)
            0xD4 => {
                let _imm = self.fetch_byte(mmu)?;
                // Simplified: just continue
                log::debug!("AAM instruction");
                Ok(RealModeStep::Continue)
            }
            0xD5 => {
                let _imm = self.fetch_byte(mmu)?;
                log::debug!("AAD instruction");
                Ok(RealModeStep::Continue)
            }

            // DAA (27) / DAS (2F) - Decimal adjust
            0x27 | 0x2F => {
                log::debug!("Decimal adjust instruction");
                Ok(RealModeStep::Continue)
            }

            // NOP extensions (90 is NOP, 9B is WAIT, etc.)
            0x9B => Ok(RealModeStep::Continue), // WAIT

            // LOCK prefix (F0)
            0xF0 => {
                log::debug!("LOCK prefix {:02X} at CS:IP={:04X}:{:08X} - ignoring",
                          opcode, self.regs.cs, self.regs.eip - 1);
                // LOCK prefix is only meaningful for multiprocessor systems
                // In single-processor emulation, we can ignore it
                Ok(RealModeStep::Continue)
            }

            // Segment override prefixes (2E, 36, 3E, 26, 64, 65)
            // In real mode, these prefixes don't change behavior much
            // We ignore them and continue to next instruction
            0x2E | 0x36 | 0x3E | 0x26 | 0x64 | 0x65 => {
                log::debug!("Segment override prefix {:02X} at CS:IP={:04X}:{:08X} - ignoring",
                          opcode, self.regs.cs, self.regs.eip - 1);
                // Don't adjust EIP - just skip the prefix and continue
                // The next fetch will get the actual opcode
                Ok(RealModeStep::Continue)
            }

            // Operand size prefix (66)
            0x66 => {
                log::debug!("Operand size prefix {:02X} at CS:IP={:04X}:{:08X} - ignoring",
                          opcode, self.regs.cs, self.regs.eip - 1);
                // Don't adjust EIP - skip the prefix
                Ok(RealModeStep::Continue)
            }

            // Address size prefix (67)
            0x67 => {
                log::debug!("Address size prefix {:02X} at CS:IP={:04X}:{:08X} - ignoring",
                          opcode, self.regs.cs, self.regs.eip - 1);
                // Don't adjust EIP - skip the prefix
                Ok(RealModeStep::Continue)
            }

            // HLT
            0xF4 => {
                log::info!("HLT encountered in real-mode");
                Ok(RealModeStep::Halt)
            }

            // Unknown opcode - try to skip
            _ => {
                log::warn!("Unknown real-mode opcode: {:02X} at CS:{:04X}, IP:{:08X}",
                          opcode, self.regs.cs, self.regs.eip - 1);
                // Try to continue anyway
                Ok(RealModeStep::Continue)
            }
        }
    }

    /// Get current registers
    pub fn regs(&self) -> &RealModeRegs {
        &self.regs
    }

    /// Get mutable registers
    pub fn regs_mut(&mut self) -> &mut RealModeRegs {
        &mut self.regs
    }

    /// Get mode transition manager
    pub fn mode_trans(&self) -> &ModeTransition {
        &self.mode_trans
    }

    /// Get mutable mode transition manager
    pub fn mode_trans_mut(&mut self) -> &mut ModeTransition {
        &mut self.mode_trans
    }

    /// Get BIOS handler
    pub fn bios(&self) -> &BiosInt {
        &self.bios
    }

    /// Get mutable BIOS handler
    pub fn bios_mut(&mut self) -> &mut BiosInt {
        &mut self.bios
    }
}

/// Result of a real-mode execution step
#[derive(Debug, Clone, PartialEq)]
pub enum RealModeStep {
    /// Continue execution
    Continue,
    /// Halted (HLT instruction)
    Halt,
    /// Switch to protected/long mode
    SwitchMode,
    /// Error occurred
    Error(VmError),
    /// Real-mode not active
    NotActive,
}

impl Default for RealModeEmulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seg_to_linear() {
        let mut regs = RealModeRegs::default();
        regs.cs = 0x07C0;
        assert_eq!(regs.seg_to_linear(0x07C0, 0x0000), 0x07C00);
        assert_eq!(regs.seg_to_linear(0x07C0, 0x0100), 0x07D00);
    }

    #[test]
    fn test_realmode_create() {
        let emu = RealModeEmulator::new();
        assert!(!emu.is_active());
        assert_eq!(emu.regs().cs, 0x07C0);
    }
}
