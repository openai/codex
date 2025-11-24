//! Development-time-only helper module for exporting private APIs so that they can be benchmarked.
//! This module is gated behind the "benchmarks" feature.

use crate::adam7::{expand_pass, Adam7Iterator};
use crate::common::BytesPerPixel;
use crate::filter::{Filter, RowFilter};
use crate::{BitDepth, ColorType, Info};

/// Re-exporting `unfilter` to make it easier to benchmark, despite some items being only
/// `pub(crate)`: `fn unfilter`, `enum BytesPerPixel`.
pub fn unfilter(filter: Filter, tbpp: u8, previous: &[u8], current: &mut [u8]) {
    let filter = RowFilter::from_method(filter).unwrap(); // RowFilter type is private
    let tbpp = BytesPerPixel::from_usize(tbpp as usize);
    crate::filter::unfilter(filter, tbpp, previous, current)
}

pub fn adam7(img: &mut [u8], buffer: &[u8], width: u32, height: u32, bpp: u8) {
    fn bytes_of_width(width: u32, bpp: u8) -> usize {
        let total = (u64::from(width) * u64::from(bpp)).div_ceil(8);
        usize::try_from(total).unwrap()
    }

    let img_row_stride = bytes_of_width(width, bpp);
    for adam7 in Adam7Iterator::new(width as u32, height as u32).into_iter() {
        // We use the same buffer for all interlace passes, to avoid counting the creation time in
        // the benchmark here. But the expansion expects us to pass a slice so make sure we use the
        // correct one. As of writing the implementation is not sensitive to this but it may become
        // so.
        let used_bytes = bytes_of_width(adam7.width, bpp);
        expand_pass(img, img_row_stride, &buffer[..used_bytes], &adam7, bpp);
    }
}

pub use crate::decoder::transform::{create_transform_fn, TransformFn};

pub fn create_info_from_plte_trns_bitdepth<'a>(
    plte: &'a [u8],
    trns: Option<&'a [u8]>,
    bit_depth: u8,
) -> Info<'a> {
    Info {
        color_type: ColorType::Indexed,
        bit_depth: BitDepth::from_u8(bit_depth).unwrap(),
        palette: Some(plte.into()),
        trns: trns.map(Into::into),
        ..Info::default()
    }
}
