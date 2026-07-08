use crate::proxy_routing::close_fd;
use crate::proxy_routing::harden_bridge_process as harden_relay_process;
use codex_network_proxy::ManagedNetworkDomainPolicy;
use codex_network_proxy::NetworkDomainMatcher;
use codex_network_proxy::normalize_host;
use dns_lookup::AddrInfoHints;
use dns_lookup::getaddrinfo;
use hickory_proto::op::Message;
use hickory_proto::op::MessageType;
use hickory_proto::op::OpCode;
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::DNSClass;
use hickory_proto::rr::Name;
use hickory_proto::rr::RData;
use hickory_proto::rr::Record;
use hickory_proto::rr::RecordType;
use hickory_proto::rr::rdata::A;
use hickory_proto::rr::rdata::AAAA;
use hickory_proto::rr::rdata::CNAME;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::net::IpAddr;
use std::net::TcpListener;
use std::net::UdpSocket;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::IntoRawFd;
use std::os::fd::RawFd;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

const RESPONSE_TTL: u32 = 0;
const SANDBOX_DNS_LISTEN_ADDR: &str = "127.0.0.1:53";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct DnsRouteSpec {
    pub(crate) udp_fd: RawFd,
    pub(crate) tcp_fd: RawFd,
}

pub(crate) struct BoundDnsStub(UdpSocket, TcpListener);

pub(crate) fn spawn_host_dns_relay(
    policy: &ManagedNetworkDomainPolicy,
) -> io::Result<(DnsRouteSpec, Vec<File>, libc::pid_t)> {
    let matcher = NetworkDomainMatcher::new(policy).map_err(io::Error::other)?;
    let (host_udp, netns_udp) = UnixStream::pair()?;
    let (host_tcp, netns_tcp) = UnixStream::pair()?;
    let netns_udp = unsafe { File::from_raw_fd(netns_udp.into_raw_fd()) };
    let netns_tcp = unsafe { File::from_raw_fd(netns_tcp.into_raw_fd()) };
    let spec = DnsRouteSpec {
        udp_fd: netns_udp.as_raw_fd(),
        tcp_fd: netns_tcp.as_raw_fd(),
    };
    let inherited_fds = [spec.udp_fd, spec.tcp_fd];
    let pid = spawn_process(move || {
        for fd in inherited_fds {
            close_fd(fd)?;
        }
        run_host_dns_relay(host_udp, host_tcp, matcher)
    })?;
    Ok((spec, vec![netns_udp, netns_tcp], pid))
}

pub(crate) fn bind_netns_dns_stub() -> io::Result<BoundDnsStub> {
    let udp_socket = UdpSocket::bind(SANDBOX_DNS_LISTEN_ADDR)?;
    let listen_addr = udp_socket.local_addr()?;
    let tcp_listener = TcpListener::bind(listen_addr)?;
    Ok(BoundDnsStub(udp_socket, tcp_listener))
}

pub(crate) fn spawn_netns_dns_stub(
    spec: &DnsRouteSpec,
    bound: BoundDnsStub,
) -> io::Result<libc::pid_t> {
    if spec.udp_fd == spec.tcp_fd {
        return Err(invalid_input("DNS relay file descriptors must be distinct"));
    }
    let udp_stream = take_inherited_stream(spec.udp_fd)?;
    let tcp_stream = take_inherited_stream(spec.tcp_fd)?;
    spawn_process(move || run_netns_dns_stub(bound, udp_stream, tcp_stream))
}

struct DnsPolicy {
    matcher: NetworkDomainMatcher,
}

impl DnsPolicy {
    fn resolve(&self, wire: &[u8]) -> io::Result<Vec<u8>> {
        self.response(wire).to_vec().map_err(io::Error::other)
    }

    fn response(&self, wire: &[u8]) -> Message {
        let parsed = Message::from_vec(wire).ok();
        let Some(query) = parsed.as_ref().filter(|query| {
            query.message_type() == MessageType::Query
                && query.op_code() == OpCode::Query
                && query.queries().len() == 1
                && query.queries()[0].query_class() == DNSClass::IN
        }) else {
            return refused_response(wire, parsed.as_ref());
        };
        let question = &query.queries()[0];
        let lookup_name = question.name().to_utf8();
        let host = normalize_host(&lookup_name);
        match question.query_type() {
            RecordType::A | RecordType::AAAA | RecordType::CNAME
                if self.matcher.is_allowed(&host) =>
            {
                self.resolve_forward(query, &lookup_name, question.query_type())
            }
            _ => refused_response(wire, Some(query)),
        }
    }

    fn resolve_forward(&self, query: &Message, host: &str, record_type: RecordType) -> Message {
        let hints = AddrInfoHints {
            flags: libc::AI_CANONNAME,
            socktype: libc::SOCK_STREAM,
            ..Default::default()
        };
        let Ok(addresses) = getaddrinfo(Some(host), None, Some(hints)) else {
            return response_message(query, ResponseCode::ServFail);
        };
        let Ok(addresses) = addresses.collect::<io::Result<Vec<_>>>() else {
            return response_message(query, ResponseCode::ServFail);
        };
        let query_host = normalize_host(host);
        let canonical_host = addresses
            .iter()
            .find_map(|address| address.canonname.as_deref())
            .map(normalize_host)
            .filter(|canonical| !canonical.is_empty())
            .unwrap_or_else(|| query_host.clone());
        let emit_canonical =
            canonical_host != query_host && self.matcher.is_allowed(&canonical_host);
        if record_type == RecordType::CNAME && canonical_host != query_host && !emit_canonical {
            return refused_response(&[], Some(query));
        }
        let mut seen = HashSet::new();
        let mut response = response_message(query, ResponseCode::NoError);
        let answer_name = query.queries()[0].name().clone();
        let canonical_name = if emit_canonical {
            let Ok(name) = Name::from_ascii(&canonical_host) else {
                return response_message(query, ResponseCode::ServFail);
            };
            response.add_answer(Record::from_rdata(
                answer_name.clone(),
                RESPONSE_TTL,
                RData::CNAME(CNAME(name.clone())),
            ));
            name
        } else {
            answer_name
        };
        if record_type == RecordType::CNAME {
            return response;
        }
        for (_, data) in addresses
            .into_iter()
            .filter_map(|address| {
                let address = address.sockaddr.ip();
                match (record_type, address) {
                    (RecordType::A, IpAddr::V4(ip)) => Some((address, RData::A(A(ip)))),
                    (RecordType::AAAA, IpAddr::V6(ip)) => Some((address, RData::AAAA(AAAA(ip)))),
                    _ => None,
                }
            })
            .filter(|(address, _)| seen.insert(*address))
            .take(/*n*/ 16)
        {
            response.add_answer(Record::from_rdata(
                canonical_name.clone(),
                RESPONSE_TTL,
                data,
            ));
        }
        response
    }
}

fn response_message(query: &Message, code: ResponseCode) -> Message {
    let mut response = Message::error_msg(query.id(), query.op_code(), code);
    response
        .set_recursion_desired(query.recursion_desired())
        .set_recursion_available(true)
        .add_queries(query.queries().iter().take(1).cloned());
    response
}

fn refused_response(wire: &[u8], query: Option<&Message>) -> Message {
    if let Some(query) = query {
        return response_message(query, ResponseCode::Refused);
    }
    let id = wire
        .get(..2)
        .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
        .unwrap_or_default();
    Message::error_msg(id, OpCode::Query, ResponseCode::Refused)
}

fn run_host_dns_relay(
    mut udp_stream: UnixStream,
    mut tcp_stream: UnixStream,
    matcher: NetworkDomainMatcher,
) -> io::Result<()> {
    let policy = Arc::new(DnsPolicy { matcher });
    let udp_policy = Arc::clone(&policy);
    std::thread::Builder::new()
        .spawn(move || serve_session(&mut udp_stream, |query| udp_policy.resolve(query)))?;
    serve_session(&mut tcp_stream, |query| policy.resolve(query))
}

fn run_netns_dns_stub(
    bound: BoundDnsStub,
    mut udp_stream: UnixStream,
    mut tcp_stream: UnixStream,
) -> io::Result<()> {
    std::thread::Builder::new().spawn(move || netns_udp_loop(bound.0, &mut udp_stream))?;
    netns_tcp_loop(bound.1, &mut tcp_stream)
}

fn netns_udp_loop(socket: UdpSocket, stream: &mut UnixStream) -> io::Result<()> {
    let mut query = vec![0; usize::from(u16::MAX)];
    loop {
        let (len, peer) = socket.recv_from(&mut query)?;
        write_frame(stream, &query[..len])?;
        socket.send_to(&read_frame(stream)?, peer)?;
    }
}

fn netns_tcp_loop(listener: TcpListener, relay: &mut UnixStream) -> io::Result<()> {
    loop {
        let (mut client, _) = listener.accept()?;
        serve_session(&mut client, |query| {
            write_frame(relay, query)?;
            read_frame(relay)
        })?;
    }
}

fn serve_session(
    stream: &mut (impl Read + Write),
    mut resolve: impl FnMut(&[u8]) -> io::Result<Vec<u8>>,
) -> io::Result<()> {
    while let Ok(query) = read_frame(stream) {
        write_frame(stream, &resolve(&query)?)?;
    }
    Ok(())
}

fn write_frame(writer: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    let len = u16::try_from(payload.len())
        .map_err(|_| invalid_input("DNS message exceeds maximum wire length"))?;
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

fn take_inherited_stream(fd: RawFd) -> io::Result<UnixStream> {
    if fd <= libc::STDERR_FILENO || unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
        return Err(invalid_input("invalid inherited DNS relay file descriptor"));
    }
    Ok(unsafe { UnixStream::from_raw_fd(fd) })
}

fn spawn_process(run: impl FnOnce() -> io::Result<()>) -> io::Result<libc::pid_t> {
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }
    if pid == 0 {
        let status = i32::from(harden_relay_process().and_then(|_| run()).is_err());
        unsafe { libc::_exit(status) };
    }
    Ok(pid)
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
#[path = "dns_routing_tests.rs"]
mod tests;
