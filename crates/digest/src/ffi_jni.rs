//! The JNI surface of `sillstone-digest`: the
//! `Java_hash_sillstone_digest_PithDigest_*` exports the Java SDK
//! (`sdk/java`) binds its `native` methods through.
//!
//! The C ABI of [`crate::ffi`] is untouched: JNI requires exports
//! named `Java_<package>_<Class>_<method>`, so the Java-facing shims
//! live here and forward every call to the existing `sillstone_digest_*` C
//! exports — same status codes, same refusals, no second
//! implementation of any primitive. The module is compiled out of the
//! unit-test build (`#[cfg(all(not(test), feature = "std"))]` at the
//! registration site in `lib.rs`); the integration tests in
//! `tests/java_ffi.rs` exercise every export against a synthetic
//! environment so the coverage gate still sees the glue.
//!
//! JNI conventions of this module (the Java-side contract):
//!
//! * every export takes the JNI environment first and the receiving
//!   class second (the methods are static), then the Java arguments;
//! * the status code crosses back through a trailing one-element
//!   `int[]` — the same `PITH_OK` / `PITH_E_INVALID` /
//!   `PITH_E_REJECTED` values the C ABI returns;
//! * digests and base64 text cross back as a fresh `byte[]` — null
//!   unless the status is `PITH_OK`; base64 text is the RFC 4648
//!   ASCII bytes, so no UTF machinery is needed;
//! * scalar results (the u32 checksums, the u64 hashes) cross back as
//!   the plain `jint`/`jlong` return value, zero unless the status is
//!   `PITH_OK`;
//! * the splitmix64/xoshiro256** output sequences cross back as a
//!   fresh `long[]`; a null input array maps to `PITH_E_INVALID`,
//!   mirroring the C ABI's null-pointer rule; a null environment or
//!   status array short-circuits to a zero return without touching
//!   memory.
//!
//! The suite is zero-third-party (CI's `check-zero-deps.py` fails any
//! registry crate), so the JNI function table is hand-declared below:
//! every slot is pointer-sized and the positions are the fixed
//! `JNINativeInterface_` member order of `jni.h`. The slot indices
//! were parsed mechanically from the JDK 21 header and are validated
//! end-to-end against a live JVM every time the Java suite runs.

#![allow(unsafe_code)]
// The JNI typedefs keep the jni.h spelling (jint, jbyte, ...).
#![allow(non_camel_case_types)]

use core::ffi::c_void;

use crate::ffi::{
    PITH_E_INVALID, PITH_E_REJECTED, PITH_OK, sillstone_digest_adler32, sillstone_digest_base64_decode,
    sillstone_digest_base64_encode, sillstone_digest_crc32, sillstone_digest_crc32c, sillstone_digest_fnv1a64,
    sillstone_digest_hmac_sha1, sillstone_digest_hmac_sha256, sillstone_digest_hmac_sha512,
    sillstone_digest_murmur3_x64_128, sillstone_digest_sha1, sillstone_digest_sha256, sillstone_digest_sha512,
    sillstone_digest_splitmix64_fill, sillstone_digest_xoshiro256_fill, sillstone_digest_xxh64,
};

/// A JNI environment handle — C-mode `JNIEnv*`, a pointer to the
/// function table.
type JNIEnv = *const JniTable;

/// Any Java array reference; the glue only checks nullness before
/// handing arrays through the table.
type JArray = *mut c_void;

/// A Java `int[]` reference.
type JIntArray = *mut c_void;

/// A Java class object reference (static methods receive the class).
type JClass = *mut c_void;

/// `jbyte` per `jni.h`.
type jbyte = i8;
/// `jint`/`jsize` per `jni.h`.
type jint = i32;
/// `jlong` per `jni.h`.
type jlong = i64;

/// The JNI function-table slots this module calls.
///
/// Underscore-prefixed gap fields hold the slots between the used ones
/// (slot = field position; the four reserved pointers are part of the
/// prefix). Slot indices parsed from the JDK 21 `include/jni.h`:
/// `GetArrayLength` = 171, `NewByteArray` = 176, `NewLongArray` = 180,
/// `GetByteArrayRegion` = 200, `SetByteArrayRegion` = 208,
/// `SetIntArrayRegion` = 211, `SetLongArrayRegion` = 212.
#[repr(C)]
struct JniTable {
    /// Slots 0..=170: the four reserved pointers through
    /// `ReleaseStringUTFChars`.
    _prefix: [*mut c_void; 171],
    /// Slot 171.
    get_array_length: unsafe extern "system" fn(env: *mut JNIEnv, array: JArray) -> jint,
    /// Slots 172..=175.
    _gap_before_new_byte_array: [*mut c_void; 4],
    /// Slot 176.
    new_byte_array: unsafe extern "system" fn(env: *mut JNIEnv, len: jint) -> JArray,
    /// Slots 177..=179.
    _gap_before_new_long_array: [*mut c_void; 3],
    /// Slot 180.
    new_long_array: unsafe extern "system" fn(env: *mut JNIEnv, len: jint) -> JArray,
    /// Slots 181..=199.
    _gap_before_byte_region: [*mut c_void; 19],
    /// Slot 200.
    get_byte_array_region: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: JArray,
        start: jint,
        len: jint,
        buf: *mut jbyte,
    ),
    /// Slots 201..=207.
    _gap_before_set_byte_region: [*mut c_void; 7],
    /// Slot 208.
    set_byte_array_region: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: JArray,
        start: jint,
        len: jint,
        buf: *const jbyte,
    ),
    /// Slots 209..=210.
    _gap_before_set_int_region: [*mut c_void; 2],
    /// Slot 211.
    set_int_array_region: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: JIntArray,
        start: jint,
        len: jint,
        buf: *const jint,
    ),
    /// Slot 212.
    set_long_array_region: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: JArray,
        start: jint,
        len: jint,
        buf: *const jlong,
    ),
}

/// The function table behind an environment handle.
///
/// # Safety
///
/// `env` must be a live JNI environment pointer.
unsafe fn table<'a>(env: *mut JNIEnv) -> &'a JniTable {
    // `env` points at the function-table pointer (C-mode `JNIEnv*`):
    // deref twice to reach the table itself.
    unsafe { &**env }
}

/// Copies a Java `byte[]` through the environment into an owned
/// buffer.
///
/// # Safety
///
/// `env` must be a live JNI environment and `array` a live `byte[]`
/// reference for the duration of the call; a null array is
/// [`PITH_E_INVALID`], mirroring the C ABI's null-pointer rule.
unsafe fn java_bytes(env: *mut JNIEnv, array: JArray) -> Result<Vec<u8>, i32> {
    if array.is_null() {
        return Err(PITH_E_INVALID);
    }
    let functions = unsafe { table(env) };
    let len = unsafe { (functions.get_array_length)(env, array) };
    if len < 0 {
        return Err(PITH_E_INVALID);
    }
    let mut bytes = vec![0u8; len as usize];
    unsafe { (functions.get_byte_array_region)(env, array, 0, len, bytes.as_mut_ptr().cast()) };
    Ok(bytes)
}

/// Builds a fresh Java `byte[]` holding `bytes`, or `None` if the
/// environment refuses the allocation.
///
/// # Safety
///
/// `env` must be a live JNI environment.
unsafe fn new_java_bytes(env: *mut JNIEnv, bytes: &[u8]) -> Option<JArray> {
    let functions = unsafe { table(env) };
    let array = unsafe { (functions.new_byte_array)(env, bytes.len() as jint) };
    if array.is_null() {
        return None;
    }
    unsafe {
        (functions.set_byte_array_region)(env, array, 0, bytes.len() as jint, bytes.as_ptr().cast())
    };
    Some(array)
}

/// Builds a fresh Java `long[]` holding `values`, or `None` if the
/// environment refuses the allocation.
///
/// # Safety
///
/// `env` must be a live JNI environment.
unsafe fn new_java_longs(env: *mut JNIEnv, values: &[u64]) -> Option<JArray> {
    let functions = unsafe { table(env) };
    let array = unsafe { (functions.new_long_array)(env, values.len() as jint) };
    if array.is_null() {
        return None;
    }
    let longs: Vec<i64> = values.iter().map(|&value| value as i64).collect();
    unsafe {
        (functions.set_long_array_region)(env, array, 0, longs.len() as jint, longs.as_ptr())
    };
    Some(array)
}

/// Writes `value` into the one-element `int[]` status slot.
///
/// # Safety
///
/// `status` must be a live `int[]` of length ≥ 1 (checked by the
/// caller).
unsafe fn set_status(env: *mut JNIEnv, status: JIntArray, value: jint) {
    let functions = unsafe { table(env) };
    unsafe { (functions.set_int_array_region)(env, status, 0, 1, &value) };
}

/// Shared body of the fixed-width digest exports: copies `data`
/// through the environment once, calls `produce` with a stack
/// out-buffer of `LEN` bytes and hands the written bytes back as a
/// fresh `byte[]`.
///
/// # Safety
///
/// `env` must be a live JNI environment and `status` a live `int[]`.
unsafe fn digest_in<const LEN: usize>(
    env: *mut JNIEnv,
    status: JIntArray,
    data: JArray,
    produce: impl FnOnce(&[u8], *mut u8) -> i32,
) -> JArray {
    if env.is_null() || status.is_null() {
        return core::ptr::null_mut();
    }
    let bytes = match unsafe { java_bytes(env, data) } {
        Ok(bytes) => bytes,
        Err(status_code) => {
            unsafe { set_status(env, status, status_code) };
            return core::ptr::null_mut();
        }
    };
    let mut out = [0u8; LEN];
    let code = produce(&bytes, out.as_mut_ptr());
    if code != PITH_OK {
        unsafe { set_status(env, status, code) };
        return core::ptr::null_mut();
    }
    match unsafe { new_java_bytes(env, &out) } {
        Some(array) => {
            unsafe { set_status(env, status, PITH_OK) };
            array
        }
        None => {
            // The only failure left is the environment refusing the
            // array allocation; there is no dedicated code for it, so
            // it surfaces as a rejection, never as a panic.
            unsafe { set_status(env, status, PITH_E_REJECTED) };
            core::ptr::null_mut()
        }
    }
}

/// Shared body of the scalar exports: copies `data` through the
/// environment once and hands it to `produce`; the status crosses
/// through `status` and `PITH_E_INVALID` is returned when the
/// environment or status slot is null.
///
/// # Safety
///
/// `env` must be a live JNI environment and `status` a live `int[]`.
unsafe fn with_data(
    env: *mut JNIEnv,
    status: JIntArray,
    data: JArray,
    produce: impl FnOnce(&[u8]) -> i32,
) -> i32 {
    if env.is_null() || status.is_null() {
        return PITH_E_INVALID;
    }
    let bytes = match unsafe { java_bytes(env, data) } {
        Ok(bytes) => bytes,
        Err(status_code) => {
            unsafe { set_status(env, status, status_code) };
            return PITH_E_INVALID;
        }
    };
    produce(&bytes)
}

/// Shared body of the two seeded `long[]` fills.
///
/// # Safety
///
/// `env` must be a live JNI environment and `status` a live `int[]`.
unsafe fn longs_out(
    env: *mut JNIEnv,
    status: JIntArray,
    seed: jlong,
    count: jint,
    fill: unsafe extern "C" fn(u64, *mut u64, usize) -> i32,
) -> JArray {
    if env.is_null() || status.is_null() {
        return core::ptr::null_mut();
    }
    if count < 0 {
        unsafe { set_status(env, status, PITH_E_INVALID) };
        return core::ptr::null_mut();
    }
    let mut out = vec![0u64; count as usize];
    let code = unsafe { fill(seed as u64, out.as_mut_ptr(), out.len()) };
    if code != PITH_OK {
        unsafe { set_status(env, status, code) };
        return core::ptr::null_mut();
    }
    match unsafe { new_java_longs(env, &out) } {
        Some(array) => {
            unsafe { set_status(env, status, PITH_OK) };
            array
        }
        None => {
            unsafe { set_status(env, status, PITH_E_REJECTED) };
            core::ptr::null_mut()
        }
    }
}

/// The Java binding of [`sillstone_digest_sha256`]: the 32-byte digest.
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
//
// Private: the JVM links the export by symbol name; a public Rust
// signature over the private table type would trip
// `private_interfaces`.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_sha256Native(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    status: JIntArray,
) -> JArray {
    unsafe {
        digest_in::<32>(env, status, data, |bytes, out| {
            sillstone_digest_sha256(bytes.as_ptr(), bytes.len(), out)
        })
    }
}

/// The Java binding of [`sillstone_digest_sha1`]: the 20-byte digest.
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_sha1Native(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    status: JIntArray,
) -> JArray {
    unsafe {
        digest_in::<20>(env, status, data, |bytes, out| {
            sillstone_digest_sha1(bytes.as_ptr(), bytes.len(), out)
        })
    }
}

/// The Java binding of [`sillstone_digest_sha512`]: the 64-byte digest.
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_sha512Native(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    status: JIntArray,
) -> JArray {
    unsafe {
        digest_in::<64>(env, status, data, |bytes, out| {
            sillstone_digest_sha512(bytes.as_ptr(), bytes.len(), out)
        })
    }
}

/// Shared body of the three (key, data) HMAC exports.
///
/// # Safety
///
/// `env` must be a live JNI environment and `key`/`data`/`status`
/// live Java array references for the duration of the call.
unsafe fn hmac_out<const LEN: usize>(
    env: *mut JNIEnv,
    status: JIntArray,
    key: JArray,
    data: JArray,
    hmac: unsafe extern "C" fn(*const u8, usize, *const u8, usize, *mut u8) -> i32,
) -> JArray {
    if env.is_null() || status.is_null() {
        return core::ptr::null_mut();
    }
    let key_bytes = match unsafe { java_bytes(env, key) } {
        Ok(bytes) => bytes,
        Err(status_code) => {
            unsafe { set_status(env, status, status_code) };
            return core::ptr::null_mut();
        }
    };
    let data_bytes = match unsafe { java_bytes(env, data) } {
        Ok(bytes) => bytes,
        Err(status_code) => {
            unsafe { set_status(env, status, status_code) };
            return core::ptr::null_mut();
        }
    };
    let mut out = [0u8; LEN];
    let code = unsafe {
        hmac(
            key_bytes.as_ptr(),
            key_bytes.len(),
            data_bytes.as_ptr(),
            data_bytes.len(),
            out.as_mut_ptr(),
        )
    };
    if code != PITH_OK {
        unsafe { set_status(env, status, code) };
        return core::ptr::null_mut();
    }
    match unsafe { new_java_bytes(env, &out) } {
        Some(array) => {
            unsafe { set_status(env, status, PITH_OK) };
            array
        }
        None => {
            unsafe { set_status(env, status, PITH_E_REJECTED) };
            core::ptr::null_mut()
        }
    }
}

/// The Java binding of [`sillstone_digest_hmac_sha1`].
///
/// # Safety
///
/// `env` must be a live JNI environment and `key`/`data`/`status`
/// live Java array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_hmacSha1Native(
    env: *mut JNIEnv,
    _class: JClass,
    key: JArray,
    data: JArray,
    status: JIntArray,
) -> JArray {
    unsafe { hmac_out::<20>(env, status, key, data, sillstone_digest_hmac_sha1) }
}

/// The Java binding of [`sillstone_digest_hmac_sha256`].
///
/// # Safety
///
/// `env` must be a live JNI environment and `key`/`data`/`status`
/// live Java array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_hmacSha256Native(
    env: *mut JNIEnv,
    _class: JClass,
    key: JArray,
    data: JArray,
    status: JIntArray,
) -> JArray {
    unsafe { hmac_out::<32>(env, status, key, data, sillstone_digest_hmac_sha256) }
}

/// The Java binding of [`sillstone_digest_hmac_sha512`].
///
/// # Safety
///
/// `env` must be a live JNI environment and `key`/`data`/`status`
/// live Java array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_hmacSha512Native(
    env: *mut JNIEnv,
    _class: JClass,
    key: JArray,
    data: JArray,
    status: JIntArray,
) -> JArray {
    unsafe { hmac_out::<64>(env, status, key, data, sillstone_digest_hmac_sha512) }
}

/// Shared body of the u32-checksum exports: the value crosses back as
/// the `jint` return, zero unless the status is `PITH_OK`.
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
unsafe fn checksum32_out(
    env: *mut JNIEnv,
    status: JIntArray,
    data: JArray,
    sum: unsafe extern "C" fn(*const u8, usize, *mut u32) -> i32,
) -> jint {
    let mut out: u32 = 0;
    if unsafe {
        with_data(env, status, data, |bytes| {
            sum(bytes.as_ptr(), bytes.len(), &mut out)
        })
    } != PITH_OK
    {
        return 0;
    }
    out as jint
}

/// The Java binding of [`sillstone_digest_crc32`].
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_crc32Native(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    status: JIntArray,
) -> jint {
    unsafe { checksum32_out(env, status, data, sillstone_digest_crc32) }
}

/// The Java binding of [`sillstone_digest_adler32`].
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_adler32Native(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    status: JIntArray,
) -> jint {
    unsafe { checksum32_out(env, status, data, sillstone_digest_adler32) }
}

/// The Java binding of [`sillstone_digest_crc32c`].
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_crc32cNative(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    status: JIntArray,
) -> jint {
    unsafe { checksum32_out(env, status, data, sillstone_digest_crc32c) }
}

/// Shared body of the u64-hash exports: the value crosses back as the
/// `jlong` return, zero unless the status is `PITH_OK`.
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
unsafe fn hash64_out(
    env: *mut JNIEnv,
    status: JIntArray,
    data: JArray,
    hash: unsafe extern "C" fn(*const u8, usize, *mut u64) -> i32,
) -> jlong {
    let mut out: u64 = 0;
    if unsafe {
        with_data(env, status, data, |bytes| {
            hash(bytes.as_ptr(), bytes.len(), &mut out)
        })
    } != PITH_OK
    {
        return 0;
    }
    out as jlong
}

/// The Java binding of [`sillstone_digest_fnv1a64`].
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_fnv1a64Native(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    status: JIntArray,
) -> jlong {
    unsafe { hash64_out(env, status, data, sillstone_digest_fnv1a64) }
}

/// The Java binding of [`sillstone_digest_xxh64`].
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_xxh64Native(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    seed: jlong,
    status: JIntArray,
) -> jlong {
    let mut out: u64 = 0;
    if unsafe {
        with_data(env, status, data, |bytes| {
            sillstone_digest_xxh64(bytes.as_ptr(), bytes.len(), seed as u64, &mut out)
        })
    } != PITH_OK
    {
        return 0;
    }
    out as jlong
}

/// The Java binding of [`sillstone_digest_murmur3_x64_128`].
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_murmur3X64128Native(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    seed: jint,
    status: JIntArray,
) -> JArray {
    unsafe {
        digest_in::<16>(env, status, data, |bytes, out| {
            sillstone_digest_murmur3_x64_128(bytes.as_ptr(), bytes.len(), seed as u32, out)
        })
    }
}

/// The Java binding of [`sillstone_digest_splitmix64_fill`].
///
/// # Safety
///
/// `env` must be a live JNI environment and `status` a live `int[]`.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_splitmix64FillNative(
    env: *mut JNIEnv,
    _class: JClass,
    seed: jlong,
    count: jint,
    status: JIntArray,
) -> JArray {
    unsafe { longs_out(env, status, seed, count, sillstone_digest_splitmix64_fill) }
}

/// The Java binding of [`sillstone_digest_xoshiro256_fill`].
///
/// # Safety
///
/// `env` must be a live JNI environment and `status` a live `int[]`.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_xoshiro256FillNative(
    env: *mut JNIEnv,
    _class: JClass,
    seed: jlong,
    count: jint,
    status: JIntArray,
) -> JArray {
    unsafe { longs_out(env, status, seed, count, sillstone_digest_xoshiro256_fill) }
}

/// The Java binding of [`sillstone_digest_base64_encode`]: the encoded
/// text as its RFC 4648 ASCII bytes.
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_base64EncodeNative(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    status: JIntArray,
) -> JArray {
    if env.is_null() || status.is_null() {
        return core::ptr::null_mut();
    }
    let bytes = match unsafe { java_bytes(env, data) } {
        Ok(bytes) => bytes,
        Err(status_code) => {
            unsafe { set_status(env, status, status_code) };
            return core::ptr::null_mut();
        }
    };
    let mut out = vec![0u8; crate::base64::encoded_len(bytes.len())];
    let mut out_len: usize = 0;
    let code = unsafe {
        sillstone_digest_base64_encode(
            bytes.as_ptr(),
            bytes.len(),
            out.as_mut_ptr(),
            out.len(),
            &mut out_len,
        )
    };
    if code != PITH_OK {
        unsafe { set_status(env, status, code) };
        return core::ptr::null_mut();
    }
    match unsafe { new_java_bytes(env, &out[..out_len]) } {
        Some(array) => {
            unsafe { set_status(env, status, PITH_OK) };
            array
        }
        None => {
            unsafe { set_status(env, status, PITH_E_REJECTED) };
            core::ptr::null_mut()
        }
    }
}

/// The Java binding of [`sillstone_digest_base64_decode`]: `data` is the
/// canonical padded base64 text as its ASCII bytes.
///
/// # Safety
///
/// `env` must be a live JNI environment and `data`/`status` live Java
/// array references for the duration of the call.
//
// Private: the JVM links the export by symbol name.
#[unsafe(no_mangle)]
unsafe extern "system" fn Java_hash_sillstone_digest_PithDigest_base64DecodeNative(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    status: JIntArray,
) -> JArray {
    if env.is_null() || status.is_null() {
        return core::ptr::null_mut();
    }
    let bytes = match unsafe { java_bytes(env, data) } {
        Ok(bytes) => bytes,
        Err(status_code) => {
            unsafe { set_status(env, status, status_code) };
            return core::ptr::null_mut();
        }
    };
    let mut out = vec![0u8; bytes.len() / 4 * 3];
    let mut out_len: usize = 0;
    let code = unsafe {
        sillstone_digest_base64_decode(
            bytes.as_ptr(),
            bytes.len(),
            out.as_mut_ptr(),
            out.len(),
            &mut out_len,
        )
    };
    if code != PITH_OK {
        unsafe { set_status(env, status, code) };
        return core::ptr::null_mut();
    }
    match unsafe { new_java_bytes(env, &out[..out_len]) } {
        Some(array) => {
            unsafe { set_status(env, status, PITH_OK) };
            array
        }
        None => {
            unsafe { set_status(env, status, PITH_E_REJECTED) };
            core::ptr::null_mut()
        }
    }
}
