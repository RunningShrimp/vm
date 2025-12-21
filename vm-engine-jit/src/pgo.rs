use std::sync::Mutex;
use std::time::Duration;

pub struct ProfileCollector {
    /// 采样间隔
    ///
    /// 当前用于初始化，未来可能用于动态调整采样频率。
    #[allow(dead_code)] // Used in initialization, may be used for dynamic adjustment in future
    interval: Duration,
    // internal state for stubbed PGO
    inner: Mutex<ProfileData>,
}

#[derive(Clone, Debug, Default)]
pub struct BlockProfile {
    pub callers: std::collections::HashMap<vm_core::GuestAddr, usize>,
    pub callees: std::collections::HashMap<vm_core::GuestAddr, usize>,
    pub execution_count: u64,
}

#[derive(Clone, Debug)]
pub struct ProfileData {
    // stub fields
    pub total_runs: u64,
    pub block_profiles: std::collections::HashMap<vm_core::GuestAddr, BlockProfile>,
}

impl ProfileCollector {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            inner: Mutex::new(ProfileData {
                total_runs: 0,
                block_profiles: std::collections::HashMap::new(),
            }),
        }
    }

    pub fn start(&self) {
        // stub: start collecting profiles
    }

    pub fn record_block_call(&self, from: vm_core::GuestAddr, to: vm_core::GuestAddr) {
        let mut data = self.inner.lock().unwrap();
        let entry = data.block_profiles.entry(to).or_default();
        *entry.callers.entry(from).or_insert(0) += 1;
    }

    pub fn record_block_execution(&self, pc: vm_core::GuestAddr, _exec_time_ns: u64) {
        let mut data = self.inner.lock().unwrap();
        let entry = data.block_profiles.entry(pc).or_default();
        entry.execution_count += 1;
        data.total_runs += 1;
    }

    pub fn record_branch(&self, pc: vm_core::GuestAddr, target: vm_core::GuestAddr, taken: bool) {
        let mut data = self.inner.lock().unwrap();
        let entry = data.block_profiles.entry(pc).or_default();
        if taken {
            *entry.callees.entry(target).or_insert(0) += 1;
        }
    }

    pub fn record_function_call(
        &self,
        target: vm_core::GuestAddr,
        caller: Option<vm_core::GuestAddr>,
        _exec_time_ns: u64,
    ) {
        let mut data = self.inner.lock().unwrap();
        let entry = data.block_profiles.entry(target).or_default();
        if let Some(c) = caller {
            *entry.callers.entry(c).or_insert(0) += 1;
        }
    }

    pub fn snapshot(&self) -> ProfileData {
        self.inner.lock().unwrap().clone()
    }

    pub fn serialize_to_file<P: AsRef<std::path::Path>>(
        &self,
        _path: P,
    ) -> Result<(), std::io::Error> {
        Ok(())
    }

    pub fn get_profile_data(&self) -> ProfileData {
        self.snapshot()
    }
}
