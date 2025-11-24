#![cfg(feature = "serde-compat")]

use ts_rs::TS;

struct Foreign;

#[derive(TS)]
#[ts(export, export_to = "issue_415/")]
struct Issue415 {
    #[ts(optional, type = "Date")]
    a: Option<Foreign>,
}

#[test]
fn issue_415() {
    assert_eq!(Issue415::decl(), "type Issue415 = { a?: Date, };");
}

#[derive(TS)]
#[ts(export, export_to = "issue_415/")]
struct InTuple(i32, #[ts(optional, type = "Date")] Option<Foreign>);

#[test]
fn in_tuple() {
    assert_eq!(InTuple::decl(), "type InTuple = [number, (Date)?];");
}
