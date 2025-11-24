/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::str::FromStr;

use starlark_syntax::dot_format_parser::FormatConv;
use starlark_syntax::dot_format_parser::FormatParser;
use starlark_syntax::dot_format_parser::FormatToken;

use crate::collections::string_pool::StringPool;
use crate::values::dict::Dict;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::Value;
use crate::values::ValueError;
use crate::values::ValueLike;

/// Try parse `"aaa{}bbb"` and return `("aaa", "bbb")`.
pub(crate) fn parse_format_one(s: &str) -> Option<(String, String)> {
    let mut parser = FormatParser::new(s);
    let mut before = String::with_capacity(s.len());
    loop {
        match parser.next().ok()?? {
            FormatToken::Text(text) => before.push_str(text),
            FormatToken::Escape(e) => before.push_str(e.as_str()),
            FormatToken::Capture {
                capture: "",
                conv: FormatConv::Str,
                pos: _,
            } => break,
            FormatToken::Capture { .. } => return None,
        }
    }

    let mut after = String::with_capacity(s.len() - before.len());
    loop {
        match parser.next().ok()? {
            Some(FormatToken::Text(text)) => after.push_str(text),
            Some(FormatToken::Escape(e)) => after.push_str(e.as_str()),
            Some(FormatToken::Capture { .. }) => return None,
            None => break,
        }
    }

    Some((before, after))
}

/// Evaluate `"<before>{}<after>".format(arg)`.
pub(crate) fn format_one<'v>(
    before: &str,
    arg: Value<'v>,
    after: &str,
    heap: &'v Heap,
) -> StringValue<'v> {
    match StringValue::new(arg) {
        Some(arg) => heap.alloc_str_concat3(before, &arg, after),
        None => {
            let mut result = String::with_capacity(before.len() + after.len() + 10);
            result.push_str(before);
            arg.collect_repr(&mut result);
            result.push_str(after);
            heap.alloc_str(&result)
        }
    }
}

/// The format string can either have explicit indices,
/// or grab things sequentially, but not both.
/// FormatArgs knows which we are doing and keeps them in mind.
struct FormatArgs<'v, T: Iterator<Item = Value<'v>>> {
    // Initially we have the iterator set and the args empty.
    // If we ever ask by index, we decant the iterator into args.
    iterator: T,
    args: Vec<Value<'v>>,
    by_index: bool,
    by_order: bool,
}

impl<'v, T: Iterator<Item = Value<'v>>> FormatArgs<'v, T> {
    fn new(iterator: T) -> Self {
        Self {
            iterator,
            args: Vec::new(),
            by_index: false,
            by_order: false,
        }
    }

    fn next_ordered(&mut self) -> anyhow::Result<Value<'v>> {
        if self.by_index {
            Err(anyhow::anyhow!(
                "Cannot mix manual field specification and automatic field numbering in format string",
            ))
        } else {
            self.by_order = true;
            match self.iterator.next() {
                None => Err(anyhow::anyhow!("Not enough parameters in format string")),
                Some(x) => Ok(x),
            }
        }
    }

    fn by_index(&mut self, index: usize) -> anyhow::Result<Value<'v>> {
        if self.by_order {
            Err(anyhow::anyhow!(
                "Cannot mix manual field specification and automatic field numbering in format string",
            ))
        } else {
            if !self.by_index {
                self.args.extend(&mut self.iterator);
                self.by_index = true;
            }
            match self.args.get(index) {
                None => Err(ValueError::IndexOutOfBound(index as i32).into()),
                Some(v) => Ok(*v),
            }
        }
    }
}

pub(crate) fn format<'v>(
    this: &str,
    args: impl Iterator<Item = Value<'v>>,
    kwargs: Dict<'v>,
    string_pool: &mut StringPool,
    heap: &'v Heap,
) -> anyhow::Result<StringValue<'v>> {
    let mut parser = FormatParser::new(this);
    let mut result = string_pool.alloc();
    let mut args = FormatArgs::new(args);
    while let Some(token) = parser.next()? {
        match token {
            FormatToken::Text(text) => result.push_str(text),
            FormatToken::Escape(e) => result.push_str(e.as_str()),
            FormatToken::Capture {
                capture,
                conv,
                pos: _,
            } => format_capture(capture, conv, &mut args, &kwargs, &mut result)?,
        }
    }
    let r = heap.alloc_str(&result);
    string_pool.release(result);
    Ok(r)
}

fn format_capture<'v, T: Iterator<Item = Value<'v>>>(
    field: &str,
    conv: FormatConv,
    args: &mut FormatArgs<'v, T>,
    kwargs: &Dict,
    result: &mut String,
) -> anyhow::Result<()> {
    let conv_s = |x: Value, result: &mut String| x.collect_str(result);
    let conv_r = |x: Value, result: &mut String| x.collect_repr(result);
    let conv: &dyn Fn(Value, &mut String) = match conv {
        FormatConv::Str => &conv_s,
        FormatConv::Repr => &conv_r,
    };
    if field.is_empty() {
        conv(args.next_ordered()?, result);
        Ok(())
    } else if field.bytes().all(|c| c.is_ascii_digit()) {
        let i = usize::from_str(field).map_err(|e| {
            anyhow::anyhow!("Error parsing `{field}` as a format string index: {e}")
        })?;
        conv(args.by_index(i)?, result);
        Ok(())
    } else {
        if let Some(x) = field.bytes().find(|c| match c {
            b'.' | b',' | b'[' | b']' => true,
            _ => false,
        }) {
            return Err(anyhow::anyhow!(
                "Invalid character '{}' inside replacement field",
                char::from(x)
            ));
        }
        match kwargs.get_str(field) {
            None => Err(ValueError::KeyNotFound(field.to_owned()).into()),
            Some(v) => {
                conv(v, result);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use starlark_map::small_map::SmallMap;
    use starlark_syntax::dot_format_parser::FormatConv;

    use crate::assert;
    use crate::coerce::coerce;
    use crate::values::dict::Dict;
    use crate::values::string::dot_format::format_capture;
    use crate::values::string::dot_format::parse_format_one;
    use crate::values::string::dot_format::FormatArgs;
    use crate::values::Heap;
    use crate::values::Value;

    fn format_capture_for_test<'v, T: Iterator<Item = Value<'v>>>(
        capture: &str,
        conv: FormatConv,
        args: &mut FormatArgs<'v, T>,
        kwargs: &Dict,
    ) -> anyhow::Result<String> {
        let mut result = String::new();
        format_capture(capture, conv, args, kwargs, &mut result)?;
        Ok(result)
    }

    #[test]
    fn test_format_capture() {
        let heap = Heap::new();
        let original_args = [heap.alloc("1"), heap.alloc("2"), heap.alloc("3")];
        let mut args = FormatArgs::new(original_args.iter().copied());
        let mut kwargs = SmallMap::new();

        kwargs.insert_hashed(heap.alloc_str("a").get_hashed(), heap.alloc("x"));
        kwargs.insert_hashed(heap.alloc_str("b").get_hashed(), heap.alloc("y"));
        kwargs.insert_hashed(heap.alloc_str("c").get_hashed(), heap.alloc("z"));
        let kwargs = Dict::new(coerce(kwargs));
        assert_eq!(
            format_capture_for_test("", FormatConv::Str, &mut args, &kwargs).unwrap(),
            "1"
        );
        assert_eq!(
            format_capture_for_test("", FormatConv::Str, &mut args, &kwargs).unwrap(),
            "2"
        );
        assert_eq!(
            format_capture_for_test("", FormatConv::Repr, &mut args, &kwargs).unwrap(),
            "\"3\""
        );
        assert_eq!(
            format_capture_for_test("a", FormatConv::Repr, &mut args, &kwargs).unwrap(),
            "\"x\""
        );
        assert_eq!(
            format_capture_for_test("a", FormatConv::Str, &mut args, &kwargs).unwrap(),
            "x"
        );
        assert!(format_capture_for_test("1", FormatConv::Str, &mut args, &kwargs).is_err());
        let mut args = FormatArgs::new(original_args.iter().copied());
        assert_eq!(
            format_capture_for_test("1", FormatConv::Str, &mut args, &kwargs).unwrap(),
            "2"
        );
        assert!(format_capture_for_test("", FormatConv::Str, &mut args, &kwargs).is_err());
    }

    #[test]
    fn test_format() {
        assert::eq("'a{x}b{y}c{}'.format(1, x=2, y=3)", "'a2b3c1'");
        assert::eq("'a{x}b{{y}}c{}'.format(1, x=2)", "'a2b{y}c1'");
    }

    #[test]
    fn test_parse_format_one() {
        assert_eq!(
            Some(("abc".to_owned(), "def".to_owned())),
            parse_format_one("abc{}def")
        );
        assert_eq!(
            Some(("abc".to_owned(), "def".to_owned())),
            parse_format_one("abc{!s}def")
        );
        assert_eq!(None, parse_format_one("abc{!r}def"));
        assert_eq!(
            Some(("a{b".to_owned(), "c}d{".to_owned())),
            parse_format_one("a{{b{}c}}d{{")
        );
        assert_eq!(None, parse_format_one("a{"));
        assert_eq!(None, parse_format_one("a{}{}"));
        assert_eq!(None, parse_format_one("{x}"));
    }
}
