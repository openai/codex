use rand::{Rng, SeedableRng};

use base64::engine::{general_purpose::STANDARD, Engine};
use base64::*;

use base64::engine::general_purpose::{GeneralPurpose, NO_PAD};

// generate random contents of the specified length and test encode/decode roundtrip
fn roundtrip_random<E: Engine>(
    byte_buf: &mut Vec<u8>,
    str_buf: &mut String,
    engine: &E,
    byte_len: usize,
    approx_values_per_byte: u8,
    max_rounds: u64,
) {
    // let the short ones be short but don't let it get too crazy large
    let num_rounds = calculate_number_of_rounds(byte_len, approx_values_per_byte, max_rounds);
    let mut r = rand::rngs::SmallRng::from_entropy();
    let mut decode_buf = Vec::new();

    for _ in 0..num_rounds {
        byte_buf.clear();
        str_buf.clear();
        decode_buf.clear();
        while byte_buf.len() < byte_len {
            byte_buf.push(r.gen::<u8>());
        }

        engine.encode_string(&byte_buf, str_buf);
        engine.decode_vec(&str_buf, &mut decode_buf).unwrap();

        assert_eq!(byte_buf, &decode_buf);
    }
}

fn calculate_number_of_rounds(byte_len: usize, approx_values_per_byte: u8, max: u64) -> u64 {
    // don't overflow
    let mut prod = approx_values_per_byte as u64;

    for _ in 0..byte_len {
        if prod > max {
            return max;
        }

        prod = prod.saturating_mul(prod);
    }

    prod
}

#[test]
fn roundtrip_random_short_standard() {
    let mut byte_buf: Vec<u8> = Vec::new();
    let mut str_buf = String::new();

    for input_len in 0..40 {
        roundtrip_random(&mut byte_buf, &mut str_buf, &STANDARD, input_len, 4, 10000);
    }
}

#[test]
fn roundtrip_random_with_fast_loop_standard() {
    let mut byte_buf: Vec<u8> = Vec::new();
    let mut str_buf = String::new();

    for input_len in 40..100 {
        roundtrip_random(&mut byte_buf, &mut str_buf, &STANDARD, input_len, 4, 1000);
    }
}

#[test]
fn roundtrip_random_short_no_padding() {
    let mut byte_buf: Vec<u8> = Vec::new();
    let mut str_buf = String::new();

    let engine = GeneralPurpose::new(&alphabet::STANDARD, NO_PAD);
    for input_len in 0..40 {
        roundtrip_random(&mut byte_buf, &mut str_buf, &engine, input_len, 4, 10000);
    }
}

#[test]
fn roundtrip_random_no_padding() {
    let mut byte_buf: Vec<u8> = Vec::new();
    let mut str_buf = String::new();

    let engine = GeneralPurpose::new(&alphabet::STANDARD, NO_PAD);

    for input_len in 40..100 {
        roundtrip_random(&mut byte_buf, &mut str_buf, &engine, input_len, 4, 1000);
    }
}

#[test]
fn roundtrip_decode_trailing_10_bytes() {
    // This is a special case because we decode 8 byte blocks of input at a time as much as we can,
    // ideally unrolled to 32 bytes at a time, in stages 1 and 2. Since we also write a u64's worth
    // of bytes (8) to the output, we always write 2 garbage bytes that then will be overwritten by
    // the NEXT block. However, if the next block only contains 2 bytes, it will decode to 1 byte,
    // and therefore be too short to cover up the trailing 2 garbage bytes. Thus, we have stage 3
    // to handle that case.

    for num_quads in 0..25 {
        let mut s: String = "ABCD".repeat(num_quads);
        s.push_str("EFGHIJKLZg");

        let engine = GeneralPurpose::new(&alphabet::STANDARD, NO_PAD);
        let decoded = engine.decode(&s).unwrap();
        assert_eq!(num_quads * 3 + 7, decoded.len());

        assert_eq!(s, engine.encode(&decoded));
    }
}

#[test]
fn display_wrapper_matches_normal_encode() {
    let mut bytes = Vec::<u8>::with_capacity(256);

    for i in 0..255 {
        bytes.push(i);
    }
    bytes.push(255);

    assert_eq!(
        STANDARD.encode(&bytes),
        format!("{}", display::Base64Display::new(&bytes, &STANDARD))
    );
}

#[test]
fn encode_engine_slice_error_when_buffer_too_small() {
    for num_triples in 1..100 {
        let input = "AAA".repeat(num_triples);
        let mut vec = vec![0; (num_triples - 1) * 4];
        assert_eq!(
            EncodeSliceError::OutputSliceTooSmall,
            STANDARD.encode_slice(&input, &mut vec).unwrap_err()
        );
        vec.push(0);
        assert_eq!(
            EncodeSliceError::OutputSliceTooSmall,
            STANDARD.encode_slice(&input, &mut vec).unwrap_err()
        );
        vec.push(0);
        assert_eq!(
            EncodeSliceError::OutputSliceTooSmall,
            STANDARD.encode_slice(&input, &mut vec).unwrap_err()
        );
        vec.push(0);
        assert_eq!(
            EncodeSliceError::OutputSliceTooSmall,
            STANDARD.encode_slice(&input, &mut vec).unwrap_err()
        );
        vec.push(0);
        assert_eq!(
            num_triples * 4,
            STANDARD.encode_slice(&input, &mut vec).unwrap()
        );
    }
}
