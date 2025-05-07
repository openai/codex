use std::path::PathBuf;

use serde::Serialize;

use crate::arg_matcher::ArgMatcher;
use crate::arg_resolver::PositionalArg;
use serde_with::DisplayFromStr;
use serde_with::serde_as;

pub type Result<T> = std::result::Result<T, Error>;

#[serde_as]
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum Error {
    NoSpecForProgram {
        program: String,
    },
    OptionMissingValue {
        program: String,
        option: String,
    },
    OptionFollowedByOptionInsteadOfValue {
        program: String,
        option: String,
        value: String,
    },
    UnknownOption {
        program: String,
        option: String,
    },
    UnexpectedArguments {
        program: String,
        args: Vec<PositionalArg>,
    },
    DoubleDashNotSupportedYet {
        program: String,
    },
    MultipleVarargPatterns {
        program: String,
        first: ArgMatcher,
        second: ArgMatcher,
    },
    RangeStartExceedsEnd {
        start: usize,
        end: usize,
    },
    RangeEndOutOfBounds {
        end: usize,
        len: usize,
    },
    PrefixOverlapsSuffix {},
    NotEnoughArgs {
        program: String,
        args: Vec<PositionalArg>,
        arg_patterns: Vec<ArgMatcher>,
    },
    InternalInvariantViolation {
        message: String,
    },
    VarargMatcherDidNotMatchAnything {
        program: String,
        matcher: ArgMatcher,
    },
    EmptyFileName {},
    LiteralValueDidNotMatch {
        expected: String,
        actual: String,
    },
    InvalidPositiveInteger {
        value: String,
    },
    MissingRequiredOptions {
        program: String,
        options: Vec<String>,
    },
    SedCommandNotProvablySafe {
        command: String,
    },
    ReadablePathNotInReadableFolders {
        file: PathBuf,
        folders: Vec<PathBuf>,
    },
    WriteablePathNotInWriteableFolders {
        file: PathBuf,
        folders: Vec<PathBuf>,
    },
    CannotCheckRelativePath {
        file: PathBuf,
    },
    CannotCanonicalizePath {
        file: String,
        #[serde_as(as = "DisplayFromStr")]
        error: std::io::ErrorKind,
    },
}
