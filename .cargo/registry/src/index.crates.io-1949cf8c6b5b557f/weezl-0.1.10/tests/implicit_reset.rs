use std::{env, fs};
use weezl::{decode, encode, BitOrder};

#[test]
fn read_from_mangled() {
    let file = env::args().next().unwrap();
    let data = fs::read(file).unwrap();

    // For simplicity, encode 7-bit data.
    let data: Vec<_> = data.iter().copied().map(|b| b & 0x7f).collect();

    let mut encoder = encode::Encoder::new(BitOrder::Lsb, 7);
    let mut buffer = Vec::with_capacity(2 * data.len() + 40);
    let _ = encoder.into_stream(&mut buffer).encode_all(&*data);

    let mut decoder = decode::Decoder::new(BitOrder::Lsb, 7);
    let mut compare = vec![];
    let result = decoder.into_stream(&mut compare).decode_all(&buffer[1..]);
    assert!(result.status.is_ok(), "{:?}", result.status);
    assert!(data == &*compare, "{:?}\n{:?}", data, compare);
}
