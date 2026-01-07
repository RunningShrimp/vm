//! vm-boot运行时和热插拔测试
//!
//! 测试运行时控制、热插拔设备管理功能

use std::thread;
use vm_boot::{DeviceInfo, DeviceType, HotplugEvent, HotplugManager};
use vm_boot::{RuntimeCommand, RuntimeController, RuntimeState};
use vm_core::GuestAddr;

#[cfg(test)]
mod runtime_tests {
    use super::*;

    // Test 1: RuntimeController创建
    #[test]
    fn test_runtime_controller_creation() {
        let controller = RuntimeController::new();

        assert_eq!(controller.state(), RuntimeState::Stopped);
        assert!(!controller.is_running());
        assert!(!controller.is_paused());
    }

    // Test 2: RuntimeState枚举比较
    #[test]
    fn test_runtime_state_equality() {
        assert_eq!(RuntimeState::Stopped, RuntimeState::Stopped);
        assert_ne!(RuntimeState::Running, RuntimeState::Paused);
        assert_ne!(RuntimeState::Running, RuntimeState::Stopped);
    }

    // Test 3: RuntimeCommand枚举比较
    #[test]
    fn test_runtime_command_equality() {
        assert_eq!(RuntimeCommand::Pause, RuntimeCommand::Pause);
        assert_ne!(RuntimeCommand::Pause, RuntimeCommand::Resume);
        assert_ne!(RuntimeCommand::Shutdown, RuntimeCommand::Stop);
    }

    // Test 4: 发送暂停命令
    #[test]
    fn test_send_pause_command() {
        let controller = RuntimeController::new();

        // 由于VM未运行，应该失败
        let result = controller.send_command(RuntimeCommand::Pause);
        // 命令发送成功，但执行会失败
        assert!(result.is_ok() || result.is_err());
    }

    // Test 5: 发送恢复命令
    #[test]
    fn test_send_resume_command() {
        let controller = RuntimeController::new();

        let result = controller.send_command(RuntimeCommand::Resume);
        // 命令发送成功
        assert!(result.is_ok() || result.is_err());
    }

    // Test 6: 发送关闭命令
    #[test]
    fn test_send_shutdown_command() {
        let controller = RuntimeController::new();

        let result = controller.send_command(RuntimeCommand::Shutdown);
        assert!(result.is_ok());
    }

    // Test 7: 发送停止命令
    #[test]
    fn test_send_stop_command() {
        let controller = RuntimeController::new();

        let result = controller.send_command(RuntimeCommand::Stop);
        assert!(result.is_ok());
    }

    // Test 8: 发送重置命令
    #[test]
    fn test_send_reset_command() {
        let controller = RuntimeController::new();

        let result = controller.send_command(RuntimeCommand::Reset);
        assert!(result.is_ok());
    }

    // Test 9: 发送快照保存命令
    #[test]
    fn test_send_save_snapshot_command() {
        let controller = RuntimeController::new();

        let result = controller.send_command(RuntimeCommand::SaveSnapshot);
        assert!(result.is_ok());
    }

    // Test 10: 发送快照加载命令
    #[test]
    fn test_send_load_snapshot_command() {
        let controller = RuntimeController::new();

        let result = controller.send_command(RuntimeCommand::LoadSnapshot);
        assert!(result.is_ok());
    }

    // Test 11: RuntimeState克隆
    #[test]
    fn test_runtime_state_clone() {
        let state1 = RuntimeState::Running;
        let state2 = state1;

        assert_eq!(state1, state2);
    }

    // Test 12: RuntimeCommand克隆
    #[test]
    fn test_runtime_command_clone() {
        let cmd1 = RuntimeCommand::Pause;
        let cmd2 = cmd1;

        assert_eq!(cmd1, cmd2);
    }

    // Test 13: 运行时状态转换
    #[test]
    fn test_runtime_state_transitions() {
        let states = vec![
            RuntimeState::Stopped,
            RuntimeState::Running,
            RuntimeState::Paused,
            RuntimeState::ShuttingDown,
        ];

        // 所有状态应该都是不同的（除非有重复）
        for (i, state1) in states.iter().enumerate() {
            for (j, state2) in states.iter().enumerate() {
                if i != j {
                    assert_ne!(state1, state2);
                }
            }
        }
    }

    // Test 14: 命令发送成功率
    #[test]
    fn test_command_send_success_rate() {
        let controller = RuntimeController::new();
        let commands = vec![
            RuntimeCommand::Pause,
            RuntimeCommand::Resume,
            RuntimeCommand::Shutdown,
            RuntimeCommand::Stop,
            RuntimeCommand::Reset,
        ];

        let mut success_count = 0;
        for cmd in commands {
            if controller.send_command(cmd).is_ok() {
                success_count += 1;
            }
        }

        // 所有命令应该成功发送
        assert_eq!(success_count, 5);
    }

    // Test 15: 并发命令发送
    #[test]
    fn test_concurrent_command_send() {
        let controller = std::sync::Arc::new(RuntimeController::new());
        let mut handles = vec![];

        for _ in 0..5 {
            let controller_clone = controller.clone();
            let handle =
                thread::spawn(move || controller_clone.send_command(RuntimeCommand::Pause));
            handles.push(handle);
        }

        let mut results = vec![];
        for handle in handles {
            let result = handle.join().unwrap();
            results.push(result);
        }

        // 所有发送操作应该都成功或都失败
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        assert!(success_count == 5 || success_count == 0);
    }

    // Test 16: 运行时控制器线程安全
    #[test]
    fn test_runtime_controller_thread_safety() {
        let controller = std::sync::Arc::new(RuntimeController::new());
        let controller_clone1 = controller.clone();
        let controller_clone2 = controller.clone();

        let handle1 = thread::spawn(move || controller_clone1.state());

        let handle2 = thread::spawn(move || controller_clone2.is_running());

        let state = handle1.join().unwrap();
        let running = handle2.join().unwrap();

        assert_eq!(state, RuntimeState::Stopped);
        assert!(!running);
    }

    // Test 17: RuntimeState Debug trait
    #[test]
    fn test_runtime_state_debug() {
        let state = RuntimeState::Running;
        let debug_str = format!("{:?}", state);

        assert!(debug_str.contains("Running"));
    }

    // Test 18: RuntimeCommand Debug trait
    #[test]
    fn test_runtime_command_debug() {
        let cmd = RuntimeCommand::Pause;
        let debug_str = format!("{:?}", cmd);

        assert!(debug_str.contains("Pause"));
    }

    // Test 19: 运行时控制器状态查询
    #[test]
    fn test_runtime_controller_state_queries() {
        let controller = RuntimeController::new();

        // 初始状态
        assert_eq!(controller.state(), RuntimeState::Stopped);
        assert!(!controller.is_running());
        assert!(!controller.is_paused());

        // 多次查询应该返回相同结果
        assert_eq!(controller.state(), RuntimeState::Stopped);
        assert_eq!(controller.state(), RuntimeState::Stopped);
    }

    // Test 20: 所有运行时命令类型
    #[test]
    fn test_all_runtime_commands() {
        let commands = vec![
            RuntimeCommand::Pause,
            RuntimeCommand::Resume,
            RuntimeCommand::Shutdown,
            RuntimeCommand::Stop,
            RuntimeCommand::Reset,
            RuntimeCommand::SaveSnapshot,
            RuntimeCommand::LoadSnapshot,
        ];

        // 验证所有命令都可以创建
        assert_eq!(commands.len(), 7);

        // 所有命令应该是不同的
        for (i, cmd1) in commands.iter().enumerate() {
            for (j, cmd2) in commands.iter().enumerate() {
                if i != j {
                    assert_ne!(cmd1, cmd2);
                }
            }
        }
    }
}

#[cfg(test)]
mod hotplug_tests {
    use super::*;

    // Test 21: HotplugManager创建
    #[test]
    fn test_hotplug_manager_creation() {
        let base_addr = GuestAddr(0x40000000);
        let addr_space_size = 0x10000000; // 256MB

        let manager = HotplugManager::new(base_addr, addr_space_size);

        // 验证管理器创建成功 - 不访问私有字段，只验证它能创建
        // 实际使用中需要通过公共方法来验证
        let _ = manager; // 避免unused警告
    }

    // Test 22: DeviceType枚举
    #[test]
    fn test_device_type_variants() {
        let types = vec![
            DeviceType::Block,
            DeviceType::Network,
            DeviceType::Serial,
            DeviceType::Gpu,
            DeviceType::Other,
        ];

        // 验证所有设备类型可以创建
        assert_eq!(types.len(), 5);

        // 所有类型应该不同
        for (i, type1) in types.iter().enumerate() {
            for (j, type2) in types.iter().enumerate() {
                if i != j {
                    assert_ne!(type1, type2);
                }
            }
        }
    }

    // Test 23: DeviceType比较
    #[test]
    fn test_device_type_equality() {
        assert_eq!(DeviceType::Network, DeviceType::Network);
        assert_ne!(DeviceType::Network, DeviceType::Block);
        assert_ne!(DeviceType::Gpu, DeviceType::Other);
    }

    // Test 24: DeviceType名称
    #[test]
    fn test_device_type_name() {
        assert_eq!(DeviceType::Block.name(), "block");
        assert_eq!(DeviceType::Network.name(), "network");
        assert_eq!(DeviceType::Serial.name(), "serial");
        assert_eq!(DeviceType::Gpu.name(), "gpu");
        assert_eq!(DeviceType::Other.name(), "other");
    }

    // Test 25: DeviceType克隆
    #[test]
    fn test_device_type_clone() {
        let dt1 = DeviceType::Gpu;
        let dt2 = dt1;

        assert_eq!(dt1, dt2);
    }

    // Test 26: HotplugEvent枚举
    #[test]
    fn test_hotplug_event_variants() {
        let info = DeviceInfo::new("test0", DeviceType::Other, GuestAddr(0x40000000), 0x1000);

        let events = vec![
            HotplugEvent::DeviceAdded(info.clone()),
            HotplugEvent::DeviceRemoved(info),
        ];

        assert_eq!(events.len(), 2);
        // DeviceAdded和DeviceRemoved即使参数相同也是不同的变体
    }

    // Test 27: DeviceInfo创建
    #[test]
    fn test_device_info_creation() {
        let info = DeviceInfo::new("net0", DeviceType::Network, GuestAddr(0x40000000), 0x1000);

        assert_eq!(info.id, "net0");
        assert_eq!(info.device_type, DeviceType::Network);
        assert_eq!(info.base_addr, GuestAddr(0x40000000));
        assert_eq!(info.size, 0x1000);
        assert!(info.hotpluggable);
    }

    // Test 28: DeviceInfo Builder模式
    #[test]
    fn test_device_info_builder() {
        let info = DeviceInfo::new("gpu0", DeviceType::Gpu, GuestAddr(0x50000000), 0x10000)
            .with_hotpluggable(false)
            .with_description("NVIDIA RTX4090");

        assert_eq!(info.id, "gpu0");
        assert_eq!(info.device_type, DeviceType::Gpu);
        assert!(!info.hotpluggable);
        assert_eq!(info.description, Some("NVIDIA RTX4090".to_string()));
    }

    // Test 29: 不同设备类型的DeviceInfo
    #[test]
    fn test_device_info_all_types() {
        let devices = vec![
            DeviceInfo::new("net0", DeviceType::Network, GuestAddr(0x40000000), 0x1000),
            DeviceInfo::new("gpu0", DeviceType::Gpu, GuestAddr(0x50000000), 0x10000),
            DeviceInfo::new("block0", DeviceType::Block, GuestAddr(0x60000000), 0x100000),
        ];

        assert_eq!(devices.len(), 3);
        assert_eq!(devices[0].device_type, DeviceType::Network);
        assert_eq!(devices[1].device_type, DeviceType::Gpu);
        assert_eq!(devices[2].device_type, DeviceType::Block);
    }

    // Test 30: DeviceInfo字段验证
    #[test]
    fn test_device_info_fields() {
        let info = DeviceInfo::new("serial0", DeviceType::Serial, GuestAddr(0x70000000), 0x100)
            .with_description("UART Console");

        assert_eq!(info.id, "serial0");
        assert_eq!(info.device_type, DeviceType::Serial);
        assert_eq!(info.base_addr, GuestAddr(0x70000000));
        assert_eq!(info.size, 0x100);
        assert!(info.hotpluggable);
        assert_eq!(info.description, Some("UART Console".to_string()));
    }

    // Test 31: HotplugEvent比较
    #[test]
    fn test_hotplug_event_types() {
        let info = DeviceInfo::new("test0", DeviceType::Other, GuestAddr(0x40000000), 0x1000);

        // 创建添加和移除事件
        let add_event = HotplugEvent::DeviceAdded(info.clone());
        let remove_event = HotplugEvent::DeviceRemoved(info);

        // 通过match验证事件类型
        match add_event {
            HotplugEvent::DeviceAdded(_) => {}
            HotplugEvent::DeviceRemoved(_) => panic!("Wrong type"),
            HotplugEvent::DeviceError { .. } => panic!("Wrong type"),
        }

        match remove_event {
            HotplugEvent::DeviceAdded(_) => panic!("Wrong type"),
            HotplugEvent::DeviceRemoved(_) => {}
            HotplugEvent::DeviceError { .. } => panic!("Wrong type"),
        }
    }

    // Test 32: 热插拔事件序列
    #[test]
    fn test_hotplug_event_sequence() {
        let info1 = DeviceInfo::new("dev1", DeviceType::Other, GuestAddr(0x40000000), 0x1000);
        let info2 = DeviceInfo::new("dev2", DeviceType::Other, GuestAddr(0x40001000), 0x1000);
        let info3 = DeviceInfo::new("dev3", DeviceType::Other, GuestAddr(0x40002000), 0x1000);

        let events = vec![
            HotplugEvent::DeviceAdded(info1),
            HotplugEvent::DeviceRemoved(info2),
            HotplugEvent::DeviceAdded(info3),
        ];

        // 验证事件序列
        assert_eq!(events.len(), 3);
        // 可以通过match来判断事件类型
        match &events[0] {
            HotplugEvent::DeviceAdded(_) => {}
            _ => panic!("First event should be DeviceAdded"),
        }
    }

    // Test 33: DeviceInfo克隆
    #[test]
    fn test_device_info_clone() {
        let info1 = DeviceInfo::new("test", DeviceType::Other, GuestAddr(0x80000000), 0x1000);
        let info2 = info1.clone();

        assert_eq!(info1.id, info2.id);
        assert_eq!(info1.device_type, info2.device_type);
    }

    // Test 34: DeviceType Debug trait
    #[test]
    fn test_device_type_debug() {
        let dt = DeviceType::Gpu;
        let debug_str = format!("{:?}", dt);

        assert!(debug_str.contains("Gpu"));
    }

    // Test 35: DeviceInfo Debug trait
    #[test]
    fn test_device_info_debug() {
        let info = DeviceInfo::new("eth0", DeviceType::Network, GuestAddr(0x40000000), 0x1000);

        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("Network"));
        assert!(debug_str.contains("eth0"));
    }

    // Test 36: HotplugEvent Debug trait
    #[test]
    fn test_hotplug_event_debug() {
        let info = DeviceInfo::new("eth0", DeviceType::Network, GuestAddr(0x40000000), 0x1000);
        let event = HotplugEvent::DeviceAdded(info);
        let debug_str = format!("{:?}", event);

        assert!(debug_str.contains("DeviceAdded"));
    }

    // Test 37: 设备地址对齐
    #[test]
    fn test_device_alignment() {
        let info = DeviceInfo::new("aligned", DeviceType::Block, GuestAddr(0x40000000), 0x1000);

        // 地址应该4KB对齐
        assert_eq!(info.base_addr.0, 0x40000000);
        assert!(info.base_addr.0 % 0x1000 == 0);
    }

    // Test 38: 设备大小验证
    #[test]
    fn test_device_sizes() {
        let sizes = vec![0x1000, 0x10000, 0x100000, 0x1000000];

        for size in sizes {
            let info = DeviceInfo::new("test", DeviceType::Other, GuestAddr(0x40000000), size);
            assert_eq!(info.size, size);
        }
    }

    // Test 39: 热插拔设备添加移除
    #[test]
    fn test_hotplug_add_remove_cycle() {
        let info = DeviceInfo::new("test", DeviceType::Other, GuestAddr(0x40000000), 0x1000);

        let add_event = HotplugEvent::DeviceAdded(info.clone());
        let remove_event = HotplugEvent::DeviceRemoved(info);

        // 验证事件类型 - 添加事件
        match add_event {
            HotplugEvent::DeviceAdded(device_info) => {
                assert_eq!(device_info.id, "test");
            }
            HotplugEvent::DeviceRemoved(_) => panic!("Expected DeviceAdded"),
            HotplugEvent::DeviceError { .. } => panic!("Expected DeviceAdded"),
        }

        // 验证事件类型 - 移除事件
        match remove_event {
            HotplugEvent::DeviceAdded(_) => panic!("Expected DeviceRemoved"),
            HotplugEvent::DeviceRemoved(device_info) => {
                assert_eq!(device_info.id, "test");
            }
            HotplugEvent::DeviceError { .. } => panic!("Expected DeviceRemoved"),
        }
    }

    // Test 40: 设备ID唯一性
    #[test]
    fn test_device_id_uniqueness() {
        let devices = vec![
            DeviceInfo::new("net0", DeviceType::Network, GuestAddr(0x40000000), 0x1000),
            DeviceInfo::new("net1", DeviceType::Network, GuestAddr(0x40001000), 0x1000),
        ];

        // 相同设备类型，不同ID应该创建不同设备
        assert_ne!(devices[0].id, devices[1].id);
        assert_ne!(devices[0].base_addr, devices[1].base_addr);
    }
}
