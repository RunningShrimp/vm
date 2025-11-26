use super::Accel;

pub struct AccelHvf;
impl AccelHvf { pub fn new() -> Self { Self } }
impl Accel for AccelHvf {
    fn init(&mut self) -> bool { true }
    fn map_memory(&mut self, _guest_pa: u64, _size: u64) -> bool { true }
    fn create_vcpu(&mut self, _id: u32) -> bool { true }
    fn run(&mut self) -> bool { true }
}
