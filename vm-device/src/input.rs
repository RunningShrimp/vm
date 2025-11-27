//! VirtIO 输入设备实现
//!
//! 支持键盘、鼠标和触摸屏

use vm_core::MmioDevice;
use std::collections::VecDeque;

/// 输入设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InputDeviceType {
    /// 键盘
    Keyboard = 1,
    /// 鼠标
    Mouse = 2,
    /// 触摸屏
    Touchscreen = 3,
}

/// 输入事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum EventType {
    /// 按键事件
    Key = 0x01,
    /// 相对运动
    Rel = 0x02,
    /// 绝对位置
    Abs = 0x03,
}

/// 输入事件
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct InputEvent {
    /// 事件类型
    pub event_type: u16,
    /// 事件代码
    pub code: u16,
    /// 事件值
    pub value: i32,
}

impl InputEvent {
    /// 创建按键事件
    pub fn key(code: u16, pressed: bool) -> Self {
        Self {
            event_type: EventType::Key as u16,
            code,
            value: if pressed { 1 } else { 0 },
        }
    }

    /// 创建鼠标移动事件
    pub fn mouse_move(dx: i32, dy: i32) -> [Self; 2] {
        [
            Self {
                event_type: EventType::Rel as u16,
                code: 0, // REL_X
                value: dx,
            },
            Self {
                event_type: EventType::Rel as u16,
                code: 1, // REL_Y
                value: dy,
            },
        ]
    }

    /// 创建鼠标按键事件
    pub fn mouse_button(button: u16, pressed: bool) -> Self {
        Self {
            event_type: EventType::Key as u16,
            code: 0x110 + button, // BTN_LEFT = 0x110
            value: if pressed { 1 } else { 0 },
        }
    }

    /// 创建触摸事件
    pub fn touch(x: i32, y: i32, pressed: bool) -> [Self; 3] {
        [
            Self {
                event_type: EventType::Abs as u16,
                code: 0, // ABS_X
                value: x,
            },
            Self {
                event_type: EventType::Abs as u16,
                code: 1, // ABS_Y
                value: y,
            },
            Self {
                event_type: EventType::Key as u16,
                code: 0x14a, // BTN_TOUCH
                value: if pressed { 1 } else { 0 },
            },
        ]
    }
}

/// VirtIO Input 配置
pub struct VirtioInputConfig {
    /// 设备名称
    pub name: String,
    /// 设备类型
    pub device_type: InputDeviceType,
}

impl Default for VirtioInputConfig {
    fn default() -> Self {
        Self {
            name: "VirtIO Input".to_string(),
            device_type: InputDeviceType::Keyboard,
        }
    }
}

/// VirtIO Input 设备
pub struct VirtioInput {
    /// 配置
    config: VirtioInputConfig,
    /// 事件队列
    event_queue: VecDeque<InputEvent>,
    /// 设备状态
    status: u32,
    /// 队列选择器
    queue_sel: u32,
}

impl VirtioInput {
    /// 创建键盘设备
    pub fn keyboard() -> Self {
        Self {
            config: VirtioInputConfig {
                name: "VirtIO Keyboard".to_string(),
                device_type: InputDeviceType::Keyboard,
            },
            event_queue: VecDeque::new(),
            status: 0,
            queue_sel: 0,
        }
    }

    /// 创建鼠标设备
    pub fn mouse() -> Self {
        Self {
            config: VirtioInputConfig {
                name: "VirtIO Mouse".to_string(),
                device_type: InputDeviceType::Mouse,
            },
            event_queue: VecDeque::new(),
            status: 0,
            queue_sel: 0,
        }
    }

    /// 创建触摸屏设备
    pub fn touchscreen() -> Self {
        Self {
            config: VirtioInputConfig {
                name: "VirtIO Touchscreen".to_string(),
                device_type: InputDeviceType::Touchscreen,
            },
            event_queue: VecDeque::new(),
            status: 0,
            queue_sel: 0,
        }
    }

    /// 发送事件
    pub fn send_event(&mut self, event: InputEvent) {
        self.event_queue.push_back(event);
        log::debug!("Input event: type={} code={} value={}", 
                   event.event_type, event.code, event.value);
    }

    /// 发送多个事件
    pub fn send_events(&mut self, events: &[InputEvent]) {
        for event in events {
            self.send_event(*event);
        }
    }

    /// 获取待处理事件数
    pub fn pending_events(&self) -> usize {
        self.event_queue.len()
    }

    /// 弹出事件
    pub fn pop_event(&mut self) -> Option<InputEvent> {
        self.event_queue.pop_front()
    }

    /// 处理键盘按键
    pub fn handle_key(&mut self, keycode: u16, pressed: bool) {
        self.send_event(InputEvent::key(keycode, pressed));
    }

    /// 处理鼠标移动
    pub fn handle_mouse_move(&mut self, dx: i32, dy: i32) {
        let events = InputEvent::mouse_move(dx, dy);
        self.send_events(&events);
    }

    /// 处理鼠标按键
    pub fn handle_mouse_button(&mut self, button: u16, pressed: bool) {
        self.send_event(InputEvent::mouse_button(button, pressed));
    }

    /// 处理触摸
    pub fn handle_touch(&mut self, x: i32, y: i32, pressed: bool) {
        let events = InputEvent::touch(x, y, pressed);
        self.send_events(&events);
    }
}

impl MmioDevice for VirtioInput {
    fn read(&self, offset: u64, _size: u8) -> u64 {
        match offset {
            0x00 => 0x74726976, // Magic "virt"
            0x04 => 2,          // Version
            0x08 => 18,         // Device ID (Input)
            0x0C => 0x554D4551, // Vendor ID
            0x10 => 0x00000001, // Device features (low)
            0x14 => 0x00000001, // Device features (high): VIRTIO_F_VERSION_1
            0x70 => self.status as u64, // Status
            // 配置空间 (0x100+)
            0x100 => self.config.device_type as u64,
            0x101 => self.pending_events() as u64,
            _ => 0,
        }
    }

    fn write(&mut self, offset: u64, val: u64, _size: u8) {
        match offset {
            0x30 => self.queue_sel = val as u32, // Queue select
            0x50 => {
                // Queue notify
                log::debug!("VirtioInput: Queue {} notified", val);
                // 这里应该处理队列请求
            }
            0x70 => self.status = val as u32, // Status
            _ => {
                log::trace!("VirtioInput write: offset={:#x} val={:#x}", offset, val);
            }
        }
    }
}

/// 键盘扫描码
pub mod keycodes {
    pub const KEY_ESC: u16 = 1;
    pub const KEY_1: u16 = 2;
    pub const KEY_2: u16 = 3;
    pub const KEY_3: u16 = 4;
    pub const KEY_4: u16 = 5;
    pub const KEY_5: u16 = 6;
    pub const KEY_6: u16 = 7;
    pub const KEY_7: u16 = 8;
    pub const KEY_8: u16 = 9;
    pub const KEY_9: u16 = 10;
    pub const KEY_0: u16 = 11;
    pub const KEY_MINUS: u16 = 12;
    pub const KEY_EQUAL: u16 = 13;
    pub const KEY_BACKSPACE: u16 = 14;
    pub const KEY_TAB: u16 = 15;
    pub const KEY_Q: u16 = 16;
    pub const KEY_W: u16 = 17;
    pub const KEY_E: u16 = 18;
    pub const KEY_R: u16 = 19;
    pub const KEY_T: u16 = 20;
    pub const KEY_Y: u16 = 21;
    pub const KEY_U: u16 = 22;
    pub const KEY_I: u16 = 23;
    pub const KEY_O: u16 = 24;
    pub const KEY_P: u16 = 25;
    pub const KEY_LEFTBRACE: u16 = 26;
    pub const KEY_RIGHTBRACE: u16 = 27;
    pub const KEY_ENTER: u16 = 28;
    pub const KEY_LEFTCTRL: u16 = 29;
    pub const KEY_A: u16 = 30;
    pub const KEY_S: u16 = 31;
    pub const KEY_D: u16 = 32;
    pub const KEY_F: u16 = 33;
    pub const KEY_G: u16 = 34;
    pub const KEY_H: u16 = 35;
    pub const KEY_J: u16 = 36;
    pub const KEY_K: u16 = 37;
    pub const KEY_L: u16 = 38;
    pub const KEY_SEMICOLON: u16 = 39;
    pub const KEY_APOSTROPHE: u16 = 40;
    pub const KEY_GRAVE: u16 = 41;
    pub const KEY_LEFTSHIFT: u16 = 42;
    pub const KEY_BACKSLASH: u16 = 43;
    pub const KEY_Z: u16 = 44;
    pub const KEY_X: u16 = 45;
    pub const KEY_C: u16 = 46;
    pub const KEY_V: u16 = 47;
    pub const KEY_B: u16 = 48;
    pub const KEY_N: u16 = 49;
    pub const KEY_M: u16 = 50;
    pub const KEY_SPACE: u16 = 57;
}

/// 鼠标按键
pub mod mouse_buttons {
    pub const BTN_LEFT: u16 = 0;
    pub const BTN_RIGHT: u16 = 1;
    pub const BTN_MIDDLE: u16 = 2;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_creation() {
        let keyboard = VirtioInput::keyboard();
        
        assert_eq!(keyboard.read(0x00, 4), 0x74726976); // Magic
        assert_eq!(keyboard.read(0x08, 4), 18); // Device ID
        assert_eq!(keyboard.config.device_type, InputDeviceType::Keyboard);
    }

    #[test]
    fn test_key_event() {
        let mut keyboard = VirtioInput::keyboard();
        
        keyboard.handle_key(keycodes::KEY_A, true);
        assert_eq!(keyboard.pending_events(), 1);
        
        let event = keyboard.pop_event().expect("Failed to pop keyboard event");
        assert_eq!(event.event_type, EventType::Key as u16);
        assert_eq!(event.code, keycodes::KEY_A);
        assert_eq!(event.value, 1);
    }

    #[test]
    fn test_mouse_move() {
        let mut mouse = VirtioInput::mouse();
        
        mouse.handle_mouse_move(10, -5);
        assert_eq!(mouse.pending_events(), 2);
        
        let event1 = mouse.pop_event().expect("Failed to pop first mouse event");
        assert_eq!(event1.value, 10);
        
        let event2 = mouse.pop_event().expect("Failed to pop second mouse event");
        assert_eq!(event2.value, -5);
    }

    #[test]
    fn test_touch() {
        let mut touchscreen = VirtioInput::touchscreen();
        
        touchscreen.handle_touch(100, 200, true);
        assert_eq!(touchscreen.pending_events(), 3);
    }
}
