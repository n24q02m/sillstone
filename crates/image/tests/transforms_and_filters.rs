//! Tests for image geometric transforms and 2D filters.

use sillstone_image::filter::{BuiltinFilter, adjust_brightness, apply_filter, gaussian_blur, invert};
use sillstone_image::raster::{Gray, Image, ResamplingFilter, Rgb};
use sillstone_image::transform::{TransposeMethod, crop, rotate, transpose};

#[test]
fn test_crop_valid() {
    let im: Image<Gray> = Image::from_vec(4, 4, (0..16).collect()).unwrap();
    let cropped = crop(&im, 1, 1, 3, 3).unwrap();
    assert_eq!((cropped.width(), cropped.height()), (2, 2));
    assert_eq!(cropped.as_slice(), &[5, 6, 9, 10]);
}

#[test]
fn test_crop_invalid() {
    let im: Image<Gray> = Image::from_vec(4, 4, (0..16).collect()).unwrap();
    assert!(crop(&im, 2, 2, 2, 3).is_err()); // x0 == x1
    assert!(crop(&im, 1, 1, 5, 3).is_err()); // x1 > width
}

#[test]
fn test_transpose_flips() {
    let im: Image<Gray> = Image::from_vec(2, 2, vec![1, 2, 3, 4]).unwrap();
    let h_flip = transpose(&im, TransposeMethod::FlipHorizontal).unwrap();
    assert_eq!(h_flip.as_slice(), &[2, 1, 4, 3]);

    let v_flip = transpose(&im, TransposeMethod::FlipVertical).unwrap();
    assert_eq!(v_flip.as_slice(), &[3, 4, 1, 2]);

    let rot90 = transpose(&im, TransposeMethod::Rotate90).unwrap();
    assert_eq!(rot90.as_slice(), &[2, 4, 1, 3]);

    let rot180 = transpose(&im, TransposeMethod::Rotate180).unwrap();
    assert_eq!(rot180.as_slice(), &[4, 3, 2, 1]);
}

#[test]
fn test_rotate_arbitrary() {
    let im: Image<Gray> = Image::from_vec(4, 4, vec![100u8; 16]).unwrap();
    let rotated = rotate(&im, 45.0, true, ResamplingFilter::Bilinear).unwrap();
    assert!(rotated.width() >= 4);
    assert!(rotated.height() >= 4);

    let rot_no_expand = rotate(&im, 45.0, false, ResamplingFilter::Bilinear).unwrap();
    assert_eq!((rot_no_expand.width(), rot_no_expand.height()), (4, 4));
}

#[test]
fn test_filters_blur_and_sharpen() {
    let im: Image<Rgb> = Image::from_vec(4, 4, vec![120u8; 48]).unwrap();
    let blurred = apply_filter(&im, BuiltinFilter::Blur).unwrap();
    assert_eq!((blurred.width(), blurred.height()), (4, 4));
    assert!(blurred.as_slice().iter().all(|&s| (s as i32 - 120).abs() <= 1));

    let sharpened = apply_filter(&im, BuiltinFilter::Sharpen).unwrap();
    assert_eq!((sharpened.width(), sharpened.height()), (4, 4));
    assert!(sharpened.as_slice().iter().all(|&s| (s as i32 - 120).abs() <= 1));
}

#[test]
fn test_gaussian_blur() {
    let im: Image<Gray> = Image::from_vec(5, 5, vec![50u8; 25]).unwrap();
    let blurred = gaussian_blur(&im, 1.5).unwrap();
    assert_eq!((blurred.width(), blurred.height()), (5, 5));
    assert!(blurred.as_slice().iter().all(|&s| (s as i32 - 50).abs() <= 1));
}

#[test]
fn test_adjust_brightness_and_invert() {
    let im: Image<Gray> = Image::from_vec(2, 2, vec![10, 50, 100, 200]).unwrap();
    let brighter = adjust_brightness(&im, 1.5).unwrap();
    assert_eq!(brighter.as_slice(), &[15, 75, 150, 255]); // 200 * 1.5 = 300 -> clamp 255

    let inverted = invert(&im).unwrap();
    assert_eq!(inverted.as_slice(), &[245, 205, 155, 55]);
}
