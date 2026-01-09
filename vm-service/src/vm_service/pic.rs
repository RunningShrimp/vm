//! # 8259A Programmable Interrupt Controller (PIC)
//!
//! Minimal implementation of the dual 8259A PIC chips used in x86 systems.
//! Handles IRQ routing and interrupt masking.

use log::debug;

/// 8259A PIC - Master and Slave
pub struct Pic {
    /// Master PIC (IRQs 0-7)
    master: PicChip,
    /// Slave PIC (IRQs 8-15)
    slave: PicChip,
    /// Interrupt request lines (0-15)
    irq_pending: u16,
    /// Interrupt mask (0 = enabled, 1 = disabled)
    irq_mask: u16,
    /// In-service register (currently being handled)
    in_service: u8,
}

/// Single 8259A PIC chip
struct PicChip {
    /// Interrupt Request register
    irr: u8,
    /// Interrupt Mask register
    imr: u8,
    /// In-Service register
    isr: u8,
    /// Initialization Command Word state
    init_state: u8,
    /// ICW1: Edge/level trigger
    edge_trigger: bool,
    /// ICW1: Cascade mode
    cascade: bool,
    /// ICW1: ICW4 needed
    icw4_needed: bool,
    /// ICW3: Slave identification (for master) or master ID (for slave)
    icw3: u8,
    /// ICW4: 8086 mode
    i8086_mode: bool,
    /// ICW4: Automatic EOI
    auto_eoi: bool,
    /// ICW4: Buffered mode
    buffered_mode: bool,
    /// ICW4: Slave/Full nested
    slave_full_nested: bool,
}

impl Default for PicChip {
    fn default() -> Self {
        Self {
            irr: 0,
            imr: 0xFF, // All IRQs masked by default
            isr: 0,
            init_state: 0,
            edge_trigger: true,
            cascade: false,
            icw4_needed: false,
            icw3: 0,
            i8086_mode: true,
            auto_eoi: false,
            buffered_mode: false,
            slave_full_nested: false,
        }
    }
}

impl Pic {
    /// Create new dual PIC configuration
    pub fn new() -> Self {
        Self {
            master: PicChip::default(),
            slave: PicChip::default(),
            irq_pending: 0,
            irq_mask: 0xFFFF, // All IRQs masked by default
            in_service: 0,
        }
    }

    /// Configure PIC for system timer (IRQ0)
    /// This unmasks IRQ0 to allow PIT interrupts to pass through
    pub fn enable_timer_interrupt(&mut self) {
        // Unmask IRQ0 (bit 0 = 1 means disabled, so we clear it)
        self.master.imr &= 0xFE; // Clear bit 0 to enable IRQ0
        self.irq_mask &= 0xFFFE; // Clear bit 0 in overall mask

        log::info!("PIC: IRQ0 (timer interrupt) enabled");
        log::info!("  Master IMR: {:#04X}", self.master.imr);
        log::info!("  IRQ mask: {:#04X}", self.irq_mask);
    }

    /// Raise an IRQ line
    pub fn raise_irq(&mut self, irq: u8) {
        if irq < 16 {
            self.irq_pending |= 1 << irq;
            if irq < 8 {
                self.master.irr |= 1 << irq;
            } else {
                self.slave.irr |= 1 << (irq - 8);
            }
            debug!("PIC: IRQ {} raised, pending={:04X}", irq, self.irq_pending);
        }
    }

    /// Check if there's a pending interrupt
    pub fn has_pending_interrupt(&self) -> bool {
        // Check if any unmasked IRQ is pending
        let unmasked = self.irq_pending & !self.irq_mask;
        unmasked != 0
    }

    /// Get the highest priority pending interrupt
    pub fn get_pending_interrupt(&mut self) -> Option<u8> {
        // Find highest priority (lowest number) unmasked IRQ
        let unmasked = self.irq_pending & !self.irq_mask;

        for irq in 0..16 {
            if (unmasked & (1 << irq)) != 0 {
                // Mark this IRQ as in-service
                if irq < 8 {
                    self.master.isr |= 1 << irq;
                    self.master.irr &= !(1 << irq);
                } else {
                    self.slave.isr |= 1 << (irq - 8);
                    self.slave.irr &= !(1 << (irq - 8));
                }
                self.irq_pending &= !(1 << irq);
                self.in_service = irq as u8;
                return Some(irq);
            }
        }

        None
    }

    /// End of Interrupt - specific IRQ
    pub fn eoi_specific(&mut self, irq: u8) {
        if irq < 8 {
            self.master.isr &= !(1 << irq);
        } else {
            self.slave.isr &= !(1 << (irq - 8));
        }
        self.in_service = 0;
        debug!("PIC: EOI for IRQ {}", irq);
    }

    /// End of Interrupt - non-specific (highest priority in-service)
    pub fn eoi_nonspecific(&mut self) {
        // Clear highest priority in-service bit
        if self.master.isr != 0 {
            let irq = self.master.isr.trailing_zeros() as u8;
            self.master.isr &= !(1 << irq);
            debug!("PIC: Non-specific EOI for IRQ {}", irq);
        }
        if self.slave.isr != 0 {
            let irq = self.slave.isr.trailing_zeros() as u8;
            self.slave.isr &= !(1 << irq);
            debug!("PIC: Non-specific EOI for IRQ {}", irq + 8);
        }
        self.in_service = 0;
    }

    /// Handle PIC I/O port read
    pub fn port_read(&self, port: u16) -> u8 {
        match port {
            0x20 => self.master.irr & !self.master.imr, // Master IRR
            0x21 => self.master.imr,                    // Master IMR
            0xA0 => self.slave.irr & !self.slave.imr,   // Slave IRR
            0xA1 => self.slave.imr,                     // Slave IMR
            _ => {
                debug!("PIC: Unexpected read from port {:04X}", port);
                0
            }
        }
    }

    /// Handle PIC I/O port write
    pub fn port_write(&mut self, port: u16, value: u8) {
        debug!("PIC: Write {:02X} to port {:04X}", value, port);
        // Simplified: just mask/unmask IRQs
        match port {
            0x21 => {
                // Master IMR
                self.master.imr = value;
                self.irq_mask = (self.irq_mask & 0xFF00) | (value as u16);
            }
            0xA1 => {
                // Slave IMR
                self.slave.imr = value;
                self.irq_mask = (self.irq_mask & 0x00FF) | ((value as u16) << 8);
            }
            _ => {
                debug!("PIC: Write to port {:04X} not fully implemented", port);
            }
        }
    }

    /// Set IRQ mask (1 = masked/disabled, 0 = unmasked/enabled)
    pub fn mask_irq(&mut self, irq: u8, masked: bool) {
        if irq < 16 {
            if masked {
                self.irq_mask |= 1 << irq;
            } else {
                self.irq_mask &= !(1 << irq);
            }
            debug!("PIC: IRQ {} {}masked", irq, if masked { "" } else { "un" });
        }
    }

    /// Get current IRQ mask
    pub fn irq_mask(&self) -> u16 {
        self.irq_mask
    }
}

impl Default for Pic {
    fn default() -> Self {
        Self::new()
    }
}
