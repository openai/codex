use flate2::write::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io;
use std::io::prelude::*;

// Compress a sample string and print it after transformation.
fn main() {
    let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
    e.write_all(b"Hello World").unwrap();
    let bytes = e.finish().unwrap();
    println!("{}", decode_reader(bytes).unwrap());
}

// Uncompresses a Deflate Encoded vector of bytes and returns a string or error
// Here Vec<u8> implements Write
fn decode_reader(bytes: Vec<u8>) -> io::Result<String> {
    let mut writer = Vec::new();
    let mut deflater = DeflateDecoder::new(writer);
    deflater.write_all(&bytes[..])?;
    writer = deflater.finish()?;
    let return_string = String::from_utf8(writer).expect("String parsing error");
    Ok(return_string)
}
