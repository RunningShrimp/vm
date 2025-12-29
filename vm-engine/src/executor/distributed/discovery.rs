//! VM Discovery Service
//!
//! This module provides mechanisms for VMs to discover each other and the coordinator.

use crate::executor::distributed::protocol::{VmCapabilities, VmId};
use parking_lot::Mutex;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

/// VM information
#[derive(Debug, Clone)]
pub struct VmInfo {
    pub vm_id: VmId,
    pub vm_addr: SocketAddr,
    pub capabilities: VmCapabilities,
    pub last_seen: std::time::Instant,
    pub cpu_usage: u8,
    pub memory_usage: u8,
}

/// VM Discovery Service
#[derive(Clone)]
pub struct VmDiscovery {
    config: Arc<crate::executor::distributed::architecture::DistributedArchitectureConfig>,
    vm_list: Arc<Mutex<Vec<VmInfo>>>,
    socket: Arc<UdpSocket>,
}

impl VmDiscovery {
    /// Create a new VM discovery service
    pub fn new(
        config: &crate::executor::distributed::architecture::DistributedArchitectureConfig,
    ) -> Result<Self, anyhow::Error> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", config.discovery_port))?;

        Ok(VmDiscovery {
            config: Arc::new(config.clone()),
            vm_list: Arc::new(Mutex::new(Vec::new())),
            socket: Arc::new(socket),
        })
    }

    /// Start the discovery service
    pub async fn start(&self) -> Result<(), anyhow::Error> {
        let socket = self.socket.clone();
        let vm_list = self.vm_list.clone();
        let config = self.config.clone();

        log::info!(
            "Starting VM discovery service on port {}",
            config.discovery_port
        );

        #[cfg(feature = "async")]
        tokio::spawn(async move {
            let mut buf = [0; 1024];

            loop {
                // Set a timeout for receiving messages based on configuration
                if let Err(e) = socket.set_read_timeout(Some(std::time::Duration::from_secs(5))) {
                    log::error!("Failed to set read timeout: {}", e);
                    continue;
                }

                match socket.recv_from(&mut buf) {
                    Ok((len, addr)) => {
                        let msg = &buf[0..len];

                        // Parse the message (in a real implementation, this would be JSON or Protobuf)
                        // For now, we'll mock it
                        if msg.starts_with(b"HELLO") {
                            log::debug!("Received HELLO from {}", addr);

                            // Mock processing of hello message by adding a VM to the list
                            let vm_info = VmInfo {
                                vm_id: VmId::new(),
                                vm_addr: addr,
                                capabilities: VmCapabilities {
                                    cpu_count: 4,
                                    memory_mb: 4096,
                                    instruction_sets: vec!["x86_64".to_string()],
                                    has_gpu: false,
                                },
                                last_seen: std::time::Instant::now(),
                                cpu_usage: 0,
                                memory_usage: 0,
                            };

                            let mut vm_list_lock = vm_list.lock();
                            vm_list_lock.retain(|v| v.vm_addr != vm_info.vm_addr);
                            vm_list_lock.push(vm_info);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // Timeout occurred, continue to next iteration
                        continue;
                    }
                    Err(e) => {
                        log::error!("Discovery error: {}", e);
                        continue;
                    }
                }
            }
        });

        Ok(())
    }

    /// Get the list of active VMs
    pub async fn get_active_vms(&self) -> Vec<VmInfo> {
        let mut vm_list = self.vm_list.lock();

        // Remove inactive VMs (last seen > 30 seconds)
        vm_list.retain(|vm| vm.last_seen.elapsed() < std::time::Duration::from_secs(30));

        vm_list.clone()
    }

    /// Add a VM to the discovery list
    pub async fn add_vm(&self, vm_info: VmInfo) {
        let mut vm_list = self.vm_list.lock();

        // Remove existing entry if present
        vm_list.retain(|v| v.vm_id != vm_info.vm_id);

        // Add new entry
        vm_list.push(vm_info);
    }
}
