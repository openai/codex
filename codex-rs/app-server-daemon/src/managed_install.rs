use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use tokio::process::Command;

pub(crate) fn managed_codex_bin(codex_home: &Path) -> PathBuf {
    codex_home
        .join("packages")
        .join("standalone")
        .join("current")
        .join(managed_codex_file_name())
}

pub(crate) async fn managed_codex_version(codex_bin: &Path) -> Result<String> {
    let output = Command::new(codex_bin)
        .arg("--version")
        .output()
        .await
        .with_context(|| {
            format!(
                "failed to invoke managed Codex binary {}",
                codex_bin.display()
            )
        })?;
    if !output.status.success() {
        return Err(anyhow!(
            "managed Codex binary {} exited with status {}",
            codex_bin.display(),
            output.status
        ));
    }

    let stdout = String::from_utf8(output.stdout).with_context(|| {
        format!(
            "managed Codex version was not utf-8: {}",
            codex_bin.display()
        )
    })?;
    parse_codex_version(&stdout)
}

fn managed_codex_file_name() -> &'static str {
    if cfg!(windows) { "codex.exe" } else { "codex" }
}

fn parse_codex_version(output: &str) -> Result<String> {
    let version = output
        .split_whitespace()
        .nth(1)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| anyhow!("managed Codex version output was malformed"))?;
    Ok(version.to_string())
}

#[cfg(test)]
#[path = "managed_install_tests.rs"]
mod tests;
