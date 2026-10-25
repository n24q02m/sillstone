//! Fake-JNI-environment coverage for the glue in `src/ffi_jni.rs`.
//!
//! `src/ffi_jni.rs` is compiled out of the unit-test build (the
//! `#[no_mangle]` exports would collide with the unit-test binary), so
//! this integration test drives every export through an `unsafe extern`
//! declaration against a synthetic environment: a zeroed function
//! table whose slots the glue calls carry test-local implementations
//! backed by in-test buffers. The real-JVM proof is the Java suite
//! (`sdk/java`, `mvn test` against the built cdylib); this file keeps
//! the glue executed and visible to the coverage gate with zero new
//! dependencies (the suite's `check-zero-deps.py` gate forbids
//! registry crates, so the plain `std` mutexes stay unwrapped here).

#![allow(unsafe_code)]
// The JNI typedefs keep the jni.h spelling.
#![allow(non_camel_case_types)]

use core::ffi::c_void;
use std::sync::{LazyLock, Mutex};

use sillstone_image::ffi::{PITH_E_INVALID, PITH_LAYOUT_GRAY8, PITH_LAYOUT_RGB8, sillstone_image_phash};

type JNIEnv = *const FakeTable;
type JArray = *mut c_void;
type JIntArray = *mut c_void;
type JClass = *mut c_void;
type jbyte = i8;
type jint = i32;
type jlong = i64;

/// Mirror of `src/ffi_jni.rs`'s function table — the same slot
/// positions (`GetArrayLength` = 171, `GetByteArrayRegion` = 200,
/// `SetIntArrayRegion` = 211, four reserved pointers in the prefix).
#[repr(C)]
struct FakeTable {
    /// Slots 0..=170.
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

// The exported symbol under test (linked from the crate's rlib).
unsafe extern "system" {
    fn Java_hash_sillstone_image_PithImage_phashNative(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        width: jint,
        height: jint,
        layout: jint,
        status: JIntArray,
    ) -> jlong;
}

/// The state of one native call under test.
struct FakeCall {
    input: Vec<u8>,
    negative_length: bool,
    out_ints: Vec<i32>,
}

static CALL: LazyLock<Mutex<Option<FakeCall>>> = LazyLock::new(|| Mutex::new(None));
static SERIAL: Mutex<()> = Mutex::new(());

/// `GetArrayLength` (slot 171): the current input's length.
unsafe extern "system" fn fake_get_array_length(_env: *mut JNIEnv, _array: JArray) -> jint {
    let guard = CALL.lock().unwrap();
    let current = guard.as_ref().expect("no fake call state installed");
    if current.negative_length {
        -1
    } else {
        current.input.len() as jint
    }
}

/// `GetByteArrayRegion` (slot 200): copies the current input bytes.
unsafe extern "system" fn fake_get_byte_array_region(
    _env: *mut JNIEnv,
    _array: JArray,
    start: jint,
    len: jint,
    buf: *mut jbyte,
) {
    let guard = CALL.lock().unwrap();
    let current = guard.as_ref().expect("no fake call state installed");
    let start = start as usize;
    let bytes = &current.input[start..start + len as usize];
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf.cast(), bytes.len()) };
}

/// `SetIntArrayRegion` (slot 211): copies into the target array —
/// faithful to the JVM — and records the written values.
unsafe extern "system" fn fake_set_int_array_region(
    _env: *mut JNIEnv,
    array: JIntArray,
    _start: jint,
    len: jint,
    buf: *const jint,
) {
    unsafe { std::ptr::copy_nonoverlapping(buf, array as *mut jint, len as usize) };
    let mut guard = CALL.lock().unwrap();
    let current = guard.as_mut().expect("no fake call state installed");
    for i in 0..len as usize {
        current.out_ints.push(unsafe { *buf.add(i) });
    }
}

/// A zero-initialized `FakeTable`, leaked.
///
/// Raw `alloc_zeroed` bytes rather than `mem::zeroed`: the latter
/// runtime-refuses zeroed fn-pointer fields, while the former is just
/// memory — every slot the glue calls is assigned below before use.
fn zeroed_table() -> *mut FakeTable {
    let raw = unsafe { std::alloc::alloc_zeroed(std::alloc::Layout::new::<FakeTable>()) };
    assert!(!raw.is_null(), "alloc_zeroed failed");
    raw.cast::<FakeTable>()
}

/// Runs `f` against a synthetic environment backed by `input`,
/// returning its result and the status slots the glue wrote.
///
/// The big lock serializes sections across test threads: the fake
/// table callbacks address this one global call state.
fn with_fake_env<T>(input: Vec<u8>, negative_length: bool, f: impl FnOnce(*mut JNIEnv) -> T) -> T {
    let _serial = SERIAL.lock().unwrap();
    *CALL.lock().unwrap() = Some(FakeCall {
        input,
        negative_length,
        out_ints: Vec::new(),
    });

    let table = zeroed_table();
    unsafe {
        (*table).get_array_length = fake_get_array_length;
        (*table).get_byte_array_region = fake_get_byte_array_region;
        (*table).set_int_array_region = fake_set_int_array_region;
    }
    let functions: *const FakeTable = table;
    let env: *mut JNIEnv = Box::into_raw(Box::new(functions));

    let result = f(env);
    let state = CALL.lock().unwrap().take().expect("fake call state");
    drop(state);
    result
}

/// A live one-element status array; read it back with [`read_status`].
fn status_slot() -> JIntArray {
    Box::into_raw(Box::new([i32::MIN; 1])) as JIntArray
}

/// Reads the slot's content and releases it.
fn read_status(slot: JIntArray) -> i32 {
    let value = unsafe { *(slot as *mut jint) };
    unsafe { drop(Box::from_raw(slot as *mut jint)) };
    value
}

/// The committed oracle fixture (32×24 RGB8, the vector the Rust and
/// Java suites both pin).
fn base_444() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/phash/base_444.raw"
    ))
    .expect("fixture")
}

/// A non-null opaque array handle (the glue only checks nullness).
const SOME_ARRAY: JArray = 1usize as JArray;

#[test]
fn jni_phash_matches_the_pinned_oracle() {
    let dump = base_444();
    let (hash, status) = with_fake_env(dump, false, |env| unsafe {
        let slot = status_slot();
        let hash = Java_hash_sillstone_image_PithImage_phashNative(
            env,
            std::ptr::null_mut(),
            SOME_ARRAY,
            32,
            24,
            PITH_LAYOUT_RGB8 as jint,
            slot,
        );
        (hash, read_status(slot))
    });
    assert_eq!(status, 0, "status");
    assert_eq!(hash as u64, 0x858a_3f41_1ef5_0f78, "hash");
}

#[test]
fn jni_phash_agrees_with_the_c_export_on_a_gradient() {
    let dump: Vec<u8> = (0..8 * 8).map(|i| (i * 7 % 251) as u8).collect();
    let mut expected = 0u64;
    let code = unsafe {
        sillstone_image_phash(
            dump.as_ptr(),
            dump.len(),
            8,
            8,
            PITH_LAYOUT_GRAY8,
            &mut expected,
        )
    };
    assert_eq!(code, 0);
    let (hash, status) = with_fake_env(dump, false, |env| unsafe {
        let slot = status_slot();
        let hash = Java_hash_sillstone_image_PithImage_phashNative(
            env,
            std::ptr::null_mut(),
            SOME_ARRAY,
            8,
            8,
            PITH_LAYOUT_GRAY8 as jint,
            slot,
        );
        (hash, read_status(slot))
    });
    assert_eq!(status, 0, "status");
    assert_eq!(hash as u64, expected, "hash must match the C export");
}

#[test]
fn jni_null_data_array_is_invalid() {
    let dump = base_444();
    let (hash, status) = with_fake_env(dump, false, |env| unsafe {
        let slot = status_slot();
        let hash = Java_hash_sillstone_image_PithImage_phashNative(
            env,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            32,
            24,
            PITH_LAYOUT_RGB8 as jint,
            slot,
        );
        (hash, read_status(slot))
    });
    assert_eq!(status, PITH_E_INVALID, "status");
    assert_eq!(hash, 0, "hash");
}

#[test]
fn jni_unknown_layout_is_invalid() {
    let dump = base_444();
    let (hash, status) = with_fake_env(dump, false, |env| unsafe {
        let slot = status_slot();
        let hash = Java_hash_sillstone_image_PithImage_phashNative(
            env,
            std::ptr::null_mut(),
            SOME_ARRAY,
            32,
            24,
            99,
            slot,
        );
        (hash, read_status(slot))
    });
    assert_eq!(status, PITH_E_INVALID, "status");
    assert_eq!(hash, 0, "hash");
}

#[test]
fn jni_truncated_dump_is_invalid() {
    let dump = base_444();
    let (hash, status) = with_fake_env(dump[..dump.len() - 1].to_vec(), false, |env| unsafe {
        let slot = status_slot();
        let hash = Java_hash_sillstone_image_PithImage_phashNative(
            env,
            std::ptr::null_mut(),
            SOME_ARRAY,
            32,
            24,
            PITH_LAYOUT_RGB8 as jint,
            slot,
        );
        (hash, read_status(slot))
    });
    assert_eq!(status, PITH_E_INVALID, "status");
    assert_eq!(hash, 0, "hash");
}

#[test]
fn jni_negative_array_length_is_invalid() {
    let dump = base_444();
    let (hash, status) = with_fake_env(dump, true, |env| unsafe {
        let slot = status_slot();
        let hash = Java_hash_sillstone_image_PithImage_phashNative(
            env,
            std::ptr::null_mut(),
            SOME_ARRAY,
            32,
            24,
            PITH_LAYOUT_RGB8 as jint,
            slot,
        );
        (hash, read_status(slot))
    });
    assert_eq!(status, PITH_E_INVALID, "status");
    assert_eq!(hash, 0, "hash");
}

#[test]
fn jni_null_status_array_short_circuits() {
    let dump = base_444();
    let hash = with_fake_env(dump, false, |env| unsafe {
        Java_hash_sillstone_image_PithImage_phashNative(
            env,
            std::ptr::null_mut(),
            SOME_ARRAY,
            32,
            24,
            PITH_LAYOUT_RGB8 as jint,
            std::ptr::null_mut(),
        )
    });
    assert_eq!(hash, 0);
}

#[test]
fn jni_null_environment_short_circuits() {
    let dump = base_444();
    let (hash, status) = with_fake_env(dump, false, |_env| unsafe {
        let slot = status_slot();
        let hash = Java_hash_sillstone_image_PithImage_phashNative(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            SOME_ARRAY,
            32,
            24,
            PITH_LAYOUT_RGB8 as jint,
            slot,
        );
        (hash, read_status(slot))
    });
    assert_eq!(hash, 0);
    assert_eq!(status, i32::MIN, "status untouched without an environment");
}
