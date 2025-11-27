# Phase 1 Quick Reference - 已完成模块速查表

## 📖 文档导航

| 文档 | 用途 | 链接 |
|------|------|------|
| PHASE1_COMPLETION_SUMMARY.md | 完整总结 | 详见本文件 |
| PHASE1_PROGRESS_REPORT.md | 进度报告 | 详细的指标和成就 |
| REFACTORING_PHASE1_TASK1.3.md | Task 1.3 详解 | 解码器重构技术细节 |
| REFACTORING_PHASE1_TASK1.4.md | Task 1.4 详解 | 代码消重技术细节 |

## 🔧 新模块速查

### vm-core::domain - 领域接口
**文件:** `vm-core/src/domain.rs` (50 行)  
**用途:** 定义 TLB、页表、执行管理的接口

```rust
// 导入
use vm_core::domain::{TlbManager, PageTableWalker, ExecutionManager, TlbEntry};

// 关键 trait
pub trait TlbManager {
    fn lookup(...) -> Option<TlbEntry>;
    fn update(&mut self, entry: TlbEntry);
    fn flush(&mut self);
    fn flush_asid(&mut self, asid: u16);
}

pub trait PageTableWalker {
    fn walk(...) -> Result<(GuestPhysAddr, u8), Fault>;
}

pub trait ExecutionManager<B> {
    fn run(&mut self, block: &B) -> Result<GuestAddr, Fault>;
    fn next_pc(&self) -> GuestAddr;
    fn set_pc(&mut self, pc: GuestAddr);
}
```

### vm-mem::tlb_manager - TLB 实现
**文件:** `vm-mem/src/tlb_manager.rs` (150 行)  
**用途:** 标准 TLB 管理器实现

```rust
// 导入
use vm_mem::tlb_manager::StandardTlbManager;

// 关键类型
pub struct StandardTlbManager {
    // 私有: HashMap + LRU 缓存
}

impl TlbManager for StandardTlbManager {
    // O(1) 查找性能
    // ASID 感知刷新
    // 统计跟踪
}

// 使用
let mut tlb = StandardTlbManager::new(1024);
if let Some(entry) = tlb.lookup(va, AccessType::Read, asid) {
    // 处理缓存命中
}
```

### vm-mem::page_table_walker - 页表遍历
**文件:** `vm-mem/src/page_table_walker.rs` (210 行)  
**用途:** RISC-V Sv39/Sv48 页表遍历

```rust
// 导入
use vm_mem::page_table_walker::{Sv39PageTableWalker, Sv48PageTableWalker};

// 关键类型
pub struct Sv39PageTableWalker { /* 3-level paging */ }
pub struct Sv48PageTableWalker { /* 4-level paging */ }

impl PageTableWalker for Sv39PageTableWalker {
    fn walk(&mut self, addr: GuestAddr, access: AccessType, asid: u16) 
        -> Result<(GuestPhysAddr, u8), Fault> {
        // VPN 提取 → PTE 查询 → 权限检查 → 超级页处理
    }
}
```

### vm-frontend-x86_64::prefix_decode - 前缀解码
**文件:** `vm-frontend-x86_64/src/prefix_decode.rs` (110 行)  
**用途:** 解析 x86-64 指令前缀

```rust
// 导入
use vm_frontend_x86_64::prefix_decode::{PrefixInfo, RexPrefix, decode_prefixes};

// 关键类型
pub struct PrefixInfo {
    pub lock: bool,
    pub rep: bool,
    pub repne: bool,
    pub seg: Option<u8>,
    pub op_size: bool,
    pub addr_size: bool,
    pub rex: Option<RexPrefix>,
}

pub struct RexPrefix {
    pub w: bool,  // 64-bit operand
    pub r: bool,  // Reg extension
    pub x: bool,  // Index extension
    pub b: bool,  // Base/Rm extension
}

// 使用
let (prefix_info, opcode) = decode_prefixes(|| /* byte reader */)?;
println!("REX.W: {}", prefix_info.rex.map(|r| r.w).unwrap_or(false));
```

### vm-frontend-x86_64::opcode_decode - 操作码解码
**文件:** `vm-frontend-x86_64/src/opcode_decode.rs` (180 行)  
**用途:** 识别指令并确定操作数模式

```rust
// 导入
use vm_frontend_x86_64::opcode_decode::{OpcodeInfo, OperandKind, decode_opcode};

// 关键类型
#[derive(Debug)]
pub struct OpcodeInfo {
    pub mnemonic: &'static str,
    pub is_two_byte: bool,
    pub opcode_byte: u8,
    pub op1_kind: OperandKind,
    pub op2_kind: OperandKind,
    pub op3_kind: OperandKind,
    pub requires_modrm: bool,
}

pub enum OperandKind {
    None, Reg, Rm, Imm8, Imm32, Imm64, Rel8, Rel32,
    OpReg, XmmReg, XmmRm, Moffs,
}

// 使用
if let Some(info) = decode_opcode(0x89, &prefix, false)? {
    println!("Mnemonic: {}", info.mnemonic);  // "mov"
    println!("Op1: {:?}", info.op1_kind);      // Rm
}
```

### vm-frontend-x86_64::operand_decode - 操作数解码
**文件:** `vm-frontend-x86_64/src/operand_decode.rs` (260 行)  
**用途:** 解析 ModR/M、SIB 和操作数

```rust
// 导入
use vm_frontend_x86_64::operand_decode::{
    Operand, OperandDecoder, ModRM, SIB, MemoryOperand
};

// 关键类型
pub struct ModRM {
    pub mode: u8,  // 0-3
    pub reg: u8,   // 0-7 (+ REX.r)
    pub rm: u8,    // 0-7 (+ REX.b)
}

pub struct SIB {
    pub scale: u8,  // 00-11 (×1,2,4,8)
    pub index: u8,  // 0-7 (+ REX.x)
    pub base: u8,   // 0-7 (+ REX.b)
}

pub enum MemoryOperand {
    Direct { base: u8, disp: i64 },
    Indexed { base: Option<u8>, index: u8, scale: u8, disp: i64 },
    Rip { disp: i32 },
}

pub enum Operand {
    None,
    Reg { reg: u8, size: u8 },
    Xmm { reg: u8 },
    Memory { addr: MemoryOperand, size: u8 },
    Immediate { value: i64, size: u8 },
    Relative { offset: i32 },
}

// 使用
let mut decoder = OperandDecoder::new(bytes);
let op = decoder.decode_operand(OperandKind::Rm, Some(modrm), &prefix, 8)?;
```

### vm-engine-jit::jit_helpers - JIT 助手库
**文件:** `vm-engine-jit/src/jit_helpers.rs` (270 行)  
**用途:** 消除 JIT 代码的重复操作

```rust
// 导入
use vm_engine_jit::{RegisterHelper, FloatRegHelper, MemoryHelper};
use cranelift::prelude::*;

// RegisterHelper - 寄存器操作
pub struct RegisterHelper;
impl RegisterHelper {
    pub fn load_reg(...) -> Value;
    pub fn store_reg(...);
    pub fn binary_op(...);      // 加载 + 操作 + 存储
    pub fn binary_op_imm(...);
    pub fn shift_op(...);
    pub fn compare_op(...);
    pub fn unary_op(...);
}

// 使用示例
RegisterHelper::binary_op(&mut builder, regs_ptr, dst, src1, src2, |b, v1, v2| {
    b.ins().iadd(v1, v2)
});

// FloatRegHelper - 浮点寄存器
pub struct FloatRegHelper;
impl FloatRegHelper {
    pub fn load_freg(...) -> Value;
    pub fn store_freg(...);
    pub fn binary_op(...);
    pub fn unary_op(...);
    pub fn convert_from_reg(...);
    pub fn convert_to_reg(...);
}

// MemoryHelper - 内存操作
pub struct MemoryHelper;
impl MemoryHelper {
    pub fn compute_address(...) -> Value;
    pub fn compute_scaled_address(...) -> Value;
    pub fn load_with_size(...) -> Value;
    pub fn store_with_size(...);
    pub fn load_sext(...) -> Value;
    pub fn load_zext(...) -> Value;
}

// 使用示例
let base_val = RegisterHelper::load_reg(&mut builder, regs_ptr, base_reg);
let addr = MemoryHelper::compute_address(&mut builder, base_val, offset);
```

---

## 📊 性能特征

| 组件 | 性能 | 说明 |
|------|------|------|
| TlbManager::lookup | O(1) | 哈希表查询 |
| PageTableWalker::walk | O(levels) | 3-4 级页表遍历 |
| prefix_decode | O(n) | n = 前缀字节数 (1-2) |
| opcode_decode | O(1) | 表查询 |
| operand_decode | O(1) | ModR/M 解析 |
| RegisterHelper | O(1) | 内联编译 |

---

## 🧪 单元测试

### TLB 测试
```bash
cargo test --package vm-mem --lib tlb_manager::tests
# 测试: lookup, miss, flush_asid
```

### 页表测试
```bash
cargo test --package vm-mem --lib page_table_walker::tests
# 测试: Sv39, Sv48, 权限检查
```

### 前缀解码测试
```bash
cargo test --package vm-frontend-x86_64 --lib prefix_decode::tests
# 测试: no_prefix, lock, rex, segment, rep
```

### 操作数解码测试
```bash
cargo test --package vm-frontend-x86_64 --lib operand_decode::tests
# 测试: modrm, sib, imm, rel32
```

---

## 🔗 依赖关系图

```
vm-core
  ├─ domain.rs (TlbManager, PageTableWalker trait)
  └─ [其他模块]

vm-mem
  ├─ lib.rs (导入 domain traits)
  ├─ tlb_manager.rs (实现 TlbManager trait)
  └─ page_table_walker.rs (实现 PageTableWalker trait)

vm-frontend-x86_64
  ├─ lib.rs (导出所有解码器)
  ├─ prefix_decode.rs (前缀解析)
  ├─ opcode_decode.rs (依赖 prefix_decode 类型)
  └─ operand_decode.rs (依赖 opcode_decode 类型)

vm-engine-jit
  ├─ lib.rs (导出 jit_helpers)
  └─ jit_helpers.rs (助手函数)
```

---

## 🎯 集成指南

### 在新代码中使用这些模块

**1. 使用 TLB 管理器**
```rust
use vm_mem::tlb_manager::StandardTlbManager;
use vm_core::domain::TlbManager;

let mut tlb = StandardTlbManager::new(512);
if let Some(entry) = tlb.lookup(va, AccessType::Read, asid) {
    let pa = entry.phys_addr;
}
```

**2. 使用页表遍历器**
```rust
use vm_mem::page_table_walker::Sv39PageTableWalker;
use vm_core::domain::PageTableWalker;

let mut walker = Sv39PageTableWalker::new(mmu);
let (pa, flags) = walker.walk(va, AccessType::Read, asid)?;
```

**3. 使用 x86-64 解码器**
```rust
use vm_frontend_x86_64::prefix_decode::decode_prefixes;
use vm_frontend_x86_64::opcode_decode::decode_opcode;

let (prefix, opcode) = decode_prefixes(/* byte reader */)?;
let info = decode_opcode(opcode, &prefix, false)?;
```

**4. 使用 JIT 助手**
```rust
use vm_engine_jit::RegisterHelper;

RegisterHelper::binary_op(&mut builder, regs_ptr, dst, src1, src2, |b, v1, v2| {
    b.ins().iadd(v1, v2)
});
```

---

## ✅ 快速验证清单

- ✅ 所有模块编译无错
- ✅ 所有单元测试通过
- ✅ 所有公共 API 有文档
- ✅ 符合 Rust 最佳实践
- ✅ 零成本抽象设计
- ✅ 向后兼容性保证

---

## 🚀 后续任务

| 任务 | 状态 | 优先级 |
|------|------|--------|
| 1.5 替换 unwrap() | ⏳ | 高 |
| 1.6 统一解码器接口 | ⏳ | 高 |
| Phase 2 性能优化 | ⏳ | 中 |

---

**快速参考版本:** 1.0  
**最后更新:** Phase 1 完成  
**维护者:** GitHub Copilot
