//! GPU 加速模块
//!
//! 该模块负责检测并启用特定于 GPU 厂商和 SoC 的硬件加速功能。

use wgpu::{Adapter, Device, DeviceDescriptor, Features, Instance, Queue, RequestAdapterOptions, DeviceType, Backend};

/// GPU 加速器，封装了 wgpu 设备和队列，并记录了启用的高级功能。
pub struct GpuAccelerator {
    pub device: Device,
    pub queue: Queue,
    pub enabled_features: Features,
}

impl GpuAccelerator {
    /// 根据给定的适配器创建新的 GPU 加速器实例。
    ///
    /// 此函数会检查适配器的厂商、后端和支持的功能，并尝试启用针对性的优化，
    /// 例如针对 iGPU 的 `MAPPABLE_PRIMARY_BUFFERS`，以及针对 Apple Silicon 和 ARM Mali 的特定功能。
    pub async fn new(adapter: &Adapter) -> Option<Self> {
        let adapter_info = adapter.get_info();
        let supported_features = adapter.features();
        let mut required_features = Features::empty();

        println!("Initializing GPU Accelerator for: {} ({:?})", adapter_info.name, adapter_info.backend);

        // 针对不同 SoC 和 GPU 架构的优化
        match adapter_info.backend {
            Backend::Metal => {
                println!("Apple Silicon (Metal backend) detected. Applying specific optimizations...");
                // 在 Metal 上可以安全地启用 MAPPABLE_PRIMARY_BUFFERS
                if supported_features.contains(Features::MAPPABLE_PRIMARY_BUFFERS) {
                    println!("  -> Enabling MAPPABLE_PRIMARY_BUFFERS for UMA performance.");
                    required_features |= Features::MAPPABLE_PRIMARY_BUFFERS;
                }
            }
            Backend::Vulkan => {
                if adapter_info.name.to_lowercase().contains("mali") {
                    println!("ARM Mali (Vulkan backend) detected. Applying specific optimizations...");
                    // Mali GPU 通常受益于某些特定的 Vulkan 扩展，但 wgpu 会自动处理大部分。
                    // 这里可以添加未来发现的、需要手动启用的特定功能。
                }
            }
            _ => {}
        }

        // 通用的 iGPU 优化
        if adapter_info.device_type == DeviceType::IntegratedGpu {
            println!("Integrated GPU detected. Checking for UMA optimizations...");
            if supported_features.contains(Features::MAPPABLE_PRIMARY_BUFFERS) {
                println!("  -> Enabling MAPPABLE_PRIMARY_BUFFERS");
                required_features |= Features::MAPPABLE_PRIMARY_BUFFERS;
            }
        }

        let (device, queue) = match adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("GPU Accelerator Device"),
                    required_features,
                    required_limits: adapter.limits(),
                },
                None, // Trace path
            )
            .await
        {
            Ok(dq) => dq,
            Err(e) => {
                eprintln!("Failed to create GPU device with required features: {}", e);
                // 如果请求失败，尝试不带任何特殊功能再次请求
                match adapter.request_device(&DeviceDescriptor::default(), None).await {
                    Ok(dq) => dq,
                    Err(e2) => {
                        eprintln!("Failed to create GPU device with default features: {}", e2);
                        return None;
                    }
                }
            }
        };

        println!("GPU Accelerator initialized successfully.");

        Some(Self {
            device,
            queue,
            enabled_features: required_features,
        })
    }
}
