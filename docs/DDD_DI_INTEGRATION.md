# DDD 依赖注入集成文档

**创建日期**: 2024年现代化升级计划
**状态**: ✅ 服务容器已创建，待集成到 vm-service 初始化

## 概述

本文档描述了如何将 DDD 架构迁移后的领域服务通过依赖注入集成到 `vm-service` 中。

## 服务容器

### 位置

`vm-service/src/di_setup.rs`

### 功能

`ServiceContainer` 结构体管理所有基础设施层实现的创建和配置：

- **TLB 管理器**: `MultiLevelTlbManager`
- **缓存管理器**: `GenericCacheManager` (L1, L2, L3)
- **优化策略**: `OptimizationStrategyImpl`
- **寄存器分配器**: `RegisterAllocatorAdapter`
- **事件总线**: `DomainEventBus`

### 使用示例

```rust
use vm_service::di_setup::ServiceContainer;

// 创建服务容器
let container = ServiceContainer::new();

// 创建领域服务
let tlb_service = container.create_tlb_management_service();
let cache_service = container.create_cache_management_service();
let optimization_service = container.create_optimization_pipeline_service();
let register_service = container.create_register_allocation_service();
```

## 集成步骤

### 1. 在 vm-service 初始化中使用 ServiceContainer

在 `vm-service/src/lib.rs` 的 `VmService::new` 方法中：

```rust
use vm_service::di_setup::ServiceContainer;

impl VmService {
    pub async fn new(config: VmConfig, gpu_backend: Option<String>) -> Result<Self, VmError> {
        // ... 现有初始化代码 ...

        // 创建服务容器
        let service_container = ServiceContainer::new();

        // 创建领域服务（可选，根据需要使用）
        let _tlb_service = service_container.create_tlb_management_service();
        let _cache_service = service_container.create_cache_management_service();

        // ... 继续初始化 ...
    }
}
```

### 2. 将服务容器添加到 VmService 结构体

```rust
pub struct VmService {
    // ... 现有字段 ...

    /// 服务容器（管理所有领域服务）
    service_container: ServiceContainer,
}
```

### 3. 在需要时使用领域服务

```rust
impl VmService {
    pub async fn optimize_code(&mut self, ir: &[u8]) -> VmResult<Vec<u8>> {
        let optimization_service = self.service_container.create_optimization_pipeline_service();
        let config = OptimizationPipelineConfig::new(
            GuestArch::X86_64,
            GuestArch::ARM64,
            2,
        );
        optimization_service.execute_pipeline(&config, ir).await
    }
}
```

## 架构优势

1. **依赖倒置**: 领域服务通过 trait 接口依赖基础设施层实现
2. **可测试性**: 可以轻松注入 mock 实现进行测试
3. **可扩展性**: 可以轻松替换基础设施层实现
4. **单一职责**: 服务容器专注于服务创建和配置

## 集成状态

✅ **已完成**:
- ✅ 创建了 `ServiceContainer` 结构体
- ✅ 在 `VmService` 结构体中添加了 `service_container` 字段
- ✅ 在 `VmService::new` 中初始化服务容器
- ✅ 导出了 `ServiceContainer` 供外部使用

⚠️ **待完成**:
- [ ] 在需要时使用领域服务（例如在优化、缓存管理等场景）
- [ ] 更新测试以使用新的领域服务接口
- [ ] 添加性能监控和日志记录

---

**文档维护者**: VM 项目团队
**最后审查**: 2024年现代化升级计划
