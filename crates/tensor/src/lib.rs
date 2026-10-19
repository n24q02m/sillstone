//! Pure-Rust N-dimensional strided tensor and array computing engine (NumPy replacement).

#![no_std]

extern crate alloc;

pub mod broadcast;
pub mod dtype;
pub mod ffi;
pub mod layout;
pub mod linalg;
pub mod ops;
pub mod reduce;
pub mod tensor;

pub use broadcast::broadcast_shapes;
pub use dtype::Dtype;
pub use layout::c_strides;
pub use tensor::Tensor;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_tensor_basics_and_indexing() {
        let t = Tensor::from_vec(vec![2, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        assert_eq!(t.ndim(), 2);
        assert_eq!(t.numel(), 6);
        assert_eq!(t.shape(), &[2, 3]);
        assert_eq!(t.strides(), &[3, 1]);
        assert!(t.is_contiguous());

        assert_eq!(t.get(&[0, 0]), Some(1.0));
        assert_eq!(t.get(&[0, 2]), Some(3.0));
        assert_eq!(t.get(&[1, 1]), Some(5.0));
        assert_eq!(t.get(&[1, 2]), Some(6.0));
        assert_eq!(t.get(&[2, 0]), None);
    }

    #[test]
    fn test_transpose_and_slice() {
        let t = Tensor::from_vec(vec![2, 3], vec![1, 2, 3, 4, 5, 6]).unwrap();
        let transposed = t.transpose(None).unwrap();
        assert_eq!(transposed.shape(), &[3, 2]);
        assert_eq!(transposed.strides(), &[1, 3]);
        assert!(!transposed.is_contiguous());

        assert_eq!(transposed.get(&[0, 0]), Some(1));
        assert_eq!(transposed.get(&[1, 0]), Some(2));
        assert_eq!(transposed.get(&[2, 0]), Some(3));
        assert_eq!(transposed.get(&[0, 1]), Some(4));
        assert_eq!(transposed.get(&[1, 1]), Some(5));
        assert_eq!(transposed.get(&[2, 1]), Some(6));

        // Contiguous conversion
        let contig = transposed.contiguous();
        assert!(contig.is_contiguous());
        assert_eq!(contig.as_slice(), Some(&[1, 4, 2, 5, 3, 6][..]));

        // Slicing
        let sliced = t.slice(&[(0, 2), (1, 3)]).unwrap();
        assert_eq!(sliced.shape(), &[2, 2]);
        assert_eq!(sliced.get(&[0, 0]), Some(2));
        assert_eq!(sliced.get(&[0, 1]), Some(3));
        assert_eq!(sliced.get(&[1, 0]), Some(5));
        assert_eq!(sliced.get(&[1, 1]), Some(6));
    }

    #[test]
    fn test_broadcasting_and_arithmetic() {
        // [2, 3] + [3] -> [2, 3]
        let a = Tensor::from_vec(vec![2, 3], vec![10, 20, 30, 40, 50, 60]).unwrap();
        let b = Tensor::from_vec(vec![3], vec![1, 2, 3]).unwrap();

        let c = a.add(&b).unwrap();
        assert_eq!(c.shape(), &[2, 3]);
        assert_eq!(c.contiguous().as_slice(), Some(&[11, 22, 33, 41, 52, 63][..]));

        // [2, 1] * [1, 3] -> [2, 3]
        let x = Tensor::from_vec(vec![2, 1], vec![2, 3]).unwrap();
        let y = Tensor::from_vec(vec![1, 3], vec![10, 20, 30]).unwrap();
        let z = x.mul(&y).unwrap();
        assert_eq!(z.shape(), &[2, 3]);
        assert_eq!(z.contiguous().as_slice(), Some(&[20, 40, 60, 30, 60, 90][..]));
    }

    #[test]
    fn test_reductions() {
        let t = Tensor::from_vec(vec![2, 3], vec![1.0, 5.0, 2.0, 6.0, 3.0, 4.0]).unwrap();

        // Global sum
        let sum_all = t.sum(None).unwrap();
        assert_eq!(sum_all.shape(), &[] as &[usize]);
        assert_eq!(sum_all.get(&[]), Some(21.0));

        // Sum along axis 0: [2, 3] -> [3]
        let sum_ax0 = t.sum(Some(0)).unwrap();
        assert_eq!(sum_ax0.shape(), &[3]);
        assert_eq!(sum_ax0.contiguous().as_slice(), Some(&[7.0, 8.0, 6.0][..]));

        // Max along axis 1: [2, 3] -> [2]
        let max_ax1 = t.max(Some(1)).unwrap();
        assert_eq!(max_ax1.shape(), &[2]);
        assert_eq!(max_ax1.contiguous().as_slice(), Some(&[5.0, 6.0][..]));

        // Argmax along axis 1
        let argmax_ax1 = t.argmax(Some(1)).unwrap();
        assert_eq!(argmax_ax1.shape(), &[2]);
        assert_eq!(argmax_ax1.contiguous().as_slice(), Some(&[1, 0][..]));
    }

    #[test]
    fn test_gemm_matmul() {
        // [2, 3] x [3, 2] -> [2, 2]
        let a = Tensor::from_vec(vec![2, 3], vec![1, 2, 3, 4, 5, 6]).unwrap();
        let b = Tensor::from_vec(vec![3, 2], vec![7, 8, 9, 1, 2, 3]).unwrap();

        let c = a.matmul(&b).unwrap();
        assert_eq!(c.shape(), &[2, 2]);
        // Row 0: 1*7 + 2*9 + 3*2 = 7 + 18 + 6 = 31
        // Col 1: 1*8 + 2*1 + 3*3 = 8 + 2 + 9 = 19
        // Row 1: 4*7 + 5*9 + 6*2 = 28 + 45 + 12 = 85
        // Col 1: 4*8 + 5*1 + 6*3 = 32 + 5 + 18 = 55
        assert_eq!(c.contiguous().as_slice(), Some(&[31, 19, 85, 55][..]));
    }
}
