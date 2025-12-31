# 内存系统架构

## 目录

- [内存系统概述](#内存系统概述)
- [物理内存管理](#物理内存管理)
- [软件MMU](#软件mmu)
- [TLB优化](#tlb优化)
- [分页模式](#分页模式)
- [地址翻译流程](#地址翻译流程)

---

## 内存系统概述

### 职责

内存子系统负责Guest物理内存管理、虚拟地址到物理地址的翻译（MMU）、TLB缓存优化和MMIO设备映射。

### 架构层次

```
┌─────────────────────────────────────────────────────────┐
│                     应用层                              │
│                  Guest操作系统                         │
└────────────────────┬────────────────────────────────────┘
                     │ 虚拟地址访问
                     ↓
┌─────────────────────────────────────────────────────────┐
│                   软件MMU层                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐     │
│  │  ITLB    │  │  DTLB    │  │  页表遍历器       │     │
│  │ 指令TLB  │  │ 数据TLB  │  │ PageTableWalker  │     │
│  └──────────┘  └──────────┘  └──────────────────┘     │
└────────────────────┬────────────────────────────────────┘
                     │ 物理地址
                     ↓
┌─────────────────────────────────────────────────────────┐
│                  物理内存层                             │
│  ┌──────────────────────────────────────────────────┐  │
│  │        PhysicalMemory (分片内存)                 │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐           │  │
│  │  │ Shard0  │ │ Shard1  │ │ ShardN  │           │  │
│  │  └─────────┘ └─────────┘ └─────────┘           │  │
│  └──────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────┐  │
│  │           MMIO设备映射                           │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## 物理内存管理

### PhysicalMemory设计

```rust
pub struct PhysicalMemory {
    /// 分片内存 (16个分片，减少锁竞争)
    shards: Vec<RwLock<Vec<u8>>>,
    shard_size: usize,
    total_size: usize,
    /// MMIO设备区域
    mmio_regions: RwLock<Vec<MmioRegion>>,
    /// LR/SC保留
    reservations: RwLock<Vec<(GuestPhysAddr, u64, u8)>>,
    huge_page_allocator: HugePageAllocator,
}
```

### 分片设计

**目标**: 减少多vCPU并发访问时的锁竞争

```
传统设计:
┌────────────────────────────────┐
│    RwLock<Vec<u8>>             │  单个大锁
│    ┌───────────────────────┐   │
│    │   全部内存             │   │
│    └───────────────────────┘   │
└────────────────────────────────┘
    ↑
    所有vCPU竞争同一个锁

分片设计:
┌───────────┬───────────┬──────────────┐
│  Shard0   │  Shard1   │ ...  Shard15 │  每个分片独立锁
│  RwLock   │  RwLock   │    RwLock    │
└───────────┴───────────┴──────────────┘
    ↓           ↓              ↓
  vCPU0      vCPU1          vCPU15
  
竞争减少16倍
```

**实现**:

```rust
impl PhysicalMemory {
    pub fn new(size: usize, use_hugepages: bool) -> Self {
        const SHARD_COUNT: usize = 16;
        let shard_size = size.div_ceil(SHARD_COUNT);

        let mut shards = Vec::with_capacity(SHARD_COUNT);
        for i in 0..SHARD_COUNT {
            let current_shard_size = if i == SHARD_COUNT - 1 {
                size - (shard_size * (SHARD_COUNT - 1))
            } else {
                shard_size
            };

            shards.push(RwLock::new(vec![0u8; current_shard_size]));
        }

        Self {
            shards,
            shard_size,
            total_size: size,
            mmio_regions: RwLock::new(Vec::new()),
            reservations: RwLock::new(Vec::new()),
            huge_page_allocator: HugePageAllocator::new(use_hugepages),
        }
    }

    #[inline]
    fn get_shard_index(&self, addr: usize) -> (usize, usize) {
        (addr / self.shard_size, addr % self.shard_size)
    }

    #[inline]
    pub fn read_u64(&self, addr: usize) -> Result<u64, VmError> {
        let (idx, offset) = self.get_shard_index(addr);
        let shard = self.shards[idx].read();
        
        // 快速路径：不跨越分片边界
        if offset + 8 <= shard.len() {
            Ok(u64::from_le_bytes([
                shard[offset],
                shard[offset + 1],
                shard[offset + 2],
                shard[offset + 3],
                shard[offset + 4],
                shard[offset + 5],
                shard[offset + 6],
                shard[offset + 7],
            ]))
        } else {
            // 慢速路径：跨越分片边界
            drop(shard);
            let mut buf = [0u8; 8];
            self.read_buf(addr, &mut buf)?;
            Ok(u64::from_le_bytes(buf))
        }
    }
}
```

### 大页支持

**优势**:
- 减少页表项数量
- 减少TLB压力
- 提高TLB命中率

```rust
pub struct HugePageAllocator {
    use_hugepages: bool,
    page_size: HugePageSize,
}

pub enum HugePageSize {
    Size2M,   // 2MB大页
    Size1G,   // 1GB巨页
}

impl HugePageAllocator {
    pub fn allocate_linux(&self, size: usize) -> Result<*mut u8, Error> {
        if !self.use_hugepages {
            return Ok(alloc::alloc::alloc(Layout::from_size_align(
                size, 4096
            )?));
        }

        // 使用mmap MAP_HUGETLB
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_HUGETLB,
                -1,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            Err(Error::AllocationFailed)
        } else {
            Ok(ptr as *mut u8)
        }
    }
}
```

---

## 软件MMU

### SoftMmu设计

```rust
pub struct SoftMmu {
    id: u64,
    phys_mem: Arc<PhysicalMemory>,
    itlb: Tlb,              // 指令TLB (64条目)
    dtlb: Tlb,              // 数据TLB (128条目)
    paging_mode: PagingMode,
    page_table_base: GuestPhysAddr,
    asid: u16,              // 地址空间ID
    page_table_walker: Option<Box<dyn PageTableWalker>>,
    tlb_hits: u64,
    tlb_misses: u64,
    strict_align: bool,
}
```

### 分页模式

```rust
pub enum PagingMode {
    /// 无分页（恒等映射）
    Bare,
    /// RISC-V SV39（3级页表，39位虚拟地址）
    Sv39,
    /// RISC-V SV48（4级页表，48位虚拟地址）
    Sv48,
    /// ARM64四级页表
    Arm64,
    /// x86_64四级页表
    X86_64,
}
```

**RISC-V Sv39页表结构**:

```
39位虚拟地址布局:
┌──────┬──────┬──────┬─────────┐
│ VPN2 │ VPN1 │ VPN0 │ offset  │
│ [38:30] │ [29:21] │ [20:12] │ [11:0] │
└──────┴──────┴──────┴─────────┘
   ↓         ↓         ↓         ↓
  512      512      512       4096
  项       项       项        字节

三级页表:
┌─────────┐    ┌─────────┐    ┌─────────┐
│ Page    │ →  │ Page    │ →  │ Page    │
│ Table   │    │ Table   │    │ Table   │
│ Level 2 │    │ Level 1 │    │ Level 0 │
└─────────┘    └─────────┘    └─────────┘
   ↓                            ↓
512个PTE                     512个PTE
每个PTE: 8字节               指向物理页

页表项 (PTE) 格式:
┌────┬──────┬──────┬──────┬──────┐
│ 0  │  [53:10]  │  [9:8] │ [7:6] │ [5:0] │
│ V  │   PPN     │  R/W  │  A/D  │ G U X W R │
└────┴──────────┴──────┴──────┴──────┘
```

### 地址翻译流程

```rust
impl SoftMmu {
    pub fn translate(&mut self, va: GuestAddr, access: AccessType)
        -> Result<GuestPhysAddr, VmError>
    {
        match self.paging_mode {
            PagingMode::Bare => {
                // Bare模式：恒等映射
                Ok(GuestPhysAddr(va.0))
            }
            _ => {
                // 1. 提取VPN
                let vpn = va.0 >> PAGE_SHIFT;

                // 2. TLB查找
                let tlb_result = match access {
                    AccessType::Execute => self.itlb.lookup(vpn, self.asid),
                    _ => self.dtlb.lookup(vpn, self.asid),
                };

                if let Some((ppn, flags)) = tlb_result {
                    // 3. TLB命中：检查权限
                    self.check_permission(flags, access)?;
                    self.tlb_hits += 1;
                    
                    let offset = va.0 & (PAGE_SIZE - 1);
                    return Ok(GuestPhysAddr((ppn << PAGE_SHIFT) | offset));
                }

                // 4. TLB未命中：页表遍历
                self.tlb_misses += 1;
                let mut walker = self.page_table_walker.take().unwrap();
                let (phys_addr, flags) = walker.walk(va, access, self.asid, self)?;
                self.page_table_walker = Some(walker);

                // 5. 填充TLB
                let ppn = phys_addr.0 >> PAGE_SHIFT;
                match access {
                    AccessType::Execute => self.itlb.insert(vpn, ppn, flags, self.asid),
                    _ => self.dtlb.insert(vpn, ppn, flags, self.asid),
                }

                Ok(phys_addr)
            }
        }
    }
}
```

---

## TLB优化

### TLB设计

```rust
struct Tlb {
    /// 非全局条目 (ASID-tagged)
    entries: HashMap<u64, TlbEntry>,
    /// LRU状态
    lru: LruCache<u64, ()>,
    /// 全局条目 (G-flag set)
    global_entries: HashMap<u64, TlbEntry>,
    max_size: usize,
}

struct TlbEntry {
    guest_addr: GuestAddr,
    phys_addr: GuestPhysAddr,
    flags: u64,      // PTE标志
    asid: u16,       // 地址空间ID
}
```

### 组合键

```rust
/// 将VPN和ASID组合为单个u64键
#[inline]
fn make_tlb_key(vpn: u64, asid: u16) -> u64 {
    (vpn << 16) | (asid as u64)
}

// 查找示例
impl Tlb {
    fn lookup(&mut self, vpn: u64, asid: u16) -> Option<(u64, u64)> {
        // 1. 先查全局条目（不需要ASID匹配）
        if let Some(entry) = self.global_entries.get(&vpn) {
            return Some((entry.phys_addr.0, entry.flags));
        }

        // 2. 再查ASID特定条目
        let key = make_tlb_key(vpn, asid);
        if let Some(entry) = self.entries.get(&key) {
            self.lru.get(&key);  // 更新LRU
            return Some((entry.phys_addr.0, entry.flags));
        }

        None
    }
}
```

### TLB刷新

```rust
impl Tlb {
    /// 刷新所有TLB
    fn flush(&mut self) {
        self.entries.clear();
        self.lru.clear();
        self.global_entries.clear();
    }

    /// 刷新特定ASID
    fn flush_asid(&mut self, target_asid: u16) {
        let keys_to_remove: Vec<u64> = self.entries
            .iter()
            .filter(|(_, e)| e.asid == target_asid)
            .map(|(k, _)| *k)
            .collect();

        for key in keys_to_remove {
            self.entries.remove(&key);
            self.lru.pop(&key);
        }
    }

    /// 刷新特定页
    fn flush_page(&mut self, vpn: u64) {
        self.global_entries.remove(&vpn);
        
        let keys_to_remove: Vec<u64> = self.entries
            .iter()
            .filter(|(_, e)| e.guest_addr.0 == (vpn << PAGE_SHIFT))
            .map(|(k, _)| *k)
            .collect();

        for key in keys_to_remove {
            self.entries.remove(&key);
            self.lru.pop(&key);
        }
    }
}
```

### TLB性能

| 场景 | 延迟 | 吞吐量 |
|------|------|--------|
| TLB命中 | ~5ns | >200M ops/s |
| TLB未命中 | ~50-100ns | >10M translations/s |
| TLB刷新 | ~100-500ns | N/A |

---

## 分页模式

### RISC-V Sv39页表遍历

```rust
pub struct Sv39PageTableWalker {
    root_table: GuestPhysAddr,
    asid: u16,
}

impl PageTableWalker for Sv39PageTableWalker {
    fn walk(&mut self, va: GuestAddr, access: AccessType,
        asid: u16, mmu: &dyn MMU)
        -> Result<(GuestPhysAddr, u64), VmError>
    {
        // 提取VPN和偏移
        let vpn = [
            (va >> 12) & 0x1FF,  // VPN0 [20:12]
            (va >> 21) & 0x1FF,  // VPN1 [29:21]
            (va >> 30) & 0x1FF,  // VPN2 [38:30]
        ];
        let offset = va.0 & 0xFFF;  // [11:0]

        // 从根页表开始遍历
        let mut table_addr = self.root_table;

        for level in (0..=2).rev() {
            // 读取PTE
            let pte_addr = table_addr + vpn[level] * 8;
            let pte = mmu.read(GuestAddr(pte_addr), 8)?;

            // 检查V位
            if pte & 1 == 0 {
                return Err(VmError::Execution(
                    ExecutionError::Fault(Fault::PageFault {
                        addr: va,
                        access_type: access,
                        is_write: access == AccessType::Write,
                        is_user: false,
                    })
                ));
            }

            // 检查是否为叶子PTE
            let is_leaf = (pte & 0xE) != 0 && (level == 0 || (pte & 0x80) != 0);
            
            if is_leaf {
                // 叶子PTE：检查权限并返回物理地址
                Self::check_permission(pte, access)?;
                
                let ppn = pte >> 10;
                let phys_addr = (ppn << 12) | offset;
                
                return Ok((GuestPhysAddr(phys_addr), pte & 0xFF));
            }

            // 非叶子PTE：继续遍历下一级
            table_addr = GuestPhysAddr((pte >> 10) << 12);
        }

        Err(VmError::Execution(ExecutionError::Fault(Fault::PageFault {
            addr: va,
            access_type: access,
            is_write: access == AccessType::Write,
            is_user: false,
        })))
    }
}
```

---

## 地址翻译流程

### 完整流程图

```
Guest虚拟地址 (39位)
    │
    ↓
┌─────────────────────────────────────┐
│         1. TLB查找                 │
│  ┌───────────────────────────────┐ │
│  │ key = (vpn << 16) | asid      │ │
│  │ if ITLB/DTLB.contains(key):   │ │
│  │     return phys_addr, flags   │ │
│  └───────────────────────────────┘ │
└────────────┬────────────────────────┘
             │ 未命中
             ↓
┌─────────────────────────────────────┐
│         2. 页表遍历                │
│  ┌───────────────────────────────┐ │
│  │ for level = 2 downto 0:       │ │
│  │   pte = page_table[level]     │ │
│  │   if pte.V == 0: fault       │ │
│  │   if is_leaf(pte):           │ │
│  │     check_permission(pte)     │ │
│  │     return phys_addr         │ │
│  │   else:                      │ │
│  │     next_table = pte.PPN     │ │
│  └───────────────────────────────┘ │
└────────────┬────────────────────────┘
             │
             ↓
┌─────────────────────────────────────┐
│         3. 权限检查                │
│  ┌───────────────────────────────┐ │
│  │ if access == Read && !pte.R: │ │
│  │     fault                    │ │
│  │ if access == Write && !pte.W:│ │
│  │     fault                    │ │
│  │ if access == Exec && !pte.X: │ │
│  │     fault                    │ │
│  │                               │ │
│  │ 设置pte.A和pte.D位            │ │
│  └───────────────────────────────┘ │
└────────────┬────────────────────────┘
             │
             ↓
┌─────────────────────────────────────┐
│         4. 填充TLB                │
│  ┌───────────────────────────────┐ │
│  │ tlb.insert(vpn, ppn, flags)   │ │
│  └───────────────────────────────┘ │
└────────────┬────────────────────────┘
             │
             ↓
        Guest物理地址
```

### 性能优化

#### 1. 页表缓存

```rust
pub struct PageTableCache {
    /// 最近使用的页表页
    cache: LruCache<GuestPhysAddr, PageTablePage>,
}

struct PageTablePage {
    entries: [u64; 512],  // 512个PTE
    dirty: bool,
}

impl PageTableCache {
    pub fn read_pte(&mut self, addr: GuestPhysAddr, mmu: &dyn MMU)
        -> Result<u64, VmError>
    {
        let page_addr = addr & !(0xFFF);  // 对齐到页边界
        let offset = (addr.0 & 0xFFF) / 8;

        if let Some(page) = self.cache.get_mut(&page_addr) {
            return Ok(page.entries[offset as usize]);
        }

        // 缓存未命中：读取整个页表页
        let mut entries = [0u64; 512];
        for i in 0..512 {
            entries[i] = mmu.read(GuestAddr(page_addr.0 + i * 8), 8)?;
        }

        self.cache.put(page_addr, PageTablePage {
            entries,
            dirty: false,
        });

        Ok(entries[offset as usize])
    }
}
```

#### 2. 批量页表遍历

```rust
impl SoftMmu {
    /// 批量翻译多个地址（预取优化）
    pub fn translate_batch(&mut self, addrs: &[GuestAddr], access: AccessType)
        -> Result<Vec<GuestPhysAddr>, VmError>
    {
        addrs.iter()
            .map(|&addr| self.translate(addr, access))
            .collect()
    }
}
```

---

**文档版本**: 1.0
**最后更新**: 2025-12-31
**作者**: VM开发团队
