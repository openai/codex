use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

#[cfg(target_os = "macos")]
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::PROXY_ATTRIBUTION_TOKEN_ENV_KEY;
use codex_network_proxy::RemoteNetworkProxyConfig;
use codex_network_proxy::RemoteNetworkProxyLaunchConfig;
#[cfg(windows)]
use codex_protocol::config_types::WindowsSandboxLevel;
#[cfg(any(unix, windows))]
use codex_protocol::models::PermissionProfile;
#[cfg(target_os = "linux")]
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

use super::prepare_exec_request;
#[cfg(unix)]
use crate::CODEX_ARG0_EXEC_HELPER_ARG1;
use crate::ExecParams;
#[cfg(any(unix, windows))]
use crate::ExecServerRuntimePaths;
#[cfg(any(unix, windows))]
use crate::FileSystemSandboxContext;
use crate::ProcessId;

#[cfg(unix)]
#[tokio::test]
async fn sandbox_request_wraps_native_argv_on_executor() {
    let cwd: AbsolutePathBuf = std::env::current_dir()
        .expect("current directory")
        .try_into()
        .expect("absolute cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let self_exe = std::env::current_exe().expect("current executable");
    let runtime_paths =
        ExecServerRuntimePaths::new(self_exe.clone(), Some(self_exe)).expect("runtime paths");
    let sandbox = FileSystemSandboxContext::from_permission_profile_with_cwd(
        PermissionProfile::workspace_write(),
        cwd_uri.clone(),
    );
    let params = ExecParams {
        process_id: ProcessId::from("process-1"),
        argv: vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "pwd".to_string(),
        ],
        cwd: cwd_uri,
        env_policy: None,
        env: HashMap::new(),
        tty: false,
        pipe_stdin: false,
        arg0: None,
        sandbox: Some(sandbox),
        enforce_managed_network: false,
        managed_network: None,
        network_proxy: None,
    };

    let prepared = prepare_exec_request(
        &params,
        HashMap::new(),
        Some(&runtime_paths),
        /*network_policy_decider*/ None,
    )
    .await
    .expect("prepare sandboxed request");

    assert_ne!(prepared.command, params.argv);
    assert_eq!(prepared.cwd, cwd);
    #[cfg(target_os = "linux")]
    {
        assert_eq!(
            prepared.command.first(),
            Some(&runtime_paths.codex_self_exe.to_string_lossy().into_owned())
        );
        let permission_profile_json = prepared
            .command
            .iter()
            .position(|arg| arg == "--permission-profile")
            .and_then(|index| prepared.command.get(index + 1))
            .expect("sandbox wrapper permission profile");
        let permission_profile: PermissionProfile =
            serde_json::from_str(permission_profile_json).expect("permission profile JSON");
        assert_eq!(
            permission_profile,
            PermissionProfile::workspace_write()
                .materialize_project_roots_with_workspace_roots(std::slice::from_ref(&cwd))
        );
    }
    #[cfg(target_os = "macos")]
    assert_eq!(
        prepared.command.first().map(String::as_str),
        Some("/usr/bin/sandbox-exec")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn sandbox_request_routes_custom_arg0_to_inner_helper() {
    let cwd: AbsolutePathBuf = std::env::current_dir()
        .expect("current directory")
        .try_into()
        .expect("absolute cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let self_exe = std::env::current_exe().expect("current executable");
    let runtime_paths =
        ExecServerRuntimePaths::new(self_exe.clone(), Some(self_exe)).expect("runtime paths");
    let sandbox = FileSystemSandboxContext::from_permission_profile_with_cwd(
        PermissionProfile::workspace_write(),
        cwd_uri.clone(),
    );
    let params = ExecParams {
        process_id: ProcessId::from("process-custom-arg0"),
        argv: vec!["/bin/sh".to_string(), "-c".to_string(), "true".to_string()],
        cwd: cwd_uri,
        env_policy: None,
        env: HashMap::new(),
        tty: false,
        pipe_stdin: false,
        arg0: Some("custom-arg0".to_string()),
        sandbox: Some(sandbox),
        enforce_managed_network: false,
        managed_network: None,
        network_proxy: None,
    };

    let prepared = prepare_exec_request(
        &params,
        HashMap::new(),
        Some(&runtime_paths),
        /*network_policy_decider*/ None,
    )
    .await
    .expect("prepare sandboxed request");
    let helper_mode = prepared
        .command
        .iter()
        .position(|arg| arg == CODEX_ARG0_EXEC_HELPER_ARG1)
        .expect("sandboxed command should invoke arg0 helper");

    assert_eq!(
        prepared.command[helper_mode..],
        [
            CODEX_ARG0_EXEC_HELPER_ARG1,
            "custom-arg0",
            "/bin/sh",
            "-c",
            "true",
        ]
    );
    #[cfg(target_os = "linux")]
    assert_eq!(prepared.arg0, Some(CODEX_LINUX_SANDBOX_ARG0.to_string()));
    #[cfg(target_os = "macos")]
    assert_eq!(prepared.arg0, None);
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn sandbox_request_allows_prepared_managed_proxy_port() {
    let cwd: AbsolutePathBuf = std::env::current_dir()
        .expect("current directory")
        .try_into()
        .expect("absolute cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let self_exe = std::env::current_exe().expect("current executable");
    let runtime_paths =
        ExecServerRuntimePaths::new(self_exe.clone(), Some(self_exe)).expect("runtime paths");
    let sandbox = FileSystemSandboxContext::from_permission_profile_with_cwd(
        PermissionProfile::workspace_write(),
        cwd_uri.clone(),
    );
    let params = ExecParams {
        process_id: ProcessId::from("process-managed-network"),
        argv: vec!["/usr/bin/true".to_string()],
        cwd: cwd_uri,
        env_policy: None,
        env: HashMap::new(),
        tty: false,
        pipe_stdin: false,
        arg0: None,
        sandbox: Some(sandbox),
        enforce_managed_network: true,
        managed_network: Some(ManagedNetworkSandboxContext {
            loopback_ports: vec![43123],
            allow_local_binding: false,
        }),
        network_proxy: None,
    };

    let prepared = prepare_exec_request(
        &params,
        HashMap::new(),
        Some(&runtime_paths),
        /*network_policy_decider*/ None,
    )
    .await
    .expect("prepare managed-network sandbox request");
    let policy = prepared
        .command
        .windows(2)
        .find_map(|args| (args[0] == "-p").then_some(args[1].as_str()))
        .expect("Seatbelt policy argument");

    assert!(policy.contains("(allow network-outbound (remote ip \"localhost:43123\"))"));
}

#[tokio::test]
async fn native_request_preserves_native_launch_fields() {
    let cwd: AbsolutePathBuf = std::env::current_dir()
        .expect("current directory")
        .try_into()
        .expect("absolute cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let env = HashMap::from([("TEST_ENV".to_string(), "value".to_string())]);
    let params = ExecParams {
        process_id: ProcessId::from("process-1"),
        argv: vec!["echo".to_string(), "hello".to_string()],
        cwd: cwd_uri,
        env_policy: None,
        env: HashMap::new(),
        tty: false,
        pipe_stdin: false,
        arg0: Some("custom-arg0".to_string()),
        sandbox: None,
        enforce_managed_network: false,
        managed_network: None,
        network_proxy: None,
    };

    let prepared = prepare_exec_request(
        &params,
        env.clone(),
        /*runtime_paths*/ None,
        /*network_policy_decider*/ None,
    )
    .await
    .expect("prepare native request");

    assert_eq!(prepared.command, params.argv);
    assert_eq!(prepared.cwd, cwd);
    assert_eq!(prepared.env, env);
    assert_eq!(prepared.arg0, params.arg0);
}

#[tokio::test]
async fn native_request_handles_remote_proxy_config_for_platform() {
    let cwd: AbsolutePathBuf = std::env::current_dir()
        .expect("current directory")
        .try_into()
        .expect("absolute cwd");
    let mut config = NetworkProxyConfig {
        enabled: true,
        ..NetworkProxyConfig::default()
    };
    config.set_allowed_domains(vec!["allowed.example".to_string()]);
    let proxy_config = RemoteNetworkProxyConfig::from_effective_config(&config)
        .expect("supported remote proxy config");
    let params = ExecParams {
        process_id: ProcessId::from("process-remote-proxy"),
        argv: vec!["echo".to_string(), "hello".to_string()],
        cwd: PathUri::from_abs_path(&cwd),
        env_policy: None,
        env: HashMap::new(),
        tty: false,
        pipe_stdin: false,
        arg0: None,
        sandbox: None,
        enforce_managed_network: false,
        managed_network: None,
        network_proxy: Some(
            RemoteNetworkProxyLaunchConfig::new(proxy_config)
                .for_execution("remote".to_string(), "execution-1".to_string()),
        ),
    };
    let stale_proxy = "http://127.0.0.1:9".to_string();
    let env = HashMap::from([
        ("HTTP_PROXY".to_string(), stale_proxy.clone()),
        ("TEST_ENV".to_string(), "value".to_string()),
        (
            PROXY_ATTRIBUTION_TOKEN_ENV_KEY.to_string(),
            "foreign-token".to_string(),
        ),
    ]);

    let prepared = prepare_exec_request(
        &params, env, /*runtime_paths*/ None, /*network_policy_decider*/ None,
    )
    .await
    .expect("prepare request with executor-local proxy");

    if cfg!(target_os = "windows") {
        assert_eq!(prepared.env.get("HTTP_PROXY"), None);
        assert_eq!(prepared.env.get("TEST_ENV"), Some(&"value".to_string()));
        assert!(prepared.network_proxy_handle.is_none());
        return;
    }

    let http_proxy = prepared.env.get("HTTP_PROXY").expect("HTTP proxy env");
    assert_ne!(http_proxy, &stale_proxy);
    assert!(http_proxy.starts_with("http://127.0.0.1:"));
    assert!(!prepared.env.contains_key(PROXY_ATTRIBUTION_TOKEN_ENV_KEY));
    let proxy_addr: SocketAddr = http_proxy
        .strip_prefix("http://")
        .expect("HTTP proxy scheme")
        .parse()
        .expect("HTTP proxy address");
    let mut stream = tokio::net::TcpStream::connect(proxy_addr)
        .await
        .expect("connect to executor proxy");
    stream
        .write_all(b"CONNECT blocked.example:443 HTTP/1.1\r\nHost: blocked.example:443\r\n\r\n")
        .await
        .expect("write CONNECT request");
    let mut response = [0_u8; 256];
    let response_len = timeout(Duration::from_secs(2), stream.read(&mut response))
        .await
        .expect("proxy response timeout")
        .expect("read proxy response");
    assert!(String::from_utf8_lossy(&response[..response_len]).starts_with("HTTP/1.1 403"));

    prepared
        .network_proxy_handle
        .expect("running executor proxy")
        .shutdown()
        .await
        .expect("shut down executor proxy");
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn disabled_remote_proxy_config_is_rejected_before_exporting_ports() {
    let cwd: AbsolutePathBuf = std::env::current_dir()
        .expect("current directory")
        .try_into()
        .expect("absolute cwd");
    let proxy_config =
        RemoteNetworkProxyConfig::from_effective_config(&NetworkProxyConfig::default())
            .expect("serializable disabled proxy config");
    let params = ExecParams {
        process_id: ProcessId::from("process-disabled-remote-proxy"),
        argv: vec!["echo".to_string(), "hello".to_string()],
        cwd: PathUri::from_abs_path(&cwd),
        env_policy: None,
        env: HashMap::new(),
        tty: false,
        pipe_stdin: false,
        arg0: None,
        sandbox: None,
        enforce_managed_network: false,
        managed_network: None,
        network_proxy: Some(RemoteNetworkProxyLaunchConfig::new(proxy_config)),
    };

    let error = prepare_exec_request(
        &params,
        HashMap::new(),
        /*runtime_paths*/ None,
        /*network_policy_decider*/ None,
    )
    .await
    .err()
    .expect("disabled executor proxy launch must fail closed");

    assert_eq!(error.code, -32602);
    assert!(
        error
            .message
            .contains("executor-local network proxy launch requires an enabled proxy")
    );
}

#[cfg(windows)]
#[tokio::test]
async fn managed_network_selects_elevated_windows_spawn() {
    let cwd: AbsolutePathBuf = std::env::current_dir()
        .expect("current directory")
        .try_into()
        .expect("absolute cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let self_exe = std::env::current_exe().expect("current executable");
    let runtime_paths = ExecServerRuntimePaths::new(self_exe, None).expect("runtime paths");
    let permissions = PermissionProfile::read_only();
    let mut sandbox = FileSystemSandboxContext::from_permission_profile_with_cwd(
        permissions.clone(),
        cwd_uri.clone(),
    );
    sandbox.windows_sandbox_level = WindowsSandboxLevel::RestrictedToken;
    sandbox.windows_sandbox_proxy_settings_mode =
        Some(codex_sandboxing::WindowsSandboxProxySettingsMode::Preserve);
    let proxy_config = RemoteNetworkProxyConfig::from_effective_config(&NetworkProxyConfig {
        enabled: true,
        enable_socks5: false,
        ..NetworkProxyConfig::default()
    })
    .expect("supported remote proxy config");
    let params = ExecParams {
        process_id: ProcessId::from("process-managed-network"),
        argv: vec!["cmd.exe".to_string(), "/c".to_string(), "exit".to_string()],
        cwd: cwd_uri,
        env_policy: None,
        env: HashMap::new(),
        tty: false,
        pipe_stdin: false,
        arg0: None,
        sandbox: Some(sandbox),
        enforce_managed_network: true,
        managed_network: None,
        network_proxy: Some(RemoteNetworkProxyLaunchConfig::new(proxy_config)),
    };

    let mut prepared = prepare_exec_request(
        &params,
        HashMap::new(),
        Some(&runtime_paths),
        /*network_policy_decider*/ None,
    )
    .await
    .expect("prepare sandboxed request");
    {
        let spawn = prepared
            .windows_sandbox_spawn_request()
            .expect("Windows sandbox spawn request");

        assert_eq!(
            spawn.windows_sandbox_level,
            WindowsSandboxLevel::RestrictedToken
        );
        assert!(spawn.proxy_enforced);
        assert!(spawn.network_proxy_restricting_sid.is_some());
        assert_eq!(
            spawn.proxy_settings_mode,
            codex_sandboxing::WindowsSandboxProxySettingsMode::Preserve
        );
        assert_eq!(spawn.permission_profile, &permissions);
        assert_eq!(spawn.workspace_roots, std::slice::from_ref(&cwd));
    }
    prepared
        .network_proxy_handle
        .take()
        .expect("running executor proxy")
        .shutdown()
        .await
        .expect("shut down executor proxy");
}
