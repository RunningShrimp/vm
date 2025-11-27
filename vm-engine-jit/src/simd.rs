use vm_simd::{vec_add, vec_sub, vec_mul};

pub extern "C" fn jit_vec_add(a: u64, b: u64, element_size: u64) -> u64 {
    vec_add(a, b, element_size as u8)
}

pub extern "C" fn jit_vec_sub(a: u64, b: u64, element_size: u64) -> u64 {
    vec_sub(a, b, element_size as u8)
}

pub extern "C" fn jit_vec_mul(a: u64, b: u64, element_size: u64) -> u64 {
    vec_mul(a, b, element_size as u8)
}
