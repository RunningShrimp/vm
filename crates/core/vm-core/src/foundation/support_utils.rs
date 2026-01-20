// 工具函数（Utilities）
//
// 本模块提供VM开发的常用工具函数：
// - 位操作
// - 内存操作
// - 数据结构辅助

/// 位操作工具
pub mod bit_ops {
    /// 检查是否设置了特定位
    pub fn is_set(val: u64, bit: u8) -> bool {
        (val & (1u64 << bit)) != 0
    }

    /// 设置特定位
    pub fn set_bit(val: u64, bit: u8) -> u64 {
        val | (1u64 << bit)
    }

    /// 清除特定位
    pub fn clear_bit(val: u64, bit: u8) -> u64 {
        val & !(1u64 << bit)
    }

    /// 切换特定位
    pub fn toggle_bit(val: u64, bit: u8) -> u64 {
        val ^ (1u64 << bit)
    }

    /// 提取位字段
    pub fn extract_bits(val: u64, start: u8, end: u8) -> u64 {
        let mask = ((1u64 << (end - start)) - 1) << start;
        (val & mask) >> start
    }

    /// 检查多个位是否全部设置
    pub fn is_set_all(val: u64, mask: u64) -> bool {
        (val & mask) == mask
    }

    /// 检查多个位中是否至少有一个设置
    pub fn is_set_any(val: u64, mask: u64) -> bool {
        (val & mask) != 0
    }
}

/// 内存操作工具
pub mod mem_ops {
    /// 对齐到指定大小
    pub fn align_up(val: u64, alignment: u64) -> u64 {
        (val + alignment - 1) & !(alignment - 1)
    }

    /// 向下对齐到指定大小
    pub fn align_down(val: u64, alignment: u64) -> u64 {
        val & !(alignment - 1)
    }

    /// 检查是否对齐
    pub fn is_aligned(val: u64, alignment: u64) -> bool {
        (val & (alignment - 1)) == 0
    }

    /// 计算对齐偏移
    pub fn alignment_offset(val: u64, alignment: u64) -> u64 {
        val & (alignment - 1)
    }

    /// 分配页面大小的内存
    pub fn alloc_page_aligned(size: usize) -> *mut u8 {
        unsafe extern "C" {
            fn aligned_alloc(size: usize, alignment: usize) -> *mut u8;
        }

        unsafe {
            aligned_alloc(size, 4096) // 4KB对齐
        }
    }

    /// 释放页面大小的内存
    ///
    /// # Safety
    ///
    /// 调用此函数必须确保 ptr 是通过 `allocate_page_aligned` 或其他兼容方式分配的有效指针。
    pub unsafe fn free_page_aligned(ptr: *mut u8) {
        unsafe extern "C" {
            fn aligned_free(ptr: *mut u8);
        }

        unsafe {
            aligned_free(ptr);
        }
    }
}

/// 数据结构辅助
pub mod data_structures {
    use std::collections::HashMap;

    /// 创建固定大小的HashMap
    pub fn fixed_hashmap<K, V>(capacity: usize) -> HashMap<K, V> {
        HashMap::with_capacity(capacity)
    }

    /// 安全地获取HashMap的值
    pub fn get_or_default<K: Clone + std::hash::Hash + Eq, V: Default + Clone>(
        map: &HashMap<K, V>,
        key: &K,
    ) -> V {
        map.get(key).cloned().unwrap_or_default()
    }

    /// 合并两个HashMap
    pub fn merge_hashmaps<K, V>(mut base: HashMap<K, V>, other: HashMap<K, V>) -> HashMap<K, V>
    where
        K: Clone + std::hash::Hash + Eq,
        V: Clone,
    {
        for (key, value) in other {
            base.insert(key, value);
        }
        base
    }
}

/// 时间辅助
pub mod time {
    use std::time::{Duration, Instant};

    /// 获取当前时间戳（毫秒）
    pub fn timestamp_ms() -> u64 {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }

    /// 获取当前时间戳（微秒）
    pub fn timestamp_us() -> u64 {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_micros() as u64
    }

    /// 计算经过的时间（毫秒）
    pub fn elapsed_ms(start: Instant) -> u64 {
        start.elapsed().as_millis() as u64
    }

    /// 计算经过的时间（微秒）
    pub fn elapsed_us(start: Instant) -> u64 {
        start.elapsed().as_micros() as u64
    }

    /// 转换Duration为毫秒
    pub fn duration_to_ms(d: Duration) -> u64 {
        d.as_millis() as u64
    }

    /// 转换Duration为微秒
    pub fn duration_to_us(d: Duration) -> u64 {
        d.as_micros() as u64
    }
}

/// 字符串辅助
pub mod str_ops {
    /// 安全地截断字符串
    pub fn truncate(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            s[..max_len].to_string()
        }
    }

    /// 填充字符串到指定长度
    pub fn pad_left(s: &str, ch: char, len: usize) -> String {
        if s.len() >= len {
            s.to_string()
        } else {
            format!("{}{}", &ch.to_string().repeat(len - s.len()), s)
        }
    }

    /// 填充字符串到指定长度
    pub fn pad_right(s: &str, ch: char, len: usize) -> String {
        if s.len() >= len {
            s.to_string()
        } else {
            format!("{}{}", s, &ch.to_string().repeat(len - s.len()))
        }
    }

    /// 将字符串转为蛇形命名（snake_case）
    pub fn to_snake_case(s: &str) -> String {
        s.chars()
            .enumerate()
            .map(|(i, c)| {
                if c.is_uppercase() && i > 0 {
                    format!("_{}", c.to_lowercase())
                } else {
                    c.to_lowercase().to_string()
                }
            })
            .collect()
    }

    /// 将字符串转为驼峰命名（camelCase）
    pub fn to_camel_case(s: &str) -> String {
        s.split('_')
            .enumerate()
            .map(|(i, word)| {
                if i == 0 {
                    word.to_lowercase()
                } else {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(first) => {
                            format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase())
                        }
                        None => word.to_lowercase(),
                    }
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_ops() {
        assert!(bit_ops::is_set(0b1000, 3));
        assert_eq!(bit_ops::set_bit(0b1000, 2), 0b1100);
        assert_eq!(bit_ops::clear_bit(0b1100, 2), 0b1000);
    }

    #[test]
    fn test_extract_bits() {
        let val = 0b1100_0000;
        assert_eq!(bit_ops::extract_bits(val, 4, 8), 0b1100); // Extract bits 4-7: should be 12
    }

    #[test]
    fn test_align() {
        assert_eq!(mem_ops::align_up(0x1001, 4096), 0x2000);
        assert_eq!(mem_ops::align_down(0x1FFF, 4096), 0x1000);
        assert!(mem_ops::is_aligned(0x2000, 4096));
        assert!(!mem_ops::is_aligned(0x2001, 4096));
    }

    #[test]
    fn test_timestamp() {
        let ts = time::timestamp_ms();
        assert!(ts > 0);
    }

    #[test]
    fn test_elapsed() {
        let start = std::time::Instant::now();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = time::elapsed_ms(start);
        assert!(elapsed >= 10);
    }

    #[test]
    fn test_str_truncate() {
        assert_eq!(str_ops::truncate("hello", 3), "hel");
        assert_eq!(str_ops::truncate("hi", 10), "hi");
    }

    #[test]
    fn test_str_pad() {
        assert_eq!(str_ops::pad_left("test", ' ', 8), "    test");
        assert_eq!(str_ops::pad_right("test", ' ', 8), "test    ");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(str_ops::to_snake_case("HelloWorld"), "hello_world");
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(str_ops::to_camel_case("hello_world"), "helloWorld");
    }
}
