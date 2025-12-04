#![doc = include_str!("../README.md")]

pub mod fuzz;
pub mod highlight;
pub mod init;
pub mod input;
pub mod logger;
pub mod parse;
pub mod playground;
pub mod query;
pub mod query_testing;
pub mod tags;
pub mod test;
pub mod test_highlight;
pub mod test_tags;
pub mod util;
pub mod version;
pub mod wasm;

#[cfg(test)]
mod tests;

// To run compile fail tests
#[cfg(doctest)]
mod tests;
