# 自定义设备示例

这个示例展示如何在VM中添加和使用自定义设备,包括MMIO接口、中断处理和设备模拟。

## 功能特性

1. **自定义设备** - 创建一个简单的计数器设备
2. **MMIO接口** - 实现内存映射I/O
3. **设备寄存器** - 读写设备配置和状态
4. **中断处理** - 模拟设备中断
5. **设备交互** - 通过RISC-V程序访问设备

## 运行示例

```bash
cargo run --example custom_device
```

## 设备规范

### 计数器设备 (Counter Device)

一个简单的计数设备,支持启用/禁用、计数、重置和中断。

#### 寄存器映射

| 偏移   | 名称         | 访问 | 说明                     |
|--------|--------------|------|--------------------------|
| 0x00   | COUNT        | RO   | 当前计数值               |
| 0x08   | RESET        | WO   | 写1重置计数器            |
| 0x10   | ENABLE       | RW   | 1启用/0禁用计数器        |
| 0x18   | IRQ_STATUS   | RC   | 中断状态(读清除)         |

#### 中断行为

- 每计数10次触发一次中断
- 中断状态下IRQ_STATUS寄存器为1
- 读取IRQ_STATUS寄存器清除中断

## 预期输出

```
=== 自定义设备示例 ===

步骤 1: 创建计数器设备
✅ 设备创建成功
   基地址: 0x10000

步骤 2: 创建VM配置
✅ 配置创建成功

步骤 3: 设置内存
✅ 内存区域配置完成
   代码段: 0x1000
   设备MMIO: 0x10000

步骤 4: 准备测试程序
✅ 程序已加载 (XXX 字节)

步骤 5: 创建执行引擎
✅ 执行引擎就绪

步骤 6: 执行程序并处理设备交互
--- 开始执行 ---

[设备] 计数器启用
[设备] 中断触发! count = 10
[VM] 检测到设备中断!
[设备] 中断触发! count = 20
[VM] 检测到设备中断!

✅ 程序执行完成
总执行指令数: XXX

步骤 7: 设备状态
  计数值: 20
  状态: 启用
  中断: 待处理

步骤 8: 通过程序读取设备寄存器
执行读取程序...
程序读取到的计数值: 20

=== 示例完成 ===
```

## 程序结构

### 设备实现

```rust
struct CounterDevice {
    count: u64,           // 当前计数值
    enabled: bool,        // 是否启用
    irq_pending: bool,    // 中断待处理
    base_address: u64,    // MMIO基址
}

impl CounterDevice {
    fn read(&mut self, offset: u64, size: u8) -> Result<u64> {
        // 根据偏移读取对应寄存器
    }

    fn write(&mut self, offset: u64, value: u64, size: u8) -> Result<()> {
        // 根据偏移写入对应寄存器
    }

    fn tick(&mut self) {
        // 模拟设备操作
    }
}
```

### MMIO拦截

使用拦截器捕获设备访问:

```rust
struct MmioInterceptor {
    device: Arc<Mutex<CounterDevice>>,
}

impl MmioInterceptor {
    fn intercept_read(&self, addr: u64, size: u8) -> Result<u64> {
        // 检查是否是设备地址
        // 如果是,调用device.read()
        // 否则,执行普通内存访问
    }

    fn intercept_write(&self, addr: u64, value: u64, size: u8) -> Result<()> {
        // 类似read的处理
    }
}
```

### RISC-V程序访问设备

```assembly
# 加载设备基址
lui x1, 0x10000

# 启用设备
li  x2, 1
sw  x2, 16(x1)    # 写ENABLE寄存器

# 读取计数值
ld  x3, 0(x1)     # 读COUNT寄存器

# 重置设备
li  x4, 1
sw  x4, 8(x1)     # 写RESET寄存器
```

## 学习要点

### 1. MMIO原理

内存映射I/O(Memory-Mapped I/O)将设备寄存器映射到内存地址空间:
- 设备寄存器占用特定的内存地址
- CPU使用普通load/store指令访问设备
- 硬件拦截这些访问并路由到设备

### 2. 设备寄存器

设备寄存器提供设备控制和状态接口:
- **控制寄存器**: 配置设备行为(如ENABLE)
- **状态寄存器**: 报告设备状态(如COUNT, IRQ_STATUS)
- **数据寄存器**: 传输数据

### 3. 中断处理

设备通过中断通知CPU:
1. 设备产生中断条件
2. 设置中断状态寄存器
3. CPU检测到中断
4. CPU读取中断状态并处理
5. 读取状态寄存器清除中断

### 4. 设备模拟

在VM中模拟设备需要:
- 维护设备状态
- 拦截MMIO访问
- 模拟设备行为(如tick)
- 产生中断信号

## 扩展练习

### 1. 添加更多设备

创建其他类型的设备:
- **定时器设备**: 可配置的定时器
- **UART设备**: 串口通信
- **GPIO设备**: 通用I/O
- **块设备**: 简单的存储设备

### 2. 实现DMA

添加直接内存访问功能:
```rust
fn dma_transfer(&mut self, src_addr: u64, dst_addr: u64, size: u64) {
    // 直接在内存间传输数据,不经过CPU
}
```

### 3. 设备队列

实现队列接口(如virtio):
- 环形缓冲区
- 描述符队列
- 可用/使用队列

### 4. 中断控制器

创建中断控制器管理多个设备:
- 中断优先级
- 中断屏蔽
- 中断分发

## 实际应用

### Virtio设备

Virtio是虚拟化环境的I/O设备标准:
- virtio-net: 网络设备
- virtio-blk: 块设备
- virtio-serial: 串口设备
- virtio-gpu: 图形设备

### 示例: 简单的virtio-blk

```rust
struct VirtioBlockDevice {
    queue: VirtQueue,
    config: BlockConfig,
    status: u32,
    // ...
}

impl VirtioBlockDevice {
    fn handle_queue(&mut self) -> Result<()> {
        // 处理I/O请求
    }
}
```

## 故障排除

### 访问违规

如果程序访问未映射的地址:
- 检查内存区域配置
- 检查设备地址范围
- 确认访问权限(RO/WO/RW)

### 中断丢失

如果中断未被正确处理:
- 检查中断状态寄存器
- 确认中断被正确清除
- 验证中断检查频率

### 设备状态不一致

如果设备状态异常:
- 检查寄存器访问顺序
- 验证设备初始化
- 添加日志调试

## 相关文档

- [Virtio规范](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)
- [RISC-V物理内存保护](https://riscv.org/technical/specifications/)
- [VM设备管理](../../docs/tutorials/ADVANCED_USAGE.md)
- [JIT执行示例](../jit_execution/)
