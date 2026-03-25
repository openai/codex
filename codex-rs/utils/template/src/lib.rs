//! Minimal strict templating for prompt and text assets.
//!
//! Supported syntax:
//! - `{{ name }}` placeholder interpolation
//! - `{{{{` for a literal `{{`
//! - `}}}}` for a literal `}}`

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateParseError {
    EmptyPlaceholder { start: usize },
    NestedPlaceholder { start: usize },
    UnmatchedClosingDelimiter { start: usize },
    UnterminatedPlaceholder { start: usize },
}

impl fmt::Display for TemplateParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPlaceholder { start } => {
                write!(f, "template placeholder at byte {start} is empty")
            }
            Self::NestedPlaceholder { start } => {
                write!(
                    f,
                    "template placeholder starting at byte {start} contains a nested `{{`"
                )
            }
            Self::UnmatchedClosingDelimiter { start } => {
                write!(f, "template contains an unmatched `}}` at byte {start}")
            }
            Self::UnterminatedPlaceholder { start } => {
                write!(
                    f,
                    "template placeholder starting at byte {start} is missing `}}`"
                )
            }
        }
    }
}

impl Error for TemplateParseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateRenderError {
    DuplicateValue { name: String },
    ExtraValue { name: String },
    MissingValue { name: String },
}

impl fmt::Display for TemplateRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateValue { name } => {
                write!(f, "template value `{name}` was provided more than once")
            }
            Self::ExtraValue { name } => {
                write!(f, "template value `{name}` is not used by this template")
            }
            Self::MissingValue { name } => {
                write!(f, "template placeholder `{name}` is missing a value")
            }
        }
    }
}

impl Error for TemplateRenderError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    Parse(TemplateParseError),
    Render(TemplateRenderError),
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => err.fmt(f),
            Self::Render(err) => err.fmt(f),
        }
    }
}

impl Error for TemplateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Parse(err) => Some(err),
            Self::Render(err) => Some(err),
        }
    }
}

impl From<TemplateParseError> for TemplateError {
    fn from(value: TemplateParseError) -> Self {
        Self::Parse(value)
    }
}

impl From<TemplateRenderError> for TemplateError {
    fn from(value: TemplateRenderError) -> Self {
        Self::Render(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Literal(String),
    Placeholder(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    placeholders: BTreeSet<String>,
    segments: Vec<Segment>,
}

impl Template {
    pub fn parse(source: &str) -> Result<Self, TemplateParseError> {
        let mut placeholders = BTreeSet::new();
        let mut segments = Vec::new();
        let mut literal_start = 0usize;
        let mut cursor = 0usize;

        while cursor < source.len() {
            let rest = &source[cursor..];
            if rest.starts_with("{{{{") {
                push_literal(&mut segments, &source[literal_start..cursor]);
                push_literal(&mut segments, "{{");
                cursor += "{{{{".len();
                literal_start = cursor;
                continue;
            }
            if rest.starts_with("}}}}") {
                push_literal(&mut segments, &source[literal_start..cursor]);
                push_literal(&mut segments, "}}");
                cursor += "}}}}".len();
                literal_start = cursor;
                continue;
            }
            if rest.starts_with("{{") {
                push_literal(&mut segments, &source[literal_start..cursor]);
                let (placeholder, next_cursor) = parse_placeholder(source, cursor)?;
                placeholders.insert(placeholder.clone());
                segments.push(Segment::Placeholder(placeholder));
                cursor = next_cursor;
                literal_start = cursor;
                continue;
            }
            if rest.starts_with("}}") {
                return Err(TemplateParseError::UnmatchedClosingDelimiter { start: cursor });
            }

            let ch = rest
                .chars()
                .next()
                .expect("cursor is always on a char boundary");
            cursor += ch.len_utf8();
        }

        push_literal(&mut segments, &source[literal_start..]);
        Ok(Self {
            placeholders,
            segments,
        })
    }

    pub fn placeholders(&self) -> impl ExactSizeIterator<Item = &str> {
        self.placeholders.iter().map(String::as_str)
    }

    pub fn render<I, K, V>(&self, variables: I) -> Result<String, TemplateRenderError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let variables = build_variable_map(variables)?;

        for placeholder in &self.placeholders {
            if !variables.contains_key(placeholder.as_str()) {
                return Err(TemplateRenderError::MissingValue {
                    name: placeholder.clone(),
                });
            }
        }

        for name in variables.keys() {
            if !self.placeholders.contains(name.as_str()) {
                return Err(TemplateRenderError::ExtraValue { name: name.clone() });
            }
        }

        let mut rendered = String::new();
        for segment in &self.segments {
            match segment {
                Segment::Literal(literal) => rendered.push_str(literal),
                Segment::Placeholder(name) => rendered.push_str(
                    variables
                        .get(name.as_str())
                        .expect("missing values are validated before rendering"),
                ),
            }
        }
        Ok(rendered)
    }
}

pub fn render<I, K, V>(template: &str, variables: I) -> Result<String, TemplateError>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    Template::parse(template)?
        .render(variables)
        .map_err(Into::into)
}

fn push_literal(segments: &mut Vec<Segment>, literal: &str) {
    if literal.is_empty() {
        return;
    }

    if let Some(Segment::Literal(existing)) = segments.last_mut() {
        existing.push_str(literal);
    } else {
        segments.push(Segment::Literal(literal.to_string()));
    }
}

fn parse_placeholder(source: &str, start: usize) -> Result<(String, usize), TemplateParseError> {
    let placeholder_start = start + "{{".len();
    let mut cursor = placeholder_start;

    while cursor < source.len() {
        let rest = &source[cursor..];
        if rest.starts_with("{{") {
            return Err(TemplateParseError::NestedPlaceholder { start });
        }
        if rest.starts_with("}}") {
            let placeholder = source[placeholder_start..cursor].trim();
            if placeholder.is_empty() {
                return Err(TemplateParseError::EmptyPlaceholder { start });
            }
            return Ok((placeholder.to_string(), cursor + "}}".len()));
        }

        let ch = rest
            .chars()
            .next()
            .expect("cursor is always on a char boundary");
        cursor += ch.len_utf8();
    }

    Err(TemplateParseError::UnterminatedPlaceholder { start })
}

fn build_variable_map<I, K, V>(
    variables: I,
) -> Result<BTreeMap<String, String>, TemplateRenderError>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut map = BTreeMap::new();
    for (name, value) in variables {
        let name = name.as_ref().to_string();
        if map
            .insert(name.clone(), value.as_ref().to_string())
            .is_some()
        {
            return Err(TemplateRenderError::DuplicateValue { name });
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::Template;
    use super::TemplateError;
    use super::TemplateParseError;
    use super::TemplateRenderError;
    use super::render;
    use pretty_assertions::assert_eq;

    #[test]
    fn render_replaces_placeholders_with_and_without_whitespace() {
        let rendered = render(
            "Hello, {{ name }}. You are in {{place}}. {{ name }} is repeated.",
            [("name", "Codex"), ("place", "codex-rs")],
        )
        .unwrap();

        assert_eq!(
            rendered,
            "Hello, Codex. You are in codex-rs. Codex is repeated."
        );
    }

    #[test]
    fn parsed_templates_can_be_reused() {
        let template = Template::parse("{{greeting}}, {{ name }}!").unwrap();

        assert_eq!(
            template.render([("greeting", "Hello"), ("name", "Codex")]),
            Ok("Hello, Codex!".to_string())
        );
        assert_eq!(
            template.render([("greeting", "Hi"), ("name", "builder")]),
            Ok("Hi, builder!".to_string())
        );
    }

    #[test]
    fn placeholders_are_sorted_and_unique() {
        let template = Template::parse("{{ b }} {{ a }} {{ b }}").unwrap();

        assert_eq!(template.placeholders().collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn render_supports_multiline_templates_and_adjacent_placeholders() {
        let rendered = render(
            "Line 1: {{first}}{{second}}\nLine 2: {{ third }}",
            [("first", "A"), ("second", "B"), ("third", "C")],
        )
        .unwrap();

        assert_eq!(rendered, "Line 1: AB\nLine 2: C");
    }

    #[test]
    fn render_supports_literal_delimiter_escapes() {
        let rendered = render(
            "literal open: {{{{, literal close: }}}}, value: {{ name }}",
            [("name", "Codex")],
        )
        .unwrap();

        assert_eq!(
            rendered,
            "literal open: {{, literal close: }}, value: Codex"
        );
    }

    #[test]
    fn parse_errors_when_placeholder_is_empty() {
        let err = Template::parse("Hello, {{   }}.").unwrap_err();

        assert_eq!(err, TemplateParseError::EmptyPlaceholder { start: 7 });
    }

    #[test]
    fn parse_errors_when_placeholder_is_unterminated() {
        let err = Template::parse("Hello, {{ name.").unwrap_err();

        assert_eq!(
            err,
            TemplateParseError::UnterminatedPlaceholder { start: 7 }
        );
    }

    #[test]
    fn parse_errors_when_placeholder_is_nested() {
        let err = Template::parse("Hello, {{ outer {{ inner }} }}.").unwrap_err();

        assert_eq!(err, TemplateParseError::NestedPlaceholder { start: 7 });
    }

    #[test]
    fn parse_errors_when_closing_delimiter_is_unmatched() {
        let err = Template::parse("Hello, }} world.").unwrap_err();

        assert_eq!(
            err,
            TemplateParseError::UnmatchedClosingDelimiter { start: 7 }
        );
    }

    #[test]
    fn render_errors_when_placeholder_is_missing() {
        let template = Template::parse("Hello, {{ name }}.").unwrap();

        assert_eq!(
            template.render(Vec::<(&str, &str)>::new()),
            Err(TemplateRenderError::MissingValue {
                name: "name".to_string()
            })
        );
    }

    #[test]
    fn render_errors_when_extra_value_is_provided() {
        let template = Template::parse("Hello, {{ name }}.").unwrap();

        assert_eq!(
            template.render([("name", "Codex"), ("unused", "extra")]),
            Err(TemplateRenderError::ExtraValue {
                name: "unused".to_string()
            })
        );
    }

    #[test]
    fn render_errors_when_duplicate_value_is_provided() {
        let template = Template::parse("Hello, {{ name }}.").unwrap();

        assert_eq!(
            template.render([("name", "Codex"), ("name", "other")]),
            Err(TemplateRenderError::DuplicateValue {
                name: "name".to_string()
            })
        );
    }

    #[test]
    fn render_function_wraps_parse_errors() {
        let err = render("Hello, }} world.", [("name", "Codex")]).unwrap_err();

        assert_eq!(
            err,
            TemplateError::Parse(TemplateParseError::UnmatchedClosingDelimiter { start: 7 })
        );
    }

    #[test]
    fn render_function_wraps_render_errors() {
        let err = render("Hello, {{ name }}.", [("extra", "Codex")]).unwrap_err();

        assert_eq!(
            err,
            TemplateError::Render(TemplateRenderError::MissingValue {
                name: "name".to_string()
            })
        );
    }
}
