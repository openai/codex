#![cfg(target_os = "macos")]

use codex_network_proxy::ALLOW_LOCAL_BINDING_ENV_KEY;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::PROXY_URL_ENV_KEYS;
use codex_network_proxy::has_proxy_url_env_vars;
use codex_network_proxy::proxy_url_env_value;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::ffi::CStr;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;
use url::Url;

use crate::protocol::SandboxPolicy;
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::SpawnChildRequest;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;

const MACOS_SEATBELT_BASE_POLICY: &str = include_str!("seatbelt_base_policy.sbpl");
const MACOS_SEATBELT_NETWORK_POLICY: &str = include_str!("seatbelt_network_policy.sbpl");

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
    network: Option<&NetworkProxy>,
    mut env: HashMap<String, String>,
) -> std::io::Result<Child> {
    let args =
        create_seatbelt_command_args(command, sandbox_policy, sandbox_policy_cwd, false, network);
    let arg0 = None;
    env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
    spawn_child_async(SpawnChildRequest {
        program: PathBuf::from(MACOS_PATH_TO_SEATBELT_EXECUTABLE),
        args,
        arg0,
        cwd: command_cwd,
        sandbox_policy,
        network,
        stdio_policy,
        env,
    })
    .await
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

fn proxy_scheme_default_port(scheme: &str) -> u16 {
    match scheme {
        "https" => 443,
        "socks5" | "socks5h" | "socks4" | "socks4a" => 1080,
        _ => 80,
    }
}

fn proxy_loopback_ports_from_env(env: &HashMap<String, String>) -> Vec<u16> {
    let mut ports = BTreeSet::new();
    for key in PROXY_URL_ENV_KEYS {
        let Some(proxy_url) = proxy_url_env_value(env, key) else {
            continue;
        };
        let trimmed = proxy_url.trim();
        if trimmed.is_empty() {
            continue;
        }

        let candidate = if trimmed.contains("://") {
            trimmed.to_string()
        } else {
            format!("http://{trimmed}")
        };
        let Ok(parsed) = Url::parse(&candidate) else {
            continue;
        };
        let Some(host) = parsed.host_str() else {
            continue;
        };
        if !is_loopback_host(host) {
            continue;
        }

        let scheme = parsed.scheme().to_ascii_lowercase();
        let port = parsed
            .port()
            .unwrap_or_else(|| proxy_scheme_default_port(scheme.as_str()));
        ports.insert(port);
    }
    ports.into_iter().collect()
}

fn local_binding_enabled(env: &HashMap<String, String>) -> bool {
    env.get(ALLOW_LOCAL_BINDING_ENV_KEY).is_some_and(|value| {
        let trimmed = value.trim();
        trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
    })
}

#[derive(Debug, Default)]
struct ProxyPolicyInputs {
    ports: Vec<u16>,
    has_proxy_config: bool,
    allow_local_binding: bool,
}

fn proxy_policy_inputs(network: Option<&NetworkProxy>) -> ProxyPolicyInputs {
    if let Some(network) = network {
        let mut env = HashMap::new();
        network.apply_to_env(&mut env);
        return ProxyPolicyInputs {
            ports: proxy_loopback_ports_from_env(&env),
            has_proxy_config: has_proxy_url_env_vars(&env),
            allow_local_binding: local_binding_enabled(&env),
        };
    }

    ProxyPolicyInputs::default()
}

fn dynamic_network_policy(
    sandbox_policy: &SandboxPolicy,
    enforce_managed_network: bool,
    proxy: &ProxyPolicyInputs,
) -> String {
    if !proxy.ports.is_empty() {
        let mut policy =
            String::from("; allow outbound access only to configured loopback proxy endpoints\n");
        if proxy.allow_local_binding {
            policy.push_str("; allow localhost-only binding and loopback traffic\n");
            policy.push_str("(allow network-bind (local ip \"localhost:*\"))\n");
            policy.push_str("(allow network-inbound (local ip \"localhost:*\"))\n");
            policy.push_str("(allow network-outbound (remote ip \"localhost:*\"))\n");
        }
        for port in &proxy.ports {
            policy.push_str(&format!(
                "(allow network-outbound (remote ip \"localhost:{port}\"))\n"
            ));
        }
        return format!("{policy}{MACOS_SEATBELT_NETWORK_POLICY}");
    }

    if proxy.has_proxy_config {
        // Proxy configuration is present but we could not infer any valid loopback endpoints.
        // Fail closed to avoid silently widening network access in proxy-enforced sessions.
        return String::new();
    }

    if enforce_managed_network {
        // Managed network requirements are active but no usable proxy endpoints
        // are available. Fail closed for network access.
        return String::new();
    }

    if sandbox_policy.has_full_network_access() {
        // No proxy env is configured: retain the existing full-network behavior.
        format!(
            "(allow network-outbound)\n(allow network-inbound)\n{MACOS_SEATBELT_NETWORK_POLICY}"
        )
    } else {
        String::new()
    }
}

pub(crate) fn create_seatbelt_command_args(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    enforce_managed_network: bool,
    network: Option<&NetworkProxy>,
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

    let (file_read_policy, file_read_dir_params) = if sandbox_policy.has_full_disk_read_access() {
        (
            "; allow read-only file operations\n(allow file-read*)".to_string(),
            Vec::new(),
        )
    } else {
        let mut readable_roots_policies: Vec<String> = Vec::new();
        let mut file_read_params = Vec::new();
        for (index, root) in sandbox_policy
            .get_readable_roots_with_cwd(sandbox_policy_cwd)
            .into_iter()
            .enumerate()
        {
            // Canonicalize to avoid mismatches like /var vs /private/var on macOS.
            let canonical_root = root
                .as_path()
                .canonicalize()
                .unwrap_or_else(|_| root.to_path_buf());
            let root_param = format!("READABLE_ROOT_{index}");
            file_read_params.push((root_param.clone(), canonical_root));
            readable_roots_policies.push(format!("(subpath (param \"{root_param}\"))"));
        }

        if readable_roots_policies.is_empty() {
            ("".to_string(), Vec::new())
        } else {
            (
                format!(
                    "; allow read-only file operations\n(allow file-read*\n{}\n)",
                    readable_roots_policies.join(" ")
                ),
                file_read_params,
            )
        }
    };

    let proxy = proxy_policy_inputs(network);
    let network_policy = dynamic_network_policy(sandbox_policy, enforce_managed_network, &proxy);

    let full_policy = format!(
        "{MACOS_SEATBELT_BASE_POLICY}\n{file_read_policy}\n{file_write_policy}\n{network_policy}"
    );

    let dir_params = [
        file_read_dir_params,
        file_write_dir_params,
        macos_dir_params(),
    ]
    .concat();

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
    use super::ProxyPolicyInputs;
    use super::create_seatbelt_command_args;
    use super::dynamic_network_policy;
    use super::macos_dir_params;
    use crate::protocol::SandboxPolicy;
    use crate::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::TempDir;

    fn assert_seatbelt_denied(stderr: &[u8], path: &Path) {
        let stderr = String::from_utf8_lossy(stderr);
        let expected = format!("bash: {}: Operation not permitted\n", path.display());
        assert!(
            stderr == expected
                || stderr.contains("sandbox-exec: sandbox_apply: Operation not permitted"),
            "unexpected stderr: {stderr}"
        );
    }

    #[test]
    fn base_policy_allows_node_cpu_sysctls() {
        assert!(
            MACOS_SEATBELT_BASE_POLICY.contains("(sysctl-name \"machdep.cpu.brand_string\")"),
            "base policy must allow CPU brand lookup for os.cpus()"
        );
        assert!(
            MACOS_SEATBELT_BASE_POLICY.contains("(sysctl-name \"hw.model\")"),
            "base policy must allow hardware model lookup for os.cpus()"
        );
    }

    #[test]
    fn create_seatbelt_args_routes_network_through_proxy_ports() {
        let policy = dynamic_network_policy(
            &SandboxPolicy::new_read_only_policy(),
            false,
            &ProxyPolicyInputs {
                ports: vec![43128, 48081],
                has_proxy_config: true,
                allow_local_binding: false,
            },
        );

        assert!(
            policy.contains("(allow network-outbound (remote ip \"localhost:43128\"))"),
            "expected HTTP proxy port allow rule in policy:\n{policy}"
        );
        assert!(
            policy.contains("(allow network-outbound (remote ip \"localhost:48081\"))"),
            "expected SOCKS proxy port allow rule in policy:\n{policy}"
        );
        assert!(
            !policy.contains("\n(allow network-outbound)\n"),
            "policy should not include blanket outbound allowance when proxy ports are present:\n{policy}"
        );
        assert!(
            !policy.contains("(allow network-bind (local ip \"localhost:*\"))"),
            "policy should not allow loopback binding unless explicitly enabled:\n{policy}"
        );
        assert!(
            !policy.contains("(allow network-inbound (local ip \"localhost:*\"))"),
            "policy should not allow loopback inbound unless explicitly enabled:\n{policy}"
        );
    }

    #[test]
    fn create_seatbelt_args_allows_local_binding_when_explicitly_enabled() {
        let policy = dynamic_network_policy(
            &SandboxPolicy::new_read_only_policy(),
            false,
            &ProxyPolicyInputs {
                ports: vec![43128],
                has_proxy_config: true,
                allow_local_binding: true,
            },
        );

        assert!(
            policy.contains("(allow network-bind (local ip \"localhost:*\"))"),
            "policy should allow loopback binding when explicitly enabled:\n{policy}"
        );
        assert!(
            policy.contains("(allow network-inbound (local ip \"localhost:*\"))"),
            "policy should allow loopback inbound when explicitly enabled:\n{policy}"
        );
        assert!(
            policy.contains("(allow network-outbound (remote ip \"localhost:*\"))"),
            "policy should allow loopback outbound when explicitly enabled:\n{policy}"
        );
        assert!(
            !policy.contains("\n(allow network-outbound)\n"),
            "policy should keep proxy-routed behavior without blanket outbound allowance:\n{policy}"
        );
    }

    #[test]
    fn dynamic_network_policy_fails_closed_when_proxy_config_without_ports() {
        let policy = dynamic_network_policy(
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                read_only_access: Default::default(),
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
            false,
            &ProxyPolicyInputs {
                ports: vec![],
                has_proxy_config: true,
                allow_local_binding: false,
            },
        );

        assert!(
            !policy.contains("\n(allow network-outbound)\n"),
            "policy should not include blanket outbound allowance when proxy config is present without ports:\n{policy}"
        );
        assert!(
            !policy.contains("(allow network-outbound (remote ip \"localhost:"),
            "policy should not include proxy port allowance when proxy config is present without ports:\n{policy}"
        );
    }

    #[test]
    fn dynamic_network_policy_fails_closed_for_managed_network_without_proxy_config() {
        let policy = dynamic_network_policy(
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                read_only_access: Default::default(),
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
            true,
            &ProxyPolicyInputs {
                ports: vec![],
                has_proxy_config: false,
                allow_local_binding: false,
            },
        );

        assert_eq!(policy, "");
    }

    #[test]
    fn create_seatbelt_args_full_network_with_proxy_is_still_proxy_only() {
        let policy = dynamic_network_policy(
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                read_only_access: Default::default(),
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
            false,
            &ProxyPolicyInputs {
                ports: vec![43128],
                has_proxy_config: true,
                allow_local_binding: false,
            },
        );

        assert!(
            policy.contains("(allow network-outbound (remote ip \"localhost:43128\"))"),
            "expected proxy endpoint allow rule in policy:\n{policy}"
        );
        assert!(
            !policy.contains("\n(allow network-outbound)\n"),
            "policy should not include blanket outbound allowance when proxy is configured:\n{policy}"
        );
        assert!(
            !policy.contains("\n(allow network-inbound)\n"),
            "policy should not include blanket inbound allowance when proxy is configured:\n{policy}"
        );
    }

    #[test]
    fn create_seatbelt_args_with_read_only_git_and_codex_subpaths() {
        // Create a temporary workspace with two writable roots: one containing
        // top-level .git and .codex directories and one without them.
        let tmp = TempDir::new().expect("tempdir");
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

        // Build a policy that only includes the two test roots as writable and
        // does not automatically include defaults TMPDIR or /tmp.
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![vulnerable_root, empty_root]
                .into_iter()
                .map(|p| p.try_into().unwrap())
                .collect(),
            read_only_access: Default::default(),
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
        let args = create_seatbelt_command_args(shell_command.clone(), &policy, &cwd, false, None);

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
        assert_seatbelt_denied(&output.stderr, &config_toml);

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
            create_seatbelt_command_args(shell_command_git, &policy, &cwd, false, None);
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
        assert_seatbelt_denied(&output.stderr, &pre_commit_hook);

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
            create_seatbelt_command_args(shell_command_allowed, &policy, &cwd, false, None);
        let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
            .args(&write_allowed_file_args)
            .current_dir(&cwd)
            .output()
            .expect("execute seatbelt command");
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success()
            && stderr.contains("sandbox-exec: sandbox_apply: Operation not permitted")
        {
            return;
        }
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
    fn create_seatbelt_args_with_read_only_git_pointer_file() {
        let tmp = TempDir::new().expect("tempdir");
        let worktree_root = tmp.path().join("worktree_root");
        fs::create_dir_all(&worktree_root).expect("create worktree_root");
        let gitdir = worktree_root.join("actual-gitdir");
        fs::create_dir_all(&gitdir).expect("create gitdir");
        let gitdir_config = gitdir.join("config");
        let gitdir_config_contents = "[core]\n";
        fs::write(&gitdir_config, gitdir_config_contents).expect("write gitdir config");

        let dot_git = worktree_root.join(".git");
        let dot_git_contents = format!("gitdir: {}\n", gitdir.to_string_lossy());
        fs::write(&dot_git, &dot_git_contents).expect("write .git pointer");

        let cwd = tmp.path().join("cwd");
        fs::create_dir_all(&cwd).expect("create cwd");

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![worktree_root.try_into().expect("worktree_root is absolute")],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let shell_command: Vec<String> = [
            "bash",
            "-c",
            "echo 'pwned!' > \"$1\"",
            "bash",
            dot_git.to_string_lossy().as_ref(),
        ]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
        let args = create_seatbelt_command_args(shell_command, &policy, &cwd, false, None);

        let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
            .args(&args)
            .current_dir(&cwd)
            .output()
            .expect("execute seatbelt command");

        assert_eq!(
            dot_git_contents,
            String::from_utf8_lossy(&fs::read(&dot_git).expect("read .git pointer")),
            ".git pointer file should not be modified under seatbelt"
        );
        assert!(
            !output.status.success(),
            "command to write {} should fail under seatbelt",
            dot_git.display()
        );
        assert_seatbelt_denied(&output.stderr, &dot_git);

        let shell_command_gitdir: Vec<String> = [
            "bash",
            "-c",
            "echo 'pwned!' > \"$1\"",
            "bash",
            gitdir_config.to_string_lossy().as_ref(),
        ]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
        let gitdir_args =
            create_seatbelt_command_args(shell_command_gitdir, &policy, &cwd, false, None);
        let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
            .args(&gitdir_args)
            .current_dir(&cwd)
            .output()
            .expect("execute seatbelt command");

        assert_eq!(
            gitdir_config_contents,
            String::from_utf8_lossy(&fs::read(&gitdir_config).expect("read gitdir config")),
            "gitdir config should contain its original contents because it should not have been modified"
        );
        assert!(
            !output.status.success(),
            "command to write {} should fail under seatbelt",
            gitdir_config.display()
        );
        assert_seatbelt_denied(&output.stderr, &gitdir_config);
    }

    #[test]
    fn create_seatbelt_args_for_cwd_as_git_repo() {
        // Create a temporary workspace with two writable roots: one containing
        // top-level .git and .codex directories and one without them.
        let tmp = TempDir::new().expect("tempdir");
        let PopulatedTmp {
            vulnerable_root,
            vulnerable_root_canonical,
            dot_git_canonical,
            dot_codex_canonical,
            ..
        } = populate_tmpdir(tmp.path());

        // Build a policy that does not specify any writable_roots, but does
        // use the default ones (cwd and TMPDIR) and verifies the `.git` and
        // `.codex` checks are done properly for cwd.
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
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
            false,
            None,
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
