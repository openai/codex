//! Create a private user and network namespace before invoking stock bubblewrap.
//! Only the trusted launcher configures the TUN and the resolver sockets.

use rustix::thread::CapabilitySet;
use rustix::thread::CapabilitySets;
use rustix::thread::UnshareFlags;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::TcpListener;
use std::net::UdpSocket;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;
use std::os::unix::fs::OpenOptionsExt;

// linux/ipv6_route.h. libc's in6_rtmsg fields are private.
#[repr(C)]
struct Ipv6Route {
    dst: libc::in6_addr,
    src: libc::in6_addr,
    gateway: libc::in6_addr,
    kind: u32,
    dst_len: u16,
    src_len: u16,
    metric: u32,
    info: libc::c_ulong,
    flags: u32,
    ifindex: libc::c_int,
}
const _: () = assert!(std::mem::size_of::<Ipv6Route>() == std::mem::size_of::<libc::in6_rtmsg>());

pub(super) fn create() -> io::Result<[File; 3]> {
    let uid = rustix::process::geteuid().as_raw();
    let gid = rustix::process::getegid().as_raw();
    // SAFETY: this launcher is single-threaded and keeps its file descriptor table.
    unsafe { rustix::thread::unshare_unsafe(UnshareFlags::NEWUSER | UnshareFlags::NEWNET)? };
    fs::write("/proc/self/uid_map", format!("{uid} {uid} 1\n"))?;
    fs::write("/proc/self/setgroups", "deny")?;
    fs::write("/proc/self/gid_map", format!("{gid} {gid} 1\n"))?;
    let tun = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open("/dev/net/tun")?;
    crate::proxy_routing::ensure_loopback_interface_up()?;
    // SAFETY: these Linux ioctl records consist of integers, byte arrays and
    // nullable pointers. Start with zeroed padding and set the active union
    // member to the type required by each specific ioctl before invoking it.
    unsafe {
        let name = c"codextun";
        let mut iface: libc::ifreq = std::mem::zeroed();
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            iface.ifr_name.as_mut_ptr(),
            name.to_bytes_with_nul().len(),
        );
        iface.ifr_ifru.ifru_flags = (libc::IFF_TUN | libc::IFF_NO_PI) as i16;
        ioctl(tun.as_raw_fd(), libc::TUNSETIFF, &mut iface)?;
        let v4 = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        let v6 = UdpSocket::bind((Ipv6Addr::UNSPECIFIED, 0))?;
        ioctl(v4.as_raw_fd(), libc::SIOCGIFFLAGS as _, &mut iface)?;
        iface.ifr_ifru.ifru_flags |= libc::IFF_UP as i16;
        ioctl(v4.as_raw_fd(), libc::SIOCSIFFLAGS as _, &mut iface)?;
        let index = libc::if_nametoindex(name.as_ptr()) as i32;
        if index == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut addr: libc::sockaddr_in = std::mem::zeroed();
        addr.sin_family = libc::AF_INET as _;
        addr.sin_addr.s_addr = u32::from_ne_bytes([192, 0, 2, 1]);
        iface.ifr_ifru.ifru_addr = std::mem::transmute::<libc::sockaddr_in, libc::sockaddr>(addr);
        ioctl(v4.as_raw_fd(), libc::SIOCSIFADDR as _, &mut iface)?;
        addr.sin_addr.s_addr = u32::MAX;
        iface.ifr_ifru.ifru_netmask =
            std::mem::transmute::<libc::sockaddr_in, libc::sockaddr>(addr);
        ioctl(v4.as_raw_fd(), libc::SIOCSIFNETMASK as _, &mut iface)?;
        let mut addr6: libc::in6_ifreq = std::mem::zeroed();
        addr6.ifr6_addr.s6_addr = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1).octets();
        addr6.ifr6_prefixlen = 128;
        addr6.ifr6_ifindex = index;
        ioctl(v6.as_raw_fd(), libc::SIOCSIFADDR as _, &mut addr6)?;
        let mut route: libc::rtentry = std::mem::zeroed();
        route.rt_dst.sa_family = libc::AF_INET as _;
        route.rt_genmask.sa_family = libc::AF_INET as _;
        route.rt_gateway.sa_family = libc::AF_INET as _;
        route.rt_flags = libc::RTF_UP;
        route.rt_dev = iface.ifr_name.as_mut_ptr();
        ioctl(v4.as_raw_fd(), libc::SIOCADDRT as _, &mut route)?;
        let mut route6: Ipv6Route = std::mem::zeroed();
        route6.ifindex = index;
        route6.flags = u32::from(libc::RTF_UP);
        route6.metric = 1;
        ioctl(v6.as_raw_fd(), libc::SIOCADDRT as _, &mut route6)?;
    }
    let dns = (Ipv4Addr::new(127, 0, 0, 53), 53);
    let udp = UdpSocket::bind(dns)?;
    let tcp = TcpListener::bind(dns)?;
    // Linux 5.12+ requires SETFCAP to map parent UID 0 into bwrap's nested
    // namespace. The inner stage verifies that bwrap dropped this final capability.
    let keep = if uid == 0 {
        CapabilitySet::SETFCAP
    } else {
        CapabilitySet::empty()
    };
    rustix::thread::set_no_new_privs(true)?;
    rustix::thread::set_capabilities(
        None,
        CapabilitySets {
            effective: keep,
            permitted: keep,
            inheritable: CapabilitySet::empty(),
        },
    )?;
    let [tun, udp, tcp] = [OwnedFd::from(tun), udp.into(), tcp.into()];
    Ok([
        File::from(crate::proxy_lifecycle::move_fd_above_stdio(tun)?),
        File::from(crate::proxy_lifecycle::move_fd_above_stdio(udp)?),
        File::from(crate::proxy_lifecycle::move_fd_above_stdio(tcp)?),
    ])
}

// SAFETY: the caller pairs each request number with its exact Linux ABI record.
unsafe fn ioctl<T>(fd: RawFd, request: libc::Ioctl, record: &mut T) -> io::Result<()> {
    if unsafe { libc::ioctl(fd, request, record) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
