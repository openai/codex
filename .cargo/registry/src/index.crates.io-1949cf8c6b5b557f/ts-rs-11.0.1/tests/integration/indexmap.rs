#![allow(dead_code)]
#![cfg(feature = "indexmap-impl")]

use indexmap::{IndexMap, IndexSet};
use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "indexmap/")]
struct Indexes {
    map: IndexMap<String, String>,
    set: IndexSet<String>,
}

#[test]
fn indexmap() {
    assert_eq!(
        Indexes::decl(),
        "type Indexes = { map: { [key in string]?: string }, set: Array<string>, };"
    )
}
