//! Code for parsing resource management things

use super::{Binding, Component, Entry};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

// =======================
// Common helper functions
// =======================

/// Check if a character (well, u8) is an octal digit
fn is_octal_digit(c: u8) -> bool {
    matches!(c, b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7')
}

/// Find the longest prefix of the given data where the given callback returns true
fn parse_with_matcher<M>(data: &[u8], matcher: M) -> (&[u8], &[u8])
where
    M: Fn(u8) -> bool,
{
    let end = data
        .iter()
        .enumerate()
        .find(|(_, &c)| !matcher(c))
        .map(|(idx, _)| idx)
        .unwrap_or(data.len());
    (&data[..end], &data[end..])
}

/// Check if a character is allowed in a quark name
fn allowed_in_quark_name(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b'_'
}

/// Find the longest prefix satisfying allowed_in_quark_name().
/// This returns (Some(prefix), remaining) if a prefix is found, else (None, data).
fn next_component(data: &[u8]) -> (Option<&[u8]>, &[u8]) {
    let (prefix, remaining) = parse_with_matcher(data, allowed_in_quark_name);
    match prefix {
        [] => (None, remaining),
        prefix => (Some(prefix), remaining),
    }
}

// =========================
// Parser for resource files
// =========================

/// Skip to the next end of line in the given data
fn skip_to_eol(data: &[u8]) -> &[u8] {
    parse_with_matcher(data, |c| c != b'\n').1
}

/// Skip all spaces in the given data
fn skip_spaces(data: &[u8]) -> &[u8] {
    parse_with_matcher(data, |c| c == b' ').1
}

/// Skip the given text. Returns `None` if the text was not found
fn skip_text<'a>(data: &'a [u8], text: &[u8]) -> Option<&'a [u8]> {
    if data.starts_with(text) {
        Some(&data[text.len()..])
    } else {
        None
    }
}

/// Parse a single `Component` from the data. This can either be a wildcard ("?") or a
/// component made up of characters accepted by `allowed_in_quark_name`.
fn next_component_name(data: &[u8]) -> (Option<Component>, &[u8]) {
    if data.first() == Some(&b'?') {
        (Some(Component::Wildcard), &data[1..])
    } else {
        let (comp, remaining) = next_component(data);
        let comp = comp.map(|s| {
            let s = std::str::from_utf8(s).expect("ascii-only");
            Component::Normal(s.to_string())
        });
        (comp, remaining)
    }
}

/// Parse a resource like "foo.?*baz" (wildcards allowed)
fn parse_components(data: &[u8]) -> (Vec<(Binding, Component)>, &[u8]) {
    fn parse_binding(mut data: &[u8]) -> (Binding, &[u8]) {
        let mut binding = Binding::Tight;
        loop {
            match data.first() {
                Some(&b'*') => binding = Binding::Loose,
                Some(&b'.') => {}
                _ => break,
            }
            data = &data[1..];
        }
        (binding, data)
    }

    let mut data = data;
    let mut result = Vec::new();
    loop {
        let (binding, remaining) = parse_binding(data);
        if let (Some(component), remaining) = next_component_name(remaining) {
            data = remaining;
            result.push((binding, component));
        } else {
            break;
        }
    }
    (result, data)
}

/// Parse a full entry from the data. This begins with components (see `parse_components()`),
/// then after a colon (":") comes the value. The value may contain escape sequences.
fn parse_entry(data: &[u8]) -> (Result<Entry, ()>, &[u8]) {
    let (components, data) = parse_components(data);

    match components.last() {
        // Empty components are not allowed
        None => return (Err(()), skip_to_eol(data)),
        // The last component may not be a wildcard
        Some((_, Component::Wildcard)) => return (Err(()), skip_to_eol(data)),
        _ => {}
    }

    let data = skip_spaces(data);

    // next comes a colon
    let data = match data.split_first() {
        Some((&b':', data)) => data,
        _ => return (Err(()), skip_to_eol(data)),
    };

    // skip more spaces and let \ escape line breaks
    let mut data = data;
    loop {
        let (_, remaining) = parse_with_matcher(data, |c| c == b' ' || c == b'\t');
        if remaining.get(..2) == Some(&b"\\\n"[..]) {
            data = &remaining[2..];
        } else {
            data = remaining;
            break;
        }
    }

    // Parse the value, decoding escape sequences. The most complicated case are octal escape
    // sequences like \123.
    let mut value = Vec::new();
    let mut index = 0;
    let mut octal = None;
    while let Some(&b) = data.get(index) {
        index += 1;
        if b == b'\n' {
            break;
        }
        if let Some(oct) = octal {
            if is_octal_digit(b) {
                // We are currently parsing an octal; add the new character
                match oct {
                    (x, None) => octal = Some((x, Some(b))),
                    (x, Some(y)) => {
                        let (x, y, z) = (x - b'0', y - b'0', b - b'0');
                        let decoded = (x * 8 + y) * 8 + z;
                        value.push(decoded);
                        octal = None;
                    }
                }
                continue;
            } else {
                // Not an octal sequence; add the collected characters to the output
                value.push(b'\\');
                value.push(oct.0);
                if let Some(oct2) = oct.1 {
                    value.push(oct2);
                }
                octal = None;

                // Fall through to the parsing code below
            }
        }
        if b != b'\\' {
            value.push(b);
        } else {
            match data.get(index) {
                None => {
                    value.push(b);
                    // Keep index as-is. This is to counter the += 1 below.
                    index -= 1;
                }
                Some(b' ') => value.push(b' '),
                Some(b'\t') => value.push(b'\t'),
                Some(b'n') => value.push(b'\n'),
                Some(b'\\') => value.push(b'\\'),
                Some(b'\n') => { /* Continue parsing next line */ }
                Some(&x) if is_octal_digit(x) => octal = Some((x, None)),
                Some(&x) => {
                    value.push(b);
                    value.push(x);
                }
            }
            index += 1;
        }
    }

    let entry = Entry { components, value };
    (Ok(entry), &data[index..])
}

/// Parse the contents of a database
pub(crate) fn parse_database<F>(mut data: &[u8], result: &mut Vec<Entry>, mut include_callback: F)
where
    for<'r> F: FnMut(&'r [u8], &mut Vec<Entry>),
{
    // Iterate over lines
    while let Some(first) = data.first() {
        match first {
            // Skip empty lines
            b'\n' => data = &data[1..],
            // Comment, skip the line
            b'!' => data = skip_to_eol(data),
            b'#' => {
                let remaining = skip_spaces(&data[1..]);
                // Skip to the next line for the next loop iteration. The rest of the code here
                // tried to parse the line.
                data = skip_to_eol(remaining);

                // Only #include is defined
                if let Some(remaining) = skip_text(remaining, b"include") {
                    let (_, remaining) = parse_with_matcher(remaining, |c| c == b' ');
                    // Find the text enclosed in quotation marks
                    if let Some(b'\"') = remaining.first() {
                        let (file, remaining) =
                            parse_with_matcher(&remaining[1..], |c| c != b'"' && c != b'\n');
                        if let Some(b'\"') = remaining.first() {
                            // Okay, we found a well-formed include directive.
                            include_callback(file, result);
                        }
                    }
                }
            }
            _ => {
                let (entry, remaining) = parse_entry(data);
                data = remaining;
                // Add the entry to the result if we parsed one; ignore errors
                result.extend(entry.ok());
            }
        }
    }
}

/// Parse a resource query like "foo.bar.baz" (no wildcards allowed, no bindings allowed)
pub(crate) fn parse_query(data: &[u8]) -> Option<Vec<String>> {
    let mut data = data;
    let mut result = Vec::new();
    while let (Some(component), remaining) = next_component(data) {
        data = remaining;
        while let Some(&b'.') = data.first() {
            data = &data[1..];
        }
        let component = std::str::from_utf8(component).expect("ascii-only");
        result.push(component.to_string());
    }
    if data.is_empty() {
        Some(result)
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use super::{parse_database, parse_entry, parse_query, Binding, Component, Entry};
    use alloc::string::{String, ToString};
    use alloc::vec;
    use alloc::vec::Vec;
    use std::eprintln;

    // Most tests in here are based on [1], which is: Copyright © 2016 Ingo Bürk
    // [1]: https://github.com/Airblader/xcb-util-xrm/blob/master/tests/tests_parser.c

    #[test]
    fn test_parse_query_success() {
        let tests = [
            (
                &b"First.second"[..],
                vec!["First".to_string(), "second".to_string()],
            ),
            (b"", Vec::new()),
            (
                b"urxvt.scrollBar_right",
                vec!["urxvt".to_string(), "scrollBar_right".to_string()],
            ),
            (
                b"urxvt.Control-Shift-Up",
                vec!["urxvt".to_string(), "Control-Shift-Up".to_string()],
            ),
        ];
        for (data, expected) in tests.iter() {
            let result = parse_query(data);
            assert_eq!(result.as_ref(), Some(expected), "while parsing {:?}", data);
        }
    }

    #[test]
    fn test_parse_query_error() {
        let tests = [
            &b"First.second: on"[..],
            b"First*second",
            b"First.?.second",
            b"*second",
            b"?.second",
        ];
        for data in tests.iter() {
            let result = parse_query(data);
            assert!(
                result.is_none(),
                "Unexpected success parsing '{:?}': {:?}",
                data,
                result,
            );
        }
    }

    #[test]
    fn test_parse_entry_success() {
        let tests = [
            // Basics
            (
                &b"First: 1"[..],
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                &b"1"[..],
            ),
            (
                b"First.second: 1",
                vec![
                    (Binding::Tight, Component::Normal("First".to_string())),
                    (Binding::Tight, Component::Normal("second".to_string())),
                ],
                b"1",
            ),
            (
                b"First..second: 1",
                vec![
                    (Binding::Tight, Component::Normal("First".to_string())),
                    (Binding::Tight, Component::Normal("second".to_string())),
                ],
                b"1",
            ),
            // Wildcards
            (
                b"?.second: 1",
                vec![
                    (Binding::Tight, Component::Wildcard),
                    (Binding::Tight, Component::Normal("second".to_string())),
                ],
                b"1",
            ),
            (
                b"First.?.third: 1",
                vec![
                    (Binding::Tight, Component::Normal("First".to_string())),
                    (Binding::Tight, Component::Wildcard),
                    (Binding::Tight, Component::Normal("third".to_string())),
                ],
                b"1",
            ),
            // Loose bindings
            (
                b"*second: 1",
                vec![(Binding::Loose, Component::Normal("second".to_string()))],
                b"1",
            ),
            (
                b"First*third: 1",
                vec![
                    (Binding::Tight, Component::Normal("First".to_string())),
                    (Binding::Loose, Component::Normal("third".to_string())),
                ],
                b"1",
            ),
            (
                b"First**third: 1",
                vec![
                    (Binding::Tight, Component::Normal("First".to_string())),
                    (Binding::Loose, Component::Normal("third".to_string())),
                ],
                b"1",
            ),
            // Combinations
            (
                b"First*?.fourth: 1",
                vec![
                    (Binding::Tight, Component::Normal("First".to_string())),
                    (Binding::Loose, Component::Wildcard),
                    (Binding::Tight, Component::Normal("fourth".to_string())),
                ],
                b"1",
            ),
            // Values
            (
                b"First: 1337",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"1337",
            ),
            (
                b"First: -1337",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"-1337",
            ),
            (
                b"First: 13.37",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"13.37",
            ),
            (
                b"First: value",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"value",
            ),
            (
                b"First: #abcdef",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"#abcdef",
            ),
            (
                b"First: { key: 'value' }",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"{ key: 'value' }",
            ),
            (
                b"First: x?y",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"x?y",
            ),
            (
                b"First: x*y",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"x*y",
            ),
            // Whitespace
            (
                b"First:    x",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"x",
            ),
            (
                b"First: x   ",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"x   ",
            ),
            (
                b"First:    x   ",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"x   ",
            ),
            (
                b"First:x",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"x",
            ),
            (
                b"First: \t x",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"x",
            ),
            (
                b"First: \t x \t",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"x \t",
            ),
            // Special characters
            (
                b"First: \\ x",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b" x",
            ),
            (
                b"First: x\\ x",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"x x",
            ),
            (
                b"First: \\\tx",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"\tx",
            ),
            (
                b"First: \\011x",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"\tx",
            ),
            (
                b"First: x\\\\x",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"x\\x",
            ),
            (
                b"First: x\\nx",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"x\nx",
            ),
            (
                b"First: \\080",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"\\080",
            ),
            (
                b"First: \\00a",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"\\00a",
            ),
            // Own tests
            // Some more escape tests, e.g. escape at end of input
            (
                b"First: \\",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"\\",
            ),
            (
                b"First: \\xxx",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"\\xxx",
            ),
            (
                b"First: \\1xx",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"\\1xx",
            ),
            (
                b"First: \\10x",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"\\10x",
            ),
            (
                b"First: \\100",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"@",
            ),
            (
                b"First: \\n",
                vec![(Binding::Tight, Component::Normal("First".to_string()))],
                b"\n",
            ),
        ];
        for (data, resource, value) in tests.iter() {
            run_entry_test(data, resource, value);
        }
    }

    #[test]
    fn test_parse_entry_error() {
        let tests = [
            &b": 1"[..],
            b"?: 1",
            b"First",
            b"First second",
            b"First.?: 1",
            b"F\xc3\xb6rst: 1",
            b"F~rst: 1",
        ];
        for data in tests.iter() {
            match parse_entry(data) {
                (Ok(v), _) => panic!("Unexpected success parsing '{:?}': {:?}", data, v),
                (Err(_), b"") => {}
                (Err(_), remaining) => panic!(
                    "Unexpected remaining data parsing '{:?}': {:?}",
                    data, remaining
                ),
            }
        }
    }

    #[test]
    fn test_parse_large_value() {
        let value = vec![b'x'; 1025];
        let mut data = b"First: ".to_vec();
        data.extend(&value);
        let resource = (Binding::Tight, Component::Normal("First".to_string()));
        run_entry_test(&data, &[resource], &value);
    }

    #[test]
    fn test_parse_large_resource() {
        let x = vec![b'x'; 1025];
        let y = vec![b'y'; 1025];
        let mut data = x.clone();
        data.push(b'.');
        data.extend(&y);
        data.extend(b": 1");
        let resource = [
            (
                Binding::Tight,
                Component::Normal(String::from_utf8(x).unwrap()),
            ),
            (
                Binding::Tight,
                Component::Normal(String::from_utf8(y).unwrap()),
            ),
        ];
        run_entry_test(&data, &resource, b"1");
    }

    #[test]
    fn test_parse_database() {
        let expected_entry = Entry {
            components: vec![(Binding::Tight, Component::Normal("First".to_string()))],
            value: b"1".to_vec(),
        };
        let tests = [
            (&b"First: 1\n\n\n"[..], vec![expected_entry.clone()]),
            (b"First: 1\n!Foo", vec![expected_entry.clone()]),
            (b"!First: 1\nbar\n\n\n", Vec::new()),
            (b"!bar\nFirst: 1\nbaz", vec![expected_entry.clone()]),
            (b"First :\\\n \\\n\\\n1\n", vec![expected_entry]),
            (
                b"First: \\\n  1\\\n2\n",
                vec![Entry {
                    components: vec![(Binding::Tight, Component::Normal("First".to_string()))],
                    value: b"12".to_vec(),
                }],
            ),
        ];
        let mut success = true;
        for (data, expected) in tests.iter() {
            let mut result = Vec::new();
            parse_database(data, &mut result, |_, _| unreachable!());
            if &result != expected {
                eprintln!("While testing {:?}", data);
                eprintln!("Expected: {:?}", expected);
                eprintln!("Got:      {:?}", result);
                eprintln!();
                success = false;
            }
        }
        if !success {
            panic!()
        }
    }

    #[test]
    fn test_include_parsing() {
        let tests = [
            (&b"#include\"test\""[..], vec![&b"test"[..]]),
            (b"#include\"test", Vec::new()),
            (b"#include\"", Vec::new()),
            (b"#include", Vec::new()),
            (b"#includ", Vec::new()),
            (b"#in", Vec::new()),
            (b"#  foo", Vec::new()),
            (
                b"#  include   \" test \"   \n#include  \"foo\"",
                vec![b" test ", b"foo"],
            ),
        ];
        let mut success = true;
        for (data, expected) in tests.iter() {
            let mut result = Vec::new();
            let mut calls = Vec::new();
            parse_database(data, &mut result, |file, _| calls.push(file.to_vec()));
            if &calls != expected {
                eprintln!("While testing {:?}", data);
                eprintln!("Expected: {:?}", expected);
                eprintln!("Got:      {:?}", calls);
                eprintln!();
                success = false;
            }
        }
        if !success {
            panic!()
        }
    }

    #[test]
    fn test_include_additions() {
        let entry = Entry {
            components: Vec::new(),
            value: b"42".to_vec(),
        };
        let mut result = Vec::new();
        parse_database(b"#include\"test\"", &mut result, |file, result| {
            assert_eq!(file, b"test");
            result.push(entry.clone());
        });
        assert_eq!(result, [entry]);
    }

    fn run_entry_test(data: &[u8], resource: &[(Binding, Component)], value: &[u8]) {
        match parse_entry(data) {
            (Ok(result), remaining) => {
                assert_eq!(remaining, b"", "failed to parse {:?}", data);
                assert_eq!(
                    result.components, resource,
                    "incorrect components when parsing {:?}",
                    data
                );
                assert_eq!(
                    result.value, value,
                    "incorrect value when parsing {:?}",
                    data
                );
            }
            (Err(err), _) => panic!("Failed to parse '{:?}': {:?}", data, err),
        }
    }
}
