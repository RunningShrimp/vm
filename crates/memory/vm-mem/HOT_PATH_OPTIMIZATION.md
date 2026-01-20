# 热点路径优化策略

本文档描述虚拟机系统中关键热点路径的优化策略。

## 目录

- [TLB 查找优化](#tlb-查找优化)
- [指令解码优化](#指令解码优化)
- [代码生成优化](#代码生成优化)
- [内存操作优化](#内存操作优化)

---

## TLB 查找优化

### 当前实现

TLB 查找是虚拟机中最频繁的操作之一。当前 `MultiLevelTlb` 实现了多级缓存结构：

```rust
pub fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<vm_core::TlbEntry> {
    let vpn = addr.0 >> PAGE_SHIFT;
    let key = SingleLevelTlb::make_key(vpn, asid);

    // L1 查找
    if let Some(entry) = self.l1_tlb.entries.get(&key) {
        if entry.check_permission(access) {
            return Some(vm_core::TlbEntry { ... });
        }
    }
    // L2 查找
    if let Some(entry) = self.l2_tlb.entries.get(&key) {
        // ...
    }
    // L3 查找
    if let Some(entry) = self.l3_tlb.entries.get(&key) {
        // ...
    }
    None
}
```

### 优化策略

#### 1. 哈希表优化

- **使用 `FxHashMap` 替代 `HashMap`**: `FxHashMap` 使用更好的哈希函数，减少冲突
- **预分配哈希表容量**: 根据预期负载预分配，减少 rehash

```rust
use rustc_hash::FxHashMap;

pub struct SingleLevelTlb {
    entries: FxHashMap<u64, OptimizedTlbEntry>,
    // ...
}
```

#### 2. 分支预测优化

- **使用 `likely/unlikely` 宏**: 提示编译器分支预测方向
- **减少嵌套 if-else**: 使用 guard clauses 提前返回

```rust
use std::intrinsics::likely;

if likely(self.entries.contains_key(&key)) {
    // 快速路径
} else {
    // 慢速路径
}
```

#### 3. SIMD 并行查找

对于小容量 TLB，可以使用 SIMD 并行查找多个条目：

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

unsafe fn simd_lookup(keys: &[u64], target: u64) -> Option<usize> {
    let target_vec = _mm256_set1_epi64x(target);
    for (i, chunk) in keys.chunks(4).enumerate() {
        let chunk_vec = _mm256_loadu_si256(chunk.as_ptr() as *const __m256i);
        let cmp = _mm256_cmpeq_epi64(chunk_vec, target_vec);
        let mask = _mm256_movemask_epi8(cmp);
        if mask != 0 {
            return Some(i * 4 + mask.trailing_zeros() as usize / 8);
        }
    }
    None
}
```

#### 4. 内联优化

对关键查找函数使用 `#[inline(always)]`:

```rust
#[inline(always)]
pub fn lookup(&mut self, vpn: u64, asid: u16) -> Option<&OptimizedTlbEntry> {
    // ...
}
```

#### 5. 缓存友好访问

- **使用 `Vec` 代替 `HashMap` 对于小容量 TLB**: 对于 L1 TLB（< 128 条目），线性扫描可能更快
- **使用 `BTreeMap` 对于中等容量 TLB**: 更好的缓存局部性

---

## 指令解码优化

### 当前实现

指令解码是另一个热点路径。解码器需要解析指令字节并生成 IR。

### 优化策略

#### 1. 查找表优化

使用查找表加速常见指令的解码：

```rust
pub struct InstructionDecoder {
    opcode_table: [InstructionHandler; 256],
    // ...
}

impl InstructionDecoder {
    pub fn decode(&self, bytes: &[u8]) -> Result<DecodedInstruction, DecodeError> {
        let opcode = bytes[0];
        let handler = self.opcode_table[opcode as usize];
        handler.decode(bytes)
    }
}
```

#### 2. 分支预测优化

使用 `match` 替代 `if-else` 链，Rust 编译器会优化为跳转表：

```rust
fn decode_opcode(opcode: u8) -> InstructionType {
    match opcode {
        0x13 => InstructionType::Addi,
        0x33 => InstructionType::Add,
        0x03 => InstructionType::Lw,
        // ...
        _ => InstructionType::Unknown,
    }
}
```

#### 3. 批量解码

对于循环中的指令，可以批量解码：

```rust
pub fn decode_batch(&self, bytes: &[u8], count: usize) -> Vec<DecodedInstruction> {
    let mut result = Vec::with_capacity(count);
    let mut offset = 0;

    for _ in 0..count {
        if offset + 4 > bytes.len() {
            break;
        }
        match self.decode(&bytes[offset..offset + 4]) {
            Ok(instr) => {
                result.push(instr);
                offset += 4;
            }
            Err(_) => break,
        }
    }

    result
}
```

#### 4. SIMD 指令解码

对于固定长度的指令（如 RISC-V），可以使用 SIMD 并行解码：

```rust
#[cfg(target_arch = "x86_64")]
unsafe fn parallel_decode_riscv(input: &[u8]) -> Vec<DecodedInstruction> {
    let mut result = Vec::new();

    for chunk in input.chunks(32) {
        let vec = _mm256_loadu_si256(chunk.as_ptr() as *const __m256i);

        // 并行检查多个指令
        let opcodes = _mm256_and_si256(vec, _mm256_set1_epi8(0x7F));
        // ... 更多 SIMD 操作

        result.extend(decode_simd_result(vec, opcodes));
    }

    result
}
```

---

## 代码生成优化

### 当前实现

JIT 代码生成将 IR 转换为机器码。

### 优化策略

#### 1. 寄存器分配优化

使用图着色算法优化寄存器分配：

```rust
pub struct RegisterAllocator {
    available_registers: HashSet<Register>,
    live_ranges: HashMap<IRValue, LiveRange>,
    // ...
}

impl RegisterAllocator {
    pub fn allocate(&mut self, ir_block: &IRBlock) -> AllocationResult {
        // 构建干扰图
        let interference_graph = self.build_interference_graph(ir_block);

        // 图着色
        let coloring = self.color_graph(&interference_graph);

        AllocationResult { coloring }
    }
}
```

#### 2. 代码缓存优化

使用缓存避免重复编译相同的 IR 块：

```rust
pub struct CodeCache {
    cache: LruCache<IRHash, CompiledCode>,
    max_size: usize,
}

impl CodeCache {
    pub fn get_or_compile(&mut self, ir_block: &IRBlock, compiler: &mut JITCompiler) -> &CompiledCode {
        let hash = hash_ir_block(ir_block);

        if !self.cache.contains(&hash) {
            let compiled = compiler.compile(ir_block).unwrap();
            self.cache.put(hash, compiled);
        }

        self.cache.get(&hash).unwrap()
    }
}
```

#### 3. 内联扩展

对小函数进行内联扩展，减少函数调用开销：

```rust
pub fn should_inline(ir_func: &IRFunction, caller: &IRFunction) -> bool {
    ir_func.instruction_count() < INLINE_THRESHOLD
        && !ir_func.is_recursive()
        && caller.call_count(ir_func) > INLINE_HOTNESS_THRESHOLD
}
```

#### 4. 死代码消除

在代码生成前消除死代码：

```rust
pub fn eliminate_dead_code(ir_block: &mut IRBlock) {
    let mut used = HashSet::new();

    // 标记所有使用的值
    for instr in &ir_block.instructions {
        if let IRInstruction::Return { value } = instr {
            used.insert(*value);
        }
    }

    // 逆向遍历，标记所有使用的操作数
    for instr in ir_block.instructions.iter().rev() {
        if let Some(dest) = instr.dest() {
            if used.contains(&dest) {
                if let Some(operands) = instr.operands() {
                    used.extend(operands);
                }
            }
        }
    }

    // 删除未使用的指令
    ir_block.instructions.retain(|instr| {
        if let Some(dest) = instr.dest() {
            used.contains(&dest)
        } else {
            true
        }
    });
}
```

---

## 内存操作优化

### 当前实现

内存读写是虚拟机的关键操作。

### 优化策略

#### 1. 批量读写

使用批量操作减少函数调用开销：

```rust
pub fn read_bulk(&mut self, addr: GuestAddr, buffer: &mut [u8]) -> Result<(), VmError> {
    let mut offset = 0;
    while offset < buffer.len() {
        let chunk_size = (buffer.len() - offset).min(BATCH_SIZE);
        self.read_bulk_internal(addr + offset, &mut buffer[offset..offset + chunk_size])?;
        offset += chunk_size;
    }
    Ok(())
}
```

#### 2. 零拷贝操作

使用 `Cow` 或引用减少内存拷贝：

```rust
use std::borrow::Cow;

pub fn read_cow(&self, addr: GuestAddr, size: usize) -> Result<Cow<[u8]>, VmError> {
    if addr.0 + size as u64 <= self.mapped_range.end {
        Ok(Cow::Borrowed(&self.memory[addr.0 as usize..addr.0 as usize + size]))
    } else {
        let buffer = self.read(addr, size)?;
        Ok(Cow::Owned(buffer))
    }
}
```

#### 3. 预取优化

使用 CPU 预取指令：

```rust
#[cfg(target_arch = "x86_64")]
unsafe fn prefetch_memory(addr: *const u8) {
    _mm_prefetch(addr as *const i8, _MM_HINT_T0);
}

pub fn read_with_prefetch(&mut self, addr: GuestAddr) -> Result<u64, VmError> {
    unsafe {
        prefetch_memory((addr.0 + 64) as *const u8);
    }
    self.read(addr, 8)
}
```

#### 4. 内存池

使用内存池减少分配：

```rust
pub struct MemoryPool {
    pool: Vec<Vec<u8>>,
    free_list: Vec<usize>,
}

impl MemoryPool {
    pub fn allocate(&mut self, size: usize) -> Vec<u8> {
        if let Some(idx) = self.free_list.pop() {
            if self.pool[idx].capacity() >= size {
                let mut buffer = self.pool.swap_remove(idx);
                buffer.clear();
                buffer
            } else {
                vec![0u8; size]
            }
        } else {
            vec![0u8; size]
        }
    }

    pub fn deallocate(&mut self, buffer: Vec<u8>) {
        self.free_list.push(self.pool.len());
        self.pool.push(buffer);
    }
}
```

---

## 性能监控

### 添加热点检测

```rust
pub struct HotspotDetector {
    counters: HashMap<String, AtomicU64>,
    threshold: u64,
}

impl HotspotDetector {
    pub fn record(&self, name: &str) {
        if let Some(counter) = self.counters.get(name) {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn is_hot(&self, name: &str) -> bool {
        self.counters.get(name)
            .map(|c| c.load(Ordering::Relaxed) >= self.threshold)
            .unwrap_or(false)
    }
}
```

---

## 总结

热点路径优化包括：

1. **TLB 查找优化**: 使用更好的哈希表、分支预测、SIMD、内联
2. **指令解码优化**: 查找表、分支预测、批量解码、SIMD
3. **代码生成优化**: 寄存器分配、代码缓存、内联扩展、死代码消除
4. **内存操作优化**: 批量读写、零拷贝、预取、内存池

这些优化可以显著提高虚拟机的性能，特别是在高频操作上。
