#![allow(dead_code)]

use std::cell::RefCell;

use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "simple/")]
struct Simple {
    a: i32,
    b: String,
    c: (i32, String, RefCell<i32>),
    d: Vec<String>,
    e: Option<String>,
    f: char,
    g: Option<char>,
}

#[test]
fn test_def() {
    assert_eq!(
        Simple::inline(),
        "{ a: number, b: string, c: [number, string, number], d: Array<string>, e: string | null, f: string, g: string | null, }"
    )
}
