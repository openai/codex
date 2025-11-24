use fax::{encoder::Encoder, slice_bits, slice_reader, BitReader, BitWriter, Bits, ByteReader, Color};
use std::{convert::Infallible, fs};

fn main() {
    let mut args = std::env::args().skip(1);
    let input: String = args.next().unwrap();
    let output = args.next().unwrap();

    let data = fs::read(&input).unwrap();
    let reference_data = fs::read(&output).unwrap();
    let mut parts = data.splitn(3, |&b| b == b'\n');
    assert_eq!(parts.next().unwrap(), b"P4");
    let mut size = parts.next().unwrap().splitn(2, |&b| b == b' ');
    let width: u16 = std::str::from_utf8(size.next().unwrap()).unwrap().parse().unwrap();
    let height: u16 = std::str::from_utf8(size.next().unwrap()).unwrap().parse().unwrap();

    //let writer = VecWriter::new();
    let writer = Validator { reader: slice_reader(&reference_data) };
    let mut encoder = Encoder::new(writer);
    
    for (y, line) in parts.next().unwrap().chunks((width as usize + 7) / 8).enumerate().take(height as _) {
        println!("\nline {}", y);
        let line = slice_bits(line).take(width as usize)
        .map(|b| match b {
            false => Color::Black,
            true => Color::White
        });
        encoder.encode_line(line, width).unwrap();
    }
    let mut writer = encoder.finish().unwrap();
    writer.reader.print_remaining();
    

    //let (data, _) = encoder.into_writer().finish();
    //fs::write(&output, &data).unwrap();
}

struct Validator<R> {
    reader: ByteReader<R>
}
impl<R> BitWriter for Validator<R> 
where ByteReader<R>: BitReader
{
    type Error = ();
    fn write(&mut self, bits: Bits) -> Result<(), ()> {
        let expected = Bits { data: self.reader.peek(bits.len).unwrap(), len: bits.len };
        if expected != bits {
            println!("{} != {}", expected, bits);
            return Err(());
        }
        self.reader.consume(bits.len);
        Ok(())
    }
}