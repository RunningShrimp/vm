# Architecture

架构支持，包括跨架构支持、代码生成和构建依赖。

## 包含模块

- **vm-cross-arch-support** - 跨架构支持
  - 二进制翻译
  - 指令模式匹配
  - 优化缓存
  - 翻译后端

- **vm-codegen** - 代码生成工具
  - 前端代码生成
  - 架构定义解析
  - 构建脚本

- **vm-build-deps** - 构建依赖
  - 共享依赖管理
  - 特性统一管理

## 跨架构翻译

```
源架构指令
    ↓
模式匹配 (vm-cross-arch-support)
    ↓
目标架构指令
    ↓
执行
```

## 支持的架构

| 源架构 | 目标架构 | 状态 |
|---------|---------|------|
| x86_64 | ARM64 | ✅ |
| x86_64 | RISC-V | 🚧 |
| ARM64 | x86_64 | 🚧 |

## 快速开始

查看各模块的 README 了解详细文档：
- [vm-cross-arch-support/README.md](./vm-cross-arch-support/README.md)
- [vm-codegen/README.md](./vm-codegen/README.md)
- [vm-build-deps/README.md](./vm-build-deps/README.md)
