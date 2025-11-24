use flate2::read::GzDecoder;
use flate2::read::MultiGzDecoder;
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, BufReader};
use std::path::Path;

// test extraction of a gzipped file
#[test]
fn test_extract_success() {
    let content = extract_file(Path::new("tests/good-file.gz")).unwrap();
    let mut expected = Vec::new();
    File::open("tests/good-file.txt")
        .unwrap()
        .read_to_end(&mut expected)
        .unwrap();
    assert_eq!(content, expected);
}
//
// test partial extraction of a multistream gzipped file
#[test]
fn test_extract_success_partial_multi() {
    let content = extract_file(Path::new("tests/multi.gz")).unwrap();
    let mut expected = String::new();
    BufReader::new(File::open("tests/multi.txt").unwrap())
        .read_line(&mut expected)
        .unwrap();
    assert_eq!(content, expected.as_bytes());
}

// test extraction fails on a corrupt file
#[test]
fn test_extract_failure() {
    let result = extract_file(Path::new("tests/corrupt-gz-file.bin"));
    assert_eq!(result.err().unwrap().kind(), io::ErrorKind::InvalidInput);
}

//test complete extraction of a multistream gzipped file
#[test]
fn test_extract_success_multi() {
    let content = extract_file_multi(Path::new("tests/multi.gz")).unwrap();
    let mut expected = Vec::new();
    File::open("tests/multi.txt")
        .unwrap()
        .read_to_end(&mut expected)
        .unwrap();
    assert_eq!(content, expected);
}

// Tries to extract path into memory (assuming a .gz file).
fn extract_file(path_compressed: &Path) -> io::Result<Vec<u8>> {
    let mut v = Vec::new();
    let f = File::open(path_compressed)?;
    GzDecoder::new(f).read_to_end(&mut v)?;
    Ok(v)
}

// Tries to extract path into memory (decompressing all members in case
// of a multi member .gz file).
fn extract_file_multi(path_compressed: &Path) -> io::Result<Vec<u8>> {
    let mut v = Vec::new();
    let f = File::open(path_compressed)?;
    MultiGzDecoder::new(f).read_to_end(&mut v)?;
    Ok(v)
}

#[test]
fn empty_error_once() {
    let data: &[u8] = &[];
    let cbjson = GzDecoder::new(data);
    let reader = BufReader::new(cbjson);
    let mut stream = reader.lines();
    assert!(stream.next().unwrap().is_err());
    assert!(stream.next().is_none());
}
