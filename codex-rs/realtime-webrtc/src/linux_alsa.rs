//! Locate system ALSA plugins for the statically linked Linux voice helper.
//!
//! Bazel's ALSA defaults to /usr/lib/alsa-lib. Distribution plugins may instead
//! live in a multiarch or lib64 directory. Only fixed system paths are admitted;
//! caller-provided plugin directories and loader search paths remain excluded.

use std::path::Path;

#[cfg(target_os = "linux")]
pub(crate) const PLUGIN_DIRECTORIES: &[&str] = &[
    #[cfg(target_arch = "x86_64")]
    "/usr/lib/x86_64-linux-gnu/alsa-lib",
    #[cfg(target_arch = "aarch64")]
    "/usr/lib/aarch64-linux-gnu/alsa-lib",
    "/usr/lib64/alsa-lib",
    "/usr/lib/alsa-lib",
];

pub(crate) fn plugin_directory<'a>(candidates: &[&'a str]) -> Option<&'a str> {
    candidates
        .iter()
        .copied()
        .find(|path| Path::new(path).is_dir())
}

#[cfg(test)]
#[path = "linux_alsa_tests.rs"]
mod tests;
