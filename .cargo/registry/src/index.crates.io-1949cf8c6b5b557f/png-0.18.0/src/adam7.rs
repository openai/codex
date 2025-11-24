//! Utility functions related to handling of
//! [the Adam7 algorithm](https://en.wikipedia.org/wiki/Adam7_algorithm).
use core::ops::RangeTo;

/// Describes which stage of
/// [the Adam7 algorithm](https://en.wikipedia.org/wiki/Adam7_algorithm)
/// applies to a decoded row.
///
/// See also [Reader.next_interlaced_row](crate::decoder::Reader::next_interlaced_row).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Adam7Info {
    /// The Adam7 pass number, 1..7.
    pub(crate) pass: u8,
    /// The index of the line within this pass.
    pub(crate) line: u32,
    /// The original pixel count.
    pub(crate) width: u32,
    /// How many Adam7 samples there are.
    pub(crate) samples: u32,
}

/// The index of a bit in the image buffer.
///
/// We do not use a pure `usize` to avoid overflows on 32-bit targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BitPostion {
    byte: usize,
    /// [0..8)
    bit: u8,
}

impl Adam7Info {
    /// Creates a new `Adam7Info`.  May panic if the arguments are out of range (e.g. if `pass` is
    /// 0 or greater than 8).
    ///
    /// * `pass` corresponds to a pass of the
    ///   [the Adam7 algorithm](https://en.wikipedia.org/wiki/Adam7_algorithm)
    /// * `line` is the number of a line within a pass (starting with 0).  For example,
    ///   in an image of height 8, `line` can be beteween `0..4` in the 7th `pass`
    ///   (those 4 interlaced rows correspond to 2nd, 4th, 6th, and 8th row of the full image).
    /// * `width` describes how many pixels are in a full row of the image. The bytes in each
    ///   passline of the Adam7 are calculated from this number.
    ///
    /// Note that in typical usage, `Adam7Info`s are returned by [Reader.next_interlaced_row]
    /// and there is no need to create them by calling `Adam7Info::new`.  `Adam7Info::new` is
    /// nevertheless exposed as a public API, because it helps to provide self-contained example
    /// usage of [expand_interlaced_row](crate::expand_interlaced_row).
    pub fn new(pass: u8, line: u32, width: u32) -> Self {
        assert!(1 <= pass && pass <= 7);
        assert!(width > 0);

        let info = PassConstants::PASSES[pass as usize - 1];
        let samples = info.count_samples(width);

        Self {
            pass,
            line,
            width,
            samples,
        }
    }

    fn pass_constants(&self) -> PassConstants {
        PassConstants::PASSES[self.pass as usize - 1]
    }

    /// How often to repeat a pixel.
    fn splat_pixel_repeat(self, idx: usize) -> u8 {
        let pass = self.pass_constants();
        let x_pixel = idx as u32 * u32::from(pass.x_sampling) + u32::from(pass.x_offset);
        (self.width - x_pixel).min(pass.splat_x_repeat().into()) as u8
    }

    fn splat_line_repeat(self, height: u32) -> u8 {
        let pass = self.pass_constants();
        let y_line = self.line * u32::from(pass.y_sampling) + u32::from(pass.y_offset);
        (height - y_line).min(pass.splat_y_repeat().into()) as u8
    }
}

#[derive(Clone, Copy)]
struct PassConstants {
    x_sampling: u8,
    x_offset: u8,
    y_sampling: u8,
    y_offset: u8,
}

impl PassConstants {
    const fn splat_x_repeat(self) -> u8 {
        self.x_sampling - self.x_offset
    }

    const fn splat_y_repeat(self) -> u8 {
        self.y_sampling - self.y_offset
    }

    fn count_samples(self, width: u32) -> u32 {
        width
            .saturating_sub(u32::from(self.x_offset))
            .div_ceil(u32::from(self.x_sampling))
    }

    fn count_lines(self, height: u32) -> u32 {
        height
            .saturating_sub(u32::from(self.y_offset))
            .div_ceil(u32::from(self.y_sampling))
    }

    /// The constants associated with each of the 7 passes. Note that it is 0-indexed while the
    /// pass number (as per specification) is 1-indexed.
    pub const PASSES: [Self; 7] = {
        // Shortens the constructor for readability, retains clear argument order below.
        const fn new(x_sampling: u8, x_offset: u8, y_sampling: u8, y_offset: u8) -> PassConstants {
            PassConstants {
                x_sampling,
                x_offset,
                y_sampling,
                y_offset,
            }
        }

        [
            new(8, 0, 8, 0),
            new(8, 4, 8, 0),
            new(4, 0, 8, 4),
            new(4, 2, 4, 0),
            new(2, 0, 4, 2),
            new(2, 1, 2, 0),
            new(1, 0, 2, 1),
        ]
    };
}

/// This iterator iterates over the different passes of an image Adam7 encoded
/// PNG image
/// The pattern is:
///     16462646
///     77777777
///     56565656
///     77777777
///     36463646
///     77777777
///     56565656
///     77777777
///
#[derive(Clone)]
pub(crate) struct Adam7Iterator {
    line: u32,
    lines: u32,
    line_width: u32,
    current_pass: u8,
    width: u32,
    height: u32,
}

impl Adam7Iterator {
    pub fn new(width: u32, height: u32) -> Adam7Iterator {
        let mut this = Adam7Iterator {
            line: 0,
            lines: 0,
            line_width: 0,
            current_pass: 1,
            width,
            height,
        };
        this.init_pass();
        this
    }

    /// Calculates the bounds of the current pass
    fn init_pass(&mut self) {
        let info = PassConstants::PASSES[self.current_pass as usize - 1];
        self.line_width = info.count_samples(self.width);
        self.lines = info.count_lines(self.height);
        self.line = 0;
    }
}

/// Iterates over `Adam7Info`s.
impl Iterator for Adam7Iterator {
    type Item = Adam7Info;
    fn next(&mut self) -> Option<Self::Item> {
        if self.line < self.lines && self.line_width > 0 {
            let this_line = self.line;
            self.line += 1;
            Some(Adam7Info {
                pass: self.current_pass,
                line: this_line,
                width: self.width,
                samples: self.line_width,
            })
        } else if self.current_pass < 7 {
            self.current_pass += 1;
            self.init_pass();
            self.next()
        } else {
            None
        }
    }
}

/// The algorithm to use when progressively filling pixel data from Adam7 interlaced passes.
///
/// Adam7 interlacing is a technique optionally used in PNG by which only a sub-sample of pixel
/// data is encoded in the beginning of the image data chunks, followed by progressively larger
/// subsets of the data in subsequent passes. Therefore a 'rough image' is available after ust a
/// very tiny fraction of the data has been read which can be advantageous for loading an image
/// from a slow IO medium while optimizing time-to-first-meaningful-paint and then replacing the
/// presented data as it is streamed in.
///
/// There are trade-offs to make here. The strictly necessary requirement for an implementation is
/// that the exact image is recovered after all passes are applied. However the intermediate states
/// of the output are left to the implementation, as long as it follows the restriction of
/// resulting in the intended image when all passes have been applied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Adam7Variant {
    /// This is the adam7 de-interlace we do by default. Only pixels related to the pass are
    /// written. The output buffer should not be directly used for presentation until all passes
    /// are complete. At least the invalid pixels in the buffer should be masked. However, this
    /// performs the least amount of writes and is optimal when you're only reading full frames.
    ///
    /// This corresponds to [`crate::expand_interlaced_row`].
    #[default]
    Sparse,
    /// A variant of the Adam7 de-interlace that ensures that all pixels are initialized after each
    /// pass, and are progressively refined towards the final image. Performs more writes than the
    /// other variant as some pixels are touched repeatedly, but ensures the buffer can be used as
    /// directly as possible for presentation.
    ///
    /// This corresponds to [`crate::splat_interlaced_row`].
    Splat,
}

fn subbyte_values<const N: usize>(
    scanline: &[u8],
    bit_pos: [u8; N],
    mask: u8,
) -> impl Iterator<Item = u8> + '_ {
    (scanline.iter().copied()).flat_map(move |value| bit_pos.map(|n| (value >> n) & mask))
}

/// Given `row_stride`, interlace `info`, and bits-per-pixel, produce an iterator of bit positions
/// of pixels to copy from the input scanline to the image buffer.  The positions are expressed as
/// bit offsets from position (0,0) in the frame that is currently being decoded.
///
/// This should only be used with `bits_pp < 8`.
fn expand_adam7_bits(
    row_stride_in_bytes: usize,
    info: &Adam7Info,
    bits_pp: u8,
) -> impl Iterator<Item = BitPostion> {
    debug_assert!(bits_pp == 1 || bits_pp == 2 || bits_pp == 4);
    let (line_mul, line_off, samp_mul, samp_off) = {
        let constants = info.pass_constants();
        (
            // Convert each to their respectively required type from u8.
            usize::from(constants.y_sampling),
            usize::from(constants.y_offset),
            u64::from(constants.x_sampling),
            u64::from(constants.x_offset),
        )
    };

    // the equivalent line number in progressive scan
    let prog_line = line_mul * info.line as usize + line_off;
    let byte_start = prog_line * row_stride_in_bytes;

    // In contrast to `subbyte_values` we *must* be precise with our length here.
    (0..u64::from(info.samples))
        // Bounded by u32::MAX * 8 * 4 + 16 so does not overflow `u64`.
        .map(move |i| (i * samp_mul + samp_off) * u64::from(bits_pp))
        .map(move |i| BitPostion {
            // Bounded by the buffer size which already exists.
            byte: byte_start + (i / 8) as usize,
            bit: i as u8 % 8,
        })
}

fn expand_adam7_bytes(
    row_stride_in_bytes: usize,
    info: &Adam7Info,
    bytes_pp: u8,
) -> impl Iterator<Item = usize> {
    let (line_mul, line_off, samp_mul, samp_off) = {
        let constants = info.pass_constants();
        (
            // Convert each to their respectively required type from u8.
            usize::from(constants.y_sampling),
            usize::from(constants.y_offset),
            u64::from(constants.x_sampling),
            u64::from(constants.x_offset),
        )
    };

    // the equivalent line number in progressive scan
    let prog_line = line_mul * info.line as usize + line_off;
    let byte_start = prog_line * row_stride_in_bytes;

    (0..u64::from(info.samples))
        .map(move |i| (i * samp_mul + samp_off) * u64::from(bytes_pp))
        // Bounded by the allocated buffer size so must fit in `usize`
        .map(move |i| i as usize + byte_start)
}

/// Copies pixels from `interlaced_row` into the right location in `img`.
///
/// First bytes of `img` should belong to the top-left corner of the currently decoded frame.
///
/// `img_row_stride` specifies an offset in bytes between subsequent rows of `img`.
/// This can be the width of the current frame being decoded, but this is not required - a bigger
/// stride may be useful if the frame being decoded is a sub-region of `img`.
///
/// `interlaced_row` and `interlace_info` typically come from
/// [crate::decoder::Reader::next_interlaced_row], but this is not required.  In particular, before
/// calling `expand_interlaced_row` one may need to expand the decoded row, so that its format and
/// `bits_per_pixel` matches that of `img`.  Note that in initial Adam7 passes the `interlaced_row`
/// may contain less pixels that the width of the frame being decoded (e.g. it contains only 1/8th
/// of pixels in the initial pass).
///
/// Example:
///
/// ```
/// use png::{expand_interlaced_row, Adam7Info};
/// let info = Adam7Info::new(5, 0, 8);
/// let mut img = vec![0; 8 * 8];
/// let row = vec![1, 2, 3, 4];
/// expand_interlaced_row(&mut img, 8, &row, &info, 8);
/// assert_eq!(&img, &[
///     0, 0, 0, 0, 0, 0, 0, 0,
///     0, 0, 0, 0, 0, 0, 0, 0,
///     1, 0, 2, 0, 3, 0, 4, 0,  // <= this is where the 1st line of 5s appears
///     0, 0, 0, 0, 0, 0, 0, 0,  //    in the schematic drawing of the passes at
///     0, 0, 0, 0, 0, 0, 0, 0,  //    https://en.wikipedia.org/wiki/Adam7_algorithm
///     0, 0, 0, 0, 0, 0, 0, 0,
///     0, 0, 0, 0, 0, 0, 0, 0,
///     0, 0, 0, 0, 0, 0, 0, 0,
/// ]);
/// ```
pub fn expand_pass(
    img: &mut [u8],
    img_row_stride: usize,
    interlaced_row: &[u8],
    interlace_info: &Adam7Info,
    bits_per_pixel: u8,
) {
    match bits_per_pixel {
        // Note: for 1, 2, 4 multiple runs through the iteration will access the same byte in `img`
        // so we can not iterate over `&mut u8` values. A better strategy would write multiple bit
        // groups in one go. This would then also not be as bounds-check heavy?
        1 => {
            const BIT_POS_1: [u8; 8] = [7, 6, 5, 4, 3, 2, 1, 0];
            let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 1);
            for (pos, px) in bit_indices.zip(subbyte_values(interlaced_row, BIT_POS_1, 0b1)) {
                let shift = 8 - bits_per_pixel - pos.bit;
                img[pos.byte] |= px << shift;
            }
        }
        2 => {
            const BIT_POS_2: [u8; 4] = [6, 4, 2, 0];
            let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 2);

            for (pos, px) in bit_indices.zip(subbyte_values(interlaced_row, BIT_POS_2, 0b11)) {
                let shift = 8 - bits_per_pixel - pos.bit;
                img[pos.byte] |= px << shift;
            }
        }
        4 => {
            const BIT_POS_4: [u8; 2] = [4, 0];
            let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 4);

            for (pos, px) in bit_indices.zip(subbyte_values(interlaced_row, BIT_POS_4, 0b1111)) {
                let shift = 8 - bits_per_pixel - pos.bit;
                img[pos.byte] |= px << shift;
            }
        }
        // While caught by the below loop, we special case this for codegen. The effects are
        // massive when the compiler uses the constant chunk size in particular for this case where
        // no more copy_from_slice is being issued by everything happens in the register alone.
        8 => {
            let byte_indices = expand_adam7_bytes(img_row_stride, interlace_info, 1);

            for (bytepos, &px) in byte_indices.zip(interlaced_row) {
                img[bytepos] = px;
            }
        }
        16 => {
            let byte_indices = expand_adam7_bytes(img_row_stride, interlace_info, 2);

            for (bytepos, px) in byte_indices.zip(interlaced_row.chunks(2)) {
                img[bytepos..][..2].copy_from_slice(px);
            }
        }
        _ => {
            debug_assert!(bits_per_pixel % 8 == 0);
            let bytes_pp = bits_per_pixel / 8;
            let byte_indices = expand_adam7_bytes(img_row_stride, interlace_info, bytes_pp);

            for (bytepos, px) in byte_indices.zip(interlaced_row.chunks(bytes_pp.into())) {
                img[bytepos..][..px.len()].copy_from_slice(px);
            }
        }
    }
}

/// Expand pass, but also ensure that after each pass the whole image has been initialized up to
/// the data available. In constrast to `expand_pass` there are no holes left in the image.
///
/// For instance, consider the first pass which is an eighth subsampling of the original image.
/// Here's a side by-side of pixel data written from each of the two algorithms:
///
/// ```text
/// normal:   splat:
/// 1-------  11111111
/// --------  11111111
/// --------  11111111
/// --------  11111111
/// --------  11111111
/// --------  11111111
/// --------  11111111
/// ```
///
/// Data written in previous passes must not be modified. We 'weave' the data of passes and repeat
/// them in the neighbouring pixels until their subsampling alignment. For details, see the
/// `x_repeat` and `y_repeat` data. Thus the 4th pass would look like this:
///
/// ```text
/// normal:   splat:
/// --4---4-  --44--44
/// --------  --44--44
/// --------  --44--44
/// --4---4-  --44--44
/// --------  --44--44
/// --------  --44--44
/// --------  --44--44
/// ```
///
pub fn expand_pass_splat(
    img: &mut [u8],
    img_row_stride: usize,
    interlaced_row: &[u8],
    interlace_info: &Adam7Info,
    bits_per_pixel: u8,
) {
    fn expand_bits_to_img(
        img: &mut [u8],
        px: u8,
        mut pos: BitPostion,
        repeat: RangeTo<u8>,
        bpp: u8,
    ) {
        let (mut into, mut tail) = img[pos.byte..].split_first_mut().unwrap();
        let mask = (1u8 << bpp) - 1;

        for _ in 0..repeat.end {
            if pos.bit >= 8 {
                pos.byte += 1;
                pos.bit -= 8;

                (into, tail) = tail.split_first_mut().unwrap();
            }

            let shift = 8 - bpp - pos.bit;
            // Preserve all other bits, but be prepared for existing bits
            let pre = (*into >> shift) & mask;
            *into ^= (pre ^ px) << shift;

            pos.bit += bpp;
        }
    }

    let height = (img.len() / img_row_stride) as u32;
    let y_repeat = interlace_info.splat_line_repeat(height);
    debug_assert!(y_repeat > 0);

    match bits_per_pixel {
        // Note: for 1, 2, 4 multiple runs through the iteration will access the same byte in `img`
        // so we can not iterate over `&mut u8` values. A better strategy would write multiple bit
        // groups in one go. This would then also not be as bounds-check heavy?
        1 => {
            const BIT_POS_1: [u8; 8] = [7, 6, 5, 4, 3, 2, 1, 0];

            for offset in 0..y_repeat {
                let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 1);
                let line_offset = usize::from(offset) * img_row_stride;

                for (idx, (mut pos, px)) in bit_indices
                    .zip(subbyte_values(interlaced_row, BIT_POS_1, 0b1))
                    .enumerate()
                {
                    let x_repeat = interlace_info.splat_pixel_repeat(idx);
                    debug_assert!(x_repeat > 0);
                    pos.byte += line_offset;
                    expand_bits_to_img(img, px, pos, ..x_repeat, bits_per_pixel);
                }
            }
        }
        2 => {
            const BIT_POS_2: [u8; 4] = [6, 4, 2, 0];

            for offset in 0..y_repeat {
                let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 2);
                let line_offset = usize::from(offset) * img_row_stride;

                for (idx, (mut pos, px)) in bit_indices
                    .zip(subbyte_values(interlaced_row, BIT_POS_2, 0b11))
                    .enumerate()
                {
                    let x_repeat = interlace_info.splat_pixel_repeat(idx);
                    pos.byte += line_offset;
                    expand_bits_to_img(img, px, pos, ..x_repeat, bits_per_pixel);
                }
            }
        }
        4 => {
            const BIT_POS_4: [u8; 2] = [4, 0];

            for offset in 0..y_repeat {
                let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 4);
                let line_offset = usize::from(offset) * img_row_stride;

                for (idx, (mut pos, px)) in bit_indices
                    .zip(subbyte_values(interlaced_row, BIT_POS_4, 0b1111))
                    .enumerate()
                {
                    let x_repeat = interlace_info.splat_pixel_repeat(idx);
                    pos.byte += line_offset;
                    expand_bits_to_img(img, px, pos, ..x_repeat, bits_per_pixel);
                }
            }
        }
        // While caught by the below loop, we special case this for codegen. The effects are
        // massive when the compiler uses the constant chunk size in particular for this case where
        // no more copy_from_slice is being issued by everything happens in the register alone.
        8 => {
            for offset in 0..y_repeat {
                let byte_indices = expand_adam7_bytes(img_row_stride, interlace_info, 1);
                let line_offset = usize::from(offset) * img_row_stride;

                for (idx, (bytepos, px)) in byte_indices.zip(interlaced_row).enumerate() {
                    let x_repeat = usize::from(interlace_info.splat_pixel_repeat(idx));
                    debug_assert!(x_repeat > 0);
                    img[line_offset + bytepos..][..x_repeat].fill(*px);
                }
            }
        }
        _ => {
            debug_assert!(bits_per_pixel % 8 == 0);
            let bytes = bits_per_pixel / 8;
            let chunk = usize::from(bytes);

            for offset in 0..y_repeat {
                let byte_indices = expand_adam7_bytes(img_row_stride, interlace_info, bytes);
                let line_offset = usize::from(offset) * img_row_stride;

                for (idx, (bytepos, px)) in byte_indices
                    .zip(interlaced_row.chunks_exact(chunk))
                    .enumerate()
                {
                    let x_repeat = usize::from(interlace_info.splat_pixel_repeat(idx));
                    let target = &mut img[line_offset + bytepos..][..chunk * x_repeat];

                    for target in target.chunks_exact_mut(chunk) {
                        target.copy_from_slice(px);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adam7() {
        /*
            1646
            7777
            5656
            7777
        */
        let it = Adam7Iterator::new(4, 4);
        let passes: Vec<_> = it.collect();
        assert_eq!(
            &*passes,
            &[
                Adam7Info {
                    pass: 1,
                    line: 0,
                    samples: 1,
                    width: 4,
                },
                Adam7Info {
                    pass: 4,
                    line: 0,
                    samples: 1,
                    width: 4,
                },
                Adam7Info {
                    pass: 5,
                    line: 0,
                    samples: 2,
                    width: 4,
                },
                Adam7Info {
                    pass: 6,
                    line: 0,
                    samples: 2,
                    width: 4,
                },
                Adam7Info {
                    pass: 6,
                    line: 1,
                    samples: 2,
                    width: 4,
                },
                Adam7Info {
                    pass: 7,
                    line: 0,
                    samples: 4,
                    width: 4,
                },
                Adam7Info {
                    pass: 7,
                    line: 1,
                    samples: 4,
                    width: 4,
                }
            ]
        );
    }

    #[test]
    fn test_subbyte_pixels() {
        const BIT_POS_1: [u8; 8] = [7, 6, 5, 4, 3, 2, 1, 0];

        let scanline = &[0b10101010, 0b10101010];
        let pixels = subbyte_values(scanline, BIT_POS_1, 1).collect::<Vec<_>>();

        assert_eq!(pixels.len(), 16);
        assert_eq!(pixels, [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0]);
    }

    #[test]
    fn test_expand_adam7_bits() {
        let width = 32;
        let bits_pp = 1;
        let stride = width / 8;
        let info =
            |pass, line, img_width| create_adam7_info_for_tests(pass, line as u32, img_width);

        let expected = |offset: usize, step: usize, count: usize| {
            (0..count)
                .map(move |i| step * i + offset)
                .map(|i| BitPostion {
                    byte: i / 8,
                    bit: (i % 8) as u8,
                })
                .collect::<Vec<_>>()
        };

        for line_no in 0..8 {
            let start = 8 * line_no * width;

            assert_eq!(
                expand_adam7_bits(stride, &info(1, line_no, width), bits_pp).collect::<Vec<_>>(),
                expected(start, 8, 4)
            );

            let start = start + 4;

            assert_eq!(
                expand_adam7_bits(stride, &info(2, line_no, width), bits_pp).collect::<Vec<_>>(),
                expected(start, 8, 4)
            );

            let start = (8 * line_no + 4) * width;

            assert_eq!(
                expand_adam7_bits(stride, &info(3, line_no, width), bits_pp).collect::<Vec<_>>(),
                expected(start, 4, 8)
            );
        }

        for line_no in 0..16 {
            let start = 4 * line_no * width + 2;

            assert_eq!(
                expand_adam7_bits(stride, &info(4, line_no, width), bits_pp).collect::<Vec<_>>(),
                expected(start, 4, 8)
            );

            let start = (4 * line_no + 2) * width;

            assert_eq!(
                expand_adam7_bits(stride, &info(5, line_no, width), bits_pp).collect::<Vec<_>>(),
                expected(start, 2, 16)
            )
        }

        for line_no in 0..32 {
            let start = 2 * line_no * width + 1;

            assert_eq!(
                expand_adam7_bits(stride, &info(6, line_no, width), bits_pp).collect::<Vec<_>>(),
                expected(start, 2, 16),
                "line_no: {}",
                line_no
            );

            let start = (2 * line_no + 1) * width;

            assert_eq!(
                expand_adam7_bits(stride, &info(7, line_no, width), bits_pp).collect::<Vec<_>>(),
                expected(start, 1, 32)
            );
        }
    }

    #[test]
    fn test_expand_adam7_bits_independent_row_stride() {
        let pass = 1;
        let line_no = 1;
        let width = 32;
        let info = create_adam7_info_for_tests;

        {
            let stride = width;
            assert_eq!(
                expand_adam7_bytes(stride, &info(pass, line_no, width), 1).collect::<Vec<_>>(),
                [2048, 2112, 2176, 2240].map(|n| n / 8),
            );
        }

        {
            let stride = 10000;
            assert_eq!(
                expand_adam7_bytes(stride, &info(pass, line_no, width), 1).collect::<Vec<_>>(),
                [640000, 640064, 640128, 640192].map(|n| n / 8),
            );
        }
    }

    #[test]
    fn test_expand_pass_subbyte() {
        let mut img = [0u8; 8];
        let width = 8;
        let stride = width / 8;
        let bits_pp = 1;
        let info = create_adam7_info_for_tests;

        expand_pass(&mut img, stride, &[0b10000000], &info(1, 0, width), bits_pp);
        assert_eq!(img, [0b10000000u8, 0, 0, 0, 0, 0, 0, 0]);

        expand_pass(&mut img, stride, &[0b10000000], &info(2, 0, width), bits_pp);
        assert_eq!(img, [0b10001000u8, 0, 0, 0, 0, 0, 0, 0]);

        expand_pass(&mut img, stride, &[0b11000000], &info(3, 0, width), bits_pp);
        assert_eq!(img, [0b10001000u8, 0, 0, 0, 0b10001000, 0, 0, 0]);

        expand_pass(&mut img, stride, &[0b11000000], &info(4, 0, width), bits_pp);
        assert_eq!(img, [0b10101010u8, 0, 0, 0, 0b10001000, 0, 0, 0]);

        expand_pass(&mut img, stride, &[0b11000000], &info(4, 1, width), bits_pp);
        assert_eq!(img, [0b10101010u8, 0, 0, 0, 0b10101010, 0, 0, 0]);

        expand_pass(&mut img, stride, &[0b11110000], &info(5, 0, width), bits_pp);
        assert_eq!(img, [0b10101010u8, 0, 0b10101010, 0, 0b10101010, 0, 0, 0]);

        expand_pass(&mut img, stride, &[0b11110000], &info(5, 1, width), bits_pp);
        assert_eq!(
            img,
            [0b10101010u8, 0, 0b10101010, 0, 0b10101010, 0, 0b10101010, 0]
        );

        expand_pass(&mut img, stride, &[0b11110000], &info(6, 0, width), bits_pp);
        assert_eq!(
            img,
            [0b11111111u8, 0, 0b10101010, 0, 0b10101010, 0, 0b10101010, 0]
        );

        expand_pass(&mut img, stride, &[0b11110000], &info(6, 1, width), bits_pp);
        assert_eq!(
            img,
            [0b11111111u8, 0, 0b11111111, 0, 0b10101010, 0, 0b10101010, 0]
        );

        expand_pass(&mut img, stride, &[0b11110000], &info(6, 2, width), bits_pp);
        assert_eq!(
            img,
            [0b11111111u8, 0, 0b11111111, 0, 0b11111111, 0, 0b10101010, 0]
        );

        expand_pass(&mut img, stride, &[0b11110000], &info(6, 3, width), bits_pp);
        assert_eq!(
            [0b11111111u8, 0, 0b11111111, 0, 0b11111111, 0, 0b11111111, 0],
            img
        );

        expand_pass(&mut img, stride, &[0b11111111], &info(7, 0, width), bits_pp);
        assert_eq!(
            [
                0b11111111u8,
                0b11111111,
                0b11111111,
                0,
                0b11111111,
                0,
                0b11111111,
                0
            ],
            img
        );

        expand_pass(&mut img, stride, &[0b11111111], &info(7, 1, width), bits_pp);
        assert_eq!(
            [
                0b11111111u8,
                0b11111111,
                0b11111111,
                0b11111111,
                0b11111111,
                0,
                0b11111111,
                0
            ],
            img
        );

        expand_pass(&mut img, stride, &[0b11111111], &info(7, 2, width), bits_pp);
        assert_eq!(
            [
                0b11111111u8,
                0b11111111,
                0b11111111,
                0b11111111,
                0b11111111,
                0b11111111,
                0b11111111,
                0
            ],
            img
        );

        expand_pass(&mut img, stride, &[0b11111111], &info(7, 3, width), bits_pp);
        assert_eq!(
            [
                0b11111111u8,
                0b11111111,
                0b11111111,
                0b11111111,
                0b11111111,
                0b11111111,
                0b11111111,
                0b11111111
            ],
            img
        );
    }

    // We use 4bpp as representative for bit-fiddling passes bpp 1, 2, 4. The choice was made
    // because it is succinct to write in hex so one can read this and understand it.
    #[test]
    fn test_expand_pass_splat_4bpp() {
        let width = 8;
        let bits_pp = 4;

        let mut img = [0u8; 8];
        let stride = width / 2;

        let passes: &[(&'static [u8], &'static [u8])] = &[
            (&[0x10], &[0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11]), // pass 1, 0
            (&[0x20], &[0x11, 0x11, 0x22, 0x22, 0x11, 0x11, 0x22, 0x22]), // pass 2, 0
            // no third pass..
            (&[0x4a], &[0x11, 0x44, 0x22, 0xaa, 0x11, 0x44, 0x22, 0xaa]), // pass 4, 0
            // no fifth pass..
            (
                &[0x6b, 0x6b],
                &[0x16, 0x4b, 0x26, 0xab, 0x16, 0x4b, 0x26, 0xab],
            ), // pass 6, 0
            (
                &[0x7c, 0xc7, 0x7c, 0x7c],
                &[0x16, 0x4b, 0x26, 0xab, 0x7c, 0xc7, 0x7c, 0x7c],
            ), // pass 7, 0
        ];

        let adam7 = Adam7Iterator::new(8, 2);
        for ((data, expected), adam7_info) in passes.iter().zip(adam7) {
            expand_pass_splat(&mut img, stride, data, &adam7_info, bits_pp);
            assert_eq!(img, *expected, "{img:x?} {expected:x?} {adam7_info:?}");
        }
    }

    /// Check that our different Adam7 strategies lead to the same result once all interlace lines
    /// have been applied.
    #[test]
    fn adam7_equivalence() {
        // Choose ragged sizes to cover bugs that write outside etc.
        const WIDTH: u32 = 8;
        const HEIGHT: u32 = 8;

        let interace_pool: Vec<_> = (0x42u8..).take(32).collect();

        for &bpp in &[1u8, 2, 4, 8, 16, 24, 32][2..] {
            let bytes_of = |pix: u32| (u32::from(bpp) * pix).next_multiple_of(8) as usize / 8;

            let rowbytes = bytes_of(WIDTH);

            // In the sparse case we do not promise to override all bits
            let mut buf_sparse = vec![0x00; rowbytes * HEIGHT as usize];
            // Whereas in the spat case we do, so we may as well set some arbitrary initial
            let mut buf_splat = vec![0xaa; rowbytes * HEIGHT as usize];

            // Now execute all the iterations, then compare buffers.
            for adam7_info in Adam7Iterator::new(WIDTH, HEIGHT) {
                let adam7_bytes = bytes_of(adam7_info.samples);
                let interlace_line = &interace_pool[..adam7_bytes];

                expand_pass(&mut buf_sparse, rowbytes, interlace_line, &adam7_info, bpp);
                expand_pass_splat(&mut buf_splat, rowbytes, interlace_line, &adam7_info, bpp);
            }

            assert_eq!(
                buf_sparse, buf_splat,
                "{buf_sparse:x?} {buf_splat:x?} bpp={bpp}"
            );
        }
    }

    #[test]
    fn test_expand_pass_splat_1bpp() {
        let width = 8;
        let bits_pp = 1;

        let mut img = [0u8; 8];
        let stride = 1;

        // Since bits do not suffice to represent the pass number in pixels we choose interlace
        // rows such that we toggle all affected bits each time. In particular the input bits that
        // must not be used are set to the inverse.
        let passes: &[(&'static [u8], &'static [u8])] = &[
            (&[0x80], &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]), // pass 1, 0
            (&[0x7f], &[0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0]), // pass 2, 0
            (&[0xc0], &[0xf0, 0xf0, 0xf0, 0xf0, 0xff, 0xff, 0xff, 0xff]), // pass 3, 0
            (&[0x3f], &[0xc0, 0xc0, 0xc0, 0xc0, 0xff, 0xff, 0xff, 0xff]), // pass 4, 0
            (&[0x3f], &[0xc0, 0xc0, 0xc0, 0xc0, 0xcc, 0xcc, 0xcc, 0xcc]), // pass 4, 1
            (&[0xf0], &[0xc0, 0xc0, 0xff, 0xff, 0xcc, 0xcc, 0xcc, 0xcc]), // pass 5, 0
            (&[0xf0], &[0xc0, 0xc0, 0xff, 0xff, 0xcc, 0xcc, 0xff, 0xff]), // pass 5, 1
            (&[0x0f], &[0x80, 0x80, 0xff, 0xff, 0xcc, 0xcc, 0xff, 0xff]), // pass 6, 0
            (&[0x0f], &[0x80, 0x80, 0xaa, 0xaa, 0xcc, 0xcc, 0xff, 0xff]), // pass 6, 1
            (&[0x0f], &[0x80, 0x80, 0xaa, 0xaa, 0x88, 0x88, 0xff, 0xff]), // pass 6, 2
            (&[0x0f], &[0x80, 0x80, 0xaa, 0xaa, 0x88, 0x88, 0xaa, 0xaa]), // pass 6, 3
            (&[0xff], &[0x80, 0xff, 0xaa, 0xaa, 0x88, 0x88, 0xaa, 0xaa]), // pass 7, 0
            (&[0xff], &[0x80, 0xff, 0xaa, 0xff, 0x88, 0x88, 0xaa, 0xaa]), // pass 7, 1
            (&[0xff], &[0x80, 0xff, 0xaa, 0xff, 0x88, 0xff, 0xaa, 0xaa]), // pass 7, 2
            (&[0xff], &[0x80, 0xff, 0xaa, 0xff, 0x88, 0xff, 0xaa, 0xff]), // pass 7, 3
        ];

        let adam7 = Adam7Iterator::new(width, 8);
        for ((data, expected), adam7_info) in passes.iter().zip(adam7) {
            expand_pass_splat(&mut img, stride, data, &adam7_info, bits_pp);
            assert_eq!(img, *expected, "{img:x?} {expected:x?} {adam7_info:?}");
        }
    }

    /// This test ensures that `expand_pass` works correctly on 32-bit machines, even when the indices
    /// of individual bits in the target buffer can not be expressed within a `usize`. We ensure that
    /// the output buffer size is between `usize::MAX / 8` and `isize::MAX` to trigger that condition.
    #[cfg(target_pointer_width = "32")]
    #[test]
    fn regression_overflow_adam7_bitfill() {
        fn multibyte_expand_pass_test_helper(width: usize, height: usize, bits_pp: u8) -> Vec<u8> {
            let bytes_pp = bits_pp / 8;
            let size = width * height * bytes_pp as usize;
            let mut img = vec![0u8; size];
            let img_row_stride = width * bytes_pp as usize;

            for it in Adam7Iterator::new(width as u32, height as u32).into_iter() {
                if it.pass != 7 {
                    continue;
                }

                if it.line != (width / 2) as u32 - 1 {
                    continue;
                }

                let interlace_size = it.width * (bytes_pp as u32);
                // Ensure that expanded pixels are never empty bits. This differentiates the written bits
                // from the initial bits that are all zeroed.
                let interlaced_row: Vec<_> = (0..interlace_size).map(|_| 0xff).collect();

                expand_pass(&mut img, img_row_stride, &interlaced_row, &it, bits_pp);
            }

            img
        }

        let expanded = multibyte_expand_pass_test_helper(1 << 14, 1 << 14, 32);
        assert_eq!(*expanded.last().unwrap(), 0xff);
    }

    #[cfg(test)]
    fn create_adam7_info_for_tests(pass: u8, line: u32, img_width: usize) -> Adam7Info {
        let width = {
            let img_height = 8;
            Adam7Iterator::new(img_width as u32, img_height)
                .filter(|info| info.pass == pass)
                .map(|info| info.samples)
                .next()
                .unwrap()
        };

        Adam7Info {
            pass,
            line,
            samples: width,
            width,
        }
    }
}
