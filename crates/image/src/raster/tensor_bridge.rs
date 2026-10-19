//! Zero-copy bridge between Image buffers and N-dimensional Tensors.

use alloc::vec;
use sillstone_digest::{Error, Result};
use sillstone_tensor::Tensor;

use crate::raster::{Image, Layout, Sample};

impl<L: Layout, T: Sample> Image<L, T> {
    /// Converts an Image into an N-dimensional Tensor with shape `[height, width, channels]`.
    pub fn to_tensor(&self) -> Tensor<T>
    where
        T: sillstone_tensor::Dtype,
    {
        let h = self.height() as usize;
        let w = self.width() as usize;
        let c = L::CHANNELS as usize;
        let shape = vec![h, w, c];
        Tensor::from_vec(shape, self.as_slice().to_vec()).expect("image size must match product")
    }

    /// Reconstructs an Image from a 2D `[height, width]` or 3D `[height, width, channels]` Tensor.
    pub fn from_tensor(tensor: &Tensor<T>) -> Result<Self>
    where
        T: sillstone_tensor::Dtype,
    {
        let shape = tensor.shape();
        if shape.len() != 2 && shape.len() != 3 {
            return Err(Error::BadValue("tensor must be 2D or 3D to convert to image"));
        }

        let h = shape[0] as u32;
        let w = shape[1] as u32;
        let c = if shape.len() == 3 { shape[2] } else { 1 };

        if c != L::CHANNELS as usize {
            return Err(Error::BadValue("tensor channel count does not match layout"));
        }

        let contig = tensor.contiguous();
        let slice = contig.as_slice().ok_or(Error::BadValue("expected contiguous tensor"))?;

        Image::from_vec(w, h, slice.to_vec())
    }
}
