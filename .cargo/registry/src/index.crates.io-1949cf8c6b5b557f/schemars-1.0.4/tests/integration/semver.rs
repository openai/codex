use crate::prelude::*;
use semver1::Version;

#[test]
fn semver() {
    test!(Version)
        .assert_snapshot()
        .assert_allows_de_roundtrip(
            [
                "1.2.3",
                "1.2.3-alpha4",
                "1.2.3+build4",
                "1.2.3+04",
                "1.2.3-1.alpha.2+5.build.4.3-21",
            ]
            .into_iter()
            .map(Value::from),
        )
        .assert_rejects_de(
            [
                "1.2",
                "1.2.3.4",
                "1.2.03",
                "1.2.3-alpha..",
                "1.2.3-alpha.04",
                "1.2.3++",
            ]
            .into_iter()
            .map(Value::from),
        )
        .assert_matches_de_roundtrip(arbitrary_values());
}
