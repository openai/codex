//! Test Cases

#[cfg_attr(tarpaulin, ignore)]
// The error messages are different on beta and nightly, thus breaking the test.
#[rustversion::attr(beta, ignore)]
#[rustversion::attr(nightly, ignore)]
#[test]
fn compile_test() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail/*.rs");
}
