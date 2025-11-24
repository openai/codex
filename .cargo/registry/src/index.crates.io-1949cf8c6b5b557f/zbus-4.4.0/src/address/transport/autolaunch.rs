use crate::{Error, Result};
use std::collections::HashMap;

/// Transport properties of an autolaunch D-Bus address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Autolaunch {
    pub(super) scope: Option<AutolaunchScope>,
}

impl std::fmt::Display for Autolaunch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "autolaunch:")?;
        if let Some(scope) = &self.scope {
            write!(f, "scope={}", scope)?;
        }

        Ok(())
    }
}

impl Default for Autolaunch {
    fn default() -> Self {
        Self::new()
    }
}

impl Autolaunch {
    /// Create a new autolaunch transport.
    pub fn new() -> Self {
        Self { scope: None }
    }

    /// Set the `autolaunch:` address `scope` value.
    pub fn set_scope(mut self, scope: Option<AutolaunchScope>) -> Self {
        self.scope = scope;

        self
    }

    /// The optional scope.
    pub fn scope(&self) -> Option<&AutolaunchScope> {
        self.scope.as_ref()
    }

    pub(super) fn from_options(opts: HashMap<&str, &str>) -> Result<Self> {
        opts.get("scope")
            .map(|scope| -> Result<_> {
                let decoded = super::decode_percents(scope)?;
                match decoded.as_slice() {
                    b"install-path" => Ok(AutolaunchScope::InstallPath),
                    b"user" => Ok(AutolaunchScope::User),
                    _ => String::from_utf8(decoded)
                        .map(AutolaunchScope::Other)
                        .map_err(|_| {
                            Error::Address("autolaunch scope is not valid UTF-8".to_owned())
                        }),
                }
            })
            .transpose()
            .map(|scope| Self { scope })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AutolaunchScope {
    /// Limit session bus to dbus installation path.
    InstallPath,
    /// Limit session bus to the recent user.
    User,
    /// other values - specify dedicated session bus like "release", "debug" or other.
    Other(String),
}

impl std::fmt::Display for AutolaunchScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InstallPath => write!(f, "*install-path"),
            Self::User => write!(f, "*user"),
            Self::Other(o) => write!(f, "{o}"),
        }
    }
}
