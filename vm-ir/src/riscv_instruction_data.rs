//! RISC-V指令集扩展数据
//!
//! 定义RISC-V指令集的各种扩展指令的性能数据。
//!
//! 本模块提供了RISC-V扩展指令的特征数据（延迟、吞吐量、大小等），
//! 供JIT编译器进行优化决策。
//!
//! # 数据结构
//!
//! - [`RiscvInstructionData`][]: 指令特征数据结构
//! - [`ExecutionUnitType`][]: 执行单元类型枚举
//!
//! # 初始化函数
//!
//! - [`init_riscv_m_extension_data()`][]: M扩展数据初始化
//! - [`init_riscv_a_extension_data()`][]: A扩展数据初始化
//! - [`init_riscv_f_extension_data()`][]: F扩展数据初始化
//! - [`init_riscv_d_extension_data()`][]: D扩展数据初始化
//! - [`init_riscv_c_extension_data()`][]: C扩展数据初始化
//! - [`init_all_riscv_extension_data()`][]: 所有扩展数据初始化
//!
//! # 使用示例
//!
//! ```ignore
//! use vm_ir::riscv_instruction_data::{
//!     init_all_riscv_extension_data,
//!     RiscvInstructionData,
//!     ExecutionUnitType,
//! };
//!
//! // 在JIT编译器中初始化RISC-V指令数据
//! let mut instruction_data = HashMap::new();
//! init_all_riscv_extension_data(&mut instruction_data);
//!
//! // 获取特定指令的数据
//! if let Some(data) = instruction_data.get("mul") {
//!     println!("MUL: latency={}, throughput={}", data.latency, data.throughput);
//! }
//! ```

use std::collections::HashMap;

/// 执行单元类型
///
/// 描述指令主要在哪个执行单元上执行
///
/// # 变体
///
/// - [`ExecutionUnitType::ALU`][]: 算术逻辑单元
/// - [`ExecutionUnitType::Multiplier`][]: 乘法器
/// - [`ExecutionUnitType::FPU`][]: 浮点单元
/// - [`ExecutionUnitType::Branch`][]: 分支单元
/// - [`ExecutionUnitType::LoadStore`][]: 加载/存储单元
/// - [`ExecutionUnitType::System`][]: 系统单元
/// - [`ExecutionUnitType::Vector`][]: 向量单元（预留）
///
/// 注意：这些值应该与vm-engine-jit/codegen.rs中的ExecutionUnit枚举对应
/// 通常映射为：ALU=0, Multiplier=1, FPU=2, Branch=3, LoadStore=4, System=5, Vector=6
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExecutionUnitType {
    /// ALU（算术逻辑单元）
    ALU = 0,
    /// 乘法器
    Multiplier = 1,
    /// 浮点单元
    FPU = 2,
    /// 分支单元
    Branch = 3,
    /// 加载/存储单元
    LoadStore = 4,
    /// 系统单元（CSR操作等）
    System = 5,
    /// 向量单元（预留）
    Vector = 6,
}

/// RISC-V指令特征数据
///
/// 包含RISC-V指令的性能特征数据，用于JIT编译器进行优化决策。
///
/// # 字段说明
///
/// - `latency`: 指令延迟（周期），指令从发射到完成的周期数
/// - `throughput`: 指令吞吐量（每周期可执行次数），表示流水线能力
/// - `size`: 指令大小（字节），编码后的指令长度
/// - `execution_unit`: 执行单元类型（[`ExecutionUnitType`][]枚举值）
/// - `has_side_effects`: 是否有副作用（内存写入、CSR写等），影响重排序
/// - `can_reorder`: 是否可以与前后指令重排序，影响指令调度
///
/// # 使用示例
///
/// ```ignore
/// let data = RiscvInstructionData {
///     latency: 4,
///     throughput: 1,
///     size: 4,
///     execution_unit: ExecutionUnitType::Multiplier,
///     has_side_effects: false,
///     can_reorder: true,
/// };
/// ```
#[derive(Clone, Debug)]
pub struct RiscvInstructionData {
    /// 指令延迟（周期）
    pub latency: u8,
    /// 指令吞吐量（每周期可执行次数）
    pub throughput: u8,
    /// 指令大小（字节）
    pub size: u8,
    /// 执行单元类型
    pub execution_unit: ExecutionUnitType,
    /// 是否有副作用（内存写入、CSR写等）
    pub has_side_effects: bool,
    /// 是否可以与前后指令重排序
    pub can_reorder: bool,
}

/// 初始化RISC-V M扩展指令数据
///
/// M扩展包含乘法指令
///
/// # 指令列表
///
/// **乘法指令**（8个）：
/// - `mul` - 乘法
/// - `mulh` - 乘法高位
/// - `mulhsu` - 乘法高位有符号
/// - `mulhu` - 乘法高位无符号
/// - `mulw` - 乘法字
/// - `mulhw` - 乘法字高位
/// - `mulhsuw` - 乘法字高位有符号
/// - `mulhuw` - 乘法字高位无符号
///
/// **除法指令**（8个）：
/// - `div` - 除法
/// - `divu` - 无符号除法
/// - `rem` - 余数
/// - `remu` - 无符号余数
/// - `divw` - 除法字
/// - `divuw` - 无符号除法字
/// - `remw` - 余数字
/// - `remuw` - 无符号余数字
pub fn init_riscv_m_extension_data(data: &mut HashMap<String, RiscvInstructionData>) {
    // 乘法指令（延迟4周期，吞吐量1）
    for mnemonic in [
        "mul", "mulh", "mulhsu", "mulhu", "mulw", "mulhw", "mulhsuw", "mulhuw",
    ] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 4,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::Multiplier,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 除法指令（延迟32周期，吞吐量8）
    for mnemonic in [
        "div", "divu", "rem", "remu", "divw", "divuw", "remw", "remuw",
    ] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 32,
                throughput: 8,
                size: 4,
                execution_unit: ExecutionUnitType::ALU,
                has_side_effects: false,
                can_reorder: false,
            },
        );
    }
}

/// 初始化RISC-V A扩展指令数据
///
/// A扩展包含原子指令
///
/// # 指令列表
///
/// **加载保留/存储条件指令**（4个）：
/// - `lr.w` - 加载保留字
/// - `lr.d` - 加载保留双字
/// - `sc.w` - 存储条件字
/// - `sc.d` - 存储条件双字
///
/// **原子内存操作指令**（16个）：
/// - `amoswap.w/d` - 原子交换
/// - `amoswap.d` - 原子交换
/// - `amoadd.w/d` - 原子加法
/// - `amoadd.d` - 原子加法
/// - `amoxor.w/d` - 原子异或
/// - `amoxor.d` - 原子异或
/// - `amoand.w/d` - 原子与
/// - `amoand.d` - 原子与
/// - `amoor.w/d` - 原子或
/// - `amoor.d` - 原子或
/// - `amomin.w/d` - 原子最小有符号
/// - `amomin.d` - 原子最小无符号
/// - `amomax.w/d` - 原子最大有符号
/// - `amomax.d` - 原子最大无符号
pub fn init_riscv_a_extension_data(data: &mut HashMap<String, RiscvInstructionData>) {
    // LR/SC指令（延迟8周期，吞吐量4）
    for mnemonic in ["lr.w", "lr.d", "sc.w", "sc.d"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 8,
                throughput: 4,
                size: 4,
                execution_unit: ExecutionUnitType::LoadStore,
                has_side_effects: true,
                can_reorder: false,
            },
        );
    }

    // AMO指令（延迟12周期，吞吐量8）
    for mnemonic in [
        "amoswap.w",
        "amoswap.d",
        "amoadd.w",
        "amoadd.d",
        "amoxor.w",
        "amoxor.d",
        "amoand.w",
        "amoand.d",
        "amoor.w",
        "amoor.d",
        "amomin.w",
        "amomin.d",
        "amomax.w",
        "amomax.d",
    ] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 12,
                throughput: 8,
                size: 4,
                execution_unit: ExecutionUnitType::LoadStore,
                has_side_effects: true,
                can_reorder: false,
            },
        );
    }
}

/// 初始化RISC-V F扩展指令数据
///
/// F扩展包含单精度浮点指令
///
/// # 指令列表
///
/// **浮点算术指令**（8个）：
/// - `fadd.s` - 浮点加法
/// - `fsub.s` - 浮点减法
/// - `fmul.s` - 浮点乘法
/// - `fdiv.s` - 浮点除法
/// - `fsqrt.s` - 浮点平方根
/// - `fmin.s` - 浮点最小值
/// - `fmax.s` - 浮点最大值
///
/// **融合乘加指令**（4个）：
/// - `fmadd.s` - 融合乘加
/// - `fmsub.s` - 融合乘减
/// - `fnmsub.s` - 融合负乘加
/// - `fnmsub.s` - 融合负乘加
///
/// **浮点比较指令**（4个）：
/// - `feq.s` - 浮点相等
/// - `flt.s` - 浮点小于
/// - `fle.s` - 浮点小于等于
///
/// **浮点转换指令**（12个）：
/// - `fcvt.w.s` - 浮点转整数
/// - `fcvt.wu.s` - 浮点转无符号整数
/// - `fcvt.l.s` - 浮点转长整数
/// - `fcvt.lu.s` - 浮点转无符号长整数
/// - `fcvt.s.w` - 整数转浮点
/// - `fcvt.s.wu` - 无符号整数转浮点
/// - `fcvt.s.l` - 长整数转浮点
/// - `fcvt.s.lu` - 无符号长整数转浮点
///
/// **浮点符号操作**（4个）：
/// - `fsgnj.s` - 浮点符号合并
/// - `fsgnjn.s` - 浮点符号合并取反
///
/// **浮点分类指令**（1个）：
/// - `fclass.s` - 浮点分类
///
/// **浮点移动指令**（2个）：
/// - `fmv.x.w` - 浮点转整数移动
/// - `fmv.w.x` - 整数转浮点移动
///
/// **浮点绝对值/负值指令**（2个）：
/// - `fabs.s` - 浮点绝对值
/// - `fneg.s` - 浮点取反
pub fn init_riscv_f_extension_data(data: &mut HashMap<String, RiscvInstructionData>) {
    // 浮点算术指令（延迟4-5周期，吞吐量1）
    for mnemonic in ["fadd.s", "fsub.s", "fmul.s", "fmin.s", "fmax.s"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 4,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 浮点除法/平方根（延迟12周期，吞吐量8）
    for mnemonic in ["fdiv.s", "fsqrt.s"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 12,
                throughput: 8,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: false,
            },
        );
    }

    // 融合乘加指令（延迟5周期，吞吐量1）
    for mnemonic in ["fmadd.s", "fmsub.s", "fnmsub.s"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 5,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 浮点比较指令（延迟4周期，吞吐量1）
    for mnemonic in ["feq.s", "flt.s", "fle.s"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 4,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 浮点转换指令（延迟4周期，吞吐量1）
    for mnemonic in [
        "fcvt.w.s",
        "fcvt.wu.s",
        "fcvt.l.s",
        "fcvt.lu.s",
        "fcvt.s.w",
        "fcvt.s.wu",
        "fcvt.s.l",
        "fcvt.s.lu",
    ] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 4,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 浮点符号操作（延迟2周期，吞吐量1）
    for mnemonic in ["fsgnj.s", "fsgnjn.s"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 2,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 浮点分类指令（延迟4周期，吞吐量1）
    data.insert(
        "fclass.s".to_string(),
        RiscvInstructionData {
            latency: 4,
            throughput: 1,
            size: 4,
            execution_unit: ExecutionUnitType::FPU,
            has_side_effects: false,
            can_reorder: true,
        },
    );

    // 浮点移动指令（延迟2周期，吞吐量1）
    for mnemonic in ["fmv.x.w", "fmv.w.x"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 2,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::ALU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 浮点绝对值/负值指令（延迟2-3周期，吞吐量1）
    data.insert(
        "fabs.s".to_string(),
        RiscvInstructionData {
            latency: 3,
            throughput: 1,
            size: 4,
            execution_unit: ExecutionUnitType::FPU,
            has_side_effects: false,
            can_reorder: true,
        },
    );
    data.insert(
        "fneg.s".to_string(),
        RiscvInstructionData {
            latency: 2,
            throughput: 1,
            size: 4,
            execution_unit: ExecutionUnitType::FPU,
            has_side_effects: false,
            can_reorder: true,
        },
    );
}

/// 初始化RISC-V D扩展指令数据
///
/// D扩展包含双精度浮点指令
///
/// # 指令列表
///
/// **双精度浮点算术指令**（8个）：
/// - `fadd.d` - 双精度加法
/// - `fsub.d` - 双精度减法
/// - `fmul.d` - 双精度乘法
/// - `fdiv.d` - 双精度除法
/// - `fsqrt.d` - 双精度平方根
/// - `fmin.d` - 双精度最小值
/// - `fmax.d` - 双精度最大值
///
/// **融合乘加指令**（4个）：
/// - `fmadd.d` - 融合乘加
/// - `fmsub.d` - 融合乘减
/// - `fnmsub.d` - 融合负乘加
/// - `fnmsub.d` - 融合负乘加
///
/// **双精度浮点比较指令**（4个）：
/// - `feq.d` - 双精度相等
/// - `flt.d` - 双精度小于
/// - `fle.d` - 双精度小于等于
///
/// **双精度浮点转换指令**（12个）：
/// - `fcvt.w.d` - 双精度转整数
/// - `fcvt.wu.d` - 双精度转无符号整数
/// - `fcvt.l.d` - 双精度转长整数
/// - `fcvt.lu.d` - 双精度转无符号长整数
/// - `fcvt.d.w` - 整数转双精度
/// - `fcvt.d.wu` - 无符号整数转双精度
/// - `fcvt.d.l` - 长整数转双精度
/// - `fcvt.d.lu` - 无符号长整数转双精度
/// - `fcvt.s.d` - 单精度转双精度
/// - `fcvt.d.s` - 双精度转单精度
///
/// **双精度浮点符号操作**（4个）：
/// - `fsgnj.d` - 双精度符号合并
/// - `fsgnjn.d` - 双精度符号合并取反
///
/// **双精度浮点分类指令**（1个）：
/// - `fclass.d` - 双精度分类
///
/// **双精度浮点移动指令**（2个）：
/// - `fmv.x.d` - 双精度转整数移动
/// - `fmv.d.x` - 整数转双精度移动
///
/// **双精度浮点绝对值/负值指令**（2个）：
/// - `fabs.d` - 双精度绝对值
/// - `fneg.d` - 双精度取反
pub fn init_riscv_d_extension_data(data: &mut HashMap<String, RiscvInstructionData>) {
    // 双精度浮点算术指令（延迟5周期，吞吐量1）
    for mnemonic in ["fadd.d", "fsub.d", "fmul.d", "fmin.d", "fmax.d"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 5,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 双精度浮点除法/平方根（延迟20周期，吞吐量12）
    for mnemonic in ["fdiv.d", "fsqrt.d"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 20,
                throughput: 12,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: false,
            },
        );
    }

    // 融合乘加指令（延迟6周期，吞吐量1）
    for mnemonic in ["fmadd.d", "fmsub.d", "fnmsub.d"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 6,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 双精度浮点比较指令（延迟5周期，吞吐量1）
    for mnemonic in ["feq.d", "flt.d", "fle.d"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 5,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 双精度浮点转换指令（延迟5周期，吞吐量1）
    for mnemonic in [
        "fcvt.w.d",
        "fcvt.wu.d",
        "fcvt.l.d",
        "fcvt.lu.d",
        "fcvt.d.w",
        "fcvt.d.wu",
        "fcvt.d.l",
        "fcvt.d.lu",
        "fcvt.s.d",
        "fcvt.d.s",
    ] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 5,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 双精度浮点符号操作（延迟2周期，吞吐量1）
    for mnemonic in ["fsgnj.d", "fsgnjn.d"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 2,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::FPU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 双精度浮点分类指令（延迟5周期，吞吐量1）
    data.insert(
        "fclass.d".to_string(),
        RiscvInstructionData {
            latency: 5,
            throughput: 1,
            size: 4,
            execution_unit: ExecutionUnitType::FPU,
            has_side_effects: false,
            can_reorder: true,
        },
    );

    // 双精度浮点移动指令（延迟2周期，吞吐量1）
    for mnemonic in ["fmv.x.d", "fmv.d.x"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 2,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::ALU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 双精度浮点绝对值/负值指令（延迟2-3周期，吞吐量1）
    data.insert(
        "fabs.d".to_string(),
        RiscvInstructionData {
            latency: 3,
            throughput: 1,
            size: 4,
            execution_unit: ExecutionUnitType::FPU,
            has_side_effects: false,
            can_reorder: true,
        },
    );
    data.insert(
        "fneg.d".to_string(),
        RiscvInstructionData {
            latency: 2,
            throughput: 1,
            size: 4,
            execution_unit: ExecutionUnitType::FPU,
            has_side_effects: false,
            can_reorder: true,
        },
    );
}

/// 初始化RISC-V C扩展指令数据
///
/// C扩展包含压缩指令（16位编码）
///
/// # 指令列表
///
/// **算术指令**（16位）**（6个）：
/// - `c.add` - 压缩加法
/// - `c.sub` - 压缩减法
/// - `c.mv` - 压缩移动
/// - `c.and` - 压缩与
/// - `c.or` - 压缩或
/// - `c.xor` - 压缩异或
///
/// **移位指令**（16位）**（7个）：
/// - `c.slli` - 压缩逻辑左移
/// - `c.srli` - 压缩逻辑右移
/// - `c.srai` - 压缩算术右移
/// - `c.andi` - 压缩与立即数
/// - `c.slli` - 压缩移位左立即
/// - `c.sra` - 压缩算术右移
///
/// **加载/存储指令**（16位）**（4个）：
/// - `c.lwsp` - 压缩加载栈指针
/// - `c.swsp` - 压缩存储栈指针
/// - `c.lw` - 压缩加载字
/// - `c.sw` - 压缩存储字
///
/// **分支指令**（16位）**（7个）：
/// - `c.beqz` - 压缩等于跳转
/// - `c.bnez` - 压缩不等于跳转
/// - `c.j` - 压缩跳转
/// - `c.jal` - 压缩跳转并链接
/// - `c.jr` - 压缩寄存器跳转
/// - `c.jalr` - 压缩寄存器跳转并链接
/// - `c.ebreak` - 压缩断点
///
/// **立即数加载指令**（1个）：
/// - `c.li` - 压缩加载立即数
pub fn init_riscv_c_extension_data(data: &mut HashMap<String, RiscvInstructionData>) {
    // 算术指令（延迟1周期，吞吐量1）
    for mnemonic in ["c.add", "c.sub", "c.mv", "c.and", "c.or", "c.xor"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 1,
                throughput: 1,
                size: 2,
                execution_unit: ExecutionUnitType::ALU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 移位指令（延迟1周期，吞吐量1）
    for mnemonic in ["c.slli", "c.srli", "c.srai", "c.andi", "c.slli", "c.sra"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 1,
                throughput: 1,
                size: 2,
                execution_unit: ExecutionUnitType::ALU,
                has_side_effects: false,
                can_reorder: true,
            },
        );
    }

    // 加载/存储指令（延迟2周期，吞吐量1）
    for mnemonic in ["c.lwsp", "c.swsp", "c.lw", "c.sw"] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 2,
                throughput: 1,
                size: 2,
                execution_unit: ExecutionUnitType::LoadStore,
                has_side_effects: true,
                can_reorder: false,
            },
        );
    }

    // 分支指令（延迟2周期，吞吐量1）
    for mnemonic in [
        "c.beqz", "c.bnez", "c.j", "c.jal", "c.jr", "c.jalr", "c.ebreak",
    ] {
        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency: 2,
                throughput: 1,
                size: 2,
                execution_unit: ExecutionUnitType::Branch,
                has_side_effects: true,
                can_reorder: false,
            },
        );
    }

    // 立即数加载指令（延迟1周期，吞吐量1）
    data.insert(
        "c.li".to_string(),
        RiscvInstructionData {
            latency: 1,
            throughput: 1,
            size: 2,
            execution_unit: ExecutionUnitType::ALU,
            has_side_effects: false,
            can_reorder: true,
        },
    );
}

/// 初始化所有RISC-V扩展指令数据
///
/// 调用所有扩展的初始化函数
///
/// # 参数
///
/// - `data`: 用于存储指令数据的HashMap
///
/// # 示例
///
/// ```ignore
/// use vm_ir::riscv_instruction_data::init_all_riscv_extension_data;
///
/// let mut instruction_data = HashMap::new();
/// init_all_riscv_extension_data(&mut instruction_data);
///
/// // 获取特定指令的数据
/// if let Some(data) = instruction_data.get("mul") {
///     println!("MUL: latency={}, throughput={}", data.latency, data.throughput);
/// }
/// ```
pub fn init_all_riscv_extension_data(data: &mut HashMap<String, RiscvInstructionData>) {
    init_riscv_m_extension_data(data);
    init_riscv_a_extension_data(data);
    init_riscv_f_extension_data(data);
    init_riscv_d_extension_data(data);
    init_riscv_c_extension_data(data);
    init_riscv_privileged_extension_data(data);
}

/// 初始化RISC-V特权指令数据
///
/// 特权指令包括：
/// - 系统调用和异常处理
/// - CSR（控制和状态寄存器）访问
/// - 特权返回指令
pub fn init_riscv_privileged_extension_data(data: &mut HashMap<String, RiscvInstructionData>) {
    // 系统调用和异常处理指令（延迟10-30周期）
    for mnemonic in [
        // 系统调用
        "ecall", "ebreak", // 异常处理和返回
        "mret",   // 从机器模式返回
        "sret",   // 从监管者模式返回
        "wfi",    // 等待中断
    ] {
        // 系统调用指令延迟较高
        let latency = if mnemonic == "ecall" {
            30
        } else if mnemonic == "mret" || mnemonic == "sret" {
            15
        } else if mnemonic == "wfi" {
            20
        } else {
            10
        };

        data.insert(
            mnemonic.to_string(),
            RiscvInstructionData {
                latency,
                throughput: 1,
                size: 4,
                execution_unit: ExecutionUnitType::System,
                has_side_effects: false,
                can_reorder: false,
            },
        );
    }

    // CSR读写指令（延迟3-5周期）
    for mnemonic in [
        // CSR读写指令
        "csrrw", "csrrs", "csrrc", "csrrwi", "csrrsi", "csrrci", "csrrc",
        // CSR立即数读写
        "csrrw", "csrrwi", // CSR置位和清位
        "csrrs", "csrrc", "csrsi", "csrci",
    ] {
        let latency = if mnemonic.contains('i') || mnemonic.contains('I') {
            5
        } else {
            3
        };

        // 避免重复插入
        if !data.contains_key(mnemonic) {
            data.insert(
                mnemonic.to_string(),
                RiscvInstructionData {
                    latency,
                    throughput: 1,
                    size: 4,
                    execution_unit: ExecutionUnitType::System,
                    has_side_effects: false,
                    can_reorder: false,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_m_extension_data() {
        let mut data = HashMap::new();
        init_riscv_m_extension_data(&mut data);

        // 验证乘法指令
        assert!(data.contains_key("mul"));
        assert_eq!(data["mul"].latency, 4);
        assert_eq!(data["mul"].throughput, 1);
        assert_eq!(data["mul"].execution_unit, ExecutionUnitType::Multiplier);

        // 验证除法指令
        assert!(data.contains_key("div"));
        assert_eq!(data["div"].latency, 32);
        assert_eq!(data["div"].throughput, 8);
        assert!(!data["div"].can_reorder);
    }

    #[test]
    fn test_a_extension_data() {
        let mut data = HashMap::new();
        init_riscv_a_extension_data(&mut data);

        // 验证原子指令
        assert!(data.contains_key("lr.w"));
        assert!(data.contains_key("sc.w"));
        assert!(data.contains_key("amoswap.w"));

        // 验证原子指令有副作用
        assert!(data["lr.w"].has_side_effects);
        assert!(!data["lr.w"].can_reorder);
    }

    #[test]
    fn test_f_extension_data() {
        let mut data = HashMap::new();
        init_riscv_f_extension_data(&mut data);

        // 验证浮点指令
        assert!(data.contains_key("fadd.s"));
        assert!(data.contains_key("fdiv.s"));
        assert!(data.contains_key("fsqrt.s"));

        // 验证执行单元
        assert_eq!(data["fadd.s"].execution_unit, ExecutionUnitType::FPU);
        assert_eq!(data["fdiv.s"].throughput, 8);
    }

    #[test]
    fn test_d_extension_data() {
        let mut data = HashMap::new();
        init_riscv_f_extension_data(&mut data);
        init_riscv_d_extension_data(&mut data);

        // 验证双精度浮点指令
        assert!(data.contains_key("fadd.d"));
        assert!(data.contains_key("fdiv.d"));

        // 验证双精度延迟比单精度高
        assert!(data["fdiv.d"].latency > data["fdiv.s"].latency);
        assert!(data["fadd.d"].latency > data["fadd.s"].latency);
    }

    #[test]
    fn test_c_extension_data() {
        let mut data = HashMap::new();
        init_riscv_c_extension_data(&mut data);

        // 验证压缩指令
        assert!(data.contains_key("c.add"));
        assert!(data.contains_key("c.mv"));
        assert!(data.contains_key("c.lwsp"));
        assert!(data.contains_key("c.jal"));

        // 验证压缩指令更小（2字节）
        assert_eq!(data["c.add"].size, 2);
        assert_eq!(data["c.mv"].size, 2);
        assert_eq!(data["c.lw"].size, 2);
    }

    #[test]
    fn test_all_extensions() {
        let mut data = HashMap::new();
        init_all_riscv_extension_data(&mut data);

        // 验证所有扩展都已初始化
        assert!(data.contains_key("mul")); // M扩展
        assert!(data.contains_key("lr.w")); // A扩展
        assert!(data.contains_key("fadd.s")); // F扩展
        assert!(data.contains_key("fadd.d")); // D扩展
        assert!(data.contains_key("c.add")); // C扩展

        // 统计指令数量
        let m_count = data
            .keys()
            .filter(|k| k.starts_with("mul") || k.starts_with("div") || k.starts_with("rem"))
            .count();
        assert!(m_count >= 16, "M扩展应该有至少16个指令");

        // 验证系统调用指令
        assert!(data.contains_key("ecall"));
        assert_eq!(data["ecall"].latency, 30);
        assert_eq!(data["ecall"].execution_unit, ExecutionUnitType::System);

        // 验证异常返回指令
        assert!(data.contains_key("mret"));
        assert!(data.contains_key("sret"));
        assert_eq!(data["mret"].latency, 15);
        assert_eq!(data["mret"].latency, 15);

        // 验证等待中断指令
        assert!(data.contains_key("wfi"));
        assert_eq!(data["wfi"].latency, 20);

        // 验证CSR读写指令
        assert!(data.contains_key("csrrw"));
        assert_eq!(data["csrrw"].latency, 3);
        assert_eq!(data["csrrw"].throughput, 1);
        assert_eq!(data["csrrw"].execution_unit, ExecutionUnitType::System);
    }
}
