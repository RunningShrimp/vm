# Runtime

运行时服务，包括服务编排、插件系统和监控。

## 包含模块

- **vm-service** - VM 服务编排
  - VM 生命周期管理
  - 服务依赖管理
  - 配置管理

- **vm-plugin** - 插件系统
  - 插件加载和卸载
  - 沙箱执行环境
  - 插件 API

- **vm-monitor** - 监控和指标
  - 性能指标收集
  - 实时监控
  - 告警和日志

## 服务架构

```
vm-service (服务编排)
    ↓
├── vm-plugin (扩展能力)
├── vm-monitor (监控)
└── 其他核心组件
```

## 快速开始

查看各模块的 README 了解详细文档：
- [vm-service/README.md](./vm-service/README.md)
- [vm-plugin/README.md](./vm-plugin/README.md)
- [vm-monitor/README.md](./vm-monitor/README.md)
