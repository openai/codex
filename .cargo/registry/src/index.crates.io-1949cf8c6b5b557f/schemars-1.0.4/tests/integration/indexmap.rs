use crate::prelude::*;
use indexmap2::{indexmap, indexset, IndexMap, IndexSet};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn indexmap() {
    test!(IndexMap<String, bool>)
        .assert_identical::<BTreeMap<String, bool>>()
        .assert_allows_ser_roundtrip([indexmap!(), indexmap!("key".to_owned() => true)])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn indexset() {
    test!(IndexSet<String>)
        .assert_identical::<BTreeSet<String>>()
        .assert_allows_ser_roundtrip([indexset!(), indexset!("test".to_owned())])
        .assert_matches_de_roundtrip(arbitrary_values());
}
