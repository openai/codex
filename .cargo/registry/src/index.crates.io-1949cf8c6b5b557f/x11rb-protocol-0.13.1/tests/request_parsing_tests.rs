#![cfg(feature = "request-parsing")]
#![cfg(feature = "extra-traits")]

use std::borrow::Cow;

use x11rb_protocol::errors::ParseError;
use x11rb_protocol::x11_utils::RequestHeader;

macro_rules! add_ne {
    ($data:expr, $add:expr) => {
        $data.extend(&$add.to_ne_bytes())
    };
}

#[test]
fn test_bad_request_header_opcode() {
    use x11rb_protocol::protocol::xproto::GetInputFocusRequest;
    let header = RequestHeader {
        major_opcode: 1,
        minor_opcode: 0,
        remaining_length: 0,
    };
    let body = &[];
    assert_eq!(
        GetInputFocusRequest::try_parse_request(header, body),
        Err(ParseError::InvalidValue)
    );
}

#[test]
fn test_bad_request_header_length() {
    use x11rb_protocol::protocol::xproto::CreateWindowRequest;
    let header = RequestHeader {
        major_opcode: 1,
        minor_opcode: 0,
        remaining_length: 42,
    };
    let body = &[];
    assert_eq!(
        CreateWindowRequest::try_parse_request(header, body),
        Err(ParseError::InsufficientData)
    );
}

#[test]
fn test_create_window1() {
    use x11rb_protocol::protocol::xproto::{
        CreateWindowAux, CreateWindowRequest, Gravity, WindowClass,
    };
    let header = RequestHeader {
        major_opcode: 1,
        minor_opcode: 0x18,
        remaining_length: 10,
    };
    let mut body = vec![];
    add_ne!(body, 0x05c0_0001u32);
    add_ne!(body, 0x0000_0513u32);
    add_ne!(body, 0x047bu16);
    add_ne!(body, 0x0134u16);
    add_ne!(body, 0x03f5u16);
    add_ne!(body, 0x033bu16);
    add_ne!(body, 0x0000u16);
    add_ne!(body, 0x0001u16);
    add_ne!(body, 0x0000_0047u32);
    add_ne!(body, 0x0000_001au32);
    add_ne!(body, 0xfff2_f1f0u32);
    add_ne!(body, 0x0000_0000u32);
    add_ne!(body, 0x0000_0001u32);
    let r = CreateWindowRequest::try_parse_request(header, &body).unwrap();
    assert_eq!(
        r,
        CreateWindowRequest {
            depth: 0x18,
            wid: 0x05c0_0001,
            parent: 0x0000_0513,
            x: 0x047b,
            y: 0x0134,
            width: 0x03f5,
            height: 0x033b,
            border_width: 0,
            class: WindowClass::INPUT_OUTPUT,
            visual: 0x47,
            value_list: Cow::Owned(CreateWindowAux {
                background_pixel: Some(0xfff2_f1f0),
                border_pixel: Some(0),
                bit_gravity: Some(Gravity::NORTH_WEST),
                ..Default::default()
            }),
        }
    );
}

#[test]
fn test_create_window2() {
    use x11rb_protocol::protocol::xproto::{
        CreateWindowAux, CreateWindowRequest, Gravity, WindowClass,
    };
    let header = RequestHeader {
        major_opcode: 1,
        minor_opcode: 0x18,
        remaining_length: 11,
    };
    let mut body = vec![];
    add_ne!(body, 0x0540_0003u32);
    add_ne!(body, 0x0000_0513u32);
    add_ne!(body, 0x0000_0000u32);
    add_ne!(body, 0x0001u16);
    add_ne!(body, 0x0001u16);
    add_ne!(body, 0x0000u16);
    add_ne!(body, 0x0001u16);
    add_ne!(body, 0x0000_035au32);
    add_ne!(body, 0x0000_201au32);
    add_ne!(body, 0x0000_0000u32);
    add_ne!(body, 0x0000_0000u32);
    add_ne!(body, 0x0000_0001u32);
    add_ne!(body, 0x0540_0002u32);
    let r = CreateWindowRequest::try_parse_request(header, &body).unwrap();
    assert_eq!(
        r,
        CreateWindowRequest {
            depth: 0x18,
            wid: 0x0540_0003,
            parent: 0x0000_0513,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            border_width: 0,
            class: WindowClass::INPUT_OUTPUT,
            visual: 0x35a,
            value_list: Cow::Owned(CreateWindowAux {
                background_pixel: Some(0),
                border_pixel: Some(0),
                bit_gravity: Some(Gravity::NORTH_WEST),
                colormap: Some(0x0540_0002),
                ..Default::default()
            }),
        }
    );
}

#[test]
fn test_change_window_attributes() {
    use x11rb_protocol::protocol::xproto::{
        ChangeWindowAttributesAux, ChangeWindowAttributesRequest, EventMask,
    };
    let header = RequestHeader {
        major_opcode: 2,
        minor_opcode: 0,
        remaining_length: 3,
    };
    let mut body = vec![];
    add_ne!(body, 0x0000_0513u32);
    add_ne!(body, 0x0000_0800u32);
    add_ne!(body, 0x0040_0000u32);
    let r = ChangeWindowAttributesRequest::try_parse_request(header, &body).unwrap();
    assert_eq!(
        r,
        ChangeWindowAttributesRequest {
            window: 0x0513,
            value_list: Cow::Owned(ChangeWindowAttributesAux {
                event_mask: Some(EventMask::PROPERTY_CHANGE),
                ..Default::default()
            }),
        }
    );
}

#[test]
fn test_get_window_attributes() {
    use x11rb_protocol::protocol::xproto::GetWindowAttributesRequest;
    let header = RequestHeader {
        major_opcode: 3,
        minor_opcode: 0,
        remaining_length: 1,
    };
    let mut body = vec![];
    add_ne!(body, 0x00e0_0002u32);
    let r = GetWindowAttributesRequest::try_parse_request(header, &body).unwrap();
    assert_eq!(
        r,
        GetWindowAttributesRequest {
            window: 0x00e0_0002
        }
    );
}

#[test]
fn test_get_input_focus() {
    use x11rb_protocol::protocol::xproto::GetInputFocusRequest;
    let header = RequestHeader {
        major_opcode: 43,
        minor_opcode: 0,
        remaining_length: 0,
    };
    let body = &[];
    let r = GetInputFocusRequest::try_parse_request(header, body).unwrap();
    assert_eq!(r, GetInputFocusRequest,);
}

#[test]
fn test_query_text_extents() {
    use x11rb_protocol::protocol::xproto::{Char2b, QueryTextExtentsRequest};
    let header = RequestHeader {
        major_opcode: 48,
        minor_opcode: 0,
        remaining_length: 2,
    };
    let mut body = vec![];
    add_ne!(body, 0x1234_5678u32);
    body.extend([0xbc, 0x9a, 0xf0, 0xde]);
    let r = QueryTextExtentsRequest::try_parse_request(header, &body).unwrap();
    assert_eq!(
        r,
        QueryTextExtentsRequest {
            font: 0x1234_5678,
            string: Cow::Owned(vec![
                Char2b {
                    byte1: 0xbc,
                    byte2: 0x9a,
                },
                Char2b {
                    byte1: 0xf0,
                    byte2: 0xde,
                },
            ]),
        }
    );
}

#[test]
fn test_query_text_extents_odd_length() {
    use x11rb_protocol::protocol::xproto::{Char2b, QueryTextExtentsRequest};
    let header = RequestHeader {
        major_opcode: 48,
        minor_opcode: 1,
        remaining_length: 2,
    };
    let mut body = vec![];
    add_ne!(body, 0x1234_5678u32);
    body.extend([0xbc, 0x9a, 0xf0, 0xde]);
    let r = QueryTextExtentsRequest::try_parse_request(header, &body).unwrap();
    assert_eq!(
        r,
        QueryTextExtentsRequest {
            font: 0x1234_5678,
            string: Cow::Owned(vec![Char2b {
                byte1: 0xbc,
                byte2: 0x9a,
            },]),
        }
    );
}

#[cfg(feature = "randr")]
#[test]
fn test_randr_get_output_property() {
    use x11rb_protocol::protocol::randr::GetOutputPropertyRequest;
    let header = RequestHeader {
        major_opcode: 140,
        minor_opcode: 15,
        remaining_length: 6,
    };
    let mut body = vec![];
    add_ne!(body, 0x0000_008au32);
    add_ne!(body, 0x0000_0045u32);
    add_ne!(body, 0x0000_0000u32);
    add_ne!(body, 0x0000_0000u32);
    add_ne!(body, 0x0000_0080u32);
    add_ne!(body, 0x0000_0000u32);
    let r = GetOutputPropertyRequest::try_parse_request(header, &body).unwrap();
    assert_eq!(
        r,
        GetOutputPropertyRequest {
            output: 0x8a,
            property: 0x45,
            type_: 0,
            long_offset: 0,
            long_length: 128,
            delete: false,
            pending: false,
        },
    );
}
