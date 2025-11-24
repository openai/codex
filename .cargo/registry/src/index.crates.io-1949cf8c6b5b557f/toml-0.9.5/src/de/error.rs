use crate::alloc_prelude::*;

/// Errors that can occur when deserializing a type.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Error {
    message: String,
    input: Option<alloc::sync::Arc<str>>,
    keys: Vec<String>,
    span: Option<core::ops::Range<usize>>,
}

impl Error {
    #[cfg(feature = "parse")]
    pub(crate) fn new(input: alloc::sync::Arc<str>, error: toml_parser::ParseError) -> Self {
        let mut message = String::new();
        message.push_str(error.description());
        if let Some(expected) = error.expected() {
            message.push_str(", expected ");
            if expected.is_empty() {
                message.push_str("nothing");
            } else {
                for (i, expected) in expected.iter().enumerate() {
                    if i != 0 {
                        message.push_str(", ");
                    }
                    match expected {
                        toml_parser::Expected::Literal(desc) => {
                            message.push_str(&render_literal(desc));
                        }
                        toml_parser::Expected::Description(desc) => message.push_str(desc),
                        _ => message.push_str("etc"),
                    }
                }
            }
        }

        let span = error.unexpected().map(|span| span.start()..span.end());

        Self {
            message,
            input: Some(input),
            keys: Vec::new(),
            span,
        }
    }

    pub(crate) fn custom<T>(msg: T, span: Option<core::ops::Range<usize>>) -> Self
    where
        T: core::fmt::Display,
    {
        Self {
            message: msg.to_string(),
            input: None,
            keys: Vec::new(),
            span,
        }
    }

    pub(crate) fn add_key(&mut self, key: String) {
        self.keys.insert(0, key);
    }

    /// What went wrong
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The start/end index into the original document where the error occurred
    pub fn span(&self) -> Option<core::ops::Range<usize>> {
        self.span.clone()
    }

    pub(crate) fn set_span(&mut self, span: Option<core::ops::Range<usize>>) {
        self.span = span;
    }

    /// Provide the encoded TOML the error applies to
    pub fn set_input(&mut self, input: Option<&str>) {
        self.input = input.map(|s| s.into());
    }
}

#[cfg(feature = "serde")]
impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: core::fmt::Display,
    {
        Self::custom(msg.to_string(), None)
    }
}

fn render_literal(literal: &str) -> String {
    match literal {
        "\n" => "newline".to_owned(),
        "`" => "'`'".to_owned(),
        s if s.chars().all(|c| c.is_ascii_control()) => {
            format!("`{}`", s.escape_debug())
        }
        s => format!("`{s}`"),
    }
}

/// Displays a TOML parse error
///
/// # Example
///
/// TOML parse error at line 1, column 10
///   |
/// 1 | 00:32:00.a999999
///   |          ^
/// Unexpected `a`
/// Expected `digit`
/// While parsing a Time
/// While parsing a Date-Time
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut context = false;
        if let (Some(input), Some(span)) = (&self.input, self.span()) {
            context = true;

            let (line, column) = translate_position(input.as_bytes(), span.start);
            let line_num = line + 1;
            let col_num = column + 1;
            let gutter = line_num.to_string().len();
            let content = input.split('\n').nth(line).expect("valid line number");
            let highlight_len = span.end - span.start;
            // Allow highlight to go one past the line
            let highlight_len = highlight_len.min(content.len().saturating_sub(column));

            writeln!(f, "TOML parse error at line {line_num}, column {col_num}")?;
            //   |
            for _ in 0..=gutter {
                write!(f, " ")?;
            }
            writeln!(f, "|")?;

            // 1 | 00:32:00.a999999
            write!(f, "{line_num} | ")?;
            writeln!(f, "{content}")?;

            //   |          ^
            for _ in 0..=gutter {
                write!(f, " ")?;
            }
            write!(f, "|")?;
            for _ in 0..=column {
                write!(f, " ")?;
            }
            // The span will be empty at eof, so we need to make sure we always print at least
            // one `^`
            write!(f, "^")?;
            for _ in 1..highlight_len {
                write!(f, "^")?;
            }
            writeln!(f)?;
        }
        writeln!(f, "{}", self.message)?;
        if !context && !self.keys.is_empty() {
            writeln!(f, "in `{}`", self.keys.join("."))?;
        }

        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
#[cfg(not(feature = "std"))]
#[cfg(feature = "serde")]
impl serde::de::StdError for Error {}

fn translate_position(input: &[u8], index: usize) -> (usize, usize) {
    if input.is_empty() {
        return (0, index);
    }

    let safe_index = index.min(input.len() - 1);
    let column_offset = index - safe_index;
    let index = safe_index;

    let nl = input[0..index]
        .iter()
        .rev()
        .enumerate()
        .find(|(_, b)| **b == b'\n')
        .map(|(nl, _)| index - nl - 1);
    let line_start = match nl {
        Some(nl) => nl + 1,
        None => 0,
    };
    let line = input[0..line_start].iter().filter(|b| **b == b'\n').count();

    let column = core::str::from_utf8(&input[line_start..=index])
        .map(|s| s.chars().count() - 1)
        .unwrap_or_else(|_| index - line_start);
    let column = column + column_offset;

    (line, column)
}

#[cfg(feature = "parse")]
pub(crate) struct TomlSink<'i, S> {
    source: toml_parser::Source<'i>,
    input: Option<alloc::sync::Arc<str>>,
    sink: S,
}

#[cfg(feature = "parse")]
impl<'i, S: Default> TomlSink<'i, S> {
    pub(crate) fn new(source: toml_parser::Source<'i>) -> Self {
        Self {
            source,
            input: None,
            sink: Default::default(),
        }
    }

    pub(crate) fn into_inner(self) -> S {
        self.sink
    }
}

#[cfg(feature = "parse")]
impl<'i> toml_parser::ErrorSink for TomlSink<'i, Option<Error>> {
    fn report_error(&mut self, error: toml_parser::ParseError) {
        if self.sink.is_none() {
            let input = self
                .input
                .get_or_insert_with(|| alloc::sync::Arc::from(self.source.input()));
            let error = Error::new(input.clone(), error);
            self.sink = Some(error);
        }
    }
}

#[cfg(feature = "parse")]
impl<'i> toml_parser::ErrorSink for TomlSink<'i, Vec<Error>> {
    fn report_error(&mut self, error: toml_parser::ParseError) {
        let input = self
            .input
            .get_or_insert_with(|| alloc::sync::Arc::from(self.source.input()));
        let error = Error::new(input.clone(), error);
        self.sink.push(error);
    }
}

#[cfg(test)]
mod test_translate_position {
    use super::*;

    #[test]
    fn empty() {
        let input = b"";
        let index = 0;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, 0));
    }

    #[test]
    fn start() {
        let input = b"Hello";
        let index = 0;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, 0));
    }

    #[test]
    fn end() {
        let input = b"Hello";
        let index = input.len() - 1;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, input.len() - 1));
    }

    #[test]
    fn after() {
        let input = b"Hello";
        let index = input.len();
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, input.len()));
    }

    #[test]
    fn first_line() {
        let input = b"Hello\nWorld\n";
        let index = 2;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, 2));
    }

    #[test]
    fn end_of_line() {
        let input = b"Hello\nWorld\n";
        let index = 5;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, 5));
    }

    #[test]
    fn start_of_second_line() {
        let input = b"Hello\nWorld\n";
        let index = 6;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (1, 0));
    }

    #[test]
    fn second_line() {
        let input = b"Hello\nWorld\n";
        let index = 8;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (1, 2));
    }
}
