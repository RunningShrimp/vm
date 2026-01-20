//! Display Module - GUI and Terminal Mode Support
//!
//! Handles rendering framebuffer data for GUI mode and serial port output for terminal mode.

use std::io::Cursor;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FramebufferConfig {
    pub width: u32,
    pub height: u32,
    pub bits_per_pixel: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct FramebufferUpdate {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Base64-encoded image data or raw pixel data
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TerminalData {
    pub sequence: String,
}

/// Manages display output for both GUI and Terminal modes
pub struct DisplayManager {
    fb_tx: mpsc::UnboundedSender<FramebufferUpdate>,
    term_tx: mpsc::UnboundedSender<TerminalData>,
}

impl DisplayManager {
    /// Create a new display manager
    pub fn new() -> (
        Self,
        mpsc::UnboundedReceiver<FramebufferUpdate>,
        mpsc::UnboundedReceiver<TerminalData>,
    ) {
        let (fb_tx, fb_rx) = mpsc::unbounded_channel();
        let (term_tx, term_rx) = mpsc::unbounded_channel();

        (Self { fb_tx, term_tx }, fb_rx, term_rx)
    }

    /// Send a framebuffer update (GUI mode)
    pub fn send_framebuffer_update(&self, update: FramebufferUpdate) -> Result<(), String> {
        self.fb_tx.send(update).map_err(|e| e.to_string())
    }

    /// Send terminal data (Terminal mode)
    pub fn send_terminal_data(&self, data: TerminalData) -> Result<(), String> {
        self.term_tx.send(data).map_err(|e| e.to_string())
    }
}

impl Default for DisplayManager {
    fn default() -> Self {
        let (manager, _, _) = Self::new();
        manager
    }
}

/// Framebuffer renderer - converts guest framebuffer to displayable format
pub struct FramebufferRenderer {
    width: u32,
    height: u32,
    bits_per_pixel: u8,
}

impl FramebufferRenderer {
    pub fn new(config: FramebufferConfig) -> Self {
        Self {
            width: config.width,
            height: config.height,
            bits_per_pixel: config.bits_per_pixel,
        }
    }

    /// Convert raw framebuffer data to PNG or other displayable format
    pub fn render_frame(&self, raw_data: &[u8]) -> Result<Vec<u8>, String> {
        // Basic implementation: validate data size based on bits_per_pixel
        let expected_bytes = (self.width * self.height * self.bits_per_pixel as u32) / 8;
        if raw_data.len() < expected_bytes as usize {
            return Err(format!(
                "Insufficient framebuffer data: expected {} bytes, got {}",
                expected_bytes,
                raw_data.len()
            ));
        }

        // 实现帧缓冲区渲染
        // 1. 转换像素格式 (ARGB, RGB等)
        // 2. 处理字节序
        // 3. 编码为PNG/JPEG以高效传输

        // 简单实现：假设输入为RGB格式
        let mut png_data = Vec::new();
        let cursor = Cursor::new(&mut png_data);

        // 将原始数据转换为图像格式
        let img = image::ImageBuffer::from_raw(self.width, self.height, raw_data.to_vec())
            .ok_or("Failed to create image buffer".to_string())?;

        // 编码为PNG
        let dyn_img = image::DynamicImage::ImageRgb8(img);
        dyn_img
            .write_to(cursor, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;

        Ok(png_data)
    }

    /// Handle dynamic resolution change
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

/// Terminal emulator interface
pub struct TerminalEmulator {
    columns: u32,
    rows: u32,
    buffer: Vec<char>,
}

impl TerminalEmulator {
    pub fn new(columns: u32, rows: u32) -> Self {
        Self {
            columns,
            rows,
            buffer: vec![' '; (columns * rows) as usize],
        }
    }

    /// Process incoming terminal data (handles ANSI sequences)
    pub fn process_data(&mut self, data: &[u8]) -> Result<(), String> {
        // Convert data to string
        let text = String::from_utf8_lossy(data).to_string();

        let mut result: Vec<String> = Vec::new();
        let mut i = 0;

        // Convert to char iterator for efficient indexing
        let chars: Vec<char> = text.chars().collect();

        while i < chars.len() {
            if chars[i] == '\x1b' {
                // 检测到转义序列
                if i + 1 < chars.len() && chars[i + 1] == '[' {
                    // CSI序列
                    // Find the end of the sequence (look for 'm' character)
                    let seq_end = chars[i..]
                        .iter()
                        .position(|&c| c == 'm')
                        .map(|pos| i + pos)
                        .unwrap_or(chars.len());

                    if seq_end < chars.len() {
                        let seq: String = chars[i..=seq_end].iter().collect();

                        // 解析颜色代码
                        if seq.contains("31m") {
                            result.push("<span style=\"color:red\">".to_string());
                        } else if seq.contains("32m") {
                            result.push("<span style=\"color:green\">".to_string());
                        } else if seq.contains("0m") {
                            result.push("</span>".to_string());
                        }
                    }

                    i = seq_end + 1;
                } else {
                    result.push(chars[i].to_string());
                    i += 1;
                }
            } else {
                result.push(chars[i].to_string());
                i += 1;
            }
        }

        // Basic implementation: treat result as raw text and add to buffer
        for c in result.join("").chars() {
            let c_u8 = c as u8;
            if (32..=126).contains(&c_u8) {
                // Printable ASCII range
                // Find first empty space in buffer (simplified)
                if let Some(pos) = self.buffer.iter().position(|&c| c == ' ') {
                    self.buffer[pos] = c;
                }
            }
        }

        Ok(())
    }

    /// Get current terminal content
    pub fn get_content(&self) -> String {
        self.buffer.iter().collect()
    }

    /// Handle terminal resize
    pub fn resize(&mut self, columns: u32, rows: u32) {
        self.columns = columns;
        self.rows = rows;
        self.buffer.resize((columns * rows) as usize, ' ');
    }
}
