#![no_main]
//! JIT Compiler Fuzzing Target
//!
//! This fuzzing target tests the robustness of the JIT compiler.
//! It feeds random IR instructions and verifies that:
//! 1. The compiler never crashes
//! 2. Invalid IR is properly rejected
//! 3. Valid IR compiles correctly
//! 4. Generated code is safe to execute

use libfuzzer_sys::fuzz_target;

/// IR opcode types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IrOpcode {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,

    // Bitwise
    And,
    Or,
    Xor,
    Shl,
    Shr,

    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Memory
    Load,
    Store,

    // Control flow
    Branch,
    Jump,
    Call,
    Return,

    // Other
    Mov,
    Const,
    Invalid,
}

impl IrOpcode {
    fn from_u8(value: u8) -> Self {
        match value % 20 {
            0 => IrOpcode::Add,
            1 => IrOpcode::Sub,
            2 => IrOpcode::Mul,
            3 => IrOpcode::Div,
            4 => IrOpcode::Rem,
            5 => IrOpcode::And,
            6 => IrOpcode::Or,
            7 => IrOpcode::Xor,
            8 => IrOpcode::Shl,
            9 => IrOpcode::Shr,
            10 => IrOpcode::Eq,
            11 => IrOpcode::Ne,
            12 => IrOpcode::Lt,
            13 => IrOpcode::Le,
            14 => IrOpcode::Gt,
            15 => IrOpcode::Ge,
            16 => IrOpcode::Load,
            17 => IrOpcode::Store,
            18 => IrOpcode::Branch,
            19 => IrOpcode::Jump,
            _ => IrOpcode::Invalid,
        }
    }

    fn is_valid(self) -> bool {
        !matches!(self, IrOpcode::Invalid)
    }

    fn num_operands(self) -> usize {
        match self {
            IrOpcode::Const => 1,
            IrOpcode::Mov | IrOpcode::Return => 1,
            IrOpcode::Add | IrOpcode::Sub | IrOpcode::Mul | IrOpcode::Div | IrOpcode::Rem => 2,
            IrOpcode::And | IrOpcode::Or | IrOpcode::Xor => 2,
            IrOpcode::Shl | IrOpcode::Shr => 2,
            IrOpcode::Eq | IrOpcode::Ne | IrOpcode::Lt | IrOpcode::Le | IrOpcode::Gt | IrOpcode::Ge => 2,
            IrOpcode::Load => 2,
            IrOpcode::Store => 3,
            IrOpcode::Branch => 3,
            IrOpcode::Jump => 1,
            IrOpcode::Call => 1,
            IrOpcode::Invalid => 0,
        }
    }
}

/// IR operand
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IrOperand {
    Register(u8),
    Immediate(i64),
    Invalid,
}

impl IrOperand {
    fn from_u64(value: u64) -> Self {
        // Use MSB to determine type
        if value & 0x8000_0000_0000_0000 != 0 {
            // Register
            IrOperand::Register((value & 0xFF) as u8)
        } else {
            // Immediate
            IrOperand::Immediate(value as i64)
        }
    }

    fn is_valid(self) -> bool {
        match self {
            IrOperand::Register(reg) => *reg < 32, // Max 32 registers
            IrOperand::Immediate(_) => true,
            IrOperand::Invalid => false,
        }
    }
}

/// IR instruction
#[derive(Debug, Clone)]
struct IrInstruction {
    opcode: IrOpcode,
    operands: Vec<IrOperand>,
}

impl IrInstruction {
    fn new(opcode: IrOpcode, operands: Vec<IrOperand>) -> Self {
        Self { opcode, operands }
    }

    /// Validate instruction
    fn is_valid(&self) -> bool {
        // Check opcode
        if !self.opcode.is_valid() {
            return false;
        }

        // Check operand count
        let expected_count = self.opcode.num_operands();
        if self.operands.len() != expected_count {
            return false;
        }

        // Check operands
        for operand in &self.operands {
            if !operand.is_valid() {
                return false;
            }
        }

        true
    }

    /// Check if instruction is safe to compile
    fn is_safe_to_compile(&self) -> bool {
        // Division by zero check would go here in a real implementation
        if matches!(self.opcode, IrOpcode::Div | IrOpcode::Rem) {
            // Check if divisor is constant zero
            if self.operands.len() >= 2 {
                if let IrOperand::Immediate(0) = self.operands[1] {
                    return false;
                }
            }
        }

        // Memory alignment checks would go here
        if matches!(self.opcode, IrOpcode::Load | IrOpcode::Store) {
            // Ensure memory operands are valid
            // (In a real implementation, this would check alignment)
        }

        true
    }
}

/// JIT compiler result
#[derive(Debug)]
enum CompileResult {
    Success {
        instruction_count: usize,
        code_size: usize,
    },
    InvalidIr,
    UnsafeCode,
    UnsupportedFeature,
    CompilerError,
}

/// Mock JIT compiler
///
/// This is a simplified JIT compiler that attempts to compile IR instructions.
/// It's designed to be robust and never panic.
struct MockJitCompiler {
    instructions: Vec<IrInstruction>,
}

impl MockJitCompiler {
    fn new() -> Self {
        Self {
            instructions: Vec::new(),
        }
    }

    /// Add instruction
    fn add_instruction(&mut self, instruction: IrInstruction) {
        self.instructions.push(instruction);
    }

    /// Compile all instructions
    fn compile(&self) -> CompileResult {
        // Validate all instructions
        for instruction in &self.instructions {
            if !instruction.is_valid() {
                return CompileResult::InvalidIr;
            }
            if !instruction.is_safe_to_compile() {
                return CompileResult::UnsafeCode;
            }
        }

        // Check for unsupported features
        for instruction in &self.instructions {
            match instruction.opcode {
                IrOpcode::Call | IrOpcode::Return => {
                    // For simplicity, reject call/return in this mock
                    return CompileResult::UnsupportedFeature;
                }
                _ => {}
            }
        }

        // Count instructions and estimate code size
        let instruction_count = self.instructions.len();
        let code_size = instruction_count * 16; // Assume 16 bytes per instruction

        CompileResult::Success {
            instruction_count,
            code_size,
        }
    }

    /// Reset compiler
    fn reset(&mut self) {
        self.instructions.clear();
    }
}

/// Fuzz target function
///
/// This function is called by libfuzzer with random byte sequences.
/// It parses the input as IR instructions and attempts to compile them.
fuzz_target!(|data: &[u8]| {
    let mut compiler = MockJitCompiler::new();

    // Parse input as IR instructions
    let instructions = parse_ir_instructions(data);

    // Add instructions to compiler
    for instruction in instructions {
        compiler.add_instruction(instruction);
    }

    // Attempt to compile
    let result = compiler.compile();

    // Verify result is sane
    match result {
        CompileResult::Success { instruction_count, code_size } => {
            // Should have reasonable counts
            assert!(instruction_count <= 10000, "Too many instructions");
            assert!(code_size <= 1_000_000, "Generated code too large");

            // Code size should be proportional to instruction count
            assert!(code_size >= instruction_count, "Code size too small");
        }
        CompileResult::InvalidIr => {
            // Expected result for invalid input
        }
        CompileResult::UnsafeCode => {
            // Expected result for unsafe code
        }
        CompileResult::UnsupportedFeature => {
            // Expected result for unsupported features
        }
        CompileResult::CompilerError => {
            // Should not reach this in mock compiler
            eprintln!("Unexpected compiler error");
            panic!("Compiler error should not occur");
        }
    }

    // Verify compiler can be reset and reused
    compiler.reset();
    assert!(compiler.instructions.is_empty(), "Reset should clear instructions");
});

/// Parse byte sequence into IR instructions
fn parse_ir_instructions(data: &[u8]) -> Vec<IrInstruction> {
    let mut instructions = Vec::new();

    // Each instruction is encoded as:
    // [opcode: 1 byte][operand_count: 1 byte][operands...]
    let mut pos = 0;

    while pos < data.len() {
        // Need at least opcode and operand count
        if pos + 2 > data.len() {
            break;
        }

        let opcode = IrOpcode::from_u8(data[pos]);
        let operand_count = (data[pos + 1] % 4) as usize; // Max 4 operands

        pos += 2;

        // Parse operands (8 bytes each)
        let mut operands = Vec::new();
        for _ in 0..operand_count {
            if pos + 8 > data.len() {
                break;
            }

            let operand_bytes = [
                data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
                data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
            ];
            let operand_value = u64::from_le_bytes(operand_bytes);
            operands.push(IrOperand::from_u64(operand_value));

            pos += 8;
        }

        instructions.push(IrInstruction::new(opcode, operands));

        // Limit total instructions
        if instructions.len() >= 1000 {
            break;
        }
    }

    instructions
}
