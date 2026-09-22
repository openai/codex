use super::*;
#[cfg(unix)]
use crate::config::NetworkProxySpec;
#[cfg(unix)]
use crate::tools::runtimes::RuntimePathPrepends;
#[cfg(unix)]
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
#[cfg(unix)]
use crate::tools::runtimes::prepare_brokered_shell_snapshot_env;
#[cfg(unix)]
use codex_network_proxy::CredentialProviderConfig;
#[cfg(unix)]
use codex_network_proxy::NetworkProxyConfig;
#[cfg(unix)]
use codex_protocol::config_types::EnvironmentVariablePattern;
#[cfg(unix)]
use codex_protocol::models::PermissionProfile;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use pretty_assertions::assert_eq;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
#[cfg(unix)]
use std::process::Command;
#[cfg(target_os = "linux")]
use std::process::Command as StdCommand;

use tempfile::tempdir;

#[cfg(unix)]
struct BlockingStdinPipe {
    original: i32,
    write_end: i32,
}

#[cfg(unix)]
impl BlockingStdinPipe {
    fn install() -> Result<Self> {
        let mut fds = [0i32; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } == -1 {
            return Err(std::io::Error::last_os_error()).context("create stdin pipe");
        }

        let original = unsafe { libc::dup(libc::STDIN_FILENO) };
        if original == -1 {
            let err = std::io::Error::last_os_error();
            unsafe {
                libc::close(fds[0]);
                libc::close(fds[1]);
            }
            return Err(err).context("dup stdin");
        }

        if unsafe { libc::dup2(fds[0], libc::STDIN_FILENO) } == -1 {
            let err = std::io::Error::last_os_error();
            unsafe {
                libc::close(fds[0]);
                libc::close(fds[1]);
                libc::close(original);
            }
            return Err(err).context("replace stdin");
        }

        unsafe {
            libc::close(fds[0]);
        }

        Ok(Self {
            original,
            write_end: fds[1],
        })
    }
}

#[cfg(unix)]
impl Drop for BlockingStdinPipe {
    fn drop(&mut self) {
        unsafe {
            libc::dup2(self.original, libc::STDIN_FILENO);
            libc::close(self.original);
            libc::close(self.write_end);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn assert_posix_snapshot_sections(snapshot: &str) {
    assert!(snapshot.contains("# Snapshot file"));
    assert!(snapshot.contains("aliases "));
    assert!(snapshot.contains("exports "));
    assert!(
        snapshot.contains("PATH"),
        "snapshot should capture a PATH export"
    );
    assert!(snapshot.contains("setopts "));
}

#[cfg(not(windows))]
#[test]
fn preserved_provider_context_respects_unix_environment_casing() {
    let mut env = HashMap::from([
        ("GH_HOST".to_string(), "startup.example.com".to_string()),
        ("gh_host".to_string(), "custom.example.com".to_string()),
    ]);

    replace_provider_context_with_trusted(
        &mut env,
        &HashMap::new(),
        ["GH_HOST".to_string(), "gh_host".to_string()],
        &["gh_host".to_string()],
    );

    assert_eq!(
        env,
        HashMap::from([("gh_host".to_string(), "custom.example.com".to_string())])
    );
}

#[cfg(windows)]
#[test]
fn inherited_provider_context_matches_windows_environment_casing() {
    let mut env = HashMap::new();
    let inherited = HashMap::from([("gh_host".to_string(), "github.example.com".to_string())]);

    replace_provider_context_with_trusted(
        &mut env,
        &inherited,
        std::iter::once("GH_HOST".to_string()),
        &[],
    );

    assert_eq!(
        env,
        HashMap::from([("GH_HOST".to_string(), "github.example.com".to_string())])
    );

    insert_env_value(&mut env, "gh_host", "github.alternate.example");
    assert_eq!(
        env_value(&env, "GH_HOST").map(String::as_str),
        Some("github.alternate.example")
    );
    assert_eq!(env.len(), 1);
}

async fn get_snapshot(shell_type: ShellType) -> Result<String> {
    let dir = tempdir()?;
    let path = dir.path().join("snapshot.sh");
    let shell = crate::shell::get_shell(shell_type)
        .with_context(|| format!("No available shell for {shell_type:?}"))?;
    write_shell_snapshot(
        &shell,
        &path.abs(),
        &dir.path().abs(),
        /*credential_broker*/ None,
        /*sandbox*/ None,
    )
    .await?;
    let content = fs::read_to_string(&path).await?;
    Ok(content)
}

#[test]
fn snapshot_file_name_parser_supports_legacy_and_suffixed_names() {
    let session_id = "019cf82b-6a62-7700-bbbd-46909794ef89";

    assert_eq!(
        snapshot_session_id_from_file_name(&format!("{session_id}.sh")),
        Some(session_id)
    );
    assert_eq!(
        snapshot_session_id_from_file_name(&format!("{session_id}.123.sh")),
        Some(session_id)
    );
    assert_eq!(
        snapshot_session_id_from_file_name(&format!("{session_id}.tmp-123")),
        Some(session_id)
    );
    assert_eq!(
        snapshot_session_id_from_file_name("not-a-snapshot.txt"),
        None
    );
}

#[cfg(unix)]
#[tokio::test]
async fn inactive_profiles_keep_snapshots_but_active_brokers_require_sandbox() -> Result<()> {
    use crate::config::PermissionProfileSnapshot;
    use crate::environment_selection::ThreadEnvironments;
    use crate::environment_selection::TurnEnvironmentSnapshot;
    use crate::tools::sandboxing::SandboxAttempt;
    use codex_exec_server::EnvironmentManager;
    use codex_exec_server::LOCAL_ENVIRONMENT_ID;
    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_protocol::protocol::EnvironmentConfig;
    use codex_protocol::protocol::EnvironmentConfigState;
    use codex_protocol::protocol::TurnEnvironmentSelection;
    use codex_sandboxing::SandboxManager;
    use codex_sandboxing::SandboxType;
    use tokio_util::sync::CancellationToken;

    let dir = tempdir()?;
    std::fs::create_dir(dir.path().join(".codex"))?;
    std::fs::write(
        dir.path().join(".codex/startup.sh"),
        "printf started > startup-ran\n",
    )?;
    let session_id = ThreadId::new();
    let session_telemetry = SessionTelemetry::new(
        session_id,
        "test",
        "test",
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test".to_string(),
        /*log_user_prompts*/ false,
        "test".to_string(),
        codex_protocol::protocol::SessionSource::Cli,
    );
    let shell = Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
    };
    let permission_profile = PermissionProfile::workspace_write();
    let mut network_config = NetworkProxyConfig {
        enabled: true,
        proxy_url: "http://127.0.0.1:0".to_string(),
        socks_url: "socks5://127.0.0.1:0".to_string(),
        ..NetworkProxyConfig::default()
    };
    network_config.set_credential_broker_enabled(/*enabled*/ true);
    let network_spec = NetworkProxySpec::from_config_and_constraints(
        network_config,
        /*requirements*/ None,
        &permission_profile,
    )?;
    let started_proxy = network_spec
        .start_proxy(
            &permission_profile,
            codex_network_proxy::ManagedProxyRouting::SharedIngress,
            codex_network_proxy::LocalBindingPolicy::DefaultFalse,
            /*policy_decider*/ None,
            /*blocked_request_observer*/ None,
            /*enable_network_approval_flow*/ false,
            Default::default(),
        )
        .await?;
    for (state, prefer_executor_snapshots, expect_snapshot) in [
        (SnapshotCredentialBrokerState::Unavailable, false, false),
        (SnapshotCredentialBrokerState::Inactive, false, true),
        (SnapshotCredentialBrokerState::Inactive, true, false),
        (
            SnapshotCredentialBrokerState::Ready(started_proxy.proxy()),
            false,
            false,
        ),
    ] {
        let (_sender, receiver) = watch::channel(state);
        let config = Arc::new(ShellSnapshotConfig {
            codex_home: dir.path().abs(),
            session_id,
            session_telemetry: session_telemetry.clone(),
            state_db: None,
            credential_broker: Some(receiver),
            prefer_executor_snapshots,
        });
        let snapshot = tokio::time::timeout(
            SNAPSHOT_TIMEOUT,
            ShellSnapshot::build_for_cwd(
                config,
                dir.path().abs(),
                shell.clone(),
                /*allow_login_shell*/ false,
                ShellEnvironmentPolicy {
                    r#set: HashMap::from([(
                        "BASH_ENV".to_string(),
                        "./.codex/startup.sh".to_string(),
                    )]),
                    ..ShellEnvironmentPolicy::default()
                },
                /*sandbox*/ None,
            ),
        )
        .await?;

        assert_eq!(snapshot.is_some(), expect_snapshot);
    }

    let (sender, _) = watch::channel(SnapshotCredentialBrokerState::Starting);
    let snapshot_builder = ShellSnapshot::new(
        dir.path().abs(),
        session_id,
        session_telemetry,
        /*state_db*/ None,
        Some(sender),
        /*prefer_executor_snapshots*/ false,
    );
    let mut config = EnvironmentConfig {
        allow_login_shell: false,
        workspace_roots: Vec::new(),
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_type: SandboxType::None,
        use_legacy_landlock: false,
        permission_profile: PermissionProfileSnapshot::legacy(permission_profile),
        shell_environment_policy: ShellEnvironmentPolicy::default(),
        exec_policy: None,
        mcp_policy: None,
        network_policy: None,
        selected_capability_roots: Vec::new(),
    };
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    let environments = ThreadEnvironments::new(
        Arc::clone(&manager),
        shell.clone(),
        |_| config.clone(),
        snapshot_builder.clone(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ false,
    );
    environments.update_selections(
        &[TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&dir.path().abs()),
            workspace_roots: Vec::new(),
            config: EnvironmentConfigState::FromThread,
        }],
        |_| config.clone(),
    );
    for (state, expect_snapshot) in [
        (SnapshotCredentialBrokerState::Starting, false),
        (SnapshotCredentialBrokerState::Inactive, true),
        (
            SnapshotCredentialBrokerState::Ready(started_proxy.proxy()),
            false,
        ),
        (SnapshotCredentialBrokerState::Inactive, true),
    ] {
        environments.set_snapshot_credential_broker(state.clone());
        let turn = environments.snapshot().await;
        let environment = turn.primary().expect("local environment");
        let snapshot = timeout(SNAPSHOT_TIMEOUT, environment.shell_snapshot.clone()).await?;
        assert_eq!(snapshot.is_some(), expect_snapshot);
        assert_eq!(environment.shell_snapshot_v2_supported, expect_snapshot);

        environments.set_snapshot_credential_broker(state.clone());
        let next = environments.snapshot().await;
        assert!(Arc::ptr_eq(
            &environment.shell_snapshot_cache,
            &next.primary().expect("next turn").shell_snapshot_cache,
        ));
        if matches!(state, SnapshotCredentialBrokerState::Ready(_)) {
            config
                .shell_environment_policy
                .r#set
                .insert("CORP_REGION".into(), "west".into());
            environments.update_thread_config(|_| config.clone());
            let updated = environments.snapshot().await;
            assert!(!Arc::ptr_eq(
                &environment.shell_snapshot_cache,
                &updated
                    .primary()
                    .expect("updated turn")
                    .shell_snapshot_cache,
            ));
            let child = ThreadEnvironments::new(
                Arc::clone(&manager),
                shell.clone(),
                |_| config.clone(),
                snapshot_builder.clone(),
                updated.clone(),
                /*non_blocking_snapshots*/ false,
            );
            let child = child.snapshot().await;
            assert!(!Arc::ptr_eq(
                &updated.primary().expect("parent turn").shell_snapshot_cache,
                &child.primary().expect("child turn").shell_snapshot_cache,
            ));
        }
    }
    assert!(!dir.path().join("startup-ran").exists());

    // Identical concurrent captures must not share one command's cancellation.
    fs::write(
        dir.path().join(".codex/startup.sh"),
        "export OPENAI_API_KEY=sk-snapshot-cache-test\nprintf started > capture-started\nwhile [ ! -f finish-startup ]; do sleep 0.01; done\n",
    )
    .await?;
    config.shell_environment_policy.r#set.insert(
        "BASH_ENV".into(),
        dir.path().join(".codex/startup.sh").display().to_string(),
    );
    environments.set_snapshot_credential_broker(SnapshotCredentialBrokerState::Ready(
        started_proxy.proxy(),
    ));
    environments.update_thread_config(|_| config.clone());
    let turn = environments.snapshot().await;
    let environment = turn.primary().expect("brokered environment");
    let mut tool_config = crate::config::ConfigBuilder::without_managed_config_for_tests()
        .codex_home(dir.path().to_path_buf())
        .build()
        .await?;
    tool_config
        .features
        .enable(codex_features::Feature::ShellSnapshot)?;
    tool_config.permissions.network = Some(network_spec);
    let cwd = dir.path().abs();
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let manager = SandboxManager::new();
    let permissions = PermissionProfile::Disabled;
    let cancellation = CancellationToken::new();
    let mut attempt = SandboxAttempt {
        sandbox: SandboxType::None,
        sandbox_requested: false,
        permissions: &permissions,
        exec_server_permissions: &permissions,
        enforce_managed_network: false,
        manager: &manager,
        sandbox_cwd: &cwd_uri,
        workspace_roots: std::slice::from_ref(&cwd_uri),
        sandbox_exe: None,
        use_legacy_landlock: false,
        windows_sandbox_type: SandboxType::None,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        network_denial_cancellation_token: Some(cancellation.clone()),
        network_proxy: None,
    };
    let first_sandbox = ShellSnapshotSandbox::new(
        &attempt,
        /*network*/ None,
        LOCAL_ENVIRONMENT_ID,
        /*additional_permissions*/ None,
    );
    attempt.network_denial_cancellation_token = Some(CancellationToken::new());
    let second_sandbox = ShellSnapshotSandbox::new(
        &attempt,
        /*network*/ None,
        LOCAL_ENVIRONMENT_ID,
        /*additional_permissions*/ None,
    );
    let command = shell.derive_exec_args("true", /*use_login_shell*/ false);
    assert!(
        environment
            .shell_snapshot(&cwd, &command, &shell, &tool_config, /*sandbox*/ None)
            .await
            .is_none()
    );
    let first = async {
        let snapshot = environment
            .shell_snapshot(&cwd, &command, &shell, &tool_config, Some(first_sandbox))
            .await;
        fs::write(dir.path().join("finish-startup"), "").await?;
        Ok::<_, anyhow::Error>(snapshot)
    };
    let second = environment.shell_snapshot(
        &cwd,
        &command,
        &shell,
        &tool_config,
        Some(second_sandbox.clone()),
    );
    let cancel_after_startup = async {
        while !dir.path().join("capture-started").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        started_proxy
            .proxy()
            .add_allowed_domain("startup.example")
            .await
            .expect("startup network approval should not invalidate either capture");
        cancellation.cancel();
    };
    let (first, second, ()) = timeout(SNAPSHOT_TIMEOUT * 2, async {
        tokio::join!(biased; first, second, cancel_after_startup)
    })
    .await?;
    assert!(first?.is_none(), "the cancelled capture must fail");
    let second = second.context("the unrelated capture must succeed")?;
    let reused = environment
        .shell_snapshot(
            &cwd,
            &command,
            &shell,
            &tool_config,
            Some(second_sandbox.clone()),
        )
        .await
        .context("successful captures remain reusable")?;
    assert!(Arc::ptr_eq(&second, &reused));

    fs::remove_file(reused.path()).await?;
    let rebuilt = environment
        .shell_snapshot(
            &cwd,
            &command,
            &shell,
            &tool_config,
            Some(second_sandbox.clone()),
        )
        .await
        .context("a deleted cached snapshot must be rebuilt")?;
    assert!(!Arc::ptr_eq(&reused, &rebuilt));
    assert!(rebuilt.path().as_path().is_file());
    let proxy = started_proxy.proxy();
    let mut old_env = HashMap::new();
    let context = rebuilt.restore_credentials(&mut old_env, environment.shell_environment_policy());
    old_env = context
        .prepare_child_environment(&proxy, old_env, /*environment_id*/ None)?
        .env;

    let original_config = proxy.current_cfg().await?;
    let mut changed_config = original_config.clone();
    changed_config.set_credential_broker_openai_base_url(Some("https://gateway.example/v1"));
    for (config, remains_valid) in [
        (original_config.clone(), true),
        (changed_config, false),
        (original_config, false),
    ] {
        proxy
            .replace_config_state(codex_network_proxy::build_config_state(
                config,
                codex_network_proxy::NetworkProxyConstraints::default(),
                codex_utils_path_uri::Platform::native(),
            )?)
            .await?;
        assert_eq!(
            rebuilt.is_brokered_for(environment.shell_environment_policy()),
            remains_valid
        );
    }
    let refreshed = environment
        .shell_snapshot(
            &cwd,
            &command,
            &shell,
            &tool_config,
            Some(second_sandbox.clone()),
        )
        .await
        .context("a changed broker configuration must invalidate cached snapshots")?;
    assert!(!Arc::ptr_eq(&rebuilt, &refreshed));
    let mut new_env = HashMap::new();
    let context =
        refreshed.restore_credentials(&mut new_env, environment.shell_environment_policy());
    new_env = context
        .prepare_child_environment(&proxy, new_env, /*environment_id*/ None)?
        .env;
    assert_ne!(new_env["OPENAI_API_KEY"], old_env["OPENAI_API_KEY"]);
    assert_ne!(new_env["OPENAI_API_KEY"], "sk-snapshot-cache-test");

    // A credential change during capture should recapture once under the new configuration.
    fs::remove_file(refreshed.path()).await?;
    fs::remove_file(dir.path().join("finish-startup")).await?;
    fs::remove_file(dir.path().join("capture-started")).await?;
    fs::write(
        dir.path().join(".codex/startup.sh"),
        "export OPENAI_API_KEY=sk-snapshot-cache-test\nprintf x >> capture-started\nwhile [ ! -f finish-startup ]; do sleep 0.01; done\n",
    )
    .await?;
    let capture =
        environment.shell_snapshot(&cwd, &command, &shell, &tool_config, Some(second_sandbox));
    let reconfigure = async {
        while !dir.path().join("capture-started").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let mut config = proxy.current_cfg().await?;
        config.set_credential_broker_openai_base_url(Some("https://new-gateway.example/v1"));
        proxy
            .replace_config_state(codex_network_proxy::build_config_state(
                config,
                codex_network_proxy::NetworkProxyConstraints::default(),
                codex_utils_path_uri::Platform::native(),
            )?)
            .await?;
        fs::write(dir.path().join("finish-startup"), "").await?;
        Ok::<_, anyhow::Error>(())
    };
    let (recaptured, updated) = timeout(SNAPSHOT_TIMEOUT * 2, async {
        tokio::join!(capture, reconfigure)
    })
    .await?;
    updated?;
    let recaptured = recaptured.context("credential changes during startup must recapture")?;
    assert!(recaptured.is_brokered_for(environment.shell_environment_policy()));
    assert_eq!(
        fs::read_to_string(dir.path().join("capture-started")).await?,
        "xx"
    );
    Ok(())
}

#[cfg(unix)]
async fn credential_snapshot_proxy() -> Result<crate::config::StartedNetworkProxy> {
    let mut network_config = NetworkProxyConfig::default();
    network_config.set_credential_broker_enabled(/*enabled*/ true);
    network_config.allow_local_binding = Some(true);
    network_config.credential_providers.insert(
        "local".to_string(),
        CredentialProviderConfig {
            env: vec!["LOCAL_TOKEN".to_string()],
            patterns: vec!["local_[a-z]{24}".to_string()],
            url_prefixes: vec!["https://public.local-provider.example/v1".to_string()],
            url_prefix_from_env: Some("LOCAL_URL".to_string()),
            ..CredentialProviderConfig::default()
        },
    );
    network_config.credential_providers.insert(
        "stripe".to_string(),
        CredentialProviderConfig {
            env: vec!["STRIPE_API_KEY".to_string()],
            patterns: vec!["stripe_live_[a-z]{24}".to_string()],
            url_prefix_from_env: Some("STRIPE_HOST".to_string()),
            ..CredentialProviderConfig::default()
        },
    );
    network_config.credential_providers.insert(
        "vendor".to_string(),
        CredentialProviderConfig {
            env: vec!["VENDOR_PASSWORD".to_string()],
            patterns: vec!["pin_[a-z]{8}".to_string()],
            url_prefix_from_env: Some("VENDOR_HOST".to_string()),
            ..CredentialProviderConfig::default()
        },
    );
    network_config.configure_credential_broker_environment(&HashMap::from([(
        "STRIPE_HOST".to_string(),
        "api.stripe.example".to_string(),
    )]));
    let permission_profile = PermissionProfile::workspace_write();
    let network_spec = NetworkProxySpec::from_config_and_constraints(
        network_config,
        /*requirements*/ None,
        &permission_profile,
    )?;
    Ok(network_spec
        .start_proxy(
            &permission_profile,
            codex_network_proxy::ManagedProxyRouting::SharedIngress,
            codex_network_proxy::LocalBindingPolicy::DefaultFalse,
            /*policy_decider*/ None,
            /*blocked_request_observer*/ None,
            /*enable_network_approval_flow*/ false,
            Default::default(),
        )
        .await?)
}

#[cfg(unix)]
#[tokio::test]
async fn snapshot_discovers_and_redacts_shell_initialized_credentials() -> Result<()> {
    let dir = tempdir()?;
    let startup = dir.path().join("startup.sh");
    std::fs::write(
        &startup,
        "unset GITHUB_ENTERPRISE_TOKEN UNSET_AUTH_HEADER\n\
         export GH_TOKEN='ghp_shell_only_secret'\n\
         export AUTH_HEADER=\"Bearer $GH_TOKEN\"\n\
         declare -rx GITHUB_TOKEN='ghp_readonly_secret'\n\
         declare -rx HOMEBREW_GITHUB_API_TOKEN=\"$GITHUB_TOKEN\"\n\
         export GH_ENTERPRISE_TOKEN='ghp_enterprise_secret'\n\
         export GH_HOST='attacker.example'\n\
         export OPENAI_API_KEY='sk-proj-snapshot-secret'\n\
         export STRIPE_API_KEY='stripe_live_abcdefghijklmnopqrstuvwx'\n\
         export STRIPE_HOST='https://startup.stripe.example/v1'\n\
         export STRIPE_AUTH_HEADER=\"Bearer $STRIPE_API_KEY\"\n\
         export VENDOR_PASSWORD='pin_abcdefgh'\n\
         export VENDOR_AUTH_HEADER=\"Bearer $VENDOR_PASSWORD\"\n\
         export VENDOR_HOST='https://attacker.vendor.example/v2'\n\
         export LOCAL_TOKEN='local_abcdefghijklmnopqrstuvwx'\n\
         export LOCAL_URL='http://127.0.0.1:1234/v1'\n\
         export LOCAL_AUTH_HEADER=\"Bearer $LOCAL_TOKEN\"\n\
         unset LOCAL_TOKEN\n\
         export AUTH_BUNDLE=\"GitHub $GH_TOKEN\n\
         OpenAI $OPENAI_API_KEY\"\n\
         export OPENAI_BASE_URL='https://api.snapshot.example/v1'\n\
         export IDENTITY_SEEN=\"${OPENAI_IDENTITY_TOKEN_FILE-missing}\"\n\
         export EXCLUDED_PARENT_HOME=\"${HOME-missing}\"\n\
         export STARTUP_PATH_OVERRIDE_SEEN=\"${PATH%%:*}\"\n\
         export STARTUP_CORP_REGION_SEEN=\"${CORP_REGION-missing}\"\n\
         export STARTUP_NPM_TOKEN_SEEN=\"${NPM_TOKEN-missing}\"\n",
    )?;

    let started_proxy = credential_snapshot_proxy().await?;
    let network_proxy = started_proxy.proxy();

    let shell = Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
    };
    let trusted_startup = ("BASH_ENV".to_string(), startup.display().to_string());
    let shell_environment_policy = ShellEnvironmentPolicy {
        exclude: vec![
            EnvironmentVariablePattern::new_case_insensitive("HOME"),
            EnvironmentVariablePattern::new_case_insensitive("VENDOR_HOST"),
            EnvironmentVariablePattern::new_case_insensitive("STRIPE_HOST"),
        ],
        r#set: HashMap::from([
            trusted_startup.clone(),
            ("GH_HOST".to_string(), "github.example.com".to_string()),
            (
                "GITHUB_ENTERPRISE_TOKEN".to_string(),
                "ghp_removed_enterprise_secret".to_string(),
            ),
            (
                "UNSET_AUTH_HEADER".to_string(),
                "Bearer ghp_removed_enterprise_secret".to_string(),
            ),
            (
                "OPENAI_IDENTITY_TOKEN_FILE".to_string(),
                "identity-token-secret".to_string(),
            ),
            ("PATH".to_string(), "/enterprise/bin".to_string()),
            ("CORP_REGION".to_string(), "production".to_string()),
            ("NPM_TOKEN".to_string(), "npm_enterprise_token".to_string()),
            (
                "VENDOR_HOST".to_string(),
                "https://api.vendor.example/v2".to_string(),
            ),
        ]),
        ..ShellEnvironmentPolicy::default()
    };
    let credential_broker = SnapshotCredentialBroker {
        network_proxy: network_proxy.clone(),
        shell_environment_policy: shell_environment_policy.clone(),
        allow_login_shell: false,
    };
    let path = dir.path().join("snapshot.sh").abs();
    let credentials = write_shell_snapshot(
        &shell,
        &path,
        &dir.path().abs(),
        Some(&credential_broker),
        /*sandbox*/ None,
    )
    .await?;
    let captured_credentials = credentials.as_ref().expect("brokered snapshot credentials");
    assert_eq!(
        captured_credentials
            .context_env
            .get("STRIPE_HOST")
            .map(String::as_str),
        Some("api.stripe.example"),
        "excluding a trusted destination must not let startup replace it"
    );
    assert_eq!(
        captured_credentials
            .context_env
            .get("VENDOR_HOST")
            .map(String::as_str),
        Some("https://api.vendor.example/v2")
    );
    assert_eq!(
        captured_credentials
            .context_env
            .get("OPENAI_BASE_URL")
            .map(String::as_str),
        Some("https://api.snapshot.example/v1")
    );
    let snapshot = fs::read_to_string(&path).await?;

    for secret in [
        "ghp_shell_only_secret",
        "ghp_readonly_secret",
        "ghp_enterprise_secret",
        "sk-proj-snapshot-secret",
        "stripe_live_abcdefghijklmnopqrstuvwx",
        "pin_abcdefgh",
        "local_abcdefghijklmnopqrstuvwx",
        "identity-token-secret",
    ] {
        assert!(!snapshot.contains(secret), "snapshot exposed {secret}");
    }
    assert!(!snapshot.contains("attacker.example"));
    assert!(!snapshot.contains("CODEX_NETWORK_PROXY_BROKERED_CREDENTIALS"));
    assert!(snapshot.contains("api.snapshot.example"));
    assert!(!snapshot.contains("attacker.vendor.example"));
    assert!(snapshot.contains("IDENTITY_SEEN=\"missing\""));
    assert!(snapshot.contains("EXCLUDED_PARENT_HOME=\"missing\""));
    assert!(snapshot.contains("STARTUP_PATH_OVERRIDE_SEEN=\"/enterprise/bin\""));
    assert!(snapshot.contains("declare -x PATH=\"/enterprise/bin\""));
    assert!(snapshot.contains("STARTUP_CORP_REGION_SEEN=\"production\""));
    assert!(snapshot.contains("STARTUP_NPM_TOKEN_SEEN=\"npm_enterprise_token\""));
    assert!(snapshot.contains("declare -rx HOMEBREW_GITHUB_API_TOKEN=\"${GITHUB_TOKEN-}\""));
    assert!(snapshot.contains("${VENDOR_PASSWORD-}"));
    assert!(!snapshot.contains("api.stripe.example"));

    validate_snapshot(
        &shell,
        &path,
        &dir.path().abs(),
        Some(&credential_broker),
        /*sandbox*/ None,
    )
    .await?;

    let validation_path = dir.path().join("validation.sh").abs();
    fs::write(
        &validation_path,
        "test \"${HOME-missing}\" = missing && \
         test \"${CODEX_NETWORK_PROXY_CREDENTIAL_BROKER_ACTIVE-}\" = 1\n",
    )
    .await?;
    validate_snapshot(
        &shell,
        &validation_path,
        &dir.path().abs(),
        Some(&credential_broker),
        /*sandbox*/ None,
    )
    .await?;

    let filtered_startup_broker = SnapshotCredentialBroker {
        network_proxy: network_proxy.clone(),
        shell_environment_policy: ShellEnvironmentPolicy {
            r#set: HashMap::from([trusted_startup.clone()]),
            include_only: vec![
                EnvironmentVariablePattern::new_case_insensitive("PATH"),
                EnvironmentVariablePattern::new_case_insensitive("EXCLUDED_PARENT_HOME"),
            ],
            ..ShellEnvironmentPolicy::default()
        },
        allow_login_shell: false,
    };
    let filtered_startup_path = dir.path().join("filtered-startup-snapshot.sh").abs();
    write_shell_snapshot(
        &shell,
        &filtered_startup_path,
        &dir.path().abs(),
        Some(&filtered_startup_broker),
        /*sandbox*/ None,
    )
    .await?;
    assert!(
        !fs::read_to_string(&filtered_startup_path)
            .await?
            .contains("EXCLUDED_PARENT_HOME")
    );

    let mut excluded_credential_broker = SnapshotCredentialBroker {
        network_proxy: network_proxy.clone(),
        shell_environment_policy: shell_environment_policy.clone(),
        allow_login_shell: false,
    };
    excluded_credential_broker
        .shell_environment_policy
        .exclude
        .push(EnvironmentVariablePattern::new_case_insensitive("GH_TOKEN"));
    let (_, excluded_credentials) = capture_snapshot(
        &shell,
        &dir.path().abs(),
        Some(&excluded_credential_broker),
        /*sandbox*/ None,
    )
    .await?;
    assert_eq!(
        excluded_credentials.unwrap().unset_credential_keys,
        [
            "AUTH_BUNDLE",
            "AUTH_HEADER",
            "GH_TOKEN",
            "GITHUB_ENTERPRISE_TOKEN",
            "UNSET_AUTH_HEADER",
        ]
    );

    let inherited_secret = "ghp_inherited_enterprise_secret";
    let inherited_context = HashMap::from([(
        "GH_HOST".to_string(),
        "github.inherited.example".to_string(),
    )]);
    let mut inherited_discovery_env = HashMap::from([
        (
            "GH_ENTERPRISE_TOKEN".to_string(),
            inherited_secret.to_string(),
        ),
        ("GH_HOST".to_string(), "attacker.example".to_string()),
    ]);
    replace_provider_context_with_trusted(
        &mut inherited_discovery_env,
        &inherited_context,
        network_proxy
            .credential_broker_environment(&HashMap::new())
            .provider_context_keys,
        &[],
    );
    assert_eq!(
        inherited_discovery_env.get("GH_HOST").map(String::as_str),
        Some("github.inherited.example")
    );
    network_proxy.apply_to_env(&mut inherited_discovery_env);
    let mut inherited_allowed_env = HashMap::from([(
        "GH_ENTERPRISE_TOKEN".to_string(),
        inherited_secret.to_string(),
    )]);
    replace_provider_context_with_trusted(
        &mut inherited_allowed_env,
        &inherited_context,
        network_proxy
            .credential_broker_environment(&inherited_discovery_env)
            .binding_keys,
        &[],
    );
    network_proxy.apply_to_env(&mut inherited_allowed_env);
    assert_eq!(
        inherited_allowed_env.get("GH_HOST").map(String::as_str),
        Some("github.inherited.example")
    );
    assert_ne!(
        inherited_allowed_env
            .get("GH_ENTERPRISE_TOKEN")
            .map(String::as_str),
        Some(inherited_secret)
    );
    let mut inherited_snapshot = format!("export GH_ENTERPRISE_TOKEN={inherited_secret}\n");
    assert!(
        network_proxy.virtualize_brokered_text(&mut inherited_snapshot, &inherited_allowed_env)
    );
    assert!(!inherited_snapshot.contains(inherited_secret));

    let snapshot_file = ShellSnapshotFile { path, credentials };
    let unset_credential_keys = snapshot_file
        .credentials
        .as_ref()
        .unwrap()
        .unset_credential_keys
        .as_slice();
    assert_eq!(
        unset_credential_keys,
        &["GITHUB_ENTERPRISE_TOKEN", "UNSET_AUTH_HEADER"]
    );
    let mut unset_policy = shell_environment_policy.clone();
    let mut unset_env = HashMap::new();
    for key in unset_credential_keys {
        unset_env.insert(key.clone(), unset_policy.r#set.remove(key).unwrap());
    }
    let _ = snapshot_file.restore_credentials(&mut unset_env, &unset_policy);
    snapshot_file.restore_fail_open_aliases(&mut unset_env, /*environment_id*/ None);
    assert!(
        unset_credential_keys
            .iter()
            .all(|key| !unset_env.contains_key(key))
    );

    let mut env = HashMap::from([
        ("GH_HOST".to_string(), "github.example.com".to_string()),
        ("STRIPE_HOST".to_string(), "api.stripe.example".to_string()),
        (
            "VENDOR_HOST".to_string(),
            "https://api.vendor.example/v2".to_string(),
        ),
    ]);
    env.extend(
        shell_environment_policy
            .r#set
            .iter()
            .filter(|(key, _)| unset_credential_keys.contains(key))
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    let credential_context = snapshot_file.restore_credentials(&mut env, &shell_environment_policy);
    for (key, value) in [
        ("GH_TOKEN", "ghp_shell_only_secret"),
        ("GITHUB_TOKEN", "ghp_readonly_secret"),
        ("GH_ENTERPRISE_TOKEN", "ghp_enterprise_secret"),
        ("GITHUB_ENTERPRISE_TOKEN", "ghp_removed_enterprise_secret"),
        ("UNSET_AUTH_HEADER", "Bearer ghp_removed_enterprise_secret"),
        ("OPENAI_API_KEY", "sk-proj-snapshot-secret"),
        ("STRIPE_API_KEY", "stripe_live_abcdefghijklmnopqrstuvwx"),
        ("STRIPE_HOST", "api.stripe.example"),
        ("VENDOR_PASSWORD", "pin_abcdefgh"),
        ("VENDOR_HOST", "https://api.vendor.example/v2"),
    ] {
        assert_eq!(env.get(key).map(String::as_str), Some(value), "{key}");
    }
    assert_eq!(
        env.get("OPENAI_BASE_URL").map(String::as_str),
        Some("https://api.snapshot.example/v1")
    );
    let explicit_vendor_policy = ShellEnvironmentPolicy {
        inherit: codex_protocol::config_types::ShellEnvironmentPolicyInherit::None,
        r#set: HashMap::from([("VENDOR_PASSWORD".to_string(), "pin_abcdefgh".to_string())]),
        ..ShellEnvironmentPolicy::default()
    };
    let mut explicit_vendor_env = explicit_vendor_policy.r#set.clone();
    let explicit_vendor_context =
        snapshot_file.restore_credentials(&mut explicit_vendor_env, &explicit_vendor_policy);
    assert!(!explicit_vendor_env.contains_key("VENDOR_HOST"));
    assert_eq!(
        explicit_vendor_context.with_fallbacks(&explicit_vendor_env)["VENDOR_HOST"],
        "https://api.vendor.example/v2"
    );
    explicit_vendor_env = explicit_vendor_context
        .prepare_child_environment(
            &network_proxy,
            explicit_vendor_env,
            /*environment_id*/ None,
        )?
        .env;
    assert!(!explicit_vendor_env.contains_key("VENDOR_HOST"));
    assert_ne!(explicit_vendor_env["VENDOR_PASSWORD"], "pin_abcdefgh");
    let prepared_vendor_env = explicit_vendor_env.clone();
    network_proxy.apply_to_env(&mut explicit_vendor_env);
    assert_eq!(explicit_vendor_env, prepared_vendor_env);

    let current_vendor_host = "https://api.current-vendor.example/v2";
    let mut current_vendor_env = explicit_vendor_policy.r#set.clone();
    current_vendor_env.insert("VENDOR_HOST".to_string(), current_vendor_host.to_string());
    let current_vendor_context =
        snapshot_file.restore_credentials(&mut current_vendor_env, &explicit_vendor_policy);
    current_vendor_env = current_vendor_context
        .prepare_child_environment(
            &network_proxy,
            current_vendor_env,
            /*environment_id*/ None,
        )?
        .env;
    assert_eq!(
        current_vendor_env.get("VENDOR_HOST").map(String::as_str),
        Some(current_vendor_host)
    );
    assert_ne!(current_vendor_env["VENDOR_PASSWORD"], "pin_abcdefgh");

    for excluded_context in [false, true] {
        let mut policy = shell_environment_policy.clone();
        if excluded_context {
            policy
                .exclude
                .push(EnvironmentVariablePattern::new_case_insensitive(
                    "OPENAI_BASE_URL",
                ));
        }
        let ambient_host = "https://api.ambient.example/v1";
        let mut restored_env =
            HashMap::from([("OPENAI_BASE_URL".to_string(), ambient_host.to_string())]);
        let context = snapshot_file.restore_credentials(&mut restored_env, &policy);
        assert_eq!(
            restored_env["OPENAI_BASE_URL"],
            if excluded_context {
                ambient_host
            } else {
                "https://api.snapshot.example/v1"
            }
        );
        restored_env = context
            .prepare_child_environment(&network_proxy, restored_env, /*environment_id*/ None)?
            .env;
        assert_ne!(restored_env["OPENAI_API_KEY"], "sk-proj-snapshot-secret");
        assert_eq!(
            restored_env["OPENAI_BASE_URL"],
            if excluded_context {
                ambient_host
            } else {
                "https://api.snapshot.example/v1"
            }
        );
    }

    for (credential_key, context_key, real, current_host) in [
        (
            "GH_ENTERPRISE_TOKEN",
            "GH_HOST",
            "ghp_enterprise_secret",
            "github.override.example",
        ),
        (
            "VENDOR_PASSWORD",
            "VENDOR_HOST",
            "pin_abcdefgh",
            "https://api.current-vendor.example/v2",
        ),
    ] {
        let policy = ShellEnvironmentPolicy {
            ignore_default_excludes: true,
            include_only: vec![EnvironmentVariablePattern::new_case_insensitive(
                credential_key,
            )],
            r#set: HashMap::from([(context_key.to_string(), current_host.to_string())]),
            ..ShellEnvironmentPolicy::default()
        };
        let mut restored_env = create_env_from_vars([], &policy, /*thread_id*/ None);
        let context = snapshot_file.restore_credentials(&mut restored_env, &policy);
        assert!(!restored_env.contains_key(context_key));
        assert_eq!(
            context.with_fallbacks(&restored_env)[context_key],
            current_host
        );
        restored_env = context
            .prepare_child_environment(&network_proxy, restored_env, /*environment_id*/ None)?
            .env;
        assert_ne!(restored_env[credential_key], real, "{credential_key}");
        assert!(!restored_env.contains_key(context_key));
    }

    let unbrokered_replay = Command::new("/bin/bash")
        .arg("-c")
        .arg(". \"$1\" && printf '%s\\n%s' \"$AUTH_HEADER\" \"$AUTH_BUNDLE\"")
        .arg("snapshot")
        .arg(snapshot_file.path().as_path())
        .env_clear()
        .envs(&env)
        .output()?;
    assert!(unbrokered_replay.status.success());
    assert_eq!(
        String::from_utf8(unbrokered_replay.stdout)?,
        "Bearer ghp_shell_only_secret\nGitHub ghp_shell_only_secret\nOpenAI sk-proj-snapshot-secret"
    );

    let mut cleared_local_context = HashMap::from([
        (
            "LOCAL_AUTH_HEADER".to_string(),
            snapshot_file
                .credentials
                .as_ref()
                .unwrap()
                .fail_open_aliases["LOCAL_AUTH_HEADER"]
                .dummy_value
                .clone()
                .unwrap(),
        ),
        ("LOCAL_URL".to_string(), String::new()),
    ]);
    network_proxy.apply_to_env_for_snapshot(&mut cleared_local_context);
    env = credential_context
        .prepare_child_environment(&network_proxy, env, Some("snapshot-child"))?
        .env;
    snapshot_file.restore_fail_open_aliases(&mut env, Some("snapshot-child"));
    assert_ne!(env["GH_TOKEN"], "ghp_shell_only_secret");
    assert_ne!(env["GITHUB_TOKEN"], "ghp_readonly_secret");
    assert_ne!(env["GH_ENTERPRISE_TOKEN"], "ghp_enterprise_secret");
    assert_ne!(env["OPENAI_API_KEY"], "sk-proj-snapshot-secret");
    assert_ne!(
        env["STRIPE_API_KEY"],
        "stripe_live_abcdefghijklmnopqrstuvwx"
    );
    assert_ne!(env["VENDOR_PASSWORD"], "pin_abcdefgh");
    assert!(!env.contains_key("LOCAL_TOKEN"));
    let mut replay_env = env.clone();
    replay_env.remove("STRIPE_HOST");
    let snapshot_path = snapshot_file.path();
    prepare_brokered_shell_snapshot_env(&mut replay_env, Some(&snapshot_path), &shell);
    let replay_command = maybe_wrap_shell_lc_with_snapshot(
        &shell.derive_exec_args(
            "printf '%s\\n%s\\n%s\\n%s\\n%s\\n%s\\n%s' \"$HOMEBREW_GITHUB_API_TOKEN\" \"$AUTH_HEADER\" \"$AUTH_BUNDLE\" \"$VENDOR_PASSWORD\" \"${VENDOR_HOST-missing}\" \"${STRIPE_HOST-missing}\" \"$LOCAL_AUTH_HEADER\"",
            /*use_login_shell*/ false,
        ),
        &shell,
        Some(&snapshot_path),
        &shell_environment_policy.r#set,
        &replay_env,
        &RuntimePathPrepends::default(),
    );
    let replay = Command::new(&replay_command[0])
        .args(&replay_command[1..])
        .env_clear()
        .envs(&replay_env)
        .output()?;
    assert!(
        replay.status.success(),
        "snapshot replay failed: {replay:?}"
    );
    assert_eq!(
        String::from_utf8(replay.stdout)?,
        format!(
            "{}\nBearer {}\nGitHub {}\nOpenAI {}\n{}\n{}\nmissing\nBearer local_abcdefghijklmnopqrstuvwx",
            env["GITHUB_TOKEN"],
            env["GH_TOKEN"],
            env["GH_TOKEN"],
            env["OPENAI_API_KEY"],
            env["VENDOR_PASSWORD"],
            env["VENDOR_HOST"]
        )
    );

    let filtered_policy = ShellEnvironmentPolicy {
        include_only: vec![EnvironmentVariablePattern::new_case_insensitive("GH_HOST")],
        ..ShellEnvironmentPolicy::default()
    };
    let mut filtered_env = HashMap::new();
    let _ = snapshot_file.restore_credentials(&mut filtered_env, &filtered_policy);
    assert!(filtered_env.is_empty());

    let partially_filtered_credential_broker = SnapshotCredentialBroker {
        network_proxy: network_proxy.clone(),
        shell_environment_policy: ShellEnvironmentPolicy {
            r#set: HashMap::from([trusted_startup.clone()]),
            include_only: vec![
                EnvironmentVariablePattern::new_case_insensitive("BASH_ENV"),
                EnvironmentVariablePattern::new_case_insensitive("GH_TOKEN"),
                EnvironmentVariablePattern::new_case_insensitive("AUTH_HEADER"),
                EnvironmentVariablePattern::new_case_insensitive("AUTH_BUNDLE"),
            ],
            ..ShellEnvironmentPolicy::default()
        },
        allow_login_shell: false,
    };
    for (github_token, openai_api_key) in [
        ("ghp_shell_only_secret", "sk-proj-snapshot-secret"),
        (env["GH_TOKEN"].as_str(), env["OPENAI_API_KEY"].as_str()),
    ] {
        std::fs::write(
            &startup,
            format!(
                "export GH_TOKEN='{github_token}'\n\
                 export GITHUB_TOKEN=\"$GH_TOKEN\"\n\
                 export AUTH_HEADER=\"Bearer $GH_TOKEN\"\n\
                 export OPENAI_API_KEY='{openai_api_key}'\n\
                 export AUTH_BUNDLE=\"GitHub $GH_TOKEN\n\
                 OpenAI $OPENAI_API_KEY\"\n\
                 unset OPENAI_API_KEY\n"
            ),
        )?;
        let partially_filtered_path = dir.path().join("partially-filtered-snapshot.sh").abs();
        let filtered_credentials = write_shell_snapshot(
            &shell,
            &partially_filtered_path,
            &dir.path().abs(),
            Some(&partially_filtered_credential_broker),
            /*sandbox*/ None,
        )
        .await?;
        let filtered_replay = Command::new("/bin/bash")
            .arg("-c")
            .arg(". \"$1\" && printf '%s\\n%s' \"$AUTH_HEADER\" \"${AUTH_BUNDLE-unset}\"")
            .arg("snapshot")
            .arg(partially_filtered_path.as_path())
            .env_clear()
            .env("GH_TOKEN", "ghp_filtered_dummy")
            .output()?;
        assert!(filtered_replay.status.success());
        assert_eq!(
            String::from_utf8(filtered_replay.stdout)?,
            "Bearer ghp_filtered_dummy\nunset"
        );
        let filtered_snapshot = ShellSnapshotFile {
            path: partially_filtered_path,
            credentials: filtered_credentials,
        };
        let mut fail_open_env = HashMap::new();
        filtered_snapshot
            .restore_fail_open_aliases(&mut fail_open_env, /*environment_id*/ None);
        assert!(!fail_open_env.contains_key("AUTH_BUNDLE"));
    }

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn snapshot_protects_posix_startup_only_when_it_contains_credentials() -> Result<()> {
    let dir = tempdir()?;
    let started_proxy = credential_snapshot_proxy().await?;
    let network_proxy = started_proxy.proxy();
    let posix_startup = dir.path().join("posix-startup.sh");
    std::fs::write(
        &posix_startup,
        "export GH_TOKEN='ghp_posix_shell_secret'\nexport POSIX_STARTUP_LOADED=1\n",
    )?;
    let posix_shell = Shell {
        shell_type: ShellType::Sh,
        shell_path: PathBuf::from("/bin/sh"),
    };
    let posix_credential_broker = SnapshotCredentialBroker {
        network_proxy: network_proxy.clone(),
        shell_environment_policy: ShellEnvironmentPolicy {
            r#set: HashMap::from([("ENV".to_string(), "$PWD/posix-startup.sh".to_string())]),
            ..ShellEnvironmentPolicy::default()
        },
        allow_login_shell: true,
    };
    let posix_snapshot_path = dir.path().join("posix-snapshot.sh").abs();
    let posix_credentials = write_shell_snapshot(
        &posix_shell,
        &posix_snapshot_path,
        &dir.path().abs(),
        Some(&posix_credential_broker),
        /*sandbox*/ None,
    )
    .await?
    .expect("brokered POSIX snapshot has credentials");
    let posix_snapshot = fs::read_to_string(&posix_snapshot_path).await?;
    assert!(posix_snapshot.contains("POSIX_STARTUP_LOADED"));
    assert!(!posix_snapshot.contains("ghp_posix_shell_secret"));
    assert_eq!(
        posix_credentials
            .protected_startup_env
            .as_deref()
            .map(std::fs::canonicalize)
            .transpose()?,
        Some(std::fs::canonicalize(&posix_startup)?)
    );

    std::fs::write(&posix_startup, "export APPLICATION_SETTING=production\n")?;
    let application_snapshot_path = dir.path().join("posix-application-snapshot.sh").abs();
    let application_credentials = write_shell_snapshot(
        &posix_shell,
        &application_snapshot_path,
        &dir.path().abs(),
        Some(&posix_credential_broker),
        /*sandbox*/ None,
    )
    .await?
    .expect("brokered POSIX application snapshot has credentials");
    assert!(application_credentials.protected_startup_env.is_none());

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn snapshot_discovers_and_restores_inherited_credential_aliases() -> Result<()> {
    let dir = tempdir()?;
    let startup = dir.path().join("startup.sh");
    let started_proxy = credential_snapshot_proxy().await?;
    let network_proxy = started_proxy.proxy();
    let shell = Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
    };
    let trusted_startup = ("BASH_ENV".to_string(), startup.display().to_string());
    std::fs::write(
        &startup,
        "export GH_TOKEN='ghp_hidden_alias_secret'\n\
         export HIDDEN_HEADER=\"prefix\\\"$GH_TOKEN\\\"suffix\"\n\
         export GITHUB_TOKEN='ghp_0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefgh'\n\
         export UNKNOWN_HEADER=\"Bearer $GITHUB_TOKEN\"\n\
         export STRIPE_API_KEY='stripe_live_zyxwvutsrqponmlkjihgfedc'\n\
         export STRIPE_HIDDEN_HEADER=\"Bearer $STRIPE_API_KEY\"\n\
         export STRIPE_HOST='api.stripe.example'\n\
         unset GH_TOKEN GITHUB_TOKEN GITHUB_ENTERPRISE_TOKEN STRIPE_API_KEY\n",
    )?;
    let mut previously_brokered_env = HashMap::from([(
        "GH_TOKEN".to_string(),
        "ghp_hidden_alias_secret".to_string(),
    )]);
    network_proxy.apply_to_env(&mut previously_brokered_env);
    let inherited_credential_broker = SnapshotCredentialBroker {
        network_proxy: network_proxy.clone(),
        shell_environment_policy: ShellEnvironmentPolicy {
            r#set: HashMap::from([trusted_startup]),
            ..ShellEnvironmentPolicy::default()
        },
        allow_login_shell: false,
    };
    let inherited_path = dir.path().join("inherited-snapshot.sh").abs();
    let inherited_credentials = write_shell_snapshot(
        &shell,
        &inherited_path,
        &dir.path().abs(),
        Some(&inherited_credential_broker),
        /*sandbox*/ None,
    )
    .await?;
    let inherited_snapshot = fs::read_to_string(&inherited_path).await?;
    assert!(!inherited_snapshot.contains("ghp_hidden_alias_secret"));
    assert!(!inherited_snapshot.contains("ghp_0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefgh"));
    assert!(!inherited_snapshot.contains("stripe_live_zyxwvutsrqponmlkjihgfedc"));
    let inherited_replay = Command::new("/bin/bash")
        .arg("-c")
        .arg(". \"$1\" && printf '%s\\n%s\\n%s' \"${HIDDEN_HEADER-unset}\" \"${UNKNOWN_HEADER-unset}\" \"${STRIPE_HIDDEN_HEADER-unset}\"")
        .arg("snapshot")
        .arg(inherited_path.as_path())
        .env_clear()
        .output()?;
    assert!(inherited_replay.status.success());
    assert_eq!(
        String::from_utf8(inherited_replay.stdout)?,
        "unset\nunset\nunset"
    );

    let snapshot_file = ShellSnapshotFile {
        path: inherited_path,
        credentials: inherited_credentials,
    };
    let real_hidden_header = "prefix\"ghp_hidden_alias_secret\"suffix";
    let real_header = "Bearer ghp_0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefgh";
    let real_stripe_header = "Bearer stripe_live_zyxwvutsrqponmlkjihgfedc";
    let mut brokered_env = HashMap::new();
    let credential_context = snapshot_file.restore_credentials(
        &mut brokered_env,
        &inherited_credential_broker.shell_environment_policy,
    );
    brokered_env = credential_context
        .prepare_child_environment(
            &inherited_credential_broker.network_proxy,
            brokered_env,
            /*environment_id*/ None,
        )?
        .env;
    let dummy_header = brokered_env["UNKNOWN_HEADER"].clone();
    let dummy_hidden_header = brokered_env["HIDDEN_HEADER"].clone();
    let dummy_stripe_header = brokered_env["STRIPE_HIDDEN_HEADER"].clone();
    assert_ne!(dummy_header, real_header);
    assert_ne!(dummy_hidden_header, real_hidden_header);
    assert_ne!(dummy_stripe_header, real_stripe_header);
    snapshot_file.restore_fail_open_aliases(&mut brokered_env, /*environment_id*/ None);
    assert_eq!(
        brokered_env.get("UNKNOWN_HEADER").map(String::as_str),
        Some(dummy_header.as_str())
    );
    assert_eq!(
        brokered_env.get("HIDDEN_HEADER").map(String::as_str),
        Some(dummy_hidden_header.as_str())
    );
    assert_eq!(
        brokered_env.get("STRIPE_HIDDEN_HEADER").map(String::as_str),
        Some(dummy_stripe_header.as_str())
    );

    let mut fail_open_env = HashMap::new();
    let _ = snapshot_file.restore_credentials(
        &mut fail_open_env,
        &inherited_credential_broker.shell_environment_policy,
    );
    snapshot_file.restore_fail_open_aliases(&mut fail_open_env, /*environment_id*/ None);
    assert_eq!(
        fail_open_env.get("UNKNOWN_HEADER").map(String::as_str),
        Some(real_header)
    );
    assert_eq!(
        fail_open_env.get("HIDDEN_HEADER").map(String::as_str),
        Some(real_hidden_header)
    );
    assert_eq!(
        fail_open_env
            .get("STRIPE_HIDDEN_HEADER")
            .map(String::as_str),
        Some(real_stripe_header)
    );

    std::fs::write(
        &startup,
        "export GH_TOKEN='ghp_hidden_alias_secret'\n\
         credential_function() { printf '%s' 'ghp_hidden_alias_secret'; }\n\
         unset GH_TOKEN\n",
    )?;
    let residual_credential = capture_snapshot(
        &shell,
        &dir.path().abs(),
        Some(&inherited_credential_broker),
        /*sandbox*/ None,
    )
    .await;
    assert!(
        residual_credential.is_err(),
        "snapshot with a credential-bearing shell function must be rejected"
    );

    std::fs::write(
        &startup,
        "unset PATH\nset -u\nexport GH_TOKEN='ghp_unset_path_secret'\n",
    )?;
    let (unset_path_snapshot, _) = capture_snapshot(
        &shell,
        &dir.path().abs(),
        Some(&inherited_credential_broker),
        /*sandbox*/ None,
    )
    .await?;
    assert!(!unset_path_snapshot.contains("declare -x PATH="));
    assert!(!unset_path_snapshot.contains("ghp_unset_path_secret"));

    for real in [
        "ghp_abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGH",
        "opaque_enterprise_credential",
        "0123456789abcdef0123456789abcdef01234567",
    ] {
        std::fs::write(
            &startup,
            format!(
                "unset GH_HOST\nexport GH_ENTERPRISE_TOKEN='{real}'\nexport AUTH_HEADER=\"Bearer $GH_ENTERPRISE_TOKEN\"\n"
            ),
        )?;
        let (snapshot, credentials) = capture_snapshot(
            &shell,
            &dir.path().abs(),
            Some(&inherited_credential_broker),
            /*sandbox*/ None,
        )
        .await?;
        assert!(!snapshot.contains(real));
        let snapshot_file = ShellSnapshotFile {
            path: dir.path().join("hostless-snapshot.sh").abs(),
            credentials,
        };
        let policy = &inherited_credential_broker.shell_environment_policy;
        let mut restored = HashMap::new();
        let context = snapshot_file.restore_credentials(&mut restored, policy);
        restored = context
            .prepare_child_environment(&network_proxy, restored, /*environment_id*/ None)?
            .env;
        snapshot_file.restore_fail_open_aliases(&mut restored, /*environment_id*/ None);
        assert_eq!(
            restored.get("GH_ENTERPRISE_TOKEN").map(String::as_str),
            Some(real)
        );

        let mut excluded = policy.clone();
        excluded
            .exclude
            .push(EnvironmentVariablePattern::new_case_insensitive(
                "GH_ENTERPRISE_TOKEN",
            ));
        let mut filtered = HashMap::new();
        let _ = snapshot_file.restore_credentials(&mut filtered, &excluded);
        assert!(!filtered.contains_key("GH_ENTERPRISE_TOKEN"));

        let mut overridden = policy.clone();
        overridden
            .r#set
            .insert("GH_ENTERPRISE_TOKEN".to_string(), String::new());
        let mut empty = overridden.r#set.clone();
        let _ = snapshot_file.restore_credentials(&mut empty, &overridden);
        assert_eq!(
            empty.get("GH_ENTERPRISE_TOKEN").map(String::as_str),
            Some("")
        );

        let policies = if real.starts_with("ghp_") {
            vec![excluded, overridden]
        } else {
            vec![policy.clone(), excluded, overridden]
        };
        for policy in policies {
            let broker = SnapshotCredentialBroker {
                network_proxy: network_proxy.clone(),
                shell_environment_policy: policy.clone(),
                allow_login_shell: false,
            };
            let (snapshot, credentials) = capture_snapshot(
                &shell,
                &dir.path().abs(),
                Some(&broker),
                /*sandbox*/ None,
            )
            .await?;
            assert!(!snapshot.contains(real));
            let snapshot_file = ShellSnapshotFile {
                path: dir.path().join("hostless-alias-snapshot.sh").abs(),
                credentials,
            };
            std::fs::write(&snapshot_file.path, snapshot)?;
            let mut env = HashMap::from([("AUTH_HEADER".to_string(), format!("Bearer {real}"))]);
            let context = snapshot_file.restore_credentials(&mut env, &policy);
            env = context
                .prepare_child_environment(&network_proxy, env, /*environment_id*/ None)?
                .env;
            snapshot_file.restore_fail_open_aliases(&mut env, /*environment_id*/ None);
            let replayed = Command::new("/bin/bash")
                .args([
                    "-c",
                    ". \"$1\"; printf '%s' \"${AUTH_HEADER-}\"",
                    "snapshot",
                ])
                .arg(snapshot_file.path.as_path())
                .env_clear()
                .envs(env)
                .output()?;
            assert!(replayed.status.success());
            let expected =
                if policy.exclude.is_empty() && !policy.r#set.contains_key("GH_ENTERPRISE_TOKEN") {
                    format!("Bearer {real}")
                } else {
                    String::new()
                };
            assert_eq!(String::from_utf8(replayed.stdout)?, expected);
        }

        restored.insert("GH_HOST".to_string(), "github.later.example".to_string());
        network_proxy.apply_to_env(&mut restored);
        assert_ne!(
            restored.get("GH_ENTERPRISE_TOKEN").map(String::as_str),
            Some(real)
        );
    }

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn try_create_creates_and_deletes_snapshot_file() -> Result<()> {
    let dir = tempdir()?;
    let shell = Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
    };

    let snapshot = ShellSnapshot::try_create(
        &dir.path().abs(),
        ThreadId::new(),
        &dir.path().abs(),
        &shell,
        /*state_db*/ None,
        /*credential_broker*/ None,
        /*sandbox*/ None,
    )
    .await
    .expect("snapshot should be created");
    let path = snapshot.path.clone();
    assert!(path.exists());

    drop(snapshot);

    assert!(!path.exists());

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn try_create_uses_distinct_generation_paths() -> Result<()> {
    let dir = tempdir()?;
    let session_id = ThreadId::new();
    let shell = Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
    };

    let initial_snapshot = ShellSnapshot::try_create(
        &dir.path().abs(),
        session_id,
        &dir.path().abs(),
        &shell,
        /*state_db*/ None,
        /*credential_broker*/ None,
        /*sandbox*/ None,
    )
    .await
    .expect("initial snapshot should be created");
    let refreshed_snapshot = ShellSnapshot::try_create(
        &dir.path().abs(),
        session_id,
        &dir.path().abs(),
        &shell,
        /*state_db*/ None,
        /*credential_broker*/ None,
        /*sandbox*/ None,
    )
    .await
    .expect("refreshed snapshot should be created");
    let initial_path = initial_snapshot.path.clone();
    let refreshed_path = refreshed_snapshot.path.clone();
    assert_ne!(initial_path, refreshed_path);
    assert_eq!(initial_path.exists(), true);
    assert_eq!(refreshed_path.exists(), true);

    drop(initial_snapshot);

    assert_eq!(initial_path.exists(), false);
    assert_eq!(refreshed_path.exists(), true);

    drop(refreshed_snapshot);

    assert_eq!(refreshed_path.exists(), false);

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn snapshot_shell_does_not_inherit_stdin() -> Result<()> {
    let _stdin_guard = BlockingStdinPipe::install()?;

    let dir = tempdir()?;
    let home = dir.path().abs();
    let read_status_path = home.join("stdin-read-status");
    let read_status_display = read_status_path.display();
    // Persist the startup `read` exit status so the test can assert whether
    // bash saw EOF on stdin after the snapshot process exits.
    let bashrc = format!("read -t 1 -r ignored\nprintf '%s' \"$?\" > \"{read_status_display}\"\n");
    fs::write(home.join(".bashrc"), bashrc).await?;

    let shell = Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
    };

    let home_display = home.display();
    let script = format!(
        "HOME=\"{home_display}\"; export HOME; {}",
        snapshot_capture_script(
            ShellType::Bash,
            SnapshotCaptureOptions {
                startup: SnapshotStartup::Interactive,
                declarations: true,
                environment: false,
            }
        )
        .expect("bash supports snapshots")
    );
    let output = run_script_with_timeout(
        &shell,
        &script,
        Duration::from_secs(2),
        SnapshotShellMode::Login,
        &home,
        /*credential_broker*/ None,
        /*sandbox*/ None,
    )
    .await
    .context("run snapshot command")?;
    let read_status = fs::read_to_string(&read_status_path)
        .await
        .context("read stdin probe status")?;

    assert_eq!(
        read_status, "1",
        "expected shell startup read to see EOF on stdin; status={read_status:?}"
    );

    assert!(
        output.contains("# Snapshot file"),
        "expected snapshot marker in output; output={output:?}"
    );

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn timed_out_snapshot_shell_is_terminated() -> Result<()> {
    use std::process::Stdio;
    use tokio::time::Duration as TokioDuration;
    use tokio::time::Instant;
    use tokio::time::sleep;

    let dir = tempdir()?;
    let pid_path = dir.path().join("pid");
    let script = format!("echo $$ > \"{}\"; sleep 30", pid_path.display());

    let shell = Shell {
        shell_type: ShellType::Sh,
        shell_path: PathBuf::from("/bin/sh"),
    };

    let err = run_script_with_timeout(
        &shell,
        &script,
        Duration::from_secs(1),
        SnapshotShellMode::Login,
        &dir.path().abs(),
        /*credential_broker*/ None,
        /*sandbox*/ None,
    )
    .await
    .expect_err("snapshot shell should time out");
    assert!(
        err.to_string().contains("timed out"),
        "expected timeout error, got {err:?}"
    );

    let pid = fs::read_to_string(&pid_path)
        .await
        .expect("snapshot shell writes its pid before timing out")
        .trim()
        .parse::<i32>()?;

    let deadline = Instant::now() + TokioDuration::from_secs(1);
    loop {
        let kill_status = StdCommand::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()?;
        if !kill_status.success() {
            break;
        }
        if Instant::now() >= deadline {
            panic!("timed out snapshot shell is still alive after grace period");
        }
        sleep(TokioDuration::from_millis(50)).await;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn macos_zsh_snapshot_includes_sections() -> Result<()> {
    let snapshot = get_snapshot(ShellType::Zsh).await?;
    assert_posix_snapshot_sections(&snapshot);
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn linux_bash_snapshot_includes_sections() -> Result<()> {
    let snapshot = get_snapshot(ShellType::Bash).await?;
    assert_posix_snapshot_sections(&snapshot);
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn linux_sh_snapshot_includes_sections() -> Result<()> {
    let snapshot = get_snapshot(ShellType::Sh).await?;
    assert_posix_snapshot_sections(&snapshot);
    Ok(())
}

#[cfg(target_os = "windows")]
#[ignore]
#[tokio::test]
async fn windows_powershell_snapshot_includes_sections() -> Result<()> {
    let snapshot = get_snapshot(ShellType::PowerShell).await?;
    assert!(snapshot.contains("# Snapshot file"));
    assert!(snapshot.contains("aliases "));
    assert!(snapshot.contains("exports "));
    Ok(())
}

async fn write_rollout_stub(codex_home: &Path, session_id: ThreadId) -> Result<PathBuf> {
    let dir = codex_home
        .join("sessions")
        .join("2025")
        .join("01")
        .join("01");
    fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("rollout-2025-01-01T00-00-00-{session_id}.jsonl"));
    fs::write(&path, "").await?;
    Ok(path)
}

#[tokio::test]
async fn cleanup_stale_snapshots_removes_orphans_and_keeps_live() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = dir.path().abs();
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
    fs::create_dir_all(&snapshot_dir).await?;

    let live_session = ThreadId::new();
    let orphan_session = ThreadId::new();
    let live_snapshot = snapshot_dir.join(format!("{live_session}.123.sh"));
    let orphan_snapshot = snapshot_dir.join(format!("{orphan_session}.456.sh"));
    let invalid_snapshot = snapshot_dir.join("not-a-snapshot.txt");

    write_rollout_stub(&codex_home, live_session).await?;
    fs::write(&live_snapshot, "live").await?;
    fs::write(&orphan_snapshot, "orphan").await?;
    fs::write(&invalid_snapshot, "invalid").await?;

    cleanup_stale_snapshots(&codex_home, ThreadId::new(), /*state_db*/ None).await?;

    assert_eq!(live_snapshot.exists(), true);
    assert_eq!(orphan_snapshot.exists(), false);
    assert_eq!(invalid_snapshot.exists(), false);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn cleanup_stale_snapshots_removes_stale_rollouts() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = dir.path().abs();
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
    fs::create_dir_all(&snapshot_dir).await?;

    let stale_session = ThreadId::new();
    let stale_snapshot = snapshot_dir.join(format!("{stale_session}.123.sh"));
    let rollout_path = write_rollout_stub(&codex_home, stale_session).await?;
    fs::write(&stale_snapshot, "stale").await?;

    set_file_mtime(&rollout_path, SNAPSHOT_RETENTION + Duration::from_secs(60))?;

    cleanup_stale_snapshots(&codex_home, ThreadId::new(), /*state_db*/ None).await?;

    assert_eq!(stale_snapshot.exists(), false);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn cleanup_stale_snapshots_skips_active_session() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = dir.path().abs();
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
    fs::create_dir_all(&snapshot_dir).await?;

    let active_session = ThreadId::new();
    let active_snapshot = snapshot_dir.join(format!("{active_session}.123.sh"));
    let rollout_path = write_rollout_stub(&codex_home, active_session).await?;
    fs::write(&active_snapshot, "active").await?;

    set_file_mtime(&rollout_path, SNAPSHOT_RETENTION + Duration::from_secs(60))?;

    cleanup_stale_snapshots(&codex_home, active_session, /*state_db*/ None).await?;

    assert_eq!(active_snapshot.exists(), true);
    Ok(())
}

#[cfg(unix)]
fn set_file_mtime(path: &Path, age: Duration) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs()
        .saturating_sub(age.as_secs());
    let tv_sec = now
        .try_into()
        .map_err(|_| anyhow!("Snapshot mtime is out of range for libc::timespec"))?;
    let ts = libc::timespec { tv_sec, tv_nsec: 0 };
    let times = [ts, ts];
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    let result = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
