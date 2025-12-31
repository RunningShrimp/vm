# ADR-001: Rust 2024 Edition选择

## 状态
已接受 (2024-12-31)

## 上下文
VM项目需要选择一个编程语言版本，以平衡：
- 最新语言特性
- 编译器稳定性
- 生态系统兼容性
- 长期可维护性

## 决策
采用Rust 2024 Edition，最低Rust版本1.85。

```toml
[workspace.package]
edition = "2024"
rust-version = "1.85"
```

## 理由

### 优势

1. **最新语言特性**:
   - `let-else`绑定简化代码
   - 增强的const generics
   - 稳定的async trait
   - 改进的pattern matching

2. **更好的错误信息**:
   - 编译器诊断改进
   - 更清晰的类型推断错误

3. **生态系统**:
   - crates.io上的新crate普遍采用2024 edition
   - 社区支持活跃

### 劣势及缓解

1. **编译器较新**:
   - 缓解：使用稳定版rustc 1.85+
   - 缓解：CI/CD固定工具链版本

2. **生态系统兼容性**:
   - 缓解：大多数crate已支持2024 edition
   - 缓解：必要时可以per-crate edition

## 替代方案

### Rust 2021 Edition
**优势**:
- 更成熟稳定
- 兼容性最好

**劣势**:
- 缺少新特性
- 代码可能不够简洁

### 结论: 2024 edition优势明显，且1.85已稳定。

## 后果

### 短期
- ✅ 可使用let-else简化代码
- ✅ async trait开箱即用
- ⚠️ 需要更新CI/CD配置

### 长期
- ✅ 保持技术栈现代性
- ✅ 降低技术债务
- ✅ 改善开发体验

## 参考
- [Rust Edition Guide](https://doc.rust-lang.org/edition-guide/)
- [Rust 1.85 Release Notes](https://github.com/rust-lang/rust/releases/tag/1.85.0)

---
**创建时间**: 2024-12-31
**作者**: VM开发团队
