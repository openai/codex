mod util;
use smallvec::SmallVec;
use util::*;

#[test]
fn smallvec() -> TestResult {
    test_default_generated_schema::<SmallVec<[String; 2]>>("smallvec")
}
