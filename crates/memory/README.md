# Memory

内存管理、垃圾收集和性能优化。

## 包含模块

- **vm-mem** - 内存管理
  - MMU (内存管理单元)
  - 地址空间管理
  - TLB (转换后备缓冲)
  - NUMA 优化
  - DMA 支持

- **vm-gc** - 垃圾收集
  - 增量 GC
  - 分代 GC
  - 并行 GC

- **vm-optimizers** - 性能优化器
  - 翻译缓存优化
  - 批处理优化
  - 自适应优化

## 内存架构

```
虚拟地址 → MMU → 物理地址 → 主机内存
             ↓
            TLB (缓存)
             ↓
           DMA (设备访问)
```

## 快速开始

查看各模块的 README 了解详细文档：
- [vm-mem/README.md](./vm-mem/README.md)
- [vm-gc/README.md](./vm-gc/README.md)
- [vm-optimizers/README.md](./vm-optimizers/README.md)
