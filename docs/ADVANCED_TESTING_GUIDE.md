# Advanced Testing Guide for VM Project

This guide covers advanced testing methodologies used in the VM project, including property-based testing and fuzzing. These techniques help discover edge cases, improve code quality, and increase confidence in system correctness.

## Table of Contents

1. [Property-Based Testing](#property-based-testing)
2. [Fuzzing](#fuzzing)
3. [Writing Effective Property Tests](#writing-effective-property-tests)
4. [Debugging Failing Tests](#debugging-failing-tests)
5. [Best Practices](#best-practices)
6. [CI/CD Integration](#cicd-integration)

---

## Property-Based Testing

### What is Property-Based Testing?

Property-based testing (PBT) is a testing approach where you specify **properties** (invariants) that your code should satisfy, and the testing framework generates hundreds or thousands of random inputs to verify these properties.

Unlike traditional unit testing that checks specific examples, PBT explores the entire input space to find edge cases and bugs.

### Key Concepts

#### Properties vs. Examples

**Example-based testing (traditional)**:
```rust
#[test]
fn test_addition() {
    assert_eq!(add(2, 3), 5);
    assert_eq!(add(0, 0), 0);
    assert_eq!(add(-1, 1), 0);
}
```

**Property-based testing**:
```rust
proptest! {
    #[test]
    fn prop_add_commutative(a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(add(a, b), add(b, a));
    }
}
```

#### Common Properties

1. **Round-trip properties**: encode → decode → encode should give same result
2. **Inverse properties**: f(g(x)) == x for valid inputs
3. **Idempotence**: f(f(x)) == f(x)
4. **Commutativity**: f(a, b) == f(b, a)
5. **Consistency**: Different approaches give same result

### Running Property Tests

We use `proptest` for property-based testing in the VM project.

```bash
# Run all property tests
cargo test --test *property_tests*

# Run specific property test file
cargo test --test memory_property_tests

# Run with more test cases (default is 256)
cargo test --test memory_property_tests -- --test-threads=1 PROPTEST_CASES=1000

# Run with specific seed for reproducibility
cargo test --test memory_property_tests -- --test-threads=1 PROPTEST_SEED=12345
```

### Property Tests in VM Project

#### Memory Properties (`tests/memory_property_tests.rs`)

```rust
proptest! {
    /// Property: Reading after writing should return the written value
    #[test]
    fn prop_read_after_write_consistency(
        addr in 0usize..(1usize << 20),
        value in any::<[u8; 8]>(),
    ) {
        let pool = MemoryPool::new(1 << 20);
        pool.write(GuestAddr(addr as u64), &value).unwrap();
        let mut buffer = [0u8; 8];
        pool.read(GuestAddr(addr as u64), &mut buffer).unwrap();
        prop_assert_eq!(buffer, value);
    }
}
```

Key memory properties tested:
- **Read-after-write consistency**: Written data should be readable
- **Cross-boundary access**: Operations spanning multiple pages work correctly
- **Independence**: Operations on different regions don't interfere
- **Bounds checking**: Out-of-bounds access is properly rejected

#### Instruction Properties (`tests/instruction_property_tests.rs`)

```rust
proptest! {
    /// Property: Encoding then decoding should preserve the instruction
    #[test]
    fn prop_encode_decode_roundtrip(fields in any::<InstructionFields>()) {
        let encoded = encode_r_type(fields.opcode, fields.rd, ...);
        let decoded_opcode = encoded.opcode();
        prop_assert_eq!(decoded_opcode as u8, fields.opcode as u8);
    }
}
```

Key instruction properties tested:
- **Encode-decode roundtrip**: decode(encode(instruction)) == instruction
- **Length consistency**: All instructions are 4 bytes (RISC-V)
- **Register range**: Register indices are always 0-31
- **Sign extension**: Immediates are correctly sign-extended

#### Device Properties (`tests/device_property_tests.rs`)

```rust
proptest! {
    /// Property: Writing a block and reading it should return same data
    #[test]
    fn prop_block_read_write_consistency(
        block_idx in 0u64..1000u64,
        data in prop::collection::vec(any::<u8>(), 512),
    ) {
        let device = MockBlockDevice::new(512, 1000);
        device.write_block(block_idx, &data).unwrap();
        let mut buffer = vec![0u8; 512];
        device.read_block(block_idx, &mut buffer).unwrap();
        prop_assert_eq!(buffer, data);
    }
}
```

Key device properties tested:
- **Read-write consistency**: Data persists after writes
- **Independence**: Different blocks don't interfere
- **State validity**: Device state transitions are valid
- **Error recovery**: Devices recover from error states

---

## Fuzzing

### What is Fuzzing?

Fuzzing is an automated testing technique that provides **invalid**, **unexpected**, or **random** data as inputs to a program. The goal is to find crashes, memory corruption, assertion failures, or security vulnerabilities.

### Key Concepts

#### Coverage-Guided Fuzzing

Modern fuzzers like libFuzzer use **code coverage** to guide input generation:
1. Maintain a set of interesting inputs
2. Track which code paths each input exercises
3. Mutate inputs to discover new code paths
4. Prioritize inputs that increase coverage

#### Crash Reporting

When the fuzzer finds a crash:
1. It saves the minimal reproducing input
2. Provides a stack trace
3. Can often minimize the test case automatically

### Running Fuzz Tests

We use `cargo-fuzz` with libFuzzer in the VM project.

```bash
# Initialize fuzzing (first time only)
cargo install cargo-fuzz
cd fuzz && cargo fuzz init

# Run a specific fuzz target
cargo fuzz run instruction_decoder

# Run with specific corpus directory
cargo fuzz run memory_access fuzz/corpus/memory_access/

# Run with specific timeout (in seconds)
cargo fuzz run instruction_decoder -- -timeout=10

# Run with limited number of iterations
cargo fuzz run instruction_decoder -- -runs=1000000

# Minimize a crashing test case
cargo fuzz tmin instruction_detector crash-abc123
```

### Fuzz Targets in VM Project

#### Instruction Decoder Fuzzer (`fuzz/fuzz_targets/instruction_decoder.rs`)

**Purpose**: Test robustness of instruction decoder against random byte sequences.

**What it does**:
1. Takes random byte sequences as input
2. Attempts to decode them as RISC-V instructions
3. Verifies the decoder never crashes
4. Validates decoded instructions are well-formed

**Key invariants tested**:
```rust
// Decoder should never panic
let result = decode_instruction(data);

// Result should always be valid
let is_valid = validate_instruction(&result);
assert!(is_valid, "Invalid instruction decoded");

// If decoded as valid instruction, verify fields
if let DecodeResult::ValidInstruction { opcode, rd, rs1, rs2, imm } = result {
    assert!(opcode < 128, "Opcode out of range");
    if let Some(r) = rd {
        assert!(r < 32, "Register out of range");
    }
}
```

#### Memory Access Fuzzer (`fuzz/fuzz_targets/memory_access.rs`)

**Purpose**: Test memory subsystem against random access patterns.

**What it does**:
1. Parses random bytes as memory operations (read/write)
2. Executes operations on a memory pool
3. Verifies bounds checking prevents invalid access
4. Checks memory state remains consistent

**Key invariants tested**:
```rust
// Out-of-bounds access should fail
if addr + size > pool_size {
    assert!(pool.read(addr, &mut buffer).is_err());
}

// In-bounds access should succeed
if addr + size <= pool_size {
    assert!(pool.read(addr, &mut buffer).is_ok());
}

// Written data should be readable
pool.write(addr, &data).unwrap();
pool.read(addr, &mut buffer).unwrap();
assert_eq!(buffer, data);
```

#### JIT Compiler Fuzzer (`fuzz/fuzz_targets/jit_compiler.rs`)

**Purpose**: Test JIT compiler robustness against random IR instructions.

**What it does**:
1. Parses random bytes as IR instructions
2. Attempts to compile them
3. Verifies compiler handles all inputs gracefully
4. Checks for division-by-zero and other unsafe patterns

**Key invariants tested**:
```rust
// Invalid IR should be rejected
if !instruction.is_valid() {
    assert_eq!(compiler.compile(), CompileResult::InvalidIr);
}

// Unsafe code should be rejected
if !instruction.is_safe_to_compile() {
    assert_eq!(compiler.compile(), CompileResult::UnsafeCode);
}

// Compilation should produce reasonable output
if let CompileResult::Success { instruction_count, code_size } = result {
    assert!(instruction_count <= 10000);
    assert!(code_size <= 1_000_000);
}
```

---

## Writing Effective Property Tests

### 1. Choose Good Properties

Good properties are:
- **Simple and clear**: Easy to understand and verify
- **High-value**: Test important functionality
- **Non-trivial**: Likely to catch bugs
- **Maintainable**: Don't break easily with refactoring

**Example of good property**:
```rust
// Memory read-after-write: fundamental invariant
prop_assert_eq!(read(write(addr, data)), data);
```

**Example of bad property**:
```rust
// Too specific, implementation-dependent
prop_assert_eq!(memory.internal_buffer[0], 0x42);
```

### 2. Use Appropriate Strategies

`proptest` provides strategies for generating random data:

```rust
// Any type (with Arbitrary trait)
value in any::<i32>()

// Ranges
num in 0usize..1000usize

// Collections
vec in prop::collection::vec(any::<u8>(), 1..256)

// Custom strategies
addr in prop::array::uniform11(0usize..(1usize << 20))
```

### 3. Handle Edge Cases

Use `prop_assume!` to filter invalid inputs:

```rust
proptest! {
    #[test]
    fn prop_memory_operation(
        addr in 0usize..(1usize << 21),
        size in 1usize..4096usize,
    ) {
        // Skip cases that would overflow
        let end_addr = addr.checked_add(size).unwrap();
        prop_assume!(end_addr <= (1usize << 20));

        // Test logic here
        let pool = MemoryPool::new(1 << 20);
        prop_assert!(pool.read(GuestAddr(addr as u64), &mut vec![0u8; size]).is_ok());
    }
}
```

### 4. Provide Helpful Failure Messages

Customize output for debugging:

```rust
proptest! {
    #[test]
    fn prop_with_debug(
        addr in any::<u64>(),
        data in prop::collection::vec(any::<u8>(), 1..256),
    ) {
        let pool = MemoryPool::new(1 << 20);

        // Add context to assertions
        if let Err(e) = pool.write(GuestAddr(addr), &data) {
            eprintln!("Write failed at addr 0x{:x}, size {}", addr, data.len());
            eprintln!("Error: {:?}", e);
        }

        prop_assert!(pool.write(GuestAddr(addr), &data).is_ok());
    }
}
```

---

## Debugging Failing Tests

### Property Tests

When a property test fails, proptest shows:
1. The failing input (simplified/minimized)
2. A seed for reproduction
3. The exact assertion that failed

```rust
thread 'main' panicked at 'Test failed: AssertionError at ...
  Failing input:
    addr = 4096
    data = [1, 2, 3]
  Successes: 127
  Seed: 1234567890
```

**Reproduce with seed**:
```bash
# Use the seed from failure message
cargo test --test memory_property_tests -- --test-threads=1 PROPTEST_SEED=1234567890
```

**Debug with fewer cases**:
```bash
# Run with minimal cases to debug
cargo test --test memory_property_tests -- --test-threads=1 PROPTEST_CASES=10
```

### Fuzz Tests

When a fuzzer finds a crash:
1. It saves the crash input to `fuzz/artifacts/`
2. Provides the filename in the error message

```bash
# Reproduce crash
cargo fuzz run instruction_decoder fuzz/artifacts/instruction_decoder/crash-abc123

# Minimize crash case
cargo fuzz tmin instruction_decoder fuzz/artifacts/instruction_decoder/crash-abc123

# Debug with gdb
cargo fuzz run instruction_decoder -- -runs=0 fuzz/artifacts/instruction_decoder/crash-abc123
# Then attach debugger to the PID shown
```

**Common fuzzer flags**:
```bash
# Limit execution time per input
-timeout=10

# Limit total time
-max_total_time=3600

# Limit memory usage
-max_total_time=3600 -rss_limit_mb=2048

# Print coverage information
-print_coverage=1

# Use dictionary for better inputs
-dict=fuzz/dictionaries/instructions.txt
```

---

## Best Practices

### Property-Based Testing

1. **Start small**: Begin with simple properties
2. **Focus on invariants**: What must always be true?
3. **Test boundaries**: Edge cases and limits
4. **Keep tests fast**: Fast tests = more iterations = better coverage
5. **Use meaningful names**: `prop_read_write_consistency` vs `prop_test_1`

### Fuzzing

1. **Fuzz critical code**: Focus on security-sensitive components
2. **Provide structure**: Use dictionaries for valid input formats
3. **Run continuously**: Integrate into CI/CD pipeline
4. **Review crashes**: Investigate all findings
5. **Keep targets simple**: Minimize code in fuzz target

### General

1. **Test in isolation**: Mock external dependencies
2. **Seed corpus**: Provide known-good inputs to guide fuzzer
3. **Monitor performance**: Don't let tests slow down development
4. **Document properties**: Explain what you're testing and why
5. **Update regularly**: Keep tests in sync with code changes

---

## CI/CD Integration

### Property Tests in CI

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Run property tests
        run: |
          cargo test --test *property_tests* -- --test-threads=1 \
            PROPTEST_CASES=1000

      - name: Upload test results
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: property-test-results
          path: target/ptest-results/
```

### Fuzzing in CI

```yaml
# .github/workflows/fuzz.yml
name: Fuzz

on:
  schedule:
    - cron: '0 0 * * *'  # Daily
  workflow_dispatch:

jobs:
  fuzz:
    runs-on: ubuntu-latest
    timeout-minutes: 360  # 6 hours
    steps:
      - uses: actions/checkout@v3

      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz

      - name: Run fuzzers
        run: |
          cargo fuzz run instruction_decoder -- -max_total_time=3600
          cargo fuzz run memory_access -- -max_total_time=3600
          cargo fuzz run jit_compiler -- -max_total_time=3600

      - name: Upload corpus
        uses: actions/upload-artifact@v3
        with:
          name: fuzz-corpus
          path: fuzz/corpus/

      - name: Check for crashes
        run: |
          if [ -n "$(find fuzz/artifacts -name 'crash-*')" ]; then
            echo "Fuzzer found crashes!"
            exit 1
          fi
```

### Continuous Fuzzing

For serious projects, consider:
1. **OSSFuzz**: Google's continuous fuzzing service for open source
2. **ClusterFuzz**: Automated fuzzing at scale
3. **Fuzz-introspector**: Analyze fuzz coverage

---

## Resources

### Tools and Libraries

- **proptest**: Property-based testing framework for Rust
- **cargo-fuzz**: Fuzzing integration for Cargo
- **libFuzzer**: Coverage-guided fuzzer (used by cargo-fuzz)
- **honggfuzz**: Security-oriented fuzzer
- **afl**: American Fuzzy Lop (classic fuzzer)

### Further Reading

- [proptest book](https://altsysrq.github.io/proptest-book/intro.html)
- [cargo-fuzz guide](https://rust-fuzz.github.io/book/cargo-fuzz.html)
- [Fuzzing book](https://www.fuzzingbook.org/)
- [Google's Fuzzing Guide](https://github.com/google/oss-fuzz/blob/master/docs/fuzzing_guide.md)

### Internal Resources

- `tests/memory_property_tests.rs` - Memory property tests
- `tests/instruction_property_tests.rs` - Instruction property tests
- `tests/device_property_tests.rs` - Device property tests
- `fuzz/fuzz_targets/instruction_decoder.rs` - Instruction decoder fuzzer
- `fuzz/fuzz_targets/memory_access.rs` - Memory access fuzzer
- `fuzz/fuzz_targets/jit_compiler.rs` - JIT compiler fuzzer

---

## Conclusion

Property-based testing and fuzzing are powerful techniques for discovering bugs that traditional testing might miss. By investing in these advanced testing methodologies, the VM project achieves:

1. **Higher code quality**: More bugs found and fixed
2. **Greater confidence**: Better coverage of edge cases
3. **Improved robustness**: Code handles unexpected inputs gracefully
4. **Security assurance**: Vulnerabilities discovered before production

Start with simple properties and fuzz targets, then gradually increase complexity as you become more familiar with the techniques. Happy testing!
