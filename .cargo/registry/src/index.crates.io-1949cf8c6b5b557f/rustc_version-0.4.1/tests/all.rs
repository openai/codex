#![allow(clippy::match_like_matches_macro)]

use std::process::Command;

use rustc_version::{
    version, version_meta, version_meta_for, Channel, Error, LlvmVersion, LlvmVersionParseError,
    Version, VersionMeta,
};

#[test]
fn rustc_error() {
    let mut cmd = Command::new("rustc");
    cmd.arg("--FOO");
    let stderr = match VersionMeta::for_command(cmd) {
        Err(Error::CommandError { stdout: _, stderr }) => stderr,
        _ => panic!("command error expected"),
    };
    assert_eq!(stderr, "error: Unrecognized option: \'FOO\'\n\n");
}

#[test]
fn smoketest() {
    let v = version().unwrap();
    assert!(v.major >= 1);

    let v = version_meta().unwrap();
    assert!(v.semver.major >= 1);

    assert!(version().unwrap() >= Version::parse("1.0.0").unwrap());
}

#[test]
fn parse_1_0_0() {
    let version = version_meta_for(
        "rustc 1.0.0 (a59de37e9 2015-05-13) (built 2015-05-14)
binary: rustc
commit-hash: a59de37e99060162a2674e3ff45409ac73595c0e
commit-date: 2015-05-13
build-date: 2015-05-14
host: x86_64-unknown-linux-gnu
release: 1.0.0",
    )
    .unwrap();

    assert_eq!(version.semver, Version::parse("1.0.0").unwrap());
    assert_eq!(
        version.commit_hash,
        Some("a59de37e99060162a2674e3ff45409ac73595c0e".into())
    );
    assert_eq!(version.commit_date, Some("2015-05-13".into()));
    assert_eq!(version.build_date, Some("2015-05-14".into()));
    assert_eq!(version.channel, Channel::Stable);
    assert_eq!(version.host, "x86_64-unknown-linux-gnu");
    assert_eq!(
        version.short_version_string,
        "rustc 1.0.0 (a59de37e9 2015-05-13) (built 2015-05-14)"
    );
    assert_eq!(version.llvm_version, None);
}

#[test]
fn parse_unknown() {
    let version = version_meta_for(
        "rustc 1.3.0
binary: rustc
commit-hash: unknown
commit-date: unknown
host: x86_64-unknown-linux-gnu
release: 1.3.0",
    )
    .unwrap();

    assert_eq!(version.semver, Version::parse("1.3.0").unwrap());
    assert_eq!(version.commit_hash, None);
    assert_eq!(version.commit_date, None);
    assert_eq!(version.channel, Channel::Stable);
    assert_eq!(version.host, "x86_64-unknown-linux-gnu");
    assert_eq!(version.short_version_string, "rustc 1.3.0");
    assert_eq!(version.llvm_version, None);
}

#[test]
fn parse_nightly() {
    let version = version_meta_for(
        "rustc 1.5.0-nightly (65d5c0833 2015-09-29)
binary: rustc
commit-hash: 65d5c083377645a115c4ac23a620d3581b9562b6
commit-date: 2015-09-29
host: x86_64-unknown-linux-gnu
release: 1.5.0-nightly",
    )
    .unwrap();

    assert_eq!(version.semver, Version::parse("1.5.0-nightly").unwrap());
    assert_eq!(
        version.commit_hash,
        Some("65d5c083377645a115c4ac23a620d3581b9562b6".into())
    );
    assert_eq!(version.commit_date, Some("2015-09-29".into()));
    assert_eq!(version.channel, Channel::Nightly);
    assert_eq!(version.host, "x86_64-unknown-linux-gnu");
    assert_eq!(
        version.short_version_string,
        "rustc 1.5.0-nightly (65d5c0833 2015-09-29)"
    );
    assert_eq!(version.llvm_version, None);
}

#[test]
fn parse_stable() {
    let version = version_meta_for(
        "rustc 1.3.0 (9a92aaf19 2015-09-15)
binary: rustc
commit-hash: 9a92aaf19a64603b02b4130fe52958cc12488900
commit-date: 2015-09-15
host: x86_64-unknown-linux-gnu
release: 1.3.0",
    )
    .unwrap();

    assert_eq!(version.semver, Version::parse("1.3.0").unwrap());
    assert_eq!(
        version.commit_hash,
        Some("9a92aaf19a64603b02b4130fe52958cc12488900".into())
    );
    assert_eq!(version.commit_date, Some("2015-09-15".into()));
    assert_eq!(version.channel, Channel::Stable);
    assert_eq!(version.host, "x86_64-unknown-linux-gnu");
    assert_eq!(
        version.short_version_string,
        "rustc 1.3.0 (9a92aaf19 2015-09-15)"
    );
    assert_eq!(version.llvm_version, None);
}

#[test]
fn parse_1_16_0_nightly() {
    let version = version_meta_for(
        "rustc 1.16.0-nightly (5d994d8b7 2017-01-05)
binary: rustc
commit-hash: 5d994d8b7e482e87467d4a521911477bd8284ce3
commit-date: 2017-01-05
host: x86_64-unknown-linux-gnu
release: 1.16.0-nightly
LLVM version: 3.9",
    )
    .unwrap();

    assert_eq!(version.semver, Version::parse("1.16.0-nightly").unwrap());
    assert_eq!(
        version.commit_hash,
        Some("5d994d8b7e482e87467d4a521911477bd8284ce3".into())
    );
    assert_eq!(version.commit_date, Some("2017-01-05".into()));
    assert_eq!(version.channel, Channel::Nightly);
    assert_eq!(version.host, "x86_64-unknown-linux-gnu");
    assert_eq!(
        version.short_version_string,
        "rustc 1.16.0-nightly (5d994d8b7 2017-01-05)"
    );
    assert_eq!(
        version.llvm_version,
        Some(LlvmVersion { major: 3, minor: 9 })
    );
}

#[test]
fn parse_1_47_0_stable() {
    let version = version_meta_for(
        "rustc 1.47.0 (18bf6b4f0 2020-10-07)
binary: rustc
commit-hash: 18bf6b4f01a6feaf7259ba7cdae58031af1b7b39
commit-date: 2020-10-07
host: powerpc64le-unknown-linux-gnu
release: 1.47.0
LLVM version: 11.0",
    )
    .unwrap();

    assert_eq!(version.semver, Version::parse("1.47.0").unwrap());
    assert_eq!(
        version.commit_hash,
        Some("18bf6b4f01a6feaf7259ba7cdae58031af1b7b39".into())
    );
    assert_eq!(version.commit_date, Some("2020-10-07".into()));
    assert_eq!(version.channel, Channel::Stable);
    assert_eq!(version.host, "powerpc64le-unknown-linux-gnu");
    assert_eq!(
        version.short_version_string,
        "rustc 1.47.0 (18bf6b4f0 2020-10-07)"
    );
    assert_eq!(
        version.llvm_version,
        Some(LlvmVersion {
            major: 11,
            minor: 0,
        })
    );
}

#[test]
fn parse_llvm_micro() {
    let version = version_meta_for(
        "rustc 1.51.0-nightly (4253153db 2021-01-17)
binary: rustc
commit-hash: 4253153db205251f72ea4493687a31e04a2a8ca0
commit-date: 2021-01-17
host: x86_64-pc-windows-msvc
release: 1.51.0-nightly
LLVM version: 11.0.1",
    )
    .unwrap();

    assert_eq!(version.semver, Version::parse("1.51.0-nightly").unwrap());
    assert_eq!(
        version.commit_hash.unwrap(),
        "4253153db205251f72ea4493687a31e04a2a8ca0"
    );
    assert_eq!(version.commit_date.unwrap(), "2021-01-17");
    assert_eq!(version.host, "x86_64-pc-windows-msvc");
    assert_eq!(
        version.short_version_string,
        "rustc 1.51.0-nightly (4253153db 2021-01-17)"
    );
    assert_eq!(
        version.llvm_version,
        Some(LlvmVersion {
            major: 11,
            minor: 0
        })
    );
}

#[test]
fn parse_debian_buster() {
    let version = version_meta_for(
        "rustc 1.41.1
binary: rustc
commit-hash: unknown
commit-date: unknown
host: powerpc64le-unknown-linux-gnu
release: 1.41.1
LLVM version: 7.0",
    )
    .unwrap();

    assert_eq!(version.semver, Version::parse("1.41.1").unwrap());
    assert_eq!(version.commit_hash, None);
    assert_eq!(version.commit_date, None);
    assert_eq!(version.channel, Channel::Stable);
    assert_eq!(version.host, "powerpc64le-unknown-linux-gnu");
    assert_eq!(version.short_version_string, "rustc 1.41.1");
    assert_eq!(
        version.llvm_version,
        Some(LlvmVersion { major: 7, minor: 0 })
    );
}

#[test]
fn parse_termux() {
    let version = version_meta_for(
        "rustc 1.46.0
binary: rustc
commit-hash: unknown
commit-date: unknown
host: aarch64-linux-android
release: 1.46.0
LLVM version: 10.0",
    )
    .unwrap();

    assert_eq!(version.semver, Version::parse("1.46.0").unwrap());
    assert_eq!(version.commit_hash, None);
    assert_eq!(version.commit_date, None);
    assert_eq!(version.channel, Channel::Stable);
    assert_eq!(version.host, "aarch64-linux-android");
    assert_eq!(version.short_version_string, "rustc 1.46.0");
    assert_eq!(
        version.llvm_version,
        Some(LlvmVersion {
            major: 10,
            minor: 0,
        })
    );
}

#[test]
fn parse_llvm_version_empty() {
    let res: Result<LlvmVersion, _> = "".parse();
    assert!(match res {
        Err(LlvmVersionParseError::ParseIntError(_)) => true,
        _ => false,
    });
}

#[test]
fn parse_llvm_version_invalid_char() {
    let res: Result<LlvmVersion, _> = "A".parse();
    assert!(match res {
        Err(LlvmVersionParseError::ParseIntError(_)) => true,
        _ => false,
    });
}

#[test]
fn parse_llvm_version_overflow() {
    let res: Result<LlvmVersion, _> = "9999999999999999999999999999999".parse();
    assert!(match res {
        Err(LlvmVersionParseError::ParseIntError(_)) => true,
        _ => false,
    });
}

#[test]
fn parse_llvm_version_leading_zero_on_zero() {
    let res: Result<LlvmVersion, _> = "00".parse();
    assert!(match res {
        Err(LlvmVersionParseError::ComponentMustNotHaveLeadingZeros) => true,
        _ => false,
    });
}

#[test]
fn parse_llvm_version_leading_zero_on_nonzero() {
    let res: Result<LlvmVersion, _> = "01".parse();
    assert!(match res {
        Err(LlvmVersionParseError::ComponentMustNotHaveLeadingZeros) => true,
        _ => false,
    });
}

#[test]
fn parse_llvm_version_4_components() {
    let res: Result<LlvmVersion, _> = "4.0.0.0".parse();

    assert!(match res {
        Err(LlvmVersionParseError::TooManyComponents) => true,
        _ => false,
    });
}

#[test]
fn parse_llvm_version_component_sign_plus() {
    let res: Result<LlvmVersion, _> = "1.+3".parse();

    assert!(match res {
        Err(LlvmVersionParseError::ComponentMustNotHaveSign) => true,
        _ => false,
    });
}

#[test]
fn parse_llvm_version_component_sign_minus() {
    let res: Result<LlvmVersion, _> = "1.-3".parse();

    assert!(match res {
        Err(LlvmVersionParseError::ComponentMustNotHaveSign) => true,
        _ => false,
    });
}

#[test]
fn parse_llvm_version_3() {
    let res: Result<LlvmVersion, _> = "3".parse();

    assert!(match res {
        Err(LlvmVersionParseError::MinorVersionRequiredBefore4) => true,
        _ => false,
    });
}

#[test]
fn parse_llvm_version_5() {
    let v: LlvmVersion = "5".parse().unwrap();
    assert_eq!(v, LlvmVersion { major: 5, minor: 0 });
}

#[test]
fn parse_llvm_version_5_0() {
    let v: LlvmVersion = "5.0".parse().unwrap();
    assert_eq!(v, LlvmVersion { major: 5, minor: 0 });
}

#[test]
fn parse_llvm_version_4_0() {
    let v: LlvmVersion = "4.0".parse().unwrap();
    assert_eq!(v, LlvmVersion { major: 4, minor: 0 });
}

#[test]
fn parse_llvm_version_3_0() {
    let v: LlvmVersion = "3.0".parse().unwrap();
    assert_eq!(v, LlvmVersion { major: 3, minor: 0 });
}

#[test]
fn parse_llvm_version_3_9() {
    let v: LlvmVersion = "3.9".parse().unwrap();
    assert_eq!(v, LlvmVersion { major: 3, minor: 9 });
}

#[test]
fn parse_llvm_version_11_0() {
    let v: LlvmVersion = "11.0".parse().unwrap();
    assert_eq!(
        v,
        LlvmVersion {
            major: 11,
            minor: 0
        }
    );
}

#[test]
fn parse_llvm_version_11() {
    let v: LlvmVersion = "11".parse().unwrap();
    assert_eq!(
        v,
        LlvmVersion {
            major: 11,
            minor: 0
        }
    );
}

#[test]
fn test_llvm_version_comparison() {
    // check that field order is correct
    assert!(LlvmVersion { major: 3, minor: 9 } < LlvmVersion { major: 4, minor: 0 });
}

/*
#[test]
fn version_matches_replacement() {
    let f = |s1: &str, s2: &str| {
        let a = Version::parse(s1).unwrap();
        let b = Version::parse(s2).unwrap();
        println!("{} <= {} : {}", s1, s2, a <= b);
    };

    println!();

    f("1.5.0",         "1.5.0");
    f("1.5.0-nightly", "1.5.0");
    f("1.5.0",         "1.5.0-nightly");
    f("1.5.0-nightly", "1.5.0-nightly");

    f("1.5.0",         "1.6.0");
    f("1.5.0-nightly", "1.6.0");
    f("1.5.0",         "1.6.0-nightly");
    f("1.5.0-nightly", "1.6.0-nightly");

    panic!();

}
*/
