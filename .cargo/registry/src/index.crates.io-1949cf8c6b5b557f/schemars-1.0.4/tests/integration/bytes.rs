use crate::prelude::*;
use bytes1::{Bytes, BytesMut};

#[test]
fn bytes() {
    test!(Bytes)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([Bytes::new(), Bytes::from_iter([12; 34])])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn bytes_mut() {
    test!(BytesMut).assert_identical::<Bytes>();
}
