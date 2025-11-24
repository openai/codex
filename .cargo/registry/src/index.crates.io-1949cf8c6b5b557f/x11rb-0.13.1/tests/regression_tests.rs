use std::cell::RefCell;
use std::io::IoSlice;
use std::ops::Deref;

use x11rb::connection::{
    compute_length_field, BufWithFds, ReplyOrError, RequestConnection, RequestKind,
};
use x11rb::cookie::{Cookie, CookieWithFds, VoidCookie};
use x11rb::errors::{ConnectionError, ParseError, ReplyError};
use x11rb::protocol::xproto::{
    ClientMessageData, ConnectionExt, KeymapNotifyEvent, Segment, SetupAuthenticate,
};
use x11rb::utils::RawFdContainer;
use x11rb::x11_utils::{ExtensionInformation, Serialize, TryParse, TryParseFd};

use x11rb_protocol::{DiscardMode, SequenceNumber};

#[derive(Debug)]
struct SavedRequest {
    has_reply: bool,
    data: Vec<u8>,
}

impl SavedRequest {
    fn new(has_reply: bool, data: &[IoSlice]) -> SavedRequest {
        let data = data
            .iter()
            .flat_map(|slice| slice.deref())
            .copied()
            .collect::<Vec<_>>();
        SavedRequest { has_reply, data }
    }
}

#[derive(Debug, Default)]
struct FakeConnection(RefCell<Vec<SavedRequest>>);

impl FakeConnection {
    fn check_requests(&self, expected: &[(bool, Vec<u8>)]) {
        let vec = self.0.borrow();
        for (expected, actual) in expected.iter().zip(vec.iter()) {
            assert_eq!(expected.0, actual.has_reply);
            assert_eq!(actual.data, expected.1);
        }
        assert_eq!(expected.len(), vec.len());
    }

    fn internal_send_request(
        &self,
        bufs: &[IoSlice],
        fds: Vec<RawFdContainer>,
    ) -> Result<SequenceNumber, ConnectionError> {
        assert_eq!(fds.len(), 0);

        let mut storage = Default::default();
        let bufs = compute_length_field(self, bufs, &mut storage)?;

        self.0.borrow_mut().push(SavedRequest::new(false, bufs));
        Ok(0)
    }
}

impl RequestConnection for FakeConnection {
    type Buf = Vec<u8>;

    fn send_request_with_reply<R>(
        &self,
        bufs: &[IoSlice],
        fds: Vec<RawFdContainer>,
    ) -> Result<Cookie<Self, R>, ConnectionError>
    where
        R: TryParse,
    {
        Ok(Cookie::new(self, self.internal_send_request(bufs, fds)?))
    }

    fn send_request_with_reply_with_fds<R>(
        &self,
        _bufs: &[IoSlice],
        _fds: Vec<RawFdContainer>,
    ) -> Result<CookieWithFds<Self, R>, ConnectionError>
    where
        R: TryParseFd,
    {
        unimplemented!()
    }

    fn send_request_without_reply(
        &self,
        bufs: &[IoSlice],
        fds: Vec<RawFdContainer>,
    ) -> Result<VoidCookie<Self>, ConnectionError> {
        Ok(VoidCookie::new(
            self,
            self.internal_send_request(bufs, fds)?,
        ))
    }

    fn discard_reply(&self, _sequence: SequenceNumber, _kind: RequestKind, _mode: DiscardMode) {
        // Just ignore this
    }

    fn prefetch_extension_information(
        &self,
        _extension_name: &'static str,
    ) -> Result<(), ConnectionError> {
        unimplemented!();
    }

    fn extension_information(
        &self,
        _extension_name: &'static str,
    ) -> Result<Option<ExtensionInformation>, ConnectionError> {
        unimplemented!()
    }

    fn wait_for_reply_or_raw_error(
        &self,
        _sequence: SequenceNumber,
    ) -> Result<ReplyOrError<Vec<u8>>, ConnectionError> {
        unimplemented!()
    }

    fn wait_for_reply(
        &self,
        _sequence: SequenceNumber,
    ) -> Result<Option<Vec<u8>>, ConnectionError> {
        unimplemented!()
    }

    fn wait_for_reply_with_fds_raw(
        &self,
        _sequence: SequenceNumber,
    ) -> Result<ReplyOrError<BufWithFds<Vec<u8>>, Vec<u8>>, ConnectionError> {
        unimplemented!()
    }

    fn check_for_raw_error(
        &self,
        _sequence: SequenceNumber,
    ) -> Result<Option<Vec<u8>>, ConnectionError> {
        unimplemented!()
    }

    fn maximum_request_bytes(&self) -> usize {
        // Must be at least 4 * 2^16 so that we can test BIG-REQUESTS
        2usize.pow(19)
    }

    fn prefetch_maximum_request_bytes(&self) {
        unimplemented!()
    }

    fn parse_error(&self, _error: &[u8]) -> Result<x11rb::x11_utils::X11Error, ParseError> {
        unimplemented!()
    }

    fn parse_event(&self, _event: &[u8]) -> Result<x11rb::protocol::Event, ParseError> {
        unimplemented!()
    }
}

#[test]
fn test_poly_segment() -> Result<(), ReplyError> {
    let conn = FakeConnection::default();
    let drawable = 42;
    let gc = 0x1337;
    let segments = [
        Segment {
            x1: 1,
            y1: 2,
            x2: 3,
            y2: 4,
        },
        Segment {
            x1: 5,
            y1: 6,
            x2: 7,
            y2: 8,
        },
    ];
    let length: u16 = (12 + segments.len() * 8) as u16 / 4;
    conn.poly_segment(drawable, gc, &segments)?;

    let mut expected = vec![
        x11rb::protocol::xproto::POLY_SEGMENT_REQUEST,
        0, // padding
    ];
    expected.extend(length.to_ne_bytes()); // length, not in the xml
    expected.extend(drawable.to_ne_bytes());
    expected.extend(gc.to_ne_bytes());
    // Segments
    for x in 1u16..9u16 {
        expected.extend(x.to_ne_bytes());
    }
    conn.check_requests(&[(false, expected)]);
    Ok(())
}

#[test]
fn test_big_requests() -> Result<(), ConnectionError> {
    let conn = FakeConnection::default();
    let big_buffer = [0; (1 << 18) + 1];
    let drawable: u32 = 42;
    let gc: u32 = 0x1337;
    let x: i16 = 21;
    let y: i16 = 7;
    let padding = 3; // big_buffer's size rounded up to a multiple of 4
    let big_request_length_field = 4;
    let length: u32 = (16 + big_request_length_field + big_buffer.len() as u32 + padding) / 4;
    conn.poly_text16(drawable, gc, x, y, &big_buffer)?;

    let mut expected = vec![
        x11rb::protocol::xproto::POLY_TEXT16_REQUEST,
        // padding
        0,
        // Length of zero: we use big requests
        0,
        0,
    ];
    // Actual length
    expected.extend(length.to_ne_bytes());

    expected.extend(drawable.to_ne_bytes());
    expected.extend(gc.to_ne_bytes());
    expected.extend(x.to_ne_bytes());
    expected.extend(y.to_ne_bytes());
    expected.extend(big_buffer.iter());
    expected.extend((0..padding).map(|_| 0));

    conn.check_requests(&[(false, expected)]);
    Ok(())
}

#[test]
fn test_too_large_request() {
    let conn = FakeConnection::default();
    let big_buffer = [0; (1 << 19) + 1];
    let drawable: u32 = 42;
    let gc: u32 = 0x1337;
    let x: i16 = 21;
    let y: i16 = 7;
    let res = conn.poly_text16(drawable, gc, x, y, &big_buffer);
    match res.unwrap_err() {
        ConnectionError::MaximumRequestLengthExceeded => {}
        err => panic!("Wrong error: {:?}", err),
    };
}

#[test]
fn test_send_event() -> Result<(), ConnectionError> {
    // Prepare the event
    let buffer: [u8; 32] = [
        11, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
        24, 25, 26, 27, 28, 29, 30,
    ];
    let event = KeymapNotifyEvent::try_parse(&buffer[..])?.0;

    // "Send" it
    let conn = FakeConnection::default();
    let propagate = true;
    let destination: u32 = 0x1337;
    let event_mask = x11rb_protocol::protocol::xproto::EventMask::BUTTON3_MOTION;
    conn.send_event(propagate, destination, event_mask, event)?;

    let mut expected = vec![x11rb::protocol::xproto::SEND_EVENT_REQUEST, propagate as _];
    expected.extend(((12u16 + 32u16) / 4).to_ne_bytes());
    expected.extend(destination.to_ne_bytes());
    expected.extend(u32::from(event_mask).to_ne_bytes());
    expected.extend(buffer.iter());
    conn.check_requests(&[(false, expected)]);
    Ok(())
}

#[test]
fn test_get_keyboard_mapping() -> Result<(), ConnectionError> {
    let conn = FakeConnection::default();
    let cookie = conn.get_keyboard_mapping(1, 2)?;

    // Prevent call to discard_reply(), we only check request sending
    std::mem::forget(cookie);

    let mut expected = Vec::new();
    let length: u16 = 2;
    expected.push(101); // request major code
    expected.push(0); // padding
    expected.extend(length.to_ne_bytes()); // length, not in the xml
    expected.push(1); // first keycode
    expected.push(2); // length
    expected.extend([0, 0]); // padding

    conn.check_requests(&[(false, expected)]);
    Ok(())
}

#[test]
fn test_client_message_data_parse() {
    let short_array = [0, 0, 0];
    let err = ClientMessageData::try_parse(&short_array);
    assert!(err.is_err());
    assert_eq!(err.unwrap_err(), ParseError::InsufficientData);
}

#[test]
fn test_set_modifier_mapping() -> Result<(), ConnectionError> {
    let conn = FakeConnection::default();
    let cookie = conn.set_modifier_mapping(&(1..17).collect::<Vec<_>>())?;

    // Prevent call to discard_reply(), we only check request sending
    std::mem::forget(cookie);

    let mut expected = Vec::new();
    let length: u16 = 5;
    expected.push(118); // request major code
    expected.push(2); // keycodes per modifier
    expected.extend(length.to_ne_bytes()); // length, not in the xml
    expected.extend(1u8..17u8);

    conn.check_requests(&[(false, expected)]);
    Ok(())
}

#[test]
fn test_serialize_setup_authenticate() {
    let setup = SetupAuthenticate {
        status: 2,
        reason: b"12345678".to_vec(),
    };
    // At the time of writing, the code generator does not produce the correct code...
    let length = 2u16.to_ne_bytes();
    let setup_bytes = [
        2, 0, 0, 0, 0, 0, length[0], length[1], b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8',
    ];
    assert_eq!(&setup_bytes[..], &setup.serialize()[..]);
}

#[cfg(feature = "xinput")]
#[allow(dead_code)]
fn compile_test(conn: &impl RequestConnection) {
    use x11rb::protocol::xinput::{xi_query_device, Device};
    let _ = xi_query_device(conn, Device::ALL);
}
