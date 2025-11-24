//! Helpers for taking a slice of indices (indices into `PLTE` and/or `trNS`
//! entries) and transforming this into RGB or RGBA output.
//!
//! # Memoization
//!
//! To achieve higher throughput, `create_rgba_palette` combines entries from
//! `PLTE` and `trNS` chunks into a single lookup table.  This is based on the
//! ideas explored in <https://crbug.com/706134>.
//!
//! Memoization is a trade-off:
//! * On one hand, memoization requires spending X ns before starting to call
//!   `expand_paletted_...` functions.
//! * On the other hand, memoization improves the throughput of the
//!   `expand_paletted_...` functions - they take Y ns less to process each byte
//!
//! Based on X and Y, we can try to calculate the breakeven point.  It seems
//! that memoization is a net benefit for images bigger than around 13x13 pixels.

use super::{unpack_bits, TransformFn};
use crate::{BitDepth, Info};

pub fn create_expansion_into_rgb8(info: &Info) -> TransformFn {
    let rgba_palette = create_rgba_palette(info);

    if info.bit_depth == BitDepth::Eight {
        Box::new(move |input, output, _info| expand_8bit_into_rgb8(input, output, &rgba_palette))
    } else {
        Box::new(move |input, output, info| expand_into_rgb8(input, output, info, &rgba_palette))
    }
}

pub fn create_expansion_into_rgba8(info: &Info) -> TransformFn {
    let rgba_palette = create_rgba_palette(info);
    Box::new(move |input, output, info| {
        expand_paletted_into_rgba8(input, output, info, &rgba_palette)
    })
}

fn create_rgba_palette(info: &Info) -> [[u8; 4]; 256] {
    let palette = info.palette.as_deref().expect("Caller should verify");
    let trns = info.trns.as_deref().unwrap_or(&[]);

    // > The tRNS chunk shall not contain more alpha values than there are palette
    // entries, but a tRNS chunk may contain fewer values than there are palette
    // entries. In this case, the alpha value for all remaining palette entries is
    // assumed to be 255.
    //
    // It seems, accepted reading is to fully *ignore* an invalid tRNS as if it were
    // completely empty / all pixels are non-transparent.
    let trns = if trns.len() <= palette.len() / 3 {
        trns
    } else {
        &[]
    };

    // Default to black, opaque entries.
    let mut rgba_palette = [[0, 0, 0, 0xFF]; 256];

    // Copy `palette` (RGB) entries into `rgba_palette`.  This may clobber alpha
    // values in `rgba_palette` - we need to fix this later.
    {
        let mut palette_iter = palette;
        let mut rgba_iter = &mut rgba_palette[..];
        while palette_iter.len() >= 4 {
            // Copying 4 bytes at a time is more efficient than copying 3.
            // OTOH, this clobbers the alpha value in `rgba_iter[0][3]` - we
            // need to fix this later.
            rgba_iter[0].copy_from_slice(&palette_iter[0..4]);

            palette_iter = &palette_iter[3..];
            rgba_iter = &mut rgba_iter[1..];
        }
        if !palette_iter.is_empty() {
            rgba_iter[0][0..3].copy_from_slice(&palette_iter[0..3]);
        }
    }

    // Copy `trns` (alpha) entries into `rgba_palette`.  `trns.len()` may be
    // smaller than `palette.len()` and therefore this is not sufficient to fix
    // all the clobbered alpha values.
    for (alpha, rgba) in trns.iter().copied().zip(rgba_palette.iter_mut()) {
        rgba[3] = alpha;
    }

    // Unclobber the remaining alpha values.
    for rgba in rgba_palette[trns.len()..(palette.len() / 3)].iter_mut() {
        rgba[3] = 0xFF;
    }

    rgba_palette
}

fn expand_8bit_into_rgb8(mut input: &[u8], mut output: &mut [u8], rgba_palette: &[[u8; 4]; 256]) {
    while output.len() >= 4 {
        // Copying 4 bytes at a time is more efficient than 3.
        let rgba = &rgba_palette[input[0] as usize];
        output[0..4].copy_from_slice(rgba);

        input = &input[1..];
        output = &mut output[3..];
    }
    if !output.is_empty() {
        let rgba = &rgba_palette[input[0] as usize];
        output[0..3].copy_from_slice(&rgba[0..3]);
    }
}

fn expand_into_rgb8(row: &[u8], buffer: &mut [u8], info: &Info, rgba_palette: &[[u8; 4]; 256]) {
    unpack_bits(row, buffer, 3, info.bit_depth as u8, |i, chunk| {
        let rgba = &rgba_palette[i as usize];
        chunk[0] = rgba[0];
        chunk[1] = rgba[1];
        chunk[2] = rgba[2];
    })
}

fn expand_paletted_into_rgba8(
    row: &[u8],
    buffer: &mut [u8],
    info: &Info,
    rgba_palette: &[[u8; 4]; 256],
) {
    unpack_bits(row, buffer, 4, info.bit_depth as u8, |i, chunk| {
        chunk.copy_from_slice(&rgba_palette[i as usize]);
    });
}

#[cfg(test)]
mod test {
    use crate::{BitDepth, ColorType, Info, Transformations};

    /// Old, non-memoized version of the code is used as a test oracle.
    fn oracle_expand_paletted_into_rgb8(row: &[u8], buffer: &mut [u8], info: &Info) {
        let palette = info.palette.as_deref().expect("Caller should verify");
        let black = [0, 0, 0];

        super::unpack_bits(row, buffer, 3, info.bit_depth as u8, |i, chunk| {
            let rgb = palette
                .get(3 * i as usize..3 * i as usize + 3)
                .unwrap_or(&black);
            chunk[0] = rgb[0];
            chunk[1] = rgb[1];
            chunk[2] = rgb[2];
        })
    }

    /// Old, non-memoized version of the code is used as a test oracle.
    fn oracle_expand_paletted_into_rgba8(row: &[u8], buffer: &mut [u8], info: &Info) {
        let palette = info.palette.as_deref().expect("Caller should verify");
        let trns = info.trns.as_deref().unwrap_or(&[]);
        let black = [0, 0, 0];

        // > The tRNS chunk shall not contain more alpha values than there are palette
        // entries, but a tRNS chunk may contain fewer values than there are palette
        // entries. In this case, the alpha value for all remaining palette entries is
        // assumed to be 255.
        //
        // It seems, accepted reading is to fully *ignore* an invalid tRNS as if it were
        // completely empty / all pixels are non-transparent.
        let trns = if trns.len() <= palette.len() / 3 {
            trns
        } else {
            &[]
        };

        super::unpack_bits(row, buffer, 4, info.bit_depth as u8, |i, chunk| {
            let (rgb, a) = (
                palette
                    .get(3 * i as usize..3 * i as usize + 3)
                    .unwrap_or(&black),
                *trns.get(i as usize).unwrap_or(&0xFF),
            );
            chunk[0] = rgb[0];
            chunk[1] = rgb[1];
            chunk[2] = rgb[2];
            chunk[3] = a;
        });
    }

    fn create_info<'a>(src_bit_depth: u8, palette: &'a [u8], trns: Option<&'a [u8]>) -> Info<'a> {
        Info {
            color_type: ColorType::Indexed,
            bit_depth: BitDepth::from_u8(src_bit_depth).unwrap(),
            palette: Some(palette.into()),
            trns: trns.map(Into::into),
            ..Info::default()
        }
    }

    fn expand_paletted(
        src: &[u8],
        src_bit_depth: u8,
        palette: &[u8],
        trns: Option<&[u8]>,
    ) -> Vec<u8> {
        let info = create_info(src_bit_depth, palette, trns);
        let output_bytes_per_input_sample = match trns {
            None => 3,
            Some(_) => 4,
        };
        let samples_count_per_byte = (8 / src_bit_depth) as usize;
        let samples_count = src.len() * samples_count_per_byte;

        let mut dst = vec![0; samples_count * output_bytes_per_input_sample];
        let transform_fn =
            super::super::create_transform_fn(&info, Transformations::EXPAND).unwrap();
        transform_fn(src, dst.as_mut_slice(), &info);

        {
            // Compare the memoization-based calculations with the old, non-memoized code.
            let mut simple_dst = vec![0; samples_count * output_bytes_per_input_sample];
            if trns.is_none() {
                oracle_expand_paletted_into_rgb8(src, &mut simple_dst, &info)
            } else {
                oracle_expand_paletted_into_rgba8(src, &mut simple_dst, &info)
            }
            assert_eq!(&dst, &simple_dst);
        }

        dst
    }

    #[test]
    fn test_expand_paletted_rgba_8bit() {
        let actual = expand_paletted(
            &[0, 1, 2, 3], // src
            8,             // src_bit_depth
            &[
                // palette
                0, 1, 2, //    entry #0
                4, 5, 6, //    entry #1
                8, 9, 10, //   entry #2
                12, 13, 14, // entry #3
            ],
            Some(&[3, 7, 11, 15]), // trns
        );
        assert_eq!(actual, (0..16).collect::<Vec<u8>>());
    }

    #[test]
    fn test_expand_paletted_rgb_8bit() {
        let actual = expand_paletted(
            &[0, 1, 2, 3], // src
            8,             // src_bit_depth
            &[
                // palette
                0, 1, 2, //   entry #0
                3, 4, 5, //   entry #1
                6, 7, 8, //   entry #2
                9, 10, 11, // entry #3
            ],
            None, // trns
        );
        assert_eq!(actual, (0..12).collect::<Vec<u8>>());
    }

    #[test]
    fn test_expand_paletted_rgba_4bit() {
        let actual = expand_paletted(
            &[0x01, 0x23], // src
            4,             // src_bit_depth
            &[
                // palette
                0, 1, 2, //    entry #0
                4, 5, 6, //    entry #1
                8, 9, 10, //   entry #2
                12, 13, 14, // entry #3
            ],
            Some(&[3, 7, 11, 15]), // trns
        );
        assert_eq!(actual, (0..16).collect::<Vec<u8>>());
    }

    #[test]
    fn test_expand_paletted_rgb_4bit() {
        let actual = expand_paletted(
            &[0x01, 0x23], // src
            4,             // src_bit_depth
            &[
                // palette
                0, 1, 2, //   entry #0
                3, 4, 5, //   entry #1
                6, 7, 8, //   entry #2
                9, 10, 11, // entry #3
            ],
            None, // trns
        );
        assert_eq!(actual, (0..12).collect::<Vec<u8>>());
    }

    #[test]
    fn test_expand_paletted_rgba_8bit_more_trns_entries_than_palette_entries() {
        let actual = expand_paletted(
            &[0, 1, 2, 3], // src
            8,             // src_bit_depth
            &[
                // palette
                0, 1, 2, //    entry #0
                4, 5, 6, //    entry #1
                8, 9, 10, //   entry #2
                12, 13, 14, // entry #3
            ],
            Some(&[123; 5]), // trns
        );

        // Invalid (too-long) `trns` means that we'll use 0xFF / opaque alpha everywhere.
        assert_eq!(
            actual,
            vec![0, 1, 2, 0xFF, 4, 5, 6, 0xFF, 8, 9, 10, 0xFF, 12, 13, 14, 0xFF],
        );
    }

    #[test]
    fn test_expand_paletted_rgba_8bit_less_trns_entries_than_palette_entries() {
        let actual = expand_paletted(
            &[0, 1, 2, 3], // src
            8,             // src_bit_depth
            &[
                // palette
                0, 1, 2, //    entry #0
                4, 5, 6, //    entry #1
                8, 9, 10, //   entry #2
                12, 13, 14, // entry #3
            ],
            Some(&[3, 7]), // trns
        );

        // Too-short `trns` is treated differently from too-long - only missing entries are
        // replaced with 0XFF / opaque.
        assert_eq!(
            actual,
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0xFF, 12, 13, 14, 0xFF],
        );
    }

    #[test]
    fn test_create_rgba_palette() {
        fn create_expected_rgba_palette(plte: &[u8], trns: &[u8]) -> [[u8; 4]; 256] {
            let mut rgba = [[1, 2, 3, 4]; 256];
            for (i, rgba) in rgba.iter_mut().enumerate() {
                rgba[0] = plte.get(i * 3 + 0).map(|&r| r).unwrap_or(0);
                rgba[1] = plte.get(i * 3 + 1).map(|&g| g).unwrap_or(0);
                rgba[2] = plte.get(i * 3 + 2).map(|&b| b).unwrap_or(0);
                rgba[3] = trns.get(i * 1 + 0).map(|&a| a).unwrap_or(0xFF);
            }
            rgba
        }

        for plte_len in 1..=32 {
            for trns_len in 0..=plte_len {
                let plte: Vec<u8> = (0..plte_len * 3).collect();
                let trns: Vec<u8> = (0..trns_len).map(|alpha| alpha + 200).collect();

                let info = create_info(8, &plte, Some(&trns));
                let expected = create_expected_rgba_palette(&plte, &trns);
                let actual = super::create_rgba_palette(&info);
                assert_eq!(actual, expected);
            }
        }
    }
}
