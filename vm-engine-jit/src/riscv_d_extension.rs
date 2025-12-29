//! RISC-V D扩展（双精度浮点）指令特征
//!
//! 包含RISC-V D扩展（双精度浮点）的指令特征数据：
//! - 双精度浮点加载
//! - 双精度浮点存储
//! - 双精度浮点算术运算
//! - 双精度浮点比较
//! - 双精度浮点转换

use super::InstructionFeatures;
use super::ExecutionUnit;

/// 添加D扩展（双精度浮点）指令特征
pub fn add_d_extension_features(features: &mut super::HashMap<String, InstructionFeatures>) {
    // FL.D - 双精度浮点加载
    features.insert("fl.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU, ExecutionUnit::LoadStore],
    });

    // FLD - 双精度浮点加载
    features.insert("fld".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU, ExecutionUnit::LoadStore],
    });

    // FSD - 双精度浮点存储
    features.insert("fsd".to_string(), InstructionFeatures {
        latency: 4,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU, ExecutionUnit::LoadStore],
    });

    // FADD.D - 双精度浮点加
    features.insert("fadd.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FSUB.D - 双精度浮点减
    features.insert("fsub.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FMUL.D - 双精度浮点乘
    features.insert("fmul.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FDIV.D - 双精度浮点除
    features.insert("fdiv.d".to_string(), InstructionFeatures {
        latency: 32,
        throughput: 32,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FSQRT.D - 双精度浮点平方根
    features.insert("fsqrt.d".to_string(), InstructionFeatures {
        latency: 32,
        throughput: 32,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FMAX.D - 双精度浮点最大值
    features.insert("fmax.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FMIN.D - 双精度浮点最小值
    features.insert("fmin.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FCVT.S.D - 单精度转双精度
    features.insert("fcvt.s.d".to_string(), InstructionFeatures {
        latency: 4,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FCVT.D.S - 双精度转单精度
    features.insert("fcvt.d.s".to_string(), InstructionFeatures {
        latency: 4,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FCVT.D.W - 整数转双精度
    features.insert("fcvt.d.w".to_string(), InstructionFeatures {
        latency: 4,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FCVT.W.D - 双精度转整数
    features.insert("fcvt.w.d".to_string(), InstructionFeatures {
        latency: 4,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FEQ.D - 双精度浮点相等
    features.insert("feq.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::Branch],
    });

    // FLT.D - 双精度浮点小于
    features.insert("flt.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::Branch],
    });

    // FLE.D - 双精度浮点小于等于
    features.insert("fle.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::Branch],
    });

    // FGT.D - 双精度浮点大于
    features.insert("fgt.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::Branch],
    });

    // FGE.D - 双精度浮点大于等于
    features.insert("fge.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::Branch],
    });

    // FCLASS.D - 浮点分类
    features.insert("fclass.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FSGNJ.D - 符号注入（join）
    features.insert("fsgnj.d".to_string(), InstructionFeatures {
        latency: 4,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FSGNJN.D - 符号注入（negated join）
    features.insert("fsgnjn.d".to_string(), InstructionFeatures {
        latency: 4,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FSGNJX.D - 符号注入（xor）
    features.insert("fsgnjx.d".to_string(), InstructionFeatures {
        latency: 4,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FMADD.D - 融合乘加（双精度）
    features.insert("fmadd.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FMSUB.D - 融合乘减（双精度）
    features.insert("fmsub.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FNMSUB.D - 融合负乘减（双精度）
    features.insert("fnmsub.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });

    // FNMADD.D - 融合负乘加（双精度）
    features.insert("fnmadd.d".to_string(), InstructionFeatures {
        latency: 6,
        throughput: 1,
        size: 8,
        is_micro_op: false,
        dependencies: Vec::new(),
        execution_units: vec![ExecutionUnit::FPU],
    });
}

