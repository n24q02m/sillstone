//! Fake-JNI-environment coverage for the glue in `src/ffi_jni.rs`.
//!
//! `src/ffi_jni.rs` is compiled out of the unit-test build (the
//! `#[no_mangle]` exports would collide with the unit-test binary), so
//! this integration test drives every export through an `unsafe extern`
//! declaration against a synthetic environment: a zeroed function
//! table whose slots the glue calls carry test-local implementations
//! backed by a registry of fake Java arrays. The real-JVM proof is the
//! Java suite (`sdk/java`, `mvn test` against the built cdylib); this
//! file keeps the glue executed and visible to the coverage gate with
//! zero new dependencies (the suite's `check-zero-deps.py` gate forbids
//! registry crates, so the plain `std` mutexes stay unwrapped here).

#![allow(unsafe_code)]
// The JNI typedefs keep the jni.h spelling.
#![allow(non_camel_case_types)]

use core::ffi::c_void;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use sillstone_digest::{
    Digest, SplitMix64, Xoshiro256StarStar, adler32, crc32, crc32c, fnv1a64, hmac_sha1,
    hmac_sha256, hmac_sha512, murmur3_x64_128, sha1, sha256, sha512, xxh64,
};

type JNIEnv = *const FakeTable;
type JArray = *mut c_void;
type JIntArray = *mut c_void;
type JClass = *mut c_void;
type jbyte = i8;
type jint = i32;
type jlong = i64;

/// Mirror of `src/ffi_jni.rs`'s function table — the same slot
/// positions (171, 176, 180, 200, 208, 211, 212; four reserved
/// pointers in the prefix).
#[repr(C)]
struct FakeTable {
    /// Slots 0..=170.
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

// The exported symbols under test (linked from the crate's rlib).
unsafe extern "system" {
    fn Java_hash_sillstone_digest_PithDigest_sha256Native(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        status: JIntArray,
    ) -> JArray;

    fn Java_hash_sillstone_digest_PithDigest_sha1Native(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        status: JIntArray,
    ) -> JArray;

    fn Java_hash_sillstone_digest_PithDigest_sha512Native(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        status: JIntArray,
    ) -> JArray;

    fn Java_hash_sillstone_digest_PithDigest_hmacSha1Native(
        env: *mut JNIEnv,
        class: JClass,
        key: JArray,
        data: JArray,
        status: JIntArray,
    ) -> JArray;

    fn Java_hash_sillstone_digest_PithDigest_hmacSha256Native(
        env: *mut JNIEnv,
        class: JClass,
        key: JArray,
        data: JArray,
        status: JIntArray,
    ) -> JArray;

    fn Java_hash_sillstone_digest_PithDigest_hmacSha512Native(
        env: *mut JNIEnv,
        class: JClass,
        key: JArray,
        data: JArray,
        status: JIntArray,
    ) -> JArray;

    fn Java_hash_sillstone_digest_PithDigest_crc32Native(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        status: JIntArray,
    ) -> jint;

    fn Java_hash_sillstone_digest_PithDigest_adler32Native(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        status: JIntArray,
    ) -> jint;
    fn Java_hash_sillstone_digest_PithDigest_fnv1a64Native(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        status: JIntArray,
    ) -> jlong;
    fn Java_hash_sillstone_digest_PithDigest_crc32cNative(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        status: JIntArray,
    ) -> jint;

    fn Java_hash_sillstone_digest_PithDigest_xxh64Native(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        seed: jlong,
        status: JIntArray,
    ) -> jlong;

    fn Java_hash_sillstone_digest_PithDigest_murmur3X64128Native(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        seed: jint,
        status: JIntArray,
    ) -> JArray;

    fn Java_hash_sillstone_digest_PithDigest_splitmix64FillNative(
        env: *mut JNIEnv,
        class: JClass,
        seed: jlong,
        count: jint,
        status: JIntArray,
    ) -> JArray;

    fn Java_hash_sillstone_digest_PithDigest_xoshiro256FillNative(
        env: *mut JNIEnv,
        class: JClass,
        seed: jlong,
        count: jint,
        status: JIntArray,
    ) -> JArray;

    fn Java_hash_sillstone_digest_PithDigest_base64EncodeNative(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        status: JIntArray,
    ) -> JArray;

    fn Java_hash_sillstone_digest_PithDigest_base64DecodeNative(
        env: *mut JNIEnv,
        class: JClass,
        data: JArray,
        status: JIntArray,
    ) -> JArray;
}

/// The state of one native call under test.
struct FakeCall {
    byte_arrays: HashMap<usize, Vec<u8>>,
    long_arrays: HashMap<usize, Vec<i64>>,
    next_handle: usize,
    out_bytes: Vec<u8>,
    out_ints: Vec<i32>,
    out_longs: Vec<i64>,
    fail_new_array: bool,
}

static CALL: LazyLock<Mutex<Option<FakeCall>>> = LazyLock::new(|| Mutex::new(None));
static SERIAL: Mutex<()> = Mutex::new(());

fn with_state<T>(f: impl FnOnce(&mut FakeCall) -> T) -> T {
    let mut guard = CALL.lock().unwrap();
    let state = guard.as_mut().expect("no fake call state installed");
    f(state)
}

/// Registers a fake `byte[]`; returns its opaque handle.
fn byte_array(bytes: Vec<u8>) -> JArray {
    with_state(|state| {
        state.next_handle += 8;
        let handle = state.next_handle;
        state.byte_arrays.insert(handle, bytes);
        handle as JArray
    })
}

/// `GetArrayLength` (slot 171): byte arrays report their byte length;
/// long arrays report their element count, the JVM behavior the glue
/// relies on for `new_java_longs` (it passes the value straight to
/// `NewLongArray`).
unsafe extern "system" fn fake_get_array_length(_env: *mut JNIEnv, array: JArray) -> jint {
    with_state(|state| {
        if let Some(bytes) = state.byte_arrays.get(&(array as usize)) {
            bytes.len() as jint
        } else if let Some(longs) = state.long_arrays.get(&(array as usize)) {
            longs.len() as jint
        } else {
            -1
        }
    })
}

/// `NewByteArray` (slot 176).
unsafe extern "system" fn fake_new_byte_array(_env: *mut JNIEnv, len: jint) -> JArray {
    let fresh = with_state(|state| {
        if state.fail_new_array {
            return None;
        }
        Some(alloc_byte_array(state, vec![0; len as usize]))
    });
    fresh.unwrap_or_else(std::ptr::null_mut)
}

/// `NewLongArray` (slot 180).
unsafe extern "system" fn fake_new_long_array(_env: *mut JNIEnv, len: jint) -> JArray {
    let fresh = with_state(|state| {
        if state.fail_new_array {
            return None;
        }
        state.next_handle += 8;
        let handle = state.next_handle;
        state.long_arrays.insert(handle, vec![0; len as usize]);
        Some(handle as JArray)
    });
    fresh.unwrap_or_else(std::ptr::null_mut)
}

/// Registers `bytes` as a fresh `byte[]` behind `state`.
fn alloc_byte_array(state: &mut FakeCall, bytes: Vec<u8>) -> JArray {
    state.next_handle += 8;
    let handle = state.next_handle;
    state.byte_arrays.insert(handle, bytes);
    handle as JArray
}

/// `GetByteArrayRegion` (slot 200).
unsafe extern "system" fn fake_get_byte_array_region(
    _env: *mut JNIEnv,
    array: JArray,
    start: jint,
    len: jint,
    buf: *mut jbyte,
) {
    let bytes = with_state(|state| {
        state
            .byte_arrays
            .get(&(array as usize))
            .expect("live array")[start as usize..start as usize + len as usize]
            .to_vec()
    });
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf.cast(), bytes.len()) };
}

/// `SetByteArrayRegion` (slot 208): copies into the target array —
/// faithful to the JVM — and records the written bytes.
unsafe extern "system" fn fake_set_byte_array_region(
    _env: *mut JNIEnv,
    array: JArray,
    start: jint,
    len: jint,
    buf: *const jbyte,
) {
    let written = unsafe { std::slice::from_raw_parts(buf.cast::<u8>(), len as usize) }.to_vec();
    with_state(|state| {
        let target = state
            .byte_arrays
            .get_mut(&(array as usize))
            .expect("live array");
        target[start as usize..start as usize + len as usize].copy_from_slice(&written);
        state.out_bytes.extend_from_slice(&written);
    });
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
    let written = unsafe { std::slice::from_raw_parts(buf, len as usize) }.to_vec();
    if !array.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(buf, array as *mut jint, len as usize) };
    }
    with_state(|state| {
        state.out_ints.extend_from_slice(&written);
        for value in &written {
            state
                .long_arrays
                .entry(array as usize)
                .or_default()
                .push(*value as i64);
        }
    });
}

/// `SetLongArrayRegion` (slot 212): copies into the target array —
/// faithful to the JVM — and records the written values.
unsafe extern "system" fn fake_set_long_array_region(
    _env: *mut JNIEnv,
    array: JArray,
    start: jint,
    len: jint,
    buf: *const jlong,
) {
    let written = unsafe { std::slice::from_raw_parts(buf, len as usize) }.to_vec();
    with_state(|state| {
        let target = state
            .long_arrays
            .get_mut(&(array as usize))
            .expect("live array");
        target[start as usize..start as usize + len as usize].copy_from_slice(&written);
        state.out_longs.extend_from_slice(&written);
    });
}

/// A zero-initialized `FakeTable`, leaked.
///
/// Raw `alloc_zeroed` bytes rather than `mem::zeroed`: the latter
/// runtime-refuses zeroed fn-pointer fields, while the former is just
/// memory — every slot the glue calls is assigned below before use.
fn zeroed_table() -> *mut FakeTable {
    let raw = unsafe { alloc_zeroed_fake_table() };
    assert!(!raw.is_null(), "alloc_zeroed failed");
    raw
}

/// `alloc_zeroed` behind a helper so the unsafe block stays tiny.
unsafe fn alloc_zeroed_fake_table() -> *mut FakeTable {
    unsafe { std::alloc::alloc_zeroed(std::alloc::Layout::new::<FakeTable>()).cast::<FakeTable>() }
}

/// Runs `f` against a synthetic environment; returns its result, the
/// bytes/values the glue wrote into Java arrays, and the int slots it
/// wrote.
fn with_fake_env<T>(f: impl FnOnce(*mut JNIEnv) -> T) -> (T, Vec<u8>, Vec<i32>, Vec<i64>) {
    let _serial = SERIAL.lock().unwrap();
    *CALL.lock().unwrap() = Some(FakeCall {
        byte_arrays: HashMap::new(),
        long_arrays: HashMap::new(),
        next_handle: 0,
        out_bytes: Vec::new(),
        out_ints: Vec::new(),
        out_longs: Vec::new(),
        fail_new_array: false,
    });

    let table = zeroed_table();
    unsafe {
        (*table).get_array_length = fake_get_array_length;
        (*table).new_byte_array = fake_new_byte_array;
        (*table).new_long_array = fake_new_long_array;
        (*table).get_byte_array_region = fake_get_byte_array_region;
        (*table).set_byte_array_region = fake_set_byte_array_region;
        (*table).set_int_array_region = fake_set_int_array_region;
        (*table).set_long_array_region = fake_set_long_array_region;
    }
    let functions: *const FakeTable = table;
    let env: *mut JNIEnv = Box::into_raw(Box::new(functions));

    let result = f(env);
    let state = CALL.lock().unwrap().take().expect("fake call state");
    (result, state.out_bytes, state.out_ints, state.out_longs)
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

/// Reads back the fresh `byte[]` the glue produced through
/// `SetByteArrayRegion` (the recorded bytes).
fn produced(bytes: &[u8]) -> Vec<u8> {
    bytes.to_vec()
}

/// The lowercase hex of `bytes`, for the reference.json comparisons.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[test]
fn jni_sha256_matches_the_library_and_the_reference() {
    let input = b"abc".to_vec();
    let ((digest, status), written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(input.clone());
        let slot = status_slot();
        let digest =
            Java_hash_sillstone_digest_PithDigest_sha256Native(env, std::ptr::null_mut(), data, slot);
        (digest, read_status(slot))
    });
    assert_eq!(status, 0, "status");
    assert!(!digest.is_null(), "digest array");
    assert_eq!(
        hex(&produced(&written)),
        sha256(&input)
            .unwrap()
            .as_bytes()
            .iter()
            .fold(String::with_capacity(64), |mut s, b| {
                s.push_str(&format!("{b:02x}"));
                s
            })
    );
    // reference.json `sha256`: sha256("abc").
    assert_eq!(
        hex(&produced(&written)),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn jni_sha1_matches_the_library_and_the_reference() {
    let input = b"abc".to_vec();
    let ((digest, status), written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(input.clone());
        let slot = status_slot();
        let digest =
            Java_hash_sillstone_digest_PithDigest_sha1Native(env, std::ptr::null_mut(), data, slot);
        (digest, read_status(slot))
    });
    assert_eq!(status, 0, "status");
    assert!(!digest.is_null(), "digest array");
    assert_eq!(written.len(), 20, "20-byte out");
    assert_eq!(
        hex(&produced(&written)),
        sha1(&input)
            .unwrap()
            .as_bytes()
            .iter()
            .fold(String::with_capacity(64), |mut s, b| {
                s.push_str(&format!("{b:02x}"));
                s
            })
    );
    // reference.json `sha1`: sha1("abc").
    assert_eq!(
        hex(&produced(&written)),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
}

#[test]
fn jni_sha512_matches_the_library() {
    let input = b"abc".to_vec();
    let ((_digest, status), written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(input.clone());
        let slot = status_slot();
        let digest =
            Java_hash_sillstone_digest_PithDigest_sha512Native(env, std::ptr::null_mut(), data, slot);
        (digest, read_status(slot))
    });
    assert_eq!(status, 0, "status");
    assert_eq!(written.len(), 64, "64-byte out");
    assert_eq!(
        produced(&written),
        sha512(&input).as_bytes().to_vec(),
        "digest bytes"
    );
}

#[test]
fn jni_hmac_sha256_matches_the_library_and_the_reference() {
    // The RFC 4231 case #2 from reference.json `hmac_sha256`: key
    // "Jefe", data "what do ya want for nothing?".
    let key = b"Jefe".to_vec();
    let data = b"what do ya want for nothing?".to_vec();
    let ((mac, status), written, _ints, _longs) = with_fake_env(|env| unsafe {
        let key = byte_array(key);
        let data = byte_array(data);
        let slot = status_slot();
        let mac = Java_hash_sillstone_digest_PithDigest_hmacSha256Native(
            env,
            std::ptr::null_mut(),
            key,
            data,
            slot,
        );
        (mac, read_status(slot))
    });
    assert_eq!(status, 0, "status");
    assert!(!mac.is_null(), "mac array");
    assert_eq!(written.len(), 32, "32-byte mac");
    assert_eq!(
        hex(&produced(&written)),
        hmac_sha256(b"Jefe", b"what do ya want for nothing?")
            .unwrap()
            .as_bytes()
            .iter()
            .fold(String::with_capacity(64), |mut s, b| {
                s.push_str(&format!("{b:02x}"));
                s
            })
    );
    assert_eq!(
        hex(&produced(&written)),
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
}

#[test]
fn jni_hmac_sha1_matches_the_library() {
    let ((_mac, status), written, _ints, _longs) = with_fake_env(|env| unsafe {
        let key = byte_array(b"key".to_vec());
        let data = b"The quick brown fox jumps over the lazy dog".to_vec();
        let data = byte_array(data);
        let slot = status_slot();
        let mac = Java_hash_sillstone_digest_PithDigest_hmacSha1Native(
            env,
            std::ptr::null_mut(),
            key,
            data,
            slot,
        );
        (mac, read_status(slot))
    });
    assert_eq!(status, 0, "status");
    assert_eq!(written.len(), 20, "20-byte mac");
    assert_eq!(
        produced(&written),
        hmac_sha1(b"key", b"The quick brown fox jumps over the lazy dog")
            .unwrap()
            .as_bytes()
            .to_vec(),
        "mac bytes"
    );
}

#[test]
fn jni_hmac_sha512_matches_the_library() {
    let ((_mac, status), written, _ints, _longs) = with_fake_env(|env| unsafe {
        let key = byte_array(b"Jefe".to_vec());
        let data = byte_array(b"what do ya want for nothing?".to_vec());
        let slot = status_slot();
        let mac = Java_hash_sillstone_digest_PithDigest_hmacSha512Native(
            env,
            std::ptr::null_mut(),
            key,
            data,
            slot,
        );
        (mac, read_status(slot))
    });
    assert_eq!(status, 0, "status");
    assert_eq!(written.len(), 64, "64-byte mac");
    assert_eq!(
        produced(&written),
        hmac_sha512(b"Jefe", b"what do ya want for nothing?")
            .as_bytes()
            .to_vec(),
        "mac bytes"
    );
}

#[test]
fn jni_murmur3_matches_the_library() {
    let ((digest, status), written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(b"hello".to_vec());
        let slot = status_slot();
        let digest = Java_hash_sillstone_digest_PithDigest_murmur3X64128Native(
            env,
            std::ptr::null_mut(),
            data,
            0,
            slot,
        );
        (digest, read_status(slot))
    });
    assert_eq!(status, 0, "status");
    assert!(!digest.is_null(), "digest array");
    assert_eq!(written.len(), 16, "128-bit digest");
    let expected: Digest<16> = murmur3_x64_128(b"hello", 0);
    assert_eq!(
        produced(&written),
        expected.as_bytes().to_vec(),
        "digest bytes"
    );
}

#[test]
fn jni_xxh64_matches_the_library() {
    let ((value, status), _written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(b"foobar".to_vec());
        let slot = status_slot();
        let value =
            Java_hash_sillstone_digest_PithDigest_xxh64Native(env, std::ptr::null_mut(), data, 7, slot);
        (value, read_status(slot))
    });
    // The scalar exports carry the result in the return value and only
    // touch the status slot on the error path, so it stays untouched.
    assert_eq!(status, i32::MIN, "status slot untouched on success");
    assert_eq!(value as u64, xxh64(b"foobar", 7), "xxh64 value");
}

#[test]
fn jni_crc32_matches_the_library_and_the_reference() {
    // reference.json `crc32`: crc32("123456789") = cbf43926.
    let ((value, status), _written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(b"123456789".to_vec());
        let slot = status_slot();
        let value =
            Java_hash_sillstone_digest_PithDigest_crc32Native(env, std::ptr::null_mut(), data, slot);
        (value, read_status(slot))
    });
    assert_eq!(status, i32::MIN, "status slot untouched on success");
    assert_eq!(value as u32, crc32(b"123456789"), "crc32 value");
    assert_eq!(value as u32, 0xcbf4_3926, "reference value");
}

#[test]
fn jni_crc32c_matches_the_library() {
    let ((value, status), _written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(b"123456789".to_vec());
        let slot = status_slot();
        let value =
            Java_hash_sillstone_digest_PithDigest_crc32cNative(env, std::ptr::null_mut(), data, slot);
        (value, read_status(slot))
    });
    assert_eq!(status, i32::MIN, "status slot untouched on success");
    assert_eq!(value as u32, crc32c(b"123456789"), "crc32c value");
}

#[test]
fn jni_adler32_matches_the_library() {
    let ((value, status), _written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(b"Wikipedia".to_vec());
        let slot = status_slot();
        let value =
            Java_hash_sillstone_digest_PithDigest_adler32Native(env, std::ptr::null_mut(), data, slot);
        (value, read_status(slot))
    });
    assert_eq!(status, i32::MIN, "status slot untouched on success");
    assert_eq!(value as u32, adler32(b"Wikipedia"), "adler32 value");
    assert_eq!(value as u32, 0x11e6_0398, "reference value");
}

#[test]
fn jni_fnv1a64_matches_the_library() {
    let ((value, status), _written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(b"foobar".to_vec());
        let slot = status_slot();
        let value =
            Java_hash_sillstone_digest_PithDigest_fnv1a64Native(env, std::ptr::null_mut(), data, slot);
        (value, read_status(slot))
    });
    assert_eq!(status, i32::MIN, "status slot untouched on success");
    assert_eq!(value as u64, fnv1a64(b"foobar"), "fnv1a64 value");
}

#[test]
fn jni_splitmix64_fill_matches_the_safe_core() {
    let ((array, status), _written, _ints, longs) = with_fake_env(|env| unsafe {
        let slot = status_slot();
        let array = Java_hash_sillstone_digest_PithDigest_splitmix64FillNative(
            env,
            std::ptr::null_mut(),
            0x0000_D1CE,
            4,
            slot,
        );
        (array, read_status(slot))
    });
    assert_eq!(status, 0, "status");
    assert!(!array.is_null(), "long[] array");
    let mut rng = SplitMix64::new(0x0000_D1CE);
    let expected: Vec<i64> = (0..4).map(|_| rng.next_u64() as i64).collect();
    assert_eq!(longs, expected, "splitmix64 sequence through slots 180/212");
}

#[test]
fn jni_xoshiro256_fill_matches_the_safe_core() {
    let ((array, status), _written, _ints, longs) = with_fake_env(|env| unsafe {
        let slot = status_slot();
        let array = Java_hash_sillstone_digest_PithDigest_xoshiro256FillNative(
            env,
            std::ptr::null_mut(),
            0x0000_F00D,
            4,
            slot,
        );
        (array, read_status(slot))
    });
    assert_eq!(status, 0, "status");
    assert!(!array.is_null(), "long[] array");
    let mut rng = Xoshiro256StarStar::from_seed(0x0000_F00D);
    let expected: Vec<i64> = (0..4).map(|_| rng.next_u64() as i64).collect();
    assert_eq!(
        longs, expected,
        "xoshiro256** sequence through slots 180/212"
    );
}

#[test]
fn jni_negative_fill_count_is_invalid() {
    let ((array, status), _written, _ints, longs) = with_fake_env(|env| unsafe {
        let slot = status_slot();
        let array = Java_hash_sillstone_digest_PithDigest_splitmix64FillNative(
            env,
            std::ptr::null_mut(),
            1,
            -1,
            slot,
        );
        (array, read_status(slot))
    });
    assert_eq!(status, -1, "status");
    assert!(array.is_null());
    assert!(longs.is_empty(), "no longs written");
}

#[test]
fn jni_base64_round_trip_matches_the_reference() {
    let ((encoded, status), written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(b"foobar".to_vec());
        let slot = status_slot();
        let encoded = Java_hash_sillstone_digest_PithDigest_base64EncodeNative(
            env,
            std::ptr::null_mut(),
            data,
            slot,
        );
        (encoded, read_status(slot))
    });
    assert_eq!(status, 0, "encode status");
    assert!(!encoded.is_null(), "encoded array");
    // reference.json `base64`: base64("foobar") = "Zm9vYmFy".
    let encoded = produced(&written);
    assert_eq!(String::from_utf8(encoded.clone()).unwrap(), "Zm9vYmFy");

    let ((decoded, status), written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(encoded);
        let slot = status_slot();
        let decoded = Java_hash_sillstone_digest_PithDigest_base64DecodeNative(
            env,
            std::ptr::null_mut(),
            data,
            slot,
        );
        (decoded, read_status(slot))
    });
    assert_eq!(status, 0, "decode status");
    assert!(!decoded.is_null(), "decoded array");
    assert_eq!(produced(&written), b"foobar".to_vec(), "round trip");
}

#[test]
fn jni_base64_decode_non_canonical_is_rejected() {
    let ((decoded, status), _written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(b"Zm9vYmF!".to_vec());
        let slot = status_slot();
        let decoded = Java_hash_sillstone_digest_PithDigest_base64DecodeNative(
            env,
            std::ptr::null_mut(),
            data,
            slot,
        );
        (decoded, read_status(slot))
    });
    assert_eq!(status, -2, "status");
    assert!(decoded.is_null());
}

#[test]
fn jni_digest_null_data_is_invalid() {
    let ((digest, status), _written, _ints, _longs) = with_fake_env(|env| unsafe {
        let slot = status_slot();
        let digest = Java_hash_sillstone_digest_PithDigest_sha256Native(
            env,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            slot,
        );
        (digest, read_status(slot))
    });
    assert_eq!(status, -1, "status");
    assert!(digest.is_null());
}

#[test]
fn jni_environment_refusing_the_array_is_rejected() {
    let ((digest, status), _written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(b"abc".to_vec());
        let slot = status_slot();
        with_state(|state| state.fail_new_array = true);
        let digest =
            Java_hash_sillstone_digest_PithDigest_sha256Native(env, std::ptr::null_mut(), data, slot);
        (digest, read_status(slot))
    });
    assert_eq!(status, -2, "status");
    assert!(digest.is_null());
}

#[test]
fn jni_digest_null_status_short_circuits() {
    let (digest, _written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(b"abc".to_vec());
        Java_hash_sillstone_digest_PithDigest_sha256Native(
            env,
            std::ptr::null_mut(),
            data,
            std::ptr::null_mut(),
        )
    });
    assert!(digest.is_null());
}

#[test]
fn jni_null_environment_short_circuits() {
    let ((digest, mac, fill, crc), _written, _ints, _longs) = with_fake_env(|_env| unsafe {
        let data = byte_array(b"abc".to_vec());
        let slot = status_slot();
        let digest = Java_hash_sillstone_digest_PithDigest_sha256Native(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            data,
            slot,
        );
        let mac = Java_hash_sillstone_digest_PithDigest_hmacSha256Native(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            data,
            data,
            slot,
        );
        let fill = Java_hash_sillstone_digest_PithDigest_xoshiro256FillNative(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            1,
            4,
            slot,
        );
        let crc = Java_hash_sillstone_digest_PithDigest_crc32Native(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            data,
            slot,
        );
        (digest, mac, fill, crc)
    });
    assert!(digest.is_null());
    assert!(mac.is_null());
    assert!(fill.is_null());
    assert_eq!(crc, 0);
}

#[test]
fn jni_stale_array_handle_is_invalid() {
    let ((digest, status), _written, _ints, _longs) = with_fake_env(|env| unsafe {
        let data = byte_array(b"abc".to_vec());
        // Unregister the handle behind the JVM's back: the next
        // GetArrayLength reports -1, which the glue maps to INVALID.
        let stale = data as usize;
        with_state(|state| state.byte_arrays.remove(&stale));
        let slot = status_slot();
        let digest =
            Java_hash_sillstone_digest_PithDigest_sha1Native(env, std::ptr::null_mut(), data, slot);
        (digest, read_status(slot))
    });
    assert_eq!(status, -1, "status");
    assert!(digest.is_null());
}
