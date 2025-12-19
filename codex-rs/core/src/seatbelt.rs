#![cfg(target_os = "macos")]

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::CStr;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

use crate::config;
use crate::network_proxy;
use crate::protocol::SandboxPolicy;
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;

const MACOS_SEATBELT_BASE_POLICY: &str = include_str!("seatbelt_base_policy.sbpl");
const MACOS_SEATBELT_NETWORK_POLICY_BASE: &str = include_str!("seatbelt_network_policy.sbpl");
const PROXY_ENV_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
];

/// When working with `sandbox-exec`, only consider `sandbox-exec` in `/usr/bin`
/// to defend against an attacker trying to inject a malicious version on the
/// PATH. If /usr/bin/sandbox-exec has been tampered with, then the attacker
/// already has root access.
pub(crate) const MACOS_PATH_TO_SEATBELT_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

pub async fn spawn_command_under_seatbelt(
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    stdio_policy: StdioPolicy,
    mut env: HashMap<String, String>,
) -> std::io::Result<Child> {
    let args = create_seatbelt_command_args(command, sandbox_policy, sandbox_policy_cwd, &env);
    let arg0 = None;
    env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
    spawn_child_async(
        PathBuf::from(MACOS_PATH_TO_SEATBELT_EXECUTABLE),
        args,
        arg0,
        command_cwd,
        sandbox_policy,
        stdio_policy,
        env,
    )
    .await
}

fn proxy_allowlist_from_env(env: &HashMap<String, String>) -> Vec<String> {
    let mut allowlist = Vec::new();
    let mut seen = HashSet::new();

    for key in PROXY_ENV_KEYS {
        let Some(proxy_url) = env.get(*key) else {
            continue;
        };
        let Some((host, port)) = network_proxy::proxy_host_port(proxy_url) else {
            continue;
        };
        for entry in proxy_allowlist_entries(&host, port) {
            if seen.insert(entry.clone()) {
                allowlist.push(entry);
            }
        }
    }

    allowlist
}

fn proxy_allowlist_entries(host: &str, port: i64) -> Vec<String> {
    let mut entries = Vec::new();
    let is_loopback = is_loopback_host(host);

    if is_loopback {
        for candidate in ["localhost", "127.0.0.1", "::1"] {
            entries.push(format_proxy_host_port(candidate, port));
        }
    } else {
        entries.push(format_proxy_host_port(host, port));
    }

    entries
}

fn format_proxy_host_port(host: &str, port: i64) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn is_loopback_host(host: &str) -> bool {
    let host_lower = host.to_ascii_lowercase();
    host_lower == "localhost" || host == "127.0.0.1" || host == "::1"
}

#[derive(Default)]
struct ProxyPorts {
    http: Vec<u16>,
    socks: Vec<u16>,
}

fn proxy_ports_from_env(env: &HashMap<String, String>) -> ProxyPorts {
    let mut http_ports = BTreeSet::new();
    let mut socks_ports = BTreeSet::new();

    for key in PROXY_ENV_KEYS {
        let Some(proxy_url) = env.get(*key) else {
            continue;
        };
        let Some((host, port)) = network_proxy::proxy_host_port(proxy_url) else {
            continue;
        };
        let Some(port) = normalize_proxy_port(port) else {
            continue;
        };
        if !is_loopback_host(&host) {
            continue;
        }
        let scheme = proxy_url_scheme(proxy_url).unwrap_or("http");
        if scheme.to_ascii_lowercase().starts_with("socks") {
            socks_ports.insert(port);
        } else {
            http_ports.insert(port);
        }
    }

    ProxyPorts {
        http: http_ports.into_iter().collect(),
        socks: socks_ports.into_iter().collect(),
    }
}

fn proxy_url_scheme(proxy_url: &str) -> Option<&str> {
    proxy_url.split_once("://").map(|(scheme, _)| scheme)
}

fn normalize_proxy_port(port: i64) -> Option<u16> {
    if (1..=u16::MAX as i64).contains(&port) {
        Some(port as u16)
    } else {
        None
    }
}

fn normalize_unix_socket_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path_buf = PathBuf::from(trimmed);
    let normalized = if path_buf.is_absolute() {
        path_buf.canonicalize().unwrap_or(path_buf)
    } else {
        path_buf
    };
    Some(normalized.to_string_lossy().to_string())
}

fn escape_sbpl_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn build_network_policy(
    proxy_allowlist: &[String],
    policy: &network_proxy::NetworkPolicy,
    proxy_ports: &ProxyPorts,
) -> String {
    let mut network_rules = String::from("; Network\n");
    if proxy_allowlist.is_empty() {
        network_rules.push_str("(allow network*)\n");
        return format!("{network_rules}{MACOS_SEATBELT_NETWORK_POLICY_BASE}");
    }

    if policy.allow_local_binding {
        network_rules.push_str("(allow network-bind (local ip \"localhost:*\"))\n");
        network_rules.push_str("(allow network-inbound (local ip \"localhost:*\"))\n");
        network_rules.push_str("(allow network-outbound (local ip \"localhost:*\"))\n");
    }

    if !policy.allow_unix_sockets.is_empty() {
        let mut seen = HashSet::new();
        for socket_path in &policy.allow_unix_sockets {
            let Some(normalized) = normalize_unix_socket_path(socket_path) else {
                continue;
            };
            if !seen.insert(normalized.clone()) {
                continue;
            }
            let escaped = escape_sbpl_string(&normalized);
            network_rules.push_str(&format!("(allow network* (subpath \"{escaped}\"))\n"));
        }
    }

    for port in &proxy_ports.http {
        network_rules.push_str(&format!(
            "(allow network-bind (local ip \"localhost:{port}\"))\n"
        ));
        network_rules.push_str(&format!(
            "(allow network-inbound (local ip \"localhost:{port}\"))\n"
        ));
        network_rules.push_str(&format!(
            "(allow network-outbound (remote ip \"localhost:{port}\"))\n"
        ));
    }

    for port in &proxy_ports.socks {
        network_rules.push_str(&format!(
            "(allow network-bind (local ip \"localhost:{port}\"))\n"
        ));
        network_rules.push_str(&format!(
            "(allow network-inbound (local ip \"localhost:{port}\"))\n"
        ));
        network_rules.push_str(&format!(
            "(allow network-outbound (remote ip \"localhost:{port}\"))\n"
        ));
    }

    let mut outbound = String::from("(allow network-outbound\n");
    for endpoint in proxy_allowlist {
        outbound.push_str(&format!("  (remote tcp \"{endpoint}\")\n"));
    }
    outbound.push_str(")\n");
    network_rules.push_str(&outbound);

    format!("{network_rules}{MACOS_SEATBELT_NETWORK_POLICY_BASE}")
}

pub(crate) fn create_seatbelt_command_args(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    env: &HashMap<String, String>,
) -> Vec<String> {
    let (file_write_policy, file_write_dir_params) = {
        if sandbox_policy.has_full_disk_write_access() {
            // Allegedly, this is more permissive than `(allow file-write*)`.
            (
                r#"(allow file-write* (regex #"^/"))"#.to_string(),
                Vec::new(),
            )
        } else {
            let writable_roots = sandbox_policy.get_writable_roots_with_cwd(sandbox_policy_cwd);

            let mut writable_folder_policies: Vec<String> = Vec::new();
            let mut file_write_params = Vec::new();

            for (index, wr) in writable_roots.iter().enumerate() {
                // Canonicalize to avoid mismatches like /var vs /private/var on macOS.
                let canonical_root = wr
                    .root
                    .as_path()
                    .canonicalize()
                    .unwrap_or_else(|_| wr.root.to_path_buf());
                let root_param = format!("WRITABLE_ROOT_{index}");
                file_write_params.push((root_param.clone(), canonical_root));

                if wr.read_only_subpaths.is_empty() {
                    writable_folder_policies.push(format!("(subpath (param \"{root_param}\"))"));
                } else {
                    // Add parameters for each read-only subpath and generate
                    // the `(require-not ...)` clauses.
                    let mut require_parts: Vec<String> = Vec::new();
                    require_parts.push(format!("(subpath (param \"{root_param}\"))"));
                    for (subpath_index, ro) in wr.read_only_subpaths.iter().enumerate() {
                        let canonical_ro = ro
                            .as_path()
                            .canonicalize()
                            .unwrap_or_else(|_| ro.to_path_buf());
                        let ro_param = format!("WRITABLE_ROOT_{index}_RO_{subpath_index}");
                        require_parts
                            .push(format!("(require-not (subpath (param \"{ro_param}\")))"));
                        file_write_params.push((ro_param, canonical_ro));
                    }
                    let policy_component = format!("(require-all {} )", require_parts.join(" "));
                    writable_folder_policies.push(policy_component);
                }
            }

            if writable_folder_policies.is_empty() {
                ("".to_string(), Vec::new())
            } else {
                let file_write_policy = format!(
                    "(allow file-write*\n{}\n)",
                    writable_folder_policies.join(" ")
                );
                (file_write_policy, file_write_params)
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
        let proxy_allowlist = proxy_allowlist_from_env(env);
        let proxy_ports = proxy_ports_from_env(env);
        let policy = config::default_config_path()
            .ok()
            .and_then(|path| network_proxy::load_network_policy(&path).ok())
            .unwrap_or_default();
        build_network_policy(&proxy_allowlist, &policy, &proxy_ports)
    } else {
        String::new()
    };

    let full_policy = format!(
        "{MACOS_SEATBELT_BASE_POLICY}\n{file_read_policy}\n{file_write_policy}\n{network_policy}"
    );

    let dir_params = [file_write_dir_params, macos_dir_params()].concat();

    let mut seatbelt_args: Vec<String> = vec!["-p".to_string(), full_policy];
    let definition_args = dir_params
        .into_iter()
        .map(|(key, value)| format!("-D{key}={value}", value = value.to_string_lossy()));
    seatbelt_args.extend(definition_args);
    seatbelt_args.push("--".to_string());
    seatbelt_args.extend(command);
    seatbelt_args
}

/// Wraps libc::confstr to return a String.
fn confstr(name: libc::c_int) -> Option<String> {
    let mut buf = vec![0_i8; (libc::PATH_MAX as usize) + 1];
    let len = unsafe { libc::confstr(name, buf.as_mut_ptr(), buf.len()) };
    if len == 0 {
        return None;
    }
    // confstr guarantees NUL-termination when len > 0.
    let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
    cstr.to_str().ok().map(ToString::to_string)
}

/// Wraps confstr to return a canonicalized PathBuf.
fn confstr_path(name: libc::c_int) -> Option<PathBuf> {
    let s = confstr(name)?;
    let path = PathBuf::from(s);
    path.canonicalize().ok().or(Some(path))
}

fn macos_dir_params() -> Vec<(String, PathBuf)> {
    if let Some(p) = confstr_path(libc::_CS_DARWIN_USER_CACHE_DIR) {
        return vec![("DARWIN_USER_CACHE_DIR".to_string(), p)];
    }
    vec![]
}

#[cfg(test)]
mod tests {
    use super::MACOS_SEATBELT_BASE_POLICY;
    use super::create_seatbelt_command_args;
    use super::macos_dir_params;
    use crate::protocol::SandboxPolicy;
    use crate::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
    use pretty_assertions::assert_eq;
    use serial_test::serial;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::TempDir;

    struct CodexHomeGuard {
        previous: Option<String>,
    }

    impl CodexHomeGuard {
        fn new(path: &Path) -> Self {
            let previous = std::env::var("CODEX_HOME").ok();
            std::env::set_var("CODEX_HOME", path);
            Self { previous }
        }
    }

    impl Drop for CodexHomeGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var("CODEX_HOME", previous);
            } else {
                std::env::remove_var("CODEX_HOME");
            }
        }
    }

    #[test]
    #[serial]
    fn create_seatbelt_args_with_read_only_git_and_codex_subpaths() {
        // Create a temporary workspace with two writable roots: one containing
        // top-level .git and .codex directories and one without them.
        let tmp = TempDir::new().expect("tempdir");
        let _codex_home_guard = CodexHomeGuard::new(tmp.path());
        let PopulatedTmp {
            vulnerable_root,
            vulnerable_root_canonical,
            dot_git_canonical,
            dot_codex_canonical,
            empty_root,
            empty_root_canonical,
        } = populate_tmpdir(tmp.path());
        let cwd = tmp.path().join("cwd");
        fs::create_dir_all(&cwd).expect("create cwd");
        let env = std::collections::HashMap::new();

        // Build a policy that only includes the two test roots as writable and
        // does not automatically include defaults TMPDIR or /tmp.
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![vulnerable_root, empty_root]
                .into_iter()
                .map(|p| p.try_into().unwrap())
                .collect(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        // Create the Seatbelt command to wrap a shell command that tries to
        // write to .codex/config.toml in the vulnerable root.
        let shell_command: Vec<String> = [
            "bash",
            "-c",
            "echo 'sandbox_mode = \"danger-full-access\"' > \"$1\"",
            "bash",
            dot_codex_canonical
                .join("config.toml")
                .to_string_lossy()
                .as_ref(),
        ]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
        let args = create_seatbelt_command_args(shell_command.clone(), &policy, &cwd, &env);

        // Build the expected policy text using a raw string for readability.
        // Note that the policy includes:
        // - the base policy,
        // - read-only access to the filesystem,
        // - write access to WRITABLE_ROOT_0 (but not its .git or .codex), WRITABLE_ROOT_1, and cwd as WRITABLE_ROOT_2.
        let expected_policy = format!(
            r#"{MACOS_SEATBELT_BASE_POLICY}
; allow read-only file operations
(allow file-read*)
(allow file-write*
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) (require-not (subpath (param "WRITABLE_ROOT_0_RO_1"))) ) (subpath (param "WRITABLE_ROOT_1")) (subpath (param "WRITABLE_ROOT_2"))
)
"#,
        );

        let mut expected_args = vec![
            "-p".to_string(),
            expected_policy,
            format!(
                "-DWRITABLE_ROOT_0={}",
                vulnerable_root_canonical.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_0={}",
                dot_git_canonical.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_1={}",
                dot_codex_canonical.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_1={}",
                empty_root_canonical.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_2={}",
                cwd.canonicalize()
                    .expect("canonicalize cwd")
                    .to_string_lossy()
            ),
        ];

        expected_args.extend(
            macos_dir_params()
                .into_iter()
                .map(|(key, value)| format!("-D{key}={value}", value = value.to_string_lossy())),
        );

        expected_args.push("--".to_string());
        expected_args.extend(shell_command);

        assert_eq!(expected_args, args);

        // Verify that .codex/config.toml cannot be modified under the generated
        // Seatbelt policy.
        let config_toml = dot_codex_canonical.join("config.toml");
        let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
            .args(&args)
            .current_dir(&cwd)
            .output()
            .expect("execute seatbelt command");
        assert_eq!(
            "sandbox_mode = \"read-only\"\n",
            String::from_utf8_lossy(&fs::read(&config_toml).expect("read config.toml")),
            "config.toml should contain its original contents because it should not have been modified"
        );
        assert!(
            !output.status.success(),
            "command to write {} should fail under seatbelt",
            &config_toml.display()
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!("bash: {}: Operation not permitted\n", config_toml.display()),
        );

        // Create a similar Seatbelt command that tries to write to a file in
        // the .git folder, which should also be blocked.
        let pre_commit_hook = dot_git_canonical.join("hooks").join("pre-commit");
        let shell_command_git: Vec<String> = [
            "bash",
            "-c",
            "echo 'pwned!' > \"$1\"",
            "bash",
            pre_commit_hook.to_string_lossy().as_ref(),
        ]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
        let write_hooks_file_args =
            create_seatbelt_command_args(shell_command_git, &policy, &cwd, &env);
        let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
            .args(&write_hooks_file_args)
            .current_dir(&cwd)
            .output()
            .expect("execute seatbelt command");
        assert!(
            !fs::exists(&pre_commit_hook).expect("exists pre-commit hook"),
            "{} should not exist because it should not have been created",
            pre_commit_hook.display()
        );
        assert!(
            !output.status.success(),
            "command to write {} should fail under seatbelt",
            &pre_commit_hook.display()
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!(
                "bash: {}: Operation not permitted\n",
                pre_commit_hook.display()
            ),
        );

        // Verify that writing a file to the folder containing .git and .codex is allowed.
        let allowed_file = vulnerable_root_canonical.join("allowed.txt");
        let shell_command_allowed: Vec<String> = [
            "bash",
            "-c",
            "echo 'this is allowed' > \"$1\"",
            "bash",
            allowed_file.to_string_lossy().as_ref(),
        ]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
        let write_allowed_file_args =
            create_seatbelt_command_args(shell_command_allowed, &policy, &cwd, &env);
        let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
            .args(&write_allowed_file_args)
            .current_dir(&cwd)
            .output()
            .expect("execute seatbelt command");
        assert!(
            output.status.success(),
            "command to write {} should succeed under seatbelt",
            &allowed_file.display()
        );
        assert_eq!(
            "this is allowed\n",
            String::from_utf8_lossy(&fs::read(&allowed_file).expect("read allowed.txt")),
            "{} should contain the written text",
            allowed_file.display()
        );
    }

    #[test]
    #[serial]
    fn create_seatbelt_args_for_cwd_as_git_repo() {
        // Create a temporary workspace with two writable roots: one containing
        // top-level .git and .codex directories and one without them.
        let tmp = TempDir::new().expect("tempdir");
        let _codex_home_guard = CodexHomeGuard::new(tmp.path());
        let PopulatedTmp {
            vulnerable_root,
            vulnerable_root_canonical,
            dot_git_canonical,
            dot_codex_canonical,
            ..
        } = populate_tmpdir(tmp.path());
        let env = std::collections::HashMap::new();

        // Build a policy that does not specify any writable_roots, but does
        // use the default ones (cwd and TMPDIR) and verifies the `.git` and
        // `.codex` checks are done properly for cwd.
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

        let shell_command: Vec<String> = [
            "bash",
            "-c",
            "echo 'sandbox_mode = \"danger-full-access\"' > \"$1\"",
            "bash",
            dot_codex_canonical
                .join("config.toml")
                .to_string_lossy()
                .as_ref(),
        ]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
        let args = create_seatbelt_command_args(
            shell_command.clone(),
            &policy,
            vulnerable_root.as_path(),
            &env,
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
        // - write access to WRITABLE_ROOT_0 (but not its .git or .codex), WRITABLE_ROOT_1, and cwd as WRITABLE_ROOT_2.
        let expected_policy = format!(
            r#"{MACOS_SEATBELT_BASE_POLICY}
; allow read-only file operations
(allow file-read*)
(allow file-write*
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) (require-not (subpath (param "WRITABLE_ROOT_0_RO_1"))) ) (subpath (param "WRITABLE_ROOT_1")){tempdir_policy_entry}
)
"#,
        );

        let mut expected_args = vec![
            "-p".to_string(),
            expected_policy,
            format!(
                "-DWRITABLE_ROOT_0={}",
                vulnerable_root_canonical.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_0={}",
                dot_git_canonical.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_1={}",
                dot_codex_canonical.to_string_lossy()
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

        expected_args.extend(
            macos_dir_params()
                .into_iter()
                .map(|(key, value)| format!("-D{key}={value}", value = value.to_string_lossy())),
        );

        expected_args.push("--".to_string());
        expected_args.extend(shell_command);

        assert_eq!(expected_args, args);
    }

    #[test]
    #[serial]
    fn create_seatbelt_args_with_proxy_allowlist() {
        let tmp = TempDir::new().expect("tempdir");
        let _codex_home_guard = CodexHomeGuard::new(tmp.path());
        let policy = SandboxPolicy::DangerFullAccess;
        let cwd = std::env::current_dir().expect("getcwd");
        let env = std::collections::HashMap::from([(
            "HTTP_PROXY".to_string(),
            "http://127.0.0.1:3128".to_string(),
        )]);
        let args = create_seatbelt_command_args(vec!["true".to_string()], &policy, &cwd, &env);
        let policy_text = &args[1];
        assert!(
            policy_text.contains("(allow network-bind (local ip \"localhost:3128\"))"),
            "expected seatbelt policy to allow local proxy binding"
        );
        assert!(
            policy_text.contains("(allow network-inbound (local ip \"localhost:3128\"))"),
            "expected seatbelt policy to allow local proxy inbound"
        );
        assert!(
            policy_text.contains("(allow network-outbound (remote ip \"localhost:3128\"))"),
            "expected seatbelt policy to allow local proxy outbound"
        );
        assert!(
            policy_text.contains("(remote tcp \"127.0.0.1:3128\")"),
            "expected seatbelt policy to include the proxy allowlist"
        );
        assert!(
            !policy_text.contains("localhost:*"),
            "proxy-restricted policy should not allow all localhost ports"
        );
    }

    struct PopulatedTmp {
        /// Path containing a .git and .codex subfolder.
        /// For the purposes of this test, we consider this a "vulnerable" root
        /// because a bad actor could write to .git/hooks/pre-commit so an
        /// unsuspecting user would run code as privileged the next time they
        /// ran `git commit` themselves, or modified .codex/config.toml to
        /// contain `sandbox_mode = "danger-full-access"` so the agent would
        /// have full privileges the next time it ran in that repo.
        vulnerable_root: PathBuf,
        vulnerable_root_canonical: PathBuf,
        dot_git_canonical: PathBuf,
        dot_codex_canonical: PathBuf,

        /// Path without .git or .codex subfolders.
        empty_root: PathBuf,
        /// Canonicalized version of `empty_root`.
        empty_root_canonical: PathBuf,
    }

    fn populate_tmpdir(tmp: &Path) -> PopulatedTmp {
        let vulnerable_root = tmp.join("vulnerable_root");
        fs::create_dir_all(&vulnerable_root).expect("create vulnerable_root");

        // TODO(mbolin): Should also support the case where `.git` is a file
        // with a gitdir: ... line.
        Command::new("git")
            .arg("init")
            .arg(".")
            .current_dir(&vulnerable_root)
            .output()
            .expect("git init .");

        fs::create_dir_all(vulnerable_root.join(".codex")).expect("create .codex");
        fs::write(
            vulnerable_root.join(".codex").join("config.toml"),
            "sandbox_mode = \"read-only\"\n",
        )
        .expect("write .codex/config.toml");

        let empty_root = tmp.join("empty_root");
        fs::create_dir_all(&empty_root).expect("create empty_root");

        // Ensure we have canonical paths for -D parameter matching.
        let vulnerable_root_canonical = vulnerable_root
            .canonicalize()
            .expect("canonicalize vulnerable_root");
        let dot_git_canonical = vulnerable_root_canonical.join(".git");
        let dot_codex_canonical = vulnerable_root_canonical.join(".codex");
        let empty_root_canonical = empty_root.canonicalize().expect("canonicalize empty_root");
        PopulatedTmp {
            vulnerable_root,
            vulnerable_root_canonical,
            dot_git_canonical,
            dot_codex_canonical,
            empty_root,
            empty_root_canonical,
        }
    }
}
