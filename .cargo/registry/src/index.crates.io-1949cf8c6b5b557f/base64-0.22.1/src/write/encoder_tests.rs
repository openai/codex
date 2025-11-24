use std::io::{Cursor, Write};
use std::{cmp, io, str};

use rand::Rng;

use crate::{
    alphabet::{STANDARD, URL_SAFE},
    engine::{
        general_purpose::{GeneralPurpose, NO_PAD, PAD},
        Engine,
    },
    tests::random_engine,
};

use super::EncoderWriter;

const URL_SAFE_ENGINE: GeneralPurpose = GeneralPurpose::new(&URL_SAFE, PAD);
const NO_PAD_ENGINE: GeneralPurpose = GeneralPurpose::new(&STANDARD, NO_PAD);

#[test]
fn encode_three_bytes() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &URL_SAFE_ENGINE);

        let sz = enc.write(b"abc").unwrap();
        assert_eq!(sz, 3);
    }
    assert_eq!(&c.get_ref()[..], URL_SAFE_ENGINE.encode("abc").as_bytes());
}

#[test]
fn encode_nine_bytes_two_writes() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &URL_SAFE_ENGINE);

        let sz = enc.write(b"abcdef").unwrap();
        assert_eq!(sz, 6);
        let sz = enc.write(b"ghi").unwrap();
        assert_eq!(sz, 3);
    }
    assert_eq!(
        &c.get_ref()[..],
        URL_SAFE_ENGINE.encode("abcdefghi").as_bytes()
    );
}

#[test]
fn encode_one_then_two_bytes() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &URL_SAFE_ENGINE);

        let sz = enc.write(b"a").unwrap();
        assert_eq!(sz, 1);
        let sz = enc.write(b"bc").unwrap();
        assert_eq!(sz, 2);
    }
    assert_eq!(&c.get_ref()[..], URL_SAFE_ENGINE.encode("abc").as_bytes());
}

#[test]
fn encode_one_then_five_bytes() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &URL_SAFE_ENGINE);

        let sz = enc.write(b"a").unwrap();
        assert_eq!(sz, 1);
        let sz = enc.write(b"bcdef").unwrap();
        assert_eq!(sz, 5);
    }
    assert_eq!(
        &c.get_ref()[..],
        URL_SAFE_ENGINE.encode("abcdef").as_bytes()
    );
}

#[test]
fn encode_1_2_3_bytes() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &URL_SAFE_ENGINE);

        let sz = enc.write(b"a").unwrap();
        assert_eq!(sz, 1);
        let sz = enc.write(b"bc").unwrap();
        assert_eq!(sz, 2);
        let sz = enc.write(b"def").unwrap();
        assert_eq!(sz, 3);
    }
    assert_eq!(
        &c.get_ref()[..],
        URL_SAFE_ENGINE.encode("abcdef").as_bytes()
    );
}

#[test]
fn encode_with_padding() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &URL_SAFE_ENGINE);

        enc.write_all(b"abcd").unwrap();

        enc.flush().unwrap();
    }
    assert_eq!(&c.get_ref()[..], URL_SAFE_ENGINE.encode("abcd").as_bytes());
}

#[test]
fn encode_with_padding_multiple_writes() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &URL_SAFE_ENGINE);

        assert_eq!(1, enc.write(b"a").unwrap());
        assert_eq!(2, enc.write(b"bc").unwrap());
        assert_eq!(3, enc.write(b"def").unwrap());
        assert_eq!(1, enc.write(b"g").unwrap());

        enc.flush().unwrap();
    }
    assert_eq!(
        &c.get_ref()[..],
        URL_SAFE_ENGINE.encode("abcdefg").as_bytes()
    );
}

#[test]
fn finish_writes_extra_byte() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &URL_SAFE_ENGINE);

        assert_eq!(6, enc.write(b"abcdef").unwrap());

        // will be in extra
        assert_eq!(1, enc.write(b"g").unwrap());

        // 1 trailing byte = 2 encoded chars
        let _ = enc.finish().unwrap();
    }
    assert_eq!(
        &c.get_ref()[..],
        URL_SAFE_ENGINE.encode("abcdefg").as_bytes()
    );
}

#[test]
fn write_partial_chunk_encodes_partial_chunk() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &NO_PAD_ENGINE);

        // nothing encoded yet
        assert_eq!(2, enc.write(b"ab").unwrap());
        // encoded here
        let _ = enc.finish().unwrap();
    }
    assert_eq!(&c.get_ref()[..], NO_PAD_ENGINE.encode("ab").as_bytes());
    assert_eq!(3, c.get_ref().len());
}

#[test]
fn write_1_chunk_encodes_complete_chunk() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &NO_PAD_ENGINE);

        assert_eq!(3, enc.write(b"abc").unwrap());
        let _ = enc.finish().unwrap();
    }
    assert_eq!(&c.get_ref()[..], NO_PAD_ENGINE.encode("abc").as_bytes());
    assert_eq!(4, c.get_ref().len());
}

#[test]
fn write_1_chunk_and_partial_encodes_only_complete_chunk() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &NO_PAD_ENGINE);

        // "d" not consumed since it's not a full chunk
        assert_eq!(3, enc.write(b"abcd").unwrap());
        let _ = enc.finish().unwrap();
    }
    assert_eq!(&c.get_ref()[..], NO_PAD_ENGINE.encode("abc").as_bytes());
    assert_eq!(4, c.get_ref().len());
}

#[test]
fn write_2_partials_to_exactly_complete_chunk_encodes_complete_chunk() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &NO_PAD_ENGINE);

        assert_eq!(1, enc.write(b"a").unwrap());
        assert_eq!(2, enc.write(b"bc").unwrap());
        let _ = enc.finish().unwrap();
    }
    assert_eq!(&c.get_ref()[..], NO_PAD_ENGINE.encode("abc").as_bytes());
    assert_eq!(4, c.get_ref().len());
}

#[test]
fn write_partial_then_enough_to_complete_chunk_but_not_complete_another_chunk_encodes_complete_chunk_without_consuming_remaining(
) {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &NO_PAD_ENGINE);

        assert_eq!(1, enc.write(b"a").unwrap());
        // doesn't consume "d"
        assert_eq!(2, enc.write(b"bcd").unwrap());
        let _ = enc.finish().unwrap();
    }
    assert_eq!(&c.get_ref()[..], NO_PAD_ENGINE.encode("abc").as_bytes());
    assert_eq!(4, c.get_ref().len());
}

#[test]
fn write_partial_then_enough_to_complete_chunk_and_another_chunk_encodes_complete_chunks() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &NO_PAD_ENGINE);

        assert_eq!(1, enc.write(b"a").unwrap());
        // completes partial chunk, and another chunk
        assert_eq!(5, enc.write(b"bcdef").unwrap());
        let _ = enc.finish().unwrap();
    }
    assert_eq!(&c.get_ref()[..], NO_PAD_ENGINE.encode("abcdef").as_bytes());
    assert_eq!(8, c.get_ref().len());
}

#[test]
fn write_partial_then_enough_to_complete_chunk_and_another_chunk_and_another_partial_chunk_encodes_only_complete_chunks(
) {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &NO_PAD_ENGINE);

        assert_eq!(1, enc.write(b"a").unwrap());
        // completes partial chunk, and another chunk, with one more partial chunk that's not
        // consumed
        assert_eq!(5, enc.write(b"bcdefe").unwrap());
        let _ = enc.finish().unwrap();
    }
    assert_eq!(&c.get_ref()[..], NO_PAD_ENGINE.encode("abcdef").as_bytes());
    assert_eq!(8, c.get_ref().len());
}

#[test]
fn drop_calls_finish_for_you() {
    let mut c = Cursor::new(Vec::new());
    {
        let mut enc = EncoderWriter::new(&mut c, &NO_PAD_ENGINE);
        assert_eq!(1, enc.write(b"a").unwrap());
    }
    assert_eq!(&c.get_ref()[..], NO_PAD_ENGINE.encode("a").as_bytes());
    assert_eq!(2, c.get_ref().len());
}

#[test]
fn every_possible_split_of_input() {
    let mut rng = rand::thread_rng();
    let mut orig_data = Vec::<u8>::new();
    let mut stream_encoded = Vec::<u8>::new();
    let mut normal_encoded = String::new();

    let size = 5_000;

    for i in 0..size {
        orig_data.clear();
        stream_encoded.clear();
        normal_encoded.clear();

        for _ in 0..size {
            orig_data.push(rng.gen());
        }

        let engine = random_engine(&mut rng);
        engine.encode_string(&orig_data, &mut normal_encoded);

        {
            let mut stream_encoder = EncoderWriter::new(&mut stream_encoded, &engine);
            // Write the first i bytes, then the rest
            stream_encoder.write_all(&orig_data[0..i]).unwrap();
            stream_encoder.write_all(&orig_data[i..]).unwrap();
        }

        assert_eq!(normal_encoded, str::from_utf8(&stream_encoded).unwrap());
    }
}

#[test]
fn encode_random_config_matches_normal_encode_reasonable_input_len() {
    // choose up to 2 * buf size, so ~half the time it'll use a full buffer
    do_encode_random_config_matches_normal_encode(super::encoder::BUF_SIZE * 2);
}

#[test]
fn encode_random_config_matches_normal_encode_tiny_input_len() {
    do_encode_random_config_matches_normal_encode(10);
}

#[test]
fn retrying_writes_that_error_with_interrupted_works() {
    let mut rng = rand::thread_rng();
    let mut orig_data = Vec::<u8>::new();
    let mut stream_encoded = Vec::<u8>::new();
    let mut normal_encoded = String::new();

    for _ in 0..1_000 {
        orig_data.clear();
        stream_encoded.clear();
        normal_encoded.clear();

        let orig_len: usize = rng.gen_range(100..20_000);
        for _ in 0..orig_len {
            orig_data.push(rng.gen());
        }

        // encode the normal way
        let engine = random_engine(&mut rng);
        engine.encode_string(&orig_data, &mut normal_encoded);

        // encode via the stream encoder
        {
            let mut interrupt_rng = rand::thread_rng();
            let mut interrupting_writer = InterruptingWriter {
                w: &mut stream_encoded,
                rng: &mut interrupt_rng,
                fraction: 0.8,
            };

            let mut stream_encoder = EncoderWriter::new(&mut interrupting_writer, &engine);
            let mut bytes_consumed = 0;
            while bytes_consumed < orig_len {
                // use short inputs since we want to use `extra` a lot as that's what needs rollback
                // when errors occur
                let input_len: usize = cmp::min(rng.gen_range(0..10), orig_len - bytes_consumed);

                retry_interrupted_write_all(
                    &mut stream_encoder,
                    &orig_data[bytes_consumed..bytes_consumed + input_len],
                )
                .unwrap();

                bytes_consumed += input_len;
            }

            loop {
                let res = stream_encoder.finish();
                match res {
                    Ok(_) => break,
                    Err(e) => match e.kind() {
                        io::ErrorKind::Interrupted => continue,
                        _ => panic!("{:?}", e), // bail
                    },
                }
            }

            assert_eq!(orig_len, bytes_consumed);
        }

        assert_eq!(normal_encoded, str::from_utf8(&stream_encoded).unwrap());
    }
}

#[test]
fn writes_that_only_write_part_of_input_and_sometimes_interrupt_produce_correct_encoded_data() {
    let mut rng = rand::thread_rng();
    let mut orig_data = Vec::<u8>::new();
    let mut stream_encoded = Vec::<u8>::new();
    let mut normal_encoded = String::new();

    for _ in 0..1_000 {
        orig_data.clear();
        stream_encoded.clear();
        normal_encoded.clear();

        let orig_len: usize = rng.gen_range(100..20_000);
        for _ in 0..orig_len {
            orig_data.push(rng.gen());
        }

        // encode the normal way
        let engine = random_engine(&mut rng);
        engine.encode_string(&orig_data, &mut normal_encoded);

        // encode via the stream encoder
        {
            let mut partial_rng = rand::thread_rng();
            let mut partial_writer = PartialInterruptingWriter {
                w: &mut stream_encoded,
                rng: &mut partial_rng,
                full_input_fraction: 0.1,
                no_interrupt_fraction: 0.1,
            };

            let mut stream_encoder = EncoderWriter::new(&mut partial_writer, &engine);
            let mut bytes_consumed = 0;
            while bytes_consumed < orig_len {
                // use at most medium-length inputs to exercise retry logic more aggressively
                let input_len: usize = cmp::min(rng.gen_range(0..100), orig_len - bytes_consumed);

                let res =
                    stream_encoder.write(&orig_data[bytes_consumed..bytes_consumed + input_len]);

                // retry on interrupt
                match res {
                    Ok(len) => bytes_consumed += len,
                    Err(e) => match e.kind() {
                        io::ErrorKind::Interrupted => continue,
                        _ => {
                            panic!("should not see other errors");
                        }
                    },
                }
            }

            let _ = stream_encoder.finish().unwrap();

            assert_eq!(orig_len, bytes_consumed);
        }

        assert_eq!(normal_encoded, str::from_utf8(&stream_encoded).unwrap());
    }
}

/// Retry writes until all the data is written or an error that isn't Interrupted is returned.
fn retry_interrupted_write_all<W: Write>(w: &mut W, buf: &[u8]) -> io::Result<()> {
    let mut bytes_consumed = 0;

    while bytes_consumed < buf.len() {
        let res = w.write(&buf[bytes_consumed..]);

        match res {
            Ok(len) => bytes_consumed += len,
            Err(e) => match e.kind() {
                io::ErrorKind::Interrupted => continue,
                _ => return Err(e),
            },
        }
    }

    Ok(())
}

fn do_encode_random_config_matches_normal_encode(max_input_len: usize) {
    let mut rng = rand::thread_rng();
    let mut orig_data = Vec::<u8>::new();
    let mut stream_encoded = Vec::<u8>::new();
    let mut normal_encoded = String::new();

    for _ in 0..1_000 {
        orig_data.clear();
        stream_encoded.clear();
        normal_encoded.clear();

        let orig_len: usize = rng.gen_range(100..20_000);
        for _ in 0..orig_len {
            orig_data.push(rng.gen());
        }

        // encode the normal way
        let engine = random_engine(&mut rng);
        engine.encode_string(&orig_data, &mut normal_encoded);

        // encode via the stream encoder
        {
            let mut stream_encoder = EncoderWriter::new(&mut stream_encoded, &engine);
            let mut bytes_consumed = 0;
            while bytes_consumed < orig_len {
                let input_len: usize =
                    cmp::min(rng.gen_range(0..max_input_len), orig_len - bytes_consumed);

                // write a little bit of the data
                stream_encoder
                    .write_all(&orig_data[bytes_consumed..bytes_consumed + input_len])
                    .unwrap();

                bytes_consumed += input_len;
            }

            let _ = stream_encoder.finish().unwrap();

            assert_eq!(orig_len, bytes_consumed);
        }

        assert_eq!(normal_encoded, str::from_utf8(&stream_encoded).unwrap());
    }
}

/// A `Write` implementation that returns Interrupted some fraction of the time, randomly.
struct InterruptingWriter<'a, W: 'a + Write, R: 'a + Rng> {
    w: &'a mut W,
    rng: &'a mut R,
    /// In [0, 1]. If a random number in [0, 1] is  `<= threshold`, `Write` methods will return
    /// an `Interrupted` error
    fraction: f64,
}

impl<'a, W: Write, R: Rng> Write for InterruptingWriter<'a, W, R> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.rng.gen_range(0.0..1.0) <= self.fraction {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "interrupted"));
        }

        self.w.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.rng.gen_range(0.0..1.0) <= self.fraction {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "interrupted"));
        }

        self.w.flush()
    }
}

/// A `Write` implementation that sometimes will only write part of its input.
struct PartialInterruptingWriter<'a, W: 'a + Write, R: 'a + Rng> {
    w: &'a mut W,
    rng: &'a mut R,
    /// In [0, 1]. If a random number in [0, 1] is  `<= threshold`, `write()` will write all its
    /// input. Otherwise, it will write a random substring
    full_input_fraction: f64,
    no_interrupt_fraction: f64,
}

impl<'a, W: Write, R: Rng> Write for PartialInterruptingWriter<'a, W, R> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.rng.gen_range(0.0..1.0) > self.no_interrupt_fraction {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "interrupted"));
        }

        if self.rng.gen_range(0.0..1.0) <= self.full_input_fraction || buf.is_empty() {
            // pass through the buf untouched
            self.w.write(buf)
        } else {
            // only use a prefix of it
            self.w
                .write(&buf[0..(self.rng.gen_range(0..(buf.len() - 1)))])
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}
