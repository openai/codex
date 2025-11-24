#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use std::io::ErrorKind;
use tokio::io::{AsyncBufReadExt, BufReader, Error};
use tokio_test::{assert_ok, io::Builder};

#[tokio::test]
async fn read_until() {
    let mut buf = vec![];
    let mut rd: &[u8] = b"hello world";

    let n = assert_ok!(rd.read_until(b' ', &mut buf).await);
    assert_eq!(n, 6);
    assert_eq!(buf, b"hello ");
    buf.clear();
    let n = assert_ok!(rd.read_until(b' ', &mut buf).await);
    assert_eq!(n, 5);
    assert_eq!(buf, b"world");
    buf.clear();
    let n = assert_ok!(rd.read_until(b' ', &mut buf).await);
    assert_eq!(n, 0);
    assert_eq!(buf, []);
}

#[tokio::test]
async fn read_until_not_all_ready() {
    let mock = Builder::new()
        .read(b"Hello Wor")
        .read(b"ld#Fizz\xffBuz")
        .read(b"z#1#2")
        .build();

    let mut read = BufReader::new(mock);

    let mut chunk = b"We say ".to_vec();
    let bytes = read.read_until(b'#', &mut chunk).await.unwrap();
    assert_eq!(bytes, b"Hello World#".len());
    assert_eq!(chunk, b"We say Hello World#");

    chunk = b"I solve ".to_vec();
    let bytes = read.read_until(b'#', &mut chunk).await.unwrap();
    assert_eq!(bytes, b"Fizz\xffBuzz\n".len());
    assert_eq!(chunk, b"I solve Fizz\xffBuzz#");

    chunk.clear();
    let bytes = read.read_until(b'#', &mut chunk).await.unwrap();
    assert_eq!(bytes, 2);
    assert_eq!(chunk, b"1#");

    chunk.clear();
    let bytes = read.read_until(b'#', &mut chunk).await.unwrap();
    assert_eq!(bytes, 1);
    assert_eq!(chunk, b"2");
}

#[tokio::test]
async fn read_until_fail() {
    let mock = Builder::new()
        .read(b"Hello \xffWor")
        .read_error(Error::new(ErrorKind::Other, "The world has no end"))
        .build();

    let mut read = BufReader::new(mock);

    let mut chunk = b"Foo".to_vec();
    let err = read
        .read_until(b'#', &mut chunk)
        .await
        .expect_err("Should fail");
    assert_eq!(err.kind(), ErrorKind::Other);
    assert_eq!(err.to_string(), "The world has no end");
    assert_eq!(chunk, b"FooHello \xffWor");
}
