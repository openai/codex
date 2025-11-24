#![allow(clippy::unwrap_used)]

#[macro_use]
extern crate log;

use std::env;
use std::fmt;
use std::process;
use std::str;

struct Foo;

impl fmt::Display for Foo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        info!("test");
        f.write_str("bar")
    }
}

fn main() {
    env_logger::init();
    if env::var("YOU_ARE_TESTING_NOW").is_ok() {
        return info!("{}", Foo);
    }

    let exe = env::current_exe().unwrap();
    let out = process::Command::new(exe)
        .env("YOU_ARE_TESTING_NOW", "1")
        .env("RUST_LOG", "debug")
        .output()
        .unwrap_or_else(|e| panic!("Unable to start child process: {e}"));
    if out.status.success() {
        return;
    }

    println!("test failed: {}", out.status);
    println!("--- stdout\n{}", str::from_utf8(&out.stdout).unwrap());
    println!("--- stderr\n{}", str::from_utf8(&out.stderr).unwrap());
    process::exit(1);
}
