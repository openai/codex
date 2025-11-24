#![allow(clippy::exit)]

use std::env;
use std::error::Error;
use std::io;
use std::io::Write;
use std::process;

fn run() -> Result<(), Box<dyn Error>> {
    if let Ok(text) = env::var("stdout") {
        println!("{text}");
    }
    if let Ok(text) = env::var("stderr") {
        eprintln!("{text}");
    }

    let code = env::var("exit")
        .ok()
        .map(|v| v.parse::<i32>())
        .map(|r| r.map(Some))
        .unwrap_or(Ok(None))?
        .unwrap_or(0);
    process::exit(code);
}

fn main() {
    let code = match run() {
        Ok(_) => 0,
        Err(ref e) => {
            write!(&mut io::stderr(), "{e}").expect("writing to stderr won't fail");
            1
        }
    };
    process::exit(code);
}
