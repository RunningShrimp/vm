//! ARM64 API模块
//!
//! 提供ARM64架构的API和工具函数

use super::Cond;

/// 将26位立即数限制到指定范围内
pub fn clamp_imm26_bytes(imm: i64) -> i32 {
    let mut v = imm >> 2;
    let min = -(1 << 25);
    let max = (1 << 25) - 1;
    if v < min {
        v = min;
    }
    if v > max {
        v = max;
    }
    v as i32
}

/// 生成字节码指令
pub fn encode_b(imm_bytes: i64) -> u32 {
    // 使用ARM64字节码编码
    // 这里只是一个简化示例
    (imm_bytes & 0xFF) as u32
}

/// 检查指令的内存访问模式
pub fn analyze_memory_access(insn: u32) -> (bool, bool, bool) {
    // 分析指令是否进行读、写或原子操作
    let has_load = (insn & 0x3B000000) != 0;
    let has_store = (insn & 0x3B200000) != 0;
    let has_atomic = (insn & 0x3F200000) != 0;
    
    (has_load, has_store, has_atomic)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_clamp_imm26() {
        assert_eq!(clamp_imm26_bytes(127), 127);
        assert_eq!(clamp_imm26_bytes(128), 127); // 被限制到最大值
        assert_eq!(clamp_imm26_bytes(-129), -128); // 被限制到最小值
        assert_eq!(clamp_imm26_bytes(2 << 24), 127); // 被限制到最大值
        assert_eq!(clamp_imm26_bytes(-(2 << 24)), -128); // 被限制到最小值
    }
    
    #[test]
    fn test_encode_b() {
        assert_eq!(encode_b(0x123456), 0x56);
        assert_eq!(encode_b(0x1234789), 0x89);
    }
    
    #[test]
    fn test_analyze_memory_access() {
        // 测试加载指令 (e.g., LDR X1, [X1])
        let (load, store, atomic) = analyze_memory_access(0b01000000);
        assert!(load && !store && !atomic);
        
        // 测试存储指令 (e.g., STR X1, [X1])
        let (load, store, atomic) = analyze_memory_access(0b00000000);
        assert!(!load && store && !atomic);
        
        // 测试原子指令 (e.g., LDAR X1, [X1])
        let (load, store, atomic) = analyze_memory_access(0b00200000);
        assert!(!load && !store && atomic);
    }
}
