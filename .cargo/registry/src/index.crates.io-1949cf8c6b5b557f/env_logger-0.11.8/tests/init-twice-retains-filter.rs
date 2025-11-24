#![allow(clippy::unwrap_used)]

use std::env;
use std::process;
use std::str;

fn main() {
    if env::var("YOU_ARE_TESTING_NOW").is_ok() {
        // Init from the env (which should set the max level to `Debug`)
        env_logger::init();

        assert_eq!(log::LevelFilter::Debug, log::max_level());

        // Init again using a different max level
        // This shouldn't clobber the level that was previously set
        env_logger::Builder::new()
            .parse_filters("info")
            .try_init()
            .unwrap_err();

        assert_eq!(log::LevelFilter::Debug, log::max_level());
        return;
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
