extern crate tiff;

use tiff::decoder::Decoder;
use tiff::TiffResult;

use std::fs::File;

fn test_directory<F: Fn(File) -> bool>(path: &str, f: F) {
    for entry in std::fs::read_dir(path).unwrap() {
        let file = File::open(entry.unwrap().path()).unwrap();
        assert!(f(file));
    }
}

fn decode_tiff(file: File) -> TiffResult<()> {
    let mut decoder = Decoder::new(file)?;
    decoder.read_image()?;
    Ok(())
}

#[test]
fn oor_panic() {
    test_directory("./tests/fuzz_images/oor_panic", |file| {
        let _ = decode_tiff(file);
        true
    });
}

#[test]
fn oom_crash() {
    test_directory("./tests/fuzz_images/oom_crash", |file| {
        decode_tiff(file).is_err()
    });
}

#[test]
fn inf_loop() {
    test_directory("./tests/fuzz_images/inf_loop", |file| {
        let _ = decode_tiff(file);
        true
    });
}

// https://github.com/image-rs/image-tiff/issues/33
#[test]
fn divide_by_zero() {
    test_directory("./tests/fuzz_images/divide_by_zero", |file| {
        let _ = decode_tiff(file);
        true
    });
}
