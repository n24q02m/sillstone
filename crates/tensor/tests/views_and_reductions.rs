//! Consumer-level conformance for strided views, elementwise ops, reductions
//! and `matmul`: every reduction must see exactly the elements the view
//! exposes, never the (possibly larger) backing buffer.

use sillstone_tensor::{Dtype, Tensor};

fn mat() -> Tensor<i64> {
    // [[1, 2, 3], [4, 5, 6]]
    Tensor::from_vec(vec![2, 3], vec![1, 2, 3, 4, 5, 6]).unwrap()
}

fn scalar<T: Dtype>(t: &Tensor<T>) -> T {
    assert_eq!(t.ndim(), 0);
    t.get(&[]).unwrap()
}

#[test]
fn constructors_and_accessors() {
    assert!(Tensor::<i64>::from_vec(vec![2, 2], vec![1, 2, 3]).is_err());
    let z = Tensor::<f64>::zeros(&[2, 2]);
    assert_eq!(z.as_slice(), Some(&[0.0, 0.0, 0.0, 0.0][..]));
    let o = Tensor::<i32>::ones(&[3]);
    assert_eq!(o.as_slice(), Some(&[1, 1, 1][..]));
    let f = Tensor::full(&[2, 1], 7u8);
    assert_eq!(f.as_slice(), Some(&[7, 7][..]));
    assert!(format!("{f:?}").contains("shape=[2, 1]"));
    // out-of-range and wrong-rank reads are None, not a panic
    assert_eq!(mat().get(&[2, 0]), None);
    assert_eq!(mat().get(&[0]), None);
    assert_eq!(mat().get(&[0, 3]), None);
}

#[test]
fn reshape_shares_or_copies_and_validates() {
    let t = mat();
    let r = t.reshape(&[3, 2]).unwrap();
    assert_eq!(r.shape(), &[3, 2]);
    assert_eq!(r.get(&[2, 1]), Some(6));
    assert!(t.reshape(&[4, 2]).is_err());

    // A transposed (non-contiguous) tensor reshapes through a copy.
    let tr = t.transpose(None).unwrap();
    let flat = tr.reshape(&[6]).unwrap();
    assert_eq!(flat.as_slice(), Some(&[1, 4, 2, 5, 3, 6][..]));
}

#[test]
fn transpose_and_slice_validate_arguments() {
    let t = mat();
    assert!(t.transpose(Some(&[0])).is_err());
    assert!(t.transpose(Some(&[0, 2])).is_err());
    let swapped = t.transpose(Some(&[1, 0])).unwrap();
    assert_eq!(swapped.shape(), &[3, 2]);

    assert!(t.slice(&[(0, 2)]).is_err());
    assert!(t.slice(&[(1, 0), (0, 3)]).is_err());
    assert!(t.slice(&[(0, 3), (0, 3)]).is_err());
}

#[test]
fn reductions_on_a_leading_row_slice_ignore_the_rest_of_the_buffer() {
    // Rows [0..1] keeps offset 0 and C-strides, so the view reports itself
    // contiguous while the backing buffer still holds all six elements.
    let row = mat().slice(&[(0, 1), (0, 3)]).unwrap();
    assert_eq!(row.shape(), &[1, 3]);
    assert_eq!(scalar(&row.sum(None).unwrap()), 6);
    assert_eq!(scalar(&row.max(None).unwrap()), 3);
    assert_eq!(scalar(&row.min(None).unwrap()), 1);
    assert_eq!(scalar(&row.argmax(None).unwrap()), 2);
    assert_eq!(row.contiguous().as_slice(), Some(&[1, 2, 3][..]));
}

#[test]
fn global_reductions_on_a_strided_view() {
    let tr = mat().transpose(None).unwrap();
    assert_eq!(scalar(&tr.sum(None).unwrap()), 21);
    assert_eq!(scalar(&tr.max(None).unwrap()), 6);
    assert_eq!(scalar(&tr.min(None).unwrap()), 1);
    assert_eq!(scalar(&tr.argmax(None).unwrap()), 5);
}

#[test]
fn axis_reductions_cover_sum_min_max_argmax() {
    let t = Tensor::from_vec(vec![2, 3], vec![1.0f64, 5.0, 2.0, 6.0, 3.0, 4.0]).unwrap();
    assert_eq!(t.sum(Some(1)).unwrap().as_slice(), Some(&[8.0, 13.0][..]));
    assert_eq!(
        t.min(Some(0)).unwrap().as_slice(),
        Some(&[1.0, 3.0, 2.0][..])
    );
    assert_eq!(t.min(Some(1)).unwrap().as_slice(), Some(&[1.0, 3.0][..]));
    assert_eq!(
        t.max(Some(0)).unwrap().as_slice(),
        Some(&[6.0, 5.0, 4.0][..])
    );
    assert_eq!(t.argmax(Some(0)).unwrap().as_slice(), Some(&[1, 0, 1][..]));
    // Ties keep the first index, like NumPy.
    let tie = Tensor::from_vec(vec![4], vec![3, 9, 9, 1]).unwrap();
    assert_eq!(scalar(&tie.argmax(None).unwrap()), 1);
    assert_eq!(scalar(&tie.argmax(Some(0)).unwrap()), 1);
    assert_eq!(scalar(&tie.min(Some(0)).unwrap()), 1);
    assert_eq!(scalar(&tie.max(Some(0)).unwrap()), 9);
    assert_eq!(scalar(&tie.sum(Some(0)).unwrap()), 22);
}

#[test]
fn reductions_reject_bad_axes_and_empty_tensors() {
    let t = mat();
    assert!(t.sum(Some(2)).is_err());
    assert!(t.max(Some(2)).is_err());
    assert!(t.min(Some(2)).is_err());
    assert!(t.argmax(Some(2)).is_err());

    let empty = Tensor::<i64>::zeros(&[0, 3]);
    assert!(empty.max(None).is_err());
    assert!(empty.min(None).is_err());
    assert!(empty.argmax(None).is_err());
    assert!(empty.max(Some(0)).is_err());
}

#[test]
fn elementwise_ops_with_broadcasting_and_views() {
    let a = mat();
    let b = Tensor::from_vec(vec![3], vec![10, 20, 30]).unwrap();
    assert_eq!(
        a.sub(&b).unwrap().as_slice(),
        Some(&[-9, -18, -27, -6, -15, -24][..])
    );
    assert_eq!(a.neg().as_slice(), Some(&[-1, -2, -3, -4, -5, -6][..]));

    let x = Tensor::from_vec(vec![2, 1], vec![8.0f64, 9.0]).unwrap();
    let y = Tensor::from_vec(vec![2], vec![2.0f64, 4.0]).unwrap();
    assert_eq!(
        x.div(&y).unwrap().as_slice(),
        Some(&[4.0, 2.0, 4.5, 2.25][..])
    );

    // Ops read through the view's strides, not the raw buffer order.
    let tr = a.transpose(None).unwrap();
    let ones = Tensor::<i64>::ones(&[3, 2]);
    assert_eq!(
        tr.add(&ones).unwrap().as_slice(),
        Some(&[2, 5, 3, 6, 4, 7][..])
    );
    assert_eq!(tr.neg().as_slice(), Some(&[-1, -4, -2, -5, -3, -6][..]));

    // Incompatible shapes fail instead of guessing.
    assert!(a.add(&Tensor::<i64>::ones(&[2])).is_err());
    assert!(a.broadcast_to(&[3]).is_err());
    assert!(a.broadcast_to(&[2, 4]).is_err());
}

#[test]
fn comparisons_scalars_and_zero_dimensional_tensors() {
    let a = Tensor::from_vec(vec![3], vec![1, 2, 3]).unwrap();
    let b = Tensor::from_vec(vec![3], vec![1, 0, 3]).unwrap();
    assert_eq!(a.eq(&b).unwrap().as_slice(), Some(&[true, false, true][..]));
    assert_eq!(
        a.ne(&b).unwrap().as_slice(),
        Some(&[false, true, false][..])
    );

    let s = Tensor::from_vec(vec![], vec![5i64]).unwrap();
    let t = Tensor::from_vec(vec![], vec![7i64]).unwrap();
    assert_eq!(scalar(&s.add(&t).unwrap()), 12);
    assert_eq!(scalar(&s.neg()), -5);
    assert_eq!(scalar(&s.contiguous()), 5);
    let lifted = s.add(&a).unwrap();
    assert_eq!(lifted.as_slice(), Some(&[6, 7, 8][..]));
}

#[test]
fn zero_sized_tensors_pass_through_without_panicking() {
    let e = Tensor::<i64>::zeros(&[0, 3]);
    assert_eq!(e.numel(), 0);
    assert_eq!(e.neg().shape(), &[0, 3]);
    assert_eq!(e.add(&e).unwrap().numel(), 0);
    assert_eq!(e.transpose(None).unwrap().contiguous().numel(), 0);
    assert_eq!(scalar(&e.sum(None).unwrap()), 0);
    assert_eq!(e.sum(Some(0)).unwrap().shape(), &[3]);
    assert_eq!(e.sum(Some(0)).unwrap().as_slice(), Some(&[0, 0, 0][..]));
}

#[test]
fn matmul_validates_shapes_and_handles_views_and_blocks() {
    let v = Tensor::from_vec(vec![3], vec![1, 2, 3]).unwrap();
    assert!(v.matmul(&v).is_err());
    assert!(mat().matmul(&mat()).is_err());

    // A transposed operand is materialised before the GEMM.
    let a = mat();
    let at = a.transpose(None).unwrap();
    let g = a.matmul(&at).unwrap();
    assert_eq!(g.as_slice(), Some(&[14, 32, 32, 77][..]));

    // Larger than one 32-wide tile in every dimension, versus a naive triple loop.
    let (m, k, n) = (37usize, 35usize, 41usize);
    let av: Vec<i64> = (0..m * k).map(|i| (i as i64 * 7) % 13 - 6).collect();
    let bv: Vec<i64> = (0..k * n).map(|i| (i as i64 * 5) % 11 - 5).collect();
    let got = Tensor::from_vec(vec![m, k], av.clone())
        .unwrap()
        .matmul(&Tensor::from_vec(vec![k, n], bv.clone()).unwrap())
        .unwrap();
    let mut want = vec![0i64; m * n];
    for i in 0..m {
        for j in 0..n {
            for p in 0..k {
                want[i * n + j] += av[i * k + p] * bv[p * n + j];
            }
        }
    }
    assert_eq!(got.as_slice(), Some(&want[..]));
}
