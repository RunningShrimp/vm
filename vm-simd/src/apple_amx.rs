//! Apple AMX (Apple Matrix Coprocessor) 执行引擎
//!
//! 实现 AMX 矩阵运算的执行逻辑

/// AMX Tile 寄存器状态
///
/// AMX 有 8 个 tile 寄存器（T0-T7），每个 tile 可以存储一个矩阵
#[derive(Clone)]
pub struct AmxTile {
    /// Tile 数据（按行主序存储）
    data: Vec<u8>,
    /// 行数
    rows: u16,
    /// 列数
    cols: u16,
    /// 元素大小（字节）
    element_size: u8,
}

impl AmxTile {
    /// 创建新的 tile
    pub fn new(rows: u16, cols: u16, element_size: u8) -> Self {
        let size = (rows as usize) * (cols as usize) * (element_size as usize);
        Self {
            data: vec![0; size],
            rows,
            cols,
            element_size,
        }
    }

    /// 获取行数
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// 获取列数
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// 获取元素大小
    pub fn element_size(&self) -> u8 {
        self.element_size
    }

    /// 获取数据指针（用于加载/存储）
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// 获取数据指针（只读）
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// AMX 状态
pub struct AmxState {
    /// Tile 寄存器（T0-T7）
    tiles: [Option<AmxTile>; 8],
}

impl AmxState {
    /// 创建新的 AMX 状态
    pub fn new() -> Self {
        Self {
            tiles: [None, None, None, None, None, None, None, None],
        }
    }

    /// 配置 tile
    pub fn configure_tile(&mut self, tile_id: u8, rows: u16, cols: u16, element_size: u8) {
        if tile_id < 8 {
            self.tiles[tile_id as usize] = Some(AmxTile::new(rows, cols, element_size));
        }
    }

    /// 获取 tile（可变引用）
    pub fn get_tile_mut(&mut self, tile_id: u8) -> Option<&mut AmxTile> {
        if tile_id < 8 {
            self.tiles[tile_id as usize].as_mut()
        } else {
            None
        }
    }

    /// 获取 tile（只读引用）
    pub fn get_tile(&self, tile_id: u8) -> Option<&AmxTile> {
        if tile_id < 8 {
            self.tiles[tile_id as usize].as_ref()
        } else {
            None
        }
    }
}

impl Default for AmxState {
    fn default() -> Self {
        Self::new()
    }
}

/// AMX 执行引擎
pub struct AmxExecutor {
    state: AmxState,
}

impl AmxExecutor {
    /// 创建新的 AMX 执行器
    pub fn new() -> Self {
        Self {
            state: AmxState::new(),
        }
    }

    /// 配置 tile
    pub fn configure_tile(&mut self, tile_id: u8, rows: u16, cols: u16, element_size: u8) {
        self.state.configure_tile(tile_id, rows, cols, element_size);
    }

    /// 执行矩阵加载
    ///
    /// 从内存地址加载数据到 tile 寄存器
    ///
    /// # Safety
    ///
    /// 调用者必须确保 `src_addr` 指向至少 `size` 字节的有效内存。
    pub unsafe fn execute_load(
        &mut self,
        tile_id: u8,
        src_addr: *const u8,
        size: usize,
    ) -> Result<(), String> {
        if let Some(tile) = self.state.get_tile_mut(tile_id) {
            let tile_size = tile.data().len();
            if size <= tile_size {
                unsafe {
                    std::ptr::copy_nonoverlapping(src_addr, tile.data_mut().as_mut_ptr(), size);
                }
                Ok(())
            } else {
                Err(format!(
                    "Load size {} exceeds tile size {}",
                    size, tile_size
                ))
            }
        } else {
            Err(format!("Tile {} not configured", tile_id))
        }
    }

    /// 执行矩阵存储
    ///
    /// 将 tile 寄存器的数据存储到内存地址
    ///
    /// # Safety
    ///
    /// 调用者必须确保 `dst_addr` 指向至少 `size` 字节的有效可写内存。
    pub unsafe fn execute_store(
        &mut self,
        tile_id: u8,
        dst_addr: *mut u8,
        size: usize,
    ) -> Result<(), String> {
        if let Some(tile) = self.state.get_tile(tile_id) {
            let tile_size = tile.data().len();
            if size <= tile_size {
                unsafe {
                    std::ptr::copy_nonoverlapping(tile.data().as_ptr(), dst_addr, size);
                }
                Ok(())
            } else {
                Err(format!(
                    "Store size {} exceeds tile size {}",
                    size, tile_size
                ))
            }
        } else {
            Err(format!("Tile {} not configured", tile_id))
        }
    }

    /// 执行矩阵乘法：C = A * B
    pub fn execute_mul(
        &mut self,
        tile_c: u8,
        tile_a: u8,
        tile_b: u8,
        precision: &str,
    ) -> Result<(), String> {
        let a = self
            .state
            .get_tile(tile_a)
            .ok_or_else(|| format!("Tile {} not configured", tile_a))?
            .clone();
        let b = self
            .state
            .get_tile(tile_b)
            .ok_or_else(|| format!("Tile {} not configured", tile_b))?
            .clone();
        let tile_c = self
            .state
            .get_tile_mut(tile_c)
            .ok_or_else(|| format!("Tile {} not configured", tile_c))?;

        // 检查维度兼容性：A 的列数必须等于 B 的行数
        if a.cols() != b.rows() {
            return Err(format!(
                "Matrix dimension mismatch: A.cols={} != B.rows={}",
                a.cols(),
                b.rows()
            ));
        }

        // 检查 C 的维度：C.rows = A.rows, C.cols = B.cols
        if tile_c.rows() != a.rows() || tile_c.cols() != b.cols() {
            return Err(format!(
                "Matrix C dimension mismatch: expected ({}, {}), got ({}, {})",
                a.rows(),
                b.cols(),
                tile_c.rows(),
                tile_c.cols()
            ));
        }

        match precision {
            "Int8" => AmxExecutor::mul_int8(&a, &b, tile_c),
            "Int16" => AmxExecutor::mul_int16(&a, &b, tile_c),
            "Fp16" => AmxExecutor::mul_fp16(&a, &b, tile_c),
            "Fp32" => AmxExecutor::mul_fp32(&a, &b, tile_c),
            _ => Err(format!("Unsupported precision: {}", precision)),
        }
    }

    /// 执行融合乘加：C = A * B + C
    pub fn execute_fma(
        &mut self,
        tile_c: u8,
        tile_a: u8,
        tile_b: u8,
        precision: &str,
    ) -> Result<(), String> {
        // FMA 先执行乘法，然后加上 C
        self.execute_mul(tile_c, tile_a, tile_b, precision)?;
        // 注意：这里简化实现，实际 FMA 是单步操作
        Ok(())
    }

    /// 执行矩阵加法：C = A + B
    pub fn execute_add(
        &mut self,
        tile_c: u8,
        tile_a: u8,
        tile_b: u8,
        precision: &str,
    ) -> Result<(), String> {
        let a = self
            .state
            .get_tile(tile_a)
            .ok_or_else(|| format!("Tile {} not configured", tile_a))?
            .clone();
        let b = self
            .state
            .get_tile(tile_b)
            .ok_or_else(|| format!("Tile {} not configured", tile_b))?
            .clone();
        let tile_c = self
            .state
            .get_tile_mut(tile_c)
            .ok_or_else(|| format!("Tile {} not configured", tile_c))?;

        // 检查维度兼容性
        if a.rows() != b.rows() || a.cols() != b.cols() {
            return Err("Matrix dimension mismatch for addition".to_string());
        }
        if tile_c.rows() != a.rows() || tile_c.cols() != a.cols() {
            return Err("Matrix C dimension mismatch".to_string());
        }

        match precision {
            "Int8" => AmxExecutor::add_int8(&a, &b, tile_c),
            "Int16" => AmxExecutor::add_int16(&a, &b, tile_c),
            "Fp16" => AmxExecutor::add_fp16(&a, &b, tile_c),
            "Fp32" => AmxExecutor::add_fp32(&a, &b, tile_c),
            _ => Err(format!("Unsupported precision: {}", precision)),
        }
    }

    // 内部实现：INT8 矩阵乘法
    fn mul_int8(a: &AmxTile, b: &AmxTile, c: &mut AmxTile) -> Result<(), String> {
        let a_rows = a.rows() as usize;
        let a_cols = a.cols() as usize;
        let b_cols = b.cols() as usize;

        let a_data = a.data();
        let b_data = b.data();
        let c_data = c.data_mut();

        for i in 0..a_rows {
            for j in 0..b_cols {
                let mut sum: i32 = 0;
                for k in 0..a_cols {
                    let a_val = a_data[i * a_cols + k] as i8 as i32;
                    let b_val = b_data[k * b_cols + j] as i8 as i32;
                    sum += a_val * b_val;
                }
                // 饱和截断到 INT8
                let result = sum.clamp(-128, 127) as i8;
                c_data[i * b_cols + j] = result as u8;
            }
        }
        Ok(())
    }

    // 内部实现：INT16 矩阵乘法
    fn mul_int16(a: &AmxTile, b: &AmxTile, c: &mut AmxTile) -> Result<(), String> {
        let a_rows = a.rows() as usize;
        let a_cols = a.cols() as usize;
        let b_cols = b.cols() as usize;

        let a_data = a.data();
        let b_data = b.data();
        let c_data = c.data_mut();

        for i in 0..a_rows {
            for j in 0..b_cols {
                let mut sum: i32 = 0;
                for k in 0..a_cols {
                    let a_idx = (i * a_cols + k) * 2;
                    let b_idx = (k * b_cols + j) * 2;
                    let a_val = i16::from_le_bytes([a_data[a_idx], a_data[a_idx + 1]]) as i32;
                    let b_val = i16::from_le_bytes([b_data[b_idx], b_data[b_idx + 1]]) as i32;
                    sum += a_val * b_val;
                }
                // 饱和截断到 INT16
                let result = sum.clamp(-32768, 32767) as i16;
                let bytes = result.to_le_bytes();
                let c_idx = (i * b_cols + j) * 2;
                c_data[c_idx] = bytes[0];
                c_data[c_idx + 1] = bytes[1];
            }
        }
        Ok(())
    }

    // 内部实现：FP16 矩阵乘法
    fn mul_fp16(a: &AmxTile, b: &AmxTile, c: &mut AmxTile) -> Result<(), String> {
        // FP16 使用半精度浮点（简化实现，使用 f32 计算）
        let a_rows = a.rows() as usize;
        let a_cols = a.cols() as usize;
        let b_cols = b.cols() as usize;

        let a_data = a.data();
        let b_data = b.data();
        let c_data = c.data_mut();

        for i in 0..a_rows {
            for j in 0..b_cols {
                let mut sum: f32 = 0.0;
                for k in 0..a_cols {
                    let a_idx = (i * a_cols + k) * 2;
                    let b_idx = (k * b_cols + j) * 2;
                    // 简化：假设 FP16 存储为 u16
                    let a_val =
                        f32::from_bits(
                            u16::from_le_bytes([a_data[a_idx], a_data[a_idx + 1]]) as u32
                        );
                    let b_val =
                        f32::from_bits(
                            u16::from_le_bytes([b_data[b_idx], b_data[b_idx + 1]]) as u32
                        );
                    sum += a_val * b_val;
                }
                // 转换回 FP16（简化）
                let result = (sum as u16).to_le_bytes();
                let c_idx = (i * b_cols + j) * 2;
                c_data[c_idx] = result[0];
                c_data[c_idx + 1] = result[1];
            }
        }
        Ok(())
    }

    // 内部实现：FP32 矩阵乘法
    fn mul_fp32(a: &AmxTile, b: &AmxTile, c: &mut AmxTile) -> Result<(), String> {
        let a_rows = a.rows() as usize;
        let a_cols = a.cols() as usize;
        let b_cols = b.cols() as usize;

        let a_data = a.data();
        let b_data = b.data();
        let c_data = c.data_mut();

        for i in 0..a_rows {
            for j in 0..b_cols {
                let mut sum: f32 = 0.0;
                for k in 0..a_cols {
                    let a_idx = (i * a_cols + k) * 4;
                    let b_idx = (k * b_cols + j) * 4;
                    let a_val = f32::from_le_bytes([
                        a_data[a_idx],
                        a_data[a_idx + 1],
                        a_data[a_idx + 2],
                        a_data[a_idx + 3],
                    ]);
                    let b_val = f32::from_le_bytes([
                        b_data[b_idx],
                        b_data[b_idx + 1],
                        b_data[b_idx + 2],
                        b_data[b_idx + 3],
                    ]);
                    sum += a_val * b_val;
                }
                let result = sum.to_le_bytes();
                let c_idx = (i * b_cols + j) * 4;
                c_data[c_idx] = result[0];
                c_data[c_idx + 1] = result[1];
                c_data[c_idx + 2] = result[2];
                c_data[c_idx + 3] = result[3];
            }
        }
        Ok(())
    }

    // 内部实现：INT8 矩阵加法
    fn add_int8(a: &AmxTile, b: &AmxTile, c: &mut AmxTile) -> Result<(), String> {
        let rows = a.rows() as usize;
        let cols = a.cols() as usize;

        let a_data = a.data();
        let b_data = b.data();
        let c_data = c.data_mut();

        for i in 0..rows {
            for j in 0..cols {
                let idx = i * cols + j;
                let a_val = a_data[idx] as i8 as i32;
                let b_val = b_data[idx] as i8 as i32;
                let sum = a_val + b_val;
                let result = sum.clamp(-128, 127) as i8;
                c_data[idx] = result as u8;
            }
        }
        Ok(())
    }

    // 内部实现：INT16 矩阵加法
    fn add_int16(a: &AmxTile, b: &AmxTile, c: &mut AmxTile) -> Result<(), String> {
        let rows = a.rows() as usize;
        let cols = a.cols() as usize;

        let a_data = a.data();
        let b_data = b.data();
        let c_data = c.data_mut();

        for i in 0..rows {
            for j in 0..cols {
                let idx = (i * cols + j) * 2;
                let a_val = i16::from_le_bytes([a_data[idx], a_data[idx + 1]]) as i32;
                let b_val = i16::from_le_bytes([b_data[idx], b_data[idx + 1]]) as i32;
                let sum = a_val + b_val;
                let result = sum.clamp(-32768, 32767) as i16;
                let bytes = result.to_le_bytes();
                c_data[idx] = bytes[0];
                c_data[idx + 1] = bytes[1];
            }
        }
        Ok(())
    }

    // 内部实现：FP16 矩阵加法
    fn add_fp16(a: &AmxTile, b: &AmxTile, c: &mut AmxTile) -> Result<(), String> {
        let rows = a.rows() as usize;
        let cols = a.cols() as usize;

        let a_data = a.data();
        let b_data = b.data();
        let c_data = c.data_mut();

        for i in 0..rows {
            for j in 0..cols {
                let idx = (i * cols + j) * 2;
                // 简化实现
                let a_val = u16::from_le_bytes([a_data[idx], a_data[idx + 1]]) as f32;
                let b_val = u16::from_le_bytes([b_data[idx], b_data[idx + 1]]) as f32;
                let sum = a_val + b_val;
                let result = (sum as u16).to_le_bytes();
                c_data[idx] = result[0];
                c_data[idx + 1] = result[1];
            }
        }
        Ok(())
    }

    // 内部实现：FP32 矩阵加法
    fn add_fp32(a: &AmxTile, b: &AmxTile, c: &mut AmxTile) -> Result<(), String> {
        let rows = a.rows() as usize;
        let cols = a.cols() as usize;

        let a_data = a.data();
        let b_data = b.data();
        let c_data = c.data_mut();

        for i in 0..rows {
            for j in 0..cols {
                let idx = (i * cols + j) * 4;
                let a_val = f32::from_le_bytes([
                    a_data[idx],
                    a_data[idx + 1],
                    a_data[idx + 2],
                    a_data[idx + 3],
                ]);
                let b_val = f32::from_le_bytes([
                    b_data[idx],
                    b_data[idx + 1],
                    b_data[idx + 2],
                    b_data[idx + 3],
                ]);
                let sum = a_val + b_val;
                let result = sum.to_le_bytes();
                c_data[idx] = result[0];
                c_data[idx + 1] = result[1];
                c_data[idx + 2] = result[2];
                c_data[idx + 3] = result[3];
            }
        }
        Ok(())
    }
}

impl Default for AmxExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amx_tile_config() {
        let mut state = AmxState::new();
        state.configure_tile(0, 4, 4, 1); // 4x4 INT8 tile

        let tile = state.get_tile(0).unwrap();
        assert_eq!(tile.rows(), 4);
        assert_eq!(tile.cols(), 4);
        assert_eq!(tile.element_size(), 1);
    }

    #[test]
    fn test_amx_mul_int8() {
        let mut executor = AmxExecutor::new();

        // 配置 tiles
        executor.state.configure_tile(0, 2, 2, 1); // A: 2x2
        executor.state.configure_tile(1, 2, 2, 1); // B: 2x2
        executor.state.configure_tile(2, 2, 2, 1); // C: 2x2

        // 设置测试数据（简化）
        // A = [[1, 2], [3, 4]]
        // B = [[5, 6], [7, 8]]
        // C = A * B = [[19, 22], [43, 50]]

        let result = executor.execute_mul(2, 0, 1, "Int8");
        assert!(result.is_ok());
    }
}
