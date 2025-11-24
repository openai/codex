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

use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::eval::Evaluator;
use crate::values::string::StarlarkStr;
use crate::values::StringValue;
use crate::values::Value;
use crate::values::ValueLike;

#[starlark_module]
pub(crate) fn register_str(globals: &mut GlobalsBuilder) {
    /// [chr](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#bool
    /// ): returns a string encoding a codepoint.
    ///
    /// `chr(i)` returns a string that encodes the single Unicode code
    /// point whose value is specified by the integer `i`. `chr` fails
    /// unless `0 ≤ i ≤ 0x10FFFF`.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// chr(65) == 'A'
    /// chr(1049) == 'Й'
    /// chr(0x1F63F) == '😿'
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn chr(#[starlark(require = pos)] i: i32) -> anyhow::Result<char> {
        let cp = u32::try_from(i)
            .map_err(|_| anyhow::anyhow!("chr() parameter value negative integer {i}"))?;
        match char::from_u32(cp) {
            Some(x) => Ok(x),
            None => Err(anyhow::anyhow!(
                "chr() parameter value is 0x{:x} which is not a valid UTF-8 codepoint",
                cp
            )),
        }
    }

    /// [ord](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#ord
    /// ): returns the codepoint of a character
    ///
    /// `ord(s)` returns the integer value of the sole Unicode code point
    /// encoded by the string `s`.
    ///
    /// If `s` does not encode exactly one Unicode code point, `ord` fails.
    /// Each invalid code within the string is treated as if it encodes the
    /// Unicode replacement character, U+FFFD.
    ///
    /// Example:
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// ord("A")                                == 65
    /// ord("Й")                                == 1049
    /// ord("😿")                               == 0x1F63F
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn ord<'v>(#[starlark(require = pos)] a: StringValue<'v>) -> anyhow::Result<i32> {
        let mut chars = a.as_str().chars();
        if let Some(c) = chars.next() {
            if chars.next().is_none() {
                return Ok(u32::from(c) as i32);
            }
        }
        Err(anyhow::anyhow!(
            "ord(): {} is not a single character string",
            a.to_value().to_repr()
        ))
    }

    /// [repr](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#repr
    /// ): formats its argument as a string.
    ///
    /// All strings in the result are double-quoted.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// repr(1)                 == '1'
    /// repr("x")               == "\"x\""
    /// repr([1, "x"])          == "[1, \"x\"]"
    /// repr("test \"'")        == "\"test \\\"'\""
    /// repr("x\"y😿 \\'")      == "\"x\\\"y\\U0001f63f \\\\'\""
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn repr<'v>(
        #[starlark(require = pos)] a: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<StringValue<'v>> {
        let mut s = eval.string_pool.alloc();
        a.collect_repr(&mut s);
        let r = eval.heap().alloc_str(&s);
        eval.string_pool.release(s);
        Ok(r)
    }

    /// [str](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#str
    /// ): formats its argument as a string.
    ///
    /// If x is a string, the result is x (without quotation).
    /// All other strings, such as elements of a list of strings, are
    /// double-quoted.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// str(1)                          == '1'
    /// str("x")                        == 'x'
    /// str([1, "x"])                   == "[1, \"x\"]"
    /// # "#);
    /// ```
    #[starlark(as_type = StarlarkStr, speculative_exec_safe)]
    fn str<'v>(
        #[starlark(require = pos)] a: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<StringValue<'v>> {
        if let Some(a) = StringValue::new(a) {
            // Special case that can avoid reallocating, but is equivalent.
            Ok(a)
        } else {
            let mut s = eval.string_pool.alloc();
            a.collect_repr(&mut s);
            let r = eval.heap().alloc_str(&s);
            eval.string_pool.release(s);
            Ok(r)
        }
    }
}
