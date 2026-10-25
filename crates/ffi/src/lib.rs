//! C ABI and Foreign Function Interface exports for Sillstone.
//!
//! Exposes stable C-compatible symbols conforming to `include/sillstone.h`.

#![allow(unsafe_code)]
#![deny(missing_docs)]

mod jni;

/// Parses one decimal `CARGO_PKG_VERSION_*` component at compile time.
const fn version_component(s: &str) -> u32 {
    let b = s.as_bytes();
    let mut n = 0u32;
    let mut i = 0;
    while i < b.len() {
        n = n * 10 + (b[i] - b'0') as u32;
        i += 1;
    }
    n
}

/// Returns the major version of the Sillstone library.
#[cfg_attr(not(test), unsafe(no_mangle))]
pub extern "C" fn sillstone_version_major() -> u32 {
    version_component(env!("CARGO_PKG_VERSION_MAJOR"))
}

/// Returns the minor version of the Sillstone library.
#[cfg_attr(not(test), unsafe(no_mangle))]
pub extern "C" fn sillstone_version_minor() -> u32 {
    version_component(env!("CARGO_PKG_VERSION_MINOR"))
}

/// Returns the patch version of the Sillstone library.
#[cfg_attr(not(test), unsafe(no_mangle))]
pub extern "C" fn sillstone_version_patch() -> u32 {
    version_component(env!("CARGO_PKG_VERSION_PATCH"))
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


#[doc(hidden)]
pub fn _touch_deps() {
    let _ = sillstone_curator::ffi::sillstone_curator_detect as *const () as usize;
    let _ = sillstone_curator::ffi::sillstone_curator_content_hash as *const () as usize;
    let _ = sillstone_curator::ffi::sillstone_curator_signature as *const () as usize;
    let _ = sillstone_curator::ffi::sillstone_curator_describe as *const () as usize;
    let _ = sillstone_curator::ffi::sillstone_curator_match as *const () as usize;
    let _ = sillstone_curator::ffi::sillstone_curator_free as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_phash as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_ahash as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_dhash as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_whash as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_colorhash as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_resize as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_crop as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_transpose as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_rotate as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_filter as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_adjust as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_color_convert as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_qoi_encode as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_qoi_decode as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_bmp_encode as *const () as usize;
    let _ = sillstone_image::ffi::sillstone_image_free_bytes as *const () as usize;
    let _ = sillstone_text::ffi::sillstone_text_tlsh as *const () as usize;
    let _ = sillstone_text::ffi::sillstone_text_tlsh_distance as *const () as usize;
    let _ = sillstone_tensor::ffi::sillstone_tensor_matmul_f64 as *const () as usize;
    let _ = sillstone_tensor::ffi::sillstone_tensor_add_f64 as *const () as usize;
    let _ = sillstone_tensor::ffi::sillstone_tensor_sum_f64 as *const () as usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_versions() {
        let parts: Vec<u32> = env!("CARGO_PKG_VERSION")
            .split(['.', '-'])
            .take(3)
            .map(|p| p.parse().unwrap())
            .collect();
        assert_eq!(
            [sillstone_version_major(), sillstone_version_minor(), sillstone_version_patch()],
            [parts[0], parts[1], parts[2]]
        );
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
