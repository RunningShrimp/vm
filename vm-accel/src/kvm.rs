use super::Accel;
use std::path::Path;

pub struct AccelKvm;
impl AccelKvm { pub fn new() -> Self { Self } }
impl Accel for AccelKvm {
    fn init(&mut self) -> bool { Path::new("/dev/kvm").exists() }
    fn map_memory(&mut self, _guest_pa: u64, _size: u64) -> bool { true }
    fn create_vcpu(&mut self, _id: u32) -> bool { true }
    fn run(&mut self) -> bool { true }
}
