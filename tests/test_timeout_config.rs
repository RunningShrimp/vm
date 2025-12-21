//! 测试超时配置
//!
//! 定义不同测试类型的默认超时时间

/// 测试超时配置
pub struct TestTimeoutConfig;

impl TestTimeoutConfig {
    /// 单元测试默认超时（30秒）
    pub const UNIT_TEST: u64 = 30;

    /// 集成测试默认超时（60秒）
    pub const INTEGRATION_TEST: u64 = 60;

    /// 性能测试默认超时（120秒）
    pub const PERFORMANCE_TEST: u64 = 120;

    /// 并发测试默认超时（180秒）
    pub const CONCURRENCY_TEST: u64 = 180;

    /// 压力测试默认超时（300秒，5分钟）
    pub const STRESS_TEST: u64 = 300;

    /// 端到端测试默认超时（600秒，10分钟）
    pub const E2E_TEST: u64 = 600;

    /// 根据测试名称推断超时时间
    pub fn infer_timeout(test_name: &str) -> u64 {
        let name_lower = test_name.to_lowercase();
        
        if name_lower.contains("stress") || name_lower.contains("load") {
            Self::STRESS_TEST
        } else if name_lower.contains("e2e") || name_lower.contains("end_to_end") {
            Self::E2E_TEST
        } else if name_lower.contains("concurrent") || name_lower.contains("parallel") || name_lower.contains("race") {
            Self::CONCURRENCY_TEST
        } else if name_lower.contains("performance") || name_lower.contains("benchmark") || name_lower.contains("perf") {
            Self::PERFORMANCE_TEST
        } else if name_lower.contains("integration") {
            Self::INTEGRATION_TEST
        } else {
            Self::UNIT_TEST
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_inference() {
        assert_eq!(TestTimeoutConfig::infer_timeout("test_stress_load"), TestTimeoutConfig::STRESS_TEST);
        assert_eq!(TestTimeoutConfig::infer_timeout("test_e2e_scenario"), TestTimeoutConfig::E2E_TEST);
        assert_eq!(TestTimeoutConfig::infer_timeout("test_concurrent_access"), TestTimeoutConfig::CONCURRENCY_TEST);
        assert_eq!(TestTimeoutConfig::infer_timeout("test_performance_benchmark"), TestTimeoutConfig::PERFORMANCE_TEST);
        assert_eq!(TestTimeoutConfig::infer_timeout("test_integration"), TestTimeoutConfig::INTEGRATION_TEST);
        assert_eq!(TestTimeoutConfig::infer_timeout("test_simple"), TestTimeoutConfig::UNIT_TEST);
    }
}

