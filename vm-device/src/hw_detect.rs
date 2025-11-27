use wgpu;

pub struct HardwareSummary {
    pub gpus: Vec<wgpu::AdapterInfo>,
}

pub struct HardwareDetector;

impl HardwareDetector {
    pub async fn detect() -> HardwareSummary {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let adapters = instance.enumerate_adapters(wgpu::Backends::all());
        let gpus = adapters.into_iter().map(|a| a.get_info()).collect();

        HardwareSummary { gpus }
    }

    pub fn print_summary(summary: &HardwareSummary) {
        println!("--- Hardware Detection Summary ---");
        if summary.gpus.is_empty() {
            println!("No GPUs found.");
        } else {
            for (i, gpu) in summary.gpus.iter().enumerate() {
                println!("GPU #{}:", i);
                println!("  Name: {}", gpu.name);
                println!("  Backend: {:?}", gpu.backend);
                println!("  Device Type: {:?}", gpu.device_type);
                println!("  Driver: {}", gpu.driver);
                println!("  Driver Info: {}", gpu.driver_info);
            }
        }
        println!("--------------------------------");
    }
}
