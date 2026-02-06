//! Clipboard paste support for reading images and text from the system clipboard.
//!
//! Uses `arboard` to access native clipboard on macOS, Linux (X11/Wayland), and Windows.
//! Falls back gracefully when clipboard is unavailable (e.g., headless environments, Android).
//!
//! ## Supported image formats
//!
//! - **File copies** (Finder/Explorer): preserves original format (JPEG, PNG, GIF, WebP)
//! - **Raw clipboard data** (screenshots, browser copies): encoded as PNG

use std::path::Path;

/// Supported MIME types for images across all providers.
const SUPPORTED_IMAGE_EXTENSIONS: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
];

/// Error type for clipboard paste operations.
#[derive(Debug, Clone)]
pub enum ClipboardImageError {
    /// Clipboard could not be opened.
    ClipboardUnavailable(String),
    /// No image data found on clipboard.
    NoImage(String),
    /// Failed to encode image.
    EncodeFailed(String),
}

impl std::fmt::Display for ClipboardImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardImageError::ClipboardUnavailable(msg) => {
                write!(f, "clipboard unavailable: {msg}")
            }
            ClipboardImageError::NoImage(msg) => write!(f, "no image on clipboard: {msg}"),
            ClipboardImageError::EncodeFailed(msg) => {
                write!(f, "could not encode image: {msg}")
            }
        }
    }
}

impl std::error::Error for ClipboardImageError {}

/// Detect MIME type from file extension.
fn media_type_from_path(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    SUPPORTED_IMAGE_EXTENSIONS
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, mime)| *mime)
}

/// Try to read image data from the system clipboard.
///
/// Returns `(image_bytes, media_type)` on success.
///
/// Detection order:
/// 1. File list — preserves original format (JPEG stays JPEG, etc.)
/// 2. Raw image data — encoded as PNG (clipboard provides raw RGBA pixels)
#[cfg(not(target_os = "android"))]
pub fn paste_image(mut cb: arboard::Clipboard) -> Result<(Vec<u8>, String), ClipboardImageError> {
    // 1. Try file_list first (Finder/Explorer copies — preserve original format)
    if let Ok(files) = cb.get().file_list() {
        for file in files {
            let path = Path::new(&file);
            if let Some(media_type) = media_type_from_path(path) {
                if let Ok(data) = std::fs::read(path) {
                    tracing::debug!(
                        media_type,
                        bytes = data.len(),
                        "clipboard image from file"
                    );
                    return Ok((data, media_type.to_string()));
                }
            }
        }
    }

    // 2. Fall back to raw image data (screenshots, browser copies → encode as PNG)
    let img = cb
        .get_image()
        .map_err(|e| ClipboardImageError::NoImage(e.to_string()))?;
    let w = img.width as u32;
    let h = img.height as u32;
    tracing::debug!("clipboard image from data: {w}x{h}");

    let Some(rgba_img) = image::RgbaImage::from_raw(w, h, img.bytes.into_owned()) else {
        return Err(ClipboardImageError::EncodeFailed(
            "invalid RGBA buffer".into(),
        ));
    };

    let dyn_img = image::DynamicImage::ImageRgba8(rgba_img);
    let mut png: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png);
    dyn_img
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| ClipboardImageError::EncodeFailed(e.to_string()))?;

    Ok((png, "image/png".to_string()))
}

/// Try to read text from the system clipboard.
#[cfg(not(target_os = "android"))]
pub fn paste_text(mut cb: arboard::Clipboard) -> Result<String, ClipboardImageError> {
    cb.get_text()
        .map_err(|e| ClipboardImageError::NoImage(e.to_string()))
}

/// Open the system clipboard once, reusable for both image and text attempts.
#[cfg(not(target_os = "android"))]
pub fn open_clipboard() -> Result<arboard::Clipboard, ClipboardImageError> {
    arboard::Clipboard::new().map_err(|e| ClipboardImageError::ClipboardUnavailable(e.to_string()))
}

/// Android: clipboard is not supported.
#[cfg(target_os = "android")]
pub fn paste_image(
    _cb: arboard::Clipboard,
) -> Result<(Vec<u8>, String), ClipboardImageError> {
    Err(ClipboardImageError::ClipboardUnavailable(
        "clipboard image paste is unsupported on Android".into(),
    ))
}

/// Android: clipboard is not supported.
#[cfg(target_os = "android")]
pub fn paste_text(_cb: arboard::Clipboard) -> Result<String, ClipboardImageError> {
    Err(ClipboardImageError::ClipboardUnavailable(
        "clipboard text paste is unsupported on Android".into(),
    ))
}

/// Android: clipboard is not supported.
#[cfg(target_os = "android")]
pub fn open_clipboard() -> Result<arboard::Clipboard, ClipboardImageError> {
    Err(ClipboardImageError::ClipboardUnavailable(
        "clipboard is unsupported on Android".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_type_from_path() {
        assert_eq!(
            media_type_from_path(Path::new("/tmp/photo.jpg")),
            Some("image/jpeg")
        );
        assert_eq!(
            media_type_from_path(Path::new("/tmp/photo.JPEG")),
            Some("image/jpeg")
        );
        assert_eq!(
            media_type_from_path(Path::new("/tmp/screenshot.png")),
            Some("image/png")
        );
        assert_eq!(
            media_type_from_path(Path::new("/tmp/anim.gif")),
            Some("image/gif")
        );
        assert_eq!(
            media_type_from_path(Path::new("/tmp/modern.webp")),
            Some("image/webp")
        );
        // Unsupported extensions
        assert_eq!(media_type_from_path(Path::new("/tmp/doc.pdf")), None);
        assert_eq!(media_type_from_path(Path::new("/tmp/noext")), None);
    }
}
