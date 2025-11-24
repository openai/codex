//! Run tests from Oniguruma's test suite, see `oniguruma/README.md`

use std::collections::HashMap;
use std::panic;

use regex::Regex;

use fancy_regex::Regex as FancyRegex;

#[derive(Debug, Eq, Hash, PartialEq)]
struct Test {
    source: String,
    pattern: String,
    text: String,
    assertion: Assertion,
}

#[derive(Debug, Eq, Hash, PartialEq)]
enum Assertion {
    Match {
        group: usize,
        start: usize,
        end: usize,
    },
    NoMatch,
}

/// Extract tests from the C source file (or the ignore file).
///
/// Returns a vec of tuple of the test data and the comment for the test.
fn parse_tests(test_source: &str) -> Vec<(Test, String)> {
    let mut tests = Vec::new();

    let c_string = r#""((?:\\\\|\\"|[^"])*)""#;
    let re = Regex::new(&format!(
        r"(?m)((?:^  //.*\n)*)^\s*((x2|x3|n)\({},\s*{},?([^\)]+)\);)",
        c_string, c_string
    ))
    .unwrap();
    for caps in re.captures_iter(test_source) {
        let comment = caps
            .get(1)
            .unwrap()
            .as_str()
            .replace("  // ", "")
            .trim()
            .to_string();
        let source = caps.get(2).unwrap().as_str().to_string();
        let kind = caps.get(3).unwrap().as_str();
        let pattern = unescape(caps.get(4).unwrap().as_str());
        let text = unescape(caps.get(5).unwrap().as_str());
        let args: Vec<usize> = caps
            .get(6)
            .unwrap()
            .as_str()
            .split(",")
            .map(|s| s.trim().parse().unwrap())
            .collect();

        let assertion = match kind {
            "x2" => Assertion::Match {
                start: args[0],
                end: args[1],
                group: 0,
            },
            "x3" => Assertion::Match {
                start: args[0],
                end: args[1],
                group: args[2],
            },
            "n" => Assertion::NoMatch,
            _ => {
                panic!("Unexpected test type {}", kind);
            }
        };

        let test = Test {
            source,
            pattern,
            text,
            assertion,
        };

        tests.push((test, comment));
    }
    tests
}

/// Unescape a string as it appears in C source. This is probably not a perfect implementation, but
/// it's good enough for these tests.
fn unescape(escaped: &str) -> String {
    let mut s: Vec<u8> = Vec::new();
    let mut chars = escaped.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let next = chars.next().expect("Expected character after backslash");
                match next {
                    '\\' => {
                        s.push(b'\\');
                    }
                    '"' => {
                        s.push(b'"');
                    }
                    '?' => {
                        // '?' has to be escaped in C to avoid trigraphs
                        s.push(b'?');
                    }
                    'n' => {
                        s.push(b'\n');
                    }
                    'r' => {
                        s.push(b'\r');
                    }
                    '0' => {
                        // octal escape, e.g. \001
                        let mut octal = String::new();
                        octal.push(chars.next().expect("Expected character after \\0"));
                        octal.push(chars.next().expect("Expected second character after \\0"));
                        let num =
                            u8::from_str_radix(&octal, 8).expect("Error parsing octal number");
                        s.push(num);
                    }
                    'x' => {
                        // hex escape, e.g. \x1f
                        let mut hex = String::new();
                        hex.push(chars.next().expect("Expected character after \\x"));
                        hex.push(chars.next().expect("Expected second character after \\x"));
                        let num = u8::from_str_radix(&hex, 16).expect("Error parsing hex number");
                        s.push(num);
                    }
                    _ => {
                        unimplemented!("Unknown escaped character {} in {}", next, escaped);
                    }
                }
            }
            _ => {
                s.append(&mut c.to_string().into_bytes());
            }
        }
    }
    // Some strings in the test are invalid UTF-8. We handle them via ignores.
    String::from_utf8_lossy(&s).to_string()
}

fn run_test(test: &Test) -> Option<String> {
    let Test {
        pattern,
        text,
        assertion,
        ..
    } = test;

    let compile_result = FancyRegex::new(pattern);
    let Ok(regex) = compile_result else {
        let error = format!("{:?}", compile_result.unwrap_err());
        return Some(format!("Compile failed: {}", error));
    };

    match *assertion {
        Assertion::Match { group, start, end } => {
            let captures_result = regex.captures(&text).unwrap();

            if let Some(captures) = captures_result {
                let Some(m) = captures.get(group) else {
                    return Some("Expected group to exist".to_owned());
                };
                if m.start() != start || m.end() != end {
                    Some(format!(
                        "Match found at start {} and end {} (expected {} and {})",
                        m.start(),
                        m.end(),
                        start,
                        end
                    ))
                } else {
                    None
                }
            } else {
                Some("No match found".to_string())
            }
        }
        Assertion::NoMatch => {
            let regex = FancyRegex::new(pattern).unwrap();
            let result = regex.find(&text).unwrap();
            if result.is_some() {
                Some("Match found".to_string())
            } else {
                // We expected it not to match and it didn't -> good
                None
            }
        }
    }
}

#[test]
fn oniguruma() {
    let tests: Vec<Test> = parse_tests(include_str!("oniguruma/test_utf8.c"))
        .into_iter()
        .map(|(test, _comment)| test)
        .collect();
    let ignore: HashMap<Test, String> = parse_tests(include_str!("oniguruma/test_utf8_ignore.c"))
        .into_iter()
        .collect();

    let mut ignored = 0;
    let mut success = 0;

    for test in tests {
        let result = run_test(&test);

        if let Some(expected_failure) = ignore.get(&test) {
            assert!(result.is_some(),
                    "Expected ignored test to fail, but it succeeded. Remove it from the ignore file: {}", &test.source);
            let failure = result.unwrap();
            assert!(failure.starts_with(expected_failure),
                "Expected failure differed for test, change it in the ignore file: {}\nExpected: {}\nActual  : {}\n",
                &test.source, &expected_failure, &failure
            );
            ignored += 1;
        } else {
            if let Some(failure) = result {
                // This is a weird way to do the assertions, but the nice thing about it is that we
                // can run the tests without an "ignore" file and instead of failing, print the
                // content for the ignore file. To do that, disable the assert and enable the print:

                // println!("  // {}\n  {}\n", failure, test.source);
                assert!(false, "Test {} failed: {}", &test.source, failure);
            } else {
                // println!("Success: {}", test.source);
                success += 1;
            }
        }
    }

    println!(
        "{} successful Oniguruma tests, {} ignored",
        success, ignored
    );
}
