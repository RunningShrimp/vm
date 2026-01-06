//! # GPU计算加速模块
//!
//! 本模块提供统一的GPU计算抽象层，支持多种GPU后端（CUDA、ROCm等）。
//!
//! ## 架构概览
//!
//! GPU模块采用分层架构设计，提供从硬件抽象到高级执行的完整功能：
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                   Application Layer                     │
//! │                  (您的应用程序)                         │
//! └────────────────────┬────────────────────────────────────┘
//!                       │
//!                       ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │                   GpuExecutor                           │
//! │  • 内核缓存管理                                          │
//! │  • CPU回退机制                                          │
//! │  • 性能监控统计                                          │
//! │  • 执行配置管理                                          │
//! └────────────────────┬────────────────────────────────────┘
//!                       │
//!                       ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │                  GpuDeviceManager                       │
//! │  • 设备检测与初始化                                      │
//! │  • 多设备管理                                            │
//! │  • 默认设备选择                                          │
//! └─────────┬───────────────────────────┬───────────────────┘
//!           │                           │
//!           ▼                           ▼
//! ┌─────────────────────┐   ┌─────────────────────┐
//! │   CUDA Backend      │   │   ROCm Backend      │
//! │   (NVIDIA GPU)      │   │   (AMD GPU)         │
//! │                     │   │                     │
//! │ • 设备管理          │   │ • 设备管理          │
//! │ • 内存分配          │   │ • 内存分配          │
//! │ • 内核编译(NVRTC)   │   │ • 内核编译(HIPC)    │
//! │ • 内核执行          │   │ • 内核执行          │
//! └─────────────────────┘   └─────────────────────┘
//!           │                           │
//!           └───────────┬───────────────┘
//!                       │
//!                       ▼
//!              ┌─────────────────┐
//!              │   GpuCompute    │
//!              │   (统一trait)   │
//!              └─────────────────┘
//! ```
//!
//! ## 核心组件
//!
//! ### 1. GpuCompute Trait
//!
//! 定义所有GPU设备必须实现的统一接口：
//!
//! - `initialize()`: 设备初始化
//! - `device_info()`: 获取设备信息
//! - `allocate_memory()`: 分配设备内存
//! - `copy_h2d()` / `copy_d2h()`: 主机与设备间数据传输
//! - `compile_kernel()`: 编译GPU内核
//! - `execute_kernel()`: 执行GPU内核
//! - `synchronize()`: 同步设备操作
//!
//! ### 2. GpuDeviceManager
//!
//! 负责检测和管理所有可用的GPU设备：
//!
//! - 自动检测CUDA/ROCm设备
//! - 管理多个GPU设备
//! - 提供默认设备选择
//!
//! ### 3. GpuExecutor
//!
//! 高级GPU执行接口，提供：
//!
//! - **内核缓存**: 避免重复编译
//! - **CPU回退**: GPU失败时自动回退到CPU
//! - **性能监控**: 详细的执行统计
//! - **配置管理**: 灵活的执行配置
//!
//! ## 使用示例
//!
//! ### 基础设备检测
//!
//! ```rust,no_run
//! use vm_core::gpu::{GpuDeviceManager, GpuCompute};
//!
//! let manager = GpuDeviceManager::new();
//!
//! if manager.has_gpu() {
//!     let device = manager.default_device().unwrap();
//!     let info = device.device_info();
//!     println!("GPU: {} ({} MB)", info.name, info.total_memory_mb);
//! } else {
//!     println!("No GPU available");
//! }
//! ```
//!
//! ### 使用执行器（推荐）
//!
//! ```rust,no_run
//! use vm_core::gpu::{GpuExecutor, GpuExecutionConfig, GpuArg};
//!
//! // 创建执行器
//! let executor = GpuExecutor::default();
//!
//! // 配置执行参数
//! let config = GpuExecutionConfig {
//!     kernel_source: r#"
//!         __global__ void vector_add(float* a, float* b, float* c, int n) {
//!             int idx = blockIdx.x * blockDim.x + threadIdx.x;
//!             if (idx < n) {
//!                 c[idx] = a[idx] + b[idx];
//!             }
//!         }
//!     "#.to_string(),
//!     kernel_name: "vector_add".to_string(),
//!     grid_dim: (256, 1, 1),
//!     block_dim: (256, 1, 1),
//!     args: vec![
//!         GpuArg::Buffer(a_buffer),
//!         GpuArg::Buffer(b_buffer),
//!         GpuArg::Buffer(c_buffer),
//!         GpuArg::U32(n),
//!     ],
//!     shared_memory_size: 0,
//! };
//!
//! // 执行（带CPU回退）
//! let result = executor.execute_with_fallback(&config, || {
//!     // CPU回退实现
//!     println!("Falling back to CPU execution");
//!     cpu_vector_add(&a, &b, &mut c, n);
//!     Ok(())
//! });
//!
//! if result.success {
//!     println!("Execution successful in {:?}", result.execution_time_ns);
//! }
//! ```
//!
//! ## 性能监控
//!
//! ```rust,no_run
//! # use vm_core::gpu::GpuExecutor;
//! let executor = GpuExecutor::default();
//!
//! // 执行一些操作...
//!
//! // 打印性能统计
//! executor.print_stats();
//!
//! // 或获取详细统计
//! let stats = executor.stats();
//! println!("Cache hit rate: {:.2}%", stats.cache_hit_rate() * 100.0);
//! println!("GPU success rate: {:.2}%", stats.gpu_success_rate() * 100.0);
//! ```
//!
//! ## Feature Flags
//!
//! GPU支持需要启用相应的feature：
//!
//! ```toml
//! [dependencies.vm-core]
//! version = "0.1.0"
//! features = ["gpu"]  # 启用所有GPU支持
//!
//! # 或单独启用
//! features = ["cuda"]  # 仅CUDA
//! features = ["rocm"]  # 仅ROCm
//! ```
//!
//! **注意**:
//! - `cuda` feature需要NVIDIA GPU和CUDA Toolkit
//! - `rocm` feature需要AMD GPU和ROCm环境
//! - 实际的GPU实现在`vm-passthrough` crate中
//!
//! ## 错误处理
//!
//! 所有GPU操作返回`GpuResult<T>`：
//!
//! ```rust,ignore
//! use vm_core::gpu::{GpuError, GpuResult};
//!
//! pub enum GpuError {
//!     NoDeviceAvailable,
//!     DeviceInitializationFailed { device_type: String, reason: String },
//!     MemoryAllocationFailed { requested_size: usize, reason: String },
//!     KernelCompilationFailed { kernel_name: String, source: String, reason: String },
//!     KernelExecutionFailed { kernel_name: String, reason: String },
//!     // ... 更多错误类型
//! }
//! ```
//!
//! ## 开发状态
//!
//! | 模块 | 状态 | 完成度 |
//! |------|------|--------|
//! | 接口设计 | ✅ 完成 | 100% |
//! | CUDA设备管理 | ✅ 完成 | 100% |
//! | ROCm设备管理 | ⏳ 进行中 | 30% |
//! | 内核编译 | 🚧 未开始 | 0% |
//! | 内核执行 | 🚧 未开始 | 0% |
//! | 执行器优化 | ✅ 完成 | 100% |
//!
//! ## 下一步计划
//!
//! ### Phase 2: 内核编译与执行
//! - [ ] 实现NVRTC编译器集成
//! - [ ] 实现HIP编译器集成
//! - [ ] 添加内核缓存机制
//! - [ ] 实现内核执行器
//!
//! ### Phase 3: 性能优化
//! - [ ] 实现异步执行
//! - [ ] 添加流管理
//! - [ ] 优化内存传输
//! - [ ] 实现多GPU支持
//!
//! ## 参考资源
//!
//! - [CUDA Programming Guide](https://docs.nvidia.com/cuda/cuda-c-programming-guide/)
//! - [ROCm HIP Programming Guide](https://rocm.docs.amd.com/en/latest/ HIP_HTML_TOPIC.html)
//! - [NVRTC Reference](https://docs.nvidia.com/cuda/nvrtc/index.html)

pub mod device;
pub mod error;
pub mod executor;

// 重新导出主要类型
pub use device::{GpuDeviceManager, GpuDeviceInfo, GpuExecutionResult};
pub use error::{GpuError, GpuResult};
pub use executor::{GpuExecutionConfig, GpuExecutor};
