//! AOT Loader
//!
//! 负责加载 AOT 镜像

use crate::jit::aot::format::AotImage;
use crate::jit::executable_memory::ExecutableMemory;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use vm_core::GuestAddr;

/// 代码块条目
#[derive(Debug)]
pub struct BlockEntry {
    pub guest_pc: GuestAddr,
    pub host_addr: *const u8,
    pub size: usize,
    // 可执行内存，防止释放
    _exec_mem: Option<Box<ExecutableMemory>>,
}

unsafe impl Send for BlockEntry {}
unsafe impl Sync for BlockEntry {}

/// AOT 加载器
pub struct AotLoader {
    image: AotImage,
    // 映射: guest_pc -> BlockEntry
    block_map: HashMap<GuestAddr, BlockEntry>,
}

impl AotLoader {
    /// 从文件加载 AOT 镜像
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = std::fs::File::open(path)?;
        let image = AotImage::deserialize(&mut file)?;
        let mut loader = Self {
            image,
            block_map: HashMap::new(),
        };
        loader.build_map();
        Ok(loader)
    }

    /// 构建内存映射
    fn build_map(&mut self) {
        for section in &self.image.sections {
            if let Some(mut exec_mem) = ExecutableMemory::new(section.data.len()) {
                let slice = exec_mem.as_mut_slice();
                slice.copy_from_slice(&section.data);

                if exec_mem.make_executable() {
                    exec_mem.invalidate_icache();

                    let entry = BlockEntry {
                        guest_pc: section.addr,
                        host_addr: exec_mem.as_mut_slice().as_ptr(),
                        size: section.data.len(),
                        _exec_mem: Some(Box::new(exec_mem)),
                    };
                    self.block_map.insert(section.addr, entry);
                }
            }
        }
    }

    /// 查找代码块
    pub fn lookup_block(&self, pc: GuestAddr) -> Option<&BlockEntry> {
        self.block_map.get(&pc)
    }

    /// 遍历所有代码块
    pub fn iter_blocks(&self) -> impl Iterator<Item = &BlockEntry> {
        self.block_map.values()
    }

    /// 获取代码块数量
    pub fn code_block_count(&self) -> usize {
        self.block_map.len()
    }
}
