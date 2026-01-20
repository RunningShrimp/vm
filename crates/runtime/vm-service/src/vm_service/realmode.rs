//! # Real-Mode x86 Emulator
//!
//! Minimal 16-bit x86 real-mode emulation for booting bzImage kernels.
//! This handles the initial boot sequence before the kernel switches to protected/long mode.

use super::apic::{IoApic, LocalApic};
use super::bios::BiosInt;
use super::mode_trans::{ModeTransition, X86Mode};
use super::pic::Pic;
use super::pit::Pit;
use vm_core::{AccessType, GuestAddr, MemoryError, VmError, VmResult, MMU};

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
            log::warn!(
                "read_mem_byte: seg={:04x}, offset={:04x} -> linear={:#08x}, addr={:#010x}",
                seg,
                offset,
                linear,
                addr.0
            );
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
    pub fn write_mem_byte(
        &self,
        mmu: &mut dyn MMU,
        seg: u16,
        offset: u16,
        val: u8,
    ) -> VmResult<()> {
        let linear = self.seg_to_linear(seg, offset);
        let addr = GuestAddr(linear as u64);

        // CRITICAL: Log framebuffer writes for debugging
        // VESA LFB is typically at 0xE0000000
        if linear >= 0xE0000000 && linear < 0xF0000000 {
            log::info!("FRAMEBUFFER WRITE: addr={:#010X}, val={:02X}", linear, val);
        }

        mmu.write(addr, val as u64, 1)
    }

    /// Write a word to memory using segment:offset addressing
    pub fn write_mem_word(
        &self,
        mmu: &mut dyn MMU,
        seg: u16,
        offset: u16,
        val: u16,
    ) -> VmResult<()> {
        let linear = self.seg_to_linear(seg, offset);
        let addr = GuestAddr(linear as u64);

        // CRITICAL: Log framebuffer writes for debugging
        if linear >= 0xE0000000 && linear < 0xF0000000 {
            log::info!(
                "FRAMEBUFFER WRITE (word): addr={:#010X}, val={:04X}",
                linear,
                val
            );
        }

        mmu.write(addr, val as u64, 2)
    }

    /// Write a dword to memory using segment:offset addressing
    pub fn write_mem_dword(
        &self,
        mmu: &mut dyn MMU,
        seg: u16,
        offset: u16,
        val: u32,
    ) -> VmResult<()> {
        let linear = self.seg_to_linear(seg, offset);
        let addr = GuestAddr(linear as u64);
        mmu.write(addr, val as u64, 4)
    }
}

/// Minimal real-mode emulator
/// VGA state tracker for simulating VGA hardware
#[derive(Debug, Default)]
struct VgaState {
    /// Number of OUTSW operations performed
    outsw_count: usize,
    /// Threshold after which VGA is considered "initialized"
    initialization_threshold: usize,
    /// Whether VGA is initialized
    is_initialized: bool,
    /// Last written VGA port
    last_port: u16,
}

impl VgaState {
    fn new() -> Self {
        Self {
            outsw_count: 0,
            // Consider VGA initialized after 10,000 OUTSW operations
            // This is a heuristic - real VGA initialization may need fewer or more
            initialization_threshold: 10_000,
            is_initialized: false,
            last_port: 0,
        }
    }

    /// Record an OUTSW operation and check if VGA should be initialized
    fn record_outsw(&mut self, port: u16) -> bool {
        self.outsw_count += 1;
        self.last_port = port;

        // Check if we've reached the threshold
        if !self.is_initialized && self.outsw_count >= self.initialization_threshold {
            self.is_initialized = true;
            log::info!(
                "VGA initialization completed after {} OUTSW operations",
                self.outsw_count
            );
            return true; // State changed
        }

        false
    }
}

pub struct RealModeEmulator {
    /// Registers
    regs: RealModeRegs,
    /// Whether emulation is active
    active: bool,
    /// BIOS interrupt handler
    bios: BiosInt,
    /// Mode transition manager
    mode_trans: ModeTransition,
    /// Programmable Interrupt Controller (legacy 8259A)
    pic: Pic,
    /// Programmable Interval Timer (8253/8254)
    pit: Pit,
    /// Local APIC (Advanced Programmable Interrupt Controller)
    local_apic: LocalApic,
    /// I/O APIC
    io_apic: IoApic,
    /// HLT flag (waiting for interrupt)
    hlt_waiting: bool,
    /// Virtual time counter (in nanoseconds)
    virtual_time_ns: u64,
    /// Instructions executed counter
    instruction_count: u64,
    /// VGA state tracker
    vga: VgaState,
}

impl RealModeEmulator {
    /// Create new real-mode emulator
    pub fn new() -> Self {
        let mut regs = RealModeRegs::default();

        // Initialize to typical BIOS boot state
        regs.cs = 0x07C0; // BIOS code segment
        regs.ds = 0x07C0; // BIOS data segment
        regs.es = 0x07C0;
        regs.ss = 0x07C0;
        regs.esp = 0x7C00; // Stack below code
        regs.eip = 0;
        regs.eflags = 0x202; // Interrupts enabled, reserved bit set

        // Initialize PIT Channel 0 with typical frequency (100 Hz)
        let mut pit = Pit::new();
        pit.set_channel0_reload(11931); // ~100 Hz (11931818 / 100)

        // Initialize PIC and unmask IRQ0 (timer)
        let mut pic = Pic::new();
        pic.mask_irq(0, false); // Unmask IRQ0 (timer)

        // Initialize Local APIC and I/O APIC
        let mut local_apic = LocalApic::new();
        local_apic.enable(); // Enable Local APIC

        let mut io_apic = IoApic::new();
        io_apic.enable();
        io_apic.setup_default_irqs();

        log::info!("APIC: Local APIC and I/O APIC initialized");

        Self {
            regs,
            active: false,
            bios: BiosInt::new(),
            mode_trans: ModeTransition::new(),
            pic,
            pit,
            local_apic,
            io_apic,
            hlt_waiting: false,
            virtual_time_ns: 0,
            instruction_count: 0,
            vga: VgaState::new(),
        }
    }

    /// Check if emulator is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate real-mode emulation
    pub fn activate(&mut self) {
        self.active = true;
        log::info!(
            "Real-mode emulation activated: CS={:04X}, IP={:08X}",
            self.regs.cs,
            self.regs.eip
        );
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

    /// Force a mode transition (used to bypass stuck initialization loops)
    pub fn force_mode_transition(&mut self, mmu: &mut dyn MMU) -> VmResult<RealModeStep> {
        log::info!(
            "Forcing mode transition (current mode: {:?})",
            self.mode_trans.current_mode()
        );

        match self.mode_trans.current_mode() {
            X86Mode::Real => {
                // Force switch to protected mode
                log::info!("Forcing switch from Real to Protected mode");
                let step = self
                    .mode_trans
                    .switch_to_protected_mode(&mut self.regs, mmu)?;
                Ok(step)
            }
            X86Mode::Protected => {
                // Force switch to long mode
                log::info!("Forcing switch from Protected to Long mode");
                let step = self.mode_trans.switch_to_long_mode(&mut self.regs, mmu)?;
                Ok(step)
            }
            X86Mode::Long => {
                log::warn!("Already in Long mode, no transition needed");
                Ok(RealModeStep::Continue)
            }
        }
    }

    /// Force transition to Long Mode (64-bit) specifically
    pub fn force_long_mode_transition(&mut self, mmu: &mut dyn MMU) -> VmResult<RealModeStep> {
        log::info!(
            "Forcing Long Mode transition (current mode: {:?})",
            self.mode_trans.current_mode()
        );

        match self.mode_trans.current_mode() {
            X86Mode::Real => {
                log::warn!("Cannot transition directly from Real to Long mode");
                log::info!("Transitioning through Protected mode first");
                let step = self
                    .mode_trans
                    .switch_to_protected_mode(&mut self.regs, mmu)?;
                Ok(step)
            }
            X86Mode::Protected => {
                log::info!("Forcing transition from Protected to Long mode");
                let step = self.mode_trans.switch_to_long_mode(&mut self.regs, mmu)?;
                Ok(step)
            }
            X86Mode::Long => {
                log::info!("Already in Long mode");
                Ok(RealModeStep::Continue)
            }
        }
    }

    /// Fetch next instruction byte
    pub fn fetch_byte(&mut self, mmu: &mut dyn MMU) -> VmResult<u8> {
        let byte = self
            .regs
            .read_mem_byte(mmu, self.regs.cs, self.regs.eip as u16)?;
        self.regs.eip += 1;
        Ok(byte)
    }

    /// Fetch next instruction word
    pub fn fetch_word(&mut self, mmu: &mut dyn MMU) -> VmResult<u16> {
        let word = self
            .regs
            .read_mem_word(mmu, self.regs.cs, self.regs.eip as u16)?;
        self.regs.eip += 2;
        Ok(word)
    }

    /// Fetch next instruction dword
    pub fn fetch_dword(&mut self, mmu: &mut dyn MMU) -> VmResult<u32> {
        let dword = self
            .regs
            .read_mem_dword(mmu, self.regs.cs, self.regs.eip as u16)?;
        self.regs.eip += 4;
        Ok(dword)
    }

    /// Fetch next instruction byte as signed
    pub fn fetch_byte_signed(&mut self, mmu: &mut dyn MMU) -> VmResult<i8> {
        let byte = self.fetch_byte(mmu)?;
        Ok(byte as i8)
    }

    /// Fetch next instruction word as signed
    pub fn fetch_word_signed(&mut self, mmu: &mut dyn MMU) -> VmResult<i16> {
        let word = self.fetch_word(mmu)?;
        Ok(word as i16)
    }

    /// Port I/O: Read byte from port
    fn port_read_byte(&mut self, _mmu: &mut dyn MMU, _port: u16) -> VmResult<u8> {
        // For now, return 0 - real I/O will be implemented later
        Ok(0)
    }

    /// Port I/O: Read word from port
    fn port_read_word(&mut self, _mmu: &mut dyn MMU, _port: u16) -> VmResult<u16> {
        // For now, return 0 - real I/O will be implemented later
        Ok(0)
    }

    /// Port I/O: Write byte to port
    fn port_write_byte(&mut self, _mmu: &mut dyn MMU, _port: u16, _val: u8) -> VmResult<()> {
        // For now, just ignore - real I/O will be implemented later
        log::debug!("OUT to port (ignored)");
        Ok(())
    }

    /// Port I/O: Write word to port
    fn port_write_word(&mut self, _mmu: &mut dyn MMU, _port: u16, _val: u16) -> VmResult<()> {
        // For now, just ignore - real I/O will be implemented later
        log::debug!("OUT to port (ignored)");
        Ok(())
    }

    // ===== Helper Methods for Instruction Execution =====

    /// Get 8-bit register value
    fn get_reg8(&self, reg: usize) -> u8 {
        match reg {
            0 => (self.regs.eax & 0xFF) as u8,        // AL
            1 => ((self.regs.eax >> 8) & 0xFF) as u8, // AH (simplified)
            2 => (self.regs.ecx & 0xFF) as u8,        // CL
            3 => ((self.regs.ecx >> 8) & 0xFF) as u8, // CH
            4 => (self.regs.edx & 0xFF) as u8,        // DL
            5 => ((self.regs.edx >> 8) & 0xFF) as u8, // DH
            6 => (self.regs.ebx & 0xFF) as u8,        // BL
            7 => ((self.regs.ebx >> 8) & 0xFF) as u8, // BH
            _ => 0,
        }
    }

    /// Set 8-bit register value
    fn set_reg8(&mut self, reg: usize, val: u8) {
        match reg {
            0 => self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (val as u32), // AL
            1 => self.regs.eax = (self.regs.eax & 0xFFFF00FF) | ((val as u32) << 8), // AH
            2 => self.regs.ecx = (self.regs.ecx & 0xFFFFFF00) | (val as u32), // CL
            3 => self.regs.ecx = (self.regs.ecx & 0xFFFF00FF) | ((val as u32) << 8), // CH
            4 => self.regs.edx = (self.regs.edx & 0xFFFFFF00) | (val as u32), // DL
            5 => self.regs.edx = (self.regs.edx & 0xFFFF00FF) | ((val as u32) << 8), // DH
            6 => self.regs.ebx = (self.regs.ebx & 0xFFFFFF00) | (val as u32), // BL
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
        self.regs
            .write_mem_word(mmu, self.regs.ss, self.regs.esp as u16, val)
    }

    /// Pop 16-bit value from stack
    fn pop16(&mut self, mmu: &mut dyn MMU) -> VmResult<u16> {
        let val = self
            .regs
            .read_mem_word(mmu, self.regs.ss, self.regs.esp as u16)?;
        self.regs.esp = self.regs.esp.wrapping_add(2);
        Ok(val)
    }

    /// Calculate effective address for ModR/M byte (16-bit addressing)
    fn calc_effective_address(
        &mut self,
        mmu: &mut dyn MMU,
        mod_val: u8,
        rm: usize,
    ) -> VmResult<u16> {
        Ok(match (mod_val, rm) {
            // Mod 00: No displacement
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

            // Mod 01: 8-bit displacement
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

            // Mod 10: 16-bit displacement
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
                log::warn!("Invalid addressing mode: mod={}, rm={}", mod_val, rm);
                return Err(VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Invalid addressing mode: mod={}, rm={}", mod_val, rm),
                    module: "realmode".to_string(),
                }));
            }
        })
    }

    /// Perform ALU operation on 8-bit value
    fn alu_op8(&mut self, reg: usize, opcode: u8, val: u8) -> VmResult<()> {
        let dst = self.get_reg8(reg);
        let result: u8 = match opcode {
            0x80 => match reg {
                // ADD/OR/ADC/SBB/AND/SUB/CMP/XOR
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
            0x81 => match reg {
                // ADD/OR/ADC/SBB/AND/SUB/CMP/XOR
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
            0x70 => of,                // JO
            0x71 => !of,               // JNO
            0x72 => cf,                // JB/JC/JNAE
            0x73 => !cf,               // JNB/JAE/JNC
            0x74 => zf,                // JZ/JE
            0x75 => !zf,               // JNZ/JNE
            0x76 => cf || zf,          // JBE/JNA
            0x77 => !cf && !zf,        // JA/JNBE
            0x78 => sf,                // JS
            0x79 => !sf,               // JNS
            0x7A => pf,                // JP/JPE
            0x7B => !pf,               // JNP/JPO
            0x7C => sf != of,          // JL/JNGE
            0x7D => sf == of,          // JGE/JNL
            0x7E => zf || (sf != of),  // JLE/JNG
            0x7F => !zf && (sf == of), // JG/JNLE
            _ => false,
        }
    }

    /// Get zero flag state
    fn get_zf(&self) -> bool {
        (self.regs.eflags & 0x0040) != 0
    }

    /// Read IDT entry from memory (for protected/long mode)
    fn read_idt_entry(&mut self, int_num: u8, mmu: &mut dyn MMU) -> VmResult<(u32, u16)> {
        use super::mode_trans::X86Mode;

        let current_mode = self.mode_trans.current_mode();

        // In real mode, always use IVT
        if current_mode == X86Mode::Real {
            let vec_addr = (int_num as u16) * 4;
            let offset = self.regs.read_mem_word(mmu, 0x0000, vec_addr)? as u32;
            let segment = self.regs.read_mem_word(mmu, 0x0000, vec_addr + 2)?;
            return Ok((offset, segment));
        }

        // In protected/long mode, check if IDT is loaded
        if !self.mode_trans.idt_loaded() {
            log::warn!(
                "INT {:02X} in protected mode but IDT not loaded, falling back to IVT",
                int_num
            );
            let vec_addr = (int_num as u16) * 4;
            let offset = self.regs.read_mem_word(mmu, 0x0000, vec_addr)? as u32;
            let segment = self.regs.read_mem_word(mmu, 0x0000, vec_addr + 2)?;
            return Ok((offset, segment));
        }

        // Read from IDT
        let idtr = self.mode_trans.idtr();
        let entry_addr = idtr.base + (int_num as u32 * 8);

        log::debug!(
            "Reading IDT entry {} from address {:#010X}",
            int_num,
            entry_addr
        );

        // For protected mode, IDT entry is 8 bytes:
        // Offset low (16), Selector (16), Reserved (8), Type (8), Offset high (16)
        // We need to read these as bytes and reconstruct

        // Read the 8-byte IDT entry
        let byte0 = self.regs.read_mem_byte(mmu, 0x0000, entry_addr as u16)?;
        let byte1 = self
            .regs
            .read_mem_byte(mmu, 0x0000, (entry_addr + 1) as u16)?;
        let byte2 = self
            .regs
            .read_mem_byte(mmu, 0x0000, (entry_addr + 2) as u16)?;
        let byte3 = self
            .regs
            .read_mem_byte(mmu, 0x0000, (entry_addr + 3) as u16)?;
        let byte4 = self
            .regs
            .read_mem_byte(mmu, 0x0000, (entry_addr + 4) as u16)?;
        let byte5 = self
            .regs
            .read_mem_byte(mmu, 0x0000, (entry_addr + 5) as u16)?;
        let byte6 = self
            .regs
            .read_mem_byte(mmu, 0x0000, (entry_addr + 6) as u16)?;
        let byte7 = self
            .regs
            .read_mem_byte(mmu, 0x0000, (entry_addr + 7) as u16)?;

        // Reconstruct the IDT entry
        let offset_low = (byte1 as u16) << 8 | (byte0 as u16);
        let selector = (byte3 as u16) << 8 | (byte2 as u16);
        let type_attr = byte6;
        let offset_high = (byte7 as u16) << 8 | (byte6 as u16);

        // Combine offset parts
        let offset = (offset_high as u32) << 16 | (offset_low as u32);

        log::info!(
            "IDT entry {}: offset={:#010X}, selector={:#06X}, type={:#04X}",
            int_num,
            offset,
            selector,
            type_attr
        );

        Ok((offset, selector))
    }

    /// Handle interrupt (hardware or software)
    fn handle_interrupt(&mut self, int_num: u8, mmu: &mut dyn MMU) -> VmResult<RealModeStep> {
        use super::mode_trans::X86Mode;

        let current_mode = self.mode_trans.current_mode();
        log::info!(
            "Handling interrupt INT {:02X} in {:?} mode",
            int_num,
            current_mode
        );

        // Read interrupt vector address (from IVT or IDT)
        let (offset, selector) = self.read_idt_entry(int_num, mmu)?;

        // Push flags, CS, IP onto stack
        let flags = (self.regs.eflags & 0xFFFF) as u16;
        self.push16(mmu, flags)?;
        self.push16(mmu, self.regs.cs)?;
        self.push16(mmu, (self.regs.eip & 0xFFFF) as u16)?;

        // Clear interrupt flag
        self.regs.eflags &= !0x0200; // Clear IF

        // Jump to interrupt handler
        self.regs.cs = selector;
        self.regs.eip = offset;

        log::info!(
            "INT {:02X}: Jump to {:04X}:{:#010X}",
            int_num,
            selector,
            offset
        );

        Ok(RealModeStep::Continue)
    }

    /// Calculate effective address for memory operands
    ///
    /// This handles x86 addressing modes:
    /// - mod=0: [reg] or [reg+reg] (no displacement, except [BP+disp16])
    /// - mod=1: [reg+reg+disp8] (8-bit displacement)
    /// - mod=2: [reg+reg+disp16] (16-bit displacement)
    /// - mod=3: register (not used here)
    ///
    /// Returns the calculated effective address
    fn calculate_effective_address(
        &mut self,
        mmu: &mut dyn MMU,
        mod_val: usize,
        rm: usize,
    ) -> VmResult<u16> {
        match mod_val {
            0 => {
                // mod=0: [reg+reg] or disp16
                match rm {
                    0 => {
                        // [BX+SI]
                        let bx = (self.regs.ebx & 0xFFFF) as u16;
                        let si = (self.regs.esi & 0xFFFF) as u16;
                        Ok(bx.wrapping_add(si))
                    }
                    1 => {
                        // [BX+DI]
                        let bx = (self.regs.ebx & 0xFFFF) as u16;
                        let di = (self.regs.edi & 0xFFFF) as u16;
                        Ok(bx.wrapping_add(di))
                    }
                    2 => {
                        // [BP+SI]
                        let bp = (self.regs.ebp & 0xFFFF) as u16;
                        let si = (self.regs.esi & 0xFFFF) as u16;
                        Ok(bp.wrapping_add(si))
                    }
                    3 => {
                        // [BP+DI]
                        let bp = (self.regs.ebp & 0xFFFF) as u16;
                        let di = (self.regs.edi & 0xFFFF) as u16;
                        Ok(bp.wrapping_add(di))
                    }
                    4 => {
                        // [SI]
                        Ok((self.regs.esi & 0xFFFF) as u16)
                    }
                    5 => {
                        // [DI]
                        Ok((self.regs.edi & 0xFFFF) as u16)
                    }
                    6 => {
                        // [disp16] - fetch 16-bit displacement
                        Ok(self.fetch_word(mmu)?)
                    }
                    7 => {
                        // [BX]
                        Ok((self.regs.ebx & 0xFFFF) as u16)
                    }
                    _ => unreachable!(),
                }
            }
            1 => {
                // mod=1: [reg+reg+disp8]
                let disp8 = self.fetch_byte(mmu)? as i8;
                let base = match rm {
                    0 => {
                        // [BX+SI+disp8]
                        let bx = (self.regs.ebx & 0xFFFF) as u16;
                        let si = (self.regs.esi & 0xFFFF) as u16;
                        bx.wrapping_add(si)
                    }
                    1 => {
                        // [BX+DI+disp8]
                        let bx = (self.regs.ebx & 0xFFFF) as u16;
                        let di = (self.regs.edi & 0xFFFF) as u16;
                        bx.wrapping_add(di)
                    }
                    2 => {
                        // [BP+SI+disp8]
                        let bp = (self.regs.ebp & 0xFFFF) as u16;
                        let si = (self.regs.esi & 0xFFFF) as u16;
                        bp.wrapping_add(si)
                    }
                    3 => {
                        // [BP+DI+disp8]
                        let bp = (self.regs.ebp & 0xFFFF) as u16;
                        let di = (self.regs.edi & 0xFFFF) as u16;
                        bp.wrapping_add(di)
                    }
                    4 => {
                        // [SI+disp8]
                        (self.regs.esi & 0xFFFF) as u16
                    }
                    5 => {
                        // [DI+disp8]
                        (self.regs.edi & 0xFFFF) as u16
                    }
                    6 => {
                        // [BP+disp8]
                        (self.regs.ebp & 0xFFFF) as u16
                    }
                    7 => {
                        // [BX+disp8]
                        (self.regs.ebx & 0xFFFF) as u16
                    }
                    _ => unreachable!(),
                };
                Ok(base.wrapping_add(disp8 as u16))
            }
            2 => {
                // mod=2: [reg+reg+disp16]
                let disp16 = self.fetch_word(mmu)? as i16;
                let base = match rm {
                    0 => {
                        // [BX+SI+disp16]
                        let bx = (self.regs.ebx & 0xFFFF) as u16;
                        let si = (self.regs.esi & 0xFFFF) as u16;
                        bx.wrapping_add(si)
                    }
                    1 => {
                        // [BX+DI+disp16]
                        let bx = (self.regs.ebx & 0xFFFF) as u16;
                        let di = (self.regs.edi & 0xFFFF) as u16;
                        bx.wrapping_add(di)
                    }
                    2 => {
                        // [BP+SI+disp16]
                        let bp = (self.regs.ebp & 0xFFFF) as u16;
                        let si = (self.regs.esi & 0xFFFF) as u16;
                        bp.wrapping_add(si)
                    }
                    3 => {
                        // [BP+DI+disp16]
                        let bp = (self.regs.ebp & 0xFFFF) as u16;
                        let di = (self.regs.edi & 0xFFFF) as u16;
                        bp.wrapping_add(di)
                    }
                    4 => {
                        // [SI+disp16]
                        (self.regs.esi & 0xFFFF) as u16
                    }
                    5 => {
                        // [DI+disp16]
                        (self.regs.edi & 0xFFFF) as u16
                    }
                    6 => {
                        // [BP+disp16]
                        (self.regs.ebp & 0xFFFF) as u16
                    }
                    7 => {
                        // [BX+disp16]
                        (self.regs.ebx & 0xFFFF) as u16
                    }
                    _ => unreachable!(),
                };
                Ok(base.wrapping_add(disp16 as u16))
            }
            _ => {
                // mod=3 is register mode, should not be called
                log::error!("Invalid addressing mode mod={} for memory operand", mod_val);
                Err(VmError::Memory(MemoryError::AccessViolation {
                    addr: GuestAddr(0),
                    msg: "Invalid addressing mode for memory operand".to_string(),
                    access_type: None,
                }))
            }
        }
    }

    /// Execute instruction at current CS:IP
    pub fn execute(&mut self, mmu: &mut dyn MMU) -> VmResult<RealModeStep> {
        if !self.active {
            return Ok(RealModeStep::NotActive);
        }

        // Advance virtual time by ~4 nanoseconds (1 CPU cycle at ~250MHz)
        // This allows PIT to generate timer interrupts based on instruction count
        self.virtual_time_ns += 4;
        self.instruction_count += 1;

        // Update PIT timers with current virtual time and generate IRQ0 periodically
        self.pit.update(&mut self.pic, self.virtual_time_ns);

        // Update APIC timers with current virtual time (for Local APIC timer interrupts)
        self.local_apic.update_timer(self.virtual_time_ns);

        // CRITICAL: Check for and inject pending hardware interrupts BEFORE fetching next instruction
        // This allows timer interrupts to be delivered during normal execution, not just during HLT
        if self.has_pending_interrupt() {
            if let Some(irq) = self.get_pending_interrupt() {
                log::info!("Injecting hardware interrupt IRQ {} during execution", irq);

                // Convert IRQ to interrupt vector (IRQ 0-7 -> INT 8h-Fh, IRQ 8-15 -> INT 70h-77h)
                let int_num = if irq < 8 {
                    0x08 + irq
                } else {
                    0x70 + (irq - 8)
                };

                // Handle the interrupt (this will push flags/CS/IP and jump to handler)
                return self.handle_interrupt(int_num as u8, mmu);
            }
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

        // Fetch opcode and check for segment prefixes
        let opcode = self.fetch_byte(mmu)?;

        // Handle segment prefix overrides (0x26, 0x2E, 0x36, 0x3E, 0x64, 0x65)
        let segment_override: Option<u16> = match opcode {
            0x26 => Some(self.regs.es), // ES prefix
            0x2E => Some(self.regs.cs), // CS prefix
            0x36 => Some(self.regs.ss), // SS prefix
            0x3E => Some(self.regs.ds), // DS prefix
            0x64 => Some(self.regs.fs), // FS prefix
            0x65 => Some(self.regs.gs), // GS prefix
            _ => None,
        };

        // If we have a segment prefix, fetch the actual opcode and use override
        let (opcode, seg_override) = if segment_override.is_some() {
            let actual_opcode = self.fetch_byte(mmu)?;
            log::debug!(
                "Segment prefix detected: override={:04X}, actual opcode={:02X}",
                segment_override.unwrap(),
                actual_opcode
            );
            (actual_opcode, segment_override)
        } else {
            (opcode, None)
        };

        // Log when at critical address 0x44 (before fetch) or 0x45 (after fetch)
        if self.regs.cs == 0x1000 && (self.regs.eip == 0x45 || self.regs.eip == 0x44) {
            log::warn!(
                "Fetched opcode {:02X} when CS:IP={:04X}:{:08X}",
                opcode,
                self.regs.cs,
                self.regs.eip
            );

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
            // ===== Logical Instructions =====
            // MUST come before ADD since these use low opcodes (0x08-0x0B)

            // OR r/m8, r8 (08 /r) - Logical OR
            0x08 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg8(reg);

                if mod_val == 3 {
                    // Register to register
                    let dst = self.get_reg8(rm);
                    let result = dst | src;
                    self.set_reg8(rm, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "OR r8[{}], r8[{}] ({:02X} | {:02X} = {:02X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory operation
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst | src;
                    self.regs
                        .write_mem_byte(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "OR [mem], r8 addr={:04X} ({:02X} | {:02X} = {:02X}) ZF={}",
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // OR r/m16, r16 (09 /r) - Logical OR
            0x09 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg16(reg);

                if mod_val == 3 {
                    // Register to register
                    let dst = self.get_reg16(rm);
                    let result = dst | src;
                    self.set_reg16(rm, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "OR r16[{}], r16[{}] ({:04X} | {:04X} = {:04X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory operation
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let dst = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let result = dst | src;
                    self.regs
                        .write_mem_word(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "OR [mem], r16 addr={:04X} ({:04X} | {:04X} = {:04X}) ZF={}",
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // OR r8, r/m8 (0A /r) - Logical OR
            0x0A => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                if mod_val == 3 {
                    // Register to register
                    let src = self.get_reg8(rm);
                    let dst = self.get_reg8(reg);
                    let result = dst | src;
                    self.set_reg8(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "OR r8, r/m8 ({:02X} | {:02X} = {:02X}) ZF={}",
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory to register
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let src = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let dst = self.get_reg8(reg);
                    let result = dst | src;
                    self.set_reg8(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "OR r8, [mem] addr={:04X} ({:02X} | {:02X} = {:02X}) ZF={}",
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // OR r16, r/m16 (0B /r) - Logical OR
            0x0B => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                if mod_val == 3 {
                    // Register to register
                    let src = self.get_reg16(rm);
                    let dst = self.get_reg16(reg);
                    let result = dst | src;
                    self.set_reg16(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "OR r16, r/m16 ({:04X} | {:04X} = {:04X}) ZF={}",
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory to register
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let src = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let dst = self.get_reg16(reg);
                    let result = dst | src;
                    self.set_reg16(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "OR r16, [mem] addr={:04X} ({:04X} | {:04X} = {:04X}) ZF={}",
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // OR AL, imm8 (0C ib) - Logical OR immediate
            0x0C => {
                let imm = self.fetch_byte(mmu)?;
                let al = (self.regs.eax & 0xFF) as u8;
                let result = al | imm;
                self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (result as u32);

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::warn!(
                    "OR AL, imm8 (imm={:02X}, al={:02X}, result={:02X}) ZF={}",
                    imm,
                    al,
                    result,
                    (self.regs.eflags & 0x40) != 0
                );
                Ok(RealModeStep::Continue)
            }

            // OR AX, imm16 (0D iw) - Logical OR immediate
            0x0D => {
                let imm = self.fetch_word(mmu)?;
                let ax = (self.regs.eax & 0xFFFF) as u16;
                let result = ax | imm;
                self.regs.eax = (self.regs.eax & 0xFFFF0000) | (result as u32);

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::warn!(
                    "OR AX, imm16 (imm={:04X}, ax={:04X}, result={:04X}) ZF={}",
                    imm,
                    ax,
                    result,
                    (self.regs.eflags & 0x40) != 0
                );
                Ok(RealModeStep::Continue)
            }

            // TEST AL, imm8 (A8 ib) - Test immediate against AL
            0xA8 => {
                let imm = self.fetch_byte(mmu)?;
                let al = (self.regs.eax & 0xFF) as u8;
                let result = al & imm;

                // Update flags based on AND result (but don't store result)
                if result == 0 {
                    self.regs.eflags |= 0x40; // ZF
                } else {
                    self.regs.eflags &= !0x40;
                }

                // Clear OF and CF (TEST always clears these)
                self.regs.eflags &= !0x800; // OF
                self.regs.eflags &= !0x1; // CF

                log::warn!(
                    "TEST AL, imm8 (imm={:02X}, al={:02X}, result={:02X}) ZF={}",
                    imm,
                    al,
                    result,
                    (self.regs.eflags & 0x40) != 0
                );
                Ok(RealModeStep::Continue)
            }

            // ICEBP/INT1 (F1) - Ice Breakpoint / INT1
            0xF1 => {
                // ICEBP is a debug breakpoint instruction
                // In real mode, it typically triggers a debug exception
                // For our purposes, we'll treat it as a NOP since we don't have a debugger
                // Only log occasionally to reduce noise
                static ICEBP_COUNT: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let count = ICEBP_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if count % 10000 == 0 {
                    log::debug!(
                        "ICEBP/INT1 encountered (treated as NOP for emulation, count={})",
                        count
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // ADC r16, r/m16 (13 /r) - ADD with Carry
            0x13 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;
                let cf = (self.regs.eflags & 0x01) != 0; // Carry flag
                let carry = if cf { 1 } else { 0 };

                if mod_val == 3 {
                    // Register to register
                    let src = self.get_reg16(rm);
                    let dst = self.get_reg16(reg);
                    let result = dst.wrapping_add(src).wrapping_add(carry);
                    self.set_reg16(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "ADC r16[{}], r16[{}] ({:04X} + {:04X} + {} = {:04X}) ZF={}",
                        reg,
                        rm,
                        dst,
                        src,
                        carry,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory to register
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let src = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let dst = self.get_reg16(reg);
                    let result = dst.wrapping_add(src).wrapping_add(carry);
                    self.set_reg16(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "ADC r16[{}], [mem] addr={:04X} ({:04X} + {:04X} + {} = {:04X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        carry,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // ADC r8, r/m8 (12 /r) - ADD with Carry (reverse operands)
            0x12 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;
                let cf = (self.regs.eflags & 0x01) != 0; // Carry flag
                let carry = if cf { 1u8 } else { 0u8 };

                if mod_val == 3 {
                    // Register to register (reverse)
                    let src = self.get_reg8(rm);
                    let dst = self.get_reg8(reg);
                    let result = dst.wrapping_add(src).wrapping_add(carry);
                    self.set_reg8(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::trace!(
                        "ADC r8[{}], r8[{}] ({:02X} + {:02X} + {} = {:02X}) ZF={}",
                        reg,
                        rm,
                        dst,
                        src,
                        carry,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory to register
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let src = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let dst = self.get_reg8(reg);
                    let result = dst.wrapping_add(src).wrapping_add(carry);
                    self.set_reg8(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::trace!(
                        "ADC r8[{}], [mem] addr={:04X} ({:02X} + {:02X} + {} = {:02X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        carry,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // PUSH CS (0E)
            0x0E => {
                self.push16(mmu, self.regs.cs)?;
                log::debug!("PUSH CS (CS={:04X})", self.regs.cs);
                Ok(RealModeStep::Continue)
            }

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

                    log::trace!(
                        "ADD r8[{}], r8[{}] ({:02X} + {:02X} = {:02X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory operations - read, modify, write
                    let mem_addr = self.calculate_effective_address(mmu, mod_val as usize, rm)?;

                    // Read current value from memory
                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_add(src);

                    // Write result back to memory
                    self.regs
                        .write_mem_byte(mmu, self.regs.ds, mem_addr, result)?;

                    // Set flags based on result
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::trace!(
                        "ADD [mem], r8[{}] addr={:04X} ({:02X} + {:02X} = {:02X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }

                Ok(RealModeStep::Continue)
            }

            // ADD r/m16, r16 (01 /r)
            0x01 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let _rm = (modrm & 7) as usize;

                let src = self.get_reg16(reg);
                log::debug!(
                    "ADD r/m16, r16 (modrm={:02X}, reg={}, src={:04X})",
                    modrm,
                    reg,
                    src
                );

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
                let carry = if (self.regs.eflags & 0x01) != 0 {
                    1u8
                } else {
                    0u8
                };

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

                    log::trace!(
                        "ADC r8[{}], r8[{}] ({:02X} + {:02X} + {} = {:02X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        carry,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory operations - read, modify, write
                    let mem_addr = self.calculate_effective_address(mmu, mod_val as usize, rm)?;

                    // Read current value from memory
                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_add(src).wrapping_add(carry);

                    // Write result back to memory
                    self.regs
                        .write_mem_byte(mmu, self.regs.ds, mem_addr, result)?;

                    // Set flags based on result
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::trace!(
                        "ADC [mem], r8[{}] addr={:04X} ({:02X} + {:02X} + {} = {:02X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        carry,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
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
                let carry = if (self.regs.eflags & 0x01) != 0 {
                    1u16
                } else {
                    0u16
                };

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

                    log::trace!(
                        "ADC r16[{}], r16[{}] ({:04X} + {:04X} + {} = {:04X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        carry,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory operations
                    let mem_addr = self.calculate_effective_address(mmu, mod_val as usize, rm)?;

                    let dst = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_add(src).wrapping_add(carry);

                    self.regs
                        .write_mem_word(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::trace!(
                        "ADC16 [mem], r16[{}] addr={:04X} ({:04X} + {:04X} + {} = {:04X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        carry,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
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

            // PUSH ES (06)
            0x06 => {
                self.push16(mmu, self.regs.es)?;
                log::debug!("PUSH ES (ES={:04X})", self.regs.es);
                Ok(RealModeStep::Continue)
            }

            // POP ES (07)
            0x07 => {
                let val = self.pop16(mmu)?;
                self.regs.es = val;
                log::debug!("POP ES (val={:04X})", val);
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

                log::debug!(
                    "ADC AL, imm8 (imm={:02X}, cf={}, result={:02X})",
                    imm,
                    cf,
                    result
                );
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

                log::debug!(
                    "ADC AX, imm16 (imm={:04X}, cf={}, result={:04X})",
                    imm,
                    cf,
                    result
                );
                Ok(RealModeStep::Continue)
            }

            // PUSH SS (16)
            0x16 => {
                self.push16(mmu, self.regs.ss)?;
                log::debug!("PUSH SS (SS={:04X})", self.regs.ss);
                Ok(RealModeStep::Continue)
            }

            // POP SS (17)
            0x17 => {
                let val = self.pop16(mmu)?;
                self.regs.ss = val;
                log::debug!("POP SS (val={:04X})", val);
                Ok(RealModeStep::Continue)
            }

            // ===== SBB Instructions (Subtract with Borrow) =====

            // SBB r/m8, r8 (18 /r) - Subtract with Borrow
            0x18 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg8(reg);
                let cf = (self.regs.eflags & 0x01) != 0;
                let borrow = if cf { 1u8 } else { 0u8 };

                if mod_val == 3 {
                    // Register to register
                    let dst = self.get_reg8(rm);
                    let result = dst.wrapping_sub(src).wrapping_sub(borrow);
                    self.set_reg8(rm, result);

                    // Update flags
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    // CF: set if borrow occurred
                    let borrow_occurred = dst < src || (dst == src && cf);
                    if borrow_occurred {
                        self.regs.eflags |= 0x01; // CF
                    } else {
                        self.regs.eflags &= !0x01;
                    }

                    log::trace!(
                        "SBB r8[{}], r8[{}] ({:02X} - {:02X} - {} = {:02X}) ZF={} CF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        borrow,
                        result,
                        (self.regs.eflags & 0x40) != 0,
                        borrow_occurred
                    );
                } else {
                    // Memory operation
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
                            log::trace!(
                                "SBB [mem] - unsupported addressing mode mod={}, rm={}",
                                mod_val,
                                rm
                            );
                            return Ok(RealModeStep::Continue);
                        }
                    };

                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_sub(src).wrapping_sub(borrow);

                    self.regs
                        .write_mem_byte(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    let borrow_occurred = dst < src || (dst == src && cf);
                    if borrow_occurred {
                        self.regs.eflags |= 0x01;
                    } else {
                        self.regs.eflags &= !0x01;
                    }

                    log::trace!(
                        "SBB [mem], r8[{}] addr={:04X} ({:02X} - {:02X} - {} = {:02X}) ZF={} CF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        borrow,
                        result,
                        (self.regs.eflags & 0x40) != 0,
                        borrow_occurred
                    );
                }

                Ok(RealModeStep::Continue)
            }

            // SBB r/m16, r16 (19 /r) - Subtract with Borrow (16-bit)
            0x19 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg16(reg);
                let cf = (self.regs.eflags & 0x01) != 0;
                let borrow = if cf { 1u16 } else { 0u16 };

                if mod_val == 3 {
                    let dst = self.get_reg16(rm);
                    let result = dst.wrapping_sub(src).wrapping_sub(borrow);
                    self.set_reg16(rm, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    let borrow_occurred = dst < src || (dst == src && cf);
                    if borrow_occurred {
                        self.regs.eflags |= 0x01;
                    } else {
                        self.regs.eflags &= !0x01;
                    }

                    log::trace!(
                        "SBB r16[{}], r16[{}] ({:04X} - {:04X} - {} = {:04X}) ZF={} CF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        borrow,
                        result,
                        (self.regs.eflags & 0x40) != 0,
                        borrow_occurred
                    );
                } else {
                    let mem_addr = self.calculate_effective_address(mmu, mod_val as usize, rm)?;

                    let dst = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_sub(src).wrapping_sub(borrow);

                    self.regs
                        .write_mem_word(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    let borrow_occurred = dst < src || (dst == src && cf);
                    if borrow_occurred {
                        self.regs.eflags |= 0x01;
                    } else {
                        self.regs.eflags &= !0x01;
                    }

                    log::trace!(
                        "SBB [mem], r16[{}] addr={:04X} ({:04X} - {:04X} - {} = {:04X}) ZF={} CF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        borrow,
                        result,
                        (self.regs.eflags & 0x40) != 0,
                        borrow_occurred
                    );
                }

                Ok(RealModeStep::Continue)
            }

            // SBB r8, r/m8 (1A /r) - Subtract with Borrow (reverse)
            0x1A => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let cf = (self.regs.eflags & 0x01) != 0;
                let borrow = if cf { 1u8 } else { 0u8 };

                if mod_val == 3 {
                    // Register to register (reverse: dest is reg, src is rm)
                    let src = self.get_reg8(rm);
                    let dst = self.get_reg8(reg);
                    let result = dst.wrapping_sub(src).wrapping_sub(borrow);
                    self.set_reg8(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    let borrow_occurred = dst < src || (dst == src && cf);
                    if borrow_occurred {
                        self.regs.eflags |= 0x01;
                    } else {
                        self.regs.eflags &= !0x01;
                    }

                    log::trace!(
                        "SBB r8[{}], r8[{}] (rev) ({:02X} - {:02X} - {} = {:02X}) ZF={} CF={}",
                        reg,
                        rm,
                        dst,
                        src,
                        borrow,
                        result,
                        (self.regs.eflags & 0x40) != 0,
                        borrow_occurred
                    );
                } else {
                    // Memory to register
                    let mem_addr = self.calculate_effective_address(mmu, mod_val as usize, rm)?;

                    let src = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let dst = self.get_reg8(reg);
                    let result = dst.wrapping_sub(src).wrapping_sub(borrow);

                    self.set_reg8(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    let borrow_occurred = dst < src || (dst == src && cf);
                    if borrow_occurred {
                        self.regs.eflags |= 0x01;
                    } else {
                        self.regs.eflags &= !0x01;
                    }

                    log::trace!(
                        "SBB r8[{}], [mem] addr={:04X} ({:02X} - {:02X} - {} = {:02X}) ZF={} CF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        borrow,
                        result,
                        (self.regs.eflags & 0x40) != 0,
                        borrow_occurred
                    );
                }

                Ok(RealModeStep::Continue)
            }

            // SBB r16, r/m16 (1B /r) - Subtract with Borrow (reverse, 16-bit)
            0x1B => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let cf = (self.regs.eflags & 0x01) != 0;
                let borrow = if cf { 1u16 } else { 0u16 };

                if mod_val == 3 {
                    let src = self.get_reg16(rm);
                    let dst = self.get_reg16(reg);
                    let result = dst.wrapping_sub(src).wrapping_sub(borrow);
                    self.set_reg16(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    let borrow_occurred = dst < src || (dst == src && cf);
                    if borrow_occurred {
                        self.regs.eflags |= 0x01;
                    } else {
                        self.regs.eflags &= !0x01;
                    }

                    log::trace!(
                        "SBB r16[{}], r16[{}] (rev) ({:04X} - {:04X} - {} = {:04X}) ZF={} CF={}",
                        reg,
                        rm,
                        dst,
                        src,
                        borrow,
                        result,
                        (self.regs.eflags & 0x40) != 0,
                        borrow_occurred
                    );
                } else {
                    let mem_addr = self.calculate_effective_address(mmu, mod_val as usize, rm)?;

                    let src = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let dst = self.get_reg16(reg);
                    let result = dst.wrapping_sub(src).wrapping_sub(borrow);

                    self.set_reg16(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    let borrow_occurred = dst < src || (dst == src && cf);
                    if borrow_occurred {
                        self.regs.eflags |= 0x01;
                    } else {
                        self.regs.eflags &= !0x01;
                    }

                    log::trace!(
                        "SBB r16[{}], [mem] addr={:04X} ({:04X} - {:04X} - {} = {:04X}) ZF={} CF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        borrow,
                        result,
                        (self.regs.eflags & 0x40) != 0,
                        borrow_occurred
                    );
                }

                Ok(RealModeStep::Continue)
            }

            // SBB AL, imm8 (1C ib) - Subtract with Borrow immediate
            0x1C => {
                let imm = self.fetch_byte(mmu)?;
                let al = (self.regs.eax & 0xFF) as u8;
                let cf = (self.regs.eflags & 0x01) != 0;
                let borrow = if cf { 1u8 } else { 0u8 };

                let result = al.wrapping_sub(imm).wrapping_sub(borrow);
                self.regs.eax = (self.regs.eax & !0xFF) | (result as u32);

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                let borrow_occurred = al < imm || (al == imm && cf);
                if borrow_occurred {
                    self.regs.eflags |= 0x01;
                } else {
                    self.regs.eflags &= !0x01;
                }

                log::trace!(
                    "SBB AL, imm8 (imm={:02X}, al={:02X}, borrow={}, result={:02X}) ZF={} CF={}",
                    imm,
                    al,
                    borrow,
                    result,
                    (self.regs.eflags & 0x40) != 0,
                    borrow_occurred
                );
                Ok(RealModeStep::Continue)
            }

            // SBB EAX, imm32 (1D id) - Subtract with Borrow immediate (16-bit: SBB AX, imm16)
            0x1D => {
                let imm = self.fetch_word(mmu)?; // In real mode, this is 16-bit
                let ax = (self.regs.eax & 0xFFFF) as u16;
                let cf = (self.regs.eflags & 0x01) != 0;
                let borrow = if cf { 1u16 } else { 0u16 };

                let result = ax.wrapping_sub(imm).wrapping_sub(borrow);
                self.regs.eax = (self.regs.eax & !0xFFFF) | (result as u32);

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                let borrow_occurred = ax < imm || (ax == imm && cf);
                if borrow_occurred {
                    self.regs.eflags |= 0x01;
                } else {
                    self.regs.eflags &= !0x01;
                }

                log::trace!(
                    "SBB AX, imm16 (imm={:04X}, ax={:04X}, borrow={}, result={:04X}) ZF={} CF={}",
                    imm,
                    ax,
                    borrow,
                    result,
                    (self.regs.eflags & 0x40) != 0,
                    borrow_occurred
                );
                Ok(RealModeStep::Continue)
            }

            // PUSH DS (1E)
            0x1E => {
                self.push16(mmu, self.regs.ds)?;
                log::debug!("PUSH DS (DS={:04X})", self.regs.ds);
                Ok(RealModeStep::Continue)
            }

            // POP DS (1F)
            0x1F => {
                let val = self.pop16(mmu)?;
                self.regs.ds = val;
                log::debug!("POP DS (val={:04X})", val);
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

                    log::trace!(
                        "SUB r8[{}], r8[{}] ({:02X} - {:02X} = {:02X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
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
                            log::trace!(
                                "SUB [mem] - unsupported addressing mode mod={}, rm={}",
                                mod_val,
                                rm
                            );
                            return Ok(RealModeStep::Continue);
                        }
                    };

                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_sub(src);

                    self.regs
                        .write_mem_byte(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::trace!(
                        "SUB [mem], r8[{}] addr={:04X} ({:02X} - {:02X} = {:02X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
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

                    log::trace!(
                        "SUB r16[{}], r16[{}] ({:04X} - {:04X} = {:04X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    let mem_addr = self.calculate_effective_address(mmu, mod_val as usize, rm)?;

                    let dst = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_sub(src);

                    self.regs
                        .write_mem_word(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::trace!(
                        "SUB16 [mem], r16[{}] addr={:04X} ({:04X} - {:04X} = {:04X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }

                Ok(RealModeStep::Continue)
            }

            // SUB r8, r/m8 (2A /r) - Subtraction (reverse operands)
            0x2A => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                if mod_val == 3 {
                    // Register to register (reverse: dest is reg, src is rm)
                    let src = self.get_reg8(rm);
                    let dst = self.get_reg8(reg);
                    let result = dst.wrapping_sub(src);
                    self.set_reg8(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::trace!(
                        "SUB r8[{}], r8[{}] (rev) ({:02X} - {:02X} = {:02X}) ZF={}",
                        reg,
                        rm,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory to register - use the new helper function
                    let mem_addr = self.calculate_effective_address(mmu, mod_val as usize, rm)?;

                    let src = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let dst = self.get_reg8(reg);
                    let result = dst.wrapping_sub(src);

                    self.set_reg8(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::trace!(
                        "SUB r8[{}], [mem] addr={:04X} ({:02X} - {:02X} = {:02X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }

                Ok(RealModeStep::Continue)
            }

            // SUB r16, r/m16 (2B /r) - Subtraction (reverse operands, 16-bit)
            0x2B => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                if mod_val == 3 {
                    // Register to register (reverse: dest is reg, src is rm)
                    let src = self.get_reg16(rm);
                    let dst = self.get_reg16(reg);
                    let result = dst.wrapping_sub(src);
                    self.set_reg16(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::trace!(
                        "SUB r16[{}], r16[{}] (rev) ({:04X} - {:04X} = {:04X}) ZF={}",
                        reg,
                        rm,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory to register
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
                            log::trace!(
                                "SUB16 r16, [mem] - unsupported addressing mode mod={}, rm={}",
                                mod_val,
                                rm
                            );
                            return Ok(RealModeStep::Continue);
                        }
                    };

                    let src = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let dst = self.get_reg16(reg);
                    let result = dst.wrapping_sub(src);

                    self.set_reg16(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::trace!(
                        "SUB r16[{}], [mem] addr={:04X} ({:04X} - {:04X} = {:04X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
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

                log::trace!(
                    "SUB AL, imm8 (imm={:02X}, result={:02X}) ZF={}",
                    imm,
                    result,
                    (self.regs.eflags & 0x40) != 0
                );
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

                log::trace!(
                    "SUB AX, imm16 (imm={:04X}, result={:04X}) ZF={}",
                    imm,
                    result,
                    (self.regs.eflags & 0x40) != 0
                );
                Ok(RealModeStep::Continue)
            }

            // ===== XOR Instructions =====
            // XOR r/m8, r8 (30 /r) - Logical Exclusive OR
            0x30 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg8(reg);

                if mod_val == 3 {
                    // Register to register
                    let dst = self.get_reg8(rm);
                    let result = dst ^ src;
                    self.set_reg8(rm, result);

                    // Set flags
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    // Clear OF and CF
                    self.regs.eflags &= !0x800; // OF
                    self.regs.eflags &= !0x1; // CF

                    log::trace!(
                        "XOR r8[{}], r8[{}] ({:02X} ^ {:02X} = {:02X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    // Memory operation
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst ^ src;
                    self.regs
                        .write_mem_byte(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    self.regs.eflags &= !0x800;
                    self.regs.eflags &= !0x1;

                    log::trace!(
                        "XOR [mem], r8 addr={:04X} ({:02X} ^ {:02X} = {:02X}) ZF={}",
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // XOR r/m16, r16 (31 /r) - Logical Exclusive OR
            0x31 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg16(reg);

                if mod_val == 3 {
                    // Register to register
                    let dst = self.get_reg16(rm);
                    let result = dst ^ src;
                    self.set_reg16(rm, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    self.regs.eflags &= !0x800;
                    self.regs.eflags &= !0x1;

                    log::trace!(
                        "XOR r16[{}], r16[{}] ({:04X} ^ {:04X} = {:04X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let dst = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let result = dst ^ src;
                    self.regs
                        .write_mem_word(mmu, self.regs.ds, mem_addr, result)?;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    self.regs.eflags &= !0x800;
                    self.regs.eflags &= !0x1;

                    log::trace!(
                        "XOR [mem], r16 addr={:04X} ({:04X} ^ {:04X} = {:04X}) ZF={}",
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // XOR r8, r/m8 (0A /r) - Logical Exclusive OR (reverse)
            0x32 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                if mod_val == 3 {
                    let src = self.get_reg8(rm);
                    let dst = self.get_reg8(reg);
                    let result = dst ^ src;
                    self.set_reg8(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    self.regs.eflags &= !0x800;
                    self.regs.eflags &= !0x1;

                    log::trace!(
                        "XOR r8[{}], r8[{}] ({:02X} ^ {:02X} = {:02X}) ZF={}",
                        reg,
                        rm,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let src = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let dst = self.get_reg8(reg);
                    let result = dst ^ src;
                    self.set_reg8(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    self.regs.eflags &= !0x800;
                    self.regs.eflags &= !0x1;

                    log::trace!(
                        "XOR r8[{}], [mem] addr={:04X} ({:02X} ^ {:02X} = {:02X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // XOR r16, r/m16 (33 /r) - Logical Exclusive OR (reverse, 16-bit)
            0x33 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                if mod_val == 3 {
                    let src = self.get_reg16(rm);
                    let dst = self.get_reg16(reg);
                    let result = dst ^ src;
                    self.set_reg16(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    self.regs.eflags &= !0x800;
                    self.regs.eflags &= !0x1;

                    log::trace!(
                        "XOR r16[{}], r16[{}] ({:04X} ^ {:04X} = {:04X}) ZF={}",
                        reg,
                        rm,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let src = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let dst = self.get_reg16(reg);
                    let result = dst ^ src;
                    self.set_reg16(reg, result);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    self.regs.eflags &= !0x800;
                    self.regs.eflags &= !0x1;

                    log::trace!(
                        "XOR r16[{}], [mem] addr={:04X} ({:04X} ^ {:04X} = {:04X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // XOR AL, imm8 (34 ib) - Logical Exclusive OR immediate
            0x34 => {
                let imm = self.fetch_byte(mmu)?;
                let al = (self.regs.eax & 0xFF) as u8;
                let result = al ^ imm;
                self.regs.eax = (self.regs.eax & !0xFF) | (result as u32);

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                self.regs.eflags &= !0x800;
                self.regs.eflags &= !0x1;

                log::trace!(
                    "XOR AL, imm8 (imm={:02X}, al={:02X}, result={:02X}) ZF={}",
                    imm,
                    al,
                    result,
                    (self.regs.eflags & 0x40) != 0
                );
                Ok(RealModeStep::Continue)
            }

            // XOR AX, imm16 (35 iw) - Logical Exclusive OR immediate
            0x35 => {
                let imm = self.fetch_word(mmu)?;
                let ax = (self.regs.eax & 0xFFFF) as u16;
                let result = ax ^ imm;
                self.regs.eax = (self.regs.eax & !0xFFFF) | (result as u32);

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                self.regs.eflags &= !0x800;
                self.regs.eflags &= !0x1;

                log::trace!(
                    "XOR AX, imm16 (imm={:04X}, ax={:04X}, result={:04X}) ZF={}",
                    imm,
                    ax,
                    result,
                    (self.regs.eflags & 0x40) != 0
                );
                Ok(RealModeStep::Continue)
            }

            // CMP AL, imm8 (3C ib) - Compare immediate with AL
            0x3C => {
                let imm = self.fetch_byte(mmu)?;
                let al = (self.regs.eax & 0xFF) as u8;
                let result = al.wrapping_sub(imm);

                // Set flags based on result but don't store it
                if result == 0 {
                    self.regs.eflags |= 0x40; // ZF
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::debug!(
                    "CMP AL, imm8 (imm={:02X}, al={:02X}, result={:02X}) ZF={}",
                    imm,
                    al,
                    result,
                    (self.regs.eflags & 0x40) != 0
                );
                Ok(RealModeStep::Continue)
            }

            // CMP AX, imm16 (3D iw) - Compare immediate with AX
            0x3D => {
                let imm = self.fetch_word(mmu)?;
                let ax = (self.regs.eax & 0xFFFF) as u16;
                let result = ax.wrapping_sub(imm);

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::debug!(
                    "CMP AX, imm16 (imm={:04X}, ax={:04X}, result={:04X}) ZF={}",
                    imm,
                    ax,
                    result,
                    (self.regs.eflags & 0x40) != 0
                );
                Ok(RealModeStep::Continue)
            }

            // TEST r/m8, r8 (84 /r) - Test bits (AND but don't store result)
            0x84 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg8(reg);

                if mod_val == 3 {
                    let dst = self.get_reg8(rm);
                    let result = dst & src;

                    // Set flags but don't store result
                    if result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "TEST r8[{}], r8[{}] ({:02X} & {:02X} = {:02X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst & src;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "TEST [mem], r8 addr={:04X} ({:02X} & {:02X} = {:02X}) ZF={}",
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // TEST r/m16, r16 (85 /r) - Test bits (AND but don't store result)
            0x85 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg16(reg);

                if mod_val == 3 {
                    let dst = self.get_reg16(rm);
                    let result = dst & src;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "TEST r16[{}], r16[{}] ({:04X} & {:04X} = {:04X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let dst = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let result = dst & src;

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "TEST [mem], r16 addr={:04X} ({:04X} & {:04X} = {:04X}) ZF={}",
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // TEST AX, imm16 (A9 iw) - Test immediate with AX
            0xA9 => {
                let imm = self.fetch_word(mmu)?;
                let ax = (self.regs.eax & 0xFFFF) as u16;
                let result = ax & imm;

                if result == 0 {
                    self.regs.eflags |= 0x40;
                } else {
                    self.regs.eflags &= !0x40;
                }

                log::debug!(
                    "TEST AX, imm16 (imm={:04X}, ax={:04X}, result={:04X}) ZF={}",
                    imm,
                    ax,
                    result,
                    (self.regs.eflags & 0x40) != 0
                );
                Ok(RealModeStep::Continue)
            }

            // CMP r/m8, r8 (38 /r) - Compare r/m8 with r8
            0x38 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                let src = self.get_reg8(reg);

                if mod_val == 3 {
                    let dst = self.get_reg8(rm);
                    let result = dst.wrapping_sub(src);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "CMP r/m8, r8 ({:02X} - {:02X} = {:02X}) ZF={}",
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let dst = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_sub(src);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "CMP [mem], r8 addr={:04X} ({:02X} - {:02X} = {:02X}) ZF={}",
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
                Ok(RealModeStep::Continue)
            }

            // CMP r/m16, r16 (39 /r) - Compare r/m16 with r16
            0x39 => {
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

                    log::debug!(
                        "CMP r/m16, r16 ({:04X} - {:04X} = {:04X}) ZF={}",
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                } else {
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                    let dst = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                    let result = dst.wrapping_sub(src);

                    if result == 0 {
                        self.regs.eflags |= 0x40;
                    } else {
                        self.regs.eflags &= !0x40;
                    }

                    log::debug!(
                        "CMP [mem], r16 addr={:04X} ({:04X} - {:04X} = {:04X}) ZF={}",
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
                }
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

                    log::warn!(
                        "CMP r8[{}], r8[{}] ({:02X} - {:02X} = {:02X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
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
                            log::warn!(
                                "CMP [mem] - unsupported addressing mode mod={}, rm={}",
                                mod_val,
                                rm
                            );
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

                    log::warn!(
                        "CMP [mem], r8[{}] addr={:04X} ({:02X} - {:02X} = {:02X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
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

                    log::warn!(
                        "CMP r16[{}], r16[{}] ({:04X} - {:04X} = {:04X}) ZF={}",
                        rm,
                        reg,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
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
                            log::warn!(
                                "CMP16 [mem] - unsupported addressing mode mod={}, rm={}",
                                mod_val,
                                rm
                            );
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

                    log::warn!(
                        "CMP16 [mem], r16[{}] addr={:04X} ({:04X} - {:04X} = {:04X}) ZF={}",
                        reg,
                        mem_addr,
                        dst,
                        src,
                        result,
                        (self.regs.eflags & 0x40) != 0
                    );
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

                    log::debug!(
                        "AND r8[{}], r8[{}] ({:02X} & {:02X} = {:02X})",
                        rm,
                        reg,
                        dst,
                        src,
                        result
                    );
                } else {
                    // Memory mode - perform memory AND operation
                    let src_val = match opcode {
                        // AND r8, r/m8 (20 /r)
                        0x20 | 0x22 => {
                            let src_val = self.read_mem_byte(mmu, self.regs.ds, rm as u16)?;
                            let reg_val = self.get_reg8(reg);
                            let result = reg_val & src_val;
                            self.set_reg8(reg, result);
                            self.update_flags_zsp8(result);
                            result
                        }
                        _ => {
                            // Memory mode - unsupported addressing mode in real mode
                            log::debug!("AND [mem], r8[{}] - addressing mode 0x{:02X} not implemented in real mode", opcode, reg);
                            return Err(VmError::Execution(ExecutionError::InvalidOpcode {
                                message: format!("Unsupported addressing mode in real mode: {}", modrm),
                                pc: self.get_pc_linear(),
                            }));
                        }
                    };
                    Ok(RealModeStep::Continue)
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

                    log::debug!(
                        "AND r16[{}], r16[{}] ({:04X} & {:04X} = {:04X})",
                        rm,
                        reg,
                        dst,
                        src,
                        result
                    );
                 } else {
                    // Memory mode - perform AND operation between r16 and memory
                    let mem_val = self.read_mem_word(mmu, self.regs.ds, self.regs.ebx as u16)?;
                    
                    // Read destination register
                    let dst = self.get_reg16(rm);
                    let result = dst & mem_val;
                    
                    // Store result in memory
                    self.regs.write_mem_word(mmu, self.regs.ds, self.regs.ebx as u16, result)?;
                    
                    // Update flags (zero, sign, parity)
                    self.update_flags_zsp16(result);
                    
                    log::debug!("AND [mem], r16[{}] DS:[{:04X}] (dst={:04X}) = {:04X}", 
                        rm, self.regs.ebx, result, result);
                    
                    Ok(RealModeStep::Continue)
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

                    log::debug!(
                        "AND r8[{}], r8[{}] ({:02X} & {:02X} = {:02X})",
                        reg,
                        rm,
                        dst,
                        src,
                        result
                    );
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

                    log::debug!(
                        "AND r16[{}], r16[{}] ({:04X} & {:04X} = {:04X})",
                        reg,
                        rm,
                        dst,
                        src,
                        result
                    );
                 } else {
                    // Memory mode - perform 16-bit AND operation between r16 and memory
                    let mem_val = self.read_mem_word(mmu, self.regs.ds, self.regs.esi as u16)?;
                    
                    // Read destination register
                    let dst = self.get_reg16(rm);
                    
                    // Perform AND operation
                    let result = dst & mem_val;
                    
                    // Store result in memory
                    self.regs.write_mem_word(mmu, self.regs.ds, self.regs.esi as u16, result)?;
                    
                    // Update flags (zero, sign, parity)
                    self.update_flags_zsp16(result);
                    
                    result
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

                log::warn!(
                    "Group 5 (FE) opcode at CS:IP={:04X}:{:08X}, reg={}, modrm={:02X}, mod={}",
                    self.regs.cs,
                    self.regs.eip - 2,
                    reg,
                    modrm,
                    mod_val
                );

                match reg {
                    // INC r/m8 (FE /0)
                    0 => {
                        // [EAX] - increment memory byte
                        let addr = (self.regs.eax & 0xFFFF) as u16;
                        let old_val = self.read_mem_byte(mmu, self.regs.ds, addr)?;
                        let new_val = old_val.wrapping_add(1);
                        self.regs.write_mem_byte(mmu, self.regs.ds, addr, new_val)?;

                        // Update zero flag
                        if new_val == 0 {
                            self.regs.eflags |= 0x40; // ZF
                        } else {
                            self.regs.eflags &= !0x40;
                        }

                        // Update sign flag (MSB)
                        if (new_val as i8).is_negative() {
                            self.regs.eflags |= 0x80; // SF
                        } else {
                            self.regs.eflags &= !0x80;
                        }

                        // Update parity flag
                        let parity = new_val.count_ones() % 2 == 0;
                        if parity {
                            self.regs.eflags |= 0x04; // PF
                        } else {
                            self.regs.eflags &= !0x04;
                        }

                        log::debug!(
                            "  INC DS:[{:04X}] = {:02X} -> {:02X}",
                            addr,
                            old_val,
                            new_val
                        );
                        Ok(RealModeStep::Continue)
                    }
                     // DEC r/m8 (FE /1)
                     1 => {
                         // [EAX] - decrement memory byte
                         let addr = (self.regs.eax & 0xFFFF) as u16;
                         let old_val = self.read_mem_byte(mmu, self.regs.ds, addr)?;
                         let new_val = old_val.wrapping_sub(1);
                         self.regs.write_mem_byte(mmu, self.regs.ds, addr, new_val)?
                         
                         // Update zero flag
                         if new_val == 0 {
                             self.regs.eflags |= 0x40; // ZF
                         } else {
                             self.regs.eflags &= !0x40;
                         }
                         
                         // Update sign flag (MSB)
                         if (new_val as i8).is_negative() {
                             self.regs.eflags |= 0x80; // SF
                         } else {
                             self.regs.eflags &= !0x80;
                         }
                         
                         // Update parity flag
                         let parity = new_val.count_ones() % 2 == 0;
                         if parity {
                             self.regs.eflags |= 0x04; // PF
                         } else {
                             self.regs.eflags &= !0x04;
                         }
                         
                         log::debug!("  DEC DS:[{:04X}] = {:02X} -> {:02X}", addr, old_val, new_val);
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

                log::warn!(
                    "Group 5 (FF) opcode at CS:IP={:04X}:{:08X}, reg={}, modrm={:02X}, mod={}",
                    self.regs.cs,
                    self.regs.eip - 2,
                    reg,
                    modrm,
                    mod_val
                );

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
                            log::warn!(
                                "  INC [BX+SI] with BX={:04X}, SI={:04X}, addr={:04X}",
                                bx,
                                si,
                                addr
                            );
                            match self.regs.read_mem_word(mmu, self.regs.ds, addr) {
                                Ok(old_val) => {
                                    let new_val = old_val.wrapping_add(1);
                                    self.regs.write_mem_word(mmu, self.regs.ds, addr, new_val)?;
                                    log::warn!(
                                        "  INC [{:04X}] = {:04X} -> {:04X}",
                                        addr,
                                        old_val,
                                        new_val
                                    );
                                }
                                Err(e) => {
                                    log::error!(
                                        "  Failed to read/write memory at DS:{:04X}: {:?}",
                                        addr,
                                        e
                                    );
                                }
                            }
                        } else {
                            // Other memory addressing modes - treat as NOP for now
                            log::warn!(
                                "  INC [mem] - treating as NOP (mod={}, rm={})",
                                mod_val,
                                rm
                            );
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
                            log::warn!(
                                "  DEC [mem] - treating as NOP (mod={}, rm={})",
                                mod_val,
                                rm
                            );
                        }
                        Ok(RealModeStep::Continue)
                    }
                     // CALL r/m16 (FF /2) - NEAR call
                     2 => {
                        // Read target address from r/m16
                        let target = self.fetch_word(mmu)?;
                        
                        // Push current IP to stack (return address)
                        let sp = (self.regs.ss as u32).wrapping_sub(2);
                        self.regs.write_mem_word(mmu, self.regs.ss, sp as u16, self.regs.eip as u16)?;
                        self.regs.esp = sp;
                        
                        // Jump to target address
                        log::debug!("CALL r/m16[{:04X}] - pushing IP to stack and jumping to target", target);
                        self.regs.eip = target as u32;
                        
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
                            log::warn!(
                                "  JMP [BX] with BX={:04X} - reading target from DS:{:04X}",
                                bx,
                                self.regs.ds
                            );

                            // Log all registers for debugging
                            log::warn!(
                                "  REGS: AX={:04X} BX={:04X} CX={:04X} DX={:04X} SI={:04X} DI={:04X} BP={:04X} SP={:04X}",
                                self.regs.eax & 0xFFFF,
                                self.regs.ebx & 0xFFFF,
                                self.regs.ecx & 0xFFFF,
                                self.regs.edx & 0xFFFF,
                                self.regs.esi & 0xFFFF,
                                self.regs.edi & 0xFFFF,
                                self.regs.ebp & 0xFFFF,
                                self.regs.esp & 0xFFFF
                            );
                            log::warn!(
                                "  SEGS: CS={:04X} DS={:04X} ES={:04X} SS={:04X}",
                                self.regs.cs,
                                self.regs.ds,
                                self.regs.es,
                                self.regs.ss
                            );

                            match self.regs.read_mem_word(mmu, self.regs.ds, bx as u16) {
                                Ok(target) => {
                                    log::warn!("  JMP [BX] -> jumping to {:04X}", target);
                                    self.regs.eip = target as u32;

                                    // Detect potential loop
                                    if target == 0x0000 {
                                        log::error!(
                                            "  WARNING: Jumping to 0000 - this may cause infinite loop!"
                                        );
                                        log::error!(
                                            "  This usually indicates incorrect BX value or uninitialized memory"
                                        );
                                    }
                                }
                                Err(e) => {
                                    log::error!(
                                        "  Failed to read jump target from [DS:BX]: {:?}",
                                        e
                                    );
                                }
                            }
                        } else if mod_val == 0 && rm == 0 {
                            // Special case: [BX+SI] - jump to address stored at [DS:BX+SI]
                            let bx = self.regs.ebx & 0xFFFF;
                            let si = self.regs.esi & 0xFFFF;
                            let addr = (bx + si) & 0xFFFF;

                            log::debug!(
                                "  JMP [BX+SI] with BX={:04X}, SI={:04X}, addr={:04X} - reading target from DS:{:04X}",
                                bx,
                                si,
                                addr,
                                self.regs.ds
                            );

                            match self.regs.read_mem_word(mmu, self.regs.ds, addr as u16) {
                                Ok(target) => {
                                    log::debug!("  JMP [BX+SI] -> jumping to {:04X}", target);
                                    self.regs.eip = target as u32;
                                }
                                Err(e) => {
                                    log::error!(
                                        "  Failed to read jump target from [DS:BX+SI]: {:?}",
                                        e
                                    );
                                }
                            }
                        } else {
                            log::warn!(
                                "  JMP [mem] - treating as NOP (mod={}, rm={})",
                                mod_val,
                                rm
                            );
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
                         // Push 16-bit value onto stack (SS:SP - 2)
                         let val = self.fetch_word(mmu)?;
                         let sp = (self.regs.ss as u32).wrapping_sub(2);
                         
                         // Write value to stack
                         let addr = self.seg_to_linear(self.regs.ss, sp as u16);
                         self.write_mem_word(mmu, self.regs.ss, addr, val)?;
                         
                         // Update stack pointer
                         self.regs.esp = sp;
                         
                         // Update flags for push operation
                         if val == 0 {
                             self.regs.eflags |= 0x40; // ZF
                         } else {
                             self.regs.eflags &= !0x40;
                         }
                         
                         log::debug!("  PUSH r16[{:04X}] -> SS:[{:04X}] (esp={:#010X})", rm, val, sp);
                         Ok(RealModeStep::Continue)
                     }
                    // INC r/m16 (FF /7) - alternate encoding for INC
                    7 | _ => {
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

                            log::debug!(
                                "  INC r16[{}] (alt encoding) = {:04X} -> {:04X}",
                                rm,
                                old_val,
                                new_val
                            );
                        } else {
                            // Memory addressing mode - treat as NOP for now
                            log::warn!(
                                "  INC [mem] (alt encoding) - treating as NOP (mod={}, rm={})",
                                mod_val,
                                rm
                            );
                        }
                        Ok(RealModeStep::Continue)
                    }
                }
            }

            // ===== Data Movement =====

            // ===== Conditional Jumps (Short) =====

            // JO rel8 (70 cb) - Jump if Overflow
            0x70 => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x800) != 0 {
                    // OF (bit 11)
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JO rel8={:02X} (jump taken, OF=1)", offset);
                } else {
                    log::debug!("JO rel8={:02X} (not taken, OF=0)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JNO rel8 (71 cb) - Jump if Not Overflow
            0x71 => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x800) == 0 {
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JNO rel8={:02X} (jump taken, OF=0)", offset);
                 } else {
                            // Memory mode - unsupported addressing mode
                            log::debug!("AND [mem], r8[{}] - addressing mode 0x{:02X} not implemented in real mode", opcode, reg);
                            return Err(VmError::Execution(ExecutionError::InvalidOpcode {
                                message: format!("Unsupported addressing mode in real mode: {}", modrm),
                                pc: self.get_pc_linear(),
                            }));
                        }
                Ok(RealModeStep::Continue)
            }

            // JB/JC/JNAE rel8 (72 cb) - Jump if Below/Carry
            0x72 => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x01) != 0 {
                    // CF (bit 0)
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JB/JC rel8={:02X} (jump taken, CF=1)", offset);
                } else {
                    log::debug!("JB/JC rel8={:02X} (not taken, CF=0)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JNB/JNC/JAE rel8 (73 cb) - Jump if Not Below/Carry
            0x73 => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x01) == 0 {
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JNB/JNC rel8={:02X} (jump taken, CF=0)", offset);
                } else {
                    log::debug!("JNB/JNC rel8={:02X} (not taken, CF=1)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JE/JZ rel8 (74 cb) - Jump if Equal/Zero
            0x74 => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x40) != 0 {
                    // ZF (bit 6)
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JE/JZ rel8={:02X} (jump taken, ZF=1)", offset);
                } else {
                    log::debug!("JE/JZ rel8={:02X} (not taken, ZF=0)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JNE/JNZ rel8 (75 cb) - Jump if Not Equal/Zero
            0x75 => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x40) == 0 {
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JNE/JNZ rel8={:02X} (jump taken, ZF=0)", offset);
                } else {
                    log::debug!("JNE/JNZ rel8={:02X} (not taken, ZF=1)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JBE/JNA rel8 (76 cb) - Jump if Below or Equal
            0x76 => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x41) != 0 {
                    // CF or ZF set
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JBE/JNA rel8={:02X} (jump taken, CF|ZF=1)", offset);
                } else {
                    log::debug!("JBE/JNA rel8={:02X} (not taken, CF|ZF=0)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JNBE/JA rel8 (77 cb) - Jump if Not Below or Equal / Above
            0x77 => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x41) == 0 {
                    // CF and ZF clear
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JNBE/JA rel8={:02X} (jump taken, CF|ZF=0)", offset);
                } else {
                    log::debug!("JNBE/JA rel8={:02X} (not taken, CF|ZF=1)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JS rel8 (78 cb) - Jump if Sign
            0x78 => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x80) != 0 {
                    // SF (bit 7)
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JS rel8={:02X} (jump taken, SF=1)", offset);
                } else {
                    log::debug!("JS rel8={:02X} (not taken, SF=0)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JNS rel8 (79 cb) - Jump if Not Sign
            0x79 => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x80) == 0 {
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JNS rel8={:02X} (jump taken, SF=0)", offset);
                } else {
                    log::debug!("JNS rel8={:02X} (not taken, SF=1)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JP/JPE rel8 (7A cb) - Jump if Parity Even
            0x7A => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x04) != 0 {
                    // PF (bit 2)
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JP/JPE rel8={:02X} (jump taken, PF=1)", offset);
                } else {
                    log::debug!("JP/JPE rel8={:02X} (not taken, PF=0)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JNP/JPO rel8 (7B cb) - Jump if Not Parity / Parity Odd
            0x7B => {
                let offset = self.fetch_byte(mmu)? as i8;
                if (self.regs.eflags & 0x04) == 0 {
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JNP/JPO rel8={:02X} (jump taken, PF=0)", offset);
                } else {
                    log::debug!("JNP/JPO rel8={:02X} (not taken, PF=1)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JL/JNGE rel8 (7C cb) - Jump if Less / Not Greater or Equal
            0x7C => {
                let offset = self.fetch_byte(mmu)? as i8;
                let sf = (self.regs.eflags & 0x80) != 0;
                let of = (self.regs.eflags & 0x800) != 0;
                if sf != of {
                    // SF != OF
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JL/JNGE rel8={:02X} (jump taken, SF!=OF)", offset);
                } else {
                    log::debug!("JL/JNGE rel8={:02X} (not taken, SF==OF)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JNL/JGE rel8 (7D cb) - Jump if Not Less / Greater or Equal
            0x7D => {
                let offset = self.fetch_byte(mmu)? as i8;
                let sf = (self.regs.eflags & 0x80) != 0;
                let of = (self.regs.eflags & 0x800) != 0;
                if sf == of {
                    // SF == OF
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JNL/JGE rel8={:02X} (jump taken, SF==OF)", offset);
                } else {
                    log::debug!("JNL/JGE rel8={:02X} (not taken, SF!=OF)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JLE/JNG rel8 (7E cb) - Jump if Less or Equal / Not Greater
            0x7E => {
                let offset = self.fetch_byte(mmu)? as i8;
                let zf = (self.regs.eflags & 0x40) != 0;
                let sf = (self.regs.eflags & 0x80) != 0;
                let of = (self.regs.eflags & 0x800) != 0;
                if zf || (sf != of) {
                    // ZF=1 or SF!=OF
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JLE/JNG rel8={:02X} (jump taken, ZF=1 or SF!=OF)", offset);
                } else {
                    log::debug!("JLE/JNG rel8={:02X} (not taken, ZF=0 and SF==OF)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // JNLE/JG rel8 (7F cb) - Jump if Not Less or Equal / Greater
            0x7F => {
                let offset = self.fetch_byte(mmu)? as i8;
                let zf = (self.regs.eflags & 0x40) != 0;
                let sf = (self.regs.eflags & 0x80) != 0;
                let of = (self.regs.eflags & 0x800) != 0;
                if !zf && (sf == of) {
                    // ZF=0 and SF==OF
                    self.regs.eip = self.regs.eip.wrapping_add(offset as u32);
                    log::debug!("JNLE/JG rel8={:02X} (jump taken, ZF=0 and SF==OF)", offset);
                } else {
                    log::debug!("JNLE/JG rel8={:02X} (not taken, ZF=1 or SF!=OF)", offset);
                }
                Ok(RealModeStep::Continue)
            }

            // ===== Control Transfer Instructions =====

            // RET near (C3) - Return from near call
            0xC3 => {
                let sp = (self.regs.esp & 0xFFFF) as u16;
                match self.regs.read_mem_word(mmu, self.regs.ss, sp) {
                    Ok(return_addr) => {
                        self.regs.esp =
                            (self.regs.esp & 0xFFFF0000) | ((sp.wrapping_add(2) & 0xFFFF) as u32);
                        self.regs.eip = return_addr as u32;
                        log::debug!("RET near (returning to {:04X})", return_addr);
                    }
                    Err(e) => {
                        log::error!(
                            "RET near: Failed to read return address from stack: {:?}",
                            e
                        );
                    }
                }
                Ok(RealModeStep::Continue)
            }

            // NOP
            0x90 => Ok(RealModeStep::Continue),

            // MOV reg8, imm8 (B0+reg cw)
            0xB0..=0xB7 => {
                let val = self.fetch_byte(mmu)?;
                let reg = (opcode - 0xB0) as usize;
                self.set_reg8(reg, val);
                log::debug!("MOV r8[{}], imm8 (val={:02X})", reg, val);
                Ok(RealModeStep::Continue)
            }

            // Group 2 - rotate/shift with imm8 (C0/C1)
            // MUST come before MOV reg16 pattern (0xB8..=0xBF) because Rust match patterns
            // are evaluated in order, and 0xB8..=0xBF would incorrectly match 0xC0/0xC1
            0xC0 | 0xC1 => {
                let modrm = self.fetch_byte(mmu)?;
                let imm = self.fetch_byte(mmu)?;
                let reg = (modrm >> 3) & 7;
                let rm = (modrm & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let is_word = opcode == 0xC1; // C0 = byte, C1 = word

                if mod_val == 3 {
                    // Register-direct mode
                    let _result = match reg {
                        0 => {
                            // ROL - Rotate Left
                            if is_word {
                                let val = self.get_reg16(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.rotate_left(count);
                                self.set_reg16(rm, result);
                                log::debug!("ROL r16[{}] count={}", rm, count);
                                result as u32
                            } else {
                                let val = self.get_reg8(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.rotate_left(count);
                                self.set_reg8(rm, result);
                                log::debug!("ROL r8[{}] count={}", rm, count);
                                result as u32
                            }
                        }
                        1 => {
                            // ROR - Rotate Right
                            if is_word {
                                let val = self.get_reg16(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.rotate_right(count);
                                self.set_reg16(rm, result);
                                log::debug!("ROR r16[{}] count={}", rm, count);
                                result as u32
                            } else {
                                let val = self.get_reg8(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.rotate_right(count);
                                self.set_reg8(rm, result);
                                log::debug!("ROR r8[{}] count={}", rm, count);
                                result as u32
                            }
                        }
                        2 => {
                            // RCL - Rotate Through Carry Left (simplified: treat as ROL)
                            if is_word {
                                let val = self.get_reg16(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.rotate_left(count);
                                self.set_reg16(rm, result);
                                log::debug!("RCL r16[{}] count={} (simplified)", rm, count);
                                result as u32
                            } else {
                                let val = self.get_reg8(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.rotate_left(count);
                                self.set_reg8(rm, result);
                                log::debug!("RCL r8[{}] count={} (simplified)", rm, count);
                                result as u32
                            }
                        }
                        3 => {
                            // RCR - Rotate Through Carry Right (simplified: treat as ROR)
                            if is_word {
                                let val = self.get_reg16(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.rotate_right(count);
                                self.set_reg16(rm, result);
                                log::debug!("RCR r16[{}] count={} (simplified)", rm, count);
                                result as u32
                            } else {
                                let val = self.get_reg8(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.rotate_right(count);
                                self.set_reg8(rm, result);
                                log::debug!("RCR r8[{}] count={} (simplified)", rm, count);
                                result as u32
                            }
                        }
                        4 | 6 => {
                            // SHL/SAL - Shift Logical/Arithmetic Left
                            if is_word {
                                let val = self.get_reg16(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.wrapping_shl(count as u32);
                                self.set_reg16(rm, result);
                                log::debug!("SHL r16[{}] count={}", rm, count);
                                result as u32
                            } else {
                                let val = self.get_reg8(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.wrapping_shl(count as u32);
                                self.set_reg8(rm, result);
                                log::debug!("SHL r8[{}] count={}", rm, count);
                                result as u32
                            }
                        }
                        5 => {
                            // SHR - Shift Logical Right
                            if is_word {
                                let val = self.get_reg16(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.wrapping_shr(count as u32);
                                self.set_reg16(rm, result);
                                log::debug!("SHR r16[{}] count={}", rm, count);
                                result as u32
                            } else {
                                let val = self.get_reg8(rm);
                                let count = (imm & 0x1F) as u32;
                                let result = val.wrapping_shr(count as u32);
                                self.set_reg8(rm, result);
                                log::debug!("SHR r8[{}] count={}", rm, count);
                                result as u32
                            }
                        }
                        7 => {
                            // SAR - Shift Arithmetic Right (simplified: treat as SHR for now)
                            if is_word {
                                let val = self.get_reg16(rm) as i16;
                                let count = (imm & 0x1F) as u32;
                                let result = val.wrapping_shr(count as u32) as u16;
                                self.set_reg16(rm, result);
                                log::debug!("SAR r16[{}] count={} (simplified)", rm, count);
                                result as u32
                            } else {
                                let val = self.get_reg8(rm) as i8;
                                let count = (imm & 0x1F) as u32;
                                let result = val.wrapping_shr(count as u32) as u8;
                                self.set_reg8(rm, result);
                                log::debug!("SAR r8[{}] count={} (simplified)", rm, count);
                                result as u32
                            }
                        }
                        _ => {
                            log::warn!("Unknown Group 2 operation reg={}", reg);
                            0
                        }
                    };

                    // Set zero flag based on result
                    if _result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }
                } else {
                    // Memory addressing mode - implement for Group 2
                    let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;

                    let _result = if is_word {
                        let val = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                        let result = match reg {
                            0 => val.rotate_left((imm & 0x1F) as u32),      // ROL
                            1 => val.rotate_right((imm & 0x1F) as u32),     // ROR
                            2 => val.rotate_left((imm & 0x1F) as u32),      // RCL (simplified)
                            3 => val.rotate_right((imm & 0x1F) as u32),     // RCR (simplified)
                            4 | 6 => val.wrapping_shl((imm & 0x1F) as u32), // SHL/SAL
                            5 => val.wrapping_shr((imm & 0x1F) as u32),     // SHR
                            7 => (val as i16).wrapping_shr((imm & 0x1F) as u32) as u16, // SAR
                            _ => val,
                        };
                        self.regs
                            .write_mem_word(mmu, self.regs.ds, mem_addr, result)?;
                        log::debug!(
                            "Group 2 mem[{:04X}] op={} result={:04X}",
                            mem_addr,
                            reg,
                            result
                        );
                        result as u32
                    } else {
                        let val = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                        let result = match reg {
                            0 => val.rotate_left((imm & 0x1F) as u32),      // ROL
                            1 => val.rotate_right((imm & 0x1F) as u32),     // ROR
                            2 => val.rotate_left((imm & 0x1F) as u32),      // RCL (simplified)
                            3 => val.rotate_right((imm & 0x1F) as u32),     // RCR (simplified)
                            4 | 6 => val.wrapping_shl((imm & 0x1F) as u32), // SHL/SAL
                            5 => val.wrapping_shr((imm & 0x1F) as u32),     // SHR
                            7 => (val as i8).wrapping_shr((imm & 0x1F) as u32) as u8, // SAR
                            _ => val,
                        };
                        self.regs
                            .write_mem_byte(mmu, self.regs.ds, mem_addr, result)?;
                        log::debug!(
                            "Group 2 mem[{:04X}] op={} result={:02X}",
                            mem_addr,
                            reg,
                            result
                        );
                        result as u32
                    };

                    // Set zero flag based on result
                    if _result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }
                }

                Ok(RealModeStep::Continue)
            }

            // Group 2 - rotate/shift with CL count (D2 /r)
            0xD2 => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = (modrm >> 3) & 7;
                let rm = (modrm & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let cl = (self.regs.ecx & 0xFF) as u8;

                // For register-direct mode only for now
                if mod_val == 3 {
                    let _result = match reg {
                        0 => {
                            // ROL - Rotate Left by CL
                            let val = self.get_reg8(rm);
                            let count = (cl & 0x1F) as u32;
                            let result = val.rotate_left(count);
                            self.set_reg8(rm, result);
                            log::debug!("ROL r8[{}] count=CL({})", rm, cl);
                            result as u32
                        }
                        1 => {
                            // ROR - Rotate Right by CL
                            let val = self.get_reg8(rm);
                            let count = (cl & 0x1F) as u32;
                            let result = val.rotate_right(count);
                            self.set_reg8(rm, result);
                            log::debug!("ROR r8[{}] count=CL({})", rm, cl);
                            result as u32
                        }
                        2 => {
                            // RCL - Rotate Through Carry Left by CL (simplified)
                            let val = self.get_reg8(rm);
                            let count = (cl & 0x1F) as u32;
                            let result = val.rotate_left(count);
                            self.set_reg8(rm, result);
                            log::debug!("RCL r8[{}] count=CL({}) (simplified)", rm, cl);
                            result as u32
                        }
                        3 => {
                            // RCR - Rotate Through Carry Right by CL (simplified)
                            let val = self.get_reg8(rm);
                            let count = (cl & 0x1F) as u32;
                            let result = val.rotate_right(count);
                            self.set_reg8(rm, result);
                            log::debug!("RCR r8[{}] count=CL({}) (simplified)", rm, cl);
                            result as u32
                        }
                        4 | 6 => {
                            // SHL/SAL - Shift Logical/Arithmetic Left by CL
                            let val = self.get_reg8(rm);
                            let count = (cl & 0x1F) as u32;
                            let result = val.wrapping_shl(count as u32);
                            self.set_reg8(rm, result);
                            log::debug!("SHL r8[{}] count=CL({})", rm, cl);
                            result as u32
                        }
                        5 => {
                            // SHR - Shift Logical Right by CL
                            let val = self.get_reg8(rm);
                            let count = (cl & 0x1F) as u32;
                            let result = val.wrapping_shr(count as u32);
                            self.set_reg8(rm, result);
                            log::debug!("SHR r8[{}] count=CL({})", rm, cl);
                            result as u32
                        }
                        7 => {
                            // SAR - Shift Arithmetic Right by CL (simplified)
                            let val = self.get_reg8(rm) as i8;
                            let count = (cl & 0x1F) as u32;
                            let result = val.wrapping_shr(count as u32) as u8;
                            self.set_reg8(rm, result);
                            log::debug!("SAR r8[{}] count=CL({}) (simplified)", rm, cl);
                            result as u32
                        }
                        _ => {
                            log::warn!("Unknown Group 2 operation reg={}", reg);
                            0
                        }
                    };

                    // Set zero flag based on result
                    if _result == 0 {
                        self.regs.eflags |= 0x40; // ZF
                    } else {
                        self.regs.eflags &= !0x40;
                    }
                } else {
                    log::warn!(
                        "Group 2 memory addressing - treating as NOP (mod={}, rm={})",
                        mod_val,
                        rm
                    );
                }

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

                if reg == 0 {
                    // Only MOV is valid for C6
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
                                log::debug!(
                                    "MOV [BX+SI], imm8 (addr={:04X}, imm={:02X})",
                                    addr,
                                    imm
                                );
                            }
                            Err(e) => {
                                log::error!(
                                    "Failed to write to memory at DS:{:04X}: {:?}",
                                    addr,
                                    e
                                );
                            }
                        }
                    } else if mod_val == 1 && rm == 4 {
                        // SIB addressing mode with 8-bit displacement (protected mode)
                        let sib = self.fetch_byte(mmu)?;
                        let scale = 1 << ((sib >> 6) & 3);
                        let index = (sib >> 3) & 7;
                        let base = sib & 7;

                        // Get base register value
                        let base_val = match base {
                            0 => self.regs.eax,
                            1 => self.regs.ecx,
                            2 => self.regs.edx,
                            3 => self.regs.ebx,
                            4 => {
                                // SIB with no base register - use disp32 as address
                                let disp32 = self.fetch_dword(mmu)?;
                                match self.regs.write_mem_byte(
                                    mmu,
                                    self.regs.ds,
                                    (disp32 & 0xFFFF) as u16,
                                    imm,
                                ) {
                                    Ok(_) => {
                                        log::debug!(
                                            "MOV [disp32], imm8 (addr={:08X}, imm={:02X})",
                                            disp32,
                                            imm
                                        );
                                    }
                                    Err(e) => {
                                        log::error!(
                                            "Failed to write to memory at {:08X}: {:?}",
                                            disp32,
                                            e
                                        );
                                    }
                                }
                                return Ok(RealModeStep::Continue);
                            }
                            5 => self.regs.ebp,
                            6 => self.regs.esi,
                            7 => self.regs.edi,
                            _ => unreachable!(),
                        };

                        // Get index register value
                        let index_val = if index == 4 {
                            // No index register
                            0
                        } else {
                            match index {
                                0 => self.regs.eax,
                                1 => self.regs.ecx,
                                2 => self.regs.edx,
                                3 => self.regs.ebx,
                                5 => self.regs.ebp,
                                6 => self.regs.esi,
                                7 => self.regs.edi,
                                _ => unreachable!(),
                            }
                        };

                        // Get 8-bit displacement
                        let disp8 = self.fetch_byte(mmu)? as i8;

                        // Calculate effective address
                        let mut addr = (base_val as i64
                            + (index_val as i64 * scale as i64)
                            + disp8 as i64) as u32;

                        // In protected mode, we need to use segment:offset addressing
                        // For now, use DS as the segment
                        let offset = addr & 0xFFFF;
                        match self
                            .regs
                            .write_mem_byte(mmu, self.regs.ds, offset as u16, imm)
                        {
                            Ok(_) => {
                                log::debug!(
                                    "MOV [SIB], imm8 (addr={:08X}, imm={:02X}, base={}, index={}, scale={}, disp={})",
                                    addr,
                                    imm,
                                    base,
                                    index,
                                    scale,
                                    disp8
                                );
                            }
                            Err(e) => {
                                log::error!("Failed to write to memory at {:08X}: {:?}", addr, e);
                            }
                        }
                    } else if mod_val == 2 && rm == 4 {
                        // SIB addressing mode with 32-bit displacement (protected mode)
                        let sib = self.fetch_byte(mmu)?;
                        let scale = 1 << ((sib >> 6) & 3);
                        let index = (sib >> 3) & 7;
                        let base = sib & 7;

                        // Get base register value
                        let base_val = match base {
                            0 => self.regs.eax,
                            1 => self.regs.ecx,
                            2 => self.regs.edx,
                            3 => self.regs.ebx,
                            4 => {
                                // SIB with no base register - use disp32 as address
                                let disp32 = self.fetch_dword(mmu)?;
                                match self.regs.write_mem_byte(
                                    mmu,
                                    self.regs.ds,
                                    (disp32 & 0xFFFF) as u16,
                                    imm,
                                ) {
                                    Ok(_) => {
                                        log::debug!(
                                            "MOV [disp32], imm8 (addr={:08X}, imm={:02X})",
                                            disp32,
                                            imm
                                        );
                                    }
                                    Err(e) => {
                                        log::error!(
                                            "Failed to write to memory at {:08X}: {:?}",
                                            disp32,
                                            e
                                        );
                                    }
                                }
                                return Ok(RealModeStep::Continue);
                            }
                            5 => self.regs.ebp,
                            6 => self.regs.esi,
                            7 => self.regs.edi,
                            _ => unreachable!(),
                        };

                        // Get index register value
                        let index_val = if index == 4 {
                            // No index register
                            0
                        } else {
                            match index {
                                0 => self.regs.eax,
                                1 => self.regs.ecx,
                                2 => self.regs.edx,
                                3 => self.regs.ebx,
                                5 => self.regs.ebp,
                                6 => self.regs.esi,
                                7 => self.regs.edi,
                                _ => unreachable!(),
                            }
                        };

                        // Get 32-bit displacement
                        let disp32 = self.fetch_dword(mmu)?;

                        // Calculate effective address
                        let addr = (base_val as i64
                            + (index_val as i64 * scale as i64)
                            + disp32 as i64) as u32;

                        // In protected mode, we need to use segment:offset addressing
                        // For now, use DS as the segment
                        let offset = addr & 0xFFFF;
                        match self
                            .regs
                            .write_mem_byte(mmu, self.regs.ds, offset as u16, imm)
                        {
                            Ok(_) => {
                                log::debug!(
                                    "MOV [SIB+disp32], imm8 (addr={:08X}, imm={:02X}, base={}, index={}, scale={}, disp={:08X})",
                                    addr,
                                    imm,
                                    base,
                                    index,
                                    scale,
                                    disp32
                                );
                            }
                            Err(e) => {
                                log::error!("Failed to write to memory at {:08X}: {:?}", addr, e);
                            }
                        }
                     } else {
                        // Unsupported addressing mode for 8-bit MOV
                        return Err(VmError::Execution(ExecutionError::InvalidOpcode {
                            message: format!("Unsupported addressing mode in MOV r/m8, imm8: modrm={:02X}", modrm),
                            pc: self.get_pc_linear(),
                        }));
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

                if reg == 0 {
                    // Only MOV is valid for C7
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
                                log::debug!(
                                    "MOV [BX+SI], imm16 (addr={:04X}, imm={:04X})",
                                    addr,
                                    imm
                                );
                            }
                            Err(e) => {
                                log::error!(
                                    "Failed to write to memory at DS:{:04X}: {:?}",
                                    addr,
                                    e
                                );
                            }
                        }
                    } else if mod_val == 0 && rm == 4 {
                        // [SI] addressing mode
                        let si = (self.regs.esi & 0xFFFF) as u16;
                        match self.regs.write_mem_word(mmu, self.regs.ds, si, imm) {
                            Ok(_) => {
                                log::debug!("MOV [SI], imm16 (SI={:04X}, imm={:04X})", si, imm);
                            }
                            Err(e) => {
                                log::error!("Failed to write to memory at DS:{:04X}: {:?}", si, e);
                            }
                        }
                    } else if mod_val == 0 && rm == 5 {
                        // [DI] addressing mode
                        let di = (self.regs.edi & 0xFFFF) as u16;
                        match self.regs.write_mem_word(mmu, self.regs.ds, di, imm) {
                            Ok(_) => {
                                log::debug!("MOV [DI], imm16 (DI={:04X}, imm={:04X})", di, imm);
                            }
                            Err(e) => {
                                log::error!("Failed to write to memory at DS:{:04X}: {:?}", di, e);
                            }
                        }
                    } else if mod_val == 1 && rm == 4 {
                        // [BX+SI+disp8] addressing mode
                        let disp8 = self.fetch_byte(mmu)? as i8;
                        let bx = (self.regs.ebx & 0xFFFF) as u16;
                        let si = (self.regs.esi & 0xFFFF) as u16;
                        let addr = (bx as i32)
                            .wrapping_add(si as i32)
                            .wrapping_add(disp8 as i32) as u16;
                        match self.regs.write_mem_word(mmu, self.regs.ds, addr, imm) {
                            Ok(_) => {
                                log::debug!(
                                    "MOV [BX+SI+{:02X}], imm16 (addr={:04X}, imm={:04X})",
                                    disp8,
                                    addr,
                                    imm
                                );
                            }
                            Err(e) => {
                                log::error!(
                                    "Failed to write to memory at DS:{:04X}: {:?}",
                                    addr,
                                    e
                                );
                            }
                        }
                    } else {
                        // Other memory addressing modes - try to handle gracefully
                        log::debug!(
                            "MOV [mem], imm16 (modrm={:02X}, imm={:04X}) - addressing mode mod={} rm={} not fully implemented, treating as NOP",
                            modrm,
                            imm,
                            mod_val,
                            rm
                        );
                        // Don't fail - allow boot to continue
                    }
                } else {
                    log::warn!(
                        "Invalid C7 extension (reg={}) - treating as NOP for boot compatibility",
                        reg
                    );
                    // Don't fail - allow boot to continue
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
                    log::debug!(
                        "XCHG r8[{}], r8[{}] (swapped {:02X} <-> {:02X})",
                        reg,
                        rm,
                        reg_val,
                        rm_val
                    );
                 } else {
                    // Memory-to-register exchange (memory-register exchange)
                    let mem_val = self.read_mem_byte(mmu, self.regs.ds, rm as u16)?;
                    let reg_val = self.get_reg8(reg);
                    
                    // Store reg value in memory
                    self.write_mem_byte(mmu, self.regs.ds, rm as u16, reg_val)?;
                    
                    // Load memory value into register
                    self.set_reg8(reg, mem_val);
                    
                    log::debug!("XCHG [mem], r8[{}] - DS:[{:04X}] <-> r8[{}] (reg={:02X} <-> mem={:02X})", 
                        rm, reg, mem_val, reg);
                    
                    Ok(RealModeStep::Continue)
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
                    log::debug!(
                        "XCHG r16[{}], r16[{}] (swapped {:04X} <-> {:04X})",
                        reg,
                        rm,
                        reg_val,
                        rm_val
                    );
                 } else {
                    // Memory-to-register exchange
                    let mem_val = self.read_mem_word(mmu, self.regs.ds, rm as u16)?;
                    let reg_val = self.get_reg16(reg);
                    
                    // Store register value in memory
                    self.regs.write_mem_word(mmu, self.regs.ds, rm as u16, reg_val)?;
                    
                    // Load memory value into register
                    self.set_reg16(reg, mem_val);
                    
                    log::debug!("XCHG [mem], r16[{}] DS:[{:04X}] <-> r16[{}] ({:04X})",
                        reg, rm, mem_val, reg_val);
                    
                    Ok(RealModeStep::Continue)
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
                    0x88 => {
                        // MOV r/m8, r8
                        let src = self.get_reg8(reg);
                        if mod_val == 3 {
                            // Register to register
                            self.set_reg8(rm, src);
                        } else {
                            // Register to memory - calculate effective address and write
                            let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                            self.regs.write_mem_byte(mmu, self.regs.ds, mem_addr, src)?;
                            log::debug!("MOV [mem], r8 addr={:04X} src={:02X}", mem_addr, src);
                        }
                    }
                    0x89 => {
                        // MOV r/m16, r16
                        let src = self.get_reg16(reg);
                        if mod_val == 3 {
                            // Register to register
                            self.set_reg16(rm, src);
                        } else {
                            // Register to memory - calculate effective address and write
                            let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                            self.regs.write_mem_word(mmu, self.regs.ds, mem_addr, src)?;
                            log::debug!("MOV [mem], r16 addr={:04X} src={:04X}", mem_addr, src);
                        }
                    }
                    0x8A => {
                        // MOV r8, r/m8
                        if mod_val == 3 {
                            // Register to register
                            let src = self.get_reg8(rm);
                            self.set_reg8(reg, src);
                        } else {
                            // Memory to register - calculate effective address and read
                            let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                            let val = self.regs.read_mem_byte(mmu, self.regs.ds, mem_addr)?;
                            self.set_reg8(reg, val);
                            log::debug!("MOV r8, [mem] addr={:04X} dst={:02X}", mem_addr, val);
                        }
                    }
                    0x8B => {
                        // MOV r16, r/m16
                        if mod_val == 3 {
                            // Register to register
                            let src = self.get_reg16(rm);
                            self.set_reg16(reg, src);
                        } else {
                            // Memory to register - calculate effective address and read
                            let mem_addr = self.calc_effective_address(mmu, mod_val, rm)?;
                            let val = self.regs.read_mem_word(mmu, self.regs.ds, mem_addr)?;
                            self.set_reg16(reg, val);
                            log::debug!("MOV r16, [mem] addr={:04X} dst={:04X}", mem_addr, val);
                        }
                    }
                    _ => unreachable!(),
                }
                Ok(RealModeStep::Continue)
            }

            // MOV acc, moffs (A0-A3)
            0xA0 => {
                // MOV AL, moffs8
                let addr = self.fetch_word(mmu)? as u32;
                let val = self.regs.read_mem_byte(mmu, self.regs.ds, addr as u16)?;
                self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (val as u32);
                Ok(RealModeStep::Continue)
            }
            0xA1 => {
                // MOV AX, moffs16
                let addr = self.fetch_word(mmu)? as u32;
                let val = self.regs.read_mem_word(mmu, self.regs.ds, addr as u16)?;
                self.regs.eax = (self.regs.eax & 0xFFFF0000) | (val as u32);
                Ok(RealModeStep::Continue)
            }
            0xA2 => {
                // MOV moffs8, AL
                let addr = self.fetch_word(mmu)? as u32;
                self.regs.write_mem_byte(
                    mmu,
                    self.regs.ds,
                    addr as u16,
                    (self.regs.eax & 0xFF) as u8,
                )?;
                Ok(RealModeStep::Continue)
            }
            0xA3 => {
                // MOV moffs16, AX
                let addr = self.fetch_word(mmu)? as u32;
                self.regs.write_mem_word(
                    mmu,
                    self.regs.ds,
                    addr as u16,
                    (self.regs.eax & 0xFFFF) as u16,
                )?;
                Ok(RealModeStep::Continue)
            }

            // MOV seg, r/m (8E)
            0x8E => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let val = self.get_reg16(reg);
                match reg {
                    0 => self.regs.es = val,
                    1 => {
                        // Log CS changes in protected mode
                        if self.mode_trans.current_mode() == X86Mode::Protected {
                            log::warn!("========================================");
                            log::warn!("MOV to CS in PROTECTED MODE");
                            log::warn!("========================================");
                            log::warn!("CS changing from {:04X} to {:04X}", self.regs.cs, val);
                            log::warn!("Current IP: {:08X}", self.regs.eip);
                            log::warn!("Current mode: {:?}", self.mode_trans.current_mode());
                            log::warn!("========================================");
                        }
                        self.regs.cs = val;
                    }
                    2 => self.regs.ss = val,
                    3 => self.regs.ds = val,
                    _ => log::warn!("Invalid segment register: {}", reg),
                }
                Ok(RealModeStep::Continue)
            }

            // MOV r/m16, Sreg (8C) - Move segment register to memory/register
            0x8C => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                // Get segment register value
                let val = match reg {
                    0 => self.regs.es,
                    1 => self.regs.cs,
                    2 => self.regs.ss,
                    3 => self.regs.ds,
                    _ => {
                        log::warn!("Invalid segment register in MOV r/m16, Sreg: {}", reg);
                        return Ok(RealModeStep::Continue);
                    }
                };

                if mod_val == 3 {
                    // Register to register
                    self.set_reg16(rm, val);
                    log::warn!("MOV r16[{}], Sreg[{}] (val={:04X})", rm, reg, val);
                } else {
                    // Register to memory
                    let mem_addr = self.calculate_effective_address(mmu, mod_val as usize, rm)?;

                    self.regs.write_mem_word(mmu, self.regs.ds, mem_addr, val)?;
                    log::warn!(
                        "MOV [mem], Sreg[{}] addr={:04X} (val={:04X})",
                        reg,
                        mem_addr,
                        val
                    );
                }

                Ok(RealModeStep::Continue)
            }

            // POP r/m16 (8F /0) - Pop from stack to memory/register
            0x8F => {
                let modrm = self.fetch_byte(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;
                let mod_val = (modrm >> 6) & 3;
                let rm = (modrm & 7) as usize;

                // Only /0 (POP) is valid in real mode
                if reg != 0 {
                    log::warn!("POP r/m16 - invalid reg field (must be 0): {}", reg);
                    return Ok(RealModeStep::Continue);
                }

                // Pop value from stack
                let val = self.pop16(mmu)?;

                if mod_val == 3 {
                    // Register to register
                    self.set_reg16(rm, val);
                    log::warn!("POP r16[{}] (val={:04X})", rm, val);
                } else {
                    // Stack to memory with full ModR/M addressing
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
                        (1, 6) => self.fetch_word_signed(mmu)? as u16,
                        (1, 0) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            let disp = self.fetch_byte_signed(mmu)? as i16;
                            bx.wrapping_add(si).wrapping_add(disp as u16)
                        }
                        (1, 1) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            let disp = self.fetch_byte_signed(mmu)? as i16;
                            bx.wrapping_add(di).wrapping_add(disp as u16)
                        }
                        (1, 2) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            let disp = self.fetch_byte_signed(mmu)? as i16;
                            bp.wrapping_add(si).wrapping_add(disp as u16)
                        }
                        (1, 3) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            let disp = self.fetch_byte_signed(mmu)? as i16;
                            bp.wrapping_add(di).wrapping_add(disp as u16)
                        }
                        (1, 4) => {
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            let disp = self.fetch_byte_signed(mmu)? as i16;
                            si.wrapping_add(disp as u16)
                        }
                        (1, 5) => {
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            let disp = self.fetch_byte_signed(mmu)? as i16;
                            di.wrapping_add(disp as u16)
                        }
                        (1, 7) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let disp = self.fetch_byte_signed(mmu)? as i16;
                            bx.wrapping_add(disp as u16)
                        }
                        (2, 6) => self.fetch_word_signed(mmu)? as u16,
                        (2, 0) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            let disp = self.fetch_word_signed(mmu)?;
                            bx.wrapping_add(si).wrapping_add(disp as u16)
                        }
                        (2, 1) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            let disp = self.fetch_word_signed(mmu)?;
                            bx.wrapping_add(di).wrapping_add(disp as u16)
                        }
                        (2, 2) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            let disp = self.fetch_word_signed(mmu)?;
                            bp.wrapping_add(si).wrapping_add(disp as u16)
                        }
                        (2, 3) => {
                            let bp = (self.regs.ebp & 0xFFFF) as u16;
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            let disp = self.fetch_word_signed(mmu)?;
                            bp.wrapping_add(di).wrapping_add(disp as u16)
                        }
                        (2, 4) => {
                            let si = (self.regs.esi & 0xFFFF) as u16;
                            let disp = self.fetch_word_signed(mmu)?;
                            si.wrapping_add(disp as u16)
                        }
                        (2, 5) => {
                            let di = (self.regs.edi & 0xFFFF) as u16;
                            let disp = self.fetch_word_signed(mmu)?;
                            di.wrapping_add(disp as u16)
                        }
                        (2, 7) => {
                            let bx = (self.regs.ebx & 0xFFFF) as u16;
                            let disp = self.fetch_word_signed(mmu)?;
                            bx.wrapping_add(disp as u16)
                        }
                        _ => {
                            log::warn!(
                                "POP r/m16 - unsupported addressing mode mod={}, rm={}",
                                mod_val,
                                rm
                            );
                            return Ok(RealModeStep::Continue);
                        }
                    };

                    self.regs.write_mem_word(mmu, self.regs.ss, mem_addr, val)?;
                    log::warn!("POP [mem] addr={:04X} (val={:04X})", mem_addr, val);
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

            // BOUND (62) - Array Index Bounds Check
            0x62 => {
                let modrm = self.fetch_byte(mmu)?;
                log::debug!(
                    "BOUND r16, r/m16 - bounds checking (treating as NOP, modrm={:02X})",
                    modrm
                );
                // BOUND checks if a register is within bounds specified by memory
                // For now, we treat it as NOP since it's rarely used in modern code
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

            // INSB (6C) / INSW (6D) - Input from port to string
            0x6C | 0x6D => {
                let is_word = opcode == 0x6D;
                let di = self.get_reg16(7); // DI

                // Read from port DX (default port for string I/O)
                let dx = (self.regs.edx & 0xFFFF) as u16;

                if is_word {
                    // INSW - Input word from port DX
                    let val = self.port_read_word(mmu, dx)?;
                    self.regs.write_mem_word(mmu, self.regs.es, di, val)?;
                    self.set_reg16(7, di.wrapping_add(2));
                    log::warn!("INSW DX={:04X}, ES:[DI]={:04X}, val={:04X}", dx, di, val);
                } else {
                    // INSB - Input byte from port DX
                    let val = self.port_read_byte(mmu, dx)?;
                    self.regs.write_mem_byte(mmu, self.regs.es, di, val)?;
                    self.set_reg16(7, di.wrapping_add(1));
                    log::warn!("INSB DX={:04X}, ES:[DI]={:04X}, val={:02X}", dx, di, val);
                }
                Ok(RealModeStep::Continue)
            }

            // OUTSB (6E) / OUTSW (6F) - Output string to port
            0x6E | 0x6F => {
                let is_word = opcode == 0x6F;
                let si = self.get_reg16(6); // SI

                // Write to port DX (default port for string I/O)
                let dx = (self.regs.edx & 0xFFFF) as u16;

                if is_word {
                    // OUTSW - Output word to port DX
                    let val = self.regs.read_mem_word(mmu, self.regs.ds, si)?;
                    self.port_write_word(mmu, dx, val)?;
                    self.set_reg16(6, si.wrapping_add(2));

                    // Track VGA operations
                    let vga_completed = self.vga.record_outsw(dx);

                    // Reduce log frequency after first 100 operations
                    if self.vga.outsw_count <= 100
                        || self.vga.outsw_count % 10000 == 0
                        || vga_completed
                    {
                        log::info!(
                            "OUTSW #{:05} DS:[SI]={:04X}, DX={:04X}, val={:04X} {}",
                            self.vga.outsw_count,
                            si,
                            dx,
                            val,
                            if vga_completed {
                                "[VGA INIT COMPLETE]"
                            } else {
                                ""
                            }
                        );
                    }
                } else {
                    // OUTSB - Output byte to port DX
                    let val = self.regs.read_mem_byte(mmu, self.regs.ds, si)?;
                    self.port_write_byte(mmu, dx, val)?;
                    self.set_reg16(6, si.wrapping_add(1));
                    log::warn!("OUTSB DS:[SI]={:04X}, DX={:04X}, val={:02X}", si, dx, val);
                }
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
                        if let Some(step) =
                            self.mode_trans.check_mode_switch(&mut self.regs, mmu)?
                        {
                            return Ok(step);
                        }

                        Ok(RealModeStep::Continue)
                    }

                    // LGDT/LIDT (0F 01 /2 and /3)
                 0x02 => {
                    // Memory mode - perform 8-bit AND operation between r/m8 and /r8
                    let modrm = self.fetch_byte(mmu)?;
                    
                    // Check addressing mode from modrm value
                    let is_mem_mode = ((modrm & 0x38) != 0);
                    
                    if is_mem_mode {
                        // Memory mode - /r8 is source, /r8 is destination
                        let dst = self.regs.esi;
                        let src = self.regs.edi;
                        
                        // Read source value from memory at /r8 address
                        let addr = self.seg_to_linear(self.regs.ds, self.regs.esi as u16);
                        let mem_val = self.read_mem_byte(mmu, self.regs.ds, addr)?;
                        
                        // Perform AND operation
                        let result = src.wrapping_sub(mem_val);
                        
                        // Store result in /r8 destination
                        self.set_reg8(dst, result);
                        
                        // Update flags
                        self.update_flags_zsp8(result);
                        
                        log::debug!("AND r8[{}], /r8[{}] ({:02X} & {:02X}) = {:02X}", 
                            dst, src, mem_val, result);
                        
                        Ok(RealModeStep::Continue)
                    } else {
                        // Register mode - /r8 is source, /r8 is destination
                        let dst = self.regs.esi;
                        let src = self.regs.edi;
                        
                        // Read source value from memory at /r8 address
                        let addr = self.seg_to_linear(self.regs.ds, self.regs.esi as u16);
                        let mem_val = self.read_mem_byte(mmu, self.regs.ds, addr)?;
                        
                        // Perform AND operation
                        let result = src.wrapping_sub(mem_val);
                        
                        // Store result in /r8 destination
                        self.set_reg8(dst, result);
                        
                        // Update flags
                        self.update_flags_zsp8(result);
                        
                        log::debug!("AND r8[{}], /r8[{}] ({:02X} - {:02X}) = {:02X}", 
                            dst, src, mem_val, result);
                        
                        Ok(RealModeStep::Continue)
                    }
                }
                            3 => {
                                // LIDT - Load Interrupt Descriptor Table
                                // Format: LIDT m - loads 6 bytes: limit (16-bit) and base (32-bit)
                                log::info!(
                                    "LIDT instruction encountered (modrm={:02X}, mod={}, rm={})",
                                    modrm,
                                    mod_val,
                                    rm
                                );

                                // Calculate effective address based on addressing mode
                                let mem_addr: u32 = if mod_val == 0 && rm == 6 {
                                    // [disp16] - 16-bit displacement follows ModRM
                                    let disp16 = self.fetch_word(mmu)? as u32;
                                    log::debug!("LIDT [disp16] - disp16={:04X}", disp16);
                                    disp16
                                } else if mod_val == 0 {
                                    // [mem] addressing - use register-based addressing
                                    // For simplicity, handle common cases
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    let addr = bx.wrapping_add(si) as u32;
                                    log::debug!("LIDT [mem] - addr={:04X}", addr);
                                    addr
                                } else if mod_val == 2 {
                                    // [mem+disp16] - displacement follows ModRM
                                    let disp16 = self.fetch_word(mmu)? as u32;
                                    let bx = (self.regs.ebx & 0xFFFF) as u16;
                                    let si = (self.regs.esi & 0xFFFF) as u16;
                                    let addr =
                                        bx.wrapping_add(si).wrapping_add(disp16 as u16) as u32;
                                    log::debug!("LIDT [mem+disp16] - addr={:04X}", addr);
                                    addr
                                } else {
                                    log::warn!(
                                        "LIDT with unsupported addressing mode (mod={}, rm={})",
                                        mod_val,
                                        rm
                                    );
                                    // For now, don't block execution
                                    return Ok(RealModeStep::Continue);
                                };

                                // Read 6-byte IDT descriptor from memory:
                                // Bytes 0-1: Limit (16-bit)
                                // Bytes 2-5: Base address (32-bit)
                                let limit =
                                    self.regs
                                        .read_mem_word(mmu, self.regs.ds, mem_addr as u16)?;
                                let base_low = self.regs.read_mem_word(
                                    mmu,
                                    self.regs.ds,
                                    (mem_addr + 2) as u16,
                                )? as u32;
                                let base_high = self.regs.read_mem_word(
                                    mmu,
                                    self.regs.ds,
                                    (mem_addr + 4) as u16,
                                )? as u32;
                                let base = (base_high << 16) | base_low;

                                log::info!(
                                    "LIDT: Loading IDTR with base={:#010X}, limit={:#06X}",
                                    base,
                                    limit
                                );

                                // Load IDTR
                                use super::mode_trans::IdtPointer;
                                let idtr = IdtPointer { limit, base };
                                self.mode_trans.load_idtr(idtr)?;

                                log::info!("LIDT: IDTR loaded successfully");

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
                            log::debug!(
                                "RDMSR EFER: EAX={:#010X}, EDX={:#010X}",
                                self.regs.eax,
                                self.regs.edx
                            );
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
                            if let Some(step) =
                                self.mode_trans.check_mode_switch(&mut self.regs, mmu)?
                            {
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
                        
                        // Read 16-bit source value from memory
                        let src_val = self.read_mem_word(mmu, self.regs.ds, self.regs.ebx as u16)?;
                        
                        // Sign-extend to 32-bit
                        let src_extended = (src_val as i16) as i32 as u32;
                        
                        // Store in 8-bit destination register (sign-extended)
                        self.set_reg8(reg, (src_extended & 0xFF) as u8);
                        
                        // Update flags (zero, sign, parity, overflow)
                        let result = src_extended & 0xFF;
                        if result == 0 {
                            self.regs.eflags |= 0x40; // ZF
                        } else {
                            self.regs.eflags &= !0x40;
                        }
                        
                        if (src_extended as i32) < 0 {
                            self.regs.eflags |= 0x80; // SF
                        } else {
                            self.regs.eflags &= !0x80;
                        }
                        
                        let parity = (src_extended as u8).count_ones() % 2 == 0;
                        if parity {
                            self.regs.eflags |= 0x04; // PF
                        } else {
                            self.regs.eflags &= !0x04;
                        }
                        
                        // Overflow flag for sign extension
                        if (src_val as i16) < 0 && (src_extended as i32) < 0 {
                            self.regs.eflags |= 0x800; // OF
                        } else {
                            self.regs.eflags &= !0x800;
                        }
                        
                        log::debug!("MOVSX r8[{}], DS:[{:04X}] (sign-ext {:08X})", reg, src_val, src_extended);
                        Ok(RealModeStep::Continue)
                    }

                    // MOVSX r32, r/m8 (0F BE /r) with 0x66 prefix
                    // Handled by 0x66 prefix processing

                     // MOVZX r16, r/m8 (0F B6 /r) - Move with Zero-Extension
                     0xB6 => {
                        let modrm = self.fetch_byte(mmu)?;
                        let reg = ((modrm >> 3) & 7) as usize;
                        let _rm = (modrm & 7) as usize;
                        
                        // Read 16-bit source value from memory
                        let src_val = self.read_mem_word(mmu, self.regs.ds, self.regs.ebx as u16)?;
                        
                        // Zero-extend to 32-bit
                        let src_extended = src_val as u32;
                        
                        // Store in 8-bit destination register (zero-extended)
                        self.set_reg8(reg, src_extended as u8);
                        
                        // Update flags (zero, sign, parity)
                        let result = src_extended & 0xFF;
                        if result == 0 {
                            self.regs.eflags |= 0x40; // ZF
                        } else {
                            self.regs.eflags &= !0x40;
                        }
                        
                        if (result as i8).is_negative() {
                            self.regs.eflags |= 0x80; // SF
                        } else {
                            self.regs.eflags &= !0x80;
                        }
                        
                        let parity = result.count_ones() % 2 == 0;
                        if parity {
                            self.regs.eflags |= 0x04; // PF
                        } else {
                            self.regs.eflags &= !0x04;
                        }
                        
                        // Zero-extension never sets overflow flag
                        // Sign-extended values are always positive
                        
                        log::debug!("MOVZX r8[{}], DS:[{:04X}] (zero-ext {:08X})", reg, src_val, src_extended);
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
                    // BTS (Bit Test and Set) - 0F AB
                    0xAB => {
                        let modrm = self.fetch_byte(mmu)?;
                        log::debug!("BTS instruction (modrm={:02X}) - treating as NOP", modrm);
                        // BTS is used for bit manipulation in protected mode
                        // For now, treat as NOP to allow boot to continue
                        Ok(RealModeStep::Continue)
                    }

                    // Group 8 - BT/BTS/BTR/BTC (0F BA /0 ib)
                    0xBA => {
                        let modrm = self.fetch_byte(mmu)?;
                        let _imm8 = self.fetch_byte(mmu)?;
                        let reg = (modrm >> 3) & 7;
                        log::debug!(
                            "Group 8 instruction (opcode 0F BA, reg={}) - treating as NOP",
                            reg
                        );
                        // BT/BTS/BTR/BTC are bit test and modify instructions
                        // For now, treat as NOP to allow boot to continue
                        Ok(RealModeStep::Continue)
                    }

                    // 0F 00 - Various protected mode instructions
                    0x00 => {
                        let modrm = self.fetch_byte(mmu)?;
                        let reg = (modrm >> 3) & 7;
                        log::debug!(
                            "0F 00 instruction (modrm={:02X}, reg={}) - treating as NOP",
                            modrm,
                            reg
                        );
                        // This includes SLDT, STR, LLDT, LTR, VERR, VERW, etc.
                        // For now, treat as NOP to allow boot to continue
                        Ok(RealModeStep::Continue)
                    }

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
                if reg == 2 {
                    // NOT
                    if opcode == 0xF6 {
                        // 8-bit NOT - simplified
                        log::debug!("NOT r/m8");
                    } else {
                        // 16-bit NOT
                        let val = self.get_reg16(rm);
                        self.set_reg16(rm, !val);
                    }
                } else if reg == 3 {
                    // NEG
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
                if reg <= 3 {
                    // SHL/SAL/SHR/SAR
                    if opcode == 0xD1 {
                        let mut val = self.get_reg16(rm);
                        match reg {
                            4 => {
                                // SHL
                                val <<= 1;
                            }
                            5 => {
                                // SHR
                                val >>= 1;
                            }
                            7 => {
                                // SAR
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
                let old_cs = self.regs.cs;

                // Log far jumps in protected mode
                if self.mode_trans.current_mode() == X86Mode::Protected {
                    log::warn!("========================================");
                    log::warn!("FAR JMP in PROTECTED MODE");
                    log::warn!("========================================");
                    log::warn!("CS changing from {:04X} to {:04X}", old_cs, seg);
                    log::warn!(
                        "IP changing from {:08X} to {:08X}",
                        self.regs.eip,
                        offset as u32
                    );
                    log::warn!("Current mode: {:?}", self.mode_trans.current_mode());
                    log::warn!("========================================");
                }

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
                    log::warn!(
                        "J{:02X} at CS:IP={:04X}:{:08X} rel={:02X} (cond={}) - ZF={}, CF={}",
                        opcode,
                        self.regs.cs,
                        self.regs.eip - 2,
                        rel,
                        cond_met,
                        (self.regs.eflags & 0x0040) != 0,
                        (self.regs.eflags & 0x0001) != 0
                    );
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
                if self.get_reg16(1) == 0 {
                    // CX
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

            // INT 3 (CC) - Software Breakpoint
            0xCC => {
                log::debug!("INT 3 - Software breakpoint (ignoring)");
                // INT 3 is used for debugging breakpoints
                // In our VM, we treat it as NOP since there's no debugger attached
                Ok(RealModeStep::Continue)
            }

            // LOOP/LOOPE/LOOPNE rel8 (E0, E1, E2)
            0xE0 | 0xE1 | 0xE2 => {
                let rel = self.fetch_byte(mmu)? as i8;
                let cx = self.get_reg16(1);
                let new_cx = cx.wrapping_sub(1);
                self.set_reg16(1, new_cx);

                let should_loop = match opcode {
                    0xE0 => new_cx != 0,                  // LOOPNZ
                    0xE1 => new_cx != 0 && self.get_zf(), // LOOPZ
                    0xE2 => new_cx != 0,                  // LOOP
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

            // LEAVE (C9) - High Level Procedure Exit
            0xC9 => {
                // LEAVE is equivalent to: MOV ESP, EBP; POP EBP
                // Set ESP to EBP, then pop the stack into EBP
                let old_ebp = self.regs.ebp;
                self.regs.esp = self.regs.ebp;
                self.regs.ebp = self.pop16(mmu)? as u32;

                log::debug!(
                    "LEAVE: EBP {:#010X} -> {:#010X}, ESP restored to old EBP",
                    old_ebp,
                    self.regs.ebp
                );
                Ok(RealModeStep::Continue)
            }

            // INTO (CE) - Interrupt on Overflow
            0xCE => {
                // If overflow flag (OF) is set, call INT 4
                if (self.regs.eflags & 0x800) != 0 {
                    log::debug!("INTO: Overflow flag set, calling INT 4");
                    return self.handle_interrupt(4, mmu);
                } else {
                    log::debug!("INTO: Overflow flag clear, ignoring");
                    Ok(RealModeStep::Continue)
                }
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
                log::debug!(
                    "REP prefix, executing once (next opcode: {:02X})",
                    next_opcode
                );
                // Push back the opcode so it gets processed
                self.regs.eip -= 1;
                Ok(RealModeStep::Continue)
            }

            // ===== Flag Control =====

            // CLC (F8), STC (F9), CLI (FA), STI (FB), CLD (FC), STD (FD)
            0xF8 => {
                self.regs.eflags &= !0x0001;
                Ok(RealModeStep::Continue)
            } // CLC
            0xF9 => {
                self.regs.eflags |= 0x0001;
                Ok(RealModeStep::Continue)
            } // STC
            0xFA => {
                self.regs.eflags &= !0x0200;
                Ok(RealModeStep::Continue)
            } // CLI
            0xFB => {
                self.regs.eflags |= 0x0200;
                Ok(RealModeStep::Continue)
            } // STI
            0xFC => {
                self.regs.eflags &= !0x0400;
                Ok(RealModeStep::Continue)
            } // CLD
            0xFD => {
                self.regs.eflags |= 0x0400;
                Ok(RealModeStep::Continue)
            } // STD

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

            // SALC - Set AL from Carry (D6)
            0xD6 => {
                // SALC sets AL to 0xFF if carry flag is set, 0x00 if clear
                let cf = (self.regs.eflags & 0x01) != 0;
                let al_value = if cf { 0xFF } else { 0x00 };
                self.regs.eax = (self.regs.eax & 0xFFFFFF00) | (al_value as u32);

                log::debug!("SALC instruction (CF={}, AL={:02X})", cf, al_value);
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
                log::debug!(
                    "LOCK prefix {:02X} at CS:IP={:04X}:{:08X} - ignoring",
                    opcode,
                    self.regs.cs,
                    self.regs.eip - 1
                );
                // LOCK prefix is only meaningful for multiprocessor systems
                // In single-processor emulation, we can ignore it
                Ok(RealModeStep::Continue)
            }

            // Segment override prefixes (2E, 36, 3E, 26, 64, 65)
            // In real mode, these prefixes don't change behavior much
            // We ignore them and continue to next instruction
            0x2E | 0x36 | 0x3E | 0x26 | 0x64 | 0x65 => {
                log::debug!(
                    "Segment override prefix {:02X} at CS:IP={:04X}:{:08X} - ignoring",
                    opcode,
                    self.regs.cs,
                    self.regs.eip - 1
                );
                // Don't adjust EIP - just skip the prefix and continue
                // The next fetch will get the actual opcode
                Ok(RealModeStep::Continue)
            }

            // Operand size prefix (66)
            0x66 => {
                log::debug!(
                    "Operand size prefix {:02X} at CS:IP={:04X}:{:08X} - ignoring",
                    opcode,
                    self.regs.cs,
                    self.regs.eip - 1
                );
                // Don't adjust EIP - skip the prefix
                Ok(RealModeStep::Continue)
            }

            // Address size prefix (67)
            0x67 => {
                log::debug!(
                    "Address size prefix {:02X} at CS:IP={:04X}:{:08X} - ignoring",
                    opcode,
                    self.regs.cs,
                    self.regs.eip - 1
                );
                // Don't adjust EIP - skip the prefix
                Ok(RealModeStep::Continue)
            }

            // HLT
            0xF4 => {
                log::info!("HLT encountered in real-mode");

                // Update PIT with current virtual time and check for interrupts
                self.pit.update(&mut self.pic, self.virtual_time_ns);

                // Check if there's a pending interrupt
                if self.pic.has_pending_interrupt() {
                    if let Some(irq) = self.pic.get_pending_interrupt() {
                        log::info!("HLT: Interrupt pending, IRQ {}", irq);

                        // Convert IRQ to interrupt vector (IRQ 0-7 -> INT 8h-Fh, IRQ 8-15 -> INT 70h-77h)
                        let int_num = if irq < 8 {
                            0x08 + irq
                        } else {
                            0x70 + (irq - 8)
                        };

                        // Handle the interrupt
                        return self.handle_interrupt(int_num as u8, mmu);
                    }
                }

                // No interrupt, halt execution
                Ok(RealModeStep::Halt)
            }

            // CMC (Complement Carry) - F5
            0xF5 => {
                // Complement the carry flag (CF)
                self.regs.eflags ^= 0x01;
                log::debug!(
                    "CMC - Complement Carry, new CF={}",
                    (self.regs.eflags & 0x01) != 0
                );
                Ok(RealModeStep::Continue)
            }

            // D8 - floating point instructions (partial implementation)
            0xD8 => {
                let modrm = self.fetch_byte(mmu)?;
                log::debug!(
                    "D8 opcode encountered (modrm={:02X}) - treating as NOP for boot compatibility",
                    modrm
                );
                // For now, treat as NOP to allow boot to continue
                // Full FPU implementation would be needed for proper floating point support
                // D8 = FADD float instructions (32-bit)
                Ok(RealModeStep::Continue)
            }

            // D9 - floating point instructions (partial implementation)
            0xD9 => {
                let modrm = self.fetch_byte(mmu)?;
                log::debug!(
                    "D9 opcode encountered (modrm={:02X}) - treating as NOP for boot compatibility",
                    modrm
                );
                // For now, treat as NOP to allow boot to continue
                // Full FPU implementation would be needed for proper floating point support
                // D9 = FLD float, FSTP, and other FPU instructions
                Ok(RealModeStep::Continue)
            }

            // DA - floating point instructions (partial implementation)
            0xDA => {
                let modrm = self.fetch_byte(mmu)?;
                log::debug!(
                    "DA opcode encountered (modrm={:02X}) - treating as NOP for boot compatibility",
                    modrm
                );
                // For now, treat as NOP to allow boot to continue
                // DA = FIADD, FIMUL, FICOM, etc. (integer to FPU)
                Ok(RealModeStep::Continue)
            }

            // DB - floating point instructions (partial implementation)
            0xDB => {
                let modrm = self.fetch_byte(mmu)?;
                log::debug!(
                    "DB opcode encountered (modrm={:02X}) - treating as NOP for boot compatibility",
                    modrm
                );
                // For now, treat as NOP to allow boot to continue
                // DB = FILD, FISTP, etc. (integer to FPU)
                Ok(RealModeStep::Continue)
            }

            // DC - floating point instructions (partial implementation)
            0xDC => {
                let modrm = self.fetch_byte(mmu)?;
                log::debug!(
                    "DC opcode encountered (modrm={:02X}) - treating as NOP for boot compatibility",
                    modrm
                );
                // For now, treat as NOP to allow boot to continue
                // Full FPU implementation would be needed for proper floating point support
                Ok(RealModeStep::Continue)
            }

            // DD - floating point instructions (partial implementation)
            0xDD => {
                let modrm = self.fetch_byte(mmu)?;
                log::debug!(
                    "DD opcode encountered (modrm={:02X}) - treating as NOP for boot compatibility",
                    modrm
                );
                // For now, treat as NOP to allow boot to continue
                // DD = FLD double, FSTP double, etc. (64-bit floating point)
                Ok(RealModeStep::Continue)
            }

            // DE - floating point instructions (partial implementation)
            0xDE => {
                let modrm = self.fetch_byte(mmu)?;
                log::debug!(
                    "DE opcode encountered (modrm={:02X}) - treating as NOP for boot compatibility",
                    modrm
                );
                // For now, treat as NOP to allow boot to continue
                // DE = FIADD, FIMUL, FICOMP, etc. (integer to FPU)
                Ok(RealModeStep::Continue)
            }

            // DF - floating point instructions (partial implementation)
            0xDF => {
                let modrm = self.fetch_byte(mmu)?;
                log::debug!(
                    "DF opcode encountered (modrm={:02X}) - treating as NOP for boot compatibility",
                    modrm
                );
                // For now, treat as NOP to allow boot to continue
                // DF = FILD, FISTP, FBLD, etc. (integer/BCD to FPU)
                Ok(RealModeStep::Continue)
            }

            // IMUL r16, r/m16, imm16 (69) - Signed multiply with immediate
            0x69 => {
                let modrm = self.fetch_byte(mmu)?;
                let imm16 = self.fetch_word(mmu)?;
                let reg = ((modrm >> 3) & 7) as usize;

                // For simplicity, just log and continue (full IMUL implementation is complex)
                log::debug!(
                    "IMUL r16, r/m16, imm16 ({:04X}) - treating as NOP for now",
                    imm16
                );
                Ok(RealModeStep::Continue)
            }

            // Shift/Rotate by CL (D0-D3)
            0xD0 | 0xD1 | 0xD2 | 0xD3 => {
                let modrm = self.fetch_byte(mmu)?;
                // D0 = r/m8, 1; D1 = r/m16, 1; D2 = r/m8, CL; D3 = r/m16, CL
                let is_word = (opcode == 0xD1) || (opcode == 0xD3);
                let use_cl = (opcode == 0xD2) || (opcode == 0xD3);
                let reg = (modrm >> 3) & 7;

                // For simplicity, just log and continue
                if use_cl {
                    log::debug!("Shift/Rotate by CL, reg={}", reg);
                } else {
                    log::debug!("Shift/Rotate by 1, reg={}", reg);
                }
                Ok(RealModeStep::Continue)
            }

            // ARPL (63) - Adjust RPL Field of Segment Selector
            0x63 => {
                let modrm = self.fetch_byte(mmu)?;
                log::debug!("ARPL instruction (modrm={:02X}) - treating as NOP", modrm);
                // ARPL is used in protected mode for privilege level adjustments
                // For now, treat as NOP to allow boot to continue
                Ok(RealModeStep::Continue)
            }

            // Unknown opcode - try to skip
            _ => {
                log::warn!(
                    "Unknown real-mode opcode: {:02X} at CS:{:04X}, IP:{:08X}",
                    opcode,
                    self.regs.cs,
                    self.regs.eip - 1
                );
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

    /// Configure PIT timer for system timer interrupts
    /// This pre-configures Channel 0 to generate IRQ0 at approximately 100 Hz
    pub fn configure_pit_timer(&mut self, reload_value: u16) {
        self.pit.configure_channel0_timer(reload_value);
    }

    /// Enable PIC timer interrupt (unmask IRQ0)
    /// This allows PIT interrupts to pass through the PIC to the CPU
    pub fn enable_pic_timer_interrupt(&mut self) {
        self.pic.enable_timer_interrupt();
    }

    /// Get Local APIC reference
    pub fn local_apic(&self) -> &LocalApic {
        &self.local_apic
    }

    /// Get mutable Local APIC reference
    pub fn local_apic_mut(&mut self) -> &mut LocalApic {
        &mut self.local_apic
    }

    /// Get I/O APIC reference
    pub fn io_apic(&self) -> &IoApic {
        &self.io_apic
    }

    /// Get mutable I/O APIC reference
    pub fn io_apic_mut(&mut self) -> &mut IoApic {
        &mut self.io_apic
    }

    /// Check if Local APIC has a pending interrupt
    pub fn has_apic_interrupt(&self) -> bool {
        self.local_apic.has_pending_interrupt()
    }

    /// Get pending interrupt from Local APIC (if any)
    pub fn get_apic_interrupt(&mut self) -> Option<u8> {
        self.local_apic.get_pending_interrupt()
    }

    /// Check if there's a pending interrupt from any source (PIC or APIC)
    /// This is called by the execution loop to determine if an interrupt should be injected
    pub fn has_pending_interrupt(&self) -> bool {
        // Check PIC (legacy 8259A) first
        if self.pic.has_pending_interrupt() {
            return true;
        }

        // Check Local APIC (for modern systems)
        if self.local_apic.has_pending_interrupt() {
            return true;
        }

        false
    }

    /// Get the highest priority pending interrupt from any source
    /// Priority: APIC > PIC (APIC has higher priority in x86 systems)
    pub fn get_pending_interrupt(&mut self) -> Option<u8> {
        // Check Local APIC first (higher priority)
        if self.local_apic.has_pending_interrupt() {
            return self.local_apic.get_pending_interrupt();
        }

        // Check PIC (legacy)
        if self.pic.has_pending_interrupt() {
            return self.pic.get_pending_interrupt();
        }

        None
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
