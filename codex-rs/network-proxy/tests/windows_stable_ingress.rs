#![cfg(target_os = "windows")]

use codex_network_proxy::ConfigReloader;
use codex_network_proxy::ConfigReloaderFuture;
use codex_network_proxy::ConfigState;
use codex_network_proxy::NetworkDecision;
use codex_network_proxy::NetworkMode;
use codex_network_proxy::NetworkPolicyDecider;
use codex_network_proxy::NetworkPolicyRequest;
use codex_network_proxy::NetworkProtocol;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::NetworkProxyState;
use codex_network_proxy::build_config_state;
use codex_windows_sandbox::ConsoleMode;
use codex_windows_sandbox::LocalSid;
use codex_windows_sandbox::create_process_as_user;
use codex_windows_sandbox::create_readonly_token_with_caps_and_user_from;
use codex_windows_sandbox::get_current_token_for_restriction;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use windows_sys::Win32::System::Threading::GetExitCodeProcess;
use windows_sys::Win32::System::Threading::TerminateProcess;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

const CHILD_MODE_ENV: &str = "CODEX_WINDOWS_PROXY_TEST_CHILD";
const HTTP_ADDR_ENV: &str = "CODEX_WINDOWS_PROXY_TEST_HTTP_ADDR";
const SOCKS_ADDR_ENV: &str = "CODEX_WINDOWS_PROXY_TEST_SOCKS_ADDR";
const ORIGIN_PORT_ENV: &str = "CODEX_WINDOWS_PROXY_TEST_ORIGIN_PORT";
const ALLOWED_HOST_ENV: &str = "CODEX_WINDOWS_PROXY_TEST_ALLOWED_HOST";
const DENIED_HOST_ENV: &str = "CODEX_WINDOWS_PROXY_TEST_DENIED_HOST";
const FIRST_ENVIRONMENT_ID: &str = "first-environment";
const SECOND_ENVIRONMENT_ID: &str = "second-environment";
const DECIDER_DENIED_HOST: &str = "not-allowed.invalid";
const CHILD_TIMEOUT_MS: u32 = 30_000;
const WAIT_OBJECT_0: u32 = 0;

#[derive(Clone)]
struct StaticReloader(ConfigState);

impl ConfigReloader for StaticReloader {
    fn source_label(&self) -> String {
        "test config".to_string()
    }

    fn maybe_reload(&self) -> ConfigReloaderFuture<'_, Option<ConfigState>> {
        Box::pin(async { Ok(None) })
    }

    fn reload_now(&self) -> ConfigReloaderFuture<'_, ConfigState> {
        let state = self.0.clone();
        Box::pin(async move { Ok(state) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restricted_tokens_select_stable_routes_and_cleanup() -> anyhow::Result<()> {
    let (origin_port, origin_task) = start_http_origin().await?;
    let (first_decider, first_requests) = recording_decider();
    let (second_decider, second_requests) = recording_decider();
    let first_requested = requested_addrs()?;
    let first = build_proxy(
        first_requested,
        "localhost",
        /*enable_socks5*/ false,
        Some(first_decider),
    )
    .await?;
    let initial_addrs = (first.http_addr(), first.socks_addr());
    let first_handle = first.run().await?;
    let first_sid = first
        .network_proxy_restricting_sid(None)
        .expect("running proxy should have a route SID");
    first.prepare_for_optional_environment(HashMap::new(), Some(FIRST_ENVIRONMENT_ID))?;
    let first_environment_sid = first
        .network_proxy_restricting_sid(Some(FIRST_ENVIRONMENT_ID))
        .expect("first environment should have a route SID");

    let second_requested = requested_addrs()?;
    assert_ne!(second_requested.0, initial_addrs.0);
    let second = build_proxy(
        second_requested,
        "127.0.0.1",
        /*enable_socks5*/ true,
        Some(second_decider),
    )
    .await?;
    let stable_addrs = (second.http_addr(), second.socks_addr());
    assert_eq!(stable_addrs.0, initial_addrs.0);
    assert_eq!(stable_addrs.1, second_requested.1);
    assert_eq!((first.http_addr(), first.socks_addr()), stable_addrs);
    let second_handle = second.run().await?;
    let second_sid = second
        .network_proxy_restricting_sid(None)
        .expect("running proxy should have a route SID");
    assert_ne!(second_sid, first_sid);

    second.prepare_for_optional_environment(HashMap::new(), Some(SECOND_ENVIRONMENT_ID))?;
    let second_environment_sid = second
        .network_proxy_restricting_sid(Some(SECOND_ENVIRONMENT_ID))
        .expect("second environment should have a route SID");

    run_restricted_child(
        &first_environment_sid,
        stable_addrs,
        origin_port,
        Some(("localhost", DECIDER_DENIED_HOST)),
        /*expect_socks*/ false,
    )
    .await?;
    assert_recorded_requests(
        &first_requests,
        FIRST_ENVIRONMENT_ID,
        DECIDER_DENIED_HOST,
        origin_port,
        &[NetworkProtocol::Http],
    );
    assert!(
        second_requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );

    run_restricted_child(
        &second_environment_sid,
        stable_addrs,
        origin_port,
        Some(("127.0.0.1", DECIDER_DENIED_HOST)),
        /*expect_socks*/ true,
    )
    .await?;
    assert_recorded_requests(
        &first_requests,
        FIRST_ENVIRONMENT_ID,
        DECIDER_DENIED_HOST,
        origin_port,
        &[NetworkProtocol::Http],
    );
    assert_recorded_requests(
        &second_requests,
        SECOND_ENVIRONMENT_ID,
        DECIDER_DENIED_HOST,
        origin_port,
        &[NetworkProtocol::Http, NetworkProtocol::Socks5Tcp],
    );

    run_restricted_child(
        &first_sid,
        stable_addrs,
        origin_port,
        Some(("localhost", "127.0.0.1")),
        /*expect_socks*/ false,
    )
    .await?;
    run_restricted_child(
        &second_sid,
        stable_addrs,
        origin_port,
        Some(("127.0.0.1", "localhost")),
        /*expect_socks*/ true,
    )
    .await?;

    first_handle.shutdown().await?;
    run_restricted_child(
        &first_sid,
        stable_addrs,
        origin_port,
        None,
        /*expect_socks*/ false,
    )
    .await?;
    run_restricted_child(
        &first_environment_sid,
        stable_addrs,
        origin_port,
        None,
        /*expect_socks*/ false,
    )
    .await?;
    run_restricted_child(
        &second_sid,
        stable_addrs,
        origin_port,
        Some(("127.0.0.1", "localhost")),
        /*expect_socks*/ true,
    )
    .await?;

    second_handle.shutdown().await?;
    drop((first, second));

    let third_requested = requested_addrs()?;
    assert_ne!(third_requested, stable_addrs);
    let third = build_proxy(
        third_requested,
        "localhost",
        /*enable_socks5*/ false,
        None,
    )
    .await?;
    assert_eq!((third.http_addr(), third.socks_addr()), stable_addrs);
    let third_handle = third.run().await?;
    let third_sid = third
        .network_proxy_restricting_sid(None)
        .expect("running proxy should have a route SID");
    assert_ne!(third_sid, first_sid);
    assert_ne!(third_sid, second_sid);

    run_restricted_child(
        &third_sid,
        stable_addrs,
        origin_port,
        Some(("localhost", "127.0.0.1")),
        /*expect_socks*/ false,
    )
    .await?;
    drop(third_handle);
    assert_eq!(third.network_proxy_restricting_sid(None), None);
    run_restricted_child(
        &third_sid,
        stable_addrs,
        origin_port,
        None,
        /*expect_socks*/ false,
    )
    .await?;
    origin_task.abort();
    Ok(())
}

#[test]
fn restricted_child_exercises_http_and_socks() -> anyhow::Result<()> {
    let Ok(mode) = std::env::var(CHILD_MODE_ENV) else {
        return Ok(());
    };
    let http_addr = required_env(HTTP_ADDR_ENV)?.parse::<SocketAddr>()?;
    let socks_addr = required_env(SOCKS_ADDR_ENV)?.parse::<SocketAddr>()?;
    let origin_port = required_env(ORIGIN_PORT_ENV)?.parse::<u16>()?;

    if mode == "missing-route" {
        let authority = format!("localhost:{origin_port}");
        assert!(http_status(http_addr, &authority).is_err());
        assert!(socks_status(socks_addr, "localhost", origin_port).is_err());
        return Ok(());
    }

    let allowed_host = required_env(ALLOWED_HOST_ENV)?;
    let denied_host = required_env(DENIED_HOST_ENV)?;
    let allowed_authority = format!("{allowed_host}:{origin_port}");
    let denied_authority = format!("{denied_host}:{origin_port}");
    assert_eq!(http_status(http_addr, &allowed_authority)?, 200);
    assert_eq!(http_status(http_addr, &denied_authority)?, 403);
    if mode == "http-only" {
        assert!(socks_status(socks_addr, &allowed_host, origin_port).is_err());
        return Ok(());
    }
    assert_eq!(
        socks_status(socks_addr, &allowed_host, origin_port)?,
        SocksOutcome::Connected
    );
    assert!(matches!(
        socks_status(socks_addr, &denied_host, origin_port)?,
        SocksOutcome::Denied(_)
    ));
    Ok(())
}

async fn build_proxy(
    requested_addrs: (SocketAddr, SocketAddr),
    allowed_domain: &str,
    enable_socks5: bool,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
) -> anyhow::Result<NetworkProxy> {
    let (http_addr, socks_addr) = requested_addrs;
    let mut config = NetworkProxyConfig {
        enabled: true,
        proxy_url: format!("http://{http_addr}"),
        socks_url: format!("socks5://{socks_addr}"),
        enable_socks5,
        enable_socks5_udp: false,
        allow_local_binding: true,
        mode: NetworkMode::Full,
        ..NetworkProxyConfig::default()
    };
    config.set_allowed_domains(vec![allowed_domain.to_string()]);
    let config_state = build_config_state(config, Default::default())?;
    let reloader = Arc::new(StaticReloader(config_state.clone()));
    let state = Arc::new(NetworkProxyState::with_reloader(config_state, reloader));
    let mut builder = NetworkProxy::builder().state(state);
    if let Some(policy_decider) = policy_decider {
        builder = builder.policy_decider_arc(policy_decider);
    }
    builder.build().await
}

fn recording_decider() -> (
    Arc<dyn NetworkPolicyDecider>,
    Arc<Mutex<Vec<NetworkPolicyRequest>>>,
) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded_requests = Arc::clone(&requests);
    let decider: Arc<dyn NetworkPolicyDecider> = Arc::new(move |request: NetworkPolicyRequest| {
        recorded_requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request);
        async { NetworkDecision::deny("integration test denial") }
    });
    (decider, requests)
}

fn assert_recorded_requests(
    requests: &Arc<Mutex<Vec<NetworkPolicyRequest>>>,
    environment_id: &str,
    host: &str,
    port: u16,
    expected_protocols: &[NetworkProtocol],
) {
    let requests = requests
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(requests.len(), expected_protocols.len());
    let actual_protocols = requests
        .iter()
        .map(|request| request.protocol)
        .collect::<Vec<_>>();
    assert_eq!(actual_protocols, expected_protocols);
    assert!(requests.iter().all(|request| {
        request.environment_id.as_deref() == Some(environment_id)
            && request.host == host
            && request.port == port
    }));
}

fn requested_addrs() -> std::io::Result<(SocketAddr, SocketAddr)> {
    let http = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let socks = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    Ok((http.local_addr()?, socks.local_addr()?))
}

async fn start_http_origin() -> std::io::Result<(u16, tokio::task::JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let task = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut request = [0_u8; 1024];
                let _ = stream.read(&mut request).await;
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK",
                    )
                    .await;
            });
        }
    });
    Ok((port, task))
}

async fn run_restricted_child(
    route_sid: &str,
    proxy_addrs: (SocketAddr, SocketAddr),
    origin_port: u16,
    policy: Option<(&str, &str)>,
    expect_socks: bool,
) -> anyhow::Result<()> {
    let route_sid = route_sid.to_string();
    let policy = policy.map(|(allowed, denied)| (allowed.to_string(), denied.to_string()));
    tokio::task::spawn_blocking(move || {
        run_restricted_child_blocking(&route_sid, proxy_addrs, origin_port, policy, expect_socks)
    })
    .await??;
    Ok(())
}

fn run_restricted_child_blocking(
    route_sid: &str,
    (http_addr, socks_addr): (SocketAddr, SocketAddr),
    origin_port: u16,
    policy: Option<(String, String)>,
    expect_socks: bool,
) -> anyhow::Result<()> {
    let route_sid = LocalSid::from_string(route_sid)?;
    let capability_sid = LocalSid::from_string("S-1-5-21-10-20-30-40")?;
    let base_token = unsafe {
        OwnedHandle::from_raw_handle(get_current_token_for_restriction()? as *mut std::ffi::c_void)
    };
    let restricted_token = unsafe {
        create_readonly_token_with_caps_and_user_from(
            base_token.as_raw_handle() as isize,
            &[capability_sid.as_ptr()],
            &[route_sid.as_ptr()],
        )?
    };
    let restricted_token =
        unsafe { OwnedHandle::from_raw_handle(restricted_token as *mut std::ffi::c_void) };

    let mut env = std::env::vars().collect::<HashMap<_, _>>();
    env.insert(HTTP_ADDR_ENV.to_string(), http_addr.to_string());
    env.insert(SOCKS_ADDR_ENV.to_string(), socks_addr.to_string());
    env.insert(ORIGIN_PORT_ENV.to_string(), origin_port.to_string());
    match policy {
        Some((allowed, denied)) => {
            let mode = if expect_socks { "policy" } else { "http-only" };
            env.insert(CHILD_MODE_ENV.to_string(), mode.to_string());
            env.insert(ALLOWED_HOST_ENV.to_string(), allowed);
            env.insert(DENIED_HOST_ENV.to_string(), denied);
        }
        None => {
            env.insert(CHILD_MODE_ENV.to_string(), "missing-route".to_string());
        }
    }

    let test_exe = std::env::current_exe()?;
    let command = vec![
        test_exe.to_string_lossy().into_owned(),
        "--exact".to_string(),
        "restricted_child_exercises_http_and_socks".to_string(),
        "--nocapture".to_string(),
        "--test-threads=1".to_string(),
    ];
    let cwd = std::env::current_dir()?;
    let spawned = unsafe {
        create_process_as_user(
            restricted_token.as_raw_handle() as isize,
            &command,
            &cwd,
            &env,
            /*logs_base_dir*/ None,
            /*stdio*/ None,
            /*console_mode*/ ConsoleMode::Inherit,
            /*use_private_desktop*/ false,
        )?
    };
    let process = unsafe {
        OwnedHandle::from_raw_handle(spawned.process_info.hProcess as *mut std::ffi::c_void)
    };
    let _thread = unsafe {
        OwnedHandle::from_raw_handle(spawned.process_info.hThread as *mut std::ffi::c_void)
    };

    let wait = unsafe {
        WaitForSingleObject(
            process.as_raw_handle() as isize,
            /*dwMilliseconds*/ CHILD_TIMEOUT_MS,
        )
    };
    if wait != WAIT_OBJECT_0 {
        unsafe {
            TerminateProcess(process.as_raw_handle() as isize, 1);
        }
    }
    let mut exit_code = 1_u32;
    unsafe {
        GetExitCodeProcess(process.as_raw_handle() as isize, &mut exit_code);
    }
    anyhow::ensure!(
        wait == WAIT_OBJECT_0 && exit_code == 0,
        "restricted proxy child failed (wait={wait}, exit={exit_code})"
    );
    Ok(())
}

fn required_env(key: &str) -> anyhow::Result<String> {
    std::env::var(key).map_err(Into::into)
}

fn http_status(proxy_addr: SocketAddr, authority: &str) -> std::io::Result<u16> {
    let mut stream = TcpStream::connect(proxy_addr)?;
    configure_stream(&stream)?;
    write!(
        stream,
        "GET http://{authority}/ HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )?;
    read_http_status(&mut stream)
}

#[derive(Debug, Eq, PartialEq)]
enum SocksOutcome {
    Connected,
    Denied(u8),
}

fn socks_status(
    proxy_addr: SocketAddr,
    host: &str,
    origin_port: u16,
) -> std::io::Result<SocksOutcome> {
    let mut stream = TcpStream::connect(proxy_addr)?;
    configure_stream(&stream)?;
    stream.write_all(&[5, 1, 0])?;
    let mut greeting = [0_u8; 2];
    stream.read_exact(&mut greeting)?;
    if greeting != [5, 0] {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "SOCKS5 proxy rejected no-authentication method",
        ));
    }

    let mut request = vec![5, 1, 0];
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        request.push(1);
        request.extend_from_slice(&ip.octets());
    } else {
        let host_len = u8::try_from(host.len()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "SOCKS5 hostname too long")
        })?;
        request.extend_from_slice(&[3, host_len]);
        request.extend_from_slice(host.as_bytes());
    }
    request.extend_from_slice(&origin_port.to_be_bytes());
    stream.write_all(&request)?;

    let mut reply = [0_u8; 4];
    stream.read_exact(&mut reply)?;
    if reply[0] != 5 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid SOCKS5 response version",
        ));
    }
    if reply[1] != 0 {
        return Ok(SocksOutcome::Denied(reply[1]));
    }
    consume_socks_bound_address(&mut stream, reply[3])?;
    Ok(SocksOutcome::Connected)
}

fn consume_socks_bound_address(stream: &mut TcpStream, address_type: u8) -> std::io::Result<()> {
    let address_len = match address_type {
        1 => 4,
        3 => {
            let mut len = [0_u8; 1];
            stream.read_exact(&mut len)?;
            usize::from(len[0])
        }
        4 => 16,
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid SOCKS5 bound address type",
            ));
        }
    };
    let mut address_and_port = vec![0_u8; address_len + 2];
    stream.read_exact(&mut address_and_port)
}

fn configure_stream(stream: &TcpStream) -> std::io::Result<()> {
    let timeout = Some(Duration::from_secs(5));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)
}

fn read_http_status(stream: &mut TcpStream) -> std::io::Result<u16> {
    let mut status_line = String::new();
    if BufReader::new(stream).read_line(&mut status_line)? == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "proxy closed before an HTTP status line",
        ));
    }
    status_line
        .split_ascii_whitespace()
        .nth(1)
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "missing HTTP status code")
        })?
        .parse::<u16>()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}
