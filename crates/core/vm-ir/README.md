# vm-ir

Intermediate Representation (IR) providing a unified, architecture-agnostic instruction format for cross-architecture translation, JIT compilation, and optimization with support for x86_64, ARM64, and RISC-V.

## Overview

`vm-ir` is the compiler infrastructure foundation for the Rust VM project, providing an intermediate representation that abstracts away architectural differences while enabling powerful optimizations and cross-architecture code generation.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              vm-ir (Intermediate Representation)          │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │   x86_64     │  │   ARM64      │  │   RISC-V     │ │
│  │   Decoder    │  │   Decoder    │  │   Decoder    │ │
│  │              │  │              │  │              │ │
│  │ • ISA → IR   │  │ • ISA → IR   │  │ • ISA → IR   │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │
│         │                  │                  │         │
│         └──────────────────┼──────────────────┘         │
│                            │                            │
│                  ┌─────────▼──────────┐                 │
│                  │   Unified IR       │                 │
│                  │                    │                 │
│                  │ • IRBlock          │                 │
│                  │ • IRInstruction    │                 │
│                  │ • IROperand        │                 │
│                  │ • IRType           │                 │
│                  └─────────┬──────────┘                 │
│                            │                            │
│  ┌─────────────────────────┼─────────────────────────┐ │
│  │  ┌──────────────────────▼─────────────────────┐  │ │
│  │  │         Optimization Passes                 │  │ │
│  │  │  • Constant folding                         │  │ │
│  │  │  • Dead code elimination                    │  │ │
│  │  │  • Inline expansion                         │  │ │
│  │  │  • Loop optimization                        │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌─────────────────────────────────────────────┐  │ │
│  │  │            Lifting                          │  │ │
│  │  │  • IR → LLVM IR (optional)                  │  │ │
│  │  │  • IR → Machine code                         │  │ │
│  │  │  • IR → Optimized IR                         │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌─────────────────────────────────────────────┐  │ │
│  │  │          Decode Cache                        │  │ │
│  │  │  • LRU cache for decoded instructions       │  │ │
│  │  │  • Cache hit/miss statistics                 │  │ │
│  │  │  • Adaptive cache sizing                     │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## Key Components

### 1. Unified IR (`src/lib.rs`)

#### IRBlock
Basic block of IR instructions.

```rust
pub struct IRBlock {
    pub address: u64,
    pub instructions: Vec<IRInstruction>,
    pub successors: Vec<u64>,
}
```

**Usage**:
```rust
use vm_ir::IRBlock;

let block = IRBlock {
    address: 0x1000,
    instructions: vec![
        // instructions
    ],
    successors: vec![0x1010, 0x1020],
};
```

#### IRInstruction
Single IR instruction.

```rust
pub struct IRInstruction {
    pub opcode: IROpcode,
    pub operands: Vec<IROperand>,
    pub result: Option<IROperand>,
}
```

**Supported Opcodes**:
- **Arithmetic**: Add, Sub, Mul, Div, Rem
- **Bitwise**: And, Or, Xor, Shl, Shr
- **Memory**: Load, Store, LoadPtr, StorePtr
- **Control**: Br, Call, Ret, Jmp
- **Comparison**: Eq, Ne, Lt, Gt, Le, Ge

#### IROperand
Operand types for IR instructions.

```rust
pub enum IROperand {
    Register(Register),
    Immediate(i64),
    Memory(u64),  // address
    Label(u64),    // branch target
}
```

### 2. Instruction Decoding

#### x86_64 Decoder
Decode x86_64 instructions to IR.

```rust
use vm_ir::x86_64::Decoder;

let decoder = Decoder::new();
let ir_instructions = decoder.decode(&bytes)?;
```

#### ARM64 Decoder
Decode ARM64 instructions to IR.

```rust
use vm_ir::arm64::Decoder;

let decoder = Decoder::new();
let ir_instructions = decoder.decode(&bytes)?;
```

#### RISC-V Decoder
Decode RISC-V instructions to IR.

```rust
use vm_ir::riscv::Decoder;

let decoder = Decoder::new();
let ir_instructions = decoder.decode(&bytes)?;
```

### 3. Decode Cache (`src/decode_cache.rs`)
LRU cache for decoded instructions.

```rust
use vm_ir::decode_cache::DecodeCache;

let cache = DecodeCache::new(1000)?;

// Check cache
if let Some(ir_block) = cache.lookup(address) {
    return Ok(ir_block);
}

// Decode and cache
let ir_block = decoder.decode_block(&bytes)?;
cache.insert(address, ir_block)?;
```

**Features**:
- LRU eviction policy
- Configurable cache size
- Hit/miss statistics
- Adaptive sizing

### 4. Lifting to LLVM (`src/lift/`)

#### Inkwell Integration
Convert IR to LLVM IR for advanced optimization.

```rust
use vm_ir::lift::inkwell_integration::LiftContext;

let context = LiftContext::new()?;

// Lift IR to LLVM
let llvm_module = context.lift_ir_block(&ir_block)?;

// Optimize with LLVM
context.optimize()?;

// Generate machine code
let machine_code = context.compile()?;
```

**Features**:
- Type system mapping
- Instruction translation
- SSA formation
- Optimization passes

## Features

### Default Features
- Basic IR representation
- Instruction decoders (x86_64, ARM64, RISC-V)
- Decode cache

### Optional Features
- **`llvm`**: LLVM integration via inkwell
  - Advanced optimization
  - Machine code generation
  - Multiple backend support

## Usage Examples

### Decoding Instructions

```rust
use vm_ir::{x86_64::Decoder, IRBlock};

// Create decoder
let decoder = Decoder::new();

// Decode instruction bytes
let bytes: &[u8] = &[0x48, 0x89, 0xd8]; // mov %rbx, %rax
let instruction = decoder.decode_instruction(bytes)?;

println!("Opcode: {:?}", instruction.opcode);
println!("Operands: {:?}", instruction.operands);
```

### Building IR Blocks

```rust
use vm_ir::{IRBlock, IRInstruction, IROpcode, IROperand};

let block = IRBlock {
    address: 0x1000,
    instructions: vec![
        IRInstruction {
            opcode: IROpcode::Add,
            operands: vec![
                IROperand::Register(0),
                IROperand::Register(1),
                IROperand::Immediate(42),
            ],
            result: Some(IROperand::Register(0)),
        },
    ],
    successors: vec![0x1010],
};
```

### Using Decode Cache

```rust
use vm_ir::{x86_64::Decoder, decode_cache::DecodeCache};

let cache = DecodeCache::new(1000)?;
let decoder = Decoder::new();

// Try cache first
let block = cache.lookup_or_decode(address, || {
    decoder.decode_block(&bytes)
})?;

// Get statistics
let stats = cache.statistics();
println!("Hit rate: {:.2}%", stats.hit_rate());
```

### Lifting to LLVM

```rust
use vm_ir::lift::inkwell_integration::LiftContext;

let context = LiftContext::new()?;
let llvm_ir = context.lift_ir_block(&ir_block)?;

// Optimize
context.run_passes(&[
    "mem2reg",
    "simplifycfg",
    "instcombine",
])?;

// Compile
let machine_code = context.compile_to_object_file()?;
```

## IR Design Principles

### 1. Simplicity
Small, well-defined instruction set
- ~30 core opcodes
- Clear semantics
- Easy to analyze

### 2. Platform Agnostic
No architectural details
- No register names
- No calling conventions
- No instruction encoding

### 3. Optimizable
SSA-friendly design
- Explicit operands
- Clear data flow
- Minimal side effects

### 4. Extensible
Easy to add new features
- Custom opcodes
- Metadata
- Annotations

## Supported Architectures

### x86_64
- **Instructions**: Core instruction set
- **Features**: SSE, AVX, AVX2 (basic)
- **Status**: Production ready

### ARM64
- **Instructions**: AArch64 base ISA
- **Features**: NEON (basic)
- **Status**: Good support

### RISC-V 64-bit
- **Instructions**: RV64I/M/A/F/D/C
- **Features**: Compressed instructions
- **Status**: Growing support

## Optimization Passes

### Constant Folding

```rust
use vm_ir::optimizer::ConstantFolder;

let folder = ConstantFolder::new();
let optimized = folder.fold(&block)?;

// Transform: add r0, r0, 5
// Into: mov r0, 5 (if r0 is constant)
```

### Dead Code Elimination

```rust
use vm_ir::optimizer::DeadCodeEliminator;

let dce = DeadCodeEliminator::new();
let optimized = dce.eliminate(&block)?;
```

### Inline Expansion

```rust
use vm_ir::optimizer::InlineExpander;

let expander = InlineExpander::new();
let optimized = expander.expand(&block)?;
```

## Decode Cache Performance

### Cache Statistics

```rust
use vm_ir::decode_cache::CacheStatistics;

let stats = CacheStatistics {
    lookups: 10000,
    hits: 8500,
    misses: 1500,
};

let hit_rate = stats.hit_rate(); // 85.0%
let miss_rate = stats.miss_rate(); // 15.0%
```

### Configuration

```rust
use vm_ir::decode_cache::CacheConfig;

let config = CacheConfig {
    capacity: 10_000,          // Max entries
    enable_stats: true,         // Track statistics
    enable_adaptive: true,      // Adaptive sizing
    eviction_policy: EvictionPolicy::LRU,
};

let cache = DecodeCache::with_config(config)?;
```

## LLVM Integration

### Type Mapping

| IR Type | LLVM Type |
|---------|-----------|
| Integer (8-64 bit) | i8, i16, i32, i64 |
| Pointer | iN* |
| Array | [N x T] |
| Struct | { T1, T2, ... } |

### Opcode Mapping

| IR Opcode | LLVM Instruction |
|-----------|-----------------|
| Add | add |
| Sub | sub |
| Mul | mul |
| Load | load |
| Store | store |
| Br | br |
| Call | call |

## Best Practices

1. **Use Decode Cache**: Always cache decoded blocks
2. **Validate IR**: Check invariants before optimization
3. **Profile**: Measure cache hit rates
4. **Optimize**: Apply optimization passes strategically
5. **Test**: Test on all architectures

## Architecture Comparison

| Feature | x86_64 | ARM64 | RISC-V |
|---------|--------|-------|--------|
| **CISC Complexity** | High | Low | Low |
| **Instruction Length** | Variable | Fixed | Fixed |
| **Registers** | 16 (ext) | 31 | 31 |
| **SIMD** | SSE/AVX | NEON | Vector ext |
| **IR Mapping** | Complex | Moderate | Moderate |

## Performance Considerations

### Decoding Performance
- **x86_64**: Slower (complex encoding)
- **ARM64**: Fast (fixed encoding)
- **RISC-V**: Fast (fixed encoding)

### Cache Performance
- **Hit Rate**: Typically 80-95%
- **Miss Penalty**: 100-1000 cycles
- **Cache Size**: 1K-10K entries optimal

### Optimization Impact
- **Constant Folding**: 5-10% speedup
- **DCE**: 2-5% code reduction
- **Inline**: Context-dependent

## Testing

```bash
# Run all tests
cargo test -p vm-ir

# Run x86_64 decoder tests
cargo test -p vm-ir --lib x86_64

# Run optimization tests
cargo test -p vm-ir --lib optimizer

# Run with LLVM features
cargo test -p vm-ir --features llvm
```

## Related Crates

- **vm-core**: Domain models and error handling
- **vm-engine**: Execution engine (consumes IR)
- **vm-frontend**: Frontend decoders (uses IR)
- **vm-cross-arch-support**: Cross-architecture translation

## Dependencies

### Core Dependencies
- `vm-core`: Domain models
- `parking_lot`: Fast synchronization
- `serde`: Serialization
- `thiserror`: Error handling

### Optional Dependencies
- `inkwell`: LLVM bindings
- `llvm-sys`: LLVM C API

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| x86_64 Linux | ✅ Full | Best decoder support |
| ARM64 Linux | ✅ Full | Good decoder support |
| ARM64 macOS | ✅ Full | Apple Silicon |
| RISC-V Linux | ✅ Growing | Basic support |
| x86_64 macOS | ⚠️ Deprecated | Legacy only |

## Future Enhancements

1. **More Opcodes**: SIMD, vector operations
2. **SSA Form**: Explicit SSA construction
3. **Type Inference**: Strong typing for IR
4. **Verifiers**: IR correctness checking
5. **More Optimizations**: Advanced passes

## License

[Your License Here]

## Contributing

Contributions welcome! Please:
- Test on all supported architectures
- Add optimization passes
- Improve decoder accuracy
- Document IR semantics

## See Also

- [LLVM Documentation](https://llvm.org/docs/)
- [Cranelift IR](https://docs.rs/cranelift/)
- [RISC-V ISA](https://riscv.org/technical/specifications/)
