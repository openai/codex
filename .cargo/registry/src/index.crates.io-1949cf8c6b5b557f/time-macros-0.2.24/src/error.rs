use std::borrow::Cow;
use std::fmt;

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

trait WithSpan {
    fn with_span(self, span: Span) -> Self;
}

impl WithSpan for TokenTree {
    fn with_span(mut self, span: Span) -> Self {
        self.set_span(span);
        self
    }
}

pub(crate) enum Error {
    MissingComponent {
        name: &'static str,
        span_start: Option<Span>,
        span_end: Option<Span>,
    },
    InvalidComponent {
        name: &'static str,
        value: String,
        span_start: Option<Span>,
        span_end: Option<Span>,
    },
    #[cfg(any(feature = "formatting", feature = "parsing"))]
    ExpectedString {
        span_start: Option<Span>,
        span_end: Option<Span>,
    },
    UnexpectedToken {
        tree: TokenTree,
    },
    UnexpectedEndOfInput,
    Custom {
        message: Cow<'static, str>,
        span_start: Option<Span>,
        span_end: Option<Span>,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingComponent { name, .. } => write!(f, "missing component: {name}"),
            Self::InvalidComponent { name, value, .. } => {
                write!(f, "invalid component: {name} was {value}")
            }
            #[cfg(any(feature = "formatting", feature = "parsing"))]
            Self::ExpectedString { .. } => f.write_str("expected string literal"),
            Self::UnexpectedToken { tree } => write!(f, "unexpected token: {tree}"),
            Self::UnexpectedEndOfInput => f.write_str("unexpected end of input"),
            Self::Custom { message, .. } => f.write_str(message),
        }
    }
}

impl Error {
    fn span_start(&self) -> Span {
        match self {
            Self::MissingComponent { span_start, .. }
            | Self::InvalidComponent { span_start, .. }
            | Self::Custom { span_start, .. } => *span_start,
            #[cfg(any(feature = "formatting", feature = "parsing"))]
            Self::ExpectedString { span_start, .. } => *span_start,
            Self::UnexpectedToken { tree } => Some(tree.span()),
            Self::UnexpectedEndOfInput => Some(Span::mixed_site()),
        }
        .unwrap_or_else(Span::mixed_site)
    }

    fn span_end(&self) -> Span {
        match self {
            Self::MissingComponent { span_end, .. }
            | Self::InvalidComponent { span_end, .. }
            | Self::Custom { span_end, .. } => *span_end,
            #[cfg(any(feature = "formatting", feature = "parsing"))]
            Self::ExpectedString { span_end, .. } => *span_end,
            Self::UnexpectedToken { tree, .. } => Some(tree.span()),
            Self::UnexpectedEndOfInput => Some(Span::mixed_site()),
        }
        .unwrap_or_else(|| self.span_start())
    }

    pub(crate) fn to_compile_error(&self) -> TokenStream {
        let (start, end) = (self.span_start(), self.span_end());

        [
            TokenTree::from(Punct::new(':', Spacing::Joint)).with_span(start),
            TokenTree::from(Punct::new(':', Spacing::Alone)).with_span(start),
            TokenTree::from(Ident::new("core", start)),
            TokenTree::from(Punct::new(':', Spacing::Joint)).with_span(start),
            TokenTree::from(Punct::new(':', Spacing::Alone)).with_span(start),
            TokenTree::from(Ident::new("compile_error", start)),
            TokenTree::from(Punct::new('!', Spacing::Alone)).with_span(start),
            TokenTree::from(Group::new(
                Delimiter::Parenthesis,
                TokenStream::from(
                    TokenTree::from(Literal::string(&self.to_string())).with_span(end),
                ),
            ))
            .with_span(end),
        ]
        .iter()
        .cloned()
        .collect()
    }

    /// Like `to_compile_error`, but for use in macros that produce items.
    #[cfg(all(feature = "serde", any(feature = "formatting", feature = "parsing")))]
    pub(crate) fn to_compile_error_standalone(&self) -> TokenStream {
        let end = self.span_end();
        self.to_compile_error()
            .into_iter()
            .chain(std::iter::once(
                TokenTree::from(Punct::new(';', Spacing::Alone)).with_span(end),
            ))
            .collect()
    }
}
