use std::collections::HashMap;
use std::io::Read;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use proptest::prelude::*;
use wayland_protocols_wlr::data_control::v1::server::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1;

use crate::copy::*;
use crate::paste;
use crate::paste::get_contents_internal;
use crate::tests::state::*;
use crate::tests::TestServer;

#[test]
fn clear_test() {
    let server = TestServer::new();
    server
        .display
        .handle()
        .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

    let state = State {
        seats: HashMap::from([(
            "seat0".into(),
            SeatInfo {
                offer: Some(OfferInfo::Buffered {
                    data: HashMap::from([("regular".into(), vec![1, 2, 3])]),
                }),
                primary_offer: Some(OfferInfo::Buffered {
                    data: HashMap::from([("primary".into(), vec![1, 2, 3])]),
                }),
            },
        )]),
        ..Default::default()
    };
    state.create_seats(&server);
    let state = Arc::new(Mutex::new(state));

    let socket_name = server.socket_name().to_owned();
    server.run_mutex(state.clone());

    clear_internal(ClipboardType::Regular, Seat::All, Some(socket_name)).unwrap();

    let state = state.lock().unwrap();
    assert!(state.seats["seat0"].offer.is_none());
    assert!(state.seats["seat0"].primary_offer.is_some());
}

#[test]
fn copy_test() {
    let server = TestServer::new();
    server
        .display
        .handle()
        .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

    let (tx, rx) = channel();

    let state = State {
        seats: HashMap::from([(
            "seat0".into(),
            SeatInfo {
                ..Default::default()
            },
        )]),
        selection_updated_sender: Some(tx),
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let sources = vec![MimeSource {
        source: Source::Bytes([1, 3, 3, 7][..].into()),
        mime_type: MimeType::Specific("test".into()),
    }];
    copy_internal(Options::new(), sources, Some(socket_name.clone())).unwrap();

    // Wait for the copy.
    let mime_types = rx.recv().unwrap().unwrap();
    assert_eq!(mime_types, ["test"]);

    let (mut read, mime_type) = get_contents_internal(
        paste::ClipboardType::Regular,
        paste::Seat::Unspecified,
        paste::MimeType::Any,
        Some(socket_name.clone()),
    )
    .unwrap();

    let mut contents = vec![];
    read.read_to_end(&mut contents).unwrap();

    assert_eq!(mime_type, "test");
    assert_eq!(contents, [1, 3, 3, 7]);

    clear_internal(ClipboardType::Both, Seat::All, Some(socket_name)).unwrap();
}

#[test]
fn copy_multi_test() {
    let server = TestServer::new();
    server
        .display
        .handle()
        .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

    let (tx, rx) = channel();

    let state = State {
        seats: HashMap::from([(
            "seat0".into(),
            SeatInfo {
                ..Default::default()
            },
        )]),
        selection_updated_sender: Some(tx),
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let sources = vec![
        MimeSource {
            source: Source::Bytes([1, 3, 3, 7][..].into()),
            mime_type: MimeType::Specific("test".into()),
        },
        MimeSource {
            source: Source::Bytes([2, 4, 4][..].into()),
            mime_type: MimeType::Specific("test2".into()),
        },
        // Ignored because it's the second "test" MIME type.
        MimeSource {
            source: Source::Bytes([4, 3, 2, 1][..].into()),
            mime_type: MimeType::Specific("test".into()),
        },
        // The first text source, additional text types should fall back here.
        MimeSource {
            source: Source::Bytes(b"hello fallback"[..].into()),
            mime_type: MimeType::Text,
        },
        // A specific override of an additional text type.
        MimeSource {
            source: Source::Bytes(b"hello TEXT"[..].into()),
            mime_type: MimeType::Specific("TEXT".into()),
        },
    ];
    copy_internal(Options::new(), sources, Some(socket_name.clone())).unwrap();

    // Wait for the copy.
    let mut mime_types = rx.recv().unwrap().unwrap();
    mime_types.sort_unstable();
    assert_eq!(
        mime_types,
        [
            "STRING",
            "TEXT",
            "UTF8_STRING",
            "test",
            "test2",
            "text/plain",
            "text/plain;charset=utf-8",
        ]
    );

    let expected = [
        ("test", &[1, 3, 3, 7][..]),
        ("test2", &[2, 4, 4][..]),
        ("STRING", &b"hello fallback"[..]),
        ("TEXT", &b"hello TEXT"[..]),
    ];

    for (mime_type, expected_contents) in expected {
        let mut read = get_contents_internal(
            paste::ClipboardType::Regular,
            paste::Seat::Unspecified,
            paste::MimeType::Specific(mime_type),
            Some(socket_name.clone()),
        )
        .unwrap()
        .0;

        let mut contents = vec![];
        read.read_to_end(&mut contents).unwrap();

        assert_eq!(contents, expected_contents);
    }

    clear_internal(ClipboardType::Both, Seat::All, Some(socket_name)).unwrap();
}

#[test]
fn copy_multi_no_additional_text_mime_types_test() {
    let server = TestServer::new();
    server
        .display
        .handle()
        .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

    let (tx, rx) = channel();

    let state = State {
        seats: HashMap::from([(
            "seat0".into(),
            SeatInfo {
                ..Default::default()
            },
        )]),
        selection_updated_sender: Some(tx),
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let mut opts = Options::new();
    opts.omit_additional_text_mime_types(true);
    let sources = vec![
        MimeSource {
            source: Source::Bytes([1, 3, 3, 7][..].into()),
            mime_type: MimeType::Specific("test".into()),
        },
        MimeSource {
            source: Source::Bytes([2, 4, 4][..].into()),
            mime_type: MimeType::Specific("test2".into()),
        },
        // Ignored because it's the second "test" MIME type.
        MimeSource {
            source: Source::Bytes([4, 3, 2, 1][..].into()),
            mime_type: MimeType::Specific("test".into()),
        },
        // A specific override of an additional text type.
        MimeSource {
            source: Source::Bytes(b"hello TEXT"[..].into()),
            mime_type: MimeType::Specific("TEXT".into()),
        },
    ];
    copy_internal(opts, sources, Some(socket_name.clone())).unwrap();

    // Wait for the copy.
    let mut mime_types = rx.recv().unwrap().unwrap();
    mime_types.sort_unstable();
    assert_eq!(mime_types, ["TEXT", "test", "test2"]);

    let expected = [
        ("test", &[1, 3, 3, 7][..]),
        ("test2", &[2, 4, 4][..]),
        ("TEXT", &b"hello TEXT"[..]),
    ];

    for (mime_type, expected_contents) in expected {
        let mut read = get_contents_internal(
            paste::ClipboardType::Regular,
            paste::Seat::Unspecified,
            paste::MimeType::Specific(mime_type),
            Some(socket_name.clone()),
        )
        .unwrap()
        .0;

        let mut contents = vec![];
        read.read_to_end(&mut contents).unwrap();

        assert_eq!(contents, expected_contents);
    }

    clear_internal(ClipboardType::Both, Seat::All, Some(socket_name)).unwrap();
}

// The idea here is to exceed the pipe capacity. This fails unless O_NONBLOCK is cleared when
// sending data over the pipe using cat.
#[test]
fn copy_large() {
    // Assuming the default pipe capacity is 65536.
    let mut bytes_to_copy = vec![];
    for i in 0..65536 * 10 {
        bytes_to_copy.push((i % 256) as u8);
    }

    let server = TestServer::new();
    server
        .display
        .handle()
        .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

    let (tx, rx) = channel();

    let state = State {
        seats: HashMap::from([(
            "seat0".into(),
            SeatInfo {
                ..Default::default()
            },
        )]),
        selection_updated_sender: Some(tx),
        // Emulate what XWayland does and set O_NONBLOCK.
        set_nonblock_on_write_fd: true,
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let sources = vec![MimeSource {
        source: Source::Bytes(bytes_to_copy.clone().into_boxed_slice()),
        mime_type: MimeType::Specific("test".into()),
    }];
    copy_internal(Options::new(), sources, Some(socket_name.clone())).unwrap();

    // Wait for the copy.
    let mime_types = rx.recv().unwrap().unwrap();
    assert_eq!(mime_types, ["test"]);

    let (mut read, mime_type) = get_contents_internal(
        paste::ClipboardType::Regular,
        paste::Seat::Unspecified,
        paste::MimeType::Any,
        Some(socket_name.clone()),
    )
    .unwrap();

    let mut contents = vec![];
    read.read_to_end(&mut contents).unwrap();

    assert_eq!(mime_type, "test");
    assert_eq!(contents.len(), bytes_to_copy.len());
    assert_eq!(contents, bytes_to_copy);

    clear_internal(ClipboardType::Both, Seat::All, Some(socket_name)).unwrap();
}

proptest! {
    #[test]
    fn copy_randomized(
        mut state: State,
        clipboard_type: ClipboardType,
        source: Source,
        mime_type: MimeType,
        seat_index: prop::sample::Index,
        clipboard_type_index: prop::sample::Index,
    ) {
        prop_assume!(!state.seats.is_empty());

        let server = TestServer::new();
        server
            .display
            .handle()
            .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

        let (tx, rx) = channel();
        state.selection_updated_sender = Some(tx);

        state.create_seats(&server);

        let seat_index = seat_index.index(state.seats.len());
        let seat_name = state.seats.keys().nth(seat_index).unwrap();
        let seat_name = seat_name.to_owned();

        let paste_clipboard_type = match clipboard_type {
            ClipboardType::Regular => paste::ClipboardType::Regular,
            ClipboardType::Primary => paste::ClipboardType::Primary,
            ClipboardType::Both => *clipboard_type_index
                .get(&[paste::ClipboardType::Regular, paste::ClipboardType::Primary]),
        };

        let socket_name = server.socket_name().to_owned();
        server.run(state);

        let expected_contents = match &source {
            Source::Bytes(bytes) => bytes.clone(),
            Source::StdIn => unreachable!(),
        };

        let sources = vec![MimeSource {
            source,
            mime_type: mime_type.clone(),
        }];
        let mut opts = Options::new();
        opts.clipboard(clipboard_type);
        opts.seat(Seat::Specific(seat_name.clone()));
        opts.omit_additional_text_mime_types(true);
        copy_internal(opts, sources, Some(socket_name.clone())).unwrap();

        // Wait for the copy.
        let mut mime_types = rx.recv().unwrap().unwrap();
        mime_types.sort_unstable();
        match &mime_type {
            MimeType::Autodetect => unreachable!(),
            MimeType::Text => assert_eq!(mime_types, ["text/plain"]),
            MimeType::Specific(mime) => assert_eq!(mime_types, [mime.clone()]),
        }

        let paste_mime_type = match mime_type {
            MimeType::Autodetect => unreachable!(),
            MimeType::Text => "text/plain".into(),
            MimeType::Specific(mime) => mime,
        };
        let (mut read, mime_type) = get_contents_internal(
            paste_clipboard_type,
            paste::Seat::Specific(&seat_name),
            paste::MimeType::Specific(&paste_mime_type),
            Some(socket_name.clone()),
        )
        .unwrap();

        let mut contents = vec![];
        read.read_to_end(&mut contents).unwrap();

        assert_eq!(mime_type, paste_mime_type);
        assert_eq!(contents.into_boxed_slice(), expected_contents);

        clear_internal(clipboard_type, Seat::Specific(seat_name), Some(socket_name)).unwrap();
    }
}
