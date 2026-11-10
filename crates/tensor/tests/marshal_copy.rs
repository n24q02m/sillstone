//! Measured cost of the marshalling before and after the copy removal.
//!
//! The allocation counts are the hard evidence: they are deterministic, while a
//! wall-clock ratio on a laptop that drops its clock by a third under a
//! sustained loop is not. The timings are printed for the record and gated only
//! loosely.
//!
//! Run with `cargo test -p sillstone-tensor --test marshal_copy -- --nocapture`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::time::Instant;

use sillstone_tensor::Tensor;
use sillstone_tensor::ffi::sillstone_tensor_matmul_f64;

/// Counts the allocations *of the measuring thread*, in blocks and in bytes, so
/// that a test running in parallel cannot rewrite the numbers.
struct Counting;

thread_local! {
    /// Blocks allocated by this thread.
    static BLOCKS: Cell<usize> = const { Cell::new(0) };
    /// Bytes allocated by this thread.
    static BYTES: Cell<usize> = const { Cell::new(0) };
}

fn count(size: usize) {
    // `try_with` keeps the allocator from panicking while a thread is torn down.
    let _ = BLOCKS.try_with(|b| b.set(b.get() + 1));
    let _ = BYTES.try_with(|b| b.set(b.get() + size));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocated() -> (usize, usize) {
    (BLOCKS.with(Cell::get), BYTES.with(Cell::get))
}

/// Runs `f` and reports the blocks and bytes it allocated.
fn measure<T, F: FnOnce() -> T>(f: F) -> (T, usize, usize) {
    let (blocks0, bytes0) = allocated();
    let value = f();
    let (blocks1, bytes1) = allocated();
    (value, blocks1 - blocks0, bytes1 - bytes0)
}

/// Elements in the round-trip payload: 2 Mi elements = 16 MiB of f64.
const PAYLOAD: usize = 2 * 1024 * 1024;

fn payload(n: usize) -> Vec<f64> {
    (0..n).map(|i| i as f64 + 0.5).collect()
}

/// Copy-based bridge: materialize the input into a tensor, then copy the
/// elements back out through `contiguous().as_slice().to_vec()`.
fn round_trip_with_copies(data: &[f64]) -> Vec<f64> {
    let t = Tensor::from_vec(vec![data.len()], data.to_vec()).unwrap();
    t.contiguous().as_slice().expect("contiguous").to_vec()
}

/// Nanoseconds per call, best of `REPEATS` runs of `iters` calls.
fn ns_per_call<F: FnMut()>(iters: usize, mut f: F) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..5 {
        let start = Instant::now();
        for _ in 0..iters {
            f();
        }
        best = best.min(start.elapsed().as_nanos() as f64 / iters as f64);
    }
    best
}

#[test]
fn marshalling_moves_the_payload_instead_of_copying_it() {
    let data = payload(PAYLOAD);
    let mib = PAYLOAD as f64 * 8.0 / (1024.0 * 1024.0);

    // Before: the bridge copied the payload in and copied it back out.
    let (copied, copy_blocks, copy_bytes) = measure(|| round_trip_with_copies(black_box(&data)));
    assert_eq!(copied.len(), PAYLOAD);
    assert_eq!(copied, data);
    drop(copied);

    // After: `from_vec` takes the buffer, `into_data` hands it back. The only
    // allocation left is the `Arc` control block that makes a tensor shareable.
    let mut recycled = Some(data.clone());
    let (_, move_blocks, move_bytes) = measure(|| {
        let owned = recycled.take().expect("buffer is returned every iteration");
        let t = Tensor::from_vec(vec![PAYLOAD], owned).unwrap();
        recycled = Some(t.into_data().expect("sole owner"));
    });
    assert_eq!(recycled.as_ref().map(Vec::len), Some(PAYLOAD));

    let copy_ns = ns_per_call(4, || {
        assert_eq!(round_trip_with_copies(black_box(&data)).len(), PAYLOAD)
    });
    let move_ns = ns_per_call(4, || {
        let owned = recycled.take().expect("buffer is returned every iteration");
        let t = Tensor::from_vec(vec![PAYLOAD], owned).unwrap();
        recycled = Some(t.into_data().expect("sole owner"));
    });

    println!(
        "round trip of {PAYLOAD} f64 ({mib:.0} MiB):\n  \
         copying bridge: {copy_blocks} blocks, {copy_bytes} bytes, {:.1} us\n  \
         moving bridge:  {move_blocks} blocks, {move_bytes} bytes, {:.3} us",
        copy_ns / 1000.0,
        move_ns / 1000.0
    );

    // The payload is 16 MiB: copying in and out allocates two buffers of it,
    // moving allocates nothing but the `Arc` header.
    assert!(
        copy_bytes >= 2 * PAYLOAD * 8,
        "the copying bridge should allocate both payload buffers"
    );
    assert!(
        move_bytes < 128,
        "the moving bridge allocated {move_bytes} bytes"
    );
    assert!(
        move_ns * 100.0 < copy_ns,
        "moving must cost far less than copying in and copying out"
    );
}

/// The marshalling the export used before: copy both operands into owned
/// buffers, multiply into a third buffer, then copy that into `out`.
fn matmul_with_copies(a: &[f64], b: &[f64], n: usize, out: &mut [f64]) {
    let ta = Tensor::from_vec(vec![n, n], a.to_vec()).unwrap();
    let tb = Tensor::from_vec(vec![n, n], b.to_vec()).unwrap();
    let res = ta.matmul(&tb).unwrap();
    out.copy_from_slice(res.as_slice().unwrap());
}

/// The marshalling the export uses now: multiply straight into `out`.
fn matmul_in_place(a: &[f64], b: &[f64], n: usize, out: &mut [f64]) {
    let rc = unsafe {
        sillstone_tensor_matmul_f64(a.as_ptr(), n, n, b.as_ptr(), n, n, out.as_mut_ptr())
    };
    assert_eq!(rc, 0);
}

#[test]
fn abi_matmul_call_no_longer_allocates_payload_buffers() {
    for n in [4usize, 16, 64] {
        let a: Vec<f64> = (0..n * n)
            .map(|i| ((i * 7 + 3) % 101) as f64 * 0.25)
            .collect();
        let b: Vec<f64> = (0..n * n)
            .map(|i| ((i * 11 + 5) % 97) as f64 * 0.5)
            .collect();
        let mut copied = vec![0.0f64; n * n];
        let mut in_place = vec![0.0f64; n * n];

        matmul_with_copies(&a, &b, n, &mut copied);
        matmul_in_place(&a, &b, n, &mut in_place);
        assert_eq!(copied, in_place, "both paths must produce the same product");

        let (_, copy_blocks, copy_bytes) =
            measure(|| matmul_with_copies(black_box(&a), black_box(&b), n, black_box(&mut copied)));
        let (_, new_blocks, new_bytes) =
            measure(|| matmul_in_place(black_box(&a), black_box(&b), n, black_box(&mut in_place)));

        let iters = 60_000 / n;
        let old_ns = ns_per_call(iters, || {
            matmul_with_copies(black_box(&a), black_box(&b), n, black_box(&mut copied))
        });
        let new_ns = ns_per_call(iters, || {
            matmul_in_place(black_box(&a), black_box(&b), n, black_box(&mut in_place))
        });

        println!(
            "{n}x{n} f64 ABI matmul call:\n  \
             copying path: {copy_blocks} blocks, {copy_bytes} bytes, {old_ns:.0} ns\n  \
             in-place path: {new_blocks} blocks, {new_bytes} bytes, {new_ns:.0} ns",
        );

        assert_eq!(
            new_blocks, 0,
            "{n}x{n}: the in-place call must not allocate"
        );
        assert_eq!(new_bytes, 0, "{n}x{n}: the in-place call must not allocate");
        // Three payload buffers of n*n f64 were copied per call before.
        assert!(
            copy_bytes >= 3 * n * n * 8,
            "{n}x{n}: the copying path should allocate the three payload buffers"
        );
    }
}
