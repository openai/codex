use super::*;
use pretty_assertions::assert_eq;
use rtc::rtp::header::Header;
use rtc::sansio::Protocol;

fn pair() -> (Incoming, Ingress) {
    let (incoming, ingress) = Incoming::new();
    incoming.set_suppressed(/*suppressed*/ false).unwrap();
    (incoming, ingress)
}

fn packet(size: usize, at: Instant) -> TaggedPacket {
    TaggedPacket {
        now: at,
        transport: Default::default(),
        message: Packet::Rtp(rtc::rtp::Packet {
            header: Header {
                version: 2,
                payload_type: 111,
                ssrc: 7,
                ..Default::default()
            },
            payload: vec![5; size - 12].into(),
        }),
    }
}

#[test]
fn arrival_and_rtp_bytes_survive_the_adapter_without_an_upstream_copy() {
    let (mut incoming, mut ingress) = pair();
    let at = Instant::now() - Duration::from_millis(/*millis*/ 20);
    let input = packet(/*size*/ 32, at);
    let Packet::Rtp(expected) = &input.message else {
        panic!()
    };
    let expected = expected.marshal().unwrap();
    ingress.handle_read(input).unwrap();
    let received = incoming.take().unwrap().unwrap();
    assert_eq!((received.at, received.as_ref()), (at, expected.as_ref()));
    assert!(ingress.poll_read().is_none());
}

#[test]
fn consumed_packets_keep_their_budget_until_the_last_owner_drops_them() {
    for size in [32, PACKET_BYTES] {
        let (mut incoming, mut ingress) = pair();
        let count = PACKETS.min(BYTES / size);
        let mut held = Vec::new();
        for _ in 0..count {
            ingress.handle_read(packet(size, Instant::now())).unwrap();
            held.push(incoming.take().unwrap().unwrap());
        }
        drop(held.pop());
        ingress.handle_read(packet(size, Instant::now())).unwrap();
        held.push(incoming.take().unwrap().unwrap());
        ingress.handle_read(packet(size, Instant::now())).unwrap();
        assert_eq!(incoming.take().err(), Some("incoming audio failed"));
        drop(held);
        assert_eq!(
            (
                incoming.state.packets.available_permits(),
                incoming.state.bytes.available_permits()
            ),
            (PACKETS, BYTES)
        );
    }
}

#[test]
fn suppression_discards_queued_and_in_flight_old_epochs() {
    let (mut incoming, mut ingress) = pair();
    ingress
        .handle_read(packet(/*size*/ 32, Instant::now()))
        .unwrap();
    incoming.set_suppressed(/*suppressed*/ true).unwrap();
    ingress
        .handle_read(packet(/*size*/ 32, Instant::now()))
        .unwrap();
    incoming.set_suppressed(/*suppressed*/ false).unwrap();
    assert!(incoming.take().unwrap().is_none());
    ingress
        .handle_read(packet(/*size*/ 32, Instant::now()))
        .unwrap();
    assert!(incoming.take().unwrap().is_some());
}

#[test]
fn invalid_stream_and_oversize_packets_fail_without_returning_media() {
    for size in [32, PACKET_BYTES + 1] {
        let (mut incoming, mut ingress) = pair();
        let mut input = packet(size, Instant::now());
        if size == 32
            && let Packet::Rtp(packet) = &mut input.message
        {
            packet.header.payload_type = 96;
        }
        ingress.handle_read(input).unwrap();
        assert_eq!(incoming.take().err(), Some("incoming audio failed"));
    }
}
