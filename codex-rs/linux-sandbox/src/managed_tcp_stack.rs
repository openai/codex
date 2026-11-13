//! Drive a bounded set of smoltcp sockets on the TUN inherited from namespace setup.
//! The kernel still owns the native client sockets. Only the managed SOCKS bridge
//! can supply an upstream stream, and only after it has approved the destination.
use super::Names;
use etherparse::IpNumber;
use etherparse::IpSlice;
use etherparse::Ipv6ExtensionSlice;
use etherparse::TcpSlice;
use smoltcp::iface::Config;
use smoltcp::iface::Interface;
use smoltcp::iface::PollResult;
use smoltcp::iface::SocketHandle;
use smoltcp::iface::SocketSet;
use smoltcp::phy::Device;
use smoltcp::phy::DeviceCapabilities;
use smoltcp::phy::Medium;
use smoltcp::phy::RxToken;
use smoltcp::phy::TunTapInterface;
use smoltcp::socket::tcp::Socket;
use smoltcp::socket::tcp::SocketBuffer;
use smoltcp::socket::tcp::State;
use smoltcp::time::Instant;
use smoltcp::wire::HardwareAddress;
use smoltcp::wire::IpCidr;
use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::io::Write;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::Shutdown;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::os::fd::AsRawFd;
use std::os::fd::IntoRawFd;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;

const MTU: usize = 1500;
const MAX_FLOWS: usize = 128;
const WINDOW: usize = 32 * 1024;

struct Packet<'a>(&'a [u8]);
impl RxToken for Packet<'_> {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R {
        f(self.0)
    }
}

// Give smoltcp exactly the frame admitted below. Its ordinary TUN device
// owns transmission, including backpressure and MTU handling.
struct Ingress<'a> {
    device: &'a mut TunTapInterface,
    packet: Option<&'a [u8]>,
}

impl Device for Ingress<'_> {
    type RxToken<'a>
        = Packet<'a>
    where
        Self: 'a;
    type TxToken<'a>
        = <TunTapInterface as Device>::TxToken<'a>
    where
        Self: 'a;

    fn receive(&mut self, now: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        Some((Packet(self.packet.take()?), self.device.transmit(now)?))
    }
    fn transmit(&mut self, now: Instant) -> Option<Self::TxToken<'_>> {
        self.device.transmit(now)
    }
    fn capabilities(&self) -> DeviceCapabilities {
        self.device.capabilities()
    }
}

type Endpoints = (SocketAddr, SocketAddr);

// Allocate a listener only for an opening SYN. smoltcp validates the complete
// packet, tracks sequence numbers, and owns retransmission and flow control.
fn tcp_endpoints(packet: &[u8]) -> Option<(Endpoints, bool)> {
    let ip = IpSlice::from_slice(packet).ok()?;
    if ip.is_fragmenting_payload() || ip.payload_ip_number() != IpNumber::TCP {
        return None;
    }
    // Match the extensions supported by the stack before allocating a listener.
    let supported = match &ip {
        IpSlice::Ipv4(v4) => v4.extensions().auth.is_none(),
        IpSlice::Ipv6(v6) => v6.extensions().clone().into_iter().all(|ext| {
            matches!(ext, Ipv6ExtensionSlice::HopByHop(header) if header.next_header() == IpNumber::TCP)
        }),
    };
    if !supported {
        return None;
    }
    let tcp = TcpSlice::from_slice(ip.payload().payload).ok()?;
    let endpoints = (
        SocketAddr::new(ip.source_addr(), tcp.source_port()),
        SocketAddr::new(ip.destination_addr(), tcp.destination_port()),
    );
    Some((
        endpoints,
        tcp.syn() && !tcp.ack() && !tcp.rst() && !tcp.fin() && tcp.destination_port() != 0,
    ))
}

enum Session {
    Handshake,
    Pending(mpsc::Receiver<TcpStream>),
    Open {
        upstream: TcpStream,
        write_closed: bool,
        read_closed: bool,
    },
    Done,
}

impl Session {
    fn pump(&mut self, socket: &mut Socket<'_>, poll: &mut Vec<libc::pollfd>) -> io::Result<()> {
        if let Self::Pending(result) = self {
            match result.try_recv() {
                Ok(upstream) => {
                    *self = Self::Open {
                        upstream,
                        write_closed: false,
                        read_closed: false,
                    }
                }
                Err(mpsc::TryRecvError::Empty) => return Ok(()),
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(io::ErrorKind::ConnectionAborted.into());
                }
            }
        }
        let Self::Open {
            upstream,
            write_closed,
            read_closed,
        } = self
        else {
            return Ok(());
        };
        if socket.can_recv() {
            socket
                .recv(|bytes| transfer(upstream.write(bytes)))
                .map_err(io::Error::other)??;
        }
        if !socket.may_recv() && !*write_closed {
            upstream.shutdown(Shutdown::Write)?;
            *write_closed = true;
        }
        if !*read_closed && socket.can_send() {
            let n = socket
                .send(|space| transfer(upstream.read(space)))
                .map_err(io::Error::other)??;
            if n == Some(0) {
                socket.close();
                *read_closed = true;
            }
        }
        let mut events = 0;
        if !*read_closed && socket.can_send() {
            events |= libc::POLLIN;
        }
        if socket.can_recv() {
            events |= libc::POLLOUT;
        }
        if events != 0 {
            poll.push(libc::pollfd {
                fd: upstream.as_raw_fd(),
                events,
                revents: 0,
            });
        }
        Ok(())
    }
}

fn transfer(result: io::Result<usize>) -> (usize, io::Result<Option<usize>>) {
    match result {
        Ok(n) => (n, Ok(Some(n))),
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            (0, Ok(None))
        }
        Err(e) => (0, Err(e)),
    }
}

fn interface(tun: &mut TunTapInterface) -> io::Result<Interface> {
    let mut config = Config::new(HardwareAddress::Ip);
    config.random_seed = rand::random();
    let mut iface = Interface::new(config, tun, Instant::ZERO);
    let v4 = Ipv4Addr::new(192, 0, 2, 2);
    let v6 = Ipv6Addr::LOCALHOST;
    let mut addresses_fit = true;
    iface.update_ip_addrs(|ips| {
        for ip in [IpAddr::V4(v4), IpAddr::V6(v6)] {
            addresses_fit &= ips
                .push(IpCidr::new(ip.into(), if ip.is_ipv4() { 32 } else { 128 }))
                .is_ok();
        }
    });
    if !addresses_fit {
        return Err(io::Error::other("insufficient TCP interface addresses"));
    }
    iface
        .routes_mut()
        .add_default_ipv4_route(v4)
        .map_err(io::Error::other)?;
    iface
        .routes_mut()
        .add_default_ipv6_route(v6)
        .map_err(io::Error::other)?;
    iface.set_any_ip(true);
    Ok(iface)
}

pub(super) fn run(
    fd: OwnedFd,
    proxy: SocketAddr,
    names: Arc<Mutex<Names>>,
    started: mpsc::SyncSender<io::Result<SocketAddr>>,
) -> io::Result<()> {
    use rustix::fs::OFlags;
    use rustix::fs::fcntl_getfl;
    use rustix::fs::fcntl_setfl;
    fcntl_setfl(&fd, fcntl_getfl(&fd)? | OFlags::NONBLOCK)?;
    // from_fd takes ownership of the descriptor opened in the private namespace.
    let mut tun = TunTapInterface::from_fd(fd.into_raw_fd(), Medium::Ip, MTU)?;
    let mut iface = interface(&mut tun)?;
    let clock = std::time::Instant::now();
    let mut started = Some(started);
    let mut sockets = SocketSet::new(Vec::new());
    let mut flows: HashMap<Endpoints, (SocketHandle, Session)> = HashMap::new();
    let permits = Arc::new(async_lock::Semaphore::new(MAX_FLOWS));
    let mut poll = Vec::with_capacity(MAX_FLOWS + 1);
    loop {
        let now = Instant::from_micros(clock.elapsed().as_micros() as i64);
        iface.poll_maintenance(now);
        if let Some((rx, _)) = tun.receive(now) {
            rx.consume(|packet| -> io::Result<()> {
                if packet.is_empty() {
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                let Some((key, syn)) = tcp_endpoints(packet) else {
                    return Ok(());
                };
                if syn && !flows.contains_key(&key) {
                    if flows.len() == MAX_FLOWS {
                        return Ok(()); // The kernel retries when a slot opens.
                    }
                    let buffer = || SocketBuffer::new(vec![0; WINDOW]);
                    let mut native = Socket::new(buffer(), buffer());
                    native.set_timeout(Some(super::TIMEOUT.into()));
                    native.listen(key.1).map_err(io::Error::other)?;
                    flows.insert(key, (sockets.add(native), Session::Handshake));
                }
                iface.poll_ingress_single(
                    now,
                    &mut Ingress {
                        device: &mut tun,
                        packet: Some(packet),
                    },
                    &mut sockets,
                );
                Ok(())
            })?;
        }
        iface.poll_egress(now, &mut tun, &mut sockets);
        poll.clear();
        poll.push(libc::pollfd {
            fd: tun.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });
        let mut pending = false;
        flows.retain(|&(src, dst), (handle, session)| {
            let native = sockets.get_mut::<Socket>(*handle);
            if !native.is_active() {
                sockets.remove(*handle);
                return false;
            }
            if matches!(session, Session::Handshake)
                && matches!(native.state(), State::Established | State::CloseWait)
            {
                native.set_timeout(None);
                if let Some(started) = started.take() {
                    let result = if dst == SocketAddr::from(([198, 18, 0, 0], 9)) {
                        super::pending::Owner::capture(src, dst).map(|_| src)
                    } else {
                        Err(io::Error::other("unexpected readiness destination"))
                    };
                    let _ = started.send(result);
                    native.close();
                    *session = Session::Done;
                    return true;
                }
                let Some(permit) = permits.try_acquire_arc() else {
                    pending = true;
                    return true;
                };
                let (reply, result) = mpsc::sync_channel(1);
                let names = Arc::clone(&names);
                let _ = thread::Builder::new().spawn(move || {
                    let _permit = permit;
                    if let Ok(upstream) = super::connect(src, dst, proxy, &names) {
                        let _ = reply.send(upstream);
                    }
                });
                *session = Session::Pending(result);
            }
            if session.pump(native, &mut poll).is_err() {
                native.abort();
                *session = Session::Done;
            }
            pending |= matches!(session, Session::Pending(_));
            true
        });
        while iface.poll_egress(now, &mut tun, &mut sockets) == PollResult::SocketStateChanged {}
        let delay = iface
            .poll_delay(now, &sockets)
            .map(|d| d.total_millis().min(i32::MAX as u64) as i32);
        let timeout = if pending {
            delay.unwrap_or(25).min(25)
        } else {
            delay.unwrap_or(-1)
        };
        // SAFETY: all descriptors in poll are owned by tun or live entries in flows.
        let result = unsafe { libc::poll(poll.as_mut_ptr(), poll.len() as _, timeout) };
        if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return Err(io::Error::last_os_error());
        }
    }
}
