//! Run the bundled TCP stack in the command's private network namespace.
use crate::managed_tcp_dns::Names;
use rustix::net::sockopt;
use socks5_protocol::Address;
use socks5_protocol::AuthMethod;
use socks5_protocol::AuthRequest;
use socks5_protocol::AuthResponse;
use socks5_protocol::CommandReply;
use socks5_protocol::CommandRequest;
use socks5_protocol::CommandResponse;
use socks5_protocol::Version;
use socks5_protocol::sync::FromIO;
use std::io;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::net::UdpSocket;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

#[path = "managed_tcp_pending.rs"]
mod pending;
#[path = "managed_tcp_stack.rs"]
mod stack;

const TIMEOUT: Duration = Duration::from_secs(10);

pub(super) struct Relay {
    tun: OwnedFd,
    ready: std::io::PipeWriter,
    proxy: SocketAddr,
    dns_udp: UdpSocket,
    dns_tcp: TcpListener,
}

impl Relay {
    pub(super) fn prepare(
        fds: Vec<std::os::fd::RawFd>,
        proxy: Option<SocketAddr>,
    ) -> io::Result<(Self, std::io::PipeReader)> {
        let proxy =
            proxy.ok_or_else(|| io::Error::other("transparent TCP requires managed SOCKS5"))?;
        let [tun, udp, tcp] = crate::tcp_net_setup::receive(fds)?;
        let (reader, ready) = std::io::pipe()?;
        Ok((
            Self {
                tun,
                ready,
                proxy,
                dns_udp: udp.into(),
                dns_tcp: tcp.into(),
            },
            reader,
        ))
    }

    pub(super) fn serve(mut self) -> io::Result<()> {
        let names = Arc::new(Mutex::new(Names::default()));
        crate::managed_tcp_dns::serve(Arc::clone(&names), self.dns_udp, self.dns_tcp)?;
        let (started, readiness) = std::sync::mpsc::sync_channel(1);
        thread::Builder::new().spawn(move || {
            // This is the namespace reaper. Losing the data path must end the workload.
            if let Ok(Err(error)) =
                std::panic::catch_unwind(|| stack::run(self.tun, self.proxy, names, started))
            {
                eprintln!("transparent TCP stack exited: {error}");
            }
            std::process::exit(1);
        })?;

        // The workload is blocked on the pipe. Complete a native handshake through
        // the TUN and check that its socket identity is visible before releasing it.
        // This reserved address is never issued by DNS or forwarded upstream.
        let probe_target = SocketAddr::from(([198, 18, 0, 0], 9));
        let probe = TcpStream::connect_timeout(&probe_target, TIMEOUT)?;
        let source = readiness
            .recv_timeout(TIMEOUT)
            .map_err(io::Error::other)??;
        if source != probe.local_addr()? {
            return Err(io::Error::other("unexpected readiness source"));
        }
        self.ready.write_all(&[1])
    }
}

pub(super) fn accept_with_backoff(listener: &TcpListener) -> io::Result<TcpStream> {
    loop {
        let error = match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error) => error,
        };
        match error.raw_os_error() {
            Some(libc::EINTR | libc::ECONNABORTED) => continue,
            Some(libc::EMFILE | libc::ENFILE | libc::ENOBUFS | libc::ENOMEM) => {
                thread::sleep(Duration::from_millis(100))
            }
            _ => return Err(error),
        }
    }
}

fn connect(
    source: SocketAddr,
    target: SocketAddr,
    proxy: SocketAddr,
    names: &Mutex<Names>,
) -> socks5_protocol::Result<TcpStream> {
    let destination = match names
        .lock()
        .map_err(|_| io::Error::other("DNS name table is unavailable"))?
        .lookup(target.ip())?
    {
        Some(host) => Address::Domain(host, target.port()),
        None => Address::SocketAddr(target),
    };

    let owner = pending::Owner::capture(source, target)?;
    let mut upstream = TcpStream::connect_timeout(&proxy, TIMEOUT)?;
    upstream.set_read_timeout(Some(TIMEOUT))?;
    upstream.set_write_timeout(Some(TIMEOUT))?;
    Version::V5.write_to(&mut upstream)?;
    AuthRequest::new([AuthMethod::Noauth]).write_to(&mut upstream)?;
    Version::read_from(&mut upstream)?;
    if AuthResponse::read_from(&mut upstream)?.method() != AuthMethod::Noauth {
        return Err(io::Error::other("managed SOCKS5 handshake failed").into());
    }
    write_frame(&mut upstream, &CommandRequest::connect(destination))?;
    // The trusted proxy owns the approval deadline. The relay's handshake
    // timeout applies to framing and negotiation, not the human decision.
    if let Err(error) = owner.wait_for_reply(&upstream) {
        // An abort wakes both copies in the host bridge while approval is outstanding.
        let _ = sockopt::set_socket_linger(&upstream, Some(Duration::ZERO));
        return Err(error.into());
    }
    upstream.set_read_timeout(Some(TIMEOUT))?;
    let reply = CommandResponse::read_from(&mut upstream)?;
    if reply.reply != CommandReply::Succeeded
        || matches!(reply.address, Address::Domain(ref name, _) if name.is_empty())
    {
        return Err(io::Error::other("invalid managed SOCKS5 success reply").into());
    }
    upstream.set_nonblocking(true)?;
    Ok(upstream)
}

// Emit the codec's field-by-field CONNECT frame in one write.
fn write_frame(stream: &mut TcpStream, message: &impl FromIO) -> socks5_protocol::Result<()> {
    let mut bytes = Vec::new();
    message.write_to(&mut bytes)?;
    Ok(stream.write_all(&bytes)?)
}

#[cfg(test)]
#[path = "managed_tcp_relay_tests.rs"]
mod tests;
