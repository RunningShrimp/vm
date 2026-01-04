use crate::{CodePtr, Jit};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use vm_ir::IRBlock;

/// 异步JIT代码池
///
/// 使用tokio异步运行时管理JIT编译任务
/// 支持异步代码查找和插入，使用并发安全的缓存结构
pub struct JitPool {
    workers: Vec<tokio::task::JoinHandle<()>>,
    tx: mpsc::UnboundedSender<IRBlock>,
    cache: Arc<RwLock<HashMap<u64, CodePtr>>>,
}

impl JitPool {
    /// 创建新的异步JIT代码池
    ///
    /// `worker_count`: 工作线程数量（当前实现使用单个worker处理所有任务）
    pub fn new(_worker_count: usize) -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<IRBlock>();
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let mut workers = Vec::new();

        // 使用单个worker处理所有编译任务
        // 因为UnboundedReceiver不能clone
        let cache_clone = cache.clone();
        let handle = tokio::spawn(async move {
            let mut jit = Jit::new();
            let mut rx = rx;
            loop {
                match rx.recv().await {
                    Some(block) => {
                        let code_ptr = jit.compile(&block);
                        // 存储到异步缓存
                        cache_clone.write().await.insert(block.start_pc.0, code_ptr);
                    }
                    None => break,
                }
            }
        });
        workers.push(handle);

        Self { workers, tx, cache }
    }

    /// 提交代码块到编译队列（同步接口）
    pub fn submit(&self, blocks: Vec<IRBlock>) {
        for b in blocks {
            let _ = self.tx.send(b);
        }
    }

    /// 异步提交代码块到编译队列
    pub async fn submit_async(&self, blocks: Vec<IRBlock>) {
        for b in blocks {
            let _ = self.tx.send(b);
        }
    }

    /// 获取缓存（同步接口，返回Arc<RwLock>）
    pub fn cache(&self) -> Arc<RwLock<HashMap<u64, CodePtr>>> {
        self.cache.clone()
    }

    /// 异步查找代码指针
    ///
    /// 返回Some(CodePtr)如果找到，None如果未找到
    pub async fn get_async(&self, pc: u64) -> Option<CodePtr> {
        self.cache.read().await.get(&pc).copied()
    }

    /// 异步插入代码指针
    ///
    /// 如果已存在，返回旧的CodePtr；否则返回None
    pub async fn insert_async(&self, pc: u64, code_ptr: CodePtr) -> Option<CodePtr> {
        self.cache.write().await.insert(pc, code_ptr)
    }

    /// 同步查找代码指针（用于兼容性）
    pub fn get(&self, pc: u64) -> Option<CodePtr> {
        // 使用block_on在异步上下文中调用
        // 注意：这需要运行时，可能不是最优的
        // 建议在异步上下文中使用get_async
        tokio::runtime::Handle::try_current()
            .map(|handle| handle.block_on(self.get_async(pc)))
            .unwrap_or_else(|_| {
                // 如果没有运行时，使用同步方式（需要运行时）
                // 这是一个fallback，但可能阻塞
                None
            })
    }

    /// 同步插入代码指针（用于兼容性）
    pub fn insert(&self, pc: u64, code_ptr: CodePtr) -> Option<CodePtr> {
        tokio::runtime::Handle::try_current()
            .map(|handle| handle.block_on(self.insert_async(pc, code_ptr)))
            .unwrap_or_else(|_| None)
    }
}

impl Drop for JitPool {
    fn drop(&mut self) {
        // 关闭发送端不需要在Drop中处理
        // tx会自动drop，worker会在接收None时退出
        // workers会在drop时被abort
    }
}

impl JitPool {
    /// 优雅关闭代码池
    ///
    /// 关闭发送端并等待所有worker完成
    pub async fn shutdown(mut self) {
        // 关闭发送端
        drop(std::mem::replace(&mut self.tx, {
            let (tx, _) = mpsc::unbounded_channel();
            tx
        }));

        // 等待所有worker完成
        for h in self.workers.drain(..) {
            let _ = h.await;
        }
    }
}
