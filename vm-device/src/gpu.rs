use vm_core::MmioDevice;
use wgpu::{Instance, Surface, Adapter, Device, Queue};
use winit::window::Window;
use std::sync::Arc;
use crate::virgl::VirtioGpuVirgl;

pub struct GpuDevice {
    instance: Instance,
    surface: Option<Surface<'static>>,
    adapter: Option<Adapter>,
    device: Option<Device>,
    queue: Option<Queue>,
    window: Option<Arc<Window>>,
    virgl: VirtioGpuVirgl,
}

impl GpuDevice {
    pub fn new() -> Self {
        let instance = Instance::default();
        Self {
            instance,
            surface: None,
            adapter: None,
            device: None,
            queue: None,
            window: None,
            virgl: VirtioGpuVirgl::new(),
        }
    }

    pub async fn init(&mut self, window: Arc<Window>) {
        let surface = self.instance.create_surface(window.clone()).unwrap();
        let adapter = self.instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
            },
            None,
        ).await.unwrap();

        self.surface = Some(surface);
        self.adapter = Some(adapter);
        self.device = Some(device);
        self.queue = Some(queue);
        self.window = Some(window);
    }
}

impl MmioDevice for GpuDevice {
    fn read(&self, offset: u64, _size: u8) -> u64 {
        // Mock register read
        match offset {
            0x00 => 0x12345678, // Device ID
            _ => 0,
        }
    }

    fn write(&mut self, offset: u64, val: u64, _size: u8) {
        // Mock register write
        match offset {
            0x10 => {
                // Trigger render command
                let cmd_buf = val.to_le_bytes();
                self.virgl.process_gpu_command(&cmd_buf);
            }
            _ => {}
        }
    }
}
