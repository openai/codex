#[allow(clippy::wildcard_imports)] // false positive
use assert_cmd::prelude::*;

use std::process::Command;

fn main() {
    Command::new("ls")
        .args(["non-existent"])
        .assert()
        .code(&[3, 42] as &[i32]);
}
