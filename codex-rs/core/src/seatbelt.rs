use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

use crate::protocol::SandboxPolicy;
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;

/// Captures canonical and lexical representations of a path for Seatbelt policy generation.
struct SeatbeltPathExpr {
    canonical: String,
    lexical: Option<String>,
}

impl SeatbeltPathExpr {
    fn new(path: &Path) -> Self {
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let canonical = canonical_path.to_string_lossy().to_string();
        let lexical = if canonical_path == path || !path.is_absolute() {
            None
        } else {
            Some(path.to_string_lossy().to_string())
        };

        Self { canonical, lexical }
    }

    fn cli_arg(&self, param: &str) -> String {
        format!("-D{param}={}", self.canonical)
    }

    fn subpath_clause(&self, param: &str) -> String {
        match &self.lexical {
            Some(lexical) => format!(
                "(require-any (subpath (param \"{param}\")) (subpath \"{}\"))",
                escape_sbpl_string(lexical)
            ),
            None => format!("(subpath (param \"{param}\"))"),
        }
    }

    fn require_not_clause(&self, param: &str) -> String {
        match &self.lexical {
            Some(lexical) => format!(
                "(require-not (require-any (subpath (param \"{param}\")) (subpath \"{}\")))",
                escape_sbpl_string(lexical)
            ),
            None => format!("(require-not (subpath (param \"{param}\")))"),
        }
    }
}

fn escape_sbpl_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

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
                let root_param = format!("WRITABLE_ROOT_{index}");
                let root_expr = SeatbeltPathExpr::new(&wr.root);
                cli_args.push(root_expr.cli_arg(&root_param));

                if wr.read_only_subpaths.is_empty() {
                    writable_folder_policies.push(root_expr.subpath_clause(&root_param));
                } else {
                    // Add parameters for each read-only subpath and generate
                    // the `(require-not ...)` clauses.
                    let mut require_parts: Vec<String> = Vec::new();
                    require_parts.push(root_expr.subpath_clause(&root_param));
                    for (subpath_index, ro) in wr.read_only_subpaths.iter().enumerate() {
                        let ro_param = format!("WRITABLE_ROOT_{index}_RO_{subpath_index}");
                        let ro_expr = SeatbeltPathExpr::new(ro);
                        cli_args.push(ro_expr.cli_arg(&ro_param));
                        require_parts.push(ro_expr.require_not_clause(&ro_param));
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
        if cfg!(target_os = "windows") {
            // /tmp does not exist on Windows, so skip this test.
            return;
        }

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
        let cwd = tmp.path().join("cwd");

        // Build a policy that only includes the two test roots as writable and
        // does not automatically include defaults TMPDIR or /tmp.
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![root_with_git, root_without_git],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let args = create_seatbelt_command_args(
            vec!["/bin/echo".to_string(), "hello".to_string()],
            &policy,
            &cwd,
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
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) ) (subpath (param "WRITABLE_ROOT_1")) (subpath (param "WRITABLE_ROOT_2"))
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
            format!(
                "-DWRITABLE_ROOT_1={}",
                root_without_git_canon.to_string_lossy()
            ),
            format!("-DWRITABLE_ROOT_2={}", cwd.to_string_lossy()),
        ];

        expected_args.extend(vec![
            "--".to_string(),
            "/bin/echo".to_string(),
            "hello".to_string(),
        ]);

        assert_eq!(expected_args, args);
    }

    #[test]
    fn create_seatbelt_args_for_cwd_as_git_repo() {
        if cfg!(target_os = "windows") {
            // /tmp does not exist on Windows, so skip this test.
            return;
        }

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
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

        let args = create_seatbelt_command_args(
            vec!["/bin/echo".to_string(), "hello".to_string()],
            &policy,
            root_with_git.as_path(),
        );

        let tmpdir_env_var = std::env::var("TMPDIR")
            .ok()
            .map(PathBuf::from)
            .and_then(|p| p.canonicalize().ok())
            .map(|p| p.to_string_lossy().to_string());

        let tempdir_policy_entry = if tmpdir_env_var.is_some() {
            r#" (subpath (param "WRITABLE_ROOT_2"))"#
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
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) ) (subpath (param "WRITABLE_ROOT_1")){tempdir_policy_entry}
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
            format!(
                "-DWRITABLE_ROOT_1={}",
                PathBuf::from("/tmp")
                    .canonicalize()
                    .expect("canonicalize /tmp")
                    .to_string_lossy()
            ),
        ];

        if let Some(p) = tmpdir_env_var {
            expected_args.push(format!("-DWRITABLE_ROOT_2={p}"));
        }

        expected_args.extend(vec![
            "--".to_string(),
            "/bin/echo".to_string(),
            "hello".to_string(),
        ]);

        assert_eq!(expected_args, args);
    }

    #[cfg(unix)]
    #[test]
    fn create_seatbelt_args_with_symlink_root_includes_lexical_paths() {
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new().expect("tempdir");
        let real_root = tmp.path().join("real_repo");
        let symlink_root = tmp.path().join("symlink_repo");

        fs::create_dir_all(real_root.join(".git")).expect("create real repo with .git");
        symlink(&real_root, &symlink_root).expect("create symlink to repo");

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![symlink_root.clone()],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let args = create_seatbelt_command_args(
            vec!["/bin/echo".to_string(), "hello".to_string()],
            &policy,
            &symlink_root,
        );

        // Verify canonical path used for CLI argument.
        let canonical_root = real_root
            .canonicalize()
            .expect("canonicalize real root")
            .to_string_lossy()
            .to_string();
        let expected_cli_arg = format!("-DWRITABLE_ROOT_0={canonical_root}");
        assert!(
            args.contains(&expected_cli_arg),
            "missing canonical CLI arg"
        );

        let policy_text = &args[1];
        let lexical_root = symlink_root.to_string_lossy().to_string();
        assert!(
            policy_text.contains(&format!(
                "(require-any (subpath (param \"WRITABLE_ROOT_0\")) (subpath \"{lexical_root}\"))"
            )),
            "policy should allow lexical root form"
        );
        let lexical_git = symlink_root.join(".git").to_string_lossy().to_string();
        assert!(
            policy_text.contains(&format!(
                "(require-not (require-any (subpath (param \"WRITABLE_ROOT_0_RO_0\")) (subpath \"{lexical_git}\")))"
            )),
            "policy should deny lexical .git subpath"
        );
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
