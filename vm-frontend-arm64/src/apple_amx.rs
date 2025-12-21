//! Apple AMX (Apple Matrix Coprocessor) 指令解码器
//!
//! AMX 是 Apple Silicon 的专用矩阵协处理器，用于加速矩阵运算

use vm_core::{GuestAddr, VmError};
use vm_ir::{IRBuilder, IROp, RegId, RegisterFile};

/// AMX 指令类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmxInstruction {
    /// AMX_LD - 矩阵加载
    /// 从内存加载矩阵数据到 AMX tile 寄存器
    AmxLd {
        tile: u8,    // Tile 寄存器编号 (0-7)
        base: RegId, // 基址寄存器
        offset: i64, // 偏移量
    },
    /// AMX_ST - 矩阵存储
    /// 将 AMX tile 寄存器的数据存储到内存
    AmxSt {
        tile: u8,    // Tile 寄存器编号 (0-7)
        base: RegId, // 基址寄存器
        offset: i64, // 偏移量
    },
    /// AMX_FMA - 融合乘加
    /// C = A * B + C (矩阵运算)
    AmxFma {
        tile_c: u8,              // 目标 tile (C)
        tile_a: u8,              // 源 tile A
        tile_b: u8,              // 源 tile B
        precision: AmxPrecision, // 精度
    },
    /// AMX_MUL - 矩阵乘法
    /// C = A * B
    AmxMul {
        tile_c: u8,              // 目标 tile (C)
        tile_a: u8,              // 源 tile A
        tile_b: u8,              // 源 tile B
        precision: AmxPrecision, // 精度
    },
    /// AMX_ADD - 矩阵加法
    /// C = A + B
    AmxAdd {
        tile_c: u8,              // 目标 tile (C)
        tile_a: u8,              // 源 tile A
        tile_b: u8,              // 源 tile B
        precision: AmxPrecision, // 精度
    },
    /// AMX_SET - 设置 tile 配置
    /// 配置 tile 的行数和列数
    AmxSet {
        tile: u8,  // Tile 寄存器编号
        rows: u16, // 行数
        cols: u16, // 列数
    },
}

/// AMX 精度类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmxPrecision {
    Int8,  // 8位整数
    Int16, // 16位整数
    Fp16,  // 16位浮点
    Fp32,  // 32位浮点
}

/// AMX 指令解码器
pub struct AmxDecoder;

impl AmxDecoder {
    /// 创建新的 AMX 解码器
    pub fn new() -> Self {
        Self
    }

    /// 解码 AMX 指令
    ///
    /// AMX 指令通过系统寄存器访问（MRS/MSR）
    /// 这里我们识别特殊的系统寄存器地址范围
    ///
    /// 注意：实际的 AMX 指令编码格式是 Apple 专有的，这里使用简化的编码格式
    /// 格式：bits [31:28] = 0b1111 表示 AMX 指令
    ///       bits [27:24] = 操作码
    ///       bits [23:20] = tile 寄存器编号
    ///       bits [19:16] = 第二个 tile 寄存器编号（如适用）
    ///       bits [15:0]  = 立即数/偏移量
    pub fn decode(&self, insn: u32, _pc: GuestAddr) -> Result<Option<AmxInstruction>, VmError> {
        // 检查是否为 AMX 指令（简化检测：高位为特定模式）
        // 实际实现中，AMX 指令通过 MRS/MSR 访问系统寄存器
        // 系统寄存器地址范围：0xDA00-0xDAFF 用于 AMX

        // 简化实现：检查指令模式
        // bits [31:28] = 0b1111 且 bits [27:24] = 0b1010 表示 AMX 指令
        if (insn >> 28) & 0xF == 0xF && (insn >> 24) & 0xF == 0xA {
            let opcode = (insn >> 20) & 0xF;
            let tile1 = ((insn >> 16) & 0xF) as u8;
            let tile2 = ((insn >> 12) & 0xF) as u8;
            let imm = (insn & 0xFFF) as i64;

            // 符号扩展立即数
            let imm = if (imm & 0x800) != 0 {
                imm | 0xFFFFF000
            } else {
                imm
            };

            match opcode {
                0x0 => {
                    // AMX_LD
                    Ok(Some(AmxInstruction::AmxLd {
                        tile: tile1,
                        base: tile2 as RegId, // 使用 tile2 作为基址寄存器
                        offset: imm,
                    }))
                }
                0x1 => {
                    // AMX_ST
                    Ok(Some(AmxInstruction::AmxSt {
                        tile: tile1,
                        base: tile2 as RegId,
                        offset: imm,
                    }))
                }
                0x2 => {
                    // AMX_FMA
                    let precision = match (insn >> 8) & 0x3 {
                        0 => AmxPrecision::Int8,
                        1 => AmxPrecision::Int16,
                        2 => AmxPrecision::Fp16,
                        3 => AmxPrecision::Fp32,
                        _ => AmxPrecision::Fp32,
                    };
                    Ok(Some(AmxInstruction::AmxFma {
                        tile_c: tile1,
                        tile_a: tile2,
                        tile_b: ((insn >> 8) & 0xF) as u8,
                        precision,
                    }))
                }
                0x3 => {
                    // AMX_MUL
                    let precision = match (insn >> 8) & 0x3 {
                        0 => AmxPrecision::Int8,
                        1 => AmxPrecision::Int16,
                        2 => AmxPrecision::Fp16,
                        3 => AmxPrecision::Fp32,
                        _ => AmxPrecision::Fp32,
                    };
                    Ok(Some(AmxInstruction::AmxMul {
                        tile_c: tile1,
                        tile_a: tile2,
                        tile_b: ((insn >> 8) & 0xF) as u8,
                        precision,
                    }))
                }
                0x4 => {
                    // AMX_ADD
                    let precision = match (insn >> 8) & 0x3 {
                        0 => AmxPrecision::Int8,
                        1 => AmxPrecision::Int16,
                        2 => AmxPrecision::Fp16,
                        3 => AmxPrecision::Fp32,
                        _ => AmxPrecision::Fp32,
                    };
                    Ok(Some(AmxInstruction::AmxAdd {
                        tile_c: tile1,
                        tile_a: tile2,
                        tile_b: ((insn >> 8) & 0xF) as u8,
                        precision,
                    }))
                }
                0x5 => {
                    // AMX_SET
                    let rows = ((insn >> 8) & 0xFF) as u16;
                    let cols = (insn & 0xFF) as u16;
                    Ok(Some(AmxInstruction::AmxSet {
                        tile: tile1,
                        rows,
                        cols,
                    }))
                }
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    /// 将 AMX 指令转换为 IR
    pub fn to_ir(
        &self,
        insn: &AmxInstruction,
        builder: &mut IRBuilder,
        reg_file: &mut RegisterFile,
    ) -> Result<(), VmError> {
        match insn {
            AmxInstruction::AmxLd { tile, base, offset } => {
                // AMX 加载：从内存加载数据到 tile 寄存器
                // 使用 VendorLoad IR 操作
                let dst = reg_file.alloc_temp(); // 临时寄存器用于 tile 数据
                builder.push(IROp::VendorLoad {
                    dst,
                    base: *base,
                    offset: *offset,
                    vendor: "apple_amx".to_string(),
                    tile_id: *tile,
                });
            }
            AmxInstruction::AmxSt { tile, base, offset } => {
                // AMX 存储：将 tile 寄存器数据存储到内存
                let src = reg_file.alloc_temp(); // 从 tile 获取数据
                builder.push(IROp::VendorStore {
                    src,
                    base: *base,
                    offset: *offset,
                    vendor: "apple_amx".to_string(),
                    tile_id: *tile,
                });
            }
            AmxInstruction::AmxFma {
                tile_c,
                tile_a,
                tile_b,
                precision,
            } => {
                // AMX 融合乘加：C = A * B + C
                let dst = reg_file.alloc_temp();
                builder.push(IROp::VendorMatrixOp {
                    dst,
                    op: "fma".to_string(),
                    tile_c: *tile_c,
                    tile_a: *tile_a,
                    tile_b: *tile_b,
                    precision: format!("{:?}", precision),
                });
            }
            AmxInstruction::AmxMul {
                tile_c,
                tile_a,
                tile_b,
                precision,
            } => {
                // AMX 矩阵乘法：C = A * B
                let dst = reg_file.alloc_temp();
                builder.push(IROp::VendorMatrixOp {
                    dst,
                    op: "mul".to_string(),
                    tile_c: *tile_c,
                    tile_a: *tile_a,
                    tile_b: *tile_b,
                    precision: format!("{:?}", precision),
                });
            }
            AmxInstruction::AmxAdd {
                tile_c,
                tile_a,
                tile_b,
                precision,
            } => {
                // AMX 矩阵加法：C = A + B
                let dst = reg_file.alloc_temp();
                builder.push(IROp::VendorMatrixOp {
                    dst,
                    op: "add".to_string(),
                    tile_c: *tile_c,
                    tile_a: *tile_a,
                    tile_b: *tile_b,
                    precision: format!("{:?}", precision),
                });
            }
            AmxInstruction::AmxSet { tile, rows, cols } => {
                // AMX 设置 tile 配置
                builder.push(IROp::VendorConfig {
                    vendor: "apple_amx".to_string(),
                    tile_id: *tile,
                    config: format!("rows={},cols={}", rows, cols),
                });
            }
        }
        Ok(())
    }
}

impl Default for AmxDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amx_decode_ld() {
        let decoder = AmxDecoder::new();
        // 构造 AMX_LD 指令：tile=0, base=1, offset=0x100
        let insn = 0xF_A0_10_00 | (0x100 & 0xFFF);
        let result = decoder.decode(insn, vm_core::GuestAddr(0x1000));
        assert!(result.is_ok());
        if let Ok(Some(AmxInstruction::AmxLd { tile, base, offset })) = result {
            assert_eq!(tile, 0);
            assert_eq!(base, 1);
            assert_eq!(offset, 0x100);
        } else {
            panic!("Failed to decode AMX_LD");
        }
    }

    #[test]
    fn test_amx_decode_fma() {
        let decoder = AmxDecoder::new();
        // 构造 AMX_FMA 指令：tile_c=0, tile_a=1, tile_b=2, precision=FP32
        let insn = 0xF_A2_01_23 | (3 << 8); // opcode=2 (FMA), precision=3 (FP32)
        let result = decoder.decode(insn, vm_core::GuestAddr(0x1000));
        assert!(result.is_ok());
        if let Ok(Some(AmxInstruction::AmxFma {
            tile_c,
            tile_a,
            tile_b: _,
            precision,
        })) = result
        {
            assert_eq!(tile_c, 0);
            assert_eq!(tile_a, 1);
            assert_eq!(precision, AmxPrecision::Fp32);
        } else {
            panic!("Failed to decode AMX_FMA");
        }
    }
}
