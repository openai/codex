use crate::Span;

pub trait ErrorSink {
    fn report_error(&mut self, error: ParseError);
}

impl<F> ErrorSink for F
where
    F: FnMut(ParseError),
{
    fn report_error(&mut self, error: ParseError) {
        (self)(error);
    }
}

impl ErrorSink for () {
    fn report_error(&mut self, _error: ParseError) {}
}

impl ErrorSink for Option<ParseError> {
    fn report_error(&mut self, error: ParseError) {
        self.get_or_insert(error);
    }
}

#[cfg(feature = "alloc")]
#[allow(unused_qualifications)]
impl ErrorSink for alloc::vec::Vec<ParseError> {
    fn report_error(&mut self, error: ParseError) {
        self.push(error);
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub struct ParseError {
    context: Option<Span>,
    description: ErrorStr,
    expected: Option<&'static [Expected]>,
    unexpected: Option<Span>,
}

impl ParseError {
    pub fn new(description: impl Into<ErrorStr>) -> Self {
        Self {
            context: None,
            description: description.into(),
            expected: None,
            unexpected: None,
        }
    }

    pub fn with_context(mut self, context: Span) -> Self {
        self.context = Some(context);
        self
    }

    pub fn with_expected(mut self, expected: &'static [Expected]) -> Self {
        self.expected = Some(expected);
        self
    }

    pub fn with_unexpected(mut self, unexpected: Span) -> Self {
        self.unexpected = Some(unexpected);
        self
    }

    pub fn context(&self) -> Option<Span> {
        self.context
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn expected(&self) -> Option<&'static [Expected]> {
        self.expected
    }
    pub fn unexpected(&self) -> Option<Span> {
        self.unexpected
    }

    pub(crate) fn rebase_spans(mut self, offset: usize) -> Self {
        if let Some(context) = self.context.as_mut() {
            *context += offset;
        }
        if let Some(unexpected) = self.unexpected.as_mut() {
            *unexpected += offset;
        }
        self
    }
}

#[cfg(feature = "alloc")]
type ErrorStr = alloc::borrow::Cow<'static, str>;
#[cfg(not(feature = "alloc"))]
type ErrorStr = &'static str;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum Expected {
    Literal(&'static str),
    Description(&'static str),
}
