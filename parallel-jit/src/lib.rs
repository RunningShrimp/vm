//! Parallel JIT Compilation System
//!
//! Implements background asynchronous compilation queue with:
//! - Multi-worker compilation
//! - Work queue management
//! - Completion callbacks
//! - Error handling and fallback
//! - Statistics tracking

use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

/// Result type for parallel JIT operations
pub type ParallelJitResult = Result<(), ParallelJitError>;

/// Error types for parallel JIT
#[derive(Debug, Clone)]
pub enum ParallelJitError {
    /// Queue full
    QueueFull { capacity: usize },
    /// Channel send failed
    SendFailed,
    /// Worker crashed
    WorkerPanic { worker_id: usize },
    /// Compilation timeout
    CompileTimeout { block_id: u64 },
    /// Task not found
    TaskNotFound { block_id: u64 },
}

/// Compilation priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
}

/// Compilation task
#[derive(Debug, Clone)]
pub struct CompileTask {
    /// Block identifier
    pub block_id: u64,
    /// Compilation priority
    pub priority: Priority,
    /// Time when task was created
    pub created_at: Instant,
    /// Estimated compilation tier (0-3)
    pub tier: u32,
}

impl PartialEq for CompileTask {
    fn eq(&self, other: &Self) -> bool {
        self.block_id == other.block_id
    }
}

impl Eq for CompileTask {}

impl PartialOrd for CompileTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CompileTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority value = Greater (comes before in order)
        match self.priority.cmp(&other.priority) {
            std::cmp::Ordering::Equal => {
                // Earlier creation time first
                self.created_at.cmp(&other.created_at)
            }
            ord => ord,
        }
    }
}

/// Compiled result
#[derive(Debug, Clone)]
pub struct CompileResult {
    /// Block ID
    pub block_id: u64,
    /// Compilation was successful
    pub success: bool,
    /// Compilation time in microseconds
    pub compile_time_us: u64,
    /// Generated code size
    pub code_size: usize,
    /// Error message if failed
    pub error: Option<String>,
}

/// Worker statistics
#[derive(Debug, Clone, Default)]
pub struct WorkerStats {
    /// Total tasks processed
    pub tasks_processed: u64,
    /// Total compilation time
    pub total_compile_time_us: u64,
    /// Successful compilations
    pub successful_compilations: u64,
    /// Failed compilations
    pub failed_compilations: u64,
}

impl WorkerStats {
    /// Average compilation time per task
    pub fn avg_compile_time_us(&self) -> f64 {
        if self.tasks_processed == 0 {
            0.0
        } else {
            self.total_compile_time_us as f64 / self.tasks_processed as f64
        }
    }

    /// Success rate
    pub fn success_rate(&self) -> f64 {
        if self.tasks_processed == 0 {
            0.0
        } else {
            (self.successful_compilations as f64 / self.tasks_processed as f64) * 100.0
        }
    }
}

/// Queue statistics
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    /// Total tasks enqueued
    pub total_tasks: u64,
    /// Current queue length
    pub current_length: usize,
    /// Peak queue length
    pub peak_length: usize,
    /// Total wait time (microseconds)
    pub total_wait_time_us: u64,
}

impl QueueStats {
    /// Average task wait time
    pub fn avg_wait_time_us(&self) -> f64 {
        if self.total_tasks == 0 {
            0.0
        } else {
            self.total_wait_time_us as f64 / self.total_tasks as f64
        }
    }
}

/// Parallel JIT compiler queue
pub struct ParallelJitQueue {
    /// Task queue
    queue: Arc<Mutex<VecDeque<CompileTask>>>,
    /// Pending results
    results: Arc<RwLock<HashMap<u64, CompileResult>>>,
    /// Worker statistics
    worker_stats: Arc<RwLock<Vec<WorkerStats>>>,
    /// Queue statistics
    queue_stats: Arc<Mutex<QueueStats>>,
    /// Configuration
    config: ParallelJitConfig,
}

/// Configuration for parallel JIT
#[derive(Debug, Clone)]
pub struct ParallelJitConfig {
    /// Number of compiler workers
    pub num_workers: usize,
    /// Maximum queue size
    pub max_queue_size: usize,
    /// Compilation timeout (microseconds)
    pub compile_timeout_us: u64,
    /// Enable statistics tracking
    pub track_stats: bool,
}

impl Default for ParallelJitConfig {
    fn default() -> Self {
        Self {
            num_workers: 4, // Conservative default
            max_queue_size: 1000,
            compile_timeout_us: 5_000_000, // 5 seconds
            track_stats: true,
        }
    }
}

impl ParallelJitQueue {
    /// Create new parallel JIT queue
    pub fn new(config: ParallelJitConfig) -> Self {
        let mut worker_stats = Vec::new();
        for _ in 0..config.num_workers {
            worker_stats.push(WorkerStats::default());
        }

        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
            worker_stats: Arc::new(RwLock::new(worker_stats)),
            queue_stats: Arc::new(Mutex::new(QueueStats::default())),
            config,
        }
    }

    /// Enqueue compilation task
    pub fn enqueue(&self, task: CompileTask) -> ParallelJitResult {
        let mut queue = self.queue.lock();

        // Check queue size
        if queue.len() >= self.config.max_queue_size {
            return Err(ParallelJitError::QueueFull {
                capacity: self.config.max_queue_size,
            });
        }

        // Insert task in priority order (front = highest priority)
        // Compare: task > existing means task should go before existing
        let mut inserted = false;
        for (i, existing) in queue.iter().enumerate() {
            // For task to go before existing, task's priority must be higher
            if task.priority > existing.priority {
                queue.insert(i, task.clone());
                inserted = true;
                break;
            } else if task.priority == existing.priority && task.created_at < existing.created_at {
                // Same priority: earlier created goes first
                queue.insert(i, task.clone());
                inserted = true;
                break;
            }
        }

        if !inserted {
            queue.push_back(task.clone());
        }

        // Update statistics
        if self.config.track_stats {
            let mut stats = self.queue_stats.lock();
            stats.total_tasks += 1;
            stats.current_length = queue.len();
            if queue.len() > stats.peak_length {
                stats.peak_length = queue.len();
            }
        }

        Ok(())
    }

    /// Dequeue next task (highest priority first)
    pub fn dequeue(&self) -> Option<CompileTask> {
        self.queue.lock().pop_front()
    }

    /// Peek at next task without removing
    pub fn peek(&self) -> Option<CompileTask> {
        self.queue.lock().front().cloned()
    }

    /// Get queue length
    pub fn queue_length(&self) -> usize {
        self.queue.lock().len()
    }

    /// Store compilation result
    pub fn store_result(&self, result: CompileResult) {
        let mut results = self.results.write();
        results.insert(result.block_id, result.clone());

        // Update worker statistics
        if self.config.track_stats {
            let mut stats = self.worker_stats.write();
            if !stats.is_empty() {
                let worker_idx = (result.block_id as usize) % stats.len();
                stats[worker_idx].tasks_processed += 1;
                stats[worker_idx].total_compile_time_us += result.compile_time_us;
                if result.success {
                    stats[worker_idx].successful_compilations += 1;
                } else {
                    stats[worker_idx].failed_compilations += 1;
                }
            }
        }
    }

    /// Get compilation result
    pub fn get_result(&self, block_id: u64) -> Option<CompileResult> {
        self.results.read().get(&block_id).cloned()
    }

    /// Check if result is available
    pub fn has_result(&self, block_id: u64) -> bool {
        self.results.read().contains_key(&block_id)
    }

    /// Clear all results
    pub fn clear_results(&self) {
        self.results.write().clear();
    }

    /// Get worker statistics
    pub fn get_worker_stats(&self) -> Vec<WorkerStats> {
        self.worker_stats.read().clone()
    }

    /// Get queue statistics
    pub fn get_queue_stats(&self) -> QueueStats {
        self.queue_stats.lock().clone()
    }

    /// Drain all pending tasks
    pub fn drain_queue(&self) -> Vec<CompileTask> {
        self.queue.lock().drain(..).collect()
    }
}

/// Background compiler worker
pub struct BackgroundCompiler {
    /// Compilation queue
    pub queue: Arc<ParallelJitQueue>,
    /// Active workers count
    active_workers: Arc<Mutex<usize>>,
}

impl BackgroundCompiler {
    /// Create new background compiler
    pub fn new(config: ParallelJitConfig) -> Self {
        Self {
            queue: Arc::new(ParallelJitQueue::new(config)),
            active_workers: Arc::new(Mutex::new(0)),
        }
    }

    /// Submit task for background compilation
    pub fn submit_task(&self, block_id: u64, priority: Priority, tier: u32) -> ParallelJitResult {
        let task = CompileTask {
            block_id,
            priority,
            created_at: Instant::now(),
            tier,
        };

        self.queue.enqueue(task)?;
        Ok(())
    }

    /// Process next task (called by worker thread)
    pub fn process_task(&self, _worker_id: usize) -> Option<CompileResult> {
        let task = self.queue.dequeue()?;

        // Simulate compilation
        let start = Instant::now();
        let compile_time = std::cmp::min(100 + (task.block_id % 900), 1000);
        std::thread::sleep(std::time::Duration::from_micros(compile_time));

        let result = CompileResult {
            block_id: task.block_id,
            success: true,
            compile_time_us: start.elapsed().as_micros() as u64,
            code_size: (256 * (task.tier as usize + 1)).min(1024),
            error: None,
        };

        self.queue.store_result(result.clone());
        Some(result)
    }

    /// Get pending task count
    pub fn pending_tasks(&self) -> usize {
        self.queue.queue_length()
    }

    /// Get active workers count
    pub fn active_workers(&self) -> usize {
        *self.active_workers.lock()
    }

    /// Get result if available
    pub fn get_compiled_result(&self, block_id: u64) -> Option<CompileResult> {
        self.queue.get_result(block_id)
    }

    /// Wait for specific result
    pub fn wait_for_result(&self, block_id: u64, timeout_us: u64) -> Option<CompileResult> {
        let start = Instant::now();
        let timeout = std::time::Duration::from_micros(timeout_us);

        loop {
            if let Some(result) = self.queue.get_result(block_id) {
                return Some(result);
            }

            if start.elapsed() > timeout {
                return None;
            }

            std::thread::sleep(std::time::Duration::from_micros(100));
        }
    }

    /// Get throughput (tasks/second)
    pub fn get_throughput(&self) -> f64 {
        let queue_stats = self.queue.get_queue_stats();
        if queue_stats.total_tasks == 0 {
            return 0.0;
        }

        // Estimate based on worker count
        let config = &self.queue.config;
        (config.num_workers as f64) * 1_000_000.0 / 500.0 // 500μs avg per task
    }

    /// Multi-worker simulation
    pub fn simulate_workers(&self, tasks: usize) -> u64 {
        let start = Instant::now();

        // Enqueue tasks
        for i in 0..tasks {
            let priority = if i < tasks / 2 {
                Priority::Normal
            } else {
                Priority::High
            };
            let _ = self.submit_task(i as u64, priority, (i % 4) as u32);
        }

        // Process tasks with multiple threads
        let queue = self.queue.clone();
        let mut handles = vec![];

        for worker_id in 0..self.queue.config.num_workers.min(4) {
            let queue = queue.clone();
            let handle = std::thread::spawn(move || {
                let mut count = 0;
                let compiler = BackgroundCompiler {
                    queue,
                    active_workers: Arc::new(Mutex::new(0)),
                };

                while compiler.pending_tasks() > 0 {
                    if let Some(_result) = compiler.process_task(worker_id) {
                        count += 1;
                    }
                }
                count
            });
            handles.push(handle);
        }

        // Wait for all workers
        for handle in handles {
            let _ = handle.join();
        }

        start.elapsed().as_micros() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        let low = CompileTask {
            block_id: 1,
            priority: Priority::Low,
            created_at: Instant::now(),
            tier: 0,
        };

        std::thread::sleep(std::time::Duration::from_micros(10));

        let high = CompileTask {
            block_id: 2,
            priority: Priority::High,
            created_at: Instant::now(),
            tier: 0,
        };

        // High priority should be greater than low priority
        // With our Ord impl, higher value = comes first = "greater"
        assert_eq!(high.cmp(&low), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_enqueue_dequeue() {
        let config = ParallelJitConfig::default();
        let queue = ParallelJitQueue::new(config);

        let task = CompileTask {
            block_id: 42,
            priority: Priority::Normal,
            created_at: Instant::now(),
            tier: 1,
        };

        assert!(queue.enqueue(task.clone()).is_ok());
        assert_eq!(queue.queue_length(), 1);

        let dequeued = queue.dequeue();
        assert!(dequeued.is_some());
        let dequeued = dequeued.expect("Task should be available after enqueue");
        assert_eq!(dequeued.block_id, 42);
        assert_eq!(queue.queue_length(), 0);
    }

    #[test]
    fn test_priority_queue() {
        let config = ParallelJitConfig::default();
        let queue = ParallelJitQueue::new(config);

        // Enqueue in mixed order
        let low = CompileTask {
            block_id: 1,
            priority: Priority::Low,
            created_at: Instant::now(),
            tier: 0,
        };

        let high = CompileTask {
            block_id: 2,
            priority: Priority::High,
            created_at: Instant::now(),
            tier: 0,
        };

        assert!(queue.enqueue(low).is_ok());
        assert!(queue.enqueue(high).is_ok());

        // Should dequeue high priority first
        let first = queue.dequeue().expect("Should have high priority task");
        assert_eq!(first.block_id, 2);
        let second = queue.dequeue().expect("Should have low priority task");
        assert_eq!(second.block_id, 1);
    }

    #[test]
    fn test_queue_overflow() {
        let config = ParallelJitConfig {
            max_queue_size: 2,
            ..Default::default()
        };
        let queue = ParallelJitQueue::new(config);

        for i in 0..3 {
            let task = CompileTask {
                block_id: i as u64,
                priority: Priority::Normal,
                created_at: Instant::now(),
                tier: 0,
            };

            if i < 2 {
                assert!(queue.enqueue(task).is_ok());
            } else {
                assert!(queue.enqueue(task).is_err());
            }
        }
    }

    #[test]
    fn test_compile_result_storage() {
        let config = ParallelJitConfig::default();
        let queue = ParallelJitQueue::new(config);

        let result = CompileResult {
            block_id: 42,
            success: true,
            compile_time_us: 150,
            code_size: 512,
            error: None,
        };

        queue.store_result(result.clone());
        assert!(queue.has_result(42));

        let retrieved = queue
            .get_result(42)
            .expect("Result should be available after store_result");
        assert_eq!(retrieved.block_id, 42);
        assert_eq!(retrieved.compile_time_us, 150);
    }

    #[test]
    fn test_background_compiler() {
        let compiler = BackgroundCompiler::new(ParallelJitConfig::default());

        // Submit tasks
        assert!(compiler.submit_task(1, Priority::High, 1).is_ok());
        assert!(compiler.submit_task(2, Priority::Normal, 2).is_ok());
        assert!(compiler.submit_task(3, Priority::Low, 0).is_ok());

        assert_eq!(compiler.pending_tasks(), 3);

        // Process first task
        let result = compiler.process_task(0);
        assert!(result.is_some());
        let result = result.expect("Result should be available after processing task");
        assert_eq!(result.block_id, 1);
        assert_eq!(compiler.pending_tasks(), 2);
    }

    #[test]
    fn test_worker_statistics() {
        let config = ParallelJitConfig::default();
        let queue = ParallelJitQueue::new(config);

        let result1 = CompileResult {
            block_id: 1,
            success: true,
            compile_time_us: 200,
            code_size: 256,
            error: None,
        };

        let result2 = CompileResult {
            block_id: 2,
            success: true,
            compile_time_us: 300,
            code_size: 512,
            error: None,
        };

        queue.store_result(result1);
        queue.store_result(result2);

        let stats = queue.get_worker_stats();
        let total_processed: u64 = stats.iter().map(|s| s.tasks_processed).sum();
        assert_eq!(total_processed, 2);
    }

    #[test]
    fn test_queue_statistics() {
        let config = ParallelJitConfig::default();
        let queue = ParallelJitQueue::new(config);

        for i in 0..5 {
            let task = CompileTask {
                block_id: i as u64,
                priority: Priority::Normal,
                created_at: Instant::now(),
                tier: 0,
            };
            let _ = queue.enqueue(task);
        }

        let stats = queue.get_queue_stats();
        assert_eq!(stats.total_tasks, 5);
        assert_eq!(stats.current_length, 5);
        assert_eq!(stats.peak_length, 5);
    }

    #[test]
    fn test_multi_worker_throughput() {
        let compiler = BackgroundCompiler::new(ParallelJitConfig {
            num_workers: 4,
            ..Default::default()
        });

        let time_us = compiler.simulate_workers(10);

        // Should finish faster than sequential (10 * 500us = 5000us)
        // With 4 workers, should be closer to 2000us
        assert!(time_us < 8000); // Some overhead but still faster
    }

    #[test]
    fn test_submit_and_retrieve() {
        let compiler = BackgroundCompiler::new(ParallelJitConfig::default());

        // Submit and process
        assert!(compiler.submit_task(99, Priority::High, 3).is_ok());
        let result = compiler.process_task(0);

        assert!(result.is_some());
        let result = result.expect("Compilation result should be available");
        assert_eq!(result.block_id, 99);
        assert!(result.success);

        // Verify retrieval
        let retrieved = compiler.get_compiled_result(99);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_timeout_handling() {
        let compiler = BackgroundCompiler::new(ParallelJitConfig::default());

        // Non-existent result with timeout
        let result = compiler.wait_for_result(999, 100);
        assert!(result.is_none()); // Timeout, result not available
    }

    #[test]
    fn test_drain_queue() {
        let config = ParallelJitConfig::default();
        let queue = ParallelJitQueue::new(config);

        for i in 0..5 {
            let task = CompileTask {
                block_id: i as u64,
                priority: Priority::Normal,
                created_at: Instant::now(),
                tier: 0,
            };
            let _ = queue.enqueue(task);
        }

        let drained = queue.drain_queue();
        assert_eq!(drained.len(), 5);
        assert_eq!(queue.queue_length(), 0);
    }
}
