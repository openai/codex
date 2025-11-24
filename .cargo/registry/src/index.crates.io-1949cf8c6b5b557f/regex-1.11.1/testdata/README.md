This directory contains a large suite of regex tests defined in a TOML format.
They are used to drive tests in `tests/lib.rs`, `regex-automata/tests/lib.rs`
and `regex-lite/tests/lib.rs`.

See the [`regex-test`][regex-test] crate documentation for an explanation of
the format and how it generates tests.

The basic idea here is that we have many different regex engines but generally
one set of tests. We want to be able to run those tests (or most of them) on
every engine. Prior to `regex 1.9`, we used to do this with a hodge podge soup
of macros and a different test executable for each engine. It overall took a
longer time to compile, was harder to maintain and it made the test definitions
themselves less clear.

In `regex 1.9`, when we moved over to `regex-automata`, the situation got a lot
worse because of an increase in the number of engines. So I devised an engine
independent format for testing regex patterns and their semantics.

Note: the naming scheme used in these tests isn't terribly consistent. It would
be great to fix that.

[regex-test]: https://docs.rs/regex-test
