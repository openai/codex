//! Test Cases
#![allow(
    // This allows the tests to be written more uniform and not have to special case the last clone().
    clippy::redundant_clone,
)]

mod utils;

use crate::utils::{check_deserialization, check_error_deserialization, is_equal};
use expect_test::expect;
use serde::{Deserialize, Serialize};
use serde_with::{
    base64::{Base64, Bcrypt, BinHex, Crypt, ImapMutf7, Standard, UrlSafe},
    formats::{Padded, Unpadded},
    serde_as,
};

#[test]
fn base64_vec() {
    let check_equal = vec![vec![0, 1, 2, 13], vec![14, 5, 6, 7]];
    let check_deser = vec![vec![0xaa, 0xbc, 0xff], vec![0xe0, 0x7d], vec![0xe0, 0x7d]];
    let check_deser_from = r#"["qrz/","4H0=","4H0"]"#;

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct BDefault(#[serde_as(as = "Vec<Base64>")] Vec<Vec<u8>>);

    is_equal(
        BDefault(check_equal.clone()),
        expect![[r#"
            [
              "AAECDQ==",
              "DgUGBw=="
            ]"#]],
    );

    // Check mixed padding deserialization
    check_deserialization(BDefault(check_deser.clone()), check_deser_from);

    check_error_deserialization::<BDefault>(
        r#"["0"]"#,
        expect!["Invalid input length: 1 at line 1 column 4"],
    );
    check_error_deserialization::<BDefault>(
        r#"["zz"]"#,
        expect!["Invalid last symbol 122, offset 1. at line 1 column 5"],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct BPadded(#[serde_as(as = "Vec<Base64<Standard, Padded>>")] Vec<Vec<u8>>);

    is_equal(
        BPadded(check_equal.clone()),
        expect![[r#"
            [
              "AAECDQ==",
              "DgUGBw=="
            ]"#]],
    );
    check_deserialization(BPadded(check_deser.clone()), check_deser_from);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct BUnpadded(#[serde_as(as = "Vec<Base64<Standard, Unpadded>>")] Vec<Vec<u8>>);

    is_equal(
        BUnpadded(check_equal.clone()),
        expect![[r#"
            [
              "AAECDQ",
              "DgUGBw"
            ]"#]],
    );
    check_deserialization(BUnpadded(check_deser.clone()), check_deser_from);
}

#[test]
fn base64_different_charsets() {
    let bytes = [
        0x69_u8, 0xb7, 0x1d, 0x79, 0xf8, 0x21, 0x8a, 0x39, 0x25, 0x9a, 0x7a, 0x29, 0xaa, 0xbb,
        0x2d, 0xba, 0xfc, 0x31, 0xcb, 0x30, 0x01, 0x08, 0x31, 0x05, 0x18, 0x72, 0x09, 0x28, 0xb3,
        0x0d, 0x38, 0xf4, 0x11, 0x49, 0x35, 0x15, 0x59, 0x76, 0x19, 0xd3, 0x5d, 0xb7, 0xe3, 0x9e,
        0xbb, 0xf3, 0xdf, 0xbf, 0x00,
    ];

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct B64Standard(#[serde_as(as = "Base64<Standard, Padded>")] Vec<u8>);

    is_equal(
        B64Standard(bytes.to_vec()),
        expect![[r#""abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+/AA==""#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct B64UrlSafe(#[serde_as(as = "Base64<UrlSafe, Padded>")] Vec<u8>);

    is_equal(
        B64UrlSafe(bytes.to_vec()),
        expect![[r#""abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_AA==""#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct B64Crypt(#[serde_as(as = "Base64<Crypt, Padded>")] Vec<u8>);

    is_equal(
        B64Crypt(bytes.to_vec()),
        expect![[r#""OPQRSTUVWXYZabcdefghijklmn./0123456789ABCDEFGHIJKLMNopqrstuvwxyz..==""#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct B64Bcrypt(#[serde_as(as = "Base64<Bcrypt, Padded>")] Vec<u8>);

    is_equal(
        B64Bcrypt(bytes.to_vec()),
        expect![[r#""YZabcdefghijklmnopqrstuvwx./ABCDEFGHIJKLMNOPQRSTUVWXyz0123456789..==""#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct B64ImapMutf7(#[serde_as(as = "Base64<ImapMutf7, Padded>")] Vec<u8>);

    is_equal(
        B64ImapMutf7(bytes.to_vec()),
        expect![[r#""abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+,AA==""#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct B64BinHex(#[serde_as(as = "Base64<BinHex, Padded>")] Vec<u8>);

    is_equal(
        B64BinHex(bytes.to_vec()),
        expect![[r##""DEFGHIJKLMNPQRSTUVXYZ[`abc!\"#$%&'()*+,-012345689@ABCdefhijklmpqr!!==""##]],
    );
}
