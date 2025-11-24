use fax::{encoder::Encoder, slice_bits, tiff, ByteReader, Color, VecWriter};
use std::fs;

fn main() {
    let mut args = std::env::args().skip(1);
    let input: String = args.next().unwrap();
    let output = args.next().unwrap();

    let data = fs::read(&input).unwrap();
    let mut parts = data.splitn(3, |&b| b == b'\n');
    assert_eq!(parts.next().unwrap(), b"P4");
    let mut size = parts.next().unwrap().splitn(2, |&b| b == b' ');
    let width: u32 = std::str::from_utf8(size.next().unwrap()).unwrap().parse().unwrap();
    let height: u32 = std::str::from_utf8(size.next().unwrap()).unwrap().parse().unwrap();

    let writer = VecWriter::new();
    let mut encoder = Encoder::new(writer);
    
    for line in parts.next().unwrap().chunks((width as usize + 7) / 8).take(height as _) {
        let line = slice_bits(line).take(width as usize)
        .map(|b| match b {
            false => Color::White,
            true => Color::Black
        });
        encoder.encode_line(line, width as u16).unwrap();
    }
    let data = encoder.finish().unwrap().finish();

    fs::write(&output, &tiff::wrap(&data, width, height)).unwrap();
}
