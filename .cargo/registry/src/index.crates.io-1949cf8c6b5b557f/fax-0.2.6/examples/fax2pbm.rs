use fax::{VecWriter, decoder, decoder::pels, BitWriter, Bits, Color};
use std::io::Write;
use std::fs::{self, File};

fn main() {
    let mut args = std::env::args().skip(1);
    let input: String = args.next().unwrap();
    let width: u16 = args.next().unwrap().parse().unwrap();
    let output = args.next().unwrap();

    let data = fs::read(&input).unwrap();
    let mut writer = VecWriter::new();
    let mut height = 0;
    decoder::decode_g4(data.iter().cloned(), width, None,  |transitions| {
        for c in pels(transitions, width) {
            let bit = match c {
                Color::Black => Bits { data: 1, len: 1 },
                Color::White => Bits { data: 0, len: 1 }
            };
            writer.write(bit);
        }
        writer.pad();
        height += 1;
    });
    let data = writer.finish();
    assert_eq!(data.len(), height as usize * ((width as usize + 7) / 8));

    let header = format!("P4\n{} {}\n", width, height);
    let mut out = File::create(&output).unwrap();
    out.write_all(header.as_bytes()).unwrap();
    out.write_all(&data).unwrap();
}