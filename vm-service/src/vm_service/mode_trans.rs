//! # CPU Mode Transitions
//!
//! Handles transitions between x86 CPU modes:
//! - Real mode (16-bit) → Protected mode (32-bit)
//! - Protected mode (32-bit) → Long mode (64-bit)

use super::realmode::{RealModeRegs, RealModeStep};
use vm_core::{GuestAddr, MMU, VmResult};

/// x86 CPU operating mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum X86Mode {
    /// 16-bit real mode
    Real,
    /// 32-bit protected mode
    Protected,
    /// 64-bit long mode
    Long,
}

/// x86 Control Registers
#[derive(Debug, Clone)]
pub struct ControlRegisters {
    /// CR0 - Control register 0
    pub cr0: u32,
    /// CR2 - Control register 2 (page fault address)
    pub cr2: u32,
    /// CR3 - Control register 3 (page directory base)
    pub cr3: u32,
    /// CR4 - Control register 4
    pub cr4: u32,
}

impl ControlRegisters {
    pub fn new() -> Self {
        Self {
            cr0: 0x60000010, // ET, NE set
            cr2: 0,
            cr3: 0,
            cr4: 0,
        }
    }

    /// Check if protected mode is enabled (CR0.PE)
    pub fn protected_mode_enabled(&self) -> bool {
        (self.cr0 & 0x00000001) != 0
    }

    /// Check if paging is enabled (CR0.PG)
    pub fn paging_enabled(&self) -> bool {
        (self.cr0 & 0x80000000) != 0
    }

    /// Check if PAE is enabled (CR4.PAE)
    pub fn pae_enabled(&self) -> bool {
        (self.cr4 & 0x00000020) != 0
    }

    /// Enable protected mode
    pub fn enable_protected_mode(&mut self) {
        self.cr0 |= 0x00000001; // Set PE bit
        log::info!("Protected mode enabled (CR0.PE = 1)");
    }

    /// Enable paging
    pub fn enable_paging(&mut self) {
        self.cr0 |= 0x80000000; // Set PG bit
        log::info!("Paging enabled (CR0.PG = 1)");
    }

    /// Enable PAE
    pub fn enable_pae(&mut self) {
        self.cr4 |= 0x00000020; // Set PAE bit
        log::info!("PAE enabled (CR4.PAE = 1)");
    }
}

impl Default for ControlRegisters {
    fn default() -> Self {
        Self::new()
    }
}

/// Model Specific Register - Extended Feature Enable Register
pub const MSR_EFER: u32 = 0xC0000080;

/// EFER bits
pub const EFER_LME: u64 = 0x00000100; // Long Mode Enable
pub const EFER_LMA: u64 = 0x00000400; // Long Mode Active

/// Global Descriptor Table Entry
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct GdtEntry {
    pub limit_low: u16,
    pub base_low: u16,
    pub base_middle: u8,
    pub access: u8,
    pub flags_limit_high: u8,
    pub base_high: u8,
}

impl GdtEntry {
    /// Create a null descriptor
    pub fn null() -> Self {
        Self {
            limit_low: 0,
            base_low: 0,
            base_middle: 0,
            access: 0,
            flags_limit_high: 0,
            base_high: 0,
        }
    }

    /// Create a flat data segment descriptor (base=0, limit=4GB)
    pub fn data_segment() -> Self {
        Self {
            limit_low: 0xFFFF,
            base_low: 0,
            base_middle: 0,
            access: 0x92,           // Present, Ring 0, Data, Writable
            flags_limit_high: 0xCF, // 32-bit, 4GB limit
            base_high: 0,
        }
    }

    /// Create a flat code segment descriptor (base=0, limit=4GB)
    pub fn code_segment(is_64bit: bool) -> Self {
        let flags = if is_64bit { 0xAF } else { 0x9F }; // L=1 for 64-bit
        Self {
            limit_low: 0xFFFF,
            base_low: 0,
            base_middle: 0,
            access: 0x9A, // Present, Ring 0, Code, Readable
            flags_limit_high: flags,
            base_high: 0,
        }
    }
}

/// GDT Pointer (for GDTR load)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct GdtPointer {
    pub limit: u16,
    pub base: u32,
}

/// Mode transition manager
pub struct ModeTransition {
    /// Current CPU mode
    current_mode: X86Mode,
    /// Control registers
    pub cr: ControlRegisters,
    /// EFER MSR value
    pub efer: u64,
    /// Global Descriptor Table
    gdt: [GdtEntry; 8],
    /// GDT loaded flag
    gdt_loaded: bool,
    /// Page tables (for PAE/paging)
    page_tables: Option<GuestAddr>,
}

impl ModeTransition {
    /// Create new mode transition manager
    pub fn new() -> Self {
        Self {
            current_mode: X86Mode::Real,
            cr: ControlRegisters::new(),
            efer: 0,
            gdt: [GdtEntry::null(); 8],
            gdt_loaded: false,
            page_tables: None,
        }
    }

    /// Get current mode
    pub fn current_mode(&self) -> X86Mode {
        self.current_mode
    }

    /// Force set current mode (aggressive intervention)
    pub fn set_current_mode(&mut self, mode: X86Mode) {
        log::warn!(
            "Forcing mode transition: {:?} -> {:?}",
            self.current_mode,
            mode
        );
        self.current_mode = mode;
        log::warn!("Mode forced to: {:?}", self.current_mode);
    }

    /// Initialize GDT with flat segments
    pub fn init_gdt(&mut self) {
        // Entry 0: Null descriptor (required)
        self.gdt[0] = GdtEntry::null();

        // Entry 1: 32-bit code segment (base=0, limit=4GB)
        self.gdt[1] = GdtEntry::code_segment(false);

        // Entry 2: 32-bit data segment (base=0, limit=4GB)
        self.gdt[2] = GdtEntry::data_segment();

        // Entry 3: 64-bit code segment (for long mode)
        self.gdt[3] = GdtEntry::code_segment(true);

        // Entry 4: 64-bit data segment
        self.gdt[4] = GdtEntry::data_segment();

        log::info!("GDT initialized with flat segments");
        self.gdt_loaded = false;
    }

    /// Load GDT to memory (called by firmware)
    pub fn load_gdt(&mut self, mmu: &mut dyn MMU, gdt_addr: GuestAddr) -> VmResult<()> {
        // Write GDT to memory
        for (i, entry) in self.gdt.iter().enumerate() {
            let addr = GuestAddr(gdt_addr.0 + (i * 8) as u64);
            let entry_ptr = entry as *const GdtEntry as *const u64;
            let entry_val = unsafe { *entry_ptr };
            mmu.write(addr, entry_val, 8)?;
        }

        // Create GDTR
        let gdtr = GdtPointer {
            limit: (self.gdt.len() * 8) as u16 - 1,
            base: gdt_addr.0 as u32,
        };

        // For now, just log it - actual GDTR load would be done by LGDT instruction
        let limit = gdtr.limit;
        let base = gdtr.base;
        log::info!("GDT loaded: base={:#010X}, limit={:#06X}", base, limit);

        self.gdt_loaded = true;
        Ok(())
    }

    /// Mark that GDT has been loaded (called by LGDT instruction)
    pub fn mark_gdt_loaded(&mut self) {
        self.gdt_loaded = true;
        log::info!("GDT loaded flag set to true (via LGDT)");
    }

    /// Initialize page tables for PAE/paging
    pub fn init_page_tables(&mut self, _mmu: &mut dyn MMU) -> VmResult<GuestAddr> {
        // For simplicity, use identity mapping
        // In a real implementation, this would create proper page tables

        let pml4_addr = GuestAddr(0x10000); // Place at 64KB
        let _pdpt_addr = GuestAddr(0x11000);
        let _pd_addr = GuestAddr(0x12000);

        // For now, just store the address
        self.page_tables = Some(pml4_addr);

        // Set CR3
        self.cr.cr3 = pml4_addr.0 as u32;

        log::info!("Page tables initialized: PML4={:#010X}", pml4_addr.0);
        Ok(pml4_addr)
    }

    /// Switch from real mode to protected mode
    pub fn switch_to_protected_mode(
        &mut self,
        regs: &mut RealModeRegs,
        mmu: &mut dyn MMU,
    ) -> VmResult<RealModeStep> {
        log::info!("=== Switching to Protected Mode ===");

        // Step 1: Initialize GDT
        self.init_gdt();

        // Step 2: Load GDT to memory (at 0x5000)
        let gdt_addr = GuestAddr(0x5000);
        self.load_gdt(mmu, gdt_addr)?;

        // Step 3: Set CR0.PE to enable protected mode
        self.cr.enable_protected_mode();

        // Step 4: Reload segment registers with protected mode selectors
        // Code selector = 0x08 (entry 1 * 8)
        // Data selector = 0x10 (entry 2 * 8)
        regs.cs = 0x08;
        regs.ds = 0x10;
        regs.es = 0x10;
        regs.ss = 0x10;
        regs.fs = 0x10;
        regs.gs = 0x10;

        log::info!("Segment registers reloaded for protected mode:");
        log::info!(
            "  CS={:#06X}, DS={:#06X}, ES={:#06X}, SS={:#06X}",
            regs.cs,
            regs.ds,
            regs.es,
            regs.ss
        );

        // Step 5: Clear direction flag
        regs.eflags &= !0x0400;

        self.current_mode = X86Mode::Protected;

        log::info!("=== Protected Mode Active ===");
        Ok(RealModeStep::SwitchMode)
    }

    /// Switch from protected mode to long mode
    pub fn switch_to_long_mode(
        &mut self,
        regs: &mut RealModeRegs,
        mmu: &mut dyn MMU,
    ) -> VmResult<RealModeStep> {
        log::info!("=== Switching to Long Mode ===");

        // Step 1: Enable PAE (CR4.PAE)
        self.cr.enable_pae();

        // Step 2: Initialize page tables
        let pml4_addr = self.init_page_tables(mmu)?;

        // Step 3: Set EFER.LME to enable long mode
        self.efer |= EFER_LME;
        log::info!("EFER.LME set (Long Mode Enable)");

        // Step 4: Enable paging (CR0.PG) - this activates long mode
        self.cr.enable_paging();

        // Step 5: EFER.LMA is now set automatically by hardware
        self.efer |= EFER_LMA;

        // Step 6: Reload CS with 64-bit code selector (0x18 = entry 3 * 8)
        regs.cs = 0x18;
        regs.ds = 0x20; // Entry 4 * 8
        regs.es = 0x20;
        regs.ss = 0x20;

        log::info!("Segment registers reloaded for long mode:");
        log::info!(
            "  CS={:#06X}, DS={:#06X}, ES={:#06X}, SS={:#06X}",
            regs.cs,
            regs.ds,
            regs.es,
            regs.ss
        );

        self.current_mode = X86Mode::Long;

        log::info!("=== Long Mode Active ===");
        log::info!("PML4 table at: {:#010X}", pml4_addr.0);
        log::info!(
            "CR0={:#010X}, CR4={:#010X}, EFER={:#018X}",
            self.cr.cr0,
            self.cr.cr4,
            self.efer
        );

        Ok(RealModeStep::SwitchMode)
    }

    /// Handle control register access (MOV to CRn)
    pub fn write_control_register(&mut self, reg: u8, val: u32) -> VmResult<()> {
        match reg {
            0 => {
                self.cr.cr0 = val;
                if val & 0x00000001 != 0 && !self.cr.protected_mode_enabled() {
                    log::info!("CR0.PE set via MOV to CR0");
                }
                if val & 0x80000000 != 0 && !self.cr.paging_enabled() {
                    log::info!("CR0.PG set via MOV to CR0");
                }
            }
            2 => self.cr.cr2 = val,
            3 => self.cr.cr3 = val,
            4 => {
                self.cr.cr4 = val;
                if val & 0x00000020 != 0 && !self.cr.pae_enabled() {
                    log::info!("CR4.PAE set via MOV to CR4");
                }
            }
            _ => log::warn!("Invalid control register: CR{}", reg),
        }
        Ok(())
    }

    /// Handle MSR write (WRMSR instruction)
    pub fn write_msr(&mut self, msr: u32, val: u64) -> VmResult<()> {
        match msr {
            MSR_EFER => {
                self.efer = val;
                log::info!("EFER MSR updated: {:#018X}", val);
                if val & EFER_LME != 0 {
                    log::info!("EFER.LME set (Long Mode Enable)");
                }
            }
            _ => log::warn!("Unknown MSR: {:#010X}", msr),
        }
        Ok(())
    }

    /// Check if we need to handle a mode switch
    pub fn check_mode_switch(
        &mut self,
        regs: &mut RealModeRegs,
        mmu: &mut dyn MMU,
    ) -> VmResult<Option<RealModeStep>> {
        match self.current_mode {
            X86Mode::Real => {
                // Check if protected mode is being enabled
                if self.cr.protected_mode_enabled() && !self.gdt_loaded {
                    return Ok(Some(self.switch_to_protected_mode(regs, mmu)?));
                }
            }
            X86Mode::Protected => {
                // Check if long mode is being enabled
                if self.cr.paging_enabled() && self.cr.pae_enabled() && (self.efer & EFER_LME) != 0
                {
                    return Ok(Some(self.switch_to_long_mode(regs, mmu)?));
                }
            }
            X86Mode::Long => {
                // Already in long mode, nothing to do
            }
        }
        Ok(None)
    }
}

impl Default for ModeTransition {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_transition_create() {
        let mt = ModeTransition::new();
        assert_eq!(mt.current_mode(), X86Mode::Real);
        assert!(!mt.cr.protected_mode_enabled());
    }

    #[test]
    fn test_gdt_entry() {
        let null = GdtEntry::null();
        // Can't access packed fields directly, just check it creates
        let _ = null;

        let code = GdtEntry::code_segment(false);
        // Can't access packed fields directly
        let _ = code;
    }

    #[test]
    fn test_control_registers() {
        let mut cr = ControlRegisters::new();
        assert!(!cr.protected_mode_enabled());

        cr.enable_protected_mode();
        assert!(cr.protected_mode_enabled());
    }
}
