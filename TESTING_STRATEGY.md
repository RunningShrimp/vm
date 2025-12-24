# 单元测试策略

本文档定义了虚拟机系统的单元测试策略，目标是达到 80% 以上的代码覆盖率。

## 目录

- [测试覆盖目标](#测试覆盖目标)
- [测试框架](#测试框架)
- [测试分类](#测试分类)
- [测试计划](#测试计划)
- [最佳实践](#最佳实践)

---

## 测试覆盖目标

### 目标覆盖率

| 模块 | 目标覆盖率 | 当前状态 |
|-------|----------|---------|
| vm-core | 85% | 待评估 |
| vm-mem | 85% | 待评估 |
| vm-engine-jit | 85% | 待评估 |
| vm-runtime | 85% | 待评估 |
| vm-cross-arch | 85% | 待评估 |

### 测试类型分布

| 测试类型 | 比例 | 说明 |
|---------|------|------|
| 单元测试 | 70% | 测试单个函数/方法 |
| 集成测试 | 20% | 测试模块间交互 |
| 性能测试 | 5% | 测试性能指标 |
| 压力测试 | 5% | 测试极限场景 |

---

## 测试框架

### 主要框架

- **单元测试**: Rust 内置的 `#[test]` 属性
- **属性测试**: `proptest` crate 用于生成随机输入
- **模拟测试**: `mockall` crate 用于 mock 依赖
- **异步测试**: `tokio::test` 用于异步函数测试

### 测试配置

```toml
[dev-dependencies]
proptest = "1.0"
mockall = "0.11"
tokio = { version = "1.0", features = ["test-util"] }
criterion = "0.5"  # 基准测试
```

---

## 测试分类

### 1. 值对象测试

**目标**: 确保值对象正确验证输入数据

#### 测试示例：VmId

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_id_valid() {
        let id = VmId::new("vm-123".to_string());
        assert!(id.is_ok());
        assert_eq!(id.unwrap().as_str(), "vm-123");
    }

    #[test]
    fn test_vm_id_too_short() {
        let id = VmId::new("".to_string());
        assert!(id.is_err());
    }

    #[test]
    fn test_vm_id_invalid_characters() {
        let id = VmId::new("vm@123".to_string());
        assert!(id.is_err());
    }
}
```

#### 测试示例：MemorySize

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_size_from_bytes() {
        let size = MemorySize::from_bytes(1024 * 1024);
        assert!(size.is_ok());
        assert_eq!(size.unwrap().bytes(), 1024 * 1024);
    }

    #[test]
    fn test_memory_size_to_mb() {
        let size = MemorySize::from_mb(256).unwrap();
        assert_eq!(size.as_mb(), 256);
    }

    #[test]
    fn test_memory_size_page_aligned() {
        let size = MemorySize::from_bytes(4096).unwrap();
        assert!(size.is_page_aligned());

        let size = MemorySize::from_bytes(4095).unwrap();
        assert!(!size.is_page_aligned());
    }
}
```

### 2. 领域服务测试

**目标**: 确保领域服务正确执行业务逻辑

#### 测试示例：AddressTranslationDomainService

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_x86_64_success() {
        let service = create_test_service();
        let result = service.translate(GuestAddr(0x1000), GuestAddr(0));
        assert!(result.is_ok());
    }

    #[test]
    fn test_translate_page_fault() {
        let service = create_test_service();
        let result = service.translate(GuestAddr(0x1000), GuestAddr(0));
        assert!(result.is_err());
        match result {
            Err(VmError::Execution(ExecutionError::Fault(Fault::PageFault { .. }))) => {}
            _ => panic!("Expected PageFault"),
        }
    }
}
```

### 3. 聚合根测试

**目标**: 确保聚合根正确管理状态和事件

#### 测试示例：VirtualMachineAggregate

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_creation() {
        let id = VmId::new("vm-1".to_string()).unwrap();
        let config = VmConfig::default();
        let aggregate = VirtualMachineAggregate::new(id, config);

        assert_eq!(aggregate.aggregate_id(), "vm-1");
    }

    #[test]
    fn test_event_publishing() {
        let mut aggregate = create_test_aggregate();
        aggregate.start();

        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DomainEventEnum::VmStartedEvent(_)));
    }

    #[test]
    fn test_event_commit() {
        let mut aggregate = create_test_aggregate();
        aggregate.start();
        aggregate.mark_events_as_committed();

        let events = aggregate.uncommitted_events();
        assert_eq!(events.len(), 0);
    }
}
```

### 4. 错误处理测试

**目标**: 确保错误正确生成和转换

#### 测试示例：VmError

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_context() {
        let error = VmError::Memory(MemoryError::AccessViolation { ... });
        let wrapped = error.context("Test context");
        assert!(wrapped.to_string().contains("Test context"));
    }

    #[test]
    fn test_error_recovery() {
        let error = VmError::Device(DeviceError::Busy { ... });
        assert!(error.is_retryable());
    }
}
```

### 5. 性能测试

**目标**: 确保性能符合预期

#### 测试示例：TLB 命中率

```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    use criterion::{criterion_group, criterion_main, Criterion};

    fn benchmark_tlb_lookup(c: &mut Criterion) {
        let mut tlb = MultiLevelTlb::new(config);
        tlb.update(create_test_entry());

        c.bench_function("tlb_lookup", |b| {
            b.iter(|| {
                tlb.lookup(GuestAddr(0x1000), 0, AccessType::Read);
            });
        });
    }

    criterion_group!(benches, benchmark_tlb_lookup);
    criterion_main!(benches);
}
```

---

## 测试计划

### Phase 1: 值对象测试（1 周）

| 模块 | 文件 | 测试数量 |
|-------|------|---------|
| vm-core | value_objects_tests.rs | 20 |
| vm-core | aggregate_root_tests.rs | 15 |
| vm-core | domain_events_tests.rs | 10 |

### Phase 2: 领域服务测试（2 周）

| 模块 | 文件 | 测试数量 |
|-------|------|---------|
| vm-mem | domain_services_tests.rs | 25 |
| vm-mem | tlb_tests.rs | 30 |
| vm-engine-jit | compiler_tests.rs | 20 |
| vm-engine-jit | codegen_tests.rs | 15 |

### Phase 3: JIT 引擎测试（2 周）

| 模块 | 文件 | 测试数量 |
|-------|------|---------|
| vm-engine-jit | tiered_compiler_tests.rs | 25 |
| vm-engine-jit | hotspot_detector_tests.rs | 15 |
| vm-engine-jit | inline_cache_tests.rs | 10 |
| vm-engine-jit | code_cache_tests.rs | 10 |

### Phase 4: 跨架构测试（2 周）

| 模块 | 文件 | 测试数量 |
|-------|------|---------|
| vm-cross-arch | translator_tests.rs | 30 |
| vm-cross-arch | ir_optimizer_tests.rs | 15 |
| vm-cross-arch | register_allocator_tests.rs | 10 |

### Phase 5: 运行时测试（1 周）

| 模块 | 文件 | 测试数量 |
|-------|------|---------|
| vm-runtime | coroutine_scheduler_tests.rs | 20 |
| vm-runtime | coroutine_pool_tests.rs | 15 |
| vm-runtime | sandboxed_vm_tests.rs | 10 |

---

## 最佳实践

### 1. 测试命名

```rust
// 单元测试
fn test_<function_name>_<scenario>()

// 集成测试
fn test_integration_<module>_<scenario>()

// 错误测试
fn test_error_<error_type>_<condition>()
```

### 2. 测试组织

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // 辅助函数
    fn create_test_service() -> AddressTranslationDomainService {
        // ...
    }

    // 测试用例
    #[test]
    fn test_service_creation() {
        // ...
    }
}
```

### 3. Mock 依赖

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock!;

    mock! {
        MMUMock {
            fn read(&self, addr: GuestAddr, size: u8) -> Result<u64, VmError>;
        }
    }

    #[test]
    fn test_with_mock() {
        let mut mock = MMUMock::new();
        mock.expect_read()
            .returning(|_, _| Ok(0xDEADBEEF));

        let result = mock.read(GuestAddr(0), 4);
        assert_eq!(result.unwrap(), 0xDEADBEEF);
    }
}
```

### 4. 属性测试

```rust
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_vm_id_always_valid(id in "[a-zA-Z0-9_-]{1,64}") {
            let vm_id = VmId::new(id.to_string());
            assert!(vm_id.is_ok());
        }

        #[test]
        fn prop_memory_size_roundtrip(bytes in 1u64..1_000_000_000u64) {
            let size = MemorySize::from_bytes(bytes).unwrap();
            assert_eq!(size.bytes(), bytes);
        }
    }
}
```

---

## 测试覆盖率检查

### 使用 tarpaulin

```bash
# 运行测试并生成覆盖率报告
cargo tarpaulin --out Html

# 检查覆盖率
cargo tarpaulin --out Lcov --output-dir coverage
```

### 使用 grcov

```bash
# 生成覆盖率数据
CARGO_INCREMENTAL=0 RUSTFLAGS="-C instrument-coverage" \
    cargo test --no-fail-fast

# 生成 HTML 报告
grcov ./target/debug/deps/ \
    --binary-path ./target/debug/deps/ \
    -s . \
    -t html \
    --branch \
    --ignore-not-existing \
    -o ./coverage
```

---

## 持续集成

### GitHub Actions 配置

```yaml
name: Test Coverage

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
      - name: Run tests
        run: cargo test --all-features
      - name: Generate coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml
      - name: Upload to Codecov
        uses: codecov/codecov-action@v2
```

---

## 总结

本测试策略定义了：

1. **测试目标**：每个模块 85% 覆盖率
2. **测试分类**：单元测试、集成测试、性能测试、压力测试
3. **测试计划**：5 个阶段，按模块分组
4. **最佳实践**：测试命名、组织、Mock、属性测试
5. **覆盖率检查**：使用 tarpaulin 和 grcov
6. **持续集成**：GitHub Actions 自动化测试

遵循此策略，可以确保虚拟机系统的代码质量和可靠性。
