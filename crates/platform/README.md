# Platform

平台抽象层，提供硬件加速和操作系统抽象。

## 包含模块

- **vm-accel** - 硬件加速
  - KVM (Kernel-based Virtual Machine) - Linux
  - HVF (Hypervisor Framework) - macOS
  - WHP (Windows Hypervisor Platform) - Windows

- **vm-platform** - 平台特定代码
  - CPU 特性检测
  - 平台初始化
  - 系统调用接口

- **vm-osal** - 操作系统抽象层
  - 跨平台抽象
  - 系统资源管理
  - 线程和同步

## 平台支持矩阵

| 平台 | 加速器 | 状态 |
|------|--------|------|
| Linux x86_64 | KVM | ✅ |
| Linux ARM64 | KVM | ✅ |
| macOS ARM64 | HVF | ✅ |
| Windows x86_64 | WHP | ✅ |

## 快速开始

查看各模块的 README 了解详细文档：
- [vm-accel/README.md](./vm-accel/README.md)
- [vm-platform/README.md](./vm-platform/README.md)
- [vm-osal/README.md](./vm-osal/README.md)
