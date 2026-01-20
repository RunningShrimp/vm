//! Async VirtIO Block Device Implementation
//!
//! 异步版本的 VirtIO 块设备实现，使用 tokio 进行非阻塞 IO 操作。
//!
//! ## 功能特性
//!
//! - 异步文件读写操作
//! - 请求队列管理
//! - IO 完成回调机制
//!
//! ## 启用方式
//!
//! 在 Cargo.toml 中启用 `async-io` feature:
//! ```toml
//! vm-device = { path = "../vm-device", features = ["async-io"] }
//! ```

use std::path::Path;
use std::sync::Arc;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, mpsc};

/// 异步块设备请求类型
#[derive(Debug, Clone)]
pub enum AsyncBlockRequest {
    /// 读取请求
    Read {
        sector: u64,
        count: u32,
        response_tx: mpsc::Sender<AsyncBlockResponse>,
    },
    /// 写入请求
    Write {
        sector: u64,
        data: Vec<u8>,
        response_tx: mpsc::Sender<AsyncBlockResponse>,
    },
    /// 刷新请求
    Flush {
        response_tx: mpsc::Sender<AsyncBlockResponse>,
    },
    /// 关闭设备
    Shutdown,
}

/// 异步块设备响应
#[derive(Debug)]
pub enum AsyncBlockResponse {
    /// 读取成功
    ReadOk(Vec<u8>),
    /// 写入成功
    WriteOk,
    /// 刷新成功
    FlushOk,
    /// 批量操作成功
    BatchOk(Vec<BatchResult>),
    /// 错误
    Error(String),
}

/// 批量操作结果
#[derive(Debug, Clone)]
pub struct BatchResult {
    /// 操作索引
    pub index: usize,
    /// 是否成功
    pub success: bool,
    /// 读取的数据（仅读操作）
    pub data: Option<Vec<u8>>,
    /// 错误信息
    pub error: Option<String>,
}

/// IO 优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum IoPriority {
    /// 低优先级（后台操作）
    Low = 0,
    /// 普通优先级
    #[default]
    Normal = 1,
    /// 高优先级（交互式操作）
    High = 2,
    /// 实时优先级
    Realtime = 3,
}

/// 批量读取请求
#[derive(Debug, Clone)]
pub struct BatchReadRequest {
    pub sector: u64,
    pub count: u32,
    pub priority: IoPriority,
}

/// 批量写入请求
#[derive(Debug, Clone)]
pub struct BatchWriteRequest {
    pub sector: u64,
    pub data: Vec<u8>,
    pub priority: IoPriority,
}

/// 异步 IO 配置
#[derive(Debug, Clone)]
pub struct AsyncIoConfig {
    /// 请求队列大小
    pub queue_size: usize,
    /// 最大并发请求数
    pub max_concurrent: usize,
    /// 批处理阈值
    pub batch_threshold: usize,
    /// 预读扇区数
    pub readahead_sectors: u32,
    /// 写合并启用
    pub write_coalescing: bool,
    /// IO 调度算法
    pub scheduler: IoScheduler,
}

impl Default for AsyncIoConfig {
    fn default() -> Self {
        Self {
            queue_size: 64,
            max_concurrent: 16,
            batch_threshold: 4,
            readahead_sectors: 32,
            write_coalescing: true,
            scheduler: IoScheduler::Deadline,
        }
    }
}

/// IO 调度算法
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoScheduler {
    /// 先来先服务
    Fifo,
    /// 截止时间调度（默认）
    Deadline,
    /// 优先级调度
    Priority,
    /// CFQ (完全公平队列)
    Cfq,
}

/// 异步 VirtIO 块设备
pub struct AsyncVirtioBlock {
    /// 后端文件
    file: Option<Arc<Mutex<File>>>,
    /// 设备容量（扇区数）
    capacity: u64,
    /// 扇区大小（字节）
    sector_size: u32,
    /// 是否只读
    read_only: bool,
    /// 请求发送通道
    request_tx: Option<mpsc::Sender<AsyncBlockRequest>>,
    /// IO 配置
    config: AsyncIoConfig,
    /// IO 统计
    stats: Arc<Mutex<AsyncIoStats>>,
}

/// 异步 IO 统计信息
#[derive(Debug, Default, Clone)]
pub struct AsyncIoStats {
    /// 总读取操作数
    pub total_reads: u64,
    /// 总写入操作数
    pub total_writes: u64,
    /// 总读取字节数
    pub bytes_read: u64,
    /// 总写入字节数
    pub bytes_written: u64,
    /// 批处理合并次数
    pub batch_merges: u64,
    /// 平均读延迟（纳秒）
    pub avg_read_latency_ns: u64,
    /// 平均写延迟（纳秒）
    pub avg_write_latency_ns: u64,
    /// 队列深度峰值
    pub peak_queue_depth: u32,
    /// 预读命中率
    pub readahead_hit_rate: f64,
}

impl AsyncVirtioBlock {
    /// 创建新的异步 VirtIO Block 设备
    pub fn new() -> Self {
        Self::with_config(AsyncIoConfig::default())
    }

    /// 使用自定义配置创建异步块设备
    pub fn with_config(config: AsyncIoConfig) -> Self {
        Self {
            file: None,
            capacity: 0,
            sector_size: 512,
            read_only: false,
            request_tx: None,
            config,
            stats: Arc::new(Mutex::new(AsyncIoStats::default())),
        }
    }

    /// 获取 IO 统计信息
    pub async fn get_stats(&self) -> AsyncIoStats {
        self.stats.lock().await.clone()
    }

    /// 重置统计信息
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.lock().await;
        *stats = AsyncIoStats::default();
    }

    /// 获取配置
    pub fn config(&self) -> &AsyncIoConfig {
        &self.config
    }

    /// 从文件路径异步打开块设备
    pub async fn open<P: AsRef<Path>>(path: P, read_only: bool) -> std::io::Result<Self> {
        Self::open_with_config(path, read_only, AsyncIoConfig::default()).await
    }

    /// 使用自定义配置从文件路径异步打开块设备
    pub async fn open_with_config<P: AsRef<Path>>(
        path: P,
        read_only: bool,
        config: AsyncIoConfig,
    ) -> std::io::Result<Self> {
        let file = if read_only {
            File::open(path.as_ref()).await?
        } else {
            tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path.as_ref())
                .await?
        };

        let metadata = file.metadata().await?;
        let capacity = metadata.len() / 512;

        Ok(Self {
            file: Some(Arc::new(Mutex::new(file))),
            capacity,
            sector_size: 512,
            read_only,
            request_tx: None,
            config,
            stats: Arc::new(Mutex::new(AsyncIoStats::default())),
        })
    }

    /// 批量读取操作
    pub async fn batch_read(
        &self,
        requests: Vec<BatchReadRequest>,
    ) -> Result<Vec<BatchResult>, String> {
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| "Device not opened".to_string())?;

        let mut results = Vec::with_capacity(requests.len());
        let stats = self.stats.clone();

        // 按优先级排序
        let mut sorted_requests: Vec<_> = requests.into_iter().enumerate().collect();
        sorted_requests.sort_by(|a, b| b.1.priority.cmp(&a.1.priority));

        // 尝试合并相邻的读请求
        let merged_requests = if self.config.write_coalescing {
            Self::merge_read_requests(&sorted_requests)
        } else {
            sorted_requests
                .into_iter()
                .map(|(i, r)| (vec![i], r))
                .collect()
        };

        let mut file_guard = file.lock().await;

        for (indices, req) in merged_requests {
            let offset = req.sector * (self.sector_size as u64);
            let size = (req.count as usize) * (self.sector_size as usize);

            let start = std::time::Instant::now();

            if let Err(e) = file_guard.seek(std::io::SeekFrom::Start(offset)).await {
                for i in indices {
                    results.push(BatchResult {
                        index: i,
                        success: false,
                        data: None,
                        error: Some(format!("Seek failed: {}", e)),
                    });
                }
                continue;
            }

            let mut buffer = vec![0u8; size];
            match file_guard.read_exact(&mut buffer).await {
                Ok(_) => {
                    let latency = start.elapsed().as_nanos() as u64;
                    let mut s = stats.lock().await;
                    s.total_reads += 1;
                    s.bytes_read += size as u64;
                    s.avg_read_latency_ns = (s.avg_read_latency_ns + latency) / 2;
                    drop(s);

                    // 分发数据到各个请求
                    let sector_size = self.sector_size as usize;
                    let mut offset = 0usize;
                    for i in indices {
                        let data_size = req.count as usize * sector_size;
                        results.push(BatchResult {
                            index: i,
                            success: true,
                            data: Some(buffer[offset..offset + data_size].to_vec()),
                            error: None,
                        });
                        offset += data_size;
                    }
                }
                Err(e) => {
                    for i in indices {
                        results.push(BatchResult {
                            index: i,
                            success: false,
                            data: None,
                            error: Some(format!("Read failed: {}", e)),
                        });
                    }
                }
            }
        }

        // 按原始索引排序结果
        results.sort_by_key(|r| r.index);
        Ok(results)
    }

    /// 合并相邻的读请求
    fn merge_read_requests(
        requests: &[(usize, BatchReadRequest)],
    ) -> Vec<(Vec<usize>, BatchReadRequest)> {
        if requests.is_empty() {
            return Vec::new();
        }

        let mut result = Vec::new();
        let mut current_indices = vec![requests[0].0];
        let mut current = requests[0].1.clone();

        for (idx, req) in requests.iter().skip(1) {
            // 检查是否可以合并（连续扇区）
            if req.sector == current.sector + current.count as u64
                && req.priority == current.priority
            {
                current_indices.push(*idx);
                current.count += req.count;
            } else {
                result.push((current_indices, current));
                current_indices = vec![*idx];
                current = req.clone();
            }
        }
        result.push((current_indices, current));
        result
    }

    /// 批量写入操作
    pub async fn batch_write(
        &self,
        requests: Vec<BatchWriteRequest>,
    ) -> Result<Vec<BatchResult>, String> {
        if self.read_only {
            return Err("Device is read-only".to_string());
        }

        let file = self
            .file
            .as_ref()
            .ok_or_else(|| "Device not opened".to_string())?;

        let mut results = Vec::with_capacity(requests.len());
        let stats = self.stats.clone();

        // 按优先级排序
        let mut sorted_requests: Vec<_> = requests.into_iter().enumerate().collect();
        sorted_requests.sort_by(|a, b| b.1.priority.cmp(&a.1.priority));

        let mut file_guard = file.lock().await;

        for (idx, req) in sorted_requests {
            let offset = req.sector * (self.sector_size as u64);
            let start = std::time::Instant::now();

            let result = async {
                file_guard
                    .seek(std::io::SeekFrom::Start(offset))
                    .await
                    .map_err(|e| format!("Seek failed: {}", e))?;
                file_guard
                    .write_all(&req.data)
                    .await
                    .map_err(|e| format!("Write failed: {}", e))?;
                Ok::<(), String>(())
            }
            .await;

            let latency = start.elapsed().as_nanos() as u64;

            match result {
                Ok(()) => {
                    let mut s = stats.lock().await;
                    s.total_writes += 1;
                    s.bytes_written += req.data.len() as u64;
                    s.avg_write_latency_ns = (s.avg_write_latency_ns + latency) / 2;
                    drop(s);

                    results.push(BatchResult {
                        index: idx,
                        success: true,
                        data: None,
                        error: None,
                    });
                }
                Err(e) => {
                    results.push(BatchResult {
                        index: idx,
                        success: false,
                        data: None,
                        error: Some(e),
                    });
                }
            }
        }

        results.sort_by_key(|r| r.index);
        Ok(results)
    }

    /// 启动异步请求处理器
    pub fn start_request_handler(&mut self) -> mpsc::Receiver<AsyncBlockRequest> {
        let (tx, rx) = mpsc::channel(64);
        self.request_tx = Some(tx);
        rx
    }

    /// 获取请求发送器
    pub fn request_sender(&self) -> Option<mpsc::Sender<AsyncBlockRequest>> {
        self.request_tx.clone()
    }

    /// 获取设备容量（扇区数）
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// 获取扇区大小
    pub fn sector_size(&self) -> u32 {
        self.sector_size
    }

    /// 是否只读
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// 异步读取数据
    pub async fn read_async(&self, sector: u64, count: u32) -> Result<Vec<u8>, String> {
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| "Device not opened".to_string())?;

        let mut file_guard = file.lock().await;
        let offset = sector * (self.sector_size as u64);
        let size = (count as usize) * (self.sector_size as usize);

        file_guard
            .seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(|e| format!("Seek failed: {}", e))?;

        let mut buffer = vec![0u8; size];
        file_guard
            .read_exact(&mut buffer)
            .await
            .map_err(|e| format!("Read failed: {}", e))?;

        Ok(buffer)
    }

    /// 异步写入数据
    pub async fn write_async(&self, sector: u64, data: &[u8]) -> Result<(), String> {
        if self.read_only {
            return Err("Device is read-only".to_string());
        }

        let file = self
            .file
            .as_ref()
            .ok_or_else(|| "Device not opened".to_string())?;

        let mut file_guard = file.lock().await;
        let offset = sector * (self.sector_size as u64);

        file_guard
            .seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(|e| format!("Seek failed: {}", e))?;

        file_guard
            .write_all(data)
            .await
            .map_err(|e| format!("Write failed: {}", e))?;

        Ok(())
    }

    /// 异步刷新
    pub async fn flush_async(&self) -> Result<(), String> {
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| "Device not opened".to_string())?;

        let file_guard = file.lock().await;
        file_guard
            .sync_all()
            .await
            .map_err(|e| format!("Flush failed: {}", e))?;

        Ok(())
    }
}

impl Default for AsyncVirtioBlock {
    fn default() -> Self {
        Self::new()
    }
}

/// 异步请求处理器
///
/// 在独立的 tokio 任务中运行，处理来自 VM 的块设备请求
pub async fn run_async_block_handler(
    device: Arc<AsyncVirtioBlock>,
    mut rx: mpsc::Receiver<AsyncBlockRequest>,
) {
    log::info!("Async block handler started");

    while let Some(request) = rx.recv().await {
        match request {
            AsyncBlockRequest::Read {
                sector,
                count,
                response_tx,
            } => {
                let result = device.read_async(sector, count).await;
                let response = match result {
                    Ok(data) => AsyncBlockResponse::ReadOk(data),
                    Err(e) => AsyncBlockResponse::Error(e),
                };
                let _ = response_tx.send(response).await;
            }
            AsyncBlockRequest::Write {
                sector,
                data,
                response_tx,
            } => {
                let result = device.write_async(sector, &data).await;
                let response = match result {
                    Ok(()) => AsyncBlockResponse::WriteOk,
                    Err(e) => AsyncBlockResponse::Error(e),
                };
                let _ = response_tx.send(response).await;
            }
            AsyncBlockRequest::Flush { response_tx } => {
                let result = device.flush_async().await;
                let response = match result {
                    Ok(()) => AsyncBlockResponse::FlushOk,
                    Err(e) => AsyncBlockResponse::Error(e),
                };
                let _ = response_tx.send(response).await;
            }
            AsyncBlockRequest::Shutdown => {
                log::info!("Async block handler shutting down");
                break;
            }
        }
    }

    log::info!("Async block handler stopped");
}

// ============================================================================
// VirtIO MMIO 集成
// ============================================================================

use vm_core::GuestAddr;

/// VirtIO 队列请求（从 Guest 提交）
#[derive(Debug, Clone)]
pub struct VirtioQueueRequest {
    /// 描述符索引
    pub desc_idx: u16,
    /// 请求类型 (0=读, 1=写, 4=刷新)
    pub req_type: u32,
    /// 扇区号
    pub sector: u64,
    /// 数据长度
    pub data_len: u32,
    /// 数据缓冲区地址
    pub data_addr: GuestAddr,
    /// 状态字节地址
    pub status_addr: GuestAddr,
}

/// 异步 VirtIO 块设备 MMIO 接口
pub struct AsyncVirtioBlockMmio {
    /// 异步块设备
    device: Arc<AsyncVirtioBlock>,
    /// 请求发送通道
    request_tx: mpsc::Sender<AsyncBlockRequest>,
    /// 完成通知通道 (用于中断)
    completion_rx: Option<mpsc::Receiver<VirtioCompletion>>,
    completion_tx: mpsc::Sender<VirtioCompletion>,
    /// 当前选中的队列索引
    selected_queue: u32,
    /// 队列大小
    queue_size: u32,
    /// 描述符表地址
    desc_addr: GuestAddr,
    /// Available Ring 地址
    avail_addr: GuestAddr,
    /// Used Ring 地址
    used_addr: GuestAddr,
    /// 设备状态
    device_status: u32,
    /// 驱动特性
    driver_features: u32,
    /// 中断状态
    interrupt_status: u32,
    /// Used Ring 索引
    used_idx: u16,
    /// 待处理的完成事件数
    pending_completions: u32,
}

/// VirtIO 完成通知
#[derive(Debug, Clone)]
pub struct VirtioCompletion {
    /// 描述符索引
    pub desc_idx: u16,
    /// 传输长度
    pub len: u32,
    /// 状态
    pub status: u8,
}

impl AsyncVirtioBlockMmio {
    /// 创建新的异步 VirtIO 块设备 MMIO 接口
    pub fn new(device: Arc<AsyncVirtioBlock>, request_tx: mpsc::Sender<AsyncBlockRequest>) -> Self {
        let (completion_tx, completion_rx) = mpsc::channel(64);
        Self {
            device,
            request_tx,
            completion_rx: Some(completion_rx),
            completion_tx,
            selected_queue: 0,
            queue_size: 128,
            desc_addr: GuestAddr(0),
            avail_addr: GuestAddr(0),
            used_addr: GuestAddr(0),
            device_status: 0,
            driver_features: 0,
            interrupt_status: 0,
            used_idx: 0,
            pending_completions: 0,
        }
    }

    /// 获取完成通知接收器
    pub fn take_completion_rx(&mut self) -> Option<mpsc::Receiver<VirtioCompletion>> {
        self.completion_rx.take()
    }

    /// 获取完成通知发送器 (用于 IO 完成后通知)
    pub fn completion_sender(&self) -> mpsc::Sender<VirtioCompletion> {
        self.completion_tx.clone()
    }

    /// 提交异步读取请求
    pub async fn submit_read(
        &self,
        sector: u64,
        count: u32,
        desc_idx: u16,
    ) -> Result<Vec<u8>, String> {
        let (response_tx, mut response_rx) = mpsc::channel(1);

        self.request_tx
            .send(AsyncBlockRequest::Read {
                sector,
                count,
                response_tx,
            })
            .await
            .map_err(|e| format!("Send failed: {}", e))?;

        match response_rx.recv().await {
            Some(AsyncBlockResponse::ReadOk(data)) => {
                // 发送完成通知
                let _ = self
                    .completion_tx
                    .send(VirtioCompletion {
                        desc_idx,
                        len: data.len() as u32,
                        status: 0, // OK
                    })
                    .await;
                Ok(data)
            }
            Some(AsyncBlockResponse::Error(e)) => {
                let _ = self
                    .completion_tx
                    .send(VirtioCompletion {
                        desc_idx,
                        len: 0,
                        status: 1, // IO_ERR
                    })
                    .await;
                Err(e)
            }
            _ => Err("Unexpected response".to_string()),
        }
    }

    /// 提交异步写入请求
    pub async fn submit_write(
        &self,
        sector: u64,
        data: Vec<u8>,
        desc_idx: u16,
    ) -> Result<(), String> {
        let (response_tx, mut response_rx) = mpsc::channel(1);

        self.request_tx
            .send(AsyncBlockRequest::Write {
                sector,
                data,
                response_tx,
            })
            .await
            .map_err(|e| format!("Send failed: {}", e))?;

        match response_rx.recv().await {
            Some(AsyncBlockResponse::WriteOk) => {
                let _ = self
                    .completion_tx
                    .send(VirtioCompletion {
                        desc_idx,
                        len: 0,
                        status: 0,
                    })
                    .await;
                Ok(())
            }
            Some(AsyncBlockResponse::Error(e)) => {
                let _ = self
                    .completion_tx
                    .send(VirtioCompletion {
                        desc_idx,
                        len: 0,
                        status: 1,
                    })
                    .await;
                Err(e)
            }
            _ => Err("Unexpected response".to_string()),
        }
    }

    /// 提交异步刷新请求
    pub async fn submit_flush(&self, desc_idx: u16) -> Result<(), String> {
        let (response_tx, mut response_rx) = mpsc::channel(1);

        self.request_tx
            .send(AsyncBlockRequest::Flush { response_tx })
            .await
            .map_err(|e| format!("Send failed: {}", e))?;

        match response_rx.recv().await {
            Some(AsyncBlockResponse::FlushOk) => {
                let _ = self
                    .completion_tx
                    .send(VirtioCompletion {
                        desc_idx,
                        len: 0,
                        status: 0,
                    })
                    .await;
                Ok(())
            }
            Some(AsyncBlockResponse::Error(e)) => {
                let _ = self
                    .completion_tx
                    .send(VirtioCompletion {
                        desc_idx,
                        len: 0,
                        status: 1,
                    })
                    .await;
                Err(e)
            }
            _ => Err("Unexpected response".to_string()),
        }
    }

    /// MMIO 读操作
    pub fn read(&self, offset: u64, _size: u8) -> u64 {
        match offset {
            0x000 => 0x74726976, // Magic: "virt"
            0x004 => 0x2,        // Version
            0x008 => 0x2,        // Device ID: block device
            0x00C => 0x554d4551, // Vendor ID: "QEMU"
            0x010 => self.get_features() as u64,
            0x034 => self.queue_size as u64,
            0x044 => self.device_status as u64,
            0x060 => self.interrupt_status as u64,
            0x100 => self.device.capacity(),
            0x108 => self.device.sector_size() as u64,
            _ => 0,
        }
    }

    /// MMIO 写操作
    pub fn write(&mut self, offset: u64, val: u64, _size: u8) {
        match offset {
            0x014 => self.driver_features = val as u32,
            0x030 => self.selected_queue = val as u32,
            0x038 => self.queue_size = val as u32,
            0x044 => self.device_status = val as u32,
            0x064 => self.interrupt_status &= !(val as u32),
            0x080 => self.desc_addr = GuestAddr(val),
            0x090 => self.avail_addr = GuestAddr(val),
            0x0A0 => self.used_addr = GuestAddr(val),
            _ => {}
        }
    }

    /// 获取设备特性
    fn get_features(&self) -> u32 {
        let mut features = 0u32;
        features |= 1 << 6; // VIRTIO_BLK_F_BLK_SIZE
        features |= 1 << 9; // VIRTIO_BLK_F_FLUSH
        if self.device.is_read_only() {
            features |= 1 << 5; // VIRTIO_BLK_F_RO
        }
        features
    }

    /// 处理完成事件，更新 Used Ring
    pub fn process_completion(&mut self, completion: &VirtioCompletion) {
        self.used_idx = self.used_idx.wrapping_add(1);
        self.interrupt_status |= 0x1;
        self.pending_completions = self.pending_completions.saturating_sub(1);
        log::debug!(
            "Async block completion: desc_idx={}, len={}, status={}",
            completion.desc_idx,
            completion.len,
            completion.status
        );
    }

    /// 检查是否有待处理的中断
    pub fn has_pending_interrupt(&self) -> bool {
        self.interrupt_status != 0
    }

    /// 清除中断状态
    pub fn clear_interrupt(&mut self) {
        self.interrupt_status = 0;
    }
}

/// 异步 VirtIO 块设备完成处理器
///
/// 监听完成通知并更新设备状态
pub async fn run_completion_handler(
    device: Arc<Mutex<AsyncVirtioBlockMmio>>,
    mut completion_rx: mpsc::Receiver<VirtioCompletion>,
    interrupt_callback: Option<Box<dyn Fn() + Send>>,
) {
    log::info!("Async completion handler started");

    while let Some(completion) = completion_rx.recv().await {
        {
            let mut dev = device.lock().await;
            dev.process_completion(&completion);
        }

        // 触发中断回调
        if let Some(ref callback) = interrupt_callback {
            callback();
        }
    }

    log::info!("Async completion handler stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_block_creation() {
        let device = AsyncVirtioBlock::new();
        assert_eq!(device.capacity(), 0);
        assert_eq!(device.sector_size(), 512);
        assert!(!device.is_read_only());
    }

    #[tokio::test]
    async fn test_async_mmio_creation() {
        let device = Arc::new(AsyncVirtioBlock::new());
        let (tx, _rx) = mpsc::channel(64);
        let mmio = AsyncVirtioBlockMmio::new(device, tx);

        // 验证 MMIO 寄存器读取
        assert_eq!(mmio.read(0x000, 4), 0x74726976); // Magic
        assert_eq!(mmio.read(0x004, 4), 0x2); // Version
        assert_eq!(mmio.read(0x008, 4), 0x2); // Device ID
    }
}
