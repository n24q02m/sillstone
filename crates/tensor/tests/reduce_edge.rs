//! Reduction edge cases: NaN placement, `argmin`, float `mean`, bool
//! tensors and zero-sized shapes. NaN results must not depend on where in
//! the lane the NaN sits.

use sillstone_tensor::Tensor;

fn scalar<T: sillstone_tensor::Dtype>(t: &Tensor<T>) -> T {
    assert_eq!(t.ndim(), 0);
    t.get(&[]).unwrap()
}

#[test]
fn nan_propagates_wherever_it_sits() {
    for nan_at in 0..4 {
        let mut v = vec![3.0f64, 1.0, 2.0, 0.5];
        v[nan_at] = f64::NAN;
        let t = Tensor::from_vec(vec![4], v).unwrap();
        assert!(
            scalar(&t.max(None).unwrap()).is_nan(),
            "max, NaN at {nan_at}"
        );
        assert!(
            scalar(&t.min(None).unwrap()).is_nan(),
            "min, NaN at {nan_at}"
        );
        assert!(
            scalar(&t.max(Some(0)).unwrap()).is_nan(),
            "max axis, {nan_at}"
        );
        assert!(
            scalar(&t.min(Some(0)).unwrap()).is_nan(),
            "min axis, {nan_at}"
        );
        // The index is the first NaN, as NumPy reports.
        assert_eq!(scalar(&t.argmax(None).unwrap()), nan_at);
        assert_eq!(scalar(&t.argmin(None).unwrap()), nan_at);
        assert_eq!(scalar(&t.argmax(Some(0)).unwrap()), nan_at);
        assert_eq!(scalar(&t.argmin(Some(0)).unwrap()), nan_at);
    }
}

#[test]
fn nan_only_poisons_its_own_lane() {
    // [[1, NaN], [4, 2]]: column 1 is poisoned, column 0 is not; row 0 is
    // poisoned, row 1 is not.
    let t = Tensor::from_vec(vec![2, 2], vec![1.0f32, f32::NAN, 4.0, 2.0]).unwrap();
    let col_max = t.max(Some(0)).unwrap();
    let col_max = col_max.as_slice().unwrap();
    assert_eq!(col_max[0], 4.0);
    assert!(col_max[1].is_nan());
    let row_min = t.min(Some(1)).unwrap();
    let row_min = row_min.as_slice().unwrap();
    assert!(row_min[0].is_nan());
    assert_eq!(row_min[1], 2.0);
    assert_eq!(t.argmax(Some(0)).unwrap().as_slice(), Some(&[1, 0][..]));
    assert_eq!(t.argmin(Some(1)).unwrap().as_slice(), Some(&[1, 1][..]));
}

#[test]
fn argmin_takes_the_first_of_equal_minima_on_views() {
    // [[5, 1, 1], [1, 7, 1]]
    let t = Tensor::from_vec(vec![2, 3], vec![5i64, 1, 1, 1, 7, 1]).unwrap();
    assert_eq!(scalar(&t.argmin(None).unwrap()), 1);
    assert_eq!(t.argmin(Some(0)).unwrap().as_slice(), Some(&[1, 0, 0][..]));
    assert_eq!(t.argmin(Some(1)).unwrap().as_slice(), Some(&[1, 0][..]));
    let tr = t.transpose(None).unwrap();
    assert_eq!(tr.argmin(Some(0)).unwrap().as_slice(), Some(&[1, 0][..]));
    assert_eq!(tr.argmin(Some(1)).unwrap().as_slice(), Some(&[1, 0, 0][..]));
    // Flattened index follows the view's own C order, not the buffer's.
    assert_eq!(scalar(&tr.argmin(None).unwrap()), 1);
}

#[test]
fn argmin_and_mean_reject_what_they_cannot_answer() {
    let t = Tensor::from_vec(vec![2, 2], vec![1.0f64, 2.0, 3.0, 4.0]).unwrap();
    assert!(t.argmin(Some(2)).is_err());
    assert!(t.mean(Some(2)).is_err());
    let empty = Tensor::<f64>::zeros(&[0, 3]);
    assert!(empty.argmin(None).is_err());
    assert!(empty.mean(None).is_err());
    assert!(empty.mean(Some(0)).is_err());
    // Averaging over a non-empty axis of a zero-row-count tensor is fine.
    let m = empty.mean(Some(1)).unwrap();
    assert_eq!(m.shape(), &[0]);
    assert_eq!(m.numel(), 0);
    // A scalar has no axis to reduce.
    let s = t.sum(None).unwrap();
    assert!(s.mean(Some(0)).is_err());
    assert_eq!(scalar(&s.mean(None).unwrap()), 10.0);
}

#[test]
fn mean_matches_sum_over_count_on_views() {
    let t = Tensor::from_vec(vec![2, 3], vec![1.0f64, 2.0, 3.0, 4.0, 5.0, 9.0]).unwrap();
    assert_eq!(scalar(&t.mean(None).unwrap()), 24.0 / 6.0);
    assert_eq!(
        t.mean(Some(0)).unwrap().as_slice(),
        Some(&[2.5, 3.5, 6.0][..])
    );
    assert_eq!(t.mean(Some(1)).unwrap().as_slice(), Some(&[2.0, 6.0][..]));
    let tr = t.transpose(None).unwrap();
    assert_eq!(tr.mean(Some(0)).unwrap().as_slice(), Some(&[2.0, 6.0][..]));
    assert_eq!(
        tr.mean(Some(1)).unwrap().as_slice(),
        Some(&[2.5, 3.5, 6.0][..])
    );
    let f = Tensor::from_vec(vec![3], vec![0.5f32, 1.5, 4.0]).unwrap();
    assert_eq!(scalar(&f.mean(None).unwrap()), 2.0f32);
}

#[test]
fn bool_tensors_order_false_before_true() {
    let t = Tensor::from_vec(vec![2, 2], vec![false, true, true, false]).unwrap();
    assert!(scalar(&t.max(None).unwrap()));
    assert!(!scalar(&t.min(None).unwrap()));
    assert_eq!(t.argmax(Some(0)).unwrap().as_slice(), Some(&[1, 0][..]));
    assert_eq!(t.argmin(Some(1)).unwrap().as_slice(), Some(&[0, 1][..]));
}

#[test]
fn zero_sized_sums_hold_exactly_their_elements() {
    let e = Tensor::<i64>::zeros(&[0, 3]);
    let along_cols = e.sum(Some(1)).unwrap();
    assert_eq!(along_cols.shape(), &[0]);
    assert_eq!(along_cols.numel(), 0);
    assert_eq!(along_cols.contiguous().as_slice(), Some(&[][..]));
    let three_d = Tensor::<i64>::zeros(&[2, 0, 2]);
    let r = three_d.sum(Some(1)).unwrap();
    assert_eq!(r.shape(), &[2, 2]);
    assert_eq!(r.as_slice(), Some(&[0, 0, 0, 0][..]));
    assert!(three_d.max(Some(0)).is_err());
}

#[test]
fn a_zero_dimensional_tensor_reduces_to_itself() {
    let s = Tensor::from_vec(vec![], vec![7i64]).unwrap();
    assert_eq!(scalar(&s.sum(None).unwrap()), 7);
    assert_eq!(scalar(&s.max(None).unwrap()), 7);
    assert_eq!(scalar(&s.min(None).unwrap()), 7);
    assert_eq!(scalar(&s.argmax(None).unwrap()), 0);
    assert_eq!(scalar(&s.argmin(None).unwrap()), 0);
    assert!(s.sum(Some(0)).is_err());
    assert!(s.argmax(Some(0)).is_err());
}
