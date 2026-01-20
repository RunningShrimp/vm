# Tools

用户工具和应用程序，提供命令行接口、GUI 界面和调试工具。

## 包含工具

### 🖥️ CLI (命令行接口)
**位置**: [cli/](./cli/)
**命令**: `vm-cli`

主要功能：
- VM 创建和管理
- 快速启动常用系统
- 快照管理
- 日志查看和调试

快速开始：
```bash
# 安装 Debian
vm-cli install-debian

# 启动 VM
vm-cli start my-vm

# 列出所有 VM
vm-cli list
```

详细文档：[cli/README.md](./cli/README.md)

---

### 🖼️ Desktop (桌面 GUI)
**位置**: [desktop/](./desktop/)
**类型**: Tauri 应用

主要功能：
- 图形化 VM 管理
- 性能监控
- 虚拟机配置
- 设备管理

快速开始：
```bash
cd tools/desktop
cargo tauri dev
```

---

### 🔍 Debug (调试工具)
**位置**: [debug/](./debug/)
**命令**: `vm-debug`

主要功能：
- 断点调试
- 内存检查
- 寄存器查看
- 单步执行

详细文档：[debug/README.md](./debug/README.md)

---

### 🔌 Passthrough (设备直通)
**位置**: [passthrough/](./passthrough/)
**命令**: `vm-passthrough`

主要功能：
- PCI 设备直通
- GPU 直通配置
- 设备绑定管理

详细文档：[passthrough/README.md](./passthrough/README.md)

## 工具依赖关系

```
cli/ ──┐
         ├──→ crates/* (所有核心库)
debug/ ─┤
         │
desktop/├──→ crates/* (所有核心库)
         │
passthrough/
```

## 构建所有工具

```bash
# 构建所有工具
cargo build --release -p vm-cli -p vm-debug -p vm-passthrough

# 构建 desktop (需要单独构建)
cd tools/desktop && cargo tauri build
```

## 快速导航

- **CLI**: [cli/](./cli/) - 命令行接口
- **Desktop**: [desktop/](./desktop/) - 桌面应用
- **Debug**: [debug/](./debug/) - 调试工具
- **Passthrough**: [passthrough/](./passthrough/) - 设备直通
