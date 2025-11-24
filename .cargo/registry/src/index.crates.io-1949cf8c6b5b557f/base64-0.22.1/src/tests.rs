use std::str;

use rand::{
    distributions,
    distributions::{Distribution as _, Uniform},
    seq::SliceRandom,
    Rng, SeedableRng,
};

use crate::{
    alphabet,
    encode::encoded_len,
    engine::{
        general_purpose::{GeneralPurpose, GeneralPurposeConfig},
        Config, DecodePaddingMode, Engine,
    },
};

#[test]
fn roundtrip_random_config_short() {
    // exercise the slower encode/decode routines that operate on shorter buffers more vigorously
    roundtrip_random_config(Uniform::new(0, 50), 10_000);
}

#[test]
fn roundtrip_random_config_long() {
    roundtrip_random_config(Uniform::new(0, 1000), 10_000);
}

pub fn assert_encode_sanity(encoded: &str, padded: bool, input_len: usize) {
    let input_rem = input_len % 3;
    let expected_padding_len = if input_rem > 0 {
        if padded {
            3 - input_rem
        } else {
            0
        }
    } else {
        0
    };

    let expected_encoded_len = encoded_len(input_len, padded).unwrap();

    assert_eq!(expected_encoded_len, encoded.len());

    let padding_len = encoded.chars().filter(|&c| c == '=').count();

    assert_eq!(expected_padding_len, padding_len);

    let _ = str::from_utf8(encoded.as_bytes()).expect("Base64 should be valid utf8");
}

fn roundtrip_random_config(input_len_range: Uniform<usize>, iterations: u32) {
    let mut input_buf: Vec<u8> = Vec::new();
    let mut encoded_buf = String::new();
    let mut rng = rand::rngs::SmallRng::from_entropy();

    for _ in 0..iterations {
        input_buf.clear();
        encoded_buf.clear();

        let input_len = input_len_range.sample(&mut rng);

        let engine = random_engine(&mut rng);

        for _ in 0..input_len {
            input_buf.push(rng.gen());
        }

        engine.encode_string(&input_buf, &mut encoded_buf);

        assert_encode_sanity(&encoded_buf, engine.config().encode_padding(), input_len);

        assert_eq!(input_buf, engine.decode(&encoded_buf).unwrap());
    }
}

pub fn random_config<R: Rng>(rng: &mut R) -> GeneralPurposeConfig {
    let mode = rng.gen();
    GeneralPurposeConfig::new()
        .with_encode_padding(match mode {
            DecodePaddingMode::Indifferent => rng.gen(),
            DecodePaddingMode::RequireCanonical => true,
            DecodePaddingMode::RequireNone => false,
        })
        .with_decode_padding_mode(mode)
        .with_decode_allow_trailing_bits(rng.gen())
}

impl distributions::Distribution<DecodePaddingMode> for distributions::Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> DecodePaddingMode {
        match rng.gen_range(0..=2) {
            0 => DecodePaddingMode::Indifferent,
            1 => DecodePaddingMode::RequireCanonical,
            _ => DecodePaddingMode::RequireNone,
        }
    }
}

pub fn random_alphabet<R: Rng>(rng: &mut R) -> &'static alphabet::Alphabet {
    ALPHABETS.choose(rng).unwrap()
}

pub fn random_engine<R: Rng>(rng: &mut R) -> GeneralPurpose {
    let alphabet = random_alphabet(rng);
    let config = random_config(rng);
    GeneralPurpose::new(alphabet, config)
}

const ALPHABETS: &[alphabet::Alphabet] = &[
    alphabet::URL_SAFE,
    alphabet::STANDARD,
    alphabet::CRYPT,
    alphabet::BCRYPT,
    alphabet::IMAP_MUTF7,
    alphabet::BIN_HEX,
];
