//! Numeric data type traits for tensor elements.

use core::fmt::Debug;

/// Marker and capability trait for types that can serve as tensor elements.
pub trait Dtype: Copy + Clone + Default + PartialEq + Debug + 'static {
    /// Zero representation.
    const ZERO: Self;
    /// One representation.
    const ONE: Self;
}

macro_rules! impl_dtype {
    ($t:ty, $z:expr, $o:expr) => {
        impl Dtype for $t {
            const ZERO: Self = $z;
            const ONE: Self = $o;
        }
    };
}

impl_dtype!(f32, 0.0, 1.0);
impl_dtype!(f64, 0.0, 1.0);
impl_dtype!(i8, 0, 1);
impl_dtype!(i16, 0, 1);
impl_dtype!(i32, 0, 1);
impl_dtype!(i64, 0, 1);
impl_dtype!(u8, 0, 1);
impl_dtype!(u16, 0, 1);
impl_dtype!(u32, 0, 1);
impl_dtype!(u64, 0, 1);
impl_dtype!(usize, 0, 1);
impl_dtype!(bool, false, true);
