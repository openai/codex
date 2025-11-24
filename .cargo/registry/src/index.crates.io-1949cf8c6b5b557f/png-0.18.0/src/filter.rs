use core::convert::TryInto;

use crate::{common::BytesPerPixel, Compression};

/// The byte level filter applied to scanlines to prepare them for compression.
///
/// Compression in general benefits from repetitive data. The filter is a content-aware method of
/// compressing the range of occurring byte values to help the compression algorithm. Note that
/// this does not operate on pixels but on raw bytes of a scanline.
///
/// Details on how each filter works can be found in the [PNG Book](http://www.libpng.org/pub/png/book/chapter09.html).
///
/// The default filter is `Adaptive`, which uses heuristics to select the best filter for every row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Filter {
    NoFilter,
    Sub,
    Up,
    Avg,
    Paeth,
    Adaptive,
}

impl Default for Filter {
    fn default() -> Self {
        Filter::Adaptive
    }
}

impl From<RowFilter> for Filter {
    fn from(value: RowFilter) -> Self {
        match value {
            RowFilter::NoFilter => Filter::NoFilter,
            RowFilter::Sub => Filter::Sub,
            RowFilter::Up => Filter::Up,
            RowFilter::Avg => Filter::Avg,
            RowFilter::Paeth => Filter::Paeth,
        }
    }
}

impl Filter {
    pub(crate) fn from_simple(compression: Compression) -> Self {
        match compression {
            Compression::NoCompression => Filter::NoFilter, // with no DEFLATE filtering would only waste time
            Compression::Fastest => Filter::Up, // pairs well with FdeflateUltraFast, producing much smaller files while being very fast
            Compression::Fast => Filter::Adaptive,
            Compression::Balanced => Filter::Adaptive,
            Compression::High => Filter::Adaptive,
        }
    }
}

/// Unlike the public [Filter], does not include the "Adaptive" option
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum RowFilter {
    NoFilter = 0,
    Sub = 1,
    Up = 2,
    Avg = 3,
    Paeth = 4,
}

impl Default for RowFilter {
    fn default() -> Self {
        RowFilter::Up
    }
}

impl RowFilter {
    pub fn from_u8(n: u8) -> Option<Self> {
        match n {
            0 => Some(Self::NoFilter),
            1 => Some(Self::Sub),
            2 => Some(Self::Up),
            3 => Some(Self::Avg),
            4 => Some(Self::Paeth),
            _ => None,
        }
    }

    pub fn from_method(strat: Filter) -> Option<Self> {
        match strat {
            Filter::NoFilter => Some(Self::NoFilter),
            Filter::Sub => Some(Self::Sub),
            Filter::Up => Some(Self::Up),
            Filter::Avg => Some(Self::Avg),
            Filter::Paeth => Some(Self::Paeth),
            Filter::Adaptive => None,
        }
    }
}

fn filter_paeth(a: u8, b: u8, c: u8) -> u8 {
    // On ARM this algorithm performs much better than the one above adapted from stb,
    // and this is the better-studied algorithm we've always used here,
    // so we default to it on all non-x86 platforms.
    let pa = (i16::from(b) - i16::from(c)).abs();
    let pb = (i16::from(a) - i16::from(c)).abs();
    let pc = ((i16::from(a) - i16::from(c)) + (i16::from(b) - i16::from(c))).abs();

    let mut out = a;
    let mut min = pa;

    if pb < min {
        min = pb;
        out = b;
    }
    if pc < min {
        out = c;
    }

    out
}

fn filter_paeth_stbi(a: u8, b: u8, c: u8) -> u8 {
    // Decoding optimizes better with this algorithm than with `filter_paeth`
    //
    // This formulation looks very different from the reference in the PNG spec, but is
    // actually equivalent and has favorable data dependencies and admits straightforward
    // generation of branch-free code, which helps performance significantly.
    //
    // Adapted from public domain PNG implementation:
    // https://github.com/nothings/stb/blob/5c205738c191bcb0abc65c4febfa9bd25ff35234/stb_image.h#L4657-L4668
    let thresh = i16::from(c) * 3 - (i16::from(a) + i16::from(b));
    let lo = a.min(b);
    let hi = a.max(b);
    let t0 = if hi as i16 <= thresh { lo } else { c };
    let t1 = if thresh <= lo as i16 { hi } else { t0 };
    t1
}

fn filter_paeth_fpnge(a: u8, b: u8, c: u8) -> u8 {
    // This is an optimized version of the paeth filter from the PNG specification, proposed by
    // Luca Versari for [FPNGE](https://www.lucaversari.it/FJXL_and_FPNGE.pdf). It operates
    // entirely on unsigned 8-bit quantities, making it more conducive to vectorization.
    //
    //     p = a + b - c
    //     pa = |p - a| = |a + b - c - a| = |b - c| = max(b, c) - min(b, c)
    //     pb = |p - b| = |a + b - c - b| = |a - c| = max(a, c) - min(a, c)
    //     pc = |p - c| = |a + b - c - c| = |(b - c) + (a - c)| = ...
    //
    // Further optimizing the calculation of `pc` a bit tricker. However, notice that:
    //
    //        a > c && b > c
    //    ==> (a - c) > 0 && (b - c) > 0
    //    ==> pc > (a - c) && pc > (b - c)
    //    ==> pc > |a - c| && pc > |b - c|
    //    ==> pc > pb && pc > pa
    //
    // Meaning that if `c` is smaller than `a` and `b`, the value of `pc` is irrelevant. Similar
    // reasoning applies if `c` is larger than the other two inputs. Assuming that `c >= b` and
    // `c <= b` or vice versa:
    //
    //     pc = ||b - c| - |a - c|| =  |pa - pb| = max(pa, pb) - min(pa, pb)
    //
    let pa = b.max(c) - c.min(b);
    let pb = a.max(c) - c.min(a);
    let pc = if (a < c) == (c < b) {
        pa.max(pb) - pa.min(pb)
    } else {
        255
    };

    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

pub(crate) fn unfilter(
    mut filter: RowFilter,
    tbpp: BytesPerPixel,
    previous: &[u8],
    current: &mut [u8],
) {
    use self::RowFilter::*;

    // If the previous row is empty, then treat it as if it were filled with zeros.
    if previous.is_empty() {
        if filter == Paeth {
            filter = Sub;
        } else if filter == Up {
            filter = NoFilter;
        }
    }

    // Auto-vectorization notes
    // ========================
    //
    // [2023/01 @okaneco] - Notes on optimizing decoding filters
    //
    // Links:
    // [PR]: https://github.com/image-rs/image-png/pull/382
    // [SWAR]: http://aggregate.org/SWAR/over.html
    // [AVG]: http://aggregate.org/MAGIC/#Average%20of%20Integers
    //
    // #382 heavily refactored and optimized the following filters making the
    // implementation nonobvious. These comments function as a summary of that
    // PR with an explanation of the choices made below.
    //
    // #382 originally started with trying to optimize using a technique called
    // SWAR, SIMD Within a Register. SWAR uses regular integer types like `u32`
    // and `u64` as SIMD registers to perform vertical operations in parallel,
    // usually involving bit-twiddling. This allowed each `BytesPerPixel` (bpp)
    // pixel to be decoded in parallel: 3bpp and 4bpp in a `u32`, 6bpp and 8pp
    // in a `u64`. The `Sub` filter looked like the following code block, `Avg`
    // was similar but used a bitwise average method from [AVG]:
    // ```
    // // See "Unpartitioned Operations With Correction Code" from [SWAR]
    // fn swar_add_u32(x: u32, y: u32) -> u32 {
    //     // 7-bit addition so there's no carry over the most significant bit
    //     let n = (x & 0x7f7f7f7f) + (y & 0x7f7f7f7f); // 0x7F = 0b_0111_1111
    //     // 1-bit parity/XOR addition to fill in the missing MSB
    //     n ^ (x ^ y) & 0x80808080                     // 0x80 = 0b_1000_0000
    // }
    //
    // let mut prev =
    //     u32::from_ne_bytes([current[0], current[1], current[2], current[3]]);
    // for chunk in current[4..].chunks_exact_mut(4) {
    //     let cur = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    //     let new_chunk = swar_add_u32(cur, prev);
    //     chunk.copy_from_slice(&new_chunk.to_ne_bytes());
    //     prev = new_chunk;
    // }
    // ```
    // While this provided a measurable increase, @fintelia found that this idea
    // could be taken even further by unrolling the chunks component-wise and
    // avoiding unnecessary byte-shuffling by using byte arrays instead of
    // `u32::from|to_ne_bytes`. The bitwise operations were no longer necessary
    // so they were reverted to their obvious arithmetic equivalent. Lastly,
    // `TryInto` was used instead of `copy_from_slice`. The `Sub` code now
    // looked like this (with asserts to remove `0..bpp` bounds checks):
    // ```
    // assert!(len > 3);
    // let mut prev = [current[0], current[1], current[2], current[3]];
    // for chunk in current[4..].chunks_exact_mut(4) {
    //     let new_chunk = [
    //         chunk[0].wrapping_add(prev[0]),
    //         chunk[1].wrapping_add(prev[1]),
    //         chunk[2].wrapping_add(prev[2]),
    //         chunk[3].wrapping_add(prev[3]),
    //     ];
    //     *TryInto::<&mut [u8; 4]>::try_into(chunk).unwrap() = new_chunk;
    //     prev = new_chunk;
    // }
    // ```
    // The compiler was able to optimize the code to be even faster and this
    // method even sped up Paeth filtering! Assertions were experimentally
    // added within loop bodies which produced better instructions but no
    // difference in speed. Finally, the code was refactored to remove manual
    // slicing and start the previous pixel chunks with arrays of `[0; N]`.
    // ```
    // let mut prev = [0; 4];
    // for chunk in current.chunks_exact_mut(4) {
    //     let new_chunk = [
    //         chunk[0].wrapping_add(prev[0]),
    //         chunk[1].wrapping_add(prev[1]),
    //         chunk[2].wrapping_add(prev[2]),
    //         chunk[3].wrapping_add(prev[3]),
    //     ];
    //     *TryInto::<&mut [u8; 4]>::try_into(chunk).unwrap() = new_chunk;
    //     prev = new_chunk;
    // }
    // ```
    // While we're not manually bit-twiddling anymore, a possible takeaway from
    // this is to "think in SWAR" when dealing with small byte arrays. Unrolling
    // array operations and performing them component-wise may unlock previously
    // unavailable optimizations from the compiler, even when using the
    // `chunks_exact` methods for their potential auto-vectorization benefits.
    //
    // `std::simd` notes
    // =================
    //
    // In the past we have experimented with `std::simd` for unfiltering.  This
    // experiment was removed in https://github.com/image-rs/image-png/pull/585
    // because:
    //
    // * The crate's microbenchmarks showed that `std::simd` didn't have a
    //   significant advantage over auto-vectorization for most filters, except
    //   for Paeth unfiltering - see
    //   https://github.com/image-rs/image-png/pull/414#issuecomment-1736655668
    // * In the crate's microbenchmarks `std::simd` seemed to help with Paeth
    //   unfiltering only on x86/x64, with mixed results on ARM - see
    //   https://github.com/image-rs/image-png/pull/539#issuecomment-2512748043
    // * In Chromium end-to-end microbenchmarks `std::simd` either didn't help
    //   or resulted in a small regression (as measured on x64).  See
    //   https://crrev.com/c/6090592.
    // * Field trial data from some "real world" scenarios shows that
    //   performance can be quite good without relying on `std::simd` - see
    //   https://github.com/image-rs/image-png/discussions/562#discussioncomment-13303307
    match filter {
        NoFilter => {}
        Sub => match tbpp {
            BytesPerPixel::One => {
                current.iter_mut().reduce(|&mut prev, curr| {
                    *curr = curr.wrapping_add(prev);
                    curr
                });
            }
            BytesPerPixel::Two => {
                let mut prev = [0; 2];
                for chunk in current.chunks_exact_mut(2) {
                    let new_chunk = [
                        chunk[0].wrapping_add(prev[0]),
                        chunk[1].wrapping_add(prev[1]),
                    ];
                    *TryInto::<&mut [u8; 2]>::try_into(chunk).unwrap() = new_chunk;
                    prev = new_chunk;
                }
            }
            BytesPerPixel::Three => {
                let mut prev = [0; 3];
                for chunk in current.chunks_exact_mut(3) {
                    let new_chunk = [
                        chunk[0].wrapping_add(prev[0]),
                        chunk[1].wrapping_add(prev[1]),
                        chunk[2].wrapping_add(prev[2]),
                    ];
                    *TryInto::<&mut [u8; 3]>::try_into(chunk).unwrap() = new_chunk;
                    prev = new_chunk;
                }
            }
            BytesPerPixel::Four => {
                let mut prev = [0; 4];
                for chunk in current.chunks_exact_mut(4) {
                    let new_chunk = [
                        chunk[0].wrapping_add(prev[0]),
                        chunk[1].wrapping_add(prev[1]),
                        chunk[2].wrapping_add(prev[2]),
                        chunk[3].wrapping_add(prev[3]),
                    ];
                    *TryInto::<&mut [u8; 4]>::try_into(chunk).unwrap() = new_chunk;
                    prev = new_chunk;
                }
            }
            BytesPerPixel::Six => {
                let mut prev = [0; 6];
                for chunk in current.chunks_exact_mut(6) {
                    let new_chunk = [
                        chunk[0].wrapping_add(prev[0]),
                        chunk[1].wrapping_add(prev[1]),
                        chunk[2].wrapping_add(prev[2]),
                        chunk[3].wrapping_add(prev[3]),
                        chunk[4].wrapping_add(prev[4]),
                        chunk[5].wrapping_add(prev[5]),
                    ];
                    *TryInto::<&mut [u8; 6]>::try_into(chunk).unwrap() = new_chunk;
                    prev = new_chunk;
                }
            }
            BytesPerPixel::Eight => {
                let mut prev = [0; 8];
                for chunk in current.chunks_exact_mut(8) {
                    let new_chunk = [
                        chunk[0].wrapping_add(prev[0]),
                        chunk[1].wrapping_add(prev[1]),
                        chunk[2].wrapping_add(prev[2]),
                        chunk[3].wrapping_add(prev[3]),
                        chunk[4].wrapping_add(prev[4]),
                        chunk[5].wrapping_add(prev[5]),
                        chunk[6].wrapping_add(prev[6]),
                        chunk[7].wrapping_add(prev[7]),
                    ];
                    *TryInto::<&mut [u8; 8]>::try_into(chunk).unwrap() = new_chunk;
                    prev = new_chunk;
                }
            }
        },
        Up => {
            for (curr, &above) in current.iter_mut().zip(previous) {
                *curr = curr.wrapping_add(above);
            }
        }
        Avg if previous.is_empty() => match tbpp {
            BytesPerPixel::One => {
                current.iter_mut().reduce(|&mut prev, curr| {
                    *curr = curr.wrapping_add(prev / 2);
                    curr
                });
            }
            BytesPerPixel::Two => {
                let mut prev = [0; 2];
                for chunk in current.chunks_exact_mut(2) {
                    let new_chunk = [
                        chunk[0].wrapping_add(prev[0] / 2),
                        chunk[1].wrapping_add(prev[1] / 2),
                    ];
                    *TryInto::<&mut [u8; 2]>::try_into(chunk).unwrap() = new_chunk;
                    prev = new_chunk;
                }
            }
            BytesPerPixel::Three => {
                let mut prev = [0; 3];
                for chunk in current.chunks_exact_mut(3) {
                    let new_chunk = [
                        chunk[0].wrapping_add(prev[0] / 2),
                        chunk[1].wrapping_add(prev[1] / 2),
                        chunk[2].wrapping_add(prev[2] / 2),
                    ];
                    *TryInto::<&mut [u8; 3]>::try_into(chunk).unwrap() = new_chunk;
                    prev = new_chunk;
                }
            }
            BytesPerPixel::Four => {
                let mut prev = [0; 4];
                for chunk in current.chunks_exact_mut(4) {
                    let new_chunk = [
                        chunk[0].wrapping_add(prev[0] / 2),
                        chunk[1].wrapping_add(prev[1] / 2),
                        chunk[2].wrapping_add(prev[2] / 2),
                        chunk[3].wrapping_add(prev[3] / 2),
                    ];
                    *TryInto::<&mut [u8; 4]>::try_into(chunk).unwrap() = new_chunk;
                    prev = new_chunk;
                }
            }
            BytesPerPixel::Six => {
                let mut prev = [0; 6];
                for chunk in current.chunks_exact_mut(6) {
                    let new_chunk = [
                        chunk[0].wrapping_add(prev[0] / 2),
                        chunk[1].wrapping_add(prev[1] / 2),
                        chunk[2].wrapping_add(prev[2] / 2),
                        chunk[3].wrapping_add(prev[3] / 2),
                        chunk[4].wrapping_add(prev[4] / 2),
                        chunk[5].wrapping_add(prev[5] / 2),
                    ];
                    *TryInto::<&mut [u8; 6]>::try_into(chunk).unwrap() = new_chunk;
                    prev = new_chunk;
                }
            }
            BytesPerPixel::Eight => {
                let mut prev = [0; 8];
                for chunk in current.chunks_exact_mut(8) {
                    let new_chunk = [
                        chunk[0].wrapping_add(prev[0] / 2),
                        chunk[1].wrapping_add(prev[1] / 2),
                        chunk[2].wrapping_add(prev[2] / 2),
                        chunk[3].wrapping_add(prev[3] / 2),
                        chunk[4].wrapping_add(prev[4] / 2),
                        chunk[5].wrapping_add(prev[5] / 2),
                        chunk[6].wrapping_add(prev[6] / 2),
                        chunk[7].wrapping_add(prev[7] / 2),
                    ];
                    *TryInto::<&mut [u8; 8]>::try_into(chunk).unwrap() = new_chunk;
                    prev = new_chunk;
                }
            }
        },
        Avg => match tbpp {
            BytesPerPixel::One => {
                let mut lprev = [0; 1];
                for (chunk, above) in current.chunks_exact_mut(1).zip(previous.chunks_exact(1)) {
                    let new_chunk =
                        [chunk[0].wrapping_add(((above[0] as u16 + lprev[0] as u16) / 2) as u8)];
                    *TryInto::<&mut [u8; 1]>::try_into(chunk).unwrap() = new_chunk;
                    lprev = new_chunk;
                }
            }
            BytesPerPixel::Two => {
                let mut lprev = [0; 2];
                for (chunk, above) in current.chunks_exact_mut(2).zip(previous.chunks_exact(2)) {
                    let new_chunk = [
                        chunk[0].wrapping_add(((above[0] as u16 + lprev[0] as u16) / 2) as u8),
                        chunk[1].wrapping_add(((above[1] as u16 + lprev[1] as u16) / 2) as u8),
                    ];
                    *TryInto::<&mut [u8; 2]>::try_into(chunk).unwrap() = new_chunk;
                    lprev = new_chunk;
                }
            }
            BytesPerPixel::Three => {
                let mut lprev = [0; 3];
                for (chunk, above) in current.chunks_exact_mut(3).zip(previous.chunks_exact(3)) {
                    let new_chunk = [
                        chunk[0].wrapping_add(((above[0] as u16 + lprev[0] as u16) / 2) as u8),
                        chunk[1].wrapping_add(((above[1] as u16 + lprev[1] as u16) / 2) as u8),
                        chunk[2].wrapping_add(((above[2] as u16 + lprev[2] as u16) / 2) as u8),
                    ];
                    *TryInto::<&mut [u8; 3]>::try_into(chunk).unwrap() = new_chunk;
                    lprev = new_chunk;
                }
            }
            BytesPerPixel::Four => {
                let mut lprev = [0; 4];
                for (chunk, above) in current.chunks_exact_mut(4).zip(previous.chunks_exact(4)) {
                    let new_chunk = [
                        chunk[0].wrapping_add(((above[0] as u16 + lprev[0] as u16) / 2) as u8),
                        chunk[1].wrapping_add(((above[1] as u16 + lprev[1] as u16) / 2) as u8),
                        chunk[2].wrapping_add(((above[2] as u16 + lprev[2] as u16) / 2) as u8),
                        chunk[3].wrapping_add(((above[3] as u16 + lprev[3] as u16) / 2) as u8),
                    ];
                    *TryInto::<&mut [u8; 4]>::try_into(chunk).unwrap() = new_chunk;
                    lprev = new_chunk;
                }
            }
            BytesPerPixel::Six => {
                let mut lprev = [0; 6];
                for (chunk, above) in current.chunks_exact_mut(6).zip(previous.chunks_exact(6)) {
                    let new_chunk = [
                        chunk[0].wrapping_add(((above[0] as u16 + lprev[0] as u16) / 2) as u8),
                        chunk[1].wrapping_add(((above[1] as u16 + lprev[1] as u16) / 2) as u8),
                        chunk[2].wrapping_add(((above[2] as u16 + lprev[2] as u16) / 2) as u8),
                        chunk[3].wrapping_add(((above[3] as u16 + lprev[3] as u16) / 2) as u8),
                        chunk[4].wrapping_add(((above[4] as u16 + lprev[4] as u16) / 2) as u8),
                        chunk[5].wrapping_add(((above[5] as u16 + lprev[5] as u16) / 2) as u8),
                    ];
                    *TryInto::<&mut [u8; 6]>::try_into(chunk).unwrap() = new_chunk;
                    lprev = new_chunk;
                }
            }
            BytesPerPixel::Eight => {
                let mut lprev = [0; 8];
                for (chunk, above) in current.chunks_exact_mut(8).zip(previous.chunks_exact(8)) {
                    let new_chunk = [
                        chunk[0].wrapping_add(((above[0] as u16 + lprev[0] as u16) / 2) as u8),
                        chunk[1].wrapping_add(((above[1] as u16 + lprev[1] as u16) / 2) as u8),
                        chunk[2].wrapping_add(((above[2] as u16 + lprev[2] as u16) / 2) as u8),
                        chunk[3].wrapping_add(((above[3] as u16 + lprev[3] as u16) / 2) as u8),
                        chunk[4].wrapping_add(((above[4] as u16 + lprev[4] as u16) / 2) as u8),
                        chunk[5].wrapping_add(((above[5] as u16 + lprev[5] as u16) / 2) as u8),
                        chunk[6].wrapping_add(((above[6] as u16 + lprev[6] as u16) / 2) as u8),
                        chunk[7].wrapping_add(((above[7] as u16 + lprev[7] as u16) / 2) as u8),
                    ];
                    *TryInto::<&mut [u8; 8]>::try_into(chunk).unwrap() = new_chunk;
                    lprev = new_chunk;
                }
            }
        },
        #[allow(unreachable_code)]
        Paeth => {
            // Select the fastest Paeth filter implementation based on the target architecture.
            let filter_paeth_decode = if cfg!(target_arch = "x86_64") {
                filter_paeth_stbi
            } else {
                filter_paeth
            };

            // Paeth filter pixels:
            // C B D
            // A X
            match tbpp {
                BytesPerPixel::One => {
                    let mut a_bpp = [0; 1];
                    let mut c_bpp = [0; 1];
                    for (chunk, b_bpp) in current.chunks_exact_mut(1).zip(previous.chunks_exact(1))
                    {
                        let new_chunk = [chunk[0]
                            .wrapping_add(filter_paeth_decode(a_bpp[0], b_bpp[0], c_bpp[0]))];
                        *TryInto::<&mut [u8; 1]>::try_into(chunk).unwrap() = new_chunk;
                        a_bpp = new_chunk;
                        c_bpp = b_bpp.try_into().unwrap();
                    }
                }
                BytesPerPixel::Two => {
                    let mut a_bpp = [0; 2];
                    let mut c_bpp = [0; 2];
                    for (chunk, b_bpp) in current.chunks_exact_mut(2).zip(previous.chunks_exact(2))
                    {
                        let new_chunk = [
                            chunk[0]
                                .wrapping_add(filter_paeth_decode(a_bpp[0], b_bpp[0], c_bpp[0])),
                            chunk[1]
                                .wrapping_add(filter_paeth_decode(a_bpp[1], b_bpp[1], c_bpp[1])),
                        ];
                        *TryInto::<&mut [u8; 2]>::try_into(chunk).unwrap() = new_chunk;
                        a_bpp = new_chunk;
                        c_bpp = b_bpp.try_into().unwrap();
                    }
                }
                BytesPerPixel::Three => {
                    let mut a_bpp = [0; 3];
                    let mut c_bpp = [0; 3];

                    let mut previous = &previous[..previous.len() / 3 * 3];
                    let current_len = current.len();
                    let mut current = &mut current[..current_len / 3 * 3];

                    while let ([c0, c1, c2, c_rest @ ..], [p0, p1, p2, p_rest @ ..]) =
                        (current, previous)
                    {
                        current = c_rest;
                        previous = p_rest;

                        *c0 = c0.wrapping_add(filter_paeth_decode(a_bpp[0], *p0, c_bpp[0]));
                        *c1 = c1.wrapping_add(filter_paeth_decode(a_bpp[1], *p1, c_bpp[1]));
                        *c2 = c2.wrapping_add(filter_paeth_decode(a_bpp[2], *p2, c_bpp[2]));

                        a_bpp = [*c0, *c1, *c2];
                        c_bpp = [*p0, *p1, *p2];
                    }
                }
                BytesPerPixel::Four => {
                    // Using the `simd` module here has no effect on Linux
                    // and appears to regress performance on Windows, so we don't use it here.
                    // See https://github.com/image-rs/image-png/issues/567

                    let mut a_bpp = [0; 4];
                    let mut c_bpp = [0; 4];

                    let mut previous = &previous[..previous.len() & !3];
                    let current_len = current.len();
                    let mut current = &mut current[..current_len & !3];

                    while let ([c0, c1, c2, c3, c_rest @ ..], [p0, p1, p2, p3, p_rest @ ..]) =
                        (current, previous)
                    {
                        current = c_rest;
                        previous = p_rest;

                        *c0 = c0.wrapping_add(filter_paeth_decode(a_bpp[0], *p0, c_bpp[0]));
                        *c1 = c1.wrapping_add(filter_paeth_decode(a_bpp[1], *p1, c_bpp[1]));
                        *c2 = c2.wrapping_add(filter_paeth_decode(a_bpp[2], *p2, c_bpp[2]));
                        *c3 = c3.wrapping_add(filter_paeth_decode(a_bpp[3], *p3, c_bpp[3]));

                        a_bpp = [*c0, *c1, *c2, *c3];
                        c_bpp = [*p0, *p1, *p2, *p3];
                    }
                }
                BytesPerPixel::Six => {
                    let mut a_bpp = [0; 6];
                    let mut c_bpp = [0; 6];
                    for (chunk, b_bpp) in current.chunks_exact_mut(6).zip(previous.chunks_exact(6))
                    {
                        let new_chunk = [
                            chunk[0]
                                .wrapping_add(filter_paeth_decode(a_bpp[0], b_bpp[0], c_bpp[0])),
                            chunk[1]
                                .wrapping_add(filter_paeth_decode(a_bpp[1], b_bpp[1], c_bpp[1])),
                            chunk[2]
                                .wrapping_add(filter_paeth_decode(a_bpp[2], b_bpp[2], c_bpp[2])),
                            chunk[3]
                                .wrapping_add(filter_paeth_decode(a_bpp[3], b_bpp[3], c_bpp[3])),
                            chunk[4]
                                .wrapping_add(filter_paeth_decode(a_bpp[4], b_bpp[4], c_bpp[4])),
                            chunk[5]
                                .wrapping_add(filter_paeth_decode(a_bpp[5], b_bpp[5], c_bpp[5])),
                        ];
                        *TryInto::<&mut [u8; 6]>::try_into(chunk).unwrap() = new_chunk;
                        a_bpp = new_chunk;
                        c_bpp = b_bpp.try_into().unwrap();
                    }
                }
                BytesPerPixel::Eight => {
                    let mut a_bpp = [0; 8];
                    let mut c_bpp = [0; 8];
                    for (chunk, b_bpp) in current.chunks_exact_mut(8).zip(previous.chunks_exact(8))
                    {
                        let new_chunk = [
                            chunk[0]
                                .wrapping_add(filter_paeth_decode(a_bpp[0], b_bpp[0], c_bpp[0])),
                            chunk[1]
                                .wrapping_add(filter_paeth_decode(a_bpp[1], b_bpp[1], c_bpp[1])),
                            chunk[2]
                                .wrapping_add(filter_paeth_decode(a_bpp[2], b_bpp[2], c_bpp[2])),
                            chunk[3]
                                .wrapping_add(filter_paeth_decode(a_bpp[3], b_bpp[3], c_bpp[3])),
                            chunk[4]
                                .wrapping_add(filter_paeth_decode(a_bpp[4], b_bpp[4], c_bpp[4])),
                            chunk[5]
                                .wrapping_add(filter_paeth_decode(a_bpp[5], b_bpp[5], c_bpp[5])),
                            chunk[6]
                                .wrapping_add(filter_paeth_decode(a_bpp[6], b_bpp[6], c_bpp[6])),
                            chunk[7]
                                .wrapping_add(filter_paeth_decode(a_bpp[7], b_bpp[7], c_bpp[7])),
                        ];
                        *TryInto::<&mut [u8; 8]>::try_into(chunk).unwrap() = new_chunk;
                        a_bpp = new_chunk;
                        c_bpp = b_bpp.try_into().unwrap();
                    }
                }
            }
        }
    }
}

fn filter_internal(
    method: RowFilter,
    bpp: usize,
    len: usize,
    previous: &[u8],
    current: &[u8],
    output: &mut [u8],
) -> RowFilter {
    use self::RowFilter::*;

    // This value was chosen experimentally based on what achieved the best performance. The
    // Rust compiler does auto-vectorization, and 32-bytes per loop iteration seems to enable
    // the fastest code when doing so.
    const CHUNK_SIZE: usize = 32;

    match method {
        NoFilter => {
            output.copy_from_slice(current);
            NoFilter
        }
        Sub => {
            let mut out_chunks = output[bpp..].chunks_exact_mut(CHUNK_SIZE);
            let mut cur_chunks = current[bpp..].chunks_exact(CHUNK_SIZE);
            let mut prev_chunks = current[..len - bpp].chunks_exact(CHUNK_SIZE);

            for ((out, cur), prev) in (&mut out_chunks).zip(&mut cur_chunks).zip(&mut prev_chunks) {
                for i in 0..CHUNK_SIZE {
                    out[i] = cur[i].wrapping_sub(prev[i]);
                }
            }

            for ((out, cur), &prev) in out_chunks
                .into_remainder()
                .iter_mut()
                .zip(cur_chunks.remainder())
                .zip(prev_chunks.remainder())
            {
                *out = cur.wrapping_sub(prev);
            }

            output[..bpp].copy_from_slice(&current[..bpp]);
            Sub
        }
        Up => {
            let mut out_chunks = output.chunks_exact_mut(CHUNK_SIZE);
            let mut cur_chunks = current.chunks_exact(CHUNK_SIZE);
            let mut prev_chunks = previous.chunks_exact(CHUNK_SIZE);

            for ((out, cur), prev) in (&mut out_chunks).zip(&mut cur_chunks).zip(&mut prev_chunks) {
                for i in 0..CHUNK_SIZE {
                    out[i] = cur[i].wrapping_sub(prev[i]);
                }
            }

            for ((out, cur), &prev) in out_chunks
                .into_remainder()
                .iter_mut()
                .zip(cur_chunks.remainder())
                .zip(prev_chunks.remainder())
            {
                *out = cur.wrapping_sub(prev);
            }
            Up
        }
        Avg => {
            let mut out_chunks = output[bpp..].chunks_exact_mut(CHUNK_SIZE);
            let mut cur_chunks = current[bpp..].chunks_exact(CHUNK_SIZE);
            let mut cur_minus_bpp_chunks = current[..len - bpp].chunks_exact(CHUNK_SIZE);
            let mut prev_chunks = previous[bpp..].chunks_exact(CHUNK_SIZE);

            for (((out, cur), cur_minus_bpp), prev) in (&mut out_chunks)
                .zip(&mut cur_chunks)
                .zip(&mut cur_minus_bpp_chunks)
                .zip(&mut prev_chunks)
            {
                for i in 0..CHUNK_SIZE {
                    // Bitwise average of two integers without overflow and
                    // without converting to a wider bit-width. See:
                    // http://aggregate.org/MAGIC/#Average%20of%20Integers
                    // If this is unrolled by component, consider reverting to
                    // `((cur_minus_bpp[i] as u16 + prev[i] as u16) / 2) as u8`
                    out[i] = cur[i].wrapping_sub(
                        (cur_minus_bpp[i] & prev[i]) + ((cur_minus_bpp[i] ^ prev[i]) >> 1),
                    );
                }
            }

            for (((out, cur), &cur_minus_bpp), &prev) in out_chunks
                .into_remainder()
                .iter_mut()
                .zip(cur_chunks.remainder())
                .zip(cur_minus_bpp_chunks.remainder())
                .zip(prev_chunks.remainder())
            {
                *out = cur.wrapping_sub((cur_minus_bpp & prev) + ((cur_minus_bpp ^ prev) >> 1));
            }

            for i in 0..bpp {
                output[i] = current[i].wrapping_sub(previous[i] / 2);
            }
            Avg
        }
        Paeth => {
            let mut out_chunks = output[bpp..].chunks_exact_mut(CHUNK_SIZE);
            let mut cur_chunks = current[bpp..].chunks_exact(CHUNK_SIZE);
            let mut a_chunks = current[..len - bpp].chunks_exact(CHUNK_SIZE);
            let mut b_chunks = previous[bpp..].chunks_exact(CHUNK_SIZE);
            let mut c_chunks = previous[..len - bpp].chunks_exact(CHUNK_SIZE);

            for ((((out, cur), a), b), c) in (&mut out_chunks)
                .zip(&mut cur_chunks)
                .zip(&mut a_chunks)
                .zip(&mut b_chunks)
                .zip(&mut c_chunks)
            {
                for i in 0..CHUNK_SIZE {
                    out[i] = cur[i].wrapping_sub(filter_paeth_fpnge(a[i], b[i], c[i]));
                }
            }

            for ((((out, cur), &a), &b), &c) in out_chunks
                .into_remainder()
                .iter_mut()
                .zip(cur_chunks.remainder())
                .zip(a_chunks.remainder())
                .zip(b_chunks.remainder())
                .zip(c_chunks.remainder())
            {
                *out = cur.wrapping_sub(filter_paeth_fpnge(a, b, c));
            }

            for i in 0..bpp {
                output[i] = current[i].wrapping_sub(filter_paeth_fpnge(0, previous[i], 0));
            }
            Paeth
        }
    }
}

pub(crate) fn filter(
    method: Filter,
    bpp: BytesPerPixel,
    previous: &[u8],
    current: &[u8],
    output: &mut [u8],
) -> RowFilter {
    use RowFilter::*;
    let bpp = bpp.into_usize();
    let len = current.len();

    match method {
        Filter::Adaptive => {
            let mut min_sum: u64 = u64::MAX;
            let mut filter_choice = RowFilter::NoFilter;
            for &filter in [Sub, Up, Avg, Paeth].iter() {
                filter_internal(filter, bpp, len, previous, current, output);
                let sum = sum_buffer(output);
                if sum <= min_sum {
                    min_sum = sum;
                    filter_choice = filter;
                }
            }

            if filter_choice != Paeth {
                filter_internal(filter_choice, bpp, len, previous, current, output);
            }
            filter_choice
        }
        _ => {
            let filter = RowFilter::from_method(method).unwrap();
            filter_internal(filter, bpp, len, previous, current, output)
        }
    }
}

// Helper function for Adaptive filter buffer summation
fn sum_buffer(buf: &[u8]) -> u64 {
    const CHUNK_SIZE: usize = 32;

    let mut buf_chunks = buf.chunks_exact(CHUNK_SIZE);
    let mut sum = 0_u64;

    for chunk in &mut buf_chunks {
        // At most, `acc` can be `32 * (i8::MIN as u8) = 32 * 128 = 4096`.
        let mut acc = 0;
        for &b in chunk {
            acc += u64::from((b as i8).unsigned_abs());
        }
        sum = sum.saturating_add(acc);
    }

    let mut acc = 0;
    for &b in buf_chunks.remainder() {
        acc += u64::from((b as i8).unsigned_abs());
    }

    sum.saturating_add(acc)
}

#[cfg(test)]
mod test {
    use super::*;
    use core::iter;

    #[test]
    fn roundtrip() {
        // A multiple of 8, 6, 4, 3, 2, 1
        const LEN: u8 = 240;
        let previous: Vec<_> = iter::repeat(1).take(LEN.into()).collect();
        let current: Vec<_> = (0..LEN).collect();
        let expected = current.clone();

        let roundtrip = |kind: RowFilter, bpp: BytesPerPixel| {
            let mut output = vec![0; LEN.into()];
            filter(kind.into(), bpp, &previous, &current, &mut output);
            unfilter(kind, bpp, &previous, &mut output);
            assert_eq!(
                output, expected,
                "Filtering {:?} with {:?} does not roundtrip",
                bpp, kind
            );
        };

        let filters = [
            RowFilter::NoFilter,
            RowFilter::Sub,
            RowFilter::Up,
            RowFilter::Avg,
            RowFilter::Paeth,
        ];

        let bpps = [
            BytesPerPixel::One,
            BytesPerPixel::Two,
            BytesPerPixel::Three,
            BytesPerPixel::Four,
            BytesPerPixel::Six,
            BytesPerPixel::Eight,
        ];

        for &filter in filters.iter() {
            for &bpp in bpps.iter() {
                roundtrip(filter, bpp);
            }
        }
    }

    #[test]
    #[ignore] // takes ~20s without optimizations
    fn paeth_impls_are_equivalent() {
        for a in 0..=255 {
            for b in 0..=255 {
                for c in 0..=255 {
                    let baseline = filter_paeth(a, b, c);
                    let fpnge = filter_paeth_fpnge(a, b, c);
                    let stbi = filter_paeth_stbi(a, b, c);

                    assert_eq!(baseline, fpnge);
                    assert_eq!(baseline, stbi);
                }
            }
        }
    }

    #[test]
    fn roundtrip_ascending_previous_line() {
        // A multiple of 8, 6, 4, 3, 2, 1
        const LEN: u8 = 240;
        let previous: Vec<_> = (0..LEN).collect();
        let current: Vec<_> = (0..LEN).collect();
        let expected = current.clone();

        let roundtrip = |kind: RowFilter, bpp: BytesPerPixel| {
            let mut output = vec![0; LEN.into()];
            filter(kind.into(), bpp, &previous, &current, &mut output);
            unfilter(kind, bpp, &previous, &mut output);
            assert_eq!(
                output, expected,
                "Filtering {:?} with {:?} does not roundtrip",
                bpp, kind
            );
        };

        let filters = [
            RowFilter::NoFilter,
            RowFilter::Sub,
            RowFilter::Up,
            RowFilter::Avg,
            RowFilter::Paeth,
        ];

        let bpps = [
            BytesPerPixel::One,
            BytesPerPixel::Two,
            BytesPerPixel::Three,
            BytesPerPixel::Four,
            BytesPerPixel::Six,
            BytesPerPixel::Eight,
        ];

        for &filter in filters.iter() {
            for &bpp in bpps.iter() {
                roundtrip(filter, bpp);
            }
        }
    }

    #[test]
    // This tests that converting u8 to i8 doesn't overflow when taking the
    // absolute value for adaptive filtering: -128_i8.abs() will panic in debug
    // or produce garbage in release mode. The sum of 0..=255u8 should equal the
    // sum of the absolute values of -128_i8..=127, or abs(-128..=0) + 1..=127.
    fn sum_buffer_test() {
        let sum = (0..=128).sum::<u64>() + (1..=127).sum::<u64>();
        let buf: Vec<u8> = (0_u8..=255).collect();

        assert_eq!(sum, crate::filter::sum_buffer(&buf));
    }
}
