//! 优化的VirtIO队列实现
//!
//! 包含本地镜像和批量刷新机制，提高VirtIO队列性能

use vm_core::{MMU, VmError, GuestAddr};
use crate::mmu_util::MmuUtil;

/// 优化的VirtIO队列
#[derive(Debug, Clone)]
pub struct VirtioQueueOptimized {
    /// 现有字段
    pub desc_addr: GuestAddr,
    pub avail_addr: GuestAddr,
    pub used_addr: GuestAddr,
    pub size: u16,
    
    /// 本地avail索引镜像
    pub avail_shadow: Vec<u16>,
    
    /// 本地used索引镜像
    pub used_shadow: Vec<u16>,
    
    /// 批量刷新阈值
    pub batch_threshold: usize,
    
    /// 待刷新的avail索引
    pub pending_avail: Vec<u16>,
    
    /// 待刷新的used索引
    pub pending_used: Vec<u16>,
    
    /// 队列状态
    pub last_avail_idx: u16,
    pub last_used_idx: u16,
}

impl VirtioQueueOptimized {
    /// 创建新的优化队列
    pub fn new(
        desc_addr: GuestAddr,
        avail_addr: GuestAddr,
        used_addr: GuestAddr,
        size: u16,
    batch_threshold: usize,
    ) -> Self {
        Self {
            desc_addr,
            avail_addr,
            used_addr,
            size,
            avail_shadow: Vec::with_capacity(size as usize),
            used_shadow: Vec::with_capacity(size as usize),
            batch_threshold,
            pending_avail: Vec::with_capacity(batch_threshold),
            pending_used: Vec::with_capacity(batch_threshold),
            last_avail_idx: 0,
            last_used_idx: 0,
        }
    }
    
    /// 添加本地avail索引镜像
    pub fn add_avail_shadow(&mut self, idx: u16) {
        self.avail_shadow.push(idx);
        if self.avail_shadow.len() >= self.batch_threshold {
            self.flush_avail().unwrap_or_else(|e| {
                eprintln!("Failed to flush avail shadow: {}", e);
                0
            });
        }
    }
    
    /// 添加本地used索引镜像
    pub fn add_used_shadow(&mut self, idx: u16) {
        self.used_shadow.push(idx);
        if self.used_shadow.len() >= self.batch_threshold {
            self.flush_used().unwrap_or_else(|e| {
                eprintln!("Failed to flush used shadow: {}", e);
                0
            });
        }
    }
    
    /// 批量刷新avail索引到内存
    pub fn flush_avail(&mut self) -> Result<(), VmError> {
        if self.pending_avail.is_empty() && !self.avail_shadow.is_empty() {
            return Ok(());
        }
        
        // 合并待刷新的索引和本地镜像
        self.pending_avail.extend_from_slice(&self.avail_shadow);
        self.avail_shadow.clear();
        
        // 批量写入到内存
        if !self.pending_avail.is_empty() {
            self.write_avail_batch()?;
            self.pending_avail.clear();
        }
        
        Ok(())
    }
    
    /// 批量刷新used索引到内存
    pub fn flush_used(&mut self) -> Result<(), VmError> {
        if self.pending_used.is_empty() && !self.used_shadow.is_empty() {
            return Ok(());
        }
        
        // 合并待刷新的索引和本地镜像
        self.pending_used.extend_from_slice(&self.used_shadow);
        self.used_shadow.clear();
        
        // 批量写入到内存
        if !self.pending_used.is_empty() {
            self.write_used_batch()?;
            self.pending_used.clear();
        }
        
        Ok(())
    }
    
    /// 强制刷新所有待处理的索引
    pub fn flush_all(&mut self) -> Result<(), VmError> {
        self.flush_avail()?;
        self.flush_used()?;
        Ok(())
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> VirtioQueueStats {
        VirtioQueueStats {
            avail_shadow_size: self.avail_shadow.len(),
            used_shadow_size: self.used_shadow.len(),
            pending_avail_size: self.pending_avail.len(),
            pending_used_size: self.pending_used.len(),
            last_avail_idx: self.last_avail_idx,
            last_used_idx: self.last_used_idx,
        }
    }
    
    /// 批量写入avail索引
    fn write_avail_batch(&mut self) -> Result<(), VmError> {
        // 按照VirtIO协议写入avail索引
        let avail_flags_offset = 2; // avail flags
        let avail_idx_offset = 0; // avail idx
        
        for &idx in &self.pending_avail {
            // 写入avail ring的flags
            self.write_u16(
                self.avail_addr + avail_flags_offset as u64,
                idx | (1 << 15), // 设置标志位
            )?;
            
            // 写入avail ring的index
            self.write_u16(
                self.avail_addr + avail_idx_offset as u64,
                idx,
            )?;
        }
        
        // 更新last_avail_idx
        if let Some(&last_idx) = self.pending_avail.last() {
            self.last_avail_idx = *last_idx;
        }
        
        Ok(())
    }
    
    /// 批量写入used索引
    fn write_used_batch(&mut self) -> Result<(), VmError> {
        for &idx in &self.pending_used {
            self.write_used_entry(*idx)?;
        }
        
        // 更新last_used_idx
        if let Some(&last_idx) = self.pending_used.last() {
            self.last_used_idx = *last_idx;
        }
        
        Ok(())
    }
    
    /// 写入used条目
    fn write_used_entry(&mut self, idx: u16) -> Result<(), VmError> {
        // 按照VirtIO协议写入used ring
        let used_idx_offset = 0; // used idx
        let used_len_offset = 2; // used len
        let used_id_offset = 4; // used id
        
        // 写入used ring的index
        self.write_u16(
            self.used_addr + used_idx_offset as u64,
            idx,
        )?;
        
        // 写入used ring的len (1 = 1, id is ignored)
        self.write_u32(
            self.used_addr + used_len_offset as u64,
            1,
        )?;
        
        // 写入used ring的id
        self.write_u32(
            self.used_addr + used_id_offset as u64,
            idx as u32,
        )?;
        
        Ok(())
    }
    
    /// 读取u16（辅助函数）
    fn read_u16(&self, addr: GuestAddr) -> Result<u16, VmError> {
        // 使用MmuUtil的读取方法
        use vm_core::GuestAddr;
        MmuUtil::read_u16(self, addr.0)
    }
    
    /// 读取u32（辅助函数）
    fn read_u32(&self, addr: GuestAddr) -> Result<u32, VmError> {
        // 使用MmuUtil的读取方法
        MmuUtil::read_u32(self, addr.0)
    }
    
    /// 写入u16（辅助函数）
    fn write_u16(&self, addr: GuestAddr, value: u16) -> Result<(), VmError> {
        // 使用MmuUtil的写入方法
        MmuUtil::write_u16(self, addr.0, value)
    }
    
    /// 写入u32（辅助函数）
    fn write_u32(&self, addr: GuestAddr, value: u32) -> Result<(), VmError> {
        // 使用MmuUtil的写入方法
        MmuUtil::write_u32(self, addr.0, value)
    }
}

/// VirtIO队列统计信息
#[derive(Debug, Clone)]
pub struct VirtioQueueStats {
    /// 本地avail索引数量
    pub avail_shadow_size: usize,
    
    /// 本地used索引数量
    pub used_shadow_size: usize,
    
    /// 待刷新的avail索引数量
    pub pending_avail_size: usize,
    
    /// 待刷新的used索引数量
    pub pending_used_size: usize,
    
    /// 最后写入的avail索引
    pub last_avail_idx: u16,
    
    /// 最后写入的used索引
    pub last_used_idx: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_virtio_queue_optimized() {
        let mut queue = VirtioQueueOptimized::new(
            GuestAddr(0x1000),
            GuestAddr(0x2000),
            GuestAddr(0x3000),
            128,
            16,
        );
        
        // 测试添加avail索引
        queue.add_avail_shadow(1);
        assert_eq!(queue.avail_shadow.len(), 1);
        
        // 测试添加used索引
        queue.add_used_shadow(2);
        assert_eq!(queue.used_shadow.len(), 1);
        
        // 测试批量阈值
        for i in 0..15 {
            queue.add_avail_shadow((i + 3) as u16);
        }
        
        // 应该触发批量刷新
        assert_eq!(queue.avail_shadow.len(), 0);
        assert_eq!(queue.pending_avail_size(), 16);
        
        // 获取统计信息
        let stats = queue.get_stats();
        assert_eq!(stats.last_avail_idx, 17);
    }
}

