#[cfg(not(any(apple, target_os = "openbsd", solarish)))]
use std::ptr;
use std::{
    io::{self, IoSliceMut},
    mem::{self, MaybeUninit},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    os::unix::io::AsRawFd,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};

use socket2::SockRef;

use super::{
    EcnCodepoint, IO_ERROR_LOG_INTERVAL, RecvMeta, Transmit, UdpSockRef, cmsg, log_sendmsg_error,
};

// Adapted from https://github.com/apple-oss-distributions/xnu/blob/8d741a5de7ff4191bf97d57b9f54c2f6d4a15585/bsd/sys/socket_private.h
#[cfg(apple_fast)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub(crate) struct msghdr_x {
    pub msg_name: *mut libc::c_void,
    pub msg_namelen: libc::socklen_t,
    pub msg_iov: *mut libc::iovec,
    pub msg_iovlen: libc::c_int,
    pub msg_control: *mut libc::c_void,
    pub msg_controllen: libc::socklen_t,
    pub msg_flags: libc::c_int,
    pub msg_datalen: usize,
}

#[cfg(apple_fast)]
extern "C" {
    fn recvmsg_x(
        s: libc::c_int,
        msgp: *const msghdr_x,
        cnt: libc::c_uint,
        flags: libc::c_int,
    ) -> isize;

    fn sendmsg_x(
        s: libc::c_int,
        msgp: *const msghdr_x,
        cnt: libc::c_uint,
        flags: libc::c_int,
    ) -> isize;
}

// Defined in netinet6/in6.h on OpenBSD, this is not yet exported by the libc crate
// directly.  See https://github.com/rust-lang/libc/issues/3704 for when we might be able to
// rely on this from the libc crate.
#[cfg(any(target_os = "openbsd", target_os = "netbsd"))]
const IPV6_DONTFRAG: libc::c_int = 62;
#[cfg(not(any(target_os = "openbsd", target_os = "netbsd")))]
const IPV6_DONTFRAG: libc::c_int = libc::IPV6_DONTFRAG;

#[cfg(target_os = "freebsd")]
type IpTosTy = libc::c_uchar;
#[cfg(not(any(target_os = "freebsd", target_os = "netbsd")))]
type IpTosTy = libc::c_int;

/// Tokio-compatible UDP socket with some useful specializations.
///
/// Unlike a standard tokio UDP socket, this allows ECN bits to be read and written on some
/// platforms.
#[derive(Debug)]
pub struct UdpSocketState {
    last_send_error: Mutex<Instant>,
    max_gso_segments: AtomicUsize,
    gro_segments: usize,
    may_fragment: bool,

    /// True if we have received EINVAL error from `sendmsg` system call at least once.
    ///
    /// If enabled, we assume that old kernel is used and switch to fallback mode.
    /// In particular, we do not use IP_TOS cmsg_type in this case,
    /// which is not supported on Linux <3.13 and results in not sending the UDP packet at all.
    sendmsg_einval: AtomicBool,
}

impl UdpSocketState {
    pub fn new(sock: UdpSockRef<'_>) -> io::Result<Self> {
        let io = sock.0;
        let mut cmsg_platform_space = 0;
        if cfg!(target_os = "linux")
            || cfg!(bsd)
            || cfg!(apple)
            || cfg!(target_os = "android")
            || cfg!(solarish)
        {
            cmsg_platform_space +=
                unsafe { libc::CMSG_SPACE(mem::size_of::<libc::in6_pktinfo>() as _) as usize };
        }

        assert!(
            CMSG_LEN
                >= unsafe { libc::CMSG_SPACE(mem::size_of::<libc::c_int>() as _) as usize }
                    + cmsg_platform_space
        );
        assert!(
            mem::align_of::<libc::cmsghdr>() <= mem::align_of::<cmsg::Aligned<[u8; 0]>>(),
            "control message buffers will be misaligned"
        );

        io.set_nonblocking(true)?;

        let addr = io.local_addr()?;
        let is_ipv4 = addr.family() == libc::AF_INET as libc::sa_family_t;

        // mac and ios do not support IP_RECVTOS on dual-stack sockets :(
        // older macos versions also don't have the flag and will error out if we don't ignore it
        #[cfg(not(any(target_os = "openbsd", target_os = "netbsd", solarish)))]
        if is_ipv4 || !io.only_v6()? {
            if let Err(_err) =
                set_socket_option(&*io, libc::IPPROTO_IP, libc::IP_RECVTOS, OPTION_ON)
            {
                crate::log::debug!("Ignoring error setting IP_RECVTOS on socket: {_err:?}");
            }
        }

        let mut may_fragment = false;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            // opportunistically try to enable GRO. See gro::gro_segments().
            let _ = set_socket_option(&*io, libc::SOL_UDP, gro::UDP_GRO, OPTION_ON);

            // Forbid IPv4 fragmentation. Set even for IPv6 to account for IPv6 mapped IPv4 addresses.
            // Set `may_fragment` to `true` if this option is not supported on the platform.
            may_fragment |= !set_socket_option_supported(
                &*io,
                libc::IPPROTO_IP,
                libc::IP_MTU_DISCOVER,
                libc::IP_PMTUDISC_PROBE,
            )?;

            if is_ipv4 {
                set_socket_option(&*io, libc::IPPROTO_IP, libc::IP_PKTINFO, OPTION_ON)?;
            } else {
                // Set `may_fragment` to `true` if this option is not supported on the platform.
                may_fragment |= !set_socket_option_supported(
                    &*io,
                    libc::IPPROTO_IPV6,
                    libc::IPV6_MTU_DISCOVER,
                    libc::IPV6_PMTUDISC_PROBE,
                )?;
            }
        }
        #[cfg(any(target_os = "freebsd", apple))]
        {
            if is_ipv4 {
                // Set `may_fragment` to `true` if this option is not supported on the platform.
                may_fragment |= !set_socket_option_supported(
                    &*io,
                    libc::IPPROTO_IP,
                    libc::IP_DONTFRAG,
                    OPTION_ON,
                )?;
            }
        }
        #[cfg(any(bsd, apple, solarish))]
        // IP_RECVDSTADDR == IP_SENDSRCADDR on FreeBSD
        // macOS uses only IP_RECVDSTADDR, no IP_SENDSRCADDR on macOS (the same on Solaris)
        // macOS also supports IP_PKTINFO
        {
            if is_ipv4 {
                set_socket_option(&*io, libc::IPPROTO_IP, libc::IP_RECVDSTADDR, OPTION_ON)?;
            }
        }

        // Options standardized in RFC 3542
        if !is_ipv4 {
            set_socket_option(&*io, libc::IPPROTO_IPV6, libc::IPV6_RECVPKTINFO, OPTION_ON)?;
            set_socket_option(&*io, libc::IPPROTO_IPV6, libc::IPV6_RECVTCLASS, OPTION_ON)?;
            // Linux's IP_PMTUDISC_PROBE allows us to operate under interface MTU rather than the
            // kernel's path MTU guess, but actually disabling fragmentation requires this too. See
            // __ip6_append_data in ip6_output.c.
            // Set `may_fragment` to `true` if this option is not supported on the platform.
            may_fragment |=
                !set_socket_option_supported(&*io, libc::IPPROTO_IPV6, IPV6_DONTFRAG, OPTION_ON)?;
        }

        let now = Instant::now();
        Ok(Self {
            last_send_error: Mutex::new(now.checked_sub(2 * IO_ERROR_LOG_INTERVAL).unwrap_or(now)),
            max_gso_segments: AtomicUsize::new(gso::max_gso_segments()),
            gro_segments: gro::gro_segments(),
            may_fragment,
            sendmsg_einval: AtomicBool::new(false),
        })
    }

    /// Sends a [`Transmit`] on the given socket.
    ///
    /// This function will only ever return errors of kind [`io::ErrorKind::WouldBlock`].
    /// All other errors will be logged and converted to `Ok`.
    ///
    /// UDP transmission errors are considered non-fatal because higher-level protocols must
    /// employ retransmits and timeouts anyway in order to deal with UDP's unreliable nature.
    /// Thus, logging is most likely the only thing you can do with these errors.
    ///
    /// If you would like to handle these errors yourself, use [`UdpSocketState::try_send`]
    /// instead.
    pub fn send(&self, socket: UdpSockRef<'_>, transmit: &Transmit<'_>) -> io::Result<()> {
        match send(self, socket.0, transmit) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Err(e),
            // - EMSGSIZE is expected for MTU probes. Future work might be able to avoid
            //   these by automatically clamping the MTUD upper bound to the interface MTU.
            Err(e) if e.raw_os_error() == Some(libc::EMSGSIZE) => Ok(()),
            Err(e) => {
                log_sendmsg_error(&self.last_send_error, e, transmit);

                Ok(())
            }
        }
    }

    /// Sends a [`Transmit`] on the given socket without any additional error handling.
    pub fn try_send(&self, socket: UdpSockRef<'_>, transmit: &Transmit<'_>) -> io::Result<()> {
        send(self, socket.0, transmit)
    }

    pub fn recv(
        &self,
        socket: UdpSockRef<'_>,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> io::Result<usize> {
        recv(socket.0, bufs, meta)
    }

    /// The maximum amount of segments which can be transmitted if a platform
    /// supports Generic Send Offload (GSO).
    ///
    /// This is 1 if the platform doesn't support GSO. Subject to change if errors are detected
    /// while using GSO.
    #[inline]
    pub fn max_gso_segments(&self) -> usize {
        self.max_gso_segments.load(Ordering::Relaxed)
    }

    /// The number of segments to read when GRO is enabled. Used as a factor to
    /// compute the receive buffer size.
    ///
    /// Returns 1 if the platform doesn't support GRO.
    #[inline]
    pub fn gro_segments(&self) -> usize {
        self.gro_segments
    }

    /// Resize the send buffer of `socket` to `bytes`
    #[inline]
    pub fn set_send_buffer_size(&self, socket: UdpSockRef<'_>, bytes: usize) -> io::Result<()> {
        socket.0.set_send_buffer_size(bytes)
    }

    /// Resize the receive buffer of `socket` to `bytes`
    #[inline]
    pub fn set_recv_buffer_size(&self, socket: UdpSockRef<'_>, bytes: usize) -> io::Result<()> {
        socket.0.set_recv_buffer_size(bytes)
    }

    /// Get the size of the `socket` send buffer
    #[inline]
    pub fn send_buffer_size(&self, socket: UdpSockRef<'_>) -> io::Result<usize> {
        socket.0.send_buffer_size()
    }

    /// Get the size of the `socket` receive buffer
    #[inline]
    pub fn recv_buffer_size(&self, socket: UdpSockRef<'_>) -> io::Result<usize> {
        socket.0.recv_buffer_size()
    }

    /// Whether transmitted datagrams might get fragmented by the IP layer
    ///
    /// Returns `false` on targets which employ e.g. the `IPV6_DONTFRAG` socket option.
    #[inline]
    pub fn may_fragment(&self) -> bool {
        self.may_fragment
    }

    /// Returns true if we previously got an EINVAL error from `sendmsg` syscall.
    fn sendmsg_einval(&self) -> bool {
        self.sendmsg_einval.load(Ordering::Relaxed)
    }

    /// Sets the flag indicating we got EINVAL error from `sendmsg` syscall.
    #[cfg(not(any(apple, target_os = "openbsd", target_os = "netbsd")))]
    fn set_sendmsg_einval(&self) {
        self.sendmsg_einval.store(true, Ordering::Relaxed)
    }
}

#[cfg(not(any(apple, target_os = "openbsd", target_os = "netbsd")))]
fn send(
    #[allow(unused_variables)] // only used on Linux
    state: &UdpSocketState,
    io: SockRef<'_>,
    transmit: &Transmit<'_>,
) -> io::Result<()> {
    #[allow(unused_mut)] // only mutable on FreeBSD
    let mut encode_src_ip = true;
    #[cfg(target_os = "freebsd")]
    {
        let addr = io.local_addr()?;
        let is_ipv4 = addr.family() == libc::AF_INET as libc::sa_family_t;
        if is_ipv4 {
            if let Some(socket) = addr.as_socket_ipv4() {
                encode_src_ip = socket.ip() == &Ipv4Addr::UNSPECIFIED;
            }
        }
    }
    let mut msg_hdr: libc::msghdr = unsafe { mem::zeroed() };
    let mut iovec: libc::iovec = unsafe { mem::zeroed() };
    let mut cmsgs = cmsg::Aligned([0u8; CMSG_LEN]);
    let dst_addr = socket2::SockAddr::from(transmit.destination);
    prepare_msg(
        transmit,
        &dst_addr,
        &mut msg_hdr,
        &mut iovec,
        &mut cmsgs,
        encode_src_ip,
        state.sendmsg_einval(),
    );

    loop {
        let n = unsafe { libc::sendmsg(io.as_raw_fd(), &msg_hdr, 0) };

        if n >= 0 {
            return Ok(());
        }

        let e = io::Error::last_os_error();
        match e.kind() {
            // Retry the transmission
            io::ErrorKind::Interrupted => continue,
            io::ErrorKind::WouldBlock => return Err(e),
            _ => {
                // Some network adapters and drivers do not support GSO. Unfortunately, Linux
                // offers no easy way for us to detect this short of an EIO or sometimes EINVAL
                // when we try to actually send datagrams using it.
                #[cfg(any(target_os = "linux", target_os = "android"))]
                if let Some(libc::EIO) | Some(libc::EINVAL) = e.raw_os_error() {
                    // Prevent new transmits from being scheduled using GSO. Existing GSO transmits
                    // may already be in the pipeline, so we need to tolerate additional failures.
                    if state.max_gso_segments() > 1 {
                        crate::log::info!(
                            "`libc::sendmsg` failed with {e}; halting segmentation offload"
                        );
                        state
                            .max_gso_segments
                            .store(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }

                // Some arguments to `sendmsg` are not supported. Switch to
                // fallback mode and retry if we haven't already.
                if e.raw_os_error() == Some(libc::EINVAL) && !state.sendmsg_einval() {
                    state.set_sendmsg_einval();
                    prepare_msg(
                        transmit,
                        &dst_addr,
                        &mut msg_hdr,
                        &mut iovec,
                        &mut cmsgs,
                        encode_src_ip,
                        state.sendmsg_einval(),
                    );
                    continue;
                }

                return Err(e);
            }
        }
    }
}

#[cfg(apple_fast)]
fn send(state: &UdpSocketState, io: SockRef<'_>, transmit: &Transmit<'_>) -> io::Result<()> {
    let mut hdrs = unsafe { mem::zeroed::<[msghdr_x; BATCH_SIZE]>() };
    let mut iovs = unsafe { mem::zeroed::<[libc::iovec; BATCH_SIZE]>() };
    let mut ctrls = [cmsg::Aligned([0u8; CMSG_LEN]); BATCH_SIZE];
    let addr = socket2::SockAddr::from(transmit.destination);
    let segment_size = transmit.segment_size.unwrap_or(transmit.contents.len());
    let mut cnt = 0;
    debug_assert!(transmit.contents.len().div_ceil(segment_size) <= BATCH_SIZE);
    for (i, chunk) in transmit
        .contents
        .chunks(segment_size)
        .enumerate()
        .take(BATCH_SIZE)
    {
        prepare_msg(
            &Transmit {
                destination: transmit.destination,
                ecn: transmit.ecn,
                contents: chunk,
                segment_size: Some(chunk.len()),
                src_ip: transmit.src_ip,
            },
            &addr,
            &mut hdrs[i],
            &mut iovs[i],
            &mut ctrls[i],
            true,
            state.sendmsg_einval(),
        );
        hdrs[i].msg_datalen = chunk.len();
        cnt += 1;
    }
    loop {
        let n = unsafe { sendmsg_x(io.as_raw_fd(), hdrs.as_ptr(), cnt as u32, 0) };

        if n >= 0 {
            return Ok(());
        }

        let e = io::Error::last_os_error();
        match e.kind() {
            // Retry the transmission
            io::ErrorKind::Interrupted => continue,
            _ => return Err(e),
        }
    }
}

#[cfg(any(target_os = "openbsd", target_os = "netbsd", apple_slow))]
fn send(state: &UdpSocketState, io: SockRef<'_>, transmit: &Transmit<'_>) -> io::Result<()> {
    let mut hdr: libc::msghdr = unsafe { mem::zeroed() };
    let mut iov: libc::iovec = unsafe { mem::zeroed() };
    let mut ctrl = cmsg::Aligned([0u8; CMSG_LEN]);
    let addr = socket2::SockAddr::from(transmit.destination);
    prepare_msg(
        transmit,
        &addr,
        &mut hdr,
        &mut iov,
        &mut ctrl,
        cfg!(apple) || cfg!(target_os = "openbsd") || cfg!(target_os = "netbsd"),
        state.sendmsg_einval(),
    );
    loop {
        let n = unsafe { libc::sendmsg(io.as_raw_fd(), &hdr, 0) };

        if n >= 0 {
            return Ok(());
        }

        let e = io::Error::last_os_error();
        match e.kind() {
            // Retry the transmission
            io::ErrorKind::Interrupted => continue,
            _ => return Err(e),
        }
    }
}

#[cfg(not(any(apple, target_os = "openbsd", target_os = "netbsd", solarish)))]
fn recv(io: SockRef<'_>, bufs: &mut [IoSliceMut<'_>], meta: &mut [RecvMeta]) -> io::Result<usize> {
    let mut names = [MaybeUninit::<libc::sockaddr_storage>::uninit(); BATCH_SIZE];
    let mut ctrls = [cmsg::Aligned(MaybeUninit::<[u8; CMSG_LEN]>::uninit()); BATCH_SIZE];
    let mut hdrs = unsafe { mem::zeroed::<[libc::mmsghdr; BATCH_SIZE]>() };
    let max_msg_count = bufs.len().min(BATCH_SIZE);
    for i in 0..max_msg_count {
        prepare_recv(
            &mut bufs[i],
            &mut names[i],
            &mut ctrls[i],
            &mut hdrs[i].msg_hdr,
        );
    }
    let msg_count = loop {
        let n = unsafe {
            libc::recvmmsg(
                io.as_raw_fd(),
                hdrs.as_mut_ptr(),
                bufs.len().min(BATCH_SIZE) as _,
                0,
                ptr::null_mut::<libc::timespec>(),
            )
        };

        if n >= 0 {
            break n;
        }

        let e = io::Error::last_os_error();
        match e.kind() {
            // Retry receiving
            io::ErrorKind::Interrupted => continue,
            _ => return Err(e),
        }
    };
    for i in 0..(msg_count as usize) {
        meta[i] = decode_recv(&names[i], &hdrs[i].msg_hdr, hdrs[i].msg_len as usize);
    }
    Ok(msg_count as usize)
}

#[cfg(apple_fast)]
fn recv(io: SockRef<'_>, bufs: &mut [IoSliceMut<'_>], meta: &mut [RecvMeta]) -> io::Result<usize> {
    let mut names = [MaybeUninit::<libc::sockaddr_storage>::uninit(); BATCH_SIZE];
    // MacOS 10.15 `recvmsg_x` does not override the `msghdr_x`
    // `msg_controllen`. Thus, after the call to `recvmsg_x`, one does not know
    // which control messages have been written to. To prevent reading
    // uninitialized memory, do not use `MaybeUninit` for `ctrls`, instead
    // initialize `ctrls` with `0`s. A control message of all `0`s is
    // automatically skipped by `libc::CMSG_NXTHDR`.
    let mut ctrls = [cmsg::Aligned([0u8; CMSG_LEN]); BATCH_SIZE];
    let mut hdrs = unsafe { mem::zeroed::<[msghdr_x; BATCH_SIZE]>() };
    let max_msg_count = bufs.len().min(BATCH_SIZE);
    for i in 0..max_msg_count {
        prepare_recv(&mut bufs[i], &mut names[i], &mut ctrls[i], &mut hdrs[i]);
    }
    let msg_count = loop {
        let n = unsafe { recvmsg_x(io.as_raw_fd(), hdrs.as_mut_ptr(), max_msg_count as _, 0) };

        if n >= 0 {
            break n;
        }

        let e = io::Error::last_os_error();
        match e.kind() {
            // Retry receiving
            io::ErrorKind::Interrupted => continue,
            _ => return Err(e),
        }
    };
    for i in 0..(msg_count as usize) {
        meta[i] = decode_recv(&names[i], &hdrs[i], hdrs[i].msg_datalen as usize);
    }
    Ok(msg_count as usize)
}

#[cfg(any(target_os = "openbsd", target_os = "netbsd", solarish, apple_slow))]
fn recv(io: SockRef<'_>, bufs: &mut [IoSliceMut<'_>], meta: &mut [RecvMeta]) -> io::Result<usize> {
    let mut name = MaybeUninit::<libc::sockaddr_storage>::uninit();
    let mut ctrl = cmsg::Aligned(MaybeUninit::<[u8; CMSG_LEN]>::uninit());
    let mut hdr = unsafe { mem::zeroed::<libc::msghdr>() };
    prepare_recv(&mut bufs[0], &mut name, &mut ctrl, &mut hdr);
    let n = loop {
        let n = unsafe { libc::recvmsg(io.as_raw_fd(), &mut hdr, 0) };

        if hdr.msg_flags & libc::MSG_TRUNC != 0 {
            continue;
        }

        if n >= 0 {
            break n;
        }

        let e = io::Error::last_os_error();
        match e.kind() {
            // Retry receiving
            io::ErrorKind::Interrupted => continue,
            _ => return Err(e),
        }
    };
    meta[0] = decode_recv(&name, &hdr, n as usize);
    Ok(1)
}

const CMSG_LEN: usize = 88;

fn prepare_msg(
    transmit: &Transmit<'_>,
    dst_addr: &socket2::SockAddr,
    #[cfg(not(apple_fast))] hdr: &mut libc::msghdr,
    #[cfg(apple_fast)] hdr: &mut msghdr_x,
    iov: &mut libc::iovec,
    ctrl: &mut cmsg::Aligned<[u8; CMSG_LEN]>,
    #[allow(unused_variables)] // only used on FreeBSD & macOS
    encode_src_ip: bool,
    sendmsg_einval: bool,
) {
    iov.iov_base = transmit.contents.as_ptr() as *const _ as *mut _;
    iov.iov_len = transmit.contents.len();

    // SAFETY: Casting the pointer to a mutable one is legal,
    // as sendmsg is guaranteed to not alter the mutable pointer
    // as per the POSIX spec. See the section on the sys/socket.h
    // header for details. The type is only mutable in the first
    // place because it is reused by recvmsg as well.
    let name = dst_addr.as_ptr() as *mut libc::c_void;
    let namelen = dst_addr.len();
    hdr.msg_name = name as *mut _;
    hdr.msg_namelen = namelen;
    hdr.msg_iov = iov;
    hdr.msg_iovlen = 1;

    hdr.msg_control = ctrl.0.as_mut_ptr() as _;
    hdr.msg_controllen = CMSG_LEN as _;
    let mut encoder = unsafe { cmsg::Encoder::new(hdr) };
    let ecn = transmit.ecn.map_or(0, |x| x as libc::c_int);
    // True for IPv4 or IPv4-Mapped IPv6
    let is_ipv4 = transmit.destination.is_ipv4()
        || matches!(transmit.destination.ip(), IpAddr::V6(addr) if addr.to_ipv4_mapped().is_some());
    if is_ipv4 {
        if !sendmsg_einval {
            #[cfg(not(target_os = "netbsd"))]
            {
                encoder.push(libc::IPPROTO_IP, libc::IP_TOS, ecn as IpTosTy);
            }
        }
    } else {
        encoder.push(libc::IPPROTO_IPV6, libc::IPV6_TCLASS, ecn);
    }

    // Only set the segment size if it is less than the size of the contents.
    // Some network drivers don't like being told to do GSO even if there is effectively only a single segment (i.e. `segment_size == transmit.contents.len()`)
    // Additionally, a `segment_size` that is greater than the content also means there is effectively only a single segment.
    // This case is actually quite common when splitting up a prepared GSO batch again after GSO has been disabled because the last datagram in a GSO batch is allowed to be smaller than the segment size.
    if let Some(segment_size) = transmit
        .segment_size
        .filter(|segment_size| *segment_size < transmit.contents.len())
    {
        gso::set_segment_size(&mut encoder, segment_size as u16);
    }

    if let Some(ip) = &transmit.src_ip {
        match ip {
            IpAddr::V4(v4) => {
                #[cfg(any(target_os = "linux", target_os = "android"))]
                {
                    let pktinfo = libc::in_pktinfo {
                        ipi_ifindex: 0,
                        ipi_spec_dst: libc::in_addr {
                            s_addr: u32::from_ne_bytes(v4.octets()),
                        },
                        ipi_addr: libc::in_addr { s_addr: 0 },
                    };
                    encoder.push(libc::IPPROTO_IP, libc::IP_PKTINFO, pktinfo);
                }
                #[cfg(any(bsd, apple, solarish))]
                {
                    if encode_src_ip {
                        let addr = libc::in_addr {
                            s_addr: u32::from_ne_bytes(v4.octets()),
                        };
                        encoder.push(libc::IPPROTO_IP, libc::IP_RECVDSTADDR, addr);
                    }
                }
            }
            IpAddr::V6(v6) => {
                let pktinfo = libc::in6_pktinfo {
                    ipi6_ifindex: 0,
                    ipi6_addr: libc::in6_addr {
                        s6_addr: v6.octets(),
                    },
                };
                encoder.push(libc::IPPROTO_IPV6, libc::IPV6_PKTINFO, pktinfo);
            }
        }
    }

    encoder.finish();
}

#[cfg(not(apple_fast))]
fn prepare_recv(
    buf: &mut IoSliceMut,
    name: &mut MaybeUninit<libc::sockaddr_storage>,
    ctrl: &mut cmsg::Aligned<MaybeUninit<[u8; CMSG_LEN]>>,
    hdr: &mut libc::msghdr,
) {
    hdr.msg_name = name.as_mut_ptr() as _;
    hdr.msg_namelen = mem::size_of::<libc::sockaddr_storage>() as _;
    hdr.msg_iov = buf as *mut IoSliceMut as *mut libc::iovec;
    hdr.msg_iovlen = 1;
    hdr.msg_control = ctrl.0.as_mut_ptr() as _;
    hdr.msg_controllen = CMSG_LEN as _;
    hdr.msg_flags = 0;
}

#[cfg(apple_fast)]
fn prepare_recv(
    buf: &mut IoSliceMut,
    name: &mut MaybeUninit<libc::sockaddr_storage>,
    ctrl: &mut cmsg::Aligned<[u8; CMSG_LEN]>,
    hdr: &mut msghdr_x,
) {
    hdr.msg_name = name.as_mut_ptr() as _;
    hdr.msg_namelen = mem::size_of::<libc::sockaddr_storage>() as _;
    hdr.msg_iov = buf as *mut IoSliceMut as *mut libc::iovec;
    hdr.msg_iovlen = 1;
    hdr.msg_control = ctrl.0.as_mut_ptr() as _;
    hdr.msg_controllen = CMSG_LEN as _;
    hdr.msg_flags = 0;
    hdr.msg_datalen = buf.len();
}

fn decode_recv(
    name: &MaybeUninit<libc::sockaddr_storage>,
    #[cfg(not(apple_fast))] hdr: &libc::msghdr,
    #[cfg(apple_fast)] hdr: &msghdr_x,
    len: usize,
) -> RecvMeta {
    let name = unsafe { name.assume_init() };
    let mut ecn_bits = 0;
    let mut dst_ip = None;
    #[allow(unused_mut)] // only mutable on Linux
    let mut stride = len;

    let cmsg_iter = unsafe { cmsg::Iter::new(hdr) };
    for cmsg in cmsg_iter {
        match (cmsg.cmsg_level, cmsg.cmsg_type) {
            (libc::IPPROTO_IP, libc::IP_TOS) => unsafe {
                ecn_bits = cmsg::decode::<u8, libc::cmsghdr>(cmsg);
            },
            // FreeBSD uses IP_RECVTOS here, and we can be liberal because cmsgs are opt-in.
            #[cfg(not(any(target_os = "openbsd", target_os = "netbsd", solarish)))]
            (libc::IPPROTO_IP, libc::IP_RECVTOS) => unsafe {
                ecn_bits = cmsg::decode::<u8, libc::cmsghdr>(cmsg);
            },
            (libc::IPPROTO_IPV6, libc::IPV6_TCLASS) => unsafe {
                // Temporary hack around broken macos ABI. Remove once upstream fixes it.
                // https://bugreport.apple.com/web/?problemID=48761855
                #[allow(clippy::unnecessary_cast)] // cmsg.cmsg_len defined as size_t
                if cfg!(apple)
                    && cmsg.cmsg_len as usize == libc::CMSG_LEN(mem::size_of::<u8>() as _) as usize
                {
                    ecn_bits = cmsg::decode::<u8, libc::cmsghdr>(cmsg);
                } else {
                    ecn_bits = cmsg::decode::<libc::c_int, libc::cmsghdr>(cmsg) as u8;
                }
            },
            #[cfg(any(target_os = "linux", target_os = "android"))]
            (libc::IPPROTO_IP, libc::IP_PKTINFO) => {
                let pktinfo = unsafe { cmsg::decode::<libc::in_pktinfo, libc::cmsghdr>(cmsg) };
                dst_ip = Some(IpAddr::V4(Ipv4Addr::from(
                    pktinfo.ipi_addr.s_addr.to_ne_bytes(),
                )));
            }
            #[cfg(any(bsd, apple))]
            (libc::IPPROTO_IP, libc::IP_RECVDSTADDR) => {
                let in_addr = unsafe { cmsg::decode::<libc::in_addr, libc::cmsghdr>(cmsg) };
                dst_ip = Some(IpAddr::V4(Ipv4Addr::from(in_addr.s_addr.to_ne_bytes())));
            }
            (libc::IPPROTO_IPV6, libc::IPV6_PKTINFO) => {
                let pktinfo = unsafe { cmsg::decode::<libc::in6_pktinfo, libc::cmsghdr>(cmsg) };
                dst_ip = Some(IpAddr::V6(Ipv6Addr::from(pktinfo.ipi6_addr.s6_addr)));
            }
            #[cfg(any(target_os = "linux", target_os = "android"))]
            (libc::SOL_UDP, gro::UDP_GRO) => unsafe {
                stride = cmsg::decode::<libc::c_int, libc::cmsghdr>(cmsg) as usize;
            },
            _ => {}
        }
    }

    let addr = match libc::c_int::from(name.ss_family) {
        libc::AF_INET => {
            // Safety: if the ss_family field is AF_INET then storage must be a sockaddr_in.
            let addr: &libc::sockaddr_in =
                unsafe { &*(&name as *const _ as *const libc::sockaddr_in) };
            SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes()),
                u16::from_be(addr.sin_port),
            ))
        }
        libc::AF_INET6 => {
            // Safety: if the ss_family field is AF_INET6 then storage must be a sockaddr_in6.
            let addr: &libc::sockaddr_in6 =
                unsafe { &*(&name as *const _ as *const libc::sockaddr_in6) };
            SocketAddr::V6(SocketAddrV6::new(
                Ipv6Addr::from(addr.sin6_addr.s6_addr),
                u16::from_be(addr.sin6_port),
                addr.sin6_flowinfo,
                addr.sin6_scope_id,
            ))
        }
        _ => unreachable!(),
    };

    RecvMeta {
        len,
        stride,
        addr,
        ecn: EcnCodepoint::from_bits(ecn_bits),
        dst_ip,
    }
}

#[cfg(not(apple_slow))]
// Chosen somewhat arbitrarily; might benefit from additional tuning.
pub(crate) const BATCH_SIZE: usize = 32;

#[cfg(apple_slow)]
pub(crate) const BATCH_SIZE: usize = 1;

#[cfg(any(target_os = "linux", target_os = "android"))]
mod gso {
    use super::*;
    use std::{ffi::CStr, mem, str::FromStr, sync::OnceLock};

    #[cfg(not(target_os = "android"))]
    const UDP_SEGMENT: libc::c_int = libc::UDP_SEGMENT;
    #[cfg(target_os = "android")]
    // TODO: Add this to libc
    const UDP_SEGMENT: libc::c_int = 103;

    // Support for UDP GSO has been added to linux kernel in version 4.18
    // https://github.com/torvalds/linux/commit/cb586c63e3fc5b227c51fd8c4cb40b34d3750645
    const SUPPORTED_SINCE: KernelVersion = KernelVersion {
        version: 4,
        major_revision: 18,
    };

    /// Checks whether GSO support is available by checking the kernel version followed by setting
    /// the UDP_SEGMENT option on a socket
    pub(crate) fn max_gso_segments() -> usize {
        const GSO_SIZE: libc::c_int = 1500;

        if !SUPPORTED_BY_CURRENT_KERNEL.get_or_init(supported_by_current_kernel) {
            return 1;
        }

        let socket = match std::net::UdpSocket::bind("[::]:0")
            .or_else(|_| std::net::UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)))
        {
            Ok(socket) => socket,
            Err(_) => return 1,
        };

        // As defined in linux/udp.h
        // #define UDP_MAX_SEGMENTS        (1 << 6UL)
        match set_socket_option(&socket, libc::SOL_UDP, UDP_SEGMENT, GSO_SIZE) {
            Ok(()) => 64,
            Err(_e) => {
                crate::log::debug!(
                    "failed to set `UDP_SEGMENT` socket option ({_e}); setting `max_gso_segments = 1`"
                );

                1
            }
        }
    }

    pub(crate) fn set_segment_size(encoder: &mut cmsg::Encoder<libc::msghdr>, segment_size: u16) {
        encoder.push(libc::SOL_UDP, UDP_SEGMENT, segment_size);
    }

    // Avoid calling `supported_by_current_kernel` for each socket by using `OnceLock`.
    static SUPPORTED_BY_CURRENT_KERNEL: OnceLock<bool> = OnceLock::new();

    fn supported_by_current_kernel() -> bool {
        let kernel_version_string = match kernel_version_string() {
            Ok(kernel_version_string) => kernel_version_string,
            Err(_e) => {
                crate::log::warn!("GSO disabled: uname returned {_e}");
                return false;
            }
        };

        let Some(kernel_version) = KernelVersion::from_str(&kernel_version_string) else {
            crate::log::warn!(
                "GSO disabled: failed to parse kernel version ({kernel_version_string})"
            );
            return false;
        };

        if kernel_version < SUPPORTED_SINCE {
            crate::log::info!("GSO disabled: kernel too old ({kernel_version_string}); need 4.18+",);
            return false;
        }

        true
    }

    fn kernel_version_string() -> io::Result<String> {
        let mut n = unsafe { mem::zeroed() };
        let r = unsafe { libc::uname(&mut n) };
        if r != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe {
            CStr::from_ptr(n.release[..].as_ptr())
                .to_string_lossy()
                .into_owned()
        })
    }

    // https://www.linfo.org/kernel_version_numbering.html
    #[derive(Eq, PartialEq, Ord, PartialOrd, Debug)]
    struct KernelVersion {
        version: u8,
        major_revision: u8,
    }

    impl KernelVersion {
        fn from_str(release: &str) -> Option<Self> {
            let mut split = release
                .split_once('-')
                .map(|pair| pair.0)
                .unwrap_or(release)
                .split('.');

            let version = u8::from_str(split.next()?).ok()?;
            let major_revision = u8::from_str(split.next()?).ok()?;

            Some(Self {
                version,
                major_revision,
            })
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn parse_current_kernel_version_release_string() {
            let release = kernel_version_string().unwrap();
            KernelVersion::from_str(&release).unwrap();
        }

        #[test]
        fn parse_kernel_version_release_string() {
            // These are made up for the test
            assert_eq!(
                KernelVersion::from_str("4.14"),
                Some(KernelVersion {
                    version: 4,
                    major_revision: 14
                })
            );
            assert_eq!(
                KernelVersion::from_str("4.18"),
                Some(KernelVersion {
                    version: 4,
                    major_revision: 18
                })
            );
            // These were seen in the wild
            assert_eq!(
                KernelVersion::from_str("4.14.186-27095505"),
                Some(KernelVersion {
                    version: 4,
                    major_revision: 14
                })
            );
            assert_eq!(
                KernelVersion::from_str("6.8.0-59-generic"),
                Some(KernelVersion {
                    version: 6,
                    major_revision: 8
                })
            );
        }
    }
}

// On Apple platforms using the `sendmsg_x` call, UDP datagram segmentation is not
// offloaded to the NIC or even the kernel, but instead done here in user space in
// [`send`]) and then passed to the OS as individual `iovec`s (up to `BATCH_SIZE`).
#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod gso {
    use super::*;

    pub(super) fn max_gso_segments() -> usize {
        #[cfg(apple_fast)]
        {
            BATCH_SIZE
        }
        #[cfg(not(apple_fast))]
        {
            1
        }
    }

    pub(super) fn set_segment_size(
        #[cfg(not(apple_fast))] _encoder: &mut cmsg::Encoder<libc::msghdr>,
        #[cfg(apple_fast)] _encoder: &mut cmsg::Encoder<msghdr_x>,
        _segment_size: u16,
    ) {
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod gro {
    use super::*;

    #[cfg(not(target_os = "android"))]
    pub(crate) const UDP_GRO: libc::c_int = libc::UDP_GRO;
    #[cfg(target_os = "android")]
    // TODO: Add this to libc
    pub(crate) const UDP_GRO: libc::c_int = 104;

    pub(crate) fn gro_segments() -> usize {
        let socket = match std::net::UdpSocket::bind("[::]:0")
            .or_else(|_| std::net::UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)))
        {
            Ok(socket) => socket,
            Err(_) => return 1,
        };

        // As defined in net/ipv4/udp_offload.c
        // #define UDP_GRO_CNT_MAX 64
        //
        // NOTE: this MUST be set to UDP_GRO_CNT_MAX to ensure that the receive buffer size
        // (get_max_udp_payload_size() * gro_segments()) is large enough to hold the largest GRO
        // list the kernel might potentially produce. See
        // https://github.com/quinn-rs/quinn/pull/1354.
        match set_socket_option(&socket, libc::SOL_UDP, UDP_GRO, OPTION_ON) {
            Ok(()) => 64,
            Err(_) => 1,
        }
    }
}

/// Returns whether the given socket option is supported on the current platform
///
/// Yields `Ok(true)` if the option was set successfully, `Ok(false)` if setting
/// the option raised an `ENOPROTOOPT` or `EOPNOTSUPP` error, and `Err` for any other error.
fn set_socket_option_supported(
    socket: &impl AsRawFd,
    level: libc::c_int,
    name: libc::c_int,
    value: libc::c_int,
) -> io::Result<bool> {
    match set_socket_option(socket, level, name, value) {
        Ok(()) => Ok(true),
        Err(err) if err.raw_os_error() == Some(libc::ENOPROTOOPT) => Ok(false),
        Err(err) if err.raw_os_error() == Some(libc::EOPNOTSUPP) => Ok(false),
        Err(err) => Err(err),
    }
}

fn set_socket_option(
    socket: &impl AsRawFd,
    level: libc::c_int,
    name: libc::c_int,
    value: libc::c_int,
) -> io::Result<()> {
    let rc = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            level,
            name,
            &value as *const _ as _,
            mem::size_of_val(&value) as _,
        )
    };

    match rc == 0 {
        true => Ok(()),
        false => Err(io::Error::last_os_error()),
    }
}

const OPTION_ON: libc::c_int = 1;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod gro {
    pub(super) fn gro_segments() -> usize {
        1
    }
}
