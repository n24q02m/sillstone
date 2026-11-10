//! Boundary behaviour of the math, statistics, linear-algebra, manipulation and
//! cast layers: empty and length-1 inputs, strided views, NaN/Inf propagation,
//! integer overflow and the documented deviations from NumPy.

use sillstone_digest::Error;
use sillstone_tensor::{Bf16, F16, NormOrd, Tensor, concat, stack};

fn t2(shape: [usize; 2], data: &[f64]) -> Tensor<f64> {
    Tensor::from_vec(shape.to_vec(), data.to_vec()).expect("shape matches data")
}

fn v(data: &[f64]) -> Tensor<f64> {
    Tensor::from_vec(vec![data.len()], data.to_vec()).expect("shape matches data")
}

const NAN: f64 = f64::NAN;
const INF: f64 = f64::INFINITY;

// ---------------------------------------------------------------- elementwise

#[test]
fn elementwise_keeps_shape_and_element_type() {
    let x = t2([2, 3], &[0.0, 1.0, 4.0, 9.0, 16.0, 25.0]);
    let r = x.sqrt();
    assert_eq!(r.shape(), &[2, 3]);
    assert_eq!(
        r.as_slice().expect("contiguous"),
        &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0]
    );

    // float32 stays float32: no implicit promotion to float64.
    let f = Tensor::<f32>::from_vec(vec![2], vec![1.0f32, 4.0]).expect("shape");
    let s: Tensor<f32> = f.sqrt();
    assert_eq!(s.as_slice().expect("contiguous"), &[1.0f32, 2.0]);

    // float16 computes through f64 and rounds once at the end.
    let h = Tensor::<F16>::from_vec(vec![1], vec![F16::from_f64(4.0)]).expect("shape");
    assert_eq!(h.sqrt().as_slice().expect("contiguous")[0].to_f64(), 2.0);
}

#[test]
fn elementwise_empty_and_length_one() {
    let empty = Tensor::<f64>::zeros(&[0, 3]);
    for r in [
        empty.exp(),
        empty.ln(),
        empty.sqrt(),
        empty.sin(),
        empty.atan(),
    ] {
        assert_eq!(r.shape(), &[0, 3]);
        assert!(r.as_slice().expect("contiguous").is_empty());
    }
    assert_eq!(empty.pow(&empty).expect("broadcast").shape(), &[0, 3]);
    assert_eq!(empty.atan2(&empty).expect("broadcast").shape(), &[0, 3]);
    // A zero-sized binary operand still broadcasts against a real one.
    assert!(v(&[2.0, 3.0]).pow(&Tensor::<f64>::zeros(&[0])).is_err());

    // The kernel is accurate to one ulp, not necessarily correctly rounded.
    let single = v(&[2.0]);
    let root = single.sqrt().as_slice().expect("contiguous")[0];
    assert!((root - core::f64::consts::SQRT_2).abs() <= 2.0 * f64::EPSILON);
    assert_eq!(single.exp().shape(), &[1]);
}

#[test]
fn elementwise_special_values_follow_ieee() {
    let x = v(&[INF, -INF, NAN, -1.0, 0.0, -0.0, 1.0]);
    let r = x.exp();
    let s = r.as_slice().expect("contiguous");
    assert_eq!(s[0], INF);
    assert_eq!(s[1], 0.0);
    assert!(s[2].is_nan());
    assert!((s[3] - 0.367_879_441_171_442_33).abs() <= 2.0 * f64::EPSILON);
    assert_eq!(s[4], 1.0, "exp(0) is exactly one");
    assert_eq!(s[5], 1.0, "exp(-0) is exactly one");

    let sq = v(&[INF, -INF, NAN, -1.0, -0.0, 4.0]).sqrt();
    let q = sq.as_slice().expect("contiguous");
    assert_eq!(q[0], INF);
    assert!(q[1].is_nan(), "sqrt(-inf) is nan");
    assert!(q[2].is_nan());
    assert!(q[3].is_nan(), "sqrt(-1) is nan");
    assert_eq!(
        q[4].to_bits(),
        (-0.0f64).to_bits(),
        "sqrt(-0) keeps the sign of zero"
    );
    assert_eq!(q[5], 2.0);

    let l = v(&[0.0, -0.0, -1.0, INF, NAN])
        .ln()
        .as_slice()
        .expect("contiguous")
        .to_vec();
    assert_eq!(l[0], -INF, "ln(0) is -inf");
    assert_eq!(l[1], -INF, "ln(-0) is -inf");
    assert!(l[2].is_nan(), "ln(-1) is nan");
    assert_eq!(l[3], INF);
    assert!(l[4].is_nan());

    // Trig at the edges: tan(+-inf) and asin outside [-1, 1] are NaN, atan(inf)
    // is the right angle, tan(pi/4) is one to within rounding.
    let trig = v(&[INF, -INF, 2.0, -2.0])
        .asin()
        .as_slice()
        .expect("contiguous")
        .to_vec();
    assert!(trig[0].is_nan() && trig[1].is_nan() && trig[2].is_nan() && trig[3].is_nan());
    assert!(v(&[INF]).tan().as_slice().expect("contiguous")[0].is_nan());
    assert_eq!(
        v(&[INF]).atan().as_slice().expect("contiguous")[0],
        core::f64::consts::FRAC_PI_2
    );
    let tan45 = v(&[core::f64::consts::FRAC_PI_4])
        .tan()
        .as_slice()
        .expect("contiguous")[0];
    assert!((tan45 - 1.0).abs() <= 2.0 * f64::EPSILON);

    // pow/atan2 edge cases.
    let p = v(&[0.0, -1.0, 2.0, 4.0])
        .pow(&v(&[-1.0, 0.5, 10.0, 0.5]))
        .expect("broadcast");
    let ps = p.as_slice().expect("contiguous");
    assert_eq!(ps[0], INF, "0 ** -1 is inf");
    assert!(ps[1].is_nan(), "(-1) ** 0.5 is nan");
    assert_eq!(ps[2], 1024.0);
    assert_eq!(ps[3], 2.0, "pow(4, 0.5) is exactly two");
    let a = v(&[0.0, -0.0, 1.0, -1.0])
        .atan2(&v(&[-1.0, 1.0, 0.0, 0.0]))
        .expect("broadcast");
    let as_ = a.as_slice().expect("contiguous");
    assert_eq!(as_[0], core::f64::consts::PI);
    assert_eq!(as_[1], 0.0);
    assert_eq!(as_[2], core::f64::consts::FRAC_PI_2);
    assert_eq!(as_[3], -core::f64::consts::FRAC_PI_2);
}

#[test]
fn elementwise_strided_matches_contiguous() {
    let x = t2([2, 3], &[0.0, 1.0, 4.0, 9.0, 16.0, 25.0]);
    let view = x.transpose(None).expect("2-D transpose");
    assert!(!view.is_contiguous());
    let strided = view.exp();
    let materialized = view.contiguous().exp();
    assert_eq!(strided.shape(), &[3, 2]);
    assert_eq!(
        strided.as_slice().expect("contiguous"),
        materialized.as_slice().expect("contiguous")
    );

    // Slicing keeps the same guarantee for the statistics layer.
    let sliced = x.slice(&[(0, 2), (1, 3)]).expect("sub-view");
    let a = sliced.var(None, 1).expect("variance");
    let b = sliced.contiguous().var(None, 1).expect("variance");
    assert_eq!(
        a.as_slice().expect("contiguous"),
        b.as_slice().expect("contiguous")
    );
}

#[test]
fn pow_broadcasts_like_numpy() {
    let col = t2([3, 1], &[2.0, 3.0, 4.0]);
    let row = t2([1, 4], &[0.0, 1.0, 2.0, 3.0]);
    let out = col.pow(&row).expect("broadcast");
    assert_eq!(out.shape(), &[3, 4]);
    let expect = [
        1.0, 2.0, 4.0, 8.0, //
        1.0, 3.0, 9.0, 27.0, //
        1.0, 4.0, 16.0, 64.0,
    ];
    assert_eq!(out.as_slice().expect("contiguous"), &expect);
    assert!(col.pow(&t2([2, 2], &[1.0, 2.0, 3.0, 4.0])).is_err());
}

// ----------------------------------------------------------------- statistics

#[test]
fn moments_along_each_axis() {
    let x = t2(
        [3, 4],
        &[
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
        ],
    );
    let mean_all = x.mean(None).expect("mean");
    assert_eq!(mean_all.shape(), &[]);
    assert_eq!(mean_all.as_slice().expect("contiguous"), &[6.5]);

    let mean_cols = x.mean(Some(0)).expect("mean");
    assert_eq!(mean_cols.shape(), &[4]);
    assert_eq!(
        mean_cols.as_slice().expect("contiguous"),
        &[5.0, 6.0, 7.0, 8.0]
    );

    let var_rows = x.var(Some(1), 0).expect("variance");
    assert_eq!(var_rows.shape(), &[3]);
    // Population variance of 1,2,3,4 is 1.25.
    assert!((var_rows.as_slice().expect("contiguous")[0] - 1.25).abs() < 1e-15);

    let sample = x.var(None, 1).expect("sample variance");
    let population = x.var(None, 0).expect("population variance");
    let n = 12.0;
    assert!(
        (sample.as_slice().expect("contiguous")[0]
            - population.as_slice().expect("contiguous")[0] * n / (n - 1.0))
            .abs()
            < 1e-12
    );
    let std = x.std(None, 0).expect("std");
    assert!((std.as_slice().expect("contiguous")[0] - 3.452_052_529_534_663).abs() < 1e-14);
}

#[test]
fn moments_edge_cases() {
    // Empty reductions are errors, not nan: this crate refuses to invent a
    // value where NumPy warns.
    let empty = Tensor::<f64>::zeros(&[0]);
    assert!(empty.mean(None).is_err());
    assert!(empty.var(None, 0).is_err());
    assert!(empty.std(None, 0).is_err());
    // A zero-length axis with a non-zero count reduces to an empty result.
    let wide = Tensor::<f64>::zeros(&[3, 0]);
    assert_eq!(wide.var(Some(0), 0).expect("variance").shape(), &[0]);

    // ddof must leave at least one degree of freedom.
    let single = v(&[2.0]);
    assert!(single.var(None, 1).is_err());
    assert_eq!(
        single
            .var(None, 0)
            .expect("variance")
            .as_slice()
            .expect("contiguous"),
        &[0.0]
    );
    assert_eq!(
        single
            .mean(None)
            .expect("mean")
            .as_slice()
            .expect("contiguous"),
        &[2.0]
    );

    // NaN and Inf poison a lane the way IEEE arithmetic does.
    let poisoned = v(&[1.0, NAN, 3.0]);
    assert!(
        poisoned
            .mean(None)
            .expect("mean")
            .as_slice()
            .expect("contiguous")[0]
            .is_nan()
    );
    assert!(
        poisoned
            .var(None, 0)
            .expect("variance")
            .as_slice()
            .expect("contiguous")[0]
            .is_nan()
    );
    let infinite = v(&[1.0, INF]);
    assert!(
        infinite
            .mean(None)
            .expect("mean")
            .as_slice()
            .expect("contiguous")[0]
            .is_infinite()
    );
    // Inf - Inf makes the variance nan, not inf.
    assert!(
        infinite
            .var(None, 0)
            .expect("variance")
            .as_slice()
            .expect("contiguous")[0]
            .is_nan()
    );

    // float16 means accumulate in f64 and round once.
    let h = Tensor::<F16>::from_vec(
        vec![2, 2],
        vec![
            F16::from_f64(1.0),
            F16::from_f64(2.0),
            F16::from_f64(3.0),
            F16::from_f64(4.0),
        ],
    )
    .expect("shape");
    let hm = h.mean(Some(0)).expect("mean");
    assert_eq!(hm.as_slice().expect("contiguous")[0].to_f64(), 2.0);
    assert_eq!(hm.as_slice().expect("contiguous")[1].to_f64(), 3.0);
}

// ---------------------------------------------------------------------- dot

#[test]
fn dot_covers_the_four_shape_combinations() {
    let a = t2([2, 3], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let b = t2([3, 2], &[7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
    assert_eq!(
        a.dot(&b)
            .expect("2-D x 2-D")
            .as_slice()
            .expect("contiguous"),
        &[58.0, 64.0, 139.0, 154.0]
    );
    let vec3 = v(&[1.0, 1.0, 1.0]);
    assert_eq!(
        a.dot(&vec3)
            .expect("2-D x 1-D")
            .as_slice()
            .expect("contiguous"),
        &[6.0, 15.0]
    );
    assert_eq!(
        vec3.dot(&b)
            .expect("1-D x 2-D")
            .as_slice()
            .expect("contiguous"),
        &[27.0, 30.0]
    );
    let dot = vec3.dot(&vec3).expect("1-D x 1-D");
    assert_eq!(dot.shape(), &[] as &[usize]);
    assert_eq!(dot.as_slice().expect("contiguous"), &[3.0]);

    // Zero-length inner dimension is a zero, not an error.
    let zero = Tensor::<f64>::zeros(&[0]);
    assert_eq!(
        zero.dot(&zero)
            .expect("empty dot")
            .as_slice()
            .expect("contiguous"),
        &[0.0]
    );
    // Mismatched and 3-D operands are refused.
    assert!(a.dot(&t2([2, 2], &[1.0, 2.0, 3.0, 4.0])).is_err());
    assert!(Tensor::<f64>::zeros(&[2, 2, 2]).dot(&a).is_err());
    // The left operand may be a strided view.
    let transposed = a.transpose(None).expect("transpose");
    assert_eq!(
        transposed
            .dot(&t2([2, 2], &[1.0, 0.0, 0.0, 1.0]))
            .expect("3x2 x 2x2")
            .shape(),
        &[3, 2]
    );
}

// -------------------------------------------------------------------- norms

#[test]
fn norms_match_the_definitions() {
    let x = v(&[3.0, -4.0]);
    assert_eq!(x.norm().expect("2-norm"), 5.0);
    assert_eq!(x.norm_p(NormOrd::One).expect("1-norm"), 7.0);
    assert_eq!(x.norm_p(NormOrd::Infinity).expect("inf-norm"), 4.0);
    let m = t2([2, 2], &[1.0, 2.0, 3.0, 4.0]);
    assert!((m.norm().expect("frobenius") - 30.0f64.sqrt()).abs() < 1e-15);
    assert_eq!(m.norm_p(NormOrd::One).expect("elementwise"), 10.0);
    assert_eq!(m.norm_p(NormOrd::Infinity).expect("elementwise"), 4.0);
    // Empty sums are zero, not nan.
    assert_eq!(Tensor::<f64>::zeros(&[0]).norm().expect("empty norm"), 0.0);
    // Inf and NaN propagate.
    assert!(v(&[1.0, NAN]).norm().expect("norm").is_nan());
    assert_eq!(v(&[1.0, INF]).norm().expect("norm"), INF);
}

// ------------------------------------------------------------------- linalg

#[test]
fn inverse_determinant_and_solve() {
    // 1x1 is the smallest case: 1/2 and det 2.
    let one = t2([1, 1], &[2.0]);
    assert_eq!(
        one.inv().expect("inverse").as_slice().expect("contiguous"),
        &[0.5]
    );
    assert_eq!(one.det().expect("determinant"), 2.0);
    assert_eq!(
        one.solve(&v(&[6.0]))
            .expect("solve")
            .as_slice()
            .expect("contiguous"),
        &[3.0]
    );

    let a = t2([2, 2], &[4.0, 7.0, 2.0, 6.0]);
    assert!((a.det().expect("det") - 10.0).abs() < 1e-14);
    let a_inv = a.inv().expect("inverse");
    let product = a.dot(&a_inv).expect("product");
    for (i, got) in product.as_slice().expect("contiguous").iter().enumerate() {
        let want = if i % 3 == 0 { 1.0 } else { 0.0 };
        assert!((got - want).abs() < 1e-15, "A * A^-1 at {i}: {got}");
    }

    // Multiple right-hand sides and a negative determinant.
    let b = t2([2, 2], &[1.0, 0.0, 0.0, 1.0]);
    let x = a.solve(&b).expect("solve");
    assert_eq!(x.shape(), &[2, 2]);
    assert_eq!(
        x.as_slice().expect("contiguous"),
        a_inv.as_slice().expect("contiguous")
    );
    let flip = t2([2, 2], &[0.0, 1.0, 1.0, 0.0]);
    assert_eq!(flip.det().expect("det"), -1.0);

    // Singular matrices are reported, not silently inverted.
    let singular = t2([2, 2], &[1.0, 2.0, 2.0, 4.0]);
    assert!(matches!(singular.inv(), Err(Error::BadValue(_))));
    assert!(matches!(
        singular.solve(&v(&[1.0, 2.0])),
        Err(Error::BadValue(_))
    ));
    // Non-square input is refused by every entry point.
    let rect = t2([2, 3], &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    assert!(rect.inv().is_err());
    assert!(rect.det().is_err());
    assert!(rect.lu().is_err());
}

#[test]
fn lu_reconstructs_the_permuted_matrix() {
    let a = t2([3, 3], &[0.0, 2.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 3.0]);
    let lu = a.lu().expect("factorization");
    // L is unit lower triangular, U upper triangular.
    let n = 3;
    let l = lu.l.as_slice().expect("contiguous");
    let u = lu.u.as_slice().expect("contiguous");
    for i in 0..n {
        assert_eq!(l[i * n + i], 1.0, "unit diagonal");
        for j in (i + 1)..n {
            assert_eq!(l[i * n + j], 0.0, "L above the diagonal at [{i},{j}]");
            assert_eq!(u[j * n + i], 0.0, "U below the diagonal at [{j},{i}]");
        }
    }
    for i in 0..n {
        for j in 0..n {
            let mut acc = 0.0;
            for k in 0..n {
                acc += l[i * n + k] * u[k * n + j];
            }
            let source = a.as_slice().expect("contiguous")[lu.perm[i] * n + j];
            assert!(
                (acc - source).abs() < 1e-14,
                "L*U at [{i},{j}]: {acc} vs {source}"
            );
        }
    }
    // The permutation is a permutation.
    let mut seen = [false; 3];
    for &p in &lu.perm {
        assert!(!seen[p], "row {p} used twice");
        seen[p] = true;
    }
}

// ------------------------------------------------------------------- manip

#[test]
fn concat_stack_and_pad() {
    let a = t2([2, 2], &[1.0, 2.0, 3.0, 4.0]);
    let b = t2([2, 2], &[5.0, 6.0, 7.0, 8.0]);

    let rows = concat(&[a.clone(), b.clone()], 0).expect("axis 0");
    assert_eq!(rows.shape(), &[4, 2]);
    assert_eq!(
        rows.as_slice().expect("contiguous"),
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
    );
    let cols = concat(&[a.clone(), b.clone()], 1).expect("axis 1");
    assert_eq!(cols.shape(), &[2, 4]);
    assert_eq!(
        cols.as_slice().expect("contiguous"),
        &[1.0, 2.0, 5.0, 6.0, 3.0, 4.0, 7.0, 8.0]
    );

    let stacked = stack(&[a.clone(), b.clone()], 0).expect("stack axis 0");
    assert_eq!(stacked.shape(), &[2, 2, 2]);
    let stacked_last = stack(&[a.clone(), b.clone()], 2).expect("stack past the last axis");
    assert_eq!(stacked_last.shape(), &[2, 2, 2]);
    assert_eq!(
        stacked_last.as_slice().expect("contiguous"),
        &[1.0, 5.0, 2.0, 6.0, 3.0, 7.0, 4.0, 8.0]
    );

    // Mismatched shapes and out-of-range axes are refused.
    assert!(concat(&[a.clone(), Tensor::<f64>::zeros(&[2, 3])], 0).is_err());
    assert!(concat(&[a.clone(), b.clone()], 2).is_err());
    assert!(stack(&[a.clone(), Tensor::<f64>::zeros(&[2, 3])], 0).is_err());
    assert!(stack(&[a.clone(), b.clone()], 3).is_err());
    assert!(concat(&[] as &[Tensor<f64>], 0).is_err());

    // Padding: one element of the input survives, everything else is the value.
    let padded = v(&[7.0]).pad(&[(1, 2)], -1.0).expect("pad");
    assert_eq!(
        padded.as_slice().expect("contiguous"),
        &[-1.0, 7.0, -1.0, -1.0]
    );
    let no_pad = a.pad(&[(0, 0), (0, 0)], 0.0).expect("zero pad");
    assert_eq!(
        no_pad.as_slice().expect("contiguous"),
        a.as_slice().expect("contiguous")
    );
    assert!(a.pad(&[(0, 0)], 0.0).is_err(), "one width per axis");
    // A zero-sized input pads to a constant tensor.
    let filled = Tensor::<f64>::zeros(&[0])
        .pad(&[(2, 2)], 3.0)
        .expect("pad empty");
    assert_eq!(filled.shape(), &[4]);
    assert_eq!(
        filled.as_slice().expect("contiguous"),
        &[3.0, 3.0, 3.0, 3.0]
    );
    // Strided input pads by value, not by layout.
    let view = a.transpose(None).expect("transpose");
    let padded_view = view.pad(&[(1, 0), (0, 1)], 0.0).expect("pad view");
    assert_eq!(padded_view.shape(), &[3, 3]);
    assert_eq!(
        padded_view.as_slice().expect("contiguous"),
        &[0.0, 0.0, 0.0, 1.0, 3.0, 0.0, 2.0, 4.0, 0.0]
    );
}

// -------------------------------------------------------------------- casts

#[test]
fn astype_matches_rust_cast_semantics() {
    // Floats truncate toward zero and saturate at the integer bounds.
    let x = v(&[2.9, -2.9, 1e10, -1e10, NAN, INF, -INF]);
    let as_i32 = x.astype::<i32>().as_slice().expect("contiguous").to_vec();
    assert_eq!(
        as_i32,
        vec![2, -2, i32::MAX, i32::MIN, 0, i32::MAX, i32::MIN]
    );
    let as_u8 = v(&[2.9, -2.9, 300.0, NAN, -1.0])
        .astype::<u8>()
        .as_slice()
        .expect("contiguous")
        .to_vec();
    assert_eq!(as_u8, vec![2, 0, 255, 0, 0]);
    let as_i8 = v(&[127.9, -128.9, 300.0])
        .astype::<i8>()
        .as_slice()
        .expect("contiguous")
        .to_vec();
    assert_eq!(as_i8, vec![127, -128, 127]);

    // Integers widen exactly, and bool is a zero test.
    assert_eq!(
        Tensor::<i32>::from_vec(vec![3], vec![1, -1, i32::MAX])
            .expect("shape")
            .astype::<f64>()
            .as_slice()
            .expect("contiguous"),
        &[1.0, -1.0, 2147483647.0]
    );
    assert_eq!(
        v(&[0.0, -0.0, 1e-9, -2.0])
            .astype::<bool>()
            .as_slice()
            .expect("contiguous"),
        &[false, false, true, true]
    );
    // Round trip through float16 keeps the exact small integers.
    let u8s = Tensor::<u8>::from_vec(vec![3], vec![0, 1, 255]).expect("shape");
    let halves = u8s.astype::<F16>();
    assert_eq!(halves.as_slice().expect("contiguous")[2].to_f64(), 255.0);
    assert_eq!(
        halves.astype::<u8>().as_slice().expect("contiguous"),
        &[0, 1, 255]
    );
}

#[test]
fn astype_on_views_and_empty_buffers() {
    let a = t2([2, 3], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let view = a.transpose(None).expect("transpose");
    assert!(!view.is_contiguous());
    let converted = view.astype::<i32>();
    assert_eq!(converted.shape(), &[3, 2]);
    assert_eq!(
        converted.as_slice().expect("contiguous"),
        &[1, 4, 2, 5, 3, 6]
    );
    let empty = Tensor::<f64>::zeros(&[0, 2]).astype::<i32>();
    assert_eq!(empty.shape(), &[0, 2]);
    assert!(empty.as_slice().expect("contiguous").is_empty());
}

#[test]
fn float16_conversion_matches_the_ieee_patterns() {
    // Values from the binary16 specification and NumPy's float16.
    let cases: &[(f64, u16)] = &[
        (0.0, 0x0000),
        (-0.0, 0x8000),
        (1.0, 0x3c00),
        (-2.0, 0xc000),
        (0.5, 0x3800),
        (65504.0, 0x7bff),                     // largest finite
        (65519.0, 0x7bff),                     // still below the tie
        (65520.0, 0x7c00),                     // tie rounds to infinity
        (1e6, 0x7c00),                         // overflow
        (-1e6, 0xfc00),                        // overflow keeps the sign
        (6.103_515_625e-05, 0x0400),           // smallest normal, 2^-14
        (5.960_464_477_539_063e-08, 0x0001),   // smallest subnormal, 2^-24
        (2.980_232_238_769_531_5e-08, 0x0000), // half a subnormal ties to even zero
        (8.940_696_716_308_594e-08, 0x0002),   // 1.5 subnormals ties to even two
        (1e-8, 0x0000),                        // underflow
        (0.1, 0x2e66),
        (1.0 / 3.0, 0x3555),
        (INF, 0x7c00),
        (-INF, 0xfc00),
    ];
    for &(value, bits) in cases {
        assert_eq!(
            F16::from_f64(value).to_bits(),
            bits,
            "float16 of {value:e} should be {bits:#06x}"
        );
    }
    assert!(F16::from_f64(NAN).is_nan());
    assert!(F16::from_bits(0x7c00).to_f32().is_infinite());
    assert_eq!(F16::from_bits(0x0001).to_f64(), 5.960_464_477_539_063e-08);
    assert_eq!(F16::from_f64(-0.0).to_bits(), 0x8000);
    // Narrowing through float32 agrees with narrowing from float64 directly.
    for x in [0.1f64, 1.0 / 3.0, 1234.5, 1e-7, -7.25] {
        assert_eq!(
            F16::from_f32(x as f32).to_bits(),
            F16::from_f64(x).to_bits(),
            "value {x}"
        );
    }
}

#[test]
fn bfloat16_conversion_matches_the_truncation_rule() {
    let cases: &[(f64, u16)] = &[
        (0.0, 0x0000),
        (-0.0, 0x8000),
        (1.0, 0x3f80),
        (-2.0, 0xc000),
        (0.5, 0x3f00),
        (3.140_625, 0x4049), // f32 0x4048f5c3 rounds the low half up
        (1.0 / 3.0, 0x3eab), // f32 0x3eaaaaab rounds up
        (0.1, 0x3dcd),       // f32 0x3dcccccd rounds up
        (1e-40, 0x0001),     // below float32 range: flushed through f32
        (3.4e38, 0x7f80),    // float32 0x7f7fffff rounds the low half up to infinity
        (1e39, 0x7f80),      // float32 overflow first
        (INF, 0x7f80),
        (-INF, 0xff80),
    ];
    for &(value, bits) in cases {
        assert_eq!(
            Bf16::from_f64(value).to_bits(),
            bits,
            "bfloat16 of {value:e} should be {bits:#06x}"
        );
    }
    // An exact tie keeps the even mantissa.
    assert_eq!(
        Bf16::from_f32(f32::from_bits(0x3f80_8000)).to_bits(),
        0x3f80
    );
    assert_eq!(
        Bf16::from_f32(f32::from_bits(0x3f81_8000)).to_bits(),
        0x3f82
    );
    assert!(Bf16::from_f64(NAN).is_nan());
    assert!(Bf16::from_f64(1e39).to_f32().is_infinite());
    assert_eq!(Bf16::from_bits(0x3f80).to_f64(), 1.0);
    // Half <-> bfloat16 goes through the common value, not the bit pattern.
    assert_eq!(F16::from_f64(0.5).to_bits(), 0x3800);
    assert_eq!(F16::from_bits(0x3800).to_f32(), 0.5);
    assert_eq!(
        Bf16::from_f32(F16::from_f64(0.5).to_f32()).to_bits(),
        0x3f00
    );
}
