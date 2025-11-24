use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use super::error::Error;
use super::version::Version;

#[derive(Clone, Debug)]
pub struct Rustc {
    rustc: PathBuf,
    rustc_wrapper: Option<PathBuf>,
    rustc_workspace_wrapper: Option<PathBuf>,
}

impl Rustc {
    pub fn new() -> Self {
        Rustc {
            rustc: env::var_os("RUSTC")
                .unwrap_or_else(|| "rustc".into())
                .into(),
            rustc_wrapper: get_rustc_wrapper(false),
            rustc_workspace_wrapper: get_rustc_wrapper(true),
        }
    }

    /// Build the command with possible wrappers.
    pub fn command(&self) -> Command {
        let mut rustc = self
            .rustc_wrapper
            .iter()
            .chain(self.rustc_workspace_wrapper.iter())
            .chain(Some(&self.rustc));
        let mut command = Command::new(rustc.next().unwrap());
        for arg in rustc {
            command.arg(arg);
        }
        command
    }

    /// Try to get the `rustc` version.
    pub fn version(&self) -> Result<Version, Error> {
        // Some wrappers like clippy-driver don't pass through version commands,
        // so we try to fall back to combinations without each wrapper.
        macro_rules! try_version {
            ($command:expr) => {
                if let Ok(value) = Version::from_command($command) {
                    return Ok(value);
                }
            };
        }

        let rustc = &self.rustc;
        if let Some(ref rw) = self.rustc_wrapper {
            if let Some(ref rww) = self.rustc_workspace_wrapper {
                try_version!(Command::new(rw).args(&[rww, rustc]));
            }
            try_version!(Command::new(rw).arg(rustc));
        }
        if let Some(ref rww) = self.rustc_workspace_wrapper {
            try_version!(Command::new(rww).arg(rustc));
        }
        Version::from_command(&mut Command::new(rustc))
    }
}

fn get_rustc_wrapper(workspace: bool) -> Option<PathBuf> {
    // We didn't really know whether the workspace wrapper is applicable until Cargo started
    // deliberately setting or unsetting it in rust-lang/cargo#9601. We'll use the encoded
    // rustflags as a proxy for that change for now, but we could instead check version 1.55.
    if workspace && env::var_os("CARGO_ENCODED_RUSTFLAGS").is_none() {
        return None;
    }

    let name = if workspace {
        "RUSTC_WORKSPACE_WRAPPER"
    } else {
        "RUSTC_WRAPPER"
    };

    if let Some(wrapper) = env::var_os(name) {
        // NB: `OsStr` didn't get `len` or `is_empty` until 1.9.
        if wrapper != OsString::new() {
            return Some(wrapper.into());
        }
    }

    None
}
