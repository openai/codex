use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

use crate::protocol::SandboxPolicy;
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
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
    mut env: HashMap<String, String>,
) -> std::io::Result<Child> {
    let args = create_seatbelt_command_args(command, sandbox_policy, &cwd);
    let arg0 = None;
    env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
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

            let mut writable_folder_policies: Vec<String> = Vec::new();
            let mut cli_args: Vec<String> = Vec::new();

            for (index, wr) in writable_roots.iter().enumerate() {
                // Canonicalize to avoid mismatches like /var vs /private/var on macOS.
                let canonical_root = wr.root.canonicalize().unwrap_or_else(|_| wr.root.clone());
                let root_param = format!("WRITABLE_ROOT_{index}");
                cli_args.push(format!(
                    "-D{root_param}={}",
                    canonical_root.to_string_lossy()
                ));

                if wr.read_only_subpaths.is_empty() {
                    writable_folder_policies.push(format!("(subpath (param \"{root_param}\"))"));
                } else {
                    // Add parameters for each read-only subpath and generate
                    // the `(require-not ...)` clauses.
                    let mut require_parts: Vec<String> = Vec::new();
                    require_parts.push(format!("(subpath (param \"{root_param}\"))"));
                    for (subpath_index, ro) in wr.read_only_subpaths.iter().enumerate() {
                        let canonical_ro = ro.canonicalize().unwrap_or_else(|_| ro.clone());
                        let ro_param = format!("WRITABLE_ROOT_{index}_RO_{subpath_index}");
                        cli_args.push(format!("-D{ro_param}={}", canonical_ro.to_string_lossy()));
                        require_parts
                            .push(format!("(require-not (subpath (param \"{ro_param}\")))"));
                    }
                    let policy_component = format!("(require-all {} )", require_parts.join(" "));
                    writable_folder_policies.push(policy_component);
                }
            }

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

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used)]
    use super::MACOS_SEATBELT_BASE_POLICY;
    use super::create_seatbelt_command_args;
    use crate::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn create_seatbelt_args_with_read_only_git_subpath() {
        // Create a temporary workspace with two writable roots: one containing
        // a top-level .git directory and one without it.
        let tmp = TempDir::new().expect("tempdir");
        let PopulatedTmp {
            root_with_git,
            root_without_git,
            root_with_git_canon,
            root_with_git_git_canon,
            root_without_git_canon,
        } = populate_tmpdir(tmp.path());

        // Build a policy that only includes the two test roots as writable and
        // does not automatically include defaults like cwd or TMPDIR.
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![root_with_git.clone(), root_without_git.clone()],
            network_access: false,
            include_default_writable_roots: false,
        };

        let args = create_seatbelt_command_args(
            vec!["/bin/echo".to_string(), "hello".to_string()],
            &policy,
            tmp.path(),
        );

        // Build the expected policy text using a raw string for readability.
        // Note that the policy includes:
        // - the base policy,
        // - read-only access to the filesystem,
        // - write access to WRITABLE_ROOT_0 (but not its .git) and WRITABLE_ROOT_1.
        let expected_policy = format!(
            r#"{MACOS_SEATBELT_BASE_POLICY}
; allow read-only file operations
(allow file-read*)
(allow file-write*
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) ) (subpath (param "WRITABLE_ROOT_1"))
)
"#,
        );

        let expected_args = vec![
            "-p".to_string(),
            expected_policy,
            format!(
                "-DWRITABLE_ROOT_0={}",
                root_with_git_canon.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_0={}",
                root_with_git_git_canon.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_1={}",
                root_without_git_canon.to_string_lossy()
            ),
            "--".to_string(),
            "/bin/echo".to_string(),
            "hello".to_string(),
        ];

        assert_eq!(args, expected_args);
    }

    #[test]
    fn create_seatbelt_args_for_cwd_as_git_repo() {
        // Create a temporary workspace with two writable roots: one containing
        // a top-level .git directory and one without it.
        let tmp = TempDir::new().expect("tempdir");
        let PopulatedTmp {
            root_with_git,
            root_with_git_canon,
            root_with_git_git_canon,
            ..
        } = populate_tmpdir(tmp.path());

        // Build a policy that does not specify any writable_roots, but does
        // use the default ones (cwd and TMPDIR) and verifies the `.git` check
        // is done properly for cwd.
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            include_default_writable_roots: true,
        };

        let args = create_seatbelt_command_args(
            vec!["/bin/echo".to_string(), "hello".to_string()],
            &policy,
            root_with_git.as_path(),
        );

        let tmpdir_env_var = if cfg!(target_os = "macos") {
            std::env::var("TMPDIR")
                .ok()
                .map(PathBuf::from)
                .and_then(|p| p.canonicalize().ok())
                .map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };
        let tempdir_policy_entry = if tmpdir_env_var.is_some() {
            " (subpath (param \"WRITABLE_ROOT_1\"))"
        } else {
            ""
        };

        // Build the expected policy text using a raw string for readability.
        // Note that the policy includes:
        // - the base policy,
        // - read-only access to the filesystem,
        // - write access to WRITABLE_ROOT_0 (but not its .git) and WRITABLE_ROOT_1.
        let expected_policy = format!(
            r#"{MACOS_SEATBELT_BASE_POLICY}
; allow read-only file operations
(allow file-read*)
(allow file-write*
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) ){tempdir_policy_entry}
)
"#,
        );

        let mut expected_args = vec![
            "-p".to_string(),
            expected_policy,
            format!(
                "-DWRITABLE_ROOT_0={}",
                root_with_git_canon.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_0={}",
                root_with_git_git_canon.to_string_lossy()
            ),
        ];

        if let Some(p) = tmpdir_env_var {
            expected_args.push(format!("-DWRITABLE_ROOT_1={p}"));
        }

        expected_args.extend(vec![
            "--".to_string(),
            "/bin/echo".to_string(),
            "hello".to_string(),
        ]);

        assert_eq!(args, expected_args);
    }

    #[test]
    fn seatbelt_base_policy_allows_ipc_posix_sem() {
        assert!(
            MACOS_SEATBELT_BASE_POLICY.contains("(allow ipc-posix-sem)"),
            "base policy should allow ipc-posix-sem"
        );
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn python_multiprocessing_lock_works_under_seatbelt() {
        use super::spawn_command_under_seatbelt;
        use crate::spawn::StdioPolicy;
        use std::collections::HashMap;

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            include_default_writable_roots: true,
        };

        let python_code = r#"import multiprocessing
from multiprocessing import Lock, Process

def f(lock):
    with lock:
        print("Lock acquired in child process")

if __name__ == '__main__':
    lock = Lock()
    p = Process(target=f, args=(lock,))
    p.start()
    p.join()
"#;

        let mut child = spawn_command_under_seatbelt(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                python_code.to_string(),
            ],
            &policy,
            std::env::current_dir().expect("should be able to get current dir"),
            StdioPolicy::RedirectForShellTool,
            HashMap::new(),
        )
        .await
        .expect("should be able to spawn python under seatbelt");

        let status = child
            .wait()
            .await
            .expect("should wait for child process");
        assert!(status.success(), "python exited with {status:?}");
    }

    struct PopulatedTmp {
        root_with_git: PathBuf,
        root_without_git: PathBuf,
        root_with_git_canon: PathBuf,
        root_with_git_git_canon: PathBuf,
        root_without_git_canon: PathBuf,
    }

    fn populate_tmpdir(tmp: &Path) -> PopulatedTmp {
        let root_with_git = tmp.join("with_git");
        let root_without_git = tmp.join("no_git");
        fs::create_dir_all(&root_with_git).expect("create with_git");
        fs::create_dir_all(&root_without_git).expect("create no_git");
        fs::create_dir_all(root_with_git.join(".git")).expect("create .git");

        // Ensure we have canonical paths for -D parameter matching.
        let root_with_git_canon = root_with_git.canonicalize().expect("canonicalize with_git");
        let root_with_git_git_canon = root_with_git_canon.join(".git");
        let root_without_git_canon = root_without_git
            .canonicalize()
            .expect("canonicalize no_git");
        PopulatedTmp {
            root_with_git,
            root_without_git,
            root_with_git_canon,
            root_with_git_git_canon,
            root_without_git_canon,
        }
    }
}
