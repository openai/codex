#![allow(dead_code)]
#![cfg(feature = "semver-impl")]

use semver::Version;
use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "semver/")]
struct Semver {
    version: Version,
}

#[test]
fn semver() {
    assert_eq!(Semver::decl(), "type Semver = { version: string, };")
}
