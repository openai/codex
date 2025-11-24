extern crate tiny_http;

use std::io::{Cursor, Read, Write};
use std::sync::{
    atomic::{
        AtomicUsize,
        Ordering::{AcqRel, Acquire},
    },
    Arc,
};

#[allow(dead_code)]
mod support;

struct MeteredReader<T> {
    inner: T,
    position: Arc<AtomicUsize>,
}

impl<T> Read for MeteredReader<T>
where
    T: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.inner.read(buf) {
            Ok(read) => {
                self.position.fetch_add(read, AcqRel);
                Ok(read)
            }
            e => e,
        }
    }
}

type Reader = MeteredReader<Cursor<String>>;

fn big_response_reader() -> Reader {
    let big_body = "ABCDEFGHIJKLMNOPQRSTUVXYZ".repeat(1024 * 1024 * 16);
    MeteredReader {
        inner: Cursor::new(big_body),
        position: Arc::new(AtomicUsize::new(0)),
    }
}

fn identity_served(r: &mut Reader) -> tiny_http::Response<&mut Reader> {
    let body_len = r.inner.get_ref().len();
    tiny_http::Response::empty(200)
        .with_chunked_threshold(std::usize::MAX)
        .with_data(r, Some(body_len))
}

/// Checks that a body-Read:er is not called when the client has disconnected
#[test]
fn responding_to_closed_client() {
    let (server, mut stream) = support::new_one_server_one_client();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let request = server.recv().unwrap();

    // Client already disconnected
    drop(stream);

    let mut reader = big_response_reader();
    request
        .respond(identity_served(&mut reader))
        .expect("Successful");

    assert!(reader.position.load(Acquire) < 1024 * 1024);
}

/// Checks that a slow client does not cause data to be consumed and buffered from a reader
#[test]
fn responding_to_non_consuming_client() {
    let (server, mut stream) = support::new_one_server_one_client();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let request = server.recv().unwrap();

    let mut reader = big_response_reader();
    let position = reader.position.clone();

    // Client still connected, but not reading anything
    std::thread::spawn(move || {
        request
            .respond(identity_served(&mut reader))
            .expect("Successful");
    });

    std::thread::sleep(std::time::Duration::from_millis(100));

    // It seems the client TCP socket can buffer quite a lot, so we need to be permissive
    assert!(position.load(Acquire) < 8 * 1024 * 1024);

    drop(stream);
}
