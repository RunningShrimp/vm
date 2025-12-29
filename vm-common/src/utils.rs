//! 通用工具函数
//!
//! 提供跨模块共享的常用工具函数，避免代码重复

/// 向上对齐到指定对齐边界
///
/// # 参数
/// - `value`: 要对齐的值
/// - `alignment`: 对齐边界（必须是2的幂）
///
/// # 返回
/// 对齐后的值
///
/// # 示例
/// ```
/// assert_eq!(vm_common::utils::align_up(5, 8), 8);
/// assert_eq!(vm_common::utils::align_up(16, 8), 16);
/// assert_eq!(vm_common::utils::align_up(17, 8), 24);
/// ```
#[inline]
pub const fn align_up(value: u64, alignment: u64) -> u64 {
    (value + alignment - 1) & !(alignment - 1)
}

/// 向下对齐到指定对齐边界
///
/// # 参数
/// - `value`: 要对齐的值
/// - `alignment`: 对齐边界（必须是2的幂）
///
/// # 返回
/// 对齐后的值
///
/// # 示例
/// ```
/// assert_eq!(vm_common::utils::align_down(5, 8), 0);
/// assert_eq!(vm_common::utils::align_down(16, 8), 16);
/// assert_eq!(vm_common::utils::align_down(17, 8), 16);
/// ```
#[inline]
pub const fn align_down(value: u64, alignment: u64) -> u64 {
    value & !(alignment - 1)
}

/// 检查值是否对齐到指定边界
///
/// # 参数
/// - `value`: 要检查的值
/// - `alignment`: 对齐边界
///
/// # 返回
/// 如果对齐返回 true，否则返回 false
///
/// # 示例
/// ```
/// assert!(vm_common::utils::is_aligned(16, 8));
/// assert!(!vm_common::utils::is_aligned(17, 8));
/// ```
#[inline]
pub const fn is_aligned(value: u64, alignment: u64) -> bool {
    value.is_multiple_of(alignment)
}

/// 检查是否是2的幂
///
/// # 参数
/// - `value`: 要检查的值
///
/// # 返回
/// 如果是2的幂返回 true，否则返回 false
#[inline]
pub const fn is_power_of_two(value: u64) -> bool {
    value > 0 && (value & (value - 1)) == 0
}

/// 计算向上取整除法
///
/// # 参数
/// - `numerator`: 分子
/// - `denominator`: 分母
///
/// # 返回
/// 向上取整的结果
#[inline]
pub const fn ceil_div(numerator: u64, denominator: u64) -> u64 {
    numerator.div_ceil(denominator)
}

/// 计算下一个2的幂
///
/// # 参数
/// - `value`: 输入值
///
/// # 返回
/// 不小于输入值的最小2的幂
#[inline]
pub fn next_power_of_two(mut value: u64) -> u64 {
    if value == 0 {
        return 1;
    }
    value -= 1;
    value |= value >> 1;
    value |= value >> 2;
    value |= value >> 4;
    value |= value >> 8;
    value |= value >> 16;
    value |= value >> 32;
    value + 1
}

/// 将字节数格式化为人类可读的形式
///
/// # 参数
/// - `bytes`: 字节数
///
/// # 返回
/// 格式化后的字符串（如 "1.5 KB", "2.3 MB"）
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else if size < 10.0 {
        format!("{:.2} {}", size, UNITS[unit_index])
    } else if size < 100.0 {
        format!("{:.1} {}", size, UNITS[unit_index])
    } else {
        format!("{:.0} {}", size, UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 8), 0);
        assert_eq!(align_up(5, 8), 8);
        assert_eq!(align_up(8, 8), 8);
        assert_eq!(align_up(16, 8), 16);
        assert_eq!(align_up(17, 8), 24);
        assert_eq!(align_up(4095, 4096), 4096);
        assert_eq!(align_up(4096, 4096), 4096);
    }

    #[test]
    fn test_align_down() {
        assert_eq!(align_down(0, 8), 0);
        assert_eq!(align_down(5, 8), 0);
        assert_eq!(align_down(8, 8), 8);
        assert_eq!(align_down(16, 8), 16);
        assert_eq!(align_down(17, 8), 16);
        assert_eq!(align_down(4095, 4096), 0);
        assert_eq!(align_down(4096, 4096), 4096);
    }

    #[test]
    fn test_is_aligned() {
        assert!(is_aligned(0, 8));
        assert!(is_aligned(8, 8));
        assert!(is_aligned(16, 8));
        assert!(is_aligned(4096, 4096));
        assert!(!is_aligned(5, 8));
        assert!(!is_aligned(17, 8));
    }

    #[test]
    fn test_is_power_of_two() {
        assert!(is_power_of_two(1));
        assert!(is_power_of_two(2));
        assert!(is_power_of_two(4));
        assert!(is_power_of_two(8));
        assert!(is_power_of_two(16));
        assert!(is_power_of_two(256));
        assert!(is_power_of_two(4096));
        assert!(!is_power_of_two(0));
        assert!(!is_power_of_two(3));
        assert!(!is_power_of_two(5));
        assert!(!is_power_of_two(12));
    }

    #[test]
    fn test_ceil_div() {
        assert_eq!(ceil_div(0, 8), 0);
        assert_eq!(ceil_div(5, 8), 1);
        assert_eq!(ceil_div(8, 8), 1);
        assert_eq!(ceil_div(9, 8), 2);
        assert_eq!(ceil_div(16, 8), 2);
        assert_eq!(ceil_div(17, 8), 3);
    }

    #[test]
    fn test_next_power_of_two() {
        assert_eq!(next_power_of_two(0), 1);
        assert_eq!(next_power_of_two(1), 1);
        assert_eq!(next_power_of_two(2), 2);
        assert_eq!(next_power_of_two(3), 4);
        assert_eq!(next_power_of_two(5), 8);
        assert_eq!(next_power_of_two(16), 16);
        assert_eq!(next_power_of_two(17), 32);
        assert_eq!(next_power_of_two(1000), 1024);
        assert_eq!(next_power_of_two(4096), 4096);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1 MB");
        assert_eq!(format_bytes(1073741824), "1 GB");
        assert_eq!(format_bytes(1099511627776), "1 TB");
    }
}
