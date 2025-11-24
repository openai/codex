use std::io;
use std::{env, fs};
use weezl::{decode, encode, BitOrder};

#[derive(Clone, Copy, Debug)]
enum Flavor {
    Gif,
    Tiff,
}

#[test]
fn roundtrip_all_lsb_tiny() {
    roundtrip_all(BitOrder::Lsb, 1);
}

#[test]
fn roundtrip_all_msb_tiny() {
    roundtrip_all(BitOrder::Msb, 1);
}

#[test]
fn roundtrip_all_lsb() {
    roundtrip_all(BitOrder::Lsb, 1 << 20);
}

#[test]
fn roundtrip_all_msb() {
    roundtrip_all(BitOrder::Msb, 1 << 20);
}

fn roundtrip_all(bit_order: BitOrder, max_io_len: usize) {
    let file = env::args().next().unwrap();
    let data = fs::read(file).unwrap();

    for &flavor in &[Flavor::Gif, Flavor::Tiff] {
        for bit_width in 2..8 {
            let data: Vec<_> = data
                .iter()
                .copied()
                .map(|b| b & ((1 << bit_width) - 1))
                .collect();

            let enc = match flavor {
                Flavor::Gif => encode::Configuration::new,
                Flavor::Tiff => encode::Configuration::with_tiff_size_switch,
            }(bit_order, bit_width);

            let dec = match flavor {
                Flavor::Gif => decode::Configuration::new,
                Flavor::Tiff => decode::Configuration::with_tiff_size_switch,
            }(bit_order, bit_width);

            let yielding = dec.clone().with_yield_on_full_buffer(true);

            println!("Roundtrip test {:?} {:?} {}", flavor, bit_order, bit_width);
            assert_roundtrips(&*data, enc.clone(), dec, max_io_len);

            // Our encoder always passes an enclosed stream. So this must be the same.
            assert_roundtrips(&*data, enc, yielding, max_io_len);
        }
    }
}

fn assert_roundtrips(
    data: &[u8],
    enc: encode::Configuration,
    dec: decode::Configuration,
    max_io_len: usize,
) {
    let mut encoder = enc.clone().build();
    let mut writer = TinyWrite {
        data: Vec::with_capacity(2 * data.len() + 40),
        max_write_len: max_io_len,
    };
    let _ = encoder.into_stream(&mut writer).encode_all(data);

    let mut decoder = dec.clone().build();
    let mut compare = vec![];

    let buf_reader = TinyRead {
        data: &writer.data,
        max_read_len: max_io_len,
    };
    let result = decoder.into_stream(&mut compare).decode_all(buf_reader);
    assert!(result.status.is_ok(), "{:?}, {:?}", dec, result.status);
    assert!(data == &*compare, "{:?}\n{:?}\n{:?}", dec, data, compare);
}

struct TinyRead<'a> {
    data: &'a [u8],
    max_read_len: usize,
}

impl io::BufRead for TinyRead<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Ok(&self.data[..self.data.len().min(self.max_read_len)])
    }
    fn consume(&mut self, n: usize) {
        debug_assert!(n <= self.max_read_len);
        self.data = &self.data[n..];
    }
}

impl io::Read for TinyRead<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.data.len().min(buf.len()).min(self.max_read_len);
        buf[..n].copy_from_slice(&self.data[..n]);
        self.data = &self.data[n..];
        Ok(n)
    }
}

struct TinyWrite {
    data: Vec<u8>,
    max_write_len: usize,
}

impl io::Write for TinyWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = buf.len().min(self.max_write_len);
        self.data.extend_from_slice(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
