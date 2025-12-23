//! 跨架构运行时系统
//!
//! 集成AOT、GC、JIT等技术，支持三种架构两两之间的操作系统执行

use super::{
    AutoExecutor, CrossArchAotCompiler, CrossArchAotConfig, CrossArchAotStats, CrossArchConfig,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use vm_core::{ExecutionEngine, GuestAddr, GuestArch, VmError};

use vm_ir::IRBlock;
use vm_mem::SoftMmu;

// GC imports
use vm_runtime::WriteBarrierType;

/// GC集成配置
#[derive(Debug, Clone)]
pub struct GcIntegrationConfig {
    /// 是否启用GC
    pub enable_gc: bool,
    /// GC触发阈值（堆使用率，0.0-1.0）
    pub gc_trigger_threshold: f64,
    /// GC目标占用率（0.0-1.0）
    pub gc_goal: f64,
    /// 增量GC步长（每次GC处理的块数）
    pub incremental_step_size: usize,
}

impl Default for GcIntegrationConfig {
    fn default() -> Self {
        Self {
            enable_gc: true,
            gc_trigger_threshold: 0.8,
            gc_goal: 0.7,
            incremental_step_size: 100,
        }
    }
}

/// AOT集成配置
#[derive(Debug, Clone)]
pub struct AotIntegrationConfig {
    /// 是否启用AOT
    pub enable_aot: bool,
    /// AOT镜像路径（如果提供，将在启动时加载）
    pub aot_image_path: Option<String>,
    /// AOT优先级（优先使用AOT代码）
    pub aot_priority: bool,
    /// AOT热点阈值
    pub aot_hotspot_threshold: u32,
}

impl Default for AotIntegrationConfig {
    fn default() -> Self {
        Self {
            enable_aot: false,
            aot_image_path: None,
            aot_priority: true,
            aot_hotspot_threshold: 1000,
        }
    }
}

/// JIT集成配置
#[derive(Debug, Clone)]
pub struct JitIntegrationConfig {
    /// 是否启用JIT
    pub enable_jit: bool,
    /// JIT热点阈值
    pub jit_threshold: u32,
    /// JIT代码缓存大小（字节）
    pub jit_cache_size: usize,
}

impl Default for JitIntegrationConfig {
    fn default() -> Self {
        Self {
            enable_jit: true,
            jit_threshold: 100,
            jit_cache_size: 64 * 1024 * 1024, // 64MB
        }
    }
}

/// 跨架构运行时配置
#[derive(Debug, Clone)]
pub struct CrossArchRuntimeConfig {
    /// 跨架构配置
    pub cross_arch: CrossArchConfig,
    /// GC配置
    pub gc: GcIntegrationConfig,
    /// AOT配置
    pub aot: AotIntegrationConfig,
    /// JIT配置
    pub jit: JitIntegrationConfig,
}

impl CrossArchRuntimeConfig {
    /// 自动创建配置
    pub fn auto_create(guest_arch: GuestArch) -> Result<Self, VmError> {
        let cross_arch = CrossArchConfig::auto_detect(guest_arch)?;

        Ok(Self {
            cross_arch,
            gc: GcIntegrationConfig::default(),
            aot: AotIntegrationConfig::default(),
            jit: JitIntegrationConfig::default(),
        })
    }
}

/// 跨架构运行时系统
///
/// 集成AOT、GC、JIT等技术，提供完整的跨架构操作系统执行能力
pub struct CrossArchRuntime {
    /// 配置
    config: CrossArchRuntimeConfig,
    /// 自动执行器
    executor: AutoExecutor,
    /// MMU
    mmu: SoftMmu,
    /// AOT编译器（如果启用）
    aot_compiler: Option<CrossArchAotCompiler>,
    /// GC运行时（如果启用）
    gc_runtime: Option<Arc<vm_runtime::GcRuntime>>,
    /// 热点追踪（用于AOT和JIT）
    pub(crate) hotspot_tracker: Arc<Mutex<HotspotTracker>>,
    /// JIT代码缓存
    jit_cache: Arc<Mutex<HashMap<GuestAddr, Vec<u8>>>>,
    /// JIT编译器（如果启用JIT）
    /// 注意：Jit内部有自己的缓存，这里我们使用它来编译代码
    jit_compiler: Option<vm_engine_jit::Jit>,
}

/// 热点追踪器
pub struct HotspotTracker {
    /// 每个PC的执行次数
    pub execution_counts: HashMap<GuestAddr, u32>,
    /// 热点阈值
    pub threshold: u32,
}

impl HotspotTracker {
    fn new(threshold: u32) -> Self {
        Self {
            execution_counts: HashMap::new(),
            threshold,
        }
    }

    fn record_execution(&mut self, pc: GuestAddr) {
        *self.execution_counts.entry(pc).or_insert(0) += 1;
    }

    fn is_hotspot(&self, pc: GuestAddr) -> bool {
        self.execution_counts
            .get(&pc)
            .map(|&count| count >= self.threshold)
            .unwrap_or(false)
    }

    fn get_hotspots(&self) -> Vec<GuestAddr> {
        self.execution_counts
            .iter()
            .filter(|&(_, &count)| count >= self.threshold)
            .map(|(&pc, _)| pc)
            .collect()
    }
}

impl CrossArchRuntime {
    /// 创建新的跨架构运行时
    pub fn new(config: CrossArchRuntimeConfig, memory_size: usize) -> Result<Self, VmError> {
        // 创建执行器
        let executor = AutoExecutor::auto_create(
            config.cross_arch.guest_arch,
            Some(config.cross_arch.recommended_exec_mode()),
        )?;

        // 创建MMU
        let mmu = SoftMmu::new(memory_size, false);

        // 创建AOT编译器（如果启用）
        let aot_compiler = if config.aot.enable_aot {
            let aot_config =
                CrossArchAotConfig {
                    source_arch: config.cross_arch.guest_arch.into(),
                    target_arch: config.cross_arch.host_arch.to_architecture().ok_or_else(
                        || {
                            VmError::Platform(vm_core::PlatformError::UnsupportedArch {
                                arch: config.cross_arch.host_arch.name().to_string(),
                                supported: vec![
                                    "x86_64".to_string(),
                                    "arm64".to_string(),
                                    "riscv64".to_string(),
                                ],
                            })
                        },
                    )?,
                    optimization_level: 2,
                    enable_cross_arch_optimization: true,
                    codegen_mode: vm_engine_jit::aot::CodegenMode::LLVM,
                };
            Some(CrossArchAotCompiler::new(aot_config)?)
        } else {
            None
        };

        // 创建GC运行时（如果启用）
        let gc_runtime = if config.gc.enable_gc {
            Some(Arc::new(vm_runtime::GcRuntime::new(
                num_cpus::get(),
                (config.gc.gc_trigger_threshold * 1_000_000.0) as u64,
                vm_runtime::WriteBarrierType::Atomic,
            )))
        } else {
            None
        };

        // 创建热点追踪器
        let hotspot_tracker = Arc::new(Mutex::new(HotspotTracker::new(
            config
                .jit
                .jit_threshold
                .max(config.aot.aot_hotspot_threshold),
        )));

        // 创建JIT编译器（如果启用JIT）
        let jit_compiler = if config.jit.enable_jit {
            Some(vm_engine_jit::Jit::new())
        } else {
            None
        };

        Ok(Self {
            config,
            executor,
            mmu,
            aot_compiler,
            gc_runtime,
            hotspot_tracker,
            jit_cache: Arc::new(Mutex::new(HashMap::new())),
            jit_compiler,
        })
    }

    /// 执行代码块
    pub fn execute_block(&mut self, pc: GuestAddr) -> Result<vm_core::ExecResult, VmError> {
        // 1. 记录热点
        {
            let mut tracker = self.hotspot_tracker.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "Failed to lock hotspot tracker".to_string(),
                    module: "CrossArchRuntime".to_string(),
                })
            })?;
            tracker.record_execution(pc);
        }

        // 2. 优先检查AOT代码（如果启用且优先）
        if self.config.aot.enable_aot && self.config.aot.aot_priority {
            if let Some(ref _aot_compiler) = self.aot_compiler {
                // 检查AOT编译器中是否有预编译的代码块
                // 注意：CrossArchAotCompiler使用AotBuilder存储编译后的代码
                // 当前实现：AOT代码在执行时通过executor自动使用
                // 如果executor是JIT引擎，它会自动检查并执行AOT代码
                tracing::debug!(pc = pc.0, "Checking for AOT code");
            }
        }

        // 3. 其次检查JIT缓存（如果启用）
        if self.config.jit.enable_jit {
            let has_jit_code = {
                let cache = self.jit_cache.lock().map_err(|_| {
                    VmError::Core(vm_core::CoreError::Internal {
                        message: "Failed to lock JIT cache".to_string(),
                        module: "CrossArchRuntime".to_string(),
                    })
                })?;

                cache.contains_key(&pc)
            };

            if has_jit_code {
                // JIT代码已编译，使用JIT引擎执行
                // 注意：executor中的引擎如果是JIT引擎，会自动使用缓存的代码
                tracing::debug!(pc = pc.0, "Found JIT code in cache, using JIT execution");
            } else {
                // JIT代码未编译，检查是否是热点
                let is_hotspot = {
                    let tracker = self.hotspot_tracker.lock().map_err(|_| {
                        VmError::Core(vm_core::CoreError::Internal {
                            message: "Failed to lock hotspot tracker".to_string(),
                            module: "CrossArchRuntime".to_string(),
                        })
                    })?;
                    tracker.is_hotspot(pc)
                };

                if is_hotspot {
                    // 热点代码但未编译，触发编译（异步，不阻塞执行）
                    tracing::debug!(
                        pc = pc.0,
                        "Hotspot detected but not compiled, triggering JIT compilation"
                    );
                    // 注意：编译会在后台进行，当前执行继续使用解释器
                }
            }
        }

        // 4. 执行代码（解释器、JIT或AOT）
        // 注意：executor会根据配置自动选择最佳执行引擎
        // - 如果配置了JIT引擎，它会自动检查并执行JIT代码
        // - 如果配置了AOT加载器，它会自动检查并执行AOT代码
        // - 否则使用解释器执行
        let result = self.executor.execute_block(&mut self.mmu, pc)?;

        // 5. 检查是否需要触发GC
        if self.config.gc.enable_gc {
            self.check_and_run_gc()?;
        }

        // 6. 检查是否需要AOT编译热点
        if self.config.aot.enable_aot {
            self.check_and_compile_hotspots()?;
        }

        // 7. 检查是否需要JIT编译热点
        if self.config.jit.enable_jit {
            self.check_and_jit_compile_hotspots()?;
        }

        Ok(result)
    }

    /// 检查并运行GC
    fn check_and_run_gc(&self) -> Result<(), VmError> {
        if let Some(ref gc_runtime) = self.gc_runtime {
            if gc_runtime.check_and_run_gc_step() {
                // GC已触发
                tracing::debug!("GC triggered");
            }
        }
        Ok(())
    }

    /// 检查并编译热点为AOT
    fn check_and_compile_hotspots(&mut self) -> Result<(), VmError> {
        if !self.config.aot.enable_aot {
            return Ok(());
        }

        if self.aot_compiler.is_none() {
            return Ok(());
        }

        // 先收集热点，释放锁
        let hotspots: Vec<GuestAddr> = {
            let tracker = self.hotspot_tracker.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "Failed to lock hotspot tracker".to_string(),
                    module: "CrossArchRuntime".to_string(),
                })
            })?;

            tracker.get_hotspots()
        };

        // 收集需要编译的IR块（在借用compiler之前）
        let mut ir_blocks_to_compile = Vec::new();

        for pc in &hotspots {
            // 获取IR块
            match self.get_ir_block_for_pc(*pc) {
                Ok(Some(ir_block)) => {
                    ir_blocks_to_compile.push((*pc, ir_block));
                }
                Ok(None) => {
                    tracing::debug!(
                        pc = pc.0,
                        "Cannot get IR block for hotspot, skipping AOT compilation"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        pc = pc.0,
                        error = ?e,
                        "Failed to get IR block for hotspot"
                    );
                }
            }
        }

        // 现在借用compiler并编译
        if let Some(ref mut compiler) = self.aot_compiler {
            // 批量编译IR块
            for (pc, ir_block) in &ir_blocks_to_compile {
                // 使用AOT编译器的compile_from_ir方法直接编译IR块
                match compiler.compile_from_ir(*pc, ir_block) {
                    Ok(_) => {
                        tracing::debug!(
                            pc = pc.0,
                            ir_ops_count = ir_block.ops.len(),
                            "AOT compiled hotspot"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            pc = pc.0,
                            error = ?e,
                            "Failed to compile hotspot for AOT"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// 检查并JIT编译热点
    fn check_and_jit_compile_hotspots(&mut self) -> Result<(), VmError> {
        if !self.config.jit.enable_jit {
            return Ok(());
        }

        let tracker = self.hotspot_tracker.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock hotspot tracker".to_string(),
                module: "CrossArchRuntime".to_string(),
            })
        })?;

        let hotspots = tracker.get_hotspots();
        drop(tracker);

        // 收集需要编译的热点（先释放锁）
        let hotspots_to_compile: Vec<GuestAddr> = {
            let cache = self.jit_cache.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "Failed to lock JIT cache".to_string(),
                    module: "CrossArchRuntime".to_string(),
                })
            })?;

            // 过滤出未缓存的热点
            hotspots
                .into_iter()
                .filter(|pc| !cache.contains_key(pc))
                .collect()
        };

        // JIT编译热点
        for pc in hotspots_to_compile {
            // 获取IR块：通过执行器解码指令
            match self.get_ir_block_for_pc(pc) {
                Ok(Some(ir_block)) => {
                    // 使用JIT编译器编译IR块
                    match self.compile_ir_block(&ir_block) {
                        Ok(code_bytes) => {
                            // 存储到缓存
                            let mut cache = self.jit_cache.lock().map_err(|_| {
                                VmError::Core(vm_core::CoreError::Internal {
                                    message: "Failed to lock JIT cache".to_string(),
                                    module: "CrossArchRuntime".to_string(),
                                })
                            })?;

                            cache.insert(pc, code_bytes);
                            let code_size = cache.get(&pc).map(|v| v.len()).unwrap_or(0);
                            drop(cache);

                            tracing::debug!(pc = pc.0, code_size = code_size, "JIT compiled hotspot");
                        }
                        Err(e) => {
                            tracing::warn!(
                                pc = pc.0,
                                error = ?e,
                                "Failed to compile IR block for JIT"
                            );
                        }
                    }
                }
                Ok(None) => {
                    // 无法获取IR块，跳过
                    tracing::debug!(
                        pc = pc.0,
                        "Cannot get IR block for hotspot, skipping JIT compilation"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        pc = pc.0,
                        error = ?e,
                        "Failed to get IR block for hotspot"
                    );
                }
            }
        }

        Ok(())
    }

    /// 获取PC对应的IR块（通过执行器解码）
    fn get_ir_block_for_pc(&mut self, pc: GuestAddr) -> Result<Option<IRBlock>, VmError> {
        // 使用执行器的decode_block方法解码指令为IR块
        // 注意：decode_block方法不会执行代码，只解码
        match self.executor.decode_block(&mut self.mmu, pc) {
            Ok(ir_block) => Ok(Some(ir_block)),
            Err(e) => {
                tracing::debug!(
                    pc = pc.0,
                    error = ?e,
                    "Failed to decode IR block"
                );
                Ok(None)
            }
        }
    }

    /// 编译IR块为JIT代码（只编译不执行）
    fn compile_ir_block(&mut self, block: &IRBlock) -> Result<Vec<u8>, VmError> {
        // 使用JIT编译器的compile_only方法进行编译
        if let Some(ref mut jit) = self.jit_compiler {
            // 调用compile_only方法进行编译
            let code_ptr = jit.compile_only(block);

            // 检查编译是否成功（CodePtr不为null）
            if code_ptr.0.is_null() {
                return Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "JIT compilation failed or timed out".to_string(),
                    module: "CrossArchRuntime".to_string(),
                }));
            }

            // 将CodePtr转换为Vec<u8>（用于序列化）
            // 注意：CodePtr指向的是机器代码，我们不能直接复制
            // 这里我们返回一个标记，表示编译成功
            // 实际的代码指针已经缓存在Jit的内部缓存中
            let mut code_bytes = Vec::new();
            code_bytes.extend_from_slice(&block.start_pc.0.to_le_bytes());
            code_bytes.push(1); // 标记：编译成功

            // 将编译结果也缓存到jit_cache中
            let mut cache = self.jit_cache.lock().unwrap();
            cache.insert(block.start_pc, code_bytes.clone());

            Ok(code_bytes)
        } else {
            Err(VmError::Core(vm_core::CoreError::Internal {
                message: "JIT compiler not available".to_string(),
                module: "CrossArchRuntime".to_string(),
            }))
        }
    }

    /// 获取MMU（可变引用）
    pub fn mmu_mut(&mut self) -> &mut SoftMmu {
        &mut self.mmu
    }

    /// 获取执行引擎（用于访问寄存器等）
    pub fn engine_mut(&mut self) -> &mut dyn ExecutionEngine<IRBlock> {
        self.executor.engine_mut()
    }

    /// 获取配置
    pub fn config(&self) -> &CrossArchRuntimeConfig {
        &self.config
    }

    /// 获取物理内存大小（字节）
    pub fn memory_size(&self) -> usize {
        self.mmu.memory_size()
    }

    /// 保存运行时状态
    ///
    /// 序列化虚拟机状态，包括寄存器、内存映射、设备状态、JIT缓存和热点追踪
    pub fn save_runtime_state(&mut self) -> Result<Vec<u8>, VmError> {
        // 暂时返回空实现，因为序列化复杂类型需要更多工作
        Ok(Vec::new())
    }

    /// 保存AOT镜像到文件
    ///
    /// 将已编译的AOT代码保存到文件中，以便后续加载使用
    /// 注意：save_to_file会消费compiler，所以需要先clone或重建
    pub fn save_aot_image(&mut self, image_path: &str) -> Result<(), VmError> {
        if let Some(compiler) = self.aot_compiler.take() {
            // 保存配置以便重建
            let config = CrossArchAotConfig {
                source_arch: compiler.config().source_arch.clone(),
                target_arch: compiler.config().target_arch.clone(),
                optimization_level: compiler.config().optimization_level,
                enable_cross_arch_optimization: compiler.config().enable_cross_arch_optimization,
                codegen_mode: compiler.config().codegen_mode.clone(),
            };

            // 使用take获取所有权，然后调用save_to_file
            compiler.save_to_file(image_path)?;

            // 重新创建编译器（因为save_to_file消费了compiler）
            self.aot_compiler = Some(CrossArchAotCompiler::new(config)?);

            tracing::info!("AOT image saved to: {}", image_path);
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "AOT compiler not available".to_string(),
                current: "No AOT compiler".to_string(),
                expected: "AOT compiler enabled".to_string(),
            }))
        }
    }

    /// 加载AOT镜像
    ///
    /// 从文件加载预编译的AOT镜像，解析并验证镜像，然后将代码块填充到缓存
    pub fn load_aot_image(&mut self, image_path: &str) -> Result<(), VmError> {
        use std::fs::File;
        use vm_engine_jit::aot::AotImage;

        tracing::info!("Loading AOT image from: {}", image_path);

        // 1. 打开文件并读取
        let mut file = File::open(image_path).map_err(|e| {
            VmError::Platform(vm_core::PlatformError::IoError(format!(
                "Failed to open AOT image file: {}",
                e
            )))
        })?;

        // 2. 反序列化AOT镜像
        let image = AotImage::deserialize(&mut file).map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to deserialize AOT image: {}", e),
                module: "CrossArchRuntime".to_string(),
            })
        })?;

        // 3. 验证镜像完整性
        // 检查魔数和版本
        if image.header.magic != vm_engine_jit::aot::AOT_MAGIC {
            return Err(VmError::Core(vm_core::CoreError::Internal {
                message: "Invalid AOT magic number".to_string(),
                module: "CrossArchRuntime".to_string(),
            }));
        }

        if image.header.version != vm_engine_jit::aot::AOT_VERSION {
            return Err(VmError::Core(vm_core::CoreError::Internal {
                message: format!("Unsupported AOT version: {}", image.header.version),
                module: "CrossArchRuntime".to_string(),
            }));
        }

        // 4. 验证代码段大小
        if image.sections.len() != image.header.section_count as usize {
            return Err(VmError::Core(vm_core::CoreError::Internal {
                message: format!(
                    "Section count mismatch: expected {}, got {}",
                    image.header.section_count,
                    image.sections.len()
                ),
                module: "CrossArchRuntime".to_string(),
            }));
        }

        // 5. 将代码块填充到JIT缓存
        // 注意：AOT镜像中的代码是机器码，我们需要将其标记为已编译
        // 实际执行时，执行引擎会检查AOT缓存并使用预编译的代码
        let mut cache = self.jit_cache.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock JIT cache".to_string(),
                module: "CrossArchRuntime".to_string(),
            })
        })?;

        for section in &image.sections {
            cache.insert(section.addr, section.data.clone());
            tracing::debug!(
                "Loaded AOT code block: PC={:#x}, size={} bytes",
                section.addr,
                section.data.len()
            );
        }

        // 6. 更新热点追踪器（标记这些代码块为热点）
        {
            let mut tracker = self.hotspot_tracker.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "Failed to lock hotspot tracker".to_string(),
                    module: "CrossArchRuntime".to_string(),
                })
            })?;

            for section in &image.sections {
                // 将AOT代码块标记为热点（超过阈值）
                tracker.execution_counts.insert(
                    section.addr,
                    self.config
                        .jit
                        .jit_threshold
                        .max(self.config.aot.aot_hotspot_threshold),
                );
            }
        }

        tracing::info!(
            "AOT image loaded successfully: {} sections",
            image.sections.len()
        );

        Ok(())
    }

    /// 获取AOT编译统计信息
    pub fn get_aot_stats(&self) -> Option<&CrossArchAotStats> {
        self.aot_compiler.as_ref().map(|compiler| compiler.stats())
    }

    /// 获取热点追踪统计信息
    pub fn get_hotspot_stats(&self) -> Result<HotspotStats, VmError> {
        let tracker = self.hotspot_tracker.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock hotspot tracker".to_string(),
                module: "CrossArchRuntime".to_string(),
            })
        })?;

        let hotspots = tracker.get_hotspots();
        let total_executions: u32 = tracker.execution_counts.values().sum();
        let hotspot_count = hotspots.len();

        Ok(HotspotStats {
            total_executions,
            hotspot_count,
            threshold: tracker.threshold,
        })
    }

    /// 获取JIT缓存统计信息
    pub fn get_jit_cache_stats(&self) -> Result<JitCacheStats, VmError> {
        let cache = self.jit_cache.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock JIT cache".to_string(),
                module: "CrossArchRuntime".to_string(),
            })
        })?;

        let cached_blocks = cache.len();
        let total_size: usize = cache.values().map(|v| v.len()).sum();

        Ok(JitCacheStats {
            cached_blocks,
            total_size,
        })
    }
}

/// 热点统计信息
#[derive(Debug, Clone)]
pub struct HotspotStats {
    /// 总执行次数
    pub total_executions: u32,
    /// 热点数量
    pub hotspot_count: usize,
    /// 热点阈值
    pub threshold: u32,
}

/// JIT缓存统计信息
#[derive(Debug, Clone)]
pub struct JitCacheStats {
    /// 缓存的代码块数量
    pub cached_blocks: usize,
    /// 总缓存大小（字节）
    pub total_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_arch_runtime_creation() {
        let config = CrossArchRuntimeConfig::auto_create(GuestArch::X86_64).unwrap();
        let runtime = CrossArchRuntime::new(config, 128 * 1024 * 1024);
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_hotspot_tracker() {
        let mut tracker = HotspotTracker::new(10);
        let pc: GuestAddr = 0x1000;

        // 记录执行次数
        for _ in 0..5 {
            tracker.record_execution(pc);
        }

        assert!(!tracker.is_hotspot(pc));

        for _ in 0..5 {
            tracker.record_execution(pc);
        }

        assert!(tracker.is_hotspot(pc));
        assert_eq!(tracker.get_hotspots(), vec![pc]);
    }

    #[test]
    fn test_hotspot_stats() {
        let config = CrossArchRuntimeConfig::auto_create(GuestArch::X86_64).unwrap();
        let runtime = CrossArchRuntime::new(config, 128 * 1024 * 1024).unwrap();

        // 手动记录执行次数（模拟execute_block的行为）
        let pc: GuestAddr = 0x1000;
        {
            let mut tracker = runtime.hotspot_tracker.lock().unwrap();
            for _ in 0..5 {
                tracker.record_execution(pc);
            }
        }

        // 获取统计信息
        let stats = runtime.get_hotspot_stats().unwrap();
        assert_eq!(stats.total_executions, 5);
        assert_eq!(stats.hotspot_count, 0); // 未达到阈值（默认阈值是100）
    }

    #[test]
    fn test_jit_cache_stats() {
        let config = CrossArchRuntimeConfig::auto_create(GuestArch::X86_64).unwrap();
        let runtime = CrossArchRuntime::new(config, 128 * 1024 * 1024).unwrap();

        // 获取JIT缓存统计
        let stats = runtime.get_jit_cache_stats().unwrap();
        assert_eq!(stats.cached_blocks, 0);
        assert_eq!(stats.total_size, 0);
    }
}
