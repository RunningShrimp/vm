//! # Local APIC (Advanced Programmable Interrupt Controller)
//!
//! Implementation of the Local APIC as specified in the Intel 64 and IA-32 Architectures
//! Software Developer's Manual, Volume 3A, Chapter 10.
//!
//! The Local APIC is used in modern x86 systems for:
//! - Interrupt handling (timer, IPIs, performance counters)
//! - Inter-processor interrupts (SMP)
//! - System management interrupts
//!
//! This implementation provides the minimal functionality required for Ubuntu 25.10:
//! - Timer interrupt generation (critical for kernel scheduling)
//! - I/O APIC interface for device interrupts
//! - Basic interrupt delivery to the CPU

use log::{debug, info, warn};

/// Local APIC base address (standard x86_64 location)
pub const LOCAL_APIC_BASE: u64 = 0xFEE0_0000;

/// Local APIC register offsets (from base address)
mod offsets {
    pub const LAPIC_ID: u16 = 0x020;
    pub const LAPIC_VERSION: u16 = 0x030;
    pub const TPR: u16 = 0x080; // Task Priority Register
    pub const APR: u16 = 0x090; // Arbitration Priority
    pub const PPR: u16 = 0x0A0; // Processor Priority
    pub const EOI: u16 = 0x0B0; // End of Interrupt
    pub const LDR: u16 = 0x0D0; // Logical Destination
    pub const DFR: u16 = 0x0E0; // Destination Format
    pub const SVR: u16 = 0x0F0; // Spurious Interrupt Vector
    pub const ISR_BASE: u16 = 0x100; // In-Service Register (0-7, 8 bits each, 256 bits total)
    pub const TMR_BASE: u16 = 0x180; // Trigger Mode Register
    pub const IRR_BASE: u16 = 0x200; // Interrupt Request Register
    pub const ERROR_STATUS: u16 = 0x280; // Error Status
    pub const ICR_LOW: u16 = 0x300; // Interrupt Command Register
    pub const ICR_HIGH: u16 = 0x310;
    pub const TIMER_LVT: u16 = 0x320; // Timer Local Vector Table
    pub const THERMAL_LVT: u16 = 0x330;
    pub const PERF_LVT: u16 = 0x340;
    pub const LINT0_LVT: u16 = 0x350;
    pub const LINT1_LVT: u16 = 0x360;
    pub const ERROR_LVT: u16 = 0x370;
    pub const TIMER_INITIAL: u16 = 0x380; // Timer Initial Count
    pub const TIMER_CURRENT: u16 = 0x390; // Timer Current Count
    pub const TIMER_DIVIDE: u16 = 0x3E0; // Timer Divide Configuration
}

/// Local APIC state
#[derive(Debug)]
pub struct LocalApic {
    /// APIC ID (typically 0 for uniprocessor)
    apic_id: u32,
    /// APIC version
    version: u32,
    /// Task Priority Register
    tpr: u32,
    /// Spurious Interrupt Vector Register
    svr: u32,
    /// In-Service Registers (256 bits for 256 potential interrupts)
    isr: [u32; 8],
    /// Trigger Mode Registers
    tmr: [u32; 8],
    /// Interrupt Request Registers
    irr: [u32; 8],
    /// Error Status
    error_status: u32,
    /// Interrupt Command Register (low and high)
    icr_low: u32,
    icr_high: u32,
    /// Local Vector Table entries
    timer_lvt: u32,
    thermal_lvt: u32,
    perf_lvt: u32,
    lint0_lvt: u32,
    lint1_lvt: u32,
    error_lvt: u32,
    /// Timer state
    timer_initial: u32,
    timer_current: u32,
    timer_divide: u32,
    timer_active: bool,
    /// Virtual time tracking (nanoseconds)
    virtual_time_ns: u64,
    last_update_ns: u64,
    /// Pending interrupt vector (if any)
    pending_interrupt: Option<u8>,
    /// APIC enabled flag
    enabled: bool,
}

impl Default for LocalApic {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalApic {
    /// Create a new Local APIC
    pub fn new() -> Self {
        Self {
            apic_id: 0,
            version: 0x0001_0014, // Version 1h, max LVT entry 1Fh (32)
            tpr: 0,
            svr: 0x0000_00FF, // Disabled (bit 8 = 0), spurious vector FF
            isr: [0; 8],
            tmr: [0; 8],
            irr: [0; 8],
            error_status: 0,
            icr_low: 0,
            icr_high: 0,
            timer_lvt: 0x0001_0000,   // Masked timer interrupt
            thermal_lvt: 0x0001_0000, // Masked
            perf_lvt: 0x0001_0000,    // Masked
            lint0_lvt: 0x0001_0000,   // Masked
            lint1_lvt: 0x0001_0000,   // Masked
            error_lvt: 0x0001_0000,   // Masked
            timer_initial: 0,
            timer_current: 0,
            timer_divide: 0, // Divide by 2
            timer_active: false,
            virtual_time_ns: 0,
            last_update_ns: 0,
            pending_interrupt: None,
            enabled: false,
        }
    }

    /// Enable the APIC (set SVR bit 8)
    pub fn enable(&mut self) {
        self.svr |= 0x100;
        self.enabled = true;
        info!("APIC: Enabled (SVR={:#010X})", self.svr);
    }

    /// Check if APIC is enabled
    pub fn is_enabled(&self) -> bool {
        (self.svr & 0x100) != 0 && self.enabled
    }

    /// Handle MMIO read from APIC register
    pub fn mmio_read(&mut self, offset: u16) -> u32 {
        // APIC registers are 32-bit, accessed at 16-bit aligned offsets
        let offset_16 = offset & 0xFF0;

        match offset_16 {
            offsets::LAPIC_ID => self.apic_id,
            offsets::LAPIC_VERSION => self.version,
            offsets::TPR => self.tpr,
            offsets::SVR => self.svr,
            offsets::EOI => {
                // Reads return 0
                0
            }
            offsets::ICR_LOW => {
                // Delivery status bit (12) is read-only
                self.icr_low & 0x1000
            }
            offsets::ICR_HIGH => self.icr_high,
            offsets::TIMER_LVT => self.timer_lvt,
            offsets::THERMAL_LVT => self.thermal_lvt,
            offsets::PERF_LVT => self.perf_lvt,
            offsets::LINT0_LVT => self.lint0_lvt,
            offsets::LINT1_LVT => self.lint1_lvt,
            offsets::ERROR_LVT => self.error_lvt,
            offsets::TIMER_INITIAL => self.timer_initial,
            offsets::TIMER_CURRENT => self.timer_current,
            offsets::TIMER_DIVIDE => self.timer_divide,
            offsets::ERROR_STATUS => self.error_status,
            _ => {
                // Handle ISR, IRR, TMR reads (8 registers each, 32 bits each)
                if offset_16 >= offsets::ISR_BASE && offset_16 < offsets::ISR_BASE + 0x80 {
                    let index = ((offset_16 - offsets::ISR_BASE) / 0x10) as usize;
                    if index < 8 {
                        return self.isr[index];
                    }
                }
                if offset_16 >= offsets::IRR_BASE && offset_16 < offsets::IRR_BASE + 0x80 {
                    let index = ((offset_16 - offsets::IRR_BASE) / 0x10) as usize;
                    if index < 8 {
                        return self.irr[index];
                    }
                }
                if offset_16 >= offsets::TMR_BASE && offset_16 < offsets::TMR_BASE + 0x80 {
                    let index = ((offset_16 - offsets::TMR_BASE) / 0x10) as usize;
                    if index < 8 {
                        return self.tmr[index];
                    }
                }

                debug!("APIC: Read from unimplemented offset {:#04X}", offset_16);
                0
            }
        }
    }

    /// Handle MMIO write to APIC register
    pub fn mmio_write(&mut self, offset: u16, value: u32) {
        let offset_16 = offset & 0xFF0;

        match offset_16 {
            offsets::TPR => {
                self.tpr = value & 0xFF; // Only lower 4 bits used
                debug!("APIC: TPR write = {:#04X}", self.tpr);
            }
            offsets::SVR => {
                let was_enabled = self.is_enabled();
                self.svr = value;
                let now_enabled = (value & 0x100) != 0;

                if !was_enabled && now_enabled {
                    self.enabled = true;
                    info!("APIC: Enabled via SVR write");
                } else if was_enabled && !now_enabled {
                    self.enabled = false;
                    info!("APIC: Disabled via SVR write");
                }

                debug!("APIC: SVR write = {:#010X}", self.svr);
            }
            offsets::EOI => {
                // End of Interrupt - clear highest-priority bit in ISR
                for i in 0..8 {
                    if self.isr[i] != 0 {
                        let bit = self.isr[i].trailing_zeros() as usize;
                        self.isr[i] &= !(1 << bit);
                        debug!("APIC: EOI for interrupt in ISR[{}] bit {}", i, bit);
                        break;
                    }
                }
            }
            offsets::ICR_LOW => {
                self.icr_low = value;
                // Handle IPI delivery if delivery bit (10) is set
                if value & 0x1000 != 0 {
                    debug!("APIC: ICR_LOW write = {:#010X} (IPI delivery)", value);
                    // In uniprocessor, we just acknowledge but don't deliver to another CPU
                }
            }
            offsets::ICR_HIGH => {
                self.icr_high = value;
                debug!("APIC: ICR_HIGH write = {:#010X}", value);
            }
            offsets::TIMER_LVT => {
                self.timer_lvt = value;
                debug!(
                    "APIC: TIMER_LVT write = {:#010X} (vector={}, masked={})",
                    value,
                    value & 0xFF,
                    (value >> 16) & 1
                );
            }
            offsets::THERMAL_LVT => {
                self.thermal_lvt = value;
                debug!("APIC: THERMAL_LVT write = {:#010X}", value);
            }
            offsets::PERF_LVT => {
                self.perf_lvt = value;
                debug!("APIC: PERF_LVT write = {:#010X}", value);
            }
            offsets::LINT0_LVT => {
                self.lint0_lvt = value;
                debug!("APIC: LINT0_LVT write = {:#010X}", value);
            }
            offsets::LINT1_LVT => {
                self.lint1_lvt = value;
                debug!("APIC: LINT1_LVT write = {:#010X}", value);
            }
            offsets::ERROR_LVT => {
                self.error_lvt = value;
                debug!("APIC: ERROR_LVT write = {:#010X}", value);
            }
            offsets::TIMER_INITIAL => {
                self.timer_initial = value;
                self.timer_current = value;
                self.timer_active = value > 0;

                if value > 0 {
                    info!("APIC: Timer started - initial count = {}", value);
                    info!("  Divide config = {:#04X}", self.timer_divide);
                    info!(
                        "  Timer LVT = {:#010X} (vector={})",
                        self.timer_lvt,
                        self.timer_lvt & 0xFF
                    );
                }
            }
            offsets::TIMER_DIVIDE => {
                self.timer_divide = value & 0x0B; // Only bits 0-3 used
                debug!("APIC: TIMER_DIVIDE write = {:#04X}", self.timer_divide);
            }
            offsets::ERROR_STATUS => {
                // Write-only clear specific bits
                self.error_status &= !value;
                debug!("APIC: ERROR_STATUS write = {:#010X}", value);
            }
            _ => {
                debug!(
                    "APIC: Write to unimplemented offset {:#04X} = {:#010X}",
                    offset_16, value
                );
            }
        }
    }

    /// Update APIC timer (call this periodically with virtual time)
    pub fn update_timer(&mut self, virtual_time_ns: u64) {
        if !self.is_enabled() {
            return;
        }

        if !self.timer_active || self.timer_initial == 0 {
            return;
        }

        // Calculate elapsed virtual time
        let elapsed_ns = virtual_time_ns.saturating_sub(self.last_update_ns);
        self.last_update_ns = virtual_time_ns;

        // Calculate timer divide value
        // Timer divide configuration (bits 0-2 of TIMER_DIVIDE):
        // 000 = divide by 2, 001 = divide by 4, 010 = divide by 8, 011 = divide by 16
        // 100 = divide by 32, 101 = divide by 64, 110 = divide by 128, 111 = divide by 1
        let divide_value = match self.timer_divide & 0x0B {
            0b0000 => 2,
            0b0001 => 4,
            0b0010 => 8,
            0b0011 => 16,
            0b1000 => 32,
            0b1001 => 64,
            0b1010 => 128,
            0b1011 => 1,
            _ => 2,
        };

        // APIC timer frequency: based on bus frequency (typically 100-200 MHz)
        // We'll use a conservative 100 MHz for timing
        let bus_frequency_hz = 100_000_000u64;
        let timer_period_ns = 1_000_000_000u64 / (bus_frequency_hz / divide_value as u64);

        // Calculate how many timer ticks should have occurred
        let total_ticks = virtual_time_ns / timer_period_ns;
        let previous_total_ticks = self.virtual_time_ns / timer_period_ns;
        let ticks = total_ticks - previous_total_ticks;

        // Update virtual time counter
        self.virtual_time_ns = virtual_time_ns;

        if ticks > 0 {
            // Decrement timer counter
            let prev_counter = self.timer_current;

            if ticks >= self.timer_current as u64 {
                // Timer expired
                self.timer_current = 0;

                // Generate timer interrupt if not masked
                let timer_masked = (self.timer_lvt >> 16) & 1 != 0;
                let timer_vector = (self.timer_lvt & 0xFF) as u8;

                if !timer_masked && timer_vector != 0 {
                    info!(
                        "APIC: Timer expired ({} -> {}), raising interrupt vector {}",
                        prev_counter, self.timer_current, timer_vector
                    );

                    self.raise_interrupt(timer_vector);

                    // For one-shot mode, timer stops
                    // For periodic mode, timer would reload (not implemented yet)
                    self.timer_active = false;
                } else {
                    debug!("APIC: Timer expired but interrupt masked or vector=0");
                    self.timer_active = false;
                }
            } else {
                self.timer_current -= ticks as u32;
            }
        }
    }

    /// Raise an interrupt (set bit in IRR)
    pub fn raise_interrupt(&mut self, vector: u8) {
        // Note: vector is u8 (0-255), so it's always < 256
        let reg_index = vector as usize / 32;
        let bit_index = vector as usize % 32;

        if reg_index < 8 {
            self.irr[reg_index] |= 1 << bit_index;
            debug!(
                "APIC: Interrupt {} raised (IRR[{}] bit {:02X})",
                vector, reg_index, bit_index
            );
        }
    }

    /// Check if there's a pending interrupt for the CPU
    pub fn has_pending_interrupt(&self) -> bool {
        if !self.is_enabled() {
            return false;
        }

        // Check if any bit in IRR is set that's not masked by TPR
        // TPR is a 4-bit priority field (bits 0-3)
        // Interrupts with priority <= TPR are masked
        let tpr_priority = (self.tpr & 0x0F) as u8;

        for i in 0..8 {
            if self.irr[i] != 0 {
                // Check each bit in this register
                for bit in 0..32 {
                    if (self.irr[i] & (1 << bit)) != 0 {
                        let vector = (i * 32 + bit) as u8;
                        let priority = vector >> 4;

                        // Interrupt is deliverable if its priority > TPR
                        if priority > tpr_priority {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Get the highest priority pending interrupt vector
    pub fn get_pending_interrupt(&mut self) -> Option<u8> {
        if !self.is_enabled() {
            return None;
        }

        let tpr_priority = (self.tpr & 0x0F) as u8;
        let mut highest_vector: Option<u8> = None;
        let mut highest_priority: u8 = 0;

        // Find highest priority interrupt in IRR
        for i in 0..8 {
            for bit in 0..32 {
                if (self.irr[i] & (1 << bit)) != 0 {
                    let vector = (i * 32 + bit) as u8;
                    let priority = vector >> 4;

                    if priority > tpr_priority && priority >= highest_priority {
                        highest_priority = priority;
                        highest_vector = Some(vector);
                    }
                }
            }
        }

        // If we found an interrupt, move it from IRR to ISR
        if let Some(vector) = highest_vector {
            let reg_index = vector as usize / 32;
            let bit_index = vector as usize % 32;

            if reg_index < 8 {
                self.irr[reg_index] &= !(1 << bit_index);
                self.isr[reg_index] |= 1 << bit_index;

                info!(
                    "APIC: Delivering interrupt {} (IRR->ISR[{}] bit {:02X})",
                    vector, reg_index, bit_index
                );

                return Some(vector);
            }
        }

        None
    }

    /// Get current timer value (for debugging)
    pub fn get_timer_current(&self) -> u32 {
        self.timer_current
    }

    /// Check if timer is active
    pub fn is_timer_active(&self) -> bool {
        self.timer_active
    }
}

/// I/O APIC (Advanced Programmable Interrupt Controller)
///
/// The I/O APIC receives interrupts from devices and routes them to Local APICs.
/// In a uniprocessor system, it routes all interrupts to the single Local APIC.
pub struct IoApic {
    /// I/O APIC ID
    id: u32,
    /// I/O APIC version
    version: u32,
    /// Maximum redirection entry (24 entries typical)
    max_redirection_entry: u32,
    /// I/O APIC address (typically 0xFEC00000)
    base_address: u64,
    /// Interrupt redirection entries (24 entries, 64 bits each)
    redirection_entries: [u64; 24],
    /// I/O APIC enabled flag
    enabled: bool,
}

impl Default for IoApic {
    fn default() -> Self {
        Self::new()
    }
}

impl IoApic {
    /// Create a new I/O APIC
    pub fn new() -> Self {
        Self {
            id: 0,
            version: 0x0017_0011, // Version 17h, max entry 23h
            max_redirection_entry: 23,
            base_address: 0xFEC0_0000,
            redirection_entries: [0; 24],
            enabled: false,
        }
    }

    /// Enable the I/O APIC
    pub fn enable(&mut self) {
        self.enabled = true;
        info!("I/O APIC: Enabled");
    }

    /// Check if I/O APIC is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Handle MMIO read from I/O APIC register
    pub fn mmio_read(&mut self, offset: u32) -> u32 {
        match offset {
            0x00 => self.id,
            0x01 => self.version,
            0x02 => (self.id >> 24) | 0x1000, // Arbitration ID (hardcoded)
            _ => {
                // Read redirection entry
                if offset >= 0x10 && offset <= 0x3F {
                    let entry_index = ((offset - 0x10) / 2) as usize;
                    let high = (offset % 2) != 0;

                    if entry_index < 24 {
                        if high {
                            (self.redirection_entries[entry_index] >> 32) as u32
                        } else {
                            (self.redirection_entries[entry_index] & 0xFFFFFFFF) as u32
                        }
                    } else {
                        0
                    }
                } else {
                    debug!("I/O APIC: Read from unimplemented offset {:#04X}", offset);
                    0
                }
            }
        }
    }

    /// Handle MMIO write to I/O APIC register
    pub fn mmio_write(&mut self, offset: u32, value: u32) {
        match offset {
            0x00 => {
                self.id = value & 0x0F00_0000; // Only bits 24-27 used
                debug!("I/O APIC: ID write = {:#010X}", self.id);
            }
            _ => {
                // Write redirection entry
                if offset >= 0x10 && offset <= 0x3F {
                    let entry_index = ((offset - 0x10) / 2) as usize;
                    let high = (offset % 2) != 0;

                    if entry_index < 24 {
                        if high {
                            self.redirection_entries[entry_index] =
                                (self.redirection_entries[entry_index] & 0xFFFFFFFF)
                                    | ((value as u64) << 32);
                        } else {
                            self.redirection_entries[entry_index] =
                                (self.redirection_entries[entry_index] & !0xFFFFFFFF)
                                    | (value as u64);
                        }

                        debug!(
                            "I/O APIC: Redirection entry {} {} dword = {:#010X}",
                            entry_index,
                            if high { "high" } else { "low" },
                            value
                        );
                    }
                } else {
                    debug!(
                        "I/O APIC: Write to unimplemented offset {:#04X} = {:#010X}",
                        offset, value
                    );
                }
            }
        }
    }

    /// Set up default redirection entries for standard IRQs
    pub fn setup_default_irqs(&mut self) {
        // Standard IRQ mappings (ISA IRQs 0-15)
        // IRQ 0-15 map to I/O APIC entries 0-15 with vectors 32-47
        for i in 0..16 {
            self.redirection_entries[i] = 0x0000_0000_0001_0000u64; // Masked initially
        }

        info!("I/O APIC: Default IRQ redirection entries configured (IRQs 0-15)");
    }

    /// Unmask an IRQ (allow it to be delivered)
    pub fn unmask_irq(&mut self, irq: u8) {
        if irq < 24 {
            self.redirection_entries[irq as usize] &= !(1 << 16); // Clear mask bit
            info!("I/O APIC: IRQ {} unmasked", irq);
        }
    }

    /// Raise an IRQ from a device
    pub fn raise_irq(&mut self, irq: u8, local_apic: &mut LocalApic) {
        if irq >= 24 {
            return;
        }

        let entry = self.redirection_entries[irq as usize];

        // Check if IRQ is masked
        if (entry & (1 << 16)) != 0 {
            debug!("I/O APIC: IRQ {} is masked, not delivered", irq);
            return;
        }

        // Extract vector from redirection entry (bits 0-7)
        let vector = (entry & 0xFF) as u8;

        if vector != 0 {
            info!("I/O APIC: IRQ {} -> Local APIC vector {}", irq, vector);
            local_apic.raise_interrupt(vector);
        }
    }
}
