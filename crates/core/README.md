# Core

核心 VM 组件，提供领域模型、中间表示和启动服务。

## 包含模块

- **vm-core** - 核心 VM 引擎和领域逻辑
  - 领域模型（aggregates, entities）
  - 事件存储和领域事件
  - 核心抽象和 trait 定义

- **vm-ir** - 中间表示
  - 指令级中间表示
  - IR 优化和转换
  - 代码生成接口

- **vm-boot** - 启动和运行时服务
  - VM 启动流程
  - 运行时服务管理
  - 快照和恢复

## 依赖关系

```
vm-core (领域核心)
    ↓
vm-ir (中间表示) ← vm-boot (启动流程)
```

## 快速开始

查看各模块的 README 了解详细文档：
- [vm-core/README.md](./vm-core/README.md)
- [vm-ir/README.md](./vm-ir/README.md)
- [vm-boot/README.md](./vm-boot/README.md)
