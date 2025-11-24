#![cfg(compiletests)]

#[rustversion::stable(1.77)]
#[test]
fn compile_test() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail/*.rs");
}

#[rustversion::not(stable(1.77))]
#[test]
fn wrong_rustc_version() {
    panic!(
        "This is not the expected version of rustc. Error messages vary across compiler versions so tests may produce spurious errors"
    );
}
