//! Refusals of the image C ABI that `ffi_surface.rs` does not reach, called
//! from a consumer crate the way the language SDKs call it.
//!
//! Every refusal asserts the exact status code and that the caller's output
//! memory is untouched (sentinel bytes), so a mutation that returns the right
//! code but still writes, or writes and returns the wrong code, is visible.
//! Each refusal sits next to a control call that differs only in the refused
//! argument and succeeds, so the refusal cannot be a side effect of some other
//! bad argument. Expected bytes are worked out by hand from Pillow's
//! documented conversions and the BMP file format, not recomputed from the
//! crate.

use sillstone_image::ffi::*;

/// Sentinel the 64-bit hash slot starts with.
const SLOT: u64 = 0xDEAD_BEEF_CAFE_F00D;
/// Sentinel byte output buffers start with.
const FILL: u8 = 0xAB;

/// Calls the hash entry point `kind` (0 pHash, 1 aHash, 2 dHash, 3 wHash with
/// level 3, 4 colorHash with 3 bins) on `dump`.
fn run_hash(kind: usize, dump: &[u8], w: u32, h: u32, layout: u32, slot: &mut u64) -> i32 {
    let (p, n) = (dump.as_ptr(), dump.len());
    unsafe {
        match kind {
            0 => sillstone_image_phash(p, n, w, h, layout, slot),
            1 => sillstone_image_ahash(p, n, w, h, layout, slot),
            2 => sillstone_image_dhash(p, n, w, h, layout, slot),
            3 => sillstone_image_whash(p, n, w, h, layout, 3, slot),
            4 => sillstone_image_colorhash(p, n, w, h, layout, 3, slot),
            _ => unreachable!("test names five hashes"),
        }
    }
}

const HASH_NAMES: [&str; 5] = ["phash", "ahash", "dhash", "whash", "colorhash"];

#[test]
fn phash_refuses_null_data_and_null_out() {
    let data = [0u8; 4];
    let mut slot = SLOT;
    let status =
        unsafe { sillstone_image_phash(core::ptr::null(), 4, 2, 2, PITH_LAYOUT_GRAY8, &mut slot) };
    assert_eq!(status, PITH_E_INVALID, "null data");
    assert_eq!(slot, SLOT, "slot written on refusal");

    let status = unsafe {
        sillstone_image_phash(
            data.as_ptr(),
            data.len(),
            2,
            2,
            PITH_LAYOUT_GRAY8,
            core::ptr::null_mut(),
        )
    };
    assert_eq!(status, PITH_E_INVALID, "null out");
}

#[test]
fn sixteen_bit_hashes_refuse_a_dump_that_does_not_match_the_geometry() {
    // (layout, bytes per pixel, name)
    for (layout, bytes_per_pixel) in [(PITH_LAYOUT_GRAY16, 2usize), (PITH_LAYOUT_RGB16, 6)] {
        for (kind, name) in HASH_NAMES.iter().enumerate() {
            // 2x2: needs 4 * bytes_per_pixel bytes; one byte short and one
            // byte long are both refused, an exact dump is accepted.
            let need = 4 * bytes_per_pixel;
            for bad_len in [need - 2, need + 2] {
                let dump = vec![0u8; bad_len];
                let mut slot = SLOT;
                let status = run_hash(kind, &dump, 2, 2, layout, &mut slot);
                assert_eq!(
                    status, PITH_E_INVALID,
                    "{name} layout {layout} len {bad_len}"
                );
                assert_eq!(slot, SLOT, "{name} wrote the slot on a length mismatch");
            }

            // u32::MAX x u32::MAX samples of two bytes overflow usize: refused
            // before any allocation, with an empty dump.
            let mut slot = SLOT;
            let status = run_hash(kind, &[], u32::MAX, u32::MAX, layout, &mut slot);
            assert_eq!(status, PITH_E_INVALID, "{name} layout {layout} overflow");
            assert_eq!(slot, SLOT, "{name} wrote the slot on overflowing geometry");
        }
    }
}

/// Control for the test above: the same entry points, layouts and call shape
/// accept a 16x16 constant dump of exactly the right length. A constant image
/// has no pixel strictly above the mean and no strictly rising neighbour, so
/// aHash and dHash (both strict comparisons) are 0 by definition.
#[test]
fn sixteen_bit_hashes_accept_an_exact_dump() {
    for (layout, bytes_per_pixel) in [(PITH_LAYOUT_GRAY16, 2usize), (PITH_LAYOUT_RGB16, 6)] {
        let dump = vec![0x80u8; 16 * 16 * bytes_per_pixel];
        for (kind, name) in HASH_NAMES.iter().enumerate() {
            let mut slot = SLOT;
            let status = run_hash(kind, &dump, 16, 16, layout, &mut slot);
            assert_eq!(status, PITH_OK, "{name} layout {layout}");
            if kind == 1 || kind == 2 {
                assert_eq!(slot, 0, "{name} of a constant image");
            }
        }
    }
}

#[test]
fn resize_rejects_a_zero_input_dimension() {
    for (layout, c) in [
        (PITH_LAYOUT_GRAY8, 1usize),
        (PITH_LAYOUT_RGB8, 3),
        (PITH_LAYOUT_RGBA8, 4),
    ] {
        // 0x5 input: the declared geometry is empty, so a zero-length dump
        // matches it, and the image constructor is what refuses the zero side.
        let empty: [u8; 0] = [];
        let mut out = vec![FILL; 2 * 2 * c];
        let status = unsafe {
            sillstone_image_resize(
                empty.as_ptr(),
                0,
                0,
                5,
                layout,
                2,
                2,
                0,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(status, PITH_E_REJECTED, "layout {layout} zero width");
        assert!(
            out.iter().all(|&b| b == FILL),
            "layout {layout} output written"
        );

        let status = unsafe {
            sillstone_image_resize(
                empty.as_ptr(),
                0,
                5,
                0,
                layout,
                2,
                2,
                0,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(status, PITH_E_REJECTED, "layout {layout} zero height");
        assert!(
            out.iter().all(|&b| b == FILL),
            "layout {layout} output written"
        );
    }
}

#[test]
fn resize_rejects_a_zero_output_dimension() {
    for (layout, c) in [
        (PITH_LAYOUT_GRAY8, 1usize),
        (PITH_LAYOUT_RGB8, 3),
        (PITH_LAYOUT_RGBA8, 4),
    ] {
        let src = vec![9u8; 2 * 2 * c];
        let mut out = vec![FILL; 16];
        for (ow, oh) in [(0u32, 3u32), (3, 0)] {
            // The declared output holds zero samples, so out_len 0 is enough
            // and the resampler is what refuses the empty target.
            let status = unsafe {
                sillstone_image_resize(
                    src.as_ptr(),
                    src.len(),
                    2,
                    2,
                    layout,
                    ow,
                    oh,
                    0,
                    out.as_mut_ptr(),
                    0,
                )
            };
            assert_eq!(status, PITH_E_REJECTED, "layout {layout} out {ow}x{oh}");
            assert!(
                out.iter().all(|&b| b == FILL),
                "layout {layout} output written"
            );
        }

        // Control: a non-empty target of the same input is accepted and, for a
        // flat image resized to 1x1 with the nearest filter, is that colour.
        let mut one = vec![FILL; c];
        let status = unsafe {
            sillstone_image_resize(
                src.as_ptr(),
                src.len(),
                2,
                2,
                layout,
                1,
                1,
                0,
                one.as_mut_ptr(),
                one.len(),
            )
        };
        assert_eq!(status, PITH_OK, "layout {layout} control");
        assert_eq!(one, vec![9u8; c], "layout {layout} control pixel");
    }
}

#[test]
fn rotate_size_rejects_a_bounding_box_wider_than_u32() {
    // 45 degrees with expand: width = (w + h) / sqrt(2) ~ 6.07e9 > u32::MAX.
    let (mut w, mut h) = (7u32, 9u32);
    let status =
        unsafe { sillstone_image_rotate_size(u32::MAX, u32::MAX, 45.0, 1, &mut w, &mut h) };
    assert_eq!(status, PITH_E_REJECTED);
    assert_eq!((w, h), (7, 9), "dimensions written on refusal");

    // Control: a 90 degree turn of the same extreme width only swaps sides and
    // fits, so the refusal above is the size, not the extreme input.
    let status = unsafe { sillstone_image_rotate_size(u32::MAX, 1, 90.0, 1, &mut w, &mut h) };
    assert_eq!(status, PITH_OK);
    assert_eq!((w, h), (1, u32::MAX));
}

/// (name, source, from layout, to layout, expected converted bytes)
type Conversion<'a> = (&'a str, &'a [u8], u32, u32, Vec<u8>);

#[test]
fn color_convert_refuses_an_output_buffer_one_byte_short() {
    const RGBA: u32 = PITH_LAYOUT_RGBA8;
    const RGB: u32 = PITH_LAYOUT_RGB8;
    const GRAY: u32 = PITH_LAYOUT_GRAY8;
    // Two pixels. Luma of (10,20,30) by Pillow's integer form
    // (19595*r + 38470*g + 7471*b + 0x8000) >> 16 is 1222248 >> 16 = 18, and a
    // grey pixel (100,100,100) maps to 100.
    let rgba = [100u8, 100, 100, 7, 10, 20, 30, 255];
    let gray = [10u8, 200];
    let cases: [Conversion; 4] = [
        ("rgba->gray", &rgba, RGBA, GRAY, vec![100, 18]),
        (
            "rgba->rgb",
            &rgba,
            RGBA,
            RGB,
            vec![100, 100, 100, 10, 20, 30],
        ),
        (
            "gray->rgb",
            &gray,
            GRAY,
            RGB,
            vec![10, 10, 10, 200, 200, 200],
        ),
        (
            "gray->rgba",
            &gray,
            GRAY,
            RGBA,
            vec![10, 10, 10, 255, 200, 200, 200, 255],
        ),
    ];
    for (name, src, from, to, expected) in cases {
        // The buffer is physically large enough; only `out_len` is short, so
        // an unchecked write would land in memory the caller owns and show.
        let mut out = vec![FILL; 16];
        let status = unsafe {
            sillstone_image_color_convert(
                src.as_ptr(),
                src.len(),
                2,
                1,
                from,
                to,
                out.as_mut_ptr(),
                expected.len() - 1,
            )
        };
        assert_eq!(status, PITH_E_INVALID, "{name}");
        assert!(out.iter().all(|&b| b == FILL), "{name} wrote on refusal");

        let status = unsafe {
            sillstone_image_color_convert(
                src.as_ptr(),
                src.len(),
                2,
                1,
                from,
                to,
                out.as_mut_ptr(),
                expected.len(),
            )
        };
        assert_eq!(status, PITH_OK, "{name} control");
        assert_eq!(
            &out[..expected.len()],
            &expected[..],
            "{name} control bytes"
        );
    }
}

#[test]
fn bmp_encode_rgba_refuses_a_dump_of_the_wrong_length() {
    let data = [1u8, 2, 3, 4];
    let mut out_ptr: *mut u8 = core::ptr::null_mut();
    let mut out_len: usize = 77;
    let status = unsafe {
        sillstone_image_bmp_encode(
            data.as_ptr(),
            3,
            1,
            1,
            PITH_LAYOUT_RGBA8,
            &mut out_ptr,
            &mut out_len,
        )
    };
    assert_eq!(status, PITH_E_REJECTED);
    assert!(out_ptr.is_null(), "pointer written on refusal");
    assert_eq!(out_len, 77, "length written on refusal");

    // Control: the exact four bytes encode. A 32-bit BMP with alpha is the
    // 14-byte file header, the 108-byte BITMAPV4HEADER and the 4 pixel bytes.
    let status = unsafe {
        sillstone_image_bmp_encode(
            data.as_ptr(),
            4,
            1,
            1,
            PITH_LAYOUT_RGBA8,
            &mut out_ptr,
            &mut out_len,
        )
    };
    assert_eq!(status, PITH_OK);
    assert_eq!(out_len, 14 + 108 + 4);
    let file = unsafe { core::slice::from_raw_parts(out_ptr, out_len) };
    assert_eq!(&file[..2], b"BM");
    unsafe { sillstone_image_free_bytes(out_ptr, out_len) };
}
