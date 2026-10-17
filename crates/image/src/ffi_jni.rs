//! The JNI surface of `sillstone-image`: the `Java_hash_sillstone_image_PithImage_*`
//! exports the Java SDK (`sdk/java`) binds its `native` methods through.
//!
//! The C ABI of [`crate::ffi`] is untouched: JNI requires exports named
//! `Java_<package>_<Class>_<method>`, so the Java-facing shims live here
//! and forward every call to the existing `pith_*` C export — same
//! status codes, same refusals, no second implementation of the
//! pipeline. The module is compiled out of the unit-test build
//! (`#[cfg(not(test))]` at the registration site in `lib.rs`); the
//! integration tests in `tests/java_ffi.rs` exercise the export against
//! a synthetic environment so the coverage gate still sees the glue.
//!
//! JNI conventions of this module (the Java-side contract):
//!
//! * the export takes the JNI environment first and the receiving
//!   class second (the methods are static), then the Java arguments;
//! * the status code crosses back through a trailing one-element
//!   `int[]` — the same `PITH_OK` / `PITH_E_INVALID` values the C ABI
//!   returns;
//! * the hash crosses back bit-cast to `jlong` (0 unless the status is
//!   `PITH_OK`);
//! * a null `data` array maps to `PITH_E_INVALID` exactly as the C ABI
//!   maps a null pointer; a null environment or status array
//!   short-circuits to a zero return without touching memory (both are
//!   unreachable through the Java wrapper, which always passes live
//!   arrays from a live JVM).
//!
//! The suite is zero-third-party (CI's `check-zero-deps.py` fails any
//! registry crate), so the JNI function table is hand-declared below:
//! every slot is pointer-sized and the positions are the fixed
//! `JNINativeInterface_` member order of `jni.h`. The slot indices were
//! parsed mechanically from the JDK 21 header and are validated
//! end-to-end against a live JVM every time the Java suite runs.

#![allow(unsafe_code)]
// The JNI typedefs keep the jni.h spelling (jint, jbyte, ...).
#![allow(non_camel_case_types)]

use core::ffi::c_void;

use crate::ffi::{PITH_E_INVALID, PITH_OK, sillstone_image_phash};

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
/// `GetArrayLength` = 171, `GetByteArrayRegion` = 200,
/// `SetIntArrayRegion` = 211.
#[repr(C)]
struct JniTable {
    /// Slots 0..=170: the four reserved pointers through
    /// `ReleaseStringUTFChars`.
    _prefix: [*mut c_void; 171],
    /// Slot 171.
    get_array_length: unsafe extern "system" fn(env: *mut JNIEnv, array: JArray) -> jint,
    /// Slots 172..=199.
    _gap_before_byte_region: [*mut c_void; 28],
    /// Slot 200.
    get_byte_array_region: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: JArray,
        start: jint,
        len: jint,
        buf: *mut jbyte,
    ),
    /// Slots 201..=210.
    _gap_before_int_region: [*mut c_void; 10],
    /// Slot 211.
    set_int_array_region: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: JIntArray,
        start: jint,
        len: jint,
        buf: *const jint,
    ),
}

/// The JNI 1.1-era function table behind an environment handle.
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

/// The Java binding of [`sillstone_image_phash`]: the 64-bit perceptual
/// hash of a raw pixel dump.
///
/// `data` is the flat, row-major, channel-interleaved dump of a
/// `width`×`height` image; `layout` is one of the Java `LAYOUT_*`
/// codes — the same wire codes as the C ABI. The hash crosses back
/// bit-cast to `jlong`, and the status (`PITH_OK`, or
/// `PITH_E_INVALID` for a null array, an unknown layout code or a
/// geometry mismatch) through `status[0]`.
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
unsafe extern "system" fn Java_hash_sillstone_image_PithImage_phashNative(
    env: *mut JNIEnv,
    _class: JClass,
    data: JArray,
    width: jint,
    height: jint,
    layout: jint,
    status: JIntArray,
) -> jlong {
    if env.is_null() || status.is_null() {
        return 0;
    }
    let bytes = match unsafe { java_bytes(env, data) } {
        Ok(bytes) => bytes,
        Err(status_code) => {
            unsafe { set_status(env, status, status_code) };
            return 0;
        }
    };
    let mut hash: u64 = 0;
    let code = unsafe {
        sillstone_image_phash(
            bytes.as_ptr(),
            bytes.len(),
            width as u32,
            height as u32,
            layout as u32,
            &mut hash,
        )
    };
    unsafe { set_status(env, status, code) };
    if code == PITH_OK { hash as jlong } else { 0 }
}
