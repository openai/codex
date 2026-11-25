//! Supply the pixel offset missing from packed Windows clipboard bitmaps.

use super::PasteImageError;
use clipboard_win::formats::CF_DIBV5;
use image::DynamicImage;
use image::ImageFormat;
use image::ImageReader;
use std::io::Cursor;

const BMP_HEADER: usize = 14;
const V5_HEADER: usize = 124;

pub(super) fn clipboard_image() -> Option<Result<DynamicImage, PasteImageError>> {
    let mut bmp = vec![0; BMP_HEADER];
    {
        let _clipboard = clipboard_win::Clipboard::new_attempts(/*num*/ 5).ok()?;
        // Keep arboard's preference for the registered PNG format.
        if clipboard_win::register_format("PNG")
            .is_some_and(|id| clipboard_win::is_format_avail(id.get()))
            || !clipboard_win::is_format_avail(CF_DIBV5)
        {
            return None;
        }
        clipboard_win::raw::get_vec(CF_DIBV5, &mut bmp).ok()?;
    }
    decode_bitmap(bmp, image::Limits::default())
}

fn decode_bitmap(
    mut bmp: Vec<u8>,
    limits: image::Limits,
) -> Option<Result<DynamicImage, PasteImageError>> {
    let dib = bmp.get(BMP_HEADER..)?;
    let field = |offset: usize| {
        Some(u32::from_le_bytes(
            dib.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    // Only handle 32-bit BI_BITFIELDS without a color table or embedded profile.
    if field(0)? != V5_HEADER as u32
        || dib.get(14..16)? != 32u16.to_le_bytes()
        || field(16)? != 3 // bV5Compression
        || field(32)? != 0 // bV5ClrUsed
        // bV5ProfileData and bV5ProfileSize
        || field(112)? != 0
        || field(116)? != 0
    {
        return None;
    }
    let width = field(4)? as usize;
    let height = (field(8)? as i32).unsigned_abs() as usize;
    let pixels = width.checked_mul(height)?.checked_mul(4)?;
    let data = dib.get(V5_HEADER..)?;
    let Some(spare) = data.len().checked_sub(pixels) else {
        // arboard can allocate the declared pixel buffer before noticing a short read.
        return Some(Err(PasteImageError::ImageReadFailed(
            "incomplete bitmap pixel data".into(),
        )));
    };

    // image's headerless decoder skips three extra masks even when the pixels
    // immediately follow the V5 header (arboard#234). Trailing allocation
    // padding can hide the short read. If the first three pixels match the
    // masks and there's enough padding, the layouts are indistinguishable.
    // Prefer repeated masks to preserve compatibility with those producers.
    let masks = &dib[40..52];
    let mut offset = BMP_HEADER + V5_HEADER;
    if spare >= masks.len() && data.starts_with(masks) {
        offset += masks.len();
    }
    let Ok(size) = u32::try_from(bmp.len()) else {
        return Some(Err(PasteImageError::ImageReadFailed(
            "bitmap exceeds the BMP file size limit".into(),
        )));
    };
    bmp[..2].copy_from_slice(b"BM");
    bmp[2..6].copy_from_slice(&size.to_le_bytes());
    bmp[10..14].copy_from_slice(&(offset as u32).to_le_bytes());
    let mut reader = ImageReader::with_format(Cursor::new(bmp), ImageFormat::Bmp);
    reader.limits(limits);
    match reader.decode() {
        Ok(image) => Some(Ok(image)),
        // Retrying through arboard would bypass the decoded allocation limit.
        Err(err @ image::ImageError::Limits(_)) => {
            Some(Err(PasteImageError::ImageReadFailed(err.to_string())))
        }
        Err(_) => None,
    }
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
