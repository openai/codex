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

//! Run conformance tests, which are used by the Go starlark.
//! e.g. <https://github.com/google/skylark/tree/master/testdata>

// `if_then_panic` is only in newer clippy, delete this in future.
#![allow(unknown_lints)]
// We want to carefully control the panic message.
#![allow(clippy::if_then_panic)]

use itertools::Itertools;

use crate::assert::assert::Assert;

impl<'a> Assert<'a> {
    /// Run a conformance test, e.g. the Go Starlark tests
    pub fn conformance(&self, code: &str) {
        self.conformance_except(code, &[])
    }

    /// Run a conformance test, but where some test cases are allowed to fail.
    /// The `except` argument represents a list of these permissible failures,
    /// in the order they occur in the conformance test set,
    /// identified by a substring that occurs in the test.
    pub fn conformance_except(&self, code: &str, except: &[&str]) {
        let mut except = except.iter().peekable();

        'next: for x in ConformanceTest::parse(code) {
            if let Some(e) = except.peek() {
                if x.code.contains(**e) {
                    except.next();
                    continue 'next;
                }
            }

            x.test(self)
        }

        if let Some(missed) = except.next() {
            panic!("Exception given but not used, `{}`", missed);
        }
    }
}

/// Describe a conformance test
struct ConformanceTest {
    /// The code of the test
    code: String,
    /// If this might throw an error, what is it
    error: Option<(usize, String)>,
}

impl ConformanceTest {
    fn parse(code: &str) -> Vec<Self> {
        // First split on "---"
        code.lines()
            .collect::<Vec<_>>()
            .split(|x| *x == "---")
            .map(|xs| Self {
                code: xs.join("\n"),
                error: xs
                    .iter()
                    .find_position(|x| x.contains("###"))
                    .map(|(i, x)| {
                        (
                            i + 1,
                            (**x).split_once("###").unwrap().1.trim_start().to_owned(),
                        )
                    }),
            })
            .collect()
    }

    fn test(&self, assert: &Assert) {
        fn get_line(err: &crate::Error) -> Option<usize> {
            err.span().map(|span| span.resolve_span().begin.line + 1)
        }

        match &self.error {
            None => {
                assert.pass(&self.code);
            }
            Some((line, _msg)) => {
                // We don't actually check error messages, since these tests were taken from upstream
                // and our error messages are different
                let err = assert.fail(&self.code, "");
                let got = get_line(&err);
                if got != Some(*line) {
                    panic!(
                        "starlark::assert::conformance, failed at wrong line!\nCode:\n{}\nError:\n{}\nExpected: {}\nGot: {:?}\n",
                        &self.code, &err, line, got
                    );
                }
            }
        }
    }
}
