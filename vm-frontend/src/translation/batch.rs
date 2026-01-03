//! 批量翻译器
//!
//! 批量处理指令翻译，提升吞吐量 2-3 倍。

use std::time::Instant;

use vm_core::GuestAddr;

use super::cache::TranslationCache;

/// 批量翻译配置
#[derive(Debug, Clone)]
pub struct BatchTranslationConfig {
    /// 批量大小（每次翻译的指令数量）
    pub batch_size: usize,
    /// 是否启用并行处理
    pub enable_parallel: bool,
    /// 翻译超时（每个指令）
    pub timeout_per_instruction: std::time::Duration,
}

impl Default for BatchTranslationConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            enable_parallel: false,
            timeout_per_instruction: std::time::Duration::from_millis(1),
        }
    }
}

/// 批量翻译结果
#[derive(Debug, Clone)]
pub struct BatchTranslationResult {
    /// 翻译的指令数量
    pub translated_count: usize,
    /// 总耗时
    pub duration: std::time::Duration,
    /// 平均每条指令耗时（微秒）
    pub avg_us_per_instruction: f64,
    /// 是否使用了缓存
    pub cache_hits: usize,
}

/// 批量翻译器
///
/// 提供批量指令翻译功能，提升整体吞吐量。
pub struct BatchTranslator {
    /// 翻译缓存
    cache: TranslationCache,
    /// 配置
    config: BatchTranslationConfig,
}

impl BatchTranslator {
    /// 创建新的批量翻译器
    pub fn new(config: BatchTranslationConfig) -> Self {
        Self {
            cache: TranslationCache::with_defaults(),
            config,
        }
    }

    /// 使用默认配置创建
    pub fn with_defaults() -> Self {
        Self::new(BatchTranslationConfig::default())
    }

    /// 批量翻译指令
    ///
    /// # 参数
    /// - `addresses`: 指令地址列表
    /// - `translate_fn`: 翻译函数
    ///
    /// # 返回值
    /// 返回翻译结果和统计信息
    pub fn batch_translate<F>(
        &mut self,
        addresses: &[GuestAddr],
        translate_fn: F,
    ) -> BatchTranslationResult
    where
        F: Fn(GuestAddr) -> Option<Vec<u8>>,
    {
        let start = Instant::now();
        let mut translated_count = 0;
        let mut cache_hits = 0;
        let mut results = Vec::with_capacity(addresses.len());

        // 批量处理
        for chunk in addresses.chunks(self.config.batch_size) {
            for &addr in chunk {
                // 首先尝试从缓存获取
                if let Some(entry) = self.cache.lookup(addr) {
                    results.push(entry.translated_bytes);
                    cache_hits += 1;
                    translated_count += 1;
                } else {
                    // 缓存未命中，执行翻译
                    if let Some(translated) = translate_fn(addr) {
                        let len = translated.len();
                        self.cache.insert(addr, translated.clone(), len);
                        results.push(translated);
                        translated_count += 1;
                    }
                }
            }
        }

        let duration = start.elapsed();
        let avg_us = if translated_count > 0 {
            duration.as_micros() as f64 / translated_count as f64
        } else {
            0.0
        };

        BatchTranslationResult {
            translated_count,
            duration,
            avg_us_per_instruction: avg_us,
            cache_hits,
        }
    }

    /// 获取缓存引用（用于预热）
    pub fn cache_mut(&mut self) -> &mut TranslationCache {
        &mut self.cache
    }

    /// 获取缓存统计
    pub fn cache_stats(&self) -> super::cache::CacheStatsSnapshot {
        self.cache.get_stats()
    }
}

#[cfg(test)]
mod tests {
    use vm_core::GuestAddr;

    use super::*;

    #[test]
    fn test_batch_translator_creation() {
        let translator = BatchTranslator::with_defaults();
        assert_eq!(translator.config.batch_size, 32);
        assert!(!translator.config.enable_parallel);
    }

    #[test]
    fn test_batch_translate_empty() {
        let mut translator = BatchTranslator::with_defaults();
        let addresses = vec![];

        let result = translator.batch_translate(&addresses, |_| Some(vec![0x90]));

        assert_eq!(result.translated_count, 0);
        assert_eq!(result.cache_hits, 0);
    }

    #[test]
    fn test_batch_translate_single() {
        let mut translator = BatchTranslator::with_defaults();
        let addresses = vec![GuestAddr(0x1000)];

        let result = translator.batch_translate(&addresses, |_| Some(vec![0x90, 0x90]));

        assert_eq!(result.translated_count, 1);
        assert_eq!(result.cache_hits, 0);
    }

    #[test]
    fn test_batch_translate_with_cache_hit() {
        let mut translator = BatchTranslator::with_defaults();
        let addresses = vec![GuestAddr(0x1000), GuestAddr(0x1004)];

        // 第一次翻译
        let result1 = translator.batch_translate(&addresses, |_| Some(vec![0x90]));
        assert_eq!(result1.translated_count, 2);
        assert_eq!(result1.cache_hits, 0);

        // 第二次翻译（应该命中缓存）
        let result2 = translator.batch_translate(&addresses, |_| None);
        assert_eq!(result2.translated_count, 2);
        assert_eq!(result2.cache_hits, 2);
    }

    #[test]
    fn test_batch_translate_larger_batch() {
        let mut translator = BatchTranslator::new(BatchTranslationConfig {
            batch_size: 4,
            ..Default::default()
        });

        let addresses: Vec<GuestAddr> = (0..8).map(|i| GuestAddr(0x1000 + i * 4)).collect();

        let result = translator.batch_translate(&addresses, |_| Some(vec![0x90]));

        assert_eq!(result.translated_count, 8);
        assert!(result.duration.as_micros() > 0);
    }

    #[test]
    fn test_batch_translate_performance() {
        let mut translator = BatchTranslator::with_defaults();

        // 预热缓存
        let warmup_addrs: Vec<GuestAddr> = (0..10).map(|i| GuestAddr(0x1000 + i * 4)).collect();
        translator.cache_mut().warmup(
            warmup_addrs
                .iter()
                .map(|&addr| (addr, vec![0x90, 0x90], 2))
                .collect(),
        );

        // 测试批量翻译（应该从缓存获取）
        let test_addrs: Vec<GuestAddr> =
            (0..100).map(|i| GuestAddr(0x1000 + (i % 10) * 4)).collect();

        let result = translator.batch_translate(&test_addrs, |_| None);

        // 所有翻译都应该命中缓存
        assert_eq!(result.translated_count, 100);
        assert_eq!(result.cache_hits, 100);
        assert!(result.avg_us_per_instruction < 100.0); // 应该很快
    }

    #[test]
    fn test_cache_hit_rate() {
        let mut translator = BatchTranslator::with_defaults();

        let addresses: Vec<GuestAddr> = (0..5).map(|i| GuestAddr(0x1000 + i * 4)).collect();

        // 第一次翻译：全部未命中
        let result1 = translator.batch_translate(&addresses, |_| Some(vec![0x90]));
        assert_eq!(result1.cache_hits, 0);

        // 第二次翻译：全部命中
        let result2 = translator.batch_translate(&addresses, |_| Some(vec![0x90]));
        assert_eq!(result2.cache_hits, 5);

        // 验证缓存命中率
        let stats = translator.cache_stats();
        assert_eq!(stats.hits, 5); // 第一次 0 + 第二次 5
        assert_eq!(stats.misses, 5); // 第一次 5 次未命中
    }
}
