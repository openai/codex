//! A table-driven UTF-8 Parser
//!
//! This module implements a table-driven UTF-8 parser which should
//! theoretically contain the minimal number of branches (1). The only branch is
//! on the `Action` returned from unpacking a transition.
#![deny(clippy::all, clippy::if_not_else, clippy::enum_glob_use)]
#![cfg_attr(all(feature = "nightly", test), feature(test))]
#![no_std]

use core::char;

mod types;

use types::{Action, State};

/// Handles codepoint and invalid sequence events from the parser.
pub trait Receiver {
    /// Called whenever a codepoint is parsed successfully
    fn codepoint(&mut self, _: char);

    /// Called when an invalid_sequence is detected
    fn invalid_sequence(&mut self);
}

/// A parser for Utf8 Characters
///
/// Repeatedly call `advance` with bytes to emit Utf8 characters
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Parser {
    point: u32,
    state: State,
}

/// Continuation bytes are masked with this value.
const CONTINUATION_MASK: u8 = 0b0011_1111;

impl Parser {
    /// Create a new Parser
    pub fn new() -> Parser {
        Parser { point: 0, state: State::Ground }
    }

    /// Advance the parser
    ///
    /// The provider receiver will be called whenever a codepoint is completed or an invalid
    /// sequence is detected.
    pub fn advance<R>(&mut self, receiver: &mut R, byte: u8)
    where
        R: Receiver,
    {
        let (state, action) = self.state.advance(byte);
        self.perform_action(receiver, byte, action);
        self.state = state;
    }

    fn perform_action<R>(&mut self, receiver: &mut R, byte: u8, action: Action)
    where
        R: Receiver,
    {
        match action {
            Action::InvalidSequence => {
                self.point = 0;
                receiver.invalid_sequence();
            },
            Action::EmitByte => {
                receiver.codepoint(byte as char);
            },
            Action::SetByte1 => {
                let point = self.point | ((byte & CONTINUATION_MASK) as u32);
                let c = unsafe { char::from_u32_unchecked(point) };
                self.point = 0;

                receiver.codepoint(c);
            },
            Action::SetByte2 => {
                self.point |= ((byte & CONTINUATION_MASK) as u32) << 6;
            },
            Action::SetByte2Top => {
                self.point |= ((byte & 0b0001_1111) as u32) << 6;
            },
            Action::SetByte3 => {
                self.point |= ((byte & CONTINUATION_MASK) as u32) << 12;
            },
            Action::SetByte3Top => {
                self.point |= ((byte & 0b0000_1111) as u32) << 12;
            },
            Action::SetByte4 => {
                self.point |= ((byte & 0b0000_0111) as u32) << 18;
            },
        }
    }
}

#[cfg(all(feature = "nightly", test))]
mod benches {
    extern crate std;
    extern crate test;

    use super::{Parser, Receiver};

    use self::test::{black_box, Bencher};

    static UTF8_DEMO: &[u8] = include_bytes!("../tests/UTF-8-demo.txt");

    impl Receiver for () {
        fn codepoint(&mut self, c: char) {
            black_box(c);
        }

        fn invalid_sequence(&mut self) {}
    }

    #[bench]
    fn parse_bench_utf8_demo(b: &mut Bencher) {
        let mut parser = Parser::new();

        b.iter(|| {
            for byte in UTF8_DEMO {
                parser.advance(&mut (), *byte);
            }
        })
    }

    #[bench]
    fn std_string_parse_utf8(b: &mut Bencher) {
        b.iter(|| {
            for c in std::str::from_utf8(UTF8_DEMO).unwrap().chars() {
                black_box(c);
            }
        });
    }
}
