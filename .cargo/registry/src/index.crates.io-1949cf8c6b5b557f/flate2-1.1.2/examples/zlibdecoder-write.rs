use flate2::write::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io;
use std::io::prelude::*;

// Compress a sample string and print it after transformation.
fn main() {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(b"Hello World").unwrap();
    let bytes = e.finish().unwrap();
    println!("{}", decode_reader(bytes).unwrap());
}

// Uncompresses a Zlib Encoded vector of bytes and returns a string or error
// Here Vec<u8> implements Write
fn decode_reader(bytes: Vec<u8>) -> io::Result<String> {
    let mut writer = Vec::new();
    let mut z = ZlibDecoder::new(writer);
    z.write_all(&bytes[..])?;
    writer = z.finish()?;
    let return_string = String::from_utf8(writer).expect("String parsing error");
    Ok(return_string)
}
