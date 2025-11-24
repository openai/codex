use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::prelude::*;

// Vec<u8> implements Write to print the compressed bytes of sample string
fn main() {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(b"Hello World").unwrap();
    println!("{:?}", e.finish().unwrap());
}
