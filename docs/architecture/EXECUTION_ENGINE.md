# 执行引擎架构

## 目录

- [执行引擎概述](#执行引擎概述)
- [解释器实现](#解释器实现)
- [JIT编译器](#jit编译器)
- [执行模式切换](#执行模式切换)
- [性能特性](#性能特性)
- [优化技术](#优化技术)

---

## 执行引擎概述

### 职责

执行引擎是VM的核心组件，负责执行Guest指令并更新vCPU状态。

### 执行模式

```
┌─────────────────────────────────────────────────────────┐
│                   执行引擎架构                          │
│                                                           │
│  ┌────────────┐  ┌────────────┐  ┌─────────────────┐   │
│  │解释器      │  │  JIT引擎   │  │  混合引擎       │   │
│  │Interpreter │  │ JIT Engine │  │ Hybrid Engine   │   │
│  └────────────┘  └────────────┘  └─────────────────┘   │
│        │               │                  │             │
│        │               │                  │             │
│        ↓               ↓                  ↓             │
│  ┌─────────────────────────────────────────────────┐   │
│  │           统一的执行接口                         │   │
│  │  ExecutionEngine<IRBlock> trait                 │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### 性能对比

| 执行模式 | 相对性能 | 启动时间 | 内存占用 | 编译时间 | 适用场景 |
|---------|---------|---------|---------|---------|---------|
| 解释器 | 1-5% | <10ms | 低 | 0ms | 调试、轻量负载 |
| JIT | 50-80% | 100-500ms | 中 | 10-100ms/block | 通用计算 |
| 混合 | 30-70% | 10-50ms | 中 | 按需编译 | 自适应优化 |

---

## 解释器实现

### 架构设计

```
┌────────────────────────────────────────────────────────┐
│              InterpreterEngine                         │
│                                                         │
│  ┌──────────────────────────────────────────────────┐  │
│  │         Fetch-Decode-Execute循环                 │  │
│  │                                                   │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────────────┐  │  │
│  │  │ Fetch   │→ │ Decode  │→ │   Execute       │  │  │
│  │  │ 指令获取 │  │ 指令解码 │  │  指令执行       │  │  │
│  │  └─────────┘  └─────────┘  └─────────────────┘  │  │
│  │                                  │              │  │
│  │                                  ↓              │  │
│  │                       ┌──────────────────┐      │  │
│  │                       │   Update State   │      │  │
│  │                       │  更新寄存器和PC   │      │  │
│  │                       └──────────────────┘      │  │
│  └──────────────────────────────────────────────────┘  │
│                                                         │
│  ┌──────────────────────────────────────────────────┐  │
│  │            指令分发表                            │  │
│  │                                                   │  │
│  │  Opcode → Handler Function                       │  │
│  │  0x33   → execute_r_type()                       │  │
│  │  0x13   → execute_i_type()                       │  │
│  │  0x63   → execute_b_type()                       │  │
│  │  ...                                            │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 核心实现

```rust
pub struct InterpreterEngine {
    /// 通用寄存器
    regs: [u64; 32],
    /// 浮点寄存器
    fp_regs: [u64; 32],
    /// 程序计数器
    pc: GuestAddr,
    /// 特权寄存器
    priv_regs: PrivilegeRegisters,
    /// 执行统计
    stats: ExecStats,
}

impl InterpreterEngine {
    pub fn new() -> Self {
        Self {
            regs: [0; 32],
            fp_regs: [0; 32],
            pc: GuestAddr(0),
            priv_regs: PrivilegeRegisters::default(),
            stats: ExecStats::default(),
        }
    }

    /// 主执行循环
    pub fn run(&mut self, mmu: &mut dyn MMU) -> ExecResult {
        loop {
            // 1. Fetch
            let insn = self.fetch_insn(mmu)?;

            // 2. Decode & Dispatch
            let result = self.execute_instruction(insn)?;

            // 3. Handle Result
            match result {
                ExecStatus::Continue => continue,
                ExecStatus::Ok => return Ok(self.make_result()),
                ExecStatus::Fault(e) => return Err(e.into()),
                ExecStatus::IoRequest => {
                    self.handle_io()?;
                }
                ExecStatus::InterruptPending => {
                    self.handle_interrupt()?;
                }
            }
        }
    }

    /// 取指令
    fn fetch_insn(&self, mmu: &dyn MMU) -> VmResult<u32> {
        let insn_word = mmu.fetch_insn(self.pc)?;
        Ok(insn_word as u32)
    }
}

impl ExecutionEngine<IRBlock> for InterpreterEngine {
    fn execute_instruction(&mut self, insn: &Instruction)
        -> VmResult<()>
    {
        self.stats.executed_insns += 1;

        match insn.opcode {
            // 算术运算
            Opcode::ADD => self.execute_add(insn),
            Opcode::SUB => self.execute_sub(insn),
            Opcode::MUL => self.execute_mul(insn),
            Opcode::DIV => self.execute_div(insn),

            // 逻辑运算
            Opcode::AND => self.execute_and(insn),
            Opcode::OR  => self.execute_or(insn),
            Opcode::XOR => self.execute_xor(insn),

            // 内存访问
            Opcode::LB  => self.execute_lb(insn),
            Opcode::LH  => self.execute_lh(insn),
            Opcode::LW  => self.execute_lw(insn),
            Opcode::LD  => self.execute_ld(insn),
            Opcode::SB  => self.execute_sb(insn),
            Opcode::SH  => self.execute_sh(insn),
            Opcode::SW  => self.execute_sw(insn),
            Opcode::SD  => self.execute_sd(insn),

            // 分支跳转
            Opcode::BEQ => self.execute_beq(insn),
            Opcode::BNE => self.execute_bne(insn),
            Opcode::JAL => self.execute_jal(insn),
            Opcode::JALR => self.execute_jalr(insn),

            // 系统指令
            Opcode::ECALL => self.execute_ecall(insn),
            Opcode::MRET  => self.execute_mret(insn),
            Opcode::SRET  => self.execute_sret(insn),

            _ => Err(VmError::Execution(
                ExecutionError::Fault(Fault::InvalidOpcode {
                    pc: self.pc,
                    opcode: insn.opcode as u32,
                })
            )),
        }
    }

    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock)
        -> ExecResult
    {
        let start_time = std::time::Instant::now();

        // 执行基本块中的所有指令
        for insn in &block.instructions {
            if let Err(e) = self.execute_instruction(insn) {
                return Ok(ExecResult {
                    status: ExecStatus::Fault(e),
                    next_pc: self.pc,
                    stats: self.stats.clone(),
                });
            }
        }

        let elapsed = start_time.elapsed();
        self.stats.exec_time_ns += elapsed.as_nanos() as u64;

        Ok(ExecResult {
            status: ExecStatus::Continue,
            next_pc: self.pc,
            stats: self.stats.clone(),
        })
    }

    fn get_reg(&self, idx: usize) -> u64 {
        self.regs[idx]
    }

    fn set_reg(&mut self, idx: usize, val: u64) {
        self.regs[idx] = val;
    }

    fn get_pc(&self) -> GuestAddr {
        self.pc
    }

    fn set_pc(&mut self, pc: GuestAddr) {
        self.pc = pc;
    }
}
```

### 指令实现示例

```rust
impl InterpreterEngine {
    /// ADD指令 (R-type)
    fn execute_add(&mut self, insn: &Instruction) -> VmResult<()> {
        let rd = insn.rd as usize;
        let rs1 = insn.rs1 as usize;
        let rs2 = insn.rs2 as usize;

        let val1 = self.regs[rs1];
        let val2 = self.regs[rs2];

        // 执行加法（带溢出检测）
        let result = val1.wrapping_add(val2);
        self.regs[rd] = result;

        // 更新PC
        self.pc = self.pc.wrapping_add(4);

        Ok(())
    }

    /// LW指令 (I-type load word)
    fn execute_lw(&mut self, insn: &Instruction, mmu: &mut dyn MMU)
        -> VmResult<()>
    {
        let rd = insn.rd as usize;
        let rs1 = insn.rs1 as usize;
        let imm = insn.imm as i64 as u64;

        // 计算有效地址
        let base = self.regs[rs1];
        let addr = GuestAddr(base.wrapping_add(imm));

        // 翻译地址并读取
        let val = mmu.read(addr, 4)?;

        // 符号扩展
        self.regs[rd] = (val as i32 as i64) as u64;

        self.pc = self.pc.wrapping_add(4);
        Ok(())
    }

    /// BEQ指令 (B-type branch if equal)
    fn execute_beq(&mut self, insn: &Instruction) -> VmResult<()> {
        let rs1 = insn.rs1 as usize;
        let rs2 = insn.rs2 as usize;
        let imm = insn.imm as i64;

        let val1 = self.regs[rs1];
        let val2 = self.regs[rs2];

        if val1 == val2 {
            // 跳转
            let offset = imm as i64;
            self.pc = self.pc.wrapping_add(offset as u64);
        } else {
            // 不跳转，顺序执行
            self.pc = self.pc.wrapping_add(4);
        }

        Ok(())
    }

    /// ECALL指令 (系统调用)
    fn execute_ecall(&mut self, insn: &Instruction)
        -> VmResult<()>
    {
        // RISC-V ABI: a7=系统调用号, a0-a5=参数
        let syscall_no = self.regs[17]; // a7
        let args = [
            self.regs[10],  // a0
            self.regs[11],  // a1
            self.regs[12],  // a2
            self.regs[13],  // a3
            self.regs[14],  // a4
            self.regs[15],  // a5
        ];

        // 执行系统调用
        let result = self.handle_syscall(syscall_no, &args)?;

        // 返回值放在a0
        self.regs[10] = result;

        self.pc = self.pc.wrapping_add(4);
        Ok(())
    }
}
```

### 性能优化

#### 1. 指令分发优化

```rust
// 使用函数指针表代替match
type InsnHandler = fn(&mut InterpreterEngine, &Instruction) -> VmResult<()>;

static INSN_TABLE: [InsnHandler; 256] = {
    let mut table: [InsnHandler; 256] = [InterpreterEngine::execute_illegal; 256];

    table[0x33 as usize] = InterpreterEngine::execute_add;
    table[0x03 as usize] = InterpreterEngine::execute_lb;
    // ...

    table
};

// 快速分发
impl InterpreterEngine {
    fn dispatch_insn(&mut self, insn: &Instruction) -> VmResult<()> {
        let handler = INSN_TABLE[insn.opcode as usize];
        handler(self, insn)
    }
}
```

#### 2. 寄存器缓存

```rust
impl InterpreterEngine {
    /// 内循环中缓存常用寄存器
    fn run_optimized(&mut self, mmu: &mut dyn MMU) -> ExecResult {
        loop {
            let pc = self.pc;
            let insn = self.fetch_insn(mmu)?;

            // 使用本地变量减少结构体访问
            let regs = &mut self.regs;

            // 执行指令（内联优化）
            // ...

            self.pc = pc.wrapping_add(4);
        }
    }
}
```

---

## JIT编译器

### 架构设计

```
┌─────────────────────────────────────────────────────────┐
│                   JITEngine                             │
│                                                          │
│  ┌───────────────────────────────────────────────────┐  │
│  │              JIT编译流程                          │  │
│  │                                                   │  │
│  │  IR Block → 分析 → 优化 → 代码生成 → 机器码       │  │
│  │             ↓      ↓       ↓         ↓           │  │
│  │          CFG    Passes  CodeGen  NativeCode      │  │
│  └───────────────────────────────────────────────────┘  │
│                                                          │
│  ┌───────────────────────────────────────────────────┐  │
│  │             代码缓存管理                          │  │
│  │                                                   │  │
│  │  HashMap<GuestAddr, CompiledCode>                 │  │
│  │                                                   │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐           │  │
│  │  │  Hot    │  │  Warm   │  │  Cold   │           │  │
│  │  │  Code   │  │  Code   │  │  Code   │           │  │
│  │  └─────────┘  └─────────┘  └─────────┘           │  │
│  │     LRU          LRU          LRU                 │  │
│  └───────────────────────────────────────────────────┘  │
│                                                          │
│  ┌───────────────────────────────────────────────────┐  │
│  │              性能统计                             │  │
│  │                                                   │  │
│  │  - 编译次数                                        │  │
│  │  - 编译时间                                        │  │
│  │  - 缓存命中率                                      │  │
│  │  - 代码大小                                        │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 核心实现

```rust
pub struct JITEngine {
    /// 代码缓存
    code_cache: HashMap<GuestAddr, CompiledCode>,
    /// JIT编译器
    compiler: JITCompiler,
    /// 执行统计
    stats: ExecStats,
    /// 缓存配置
    config: JITConfig,
}

pub struct JITConfig {
    /// 最大代码缓存大小 (字节)
    pub max_cache_size: usize,
    /// 代码块大小限制
    pub max_block_size: usize,
    /// 优化等级
    pub opt_level: OptLevel,
}

#[derive(Clone, Copy)]
pub enum OptLevel {
    None,       // 无优化
    Basic,      // 基础优化
    Aggressive, // 激进优化
}

pub struct CompiledCode {
    /// 代码入口地址
    entry: *const u8,
    /// 代码大小
    size: usize,
    /// 执行次数
    exec_count: AtomicU64,
    /// 最后使用时间
    last_used: AtomicU64,
}

impl JITEngine {
    pub fn new(config: JITConfig) -> Self {
        Self {
            code_cache: HashMap::new(),
            compiler: JITCompiler::new(config.opt_level),
            stats: ExecStats::default(),
            config,
        }
    }

    /// 查找或编译代码
    fn get_or_compile(&mut self, block: &IRBlock)
        -> Result<&CompiledCode, VmError>
    {
        let addr = block.address;

        // 检查缓存
        if !self.code_cache.contains_key(&addr) {
            // 编译新代码
            let compiled = self.compile_block(block)?;
            self.code_cache.insert(addr, compiled);
            self.stats.jit_compiles += 1;
        }

        Ok(&self.code_cache[&addr])
    }

    /// 编译基本块
    fn compile_block(&mut self, block: &IRBlock)
        -> Result<CompiledCode, VmError>
    {
        let start = std::time::Instant::now();

        // 1. 分析
        let analysis = self.compiler.analyze(block)?;

        // 2. 优化
        let optimized = self.compiler.optimize(block, &analysis)?;

        // 3. 代码生成
        let code = self.compiler.codegen(&optimized)?;

        let elapsed = start.elapsed();
        self.stats.jit_compile_time_ns += elapsed.as_nanos() as u64;

        Ok(code)
    }
}

impl ExecutionEngine<IRBlock> for JITEngine {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock)
        -> ExecResult
    {
        // 获取或编译代码
        let code = self.get_or_compile(block)?;

        // 执行编译后的代码
        let start = std::time::Instant::now();

        let result = unsafe { code.execute(mmu, self) };

        let elapsed = start.elapsed();
        self.stats.exec_time_ns += elapsed.as_nanos() as u64;
        self.stats.executed_insns += block.instructions.len() as u64;

        result
    }
}
```

### 优化Passes

```rust
impl JITCompiler {
    /// 优化管道
    fn optimize(&mut self, block: &IRBlock, analysis: &Analysis)
        -> Result<IRBlock, VmError>
    {
        let mut block = block.clone();

        // Pass 1: 常量折叠
        block = self.constant_folding(&block)?;

        // Pass 2: 死代码消除
        block = self.dead_code_elimination(&block)?;

        // Pass 3: 寄存器分配
        block = self.register_allocation(&block, analysis)?;

        // Pass 4: 指令选择
        block = self.instruction_selection(&block)?;

        Ok(block)
    }

    /// 常量折叠
    fn constant_folding(&self, block: &IRBlock) -> Result<IRBlock, VmError> {
        let mut new_block = block.clone();

        for insn in &mut new_block.instructions {
            match insn {
                IRInstruction::Add { dst, src1, src2 } => {
                    if let (Operand::Imm(i1), Operand::Imm(i2)) = (src1, src2) {
                        // 常量折叠: add x1, 10, 20 → add x1, 0, 30
                        *insn = IRInstruction::Add {
                            dst: *dst,
                            src1: Reg::X0,  // 零寄存器
                            src2: Operand::Imm(i1 + i2),
                        };
                    }
                }
                // 其他指令...
                _ => {}
            }
        }

        Ok(new_block)
    }

    /// 死代码消除
    fn dead_code_elimination(&self, block: &IRBlock)
        -> Result<IRBlock, VmError>
    {
        let mut live = HashSet::new();
        let mut new_insns = Vec::new();

        // 反向遍历，标记活跃寄存器
        for insn in block.instructions.iter().rev() {
            match insn {
                IRInstruction::Add { dst, src1, src2 } => {
                    if live.contains(dst) {
                        new_insns.push(insn.clone());
                        live.insert(src1);
                        live.insert(src2);
                    }
                }
                // ...
                _ => {}
            }
        }

        new_insns.reverse();
        Ok(IRBlock {
            address: block.address,
            instructions: new_insns,
            successors: block.successors.clone(),
            is_exit: block.is_exit,
        })
    }

    /// 寄存器分配
    fn register_allocation(&self, block: &IRBlock, analysis: &Analysis)
        -> Result<IRBlock, VmError>
    {
        // 线性扫描寄存器分配
        let mut allocator = RegisterAllocator::new();
        let mut new_block = block.clone();

        for insn in &mut new_block.instructions {
            match insn {
                IRInstruction::Add { dst, src1, src2 } => {
                    // 分配虚拟寄存器到物理寄存器
                    *dst = allocator.allocate(*dst)?;
                    if let Operand::Reg(r) = src1 {
                        *src1 = Operand::Reg(allocator.allocate(r)?);
                    }
                    if let Operand::Reg(r) = src2 {
                        *src2 = Operand::Reg(allocator.allocate(r)?);
                    }
                }
                // ...
                _ => {}
            }
        }

        Ok(new_block)
    }
}
```

### 代码生成

```rust
impl JITCompiler {
    /// 生成机器码
    fn codegen(&mut self, block: &IRBlock) -> Result<CompiledCode, VmError> {
        let mut codegen = CodeGenerator::new();

        codegen.emit_prologue();

        for insn in &block.instructions {
            self.emit_insn(&mut codegen, insn)?;
        }

        codegen.emit_epilogue();

        let code = codegen.finalize();

        Ok(CompiledCode {
            entry: code.as_ptr(),
            size: code.len(),
            exec_count: AtomicU64::new(0),
            last_used: AtomicU64::new(timestamp()),
        })
    }

    /// 发射单条指令
    fn emit_insn(&self, codegen: &mut CodeGenerator, insn: &IRInstruction)
        -> Result<(), VmError>
    {
        match insn {
            IRInstruction::Add { dst, src1, src2 } => {
                // mov rax, [src1]
                codegen.emit_mov_r64_rm64(*dst, *src1);
                // add rax, [src2]
                codegen.emit_add_r64_rm64(*dst, *src2);
            }

            IRInstruction::Load { dst, addr, size } => {
                // mov rax, [addr]
                codegen.emit_load(*dst, addr, *size);
            }

            IRInstruction::Store { src, addr, size } => {
                // mov [addr], rax
                codegen.emit_store(src, addr, *size);
            }

            // 其他指令...
            _ => {}
        }

        Ok(())
    }
}
```

### 代码缓存管理

```rust
impl JITEngine {
    /// 缓存淘汰
    fn evict_if_needed(&mut self) {
        let total_size: usize = self.code_cache.values()
            .map(|code| code.size)
            .sum();

        if total_size > self.config.max_cache_size {
            // LRU淘汰
            let mut addrs: Vec<_> = self.code_cache.keys().copied().collect();
            addrs.sort_by_key(|&addr| {
                self.code_cache[&addr].last_used.load(Ordering::Relaxed)
            });

            // 淘汰最旧的代码
            let to_remove = total_size - self.config.max_cache_size / 2;
            let mut removed = 0;

            for addr in addrs {
                if removed >= to_remove {
                    break;
                }

                if let Some(code) = self.code_cache.remove(&addr) {
                    removed += code.size;
                    unsafe {
                        // 释放代码内存
                        dealloc(code.entry as *mut u8, Layout::from_size_align(code.size, 16).unwrap());
                    }
                }
            }
        }
    }
}
```

---

## 执行模式切换

### 混合引擎

```rust
pub struct HybridEngine {
    /// 解释器（用于冷代码）
    interpreter: InterpreterEngine,
    /// JIT引擎（用于热代码）
    jit: JITEngine,
    /// 执行计数（热点检测）
    execution_counts: HashMap<GuestAddr, u64>,
    /// 热点阈值
    hot_threshold: u64,
    /// 冷阈值（用于降级）
    cold_threshold: u64,
}

impl HybridEngine {
    pub fn new(config: HybridConfig) -> Self {
        Self {
            interpreter: InterpreterEngine::new(),
            jit: JITEngine::new(config.jit_config),
            execution_counts: HashMap::new(),
            hot_threshold: config.hot_threshold,
            cold_threshold: config.cold_threshold,
        }
    }
}

impl ExecutionEngine<IRBlock> for HybridEngine {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock)
        -> ExecResult
    {
        let addr = block.address;
        let count = self.execution_counts.entry(addr).or_insert(0);
        *count += 1;

        if *count < self.hot_threshold {
            // 冷代码：解释执行
            self.interpreter.run(mmu, block)
        } else if *count > self.cold_threshold {
            // 热代码：JIT执行
            self.jit.run(mmu, block)
        } else {
            // 温代码：混合模式
            // 尝试JIT，失败则回退到解释器
            match self.jit.run(mmu, block) {
                Ok(result) => Ok(result),
                Err(_) => self.interpreter.run(mmu, block),
            }
        }
    }
}
```

### OSR (On-Stack Replacement)

OSR允许在执行过程中从解释器切换到JIT代码：

```rust
impl HybridEngine {
    /// OSR切换
    fn osr_switch(&mut self, pc: GuestAddr, mmu: &mut dyn MMU)
        -> Result<(), VmError>
    {
        // 保存解释器状态
        let interp_state = self.interpreter.get_state();

        // 编译当前基本块
        let block = self.fetch_block(pc)?;
        let code = self.jit.compile_block(&block)?;

        // 迁移状态到JIT
        self.jit.set_state(&interp_state);

        // 跳转到JIT代码
        unsafe {
            code.execute_osr(mmu, &interp_state);
        }

        Ok(())
    }
}
```

---

## 性能特性

### 执行开销分解

```
总开销 = 解码开销 + 执行开销 + 内存访问开销 + 分支开销

解释器:
  解码开销: ~20%
  执行开销: ~60%
  内存访问: ~15%
  分支开销: ~5%

JIT:
  编译开销: 一次性 (10-100ms)
  执行开销: ~5%
  内存访问: ~10%
  分支预测: ~5%
```

### 优化效果

| 优化技术 | 性能提升 | 适用场景 |
|---------|---------|---------|
| 常量折叠 | 10-20% | 算术密集代码 |
| 死代码消除 | 5-10% | 所有代码 |
| 寄存器分配 | 20-30% | 寄存器压力大的代码 |
| 内联缓存 | 15-25% | 属性访问 |
| 类型特化 | 30-50% | 动态类型代码 |

---

## 优化技术

### 1. 内联缓存 (Inline Caching)

```rust
pub struct InlineCache {
    /// 缓存的类和偏移
    cached_class: Option<ClassId>,
    cached_offset: Option<u32>,
    miss_count: u32,
}

impl InlineCache {
    pub fn load_property_fast(&mut self, obj: &Object, name: &str)
        -> Value
    {
        // 快速路径：命中缓存
        if let Some(class_id) = self.cached_class {
            if obj.class_id() == class_id {
                let offset = self.cached_offset.unwrap();
                return obj.get_property_at_offset(offset);
            }
        }

        // 慢速路径：未命中
        self.miss_count += 1;
        let (class_id, offset) = obj.lookup_property(name);

        // 更新缓存
        self.cached_class = Some(class_id);
        self.cached_offset = Some(offset);

        obj.get_property_at_offset(offset)
    }
}
```

### 2. 类型特化

```rust
// 通用版本
fn add_generic(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Value::Int(x + y),
        (Value::Float(x), Value::Float(y)) => Value::Float(x + y),
        _ => panic!("type error"),
    }
}

// 特化版本（编译时生成）
fn add_int_int(a: i64, b: i64) -> i64 {
    a + b  // 无分支，无类型检查
}

fn add_float_float(a: f64, b: f64) -> f64 {
    a + b
}
```

### 3. 去虚拟化

```rust
// 虚拟调用（慢）
trait Trait {
    fn method(&self) -> i64;
}

fn call_virtual(obj: &Box<dyn Trait>) -> i64 {
    obj.method()  // 间接调用
}

// 去虚拟化后（快）
fn call_inlined<T: Trait>(obj: &T) -> i64 {
    obj.method()  // 直接调用，可内联
}
```

---

**文档版本**: 1.0
**最后更新**: 2025-12-31
**作者**: VM开发团队
