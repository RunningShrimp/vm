# VM项目测试覆盖率提升 - 快速参考指南

**最后更新**: 2025-12-31  
**完整报告**: `TEST_COVERAGE_FINAL_REPORT.md`

---

## 📊 当前状态摘要

### 整体情况
- **当前覆盖率**: ~60-70%
- **目标覆盖率**: 80%+
- **差距**: 需要增加约200-300个测试

### 各模块状态

| 模块 | 覆盖率 | 测试数 | 状态 | 优先级 |
|------|-------|--------|------|--------|
| vm-frontend | 0% | 0 | 🔴 严重 | P0 |
| vm-engine | 60-70% | 86+ | 🔴 SIGBUS错误 | P0 |
| vm-core | 55-65% | 110 | 🟡 中等 | P1 |
| vm-mem | 70-75% | 121 | 🟢 良好 | P1 |
| vm-device | 70-75% | 121 | 🟢 良好 | P2 |
| vm-accel | 55-65% | 64 | 🟡 1个失败 | P2 |

---

## ✅ 已完成

1. ✅ 修复vm-engine JITConfig编译错误
2. ✅ 修复vm-device重复模块定义
3. ✅ 分析所有主要crate测试状况
4. ✅ 创建vm-frontend测试框架(需修复编译错误)
5. ✅ 生成完整改进报告

---

## 🚨 立即行动项

### 今天/明天必须完成

1. **修复vm-engine SIGBUS错误** (4-6小时)
   ```bash
   # 运行详细测试
   RUST_BACKTRACE=1 cargo test --package vm-engine --lib
   
   # 定位并修复内存访问错误
   ```

2. **修复vm-accel测试** (1-2小时)
   ```bash
   # 文件: vm-accel/src/hvf.rs
   # 添加平台检测或条件编译
   ```

3. **修复vm-frontend测试编译** (2-3小时)
   ```bash
   # 文件: vm-frontend/src/riscv64/tests.rs
   # 修复:
   # - 添加Decoder trait导入
   # - 修复字符串切片问题
   # - 添加#[cfg(feature = "all")]
   ```

---

## 📅 本周目标 (阶段1)

**目标**: 稳定测试基础

- [ ] vm-engine: 所有测试通过
- [ ] vm-accel: 所有测试通过
- [ ] vm-frontend: 0% → 25% (至少60个测试)
- [ ] 设置基础CI/CD

**验收**:
```bash
cargo test --workspace
# 预期: ok. XXX passed; 0 failed
```

---

## 🎯 未来2-3周目标 (阶段2)

**目标**: 核心覆盖率提升

- [ ] vm-frontend: 25% → 75% (280个测试)
- [ ] vm-core: 55% → 80% (90个测试)
- [ ] vm-engine: 60% → 75% (80个测试)
- [ ] 整体: 70% → **80%** ✅

---

## 🔧 快速命令

### 测试命令
```bash
# 测试单个crate
cargo test --package vm-core --lib

# 测试并显示输出
cargo test --package vm-mem -- --nocapture

# 运行所有测试
cargo test --workspace

# 测试特定功能
cargo test --package vm-core test_vm_id
```

### 覆盖率命令
```bash
# 安装工具
cargo install cargo-tarpaulin

# 生成HTML覆盖率报告
cargo tarpaulin --workspace --out Html --output-dir coverage

# 查看报告
open coverage/index.html
```

### 调试命令
```bash
# 带栈回溯运行
RUST_BACKTRACE=1 cargo test --package vm-engine

# 单线程运行(避免并发问题)
cargo test --workspace -- --test-threads=1

# 只编译不运行
cargo test --workspace --no-run
```

---

## 📁 重要文件

### 报告文档
- **完整报告**: `TEST_COVERAGE_FINAL_REPORT.md`
- **快速参考**: `QUICK_TEST_GUIDE.md` (本文件)
- **旧报告**: `TEST_COVERAGE_IMPROVEMENT_REPORT.md`

### 测试文件
- **vm-frontend测试**: `vm-frontend/src/riscv64/tests.rs` (新创建,需修复)
- **vm-engine测试**: `vm-engine/tests/jit_compiler_tests.rs` (已修复)

### 配置文件
- **CI配置** (需创建): `.github/workflows/coverage.yml`
- **测试脚本** (需创建): `scripts/quick_test.sh`

---

## 💡 测试模板

### 基础单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // Arrange
        let input = create_test_input();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result, expected);
    }

    #[test]
    fn test_edge_cases() {
        // 边界测试
        assert_eq!(func(0), expected_min);
        assert_eq!(func(u64::MAX), expected_max);
    }

    #[test]
    fn test_error_handling() {
        let result = function_that_can_fail(invalid_input);
        assert!(result.is_err());
    }
}
```

### 集成测试
```rust
// tests/integration/full_pipeline_test.rs

#[test]
fn test_decode_compile_execute() {
    // 1. 创建VM
    let mut vm = create_test_vm();
    
    // 2. 加载程序
    vm.load_binary(GuestAddr(0), &binary);
    
    // 3. 解码指令
    let insn = decoder.decode(&vm.mmu, GuestAddr(0))?;
    
    // 4. 编译
    let compiled = jit.compile(&insn)?;
    
    // 5. 执行
    let result = vm.execute(compiled);
    
    // 6. 验证
    assert!(result.is_ok());
}
```

---

## 🎓 最佳实践

### 测试命名
- ✅ `test_decode_lui_instruction` - 清晰描述测试内容
- ❌ `test1` - 无意义
- ❌ `test_it_works` - 过于模糊

### 测试结构
遵循AAA模式:
- **Arrange**: 准备测试数据
- **Act**: 执行被测功能
- **Assert**: 验证结果

### 测试独立
- 每个测试应该独立运行
- 不依赖其他测试
- 不依赖执行顺序

### 测试速度
- 单元测试应该快速 (<100ms)
- 使用mock避免慢速操作
- 集成测试可以慢些,但要标记

---

## 🚀 下一步

1. **立即**: 修复vm-engine SIGBUS错误
2. **今天**: 修复vm-accel和vm-frontend编译错误
3. **本周**: 完成阶段1目标
4. **下周**: 开始阶段2核心测试

详细计划见: `TEST_COVERAGE_FINAL_REPORT.md`

---

**祝测试愉快! 🎉**

有问题?查看完整报告或联系维护团队。
