//! CPU特性检测和使用模块
//!
//! 提供ARM64 CPU特性的检测、报告和使用功能

use crate::extended_insns::{CpuFeatures, has_sve, has_crypto, get_cpu_features};

/// 获取CPU特性摘要报告
pub fn get_cpu_feature_report() -> String {
    let features = get_cpu_features();
    
    let mut report = String::from("ARM64 CPU Features:\n");
    
    if features.has_neon {
        report.push_str("  NEON: Advanced SIMD\n");
    }
    
    if features.has_sve {
        report.push_str("  SVE: Scalable Vector Extension\n");
    }
    
    if features.has_crypto {
        report.push_str("  Crypto: Hardware cryptographic acceleration\n");
    }
    
    report
}

/// 示例：使用NEON指令进行向量计算
pub fn neon_vector_add_example(a: &[i32], b: &[i32]) -> Vec<i32> {
    // 确保CPU支持NEON
    if !get_cpu_features().has_neon {
        panic!("NEON not supported on this CPU");
    }
    
    // 简单的向量加法示例
    // 在实际实现中，这里会使用NEON指令进行并行计算
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

/// 示例：使用SVE进行可变长度向量操作
pub fn sve_vector_operation_example(data: &[f32], multiplier: f32) -> Vec<f32> {
    // 确保CPU支持SVE
    if !has_sve() {
        // 回退到NEON实现
        return neon_vector_add_example(
            &data.iter().map(|&x| x as i32).collect::<Vec<_>>(),
            &vec![multiplier as i32; data.len()]
        ).iter().map(|&x| x as f32).collect();
    }
    
    // SVE允许根据硬件能力动态调整向量长度
    // 这里只是一个简化示例
    data.iter().map(|&x| x * multiplier).collect()
}

/// 示例：使用加密加速进行数据处理
pub fn crypto_acceleration_example(data: &[u8]) -> Vec<u8> {
    // 确保CPU支持硬件加密
    if !has_crypto() {
        // 回退到软件实现
        return data.iter().map(|&x| x.wrapping_add(1)).collect();
    }
    
    // 使用硬件加密加速
    // 这里只是一个简化示例，实际会调用AES或其他加密指令
    data.iter().map(|&x| x.wrapping_add(1)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cpu_feature_detection() {
        let features = get_cpu_features();
        println!("CPU Features: {:?}", features);
        
        // 测试特性报告
        let report = get_cpu_feature_report();
        println!("{}", report);
        
        // 验证至少有一些特性
        assert!(features.has_neon || features.has_crypto || features.has_sve);
    }
    
    #[test]
    fn test_neon_example() {
        let a = vec![1, 2, 3, 4];
        let b = vec![5, 6, 7, 8];
        let result = neon_vector_add_example(&a, &b);
        assert_eq!(result, vec![6, 8, 10, 12]);
    }
    
    #[test]
    fn test_sve_example() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let result = sve_vector_operation_example(&data, 2.0);
        assert_eq!(result, vec![2.0, 4.0, 6.0, 8.0]);
    }
    
    #[test]
    fn test_crypto_example() {
        let data = vec![1, 2, 3, 4];
        let result = crypto_acceleration_example(&data);
        assert_eq!(result, vec![2, 3, 4, 5]);
    }
}