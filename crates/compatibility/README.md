# Compatibility

兼容性层，提供安全沙箱和系统调用兼容性。

## 包含模块

- **security-sandbox** - 安全沙箱
  - seccomp 过滤器
  - 命名空间隔离
  - 资源限制

- **syscall-compat** - 系统调用兼容性
  - 系统调用转换
  - 跨平台适配
  - 语义保持

## 安全架构

```
应用程序
    ↓
Syscall
    ↓
security-sandbox (沙箱)
    ↓
syscall-compat (兼容转换)
    ↓
主机系统
```

## 快速开始

查看各模块的 README 了解详细文档：
- [security-sandbox/README.md](./security-sandbox/README.md)
- [syscall-compat/README.md](./syscall-compat/README.md)
