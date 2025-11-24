extern crate trybuild;

#[test]
#[ignore]
fn compile_test() {
    let t = trybuild::TestCases::new();
    let pattern = std::env::var("DERIVATIVE_TEST_FILTER").unwrap_or_else(|_| String::from("*.rs"));
    t.compile_fail(format!("tests/compile-fail/{}", pattern));
}
