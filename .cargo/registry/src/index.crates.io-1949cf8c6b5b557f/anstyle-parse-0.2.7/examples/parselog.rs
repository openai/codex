//! Parse input from stdin and log actions on stdout
use std::io::{self, Read};

use anstyle_parse::{DefaultCharAccumulator, Params, Parser, Perform};

/// A type implementing Perform that just logs actions
struct Log;

impl Perform for Log {
    fn print(&mut self, c: char) {
        println!("[print] {c:?}");
    }

    fn execute(&mut self, byte: u8) {
        println!("[execute] {byte:02x}");
    }

    fn hook(&mut self, params: &Params, intermediates: &[u8], ignore: bool, c: u8) {
        println!(
            "[hook] params={params:?}, intermediates={intermediates:?}, ignore={ignore:?}, char={c:?}"
        );
    }

    fn put(&mut self, byte: u8) {
        println!("[put] {byte:02x}");
    }

    fn unhook(&mut self) {
        println!("[unhook]");
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        println!("[osc_dispatch] params={params:?} bell_terminated={bell_terminated}");
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, c: u8) {
        println!(
            "[csi_dispatch] params={params:#?}, intermediates={intermediates:?}, ignore={ignore:?}, char={c:?}"
        );
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        println!(
            "[esc_dispatch] intermediates={intermediates:?}, ignore={ignore:?}, byte={byte:02x}"
        );
    }
}

fn main() {
    let input = io::stdin();
    let mut handle = input.lock();

    let mut statemachine = Parser::<DefaultCharAccumulator>::new();
    let mut performer = Log;

    let mut buf = [0; 2048];

    loop {
        match handle.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                for byte in &buf[..n] {
                    statemachine.advance(&mut performer, *byte);
                }
            }
            Err(err) => {
                println!("err: {err}");
                break;
            }
        }
    }
}
