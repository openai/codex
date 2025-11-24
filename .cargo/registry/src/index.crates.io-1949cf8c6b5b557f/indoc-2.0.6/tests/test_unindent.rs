#![allow(clippy::shadow_unrelated)]

use unindent::{unindent, unindent_bytes, Unindent};

#[test]
fn fn_unindent_str() {
    let s = "
        line one
        line two";
    assert_eq!(unindent(s), "line one\nline two");

    let s = "\n\t\t\tline one\n\t\t\tline two";
    assert_eq!(unindent(s), "line one\nline two");
}

#[test]
fn fn_unindent_bytes() {
    let b = b"
        line one
        line two";
    assert_eq!(unindent_bytes(b), b"line one\nline two");

    let b = b"\n\t\t\tline one\n\t\t\tline two";
    assert_eq!(unindent_bytes(b), b"line one\nline two");
}

#[test]
fn trait_unindent_str() {
    let s = "
        line one
        line two";
    assert_eq!(s.unindent(), "line one\nline two");

    let s = "\n\t\t\tline one\n\t\t\tline two";
    assert_eq!(s.unindent(), "line one\nline two");
}

#[test]
fn trait_unindent_bytes() {
    let b = b"
        line one
        line two";
    assert_eq!(b.unindent(), b"line one\nline two");

    let b = b"\n\t\t\tline one\n\t\t\tline two";
    assert_eq!(b.unindent(), b"line one\nline two");
}

#[test]
fn carriage_returns() {
    let s = "\r\n\tline one\r\n\tline two";
    assert_eq!(unindent(s), "line one\r\nline two");
}
