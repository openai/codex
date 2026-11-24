//! Compare validated JSON without rounding numbers or dropping repeated keys.
//! Index container boundaries once so visiting a field never reparses its descendants.

use std::borrow::Cow;
use std::collections::HashMap;

use serde::Deserialize;
use serde_json::value::RawValue;

const MAX_DEPTH: usize = 64;

/// Ignore formatting and object key order. Keep the sign of zero and decline
/// comparisons that require rounding or ambiguous matches of repeated keys.
pub(super) fn equivalent(left: &RawValue, right: &RawValue) -> bool {
    let (left, right) = (left.get(), right.get());
    if left == right {
        return true;
    }
    let left = Json {
        text: left,
        spans: &index(left),
    };
    let right = Json {
        text: right,
        spans: &index(right),
    };
    left.compare(right, MAX_DEPTH).is_some()
}

/// The text and index lengths of a subtree, including its own entry.
struct Span {
    bytes: usize,
    entries: usize,
}

/// Serde has checked the grammar. Skip escaped quotes and punctuation inside strings.
fn brackets(text: &str) -> impl Iterator<Item = (usize, u8)> {
    let mut bytes = text.bytes().enumerate();
    std::iter::from_fn(move || {
        while let Some((offset, byte)) = bytes.next() {
            match byte {
                b'[' | b'{' | b']' | b'}' => return Some((offset, byte)),
                b'"' => {
                    while let Some((_, byte)) = bytes.next() {
                        match byte {
                            b'"' => break,
                            b'\\' => {
                                bytes.next();
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        None
    })
}

/// Only containers with nested containers need an index entry. The first
/// bracket inside a flat container already gives us its end.
fn index(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut open = Vec::new();
    let mut brackets = brackets(text).peekable();
    let mut depth = 0;
    while let Some((offset, bracket)) = brackets.next() {
        if matches!(bracket, b'{' | b'[') {
            if matches!(brackets.peek(), Some((_, b'}' | b']'))) {
                brackets.next();
                continue;
            }
            // At MAX_DEPTH we still need the outer span for an exact match.
            if depth <= MAX_DEPTH {
                open.push((offset, spans.len()));
                spans.push(Span {
                    bytes: 0,
                    entries: 0,
                });
            }
            depth += 1;
        } else {
            depth -= 1;
            if depth <= MAX_DEPTH
                && let Some((start, slot)) = open.pop()
            {
                spans[slot] = Span {
                    bytes: offset + 1 - start,
                    entries: spans.len() - slot,
                };
            }
        }
    }
    spans
}

/// A value, or the remaining members of a container, borrowed from the same two slices.
struct Json<'a> {
    text: &'a str,
    spans: &'a [Span],
}

#[derive(Deserialize)]
struct Key<'a>(#[serde(borrow)] Cow<'a, str>);

impl<'a> Json<'a> {
    fn members(self) -> Self {
        Self {
            text: self.text[1..self.text.len() - 1].trim_ascii_start(),
            spans: self.spans.get(1..).unwrap_or_default(),
        }
    }

    fn take_key(&mut self) -> Option<Cow<'a, str>> {
        let mut keys = serde_json::Deserializer::from_str(self.text).into_iter::<Key>();
        let Key(key) = keys.next()?.ok()?;
        self.text = self.text[keys.byte_offset()..]
            .trim_ascii_start()
            .strip_prefix(':')?
            .trim_ascii_start();
        Some(key)
    }

    /// Advance past one member, jumping directly over nested containers.
    fn take_value(&mut self) -> Option<Self> {
        let (len, count) = match self.text.as_bytes().first()? {
            b'{' | b'[' => {
                let (offset, bracket) = brackets(&self.text[1..]).next()?;
                if matches!(bracket, b'}' | b']') {
                    (offset + 2, 0)
                } else {
                    let span = self.spans.first()?;
                    (span.bytes, span.entries)
                }
            }
            _ => {
                let mut values =
                    serde_json::Deserializer::from_str(self.text).into_iter::<&RawValue>();
                (values.next()?.ok()?.get().len(), 0)
            }
        };
        let (text, rest) = self.text.split_at(len);
        let (spans, tail) = self.spans.split_at(count);
        let rest = rest.trim_ascii_start();
        self.text = rest.strip_prefix(',').unwrap_or(rest).trim_ascii_start();
        self.spans = tail;
        Some(Self { text, spans })
    }

    /// A mismatch or a value we can't compare losslessly ends the comparison.
    fn compare(self, other: Self, remaining_depth: usize) -> Option<()> {
        // Byte-compare leaves and their immediate parents. Comparing at every parent could rescan
        // a large common prefix at each level when a difference is near the end.
        if (self.spans.len() <= 1 || remaining_depth == 0) && self.text == other.text {
            return Some(());
        }
        let depth = remaining_depth.checked_sub(1)?;
        match (
            self.text.as_bytes().first()?,
            other.text.as_bytes().first()?,
        ) {
            (b'{', b'{') => self.members().compare_fields(other.members(), depth),
            (b'[', b'[') => {
                let (mut left, mut right) = (self.members(), other.members());
                while !left.text.is_empty() {
                    left.take_value()?.compare(right.take_value()?, depth)?;
                }
                right.text.is_empty().then_some(())
            }
            (b'"', b'"') => {
                let a: String = serde_json::from_str(self.text).ok()?;
                let b: String = serde_json::from_str(other.text).ok()?;
                (a == b).then_some(())
            }
            (b'-' | b'0'..=b'9', b'-' | b'0'..=b'9') => {
                (decimal_parts(self.text)? == decimal_parts(other.text)?).then_some(())
            }
            _ => None,
        }
    }

    /// Walk the common prefix in order. If the keys diverge, look up the
    /// remaining fields by name and consume each once. Refuse ambiguous repeats.
    fn compare_fields(mut self, mut other: Self, depth: usize) -> Option<()> {
        while !self.text.is_empty() {
            let key = self.take_key()?;
            let name = other.take_key()?;
            if key == name {
                self.take_value()?.compare(other.take_value()?, depth)?;
                continue;
            }

            let mut fields = HashMap::from([(name, other.take_value()?)]);
            while !other.text.is_empty() {
                if fields
                    .insert(other.take_key()?, other.take_value()?)
                    .is_some()
                {
                    return None;
                }
            }
            self.take_value()?.compare(fields.remove(&key)?, depth)?;
            while !self.text.is_empty() {
                let key = self.take_key()?;
                self.take_value()?.compare(fields.remove(&key)?, depth)?;
            }
            return fields.is_empty().then_some(());
        }
        other.text.is_empty().then_some(())
    }
}

/// Express a valid JSON number as a sign, significant digits and a power of ten.
/// For example, 0.0120 and 12e-3 both have digits "12" and exponent -3.
/// Return None if the exponent is too large to compare exactly.
fn decimal_parts(number: &str) -> Option<(bool, Cow<'_, str>, i64)> {
    let negative = number.starts_with('-');
    let unsigned = number.trim_start_matches('-');
    let (mantissa, exponent) = unsigned.split_once(['e', 'E']).unwrap_or((unsigned, "0"));
    let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let fraction = fraction.trim_end_matches('0');
    if integer == "0" && fraction.is_empty() {
        // Do not discard the sign even if the significand is zero.
        return Some((negative, Cow::Borrowed("0"), 0));
    }

    let exponent = exponent.parse::<i64>().ok()?;
    if fraction.is_empty() {
        let digits = integer.trim_end_matches('0');
        let trailing_zeros = i64::try_from(integer.len() - digits.len()).ok()?;
        return Some((
            negative,
            Cow::Borrowed(digits),
            exponent.checked_add(trailing_zeros)?,
        ));
    }
    let exponent = exponent.checked_sub(i64::try_from(fraction.len()).ok()?)?;
    let digits = if integer == "0" {
        Cow::Borrowed(fraction.trim_start_matches('0'))
    } else {
        Cow::Owned(format!("{integer}{fraction}"))
    };
    Some((negative, digits, exponent))
}

#[cfg(test)]
#[path = "json_equality_tests.rs"]
mod tests;
