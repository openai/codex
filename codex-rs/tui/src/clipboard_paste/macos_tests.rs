#![cfg(test)]

use super::*;
use objc2::rc::Retained;
use objc2_foundation::NSData;
use objc2_foundation::NSString;
use pretty_assertions::assert_eq;

fn item(ty: &NSString, bytes: &[u8]) -> Retained<NSPasteboardItem> {
    let item = NSPasteboardItem::new();
    assert!(item.setData_forType(&NSData::with_bytes(bytes), ty));
    item
}

fn encoded(format: ImageFormat, width: u32) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::new_rgb8(width, /*height*/ 1)
        .write_to(&mut bytes, format)
        .unwrap();
    bytes.into_inner()
}

#[test]
fn tiff_item_precedes_later_png() {
    let tiff = item(ns_string!("public.tiff"), b"owned by arboard");
    let png = item(
        ns_string!("public.png"),
        &encoded(ImageFormat::Png, /*width*/ 2),
    );
    let items = NSArray::from_retained_slice(&[tiff, png]);
    assert_eq!(first_image(&items), None);
}

#[test]
fn jpeg_item_precedes_later_png() {
    let text = item(ns_string!("public.utf8-plain-text"), b"text");
    let jpeg = item(
        ns_string!("public.jpeg"),
        &encoded(ImageFormat::Jpeg, /*width*/ 3),
    );
    let png = item(
        ns_string!("public.png"),
        &encoded(ImageFormat::Png, /*width*/ 2),
    );
    let items = NSArray::from_retained_slice(&[text, jpeg, png]);
    assert_eq!(
        first_image(&items),
        Some(DynamicImage::new_rgb8(/*width*/ 3, /*height*/ 1))
    );
}
