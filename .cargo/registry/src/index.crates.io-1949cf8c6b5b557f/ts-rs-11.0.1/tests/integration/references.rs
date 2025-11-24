#![allow(dead_code)]

use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "references/")]
struct FullOfRefs<'a> {
    str_slice: &'a str,
    ref_slice: &'a [&'a str],
    num_ref: &'a i32,
}

#[test]
fn references() {
    assert_eq!(
        FullOfRefs::inline(),
        "{ str_slice: string, ref_slice: Array<string>, num_ref: number, }"
    )
}
