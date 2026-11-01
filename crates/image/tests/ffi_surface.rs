//! The C ABI of `sillstone-image`, driven the way a foreign caller drives
//! it: raw pointers, lengths and geometry that may lie about each other.
//!
//! Two kinds of test live here. The geometry tests pin the contract that a
//! caller-supplied `len` and `width`/`height` are untrusted: a mismatch is a
//! status code, never an out-of-bounds slice index or an arithmetic panic
//! (the cdylib is `panic = "abort"`, so a panic takes the host process
//! down). The operation tests compare every exported operation against a
//! small independent model written in this file, not against the crate's
//! own safe API.

use sillstone_image::decode_bmp;
use sillstone_image::ffi::*;

const GRAY: u32 = PITH_LAYOUT_GRAY8;
const RGB: u32 = PITH_LAYOUT_RGB8;
const RGBA: u32 = PITH_LAYOUT_RGBA8;

fn channels(layout: u32) -> usize {
    match layout {
        GRAY => 1,
        RGB => 3,
        RGBA => 4,
        _ => panic!("test uses 8-bit layouts only"),
    }
}

/// A deterministic image whose every sample differs from its neighbours.
fn pattern(w: u32, h: u32, layout: u32) -> Vec<u8> {
    let c = channels(layout);
    let mut v = Vec::with_capacity(w as usize * h as usize * c);
    for y in 0..h {
        for x in 0..w {
            for k in 0..c {
                v.push((x * 37 + y * 91 + k as u32 * 13 + 5) as u8);
            }
        }
    }
    v
}

fn px(img: &[u8], w: u32, x: u32, y: u32, c: usize) -> &[u8] {
    let i = (y as usize * w as usize + x as usize) * c;
    &img[i..i + c]
}

// ---------------------------------------------------------------- geometry

/// `crop` used to index the source with `len` taken on trust: a buffer
/// shorter than the declared geometry made `src[start..end]` panic.
#[test]
fn crop_refuses_a_buffer_shorter_than_the_declared_geometry() {
    let data = [1u8, 2, 3, 4];
    let mut out = [0u8; 16];
    let code = unsafe {
        sillstone_image_crop(
            data.as_ptr(),
            data.len(),
            4,
            4,
            GRAY,
            0,
            0,
            4,
            4,
            out.as_mut_ptr(),
            out.len(),
        )
    };
    assert_eq!(code, PITH_E_INVALID);
}

/// A geometry that claims far more pixels than the buffer holds must not be
/// accepted just because the requested box happens to fit inside `len`.
#[test]
fn crop_refuses_a_geometry_that_overflows() {
    let data = [1u8, 2, 3, 4];
    let mut out = [0u8; 4];
    let code = unsafe {
        sillstone_image_crop(
            data.as_ptr(),
            data.len(),
            u32::MAX,
            u32::MAX,
            RGBA,
            0,
            0,
            1,
            1,
            out.as_mut_ptr(),
            out.len(),
        )
    };
    assert_eq!(code, PITH_E_INVALID);
}

#[test]
fn resize_refuses_an_overflowing_source_geometry() {
    let data = [0u8; 1];
    let mut out = [0u8; 3];
    let code = unsafe {
        sillstone_image_resize(
            data.as_ptr(),
            0,
            u32::MAX,
            u32::MAX,
            RGB,
            1,
            1,
            0,
            out.as_mut_ptr(),
            out.len(),
        )
    };
    assert_eq!(code, PITH_E_INVALID);
}

#[test]
fn resize_refuses_an_overflowing_target_geometry() {
    let data = pattern(2, 2, RGB);
    let mut out = [0u8; 3];
    let code = unsafe {
        sillstone_image_resize(
            data.as_ptr(),
            data.len(),
            2,
            2,
            RGB,
            u32::MAX,
            u32::MAX,
            0,
            out.as_mut_ptr(),
            out.len(),
        )
    };
    assert_eq!(code, PITH_E_INVALID);
}

#[test]
fn transpose_refuses_an_overflowing_geometry() {
    let data = [0u8; 1];
    let mut out = [0u8; 3];
    let code = unsafe {
        sillstone_image_transpose(
            data.as_ptr(),
            0,
            u32::MAX,
            u32::MAX,
            RGB,
            0,
            out.as_mut_ptr(),
            out.len(),
        )
    };
    assert_eq!(code, PITH_E_INVALID);
}

#[test]
fn color_convert_refuses_an_overflowing_geometry() {
    let data = [0u8; 1];
    let mut out = [0u8; 4];
    for (from, to) in [
        (RGB, GRAY),
        (RGBA, GRAY),
        (RGBA, RGB),
        (RGB, RGBA),
        (GRAY, RGB),
        (GRAY, RGBA),
    ] {
        let code = unsafe {
            sillstone_image_color_convert(
                data.as_ptr(),
                0,
                u32::MAX,
                u32::MAX,
                from,
                to,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert!(
            code == PITH_E_INVALID || code == PITH_E_REJECTED,
            "{from}->{to} returned {code}"
        );
    }
}

/// Every operation that takes a pixel buffer refuses a `len` that does not
/// match `width * height * channels` with a status code.
#[test]
fn every_operation_refuses_a_mismatched_len() {
    let data = pattern(4, 4, RGB);
    let short = data.len() - 1;
    let mut out = vec![0u8; 4 * 4 * 4 * 2];
    let (op, ol) = (out.as_mut_ptr(), out.len());
    let mut out_ptr: *mut u8 = core::ptr::null_mut();
    let mut out_len = 0usize;
    let results = unsafe {
        [
            sillstone_image_resize(data.as_ptr(), short, 4, 4, RGB, 2, 2, 0, op, ol),
            sillstone_image_crop(data.as_ptr(), short, 4, 4, RGB, 0, 0, 2, 2, op, ol),
            sillstone_image_transpose(data.as_ptr(), short, 4, 4, RGB, 2, op, ol),
            sillstone_image_rotate(data.as_ptr(), short, 4, 4, RGB, 45.0, 1, 1, op, ol),
            sillstone_image_filter(data.as_ptr(), short, 4, 4, RGB, 0, op, ol),
            sillstone_image_adjust(data.as_ptr(), short, 4, 4, RGB, 1, 1.0, op, ol),
            sillstone_image_qoi_encode(data.as_ptr(), short, 4, 4, RGB, &mut out_ptr, &mut out_len),
            sillstone_image_bmp_encode(data.as_ptr(), short, 4, 4, RGB, &mut out_ptr, &mut out_len),
        ]
    };
    for (i, code) in results.iter().enumerate() {
        assert!(
            *code == PITH_E_INVALID || *code == PITH_E_REJECTED,
            "operation {i} accepted a short buffer: {code}"
        );
    }
    assert!(
        out_ptr.is_null(),
        "a refused encode must not hand out a buffer"
    );
}

#[test]
fn null_pointers_are_refused_everywhere() {
    let data = pattern(2, 2, GRAY);
    let mut out = [0u8; 16];
    let (n, nm) = (core::ptr::null::<u8>(), core::ptr::null_mut::<u8>());
    let mut w = 0u32;
    let mut h = 0u32;
    let mut l = 0u32;
    let mut p: *mut u8 = core::ptr::null_mut();
    let mut pl = 0usize;
    let mut hash = 0u64;
    let results = unsafe {
        [
            sillstone_image_ahash(n, 0, 2, 2, GRAY, &mut hash),
            sillstone_image_dhash(n, 0, 2, 2, GRAY, &mut hash),
            sillstone_image_whash(n, 0, 2, 2, GRAY, 0, &mut hash),
            sillstone_image_colorhash(n, 0, 2, 2, GRAY, 3, &mut hash),
            sillstone_image_ahash(data.as_ptr(), 4, 2, 2, GRAY, core::ptr::null_mut()),
            sillstone_image_resize(n, 0, 2, 2, GRAY, 1, 1, 0, out.as_mut_ptr(), 16),
            sillstone_image_resize(data.as_ptr(), 4, 2, 2, GRAY, 1, 1, 0, nm, 16),
            sillstone_image_crop(n, 0, 2, 2, GRAY, 0, 0, 1, 1, out.as_mut_ptr(), 16),
            sillstone_image_transpose(n, 0, 2, 2, GRAY, 0, out.as_mut_ptr(), 16),
            sillstone_image_rotate(n, 0, 2, 2, GRAY, 0.0, 0, 0, out.as_mut_ptr(), 16),
            sillstone_image_filter(n, 0, 2, 2, GRAY, 0, out.as_mut_ptr(), 16),
            sillstone_image_adjust(n, 0, 2, 2, GRAY, 0, 1.0, out.as_mut_ptr(), 16),
            sillstone_image_color_convert(n, 0, 2, 2, GRAY, RGB, out.as_mut_ptr(), 16),
            sillstone_image_qoi_encode(n, 0, 2, 2, RGB, &mut p, &mut pl),
            sillstone_image_qoi_encode(data.as_ptr(), 4, 2, 2, RGB, core::ptr::null_mut(), &mut pl),
            sillstone_image_qoi_decode(n, 0, &mut w, &mut h, &mut l, &mut p, &mut pl),
            sillstone_image_qoi_decode(
                data.as_ptr(),
                4,
                core::ptr::null_mut(),
                &mut h,
                &mut l,
                &mut p,
                &mut pl,
            ),
            sillstone_image_bmp_encode(n, 0, 2, 2, RGB, &mut p, &mut pl),
            sillstone_image_rotate_size(2, 2, 0.0, 0, core::ptr::null_mut(), &mut h),
            sillstone_image_rotate_size(2, 2, 0.0, 0, &mut w, core::ptr::null_mut()),
        ]
    };
    for (i, code) in results.iter().enumerate() {
        assert_eq!(*code, PITH_E_INVALID, "case {i}");
    }
}

#[test]
fn unknown_codes_are_refused() {
    let data = pattern(2, 2, RGB);
    let mut out = [0u8; 64];
    let (op, ol) = (out.as_mut_ptr(), out.len());
    let mut p: *mut u8 = core::ptr::null_mut();
    let mut pl = 0usize;
    let results = unsafe {
        [
            sillstone_image_resize(data.as_ptr(), 12, 2, 2, RGB, 1, 1, 9, op, ol),
            sillstone_image_resize(data.as_ptr(), 12, 2, 2, 77, 1, 1, 0, op, ol),
            sillstone_image_crop(data.as_ptr(), 12, 2, 2, 77, 0, 0, 1, 1, op, ol),
            sillstone_image_transpose(data.as_ptr(), 12, 2, 2, RGB, 7, op, ol),
            sillstone_image_transpose(data.as_ptr(), 12, 2, 2, 77, 0, op, ol),
            sillstone_image_rotate(data.as_ptr(), 12, 2, 2, 77, 0.0, 0, 0, op, ol),
            sillstone_image_filter(data.as_ptr(), 12, 2, 2, RGB, 7, op, ol),
            sillstone_image_filter(data.as_ptr(), 12, 2, 2, 77, 0, op, ol),
            sillstone_image_adjust(data.as_ptr(), 12, 2, 2, RGB, 2, 1.0, op, ol),
            sillstone_image_adjust(data.as_ptr(), 12, 2, 2, 77, 0, 1.0, op, ol),
            sillstone_image_color_convert(data.as_ptr(), 12, 2, 2, RGB, RGB, op, ol),
            sillstone_image_qoi_encode(data.as_ptr(), 12, 2, 2, GRAY, &mut p, &mut pl),
            sillstone_image_bmp_encode(data.as_ptr(), 12, 2, 2, GRAY, &mut p, &mut pl),
            sillstone_image_ahash(data.as_ptr(), 12, 2, 2, 77, &mut 0u64),
            sillstone_image_dhash(data.as_ptr(), 12, 2, 2, 77, &mut 0u64),
            sillstone_image_whash(data.as_ptr(), 12, 2, 2, 77, 0, &mut 0u64),
            sillstone_image_colorhash(data.as_ptr(), 12, 2, 2, 77, 3, &mut 0u64),
        ]
    };
    for (i, code) in results.iter().enumerate() {
        assert_eq!(*code, PITH_E_INVALID, "case {i}");
    }
}

#[test]
fn output_buffers_that_are_too_small_are_refused() {
    let data = pattern(4, 4, RGB);
    let mut out = [0u8; 64];
    let op = out.as_mut_ptr();
    let results = unsafe {
        [
            sillstone_image_resize(data.as_ptr(), data.len(), 4, 4, RGB, 2, 2, 0, op, 11),
            sillstone_image_crop(data.as_ptr(), data.len(), 4, 4, RGB, 0, 0, 2, 2, op, 11),
            sillstone_image_transpose(data.as_ptr(), data.len(), 4, 4, RGB, 0, op, data.len() - 1),
            sillstone_image_rotate(data.as_ptr(), data.len(), 4, 4, RGB, 45.0, 1, 1, op, 3),
            sillstone_image_filter(data.as_ptr(), data.len(), 4, 4, RGB, 0, op, data.len() - 1),
            sillstone_image_adjust(
                data.as_ptr(),
                data.len(),
                4,
                4,
                RGB,
                1,
                1.0,
                op,
                data.len() - 1,
            ),
            sillstone_image_color_convert(data.as_ptr(), data.len(), 4, 4, RGB, GRAY, op, 15),
            sillstone_image_color_convert(data.as_ptr(), data.len(), 4, 4, RGB, RGBA, op, 63),
        ]
    };
    for (i, code) in results.iter().enumerate() {
        assert_eq!(*code, PITH_E_INVALID, "case {i}");
    }
}

#[test]
fn crop_refuses_empty_and_out_of_range_boxes() {
    let data = pattern(4, 4, GRAY);
    let mut out = [0u8; 16];
    let boxes = [
        (2, 0, 2, 4),
        (0, 2, 4, 2),
        (3, 0, 2, 4),
        (0, 0, 5, 4),
        (0, 0, 4, 5),
    ];
    for (x0, y0, x1, y1) in boxes {
        let code = unsafe {
            sillstone_image_crop(
                data.as_ptr(),
                16,
                4,
                4,
                GRAY,
                x0,
                y0,
                x1,
                y1,
                out.as_mut_ptr(),
                16,
            )
        };
        assert_eq!(code, PITH_E_INVALID, "{x0},{y0},{x1},{y1}");
    }
}

// -------------------------------------------------------------- operations

#[test]
fn crop_matches_the_row_model() {
    for layout in [GRAY, RGB, RGBA] {
        let c = channels(layout);
        let (w, h) = (7u32, 5u32);
        let img = pattern(w, h, layout);
        let (x0, y0, x1, y1) = (2u32, 1u32, 6u32, 4u32);
        let mut out = vec![0u8; (x1 - x0) as usize * (y1 - y0) as usize * c];
        let code = unsafe {
            sillstone_image_crop(
                img.as_ptr(),
                img.len(),
                w,
                h,
                layout,
                x0,
                y0,
                x1,
                y1,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(code, PITH_OK);
        let mut want = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                want.extend_from_slice(px(&img, w, x, y, c));
            }
        }
        assert_eq!(out, want, "layout {layout}");
    }
}

#[test]
fn transpose_matches_the_pixel_model_for_every_method() {
    // Pillow's Image.Transpose, expressed as where output pixel (ox, oy)
    // reads from in an `iw` x `ih` source.
    type Src = fn(u32, u32, u32, u32) -> (u32, u32);
    let methods: [(u32, bool, Src); 7] = [
        (0, false, |ox, oy, iw, _| (iw - 1 - ox, oy)),
        (1, false, |ox, oy, _, ih| (ox, ih - 1 - oy)),
        (2, true, |ox, oy, iw, _| (iw - 1 - oy, ox)),
        (3, false, |ox, oy, iw, ih| (iw - 1 - ox, ih - 1 - oy)),
        (4, true, |ox, oy, _, ih| (oy, ih - 1 - ox)),
        (5, true, |ox, oy, _, _| (oy, ox)),
        (6, true, |ox, oy, iw, ih| (iw - 1 - oy, ih - 1 - ox)),
    ];
    for layout in [GRAY, RGB, RGBA] {
        let c = channels(layout);
        let (iw, ih) = (5u32, 3u32);
        let img = pattern(iw, ih, layout);
        for (id, swaps, src) in methods {
            let (ow, oh) = if swaps { (ih, iw) } else { (iw, ih) };
            let mut out = vec![0u8; img.len()];
            let code = unsafe {
                sillstone_image_transpose(
                    img.as_ptr(),
                    img.len(),
                    iw,
                    ih,
                    layout,
                    id,
                    out.as_mut_ptr(),
                    out.len(),
                )
            };
            assert_eq!(code, PITH_OK, "method {id}");
            for oy in 0..oh {
                for ox in 0..ow {
                    let (sx, sy) = src(ox, oy, iw, ih);
                    assert_eq!(
                        px(&out, ow, ox, oy, c),
                        px(&img, iw, sx, sy, c),
                        "layout {layout} method {id} at {ox},{oy}"
                    );
                }
            }
        }
    }
}

#[test]
fn resize_keeps_a_flat_image_flat_for_every_filter() {
    for layout in [GRAY, RGB, RGBA] {
        let c = channels(layout);
        // RGBA resize premultiplies by alpha (as Pillow does), which only
        // round-trips exactly for opaque pixels, so keep alpha at 255.
        let mut img = vec![123u8; 8 * 6 * c];
        if layout == RGBA {
            for px in img.chunks_exact_mut(4) {
                px[3] = 255;
            }
        }
        for filter in 0..=4u32 {
            for (ow, oh) in [(3u32, 2u32), (8, 6), (13, 9)] {
                let mut out = vec![0u8; ow as usize * oh as usize * c];
                let code = unsafe {
                    sillstone_image_resize(
                        img.as_ptr(),
                        img.len(),
                        8,
                        6,
                        layout,
                        ow,
                        oh,
                        filter,
                        out.as_mut_ptr(),
                        out.len(),
                    )
                };
                assert_eq!(code, PITH_OK, "layout {layout} filter {filter} {ow}x{oh}");
                for (i, &b) in out.iter().enumerate() {
                    let want = if layout == RGBA && i % 4 == 3 {
                        255
                    } else {
                        123
                    };
                    assert_eq!(
                        b, want,
                        "layout {layout} filter {filter} {ow}x{oh} sample {i}"
                    );
                }
            }
        }
    }
}

#[test]
fn nearest_resize_to_the_same_size_is_the_identity() {
    for layout in [GRAY, RGB, RGBA] {
        let c = channels(layout);
        let img = pattern(6, 4, layout);
        let mut out = vec![0u8; img.len()];
        let code = unsafe {
            sillstone_image_resize(
                img.as_ptr(),
                img.len(),
                6,
                4,
                layout,
                6,
                4,
                0,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(code, PITH_OK);
        assert_eq!(out.len(), 6 * 4 * c);
        assert_eq!(out, img);
    }
}

#[test]
fn rotate_by_a_quarter_turn_equals_the_counter_clockwise_transpose() {
    for layout in [GRAY, RGB, RGBA] {
        let c = channels(layout);
        let (iw, ih) = (5u32, 3u32);
        let img = pattern(iw, ih, layout);
        let (mut ow, mut oh) = (0u32, 0u32);
        assert_eq!(
            unsafe { sillstone_image_rotate_size(iw, ih, 90.0, 1, &mut ow, &mut oh) },
            PITH_OK
        );
        assert_eq!((ow, oh), (ih, iw));
        let mut out = vec![0u8; ow as usize * oh as usize * c];
        let code = unsafe {
            sillstone_image_rotate(
                img.as_ptr(),
                img.len(),
                iw,
                ih,
                layout,
                90.0,
                1,
                0,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(code, PITH_OK);
        for oy in 0..oh {
            for ox in 0..ow {
                assert_eq!(
                    px(&out, ow, ox, oy, c),
                    px(&img, iw, iw - 1 - oy, ox, c),
                    "layout {layout} {ox},{oy}"
                );
            }
        }
    }
}

#[test]
fn rotate_covers_zero_expand_and_both_filters() {
    let img = pattern(6, 6, RGB);
    for (angle, expand, filter) in [
        (0.0, 0u32, 0u32),
        (45.0, 0, 0),
        (45.0, 1, 1),
        (-30.0, 1, 0),
        (180.0, 0, 1),
    ] {
        let (mut ow, mut oh) = (0u32, 0u32);
        assert_eq!(
            unsafe { sillstone_image_rotate_size(6, 6, angle, expand, &mut ow, &mut oh) },
            PITH_OK
        );
        let mut out = vec![0u8; ow as usize * oh as usize * 3];
        let code = unsafe {
            sillstone_image_rotate(
                img.as_ptr(),
                img.len(),
                6,
                6,
                RGB,
                angle,
                expand,
                filter,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(code, PITH_OK, "angle {angle} expand {expand}");
        if angle == 0.0 {
            assert_eq!(out, img);
        }
    }
    // A square rotated by 45 degrees with expand needs a larger canvas.
    let (mut ow, mut oh) = (0u32, 0u32);
    unsafe { sillstone_image_rotate_size(6, 6, 45.0, 1, &mut ow, &mut oh) };
    assert!(ow > 6 && oh > 6, "{ow}x{oh}");
}

#[test]
fn rotate_size_refuses_a_non_finite_angle() {
    let (mut w, mut h) = (0u32, 0u32);
    for angle in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            unsafe { sillstone_image_rotate_size(4, 4, angle, 1, &mut w, &mut h) },
            PITH_E_INVALID
        );
    }
}

#[test]
fn rotate_refuses_a_small_output_buffer() {
    let img = pattern(6, 6, GRAY);
    let mut out = [0u8; 4];
    let code = unsafe {
        sillstone_image_rotate(
            img.as_ptr(),
            img.len(),
            6,
            6,
            GRAY,
            45.0,
            1,
            1,
            out.as_mut_ptr(),
            out.len(),
        )
    };
    assert_eq!(code, PITH_E_INVALID);
}

#[test]
fn adjust_matches_the_sample_model() {
    for layout in [GRAY, RGB, RGBA] {
        let img = pattern(5, 4, layout);
        let mut out = vec![0u8; img.len()];
        let code = unsafe {
            sillstone_image_adjust(
                img.as_ptr(),
                img.len(),
                5,
                4,
                layout,
                1,
                0.0,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(code, PITH_OK);
        let want: Vec<u8> = img.iter().map(|b| 255 - b).collect();
        assert_eq!(out, want, "invert, layout {layout}");

        for factor in [0.0f64, 0.5, 1.0, 1.5, 4.0] {
            let code = unsafe {
                sillstone_image_adjust(
                    img.as_ptr(),
                    img.len(),
                    5,
                    4,
                    layout,
                    0,
                    factor,
                    out.as_mut_ptr(),
                    out.len(),
                )
            };
            assert_eq!(code, PITH_OK);
            let want: Vec<u8> = img
                .iter()
                .map(|&b| (b as f64 * factor).round().clamp(0.0, 255.0) as u8)
                .collect();
            assert_eq!(out, want, "brightness {factor}, layout {layout}");
        }
    }
}

#[test]
fn every_builtin_filter_runs_and_blur_keeps_a_flat_image_flat() {
    for layout in [GRAY, RGB, RGBA] {
        let flat = vec![90u8; 6 * 6 * channels(layout)];
        let busy = pattern(6, 6, layout);
        for filter in 0..=6u32 {
            for img in [&flat, &busy] {
                let mut out = vec![0u8; img.len()];
                let code = unsafe {
                    sillstone_image_filter(
                        img.as_ptr(),
                        img.len(),
                        6,
                        6,
                        layout,
                        filter,
                        out.as_mut_ptr(),
                        out.len(),
                    )
                };
                assert_eq!(code, PITH_OK, "layout {layout} filter {filter}");
            }
        }
        let mut out = vec![0u8; flat.len()];
        unsafe {
            sillstone_image_filter(
                flat.as_ptr(),
                flat.len(),
                6,
                6,
                layout,
                0,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(out, flat, "blur of a flat image, layout {layout}");
    }
}

#[test]
fn color_convert_follows_pillow_for_every_supported_pair() {
    let rgb: Vec<u8> = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
    let rgba: Vec<u8> = vec![
        255, 0, 0, 10, 0, 255, 0, 20, 0, 0, 255, 30, 255, 255, 255, 40,
    ];
    let gray: Vec<u8> = vec![0, 76, 150, 255];
    let luma = |r: u32, g: u32, b: u32| ((r * 19595 + g * 38470 + b * 7471 + 0x8000) >> 16) as u8;
    let mut out = vec![0u8; 16];

    let run = |data: &[u8], from, to, out: &mut Vec<u8>| unsafe {
        sillstone_image_color_convert(
            data.as_ptr(),
            data.len(),
            2,
            2,
            from,
            to,
            out.as_mut_ptr(),
            out.len(),
        )
    };

    assert_eq!(run(&rgb, RGB, GRAY, &mut out), PITH_OK);
    assert_eq!(
        &out[..4],
        &[luma(255, 0, 0), luma(0, 255, 0), luma(0, 0, 255), 255]
    );
    assert_eq!(&out[..3], &[76, 150, 29]);

    assert_eq!(run(&rgba, RGBA, GRAY, &mut out), PITH_OK);
    assert_eq!(&out[..4], &[76, 150, 29, 255]);

    assert_eq!(run(&rgba, RGBA, RGB, &mut out), PITH_OK);
    assert_eq!(
        &out[..12],
        &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]
    );

    assert_eq!(run(&rgb, RGB, RGBA, &mut out), PITH_OK);
    assert_eq!(
        &out[..16],
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255
        ]
    );

    assert_eq!(run(&gray, GRAY, RGB, &mut out), PITH_OK);
    assert_eq!(
        &out[..12],
        &[0, 0, 0, 76, 76, 76, 150, 150, 150, 255, 255, 255]
    );

    assert_eq!(run(&gray, GRAY, RGBA, &mut out), PITH_OK);
    assert_eq!(
        &out[..16],
        &[
            0, 0, 0, 255, 76, 76, 76, 255, 150, 150, 150, 255, 255, 255, 255, 255
        ]
    );
}

#[test]
fn color_convert_refuses_a_source_of_the_wrong_length() {
    let mut out = vec![0u8; 16];
    let rgb = [0u8; 11];
    let rgba = [0u8; 15];
    for (data, from, to) in [(&rgb[..], RGB, GRAY), (&rgba[..], RGBA, GRAY)] {
        let code = unsafe {
            sillstone_image_color_convert(
                data.as_ptr(),
                data.len(),
                2,
                2,
                from,
                to,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(code, PITH_E_REJECTED);
    }
    for (data, from, to) in [
        (&rgba[..], RGBA, RGB),
        (&rgb[..], RGB, RGBA),
        (&rgb[..], GRAY, RGB),
        (&rgb[..], GRAY, RGBA),
    ] {
        let code = unsafe {
            sillstone_image_color_convert(
                data.as_ptr(),
                data.len(),
                2,
                2,
                from,
                to,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(code, PITH_E_REJECTED, "{from}->{to}");
    }
}

#[test]
fn qoi_round_trips_through_the_c_abi() {
    for layout in [RGB, RGBA] {
        let c = channels(layout);
        let img = pattern(9, 7, layout);
        let (mut p, mut n) = (core::ptr::null_mut::<u8>(), 0usize);
        assert_eq!(
            unsafe {
                sillstone_image_qoi_encode(img.as_ptr(), img.len(), 9, 7, layout, &mut p, &mut n)
            },
            PITH_OK
        );
        assert!(!p.is_null() && n > 14);
        let encoded = unsafe { core::slice::from_raw_parts(p, n) }.to_vec();
        assert_eq!(&encoded[..4], b"qoif");

        let (mut w, mut h, mut l) = (0u32, 0u32, 0u32);
        let (mut dp, mut dn) = (core::ptr::null_mut::<u8>(), 0usize);
        let code = unsafe {
            sillstone_image_qoi_decode(
                encoded.as_ptr(),
                encoded.len(),
                &mut w,
                &mut h,
                &mut l,
                &mut dp,
                &mut dn,
            )
        };
        assert_eq!(code, PITH_OK);
        assert_eq!((w, h, l, dn), (9, 7, layout, 9 * 7 * c));
        assert_eq!(unsafe { core::slice::from_raw_parts(dp, dn) }, &img[..]);
        unsafe {
            sillstone_image_free_bytes(p, n);
            sillstone_image_free_bytes(dp, dn);
        }
    }
}

#[test]
fn qoi_decode_rejects_garbage_without_handing_out_a_buffer() {
    let garbage = [0x71u8, 0x6f, 0x69, 0x66, 0, 0, 0];
    let (mut w, mut h, mut l) = (0u32, 0u32, 0u32);
    let (mut p, mut n) = (core::ptr::null_mut::<u8>(), 0usize);
    let code = unsafe {
        sillstone_image_qoi_decode(
            garbage.as_ptr(),
            garbage.len(),
            &mut w,
            &mut h,
            &mut l,
            &mut p,
            &mut n,
        )
    };
    assert_eq!(code, PITH_E_REJECTED);
    assert!(p.is_null());
}

#[test]
fn bmp_encode_round_trips_through_the_decoder() {
    for layout in [RGB, RGBA] {
        let c = channels(layout);
        let (w, h) = (5u32, 3u32);
        let img = pattern(w, h, layout);
        let (mut p, mut n) = (core::ptr::null_mut::<u8>(), 0usize);
        assert_eq!(
            unsafe {
                sillstone_image_bmp_encode(img.as_ptr(), img.len(), w, h, layout, &mut p, &mut n)
            },
            PITH_OK
        );
        let bytes = unsafe { core::slice::from_raw_parts(p, n) }.to_vec();
        unsafe { sillstone_image_free_bytes(p, n) };
        assert_eq!(&bytes[..2], b"BM");
        assert_eq!(
            u32::from_le_bytes(bytes[2..6].try_into().unwrap()) as usize,
            bytes.len()
        );

        let back = decode_bmp(&bytes).expect("own BMP decodes");
        assert_eq!((back.width(), back.height()), (w, h));
        for y in 0..h {
            for x in 0..w {
                let got = px(back.as_slice(), w, x, y, 4);
                let want = px(&img, w, x, y, c);
                assert_eq!(&got[..3], &want[..3], "layout {layout} at {x},{y}");
                if c == 4 {
                    assert_eq!(got[3], want[3], "alpha at {x},{y}");
                }
            }
        }
    }
}

#[test]
fn free_bytes_ignores_null_and_empty() {
    unsafe {
        sillstone_image_free_bytes(core::ptr::null_mut(), 16);
        sillstone_image_free_bytes(core::ptr::NonNull::<u8>::dangling().as_ptr(), 0);
    }
}

#[test]
fn the_other_hashes_agree_across_eight_and_sixteen_bit_dumps() {
    // A 16-bit dump of `v << 8 | v` per sample hashes like the 8-bit dump
    // of `v`: the facade's documented `>> 8` rule must not change ordering.
    let (w, h) = (16u32, 16u32);
    for (layout8, layout16, c) in [
        (GRAY, PITH_LAYOUT_GRAY16, 1usize),
        (RGB, PITH_LAYOUT_RGB16, 3),
    ] {
        let img8 = pattern(w, h, layout8);
        let mut img16 = Vec::with_capacity(img8.len() * 2);
        for &v in &img8 {
            img16.extend_from_slice(&u16::from(v).wrapping_mul(257).to_le_bytes());
        }
        let _ = c;
        type Call = unsafe extern "C" fn(*const u8, usize, u32, u32, u32, *mut u64) -> i32;
        let simple: [Call; 2] = [sillstone_image_ahash, sillstone_image_dhash];
        for f in simple {
            let (mut a, mut b) = (0u64, 0u64);
            assert_eq!(
                unsafe { f(img8.as_ptr(), img8.len(), w, h, layout8, &mut a) },
                PITH_OK
            );
            assert_eq!(
                unsafe { f(img16.as_ptr(), img16.len(), w, h, layout16, &mut b) },
                PITH_OK
            );
            assert_eq!(a, b, "layout {layout8}");
        }
        let (mut a, mut b) = (0u64, 0u64);
        assert_eq!(
            unsafe { sillstone_image_whash(img8.as_ptr(), img8.len(), w, h, layout8, 0, &mut a) },
            PITH_OK
        );
        assert_eq!(
            unsafe {
                sillstone_image_whash(img16.as_ptr(), img16.len(), w, h, layout16, 0, &mut b)
            },
            PITH_OK
        );
        assert_eq!(a, b);
        assert_eq!(
            unsafe {
                sillstone_image_colorhash(img8.as_ptr(), img8.len(), w, h, layout8, 3, &mut a)
            },
            PITH_OK
        );
        assert_eq!(
            unsafe {
                sillstone_image_colorhash(img16.as_ptr(), img16.len(), w, h, layout16, 3, &mut b)
            },
            PITH_OK
        );
        assert_eq!(a, b);
    }
}

#[test]
fn every_hash_accepts_rgba_and_is_deterministic() {
    let img = pattern(16, 16, RGBA);
    let mut first = [0u64; 5];
    let mut second = [0u64; 5];
    for out in [&mut first, &mut second] {
        unsafe {
            assert_eq!(
                sillstone_image_phash(img.as_ptr(), img.len(), 16, 16, RGBA, &mut out[0]),
                PITH_OK
            );
            assert_eq!(
                sillstone_image_ahash(img.as_ptr(), img.len(), 16, 16, RGBA, &mut out[1]),
                PITH_OK
            );
            assert_eq!(
                sillstone_image_dhash(img.as_ptr(), img.len(), 16, 16, RGBA, &mut out[2]),
                PITH_OK
            );
            assert_eq!(
                sillstone_image_whash(img.as_ptr(), img.len(), 16, 16, RGBA, 0, &mut out[3]),
                PITH_OK
            );
            assert_eq!(
                sillstone_image_colorhash(img.as_ptr(), img.len(), 16, 16, RGBA, 3, &mut out[4]),
                PITH_OK
            );
        }
    }
    assert_eq!(first, second);
}

#[test]
fn hashes_refuse_a_mismatched_dump_length() {
    let img = pattern(8, 8, RGB);
    let mut hash = 0u64;
    let code = unsafe { sillstone_image_dhash(img.as_ptr(), img.len() - 1, 8, 8, RGB, &mut hash) };
    assert_eq!(code, PITH_E_INVALID);
    let code =
        unsafe { sillstone_image_whash(img.as_ptr(), img.len() + 1, 8, 8, RGB, 0, &mut hash) };
    assert_eq!(code, PITH_E_INVALID);
}
