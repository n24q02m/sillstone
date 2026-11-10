//! Ownership transfer in and out of a `Tensor` must not touch the payload.
//!
//! The buffer is identified by its address: a copy lands in a fresh
//! allocation, so a surviving address proves the elements were moved, not
//! duplicated. Capacity is checked too — a copy of a `Vec` sized exactly to
//! its length cannot report a larger capacity than its length.

use std::sync::Arc;

use sillstone_tensor::Tensor;

fn payload(n: usize) -> Vec<f64> {
    (0..n).map(|i| i as f64 + 0.5).collect()
}

#[test]
fn from_vec_takes_ownership_of_the_caller_buffer() {
    let mut data = payload(1024);
    data.reserve(2048);
    let (ptr, cap) = (data.as_ptr(), data.capacity());
    let t = Tensor::from_vec(vec![32, 32], data).unwrap();

    assert_eq!(
        t.as_slice().unwrap().as_ptr(),
        ptr,
        "construction copied the buffer"
    );
    let copied = t.to_vec();
    assert_ne!(copied.as_ptr(), ptr, "to_vec is the copying accessor");
    assert_eq!(copied[0], 0.5);

    let back = t
        .into_data()
        .expect("sole owner must extract without a copy");
    assert_eq!(back.as_ptr(), ptr, "extraction copied the buffer");
    assert_eq!(back.capacity(), cap, "the extraction reallocated");
    assert_eq!(back.len(), 1024);
    assert_eq!(back[1023], 1023.5);
}

#[test]
fn from_arc_shares_an_existing_buffer() {
    let data = Arc::new(payload(64));
    let ptr = data.as_ptr();
    let t = Tensor::from_arc(vec![8, 8], Arc::clone(&data)).unwrap();
    let clone = t.clone();

    assert_eq!(t.as_slice().unwrap().as_ptr(), ptr);
    assert_eq!(data[3], 3.5);
    assert!(
        clone.into_data().is_none(),
        "a shared buffer must not be reported as owned"
    );
    assert!(
        t.into_data().is_none(),
        "the last handle of a shared buffer still sees a clone"
    );
    assert_eq!(data[7], 7.5);
}

#[test]
fn into_data_refuses_strided_and_offset_views() {
    let t = Tensor::from_vec(vec![2, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();

    let view = t.slice(&[(0, 2), (1, 3)]).unwrap();
    let transposed = t.transpose(None).unwrap();

    assert_eq!(view.to_vec(), vec![2.0, 3.0, 5.0, 6.0]);
    assert_eq!(transposed.to_vec(), vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
    assert!(
        view.into_data().is_none(),
        "an offset view is not an owned C-order buffer"
    );
    assert!(transposed.into_data().is_none());
}

#[test]
fn into_data_refuses_a_buffer_with_slack() {
    // A leading row slice: offset 0, C strides, but the buffer still holds row 1.
    let t = Tensor::from_vec(vec![2, 3], vec![1u8, 2, 3, 4, 5, 6]).unwrap();
    let row = t.slice(&[(0, 1), (0, 3)]).unwrap();
    assert!(row.is_contiguous());
    assert_eq!(row.to_vec(), vec![1u8, 2, 3]);
    assert!(row.into_data().is_none());
}

#[test]
fn empty_tensors_round_trip_without_a_payload() {
    let t = Tensor::<f32>::zeros(&[0, 4]);
    assert_eq!(t.to_vec(), Vec::<f32>::new());
    assert_eq!(t.into_data(), Some(Vec::new()));

    let scalar = Tensor::from_vec(vec![], vec![7.0f64]).unwrap();
    assert_eq!(scalar.to_vec(), vec![7.0]);
    let back = scalar
        .into_data()
        .expect("a 0-d tensor owns its single element");
    assert_eq!(back, vec![7.0]);
}

#[test]
fn from_arc_rejects_a_buffer_that_does_not_match_the_shape() {
    let data = Arc::new(payload(6));
    assert!(Tensor::from_arc(vec![2, 2], Arc::clone(&data)).is_err());
    assert!(Tensor::from_arc(vec![2, 3], data).is_ok());
}
