// Only test on stable, since UI tests are bound to change over time

#[rustversion::stable]
#[test]
fn pass() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/*.rs");
}

#[rustversion::stable]
#[test]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/fail/*.rs");
}
