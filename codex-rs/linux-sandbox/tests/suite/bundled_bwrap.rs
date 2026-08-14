#![cfg(target_os = "linux")]

use codex_linux_sandbox::BUNDLED_BWRAP_DIGEST_VERIFICATION_FAILURE_EXIT_CODE;
use codex_protocol::models::PermissionProfile;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn bazel_build_rejects_tampered_bundled_bwrap() {
    // Cargo embeds the bundled bwrap digest only in release builds.
    if option_env!("BAZEL_PACKAGE").is_none() {
        return;
    }

    let bwrap_runfile =
        std::env::var("CARGO_BIN_EXE_bwrap").expect("Bazel should provide the bwrap runfile");
    let runfiles_dir =
        std::env::var_os("TEST_SRCDIR").expect("Bazel should provide its runfiles directory");
    let bwrap_binary = PathBuf::from(runfiles_dir).join(bwrap_runfile);

    let original_bwrap_bytes =
        std::fs::read(&bwrap_binary).expect("built bwrap should be readable");

    let package = tempfile::tempdir().expect("package directory should be created");
    let resources = package.path().join("codex-resources");
    std::fs::create_dir(&resources).expect("package resource directory should be created");

    let sandbox_binary = package.path().join("codex-linux-sandbox");
    std::fs::copy(env!("CARGO_BIN_EXE_codex-linux-sandbox"), &sandbox_binary)
        .expect("sandbox binary should be copied into the package");

    let bundled_bwrap = resources.join("bwrap");
    std::fs::copy(&bwrap_binary, &bundled_bwrap)
        .expect("built bwrap should be copied into the package");

    let permission_profile = serde_json::to_string(&PermissionProfile::read_only())
        .expect("read-only permission profile should serialize");
    let run_sandbox = || {
        Command::new(&sandbox_binary)
            .arg("--sandbox-policy-cwd")
            .arg(package.path())
            .arg("--permission-profile")
            .arg(&permission_profile)
            .arg("--no-proc")
            .arg("--")
            .arg("/bin/true")
            .env_clear()
            .env("PATH", package.path())
            .output()
            .expect("sandbox binary should start")
    };

    let original_output = run_sandbox();
    assert_ne!(
        original_output.status.code(),
        Some(BUNDLED_BWRAP_DIGEST_VERIFICATION_FAILURE_EXIT_CODE),
        "sandbox should accept the digest of the unmodified bundled bwrap"
    );

    std::fs::remove_file(&bundled_bwrap).expect("read-only bundled bwrap should be replaceable");
    let mut tampered_bwrap_bytes = original_bwrap_bytes;
    tampered_bwrap_bytes.push(0);
    std::fs::write(&bundled_bwrap, &tampered_bwrap_bytes)
        .expect("modified bwrap should be copied into the package");
    let bwrap_permissions = std::fs::metadata(&bwrap_binary)
        .expect("built bwrap metadata should be readable")
        .permissions();
    std::fs::set_permissions(&bundled_bwrap, bwrap_permissions)
        .expect("bundled bwrap should remain executable");

    let tampered_output = run_sandbox();
    assert_eq!(
        tampered_output.status.code(),
        Some(BUNDLED_BWRAP_DIGEST_VERIFICATION_FAILURE_EXIT_CODE),
        "sandbox should reject the tampered bundled bwrap"
    );
}
