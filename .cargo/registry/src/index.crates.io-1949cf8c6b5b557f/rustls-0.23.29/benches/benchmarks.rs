#![cfg(feature = "ring")]
#![allow(clippy::disallowed_types)]

use std::io;
use std::sync::Arc;

use bencher::{Bencher, benchmark_group, benchmark_main};
use rustls::ServerConnection;
use rustls::crypto::ring as provider;
use rustls_test::{FailsReads, KeyType, make_server_config};

fn bench_ewouldblock(c: &mut Bencher) {
    let server_config = make_server_config(KeyType::Rsa2048, &provider::default_provider());
    let mut server = ServerConnection::new(Arc::new(server_config)).unwrap();
    let mut read_ewouldblock = FailsReads::new(io::ErrorKind::WouldBlock);
    c.iter(|| server.read_tls(&mut read_ewouldblock));
}

benchmark_group!(benches, bench_ewouldblock);
benchmark_main!(benches);
