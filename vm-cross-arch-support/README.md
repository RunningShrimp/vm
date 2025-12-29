# VM Cross-Architecture Support

This crate provides consolidated cross-architecture utilities for VM translation, combining four previously separate packages into a single, unified module.

## Overview

This package merges the functionality of:
- `vm-encoding` - Instruction encoding framework
- `vm-memory-access` - Memory access patterns and optimization
- `vm-instruction-patterns` - Instruction pattern recognition and matching
- `vm-register` - Register management, mapping, and allocation

## Modules

### encoding
Provides common encoding framework for instruction encoding across different architectures.

**Key Types:**
- `Architecture` - Supported CPU architectures (X86_64, ARM64, RISCV64)
- `RegId` - Register ID wrapper
- `EncodingError` - Errors during instruction encoding
- `EncodingContext` - Context for instruction generation
- `InstructionEncoding` - Trait for architecture-specific encoding
- `InstructionBuilder` - Trait for building instructions incrementally

### memory_access
Provides unified memory access patterns, alignment handling, and endianness conversion.

**Key Types:**
- `MemoryAccessPattern` - Description of memory access patterns
- `MemoryAccessOptimizer` - Trait for optimizing memory access
- `DefaultMemoryAccessOptimizer` - Default optimizer implementation
- `EndiannessConverter` - Cross-architecture endianness conversion
- `MemoryAccessAnalyzer` - Pattern analysis and statistics

### instruction_patterns
Provides instruction pattern matching and semantic analysis across architectures.

**Key Types:**
- `InstructionPattern` - Instruction pattern definition
- `PatternMatcher` - Trait for pattern matching
- `DefaultPatternMatcher` - Default pattern matcher implementation
- `IROp` - IR operation representation
- `InstructionCategory` - Instruction categories for classification

### register
Provides register management, mapping, and allocation across architectures.

**Key Types:**
- `RegisterSet` - Register set for an architecture
- `RegisterMapper` - Cross-architecture register mapping
- `RegisterAllocator` - Optimized register allocation
- `RegisterInfo` - Register information
- `MappingStrategy` - Register mapping strategies

## Features

- **Unified Interfaces**: Single, consistent API for multiple architectures
- **Cross-Architecture Translation**: Support for translating between x86_64, ARM64, and RISC-V64
- **Pattern Matching**: Advanced pattern recognition for instruction optimization
- **Register Management**: Comprehensive register mapping and allocation utilities
- **Memory Optimization**: Memory access pattern analysis and optimization

## Dependencies

- `thiserror` - Error handling
- `serde` - Serialization support
- `log` - Logging facade
- `static_assertions` - Compile-time assertions

## Usage Example

```rust
use vm_cross_arch_support::{
    encoding::{Architecture, EncodingContext, RegId},
    memory_access::{MemoryAccessPattern, AccessWidth, DefaultMemoryAccessOptimizer},
    register::{RegisterSet, RegisterMapper, MappingStrategy},
};

// Create encoding context
let ctx = EncodingContext::new(Architecture::X86_64);

// Optimize memory access
let optimizer = DefaultMemoryAccessOptimizer::new(Architecture::ARM64);
let pattern = MemoryAccessPattern::new(RegId(0), 0x1000, AccessWidth::Word);
let optimized = optimizer.optimize_access_pattern(&pattern);

// Map registers
let source_set = RegisterSet::new(Architecture::X86_64);
let target_set = RegisterSet::new(Architecture::ARM64);
let mut mapper = RegisterMapper::new(source_set, target_set, MappingStrategy::Direct);
let mapped = mapper.map_register(RegId(5)).unwrap();
```

## Architecture Support

- **x86_64** - Full support for encoding, patterns, and register management
- **ARM64** - Full support for encoding, patterns, and register management
- **RISC-V64** - Full support for encoding, patterns, and register management

## Testing

The package includes comprehensive unit tests for all modules:

```bash
cargo test -p vm-cross-arch-support
```

## License

MIT OR Apache-2.0
