//! 输入系统映射器
//!
//! 将主机输入（键盘、鼠标、游戏手柄）映射到虚拟机输入。
//!
//! ## 功能
//!
//! - 键盘映射
//! - 鼠标映射
//! - 游戏手柄映射
//! - 触摸屏映射（移动设备）

use std::collections::HashMap;

/// 虚拟输入设备
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VmInputDevice {
    Keyboard,
    Mouse,
    Gamepad(u32), // 游戏手柄ID
    Touchscreen,
}

/// 虚拟输入代码
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VmInputCode {
    Keyboard(u32),
    MouseButton(u8),
    MouseAxis { axis: u8, value: i32 },
    GamepadButton(u32),
    GamepadAxis { axis: u8, value: f32 },
}

/// 主机输入
#[derive(Debug, Clone)]
pub enum HostInput {
    Keyboard {
        keycode: u32,
        pressed: bool,
    },
    Mouse {
        button: Option<u8>,
        axis: Option<(u8, i32)>,
    },
    Gamepad {
        device_id: u32,
        button: Option<u32>,
        axis: Option<(u8, f32)>,
    },
    Touchscreen {
        x: u16,
        y: u16,
        pressed: bool,
    },
}

/// 输入映射器
pub struct InputMapper {
    /// 键盘映射表
    keyboard_map: HashMap<u32, VmInputCode>,

    /// 鼠标按钮映射
    mouse_button_map: HashMap<u8, VmInputCode>,

    /// 游戏手柄映射
    gamepad_maps: HashMap<u32, GamepadMapping>,

    /// 映射统计
    stats: InputMappingStats,
}

/// 游戏手柄映射配置
#[derive(Debug, Clone)]
pub struct GamepadMapping {
    pub device_id: u32,
    pub button_map: HashMap<u32, VmInputCode>,
    pub axis_map: HashMap<u8, VmInputCode>,
    pub deadzone: f32,
}

/// 输入映射统计
#[derive(Debug, Clone, Default)]
pub struct InputMappingStats {
    pub total_mappings: u64,
    pub successful_mappings: u64,
    pub unmapped_inputs: u64,
}

impl InputMapper {
    /// 创建新的输入映射器
    pub fn new() -> Self {
        Self {
            keyboard_map: HashMap::new(),
            mouse_button_map: HashMap::new(),
            gamepad_maps: HashMap::new(),
            stats: InputMappingStats::default(),
        }
    }

    /// 映射键盘输入
    pub fn map_keyboard(&mut self, host_keycode: u32, vm_keycode: u32) {
        self.keyboard_map
            .insert(host_keycode, VmInputCode::Keyboard(vm_keycode));
        self.stats.total_mappings += 1;
    }

    /// 映射鼠标按钮
    pub fn map_mouse_button(&mut self, host_button: u8, vm_button: u8) {
        self.mouse_button_map
            .insert(host_button, VmInputCode::MouseButton(vm_button));
        self.stats.total_mappings += 1;
    }

    /// 添加游戏手柄映射
    pub fn add_gamepad_mapping(&mut self, mapping: GamepadMapping) {
        self.gamepad_maps.insert(mapping.device_id, mapping);
        self.stats.total_mappings += 1;
    }

    /// 翻译主机输入为虚拟输入
    pub fn translate_input(&mut self, input: &HostInput) -> Option<(VmInputDevice, VmInputCode)> {
        self.stats.successful_mappings += 1;

        match input {
            HostInput::Keyboard { keycode, .. } => {
                if let Some(vm_code) = self.keyboard_map.get(keycode) {
                    Some((VmInputDevice::Keyboard, *vm_code))
                } else {
                    self.stats.unmapped_inputs += 1;
                    None
                }
            }
            HostInput::Mouse { button, axis } => {
                if let Some(btn) = button {
                    if let Some(vm_code) = self.mouse_button_map.get(btn) {
                        return Some((VmInputDevice::Mouse, *vm_code));
                    }
                }

                if let Some((ax, val)) = axis {
                    return Some((
                        VmInputDevice::Mouse,
                        VmInputCode::MouseAxis {
                            axis: *ax,
                            value: *val,
                        },
                    ));
                }

                None
            }
            HostInput::Gamepad {
                device_id,
                button,
                axis,
            } => {
                if let Some(mapping) = self.gamepad_maps.get(device_id) {
                    if let Some(btn) = button {
                        if let Some(vm_code) = mapping.button_map.get(btn) {
                            return Some((VmInputDevice::Gamepad(*device_id), *vm_code));
                        }
                    }

                    if let Some((ax, val)) = axis {
                        if let Some(_vm_code) = mapping.axis_map.get(ax) {
                            return Some((
                                VmInputDevice::Gamepad(*device_id),
                                VmInputCode::GamepadAxis {
                                    axis: *ax,
                                    value: *val,
                                },
                            ));
                        }
                    }
                }

                None
            }
            HostInput::Touchscreen { .. } => {
                // 触摸屏输入直接传递
                None
            }
        }
    }

    /// 获取映射统计
    pub fn get_stats(&self) -> &InputMappingStats {
        &self.stats
    }

    /// 加载预设映射配置
    pub fn load_preset(&mut self, preset: InputPreset) {
        match preset {
            InputPreset::Standard => {
                // 标准键盘布局
                self.map_keyboard(30, 30); // A → A
                self.map_keyboard(31, 31); // S → S
                // ... 更多映射
            }
            InputPreset::Gaming => {
                // 游戏优化布局
                self.map_keyboard(42, 30); // Shift_L → A
                // ... 游戏特定映射
            }
        }
    }
}

/// 输入预设配置
#[derive(Debug, Clone, Copy)]
pub enum InputPreset {
    Standard,
    Gaming,
}

impl Default for InputMapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_mapper_creation() {
        let mapper = InputMapper::new();
        assert_eq!(mapper.stats.total_mappings, 0);
    }

    #[test]
    fn test_keyboard_mapping() {
        let mut mapper = InputMapper::new();

        mapper.map_keyboard(30, 30);
        mapper.map_keyboard(31, 31);

        assert_eq!(mapper.keyboard_map.len(), 2);
        assert_eq!(mapper.stats.total_mappings, 2);
    }

    #[test]
    fn test_keyboard_translation() {
        let mut mapper = InputMapper::new();
        mapper.map_keyboard(30, 30);

        let input = HostInput::Keyboard {
            keycode: 30,
            pressed: true,
        };

        let result = mapper.translate_input(&input);
        assert!(result.is_some());

        let (device, code) = result.unwrap();
        assert_eq!(device, VmInputDevice::Keyboard);
        assert_eq!(code, VmInputCode::Keyboard(30));
    }

    #[test]
    fn test_unmapped_input() {
        let mut mapper = InputMapper::new();

        let input = HostInput::Keyboard {
            keycode: 999,
            pressed: true,
        };

        let result = mapper.translate_input(&input);
        assert!(result.is_none());
        assert_eq!(mapper.stats.unmapped_inputs, 1);
    }

    #[test]
    fn test_mouse_translation() {
        let mut mapper = InputMapper::new();
        mapper.map_mouse_button(1, 1);

        let input = HostInput::Mouse {
            button: Some(1),
            axis: None,
        };

        let result = mapper.translate_input(&input);
        assert!(result.is_some());
    }

    #[test]
    fn test_gamepad_mapping() {
        let mut mapper = InputMapper::new();

        let mut button_map = HashMap::new();
        button_map.insert(0, VmInputCode::GamepadButton(100));

        let mut axis_map = HashMap::new();
        axis_map.insert(
            0,
            VmInputCode::GamepadAxis {
                axis: 0,
                value: 0.5,
            },
        );

        mapper.add_gamepad_mapping(GamepadMapping {
            device_id: 0,
            button_map,
            axis_map,
            deadzone: 0.15,
        });

        assert_eq!(mapper.gamepad_maps.len(), 1);
    }
}
