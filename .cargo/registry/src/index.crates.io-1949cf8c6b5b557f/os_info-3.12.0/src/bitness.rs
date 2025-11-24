// spell-checker:ignore getconf

use std::fmt::{self, Display, Formatter};
#[cfg(any(
    target_os = "aix",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "cygwin"
))]
use std::process::{Command, Output};

/// Operating system architecture in terms of how many bits compose the basic values it can deal with.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Bitness {
    /// Unknown bitness (unable to determine).
    Unknown,
    /// 32-bit.
    X32,
    /// 64-bit.
    X64,
}

impl Display for Bitness {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Bitness::Unknown => write!(f, "unknown bitness"),
            Bitness::X32 => write!(f, "32-bit"),
            Bitness::X64 => write!(f, "64-bit"),
        }
    }
}

#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "linux",
    target_os = "cygwin",
    target_os = "macos",
))]
pub fn get() -> Bitness {
    match &Command::new("getconf").arg("LONG_BIT").output() {
        Ok(Output { stdout, .. }) if stdout == b"32\n" => Bitness::X32,
        Ok(Output { stdout, .. }) if stdout == b"64\n" => Bitness::X64,
        _ => Bitness::Unknown,
    }
}

#[cfg(target_os = "netbsd")]
pub fn get() -> Bitness {
    match &Command::new("sysctl")
        .arg("-n")
        .arg("hw.machine_arch")
        .output()
    {
        Ok(Output { stdout, .. }) if stdout == b"amd64\n" => Bitness::X64,
        Ok(Output { stdout, .. }) if stdout == b"x86_64\n" => Bitness::X64,
        Ok(Output { stdout, .. }) if stdout == b"i386\n" => Bitness::X32,
        Ok(Output { stdout, .. }) if stdout == b"aarch64\n" => Bitness::X64,
        Ok(Output { stdout, .. }) if stdout == b"earmv7hf\n" => Bitness::X32,
        Ok(Output { stdout, .. }) if stdout == b"sparc64\n" => Bitness::X64,
        _ => Bitness::Unknown,
    }
}

#[cfg(target_os = "openbsd")]
pub fn get() -> Bitness {
    match &Command::new("sysctl").arg("-n").arg("hw.machine").output() {
        Ok(Output { stdout, .. }) if stdout == b"amd64\n" => Bitness::X64,
        Ok(Output { stdout, .. }) if stdout == b"x86_64\n" => Bitness::X64,
        Ok(Output { stdout, .. }) if stdout == b"i386\n" => Bitness::X32,
        Ok(Output { stdout, .. }) if stdout == b"aarch64\n" => Bitness::X64,
        Ok(Output { stdout, .. }) if stdout == b"earmv7hf\n" => Bitness::X32,
        Ok(Output { stdout, .. }) if stdout == b"sparc64\n" => Bitness::X64,
        _ => Bitness::Unknown,
    }
}

#[cfg(target_os = "illumos")]
pub fn get() -> Bitness {
    match &Command::new("isainfo").arg("-b").output() {
        Ok(Output { stdout, .. }) if stdout == b"64\n" => Bitness::X64,
        Ok(Output { stdout, .. }) if stdout == b"32\n" => Bitness::X32,
        _ => Bitness::Unknown,
    }
}

#[cfg(target_os = "aix")]
pub fn get() -> Bitness {
    match &Command::new("prtconf").arg("-c").output() {
        Ok(Output { stdout, .. }) if stdout == b"CPU Type: 64-bit\n" => Bitness::X64,
        Ok(Output { stdout, .. }) if stdout == b"CPU Type: 32-bit\n" => Bitness::X32,
        _ => Bitness::Unknown,
    }
}

#[cfg(all(
    test,
    any(
        target_os = "aix",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin"
    )
))]
mod tests {
    use super::*;
    use pretty_assertions::assert_ne;

    #[test]
    fn get_bitness() {
        let b = get();
        assert_ne!(b, Bitness::Unknown);
    }

    #[test]
    fn display() {
        let data = [
            (Bitness::Unknown, "unknown bitness"),
            (Bitness::X32, "32-bit"),
            (Bitness::X64, "64-bit"),
        ];

        for (bitness, expected) in &data {
            assert_eq!(&bitness.to_string(), expected);
        }
    }
}
