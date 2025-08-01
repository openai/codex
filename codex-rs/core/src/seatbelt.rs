use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

use crate::protocol::SandboxPolicy;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;

const MACOS_SEATBELT_BASE_POLICY: &str = include_str!("seatbelt_base_policy.sbpl");

/// When working with `sandbox-exec`, only consider `sandbox-exec` in `/usr/bin`
/// to defend against an attacker trying to inject a malicious version on the
/// PATH. If /usr/bin/sandbox-exec has been tampered with, then the attacker
/// already has root access.
const MACOS_PATH_TO_SEATBELT_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

pub async fn spawn_command_under_seatbelt(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: PathBuf,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> std::io::Result<Child> {
    let args = create_seatbelt_command_args(command, sandbox_policy, &cwd);
    let arg0 = None;
    spawn_child_async(
        PathBuf::from(MACOS_PATH_TO_SEATBELT_EXECUTABLE),
        args,
        arg0,
        cwd,
        sandbox_policy,
        stdio_policy,
        env,
    )
    .await
}

fn create_seatbelt_command_args(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> Vec<String> {
    let (file_write_policy, extra_cli_args) = {
        if sandbox_policy.has_full_disk_write_access() {
            // Allegedly, this is more permissive than `(allow file-write*)`.
            (
                r#"(allow file-write* (regex #"^/"))"#.to_string(),
                Vec::<String>::new(),
            )
        } else {
            let writable_roots = sandbox_policy.get_writable_roots_with_cwd(cwd);
            let (writable_folder_policies, cli_args): (Vec<String>, Vec<String>) = writable_roots
                .iter()
                .enumerate()
                .map(|(index, root)| {
                    let param_name = format!("WRITABLE_ROOT_{index}");
                    let policy: String = format!("(subpath (param \"{param_name}\"))");
                    let cli_arg = format!("-D{param_name}={}", root.to_string_lossy());
                    (policy, cli_arg)
                })
                .unzip();
            if writable_folder_policies.is_empty() {
                ("".to_string(), Vec::<String>::new())
            } else {
                let file_write_policy = format!(
                    "(allow file-write*\n{}\n)",
                    writable_folder_policies.join(" ")
                );
                (file_write_policy, cli_args)
            }
        }
    };

    let file_read_policy = if sandbox_policy.has_full_disk_read_access() {
        "; allow read-only file operations\n(allow file-read*)"
    } else {
        ""
    };

    // TODO(mbolin): apply_patch calls must also honor the SandboxPolicy.
    let network_policy = if sandbox_policy.has_full_network_access() {
        "(allow network-outbound)\n(allow network-inbound)\n(allow system-socket)"
    } else {
        ""
    };

    let full_policy = format!(
        "{MACOS_SEATBELT_BASE_POLICY}\n{file_read_policy}\n{file_write_policy}\n{network_policy}"
    );
    let mut seatbelt_args: Vec<String> = vec!["-p".to_string(), full_policy];
    seatbelt_args.extend(extra_cli_args);
    seatbelt_args.push("--".to_string());
    seatbelt_args.extend(command);
    seatbelt_args
}
