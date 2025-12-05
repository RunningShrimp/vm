//! VirtIO Crypto 设备实现
//!
//! 提供加密加速功能，支持对称加密、非对称加密和哈希操作

use crate::virtio::{Queue, VirtioDevice};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::{MMU, VmError};

/// 加密算法类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CryptoAlgorithm {
    /// AES-128-CBC
    Aes128Cbc,
    /// AES-256-CBC
    Aes256Cbc,
    /// AES-128-GCM
    Aes128Gcm,
    /// AES-256-GCM
    Aes256Gcm,
    /// SHA-256
    Sha256,
    /// SHA-512
    Sha512,
    /// RSA-2048
    Rsa2048,
    /// RSA-4096
    Rsa4096,
}

/// 加密操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoOp {
    /// 加密
    Encrypt,
    /// 解密
    Decrypt,
    /// 哈希
    Hash,
    /// 签名
    Sign,
    /// 验证
    Verify,
}

/// 加密请求
#[derive(Debug, Clone)]
pub struct CryptoRequest {
    /// 操作类型
    pub op: CryptoOp,
    /// 算法
    pub algorithm: CryptoAlgorithm,
    /// 输入数据地址
    pub input_addr: u64,
    /// 输入数据长度
    pub input_len: u32,
    /// 输出数据地址
    pub output_addr: u64,
    /// 输出数据长度
    pub output_len: u32,
    /// 密钥地址（可选）
    pub key_addr: Option<u64>,
    /// 密钥长度
    pub key_len: Option<u32>,
    /// IV地址（可选）
    pub iv_addr: Option<u64>,
    /// IV长度
    pub iv_len: Option<u32>,
}

/// VirtIO Crypto 设备
pub struct VirtioCrypto {
    /// VirtIO队列（控制队列和数据队列）
    queues: Vec<Queue>,
    /// 支持的算法列表
    supported_algorithms: Vec<CryptoAlgorithm>,
    /// 请求ID到请求的映射
    requests: Arc<Mutex<HashMap<u64, CryptoRequest>>>,
    /// 下一个请求ID
    next_request_id: Arc<Mutex<u64>>,
    /// 设备状态
    device_status: u32,
    /// 最大段大小
    max_segment_size: u32,
}

impl VirtioCrypto {
    /// 创建新的VirtIO Crypto设备
    pub fn new() -> Self {
        Self {
            queues: vec![Queue::new(256); 2], // 控制队列和数据队列
            supported_algorithms: vec![
                CryptoAlgorithm::Aes128Cbc,
                CryptoAlgorithm::Aes256Cbc,
                CryptoAlgorithm::Aes128Gcm,
                CryptoAlgorithm::Aes256Gcm,
                CryptoAlgorithm::Sha256,
                CryptoAlgorithm::Sha512,
            ],
            requests: Arc::new(Mutex::new(HashMap::new())),
            next_request_id: Arc::new(Mutex::new(1)),
            device_status: 0,
            max_segment_size: 65536,
        }
    }

    /// 分配新的请求ID
    fn allocate_request_id(&self) -> u64 {
        let mut next = self.next_request_id.lock().unwrap();
        let id = *next;
        *next = next.wrapping_add(1);
        id
    }

    /// 处理加密请求
    fn process_crypto_request(
        &mut self,
        mmu: &mut dyn MMU,
        chain: &crate::virtio::DescChain,
    ) -> u32 {
        // 读取请求数据
        let mut request_data = Vec::new();
        for desc in &chain.descs {
            if desc.flags & 0x1 == 0 {
                // 可读
                let mut data = vec![0u8; desc.len as usize];
                if mmu.read_bulk(desc.addr, &mut data).is_ok() {
                    request_data.extend_from_slice(&data);
                }
            }
        }

        if request_data.len() < 16 {
            return 0;
        }

        // 解析请求（简化实现）
        let request_id = u64::from_le_bytes([
            request_data[0],
            request_data[1],
            request_data[2],
            request_data[3],
            request_data[4],
            request_data[5],
            request_data[6],
            request_data[7],
        ]);
        let op_code = request_data[8];
        let algorithm_code = request_data[9];

        // 创建请求对象
        let request = CryptoRequest {
            op: match op_code {
                0 => CryptoOp::Encrypt,
                1 => CryptoOp::Decrypt,
                2 => CryptoOp::Hash,
                _ => return 0,
            },
            algorithm: match algorithm_code {
                0 => CryptoAlgorithm::Aes128Cbc,
                1 => CryptoAlgorithm::Aes256Cbc,
                2 => CryptoAlgorithm::Sha256,
                _ => return 0,
            },
            input_addr: 0,
            input_len: 0,
            output_addr: 0,
            output_len: 0,
            key_addr: None,
            key_len: None,
            iv_addr: None,
            iv_len: None,
        };

        // 执行加密操作（简化实现：仅模拟）
        let result_len = self.execute_crypto_op(mmu, &request);

        // 存储请求
        if let Ok(mut requests) = self.requests.lock() {
            requests.insert(request_id, request);
        }

        result_len
    }

    /// 执行加密操作
    fn execute_crypto_op(&self, mmu: &mut dyn MMU, request: &CryptoRequest) -> u32 {
        // 简化实现：在实际系统中，这里应该调用硬件加速器或软件加密库
        // 这里我们只是模拟操作，返回输出长度

        match request.op {
            CryptoOp::Encrypt | CryptoOp::Decrypt => {
                // 对称加密/解密：输出长度通常等于输入长度（对于块加密）
                request.input_len
            }
            CryptoOp::Hash => {
                // 哈希：输出长度取决于算法
                match request.algorithm {
                    CryptoAlgorithm::Sha256 => 32,
                    CryptoAlgorithm::Sha512 => 64,
                    _ => 0,
                }
            }
            _ => 0,
        }
    }

    /// 获取支持的算法列表
    pub fn supported_algorithms(&self) -> &[CryptoAlgorithm] {
        &self.supported_algorithms
    }

    /// 设置设备状态
    pub fn set_device_status(&mut self, status: u32) {
        self.device_status = status;
    }

    /// 获取设备状态
    pub fn device_status(&self) -> u32 {
        self.device_status
    }
}

impl Default for VirtioCrypto {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtioDevice for VirtioCrypto {
    fn device_id(&self) -> u32 {
        20 // VirtIO Crypto device ID
    }

    fn num_queues(&self) -> usize {
        self.queues.len()
    }

    fn get_queue(&mut self, index: usize) -> &mut Queue {
        &mut self.queues[index]
    }

    fn process_queues(&mut self, mmu: &mut dyn MMU) {
        // 处理数据队列（索引1）
        while let Some(chain) = self.queues[1].pop(mmu) {
            let result_len = self.process_crypto_request(mmu, &chain);

            // 标记为已使用
            self.queues[1].add_used(mmu, chain.head_index, result_len);
        }
    }
}

/// VirtIO Crypto MMIO设备
pub struct VirtioCryptoMmio {
    device: VirtioCrypto,
}

impl VirtioCryptoMmio {
    pub fn new(device: VirtioCrypto) -> Self {
        Self { device }
    }

    pub fn device_mut(&mut self) -> &mut VirtioCrypto {
        &mut self.device
    }

    pub fn device(&self) -> &VirtioCrypto {
        &self.device
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::GuestAddr;

    #[test]
    fn test_virtio_crypto_creation() {
        let crypto = VirtioCrypto::new();
        assert!(!crypto.supported_algorithms().is_empty());
        assert_eq!(crypto.device_status(), 0);
    }

    #[test]
    fn test_virtio_crypto_device_id() {
        let mut crypto = VirtioCrypto::new();
        let mut mmu = MockMmu {
            memory: std::collections::HashMap::new(),
        };

        assert_eq!(crypto.device_id(), 20); // VirtIO Crypto device ID
        assert_eq!(crypto.num_queues(), 2); // 控制队列和数据队列
    }

    struct MockMmu {
        memory: std::collections::HashMap<u64, u8>,
    }

    impl MMU for MockMmu {
        fn translate(
            &mut self,
            va: GuestAddr,
            _access: vm_core::AccessType,
        ) -> Result<vm_core::GuestPhysAddr, VmError> {
            Ok(va)
        }

        fn fetch_insn(&self, _pc: GuestAddr) -> Result<u64, VmError> {
            Ok(0)
        }

        fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
            let mut value = 0u64;
            for i in 0..size {
                let byte = self.memory.get(&(pa + i as u64)).copied().unwrap_or(0);
                value |= (byte as u64) << (i * 8);
            }
            Ok(value)
        }

        fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
            for i in 0..size {
                let byte = ((val >> (i * 8)) & 0xFF) as u8;
                self.memory.insert(pa + i as u64, byte);
            }
            Ok(())
        }

        fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
            for (i, byte) in buf.iter_mut().enumerate() {
                *byte = self.memory.get(&(pa + i as u64)).copied().unwrap_or(0);
            }
            Ok(())
        }

        fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
            for (i, &byte) in buf.iter().enumerate() {
                self.memory.insert(pa + i as u64, byte);
            }
            Ok(())
        }

        fn map_mmio(
            &mut self,
            _base: GuestAddr,
            _size: u64,
            _device: Box<dyn vm_core::MmioDevice>,
        ) {
        }
        fn flush_tlb(&mut self) {}
        fn memory_size(&self) -> usize {
            0
        }
        fn dump_memory(&self) -> Vec<u8> {
            Vec::new()
        }
        fn restore_memory(&mut self, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }
}
