---
name: Bug report
about: Create a report to help us improve.
title: "[BUG]"
labels: bug
assignees: Alexhuszagh

---

## Description

Please include a clear and concise description of the bug. If the bug includes a security vulnerability, you may also privately report the issue to the [maintainer](mailto:ahuszagh@gmail.com).

## Prerequisites

Here are a few things you should provide to help me understand the issue:

- Rust version: `rustc -V`
- minimal-lexical version:

## Test case

Please provide a short, complete (with crate import, etc) test case for
the issue, showing clearly the expected and obtained results.

Example test case:

```
#[macro_use]
extern crate minimal_Lexical;

fn main() {
    let integer = b"1";
    let fraction = b"2345";
    let float: f64 = minimal_lexical::parse_float(integer.iter(), fraction.iter(), 0);
    assert_eq!(value, 1.2345);
}
```

## Additional Context
Add any other context about the problem here.
