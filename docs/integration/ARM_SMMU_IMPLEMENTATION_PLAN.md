# ARM SMMU实施计划

## 📊 概览

**计划目标**：实现ARM SMMUv3（System Memory Management Unit）支持，提供IOMMU虚拟化功能
**预期效果**：实现完整的设备地址转换、安全隔离、DMA优化
**实施时间**：2-4周
**状态**：📋 规划中

---

## ✅ SMMUv3规范分析

### SMMUv3架构概览

SMMUv3是ARM架构的系统内存管理单元，用于：

| 功能 | 描述 | 重要性 |
|------|------|--------|
| **设备地址转换** | 将设备看到的IPA（中间物理地址）转换为PA（物理地址） | 核心功能 |
| **安全隔离** | 提供Stream ID（SID）隔离不同设备 | 安全特性 |
| **DMA优化** | 批量地址转换和缓存 | 性能优化 |
| **中断支持** | 支持MSI（消息信号中断） | 高级特性 |
| **页表支持** | 多级页表结构（类似MMU） | 基础支持 |

### SMMUv3关键寄存器

| 寄存器组 | 寄存器 | 功能 |
|----------|--------|------|
| **SMMU_CR0** | SMMU配置寄存器0 | SMMU使能、初始化状态 |
| **SMMU_CR1** | SMMU配置寄存器1 | VMID、中断配置 |
| **SMMU_CR2** | SMMU配置寄存器2 | 翻译故障配置 |
| **SMMU_SCR0** | SMMU事务控制寄存器0 | 缓存维护、命令执行 |
| **SMMU_CBFR** | 命令队列刷新寄存器 | 刷新命令队列 |
| **SMMU_SME** | SMMU错误管理寄存器 | 错误报告和控制 |
| **SMMU_SCTRLR** | SMMU系统错误控制寄存器 | 系统错误控制 |

---

## 🎯 架构设计

### SMMU模块结构

```
vm-smmu/                    (SMMU实现)
├── src/
│   ├── lib.rs                  (主模块导出)
│   ├── smmu.rs                 (SMMU核心实现)
│   ├── translation.rs           (地址转换逻辑)
│   ├── page_table.rs           (页表管理)
│   ├── stream_table.rs         (流表管理)
│   ├── cache.rs               (TLB缓存)
│   ├── interrupt.rs            (中断处理)
│   ├── config.rs              (配置管理)
│   └── error.rs               (错误定义)
├── tests/
│   ├── translation_test.rs       (转换测试)
│   ├── page_table_test.rs      (页表测试)
│   └── integration_test.rs      (集成测试)
└── Cargo.toml
```

### 核心数据结构

```rust
/// SMMU配置
pub struct SmmuConfig {
    pub base_address: u64,           // SMMU寄存器基址
    pub num_sids: u16,               // Stream ID数量
    pub page_size: usize,             // 页面大小（4KB/64KB）
    pub tlb_entries: usize,            // TLB条目数
    pub enable_msix: bool,           // 是否启用MSIX
    pub enable_stall: bool,           // 是否启用中断暂停
}

/// SMMU设备
pub struct SmmuDevice {
    base: u64,                      // 寄存器基址
    config: SmmuConfig,             // 配置
    stream_tables: Vec<StreamTable>,   // 流表（SID表）
    tlb: SmmuTlb,                   // TLB缓存
    translation_cache: Arc<Mutex<TranslationCache>>, // 转换缓存
    stats: Arc<AtomicSmmuStats>,    // 统计信息
}

/// 流表条目
pub struct StreamTableEntry {
    pub sid: u16,                    // Stream ID
    pub vmid: u16,                   // 虚拟机ID
    pub s1cr: u64,                   // Stage 1 Control Register
    pub s2cr: u64,                   // Stage 2 Control Register
    pub smr: u64,                     // Stream Mapping Register
}

/// 转换结果
pub struct TranslationResult {
    pub pa: u64,                     // 物理地址
    pub valid: bool,                   // 转换是否有效
    pub permissions: u8,               // 访问权限
    pub fault_info: Option<FaultInfo>,  // 故障信息（如果失败）
}
```

---

## 📈 实施计划

### 第1周：SMMU基础框架

#### 周一：模块结构创建

**目标**：创建vm-smmu crate和基础结构

**步骤**：
- [ ] 创建vm-smmu crate
- [ ] 创建lib.rs导出所有公共接口
- [ ] 创建error.rs定义错误类型
- [ ] 创建config.rs定义配置结构
- [ ] 编写单元测试（2-3个）

**预期成果**：
- ✅ vm-smmu crate创建完成
- ✅ 基础数据结构定义完成
- ✅ 编译成功，无错误

#### 周二-三：SMMU核心实现

**目标**：实现SMMU设备核心功能

**步骤**：
- [ ] 实现SmmuDevice结构和方法
- [ ] 实现寄存器访问接口
- [ ] 实现SMMU初始化和重置
- [ ] 实现SMMU使能/禁用
- [ ] 编写单元测试（3-5个）

**预期成果**：
- ✅ SMMU设备基本操作实现
- ✅ 寄存器读写功能完成
- ✅ 单元测试通过

#### 周四-五：页表管理

**目标**：实现SMMU页表结构

**步骤**：
- [ ] 实现SMMU页表结构（类似MMU）
- [ ] 实现页表遍历和查找
- [ ] 实现页表更新和失效
- [ ] 编写单元测试（3-5个）

**预期成果**：
- ✅ 页表管理功能完成
- ✅ 支持多级页表（2/3/4级）
- ✅ 单元测试通过

#### 周六：流表管理

**目标**：实现Stream Table（SID表）管理

**步骤**：
- [ ] 实现StreamTable结构
- [ ] 实现SID分配和释放
- [ ] 实现SID到VMID映射
- [ ] 编写单元测试（3-5个）

**预期成果**：
- ✅ 流表管理功能完成
- ✅ 支持SID隔离
- ✅ 单元测试通过

### 第2周：地址转换和TLB

#### 周一-三：地址转换逻辑

**目标**：实现IPA到PA的地址转换

**步骤**：
- [ ] 实现地址转换算法（查询页表）
- [ ] 实现转换错误处理
- [ ] 实现转换统计收集
- [ ] 实现缓存优化
- [ ] 编写单元测试（5-7个）

**预期成果**：
- ✅ 地址转换功能完成
- ✅ 支持多级页表查找
- ✅ 性能优化（缓存、批处理）
- ✅ 单元测试通过

#### 周四-五：TLB缓存

**目标**：实现SMMU专用TLB

**步骤**：
- [ ] 实现SMMU TLB结构
- [ ] 实现TLB查找和更新
- [ ] 实现TLB替换策略（LRU/LFU）
- [ ] 实现TLB统计
- [ ] 编写单元测试（3-5个）

**预期成果**：
- ✅ TLB缓存功能完成
- ✅ 快速地址查找
- ✅ 单元测试通过

#### 周六：集成测试

**目标**：创建集成测试

**步骤**：
- [ ] 创建地址转换集成测试
- [ ] 创建页表集成测试
- [ ] 创建TLB集成测试
- [ ] 性能基准测试（2-3个）

**预期成果**：
- ✅ 集成测试完成
- ✅ 所有测试通过
- ✅ 性能基准建立

### 第3周：中断和高级功能

#### 周一-三：中断处理

**目标**：实现MSI和中断支持

**步骤**：
- [ ] 实现MSI消息格式定义
- [ ] 实现MSI中断触发
- [ ] 实现中断状态跟踪
- [ ] 实现中断配置
- [ ] 编写单元测试（3-5个）

**预期成果**：
- ✅ MSI中断支持完成
- ✅ 中断管理功能
- ✅ 单元测试通过

#### 周四-五：高级功能

**目标**：实现SMMU高级特性

**步骤**：
- [ ] 实现命令队列管理
- [ ] 实现缓存维护命令
- [ ] 实现错误报告机制
- [ ] 实现性能监控
- [ ] 编写单元测试（3-5个）

**预期成果**：
- ✅ 高级功能完成
- ✅ 命令队列管理
- ✅ 性能监控集成
- ✅ 单元测试通过

#### 周六：集成测试和优化

**目标**：创建集成测试和性能优化

**步骤**：
- [ ] 创建完整SMMU集成测试
- [ ] 性能基准测试（3-5个）
- [ ] 性能分析和优化
- [ ] 内存泄漏检测
- [ ] 性能调优

**预期成果**：
- ✅ 集成测试完成
- ✅ 性能优化完成
- ✅ 性能基准建立
- ✅ 内存使用优化

### 第4周：集成和文档

#### 周一-三：系统集成

**目标**：将SMMU集成到整个系统

**步骤**：
- [ ] 集成到vm-platform（passthrough.rs）
- [ ] 集成到vm-vcpu（地址转换）
- [ ] 集成到vm-io（设备管理）
- [ ] 集成到vm-mem（页表）
- [ ] 编写集成测试（5-7个）

**预期成果**：
- ✅ SMMU完全集成
- ✅ 所有模块协同工作
- ✅ 集成测试通过

#### 周四-五：文档完善

**目标**：编写完整的文档

**步骤**：
- [ ] 编写SMMU架构设计文档
- [ ] 编写API参考文档
- [ ] 编写配置指南
- [ ] 编写性能调优指南
- [ ] 编写故障排除指南

**预期成果**：
- ✅ 设计文档完整
- ✅ API文档完整
- ✅ 用户指南完整
- ✅ 性能调优指南完整

#### 周六：最终测试和发布

**目标**：最终测试和发布

**步骤**：
- [ ] 运行完整测试套件
- [ ] 性能基准测试
- [ ] 安全测试
- [ ] 内存泄漏检测
- [ ] 准备发布

**预期成果**：
- ✅ 所有测试通过
- ✅ 性能指标达标
- ✅ 安全测试通过
- ✅ 准备发布

---

## 🎯 核心实现细节

### 地址转换算法

```rust
pub struct SmmuTranslator {
    page_tables: Vec<Arc<RwLock<PageTable>>>,
    tlb: SmmuTlb,
    config: SmmuConfig,
}

impl SmmuTranslator {
    /// IPA到PA转换
    pub fn translate_ipa_to_pa(
        &self,
        ipa: u64,
        sid: u16,
        access_type: AccessType,
    ) -> Result<TranslationResult, SmmuError> {
        // 1. 查询TLB
        if let Some(result) = self.tlb.lookup(ipa, sid) {
            if result.valid {
                return Ok(result);
            }
        }
        
        // 2. 查询流表获取SID配置
        let stream_entry = self.lookup_stream_table(sid)?;
        
        // 3. 遍历页表进行转换
        let pa = self.walk_page_tables(ipa, stream_entry)?;
        
        // 4. 更新TLB
        self.tlb.update(ipa, sid, pa);
        
        Ok(TranslationResult {
            pa,
            valid: true,
            permissions: stream_entry.permissions,
            fault_info: None,
        })
    }
    
    /// 遍历页表
    fn walk_page_tables(
        &self,
        ipa: u64,
        stream_entry: &StreamTableEntry,
    ) -> Result<u64, SmmuError> {
        let mut table = self.get_page_table(stream_entry.s1cr)?;
        let mut level = 0;
        
        loop {
            let entry = table.lookup(ipa, level)?;
            
            if entry.is_block_table() {
                // 继续遍历下一级
                table = self.get_page_table(entry.address)?;
                level += 1;
            } else {
                // 找到最终物理地址
                return Ok(entry.address());
            }
        }
    }
}
```

### TLB缓存实现

```rust
pub struct SmmuTlb {
    entries: HashMap<(u64, u16), TlbEntry>,
    max_entries: usize,
    replacement_policy: TlbReplacementPolicy,
    stats: Arc<AtomicTlbStats>,
}

impl SmmuTlb {
    pub fn lookup(&self, ipa: u64, sid: u16) -> Option<TlbEntry> {
        let key = (ipa, sid);
        self.entries.get(&key)
    }
    
    pub fn update(&mut self, ipa: u64, sid: u16, pa: u64) {
        let key = (ipa, sid);
        
        if self.entries.len() >= self.max_entries {
            // 替换策略
            match self.replacement_policy {
                TlbReplacementPolicy::LRU => self.evict_lru(),
                TlbReplacementPolicy::LFU => self.evict_lfu(),
            }
        }
        
        self.entries.insert(key, TlbEntry::new(ipa, pa));
    }
}
```

### 中断处理

```rust
pub struct MsiHandler {
    msi_addresses: Vec<(u64, u8)>,  // (地址, 数据长度)
    pending_interrupts: Arc<Mutex<Vec<MsiInterrupt>>>,
    stats: Arc<AtomicMsiStats>,
}

impl MsiHandler {
    /// 触发MSI中断
    pub fn trigger_msi(&self, addr: u64, data: &[u8]) -> Result<(), SmmuError> {
        // 1. 验证MSI配置
        if !self.is_msi_enabled(addr) {
            return Err(SmmuError::MsiNotEnabled);
        }
        
        // 2. 生成MSI消息
        let msi = MsiInterrupt::new(addr, data);
        
        // 3. 发送中断
        self.send_interrupt(msi)?;
        
        // 4. 更新统计
        self.stats.increment_interrupts();
        
        Ok(())
    }
}
```

---

## 📈 预期成果

### 性能指标

| 指标 | 目标 | 当前 | 预期提升 |
|------|------|------|-----------|
| **地址转换延迟** | <100ns | N/A | -50% |
| **TLB命中率** | >95% | N/A | +15% |
| **批量转换吞吐量** | >10M ops/s | N/A | +200% |
| **中断响应时间** | <1μs | N/A | -30% |
| **内存使用** | <10MB | N/A | 基准 |

### 功能完整性

| 功能 | 目标 | 状态 |
|------|------|------|
| **基础地址转换** | ✅ 完成 | 📋 待实现 |
| **多级页表** | ✅ 支持 | 📋 待实现 |
| **TLB缓存** | ✅ 完成 | 📋 待实现 |
| **Stream ID隔离** | ✅ 支持 | 📋 待实现 |
| **MSI中断** | ✅ 支持 | 📋 待实现 |
| **命令队列** | ✅ 完成 | 📋 待实现 |
| **错误报告** | ✅ 完成 | 📋 待实现 |
| **性能监控** | ✅ 完成 | 📋 待实现 |

---

## 🎯 成功标准

### 功能完整性
- [ ] SMMU基础框架完成
- [ ] 地址转换功能完成并测试
- [ ] TLB缓存功能完成并测试
- [ ] 页表管理功能完成并测试
- [ ] Stream Table功能完成并测试
- [ ] MSI中断功能完成并测试
- [ ] 命令队列功能完成并测试
- [ ] 错误报告功能完成并测试
- [ ] 性能监控功能完成并测试

### 测试覆盖
- [ ] 单元测试覆盖率>90%
- [ ] 集成测试覆盖率>80%
- [ ] 性能基准测试完成（至少8个）
- [ ] 安全测试完成
- [ ] 压力测试完成

### 文档
- [ ] SMMU架构设计文档
- [ ] API参考文档
- [ ] 配置指南
- [ ] 性能调优指南
- [ ] 故障排除指南
- [ ] 集成指南

### 性能指标
- [ ] 地址转换延迟<100ns
- [ ] TLB命中率>95%
- [ ] 批量转换吞吐量>10M ops/s
- [ ] 中断响应时间<1μs
- [ ] 内存使用<10MB

---

## 🚀 风险评估

### 技术风险
- [ ] **高风险**：SMMUv3规范复杂，可能存在理解偏差
  - 缓解方案：仔细研读ARM官方文档，参考开源实现
  - 预期影响：可能需要1-2周额外调研时间

- [ ] **中风险**：性能优化可能引入新的bug
  - 缓解方案：渐进式实施，充分的性能测试
  - 预期影响：每个阶段1-2周调试时间

### 集成风险
- [ ] **中风险**：与现有vm-platform的集成可能存在冲突
  - 缓解方案：仔细分析现有接口，设计清晰的集成点
  - 预期影响：可能需要1-2周适配时间

---

## 📚 文档产出

### 计划文档
- [ ] `ARM_SMMU_IMPLEMENTATION_PLAN.md`（本文档）
- [ ] `SMMU_ARCHITECTURE_DESIGN.md`（架构设计）
- [ ] `SMMU_API_REFERENCE.md`（API参考）
- [ ] `SMMU_CONFIGURATION_GUIDE.md`（配置指南）

### 实现文档
- [ ] `SMMU_TRANSLATION_ALGORITHM.md`（转换算法）
- [ ] `SMMU_TLB_IMPLEMENTATION.md`（TLB实现）
- [ ] `SMMU_INTERRUPT_HANDLING.md`（中断处理）
- [ ] `SMMU_INTEGRATION_GUIDE.md`（集成指南）

### 测试文档
- [ ] `SMMU_TEST_SUITE.md`（测试套件）
- [ ] `SMMU_PERFORMANCE_BENCHMARK.md`（性能基准）
- [ ] `SMMU_TROUBLESHOOTING.md`（故障排除）
- [ ] `SMMU_MIGRATION_GUIDE.md`（迁移指南）

---

## 🎯 与其他任务的关联

SMMU实施与以下任务协同：

1. **TLB优化**：SMMU需要高效的TLB支持，可以复用TLB优化成果
2. **模块简化**：SMMU作为vm-platform的一部分，需要统一的接口
3. **性能基准**：SMMU性能需要在性能测试环境中验证
4. **设备穿透**：SMMU与vm-passthrough协同工作，提供完整的设备虚拟化

---

## 🚀 下一步行动

### 立即行动（本周）

1. **创建vm-smmu crate**
   - 创建基础模块结构
   - 定义核心数据结构
   - 编写初始单元测试

2. **研究SMMUv3规范**
   - 阅读ARM官方SMMUv3规范文档
   - 参考开源SMMU实现
   - 编写研究总结

### 短期行动（1-2周）

1. **完成SMMU基础框架**
   - 实现核心SMMU设备
   - 实现页表管理
   - 实现流表管理

2. **实现地址转换**
   - 实现IPA到PA转换算法
   - 实现TLB缓存
   - 实现错误处理

---

**状态**：📋 规划完成，等待开始实施

**预期完成时间**：2025年1月底

**预期成果**：完整的ARM SMMUv3实现，支持设备地址转换、安全隔离、DMA优化
