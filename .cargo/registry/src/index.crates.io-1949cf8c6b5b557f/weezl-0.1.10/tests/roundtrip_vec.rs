use std::{env, fs};
use weezl::{decode, encode, BitOrder};

#[derive(Clone, Copy, Debug)]
enum Flavor {
    Gif,
    Tiff,
}

#[test]
fn roundtrip_all_lsb() {
    roundtrip_all(BitOrder::Lsb);
}

#[test]
fn roundtrip_all_msb() {
    roundtrip_all(BitOrder::Msb);
}

fn roundtrip_all(bit_order: BitOrder) {
    let file = env::args().next().unwrap();
    let data = fs::read(file).unwrap();

    for &flavor in &[Flavor::Gif, Flavor::Tiff] {
        for bit_width in 2..8 {
            let data: Vec<_> = data
                .iter()
                .copied()
                .map(|b| b & ((1 << bit_width) - 1))
                .collect();

            println!("Roundtrip test {:?} {:?} {}", flavor, bit_order, bit_width);
            assert_roundtrips(&*data, flavor, bit_width, bit_order);
        }
    }
}

fn assert_roundtrips(data: &[u8], flavor: Flavor, bit_width: u8, bit_order: BitOrder) {
    let (c, d): (
        fn(BitOrder, u8) -> encode::Encoder,
        fn(BitOrder, u8) -> decode::Decoder,
    ) = match flavor {
        Flavor::Gif => (encode::Encoder::new, decode::Decoder::new),
        Flavor::Tiff => (
            encode::Encoder::with_tiff_size_switch,
            decode::Decoder::with_tiff_size_switch,
        ),
    };
    let mut encoder = c(bit_order, bit_width);
    let mut buffer = Vec::with_capacity(2 * data.len() + 40);

    let _ = encoder.into_vec(&mut buffer).encode_all(data);

    let mut decoder = d(bit_order, bit_width);
    let mut compare = vec![];
    let result = decoder.into_vec(&mut compare).decode_all(buffer.as_slice());
    assert!(
        result.status.is_ok(),
        "{:?}, {}, {:?}",
        bit_order,
        bit_width,
        result.status
    );
    assert!(
        data == &*compare,
        "{:?}, {}\n{:?}\n{:?}",
        bit_order,
        bit_width,
        data,
        compare
    );
}
