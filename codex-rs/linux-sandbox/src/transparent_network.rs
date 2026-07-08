use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::Shutdown;
use std::net::TcpListener;
use std::net::TcpStream;
use std::net::UdpSocket;
use std::os::fd::FromRawFd;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use crate::capabilities::drop_all_capabilities;
use crate::proxy_routing::ensure_loopback_interface_up;
use crate::proxy_routing::harden_bridge_process;

const FAKE_POOL_BASE: u32 = (127 << 24) | (1 << 16);
const MAX_DNS_PACKET: usize = 4096;
const READY: u8 = 1;

pub(crate) fn configure_and_spawn(http_bridge_port: u16) -> io::Result<()> {
    let mut fds = [0; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: pipe2 initialized both descriptors, each of which is moved into one File.
    let mut read_pipe = unsafe { File::from_raw_fd(fds[0]) };
    let mut write_pipe = unsafe { File::from_raw_fd(fds[1]) };
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }
    if pid == 0 {
        drop(read_pipe);
        let exit_code = i32::from(run_service(http_bridge_port, &mut write_pipe).is_err());
        unsafe { libc::_exit(exit_code) };
    }
    drop(write_pipe);
    let mut ready = [0];
    if read_pipe.read_exact(&mut ready).is_ok() && ready == [READY] {
        return Ok(());
    }
    let _ = unsafe { libc::waitpid(pid, std::ptr::null_mut(), 0) };
    Err(io::Error::other(
        "transparent network service failed before becoming ready",
    ))
}

fn run_service(http_bridge_port: u16, ready: &mut File) -> io::Result<()> {
    harden_bridge_process()?;
    ensure_loopback_interface_up()?;
    let dns = UdpSocket::bind((Ipv4Addr::LOCALHOST, 53))?;
    let http = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 80))?;
    let https = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 443))?;
    let pool = Arc::new(Mutex::new(FakeIpPool::default()));

    drop_all_capabilities()?;
    spawn_service("codex-dns", {
        let pool = Arc::clone(&pool);
        move || serve_dns(dns, pool)
    })?;
    spawn_service("codex-http-capture", {
        let pool = Arc::clone(&pool);
        move || serve_capture(http, 80, http_bridge_port, pool)
    })?;
    ready.write_all(&[READY])?;
    serve_capture(https, 443, http_bridge_port, pool)
}

fn spawn_service<F>(name: &str, task: F) -> io::Result<()>
where
    F: FnOnce() -> io::Result<()> + Send + 'static,
{
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(task)
        .map(drop)
}

#[derive(Default)]
struct FakeIpPool {
    host_to_ip: HashMap<String, Ipv4Addr>,
    ip_to_host: HashMap<Ipv4Addr, String>,
}

impl FakeIpPool {
    fn address_for(&mut self, hostname: &str) -> Option<Ipv4Addr> {
        let hostname = hostname.to_ascii_lowercase();
        if let Some(address) = self.host_to_ip.get(&hostname) {
            return Some(*address);
        }
        let slot = u32::try_from(self.ip_to_host.len()).ok()?.checked_add(1)?;
        (slot < 1 << 16).then_some(())?;
        let address = Ipv4Addr::from(FAKE_POOL_BASE + slot);
        self.host_to_ip.insert(hostname.clone(), address);
        self.ip_to_host.insert(address, hostname);
        Some(address)
    }
}

fn parse_dns_query(packet: &[u8]) -> Option<(u16, String, u16, usize)> {
    if !(12..=MAX_DNS_PACKET).contains(&packet.len()) {
        return None;
    }
    let flags = read_u16(packet, 2)?;
    if flags & 0xf800 != 0 || read_u16(packet, 4)? != 1 {
        return None;
    }
    let mut labels = Vec::new();
    let mut offset = 12;
    let mut name_len = 1;
    loop {
        let len = usize::from(*packet.get(offset)?);
        offset += 1;
        if len == 0 {
            break;
        }
        if len > 63 || offset.checked_add(len)? > packet.len() {
            return None;
        }
        name_len += len + 1;
        let label = packet.get(offset..offset + len)?;
        if name_len > 255
            || !label
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return None;
        }
        labels.push(std::str::from_utf8(label).ok()?.to_ascii_lowercase());
        offset += len;
    }
    let end = offset.checked_add(4)?;
    if read_u16(packet, offset + 2)? != 1 {
        return None;
    }
    Some((flags, labels.join("."), read_u16(packet, offset)?, end))
}

fn dns_response(packet: &[u8], pool: &Mutex<FakeIpPool>) -> Option<Vec<u8>> {
    let (flags, name, query_type, question_end) = parse_dns_query(packet)?;
    let answer = if query_type != 1 || name.is_empty() {
        None
    } else if name == "localhost" {
        Some(Ipv4Addr::LOCALHOST)
    } else {
        pool.lock().ok()?.address_for(&name)
    };
    let mut response = Vec::with_capacity(question_end + 16);
    response.extend_from_slice(packet.get(..2)?);
    response.extend_from_slice(&(0x8080 | (flags & 0x0100)).to_be_bytes());
    response.extend_from_slice(&1_u16.to_be_bytes());
    response.extend_from_slice(&u16::from(answer.is_some()).to_be_bytes());
    response.extend_from_slice(&[0; 4]);
    response.extend_from_slice(packet.get(12..question_end)?);
    if let Some(address) = answer {
        response.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1, 0, 0, 0, 1, 0, 4]);
        response.extend_from_slice(&address.octets());
    }
    Some(response)
}

fn serve_dns(socket: UdpSocket, pool: Arc<Mutex<FakeIpPool>>) -> io::Result<()> {
    let mut packet = [0_u8; MAX_DNS_PACKET];
    loop {
        let (len, peer) = socket.recv_from(&mut packet)?;
        if let Some(response) = dns_response(&packet[..len], &pool) {
            let _ = socket.send_to(&response, peer);
        }
    }
}

fn serve_capture(
    listener: TcpListener,
    port: u16,
    bridge_port: u16,
    pool: Arc<Mutex<FakeIpPool>>,
) -> io::Result<()> {
    loop {
        let (client, _) = listener.accept()?;
        let pool = Arc::clone(&pool);
        spawn_service("codex-captured-connection", move || {
            capture_connection(client, port, bridge_port, &pool)
        })?;
    }
}

fn capture_connection(
    mut client: TcpStream,
    port: u16,
    bridge_port: u16,
    pool: &Mutex<FakeIpPool>,
) -> io::Result<()> {
    let destination = match client.local_addr()?.ip() {
        std::net::IpAddr::V4(address) => address,
        std::net::IpAddr::V6(_) => return Err(io::Error::other("unexpected IPv6 capture")),
    };
    let target = destination_target(destination, pool).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "captured destination is unsafe",
        )
    })?;
    let mut bridge = TcpStream::connect((Ipv4Addr::LOCALHOST, bridge_port))?;
    if port == 443 {
        let authority = format!("{target}:443");
        write!(
            bridge,
            "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n\r\n"
        )?;
        let leftover = read_connect_response(&mut bridge)?;
        client.write_all(&leftover)?;
    }
    splice(client, bridge)
}

fn destination_target(address: Ipv4Addr, pool: &Mutex<FakeIpPool>) -> Option<String> {
    matches!(address.octets(), [127, 1, _, _]).then_some(())?;
    pool.lock().ok()?.ip_to_host.get(&address).cloned()
}

fn read_connect_response(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    stream.set_read_timeout(Some(Duration::from_secs(300)))?;
    let mut head = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        head.extend_from_slice(&chunk[..read]);
        if head.len() > 8192 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        if let Some(end) = head.windows(4).position(|window| window == b"\r\n\r\n") {
            let line_end = head
                .windows(2)
                .position(|window| window == b"\r\n")
                .unwrap_or(end);
            let line = std::str::from_utf8(&head[..line_end])
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            if line.split_ascii_whitespace().nth(1) != Some("200") {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "proxy refused captured connection",
                ));
            }
            stream.set_read_timeout(/*duration*/ None)?;
            return Ok(head[end + 4..].to_vec());
        }
    }
}

fn splice(mut left: TcpStream, mut right: TcpStream) -> io::Result<()> {
    let mut left_reader = left.try_clone()?;
    let mut right_writer = right.try_clone()?;
    let forward = std::thread::spawn(move || {
        let result = io::copy(&mut left_reader, &mut right_writer);
        let _ = right_writer.shutdown(Shutdown::Write);
        result
    });
    let reverse = io::copy(&mut right, &mut left);
    let _ = left.shutdown(Shutdown::Write);
    forward
        .join()
        .map_err(|_| io::Error::other("capture forwarding thread panicked"))??;
    reverse?;
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}
