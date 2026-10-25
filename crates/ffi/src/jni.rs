//! JNI bridge exports for the Java SDK (`io.github.sillstone.Generated`).

#![allow(unsafe_code)]
#![allow(missing_docs)]
#![allow(private_interfaces)]
#![allow(nonstandard_style)]

use core::ffi::c_void;

type JNIEnv = *const JniTable;
type JArray = *mut c_void;
type jbyte = i8;
type jint = i32;
type jlong = i64;

#[repr(C)]
struct JniTable {
    _prefix: [*mut c_void; 171],
    get_array_length: unsafe extern "system" fn(env: *mut JNIEnv, array: JArray) -> jint,
    _gap_before_new_byte_array: [*mut c_void; 4],
    new_byte_array: unsafe extern "system" fn(env: *mut JNIEnv, len: jint) -> JArray,
    _gap_before_new_long_array: [*mut c_void; 3],
    new_long_array: unsafe extern "system" fn(env: *mut JNIEnv, len: jint) -> JArray,
    _gap_before_byte_region: [*mut c_void; 19],
    get_byte_array_region: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: JArray,
        start: jint,
        len: jint,
        buf: *mut jbyte,
    ),
    _gap_before_set_byte_region: [*mut c_void; 7],
    set_byte_array_region: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: JArray,
        start: jint,
        len: jint,
        buf: *const jbyte,
    ),
}

unsafe fn table<'a>(env: *mut JNIEnv) -> &'a JniTable {
    unsafe { &**env }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1version_1major(
    _env: *mut c_void,
    _class: *mut c_void,
) -> jint {
    crate::sillstone_version_major() as jint
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1version_1minor(
    _env: *mut c_void,
    _class: *mut c_void,
) -> jint {
    crate::sillstone_version_minor() as jint
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1version_1patch(
    _env: *mut c_void,
    _class: *mut c_void,
) -> jint {
    crate::sillstone_version_patch() as jint
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1buffer_1checksum(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    _len: jlong,
) -> jlong {
    if env.is_null() || data.is_null() {
        return unsafe { crate::sillstone_buffer_checksum(core::ptr::null(), 0) as jlong };
    }
    let functions = unsafe { table(env) };
    let len = unsafe { (functions.get_array_length)(env, data) };
    if len <= 0 {
        return unsafe { crate::sillstone_buffer_checksum(core::ptr::null(), 0) as jlong };
    }
    let mut bytes = vec![0u8; len as usize];
    unsafe { (functions.get_byte_array_region)(env, data, 0, len, bytes.as_mut_ptr().cast()) };
    unsafe { crate::sillstone_buffer_checksum(bytes.as_ptr(), bytes.len()) as jlong }
}

unsafe fn get_input_bytes(env: *mut JNIEnv, data: JArray) -> Vec<u8> {
    if env.is_null() || data.is_null() {
        return Vec::new();
    }
    let functions = unsafe { table(env) };
    let len = unsafe { (functions.get_array_length)(env, data) };
    if len <= 0 {
        return Vec::new();
    }
    let mut bytes = vec![0u8; len as usize];
    unsafe { (functions.get_byte_array_region)(env, data, 0, len, bytes.as_mut_ptr().cast()) };
    bytes
}

unsafe fn create_output_byte_array(env: *mut JNIEnv, bytes: &[u8]) -> JArray {
    if env.is_null() {
        return core::ptr::null_mut();
    }
    let functions = unsafe { table(env) };
    let arr = unsafe { (functions.new_byte_array)(env, bytes.len() as jint) };
    if arr.is_null() {
        return core::ptr::null_mut();
    }
    unsafe {
        (functions.set_byte_array_region)(
            env,
            arr,
            0,
            bytes.len() as jint,
            bytes.as_ptr().cast(),
        )
    };
    arr
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1digest_1sha256(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
) -> JArray {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = [0u8; 32];
    let status = unsafe { sillstone_digest::ffi::sillstone_digest_sha256(bytes.as_ptr(), bytes.len(), out.as_mut_ptr()) };
    if status != 0 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_byte_array(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1digest_1sha3_1256(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
) -> JArray {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = [0u8; 32];
    let status = unsafe { sillstone_digest::ffi::sillstone_digest_sha3_256(bytes.as_ptr(), bytes.len(), out.as_mut_ptr()) };
    if status != 0 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_byte_array(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1digest_1sha3_1512(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
) -> JArray {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = [0u8; 64];
    let status = unsafe { sillstone_digest::ffi::sillstone_digest_sha3_512(bytes.as_ptr(), bytes.len(), out.as_mut_ptr()) };
    if status != 0 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_byte_array(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1digest_1blake2b_1512(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
) -> JArray {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = [0u8; 64];
    let status = unsafe { sillstone_digest::ffi::sillstone_digest_blake2b_512(bytes.as_ptr(), bytes.len(), out.as_mut_ptr()) };
    if status != 0 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_byte_array(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1digest_1blake3(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
) -> JArray {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = [0u8; 32];
    let status = unsafe { sillstone_digest::ffi::sillstone_digest_blake3(bytes.as_ptr(), bytes.len(), out.as_mut_ptr()) };
    if status != 0 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_byte_array(env, &out) }
}

unsafe fn get_input_doubles(env: *mut JNIEnv, array: JArray) -> Vec<f64> {
    if array.is_null() {
        return Vec::new();
    }
    let table: &[usize; 256] = unsafe { &*(*env as *const [usize; 256]) };
    let len = unsafe {
        let f: extern "system" fn(*mut JNIEnv, JArray) -> jint =
            core::mem::transmute(table[171]);
        f(env, array) as usize
    };
    let mut out = vec![0.0f64; len];
    if len > 0 {
        unsafe {
            let f: extern "system" fn(*mut JNIEnv, JArray, *mut f64, *mut u8) -> *mut f64 =
                core::mem::transmute(table[190]);
            let data = f(env, array, core::ptr::null_mut(), core::ptr::null_mut());
            out.copy_from_slice(core::slice::from_raw_parts(data, len));
            let r: extern "system" fn(*mut JNIEnv, JArray, *mut f64, jint) =
                core::mem::transmute(table[198]);
            r(env, array, data, 0);
        }
    }
    out
}

unsafe fn create_output_double_array(env: *mut JNIEnv, data: &[f64]) -> JArray {
    let table: &[usize; 256] = unsafe { &*(*env as *const [usize; 256]) };
    unsafe {
        let new: extern "system" fn(*mut JNIEnv, jint) -> JArray =
            core::mem::transmute(table[182]);
        let arr = new(env, data.len() as jint);
        if arr.is_null() {
            return core::ptr::null_mut();
        }
        let set: extern "system" fn(*mut JNIEnv, JArray, jint, jint, *const f64) =
            core::mem::transmute(table[214]);
        set(env, arr, 0, data.len() as jint, data.as_ptr());
        arr
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1math_1dst1(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
) -> JArray {
    let doubles = unsafe { get_input_doubles(env, data) };
    let mut out = vec![0.0f64; doubles.len()];
    let status = unsafe { sillstone_math::ffi::sillstone_math_dst1(doubles.as_ptr(), doubles.len(), out.as_mut_ptr()) };
    if status != 0 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_double_array(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1math_1dst2(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
) -> JArray {
    let doubles = unsafe { get_input_doubles(env, data) };
    let mut out = vec![0.0f64; doubles.len()];
    let status = unsafe { sillstone_math::ffi::sillstone_math_dst2(doubles.as_ptr(), doubles.len(), out.as_mut_ptr()) };
    if status != 0 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_double_array(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1math_1dst3(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
) -> JArray {
    let doubles = unsafe { get_input_doubles(env, data) };
    let mut out = vec![0.0f64; doubles.len()];
    let status = unsafe { sillstone_math::ffi::sillstone_math_dst3(doubles.as_ptr(), doubles.len(), out.as_mut_ptr()) };
    if status != 0 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_double_array(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1unicode_1grapheme_1count(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
) -> jlong {
    let bytes = unsafe { get_input_bytes(env, data) };
    let s = match core::str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(_) => return -2,
    };
    sillstone_unicode::graphemes(s).count() as jlong
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1unicode_1char_1category(
    _env: *mut JNIEnv,
    _class: *mut c_void,
    cp: jint,
) -> jint {
    let c = match char::from_u32(cp as u32) {
        Some(c) => c,
        None => return -1,
    };
    sillstone_unicode::char_category(c) as jint
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1phash(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    width: jint,
    height: jint,
    layout: jint,
) -> jlong {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = 0u64;
    let status = unsafe {
        sillstone_image::ffi::sillstone_image_phash(
            bytes.as_ptr(),
            bytes.len(),
            width as u32,
            height as u32,
            layout as u32,
            &mut out,
        )
    };
    if status != 0 {
        -1
    } else {
        out as jlong
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1ahash(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    width: jint,
    height: jint,
    layout: jint,
) -> jlong {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = 0u64;
    let status = unsafe {
        sillstone_image::ffi::sillstone_image_ahash(
            bytes.as_ptr(),
            bytes.len(),
            width as u32,
            height as u32,
            layout as u32,
            &mut out,
        )
    };
    if status != 0 {
        -1
    } else {
        out as jlong
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1dhash(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    width: jint,
    height: jint,
    layout: jint,
) -> jlong {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = 0u64;
    let status = unsafe {
        sillstone_image::ffi::sillstone_image_dhash(
            bytes.as_ptr(),
            bytes.len(),
            width as u32,
            height as u32,
            layout as u32,
            &mut out,
        )
    };
    if status != 0 {
        -1
    } else {
        out as jlong
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1whash(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    width: jint,
    height: jint,
    layout: jint,
) -> jlong {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = 0u64;
    let status = unsafe {
        sillstone_image::ffi::sillstone_image_whash(
            bytes.as_ptr(),
            bytes.len(),
            width as u32,
            height as u32,
            layout as u32,
            0,
            &mut out,
        )
    };
    if status != 0 {
        -1
    } else {
        out as jlong
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1colorhash(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    width: jint,
    height: jint,
    layout: jint,
) -> jlong {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = 0u64;
    let status = unsafe {
        sillstone_image::ffi::sillstone_image_colorhash(
            bytes.as_ptr(),
            bytes.len(),
            width as u32,
            height as u32,
            layout as u32,
            3,
            &mut out,
        )
    };
    if status != 0 {
        -1
    } else {
        out as jlong
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1text_1tlsh(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
) -> JArray {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = [0u8; 72];
    let status = unsafe {
        sillstone_text::ffi::sillstone_text_tlsh(
            bytes.as_ptr(),
            bytes.len(),
            out.as_mut_ptr(),
        )
    };
    if status != 0 {
        core::ptr::null_mut()
    } else {
        unsafe { create_output_byte_array(env, &out) }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1text_1tlsh_1distance(
    env: *mut JNIEnv,
    _class: *mut c_void,
    a: JArray,
    b: JArray,
) -> jint {
    let bytes_a = unsafe { get_input_bytes(env, a) };
    let bytes_b = unsafe { get_input_bytes(env, b) };
    let mut dist = 0u32;
    let status = unsafe {
        sillstone_text::ffi::sillstone_text_tlsh_distance(
            bytes_a.as_ptr(),
            bytes_b.as_ptr(),
            &mut dist,
        )
    };
    if status != 0 {
        -1
    } else {
        dist as jint
    }
}

// ---------------------------------------------------------------------------
// Image buffer operations and float64 tensor kernels.
//
// Failure is reported as a `null` array (or NaN for `sum`); the Java facade turns
// that into a `SillstoneFfiException`. Sizes handed to the C ABI are validated
// against the real array lengths here, so a wrong `outLen` or shape can never make
// the cdylib read or write out of bounds.
// ---------------------------------------------------------------------------

/// Runs a fixed-output image operation: `f(data, len, out_buf, out_len)`.
unsafe fn run_buffer_op(
    env: *mut JNIEnv,
    data: JArray,
    out_len: jint,
    f: impl FnOnce(*const u8, usize, *mut u8, usize) -> i32,
) -> JArray {
    if out_len < 0 {
        return core::ptr::null_mut();
    }
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut out = vec![0u8; out_len as usize];
    let status = f(bytes.as_ptr(), bytes.len(), out.as_mut_ptr(), out.len());
    if status != 0 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_byte_array(env, &out) }
}

/// Runs an encoder that hands back a library-allocated buffer, copies it into a
/// Java array and frees it: `f(data, len, out_ptr, out_len)`.
unsafe fn run_encode_op(
    env: *mut JNIEnv,
    data: JArray,
    f: impl FnOnce(*const u8, usize, *mut *mut u8, *mut usize) -> i32,
) -> JArray {
    let bytes = unsafe { get_input_bytes(env, data) };
    let mut ptr: *mut u8 = core::ptr::null_mut();
    let mut len = 0usize;
    let status = f(bytes.as_ptr(), bytes.len(), &mut ptr, &mut len);
    if status != 0 || ptr.is_null() {
        return core::ptr::null_mut();
    }
    let result = unsafe { create_output_byte_array(env, core::slice::from_raw_parts(ptr, len)) };
    unsafe { sillstone_image::ffi::sillstone_image_free_bytes(ptr, len) };
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1resize(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    in_w: jint,
    in_h: jint,
    layout: jint,
    out_w: jint,
    out_h: jint,
    filter: jint,
    out_len: jint,
) -> JArray {
    unsafe {
        run_buffer_op(env, data, out_len, |p, n, o, on| {
            sillstone_image::ffi::sillstone_image_resize(
                p, n, in_w as u32, in_h as u32, layout as u32, out_w as u32, out_h as u32,
                filter as u32, o, on,
            )
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1crop(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    in_w: jint,
    in_h: jint,
    layout: jint,
    x0: jint,
    y0: jint,
    x1: jint,
    y1: jint,
    out_len: jint,
) -> JArray {
    unsafe {
        run_buffer_op(env, data, out_len, |p, n, o, on| {
            sillstone_image::ffi::sillstone_image_crop(
                p, n, in_w as u32, in_h as u32, layout as u32, x0 as u32, y0 as u32, x1 as u32,
                y1 as u32, o, on,
            )
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1transpose(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    in_w: jint,
    in_h: jint,
    layout: jint,
    method: jint,
    out_len: jint,
) -> JArray {
    unsafe {
        run_buffer_op(env, data, out_len, |p, n, o, on| {
            sillstone_image::ffi::sillstone_image_transpose(
                p, n, in_w as u32, in_h as u32, layout as u32, method as u32, o, on,
            )
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1rotate(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    in_w: jint,
    in_h: jint,
    layout: jint,
    angle: f64,
    expand: jint,
    filter: jint,
    out_len: jint,
) -> JArray {
    unsafe {
        run_buffer_op(env, data, out_len, |p, n, o, on| {
            sillstone_image::ffi::sillstone_image_rotate(
                p, n, in_w as u32, in_h as u32, layout as u32, angle, expand as u32,
                filter as u32, o, on,
            )
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1filter(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    in_w: jint,
    in_h: jint,
    layout: jint,
    filter_id: jint,
    out_len: jint,
) -> JArray {
    unsafe {
        run_buffer_op(env, data, out_len, |p, n, o, on| {
            sillstone_image::ffi::sillstone_image_filter(
                p, n, in_w as u32, in_h as u32, layout as u32, filter_id as u32, o, on,
            )
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1adjust(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    in_w: jint,
    in_h: jint,
    layout: jint,
    op_id: jint,
    factor: f64,
    out_len: jint,
) -> JArray {
    unsafe {
        run_buffer_op(env, data, out_len, |p, n, o, on| {
            sillstone_image::ffi::sillstone_image_adjust(
                p, n, in_w as u32, in_h as u32, layout as u32, op_id as u32, factor, o, on,
            )
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1color_1convert(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    in_w: jint,
    in_h: jint,
    from_layout: jint,
    to_layout: jint,
    out_len: jint,
) -> JArray {
    unsafe {
        run_buffer_op(env, data, out_len, |p, n, o, on| {
            sillstone_image::ffi::sillstone_image_color_convert(
                p, n, in_w as u32, in_h as u32, from_layout as u32, to_layout as u32, o, on,
            )
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1qoi_1encode(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    in_w: jint,
    in_h: jint,
    layout: jint,
) -> JArray {
    unsafe {
        run_encode_op(env, data, |p, n, op, ol| {
            sillstone_image::ffi::sillstone_image_qoi_encode(
                p, n, in_w as u32, in_h as u32, layout as u32, op, ol,
            )
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1bmp_1encode(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
    in_w: jint,
    in_h: jint,
    layout: jint,
) -> JArray {
    unsafe {
        run_encode_op(env, data, |p, n, op, ol| {
            sillstone_image::ffi::sillstone_image_bmp_encode(
                p, n, in_w as u32, in_h as u32, layout as u32, op, ol,
            )
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1qoi_1decode(
    env: *mut JNIEnv,
    _class: *mut c_void,
    data: JArray,
) -> JArray {
    let bytes = unsafe { get_input_bytes(env, data) };
    let (mut w, mut h, mut layout) = (0u32, 0u32, 0u32);
    let mut ptr: *mut u8 = core::ptr::null_mut();
    let mut len = 0usize;
    let status = unsafe {
        sillstone_image::ffi::sillstone_image_qoi_decode(
            bytes.as_ptr(),
            bytes.len(),
            &mut w,
            &mut h,
            &mut layout,
            &mut ptr,
            &mut len,
        )
    };
    if status != 0 || ptr.is_null() {
        return core::ptr::null_mut();
    }
    let mut packed = Vec::with_capacity(12 + len);
    packed.extend_from_slice(&w.to_le_bytes());
    packed.extend_from_slice(&h.to_le_bytes());
    packed.extend_from_slice(&layout.to_le_bytes());
    packed.extend_from_slice(unsafe { core::slice::from_raw_parts(ptr, len) });
    unsafe { sillstone_image::ffi::sillstone_image_free_bytes(ptr, len) };
    unsafe { create_output_byte_array(env, &packed) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1tensor_1matmul_1f64(
    env: *mut JNIEnv,
    _class: *mut c_void,
    a: JArray,
    m: jint,
    k1: jint,
    b: JArray,
    k2: jint,
    n: jint,
) -> JArray {
    if m <= 0 || k1 <= 0 || k2 <= 0 || n <= 0 {
        return core::ptr::null_mut();
    }
    let (m, k1, k2, n) = (m as usize, k1 as usize, k2 as usize, n as usize);
    let a = unsafe { get_input_doubles(env, a) };
    let b = unsafe { get_input_doubles(env, b) };
    if a.len() != m.saturating_mul(k1) || b.len() != k2.saturating_mul(n) {
        return core::ptr::null_mut();
    }
    let mut out = vec![0.0f64; m * n];
    let status = unsafe {
        sillstone_tensor::ffi::sillstone_tensor_matmul_f64(
            a.as_ptr(),
            m,
            k1,
            b.as_ptr(),
            k2,
            n,
            out.as_mut_ptr(),
        )
    };
    if status != 0 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_double_array(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1tensor_1add_1f64(
    env: *mut JNIEnv,
    _class: *mut c_void,
    a: JArray,
    b: JArray,
) -> JArray {
    let a = unsafe { get_input_doubles(env, a) };
    let b = unsafe { get_input_doubles(env, b) };
    if a.is_empty() || a.len() != b.len() {
        return core::ptr::null_mut();
    }
    let mut out = vec![0.0f64; a.len()];
    let status = unsafe {
        sillstone_tensor::ffi::sillstone_tensor_add_f64(
            a.as_ptr(),
            b.as_ptr(),
            a.len(),
            out.as_mut_ptr(),
        )
    };
    if status != 0 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_double_array(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1tensor_1sum_1f64(
    env: *mut JNIEnv,
    _class: *mut c_void,
    a: JArray,
) -> f64 {
    let a = unsafe { get_input_doubles(env, a) };
    if a.is_empty() {
        return f64::NAN;
    }
    let mut total = 0.0f64;
    let status = unsafe {
        sillstone_tensor::ffi::sillstone_tensor_sum_f64(a.as_ptr(), a.len(), &mut total)
    };
    if status != 0 { f64::NAN } else { total }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_sillstone_Generated_sillstone_1image_1rotate_1size(
    env: *mut JNIEnv,
    _class: *mut c_void,
    in_w: jint,
    in_h: jint,
    angle: f64,
    expand: jint,
) -> JArray {
    let (mut w, mut h) = (0u32, 0u32);
    let status = unsafe {
        sillstone_image::ffi::sillstone_image_rotate_size(
            in_w as u32, in_h as u32, angle, expand as u32, &mut w, &mut h,
        )
    };
    if status != 0 || w > i32::MAX as u32 || h > i32::MAX as u32 {
        return core::ptr::null_mut();
    }
    unsafe { create_output_int_array(env, &[w as jint, h as jint]) }
}

unsafe fn create_output_int_array(env: *mut JNIEnv, data: &[jint]) -> JArray {
    // JNI function table: NewIntArray = 179, SetIntArrayRegion = 211.
    let table: &[usize; 256] = unsafe { &*(*env as *const [usize; 256]) };
    unsafe {
        let new: extern "system" fn(*mut JNIEnv, jint) -> JArray = core::mem::transmute(table[179]);
        let arr = new(env, data.len() as jint);
        if arr.is_null() {
            return core::ptr::null_mut();
        }
        let set: extern "system" fn(*mut JNIEnv, JArray, jint, jint, *const jint) =
            core::mem::transmute(table[211]);
        set(env, arr, 0, data.len() as jint, data.as_ptr());
        arr
    }
}
