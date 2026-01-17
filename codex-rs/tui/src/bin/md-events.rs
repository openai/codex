//! Debug helper that prints pulldown-cmark events for stdin.
//!
//! Feed markdown on stdin and this tool will emit the parsed event stream so
//! developers can inspect how pulldown-cmark tokenizes input.

use std::io::Read;
use std::io::{self};

/// Read markdown from stdin and dump the parsed event stream.
fn main() {
    let mut input = String::new();
    if let Err(err) = io::stdin().read_to_string(&mut input) {
        eprintln!("failed to read stdin: {err}");
        std::process::exit(1);
    }

    let parser = pulldown_cmark::Parser::new(&input);
    for event in parser {
        println!("{event:?}");
    }
}
