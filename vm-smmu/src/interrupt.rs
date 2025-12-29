// 中断管理（Interrupt Management）实现
//
// 实现SMMUv3的中断和MSI管理功能，包括：
// - MSI消息处理
// - GERROR中断处理
// - 命令同步中断处理
// - 中断优先级管理

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// 中断类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptType {
    /// 全局错误中断
    GERROR = 0,
    /// PRIQ中断
    PRIQ = 1,
    /// 命令同步中断
    CmdSync = 2,
    /// 流表中断
    STRTBL = 3,
    /// 暂停中断
    STALL = 4,
    /// MSI消息信号中断
    MSI = 5,
}

/// MSI消息
#[derive(Debug, Clone)]
pub struct MsiMessage {
    /// 目标地址
    pub target_address: u64,
    /// 数据字段
    pub data: [u64; 4],
    /// 消息属性
    pub attributes: u32,
}

impl MsiMessage {
    /// 创建新的MSI消息
    pub fn new(target_address: u64, data: [u64; 4], attributes: u32) -> Self {
        Self {
            target_address,
            data,
            attributes,
        }
    }
}

/// 中断控制器
pub struct InterruptController {
    /// MSI配置
    pub msi_enabled: bool,
    /// MSI地址
    pub msi_address: u64,
    /// MSI数据
    pub msi_data: u64,
    /// GERROR中断使能
    pub gerror_enabled: bool,
    /// GERROR中断地址
    pub gerror_address: u64,
    /// 中断队列
    pub interrupt_queue: Arc<Mutex<VecDeque<InterruptRecord>>>,
    /// 中断统计
    pub stats: InterruptStats,
}

/// 中断记录
#[derive(Debug, Clone)]
pub struct InterruptRecord {
    /// 中断类型
    pub interrupt_type: InterruptType,
    /// 时间戳
    pub timestamp: u64,
    /// 数据
    pub data: Vec<u8>,
}

/// 中断统计
#[derive(Debug, Clone)]
pub struct InterruptStats {
    /// 总中断次数
    pub total_interrupts: u64,
    /// GERROR中断次数
    pub gerror_count: u64,
    /// PRIQ中断次数
    pub priq_count: u64,
    /// CMD_SYNC中断次数
    pub cmd_sync_count: u64,
    /// STRTBL中断次数
    pub strtbl_count: u64,
    /// STALL中断次数
    pub stall_count: u64,
    /// MSI中断次数
    pub msi_count: u64,
}

impl Default for InterruptStats {
    fn default() -> Self {
        Self::new()
    }
}

impl InterruptStats {
    pub fn new() -> Self {
        Self {
            total_interrupts: 0,
            gerror_count: 0,
            priq_count: 0,
            cmd_sync_count: 0,
            strtbl_count: 0,
            stall_count: 0,
            msi_count: 0,
        }
    }
}

impl InterruptController {
    /// 创建新的中断控制器
    ///
    /// # 参数
    /// - `msi_enabled`: 是否启用MSI（默认true）
    /// - `gerror_enabled`: 是否启用GERROR（默认true）
    ///
    /// # 示例
    /// ```ignore
    /// let controller = InterruptController::new(true, true);
    /// ```
    pub fn new(msi_enabled: bool, gerror_enabled: bool) -> Self {
        Self {
            msi_enabled,
            msi_address: 0,
            msi_data: 0,
            gerror_enabled,
            gerror_address: 0,
            interrupt_queue: Arc::new(Mutex::new(VecDeque::new())),
            stats: InterruptStats::new(),
        }
    }

    /// Helper: Lock interrupt_queue for write operations
    fn lock_queue(&self) -> std::sync::MutexGuard<'_, VecDeque<InterruptRecord>> {
        self.interrupt_queue
            .lock()
            .expect("Failed to lock interrupt_queue")
    }

    /// 设置MSI配置
    ///
    /// # 参数
    /// - `address`: MSI目标地址
    /// - `data`: MSI数据
    ///
    /// # 示例
    /// ```ignore
    /// controller.set_msi_config(0x1000, 0x200);
    /// ```
    pub fn set_msi_config(&mut self, address: u64, data: u64) {
        self.msi_address = address;
        self.msi_data = data;
    }

    /// 发送MSI消息
    ///
    /// # 参数
    /// - `message`: MSI消息
    ///
    /// # 返回
    /// - `Ok(())`: 发送成功
    /// - `Err(err)`: 发送失败
    ///
    /// # 示例
    /// ```ignore
    /// let message = MsiMessage::new(0x1000, [0, 0, 0, 0], 0);
    /// controller.send_msi(message)?;
    /// ```
    pub fn send_msi(&mut self, message: MsiMessage) -> Result<(), String> {
        if !self.msi_enabled {
            return Err("MSI is not enabled".to_string());
        }

        // 简化的MSI发送：记录到中断队列
        let record = InterruptRecord {
            interrupt_type: InterruptType::MSI,
            timestamp: self.get_timestamp(),
            data: vec![
                (message.target_address >> 24) as u8,
                (message.target_address >> 16) as u8,
                (message.target_address >> 8) as u8,
                message.target_address as u8,
            ],
        };

        self.enqueue_interrupt(record);

        // 更新统计
        self.stats.total_interrupts += 1;
        self.stats.msi_count += 1;

        Ok(())
    }

    /// 处理GERROR中断
    ///
    /// # 示例
    /// ```ignore
    /// controller.handle_gerror();
    /// ```
    pub fn handle_gerror(&mut self) {
        if !self.gerror_enabled {
            return;
        }

        // 创建GERROR中断记录
        let record = InterruptRecord {
            interrupt_type: InterruptType::GERROR,
            timestamp: self.get_timestamp(),
            data: vec![],
        };

        self.enqueue_interrupt(record);

        // 更新统计
        self.stats.total_interrupts += 1;
        self.stats.gerror_count += 1;
    }

    /// 获取下一个中断
    ///
    /// # 返回
    /// - `Some(record)`: 有中断
    /// - `None`: 无中断
    ///
    /// # 示例
    /// ```ignore
    /// if let Some(record) = controller.get_next_interrupt() {
    ///     println!("Interrupt: {:?}", record.interrupt_type);
    /// }
    /// ```
    pub fn get_next_interrupt(&self) -> Option<InterruptRecord> {
        let mut queue = self.lock_queue();
        queue.pop_front()
    }

    /// 检查是否有待处理的中断
    pub fn has_pending_interrupts(&self) -> bool {
        let queue = self.lock_queue();
        !queue.is_empty()
    }

    /// 使能/禁用MSI
    ///
    /// # 参数
    /// - `enabled`: 是否启用MSI
    ///
    /// # 示例
    /// ```ignore
    /// controller.set_msi_enabled(true);
    /// ```
    pub fn set_msi_enabled(&mut self, enabled: bool) {
        self.msi_enabled = enabled;
    }

    /// 使能/禁用GERROR
    ///
    /// # 参数
    /// - `enabled`: 是否启用GERROR
    ///
    /// # 示例
    /// ```ignore
    /// controller.set_gerror_enabled(true);
    /// ```
    pub fn set_gerror_enabled(&mut self, enabled: bool) {
        self.gerror_enabled = enabled;
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> InterruptStats {
        self.stats.clone()
    }

    /// 重置统计
    pub fn reset_stats(&mut self) {
        self.stats = InterruptStats::new();
    }

    /// 入队中断
    fn enqueue_interrupt(&self, record: InterruptRecord) {
        let mut queue = self.lock_queue();
        queue.push_back(record);
    }

    /// 获取当前时间戳
    fn get_timestamp(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_millis() as u64
    }
}

impl std::fmt::Display for InterruptStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "中断统计信息")?;
        writeln!(f, "  总中断次数: {}", self.total_interrupts)?;
        writeln!(f, "  GERROR次数: {}", self.gerror_count)?;
        writeln!(f, "  PRIQ次数: {}", self.priq_count)?;
        writeln!(f, "  CMD_SYNC次数: {}", self.cmd_sync_count)?;
        writeln!(f, "  STRTBL次数: {}", self.strtbl_count)?;
        writeln!(f, "  STALL次数: {}", self.stall_count)?;
        writeln!(f, "  MSI次数: {}", self.msi_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interrupt_controller_creation() {
        let controller = InterruptController::new(true, true);
        assert!(controller.msi_enabled);
        assert!(controller.gerror_enabled);
        assert_eq!(controller.stats.total_interrupts, 0);
    }

    #[test]
    fn test_msi_message_creation() {
        let message = MsiMessage::new(0x1000, [0, 0, 0, 0], 0);
        assert_eq!(message.target_address, 0x1000);
    }

    #[test]
    fn test_msi_config() {
        let mut controller = InterruptController::new(true, true);

        controller.set_msi_config(0x1000, 0x200);
        assert_eq!(controller.msi_address, 0x1000);
        assert_eq!(controller.msi_data, 0x200);
    }

    #[test]
    fn test_send_msi() {
        let mut controller = InterruptController::new(true, true);

        let message = MsiMessage::new(0x1000, [0, 0, 0, 0], 0);
        let result = controller.send_msi(message);

        assert!(result.is_ok());
        assert_eq!(controller.stats.msi_count, 1);
    }

    #[test]
    fn test_send_msi_disabled() {
        let mut controller = InterruptController::new(false, true);

        let message = MsiMessage::new(0x1000, [0, 0, 0, 0], 0);
        let result = controller.send_msi(message);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("MSI is not enabled"));
    }

    #[test]
    fn test_handle_gerror() {
        let mut controller = InterruptController::new(true, true);

        controller.handle_gerror();
        assert_eq!(controller.stats.gerror_count, 1);
        assert!(controller.has_pending_interrupts());
    }

    #[test]
    fn test_get_next_interrupt() {
        let mut controller = InterruptController::new(true, true);

        controller.handle_gerror();

        let record = controller.get_next_interrupt();
        assert!(record.is_some());
        assert_eq!(
            record
                .expect("Expected GERROR interrupt record")
                .interrupt_type,
            InterruptType::GERROR
        );
    }

    #[test]
    fn test_msi_enabled() {
        let mut controller = InterruptController::new(true, true);

        controller.set_msi_enabled(false);
        assert!(!controller.msi_enabled);
    }

    #[test]
    fn test_gerror_enabled() {
        let mut controller = InterruptController::new(true, true);

        controller.set_gerror_enabled(false);
        assert!(!controller.gerror_enabled);
    }

    #[test]
    fn test_interrupt_stats() {
        let controller = InterruptController::new(true, true);

        let stats = controller.get_stats();
        assert_eq!(stats.total_interrupts, 0);
        assert_eq!(stats.gerror_count, 0);
        assert_eq!(stats.msi_count, 0);
    }

    #[test]
    fn test_interrupt_stats_display() {
        let controller = InterruptController::new(true, true);

        let stats = controller.get_stats();
        let display = format!("{}", stats);
        assert!(display.contains("中断统计信息"));
        assert!(display.contains("总中断次数"));
    }

    #[test]
    fn test_reset_stats() {
        let mut controller = InterruptController::new(true, true);

        controller.handle_gerror();
        assert_eq!(controller.stats.gerror_count, 1);

        controller.reset_stats();
        assert_eq!(controller.stats.gerror_count, 0);
    }
}
