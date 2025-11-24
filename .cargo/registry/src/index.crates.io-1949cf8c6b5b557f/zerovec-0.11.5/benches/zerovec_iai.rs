// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use iai::black_box;

#[path = "../src/samples.rs"]
mod samples;
use samples::*;

use zerovec::VarZeroSlice;
use zerovec::ZeroVec;

fn sum_slice() -> u32 {
    black_box(TEST_SLICE).iter().sum::<u32>()
}

fn sum_zerovec() -> u32 {
    ZeroVec::<u32>::parse_bytes(black_box(TEST_BUFFER_LE))
        .unwrap()
        .iter()
        .sum::<u32>()
}

fn binarysearch_slice() -> Result<usize, usize> {
    black_box(TEST_SLICE).binary_search(&0x0c0d0c)
}

fn binarysearch_zerovec() -> Result<usize, usize> {
    ZeroVec::<u32>::parse_bytes(black_box(TEST_BUFFER_LE))
        .unwrap()
        .binary_search(&0x0c0d0c)
}

fn varzeroslice_parse_get() -> Option<&'static str> {
    let slice: &'static VarZeroSlice<str> =
        VarZeroSlice::parse_bytes(black_box(TEST_VARZEROSLICE_BYTES)).unwrap();
    slice.get(black_box(1))
}

fn varzeroslice_get() -> Option<&'static str> {
    // Safety: The bytes are valid.
    let slice: &'static VarZeroSlice<str> =
        unsafe { VarZeroSlice::from_bytes_unchecked(black_box(TEST_VARZEROSLICE_BYTES)) };
    slice.get(black_box(1))
}

fn varzeroslice_get_unchecked() -> &'static str {
    // Safety: The bytes are valid.
    let slice: &'static VarZeroSlice<str> =
        unsafe { VarZeroSlice::from_bytes_unchecked(black_box(TEST_VARZEROSLICE_BYTES)) };
    // Safety: The VarZeroVec has length 4.
    unsafe { slice.get_unchecked(black_box(1)) }
}

iai::main!(
    sum_slice,
    sum_zerovec,
    binarysearch_slice,
    binarysearch_zerovec,
    varzeroslice_parse_get,
    varzeroslice_get,
    varzeroslice_get_unchecked,
);
