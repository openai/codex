// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use litemap::LiteMap;
use rkyv::archived_root;
use rkyv::check_archived_root;
use rkyv::ser::serializers::AllocSerializer;
use rkyv::ser::Serializer;
use rkyv::util::AlignedBytes;
use rkyv::util::AlignedVec;
use rkyv::Deserialize;
use rkyv::Infallible;

const DATA: [(&str, &str); 11] = [
    ("ar", "Arabic"),
    ("bn", "Bangla"),
    ("ccp", "Chakma"),
    ("en", "English"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("ja", "Japanese"),
    ("ru", "Russian"),
    ("sr", "Serbian"),
    ("th", "Thai"),
    ("tr", "Turkish"),
];

const RKYV: AlignedBytes<192> = AlignedBytes(if cfg!(target_endian = "little") {
    [
        74, 97, 112, 97, 110, 101, 115, 101, 97, 114, 0, 0, 0, 0, 0, 2, 65, 114, 97, 98, 105, 99,
        0, 6, 98, 110, 0, 0, 0, 0, 0, 2, 66, 97, 110, 103, 108, 97, 0, 6, 99, 99, 112, 0, 0, 0, 0,
        3, 67, 104, 97, 107, 109, 97, 0, 6, 101, 110, 0, 0, 0, 0, 0, 2, 69, 110, 103, 108, 105,
        115, 104, 7, 101, 115, 0, 0, 0, 0, 0, 2, 83, 112, 97, 110, 105, 115, 104, 7, 102, 114, 0,
        0, 0, 0, 0, 2, 70, 114, 101, 110, 99, 104, 0, 6, 106, 97, 0, 0, 0, 0, 0, 2, 8, 0, 0, 0,
        144, 255, 255, 255, 114, 117, 0, 0, 0, 0, 0, 2, 82, 117, 115, 115, 105, 97, 110, 7, 115,
        114, 0, 0, 0, 0, 0, 2, 83, 101, 114, 98, 105, 97, 110, 7, 116, 104, 0, 0, 0, 0, 0, 2, 84,
        104, 97, 105, 0, 0, 0, 4, 116, 114, 0, 0, 0, 0, 0, 2, 84, 117, 114, 107, 105, 115, 104, 7,
        80, 255, 255, 255, 11, 0, 0, 0,
    ]
} else {
    [
        74, 97, 112, 97, 110, 101, 115, 101, 97, 114, 0, 0, 0, 0, 0, 2, 65, 114, 97, 98, 105, 99,
        0, 6, 98, 110, 0, 0, 0, 0, 0, 2, 66, 97, 110, 103, 108, 97, 0, 6, 99, 99, 112, 0, 0, 0, 0,
        3, 67, 104, 97, 107, 109, 97, 0, 6, 101, 110, 0, 0, 0, 0, 0, 2, 69, 110, 103, 108, 105,
        115, 104, 7, 101, 115, 0, 0, 0, 0, 0, 2, 83, 112, 97, 110, 105, 115, 104, 7, 102, 114, 0,
        0, 0, 0, 0, 2, 70, 114, 101, 110, 99, 104, 0, 6, 106, 97, 0, 0, 0, 0, 0, 2, 0, 0, 0, 8,
        144, 255, 255, 255, 114, 117, 0, 0, 0, 0, 0, 2, 82, 117, 115, 115, 105, 97, 110, 7, 115,
        114, 0, 0, 0, 0, 0, 2, 83, 101, 114, 98, 105, 97, 110, 7, 116, 104, 0, 0, 0, 0, 0, 2, 84,
        104, 97, 105, 0, 0, 0, 4, 116, 114, 0, 0, 0, 0, 0, 2, 84, 117, 114, 107, 105, 115, 104, 7,
        255, 255, 255, 80, 0, 0, 0, 11,
    ]
});

type LiteMapOfStrings = LiteMap<String, String>;
type TupleVecOfStrings = Vec<(String, String)>;

fn generate() -> AlignedVec {
    let map = DATA
        .iter()
        .map(|&(k, v)| (k.to_owned(), v.to_owned()))
        .collect::<LiteMapOfStrings>();

    let mut serializer = AllocSerializer::<4096>::default();
    serializer
        .serialize_value(&map.into_tuple_vec())
        .expect("failed to archive test");
    serializer.into_serializer().into_inner()
}

#[test]
fn rkyv_serialize() {
    let serialized = generate();
    assert_eq!(RKYV.0, serialized.as_slice());
}

#[test]
fn rkyv_archive() {
    let archived = unsafe { archived_root::<TupleVecOfStrings>(&RKYV.0) };
    let s = archived[0].1.as_str();
    assert_eq!(s, "Arabic");
}

#[test]
fn rkyv_checked_archive() {
    let archived = check_archived_root::<TupleVecOfStrings>(&RKYV.0).unwrap();
    let s = archived[0].1.as_str();
    assert_eq!(s, "Arabic");
}

#[test]
fn rkyv_deserialize() {
    let archived = unsafe { archived_root::<TupleVecOfStrings>(&RKYV.0) };
    let deserialized = archived.deserialize(&mut Infallible).unwrap();
    // Safe because we are deserializing a buffer from a trusted source
    let deserialized: LiteMapOfStrings = LiteMap::from_sorted_store_unchecked(deserialized);
    assert_eq!(deserialized.get("tr").map(String::as_str), Some("Turkish"));
}
