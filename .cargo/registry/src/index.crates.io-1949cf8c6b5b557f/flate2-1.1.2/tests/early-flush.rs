use std::io::{Read, Write};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

#[test]
fn smoke() {
    let mut w = GzEncoder::new(Vec::new(), flate2::Compression::default());
    w.flush().unwrap();
    w.write_all(b"hello").unwrap();

    let bytes = w.finish().unwrap();

    let mut r = GzDecoder::new(&bytes[..]);
    let mut s = String::new();
    r.read_to_string(&mut s).unwrap();
    assert_eq!(s, "hello");
}
