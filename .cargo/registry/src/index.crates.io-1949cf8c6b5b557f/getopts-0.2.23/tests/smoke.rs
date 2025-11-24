extern crate getopts;

use std::env;

#[test]
fn main() {
    getopts::Options::new().parse(env::args()).unwrap();
}
