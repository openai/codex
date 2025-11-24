use crate::prelude::*;

pub type pthread_t = c_ulong;

pub const SFD_CLOEXEC: c_int = 0x080000;

pub const NCCS: usize = 32;

pub const O_TRUNC: c_int = 512;

pub const O_CLOEXEC: c_int = 0x80000;

pub const EBFONT: c_int = 59;
pub const ENOSTR: c_int = 60;
pub const ENODATA: c_int = 61;
pub const ETIME: c_int = 62;
pub const ENOSR: c_int = 63;
pub const ENONET: c_int = 64;
pub const ENOPKG: c_int = 65;
pub const EREMOTE: c_int = 66;
pub const ENOLINK: c_int = 67;
pub const EADV: c_int = 68;
pub const ESRMNT: c_int = 69;
pub const ECOMM: c_int = 70;
pub const EPROTO: c_int = 71;
pub const EDOTDOT: c_int = 73;

pub const SA_NODEFER: c_uint = 0x40000000;
pub const SA_RESETHAND: c_uint = 0x80000000;
pub const SA_RESTART: c_uint = 0x10000000;
pub const SA_NOCLDSTOP: c_uint = 0x00000001;

pub const EPOLL_CLOEXEC: c_int = 0x80000;

pub const EFD_CLOEXEC: c_int = 0x80000;

pub const TMP_MAX: c_uint = 238328;
pub const _SC_2_C_VERSION: c_int = 96;
pub const O_ACCMODE: c_int = 3;
pub const O_DIRECT: c_int = 0x8000;
pub const O_DIRECTORY: c_int = 0x10000;
pub const O_NOFOLLOW: c_int = 0x20000;
pub const O_NOATIME: c_int = 0x40000;
pub const O_PATH: c_int = 0o010000000;

pub const O_APPEND: c_int = 8;
pub const O_CREAT: c_int = 256;
pub const O_EXCL: c_int = 1024;
pub const O_NOCTTY: c_int = 2048;
pub const O_NONBLOCK: c_int = 128;
pub const O_SYNC: c_int = 0x10;
pub const O_RSYNC: c_int = 0x10;
pub const O_DSYNC: c_int = 0x10;
pub const O_FSYNC: c_int = 0x10;
pub const O_ASYNC: c_int = 0x1000;
pub const O_LARGEFILE: c_int = 0x2000;
pub const O_NDELAY: c_int = 0x80;

pub const SOCK_NONBLOCK: c_int = 128;

pub const EDEADLK: c_int = 45;
pub const ENAMETOOLONG: c_int = 78;
pub const ENOLCK: c_int = 46;
pub const ENOSYS: c_int = 89;
pub const ENOTEMPTY: c_int = 93;
pub const ELOOP: c_int = 90;
pub const ENOMSG: c_int = 35;
pub const EIDRM: c_int = 36;
pub const ECHRNG: c_int = 37;
pub const EL2NSYNC: c_int = 38;
pub const EL3HLT: c_int = 39;
pub const EL3RST: c_int = 40;
pub const ELNRNG: c_int = 41;
pub const EUNATCH: c_int = 42;
pub const ENOCSI: c_int = 43;
pub const EL2HLT: c_int = 44;
pub const EBADE: c_int = 50;
pub const EBADR: c_int = 51;
pub const EXFULL: c_int = 52;
pub const FFDLY: c_int = 0o0100000;
pub const ENOANO: c_int = 53;
pub const EBADRQC: c_int = 54;
pub const EBADSLT: c_int = 55;
pub const EMULTIHOP: c_int = 74;
pub const EOVERFLOW: c_int = 79;
pub const ENOTUNIQ: c_int = 80;
pub const EBADFD: c_int = 81;
pub const EBADMSG: c_int = 77;
pub const EREMCHG: c_int = 82;
pub const ELIBACC: c_int = 83;
pub const ELIBBAD: c_int = 84;
pub const ELIBSCN: c_int = 85;
pub const ELIBMAX: c_int = 86;
pub const ELIBEXEC: c_int = 87;
pub const EILSEQ: c_int = 88;
pub const ERESTART: c_int = 91;
pub const ESTRPIPE: c_int = 92;
pub const EUSERS: c_int = 94;
pub const ENOTSOCK: c_int = 95;
pub const EDESTADDRREQ: c_int = 96;
pub const EMSGSIZE: c_int = 97;
pub const EPROTOTYPE: c_int = 98;
pub const ENOPROTOOPT: c_int = 99;
pub const EPROTONOSUPPORT: c_int = 120;
pub const ESOCKTNOSUPPORT: c_int = 121;
pub const EOPNOTSUPP: c_int = 122;
pub const EPFNOSUPPORT: c_int = 123;
pub const EAFNOSUPPORT: c_int = 124;
pub const EADDRINUSE: c_int = 125;
pub const EADDRNOTAVAIL: c_int = 126;
pub const ENETDOWN: c_int = 127;
pub const ENETUNREACH: c_int = 128;
pub const ENETRESET: c_int = 129;
pub const ECONNABORTED: c_int = 130;
pub const ECONNRESET: c_int = 131;
pub const ENOBUFS: c_int = 132;
pub const EISCONN: c_int = 133;
pub const ENOTCONN: c_int = 134;
pub const ESHUTDOWN: c_int = 143;
pub const ETOOMANYREFS: c_int = 144;
pub const ETIMEDOUT: c_int = 145;
pub const ECONNREFUSED: c_int = 146;
pub const EHOSTDOWN: c_int = 147;
pub const EHOSTUNREACH: c_int = 148;
pub const EALREADY: c_int = 149;
pub const EINPROGRESS: c_int = 150;
pub const ESTALE: c_int = 151;
pub const EUCLEAN: c_int = 135;
pub const ENOTNAM: c_int = 137;
pub const ENAVAIL: c_int = 138;
pub const EISNAM: c_int = 139;
pub const EREMOTEIO: c_int = 140;
pub const EDQUOT: c_int = 1133;
pub const ENOMEDIUM: c_int = 159;
pub const EMEDIUMTYPE: c_int = 160;
pub const ECANCELED: c_int = 158;
pub const ENOKEY: c_int = 161;
pub const EKEYEXPIRED: c_int = 162;
pub const EKEYREVOKED: c_int = 163;
pub const EKEYREJECTED: c_int = 164;
pub const EOWNERDEAD: c_int = 165;
pub const ENOTRECOVERABLE: c_int = 166;
pub const ERFKILL: c_int = 167;

pub const MAP_NORESERVE: c_int = 0x400;
pub const MAP_ANON: c_int = 0x800;
pub const MAP_ANONYMOUS: c_int = 0x800;
pub const MAP_GROWSDOWN: c_int = 0x1000;
pub const MAP_DENYWRITE: c_int = 0x2000;
pub const MAP_EXECUTABLE: c_int = 0x4000;
pub const MAP_LOCKED: c_int = 0x8000;
pub const MAP_POPULATE: c_int = 0x10000;
pub const MAP_NONBLOCK: c_int = 0x20000;
pub const MAP_STACK: c_int = 0x40000;

pub const NLDLY: crate::tcflag_t = 0o0000400;

pub const SOCK_STREAM: c_int = 2;
pub const SOCK_DGRAM: c_int = 1;
pub const SOCK_SEQPACKET: c_int = 5;

pub const SA_ONSTACK: c_uint = 0x08000000;
pub const SA_SIGINFO: c_uint = 0x00000008;
pub const SA_NOCLDWAIT: c_int = 0x00010000;

pub const SIGCHLD: c_int = 18;
pub const SIGBUS: c_int = 10;
pub const SIGTTIN: c_int = 26;
pub const SIGTTOU: c_int = 27;
pub const SIGXCPU: c_int = 30;
pub const SIGXFSZ: c_int = 31;
pub const SIGVTALRM: c_int = 28;
pub const SIGPROF: c_int = 29;
pub const SIGWINCH: c_int = 20;
pub const SIGUSR1: c_int = 16;
pub const SIGUSR2: c_int = 17;
pub const SIGCONT: c_int = 25;
pub const SIGSTOP: c_int = 23;
pub const SIGTSTP: c_int = 24;
pub const SIGURG: c_int = 21;
pub const SIGIO: c_int = 22;
pub const SIGSYS: c_int = 12;
pub const SIGPWR: c_int = 19;
pub const SIG_SETMASK: c_int = 3;
pub const SIG_BLOCK: c_int = 0x1;
pub const SIG_UNBLOCK: c_int = 0x2;

pub const POLLWRNORM: c_short = 0x004;
pub const POLLWRBAND: c_short = 0x100;

pub const PTHREAD_STACK_MIN: size_t = 16384;

pub const VEOF: usize = 16;
pub const VEOL: usize = 17;
pub const VEOL2: usize = 6;
pub const VMIN: usize = 4;
pub const IEXTEN: crate::tcflag_t = 0x00000100;
pub const TOSTOP: crate::tcflag_t = 0x00008000;
pub const FLUSHO: crate::tcflag_t = 0x00002000;
pub const TCSANOW: c_int = 0x540e;
pub const TCSADRAIN: c_int = 0x540f;
pub const TCSAFLUSH: c_int = 0x5410;

pub const CPU_SETSIZE: c_int = 0x400;

pub const EFD_NONBLOCK: c_int = 0x80;

pub const F_GETLK: c_int = 14;
pub const F_SETLK: c_int = 6;
pub const F_SETLKW: c_int = 7;

pub const SFD_NONBLOCK: c_int = 0x80;

pub const RTLD_GLOBAL: c_int = 0x4;

pub const SIGSTKSZ: size_t = 8192;
pub const CBAUD: crate::tcflag_t = 0o0010017;
pub const CBAUDEX: crate::tcflag_t = 0o0010000;
pub const CIBAUD: crate::tcflag_t = 0o002003600000;
pub const TAB1: crate::tcflag_t = 0x00000800;
pub const TAB2: crate::tcflag_t = 0x00001000;
pub const TAB3: crate::tcflag_t = 0x00001800;
pub const TABDLY: crate::tcflag_t = 0o0014000;
pub const CR1: crate::tcflag_t = 0x00000200;
pub const CR2: crate::tcflag_t = 0x00000400;
pub const CR3: crate::tcflag_t = 0x00000600;
pub const FF1: crate::tcflag_t = 0x00008000;
pub const BS1: crate::tcflag_t = 0x00002000;
pub const BSDLY: crate::tcflag_t = 0o0020000;
pub const VT1: crate::tcflag_t = 0x00004000;
pub const VWERASE: usize = 14;
pub const XTABS: crate::tcflag_t = 0o0014000;
pub const VREPRINT: usize = 12;
pub const VSUSP: usize = 10;
pub const VSWTC: usize = 7;
pub const VTDLY: c_int = 0o0040000;
pub const VSTART: usize = 8;
pub const VSTOP: usize = 9;
pub const VDISCARD: usize = 13;
pub const VTIME: usize = 5;
pub const IXON: crate::tcflag_t = 0x00000400;
pub const IXOFF: crate::tcflag_t = 0x00001000;
pub const OLCUC: crate::tcflag_t = 0o0000002;
pub const ONLCR: crate::tcflag_t = 0x4;
pub const CSIZE: crate::tcflag_t = 0x00000030;
pub const CS6: crate::tcflag_t = 0x00000010;
pub const CS7: crate::tcflag_t = 0x00000020;
pub const CS8: crate::tcflag_t = 0x00000030;
pub const CSTOPB: crate::tcflag_t = 0x00000040;
pub const CRDLY: c_int = 0o0003000;
pub const CREAD: crate::tcflag_t = 0x00000080;
pub const PARENB: crate::tcflag_t = 0x00000100;
pub const PARODD: crate::tcflag_t = 0x00000200;
pub const HUPCL: crate::tcflag_t = 0x00000400;
pub const CLOCAL: crate::tcflag_t = 0x00000800;
pub const ECHOKE: crate::tcflag_t = 0x00000800;
pub const ECHOE: crate::tcflag_t = 0x00000010;
pub const ECHOK: crate::tcflag_t = 0x00000020;
pub const ECHONL: crate::tcflag_t = 0x00000040;
pub const ECHOPRT: crate::tcflag_t = 0x00000400;
pub const ECHOCTL: crate::tcflag_t = 0x00000200;
pub const ISIG: crate::tcflag_t = 0x00000001;
pub const ICANON: crate::tcflag_t = 0x00000002;
pub const PENDIN: crate::tcflag_t = 0x00004000;
pub const NOFLSH: crate::tcflag_t = 0x00000080;

pub const MAP_HUGETLB: c_int = 0x80000;

pub const B0: crate::speed_t = 0o000000;
pub const B50: crate::speed_t = 0o000001;
pub const B75: crate::speed_t = 0o000002;
pub const B110: crate::speed_t = 0o000003;
pub const B134: crate::speed_t = 0o000004;
pub const B150: crate::speed_t = 0o000005;
pub const B200: crate::speed_t = 0o000006;
pub const B300: crate::speed_t = 0o000007;
pub const B600: crate::speed_t = 0o000010;
pub const B1200: crate::speed_t = 0o000011;
pub const B1800: crate::speed_t = 0o000012;
pub const B2400: crate::speed_t = 0o000013;
pub const B4800: crate::speed_t = 0o000014;
pub const B9600: crate::speed_t = 0o000015;
pub const B19200: crate::speed_t = 0o000016;
pub const B38400: crate::speed_t = 0o000017;
pub const B57600: crate::speed_t = 0o010001;
pub const B115200: crate::speed_t = 0o010002;
pub const B230400: crate::speed_t = 0o010003;
pub const B460800: crate::speed_t = 0o010004;
pub const B500000: crate::speed_t = 0o010005;
pub const B576000: crate::speed_t = 0o010006;
pub const B921600: crate::speed_t = 0o010007;
pub const B1000000: crate::speed_t = 0o010010;
pub const B1152000: crate::speed_t = 0o010011;
pub const B1500000: crate::speed_t = 0o010012;
pub const B2000000: crate::speed_t = 0o010013;
pub const B2500000: crate::speed_t = 0o010014;
pub const B3000000: crate::speed_t = 0o010015;
pub const B3500000: crate::speed_t = 0o010016;
pub const B4000000: crate::speed_t = 0o010017;

cfg_if! {
    if #[cfg(target_arch = "mips")] {
        mod mips32;
        pub use self::mips32::*;
    } else if #[cfg(target_arch = "mips64")] {
        mod mips64;
        pub use self::mips64::*;
    } else {
        // Unknown target_arch
    }
}
