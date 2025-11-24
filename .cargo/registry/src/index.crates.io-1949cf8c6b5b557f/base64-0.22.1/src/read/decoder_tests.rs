use std::{
    cmp,
    io::{self, Read as _},
    iter,
};

use rand::{Rng as _, RngCore as _};

use super::decoder::{DecoderReader, BUF_SIZE};
use crate::{
    alphabet,
    engine::{general_purpose::STANDARD, Engine, GeneralPurpose},
    tests::{random_alphabet, random_config, random_engine},
    DecodeError, PAD_BYTE,
};

#[test]
fn simple() {
    let tests: &[(&[u8], &[u8])] = &[
        (&b"0"[..], &b"MA=="[..]),
        (b"01", b"MDE="),
        (b"012", b"MDEy"),
        (b"0123", b"MDEyMw=="),
        (b"01234", b"MDEyMzQ="),
        (b"012345", b"MDEyMzQ1"),
        (b"0123456", b"MDEyMzQ1Ng=="),
        (b"01234567", b"MDEyMzQ1Njc="),
        (b"012345678", b"MDEyMzQ1Njc4"),
        (b"0123456789", b"MDEyMzQ1Njc4OQ=="),
    ][..];

    for (text_expected, base64data) in tests.iter() {
        // Read n bytes at a time.
        for n in 1..base64data.len() + 1 {
            let mut wrapped_reader = io::Cursor::new(base64data);
            let mut decoder = DecoderReader::new(&mut wrapped_reader, &STANDARD);

            // handle errors as you normally would
            let mut text_got = Vec::new();
            let mut buffer = vec![0u8; n];
            while let Ok(read) = decoder.read(&mut buffer[..]) {
                if read == 0 {
                    break;
                }
                text_got.extend_from_slice(&buffer[..read]);
            }

            assert_eq!(
                text_got,
                *text_expected,
                "\nGot: {}\nExpected: {}",
                String::from_utf8_lossy(&text_got[..]),
                String::from_utf8_lossy(text_expected)
            );
        }
    }
}

// Make sure we error out on trailing junk.
#[test]
fn trailing_junk() {
    let tests: &[&[u8]] = &[&b"MDEyMzQ1Njc4*!@#$%^&"[..], b"MDEyMzQ1Njc4OQ== "][..];

    for base64data in tests.iter() {
        // Read n bytes at a time.
        for n in 1..base64data.len() + 1 {
            let mut wrapped_reader = io::Cursor::new(base64data);
            let mut decoder = DecoderReader::new(&mut wrapped_reader, &STANDARD);

            // handle errors as you normally would
            let mut buffer = vec![0u8; n];
            let mut saw_error = false;
            loop {
                match decoder.read(&mut buffer[..]) {
                    Err(_) => {
                        saw_error = true;
                        break;
                    }
                    Ok(0) => break,
                    Ok(_len) => (),
                }
            }

            assert!(saw_error);
        }
    }
}

#[test]
fn handles_short_read_from_delegate() {
    let mut rng = rand::thread_rng();
    let mut bytes = Vec::new();
    let mut b64 = String::new();
    let mut decoded = Vec::new();

    for _ in 0..10_000 {
        bytes.clear();
        b64.clear();
        decoded.clear();

        let size = rng.gen_range(0..(10 * BUF_SIZE));
        bytes.extend(iter::repeat(0).take(size));
        bytes.truncate(size);
        rng.fill_bytes(&mut bytes[..size]);
        assert_eq!(size, bytes.len());

        let engine = random_engine(&mut rng);
        engine.encode_string(&bytes[..], &mut b64);

        let mut wrapped_reader = io::Cursor::new(b64.as_bytes());
        let mut short_reader = RandomShortRead {
            delegate: &mut wrapped_reader,
            rng: &mut rng,
        };

        let mut decoder = DecoderReader::new(&mut short_reader, &engine);

        let decoded_len = decoder.read_to_end(&mut decoded).unwrap();
        assert_eq!(size, decoded_len);
        assert_eq!(&bytes[..], &decoded[..]);
    }
}

#[test]
fn read_in_short_increments() {
    let mut rng = rand::thread_rng();
    let mut bytes = Vec::new();
    let mut b64 = String::new();
    let mut decoded = Vec::new();

    for _ in 0..10_000 {
        bytes.clear();
        b64.clear();
        decoded.clear();

        let size = rng.gen_range(0..(10 * BUF_SIZE));
        bytes.extend(iter::repeat(0).take(size));
        // leave room to play around with larger buffers
        decoded.extend(iter::repeat(0).take(size * 3));

        rng.fill_bytes(&mut bytes[..]);
        assert_eq!(size, bytes.len());

        let engine = random_engine(&mut rng);

        engine.encode_string(&bytes[..], &mut b64);

        let mut wrapped_reader = io::Cursor::new(&b64[..]);
        let mut decoder = DecoderReader::new(&mut wrapped_reader, &engine);

        consume_with_short_reads_and_validate(&mut rng, &bytes[..], &mut decoded, &mut decoder);
    }
}

#[test]
fn read_in_short_increments_with_short_delegate_reads() {
    let mut rng = rand::thread_rng();
    let mut bytes = Vec::new();
    let mut b64 = String::new();
    let mut decoded = Vec::new();

    for _ in 0..10_000 {
        bytes.clear();
        b64.clear();
        decoded.clear();

        let size = rng.gen_range(0..(10 * BUF_SIZE));
        bytes.extend(iter::repeat(0).take(size));
        // leave room to play around with larger buffers
        decoded.extend(iter::repeat(0).take(size * 3));

        rng.fill_bytes(&mut bytes[..]);
        assert_eq!(size, bytes.len());

        let engine = random_engine(&mut rng);

        engine.encode_string(&bytes[..], &mut b64);

        let mut base_reader = io::Cursor::new(&b64[..]);
        let mut decoder = DecoderReader::new(&mut base_reader, &engine);
        let mut short_reader = RandomShortRead {
            delegate: &mut decoder,
            rng: &mut rand::thread_rng(),
        };

        consume_with_short_reads_and_validate(
            &mut rng,
            &bytes[..],
            &mut decoded,
            &mut short_reader,
        );
    }
}

#[test]
fn reports_invalid_last_symbol_correctly() {
    let mut rng = rand::thread_rng();
    let mut bytes = Vec::new();
    let mut b64 = String::new();
    let mut b64_bytes = Vec::new();
    let mut decoded = Vec::new();
    let mut bulk_decoded = Vec::new();

    for _ in 0..1_000 {
        bytes.clear();
        b64.clear();
        b64_bytes.clear();

        let size = rng.gen_range(1..(10 * BUF_SIZE));
        bytes.extend(iter::repeat(0).take(size));
        decoded.extend(iter::repeat(0).take(size));
        rng.fill_bytes(&mut bytes[..]);
        assert_eq!(size, bytes.len());

        let config = random_config(&mut rng);
        let alphabet = random_alphabet(&mut rng);
        // changing padding will cause invalid padding errors when we twiddle the last byte
        let engine = GeneralPurpose::new(alphabet, config.with_encode_padding(false));
        engine.encode_string(&bytes[..], &mut b64);
        b64_bytes.extend(b64.bytes());
        assert_eq!(b64_bytes.len(), b64.len());

        // change the last character to every possible symbol. Should behave the same as bulk
        // decoding whether invalid or valid.
        for &s1 in alphabet.symbols.iter() {
            decoded.clear();
            bulk_decoded.clear();

            // replace the last
            *b64_bytes.last_mut().unwrap() = s1;
            let bulk_res = engine.decode_vec(&b64_bytes[..], &mut bulk_decoded);

            let mut wrapped_reader = io::Cursor::new(&b64_bytes[..]);
            let mut decoder = DecoderReader::new(&mut wrapped_reader, &engine);

            let stream_res = decoder.read_to_end(&mut decoded).map(|_| ()).map_err(|e| {
                e.into_inner()
                    .and_then(|e| e.downcast::<DecodeError>().ok())
            });

            assert_eq!(bulk_res.map_err(|e| Some(Box::new(e))), stream_res);
        }
    }
}

#[test]
fn reports_invalid_byte_correctly() {
    let mut rng = rand::thread_rng();
    let mut bytes = Vec::new();
    let mut b64 = String::new();
    let mut stream_decoded = Vec::new();
    let mut bulk_decoded = Vec::new();

    for _ in 0..10_000 {
        bytes.clear();
        b64.clear();
        stream_decoded.clear();
        bulk_decoded.clear();

        let size = rng.gen_range(1..(10 * BUF_SIZE));
        bytes.extend(iter::repeat(0).take(size));
        rng.fill_bytes(&mut bytes[..size]);
        assert_eq!(size, bytes.len());

        let engine = GeneralPurpose::new(&alphabet::STANDARD, random_config(&mut rng));

        engine.encode_string(&bytes[..], &mut b64);
        // replace one byte, somewhere, with '*', which is invalid
        let bad_byte_pos = rng.gen_range(0..b64.len());
        let mut b64_bytes = b64.bytes().collect::<Vec<u8>>();
        b64_bytes[bad_byte_pos] = b'*';

        let mut wrapped_reader = io::Cursor::new(b64_bytes.clone());
        let mut decoder = DecoderReader::new(&mut wrapped_reader, &engine);

        let read_decode_err = decoder
            .read_to_end(&mut stream_decoded)
            .map_err(|e| {
                let kind = e.kind();
                let inner = e
                    .into_inner()
                    .and_then(|e| e.downcast::<DecodeError>().ok());
                inner.map(|i| (*i, kind))
            })
            .err()
            .and_then(|o| o);

        let bulk_decode_err = engine.decode_vec(&b64_bytes[..], &mut bulk_decoded).err();

        // it's tricky to predict where the invalid data's offset will be since if it's in the last
        // chunk it will be reported at the first padding location because it's treated as invalid
        // padding. So, we just check that it's the same as it is for decoding all at once.
        assert_eq!(
            bulk_decode_err.map(|e| (e, io::ErrorKind::InvalidData)),
            read_decode_err
        );
    }
}

#[test]
fn internal_padding_error_with_short_read_concatenated_texts_invalid_byte_error() {
    let mut rng = rand::thread_rng();
    let mut bytes = Vec::new();
    let mut b64 = String::new();
    let mut reader_decoded = Vec::new();
    let mut bulk_decoded = Vec::new();

    // encodes with padding, requires that padding be present so we don't get InvalidPadding
    // just because padding is there at all
    let engine = STANDARD;

    for _ in 0..10_000 {
        bytes.clear();
        b64.clear();
        reader_decoded.clear();
        bulk_decoded.clear();

        // at least 2 bytes so there can be a split point between bytes
        let size = rng.gen_range(2..(10 * BUF_SIZE));
        bytes.resize(size, 0);
        rng.fill_bytes(&mut bytes[..size]);

        // Concatenate two valid b64s, yielding padding in the middle.
        // This avoids scenarios that are challenging to assert on, like random padding location
        // that might be InvalidLastSymbol when decoded at certain buffer sizes but InvalidByte
        // when done all at once.
        let split = loop {
            // find a split point that will produce padding on the first part
            let s = rng.gen_range(1..size);
            if s % 3 != 0 {
                // short enough to need padding
                break s;
            };
        };

        engine.encode_string(&bytes[..split], &mut b64);
        assert!(b64.contains('='), "split: {}, b64: {}", split, b64);
        let bad_byte_pos = b64.find('=').unwrap();
        engine.encode_string(&bytes[split..], &mut b64);
        let b64_bytes = b64.as_bytes();

        // short read to make it plausible for padding to happen on a read boundary
        let read_len = rng.gen_range(1..10);
        let mut wrapped_reader = ShortRead {
            max_read_len: read_len,
            delegate: io::Cursor::new(&b64_bytes),
        };

        let mut decoder = DecoderReader::new(&mut wrapped_reader, &engine);

        let read_decode_err = decoder
            .read_to_end(&mut reader_decoded)
            .map_err(|e| {
                *e.into_inner()
                    .and_then(|e| e.downcast::<DecodeError>().ok())
                    .unwrap()
            })
            .unwrap_err();

        let bulk_decode_err = engine.decode_vec(b64_bytes, &mut bulk_decoded).unwrap_err();

        assert_eq!(
            bulk_decode_err,
            read_decode_err,
            "read len: {}, bad byte pos: {}, b64: {}",
            read_len,
            bad_byte_pos,
            std::str::from_utf8(b64_bytes).unwrap()
        );
        assert_eq!(
            DecodeError::InvalidByte(
                split / 3 * 4
                    + match split % 3 {
                        1 => 2,
                        2 => 3,
                        _ => unreachable!(),
                    },
                PAD_BYTE
            ),
            read_decode_err
        );
    }
}

#[test]
fn internal_padding_anywhere_error() {
    let mut rng = rand::thread_rng();
    let mut bytes = Vec::new();
    let mut b64 = String::new();
    let mut reader_decoded = Vec::new();

    // encodes with padding, requires that padding be present so we don't get InvalidPadding
    // just because padding is there at all
    let engine = STANDARD;

    for _ in 0..10_000 {
        bytes.clear();
        b64.clear();
        reader_decoded.clear();

        bytes.resize(10 * BUF_SIZE, 0);
        rng.fill_bytes(&mut bytes[..]);

        // Just shove a padding byte in there somewhere.
        // The specific error to expect is challenging to predict precisely because it
        // will vary based on the position of the padding in the quad and the read buffer
        // length, but SOMETHING should go wrong.

        engine.encode_string(&bytes[..], &mut b64);
        let mut b64_bytes = b64.as_bytes().to_vec();
        // put padding somewhere other than the last quad
        b64_bytes[rng.gen_range(0..bytes.len() - 4)] = PAD_BYTE;

        // short read to make it plausible for padding to happen on a read boundary
        let read_len = rng.gen_range(1..10);
        let mut wrapped_reader = ShortRead {
            max_read_len: read_len,
            delegate: io::Cursor::new(&b64_bytes),
        };

        let mut decoder = DecoderReader::new(&mut wrapped_reader, &engine);

        let result = decoder.read_to_end(&mut reader_decoded);
        assert!(result.is_err());
    }
}

fn consume_with_short_reads_and_validate<R: io::Read>(
    rng: &mut rand::rngs::ThreadRng,
    expected_bytes: &[u8],
    decoded: &mut [u8],
    short_reader: &mut R,
) {
    let mut total_read = 0_usize;
    loop {
        assert!(
            total_read <= expected_bytes.len(),
            "tr {} size {}",
            total_read,
            expected_bytes.len()
        );
        if total_read == expected_bytes.len() {
            assert_eq!(expected_bytes, &decoded[..total_read]);
            // should be done
            assert_eq!(0, short_reader.read(&mut *decoded).unwrap());
            // didn't write anything
            assert_eq!(expected_bytes, &decoded[..total_read]);

            break;
        }
        let decode_len = rng.gen_range(1..cmp::max(2, expected_bytes.len() * 2));

        let read = short_reader
            .read(&mut decoded[total_read..total_read + decode_len])
            .unwrap();
        total_read += read;
    }
}

/// Limits how many bytes a reader will provide in each read call.
/// Useful for shaking out code that may work fine only with typical input sources that always fill
/// the buffer.
struct RandomShortRead<'a, 'b, R: io::Read, N: rand::Rng> {
    delegate: &'b mut R,
    rng: &'a mut N,
}

impl<'a, 'b, R: io::Read, N: rand::Rng> io::Read for RandomShortRead<'a, 'b, R, N> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        // avoid 0 since it means EOF for non-empty buffers
        let effective_len = cmp::min(self.rng.gen_range(1..20), buf.len());

        self.delegate.read(&mut buf[..effective_len])
    }
}

struct ShortRead<R: io::Read> {
    delegate: R,
    max_read_len: usize,
}

impl<R: io::Read> io::Read for ShortRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = self.max_read_len.max(buf.len());
        self.delegate.read(&mut buf[..len])
    }
}
