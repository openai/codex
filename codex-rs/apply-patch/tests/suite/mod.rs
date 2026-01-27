use anyhow::Context;
use std::path::PathBuf;
use std::sync::OnceLock;

mod cli;
mod scenarios;
#[cfg(not(target_os = "windows"))]
mod tool;

static APPLY_PATCH_BIN: OnceLock<Result<PathBuf, String>> = OnceLock::new();

fn apply_patch_bin() -> anyhow::Result<&'static PathBuf> {
    match APPLY_PATCH_BIN.get_or_init(|| {
        codex_utils_cargo_bin::cargo_bin("apply_patch")
            .map_err(|err| anyhow::anyhow!(err))
            .context("resolve apply_patch bin")
            .map_err(|err| err.to_string())
    }) {
        Ok(path) => Ok(path),
        Err(message) => Err(anyhow::anyhow!("{message}")),
    }
}
