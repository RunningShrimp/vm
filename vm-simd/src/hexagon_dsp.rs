//! Qualcomm Hexagon DSP 执行引擎
//!
//! 实现 Hexagon DSP 的标量和向量运算

/// Hexagon DSP 执行器
pub struct HexagonExecutor;

impl HexagonExecutor {
    /// 创建新的 Hexagon 执行器
    pub fn new() -> Self {
        Self
    }

    /// 执行向量加法
    pub fn execute_vadd(
        &self,
        a: &[u8],
        b: &[u8],
        result: &mut [u8],
        element_size: u8,
    ) -> Result<(), String> {
        if a.len() != b.len() || result.len() != a.len() {
            return Err("Vector length mismatch".to_string());
        }

        match element_size {
            1 => {
                for i in 0..a.len() {
                    result[i] = a[i].wrapping_add(b[i]);
                }
            }
            2 => {
                for i in 0..(a.len() / 2) {
                    let idx = i * 2;
                    let a_val = u16::from_le_bytes([a[idx], a[idx + 1]]);
                    let b_val = u16::from_le_bytes([b[idx], b[idx + 1]]);
                    let sum = a_val.wrapping_add(b_val);
                    let bytes = sum.to_le_bytes();
                    result[idx] = bytes[0];
                    result[idx + 1] = bytes[1];
                }
            }
            4 => {
                for i in 0..(a.len() / 4) {
                    let idx = i * 4;
                    let a_val = u32::from_le_bytes([a[idx], a[idx + 1], a[idx + 2], a[idx + 3]]);
                    let b_val = u32::from_le_bytes([b[idx], b[idx + 1], b[idx + 2], b[idx + 3]]);
                    let sum = a_val.wrapping_add(b_val);
                    let bytes = sum.to_le_bytes();
                    result[idx..idx + 4].copy_from_slice(&bytes);
                }
            }
            _ => return Err(format!("Unsupported element size: {}", element_size)),
        }
        Ok(())
    }

    /// 执行向量乘法
    pub fn execute_vmul(
        &self,
        a: &[u8],
        b: &[u8],
        result: &mut [u8],
        element_size: u8,
    ) -> Result<(), String> {
        if a.len() != b.len() || result.len() != a.len() {
            return Err("Vector length mismatch".to_string());
        }

        match element_size {
            1 => {
                for i in 0..a.len() {
                    result[i] = a[i].wrapping_mul(b[i]);
                }
            }
            2 => {
                for i in 0..(a.len() / 2) {
                    let idx = i * 2;
                    let a_val = u16::from_le_bytes([a[idx], a[idx + 1]]);
                    let b_val = u16::from_le_bytes([b[idx], b[idx + 1]]);
                    let prod = a_val.wrapping_mul(b_val);
                    let bytes = prod.to_le_bytes();
                    result[idx] = bytes[0];
                    result[idx + 1] = bytes[1];
                }
            }
            _ => return Err(format!("Unsupported element size: {}", element_size)),
        }
        Ok(())
    }
}

impl Default for HexagonExecutor {
    fn default() -> Self {
        Self::new()
    }
}

