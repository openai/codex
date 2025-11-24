use std::io::{Read, Write};

#[test]
fn deflate_decoder_empty_read() {
    let original: &[u8] = b"Lorem ipsum dolor sit amet.";
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(original).unwrap();
    let encoded: Vec<u8> = encoder.finish().unwrap();
    let mut decoder = flate2::read::DeflateDecoder::new(encoded.as_slice());
    assert_eq!(decoder.read(&mut []).unwrap(), 0);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).unwrap();
    assert_eq!(decoded.as_slice(), original);
}

#[test]
fn deflate_encoder_empty_read() {
    let original: &[u8] = b"Lorem ipsum dolor sit amet.";
    let mut encoder = flate2::read::DeflateEncoder::new(original, flate2::Compression::default());
    assert_eq!(encoder.read(&mut []).unwrap(), 0);
    let mut encoded = Vec::new();
    encoder.read_to_end(&mut encoded).unwrap();
    let mut decoder = flate2::read::DeflateDecoder::new(encoded.as_slice());
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).unwrap();
    assert_eq!(decoded.as_slice(), original);
}

#[test]
fn gzip_decoder_empty_read() {
    let original: &[u8] = b"Lorem ipsum dolor sit amet.";
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(original).unwrap();
    let encoded: Vec<u8> = encoder.finish().unwrap();
    let mut decoder = flate2::read::GzDecoder::new(encoded.as_slice());
    assert_eq!(decoder.read(&mut []).unwrap(), 0);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).unwrap();
    assert_eq!(decoded.as_slice(), original);
}

#[test]
fn gzip_encoder_empty_read() {
    let original: &[u8] = b"Lorem ipsum dolor sit amet.";
    let mut encoder = flate2::read::GzEncoder::new(original, flate2::Compression::default());
    assert_eq!(encoder.read(&mut []).unwrap(), 0);
    let mut encoded = Vec::new();
    encoder.read_to_end(&mut encoded).unwrap();
    let mut decoder = flate2::read::GzDecoder::new(encoded.as_slice());
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).unwrap();
    assert_eq!(decoded.as_slice(), original);
}

#[test]
fn zlib_decoder_empty_read() {
    let original: &[u8] = b"Lorem ipsum dolor sit amet.";
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(original).unwrap();
    let encoded: Vec<u8> = encoder.finish().unwrap();
    let mut decoder = flate2::read::ZlibDecoder::new(encoded.as_slice());
    assert_eq!(decoder.read(&mut []).unwrap(), 0);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).unwrap();
    assert_eq!(decoded.as_slice(), original);
}

#[test]
fn zlib_encoder_empty_read() {
    let original: &[u8] = b"Lorem ipsum dolor sit amet.";
    let mut encoder = flate2::read::ZlibEncoder::new(original, flate2::Compression::default());
    assert_eq!(encoder.read(&mut []).unwrap(), 0);
    let mut encoded = Vec::new();
    encoder.read_to_end(&mut encoded).unwrap();
    let mut decoder = flate2::read::ZlibDecoder::new(encoded.as_slice());
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).unwrap();
    assert_eq!(decoded.as_slice(), original);
}
