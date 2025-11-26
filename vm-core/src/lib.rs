pub type GuestAddr = u64;

#[derive(Clone, Copy, Debug)]
pub enum AccessType {
    Read,
    Write,
    Exec,
}

#[derive(Debug)]
pub enum Fault {
    PageFault,
    AccessViolation,
    InvalidOpcode,
}

pub trait MMU {
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestAddr, Fault>;
    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, Fault>;
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, Fault>;
    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), Fault>;
}

pub struct ExecStats {
    pub executed_ops: u64,
}

pub enum ExecStatus {
    Ok,
    Fault(Fault),
}

pub struct ExecResult {
    pub status: ExecStatus,
    pub stats: ExecStats,
}

pub trait ExecutionEngine<B> {
    fn run(&mut self, mmu: &mut dyn MMU, block: &B) -> ExecResult;
}

pub trait Decoder {
    type Block;
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, Fault>;
}

 

