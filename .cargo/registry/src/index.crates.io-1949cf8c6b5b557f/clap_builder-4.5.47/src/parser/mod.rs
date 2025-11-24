//! [`Command`][crate::Command] line argument parser

mod arg_matcher;
mod error;
mod matches;
#[allow(clippy::module_inception)]
mod parser;
mod validator;

pub(crate) mod features;

pub(crate) use self::arg_matcher::ArgMatcher;
pub(crate) use self::matches::{MatchedArg, SubCommand};
pub(crate) use self::parser::Identifier;
pub(crate) use self::parser::Parser;
pub(crate) use self::parser::PendingArg;
pub(crate) use self::validator::get_possible_values_cli;
pub(crate) use self::validator::Validator;

pub use self::matches::IdsRef;
pub use self::matches::RawValues;
pub use self::matches::Values;
pub use self::matches::ValuesRef;
pub use self::matches::{ArgMatches, Indices, ValueSource};
pub use error::MatchesError;
