//! Pillow-exact 8-bit primitives: separable convolution resampling, the
//! `RGB → L` luma and the `RGB → HSV` conversion.
//!
//! The perceptual hashes of `imagehash` are defined by what Pillow does to
//! the pixels before the hash proper runs (`convert("L")`, then
//! `resize(..., LANCZOS)`), so bit-exact parity with that library needs
//! bit-exact parity with those two Pillow steps. Everything here mirrors
//! `libImaging/Resample.c` and `libImaging/Convert.c`:
//!
//! * coefficients are computed in `f64` per output pixel with the
//!   `(center, support)` window rule of `precompute_coeffs`, normalised to
//!   sum 1, then quantised to `PRECISION_BITS = 22` fixed point with
//!   round-half-away-from-zero;
//! * the horizontal pass runs first, unless the height shrinks by more than
//!   twice the width's shrink (`dh > 0 && dh > 2 * dw`, the rule of Pillow
//!   13.0, measured against its output; Pillow 12.3 always filters
//!   horizontally first). The first pass stores an 8-bit intermediate
//!   (clipped), the second pass reads that intermediate — two roundings,
//!   exactly as Pillow does;
//! * accumulators start at `1 << 21` (the rounding term) and are shifted
//!   right by 22 and clamped to `0..=255`.
//!
//! With the horizontal pass first, only the rows the vertical pass reads
//! are filtered.

use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::{OnceCell, RefCell};

use crate::raster::ResamplingFilter;

/// Fixed-point precision of the quantised coefficients (Pillow's
/// `PRECISION_BITS = 32 - 8 - 2`).
const PRECISION_BITS: u32 = 22;

/// Pillow's `ImagingConvert` RGB → L luma: ITU-R 601-2 on non-linear RGB,
/// `(19595·r + 38470·g + 7471·b + 0x8000) >> 16`.
#[inline]
pub fn rgb_to_l(r: u8, g: u8, b: u8) -> u8 {
    ((u32::from(r) * 19595 + u32::from(g) * 38470 + u32::from(b) * 7471 + 0x8000) >> 16) as u8
}

/// `convert("L")` of `n` pixels of `c` interleaved 8-bit channels (`c` is 3
/// for RGB or 4 for RGBA; the first three channels are R, G, B): the
/// Pillow luma of every pixel. Runtime-dispatched SIMD where available,
/// all routes integer arithmetic and so bit-identical to [`rgb_to_l`].
#[allow(unsafe_code)]
pub fn rgb_plane_to_l(src: &[u8], c: usize, n: usize) -> Vec<u8> {
    debug_assert!(c == 3 || c == 4);
    debug_assert!(src.len() >= n * c);
    let mut out = vec![0u8; n];
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 support was just detected.
            let done = unsafe { luma_avx2(src, c, &mut out) };
            return finish_luma(src, c, out, done);
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is part of the aarch64 baseline.
        let done = unsafe { luma_neon(src, c, &mut out) };
        return finish_luma(src, c, out, done);
    }
    #[allow(unreachable_code)]
    finish_luma(src, c, out, 0)
}

/// Scalar tail: converts pixels `from..` of `src` into `out`.
fn finish_luma(src: &[u8], c: usize, mut out: Vec<u8>, from: usize) -> Vec<u8> {
    for (o, p) in out[from..].iter_mut().zip(src[from * c..].chunks_exact(c)) {
        *o = rgb_to_l(p[0], p[1], p[2]);
    }
    out
}

/// Eight pixels per step: gather R/G/B into 32-bit lanes with `pshufb`,
/// multiply-accumulate, keep the byte at bit 16. Returns the pixels done.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(unsafe_code, unused_unsafe)]
unsafe fn luma_avx2(src: &[u8], c: usize, out: &mut [u8]) -> usize {
    use core::arch::x86_64::{
        __m256i, _mm_cvtsi128_si32, _mm_loadu_si128, _mm256_add_epi32, _mm256_castsi128_si256,
        _mm256_castsi256_si128, _mm256_extracti128_si256, _mm256_inserti128_si256,
        _mm256_loadu_si256, _mm256_mullo_epi32, _mm256_set1_epi32, _mm256_setr_epi8,
        _mm256_shuffle_epi8, _mm256_srli_epi32,
    };

    let n = out.len();
    // Byte index of channel `ch` of pixel `px` within a 128-bit lane.
    macro_rules! gather {
        ($ch:expr) => {{
            let s = if c == 3 { 3 } else { 4 };
            let idx = |px: i8| -> i8 { px * s + $ch };
            let (a, b, d, e) = (idx(0), idx(1), idx(2), idx(3));
            _mm256_setr_epi8(
                a, -1, -1, -1, b, -1, -1, -1, d, -1, -1, -1, e, -1, -1, -1, a, -1, -1, -1, b, -1,
                -1, -1, d, -1, -1, -1, e, -1, -1, -1,
            )
        }};
    }
    let (mr, mg, mb) = (gather!(0), gather!(1), gather!(2));
    let (kr, kg, kb) = (
        _mm256_set1_epi32(19595),
        _mm256_set1_epi32(38470),
        _mm256_set1_epi32(7471),
    );
    let round = _mm256_set1_epi32(0x8000);
    // Low byte of every 32-bit lane to the front of its 128-bit lane.
    let pack = _mm256_setr_epi8(
        0, 4, 8, 12, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 4, 8, 12, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1,
    );

    let mut i = 0;
    // Each step reads `8 * c` bytes plus, for RGB, a 16-byte window at +12.
    let reach = if c == 3 { 28 } else { 32 };
    while i + 8 <= n && i * c + reach <= src.len() {
        // SAFETY: `i * c` is within `src` (loop guard).
        let base = unsafe { src.as_ptr().add(i * c) };
        let v: __m256i = if c == 3 {
            // SAFETY: bytes `i*3 .. i*3 + 28` are in bounds (loop guard).
            unsafe {
                _mm256_inserti128_si256(
                    _mm256_castsi128_si256(_mm_loadu_si128(base.cast())),
                    _mm_loadu_si128(base.add(12).cast()),
                    1,
                )
            }
        } else {
            // SAFETY: bytes `i*4 .. i*4 + 32` are in bounds (loop guard).
            unsafe { _mm256_loadu_si256(base.cast()) }
        };
        let r = _mm256_shuffle_epi8(v, mr);
        let g = _mm256_shuffle_epi8(v, mg);
        let b = _mm256_shuffle_epi8(v, mb);
        let acc = _mm256_add_epi32(
            _mm256_add_epi32(_mm256_mullo_epi32(r, kr), _mm256_mullo_epi32(g, kg)),
            _mm256_add_epi32(_mm256_mullo_epi32(b, kb), round),
        );
        let packed = _mm256_shuffle_epi8(_mm256_srli_epi32(acc, 16), pack);
        let lo = _mm_cvtsi128_si32(_mm256_castsi256_si128(packed)) as u32;
        let hi = _mm_cvtsi128_si32(_mm256_extracti128_si256(packed, 1)) as u32;
        out[i..i + 4].copy_from_slice(&lo.to_le_bytes());
        out[i + 4..i + 8].copy_from_slice(&hi.to_le_bytes());
        i += 8;
    }
    i
}

/// Sixteen pixels per step: `vld3q_u8` de-interleaves R/G/B, widening
/// multiply-accumulate by the u16 luma weights, narrow the byte at bit 16.
#[cfg(target_arch = "aarch64")]
#[allow(unsafe_code, unused_unsafe)]
unsafe fn luma_neon(src: &[u8], c: usize, out: &mut [u8]) -> usize {
    use core::arch::aarch64::{
        uint8x16x3_t, uint8x16x4_t, vaddq_u32, vcombine_u8, vcombine_u16, vdupq_n_u32,
        vget_high_u8, vget_high_u16, vget_low_u8, vget_low_u16, vld3q_u8, vld4q_u8, vmlal_n_u16,
        vmovl_u8, vmovn_u16, vmull_n_u16, vshrn_n_u32, vst1q_u8,
    };

    // Luma of four widened pixels: `(19595 r + 38470 g + 7471 b + 0x8000) >> 16`.
    #[inline(always)]
    unsafe fn lane4(
        r: core::arch::aarch64::uint16x4_t,
        g: core::arch::aarch64::uint16x4_t,
        b: core::arch::aarch64::uint16x4_t,
    ) -> core::arch::aarch64::uint16x4_t {
        // SAFETY: NEON baseline.
        unsafe {
            let mut acc = vmull_n_u16(r, 19595);
            acc = vmlal_n_u16(acc, g, 38470);
            acc = vmlal_n_u16(acc, b, 7471);
            vshrn_n_u32::<16>(vaddq_u32(acc, vdupq_n_u32(0x8000)))
        }
    }

    let n = out.len();
    let mut i = 0;
    while i + 16 <= n && (i + 16) * c <= src.len() {
        // SAFETY: bytes `i*c .. (i+16)*c` are in bounds (loop guard).
        let (r, g, b) = unsafe {
            if c == 3 {
                let uint8x16x3_t(r, g, b) = vld3q_u8(src.as_ptr().add(i * 3));
                (r, g, b)
            } else {
                let uint8x16x4_t(r, g, b, _) = vld4q_u8(src.as_ptr().add(i * 4));
                (r, g, b)
            }
        };
        // SAFETY: NEON baseline; `out[i..i+16]` is in bounds (loop guard).
        unsafe {
            let (rl, rh) = (vmovl_u8(vget_low_u8(r)), vmovl_u8(vget_high_u8(r)));
            let (gl, gh) = (vmovl_u8(vget_low_u8(g)), vmovl_u8(vget_high_u8(g)));
            let (bl, bh) = (vmovl_u8(vget_low_u8(b)), vmovl_u8(vget_high_u8(b)));
            let lo = vcombine_u16(
                lane4(vget_low_u16(rl), vget_low_u16(gl), vget_low_u16(bl)),
                lane4(vget_high_u16(rl), vget_high_u16(gl), vget_high_u16(bl)),
            );
            let hi = vcombine_u16(
                lane4(vget_low_u16(rh), vget_low_u16(gh), vget_low_u16(bh)),
                lane4(vget_high_u16(rh), vget_high_u16(gh), vget_high_u16(bh)),
            );
            vst1q_u8(
                out.as_mut_ptr().add(i),
                vcombine_u8(vmovn_u16(lo), vmovn_u16(hi)),
            );
        }
        i += 16;
    }
    i
}

/// Pillow's RGB → HSV (`rgb2hsv_row`): all three outputs scaled to
/// `0..=255`, computed in single precision exactly like the C source.
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let (ri, gi, bi) = (i32::from(r), i32::from(g), i32::from(b));
    let maxc = ri.max(gi).max(bi);
    let minc = ri.min(gi).min(bi);
    let v = maxc as u8;
    if minc == maxc {
        return (0, 0, v);
    }
    let cr = (maxc - minc) as f32;
    let s = cr / maxc as f32;
    let rc = (maxc - ri) as f32 / cr;
    let gc = (maxc - gi) as f32 / cr;
    let bc = (maxc - bi) as f32 / cr;
    let h = if ri == maxc {
        bc - gc
    } else if gi == maxc {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    // C `fmodf(h / 6 + 1, 1)`; the operand is positive, so `%` is `fmodf`.
    let h = (h / 6.0 + 1.0) % 1.0;
    let uh = ((h * 255.0) as i32).clamp(0, 255) as u8;
    let us = ((s * 255.0) as i32).clamp(0, 255) as u8;
    (uh, us, v)
}

/// The `(support, kernel)` pair of a convolution filter, as in
/// `libImaging/Resample.c`.
fn kernel(filter: ResamplingFilter) -> (f64, fn(f64) -> f64) {
    fn sinc(x: f64) -> f64 {
        if x == 0.0 {
            1.0
        } else {
            let px = x * core::f64::consts::PI;
            px.sin() / px
        }
    }
    fn lanczos(x: f64) -> f64 {
        if (-3.0..3.0).contains(&x) {
            sinc(x) * sinc(x / 3.0)
        } else {
            0.0
        }
    }
    fn bilinear(x: f64) -> f64 {
        let x = x.abs();
        if x < 1.0 { 1.0 - x } else { 0.0 }
    }
    fn bicubic(x: f64) -> f64 {
        const A: f64 = -0.5;
        let x = x.abs();
        if x < 1.0 {
            ((A + 2.0) * x - (A + 3.0)) * x * x + 1.0
        } else if x < 2.0 {
            (((x - 5.0) * x + 8.0) * x - 4.0) * A
        } else {
            0.0
        }
    }
    fn boxf(x: f64) -> f64 {
        if x > -0.5 && x <= 0.5 { 1.0 } else { 0.0 }
    }
    match filter {
        ResamplingFilter::Lanczos => (3.0, lanczos),
        ResamplingFilter::Bicubic => (2.0, bicubic),
        ResamplingFilter::Bilinear => (1.0, bilinear),
        ResamplingFilter::Box | ResamplingFilter::Nearest => (0.5, boxf),
    }
}

/// Per-output-pixel source windows and fixed-point coefficients along one
/// axis (`precompute_coeffs` + `normalize_coeffs_8bpc`).
struct Coeffs {
    /// Stride of one output pixel's coefficient row in `kk`.
    ksize: usize,
    /// `(first source index, tap count)` per output pixel.
    bounds: Vec<(usize, usize)>,
    kk: Vec<i32>,
    /// `kk` split as `k = hi · 2¹² + lo` into two `i16` tables (rows padded
    /// with zeros to a multiple of 16), built on first use by the AVX2
    /// horizontal pass.
    #[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
    halves: OnceCell<Halves>,
}

/// The `i16` halves of a coefficient table; see [`Coeffs::halves`].
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
struct Halves {
    /// Row stride (a multiple of 16).
    stride: usize,
    hi: Vec<i16>,
    lo: Vec<i16>,
}

#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
impl Coeffs {
    /// `k = hi · 4096 + lo` with `lo ∈ [0, 4096)` and `hi` signed: both fit
    /// `i16` (|k| ≤ 2²³), so `Σ p·k` becomes two `madd_epi16` dot products.
    fn halves(&self) -> &Halves {
        self.halves.get_or_init(|| {
            let stride = self.ksize.next_multiple_of(16);
            let rows = self.bounds.len();
            let mut hi = vec![0i16; rows * stride];
            let mut lo = vec![0i16; rows * stride];
            for r in 0..rows {
                for t in 0..self.ksize {
                    let k = self.kk[r * self.ksize + t];
                    hi[r * stride + t] = (k >> 12) as i16;
                    lo[r * stride + t] = (k & 0xfff) as i16;
                }
            }
            Halves { stride, hi, lo }
        })
    }
}

/// Normalised `f64` weights of one axis (`precompute_coeffs`): per output
/// pixel the first source index, the tap count and `ksize` weights summing
/// to 1. `[lo, hi)` is the source interval being resampled (Pillow's `box`),
/// `[0, in_size)` for a plain resize.
struct Weights {
    ksize: usize,
    bounds: Vec<(usize, usize)>,
    w: Vec<f64>,
}

fn weights_f64(
    in_size: usize,
    out_size: usize,
    filter: ResamplingFilter,
    lo: f64,
    hi: f64,
) -> Weights {
    let (fsupport, f) = kernel(filter);
    let scale = (hi - lo) / out_size as f64;
    let filterscale = scale.max(1.0);
    let support = fsupport * filterscale;
    let ksize = support.ceil() as usize * 2 + 1;
    let ss = 1.0 / filterscale;

    let mut bounds = Vec::with_capacity(out_size);
    let mut w = vec![0.0f64; out_size * ksize];
    for xx in 0..out_size {
        let center = lo + (xx as f64 + 0.5) * scale;
        // C `(int)(x + 0.5)` truncates toward zero; the clamp below makes
        // the negative case irrelevant.
        let xmin = ((center - support + 0.5) as i64).max(0) as usize;
        let xmax = (((center + support + 0.5) as i64).max(0) as usize).min(in_size);
        let taps = xmax.saturating_sub(xmin);
        let row = &mut w[xx * ksize..(xx + 1) * ksize];
        let mut ww = 0.0f64;
        for (x, slot) in row.iter_mut().enumerate().take(taps) {
            let v = f((x as f64 + xmin as f64 - center + 0.5) * ss);
            *slot = v;
            ww += v;
        }
        if ww != 0.0 {
            for slot in row.iter_mut().take(taps) {
                *slot /= ww;
            }
        }
        bounds.push((xmin, taps));
    }
    Weights { ksize, bounds, w }
}

/// `normalize_coeffs_8bpc`: quantises [`Weights`] to `PRECISION_BITS`
/// fixed point.
fn compute_coeffs(
    in_size: usize,
    out_size: usize,
    filter: ResamplingFilter,
    lo: f64,
    hi: f64,
) -> Coeffs {
    let Weights { ksize, bounds, w } = weights_f64(in_size, out_size, filter, lo, hi);
    let kk = w
        .iter()
        .map(|&w| {
            // `(int)(±0.5 + w·2^22)`: truncation toward zero.
            let scaled = w * f64::from(1u32 << PRECISION_BITS);
            if w < 0.0 {
                (-0.5 + scaled) as i32
            } else {
                (0.5 + scaled) as i32
            }
        })
        .collect();
    Coeffs {
        ksize,
        bounds,
        kk,
        halves: OnceCell::new(),
    }
}

/// Coefficient tables cached per thread: the kernel evaluates one `sin`
/// per tap, which dominates a small-output resize (the perceptual hashes
/// resize to the same few shapes every call).
const CACHE_ENTRIES: usize = 8;
/// Tables above this many taps are recomputed rather than retained.
const CACHE_MAX_TAPS: usize = 1 << 16;

type CacheKey = (usize, usize, ResamplingFilter);

thread_local! {
    static COEFF_CACHE: RefCell<Vec<(CacheKey, Rc<Coeffs>)>> = const { RefCell::new(Vec::new()) };
}

/// Coefficients for resampling `[lo, hi)` of an `in_size` axis. Only the
/// whole-axis tables (the case every perceptual hash hits) are cached.
fn precompute(
    in_size: usize,
    out_size: usize,
    filter: ResamplingFilter,
    lo: f64,
    hi: f64,
) -> Rc<Coeffs> {
    if lo != 0.0 || hi != in_size as f64 {
        return Rc::new(compute_coeffs(in_size, out_size, filter, lo, hi));
    }
    let key = (in_size, out_size, filter);
    if let Some(hit) = COEFF_CACHE.with(|c| {
        c.borrow()
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| Rc::clone(v))
    }) {
        return hit;
    }
    let coeffs = Rc::new(compute_coeffs(in_size, out_size, filter, lo, hi));
    if coeffs.kk.len() <= CACHE_MAX_TAPS {
        COEFF_CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if c.len() == CACHE_ENTRIES {
                c.remove(0);
            }
            c.push((key, Rc::clone(&coeffs)));
        });
    }
    coeffs
}

#[inline]
fn clip8(acc: i32) -> u8 {
    (acc >> PRECISION_BITS).clamp(0, 255) as u8
}

/// Resizes an interleaved 8-bit raster (`c` channels per pixel) with
/// Pillow's separable fixed-point convolution.
///
/// Mirrors `Image.resize(size, resample=filter)` for modes `L`, `RGB` and
/// the premultiplied `La` / `RGBa` (the caller premultiplies; see
/// `resample::resize`). `Nearest` follows Pillow's affine
/// nearest-neighbour path (`src = ⌊(x + ½)·in/out⌋` with the centre
/// accumulated by addition).
pub fn resize_u8(
    src: &[u8],
    in_w: usize,
    in_h: usize,
    c: usize,
    out_w: usize,
    out_h: usize,
    filter: ResamplingFilter,
) -> Vec<u8> {
    let full = [0.0, 0.0, in_w as f64, in_h as f64];
    resize_u8_box(src, in_w, in_h, c, out_w, out_h, filter, full)
}

/// Pillow parses `box` into C `float`s (`"(ffff)"` in `_imaging.c`), so a
/// fractional box such as the `22.333333333333332` that `reducing_gap`
/// produces reaches the coefficient code as the nearest `f32`.
fn box_as_c_floats(bx: [f64; 4]) -> [f64; 4] {
    bx.map(|v| f64::from(v as f32))
}

/// [`resize_u8`] over the source rectangle `bx = [x0, y0, x1, y1]` (Pillow's
/// `box`, fractional coordinates allowed). `Nearest` ignores the box.
#[allow(clippy::too_many_arguments)]
pub fn resize_u8_box(
    src: &[u8],
    in_w: usize,
    in_h: usize,
    c: usize,
    out_w: usize,
    out_h: usize,
    filter: ResamplingFilter,
    bx: [f64; 4],
) -> Vec<u8> {
    let bx = box_as_c_floats(bx);
    let whole = bx == [0.0, 0.0, in_w as f64, in_h as f64];
    if whole && in_w == out_w && in_h == out_h {
        return src.to_vec();
    }
    if filter == ResamplingFilter::Nearest {
        return nearest(src, in_w, in_h, c, out_w, out_h, false);
    }
    let need_h = in_w != out_w || bx[0] != 0.0 || bx[2] != in_w as f64;
    let need_v = in_h != out_h || bx[1] != 0.0 || bx[3] != in_h as f64;

    let hc = need_h.then(|| precompute(in_w, out_w, filter, bx[0], bx[2]));
    let vc = need_v.then(|| precompute(in_h, out_h, filter, bx[1], bx[3]));

    if !vertical_first(in_w, in_h, out_w, out_h) {
        // Rows of `src` the vertical pass reads.
        let (ybox_first, ybox_last) = match &vc {
            Some(vc) => {
                let (first, _) = vc.bounds[0];
                let (lm, ln) = vc.bounds[out_h - 1];
                (first, lm + ln)
            }
            None => (0, in_h),
        };
        let mid = match &hc {
            Some(hc) => horizontal(
                &src[ybox_first * in_w * c..ybox_last * in_w * c],
                in_w,
                c,
                hc,
            ),
            None => src[ybox_first * in_w * c..ybox_last * in_w * c].to_vec(),
        };
        match &vc {
            Some(vc) => vertical(&mid, out_w * c, ybox_first, vc),
            None => mid,
        }
    } else {
        let mid = match &vc {
            Some(vc) => vertical(src, in_w * c, 0, vc),
            None => src.to_vec(),
        };
        match &hc {
            Some(hc) => horizontal(&mid, in_w, c, hc),
            None => mid,
        }
    }
}

/// Pillow's pass-order rule (`ImagingResampleInner`): filter vertically
/// first when the height shrinks by more than twice the width's shrink,
/// otherwise horizontally first. The two orders differ because each pass
/// rounds its intermediate.
fn vertical_first(in_w: usize, in_h: usize, out_w: usize, out_h: usize) -> bool {
    let dh = in_h as i64 - out_h as i64;
    let dw = in_w as i64 - out_w as i64;
    dh > 0 && dh > dw * 2
}

/// Horizontal pass over every row of `src` (`in_w` pixels of `c` channels
/// per row) → 8-bit rows of `hc.bounds.len()` pixels.
#[allow(unsafe_code)]
fn horizontal(src: &[u8], in_w: usize, c: usize, hc: &Coeffs) -> Vec<u8> {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 support was just detected.
            return unsafe { horizontal_avx2(src, in_w, c, hc) };
        }
    }
    horizontal_impl(src, in_w, c, hc)
}

/// AVX2 horizontal pass. Single-channel rows (the perceptual hashes' only
/// shape) use 16-tap `madd_epi16` dot products over the split coefficient
/// tables; interleaved rows keep the auto-vectorised loops. Integer
/// arithmetic modulo 2³² on both routes, so the result is bit-identical.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(unsafe_code)]
unsafe fn horizontal_avx2(src: &[u8], in_w: usize, c: usize, hc: &Coeffs) -> Vec<u8> {
    if c != 1 {
        return horizontal_impl(src, in_w, c, hc);
    }
    // SAFETY: AVX2 is enabled for this function (callers detected it).
    unsafe { horizontal_gray_avx2(src, in_w, hc) }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(unsafe_code, unused_unsafe)]
unsafe fn horizontal_gray_avx2(src: &[u8], in_w: usize, hc: &Coeffs) -> Vec<u8> {
    use core::arch::x86_64::{
        __m256i, _mm_loadu_si128, _mm256_add_epi32, _mm256_cvtepu8_epi16, _mm256_loadu_si256,
        _mm256_madd_epi16, _mm256_setzero_si256, _mm256_storeu_si256,
    };

    let out_w = hc.bounds.len();
    let rows = src.len() / in_w;
    let halves = hc.halves();
    let mut buf = vec![0u8; rows * out_w];
    for r in 0..rows {
        let srow = &src[r * in_w..(r + 1) * in_w];
        for (xx, &(xmin, taps)) in hc.bounds.iter().enumerate() {
            let hi = &halves.hi[xx * halves.stride..(xx + 1) * halves.stride];
            let lo = &halves.lo[xx * halves.stride..(xx + 1) * halves.stride];
            let mut acc_hi = _mm256_setzero_si256();
            let mut acc_lo = _mm256_setzero_si256();
            let mut t = 0;
            while t < taps {
                let at = xmin + t;
                let px = if at + 16 <= in_w {
                    // SAFETY: 16 bytes in bounds of `srow`.
                    unsafe { _mm_loadu_si128(srow.as_ptr().add(at).cast()) }
                } else {
                    // Tail: `taps` never reaches past the row, the zero
                    // coefficients cover the padding.
                    let mut tail = [0u8; 16];
                    tail[..in_w - at].copy_from_slice(&srow[at..]);
                    // SAFETY: `tail` is 16 bytes.
                    unsafe { _mm_loadu_si128(tail.as_ptr().cast()) }
                };
                let p = _mm256_cvtepu8_epi16(px);
                // SAFETY: `t < taps <= ksize <= stride` and `stride % 16 == 0`
                // leave 16 `i16`s at `t` (`t` steps by 16).
                let (kh, kl) = unsafe {
                    (
                        _mm256_loadu_si256(hi.as_ptr().add(t).cast::<__m256i>()),
                        _mm256_loadu_si256(lo.as_ptr().add(t).cast::<__m256i>()),
                    )
                };
                acc_hi = _mm256_add_epi32(acc_hi, _mm256_madd_epi16(p, kh));
                acc_lo = _mm256_add_epi32(acc_lo, _mm256_madd_epi16(p, kl));
                t += 16;
            }
            let (mut h8, mut l8) = ([0i32; 8], [0i32; 8]);
            // SAFETY: both targets are 32 bytes.
            unsafe {
                _mm256_storeu_si256(h8.as_mut_ptr().cast(), acc_hi);
                _mm256_storeu_si256(l8.as_mut_ptr().cast(), acc_lo);
            }
            let sum_hi = h8.iter().fold(0i32, |a, &v| a.wrapping_add(v));
            let sum_lo = l8.iter().fold(0i32, |a, &v| a.wrapping_add(v));
            let acc = (1i32 << (PRECISION_BITS - 1))
                .wrapping_add(sum_hi.wrapping_shl(12))
                .wrapping_add(sum_lo);
            buf[r * out_w + xx] = clip8(acc);
        }
    }
    buf
}

#[inline(always)]
fn horizontal_impl(src: &[u8], in_w: usize, c: usize, hc: &Coeffs) -> Vec<u8> {
    let out_w = hc.bounds.len();
    let rows = src.len() / (in_w * c);
    let mut buf = vec![0u8; rows * out_w * c];
    for r in 0..rows {
        let srow = &src[r * in_w * c..(r + 1) * in_w * c];
        let drow = &mut buf[r * out_w * c..(r + 1) * out_w * c];
        for (xx, &(xmin, taps)) in hc.bounds.iter().enumerate() {
            let k = &hc.kk[xx * hc.ksize..xx * hc.ksize + taps];
            if c == 1 {
                let px = &srow[xmin..xmin + taps];
                let acc = px
                    .iter()
                    .zip(k)
                    .fold(1i32 << (PRECISION_BITS - 1), |a, (&p, &w)| {
                        a + i32::from(p) * w
                    });
                drow[xx] = clip8(acc);
            } else {
                for ch in 0..c {
                    let mut acc = 1i32 << (PRECISION_BITS - 1);
                    for (t, &w) in k.iter().enumerate() {
                        acc += i32::from(srow[(xmin + t) * c + ch]) * w;
                    }
                    drow[xx * c + ch] = clip8(acc);
                }
            }
        }
    }
    buf
}

/// Vertical pass: `src` holds rows of `width` samples starting at source
/// row `first`; `vc.bounds` index absolute source rows.
#[allow(unsafe_code)]
fn vertical(src: &[u8], width: usize, first: usize, vc: &Coeffs) -> Vec<u8> {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 support was just detected.
            return unsafe { vertical_avx2(src, width, first, vc) };
        }
    }
    vertical_impl(src, width, first, vc)
}

/// AVX2 build of [`vertical_impl`]; bit-identical to the baseline.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(unsafe_code)]
unsafe fn vertical_avx2(src: &[u8], width: usize, first: usize, vc: &Coeffs) -> Vec<u8> {
    vertical_impl(src, width, first, vc)
}

#[inline(always)]
fn vertical_impl(src: &[u8], width: usize, first: usize, vc: &Coeffs) -> Vec<u8> {
    let out_h = vc.bounds.len();
    let mut out = vec![0u8; out_h * width];
    let mut acc = vec![0i32; width];
    for (yy, &(ymin, taps)) in vc.bounds.iter().enumerate() {
        let k = &vc.kk[yy * vc.ksize..yy * vc.ksize + taps];
        acc.fill(1i32 << (PRECISION_BITS - 1));
        for (t, &w) in k.iter().enumerate() {
            let row = &src[(ymin - first + t) * width..(ymin - first + t + 1) * width];
            for (a, &p) in acc.iter_mut().zip(row) {
                *a += i32::from(p) * w;
            }
        }
        for (o, &a) in out[yy * width..(yy + 1) * width].iter_mut().zip(&acc) {
            *o = clip8(a);
        }
    }
    out
}

/// Source index table of Pillow's `ImagingScaleAffine`: the sample centre
/// starts at `scale / 2` and is advanced by repeated *addition* (not
/// multiplication), so exact `.0` boundaries round the way Pillow's
/// accumulated `f64` does. Pillow's `I;16` goes through a different
/// transform that evaluates `scale * (i + 0.5)` per sample instead
/// (`product`); the two disagree when the centre lands on an integer.
fn nearest_table(in_size: usize, out_size: usize, product: bool) -> Vec<usize> {
    let scale = in_size as f64 / out_size as f64;
    let mut pos = scale * 0.5;
    (0..out_size)
        .map(|i| {
            let centre = if product {
                scale * (i as f64 + 0.5)
            } else {
                pos
            };
            let idx = (centre as usize).min(in_size - 1);
            pos += scale;
            idx
        })
        .collect()
}

pub(crate) fn nearest<T: Copy + Default>(
    src: &[T],
    in_w: usize,
    in_h: usize,
    c: usize,
    out_w: usize,
    out_h: usize,
    product: bool,
) -> Vec<T> {
    let xtab = nearest_table(in_w, out_w, product);
    let ytab = nearest_table(in_h, out_h, product);
    let mut out = vec![T::default(); out_w * out_h * c];
    for (y, &sy) in ytab.iter().enumerate() {
        let srow = &src[sy * in_w * c..(sy + 1) * in_w * c];
        let drow = &mut out[y * out_w * c..(y + 1) * out_w * c];
        for (x, &sx) in xtab.iter().enumerate() {
            drow[x * c..(x + 1) * c].copy_from_slice(&srow[sx * c..(sx + 1) * c]);
        }
    }
    out
}

/// How Pillow stores the value between and after the two passes of a
/// resize in a wide (non-8-bit) mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Quant {
    /// `I;16`: round half away from zero and clamp to `0..=65535` after
    /// *each* pass (`ROUND_UP` + `clip16`).
    U16,
    /// `F`: the accumulator is `f64`, every stored sample is an `f32`.
    F32,
}

impl Quant {
    #[inline]
    fn store(self, acc: f64) -> f64 {
        match self {
            Quant::U16 => {
                let r = if acc >= 0.0 { acc + 0.5 } else { acc - 0.5 };
                f64::from((r as i64).clamp(0, 65_535) as u16)
            }
            Quant::F32 => f64::from(acc as f32),
        }
    }
}

/// Pillow's resize for the wide modes (`I;16`, `F`): unquantised `f64`
/// coefficients, an `f64` accumulator, the stored intermediate rounded as
/// described by [`Quant`]. Samples are read through `get`; the result holds
/// the stored values as `f64`. Same pass order and `box` semantics as
/// [`resize_u8_box`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn resize_wide<T: Copy + Default>(
    src: &[T],
    get: impl Fn(T) -> f64,
    in_w: usize,
    in_h: usize,
    c: usize,
    out_w: usize,
    out_h: usize,
    filter: ResamplingFilter,
    bx: [f64; 4],
    quant: Quant,
) -> Vec<f64> {
    let bx = box_as_c_floats(bx);
    let whole = bx == [0.0, 0.0, in_w as f64, in_h as f64];
    if whole && in_w == out_w && in_h == out_h {
        return src.iter().map(|&v| get(v)).collect();
    }
    if filter == ResamplingFilter::Nearest {
        let picked = nearest(src, in_w, in_h, c, out_w, out_h, quant == Quant::U16);
        return picked.into_iter().map(get).collect();
    }
    let need_h = in_w != out_w || bx[0] != 0.0 || bx[2] != in_w as f64;
    let need_v = in_h != out_h || bx[1] != 0.0 || bx[3] != in_h as f64;

    let hw = need_h.then(|| weights_f64(in_w, out_w, filter, bx[0], bx[2]));
    let vw = need_v.then(|| weights_f64(in_h, out_h, filter, bx[1], bx[3]));

    // Horizontal pass: `buf` holds whole rows of `in_w` pixels.
    let h_pass = |buf: &[f64], hw: &Weights| -> Vec<f64> {
        let mut out = Vec::with_capacity(buf.len() / in_w * hw.bounds.len());
        for row in buf.chunks_exact(in_w * c) {
            for (xx, &(xmin, taps)) in hw.bounds.iter().enumerate() {
                let k = &hw.w[xx * hw.ksize..xx * hw.ksize + taps];
                for ch in 0..c {
                    let acc: f64 = k
                        .iter()
                        .enumerate()
                        .map(|(t, &wt)| row[(xmin + t) * c + ch] * wt)
                        .sum();
                    out.push(quant.store(acc));
                }
            }
        }
        out
    };
    // Vertical pass: `buf` holds rows of `width` samples starting at
    // source row `first`; `vw.bounds` index absolute source rows.
    let v_pass = |buf: &[f64], width: usize, first: usize, vw: &Weights| -> Vec<f64> {
        let mut out = vec![0.0f64; vw.bounds.len() * width];
        for (yy, &(ymin, taps)) in vw.bounds.iter().enumerate() {
            let k = &vw.w[yy * vw.ksize..yy * vw.ksize + taps];
            for x in 0..width {
                let acc: f64 = k
                    .iter()
                    .enumerate()
                    .map(|(t, &wt)| buf[(ymin - first + t) * width + x] * wt)
                    .sum();
                out[yy * width + x] = quant.store(acc);
            }
        }
        out
    };

    if vertical_first(in_w, in_h, out_w, out_h) {
        let all: Vec<f64> = src.iter().map(|&v| get(v)).collect();
        let mid = match &vw {
            Some(vw) => v_pass(&all, in_w * c, 0, vw),
            None => all,
        };
        return match &hw {
            Some(hw) => h_pass(&mid, hw),
            None => mid,
        };
    }

    // Only the source rows the vertical pass reads are filtered.
    let (first, last) = match &vw {
        Some(vw) => {
            let (first, _) = vw.bounds[0];
            let (lm, ln) = vw.bounds[out_h - 1];
            (first, lm + ln)
        }
        None => (0, in_h),
    };
    let rows: Vec<f64> = src[first * in_w * c..last * in_w * c]
        .iter()
        .map(|&v| get(v))
        .collect();
    let mid = match &hw {
        Some(hw) => h_pass(&rows, hw),
        None => rows,
    };
    match &vw {
        Some(vw) => v_pass(&mid, out_w * c, first, vw),
        None => mid,
    }
}
