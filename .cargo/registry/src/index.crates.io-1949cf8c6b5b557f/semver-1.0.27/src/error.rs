use crate::parse::Error;
use core::fmt::{self, Debug, Display};

pub(crate) enum ErrorKind {
    Empty,
    UnexpectedEnd(Position),
    UnexpectedChar(Position, char),
    UnexpectedCharAfter(Position, char),
    ExpectedCommaFound(Position, char),
    LeadingZero(Position),
    Overflow(Position),
    EmptySegment(Position),
    IllegalCharacter(Position),
    WildcardNotTheOnlyComparator(char),
    UnexpectedAfterWildcard,
    ExcessiveComparators,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) enum Position {
    Major,
    Minor,
    Patch,
    Pre,
    Build,
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            ErrorKind::Empty => formatter.write_str("empty string, expected a semver version"),
            ErrorKind::UnexpectedEnd(pos) => {
                write!(formatter, "unexpected end of input while parsing {}", pos)
            }
            ErrorKind::UnexpectedChar(pos, ch) => {
                write!(
                    formatter,
                    "unexpected character {} while parsing {}",
                    QuotedChar(*ch),
                    pos,
                )
            }
            ErrorKind::UnexpectedCharAfter(pos, ch) => {
                write!(
                    formatter,
                    "unexpected character {} after {}",
                    QuotedChar(*ch),
                    pos,
                )
            }
            ErrorKind::ExpectedCommaFound(pos, ch) => {
                write!(
                    formatter,
                    "expected comma after {}, found {}",
                    pos,
                    QuotedChar(*ch),
                )
            }
            ErrorKind::LeadingZero(pos) => {
                write!(formatter, "invalid leading zero in {}", pos)
            }
            ErrorKind::Overflow(pos) => {
                write!(formatter, "value of {} exceeds u64::MAX", pos)
            }
            ErrorKind::EmptySegment(pos) => {
                write!(formatter, "empty identifier segment in {}", pos)
            }
            ErrorKind::IllegalCharacter(pos) => {
                write!(formatter, "unexpected character in {}", pos)
            }
            ErrorKind::WildcardNotTheOnlyComparator(ch) => {
                write!(
                    formatter,
                    "wildcard req ({}) must be the only comparator in the version req",
                    ch,
                )
            }
            ErrorKind::UnexpectedAfterWildcard => {
                formatter.write_str("unexpected character after wildcard in version req")
            }
            ErrorKind::ExcessiveComparators => {
                formatter.write_str("excessive number of version comparators")
            }
        }
    }
}

impl Display for Position {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(match self {
            Position::Major => "major version number",
            Position::Minor => "minor version number",
            Position::Patch => "patch version number",
            Position::Pre => "pre-release identifier",
            Position::Build => "build metadata",
        })
    }
}

impl Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Error(\"")?;
        Display::fmt(self, formatter)?;
        formatter.write_str("\")")?;
        Ok(())
    }
}

struct QuotedChar(char);

impl Display for QuotedChar {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        // Standard library versions prior to https://github.com/rust-lang/rust/pull/95345
        // print character 0 as '\u{0}'. We prefer '\0' to keep error messages
        // the same across all supported Rust versions.
        if self.0 == '\0' {
            formatter.write_str("'\\0'")
        } else {
            write!(formatter, "{:?}", self.0)
        }
    }
}
