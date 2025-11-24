#![allow(
    clippy::derive_partial_eq_without_eq,
    clippy::enum_glob_use,
    clippy::must_use_candidate
)]

include!("../build/rustc.rs");

#[test]
fn test_parse() {
    let cases = &[
        (
            "rustc 1.0.0 (a59de37e9 2015-05-13) (built 2015-05-14)",
            Version {
                minor: 0,
                patch: 0,
                channel: Stable,
            },
        ),
        (
            "rustc 1.18.0",
            Version {
                minor: 18,
                patch: 0,
                channel: Stable,
            },
        ),
        (
            "rustc 1.24.1 (d3ae9a9e0 2018-02-27)",
            Version {
                minor: 24,
                patch: 1,
                channel: Stable,
            },
        ),
        (
            "rustc 1.35.0-beta.3 (c13114dc8 2019-04-27)",
            Version {
                minor: 35,
                patch: 0,
                channel: Beta,
            },
        ),
        (
            "rustc 1.36.0-nightly (938d4ffe1 2019-04-27)",
            Version {
                minor: 36,
                patch: 0,
                channel: Nightly(Date {
                    year: 2019,
                    month: 4,
                    day: 27,
                }),
            },
        ),
        (
            "rustc 1.36.0-dev",
            Version {
                minor: 36,
                patch: 0,
                channel: Dev,
            },
        ),
        (
            "rustc 1.36.0-nightly",
            Version {
                minor: 36,
                patch: 0,
                channel: Dev,
            },
        ),
        (
            "warning: invalid logging spec 'warning', ignoring it
             rustc 1.30.0-nightly (3bc2ca7e4 2018-09-20)",
            Version {
                minor: 30,
                patch: 0,
                channel: Nightly(Date {
                    year: 2018,
                    month: 9,
                    day: 20,
                }),
            },
        ),
        (
            "rustc 1.52.1-nightly (gentoo)",
            Version {
                minor: 52,
                patch: 1,
                channel: Dev,
            },
        ),
    ];

    for (string, expected) in cases {
        match parse(string) {
            ParseResult::Success(version) => assert_eq!(version, *expected),
            ParseResult::OopsClippy | ParseResult::OopsMirai | ParseResult::Unrecognized => {
                panic!("unrecognized: {:?}", string);
            }
        }
    }
}
