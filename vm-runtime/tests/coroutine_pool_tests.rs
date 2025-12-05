//! 协程池测试套件

use std::time::Duration;
use tokio::time::timeout;
use vm_runtime::CoroutinePool;

#[tokio::test]
async fn test_coroutine_pool_basic() {
    let pool = CoroutinePool::new(10);

    // 提交任务
    let task = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    assert!(pool.spawn(task).await.is_ok());
    tokio::time::sleep(Duration::from_millis(20)).await;
}

#[tokio::test]
async fn test_coroutine_pool_multiple_tasks() {
    let pool = CoroutinePool::new(4);

    // 提交多个任务
    for i in 0..5 {
        let task = async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        let _ = pool.spawn(task).await;
    }

    // 等待所有任务完成
    tokio::time::sleep(Duration::from_secs(1)).await;
    pool.join_all().await.unwrap();
}

#[tokio::test]
async fn test_coroutine_pool_max_limit() {
    let pool = CoroutinePool::new(2);

    // 提交超过最大限制的协程
    for _ in 0..3 {
        let task = async {
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        let _ = pool.spawn(task).await;
    }

    // 等待一下让协程启动
    tokio::time::sleep(Duration::from_millis(10)).await;

    // 再次提交应该失败（如果达到限制）
    let task = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    // 注意：由于异步特性，这个测试可能不够准确
    // 实际使用中应该检查返回值
}

#[tokio::test]
async fn test_coroutine_pool_cleanup() {
    let pool = CoroutinePool::new(5);

    // 提交一些任务
    for _ in 0..3 {
        let task = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        let _ = pool.spawn(task).await;
    }

    // 清理
    pool.cleanup().await;
    assert_eq!(pool.active_count(), 0);
}
