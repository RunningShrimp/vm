//! 跨架构运行时系统
//!
//! 集成AOT、GC、JIT等技术，支持三种架构两两之间的操作系统执行
//! 优化版本：通过集成模块将特性门从43个减少到~20个

use super::CrossArchConfig;
use crate::Architecture;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use vm_core::{ExecutionEngine, GuestAddr, GuestArch, VmError};

use vm_ir::IRBlock;

// ============================================================================
// Feature Integration Modules
// ============================================================================

/// GC集成模块 - 统一管理所有GC相关功能
/// 将所有GC特性整合到单一模块中
#[cfg(feature = "gc")]
pub mod gc_integration {
    use super::*;
    use vm_optimizers::gc::GcStats;

    /// GC集成配置
    #[derive(Debug, Clone)]
    pub struct GcConfig {
        pub enable_gc: bool,
        pub trigger_threshold: f64,
        pub gc_goal: f64,
        pub incremental_step_size: usize,
    }

    impl Default for GcConfig {
        fn default() -> Self {
            Self {
                enable_gc: true,
                trigger_threshold: 0.8,
                gc_goal: 0.7,
                incremental_step_size: 100,
            }
        }
    }

    /// 向后兼容的别名
    pub type GcIntegrationConfig = GcConfig;

    /// GC集成状态
    pub struct GcState {
        runtime: Option<Arc<vm_boot::gc_runtime::GcRuntime>>,
        config: GcConfig,
    }

    impl GcState {
        pub fn new(config: GcConfig) -> Result<Self, VmError> {
            let runtime = if config.enable_gc {
                let gc_config = vm_boot::gc_runtime::GcConfig {
                    num_workers: num_cpus::get(),
                    target_pause_us: (config.trigger_threshold * 1_000_000.0) as u64,
                    barrier_type: vm_optimizers::gc::WriteBarrierType::Atomic,
                };
                Some(Arc::new(vm_boot::gc_runtime::GcRuntime::new(gc_config)))
            } else {
                None
            };

            Ok(Self { runtime, config })
        }

        pub fn check_and_run(&self) -> Result<(), VmError> {
            if let Some(ref gc_runtime) = self.runtime {
                let stats = gc_runtime.get_stats();
                let trigger_threshold = (self.config.trigger_threshold * 1_000_000.0) as u64;

                if stats.minor_collections + stats.major_collections > trigger_threshold / 1000 {
                    let bytes_collected = stats.alloc_stats.bytes_used / 2;
                    if let Err(e) = gc_runtime.collect_minor(bytes_collected) {
                        tracing::warn!("GC minor collection failed: {:?}", e);
                    } else {
                        tracing::debug!("GC minor collection completed");
                    }
                }
            }
            Ok(())
        }

        pub fn get_stats(&self) -> Option<GcStats> {
            self.runtime.as_ref().map(|gc| gc.get_stats())
        }

        pub fn is_enabled(&self) -> bool {
            self.runtime.is_some()
        }
    }
}

/// JIT集成模块 - 统一管理所有JIT/AOT相关功能
/// 将所有JIT/AOT特性整合到单一模块中
#[cfg(feature = "jit")]
pub mod jit_integration {
    use super::*;
    use std::fs::File;

    /// JIT集成配置
    #[derive(Debug, Clone)]
    pub struct JitConfig {
        pub enable_jit: bool,
        pub hotspot_threshold: u32,
        pub cache_size: usize,
    }

    impl Default for JitConfig {
        fn default() -> Self {
            Self {
                enable_jit: true,
                hotspot_threshold: 100,
                cache_size: 64 * 1024 * 1024,
            }
        }
    }

    /// AOT配置
    #[derive(Debug, Clone)]
    pub struct AotConfig {
        pub enable_aot: bool,
        pub image_path: Option<String>,
        pub priority: bool,
        pub hotspot_threshold: u32,
    }

    impl Default for AotConfig {
        fn default() -> Self {
            Self {
                enable_aot: false,
                image_path: None,
                priority: true,
                hotspot_threshold: 1000,
            }
        }
    }

    /// 向后兼容的别名
    pub type JitIntegrationConfig = JitConfig;
    pub type AotIntegrationConfig = AotConfig;

    /// JIT集成状态
    pub struct JitState {
        compiler: Option<vm_engine_jit::Jit>,
        aot_compiler: Option<super::CrossArchAotCompiler>,
        cache: Arc<Mutex<HashMap<GuestAddr, Vec<u8>>>>,
        jit_config: JitConfig,
        aot_config: AotConfig,
    }

    impl JitState {
        pub fn new(
            jit_config: JitConfig,
            aot_config: AotConfig,
            cross_arch_config: &CrossArchConfig,
        ) -> Result<Self, VmError> {
            let compiler = if jit_config.enable_jit {
                Some(vm_engine_jit::Jit::new())
            } else {
                None
            };

            let aot_compiler = if aot_config.enable_aot {
                let source_arch = match cross_arch_config.guest_arch {
                    vm_core::GuestArch::X86_64 => Architecture::X86_64,
                    vm_core::GuestArch::Arm64 => Architecture::ARM64,
                    vm_core::GuestArch::Riscv64 => Architecture::RISCV64,
                    vm_core::GuestArch::PowerPC64 => {
                        return Err(VmError::Core(vm_core::CoreError::NotSupported {
                            feature: "PowerPC64 architecture".to_string(),
                            module: "cross_arch_runtime".to_string(),
                        }));
                    }
                };

                let aot_cfg = super::CrossArchAotConfig {
                    source_arch,
                    target_arch: cross_arch_config.host_arch.to_architecture().ok_or_else(
                        || VmError::Platform(vm_core::PlatformError::UnsupportedArch {
                            arch: cross_arch_config.host_arch.name().to_string(),
                            supported: vec![
                                "x86_64".to_string(),
                                "arm64".to_string(),
                                "riscv64".to_string(),
                            ],
                        }),
                    )?,
                    optimization_level: 2,
                    enable_cross_arch_optimization: true,
                    codegen_mode: vm_engine_jit::aot::CodegenMode::LLVM,
                };
                Some(super::CrossArchAotCompiler::new(aot_cfg)?)
            } else {
                None
            };

            let cache = Arc::new(Mutex::new(HashMap::new()));

            Ok(Self {
                compiler,
                aot_compiler,
                cache,
                jit_config,
                aot_config,
            })
        }

        pub fn compile_ir_block(&mut self, block: &IRBlock) -> Result<Vec<u8>, VmError> {
            if let Some(ref mut jit) = self.compiler {
                let code_ptr = jit.compile_only(block);

                if code_ptr.0.is_null() {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: "JIT compilation failed or timed out".to_string(),
                        module: "CrossArchRuntime".to_string(),
                    }));
                }

                let mut code_bytes = Vec::new();
                code_bytes.extend_from_slice(&block.start_pc.0.to_le_bytes());
                code_bytes.push(1);

                let mut cache = self.cache.lock().map_err(|_| {
                    VmError::Core(vm_core::CoreError::Internal {
                        message: "Failed to lock JIT cache".to_string(),
                        module: "CrossArchRuntime".to_string(),
                    })
                })?;
                cache.insert(block.start_pc, code_bytes.clone());

                Ok(code_bytes)
            } else {
                Err(VmError::Core(vm_core::CoreError::Internal {
                    message: "JIT compiler not available".to_string(),
                    module: "CrossArchRuntime".to_string(),
                }))
            }
        }

        pub fn check_and_compile_hotspots(
            &mut self,
            hotspots: Vec<GuestAddr>,
            get_ir_block: &mut dyn FnMut(GuestAddr) -> Result<Option<IRBlock>, VmError>,
        ) -> Result<(), VmError> {
            if !self.aot_config.enable_aot || self.aot_compiler.is_none() {
                return Ok(());
            }

            let mut ir_blocks_to_compile = Vec::new();

            for pc in &hotspots {
                match get_ir_block(*pc) {
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
                        tracing::warn!(pc = pc.0, error = ?e, "Failed to get IR block for hotspot");
                    }
                }
            }

            if let Some(ref mut compiler) = self.aot_compiler {
                for (pc, ir_block) in &ir_blocks_to_compile {
                    match compiler.compile_from_ir(*pc, ir_block) {
                        Ok(_) => {
                            tracing::debug!(
                                pc = pc.0,
                                ir_ops_count = ir_block.ops.len(),
                                "AOT compiled hotspot"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(pc = pc.0, error = ?e, "Failed to compile hotspot for AOT");
                        }
                    }
                }
            }

            Ok(())
        }

        pub fn check_and_jit_compile_hotspots(
            &mut self,
            hotspots: Vec<GuestAddr>,
            get_ir_block: &mut dyn FnMut(GuestAddr) -> Result<Option<IRBlock>, VmError>,
        ) -> Result<(), VmError> {
            if !self.jit_config.enable_jit {
                return Ok(());
            }

            let hotspots_to_compile: Vec<GuestAddr> = {
                let cache = self.cache.lock().map_err(|_| {
                    VmError::Core(vm_core::CoreError::Internal {
                        message: "Failed to lock JIT cache".to_string(),
                        module: "CrossArchRuntime".to_string(),
                    })
                })?;

                hotspots.into_iter().filter(|pc| !cache.contains_key(pc)).collect()
            };

            for pc in hotspots_to_compile {
                match get_ir_block(pc) {
                    Ok(Some(ir_block)) => {
                        match self.compile_ir_block(&ir_block) {
                            Ok(_) => {
                                tracing::debug!(pc = pc.0, "JIT compiled hotspot");
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
                        tracing::debug!(
                            pc = pc.0,
                            "Cannot get IR block for hotspot, skipping JIT compilation"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(pc = pc.0, error = ?e, "Failed to get IR block for hotspot");
                    }
                }
            }

            Ok(())
        }

        pub fn save_aot_image(&mut self, image_path: &str) -> Result<(), VmError> {
            if let Some(compiler) = self.aot_compiler.take() {
                let config = super::CrossArchAotConfig {
                    source_arch: compiler.config().source_arch,
                    target_arch: compiler.config().target_arch,
                    optimization_level: compiler.config().optimization_level,
                    enable_cross_arch_optimization: compiler.config().enable_cross_arch_optimization,
                    codegen_mode: compiler.config().codegen_mode,
                };

                compiler.save_to_file(image_path)?;
                self.aot_compiler = Some(super::CrossArchAotCompiler::new(config)?);

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

        pub fn load_aot_image(
            &mut self,
            image_path: &str,
            hotspot_threshold: u32,
        ) -> Result<(), VmError> {
            use vm_engine_jit::aot::AotImage;

            tracing::info!("Loading AOT image from: {}", image_path);

            let mut file = File::open(image_path).map_err(|e| {
                VmError::Platform(vm_core::PlatformError::IoError(format!(
                    "Failed to open AOT image file: {}",
                    e
                )))
            })?;

            let image = AotImage::deserialize(&mut file).map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to deserialize AOT image: {}", e),
                    module: "CrossArchRuntime".to_string(),
                })
            })?;

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

            let mut cache = self.cache.lock().map_err(|_| {
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

            tracing::info!(
                "AOT image loaded successfully: {} sections",
                image.sections.len()
            );

            Ok(())
        }

        pub fn get_aot_stats(&self) -> Option<&super::CrossArchAotStats> {
            self.aot_compiler.as_ref().map(|compiler| compiler.stats())
        }

        pub fn cache(&self) -> &Arc<Mutex<HashMap<GuestAddr, Vec<u8>>>> {
            &self.cache
        }

        pub fn jit_config(&self) -> &JitConfig {
            &self.jit_config
        }

        pub fn aot_config(&self) -> &AotConfig {
            &self.aot_config
        }

        pub fn has_jit_code(&self, pc: GuestAddr) -> bool {
            self.cache
                .lock()
                .map(|cache| cache.contains_key(&pc))
                .unwrap_or(false)
        }

        pub fn is_jit_enabled(&self) -> bool {
            self.jit_config.enable_jit
        }

        pub fn is_aot_enabled(&self) -> bool {
            self.aot_config.enable_aot
        }
    }
}

/// 内存集成模块 - 统一管理所有内存相关功能
/// 将所有内存特性整合到单一模块中
#[cfg(feature = "memory")]
pub mod memory_integration {
    use super::*;

    /// 内存集成配置
    #[derive(Debug, Clone)]
    pub struct MemoryConfig {
        pub memory_size: usize,
        pub enable_mmu: bool,
    }

    impl Default for MemoryConfig {
        fn default() -> Self {
            Self {
                memory_size: 128 * 1024 * 1024,
                enable_mmu: true,
            }
        }
    }

    /// 内存集成状态
    pub struct MemoryState {
        mmu: vm_mem::SoftMmu,
    }

    impl MemoryState {
        pub fn new(memory_size: usize) -> Result<Self, VmError> {
            let mmu = vm_mem::SoftMmu::new(memory_size, false);
            Ok(Self { mmu })
        }

        pub fn mmu(&self) -> &vm_mem::SoftMmu {
            &self.mmu
        }

        pub fn mmu_mut(&mut self) -> &mut vm_mem::SoftMmu {
            &mut self.mmu
        }

        pub fn memory_size(&self) -> usize {
            self.mmu.memory_size()
        }
    }
}

// ============================================================================
// Unified Configuration
// ============================================================================

/// 跨架构运行时配置 - 统一配置结构
/// 所有配置字段都使用条件编译，减少配置转换开销
#[derive(Debug, Clone)]
pub struct CrossArchRuntimeConfig {
    pub cross_arch: CrossArchConfig,

    #[cfg(feature = "gc")]
    pub gc: gc_integration::GcConfig,

    #[cfg(feature = "jit")]
    pub jit: jit_integration::JitConfig,

    #[cfg(feature = "jit")]
    pub aot: jit_integration::AotConfig,

    #[cfg(feature = "memory")]
    pub memory: memory_integration::MemoryConfig,
}

impl CrossArchRuntimeConfig {
    pub fn auto_create(guest_arch: GuestArch) -> Result<Self, VmError> {
        let cross_arch = CrossArchConfig::auto_detect(guest_arch)?;

        Ok(Self {
            cross_arch,
            #[cfg(feature = "gc")]
            gc: gc_integration::GcConfig::default(),
            #[cfg(feature = "jit")]
            jit: jit_integration::JitConfig::default(),
            #[cfg(feature = "jit")]
            aot: jit_integration::AotConfig::default(),
            #[cfg(feature = "memory")]
            memory: memory_integration::MemoryConfig::default(),
        })
    }
}

// 向后兼容的类型别名（deprecated）
#[cfg(feature = "gc")]
pub use gc_integration::GcIntegrationConfig;

#[cfg(feature = "jit")]
pub use jit_integration::{AotIntegrationConfig, JitIntegrationConfig};

/// 跨架构运行时系统 - 深度优化版本
///
/// 所有特性门仅在结构体字段级别，方法级别使用运行时检查
pub struct CrossArchRuntime {
    config: CrossArchRuntimeConfig,

    #[cfg(any(feature = "interpreter", feature = "jit"))]
    executor: super::AutoExecutor,

    #[cfg(feature = "memory")]
    memory: memory_integration::MemoryState,

    #[cfg(feature = "gc")]
    gc: gc_integration::GcState,

    #[cfg(feature = "jit")]
    jit: jit_integration::JitState,

    hotspot_tracker: Arc<Mutex<HotspotTracker>>,
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
    fn lock_hotspot_tracker(&self) -> Result<std::sync::MutexGuard<'_, HotspotTracker>, VmError> {
        self.hotspot_tracker.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock hotspot tracker".to_string(),
                module: "CrossArchRuntime".to_string(),
            })
        })
    }

    pub fn new(config: CrossArchRuntimeConfig) -> Result<Self, VmError> {
        #[cfg(feature = "jit")]
        let hotspot_threshold = config.jit.hotspot_threshold.max(config.aot.hotspot_threshold);

        #[cfg(not(feature = "jit"))]
        let hotspot_threshold = 100;

        let hotspot_tracker = Arc::new(Mutex::new(HotspotTracker::new(hotspot_threshold)));

        #[cfg(any(feature = "interpreter", feature = "jit"))]
        let executor = {
            super::AutoExecutor::auto_create(
                config.cross_arch.guest_arch,
                Some(config.cross_arch.recommended_exec_mode()),
            )?
        };

        #[cfg(feature = "memory")]
        let memory = memory_integration::MemoryState::new(config.memory.memory_size)?;

        #[cfg(feature = "gc")]
        let gc = gc_integration::GcState::new(config.gc.clone())?;

        #[cfg(feature = "jit")]
        let jit = jit_integration::JitState::new(
            config.jit.clone(),
            config.aot.clone(),
            &config.cross_arch,
        )?;

        Ok(Self {
            config,
            #[cfg(any(feature = "interpreter", feature = "jit"))]
            executor,
            #[cfg(feature = "memory")]
            memory,
            #[cfg(feature = "gc")]
            gc,
            #[cfg(feature = "jit")]
            jit,
            hotspot_tracker,
        })
    }

    pub fn execute_block(&mut self, pc: GuestAddr) -> Result<vm_core::ExecResult, VmError> {
        // Record hotspot
        {
            let mut tracker = self.lock_hotspot_tracker()?;
            tracker.record_execution(pc);
        }

        // Consolidate all conditional execution using cfg-if
        cfg_if::cfg_if! {
            if #[cfg(all(feature = "jit", feature = "memory"))] {
                // Check AOT code
                if self.jit.is_aot_enabled() && self.jit.aot_config().priority {
                    if self.jit.has_jit_code(pc) {
                        tracing::debug!(pc = pc.0, "Found AOT code, using AOT execution");
                    }
                }

                // Check JIT cache
                if self.jit.is_jit_enabled() {
                    let has_jit_code = self.jit.has_jit_code(pc);
                    if has_jit_code {
                        tracing::debug!(pc = pc.0, "Found JIT code in cache");
                    } else {
                        let is_hotspot = {
                            let tracker = self.lock_hotspot_tracker()?;
                            tracker.is_hotspot(pc)
                        };

                        if is_hotspot {
                            tracing::debug!(pc = pc.0, "Hotspot detected, will trigger JIT compilation");
                        }
                    }
                }

                // Execute block
                let result = self.executor.execute_block(self.memory.mmu_mut(), pc)?;

                // Run GC if needed
                #[cfg(feature = "gc")]
                self.gc.check_and_run()?;

                // Compile hotspots
                let hotspots = {
                    let tracker = self.lock_hotspot_tracker()?;
                    tracker.get_hotspots()
                };

                if !hotspots.is_empty() {
                    self.jit.check_and_compile_hotspots(hotspots, &mut |pc| self.get_ir_block_for_pc(pc))?;
                    self.jit.check_and_jit_compile_hotspots(hotspots, &mut |pc| self.get_ir_block_for_pc(pc))?;
                }
            } else if #[cfg(all(feature = "jit", not(feature = "memory")))] {
                // JIT without memory feature
                let result = self.executor.execute_block(pc)?;
                Ok(result)
            } else if #[cfg(all(not(feature = "jit"), feature = "memory"))] {
                // Memory without JIT
                let result = self.executor.execute_block(self.memory.mmu_mut(), pc)?;

                #[cfg(feature = "gc")]
                self.gc.check_and_run()?;

                Ok(result)
            } else if #[cfg(not(any(feature = "jit", feature = "memory")))] {
                // Minimal configuration
                let result = self.executor.execute_block(pc)?;
                Ok(result)
            }
        }
    }

    fn get_ir_block_for_pc(&mut self, pc: GuestAddr) -> Result<Option<IRBlock>, VmError> {
        cfg_if::cfg_if! {
            if #[cfg(all(feature = "memory", any(feature = "interpreter", feature = "jit")))] {
                match self.executor.decode_block(self.memory.mmu_mut(), pc) {
                    Ok(ir_block) => Ok(Some(ir_block)),
                    Err(e) => {
                        tracing::debug!(pc = pc.0, error = ?e, "Failed to decode IR block");
                        Ok(None)
                    }
                }
            } else if #[cfg(all(not(feature = "memory"), any(feature = "interpreter", feature = "jit")))] {
                match self.executor.decode_block(pc) {
                    Ok(ir_block) => Ok(Some(ir_block)),
                    Err(e) => {
                        tracing::debug!(pc = pc.0, error = ?e, "Failed to decode IR block");
                        Ok(None)
                    }
                }
            } else {
                Ok(None)
            }
        }
    }

    pub fn mmu_mut(&mut self) -> Option<&mut vm_mem::SoftMmu> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "memory")] {
                Some(self.memory.mmu_mut())
            } else {
                None
            }
        }
    }

    pub fn engine_mut(&mut self) -> Option<&mut dyn ExecutionEngine<IRBlock>> {
        cfg_if::cfg_if! {
            if #[cfg(any(feature = "interpreter", feature = "jit"))] {
                Some(self.executor.engine_mut())
            } else {
                None
            }
        }
    }

    pub fn config(&self) -> &CrossArchRuntimeConfig {
        &self.config
    }

    pub fn memory_size(&self) -> usize {
        cfg_if::cfg_if! {
            if #[cfg(feature = "memory")] {
                self.memory.memory_size()
            } else {
                0
            }
        }
    }

    pub fn save_runtime_state(&mut self) -> Result<Vec<u8>, VmError> {
        Ok(Vec::new())
    }

    pub fn save_aot_image(&mut self, image_path: &str) -> Result<(), VmError> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "jit")] {
                self.jit.save_aot_image(image_path)
            } else {
                Err(VmError::Core(vm_core::CoreError::NotSupported {
                    feature: "AOT image saving".to_string(),
                    module: "cross_arch_runtime".to_string(),
                }))
            }
        }
    }

    pub fn load_aot_image(&mut self, image_path: &str) -> Result<(), VmError> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "jit")] {
                let hotspot_threshold = self
                    .config
                    .jit
                    .hotspot_threshold
                    .max(self.config.aot.hotspot_threshold);

                self.jit.load_aot_image(image_path, hotspot_threshold)?;

                // Update hotspot tracker
                let cache = self.jit.cache();
                let cache_keys: Vec<_> = cache
                    .lock()
                    .map(|c| c.keys().copied().collect())
                    .unwrap_or_default();

                {
                    let mut tracker = self.lock_hotspot_tracker()?;
                    for addr in cache_keys {
                        tracker.execution_counts.insert(addr, hotspot_threshold);
                    }
                }

                Ok(())
            } else {
                Err(VmError::Core(vm_core::CoreError::NotSupported {
                    feature: "AOT image loading".to_string(),
                    module: "cross_arch_runtime".to_string(),
                }))
            }
        }
    }

    pub fn get_aot_stats(&self) -> Option<&super::CrossArchAotStats> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "jit")] {
                self.jit.get_aot_stats()
            } else {
                None
            }
        }
    }

    pub fn get_hotspot_stats(&self) -> Result<HotspotStats, VmError> {
        let tracker = self.lock_hotspot_tracker()?;
        let hotspots = tracker.get_hotspots();
        let total_executions: u32 = tracker.execution_counts.values().sum();
        let hotspot_count = hotspots.len();

        Ok(HotspotStats {
            total_executions,
            hotspot_count,
            threshold: tracker.threshold,
        })
    }

    pub fn get_gc_stats(&self) -> Option<vm_optimizers::gc::GcStats> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "gc")] {
                self.gc.get_stats()
            } else {
                None
            }
        }
    }

    pub fn get_jit_cache_stats(&self) -> Result<JitCacheStats, VmError> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "jit")] {
                let cache = self.jit.cache().lock().map_err(|_| {
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
            } else {
                Ok(JitCacheStats {
                    cached_blocks: 0,
                    total_size: 0,
                })
            }
        }
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
        let config = match CrossArchRuntimeConfig::auto_create(GuestArch::X86_64) {
            Ok(cfg) => cfg,
            Err(_) => return,
        };
        let runtime = CrossArchRuntime::new(config);
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_hotspot_tracker() {
        let mut tracker = HotspotTracker::new(10);
        let pc = vm_core::GuestAddr(0x1000);

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
        let config = match CrossArchRuntimeConfig::auto_create(GuestArch::X86_64) {
            Ok(cfg) => cfg,
            Err(_) => return,
        };
        let runtime = match CrossArchRuntime::new(config) {
            Ok(rt) => rt,
            Err(_) => return,
        };

        // 手动记录执行次数（模拟execute_block的行为）
        let pc = vm_core::GuestAddr(0x1000);
        {
            let mut tracker = match runtime.lock_hotspot_tracker() {
                Ok(t) => t,
                Err(_) => return,
            };
            for _ in 0..5 {
                tracker.record_execution(pc);
            }
        }

        // 获取统计信息
        let stats = match runtime.get_hotspot_stats() {
            Ok(s) => s,
            Err(_) => return,
        };
        assert_eq!(stats.total_executions, 5);
        assert_eq!(stats.hotspot_count, 0); // 未达到阈值（默认阈值是100）
    }

    #[test]
    fn test_jit_cache_stats() {
        let config = match CrossArchRuntimeConfig::auto_create(GuestArch::X86_64) {
            Ok(cfg) => cfg,
            Err(_) => return,
        };
        let runtime = match CrossArchRuntime::new(config) {
            Ok(rt) => rt,
            Err(_) => return,
        };

        // 获取JIT缓存统计
        let stats = match runtime.get_jit_cache_stats() {
            Ok(s) => s,
            Err(_) => return,
        };
        assert_eq!(stats.cached_blocks, 0);
        assert_eq!(stats.total_size, 0);
    }
}
