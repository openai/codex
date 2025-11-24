#![cfg(all(windows, feature = "clock", feature = "std"))]

use std::fs;
use windows_bindgen::bindgen;

#[test]
fn gen_bindings() {
    let input = "src/offset/local/win_bindings.txt";
    let output = "src/offset/local/win_bindings.rs";
    let existing = fs::read_to_string(output).unwrap();

    bindgen(["--no-deps", "--etc", input]).unwrap();

    // Check the output is the same as before.
    // Depending on the git configuration the file may have been checked out with `\r\n` newlines or
    // with `\n`. Compare line-by-line to ignore this difference.
    let mut new = fs::read_to_string(output).unwrap();
    if existing.contains("\r\n") && !new.contains("\r\n") {
        new = new.replace("\n", "\r\n");
    } else if !existing.contains("\r\n") && new.contains("\r\n") {
        new = new.replace("\r\n", "\n");
    }

    similar_asserts::assert_eq!(existing, new);
    if !new.lines().eq(existing.lines()) {
        panic!("generated file `{output}` is changed.");
    }
}
