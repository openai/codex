use std::collections::{HashMap, HashSet};
use std::io::Read;

use proptest::prelude::*;
use wayland_protocols_wlr::data_control::v1::server::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1;

use crate::paste::*;
use crate::tests::state::*;
use crate::tests::TestServer;

#[test]
fn get_mime_types_test() {
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
                    data: HashMap::from([
                        ("first".into(), vec![]),
                        ("second".into(), vec![]),
                        ("third".into(), vec![]),
                    ]),
                }),
                ..Default::default()
            },
        )]),
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let mime_types =
        get_mime_types_internal(ClipboardType::Regular, Seat::Unspecified, Some(socket_name))
            .unwrap();

    let expected = HashSet::from(["first", "second", "third"].map(String::from));
    assert_eq!(mime_types, expected);
}

#[test]
fn get_mime_types_no_data_control() {
    let server = TestServer::new();

    let state = State {
        seats: HashMap::from([(
            "seat0".into(),
            SeatInfo {
                ..Default::default()
            },
        )]),
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let result =
        get_mime_types_internal(ClipboardType::Regular, Seat::Unspecified, Some(socket_name));
    assert!(matches!(
        result,
        Err(Error::MissingProtocol {
            name: "ext-data-control, or wlr-data-control",
            version: 1
        })
    ));
}

#[test]
fn get_mime_types_no_data_control_2() {
    let server = TestServer::new();

    let state = State {
        seats: HashMap::from([(
            "seat0".into(),
            SeatInfo {
                ..Default::default()
            },
        )]),
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let result =
        get_mime_types_internal(ClipboardType::Primary, Seat::Unspecified, Some(socket_name));
    assert!(matches!(
        result,
        Err(Error::MissingProtocol {
            name: "ext-data-control, or wlr-data-control",
            version: 2
        })
    ));
}

#[test]
fn get_mime_types_no_seats() {
    let server = TestServer::new();
    server
        .display
        .handle()
        .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

    let state = State {
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let result =
        get_mime_types_internal(ClipboardType::Primary, Seat::Unspecified, Some(socket_name));
    assert!(matches!(result, Err(Error::NoSeats)));
}

#[test]
fn get_mime_types_empty_clipboard() {
    let server = TestServer::new();
    server
        .display
        .handle()
        .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

    let state = State {
        seats: HashMap::from([(
            "seat0".into(),
            SeatInfo {
                ..Default::default()
            },
        )]),
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let result =
        get_mime_types_internal(ClipboardType::Primary, Seat::Unspecified, Some(socket_name));
    assert!(matches!(result, Err(Error::ClipboardEmpty)));
}

#[test]
fn get_mime_types_specific_seat() {
    let server = TestServer::new();
    server
        .display
        .handle()
        .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

    let state = State {
        seats: HashMap::from([
            (
                "seat0".into(),
                SeatInfo {
                    ..Default::default()
                },
            ),
            (
                "yay".into(),
                SeatInfo {
                    offer: Some(OfferInfo::Buffered {
                        data: HashMap::from([
                            ("first".into(), vec![]),
                            ("second".into(), vec![]),
                            ("third".into(), vec![]),
                        ]),
                    }),
                    ..Default::default()
                },
            ),
        ]),
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let mime_types = get_mime_types_internal(
        ClipboardType::Regular,
        Seat::Specific("yay"),
        Some(socket_name),
    )
    .unwrap();

    let expected = HashSet::from(["first", "second", "third"].map(String::from));
    assert_eq!(mime_types, expected);
}

#[test]
fn get_mime_types_primary() {
    let server = TestServer::new();
    server
        .display
        .handle()
        .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

    let state = State {
        seats: HashMap::from([(
            "seat0".into(),
            SeatInfo {
                primary_offer: Some(OfferInfo::Buffered {
                    data: HashMap::from([
                        ("first".into(), vec![]),
                        ("second".into(), vec![]),
                        ("third".into(), vec![]),
                    ]),
                }),
                ..Default::default()
            },
        )]),
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let mime_types =
        get_mime_types_internal(ClipboardType::Primary, Seat::Unspecified, Some(socket_name))
            .unwrap();

    let expected = HashSet::from(["first", "second", "third"].map(String::from));
    assert_eq!(mime_types, expected);
}

#[test]
fn get_contents_test() {
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
                    data: HashMap::from([("application/octet-stream".into(), vec![1, 3, 3, 7])]),
                }),
                ..Default::default()
            },
        )]),
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let (mut read, mime_type) = get_contents_internal(
        ClipboardType::Regular,
        Seat::Unspecified,
        MimeType::Any,
        Some(socket_name),
    )
    .unwrap();

    assert_eq!(mime_type, "application/octet-stream");

    let mut contents = vec![];
    read.read_to_end(&mut contents).unwrap();
    assert_eq!(contents, [1, 3, 3, 7]);
}

#[test]
fn get_contents_wrong_mime_type() {
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
                    data: HashMap::from([("application/octet-stream".into(), vec![1, 3, 3, 7])]),
                }),
                ..Default::default()
            },
        )]),
        ..Default::default()
    };
    state.create_seats(&server);

    let socket_name = server.socket_name().to_owned();
    server.run(state);

    let result = get_contents_internal(
        ClipboardType::Regular,
        Seat::Unspecified,
        MimeType::Specific("wrong"),
        Some(socket_name),
    );
    assert!(matches!(result, Err(Error::NoMimeType)));
}

proptest! {
    #[test]
    fn get_mime_types_randomized(
        mut state: State,
        clipboard_type: ClipboardType,
        seat_index: prop::sample::Index,
    ) {
        let server = TestServer::new();
        let socket_name = server.socket_name().to_owned();
        server
            .display
            .handle()
            .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

        state.create_seats(&server);

        if state.seats.is_empty() {
            server.run(state);

            let result = get_mime_types_internal(clipboard_type, Seat::Unspecified, Some(socket_name));
            prop_assert!(matches!(result, Err(Error::NoSeats)));
        } else {
            let seat_index = seat_index.index(state.seats.len());
            let (seat_name, seat_info) = state.seats.iter().nth(seat_index).unwrap();
            let seat_name = seat_name.to_owned();
            let seat_info = (*seat_info).clone();

            server.run(state);

            let result = get_mime_types_internal(
                clipboard_type,
                Seat::Specific(&seat_name),
                Some(socket_name),
            );

            let expected_offer = match clipboard_type {
                ClipboardType::Regular => &seat_info.offer,
                ClipboardType::Primary => &seat_info.primary_offer,
            };
            match expected_offer {
                None => prop_assert!(matches!(result, Err(Error::ClipboardEmpty))),
                Some(offer) => prop_assert_eq!(result.unwrap(), offer.data().keys().cloned().collect()),
            }
        }
    }

    #[test]
    fn get_contents_randomized(
        mut state: State,
        clipboard_type: ClipboardType,
        seat_index: prop::sample::Index,
        mime_index: prop::sample::Index,
    ) {
        let server = TestServer::new();
        let socket_name = server.socket_name().to_owned();
        server
            .display
            .handle()
            .create_global::<State, ZwlrDataControlManagerV1, ()>(2, ());

        state.create_seats(&server);

        if state.seats.is_empty() {
            server.run(state);

            let result = get_mime_types_internal(clipboard_type, Seat::Unspecified, Some(socket_name));
            prop_assert!(matches!(result, Err(Error::NoSeats)));
        } else {
            let seat_index = seat_index.index(state.seats.len());
            let (seat_name, seat_info) = state.seats.iter().nth(seat_index).unwrap();
            let seat_name = seat_name.to_owned();
            let seat_info = (*seat_info).clone();

            let expected_offer = match clipboard_type {
                ClipboardType::Regular => &seat_info.offer,
                ClipboardType::Primary => &seat_info.primary_offer,
            };

            let mime_type = match expected_offer {
                Some(offer) if !offer.data().is_empty() => {
                    let mime_index = mime_index.index(offer.data().len());
                    Some(offer.data().keys().nth(mime_index).unwrap())
                }
                _ => None,
            };

            server.run(state);

            let result = get_contents_internal(
                clipboard_type,
                Seat::Specific(&seat_name),
                mime_type.map_or(MimeType::Any, |name| MimeType::Specific(name)),
                Some(socket_name),
            );

            match expected_offer {
                None => prop_assert!(matches!(result, Err(Error::ClipboardEmpty))),
                Some(offer) => {
                    if offer.data().is_empty() {
                        prop_assert!(matches!(result, Err(Error::NoMimeType)));
                    } else {
                        let mime_type = mime_type.unwrap();

                        let (mut read, recv_mime_type) = result.unwrap();
                        prop_assert_eq!(&recv_mime_type, mime_type);

                        let mut contents = vec![];
                        read.read_to_end(&mut contents).unwrap();
                        prop_assert_eq!(&contents, &offer.data()[mime_type]);
                    }
                },
            }

        }
    }
}
