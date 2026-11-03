//! Launches the helper with its Bazel-provided native startup libraries available.

#[cfg(all(windows, target_env = "msvc"))]
use std::env;

#[cfg(all(windows, target_env = "msvc"))]
use anyhow::Context;
use anyhow::Result;
use codex_utils_cargo_bin::cargo_bin;
use tokio::process::Command;

pub(crate) fn voice_host_command() -> Result<Command> {
    let command = Command::new(cargo_bin("codex-voice-host")?);
    #[cfg(all(windows, target_env = "msvc"))]
    let mut command = command;
    #[cfg(all(windows, target_env = "msvc"))]
    if codex_utils_cargo_bin::runfiles_available() {
        let resource = format!(
            "../../third_party/voice/native_link_windows_{}/runtime.json",
            env::consts::ARCH
        );
        let runtime = codex_utils_cargo_bin::find_resource!(resource)?;
        let runtime_bin = runtime.parent().context("voice runtime root")?.join("bin");
        let path = env::join_paths(
            std::iter::once(runtime_bin)
                .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
        )?;
        command.env("PATH", path);
    }
    Ok(command)
}
