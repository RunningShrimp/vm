//! 图形设备模拟
//!
//! 支持 VGA、VBE (VESA BIOS Extensions) 和 virtio-gpu

// 图形设备不需要实现 MmioDevice trait，它们有自己的接口
use std::sync::{Arc, Mutex};

/// VGA 寄存器
const VGA_CRTC_INDEX: u16 = 0x3D4;
const VGA_CRTC_DATA: u16 = 0x3D5;
const VGA_SEQ_INDEX: u16 = 0x3C4;
const VGA_SEQ_DATA: u16 = 0x3C5;
const VGA_GFX_INDEX: u16 = 0x3CE;
const VGA_GFX_DATA: u16 = 0x3CF;
const VGA_ATTR_INDEX: u16 = 0x3C0;
const VGA_ATTR_DATA_READ: u16 = 0x3C1;
const VGA_DAC_WRITE_INDEX: u16 = 0x3C8;
const VGA_DAC_DATA: u16 = 0x3C9;

/// 帧缓冲格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    RGB565,
    RGB888,
    RGBA8888,
    BGRA8888,
}

impl PixelFormat {
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::RGB565 => 2,
            PixelFormat::RGB888 => 3,
            PixelFormat::RGBA8888 | PixelFormat::BGRA8888 => 4,
        }
    }
}

/// 帧缓冲信息
#[derive(Debug, Clone)]
pub struct FramebufferInfo {
    pub width: u32,
    pub height: u32,
    pub stride: u32,  // 每行字节数
    pub format: PixelFormat,
}

/// VGA 设备
pub struct VgaDevice {
    /// 帧缓冲
    framebuffer: Vec<u8>,
    /// 帧缓冲信息
    fb_info: FramebufferInfo,
    /// VGA 寄存器
    crtc_index: u8,
    crtc_regs: [u8; 256],
    seq_index: u8,
    seq_regs: [u8; 256],
    gfx_index: u8,
    gfx_regs: [u8; 256],
    attr_index: u8,
    attr_regs: [u8; 256],
    /// DAC 调色板
    dac_index: u8,
    dac_palette: [[u8; 3]; 256],
    /// 显示模式
    mode: VgaMode,
}

/// VGA 显示模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VgaMode {
    Text80x25,
    Graphics640x480,
    Graphics800x600,
    Graphics1024x768,
    Graphics1280x1024,
    Graphics1920x1080,
}

impl VgaMode {
    pub fn resolution(&self) -> (u32, u32) {
        match self {
            VgaMode::Text80x25 => (720, 400),  // 文本模式等效分辨率
            VgaMode::Graphics640x480 => (640, 480),
            VgaMode::Graphics800x600 => (800, 600),
            VgaMode::Graphics1024x768 => (1024, 768),
            VgaMode::Graphics1280x1024 => (1280, 1024),
            VgaMode::Graphics1920x1080 => (1920, 1080),
        }
    }
}

impl VgaDevice {
    /// 创建新的 VGA 设备
    pub fn new(mode: VgaMode) -> Self {
        let (width, height) = mode.resolution();
        let format = PixelFormat::BGRA8888;
        let stride = width * format.bytes_per_pixel() as u32;
        
        let fb_size = (stride * height) as usize;
        let framebuffer = vec![0u8; fb_size];
        
        Self {
            framebuffer,
            fb_info: FramebufferInfo {
                width,
                height,
                stride,
                format,
            },
            crtc_index: 0,
            crtc_regs: [0; 256],
            seq_index: 0,
            seq_regs: [0; 256],
            gfx_index: 0,
            gfx_regs: [0; 256],
            attr_index: 0,
            attr_regs: [0; 256],
            dac_index: 0,
            dac_palette: [[0; 3]; 256],
            mode,
        }
    }

    /// 设置显示模式
    pub fn set_mode(&mut self, mode: VgaMode) {
        let (width, height) = mode.resolution();
        let stride = width * self.fb_info.format.bytes_per_pixel() as u32;
        let fb_size = (stride * height) as usize;
        
        self.framebuffer.resize(fb_size, 0);
        self.fb_info.width = width;
        self.fb_info.height = height;
        self.fb_info.stride = stride;
        self.mode = mode;
        
        log::info!("VGA mode set to {:?} ({}x{})", mode, width, height);
    }

    /// 获取帧缓冲
    pub fn get_framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    /// 获取帧缓冲信息
    pub fn get_framebuffer_info(&self) -> &FramebufferInfo {
        &self.fb_info
    }

    /// 写入像素
    pub fn write_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x >= self.fb_info.width || y >= self.fb_info.height {
            return;
        }

        let offset = (y * self.fb_info.stride + x * self.fb_info.format.bytes_per_pixel() as u32) as usize;
        
        match self.fb_info.format {
            PixelFormat::BGRA8888 => {
                if offset + 4 <= self.framebuffer.len() {
                    self.framebuffer[offset] = (color & 0xFF) as u8;         // B
                    self.framebuffer[offset + 1] = ((color >> 8) & 0xFF) as u8;  // G
                    self.framebuffer[offset + 2] = ((color >> 16) & 0xFF) as u8; // R
                    self.framebuffer[offset + 3] = ((color >> 24) & 0xFF) as u8; // A
                }
            }
            PixelFormat::RGBA8888 => {
                if offset + 4 <= self.framebuffer.len() {
                    self.framebuffer[offset] = ((color >> 16) & 0xFF) as u8;     // R
                    self.framebuffer[offset + 1] = ((color >> 8) & 0xFF) as u8;  // G
                    self.framebuffer[offset + 2] = (color & 0xFF) as u8;         // B
                    self.framebuffer[offset + 3] = ((color >> 24) & 0xFF) as u8; // A
                }
            }
            _ => {}
        }
    }

    /// 清屏
    pub fn clear(&mut self, color: u32) {
        for y in 0..self.fb_info.height {
            for x in 0..self.fb_info.width {
                self.write_pixel(x, y, color);
            }
        }
    }

    /// 处理 I/O 端口读取
    pub fn io_read(&mut self, port: u16) -> u8 {
        match port {
            VGA_CRTC_DATA => self.crtc_regs[self.crtc_index as usize],
            VGA_SEQ_DATA => self.seq_regs[self.seq_index as usize],
            VGA_GFX_DATA => self.gfx_regs[self.gfx_index as usize],
            VGA_ATTR_DATA_READ => self.attr_regs[self.attr_index as usize],
            _ => 0,
        }
    }

    /// 处理 I/O 端口写入
    pub fn io_write(&mut self, port: u16, value: u8) {
        match port {
            VGA_CRTC_INDEX => self.crtc_index = value,
            VGA_CRTC_DATA => {
                self.crtc_regs[self.crtc_index as usize] = value;
                self.handle_crtc_write(self.crtc_index, value);
            }
            VGA_SEQ_INDEX => self.seq_index = value,
            VGA_SEQ_DATA => self.seq_regs[self.seq_index as usize] = value,
            VGA_GFX_INDEX => self.gfx_index = value,
            VGA_GFX_DATA => self.gfx_regs[self.gfx_index as usize] = value,
            VGA_ATTR_INDEX => self.attr_index = value,
            VGA_DAC_WRITE_INDEX => self.dac_index = value,
            VGA_DAC_DATA => {
                let idx = self.dac_index as usize;
                // DAC 数据以 RGB 顺序写入，每个颜色分量 6 位
                // 这里简化处理
            }
            _ => {}
        }
    }

    /// 处理 CRTC 寄存器写入
    fn handle_crtc_write(&mut self, index: u8, value: u8) {
        match index {
            0x0C => {
                // 起始地址高字节
                log::debug!("VGA CRTC: Start address high = 0x{:02X}", value);
            }
            0x0D => {
                // 起始地址低字节
                log::debug!("VGA CRTC: Start address low = 0x{:02X}", value);
            }
            _ => {}
        }
    }
}

/// virtio-gpu 设备
pub struct VirtioGpu {
    framebuffer: Vec<u8>,
    fb_info: FramebufferInfo,
    resources: Vec<GpuResource>,
}

/// GPU 资源
struct GpuResource {
    id: u32,
    width: u32,
    height: u32,
    format: PixelFormat,
    data: Vec<u8>,
}

impl VirtioGpu {
    /// 创建新的 virtio-gpu 设备
    pub fn new(width: u32, height: u32) -> Self {
        let format = PixelFormat::BGRA8888;
        let stride = width * format.bytes_per_pixel() as u32;
        let fb_size = (stride * height) as usize;
        
        Self {
            framebuffer: vec![0u8; fb_size],
            fb_info: FramebufferInfo {
                width,
                height,
                stride,
                format,
            },
            resources: Vec::new(),
        }
    }

    /// 创建资源
    pub fn create_resource(&mut self, id: u32, width: u32, height: u32, format: PixelFormat) {
        let size = (width * height * format.bytes_per_pixel() as u32) as usize;
        let resource = GpuResource {
            id,
            width,
            height,
            format,
            data: vec![0u8; size],
        };
        self.resources.push(resource);
        log::info!("Created GPU resource {} ({}x{})", id, width, height);
    }

    /// 附加资源到扫描输出
    pub fn attach_resource(&mut self, resource_id: u32) {
        if let Some(resource) = self.resources.iter().find(|r| r.id == resource_id) {
            // 将资源数据复制到帧缓冲
            let copy_size = resource.data.len().min(self.framebuffer.len());
            self.framebuffer[..copy_size].copy_from_slice(&resource.data[..copy_size]);
            log::info!("Attached GPU resource {} to scanout", resource_id);
        }
    }

    /// 获取帧缓冲
    pub fn get_framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    /// 获取帧缓冲信息
    pub fn get_framebuffer_info(&self) -> &FramebufferInfo {
        &self.fb_info
    }
}

/// 图形输出后端 trait
pub trait GraphicsBackend: Send + Sync {
    /// 更新显示
    fn update_display(&mut self, framebuffer: &[u8], info: &FramebufferInfo);
    
    /// 获取窗口事件
    fn poll_events(&mut self) -> Vec<GraphicsEvent>;
}

/// 图形事件
#[derive(Debug, Clone)]
pub enum GraphicsEvent {
    KeyPress(u32),
    KeyRelease(u32),
    MouseMove(i32, i32),
    MouseButton(u8, bool),
    WindowClose,
}

/// 共享图形设备
pub type SharedVgaDevice = Arc<Mutex<VgaDevice>>;
pub type SharedVirtioGpu = Arc<Mutex<VirtioGpu>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vga_device() {
        let mut vga = VgaDevice::new(VgaMode::Graphics640x480);
        assert_eq!(vga.get_framebuffer_info().width, 640);
        assert_eq!(vga.get_framebuffer_info().height, 480);
        
        // 测试像素写入
        vga.write_pixel(100, 100, 0xFF0000FF); // 红色
        
        // 测试清屏
        vga.clear(0xFF000000); // 黑色
    }

    #[test]
    fn test_virtio_gpu() {
        let mut gpu = VirtioGpu::new(1024, 768);
        gpu.create_resource(1, 1024, 768, PixelFormat::BGRA8888);
        gpu.attach_resource(1);
    }
}
