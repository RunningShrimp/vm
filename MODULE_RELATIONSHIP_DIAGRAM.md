# 降低LLVM依赖的模块关系图

## 系统架构概览

```mermaid
graph TB
    subgraph "核心接口层"
        CompilerBackend[CompilerBackend<br/>统一编译器接口]
        CompilerFactory[CompilerFactory<br/>编译器工厂]
        BackendRegistry[BackendRegistry<br/>后端注册表]
        CompilerManager[CompilerManager<br/>编译器管理器]
    end
    
    subgraph "编译器后端"
        CraneliftBackend[CraneliftBackend<br/>增强Cranelift后端]
        OptimizedInterpreter[OptimizedInterpreter<br/>优化解释器后端]
        LightweightJIT[LightweightJIT<br/>轻量级JIT后端]
        LLVMBackend[LLVMBackend<br/>LLVM后端]
    end
    
    subgraph "分层编译系统"
        TieredCompilationManager[TieredCompilationManager<br/>分层编译管理器]
        HotspotDetector[HotspotDetector<br/>热点检测器]
        CompilationTier[CompilationTier<br/>编译层]
    end
    
    subgraph "缓存系统"
        UnifiedCodeCache[UnifiedCodeCache<br/>统一代码缓存]
        LruPolicy[LruPolicy<br/>LRU策略]
        CacheStats[CacheStats<br/>缓存统计]
    end
    
    subgraph "现有模块重构"
        VmIrLift[vm-ir-lift<br/>指令语义库]
        VmEngineJit[vm-engine-jit<br/>JIT编译引擎]
        AotBuilder[aot-builder<br/>AOT编译器]
        VmCrossArch[vm-cross-arch<br/>跨架构优化]
    end
    
    subgraph "支持系统"
        FeatureMatrix[FeatureMatrix<br/>特性矩阵]
        CompatibilityManager[CompatibilityManager<br/>兼容性管理]
        RiskManager[RiskManager<br/>风险管理]
        TestFramework[TestFramework<br/>测试框架]
    end

    %% 核心接口关系
    CompilerBackend --> CompilerFactory
    CompilerFactory --> BackendRegistry
    BackendRegistry --> CompilerManager
    
    %% 后端实现关系
    CompilerManager --> CraneliftBackend
    CompilerManager --> OptimizedInterpreter
    CompilerManager --> LightweightJIT
    CompilerManager --> LLVMBackend
    
    %% 分层编译关系
    CompilerManager --> TieredCompilationManager
    TieredCompilationManager --> HotspotDetector
    TieredCompilationManager --> CompilationTier
    
    %% 缓存系统关系
    CompilerManager --> UnifiedCodeCache
    UnifiedCodeCache --> LruPolicy
    UnifiedCodeCache --> CacheStats
    
    %% 现有模块重构关系
    CompilerManager --> VmIrLift
    CompilerManager --> VmEngineJit
    CompilerManager --> AotBuilder
    CompilerManager --> VmCrossArch
    
    %% 支持系统关系
    CompilerManager --> FeatureMatrix
    CompilerManager --> CompatibilityManager
    CompilerManager --> RiskManager
    CompilerManager --> TestFramework
```

## 核心接口层详细关系

```mermaid
graph LR
    subgraph "编译器接口"
        CompilerBackend[CompilerBackend<br/>compile方法]
        CompilerFeatures[CompilerFeatures<br/>supported_features方法]
        CompileOptions[CompileOptions<br/>编译选项]
        CompiledCode[CompiledCode<br/>编译结果]
        CompilerStats[CompilerStats<br/>编译统计]
    end
    
    subgraph "工厂接口"
        CompilerFactory[CompilerFactory<br/>create_compiler方法]
        CompilerConfig[CompilerConfig<br/>编译器配置]
        ResourceLimits[ResourceLimits<br/>资源限制]
    end
    
    %% 接口实现关系
    CompilerBackend --|实现| CompilerFeatures
    CompilerBackend --|使用| CompileOptions
    CompilerBackend --|返回| CompiledCode
    CompilerBackend --|提供| CompilerStats
    
    CompilerFactory --|使用| CompilerConfig
    CompilerFactory --|创建| CompilerBackend
    CompilerFactory --|检查| ResourceLimits
```

## 后端架构详细关系

```mermaid
graph TB
    subgraph "Cranelift后端"
        CraneliftOptimizer[CraneliftOptimizer<br/>优化器]
        SimdSupport[SimdSupport<br/>SIMD支持]
        Vectorizer[Vectorizer<br/>向量化器]
        InstructionSpecializer[InstructionSpecializer<br/>指令特化器]
    end
    
    subgraph "解释器后端"
        OptimizedExecutor[OptimizedExecutor<br/>优化执行器]
        InstructionFuser[InstructionFuser<br/>指令融合器]
        DispatchTable[DispatchTable<br/>调度表]
        BlockCache[BlockCache<br/>块缓存]
    end
    
    subgraph "轻量级JIT后端"
        CodeGenerator[CodeGenerator<br/>代码生成器]
        MemoryManager[CodeMemoryManager<br/>内存管理器]
        SimpleRegisterAllocator[SimpleRegisterAllocator<br/>简单寄存器分配器]
    end
    
    %% Cranelift后端关系
    CraneliftBackend --> CraneliftOptimizer
    CraneliftBackend --> SimdSupport
    CraneliftBackend --> Vectorizer
    CraneliftBackend --> InstructionSpecializer
    
    %% 解释器后端关系
    OptimizedInterpreter --> OptimizedExecutor
    OptimizedInterpreter --> InstructionFuser
    OptimizedInterpreter --> DispatchTable
    OptimizedInterpreter --> BlockCache
    
    %% 轻量级JIT后端关系
    LightweightJIT --> CodeGenerator
    LightweightJIT --> MemoryManager
    LightweightJIT --> SimpleRegisterAllocator
```

## 分层编译系统详细关系

```mermaid
graph TB
    subgraph "分层决策"
        ExecutionCounter[ExecutionCounter<br/>执行计数器]
        HotspotStats[HotspotStats<br/>热点统计]
        TieredCompilationStrategy[TieredCompilationStrategy<br/>分层策略]
        HotspotAlgorithm[HotspotAlgorithm<br/>热点算法]
    end
    
    subgraph "编译层"
        InterpreterTier[InterpreterTier<br/>解释器层]
        FastJITTier[FastJITTier<br/>快速JIT层]
        OptimizedJITTier[OptimizedJITTier<br/>优化JIT层]
        AOTTier[AOTTier<br/>AOT层]
    end
    
    %% 分层决策关系
    HotspotDetector --> ExecutionCounter
    HotspotDetector --> HotspotStats
    HotspotDetector --> HotspotAlgorithm
    TieredCompilationManager --> TieredCompilationStrategy
    TieredCompilationStrategy --> InterpreterTier
    TieredCompilationStrategy --> FastJITTier
    TieredCompilationStrategy --> OptimizedJITTier
    TieredCompilationStrategy --> AOTTier
```

## 缓存系统详细关系

```mermaid
graph LR
    subgraph "缓存结构"
        CacheKey[CacheKey<br/>缓存键]
        CachedCode[CachedCode<br/>缓存代码]
        CacheConfig[CacheConfig<br/>缓存配置]
        LruPolicy[LruPolicy<br/>LRU策略]
    end
    
    subgraph "缓存操作"
        GetOperation[GetOperation<br/>获取操作]
        PutOperation[PutOperation<br/>存储操作]
        EvictOperation[EvictOperation<br/>驱逐操作]
        ShareOperation[ShareOperation<br/>共享操作]
    end
    
    %% 缓存结构关系
    UnifiedCodeCache --> CacheKey
    UnifiedCodeCache --> CachedCode
    UnifiedCodeCache --> CacheConfig
    UnifiedCodeCache --> LruPolicy
    
    %% 缓存操作关系
    UnifiedCodeCache --> GetOperation
    UnifiedCodeCache --> PutOperation
    UnifiedCodeCache --> EvictOperation
    UnifiedCodeCache --> ShareOperation
```

## 现有模块重构关系

```mermaid
graph TB
    subgraph "vm-ir-lift重构"
        MultiBackendLifter[MultiBackendLifter<br/>多后端提升器]
        LifterBackend[LifterBackend<br/>提升器接口]
        LiftingContext[LiftingContext<br/>提升上下文]
    end
    
    subgraph "vm-engine-jit重构"
        RefactoredJitEngine[RefactoredJitEngine<br/>重构JIT引擎]
        BackendManager[BackendManager<br/>后端管理器]
        JitEngineConfig[JitEngineConfig<br/>JIT引擎配置]
    end
    
    subgraph "aot-builder重构"
        UniversalAotBuilder[UniversalAotBuilder<br/>通用AOT构建器]
        ConfigManager[ConfigManager<br/>配置管理器]
        PlatformInfo[PlatformInfo<br/>平台信息]
    end
    
    %% vm-ir-lift重构关系
    MultiBackendLifter --> LifterBackend
    MultiBackendLifter --> LiftingContext
    
    %% vm-engine-jit重构关系
    RefactoredJitEngine --> BackendManager
    RefactoredJitEngine --> JitEngineConfig
    
    %% aot-builder重构关系
    UniversalAotBuilder --> ConfigManager
    UniversalAotBuilder --> PlatformInfo
```

## 支持系统详细关系

```mermaid
graph TB
    subgraph "特性管理"
        BackendCapabilities[BackendCapabilities<br/>后端能力]
        SelectionCriteria[SelectionCriteria<br/>选择标准]
        FeatureDependencies[FeatureDependencies<br/>特性依赖]
    end
    
    subgraph "兼容性检查"
        ApiVersion[ApiVersion<br/>API版本]
        DeprecationInfo[DeprecationInfo<br/>弃用信息]
        MigrationGuide[MigrationGuide<br/>迁移指南]
    end
    
    subgraph "风险管理"
        Risk[Risk<br/>风险]
        MitigationStrategy[MitigationStrategy<br/>缓解策略]
        MonitoringPlan[MonitoringPlan<br/>监控计划]
    end
    
    subgraph "测试框架"
        TestSuite[TestSuite<br/>测试套件]
        PerformanceBenchmark[PerformanceBenchmark<br/>性能基准]
        CompatibilityTest[CompatibilityTest<br/>兼容性测试]
    end
    
    %% 特性管理关系
    FeatureMatrix --> BackendCapabilities
    FeatureMatrix --> SelectionCriteria
    FeatureMatrix --> FeatureDependencies
    
    %% 兼容性检查关系
    CompatibilityManager --> ApiVersion
    CompatibilityManager --> DeprecationInfo
    CompatibilityManager --> MigrationGuide
    
    %% 风险管理关系
    RiskManager --> Risk
    RiskManager --> MitigationStrategy
    RiskManager --> MonitoringPlan
    
    %% 测试框架关系
    TestFramework --> TestSuite
    TestFramework --> PerformanceBenchmark
    TestFramework --> CompatibilityTest
```

## 数据流图

```mermaid
flowchart TD
    subgraph "编译流程"
        A[IR输入] --> B[后端选择]
        B --> C[编译执行]
        C --> D[代码生成]
        D --> E[缓存存储]
        E --> F[执行输出]
    end
    
    subgraph "决策点"
        G[性能监控] --> H{后端切换}
        H --> I[降级策略]
        I --> J[回退处理]
        J --> K[错误恢复]
    end
    
    %% 主流程
    A --> B
    B --> C
    C --> D
    D --> E
    E --> F
    
    %% 决策流程
    G --> H
    H --> I
    I --> J
    J --> K
```

## 组件交互序列图

```mermaid
sequenceDiagram
    participant Client as 客户端
    participant CM as CompilerManager
    participant BR as BackendRegistry
    participant CB as CraneliftBackend
    participant OI as OptimizedInterpreter
    participant LJ as LightweightJIT
    participant UC as UnifiedCodeCache
    
    Client->>CM: 请求编译
    CM->>BR: 查询可用后端
    BR-->>CM: 返回后端列表
    
    alt 选择Cranelift后端
        CM->>CB: 创建Cranelift编译器
        CB->>CM: 返回编译器实例
        CM->>CB: 编译IR块
        CB->>CM: 返回编译结果
    else 选择优化解释器
        CM->>OI: 创建解释器
        OI->>CM: 返回解释器实例
        CM->>OI: 执行IR块
        OI->>CM: 返回执行结果
    else 选择轻量级JIT
        CM->>LJ: 创建轻量级JIT
        LJ->>CM: 返回JIT实例
        CM->>LJ: 编译IR块
        LJ->>CM: 返回编译结果
    end
    
    CM->>UC: 检查缓存
    UC-->>CM: 返回缓存结果
    
    alt 缓存未命中
        CM->>UC: 存储编译结果
        UC-->>CM: 确认存储
    end
    
    CM-->>Client: 返回编译结果
```

## 模块依赖关系

```mermaid
graph TD
    subgraph "核心依赖"
        vm-core[vm-core<br/>核心虚拟机功能]
        vm-ir[vm-ir<br/>中间表示]
    end
    
    subgraph "编译器模块"
        vm-engine-jit[vm-engine-jit<br/>JIT编译引擎]
        vm-engine-interpreter[vm-engine-interpreter<br/>解释器引擎]
    end
    
    subgraph "工具模块"
        vm-ir-lift[vm-ir-lift<br/>指令语义库]
        aot-builder[aot-builder<br/>AOT编译器]
        vm-cross-arch[vm-cross-arch<br/>跨架构优化]
    end
    
    %% 核心依赖关系
    vm-engine-jit --> vm-core
    vm-engine-jit --> vm-ir
    vm-engine-interpreter --> vm-core
    vm-engine-interpreter --> vm-ir
    
    %% 工具模块依赖
    vm-ir-lift --> vm-core
    vm-ir-lift --> vm-ir
    aot-builder --> vm-core
    aot-builder --> vm-ir
    vm-cross-arch --> vm-core
    vm-cross-arch --> vm-ir
    vm-cross-arch --> vm-engine-jit
    vm-cross-arch --> vm-engine-interpreter
```

## 部署架构图

```mermaid
graph TB
    subgraph "编译时配置"
        BuildConfig[BuildConfig<br/>构建配置]
        FeatureFlags[FeatureFlags<br/>特性标志]
        BackendSelection[BackendSelection<br/>后端选择]
    end
    
    subgraph "运行时选择"
        RuntimeConfig[RuntimeConfig<br/>运行时配置]
        EnvironmentVars[EnvironmentVars<br/>环境变量]
        PerformanceMetrics[PerformanceMetrics<br/>性能指标]
    end
    
    subgraph "部署模式"
        AllCranelift[全Cranelift模式<br/>所有模块使用Cranelift]
        HybridMode[混合模式<br/>Cranelift+解释器]
        FallbackMode[回退模式<br/>LLVM作为回退]
    end
    
    %% 编译时配置关系
    BuildConfig --> FeatureFlags
    BuildConfig --> BackendSelection
    
    %% 运行时选择关系
    RuntimeConfig --> EnvironmentVars
    RuntimeConfig --> PerformanceMetrics
    
    %% 部署模式关系
    FeatureFlags --> AllCranelift
    FeatureFlags --> HybridMode
    FeatureFlags --> FallbackMode
```

## 总结

这个模块关系图展示了降低LLVM依赖的完整架构设计，包括：

1. **核心接口层** - 定义了统一的编译器接口和工厂模式
2. **多种后端实现** - Cranelift、优化解释器、轻量级JIT等
3. **分层编译系统** - 根据执行频率和性能需求动态选择后端
4. **缓存系统** - 统一的代码缓存机制，支持跨后端共享
5. **现有模块重构** - 各模块的适配和重构方案
6. **支持系统** - 特性管理、兼容性检查、风险管理等

通过这种模块化设计，可以逐步降低LLVM依赖，同时保持系统的灵活性和性能。