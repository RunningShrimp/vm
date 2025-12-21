//! JIT 实例池
//!
//! 提供 Jit 实例的复用机制，减少创建和销毁开销

use crate::Jit;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Jit 实例池配置
#[derive(Debug, Clone)]
pub struct JitPoolConfig {
    /// 最小池大小
    pub min_size: usize,
    /// 最大池大小
    pub max_size: usize,
    /// 实例空闲超时时间（秒）
    pub idle_timeout_secs: u64,
    /// 清理间隔（秒）
    pub cleanup_interval_secs: u64,
}

impl Default for JitPoolConfig {
    fn default() -> Self {
        Self {
            min_size: 2,
            max_size: num_cpus::get(),
            idle_timeout_secs: 300, // 5分钟
            cleanup_interval_secs: 60, // 1分钟
        }
    }
}

/// Jit 池条目
struct PooledJit {
    /// Jit 实例
    jit: Jit,
    /// 最后使用时间
    last_used: Instant,
    /// 使用次数
    use_count: u64,
}

/// JIT 实例池
pub struct JitPool {
    /// 可用实例队列
    available: Arc<Mutex<Vec<PooledJit>>>,
    /// 配置
    config: JitPoolConfig,
    /// 当前池大小
    current_size: Arc<Mutex<usize>>,
    /// 上次清理时间
    last_cleanup: Arc<Mutex<Instant>>,
}

impl JitPool {
    /// 创建新的 Jit 池
    pub fn new(config: JitPoolConfig) -> Self {
        let pool = Self {
            available: Arc::new(Mutex::new(Vec::new())),
            config: config.clone(),
            current_size: Arc::new(Mutex::new(0)),
            last_cleanup: Arc::new(Mutex::new(Instant::now())),
        };

        // 预创建最小数量的实例
        {
            let mut available = pool.available.lock();
            for _ in 0..config.min_size {
                available.push(PooledJit {
                    jit: Jit::new(),
                    last_used: Instant::now(),
                    use_count: 0,
                });
            }
        }
        *pool.current_size.lock() = config.min_size;

        pool
    }

    /// 从池中获取 Jit 实例
    pub fn acquire(&self) -> PooledJitGuard {
        // 定期清理空闲实例
        self.cleanup_idle_instances();

        // 先获取需要的值，然后释放锁
        let (jit_instance, should_return_to_pool) = {
            let mut available = self.available.lock();

            // 尝试从池中获取实例
            if let Some(mut pooled) = available.pop() {
                pooled.last_used = Instant::now();
                pooled.use_count += 1;
                (Some(pooled.jit), true)
            } else {
                // 池为空，创建新实例（如果未超过最大大小）
                let current = *self.current_size.lock();
                if current < self.config.max_size {
                    *self.current_size.lock() = current + 1;
                    (Some(Jit::new()), true)
                } else {
                    // 超过最大大小，创建临时实例（不返回池中）
                    (Some(Jit::new()), false)
                }
            }
        };

        // 创建守卫（在锁释放后）
        PooledJitGuard {
            jit: jit_instance,
            pool: self.available.clone(),
            config: self.config.clone(),
            current_size: self.current_size.clone(),
            return_to_pool: should_return_to_pool,
        }
    }

    /// 清理空闲实例
    fn cleanup_idle_instances(&self) {
        let mut last_cleanup = self.last_cleanup.lock();
        let now = Instant::now();

        // 检查是否需要清理
        if now.duration_since(*last_cleanup).as_secs() < self.config.cleanup_interval_secs {
            return;
        }

        *last_cleanup = now;

        let mut available = self.available.lock();
        let timeout = Duration::from_secs(self.config.idle_timeout_secs);
        let mut to_remove = Vec::new();

        // 找出需要移除的空闲实例
        for (idx, pooled) in available.iter().enumerate() {
            if now.duration_since(pooled.last_used) > timeout {
                to_remove.push(idx);
            }
        }

        // 移除空闲实例（从后往前移除，避免索引变化）
        for &idx in to_remove.iter().rev() {
            available.remove(idx);
        }

        // 确保至少保留最小数量的实例
        let current = available.len();
        let min_size = self.config.min_size;
        if current < min_size {
            for _ in current..min_size {
                available.push(PooledJit {
                    jit: Jit::new(),
                    last_used: Instant::now(),
                    use_count: 0,
                });
            }
        }

        *self.current_size.lock() = available.len();
    }

    /// 获取池统计信息
    pub fn stats(&self) -> JitPoolStats {
        let available = self.available.lock();
        let current_size = *self.current_size.lock();

        let total_uses: u64 = available.iter().map(|p| p.use_count).sum();
        let avg_age_secs: f64 = available
            .iter()
            .map(|p| {
                Instant::now()
                    .duration_since(p.last_used)
                    .as_secs_f64()
            })
            .sum::<f64>()
            / available.len().max(1) as f64;

        JitPoolStats {
            available_count: available.len(),
            current_size,
            total_uses,
            avg_age_secs,
        }
    }
}

/// Jit 池统计信息
#[derive(Debug, Clone)]
pub struct JitPoolStats {
    /// 可用实例数量
    pub available_count: usize,
    /// 当前池大小
    pub current_size: usize,
    /// 总使用次数
    pub total_uses: u64,
    /// 平均空闲时间（秒）
    pub avg_age_secs: f64,
}

/// Jit 实例守卫（自动返回池中）
pub struct PooledJitGuard {
    jit: Option<Jit>,
    pool: Arc<Mutex<Vec<PooledJit>>>,
    config: JitPoolConfig,
    current_size: Arc<Mutex<usize>>,
    return_to_pool: bool,
}

impl Default for PooledJitGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl PooledJitGuard {
    /// 创建不返回池的守卫（临时实例）
    fn new() -> Self {
        Self {
            jit: Some(Jit::new()),
            pool: Arc::new(Mutex::new(Vec::new())),
            config: JitPoolConfig::default(),
            current_size: Arc::new(Mutex::new(0)),
            return_to_pool: false,
        }
    }
}

impl std::ops::Deref for PooledJitGuard {
    type Target = Jit;

    fn deref(&self) -> &Self::Target {
        self.jit.as_ref().expect("Jit instance was already returned")
    }
}

impl std::ops::DerefMut for PooledJitGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.jit.as_mut().expect("Jit instance was already returned")
    }
}

impl Drop for PooledJitGuard {
    fn drop(&mut self) {
        if self.return_to_pool {
            if let Some(jit) = self.jit.take() {
                let mut pool = self.pool.lock();
                // 检查池大小，如果超过最大大小则不返回
                if pool.len() < self.config.max_size {
                    pool.push(PooledJit {
                        jit,
                        last_used: Instant::now(),
                        use_count: 0,
                    });
                }
            }
        }
    }
}

impl Default for JitPool {
    fn default() -> Self {
        Self::new(JitPoolConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jit_pool_acquire_release() {
        let pool = JitPool::new(JitPoolConfig {
            min_size: 2,
            max_size: 4,
            idle_timeout_secs: 60,
            cleanup_interval_secs: 10,
        });

        // 获取实例
        let guard1 = pool.acquire();
        let guard2 = pool.acquire();

        // 检查池大小
        let stats = pool.stats();
        assert_eq!(stats.available_count, 0); // 两个实例都被取出

        // 释放实例（通过 drop）
        drop(guard1);
        drop(guard2);

        // 检查实例是否返回池中
        let stats = pool.stats();
        assert_eq!(stats.available_count, 2);
    }

    #[test]
    fn test_jit_pool_stats() {
        let pool = JitPool::new(JitPoolConfig {
            min_size: 2,
            max_size: 4,
            idle_timeout_secs: 60,
            cleanup_interval_secs: 10,
        });

        let stats = pool.stats();
        assert_eq!(stats.available_count, 2);
        assert_eq!(stats.current_size, 2);
    }
}

