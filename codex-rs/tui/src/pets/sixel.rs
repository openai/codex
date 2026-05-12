//! Minimal Sixel encoder for pet sprites.
//!
//! This is intentionally not a general-purpose Sixel implementation. Pet frames
//! are already small RGBA images by the time they reach this module, so the
//! encoder uses deterministic RGB332 color reduction and transparent pixels are
//! simply omitted from the emitted color planes.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;

const ST: &[u8] = b"\x1b\\";
const SIXEL_BAND_HEIGHT: u32 = 6;
const PALETTE_COLOR_COUNT: usize = 256;
const TRANSPARENT_ALPHA_THRESHOLD: u8 = 128;
const TRANSPARENT_BACKGROUND_DCS: &[u8] = b"\x1bP9;1;0q";

pub(crate) fn encode_rgba(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    if width == 0 || height == 0 {
        bail!("sixel image dimensions must be non-zero");
    }

    let expected_len = pixel_count(width, height)?
        .checked_mul(4)
        .context("sixel RGBA buffer length overflow")?;
    if rgba.len() != expected_len {
        bail!(
            "sixel RGBA buffer has {} bytes, expected {expected_len}",
            rgba.len()
        );
    }

    let palette = Palette::from_rgba(rgba);
    let mut output = Vec::new();
    output.extend_from_slice(TRANSPARENT_BACKGROUND_DCS);
    output.extend_from_slice(format!("\"1;1;{width};{height}").as_bytes());
    palette.write_definitions(&mut output);
    write_pixels(&mut output, rgba, width, height, &palette)?;
    output.extend_from_slice(ST);
    Ok(output)
}

fn write_pixels(
    output: &mut Vec<u8>,
    rgba: &[u8],
    width: u32,
    height: u32,
    palette: &Palette,
) -> Result<()> {
    let band_count = height.div_ceil(SIXEL_BAND_HEIGHT);
    for band_index in 0..band_count {
        let band_top = band_index * SIXEL_BAND_HEIGHT;
        let colors = active_colors_for_band(rgba, width, height, band_top, palette)?;
        for (position, color_index) in colors.iter().enumerate() {
            output.extend_from_slice(format!("#{color_index}").as_bytes());
            let mut run_char = None;
            let mut run_len = 0usize;
            for x in 0..width {
                let data = sixel_data_for_column(rgba, width, height, band_top, x, *color_index)?;
                push_run(&mut run_char, &mut run_len, output, data);
            }
            flush_run(&mut run_char, &mut run_len, output);
            if position + 1 < colors.len() {
                output.push(b'$');
            }
        }

        if band_index + 1 < band_count {
            if colors.is_empty() {
                output.push(b'-');
            } else {
                output.extend_from_slice(b"$-");
            }
        }
    }

    Ok(())
}

fn active_colors_for_band(
    rgba: &[u8],
    width: u32,
    height: u32,
    band_top: u32,
    palette: &Palette,
) -> Result<Vec<u8>> {
    let mut active = [false; PALETTE_COLOR_COUNT];
    for y in band_top..height.min(band_top + SIXEL_BAND_HEIGHT) {
        for x in 0..width {
            if let Some(color_index) = color_index_at(rgba, width, x, y)? {
                active[usize::from(color_index)] = true;
            }
        }
    }

    Ok(palette
        .indices()
        .filter(|color_index| active[usize::from(*color_index)])
        .collect())
}

fn sixel_data_for_column(
    rgba: &[u8],
    width: u32,
    height: u32,
    band_top: u32,
    x: u32,
    color_index: u8,
) -> Result<u8> {
    let mut mask = 0u8;
    for bit in 0..SIXEL_BAND_HEIGHT {
        let y = band_top + bit;
        if y >= height {
            continue;
        }

        if color_index_at(rgba, width, x, y)? == Some(color_index) {
            mask |= 1 << bit;
        }
    }

    Ok(b'?' + mask)
}

fn color_index_at(rgba: &[u8], width: u32, x: u32, y: u32) -> Result<Option<u8>> {
    let pixel_index = pixel_offset(width, x, y)?;
    let alpha = rgba[pixel_index + 3];
    if alpha < TRANSPARENT_ALPHA_THRESHOLD {
        return Ok(None);
    }

    Ok(Some(rgb332_index(
        rgba[pixel_index],
        rgba[pixel_index + 1],
        rgba[pixel_index + 2],
    )))
}

fn push_run(run_char: &mut Option<u8>, run_len: &mut usize, output: &mut Vec<u8>, byte: u8) {
    match *run_char {
        Some(current) if current == byte => {
            *run_len += 1;
        }
        _ => {
            flush_run(run_char, run_len, output);
            *run_char = Some(byte);
            *run_len = 1;
        }
    }
}

fn flush_run(run_char: &mut Option<u8>, run_len: &mut usize, output: &mut Vec<u8>) {
    let Some(byte) = run_char.take() else {
        return;
    };

    if *run_len > 3 {
        output.extend_from_slice(format!("!{}", *run_len).as_bytes());
        output.push(byte);
    } else {
        output.extend(std::iter::repeat_n(byte, *run_len));
    }
    *run_len = 0;
}

fn pixel_offset(width: u32, x: u32, y: u32) -> Result<usize> {
    let pixel_index = u64::from(y)
        .checked_mul(u64::from(width))
        .and_then(|row| row.checked_add(u64::from(x)))
        .context("sixel pixel index overflow")?;
    let byte_index = pixel_index
        .checked_mul(4)
        .context("sixel byte index overflow")?;
    usize::try_from(byte_index).context("sixel byte index does not fit usize")
}

fn pixel_count(width: u32, height: u32) -> Result<usize> {
    let count = u64::from(width)
        .checked_mul(u64::from(height))
        .context("sixel pixel count overflow")?;
    usize::try_from(count).context("sixel pixel count does not fit usize")
}

fn rgb332_index(red: u8, green: u8, blue: u8) -> u8 {
    let red = red >> 5;
    let green = green >> 5;
    let blue = blue >> 6;
    (red << 5) | (green << 2) | blue
}

fn rgb332_color(index: u8) -> (u8, u8, u8) {
    let red = index >> 5;
    let green = (index >> 2) & 0b111;
    let blue = index & 0b11;
    (
        scale_bucket_to_byte(red, /*max*/ 7),
        scale_bucket_to_byte(green, /*max*/ 7),
        scale_bucket_to_byte(blue, /*max*/ 3),
    )
}

fn scale_bucket_to_byte(bucket: u8, max: u8) -> u8 {
    let value = (u16::from(bucket) * 255) / u16::from(max);
    u8::try_from(value).unwrap_or(u8::MAX)
}

fn byte_to_sixel_percent(value: u8) -> u8 {
    let value = (u16::from(value) * 100) / 255;
    u8::try_from(value).unwrap_or(100)
}

struct Palette {
    used: [bool; PALETTE_COLOR_COUNT],
}

impl Palette {
    fn from_rgba(rgba: &[u8]) -> Self {
        let mut used = [false; PALETTE_COLOR_COUNT];
        for pixel in rgba.chunks_exact(4) {
            if pixel[3] < TRANSPARENT_ALPHA_THRESHOLD {
                continue;
            }

            used[usize::from(rgb332_index(pixel[0], pixel[1], pixel[2]))] = true;
        }

        Self { used }
    }

    fn indices(&self) -> impl Iterator<Item = u8> + '_ {
        (0..=u8::MAX).filter(|index| self.used[usize::from(*index)])
    }

    fn write_definitions(&self, output: &mut Vec<u8>) {
        for color_index in self.indices() {
            let (red, green, blue) = rgb332_color(color_index);
            output.extend_from_slice(
                format!(
                    "#{color_index};2;{};{};{}",
                    byte_to_sixel_percent(red),
                    byte_to_sixel_percent(green),
                    byte_to_sixel_percent(blue)
                )
                .as_bytes(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_TRANSPARENT_BACKGROUND_DCS: &str = "\x1bP9;1;0q";

    #[test]
    fn encodes_red_pixel_with_palette_and_pixel_data() {
        let sixel = encode_rgba(&[255, 0, 0, 255], /*width*/ 1, /*height*/ 1).unwrap();
        let sixel = String::from_utf8(sixel).unwrap();

        assert_eq!(
            sixel,
            format!("{EXPECTED_TRANSPARENT_BACKGROUND_DCS}\"1;1;1;1#224;2;100;0;0#224@\x1b\\")
        );
    }

    #[test]
    fn transparent_pixels_do_not_emit_palette_or_pixel_data() {
        let sixel = encode_rgba(&[255, 0, 0, 0], /*width*/ 1, /*height*/ 1).unwrap();
        let sixel = String::from_utf8(sixel).unwrap();

        assert_eq!(
            sixel,
            format!("{EXPECTED_TRANSPARENT_BACKGROUND_DCS}\"1;1;1;1\x1b\\")
        );
    }

    #[test]
    fn multi_band_images_advance_to_next_sixel_band() {
        let mut rgba = Vec::new();
        for _ in 0..7 {
            rgba.extend_from_slice(&[255, 0, 0, 255]);
        }

        let sixel = encode_rgba(&rgba, /*width*/ 1, /*height*/ 7).unwrap();
        let sixel = String::from_utf8(sixel).unwrap();

        assert_eq!(
            sixel,
            format!(
                "{EXPECTED_TRANSPARENT_BACKGROUND_DCS}\"1;1;1;7#224;2;100;0;0#224~$-#224@\x1b\\"
            )
        );
    }

    #[test]
    fn repeated_cells_use_sixel_run_length_encoding() {
        let mut rgba = Vec::new();
        for _ in 0..4 {
            rgba.extend_from_slice(&[255, 0, 0, 255]);
        }

        let sixel = encode_rgba(&rgba, /*width*/ 4, /*height*/ 1).unwrap();
        let sixel = String::from_utf8(sixel).unwrap();

        assert!(sixel.contains("#224!4@"));
    }

    #[test]
    fn rejects_mismatched_rgba_buffer_length() {
        let err = encode_rgba(&[255, 0, 0], /*width*/ 1, /*height*/ 1).unwrap_err();

        assert_eq!(err.to_string(), "sixel RGBA buffer has 3 bytes, expected 4");
    }
}
