#[allow(unused_attributes)]
#[rustversion::attr(not(nightly), ignore)]
#[test]
fn no_std() {
    let t = trybuild::TestCases::new();
    #[cfg(not(feature = "std"))]
    t.compile_fail("tests/no_std/without.rs");
    #[cfg(not(feature = "std"))]
    t.compile_fail("tests/no_std/multi_line.rs");
    #[cfg(not(feature = "std"))]
    t.pass("tests/no_std/multi_line_allow.rs");
    #[cfg(not(feature = "std"))]
    t.compile_fail("tests/no_std/enum_prefix_missing.rs");
    #[cfg(not(feature = "std"))]
    t.pass("tests/no_std/enum_prefix.rs");
    #[cfg(feature = "std")]
    t.compile_fail("tests/std/without.rs");
    #[cfg(feature = "std")]
    t.compile_fail("tests/std/multi_line.rs");
    #[cfg(feature = "std")]
    t.pass("tests/std/multi_line_allow.rs");
    #[cfg(feature = "std")]
    t.compile_fail("tests/std/enum_prefix_missing.rs");
    #[cfg(feature = "std")]
    t.pass("tests/std/enum_prefix.rs");
    #[cfg(feature = "std")]
    t.pass("tests/std/multiple.rs");
    t.pass("tests/no_std/with.rs");
}
