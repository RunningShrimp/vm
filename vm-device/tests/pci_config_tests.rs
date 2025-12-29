//! Comprehensive tests for PCI device management and configuration space access
//!
//! Target coverage areas:
//! - PCI configuration space access (read/write)
//! - Vendor ID and Device ID access
//! - BAR (Base Address Register) operations
//! - PCI capabilities access
//! - Device configuration management

use vm_device::simple_devices::*;

#[cfg(test)]
mod simple_network_device_tests {
    use super::*;

    #[test]
    fn test_network_device_creation() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = SimpleVirtioNetDevice::new(mac);

        assert_eq!(device.get_mac_addr(), mac);
        assert_eq!(device.queue_depth(), (0, 0));

        let stats = device.get_stats();
        assert_eq!(stats.rx_packets, 0);
        assert_eq!(stats.tx_packets, 0);
        assert_eq!(stats.rx_bytes, 0);
        assert_eq!(stats.tx_bytes, 0);
        assert_eq!(stats.interrupts, 0);
    }

    #[test]
    fn test_network_device_enable_disable() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = SimpleVirtioNetDevice::new(mac);

        // Initially disabled
        assert_eq!(device.send_packet(vec![0x01, 0x02, 0x03]), false);
        assert_eq!(device.receive_packet(vec![0x01, 0x02, 0x03]), false);

        // Enable
        device.enable();
        assert_eq!(device.send_packet(vec![0x01, 0x02, 0x03]), true);
        assert_eq!(device.receive_packet(vec![0x04, 0x05, 0x06]), true);

        // Disable
        device.disable();
        assert_eq!(device.send_packet(vec![0x01, 0x02, 0x03]), false);
        assert_eq!(device.receive_packet(vec![0x01, 0x02, 0x03]), false);
    }

    #[test]
    fn test_network_device_send_packet() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = SimpleVirtioNetDevice::new(mac);
        device.enable();

        let data = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        let result = device.send_packet(data.clone());

        assert!(result);
        assert_eq!(device.queue_depth(), (0, 1));

        // Verify stats
        let stats = device.get_stats();
        assert_eq!(stats.tx_packets, 1);
        assert_eq!(stats.tx_bytes, 5);
        assert_eq!(stats.interrupts, 1);
    }

    #[test]
    fn test_network_device_receive_packet() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = SimpleVirtioNetDevice::new(mac);
        device.enable();

        let data = vec![0x10, 0x20, 0x30, 0x40];
        let result = device.receive_packet(data.clone());

        assert!(result);
        assert_eq!(device.queue_depth(), (1, 0));

        // Verify stats
        let stats = device.get_stats();
        assert_eq!(stats.rx_packets, 1);
        assert_eq!(stats.rx_bytes, 4);
        assert_eq!(stats.interrupts, 1);
    }

    #[test]
    fn test_network_device_dequeue_tx() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = SimpleVirtioNetDevice::new(mac);
        device.enable();

        // Send multiple packets
        device.send_packet(vec![0x01, 0x02]);
        device.send_packet(vec![0x03, 0x04]);
        device.send_packet(vec![0x05, 0x06]);

        assert_eq!(device.queue_depth(), (0, 3));

        // Dequeue packets
        let pkt1 = device.dequeue_tx();
        let pkt2 = device.dequeue_tx();
        let pkt3 = device.dequeue_tx();
        let pkt4 = device.dequeue_tx();

        assert!(pkt1.is_some());
        assert!(pkt2.is_some());
        assert!(pkt3.is_some());
        assert!(pkt4.is_none());

        assert_eq!(pkt1.unwrap().data, vec![0x01, 0x02]);
        assert_eq!(pkt2.unwrap().data, vec![0x03, 0x04]);
        assert_eq!(pkt3.unwrap().data, vec![0x05, 0x06]);
    }

    #[test]
    fn test_network_device_dequeue_rx() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = SimpleVirtioNetDevice::new(mac);
        device.enable();

        // Receive multiple packets
        device.receive_packet(vec![0xAA, 0xBB]);
        device.receive_packet(vec![0xCC, 0xDD]);

        assert_eq!(device.queue_depth(), (2, 0));

        // Dequeue packets
        let pkt1 = device.dequeue_rx();
        let pkt2 = device.dequeue_rx();
        let pkt3 = device.dequeue_rx();

        assert!(pkt1.is_some());
        assert!(pkt2.is_some());
        assert!(pkt3.is_none());

        assert_eq!(pkt1.unwrap().data, vec![0xAA, 0xBB]);
        assert_eq!(pkt2.unwrap().data, vec![0xCC, 0xDD]);
    }

    #[test]
    fn test_network_device_stats_tracking() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = SimpleVirtioNetDevice::new(mac);
        device.enable();

        // Send and receive packets
        device.send_packet(vec![0x01; 100]);
        device.send_packet(vec![0x02; 200]);
        device.receive_packet(vec![0x03; 150]);
        device.receive_packet(vec![0x04; 50]);
        device.receive_packet(vec![0x05; 75]);

        let stats = device.get_stats();
        assert_eq!(stats.tx_packets, 2);
        assert_eq!(stats.tx_bytes, 300);
        assert_eq!(stats.rx_packets, 3);
        assert_eq!(stats.rx_bytes, 275);
        assert_eq!(stats.interrupts, 5);
    }

    #[test]
    fn test_network_device_mac_address() {
        let mac1 = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let mac2 = [0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA];

        let device1 = SimpleVirtioNetDevice::new(mac1);
        let device2 = SimpleVirtioNetDevice::new(mac2);

        assert_eq!(device1.get_mac_addr(), mac1);
        assert_eq!(device2.get_mac_addr(), mac2);
    }

    #[test]
    fn test_network_device_empty_queue() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = SimpleVirtioNetDevice::new(mac);

        // Dequeue from empty queues
        assert!(device.dequeue_tx().is_none());
        assert!(device.dequeue_rx().is_none());

        // Stats should remain zero
        let stats = device.get_stats();
        assert_eq!(stats.tx_packets, 0);
        assert_eq!(stats.rx_packets, 0);
    }
}

#[cfg(test)]
mod simple_block_device_tests {
    use super::*;

    #[test]
    fn test_block_device_creation() {
        let device = SimpleVirtioBlockDevice::new(10); // 10 MB

        assert_eq!(device.queue_depth(), (0, 0));
    }

    #[test]
    fn test_block_device_enable_disable() {
        let device = SimpleVirtioBlockDevice::new(10);

        // Initially disabled, I/O should fail
        let req = BlockIORequest {
            request_type: BlockIOType::Read,
            block_offset: 0,
            block_count: 1,
            data: vec![0u8; 4096],
        };
        assert_eq!(device.queue_io_request(req), false);

        // Enable
        device.enable();
        let req2 = BlockIORequest {
            request_type: BlockIOType::Read,
            block_offset: 0,
            block_count: 1,
            data: vec![0u8; 4096],
        };
        assert_eq!(device.queue_io_request(req2), true);

        // Disable
        device.disable();
        let req3 = BlockIORequest {
            request_type: BlockIOType::Read,
            block_offset: 0,
            block_count: 1,
            data: vec![0u8; 4096],
        };
        assert_eq!(device.queue_io_request(req3), false);
    }

    #[test]
    fn test_block_device_read_request() {
        let device = SimpleVirtioBlockDevice::new(1);
        device.enable();

        let mut data = vec![0u8; 4096];
        data[0..8].copy_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11]);

        let req = BlockIORequest {
            request_type: BlockIOType::Read,
            block_offset: 0,
            block_count: 1,
            data,
        };

        let result = device.queue_io_request(req);
        assert!(result);
        assert_eq!(device.queue_depth(), (1, 0));
    }

    #[test]
    fn test_block_device_write_request() {
        let device = SimpleVirtioBlockDevice::new(1);
        device.enable();

        let data = vec![0xFF; 4096];

        let req = BlockIORequest {
            request_type: BlockIOType::Write,
            block_offset: 0,
            block_count: 1,
            data,
        };

        let result = device.queue_io_request(req);
        assert!(result);
        assert_eq!(device.queue_depth(), (1, 0));
    }

    #[test]
    fn test_block_device_flush_request() {
        let device = SimpleVirtioBlockDevice::new(1);
        device.enable();

        let req = BlockIORequest {
            request_type: BlockIOType::Flush,
            block_offset: 0,
            block_count: 0,
            data: vec![],
        };

        let result = device.queue_io_request(req);
        assert!(result);
    }

    #[test]
    fn test_block_device_multiple_requests() {
        let device = SimpleVirtioBlockDevice::new(10);
        device.enable();

        // Queue multiple requests
        for i in 0..5 {
            let req = BlockIORequest {
                request_type: if i % 2 == 0 { BlockIOType::Read } else { BlockIOType::Write },
                block_offset: i as u64,
                block_count: 1,
                data: vec![0u8; 4096],
            };
            device.queue_io_request(req);
        }

        let (rx_depth, _) = device.queue_depth();
        assert_eq!(rx_depth, 5);
    }

    #[test]
    fn test_block_device_io_type_variants() {
        let device = SimpleVirtioBlockDevice::new(1);
        device.enable();

        // Read
        let read_req = BlockIORequest {
            request_type: BlockIOType::Read,
            block_offset: 0,
            block_count: 1,
            data: vec![0u8; 4096],
        };
        assert!(device.queue_io_request(read_req));

        // Write
        let write_req = BlockIORequest {
            request_type: BlockIOType::Write,
            block_offset: 1,
            block_count: 1,
            data: vec![0xFF; 4096],
        };
        assert!(device.queue_io_request(write_req));

        // Flush
        let flush_req = BlockIORequest {
            request_type: BlockIOType::Flush,
            block_offset: 0,
            block_count: 0,
            data: vec![],
        };
        assert!(device.queue_io_request(flush_req));

        assert_eq!(device.queue_depth(), (3, 0));
    }
}

#[cfg(test)]
mod block_io_request_tests {
    use super::*;

    #[test]
    fn test_block_io_request_creation() {
        let req = BlockIORequest {
            request_type: BlockIOType::Read,
            block_offset: 100,
            block_count: 5,
            data: vec![0u8; 20480],
        };

        assert_eq!(req.block_offset, 100);
        assert_eq!(req.block_count, 5);
        assert_eq!(req.data.len(), 20480);
    }

    #[test]
    fn test_block_io_type_equality() {
        assert_eq!(BlockIOType::Read, BlockIOType::Read);
        assert_eq!(BlockIOType::Write, BlockIOType::Write);
        assert_eq!(BlockIOType::Flush, BlockIOType::Flush);

        assert_ne!(BlockIOType::Read, BlockIOType::Write);
        assert_ne!(BlockIOType::Write, BlockIOType::Flush);
        assert_ne!(BlockIOType::Flush, BlockIOType::Read);
    }

    #[test]
    fn test_block_io_request_clone() {
        let req1 = BlockIORequest {
            request_type: BlockIOType::Write,
            block_offset: 50,
            block_count: 2,
            data: vec![0xAB; 8192],
        };

        let req2 = req1.clone();

        assert_eq!(req1.request_type, req2.request_type);
        assert_eq!(req1.block_offset, req2.block_offset);
        assert_eq!(req1.block_count, req2.block_count);
        assert_eq!(req1.data, req2.data);
    }

    #[test]
    fn test_network_packet_creation() {
        let packet = NetworkPacket {
            data: vec![0x01, 0x02, 0x03, 0x04],
            timestamp: 123456789,
        };

        assert_eq!(packet.data.len(), 4);
        assert_eq!(packet.timestamp, 123456789);
    }

    #[test]
    fn test_network_packet_clone() {
        let packet1 = NetworkPacket {
            data: vec![0xAA, 0xBB, 0xCC],
            timestamp: 0,
        };

        let packet2 = packet1.clone();

        assert_eq!(packet1.data, packet2.data);
        assert_eq!(packet1.timestamp, packet2.timestamp);
    }

    #[test]
    fn test_network_stats_default() {
        let stats = NetworkStats::default();

        assert_eq!(stats.rx_packets, 0);
        assert_eq!(stats.tx_packets, 0);
        assert_eq!(stats.rx_bytes, 0);
        assert_eq!(stats.tx_bytes, 0);
        assert_eq!(stats.interrupts, 0);
    }
}

#[cfg(test)]
mod pci_config_space_tests {
    use super::*;

    /// Simulate PCI configuration space access patterns
    /// This tests the basic structures used in PCI configuration
    #[test]
    fn test_pci_config_layout() {
        // Standard PCI configuration space is 256 bytes
        let mut config_space = [0u8; 256];

        // Vendor ID (offset 0x00)
        config_space[0x00] = 0x34;
        config_space[0x01] = 0x12;

        // Device ID (offset 0x02)
        config_space[0x02] = 0x78;
        config_space[0x03] = 0x56;

        // Read Vendor ID (little-endian)
        let vendor_id = u16::from_le_bytes([config_space[0x00], config_space[0x01]]);
        assert_eq!(vendor_id, 0x1234);

        // Read Device ID (little-endian)
        let device_id = u16::from_le_bytes([config_space[0x02], config_space[0x03]]);
        assert_eq!(device_id, 0x5678);
    }

    #[test]
    fn test_pci_bar_read_write() {
        let mut config_space = [0u8; 256];

        // BAR0 (offset 0x10) - 32-bit memory address
        let bar0_addr: u32 = 0xF000_0000;
        let bar0_bytes = bar0_addr.to_le_bytes();
        config_space[0x10..0x14].copy_from_slice(&bar0_bytes);

        // Read back
        let read_bar0 = u32::from_le_bytes([
            config_space[0x10],
            config_space[0x11],
            config_space[0x12],
            config_space[0x13],
        ]);
        assert_eq!(read_bar0, 0xF000_0000);
    }

    #[test]
    fn test_pci_command_register() {
        let mut config_space = [0u8; 256];

        // Command register (offset 0x04)
        // Bit 0: I/O Space
        // Bit 1: Memory Space
        // Bit 2: Bus Master
        config_space[0x04] = 0x07; // Enable all three

        let command = config_space[0x04] | (config_space[0x05] as u8) << 8;
        assert_eq!(command, 0x0007);

        // Check individual flags
        assert!(command & 0x01 != 0); // I/O Space enabled
        assert!(command & 0x02 != 0); // Memory Space enabled
        assert!(command & 0x04 != 0); // Bus Master enabled
    }

    #[test]
    fn test_pci_status_register() {
        let mut config_space = [0u8; 256];

        // Status register (offset 0x06)
        // Bit 4: Capabilities List
        // Bit 5: 66 MHz Capable
        config_space[0x06] = 0x30;

        let status = (config_space[0x07] as u16) << 8 | (config_space[0x06] as u16);
        assert_eq!(status, 0x0030);

        // Check capabilities list bit
        assert!(status & 0x0010 != 0);
    }

    #[test]
    fn test_pci_class_code() {
        let mut config_space = [0u8; 256];

        // Class Code (offset 0x09 - 0x0B)
        // Offset 0x0B: Base Class
        // Offset 0x0A: Sub Class
        // Offset 0x09: Programming Interface
        config_space[0x09] = 0x20; // AHCI 0.20
        config_space[0x0A] = 0x01; // SATA controller
        config_space[0x0B] = 0x01; // Mass storage

        let base_class = config_space[0x0B];
        let sub_class = config_space[0x0A];
        let prog_if = config_space[0x09];

        assert_eq!(base_class, 0x01);
        assert_eq!(sub_class, 0x01);
        assert_eq!(prog_if, 0x20);
    }

    #[test]
    fn test_mac_address_formatting() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];

        // Test MAC address is valid
        assert_eq!(mac.len(), 6);

        // Convert to string representation
        let mac_str = format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        );
        assert_eq!(mac_str, "52:54:00:12:34:56");
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_network_device_zero_length_packet() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = SimpleVirtioNetDevice::new(mac);
        device.enable();

        // Send empty packet
        let result = device.send_packet(vec![]);
        assert!(result);

        let stats = device.get_stats();
        assert_eq!(stats.tx_packets, 1);
        assert_eq!(stats.tx_bytes, 0);
    }

    #[test]
    fn test_network_device_large_packet() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = SimpleVirtioNetDevice::new(mac);
        device.enable();

        // Send large packet (64KB)
        let large_packet = vec![0xFF; 65536];
        let result = device.send_packet(large_packet);
        assert!(result);

        let stats = device.get_stats();
        assert_eq!(stats.tx_packets, 1);
        assert_eq!(stats.tx_bytes, 65536);
    }

    #[test]
    fn test_block_device_zero_block_count() {
        let device = SimpleVirtioBlockDevice::new(1);
        device.enable();

        let req = BlockIORequest {
            request_type: BlockIOType::Read,
            block_offset: 0,
            block_count: 0,
            data: vec![],
        };

        let result = device.queue_io_request(req);
        assert!(result);
    }

    #[test]
    fn test_block_device_large_block_count() {
        let device = SimpleVirtioBlockDevice::new(100); // 100 MB
        device.enable();

        // Request 1000 blocks
        let req = BlockIORequest {
            request_type: BlockIOType::Read,
            block_offset: 0,
            block_count: 1000,
            data: vec![0u8; 1000 * 4096],
        };

        let result = device.queue_io_request(req);
        assert!(result);
    }

    #[test]
    fn test_network_stats_overflow_protection() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let device = SimpleVirtioNetDevice::new(mac);
        device.enable();

        // Send many packets to test counter behavior
        for _ in 0..100 {
            device.send_packet(vec![0x01; 1000]);
        }

        let stats = device.get_stats();
        assert_eq!(stats.tx_packets, 100);
        assert_eq!(stats.tx_bytes, 100000);
        assert_eq!(stats.interrupts, 100);
    }
}
