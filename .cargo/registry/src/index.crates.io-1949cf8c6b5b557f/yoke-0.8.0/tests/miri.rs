// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use yoke::Yoke;

// Test for strong protection, should pass under miri with -Zmiri-retag-fields
// See https://github.com/unicode-org/icu4x/issues/3696

fn example(_: Yoke<&'static [u8], Vec<u8>>) {}

#[test]
fn run_test() {
    example(Yoke::attach_to_cart(vec![0, 1, 2], |data| data));
}
