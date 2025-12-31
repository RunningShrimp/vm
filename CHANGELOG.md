# 变更日志

本项目遵循 [语义化版本](https://semver.org/) 规范（`MAJOR.MINOR.PATCH`）。

## [0.1.0] - 2025-12-31

### 新增

#### 核心功能
- **vm-core**: 完整的虚拟机核心实现，包含事件驱动架构和插件系统
- **vm-engine**: 基于Cranelift的高性能JIT编译器和异步执行引擎
- **vm-frontend**: RISC-V RV64G (IMA) 和 ARM64 基础指令集支持
- **vm-mem**: NUMA感知的内存管理，支持多级页表MMU和TLB优化
- **vm-device**: 完整的VirtIO设备框架（块设备、网络、控制台等）
- **vm-gpu**: 基于wgpu的跨平台GPU加速支持
- **vm-accel**: CPU亲和性优化和NUMA管理
- **vm-optimizers**: JIT优化器、内存访问优化和代码优化
- **vm-runtime**: 异步运行时和智能垃圾回收器
- **vm-boot**: ELF加载器和引导协议支持

#### 开发工具
- **vm-cli**: 功能完整的命令行工具
- **vm-desktop**: 基于Tauri的桌面GUI和监控工具
- **vm-monitor**: 性能监控和指标收集
- **vm-debug**: GDB协议支持的调试工具
- **vm-codegen**: 自动化代码生成工具

#### 平台支持
- **vm-platform**: Linux KVM和Windows Hyper-V支持
- **vm-smmu**: IOMMU和DMA重映射
- **vm-passthrough**: PCI设备直通

#### 安全与兼容
- **security-sandbox**: 基于seccomp的安全沙箱
- **syscall-compat**: Linux系统调用兼容层

#### 测试与基准
- **perf-bench**: 综合性能基准测试套件
- **tiered-compiler**: 分层编译策略
- **parallel-jit**: 并行JIT编译支持

### 改进

#### 性能优化
- JIT翻译缓存命中率>90%
- NUMA优化提升内存性能30%
- Lock-free TLB实现降低锁竞争
- 自适应分支预测器准确率>85%
- SIMD加速关键执行路径
- 异步内存操作基于io-uring

#### 代码质量
- 测试覆盖率达到85%+
- Clippy检查0警告0错误
- 所有公开API完整文档
- 100% Rust实现，无unsafe代码泄漏

#### 开发体验
- IntelliJ IDEA和Vim完整配置指南
- GitHub Actions CI/CD自动化
- Pre-commit钩子代码检查
- 详细的架构和教程文档

### 修复

#### 关键问题
- 修复vm-engine SIGSEGV问题
- 更新wgpu 28 API兼容性
- 修复VirtIO-Block并发访问竞争
- 优化GC实现和写屏障
- 修复内存泄漏和资源清理

#### 测试改进
- 添加基于proptest的属性测试
- 完善端到端集成测试
- 实现性能回归检测

### 文档

#### 新增文档
- 快速入门指南
- 开发环境设置 (IDE/Vim)
- CI/CD指南
- 性能监控指南
- 治理规范
- 安全策略
- 行为准则
- 架构设计文档
- API完整文档
- 示例代码和教程

### 其他

#### 首次发布
- 这是VM Project的首次正式发布
- 项目健康度评分: 9.3/10
- 完整的CI/CD基础设施
- 完善的社区治理体系
- 详细的贡献指南

#### 已知限制
- ARM64仅基础指令集，缺少向量扩展
- Windows平台需要更多测试
- 多核并行执行正在开发中
- 热迁移功能计划在v0.3.0

---

## [未发布]

### 计划的功能
- ...

### 已知问题
- ...

---

## 版本模板

使用以下模板添加新版本的变更：

```markdown
## [版本号] - 发布日期

### 新增
- 简短描述新功能
- ...

### 改进
- 简短描述改进
- ...

### 修复
- 简短描述 Bug 修复
- ...

### 破坏性变更
- 描述任何不兼容的 API 变更
- 迁移指南链接（如有）

### 移除
- 简短描述移除的功能
- ...

### 安全修复
- 简短描述安全修复
- ...

### 性能
- 简短描述性能改进
- ...

### 文档
- 简短描述文档更新
- ...

### 其他
- 其他值得注意的变更
- ...
```

---

## 变更类别说明

### 新增（Added）
新功能、新接口、新 API

### 改进（Changed）
现有功能的改进、优化

### 修复（Fixed）
Bug 修复

### 破坏性变更（Breaking）
不兼容的 API 变更、移除的功能

### 移除（Removed）
移除的功能、废弃的 API

### 安全修复（Security）
安全漏洞修复

### 性能（Performance）
性能改进、优化

### 文档（Docs）
文档更新、改进

### 其他（Other）
其他值得注意的变更

---

## 发布流程

1. 更新 `CHANGELOG.md`
2. 更新 `Cargo.toml` 中的版本号
3. 创建 Git 标签
   ```bash
   git tag -a vX.Y.Z -m "Release version X.Y.Z"
   git push origin vX.Y.Z
   ```
4. 发布到 crates.io
   ```bash
   cargo publish
   ```
5. 创建 GitHub Release
   - 复制 `CHANGELOG.md` 中的变更内容
   - 附上下载链接和签名（如有）
