// Test that a custom handler works on wasm32-unknown-unknown
#![cfg(all(
    target_arch = "wasm32",
    target_os = "unknown",
    feature = "custom",
    not(feature = "js")
))]

use wasm_bindgen_test::wasm_bindgen_test as test;

use core::num::NonZeroU32;
use getrandom::{getrandom, register_custom_getrandom, Error};

fn len7_err() -> Error {
    NonZeroU32::new(Error::INTERNAL_START + 7).unwrap().into()
}

fn super_insecure_rng(buf: &mut [u8]) -> Result<(), Error> {
    // `getrandom` guarantees it will not call any implementation if the output
    // buffer is empty.
    assert!(!buf.is_empty());
    // Length 7 buffers return a custom error
    if buf.len() == 7 {
        return Err(len7_err());
    }
    // Otherwise, fill bytes based on input length
    let mut start = buf.len() as u8;
    for b in buf {
        *b = start;
        start = start.wrapping_mul(3);
    }
    Ok(())
}

register_custom_getrandom!(super_insecure_rng);

use getrandom::getrandom as getrandom_impl;
mod common;

#[test]
fn custom_rng_output() {
    let mut buf = [0u8; 4];
    assert_eq!(getrandom(&mut buf), Ok(()));
    assert_eq!(buf, [4, 12, 36, 108]);

    let mut buf = [0u8; 3];
    assert_eq!(getrandom(&mut buf), Ok(()));
    assert_eq!(buf, [3, 9, 27]);
}

#[test]
fn rng_err_output() {
    assert_eq!(getrandom(&mut [0; 7]), Err(len7_err()));
}
