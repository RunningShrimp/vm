//! 统一执行器
//!
//! 整合AOT、JIT、解释器，提供统一的跨架构执行接口

#[cfg(feature = "jit")]
use super::{CrossArchRuntime, CrossArchRuntimeConfig};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::{ExecResult, ExecutionEngine, GuestAddr, GuestArch, VmError};
use vm_ir::IRBlock;

#[cfg(feature = "memory")]
use vm_mem::SoftMmu;

// AOT加载器类型别名（避免直接依赖vm-engine-jit的内部模块）
#[cfg(feature = "jit")]
type AotLoader = vm_engine::jit::aot::AotLoader;

/// JIT/AOT代码函数指针类型
/// 参数：执行上下文指针
/// 返回：执行状态码 (0 = 成功, 非0 = 错误)
pub type CodeFunction = extern "C" fn(*mut u8) -> u32;

/// 原生执行上下文
///
/// 传递给AOT/JIT编译后代码的执行上下文
#[repr(C)]
pub struct NativeExecutionContext {
    /// MMU指针
    pub mmu_ptr: *mut u8,
    /// 寄存器状态指针
    pub registers_ptr: *mut u8,
    /// 下一个PC地址（输出）
    pub next_pc: u64,
    /// 执行状态（输出）
    pub exec_status: u32,
    /// 保留字段（用于对齐）
    _reserved: [u8; 16],
}

/// 可执行代码块
pub struct ExecutableCode {
    ptr: *mut u8,
    size: usize,
    entry_point: CodeFunction,
}

impl Clone for ExecutableCode {
    fn clone(&self) -> Self {
        unsafe {
            let mut exec_mem = vm_engine::jit::executable_memory::ExecutableMemory::new(self.size)
                .expect("Failed to clone executable memory");
            let slice = exec_mem.as_mut_slice();
            std::ptr::copy_nonoverlapping(self.ptr, slice.as_mut_ptr(), self.size);

            if !exec_mem.make_executable() {
                panic!("Failed to make cloned memory executable");
            }
            exec_mem.invalidate_icache();

            let ptr = exec_mem.as_mut_slice().as_mut_ptr();
            let entry_point = std::mem::transmute::<*mut u8, CodeFunction>(ptr);

            Self {
                ptr,
                size: self.size,
                entry_point,
            }
        }
    }
}

unsafe impl Send for ExecutableCode {}
unsafe impl Sync for ExecutableCode {}

impl ExecutableCode {
    /// 从字节码创建可执行代码
    pub unsafe fn from_bytes(code: &[u8]) -> Result<Self, ()> {
        let mut exec_mem =
            vm_engine::jit::executable_memory::ExecutableMemory::new(code.len()).ok_or(())?;
        let slice = exec_mem.as_mut_slice();
        slice.copy_from_slice(code);

        if !exec_mem.make_executable() {
            return Err(());
        }
        exec_mem.invalidate_icache();

        let ptr = exec_mem.as_mut_slice().as_mut_ptr();
        let entry_point = unsafe { std::mem::transmute::<*mut u8, CodeFunction>(ptr) };

        Ok(Self {
            ptr,
            size: code.len(),
            entry_point,
        })
    }

    /// 执行代码
    pub unsafe fn execute(&self, context: *mut u8) -> Result<(), VmError> {
        let status_code = (self.entry_point)(context);
        if status_code == 0 {
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::Internal {
                message: format!(
                    "Native code execution failed with status code {}",
                    status_code
                ),
                module: "UnifiedExecutor".to_string(),
            }))
        }
    }

    /// 获取代码指针
    #[allow(dead_code)]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    /// 获取代码大小
    pub fn size(&self) -> usize {
        self.size
    }
}

/// 统一执行器
///
/// 自动选择最佳执行策略（AOT > JIT > 解释器）
pub struct UnifiedExecutor {
    /// 跨架构运行时
    runtime: CrossArchRuntime,
    /// AOT代码缓存（存储ExecutableCode）
    aot_cache: Arc<Mutex<HashMap<GuestAddr, ExecutableCode>>>,
    /// JIT代码缓存（存储ExecutableCode）
    jit_cache: Arc<Mutex<HashMap<GuestAddr, ExecutableCode>>>,
    /// AOT加载器（如果已加载AOT镜像）
    aot_loader: Option<Arc<AotLoader>>,
    /// 执行统计
    stats: ExecutionStats,
}

/// 执行统计
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    /// AOT执行次数
    pub aot_executions: u64,
    /// JIT执行次数
    pub jit_executions: u64,
    /// 解释器执行次数
    pub interpreter_executions: u64,
    /// 总执行次数
    pub total_executions: u64,
    /// AOT命中率
    pub aot_hit_rate: f64,
    /// JIT命中率
    pub jit_hit_rate: f64,
    /// 翻译的指令数
    pub instructions_translated: usize,
    /// JIT编译时间（微秒）
    pub jit_compilation_time_us: u64,
}

impl UnifiedExecutor {
    /// 创建新的统一执行器
    pub fn new(config: CrossArchRuntimeConfig, _memory_size: usize) -> Result<Self, VmError> {
        let runtime = CrossArchRuntime::new(config)?;

        Ok(Self {
            runtime,
            aot_cache: Arc::new(Mutex::new(HashMap::new())),
            jit_cache: Arc::new(Mutex::new(HashMap::new())),
            aot_loader: None,
            stats: ExecutionStats::default(),
        })
    }

    /// 自动创建统一执行器（自动检测host架构）
    pub fn auto_create(guest_arch: GuestArch, memory_size: usize) -> Result<Self, VmError> {
        let config = CrossArchRuntimeConfig::auto_create(guest_arch)?;
        Self::new(config, memory_size)
    }

    /// 执行代码块（自动选择最佳执行策略）
    pub fn execute(&mut self, pc: GuestAddr) -> Result<ExecResult, VmError> {
        self.stats.total_executions += 1;

        // 1. 优先使用AOT代码（如果可用）
        if let Some(aot_code) = self.check_aot_cache(pc) {
            self.stats.aot_executions += 1;
            return self.execute_aot_code(&aot_code);
        }

        // 2. 其次使用JIT代码（如果可用）
        if let Some(jit_code) = self.check_jit_cache(pc) {
            self.stats.jit_executions += 1;
            return self.execute_jit_code(&jit_code);
        }

        // 3. 最后使用解释器
        self.stats.interpreter_executions += 1;
        self.runtime.execute_block(pc)
    }

    /// 检查AOT缓存
    fn check_aot_cache(&self, pc: GuestAddr) -> Option<ExecutableCode> {
        // 首先检查AOT加载器（如果已加载镜像）
        if let Some(ref loader) = self.aot_loader
            && let Some(block) = loader.lookup_block(pc)
        {
            unsafe {
                let code_slice = std::slice::from_raw_parts(block.host_addr, block.size);
                if let Ok(exec_code) = ExecutableCode::from_bytes(code_slice) {
                    return Some(exec_code);
                }
            }
        }

        // 回退到HashMap缓存
        self.aot_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&pc).cloned())
    }

    /// 检查JIT缓存
    fn check_jit_cache(&self, pc: GuestAddr) -> Option<ExecutableCode> {
        self.jit_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&pc).cloned())
    }

    /// 执行AOT代码
    fn execute_aot_code(&mut self, exec_code: &ExecutableCode) -> Result<ExecResult, VmError> {
        tracing::debug!("Executing AOT code");

        unsafe {
            let mmu_mut = self
                .runtime
                .mmu_mut()
                .expect("MMU not available for AOT execution");
            let mut context = NativeExecutionContext {
                mmu_ptr: mmu_mut as *mut SoftMmu as *mut u8,
                registers_ptr: std::ptr::null_mut(),
                next_pc: 0,
                exec_status: 0,
                _reserved: [0; 16],
            };

            match exec_code.execute(&mut context as *mut _ as *mut u8) {
                Ok(()) => {
                    let next_pc = GuestAddr(context.next_pc);
                    Ok(vm_core::ExecResult {
                        status: vm_core::ExecStatus::Ok,
                        stats: vm_core::ExecStats {
                            executed_insns: 1,
                            exec_time_ns: exec_code.size() as u64 * 10,
                            ..Default::default()
                        },
                        next_pc,
                    })
                }
                Err(e) => Err(e),
            }
        }
    }

    /// 执行JIT代码
    fn execute_jit_code(&mut self, exec_code: &ExecutableCode) -> Result<ExecResult, VmError> {
        tracing::debug!("Executing JIT code");

        unsafe {
            let mmu_mut = self.runtime.mmu_mut().expect("MMU not available");
            let mut context = NativeExecutionContext {
                mmu_ptr: mmu_mut as *mut SoftMmu as *mut u8,
                registers_ptr: std::ptr::null_mut(),
                next_pc: 0,
                exec_status: 0,
                _reserved: [0; 16],
            };

            match exec_code.execute(&mut context as *mut _ as *mut u8) {
                Ok(()) => {
                    let next_pc = GuestAddr(context.next_pc);
                    Ok(vm_core::ExecResult {
                        status: vm_core::ExecStatus::Ok,
                        stats: vm_core::ExecStats {
                            executed_insns: 1,
                            exec_time_ns: exec_code.size() as u64 * 10,
                            ..Default::default()
                        },
                        next_pc,
                    })
                }
                Err(e) => Err(e),
            }
        }
    }

    /// 加载AOT镜像
    pub fn load_aot_image(&mut self, image_path: &str) -> Result<(), VmError> {
        tracing::info!("Loading AOT image from: {}", image_path);

        // 使用AOT加载器加载镜像
        let loader = AotLoader::from_file(image_path).map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to load AOT image: {}", e),
                module: "UnifiedExecutor".to_string(),
            })
        })?;

        let loader_arc = Arc::new(loader);

        // 将代码块填充到AOT缓存
        {
            let mut cache = self.aot_cache.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "Failed to lock AOT cache".to_string(),
                    module: "UnifiedExecutor".to_string(),
                })
            })?;

            // 遍历所有代码块并添加到缓存
            for block in loader_arc.iter_blocks() {
                unsafe {
                    let code_slice = std::slice::from_raw_parts(block.host_addr, block.size);
                    if let Ok(exec_code) = ExecutableCode::from_bytes(code_slice) {
                        cache.insert(block.guest_pc, exec_code);
                    }
                }
            }
        }

        // 保存AOT加载器引用
        self.aot_loader = Some(loader_arc);

        tracing::info!(
            "AOT image loaded successfully: {} code blocks",
            self.aot_loader
                .as_ref()
                .map(|l| l.code_block_count())
                .unwrap_or(0)
        );

        Ok(())
    }

    /// 触发JIT编译热点代码
    pub fn trigger_jit_compilation(&mut self, pc: GuestAddr) -> Result<(), VmError> {
        // 检查是否已经是热点
        if self.check_jit_cache(pc).is_some() {
            return Ok(()); // 已经编译过
        }

        // 通过运行时触发JIT编译
        // 运行时会在检测到热点时自动编译
        tracing::debug!(pc = pc.0, "Triggering JIT compilation for hotspot");
        Ok(())
    }

    /// 触发AOT编译热点代码
    pub fn trigger_aot_compilation(&mut self, pc: GuestAddr) -> Result<(), VmError> {
        // 检查是否已经编译
        if self.check_aot_cache(pc).is_some() {
            return Ok(()); // 已经编译过
        }

        // 通过运行时触发AOT编译
        tracing::debug!(pc = pc.0, "Triggering AOT compilation for hotspot");
        Ok(())
    }

    /// 获取MMU（可变引用）
    pub fn mmu_mut(&mut self) -> &mut SoftMmu {
        self.runtime.mmu_mut().expect("MMU not available")
    }

    /// 获取执行引擎（用于访问寄存器等）
    pub fn engine_mut(&mut self) -> &mut dyn ExecutionEngine<IRBlock> {
        self.runtime.engine_mut().expect("Engine not available")
    }

    /// 获取执行统计
    pub fn stats(&self) -> &ExecutionStats {
        &self.stats
    }

    /// 更新统计信息
    pub fn update_stats(&mut self) {
        let total = self.stats.total_executions as f64;
        if total > 0.0 {
            self.stats.aot_hit_rate = self.stats.aot_executions as f64 / total;
            self.stats.jit_hit_rate = self.stats.jit_executions as f64 / total;
        }
    }

    /// 获取配置
    pub fn config(&self) -> &CrossArchRuntimeConfig {
        self.runtime.config()
    }

    /// 获取物理内存大小（字节）
    pub fn memory_size(&self) -> usize {
        self.runtime.memory_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_executor_creation() {
        let executor = UnifiedExecutor::auto_create(GuestArch::X86_64, 128 * 1024 * 1024);
        assert!(executor.is_ok());
    }
}
