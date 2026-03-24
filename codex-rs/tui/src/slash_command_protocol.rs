use std::collections::HashMap;
use std::marker::PhantomData;
use std::str::FromStr;

use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;
use shlex::Shlex;
use shlex::try_join;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlashCommandUsageErrorKind {
    UnexpectedInlineArgs,
    InvalidInlineArgs,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SlashCommandParseInput<'a> {
    pub(crate) args: &'a str,
    pub(crate) text_elements: &'a [TextElement],
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SlashSerializedText {
    pub(crate) text: String,
    pub(crate) text_elements: Vec<TextElement>,
}

impl SlashSerializedText {
    pub(crate) fn empty() -> Self {
        Self {
            text: String::new(),
            text_elements: Vec::new(),
        }
    }

    pub(crate) fn with_prefix(&self, prefix: &str) -> Self {
        if self.text.is_empty() {
            return Self {
                text: prefix.to_string(),
                text_elements: Vec::new(),
            };
        }

        let offset = prefix.len() + 1;
        Self {
            text: format!("{prefix} {}", self.text),
            text_elements: shift_text_elements_right(&self.text_elements, offset),
        }
    }

    #[allow(dead_code)]
    fn prepend_inline(&self, prefix: &str) -> Self {
        if prefix.is_empty() {
            return self.clone();
        }

        Self {
            text: format!("{prefix}{}", self.text),
            text_elements: shift_text_elements_right(&self.text_elements, prefix.len()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SlashTokenArg {
    pub(crate) text: String,
    pub(crate) text_elements: Vec<TextElement>,
}

impl SlashTokenArg {
    pub(crate) fn new(text: String, text_elements: Vec<TextElement>) -> Self {
        Self {
            text,
            text_elements,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SlashTextArg {
    pub(crate) text: String,
    pub(crate) text_elements: Vec<TextElement>,
}

impl SlashTextArg {
    pub(crate) fn new(text: String, text_elements: Vec<TextElement>) -> Self {
        Self {
            text,
            text_elements,
        }
    }
}

pub(crate) trait SlashTokenValueSpec<T> {
    fn parse_token(&self, token: SlashTokenArg) -> Result<T, SlashCommandUsageErrorKind>;
    fn serialize_token(&self, value: &T) -> SlashTokenArg;
}

pub(crate) trait SlashTextValueSpec<T> {
    fn parse_text(&self, text: SlashTextArg) -> Result<T, SlashCommandUsageErrorKind>;
    fn serialize_text(&self, value: &T) -> SlashTextArg;
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct SlashTokenSpec;

#[allow(dead_code)]
pub(crate) fn token() -> SlashTokenSpec {
    SlashTokenSpec
}

impl SlashTokenValueSpec<SlashTokenArg> for SlashTokenSpec {
    fn parse_token(
        &self,
        token: SlashTokenArg,
    ) -> Result<SlashTokenArg, SlashCommandUsageErrorKind> {
        Ok(token)
    }

    fn serialize_token(&self, value: &SlashTokenArg) -> SlashTokenArg {
        value.clone()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SlashStringSpec;

pub(crate) fn string() -> SlashStringSpec {
    SlashStringSpec
}

impl SlashTokenValueSpec<String> for SlashStringSpec {
    fn parse_token(&self, token: SlashTokenArg) -> Result<String, SlashCommandUsageErrorKind> {
        Ok(token.text)
    }

    fn serialize_token(&self, value: &String) -> SlashTokenArg {
        SlashTokenArg::new(value.clone(), Vec::new())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SlashTextSpec;

pub(crate) fn text() -> SlashTextSpec {
    SlashTextSpec
}

impl SlashTextValueSpec<SlashTextArg> for SlashTextSpec {
    fn parse_text(&self, text: SlashTextArg) -> Result<SlashTextArg, SlashCommandUsageErrorKind> {
        Ok(text)
    }

    fn serialize_text(&self, value: &SlashTextArg) -> SlashTextArg {
        value.clone()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SlashEnumChoiceSpec<T: 'static> {
    choices: &'static [(&'static str, T)],
    ascii_case_insensitive: bool,
}

pub(crate) fn enum_choice<T>(choices: &'static [(&'static str, T)]) -> SlashEnumChoiceSpec<T>
where
    T: Clone + PartialEq + 'static,
{
    SlashEnumChoiceSpec {
        choices,
        ascii_case_insensitive: false,
    }
}

impl<T> SlashEnumChoiceSpec<T> {
    pub(crate) fn ascii_case_insensitive(mut self) -> Self {
        self.ascii_case_insensitive = true;
        self
    }
}

impl<T> SlashTokenValueSpec<T> for SlashEnumChoiceSpec<T>
where
    T: Clone + PartialEq + 'static,
{
    fn parse_token(&self, token: SlashTokenArg) -> Result<T, SlashCommandUsageErrorKind> {
        self.choices
            .iter()
            .find_map(|(literal, value)| {
                let matches = if self.ascii_case_insensitive {
                    token.text.eq_ignore_ascii_case(literal)
                } else {
                    token.text == *literal
                };
                matches.then(|| value.clone())
            })
            .ok_or(SlashCommandUsageErrorKind::InvalidInlineArgs)
    }

    fn serialize_token(&self, value: &T) -> SlashTokenArg {
        let literal = match self
            .choices
            .iter()
            .find_map(|(literal, choice)| (choice == value).then_some(*literal))
        {
            Some(literal) => literal,
            None => panic!("missing enum choice serializer mapping"),
        };
        SlashTokenArg::new(literal.to_string(), Vec::new())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SlashFromStrSpec<T> {
    _phantom: PhantomData<T>,
}

pub(crate) fn from_str_value<T>() -> SlashFromStrSpec<T>
where
    T: FromStr + ToString,
{
    SlashFromStrSpec {
        _phantom: PhantomData,
    }
}

impl<T> SlashTokenValueSpec<T> for SlashFromStrSpec<T>
where
    T: FromStr + ToString,
{
    fn parse_token(&self, token: SlashTokenArg) -> Result<T, SlashCommandUsageErrorKind> {
        token
            .text
            .parse()
            .map_err(|_| SlashCommandUsageErrorKind::InvalidInlineArgs)
    }

    fn serialize_token(&self, value: &T) -> SlashTokenArg {
        SlashTokenArg::new(value.to_string(), Vec::new())
    }
}

#[derive(Debug)]
pub(crate) struct SlashArgsParser<'a> {
    input: SlashCommandParseInput<'a>,
    positionals: Vec<SlashTokenArg>,
    next_positional: usize,
    named: HashMap<String, SlashTokenArg>,
    duplicates: HashMap<String, usize>,
}

impl<'a> SlashArgsParser<'a> {
    pub(crate) fn new(
        input: SlashCommandParseInput<'a>,
    ) -> Result<Self, SlashCommandUsageErrorKind> {
        let mut positionals = Vec::new();
        let mut named = HashMap::new();
        let mut duplicates = HashMap::new();

        for token in tokenize_with_elements(input.args, input.text_elements)? {
            if let Some((key, value)) = split_named_arg(&token) {
                if named.insert(key.clone(), value).is_some() {
                    *duplicates.entry(key).or_default() += 1;
                }
            } else if token.text.starts_with("--") {
                return Err(SlashCommandUsageErrorKind::InvalidInlineArgs);
            } else {
                positionals.push(token);
            }
        }

        Ok(Self {
            input,
            positionals,
            next_positional: 0,
            named,
            duplicates,
        })
    }

    pub(crate) fn positional<T, S>(&mut self, spec: &S) -> Result<T, SlashCommandUsageErrorKind>
    where
        S: SlashTokenValueSpec<T>,
    {
        let Some(token) = self.positionals.get(self.next_positional).cloned() else {
            return Err(SlashCommandUsageErrorKind::InvalidInlineArgs);
        };
        self.next_positional += 1;
        spec.parse_token(token)
    }

    #[allow(dead_code)]
    pub(crate) fn optional_positional<T, S>(
        &mut self,
        spec: &S,
    ) -> Result<Option<T>, SlashCommandUsageErrorKind>
    where
        S: SlashTokenValueSpec<T>,
    {
        if self.next_positional >= self.positionals.len() {
            Ok(None)
        } else {
            self.positional(spec).map(Some)
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn positional_list<T, S>(
        &mut self,
        spec: &S,
    ) -> Result<Vec<T>, SlashCommandUsageErrorKind>
    where
        S: SlashTokenValueSpec<T>,
    {
        let mut values = Vec::new();
        while self.next_positional < self.positionals.len() {
            values.push(self.positional(spec)?);
        }
        Ok(values)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn named<T, S>(
        &mut self,
        key: &'static str,
        spec: &S,
    ) -> Result<Option<T>, SlashCommandUsageErrorKind>
    where
        S: SlashTokenValueSpec<T>,
    {
        if self.duplicates.contains_key(key) {
            return Err(SlashCommandUsageErrorKind::InvalidInlineArgs);
        }
        let Some(value) = self.named.remove(key) else {
            return Ok(None);
        };
        spec.parse_token(value).map(Some)
    }

    pub(crate) fn remainder<T, S>(&self, spec: &S) -> Result<Option<T>, SlashCommandUsageErrorKind>
    where
        S: SlashTextValueSpec<T>,
    {
        parse_remainder_text_arg(self.input.args, self.input.text_elements)
            .map(|value| spec.parse_text(value))
            .transpose()
    }

    pub(crate) fn required_remainder<T, S>(&self, spec: &S) -> Result<T, SlashCommandUsageErrorKind>
    where
        S: SlashTextValueSpec<T>,
    {
        self.remainder(spec)?
            .ok_or(SlashCommandUsageErrorKind::InvalidInlineArgs)
    }

    pub(crate) fn finish(self) -> Result<(), SlashCommandUsageErrorKind> {
        if self.next_positional != self.positionals.len() {
            return Err(SlashCommandUsageErrorKind::InvalidInlineArgs);
        }
        if !self.named.is_empty() || !self.duplicates.is_empty() {
            return Err(SlashCommandUsageErrorKind::InvalidInlineArgs);
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub(crate) struct SlashArgsSerializer {
    fragments: Vec<SlashSerializedText>,
}

impl SlashArgsSerializer {
    pub(crate) fn positional<T, S>(&mut self, value: &T, spec: &S)
    where
        S: SlashTokenValueSpec<T>,
    {
        self.fragments
            .push(serialize_token(&spec.serialize_token(value)));
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn list<T, I, S>(&mut self, values: I, spec: &S)
    where
        I: IntoIterator<Item = T>,
        S: SlashTokenValueSpec<T>,
    {
        for value in values {
            self.positional(&value, spec);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn named<T, S>(&mut self, key: &'static str, value: &T, spec: &S)
    where
        S: SlashTokenValueSpec<T>,
    {
        let serialized_value = serialize_token(&spec.serialize_token(value));
        self.fragments
            .push(serialized_value.prepend_inline(&format!("--{key}=")));
    }

    pub(crate) fn remainder<T, S>(&mut self, value: &T, spec: &S)
    where
        S: SlashTextValueSpec<T>,
    {
        let serialized = spec.serialize_text(value);
        if remainder_can_roundtrip_raw(&serialized) {
            self.fragments.push(SlashSerializedText {
                text: serialized.text.clone(),
                text_elements: serialized.text_elements,
            });
        } else {
            self.fragments.push(serialize_token(&SlashTokenArg::new(
                serialized.text.clone(),
                serialized.text_elements,
            )));
        }
    }

    pub(crate) fn finish(self) -> SlashSerializedText {
        join_serialized_fragments(self.fragments)
    }
}

pub(crate) trait SlashArgsSchema<T> {
    fn parse<'a>(&self, parser: &mut SlashArgsParser<'a>) -> Result<T, SlashCommandUsageErrorKind>;

    fn serialize(&self, value: &T, serializer: &mut SlashArgsSerializer);

    fn finish<'a>(&self, parser: SlashArgsParser<'a>) -> Result<(), SlashCommandUsageErrorKind> {
        parser.finish()
    }

    fn map_result<U, P, S>(
        self,
        parse_map: P,
        serialize_map: S,
    ) -> SlashMapResultSchema<Self, P, S, T, U>
    where
        Self: Sized,
        P: Fn(T) -> Result<U, SlashCommandUsageErrorKind>,
        S: Fn(&U) -> T,
    {
        SlashMapResultSchema {
            inner: self,
            parse_map,
            serialize_map,
            _phantom: PhantomData,
        }
    }
}

pub(crate) struct SlashMapResultSchema<C, P, S, T, U> {
    inner: C,
    parse_map: P,
    serialize_map: S,
    _phantom: PhantomData<fn(T) -> U>,
}

impl<C, P, S, T, U> SlashArgsSchema<U> for SlashMapResultSchema<C, P, S, T, U>
where
    C: SlashArgsSchema<T>,
    P: Fn(T) -> Result<U, SlashCommandUsageErrorKind>,
    S: Fn(&U) -> T,
{
    fn parse<'a>(&self, parser: &mut SlashArgsParser<'a>) -> Result<U, SlashCommandUsageErrorKind> {
        let parsed = self.inner.parse(parser)?;
        (self.parse_map)(parsed)
    }

    fn serialize(&self, value: &U, serializer: &mut SlashArgsSerializer) {
        let mapped = (self.serialize_map)(value);
        self.inner.serialize(&mapped, serializer);
    }

    fn finish<'a>(&self, parser: SlashArgsParser<'a>) -> Result<(), SlashCommandUsageErrorKind> {
        self.inner.finish(parser)
    }
}

pub(crate) struct SlashPositionalSchema<S> {
    spec: S,
}

pub(crate) fn positional<S>(spec: S) -> SlashPositionalSchema<S> {
    SlashPositionalSchema { spec }
}

impl<T, S> SlashArgsSchema<T> for SlashPositionalSchema<S>
where
    S: SlashTokenValueSpec<T>,
{
    fn parse<'a>(&self, parser: &mut SlashArgsParser<'a>) -> Result<T, SlashCommandUsageErrorKind> {
        parser.positional(&self.spec)
    }

    fn serialize(&self, value: &T, serializer: &mut SlashArgsSerializer) {
        serializer.positional(value, &self.spec);
    }
}

pub(crate) struct SlashListSchema<S> {
    spec: S,
}

pub(crate) fn list<S>(spec: S) -> SlashListSchema<S> {
    SlashListSchema { spec }
}

impl<T, S> SlashArgsSchema<Vec<T>> for SlashListSchema<S>
where
    T: Clone,
    S: SlashTokenValueSpec<T>,
{
    fn parse<'a>(
        &self,
        parser: &mut SlashArgsParser<'a>,
    ) -> Result<Vec<T>, SlashCommandUsageErrorKind> {
        parser.positional_list(&self.spec)
    }

    fn serialize(&self, value: &Vec<T>, serializer: &mut SlashArgsSerializer) {
        serializer.list(value.iter().cloned(), &self.spec);
    }
}

#[allow(dead_code)]
pub(crate) struct SlashNamedSchema<S> {
    key: &'static str,
    spec: S,
}

#[allow(dead_code)]
pub(crate) fn named<S>(key: &'static str, spec: S) -> SlashNamedSchema<S> {
    SlashNamedSchema { key, spec }
}

impl<T, S> SlashArgsSchema<Option<T>> for SlashNamedSchema<S>
where
    S: SlashTokenValueSpec<T>,
{
    fn parse<'a>(
        &self,
        parser: &mut SlashArgsParser<'a>,
    ) -> Result<Option<T>, SlashCommandUsageErrorKind> {
        parser.named(self.key, &self.spec)
    }

    fn serialize(&self, value: &Option<T>, serializer: &mut SlashArgsSerializer) {
        if let Some(value) = value {
            serializer.named(self.key, value, &self.spec);
        }
    }
}

pub(crate) struct SlashNamedOrPositionalSchema<S> {
    key: &'static str,
    spec: S,
}

pub(crate) fn named_or_positional<S>(
    key: &'static str,
    spec: S,
) -> SlashNamedOrPositionalSchema<S> {
    SlashNamedOrPositionalSchema { key, spec }
}

impl<T, S> SlashArgsSchema<T> for SlashNamedOrPositionalSchema<S>
where
    S: SlashTokenValueSpec<T>,
{
    fn parse<'a>(&self, parser: &mut SlashArgsParser<'a>) -> Result<T, SlashCommandUsageErrorKind> {
        match parser.named(self.key, &self.spec)? {
            Some(value) => Ok(value),
            None => parser.positional(&self.spec),
        }
    }

    fn serialize(&self, value: &T, serializer: &mut SlashArgsSerializer) {
        serializer.positional(value, &self.spec);
    }
}

pub(crate) struct SlashRemainderSchema<S> {
    spec: S,
}

pub(crate) fn remainder<S>(spec: S) -> SlashRemainderSchema<S> {
    SlashRemainderSchema { spec }
}

impl<T, S> SlashArgsSchema<T> for SlashRemainderSchema<S>
where
    S: SlashTextValueSpec<T>,
{
    fn parse<'a>(&self, parser: &mut SlashArgsParser<'a>) -> Result<T, SlashCommandUsageErrorKind> {
        parser.required_remainder(&self.spec)
    }

    fn serialize(&self, value: &T, serializer: &mut SlashArgsSerializer) {
        serializer.remainder(value, &self.spec);
    }

    fn finish<'a>(&self, _parser: SlashArgsParser<'a>) -> Result<(), SlashCommandUsageErrorKind> {
        Ok(())
    }
}

fn trim_text_arg(text: &str, text_elements: &[TextElement]) -> Option<SlashTextArg> {
    let trimmed_start = text.len() - text.trim_start().len();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let trimmed_end = trimmed_start + trimmed.len();
    let mut elements = Vec::new();
    for element in text_elements {
        let start = element.byte_range.start.max(trimmed_start);
        let end = element.byte_range.end.min(trimmed_end);
        if start < end {
            elements.push(element.map_range(|_| ByteRange {
                start: start - trimmed_start,
                end: end - trimmed_start,
            }));
        }
    }

    Some(SlashTextArg::new(trimmed.to_string(), elements))
}

fn parse_remainder_text_arg(text: &str, text_elements: &[TextElement]) -> Option<SlashTextArg> {
    let trimmed = trim_text_arg(text, text_elements)?;
    match tokenize_with_elements(&trimmed.text, &trimmed.text_elements) {
        Ok(tokens) => match tokens.as_slice() {
            [token] => Some(SlashTextArg::new(
                token.text.clone(),
                token.text_elements.clone(),
            )),
            _ => Some(trimmed),
        },
        _ => Some(trimmed),
    }
}

fn remainder_can_roundtrip_raw(value: &SlashTextArg) -> bool {
    match tokenize_with_elements(&value.text, &value.text_elements) {
        Ok(tokens) if tokens.len() == 1 => {
            tokens[0] == SlashTokenArg::new(value.text.clone(), value.text_elements.clone())
        }
        Ok(_) => true,
        Err(_) => false,
    }
}

fn split_named_arg(token: &SlashTokenArg) -> Option<(String, SlashTokenArg)> {
    let rest = token.text.strip_prefix("--")?;
    let (key, value) = rest.split_once('=')?;
    if key.is_empty() {
        return None;
    }
    let value_offset = 2 + key.len() + 1;
    let value_elements = token
        .text_elements
        .iter()
        .filter_map(|element| shift_text_element_left(element, value_offset))
        .collect();
    Some((
        key.to_string(),
        SlashTokenArg::new(value.to_string(), value_elements),
    ))
}

fn tokenize_with_elements(
    text: &str,
    text_elements: &[TextElement],
) -> Result<Vec<SlashTokenArg>, SlashCommandUsageErrorKind> {
    let mut elements = text_elements.to_vec();
    elements.sort_by_key(|element| element.byte_range.start);
    let (text_for_shlex, replacements) = replace_text_elements_with_sentinels(text, &elements);
    let mut lexer = Shlex::new(&text_for_shlex);
    let tokens: Vec<String> = lexer.by_ref().collect();
    if lexer.had_error {
        return Err(SlashCommandUsageErrorKind::InvalidInlineArgs);
    }
    Ok(tokens
        .into_iter()
        .map(|token| {
            let restored = restore_sentinels_in_fragment(token, &replacements);
            SlashTokenArg::new(restored.text, restored.text_elements)
        })
        .collect())
}

fn serialize_token(token: &SlashTokenArg) -> SlashSerializedText {
    if token.text.is_empty() {
        return SlashSerializedText::empty();
    }

    let (token_for_shlex, replacements) =
        replace_text_elements_with_sentinels(&token.text, &token.text_elements);
    let quoted = try_join([token_for_shlex.as_str()])
        .unwrap_or_else(|_| shell_quote_token(&token_for_shlex));
    restore_sentinels_in_fragment(quoted, &replacements)
}

fn shell_quote_token(token: &str) -> String {
    if token.is_empty() {
        return "''".to_string();
    }

    let mut quoted = String::from("'");
    for ch in token.chars() {
        if ch == '\'' {
            quoted.push_str("'\"'\"'");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

fn join_serialized_fragments(fragments: Vec<SlashSerializedText>) -> SlashSerializedText {
    let mut text = String::new();
    let mut text_elements = Vec::new();

    for fragment in fragments
        .into_iter()
        .filter(|fragment| !fragment.text.is_empty())
    {
        let offset = if text.is_empty() { 0 } else { 1 };
        if offset == 1 {
            text.push(' ');
        }
        let fragment_offset = text.len();
        text.push_str(&fragment.text);
        text_elements.extend(shift_text_elements_right(
            &fragment.text_elements,
            fragment_offset,
        ));
    }

    SlashSerializedText {
        text,
        text_elements,
    }
}

fn shift_text_element_left(element: &TextElement, offset: usize) -> Option<TextElement> {
    if element.byte_range.end <= offset {
        return None;
    }
    let start = element.byte_range.start.saturating_sub(offset);
    let end = element.byte_range.end.saturating_sub(offset);
    (start < end).then(|| element.map_range(|_| ByteRange { start, end }))
}

fn shift_text_elements_right(elements: &[TextElement], offset: usize) -> Vec<TextElement> {
    elements
        .iter()
        .map(|element| {
            element.map_range(|byte_range| ByteRange {
                start: byte_range.start + offset,
                end: byte_range.end + offset,
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
struct ElementReplacement {
    sentinel: String,
    text: String,
    placeholder: Option<String>,
}

fn replace_text_elements_with_sentinels(
    text: &str,
    text_elements: &[TextElement],
) -> (String, Vec<ElementReplacement>) {
    let mut out = String::with_capacity(text.len());
    let mut replacements = Vec::new();
    let mut cursor = 0;
    let text_len = text.len();

    for (idx, element) in text_elements.iter().enumerate() {
        let start = element.byte_range.start.clamp(cursor, text_len);
        let end = element.byte_range.end.clamp(start, text_len);
        out.push_str(&text[cursor..start]);
        let mut sentinel = format!("__CODEX_ELEM_{idx}__");
        while text.contains(&sentinel) {
            sentinel.push('_');
        }
        out.push_str(&sentinel);
        let replacement_text = text
            .get(start..end)
            .or_else(|| element.placeholder(text))
            .unwrap_or_default()
            .to_string();
        replacements.push(ElementReplacement {
            sentinel,
            text: replacement_text,
            placeholder: element.placeholder(text).map(str::to_string),
        });
        cursor = end;
    }

    out.push_str(&text[cursor..]);
    (out, replacements)
}

fn restore_sentinels_in_fragment(
    fragment: String,
    replacements: &[ElementReplacement],
) -> SlashSerializedText {
    if replacements.is_empty() {
        return SlashSerializedText {
            text: fragment,
            text_elements: Vec::new(),
        };
    }

    let mut out = String::with_capacity(fragment.len());
    let mut out_elements = Vec::new();
    let mut cursor = 0;

    while cursor < fragment.len() {
        let Some((offset, replacement)) = next_replacement(&fragment, cursor, replacements) else {
            out.push_str(&fragment[cursor..]);
            break;
        };
        let start_in_fragment = cursor + offset;
        out.push_str(&fragment[cursor..start_in_fragment]);
        let start = out.len();
        out.push_str(&replacement.text);
        let end = out.len();
        if start < end {
            out_elements.push(TextElement::new(
                ByteRange { start, end },
                replacement.placeholder.clone(),
            ));
        }
        cursor = start_in_fragment + replacement.sentinel.len();
    }

    SlashSerializedText {
        text: out,
        text_elements: out_elements,
    }
}

fn next_replacement<'a>(
    text: &str,
    cursor: usize,
    replacements: &'a [ElementReplacement],
) -> Option<(usize, &'a ElementReplacement)> {
    replacements
        .iter()
        .filter_map(|replacement| {
            text[cursor..]
                .find(&replacement.sentinel)
                .map(|offset| (offset, replacement))
        })
        .min_by_key(|(offset, _)| *offset)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Switch {
        On,
        Off,
    }

    const SWITCH_CHOICES: &[(&str, Switch)] = &[("on", Switch::On), ("off", Switch::Off)];

    #[test]
    fn parser_supports_positional_list_and_named_args() {
        let mut parser = SlashArgsParser::new(SlashCommandParseInput {
            args: "on first second --path=\"some dir\"",
            text_elements: &[],
        })
        .unwrap();

        assert_eq!(
            parser.positional(&enum_choice(SWITCH_CHOICES)),
            Ok(Switch::On)
        );
        assert_eq!(
            parser.positional_list(&string()),
            Ok(vec!["first".to_string(), "second".to_string()])
        );
        assert_eq!(
            parser.named("path", &string()),
            Ok(Some("some dir".to_string()))
        );
        assert_eq!(parser.finish(), Ok(()));
    }

    #[test]
    fn parser_supports_optional_positional_args() {
        let mut parser = SlashArgsParser::new(SlashCommandParseInput {
            args: "on",
            text_elements: &[],
        })
        .unwrap();

        assert_eq!(
            parser.positional(&enum_choice(SWITCH_CHOICES)),
            Ok(Switch::On)
        );
        assert_eq!(parser.optional_positional(&string()), Ok(None));
        assert_eq!(parser.finish(), Ok(()));
    }

    #[test]
    fn serializer_stably_formats_named_args_after_positionals() {
        let mut serializer = SlashArgsSerializer::default();
        serializer.positional(&Switch::On, &enum_choice(SWITCH_CHOICES));
        serializer.list(["first".to_string(), "second".to_string()], &string());
        serializer.named("path", &"some dir".to_string(), &string());

        assert_eq!(
            serializer.finish(),
            SlashSerializedText {
                text: "on first second --path='some dir'".to_string(),
                text_elements: Vec::new(),
            }
        );
    }

    #[test]
    fn remainder_preserves_placeholder_ranges() {
        let placeholder = "[Image #1]".to_string();
        let prompt = SlashTextArg::new(
            format!("review {placeholder}"),
            vec![TextElement::new((7..18).into(), Some(placeholder.clone()))],
        );
        let mut serializer = SlashArgsSerializer::default();
        serializer.remainder(&prompt, &text());

        assert_eq!(
            serializer.finish(),
            SlashSerializedText {
                text: format!("review {placeholder}"),
                text_elements: vec![TextElement::new((7..18).into(), Some(placeholder))],
            }
        );
    }

    #[test]
    fn remainder_quotes_shell_sensitive_text_when_needed() {
        let prompt = SlashTextArg::new("a\"\" a\"".to_string(), Vec::new());
        let mut serializer = SlashArgsSerializer::default();
        serializer.remainder(&prompt, &text());

        assert_eq!(
            serializer.finish(),
            SlashSerializedText {
                text: "'a\"\" a\"'".to_string(),
                text_elements: Vec::new(),
            }
        );
        assert_eq!(
            SlashArgsParser::new(SlashCommandParseInput {
                args: "'a\"\" a\"'",
                text_elements: &[],
            })
            .unwrap()
            .required_remainder(&text()),
            Ok(prompt)
        );
    }
}
