//! Read native image representations in macOS pasteboard item order.

use crate::image_decode::decode_oriented;
use image::DynamicImage;
use image::ImageFormat;
use image::ImageReader;
use image::Limits;
use objc2_app_kit::NSPasteboard;
use objc2_app_kit::NSPasteboardItem;
use objc2_foundation::NSArray;
use objc2_foundation::NSString;
use objc2_foundation::ns_string;
use std::io::Cursor;

/// Decode an offered PNG or JPEG before falling back to [arboard's macOS image reader].
/// That reader requests TIFF even when the pasteboard already holds the original PNG or
/// JPEG. Clipboard owners such as Vicinae then generate a large TIFF just for this paste.
///
/// [arboard's macOS image reader]: https://github.com/1Password/arboard/blob/a3750c79a5d63f3987317e03c412b7d9dffdc2af/src/platform/osx.rs#L217-L231
pub(super) fn clipboard_image() -> Option<DynamicImage> {
    objc2::rc::autoreleasepool(|_| {
        let items = NSPasteboard::generalPasteboard().pasteboardItems()?;
        first_image(&items)
    })
}

fn first_image(items: &NSArray<NSPasteboardItem>) -> Option<DynamicImage> {
    let png = ns_string!("public.png");
    for item in items {
        let types = item.types();
        // Prefer lossless PNG, even when it isn't the first representation advertised.
        if types.containsObject(png)
            && let Some(image) = decode_representation(&item, png, ImageFormat::Png)
        {
            return Some(image);
        }

        // For JPEG and TIFF, respect the owner's order. arboard reads the first TIFF.
        for ty in types {
            if *ty == *ns_string!("public.tiff") {
                return None;
            }
            if *ty == *ns_string!("public.jpeg")
                && let Some(image) = decode_representation(&item, &ty, ImageFormat::Jpeg)
            {
                return Some(image);
            }
        }
    }
    None
}

fn decode_representation(
    item: &NSPasteboardItem,
    ty: &NSString,
    format: ImageFormat,
) -> Option<DynamicImage> {
    let bytes = item.dataForType(ty)?.to_vec();
    let reader = ImageReader::with_format(Cursor::new(bytes), format);
    decode_oriented(reader, Limits::default()).ok()
}

#[cfg(test)]
#[path = "macos_tests.rs"]
mod tests;
