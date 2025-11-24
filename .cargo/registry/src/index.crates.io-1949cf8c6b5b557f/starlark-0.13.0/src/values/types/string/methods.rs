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

//! Methods for the `string` type.

use std::cmp;

use starlark_derive::starlark_module;
use starlark_syntax::fast_string;
use starlark_syntax::fast_string::convert_str_indices;
use starlark_syntax::fast_string::StrIndices;

use crate as starlark;
use crate::environment::MethodsBuilder;
use crate::eval::Arguments;
use crate::eval::Evaluator;
use crate::values::list::AllocList;
use crate::values::list::UnpackList;
use crate::values::none::NoneOr;
use crate::values::string::dot_format;
use crate::values::tuple::UnpackTuple;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::string::iter::iterate_chars;
use crate::values::types::string::iter::iterate_codepoints;
use crate::values::typing::iter::StarlarkIter;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueOfUnchecked;

// This does not exists in rust, split would cut the string incorrectly and
// split_whitespace cannot take a n parameter.
fn splitn_whitespace(s: &str, maxsplit: usize) -> Vec<String> {
    let mut v = Vec::new();
    let mut cur = String::new();
    let mut split = 1;
    let mut eat_ws = true;
    for c in s.chars() {
        if split >= maxsplit && !eat_ws {
            cur.push(c)
        } else if c.is_whitespace() {
            if !cur.is_empty() {
                v.push(cur);
                cur = String::new();
                split += 1;
                eat_ws = true;
            }
        } else {
            eat_ws = false;
            cur.push(c)
        }
    }
    if !cur.is_empty() {
        v.push(cur)
    }
    v
}

fn rsplitn_whitespace(s: &str, maxsplit: usize) -> Vec<String> {
    let mut v = Vec::new();
    let mut cur = String::new();
    let mut split = 1;
    let mut eat_ws = true;
    for c in s.chars().rev() {
        if split >= maxsplit && !eat_ws {
            cur.push(c)
        } else if c.is_whitespace() {
            if !cur.is_empty() {
                v.push(cur.chars().rev().collect());
                cur = String::new();
                split += 1;
                eat_ws = true;
            }
        } else {
            eat_ws = false;
            cur.push(c)
        }
    }
    if !cur.is_empty() {
        v.push(cur.chars().rev().collect());
    }
    v.reverse();
    v
}

#[derive(StarlarkTypeRepr, UnpackValue)]
enum StringOrTuple<'v> {
    String(&'v str),
    Tuple(UnpackTuple<&'v str>),
}

#[starlark_module]
pub(crate) fn string_methods(builder: &mut MethodsBuilder) {
    /// [string.elems](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·elems
    /// ): returns an iterable of the bytes values of a string.
    ///
    /// `S.elems()` returns an iterable value containing the
    /// sequence of numeric bytes values in the string S.
    ///
    /// To materialize the entire sequence of bytes, apply `list(...)` to the
    /// result.
    ///
    /// ```
    /// # starlark::assert::is_true(r#"
    /// list("Hello, 世界".elems()) == ["H", "e", "l", "l", "o", ",", " ", "世", "界"]
    /// # "#);
    /// ```
    fn elems<'v>(
        this: StringValue<'v>,
        heap: &'v Heap,
    ) -> anyhow::Result<ValueOfUnchecked<'v, StarlarkIter<String>>> {
        Ok(iterate_chars(this, heap))
    }

    /// [string.capitalize](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string%C2%B7capitalize
    /// ): returns a copy of string S, where the first character (if any) is converted to uppercase;
    /// all other characters are converted to lowercase.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "hello, world!".capitalize() == "Hello, world!"
    /// "Hello, World!".capitalize() == "Hello, world!"
    /// "".capitalize() == ""
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn capitalize(this: &str) -> anyhow::Result<String> {
        let mut result = String::with_capacity(this.len());
        for (i, c) in this.chars().enumerate() {
            if i == 0 {
                result.extend(c.to_uppercase())
            } else {
                result.extend(c.to_lowercase())
            }
        }
        Ok(result)
    }

    /// [string.codepoints](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·codepoints
    /// ): returns an iterable of the unicode codepoint of a string.
    ///
    /// `S.codepoints()` returns an iterable value containing the
    /// sequence of integer Unicode code points encoded by the string S.
    /// Each invalid code within the string is treated as if it encodes the
    /// Unicode replacement character, U+FFFD.
    ///
    /// By returning an iterable, not a list, the cost of decoding the string
    /// is deferred until actually needed; apply `list(...)` to the result to
    /// materialize the entire sequence.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// list("Hello, 世界".codepoints()) == [72, 101, 108, 108, 111, 44, 32, 19990, 30028]
    /// # "#);
    /// ```
    fn codepoints<'v>(
        this: StringValue<'v>,
        heap: &'v Heap,
    ) -> anyhow::Result<ValueOfUnchecked<'v, StarlarkIter<String>>> {
        Ok(iterate_codepoints(this, heap))
    }

    /// [string.count](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·count
    /// ): count the number of occurrences of a string in another string.
    ///
    /// `S.count(sub[, start[, end]])` returns the number of occurrences of
    /// `sub` within the string S, or, if the optional substring indices
    /// `start` and `end` are provided, within the designated substring of S.
    /// They are interpreted according to Skylark's [indexing conventions](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#indexing).
    ///
    /// This implementation does not count occurrence of `sub` in the string `S`
    /// that overlap other occurrence of S (which can happen if some suffix of S
    /// is a prefix of S). For instance, `"abababa".count("aba")` returns 2
    /// for `[aba]a[aba]`, not counting the middle occurrence: `ab[aba]ba`
    /// (this is following Python behavior).
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "hello, world!".count("o") == 2
    /// "abababa".count("aba") == 2
    /// "hello, world!".count("o", 7, 12) == 1  # in "world"
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn count(
        this: &str,
        #[starlark(require = pos)] needle: &str,
        #[starlark(require = pos, default = NoneOr::None)] start: NoneOr<i32>,
        #[starlark(require = pos, default = NoneOr::None)] end: NoneOr<i32>,
    ) -> anyhow::Result<i32> {
        if let Some(StrIndices { haystack, .. }) =
            convert_str_indices(this, start.into_option(), end.into_option())
        {
            Ok(fast_string::count_matches(haystack, needle) as i32)
        } else {
            Ok(0)
        }
    }

    /// [string.endswith](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·endswith
    /// ): determine if a string ends with a given suffix.
    ///
    /// `S.endswith(suffix)` reports whether the string S has the specified
    /// suffix.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "filename.sky".endswith(".sky") == True
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn endswith(
        this: &str,
        #[starlark(require = pos)] suffix: StringOrTuple,
    ) -> anyhow::Result<bool> {
        match suffix {
            StringOrTuple::String(x) => Ok(this.ends_with(x)),
            StringOrTuple::Tuple(xs) => Ok(xs.items.iter().any(|x| this.ends_with(x))),
        }
    }

    /// [string.find](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·find
    /// ): find a substring in a string.
    ///
    /// `S.find(sub[, start[, end]])` returns the index of the first
    /// occurrence of the substring `sub` within S.
    ///
    /// If either or both of `start` or `end` are specified,
    /// they specify a subrange of S to which the search should be restricted.
    /// They are interpreted according to Skylark's [indexing
    /// conventions](#indexing).
    ///
    /// If no occurrence is found, `found` returns -1.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "bonbon".find("on") == 1
    /// "bonbon".find("on", 2) == 4
    /// "bonbon".find("on", 2, 5) == -1
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn find(
        this: &str,
        #[starlark(require = pos)] needle: &str,
        #[starlark(require = pos, default = NoneOr::None)] start: NoneOr<i32>,
        #[starlark(require = pos, default = NoneOr::None)] end: NoneOr<i32>,
    ) -> anyhow::Result<i32> {
        if let Some(StrIndices { start, haystack }) =
            convert_str_indices(this, start.into_option(), end.into_option())
        {
            if let Some(index) = haystack.find(needle) {
                let index = fast_string::len(&haystack[..index]);
                return Ok((start + index).0 as i32);
            }
        }
        Ok(-1)
    }

    /// [string.format](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·format
    /// ): format a string.
    ///
    /// `S.format(*args, **kwargs)` returns a version of the format string S
    /// in which bracketed portions `{...}` are replaced
    /// by arguments from `args` and `kwargs`.
    ///
    /// Within the format string, a pair of braces `{{` or `}}` is treated as
    /// a literal open or close brace.
    /// Each unpaired open brace must be matched by a close brace `}`.
    /// The optional text between corresponding open and close braces
    /// specifies which argument to use and how to format it, and consists of
    /// three components, all optional:
    /// a field name, a conversion preceded by '`!`', and a format specifier
    /// preceded by '`:`'.
    ///
    /// ```text
    /// {field}
    /// {field:spec}
    /// {field!conv}
    /// {field!conv:spec}
    /// ```
    ///
    /// The *field name* may be either a decimal number or a keyword.
    /// A number is interpreted as the index of a positional argument;
    /// a keyword specifies the value of a keyword argument.
    /// If all the numeric field names form the sequence 0, 1, 2, and so on,
    /// they may be omitted and those values will be implied; however,
    /// the explicit and implicit forms may not be mixed.
    ///
    /// The *conversion* specifies how to convert an argument value `x` to a
    /// string. It may be either `!r`, which converts the value using
    /// `repr(x)`, or `!s`, which converts the value using `str(x)` and is
    /// the default.
    ///
    /// The *format specifier*, after a colon, specifies field width,
    /// alignment, padding, and numeric precision.
    /// Currently it must be empty, but it is reserved for future use.
    ///
    /// ```rust
    /// # starlark::assert::all_true(r#"
    /// "a {} c".format(3) == "a 3 c"
    /// "a{x}b{y}c{}".format(1, x=2, y=3) == "a2b3c1"
    /// "a{}b{}c".format(1, 2) == "a1b2c"
    /// "({1}, {0})".format("zero", "one") == "(one, zero)"
    /// "Is {0!r} {0!s}?".format("heterological") == "Is \"heterological\" heterological?"
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn format<'v>(
        this: &str,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StringValue<'v>> {
        let iter = args.positions(eval.heap())?;
        dot_format::format(
            this,
            iter,
            args.names()?,
            &mut eval.string_pool,
            eval.module_env.heap(),
        )
        .map_err(Into::into)
    }

    /// [string.index](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·index
    /// ): search a substring inside a string, failing on not found.
    ///
    /// `S.index(sub[, start[, end]])` returns the index of the first
    /// occurrence of the substring `sub` within S, like `S.find`, except
    /// that if the substring is not found, the operation fails.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "bonbon".index("on") == 1
    /// "bonbon".index("on", 2) == 4
    /// # "#);
    /// # starlark::assert::fail(r#"
    /// "bonbon".index("on", 2, 5)    # error: not found
    /// # "#, "not found");
    /// ```
    #[starlark(speculative_exec_safe)]
    fn index(
        this: &str,
        #[starlark(require = pos)] needle: &str,
        #[starlark(require = pos, default = NoneOr::None)] start: NoneOr<i32>,
        #[starlark(require = pos, default = NoneOr::None)] end: NoneOr<i32>,
    ) -> anyhow::Result<i32> {
        if let Some(StrIndices { start, haystack }) =
            convert_str_indices(this, start.into_option(), end.into_option())
        {
            if let Some(index) = haystack.find(needle) {
                let index = fast_string::len(&haystack[..index]);
                return Ok((start + index).0 as i32);
            }
        }
        Err(anyhow::anyhow!(
            "Substring '{}' not found in '{}'",
            needle,
            this
        ))
    }

    /// [string.isalnum](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·isalnum
    /// ): test if a string is composed only of letters and digits.
    ///
    /// `S.isalnum()` reports whether the string S is non-empty and consists
    /// only Unicode letters and digits.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "base64".isalnum() == True
    /// "Catch-22".isalnum() == False
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn isalnum(this: &str) -> anyhow::Result<bool> {
        if this.is_empty() {
            return Ok(false);
        }
        for c in this.chars() {
            if !c.is_alphanumeric() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// [string.isalpha](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·isalpha
    /// ): test if a string is composed only of letters.
    ///
    /// `S.isalpha()` reports whether the string S is non-empty and consists
    /// only of Unicode letters.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "ABC".isalpha() == True
    /// "Catch-22".isalpha() == False
    /// "".isalpha() == False
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn isalpha(this: &str) -> anyhow::Result<bool> {
        if this.is_empty() {
            return Ok(false);
        }
        for c in this.chars() {
            if !c.is_alphabetic() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// [string.isdigit](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·isdigit
    /// ): test if a string is composed only of digits.
    ///
    /// `S.isdigit()` reports whether the string S is non-empty and consists
    /// only of Unicode digits.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "123".isdigit() == True
    /// "Catch-22".isdigit() == False
    /// "".isdigit() == False
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn isdigit(this: &str) -> anyhow::Result<bool> {
        if this.is_empty() {
            return Ok(false);
        }
        for c in this.chars() {
            if !c.is_numeric() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// [string.islower](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·islower
    /// ): test if all letters of a string are lowercase.
    ///
    /// `S.islower()` reports whether the string S contains at least one cased
    /// Unicode letter, and all such letters are lowercase.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "hello, world".islower() == True
    /// "Catch-22".islower() == False
    /// "123".islower() == False
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn islower(this: &str) -> anyhow::Result<bool> {
        let mut result = false;
        for c in this.chars() {
            if c.is_uppercase() {
                return Ok(false);
            } else if c.is_lowercase() {
                result = true;
            }
        }
        Ok(result)
    }

    /// [string.isspace](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·isspace
    /// ): test if all characters of a string are whitespaces.
    ///
    /// `S.isspace()` reports whether the string S is non-empty and consists
    /// only of Unicode spaces.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "    ".isspace() == True
    /// "\r\t\n".isspace() == True
    /// "".isspace() == False
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn isspace(this: &str) -> anyhow::Result<bool> {
        if this.is_empty() {
            return Ok(false);
        }
        for c in this.chars() {
            if !c.is_whitespace() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// [string.istitle](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·istitle
    /// ): test if the string is title cased.
    ///
    /// `S.istitle()` reports whether the string S contains at least one cased
    /// Unicode letter, and all such letters that begin a word are in title
    /// case.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "Hello, World!".istitle() == True
    /// "Catch-22".istitle() == True
    /// "HAL-9000".istitle() == False
    /// "123".istitle() == False
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn istitle(this: &str) -> anyhow::Result<bool> {
        let mut last_space = true;
        let mut result = false;

        for c in this.chars() {
            if !c.is_alphabetic() {
                last_space = true;
            } else {
                if last_space {
                    if c.is_lowercase() {
                        return Ok(false);
                    }
                } else if c.is_uppercase() {
                    return Ok(false);
                }
                if c.is_alphabetic() {
                    result = true;
                }
                last_space = false;
            }
        }
        Ok(result)
    }

    /// [string.isupper](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·isupper
    /// ): test if all letters of a string are uppercase.
    ///
    /// `S.isupper()` reports whether the string S contains at least one cased
    /// Unicode letter, and all such letters are uppercase.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "HAL-9000".isupper() == True
    /// "Catch-22".isupper() == False
    /// "123".isupper() == False
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn isupper(this: &str) -> anyhow::Result<bool> {
        let mut result = false;
        for c in this.chars() {
            if c.is_lowercase() {
                return Ok(false);
            } else if c.is_uppercase() {
                result = true;
            }
        }
        Ok(result)
    }

    /// [string.lower](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·lower
    /// ): convert a string to all lowercase.
    ///
    /// `S.lower()` returns a copy of the string S with letters converted to
    /// lowercase.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "Hello, World!".lower() == "hello, world!"
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn lower(this: &str) -> anyhow::Result<String> {
        Ok(this.to_lowercase())
    }

    /// [string.join](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·join
    /// ): join elements with a separator.
    ///
    /// `S.join(iterable)` returns the string formed by concatenating each
    /// element of its argument, with a copy of the string S between
    /// successive elements. The argument must be an iterable whose elements
    /// are strings.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// ", ".join([]) == ""
    /// ", ".join(("x", )) == "x"
    /// ", ".join(["one", "two", "three"]) == "one, two, three"
    /// "a".join("ctmrn".elems()) == "catamaran"
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn join<'v>(
        this: &str,
        #[starlark(require = pos)] to_join: ValueOfUnchecked<'v, StarlarkIter<String>>,
        heap: &'v Heap,
    ) -> starlark::Result<ValueOfUnchecked<'v, String>> {
        #[inline(always)]
        fn as_str<'v>(x: Value<'v>) -> anyhow::Result<StringValue<'v>> {
            StringValue::unpack_named_param(x, "to_join")
        }

        let mut it = to_join.get().iterate(heap)?;
        match it.next() {
            None => Ok(ValueOfUnchecked::new(Value::new_empty_string())),
            Some(x1) => {
                match it.next() {
                    None => {
                        // If there is a singleton we can avoid reallocation
                        Ok(as_str(x1)?.to_value_of_unchecked().cast())
                    }
                    Some(x2) => {
                        let s1 = as_str(x1)?.as_str();
                        let s2 = as_str(x2)?.as_str();
                        // guess towards the upper bound, since we throw away over-allocations quickly
                        // include a buffer (20 bytes)
                        let n = it.size_hint().0 + 2;
                        let guess =
                            (cmp::max(s1.len(), s2.len()) * n) + (this.len() * (n - 1)) + 20;
                        let mut r = String::with_capacity(guess);
                        r.push_str(s1);
                        r.push_str(this);
                        r.push_str(s2);
                        for x in it {
                            r.push_str(this);
                            r.push_str(as_str(x)?.as_str());
                        }
                        Ok(heap.alloc_typed_unchecked(r))
                    }
                }
            }
        }
    }

    /// [string.lstrip](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·lstrip
    /// ): trim leading whitespaces.
    ///
    /// `S.lstrip()` returns a copy of the string S with leading whitespace removed.
    /// In most cases instead of passing an argument you should use `removeprefix`.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "  hello  ".lstrip() == "hello  "
    /// "x!hello  ".lstrip("!x ") == "hello  "
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn lstrip<'v>(
        this: StringValue<'v>,
        #[starlark(require = pos)] chars: Option<&str>,
        heap: &'v Heap,
    ) -> anyhow::Result<StringValue<'v>> {
        let res = match chars {
            None => this.trim_start(),
            Some(s) => this.trim_start_matches(|c| s.contains(c)),
        };
        if res.len() == this.len() {
            Ok(this)
        } else {
            Ok(heap.alloc_str(res))
        }
    }

    /// [string.partition](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·partition
    /// ): partition a string in 3 components
    ///
    /// `S.partition(x = " ")` splits string S into three parts and returns them
    /// as a tuple: the portion before the first occurrence of string `x`,
    /// `x` itself, and the portion following it.
    /// If S does not contain `x`, `partition` returns `(S, "", "")`.
    ///
    /// `partition` fails if `x` is not a string, or is the empty string.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "one/two/three".partition("/") == ("one", "/", "two/three")
    /// "one".partition("/") == ("one", "", "")
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn partition<'v>(
        this: StringValue<'v>,
        #[starlark(require = pos)] needle: StringValue<'v>,
        heap: &'v Heap,
    ) -> anyhow::Result<(StringValue<'v>, StringValue<'v>, StringValue<'v>)> {
        if needle.is_empty() {
            return Err(anyhow::anyhow!(
                "Empty separator cannot be used for partitioning"
            ));
        }
        if let Some(offset) = this.find(needle.as_str()) {
            let offset2 = offset + needle.len();
            Ok((
                heap.alloc_str(this.get(..offset).unwrap()),
                needle,
                heap.alloc_str(this.get(offset2..).unwrap()),
            ))
        } else {
            let empty = StringValue::default();
            Ok((this, empty, empty))
        }
    }

    /// [string.replace](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·replace
    /// ): replace all occurrences of a substring.
    ///
    /// `S.replace(old, new[, count])` returns a copy of string S with all
    /// occurrences of substring `old` replaced by `new`. If the optional
    /// argument `count`, which must be an `int`, is non-negative, it
    /// specifies a maximum number of occurrences to replace.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "banana".replace("a", "o") == "bonono"
    /// "banana".replace("a", "o", 2) == "bonona"
    /// "banana".replace("z", "x") == "banana"
    /// "banana".replace("", "x") == "xbxaxnxaxnxax"
    /// "banana".replace("", "x", 2) == "xbxanana"
    /// "".replace("", "x") == "x"
    /// # "# );
    /// # starlark::assert::fail(r#"
    /// "banana".replace("a", "o", -2)  # error: argument was negative
    /// # "#, "argument was negative");
    /// ```
    #[starlark(speculative_exec_safe)]
    fn replace<'v>(
        this: StringValue<'v>,
        #[starlark(require = pos)] old: &str,
        #[starlark(require = pos)] new: &str,
        #[starlark(require = pos)] count: Option<i32>,
        heap: &'v Heap,
    ) -> anyhow::Result<StringValue<'v>> {
        match count {
            Some(count) if count >= 0 => {
                Ok(heap.alloc_str(&this.replacen(old, new, count as usize)))
            }
            Some(count) => Err(anyhow::anyhow!(
                "Replace final argument was negative '{}'",
                count
            )),
            None => {
                // Optimise `replace` using the Rust standard library definition,
                // but avoiding redundant allocation in the last step
                let x = this.as_str();
                let mut result = String::new();
                let mut last_end = 0;
                for (start, part) in x.match_indices(old) {
                    result.push_str(unsafe { x.get_unchecked(last_end..start) });
                    result.push_str(new);
                    last_end = start + part.len();
                }
                if result.is_empty() && last_end == 0 {
                    Ok(this)
                } else {
                    Ok(heap
                        .alloc_str_concat(&result, unsafe { x.get_unchecked(last_end..x.len()) }))
                }
            }
        }
    }

    /// [string.rfind](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·rfind
    /// ): find the last index of a substring.
    ///
    /// `S.rfind(sub[, start[, end]])` returns the index of the substring `sub`
    /// within S, like `S.find`, except that `rfind` returns the index of
    /// the substring's _last_ occurrence.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "bonbon".rfind("on") == 4
    /// "bonbon".rfind("on", None, 5) == 1
    /// "bonbon".rfind("on", 2, 5) == -1
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn rfind(
        this: &str,
        #[starlark(require = pos)] needle: &str,
        #[starlark(require = pos, default = NoneOr::None)] start: NoneOr<i32>,
        #[starlark(require = pos, default = NoneOr::None)] end: NoneOr<i32>,
    ) -> anyhow::Result<i32> {
        if let Some(StrIndices { start, haystack }) =
            convert_str_indices(this, start.into_option(), end.into_option())
        {
            if let Some(index) = haystack.rfind(needle) {
                let index = fast_string::len(&haystack[..index]);
                return Ok((start + index).0 as i32);
            }
        }
        Ok(-1)
    }

    /// [string.rindex](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·rindex
    /// ): find the last index of a substring, failing on not found.
    ///
    /// `S.rindex(sub[, start[, end]])` returns the index of the substring `sub`
    /// within S, like `S.index`, except that `rindex` returns the index of
    /// the substring's _last_ occurrence.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "bonbon".rindex("on") == 4
    /// "bonbon".rindex("on", None, 5) == 1  # in "bonbo"
    /// # "#);
    /// # starlark::assert::fail(r#"
    /// "bonbon".rindex("on", 2, 5) #   error: not found
    /// # "#, "not found");
    /// ```
    #[starlark(speculative_exec_safe)]
    fn rindex(
        this: &str,
        #[starlark(require = pos)] needle: &str,
        #[starlark(require = pos, default = NoneOr::None)] start: NoneOr<i32>,
        #[starlark(require = pos, default = NoneOr::None)] end: NoneOr<i32>,
    ) -> anyhow::Result<i32> {
        if let Some(StrIndices { start, haystack }) =
            convert_str_indices(this, start.into_option(), end.into_option())
        {
            if let Some(index) = haystack.rfind(needle) {
                let index = fast_string::len(&haystack[..index]);
                return Ok((start + index).0 as i32);
            }
        }
        Err(anyhow::anyhow!(
            "Substring '{}' not found in '{}'",
            needle,
            this
        ))
    }

    /// [string.rpartition](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·rpartition
    /// ): partition a string in 3 elements.
    ///
    /// `S.rpartition([x = ' '])` is like `partition`, but splits `S` at the
    /// last occurrence of `x`.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "one/two/three".rpartition("/") == ("one/two", "/", "three")
    /// "one".rpartition("/") == ("", "", "one")
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn rpartition<'v>(
        this: StringValue<'v>,
        #[starlark(require = pos)] needle: StringValue<'v>,
        heap: &'v Heap,
    ) -> anyhow::Result<(StringValue<'v>, StringValue<'v>, StringValue<'v>)> {
        if needle.is_empty() {
            return Err(anyhow::anyhow!(
                "Empty separator cannot be used for partitioning"
            ));
        }
        if let Some(offset) = this.rfind(needle.as_str()) {
            let offset2 = offset + needle.len();
            Ok((
                heap.alloc_str(this.get(..offset).unwrap()),
                needle,
                heap.alloc_str(this.get(offset2..).unwrap()),
            ))
        } else {
            let empty = StringValue::default();
            Ok((empty, empty, this))
        }
    }

    /// [string.rsplit](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·rsplit
    /// ): splits a string into substrings.
    ///
    /// `S.rsplit([sep[, maxsplit]])` splits a string into substrings like
    /// `S.split`, except that when a maximum number of splits is specified,
    /// `rsplit` chooses the rightmost splits.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "banana".rsplit("n") == ["ba", "a", "a"]
    /// "banana".rsplit("n", 1) == ["bana", "a"]
    /// "one two  three".rsplit(None, 1) == ["one two", "three"]
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn rsplit<'v>(
        this: &str,
        #[starlark(require = pos, default = NoneOr::None)] sep: NoneOr<&str>,
        #[starlark(require = pos, default = NoneOr::None)] maxsplit: NoneOr<i32>,
        heap: &'v Heap,
    ) -> anyhow::Result<ValueOfUnchecked<'v, UnpackList<String>>> {
        let maxsplit = match maxsplit.into_option() {
            None => None,
            Some(v) => {
                if v < 0 {
                    None
                } else {
                    Some((v + 1) as usize)
                }
            }
        };
        Ok(match sep.into_option() {
            None => match maxsplit {
                None => heap
                    .alloc_typed_unchecked(AllocList(this.split_whitespace()))
                    .cast(),
                Some(maxsplit) => heap
                    .alloc_typed_unchecked(rsplitn_whitespace(this, maxsplit))
                    .cast(),
            },
            Some(sep) => {
                let mut v: Vec<_> = match maxsplit {
                    None => this.rsplit(sep).collect(),
                    Some(maxsplit) => this.rsplitn(maxsplit, sep).collect(),
                };
                v.reverse();
                heap.alloc_typed_unchecked(AllocList(v)).cast()
            }
        })
    }

    /// [string.rstrip](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·rstrip
    /// ): trim trailing whitespace.
    ///
    /// `S.rstrip()` returns a copy of the string S with trailing whitespace removed.
    /// In most cases instead of passing an argument you should use `removesuffix`.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "  hello  ".rstrip() == "  hello"
    /// "  hello!x".rstrip(" x!") == "  hello"
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn rstrip<'v>(
        this: StringValue<'v>,
        #[starlark(require = pos)] chars: Option<&str>,
        heap: &'v Heap,
    ) -> anyhow::Result<StringValue<'v>> {
        let res = match chars {
            None => this.trim_end(),
            Some(s) => this.trim_end_matches(|c| s.contains(c)),
        };
        if res.len() == this.len() {
            Ok(this)
        } else {
            Ok(heap.alloc_str(res))
        }
    }

    /// [string.split](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·split
    /// ): split a string in substrings.
    ///
    /// `S.split([sep [, maxsplit]])` returns the list of substrings of S,
    /// splitting at occurrences of the delimiter string `sep`.
    ///
    /// Consecutive occurrences of `sep` are considered to delimit empty
    /// strings, so `'food'.split('o')` returns `['f', '', 'd']`.
    /// Splitting an empty string with a specified separator returns `['']`.
    /// If `sep` is the empty string, `split` fails.
    ///
    /// If `sep` is not specified or is `None`, `split` uses a different
    /// algorithm: it removes all leading spaces from S
    /// (or trailing spaces in the case of `rsplit`),
    /// then splits the string around each consecutive non-empty sequence of
    /// Unicode white space characters.
    ///
    /// If S consists only of white space, `split` returns the empty list.
    ///
    /// If `maxsplit` is given and non-negative, it specifies a maximum number
    /// of splits.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "one two  three".split() == ["one", "two", "three"]
    /// "one two  three".split(" ") == ["one", "two", "", "three"]
    /// "one two  three".split(None, 1) == ["one", "two  three"]
    /// "banana".split("n") == ["ba", "a", "a"]
    /// "banana".split("n", 1) == ["ba", "ana"]
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn split<'v>(
        this: &str,
        #[starlark(require = pos, default = NoneOr::None)] sep: NoneOr<&str>,
        #[starlark(require = pos, default = NoneOr::None)] maxsplit: NoneOr<i32>,
        heap: &'v Heap,
    ) -> anyhow::Result<ValueOfUnchecked<'v, UnpackList<String>>> {
        let maxsplit = match maxsplit.into_option() {
            None => None,
            Some(v) => {
                if v < 0 {
                    None
                } else {
                    Some((v + 1) as usize)
                }
            }
        };
        Ok(match (sep.into_option(), maxsplit) {
            (None, None) => heap
                .alloc_typed_unchecked(AllocList(this.split_whitespace()))
                .cast(),
            (None, Some(maxsplit)) => heap
                .alloc_typed_unchecked(AllocList(splitn_whitespace(this, maxsplit)))
                .cast(),
            (Some(sep), None) => {
                if sep.len() == 1 {
                    // If we are searching for a 1-byte string, we can provide a much faster path.
                    // Since it is one byte, given how UTF8 works, all the resultant slices must be UTF8 too.
                    let b = sep.as_bytes()[0];
                    let count = fast_string::count_matches_byte(this, b);
                    let mut res = Vec::with_capacity(count + 1);
                    res.extend(
                        this.as_bytes()
                            .split(|x| *x == b)
                            .map(|x| unsafe { std::str::from_utf8_unchecked(x) }),
                    );
                    debug_assert_eq!(res.len(), count + 1);
                    heap.alloc_typed_unchecked(AllocList(res)).cast()
                } else {
                    heap.alloc_typed_unchecked(AllocList(this.split(sep)))
                        .cast()
                }
            }
            (Some(sep), Some(maxsplit)) => heap
                .alloc_typed_unchecked(AllocList(this.splitn(maxsplit, sep)))
                .cast(),
        })
    }

    /// [string.splitlines](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·splitlines
    /// ): return the list of lines of a string.
    ///
    /// `S.splitlines([keepends])` returns a list whose elements are the
    /// successive lines of S, that is, the strings formed by splitting S at
    /// line terminators ('\n', '\r' or '\r\n').
    ///
    /// The optional argument, `keepends`, is interpreted as a Boolean.
    /// If true, line terminators are preserved in the result, though
    /// the final element does not necessarily end with a line terminator.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "one\n\ntwo".splitlines() == ["one", "", "two"]
    /// "one\n\ntwo".splitlines(True) == ["one\n", "\n", "two"]
    /// "a\nb".splitlines() == ["a", "b"]
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn splitlines<'v>(
        this: &str,
        #[starlark(require = pos, default = false)] keepends: bool,
        heap: &'v Heap,
    ) -> anyhow::Result<Vec<StringValue<'v>>> {
        let mut s = this;
        let mut lines: Vec<StringValue> = Vec::new();
        loop {
            if let Some(x) = s.find(['\n', '\r']) {
                let y = x;
                let x = match s.get(y..y + 2) {
                    Some("\r\n") => y + 2,
                    _ => y + 1,
                };
                if keepends {
                    lines.push(heap.alloc_str(s.get(..x).unwrap()))
                } else {
                    lines.push(heap.alloc_str(s.get(..y).unwrap()))
                }
                if x == s.len() {
                    return Ok(lines);
                }
                s = s.get(x..).unwrap();
            } else {
                if !s.is_empty() {
                    lines.push(heap.alloc_str(s));
                }
                return Ok(lines);
            }
        }
    }

    /// [string.startswith](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·startswith
    /// ): test whether a string starts with a given prefix.
    ///
    /// `S.startswith(suffix)` reports whether the string S has the specified
    /// prefix.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "filename.sky".startswith("filename") == True
    /// "filename.sky".startswith("sky") == False
    /// 'abc'.startswith(('a', 'A')) == True
    /// 'ABC'.startswith(('a', 'A')) == True
    /// 'def'.startswith(('a', 'A')) == False
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn startswith(
        this: &str,
        #[starlark(require = pos)] prefix: StringOrTuple,
    ) -> anyhow::Result<bool> {
        match prefix {
            StringOrTuple::String(x) => Ok(this.starts_with(x)),
            StringOrTuple::Tuple(xs) => Ok(xs.items.iter().any(|x| this.starts_with(x))),
        }
    }

    /// [string.strip](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·strip
    /// ): trim leading and trailing whitespaces.
    ///
    /// `S.strip()` returns a copy of the string S with leading and trailing
    /// whitespace removed.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "  hello  ".strip() == "hello"
    /// "xxhello!!".strip("x!") == "hello"
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn strip<'v>(
        this: StringValue<'v>,
        #[starlark(require = pos)] chars: Option<&str>,
        heap: &'v Heap,
    ) -> anyhow::Result<StringValue<'v>> {
        let res = match chars {
            None => this.trim(),
            Some(s) => this.trim_matches(|c| s.contains(c)),
        };
        if res.len() == this.len() {
            Ok(this)
        } else {
            Ok(heap.alloc_str(res))
        }
    }

    /// [string.title](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·title
    /// ): convert a string to title case.
    ///
    /// `S.title()` returns a copy of the string S with letters converted to
    /// titlecase.
    ///
    /// Letters are converted to uppercase at the start of words, lowercase
    /// elsewhere.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "hElLo, WoRlD!".title() == "Hello, World!"
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn title(this: &str) -> anyhow::Result<String> {
        let mut last_space = true;
        let mut result = String::with_capacity(this.len());
        for c in this.chars() {
            if !c.is_alphabetic() {
                last_space = true;
                result.extend(c.to_lowercase());
            } else {
                if last_space {
                    result.extend(c.to_uppercase())
                } else {
                    result.extend(c.to_lowercase())
                }
                last_space = false;
            }
        }
        Ok(result)
    }

    /// [string.upper](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#string·upper
    /// ): convert a string to all uppercase.
    ///
    /// `S.upper()` returns a copy of the string S with letters converted to
    /// uppercase.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "Hello, World!".upper() == "HELLO, WORLD!"
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn upper(this: &str) -> anyhow::Result<String> {
        Ok(this.to_uppercase())
    }

    /// [string.removeprefix](
    /// https://docs.python.org/3.9/library/stdtypes.html#str.removeprefix
    /// ): remove a prefix from a string. _Not part of standard Starlark._
    ///
    /// If the string starts with the prefix string, return `string[len(prefix):]`.
    /// Otherwise, return a copy of the original string:
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "Hello, World!".removeprefix("Hello") == ", World!"
    /// "Hello, World!".removeprefix("Goodbye") == "Hello, World!"
    /// "Hello".removeprefix("Hello") == ""
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn removeprefix<'v>(
        this: StringValue<'v>,
        #[starlark(require = pos)] prefix: &str,
        heap: &'v Heap,
    ) -> anyhow::Result<StringValue<'v>> {
        let x = this.as_str();
        if x.starts_with(prefix) && !prefix.is_empty() {
            Ok(heap.alloc_str(&x[prefix.len()..]))
        } else {
            Ok(this)
        }
    }

    /// [string.removesuffix](
    /// https://docs.python.org/3.9/library/stdtypes.html#str.removesuffix
    /// ): remove a prefix from a string. _Not part of standard Starlark._
    ///
    /// If the string starts with the prefix string, return `string[len(prefix):]`.
    /// Otherwise, return a copy of the original string:
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// "Hello, World!".removesuffix("World!") == "Hello, "
    /// "Hello, World!".removesuffix("World") == "Hello, World!"
    /// "Hello".removesuffix("Hello") == ""
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn removesuffix<'v>(
        this: StringValue<'v>,
        #[starlark(require = pos)] suffix: &str,
        heap: &'v Heap,
    ) -> anyhow::Result<StringValue<'v>> {
        let x = this.as_str();
        if x.ends_with(suffix) && !suffix.is_empty() {
            Ok(heap.alloc_str(&x[..x.len() - suffix.len()]))
        } else {
            Ok(this)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;

    #[test]
    fn test_error_codes() {
        assert::fail(r#""bonbon".index("on", 2, 5)"#, "not found in");
        assert::fail(r#"("banana".replace("a", "o", -2))"#, "negative");
        assert::fail(r#""bonbon".rindex("on", 2, 5)"#, "not found in");
    }

    #[test]
    fn test_count() {
        assert::eq("'abc'.count('a', 10, -10)", "0");
    }

    #[test]
    fn test_find() {
        assert::eq("'Троянская война окончена'.find('война')", "10");
    }

    #[test]
    fn test_opaque_iterator() {
        assert::is_true("type('foo'.elems()) != type([])");
        assert::is_true("type('foo'.codepoints()) != type([])");
    }
}
