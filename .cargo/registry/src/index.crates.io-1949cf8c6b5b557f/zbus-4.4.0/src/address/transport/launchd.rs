use super::{Transport, Unix, UnixSocket};
use crate::{process::run, Result};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
/// The transport properties of a launchd D-Bus address.
pub struct Launchd {
    pub(super) env: String,
}

impl Launchd {
    /// Create a new launchd D-Bus address.
    pub fn new(env: &str) -> Self {
        Self {
            env: env.to_string(),
        }
    }

    /// The path of the unix domain socket for the launchd created dbus-daemon.
    pub fn env(&self) -> &str {
        &self.env
    }

    /// Determine the actual transport details behin a launchd address.
    pub(super) async fn bus_address(&self) -> Result<Transport> {
        let output = run("launchctl", ["getenv", self.env()])
            .await
            .expect("failed to wait on launchctl output");

        if !output.status.success() {
            return Err(crate::Error::Address(format!(
                "launchctl terminated with code: {}",
                output.status
            )));
        }

        let addr = String::from_utf8(output.stdout).map_err(|e| {
            crate::Error::Address(format!("Unable to parse launchctl output as UTF-8: {}", e))
        })?;

        Ok(Transport::Unix(Unix::new(UnixSocket::File(
            addr.trim().into(),
        ))))
    }

    pub(super) fn from_options(opts: HashMap<&str, &str>) -> Result<Self> {
        opts.get("env")
            .ok_or_else(|| crate::Error::Address("missing env key".into()))
            .map(|env| Self {
                env: env.to_string(),
            })
    }
}

impl std::fmt::Display for Launchd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "launchd:env={}", self.env)
    }
}
