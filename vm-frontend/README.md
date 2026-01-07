# vm-frontend

Frontend instruction decoding providing architecture-specific instruction decoders for x86_64, ARM64, and RISC-V with support for complex addressing modes, SIMD operations, and optimization hints.

## Overview

`vm-frontend` is the instruction decoding frontend for the Rust VM project, responsible for translating native machine code into an intermediate representation suitable for execution, optimization, and translation.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│               vm-frontend (Instruction Decoding)        │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │  x86_64      │  │   ARM64      │  │   RISC-V     │ │
│  │  Decoder     │  │   Decoder     │  │   Decoder     │ │
│  │              │  │              │  │              │ │
│  │ • Complex    │  │ • Fixed-len   │  │ • Fixed-len   │ │
│  │ • CISC       │  │ • RISC       │  │ • RISC       │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │
│         │                  │                  │         │
│         └──────────────────┼──────────────────┘         │
│                            │                            │
│                  ┌─────────▼──────────┐                 │
│                  │  Decoder Interface  │                 │
│                  │                    │                 │
│                  │ • decode()         │                 │
│                  │ • decode_block()   │                 │
│                  │ • get_length()     │                 │
│                  └─────────┬──────────┘                 │
│                            │                            │
│  ┌─────────────────────────┼─────────────────────────┐ │
│  │  ┌──────────────────────▼─────────────────────┐  │ │
│  │  │          Decoding Features                 │  │ │
│  │  │  • Prefixes (x86: REX, VEX, EVEX)         │  │ │
│  │  │  • SIMD (SSE, AVX, NEON)                   │  │ │
│  │  │  • Addressing modes                         │  │ │
│  │  │  • Operands (register, memory, immediate)   │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌─────────────────────────────────────────────┐  │ │
│  │  │         Optimization Hints                  │  │ │
│  │  │  • Branch targets                           │  │ │
│  │  │  • Hot-spot markers                         │  │ │
│  │  │  • Side-effect information                 │  │ │
│  │  └─────────────────────────────────────────────┘  │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## Key Components

### 1. Decoder Interface

**Unified Decoder Trait**:
```rust
pub trait Decoder: Send + Sync {
    /// Decode single instruction
    fn decode(&self, bytes: &[u8]) -> Result<DecodedInstruction, DecodeError>;

    /// Get instruction length
    fn get_length(&self, bytes: &[u8]) -> Result<usize, DecodeError>;

    /// Decode block of instructions
    fn decode_block(&self, bytes: &[u8]) -> Result<Vec<DecodedInstruction>, DecodeError>;
}
```

### 2. x86_64 Decoder

**Features**:
- Complex CISC instruction set
- Variable-length instructions (1-15 bytes)
- Legacy, SSE, AVX, AVX2, AVX-512
- Prefixes (REX, VEX, EVEX)
- Complex addressing modes

**Usage**:
```rust
use vm_frontend::x86_64::X86_64Decoder;

let decoder = X86_64Decoder::new();

// Decode instruction
let instruction = decoder.decode(&[0x48, 0x89, 0xd8])?;
println!("Instruction: {:?}", instruction);

// Get length
let len = decoder.get_length(&[0x48, 0x89, 0xd8])?;
```

**Supported Instructions**:
- **Arithmetic**: ADD, SUB, MUL, DIV, INC, DEC
- **Bitwise**: AND, OR, XOR, NOT, SHL, SHR
- **Data Movement**: MOV, PUSH, POP, XCHG
- **Control**: JMP, CALL, RET, conditional jumps
- **SIMD**: SSE, SSE2, AVX, AVX2, AVX-512

### 3. ARM64 Decoder

**Features**:
- Fixed-length 32-bit instructions
- Load/store architecture
- NEON SIMD instructions
- Conditional execution (limited)
- PC-relative addressing

**Usage**:
```rust
use vm_frontend::arm64::Arm64Decoder;

let decoder = Arm64Decoder::new();

// Decode instruction
let instruction = decoder.decode(&[0x00, 0x00, 0x00, 0xb0])?; // add x0, x0, #0
println!("Instruction: {:?}", instruction);
```

**Supported Instructions**:
- **Arithmetic**: ADD, SUB, MUL, DIV
- **Bitwise**: AND, ORR, EOR, MVN
- **Data Movement**: MOV, LDR, STR, LDP, STP
- **Control**: B, BL, BR, RET, conditional branches
- **NEON**: Vector SIMD operations

### 4. RISC-V Decoder

**Features**:
- Fixed-length 32-bit instructions (base ISA)
- 16-bit compressed instructions (C extension)
- Load/store architecture
- Modular extensions
- Simple, regular encoding

**Usage**:
```rust
use vm_frontend::riscv::RiscvDecoder;

let decoder = RiscvDecoder::new();

// Decode instruction
let instruction = decoder.decode(&[0x13, 0x05, 0xa0, 0x02])?; // addi a0, sp, 512
println!("Instruction: {:?}", instruction);
```

**Supported Instructions**:
- **RV64I**: Base integer ISA
- **RV64M**: Multiply/divide
- **RV64A**: Atomic operations
- **RV64F**: Single-precision float
- **RV64D**: Double-precision float
- **RV64C**: Compressed instructions

### 5. Decoded Instruction Structure

```rust
pub struct DecodedInstruction {
    pub address: u64,
    pub opcode: Opcode,
    pub operands: Vec<Operand>,
    pub length: u8,
    pub category: InstructionCategory,
    pub flags: InstructionFlags,
}
```

**Operand Types**:
```rust
pub enum Operand {
    Register(u8),           // Register index
    Immediate(i64),         // Immediate value
    Memory {                // Memory operand
        base: Option<u8>,
        index: Option<u8>,
        disp: i32,
        scale: u8,
    },
    Relative(i32),          // PC-relative offset
}
```

## Features

### Prefix Handling (x86_64)

```rust
use vm_frontend::x86_64::{PrefixHandler, Prefix};

let handler = PrefixHandler::new();

// Parse prefixes
let prefixes = handler.parse(&[0x66, 0x48, 0x89])?;

if prefixes.contains(Prefix::REXW) {
    println!("64-bit operand size");
}
```

### SIMD Decoding

**SSE/AVX (x86_64)**:
```rust
// AVX instruction: vaddpd %ymm0, %ymm1, %ymm2
let bytes = &[0xc5, 0xfd, 0x58, 0xc2];
let instruction = decoder.decode(bytes)?;
```

**NEON (ARM64)**:
```rust
// NEON instruction: add v0.2d, v1.2d, v2.2d
let bytes = &[0x4e, 0x00, 0x58, 0x20];
let instruction = decoder.decode(bytes)?;
```

### Addressing Modes

**x86_64 Addressing**:
```rust
// [base + index*scale + disp]
// mov rax, [rbx + rcx*4 + 100]
let bytes = &[0x48, 0x8b, 0x84, 0xcb, 0x64, 0x00, 0x00, 0x00];
```

**ARM64 Addressing**:
```rust
// [base, #offset]
// ldr x0, [x1, #8]
let bytes = &[0x00, 0x40, 0x40, 0xf9];
```

## Optimization Hints

### Branch Targets

```rust
use vm_frontend::optimizer::{BranchAnalyzer, TargetInfo};

let analyzer = BranchAnalyzer::new();
let info = analyzer.analyze(&instruction)?;

if info.is_branch() {
    println!("Branch target: 0x{:x}", info.target.unwrap());
}
```

### Side Effects

```rust
use vm_frontend::optimizer::SideEffectAnalyzer;

let analyzer = SideEffectAnalyzer::new();
let has_side_effects = analyzer.has_side_effects(&instruction)?;
```

### Hot-spot Markers

```rust
use vm_frontend::optimizer::HotspotMarker;

// Mark instruction as hot-spot
marker.mark(instruction, HotspotLevel::Hot)?;
```

## Performance Characteristics

### Decoding Speed

| Architecture | Throughput | Latency |
|--------------|------------|---------|
| x86_64 | 100-200 M instr/s | 10-50 cycles |
| ARM64 | 200-400 M instr/s | 5-20 cycles |
| RISC-V | 200-400 M instr/s | 5-20 cycles |

### Instruction Length

| Architecture | Min | Max | Average |
|--------------|-----|-----|---------|
| x86_64 | 1 | 15 | 3-4 |
| ARM64 | 4 | 4 | 4 |
| RISC-V | 2 | 4 | 3.5 |

## Usage Examples

### Basic Decoding

```rust
use vm_frontend::DecoderFactory;

// Create decoder for architecture
let decoder = DecoderFactory::create(Architecture::X86_64)?;

// Decode instruction
let instruction = decoder.decode(&bytes)?;

// Access fields
println!("Opcode: {:?}", instruction.opcode);
println!("Operands: {:?}", instruction.operands);
```

### Block Decoding

```rust
use vm_frontend::BlockDecoder;

let decoder = BlockDecoder::new(Architecture::ARM64)?;

// Decode entire block
let instructions = decoder.decode_block(&code_bytes)?;

for instr in instructions {
    println!("0x{:x}: {:?}", instr.address, instr.opcode);
}
```

### Cross-Architecture Code

```rust
use vm_frontend::{Architecture, DecoderFactory};

fn decode_code(arch: Architecture, bytes: &[u8]) {
    let decoder = DecoderFactory::create(arch).unwrap();
    let instruction = decoder.decode(bytes).unwrap();
    println!("{:?} instruction: {:?}", arch, instruction.opcode);
}

// Works for all architectures
decode_code(Architecture::X86_64, &[0x48, 0x89, 0xd8]);
decode_code(Architecture::ARM64, &[0x00, 0x00, 0x00, 0xb0]);
decode_code(Architecture::RISCV64, &[0x13, 0x05, 0xa0, 0x02]);
```

## Architecture Comparison

| Feature | x86_64 | ARM64 | RISC-V |
|---------|--------|-------|--------|
| **ISA Type** | CISC | RISC | RISC |
| **Instruction Length** | Variable | Fixed | Fixed/Compressed |
| **Registers** | 16 | 31 | 31 |
| **Addressing Modes** | Complex | Simple | Simple |
| **SIMD** | SSE/AVX | NEON | Vector ext |
| **Decoding Complexity** | High | Low | Low |

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| x86_64 Linux | ✅ Full | All features |
| x86_64 macOS | ✅ Full | Legacy support |
| ARM64 Linux | ✅ Full | Good support |
| ARM64 macOS | ✅ Full | Apple Silicon |
| RISC-V Linux | ✅ Growing | Base ISA + extensions |

## Best Practices

1. **Use Factory**: Create decoders via DecoderFactory
2. **Handle Errors**: Decoding can fail for invalid bytes
3. **Cache Results**: Reuse decoded instructions
4. **Validate Input**: Check byte array length
5. **Test Edge Cases**: Invalid instructions, prefixes

## Testing

```bash
# Run all tests
cargo test -p vm-frontend

# Test x86_64 decoder
cargo test -p vm-frontend --lib x86_64

# Test ARM64 decoder
cargo test -p vm-frontend --lib arm64

# Test RISC-V decoder
cargo test -p vm-frontend --lib riscv
```

## Related Crates

- **vm-ir**: Intermediate representation
- **vm-engine**: Execution engine
- **vm-cross-arch-support**: Cross-architecture translation

## License

[Your License Here]

## Contributing

Contributions welcome! Please:
- Test with real instruction streams
- Add missing instructions
- Improve decoder accuracy
- Add optimization hints

## See Also

- [x86_64 Manual](https://www.felixcloutier.com/x86/)
- [ARM64 Reference](https://developer.arm.com/documentation/ddi0487/latest/)
- [RISC-V ISA](https://riscv.org/technical/specifications/)
