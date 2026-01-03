# 快速开始指南

**目标**: 帮助新贡献者快速上手VM项目开发
**更新**: 2025-12-31 (现代化后)

---

## 🚀 5分钟快速开始

### 前置要求

```bash
# 检查Rust安装
rustc --version  # 需要 >= 1.85

# 检查Cargo
cargo --version

# 检查Git
git --version
```

### 克隆项目

```bash
# 克隆仓库
git clone https://github.com/your-org/vm.git
cd vm

# 查看项目状态
cargo --version
ls -la
```

### 构建项目

```bash
# 构建整个workspace
cargo build --workspace

# 或构建特定crate
cargo build -p vm-core
cargo build -p vm-engine-jit
```

### 运行测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定测试
cargo test -p vm-core gc::
cargo test -p vm-engine-jit aot::
```

---

## 📁 项目结构

```
vm/
├── Cargo.toml                 # Workspace配置
├── README.md                  # 项目概述
│
├── docs/                      # 📚 项目文档
│   ├── TODO_AUDIT.md          # TODO审计（56个）
│   ├── MODERNIZATION_SUMMARY.md  # 现代化总结
│   ├── MIGRATION_GUIDE.md     # 迁移指南
│   ├── FINAL_COMPLETION_REPORT.md  # 完成报告
│   ├── WORK_VERIFICATION.md   # 质量验证
│   ├── ENVIRONMENT_NOTES.md   # 环境说明
│   └── QUICK_START.md         # 本文件
│
├── vm-core/                   # 核心模块
│   ├── src/gc/               # ✅ GC已修复
│   └── src/foundation/       # 基础类型
│
├── vm-engine-jit/            # JIT引擎
│   ├── src/aot_integration.rs # ✅ AOT已实现
│   ├── src/aot_format.rs     # ✅ AOT格式
│   └── src/lib.rs           # JIT核心
│
├── vm-frontend/             # 前端解码器
│   ├── src/riscv64/         # RISC-V 64位
│   ├── src/arm64/           # ARM64
│   └── src/x86_64/          # x86_64
│
├── vm-mem/                  # 内存管理
├── vm-device/               # 设备模拟
├── vm-accel/                # 硬件加速
├── vm-service/              # VM服务
└── vm-platform/             # 平台支持
```

---

## 🎯 开发工作流

### 1. 选择任务

查看待办事项：
```bash
# 查看TODO审计
cat docs/TODO_AUDIT.md

# 或搜索代码中的TODO
grep -rn "TODO" --include="*.rs" | grep -P1 "(P1|重要)"
```

推荐起点：
- **P1任务**: GPU直通、ARM NPU、JIT优化
- **P2任务**: 编译器增强、SIMD支持
- **P3任务**: 文档改进、测试补充

### 2. 创建分支

```bash
# 更新主分支
git checkout master
git pull origin master

# 创建功能分支
git checkout -b feat/your-feature-name

# 示例
git checkout -b feat/rocma-support
git checkout -b fix/gc-allocation
git checkout -b docs/update-readme
```

### 3. 开发

```bash
# 编辑代码
vim vm-core/src/gc/unified.rs

# 增量编译（更快）
cargo check -p vm-core

# 运行相关测试
cargo test -p vm-core gc::

# 代码格式化
cargo fmt

# 代码检查
cargo clippy -p vm-core
```

### 4. 提交

```bash
# 查看变更
git status
git diff

# 暂存文件
git add vm-core/src/gc/unified.rs
git add tests/test_gc.rs

# 提交（使用清晰的提交信息）
git commit -m "fix: implement actual memory allocation in GC

- Replace null_mut() return with real allocation
- Add heap space check and GC triggering
- Implement deallocate() function
- Add 13 unit tests

Fixes #123
"

# 推送到远程
git push origin feat/your-feature-name
```

### 5. 创建Pull Request

```bash
# 在GitHub上创建PR
# 或使用gh CLI
gh pr create --title "Fix: GC memory allocation" \
             --body "## 变更摘要
- 实现实际的内存分配
- 添加GC触发机制
- 新增13个单元测试

## 测试
所有测试通过：✅

## 相关Issue
Closes #123"
```

---

## 🧪 测试指南

### 单元测试

```rust
// 在src文件中添加测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        let result = something();
        assert_eq!(result, expected);
    }
}
```

### 运行测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定测试
cargo test test_gc_allocation

# 运行并显示输出
cargo test -- --nocapture

# 运行特定crate的测试
cargo test -p vm-core

# 运行忽略的测试
cargo test -- --ignored
```

### 基准测试

```bash
# 运行所有基准
cargo bench --workspace

# 运行特定基准
cargo bench -p vm-mem memory_allocation

# 运行并保存结果
cargo bench -- --save-baseline main
cargo bench -- --baseline main
```

---

## 🐛 调试技巧

### 1. 启用调试日志

```bash
# 设置日志级别
RUST_LOG=debug cargo run

# 或在代码中
use tracing::{info, debug, error};

#[tracing::instrument]
fn my_function() {
    info!("Starting function");
    debug!("Debug info: {:?}", data);
}
```

### 2. 使用GDB/LLDB

```bash
# 编译调试版本
cargo build

# 使用GDB (Linux)
gdb target/debug/my_vm

# 使用LLDB (macOS)
lldb target/debug/my_vm

# 常用命令
(gdb) break main
(gdb) run
(gdb) bt      # 查看调用栈
(gdb) print var
```

### 3. Valgrind检查内存

```bash
# Linux上使用Valgrind
valgrind --leak-check=full \
         --show-leak-kinds=all \
         --track-origins=yes \
         target/debug/my_vm
```

---

## 📊 性能分析

### 1. CPU性能

```bash
# 使用flamegraph
cargo install flamegraph
cargo flamegraph --bin my_vm

# 使用perf (Linux)
perf record -g ./target/release/my_vm
perf report
```

### 2. 内存分析

```bash
# 使用heaptrack
heaptrack ./target/release/my_vm

# 或使用dhat
cargo install dhat
cargo test --features dhat
```

---

## 🔧 常用命令

### 构建相关

```bash
# 开发构建（快速）
cargo build

# 发布构建（优化）
cargo build --release

# 检查但不构建
cargo check

# 仅构建依赖
cargo build --deps-only
```

### 测试相关

```bash
# 快速测试（不编译doc）
cargo test --no-fail-fast

# 测试并显示详细信息
cargo test -- --show-output

# 并行运行测试
cargo test -- --test-threads=4
```

### 文档相关

```bash
# 生成文档
cargo doc --no-deps

# 打开文档
cargo doc --open

# 包含私有项
cargo doc --document-private-items
```

### 清理相关

```bash
# 清理构建产物
cargo clean

# 清理特定crate
cargo clean -p vm-core

# 清理但保留依赖
cargo clean --release
```

---

## 🎨 代码风格

### 格式化

```bash
# 格式化所有代码
cargo fmt

# 检查格式
cargo fmt -- --check

# 格式化特定文件
cargo fmt -- vm-core/src/gc/unified.rs
```

### Linting

```bash
# 运行clippy
cargo clippy

# 修复自动可修复的问题
cargo clippy --fix

# 检查特定警告
cargo clippy -- -W clippy::all
```

---

## 📚 重要文档

### 必读文档

1. **`docs/TODO_AUDIT.md`** - 了解所有待办事项
2. **`docs/MIGRATION_GUIDE.md`** - 理解项目变更
3. **`docs/MODERNIZATION_SUMMARY.md`** - 了解技术细节

### 参考文档

- **`README.md`** - 项目概述
- **`ARCHITECTURE.md`** - 架构设计
- **`CONTRIBUTING.md`** - 贡献指南

---

## 🤝 贡献指南

### 贡献类型

1. **Bug修复** - 修复已知问题
2. **新功能** - 实现P1/P2任务
3. **文档改进** - 完善文档
4. **测试补充** - 增加测试覆盖
5. **性能优化** - 提升性能

### 贡献流程

```bash
# 1. Fork项目
# 2. 克隆你的fork
git clone https://github.com/YOUR_USERNAME/vm.git

# 3. 创建分支
git checkout -b feat/your-feature

# 4. 开发和测试
cargo test

# 5. 提交和推送
git commit -m "feat: add feature X"
git push origin feat/your-feature

# 6. 创建Pull Request
# 在GitHub上创建PR
```

### PR模板

```markdown
## 变更类型
- [ ] Bug修复
- [ ] 新功能
- [ ] 破坏性变更
- [ ] 文档更新

## 变更描述
简要描述你的变更

## 测试
- [ ] 单元测试通过
- [ ] 集成测试通过
- [ ] 手动测试完成

## 相关Issue
Closes #123

## 检查清单
- [ ] 代码遵循项目规范
- [ ] 已添加测试
- [ ] 文档已更新
- [ ] 无编译警告
```

---

## 🆘 获取帮助

### 文档资源

```bash
# 查看项目文档
cargo doc --open

# 查看特定crate文档
cargo doc -p vm-core --open
```

### 社区资源

- **GitHub Issues**: 报告问题和讨论
- **GitHub Discussions**: 技术讨论
- **Wiki**: 知识库和FAQ

### 常见问题

**Q: 编译错误怎么办？**
A: 检查Rust版本 >= 1.85，运行`cargo clean && cargo build`

**Q: 测试失败怎么办？**
A: 查看错误信息，确保环境正确，提交Issue

**Q: 如何选择要做的任务？**
A: 查看`docs/TODO_AUDIT.md`，从P1任务开始

**Q: 代码风格要求？**
A: 运行`cargo fmt`和`cargo clippy`

---

## 🚀 下一步

### 学习路径

1. **第1周**: 阅读核心代码，理解架构
2. **第2周**: 修复简单bug，熟悉流程
3. **第3-4周**: 实现P2任务（增强功能）
4. **第5-8周**: 实现P1任务（核心功能）

### 推荐起点

```bash
# 1. 从文档开始
cat docs/TODO_AUDIT.md

# 2. 选择一个P2任务（风险低）
# 例如：改进错误消息

# 3. 创建Issue
# 描述你要做的任务

# 4. 开发
git checkout -b feat/improve-error-messages

# 5. 提交PR
# 获得代码审查
```

---

**开始贡献吧！项目欢迎你的参与！** 🎉

---

**文档版本**: 1.0.0
**最后更新**: 2025-12-31
**维护者**: VM Development Team
