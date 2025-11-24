use crate::test;
use std::io::Read;

use super::super::*;

#[test]
fn transfer_encoding_bogus() {
    test::set_handler("/transfer_encoding_bogus", |_unit| {
        test::make_response(
            200,
            "OK",
            vec![
                "transfer-encoding: bogus", // whatever it says here, we should chunk
            ],
            "3\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n\r\n"
                .to_string()
                .into_bytes(),
        )
    });
    let resp = get("test://host/transfer_encoding_bogus").call().unwrap();
    let mut reader = resp.into_reader();
    let mut text = String::new();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text, "hello world!!!");
}

#[test]
fn content_length_limited() {
    test::set_handler("/content_length_limited", |_unit| {
        test::make_response(
            200,
            "OK",
            vec!["Content-Length: 4"],
            "abcdefgh".to_string().into_bytes(),
        )
    });
    let resp = get("test://host/content_length_limited").call().unwrap();
    let mut reader = resp.into_reader();
    let mut text = String::new();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text, "abcd");
}

#[test]
// content-length should be ignored when chunked
fn ignore_content_length_when_chunked() {
    test::set_handler("/ignore_content_length_when_chunked", |_unit| {
        test::make_response(
            200,
            "OK",
            vec!["Content-Length: 4", "transfer-encoding: chunked"],
            "3\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n\r\n"
                .to_string()
                .into_bytes(),
        )
    });
    let resp = get("test://host/ignore_content_length_when_chunked")
        .call()
        .unwrap();
    let mut reader = resp.into_reader();
    let mut text = String::new();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text, "hello world!!!");
}

#[test]
fn no_reader_on_head() {
    test::set_handler("/no_reader_on_head", |_unit| {
        // so this is technically illegal, we return a body for the HEAD request.
        test::make_response(
            200,
            "OK",
            vec!["Content-Length: 4", "transfer-encoding: chunked"],
            "3\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n\r\n"
                .to_string()
                .into_bytes(),
        )
    });
    let resp = head("test://host/no_reader_on_head").call().unwrap();
    let mut reader = resp.into_reader();
    let mut text = String::new();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text, "");
}

#[cfg(feature = "gzip")]
#[test]
fn gzip_text() {
    test::set_handler("/gzip_text", |_unit| {
        test::make_response(
            200,
            "OK",
            vec!["content-length: 35", "content-encoding: gzip"],
            vec![
                0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x03, 0xCB, 0x48, 0xCD, 0xC9,
                0xC9, 0x57, 0x28, 0xCF, 0x2F, 0xCA, 0x49, 0x51, 0xC8, 0x18, 0xBC, 0x6C, 0x00, 0xA5,
                0x5C, 0x7C, 0xEF, 0xA7, 0x00, 0x00, 0x00,
            ],
        )
    });
    // echo -n INPUT | gzip -9 -f | hexdump -ve '1/1 "0x%.2X,"'
    // INPUT is `hello world ` repeated 14 times, no trailing space
    let resp = get("test://host/gzip_text").call().unwrap();
    assert_eq!(resp.header("content-encoding"), None);
    assert_eq!(resp.header("content-length"), None);
    let text = resp.into_string().unwrap();
    assert_eq!(text, "hello world ".repeat(14).trim_end());
}

#[cfg(feature = "brotli")]
#[test]
fn brotli_text() {
    test::set_handler("/brotli_text", |_unit| {
        test::make_response(
            200,
            "OK",
            vec!["content-length: 24", "content-encoding: br"],
            vec![
                0x1F, 0xA6, 0x00, 0xF8, 0x8D, 0x94, 0x6E, 0xDE, 0x44, 0x55, 0x86, 0x96, 0x20, 0x6C,
                0x6F, 0x35, 0x8B, 0x62, 0xB5, 0x40, 0x06, 0x54, 0xBB, 0x02,
            ],
        )
    });
    // echo -n INPUT | brotli -Z -f | hexdump -ve '1/1 "0x%.2X,"'
    let resp = get("test://host/brotli_text").call().unwrap();
    assert_eq!(resp.header("content-encoding"), None);
    assert_eq!(resp.header("content-length"), None);
    let text = resp.into_string().unwrap();
    assert_eq!(text, "hello world ".repeat(14).trim_end());
}
