//! Lexed TOML tokens

use super::Span;
use super::APOSTROPHE;
use super::COMMENT_START_SYMBOL;
use super::QUOTATION_MARK;
use super::WSCHAR;
use crate::decoder::Encoding;

/// An unvalidated TOML Token
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Token {
    pub(super) kind: TokenKind,
    pub(super) span: Span,
}

impl Token {
    pub(super) fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    #[inline(always)]
    pub fn kind(&self) -> TokenKind {
        self.kind
    }

    #[inline(always)]
    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum TokenKind {
    /// Either for dotted-key or float
    Dot = b'.',
    /// Key-value separator
    Equals = b'=',
    /// Value separator
    Comma = b',',
    /// Either array or standard-table start
    LeftSquareBracket = b'[',
    /// Either array or standard-table end
    RightSquareBracket = b']',
    /// Inline table start
    LeftCurlyBracket = b'{',
    /// Inline table end
    RightCurlyBracket = b'}',
    Whitespace = WSCHAR.0,
    Comment = COMMENT_START_SYMBOL,
    Newline = b'\n',
    LiteralString = APOSTROPHE,
    BasicString = QUOTATION_MARK,
    MlLiteralString = 1,
    MlBasicString,

    /// Anything else
    Atom,

    Eof,
}

impl TokenKind {
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Dot => "`.`",
            Self::Equals => "`=`",
            Self::Comma => "`,`",
            Self::LeftSquareBracket => "`[`",
            Self::RightSquareBracket => "`]`",
            Self::LeftCurlyBracket => "`{`",
            Self::RightCurlyBracket => "`}`",
            Self::Whitespace => "whitespace",
            Self::Comment => "comment",
            Self::Newline => "newline",
            Self::LiteralString => "literal string",
            Self::BasicString => "basic string",
            Self::MlLiteralString => "multi-line literal string",
            Self::MlBasicString => "multi-line basic string",
            Self::Atom => "token",
            Self::Eof => "end-of-input",
        }
    }

    pub fn encoding(&self) -> Option<Encoding> {
        match self {
            Self::LiteralString => Some(Encoding::LiteralString),
            Self::BasicString => Some(Encoding::BasicString),
            Self::MlLiteralString => Some(Encoding::MlLiteralString),
            Self::MlBasicString => Some(Encoding::MlBasicString),
            Self::Atom
            | Self::LeftSquareBracket
            | Self::RightSquareBracket
            | Self::Dot
            | Self::Equals
            | Self::Comma
            | Self::RightCurlyBracket
            | Self::LeftCurlyBracket
            | Self::Whitespace
            | Self::Newline
            | Self::Comment
            | Self::Eof => None,
        }
    }
}
