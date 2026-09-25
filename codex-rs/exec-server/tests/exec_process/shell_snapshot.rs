use super::*;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SnapshotSandbox {
    None,
    WorkspaceWrite,
    DenyFdPath,
}

#[test_case("bash", false, SnapshotSandbox::None; "bash")]
#[test_case("bash", true, SnapshotSandbox::None; "bash_tty")]
#[cfg_attr(target_os = "macos", test_case("zsh", false, SnapshotSandbox::None; "zsh"))]
#[cfg_attr(target_os = "macos", test_case("zsh", true, SnapshotSandbox::None; "zsh_tty"))]
#[test_case("bash", false, SnapshotSandbox::DenyFdPath; "blocked_descriptor_path")]
#[cfg_attr(target_os = "macos", test_case("zsh", false, SnapshotSandbox::DenyFdPath; "zsh_blocked_descriptor_path"))]
#[cfg_attr(target_os = "macos", test_case("zsh", true, SnapshotSandbox::DenyFdPath; "zsh_blocked_descriptor_path_tty"))]
#[test_case("bash", false, SnapshotSandbox::WorkspaceWrite; "bash_protected_transport")]
#[test_case("bash", true, SnapshotSandbox::WorkspaceWrite; "bash_protected_transport_tty")]
#[cfg_attr(target_os = "macos", test_case("zsh", false, SnapshotSandbox::WorkspaceWrite; "zsh_protected_transport"))]
#[cfg_attr(target_os = "macos", test_case("zsh", true, SnapshotSandbox::WorkspaceWrite; "zsh_protected_transport_tty"))]
#[serial_test::serial(remote_exec_server)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_concurrent_replays_keep_independent_readers(
    shell: &str,
    tty: bool,
    sandbox_mode: SnapshotSandbox,
) -> Result<()> {
    let deny_fd_path = sandbox_mode == SnapshotSandbox::DenyFdPath;
    let use_sandbox = sandbox_mode != SnapshotSandbox::None;
    if use_sandbox
        && let Some(warning) =
            codex_sandboxing::system_bwrap_warning(&PermissionProfile::read_only())
    {
        eprintln!("skipping sandbox test: {warning}");
        return Ok(());
    }
    let context = if sandbox_mode == SnapshotSandbox::WorkspaceWrite {
        let (exe, linux_sandbox) = current_test_binary_helper_paths()?;
        let environment = Environment::create(
            /*exec_server_url*/ None,
            codex_exec_server::ExecServerRuntimePaths::new(exe, linux_sandbox)?,
            codex_http_client::HttpClientFactory::new(
                codex_http_client::OutboundProxyPolicy::ReqwestDefault,
            ),
        )?;
        ProcessContext {
            backend: environment.get_exec_backend(),
            _server: None,
        }
    } else {
        create_process_context(deny_fd_path).await?
    };
    let home = TempDir::new()?;
    let cwd = PathUri::from_host_native_path(home.path())?;
    // Keep full-size state in a function, below the 512 KiB state + environment cap.
    // The blocked-path case still exercises the smaller environment fallback.
    let payload_len = if deny_fd_path { 1 } else { 480 * 1024 };
    let payload = "x".repeat(payload_len);
    let source = if shell == "bash" {
        "${BASH_SOURCE[0]}"
    } else {
        "${(%):-%x}"
    };
    let source_check = if deny_fd_path {
        String::new()
    } else {
        format!("case \"{source}\" in /dev/fd/*) ;; *) return 42 ;; esac; ")
    };
    let aliases = if shell == "zsh" {
        "module_path=()\nsetopt RC_QUOTES\nalias snapshot_quoted=\"printf '%s|' 'one''two'\"\nalias eval='exit 44'\nalias case='exit 45'\n"
    } else {
        ""
    };
    std::fs::write(
        home.path().join(format!(".{shell}rc")),
        format!(
            "printf x >> \"$HOME/captures\"\nprofile_helper() {{ {source_check}local payload='{payload}'; [ \"${{#payload}}\" = {payload_len} ] || return 43; printf 'restored:%s' \"$1\"; }}\nexec() {{ exit 41; }}\nset -u\n{aliases}"
        ),
    )?;
    let protected_file =
        tempfile::NamedTempFile::new_in(codex_uds::prepare_shared_daemon_socket_directory()?)?;
    std::fs::write(protected_file.path(), "protected")?;
    let command = if sandbox_mode == SnapshotSandbox::WorkspaceWrite {
        // Broad /tmp grants must not expose the file while it still has a name.
        "if /bin/cat \"$SNAPSHOT_TRANSPORT_PROBE\" >/dev/null 2>&1 || (printf poisoned >> \"$SNAPSHOT_TRANSPORT_PROBE\") 2>/dev/null || /bin/mv \"$SNAPSHOT_TRANSPORT_PROBE\" \"$HOME/stolen\" 2>/dev/null; then exit 44; fi\n"
    } else {
        ""
    };
    let replay_checks = if shell == "zsh" {
        "eval snapshot_quoted\n[[ -o rcquotes ]] || exit 46\n"
    } else {
        ""
    };
    let command =
        format!("{command}IFS= read -r line\n{replay_checks}profile_helper \"$line\"; exit 7");
    let sandbox = if use_sandbox {
        let mut policy = FileSystemSandboxPolicy::read_only();
        policy.entries.push(FileSystemSandboxEntry::new(
            cwd.clone().into(),
            FileSystemAccessMode::Write,
        ));
        if deny_fd_path {
            policy.entries.push(FileSystemSandboxEntry::new(
                PathUri::from_host_native_path("/dev/fd")?.into(),
                FileSystemAccessMode::Deny,
            ));
        } else {
            policy.entries.push(FileSystemSandboxEntry::new(
                PathUri::from_host_native_path(std::fs::canonicalize("/tmp")?)?.into(),
                FileSystemAccessMode::Write,
            ));
        }
        Some(FileSystemSandboxContext::from_permission_profile(
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
            cwd.clone(),
        ))
    } else {
        None
    };
    let commands = (0..8).map(|index| {
        let backend = &context.backend;
        let cwd = cwd.clone();
        let home = home.path();
        let sandbox = sandbox.clone();
        let command = command.clone();
        let protected_path = protected_file.path();
        async move {
            let started = backend
                .start(ExecParams {
                    metadata: Default::default(),
                    process_id: format!("parallel-{index}").into(),
                    argv: vec![format!("/bin/{shell}"), "-lc".to_string(), command],
                    cwd,
                    env: HashMap::new(),
                    env_policy: Some(ExecEnvPolicy {
                        inherit: ShellEnvironmentPolicyInherit::None,
                        ignore_default_excludes: false,
                        exclude: Vec::new(),
                        r#set: HashMap::from([
                            ("HOME".to_string(), home.to_string_lossy().into_owned()),
                            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
                            (
                                "SNAPSHOT_TRANSPORT_PROBE".to_string(),
                                protected_path.to_string_lossy().into_owned(),
                            ),
                        ]),
                        include_only: Vec::new(),
                    }),
                    shell_snapshot: Some(ShellSnapshotRequest {
                        scope_id: "parallel".to_string(),
                        shell: ShellInfo {
                            name: shell.to_string(),
                            path: format!("/bin/{shell}"),
                        },
                    }),
                    tty,
                    pipe_stdin: true,
                    arg0: None,
                    sandbox,
                    enforce_managed_network: false,
                    managed_network: None,
                    network_proxy: None,
                })
                .await?;
            started
                .process
                .write(format!("input-{index}\n").into_bytes())
                .await?;
            let (output, errors, status, closed) =
                collect_process_output_from_events(started.process).await?;
            assert!(
                output.ends_with(&format!("restored:input-{index}")),
                "{output:?}"
            );
            if shell == "zsh" {
                assert!(
                    output.ends_with(&format!("one'two|restored:input-{index}")),
                    "{output:?}"
                );
            }
            assert_eq!((errors, status, closed), (String::new(), Some(7), true));
            Ok::<_, anyhow::Error>(())
        }
    });
    for result in futures::future::join_all(commands).await {
        result?;
    }
    assert_eq!(std::fs::read_to_string(home.path().join("captures"))?, "x");
    assert_eq!(std::fs::read_to_string(protected_file.path())?, "protected");
    Ok(())
}
