use crate::off_t;
use crate::prelude::*;

pub type wchar_t = c_int;

s! {
    pub struct stat {
        pub st_dev: crate::dev_t,
        __st_padding1: [c_long; 2],
        pub st_ino: crate::ino_t,
        pub st_mode: crate::mode_t,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        __st_padding2: [c_long; 2],
        pub st_size: off_t,
        pub st_atime: crate::time_t,
        pub st_atime_nsec: c_long,
        pub st_mtime: crate::time_t,
        pub st_mtime_nsec: c_long,
        pub st_ctime: crate::time_t,
        pub st_ctime_nsec: c_long,
        pub st_blksize: crate::blksize_t,
        __st_padding3: c_long,
        pub st_blocks: crate::blkcnt_t,
        __st_padding4: [c_long; 14],
    }

    pub struct stat64 {
        pub st_dev: crate::dev_t,
        __st_padding1: [c_long; 2],
        pub st_ino: crate::ino64_t,
        pub st_mode: crate::mode_t,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        __st_padding2: [c_long; 2],
        pub st_size: off_t,
        pub st_atime: crate::time_t,
        pub st_atime_nsec: c_long,
        pub st_mtime: crate::time_t,
        pub st_mtime_nsec: c_long,
        pub st_ctime: crate::time_t,
        pub st_ctime_nsec: c_long,
        pub st_blksize: crate::blksize_t,
        __st_padding3: c_long,
        pub st_blocks: crate::blkcnt64_t,
        __st_padding4: [c_long; 14],
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_size: size_t,
        pub ss_flags: c_int,
    }

    pub struct ipc_perm {
        #[cfg(musl_v1_2_3)]
        pub __key: crate::key_t,
        #[cfg(not(musl_v1_2_3))]
        #[deprecated(
            since = "0.2.173",
            note = "This field is incorrectly named and will be changed
                to __key in a future release."
        )]
        pub __ipc_perm_key: crate::key_t,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub cuid: crate::uid_t,
        pub cgid: crate::gid_t,
        pub mode: crate::mode_t,
        pub __seq: c_int,
        __unused1: c_long,
        __unused2: c_long,
    }

    pub struct shmid_ds {
        pub shm_perm: crate::ipc_perm,
        pub shm_segsz: size_t,
        pub shm_atime: crate::time_t,
        pub shm_dtime: crate::time_t,
        pub shm_ctime: crate::time_t,
        pub shm_cpid: crate::pid_t,
        pub shm_lpid: crate::pid_t,
        pub shm_nattch: c_ulong,
        __pad1: c_ulong,
        __pad2: c_ulong,
    }

    pub struct msqid_ds {
        pub msg_perm: crate::ipc_perm,
        #[cfg(target_endian = "big")]
        __unused1: c_int,
        pub msg_stime: crate::time_t,
        #[cfg(target_endian = "little")]
        __unused1: c_int,
        #[cfg(target_endian = "big")]
        __unused2: c_int,
        pub msg_rtime: crate::time_t,
        #[cfg(target_endian = "little")]
        __unused2: c_int,
        #[cfg(target_endian = "big")]
        __unused3: c_int,
        pub msg_ctime: crate::time_t,
        #[cfg(target_endian = "little")]
        __unused3: c_int,
        pub __msg_cbytes: c_ulong,
        pub msg_qnum: crate::msgqnum_t,
        pub msg_qbytes: crate::msglen_t,
        pub msg_lspid: crate::pid_t,
        pub msg_lrpid: crate::pid_t,
        __pad1: c_ulong,
        __pad2: c_ulong,
    }

    pub struct statfs {
        pub f_type: c_ulong,
        pub f_bsize: c_ulong,
        pub f_frsize: c_ulong,
        pub f_blocks: crate::fsblkcnt_t,
        pub f_bfree: crate::fsblkcnt_t,
        pub f_files: crate::fsfilcnt_t,
        pub f_ffree: crate::fsfilcnt_t,
        pub f_bavail: crate::fsblkcnt_t,
        pub f_fsid: crate::fsid_t,
        pub f_namelen: c_ulong,
        pub f_flags: c_ulong,
        pub f_spare: [c_ulong; 5],
    }

    pub struct statfs64 {
        pub f_type: c_ulong,
        pub f_bsize: c_ulong,
        pub f_frsize: c_ulong,
        pub f_blocks: crate::fsblkcnt_t,
        pub f_bfree: crate::fsblkcnt_t,
        pub f_files: crate::fsfilcnt_t,
        pub f_ffree: crate::fsfilcnt_t,
        pub f_bavail: crate::fsblkcnt_t,
        pub f_fsid: crate::fsid_t,
        pub f_namelen: c_ulong,
        pub f_flags: c_ulong,
        pub f_spare: [c_ulong; 5],
    }
}

s_no_extra_traits! {
    #[repr(align(8))]
    pub struct max_align_t {
        priv_: [f32; 4],
    }
}

pub const SIGSTKSZ: size_t = 8192;
pub const MINSIGSTKSZ: size_t = 2048;

pub const O_DIRECT: c_int = 0o100000;
pub const O_DIRECTORY: c_int = 0o200000;
pub const O_NOFOLLOW: c_int = 0o400000;
pub const O_ASYNC: c_int = 0o10000;
pub const O_LARGEFILE: c_int = 0x2000;

pub const MCL_CURRENT: c_int = 0x0001;
pub const MCL_FUTURE: c_int = 0x0002;
pub const MCL_ONFAULT: c_int = 0x0004;
pub const CBAUD: crate::tcflag_t = 0o0010017;
pub const TAB1: c_int = 0x00000800;
pub const TAB2: c_int = 0x00001000;
pub const TAB3: c_int = 0x00001800;
pub const CR1: c_int = 0x00000200;
pub const CR2: c_int = 0x00000400;
pub const CR3: c_int = 0x00000600;
pub const FF1: c_int = 0x00008000;
pub const BS1: c_int = 0x00002000;
pub const VT1: c_int = 0x00004000;
pub const VWERASE: usize = 14;
pub const VREPRINT: usize = 12;
pub const VSUSP: usize = 10;
pub const VSTART: usize = 8;
pub const VSTOP: usize = 9;
pub const VDISCARD: usize = 13;
pub const VTIME: usize = 5;
pub const IXON: crate::tcflag_t = 0x00000400;
pub const IXOFF: crate::tcflag_t = 0x00001000;
pub const ONLCR: crate::tcflag_t = 0x4;
pub const CSIZE: crate::tcflag_t = 0x00000030;
pub const CS6: crate::tcflag_t = 0x00000010;
pub const CS7: crate::tcflag_t = 0x00000020;
pub const CS8: crate::tcflag_t = 0x00000030;
pub const CSTOPB: crate::tcflag_t = 0x00000040;
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
pub const CIBAUD: crate::tcflag_t = 0o02003600000;
pub const CBAUDEX: crate::tcflag_t = 0o010000;
pub const VSWTC: usize = 7;
pub const OLCUC: crate::tcflag_t = 0o000002;
pub const NLDLY: crate::tcflag_t = 0o000400;
pub const CRDLY: crate::tcflag_t = 0o003000;
pub const TABDLY: crate::tcflag_t = 0o014000;
pub const BSDLY: crate::tcflag_t = 0o020000;
pub const FFDLY: crate::tcflag_t = 0o100000;
pub const VTDLY: crate::tcflag_t = 0o040000;
pub const XTABS: crate::tcflag_t = 0o014000;
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

pub const O_APPEND: c_int = 0o010;
pub const O_CREAT: c_int = 0o400;
pub const O_EXCL: c_int = 0o2000;
pub const O_NOCTTY: c_int = 0o4000;
pub const O_NONBLOCK: c_int = 0o200;
pub const O_SYNC: c_int = 0o40020;
pub const O_RSYNC: c_int = 0o40020;
pub const O_DSYNC: c_int = 0o020;

pub const MAP_ANON: c_int = 0x800;
pub const MAP_GROWSDOWN: c_int = 0x1000;
pub const MAP_DENYWRITE: c_int = 0x2000;
pub const MAP_EXECUTABLE: c_int = 0x4000;
pub const MAP_LOCKED: c_int = 0x8000;
pub const MAP_NORESERVE: c_int = 0x0400;
pub const MAP_POPULATE: c_int = 0x10000;
pub const MAP_NONBLOCK: c_int = 0x20000;
pub const MAP_STACK: c_int = 0x40000;
pub const MAP_HUGETLB: c_int = 0x80000;

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
pub const ENOANO: c_int = 53;
pub const EBADRQC: c_int = 54;
pub const EBADSLT: c_int = 55;
pub const EDEADLOCK: c_int = 56;
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
pub const ENOTSUP: c_int = EOPNOTSUPP;
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
pub const EHWPOISON: c_int = 168;
pub const ERFKILL: c_int = 167;

pub const SOCK_STREAM: c_int = 2;
pub const SOCK_DGRAM: c_int = 1;

pub const SA_ONSTACK: c_int = 0x08000000;
pub const SA_SIGINFO: c_int = 8;
pub const SA_NOCLDWAIT: c_int = 0x10000;

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
pub const SIGSTKFLT: c_int = 7;
pub const SIGPOLL: c_int = crate::SIGIO;
pub const SIGPWR: c_int = 19;
pub const SIG_SETMASK: c_int = 3;
pub const SIG_BLOCK: c_int = 1;
pub const SIG_UNBLOCK: c_int = 2;

pub const EXTPROC: crate::tcflag_t = 0o200000;

pub const F_GETLK: c_int = 33;
pub const F_GETOWN: c_int = 23;
pub const F_SETLK: c_int = 34;
pub const F_SETLKW: c_int = 35;
pub const F_SETOWN: c_int = 24;

pub const VEOF: usize = 16;
pub const VEOL: usize = 17;
pub const VEOL2: usize = 6;
pub const VMIN: usize = 4;
pub const IEXTEN: crate::tcflag_t = 0o000400;
pub const TOSTOP: crate::tcflag_t = 0o100000;
pub const FLUSHO: crate::tcflag_t = 0o020000;

pub const POLLWRNORM: c_short = 0x4;
pub const POLLWRBAND: c_short = 0x100;

pub const SYS_syscall: c_long = 4000 + 0;
pub const SYS_exit: c_long = 4000 + 1;
pub const SYS_fork: c_long = 4000 + 2;
pub const SYS_read: c_long = 4000 + 3;
pub const SYS_write: c_long = 4000 + 4;
pub const SYS_open: c_long = 4000 + 5;
pub const SYS_close: c_long = 4000 + 6;
pub const SYS_waitpid: c_long = 4000 + 7;
pub const SYS_creat: c_long = 4000 + 8;
pub const SYS_link: c_long = 4000 + 9;
pub const SYS_unlink: c_long = 4000 + 10;
pub const SYS_execve: c_long = 4000 + 11;
pub const SYS_chdir: c_long = 4000 + 12;
pub const SYS_time: c_long = 4000 + 13;
pub const SYS_mknod: c_long = 4000 + 14;
pub const SYS_chmod: c_long = 4000 + 15;
pub const SYS_lchown: c_long = 4000 + 16;
pub const SYS_break: c_long = 4000 + 17;
pub const SYS_lseek: c_long = 4000 + 19;
pub const SYS_getpid: c_long = 4000 + 20;
pub const SYS_mount: c_long = 4000 + 21;
pub const SYS_umount: c_long = 4000 + 22;
pub const SYS_setuid: c_long = 4000 + 23;
pub const SYS_getuid: c_long = 4000 + 24;
pub const SYS_stime: c_long = 4000 + 25;
pub const SYS_ptrace: c_long = 4000 + 26;
pub const SYS_alarm: c_long = 4000 + 27;
pub const SYS_pause: c_long = 4000 + 29;
pub const SYS_utime: c_long = 4000 + 30;
pub const SYS_stty: c_long = 4000 + 31;
pub const SYS_gtty: c_long = 4000 + 32;
pub const SYS_access: c_long = 4000 + 33;
pub const SYS_nice: c_long = 4000 + 34;
pub const SYS_ftime: c_long = 4000 + 35;
pub const SYS_sync: c_long = 4000 + 36;
pub const SYS_kill: c_long = 4000 + 37;
pub const SYS_rename: c_long = 4000 + 38;
pub const SYS_mkdir: c_long = 4000 + 39;
pub const SYS_rmdir: c_long = 4000 + 40;
pub const SYS_dup: c_long = 4000 + 41;
pub const SYS_pipe: c_long = 4000 + 42;
pub const SYS_times: c_long = 4000 + 43;
pub const SYS_prof: c_long = 4000 + 44;
pub const SYS_brk: c_long = 4000 + 45;
pub const SYS_setgid: c_long = 4000 + 46;
pub const SYS_getgid: c_long = 4000 + 47;
pub const SYS_signal: c_long = 4000 + 48;
pub const SYS_geteuid: c_long = 4000 + 49;
pub const SYS_getegid: c_long = 4000 + 50;
pub const SYS_acct: c_long = 4000 + 51;
pub const SYS_umount2: c_long = 4000 + 52;
pub const SYS_lock: c_long = 4000 + 53;
pub const SYS_ioctl: c_long = 4000 + 54;
pub const SYS_fcntl: c_long = 4000 + 55;
pub const SYS_mpx: c_long = 4000 + 56;
pub const SYS_setpgid: c_long = 4000 + 57;
pub const SYS_ulimit: c_long = 4000 + 58;
pub const SYS_umask: c_long = 4000 + 60;
pub const SYS_chroot: c_long = 4000 + 61;
pub const SYS_ustat: c_long = 4000 + 62;
pub const SYS_dup2: c_long = 4000 + 63;
pub const SYS_getppid: c_long = 4000 + 64;
pub const SYS_getpgrp: c_long = 4000 + 65;
pub const SYS_setsid: c_long = 4000 + 66;
pub const SYS_sigaction: c_long = 4000 + 67;
pub const SYS_sgetmask: c_long = 4000 + 68;
pub const SYS_ssetmask: c_long = 4000 + 69;
pub const SYS_setreuid: c_long = 4000 + 70;
pub const SYS_setregid: c_long = 4000 + 71;
pub const SYS_sigsuspend: c_long = 4000 + 72;
pub const SYS_sigpending: c_long = 4000 + 73;
pub const SYS_sethostname: c_long = 4000 + 74;
pub const SYS_setrlimit: c_long = 4000 + 75;
pub const SYS_getrlimit: c_long = 4000 + 76;
pub const SYS_getrusage: c_long = 4000 + 77;
pub const SYS_gettimeofday: c_long = 4000 + 78;
pub const SYS_settimeofday: c_long = 4000 + 79;
pub const SYS_getgroups: c_long = 4000 + 80;
pub const SYS_setgroups: c_long = 4000 + 81;
pub const SYS_symlink: c_long = 4000 + 83;
pub const SYS_readlink: c_long = 4000 + 85;
pub const SYS_uselib: c_long = 4000 + 86;
pub const SYS_swapon: c_long = 4000 + 87;
pub const SYS_reboot: c_long = 4000 + 88;
pub const SYS_readdir: c_long = 4000 + 89;
pub const SYS_mmap: c_long = 4000 + 90;
pub const SYS_munmap: c_long = 4000 + 91;
pub const SYS_truncate: c_long = 4000 + 92;
pub const SYS_ftruncate: c_long = 4000 + 93;
pub const SYS_fchmod: c_long = 4000 + 94;
pub const SYS_fchown: c_long = 4000 + 95;
pub const SYS_getpriority: c_long = 4000 + 96;
pub const SYS_setpriority: c_long = 4000 + 97;
pub const SYS_profil: c_long = 4000 + 98;
pub const SYS_statfs: c_long = 4000 + 99;
pub const SYS_fstatfs: c_long = 4000 + 100;
pub const SYS_ioperm: c_long = 4000 + 101;
pub const SYS_socketcall: c_long = 4000 + 102;
pub const SYS_syslog: c_long = 4000 + 103;
pub const SYS_setitimer: c_long = 4000 + 104;
pub const SYS_getitimer: c_long = 4000 + 105;
pub const SYS_stat: c_long = 4000 + 106;
pub const SYS_lstat: c_long = 4000 + 107;
pub const SYS_fstat: c_long = 4000 + 108;
pub const SYS_iopl: c_long = 4000 + 110;
pub const SYS_vhangup: c_long = 4000 + 111;
pub const SYS_idle: c_long = 4000 + 112;
pub const SYS_vm86: c_long = 4000 + 113;
pub const SYS_wait4: c_long = 4000 + 114;
pub const SYS_swapoff: c_long = 4000 + 115;
pub const SYS_sysinfo: c_long = 4000 + 116;
pub const SYS_ipc: c_long = 4000 + 117;
pub const SYS_fsync: c_long = 4000 + 118;
pub const SYS_sigreturn: c_long = 4000 + 119;
pub const SYS_clone: c_long = 4000 + 120;
pub const SYS_setdomainname: c_long = 4000 + 121;
pub const SYS_uname: c_long = 4000 + 122;
pub const SYS_modify_ldt: c_long = 4000 + 123;
pub const SYS_adjtimex: c_long = 4000 + 124;
pub const SYS_mprotect: c_long = 4000 + 125;
pub const SYS_sigprocmask: c_long = 4000 + 126;
#[deprecated(since = "0.2.70", note = "Functional up to 2.6 kernel")]
pub const SYS_create_module: c_long = 4000 + 127;
pub const SYS_init_module: c_long = 4000 + 128;
pub const SYS_delete_module: c_long = 4000 + 129;
#[deprecated(since = "0.2.70", note = "Functional up to 2.6 kernel")]
pub const SYS_get_kernel_syms: c_long = 4000 + 130;
pub const SYS_quotactl: c_long = 4000 + 131;
pub const SYS_getpgid: c_long = 4000 + 132;
pub const SYS_fchdir: c_long = 4000 + 133;
pub const SYS_bdflush: c_long = 4000 + 134;
pub const SYS_sysfs: c_long = 4000 + 135;
pub const SYS_personality: c_long = 4000 + 136;
pub const SYS_afs_syscall: c_long = 4000 + 137;
pub const SYS_setfsuid: c_long = 4000 + 138;
pub const SYS_setfsgid: c_long = 4000 + 139;
pub const SYS__llseek: c_long = 4000 + 140;
pub const SYS_getdents: c_long = 4000 + 141;
pub const SYS_flock: c_long = 4000 + 143;
pub const SYS_msync: c_long = 4000 + 144;
pub const SYS_readv: c_long = 4000 + 145;
pub const SYS_writev: c_long = 4000 + 146;
pub const SYS_cacheflush: c_long = 4000 + 147;
pub const SYS_cachectl: c_long = 4000 + 148;
pub const SYS_sysmips: c_long = 4000 + 149;
pub const SYS_getsid: c_long = 4000 + 151;
pub const SYS_fdatasync: c_long = 4000 + 152;
pub const SYS__sysctl: c_long = 4000 + 153;
pub const SYS_mlock: c_long = 4000 + 154;
pub const SYS_munlock: c_long = 4000 + 155;
pub const SYS_mlockall: c_long = 4000 + 156;
pub const SYS_munlockall: c_long = 4000 + 157;
pub const SYS_sched_setparam: c_long = 4000 + 158;
pub const SYS_sched_getparam: c_long = 4000 + 159;
pub const SYS_sched_setscheduler: c_long = 4000 + 160;
pub const SYS_sched_getscheduler: c_long = 4000 + 161;
pub const SYS_sched_yield: c_long = 4000 + 162;
pub const SYS_sched_get_priority_max: c_long = 4000 + 163;
pub const SYS_sched_get_priority_min: c_long = 4000 + 164;
pub const SYS_sched_rr_get_interval: c_long = 4000 + 165;
pub const SYS_nanosleep: c_long = 4000 + 166;
pub const SYS_mremap: c_long = 4000 + 167;
pub const SYS_accept: c_long = 4000 + 168;
pub const SYS_bind: c_long = 4000 + 169;
pub const SYS_connect: c_long = 4000 + 170;
pub const SYS_getpeername: c_long = 4000 + 171;
pub const SYS_getsockname: c_long = 4000 + 172;
pub const SYS_getsockopt: c_long = 4000 + 173;
pub const SYS_listen: c_long = 4000 + 174;
pub const SYS_recv: c_long = 4000 + 175;
pub const SYS_recvfrom: c_long = 4000 + 176;
pub const SYS_recvmsg: c_long = 4000 + 177;
pub const SYS_send: c_long = 4000 + 178;
pub const SYS_sendmsg: c_long = 4000 + 179;
pub const SYS_sendto: c_long = 4000 + 180;
pub const SYS_setsockopt: c_long = 4000 + 181;
pub const SYS_shutdown: c_long = 4000 + 182;
pub const SYS_socket: c_long = 4000 + 183;
pub const SYS_socketpair: c_long = 4000 + 184;
pub const SYS_setresuid: c_long = 4000 + 185;
pub const SYS_getresuid: c_long = 4000 + 186;
#[deprecated(since = "0.2.70", note = "Functional up to 2.6 kernel")]
pub const SYS_query_module: c_long = 4000 + 187;
pub const SYS_poll: c_long = 4000 + 188;
pub const SYS_nfsservctl: c_long = 4000 + 189;
pub const SYS_setresgid: c_long = 4000 + 190;
pub const SYS_getresgid: c_long = 4000 + 191;
pub const SYS_prctl: c_long = 4000 + 192;
pub const SYS_rt_sigreturn: c_long = 4000 + 193;
pub const SYS_rt_sigaction: c_long = 4000 + 194;
pub const SYS_rt_sigprocmask: c_long = 4000 + 195;
pub const SYS_rt_sigpending: c_long = 4000 + 196;
pub const SYS_rt_sigtimedwait: c_long = 4000 + 197;
pub const SYS_rt_sigqueueinfo: c_long = 4000 + 198;
pub const SYS_rt_sigsuspend: c_long = 4000 + 199;
pub const SYS_chown: c_long = 4000 + 202;
pub const SYS_getcwd: c_long = 4000 + 203;
pub const SYS_capget: c_long = 4000 + 204;
pub const SYS_capset: c_long = 4000 + 205;
pub const SYS_sigaltstack: c_long = 4000 + 206;
pub const SYS_sendfile: c_long = 4000 + 207;
pub const SYS_getpmsg: c_long = 4000 + 208;
pub const SYS_putpmsg: c_long = 4000 + 209;
pub const SYS_mmap2: c_long = 4000 + 210;
pub const SYS_truncate64: c_long = 4000 + 211;
pub const SYS_ftruncate64: c_long = 4000 + 212;
pub const SYS_stat64: c_long = 4000 + 213;
pub const SYS_lstat64: c_long = 4000 + 214;
pub const SYS_fstat64: c_long = 4000 + 215;
pub const SYS_pivot_root: c_long = 4000 + 216;
pub const SYS_mincore: c_long = 4000 + 217;
pub const SYS_madvise: c_long = 4000 + 218;
pub const SYS_getdents64: c_long = 4000 + 219;
pub const SYS_fcntl64: c_long = 4000 + 220;
pub const SYS_gettid: c_long = 4000 + 222;
pub const SYS_readahead: c_long = 4000 + 223;
pub const SYS_setxattr: c_long = 4000 + 224;
pub const SYS_lsetxattr: c_long = 4000 + 225;
pub const SYS_fsetxattr: c_long = 4000 + 226;
pub const SYS_getxattr: c_long = 4000 + 227;
pub const SYS_lgetxattr: c_long = 4000 + 228;
pub const SYS_fgetxattr: c_long = 4000 + 229;
pub const SYS_listxattr: c_long = 4000 + 230;
pub const SYS_llistxattr: c_long = 4000 + 231;
pub const SYS_flistxattr: c_long = 4000 + 232;
pub const SYS_removexattr: c_long = 4000 + 233;
pub const SYS_lremovexattr: c_long = 4000 + 234;
pub const SYS_fremovexattr: c_long = 4000 + 235;
pub const SYS_tkill: c_long = 4000 + 236;
pub const SYS_sendfile64: c_long = 4000 + 237;
pub const SYS_futex: c_long = 4000 + 238;
pub const SYS_sched_setaffinity: c_long = 4000 + 239;
pub const SYS_sched_getaffinity: c_long = 4000 + 240;
pub const SYS_io_setup: c_long = 4000 + 241;
pub const SYS_io_destroy: c_long = 4000 + 242;
pub const SYS_io_getevents: c_long = 4000 + 243;
pub const SYS_io_submit: c_long = 4000 + 244;
pub const SYS_io_cancel: c_long = 4000 + 245;
pub const SYS_exit_group: c_long = 4000 + 246;
pub const SYS_lookup_dcookie: c_long = 4000 + 247;
pub const SYS_epoll_create: c_long = 4000 + 248;
pub const SYS_epoll_ctl: c_long = 4000 + 249;
pub const SYS_epoll_wait: c_long = 4000 + 250;
pub const SYS_remap_file_pages: c_long = 4000 + 251;
pub const SYS_set_tid_address: c_long = 4000 + 252;
pub const SYS_restart_syscall: c_long = 4000 + 253;
pub const SYS_statfs64: c_long = 4000 + 255;
pub const SYS_fstatfs64: c_long = 4000 + 256;
pub const SYS_timer_create: c_long = 4000 + 257;
pub const SYS_timer_settime: c_long = 4000 + 258;
pub const SYS_timer_gettime: c_long = 4000 + 259;
pub const SYS_timer_getoverrun: c_long = 4000 + 260;
pub const SYS_timer_delete: c_long = 4000 + 261;
pub const SYS_clock_settime: c_long = 4000 + 262;
pub const SYS_clock_gettime: c_long = 4000 + 263;
pub const SYS_clock_getres: c_long = 4000 + 264;
pub const SYS_clock_nanosleep: c_long = 4000 + 265;
pub const SYS_tgkill: c_long = 4000 + 266;
pub const SYS_utimes: c_long = 4000 + 267;
pub const SYS_mbind: c_long = 4000 + 268;
pub const SYS_get_mempolicy: c_long = 4000 + 269;
pub const SYS_set_mempolicy: c_long = 4000 + 270;
pub const SYS_mq_open: c_long = 4000 + 271;
pub const SYS_mq_unlink: c_long = 4000 + 272;
pub const SYS_mq_timedsend: c_long = 4000 + 273;
pub const SYS_mq_timedreceive: c_long = 4000 + 274;
pub const SYS_mq_notify: c_long = 4000 + 275;
pub const SYS_mq_getsetattr: c_long = 4000 + 276;
pub const SYS_vserver: c_long = 4000 + 277;
pub const SYS_waitid: c_long = 4000 + 278;
/* pub const SYS_sys_setaltroot: c_long = 4000 + 279; */
pub const SYS_add_key: c_long = 4000 + 280;
pub const SYS_request_key: c_long = 4000 + 281;
pub const SYS_keyctl: c_long = 4000 + 282;
pub const SYS_set_thread_area: c_long = 4000 + 283;
pub const SYS_inotify_init: c_long = 4000 + 284;
pub const SYS_inotify_add_watch: c_long = 4000 + 285;
pub const SYS_inotify_rm_watch: c_long = 4000 + 286;
pub const SYS_migrate_pages: c_long = 4000 + 287;
pub const SYS_openat: c_long = 4000 + 288;
pub const SYS_mkdirat: c_long = 4000 + 289;
pub const SYS_mknodat: c_long = 4000 + 290;
pub const SYS_fchownat: c_long = 4000 + 291;
pub const SYS_futimesat: c_long = 4000 + 292;
pub const SYS_unlinkat: c_long = 4000 + 294;
pub const SYS_renameat: c_long = 4000 + 295;
pub const SYS_linkat: c_long = 4000 + 296;
pub const SYS_symlinkat: c_long = 4000 + 297;
pub const SYS_readlinkat: c_long = 4000 + 298;
pub const SYS_fchmodat: c_long = 4000 + 299;
pub const SYS_faccessat: c_long = 4000 + 300;
pub const SYS_pselect6: c_long = 4000 + 301;
pub const SYS_ppoll: c_long = 4000 + 302;
pub const SYS_unshare: c_long = 4000 + 303;
pub const SYS_splice: c_long = 4000 + 304;
pub const SYS_sync_file_range: c_long = 4000 + 305;
pub const SYS_tee: c_long = 4000 + 306;
pub const SYS_vmsplice: c_long = 4000 + 307;
pub const SYS_move_pages: c_long = 4000 + 308;
pub const SYS_set_robust_list: c_long = 4000 + 309;
pub const SYS_get_robust_list: c_long = 4000 + 310;
pub const SYS_kexec_load: c_long = 4000 + 311;
pub const SYS_getcpu: c_long = 4000 + 312;
pub const SYS_epoll_pwait: c_long = 4000 + 313;
pub const SYS_ioprio_set: c_long = 4000 + 314;
pub const SYS_ioprio_get: c_long = 4000 + 315;
pub const SYS_utimensat: c_long = 4000 + 316;
pub const SYS_signalfd: c_long = 4000 + 317;
pub const SYS_timerfd: c_long = 4000 + 318;
pub const SYS_eventfd: c_long = 4000 + 319;
pub const SYS_fallocate: c_long = 4000 + 320;
pub const SYS_timerfd_create: c_long = 4000 + 321;
pub const SYS_timerfd_gettime: c_long = 4000 + 322;
pub const SYS_timerfd_settime: c_long = 4000 + 323;
pub const SYS_signalfd4: c_long = 4000 + 324;
pub const SYS_eventfd2: c_long = 4000 + 325;
pub const SYS_epoll_create1: c_long = 4000 + 326;
pub const SYS_dup3: c_long = 4000 + 327;
pub const SYS_pipe2: c_long = 4000 + 328;
pub const SYS_inotify_init1: c_long = 4000 + 329;
pub const SYS_preadv: c_long = 4000 + 330;
pub const SYS_pwritev: c_long = 4000 + 331;
pub const SYS_rt_tgsigqueueinfo: c_long = 4000 + 332;
pub const SYS_perf_event_open: c_long = 4000 + 333;
pub const SYS_accept4: c_long = 4000 + 334;
pub const SYS_recvmmsg: c_long = 4000 + 335;
pub const SYS_fanotify_init: c_long = 4000 + 336;
pub const SYS_fanotify_mark: c_long = 4000 + 337;
pub const SYS_prlimit64: c_long = 4000 + 338;
pub const SYS_name_to_handle_at: c_long = 4000 + 339;
pub const SYS_open_by_handle_at: c_long = 4000 + 340;
pub const SYS_clock_adjtime: c_long = 4000 + 341;
pub const SYS_syncfs: c_long = 4000 + 342;
pub const SYS_sendmmsg: c_long = 4000 + 343;
pub const SYS_setns: c_long = 4000 + 344;
pub const SYS_process_vm_readv: c_long = 4000 + 345;
pub const SYS_process_vm_writev: c_long = 4000 + 346;
pub const SYS_kcmp: c_long = 4000 + 347;
pub const SYS_finit_module: c_long = 4000 + 348;
pub const SYS_sched_setattr: c_long = 4000 + 349;
pub const SYS_sched_getattr: c_long = 4000 + 350;
pub const SYS_renameat2: c_long = 4000 + 351;
pub const SYS_seccomp: c_long = 4000 + 352;
pub const SYS_getrandom: c_long = 4000 + 353;
pub const SYS_memfd_create: c_long = 4000 + 354;
pub const SYS_bpf: c_long = 4000 + 355;
pub const SYS_execveat: c_long = 4000 + 356;
pub const SYS_userfaultfd: c_long = 4000 + 357;
pub const SYS_membarrier: c_long = 4000 + 358;
pub const SYS_mlock2: c_long = 4000 + 359;
pub const SYS_copy_file_range: c_long = 4000 + 360;
pub const SYS_preadv2: c_long = 4000 + 361;
pub const SYS_pwritev2: c_long = 4000 + 362;
pub const SYS_pkey_mprotect: c_long = 4000 + 363;
pub const SYS_pkey_alloc: c_long = 4000 + 364;
pub const SYS_pkey_free: c_long = 4000 + 365;
pub const SYS_statx: c_long = 4000 + 366;
pub const SYS_pidfd_send_signal: c_long = 4000 + 424;
pub const SYS_io_uring_setup: c_long = 4000 + 425;
pub const SYS_io_uring_enter: c_long = 4000 + 426;
pub const SYS_io_uring_register: c_long = 4000 + 427;
pub const SYS_open_tree: c_long = 4000 + 428;
pub const SYS_move_mount: c_long = 4000 + 429;
pub const SYS_fsopen: c_long = 4000 + 430;
pub const SYS_fsconfig: c_long = 4000 + 431;
pub const SYS_fsmount: c_long = 4000 + 432;
pub const SYS_fspick: c_long = 4000 + 433;
pub const SYS_pidfd_open: c_long = 4000 + 434;
pub const SYS_clone3: c_long = 4000 + 435;
pub const SYS_close_range: c_long = 4000 + 436;
pub const SYS_openat2: c_long = 4000 + 437;
pub const SYS_pidfd_getfd: c_long = 4000 + 438;
pub const SYS_faccessat2: c_long = 4000 + 439;
pub const SYS_process_madvise: c_long = 4000 + 440;
pub const SYS_epoll_pwait2: c_long = 4000 + 441;
pub const SYS_mount_setattr: c_long = 4000 + 442;
pub const SYS_quotactl_fd: c_long = 4000 + 443;
pub const SYS_landlock_create_ruleset: c_long = 4000 + 444;
pub const SYS_landlock_add_rule: c_long = 4000 + 445;
pub const SYS_landlock_restrict_self: c_long = 4000 + 446;
pub const SYS_memfd_secret: c_long = 4000 + 447;
pub const SYS_process_mrelease: c_long = 4000 + 448;
pub const SYS_futex_waitv: c_long = 4000 + 449;
pub const SYS_set_mempolicy_home_node: c_long = 4000 + 450;
