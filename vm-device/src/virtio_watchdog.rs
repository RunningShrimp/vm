//! VirtIO Watchdog 设备实现//!//! 提供看门狗功能，监控虚拟机健康状态use crate::virtio::{Queue,
//! VirtioDevice};use std::sync::{Arc, Mutex};use std::time::Instant;use
//! vm_core::{AddressTranslator, MemoryAccess, MmioManager, MmuAsAny, MMU, VmError};///
//! 看门狗动作#[derive(Debug, Clone, Copy, PartialEq, Eq)]pub enum WatchdogAction {    ///
//! 重置虚拟机    Reset,    /// 关闭虚拟机    Shutdown,    /// 无动作（仅记录）    None,}/// VirtIO
//! Watchdog 设备pub struct VirtioWatchdog {    /// VirtIO队列    queues: Vec<Queue>,    ///
//! 超时时间（秒）    timeout: u32,    /// 最后喂狗时间    last_ping: Arc<Mutex<Instant>>,    ///
//! 看门狗动作    action: WatchdogAction,    /// 设备状态    device_status: u32,    /// 是否启用
//! enabled: bool,}impl VirtioWatchdog {    /// 创建新的VirtIO Watchdog设备    pub fn new(timeout:
//! u32, action: WatchdogAction) -> Self {        Self {            queues: vec![Queue::new(64); 1],
//! timeout,            last_ping: Arc::new(Mutex::new(Instant::now())),            action,
//! device_status: 0,            enabled: false,        }    }    /// 喂狗（重置计时器）    pub fn
//! ping(&self) -> Result<(), VmError> {        if let Ok(mut last) = self.last_ping.lock() {
//! *last = Instant::now();            Ok(())        } else {
//! Err(VmError::Core(vm_core::CoreError::Internal {                message: "Failed to lock
//! watchdog timer".to_string(),                module: "VirtIO Watchdog".to_string(),
//! }))        }    }    /// 检查是否超时    pub fn check_timeout(&self) -> bool {        if
//! !self.enabled {            return false;        }        if let Ok(last) = self.last_ping.lock()
//! {            let elapsed = last.elapsed();            elapsed.as_secs() >= self.timeout as u64
//! } else {            false        }    }    /// 获取超时时间    pub fn timeout(&self) -> u32 {
//! self.timeout    }    /// 设置超时时间    pub fn set_timeout(&mut self, timeout: u32) {
//! self.timeout = timeout;    }    /// 获取看门狗动作    pub fn action(&self) -> WatchdogAction {
//! self.action    }    /// 设置看门狗动作    pub fn set_action(&mut self, action: WatchdogAction) {
//! self.action = action;    }    /// 启用看门狗    pub fn enable(&mut self) {        self.enabled =
//! true;        if let Ok(mut last) = self.last_ping.lock() {            *last = Instant::now();
//! }    }    /// 禁用看门狗    pub fn disable(&mut self) {        self.enabled = false;    }    ///
//! 是否启用    pub fn is_enabled(&self) -> bool {        self.enabled    }    /// 设置设备状态
//! pub fn set_device_status(&mut self, status: u32) {        self.device_status = status;    }
//! /// 获取设备状态    pub fn device_status(&self) -> u32 {        self.device_status    }}impl
//! VirtioDevice for VirtioWatchdog {    fn device_id(&self) -> u32 {        10 // VirtIO Watchdog
//! device ID (非标准，自定义)    }    fn num_queues(&self) -> usize {        self.queues.len()    }
//! fn get_queue(&mut self, index: usize) -> &mut Queue {        &mut self.queues[index]    }    fn
//! process_queues(&mut self, mmu: &mut dyn MMU) {        // 处理喂狗请求        while let
//! Some(chain) = self.queues[0].pop(mmu) {            // 读取请求数据（如果有）            let mut
//! request_data = Vec::new();            for desc in &chain.descs {                if desc.flags &
//! 0x1 == 0 {                    // 可读                    let mut data = vec![0u8; desc.len as
//! usize];                    if mmu.read_bulk(vm_core::GuestAddr(desc.addr), &mut data).is_ok() {
//! request_data.extend_from_slice(&data);                    }                }            }
//! // 喂狗            if self.ping().is_ok() {                // 标记为已使用
//! self.queues[0].add_used(mmu, chain.head_index, 0);            }        }    }}/// VirtIO
//! Watchdog MMIO设备pub struct VirtioWatchdogMmio {    device: VirtioWatchdog,}impl
//! VirtioWatchdogMmio {    pub fn new(device: VirtioWatchdog) -> Self {        Self { device }    }
//! pub fn device_mut(&mut self) -> &mut VirtioWatchdog {        &mut self.device    }    pub fn
//! device(&self) -> &VirtioWatchdog {        &self.device    }}#[cfg(test)]mod tests {    use
//! super::*;    use std::thread;    use std::time::Duration;    use vm_core::GuestAddr;    #[test]
