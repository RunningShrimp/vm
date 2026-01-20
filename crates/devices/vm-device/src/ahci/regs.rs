//! # AHCI Registers
//!
//! AHCI memory-mapped I/O registers.

/// AHCI Generic Host Control registers
///
/// Offset 0x00-0x2B from ABAR base
#[derive(Debug, Clone)]
pub struct AhciRegs {
    /// 0x00: HBA Capabilities
    pub cap: u32,
    /// 0x03: Global HBA Control
    pub ghc: u32,
    /// 0x04: Interrupt Status
    pub is: u32,
    /// 0x08: Ports Implemented
    pub pi: u32,
    /// 0x0C: AHCI Version
    pub vs: u32,
    /// 0x10: Command Completion Coalescing Control
    pub ccc_ctl: u32,
    /// 0x14: Command Completion Coalescing Ports
    pub ccc_ports: u32,
    /// 0x1C: Enclosure Management Location
    pub em_loc: u32,
    /// 0x20: Enclosure Management Control
    pub em_ctl: u32,
    /// 0x24: HBA Capabilities Extended
    pub cap2: u32,
    /// 0x28: BIOS/OS Handoff Control and Status
    pub bohc: u32,
}

impl AhciRegs {
    /// Create new AHCI registers with default values
    pub fn new() -> Self {
        Self {
            // CAP: Number of command slots (5) = 32, NP = 1 port, SSD = 0, SXS = 0, EMS = 0, CCCS = 0,
            //      NCS = 32-1=31, CCC = 0, PSC = 0, SSS = 0, SAM = 0, SPM = 0, FBSS = 0, PMD = 0, SSC = 0,
            //      PMD = 0, FBS = 0, APST = 0, ALPE = 0, ATP = 0, AL = 0, SXDP = 0
            cap: 0x3F << 8, // NCS = 31 (5 bits set)
            // GHC: AE = 0 (AHCI enable, will be set later), MRSM = 0, IE = 0, HR = 0
            ghc: 0x80000000, // AE bit (bit 31) will be set during init
            is: 0,
            // PI: Port 0 implemented
            pi: 0x00000001,
            // VS: AHCI version 1.0
            vs: 0x00010000,
            ccc_ctl: 0,
            ccc_ports: 0,
            em_loc: 0,
            em_ctl: 0,
            // CAP2: BOH = 0, APST = 0, NVMP = 0, BOH = 0, DESO = 0, SADM = 0, SDS = 0
            cap2: 0,
            // BOHC: BIOS Ownership = 1, OS Ownership = 0
            bohc: 0x00000001,
        }
    }

    /// GHC: Set HBA Enable (AE) bit
    pub fn ghc_set_hba_enable(&mut self) {
        self.ghc |= 0x80000000; // Set bit 31
    }

    /// GHC: Clear HBA Enable (AE) bit
    pub fn ghc_clear_hba_enable(&mut self) {
        self.ghc &= !0x80000000; // Clear bit 31
    }

    /// GHC: Check if HBA is enabled
    pub fn ghc_hba_enabled(&self) -> bool {
        (self.ghc & 0x80000000) != 0
    }

    /// GHC: Set HBA Reset (HR) bit
    pub fn ghc_set_hba_reset(&mut self) {
        self.ghc |= 0x00000001; // Set bit 0
    }

    /// GHC: Clear HBA Reset (HR) bit
    pub fn ghc_clear_hba_reset(&mut self) {
        self.ghc &= !0x00000001; // Clear bit 0
    }

    /// GHC: Check if HBA reset is in progress
    pub fn ghc_hba_reset(&self) -> bool {
        (self.ghc & 0x00000001) != 0
    }

    /// GHC: Set Interrupt Enable (IE) bit
    pub fn ghc_set_interrupt_enable(&mut self) {
        self.ghc |= 0x00000002; // Set bit 1
    }

    /// GHC: Clear Interrupt Enable (IE) bit
    pub fn ghc_clear_interrupt_enable(&mut self) {
        self.ghc &= !0x00000002; // Clear bit 1
    }

    /// GHC: Check if interrupts are enabled
    pub fn ghc_interrupt_enabled(&self) -> bool {
        (self.ghc & 0x00000002) != 0
    }

    /// GHC: Check if MSI (Message Signaled Interrupts) is enabled
    pub fn ghc_msi_enabled(&self) -> bool {
        (self.ghc & 0x00000004) != 0
    }

    /// CAP: Get number of command slots
    pub fn cap_command_slots(&self) -> u32 {
        ((self.cap >> 8) & 0x1F) + 1
    }

    /// CAP: Get number of ports
    pub fn cap_ports(&self) -> u32 {
        (self.cap & 0x1F) + 1
    }

    /// CAP: Check if 64-bit addressing is supported
    pub fn cap_s64a(&self) -> bool {
        (self.cap & 0x00000020) != 0
    }

    /// CAP: Check if NCQ (Native Command Queuing) is supported
    pub fn cap_ncq(&self) -> bool {
        (self.cap & 0x00000010) != 0
    }

    /// CAP: Check if SNotification register is supported
    pub fn cap_sss(&self) -> bool {
        (self.cap & 0x00000008) != 0
    }

    /// CAP: Check if staggered spinup is supported
    pub fn cap_sss2(&self) -> bool {
        (self.cap & 0x00000004) != 0
    }

    /// CAP: Check if SATA GEN 3 (6.0 Gbps) is supported
    pub fn cap_sam(&self) -> bool {
        (self.cap & 0x00000002) != 0
    }

    /// PI: Get ports implemented bitmask
    pub fn pi_ports_implemented(&self) -> u32 {
        self.pi
    }

    /// PI: Check if specific port is implemented
    pub fn pi_is_port_implemented(&self, port: u32) -> bool {
        (self.pi & (1 << port)) != 0
    }

    /// IS: Get global interrupt status
    pub fn is_interrupt_status(&self) -> u32 {
        self.is
    }

    /// IS: Set interrupt for port
    pub fn is_set_port_interrupt(&mut self, port: u32) {
        self.is |= (1 << port);
    }

    /// IS: Clear interrupt for port
    pub fn is_clear_port_interrupt(&mut self, port: u32) {
        self.is &= !(1 << port);
    }

    /// VS: Get AHCI version
    pub fn vs_version(&self) -> (u8, u8) {
        let major = ((self.vs >> 16) & 0xFF) as u8;
        let minor = ((self.vs >> 8) & 0xFF) as u8;
        (major, minor)
    }

    /// CAP2: Check if BIOS has handed off control to OS
    pub fn bohc_os_ownership(&self) -> bool {
        (self.bohc & 0x00000002) != 0
    }

    /// CAP2: Set OS ownership bit
    pub fn bohc_set_os_ownership(&mut self) {
        self.bohc |= 0x00000002;
    }

    /// CAP2: Check if BIOS owns the controller
    pub fn bohc_bios_ownership(&self) -> bool {
        (self.bohc & 0x00000001) != 0
    }

    /// CAP2: Clear BIOS ownership bit
    pub fn bohc_clear_bios_ownership(&mut self) {
        self.bohc &= !0x00000001;
    }
}

impl Default for AhciRegs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ahci_regs_create() {
        let regs = AhciRegs::new();
        assert!(!regs.ghc_hba_enabled());
        assert_eq!(regs.ghc_hba_reset(), false);
        assert_eq!(regs.pi_ports_implemented(), 0x00000001);
    }

    #[test]
    fn test_ahci_regs_hba_enable() {
        let mut regs = AhciRegs::new();
        regs.ghc_set_hba_enable();
        assert!(regs.ghc_hba_enabled());
    }

    #[test]
    fn test_ahci_regs_hba_reset() {
        let mut regs = AhciRegs::new();
        regs.ghc_set_hba_reset();
        assert!(regs.ghc_hba_reset());
        regs.ghc_clear_hba_reset();
        assert!(!regs.ghc_hba_reset());
    }

    #[test]
    fn test_ahci_regs_interrupt_enable() {
        let mut regs = AhciRegs::new();
        regs.ghc_set_interrupt_enable();
        assert!(regs.ghc_interrupt_enabled());
    }

    #[test]
    fn test_ahci_regs_cap() {
        let regs = AhciRegs::new();
        assert!(regs.cap_command_slots() >= 1);
        assert!(regs.cap_ports() >= 1);
    }

    #[test]
    fn test_ahci_regs_pi() {
        let regs = AhciRegs::new();
        assert!(regs.pi_is_port_implemented(0));
        assert!(!regs.pi_is_port_implemented(1));
    }

    #[test]
    fn test_ahci_regs_version() {
        let regs = AhciRegs::new();
        let (major, minor) = regs.vs_version();
        assert_eq!(major, 1);
        assert_eq!(minor, 0);
    }

    #[test]
    fn test_ahci_regs_bohc() {
        let mut regs = AhciRegs::new();
        assert!(regs.bohc_bios_ownership());
        assert!(!regs.bohc_os_ownership());

        regs.bohc_set_os_ownership();
        assert!(regs.bohc_os_ownership());
    }
}
