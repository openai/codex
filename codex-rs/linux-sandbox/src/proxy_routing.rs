use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs::DirBuilder;
use std::fs::File;
use std::fs::Permissions;
use std::io;
use std::io::Read;
use std::io::Write;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::fd::FromRawFd;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;

const PROXY_ENV_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "FTP_PROXY",
    "YARN_HTTP_PROXY",
    "YARN_HTTPS_PROXY",
    "NPM_CONFIG_HTTP_PROXY",
    "NPM_CONFIG_HTTPS_PROXY",
    "NPM_CONFIG_PROXY",
    "BUNDLE_HTTP_PROXY",
    "BUNDLE_HTTPS_PROXY",
    "PIP_PROXY",
    "DOCKER_HTTP_PROXY",
    "DOCKER_HTTPS_PROXY",
];

const PROXY_SOCKET_DIR_PREFIX: &str = "codex-linux-sandbox-proxy-";
const HOST_BRIDGE_READY: u8 = 1;
const LOOPBACK_INTERFACE_NAME: &[u8] = b"lo";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ProxyRouteSpec {
    routes: Vec<ProxyRouteEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ProxyRouteEntry {
    env_key: String,
    uds_path: PathBuf,
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

pub(crate) fn prepare_host_proxy_route_spec() -> io::Result<String> {
    let env: HashMap<String, String> = std::env::vars().collect();
    let plan = plan_proxy_routes(&env);

    if plan.routes.is_empty() {
        let message = if plan.has_proxy_config {
            "managed proxy mode requires parseable loopback proxy endpoints"
        } else {
            "managed proxy mode requires proxy environment variables"
        };
        return Err(io::Error::new(io::ErrorKind::InvalidInput, message));
    }

    let socket_parent_dir = proxy_socket_parent_dir();
    let _ = cleanup_stale_proxy_socket_dirs_in(socket_parent_dir.as_path());

    let socket_dir = create_proxy_socket_dir()?;
    let mut socket_by_endpoint: BTreeMap<SocketAddr, PathBuf> = BTreeMap::new();
    let mut next_index = 0usize;
    for route in &plan.routes {
        if socket_by_endpoint.contains_key(&route.endpoint) {
            continue;
        }
        let socket_path = socket_dir.join(format!("proxy-route-{next_index}.sock"));
        next_index += 1;
        socket_by_endpoint.insert(route.endpoint, socket_path);
    }

    let mut host_bridge_pids = Vec::with_capacity(socket_by_endpoint.len());
    for (endpoint, socket_path) in &socket_by_endpoint {
        host_bridge_pids.push(spawn_host_bridge(*endpoint, socket_path)?);
    }
    spawn_proxy_socket_dir_cleanup_worker(socket_dir, host_bridge_pids)?;

    let mut routes = Vec::with_capacity(plan.routes.len());
    for route in plan.routes {
        let Some(uds_path) = socket_by_endpoint.get(&route.endpoint) else {
            return Err(io::Error::other(format!(
                "missing UDS path for endpoint {}",
                route.endpoint
            )));
        };
        routes.push(ProxyRouteEntry {
            env_key: route.env_key,
            uds_path: uds_path.clone(),
        });
    }

    serde_json::to_string(&ProxyRouteSpec { routes }).map_err(io::Error::other)
}

pub(crate) fn activate_proxy_routes_in_netns(serialized_spec: &str) -> io::Result<()> {
    let spec: ProxyRouteSpec = serde_json::from_str(serialized_spec).map_err(io::Error::other)?;

    if spec.routes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "proxy routing spec contained no routes",
        ));
    }

    let mut local_port_by_uds_path: BTreeMap<PathBuf, u16> = BTreeMap::new();
    for route in &spec.routes {
        if local_port_by_uds_path.contains_key(&route.uds_path) {
            continue;
        }
        let local_port = spawn_local_bridge(route.uds_path.as_path())?;
        local_port_by_uds_path.insert(route.uds_path.clone(), local_port);
    }

    for route in spec.routes {
        let Some(local_port) = local_port_by_uds_path.get(&route.uds_path) else {
            return Err(io::Error::other(format!(
                "missing local bridge port for UDS path {}",
                route.uds_path.display()
            )));
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
    let upper = key.to_ascii_uppercase();
    PROXY_ENV_KEYS.contains(&upper.as_str())
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

fn create_proxy_socket_dir() -> io::Result<PathBuf> {
    let temp_dir = proxy_socket_parent_dir();
    let pid = std::process::id();
    for attempt in 0..128 {
        let candidate = temp_dir.join(format!("{PROXY_SOCKET_DIR_PREFIX}{pid}-{attempt}"));
        // The bridge UDS paths live under a shared temp root, so the per-run
        // directory should not be traversable by other processes.
        let mut dir_builder = DirBuilder::new();
        dir_builder.mode(0o700);
        match dir_builder.create(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "failed to allocate proxy routing temp dir under {}",
            temp_dir.display()
        ),
    ))
}

fn proxy_socket_parent_dir() -> PathBuf {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
        let candidate = PathBuf::from(codex_home).join("tmp");
        if ensure_private_proxy_socket_parent_dir(candidate.as_path()).is_ok() {
            return candidate;
        }
    }
    std::env::temp_dir()
}

fn ensure_private_proxy_socket_parent_dir(path: &Path) -> io::Result<()> {
    let mut dir_builder = DirBuilder::new();
    dir_builder.recursive(true);
    dir_builder.mode(0o700);
    match dir_builder.create(path) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(err),
    }
    std::fs::set_permissions(path, Permissions::from_mode(0o700))
}

fn cleanup_stale_proxy_socket_dirs_in(temp_dir: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(temp_dir)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(owner_pid) = parse_proxy_socket_dir_owner_pid(file_name.as_ref()) else {
            continue;
        };
        if is_pid_alive(owner_pid) {
            continue;
        }

        let _ = cleanup_proxy_socket_dir(entry.path().as_path());
    }

    Ok(())
}

fn parse_proxy_socket_dir_owner_pid(file_name: &str) -> Option<u32> {
    let suffix = file_name.strip_prefix(PROXY_SOCKET_DIR_PREFIX)?;
    let (pid_raw, _) = suffix.split_once('-')?;
    pid_raw.parse::<u32>().ok().filter(|pid| *pid != 0)
}

fn is_pid_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    is_pid_alive_raw(pid)
}

fn is_pid_alive_raw(pid: libc::pid_t) -> bool {
    let status = unsafe { libc::kill(pid, 0) };
    if status == 0 {
        return true;
    }
    let err = io::Error::last_os_error();
    !matches!(err.raw_os_error(), Some(libc::ESRCH))
}

fn spawn_proxy_socket_dir_cleanup_worker(
    socket_dir: PathBuf,
    host_bridge_pids: Vec<libc::pid_t>,
) -> io::Result<()> {
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }

    if pid == 0 {
        loop {
            if host_bridge_pids
                .iter()
                .copied()
                .all(|bridge_pid| !is_pid_alive_raw(bridge_pid))
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        let _ = cleanup_proxy_socket_dir(socket_dir.as_path());
        unsafe { libc::_exit(0) };
    }

    Ok(())
}

fn cleanup_proxy_socket_dir(socket_dir: &Path) -> io::Result<()> {
    for _ in 0..20 {
        match std::fs::remove_dir_all(socket_dir) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }

    match std::fs::remove_dir_all(socket_dir) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn spawn_host_bridge(endpoint: SocketAddr, uds_path: &Path) -> io::Result<libc::pid_t> {
    let (read_fd, write_fd) = create_ready_pipe()?;
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        let err = io::Error::last_os_error();
        close_fd(read_fd)?;
        close_fd(write_fd)?;
        return Err(err);
    }

    if pid == 0 {
        if close_fd(read_fd).is_err() {
            unsafe { libc::_exit(1) };
        }
        let result = run_host_bridge(endpoint, uds_path, write_fd);
        if result.is_err() {
            unsafe { libc::_exit(1) };
        }
        unsafe { libc::_exit(0) };
    }

    close_fd(write_fd)?;
    let mut ready = [0_u8; 1];
    let mut read_file = unsafe { File::from_raw_fd(read_fd) };
    read_file.read_exact(&mut ready)?;
    if ready[0] != HOST_BRIDGE_READY {
        return Err(io::Error::other(
            "host bridge did not acknowledge readiness",
        ));
    }
    Ok(pid)
}

fn run_host_bridge(endpoint: SocketAddr, uds_path: &Path, ready_fd: libc::c_int) -> io::Result<()> {
    set_parent_death_signal()?;
    if uds_path.exists() {
        std::fs::remove_file(uds_path)?;
    }
    let listener = UnixListener::bind(uds_path)?;

    let mut ready_file = unsafe { File::from_raw_fd(ready_fd) };
    ready_file.write_all(&[HOST_BRIDGE_READY])?;
    drop(ready_file);

    loop {
        let (unix_stream, _) = listener.accept()?;
        std::thread::spawn(move || {
            let tcp_stream = match TcpStream::connect(endpoint) {
                Ok(stream) => stream,
                Err(_) => return,
            };
            let _ = proxy_bidirectional(tcp_stream, unix_stream);
        });
    }
}

fn spawn_local_bridge(uds_path: &Path) -> io::Result<u16> {
    let (read_fd, write_fd) = create_ready_pipe()?;
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        let err = io::Error::last_os_error();
        close_fd(read_fd)?;
        close_fd(write_fd)?;
        return Err(err);
    }

    if pid == 0 {
        if close_fd(read_fd).is_err() {
            unsafe { libc::_exit(1) };
        }
        let result = run_local_bridge(uds_path, write_fd);
        if result.is_err() {
            unsafe { libc::_exit(1) };
        }
        unsafe { libc::_exit(0) };
    }

    close_fd(write_fd)?;
    let mut port_bytes = [0_u8; 2];
    let mut read_file = unsafe { File::from_raw_fd(read_fd) };
    read_file.read_exact(&mut port_bytes)?;
    Ok(u16::from_be_bytes(port_bytes))
}

fn run_local_bridge(uds_path: &Path, ready_fd: libc::c_int) -> io::Result<()> {
    set_parent_death_signal()?;
    let listener = bind_local_loopback_listener()?;
    let port = listener.local_addr()?.port();

    let mut ready_file = unsafe { File::from_raw_fd(ready_fd) };
    ready_file.write_all(&port.to_be_bytes())?;
    drop(ready_file);

    let uds_path = uds_path.to_path_buf();
    loop {
        let (tcp_stream, _) = listener.accept()?;
        let socket_path = uds_path.clone();
        std::thread::spawn(move || {
            let unix_stream = match UnixStream::connect(socket_path) {
                Ok(stream) => stream,
                Err(_) => return,
            };
            let _ = proxy_bidirectional(tcp_stream, unix_stream);
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

fn set_parent_death_signal() -> io::Result<()> {
    let res = unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) };
    if res != 0 {
        Err(io::Error::last_os_error())
    } else if unsafe { libc::getppid() } == 1 {
        Err(io::Error::other("parent process already exited"))
    } else {
        Ok(())
    }
}

fn proxy_bidirectional(mut tcp_stream: TcpStream, mut unix_stream: UnixStream) -> io::Result<()> {
    let mut tcp_reader = tcp_stream.try_clone()?;
    let mut unix_writer = unix_stream.try_clone()?;
    let tcp_to_unix = std::thread::spawn(move || std::io::copy(&mut tcp_reader, &mut unix_writer));
    let unix_to_tcp = std::io::copy(&mut unix_stream, &mut tcp_stream);
    let tcp_to_unix = tcp_to_unix
        .join()
        .map_err(|_| io::Error::other("bridge thread panicked"))?;
    tcp_to_unix?;
    unix_to_tcp?;
    Ok(())
}

fn create_ready_pipe() -> io::Result<(libc::c_int, libc::c_int)> {
    let mut pipe_fds = [0; 2];
    let res = unsafe { libc::pipe2(pipe_fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if res != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((pipe_fds[0], pipe_fds[1]))
}

fn close_fd(fd: libc::c_int) -> io::Result<()> {
    let res = unsafe { libc::close(fd) };
    if res < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::PROXY_SOCKET_DIR_PREFIX;
    use super::ProxyRouteEntry;
    use super::ProxyRouteSpec;
    use super::cleanup_proxy_socket_dir;
    use super::cleanup_stale_proxy_socket_dirs_in;
    use super::default_proxy_port;
    use super::is_proxy_env_key;
    use super::parse_loopback_proxy_endpoint;
    use super::parse_proxy_socket_dir_owner_pid;
    use super::plan_proxy_routes;
    use super::rewrite_proxy_env_value;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::path::PathBuf;

    #[test]
    fn recognizes_proxy_env_keys_case_insensitively() {
        assert_eq!(is_proxy_env_key("HTTP_PROXY"), true);
        assert_eq!(is_proxy_env_key("http_proxy"), true);
        assert_eq!(is_proxy_env_key("PATH"), false);
    }

    #[test]
    fn parses_loopback_proxy_endpoint() {
        let endpoint = parse_loopback_proxy_endpoint("http://127.0.0.1:43128");
        assert_eq!(
            endpoint,
            Some(
                "127.0.0.1:43128"
                    .parse::<SocketAddr>()
                    .expect("valid socket")
            )
        );
    }

    #[test]
    fn ignores_non_loopback_proxy_endpoint() {
        assert_eq!(
            parse_loopback_proxy_endpoint("http://example.com:3128"),
            None
        );
    }

    #[test]
    fn plan_proxy_routes_only_includes_valid_loopback_endpoints() {
        let mut env = HashMap::new();
        env.insert(
            "HTTP_PROXY".to_string(),
            "http://127.0.0.1:43128".to_string(),
        );
        env.insert(
            "HTTPS_PROXY".to_string(),
            "http://example.com:3128".to_string(),
        );
        env.insert("PATH".to_string(), "/usr/bin".to_string());

        let plan = plan_proxy_routes(&env);
        assert_eq!(plan.has_proxy_config, true);
        assert_eq!(plan.routes.len(), 1);
        assert_eq!(plan.routes[0].env_key, "HTTP_PROXY");
        assert_eq!(
            plan.routes[0].endpoint,
            "127.0.0.1:43128"
                .parse::<SocketAddr>()
                .expect("valid socket")
        );
    }

    #[test]
    fn rewrites_proxy_url_to_local_loopback_port() {
        let rewritten =
            rewrite_proxy_env_value("socks5h://127.0.0.1:8081", 43210).expect("rewritten value");
        assert_eq!(rewritten, "socks5h://127.0.0.1:43210");
    }

    #[test]
    fn default_proxy_ports_match_expected_schemes() {
        assert_eq!(default_proxy_port("http"), 80);
        assert_eq!(default_proxy_port("https"), 443);
        assert_eq!(default_proxy_port("socks5h"), 1080);
    }

    #[test]
    fn cleanup_proxy_socket_dir_removes_bridge_artifacts() {
        let root = tempfile::tempdir().expect("tempdir should create");
        let socket_dir = root.path().join("codex-linux-sandbox-proxy-test");
        std::fs::create_dir(&socket_dir).expect("socket dir should create");
        let marker = socket_dir.join("bridge.sock");
        std::fs::write(&marker, b"test").expect("marker should write");

        cleanup_proxy_socket_dir(socket_dir.as_path()).expect("cleanup should succeed");

        assert_eq!(socket_dir.exists(), false);
    }

    #[test]
    fn proxy_route_spec_serialization_omits_proxy_urls() {
        let spec = ProxyRouteSpec {
            routes: vec![ProxyRouteEntry {
                env_key: "HTTP_PROXY".to_string(),
                uds_path: PathBuf::from("/tmp/proxy-route-0.sock"),
            }],
        };
        let serialized = serde_json::to_string(&spec).expect("proxy route spec should serialize");

        assert_eq!(
            serialized,
            r#"{"routes":[{"env_key":"HTTP_PROXY","uds_path":"/tmp/proxy-route-0.sock"}]}"#
        );
    }

    #[test]
    fn parse_proxy_socket_dir_owner_pid_reads_owner_pid() {
        assert_eq!(
            parse_proxy_socket_dir_owner_pid("codex-linux-sandbox-proxy-1234-0"),
            Some(1234)
        );
        assert_eq!(
            parse_proxy_socket_dir_owner_pid("codex-linux-sandbox-proxy-x"),
            None
        );
        assert_eq!(parse_proxy_socket_dir_owner_pid("not-a-proxy-dir"), None);
    }

    #[test]
    fn cleanup_stale_proxy_socket_dirs_removes_dead_pid_directories() {
        let root = tempfile::tempdir().expect("tempdir should create");
        let dead_dir = root
            .path()
            .join(format!("{PROXY_SOCKET_DIR_PREFIX}{}-0", u32::MAX));
        std::fs::create_dir(&dead_dir).expect("dead dir should create");

        let alive_dir = root
            .path()
            .join(format!("{PROXY_SOCKET_DIR_PREFIX}{}-1", std::process::id()));
        std::fs::create_dir(&alive_dir).expect("alive dir should create");

        let unrelated_dir = root.path().join("unrelated-proxy-dir");
        std::fs::create_dir(&unrelated_dir).expect("unrelated dir should create");

        cleanup_stale_proxy_socket_dirs_in(root.path()).expect("stale cleanup should succeed");

        assert_eq!(dead_dir.exists(), false);
        assert_eq!(alive_dir.exists(), true);
        assert_eq!(unrelated_dir.exists(), true);
    }
}
