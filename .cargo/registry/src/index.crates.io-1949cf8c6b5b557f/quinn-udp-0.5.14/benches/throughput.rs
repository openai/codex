use std::{
    cmp::min,
    io::{ErrorKind, IoSliceMut},
    net::{Ipv4Addr, Ipv6Addr, UdpSocket},
};

use criterion::{Criterion, criterion_group, criterion_main};
use tokio::{io::Interest, runtime::Runtime};

use quinn_udp::{BATCH_SIZE, RecvMeta, Transmit, UdpSocketState};

pub fn criterion_benchmark(c: &mut Criterion) {
    const TOTAL_BYTES: usize = 10 * 1024 * 1024;
    const SEGMENT_SIZE: usize = 1280;

    let rt = Runtime::new().unwrap();
    let _guard = rt.enter();

    let (send_state, send_socket) = new_socket();
    let (recv_state, recv_socket) = new_socket();
    let dst_addr = recv_socket.local_addr().unwrap();

    let mut permutations = vec![];
    for gso_enabled in [
        false,
        #[cfg(any(target_os = "linux", target_os = "windows", apple))]
        true,
    ] {
        for gro_enabled in [false, true] {
            #[cfg(target_os = "windows")]
            if gso_enabled && !gro_enabled {
                // Windows requires receive buffer to fit entire datagram on GRO
                // enabled socket.
                //
                // OS error: "A message sent on a datagram socket was larger
                // than the internal message buffer or some other network limit,
                // or the buffer used to receive a datagram into was smaller
                // than the datagram itself."
                continue;
            }

            for recvmmsg_enabled in [false, true] {
                permutations.push((gso_enabled, gro_enabled, recvmmsg_enabled));
            }
        }
    }

    for (gso_enabled, gro_enabled, recvmmsg_enabled) in permutations {
        let mut group = c.benchmark_group(format!(
            "gso_{gso_enabled}_gro_{gro_enabled}_recvmmsg_{recvmmsg_enabled}"
        ));
        group.throughput(criterion::Throughput::Bytes(TOTAL_BYTES as u64));

        let gso_segments = if gso_enabled {
            send_state.max_gso_segments()
        } else {
            1
        };
        let msg = vec![0xAB; min(MAX_DATAGRAM_SIZE, SEGMENT_SIZE * gso_segments)];
        let transmit = Transmit {
            destination: dst_addr,
            ecn: None,
            contents: &msg,
            segment_size: gso_enabled.then_some(SEGMENT_SIZE),
            src_ip: None,
        };
        let gro_segments = if gro_enabled {
            recv_state.gro_segments()
        } else {
            1
        };
        let batch_size = if recvmmsg_enabled { BATCH_SIZE } else { 1 };

        group.bench_function("throughput", |b| {
            b.to_async(&rt).iter(|| async {
                let mut receive_buffers = vec![vec![0; SEGMENT_SIZE * gro_segments]; batch_size];
                let mut receive_slices = receive_buffers
                    .iter_mut()
                    .map(|buf| IoSliceMut::new(buf))
                    .collect::<Vec<_>>();
                let mut meta = vec![RecvMeta::default(); batch_size];

                let mut sent: usize = 0;
                let mut received: usize = 0;
                while sent < TOTAL_BYTES {
                    send_socket.writable().await.unwrap();
                    send_socket
                        .try_io(Interest::WRITABLE, || {
                            send_state.send((&send_socket).into(), &transmit)
                        })
                        .unwrap();
                    sent += transmit.contents.len();

                    while received < sent {
                        recv_socket.readable().await.unwrap();
                        let n = match recv_socket.try_io(Interest::READABLE, || {
                            recv_state.recv((&recv_socket).into(), &mut receive_slices, &mut meta)
                        }) {
                            Ok(n) => n,
                            // recv.readable() can lead to false positives. Try again.
                            Err(e) if e.kind() == ErrorKind::WouldBlock => continue,
                            e => e.unwrap(),
                        };
                        received += meta.iter().map(|m| m.len).take(n).sum::<usize>();
                    }
                }
            })
        });
    }
}

fn new_socket() -> (UdpSocketState, tokio::net::UdpSocket) {
    let socket = UdpSocket::bind((Ipv6Addr::LOCALHOST, 0))
        .or_else(|_| UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)))
        .unwrap();

    (
        UdpSocketState::new((&socket).into()).unwrap(),
        tokio::net::UdpSocket::from_std(socket).unwrap(),
    )
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

const MAX_IP_UDP_HEADER_SIZE: usize = 48;
const MAX_DATAGRAM_SIZE: usize = u16::MAX as usize - MAX_IP_UDP_HEADER_SIZE;
