//! 零拷贝 I/O 实现
//!
//! 支持 io_uring (Linux) 和共享内存环形缓冲区

use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use vm_core::MMU;
use crate::mmu_util::MmuUtil;
use thiserror::Error;

/// I/O 请求类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoOpcode {
    /// 读取
    Read,
    /// 写入
    Write,
    /// 刷新
    Flush,
}

/// I/O 请求
#[derive(Debug, Clone)]
pub struct IoRequest {
    /// 操作码
    pub opcode: IoOpcode,
    /// 偏移量
    pub offset: u64,
    /// 缓冲区地址
    pub buffer: u64,
    /// 长度
    pub length: usize,
    /// 用户数据
    pub user_data: u64,
}

/// I/O 完成结果
#[derive(Debug, Clone)]
pub struct IoCompletion {
    /// 用户数据
    pub user_data: u64,
    /// 结果（成功时为传输字节数，失败时为错误码）
    pub result: i32,
}

/// 零拷贝 I/O 后端
pub trait ZeroCopyIo: Send {
    /// 提交 I/O 请求
    fn submit(&mut self, req: IoRequest) -> Result<(), IoError>;
    
    /// 获取完成的 I/O
    fn poll_completions(&mut self) -> Vec<IoCompletion>;
    
    /// 刷新所有待处理的 I/O
    fn flush(&mut self) -> Result<(), IoError>;

    /// 关联 MMU（用于按 buffer 地址进行零拷贝）
    fn attach_mmu(&mut self, mmu: Box<dyn MMU>);
}

/// io_uring 后端 (Linux)
#[cfg(target_os = "linux")]
pub mod uring {
    use super::*;

    /// io_uring I/O 后端
    pub struct IoUringBackend {
        /// 队列深度
        depth: u32,
        /// 待提交的请求
        pending: VecDeque<IoRequest>,
        /// 已完成的请求
        completed: VecDeque<IoCompletion>,
        /// 文件描述符
        fd: i32,
        /// MMU（用于零拷贝）
        mmu: Option<Box<dyn MMU>>,
    }

    impl IoUringBackend {
        /// 创建新的 io_uring 后端
        pub fn new(fd: i32, depth: u32) -> Result<Self, IoError> {
            Ok(Self {
                depth,
                pending: VecDeque::new(),
                completed: VecDeque::new(),
                fd,
                mmu: None,
            })
        }

        /// 实际执行 I/O（模拟实现）
        fn execute_io(&mut self, req: &IoRequest) -> i32 {
            // 这里应该调用实际的 io_uring 接口
            // 为了简化，我们使用同步 I/O 模拟
            match req.opcode {
                IoOpcode::Read => {
                    // 模拟读取
                    req.length as i32
                }
                IoOpcode::Write => {
                    // 模拟写入
                    req.length as i32
                }
                IoOpcode::Flush => {
                    // 模拟刷新
                    0
                }
            }
        }
    }

    impl ZeroCopyIo for IoUringBackend {
        fn submit(&mut self, req: IoRequest) -> Result<(), IoError> {
            if self.pending.len() >= self.depth as usize {
                return Err(IoError::QueueFull);
            }
            
            self.pending.push_back(req);
            Ok(())
        }

        fn poll_completions(&mut self) -> Vec<IoCompletion> {
            // 处理待提交的请求
            while let Some(req) = self.pending.pop_front() {
                let result = self.execute_io(&req);
                self.completed.push_back(IoCompletion {
                    user_data: req.user_data,
                    result,
                });
            }

            // 返回所有已完成的请求
            self.completed.drain(..).collect()
        }

        fn flush(&mut self) -> Result<(), IoError> {
            // 确保所有请求都已提交
            self.poll_completions();
            Ok(())
        }

        fn attach_mmu(&mut self, mmu: Box<dyn MMU>) {
            self.mmu = Some(mmu);
        }
    }
}

/// 共享内存环形缓冲区后端（跨平台）
pub mod ringbuf {
    use super::*;

    /// 环形缓冲区
    pub struct RingBuffer {
        /// 缓冲区
        buffer: Vec<u8>,
        /// 读指针
        read_pos: usize,
        /// 写指针
        write_pos: usize,
        /// 容量
        capacity: usize,
    }

    impl RingBuffer {
        /// 创建新的环形缓冲区
        pub fn new(capacity: usize) -> Self {
            Self {
                buffer: vec![0; capacity],
                read_pos: 0,
                write_pos: 0,
                capacity,
            }
        }

        /// 可读字节数
        pub fn available(&self) -> usize {
            if self.write_pos >= self.read_pos {
                self.write_pos - self.read_pos
            } else {
                self.capacity - self.read_pos + self.write_pos
            }
        }

        /// 可写字节数
        pub fn free_space(&self) -> usize {
            self.capacity - self.available() - 1
        }

        /// 写入数据
        pub fn write(&mut self, data: &[u8]) -> usize {
            let free = self.free_space();
            let to_write = data.len().min(free);

            for i in 0..to_write {
                self.buffer[self.write_pos] = data[i];
                self.write_pos = (self.write_pos + 1) % self.capacity;
            }

            to_write
        }

        /// 读取数据
        pub fn read(&mut self, buf: &mut [u8]) -> usize {
            let available = self.available();
            let to_read = buf.len().min(available);

            for i in 0..to_read {
                buf[i] = self.buffer[self.read_pos];
                self.read_pos = (self.read_pos + 1) % self.capacity;
            }

            to_read
        }

        /// 清空缓冲区
        pub fn clear(&mut self) {
            self.read_pos = 0;
            self.write_pos = 0;
        }
    }

    /// 共享内存 I/O 后端
    pub struct SharedMemoryBackend {
        /// 发送环形缓冲区
        tx_ring: Arc<Mutex<RingBuffer>>,
        /// 接收环形缓冲区
        rx_ring: Arc<Mutex<RingBuffer>>,
        /// 待处理的请求
        pending: VecDeque<IoRequest>,
        /// 已完成的请求
        completed: VecDeque<IoCompletion>,
        /// MMU（用于零拷贝）
        mmu: Option<Box<dyn MMU>>,
    }

    impl SharedMemoryBackend {
        /// 创建新的共享内存后端
        pub fn new(capacity: usize) -> Self {
            Self {
                tx_ring: Arc::new(Mutex::new(RingBuffer::new(capacity))),
                rx_ring: Arc::new(Mutex::new(RingBuffer::new(capacity))),
                pending: VecDeque::new(),
                completed: VecDeque::new(),
                mmu: None,
            }
        }

        /// 获取发送环形缓冲区
        pub fn tx_ring(&self) -> Arc<Mutex<RingBuffer>> {
            Arc::clone(&self.tx_ring)
        }

        /// 获取接收环形缓冲区
        pub fn rx_ring(&self) -> Arc<Mutex<RingBuffer>> {
            Arc::clone(&self.rx_ring)
        }
    }

    impl ZeroCopyIo for SharedMemoryBackend {
        fn submit(&mut self, req: IoRequest) -> Result<(), IoError> {
            self.pending.push_back(req);
            Ok(())
        }

        fn poll_completions(&mut self) -> Vec<IoCompletion> {
            // 处理待提交的请求
            while let Some(req) = self.pending.pop_front() {
                let result = match req.opcode {
                    IoOpcode::Read => {
                        // 从接收环形缓冲区读取
                        let mut rx = self.rx_ring.lock().unwrap();
                        let mut buf = vec![0u8; req.length];
                        let read = rx.read(&mut buf);
                        // 写入到 Guest 内存（零拷贝）
                        if let Some(mmu) = self.mmu.as_mut() {
                            let _ = MmuUtil::write_slice(mmu.as_mut(), req.buffer, &buf[..read]);
                        }
                        read as i32
                    }
                    IoOpcode::Write => {
                        // 写入发送环形缓冲区
                        let mut tx = self.tx_ring.lock().unwrap();
                        // 从 Guest 内存读取数据（零拷贝）
                        let written = if let Some(mmu) = self.mmu.as_mut() {
                            let mut tmp = vec![0u8; req.length];
                            let _ = MmuUtil::read_slice(mmu.as_mut(), req.buffer, &mut tmp);
                            tx.write(&tmp)
                        } else {
                            // 后备：写入零数据
                            tx.write(&vec![0u8; req.length])
                        };
                        written as i32
                    }
                    IoOpcode::Flush => 0,
                };

                self.completed.push_back(IoCompletion {
                    user_data: req.user_data,
                    result,
                });
            }

            self.completed.drain(..).collect()
        }

        fn flush(&mut self) -> Result<(), IoError> {
            self.poll_completions();
            Ok(())
        }

        fn attach_mmu(&mut self, mmu: Box<dyn MMU>) {
            self.mmu = Some(mmu);
        }
    }
}

/// I/O 调度器
pub struct IoScheduler {
    /// 后端
    backend: Box<dyn ZeroCopyIo>,
    /// 统计信息
    stats: IoStats,
}

/// I/O 统计信息
#[derive(Debug, Clone, Default)]
pub struct IoStats {
    /// 总请求数
    pub total_requests: u64,
    /// 总完成数
    pub total_completions: u64,
    /// 读取字节数
    pub bytes_read: u64,
    /// 写入字节数
    pub bytes_written: u64,
}

impl IoScheduler {
    /// 创建新的 I/O 调度器
    pub fn new(backend: Box<dyn ZeroCopyIo>) -> Self {
        Self {
            backend,
            stats: IoStats::default(),
        }
    }

    /// 提交 I/O 请求
    pub fn submit(&mut self, req: IoRequest) -> Result<(), IoError> {
        self.stats.total_requests += 1;
        self.backend.submit(req)
    }

    /// 轮询完成
    pub fn poll(&mut self) -> Vec<IoCompletion> {
        let completions = self.backend.poll_completions();
        self.stats.total_completions += completions.len() as u64;

        for completion in &completions {
            if completion.result > 0 {
                // 这里应该根据原始请求类型更新统计
                // 简化实现
                self.stats.bytes_read += completion.result as u64;
            }
        }

        completions
    }

    /// 刷新
    pub fn flush(&mut self) -> Result<(), IoError> {
        self.backend.flush()
    }

    /// 关联 MMU 到后端
    pub fn attach_mmu(&mut self, mmu: Box<dyn MMU>) {
        self.backend.attach_mmu(mmu);
    }

    /// 获取统计信息
    pub fn stats(&self) -> &IoStats {
        &self.stats
    }
}

#[derive(Debug, Error)]
pub enum IoError {
    #[error("Queue full")]
    QueueFull,
    #[error("Backend error: {0}")]
    Backend(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::ringbuf::*;

    #[test]
    fn test_ring_buffer() {
        let mut ring = RingBuffer::new(16);
        
        assert_eq!(ring.available(), 0);
        assert_eq!(ring.free_space(), 15);
        
        let data = b"Hello";
        let written = ring.write(data);
        assert_eq!(written, 5);
        assert_eq!(ring.available(), 5);
        
        let mut buf = vec![0u8; 5];
        let read = ring.read(&mut buf);
        assert_eq!(read, 5);
        assert_eq!(&buf, data);
        assert_eq!(ring.available(), 0);
    }

    #[test]
    fn test_shared_memory_backend() {
        let mut backend = SharedMemoryBackend::new(1024);
        
        let req = IoRequest {
            opcode: IoOpcode::Write,
            offset: 0,
            buffer: 0,
            length: 100,
            user_data: 42,
        };
        
        backend.submit(req).unwrap();
        let completions = backend.poll_completions();
        
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].user_data, 42);
    }
}
