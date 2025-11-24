use tester as test;

mod deprecated;
mod punycode;
mod uts46;

fn main() {
    let mut tests = Vec::new();
    {
        let mut add_test = |name, run| {
            tests.push(test::TestDescAndFn {
                desc: test::TestDesc {
                    name: test::DynTestName(name),
                    ignore: false,
                    should_panic: test::ShouldPanic::No,
                    allow_fail: false,
                    test_type: test::TestType::Unknown,
                },
                testfn: run,
            })
        };
        punycode::collect_tests(&mut add_test);
        deprecated::collect_tests(&mut add_test);
        uts46::collect_tests(&mut add_test);
    }
    test::test_main(&std::env::args().collect::<Vec<_>>(), tests, None)
}
