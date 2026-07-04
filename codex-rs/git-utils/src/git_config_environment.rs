use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::process::Command;

const FIXED_CONFIG_ENVIRONMENT_KEYS: &[&str] = &[
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_NOSYSTEM",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "HOME",
    "XDG_CONFIG_HOME",
    #[cfg(windows)]
    "APPDATA",
    #[cfg(windows)]
    "PROGRAMDATA",
    #[cfg(windows)]
    "USERPROFILE",
    #[cfg(windows)]
    "HOMEDRIVE",
    #[cfg(windows)]
    "HOMEPATH",
];

// Git config command entries are an untrusted process input. Keep capture
// bounded rather than allocating attacker-selected amounts of memory before
// Git gets a chance to reject an unreasonable count.
const MAX_CONFIG_ENVIRONMENT_ENTRIES: usize = 1024;

/// Exact config-relevant process environment bound to one
/// [`GitRunner`](crate::git_command::GitRunner).
///
/// Every ordinary Git child receives these captured values (including explicit
/// removals for variables that were absent). Commands that intentionally
/// isolate a config source may override them after command construction.
pub(crate) struct GitConfigEnvironmentSnapshot {
    entries: Box<[(OsString, Option<OsString>)]>,
}

impl std::fmt::Debug for GitConfigEnvironmentSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GitConfigEnvironmentSnapshot")
            .field("captured_entries", &self.entries.len())
            .finish()
    }
}

impl GitConfigEnvironmentSnapshot {
    pub(crate) fn capture() -> io::Result<Self> {
        let environment = std::env::vars_os().collect::<BTreeMap<_, _>>();
        Self::capture_from(|name| captured_environment_value(&environment, name))
    }

    fn capture_from(mut value_for: impl FnMut(&OsStr) -> Option<OsString>) -> io::Result<Self> {
        let mut entries = FIXED_CONFIG_ENVIRONMENT_KEYS
            .iter()
            .map(|name| (OsString::from(name), value_for(OsStr::new(name))))
            .collect::<Vec<_>>();

        if let Some(raw_count) = value_for(OsStr::new("GIT_CONFIG_COUNT")) {
            let count = raw_count
                .to_str()
                .ok_or_else(|| invalid_environment("non-UTF-8 GIT_CONFIG_COUNT"))?
                .parse::<usize>()
                .map_err(|_| invalid_environment("invalid GIT_CONFIG_COUNT"))?;
            if count > MAX_CONFIG_ENVIRONMENT_ENTRIES {
                return Err(invalid_environment(
                    "GIT_CONFIG_COUNT exceeds the supported safety bound",
                ));
            }
            for index in 0..count {
                for prefix in ["GIT_CONFIG_KEY_", "GIT_CONFIG_VALUE_"] {
                    let name = OsString::from(format!("{prefix}{index}"));
                    let value = value_for(&name);
                    entries.push((name, value));
                }
            }
        }

        Ok(Self {
            entries: entries.into_boxed_slice(),
        })
    }

    pub(crate) fn apply_to(&self, command: &mut Command) {
        for (name, value) in &self.entries {
            match value {
                Some(value) => {
                    command.env(name, value);
                }
                None => {
                    command.env_remove(name);
                }
            }
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn value(&self, name: &str) -> Option<&OsStr> {
        self.entries
            .iter()
            .find_map(|(candidate, value)| (candidate == name).then_some(value.as_deref()))
            .flatten()
    }
}

#[cfg(not(windows))]
fn captured_environment_value(
    environment: &BTreeMap<OsString, OsString>,
    name: &OsStr,
) -> Option<OsString> {
    environment.get(name).cloned()
}

#[cfg(windows)]
fn captured_environment_value(
    environment: &BTreeMap<OsString, OsString>,
    name: &OsStr,
) -> Option<OsString> {
    environment.iter().find_map(|(candidate, value)| {
        candidate
            .to_string_lossy()
            .eq_ignore_ascii_case(&name.to_string_lossy())
            .then(|| value.clone())
    })
}

fn invalid_environment(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "git_config_environment_tests.rs"]
mod tests;
