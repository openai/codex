# Date and time utils for HTTP.

[![Build Status](https://github.com/pyfisch/httpdate/actions/workflows/ci.yml/badge.svg)](https://github.com/pyfisch/httpdate/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/httpdate.svg)](https://crates.io/crates/httpdate)
[![Documentation](https://docs.rs/httpdate/badge.svg)](https://docs.rs/httpdate)

Multiple HTTP header fields store timestamps.
For example a response created on May 15, 2015 may contain the header
`Date: Fri, 15 May 2015 15:34:21 GMT`. Since the timestamp does not
contain any timezone or leap second information it is equvivalent to
writing 1431696861 Unix time. Rust’s `SystemTime` is used to store
these timestamps.

This crate provides two public functions:

* `parse_http_date` to parse a HTTP datetime string to a system time
* `fmt_http_date` to format a system time to a IMF-fixdate

In addition it exposes the `HttpDate` type that can be used to parse
and format timestamps. Convert a sytem time to `HttpDate` and vice versa.
The `HttpDate` (8 bytes) is smaller than `SystemTime` (16 bytes) and
using the display impl avoids a temporary allocation.

Read the [blog post](https://pyfisch.org/blog/http-datetime-handling/) to learn
more.

Fuzz it by installing *cargo-fuzz* and running `cargo fuzz run fuzz_target_1`.
