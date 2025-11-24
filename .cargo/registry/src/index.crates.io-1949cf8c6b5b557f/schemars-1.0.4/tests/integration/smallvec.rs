use crate::prelude::*;
use smallvec1::{smallvec, SmallVec};

#[test]
fn smallvec() {
    test!(SmallVec<[usize; 2]>)
        .assert_identical::<Vec<usize>>()
        .assert_allows_ser_roundtrip([smallvec![], smallvec![1, 2, 3, 4, 5]])
        .assert_matches_de_roundtrip(arbitrary_values());
}
