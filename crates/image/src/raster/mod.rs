//! Raw image buffers, box-average resampling, BT.601 luma and EXIF
//! orientation.
//!
//! Ported byte-compatibly from the `modhash` kit's `modhash-raster`
//! crate: this module is the shared destination every image decoder in
//! the suite writes into, and the shared source every image hash reads
//! from. The conventions that decide pixel values — tight row-major
//! layout, integer box boundaries, `round_half_up` everywhere, the EXIF
//! orientation table — are specified in the kit's
//! `docs/algorithms/raster.md` and pinned by the tests in `tests/`; an
//! implementation that silently rounds differently produces near-miss
//! hashes, so the rules live here exactly once.
//!
//! The module is written against [`core`] and [`alloc`] only (the
//! surrounding crate is `std`, which the buffer code never needs), and
//! every allocation is capped by [`MAX_BUFFER_BYTES`] before a byte is
//! reserved.

mod color;
mod exif;
// `raster::raster` keeps the upstream file naming (`modhash-raster`'s
// buffer module of the same name); the nesting is intentional.
#[allow(clippy::module_inception)]
mod raster;
mod resample;

pub use self::color::luma_bt601;
pub use self::exif::apply_orientation;
pub use self::raster::{Gray, Image, Layout, MAX_BUFFER_BYTES, Rgb, Rgba, Sample};
pub use self::resample::box_average;
