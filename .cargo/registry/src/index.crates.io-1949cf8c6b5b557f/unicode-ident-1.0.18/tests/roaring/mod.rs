use roaring::RoaringBitmap;

pub fn xid_start_bitmap() -> RoaringBitmap {
    let mut bitmap = RoaringBitmap::new();
    for ch in '\0'..=char::MAX {
        if unicode_ident::is_xid_start(ch) {
            bitmap.insert(ch as u32);
        }
    }
    bitmap
}

pub fn xid_continue_bitmap() -> RoaringBitmap {
    let mut bitmap = RoaringBitmap::new();
    for ch in '\0'..=char::MAX {
        if unicode_ident::is_xid_continue(ch) {
            bitmap.insert(ch as u32);
        }
    }
    bitmap
}
