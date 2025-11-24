#![allow(dead_code)]

use std::{
    collections::BTreeSet,
    ops::{Range, RangeInclusive},
};

use ts_rs::{Dependency, TS};

#[derive(TS)]
#[ts(export, export_to = "ranges/")]
struct Inner(i32);

#[derive(TS)]
#[ts(export, export_to = "ranges/")]
struct RangeTest {
    a: Range<u32>,
    b: Range<&'static str>,
    c: Range<Range<i32>>,
    d: RangeInclusive<u32>,
    e: Range<Inner>,
}

#[test]
fn range() {
    assert_eq!(
        RangeTest::decl(),
        "type RangeTest = { \
            a: { start: number, end: number, }, \
            b: { start: string, end: string, }, \
            c: { \
                start: { start: number, end: number, }, \
                end: { start: number, end: number, }, \
            }, \
            d: { start: number, end: number, }, \
            e: { start: Inner, end: Inner, }, \
        };"
    );
    assert_eq!(
        RangeTest::dependencies()
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
        vec![Dependency::from_ty::<Inner>().unwrap(),]
    );
}
