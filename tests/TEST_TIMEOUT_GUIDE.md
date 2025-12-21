# 测试超时策略指南

## 概述

为了防止测试因死锁、无限循环或资源竞争而卡死，所有测试都应该配置适当的超时时间。

## 超时时间配置

### 默认超时时间

根据测试类型，使用以下默认超时时间：

- **单元测试**: 30秒
- **集成测试**: 60秒
- **性能测试**: 120秒
- **并发测试**: 180秒（3分钟）
- **压力测试**: 300秒（5分钟）
- **端到端测试**: 600秒（10分钟）

### 使用方法

#### 1. 同步测试

使用 `test_with_timeout!` 宏：

```rust
use test_timeout_utils::test_with_timeout;

test_with_timeout!(60, test_my_function, {
    // 测试代码
    assert_eq!(1 + 1, 2);
});
```

#### 2. 异步测试

使用 `tokio_test_with_timeout!` 宏：

```rust
use test_timeout_utils::tokio_test_with_timeout;

tokio_test_with_timeout!(120, test_async_function, {
    // 异步测试代码
    let result = some_async_function().await;
    assert!(result.is_ok());
});
```

#### 3. 手动超时检查

对于复杂的测试，可以手动实现超时：

```rust
#[test]
fn test_complex_scenario() {
    use std::time::{Duration, Instant};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    
    let timeout = Duration::from_secs(180);
    let start = Instant::now();
    let timed_out = Arc::new(AtomicBool::new(false));
    
    // 启动超时监控
    let timed_out_clone = Arc::clone(&timed_out);
    thread::spawn(move || {
        thread::sleep(timeout);
        timed_out_clone.store(true, Ordering::Release);
    });
    
    // 执行测试，定期检查超时
    loop {
        if timed_out.load(Ordering::Acquire) {
            panic!("测试超时");
        }
        // 测试逻辑
        if test_complete {
            break;
        }
    }
}
```

## 测试类型识别

测试超时配置模块 (`test_timeout_config.rs`) 可以根据测试名称自动推断超时时间：

```rust
use test_timeout_config::TestTimeoutConfig;

let timeout = TestTimeoutConfig::infer_timeout("test_concurrent_access");
// 返回 180 秒（并发测试）
```

## 最佳实践

1. **所有长时间运行的测试都应该有超时**
   - 并发测试
   - 性能测试
   - 压力测试
   - 集成测试

2. **超时时间应该合理**
   - 太短：正常测试可能被误判为超时
   - 太长：死锁问题可能长时间不被发现

3. **超时错误信息应该清晰**
   - 包含超时时间
   - 包含实际耗时
   - 提示可能的原因（死锁、无限循环等）

4. **定期检查超时配置**
   - 如果测试经常超时，考虑优化测试或增加超时时间
   - 如果测试从不超时但时间很长，考虑优化测试性能

## 已配置超时的测试

以下测试已经配置了超时：

- `tests/concurrent_safety_tests.rs`: 120秒
- `benches/phase2_performance_benchmark.rs`: 60-120秒
- `vm-engine-interpreter/tests/async_executor_performance_tests.rs`: 60-120秒

## 故障排查

如果测试超时：

1. **检查是否是死锁**
   - 查看测试日志
   - 使用调试器检查线程状态

2. **检查是否是无限循环**
   - 添加日志输出
   - 检查循环条件

3. **检查是否是资源竞争**
   - 减少并发数
   - 增加同步点

4. **检查超时时间是否合理**
   - 在正常环境下运行测试，记录实际耗时
   - 将超时时间设置为实际耗时的2-3倍

## 相关文件

- `tests/test_timeout_utils.rs`: 超时工具和宏
- `tests/test_timeout_config.rs`: 超时配置
- `.cargo/config.toml`: Cargo测试配置（文档说明）

