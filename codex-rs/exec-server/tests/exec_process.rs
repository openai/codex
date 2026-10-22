mod common;
#[cfg(unix)]
#[path = "exec_process/shell_snapshot.rs"]
mod shell_snapshot;
#[cfg(unix)]
#[path = "common/tls_proxy.rs"]
mod tls_proxy;
#[path = "exec_process/windows_sandbox.rs"]
mod windows_sandbox;

#[cfg(unix)]
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::Environment;
use codex_exec_server::ExecBackend;
#[cfg(unix)]
use codex_exec_server::ExecEnvPolicy;
use codex_exec_server::ExecMetadata;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecProcessEvent;
#[cfg(any(unix, windows))]
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::ProcessId;
use codex_exec_server::ProcessSignal;
use codex_exec_server::ReadResponse;
#[cfg(unix)]
use codex_exec_server::ShellInfo;
#[cfg(unix)]
use codex_exec_server::ShellSnapshotRequest;
use codex_exec_server::StartedExecProcess;
#[cfg(any(unix, windows))]
use codex_exec_server::WindowsSandboxSelection;
use codex_exec_server::WriteStatus;
#[cfg(unix)]
use codex_network_proxy::NetworkProxyConfig;
#[cfg(unix)]
use codex_network_proxy::RemoteNetworkProxyConfig;
#[cfg(unix)]
use codex_network_proxy::RemoteNetworkProxyLaunchConfig;
#[cfg(unix)]
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
#[cfg(unix)]
use codex_protocol::models::PermissionProfile;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemAccessMode;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemPath;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemSandboxEntry;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemSandboxPolicy;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemSpecialPath;
#[cfg(unix)]
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;
use tokio::sync::watch;
use tokio::time::Duration;
use tokio::time::sleep;
use tokio::time::timeout;

use common::DELAYED_OUTPUT_AFTER_EXIT_PARENT_ARG;
use common::current_test_binary_helper_paths;
use common::exec_server::ExecServerHarness;
use common::exec_server::exec_server;

struct ProcessContext {
    backend: Arc<dyn ExecBackend>,
    _server: Option<ExecServerHarness>,
}

#[derive(Debug, PartialEq, Eq)]
enum ProcessEventSnapshot {
    Output {
        seq: u64,
        stream: ExecOutputStream,
        text: String,
    },
    Exited {
        seq: u64,
        exit_code: i32,
    },
    Closed {
        seq: u64,
    },
}

async fn create_process_context(use_remote: bool) -> Result<ProcessContext> {
    if use_remote {
        let server = exec_server().await?;
        let environment = Environment::create_for_tests(Some(server.websocket_url().to_string()))?;
        Ok(ProcessContext {
            backend: environment.get_exec_backend(),
            _server: Some(server),
        })
    } else {
        let environment = Environment::create_for_tests(/*exec_server_url*/ None)?;
        Ok(ProcessContext {
            backend: environment.get_exec_backend(),
            _server: None,
        })
    }
}

#[cfg(target_os = "macos")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_home_symlink_opt_out_respects_host_config_and_scope() -> Result<()> {
    use codex_exec_server::WriteFileOptions;
    use common::exec_server::exec_server_with_env;
    use std::os::unix::fs::symlink;

    let workspace = TempDir::new()?;
    let home = TempDir::new()?;
    let target = TempDir::new()?;
    let alias = home.path().join("visualizations");
    let other_alias = workspace.path().join(".codex/visualizations");
    std::fs::create_dir(workspace.path().join(".codex"))?;
    symlink(target.path(), &alias)?;
    symlink(target.path(), &other_alias)?;
    std::fs::write(
        workspace.path().join(".codex/config.toml"),
        "allow_symlinked_codex_home = true\n",
    )?;

    for enabled in [None, Some(false), Some(true)] {
        std::fs::write(
            home.path().join("config.toml"),
            enabled.map_or_else(String::new, |enabled| {
                format!("allow_symlinked_codex_home = {enabled}\n")
            }),
        )?;
        let mut server = exec_server_with_env([("CODEX_HOME", home.path())], &[]).await?;
        let environment = Environment::create_for_tests(Some(server.websocket_url().to_string()))?;
        for root in [alias.as_path(), other_alias.as_path(), workspace.path()] {
            let mut policy = FileSystemSandboxPolicy::read_only();
            policy.entries.push(FileSystemSandboxEntry::new(
                PathUri::from_host_native_path(root)?.into(),
                FileSystemAccessMode::Write,
            ));
            let sandbox = FileSystemSandboxContext::from_permission_profile(
                PermissionProfile::from_runtime_permissions(
                    &policy,
                    NetworkSandboxPolicy::Restricted,
                ),
                PathUri::from_host_native_path(workspace.path())?,
            );
            let result = environment
                .get_filesystem()
                .write_file(
                    &PathUri::from_host_native_path(root.join("output"))?,
                    b"written".to_vec(),
                    WriteFileOptions::default(),
                    Some(&sandbox),
                )
                .await;
            assert_eq!(
                result.is_ok(),
                root == workspace.path() || (enabled == Some(true) && root == alias),
                "root={root:?}, enabled={enabled:?}: {result:?}"
            );
            if let Err(error) = result {
                assert!(
                    error
                        .to_string()
                        .contains("symlinked writable roots are not supported"),
                    "{error}"
                );
            }
        }
        server.shutdown().await?;
    }
    assert_eq!(std::fs::read(target.path().join("output"))?, b"written");
    assert_eq!(std::fs::read(workspace.path().join("output"))?, b"written");
    Ok(())
}

#[cfg(unix)]
#[test_case(false, false, false, false, "bash"; "local_pipe")]
#[test_case(false, true, false, false, "bash"; "local_tty")]
#[test_case(true, false, false, false, "bash"; "remote_pipe")]
#[test_case(true, true, false, false, "bash"; "remote_tty")]
#[test_case(true, false, true, false, "bash"; "remote_sandbox")]
#[test_case(false, false, false, false, "sh"; "local_sh_pipe")]
#[test_case(false, false, false, false, "bash-sh"; "local_bash_backed_sh")]
#[test_case(false, false, false, true, "bash"; "local_bash_env")]
#[test_case(true, false, false, true, "bash"; "remote_bash_env")]
#[cfg_attr(
    target_os = "macos",
    test_case(false, false, false, false, "zsh"; "local_zsh_pipe")
)]
#[cfg_attr(
    target_os = "macos",
    test_case(false, false, false, true, "zsh"; "local_zshenv")
)]
#[cfg_attr(
    target_os = "macos",
    test_case(true, false, false, true, "zsh"; "remote_zshenv")
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn shell_snapshot_v2_filters_profile_exports_and_stays_in_memory(
    use_remote: bool,
    tty: bool,
    use_sandbox: bool,
    automatic_startup: bool,
    shell_name: &str,
) -> Result<()> {
    if use_sandbox
        && let Some(warning) = codex_sandboxing::system_bwrap_warning(
            &PermissionProfile::read_only(),
            &std::env::current_dir()?,
        )
    {
        eprintln!("skipping sandbox test: {warning}");
        return Ok(());
    }
    let context = create_process_context(use_remote).await?;
    let home = TempDir::new()?;
    let cwd = PathUri::from_host_native_path(home.path())?;
    let (shell_path, profile_name) = match shell_name {
        "bash" if automatic_startup => ("/bin/bash", ".bash-env"),
        "bash" => ("/bin/bash", ".bashrc"),
        "sh" => ("/bin/sh", ".snapshot-env"),
        "bash-sh" => ("/bin/bash", ".snapshot-env"),
        "zsh" if automatic_startup => ("/bin/zsh", ".zshenv"),
        "zsh" => ("/bin/zsh", ".zshrc"),
        name => anyhow::bail!("unsupported test shell {name}"),
    };
    let profile_path = home.path().join(profile_name);
    let profile_path_entry = home.path().join("profile-bin");
    let runtime_path_entry = home.path().join("runtime-bin");
    std::fs::create_dir(&profile_path_entry)?;
    let wc = profile_path_entry.join("wc");
    codex_utils_cargo_bin::write_executable(
        &wc,
        "#!/bin/sh\nprintf x >> \"$HOME/tool-captures\"\nexec /usr/bin/wc \"$@\"\n",
    )?;
    let posix_shell = matches!(shell_name, "sh" | "bash-sh");
    let padding = if !use_remote && !tty && shell_name == "bash" {
        format!(
            "snapshot_padding() {{ printf '%s' '{}'; }}\n",
            "🦀".repeat(200_000)
        )
    } else {
        String::new()
    };
    let shadowed_builtins = if posix_shell {
        ""
    } else {
        "unset() { exit 41; }\nbuiltin() { exit 41; }\nexec() { exit 41; }\n"
    };
    std::fs::write(
        &profile_path,
        format!(
            "printf x >> \"$HOME/captures\"\nexport PATH=\"$HOME/profile-bin:/usr/bin:/bin\"\nexport PROFILE_ALLOWED=profile\nexport PROFILE_SECRET=secret\nexport PROFILE_DENIED=denied\nprofile_helper() {{ printf helper; }}\nif [ -n \"${{BASH_VERSION-}}\" ]; then\n  shopt -s extglob nocasematch\n  eval 'profile_helper() {{ case $1 in @(foo|bar)*) printf helper ;; *) return 1 ;; esac; }}'\nfi\nset -u\n{shadowed_builtins}{padding}"
        ),
    )?;
    if shell_name == "zsh" && automatic_startup {
        std::fs::write(
            home.path().join(".zshrc"),
            "export PATH=\"$HOME/profile-bin:/usr/bin:/bin\"\n",
        )?;
    }
    let mut configured_environment = HashMap::from([(
        "HOME".to_string(),
        home.path().to_string_lossy().into_owned(),
    )]);
    if posix_shell {
        configured_environment.insert(
            "ENV".to_string(),
            "${XDG_CONFIG_HOME:-$HOME}/.snapshot-env".to_string(),
        );
        // Keep coverage for large values alongside the many-small-entry case below.
        for index in 0..3 {
            configured_environment.insert(format!("PROFILE_SDK_{index}"), "x".repeat(60 * 1024));
        }
    }
    if shell_name == "bash" && automatic_startup {
        configured_environment.insert(
            "BASH_ENV".to_string(),
            profile_path.to_string_lossy().into_owned(),
        );
    }
    // Many small entries exercise capture overhead separately from the byte limit above.
    let many_entries = !use_remote && !tty && !automatic_startup;
    if many_entries {
        configured_environment.extend(
            (0..1_000).map(|index| (format!("PROFILE_ENTRY_{index}"), format!("value-{index}"))),
        );
    }
    let policy = ExecEnvPolicy {
        inherit: ShellEnvironmentPolicyInherit::All,
        ignore_default_excludes: false,
        exclude: vec!["PROFILE_DENIED".to_string()],
        r#set: configured_environment,
        include_only: vec![
            "BASH_ENV".to_string(),
            "ENV".to_string(),
            "HOME".to_string(),
            "PATH".to_string(),
            "PROFILE_*".to_string(),
        ],
    };
    let (command_prefix, expected_prefix) = if shell_name == "sh" {
        ("", "")
    } else {
        ("profile_helper FOObar; ", "helper")
    };
    let entry_check = if many_entries {
        "[ \"${PROFILE_ENTRY_999-missing}\" = value-999 ] || exit 43; "
    } else {
        ""
    };
    let command = format!(
        "case $- in *u*) ;; *) exit 42 ;; esac; {entry_check}export PATH='{}':\"$PATH\"; {command_prefix}printf '|%s|%s|%s|%s|%s|%s' \"$PROFILE_ALLOWED\" \"${{PROFILE_SECRET-missing}}\" \"${{PROFILE_DENIED-missing}}\" \"$PATH\" \"${{__CODEX_SHELL_SNAPSHOT_STATE_0-missing}}\" \"${{__CODEX_SHELL_SNAPSHOT_STATE_1-missing}}\"",
        runtime_path_entry.display(),
    );
    let expected_stdout = format!(
        "{expected_prefix}|profile|missing|missing|{}:{}:/usr/bin:/bin|missing|missing",
        runtime_path_entry.display(),
        profile_path_entry.display(),
    );

    for attempt in 0..2 {
        let started = context
            .backend
            .start(ExecParams {
                metadata: Default::default(),
                process_id: ProcessId::from(format!("snapshot-{attempt}")),
                argv: vec![shell_path.to_string(), "-lc".to_string(), command.clone()],
                cwd: cwd.clone(),
                env_policy: Some(policy.clone()),
                shell_snapshot: Some(ShellSnapshotRequest {
                    scope_id: "attachment-1".to_string(),
                    shell: ShellInfo {
                        name: if posix_shell { "sh" } else { shell_name }.to_string(),
                        path: shell_path.to_string(),
                    },
                }),
                env: HashMap::new(),
                tty,
                pipe_stdin: false,
                arg0: (shell_name == "bash-sh").then(|| "sh".to_string()),
                sandbox: (use_sandbox && attempt == 0).then(|| {
                    FileSystemSandboxContext::from_permission_profile(
                        PermissionProfile::read_only(),
                        cwd.clone(),
                    )
                }),
                enforce_managed_network: false,
                managed_network: None,
                network_proxy: None,
            })
            .await?;
        let (stdout, stderr, status, closed) =
            collect_process_output_from_events(started.process).await?;
        assert_eq!(
            (stdout, stderr, status, closed),
            (expected_stdout.clone(), String::new(), Some(0), true,)
        );
    }

    assert_eq!(std::fs::read_to_string(home.path().join("captures"))?, "x");
    assert!(!std::fs::read(home.path().join("tool-captures"))?.is_empty());
    if let Some(server) = context._server {
        assert!(!server.codex_home().join("shell_snapshots").exists());
    }
    Ok(())
}

#[cfg(unix)]
#[test_case(false, false, false; "local_pipe")]
#[test_case(false, true, false; "local_tty")]
#[test_case(true, false, false; "remote_pipe")]
#[test_case(true, true, false; "remote_tty")]
#[test_case(true, false, true; "remote_sandbox")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(remote_exec_server)]
async fn exec_metadata_overrides_policy_and_cached_snapshot(
    use_remote: bool,
    tty: bool,
    use_sandbox: bool,
) -> Result<()> {
    if use_sandbox
        && let Some(warning) = codex_sandboxing::system_bwrap_warning(
            &PermissionProfile::workspace_write(),
            &std::env::current_dir()?,
        )
    {
        eprintln!("skipping sandbox test: {warning}");
        return Ok(());
    }
    let context = create_process_context(use_remote).await?;
    let home = TempDir::new()?;
    let cwd = PathUri::from_host_native_path(home.path())?;
    std::fs::write(
        home.path().join(".bashrc"),
        "printf '%s|%s\n' \"${CODEX_THREAD_ID-unset}\" \"${CODEX_TOOL_CALL_ID-unset}\" >> \"$HOME/capture-env\"\nexport CODEX_THREAD_ID=snapshot-thread\nexport CODEX_TOOL_CALL_ID=snapshot-call\n",
    )?;
    let policy = ExecEnvPolicy {
        inherit: ShellEnvironmentPolicyInherit::None,
        ignore_default_excludes: true,
        exclude: Vec::new(),
        r#set: HashMap::from([
            ("HOME".into(), home.path().to_string_lossy().into_owned()),
            ("PATH".into(), "/usr/bin:/bin".into()),
            ("CODEX_THREAD_ID".into(), "policy-thread".into()),
            ("CODEX_TOOL_CALL_ID".into(), "policy-call".into()),
        ]),
        include_only: Vec::new(),
    };
    let thread_id = codex_protocol::ThreadId::from_string("019d25f0-6728-7ce2-908f-c3c187323e5b")?;
    let later_thread_id =
        codex_protocol::ThreadId::from_string("019d25f0-6728-7ce2-908f-c3c187323e5c")?;

    for (attempt, call_id, expected) in [
        (0, Some(""), ""),
        (1, Some("call-λ-'\";$()"), "call-λ-'\";$()"),
        (2, None, "unset"),
        (3, Some("exec-invalid\0call"), "unset"),
    ] {
        let execution_thread_id = if attempt == 1 {
            later_thread_id
        } else {
            thread_id
        };
        let started = context
            .backend
            .start(ExecParams {
                metadata: Some(ExecMetadata {
                    thread_id: Some(execution_thread_id),
                    tool_call_id: call_id.map(str::to_string),
                }),
                process_id: ProcessId::from(format!("metadata-snapshot-{attempt}")),
                argv: vec![
                    "/bin/bash".into(),
                    "-lc".into(),
                    "printf '%s|%s' \"$CODEX_THREAD_ID\" \"${CODEX_TOOL_CALL_ID-unset}\""
                        .to_string(),
                ],
                cwd: cwd.clone(),
                env_policy: Some(policy.clone()),
                shell_snapshot: Some(ShellSnapshotRequest {
                    scope_id: "metadata-attachment".into(),
                    shell: ShellInfo {
                        name: "bash".into(),
                        path: "/bin/bash".into(),
                    },
                }),
                env: HashMap::from([
                    ("CODEX_THREAD_ID".into(), "overlay-thread".into()),
                    ("CODEX_TOOL_CALL_ID".into(), "overlay-call".into()),
                ]),
                tty,
                pipe_stdin: false,
                arg0: None,
                sandbox: use_sandbox.then(|| {
                    FileSystemSandboxContext::from_permission_profile(
                        PermissionProfile::workspace_write(),
                        cwd.clone(),
                    )
                }),
                enforce_managed_network: false,
                managed_network: None,
                network_proxy: None,
            })
            .await?;
        let (stdout, stderr, status, closed) =
            collect_process_output_from_events(started.process).await?;
        assert_eq!(
            (stdout, stderr, status, closed),
            (
                format!("{execution_thread_id}|{expected}"),
                String::new(),
                Some(0),
                true,
            )
        );
    }

    assert_eq!(
        std::fs::read_to_string(home.path().join("capture-env"))?,
        "019d25f0-6728-7ce2-908f-c3c187323e5b|unset\n"
    );
    Ok(())
}

#[cfg(unix)]
#[derive(Clone, Copy)]
enum CredentialStartup {
    Inherited,
    Profile,
    UnsupportedProfile,
    OrdinaryLogin,
}

#[cfg(unix)]
#[test_case(false, "bash", &["-lc", "-lc"], CredentialStartup::Inherited; "unmasked")]
#[cfg_attr(any(target_os = "linux", target_os = "macos"), test_case(true, "bash", &["-c", "-c", "-lc"], CredentialStartup::Inherited; "masked_bash"))]
#[cfg_attr(any(target_os = "linux", target_os = "macos"), test_case(true, "bash", &["-lc", "-lc"], CredentialStartup::Inherited; "bash_profile_fallback"))]
#[cfg_attr(any(target_os = "linux", target_os = "macos"), test_case(true, "zsh", &["-lc", "-lc"], CredentialStartup::OrdinaryLogin; "zsh_profile_gateway_fallback"))]
#[cfg_attr(any(target_os = "linux", target_os = "macos"), test_case(true, "zsh", &["-lc", "-lc"], CredentialStartup::Inherited; "masked_zsh_inherited_credentials"))]
#[cfg_attr(any(target_os = "linux", target_os = "macos"), test_case(true, "zsh", &["-lc", "-lc"], CredentialStartup::Profile; "masked_zsh_profile_credentials"))]
#[cfg_attr(any(target_os = "linux", target_os = "macos"), test_case(true, "zsh", &["-lc", "-lc"], CredentialStartup::UnsupportedProfile; "zsh_unsupported_profile_fallback"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(remote_exec_server)]
async fn shell_snapshot_v2_remote_managed_proxy_uses_prepared_execution_context(
    credential_masking: bool,
    shell_name: &str,
    shell_args: &[&str],
    credential_startup: CredentialStartup,
) -> Result<()> {
    const API_KEY: &str = "sk-proj-executor-only-0000000000000000000000000000000000";
    const PROFILE_GH_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const REQUEST_GH_TOKEN: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const VENDOR_TOKEN: &str = "vendor_abcdefgh";
    const CUSTOM_HOST: &str = "chatgpt.com";
    let configured_provider = shell_name == "bash" && shell_args.first() == Some(&"-c");
    let shell_path = format!("/bin/{shell_name}");
    if shell_name == "zsh" && !std::path::Path::new(&shell_path).exists() {
        eprintln!("skipping zsh test: {shell_path} is unavailable");
        return Ok(());
    }
    let unset_destinations =
        shell_name == "zsh" && matches!(credential_startup, CredentialStartup::Inherited);
    let api_host = if shell_name == "zsh" && !unset_destinations {
        CUSTOM_HOST
    } else {
        "api.openai.com"
    };
    let home = TempDir::new()?;
    if credential_masking
        && let Some(warning) = codex_sandboxing::system_bwrap_warning(
            &PermissionProfile::workspace_write(),
            home.path(),
        )
    {
        eprintln!("skipping sandbox test: {warning}");
        return Ok(());
    }
    let upstream = credential_masking
        .then(|| {
            tls_proxy::TlsInterceptingProxy::start(
                shell_args.len() * if unset_destinations { 2 } else { 1 },
            )
        })
        .transpose()?;
    let mut executor_env = vec![("OPENAI_API_KEY", API_KEY.to_string())];
    if configured_provider {
        executor_env.extend([
            ("VENDOR_TOKEN", VENDOR_TOKEN.to_string()),
            ("VENDOR_BASE_URL", "https://api.openai.com/v1".to_string()),
        ]);
    }
    if let Some(upstream) = &upstream {
        let ca_path = home.path().join("upstream-ca.pem");
        std::fs::write(&ca_path, &upstream.ca_cert_pem)?;
        let inherited_api_host = if unset_destinations {
            CUSTOM_HOST
        } else {
            "api.openai.com"
        };
        executor_env.extend([
            ("AUTH_ALIAS", format!("Bearer {API_KEY}")),
            (
                "OPENAI_BASE_URL",
                format!("https://{inherited_api_host}/v1"),
            ),
            (
                "GH_HOST",
                if unset_destinations { CUSTOM_HOST } else { "" }.to_string(),
            ),
            (
                "GH_ENTERPRISE_TOKEN",
                "executor-unbound-enterprise-token".to_string(),
            ),
            (
                "CODEX_CA_CERTIFICATE",
                ca_path.to_string_lossy().into_owned(),
            ),
            ("CURL_CA_BUNDLE", ca_path.to_string_lossy().into_owned()),
            ("HTTPS_PROXY", upstream.url.clone()),
            ("https_proxy", upstream.url.clone()),
            ("ALL_PROXY", upstream.url.clone()),
            ("all_proxy", upstream.url.clone()),
            ("NO_PROXY", String::new()),
            ("no_proxy", String::new()),
        ]);
    }
    if matches!(credential_startup, CredentialStartup::Profile) {
        executor_env.extend([("GH_TOKEN", String::new()), ("GH_ALIAS", String::new())]);
        for (key, value) in &mut executor_env {
            if matches!(*key, "OPENAI_API_KEY" | "AUTH_ALIAS") {
                value.clear();
            }
        }
    }
    let mut server = common::exec_server::exec_server_with_env(executor_env, &[]).await?;
    let environment = Environment::create_for_tests(Some(server.websocket_url().to_string()))?;
    let backend = environment.get_exec_backend();
    let cwd = PathUri::from_host_native_path(home.path())?;
    let profile_bin = home.path().join("profile-bin");
    std::fs::create_dir(&profile_bin)?;
    codex_utils_cargo_bin::write_executable(
        &profile_bin.join("profile_command"),
        "#!/bin/sh\nprintf helper\n",
    )?;
    let shell_path = if shell_name == "zsh" {
        // Observe the launcher's proxy before replay and simulate a global zshenv export.
        let path = home.path().join("zsh");
        codex_utils_cargo_bin::write_executable(
            &path,
            "#!/bin/sh\nexport GITHUB_TOKEN=global-test-token\nexport TEST_LIVE_HTTP_PROXY=\"$HTTP_PROXY\"\nexec /bin/zsh \"$@\"\n",
        )?;
        path.to_string_lossy().into_owned()
    } else {
        shell_path
    };
    let profile = "printf '%s\\n' \"$HTTP_PROXY\" >> \"$HOME/captures\"\ntest \"$CODEX_NETWORK_PROXY_ACTIVE\" = 1 || exit 41\nexport PROFILE_ALLOWED=profile\nprofile_helper() { profile_command; }\n";
    std::fs::write(home.path().join(".bashrc"), profile)?;
    std::fs::write(
        home.path().join(".bash_profile"),
        "export PATH=\"$HOME/profile-bin:$PATH\"\n",
    )?;
    let profile_credentials = match credential_startup {
        CredentialStartup::Inherited | CredentialStartup::OrdinaryLogin => String::new(),
        CredentialStartup::Profile => format!(
            "export OPENAI_API_KEY={API_KEY}\nexport AUTH_ALIAS=\"Bearer $OPENAI_API_KEY\"\nexport GH_TOKEN={PROFILE_GH_TOKEN}\nexport GH_ALIAS=\"Bearer $GH_TOKEN\"\nfor index in {{1..256}}; do export \"AUTH_SIZE_$index=Bearer $OPENAI_API_KEY\"; done\nunset index\n"
        ),
        CredentialStartup::UnsupportedProfile => {
            format!("credential_literal() {{ printf '%s' '{API_KEY}'; }}\n")
        }
    };
    let profile_destinations = if unset_destinations {
        "unset OPENAI_BASE_URL GH_HOST\n".to_string()
    } else {
        format!("export OPENAI_BASE_URL=https://{api_host}/v1\n")
    };
    std::fs::write(
        home.path().join(".zshenv"),
        format!(
            "unset GITHUB_TOKEN\nexport PATH=\"$HOME/profile-bin:$PATH\"\n{profile_destinations}typeset -ux FOO=profile\nexport PROFILE_OVERRIDE=profile\nprofile_local=local\ncd \"$HOME/profile-bin\"\numask 027\n{profile_credentials}test \"$OPENAI_API_KEY\" = '{API_KEY}' || exit 48\n{profile}"
        ),
    )?;
    std::fs::write(home.path().join(".zshrc"), ": > \"$HOME/zshrc-loaded\"\n")?;
    let mut policy = ExecEnvPolicy {
        inherit: ShellEnvironmentPolicyInherit::All,
        ignore_default_excludes: true,
        exclude: Vec::new(),
        r#set: HashMap::from([
            (
                "HOME".to_string(),
                home.path().to_string_lossy().into_owned(),
            ),
            ("DEBUG_FLAG".to_string(), "debug".to_string()),
            ("PROFILE_OVERRIDE".to_string(), "policy".to_string()),
        ]),
        include_only: vec![
            "HOME".to_string(),
            "PATH".to_string(),
            "PROFILE_*".to_string(),
            "OPENAI_API_KEY".to_string(),
        ],
    };
    if credential_masking {
        policy.include_only.extend(
            [
                "OPENAI_BASE_URL",
                "AUTH_ALIAS",
                "GITHUB_TOKEN",
                "GH_HOST",
                "GH_ENTERPRISE_TOKEN",
                "CURL_CA_BUNDLE",
            ]
            .map(str::to_string),
        );
    }
    if matches!(credential_startup, CredentialStartup::Profile) {
        policy
            .include_only
            .extend(["GH_TOKEN", "GH_ALIAS", "AUTH_SIZE_*"].map(str::to_string));
    }
    if configured_provider {
        policy
            .include_only
            .extend(["VENDOR_TOKEN", "VENDOR_BASE_URL"].map(str::to_string));
    }
    let mut proxy_settings = NetworkProxyConfig {
        enabled: true,
        ..NetworkProxyConfig::default()
    };
    let mut allowed_domains = vec![api_host.to_string()];
    if unset_destinations {
        allowed_domains.push(CUSTOM_HOST.to_string());
    }
    proxy_settings.set_allowed_domains(allowed_domains);
    let mut proxy_config = RemoteNetworkProxyConfig::from_effective_config(&proxy_settings)?;
    let mut providers = BTreeMap::new();
    if configured_provider {
        providers.insert(
            "vendor".to_string(),
            codex_network_proxy::CredentialProviderConfig {
                env: vec!["VENDOR_TOKEN".to_string()],
                patterns: vec!["vendor_[a-z]{8}".to_string()],
                url_prefix_from_env: Some("VENDOR_BASE_URL".to_string()),
                auth: vec![codex_network_proxy::CredentialAuthMethod::Header],
                header: Some("X-Vendor-Token".to_string()),
                ..Default::default()
            },
        );
    }
    if credential_masking {
        proxy_config.enable_credential_masking(providers);
    }
    let mut saved_api_key = None;

    for (attempt, &shell_args) in shell_args.iter().enumerate() {
        let use_snapshot =
            shell_args == "-lc" && !matches!(credential_startup, CredentialStartup::OrdinaryLogin);
        let expect_snapshot_fallback =
            matches!(credential_startup, CredentialStartup::UnsupportedProfile)
                || matches!(credential_startup, CredentialStartup::Profile) && attempt == 1;
        let expect_masking = credential_masking
            && (shell_name == "bash" && !use_snapshot
                || shell_name == "zsh" && use_snapshot && !expect_snapshot_fallback);
        let expect_profile = use_snapshot || shell_name == "zsh";
        let request_foo = if attempt == 0 { "request" } else { "" };
        let mut env = HashMap::from([("FOO".to_string(), request_foo.to_string())]);
        if shell_name == "zsh" && expect_masking {
            env.insert("HTTP_PROXY".to_string(), "http://127.0.0.1:1".to_string());
        }
        if matches!(credential_startup, CredentialStartup::Profile) {
            let (key, value) = if attempt == 0 {
                ("GH_TOKEN", REQUEST_GH_TOKEN)
            } else {
                ("OPENAI_API_KEY", "")
            };
            env.insert(key.to_string(), value.to_string());
        }
        let command = if expect_profile {
            "profile_helper; printf '|%s|%s|%s' \"$PROFILE_ALLOWED\" \"$CODEX_NETWORK_PROXY_ACTIVE\" \"$HTTP_PROXY\""
        } else {
            "printf 'masked|%s|%s' \"$CODEX_NETWORK_PROXY_ACTIVE\" \"$HTTP_PROXY\""
        };
        let command = if shell_name == "zsh" && expect_masking {
            format!(
                "test \"$HTTP_PROXY\" = \"$TEST_LIVE_HTTP_PROXY\" && test \"$HTTPS_PROXY\" = \"$TEST_LIVE_HTTP_PROXY\" || exit 51; test \"${{DEBUG_FLAG+x}}\" = '' && test \"${{GITHUB_TOKEN+x}}\" = '' && test \"$PROFILE_OVERRIDE\" = policy && test \"${{FOO-unset}}\" = '{request_foo}' && test -e \"$HOME/zshrc-loaded\" || exit 47; {command}"
            )
        } else if matches!(credential_startup, CredentialStartup::OrdinaryLogin) {
            format!(
                "test \"$profile_local\" = local && test \"$PWD\" = \"$HOME/profile-bin\" && test \"$(umask -S)\" = 'u=rwx,g=rx,o=' && test ! -e \"$HOME/zshrc-loaded\" || exit 49; {command}"
            )
        } else {
            command.to_string()
        };
        let command = if matches!(credential_startup, CredentialStartup::Profile) && attempt == 0 {
            format!(
                "test -n \"$GH_TOKEN\" && test \"$GH_ALIAS\" = \"Bearer $GH_TOKEN\" && test \"$AUTH_SIZE_1\" = \"Bearer $OPENAI_API_KEY\" && test \"$AUTH_SIZE_256\" = \"Bearer $OPENAI_API_KEY\" || exit 50; case \"$GH_TOKEN|$GH_ALIAS\" in *'{PROFILE_GH_TOKEN}'*|*'{REQUEST_GH_TOKEN}'*) exit 50 ;; esac; {command}"
            )
        } else {
            command
        };
        let command = if credential_masking {
            let vendor_header = if configured_provider {
                "--header \"X-Vendor-Token: $(<\"$HOME/vendor-token\")\""
            } else {
                ""
            };
            format!(
                "{command}; test \"$GH_ENTERPRISE_TOKEN\" = executor-unbound-enterprise-token || exit 45; test \"$AUTH_ALIAS\" = \"Bearer $OPENAI_API_KEY\" || exit 42; printf '|%s' \"$OPENAI_API_KEY\"; curl --silent --show-error --fail --max-time 5 --noproxy '' --config \"$HOME/curl.conf\" {vendor_header} \"${{OPENAI_BASE_URL:-https://api.openai.com/v1}}/models\" >/dev/null"
            )
        } else {
            format!("{command}; test \"$OPENAI_API_KEY\" = '{API_KEY}' || exit 46")
        };
        let command = if unset_destinations {
            format!(
                "test \"${{OPENAI_BASE_URL+x}}\" = '' && test \"${{GH_HOST+x}}\" = '' || exit 52; {command} && curl --silent --show-error --fail --max-time 5 --noproxy '' --config \"$HOME/curl.conf\" https://{CUSTOM_HOST}/v1/models >/dev/null"
            )
        } else {
            command
        };
        let command = if credential_masking && attempt == 0 {
            format!(
                "printf 'header = \"Authorization: %s\"\\n' \"$AUTH_ALIAS\" > \"$HOME/curl.conf\" || exit 44; {command}"
            )
        } else {
            command
        };
        let command = if configured_provider && attempt == 0 {
            format!("printf '%s' \"$VENDOR_TOKEN\" > \"$HOME/vendor-token\"; {command}")
        } else {
            command
        };
        let started = timeout(
            Duration::from_secs(30),
            backend.start(ExecParams {
                metadata: Default::default(),
                process_id: ProcessId::from(format!("managed-snapshot-{attempt}")),
                argv: vec![shell_path.clone(), shell_args.to_string(), command],
                cwd: cwd.clone(),
                env_policy: Some(policy.clone()),
                shell_snapshot: use_snapshot.then(|| ShellSnapshotRequest {
                    scope_id: "managed-attachment".to_string(),
                    shell: ShellInfo {
                        name: shell_name.to_string(),
                        path: shell_path.clone(),
                    },
                }),
                env,
                tty: false,
                pipe_stdin: false,
                arg0: None,
                sandbox: credential_masking.then(|| {
                    FileSystemSandboxContext::from_permission_profile(
                        PermissionProfile::workspace_write(),
                        cwd.clone(),
                    )
                }),
                enforce_managed_network: true,
                managed_network: None,
                network_proxy: Some(
                    RemoteNetworkProxyLaunchConfig::new(proxy_config.clone()).for_execution(
                        "remote-environment".to_string(),
                        format!("managed-snapshot-{attempt}"),
                    ),
                ),
            }),
        )
        .await??;
        let (stdout, stderr, status, closed) =
            collect_process_output_from_events(started.process).await?;
        let expected_stderr = if expect_snapshot_fallback {
            "Credential masking is unavailable for this shell snapshot; running with unmasked credentials.\n"
        } else {
            ""
        };
        assert_eq!(
            (stderr.as_str(), status, closed),
            (expected_stderr, Some(0), true)
        );
        let prefix = if expect_profile {
            "helper|profile|1|"
        } else {
            "masked|1|"
        };
        let proxy_state = stdout
            .strip_prefix(prefix)
            .context("command should preserve live proxy state")?;
        let proxy_address = if let Some(upstream) = &upstream {
            let (proxy_address, child_api_key) = proxy_state
                .split_once('|')
                .context("command should expose the child API key")?;
            if expect_masking {
                assert!(child_api_key.starts_with("sk-proj-"));
                assert_ne!(child_api_key, API_KEY);
                assert!(!stdout.contains(API_KEY));
            } else {
                assert_eq!(child_api_key, API_KEY);
            }
            let saved_api_key = saved_api_key.get_or_insert_with(|| child_api_key.to_string());
            assert_eq!(
                std::fs::read_to_string(home.path().join("curl.conf"))?,
                format!("header = \"Authorization: Bearer {saved_api_key}\"\n")
            );
            let request = upstream.next_request()?;
            if configured_provider {
                let placeholder = std::fs::read_to_string(home.path().join("vendor-token"))?;
                assert!(placeholder.starts_with("vendor_"));
                assert_eq!(placeholder.len(), VENDOR_TOKEN.len());
                assert_ne!(placeholder, VENDOR_TOKEN);
                assert_eq!(
                    request.headers.get("x-vendor-token"),
                    Some(&vec![VENDOR_TOKEN.to_string()])
                );
            }
            assert_eq!(
                (
                    request.connect_authority.as_str(),
                    request.request_line.as_str(),
                    request.headers.get("authorization"),
                ),
                (
                    format!("{api_host}:443").as_str(),
                    "GET /v1/models HTTP/1.1",
                    Some(&vec![format!("Bearer {API_KEY}")]),
                )
            );
            if unset_destinations {
                let request = upstream.next_request()?;
                assert_eq!(
                    (
                        request.connect_authority.as_str(),
                        request.request_line.as_str(),
                        request.headers.get("authorization"),
                    ),
                    (
                        format!("{CUSTOM_HOST}:443").as_str(),
                        "GET /v1/models HTTP/1.1",
                        Some(&vec![format!("Bearer {saved_api_key}")]),
                    )
                );
            }
            proxy_address
        } else {
            proxy_state
        };
        assert!(proxy_address.starts_with("http://127.0.0.1:"));
    }

    let expected_capture_count =
        if matches!(credential_startup, CredentialStartup::UnsupportedProfile) {
            Some(shell_args.len() * 2)
        } else if matches!(credential_startup, CredentialStartup::Profile) {
            Some(shell_args.len() + 1)
        } else if shell_name == "zsh" {
            Some(shell_args.len())
        } else {
            shell_args.iter().position(|flag| *flag == "-lc").map(|_| 1)
        };
    if let Some(expected_capture_count) = expected_capture_count {
        // Capture and replay can have different proxy ports in separate Linux namespaces.
        let captures = std::fs::read_to_string(home.path().join("captures"))?;
        assert_eq!(captures.lines().count(), expected_capture_count);
        assert!(
            captures
                .lines()
                .all(|address| address.starts_with("http://127.0.0.1:"))
        );
    } else {
        assert!(!home.path().join("captures").exists());
    }
    if let Some(upstream) = upstream {
        upstream.finish()?;
    }
    server.shutdown().await?;
    Ok(())
}

#[cfg(unix)]
#[derive(Clone, Copy)]
enum CaptureFailure {
    Exit(i32),
    Cancel,
    Timeout,
}

#[cfg(unix)]
#[test_case(false, false, "bash", 1, CaptureFailure::Exit(7); "local_pipe_recovery")]
#[test_case(false, true, "bash", 1, CaptureFailure::Exit(7); "local_tty_recovery")]
#[test_case(true, false, "bash", 1, CaptureFailure::Exit(7); "remote_pipe_recovery")]
#[test_case(true, true, "bash", 1, CaptureFailure::Exit(7); "remote_tty_recovery")]
#[test_case(false, false, "bash", 3, CaptureFailure::Exit(7); "local_retry_budget_exhausted")]
#[test_case(true, false, "bash", 3, CaptureFailure::Exit(7); "remote_retry_budget_exhausted")]
#[test_case(false, false, "bash", 1, CaptureFailure::Exit(0); "local_invalid_output_recovery")]
#[cfg_attr(target_os = "macos", test_case(false, false, "zsh", 1, CaptureFailure::Exit(0); "local_zsh_invalid_output_recovery"))]
#[cfg_attr(target_os = "macos", test_case(false, false, "zsh", 1, CaptureFailure::Exit(7); "local_zsh_recovery"))]
#[cfg_attr(target_os = "macos", test_case(true, false, "zsh", 1, CaptureFailure::Exit(7); "remote_zsh_recovery"))]
#[test_case(false, false, "bash", 1, CaptureFailure::Cancel; "cancel_capture")]
#[test_case(false, false, "bash", 1, CaptureFailure::Timeout; "timeout_capture")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(remote_exec_server)]
async fn shell_snapshot_v2_capture_failure_falls_back_and_retries(
    use_remote: bool,
    tty: bool,
    shell_name: &str,
    failures_before_repair: usize,
    failure: CaptureFailure,
) -> Result<()> {
    if use_remote
        && let Some(warning) = codex_sandboxing::system_bwrap_warning(
            &PermissionProfile::workspace_write(),
            &std::env::current_dir()?,
        )
    {
        eprintln!("skipping sandbox test: {warning}");
        return Ok(());
    }
    let context = create_process_context(use_remote).await?;
    let home = TempDir::new()?;
    let cwd = PathUri::from_host_native_path(home.path())?;
    let (shell_path, profile_name) = match shell_name {
        "bash" => ("/bin/bash", ".bashrc"),
        "zsh" => ("/bin/zsh", ".zshrc"),
        name => anyhow::bail!("unsupported test shell {name}"),
    };
    let child_pids = home.path().join("startup-pids");
    let _cleanup = shell_snapshot::StartupProcesses(child_pids.clone());
    // Sandboxed remote PIDs may be namespace-local; check descendants on the native path.
    let startup_child = if use_remote {
        ""
    } else {
        "/bin/sleep 30 >/dev/null 2>&1 &\nprintf '%s\\n' \"$!\" >> \"$HOME/startup-pids\"\n"
    };
    let finish = match failure {
        CaptureFailure::Exit(code) => format!("exit {code}"),
        CaptureFailure::Cancel | CaptureFailure::Timeout => "wait".to_string(),
    };
    std::fs::write(
        home.path().join(profile_name),
        format!("printf x >> \"$HOME/captures\"\n{startup_child}{finish}\n"),
    )?;
    let policy = ExecEnvPolicy {
        inherit: ShellEnvironmentPolicyInherit::All,
        ignore_default_excludes: false,
        exclude: Vec::new(),
        r#set: HashMap::from([(
            "HOME".to_string(),
            home.path().to_string_lossy().into_owned(),
        )]),
        include_only: vec!["HOME".to_string(), "PATH".to_string()],
    };
    let mut params = ExecParams {
        metadata: Default::default(),
        process_id: ProcessId::from("snapshot-first"),
        argv: vec![
            shell_path.to_string(),
            "-lc".to_string(),
            "if command -v profile_helper >/dev/null; then profile_helper; else printf original; fi".to_string(),
        ],
        cwd: cwd.clone(),
        env_policy: Some(policy),
        shell_snapshot: Some(ShellSnapshotRequest {
            scope_id: "attachment-1".to_string(),
            shell: ShellInfo {
                name: shell_name.to_string(),
                path: shell_path.to_string(),
            },
        }),
        env: HashMap::new(),
        tty,
        pipe_stdin: false,
        arg0: None,
        sandbox: use_remote.then(|| {
            FileSystemSandboxContext::from_permission_profile(
                PermissionProfile::workspace_write(),
                cwd,
            )
        }),
        enforce_managed_network: false,
        managed_network: None,
        network_proxy: None,
    };

    for attempt in 0..failures_before_repair {
        params.process_id = ProcessId::from(format!("snapshot-fallback-{attempt}"));
        let mut start = context.backend.start(params.clone());
        if matches!(failure, CaptureFailure::Cancel) {
            tokio::select! {
                result = &mut start => anyhow::bail!("capture finished before cancellation: {}", result.is_ok()),
                _ = async {
                    while std::fs::read_to_string(&child_pids).unwrap_or_default().trim().is_empty() {
                        sleep(Duration::from_millis(20)).await;
                    }
                } => {}
            }
            drop(start);
        } else {
            let fallback = start.await?;
            assert_eq!(
                collect_process_output_from_events(fallback.process).await?,
                ("original".to_string(), String::new(), Some(0), true)
            );
        }
        if !use_remote {
            let pids = std::fs::read_to_string(&child_pids)?;
            let pid = pids
                .split_whitespace()
                .last()
                .context("startup child PID")?;
            timeout(Duration::from_secs(5), async {
                loop {
                    let output = std::process::Command::new("/bin/ps")
                        .args(["-o", "stat=", "-p", pid])
                        .output()?;
                    let state = String::from_utf8(output.stdout)?;
                    if state.trim().is_empty() || state.trim().starts_with('Z') {
                        // Forget the completed child before retries can reuse its PID.
                        std::fs::remove_file(&child_pids)?;
                        return Ok::<_, anyhow::Error>(());
                    }
                    sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .context("failed capture left its startup child running")??;
        }
        // A real remote executor has its own clock; the unit test uses a
        // paused clock to check requests made during the one-second backoff.
        sleep(Duration::from_millis(1100)).await;
    }
    assert_eq!(
        std::fs::read_to_string(home.path().join("captures"))?,
        "x".repeat(failures_before_repair)
    );

    std::fs::write(
        home.path().join(profile_name),
        "printf x >> \"$HOME/captures\"\nprofile_helper() { printf recovered; }\n",
    )?;
    let (expected_output, expected_captures) = if failures_before_repair == 3 {
        ("original", "xxx")
    } else {
        ("recovered", "xx")
    };
    for attempt in 0..2 {
        params.process_id = ProcessId::from(format!("snapshot-after-repair-{attempt}"));
        let started = context.backend.start(params.clone()).await?;
        assert_eq!(
            collect_process_output_from_events(started.process).await?,
            (expected_output.to_string(), String::new(), Some(0), true)
        );
    }
    assert_eq!(
        std::fs::read_to_string(home.path().join("captures"))?,
        expected_captures
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_sandboxed_process_preserves_custom_arg0() -> Result<()> {
    if let Some(warning) = codex_sandboxing::system_bwrap_warning(
        &PermissionProfile::read_only(),
        &std::env::current_dir()?,
    ) {
        eprintln!("skipping bwrap test: {warning}");
        return Ok(());
    }

    let context = create_process_context(/*use_remote*/ true).await?;
    let workspace = TempDir::new()?;
    let outside_workspace = TempDir::new()?;
    let denied_file = outside_workspace.path().join("denied.txt");
    std::fs::write(&denied_file, b"denied")?;
    let cwd = PathUri::from_host_native_path(workspace.path())?;
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
    ]);
    let sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        cwd.clone(),
    );
    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: ProcessId::from("proc-custom-arg0"),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf '%s' \"$0\"; if /bin/cat \"$CODEX_TEST_DENIED_FILE\" >/dev/null 2>&1; then exit 42; fi"
                    .to_string(),
            ],
            cwd,
            shell_snapshot: None,
            env_policy: None,
            env: HashMap::from([
                ("PATH".to_string(), std::env::var("PATH")?),
                (
                    "CODEX_TEST_DENIED_FILE".to_string(),
                    denied_file.to_string_lossy().into_owned(),
                ),
            ]),
            tty: false,
            pipe_stdin: false,
            arg0: Some("custom-arg0".to_string()),
            sandbox: Some(sandbox),
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    let output = collect_process_output_from_events(session.process).await?;

    assert_eq!(
        output,
        ("custom-arg0".to_string(), String::new(), Some(0), true)
    );
    Ok(())
}

async fn assert_exec_process_starts_and_exits(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let thread_id = codex_protocol::ThreadId::from_string("019d25f0-6728-7ce2-908f-c3c187323e5b")?;
    let call_id = "call-λ-'\";$()";
    #[cfg(unix)]
    let argv = vec![
        "/bin/sh".into(),
        "-c".into(),
        "printf '%s|%s|%s' \"$CODEX_THREAD_ID\" \"$CODEX_TOOL_CALL_ID\" \"$CUSTOM_ENV_VAR\""
            .to_string(),
    ];
    #[cfg(windows)]
    let argv = vec![
        "powershell.exe".into(),
        "-NoProfile".into(),
        "-Command".into(),
        "[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false); [Console]::Write($env:CODEX_THREAD_ID + '|' + $env:CODEX_TOOL_CALL_ID + '|' + $env:CUSTOM_ENV_VAR)".into(),
    ];
    let mut env = std::env::vars().collect::<HashMap<_, _>>();
    env.extend([
        ("CODEX_THREAD_ID".into(), "stale-thread".into()),
        ("Codex_Thread_Id".into(), "mixed-case-thread".into()),
        ("CODEX_TOOL_CALL_ID".into(), "stale-call".into()),
        ("Codex_Tool_Call_Id".into(), "mixed-case-call".into()),
        ("CUSTOM_ENV_VAR".into(), "caller-owned".into()),
    ]);
    let session = context
        .backend
        .start(ExecParams {
            metadata: Some(ExecMetadata {
                thread_id: Some(thread_id),
                tool_call_id: Some(call_id.to_string()),
            }),
            process_id: ProcessId::from("proc-1"),
            argv,
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env,
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), "proc-1");
    let wake_rx = session.process.subscribe_wake();
    let (stdout, stderr, status, closed) =
        collect_process_output_from_events(Arc::clone(&session.process)).await?;
    assert_eq!(
        (stdout, stderr, status, closed),
        (
            format!("{thread_id}|{call_id}|caller-owned"),
            String::new(),
            Some(0),
            true,
        )
    );
    let (_, exit_code, closed) =
        collect_process_output_from_reads(session.process, wake_rx).await?;

    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_process_keeps_sandbox_helper_visible_with_restricted_reads() -> Result<()> {
    if let Some(warning) = codex_sandboxing::system_bwrap_warning(
        &PermissionProfile::read_only(),
        &std::env::current_dir()?,
    ) {
        eprintln!("skipping bwrap test: {warning}");
        return Ok(());
    }

    let context = create_process_context(/*use_remote*/ true).await?;
    let workspace = TempDir::new()?;
    let file = workspace.path().join("allowed.txt");
    std::fs::write(&file, b"allowed")?;
    let cwd = PathUri::from_host_native_path(workspace.path())?;
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
    ]);
    let sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        cwd.clone(),
    );

    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: ProcessId::from("proc-restricted-helper"),
            argv: vec!["/bin/cat".to_string(), file.to_string_lossy().into_owned()],
            cwd,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: HashMap::from([("PATH".to_string(), std::env::var("PATH")?)]),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: Some(sandbox),
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    let output = collect_process_output_from_events(session.process).await?;

    assert_eq!(
        output,
        ("allowed".to_string(), String::new(), Some(0), true)
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_tty_process_uses_configured_sandbox_helper_with_hostile_path() -> Result<()> {
    if let Some(warning) = codex_sandboxing::system_bwrap_warning(
        &PermissionProfile::read_only(),
        &std::env::current_dir()?,
    ) {
        eprintln!("skipping bwrap test: {warning}");
        return Ok(());
    }

    let context = create_process_context(/*use_remote*/ true).await?;
    let workspace = TempDir::new()?;
    let file = workspace.path().join("allowed.txt");
    std::fs::write(&file, b"allowed")?;
    let hostile_helper = workspace.path().join("codex-linux-sandbox");
    codex_utils_cargo_bin::write_executable(&hostile_helper, "#!/bin/sh\nprintf hostile")?;
    let path = std::env::var_os("PATH").context("PATH is not set")?;
    let hostile_path = std::env::join_paths(
        std::iter::once(workspace.path().to_path_buf()).chain(std::env::split_paths(&path)),
    )?;
    let cwd = PathUri::from_host_native_path(workspace.path())?;
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
    ]);
    let sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        cwd.clone(),
    );

    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: ProcessId::from("proc-hostile-helper-path"),
            argv: vec!["/bin/cat".to_string(), file.to_string_lossy().into_owned()],
            cwd,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: HashMap::from([(
                "PATH".to_string(),
                hostile_path.to_string_lossy().into_owned(),
            )]),
            tty: true,
            pipe_stdin: false,
            arg0: None,
            sandbox: Some(sandbox),
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    let output = collect_process_output_from_events(session.process).await?;

    assert_eq!(
        output,
        ("allowed".to_string(), String::new(), Some(0), true)
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_process_preserves_empty_workspace_roots() -> Result<()> {
    if let Some(warning) = codex_sandboxing::system_bwrap_warning(
        &PermissionProfile::read_only(),
        &std::env::current_dir()?,
    ) {
        eprintln!("skipping bwrap test: {warning}");
        return Ok(());
    }

    let context = create_process_context(/*use_remote*/ true).await?;
    let tmp = TempDir::new()?;
    let file = tmp.path().join("excluded.txt");
    std::fs::write(&file, b"excluded")?;
    let cwd = PathUri::from_host_native_path(tmp.path())?;
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
        },
        access: FileSystemAccessMode::Read,
        missing_path_behavior: None,
    }]);
    let mut sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        cwd.clone(),
    );
    sandbox.workspace_roots.clear();

    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: ProcessId::from("proc-empty-workspace-roots"),
            argv: vec!["/bin/cat".to_string(), file.to_string_lossy().into_owned()],
            cwd,
            shell_snapshot: None,
            env_policy: None,
            env: HashMap::new(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: Some(sandbox),
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    let (stdout, _stderr, exit_code, closed) =
        collect_process_output_from_events(session.process).await?;

    assert!(!stdout.contains("excluded"), "unexpected stdout: {stdout}");
    assert_ne!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn read_process_until_change(
    session: Arc<dyn ExecProcess>,
    wake_rx: &mut watch::Receiver<u64>,
    after_seq: Option<u64>,
) -> Result<ReadResponse> {
    let response = session
        .read(after_seq, /*max_bytes*/ None, /*wait_ms*/ Some(0))
        .await?;
    if !response.chunks.is_empty() || response.closed || response.failure.is_some() {
        return Ok(response);
    }

    timeout(Duration::from_secs(2), wake_rx.changed()).await??;
    session
        .read(after_seq, /*max_bytes*/ None, /*wait_ms*/ Some(0))
        .await
        .map_err(Into::into)
}

async fn collect_process_output_from_reads(
    session: Arc<dyn ExecProcess>,
    mut wake_rx: watch::Receiver<u64>,
) -> Result<(String, Option<i32>, bool)> {
    let mut output = String::new();
    let mut exit_code = None;
    let mut after_seq = None;
    loop {
        let response =
            read_process_until_change(Arc::clone(&session), &mut wake_rx, after_seq).await?;
        if let Some(message) = response.failure {
            anyhow::bail!("process failed before closed state: {message}");
        }
        for chunk in response.chunks {
            output.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
            after_seq = Some(chunk.seq);
        }
        if response.exited {
            exit_code = response.exit_code;
        }
        if response.closed {
            break;
        }
        after_seq = response.next_seq.checked_sub(1).or(after_seq);
    }
    drop(session);
    Ok((output, exit_code, true))
}

async fn collect_process_output_from_events(
    session: Arc<dyn ExecProcess>,
) -> Result<(String, String, Option<i32>, bool)> {
    collect_process_output_from_events_with_timeout(session, Duration::from_secs(2)).await
}

async fn collect_process_output_from_events_with_timeout(
    session: Arc<dyn ExecProcess>,
    event_timeout: Duration,
) -> Result<(String, String, Option<i32>, bool)> {
    let mut events = session.subscribe_events();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = None;
    loop {
        match timeout(event_timeout, events.recv()).await?? {
            ExecProcessEvent::Output(chunk) => match chunk.stream {
                ExecOutputStream::Stdout | ExecOutputStream::Pty => {
                    stdout.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
                }
                ExecOutputStream::Stderr => {
                    stderr.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
                }
            },
            ExecProcessEvent::Exited {
                seq: _,
                exit_code: code,
                ..
            } => {
                exit_code = Some(code);
            }
            ExecProcessEvent::Closed { seq: _ } => {
                drop(session);
                return Ok((stdout, stderr, exit_code, true));
            }
            ExecProcessEvent::Failed(message) => {
                anyhow::bail!("process failed before closed state: {message}");
            }
        }
    }
}

async fn collect_process_event_snapshots(
    session: Arc<dyn ExecProcess>,
) -> Result<Vec<ProcessEventSnapshot>> {
    let mut events = session.subscribe_events();
    let mut snapshots = Vec::new();
    loop {
        let snapshot = match timeout(Duration::from_secs(2), events.recv()).await?? {
            ExecProcessEvent::Output(chunk) => ProcessEventSnapshot::Output {
                seq: chunk.seq,
                stream: chunk.stream,
                text: String::from_utf8_lossy(&chunk.chunk.into_inner()).into_owned(),
            },
            ExecProcessEvent::Exited { seq, exit_code, .. } => {
                ProcessEventSnapshot::Exited { seq, exit_code }
            }
            ExecProcessEvent::Closed { seq } => ProcessEventSnapshot::Closed { seq },
            ExecProcessEvent::Failed(message) => {
                anyhow::bail!("process failed before closed state: {message}");
            }
        };
        let closed = matches!(snapshot, ProcessEventSnapshot::Closed { .. });
        snapshots.push(snapshot);
        if closed {
            drop(session);
            return Ok(snapshots);
        }
    }
}

async fn assert_exec_process_streams_output(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stream".to_string();
    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 0.05; printf 'session output\\n'".to_string(),
            ],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;
    assert_eq!(output, "session output\n");
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_pushes_events(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-events".to_string();
    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'event output\\n'; sleep 0.1; printf 'event err\\n' >&2; sleep 0.1; exit 7".to_string(),
            ],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process, .. } = session;
    let actual = collect_process_event_snapshots(process).await?;
    assert_eq!(
        actual,
        vec![
            ProcessEventSnapshot::Output {
                seq: 1,
                stream: ExecOutputStream::Stdout,
                text: "event output\n".to_string(),
            },
            ProcessEventSnapshot::Output {
                seq: 2,
                stream: ExecOutputStream::Stderr,
                text: "event err\n".to_string(),
            },
            ProcessEventSnapshot::Exited {
                seq: 3,
                exit_code: 7,
            },
            ProcessEventSnapshot::Closed { seq: 4 },
        ]
    );
    Ok(())
}

async fn assert_exec_process_replays_events_after_close(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-events-late".to_string();
    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'late one\\n'; printf 'late two\\n'".to_string(),
            ],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    let read_result = collect_process_output_from_reads(Arc::clone(&process), wake_rx).await?;
    assert_eq!(
        read_result,
        ("late one\nlate two\n".to_string(), Some(0), true)
    );

    let event_result = collect_process_output_from_events(process).await?;
    assert_eq!(
        event_result,
        (
            "late one\nlate two\n".to_string(),
            String::new(),
            Some(0),
            true
        )
    );
    Ok(())
}

async fn assert_exec_process_retains_output_after_exit_until_streams_close(
    use_remote: bool,
) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let (helper_binary, _) = current_test_binary_helper_paths()?;
    let release_dir = TempDir::new()?;
    let release_path = release_dir.path().join("release-delayed-output");
    let process_id = "proc-output-after-exit".to_string();
    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: process_id.clone().into(),
            argv: vec![
                helper_binary.to_string_lossy().into_owned(),
                DELAYED_OUTPUT_AFTER_EXIT_PARENT_ARG.to_string(),
                release_path.to_string_lossy().into_owned(),
            ],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process, .. } = session;

    let exit_response = timeout(
        Duration::from_secs(2),
        process.read(
            /*after_seq*/ None,
            /*max_bytes*/ None,
            /*wait_ms*/ Some(2_000),
        ),
    )
    .await??;
    assert!(
        exit_response.chunks.is_empty(),
        "parent should exit before child writes delayed output"
    );
    assert_eq!(exit_response.exit_code, Some(0));
    assert!(!exit_response.closed);
    let exit_seq = exit_response
        .next_seq
        .checked_sub(1)
        .context("exit response should advance next_seq")?;
    std::fs::write(&release_path, b"go")?;

    let late_response = timeout(
        Duration::from_secs(2),
        process.read(
            /*after_seq*/ Some(exit_seq),
            /*max_bytes*/ None,
            /*wait_ms*/ Some(2_000),
        ),
    )
    .await??;
    let mut late_output = String::new();
    for chunk in late_response.chunks {
        assert_eq!(chunk.stream, ExecOutputStream::Stdout);
        late_output.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
    }
    assert_eq!(late_output, "late output after exit\n");

    let wake_rx = process.subscribe_wake();
    let actual = collect_process_output_from_reads(process, wake_rx).await?;
    assert_eq!(
        actual,
        ("late output after exit\n".to_string(), Some(0), true)
    );
    Ok(())
}

async fn assert_exec_process_write_then_read(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stdin".to_string();
    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: process_id.clone().into(),
            argv: vec![
                // Use `/bin/sh` instead of Python so this stdin round-trip test
                // stays portable across Bazel and non-macOS runners where
                // `/usr/bin/python3` is not guaranteed to exist.
                "/bin/sh".to_string(),
                "-c".to_string(),
                "IFS= read line; printf 'from-stdin:%s\\n' \"$line\"".to_string(),
            ],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: true,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    tokio::time::sleep(Duration::from_millis(200)).await;
    session.process.write(b"hello\n".to_vec()).await?;
    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;

    assert!(
        output.contains("from-stdin:hello"),
        "unexpected output: {output:?}"
    );
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_write_then_read_without_tty(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stdin-pipe".to_string();
    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "IFS= read line; printf 'from-stdin:%s\\n' \"$line\"".to_string(),
            ],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: true,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    tokio::time::sleep(Duration::from_millis(200)).await;
    let write_response = session.process.write(b"hello\n".to_vec()).await?;
    assert_eq!(write_response.status, WriteStatus::Accepted);
    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    let actual = collect_process_output_from_reads(process, wake_rx).await?;

    assert_eq!(actual, ("from-stdin:hello\n".to_string(), Some(0), true));
    Ok(())
}

async fn assert_remote_windows_sandbox_process_write(
    expected_sandbox_type: codex_sandboxing::SandboxType,
    tty: bool,
) -> Result<()> {
    if expected_sandbox_type == codex_sandboxing::SandboxType::WindowsMxc {
        crate::skip_if_mxc_unavailable!(Ok(()));
    }
    let context = create_process_context(/*use_remote*/ true).await?;
    let workspace = TempDir::new()?;
    let blocked_file = workspace.path().join("blocked.txt");
    let cwd = PathUri::from_host_native_path(workspace.path())?;
    let mut sandbox = FileSystemSandboxContext::from_legacy_sandbox_policy(
        SandboxPolicy::new_read_only_policy(),
        cwd.clone(),
    )?;
    match expected_sandbox_type {
        codex_sandboxing::SandboxType::WindowsRestrictedToken => {
            sandbox.windows_sandbox_selection = WindowsSandboxSelection::RestrictedToken;
        }
        codex_sandboxing::SandboxType::WindowsMxc => {
            sandbox.windows_sandbox_selection = WindowsSandboxSelection::Mxc;
        }
        codex_sandboxing::SandboxType::None
        | codex_sandboxing::SandboxType::MacosSeatbelt
        | codex_sandboxing::SandboxType::LinuxSeccomp => {
            anyhow::bail!("expected a Windows sandbox type")
        }
    }

    let session = match context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: ProcessId::from("proc-windows-sandbox-stdin"),
            argv: vec![
                r"C:\Windows\System32\cmd.exe".to_string(),
                "/D".to_string(),
                "/V:ON".to_string(),
                "/S".to_string(),
                "/C".to_string(),
                format!(
                    "set /P line= & echo blocked > \"{}\" & echo from-stdin:!line!",
                    blocked_file.display()
                ),
            ],
            cwd,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty,
            pipe_stdin: !tty,
            arg0: None,
            sandbox: Some(sandbox),
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await
    {
        Ok(session) => session,
        Err(err) => return Err(err.into()),
    };
    assert_eq!(session.sandbox_type, Some(expected_sandbox_type));

    let input = if tty { b"hello\r" } else { b"hello\n" };
    let write_response = session.process.write(input.to_vec()).await?;
    assert_eq!(write_response.status, WriteStatus::Accepted);
    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;

    assert!(
        output.contains("from-stdin:hello"),
        "unexpected output: {output:?}"
    );
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    assert!(!blocked_file.exists());
    Ok(())
}

async fn assert_exec_process_rejects_write_without_pipe_stdin(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stdin-closed".to_string();
    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 0.3; if IFS= read -r line; then printf 'read:%s\\n' \"$line\"; else printf 'eof\\n'; fi".to_string(),
            ],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let write_response = session.process.write(b"ignored\n".to_vec()).await?;
    assert_eq!(write_response.status, WriteStatus::StdinClosed);
    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;

    assert_eq!(output, "eof\n");
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_signal_interrupts_process(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-signal".to_string();
    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "trap 'printf \"signal:2\\n\"; exit 7' INT; printf 'ready\\n'; while :; do :; done".to_string(),
            ],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process, .. } = session;
    let mut wake_rx = process.subscribe_wake();
    let mut ready_output = String::new();
    let mut after_seq = None;
    loop {
        let response =
            read_process_until_change(Arc::clone(&process), &mut wake_rx, after_seq).await?;
        for chunk in response.chunks {
            ready_output.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
            after_seq = Some(chunk.seq);
        }
        if ready_output.contains("ready\n") {
            break;
        }
        if response.closed {
            anyhow::bail!("process closed before readiness marker: {ready_output:?}");
        }
        after_seq = response.next_seq.checked_sub(1).or(after_seq);
    }

    process.signal(ProcessSignal::Interrupt).await?;
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;

    assert!(
        output.contains("signal:2"),
        "expected signal handler output, got {output:?}"
    );
    assert_eq!(exit_code, Some(7));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_signal_terminates_on_windows(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: ProcessId::from("proc-windows-signal"),
            argv: vec![
                "cmd".to_string(),
                "/C".to_string(),
                "echo ready && ping -n 30 127.0.0.1 >NUL".to_string(),
            ],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;

    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    process.signal(ProcessSignal::Interrupt).await?;
    let (_output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;

    assert_eq!(exit_code, Some(1));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_preserves_queued_events_before_subscribe(
    use_remote: bool,
) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let session = context
        .backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: ProcessId::from("proc-queued"),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'queued output\\n'".to_string(),
            ],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;
    assert_eq!(output, "queued output\n");
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn remote_exec_process_recovers_after_transport_disconnect() -> Result<()> {
    let server = exec_server().await?;
    let mut proxy = server.disconnectable_websocket_proxy().await?;
    let environment = Environment::create_for_tests(Some(proxy.websocket_url().to_string()))?;
    let backend = environment.get_exec_backend();
    let temp_dir = TempDir::new()?;
    let gate_path = temp_dir.path().join("release-output");
    let emitted_path = temp_dir.path().join("output-emitted");
    let session = backend
        .start(ExecParams {
            metadata: Default::default(),
            process_id: ProcessId::from("proc-recover"),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                concat!(
                    "printf 'ready:%s\\n' \"$$\"; ",
                    "while [ ! -f \"$GATE\" ]; do /bin/sleep 0.01; done; ",
                    "printf 'during:%s\\n' \"$$\"; ",
                    ": > \"$EMITTED\"; ",
                    "IFS= read -r line; ",
                    "printf 'after:%s:%s\\n' \"$$\" \"$line\"; ",
                    "exit 7",
                )
                .to_string(),
            ],
            cwd: PathUri::from_host_native_path(std::env::current_dir()?)?,
            shell_snapshot: None,
            env_policy: /*env_policy*/ None,
            env: HashMap::from([
                (
                    "GATE".to_string(),
                    gate_path.to_string_lossy().into_owned(),
                ),
                (
                    "EMITTED".to_string(),
                    emitted_path.to_string_lossy().into_owned(),
                ),
            ]),
            tty: false,
            pipe_stdin: true,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;

    let process = Arc::clone(&session.process);
    let mut events = process.subscribe_events();
    let mut output = Vec::new();
    let mut last_seq = 0;
    while !output.ends_with(b"\n") {
        match timeout(Duration::from_secs(5), events.recv()).await?? {
            ExecProcessEvent::Output(chunk) => {
                assert_eq!(chunk.seq, last_seq + 1);
                last_seq = chunk.seq;
                output.extend_from_slice(&chunk.chunk.into_inner());
            }
            event => anyhow::bail!("expected ready output before disconnect, got {event:?}"),
        }
    }
    let ready = String::from_utf8(output.clone())?;
    let pid = ready
        .strip_prefix("ready:")
        .and_then(|line| line.strip_suffix('\n'))
        .context("ready output should contain the process id")?
        .to_string();

    proxy.pause_and_disconnect().await?;
    tokio::fs::write(&gate_path, b"").await?;
    timeout(Duration::from_secs(5), async {
        while tokio::fs::metadata(&emitted_path).await.is_err() {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("process did not emit output while disconnected")?;

    let process_for_read = Arc::clone(&process);
    let mut pending_read = tokio::spawn(async move {
        process_for_read
            .read(
                /*after_seq*/ Some(last_seq),
                /*max_bytes*/ None,
                /*wait_ms*/ Some(0),
            )
            .await
    });
    assert!(
        timeout(Duration::from_millis(200), &mut pending_read)
            .await
            .is_err(),
        "process reads should wait while recovery is in progress"
    );
    proxy.resume()?;

    let recovered_read = timeout(Duration::from_secs(5), pending_read)
        .await
        .context("timed out waiting for a read after recovery")??;
    let recovered_read = recovered_read?;
    assert_eq!(recovered_read.failure, None);
    let recovered_output = recovered_read
        .chunks
        .into_iter()
        .flat_map(|chunk| chunk.chunk.into_inner())
        .collect::<Vec<_>>();
    assert_eq!(
        String::from_utf8(recovered_output)?,
        format!("during:{pid}\n")
    );

    let write = timeout(Duration::from_secs(5), process.write(b"hello\n".to_vec()))
        .await
        .context("timed out waiting for a write after recovery")??;
    assert_eq!(write.status, WriteStatus::Accepted);

    let mut saw_exit = false;
    loop {
        match timeout(Duration::from_secs(5), events.recv()).await?? {
            ExecProcessEvent::Output(chunk) => {
                assert_eq!(chunk.seq, last_seq + 1);
                last_seq = chunk.seq;
                output.extend_from_slice(&chunk.chunk.into_inner());
            }
            ExecProcessEvent::Exited { seq, exit_code, .. } => {
                assert_eq!(seq, last_seq + 1);
                assert_eq!(exit_code, 7);
                last_seq = seq;
                saw_exit = true;
            }
            ExecProcessEvent::Closed { seq } => {
                assert!(saw_exit, "closed must be delivered after exit");
                assert_eq!(seq, last_seq + 1);
                break;
            }
            ExecProcessEvent::Failed(message) => {
                anyhow::bail!("process recovery failed: {message}");
            }
        }
    }
    assert_eq!(
        String::from_utf8(output)?,
        format!("ready:{pid}\nduring:{pid}\nafter:{pid}:hello\n")
    );

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_starts_and_exits(use_remote: bool) -> Result<()> {
    assert_exec_process_starts_and_exits(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_streams_output(use_remote: bool) -> Result<()> {
    assert_exec_process_streams_output(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_pushes_events(use_remote: bool) -> Result<()> {
    assert_exec_process_pushes_events(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_replays_events_after_close(use_remote: bool) -> Result<()> {
    assert_exec_process_replays_events_after_close(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_retains_output_after_exit_until_streams_close(
    use_remote: bool,
) -> Result<()> {
    assert_exec_process_retains_output_after_exit_until_streams_close(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_write_then_read(use_remote: bool) -> Result<()> {
    assert_exec_process_write_then_read(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_write_then_read_without_tty(use_remote: bool) -> Result<()> {
    assert_exec_process_write_then_read_without_tty(use_remote).await
}

#[test_case(
    codex_sandboxing::SandboxType::WindowsRestrictedToken,
    false;
    "restricted_token"
)]
#[test_case(
    codex_sandboxing::SandboxType::WindowsMxc,
    false;
    "mxc_pipe"
)]
#[test_case(
    codex_sandboxing::SandboxType::WindowsMxc,
    true;
    "mxc_conpty"
)]
#[cfg_attr(not(windows), ignore = "Windows-only exec-server sandbox process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(remote_exec_server)]
async fn remote_windows_sandbox_process_accepts_process_write(
    expected_sandbox_type: codex_sandboxing::SandboxType,
    tty: bool,
) -> Result<()> {
    assert_remote_windows_sandbox_process_write(expected_sandbox_type, tty).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_rejects_write_without_pipe_stdin(use_remote: bool) -> Result<()> {
    assert_exec_process_rejects_write_without_pipe_stdin(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_signal_interrupts_process(use_remote: bool) -> Result<()> {
    assert_exec_process_signal_interrupts_process(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(windows), ignore = "Windows-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_signal_terminates_on_windows(use_remote: bool) -> Result<()> {
    assert_exec_process_signal_terminates_on_windows(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_preserves_queued_events_before_subscribe(use_remote: bool) -> Result<()> {
    assert_exec_process_preserves_queued_events_before_subscribe(use_remote).await
}
