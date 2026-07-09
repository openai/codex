use codex_network_proxy::DNS_PROXY_ENV_KEY;
use codex_network_proxy::DNS_PROXY_SESSION_PREFACE;
use std::io;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::net::UdpSocket;
use std::time::Duration;
use url::Url;

const SANDBOX_DNS_LISTEN_ADDR: (Ipv4Addr, u16) = (Ipv4Addr::LOCALHOST, 53);
const DNS_IO_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_DNS_MESSAGE_BYTES: usize = u16::MAX as usize;

pub(crate) struct BoundDnsStub {
    udp_socket: UdpSocket,
    tcp_listener: TcpListener,
}

pub(crate) fn bind_netns_dns_stub() -> io::Result<BoundDnsStub> {
    let udp_socket = UdpSocket::bind(SANDBOX_DNS_LISTEN_ADDR)?;
    let tcp_listener = TcpListener::bind(SANDBOX_DNS_LISTEN_ADDR)?;
    Ok(BoundDnsStub {
        udp_socket,
        tcp_listener,
    })
}

pub(crate) fn spawn_netns_dns_stub(bound: BoundDnsStub) -> io::Result<()> {
    let endpoint = take_dns_proxy_endpoint_from_env()?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "DNS bridge expected a proxy endpoint",
        )
    })?;
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }
    if pid == 0 {
        let status = i32::from(
            crate::proxy_routing::harden_bridge_process()
                .and_then(|_| run_netns_dns_stub(bound, endpoint))
                .is_err(),
        );
        unsafe { libc::_exit(status) };
    }
    Ok(())
}

fn take_dns_proxy_endpoint_from_env() -> io::Result<Option<SocketAddr>> {
    let Some(value) = std::env::var_os(DNS_PROXY_ENV_KEY) else {
        return Ok(None);
    };
    // SAFETY: the helper process is single-threaded at this point, before execing
    // the user command.
    unsafe { std::env::remove_var(DNS_PROXY_ENV_KEY) };
    let value = value
        .into_string()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "DNS proxy URL is not UTF-8"))?;
    parse_dns_proxy_endpoint(value.as_str()).map(Some)
}

fn parse_dns_proxy_endpoint(value: &str) -> io::Result<SocketAddr> {
    let parsed = Url::parse(value).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid DNS proxy URL: {err}"),
        )
    })?;
    if parsed.scheme() != "tcp" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DNS proxy URL must use tcp scheme",
        ));
    }
    let Some(host) = parsed.host_str() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DNS proxy URL is missing host",
        ));
    };
    if host != "127.0.0.1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DNS proxy URL must point at 127.0.0.1",
        ));
    }
    let Some(port) = parsed.port() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DNS proxy URL is missing port",
        ));
    };
    Ok(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
}

fn run_netns_dns_stub(bound: BoundDnsStub, endpoint: SocketAddr) -> io::Result<()> {
    let udp_endpoint = endpoint;
    std::thread::Builder::new().spawn(move || netns_udp_loop(bound.udp_socket, udp_endpoint))?;
    netns_tcp_loop(bound.tcp_listener, endpoint)
}

fn netns_udp_loop(socket: UdpSocket, endpoint: SocketAddr) -> io::Result<()> {
    let mut query = vec![0; MAX_DNS_MESSAGE_BYTES];
    loop {
        let (len, peer) = socket.recv_from(&mut query)?;
        let Ok(response) = resolve_through_proxy(endpoint, &query[..len]) else {
            continue;
        };
        socket.send_to(&response, peer)?;
    }
}

fn netns_tcp_loop(listener: TcpListener, endpoint: SocketAddr) -> io::Result<()> {
    loop {
        let (mut client, _) = listener.accept()?;
        client.set_read_timeout(Some(DNS_IO_TIMEOUT))?;
        client.set_write_timeout(Some(DNS_IO_TIMEOUT))?;
        std::thread::spawn(move || {
            let _ = serve_tcp_client(&mut client, endpoint);
        });
    }
}

fn serve_tcp_client(client: &mut TcpStream, endpoint: SocketAddr) -> io::Result<()> {
    while let Ok(query) = read_frame(client) {
        let response = resolve_through_proxy(endpoint, &query)?;
        write_frame(client, &response)?;
    }
    Ok(())
}

fn resolve_through_proxy(endpoint: SocketAddr, query: &[u8]) -> io::Result<Vec<u8>> {
    let mut stream = TcpStream::connect(endpoint)?;
    stream.set_read_timeout(Some(DNS_IO_TIMEOUT))?;
    stream.set_write_timeout(Some(DNS_IO_TIMEOUT))?;
    stream.write_all(DNS_PROXY_SESSION_PREFACE)?;
    let mut ack = [0_u8; DNS_PROXY_SESSION_PREFACE.len()];
    stream.read_exact(&mut ack)?;
    if &ack != DNS_PROXY_SESSION_PREFACE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "DNS proxy rejected session preface",
        ));
    }
    write_frame(&mut stream, query)?;
    read_frame(&mut stream)
}

fn write_frame(writer: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    let len = u16::try_from(payload.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "DNS message exceeds maximum wire length",
        )
    })?;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(payload)
}

fn read_frame(reader: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut len = [0; 2];
    reader.read_exact(&mut len)?;
    let mut payload = vec![0; usize::from(u16::from_be_bytes(len))];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

#[cfg(test)]
#[path = "dns_routing_tests.rs"]
mod tests;
