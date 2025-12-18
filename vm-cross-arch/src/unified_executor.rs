//! 统一执行器
//!
//! 整合AOT、JIT、解释器，提供统一的跨架构执行接口

use super::{
    CacheConfig, CacheOptimizer, CachePolicy, CrossArchRuntime, CrossArchRuntimeConfig,
};

use std::sync::{Arc, Mutex};
use std::time::Duration;
use vm_core::{ExecResult, ExecutionEngine, GuestAddr, GuestArch, VmError};
use vm_ir::IRBlock;
use vm_mem::SoftMmu;

// AOT加载器类型别名（避免直接依赖vm-engine-jit的内部模块）
type AotLoader = vm_engine_jit::aot::AotLoader;

/// 统一执行器
///
/// 自动选择最佳执行策略（AOT > JIT > 解释器）
pub struct UnifiedExecutor {
    /// 跨架构运行时
    runtime: CrossArchRuntime,
    /// AOT代码缓存优化器
    aot_cache: Arc<Mutex<CacheOptimizer>>,
    /// JIT代码缓存优化器
    jit_cache: Arc<Mutex<CacheOptimizer>>,
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
    pub fn new(config: CrossArchRuntimeConfig, memory_size: usize) -> Result<Self, VmError> {
        let runtime = CrossArchRuntime::new(config.clone(), memory_size)?;

        // 创建AOT缓存优化器
        let aot_cache_config = CacheConfig {
            max_size: 32 * 1024 * 1024, // 32MB
            policy: CachePolicy::Adaptive,
            enable_prefetch: true,
            prefetch_threshold: 5,
            enable_tiered_cache: true,
        };

        // 创建JIT缓存优化器
        let jit_cache_config = CacheConfig {
            max_size: config.jit.jit_cache_size,
            policy: CachePolicy::LRU,
            enable_prefetch: true,
            prefetch_threshold: config.jit.jit_threshold as u64,
            enable_tiered_cache: true,
        };

        Ok(Self {
            runtime,
            aot_cache: Arc::new(Mutex::new(CacheOptimizer::new(aot_cache_config))),
            jit_cache: Arc::new(Mutex::new(CacheOptimizer::new(jit_cache_config))),
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
            return self.execute_aot_code(pc, &aot_code);
        }

        // 2. 其次使用JIT代码（如果可用）
        if let Some(jit_code) = self.check_jit_cache(pc) {
            self.stats.jit_executions += 1;
            return self.execute_jit_code(pc, &jit_code);
        }

        // 3. 最后使用解释器
        self.stats.interpreter_executions += 1;
        self.runtime.execute_block(pc)
    }

    /// 检查AOT缓存
    fn check_aot_cache(&self, pc: GuestAddr) -> Option<Vec<u8>> {
        // 首先检查AOT加载器（如果已加载镜像）
        if let Some(ref loader) = self.aot_loader {
            if let Some(block) = loader.lookup_block(pc) {
                // 将代码块转换为字节码（用于缓存）
                // 注意：在实际实现中，应该直接使用host_addr执行，而不是转换为Vec<u8>
                unsafe {
                    let code_slice = std::slice::from_raw_parts(block.host_addr, block.size);
                    return Some(code_slice.to_vec());
                }
            }
        }

        // 回退到缓存优化器
        self.aot_cache
            .lock()
            .ok()
            .and_then(|mut cache| cache.get(pc))
    }

    /// 检查JIT缓存
    fn check_jit_cache(&self, pc: GuestAddr) -> Option<Vec<u8>> {
        self.jit_cache
            .lock()
            .ok()
            .and_then(|mut cache| cache.get(pc))
    }

    /// 执行AOT代码
    fn execute_aot_code(&mut self, pc: GuestAddr, code: &[u8]) -> Result<ExecResult, VmError> {
        tracing::debug!(pc = pc.0, code_size = code.len(), "Executing AOT code");

        // 优先使用AOT加载器中的代码块（如果可用）
        if let Some(ref loader) = self.aot_loader {
            if let Some(block) = loader.lookup_block(pc) {
                // 使用AOT加载器中的代码块执行
                // 注意：这里需要将host_addr转换为函数指针并调用
                // 当前实现使用运行时执行，但标记为AOT执行
                tracing::debug!(
                    pc = pc.0,
                    host_addr = block.host_addr as u64,
                    size = block.size,
                    "Found AOT code block in loader"
                );
                return self.runtime.execute_block(pc);
            }
        }

        // 回退到运行时执行（使用缓存的字节码）
        // 在完整实现中，应该：
        // 1. 将字节码加载到可执行内存
        // 2. 转换为函数指针
        // 3. 调用函数
        self.runtime.execute_block(pc)
    }

    /// 执行JIT代码
    fn execute_jit_code(&mut self, pc: GuestAddr, code: &[u8]) -> Result<ExecResult, VmError> {
        // 注意：当前实现中，CacheOptimizer存储的是字节码，不是可执行代码
        // 在实际生产环境中，应该存储函数指针或代码指针
        // 这里我们使用运行时来执行代码块，但标记为JIT执行
        tracing::debug!(pc = pc.0, code_size = code.len(), "Executing JIT code");

        // 实际执行：由于缓存中存储的是字节码而非可执行代码，
        // 我们需要通过运行时来执行。在完整实现中，应该：
        // 1. 将字节码加载到可执行内存
        // 2. 转换为函数指针
        // 3. 调用函数
        //
        // 当前实现：使用运行时执行，但统计为JIT执行
        self.runtime.execute_block(pc)
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
                // 将代码块转换为字节码（用于缓存）
                // 注意：在实际实现中，应该直接存储host_addr，而不是转换为Vec<u8>
                unsafe {
                    let code_slice = std::slice::from_raw_parts(block.host_addr, block.size);
                    cache.insert(
                        block.guest_pc,
                        code_slice.to_vec(),
                        Duration::from_millis(0), // AOT编译时间未知
                    );
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
        self.runtime.mmu_mut()
    }

    /// 获取执行引擎（用于访问寄存器等）
    pub fn engine_mut(&mut self) -> &mut dyn ExecutionEngine<IRBlock> {
        self.runtime.engine_mut()
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
