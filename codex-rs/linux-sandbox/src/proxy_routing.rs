use crate::proxy_lifecycle::close_fd;
use crate::proxy_lifecycle::harden_bridge_process;
use crate::proxy_lifecycle::move_fd_above_stdio;
use crate::proxy_lifecycle::receive_listener;
use crate::proxy_lifecycle::send_listener;
use codex_network_proxy::PROXY_ATTRIBUTION_TOKEN_ENV_KEY;
use codex_network_proxy::write_attribution_frame;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Shutdown;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::Semaphore;
use url::Url;

const HOST_BRIDGE_READY: u8 = 1;
const LOOPBACK_INTERFACE_NAME: &[u8] = b"lo";
const HANDOFF_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ProxyRouteSpec {
    routes: Vec<ProxyRouteEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ProxyRouteEntry {
    env_key: String,
    control_fd: libc::c_int,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlannedProxyRoute {
    env_key: String,
    endpoint: SocketAddr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProxyRoutePlan {
    routes: Vec<PlannedProxyRoute>,
    has_proxy_config: bool,
}

pub(crate) fn prepare_host_proxy_route_spec(
    port_allowlist: Option<&[u16]>,
) -> io::Result<(String, Vec<File>)> {
    let (attribution_token, plan) = extract_attribution_token_and_plan(std::env::vars().collect());
    // SAFETY: the sandbox helper is single-threaded here, before it forks bridge workers or
    // executes the user command.
    unsafe {
        std::env::remove_var(PROXY_ATTRIBUTION_TOKEN_ENV_KEY);
    }

    if plan.routes.is_empty() {
        let message = if plan.has_proxy_config {
            "managed proxy mode requires parseable loopback proxy endpoints"
        } else {
            "managed proxy mode requires proxy environment variables"
        };
        return Err(io::Error::new(io::ErrorKind::InvalidInput, message));
    }

    // Validate the entire plan before forking any host-side workers or sending
    // attribution. An empty enforced list grants no routes.
    if let Some(ports) = port_allowlist
        && plan
            .routes
            .iter()
            .any(|route| !ports.contains(&route.endpoint.port()))
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "proxy endpoint is outside the managed sandbox context",
        ));
    }

    let mut control_by_endpoint = BTreeMap::new();
    for route in &plan.routes {
        if control_by_endpoint.contains_key(&route.endpoint) {
            continue;
        }
        let control = spawn_host_bridge(
            route.endpoint,
            attribution_token.as_deref(),
            &mut control_by_endpoint,
        )?;
        control_by_endpoint.insert(route.endpoint, control);
    }

    let mut routes = Vec::with_capacity(plan.routes.len());
    for route in plan.routes {
        let Some(control) = control_by_endpoint.get(&route.endpoint) else {
            return Err(io::Error::other(format!(
                "missing bootstrap channel for endpoint {}",
                route.endpoint
            )));
        };
        routes.push(ProxyRouteEntry {
            env_key: route.env_key,
            control_fd: control.as_raw_fd(),
        });
    }

    let spec = serde_json::to_string(&ProxyRouteSpec { routes }).map_err(io::Error::other)?;
    Ok((spec, control_by_endpoint.into_values().collect()))
}

fn extract_attribution_token_and_plan(
    mut env: HashMap<String, String>,
) -> (Option<String>, ProxyRoutePlan) {
    let attribution_token = env.remove(PROXY_ATTRIBUTION_TOKEN_ENV_KEY);
    let plan = plan_proxy_routes(&env);
    (attribution_token, plan)
}

pub(crate) fn activate_proxy_routes_in_netns(serialized_spec: &str) -> io::Result<()> {
    let spec: ProxyRouteSpec = serde_json::from_str(serialized_spec).map_err(io::Error::other)?;

    if spec.routes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "proxy routing spec contained no routes",
        ));
    }

    let mut local_port_by_control = BTreeMap::new();
    for route in &spec.routes {
        if local_port_by_control.contains_key(&route.control_fd) {
            continue;
        }
        if route.control_fd <= libc::STDERR_FILENO {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "proxy bootstrap channel must not use standard descriptors",
            ));
        }
        // SAFETY: F_GETFD takes only a descriptor, with no pointer arguments;
        // an invalid inherited descriptor returns EBADF.
        let flags = unsafe { libc::fcntl(route.control_fd, libc::F_GETFD) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the outer helper left this descriptor open across exec, which
        // discarded its Rust owner. F_GETFD above confirmed it is still valid.
        // The map ensures each descriptor is claimed once, even for proxy aliases.
        let mut control = unsafe { UnixStream::from_raw_fd(route.control_fd) };
        // SAFETY: F_SETFD takes integer flags, not a pointer; `control` owns the
        // live descriptor throughout the call.
        if unsafe { libc::fcntl(control.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0
        {
            return Err(io::Error::last_os_error());
        }
        control.set_read_timeout(Some(HANDOFF_TIMEOUT))?;
        let listener = bind_local_loopback_listener()?;
        let local_port = listener.local_addr()?.port();
        send_listener(&control, &listener)?;
        let mut ready = [0_u8; 1];
        control.read_exact(&mut ready)?;
        if ready != [HOST_BRIDGE_READY] {
            return Err(io::Error::other("host bridge did not accept its listener"));
        }
        // Loop scope closes both descriptors, leaving only the host's listener.
        // Neither can reach the namespace reaper or the untrusted command.
        local_port_by_control.insert(route.control_fd, local_port);
    }

    for route in spec.routes {
        let Some(local_port) = local_port_by_control.get(&route.control_fd) else {
            return Err(io::Error::other("missing proxy listener port"));
        };
        let original_value = std::env::var(&route.env_key).map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing proxy env key {}", route.env_key),
            )
        })?;
        let Some(rewritten) = rewrite_proxy_env_value(&original_value, *local_port) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("could not rewrite proxy URL for env key {}", route.env_key),
            ));
        };
        // SAFETY: this helper process is single-threaded at this point, and
        // env mutation happens before execing the user command.
        unsafe {
            std::env::set_var(route.env_key, rewritten);
        }
    }

    Ok(())
}

fn plan_proxy_routes(env: &HashMap<String, String>) -> ProxyRoutePlan {
    let mut routes = Vec::new();
    let mut has_proxy_config = false;

    for (key, value) in env {
        if !is_proxy_env_key(key) {
            continue;
        }

        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        has_proxy_config = true;

        let Some(endpoint) = parse_loopback_proxy_endpoint(trimmed) else {
            continue;
        };
        routes.push(PlannedProxyRoute {
            env_key: key.clone(),
            endpoint,
        });
    }

    routes.sort_by(|left, right| left.env_key.cmp(&right.env_key));
    ProxyRoutePlan {
        routes,
        has_proxy_config,
    }
}

fn is_proxy_env_key(key: &str) -> bool {
    // Only exact key spellings overwritten by managed setup may nominate a host
    // socket. A caller-controlled mixed-case alias can otherwise create a second bridge.
    codex_network_proxy::PROXY_ENV_KEYS.contains(&key)
        && codex_network_proxy::PROXY_URL_ENV_KEYS
            .iter()
            .any(|url_key| key.eq_ignore_ascii_case(url_key))
}

fn parse_loopback_proxy_endpoint(proxy_url: &str) -> Option<SocketAddr> {
    let candidate = if proxy_url.contains("://") {
        proxy_url.to_string()
    } else {
        format!("http://{proxy_url}")
    };

    let parsed = Url::parse(&candidate).ok()?;
    let host = parsed.host_str()?;
    if !is_loopback_host(host) {
        return None;
    }

    let scheme = parsed.scheme().to_ascii_lowercase();
    let port = parsed
        .port()
        .unwrap_or_else(|| default_proxy_port(scheme.as_str()));
    if port == 0 {
        return None;
    }

    let ip = if host.eq_ignore_ascii_case("localhost") {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        host.parse::<IpAddr>().ok()?
    };
    if ip.is_loopback() {
        Some(SocketAddr::new(ip, port))
    } else {
        None
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

fn default_proxy_port(scheme: &str) -> u16 {
    match scheme {
        "https" => 443,
        "socks5" | "socks5h" | "socks4" | "socks4a" => 1080,
        _ => 80,
    }
}

fn rewrite_proxy_env_value(proxy_url: &str, local_port: u16) -> Option<String> {
    let had_scheme = proxy_url.contains("://");
    let candidate = if had_scheme {
        proxy_url.to_string()
    } else {
        format!("http://{proxy_url}")
    };

    let mut parsed = Url::parse(&candidate).ok()?;
    parsed.set_host(Some("127.0.0.1")).ok()?;
    parsed.set_port(Some(local_port)).ok()?;
    let mut rewritten = parsed.to_string();
    if !had_scheme {
        rewritten = rewritten
            .strip_prefix("http://")
            .unwrap_or(rewritten.as_str())
            .to_string();
    }
    if !proxy_url.ends_with('/')
        && !proxy_url.contains('?')
        && !proxy_url.contains('#')
        && rewritten.ends_with('/')
    {
        rewritten.pop();
    }
    Some(rewritten)
}

fn spawn_host_bridge(
    endpoint: SocketAddr,
    attribution_token: Option<&str>,
    inherited_controls: &mut BTreeMap<SocketAddr, File>,
) -> io::Result<File> {
    let (host_control, control) = UnixStream::pair()?;
    let host_control = UnixStream::from(move_fd_above_stdio(host_control.into())?);
    let mut control = UnixStream::from(move_fd_above_stdio(control.into())?);
    control.set_read_timeout(Some(HANDOFF_TIMEOUT))?;
    let parent_pid = unsafe { libc::getpid() };
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }

    if pid == 0 {
        drop(control);
        // A worker must not retain another route's inner bootstrap endpoint.
        // Drop this child's inherited owners; the parent's copies are unaffected by fork.
        inherited_controls.clear();
        let result = run_host_bridge(endpoint, host_control, attribution_token, parent_pid);
        if result.is_err() {
            unsafe { libc::_exit(1) };
        }
        unsafe { libc::_exit(0) };
    }

    drop(host_control);
    let mut ready = [0_u8; 1];
    control.read_exact(&mut ready)?;
    if ready != [HOST_BRIDGE_READY] {
        return Err(io::Error::other(
            "host bridge did not acknowledge readiness",
        ));
    }
    Ok(File::from(OwnedFd::from(control)))
}

/// Each connection worker holds a permit until it exits.
pub(super) struct ConnectionSlots(Arc<Semaphore>);

impl ConnectionSlots {
    pub(super) fn new() -> Self {
        const MAX_CONNECTIONS: usize = 128;
        Self(Arc::new(Semaphore::new(MAX_CONNECTIONS)))
    }

    pub(super) fn spawn(&self, serve: impl FnOnce() + Send + 'static) {
        let Ok(permit) = Arc::clone(&self.0).try_acquire_owned() else {
            return;
        };
        let _ = std::thread::Builder::new().spawn(move || {
            let _permit = permit;
            serve();
        });
    }
}

fn run_host_bridge(
    endpoint: SocketAddr,
    mut control: UnixStream,
    attribution_token: Option<&str>,
    parent_pid: libc::pid_t,
) -> io::Result<()> {
    harden_bridge_process(parent_pid)?;
    control.write_all(&[HOST_BRIDGE_READY])?;
    // The outer helper must launch bubblewrap before this listener can arrive.
    // The socket keeps its isolated network namespace even though we accept in
    // the host process; newly opened upstream sockets use our host namespace.
    let listener = receive_listener(&control)?;
    control.write_all(&[HOST_BRIDGE_READY])?;
    drop(control);

    let attribution_token = attribution_token.map(str::to_owned);
    let slots = ConnectionSlots::new();
    loop {
        let (client_stream, _) = listener.accept()?;
        let attribution_token = attribution_token.clone();
        slots.spawn(move || {
            let mut tcp_stream = match TcpStream::connect(endpoint) {
                Ok(stream) => stream,
                Err(_) => return,
            };
            if let Some(attribution_token) = attribution_token
                && write_attribution_frame(&mut tcp_stream, &attribution_token).is_err()
            {
                // The shared ingress must reject unauthenticated connections; do not forward
                // application bytes if this bridge cannot prove the exec attribution first.
                return;
            }
            let _ = proxy_bidirectional(tcp_stream, client_stream);
        });
    }
}

fn bind_local_loopback_listener() -> io::Result<TcpListener> {
    match TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) {
        Ok(listener) => Ok(listener),
        Err(bind_err) => {
            let should_retry_after_lo_up = matches!(
                bind_err.raw_os_error(),
                Some(errno) if errno == libc::EADDRNOTAVAIL || errno == libc::ENETUNREACH
            );
            if !should_retry_after_lo_up {
                return Err(bind_err);
            }

            ensure_loopback_interface_up()?;
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        }
    }
}

fn ensure_loopback_interface_up() -> io::Result<()> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let mut ifreq = unsafe { std::mem::zeroed::<libc::ifreq>() };
    for (index, byte) in LOOPBACK_INTERFACE_NAME.iter().copied().enumerate() {
        ifreq.ifr_name[index] = byte as libc::c_char;
    }

    let read_flags_result =
        unsafe { libc::ioctl(fd, libc::SIOCGIFFLAGS as libc::Ioctl, &mut ifreq) };
    if read_flags_result < 0 {
        let err = io::Error::last_os_error();
        let _ = close_fd(fd);
        return Err(err);
    }

    let current_flags = unsafe { ifreq.ifr_ifru.ifru_flags };
    let up_flag = libc::IFF_UP as libc::c_short;
    if (current_flags & up_flag) != up_flag {
        ifreq.ifr_ifru.ifru_flags = current_flags | up_flag;
        let set_flags_result =
            unsafe { libc::ioctl(fd, libc::SIOCSIFFLAGS as libc::Ioctl, &ifreq) };
        if set_flags_result < 0 {
            let err = io::Error::last_os_error();
            let _ = close_fd(fd);
            return Err(err);
        }
    }

    let mut addr_req = unsafe { std::mem::zeroed::<libc::ifreq>() };
    for (index, byte) in LOOPBACK_INTERFACE_NAME.iter().copied().enumerate() {
        addr_req.ifr_name[index] = byte as libc::c_char;
    }
    let loopback_addr = libc::sockaddr_in {
        sin_family: libc::AF_INET as libc::sa_family_t,
        sin_port: 0,
        sin_addr: libc::in_addr {
            s_addr: libc::htonl(libc::INADDR_LOOPBACK),
        },
        sin_zero: [0; 8],
    };
    unsafe {
        addr_req.ifr_ifru.ifru_addr =
            *(&loopback_addr as *const libc::sockaddr_in as *const libc::sockaddr);
    }
    let set_addr_result = unsafe { libc::ioctl(fd, libc::SIOCSIFADDR as libc::Ioctl, &addr_req) };
    if set_addr_result < 0 {
        let err = io::Error::last_os_error();
        let allow_existing_or_immutable_addr =
            matches!(err.raw_os_error(), Some(libc::EEXIST | libc::EPERM));
        if !allow_existing_or_immutable_addr {
            let _ = close_fd(fd);
            return Err(err);
        }
    }

    close_fd(fd)
}

pub(super) fn proxy_bidirectional(upstream: TcpStream, client: TcpStream) -> io::Result<()> {
    let (upload_done, upload) = mpsc::channel();
    let (download_done, download) = mpsc::channel();
    std::thread::scope(|scope| {
        let response = std::thread::Builder::new().spawn_scoped(scope, || {
            copy_and_shutdown(&upstream, &client, download_done, upload)
        })?;
        let request = copy_and_shutdown(&client, &upstream, upload_done, download);
        response
            .join()
            .map_err(|_| io::Error::other("bridge thread panicked"))??;
        request
    })
}

fn copy_and_shutdown(
    mut source: &TcpStream,
    mut destination: &TcpStream,
    done: mpsc::Sender<()>,
    peer: mpsc::Receiver<()>,
) -> io::Result<()> {
    let result = io::copy(&mut source, &mut destination);
    let _ = destination.shutdown(Shutdown::Write);
    let _ = done.send(());
    // Let the other direction flush queued bytes. A stalled peer must still release the permit.
    if result.is_err()
        && peer.recv_timeout(Duration::from_secs(1)) == Err(mpsc::RecvTimeoutError::Timeout)
    {
        let _ = source.shutdown(Shutdown::Both);
        let _ = destination.shutdown(Shutdown::Both);
    }
    result.map(|_| ())
}

#[cfg(test)]
#[path = "proxy_routing_tests.rs"]
mod tests;
