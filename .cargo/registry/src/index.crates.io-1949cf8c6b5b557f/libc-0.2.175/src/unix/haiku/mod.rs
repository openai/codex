use crate::prelude::*;

// This module contains bindings to the native Haiku API. The Haiku API
// originates from BeOS, and it was the original way to perform low level
// system and IO operations. The POSIX API was in that era was like a
// compatibility layer. In current Haiku development, both the POSIX API and
// the Haiku API are considered to be co-equal status. However, they are not
// integrated like they are on other UNIX platforms, which means that for many
// low level concepts there are two versions, like processes (POSIX) and
// teams (Haiku), or pthreads and native threads.
//
// Both the POSIX API and the Haiku API live in libroot.so, the library that is
// linked to any binary by default. Additionally, Haiku supports several
// non-POSIX APIs from BSD and GNU, which live in libbsd.so and libgnu.so. These
// modules are also supported.
//
// The module is comprised of the following files:
// - `mod.rs` (this file) implements the C11 and POSIX API found in
//     `headers/posix`
// - `b32.rs`, `b64.rs` and `x86_64.rs` contain platform-specific definitions
//      of the C11 and POSIX APIs
// - `native.rs` defines the native Haiku API that is implemented in
//     `libroot.so` and that are found in `headers/os`.
// - `bsd.rs` defines the BSD customizations available on Haiku found in
//     `headers/compatibility/bsd`

pub type rlim_t = crate::uintptr_t;
pub type sa_family_t = u8;
pub type pthread_key_t = c_int;
pub type nfds_t = c_ulong;
pub type tcflag_t = c_uint;
pub type speed_t = c_uchar;
pub type clock_t = i32;
pub type clockid_t = i32;
pub type suseconds_t = i32;
pub type wchar_t = i32;
pub type off_t = i64;
pub type ino_t = i64;
pub type blkcnt_t = i64;
pub type blksize_t = i32;
pub type dev_t = i32;
pub type mode_t = u32;
pub type nlink_t = i32;
pub type useconds_t = u32;
pub type socklen_t = u32;
pub type pthread_t = crate::uintptr_t;
pub type pthread_condattr_t = crate::uintptr_t;
pub type pthread_mutexattr_t = crate::uintptr_t;
pub type pthread_rwlockattr_t = crate::uintptr_t;
pub type sigset_t = u64;
pub type fsblkcnt_t = i64;
pub type fsfilcnt_t = i64;
pub type pthread_attr_t = *mut c_void;
pub type nl_item = c_int;
pub type id_t = i32;
pub type idtype_t = c_int;
pub type fd_mask = u32;
pub type regoff_t = c_int;
pub type key_t = i32;
pub type msgqnum_t = u32;
pub type msglen_t = u32;

pub type Elf32_Addr = u32;
pub type Elf32_Half = u16;
pub type Elf32_Off = u32;
pub type Elf32_Sword = i32;
pub type Elf32_Word = u32;

pub type Elf64_Addr = u64;
pub type Elf64_Half = u16;
pub type Elf64_Off = u64;
pub type Elf64_Sword = i32;
pub type Elf64_Sxword = i64;
pub type Elf64_Word = u32;
pub type Elf64_Xword = u64;

pub type ENTRY = entry;
pub type ACTION = c_int;

pub type posix_spawnattr_t = *mut c_void;
pub type posix_spawn_file_actions_t = *mut c_void;

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum timezone {}
impl Copy for timezone {}
impl Clone for timezone {
    fn clone(&self) -> timezone {
        *self
    }
}

impl siginfo_t {
    pub unsafe fn si_addr(&self) -> *mut c_void {
        self.si_addr
    }

    pub unsafe fn si_pid(&self) -> crate::pid_t {
        self.si_pid
    }

    pub unsafe fn si_uid(&self) -> crate::uid_t {
        self.si_uid
    }

    pub unsafe fn si_status(&self) -> c_int {
        self.si_status
    }
}

s! {
    pub struct in_addr {
        pub s_addr: crate::in_addr_t,
    }

    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct sockaddr {
        pub sa_len: u8,
        pub sa_family: sa_family_t,
        pub sa_data: [u8; 30],
    }

    pub struct sockaddr_in {
        pub sin_len: u8,
        pub sin_family: sa_family_t,
        pub sin_port: crate::in_port_t,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [i8; 24],
    }

    pub struct sockaddr_in6 {
        pub sin6_len: u8,
        pub sin6_family: u8,
        pub sin6_port: u16,
        pub sin6_flowinfo: u32,
        pub sin6_addr: crate::in6_addr,
        pub sin6_scope_id: u32,
    }

    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        pub ai_addrlen: socklen_t,
        pub ai_canonname: *mut c_char,
        pub ai_addr: *mut crate::sockaddr,
        pub ai_next: *mut addrinfo,
    }

    pub struct ifaddrs {
        pub ifa_next: *mut ifaddrs,
        pub ifa_name: *const c_char,
        pub ifa_flags: c_uint,
        pub ifa_addr: *mut crate::sockaddr,
        pub ifa_netmask: *mut crate::sockaddr,
        pub ifa_dstaddr: *mut crate::sockaddr,
        pub ifa_data: *mut c_void,
    }

    pub struct fd_set {
        // size for 1024 bits, and a fd_mask with size u32
        fds_bits: [fd_mask; 32],
    }

    pub struct tm {
        pub tm_sec: c_int,
        pub tm_min: c_int,
        pub tm_hour: c_int,
        pub tm_mday: c_int,
        pub tm_mon: c_int,
        pub tm_year: c_int,
        pub tm_wday: c_int,
        pub tm_yday: c_int,
        pub tm_isdst: c_int,
        pub tm_gmtoff: c_int,
        pub tm_zone: *mut c_char,
    }

    pub struct utsname {
        pub sysname: [c_char; 32],
        pub nodename: [c_char; 32],
        pub release: [c_char; 32],
        pub version: [c_char; 32],
        pub machine: [c_char; 32],
    }

    pub struct lconv {
        pub decimal_point: *mut c_char,
        pub thousands_sep: *mut c_char,
        pub grouping: *mut c_char,
        pub int_curr_symbol: *mut c_char,
        pub currency_symbol: *mut c_char,
        pub mon_decimal_point: *mut c_char,
        pub mon_thousands_sep: *mut c_char,
        pub mon_grouping: *mut c_char,
        pub positive_sign: *mut c_char,
        pub negative_sign: *mut c_char,
        pub int_frac_digits: c_char,
        pub frac_digits: c_char,
        pub p_cs_precedes: c_char,
        pub p_sep_by_space: c_char,
        pub n_cs_precedes: c_char,
        pub n_sep_by_space: c_char,
        pub p_sign_posn: c_char,
        pub n_sign_posn: c_char,
        pub int_p_cs_precedes: c_char,
        pub int_p_sep_by_space: c_char,
        pub int_n_cs_precedes: c_char,
        pub int_n_sep_by_space: c_char,
        pub int_p_sign_posn: c_char,
        pub int_n_sign_posn: c_char,
    }

    pub struct msghdr {
        pub msg_name: *mut c_void,
        pub msg_namelen: socklen_t,
        pub msg_iov: *mut crate::iovec,
        pub msg_iovlen: c_int,
        pub msg_control: *mut c_void,
        pub msg_controllen: socklen_t,
        pub msg_flags: c_int,
    }

    pub struct cmsghdr {
        pub cmsg_len: crate::socklen_t,
        pub cmsg_level: c_int,
        pub cmsg_type: c_int,
    }

    pub struct Dl_info {
        pub dli_fname: *const c_char,
        pub dli_fbase: *mut c_void,
        pub dli_sname: *const c_char,
        pub dli_saddr: *mut c_void,
    }

    pub struct termios {
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_line: c_char,
        pub c_ispeed: crate::speed_t,
        pub c_ospeed: crate::speed_t,
        pub c_cc: [crate::cc_t; crate::NCCS],
    }

    pub struct flock {
        pub l_type: c_short,
        pub l_whence: c_short,
        pub l_start: off_t,
        pub l_len: off_t,
        pub l_pid: crate::pid_t,
    }

    pub struct stat {
        pub st_dev: dev_t,
        pub st_ino: ino_t,
        pub st_mode: mode_t,
        pub st_nlink: nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_size: off_t,
        pub st_rdev: dev_t,
        pub st_blksize: blksize_t,
        pub st_atime: time_t,
        pub st_atime_nsec: c_long,
        pub st_mtime: time_t,
        pub st_mtime_nsec: c_long,
        pub st_ctime: time_t,
        pub st_ctime_nsec: c_long,
        pub st_crtime: time_t,
        pub st_crtime_nsec: c_long,
        pub st_type: u32,
        pub st_blocks: blkcnt_t,
    }

    pub struct glob_t {
        pub gl_pathc: size_t,
        __unused1: size_t,
        pub gl_offs: size_t,
        __unused2: size_t,
        pub gl_pathv: *mut *mut c_char,

        __unused3: *mut c_void,
        __unused4: *mut c_void,
        __unused5: *mut c_void,
        __unused6: *mut c_void,
        __unused7: *mut c_void,
        __unused8: *mut c_void,
    }

    pub struct pthread_mutex_t {
        flags: u32,
        lock: i32,
        unused: i32,
        owner: i32,
        owner_count: i32,
    }

    pub struct pthread_cond_t {
        flags: u32,
        unused: i32,
        mutex: *mut c_void,
        waiter_count: i32,
        lock: i32,
    }

    pub struct pthread_rwlock_t {
        flags: u32,
        owner: i32,
        lock_sem: i32, // this is actually a union
        lock_count: i32,
        reader_count: i32,
        writer_count: i32,
        waiters: [*mut c_void; 2],
    }

    pub struct pthread_spinlock_t {
        lock: u32,
    }

    pub struct passwd {
        pub pw_name: *mut c_char,
        pub pw_passwd: *mut c_char,
        pub pw_uid: crate::uid_t,
        pub pw_gid: crate::gid_t,
        pub pw_dir: *mut c_char,
        pub pw_shell: *mut c_char,
        pub pw_gecos: *mut c_char,
    }

    pub struct statvfs {
        pub f_bsize: c_ulong,
        pub f_frsize: c_ulong,
        pub f_blocks: crate::fsblkcnt_t,
        pub f_bfree: crate::fsblkcnt_t,
        pub f_bavail: crate::fsblkcnt_t,
        pub f_files: crate::fsfilcnt_t,
        pub f_ffree: crate::fsfilcnt_t,
        pub f_favail: crate::fsfilcnt_t,
        pub f_fsid: c_ulong,
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_size: size_t,
        pub ss_flags: c_int,
    }

    pub struct siginfo_t {
        pub si_signo: c_int,
        pub si_code: c_int,
        pub si_errno: c_int,
        pub si_pid: crate::pid_t,
        pub si_uid: crate::uid_t,
        pub si_addr: *mut c_void,
        pub si_status: c_int,
        pub si_band: c_long,
        pub sigval: *mut c_void,
    }

    pub struct sigaction {
        pub sa_sigaction: crate::sighandler_t, //actually a union with sa_handler
        pub sa_mask: crate::sigset_t,
        pub sa_flags: c_int,
        sa_userdata: *mut c_void,
    }

    pub struct sem_t {
        pub type_: i32,
        pub named_sem_id: i32, // actually a union with unnamed_sem (i32)
        pub padding: [i32; 2],
    }

    pub struct ucred {
        pub pid: crate::pid_t,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
    }

    pub struct sockaddr_dl {
        pub sdl_len: u8,
        pub sdl_family: u8,
        pub sdl_e_type: u16,
        pub sdl_index: u32,
        pub sdl_type: u8,
        pub sdl_nlen: u8,
        pub sdl_alen: u8,
        pub sdl_slen: u8,
        pub sdl_data: [u8; 46],
    }

    pub struct spwd {
        pub sp_namp: *mut c_char,
        pub sp_pwdp: *mut c_char,
        pub sp_lstchg: c_int,
        pub sp_min: c_int,
        pub sp_max: c_int,
        pub sp_warn: c_int,
        pub sp_inact: c_int,
        pub sp_expire: c_int,
        pub sp_flag: c_int,
    }

    pub struct regex_t {
        __buffer: *mut c_void,
        __allocated: size_t,
        __used: size_t,
        __syntax: c_ulong,
        __fastmap: *mut c_char,
        __translate: *mut c_char,
        __re_nsub: size_t,
        __bitfield: u8,
    }

    pub struct regmatch_t {
        pub rm_so: regoff_t,
        pub rm_eo: regoff_t,
    }

    pub struct msqid_ds {
        pub msg_perm: crate::ipc_perm,
        pub msg_qnum: crate::msgqnum_t,
        pub msg_qbytes: crate::msglen_t,
        pub msg_lspid: crate::pid_t,
        pub msg_lrpid: crate::pid_t,
        pub msg_stime: crate::time_t,
        pub msg_rtime: crate::time_t,
        pub msg_ctime: crate::time_t,
    }

    pub struct ipc_perm {
        pub key: crate::key_t,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub cuid: crate::uid_t,
        pub cgid: crate::gid_t,
        pub mode: mode_t,
    }

    pub struct sembuf {
        pub sem_num: c_ushort,
        pub sem_op: c_short,
        pub sem_flg: c_short,
    }

    pub struct entry {
        pub key: *mut c_char,
        pub data: *mut c_void,
    }

    pub struct option {
        pub name: *const c_char,
        pub has_arg: c_int,
        pub flag: *mut c_int,
        pub val: c_int,
    }
}

s_no_extra_traits! {
    pub struct sockaddr_un {
        pub sun_len: u8,
        pub sun_family: sa_family_t,
        pub sun_path: [c_char; 126],
    }
    pub struct sockaddr_storage {
        pub ss_len: u8,
        pub ss_family: sa_family_t,
        __ss_pad1: [u8; 6],
        __ss_pad2: u64,
        __ss_pad3: [u8; 112],
    }
    pub struct dirent {
        pub d_dev: dev_t,
        pub d_pdev: dev_t,
        pub d_ino: ino_t,
        pub d_pino: i64,
        pub d_reclen: c_ushort,
        pub d_name: [c_char; 1024], // Max length is _POSIX_PATH_MAX
    }

    pub struct sigevent {
        pub sigev_notify: c_int,
        pub sigev_signo: c_int,
        pub sigev_value: crate::sigval,
        __unused1: *mut c_void, // actually a function pointer
        pub sigev_notify_attributes: *mut crate::pthread_attr_t,
    }

    pub struct utmpx {
        pub ut_type: c_short,
        pub ut_tv: crate::timeval,
        pub ut_id: [c_char; 8],
        pub ut_pid: crate::pid_t,
        pub ut_user: [c_char; 32],
        pub ut_line: [c_char; 16],
        pub ut_host: [c_char; 128],
        __ut_reserved: [c_char; 64],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for utmpx {
            fn eq(&self, other: &utmpx) -> bool {
                self.ut_type == other.ut_type
                    && self.ut_tv == other.ut_tv
                    && self.ut_id == other.ut_id
                    && self.ut_pid == other.ut_pid
                    && self.ut_user == other.ut_user
                    && self.ut_line == other.ut_line
                    && self
                        .ut_host
                        .iter()
                        .zip(other.ut_host.iter())
                        .all(|(a, b)| a == b)
                    && self.__ut_reserved == other.__ut_reserved
            }
        }

        impl Eq for utmpx {}
        impl hash::Hash for utmpx {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ut_type.hash(state);
                self.ut_tv.hash(state);
                self.ut_id.hash(state);
                self.ut_pid.hash(state);
                self.ut_user.hash(state);
                self.ut_line.hash(state);
                self.ut_host.hash(state);
                self.__ut_reserved.hash(state);
            }
        }
        impl PartialEq for sockaddr_un {
            fn eq(&self, other: &sockaddr_un) -> bool {
                self.sun_len == other.sun_len
                    && self.sun_family == other.sun_family
                    && self
                        .sun_path
                        .iter()
                        .zip(other.sun_path.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for sockaddr_un {}
        impl hash::Hash for sockaddr_un {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sun_len.hash(state);
                self.sun_family.hash(state);
                self.sun_path.hash(state);
            }
        }

        impl PartialEq for sockaddr_storage {
            fn eq(&self, other: &sockaddr_storage) -> bool {
                self.ss_len == other.ss_len
                    && self.ss_family == other.ss_family
                    && self
                        .__ss_pad1
                        .iter()
                        .zip(other.__ss_pad1.iter())
                        .all(|(a, b)| a == b)
                    && self.__ss_pad2 == other.__ss_pad2
                    && self
                        .__ss_pad3
                        .iter()
                        .zip(other.__ss_pad3.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for sockaddr_storage {}
        impl hash::Hash for sockaddr_storage {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ss_len.hash(state);
                self.ss_family.hash(state);
                self.__ss_pad1.hash(state);
                self.__ss_pad2.hash(state);
                self.__ss_pad3.hash(state);
            }
        }

        impl PartialEq for dirent {
            fn eq(&self, other: &dirent) -> bool {
                self.d_dev == other.d_dev
                    && self.d_pdev == other.d_pdev
                    && self.d_ino == other.d_ino
                    && self.d_pino == other.d_pino
                    && self.d_reclen == other.d_reclen
                    && self
                        .d_name
                        .iter()
                        .zip(other.d_name.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for dirent {}
        impl hash::Hash for dirent {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.d_dev.hash(state);
                self.d_pdev.hash(state);
                self.d_ino.hash(state);
                self.d_pino.hash(state);
                self.d_reclen.hash(state);
                self.d_name.hash(state);
            }
        }

        impl PartialEq for sigevent {
            fn eq(&self, other: &sigevent) -> bool {
                self.sigev_notify == other.sigev_notify
                    && self.sigev_signo == other.sigev_signo
                    && self.sigev_value == other.sigev_value
                    && self.sigev_notify_attributes == other.sigev_notify_attributes
            }
        }
        impl Eq for sigevent {}
        impl hash::Hash for sigevent {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sigev_notify.hash(state);
                self.sigev_signo.hash(state);
                self.sigev_value.hash(state);
                self.sigev_notify_attributes.hash(state);
            }
        }
    }
}

pub const EXIT_FAILURE: c_int = 1;
pub const EXIT_SUCCESS: c_int = 0;
pub const RAND_MAX: c_int = 2147483647;
pub const EOF: c_int = -1;
pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;
pub const L_SET: c_int = SEEK_SET;
pub const L_INCR: c_int = SEEK_CUR;
pub const L_XTND: c_int = SEEK_END;
pub const _IOFBF: c_int = 0;
pub const _IONBF: c_int = 2;
pub const _IOLBF: c_int = 1;

pub const F_DUPFD: c_int = 0x0001;
pub const F_GETFD: c_int = 0x0002;
pub const F_SETFD: c_int = 0x0004;
pub const F_GETFL: c_int = 0x0008;
pub const F_SETFL: c_int = 0x0010;
pub const F_GETLK: c_int = 0x0020;
pub const F_SETLK: c_int = 0x0080;
pub const F_SETLKW: c_int = 0x0100;
pub const F_DUPFD_CLOEXEC: c_int = 0x0200;

pub const F_RDLCK: c_int = 0x0040;
pub const F_UNLCK: c_int = 0x0200;
pub const F_WRLCK: c_int = 0x0400;

pub const AT_FDCWD: c_int = -100;
pub const AT_SYMLINK_NOFOLLOW: c_int = 0x01;
pub const AT_SYMLINK_FOLLOW: c_int = 0x02;
pub const AT_REMOVEDIR: c_int = 0x04;
pub const AT_EACCESS: c_int = 0x08;

pub const POLLIN: c_short = 0x0001;
pub const POLLOUT: c_short = 0x0002;
pub const POLLRDNORM: c_short = POLLIN;
pub const POLLWRNORM: c_short = POLLOUT;
pub const POLLRDBAND: c_short = 0x0008;
pub const POLLWRBAND: c_short = 0x0010;
pub const POLLPRI: c_short = 0x0020;
pub const POLLERR: c_short = 0x0004;
pub const POLLHUP: c_short = 0x0080;
pub const POLLNVAL: c_short = 0x1000;

pub const PTHREAD_CREATE_JOINABLE: c_int = 0;
pub const PTHREAD_CREATE_DETACHED: c_int = 1;

pub const CLOCK_REALTIME: c_int = -1;
pub const CLOCK_MONOTONIC: c_int = 0;
pub const CLOCK_PROCESS_CPUTIME_ID: c_int = -2;
pub const CLOCK_THREAD_CPUTIME_ID: c_int = -3;

pub const RLIMIT_CORE: c_int = 0;
pub const RLIMIT_CPU: c_int = 1;
pub const RLIMIT_DATA: c_int = 2;
pub const RLIMIT_FSIZE: c_int = 3;
pub const RLIMIT_NOFILE: c_int = 4;
pub const RLIMIT_STACK: c_int = 5;
pub const RLIMIT_AS: c_int = 6;
pub const RLIM_INFINITY: crate::rlim_t = 0xffffffff;
// Haiku specific
pub const RLIMIT_NOVMON: c_int = 7;
#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIM_NLIMITS: c_int = 8;

pub const RUSAGE_SELF: c_int = 0;

pub const RTLD_LAZY: c_int = 0;

pub const NCCS: usize = 11;

pub const O_RDONLY: c_int = 0x0000;
pub const O_WRONLY: c_int = 0x0001;
pub const O_RDWR: c_int = 0x0002;
pub const O_ACCMODE: c_int = 0x0003;

pub const O_EXCL: c_int = 0x0100;
pub const O_CREAT: c_int = 0x0200;
pub const O_TRUNC: c_int = 0x0400;
pub const O_NOCTTY: c_int = 0x1000;
pub const O_NOTRAVERSE: c_int = 0x2000;

pub const O_CLOEXEC: c_int = 0x00000040;
pub const O_NONBLOCK: c_int = 0x00000080;
pub const O_APPEND: c_int = 0x00000800;
pub const O_SYNC: c_int = 0x00010000;
pub const O_RSYNC: c_int = 0x00020000;
pub const O_DSYNC: c_int = 0x00040000;
pub const O_NOFOLLOW: c_int = 0x00080000;
pub const O_NOCACHE: c_int = 0x00100000;
pub const O_DIRECTORY: c_int = 0x00200000;

pub const S_IFIFO: mode_t = 0o1_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_IFMT: mode_t = 0o17_0000;

pub const S_IRWXU: mode_t = 0o0700;
pub const S_IRUSR: mode_t = 0o0400;
pub const S_IWUSR: mode_t = 0o0200;
pub const S_IXUSR: mode_t = 0o0100;
pub const S_IRWXG: mode_t = 0o0070;
pub const S_IRGRP: mode_t = 0o0040;
pub const S_IWGRP: mode_t = 0o0020;
pub const S_IXGRP: mode_t = 0o0010;
pub const S_IRWXO: mode_t = 0o0007;
pub const S_IROTH: mode_t = 0o0004;
pub const S_IWOTH: mode_t = 0o0002;
pub const S_IXOTH: mode_t = 0o0001;

pub const F_OK: c_int = 0;
pub const R_OK: c_int = 4;
pub const W_OK: c_int = 2;
pub const X_OK: c_int = 1;
pub const STDIN_FILENO: c_int = 0;
pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;

pub const SIGHUP: c_int = 1;
pub const SIGINT: c_int = 2;
pub const SIGQUIT: c_int = 3;
pub const SIGILL: c_int = 4;
pub const SIGCHLD: c_int = 5;
pub const SIGABRT: c_int = 6;
pub const SIGPIPE: c_int = 7;
pub const SIGFPE: c_int = 8;
pub const SIGKILL: c_int = 9;
pub const SIGSTOP: c_int = 10;
pub const SIGSEGV: c_int = 11;
pub const SIGCONT: c_int = 12;
pub const SIGTSTP: c_int = 13;
pub const SIGALRM: c_int = 14;
pub const SIGTERM: c_int = 15;
pub const SIGTTIN: c_int = 16;
pub const SIGTTOU: c_int = 17;
pub const SIGUSR1: c_int = 18;
pub const SIGUSR2: c_int = 19;
pub const SIGWINCH: c_int = 20;
pub const SIGKILLTHR: c_int = 21;
pub const SIGTRAP: c_int = 22;
pub const SIGPOLL: c_int = 23;
pub const SIGPROF: c_int = 24;
pub const SIGSYS: c_int = 25;
pub const SIGURG: c_int = 26;
pub const SIGVTALRM: c_int = 27;
pub const SIGXCPU: c_int = 28;
pub const SIGXFSZ: c_int = 29;
pub const SIGBUS: c_int = 30;

pub const SIG_BLOCK: c_int = 1;
pub const SIG_UNBLOCK: c_int = 2;
pub const SIG_SETMASK: c_int = 3;

pub const SIGEV_NONE: c_int = 0;
pub const SIGEV_SIGNAL: c_int = 1;
pub const SIGEV_THREAD: c_int = 2;

pub const EAI_AGAIN: c_int = 2;
pub const EAI_BADFLAGS: c_int = 3;
pub const EAI_FAIL: c_int = 4;
pub const EAI_FAMILY: c_int = 5;
pub const EAI_MEMORY: c_int = 6;
pub const EAI_NODATA: c_int = 7;
pub const EAI_NONAME: c_int = 8;
pub const EAI_SERVICE: c_int = 9;
pub const EAI_SOCKTYPE: c_int = 10;
pub const EAI_SYSTEM: c_int = 11;
pub const EAI_OVERFLOW: c_int = 14;

pub const PROT_NONE: c_int = 0;
pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const PROT_EXEC: c_int = 4;

pub const LC_ALL: c_int = 0;
pub const LC_COLLATE: c_int = 1;
pub const LC_CTYPE: c_int = 2;
pub const LC_MONETARY: c_int = 3;
pub const LC_NUMERIC: c_int = 4;
pub const LC_TIME: c_int = 5;
pub const LC_MESSAGES: c_int = 6;

// FIXME(haiku): Haiku does not have MAP_FILE, but library/std/os.rs requires it
pub const MAP_FILE: c_int = 0x00;
pub const MAP_SHARED: c_int = 0x01;
pub const MAP_PRIVATE: c_int = 0x02;
pub const MAP_FIXED: c_int = 0x04;
pub const MAP_ANONYMOUS: c_int = 0x08;
pub const MAP_NORESERVE: c_int = 0x10;
pub const MAP_ANON: c_int = MAP_ANONYMOUS;

pub const MAP_FAILED: *mut c_void = !0 as *mut c_void;

pub const MS_ASYNC: c_int = 0x01;
pub const MS_INVALIDATE: c_int = 0x04;
pub const MS_SYNC: c_int = 0x02;

pub const E2BIG: c_int = -2147454975;
pub const ECHILD: c_int = -2147454974;
pub const EDEADLK: c_int = -2147454973;
pub const EFBIG: c_int = -2147454972;
pub const EMLINK: c_int = -2147454971;
pub const ENFILE: c_int = -2147454970;
pub const ENODEV: c_int = -2147454969;
pub const ENOLCK: c_int = -2147454968;
pub const ENOSYS: c_int = -2147454967;
pub const ENOTTY: c_int = -2147454966;
pub const ENXIO: c_int = -2147454965;
pub const ESPIPE: c_int = -2147454964;
pub const ESRCH: c_int = -2147454963;
pub const EFPOS: c_int = -2147454962;
pub const ESIGPARM: c_int = -2147454961;
pub const EDOM: c_int = -2147454960;
pub const ERANGE: c_int = -2147454959;
pub const EPROTOTYPE: c_int = -2147454958;
pub const EPROTONOSUPPORT: c_int = -2147454957;
pub const EPFNOSUPPORT: c_int = -2147454956;
pub const EAFNOSUPPORT: c_int = -2147454955;
pub const EADDRINUSE: c_int = -2147454954;
pub const EADDRNOTAVAIL: c_int = -2147454953;
pub const ENETDOWN: c_int = -2147454952;
pub const ENETUNREACH: c_int = -2147454951;
pub const ENETRESET: c_int = -2147454950;
pub const ECONNABORTED: c_int = -2147454949;
pub const ECONNRESET: c_int = -2147454948;
pub const EISCONN: c_int = -2147454947;
pub const ENOTCONN: c_int = -2147454946;
pub const ESHUTDOWN: c_int = -2147454945;
pub const ECONNREFUSED: c_int = -2147454944;
pub const EHOSTUNREACH: c_int = -2147454943;
pub const ENOPROTOOPT: c_int = -2147454942;
pub const ENOBUFS: c_int = -2147454941;
pub const EINPROGRESS: c_int = -2147454940;
pub const EALREADY: c_int = -2147454939;
pub const EILSEQ: c_int = -2147454938;
pub const ENOMSG: c_int = -2147454937;
pub const ESTALE: c_int = -2147454936;
pub const EOVERFLOW: c_int = -2147454935;
pub const EMSGSIZE: c_int = -2147454934;
pub const EOPNOTSUPP: c_int = -2147454933;
pub const ENOTSOCK: c_int = -2147454932;
pub const EHOSTDOWN: c_int = -2147454931;
pub const EBADMSG: c_int = -2147454930;
pub const ECANCELED: c_int = -2147454929;
pub const EDESTADDRREQ: c_int = -2147454928;
pub const EDQUOT: c_int = -2147454927;
pub const EIDRM: c_int = -2147454926;
pub const EMULTIHOP: c_int = -2147454925;
pub const ENODATA: c_int = -2147454924;
pub const ENOLINK: c_int = -2147454923;
pub const ENOSR: c_int = -2147454922;
pub const ENOSTR: c_int = -2147454921;
pub const ENOTSUP: c_int = -2147454920;
pub const EPROTO: c_int = -2147454919;
pub const ETIME: c_int = -2147454918;
pub const ETXTBSY: c_int = -2147454917;
pub const ENOATTR: c_int = -2147454916;

// INT_MIN
pub const ENOMEM: c_int = -2147483648;

// POSIX errors that can be mapped to BeOS error codes
pub const EACCES: c_int = -2147483646;
pub const EINTR: c_int = -2147483638;
pub const EIO: c_int = -2147483647;
pub const EBUSY: c_int = -2147483634;
pub const EFAULT: c_int = -2147478783;
pub const ETIMEDOUT: c_int = -2147483639;
pub const EAGAIN: c_int = -2147483637;
pub const EWOULDBLOCK: c_int = -2147483637;
pub const EBADF: c_int = -2147459072;
pub const EEXIST: c_int = -2147459070;
pub const EINVAL: c_int = -2147483643;
pub const ENAMETOOLONG: c_int = -2147459068;
pub const ENOENT: c_int = -2147459069;
pub const EPERM: c_int = -2147483633;
pub const ENOTDIR: c_int = -2147459067;
pub const EISDIR: c_int = -2147459063;
pub const ENOTEMPTY: c_int = -2147459066;
pub const ENOSPC: c_int = -2147459065;
pub const EROFS: c_int = -2147459064;
pub const EMFILE: c_int = -2147459062;
pub const EXDEV: c_int = -2147459061;
pub const ELOOP: c_int = -2147459060;
pub const ENOEXEC: c_int = -2147478782;
pub const EPIPE: c_int = -2147459059;

pub const IPPROTO_RAW: c_int = 255;

// These are prefixed with POSIX_ on Haiku
pub const MADV_NORMAL: c_int = 1;
pub const MADV_SEQUENTIAL: c_int = 2;
pub const MADV_RANDOM: c_int = 3;
pub const MADV_WILLNEED: c_int = 4;
pub const MADV_DONTNEED: c_int = 5;
pub const MADV_FREE: c_int = 6;

// https://github.com/haiku/haiku/blob/HEAD/headers/posix/net/if.h#L80
pub const IFF_UP: c_int = 0x0001;
pub const IFF_BROADCAST: c_int = 0x0002; // valid broadcast address
pub const IFF_LOOPBACK: c_int = 0x0008;
pub const IFF_POINTOPOINT: c_int = 0x0010; // point-to-point link
pub const IFF_NOARP: c_int = 0x0040; // no address resolution
pub const IFF_AUTOUP: c_int = 0x0080; // auto dial
pub const IFF_PROMISC: c_int = 0x0100; // receive all packets
pub const IFF_ALLMULTI: c_int = 0x0200; // receive all multicast packets
pub const IFF_SIMPLEX: c_int = 0x0800; // doesn't receive own transmissions
pub const IFF_LINK: c_int = 0x1000; // has link
pub const IFF_AUTO_CONFIGURED: c_int = 0x2000;
pub const IFF_CONFIGURING: c_int = 0x4000;
pub const IFF_MULTICAST: c_int = 0x8000; // supports multicast

pub const AF_UNSPEC: c_int = 0;
pub const AF_INET: c_int = 1;
pub const AF_APPLETALK: c_int = 2;
pub const AF_ROUTE: c_int = 3;
pub const AF_LINK: c_int = 4;
pub const AF_INET6: c_int = 5;
pub const AF_DLI: c_int = 6;
pub const AF_IPX: c_int = 7;
pub const AF_NOTIFY: c_int = 8;
pub const AF_LOCAL: c_int = 9;
pub const AF_UNIX: c_int = AF_LOCAL;
pub const AF_BLUETOOTH: c_int = 10;

pub const PF_UNSPEC: c_int = AF_UNSPEC;
pub const PF_INET: c_int = AF_INET;
pub const PF_ROUTE: c_int = AF_ROUTE;
pub const PF_LINK: c_int = AF_LINK;
pub const PF_INET6: c_int = AF_INET6;
pub const PF_LOCAL: c_int = AF_LOCAL;
pub const PF_UNIX: c_int = AF_UNIX;
pub const PF_BLUETOOTH: c_int = AF_BLUETOOTH;

pub const IP_OPTIONS: c_int = 1;
pub const IP_HDRINCL: c_int = 2;
pub const IP_TOS: c_int = 3;
pub const IP_TTL: c_int = 4;
pub const IP_RECVOPTS: c_int = 5;
pub const IP_RECVRETOPTS: c_int = 6;
pub const IP_RECVDSTADDR: c_int = 7;
pub const IP_RETOPTS: c_int = 8;
pub const IP_MULTICAST_IF: c_int = 9;
pub const IP_MULTICAST_TTL: c_int = 10;
pub const IP_MULTICAST_LOOP: c_int = 11;
pub const IP_ADD_MEMBERSHIP: c_int = 12;
pub const IP_DROP_MEMBERSHIP: c_int = 13;
pub const IP_BLOCK_SOURCE: c_int = 14;
pub const IP_UNBLOCK_SOURCE: c_int = 15;
pub const IP_ADD_SOURCE_MEMBERSHIP: c_int = 16;
pub const IP_DROP_SOURCE_MEMBERSHIP: c_int = 17;

pub const TCP_NODELAY: c_int = 0x01;
pub const TCP_MAXSEG: c_int = 0x02;
pub const TCP_NOPUSH: c_int = 0x04;
pub const TCP_NOOPT: c_int = 0x08;

pub const IF_NAMESIZE: size_t = 32;
pub const IFNAMSIZ: size_t = IF_NAMESIZE;

pub const IPV6_MULTICAST_IF: c_int = 24;
pub const IPV6_MULTICAST_HOPS: c_int = 25;
pub const IPV6_MULTICAST_LOOP: c_int = 26;
pub const IPV6_UNICAST_HOPS: c_int = 27;
pub const IPV6_JOIN_GROUP: c_int = 28;
pub const IPV6_LEAVE_GROUP: c_int = 29;
pub const IPV6_V6ONLY: c_int = 30;
pub const IPV6_PKTINFO: c_int = 31;
pub const IPV6_RECVPKTINFO: c_int = 32;
pub const IPV6_HOPLIMIT: c_int = 33;
pub const IPV6_RECVHOPLIMIT: c_int = 34;
pub const IPV6_HOPOPTS: c_int = 35;
pub const IPV6_DSTOPTS: c_int = 36;
pub const IPV6_RTHDR: c_int = 37;

pub const MSG_OOB: c_int = 0x0001;
pub const MSG_PEEK: c_int = 0x0002;
pub const MSG_DONTROUTE: c_int = 0x0004;
pub const MSG_EOR: c_int = 0x0008;
pub const MSG_TRUNC: c_int = 0x0010;
pub const MSG_CTRUNC: c_int = 0x0020;
pub const MSG_WAITALL: c_int = 0x0040;
pub const MSG_DONTWAIT: c_int = 0x0080;
pub const MSG_BCAST: c_int = 0x0100;
pub const MSG_MCAST: c_int = 0x0200;
pub const MSG_EOF: c_int = 0x0400;
pub const MSG_NOSIGNAL: c_int = 0x0800;

pub const SHUT_RD: c_int = 0;
pub const SHUT_WR: c_int = 1;
pub const SHUT_RDWR: c_int = 2;

pub const LOCK_SH: c_int = 0x01;
pub const LOCK_EX: c_int = 0x02;
pub const LOCK_NB: c_int = 0x04;
pub const LOCK_UN: c_int = 0x08;

pub const MINSIGSTKSZ: size_t = 8192;
pub const SIGSTKSZ: size_t = 16384;

pub const IOV_MAX: c_int = 1024;
pub const PATH_MAX: c_int = 1024;

pub const SA_NOCLDSTOP: c_int = 0x01;
pub const SA_NOCLDWAIT: c_int = 0x02;
pub const SA_RESETHAND: c_int = 0x04;
pub const SA_NODEFER: c_int = 0x08;
pub const SA_RESTART: c_int = 0x10;
pub const SA_ONSTACK: c_int = 0x20;
pub const SA_SIGINFO: c_int = 0x40;
pub const SA_NOMASK: c_int = SA_NODEFER;
pub const SA_STACK: c_int = SA_ONSTACK;
pub const SA_ONESHOT: c_int = SA_RESETHAND;

pub const SS_ONSTACK: c_int = 0x1;
pub const SS_DISABLE: c_int = 0x2;

// DIFF(main): changed to `c_int` in 500365e1
pub const FD_SETSIZE: usize = 1024;

pub const RTLD_LOCAL: c_int = 0x0;
pub const RTLD_NOW: c_int = 0x1;
pub const RTLD_GLOBAL: c_int = 0x2;
pub const RTLD_DEFAULT: *mut c_void = 0isize as *mut c_void;

pub const BUFSIZ: c_uint = 8192;
pub const FILENAME_MAX: c_uint = 256;
pub const FOPEN_MAX: c_uint = 128;
pub const L_tmpnam: c_uint = 512;
pub const TMP_MAX: c_uint = 32768;

pub const _PC_CHOWN_RESTRICTED: c_int = 1;
pub const _PC_MAX_CANON: c_int = 2;
pub const _PC_MAX_INPUT: c_int = 3;
pub const _PC_NAME_MAX: c_int = 4;
pub const _PC_NO_TRUNC: c_int = 5;
pub const _PC_PATH_MAX: c_int = 6;
pub const _PC_PIPE_BUF: c_int = 7;
pub const _PC_VDISABLE: c_int = 8;
pub const _PC_LINK_MAX: c_int = 25;
pub const _PC_SYNC_IO: c_int = 26;
pub const _PC_ASYNC_IO: c_int = 27;
pub const _PC_PRIO_IO: c_int = 28;
pub const _PC_SOCK_MAXBUF: c_int = 29;
pub const _PC_FILESIZEBITS: c_int = 30;
pub const _PC_REC_INCR_XFER_SIZE: c_int = 31;
pub const _PC_REC_MAX_XFER_SIZE: c_int = 32;
pub const _PC_REC_MIN_XFER_SIZE: c_int = 33;
pub const _PC_REC_XFER_ALIGN: c_int = 34;
pub const _PC_ALLOC_SIZE_MIN: c_int = 35;
pub const _PC_SYMLINK_MAX: c_int = 36;
pub const _PC_2_SYMLINKS: c_int = 37;
pub const _PC_XATTR_EXISTS: c_int = 38;
pub const _PC_XATTR_ENABLED: c_int = 39;

pub const FIONBIO: c_ulong = 0xbe000000;
pub const FIONREAD: c_ulong = 0xbe000001;
pub const FIOSEEKDATA: c_ulong = 0xbe000002;
pub const FIOSEEKHOLE: c_ulong = 0xbe000003;

pub const _SC_ARG_MAX: c_int = 15;
pub const _SC_CHILD_MAX: c_int = 16;
pub const _SC_CLK_TCK: c_int = 17;
pub const _SC_JOB_CONTROL: c_int = 18;
pub const _SC_NGROUPS_MAX: c_int = 19;
pub const _SC_OPEN_MAX: c_int = 20;
pub const _SC_SAVED_IDS: c_int = 21;
pub const _SC_STREAM_MAX: c_int = 22;
pub const _SC_TZNAME_MAX: c_int = 23;
pub const _SC_VERSION: c_int = 24;
pub const _SC_GETGR_R_SIZE_MAX: c_int = 25;
pub const _SC_GETPW_R_SIZE_MAX: c_int = 26;
pub const _SC_PAGESIZE: c_int = 27;
pub const _SC_PAGE_SIZE: c_int = 27;
pub const _SC_SEM_NSEMS_MAX: c_int = 28;
pub const _SC_SEM_VALUE_MAX: c_int = 29;
pub const _SC_SEMAPHORES: c_int = 30;
pub const _SC_THREADS: c_int = 31;
pub const _SC_IOV_MAX: c_int = 32;
pub const _SC_UIO_MAXIOV: c_int = 32;
pub const _SC_NPROCESSORS_CONF: c_int = 34;
pub const _SC_NPROCESSORS_ONLN: c_int = 35;
pub const _SC_ATEXIT_MAX: c_int = 37;
pub const _SC_PASS_MAX: c_int = 39;
pub const _SC_PHYS_PAGES: c_int = 40;
pub const _SC_AVPHYS_PAGES: c_int = 41;
pub const _SC_PIPE: c_int = 42;
pub const _SC_SELECT: c_int = 43;
pub const _SC_POLL: c_int = 44;
pub const _SC_MAPPED_FILES: c_int = 45;
pub const _SC_THREAD_PROCESS_SHARED: c_int = 46;
pub const _SC_THREAD_STACK_MIN: c_int = 47;
pub const _SC_THREAD_ATTR_STACKADDR: c_int = 48;
pub const _SC_THREAD_ATTR_STACKSIZE: c_int = 49;
pub const _SC_THREAD_PRIORITY_SCHEDULING: c_int = 50;
pub const _SC_REALTIME_SIGNALS: c_int = 51;
pub const _SC_MEMORY_PROTECTION: c_int = 52;
pub const _SC_SIGQUEUE_MAX: c_int = 53;
pub const _SC_RTSIG_MAX: c_int = 54;
pub const _SC_MONOTONIC_CLOCK: c_int = 55;
pub const _SC_DELAYTIMER_MAX: c_int = 56;
pub const _SC_TIMER_MAX: c_int = 57;
pub const _SC_TIMERS: c_int = 58;
pub const _SC_CPUTIME: c_int = 59;
pub const _SC_THREAD_CPUTIME: c_int = 60;
pub const _SC_HOST_NAME_MAX: c_int = 61;
pub const _SC_REGEXP: c_int = 62;
pub const _SC_SYMLOOP_MAX: c_int = 63;
pub const _SC_SHELL: c_int = 64;
pub const _SC_TTY_NAME_MAX: c_int = 65;
pub const _SC_ADVISORY_INFO: c_int = 66;
pub const _SC_BARRIERS: c_int = 67;
pub const _SC_CLOCK_SELECTION: c_int = 68;
pub const _SC_FSYNC: c_int = 69;
pub const _SC_IPV6: c_int = 70;
pub const _SC_MEMLOCK: c_int = 71;
pub const _SC_MEMLOCK_RANGE: c_int = 72;
pub const _SC_MESSAGE_PASSING: c_int = 73;
pub const _SC_PRIORITIZED_IO: c_int = 74;
pub const _SC_PRIORITY_SCHEDULING: c_int = 75;
pub const _SC_READER_WRITER_LOCKS: c_int = 76;
pub const _SC_SHARED_MEMORY_OBJECTS: c_int = 77;
pub const _SC_SPAWN: c_int = 78;
pub const _SC_SPIN_LOCKS: c_int = 79;
pub const _SC_SPORADIC_SERVER: c_int = 80;
pub const _SC_SYNCHRONIZED_IO: c_int = 81;
pub const _SC_THREAD_PRIO_INHERIT: c_int = 82;
pub const _SC_THREAD_PRIO_PROTECT: c_int = 83;
pub const _SC_THREAD_ROBUST_PRIO_INHERIT: c_int = 84;
pub const _SC_THREAD_ROBUST_PRIO_PROTECT: c_int = 85;
pub const _SC_THREAD_SAFE_FUNCTIONS: c_int = 86;
pub const _SC_THREAD_SPORADIC_SERVER: c_int = 87;
pub const _SC_TIMEOUTS: c_int = 88;
pub const _SC_TRACE: c_int = 89;
pub const _SC_TRACE_EVENT_FILTER: c_int = 90;
pub const _SC_TRACE_INHERIT: c_int = 91;
pub const _SC_TRACE_LOG: c_int = 92;
pub const _SC_TYPED_MEMORY_OBJECTS: c_int = 93;
pub const _SC_V6_ILP32_OFF32: c_int = 94;
pub const _SC_V6_ILP32_OFFBIG: c_int = 95;
pub const _SC_V6_LP64_OFF64: c_int = 96;
pub const _SC_V6_LPBIG_OFFBIG: c_int = 97;
pub const _SC_V7_ILP32_OFF32: c_int = 98;
pub const _SC_V7_ILP32_OFFBIG: c_int = 99;
pub const _SC_V7_LP64_OFF64: c_int = 100;
pub const _SC_V7_LPBIG_OFFBIG: c_int = 101;
pub const _SC_2_C_BIND: c_int = 102;
pub const _SC_2_C_DEV: c_int = 103;
pub const _SC_2_CHAR_TERM: c_int = 104;
pub const _SC_2_FORT_DEV: c_int = 105;
pub const _SC_2_FORT_RUN: c_int = 106;
pub const _SC_2_LOCALEDEF: c_int = 107;
pub const _SC_2_PBS: c_int = 108;
pub const _SC_2_PBS_ACCOUNTING: c_int = 109;
pub const _SC_2_PBS_CHECKPOINT: c_int = 110;
pub const _SC_2_PBS_LOCATE: c_int = 111;
pub const _SC_2_PBS_MESSAGE: c_int = 112;
pub const _SC_2_PBS_TRACK: c_int = 113;
pub const _SC_2_SW_DEV: c_int = 114;
pub const _SC_2_UPE: c_int = 115;
pub const _SC_2_VERSION: c_int = 116;
pub const _SC_XOPEN_CRYPT: c_int = 117;
pub const _SC_XOPEN_ENH_I18N: c_int = 118;
pub const _SC_XOPEN_REALTIME: c_int = 119;
pub const _SC_XOPEN_REALTIME_THREADS: c_int = 120;
pub const _SC_XOPEN_SHM: c_int = 121;
pub const _SC_XOPEN_STREAMS: c_int = 122;
pub const _SC_XOPEN_UNIX: c_int = 123;
pub const _SC_XOPEN_UUCP: c_int = 124;
pub const _SC_XOPEN_VERSION: c_int = 125;
pub const _SC_BC_BASE_MAX: c_int = 129;
pub const _SC_BC_DIM_MAX: c_int = 130;
pub const _SC_BC_SCALE_MAX: c_int = 131;
pub const _SC_BC_STRING_MAX: c_int = 132;
pub const _SC_COLL_WEIGHTS_MAX: c_int = 133;
pub const _SC_EXPR_NEST_MAX: c_int = 134;
pub const _SC_LINE_MAX: c_int = 135;
pub const _SC_LOGIN_NAME_MAX: c_int = 136;
pub const _SC_MQ_OPEN_MAX: c_int = 137;
pub const _SC_MQ_PRIO_MAX: c_int = 138;
pub const _SC_THREAD_DESTRUCTOR_ITERATIONS: c_int = 139;
pub const _SC_THREAD_KEYS_MAX: c_int = 140;
pub const _SC_THREAD_THREADS_MAX: c_int = 141;
pub const _SC_RE_DUP_MAX: c_int = 142;

pub const PTHREAD_STACK_MIN: size_t = 8192;

pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
    flags: 0,
    lock: 0,
    unused: -42,
    owner: -1,
    owner_count: 0,
};
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    flags: 0,
    unused: -42,
    mutex: 0 as *mut _,
    waiter_count: 0,
    lock: 0,
};
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = pthread_rwlock_t {
    flags: 0,
    owner: -1,
    lock_sem: 0,
    lock_count: 0,
    reader_count: 0,
    writer_count: 0,
    waiters: [0 as *mut _; 2],
};

pub const PTHREAD_MUTEX_DEFAULT: c_int = 0;
pub const PTHREAD_MUTEX_NORMAL: c_int = 1;
pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 2;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 3;

pub const FIOCLEX: c_ulong = 0; // FIXME(haiku): does not exist on Haiku!

pub const RUSAGE_CHILDREN: c_int = -1;

pub const SOCK_STREAM: c_int = 1;
pub const SOCK_DGRAM: c_int = 2;
pub const SOCK_RAW: c_int = 3;
pub const SOCK_SEQPACKET: c_int = 5;

pub const SOL_SOCKET: c_int = -1;
pub const SO_ACCEPTCONN: c_int = 0x00000001;
pub const SO_BROADCAST: c_int = 0x00000002;
pub const SO_DEBUG: c_int = 0x00000004;
pub const SO_DONTROUTE: c_int = 0x00000008;
pub const SO_KEEPALIVE: c_int = 0x00000010;
pub const SO_OOBINLINE: c_int = 0x00000020;
pub const SO_REUSEADDR: c_int = 0x00000040;
pub const SO_REUSEPORT: c_int = 0x00000080;
pub const SO_USELOOPBACK: c_int = 0x00000100;
pub const SO_LINGER: c_int = 0x00000200;
pub const SO_SNDBUF: c_int = 0x40000001;
pub const SO_SNDLOWAT: c_int = 0x40000002;
pub const SO_SNDTIMEO: c_int = 0x40000003;
pub const SO_RCVBUF: c_int = 0x40000004;
pub const SO_RCVLOWAT: c_int = 0x40000005;
pub const SO_RCVTIMEO: c_int = 0x40000006;
pub const SO_ERROR: c_int = 0x40000007;
pub const SO_TYPE: c_int = 0x40000008;
pub const SO_NONBLOCK: c_int = 0x40000009;
pub const SO_BINDTODEVICE: c_int = 0x4000000a;
pub const SO_PEERCRED: c_int = 0x4000000b;

pub const SCM_RIGHTS: c_int = 0x01;

pub const SOMAXCONN: c_int = 32;

pub const NI_MAXHOST: size_t = 1025;

pub const WNOHANG: c_int = 0x01;
pub const WUNTRACED: c_int = 0x02;
pub const WCONTINUED: c_int = 0x04;
pub const WEXITED: c_int = 0x08;
pub const WSTOPPED: c_int = 0x10;
pub const WNOWAIT: c_int = 0x20;

// si_code values for SIGBUS signal
pub const BUS_ADRALN: c_int = 40;
pub const BUS_ADRERR: c_int = 41;
pub const BUS_OBJERR: c_int = 42;

// si_code values for SIGCHLD signal
pub const CLD_EXITED: c_int = 60;
pub const CLD_KILLED: c_int = 61;
pub const CLD_DUMPED: c_int = 62;
pub const CLD_TRAPPED: c_int = 63;
pub const CLD_STOPPED: c_int = 64;
pub const CLD_CONTINUED: c_int = 65;

pub const P_ALL: idtype_t = 0;
pub const P_PID: idtype_t = 1;
pub const P_PGID: idtype_t = 2;

pub const UTIME_OMIT: c_long = 1000000001;
pub const UTIME_NOW: c_long = 1000000000;

pub const VINTR: usize = 0;
pub const VQUIT: usize = 1;
pub const VERASE: usize = 2;
pub const VKILL: usize = 3;
pub const VEOF: usize = 4;
pub const VEOL: usize = 5;
pub const VMIN: usize = 4;
pub const VTIME: usize = 5;
pub const VEOL2: usize = 6;
pub const VSWTCH: usize = 7;
pub const VSTART: usize = 8;
pub const VSTOP: usize = 9;
pub const VSUSP: usize = 10;

pub const IGNBRK: crate::tcflag_t = 0x01;
pub const BRKINT: crate::tcflag_t = 0x02;
pub const IGNPAR: crate::tcflag_t = 0x04;
pub const PARMRK: crate::tcflag_t = 0x08;
pub const INPCK: crate::tcflag_t = 0x10;
pub const ISTRIP: crate::tcflag_t = 0x20;
pub const INLCR: crate::tcflag_t = 0x40;
pub const IGNCR: crate::tcflag_t = 0x80;
pub const ICRNL: crate::tcflag_t = 0x100;
pub const IUCLC: crate::tcflag_t = 0x200;
pub const IXON: crate::tcflag_t = 0x400;
pub const IXANY: crate::tcflag_t = 0x800;
pub const IXOFF: crate::tcflag_t = 0x1000;

pub const OPOST: crate::tcflag_t = 0x00000001;
pub const OLCUC: crate::tcflag_t = 0x00000002;
pub const ONLCR: crate::tcflag_t = 0x00000004;
pub const OCRNL: crate::tcflag_t = 0x00000008;
pub const ONOCR: crate::tcflag_t = 0x00000010;
pub const ONLRET: crate::tcflag_t = 0x00000020;
pub const OFILL: crate::tcflag_t = 0x00000040;
pub const OFDEL: crate::tcflag_t = 0x00000080;
pub const NLDLY: crate::tcflag_t = 0x00000100;
pub const NL0: crate::tcflag_t = 0x00000000;
pub const NL1: crate::tcflag_t = 0x00000100;
pub const CRDLY: crate::tcflag_t = 0x00000600;
pub const CR0: crate::tcflag_t = 0x00000000;
pub const CR1: crate::tcflag_t = 0x00000200;
pub const CR2: crate::tcflag_t = 0x00000400;
pub const CR3: crate::tcflag_t = 0x00000600;
pub const TABDLY: crate::tcflag_t = 0x00001800;
pub const TAB0: crate::tcflag_t = 0x00000000;
pub const TAB1: crate::tcflag_t = 0x00000800;
pub const TAB2: crate::tcflag_t = 0x00001000;
pub const TAB3: crate::tcflag_t = 0x00001800;
pub const BSDLY: crate::tcflag_t = 0x00002000;
pub const BS0: crate::tcflag_t = 0x00000000;
pub const BS1: crate::tcflag_t = 0x00002000;
pub const VTDLY: crate::tcflag_t = 0x00004000;
pub const VT0: crate::tcflag_t = 0x00000000;
pub const VT1: crate::tcflag_t = 0x00004000;
pub const FFDLY: crate::tcflag_t = 0x00008000;
pub const FF0: crate::tcflag_t = 0x00000000;
pub const FF1: crate::tcflag_t = 0x00008000;

pub const CSIZE: crate::tcflag_t = 0x00000020;
pub const CS5: crate::tcflag_t = 0x00000000;
pub const CS6: crate::tcflag_t = 0x00000000;
pub const CS7: crate::tcflag_t = 0x00000000;
pub const CS8: crate::tcflag_t = 0x00000020;
pub const CSTOPB: crate::tcflag_t = 0x00000040;
pub const CREAD: crate::tcflag_t = 0x00000080;
pub const PARENB: crate::tcflag_t = 0x00000100;
pub const PARODD: crate::tcflag_t = 0x00000200;
pub const HUPCL: crate::tcflag_t = 0x00000400;
pub const CLOCAL: crate::tcflag_t = 0x00000800;
pub const XLOBLK: crate::tcflag_t = 0x00001000;
pub const CTSFLOW: crate::tcflag_t = 0x00002000;
pub const RTSFLOW: crate::tcflag_t = 0x00004000;
pub const CRTSCTS: crate::tcflag_t = RTSFLOW | CTSFLOW;

pub const ISIG: crate::tcflag_t = 0x00000001;
pub const ICANON: crate::tcflag_t = 0x00000002;
pub const XCASE: crate::tcflag_t = 0x00000004;
pub const ECHO: crate::tcflag_t = 0x00000008;
pub const ECHOE: crate::tcflag_t = 0x00000010;
pub const ECHOK: crate::tcflag_t = 0x00000020;
pub const ECHONL: crate::tcflag_t = 0x00000040;
pub const NOFLSH: crate::tcflag_t = 0x00000080;
pub const TOSTOP: crate::tcflag_t = 0x00000100;
pub const IEXTEN: crate::tcflag_t = 0x00000200;
pub const ECHOCTL: crate::tcflag_t = 0x00000400;
pub const ECHOPRT: crate::tcflag_t = 0x00000800;
pub const ECHOKE: crate::tcflag_t = 0x00001000;
pub const FLUSHO: crate::tcflag_t = 0x00002000;
pub const PENDIN: crate::tcflag_t = 0x00004000;

pub const TCGB_CTS: c_int = 0x01;
pub const TCGB_DSR: c_int = 0x02;
pub const TCGB_RI: c_int = 0x04;
pub const TCGB_DCD: c_int = 0x08;
pub const TIOCM_CTS: c_int = TCGB_CTS;
pub const TIOCM_CD: c_int = TCGB_DCD;
pub const TIOCM_CAR: c_int = TCGB_DCD;
pub const TIOCM_RI: c_int = TCGB_RI;
pub const TIOCM_RNG: c_int = TCGB_RI;
pub const TIOCM_DSR: c_int = TCGB_DSR;
pub const TIOCM_DTR: c_int = 0x10;
pub const TIOCM_RTS: c_int = 0x20;

pub const B0: speed_t = 0x00;
pub const B50: speed_t = 0x01;
pub const B75: speed_t = 0x02;
pub const B110: speed_t = 0x03;
pub const B134: speed_t = 0x04;
pub const B150: speed_t = 0x05;
pub const B200: speed_t = 0x06;
pub const B300: speed_t = 0x07;
pub const B600: speed_t = 0x08;
pub const B1200: speed_t = 0x09;
pub const B1800: speed_t = 0x0A;
pub const B2400: speed_t = 0x0B;
pub const B4800: speed_t = 0x0C;
pub const B9600: speed_t = 0x0D;
pub const B19200: speed_t = 0x0E;
pub const B38400: speed_t = 0x0F;
pub const B57600: speed_t = 0x10;
pub const B115200: speed_t = 0x11;
pub const B230400: speed_t = 0x12;
pub const B31250: speed_t = 0x13;

pub const TCSANOW: c_int = 0x01;
pub const TCSADRAIN: c_int = 0x02;
pub const TCSAFLUSH: c_int = 0x04;

pub const TCOOFF: c_int = 0x01;
pub const TCOON: c_int = 0x02;
pub const TCIOFF: c_int = 0x04;
pub const TCION: c_int = 0x08;

pub const TCIFLUSH: c_int = 0x01;
pub const TCOFLUSH: c_int = 0x02;
pub const TCIOFLUSH: c_int = 0x03;

pub const TCGETA: c_ulong = 0x8000;
pub const TCSETA: c_ulong = TCGETA + 1;
pub const TCSETAF: c_ulong = TCGETA + 2;
pub const TCSETAW: c_ulong = TCGETA + 3;
pub const TCSBRK: c_ulong = TCGETA + 5;
pub const TCFLSH: c_ulong = TCGETA + 6;
pub const TCXONC: c_ulong = TCGETA + 7;
pub const TCGETBITS: c_ulong = TCGETA + 9;
pub const TCSETDTR: c_ulong = TCGETA + 10;
pub const TCSETRTS: c_ulong = TCGETA + 11;
pub const TIOCGWINSZ: c_ulong = TCGETA + 12;
pub const TIOCSWINSZ: c_ulong = TCGETA + 13;
pub const TIOCGPGRP: c_ulong = TCGETA + 15;
pub const TIOCSPGRP: c_ulong = TCGETA + 16;
pub const TIOCSCTTY: c_ulong = TCGETA + 17;
pub const TIOCMGET: c_ulong = TCGETA + 18;
pub const TIOCMSET: c_ulong = TCGETA + 19;
pub const TIOCSBRK: c_ulong = TCGETA + 20;
pub const TIOCCBRK: c_ulong = TCGETA + 21;
pub const TIOCMBIS: c_ulong = TCGETA + 22;
pub const TIOCMBIC: c_ulong = TCGETA + 23;
pub const TIOCGSID: c_ulong = TCGETA + 24;
pub const TIOCOUTQ: c_ulong = TCGETA + 25;
pub const TIOCEXCL: c_ulong = TCGETA + 26;
pub const TIOCNXCL: c_ulong = TCGETA + 27;

pub const PRIO_PROCESS: c_int = 0;
pub const PRIO_PGRP: c_int = 1;
pub const PRIO_USER: c_int = 2;

// utmpx entry types
pub const EMPTY: c_short = 0;
pub const BOOT_TIME: c_short = 1;
pub const OLD_TIME: c_short = 2;
pub const NEW_TIME: c_short = 3;
pub const USER_PROCESS: c_short = 4;
pub const INIT_PROCESS: c_short = 5;
pub const LOGIN_PROCESS: c_short = 6;
pub const DEAD_PROCESS: c_short = 7;

pub const LOG_PID: c_int = 1 << 12;
pub const LOG_CONS: c_int = 2 << 12;
pub const LOG_ODELAY: c_int = 4 << 12;
pub const LOG_NDELAY: c_int = 8 << 12;
pub const LOG_SERIAL: c_int = 16 << 12;
pub const LOG_PERROR: c_int = 32 << 12;
pub const LOG_NOWAIT: c_int = 64 << 12;

// spawn.h
// DIFF(main): changed to `c_short` in f62eb023ab
pub const POSIX_SPAWN_RESETIDS: c_int = 0x01;
pub const POSIX_SPAWN_SETPGROUP: c_int = 0x02;
pub const POSIX_SPAWN_SETSIGDEF: c_int = 0x10;
pub const POSIX_SPAWN_SETSIGMASK: c_int = 0x20;
pub const POSIX_SPAWN_SETSID: c_int = 0x40;

const_fn! {
    {const} fn CMSG_ALIGN(len: usize) -> usize {
        len + size_of::<usize>() - 1 & !(size_of::<usize>() - 1)
    }
}

f! {
    pub fn CMSG_FIRSTHDR(mhdr: *const msghdr) -> *mut cmsghdr {
        if (*mhdr).msg_controllen as usize >= size_of::<cmsghdr>() {
            (*mhdr).msg_control as *mut cmsghdr
        } else {
            core::ptr::null_mut::<cmsghdr>()
        }
    }

    pub fn CMSG_DATA(cmsg: *const cmsghdr) -> *mut c_uchar {
        (cmsg as *mut c_uchar).offset(CMSG_ALIGN(size_of::<cmsghdr>()) as isize)
    }

    pub {const} fn CMSG_SPACE(length: c_uint) -> c_uint {
        (CMSG_ALIGN(length as usize) + CMSG_ALIGN(size_of::<cmsghdr>())) as c_uint
    }

    pub {const} fn CMSG_LEN(length: c_uint) -> c_uint {
        CMSG_ALIGN(size_of::<cmsghdr>()) as c_uint + length
    }

    pub fn CMSG_NXTHDR(mhdr: *const msghdr, cmsg: *const cmsghdr) -> *mut cmsghdr {
        if cmsg.is_null() {
            return crate::CMSG_FIRSTHDR(mhdr);
        }
        let next = cmsg as usize
            + CMSG_ALIGN((*cmsg).cmsg_len as usize)
            + CMSG_ALIGN(size_of::<cmsghdr>());
        let max = (*mhdr).msg_control as usize + (*mhdr).msg_controllen as usize;
        if next > max {
            core::ptr::null_mut::<cmsghdr>()
        } else {
            (cmsg as usize + CMSG_ALIGN((*cmsg).cmsg_len as usize)) as *mut cmsghdr
        }
    }

    pub fn FD_CLR(fd: c_int, set: *mut fd_set) -> () {
        let fd = fd as usize;
        let size = size_of_val(&(*set).fds_bits[0]) * 8;
        (*set).fds_bits[fd / size] &= !(1 << (fd % size));
        return;
    }

    pub fn FD_ISSET(fd: c_int, set: *const fd_set) -> bool {
        let fd = fd as usize;
        let size = size_of_val(&(*set).fds_bits[0]) * 8;
        return ((*set).fds_bits[fd / size] & (1 << (fd % size))) != 0;
    }

    pub fn FD_SET(fd: c_int, set: *mut fd_set) -> () {
        let fd = fd as usize;
        let size = size_of_val(&(*set).fds_bits[0]) * 8;
        (*set).fds_bits[fd / size] |= 1 << (fd % size);
        return;
    }

    pub fn FD_ZERO(set: *mut fd_set) -> () {
        for slot in (*set).fds_bits.iter_mut() {
            *slot = 0;
        }
    }
}

safe_f! {
    pub {const} fn WIFEXITED(status: c_int) -> bool {
        (status & !0xff) == 0
    }

    pub {const} fn WEXITSTATUS(status: c_int) -> c_int {
        status & 0xff
    }

    pub {const} fn WIFSIGNALED(status: c_int) -> bool {
        ((status >> 8) & 0xff) != 0
    }

    pub {const} fn WTERMSIG(status: c_int) -> c_int {
        (status >> 8) & 0xff
    }

    pub {const} fn WIFSTOPPED(status: c_int) -> bool {
        ((status >> 16) & 0xff) != 0
    }

    pub {const} fn WSTOPSIG(status: c_int) -> c_int {
        (status >> 16) & 0xff
    }

    // actually WIFCORED, but this is used everywhere else
    pub {const} fn WCOREDUMP(status: c_int) -> bool {
        (status & 0x10000) != 0
    }

    pub {const} fn WIFCONTINUED(status: c_int) -> bool {
        (status & 0x20000) != 0
    }
}

extern "C" {
    pub fn getrlimit(resource: c_int, rlim: *mut crate::rlimit) -> c_int;
    pub fn setrlimit(resource: c_int, rlim: *const crate::rlimit) -> c_int;
    pub fn getpriority(which: c_int, who: id_t) -> c_int;
    pub fn setpriority(which: c_int, who: id_t, priority: c_int) -> c_int;

    pub fn endusershell();
    pub fn getpass(prompt: *const c_char) -> *mut c_char;
    pub fn getusershell() -> *mut c_char;
    pub fn issetugid() -> c_int;
    pub fn setusershell();

    pub fn utimensat(
        fd: c_int,
        path: *const c_char,
        times: *const crate::timespec,
        flag: c_int,
    ) -> c_int;
    pub fn futimens(fd: c_int, times: *const crate::timespec) -> c_int;
    pub fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;
    pub fn _errnop() -> *mut c_int;

    pub fn abs(i: c_int) -> c_int;
    pub fn labs(i: c_long) -> c_long;
    pub fn rand() -> c_int;
    pub fn srand(seed: c_uint);
    pub fn getifaddrs(ifap: *mut *mut crate::ifaddrs) -> c_int;
    pub fn freeifaddrs(ifa: *mut crate::ifaddrs);
    pub fn ppoll(
        fds: *mut crate::pollfd,
        numfds: crate::nfds_t,
        timeout: *const crate::timespec,
        sigMask: *const sigset_t,
    ) -> c_int;

    pub fn getspent() -> *mut spwd;
    pub fn getspent_r(
        pwd: *mut spwd,
        buf: *mut c_char,
        bufferSize: size_t,
        res: *mut *mut spwd,
    ) -> c_int;
    pub fn setspent();
    pub fn endspent();
    pub fn getspnam(name: *const c_char) -> *mut spwd;
    pub fn getspnam_r(
        name: *const c_char,
        spwd: *mut spwd,
        buffer: *mut c_char,
        bufferSize: size_t,
        res: *mut *mut spwd,
    ) -> c_int;
    pub fn sgetspent(line: *const c_char) -> *mut spwd;
    pub fn sgetspent_r(
        line: *const c_char,
        spwd: *mut spwd,
        buffer: *mut c_char,
        bufferSize: size_t,
        res: *mut *mut spwd,
    ) -> c_int;
    pub fn fgetspent(file: *mut crate::FILE) -> *mut spwd;
    pub fn fgetspent_r(
        file: *mut crate::FILE,
        spwd: *mut spwd,
        buffer: *mut c_char,
        bufferSize: size_t,
        res: *mut *mut spwd,
    ) -> c_int;
    pub fn mkfifoat(dirfd: c_int, pathname: *const c_char, mode: mode_t) -> c_int;
    pub fn mknodat(dirfd: c_int, pathname: *const c_char, mode: mode_t, dev: dev_t) -> c_int;
    pub fn sem_destroy(sem: *mut sem_t) -> c_int;
    pub fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;

    pub fn clock_getres(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn clock_gettime(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn clock_settime(clk_id: crate::clockid_t, tp: *const crate::timespec) -> c_int;
    pub fn clock_getcpuclockid(pid: crate::pid_t, clk_id: *mut crate::clockid_t) -> c_int;
    pub fn pthread_create(
        thread: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        f: extern "C" fn(*mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;
    pub fn pthread_attr_getguardsize(
        attr: *const crate::pthread_attr_t,
        guardsize: *mut size_t,
    ) -> c_int;
    pub fn pthread_attr_setguardsize(attr: *mut crate::pthread_attr_t, guardsize: size_t) -> c_int;
    pub fn pthread_attr_getstack(
        attr: *const crate::pthread_attr_t,
        stackaddr: *mut *mut c_void,
        stacksize: *mut size_t,
    ) -> c_int;
    pub fn pthread_condattr_getclock(
        attr: *const pthread_condattr_t,
        clock_id: *mut clockid_t,
    ) -> c_int;
    pub fn pthread_condattr_setclock(
        attr: *mut pthread_condattr_t,
        clock_id: crate::clockid_t,
    ) -> c_int;
    pub fn valloc(numBytes: size_t) -> *mut c_void;
    pub fn malloc_usable_size(ptr: *mut c_void) -> size_t;
    pub fn memalign(align: size_t, size: size_t) -> *mut c_void;
    pub fn setgroups(ngroups: c_int, ptr: *const crate::gid_t) -> c_int;
    pub fn initgroups(name: *const c_char, basegid: crate::gid_t) -> c_int;
    pub fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;
    pub fn dirfd(dirp: *mut crate::DIR) -> c_int;
    pub fn getnameinfo(
        sa: *const crate::sockaddr,
        salen: crate::socklen_t,
        host: *mut c_char,
        hostlen: crate::socklen_t,
        serv: *mut c_char,
        servlen: crate::socklen_t,
        flags: c_int,
    ) -> c_int;
    pub fn pthread_mutex_timedlock(
        lock: *mut pthread_mutex_t,
        abstime: *const crate::timespec,
    ) -> c_int;
    pub fn pthread_sigqueue(thread: crate::pthread_t, sig: c_int, value: crate::sigval) -> c_int;
    pub fn pthread_spin_init(lock: *mut crate::pthread_spinlock_t, pshared: c_int) -> c_int;
    pub fn pthread_spin_destroy(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_lock(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_trylock(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_unlock(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn waitid(
        idtype: idtype_t,
        id: id_t,
        infop: *mut crate::siginfo_t,
        options: c_int,
    ) -> c_int;

    pub fn glob(
        pattern: *const c_char,
        flags: c_int,
        errfunc: Option<extern "C" fn(epath: *const c_char, errno: c_int) -> c_int>,
        pglob: *mut crate::glob_t,
    ) -> c_int;
    pub fn globfree(pglob: *mut crate::glob_t);
    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut c_void) -> c_int;
    pub fn posix_madvise(addr: *mut c_void, len: size_t, advice: c_int) -> c_int;
    pub fn posix_fadvise(fd: c_int, offset: off_t, len: off_t, advice: c_int) -> c_int;
    pub fn posix_fallocate(fd: c_int, offset: off_t, len: off_t) -> c_int;

    pub fn shm_open(name: *const c_char, oflag: c_int, mode: mode_t) -> c_int;
    pub fn shm_unlink(name: *const c_char) -> c_int;

    pub fn seekdir(dirp: *mut crate::DIR, loc: c_long);

    pub fn telldir(dirp: *mut crate::DIR) -> c_long;
    pub fn madvise(addr: *mut c_void, len: size_t, advice: c_int) -> c_int;

    pub fn msync(addr: *mut c_void, len: size_t, flags: c_int) -> c_int;

    pub fn recvfrom(
        socket: c_int,
        buf: *mut c_void,
        len: size_t,
        flags: c_int,
        addr: *mut crate::sockaddr,
        addrlen: *mut crate::socklen_t,
    ) -> ssize_t;
    pub fn nl_langinfo(item: crate::nl_item) -> *mut c_char;

    pub fn bind(
        socket: c_int,
        address: *const crate::sockaddr,
        address_len: crate::socklen_t,
    ) -> c_int;

    pub fn writev(fd: c_int, iov: *const crate::iovec, count: c_int) -> ssize_t;
    pub fn readv(fd: c_int, iov: *const crate::iovec, count: c_int) -> ssize_t;

    pub fn sendmsg(fd: c_int, msg: *const crate::msghdr, flags: c_int) -> ssize_t;
    pub fn recvmsg(fd: c_int, msg: *mut crate::msghdr, flags: c_int) -> ssize_t;

    // DIFF(main): changed to `*const *mut` in e77f551de9
    pub fn execvpe(
        file: *const c_char,
        argv: *const *const c_char,
        environment: *const *const c_char,
    ) -> c_int;

    pub fn getgrgid_r(
        gid: crate::gid_t,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn getgrouplist(
        user: *const c_char,
        basegroup: crate::gid_t,
        grouplist: *mut crate::gid_t,
        groupcount: *mut c_int,
    ) -> c_int;
    pub fn sigaltstack(ss: *const stack_t, oss: *mut stack_t) -> c_int;
    pub fn sigsuspend(mask: *const crate::sigset_t) -> c_int;
    pub fn sem_close(sem: *mut sem_t) -> c_int;
    pub fn getdtablesize() -> c_int;
    pub fn getgrnam_r(
        name: *const c_char,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn pthread_sigmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t) -> c_int;
    pub fn sem_open(name: *const c_char, oflag: c_int, ...) -> *mut sem_t;
    pub fn getgrnam(name: *const c_char) -> *mut crate::group;
    pub fn pthread_kill(thread: crate::pthread_t, sig: c_int) -> c_int;
    pub fn sem_unlink(name: *const c_char) -> c_int;
    pub fn getpwnam_r(
        name: *const c_char,
        pwd: *mut passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut passwd,
    ) -> c_int;
    pub fn getpwuid_r(
        uid: crate::uid_t,
        pwd: *mut passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut passwd,
    ) -> c_int;
    pub fn getpwent() -> *mut passwd;
    pub fn setpwent();
    pub fn endpwent();
    pub fn endgrent();
    pub fn getgrent() -> *mut crate::group;
    pub fn setgrent();
    pub fn sigwait(set: *const sigset_t, sig: *mut c_int) -> c_int;
    pub fn pthread_atfork(
        prepare: Option<unsafe extern "C" fn()>,
        parent: Option<unsafe extern "C" fn()>,
        child: Option<unsafe extern "C" fn()>,
    ) -> c_int;
    pub fn getgrgid(gid: crate::gid_t) -> *mut crate::group;
    pub fn popen(command: *const c_char, mode: *const c_char) -> *mut crate::FILE;
    pub fn sethostname(name: *const c_char, len: size_t) -> c_int;
    pub fn uname(buf: *mut crate::utsname) -> c_int;
    pub fn getutxent() -> *mut utmpx;
    pub fn getutxid(ut: *const utmpx) -> *mut utmpx;
    pub fn getutxline(ut: *const utmpx) -> *mut utmpx;
    pub fn pututxline(ut: *const utmpx) -> *mut utmpx;
    pub fn setutxent();
    pub fn endutxent();
    pub fn faccessat(dirfd: c_int, pathname: *const c_char, mode: c_int, flags: c_int) -> c_int;

    pub fn sigtimedwait(
        set: *const sigset_t,
        info: *mut siginfo_t,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn sigwaitinfo(set: *const sigset_t, info: *mut siginfo_t) -> c_int;

    pub fn getitimer(which: c_int, curr_value: *mut crate::itimerval) -> c_int;
    pub fn setitimer(
        which: c_int,
        new_value: *const crate::itimerval,
        old_value: *mut crate::itimerval,
    ) -> c_int;

    pub fn regcomp(preg: *mut regex_t, pattern: *const c_char, cflags: c_int) -> c_int;

    pub fn regexec(
        preg: *const regex_t,
        input: *const c_char,
        nmatch: size_t,
        pmatch: *mut regmatch_t,
        eflags: c_int,
    ) -> c_int;

    pub fn regerror(
        errcode: c_int,
        preg: *const regex_t,
        errbuf: *mut c_char,
        errbuf_size: size_t,
    ) -> size_t;

    pub fn regfree(preg: *mut regex_t);

    pub fn msgctl(msqid: c_int, cmd: c_int, buf: *mut msqid_ds) -> c_int;
    pub fn msgget(key: crate::key_t, msgflg: c_int) -> c_int;
    pub fn msgrcv(
        msqid: c_int,
        msgp: *mut c_void,
        msgsz: size_t,
        msgtype: c_long,
        msgflg: c_int,
    ) -> ssize_t;
    pub fn msgsnd(msqid: c_int, msgp: *const c_void, msgsz: size_t, msgflg: c_int) -> c_int;
    pub fn semget(key: crate::key_t, nsems: c_int, semflg: c_int) -> c_int;
    pub fn semctl(semid: c_int, semnum: c_int, cmd: c_int, ...) -> c_int;
    pub fn semop(semid: c_int, sops: *mut sembuf, nsops: size_t) -> c_int;
    pub fn ftok(pathname: *const c_char, proj_id: c_int) -> crate::key_t;

    pub fn memrchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;

    pub fn lsearch(
        key: *const c_void,
        base: *mut c_void,
        nelp: *mut size_t,
        width: size_t,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>,
    ) -> *mut c_void;
    pub fn lfind(
        key: *const c_void,
        base: *const c_void,
        nelp: *mut size_t,
        width: size_t,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>,
    ) -> *mut c_void;
    pub fn hcreate(nelt: size_t) -> c_int;
    pub fn hdestroy();
    pub fn hsearch(entry: crate::ENTRY, action: crate::ACTION) -> *mut crate::ENTRY;

    pub fn drand48() -> c_double;
    pub fn erand48(xseed: *mut c_ushort) -> c_double;
    pub fn lrand48() -> c_long;
    pub fn nrand48(xseed: *mut c_ushort) -> c_long;
    pub fn mrand48() -> c_long;
    pub fn jrand48(xseed: *mut c_ushort) -> c_long;
    pub fn srand48(seed: c_long);
    pub fn seed48(xseed: *mut c_ushort) -> *mut c_ushort;
    pub fn lcong48(p: *mut c_ushort);

    pub fn clearenv() -> c_int;
    pub fn ctermid(s: *mut c_char) -> *mut c_char;

    pub fn sync();
    pub fn getpagesize() -> c_int;

    pub fn brk(addr: *mut c_void) -> c_int;
    pub fn sbrk(increment: intptr_t) -> *mut c_void;

    pub fn posix_spawn(
        pid: *mut crate::pid_t,
        path: *const c_char,
        file_actions: *const crate::posix_spawn_file_actions_t,
        attrp: *const crate::posix_spawnattr_t,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
    ) -> c_int;
    pub fn posix_spawnp(
        pid: *mut crate::pid_t,
        file: *const c_char,
        file_actions: *const crate::posix_spawn_file_actions_t,
        attrp: *const crate::posix_spawnattr_t,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
    ) -> c_int;

    pub fn posix_spawn_file_actions_init(file_actions: *mut posix_spawn_file_actions_t) -> c_int;
    pub fn posix_spawn_file_actions_destroy(file_actions: *mut posix_spawn_file_actions_t)
        -> c_int;
    pub fn posix_spawn_file_actions_addopen(
        file_actions: *mut posix_spawn_file_actions_t,
        fildes: c_int,
        path: *const c_char,
        oflag: c_int,
        mode: mode_t,
    ) -> c_int;
    pub fn posix_spawn_file_actions_addclose(
        file_actions: *mut posix_spawn_file_actions_t,
        fildes: c_int,
    ) -> c_int;
    pub fn posix_spawn_file_actions_adddup2(
        file_actions: *mut posix_spawn_file_actions_t,
        fildes: c_int,
        newfildes: c_int,
    ) -> c_int;

    pub fn posix_spawnattr_init(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_destroy(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_getflags(attr: *const posix_spawnattr_t, _flags: *mut c_short) -> c_int;
    pub fn posix_spawnattr_setflags(attr: *mut posix_spawnattr_t, flags: c_short) -> c_int;
    pub fn posix_spawnattr_getpgroup(
        attr: *const posix_spawnattr_t,
        _pgroup: *mut crate::pid_t,
    ) -> c_int;
    pub fn posix_spawnattr_setpgroup(attr: *mut posix_spawnattr_t, pgroup: crate::pid_t) -> c_int;
    pub fn posix_spawnattr_getsigdefault(
        attr: *const posix_spawnattr_t,
        sigdefault: *mut crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_setsigdefault(
        attr: *mut posix_spawnattr_t,
        sigdefault: *const crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_getsigmask(
        attr: *const posix_spawnattr_t,
        _sigmask: *mut crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_setsigmask(
        attr: *mut posix_spawnattr_t,
        sigmask: *const crate::sigset_t,
    ) -> c_int;
    pub fn getopt_long(
        argc: c_int,
        argv: *const *mut c_char,
        optstring: *const c_char,
        longopts: *const option,
        longindex: *mut c_int,
    ) -> c_int;
    pub fn strcasecmp_l(
        string1: *const c_char,
        string2: *const c_char,
        locale: crate::locale_t,
    ) -> c_int;
    pub fn strncasecmp_l(
        string1: *const c_char,
        string2: *const c_char,
        length: size_t,
        locale: crate::locale_t,
    ) -> c_int;

    pub fn getentropy(buf: *mut c_void, buflen: size_t) -> c_int;
}

#[link(name = "gnu")]
extern "C" {
    pub fn memmem(
        source: *const c_void,
        sourceLength: size_t,
        search: *const c_void,
        searchLength: size_t,
    ) -> *mut c_void;

    pub fn pthread_getattr_np(thread: crate::pthread_t, attr: *mut crate::pthread_attr_t) -> c_int;
    pub fn pthread_getname_np(
        thread: crate::pthread_t,
        buffer: *mut c_char,
        length: size_t,
    ) -> c_int;
    pub fn pthread_setname_np(thread: crate::pthread_t, name: *const c_char) -> c_int;
}

cfg_if! {
    if #[cfg(target_pointer_width = "64")] {
        mod b64;
        pub use self::b64::*;
    } else {
        mod b32;
        pub use self::b32::*;
    }
}

cfg_if! {
    if #[cfg(target_arch = "x86")] {
        // TODO
        // mod x86;
        // pub use self::x86::*;
    } else if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(target_arch = "aarch64")] {
        // TODO
        // mod aarch64;
        // pub use self::aarch64::*;
    }
}

mod bsd;
pub use self::bsd::*;

mod native;
pub use self::native::*;
