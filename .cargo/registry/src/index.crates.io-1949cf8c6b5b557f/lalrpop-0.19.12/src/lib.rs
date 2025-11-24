// Need this for rusty_peg
#![recursion_limit = "256"]
// I hate this lint.
#![allow(unused_parens)]

// The builtin tests don't cover the CLI and so forth, and it's just
// too darn annoying to try and make them do so.
//
// ε shows up in lalrpop/src/lr1/example/test.rs
#![cfg_attr(test, allow(dead_code, mixed_script_confusables))]

extern crate ascii_canvas;
extern crate bit_set;
extern crate diff;
extern crate ena;
extern crate is_terminal;
extern crate itertools;
extern crate petgraph;
extern crate regex_syntax;
extern crate regex;
extern crate string_cache;
extern crate term;
extern crate tiny_keccak;
extern crate unicode_xid;

#[cfg_attr(feature = "test", macro_use)]
extern crate lalrpop_util;

#[cfg(test)]
extern crate rand;

// hoist the modules that define macros up earlier
#[macro_use]
mod rust;
#[macro_use]
mod log;

mod api;
mod build;
mod collections;
mod file_text;
mod grammar;
mod kernel_set;
mod lexer;
mod lr1;
mod message;
mod normalize;
mod parser;
mod session;
mod tls;
mod tok;
mod util;

#[cfg(test)]
mod generate;
#[cfg(test)]
mod test_util;

pub use crate::api::process_root;
pub use crate::api::process_root_unconditionally;
pub use crate::api::Configuration;
use ascii_canvas::style;
