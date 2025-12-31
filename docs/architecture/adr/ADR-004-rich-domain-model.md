# ADR-004: 充血模型采用

## 状态
已接受 (2024-12-31)

## 上下文
VM项目有两种主要的领域模型设计模式：

### 贫血模型 (Anemic Domain Model)
```rust
// 数据只有，无行为
pub struct BlockDevice {
    pub config: BlockConfig,
    pub queue: VirtQueue,
    pub backend: Box<dyn BlockBackend>,
}

// 业务逻辑在服务层
impl BlockService {
    pub fn process_request(&self, device: &mut BlockDevice, req: Request) {
        // 业务逻辑
    }
}
```

### 充血模型 (Rich Domain Model)
```rust
// 数据和行为绑定
pub struct BlockDevice {
    config: BlockConfig,      // 私有字段
    queue: VirtQueue,
    backend: Box<dyn BlockBackend>,
}

impl BlockDevice {
    // 业务逻辑在领域对象内部
    pub fn process_request(&mut self, req: Request) -> BlockResult {
        req.validate()?;
        self.execute_request(req)
    }
}
```

## 决策
采用充血模型（Rich Domain Model）设计。

## 理由

### 优势

1. **高内聚性**:
   - 数据和行为紧密绑定
   - 更符合对象导向原则

2. **类型安全**:
   - 强类型的业务规则
   - 编译时保证正确性

3. **可测试性**:
   - 纯函数式业务逻辑
   - 易于单元测试

4. **可维护性**:
   - 业务逻辑集中在领域对象
   - 减少服务层复杂度

### 对比示例

#### 贫血模型
```rust
// 服务层包含业务逻辑
impl BlockService {
    pub fn read(&self, device: &mut BlockDevice, sector: u64, data: &mut [u8]) 
        -> Result<(), Error> 
    {
        // 验证
        if sector >= device.config.capacity {
            return Err(Error::InvalidSector);
        }
        if data.len() != device.config.block_size as usize {
            return Err(Error::InvalidSize);
        }
        
        // 执行
        device.backend.read(sector, data)?;
        Ok(())
    }
}

// 问题：
// 1. 验证逻辑分散
// 2. 服务层臃肿
// 3. 领域对象失血
```

#### 充血模型
```rust
// 业务逻辑封装在领域对象
impl BlockDevice {
    pub fn read(&mut self, sector: u64, data: &mut [u8]) -> Result<(), BlockError> {
        // 验证
        sector.validate(&self.config)?;
        data.validate_size(&self.config)?;
        
        // 执行
        self.backend.read(sector, data)?;
        self.update_stats(BlockOperation::Read);
        
        Ok(())
    }
}

// 优势：
// 1. 验证逻辑集中
// 2. 封装良好
// 3. 服务层简化
```

## 设计模式

### 1. 建造者模式

```rust
impl BlockDevice {
    pub fn builder() -> BlockDeviceBuilder {
        BlockDeviceBuilder::default()
    }
}

pub struct BlockDeviceBuilder {
    config: Option<BlockConfig>,
    backend: Option<Box<dyn BlockBackend>>,
}

impl BlockDeviceBuilder {
    pub fn config(mut self, config: BlockConfig) -> Self {
        self.config = Some(config);
        self
    }
    
    pub fn backend(mut self, backend: Box<dyn BlockBackend>) -> Self {
        self.backend = Some(backend);
        self
    }
    
    pub fn build(self) -> Result<BlockDevice, BuildError> {
        Ok(BlockDevice {
            config: self.config.ok_or(BuildError::MissingConfig)?,
            backend: self.backend.ok_or(BuildError::MissingBackend)?,
            // ...
        })
    }
}
```

### 2. 验证模式

```rust
pub struct BlockRequest {
    sector: u64,
    data: Vec<u8>,
    operation: BlockOperation,
}

impl BlockRequest {
    pub fn validate(&self, config: &BlockConfig) -> Result<(), BlockError> {
        if self.sector >= config.capacity {
            return Err(BlockError::InvalidSector(self.sector));
        }
        
        if self.data.len() != config.block_size as usize {
            return Err(BlockError::InvalidSize(self.data.len()));
        }
        
        match self.operation {
            BlockOperation::Read if !config.read_only => {},
            BlockOperation::Write if config.read_only => {
                return Err(BlockError::WriteProtected);
            }
            _ => {}
        }
        
        Ok(())
    }
}
```

### 3. 领域事件

```rust
pub enum BlockEvent {
    DeviceAttached { device_id: DeviceId },
    RequestCompleted { sector: u64, latency: Duration },
    ErrorOccurred { error: BlockError },
}

pub trait BlockEventEmitter {
    fn emit(&self, event: BlockEvent);
}
```

## 后果

### 短期
- ✅ 提高代码内聚性
- ✅ 改善类型安全
- ⚠️ 需要重构现有贫血代码

### 长期
- ✅ 降低维护成本
- ✅ 提高代码质量
- ✅ 更符合DDD原则

## 迁移计划

### 阶段1 (已完成)
- ✅ VirtioBlock充血模型重构
- ✅ BlockRequest/BlockResult类型

### 阶段2 (进行中)
- 🔄 迁移其他设备到充血模型
- 🔄 添加领域事件

### 阶段3 (计划中)
- ⏳ 完善验证逻辑
- ⏳ 添加领域服务

## 参考
- [Domain-Driven Design (Eric Evans)](https://www.domainlanguage.com/ddd/)
- [Anemic Domain Model (Martin Fowler)](https://www.martinfowler.com/bliki/AnemicDomainModel.html)

---
**创建时间**: 2024-12-31
**作者**: VM开发团队
