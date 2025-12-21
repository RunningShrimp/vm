//! SMMU 模块测试

#[cfg(test)]
mod tests {
    use super::super::smmu::*;
    use vm_core::{GuestAddr, HostAddr};

    #[test]
    fn test_smmu_virtualizer_creation() {
        let config = SmmuConfig::default();
        let smmu = SmmuVirtualizer::new(config);
        assert!(!smmu.is_enabled());
    }

    #[test]
    fn test_smmu_enable_disable() {
        let config = SmmuConfig::default();
        let mut smmu = SmmuVirtualizer::new(config);
        
        assert!(smmu.enable().is_ok());
        assert!(smmu.is_enabled());
        
        smmu.disable();
        assert!(!smmu.is_enabled());
    }

    #[test]
    fn test_stream_configuration() {
        let config = SmmuConfig::default();
        let mut smmu = SmmuVirtualizer::new(config);
        smmu.enable().unwrap();

        let stream_id = StreamId(1);
        let base_addr = GuestAddr(0x1000);
        
        assert!(smmu.configure_stream(stream_id, base_addr).is_ok());
    }

    #[test]
    fn test_address_translation() {
        let config = SmmuConfig::default();
        let mut smmu = SmmuVirtualizer::new(config);
        smmu.enable().unwrap();

        let stream_id = StreamId(1);
        let base_addr = GuestAddr(0x1000);
        smmu.configure_stream(stream_id, base_addr).unwrap();

        let input_addr = GuestAddr(0x2000);
        let output_addr = HostAddr(0x5000);
        let flags = TranslationFlags::default();

        assert!(smmu.add_translation(stream_id, input_addr, output_addr, 4096, flags).is_ok());

        // 测试转换
        let translated = smmu.translate(stream_id, GuestAddr(0x2000)).unwrap();
        assert_eq!(translated.0, 0x5000);

        // 测试偏移地址
        let translated2 = smmu.translate(stream_id, GuestAddr(0x2000 + 0x100)).unwrap();
        assert_eq!(translated2.0, 0x5000 + 0x100);
    }

    #[test]
    fn test_tlb_flush() {
        let config = SmmuConfig::default();
        let mut smmu = SmmuVirtualizer::new(config);
        smmu.enable().unwrap();

        let stream_id = StreamId(1);
        let base_addr = GuestAddr(0x1000);
        smmu.configure_stream(stream_id, base_addr).unwrap();

        smmu.flush_tlb();
        smmu.flush_tlb_stream(stream_id);
        smmu.flush_tlb_range(stream_id, GuestAddr(0x2000), 4096);
    }

    #[test]
    fn test_multiple_streams() {
        let config = SmmuConfig::default();
        let mut smmu = SmmuVirtualizer::new(config);
        smmu.enable().unwrap();

        let stream1 = StreamId(1);
        let stream2 = StreamId(2);
        
        smmu.configure_stream(stream1, GuestAddr(0x1000)).unwrap();
        smmu.configure_stream(stream2, GuestAddr(0x2000)).unwrap();

        let flags = TranslationFlags::default();
        smmu.add_translation(stream1, GuestAddr(0x1000), HostAddr(0x5000), 4096, flags).unwrap();
        smmu.add_translation(stream2, GuestAddr(0x2000), HostAddr(0x6000), 4096, flags).unwrap();

        // 测试不同 StreamID 的转换
        let trans1 = smmu.translate(stream1, GuestAddr(0x1000)).unwrap();
        assert_eq!(trans1.0, 0x5000);

        let trans2 = smmu.translate(stream2, GuestAddr(0x2000)).unwrap();
        assert_eq!(trans2.0, 0x6000);
    }

    #[test]
    fn test_translation_without_enable() {
        let config = SmmuConfig::default();
        let mut smmu = SmmuVirtualizer::new(config);

        let stream_id = StreamId(1);
        let input_addr = GuestAddr(0x2000);
        let output_addr = HostAddr(0x5000);
        let flags = TranslationFlags::default();

        // 未启用时添加转换应该失败
        assert!(smmu.add_translation(stream_id, input_addr, output_addr, 4096, flags).is_err());

        // 未启用时转换应该返回直通地址
        let translated = smmu.translate(stream_id, input_addr).unwrap();
        assert_eq!(translated.0, input_addr.0);
    }
}

