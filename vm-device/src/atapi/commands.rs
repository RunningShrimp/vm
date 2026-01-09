//! # ATAPI Commands
//!
//! ATA Packet Interface command definitions.

/// ATAPI command opcodes
pub mod opcodes {
    pub const TEST_UNIT_READY: u8 = 0x00;
    pub const REQUEST_SENSE: u8 = 0x03;
    pub const INQUIRY: u8 = 0x12;
    pub const READ_10: u8 = 0x28;
    pub const READ_CD: u8 = 0xBE;
    pub const READ_TOC: u8 = 0x43;
    pub const READ_CAPACITY: u8 = 0x25;
    pub const START_STOP_UNIT: u8 = 0x1B;
    pub const PREVENT_ALLOW_MEDIUM_REMOVAL: u8 = 0x1E;
    pub const SEEK_10: u8 = 0x2B;
}

/// ATAPI packet commands
#[derive(Debug, Clone)]
pub enum AtapiCommand {
    /// TEST UNIT READY - Check if device is ready
    TestUnitReady,

    /// INQUIRY - Get device information
    Inquiry,

    /// READ (10-byte) - Read sectors
    Read {
        /// Logical Block Address
        lba: u32,
        /// Number of sectors to read
        count: u8,
    },

    /// READ CD - Read CD sectors
    ReadCd {
        /// Logical Block Address
        lba: u32,
        /// Number of sectors to read
        count: u8,
    },

    /// READ TOC - Read Table of Contents
    ReadToc,

    /// READ CAPACITY - Get disk capacity
    ReadCapacity,

    /// START STOP UNIT - Start/stop the device
    StartStopUnit {
        /// Start bit (1 = start, 0 = stop)
        start: bool,
        /// Load/eject bit
        load_eject: bool,
    },

    /// Unknown command
    Unknown,
}

impl AtapiCommand {
    /// Parse command from CDB (Command Descriptor Block)
    pub fn from_cdb(cdb: &[u8]) -> Self {
        if cdb.is_empty() {
            return AtapiCommand::Unknown;
        }

        match cdb[0] {
            opcodes::TEST_UNIT_READY => AtapiCommand::TestUnitReady,

            opcodes::INQUIRY => AtapiCommand::Inquiry,

            opcodes::READ_10 => {
                let lba = u32::from_be_bytes([cdb[2], cdb[3], cdb[4], cdb[5]]);
                let count = cdb[8];
                AtapiCommand::Read { lba, count }
            }

            opcodes::READ_CD => {
                let lba = u32::from_be_bytes([cdb[2], cdb[3], cdb[4], cdb[5]]);
                let count = cdb[8];
                AtapiCommand::ReadCd { lba, count }
            }

            opcodes::READ_TOC => AtapiCommand::ReadToc,

            opcodes::READ_CAPACITY => AtapiCommand::ReadCapacity,

            opcodes::START_STOP_UNIT => {
                let start = (cdb[4] & 0x01) != 0;
                let load_eject = (cdb[4] & 0x02) != 0;
                AtapiCommand::StartStopUnit { start, load_eject }
            }

            _ => AtapiCommand::Unknown,
        }
    }

    /// Get CDB (Command Descriptor Block) for this command
    pub fn to_cdb(&self) -> Vec<u8> {
        match self {
            AtapiCommand::TestUnitReady => {
                vec![opcodes::TEST_UNIT_READY, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            }

            AtapiCommand::Inquiry => vec![opcodes::INQUIRY, 0, 0, 0, 0, 36, 0, 0, 0, 0, 0, 0],

            AtapiCommand::Read { lba, count } => {
                let lba = *lba;
                let count = *count;
                let mut cdb = vec![0u8; 12];
                cdb[0] = opcodes::READ_10;
                cdb[2] = (lba >> 24) as u8;
                cdb[3] = (lba >> 16) as u8;
                cdb[4] = (lba >> 8) as u8;
                cdb[5] = lba as u8;
                cdb[8] = count;
                cdb
            }

            AtapiCommand::ReadCd { lba, count } => {
                let lba = *lba;
                let count = *count;
                let mut cdb = vec![0u8; 12];
                cdb[0] = opcodes::READ_CD;
                cdb[2] = (lba >> 24) as u8;
                cdb[3] = (lba >> 16) as u8;
                cdb[4] = (lba >> 8) as u8;
                cdb[5] = lba as u8;
                cdb[8] = count;
                cdb[9] = 0x10; // User data
                cdb
            }

            AtapiCommand::ReadToc => {
                let mut cdb = vec![0u8; 12];
                cdb[0] = opcodes::READ_TOC;
                cdb[6] = 0x00; // Format 0
                cdb[8] = 0x00; // MSB of response length
                cdb[9] = 0x0C; // LSB of response length (12 bytes)
                cdb
            }

            AtapiCommand::ReadCapacity => {
                vec![opcodes::READ_CAPACITY, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            }

            AtapiCommand::StartStopUnit { start, load_eject } => {
                let start = *start;
                let load_eject = *load_eject;
                let mut cdb = vec![0u8; 12];
                cdb[0] = opcodes::START_STOP_UNIT;
                cdb[4] = if start { 0x01 } else { 0x00 };
                if load_eject {
                    cdb[4] |= 0x02;
                }
                cdb
            }

            AtapiCommand::Unknown => vec![0; 12],
        }
    }

    /// Get CDB length
    pub fn cdb_length(&self) -> usize {
        match self {
            AtapiCommand::Read { .. } | AtapiCommand::ReadCd { .. } => 12,
            AtapiCommand::StartStopUnit { .. } => 12,
            _ => 12,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_unit_ready() {
        let cmd = AtapiCommand::TestUnitReady;
        let cdb = cmd.to_cdb();
        assert_eq!(cdb[0], opcodes::TEST_UNIT_READY);
    }

    #[test]
    fn test_inquiry() {
        let cmd = AtapiCommand::Inquiry;
        let cdb = cmd.to_cdb();
        assert_eq!(cdb[0], opcodes::INQUIRY);
        assert_eq!(cdb[6], 36); // Allocation length
    }

    #[test]
    fn test_read_command() {
        let cmd = AtapiCommand::Read {
            lba: 0x1000,
            count: 10,
        };
        let cdb = cmd.to_cdb();
        assert_eq!(cdb[0], opcodes::READ_10);
        assert_eq!(cdb[2], 0x00);
        assert_eq!(cdb[3], 0x00);
        assert_eq!(cdb[4], 0x10);
        assert_eq!(cdb[5], 0x00);
        assert_eq!(cdb[8], 10);
    }

    #[test]
    fn test_read_toc() {
        let cmd = AtapiCommand::ReadToc;
        let cdb = cmd.to_cdb();
        assert_eq!(cdb[0], opcodes::READ_TOC);
    }

    #[test]
    fn test_read_capacity() {
        let cmd = AtapiCommand::ReadCapacity;
        let cdb = cmd.to_cdb();
        assert_eq!(cdb[0], opcodes::READ_CAPACITY);
    }

    #[test]
    fn test_parse_read_command() {
        let mut cdb = vec![0u8; 12];
        cdb[0] = opcodes::READ_10;
        cdb[2] = 0x00;
        cdb[3] = 0x01;
        cdb[4] = 0x00;
        cdb[5] = 0x00;
        cdb[8] = 5;

        let cmd = AtapiCommand::from_cdb(&cdb);
        match cmd {
            AtapiCommand::Read { lba, count } => {
                assert_eq!(lba, 0x010000);
                assert_eq!(count, 5);
            }
            _ => panic!("Failed to parse READ command"),
        }
    }
}
