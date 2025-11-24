#![cfg(feature = "extra-traits")]

use x11rb_protocol::errors::ParseError;
use x11rb_protocol::protocol::xproto::{Setup, VisualClass};
use x11rb_protocol::x11_utils::TryParse;

fn get_setup_data() -> Vec<u8> {
    let mut s = Vec::new();

    let vendor_len: u16 = 2;
    let num_pixmap_formats: u8 = 1;
    let roots_len: u8 = 18;
    let header: u16 = 10;
    let length: u16 =
        header + vendor_len + 2 * u16::from(num_pixmap_formats) + u16::from(roots_len);

    s.extend([1, 0]); // Status "success" and padding
    s.extend(11u16.to_ne_bytes()); // major version
    s.extend(0u16.to_ne_bytes()); // minor version
    s.extend(length.to_ne_bytes()); // length
    s.extend(0x1234_5678u32.to_ne_bytes()); // release number
    s.extend(0x1000_0000u32.to_ne_bytes()); // resource id base
    s.extend(0x0000_00ffu32.to_ne_bytes()); // resource id mask
    s.extend(0u32.to_ne_bytes()); // motion buffer size
    s.extend(6u16.to_ne_bytes()); // vendor length
    s.extend(0x100u16.to_ne_bytes()); // maximum request length
    s.push(1); // roots length
    s.push(num_pixmap_formats); // pixmap formats length
    s.push(1); // image byte order: MSB first
    s.push(1); // bitmap format bit order: MSB first
    s.push(0); // scanline unit
    s.push(0); // scanline pad
    s.push(0); // min keycode
    s.push(0xff); // max keycode
    s.extend([0, 0, 0, 0]); // padding
    assert_eq!(s.len(), usize::from(header) * 4);

    s.extend("Vendor  ".bytes()); // vendor + padding
    assert_eq!(s.len(), usize::from(header + vendor_len) * 4);

    // Pixmap formats, we said above there is one entry
    s.push(15); // depth
    s.push(42); // bits per pixel
    s.push(21); // scanline pad
    s.extend([0, 0, 0, 0, 0]); // padding
    assert_eq!(
        s.len(),
        4 * usize::from(header + vendor_len + 2 * u16::from(num_pixmap_formats))
    );

    // Screens, we said above there is one entry
    s.extend(1u32.to_ne_bytes()); // root window
    s.extend(2u32.to_ne_bytes()); // default colormap
    s.extend(3u32.to_ne_bytes()); // white pixel
    s.extend(4u32.to_ne_bytes()); // black pixel
    s.extend(0u32.to_ne_bytes()); // current input masks
    s.extend(0u16.to_ne_bytes()); // width in pixels
    s.extend(0u16.to_ne_bytes()); // height in pixels
    s.extend(0u16.to_ne_bytes()); // width in mm
    s.extend(0u16.to_ne_bytes()); // height in mm
    s.extend(0u16.to_ne_bytes()); // min installed maps
    s.extend(0u16.to_ne_bytes()); // max installed maps
    s.extend(0u32.to_ne_bytes()); // root visual
    s.extend([0, 0, 0, 1]); // backing stores, save unders, root depths, allowed depths len

    // one depth entry
    s.extend([99, 0]); // depth and padding
    s.extend(1u16.to_ne_bytes()); // width visuals len
    s.extend([0, 0, 0, 0]); // padding

    // one visualtype entry
    s.extend(80u32.to_ne_bytes()); // visualid
    s.extend([2, 4]); // class and bits per rgb value
    s.extend(81u16.to_ne_bytes()); // colormap entries
    s.extend(82u32.to_ne_bytes()); // red mask
    s.extend(83u32.to_ne_bytes()); // green mask
    s.extend(84u32.to_ne_bytes()); // blue mask
    s.extend([0, 0, 0, 0]); // padding

    assert_eq!(s.len(), usize::from(length) * 4);

    s
}

#[test]
fn parse_setup() -> Result<(), ParseError> {
    let setup = get_setup_data();
    let (setup, remaining) = Setup::try_parse(&setup)?;

    assert_eq!(remaining.len(), 0);

    assert_eq!(
        (1, 11, 0),
        (
            setup.status,
            setup.protocol_major_version,
            setup.protocol_minor_version
        )
    );
    assert_eq!(0x1234_5678, setup.release_number);
    assert_eq!((0, 0xff), (setup.min_keycode, setup.max_keycode));
    assert_eq!(b"Vendor", &setup.vendor[..]);

    assert_eq!(1, setup.pixmap_formats.len());
    let format = &setup.pixmap_formats[0];
    assert_eq!(15, format.depth);
    assert_eq!(42, format.bits_per_pixel);
    assert_eq!(21, format.scanline_pad);

    assert_eq!(1, setup.roots.len());
    let root = &setup.roots[0];
    assert_eq!(
        (1, 2, 3, 4),
        (
            root.root,
            root.default_colormap,
            root.white_pixel,
            root.black_pixel
        )
    );

    assert_eq!(1, root.allowed_depths.len());
    let depth = &root.allowed_depths[0];
    assert_eq!(99, depth.depth);

    assert_eq!(1, depth.visuals.len());
    let visual = &depth.visuals[0];
    assert_eq!(80, visual.visual_id);
    assert_eq!(VisualClass::STATIC_COLOR, visual.class);
    assert_eq!(4, visual.bits_per_rgb_value);
    assert_eq!(81, visual.colormap_entries);
    assert_eq!(82, visual.red_mask);
    assert_eq!(83, visual.green_mask);
    assert_eq!(84, visual.blue_mask);

    Ok(())
}

#[cfg(feature = "xinput")]
#[test]
fn parse_xi_get_property_reply_format_0() {
    let mut s = vec![
        1, // response_type
        0, // pad
    ];
    s.extend(0u16.to_ne_bytes()); // sequence
    s.extend(0u32.to_ne_bytes()); // length
    s.extend(0u32.to_ne_bytes()); // type
    s.extend(0u32.to_ne_bytes()); // bytes_after
    s.extend(0u32.to_ne_bytes()); // num_items
    s.push(0); // format
    s.extend([0; 11]); // pad

    use x11rb_protocol::protocol::xinput::{XIGetPropertyItems, XIGetPropertyReply};
    let empty: &[u8] = &[];
    assert_eq!(
        XIGetPropertyReply::try_parse(&s),
        Ok((
            XIGetPropertyReply {
                sequence: 0,
                length: 0,
                type_: 0,
                bytes_after: 0,
                num_items: 0,
                items: XIGetPropertyItems::InvalidValue(0),
            },
            empty,
        )),
    );
}
