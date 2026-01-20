//! # ACPI (Advanced Configuration and Power Interface) Support
//!
//! Minimal ACPI implementation for x86_64 hardware discovery.
//! Provides essential tables for OS hardware detection (Ubuntu 25.10).

use vm_core::{GuestAddr, MMU, VmError, VmResult};

/// ACPI Signature constants
pub const ACPI_SIGNATURE_RSDP: &[u8; 8] = b"RSD PTR ";
pub const ACPI_SIGNATURE_RSDT: &[u8; 4] = b"RSDT";
pub const ACPI_SIGNATURE_FADT: &[u8; 4] = b"FACP";
pub const ACPI_SIGNATURE_MADT: &[u8; 4] = b"APIC";

/// RSDP (Root System Description Pointer) location
pub const RSDP_ADDRESS: u64 = 0xE0000; // Standard location in EBDA area
/// Alternative RSDP location
pub const RSDP_ADDRESS_ALT: u64 = 0x000F_6DC0; // In low memory

/// ACPI Table Header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AcpiTableHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

/// RSDP (Root System Description Pointer) - ACPI 2.0+
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Rsdp {
    pub signature: [u8; 8],    // "RSD PTR "
    pub checksum: u8,          // ACPI 1.0 checksum
    pub oem_id: [u8; 6],       // OEM identifier
    pub revision: u8,          // ACPI revision (2 for ACPI 2.0+)
    pub rsdt_address: u32,     // Physical address of RSDT (ACPI 1.0)
    pub length: u32,           // Length of RSDP (ACPI 2.0+)
    pub xsdt_address: u64,     // Physical address of XSDT (ACPI 2.0+)
    pub extended_checksum: u8, // Checksum of entire RSDP (ACPI 2.0+)
    pub reserved: [u8; 3],     // Reserved
}

/// RSDT (Root System Description Table)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Rsdt {
    pub header: AcpiTableHeader,
    pub table_offsets: Vec<u32>, // Pointers to other ACPI tables
}

/// FADT (Fixed ACPI Description Table)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Fadt {
    pub header: AcpiTableHeader,
    pub firmware_ctrl: u32, // Physical address of FACS
    pub dsdt: u32,          // Physical address of DSDT
    pub reserved: u8,
    pub preferred_pm_profile: u8,
    pub sci_int: u16,
    pub smi_cmd: u32,
    pub acpi_enable: u8,
    pub acpi_disable: u8,
    pub s4bios_req: u8,
    pub pstate_cnt: u8,
    pub pm1a_evt_blk: u32,
    pub pm1b_evt_blk: u32,
    pub pm1a_cnt_blk: u32,
    pub pm1b_cnt_blk: u32,
    pub pm2_cnt_blk: u32,
    pub pm_tmr_blk: u32,
    pub gpe0_blk: u32,
    pub gpe1_blk: u32,
    pub pm1_evt_len: u8,
    pub pm1_cnt_len: u8,
    pub pm2_cnt_len: u8,
    pub pm_tmr_len: u8,
    pub gpe0_blk_len: u8,
    pub gpe1_blk_len: u8,
    pub gpe1_base: u8,
    pub cst_cnt: u8,
    pub p_lvl2_lat: u16,
    pub p_lvl3_lat: u16,
    pub flush_size: u16,
    pub flush_stride: u16,
    pub duty_offset: u8,
    pub duty_width: u8,
    pub day_alrm: u8,
    pub mon_alrm: u8,
    pub century: u8,
    pub iapc_boot_arch: u16,
    pub reserved2: u8,
    pub flags: u32,
    pub reset_reg: u64,
    pub reset_value: u8,
    pub arm_boot_arch: u16,
    pub fadt_minor_version: u8,
    pub x_firmware_ctrl: u64,
    pub x_dsdt: u64,
    pub x_pm1a_evt_blk: u64,
    pub x_pm1b_evt_blk: u64,
    pub x_pm1a_cnt_blk: u64,
    pub x_pm1b_cnt_blk: u64,
    pub x_pm2_cnt_blk: u64,
    pub x_pm_tmr_blk: u64,
    pub x_gpe0_blk: u64,
    pub x_gpe1_blk: u64,
    pub sleep_control_reg: u64,
    pub sleep_status_reg: u64,
    pub hypervisor_vendor_id: u64,
}

/// MADT (Multiple APIC Description Table)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Madt {
    pub header: AcpiTableHeader,
    pub local_apic_address: u32,
    pub flags: u32,
    pub entries: Vec<u8>, // Variable-length controller entries
}

/// ACPI Manager
pub struct AcpiManager {
    /// Tables loaded in guest memory
    tables_loaded: bool,
}

impl AcpiManager {
    /// Create new ACPI manager
    pub fn new() -> Self {
        Self {
            tables_loaded: false,
        }
    }

    /// Install ACPI tables in guest memory
    pub fn install_tables(&mut self, mmu: &mut dyn MMU) -> VmResult<()> {
        log::info!("=== Installing ACPI Tables ===");

        // Define table addresses
        let rsdp_addr = RSDP_ADDRESS;
        let rsdt_addr = 0x000F_6000;
        let fadt_addr = 0x000F_6100;
        let dsdt_addr = 0x000F_6200;
        let madt_addr = 0x000F_7000;

        // Create RSDP
        let rsdp = self.create_rsdp(rsdt_addr);
        self.write_rsdp(mmu, rsdp_addr, rsdp)?;

        // Create RSDT
        let rsdt = self.create_rsdt(vec![fadt_addr, madt_addr]);
        self.write_rsdt(mmu, rsdt_addr, rsdt)?;

        // Create FADT
        let fadt = self.create_fadt(dsdt_addr as u32);
        self.write_fadt(mmu, fadt_addr, fadt)?;

        // Create minimal DSDT
        self.create_dsdt(mmu, dsdt_addr)?;

        // Create MADT
        let madt = self.create_madt();
        self.write_madt(mmu, madt_addr, madt)?;

        self.tables_loaded = true;

        log::info!("✓ ACPI Tables installed:");
        log::info!("  RSDP @ {:#010X}", rsdp_addr);
        log::info!("  RSDT @ {:#010X}", rsdt_addr);
        log::info!("  FADT @ {:#010X}", fadt_addr);
        log::info!("  DSDT @ {:#010X}", dsdt_addr);
        log::info!("  MADT @ {:#010X}", madt_addr);

        Ok(())
    }

    /// Create RSDP structure
    fn create_rsdp(&self, rsdt_address: u64) -> Rsdp {
        let mut rsdp = Rsdp {
            signature: *ACPI_SIGNATURE_RSDP,
            checksum: 0, // Will calculate
            oem_id: *b"RUSTVM",
            revision: 2, // ACPI 2.0
            rsdt_address: rsdt_address as u32,
            length: 36,      // ACPI 2.0+ RSDP length
            xsdt_address: 0, // Not using XSDT
            extended_checksum: 0,
            reserved: [0; 3],
        };

        // Calculate checksum
        rsdp.checksum = Self::calculate_checksum(&rsdp as *const Rsdp as *const u8, 20);

        rsdp
    }

    /// Create RSDT structure
    fn create_rsdt(&self, table_offsets: Vec<u32>) -> Rsdt {
        let num_tables = table_offsets.len();
        let rsdt_length = (std::mem::size_of::<AcpiTableHeader>() + num_tables * 4) as u32;

        Rsdt {
            header: AcpiTableHeader {
                signature: *ACPI_SIGNATURE_RSDT,
                length: rsdt_length,
                revision: 1,
                checksum: 0,
                oem_id: *b"RUSTVM",
                oem_table_id: *b"RUSTVM  ",
                oem_revision: 1,
                creator_id: 1,
                creator_revision: 1,
            },
            table_offsets,
        }
    }

    /// Create FADT structure
    fn create_fadt(&self, dsdt_address: u32) -> Fadt {
        Fadt {
            header: AcpiTableHeader {
                signature: *ACPI_SIGNATURE_FADT,
                length: 276, // ACPI 5.1 FADT length
                revision: 5,
                checksum: 0,
                oem_id: *b"RUSTVM",
                oem_table_id: *b"RUSTVM  ",
                oem_revision: 1,
                creator_id: 1,
                creator_revision: 1,
            },
            firmware_ctrl: 0,
            dsdt: dsdt_address as u32,
            reserved: 0,
            preferred_pm_profile: 0, // Unspecified
            sci_int: 9,
            smi_cmd: 0,
            acpi_enable: 0,
            acpi_disable: 0,
            s4bios_req: 0,
            pstate_cnt: 0,
            pm1a_evt_blk: 0,
            pm1b_evt_blk: 0,
            pm1a_cnt_blk: 0,
            pm1b_cnt_blk: 0,
            pm2_cnt_blk: 0,
            pm_tmr_blk: 0,
            gpe0_blk: 0,
            gpe1_blk: 0,
            pm1_evt_len: 0,
            pm1_cnt_len: 0,
            pm2_cnt_len: 0,
            pm_tmr_len: 0,
            gpe0_blk_len: 0,
            gpe1_blk_len: 0,
            gpe1_base: 0,
            cst_cnt: 0,
            p_lvl2_lat: 0,
            p_lvl3_lat: 0,
            flush_size: 0,
            flush_stride: 0,
            duty_offset: 0,
            duty_width: 0,
            day_alrm: 0,
            mon_alrm: 0,
            century: 0,
            iapc_boot_arch: 0, // No legacy devices
            reserved2: 0,
            flags: 0,
            reset_reg: 0,
            reset_value: 0,
            arm_boot_arch: 0,
            fadt_minor_version: 1,
            x_firmware_ctrl: 0,
            x_dsdt: dsdt_address as u64,
            x_pm1a_evt_blk: 0,
            x_pm1b_evt_blk: 0,
            x_pm1a_cnt_blk: 0,
            x_pm1b_cnt_blk: 0,
            x_pm2_cnt_blk: 0,
            x_pm_tmr_blk: 0,
            x_gpe0_blk: 0,
            x_gpe1_blk: 0,
            sleep_control_reg: 0,
            sleep_status_reg: 0,
            hypervisor_vendor_id: 0,
        }
    }

    /// Create minimal DSDT
    fn create_dsdt(&self, mmu: &mut dyn MMU, addr: u64) -> VmResult<()> {
        // Minimal DSDT with just header and basic definition block
        let dsdt_data = [
            // DSDT Header
            0x44, 0x53, 0x44, 0x54, // Signature "DSDT"
            0x20, 0x00, 0x00, 0x00, // Length (32 bytes)
            0x01, // Revision
            0x00, // Checksum (will fix)
            0x52, 0x55, 0x53, 0x54, 0x56, 0x4D, // OEM ID "RUSTVM"
            0x52, 0x54, 0x56, 0x4D, 0x44, 0x53, 0x44, 0x54, // OEM Table ID "RTVMDSDT"
            0x01, 0x00, 0x00, 0x00, // OEM Revision
            0x43, 0x52, 0x45, 0x41, // Creator ID "CREA"
            0x01, 0x00, 0x00, 0x00, // Creator Revision
            // Definition Block
            0x10, 0x2C, // ScopeOp
            0x5C, 0x2F, 0x04, 0x5F, 0x47, 0x50, 0x45, // NamePath: \_GPE
            0x08, // NameOp
            0x5F, 0x53, 0x31, 0x5F, // NameString: _S1_
            0x12, 0x0A, 0x04, // PackageOp
            0x01, // NumElements
            0x00, 0x00, 0x00, 0x00, // PkgLength (4)
            0x00, // ByteData: Sleep state 1 (S1)
        ];

        let dsdt_len = dsdt_data.len();
        for (i, &byte) in dsdt_data.iter().enumerate() {
            mmu.write(GuestAddr(addr + i as u64), byte as u64, 1)?;
        }

        log::info!("  DSDT: {} bytes written", dsdt_len);
        Ok(())
    }

    /// Create MADT structure
    fn create_madt(&self) -> Madt {
        let mut entries = Vec::new();

        // Local APIC entry (Type 0)
        let local_apic: [u8; 8] = [
            0x00, // Type: Local APIC
            0x08, // Length: 8 bytes
            0x00, // ACPI Processor UID (0)
            0x01, // Local APIC ID (1)
            0x01, // Flags: Enabled
            0x00, 0x00, 0x00, // Reserved
        ];
        entries.extend_from_slice(&local_apic);

        // I/O APIC entry (Type 1)
        let io_apic: [u8; 12] = [
            0x01, // Type: I/O APIC
            0x0C, // Length: 12 bytes
            0x00, // I/O APIC ID (0)
            0x00, // Reserved
            0x00, 0x00, 0xEC, 0x0F, // I/O APIC Address: 0xFEC00000 (little-endian)
            0x00, // Global System Interrupt Base (0)
            0x00, 0x00, 0x00,
        ];
        entries.extend_from_slice(&io_apic);

        // Interrupt Source Override (Type 2) for IRQ 0
        let iso0: [u8; 10] = [
            0x02, // Type: Interrupt Source Override
            0x0A, // Length: 10 bytes
            0x00, // Bus (ISA)
            0x00, // Source (IRQ 0)
            0x00, 0x00, 0x00, 0x00, // Global System Interrupt (0)
            0x02, // Flags: Level-triggered, active low
            0x00,
        ];
        entries.extend_from_slice(&iso0);

        let madt_length = (std::mem::size_of::<AcpiTableHeader>() + entries.len()) as u32;

        Madt {
            header: AcpiTableHeader {
                signature: *ACPI_SIGNATURE_MADT,
                length: madt_length,
                revision: 3,
                checksum: 0,
                oem_id: *b"RUSTVM",
                oem_table_id: *b"RUSTVM  ",
                oem_revision: 1,
                creator_id: 1,
                creator_revision: 1,
            },
            local_apic_address: 0xFEE0_0000, // Standard Local APIC address
            flags: 0x01,                     // PC-AT compatibility
            entries,
        }
    }

    /// Write RSDP to memory
    fn write_rsdp(&self, mmu: &mut dyn MMU, addr: u64, rsdp: Rsdp) -> VmResult<()> {
        let rsdp_bytes = unsafe {
            std::slice::from_raw_parts(
                &rsdp as *const Rsdp as *const u8,
                std::mem::size_of::<Rsdp>(),
            )
        };

        for (i, &byte) in rsdp_bytes.iter().enumerate() {
            mmu.write(GuestAddr(addr + i as u64), byte as u64, 1)?;
        }

        log::info!("  RSDP: 20 bytes written to {:#010X}", addr);
        Ok(())
    }

    /// Write RSDT to memory
    fn write_rsdt(&self, mmu: &mut dyn MMU, addr: u64, rsdt: Rsdt) -> VmResult<()> {
        let rsdt_bytes = unsafe {
            std::slice::from_raw_parts(
                &rsdt as *const Rsdt as *const u8,
                std::mem::size_of::<Rsdt>(),
            )
        };

        for (i, &byte) in rsdt_bytes.iter().enumerate() {
            mmu.write(GuestAddr(addr + i as u64), byte as u64, 1)?;
        }

        // Write table offsets
        let header_size = std::mem::size_of::<AcpiTableHeader>();
        for (i, &offset) in rsdt.table_offsets.iter().enumerate() {
            let offset_addr = addr + header_size as u64 + (i * 4) as u64;
            mmu.write(GuestAddr(offset_addr), offset as u64, 4)?;
        }

        log::info!(
            "  RSDT: {} bytes written to {:#010X}",
            rsdt_bytes.len(),
            addr
        );
        Ok(())
    }

    /// Write FADT to memory
    fn write_fadt(&self, mmu: &mut dyn MMU, addr: u32, fadt: Fadt) -> VmResult<()> {
        let fadt_bytes = unsafe {
            std::slice::from_raw_parts(
                &fadt as *const Fadt as *const u8,
                std::mem::size_of::<Fadt>(),
            )
        };

        let addr_base = addr as u64;
        for (i, &byte) in fadt_bytes.iter().enumerate() {
            mmu.write(GuestAddr(addr_base + i as u64), byte as u64, 1)?;
        }

        log::info!(
            "  FADT: {} bytes written to {:#010X}",
            fadt_bytes.len(),
            addr
        );
        Ok(())
    }

    /// Write MADT to memory
    fn write_madt(&self, mmu: &mut dyn MMU, addr: u32, madt: Madt) -> VmResult<()> {
        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                &madt.header as *const AcpiTableHeader as *const u8,
                std::mem::size_of::<AcpiTableHeader>(),
            )
        };

        let addr_base = addr as u64;
        // Write header
        for (i, &byte) in header_bytes.iter().enumerate() {
            mmu.write(GuestAddr(addr_base + i as u64), byte as u64, 1)?;
        }

        // Write entries
        let entries_addr = addr_base + header_bytes.len() as u64;
        for (i, &byte) in madt.entries.iter().enumerate() {
            mmu.write(GuestAddr(entries_addr + i as u64), byte as u64, 1)?;
        }

        log::info!(
            "  MADT: {} bytes written to {:#010X}",
            header_bytes.len() + madt.entries.len(),
            addr
        );
        Ok(())
    }

    /// Calculate ACPI checksum
    fn calculate_checksum(data: *const u8, length: usize) -> u8 {
        let mut sum: u8 = 0;
        for i in 0..length {
            unsafe {
                sum = sum.wrapping_add(*data.add(i));
            }
        }
        (!sum).wrapping_add(1)
    }
}

impl Default for AcpiManager {
    fn default() -> Self {
        Self::new()
    }
}
