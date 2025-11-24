use crate::prelude::*;
use crate::Ioctl;

s! {
    pub struct termios2 {
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_line: crate::cc_t,
        pub c_cc: [crate::cc_t; 19],
        pub c_ispeed: crate::speed_t,
        pub c_ospeed: crate::speed_t,
    }
}

// arch/sparc/include/uapi/asm/socket.h
pub const SOL_SOCKET: c_int = 0xffff;

// Defined in unix/linux_like/mod.rs
// pub const SO_DEBUG: c_int = 0x0001;
pub const SO_PASSCRED: c_int = 0x0002;
pub const SO_REUSEADDR: c_int = 0x0004;
pub const SO_KEEPALIVE: c_int = 0x0008;
pub const SO_DONTROUTE: c_int = 0x0010;
pub const SO_BROADCAST: c_int = 0x0020;
pub const SO_PEERCRED: c_int = 0x0040;
pub const SO_LINGER: c_int = 0x0080;
pub const SO_OOBINLINE: c_int = 0x0100;
pub const SO_REUSEPORT: c_int = 0x0200;
pub const SO_BSDCOMPAT: c_int = 0x0400;
pub const SO_RCVLOWAT: c_int = 0x0800;
pub const SO_SNDLOWAT: c_int = 0x1000;
pub const SO_RCVTIMEO: c_int = 0x2000;
pub const SO_SNDTIMEO: c_int = 0x4000;
// pub const SO_RCVTIMEO_OLD: c_int = 0x2000;
// pub const SO_SNDTIMEO_OLD: c_int = 0x4000;
pub const SO_ACCEPTCONN: c_int = 0x8000;
pub const SO_SNDBUF: c_int = 0x1001;
pub const SO_RCVBUF: c_int = 0x1002;
pub const SO_SNDBUFFORCE: c_int = 0x100a;
pub const SO_RCVBUFFORCE: c_int = 0x100b;
pub const SO_ERROR: c_int = 0x1007;
pub const SO_TYPE: c_int = 0x1008;
pub const SO_PROTOCOL: c_int = 0x1028;
pub const SO_DOMAIN: c_int = 0x1029;
pub const SO_NO_CHECK: c_int = 0x000b;
pub const SO_PRIORITY: c_int = 0x000c;
pub const SO_BINDTODEVICE: c_int = 0x000d;
pub const SO_ATTACH_FILTER: c_int = 0x001a;
pub const SO_DETACH_FILTER: c_int = 0x001b;
pub const SO_GET_FILTER: c_int = SO_ATTACH_FILTER;
pub const SO_PEERNAME: c_int = 0x001c;
pub const SO_PEERSEC: c_int = 0x001e;
pub const SO_PASSSEC: c_int = 0x001f;
pub const SO_MARK: c_int = 0x0022;
pub const SO_RXQ_OVFL: c_int = 0x0024;
pub const SO_WIFI_STATUS: c_int = 0x0025;
pub const SCM_WIFI_STATUS: c_int = SO_WIFI_STATUS;
pub const SO_PEEK_OFF: c_int = 0x0026;
pub const SO_NOFCS: c_int = 0x0027;
pub const SO_LOCK_FILTER: c_int = 0x0028;
pub const SO_SELECT_ERR_QUEUE: c_int = 0x0029;
pub const SO_BUSY_POLL: c_int = 0x0030;
pub const SO_MAX_PACING_RATE: c_int = 0x0031;
pub const SO_BPF_EXTENSIONS: c_int = 0x0032;
pub const SO_INCOMING_CPU: c_int = 0x0033;
pub const SO_ATTACH_BPF: c_int = 0x0034;
pub const SO_DETACH_BPF: c_int = SO_DETACH_FILTER;
pub const SO_ATTACH_REUSEPORT_CBPF: c_int = 0x0035;
pub const SO_ATTACH_REUSEPORT_EBPF: c_int = 0x0036;
pub const SO_CNX_ADVICE: c_int = 0x0037;
pub const SCM_TIMESTAMPING_OPT_STATS: c_int = 0x0038;
pub const SO_MEMINFO: c_int = 0x0039;
pub const SO_INCOMING_NAPI_ID: c_int = 0x003a;
pub const SO_COOKIE: c_int = 0x003b;
pub const SCM_TIMESTAMPING_PKTINFO: c_int = 0x003c;
pub const SO_PEERGROUPS: c_int = 0x003d;
pub const SO_ZEROCOPY: c_int = 0x003e;
pub const SO_TXTIME: c_int = 0x003f;
pub const SCM_TXTIME: c_int = SO_TXTIME;
pub const SO_BINDTOIFINDEX: c_int = 0x0041;
pub const SO_SECURITY_AUTHENTICATION: c_int = 0x5001;
pub const SO_SECURITY_ENCRYPTION_TRANSPORT: c_int = 0x5002;
pub const SO_SECURITY_ENCRYPTION_NETWORK: c_int = 0x5004;
pub const SO_TIMESTAMP: c_int = 0x001d;
pub const SO_TIMESTAMPNS: c_int = 0x0021;
pub const SO_TIMESTAMPING: c_int = 0x0023;
// pub const SO_TIMESTAMP_OLD: c_int = 0x001d;
// pub const SO_TIMESTAMPNS_OLD: c_int = 0x0021;
// pub const SO_TIMESTAMPING_OLD: c_int = 0x0023;
// pub const SO_TIMESTAMP_NEW: c_int = 0x0046;
// pub const SO_TIMESTAMPNS_NEW: c_int = 0x0042;
// pub const SO_TIMESTAMPING_NEW: c_int = 0x0043;
// pub const SO_RCVTIMEO_NEW: c_int = 0x0044;
// pub const SO_SNDTIMEO_NEW: c_int = 0x0045;
// pub const SO_DETACH_REUSEPORT_BPF: c_int = 0x0047;
pub const SO_PREFER_BUSY_POLL: c_int = 0x0048;
pub const SO_BUSY_POLL_BUDGET: c_int = 0x0049;
pub const SO_NETNS_COOKIE: c_int = 0x0050;
pub const SO_BUF_LOCK: c_int = 0x0051;
pub const SO_RESERVE_MEM: c_int = 0x0052;
pub const SO_TXREHASH: c_int = 0x0053;
pub const SO_RCVMARK: c_int = 0x0054;
pub const SO_PASSPIDFD: c_int = 0x0055;
pub const SO_PEERPIDFD: c_int = 0x0056;
pub const SO_DEVMEM_LINEAR: c_int = 0x0057;
pub const SO_DEVMEM_DMABUF: c_int = 0x0058;
pub const SO_DEVMEM_DONTNEED: c_int = 0x0059;

// Defined in unix/linux_like/mod.rs
// pub const SCM_TIMESTAMP: c_int = SO_TIMESTAMP;
pub const SCM_TIMESTAMPNS: c_int = SO_TIMESTAMPNS;
pub const SCM_TIMESTAMPING: c_int = SO_TIMESTAMPING;

pub const SCM_DEVMEM_LINEAR: c_int = SO_DEVMEM_LINEAR;
pub const SCM_DEVMEM_DMABUF: c_int = SO_DEVMEM_DMABUF;

// Ioctl Constants

pub const TCGETS: Ioctl = 0x40245408;
pub const TCSETS: Ioctl = 0x80245409;
pub const TCSETSW: Ioctl = 0x8024540a;
pub const TCSETSF: Ioctl = 0x8024540b;
pub const TCGETA: Ioctl = 0x40125401;
pub const TCSETA: Ioctl = 0x80125402;
pub const TCSETAW: Ioctl = 0x80125403;
pub const TCSETAF: Ioctl = 0x80125404;
pub const TCSBRK: Ioctl = 0x20005405;
pub const TCXONC: Ioctl = 0x20005406;
pub const TCFLSH: Ioctl = 0x20005407;
pub const TIOCEXCL: Ioctl = 0x2000740d;
pub const TIOCNXCL: Ioctl = 0x2000740e;
pub const TIOCSCTTY: Ioctl = 0x20007484;
pub const TIOCGPGRP: Ioctl = 0x40047483;
pub const TIOCSPGRP: Ioctl = 0x80047482;
pub const TIOCOUTQ: Ioctl = 0x40047473;
pub const TIOCSTI: Ioctl = 0x80017472;
pub const TIOCGWINSZ: Ioctl = 0x40087468;
pub const TIOCSWINSZ: Ioctl = 0x80087467;
pub const TIOCMGET: Ioctl = 0x4004746a;
pub const TIOCMBIS: Ioctl = 0x8004746c;
pub const TIOCMBIC: Ioctl = 0x8004746b;
pub const TIOCMSET: Ioctl = 0x8004746d;
pub const TIOCGSOFTCAR: Ioctl = 0x40047464;
pub const TIOCSSOFTCAR: Ioctl = 0x80047465;
pub const FIONREAD: Ioctl = 0x4004667f;
pub const TIOCINQ: Ioctl = FIONREAD;
pub const TIOCLINUX: Ioctl = 0x541C;
pub const TIOCCONS: Ioctl = 0x20007424;
pub const TIOCGSERIAL: Ioctl = 0x541E;
pub const TIOCSSERIAL: Ioctl = 0x541F;
pub const TIOCPKT: Ioctl = 0x80047470;
pub const FIONBIO: Ioctl = 0x8004667e;
pub const TIOCNOTTY: Ioctl = 0x20007471;
pub const TIOCSETD: Ioctl = 0x80047401;
pub const TIOCGETD: Ioctl = 0x40047400;
pub const TCSBRKP: Ioctl = 0x5425;
pub const TIOCSBRK: Ioctl = 0x2000747b;
pub const TIOCCBRK: Ioctl = 0x2000747a;
pub const TIOCGSID: Ioctl = 0x40047485;
pub const TCGETS2: Ioctl = 0x402c540c;
pub const TCSETS2: Ioctl = 0x802c540d;
pub const TCSETSW2: Ioctl = 0x802c540e;
pub const TCSETSF2: Ioctl = 0x802c540f;
pub const TIOCGPTN: Ioctl = 0x40047486;
pub const TIOCSPTLCK: Ioctl = 0x80047487;
pub const TIOCGDEV: Ioctl = 0x40045432;
pub const TIOCSIG: Ioctl = 0x80047488;
pub const TIOCVHANGUP: Ioctl = 0x20005437;
pub const TIOCGPKT: Ioctl = 0x40045438;
pub const TIOCGPTLCK: Ioctl = 0x40045439;
pub const TIOCGEXCL: Ioctl = 0x40045440;
pub const TIOCGPTPEER: Ioctl = 0x20007489;
pub const FIONCLEX: Ioctl = 0x20006602;
pub const FIOCLEX: Ioctl = 0x20006601;
pub const TIOCSERCONFIG: Ioctl = 0x5453;
pub const TIOCSERGWILD: Ioctl = 0x5454;
pub const TIOCSERSWILD: Ioctl = 0x5455;
pub const TIOCGLCKTRMIOS: Ioctl = 0x5456;
pub const TIOCSLCKTRMIOS: Ioctl = 0x5457;
pub const TIOCSERGSTRUCT: Ioctl = 0x5458;
pub const TIOCSERGETLSR: Ioctl = 0x5459;
pub const TIOCSERGETMULTI: Ioctl = 0x545A;
pub const TIOCSERSETMULTI: Ioctl = 0x545B;
pub const TIOCMIWAIT: Ioctl = 0x545C;
pub const TIOCGICOUNT: Ioctl = 0x545D;
pub const TIOCSTART: Ioctl = 0x2000746e;
pub const TIOCSTOP: Ioctl = 0x2000746f;
pub const BLKIOMIN: Ioctl = 0x20001278;
pub const BLKIOOPT: Ioctl = 0x20001279;
pub const BLKSSZGET: Ioctl = 0x20001268;
pub const BLKPBSZGET: Ioctl = 0x2000127B;

//pub const FIOASYNC: Ioctl = 0x4004667d;
//pub const FIOQSIZE: Ioctl = ;
//pub const TIOCGISO7816: Ioctl = 0x40285443;
//pub const TIOCSISO7816: Ioctl = 0xc0285444;
//pub const TIOCGRS485: Ioctl = 0x40205441;
//pub const TIOCSRS485: Ioctl = 0xc0205442;

pub const TIOCM_LE: c_int = 0x001;
pub const TIOCM_DTR: c_int = 0x002;
pub const TIOCM_RTS: c_int = 0x004;
pub const TIOCM_ST: c_int = 0x008;
pub const TIOCM_SR: c_int = 0x010;
pub const TIOCM_CTS: c_int = 0x020;
pub const TIOCM_CAR: c_int = 0x040;
pub const TIOCM_CD: c_int = TIOCM_CAR;
pub const TIOCM_RNG: c_int = 0x080;
pub const TIOCM_RI: c_int = TIOCM_RNG;
pub const TIOCM_DSR: c_int = 0x100;

pub const BOTHER: crate::speed_t = 0x1000;
pub const IBSHIFT: crate::tcflag_t = 16;

// RLIMIT Constants

pub const RLIMIT_CPU: crate::__rlimit_resource_t = 0;
pub const RLIMIT_FSIZE: crate::__rlimit_resource_t = 1;
pub const RLIMIT_DATA: crate::__rlimit_resource_t = 2;
pub const RLIMIT_STACK: crate::__rlimit_resource_t = 3;
pub const RLIMIT_CORE: crate::__rlimit_resource_t = 4;
pub const RLIMIT_RSS: crate::__rlimit_resource_t = 5;
pub const RLIMIT_NOFILE: crate::__rlimit_resource_t = 6;
pub const RLIMIT_NPROC: crate::__rlimit_resource_t = 7;
pub const RLIMIT_MEMLOCK: crate::__rlimit_resource_t = 8;
pub const RLIMIT_AS: crate::__rlimit_resource_t = 9;
pub const RLIMIT_LOCKS: crate::__rlimit_resource_t = 10;
pub const RLIMIT_SIGPENDING: crate::__rlimit_resource_t = 11;
pub const RLIMIT_MSGQUEUE: crate::__rlimit_resource_t = 12;
pub const RLIMIT_NICE: crate::__rlimit_resource_t = 13;
pub const RLIMIT_RTPRIO: crate::__rlimit_resource_t = 14;
pub const RLIMIT_RTTIME: crate::__rlimit_resource_t = 15;
#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIM_NLIMITS: crate::__rlimit_resource_t = 16;
#[allow(deprecated)]
#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIMIT_NLIMITS: crate::__rlimit_resource_t = RLIM_NLIMITS;

cfg_if! {
    if #[cfg(target_arch = "sparc64")] {
        pub const RLIM_INFINITY: crate::rlim_t = !0;
    } else if #[cfg(target_arch = "sparc")] {
        pub const RLIM_INFINITY: crate::rlim_t = 0x7fffffff;
    }
}
