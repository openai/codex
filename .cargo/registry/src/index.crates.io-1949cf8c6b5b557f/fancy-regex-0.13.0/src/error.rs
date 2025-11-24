use alloc::string::String;
use core::fmt;
use regex_automata::meta::BuildError as RaBuildError;

/// Result type for this crate with specific error enum.
pub type Result<T> = ::core::result::Result<T, Error>;

pub type ParseErrorPosition = usize;

/// An error as the result of parsing, compiling or running a regex.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Error {
    /// An error as a result of parsing a regex pattern, with the position where the error occurred
    ParseError(ParseErrorPosition, ParseError),
    /// An error as a result of compiling a regex
    CompileError(CompileError),
    /// An error as a result of running a regex
    RuntimeError(RuntimeError),
}

/// An error for the result of parsing a regex pattern.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// General parsing error
    GeneralParseError(String),
    /// Opening parenthesis without closing parenthesis, e.g. `(a|b`
    UnclosedOpenParen,
    /// Invalid repeat syntax
    InvalidRepeat,
    /// Pattern too deeply nested
    RecursionExceeded,
    /// Backslash without following character
    TrailingBackslash,
    /// Invalid escape
    InvalidEscape(String),
    /// Unicode escape not closed
    UnclosedUnicodeName,
    /// Invalid hex escape
    InvalidHex,
    /// Invalid codepoint for hex or unicode escape
    InvalidCodepointValue,
    /// Invalid character class
    InvalidClass,
    /// Unknown group flag
    UnknownFlag(String),
    /// Disabling Unicode not supported
    NonUnicodeUnsupported,
    /// Invalid back reference
    InvalidBackref,
    /// Quantifier on lookaround or other zero-width assertion
    TargetNotRepeatable,
    /// Couldn't parse group name
    InvalidGroupName,
    /// Invalid group id in escape sequence
    InvalidGroupNameBackref(String),
}

/// An error as the result of compiling a regex.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum CompileError {
    /// Regex crate error
    InnerError(RaBuildError),
    /// Look-behind assertion without constant size
    LookBehindNotConst,
    /// Couldn't parse group name
    InvalidGroupName,
    /// Invalid group id in escape sequence
    InvalidGroupNameBackref(String),
    /// Invalid back reference
    InvalidBackref,
    /// Once named groups are used you cannot refer to groups by number
    NamedBackrefOnly,
}

/// An error as the result of executing a regex.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum RuntimeError {
    /// Max stack size exceeded for backtracking while executing regex.
    StackOverflow,
    /// Max limit for backtracking count exceeded while executing the regex.
    /// Configure using
    /// [`RegexBuilder::backtrack_limit`](struct.RegexBuilder.html#method.backtrack_limit).
    BacktrackLimitExceeded,
}

#[cfg(feature = "std")]
impl ::std::error::Error for Error {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::GeneralParseError(s) => write!(f, "General parsing error: {}", s),
            ParseError::UnclosedOpenParen => {
                write!(f, "Opening parenthesis without closing parenthesis")
            }
            ParseError::InvalidRepeat => write!(f, "Invalid repeat syntax"),
            ParseError::RecursionExceeded => write!(f, "Pattern too deeply nested"),
            ParseError::TrailingBackslash => write!(f, "Backslash without following character"),
            ParseError::InvalidEscape(s) => write!(f, "Invalid escape: {}", s),
            ParseError::UnclosedUnicodeName => write!(f, "Unicode escape not closed"),
            ParseError::InvalidHex => write!(f, "Invalid hex escape"),
            ParseError::InvalidCodepointValue => {
                write!(f, "Invalid codepoint for hex or unicode escape")
            }
            ParseError::InvalidClass => write!(f, "Invalid character class"),
            ParseError::UnknownFlag(s) => write!(f, "Unknown group flag: {}", s),
            ParseError::NonUnicodeUnsupported => write!(f, "Disabling Unicode not supported"),
            ParseError::InvalidBackref => write!(f, "Invalid back reference"),
            ParseError::InvalidGroupName => write!(f, "Could not parse group name"),
            ParseError::InvalidGroupNameBackref(s) => {
                write!(f, "Invalid group name in back reference: {}", s)
            }
            ParseError::TargetNotRepeatable => write!(f, "Target of repeat operator is invalid"),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CompileError::InnerError(e) => write!(f, "Regex error: {}", e),
            CompileError::LookBehindNotConst => {
                write!(f, "Look-behind assertion without constant size")
            },
            CompileError::InvalidGroupName => write!(f, "Could not parse group name"),
            CompileError::InvalidGroupNameBackref(s) => write!(f, "Invalid group name in back reference: {}", s),
            CompileError::InvalidBackref => write!(f, "Invalid back reference"),
            CompileError::NamedBackrefOnly => write!(f, "Numbered backref/call not allowed because named group was used, use a named backref instead"),
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RuntimeError::StackOverflow => write!(f, "Max stack size exceeded for backtracking"),
            RuntimeError::BacktrackLimitExceeded => {
                write!(f, "Max limit for backtracking count exceeded")
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::ParseError(position, parse_error) => {
                write!(f, "Parsing error at position {}: {}", position, parse_error)
            }
            Error::CompileError(compile_error) => {
                write!(f, "Error compiling regex: {}", compile_error)
            }
            Error::RuntimeError(runtime_error) => {
                write!(f, "Error executing regex: {}", runtime_error)
            }
        }
    }
}

impl From<CompileError> for Error {
    fn from(compile_error: CompileError) -> Self {
        Error::CompileError(compile_error)
    }
}
