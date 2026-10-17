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
