#![allow(dead_code)]

use std::collections::{BTreeSet, HashSet};

use ts_rs::TS;

#[derive(TS, Eq, PartialEq, Hash)]
#[ts(export, export_to = "hashset/")]
struct CustomValue;

#[derive(TS)]
#[ts(export, export_to = "hashset/")]
struct HashSetWithCustomType {
    set: HashSet<CustomValue>,
}

#[derive(TS)]
#[ts(export, export_to = "hashset/")]
struct BTreeSetWithCustomType {
    set: BTreeSet<CustomValue>,
}

#[test]
fn with_custom_types() {
    assert_eq!(
        HashSetWithCustomType::inline(),
        BTreeSetWithCustomType::inline()
    );
    assert_eq!(
        HashSetWithCustomType::decl(),
        "type HashSetWithCustomType = { set: Array<CustomValue>, };"
    );
}
