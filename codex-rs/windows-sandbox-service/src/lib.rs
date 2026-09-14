use anyhow::Result;

#[cfg(windows)]
mod installation_record;
#[cfg(windows)]
mod ipc;

#[cfg(windows)]
mod machine_policy;
#[cfg(windows)]
mod package_identity;
#[cfg(windows)]
mod package_lifecycle;
#[cfg(windows)]
mod provisioning;
#[cfg(windows)]
mod registered_runtime;
#[cfg(windows)]
mod service;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunMode {
    Service,
    #[cfg(debug_assertions)]
    Foreground,
}

pub fn run(mode: RunMode) -> Result<()> {
    #[cfg(windows)]
    {
        match mode {
            RunMode::Service => service::run(),
            #[cfg(debug_assertions)]
            RunMode::Foreground => service::run_foreground(),
        }
    }

    #[cfg(not(windows))]
    {
        let _ = mode;
        anyhow::bail!("the Codex sandbox service is only available on Windows")
    }
}
