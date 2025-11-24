#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use std::io::ErrorKind;
use tokio::io::{AsyncBufReadExt, BufReader, Error};
use tokio_test::{assert_ok, io::Builder};

use std::io::Cursor;

#[tokio::test]
async fn read_line() {
    let mut buf = String::new();
    let mut rd = Cursor::new(b"hello\nworld\n\n");

    let n = assert_ok!(rd.read_line(&mut buf).await);
    assert_eq!(n, 6);
    assert_eq!(buf, "hello\n");
    buf.clear();
    let n = assert_ok!(rd.read_line(&mut buf).await);
    assert_eq!(n, 6);
    assert_eq!(buf, "world\n");
    buf.clear();
    let n = assert_ok!(rd.read_line(&mut buf).await);
    assert_eq!(n, 1);
    assert_eq!(buf, "\n");
    buf.clear();
    let n = assert_ok!(rd.read_line(&mut buf).await);
    assert_eq!(n, 0);
    assert_eq!(buf, "");
}

#[tokio::test]
async fn read_line_not_all_ready() {
    let mock = Builder::new()
        .read(b"Hello Wor")
        .read(b"ld\nFizzBuz")
        .read(b"z\n1\n2")
        .build();

    let mut read = BufReader::new(mock);

    let mut line = "We say ".to_string();
    let bytes = read.read_line(&mut line).await.unwrap();
    assert_eq!(bytes, "Hello World\n".len());
    assert_eq!(line.as_str(), "We say Hello World\n");

    line = "I solve ".to_string();
    let bytes = read.read_line(&mut line).await.unwrap();
    assert_eq!(bytes, "FizzBuzz\n".len());
    assert_eq!(line.as_str(), "I solve FizzBuzz\n");

    line.clear();
    let bytes = read.read_line(&mut line).await.unwrap();
    assert_eq!(bytes, 2);
    assert_eq!(line.as_str(), "1\n");

    line.clear();
    let bytes = read.read_line(&mut line).await.unwrap();
    assert_eq!(bytes, 1);
    assert_eq!(line.as_str(), "2");
}

#[tokio::test]
async fn read_line_invalid_utf8() {
    let mock = Builder::new().read(b"Hello Wor\xffld.\n").build();

    let mut read = BufReader::new(mock);

    let mut line = "Foo".to_string();
    let err = read.read_line(&mut line).await.expect_err("Should fail");
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert_eq!(err.to_string(), "stream did not contain valid UTF-8");
    assert_eq!(line.as_str(), "Foo");
}

#[tokio::test]
async fn read_line_fail() {
    let mock = Builder::new()
        .read(b"Hello Wor")
        .read_error(Error::new(ErrorKind::Other, "The world has no end"))
        .build();

    let mut read = BufReader::new(mock);

    let mut line = "Foo".to_string();
    let err = read.read_line(&mut line).await.expect_err("Should fail");
    assert_eq!(err.kind(), ErrorKind::Other);
    assert_eq!(err.to_string(), "The world has no end");
    assert_eq!(line.as_str(), "FooHello Wor");
}

#[tokio::test]
async fn read_line_fail_and_utf8_fail() {
    let mock = Builder::new()
        .read(b"Hello Wor")
        .read(b"\xff\xff\xff")
        .read_error(Error::new(ErrorKind::Other, "The world has no end"))
        .build();

    let mut read = BufReader::new(mock);

    let mut line = "Foo".to_string();
    let err = read.read_line(&mut line).await.expect_err("Should fail");
    assert_eq!(err.kind(), ErrorKind::Other);
    assert_eq!(err.to_string(), "The world has no end");
    assert_eq!(line.as_str(), "Foo");
}
