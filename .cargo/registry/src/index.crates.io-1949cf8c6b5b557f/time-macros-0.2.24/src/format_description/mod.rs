//! Parser for format descriptions.

macro_rules! version {
    ($range:expr) => {
        $range.contains(&VERSION)
    };
}

mod ast;
mod format_item;
mod lexer;
mod public;

pub(crate) fn parse_with_version(
    version: Option<crate::FormatDescriptionVersion>,
    s: &[u8],
    proc_span: proc_macro::Span,
) -> Result<Vec<public::OwnedFormatItem>, crate::Error> {
    match version {
        Some(crate::FormatDescriptionVersion::V1) | None => parse::<1>(s, proc_span),
        Some(crate::FormatDescriptionVersion::V2) => parse::<2>(s, proc_span),
    }
}

fn parse<const VERSION: u8>(
    s: &[u8],
    proc_span: proc_macro::Span,
) -> Result<Vec<public::OwnedFormatItem>, crate::Error> {
    let mut lexed = lexer::lex::<VERSION>(s, proc_span);
    let ast = ast::parse::<_, VERSION>(&mut lexed);
    let format_items = format_item::parse(ast);
    Ok(format_items
        .map(|res| res.map(Into::into))
        .collect::<Result<_, _>>()?)
}

#[derive(Clone, Copy)]
struct Location {
    byte: u32,
    proc_span: proc_macro::Span,
}

impl Location {
    fn to(self, end: Self) -> Span {
        Span { start: self, end }
    }

    #[must_use = "this does not modify the original value"]
    fn offset(&self, offset: u32) -> Self {
        Self {
            byte: self.byte + offset,
            proc_span: self.proc_span,
        }
    }

    fn error(self, message: &'static str) -> Error {
        Error {
            message,
            _span: unused(Span {
                start: self,
                end: self,
            }),
            proc_span: self.proc_span,
        }
    }
}

#[derive(Clone, Copy)]
struct Span {
    start: Location,
    end: Location,
}

impl Span {
    #[must_use = "this does not modify the original value"]
    const fn shrink_to_start(&self) -> Self {
        Self {
            start: self.start,
            end: self.start,
        }
    }

    #[must_use = "this does not modify the original value"]
    const fn shrink_to_end(&self) -> Self {
        Self {
            start: self.end,
            end: self.end,
        }
    }

    #[must_use = "this does not modify the original value"]
    const fn shrink_to_before(&self, pos: u32) -> Self {
        Self {
            start: self.start,
            end: Location {
                byte: self.start.byte + pos - 1,
                proc_span: self.start.proc_span,
            },
        }
    }

    #[must_use = "this does not modify the original value"]
    fn shrink_to_after(&self, pos: u32) -> Self {
        Self {
            start: Location {
                byte: self.start.byte + pos + 1,
                proc_span: self.start.proc_span,
            },
            end: self.end,
        }
    }

    fn error(self, message: &'static str) -> Error {
        Error {
            message,
            _span: unused(self),
            proc_span: self.start.proc_span,
        }
    }
}

#[derive(Clone, Copy)]
struct Spanned<T> {
    value: T,
    span: Span,
}

impl<T> core::ops::Deref for Spanned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

trait SpannedValue: Sized {
    fn spanned(self, span: Span) -> Spanned<Self>;
}

impl<T> SpannedValue for T {
    fn spanned(self, span: Span) -> Spanned<Self> {
        Spanned { value: self, span }
    }
}

struct Error {
    message: &'static str,
    _span: Unused<Span>,
    proc_span: proc_macro::Span,
}

impl From<Error> for crate::Error {
    fn from(error: Error) -> Self {
        Self::Custom {
            message: error.message.into(),
            span_start: Some(error.proc_span),
            span_end: Some(error.proc_span),
        }
    }
}

struct Unused<T>(core::marker::PhantomData<T>);

fn unused<T>(_: T) -> Unused<T> {
    Unused(core::marker::PhantomData)
}
