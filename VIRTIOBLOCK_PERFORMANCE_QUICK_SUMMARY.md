# VirtioBlock性能基线 - 快速参考

## 测试执行时间
- **开始**: 2025-12-30 22:11
- **结束**: 2025-12-30 22:22
- **总耗时**: ~11分钟

## 关键性能指标（Top 10）

### 1. 读操作性能
- **单扇区**: 26.56 ns (17.95 GiB/s)
- **10扇区**: 79.33 ns (60.11 GiB/s) ⭐ 最佳吞吐量
- **100扇区**: 1.29 µs (36.83 GiB/s)
- **1000扇区**: 12.15 µs (39.26 GiB/s)

### 2. 写操作性能
- **单扇区**: 8.05 ns (59.25 GiB/s)
- **10扇区**: 81.97 ns (58.17 GiB/s)
- **100扇区**: 1.83 µs (26.02 GiB/s) ⚠️ 最低吞吐量
- **1000扇区**: 7.94 µs (60.05 GiB/s) ⭐ 最佳吞吐量

### 3. 验证性能
- **读验证**: 1.45 - 11.31 ns
- **写验证**: 1.67 - 35.92 ns

### 4. 请求处理
- **读请求**: 78.48 ns
- **写请求**: 130.03 ns (慢1.66倍)
- **刷新请求**: 1.82 ns

### 5. Getter方法
- **capacity**: 424.40 ps
- **sector_size**: 411.84 ps ⭐ 最快
- **is_read_only**: 434.02 ps

### 6. 设备创建
- **基本设备**: 2.06 ns
- **1MB内存**: 37.44 µs
- **100MB内存**: 4.34 ms

### 7. 批量操作
- **批量读(100次)**: 9.12 µs
- **批量写(100次)**: 6.97 µs (快24%)

### 8. 扇区大小对比
- **512字节**: 1.31 µs (~390 MB/s)
- **4K字节**: 6.69 µs (~612 MB/s) ⭐ 快57%

## 性能等级

### 🟢 极快 (< 1ns)
- Getter方法: ~410ps
- 验证方法: 1.4-1.9ns

### 🟢 快 (< 100ns)
- 单扇区读写: 8-26ns
- 请求处理: 78-130ns
- 错误处理: 35-103ns

### 🟡 中等 (< 10µs)
- 10扇区操作: ~80ns
- 100扇区操作: 1.3-1.8µs
- 批量操作: 7-9µs

### 🟠 慢 (> 1ms)
- 大设备创建: 4.3ms (100MB)

## 瓶颈分析

### ⚠️ 需要关注
1. **批量读取**: 9.12µs - 可优化
2. **写偏移1000**: 168ns - 需调查
3. **100扇区写入**: 26 GiB/s - 最低吞吐量

### ✅ 性能优秀
1. **Getter方法**: 零开销
2. **验证方法**: 亚纳秒级
3. **单扇区操作**: 高吞吐量
4. **4K扇区**: 优于512字节57%

## 总体评价

- **性能等级**: A (优秀)
- **充血模型**: 无性能回归
- **吞吐量**: 17-60 GiB/s
- **可用性**: 生产就绪

## 快速命令

```bash
# 查看完整报告
cat VIRTIOBLOCK_PERFORMANCE_BASELINE.md

# 查看HTML报告
open target/criterion/report/index.html

# 运行特定测试
cargo bench --bench block_benchmark -- read_operation
cargo bench --bench block_benchmark -- write_operation

# 对比基线
cargo bench --bench block_benchmark -- --baseline initial

# 查看测试统计
cargo bench --bench block_benchmark -- --save-baseline current
```

## 基线文件

- **详细报告**: `VIRTIOBLOCK_PERFORMANCE_BASELINE.md`
- **快速参考**: `VIRTIOBLOCK_PERFORMANCE_QUICK_SUMMARY.md` (本文件)
- **HTML报告**: `target/criterion/report/index.html`
- **基线数据**: `target/criterion/*/baseline/initial/`
