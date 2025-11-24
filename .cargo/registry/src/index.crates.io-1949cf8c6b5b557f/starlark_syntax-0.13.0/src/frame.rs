/*
 * Copyright 2019 The Starlark in Rust Authors.
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

use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

use crate::codemap::FileSpan;
use crate::fast_string;
use crate::fast_string::CharIndex;

/// A frame of the call-stack.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Frame {
    /// The name of the entry on the call-stack.
    pub name: String,
    /// The location of the definition, or [`None`] for native Rust functions.
    pub location: Option<FileSpan>,
}

impl Display for Frame {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)?;
        if let Some(loc) = &self.location {
            write!(f, " (called from {})", loc)?;
        }
        Ok(())
    }
}

fn truncate_snippet(snippet: &str, max_len: usize) -> (&str, &str) {
    let ddd = "...";
    assert!(max_len >= ddd.len());
    match fast_string::split_at(snippet, CharIndex(max_len - ddd.len())) {
        None => (snippet, ""),
        Some((_, b)) if b.chars().nth(3).is_none() => (snippet, ""),
        Some((a, _)) => (a, "..."),
    }
}

impl Frame {
    pub fn write_two_lines(
        &self,
        indent: &str,
        caller: &str,
        write: &mut dyn fmt::Write,
    ) -> fmt::Result {
        if let Some(location) = &self.location {
            let line = location
                .file
                .source_line_at_pos(location.span.begin())
                .trim();
            let (line, ddd) = truncate_snippet(line, 80);
            writeln!(
                write,
                "{}* {}, in {}",
                indent,
                location.resolve().begin_file_line(),
                // Note we print caller function here as in Python, not callee,
                // so in the stack trace, top frame is printed without executed function name.
                caller,
            )?;
            writeln!(write, "{}    {}{}", indent, line, ddd)?;
        } else {
            // Python just omits builtin functions in the traceback.
            writeln!(write, "{}File <builtin>, in {}", indent, caller)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::frame::truncate_snippet;

    #[test]
    fn test_truncate_snippet() {
        assert_eq!(("", ""), truncate_snippet("", 5));
        assert_eq!(("a", ""), truncate_snippet("a", 5));
        assert_eq!(("ab", ""), truncate_snippet("ab", 5));
        assert_eq!(("abc", ""), truncate_snippet("abc", 5));
        assert_eq!(("abcd", ""), truncate_snippet("abcd", 5));
        assert_eq!(("abcde", ""), truncate_snippet("abcde", 5));
        assert_eq!(("ab", "..."), truncate_snippet("abcdef", 5));
        assert_eq!(("ab", "..."), truncate_snippet("abcdefg", 5));
        assert_eq!(("ab", "..."), truncate_snippet("abcdefgh", 5));
        assert_eq!(("ab", "..."), truncate_snippet("abcdefghi", 5));
        assert_eq!(("Київ", ""), truncate_snippet("Київ", 5));
        assert_eq!(("па", "..."), truncate_snippet("паляниця", 5));
    }
}
