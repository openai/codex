use flate2::Compression;
use flate2::GzBuilder;
use std::fs::File;
use std::io;
use std::io::prelude::*;

// Compresses content of a text file into a gzip file
fn main() {
    sample_builder().unwrap();
}

// GzBuilder opens a file and writes a sample string using Builder pattern
fn sample_builder() -> Result<(), io::Error> {
    let f = File::create("examples/hello_world.txt.gz")?;
    let mut gz = GzBuilder::new()
        .filename("hello_world.txt")
        .comment("test file, please delete")
        .write(f, Compression::default());
    gz.write_all(b"hello world")?;
    gz.finish()?;
    Ok(())
}
