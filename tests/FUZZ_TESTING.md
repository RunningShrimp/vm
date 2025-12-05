# 模糊测试文档

## 概述

本项目包含针对关键组件的模糊测试，用于发现边界条件和潜在bug。

## 测试文件

### 1. `fuzz_tests.rs` - 基础模糊测试
- **fuzz_random_ir_blocks**: 随机IR块执行测试
- **fuzz_random_memory_access**: 随机内存访问测试
- **fuzz_address_translation**: 地址翻译测试
- **fuzz_edge_cases**: 边界条件测试
- **fuzz_concurrent_execution**: 并发执行测试
- **fuzz_large_blocks**: 大块执行测试
- **fuzz_ir_generation_edge_cases**: IR生成边界条件测试
- **fuzz_ir_operation_combinations**: IR操作组合测试
- **fuzz_ir_terminator_combinations**: IR终止符组合测试

### 2. `fuzz_mmu_tests.rs` - MMU模糊测试
- **fuzz_mmu_read_write**: MMU内存读写操作测试
- **fuzz_mmu_bulk_operations**: MMU批量操作测试
- **fuzz_mmu_translation**: MMU地址翻译测试
- **fuzz_mmu_edge_cases**: MMU边界条件测试
- **fuzz_mmu_alignment**: MMU对齐访问测试
- **fuzz_mmu_tlb_flush**: MMU TLB刷新测试
- **fuzz_mmu_concurrent_access**: MMU并发访问测试
- **fuzz_mmu_large_blocks**: MMU大块内存操作测试

### 3. `fuzz_jit_tests.rs` - JIT编译模糊测试
- **fuzz_jit_compile_random_blocks**: JIT编译随机IR块测试
- **fuzz_jit_hotspot_detection**: JIT热点检测测试
- **fuzz_jit_edge_cases**: JIT编译边界条件测试
- **fuzz_jit_register_allocation**: JIT寄存器分配测试
- **fuzz_jit_memory_operations**: JIT内存操作测试
- **fuzz_jit_control_flow**: JIT控制流测试

## 运行测试

### 运行所有模糊测试
```bash
cargo test --test fuzz_tests
cargo test --test fuzz_mmu_tests
cargo test --test fuzz_jit_tests
```

### 运行特定测试
```bash
cargo test fuzz_mmu_read_write
cargo test fuzz_jit_compile_random_blocks
```

### 运行所有测试（包括模糊测试）
```bash
cargo test
```

## CI/CD集成

### GitHub Actions示例

```yaml
name: Fuzz Tests

on: [push, pull_request]

jobs:
  fuzz-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run fuzz tests
        run: |
          cargo test --test fuzz_tests
          cargo test --test fuzz_mmu_tests
          cargo test --test fuzz_jit_tests
```

### 使用cargo-fuzz（可选）

如果需要更深入的模糊测试，可以使用`cargo-fuzz`：

```bash
# 安装cargo-fuzz
cargo install cargo-fuzz

# 初始化fuzz目标
cargo fuzz init

# 运行fuzz测试
cargo fuzz run fuzz_mmu_operations
```

## 测试策略

1. **随机输入生成**: 使用`rand` crate生成随机输入
2. **边界值测试**: 测试各种边界条件（0, MAX, MIN等）
3. **并发测试**: 测试多线程/多协程场景
4. **压力测试**: 测试大量操作和大块内存
5. **组合测试**: 测试不同操作的组合

## 注意事项

- 模糊测试可能会运行较长时间
- 某些测试可能会失败（返回错误），但不应该panic
- 如果发现panic，应该报告为bug
- 测试覆盖率可以通过`cargo tarpaulin`查看

## 扩展测试

要添加新的模糊测试：

1. 在相应的测试文件中添加新的测试函数
2. 使用`#[test]`属性标记
3. 确保测试不会panic（即使输入无效）
4. 添加适当的文档注释

## 性能考虑

- 模糊测试默认运行1000-10000次迭代
- 可以根据需要调整迭代次数
- 大块测试可能需要更多时间
- 并发测试使用10个线程/协程


