//! x86-64 前缀解码阶段
//! 解析指令前缀 (LOCK, REP, REX, 段覆盖, 大小覆盖等)

/// 指令前缀信息
#[derive(Debug, Default, Clone)]
pub struct PrefixInfo {
    pub lock: bool,
    pub rep: bool,
    pub repne: bool,
    pub seg: Option<u8>,
    pub op_size: bool,   // 0x66
    pub addr_size: bool, // 0x67
    pub rex: Option<RexPrefix>,
}

/// REX 前缀详细信息 (0x4x)
#[derive(Debug, Clone, Copy)]
pub struct RexPrefix {
    pub w: bool, // 64-bit operand size
    pub r: bool, // Extension of ModR/M.reg
    pub x: bool, // Extension of SIB.index
    pub b: bool, // Extension of ModR/M.rm
}

impl RexPrefix {
    pub fn from_byte(byte: u8) -> Self {
        Self {
            w: (byte & 0x08) != 0,
            r: (byte & 0x04) != 0,
            x: (byte & 0x02) != 0,
            b: (byte & 0x01) != 0,
        }
    }
}

/// 前缀解码器 - 返回已解析的前缀信息和第一个操作码字节
pub fn decode_prefixes<F>(mut read_u8: F) -> Result<(PrefixInfo, u8), String>
where
    F: FnMut() -> Result<u8, String>,
{
    let mut prefix = PrefixInfo::default();

    loop {
        let b = read_u8()?;
        match b {
            0xF0 => {
                if prefix.lock {
                    return Err("Duplicate LOCK prefix".to_string());
                }
                prefix.lock = true;
            }
            0xF2 => {
                if prefix.rep || prefix.repne {
                    return Err("Duplicate REP prefix".to_string());
                }
                prefix.repne = true;
            }
            0xF3 => {
                if prefix.rep || prefix.repne {
                    return Err("Duplicate REP prefix".to_string());
                }
                prefix.rep = true;
            }
            // 段覆盖前缀
            0x2E | 0x36 | 0x3E | 0x26 | 0x64 | 0x65 => {
                if prefix.seg.is_some() {
                    return Err("Duplicate segment override".to_string());
                }
                prefix.seg = Some(b);
            }
            // 操作数大小覆盖
            0x66 => {
                if prefix.op_size {
                    return Err("Duplicate operand-size override".to_string());
                }
                prefix.op_size = true;
            }
            // 地址大小覆盖
            0x67 => {
                if prefix.addr_size {
                    return Err("Duplicate address-size override".to_string());
                }
                prefix.addr_size = true;
            }
            // REX 前缀 (0x40-0x4F) - 必须在操作码之前
            0x40..=0x4F => {
                if prefix.rex.is_some() {
                    return Err("Duplicate REX prefix".to_string());
                }
                prefix.rex = Some(RexPrefix::from_byte(b));
                let opcode = read_u8()?;
                return Ok((prefix, opcode));
            }
            _ => {
                // 非前缀字节，作为操作码返回
                return Ok((prefix, b));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_prefix() {
        let bytes = vec![0x90];
        let mut iter = bytes.into_iter();
        let (prefix, opcode) = decode_prefixes(|| iter.next().ok_or_else(|| "EOF".to_string()))
            .expect("Failed to decode prefixes");

        assert!(!prefix.lock);
        assert!(!prefix.rep);
        assert!(!prefix.repne);
        assert!(prefix.seg.is_none());
        assert_eq!(opcode, 0x90);
    }

    #[test]
    fn test_lock_prefix() {
        let bytes = vec![0xF0, 0x89];
        let mut iter = bytes.into_iter();
        let (prefix, opcode) = decode_prefixes(|| iter.next().ok_or_else(|| "EOF".to_string()))
            .expect("Failed to decode prefixes with LOCK");

        assert!(prefix.lock);
        assert_eq!(opcode, 0x89);
    }

    #[test]
    fn test_rex_prefix() {
        let bytes = vec![0x48, 0x89]; // REX.W MOV
        let mut iter = bytes.into_iter();
        let (prefix, opcode) = decode_prefixes(|| iter.next().ok_or_else(|| "EOF".to_string()))
            .expect("Failed to decode REX prefix");

        let rex = prefix.rex.expect("REX prefix should be present");
        assert!(rex.w);
        assert_eq!(opcode, 0x89);
    }

    #[test]
    fn test_segment_override() {
        let bytes = vec![0x64, 0x8B]; // GS override
        let mut iter = bytes.into_iter();
        let (prefix, opcode) = decode_prefixes(|| iter.next().ok_or_else(|| "EOF".to_string()))
            .expect("Failed to decode segment override");

        assert_eq!(prefix.seg, Some(0x64));
        assert_eq!(opcode, 0x8B);
    }

    #[test]
    fn test_rep_prefix() {
        let bytes = vec![0xF3, 0xAA]; // REP STOSB
        let mut iter = bytes.into_iter();
        let (prefix, opcode) = decode_prefixes(|| iter.next().ok_or_else(|| "EOF".to_string()))
            .expect("Failed to decode REP prefix");

        assert!(prefix.rep);
        assert_eq!(opcode, 0xAA);
    }
}
