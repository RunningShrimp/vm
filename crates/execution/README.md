# Execution

执行引擎，提供指令解码、解释执行和 JIT 编译。

## 包含模块

- **vm-frontend** - 前端解码器
  - x86_64 指令解码
  - ARM64 指令解码
  - RISC-V 指令解码
  - 指令提升到 IR

- **vm-engine** - 执行引擎
  - 解释器执行
  - 基础 JIT 编译器
  - 执行上下文管理

- **vm-engine-jit** - 高级 JIT 实现
  - Cranelift 后端
  - 分层 JIT 编译
  - 性能优化

## 执行流程

```
Guest 指令
    ↓
vm-frontend (解码)
    ↓
vm-ir (中间表示)
    ↓
vm-engine / vm-engine-jit (执行)
```

## 快速开始

查看各模块的 README 了解详细文档：
- [vm-frontend/README.md](./vm-frontend/README.md)
- [vm-engine/README.md](./vm-engine/README.md)
- [vm-engine-jit/README.md](./vm-engine-jit/README.md)
