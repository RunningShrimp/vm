//! # PCI Device Emulation
//!
//! Minimal PCI configuration space emulation for graphics device discovery.
//! Provides VGA device with framebuffer BAR for Ubuntu graphics initialization.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::{GuestAddr, MMU, VmError, VmResult};

/// PCI configuration space registers
pub const PCI_CONFIG_ADDRESS_PORT: u16 = 0xCF8;
pub const PCI_CONFIG_DATA_PORT: u16 = 0xCFC;

/// PCI vendor IDs
pub const PCI_VENDOR_ID_QEMU: u16 = 0x1234;
pub const PCI_VENDOR_ID_BOCHS: u16 = 0xB0B0;
pub const PCI_DEVICE_ID_VGA: u16 = 0x1111;

/// PCI class codes
pub const PCI_CLASS_DISPLAY_VGA: u8 = 0x00;
pub const PCI_SUBCLASS_VGA_COMPATIBLE: u8 = 0x01;

/// PCI command register bits
pub const PCI_COMMAND_IO_SPACE: u16 = 0x0001;
pub const PCI_COMMAND_MEMORY_SPACE: u16 = 0x0002;
pub const PCI_COMMAND_BUS_MASTER: u16 = 0x0004;

/// PCI BAR (Base Address Register) types
pub const PCI_BAR_MEMORY_SPACE: u8 = 0x00; // Memory space
pub const PCI_BAR_IO_SPACE: u8 = 0x01; // I/O space
pub const PCI_BAR_32BIT: u8 = 0x00; // 32-bit address
pub const PCI_BAR_64BIT: u8 = 0x04; // 64-bit address
pub const PCI_BAR_PREFETCHABLE: u8 = 0x08; // Prefetchable

/// PCI device structure
#[derive(Debug, Clone)]
pub struct PciDevice {
    /// Bus number (0 for single bus)
    pub bus: u8,
    /// Device number (0 for first device)
    pub device: u8,
    /// Function number (0 for single function)
    pub function: u8,
    /// Vendor ID
    pub vendor_id: u16,
    /// Device ID
    pub device_id: u16,
    /// Class code
    pub class_code: u8,
    /// Subclass code
    pub subclass_code: u8,
    /// Programming interface
    pub prog_if: u8,
    /// BARs (Base Address Registers)
    pub bars: [u64; 6],
    /// BAR sizes
    pub bar_sizes: [u64; 6],
    /// BAR flags (memory/io, 32/64bit, prefetchable)
    pub bar_flags: [u8; 6],
    /// Interrupt line
    pub interrupt_line: u8,
    /// Interrupt pin
    pub interrupt_pin: u8,
}

impl PciDevice {
    /// Create new PCI VGA device
    pub fn new_vga() -> Self {
        Self {
            bus: 0,
            device: 0x02, // VGA device at device 2 (common practice)
            function: 0,
            vendor_id: PCI_VENDOR_ID_BOCHS,
            device_id: PCI_DEVICE_ID_VGA,
            class_code: PCI_CLASS_DISPLAY_VGA,
            subclass_code: PCI_SUBCLASS_VGA_COMPATIBLE,
            prog_if: 0x00,
            bars: [
                0xE0000000, // BAR0: Framebuffer address (32MB)
                0x00000000, // BAR1: MMIO registers
                0x00000000, // BAR2: Reserved
                0x00000000, // BAR3: Reserved
                0x00000000, // BAR4: Reserved
                0x00000000, // BAR5: Reserved
            ],
            bar_sizes: [
                0x02000000, // BAR0: 32MB framebuffer
                0x00100000, // BAR1: 1MM MMIO
                0x00000000, // BAR2-5: Not implemented
                0x00000000, 0x00000000, 0x00000000,
            ],
            bar_flags: [
                PCI_BAR_MEMORY_SPACE | PCI_BAR_32BIT | PCI_BAR_PREFETCHABLE, // BAR0
                PCI_BAR_MEMORY_SPACE | PCI_BAR_32BIT,                        // BAR1
                0,
                0,
                0,
                0, // BAR2-5
            ],
            interrupt_line: 0x0A, // IRQ 10
            interrupt_pin: 0x01,  // INTA#
        }
    }

    /// Get BDF (Bus:Device:Function) encoded value
    pub fn bdf(&self) -> u32 {
        ((self.bus as u32) << 8) | ((self.device as u32) << 3) | (self.function as u32)
    }

    /// Read PCI configuration space register
    pub fn config_read(&self, reg: u8) -> u32 {
        match reg {
            0x00 => (self.device_id as u32) << 16 | (self.vendor_id as u32),
            0x04 => PCI_COMMAND_MEMORY_SPACE as u32 | PCI_COMMAND_BUS_MASTER as u32,
            0x08 => {
                (self.prog_if as u32) << 24
                    | (self.subclass_code as u32) << 16
                    | (self.class_code as u32) << 8
            }
            0x0C => 0x00,                // Cache line size
            0x0D => 0x00,                // Latency timer
            0x0E => 0x00,                // Header type (0 = normal)
            0x0F => 0x00,                // BIST (built-in self test)
            0x10 => self.bars[0] as u32, // BAR0 (framebuffer)
            0x14 => self.bars[1] as u32, // BAR1 (MMIO)
            0x18 => 0x00,                // BAR2
            0x1C => 0x00,                // BAR3
            0x20 => 0x00,                // BAR4
            0x24 => 0x00,                // BAR5
            0x28 => 0x00,                // Cardbus CIS pointer
            0x2C => 0x00,                // Subsystem vendor ID
            0x30 => 0x00,                // Expansion ROM base address
            0x34 => 0x40,                // Capabilities pointer
            0x38 => 0x00,                // Reserved
            0x3C => (self.interrupt_pin as u32) << 8 | (self.interrupt_line as u32), // Interrupt
            0x40 => 0x00,                // Capability ID (no capabilities)
            _ => 0x00,
        }
    }

    /// Write PCI configuration space register
    pub fn config_write(&mut self, reg: u8, value: u32) {
        match reg {
            0x10 => {
                // BAR0 - Framebuffer address
                let mask = if self.bar_flags[0] & PCI_BAR_MEMORY_SPACE != 0 {
                    !0xFu64 // Memory BAR has lower 4 bits reserved
                } else {
                    !0x3u64 // I/O BAR has lower 2 bits reserved
                };
                self.bars[0] = (value as u64 & mask) | (self.bars[0] & !mask);
                log::info!("PCI: BAR0 written = {:#010X}", self.bars[0]);
            }
            0x14 => {
                // BAR1 - MMIO address
                let mask = !0xFu64;
                self.bars[1] = (value as u64 & mask) | (self.bars[1] & !mask);
                log::info!("PCI: BAR1 written = {:#010X}", self.bars[1]);
            }
            0x3C => {
                // Interrupt line
                self.interrupt_line = (value & 0xFF) as u8;
                log::info!("PCI: Interrupt line = {}", self.interrupt_line);
            }
            _ => {
                log::warn!(
                    "PCI: Write to read-only register {:#02X} = {:#010X}",
                    reg,
                    value
                );
            }
        }
    }
}

/// PCI configuration space access
#[derive(Debug)]
pub struct PciConfigSpace {
    /// PCI devices on the bus
    devices: Vec<PciDevice>,
    /// Last address written to CONFIG_ADDRESS
    config_address: u32,
}

impl PciConfigSpace {
    /// Create new PCI configuration space
    pub fn new() -> Self {
        let mut devices = Vec::new();
        devices.push(PciDevice::new_vga());

        Self {
            devices,
            config_address: 0x80000000, // Enable bit set
        }
    }

    /// Read CONFIG_ADDRESS port
    pub fn read_config_address(&self) -> u32 {
        self.config_address
    }

    /// Write CONFIG_ADDRESS port
    pub fn write_config_address(&mut self, value: u32) {
        self.config_address = value;
    }

    /// Read from CONFIG_DATA port
    pub fn read_config_data(&self) -> u32 {
        // Decode CONFIG_ADDRESS
        let bus = ((self.config_address >> 16) & 0xFF) as u8;
        let device = ((self.config_address >> 11) & 0x1F) as u8;
        let function = ((self.config_address >> 8) & 0x07) as u8;
        let reg = ((self.config_address >> 2) & 0x3F) as u8;

        // Enable bit must be set
        if self.config_address & 0x80000000 == 0 {
            log::warn!("PCI: CONFIG_ADDRESS enable bit not set");
            return 0xFFFFFFFF;
        }

        // Find device
        for dev in &self.devices {
            if dev.bus == bus && dev.device == device && dev.function == function {
                let value = dev.config_read(reg);
                log::debug!(
                    "PCI: Read BDF={:02X}:{:02X}.{:X} reg={:#02X} = {:#010X}",
                    bus,
                    device,
                    function,
                    reg,
                    value
                );
                return value;
            }
        }

        // Device not present
        log::warn!(
            "PCI: Device not present BDF={:02X}:{:02X}.{:X}",
            bus,
            device,
            function
        );
        0xFFFFFFFF
    }

    /// Write to CONFIG_DATA port
    pub fn write_config_data(&mut self, value: u32) {
        // Decode CONFIG_ADDRESS
        let bus = ((self.config_address >> 16) & 0xFF) as u8;
        let device = ((self.config_address >> 11) & 0x1F) as u8;
        let function = ((self.config_address >> 8) & 0x07) as u8;
        let reg = ((self.config_address >> 2) & 0x3F) as u8;

        // Enable bit must be set
        if self.config_address & 0x80000000 == 0 {
            log::warn!("PCI: CONFIG_ADDRESS enable bit not set");
            return;
        }

        // Find device
        for mut dev in self.devices.iter_mut() {
            if dev.bus == bus && dev.device == device && dev.function == function {
                log::debug!(
                    "PCI: Write BDF={:02X}:{:02X}.{:X} reg={:#02X} = {:#010X}",
                    bus,
                    device,
                    function,
                    reg,
                    value
                );
                dev.config_write(reg, value);
                return;
            }
        }

        log::warn!(
            "PCI: Device not present BDF={:02X}:{:02X}.{:X}",
            bus,
            device,
            function
        );
    }

    /// Get framebuffer address from VGA device
    pub fn get_framebuffer_address(&self) -> u64 {
        for dev in &self.devices {
            if dev.class_code == PCI_CLASS_DISPLAY_VGA
                && dev.subclass_code == PCI_SUBCLASS_VGA_COMPATIBLE
            {
                return dev.bars[0];
            }
        }
        0xE0000000 // Default fallback
    }

    /// Initialize framebuffer in guest memory
    pub fn initialize_framebuffer(&self, mmu: &mut dyn MMU) -> VmResult<()> {
        let fb_addr = self.get_framebuffer_address();
        let fb_size = 0x02000000; // 32MB

        log::info!("=== Initializing PCI Framebuffer ===");
        log::info!("Framebuffer address: {:#010X}", fb_addr);
        log::info!("Framebuffer size: {} MB", fb_size / 1024 / 1024);

        // Clear framebuffer to black
        for i in (0..fb_size).step_by(4096) {
            mmu.write(GuestAddr(fb_addr + i), 0, 8)?;
        }

        log::info!("✓ Framebuffer initialized");

        // Write test pattern (top line white)
        for i in 0..(1920 * 4) {
            mmu.write(GuestAddr(fb_addr + i as u64), 0xFFFFFFFF, 4)?;
        }

        log::info!("✓ Test pattern written (white top line)");

        Ok(())
    }

    /// Get PCI device info for debugging
    pub fn device_info(&self) -> String {
        let mut info = String::from("PCI Devices:\n");
        for dev in &self.devices {
            info.push_str(&format!(
                "  {:02X}:{:02X}.{}: {:04X}:{:04X} VGA Controller\n",
                dev.bus, dev.device, dev.function, dev.vendor_id, dev.device_id
            ));
            info.push_str(&format!(
                "    BAR0: {:#010X} ({} MB framebuffer)\n",
                dev.bars[0],
                dev.bar_sizes[0] / 1024 / 1024
            ));
            info.push_str(&format!("    Interrupt: IRQ {}\n", dev.interrupt_line));
        }
        info
    }

    /// Capture framebuffer data via MMU
    pub fn capture_framebuffer(&self, mmu: &dyn MMU) -> VmResult<(u64, usize, Vec<u8>)> {
        let fb_addr = self.get_framebuffer_address();
        let fb_size = 0x02000000; // 32MB max

        log::debug!(
            "Capturing framebuffer @ {:#010X} ({} bytes)",
            fb_addr,
            fb_size
        );

        let mut fb_data = Vec::with_capacity(fb_size);
        let mut non_zero_count = 0;

        // Read framebuffer memory
        for i in 0..fb_size {
            match mmu.read(GuestAddr(fb_addr + i as u64), 1) {
                Ok(val) => {
                    let byte = val as u8;
                    fb_data.push(byte);
                    if byte != 0 {
                        non_zero_count += 1;
                    }
                }
                Err(e) => {
                    log::debug!(
                        "Failed to read framebuffer @ {:#010X}: {}",
                        fb_addr + i as u64,
                        e
                    );
                    break;
                }
            }
        }

        log::debug!(
            "Framebuffer captured: {} bytes, {} non-zero",
            fb_data.len(),
            non_zero_count
        );

        Ok((fb_addr, fb_data.len(), fb_data))
    }
}

impl Default for PciConfigSpace {
    fn default() -> Self {
        Self::new()
    }
}
