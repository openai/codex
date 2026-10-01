//! Exercise native initialization with real prepared libraries in a relocated package.

use std::fs;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_install_context::InstallContext;
use codex_realtime_webrtc::VoiceHost;
use codex_utils_cargo_bin::cargo_bin;
use tokio::process::Command;
use tokio::time::timeout;

const DEADLINE: Duration = Duration::from_secs(/*secs*/ 10);

#[tokio::test]
#[ignore = "requires CODEX_TEST_VOICE_RUNTIME containing real prepared native libraries"]
async fn relocated_runtime_initializes_closes_and_rejects_duplicate_initialization() -> Result<()> {
    let source = std::env::var_os("CODEX_TEST_VOICE_RUNTIME")
        .context("set CODEX_TEST_VOICE_RUNTIME to a matching prepared runtime")?;
    let source = fs::canonicalize(source)?;
    ensure!(
        source.join("runtime.json").is_file(),
        "prepared runtime receipt missing"
    );
    let directory = tempfile::Builder::new()
        .prefix("voice native package ")
        .tempdir()?;
    let root = directory.path().join("staging");
    let runtime = root.join("codex-resources/voice");
    fs::create_dir_all(&runtime)?;
    let mut pending = vec![(source, runtime.clone())];
    while let Some((source, destination)) = pending.pop() {
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let target = destination.join(entry.file_name());
            let kind = entry.file_type()?;
            if kind.is_dir() {
                fs::create_dir(&target)?;
                pending.push((entry.path(), target));
            } else {
                ensure!(kind.is_file(), "runtime inputs must be physical files");
                fs::copy(entry.path(), target)?;
            }
        }
    }
    let helper_source = cargo_bin("codex-voice-host")?;
    let name = helper_source.file_name().context("helper filename")?;
    fs::create_dir_all(runtime.join("bin"))?;
    codex_utils_cargo_bin::copy_executable(&helper_source, &runtime.join("bin").join(name))?;
    fs::create_dir(root.join("bin"))?;
    let app_name = if cfg!(windows) { "codex.exe" } else { "codex" };
    fs::write(root.join("bin").join(app_name), [])?;
    fs::write(root.join("codex-package.json"), "{}")?;
    let moved = directory.path().join("relocated package");
    fs::rename(root, &moved)?;
    let package = InstallContext::from_exe(
        /*is_macos*/ cfg!(target_os = "macos"),
        Some(&moved.join("bin").join(app_name)),
        /*method_override*/ None,
    )
    .package_layout
    .context("package layout")?;
    let output = timeout(
        DEADLINE,
        Command::new(&helper_source)
            .arg("--build-commit")
            .kill_on_drop(true)
            .output(),
    )
    .await??;
    ensure!(output.status.success(), "helper build identity failed");
    let build_commit = String::from_utf8(output.stdout)?.trim().to_owned();
    VoiceHost::connect(&package, &build_commit)
        .await?
        .initialize_runtime()
        .await?
        .close()
        .await?;

    let host = VoiceHost::connect(&package, &build_commit)
        .await?
        .initialize_runtime()
        .await?;
    assert!(host.initialize_runtime().await.is_err());
    Ok(())
}
