#![allow(dead_code)]
use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "list/")]
struct List {
    data: Option<Vec<u32>>,
}

#[test]
fn list() {
    assert_eq!(List::decl(), "type List = { data: Array<number> | null, };");
}
