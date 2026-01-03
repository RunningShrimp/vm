# 文档索引

本文档提供了项目文档的完整索引和导航。

**最后更新**: 2024年现代化升级计划

## 📚 核心文档

### 入门指南
- [README.md](./README.md) - 项目主文档
- [QUICK_START.md](./QUICK_START.md) - 快速开始指南
- [DEVELOPER_SETUP.md](./DEVELOPER_SETUP.md) - 开发环境设置（包含环境问题解决方案）
- [MODERNIZATION_AND_MIGRATION_GUIDE.md](./MODERNIZATION_AND_MIGRATION_GUIDE.md) - 现代化与迁移指南

### 架构文档
- [architecture.md](./architecture.md) - 架构概览
- [DDD_ARCHITECTURE_CLARIFICATION.md](./DDD_ARCHITECTURE_CLARIFICATION.md) - DDD 架构说明
- [DDD_MIGRATION_FINAL_SUMMARY.md](./DDD_MIGRATION_FINAL_SUMMARY.md) - DDD 迁移最终总结
- [DDD_DI_INTEGRATION.md](./DDD_DI_INTEGRATION.md) - 依赖注入集成指南
- [architecture/SYSTEM_OVERVIEW.md](./architecture/SYSTEM_OVERVIEW.md) - 系统概览
- [architecture/CORE_COMPONENTS.md](./architecture/CORE_COMPONENTS.md) - 核心组件
- [architecture/X86_CODEGEN.md](./architecture/X86_CODEGEN.md) - x86代码生成器（整合实施指南和进度）

### API 文档
- [API_INDEX.md](./API_INDEX.md) - API 索引
- [api_guide.md](./api_guide.md) - API 使用指南
- [api/](./api/) - 详细 API 文档目录

### 开发指南
- [development/CONTRIBUTING.md](./development/CONTRIBUTING.md) - 贡献指南
- [development/CODE_STYLE.md](./development/CODE_STYLE.md) - 代码风格
- [development/TESTING_STRATEGY.md](./development/TESTING_STRATEGY.md) - 测试策略
- [AUTHOR_GUIDE.md](./AUTHOR_GUIDE.md) - 作者指南
- [CODE_REVIEW_GUIDE.md](./CODE_REVIEW_GUIDE.md) - 代码审查指南

### 功能文档
- [FEATURE_CONTRACT.md](./FEATURE_CONTRACT.md) - Feature 标志说明
- [GPU_NPU_PASSTHROUGH.md](./GPU_NPU_PASSTHROUGH.md) - GPU/NPU 直通功能
- [BENCHMARKING.md](./BENCHMARKING.md) - 性能基准测试
- [PERFORMANCE_MONITORING.md](./PERFORMANCE_MONITORING.md) - 性能监控
- [IMPLEMENTATION_SUMMARIES.md](./IMPLEMENTATION_SUMMARIES.md) - 实现总结（Cranelift、SIMD、块链接、厂商优化）

### CI/CD 和发布
- [CI_CD_GUIDE.md](./CI_CD_GUIDE.md) - CI/CD 指南
- [CONTRIBUTOR_CI_CD_HANDBOOK.md](./CONTRIBUTOR_CI_CD_HANDBOOK.md) - 贡献者 CI/CD 手册
- [RELEASE_PROCESS.md](./RELEASE_PROCESS.md) - 发布流程
- [RELEASE_QUICKSTART.md](./RELEASE_QUICKSTART.md) - 快速发布指南

### 教程
- [tutorials/GETTING_STARTED.md](./tutorials/GETTING_STARTED.md) - 入门教程
- [tutorials/ADVANCED_USAGE.md](./tutorials/ADVANCED_USAGE.md) - 高级用法
- [tutorials/RISCV_PROGRAMMING.md](./tutorials/RISCV_PROGRAMMING.md) - RISC-V 编程

## 📁 文档目录结构

```
docs/
├── README.md                          # 项目主文档
├── QUICK_START.md                     # 快速开始
├── architecture/                      # 架构文档
│   ├── SYSTEM_OVERVIEW.md
│   ├── CORE_COMPONENTS.md
│   └── adr/                          # 架构决策记录
├── api/                              # API 文档
├── development/                      # 开发指南
├── tutorials/                        # 教程
└── reports/archive/intermediate_docs/  # 中间文档归档
```

## 🔍 按主题查找

### DDD 架构
- [DDD_ARCHITECTURE_CLARIFICATION.md](./DDD_ARCHITECTURE_CLARIFICATION.md)
- [DDD_MIGRATION_FINAL_SUMMARY.md](./DDD_MIGRATION_FINAL_SUMMARY.md)
- [DDD_DI_INTEGRATION.md](./DDD_DI_INTEGRATION.md)

### 性能优化
- [BENCHMARKING.md](./BENCHMARKING.md)
- [PERFORMANCE_MONITORING.md](./PERFORMANCE_MONITORING.md)
- [rust_2024_audit_report.md](./rust_2024_audit_report.md)

### 功能实现
- [FEATURE_CONTRACT.md](./FEATURE_CONTRACT.md)
- [GPU_NPU_PASSTHROUGH.md](./GPU_NPU_PASSTHROUGH.md)
- [MODERNIZATION_AND_MIGRATION_GUIDE.md](./MODERNIZATION_AND_MIGRATION_GUIDE.md)

## 📝 文档维护

- 文档应保持最新
- 过时的文档应移至 `docs/reports/archive/`
- 重复的文档应合并或删除
- 中间工作文档（分析报告、规划文档）已归档至 `reports/archive/intermediate_docs/`

---

**文档维护者**: VM 项目团队
**最后审查**: 2024年现代化升级计划
