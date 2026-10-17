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
