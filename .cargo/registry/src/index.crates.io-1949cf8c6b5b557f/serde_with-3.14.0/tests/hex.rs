//! Test Cases

mod utils;

use crate::utils::{check_deserialization, check_error_deserialization, is_equal};
use expect_test::expect;
use serde::{Deserialize, Serialize};
use serde_with::{
    formats::{Lowercase, Uppercase},
    hex::Hex,
    serde_as,
};

#[test]
fn hex_vec() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct B(#[serde_as(as = "Vec<Hex>")] Vec<Vec<u8>>);

    is_equal(
        B(vec![vec![0, 1, 2, 13], vec![14, 5, 6, 7]]),
        expect![[r#"
            [
              "0001020d",
              "0e050607"
            ]"#]],
    );

    // Check mixed case deserialization
    check_deserialization(
        B(vec![vec![0xaa, 0xbc, 0xff], vec![0xe0, 0x7d]]),
        r#"["aaBCff","E07d"]"#,
    );

    check_error_deserialization::<B>(
        r#"["0"]"#,
        expect![[r#"Odd number of digits at line 1 column 5"#]],
    );
    check_error_deserialization::<B>(
        r#"["zz"]"#,
        expect![[r#"Invalid character 'z' at position 0 at line 1 column 6"#]],
    );
}

#[test]
fn hex_vec_lowercase() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct B(#[serde_as(as = "Vec<Hex<Lowercase>>")] Vec<Vec<u8>>);

    is_equal(
        B(vec![vec![0, 1, 2, 13], vec![14, 5, 6, 7]]),
        expect![[r#"
            [
              "0001020d",
              "0e050607"
            ]"#]],
    );

    // Check mixed case deserialization
    check_deserialization(
        B(vec![vec![0xaa, 0xbc, 0xff], vec![0xe0, 0x7d]]),
        r#"["aaBCff","E07d"]"#,
    );
}

#[test]
fn hex_vec_uppercase() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct B(#[serde_as(as = "Vec<Hex<Uppercase>>")] Vec<Vec<u8>>);

    is_equal(
        B(vec![vec![0, 1, 2, 13], vec![14, 5, 6, 7]]),
        expect![[r#"
            [
              "0001020D",
              "0E050607"
            ]"#]],
    );

    // Check mixed case deserialization
    check_deserialization(
        B(vec![vec![0xaa, 0xbc, 0xff], vec![0xe0, 0x7d]]),
        r#"["aaBCff","E07d"]"#,
    );
}
