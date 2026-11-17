//! Masking system drop-ins must preserve host-key checks in the main SSH configuration.

use super::codex_linux_sandbox_exe;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use std::process::Command;

#[test]
fn ssh_masks_system_dropins_and_rejects_a_changed_host_key() {
    // The fixture stages root-owned files, then runs the client as UID 1000.
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("skipping SSH handshake: root is required to create the two-owner fixture");
        return;
    }
    let profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::read_only(),
        NetworkSandboxPolicy::Enabled,
    );
    let fixture = codex_utils_cargo_bin::find_resource!("tests/fixtures/ssh_handshake.sh").unwrap();
    let output = Command::new("unshare")
        .args(["--mount-proc", "--net", "--pid", "--fork"])
        .args(["--propagation", "private", "--", "/bin/sh"])
        .arg(fixture)
        .arg(codex_linux_sandbox_exe())
        .arg(serde_json::to_string(&profile).unwrap())
        .output();
    let output = match output {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("skipping SSH handshake: unshare is unavailable");
            return;
        }
        result => result.unwrap(),
    };
    if output.status.code() == Some(77)
        || (output.stderr.starts_with(b"unshare:")
            && String::from_utf8_lossy(&output.stderr).contains("Operation not permitted"))
    {
        eprintln!(
            "skipping SSH handshake: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    assert!(output.status.success(), "{output:?}");
}
