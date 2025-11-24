// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(not(miri))]

#[rustversion::attr(not(nightly), ignore)]
#[test]
fn expandtest() {
    let args = &["--all-features"];
    macrotest::expand_args("tests/expand/**/*.rs", args);
}
