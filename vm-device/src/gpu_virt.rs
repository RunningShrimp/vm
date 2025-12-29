use std::sync::{Arc, Mutex};
use thiserror::Error;
use wgpu;

/// GPU 后端统计信息
#[derive(Debug, Clone, Default)]
pub struct GpuStats {
    pub command_buffer_count: u64,
    pub render_pass_count: u64,
    pub compute_pass_count: u64,
    pub texture_count: u64,
    pub buffer_count: u64,
    pub total_memory_allocated: u64,
}

pub trait GpuBackend: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn init(&mut self) -> Result<(), GpuVirtError>;
    fn get_stats(&self) -> GpuStats;
    fn reset_stats(&mut self);
}

pub struct WgpuBackend {
    instance: wgpu::Instance,
    adapter: Option<wgpu::Adapter>,
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
    stats: Arc<Mutex<GpuStats>>,
}

impl Default for WgpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl WgpuBackend {
    pub fn new() -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        Self {
            instance,
            adapter: None,
            device: None,
            queue: None,
            stats: Arc::new(Mutex::new(GpuStats::default())),
        }
    }

    /// 获取设备引用
    pub fn device(&self) -> Option<Arc<wgpu::Device>> {
        self.device.clone()
    }

    /// 获取队列引用
    pub fn queue(&self) -> Option<Arc<wgpu::Queue>> {
        self.queue.clone()
    }

    /// 获取适配器信息
    pub fn adapter_info(&self) -> Option<wgpu::AdapterInfo> {
        self.adapter.as_ref().map(|a| a.get_info())
    }
}

impl GpuBackend for WgpuBackend {
    fn name(&self) -> &str {
        "WGPU"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn init(&mut self) -> Result<(), GpuVirtError> {
        // 请求适配器，优先选择高性能 GPU
        let adapter =
            pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
            .ok_or_else(|| GpuVirtError::AdapterRequest("No adapter found".to_string()))?;

        self.adapter = Some(adapter.clone());
        let info = adapter.get_info();
        println!("Selected GPU: {} ({:?})", info.name, info.backend);

        // 请求设备和队列，启用所有可用的 WebGPU 功能
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::all_webgpu_mask(),
                required_limits: wgpu::Limits {
                    max_texture_dimension_1d: 8192,
                    max_texture_dimension_2d: 8192,
                    max_texture_dimension_3d: 2048,
                    max_bind_groups: 8,
                    max_dynamic_uniform_buffers_per_pipeline_layout: 8,
                    max_dynamic_storage_buffers_per_pipeline_layout: 4,
                    max_sampled_textures_per_shader_stage: 16,
                    max_samplers_per_shader_stage: 16,
                    max_storage_buffers_per_shader_stage: 8,
                    max_storage_textures_per_shader_stage: 4,
                    max_uniform_buffers_per_shader_stage: 12,
                    max_uniform_buffer_binding_size: 65536,
                    max_storage_buffer_binding_size: 134217728,
                    max_buffer_size: 268435456,
                    max_push_constant_size: 0,
                    ..Default::default()
                },
                label: Some("VM GPU Device"),
                ..Default::default()
            },
            None, // trace_path
        ))
        .map_err(|e: wgpu::RequestDeviceError| GpuVirtError::DeviceRequest(e.to_string()))?;

        self.device = Some(Arc::new(device));
        self.queue = Some(Arc::new(queue));

        Ok(())
    }

    fn get_stats(&self) -> GpuStats {
        self.stats.lock().expect("Failed to lock receiver").clone()
    }

    fn reset_stats(&mut self) {
        *self.stats.lock().expect("Failed to lock receiver") = GpuStats::default();
    }
}

/// GPU 直通后端（用于设备直通场景）
pub struct PassthroughBackend {
    name: String,
    available: bool,
}

impl Default for PassthroughBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PassthroughBackend {
    pub fn new() -> Self {
        Self {
            name: "Passthrough".to_string(),
            available: false,
        }
    }

    /// 检查是否有可用的直通设备
    pub fn detect_devices(&mut self) -> Vec<String> {
        // 这里可以扫描 /sys/bus/pci/devices/ 来检测可用的 GPU
        vec![]
    }
}

impl GpuBackend for PassthroughBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        self.available
    }

    fn init(&mut self) -> Result<(), GpuVirtError> {
        // 直通模式下，不需要初始化虚拟 GPU
        Ok(())
    }

    fn get_stats(&self) -> GpuStats {
        GpuStats::default()
    }

    fn reset_stats(&mut self) {
        // 直通模式下没有统计信息
    }
}

pub struct GpuManager {
    backends: Vec<Box<dyn GpuBackend>>,
    selected_backend: Option<usize>,
}

impl Default for GpuManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuManager {
    pub fn new() -> Self {
        let backends: Vec<Box<dyn GpuBackend>> = vec![
            Box::new(WgpuBackend::new()),
            Box::new(PassthroughBackend::new()),
        ];

        Self {
            backends,
            selected_backend: None,
        }
    }

    pub fn auto_select_backend(&mut self) {
        for (i, backend) in self.backends.iter().enumerate() {
            if backend.is_available() {
                self.selected_backend = Some(i);
                println!("Auto-selected GPU backend: {}", backend.name());
                return;
            }
        }
    }

    pub fn select_backend_by_name(&mut self, name: &str) -> Result<(), GpuVirtError> {
        for (i, backend) in self.backends.iter().enumerate() {
            if backend.name() == name {
                if backend.is_available() {
                    self.selected_backend = Some(i);
                    return Ok(());
                } else {
                    return Err(GpuVirtError::NotAvailable(name.into()));
                }
            }
        }
        Err(GpuVirtError::NotFound(name.into()))
    }

    pub fn init_selected_backend(&mut self) -> Result<(), GpuVirtError> {
        if let Some(index) = self.selected_backend {
            self.backends[index].init()
        } else {
            Err(GpuVirtError::NoBackendSelected)
        }
    }

    pub fn get_stats(&self) -> Option<GpuStats> {
        self.selected_backend.map(|i| self.backends[i].get_stats())
    }

    pub fn reset_stats(&mut self) {
        if let Some(index) = self.selected_backend {
            self.backends[index].reset_stats();
        }
    }
}

#[derive(Debug, Error)]
pub enum GpuVirtError {
    #[error("Backend not available: {0}")]
    NotAvailable(String),
    #[error("Backend not found: {0}")]
    NotFound(String),
    #[error("No backend selected")]
    NoBackendSelected,
    #[error("Adapter request failed: {0}")]
    AdapterRequest(String),
    #[error("Device request failed: {0}")]
    DeviceRequest(String),
}
