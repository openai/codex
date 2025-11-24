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

use std::collections::HashMap;

use dupe::Dupe;
use once_cell::sync::Lazy;
use regex::Regex;
use regex::RegexBuilder;
use starlark_syntax::syntax::ast::AstLiteral;
use starlark_syntax::syntax::ast::AstPayload;
use starlark_syntax::syntax::ast::AstStmtP;
use starlark_syntax::syntax::ast::ExprP;
use starlark_syntax::syntax::ast::StmtP;

use crate::codemap::Spanned;
use crate::docs::DocFunction;
use crate::docs::DocParam;
use crate::docs::DocParams;
use crate::docs::DocReturn;
use crate::docs::DocString;
use crate::typing::Ty;

/// Controls the formatting to use when parsing `DocString`s from raw docstrings
#[derive(Copy, Clone, Dupe)]
pub enum DocStringKind {
    /// Docstrings provided by users in starlark files, following python-y documentation style.
    ///
    /// For functions, they are the piece in `"""` that come right after the `def foo():` line,
    /// and they have sections for additional details. An example from a starlark file might be:
    ///
    /// ```python
    /// """ Module level docs here """
    ///
    /// def some_function(val: "string") -> "string":
    ///     """ This function takes a string and returns it.
    ///
    ///     This is where an explanation might go, but I have none
    ///
    ///     Args:
    ///         val: This is the value that gets returned
    ///
    ///     Returns:
    ///         The original value, because identity functions are fun.
    /// ```
    Starlark,
    /// Docstrings used with `#[starlark_module]` in rust.
    ///
    /// These are the documentation strings prefixed by `///` (like these docs) on
    /// `#[starlark_module]`, and the functions / attributes within it. It supports
    /// a section `# Arguments`, and `# Returns`, and removes some lines from code
    /// blocks that are valid for rustdoc, but not useful for people using these
    /// functions via starlark. An example might be something like:
    ///
    /// ```
    /// # use starlark::starlark_module;
    /// # use starlark::environment::MethodsBuilder;
    /// # use starlark::values::Value;
    ///
    /// /// These are where the module / object level docs go
    /// #[starlark_module]
    /// fn add_some_value(builder: &mut MethodsBuilder) {
    ///     /// attr1 is an attribute that does nothing interesting.
    ///     #[starlark(attribute)]
    ///     fn attr1<'v>(this: Value<'v>) -> starlark::Result<String> {
    ///         let _ = this;
    ///         Ok("attr1".to_owned())
    ///     }
    ///     /// Copies a string
    ///     ///
    ///     /// This is where details would be, if this were
    ///     /// a more interesting function.
    ///     ///
    ///     /// # Arguments
    ///     /// * `s`: This is string that is returned.
    ///     ///
    ///     /// # Returns
    ///     /// The a copy of the original string.
    ///     fn copy_string<'v>(this: Value<'v>, s: &str) -> anyhow::Result<String> {
    ///         let _ = this;
    ///         Ok(s.to_owned())
    ///     }
    /// }
    /// ```
    Rust,
}

impl DocString {
    /// Extracts the docstring from a function or module body, iff the first
    /// statement is a string literal.
    pub(crate) fn extract_raw_starlark_docstring<P: AstPayload>(
        body: &AstStmtP<P>,
    ) -> Option<String> {
        if let StmtP::Statements(stmts) = &body.node {
            if let Some(Spanned {
                node:
                    StmtP::Expression(Spanned {
                        node: ExprP::Literal(AstLiteral::String(s)),
                        ..
                    }),
                ..
            }) = stmts.first()
            {
                return Some(s.node.to_owned());
            }
        };
        None
    }

    fn split_summary_details(s: &str) -> Option<(&str, &str)> {
        let mut summary_len = 0;
        for line in s.split_inclusive('\n') {
            if line.trim().is_empty() {
                let details_start = summary_len + line.len();
                return Some((s[..summary_len].trim(), &s[details_start..]));
            } else {
                summary_len += line.len();
            }
        }
        None
    }

    // Remove any newlines (and surrounding whitespace) in the summary, and
    // replace them with a single space.
    fn normalize_summary(summary: &str) -> String {
        let mut res = String::with_capacity(summary.len());
        for line in summary.lines() {
            if !res.is_empty() {
                res.push(' ');
            }
            res.push_str(line.trim());
        }
        res
    }

    /// Do common work to parse a docstring (dedenting, splitting summary and details, etc)
    pub fn from_docstring(kind: DocStringKind, user_docstring: &str) -> Option<DocString> {
        let trimmed_docs = user_docstring.trim();
        if trimmed_docs.is_empty() {
            None
        } else {
            let split: Option<(&str, &str)> = Self::split_summary_details(trimmed_docs);
            let (summary, details) = match split {
                Some((summary, details)) if !summary.is_empty() && !details.is_empty() => {
                    // Dedent the details separately so that people can have the summary on the
                    // same line as the opening quotes, and the details indented on subsequent
                    // lines.
                    let details = match kind {
                        DocStringKind::Starlark => textwrap::dedent(details).trim().to_owned(),
                        DocStringKind::Rust => {
                            Self::remove_rust_comments(textwrap::dedent(details).trim())
                        }
                    };
                    (summary, Some(details))
                }
                _ => (trimmed_docs, None),
            };

            let summary = Self::normalize_summary(summary);

            Some(DocString { summary, details })
        }
    }

    /// Removes rustdoc-style commented out lines from code blocks.
    fn remove_rust_comments(details: &str) -> String {
        static CODEBLOCK_RE: Lazy<Regex> = Lazy::new(|| {
            RegexBuilder::new(r"```(\w*)\n.*?```")
                .dot_matches_new_line(true)
                .build()
                .expect("regex to compile")
        });
        static COMMENT_RE: Lazy<Regex> = Lazy::new(|| {
            RegexBuilder::new(r"^# .*$\n")
                .multi_line(true)
                .build()
                .expect("regex to compile")
        });
        CODEBLOCK_RE
            .replace_all(details, |caps: &regex::Captures| {
                match caps.get(1).expect("language group").as_str() {
                    "" | "rust" => COMMENT_RE
                        .replace_all(caps.get(0).expect("$0 to exist").as_str(), "")
                        .to_string(),
                    _ => caps.get(0).expect("$0 to exist").as_str().to_owned(),
                }
            })
            .to_string()
    }

    /// Join lines up, dedent them, and trim them
    fn join_and_dedent_lines(lines: &[String]) -> String {
        textwrap::dedent(&lines.join("\n")).trim().to_owned()
    }

    /// Parse the sections out of a docstring's `details` text, and remove the requested sections from the text.
    ///
    /// "sections" are the various things in doc strings like "Arguments:", "Returns:", etc
    ///
    /// # Returns
    /// - A new instance of `DocString`, with the requested sections, if found, removed.
    /// - A mapping of section name, converted to lower case, to the cleaned up section text
    ///     i.e. dedented, section header not present, etc for any found sections.
    fn parse_and_remove_sections(
        self,
        kind: DocStringKind,
        requested_sections: &[&str],
    ) -> (Self, HashMap<String, String>) {
        let mut sections = HashMap::new();

        let mut finish_section =
            |current_section: &mut Option<String>, current_section_text: &mut Vec<String>| {
                if let Some(s) = current_section.take() {
                    sections.insert(s, DocString::join_and_dedent_lines(current_section_text));
                    current_section_text.clear();
                }
            };

        static STARLARK_SECTION_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^([\w -]+):\s*$").unwrap());
        static STARLARK_INDENTED_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(?:\s|$)").unwrap());
        static RUST_SECTION_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^# ([\w -]+)\s*$").unwrap());
        static RUST_INDENTED_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^.*").unwrap());

        let (section_re, indented_re) = match kind {
            DocStringKind::Starlark => (&STARLARK_SECTION_RE, &STARLARK_INDENTED_RE),
            DocStringKind::Rust => (&RUST_SECTION_RE, &RUST_INDENTED_RE),
        };

        if let Some(details) = self.details {
            let mut new_details = vec![];
            let mut current_section = None;
            let mut current_section_text = vec![];

            for line in details.lines() {
                if let Some(matches) = section_re.captures(line) {
                    finish_section(&mut current_section, &mut current_section_text);

                    let found_section = matches.get(1).unwrap().as_str().to_ascii_lowercase();
                    if requested_sections.contains(&found_section.as_str()) {
                        current_section = Some(found_section);
                    } else {
                        new_details.push(line.to_owned());
                    }
                } else if current_section.is_some() && indented_re.is_match(line) {
                    current_section_text.push(line.to_owned());
                } else {
                    new_details.push(line.to_owned());
                    finish_section(&mut current_section, &mut current_section_text);
                }
            }

            finish_section(&mut current_section, &mut current_section_text);

            let joined_details = new_details.join("\n").trim().to_owned();
            let details = match joined_details.is_empty() {
                true => None,
                false => Some(joined_details),
            };
            (
                Self {
                    summary: self.summary,
                    details,
                },
                sections,
            )
        } else {
            (self, sections)
        }
    }
}

impl DocFunction {
    /// Parses function documentation out of a docstring
    ///
    /// # Arguments
    /// * `kind`: The kind of docstring. This determines the formatting that is parsed.
    /// * `params`: The parameters of the function.
    /// * `return_type`: The return type. This is pulled from typing info / directly from users,
    ///                  so it cannot be inferred generically.
    /// * `raw_docstring`: The raw docstring to be parsed and potentially modified,
    ///                    removing the sections detailing arguments and return values.
    ///                    The format is determined by `kind`.
    pub fn from_docstring(
        kind: DocStringKind,
        mut params: DocParams,
        return_type: Ty,
        raw_docstring: Option<&str>,
    ) -> Self {
        match raw_docstring.and_then(|raw| DocString::from_docstring(kind, raw)) {
            Some(ds) => {
                let (function_docstring, sections) =
                    ds.parse_and_remove_sections(kind, &["arguments", "args", "returns", "return"]);

                match sections.get("arguments").or_else(|| sections.get("args")) {
                    Some(args) => {
                        let entries = Self::parse_params(kind, args);
                        for x in &mut params.doc_params_mut() {
                            let DocParam { name, docs, .. } = x;
                            match entries.get(name) {
                                Some(raw) => *docs = DocString::from_docstring(kind, raw),
                                None => {}
                            }
                        }
                    }
                    _ => (),
                }

                let return_docs = sections
                    .get("return")
                    .or_else(|| sections.get("returns"))
                    .and_then(|raw| DocString::from_docstring(kind, raw));

                DocFunction {
                    docs: Some(function_docstring),
                    params,
                    ret: DocReturn {
                        docs: return_docs,
                        typ: return_type,
                    },
                }
            }
            None => DocFunction {
                docs: None,
                params,
                ret: DocReturn {
                    docs: None,
                    typ: return_type,
                },
            },
        }
    }

    /// Parse out parameter docs from an "Args:" section of a docstring
    ///
    /// `args_section` should be dedented, and generally should just be the `args` key of
    /// the `DocString::parse_and_remove_sections()` function call. This is done as a
    /// separate function to reduce the number of times that sections are parsed out of
    /// docstring (e.g. if a user wants both the `Args:` and `Returns:` sections)
    fn parse_params(kind: DocStringKind, args_section: &str) -> HashMap<String, String> {
        static STARLARK_ARG_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^\*{0,2}(\w+):\s*(.*)").unwrap());
        static RUST_ARG_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^(?:\* )?`(\w+)`:?\s*(.*)").unwrap());

        static INDENTED_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(?:\s|$)").unwrap());

        let arg_re = match kind {
            DocStringKind::Starlark => &STARLARK_ARG_RE,
            DocStringKind::Rust => &RUST_ARG_RE,
        };

        let mut ret = HashMap::new();
        let mut current_arg = None;
        let mut current_text = vec![];

        for line in args_section.lines() {
            if let Some(matches) = arg_re.captures(line) {
                if let Some(a) = current_arg.take() {
                    ret.insert(a, DocString::join_and_dedent_lines(&current_text));
                }

                current_arg = Some(matches.get(1).unwrap().as_str().to_owned());

                let doc_match = matches.get(2).unwrap();
                current_text = vec![format!(
                    "{}{}",
                    " ".repeat(doc_match.start()),
                    doc_match.as_str()
                )];
            } else if current_arg.is_some() && INDENTED_RE.is_match(line) {
                current_text.push(line.to_owned());
            }
        }

        if let Some(a) = current_arg.take() {
            ret.insert(a, DocString::join_and_dedent_lines(&current_text));
        }

        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_starlark_docstring() {
        assert_eq!(
            DocString::from_docstring(DocStringKind::Starlark, " "),
            None
        );
        assert_eq!(
            DocString::from_docstring(
                DocStringKind::Starlark,
                " \n\nThis should be the summary\n\n"
            ),
            Some(DocString {
                summary: "This should be the summary".to_owned(),
                details: None,
            })
        );
        assert_eq!(
            DocString::from_docstring(
                DocStringKind::Starlark,
                " \n\nThis should be the summary\n\n "
            ),
            Some(DocString {
                summary: "This should be the summary".to_owned(),
                details: None,
            })
        );
        assert_eq!(
            DocString::from_docstring(
                DocStringKind::Starlark,
                "Summary line here\n    \nDetails after some spaces\n\nand some more newlines"
            ),
            Some(DocString {
                summary: "Summary line here".to_owned(),
                details: Some("Details after some spaces\n\nand some more newlines".to_owned()),
            })
        );
        assert_eq!(
            DocString::from_docstring(
                DocStringKind::Starlark,
                r#"
        This is the summary.
          It has multiple lines and some spaces, and should be collapsed

        This should be a multiline set of details.
        It should be:
            - Dedented
            - Trimmed
            - Split properly from the summary

"#
            ),
            Some(DocString {
                summary: "This is the summary. It has multiple lines and some spaces, and should be collapsed".to_owned(),
                details: Some(
                    concat!(
                        "This should be a multiline set of details.\n",
                        "It should be:\n",
                        "    - Dedented\n",
                        "    - Trimmed\n",
                        "    - Split properly from the summary"
                    )
                    .to_owned()
                ),
            })
        );
        assert_eq!(
            DocString::from_docstring(
                DocStringKind::Starlark,
                r#"This is a summary line that is not dedented like the 'details'

        Typing the first line right after the """ in python docstrings is common,
        while putting the rest of the docstring indented. Just support both so it
        doesn't surprise anyone.
        "#
            ),
            Some(DocString {
                summary: "This is a summary line that is not dedented like the 'details'"
                    .to_owned(),
                details: Some(
                    concat!(
                "Typing the first line right after the \"\"\" in python docstrings is common,\n",
                "while putting the rest of the docstring indented. Just support both so it\n",
                "doesn't surprise anyone."
                )
                    .to_owned()
                ),
            })
        );
    }

    #[test]
    fn parses_rust_docstring() {
        let raw = r#"
        This is the summary line
          that sometimes is split on two lines

        This is the second part. It has some code blocks

        ```
        # foo() {
        "bar"
        # }
        ```

        ```python
        # This is a python comment. Leave it be
        print(1)
        ```

        ```rust
        # other_foo() {
        "other_bar"
        # }
        ```
        "#;

        let parsed = DocString::from_docstring(DocStringKind::Rust, raw).unwrap();
        assert_eq!(
            "This is the summary line that sometimes is split on two lines",
            parsed.summary
        );
        assert_eq!(
            concat!(
                "This is the second part. It has some code blocks\n\n",
                "```\n",
                "\"bar\"\n",
                "```\n\n",
                "```python\n",
                "# This is a python comment. Leave it be\n",
                "print(1)\n",
                "```\n\n",
                "```rust\n",
                "\"other_bar\"",
                "\n```"
            ),
            parsed.details.unwrap()
        );
    }

    #[test]
    fn parses_and_removes_sections_from_starlark_docstring() {
        let raw_docs = r#"This is an example docstring

        We have some details up here that should not be parsed

        Some empty section:
        Example:
            First line of the section

            A newline with no space after it before the second one,
                and a third that's indented further.
        This is not in the example section

        Last:
            This is something in the last section
        "#;
        let expected_docstring = DocString::from_docstring(
            DocStringKind::Starlark,
            r#"This is an example docstring

            We have some details up here that should not be parsed

            Some empty section:
            This is not in the example section

            Last:
                This is something in the last section
            "#,
        )
        .unwrap();

        let expected_sections = HashMap::from([(
            "example".to_owned(),
            concat!(
                "First line of the section\n\n",
                "A newline with no space after it before the second one,\n",
                "    and a third that's indented further."
            )
            .to_owned(),
        )]);

        let ds = DocString::from_docstring(DocStringKind::Starlark, raw_docs).unwrap();
        let (new_ds, sections) =
            ds.parse_and_remove_sections(DocStringKind::Starlark, &["example"]);

        assert_eq!(new_ds, expected_docstring);
        assert_eq!(sections, expected_sections);
    }

    #[test]
    fn parses_and_removes_sections_from_rust_docstring() {
        let raw_docs = r#"This is an example docstring

        We have some details up here that should not be parsed

        # Some Section

        ```
        # This is a commented out line in a codeblock
        fn some_func() {}
        ```

        # Example
        First line of the section

        Note that, unlike starlark doc strings,
        we don't require indentation. The end of a
        section is either a new section appearing,
        or the end of the string.

        # Last
        This is something in the last section
        "#;
        let expected_docstring = DocString::from_docstring(
            DocStringKind::Rust,
            r#"This is an example docstring

        We have some details up here that should not be parsed

        # Some Section

        ```
        fn some_func() {}
        ```

        # Last
        This is something in the last section
        "#,
        )
        .unwrap();

        let expected_sections = HashMap::from([(
            "example".to_owned(),
            concat!(
                "First line of the section\n\n",
                "Note that, unlike starlark doc strings,\n",
                "we don't require indentation. The end of a\n",
                "section is either a new section appearing,\n",
                "or the end of the string.",
            )
            .to_owned(),
        )]);

        let ds = DocString::from_docstring(DocStringKind::Rust, raw_docs).unwrap();
        let (new_ds, sections) = ds.parse_and_remove_sections(DocStringKind::Rust, &["example"]);

        assert_eq!(new_ds, expected_docstring);
        assert_eq!(sections, expected_sections);
    }

    fn arg(name: &str) -> DocParam {
        DocParam {
            name: name.to_owned(),
            docs: None,
            typ: Ty::any(),
            default_value: None,
        }
    }

    #[test]
    fn parses_starlark_function_docstring() {
        let docstring = r#"This is an example docstring

        Details here

        Args:
            arg_foo: The argument named foo
            arg_bar: The argument named bar. It has
                     a longer doc string that spans
                     over three lines
            *args: Docs for args
            **kwargs: Docs for kwargs

        Returns:
            A value
        "#;

        let kind = DocStringKind::Starlark;
        let return_type = Ty::int();
        let expected = DocFunction {
            docs: DocString::from_docstring(kind, "This is an example docstring\n\nDetails here"),
            params: DocParams {
                kwargs: Some(DocParam {
                    name: "kwargs".to_owned(),
                    docs: DocString::from_docstring(kind, "Docs for kwargs"),
                    typ: Ty::any(),
                    default_value: None,
                }),
                args: Some(DocParam {
                    name: "args".to_owned(),
                    docs: DocString::from_docstring(kind, "Docs for args"),
                    typ: Ty::any(),
                    default_value: None,
                }),
                pos_or_named: vec![
                    DocParam {
                        name: "arg_bar".to_owned(),
                        docs: DocString::from_docstring(
                            kind,
                            concat!(
                                "The argument named bar. It has\n",
                                "a longer doc string that spans\n",
                                "over three lines"
                            ),
                        ),
                        typ: Ty::any(),
                        default_value: None,
                    },
                    DocParam {
                        name: "arg_foo".to_owned(),
                        docs: DocString::from_docstring(kind, "The argument named foo"),
                        typ: Ty::any(),
                        default_value: None,
                    },
                ],
                pos_only: Vec::new(),
                named_only: Vec::new(),
            },
            ret: DocReturn {
                docs: DocString::from_docstring(kind, "A value"),
                typ: return_type.clone(),
            },
        };

        let function_docs = DocFunction::from_docstring(
            kind,
            DocParams {
                kwargs: Some(arg("kwargs")),
                args: Some(arg("args")),
                pos_or_named: vec![arg("arg_bar"), arg("arg_foo")],
                pos_only: Vec::new(),
                named_only: Vec::new(),
            },
            return_type,
            Some(docstring),
        );

        assert_eq!(expected, function_docs);
    }

    #[test]
    fn parses_rust_function_docstring() {
        let docstring = r#"This is an example docstring

        Details here

        # Arguments
        * `arg_foo`: The argument named foo
        `arg_bar`: The argument named bar. It has
                   a longer doc string that spans
                   over three lines

        # Returns
        A value
        "#;

        let kind = DocStringKind::Rust;
        let return_type = Ty::int();
        let expected = DocFunction {
            docs: DocString::from_docstring(kind, "This is an example docstring\n\nDetails here"),
            params: DocParams {
                pos_or_named: vec![
                    DocParam {
                        name: "arg_bar".to_owned(),
                        docs: DocString::from_docstring(
                            kind,
                            concat!(
                                "The argument named bar. It has\n",
                                "a longer doc string that spans\n",
                                "over three lines"
                            ),
                        ),
                        typ: Ty::any(),
                        default_value: None,
                    },
                    DocParam {
                        name: "arg_foo".to_owned(),
                        docs: DocString::from_docstring(kind, "The argument named foo"),
                        typ: Ty::any(),
                        default_value: None,
                    },
                ],
                kwargs: None,
                args: None,
                pos_only: Vec::new(),
                named_only: Vec::new(),
            },
            ret: DocReturn {
                docs: DocString::from_docstring(kind, "A value"),
                typ: return_type.clone(),
            },
        };

        let function_docs = DocFunction::from_docstring(
            kind,
            DocParams {
                pos_or_named: vec![arg("arg_bar"), arg("arg_foo")],
                ..DocParams::default()
            },
            return_type,
            Some(docstring),
        );

        assert_eq!(expected, function_docs);
    }
}
