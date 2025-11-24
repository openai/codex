#![allow(
    clippy::nonminimal_bool,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::wildcard_imports
)]

mod util;

use crate::util::*;
use semver::{BuildMetadata, Prerelease, Version};

#[test]
fn test_parse() {
    let err = version_err("");
    assert_to_string(err, "empty string, expected a semver version");

    let err = version_err("  ");
    assert_to_string(
        err,
        "unexpected character ' ' while parsing major version number",
    );

    let err = version_err("1");
    assert_to_string(
        err,
        "unexpected end of input while parsing major version number",
    );

    let err = version_err("1.2");
    assert_to_string(
        err,
        "unexpected end of input while parsing minor version number",
    );

    let err = version_err("1.2.3-");
    assert_to_string(err, "empty identifier segment in pre-release identifier");

    let err = version_err("a.b.c");
    assert_to_string(
        err,
        "unexpected character 'a' while parsing major version number",
    );

    let err = version_err("1.2.3 abc");
    assert_to_string(err, "unexpected character ' ' after patch version number");

    let err = version_err("1.2.3-01");
    assert_to_string(err, "invalid leading zero in pre-release identifier");

    let err = version_err("1.2.3++");
    assert_to_string(err, "empty identifier segment in build metadata");

    let err = version_err("07");
    assert_to_string(err, "invalid leading zero in major version number");

    let err = version_err("111111111111111111111.0.0");
    assert_to_string(err, "value of major version number exceeds u64::MAX");

    let err = version_err("8\0");
    assert_to_string(err, "unexpected character '\\0' after major version number");

    let parsed = version("1.2.3");
    let expected = Version::new(1, 2, 3);
    assert_eq!(parsed, expected);
    let expected = Version {
        major: 1,
        minor: 2,
        patch: 3,
        pre: Prerelease::EMPTY,
        build: BuildMetadata::EMPTY,
    };
    assert_eq!(parsed, expected);

    let parsed = version("1.2.3-alpha1");
    let expected = Version {
        major: 1,
        minor: 2,
        patch: 3,
        pre: prerelease("alpha1"),
        build: BuildMetadata::EMPTY,
    };
    assert_eq!(parsed, expected);

    let parsed = version("1.2.3+build5");
    let expected = Version {
        major: 1,
        minor: 2,
        patch: 3,
        pre: Prerelease::EMPTY,
        build: build_metadata("build5"),
    };
    assert_eq!(parsed, expected);

    let parsed = version("1.2.3+5build");
    let expected = Version {
        major: 1,
        minor: 2,
        patch: 3,
        pre: Prerelease::EMPTY,
        build: build_metadata("5build"),
    };
    assert_eq!(parsed, expected);

    let parsed = version("1.2.3-alpha1+build5");
    let expected = Version {
        major: 1,
        minor: 2,
        patch: 3,
        pre: prerelease("alpha1"),
        build: build_metadata("build5"),
    };
    assert_eq!(parsed, expected);

    let parsed = version("1.2.3-1.alpha1.9+build5.7.3aedf");
    let expected = Version {
        major: 1,
        minor: 2,
        patch: 3,
        pre: prerelease("1.alpha1.9"),
        build: build_metadata("build5.7.3aedf"),
    };
    assert_eq!(parsed, expected);

    let parsed = version("1.2.3-0a.alpha1.9+05build.7.3aedf");
    let expected = Version {
        major: 1,
        minor: 2,
        patch: 3,
        pre: prerelease("0a.alpha1.9"),
        build: build_metadata("05build.7.3aedf"),
    };
    assert_eq!(parsed, expected);

    let parsed = version("0.4.0-beta.1+0851523");
    let expected = Version {
        major: 0,
        minor: 4,
        patch: 0,
        pre: prerelease("beta.1"),
        build: build_metadata("0851523"),
    };
    assert_eq!(parsed, expected);

    // for https://nodejs.org/dist/index.json, where some older npm versions are "1.1.0-beta-10"
    let parsed = version("1.1.0-beta-10");
    let expected = Version {
        major: 1,
        minor: 1,
        patch: 0,
        pre: prerelease("beta-10"),
        build: BuildMetadata::EMPTY,
    };
    assert_eq!(parsed, expected);
}

#[test]
fn test_eq() {
    assert_eq!(version("1.2.3"), version("1.2.3"));
    assert_eq!(version("1.2.3-alpha1"), version("1.2.3-alpha1"));
    assert_eq!(version("1.2.3+build.42"), version("1.2.3+build.42"));
    assert_eq!(version("1.2.3-alpha1+42"), version("1.2.3-alpha1+42"));
}

#[test]
fn test_ne() {
    assert_ne!(version("0.0.0"), version("0.0.1"));
    assert_ne!(version("0.0.0"), version("0.1.0"));
    assert_ne!(version("0.0.0"), version("1.0.0"));
    assert_ne!(version("1.2.3-alpha"), version("1.2.3-beta"));
    assert_ne!(version("1.2.3+23"), version("1.2.3+42"));
}

#[test]
fn test_display() {
    assert_to_string(version("1.2.3"), "1.2.3");
    assert_to_string(version("1.2.3-alpha1"), "1.2.3-alpha1");
    assert_to_string(version("1.2.3+build.42"), "1.2.3+build.42");
    assert_to_string(version("1.2.3-alpha1+42"), "1.2.3-alpha1+42");
}

#[test]
fn test_lt() {
    assert!(version("0.0.0") < version("1.2.3-alpha2"));
    assert!(version("1.0.0") < version("1.2.3-alpha2"));
    assert!(version("1.2.0") < version("1.2.3-alpha2"));
    assert!(version("1.2.3-alpha1") < version("1.2.3"));
    assert!(version("1.2.3-alpha1") < version("1.2.3-alpha2"));
    assert!(!(version("1.2.3-alpha2") < version("1.2.3-alpha2")));
    assert!(version("1.2.3+23") < version("1.2.3+42"));
}

#[test]
fn test_le() {
    assert!(version("0.0.0") <= version("1.2.3-alpha2"));
    assert!(version("1.0.0") <= version("1.2.3-alpha2"));
    assert!(version("1.2.0") <= version("1.2.3-alpha2"));
    assert!(version("1.2.3-alpha1") <= version("1.2.3-alpha2"));
    assert!(version("1.2.3-alpha2") <= version("1.2.3-alpha2"));
    assert!(version("1.2.3+23") <= version("1.2.3+42"));
}

#[test]
fn test_gt() {
    assert!(version("1.2.3-alpha2") > version("0.0.0"));
    assert!(version("1.2.3-alpha2") > version("1.0.0"));
    assert!(version("1.2.3-alpha2") > version("1.2.0"));
    assert!(version("1.2.3-alpha2") > version("1.2.3-alpha1"));
    assert!(version("1.2.3") > version("1.2.3-alpha2"));
    assert!(!(version("1.2.3-alpha2") > version("1.2.3-alpha2")));
    assert!(!(version("1.2.3+23") > version("1.2.3+42")));
}

#[test]
fn test_ge() {
    assert!(version("1.2.3-alpha2") >= version("0.0.0"));
    assert!(version("1.2.3-alpha2") >= version("1.0.0"));
    assert!(version("1.2.3-alpha2") >= version("1.2.0"));
    assert!(version("1.2.3-alpha2") >= version("1.2.3-alpha1"));
    assert!(version("1.2.3-alpha2") >= version("1.2.3-alpha2"));
    assert!(!(version("1.2.3+23") >= version("1.2.3+42")));
}

#[test]
fn test_spec_order() {
    let vs = [
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0-alpha.beta",
        "1.0.0-beta",
        "1.0.0-beta.2",
        "1.0.0-beta.11",
        "1.0.0-rc.1",
        "1.0.0",
    ];
    let mut i = 1;
    while i < vs.len() {
        let a = version(vs[i - 1]);
        let b = version(vs[i]);
        assert!(a < b, "nope {:?} < {:?}", a, b);
        i += 1;
    }
}

#[test]
fn test_align() {
    let version = version("1.2.3-rc1");
    assert_eq!("1.2.3-rc1           ", format!("{:20}", version));
    assert_eq!("*****1.2.3-rc1******", format!("{:*^20}", version));
    assert_eq!("           1.2.3-rc1", format!("{:>20}", version));
}
