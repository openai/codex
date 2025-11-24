#![cfg(test_node_semver)]

use semver::Version;
use std::fmt::{self, Display};
use std::process::Command;

#[derive(Default, Eq, PartialEq, Hash, Debug)]
pub(super) struct VersionReq(semver::VersionReq);

impl VersionReq {
    pub(super) const STAR: Self = VersionReq(semver::VersionReq::STAR);

    pub(super) fn matches(&self, version: &Version) -> bool {
        let out = Command::new("node")
            .arg("-e")
            .arg(format!(
                "console.log(require('semver').satisfies('{}', '{}'))",
                version,
                self.to_string().replace(',', ""),
            ))
            .output()
            .unwrap();
        if out.stdout == b"true\n" {
            true
        } else if out.stdout == b"false\n" {
            false
        } else {
            let s = String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr);
            panic!("unexpected output: {}", s);
        }
    }
}

impl Display for VersionReq {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

#[track_caller]
pub(super) fn req(text: &str) -> VersionReq {
    VersionReq(crate::util::req(text))
}
