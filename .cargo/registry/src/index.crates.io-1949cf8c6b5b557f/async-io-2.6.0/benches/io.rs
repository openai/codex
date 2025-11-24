//! Benchmarks for a variety of I/O operations.

#![allow(clippy::incompatible_msrv)] // false positive: https://github.com/rust-lang/rust-clippy/issues/12257#issuecomment-2093667187

use async_io::Async;
use criterion::{criterion_group, criterion_main, Criterion};
use futures_lite::{future, prelude::*};
use std::hint::black_box;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};

/// Block on a future, either using the I/O driver or simple parking.
fn block_on<R>(fut: impl Future<Output = R>, drive: bool) -> R {
    if drive {
        async_io::block_on(fut)
    } else {
        future::block_on(fut)
    }
}

fn read_and_write(b: &mut Criterion) {
    const TCP_AMOUNT: usize = 1024 * 1024;

    let mut group = b.benchmark_group("read_and_write");

    for (driver_name, exec) in [("Undriven", false), ("Driven", true)] {
        // Benchmark the TCP streams.
        let init_reader_writer = || {
            let listener = TcpListener::bind("localhost:12345").unwrap();
            let read_stream = TcpStream::connect("localhost:12345").unwrap();
            let (write_stream, _) = listener.accept().unwrap();

            let reader = Async::new(read_stream).unwrap();
            let writer = Async::new(write_stream).unwrap();

            (listener, reader, writer)
        };

        group.bench_function(format!("TcpStream.{driver_name}"), move |b| {
            let (_listener, mut reader, mut writer) = init_reader_writer();
            let mut buf = vec![0x42; TCP_AMOUNT];

            b.iter(|| {
                let buf = &mut buf;

                block_on(
                    async {
                        black_box(writer.write_all(&*buf).await.ok());
                        black_box(reader.read_exact(buf).await.ok());
                    },
                    exec,
                );
            });
        });

        #[cfg(unix)]
        {
            // Benchmark the Unix sockets.
            use std::os::unix::net::UnixStream;
            const UNIX_AMOUNT: usize = 1024;

            group.bench_function(format!("UnixStream.{driver_name}"), |b| {
                let (mut reader, mut writer) = Async::<UnixStream>::pair().unwrap();
                let mut buf = vec![0x42; UNIX_AMOUNT];

                b.iter(|| {
                    let buf = &mut buf;
                    block_on(
                        async {
                            black_box(writer.write_all(&*buf).await.ok());
                            black_box(reader.read_exact(buf).await.ok());
                        },
                        exec,
                    );
                });
            });
        }
    }
}

fn connect_and_accept(c: &mut Criterion) {
    let mut group = c.benchmark_group("connect_and_accept");

    for (driver_name, exec) in [("Undriven", false), ("Driven", true)] {
        // Benchmark the TCP streams.
        group.bench_function(format!("TcpStream.{driver_name}"), move |b| {
            let socket_addr =
                SocketAddr::new("127.0.0.1".parse::<Ipv4Addr>().unwrap().into(), 12345);
            let listener = Async::<TcpListener>::bind(socket_addr).unwrap();

            b.iter(|| {
                block_on(
                    async {
                        let _reader = Async::<TcpStream>::connect(socket_addr).await.ok();
                        black_box(listener.accept().await.ok());
                        drop(black_box(_reader));
                    },
                    exec,
                );
            });
        });

        #[cfg(unix)]
        {
            // Benchmark the Unix sockets.
            use std::os::unix::net::{UnixListener, UnixStream};

            let mut id = [0u8; 8];
            getrandom::fill(&mut id).unwrap();
            let id = u64::from_ne_bytes(id);

            let socket_addr = format!("/tmp/async-io-bench-{id}.sock");
            let listener = Async::<UnixListener>::bind(&socket_addr).unwrap();

            group.bench_function(format!("UnixStream.{driver_name}"), |b| {
                b.iter(|| {
                    block_on(
                        async {
                            let _reader = Async::<UnixStream>::connect(&socket_addr).await.ok();
                            black_box(listener.accept().await.ok());
                            drop(black_box(_reader));
                        },
                        exec,
                    );
                });
            });

            drop(listener);
        }
    }
}

fn udp_send_recv(c: &mut Criterion) {
    const UDP_AMOUNT: usize = 1024;

    let mut group = c.benchmark_group("udp_send_recv");

    // Create a pair of UDP sockets.
    let socket_addr = |port| SocketAddr::new("127.0.0.1".parse::<Ipv4Addr>().unwrap().into(), port);
    let socket_addr1 = socket_addr(12345);
    let socket_addr2 = socket_addr(12346);

    let reader = Async::<UdpSocket>::bind(socket_addr1).unwrap();
    let writer = Async::<UdpSocket>::bind(socket_addr2).unwrap();

    let mut buf = vec![0x42; UDP_AMOUNT];

    for (driver_name, exec) in [("Undriven", false), ("Driven", true)] {
        group.bench_function(format!("UdpSocket.{driver_name}"), |b| {
            b.iter(|| {
                let buf = &mut buf;

                block_on(
                    async {
                        black_box(writer.send_to(&*buf, socket_addr1).await.ok());
                        black_box(reader.recv_from(buf).await.ok());
                    },
                    exec,
                );
            });
        });
    }
}

criterion_group! {
    io_benchmarks,
    read_and_write,
    connect_and_accept,
    udp_send_recv
}

criterion_main!(io_benchmarks);
