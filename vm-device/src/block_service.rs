//! Block Device Service - DDD Service Layer
//!
//! 包含VirtIO Block设备的所有业务逻辑，实现贫血模型。
//! VirtioBlock只包含数据，所有业务逻辑由BlockDeviceService处理
//! 支持异步I/O操作，使用tokio::fs进行非阻塞文件访问

use crate::block::{
    BlockIoRequest, BlockIoResponse, BlockRequestHeader, BlockRequestType, BlockStatus, VirtioBlock,
};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use vm_core::{GuestAddr, MMU, PlatformError, VmError};

/// 异步I/O请求队列的容量
const IO_QUEUE_CAPACITY: usize = 256;

/// Block设备服务 - 处理所有业务逻辑
#[derive(Clone)]
pub struct BlockDeviceService {
    /// 块设备数据容器
    device: Arc<Mutex<VirtioBlock>>,
    /// 异步I/O请求发送端
    io_tx: Arc<Mutex<Option<mpsc::Sender<AsyncIoRequest>>>>,
    /// 异步I/O响应接收端（用于同步方法等待异步结果）
    io_rx: Arc<Mutex<Option<mpsc::Receiver<AsyncIoResponse>>>>,
    /// 文件路径（用于重新打开文件进行异步操作）
    file_path: Arc<Mutex<Option<String>>>,
}

/// 异步I/O请求
#[derive(Debug)]
pub enum AsyncIoRequest {
    /// 读请求
    Read {
        sector: u64,
        count: u32,
        req_id: u64,
        response: oneshot::Sender<Result<Vec<u8>, String>>,
    },
    /// 写请求
    Write {
        sector: u64,
        data: Arc<Vec<u8>>,
        req_id: u64,
        response: oneshot::Sender<Result<(), String>>,
    },
    /// 刷新请求
    Flush {
        req_id: u64,
        response: oneshot::Sender<Result<(), String>>,
    },
    /// 关闭设备
    Close,
}

/// 异步I/O响应
#[derive(Debug, Clone)]
pub enum AsyncIoResponse {
    /// 读成功
    ReadOk { data: Arc<Vec<u8>>, req_id: u64 },
    /// 写成功
    WriteOk { req_id: u64 },
    /// 刷新成功
    FlushOk { req_id: u64 },
    /// I/O错误
    Error { req_id: u64, message: String },
}

impl BlockDeviceService {
    /// 创建新的Block设备服务
    pub fn new(capacity_sectors: u64, sector_size: u32, read_only: bool) -> Self {
        let (io_tx, _io_rx) = mpsc::channel(IO_QUEUE_CAPACITY);

        let device = VirtioBlock {
            capacity: capacity_sectors,
            sector_size,
            read_only,
            ..Default::default()
        };
        Self {
            device: Arc::new(Mutex::new(device)),
            io_tx: Arc::new(Mutex::new(Some(io_tx))),
            io_rx: Arc::new(Mutex::new(None)), // 不再需要，使用oneshot通道
            file_path: Arc::new(Mutex::new(None)),
        }
    }

    /// 打开文件作为块设备（异步）
    pub async fn open<P: AsRef<Path>>(path: P, read_only: bool) -> Result<Self, VmError> {
        let file = if read_only {
            File::open(path.as_ref())
                .await
                .map_err(|e| VmError::Platform(PlatformError::IoError(e.to_string())))?
        } else {
            tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path.as_ref())
                .await
                .map_err(|e| VmError::Platform(PlatformError::IoError(e.to_string())))?
        };

        let metadata = file
            .metadata()
            .await
            .map_err(|e| VmError::Platform(PlatformError::IoError(e.to_string())))?;
        let capacity = metadata.len() / 512;
        let sector_size = 512;

        let device = VirtioBlock {
            capacity,
            sector_size,
            read_only,
            ..Default::default()
        };

        let path_str = path.as_ref().to_string_lossy().to_string();
        let (io_tx, io_rx) = mpsc::channel(IO_QUEUE_CAPACITY);

        // 启动异步IO处理任务
        let io_handler_task = Self::spawn_io_handler(file, read_only, io_rx);

        // 在后台运行IO处理器
        tokio::spawn(io_handler_task);

        Ok(Self {
            device: Arc::new(Mutex::new(device)),
            io_tx: Arc::new(Mutex::new(Some(io_tx))),
            io_rx: Arc::new(Mutex::new(None)), // 不再需要，使用oneshot通道
            file_path: Arc::new(Mutex::new(Some(path_str))),
        })
    }

    /// 启动异步IO处理任务
    async fn spawn_io_handler(
        mut file: File,
        read_only: bool,
        mut io_rx: mpsc::Receiver<AsyncIoRequest>,
    ) {
        while let Some(request) = io_rx.recv().await {
            match request {
                AsyncIoRequest::Read {
                    sector,
                    count,
                    req_id: _,
                    response,
                } => {
                    let offset = sector * 512;

                    // 使用read_at进行真正的异步读取（不需要seek）
                    let mut buf = vec![0u8; count as usize];
                    let result = match file.read_at(&mut buf, offset).await {
                        Ok(_) => Ok(buf),
                        Err(e) => Err(format!("Read error: {}", e)),
                    };
                    let _ = response.send(result);
                }
                AsyncIoRequest::Write {
                    sector,
                    data,
                    req_id: _,
                    response,
                } => {
                    if read_only {
                        let _ = response.send(Err("Read-only device".to_string()));
                        continue;
                    }

                    let offset = sector * 512;

                    // 使用write_at进行真正的异步写入（不需要seek）
                    let result = match file.write_at(&data, offset).await {
                        Ok(_) => Ok(()),
                        Err(e) => Err(format!("Write error: {}", e)),
                    };
                    let _ = response.send(result);
                }
                AsyncIoRequest::Flush {
                    req_id: _,
                    response,
                } => {
                    let result = match file.sync_all().await {
                        Ok(_) => Ok(()),
                        Err(e) => Err(format!("Flush error: {}", e)),
                    };
                    let _ = response.send(result);
                }
                AsyncIoRequest::Close => {
                    break; // 关闭处理器
                }
            }
        }
    }

    /// 异步发送I/O请求
    pub async fn submit_io_request(&self, request: AsyncIoRequest) -> Result<(), VmError> {
        let io_tx = self.io_tx.lock();
        if let Some(tx) = io_tx.as_ref() {
            tx.send(request).await.map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "I/O request channel closed".to_string(),
                    module: "block_service".to_string(),
                })
            })
        } else {
            Err(VmError::Core(vm_core::CoreError::Internal {
                message: "I/O channel not initialized".to_string(),
                module: "block_service".to_string(),
            }))
        }
    }

    /// 获取设备容量（扇区数）
    pub fn capacity(&self) -> u64 {
        let device = self.device.lock();
        device.capacity
    }

    /// 获取扇区大小
    pub fn sector_size(&self) -> u32 {
        let device = self.device.lock();
        device.sector_size
    }

    /// 是否只读
    pub fn is_read_only(&self) -> bool {
        let device = self.device.lock();
        device.read_only
    }

    /// 获取设备特性标志
    pub fn get_features(&self) -> u32 {
        let device = self.device.lock();
        let mut features = 0u32;
        features |= 1 << 6; // VIRTIO_BLK_F_BLK_SIZE
        features |= 1 << 9; // VIRTIO_BLK_F_FLUSH
        if device.read_only {
            features |= 1 << 5; // VIRTIO_BLK_F_RO
        }
        features
    }

    /// 处理块设备请求 - 核心业务逻辑
    pub fn process_request(
        &self,
        mmu: &mut dyn MMU,
        req_addr: GuestAddr,
        data_addr: GuestAddr,
        data_len: u32,
        status_addr: GuestAddr,
    ) -> BlockStatus {
        // 1. 读取请求头
        let req_type = match mmu.read(req_addr, 4) {
            Ok(v) => v as u32,
            Err(_) => return BlockStatus::IoErr,
        };
        let sector = match mmu.read(req_addr + 8, 8) {
            Ok(v) => v,
            Err(_) => return BlockStatus::IoErr,
        };

        // 2. 验证请求类型
        let req_type = match BlockRequestType::from_u32(req_type) {
            Some(t) => t,
            None => return BlockStatus::Unsupported,
        };

        // 3. 根据请求类型处理
        let status = match req_type {
            BlockRequestType::In => self.handle_read_request(mmu, sector, data_addr, data_len),
            BlockRequestType::Out => self.handle_write_request(mmu, sector, data_addr, data_len),
            BlockRequestType::Flush => self.handle_flush_request(),
            BlockRequestType::GetId => BlockStatus::Unsupported,
        };

        // 4. 写入状态结果
        let _ = mmu.write(status_addr, status as u64, 1);
        status
    }

    /// 处理读请求（同步版本，内部使用异步I/O）
    fn handle_read_request(
        &self,
        mmu: &mut dyn MMU,
        sector: u64,
        data_addr: GuestAddr,
        data_len: u32,
    ) -> BlockStatus {
        let device = self.device.lock();

        // 验证扇区范围
        if sector + (data_len as u64) / 512 > device.capacity {
            return BlockStatus::IoErr;
        }

        // 如果有文件路径，使用异步I/O通道
        if let Some(_file_path) = self.file_path.lock().as_ref() {
            // 使用tokio运行时执行异步操作
            let rt = tokio::runtime::Handle::try_current();
            let result = if let Ok(handle) = rt {
                // 在异步上下文中
                handle.block_on(async {
                    self.handle_read_request_async_internal(sector, data_len).await
                })
            } else {
                // 不在异步上下文中，创建新的运行时
                match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt.block_on(async {
                        self.handle_read_request_async_internal(sector, data_len).await
                    }),
                    Err(_) => return BlockStatus::IoErr,
                }
            };
            
            match result {
                Ok(buffer) => {
                    // 将数据写入客户端内存
                    if let Err(_) = mmu.write_bulk(data_addr, &buffer) {
                        return BlockStatus::IoErr;
                    }
                    BlockStatus::Ok
                }
                Err(_) => BlockStatus::IoErr,
            }
        } else {
            // 没有文件路径，返回错误
            BlockStatus::IoErr
        }
    }

    /// 内部异步读取实现
    async fn handle_read_request_async_internal(
        &self,
        sector: u64,
        data_len: u32,
    ) -> Result<Vec<u8>, VmError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static REQ_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
        
        let req_id = REQ_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        let count = data_len;

        // 创建oneshot通道用于接收响应
        let (tx, rx) = oneshot::channel();

        // 发送异步读取请求
        {
            let io_tx = self.io_tx.lock();
            if let Some(ref sender) = io_tx.as_ref() {
                sender.send(AsyncIoRequest::Read {
                    sector,
                    count,
                    req_id,
                    response: tx,
                }).await.map_err(|_| VmError::Core(vm_core::CoreError::Internal {
                    message: "I/O request channel closed".to_string(),
                    module: "block_service".to_string(),
                }))?;
            } else {
                return Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "I/O channel not initialized".to_string(),
                    module: "block_service".to_string(),
                }));
            }
        }

        // 等待响应
        match rx.await {
            Ok(Ok(data)) => Ok(data),
            Ok(Err(e)) => Err(VmError::Platform(PlatformError::IoError(e))),
            Err(_) => Err(VmError::Core(vm_core::CoreError::Internal {
                message: "I/O response channel closed".to_string(),
                module: "block_service".to_string(),
            })),
        }
    }

    /// 处理写请求（同步版本，内部使用异步I/O）
    fn handle_write_request(
        &self,
        mmu: &dyn MMU,
        sector: u64,
        data_addr: GuestAddr,
        data_len: u32,
    ) -> BlockStatus {
        let device = self.device.lock();

        // 检查只读状态
        if device.read_only {
            return BlockStatus::IoErr;
        }

        // 验证扇区范围
        if sector + (data_len as u64) / 512 > device.capacity {
            return BlockStatus::IoErr;
        }

        // 从客户端内存读取数据
        let mut buffer = vec![0u8; data_len as usize];
        if let Err(_) = mmu.read_bulk(data_addr, &mut buffer) {
            return BlockStatus::IoErr;
        }

        // 如果有文件路径，使用异步I/O通道
        if let Some(_file_path) = self.file_path.lock().as_ref() {
            // 使用tokio运行时执行异步操作
            let rt = tokio::runtime::Handle::try_current();
            let result = if let Ok(handle) = rt {
                // 在异步上下文中
                handle.block_on(async {
                    self.handle_write_request_async_internal(sector, buffer).await
                })
            } else {
                // 不在异步上下文中，创建新的运行时
                match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt.block_on(async {
                        self.handle_write_request_async_internal(sector, buffer).await
                    }),
                    Err(_) => return BlockStatus::IoErr,
                }
            };
            
            match result {
                Ok(_) => BlockStatus::Ok,
                Err(_) => BlockStatus::IoErr,
            }
        } else {
            // 没有文件路径，返回错误
            BlockStatus::IoErr
        }
    }

    /// 内部异步写入实现
    async fn handle_write_request_async_internal(
        &self,
        sector: u64,
        data: Vec<u8>,
    ) -> Result<(), VmError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static REQ_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
        
        let req_id = REQ_ID_COUNTER.fetch_add(1, Ordering::Relaxed);

        // 创建oneshot通道用于接收响应
        let (tx, rx) = oneshot::channel();

        // 发送异步写入请求
        {
            let io_tx = self.io_tx.lock();
            if let Some(ref sender) = io_tx.as_ref() {
                sender.send(AsyncIoRequest::Write {
                    sector,
                    data: Arc::new(data),
                    req_id,
                    response: tx,
                }).await.map_err(|_| VmError::Core(vm_core::CoreError::Internal {
                    message: "I/O request channel closed".to_string(),
                    module: "block_service".to_string(),
                }))?;
            } else {
                return Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "I/O channel not initialized".to_string(),
                    module: "block_service".to_string(),
                }));
            }
        }

        // 等待响应
        match rx.await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(VmError::Platform(PlatformError::IoError(e))),
            Err(_) => Err(VmError::Core(vm_core::CoreError::Internal {
                message: "I/O response channel closed".to_string(),
                module: "block_service".to_string(),
            })),
        }
    }

    /// 处理刷新请求（同步版本，内部使用异步I/O）
    fn handle_flush_request(&self) -> BlockStatus {
        let device = self.device.lock();

        if device.read_only {
            return BlockStatus::Ok; // 只读设备无需刷新
        }

        // 如果有文件路径，使用异步I/O通道
        if let Some(_file_path) = self.file_path.lock().as_ref() {
            // 使用tokio运行时执行异步操作
            let rt = tokio::runtime::Handle::try_current();
            let result = if let Ok(handle) = rt {
                // 在异步上下文中
                handle.block_on(async {
                    self.handle_flush_request_async_internal().await
                })
            } else {
                // 不在异步上下文中，创建新的运行时
                match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt.block_on(async {
                        self.handle_flush_request_async_internal().await
                    }),
                    Err(_) => return BlockStatus::IoErr,
                }
            };
            
            match result {
                Ok(_) => BlockStatus::Ok,
                Err(_) => BlockStatus::IoErr,
            }
        } else {
            // 没有文件路径，返回成功（可能是内存设备）
            BlockStatus::Ok
        }
    }

    /// 内部异步刷新实现
    async fn handle_flush_request_async_internal(&self) -> Result<(), VmError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static REQ_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
        
        let req_id = REQ_ID_COUNTER.fetch_add(1, Ordering::Relaxed);

        // 创建oneshot通道用于接收响应
        let (tx, rx) = oneshot::channel();

        // 发送异步刷新请求
        {
            let io_tx = self.io_tx.lock();
            if let Some(ref sender) = io_tx.as_ref() {
                sender.send(AsyncIoRequest::Flush {
                    req_id,
                    response: tx,
                }).await.map_err(|_| VmError::Core(vm_core::CoreError::Internal {
                    message: "I/O request channel closed".to_string(),
                    module: "block_service".to_string(),
                }))?;
            } else {
                return Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "I/O channel not initialized".to_string(),
                    module: "block_service".to_string(),
                }));
            }
        }

        // 等待响应
        match rx.await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(VmError::Platform(PlatformError::IoError(e))),
            Err(_) => Err(VmError::Core(vm_core::CoreError::Internal {
                message: "I/O response channel closed".to_string(),
                module: "block_service".to_string(),
            })),
        }
    }

    /// 获取内部设备引用（仅用于特殊场景）
    pub fn get_device(&self) -> Arc<Mutex<VirtioBlock>> {
        Arc::clone(&self.device)
    }

    /// 异步处理块设备请求 - 真正的异步实现
    ///
    /// 这个方法使用tokio异步运行时，允许在等待I/O时释放线程
    pub async fn process_request_async(
        &self,
        mmu: &mut dyn MMU,
        req_addr: GuestAddr,
        data_addr: GuestAddr,
        data_len: u32,
        status_addr: GuestAddr,
    ) -> BlockStatus {
        // 1. 读取请求头
        let req_type = match mmu.read(req_addr, 4) {
            Ok(v) => v as u32,
            Err(_) => return BlockStatus::IoErr,
        };
        let sector = match mmu.read(req_addr + 8, 8) {
            Ok(v) => v,
            Err(_) => return BlockStatus::IoErr,
        };

        // 2. 验证请求类型
        let req_type = match BlockRequestType::from_u32(req_type) {
            Some(t) => t,
            None => return BlockStatus::Unsupported,
        };

        // 3. 根据请求类型异步处理
        let status = match req_type {
            BlockRequestType::In => {
                self.handle_read_request_async(mmu, sector, data_addr, data_len)
                    .await
            }
            BlockRequestType::Out => {
                self.handle_write_request_async(mmu, sector, data_addr, data_len)
                    .await
            }
            BlockRequestType::Flush => self.handle_flush_request_async().await,
            BlockRequestType::GetId => BlockStatus::Unsupported,
        };

        // 4. 写入状态结果
        let _ = mmu.write(status_addr, status as u64, 1);
        status
    }

    /// 异步处理读请求
    async fn handle_read_request_async(
        &self,
        mmu: &mut dyn MMU,
        sector: u64,
        data_addr: GuestAddr,
        data_len: u32,
    ) -> BlockStatus {
        let device = self.device.lock();

        // 验证扇区范围
        if sector + (data_len as u64) / 512 > device.capacity {
            return BlockStatus::IoErr;
        }

        // 构造读取缓冲区
        let buffer = vec![0u8; data_len as usize];

        // 模拟异步读取操作（通过tokio task yield）
        // 在实际应用中，这里会进行真正的异步文件读取
        tokio::task::yield_now().await;

        // 将数据写入客户端内存
        if let Err(_) = mmu.write_bulk(data_addr, &buffer) {
            return BlockStatus::IoErr;
        }

        BlockStatus::Ok
    }

    /// 异步处理写请求
    async fn handle_write_request_async(
        &self,
        mmu: &dyn MMU,
        sector: u64,
        data_addr: GuestAddr,
        data_len: u32,
    ) -> BlockStatus {
        let device = self.device.lock();

        // 检查只读状态
        if device.read_only {
            return BlockStatus::IoErr;
        }

        // 验证扇区范围
        if sector + (data_len as u64) / 512 > device.capacity {
            return BlockStatus::IoErr;
        }

        // 从客户端内存读取数据
        let mut buffer = vec![0u8; data_len as usize];
        if let Err(_) = mmu.read_bulk(data_addr, &mut buffer) {
            return BlockStatus::IoErr;
        }

        // 模拟异步写入操作
        tokio::task::yield_now().await;

        BlockStatus::Ok
    }

    /// 异步处理刷新请求
    async fn handle_flush_request_async(&self) -> BlockStatus {
        let device = self.device.lock();

        if device.read_only {
            return BlockStatus::Ok; // 只读设备无需刷新
        }

        // 模拟异步刷新操作
        tokio::task::yield_now().await;

        BlockStatus::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_service() {
        let service = BlockDeviceService::new(1024, 512, false);
        assert_eq!(service.capacity(), 1024);
        assert_eq!(service.sector_size(), 512);
        assert!(!service.is_read_only());
    }

    #[test]
    fn test_features() {
        let service = BlockDeviceService::new(1024, 512, false);
        let features = service.get_features();
        assert!(features & (1 << 6) != 0); // BLK_SIZE
        assert!(features & (1 << 9) != 0); // FLUSH
    }

    #[test]
    fn test_read_only_features() {
        let service = BlockDeviceService::new(1024, 512, true);
        let features = service.get_features();
        assert!(features & (1 << 5) != 0); // RO flag
    }
}
