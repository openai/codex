// Copyright 2013-2014 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(clippy::assigning_clones)]
#![allow(deprecated)]

use crate::test::TestFn;
use std::char;
use std::fmt::Write;

use idna::Errors;

pub fn collect_tests<F: FnMut(String, TestFn)>(add_test: &mut F) {
    // https://www.unicode.org/Public/idna/13.0.0/IdnaTestV2.txt
    for (i, line) in include_str!("IdnaTestV2.txt").lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Remove comments
        let line = match line.find('#') {
            Some(index) => &line[0..index],
            None => line,
        };

        let mut pieces = line.split(';').map(|x| x.trim()).collect::<Vec<&str>>();
        let source = unescape(pieces.remove(0));

        // ToUnicode
        let mut to_unicode = unescape(pieces.remove(0));
        if to_unicode.is_empty() {
            to_unicode = source.clone();
        }
        let to_unicode_status = status(pieces.remove(0));

        // ToAsciiN
        let to_ascii_n = pieces.remove(0);
        let to_ascii_n = if to_ascii_n.is_empty() {
            to_unicode.clone()
        } else {
            to_ascii_n.to_owned()
        };
        let to_ascii_n_status = pieces.remove(0);
        let to_ascii_n_status = if to_ascii_n_status.is_empty() {
            to_unicode_status.clone()
        } else {
            status(to_ascii_n_status)
        };

        // ToAsciiT
        let to_ascii_t = pieces.remove(0);
        let to_ascii_t = if to_ascii_t.is_empty() {
            to_ascii_n.clone()
        } else {
            to_ascii_t.to_owned()
        };
        let to_ascii_t_status = pieces.remove(0);
        let to_ascii_t_status = if to_ascii_t_status.is_empty() {
            to_ascii_n_status.clone()
        } else {
            status(to_ascii_t_status)
        };

        let test_name = format!("UTS #46 (deprecated API) line {}", i + 1);
        add_test(
            test_name,
            TestFn::DynTestFn(Box::new(move || {
                let config = idna::Config::default()
                    .use_std3_ascii_rules(true)
                    .verify_dns_length(true)
                    .check_hyphens(true);

                // http://unicode.org/reports/tr46/#Deviations
                // applications that perform IDNA2008 lookup are not required to check
                // for these contexts, so we skip all tests annotated with C*

                // Everybody ignores V2
                // https://github.com/servo/rust-url/pull/240
                // https://github.com/whatwg/url/issues/53#issuecomment-181528158
                // http://www.unicode.org/review/pri317/

                // "The special error codes X3 and X4_2 are now returned where a toASCII error code
                // was formerly being generated in toUnicode due to an empty label."
                // This is not implemented yet, so we skip toUnicode X4_2 tests for now, too.

                let (to_unicode_value, to_unicode_result) =
                    config.transitional_processing(false).to_unicode(&source);
                let to_unicode_result = to_unicode_result.map(|()| to_unicode_value);
                check(
                    &source,
                    (&to_unicode, &to_unicode_status),
                    to_unicode_result,
                    |e| e == "X4_2" || e == "V2",
                );

                let to_ascii_n_result = config.transitional_processing(false).to_ascii(&source);
                check(
                    &source,
                    (&to_ascii_n, &to_ascii_n_status),
                    to_ascii_n_result,
                    |e| e == "V2",
                );

                let to_ascii_t_result = config.transitional_processing(true).to_ascii(&source);
                check(
                    &source,
                    (&to_ascii_t, &to_ascii_t_status),
                    to_ascii_t_result,
                    |e| e == "V2",
                );
            })),
        )
    }
}

#[allow(clippy::redundant_clone)]
fn check<F>(source: &str, expected: (&str, &[&str]), actual: Result<String, Errors>, ignore: F)
where
    F: Fn(&str) -> bool,
{
    if !expected.1.is_empty() {
        if !expected.1.iter().copied().any(ignore) {
            let res = actual.ok();
            assert_eq!(
                res.clone(),
                None,
                "Expected error {:?}. result: {} | source: {}",
                expected.1,
                res.unwrap(),
                source,
            );
        }
    } else {
        assert!(
            actual.is_ok(),
            "Couldn't parse {} | error: {:?}",
            source,
            actual.err().unwrap(),
        );
        assert_eq!(actual.unwrap(), expected.0, "source: {}", source);
    }
}

fn unescape(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars();
    loop {
        match chars.next() {
            None => return output,
            Some(c) => {
                if c == '\\' {
                    match chars.next().unwrap() {
                        '\\' => output.push('\\'),
                        'u' => {
                            let c1 = chars.next().unwrap().to_digit(16).unwrap();
                            let c2 = chars.next().unwrap().to_digit(16).unwrap();
                            let c3 = chars.next().unwrap().to_digit(16).unwrap();
                            let c4 = chars.next().unwrap().to_digit(16).unwrap();
                            match char::from_u32(((c1 * 16 + c2) * 16 + c3) * 16 + c4) {
                                Some(c) => output.push(c),
                                None => {
                                    write!(&mut output, "\\u{:X}{:X}{:X}{:X}", c1, c2, c3, c4)
                                        .expect("Could not write to output");
                                }
                            };
                        }
                        _ => panic!("Invalid test data input"),
                    }
                } else {
                    output.push(c);
                }
            }
        }
    }
}

fn status(status: &str) -> Vec<&str> {
    if status.is_empty() || status == "[]" {
        return Vec::new();
    }

    let mut result = status.split(", ").collect::<Vec<_>>();
    assert!(result[0].starts_with('['));
    result[0] = &result[0][1..];

    let idx = result.len() - 1;
    let last = &mut result[idx];
    assert!(last.ends_with(']'));
    *last = &last[..last.len() - 1];

    result
}
