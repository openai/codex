#![warn(rust_2018_idioms)]
#![cfg(feature = "std")]

use std::io::{BufRead, Read};

use bytes::Buf;

#[test]
fn read() {
    let buf1 = &b"hello "[..];
    let buf2 = &b"world"[..];
    let buf = Buf::chain(buf1, buf2); // Disambiguate with Read::chain
    let mut buffer = Vec::new();
    buf.reader().read_to_end(&mut buffer).unwrap();
    assert_eq!(b"hello world", &buffer[..]);
}

#[test]
fn buf_read() {
    let buf1 = &b"hell"[..];
    let buf2 = &b"o\nworld"[..];
    let mut reader = Buf::chain(buf1, buf2).reader();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!("hello\n", &line);
    line.clear();
    reader.read_line(&mut line).unwrap();
    assert_eq!("world", &line);
}
