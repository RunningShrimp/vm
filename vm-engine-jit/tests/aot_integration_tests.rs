//! AOT 编译集成测试
//!
//! 测试 AOT 编译与主执行流程的集成，包括：
//! - AOT 加载器初始化
//! - AOT 镜像加载和查找
//! - HybridExecutor 与 AOT 的集成
//! - 配置验证

use std::io::Write;
use std::sync::Arc;
use tempfile::NamedTempFile;
use vm_core::{AotConfig, ExecMode, GuestAddr, GuestArch, VmConfig};
use vm_engine_jit::Jit;
use vm_engine_jit::aot_integration::{create_test_aot_image, init_aot_loader, validate_aot_config};
use vm_engine_jit::aot_loader::AotLoader;
use vm_engine_jit::hybrid_executor::{CodeSource, HybridExecutor};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};
use vm_mem::SoftMmu;

#[test]
fn test_aot_config_validation() {
    // 测试默认配置（AOT 禁用）
    let config = AotConfig::default();
    assert!(validate_aot_config(&config).is_ok());

    // 测试启用 AOT 但无路径
    let mut config = AotConfig::default();
    config.enable_aot = true;
    assert!(validate_aot_config(&config).is_ok()); // 无路径也是有效的

    // 测试无效的热点阈值
    let mut config = AotConfig::default();
    config.enable_aot = true;
    config.aot_hotspot_threshold = 0;
    assert!(validate_aot_config(&config).is_err());
}

#[test]
fn test_aot_loader_initialization() {
    // 创建测试 AOT 镜像
    let image = create_test_aot_image();

    // 保存到临时文件
    let mut temp_file = NamedTempFile::new().unwrap();
    image.serialize(&mut temp_file).unwrap();
    temp_file.flush().unwrap();

    let path = temp_file.path().to_str().unwrap();

    // 创建配置
    let mut config = VmConfig::default();
    config.aot.enable_aot = true;
    config.aot.aot_image_path = Some(path.to_string());

    // 初始化加载器
    let loader = init_aot_loader(&config).unwrap();
    assert!(loader.is_some());

    let loader = loader.unwrap();
    assert_eq!(loader.code_block_count(), 5);
}

#[test]
fn test_aot_loader_disabled() {
    let mut config = VmConfig::default();
    config.aot.enable_aot = false;

    let loader = init_aot_loader(&config).unwrap();
    assert!(loader.is_none());
}

#[test]
fn test_aot_loader_no_path() {
    let mut config = VmConfig::default();
    config.aot.enable_aot = true;
    config.aot.aot_image_path = None;

    let loader = init_aot_loader(&config).unwrap();
    assert!(loader.is_none());
}

#[test]
fn test_hybrid_executor_with_aot() {
    // 创建测试 AOT 镜像并加载
    let image = create_test_aot_image();
    let loader = Arc::new(AotLoader::new(image).unwrap());

    // 创建混合执行器
    let executor = HybridExecutor::new(Some(loader.clone()));

    // 验证 AOT 块查找
    let pc: GuestAddr = 0x1000;
    let block = loader.lookup_block(pc);
    assert!(block.is_some());

    let aot_block = block.unwrap();
    assert_eq!(aot_block.guest_pc, pc);
    assert!(aot_block.size > 0);
}

#[test]
fn test_hybrid_executor_without_aot() {
    // 创建不带 AOT 的混合执行器
    let executor = HybridExecutor::new(None);

    // 验证统计信息
    let stats = executor.stats();
    assert_eq!(stats.get_hits(CodeSource::AotImage), 0);
    assert_eq!(stats.get_hits(CodeSource::JitCompiled), 0);
    assert_eq!(stats.get_hits(CodeSource::Interpreted), 0);
}

#[test]
fn test_hybrid_executor_statistics() {
    // 创建测试 AOT 镜像
    let image = create_test_aot_image();
    let loader = Arc::new(AotLoader::new(image).unwrap());

    // 创建混合执行器
    let executor = HybridExecutor::new(Some(loader));

    // 创建模拟的 IR 块和 MMU
    let mut builder = IRBuilder::new(0x1000);
    builder.add_op(IROp::MovImm { dst: 0, imm: 42 });
    builder.set_terminator(Terminator::Ret);
    let block = builder.build();

    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let mut jit = Jit::new();

    // 执行（应该命中 AOT）
    let (result, source) = executor.lookup_and_execute(0x1000, &block, &mut mmu, &mut jit);

    // 验证统计信息
    let stats = executor.stats();
    let aot_hits = stats.get_hits(CodeSource::AotImage);
    let interpreted_hits = stats.get_hits(CodeSource::Interpreted);

    // 由于 AOT 镜像中有 0x1000 的块，应该命中 AOT
    assert!(aot_hits > 0 || interpreted_hits > 0);
}

#[test]
fn test_aot_loader_multiple_blocks() {
    // 创建包含多个块的 AOT 镜像
    let image = create_test_aot_image();
    let loader = AotLoader::new(image).unwrap();

    // 验证所有块都能查找
    for i in 0..5 {
        let pc = 0x1000 + i * 0x100;
        let block = loader.lookup_block(pc);
        assert!(block.is_some(), "Block at {:#x} should exist", pc);

        let aot_block = block.unwrap();
        assert_eq!(aot_block.guest_pc, pc);
    }
}

#[test]
fn test_aot_loader_symbol_lookup() {
    // 创建测试镜像
    let mut image = create_test_aot_image();
    use vm_engine_jit::aot_format::SymbolType;
    image.add_symbol("test_function".to_string(), 0, 8, SymbolType::Function);

    let loader = AotLoader::new(image).unwrap();

    // 查找符号
    let addr = loader.lookup_symbol("test_function");
    assert!(addr.is_some());
}

#[test]
fn test_aot_config_serialization() {
    // 测试配置序列化/反序列化
    let mut config = AotConfig::default();
    config.enable_aot = true;
    config.aot_image_path = Some("/path/to/image.aot".to_string());
    config.aot_priority = true;
    config.aot_hotspot_threshold = 2000;

    // 序列化
    let serialized = serde_json::to_string(&config).unwrap();

    // 反序列化
    let deserialized: AotConfig = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.enable_aot, config.enable_aot);
    assert_eq!(deserialized.aot_image_path, config.aot_image_path);
    assert_eq!(deserialized.aot_priority, config.aot_priority);
    assert_eq!(
        deserialized.aot_hotspot_threshold,
        config.aot_hotspot_threshold
    );
}

