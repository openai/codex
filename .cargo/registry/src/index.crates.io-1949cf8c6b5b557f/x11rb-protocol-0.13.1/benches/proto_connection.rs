//! Benchmark the `x11rb_protocol::Connection` type's method, at varying levels of
//! capacity.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use std::{
    io::{Read, Write},
    mem::{replace, size_of},
    net::{TcpListener, TcpStream},
    thread,
};
use x11rb_protocol::{
    connection::{Connection, ReplyFdKind},
    protocol::xproto::{Depth, Rectangle, Screen},
    x11_utils::{Serialize, TryParse},
    DiscardMode, SequenceNumber,
};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

fn pow(i: i32, p: i32) -> i32 {
    let mut result = 1;
    for _ in 0..p {
        result *= i;
    }
    result
}

fn enqueue_packet_test(c: &mut Criterion) {
    // take the cartesian product of the following conditions:
    // - the packet is an event, a reply, or an error
    // - pending_events and pending_replies are empty, have one element, or have
    //   many elements

    enum PacketType {
        Event,
        Reply,
        Error,
    }

    enum PacketCount {
        Empty,
        One,
        Many,
    }

    use PacketCount::*;
    use PacketType::*;

    let mut group = c.benchmark_group("enqueue_packet");
    for packet_ty in &[Event, Reply, Error] {
        for packet_count in &[Empty, One, Many] {
            let packet_ty_desc = match packet_ty {
                Event => "event",
                Reply => "reply",
                Error => "error",
            };

            let packet_count_desc = match packet_count {
                Empty => "no",
                One => "one",
                Many => "many",
            };

            let name = format!(
                "enqueue_packet {} with {} packets",
                packet_ty_desc, packet_count_desc
            );
            group.bench_function(name, |b| {
                // generate a valid packet with the given first byte and sequence number
                let mut seqno = 0u16;
                let mut packet = move |ind: u8| {
                    let our_seqno = seqno + 1;
                    seqno += 1;

                    let mut v = vec![0; 32];
                    v[0] = ind;

                    // copy our_seqno to bytes 3 and 4
                    v[2..4].copy_from_slice(&our_seqno.to_ne_bytes());

                    v
                };

                // we need another one for make_conn
                let mut packet2 = packet;

                let queue_count = match packet_count {
                    PacketCount::Empty => 0,
                    PacketCount::One => 1,
                    PacketCount::Many => pow(2, 8),
                };

                // create a connection with the given stats
                let mut make_conn = || {
                    let mut conn = Connection::new();

                    for _ in 0..queue_count {
                        // push a new event
                        conn.enqueue_packet(packet2(2));
                    }

                    for _ in 0..queue_count {
                        // push a new reply
                        conn.enqueue_packet(packet2(1));
                    }

                    conn
                };

                let mut conn = make_conn();
                let packet = packet(match packet_ty {
                    Event => 2,
                    Reply => 1,
                    Error => 0,
                });

                b.iter(move || {
                    conn.enqueue_packet(packet.clone());
                })
            });
        }
    }
}

fn send_and_receive_request(c: &mut Criterion) {
    // permutations:
    // - send queue is empty or very full
    // - receive queue is empty of very full
    enum SendQueue {
        SEmpty,
        SFull,
    }

    enum RecvQueue {
        REmpty,
        RFull,
    }

    use RecvQueue::*;
    use SendQueue::*;

    let mut group = c.benchmark_group("send_and_receive_request");

    for send_queue in &[SEmpty, SFull] {
        for recv_queue in &[REmpty, RFull] {
            let name = format!(
                "send_and_receive_request (send {}, recv {})",
                match send_queue {
                    SEmpty => "empty",
                    SFull => "full",
                },
                match recv_queue {
                    REmpty => "empty",
                    RFull => "full",
                }
            );

            group.bench_function(name, |b| {
                // create a new connection
                let mut conn = Connection::new();

                // if the send queue needs to be full, flood it with sent requests
                if matches!(send_queue, SFull) {
                    for _ in 0..pow(2, 14) {
                        conn.send_request(match recv_queue {
                            REmpty => ReplyFdKind::NoReply,
                            RFull => ReplyFdKind::ReplyWithoutFDs,
                        });
                    }
                }

                // if the recv queue needs to be full, flood it with replies
                if matches!(recv_queue, RFull) {
                    for _ in 0..pow(2, 14) {
                        let mut packet = vec![0; 32];
                        packet[0] = 1;
                        conn.enqueue_packet(packet);
                    }
                }

                // create a new packet
                let mut packet = vec![0u8; 32];
                packet[0] = 1;

                b.iter(move || {
                    // send our request
                    let seq = conn.send_request(ReplyFdKind::ReplyWithoutFDs).unwrap();

                    // truncate to a u16
                    let seq_trunc = seq as u16;

                    // insert the sequence number at positions 2 and 3
                    packet[2..4].copy_from_slice(&seq_trunc.to_ne_bytes());

                    // enqueue the packet
                    conn.enqueue_packet(black_box(replace(&mut packet, vec![0u8; 32])));

                    // pop the reply
                    conn.poll_for_reply_or_error(seq)
                })
            });
        }
    }
}

fn try_parse_small_struct(c: &mut Criterion) {
    // xproto::Rectangle is a pointer wide on 64-bit, use that
    c.bench_function("try_parse an xproto::Rectangle", |b| {
        let packet = [0x42u8; size_of::<Rectangle>()];
        b.iter(|| Rectangle::try_parse(black_box(&packet)))
    });
}

fn try_parse_large_struct(c: &mut Criterion) {
    // xproto::Screen is a significantly larger structure, use that
    const SCREEN_BASE_SIZE: usize = size_of::<Screen>() - size_of::<Vec<Depth>>() + size_of::<u8>();
    const NUM_DEPTHS: usize = 3;
    const DEPTH_SIZE: usize = 8;
    const TOTAL_SIZE: usize = SCREEN_BASE_SIZE + (NUM_DEPTHS * DEPTH_SIZE);

    c.bench_function("try_parse an xproto::Screen", |b| {
        let mut packet = [0; TOTAL_SIZE];
        packet[SCREEN_BASE_SIZE - 1] = NUM_DEPTHS as u8;
        b.iter(|| Screen::try_parse(black_box(&packet)))
    });
}

fn serialize_struct(c: &mut Criterion) {
    // try the following:
    // - send it down a TCP socket
    // - send it down a Unix socket (if linux)
    //
    // this should relatively accurately tell us what kind of impact the buffering
    // and writing have on the serialization time
    //
    // note that send() and recv() degenerate into sendmsg() and recvmsg(), at least
    // on the Linux kernel end, so not using those functions should have no effect
    enum SocketTy {
        TryTcp,
        TryUnix,
    }

    enum StructType {
        Small,
        Large,
    }

    use SocketTy::*;
    use StructType::*;

    let mut group = c.benchmark_group("serialize_struct");
    for socket_ty in &[TryTcp, TryUnix] {
        let mut fd: Box<dyn Write> = match socket_ty {
            TryTcp => {
                const PORT: u16 = 41234;

                let listen = TcpListener::bind(("::1", PORT)).unwrap();
                thread::spawn(move || {
                    let (mut sock, _) = listen.accept().unwrap();

                    // read until other sock gets dropped
                    let mut buf = [0u8; 1024];
                    loop {
                        if sock.read(&mut buf).is_err() {
                            break;
                        }
                    }
                });
                let sock = TcpStream::connect(("::1", PORT)).unwrap();
                Box::new(sock)
            }
            TryUnix => {
                #[cfg(unix)]
                {
                    let (mut left, right) = UnixStream::pair().unwrap();
                    thread::spawn(move || {
                        let mut buf = [0u8; 1024];
                        loop {
                            if left.read(&mut buf).is_err() {
                                break;
                            }
                        }
                    });
                    Box::new(right)
                }
                #[cfg(not(unix))]
                {
                    continue;
                }
            }
        };

        let try_desc = match socket_ty {
            TryTcp => "TCP",
            TryUnix => "Unix",
        };

        for struct_size in &[Small, Large] {
            let size_desc = match struct_size {
                Small => "small",
                Large => "large",
            };

            let name = format!("serialize_struct {} {}", try_desc, size_desc);
            group.bench_function(name, |b| {
                b.iter(|| {
                    let bytes = match struct_size {
                        Small => {
                            let rect = Rectangle::default();
                            black_box(rect.serialize()).to_vec()
                        }
                        Large => {
                            let mut screen = Screen::default();
                            screen.allowed_depths.resize_with(3, Default::default);
                            black_box(screen.serialize())
                        }
                    };

                    // write the serialized bytes tothe output
                    fd.write_all(&bytes).unwrap();
                })
            });
        }
    }
}

fn discard_reply(c: &mut Criterion) {
    // Measure the performance of discard_reply()

    fn get_connection_and_seqnos() -> (Connection, Vec<SequenceNumber>) {
        let mut conn = Connection::new();

        let seqnos = (0..pow(2, 13))
            .map(|_| conn.send_request(ReplyFdKind::NoReply).unwrap())
            .collect();

        (conn, seqnos)
    }

    let mut group = c.benchmark_group("discard_reply");

    group.bench_function("discard oldest", |b| {
        b.iter_batched(
            get_connection_and_seqnos,
            |(mut conn, seqnos)| {
                conn.discard_reply(*seqnos.first().unwrap(), DiscardMode::DiscardReply)
            },
            BatchSize::SmallInput,
        );
    });
    group.bench_function("discard newest", |b| {
        b.iter_batched(
            get_connection_and_seqnos,
            |(mut conn, seqnos)| {
                conn.discard_reply(*seqnos.last().unwrap(), DiscardMode::DiscardReply)
            },
            BatchSize::SmallInput,
        );
    });
    group.bench_function("discard all forward", |b| {
        b.iter_batched(
            get_connection_and_seqnos,
            |(mut conn, seqnos)| {
                for seqno in seqnos {
                    conn.discard_reply(seqno, DiscardMode::DiscardReply)
                }
            },
            BatchSize::SmallInput,
        );
    });
    group.bench_function("discard all backward", |b| {
        b.iter_batched(
            get_connection_and_seqnos,
            |(mut conn, seqnos)| {
                for seqno in seqnos.into_iter().rev() {
                    conn.discard_reply(seqno, DiscardMode::DiscardReply)
                }
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    enqueue_packet_test,
    send_and_receive_request,
    try_parse_small_struct,
    try_parse_large_struct,
    serialize_struct,
    discard_reply,
);
criterion_main!(benches);
