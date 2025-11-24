//! This little test binary reads stdin, and then writes what it read to both
//! stdout and stderr, with a little tag to differentiate them. We use it to
//! test duping the standard file descriptors.

#![deny(warnings)]

use std::io;
use std::io::prelude::*;

fn main() {
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input).unwrap();

    print!("stdout: ");
    io::stdout().write_all(&input).unwrap();

    eprint!("stderr: ");
    io::stderr().write_all(&input).unwrap();
}
