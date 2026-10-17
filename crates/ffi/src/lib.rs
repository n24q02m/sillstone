//! C ABI and Foreign Function Interface exports for Sillstone.
//!
//! Exposes stable C-compatible symbols conforming to `include/sillstone.h`.

#![allow(unsafe_code)]
#![deny(missing_docs)]

/// Returns the major version of the Sillstone library.
#[cfg_attr(not(test), unsafe(no_mangle))]
pub extern "C" fn sillstone_version_major() -> u32 {
    0
}

/// Returns the minor version of the Sillstone library.
#[cfg_attr(not(test), unsafe(no_mangle))]
pub extern "C" fn sillstone_version_minor() -> u32 {
    2
}

/// Returns the patch version of the Sillstone library.
#[cfg_attr(not(test), unsafe(no_mangle))]
pub extern "C" fn sillstone_version_patch() -> u32 {
    0
}

/// Computes a standard 64-bit FNV-1a checksum over a raw contiguous byte buffer.
///
/// # Safety
/// If `ptr` is non-null, it must point to at least `len` valid initialized bytes.
#[cfg_attr(not(test), unsafe(no_mangle))]
pub unsafe extern "C" fn sillstone_buffer_checksum(ptr: *const u8, len: usize) -> u64 {
    if ptr.is_null() || len == 0 {
        return sillstone_buffer::checksum_bytes(&[]);
    }
    // SAFETY: caller contract guarantees ptr points to len valid bytes.
    let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
    sillstone_buffer::checksum_bytes(slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_versions() {
        assert_eq!(sillstone_version_major(), 0);
        assert_eq!(sillstone_version_minor(), 2);
        assert_eq!(sillstone_version_patch(), 0);
    }

    #[test]
    fn test_ffi_checksum_empty() {
        let empty_sum = unsafe { sillstone_buffer_checksum(core::ptr::null(), 0) };
        assert_ne!(empty_sum, 0);
    }

    #[test]
    fn test_ffi_checksum_data() {
        let data = b"sillstone sample vector";
        let sum = unsafe { sillstone_buffer_checksum(data.as_ptr(), data.len()) };
        let direct_sum = sillstone_buffer::checksum_bytes(data);
        assert_eq!(sum, direct_sum);
    }
}
