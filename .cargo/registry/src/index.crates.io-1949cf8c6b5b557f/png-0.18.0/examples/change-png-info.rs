/// Tests "editing"/re-encoding of an image:
/// decoding, editing, re-encoding
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use png::DecodingError::LimitsExceeded;
pub type BoxResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn main() -> BoxResult<()> {
    // # Decode
    // Read test image from pngsuite
    let path_in = Path::new(r"./tests/pngsuite/basi0g01.png");
    // The decoder is a build for reader and can be used to set various decoding options
    // via `Transformations`. The default output transformation is `Transformations::IDENTITY`.
    let decoder = png::Decoder::new(BufReader::new(File::open(path_in)?));
    let mut reader = decoder.read_info()?;
    // Allocate the output buffer.
    let png_info = reader.info();
    let mut buf = vec![0; reader.output_buffer_size().ok_or(LimitsExceeded)?];
    println!("{png_info:?}");

    // # Encode
    let path_out = Path::new(r"./target/test_modified.png");
    let file = File::create(path_out)?;
    let ref mut w = BufWriter::new(file);

    // Get defaults for interlaced parameter.
    let mut info_out = png_info.clone();
    let info_default = png::Info::default();

    // Edit previous info
    info_out.interlaced = info_default.interlaced;
    let mut encoder = png::Encoder::with_info(w, info_out)?;
    encoder.set_depth(png_info.bit_depth);

    // Edit some attribute
    encoder.add_text_chunk(
        "Testing tEXt".to_string(),
        "This is a tEXt chunk that will appear before the IDAT chunks.".to_string(),
    )?;

    // Save picture with changed info
    let mut writer = encoder.write_header()?;
    let mut counter = 0u8;
    while let Ok(info) = reader.next_frame(&mut buf) {
        let bytes = &buf[..info.buffer_size()];
        writer.write_image_data(&bytes)?;
        counter += 1;
        println!("Written frame: {}", counter);
    }
    Ok(())
}
