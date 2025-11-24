use crate::prelude::*;
use arrayvec07::{ArrayString, ArrayVec};

#[test]
fn arrayvec07() {
    test!(ArrayVec<i32, 8>)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([
            ArrayVec::from_iter([]),
            ArrayVec::from_iter([1, 2, 3, 4, 5, 6, 7, 8]),
        ])
        .assert_matches_de_roundtrip(
            (0..16).map(|len| Value::Array((0..len).map(Value::from).collect())),
        )
        .assert_matches_de_roundtrip(arbitrary_values_except(
            is_array_of_u64,
            "FIXME schema allows out-of-range positive integers",
        ));
}

#[test]
fn arrayvec07_arraystring() {
    test!(ArrayString<8>)
        .assert_identical::<String>()
        .assert_allows_ser_roundtrip(["".try_into().unwrap(), "12345678".try_into().unwrap()])
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_string,
            "There's not a good way to express UTF-8 byte length in JSON schema, so schema ignores the ArrayString's capacity.",
        ));
}

fn is_array_of_u64(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|a| a.iter().all(Value::is_u64))
}
