// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// This example demonstrates zero-copy, zero-allocation deserialization of a u32 vector
// stored in a Bincode buffer.

#![no_main] // https://github.com/unicode-org/icu4x/issues/395
icu_benchmark_macros::instrument!();

use zerovec::ZeroVec;

#[derive(serde::Serialize, serde::Deserialize)]
struct DataStruct<'s> {
    #[serde(borrow)]
    pub nums: ZeroVec<'s, u16>,
}

const U16_SLICE: [u16; 16] = [
    196, 989, 414, 731, 660, 217, 716, 353, 218, 730, 245, 846, 122, 294, 922, 488,
];

const POSTCARD_BYTES: [u8; 33] = [
    0x20, 0xc4, 0x0, 0xdd, 0x3, 0x9e, 0x1, 0xdb, 0x2, 0x94, 0x2, 0xd9, 0x0, 0xcc, 0x2, 0x61, 0x1,
    0xda, 0x0, 0xda, 0x2, 0xf5, 0x0, 0x4e, 0x3, 0x7a, 0x0, 0x26, 0x1, 0x9a, 0x3, 0xe8, 0x1,
];

#[allow(dead_code)]
fn serialize() {
    let data = DataStruct {
        nums: ZeroVec::from_slice_or_alloc(&U16_SLICE),
    };
    let postcard_bytes = postcard::to_stdvec(&data).expect("Serialization should be successful");
    println!("Postcard bytes: {postcard_bytes:#x?}");
    println!("ZeroVec bytes: {:#x?}", data.nums.as_bytes());
}

fn main() {
    // Un-comment the following line to generate postcard data:
    // serialize();

    let data: DataStruct = postcard::from_bytes(&POSTCARD_BYTES).expect("Valid bytes");
    let result = data.nums.iter().sum::<u16>();
    assert_eq!(8141, result);
}
