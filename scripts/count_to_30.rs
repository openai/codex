#!/usr/bin/env -S rust-script
//! Stream incremental counter output once per second up to 30.

use std::io::{self, Write};
use std::thread::sleep;
use std::time::Duration;

const LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn main() {
    for value in 0..=30 {
        println!("{value}");
        // Ensure the line is flushed immediately so the TUI can display it.
        io::stdout().flush().expect("flush stdout");
        if let Some(&letter) = LETTERS.get(value as usize) {
            let mut stderr = io::stderr();
            write!(stderr, "{}\n", letter as char).expect("write stderr");
            stderr.flush().expect("flush stderr");
        }
        if value < 30 {
            sleep(Duration::from_secs(1));
        }
    }
}
