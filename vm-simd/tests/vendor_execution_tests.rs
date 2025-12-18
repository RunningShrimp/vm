//! 厂商扩展执行引擎测试

use vm_simd::apple_amx::{AmxExecutor, AmxState};

#[test]
fn test_amx_tile_config() {
    let mut state = AmxState::new();
    state.configure_tile(0, 4, 4, 1);

    let tile = state.get_tile(0).unwrap();
    assert_eq!(tile.rows(), 4);
    assert_eq!(tile.cols(), 4);
    assert_eq!(tile.element_size(), 1);
}

#[test]
fn test_amx_mul_int8() {
    let mut executor = AmxExecutor::new();

    executor.configure_tile(0, 2, 2, 1);
    executor.configure_tile(1, 2, 2, 1);
    executor.configure_tile(2, 2, 2, 1);

    let result = executor.execute_mul(2, 0, 1, "Int8");
    assert!(result.is_ok());
}

#[test]
fn test_amx_add_fp32() {
    let mut executor = AmxExecutor::new();

    executor.configure_tile(0, 2, 2, 4);
    executor.configure_tile(1, 2, 2, 4);
    executor.configure_tile(2, 2, 2, 4);

    let result = executor.execute_add(2, 0, 1, "Fp32");
    assert!(result.is_ok());
}
