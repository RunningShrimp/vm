//! Windows Hypervisor Platform (WHPX) FFI bindings
//!
//! This module re-exports WHPX FFI bindings from the windows-rs crate,
//! providing centralized access to Windows virtualization APIs.

#[cfg(all(target_os = "windows", feature = "whpx"))]
pub use windows::Win32::System::Hypervisor::*;

/// WHPX API convenience functions
#[cfg(all(target_os = "windows", feature = "whpx"))]
pub mod whpx_api {
    use super::*;

    /// Get the size of the partition property
    #[inline]
    pub fn get_partition_property_size(
        partition: &WHV_PARTITION_HANDLE,
        property_code: WHV_PARTITION_PROPERTY_CODE,
    ) -> windows::core::Result<u32> {
        unsafe {
            let mut size = 0u32;
            WHvGetPartitionProperty(
                *partition,
                property_code,
                std::ptr::null_mut(),
                0,
                &mut size,
                std::ptr::null_mut(),
            )
            .map(|_| size)
        }
    }

    /// Read a partition property
    pub fn get_partition_property<T>(
        partition: &WHV_PARTITION_HANDLE,
        property_code: WHV_PARTITION_PROPERTY_CODE,
    ) -> windows::core::Result<T> {
        unsafe {
            let mut property: T = std::mem::zeroed();
            let mut size = std::mem::size_of::<T>() as u32;
            WHvGetPartitionProperty(
                *partition,
                property_code,
                &mut property as *mut _ as *mut _,
                size,
                &mut size,
                std::ptr::null_mut(),
            )?;
            Ok(property)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whpx_types_available() {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            // Test that WHPX types are available
            let _ = std::marker::PhantomData::<WHV_PARTITION_HANDLE>;
        }
    }
}
