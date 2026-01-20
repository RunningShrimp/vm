//! # SDL2 VGA Display Frontend
//!
//! 使用SDL2显示VGA文本模式(80x25字符)


/// VGA文本模式显示前端
pub struct SdlDisplayFrontend {
    /// 窗口标题
    title: String,
    /// VGA缓冲区 (80x25 = 4000字符, 每个字符2字节: 属性+字符)
    vga_buffer: [u16; 80 * 25],
    /// 字体数据 (8x16 bitmap font)
    font_data: Vec<u8>,
    /// 是否已初始化
    initialized: bool,
}

impl SdlDisplayFrontend {
    /// 创建新的SDL显示前端
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            title: "Debian Installer - VGA Display".to_string(),
            vga_buffer: [0; 80 * 25],
            font_data: Self::load_default_font(),
            initialized: false,
        })
    }

    /// 初始化SDL显示
    pub fn init(&mut self) -> Result<(), String> {
        // 在实际实现中，这里会初始化SDL2
        // 由于SDL2是外部依赖，这里使用模拟版本
        log::info!("SDL2 display frontend initialized");
        self.initialized = true;
        Ok(())
    }

    /// 更新VGA显示
    pub fn update(&mut self, vga_data: &[u16]) -> Result<(), String> {
        if !self.initialized {
            return Err("Display not initialized".to_string());
        }

        // 复制VGA数据
        let len = std::cmp::min(vga_data.len(), 80 * 25);
        self.vga_buffer[..len].copy_from_slice(&vga_data[..len]);

        // 在实际实现中，这里会渲染到SDL窗口
        self.render();

        Ok(())
    }

    /// 从内存地址更新VGA显示
    pub fn update_from_memory(&mut self, mem_slice: &[u8]) -> Result<(), String> {
        if !self.initialized {
            return Err("Display not initialized".to_string());
        }

        // VGA文本模式缓冲区格式: 每个字符2字节(属性+ASCII)
        // 80列 x 25行 = 4000字符 = 8000字节
        let vga_bytes: usize = (80 * 25 * 2).min(mem_slice.len());

        for i in 0..(vga_bytes / 2) {
            let attr = mem_slice[i * 2];
            let ch = mem_slice[i * 2 + 1];
            self.vga_buffer[i] = ((attr as u16) << 8) | (ch as u16);
        }

        self.render();
        Ok(())
    }

    /// 渲染VGA显示
    fn render(&self) {
        // 输出VGA内容到日志 (用于调试)
        let output = self.format_vga_text();
        log::debug!("VGA Display:\n{}", output);

        // 在实际SDL实现中，这里会:
        // 1. 清空画布
        // 2. 遍历VGA缓冲区
        // 3. 渲染每个字符
        // 4. 更新窗口
    }

    /// 格式化VGA文本 (用于日志输出)
    fn format_vga_text(&self) -> String {
        let mut output = String::new();
        output.push('┌');
        output.push_str(&"─".repeat(78));
        output.push_str("┐\n");

        for row in 0..25 {
            output.push('│');
            for col in 0..80 {
                let idx = row * 80 + col;
                let cell = self.vga_buffer[idx];
                let ch = (cell & 0xFF) as u8;
                let _attr = ((cell >> 8) & 0xFF) as u8;

                // 只渲染可打印字符
                if (32..=126).contains(&ch) {
                    output.push(ch as char);
                } else if ch == 0 {
                    output.push(' ');
                } else {
                    output.push('.');
                }
            }
            output.push_str("│\n");
        }

        output.push('└');
        output.push_str(&"─".repeat(78));
        output.push_str("┘\n");

        output
    }

    /// 获取VGA缓冲区引用
    pub fn vga_buffer(&self) -> &[u16; 80 * 25] {
        &self.vga_buffer
    }

    /// 获取VGA缓冲区可变引用
    pub fn vga_buffer_mut(&mut self) -> &mut [u16; 80 * 25] {
        &mut self.vga_buffer
    }

    /// 加载默认字体数据
    fn load_default_font() -> Vec<u8> {
        // 简单的8x16 bitmap字体数据 (只包含ASCII 32-126)
        // 在实际实现中，应该从文件加载完整的字体数据
        let font = vec![0u8; 128 * 16]; // 128字符 x 16行

        // 这里可以添加实际的字体数据
        font
    }

    /// 设置窗口标题
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    /// 检查是否已初始化
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// 捕获到文件 (用于保存截图)
    pub fn save_screenshot(&self, path: &str) -> Result<(), String> {
        let output = self.format_vga_text();
        std::fs::write(path, output).map_err(|e| format!("Failed to save screenshot: {}", e))?;
        log::info!("Screenshot saved to: {}", path);
        Ok(())
    }
}

impl Default for SdlDisplayFrontend {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

/// VGA显示快照
#[derive(Debug, Clone)]
pub struct VgaSnapshot {
    pub vga_buffer: Vec<u16>,
    pub timestamp: std::time::Instant,
}

impl VgaSnapshot {
    /// 从SDL前端创建快照
    pub fn from_display(display: &SdlDisplayFrontend) -> Self {
        Self {
            vga_buffer: display.vga_buffer().to_vec(),
            timestamp: std::time::Instant::now(),
        }
    }

    /// 保存到文件
    pub fn save(&self, path: &str) -> Result<(), String> {
        let mut output = String::new();
        output.push('┌');
        output.push_str(&"─".repeat(78));
        output.push_str("┐\n");

        for row in 0..25 {
            output.push('│');
            for col in 0..80 {
                let idx = row * 80 + col;
                if idx < self.vga_buffer.len() {
                    let cell = self.vga_buffer[idx];
                    let ch = (cell & 0xFF) as u8;
                    if (32..=126).contains(&ch) {
                        output.push(ch as char);
                    } else if ch == 0 {
                        output.push(' ');
                    } else {
                        output.push('.');
                    }
                }
            }
            output.push_str("│\n");
        }

        output.push('└');
        output.push_str(&"─".repeat(78));
        output.push_str("┘\n");

        std::fs::write(path, output).map_err(|e| format!("Failed to save snapshot: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_create() {
        let display = SdlDisplayFrontend::new().unwrap();
        assert!(!display.is_initialized());
    }

    #[test]
    fn test_display_init() {
        let mut display = SdlDisplayFrontend::new().unwrap();
        display.init().unwrap();
        assert!(display.is_initialized());
    }

    #[test]
    fn test_vga_buffer() {
        let mut display = SdlDisplayFrontend::new().unwrap();
        let buffer = display.vga_buffer_mut();
        assert_eq!(buffer.len(), 80 * 25);

        // 写入测试数据
        buffer[0] = 0x0F00 | b'H' as u16;
        buffer[1] = 0x0F00 | b'e' as u16;
        buffer[2] = 0x0F00 | b'l' as u16;
        buffer[3] = 0x0F00 | b'l' as u16;
        buffer[4] = 0x0F00 | b'o' as u16;

        let text = display.format_vga_text();
        assert!(text.contains("Hello"));
    }

    #[test]
    fn test_screenshot() {
        let mut display = SdlDisplayFrontend::new().unwrap();
        display.init().unwrap();

        // 写入测试数据
        let buffer = display.vga_buffer_mut();
        for i in 0..80 {
            buffer[i] = 0x0F00 | b'X' as u16;
        }

        // 保存截图
        let result = display.save_screenshot("/tmp/test_vga_screenshot.txt");
        assert!(result.is_ok());

        // 验证文件内容
        let content = std::fs::read_to_string("/tmp/test_vga_screenshot.txt").unwrap();
        assert!(content.contains("┌────────"));
        assert!(content.lines().nth(1).unwrap().contains('X'));

        // 清理
        let _ = std::fs::remove_file("/tmp/test_vga_screenshot.txt");
    }
}
