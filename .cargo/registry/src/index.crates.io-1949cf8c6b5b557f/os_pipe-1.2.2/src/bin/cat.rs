#![deny(warnings)]

/// Windows doesn't have a native equivalent for cat, so we use this little
/// Rust implementation instead.
use std::io::{copy, stdin, stdout};

fn main() {
    let stdin_handle = stdin();
    let stdout_handle = stdout();
    copy(&mut stdin_handle.lock(), &mut stdout_handle.lock()).unwrap();
}
