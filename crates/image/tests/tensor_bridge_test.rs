//! Tests for Image <-> Tensor zero-copy/bidirectional conversion.

use sillstone_image::raster::{Image, Rgb};
use sillstone_tensor::Tensor;

#[test]
fn test_image_to_tensor_and_back() {
    let w = 4u32;
    let h = 3u32;
    let mut pixels = Vec::with_capacity((w * h * 3) as usize);
    for i in 0..(w * h * 3) {
        pixels.push((i % 256) as u8);
    }

    let img = Image::<Rgb>::from_vec(w, h, pixels.clone()).unwrap();
    let tensor: Tensor<u8> = img.to_tensor();

    assert_eq!(tensor.shape(), &[3, 4, 3]);
    assert_eq!(tensor.ndim(), 3);
    assert_eq!(tensor.numel(), 36);

    // Convert back from tensor
    let restored = Image::<Rgb>::from_tensor(&tensor).unwrap();
    assert_eq!(restored.width(), 4);
    assert_eq!(restored.height(), 3);
    assert_eq!(restored.as_slice(), pixels.as_slice());
}
