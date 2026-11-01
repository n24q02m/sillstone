//! Fake-`JNIEnv` scenarios for the JNI bridge.
//!
//! The Java suite only reaches the success paths of these exports. Here a fake function
//! table backs every array with a Rust vector, so the defensive arms (null arrays, bad
//! lengths, failed allocations) run too and the marshalling of each argument is checked
//! against an independent oracle: published hash vectors, SciPy's orthonormal DSTs, the
//! FNV-1a definition, or the plain C ABI called directly with the same arguments.

use super::*;
use std::cell::RefCell;

const NULL: JArray = core::ptr::null_mut();

enum Slot {
    Bytes(Vec<u8>),
    Ints(Vec<jint>),
    Doubles(Vec<f64>),
}

#[derive(Default)]
struct Jvm {
    arrays: Vec<Slot>,
    fail_new_byte: bool,
    fail_new_int: bool,
    fail_new_double: bool,
}

thread_local! {
    static JVM: RefCell<Jvm> = RefCell::new(Jvm::default());
}

fn with_jvm<R>(f: impl FnOnce(&mut Jvm) -> R) -> R {
    JVM.with(|j| f(&mut j.borrow_mut()))
}

fn push(slot: Slot) -> JArray {
    with_jvm(|j| {
        j.arrays.push(slot);
        core::ptr::without_provenance_mut(j.arrays.len())
    })
}

fn byte_array(bytes: &[u8]) -> JArray {
    push(Slot::Bytes(bytes.to_vec()))
}

fn double_array(values: &[f64]) -> JArray {
    push(Slot::Doubles(values.to_vec()))
}

fn bytes_of(handle: JArray) -> Vec<u8> {
    assert!(!handle.is_null(), "expected a byte array, got null");
    with_jvm(|j| match &j.arrays[handle.addr() - 1] {
        Slot::Bytes(v) => v.clone(),
        _ => panic!("not a byte array"),
    })
}

fn doubles_of(handle: JArray) -> Vec<f64> {
    assert!(!handle.is_null(), "expected a double array, got null");
    with_jvm(|j| match &j.arrays[handle.addr() - 1] {
        Slot::Doubles(v) => v.clone(),
        _ => panic!("not a double array"),
    })
}

fn ints_of(handle: JArray) -> Vec<jint> {
    assert!(!handle.is_null(), "expected an int array, got null");
    with_jvm(|j| match &j.arrays[handle.addr() - 1] {
        Slot::Ints(v) => v.clone(),
        _ => panic!("not an int array"),
    })
}

extern "system" fn fake_get_array_length(_env: *mut JNIEnv, array: JArray) -> jint {
    with_jvm(|j| match &j.arrays[array.addr() - 1] {
        Slot::Bytes(v) => v.len(),
        Slot::Ints(v) => v.len(),
        Slot::Doubles(v) => v.len(),
    }) as jint
}

extern "system" fn fake_new_byte_array(_env: *mut JNIEnv, len: jint) -> JArray {
    with_jvm(|j| {
        if j.fail_new_byte {
            return NULL;
        }
        j.arrays.push(Slot::Bytes(vec![0; len as usize]));
        core::ptr::without_provenance_mut(j.arrays.len())
    })
}

extern "system" fn fake_new_int_array(_env: *mut JNIEnv, len: jint) -> JArray {
    with_jvm(|j| {
        if j.fail_new_int {
            return NULL;
        }
        j.arrays.push(Slot::Ints(vec![0; len as usize]));
        core::ptr::without_provenance_mut(j.arrays.len())
    })
}

extern "system" fn fake_new_double_array(_env: *mut JNIEnv, len: jint) -> JArray {
    with_jvm(|j| {
        if j.fail_new_double {
            return NULL;
        }
        j.arrays.push(Slot::Doubles(vec![0.0; len as usize]));
        core::ptr::without_provenance_mut(j.arrays.len())
    })
}

extern "system" fn fake_get_byte_region(
    _env: *mut JNIEnv,
    array: JArray,
    start: jint,
    len: jint,
    buf: *mut jbyte,
) {
    with_jvm(|j| {
        let Slot::Bytes(v) = &j.arrays[array.addr() - 1] else {
            panic!("not a byte array");
        };
        for i in 0..len as usize {
            unsafe { *buf.add(i) = v[start as usize + i] as jbyte };
        }
    });
}

extern "system" fn fake_set_byte_region(
    _env: *mut JNIEnv,
    array: JArray,
    start: jint,
    len: jint,
    buf: *const jbyte,
) {
    with_jvm(|j| {
        let Slot::Bytes(v) = &mut j.arrays[array.addr() - 1] else {
            panic!("not a byte array");
        };
        for i in 0..len as usize {
            v[start as usize + i] = unsafe { *buf.add(i) } as u8;
        }
    });
}

extern "system" fn fake_set_int_region(
    _env: *mut JNIEnv,
    array: JArray,
    start: jint,
    len: jint,
    buf: *const jint,
) {
    with_jvm(|j| {
        let Slot::Ints(v) = &mut j.arrays[array.addr() - 1] else {
            panic!("not an int array");
        };
        for i in 0..len as usize {
            v[start as usize + i] = unsafe { *buf.add(i) };
        }
    });
}

extern "system" fn fake_set_double_region(
    _env: *mut JNIEnv,
    array: JArray,
    start: jint,
    len: jint,
    buf: *const f64,
) {
    with_jvm(|j| {
        let Slot::Doubles(v) = &mut j.arrays[array.addr() - 1] else {
            panic!("not a double array");
        };
        for i in 0..len as usize {
            v[start as usize + i] = unsafe { *buf.add(i) };
        }
    });
}

extern "system" fn fake_get_double_elements(
    _env: *mut JNIEnv,
    array: JArray,
    _is_copy_a: *mut f64,
    _is_copy_b: *mut u8,
) -> *mut f64 {
    with_jvm(|j| match &mut j.arrays[array.addr() - 1] {
        Slot::Doubles(v) => v.as_mut_ptr(),
        _ => panic!("not a double array"),
    })
}

extern "system" fn fake_release_double_elements(
    _env: *mut JNIEnv,
    _array: JArray,
    _data: *mut f64,
    _mode: jint,
) {
}

/// Runs `f` with a `JNIEnv` whose function table is the fake above. The slot numbers are
/// the real JNI function-table indices the bridge reads.
fn with_env<R>(f: impl FnOnce(*mut JNIEnv) -> R) -> R {
    let mut table = [0usize; 256];
    table[171] = fake_get_array_length as *const () as usize;
    table[176] = fake_new_byte_array as *const () as usize;
    table[179] = fake_new_int_array as *const () as usize;
    table[182] = fake_new_double_array as *const () as usize;
    table[190] = fake_get_double_elements as *const () as usize;
    table[198] = fake_release_double_elements as *const () as usize;
    table[200] = fake_get_byte_region as *const () as usize;
    table[208] = fake_set_byte_region as *const () as usize;
    table[211] = fake_set_int_region as *const () as usize;
    table[214] = fake_set_double_region as *const () as usize;
    let mut env: *const [usize; 256] = &table;
    f((&mut env as *mut *const [usize; 256]).cast())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn assert_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-12, "{a} != {e}");
    }
}

#[test]
fn version_exports_report_the_crate_version() {
    let parts: Vec<jint> = env!("CARGO_PKG_VERSION")
        .split(['.', '-'])
        .take(3)
        .map(|p| p.parse().unwrap())
        .collect();
    let got = unsafe {
        [
            Java_io_github_sillstone_Generated_sillstone_1version_1major(NULL, NULL),
            Java_io_github_sillstone_Generated_sillstone_1version_1minor(NULL, NULL),
            Java_io_github_sillstone_Generated_sillstone_1version_1patch(NULL, NULL),
        ]
    };
    assert_eq!(got.to_vec(), parts);
}

#[test]
fn checksum_is_fnv1a_and_treats_java_bytes_as_unsigned() {
    // FNV-1a 64 of "sillstone" followed by 0xFF, computed with the published definition.
    let expected = 0xe2a8858af53377b3u64;
    with_env(|env| {
        let data = byte_array(b"sillstone\xff");
        let got = unsafe {
            Java_io_github_sillstone_Generated_sillstone_1buffer_1checksum(env, NULL, data, 10)
        };
        assert_eq!(got as u64, expected);
    });
}

#[test]
fn checksum_of_missing_or_empty_input_is_the_fnv_offset_basis() {
    let basis = 0xcbf29ce484222325u64 as jlong;
    with_env(|env| unsafe {
        let call =
            |e, d| Java_io_github_sillstone_Generated_sillstone_1buffer_1checksum(e, NULL, d, 0);
        assert_eq!(call(core::ptr::null_mut(), byte_array(b"x")), basis);
        assert_eq!(call(env, NULL), basis);
        assert_eq!(call(env, byte_array(b"")), basis);
    });
}

#[test]
fn digests_match_published_vectors() {
    type Digest = unsafe extern "system" fn(*mut JNIEnv, *mut c_void, JArray) -> JArray;
    let cases: [(Digest, &[u8], &str); 5] = [
        (
            Java_io_github_sillstone_Generated_sillstone_1digest_1sha256,
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            Java_io_github_sillstone_Generated_sillstone_1digest_1sha3_1256,
            b"abc",
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532",
        ),
        (
            Java_io_github_sillstone_Generated_sillstone_1digest_1sha3_1512,
            b"abc",
            "b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e\
             10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0",
        ),
        (
            Java_io_github_sillstone_Generated_sillstone_1digest_1blake2b_1512,
            b"abc",
            "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d1\
             7d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923",
        ),
        (
            Java_io_github_sillstone_Generated_sillstone_1digest_1blake3,
            b"",
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
        ),
    ];
    with_env(|env| {
        for (digest, input, expected) in cases {
            let out = unsafe { digest(env, NULL, byte_array(input)) };
            assert_eq!(hex(&bytes_of(out)), expected);
        }
    });
}

#[test]
fn digests_hash_a_missing_array_as_empty_input() {
    with_env(|env| {
        let out = unsafe {
            Java_io_github_sillstone_Generated_sillstone_1digest_1sha256(env, NULL, NULL)
        };
        assert_eq!(
            hex(&bytes_of(out)),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    });
}

#[test]
fn digests_return_null_without_an_env_or_when_allocation_fails() {
    let no_env = unsafe {
        Java_io_github_sillstone_Generated_sillstone_1digest_1sha256(
            core::ptr::null_mut(),
            NULL,
            NULL,
        )
    };
    assert!(no_env.is_null());
    with_env(|env| {
        with_jvm(|j| j.fail_new_byte = true);
        let out = unsafe {
            Java_io_github_sillstone_Generated_sillstone_1digest_1sha3_1256(
                env,
                NULL,
                byte_array(b"abc"),
            )
        };
        assert!(out.is_null());
    });
}

#[test]
fn dst_exports_match_scipy_orthonormal_transforms() {
    // scipy.fft.dst(x, type=t, norm="ortho") for x = [1, 2, 3, 4].
    let x = [1.0, 2.0, 3.0, 4.0];
    let dst1 = [
        4.866244947338651,
        -2.1762508994828216,
        1.1487646027368057,
        -0.5137431483730079,
    ];
    let dst2 = [4.619397662556434, -2.0, 1.9134171618254485, -1.0];
    let dst3 = [
        5.230442497387663,
        -1.1585126677811073,
        0.8414873322188928,
        -0.7695575026123374,
    ];
    with_env(|env| unsafe {
        let one =
            Java_io_github_sillstone_Generated_sillstone_1math_1dst1(env, NULL, double_array(&x));
        assert_close(&doubles_of(one), &dst1);
        let two =
            Java_io_github_sillstone_Generated_sillstone_1math_1dst2(env, NULL, double_array(&x));
        assert_close(&doubles_of(two), &dst2);
        let three =
            Java_io_github_sillstone_Generated_sillstone_1math_1dst3(env, NULL, double_array(&x));
        assert_close(&doubles_of(three), &dst3);
        // DST-III is the inverse of DST-II under the orthonormal scaling.
        let back = Java_io_github_sillstone_Generated_sillstone_1math_1dst3(env, NULL, two);
        assert_close(&doubles_of(back), &x);
    });
}

#[test]
fn dst_exports_reject_empty_input_and_report_allocation_failure() {
    with_env(|env| unsafe {
        for f in [
            Java_io_github_sillstone_Generated_sillstone_1math_1dst1,
            Java_io_github_sillstone_Generated_sillstone_1math_1dst2,
            Java_io_github_sillstone_Generated_sillstone_1math_1dst3,
        ] {
            assert!(f(env, NULL, NULL).is_null());
            assert!(f(env, NULL, double_array(&[])).is_null());
        }
        with_jvm(|j| j.fail_new_double = true);
        let out = Java_io_github_sillstone_Generated_sillstone_1math_1dst1(
            env,
            NULL,
            double_array(&[1.0, 2.0]),
        );
        assert!(out.is_null());
    });
}

#[test]
fn unicode_grapheme_count_follows_utf8_clusters() {
    with_env(|env| unsafe {
        let count = |bytes: &[u8]| {
            Java_io_github_sillstone_Generated_sillstone_1unicode_1grapheme_1count(
                env,
                NULL,
                byte_array(bytes),
            )
        };
        // "e" + combining acute, "x", and a regional-indicator flag are three clusters.
        assert_eq!(count("e\u{301}x\u{1F1FB}\u{1F1F3}".as_bytes()), 3);
        assert_eq!(count(b""), 0);
        assert_eq!(count(&[0xFF, 0xFE]), -2);
        assert_eq!(
            Java_io_github_sillstone_Generated_sillstone_1unicode_1grapheme_1count(env, NULL, NULL),
            0
        );
    });
}

#[test]
fn unicode_char_category_uses_the_declared_category_order() {
    // Lu = 8, Ll = 4, Nd = 12, Zs = 28 in the `GeneralCategory` declaration order.
    let category = |cp: jint| unsafe {
        Java_io_github_sillstone_Generated_sillstone_1unicode_1char_1category(
            core::ptr::null_mut(),
            NULL,
            cp,
        )
    };
    assert_eq!(category('A' as jint), 8);
    assert_eq!(category('a' as jint), 4);
    assert_eq!(category('7' as jint), 12);
    assert_eq!(category(' ' as jint), 28);
    // Surrogates, values past U+10FFFF and negative code points are not scalar values.
    assert_eq!(category(0xD800), -1);
    assert_eq!(category(0x110000), -1);
    assert_eq!(category(-1), -1);
}

/// 8x8 RGB8 test pattern with a different value in every channel position.
fn rgb_pattern() -> Vec<u8> {
    (0..8 * 8 * 3)
        .map(|i| ((i * 37 + (i / 3) * 11) % 251) as u8)
        .collect()
}

#[test]
fn image_hash_exports_pass_geometry_and_options_to_the_c_abi() {
    use sillstone_image::ffi::*;
    let data = rgb_pattern();
    let rgb = 2u32;
    macro_rules! direct {
        ($f:ident $(, $extra:expr)*) => {{
            let mut out = 0u64;
            // Only expanded inside the surrounding `unsafe` block below.
            let status = $f(data.as_ptr(), data.len(), 8, 8, rgb $(, $extra)*, &mut out);
            assert_eq!(status, 0);
            out as jlong
        }};
    }
    with_env(|env| unsafe {
        let call = |f: unsafe extern "system" fn(
            *mut JNIEnv,
            *mut c_void,
            JArray,
            jint,
            jint,
            jint,
        ) -> jlong| { f(env, NULL, byte_array(&data), 8, 8, rgb as jint) };
        assert_eq!(
            call(Java_io_github_sillstone_Generated_sillstone_1image_1phash),
            direct!(sillstone_image_phash)
        );
        assert_eq!(
            call(Java_io_github_sillstone_Generated_sillstone_1image_1ahash),
            direct!(sillstone_image_ahash)
        );
        assert_eq!(
            call(Java_io_github_sillstone_Generated_sillstone_1image_1dhash),
            direct!(sillstone_image_dhash)
        );
        assert_eq!(
            call(Java_io_github_sillstone_Generated_sillstone_1image_1whash),
            direct!(sillstone_image_whash, 0)
        );
        assert_eq!(
            call(Java_io_github_sillstone_Generated_sillstone_1image_1colorhash),
            direct!(sillstone_image_colorhash, 3)
        );
    });
}

#[test]
fn image_hash_exports_report_minus_one_for_a_buffer_that_disagrees_with_the_geometry() {
    with_env(|env| unsafe {
        for f in [
            Java_io_github_sillstone_Generated_sillstone_1image_1phash,
            Java_io_github_sillstone_Generated_sillstone_1image_1ahash,
            Java_io_github_sillstone_Generated_sillstone_1image_1dhash,
            Java_io_github_sillstone_Generated_sillstone_1image_1whash,
            Java_io_github_sillstone_Generated_sillstone_1image_1colorhash,
        ] {
            assert_eq!(f(env, NULL, byte_array(&[1, 2, 3]), 8, 8, 2), -1);
            assert_eq!(f(env, NULL, NULL, 8, 8, 2), -1);
        }
    });
}

fn text_sample(seed: u32) -> Vec<u8> {
    let mut state = seed;
    (0..600)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            b'a' + ((state >> 24) % 26) as u8
        })
        .collect()
}

#[test]
fn tlsh_digest_and_distance_match_the_c_abi() {
    let (a, b) = (text_sample(1), text_sample(2));
    let digest = |text: &[u8]| {
        let mut out = [0u8; 72];
        let status = unsafe {
            sillstone_text::ffi::sillstone_text_tlsh(text.as_ptr(), text.len(), out.as_mut_ptr())
        };
        assert_eq!(status, 0);
        out
    };
    let (da, db) = (digest(&a), digest(&b));
    let mut expected = 0u32;
    assert_eq!(
        unsafe {
            sillstone_text::ffi::sillstone_text_tlsh_distance(
                da.as_ptr(),
                db.as_ptr(),
                &mut expected,
            )
        },
        0
    );
    assert!(expected > 0);
    with_env(|env| unsafe {
        let got =
            Java_io_github_sillstone_Generated_sillstone_1text_1tlsh(env, NULL, byte_array(&a));
        assert_eq!(bytes_of(got), da);
        let dist = Java_io_github_sillstone_Generated_sillstone_1text_1tlsh_1distance(
            env,
            NULL,
            byte_array(&da),
            byte_array(&db),
        );
        assert_eq!(dist, expected as jint);
        // Input too short to hash is reported as a null array.
        let short = Java_io_github_sillstone_Generated_sillstone_1text_1tlsh(
            env,
            NULL,
            byte_array(b"short"),
        );
        assert!(short.is_null());
    });
}

#[test]
fn tlsh_distance_refuses_digests_that_are_not_72_bytes() {
    // The C ABI reads 72 bytes from each pointer, so a shorter Java array must never reach it.
    let digest = vec![b'0'; 72];
    with_env(|env| unsafe {
        let call = |a: JArray, b: JArray| {
            Java_io_github_sillstone_Generated_sillstone_1text_1tlsh_1distance(env, NULL, a, b)
        };
        assert_eq!(call(byte_array(&digest[..71]), byte_array(&digest)), -1);
        assert_eq!(call(byte_array(&digest), byte_array(&digest[..1])), -1);
        assert_eq!(call(byte_array(&[]), byte_array(&digest)), -1);
        assert_eq!(call(NULL, NULL), -1);
        let longer = vec![b'0'; 73];
        assert_eq!(call(byte_array(&longer), byte_array(&digest)), -1);
        // Right length but not a TLSH digest: rejected by the C ABI.
        assert_eq!(call(byte_array(&digest), byte_array(&digest)), -1);
    });
}

/// Calls a `*_buffer` C ABI function the way `run_buffer_op` does and returns its output.
fn direct_buffer(out_len: usize, f: impl FnOnce(*mut u8, usize) -> i32) -> Option<Vec<u8>> {
    let mut out = vec![0u8; out_len];
    (f(out.as_mut_ptr(), out.len()) == 0).then_some(out)
}

#[test]
fn buffer_operations_pass_every_argument_to_the_c_abi() {
    use sillstone_image::ffi::*;
    let data = rgb_pattern();
    let n = data.len();
    let rgb = 2u32;
    with_env(|env| unsafe {
        let src = || byte_array(&data);

        let want = direct_buffer(5 * 3 * 3, |o, l| {
            sillstone_image_resize(data.as_ptr(), n, 8, 8, rgb, 5, 3, 2, o, l)
        })
        .unwrap();
        let got = Java_io_github_sillstone_Generated_sillstone_1image_1resize(
            env,
            NULL,
            src(),
            8,
            8,
            2,
            5,
            3,
            2,
            45,
        );
        assert_eq!(bytes_of(got), want);

        let want = direct_buffer(3 * 2 * 3, |o, l| {
            sillstone_image_crop(data.as_ptr(), n, 8, 8, rgb, 1, 2, 4, 4, o, l)
        })
        .unwrap();
        let got = Java_io_github_sillstone_Generated_sillstone_1image_1crop(
            env,
            NULL,
            src(),
            8,
            8,
            2,
            1,
            2,
            4,
            4,
            18,
        );
        assert_eq!(bytes_of(got), want);

        for method in 0..7 {
            let want = direct_buffer(n, |o, l| {
                sillstone_image_transpose(data.as_ptr(), n, 8, 8, rgb, method, o, l)
            });
            let got = Java_io_github_sillstone_Generated_sillstone_1image_1transpose(
                env,
                NULL,
                src(),
                8,
                8,
                2,
                method as jint,
                n as jint,
            );
            match want {
                Some(want) => assert_eq!(bytes_of(got), want, "method {method}"),
                None => assert!(got.is_null(), "method {method}"),
            }
        }

        let want = direct_buffer(5 * 4 * 3, |o, l| {
            sillstone_image_rotate(data.as_ptr(), n, 8, 8, rgb, 30.0, 0, 1, o, l)
        });
        let got = Java_io_github_sillstone_Generated_sillstone_1image_1rotate(
            env,
            NULL,
            src(),
            8,
            8,
            2,
            30.0,
            0,
            1,
            (5 * 4 * 3) as jint,
        );
        assert_eq!(got.is_null(), want.is_none());

        let want = direct_buffer(n, |o, l| {
            sillstone_image_filter(data.as_ptr(), n, 8, 8, rgb, 1, o, l)
        })
        .unwrap();
        let got = Java_io_github_sillstone_Generated_sillstone_1image_1filter(
            env,
            NULL,
            src(),
            8,
            8,
            2,
            1,
            n as jint,
        );
        assert_eq!(bytes_of(got), want);

        let want = direct_buffer(n, |o, l| {
            sillstone_image_adjust(data.as_ptr(), n, 8, 8, rgb, 0, 1.5, o, l)
        })
        .unwrap();
        let got = Java_io_github_sillstone_Generated_sillstone_1image_1adjust(
            env,
            NULL,
            src(),
            8,
            8,
            2,
            0,
            1.5,
            n as jint,
        );
        assert_eq!(bytes_of(got), want);

        let want = direct_buffer(8 * 8, |o, l| {
            sillstone_image_color_convert(data.as_ptr(), n, 8, 8, rgb, 0, o, l)
        })
        .unwrap();
        let got = Java_io_github_sillstone_Generated_sillstone_1image_1color_1convert(
            env,
            NULL,
            src(),
            8,
            8,
            2,
            0,
            64,
        );
        assert_eq!(bytes_of(got), want);
    });
}

#[test]
fn buffer_operations_return_null_for_a_bad_output_size_or_a_failed_allocation() {
    let data = rgb_pattern();
    with_env(|env| unsafe {
        // A negative `outLen` is refused before any buffer is allocated.
        let got = Java_io_github_sillstone_Generated_sillstone_1image_1filter(
            env,
            NULL,
            byte_array(&data),
            8,
            8,
            2,
            1,
            -1,
        );
        assert!(got.is_null());
        // An output buffer smaller than the result is refused by the C ABI.
        let got = Java_io_github_sillstone_Generated_sillstone_1image_1filter(
            env,
            NULL,
            byte_array(&data),
            8,
            8,
            2,
            1,
            4,
        );
        assert!(got.is_null());
        let got = Java_io_github_sillstone_Generated_sillstone_1image_1resize(
            env,
            NULL,
            byte_array(&data),
            8,
            8,
            2,
            5,
            3,
            2,
            4,
        );
        assert!(got.is_null());
        // The Java array for the result cannot be allocated.
        with_jvm(|j| j.fail_new_byte = true);
        let got = Java_io_github_sillstone_Generated_sillstone_1image_1filter(
            env,
            NULL,
            byte_array(&data),
            8,
            8,
            2,
            1,
            data.len() as jint,
        );
        assert!(got.is_null());
    });
}

#[test]
fn encoders_and_decoder_round_trip_through_the_bridge() {
    let data = rgb_pattern();
    with_env(|env| unsafe {
        let qoi = Java_io_github_sillstone_Generated_sillstone_1image_1qoi_1encode(
            env,
            NULL,
            byte_array(&data),
            8,
            8,
            2,
        );
        let qoi_bytes = bytes_of(qoi);
        assert_eq!(&qoi_bytes[..4], b"qoif");
        assert_eq!(&qoi_bytes[4..8], 8u32.to_be_bytes());

        let packed = Java_io_github_sillstone_Generated_sillstone_1image_1qoi_1decode(
            env,
            NULL,
            byte_array(&qoi_bytes),
        );
        let packed = bytes_of(packed);
        assert_eq!(&packed[0..4], 8u32.to_le_bytes());
        assert_eq!(&packed[4..8], 8u32.to_le_bytes());
        assert_eq!(&packed[8..12], 2u32.to_le_bytes());
        assert_eq!(&packed[12..], data.as_slice());

        let bmp = Java_io_github_sillstone_Generated_sillstone_1image_1bmp_1encode(
            env,
            NULL,
            byte_array(&data),
            8,
            8,
            2,
        );
        let bmp_bytes = bytes_of(bmp);
        assert_eq!(&bmp_bytes[..2], b"BM");
        assert_eq!(
            u32::from_le_bytes(bmp_bytes[2..6].try_into().unwrap()) as usize,
            bmp_bytes.len()
        );
    });
}

#[test]
fn encoders_and_decoder_return_null_for_invalid_input_or_failed_allocation() {
    let data = rgb_pattern();
    with_env(|env| unsafe {
        let bad_layout = Java_io_github_sillstone_Generated_sillstone_1image_1qoi_1encode(
            env,
            NULL,
            byte_array(&data),
            8,
            8,
            99,
        );
        assert!(bad_layout.is_null());
        let bad_len = Java_io_github_sillstone_Generated_sillstone_1image_1bmp_1encode(
            env,
            NULL,
            byte_array(&data[..5]),
            8,
            8,
            2,
        );
        assert!(bad_len.is_null());
        let garbage = Java_io_github_sillstone_Generated_sillstone_1image_1qoi_1decode(
            env,
            NULL,
            byte_array(b"not a qoi stream"),
        );
        assert!(garbage.is_null());
        let missing =
            Java_io_github_sillstone_Generated_sillstone_1image_1qoi_1decode(env, NULL, NULL);
        assert!(missing.is_null());

        let valid = Java_io_github_sillstone_Generated_sillstone_1image_1qoi_1encode(
            env,
            NULL,
            byte_array(&data),
            8,
            8,
            2,
        );
        let valid = bytes_of(valid);
        with_jvm(|j| j.fail_new_byte = true);
        assert!(
            Java_io_github_sillstone_Generated_sillstone_1image_1qoi_1decode(
                env,
                NULL,
                byte_array(&valid)
            )
            .is_null()
        );
        assert!(
            Java_io_github_sillstone_Generated_sillstone_1image_1bmp_1encode(
                env,
                NULL,
                byte_array(&data),
                8,
                8,
                2
            )
            .is_null()
        );
    });
}

#[test]
fn tensor_exports_compute_matmul_add_and_sum() {
    with_env(|env| unsafe {
        // [[1, 2, 3], [4, 5, 6]] x [[7, 8], [9, 10], [11, 12]]
        let a = double_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = double_array(&[7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
        let product = Java_io_github_sillstone_Generated_sillstone_1tensor_1matmul_1f64(
            env, NULL, a, 2, 3, b, 3, 2,
        );
        assert_eq!(doubles_of(product), [58.0, 64.0, 139.0, 154.0]);

        let sum = Java_io_github_sillstone_Generated_sillstone_1tensor_1add_1f64(
            env,
            NULL,
            double_array(&[1.0, 2.0, 3.0]),
            double_array(&[0.5, 0.25, -3.0]),
        );
        assert_eq!(doubles_of(sum), [1.5, 2.25, 0.0]);

        let total = Java_io_github_sillstone_Generated_sillstone_1tensor_1sum_1f64(
            env,
            NULL,
            double_array(&[1.5, 2.5, 3.0]),
        );
        assert_eq!(total, 7.0);
    });
}

#[test]
fn tensor_exports_reject_shapes_that_disagree_with_the_arrays() {
    with_env(|env| unsafe {
        let matmul = |a: &[f64], m, k1, b: &[f64], k2, n| {
            Java_io_github_sillstone_Generated_sillstone_1tensor_1matmul_1f64(
                env,
                NULL,
                double_array(a),
                m,
                k1,
                double_array(b),
                k2,
                n,
            )
        };
        let six = [1.0; 6];
        assert!(matmul(&six, 0, 3, &six, 3, 2).is_null());
        assert!(matmul(&six, 2, -1, &six, 3, 2).is_null());
        assert!(matmul(&six, 2, 3, &six, 0, 2).is_null());
        assert!(matmul(&six, 2, 3, &six, 3, 0).is_null());
        // The declared shape needs 6 elements per side but the arrays hold fewer or more.
        assert!(matmul(&six[..5], 2, 3, &six, 3, 2).is_null());
        assert!(matmul(&six, 2, 3, &six[..5], 3, 2).is_null());
        // Inner dimensions that disagree cannot be multiplied even when the lengths fit.
        assert!(matmul(&six, 2, 3, &six, 2, 3).is_null());

        let add = |a: &[f64], b: &[f64]| {
            Java_io_github_sillstone_Generated_sillstone_1tensor_1add_1f64(
                env,
                NULL,
                double_array(a),
                double_array(b),
            )
        };
        assert!(add(&[], &[]).is_null());
        assert!(add(&[1.0, 2.0], &[1.0]).is_null());

        let total = Java_io_github_sillstone_Generated_sillstone_1tensor_1sum_1f64(
            env,
            NULL,
            double_array(&[]),
        );
        assert!(total.is_nan());
        let total = Java_io_github_sillstone_Generated_sillstone_1tensor_1sum_1f64(env, NULL, NULL);
        assert!(total.is_nan());

        with_jvm(|j| j.fail_new_double = true);
        assert!(add(&[1.0], &[2.0]).is_null());
        assert!(matmul(&six, 2, 3, &six, 3, 2).is_null());
    });
}

#[test]
fn rotate_size_reports_the_expanded_canvas() {
    with_env(|env| unsafe {
        let size = |w, h, angle: f64, expand| {
            Java_io_github_sillstone_Generated_sillstone_1image_1rotate_1size(
                env, NULL, w, h, angle, expand,
            )
        };
        // A quarter turn swaps the sides.
        assert_eq!(ints_of(size(4, 2, 90.0, 1)), [2, 4]);
        // Exact quarter turns swap the sides whether or not `expand` is set.
        assert_eq!(ints_of(size(4, 2, 90.0, 0)), [2, 4]);
        // An oblique angle keeps the canvas without `expand` and grows it to the
        // bounding box of the rotated rectangle with it: (4 + 2) * cos 45 = 4.24 -> 5.
        assert_eq!(ints_of(size(4, 2, 45.0, 0)), [4, 2]);
        assert_eq!(ints_of(size(4, 2, 45.0, 1)), [5, 5]);
        // A non-finite angle is refused by the C ABI.
        assert!(size(4, 2, f64::NAN, 1).is_null());
        // A canvas wider than a Java int cannot be reported.
        assert!(size(2_000_000_000, 2_000_000_000, 45.0, 1).is_null());
        with_jvm(|j| j.fail_new_int = true);
        assert!(size(4, 2, 90.0, 1).is_null());
    });
}
