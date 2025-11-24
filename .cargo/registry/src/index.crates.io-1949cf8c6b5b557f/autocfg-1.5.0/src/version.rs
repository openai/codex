use std::process::Command;
use std::str;

use super::{error, Error};

/// A version structure for making relative comparisons.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: usize,
    minor: usize,
    patch: usize,
}

impl Version {
    /// Creates a `Version` instance for a specific `major.minor.patch` version.
    pub fn new(major: usize, minor: usize, patch: usize) -> Self {
        Version {
            major: major,
            minor: minor,
            patch: patch,
        }
    }

    pub fn from_command(command: &mut Command) -> Result<Self, Error> {
        // Get rustc's verbose version
        let output = try!(command
            .args(&["--version", "--verbose"])
            .output()
            .map_err(error::from_io));
        if !output.status.success() {
            return Err(error::from_str("could not execute rustc"));
        }
        let output = try!(str::from_utf8(&output.stdout).map_err(error::from_utf8));

        // Find the release line in the verbose version output.
        let release = match output.lines().find(|line| line.starts_with("release: ")) {
            Some(line) => &line["release: ".len()..],
            None => return Err(error::from_str("could not find rustc release")),
        };

        // Strip off any extra channel info, e.g. "-beta.N", "-nightly"
        let version = match release.find('-') {
            Some(i) => &release[..i],
            None => release,
        };

        // Split the version into semver components.
        let mut iter = version.splitn(3, '.');
        let major = try!(iter
            .next()
            .ok_or_else(|| error::from_str("missing major version")));
        let minor = try!(iter
            .next()
            .ok_or_else(|| error::from_str("missing minor version")));
        let patch = try!(iter
            .next()
            .ok_or_else(|| error::from_str("missing patch version")));

        Ok(Version::new(
            try!(major.parse().map_err(error::from_num)),
            try!(minor.parse().map_err(error::from_num)),
            try!(patch.parse().map_err(error::from_num)),
        ))
    }
}
