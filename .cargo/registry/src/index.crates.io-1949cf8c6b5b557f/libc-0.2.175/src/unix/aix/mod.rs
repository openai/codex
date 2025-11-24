use crate::prelude::*;
use crate::{in_addr_t, in_port_t};

pub type caddr_t = *mut c_char;
pub type clockid_t = c_longlong;
pub type blkcnt_t = c_long;
pub type clock_t = c_int;
pub type daddr_t = c_long;
pub type dev_t = c_ulong;
pub type fpos64_t = c_longlong;
pub type fsblkcnt_t = c_ulong;
pub type fsfilcnt_t = c_ulong;
pub type ino_t = c_ulong;
pub type key_t = c_int;
pub type mode_t = c_uint;
pub type nlink_t = c_short;
pub type rlim_t = c_ulong;
pub type speed_t = c_uint;
pub type tcflag_t = c_uint;
pub type time_t = c_long;
pub type time64_t = i64;
pub type timer_t = c_long;
pub type wchar_t = c_uint;
pub type nfds_t = c_uint;
pub type projid_t = c_int;
pub type id_t = c_uint;
pub type blksize64_t = c_ulonglong;
pub type blkcnt64_t = c_ulonglong;
pub type suseconds_t = c_int;
pub type useconds_t = c_uint;
pub type off_t = c_long;
pub type offset_t = c_longlong;
pub type off64_t = c_longlong;
pub type idtype_t = c_uint;

pub type socklen_t = c_uint;
pub type sa_family_t = c_uchar;

pub type signal_t = c_int;
pub type pthread_t = c_uint;
pub type pthread_key_t = c_uint;
pub type thread_t = pthread_t;
pub type blksize_t = c_long;
pub type nl_item = c_int;
pub type mqd_t = c_int;
pub type shmatt_t = c_ulong;
pub type regoff_t = c_long;
pub type rlim64_t = c_ulonglong;

pub type sem_t = c_int;
pub type pollset_t = c_int;

pub type pthread_rwlockattr_t = *mut c_void;
pub type pthread_condattr_t = *mut c_void;
pub type pthread_mutexattr_t = *mut c_void;
pub type pthread_attr_t = *mut c_void;
pub type pthread_barrierattr_t = *mut c_void;
pub type posix_spawn_file_actions_t = *mut c_char;
pub type iconv_t = *mut c_void;

e! {
    #[repr(u32)]
    pub enum uio_rw {
        UIO_READ = 0,
        UIO_WRITE,
        UIO_READ_NO_MOVE,
        UIO_WRITE_NO_MOVE,
        UIO_PWRITE,
    }
    #[repr(u32)]
    pub enum ACTION {
        FIND = 0,
        ENTER,
    }
}

s! {
    pub struct fsid_t {
        pub val: [c_uint; 2],
    }

    pub struct fsid64_t {
        pub val: [crate::uint64_t; 2],
    }

    pub struct timezone {
        pub tz_minuteswest: c_int,
        pub tz_dsttime: c_int,
    }

    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct dirent {
        pub d_offset: c_ulong,
        pub d_ino: crate::ino_t,
        pub d_reclen: c_ushort,
        pub d_namlen: c_ushort,
        pub d_name: [c_char; 256],
    }

    pub struct termios {
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_cc: [crate::cc_t; crate::NCCS],
    }

    pub struct flock64 {
        pub l_type: c_short,
        pub l_whence: c_short,
        pub l_sysid: c_uint,
        pub l_pid: crate::pid_t,
        pub l_vfs: c_int,
        pub l_start: off64_t,
        pub l_len: off64_t,
    }

    pub struct msghdr {
        pub msg_name: *mut c_void,
        pub msg_namelen: crate::socklen_t,
        pub msg_iov: *mut crate::iovec,
        pub msg_iovlen: c_int,
        pub msg_control: *mut c_void,
        pub msg_controllen: socklen_t,
        pub msg_flags: c_int,
    }

    pub struct statvfs64 {
        pub f_bsize: crate::blksize64_t,
        pub f_frsize: crate::blksize64_t,
        pub f_blocks: crate::blkcnt64_t,
        pub f_bfree: crate::blkcnt64_t,
        pub f_bavail: crate::blkcnt64_t,
        pub f_files: crate::blkcnt64_t,
        pub f_ffree: crate::blkcnt64_t,
        pub f_favail: crate::blkcnt64_t,
        pub f_fsid: fsid64_t,
        pub f_basetype: [c_char; 16],
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
        pub f_fstr: [c_char; 32],
        pub f_filler: [c_ulong; 16],
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
        pub left_parenthesis: *mut c_char,
        pub right_parenthesis: *mut c_char,
        pub int_p_cs_precedes: c_char,
        pub int_p_sep_by_space: c_char,
        pub int_n_cs_precedes: c_char,
        pub int_n_sep_by_space: c_char,
        pub int_p_sign_posn: c_char,
        pub int_n_sign_posn: c_char,
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
    }

    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        pub ai_addrlen: c_ulong,
        pub ai_canonname: *mut c_char,
        pub ai_addr: *mut crate::sockaddr,
        pub ai_next: *mut addrinfo,
        pub ai_eflags: c_int,
    }

    pub struct in_addr {
        pub s_addr: in_addr_t,
    }

    pub struct ip_mreq_source {
        pub imr_multiaddr: in_addr,
        pub imr_sourceaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct sockaddr {
        pub sa_len: c_uchar,
        pub sa_family: sa_family_t,
        pub sa_data: [c_char; 14],
    }

    pub struct sockaddr_dl {
        pub sdl_len: c_uchar,
        pub sdl_family: c_uchar,
        pub sdl_index: c_ushort,
        pub sdl_type: c_uchar,
        pub sdl_nlen: c_uchar,
        pub sdl_alen: c_uchar,
        pub sdl_slen: c_uchar,
        pub sdl_data: [c_char; 120],
    }

    pub struct sockaddr_in {
        pub sin_len: c_uchar,
        pub sin_family: sa_family_t,
        pub sin_port: in_port_t,
        pub sin_addr: in_addr,
        pub sin_zero: [c_uchar; 8],
    }

    pub struct sockaddr_in6 {
        pub sin6_len: c_uchar,
        pub sin6_family: c_uchar,
        pub sin6_port: crate::uint16_t,
        pub sin6_flowinfo: crate::uint32_t,
        pub sin6_addr: crate::in6_addr,
        pub sin6_scope_id: crate::uint32_t,
    }

    pub struct sockaddr_storage {
        pub __ss_len: c_uchar,
        pub ss_family: sa_family_t,
        __ss_pad1: [c_char; 6],
        __ss_align: crate::int64_t,
        __ss_pad2: [c_char; 1265],
    }

    pub struct sockaddr_un {
        pub sun_len: c_uchar,
        pub sun_family: sa_family_t,
        pub sun_path: [c_char; 1023],
    }

    pub struct st_timespec {
        pub tv_sec: crate::time_t,
        pub tv_nsec: c_int,
    }

    pub struct statfs64 {
        pub f_version: c_int,
        pub f_type: c_int,
        pub f_bsize: blksize64_t,
        pub f_blocks: blkcnt64_t,
        pub f_bfree: blkcnt64_t,
        pub f_bavail: blkcnt64_t,
        pub f_files: crate::uint64_t,
        pub f_ffree: crate::uint64_t,
        pub f_fsid: fsid64_t,
        pub f_vfstype: c_int,
        pub f_fsize: blksize64_t,
        pub f_vfsnumber: c_int,
        pub f_vfsoff: c_int,
        pub f_vfslen: c_int,
        pub f_vfsvers: c_int,
        pub f_fname: [c_char; 32],
        pub f_fpack: [c_char; 32],
        pub f_name_max: c_int,
    }

    pub struct passwd {
        pub pw_name: *mut c_char,
        pub pw_passwd: *mut c_char,
        pub pw_uid: crate::uid_t,
        pub pw_gid: crate::gid_t,
        pub pw_gecos: *mut c_char,
        pub pw_dir: *mut c_char,
        pub pw_shell: *mut c_char,
    }

    pub struct utsname {
        pub sysname: [c_char; 32],
        pub nodename: [c_char; 32],
        pub release: [c_char; 32],
        pub version: [c_char; 32],
        pub machine: [c_char; 32],
    }

    pub struct xutsname {
        pub nid: c_uint,
        pub reserved: c_int,
        pub longnid: c_ulonglong,
    }

    pub struct cmsghdr {
        pub cmsg_len: crate::socklen_t,
        pub cmsg_level: c_int,
        pub cmsg_type: c_int,
    }

    pub struct sigevent {
        pub sigev_value: crate::sigval,
        pub sigev_signo: c_int,
        pub sigev_notify: c_int,
        pub sigev_notify_function: extern "C" fn(val: crate::sigval),
        pub sigev_notify_attributes: *mut pthread_attr_t,
    }

    pub struct osigevent {
        pub sevt_value: *mut c_void,
        pub sevt_signo: signal_t,
    }

    pub struct poll_ctl {
        pub cmd: c_short,
        pub events: c_short,
        pub fd: c_int,
    }

    pub struct sf_parms {
        pub header_data: *mut c_void,
        pub header_length: c_uint,
        pub file_descriptor: c_int,
        pub file_size: crate::uint64_t,
        pub file_offset: crate::uint64_t,
        pub file_bytes: crate::int64_t,
        pub trailer_data: *mut c_void,
        pub trailer_length: c_uint,
        pub bytes_sent: crate::uint64_t,
    }

    pub struct mmsghdr {
        pub msg_hdr: crate::msghdr,
        pub msg_len: c_uint,
    }

    pub struct sched_param {
        pub sched_priority: c_int,
        pub sched_policy: c_int,
        pub sched_reserved: [c_int; 6],
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_size: size_t,
        pub ss_flags: c_int,
        pub __pad: [c_int; 4],
    }

    pub struct posix_spawnattr_t {
        pub posix_attr_flags: c_short,
        pub posix_attr_pgroup: crate::pid_t,
        pub posix_attr_sigmask: crate::sigset_t,
        pub posix_attr_sigdefault: crate::sigset_t,
        pub posix_attr_schedpolicy: c_int,
        pub posix_attr_schedparam: sched_param,
    }

    pub struct glob_t {
        pub gl_pathc: size_t,
        pub gl_pathv: *mut *mut c_char,
        pub gl_offs: size_t,
        pub gl_padr: *mut c_void,
        pub gl_ptx: *mut c_void,
    }

    pub struct mallinfo {
        pub arena: c_ulong,
        pub ordblks: c_int,
        pub smblks: c_int,
        pub hblks: c_int,
        pub hblkhd: c_int,
        pub usmblks: c_ulong,
        pub fsmblks: c_ulong,
        pub uordblks: c_ulong,
        pub fordblks: c_ulong,
        pub keepcost: c_int,
    }

    pub struct exit_status {
        pub e_termination: c_short,
        pub e_exit: c_short,
    }

    pub struct utmp {
        pub ut_user: [c_char; 256],
        pub ut_id: [c_char; 14],
        pub ut_line: [c_char; 64],
        pub ut_pid: crate::pid_t,
        pub ut_type: c_short,
        pub ut_time: time64_t,
        pub ut_exit: exit_status,
        pub ut_host: [c_char; 256],
        pub __dbl_word_pad: c_int,
        pub __reservedA: [c_int; 2],
        pub __reservedV: [c_int; 6],
    }

    pub struct regmatch_t {
        pub rm_so: regoff_t,
        pub rm_eo: regoff_t,
    }

    pub struct regex_t {
        pub re_nsub: size_t,
        pub re_comp: *mut c_void,
        pub re_cflags: c_int,
        pub re_erroff: size_t,
        pub re_len: size_t,
        pub re_ucoll: [crate::wchar_t; 2],
        pub re_lsub: [*mut c_void; 24],
        pub re_esub: [*mut c_void; 24],
        pub re_map: *mut c_uchar,
        pub __maxsub: c_int,
        pub __unused: [*mut c_void; 34],
    }

    pub struct rlimit64 {
        pub rlim_cur: rlim64_t,
        pub rlim_max: rlim64_t,
    }

    pub struct shmid_ds {
        pub shm_perm: ipc_perm,
        pub shm_segsz: size_t,
        pub shm_lpid: crate::pid_t,
        pub shm_cpid: crate::pid_t,
        pub shm_nattch: shmatt_t,
        pub shm_cnattch: shmatt_t,
        pub shm_atime: time_t,
        pub shm_dtime: time_t,
        pub shm_ctime: time_t,
        pub shm_handle: crate::uint32_t,
        pub shm_extshm: c_int,
        pub shm_pagesize: crate::int64_t,
        pub shm_lba: crate::uint64_t,
        pub shm_reserved0: crate::int64_t,
        pub shm_reserved1: crate::int64_t,
    }

    pub struct stat64 {
        pub st_dev: dev_t,
        pub st_ino: ino_t,
        pub st_mode: mode_t,
        pub st_nlink: nlink_t,
        pub st_flag: c_ushort,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: dev_t,
        pub st_ssize: c_int,
        pub st_atim: crate::timespec,
        pub st_mtim: crate::timespec,
        pub st_ctim: crate::timespec,
        pub st_blksize: blksize_t,
        pub st_blocks: blkcnt_t,
        pub st_vfstype: c_int,
        pub st_vfs: c_uint,
        pub st_type: c_uint,
        pub st_gen: c_uint,
        pub st_reserved: [c_uint; 10],
        pub st_size: off64_t,
    }

    pub struct mntent {
        pub mnt_fsname: *mut c_char,
        pub mnt_dir: *mut c_char,
        pub mnt_type: *mut c_char,
        pub mnt_opts: *mut c_char,
        pub mnt_freq: c_int,
        pub mnt_passno: c_int,
    }

    pub struct ipc_perm {
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub cuid: crate::uid_t,
        pub cgid: crate::gid_t,
        pub mode: mode_t,
        pub seq: c_ushort,
        pub __reserved: c_ushort,
        pub key: key_t,
    }

    pub struct entry {
        pub key: *mut c_char,
        pub data: *mut c_void,
    }

    pub struct mq_attr {
        pub mq_flags: c_long,
        pub mq_maxmsg: c_long,
        pub mq_msgsize: c_long,
        pub mq_curmsgs: c_long,
    }

    pub struct sembuf {
        pub sem_num: c_ushort,
        pub sem_op: c_short,
        pub sem_flg: c_short,
    }

    pub struct if_nameindex {
        pub if_index: c_uint,
        pub if_name: *mut c_char,
    }

    pub struct itimerspec {
        pub it_interval: crate::timespec,
        pub it_value: crate::timespec,
    }

    pub struct sigaction {
        pub sa_sigaction: crate::sighandler_t, // FIXME(union): this field is actually a union
        pub sa_mask: sigset_t,
        pub sa_flags: c_int,
    }
}

s_no_extra_traits! {
    pub union __poll_ctl_ext_u {
        pub addr: *mut c_void,
        pub data32: u32,
        pub data: u64,
    }

    pub struct poll_ctl_ext {
        pub version: u8,
        pub command: u8,
        pub events: c_short,
        pub fd: c_int,
        pub u: __poll_ctl_ext_u,
        pub reserved64: [u64; 6],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for __poll_ctl_ext_u {
            fn eq(&self, other: &__poll_ctl_ext_u) -> bool {
                unsafe {
                    self.addr == other.addr
                        && self.data32 == other.data32
                        && self.data == other.data
                }
            }
        }
        impl Eq for __poll_ctl_ext_u {}
        impl hash::Hash for __poll_ctl_ext_u {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    self.addr.hash(state);
                    self.data32.hash(state);
                    self.data.hash(state);
                }
            }
        }

        impl PartialEq for poll_ctl_ext {
            fn eq(&self, other: &poll_ctl_ext) -> bool {
                self.version == other.version
                    && self.command == other.command
                    && self.events == other.events
                    && self.fd == other.fd
                    && self.reserved64 == other.reserved64
                    && self.u == other.u
            }
        }
        impl Eq for poll_ctl_ext {}
        impl hash::Hash for poll_ctl_ext {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.version.hash(state);
                self.command.hash(state);
                self.events.hash(state);
                self.fd.hash(state);
                self.u.hash(state);
                self.reserved64.hash(state);
            }
        }
    }
}

// dlfcn.h
pub const RTLD_LAZY: c_int = 0x4;
pub const RTLD_NOW: c_int = 0x2;
pub const RTLD_GLOBAL: c_int = 0x10000;
pub const RTLD_LOCAL: c_int = 0x80000;
pub const RTLD_MEMBER: c_int = 0x40000;
pub const RTLD_NOAUTODEFER: c_int = 0x20000;
pub const RTLD_DEFAULT: *mut c_void = -1isize as *mut c_void;
pub const RTLD_MYSELF: *mut c_void = -2isize as *mut c_void;
pub const RTLD_NEXT: *mut c_void = -3isize as *mut c_void;

// fcntl.h
pub const O_RDONLY: c_int = 0x0;
pub const O_WRONLY: c_int = 0x1;
pub const O_RDWR: c_int = 0x2;
pub const O_NDELAY: c_int = 0x8000;
pub const O_APPEND: c_int = 0x8;
pub const O_DSYNC: c_int = 0x400000;
pub const O_CREAT: c_int = 0x100;
pub const O_EXCL: c_int = 0x400;
pub const O_NOCTTY: c_int = 0x800;
pub const O_TRUNC: c_int = 0x200;
pub const O_NOFOLLOW: c_int = 0x1000000;
pub const O_DIRECTORY: c_int = 0x80000;
pub const O_SEARCH: c_int = 0x20;
pub const O_EXEC: c_int = 0x20;
pub const O_CLOEXEC: c_int = 0x800000;
pub const O_ACCMODE: c_int = O_RDONLY | O_WRONLY | O_RDWR | O_EXEC | O_SEARCH;
pub const O_DIRECT: c_int = 0x8000000;
pub const O_TTY_INIT: c_int = 0;
pub const O_RSYNC: c_int = 0x200000;
pub const O_LARGEFILE: c_int = 0x4000000;
pub const F_DUPFD: c_int = 0;
pub const F_DUPFD_CLOEXEC: c_int = 16;
pub const F_GETFD: c_int = 1;
pub const F_SETFD: c_int = 2;
pub const F_GETFL: c_int = 3;
pub const F_SETFL: c_int = 4;
pub const F_GETLK: c_int = F_GETLK64;
pub const F_SETLK: c_int = F_SETLK64;
pub const F_SETLKW: c_int = F_SETLKW64;
pub const F_GETOWN: c_int = 8;
pub const F_SETOWN: c_int = 9;
pub const F_CLOSEM: c_int = 10;
pub const F_GETLK64: c_int = 11;
pub const F_SETLK64: c_int = 12;
pub const F_SETLKW64: c_int = 13;
pub const F_DUP2FD: c_int = 14;
pub const F_TSTLK: c_int = 15;
pub const AT_FDCWD: c_int = -2;
pub const AT_SYMLINK_NOFOLLOW: c_int = 1;
pub const AT_SYMLINK_FOLLOW: c_int = 2;
pub const AT_REMOVEDIR: c_int = 1;
pub const AT_EACCESS: c_int = 1;
pub const O_SYNC: c_int = 16;
pub const O_NONBLOCK: c_int = 4;
pub const FASYNC: c_int = 0x20000;
pub const POSIX_FADV_NORMAL: c_int = 1;
pub const POSIX_FADV_SEQUENTIAL: c_int = 2;
pub const POSIX_FADV_RANDOM: c_int = 3;
pub const POSIX_FADV_WILLNEED: c_int = 4;
pub const POSIX_FADV_DONTNEED: c_int = 5;
pub const POSIX_FADV_NOREUSE: c_int = 6;

// glob.h
pub const GLOB_APPEND: c_int = 0x1;
pub const GLOB_DOOFFS: c_int = 0x2;
pub const GLOB_ERR: c_int = 0x4;
pub const GLOB_MARK: c_int = 0x8;
pub const GLOB_NOCHECK: c_int = 0x10;
pub const GLOB_NOSORT: c_int = 0x20;
pub const GLOB_NOESCAPE: c_int = 0x80;
pub const GLOB_NOSPACE: c_int = 0x2000;
pub const GLOB_ABORTED: c_int = 0x1000;
pub const GLOB_NOMATCH: c_int = 0x4000;
pub const GLOB_NOSYS: c_int = 0x8000;

// langinfo.h
pub const DAY_1: crate::nl_item = 13;
pub const DAY_2: crate::nl_item = 14;
pub const DAY_3: crate::nl_item = 15;
pub const DAY_4: crate::nl_item = 16;
pub const DAY_5: crate::nl_item = 17;
pub const DAY_6: crate::nl_item = 18;
pub const DAY_7: crate::nl_item = 19;
pub const ABDAY_1: crate::nl_item = 6;
pub const ABDAY_2: crate::nl_item = 7;
pub const ABDAY_3: crate::nl_item = 8;
pub const ABDAY_4: crate::nl_item = 9;
pub const ABDAY_5: crate::nl_item = 10;
pub const ABDAY_6: crate::nl_item = 11;
pub const ABDAY_7: crate::nl_item = 12;
pub const MON_1: crate::nl_item = 32;
pub const MON_2: crate::nl_item = 33;
pub const MON_3: crate::nl_item = 34;
pub const MON_4: crate::nl_item = 35;
pub const MON_5: crate::nl_item = 36;
pub const MON_6: crate::nl_item = 37;
pub const MON_7: crate::nl_item = 38;
pub const MON_8: crate::nl_item = 39;
pub const MON_9: crate::nl_item = 40;
pub const MON_10: crate::nl_item = 41;
pub const MON_11: crate::nl_item = 42;
pub const MON_12: crate::nl_item = 43;
pub const ABMON_1: crate::nl_item = 20;
pub const ABMON_2: crate::nl_item = 21;
pub const ABMON_3: crate::nl_item = 22;
pub const ABMON_4: crate::nl_item = 23;
pub const ABMON_5: crate::nl_item = 24;
pub const ABMON_6: crate::nl_item = 25;
pub const ABMON_7: crate::nl_item = 26;
pub const ABMON_8: crate::nl_item = 27;
pub const ABMON_9: crate::nl_item = 28;
pub const ABMON_10: crate::nl_item = 29;
pub const ABMON_11: crate::nl_item = 30;
pub const ABMON_12: crate::nl_item = 31;
pub const RADIXCHAR: crate::nl_item = 44;
pub const THOUSEP: crate::nl_item = 45;
pub const YESSTR: crate::nl_item = 46;
pub const NOSTR: crate::nl_item = 47;
pub const CRNCYSTR: crate::nl_item = 48;
pub const D_T_FMT: crate::nl_item = 1;
pub const D_FMT: crate::nl_item = 2;
pub const T_FMT: crate::nl_item = 3;
pub const AM_STR: crate::nl_item = 4;
pub const PM_STR: crate::nl_item = 5;
pub const CODESET: crate::nl_item = 49;
pub const T_FMT_AMPM: crate::nl_item = 55;
pub const ERA: crate::nl_item = 56;
pub const ERA_D_FMT: crate::nl_item = 57;
pub const ERA_D_T_FMT: crate::nl_item = 58;
pub const ERA_T_FMT: crate::nl_item = 59;
pub const ALT_DIGITS: crate::nl_item = 60;
pub const YESEXPR: crate::nl_item = 61;
pub const NOEXPR: crate::nl_item = 62;

// locale.h
pub const LC_GLOBAL_LOCALE: crate::locale_t = -1isize as crate::locale_t;
pub const LC_COLLATE: c_int = 0;
pub const LC_CTYPE: c_int = 1;
pub const LC_MONETARY: c_int = 2;
pub const LC_NUMERIC: c_int = 3;
pub const LC_TIME: c_int = 4;
pub const LC_MESSAGES: c_int = 5;
pub const LC_ALL: c_int = -1;
pub const LC_COLLATE_MASK: c_int = 1;
pub const LC_CTYPE_MASK: c_int = 2;
pub const LC_MESSAGES_MASK: c_int = 4;
pub const LC_MONETARY_MASK: c_int = 8;
pub const LC_NUMERIC_MASK: c_int = 16;
pub const LC_TIME_MASK: c_int = 32;
pub const LC_ALL_MASK: c_int = LC_COLLATE_MASK
    | LC_CTYPE_MASK
    | LC_MESSAGES_MASK
    | LC_MONETARY_MASK
    | LC_NUMERIC_MASK
    | LC_TIME_MASK;

// netdb.h
pub const NI_MAXHOST: crate::socklen_t = 1025;
pub const NI_MAXSERV: crate::socklen_t = 32;
pub const NI_NOFQDN: crate::socklen_t = 0x1;
pub const NI_NUMERICHOST: crate::socklen_t = 0x2;
pub const NI_NAMEREQD: crate::socklen_t = 0x4;
pub const NI_NUMERICSERV: crate::socklen_t = 0x8;
pub const NI_DGRAM: crate::socklen_t = 0x10;
pub const NI_NUMERICSCOPE: crate::socklen_t = 0x40;
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
pub const EAI_OVERFLOW: c_int = 13;
pub const AI_CANONNAME: c_int = 0x01;
pub const AI_PASSIVE: c_int = 0x02;
pub const AI_NUMERICHOST: c_int = 0x04;
pub const AI_ADDRCONFIG: c_int = 0x08;
pub const AI_V4MAPPED: c_int = 0x10;
pub const AI_ALL: c_int = 0x20;
pub const AI_NUMERICSERV: c_int = 0x40;
pub const AI_EXTFLAGS: c_int = 0x80;
pub const AI_DEFAULT: c_int = AI_V4MAPPED | AI_ADDRCONFIG;
pub const IPV6_ADDRFORM: c_int = 22;
pub const IPV6_ADDR_PREFERENCES: c_int = 74;
pub const IPV6_CHECKSUM: c_int = 39;
pub const IPV6_DONTFRAG: c_int = 45;
pub const IPV6_DSTOPTS: c_int = 54;
pub const IPV6_FLOWINFO_FLOWLABEL: c_int = 0x00ffffff;
pub const IPV6_FLOWINFO_PRIORITY: c_int = 0x0f000000;
pub const IPV6_FLOWINFO_PRIFLOW: c_int = 0x0fffffff;
pub const IPV6_FLOWINFO_SRFLAG: c_int = 0x10000000;
pub const IPV6_FLOWINFO_VERSION: c_int = 0xf0000000;
pub const IPV6_HOPLIMIT: c_int = 40;
pub const IPV6_HOPOPTS: c_int = 52;
pub const IPV6_NEXTHOP: c_int = 48;
pub const IPV6_PATHMTU: c_int = 46;
pub const IPV6_PKTINFO: c_int = 33;
pub const IPV6_PREFER_SRC_CGA: c_int = 16;
pub const IPV6_PREFER_SRC_COA: c_int = 2;
pub const IPV6_PREFER_SRC_HOME: c_int = 1;
pub const IPV6_PREFER_SRC_NONCGA: c_int = 32;
pub const IPV6_PREFER_SRC_PUBLIC: c_int = 4;
pub const IPV6_PREFER_SRC_TMP: c_int = 8;
pub const IPV6_RECVDSTOPTS: c_int = 56;
pub const IPV6_RECVHOPLIMIT: c_int = 41;
pub const IPV6_RECVHOPOPTS: c_int = 53;
pub const IPV6_RECVPATHMTU: c_int = 47;
pub const IPV6_RECVRTHDR: c_int = 51;
pub const IPV6_RECVTCLASS: c_int = 42;
pub const IPV6_RTHDR: c_int = 50;
pub const IPV6_RTHDRDSTOPTS: c_int = 55;
pub const IPV6_TCLASS: c_int = 43;

// net/bpf.h
pub const DLT_NULL: c_int = 0x18;
pub const DLT_EN10MB: c_int = 0x6;
pub const DLT_EN3MB: c_int = 0x1a;
pub const DLT_AX25: c_int = 0x5;
pub const DLT_PRONET: c_int = 0xd;
pub const DLT_IEEE802: c_int = 0x7;
pub const DLT_ARCNET: c_int = 0x23;
pub const DLT_SLIP: c_int = 0x1c;
pub const DLT_PPP: c_int = 0x17;
pub const DLT_FDDI: c_int = 0xf;
pub const DLT_ATM: c_int = 0x25;
pub const DLT_IPOIB: c_int = 0xc7;
pub const BIOCSETF: c_int = 0x80104267;
pub const BIOCGRTIMEOUT: c_int = 0x4010426e;
pub const BIOCGBLEN: c_int = 0x40044266;
pub const BIOCSBLEN: c_int = 0xc0044266;
pub const BIOCFLUSH: c_int = 0x20004268;
pub const BIOCPROMISC: c_int = 0x20004269;
pub const BIOCGDLT: c_int = 0x4004426a;
pub const BIOCSRTIMEOUT: c_int = 0x8010426d;
pub const BIOCGSTATS: c_int = 0x4008426f;
pub const BIOCIMMEDIATE: c_int = 0x80044270;
pub const BIOCVERSION: c_int = 0x40044271;
pub const BIOCSDEVNO: c_int = 0x20004272;
pub const BIOCGETIF: c_int = 0x4020426b;
pub const BIOCSETIF: c_int = 0x8020426c;
pub const BPF_ABS: c_int = 32;
pub const BPF_ADD: c_int = 0;
pub const BPF_ALIGNMENT: c_ulong = 4;
pub const BPF_ALU: c_int = 4;
pub const BPF_AND: c_int = 80;
pub const BPF_B: c_int = 16;
pub const BPF_DIV: c_int = 48;
pub const BPF_H: c_int = 8;
pub const BPF_IMM: c_int = 0;
pub const BPF_IND: c_int = 64;
pub const BPF_JA: c_int = 0;
pub const BPF_JEQ: c_int = 16;
pub const BPF_JGE: c_int = 48;
pub const BPF_JGT: c_int = 32;
pub const BPF_JMP: c_int = 5;
pub const BPF_JSET: c_int = 64;
pub const BPF_K: c_int = 0;
pub const BPF_LD: c_int = 0;
pub const BPF_LDX: c_int = 1;
pub const BPF_LEN: c_int = 128;
pub const BPF_LSH: c_int = 96;
pub const BPF_MAXINSNS: c_int = 512;
pub const BPF_MEM: c_int = 96;
pub const BPF_MEMWORDS: c_int = 16;
pub const BPF_MISC: c_int = 7;
pub const BPF_MSH: c_int = 160;
pub const BPF_MUL: c_int = 32;
pub const BPF_NEG: c_int = 128;
pub const BPF_OR: c_int = 64;
pub const BPF_RET: c_int = 6;
pub const BPF_RSH: c_int = 112;
pub const BPF_ST: c_int = 2;
pub const BPF_STX: c_int = 3;
pub const BPF_SUB: c_int = 16;
pub const BPF_W: c_int = 0;
pub const BPF_X: c_int = 8;

// net/if.h
pub const IFNET_SLOWHZ: c_int = 1;
pub const IFQ_MAXLEN: c_int = 50;
pub const IFF_UP: c_int = 0x1;
pub const IFF_BROADCAST: c_int = 0x2;
pub const IFF_DEBUG: c_int = 0x4;
pub const IFF_LOOPBACK: c_int = 0x8;
pub const IFF_POINTOPOINT: c_int = 0x10;
pub const IFF_NOTRAILERS: c_int = 0x20;
pub const IFF_RUNNING: c_int = 0x40;
pub const IFF_NOARP: c_int = 0x80;
pub const IFF_PROMISC: c_int = 0x100;
pub const IFF_ALLMULTI: c_int = 0x200;
pub const IFF_MULTICAST: c_int = 0x80000;
pub const IFF_LINK0: c_int = 0x100000;
pub const IFF_LINK1: c_int = 0x200000;
pub const IFF_LINK2: c_int = 0x400000;
pub const IFF_OACTIVE: c_int = 0x400;
pub const IFF_SIMPLEX: c_int = 0x800;

// net/if_arp.h
pub const ARPHRD_ETHER: c_int = 1;
pub const ARPHRD_802_5: c_int = 6;
pub const ARPHRD_802_3: c_int = 6;
pub const ARPHRD_FDDI: c_int = 1;

// net/route.h
pub const RTM_ADD: c_int = 0x1;
pub const RTM_DELETE: c_int = 0x2;
pub const RTM_CHANGE: c_int = 0x3;
pub const RTM_GET: c_int = 0x4;
pub const RTM_LOSING: c_int = 0x5;
pub const RTM_REDIRECT: c_int = 0x6;
pub const RTM_MISS: c_int = 0x7;
pub const RTM_LOCK: c_int = 0x8;
pub const RTM_OLDADD: c_int = 0x9;
pub const RTM_OLDDEL: c_int = 0xa;
pub const RTM_RESOLVE: c_int = 0xb;
pub const RTM_NEWADDR: c_int = 0xc;
pub const RTM_DELADDR: c_int = 0xd;
pub const RTM_IFINFO: c_int = 0xe;
pub const RTM_EXPIRE: c_int = 0xf;
pub const RTM_RTLOST: c_int = 0x10;
pub const RTM_GETNEXT: c_int = 0x11;
pub const RTM_SAMEADDR: c_int = 0x12;
pub const RTM_SET: c_int = 0x13;
pub const RTV_MTU: c_int = 0x1;
pub const RTV_HOPCOUNT: c_int = 0x2;
pub const RTV_EXPIRE: c_int = 0x4;
pub const RTV_RPIPE: c_int = 0x8;
pub const RTV_SPIPE: c_int = 0x10;
pub const RTV_SSTHRESH: c_int = 0x20;
pub const RTV_RTT: c_int = 0x40;
pub const RTV_RTTVAR: c_int = 0x80;
pub const RTA_DST: c_int = 0x1;
pub const RTA_GATEWAY: c_int = 0x2;
pub const RTA_NETMASK: c_int = 0x4;
pub const RTA_GENMASK: c_int = 0x8;
pub const RTA_IFP: c_int = 0x10;
pub const RTA_IFA: c_int = 0x20;
pub const RTA_AUTHOR: c_int = 0x40;
pub const RTA_BRD: c_int = 0x80;
pub const RTA_DOWNSTREAM: c_int = 0x100;
pub const RTAX_DST: c_int = 0;
pub const RTAX_GATEWAY: c_int = 1;
pub const RTAX_NETMASK: c_int = 2;
pub const RTAX_GENMASK: c_int = 3;
pub const RTAX_IFP: c_int = 4;
pub const RTAX_IFA: c_int = 5;
pub const RTAX_AUTHOR: c_int = 6;
pub const RTAX_BRD: c_int = 7;
pub const RTAX_MAX: c_int = 8;
pub const RTF_UP: c_int = 0x1;
pub const RTF_GATEWAY: c_int = 0x2;
pub const RTF_HOST: c_int = 0x4;
pub const RTF_REJECT: c_int = 0x8;
pub const RTF_DYNAMIC: c_int = 0x10;
pub const RTF_MODIFIED: c_int = 0x20;
pub const RTF_DONE: c_int = 0x40;
pub const RTF_MASK: c_int = 0x80;
pub const RTF_CLONING: c_int = 0x100;
pub const RTF_XRESOLVE: c_int = 0x200;
pub const RTF_LLINFO: c_int = 0x400;
pub const RTF_STATIC: c_int = 0x800;
pub const RTF_BLACKHOLE: c_int = 0x1000;
pub const RTF_BUL: c_int = 0x2000;
pub const RTF_PROTO2: c_int = 0x4000;
pub const RTF_PROTO1: c_int = 0x8000;
pub const RTF_CLONE: c_int = 0x10000;
pub const RTF_CLONED: c_int = 0x20000;
pub const RTF_PROTO3: c_int = 0x40000;
pub const RTF_BCE: c_int = 0x80000;
pub const RTF_PINNED: c_int = 0x100000;
pub const RTF_LOCAL: c_int = 0x200000;
pub const RTF_BROADCAST: c_int = 0x400000;
pub const RTF_MULTICAST: c_int = 0x800000;
pub const RTF_ACTIVE_DGD: c_int = 0x1000000;
pub const RTF_STOPSRCH: c_int = 0x2000000;
pub const RTF_FREE_IN_PROG: c_int = 0x4000000;
pub const RTF_PERMANENT6: c_int = 0x8000000;
pub const RTF_UNREACHABLE: c_int = 0x10000000;
pub const RTF_CACHED: c_int = 0x20000000;
pub const RTF_SMALLMTU: c_int = 0x40000;

// netinet/in.h
pub const IPPROTO_HOPOPTS: c_int = 0;
pub const IPPROTO_IGMP: c_int = 2;
pub const IPPROTO_GGP: c_int = 3;
pub const IPPROTO_IPIP: c_int = 4;
pub const IPPROTO_EGP: c_int = 8;
pub const IPPROTO_PUP: c_int = 12;
pub const IPPROTO_IDP: c_int = 22;
pub const IPPROTO_TP: c_int = 29;
pub const IPPROTO_ROUTING: c_int = 43;
pub const IPPROTO_FRAGMENT: c_int = 44;
pub const IPPROTO_QOS: c_int = 45;
pub const IPPROTO_RSVP: c_int = 46;
pub const IPPROTO_GRE: c_int = 47;
pub const IPPROTO_ESP: c_int = 50;
pub const IPPROTO_AH: c_int = 51;
pub const IPPROTO_NONE: c_int = 59;
pub const IPPROTO_DSTOPTS: c_int = 60;
pub const IPPROTO_LOCAL: c_int = 63;
pub const IPPROTO_EON: c_int = 80;
pub const IPPROTO_BIP: c_int = 0x53;
pub const IPPROTO_SCTP: c_int = 132;
pub const IPPROTO_MH: c_int = 135;
pub const IPPROTO_GIF: c_int = 140;
pub const IPPROTO_RAW: c_int = 255;
pub const IP_OPTIONS: c_int = 1;
pub const IP_HDRINCL: c_int = 2;
pub const IP_TOS: c_int = 3;
pub const IP_TTL: c_int = 4;
pub const IP_UNICAST_HOPS: c_int = 4;
pub const IP_RECVOPTS: c_int = 5;
pub const IP_RECVRETOPTS: c_int = 6;
pub const IP_RECVDSTADDR: c_int = 7;
pub const IP_RETOPTS: c_int = 8;
pub const IP_MULTICAST_IF: c_int = 9;
pub const IP_MULTICAST_TTL: c_int = 10;
pub const IP_MULTICAST_HOPS: c_int = 10;
pub const IP_MULTICAST_LOOP: c_int = 11;
pub const IP_ADD_MEMBERSHIP: c_int = 12;
pub const IP_DROP_MEMBERSHIP: c_int = 13;
pub const IP_RECVMACHDR: c_int = 14;
pub const IP_RECVIFINFO: c_int = 15;
pub const IP_BROADCAST_IF: c_int = 16;
pub const IP_DHCPMODE: c_int = 17;
pub const IP_RECVIF: c_int = 20;
pub const IP_ADDRFORM: c_int = 22;
pub const IP_DONTFRAG: c_int = 25;
pub const IP_FINDPMTU: c_int = 26;
pub const IP_PMTUAGE: c_int = 27;
pub const IP_RECVINTERFACE: c_int = 32;
pub const IP_RECVTTL: c_int = 34;
pub const IP_BLOCK_SOURCE: c_int = 58;
pub const IP_UNBLOCK_SOURCE: c_int = 59;
pub const IP_ADD_SOURCE_MEMBERSHIP: c_int = 60;
pub const IP_DROP_SOURCE_MEMBERSHIP: c_int = 61;
pub const IP_DEFAULT_MULTICAST_TTL: c_int = 1;
pub const IP_DEFAULT_MULTICAST_LOOP: c_int = 1;
pub const IP_INC_MEMBERSHIPS: c_int = 20;
pub const IP_INIT_MEMBERSHIP: c_int = 20;
pub const IPV6_UNICAST_HOPS: c_int = IP_TTL;
pub const IPV6_MULTICAST_IF: c_int = IP_MULTICAST_IF;
pub const IPV6_MULTICAST_HOPS: c_int = IP_MULTICAST_TTL;
pub const IPV6_MULTICAST_LOOP: c_int = IP_MULTICAST_LOOP;
pub const IPV6_RECVPKTINFO: c_int = 35;
pub const IPV6_V6ONLY: c_int = 37;
pub const IPV6_ADD_MEMBERSHIP: c_int = IP_ADD_MEMBERSHIP;
pub const IPV6_DROP_MEMBERSHIP: c_int = IP_DROP_MEMBERSHIP;
pub const IPV6_JOIN_GROUP: c_int = IP_ADD_MEMBERSHIP;
pub const IPV6_LEAVE_GROUP: c_int = IP_DROP_MEMBERSHIP;
pub const MCAST_BLOCK_SOURCE: c_int = 64;
pub const MCAST_EXCLUDE: c_int = 2;
pub const MCAST_INCLUDE: c_int = 1;
pub const MCAST_JOIN_GROUP: c_int = 62;
pub const MCAST_JOIN_SOURCE_GROUP: c_int = 66;
pub const MCAST_LEAVE_GROUP: c_int = 63;
pub const MCAST_LEAVE_SOURCE_GROUP: c_int = 67;
pub const MCAST_UNBLOCK_SOURCE: c_int = 65;

// netinet/ip.h
pub const MAXTTL: c_int = 255;
pub const IPDEFTTL: c_int = 64;
pub const IPOPT_CONTROL: c_int = 0;
pub const IPOPT_EOL: c_int = 0;
pub const IPOPT_LSRR: c_int = 131;
pub const IPOPT_MINOFF: c_int = 4;
pub const IPOPT_NOP: c_int = 1;
pub const IPOPT_OFFSET: c_int = 2;
pub const IPOPT_OLEN: c_int = 1;
pub const IPOPT_OPTVAL: c_int = 0;
pub const IPOPT_RESERVED1: c_int = 0x20;
pub const IPOPT_RESERVED2: c_int = 0x60;
pub const IPOPT_RR: c_int = 7;
pub const IPOPT_SSRR: c_int = 137;
pub const IPOPT_TS: c_int = 68;
pub const IPOPT_TS_PRESPEC: c_int = 3;
pub const IPOPT_TS_TSANDADDR: c_int = 1;
pub const IPOPT_TS_TSONLY: c_int = 0;
pub const IPTOS_LOWDELAY: c_int = 16;
pub const IPTOS_PREC_CRITIC_ECP: c_int = 160;
pub const IPTOS_PREC_FLASH: c_int = 96;
pub const IPTOS_PREC_FLASHOVERRIDE: c_int = 128;
pub const IPTOS_PREC_IMMEDIATE: c_int = 64;
pub const IPTOS_PREC_INTERNETCONTROL: c_int = 192;
pub const IPTOS_PREC_NETCONTROL: c_int = 224;
pub const IPTOS_PREC_PRIORITY: c_int = 32;
pub const IPTOS_PREC_ROUTINE: c_int = 16;
pub const IPTOS_RELIABILITY: c_int = 4;
pub const IPTOS_THROUGHPUT: c_int = 8;
pub const IPVERSION: c_int = 4;

// netinet/tcp.h
pub const TCP_NODELAY: c_int = 0x1;
pub const TCP_MAXSEG: c_int = 0x2;
pub const TCP_RFC1323: c_int = 0x4;
pub const TCP_KEEPALIVE: c_int = 0x8;
pub const TCP_KEEPIDLE: c_int = 0x11;
pub const TCP_KEEPINTVL: c_int = 0x12;
pub const TCP_KEEPCNT: c_int = 0x13;
pub const TCP_NODELAYACK: c_int = 0x14;

// pthread.h
pub const PTHREAD_BARRIER_SERIAL_THREAD: c_int = 2;
pub const PTHREAD_CREATE_JOINABLE: c_int = 0;
pub const PTHREAD_CREATE_DETACHED: c_int = 1;
pub const PTHREAD_PROCESS_SHARED: c_int = 0;
pub const PTHREAD_PROCESS_PRIVATE: c_ushort = 1;
pub const PTHREAD_STACK_MIN: size_t = PAGESIZE as size_t * 4;
pub const PTHREAD_MUTEX_NORMAL: c_int = 5;
pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 3;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 4;
pub const PTHREAD_MUTEX_DEFAULT: c_int = PTHREAD_MUTEX_NORMAL;
pub const PTHREAD_MUTEX_ROBUST: c_int = 1;
pub const PTHREAD_MUTEX_STALLED: c_int = 0;
pub const PTHREAD_PRIO_INHERIT: c_int = 3;
pub const PTHREAD_PRIO_NONE: c_int = 1;
pub const PTHREAD_PRIO_PROTECT: c_int = 2;

// regex.h
pub const REG_EXTENDED: c_int = 1;
pub const REG_ICASE: c_int = 2;
pub const REG_NEWLINE: c_int = 4;
pub const REG_NOSUB: c_int = 8;
pub const REG_NOTBOL: c_int = 0x100;
pub const REG_NOTEOL: c_int = 0x200;
pub const REG_NOMATCH: c_int = 1;
pub const REG_BADPAT: c_int = 2;
pub const REG_ECOLLATE: c_int = 3;
pub const REG_ECTYPE: c_int = 4;
pub const REG_EESCAPE: c_int = 5;
pub const REG_ESUBREG: c_int = 6;
pub const REG_EBRACK: c_int = 7;
pub const REG_EPAREN: c_int = 8;
pub const REG_EBRACE: c_int = 9;
pub const REG_BADBR: c_int = 10;
pub const REG_ERANGE: c_int = 11;
pub const REG_ESPACE: c_int = 12;
pub const REG_BADRPT: c_int = 13;
pub const REG_ECHAR: c_int = 14;
pub const REG_EBOL: c_int = 15;
pub const REG_EEOL: c_int = 16;
pub const REG_ENOSYS: c_int = 17;

// rpcsvc/mount.h
pub const NFSMNT_SOFT: c_int = 0x001;
pub const NFSMNT_WSIZE: c_int = 0x002;
pub const NFSMNT_RSIZE: c_int = 0x004;
pub const NFSMNT_TIMEO: c_int = 0x008;
pub const NFSMNT_RETRANS: c_int = 0x010;
pub const NFSMNT_HOSTNAME: c_int = 0x020;
pub const NFSMNT_INT: c_int = 0x040;
pub const NFSMNT_NOAC: c_int = 0x080;
pub const NFSMNT_ACREGMIN: c_int = 0x0100;
pub const NFSMNT_ACREGMAX: c_int = 0x0200;
pub const NFSMNT_ACDIRMIN: c_int = 0x0400;
pub const NFSMNT_ACDIRMAX: c_int = 0x0800;

// rpcsvc/rstat.h
pub const CPUSTATES: c_int = 4;

// semaphore.h
pub const SEM_FAILED: *mut sem_t = -1isize as *mut crate::sem_t;

// spawn.h
// DIFF(main): changed to `c_short` in f62eb023ab
pub const POSIX_SPAWN_SETPGROUP: c_int = 0x1;
pub const POSIX_SPAWN_SETSIGMASK: c_int = 0x2;
pub const POSIX_SPAWN_SETSIGDEF: c_int = 0x4;
pub const POSIX_SPAWN_SETSCHEDULER: c_int = 0x8;
pub const POSIX_SPAWN_SETSCHEDPARAM: c_int = 0x10;
pub const POSIX_SPAWN_RESETIDS: c_int = 0x20;
pub const POSIX_SPAWN_FORK_HANDLERS: c_int = 0x1000;

// stdio.h
pub const EOF: c_int = -1;
pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;
pub const _IOFBF: c_int = 0o000;
pub const _IONBF: c_int = 0o004;
pub const _IOLBF: c_int = 0o100;
pub const BUFSIZ: c_uint = 4096;
pub const FOPEN_MAX: c_uint = 32767;
pub const FILENAME_MAX: c_uint = 255;
pub const L_tmpnam: c_uint = 21;
pub const TMP_MAX: c_uint = 16384;

// stdlib.h
pub const EXIT_FAILURE: c_int = 1;
pub const EXIT_SUCCESS: c_int = 0;
pub const RAND_MAX: c_int = 32767;

// sys/access.h
pub const F_OK: c_int = 0;
pub const R_OK: c_int = 4;
pub const W_OK: c_int = 2;
pub const X_OK: c_int = 1;

// sys/aio.h
pub const LIO_NOP: c_int = 0;
pub const LIO_READ: c_int = 1;
pub const LIO_WRITE: c_int = 2;
pub const LIO_NOWAIT: c_int = 0;
pub const LIO_WAIT: c_int = 1;
pub const AIO_ALLDONE: c_int = 2;
pub const AIO_CANCELED: c_int = 0;
pub const AIO_NOTCANCELED: c_int = 1;

// sys/errno.h
pub const EPERM: c_int = 1;
pub const ENOENT: c_int = 2;
pub const ESRCH: c_int = 3;
pub const EINTR: c_int = 4;
pub const EIO: c_int = 5;
pub const ENXIO: c_int = 6;
pub const E2BIG: c_int = 7;
pub const ENOEXEC: c_int = 8;
pub const EBADF: c_int = 9;
pub const ECHILD: c_int = 10;
pub const EAGAIN: c_int = 11;
pub const ENOMEM: c_int = 12;
pub const EACCES: c_int = 13;
pub const EFAULT: c_int = 14;
pub const ENOTBLK: c_int = 15;
pub const EBUSY: c_int = 16;
pub const EEXIST: c_int = 17;
pub const EXDEV: c_int = 18;
pub const ENODEV: c_int = 19;
pub const ENOTDIR: c_int = 20;
pub const EISDIR: c_int = 21;
pub const EINVAL: c_int = 22;
pub const ENFILE: c_int = 23;
pub const EMFILE: c_int = 24;
pub const ENOTTY: c_int = 25;
pub const ETXTBSY: c_int = 26;
pub const EFBIG: c_int = 27;
pub const ENOSPC: c_int = 28;
pub const ESPIPE: c_int = 29;
pub const EROFS: c_int = 30;
pub const EMLINK: c_int = 31;
pub const EPIPE: c_int = 32;
pub const EDOM: c_int = 33;
pub const ERANGE: c_int = 34;
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
pub const EDEADLK: c_int = 45;
pub const ENOTREADY: c_int = 46;
pub const EWRPROTECT: c_int = 47;
pub const EFORMAT: c_int = 48;
pub const ENOLCK: c_int = 49;
pub const ENOCONNECT: c_int = 50;
pub const ESTALE: c_int = 52;
pub const EDIST: c_int = 53;
pub const EWOULDBLOCK: c_int = 54;
pub const EINPROGRESS: c_int = 55;
pub const EALREADY: c_int = 56;
pub const ENOTSOCK: c_int = 57;
pub const EDESTADDRREQ: c_int = 58;
pub const EMSGSIZE: c_int = 59;
pub const EPROTOTYPE: c_int = 60;
pub const ENOPROTOOPT: c_int = 61;
pub const EPROTONOSUPPORT: c_int = 62;
pub const ESOCKTNOSUPPORT: c_int = 63;
pub const EOPNOTSUPP: c_int = 64;
pub const EPFNOSUPPORT: c_int = 65;
pub const EAFNOSUPPORT: c_int = 66;
pub const EADDRINUSE: c_int = 67;
pub const EADDRNOTAVAIL: c_int = 68;
pub const ENETDOWN: c_int = 69;
pub const ENETUNREACH: c_int = 70;
pub const ENETRESET: c_int = 71;
pub const ECONNABORTED: c_int = 72;
pub const ECONNRESET: c_int = 73;
pub const ENOBUFS: c_int = 74;
pub const EISCONN: c_int = 75;
pub const ENOTCONN: c_int = 76;
pub const ESHUTDOWN: c_int = 77;
pub const ETIMEDOUT: c_int = 78;
pub const ECONNREFUSED: c_int = 79;
pub const EHOSTDOWN: c_int = 80;
pub const EHOSTUNREACH: c_int = 81;
pub const ERESTART: c_int = 82;
pub const EPROCLIM: c_int = 83;
pub const EUSERS: c_int = 84;
pub const ELOOP: c_int = 85;
pub const ENAMETOOLONG: c_int = 86;
pub const ENOTEMPTY: c_int = 87;
pub const EDQUOT: c_int = 88;
pub const ECORRUPT: c_int = 89;
pub const ESYSERROR: c_int = 90;
pub const EREMOTE: c_int = 93;
pub const ENOTRECOVERABLE: c_int = 94;
pub const EOWNERDEAD: c_int = 95;
// errnos 96-108 reserved for future use compatible with AIX PS/2
pub const ENOSYS: c_int = 109;
pub const EMEDIA: c_int = 110;
pub const ESOFT: c_int = 111;
pub const ENOATTR: c_int = 112;
pub const ESAD: c_int = 113;
pub const ENOTRUST: c_int = 114;
pub const ETOOMANYREFS: c_int = 115;
pub const EILSEQ: c_int = 116;
pub const ECANCELED: c_int = 117;
pub const ENOSR: c_int = 118;
pub const ETIME: c_int = 119;
pub const EBADMSG: c_int = 120;
pub const EPROTO: c_int = 121;
pub const ENODATA: c_int = 122;
pub const ENOSTR: c_int = 123;
pub const ENOTSUP: c_int = 124;
pub const EMULTIHOP: c_int = 125;
pub const ENOLINK: c_int = 126;
pub const EOVERFLOW: c_int = 127;

// sys/dr.h
pub const LPAR_INFO_FORMAT1: c_int = 1;
pub const LPAR_INFO_FORMAT2: c_int = 2;
pub const WPAR_INFO_FORMAT: c_int = 3;
pub const PROC_MODULE_INFO: c_int = 4;
pub const NUM_PROC_MODULE_TYPES: c_int = 5;
pub const LPAR_INFO_VRME_NUM_POOLS: c_int = 6;
pub const LPAR_INFO_VRME_POOLS: c_int = 7;
pub const LPAR_INFO_VRME_LPAR: c_int = 8;
pub const LPAR_INFO_VRME_RESET_HWMARKS: c_int = 9;
pub const LPAR_INFO_VRME_ALLOW_DESIRED: c_int = 10;
pub const EMTP_INFO_FORMAT: c_int = 11;
pub const LPAR_INFO_LPM_CAPABILITY: c_int = 12;
pub const ENERGYSCALE_INFO: c_int = 13;

// sys/file.h
pub const LOCK_SH: c_int = 1;
pub const LOCK_EX: c_int = 2;
pub const LOCK_NB: c_int = 4;
pub const LOCK_UN: c_int = 8;

// sys/flock.h
pub const F_RDLCK: c_short = 0o01;
pub const F_WRLCK: c_short = 0o02;
pub const F_UNLCK: c_short = 0o03;

// sys/fs/quota_common.h
pub const Q_QUOTAON: c_int = 0x100;
pub const Q_QUOTAOFF: c_int = 0x200;
pub const Q_SETUSE: c_int = 0x500;
pub const Q_SYNC: c_int = 0x600;
pub const Q_GETQUOTA: c_int = 0x300;
pub const Q_SETQLIM: c_int = 0x400;
pub const Q_SETQUOTA: c_int = 0x400;

// sys/ioctl.h
pub const IOCPARM_MASK: c_int = 0x7f;
pub const IOC_VOID: c_int = 0x20000000;
pub const IOC_OUT: c_int = 0x40000000;
pub const IOC_IN: c_int = 0x40000000 << 1;
pub const IOC_INOUT: c_int = IOC_IN | IOC_OUT;
pub const FIOCLEX: c_int = 0x20006601;
pub const FIONCLEX: c_int = 0x20006602;
pub const FIONREAD: c_int = 0x4004667f;
pub const FIONBIO: c_int = 0x8004667e;
pub const FIOASYNC: c_int = 0x8004667d;
pub const FIOSETOWN: c_int = 0x8004667c;
pub const FIOGETOWN: c_int = 0x4004667b;
pub const TIOCGETD: c_int = 0x40047400;
pub const TIOCSETD: c_int = 0x80047401;
pub const TIOCHPCL: c_int = 0x20007402;
pub const TIOCMODG: c_int = 0x40047403;
pub const TIOCMODS: c_int = 0x80047404;
pub const TIOCM_LE: c_int = 0x1;
pub const TIOCM_DTR: c_int = 0x2;
pub const TIOCM_RTS: c_int = 0x4;
pub const TIOCM_ST: c_int = 0x8;
pub const TIOCM_SR: c_int = 0x10;
pub const TIOCM_CTS: c_int = 0x20;
pub const TIOCM_CAR: c_int = 0x40;
pub const TIOCM_CD: c_int = 0x40;
pub const TIOCM_RNG: c_int = 0x80;
pub const TIOCM_RI: c_int = 0x80;
pub const TIOCM_DSR: c_int = 0x100;
pub const TIOCGETP: c_int = 0x40067408;
pub const TIOCSETP: c_int = 0x80067409;
pub const TIOCSETN: c_int = 0x8006740a;
pub const TIOCEXCL: c_int = 0x2000740d;
pub const TIOCNXCL: c_int = 0x2000740e;
pub const TIOCFLUSH: c_int = 0x80047410;
pub const TIOCSETC: c_int = 0x80067411;
pub const TIOCGETC: c_int = 0x40067412;
pub const TANDEM: c_int = 0x1;
pub const CBREAK: c_int = 0x2;
pub const LCASE: c_int = 0x4;
pub const MDMBUF: c_int = 0x800000;
pub const XTABS: c_int = 0xc00;
pub const SIOCADDMULTI: c_int = 0x80206931;
pub const SIOCADDRT: c_int = 0x8038720a;
pub const SIOCDARP: c_int = 0x804c6920;
pub const SIOCDELMULTI: c_int = 0x80206932;
pub const SIOCDELRT: c_int = 0x8038720b;
pub const SIOCDIFADDR: c_int = 0x80286919;
pub const SIOCGARP: c_int = 0xc04c6926;
pub const SIOCGIFADDR: c_int = 0xc0286921;
pub const SIOCGIFBRDADDR: c_int = 0xc0286923;
pub const SIOCGIFCONF: c_int = 0xc0106945;
pub const SIOCGIFDSTADDR: c_int = 0xc0286922;
pub const SIOCGIFFLAGS: c_int = 0xc0286911;
pub const SIOCGIFHWADDR: c_int = 0xc0546995;
pub const SIOCGIFMETRIC: c_int = 0xc0286917;
pub const SIOCGIFMTU: c_int = 0xc0286956;
pub const SIOCGIFNETMASK: c_int = 0xc0286925;
pub const SIOCSARP: c_int = 0x804c691e;
pub const SIOCSIFADDR: c_int = 0x8028690c;
pub const SIOCSIFBRDADDR: c_int = 0x80286913;
pub const SIOCSIFDSTADDR: c_int = 0x8028690e;
pub const SIOCSIFFLAGS: c_int = 0x80286910;
pub const SIOCSIFMETRIC: c_int = 0x80286918;
pub const SIOCSIFMTU: c_int = 0x80286958;
pub const SIOCSIFNETMASK: c_int = 0x80286916;
pub const TIOCUCNTL: c_int = 0x80047466;
pub const TIOCCONS: c_int = 0x80047462;
pub const TIOCPKT: c_int = 0x80047470;
pub const TIOCPKT_DATA: c_int = 0;
pub const TIOCPKT_FLUSHREAD: c_int = 1;
pub const TIOCPKT_FLUSHWRITE: c_int = 2;
pub const TIOCPKT_NOSTOP: c_int = 0x10;
pub const TIOCPKT_DOSTOP: c_int = 0x20;
pub const TIOCPKT_START: c_int = 8;
pub const TIOCPKT_STOP: c_int = 4;

// sys/ipc.h
pub const IPC_ALLOC: c_int = 0o100000;
pub const IPC_CREAT: c_int = 0o020000;
pub const IPC_EXCL: c_int = 0o002000;
pub const IPC_NOWAIT: c_int = 0o004000;
pub const IPC_RMID: c_int = 0;
pub const IPC_SET: c_int = 101;
pub const IPC_R: c_int = 0o0400;
pub const IPC_W: c_int = 0o0200;
pub const IPC_O: c_int = 0o1000;
pub const IPC_NOERROR: c_int = 0o10000;
pub const IPC_STAT: c_int = 102;
pub const IPC_PRIVATE: crate::key_t = -1;
pub const SHM_LOCK: c_int = 201;
pub const SHM_UNLOCK: c_int = 202;

// sys/ldr.h
pub const L_GETMESSAGES: c_int = 1;
pub const L_GETINFO: c_int = 2;
pub const L_GETLIBPATH: c_int = 3;
pub const L_GETKERNINFO: c_int = 4;
pub const L_GETLIB32INFO: c_int = 5;
pub const L_GETLIB64INFO: c_int = 6;
pub const L_GETPROCINFO: c_int = 7;
pub const L_GETXINFO: c_int = 8;

// sys/limits.h
pub const PATH_MAX: c_int = 1023;
pub const PAGESIZE: c_int = 4096;
pub const IOV_MAX: c_int = 16;
pub const AIO_LISTIO_MAX: c_int = 4096;
pub const PIPE_BUF: usize = 32768;
pub const OPEN_MAX: c_int = 65534;
pub const MAX_INPUT: c_int = 512;
pub const MAX_CANON: c_int = 256;
pub const ARG_MAX: c_int = 1048576;
pub const BC_BASE_MAX: c_int = 99;
pub const BC_DIM_MAX: c_int = 0x800;
pub const BC_SCALE_MAX: c_int = 99;
pub const BC_STRING_MAX: c_int = 0x800;
pub const CHARCLASS_NAME_MAX: c_int = 14;
pub const CHILD_MAX: c_int = 128;
pub const COLL_WEIGHTS_MAX: c_int = 4;
pub const EXPR_NEST_MAX: c_int = 32;
pub const NZERO: c_int = 20;

// sys/lockf.h
pub const F_LOCK: c_int = 1;
pub const F_TEST: c_int = 3;
pub const F_TLOCK: c_int = 2;
pub const F_ULOCK: c_int = 0;

// sys/machine.h
pub const BIG_ENDIAN: c_int = 4321;
pub const LITTLE_ENDIAN: c_int = 1234;
pub const PDP_ENDIAN: c_int = 3412;

// sys/mman.h
pub const PROT_NONE: c_int = 0;
pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const PROT_EXEC: c_int = 4;
pub const MAP_FILE: c_int = 0;
pub const MAP_SHARED: c_int = 1;
pub const MAP_PRIVATE: c_int = 2;
pub const MAP_FIXED: c_int = 0x100;
pub const MAP_ANON: c_int = 0x10;
pub const MAP_ANONYMOUS: c_int = 0x10;
pub const MAP_FAILED: *mut c_void = !0 as *mut c_void;
pub const MAP_TYPE: c_int = 0xf0;
pub const MCL_CURRENT: c_int = 0x100;
pub const MCL_FUTURE: c_int = 0x200;
pub const MS_SYNC: c_int = 0x20;
pub const MS_ASYNC: c_int = 0x10;
pub const MS_INVALIDATE: c_int = 0x40;
pub const POSIX_MADV_NORMAL: c_int = 1;
pub const POSIX_MADV_RANDOM: c_int = 3;
pub const POSIX_MADV_SEQUENTIAL: c_int = 2;
pub const POSIX_MADV_WILLNEED: c_int = 4;
pub const POSIX_MADV_DONTNEED: c_int = 5;
pub const MADV_NORMAL: c_int = 0;
pub const MADV_RANDOM: c_int = 1;
pub const MADV_SEQUENTIAL: c_int = 2;
pub const MADV_WILLNEED: c_int = 3;
pub const MADV_DONTNEED: c_int = 4;

// sys/mode.h
pub const S_IFMT: mode_t = 0o17_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFIFO: mode_t = 0o1_0000;
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
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_IEXEC: mode_t = 0o0100;
pub const S_IWRITE: mode_t = 0o0200;
pub const S_IREAD: mode_t = 0o0400;

// sys/msg.h
pub const MSG_NOERROR: c_int = 0o10000;

// sys/m_signal.h
pub const SIGSTKSZ: size_t = 4096;
pub const MINSIGSTKSZ: size_t = 1200;

// sys/params.h
pub const MAXPATHLEN: c_int = PATH_MAX + 1;
pub const MAXSYMLINKS: c_int = 20;
pub const MAXHOSTNAMELEN: c_int = 256;
pub const MAXUPRC: c_int = 128;
pub const NGROUPS_MAX: c_ulong = 2048;
pub const NGROUPS: c_ulong = NGROUPS_MAX;
pub const NOFILE: c_int = OPEN_MAX;

// sys/poll.h
pub const POLLIN: c_short = 0x0001;
pub const POLLPRI: c_short = 0x0004;
pub const POLLOUT: c_short = 0x0002;
pub const POLLERR: c_short = 0x4000;
pub const POLLHUP: c_short = 0x2000;
pub const POLLMSG: c_short = 0x0080;
pub const POLLSYNC: c_short = 0x8000;
pub const POLLNVAL: c_short = POLLSYNC;
pub const POLLNORM: c_short = POLLIN;
pub const POLLRDNORM: c_short = 0x0010;
pub const POLLWRNORM: c_short = POLLOUT;
pub const POLLRDBAND: c_short = 0x0020;
pub const POLLWRBAND: c_short = 0x0040;

// sys/pollset.h
pub const PS_ADD: c_uchar = 0;
pub const PS_MOD: c_uchar = 1;
pub const PS_DELETE: c_uchar = 2;
pub const PS_REPLACE: c_uchar = 3;

// sys/ptrace.h
pub const PT_TRACE_ME: c_int = 0;
pub const PT_READ_I: c_int = 1;
pub const PT_READ_D: c_int = 2;
pub const PT_WRITE_I: c_int = 4;
pub const PT_WRITE_D: c_int = 5;
pub const PT_CONTINUE: c_int = 7;
pub const PT_KILL: c_int = 8;
pub const PT_STEP: c_int = 9;
pub const PT_READ_GPR: c_int = 11;
pub const PT_READ_FPR: c_int = 12;
pub const PT_WRITE_GPR: c_int = 14;
pub const PT_WRITE_FPR: c_int = 15;
pub const PT_READ_BLOCK: c_int = 17;
pub const PT_WRITE_BLOCK: c_int = 19;
pub const PT_ATTACH: c_int = 30;
pub const PT_DETACH: c_int = 31;
pub const PT_REGSET: c_int = 32;
pub const PT_REATT: c_int = 33;
pub const PT_LDINFO: c_int = 34;
pub const PT_MULTI: c_int = 35;
pub const PT_NEXT: c_int = 36;
pub const PT_SET: c_int = 37;
pub const PT_CLEAR: c_int = 38;
pub const PT_LDXINFO: c_int = 39;
pub const PT_QUERY: c_int = 40;
pub const PT_WATCH: c_int = 41;
pub const PTT_CONTINUE: c_int = 50;
pub const PTT_STEP: c_int = 51;
pub const PTT_READ_SPRS: c_int = 52;
pub const PTT_WRITE_SPRS: c_int = 53;
pub const PTT_READ_GPRS: c_int = 54;
pub const PTT_WRITE_GPRS: c_int = 55;
pub const PTT_READ_FPRS: c_int = 56;
pub const PTT_WRITE_FPRS: c_int = 57;
pub const PTT_READ_VEC: c_int = 58;
pub const PTT_WRITE_VEC: c_int = 59;
pub const PTT_WATCH: c_int = 60;
pub const PTT_SET_TRAP: c_int = 61;
pub const PTT_CLEAR_TRAP: c_int = 62;
pub const PTT_READ_UKEYSET: c_int = 63;
pub const PT_GET_UKEY: c_int = 64;
pub const PTT_READ_FPSCR_HI: c_int = 65;
pub const PTT_WRITE_FPSCR_HI: c_int = 66;
pub const PTT_READ_VSX: c_int = 67;
pub const PTT_WRITE_VSX: c_int = 68;
pub const PTT_READ_TM: c_int = 69;
pub const PTRACE_ATTACH: c_int = 14;
pub const PTRACE_CONT: c_int = 7;
pub const PTRACE_DETACH: c_int = 15;
pub const PTRACE_GETFPREGS: c_int = 12;
pub const PTRACE_GETREGS: c_int = 10;
pub const PTRACE_KILL: c_int = 8;
pub const PTRACE_PEEKDATA: c_int = 2;
pub const PTRACE_PEEKTEXT: c_int = 1;
pub const PTRACE_PEEKUSER: c_int = 3;
pub const PTRACE_POKEDATA: c_int = 5;
pub const PTRACE_POKETEXT: c_int = 4;
pub const PTRACE_POKEUSER: c_int = 6;
pub const PTRACE_SETFPREGS: c_int = 13;
pub const PTRACE_SETREGS: c_int = 11;
pub const PTRACE_SINGLESTEP: c_int = 9;
pub const PTRACE_SYSCALL: c_int = 16;
pub const PTRACE_TRACEME: c_int = 0;

// sys/resource.h
pub const RLIMIT_CPU: c_int = 0;
pub const RLIMIT_FSIZE: c_int = 1;
pub const RLIMIT_DATA: c_int = 2;
pub const RLIMIT_STACK: c_int = 3;
pub const RLIMIT_CORE: c_int = 4;
pub const RLIMIT_RSS: c_int = 5;
pub const RLIMIT_AS: c_int = 6;
pub const RLIMIT_NOFILE: c_int = 7;
pub const RLIMIT_THREADS: c_int = 8;
pub const RLIMIT_NPROC: c_int = 9;
pub const RUSAGE_SELF: c_int = 0;
pub const RUSAGE_CHILDREN: c_int = -1;
pub const PRIO_PROCESS: c_int = 0;
pub const PRIO_PGRP: c_int = 1;
pub const PRIO_USER: c_int = 2;
pub const RUSAGE_THREAD: c_int = 1;
pub const RLIM_SAVED_MAX: c_ulong = RLIM_INFINITY - 1;
pub const RLIM_SAVED_CUR: c_ulong = RLIM_INFINITY - 2;
#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIM_NLIMITS: c_int = 10;

// sys/sched.h
pub const SCHED_OTHER: c_int = 0;
pub const SCHED_FIFO: c_int = 1;
pub const SCHED_RR: c_int = 2;
pub const SCHED_LOCAL: c_int = 3;
pub const SCHED_GLOBAL: c_int = 4;
pub const SCHED_FIFO2: c_int = 5;
pub const SCHED_FIFO3: c_int = 6;
pub const SCHED_FIFO4: c_int = 7;

// sys/sem.h
pub const SEM_UNDO: c_int = 0o10000;
pub const GETNCNT: c_int = 3;
pub const GETPID: c_int = 4;
pub const GETVAL: c_int = 5;
pub const GETALL: c_int = 6;
pub const GETZCNT: c_int = 7;
pub const SETVAL: c_int = 8;
pub const SETALL: c_int = 9;

// sys/shm.h
pub const SHMLBA: c_int = 0x10000000;
pub const SHMLBA_EXTSHM: c_int = 0x1000;
pub const SHM_SHMAT: c_int = 0x80000000;
pub const SHM_RDONLY: c_int = 0o10000;
pub const SHM_RND: c_int = 0o20000;
pub const SHM_PIN: c_int = 0o4000;
pub const SHM_LGPAGE: c_int = 0o20000000000;
pub const SHM_MAP: c_int = 0o4000;
pub const SHM_FMAP: c_int = 0o2000;
pub const SHM_COPY: c_int = 0o40000;
pub const SHM_CLEAR: c_int = 0;
pub const SHM_HGSEG: c_int = 0o10000000000;
pub const SHM_R: c_int = IPC_R;
pub const SHM_W: c_int = IPC_W;
pub const SHM_DEST: c_int = 0o2000;

// sys/signal.h
pub const SA_ONSTACK: c_int = 0x00000001;
pub const SA_RESETHAND: c_int = 0x00000002;
pub const SA_RESTART: c_int = 0x00000008;
pub const SA_SIGINFO: c_int = 0x00000100;
pub const SA_NODEFER: c_int = 0x00000200;
pub const SA_NOCLDWAIT: c_int = 0x00000400;
pub const SA_NOCLDSTOP: c_int = 0x00000004;
pub const SS_ONSTACK: c_int = 0x00000001;
pub const SS_DISABLE: c_int = 0x00000002;
pub const SIGCHLD: c_int = 20;
pub const SIGBUS: c_int = 10;
pub const SIG_BLOCK: c_int = 0;
pub const SIG_UNBLOCK: c_int = 1;
pub const SIG_SETMASK: c_int = 2;
pub const SIGEV_NONE: c_int = 1;
pub const SIGEV_SIGNAL: c_int = 2;
pub const SIGEV_THREAD: c_int = 3;
pub const SIGHUP: c_int = 1;
pub const SIGINT: c_int = 2;
pub const SIGQUIT: c_int = 3;
pub const SIGILL: c_int = 4;
pub const SIGABRT: c_int = 6;
pub const SIGEMT: c_int = 7;
pub const SIGFPE: c_int = 8;
pub const SIGKILL: c_int = 9;
pub const SIGSEGV: c_int = 11;
pub const SIGSYS: c_int = 12;
pub const SIGPIPE: c_int = 13;
pub const SIGALRM: c_int = 14;
pub const SIGTERM: c_int = 15;
pub const SIGUSR1: c_int = 30;
pub const SIGUSR2: c_int = 31;
pub const SIGPWR: c_int = 29;
pub const SIGWINCH: c_int = 28;
pub const SIGURG: c_int = 16;
pub const SIGPOLL: c_int = SIGIO;
pub const SIGIO: c_int = 23;
pub const SIGSTOP: c_int = 17;
pub const SIGTSTP: c_int = 18;
pub const SIGCONT: c_int = 19;
pub const SIGTTIN: c_int = 21;
pub const SIGTTOU: c_int = 22;
pub const SIGVTALRM: c_int = 34;
pub const SIGPROF: c_int = 32;
pub const SIGXCPU: c_int = 24;
pub const SIGXFSZ: c_int = 25;
pub const SIGTRAP: c_int = 5;
pub const SIGCLD: c_int = 20;
pub const SIGRTMAX: c_int = 57;
pub const SIGRTMIN: c_int = 50;
pub const SI_USER: c_int = 0;
pub const SI_UNDEFINED: c_int = 8;
pub const SI_EMPTY: c_int = 9;
pub const BUS_ADRALN: c_int = 1;
pub const BUS_ADRERR: c_int = 2;
pub const BUS_OBJERR: c_int = 3;
pub const BUS_UEGARD: c_int = 4;
pub const CLD_EXITED: c_int = 10;
pub const CLD_KILLED: c_int = 11;
pub const CLD_DUMPED: c_int = 12;
pub const CLD_TRAPPED: c_int = 13;
pub const CLD_STOPPED: c_int = 14;
pub const CLD_CONTINUED: c_int = 15;
pub const FPE_INTDIV: c_int = 20;
pub const FPE_INTOVF: c_int = 21;
pub const FPE_FLTDIV: c_int = 22;
pub const FPE_FLTOVF: c_int = 23;
pub const FPE_FLTUND: c_int = 24;
pub const FPE_FLTRES: c_int = 25;
pub const FPE_FLTINV: c_int = 26;
pub const FPE_FLTSUB: c_int = 27;
pub const ILL_ILLOPC: c_int = 30;
pub const ILL_ILLOPN: c_int = 31;
pub const ILL_ILLADR: c_int = 32;
pub const ILL_ILLTRP: c_int = 33;
pub const ILL_PRVOPC: c_int = 34;
pub const ILL_PRVREG: c_int = 35;
pub const ILL_COPROC: c_int = 36;
pub const ILL_BADSTK: c_int = 37;
pub const ILL_TMBADTHING: c_int = 38;
pub const POLL_IN: c_int = 40;
pub const POLL_OUT: c_int = 41;
pub const POLL_MSG: c_int = -3;
pub const POLL_ERR: c_int = 43;
pub const POLL_PRI: c_int = 44;
pub const POLL_HUP: c_int = 45;
pub const SEGV_MAPERR: c_int = 50;
pub const SEGV_ACCERR: c_int = 51;
pub const SEGV_KEYERR: c_int = 52;
pub const TRAP_BRKPT: c_int = 60;
pub const TRAP_TRACE: c_int = 61;
pub const SI_QUEUE: c_int = 71;
pub const SI_TIMER: c_int = 72;
pub const SI_ASYNCIO: c_int = 73;
pub const SI_MESGQ: c_int = 74;

// sys/socket.h
pub const AF_UNSPEC: c_int = 0;
pub const AF_UNIX: c_int = 1;
pub const AF_INET: c_int = 2;
pub const AF_IMPLINK: c_int = 3;
pub const AF_PUP: c_int = 4;
pub const AF_CHAOS: c_int = 5;
pub const AF_NS: c_int = 6;
pub const AF_ECMA: c_int = 8;
pub const AF_DATAKIT: c_int = 9;
pub const AF_CCITT: c_int = 10;
pub const AF_SNA: c_int = 11;
pub const AF_DECnet: c_int = 12;
pub const AF_DLI: c_int = 13;
pub const AF_LAT: c_int = 14;
pub const SO_TIMESTAMPNS: c_int = 0x100a;
pub const SOMAXCONN: c_int = 1024;
pub const AF_LOCAL: c_int = AF_UNIX;
pub const UIO_MAXIOV: c_int = 1024;
pub const pseudo_AF_XTP: c_int = 19;
pub const AF_HYLINK: c_int = 15;
pub const AF_APPLETALK: c_int = 16;
pub const AF_ISO: c_int = 7;
pub const AF_OSI: c_int = AF_ISO;
pub const AF_ROUTE: c_int = 17;
pub const AF_LINK: c_int = 18;
pub const AF_INET6: c_int = 24;
pub const AF_INTF: c_int = 20;
pub const AF_RIF: c_int = 21;
pub const AF_NDD: c_int = 23;
pub const AF_MAX: c_int = 30;
pub const PF_UNSPEC: c_int = AF_UNSPEC;
pub const PF_UNIX: c_int = AF_UNIX;
pub const PF_INET: c_int = AF_INET;
pub const PF_IMPLINK: c_int = AF_IMPLINK;
pub const PF_PUP: c_int = AF_PUP;
pub const PF_CHAOS: c_int = AF_CHAOS;
pub const PF_NS: c_int = AF_NS;
pub const PF_ISO: c_int = AF_ISO;
pub const PF_OSI: c_int = AF_ISO;
pub const PF_ECMA: c_int = AF_ECMA;
pub const PF_DATAKIT: c_int = AF_DATAKIT;
pub const PF_CCITT: c_int = AF_CCITT;
pub const PF_SNA: c_int = AF_SNA;
pub const PF_DECnet: c_int = AF_DECnet;
pub const PF_DLI: c_int = AF_DLI;
pub const PF_LAT: c_int = AF_LAT;
pub const PF_HYLINK: c_int = AF_HYLINK;
pub const PF_APPLETALK: c_int = AF_APPLETALK;
pub const PF_ROUTE: c_int = AF_ROUTE;
pub const PF_LINK: c_int = AF_LINK;
pub const PF_XTP: c_int = 19;
pub const PF_RIF: c_int = AF_RIF;
pub const PF_INTF: c_int = AF_INTF;
pub const PF_NDD: c_int = AF_NDD;
pub const PF_INET6: c_int = AF_INET6;
pub const PF_MAX: c_int = AF_MAX;
pub const SF_CLOSE: c_int = 1;
pub const SF_REUSE: c_int = 2;
pub const SF_DONT_CACHE: c_int = 4;
pub const SF_SYNC_CACHE: c_int = 8;
pub const SOCK_DGRAM: c_int = 2;
pub const SOCK_STREAM: c_int = 1;
pub const SOCK_RAW: c_int = 3;
pub const SOCK_RDM: c_int = 4;
pub const SOCK_SEQPACKET: c_int = 5;
pub const SOL_SOCKET: c_int = 0xffff;
pub const SO_DEBUG: c_int = 0x0001;
pub const SO_ACCEPTCONN: c_int = 0x0002;
pub const SO_REUSEADDR: c_int = 0x0004;
pub const SO_KEEPALIVE: c_int = 0x0008;
pub const SO_DONTROUTE: c_int = 0x0010;
pub const SO_BROADCAST: c_int = 0x0020;
pub const SO_USELOOPBACK: c_int = 0x0040;
pub const SO_LINGER: c_int = 0x0080;
pub const SO_OOBINLINE: c_int = 0x0100;
pub const SO_REUSEPORT: c_int = 0x0200;
pub const SO_USE_IFBUFS: c_int = 0x0400;
pub const SO_CKSUMRECV: c_int = 0x0800;
pub const SO_NOREUSEADDR: c_int = 0x1000;
pub const SO_KERNACCEPT: c_int = 0x2000;
pub const SO_NOMULTIPATH: c_int = 0x4000;
pub const SO_AUDIT: c_int = 0x8000;
pub const SO_SNDBUF: c_int = 0x1001;
pub const SO_RCVBUF: c_int = 0x1002;
pub const SO_SNDLOWAT: c_int = 0x1003;
pub const SO_RCVLOWAT: c_int = 0x1004;
pub const SO_SNDTIMEO: c_int = 0x1005;
pub const SO_RCVTIMEO: c_int = 0x1006;
pub const SO_ERROR: c_int = 0x1007;
pub const SO_TYPE: c_int = 0x1008;
pub const SCM_RIGHTS: c_int = 0x01;
pub const MSG_OOB: c_int = 0x1;
pub const MSG_PEEK: c_int = 0x2;
pub const MSG_DONTROUTE: c_int = 0x4;
pub const MSG_EOR: c_int = 0x8;
pub const MSG_TRUNC: c_int = 0x10;
pub const MSG_CTRUNC: c_int = 0x20;
pub const MSG_WAITALL: c_int = 0x40;
pub const MSG_MPEG2: c_int = 0x80;
pub const MSG_NOSIGNAL: c_int = 0x100;
pub const MSG_WAITFORONE: c_int = 0x200;
pub const MSG_ARGEXT: c_int = 0x400;
pub const MSG_NONBLOCK: c_int = 0x4000;
pub const MSG_COMPAT: c_int = 0x8000;
pub const MSG_MAXIOVLEN: c_int = 16;
pub const SHUT_RD: c_int = 0;
pub const SHUT_WR: c_int = 1;
pub const SHUT_RDWR: c_int = 2;

// sys/stat.h
pub const UTIME_NOW: c_int = -2;
pub const UTIME_OMIT: c_int = -3;

// sys/statvfs.h
pub const ST_RDONLY: c_ulong = 0x0001;
pub const ST_NOSUID: c_ulong = 0x0040;
pub const ST_NODEV: c_ulong = 0x0080;

// sys/stropts.h
pub const I_NREAD: c_int = 0x20005301;
pub const I_PUSH: c_int = 0x20005302;
pub const I_POP: c_int = 0x20005303;
pub const I_LOOK: c_int = 0x20005304;
pub const I_FLUSH: c_int = 0x20005305;
pub const I_SRDOPT: c_int = 0x20005306;
pub const I_GRDOPT: c_int = 0x20005307;
pub const I_STR: c_int = 0x20005308;
pub const I_SETSIG: c_int = 0x20005309;
pub const I_GETSIG: c_int = 0x2000530a;
pub const I_FIND: c_int = 0x2000530b;
pub const I_LINK: c_int = 0x2000530c;
pub const I_UNLINK: c_int = 0x2000530d;
pub const I_PEEK: c_int = 0x2000530f;
pub const I_FDINSERT: c_int = 0x20005310;
pub const I_SENDFD: c_int = 0x20005311;
pub const I_RECVFD: c_int = 0x20005312;
pub const I_SWROPT: c_int = 0x20005314;
pub const I_GWROPT: c_int = 0x20005315;
pub const I_LIST: c_int = 0x20005316;
pub const I_PLINK: c_int = 0x2000531d;
pub const I_PUNLINK: c_int = 0x2000531e;
pub const I_FLUSHBAND: c_int = 0x20005313;
pub const I_CKBAND: c_int = 0x20005318;
pub const I_GETBAND: c_int = 0x20005319;
pub const I_ATMARK: c_int = 0x20005317;
pub const I_SETCLTIME: c_int = 0x2000531b;
pub const I_GETCLTIME: c_int = 0x2000531c;
pub const I_CANPUT: c_int = 0x2000531a;

// sys/syslog.h
pub const LOG_CRON: c_int = 9 << 3;
pub const LOG_AUTHPRIV: c_int = 10 << 3;
pub const LOG_NFACILITIES: c_int = 24;
pub const LOG_PERROR: c_int = 0x20;

// sys/systemcfg.h
pub const SC_ARCH: c_int = 1;
pub const SC_IMPL: c_int = 2;
pub const SC_VERS: c_int = 3;
pub const SC_WIDTH: c_int = 4;
pub const SC_NCPUS: c_int = 5;
pub const SC_L1C_ATTR: c_int = 6;
pub const SC_L1C_ISZ: c_int = 7;
pub const SC_L1C_DSZ: c_int = 8;
pub const SC_L1C_ICA: c_int = 9;
pub const SC_L1C_DCA: c_int = 10;
pub const SC_L1C_IBS: c_int = 11;
pub const SC_L1C_DBS: c_int = 12;
pub const SC_L1C_ILS: c_int = 13;
pub const SC_L1C_DLS: c_int = 14;
pub const SC_L2C_SZ: c_int = 15;
pub const SC_L2C_AS: c_int = 16;
pub const SC_TLB_ATTR: c_int = 17;
pub const SC_ITLB_SZ: c_int = 18;
pub const SC_DTLB_SZ: c_int = 19;
pub const SC_ITLB_ATT: c_int = 20;
pub const SC_DTLB_ATT: c_int = 21;
pub const SC_RESRV_SZ: c_int = 22;
pub const SC_PRI_LC: c_int = 23;
pub const SC_PRO_LC: c_int = 24;
pub const SC_RTC_TYPE: c_int = 25;
pub const SC_VIRT_AL: c_int = 26;
pub const SC_CAC_CONG: c_int = 27;
pub const SC_MOD_ARCH: c_int = 28;
pub const SC_MOD_IMPL: c_int = 29;
pub const SC_XINT: c_int = 30;
pub const SC_XFRAC: c_int = 31;
pub const SC_KRN_ATTR: c_int = 32;
pub const SC_PHYSMEM: c_int = 33;
pub const SC_SLB_ATTR: c_int = 34;
pub const SC_SLB_SZ: c_int = 35;
pub const SC_MAX_NCPUS: c_int = 37;
pub const SC_MAX_REALADDR: c_int = 38;
pub const SC_ORIG_ENT_CAP: c_int = 39;
pub const SC_ENT_CAP: c_int = 40;
pub const SC_DISP_WHE: c_int = 41;
pub const SC_CAPINC: c_int = 42;
pub const SC_VCAPW: c_int = 43;
pub const SC_SPLP_STAT: c_int = 44;
pub const SC_SMT_STAT: c_int = 45;
pub const SC_SMT_TC: c_int = 46;
pub const SC_VMX_VER: c_int = 47;
pub const SC_LMB_SZ: c_int = 48;
pub const SC_MAX_XCPU: c_int = 49;
pub const SC_EC_LVL: c_int = 50;
pub const SC_AME_STAT: c_int = 51;
pub const SC_ECO_STAT: c_int = 52;
pub const SC_DFP_VER: c_int = 53;
pub const SC_VRM_STAT: c_int = 54;
pub const SC_PHYS_IMP: c_int = 55;
pub const SC_PHYS_VER: c_int = 56;
pub const SC_SPCM_STATUS: c_int = 57;
pub const SC_SPCM_MAX: c_int = 58;
pub const SC_TM_VER: c_int = 59;
pub const SC_NX_CAP: c_int = 60;
pub const SC_PKS_STATE: c_int = 61;
pub const SC_MMA_VER: c_int = 62;
pub const POWER_RS: c_int = 1;
pub const POWER_PC: c_int = 2;
pub const IA64: c_int = 3;
pub const POWER_RS1: c_int = 0x1;
pub const POWER_RSC: c_int = 0x2;
pub const POWER_RS2: c_int = 0x4;
pub const POWER_601: c_int = 0x8;
pub const POWER_604: c_int = 0x10;
pub const POWER_603: c_int = 0x20;
pub const POWER_620: c_int = 0x40;
pub const POWER_630: c_int = 0x80;
pub const POWER_A35: c_int = 0x100;
pub const POWER_RS64II: c_int = 0x200;
pub const POWER_RS64III: c_int = 0x400;
pub const POWER_4: c_int = 0x800;
pub const POWER_RS64IV: c_int = POWER_4;
pub const POWER_MPC7450: c_int = 0x1000;
pub const POWER_5: c_int = 0x2000;
pub const POWER_6: c_int = 0x4000;
pub const POWER_7: c_int = 0x8000;
pub const POWER_8: c_int = 0x10000;
pub const POWER_9: c_int = 0x20000;

// sys/time.h
pub const FD_SETSIZE: usize = 65534;
pub const TIMEOFDAY: c_int = 9;
pub const CLOCK_REALTIME: crate::clockid_t = TIMEOFDAY as clockid_t;
pub const CLOCK_MONOTONIC: crate::clockid_t = 10;
pub const TIMER_ABSTIME: c_int = 999;
pub const ITIMER_REAL: c_int = 0;
pub const ITIMER_VIRTUAL: c_int = 1;
pub const ITIMER_PROF: c_int = 2;
pub const ITIMER_VIRT: c_int = 3;
pub const ITIMER_REAL1: c_int = 20;
pub const ITIMER_REAL_TH: c_int = ITIMER_REAL1;
pub const DST_AUST: c_int = 2;
pub const DST_CAN: c_int = 6;
pub const DST_EET: c_int = 5;
pub const DST_MET: c_int = 4;
pub const DST_NONE: c_int = 0;
pub const DST_USA: c_int = 1;
pub const DST_WET: c_int = 3;

// sys/termio.h
pub const CSTART: crate::tcflag_t = 0o21;
pub const CSTOP: crate::tcflag_t = 0o23;
pub const TCGETA: c_int = TIOC | 5;
pub const TCSETA: c_int = TIOC | 6;
pub const TCSETAW: c_int = TIOC | 7;
pub const TCSETAF: c_int = TIOC | 8;
pub const TCSBRK: c_int = TIOC | 9;
pub const TCXONC: c_int = TIOC | 11;
pub const TCFLSH: c_int = TIOC | 12;
pub const TCGETS: c_int = TIOC | 1;
pub const TCSETS: c_int = TIOC | 2;
pub const TCSANOW: c_int = 0;
pub const TCSETSW: c_int = TIOC | 3;
pub const TCSADRAIN: c_int = 1;
pub const TCSETSF: c_int = TIOC | 4;
pub const TCSAFLUSH: c_int = 2;
pub const TCIFLUSH: c_int = 0;
pub const TCOFLUSH: c_int = 1;
pub const TCIOFLUSH: c_int = 2;
pub const TCOOFF: c_int = 0;
pub const TCOON: c_int = 1;
pub const TCIOFF: c_int = 2;
pub const TCION: c_int = 3;
pub const TIOC: c_int = 0x5400;
pub const TIOCGWINSZ: c_int = 0x40087468;
pub const TIOCSWINSZ: c_int = 0x80087467;
pub const TIOCLBIS: c_int = 0x8004747f;
pub const TIOCLBIC: c_int = 0x8004747e;
pub const TIOCLSET: c_int = 0x8004747d;
pub const TIOCLGET: c_int = 0x4004747c;
pub const TIOCSBRK: c_int = 0x2000747b;
pub const TIOCCBRK: c_int = 0x2000747a;
pub const TIOCSDTR: c_int = 0x20007479;
pub const TIOCCDTR: c_int = 0x20007478;
pub const TIOCSLTC: c_int = 0x80067475;
pub const TIOCGLTC: c_int = 0x40067474;
pub const TIOCOUTQ: c_int = 0x40047473;
pub const TIOCNOTTY: c_int = 0x20007471;
pub const TIOCSTOP: c_int = 0x2000746f;
pub const TIOCSTART: c_int = 0x2000746e;
pub const TIOCGPGRP: c_int = 0x40047477;
pub const TIOCSPGRP: c_int = 0x80047476;
pub const TIOCGSID: c_int = 0x40047448;
pub const TIOCSTI: c_int = 0x80017472;
pub const TIOCMSET: c_int = 0x8004746d;
pub const TIOCMBIS: c_int = 0x8004746c;
pub const TIOCMBIC: c_int = 0x8004746b;
pub const TIOCMGET: c_int = 0x4004746a;
pub const TIOCREMOTE: c_int = 0x80047469;

// sys/user.h
pub const MAXCOMLEN: c_int = 32;
pub const UF_SYSTEM: c_int = 0x1000;

// sys/vattr.h
pub const AT_FLAGS: c_int = 0x80;
pub const AT_GID: c_int = 8;
pub const AT_UID: c_int = 4;

// sys/wait.h
pub const P_ALL: idtype_t = 0;
pub const P_PID: idtype_t = 1;
pub const P_PGID: idtype_t = 2;
pub const WNOHANG: c_int = 0x1;
pub const WUNTRACED: c_int = 0x2;
pub const WEXITED: c_int = 0x04;
pub const WCONTINUED: c_int = 0x01000000;
pub const WNOWAIT: c_int = 0x10;
pub const WSTOPPED: c_int = _W_STOPPED;
pub const _W_STOPPED: c_int = 0x00000040;
pub const _W_SLWTED: c_int = 0x0000007c;
pub const _W_SEWTED: c_int = 0x0000007d;
pub const _W_SFWTED: c_int = 0x0000007e;
pub const _W_STRC: c_int = 0x0000007f;

// termios.h
pub const NCCS: usize = 16;
pub const OLCUC: crate::tcflag_t = 2;
pub const CSIZE: crate::tcflag_t = 0x00000030;
pub const CS5: crate::tcflag_t = 0x00000000;
pub const CS6: crate::tcflag_t = 0x00000010;
pub const CS7: crate::tcflag_t = 0x00000020;
pub const CS8: crate::tcflag_t = 0x00000030;
pub const CSTOPB: crate::tcflag_t = 0x00000040;
pub const ECHO: crate::tcflag_t = 0x00000008;
pub const ECHOE: crate::tcflag_t = 0x00000010;
pub const ECHOK: crate::tcflag_t = 0x00000020;
pub const ECHONL: crate::tcflag_t = 0x00000040;
pub const ECHOCTL: crate::tcflag_t = 0x00020000;
pub const ECHOPRT: crate::tcflag_t = 0x00040000;
pub const ECHOKE: crate::tcflag_t = 0x00080000;
pub const IGNBRK: crate::tcflag_t = 0x00000001;
pub const BRKINT: crate::tcflag_t = 0x00000002;
pub const IGNPAR: crate::tcflag_t = 0x00000004;
pub const PARMRK: crate::tcflag_t = 0x00000008;
pub const INPCK: crate::tcflag_t = 0x00000010;
pub const ISTRIP: crate::tcflag_t = 0x00000020;
pub const INLCR: crate::tcflag_t = 0x00000040;
pub const IGNCR: crate::tcflag_t = 0x00000080;
pub const ICRNL: crate::tcflag_t = 0x00000100;
pub const IXON: crate::tcflag_t = 0x00000200;
pub const IXOFF: crate::tcflag_t = 0x00000400;
pub const IXANY: crate::tcflag_t = 0x00001000;
pub const IMAXBEL: crate::tcflag_t = 0x00010000;
pub const OPOST: crate::tcflag_t = 0x00000001;
pub const ONLCR: crate::tcflag_t = 0x00000004;
pub const OCRNL: crate::tcflag_t = 0x00000008;
pub const ONOCR: crate::tcflag_t = 0x00000010;
pub const ONLRET: crate::tcflag_t = 0x00000020;
pub const CREAD: crate::tcflag_t = 0x00000080;
pub const IEXTEN: crate::tcflag_t = 0x00200000;
pub const TOSTOP: crate::tcflag_t = 0x00010000;
pub const FLUSHO: crate::tcflag_t = 0x00100000;
pub const PENDIN: crate::tcflag_t = 0x20000000;
pub const NOFLSH: crate::tcflag_t = 0x00000080;
pub const VINTR: usize = 0;
pub const VQUIT: usize = 1;
pub const VERASE: usize = 2;
pub const VKILL: usize = 3;
pub const VEOF: usize = 4;
pub const VEOL: usize = 5;
pub const VSTART: usize = 7;
pub const VSTOP: usize = 8;
pub const VSUSP: usize = 9;
pub const VMIN: usize = 4;
pub const VTIME: usize = 5;
pub const VEOL2: usize = 6;
pub const VDSUSP: usize = 10;
pub const VREPRINT: usize = 11;
pub const VDISCRD: usize = 12;
pub const VWERSE: usize = 13;
pub const VLNEXT: usize = 14;
pub const B0: crate::speed_t = 0x0;
pub const B50: crate::speed_t = 0x1;
pub const B75: crate::speed_t = 0x2;
pub const B110: crate::speed_t = 0x3;
pub const B134: crate::speed_t = 0x4;
pub const B150: crate::speed_t = 0x5;
pub const B200: crate::speed_t = 0x6;
pub const B300: crate::speed_t = 0x7;
pub const B600: crate::speed_t = 0x8;
pub const B1200: crate::speed_t = 0x9;
pub const B1800: crate::speed_t = 0xa;
pub const B2400: crate::speed_t = 0xb;
pub const B4800: crate::speed_t = 0xc;
pub const B9600: crate::speed_t = 0xd;
pub const B19200: crate::speed_t = 0xe;
pub const B38400: crate::speed_t = 0xf;
pub const EXTA: crate::speed_t = B19200;
pub const EXTB: crate::speed_t = B38400;
pub const IUCLC: crate::tcflag_t = 0x00000800;
pub const OFILL: crate::tcflag_t = 0x00000040;
pub const OFDEL: crate::tcflag_t = 0x00000080;
pub const CRDLY: crate::tcflag_t = 0x00000300;
pub const CR0: crate::tcflag_t = 0x00000000;
pub const CR1: crate::tcflag_t = 0x00000100;
pub const CR2: crate::tcflag_t = 0x00000200;
pub const CR3: crate::tcflag_t = 0x00000300;
pub const TABDLY: crate::tcflag_t = 0x00000c00;
pub const TAB0: crate::tcflag_t = 0x00000000;
pub const TAB1: crate::tcflag_t = 0x00000400;
pub const TAB2: crate::tcflag_t = 0x00000800;
pub const TAB3: crate::tcflag_t = 0x00000c00;
pub const BSDLY: crate::tcflag_t = 0x00001000;
pub const BS0: crate::tcflag_t = 0x00000000;
pub const BS1: crate::tcflag_t = 0x00001000;
pub const FFDLY: crate::tcflag_t = 0x00002000;
pub const FF0: crate::tcflag_t = 0x00000000;
pub const FF1: crate::tcflag_t = 0x00002000;
pub const NLDLY: crate::tcflag_t = 0x00004000;
pub const NL0: crate::tcflag_t = 0x00000000;
pub const NL1: crate::tcflag_t = 0x00004000;
pub const VTDLY: crate::tcflag_t = 0x00008000;
pub const VT0: crate::tcflag_t = 0x00000000;
pub const VT1: crate::tcflag_t = 0x00008000;
pub const OXTABS: crate::tcflag_t = 0x00040000;
pub const ONOEOT: crate::tcflag_t = 0x00080000;
pub const CBAUD: crate::tcflag_t = 0x0000000f;
pub const PARENB: crate::tcflag_t = 0x00000100;
pub const PARODD: crate::tcflag_t = 0x00000200;
pub const HUPCL: crate::tcflag_t = 0x00000400;
pub const CLOCAL: crate::tcflag_t = 0x00000800;
pub const CIBAUD: crate::tcflag_t = 0x000f0000;
pub const IBSHIFT: crate::tcflag_t = 16;
pub const PAREXT: crate::tcflag_t = 0x00100000;
pub const ISIG: crate::tcflag_t = 0x00000001;
pub const ICANON: crate::tcflag_t = 0x00000002;
pub const XCASE: crate::tcflag_t = 0x00000004;
pub const ALTWERASE: crate::tcflag_t = 0x00400000;

// time.h
pub const CLOCK_PROCESS_CPUTIME_ID: crate::clockid_t = 11;
pub const CLOCK_THREAD_CPUTIME_ID: crate::clockid_t = 12;

// unistd.h
pub const STDIN_FILENO: c_int = 0;
pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;
pub const _POSIX_VDISABLE: c_int = 0xff;
pub const _PC_LINK_MAX: c_int = 11;
pub const _PC_MAX_CANON: c_int = 12;
pub const _PC_MAX_INPUT: c_int = 13;
pub const _PC_NAME_MAX: c_int = 14;
pub const _PC_PATH_MAX: c_int = 16;
pub const _PC_PIPE_BUF: c_int = 17;
pub const _PC_NO_TRUNC: c_int = 15;
pub const _PC_VDISABLE: c_int = 18;
pub const _PC_CHOWN_RESTRICTED: c_int = 10;
pub const _PC_ASYNC_IO: c_int = 19;
pub const _PC_PRIO_IO: c_int = 21;
pub const _PC_SYNC_IO: c_int = 20;
pub const _PC_ALLOC_SIZE_MIN: c_int = 26;
pub const _PC_REC_INCR_XFER_SIZE: c_int = 27;
pub const _PC_REC_MAX_XFER_SIZE: c_int = 28;
pub const _PC_REC_MIN_XFER_SIZE: c_int = 29;
pub const _PC_REC_XFER_ALIGN: c_int = 30;
pub const _PC_SYMLINK_MAX: c_int = 25;
pub const _PC_2_SYMLINKS: c_int = 31;
pub const _PC_TIMESTAMP_RESOLUTION: c_int = 32;
pub const _PC_FILESIZEBITS: c_int = 22;
pub const _SC_ARG_MAX: c_int = 0;
pub const _SC_CHILD_MAX: c_int = 1;
pub const _SC_CLK_TCK: c_int = 2;
pub const _SC_NGROUPS_MAX: c_int = 3;
pub const _SC_OPEN_MAX: c_int = 4;
pub const _SC_JOB_CONTROL: c_int = 7;
pub const _SC_SAVED_IDS: c_int = 8;
pub const _SC_VERSION: c_int = 9;
pub const _SC_PASS_MAX: c_int = 45;
pub const _SC_PAGESIZE: c_int = _SC_PAGE_SIZE;
pub const _SC_PAGE_SIZE: c_int = 48;
pub const _SC_XOPEN_VERSION: c_int = 46;
pub const _SC_NPROCESSORS_CONF: c_int = 71;
pub const _SC_NPROCESSORS_ONLN: c_int = 72;
pub const _SC_STREAM_MAX: c_int = 5;
pub const _SC_TZNAME_MAX: c_int = 6;
pub const _SC_AIO_LISTIO_MAX: c_int = 75;
pub const _SC_AIO_MAX: c_int = 76;
pub const _SC_AIO_PRIO_DELTA_MAX: c_int = 77;
pub const _SC_ASYNCHRONOUS_IO: c_int = 78;
pub const _SC_DELAYTIMER_MAX: c_int = 79;
pub const _SC_FSYNC: c_int = 80;
pub const _SC_MAPPED_FILES: c_int = 84;
pub const _SC_MEMLOCK: c_int = 85;
pub const _SC_MEMLOCK_RANGE: c_int = 86;
pub const _SC_MEMORY_PROTECTION: c_int = 87;
pub const _SC_MESSAGE_PASSING: c_int = 88;
pub const _SC_MQ_OPEN_MAX: c_int = 89;
pub const _SC_MQ_PRIO_MAX: c_int = 90;
pub const _SC_PRIORITIZED_IO: c_int = 91;
pub const _SC_PRIORITY_SCHEDULING: c_int = 92;
pub const _SC_REALTIME_SIGNALS: c_int = 93;
pub const _SC_RTSIG_MAX: c_int = 94;
pub const _SC_SEMAPHORES: c_int = 95;
pub const _SC_SEM_NSEMS_MAX: c_int = 96;
pub const _SC_SEM_VALUE_MAX: c_int = 97;
pub const _SC_SHARED_MEMORY_OBJECTS: c_int = 98;
pub const _SC_SIGQUEUE_MAX: c_int = 99;
pub const _SC_SYNCHRONIZED_IO: c_int = 100;
pub const _SC_TIMERS: c_int = 102;
pub const _SC_TIMER_MAX: c_int = 103;
pub const _SC_2_C_BIND: c_int = 51;
pub const _SC_2_C_DEV: c_int = 32;
pub const _SC_2_C_VERSION: c_int = 52;
pub const _SC_2_FORT_DEV: c_int = 33;
pub const _SC_2_FORT_RUN: c_int = 34;
pub const _SC_2_LOCALEDEF: c_int = 35;
pub const _SC_2_SW_DEV: c_int = 36;
pub const _SC_2_UPE: c_int = 53;
pub const _SC_2_VERSION: c_int = 31;
pub const _SC_BC_BASE_MAX: c_int = 23;
pub const _SC_BC_DIM_MAX: c_int = 24;
pub const _SC_BC_SCALE_MAX: c_int = 25;
pub const _SC_BC_STRING_MAX: c_int = 26;
pub const _SC_COLL_WEIGHTS_MAX: c_int = 50;
pub const _SC_EXPR_NEST_MAX: c_int = 28;
pub const _SC_LINE_MAX: c_int = 29;
pub const _SC_RE_DUP_MAX: c_int = 30;
pub const _SC_XOPEN_CRYPT: c_int = 56;
pub const _SC_XOPEN_ENH_I18N: c_int = 57;
pub const _SC_XOPEN_SHM: c_int = 55;
pub const _SC_2_CHAR_TERM: c_int = 54;
pub const _SC_XOPEN_XCU_VERSION: c_int = 109;
pub const _SC_ATEXIT_MAX: c_int = 47;
pub const _SC_IOV_MAX: c_int = 58;
pub const _SC_XOPEN_UNIX: c_int = 73;
pub const _SC_T_IOV_MAX: c_int = 0;
pub const _SC_PHYS_PAGES: c_int = 113;
pub const _SC_AVPHYS_PAGES: c_int = 114;
pub const _SC_THREAD_DESTRUCTOR_ITERATIONS: c_int = 101;
pub const _SC_GETGR_R_SIZE_MAX: c_int = 81;
pub const _SC_GETPW_R_SIZE_MAX: c_int = 82;
pub const _SC_LOGIN_NAME_MAX: c_int = 83;
pub const _SC_THREAD_KEYS_MAX: c_int = 68;
pub const _SC_THREAD_STACK_MIN: c_int = 69;
pub const _SC_THREAD_THREADS_MAX: c_int = 70;
pub const _SC_TTY_NAME_MAX: c_int = 104;
pub const _SC_THREADS: c_int = 60;
pub const _SC_THREAD_ATTR_STACKADDR: c_int = 61;
pub const _SC_THREAD_ATTR_STACKSIZE: c_int = 62;
pub const _SC_THREAD_PRIORITY_SCHEDULING: c_int = 64;
pub const _SC_THREAD_PRIO_INHERIT: c_int = 65;
pub const _SC_THREAD_PRIO_PROTECT: c_int = 66;
pub const _SC_THREAD_PROCESS_SHARED: c_int = 67;
pub const _SC_THREAD_SAFE_FUNCTIONS: c_int = 59;
pub const _SC_XOPEN_LEGACY: c_int = 112;
pub const _SC_XOPEN_REALTIME: c_int = 110;
pub const _SC_XOPEN_REALTIME_THREADS: c_int = 111;
pub const _SC_XBS5_ILP32_OFF32: c_int = 105;
pub const _SC_XBS5_ILP32_OFFBIG: c_int = 106;
pub const _SC_XBS5_LP64_OFF64: c_int = 107;
pub const _SC_XBS5_LPBIG_OFFBIG: c_int = 108;
pub const _SC_2_PBS: c_int = 132;
pub const _SC_2_PBS_ACCOUNTING: c_int = 133;
pub const _SC_2_PBS_CHECKPOINT: c_int = 134;
pub const _SC_2_PBS_LOCATE: c_int = 135;
pub const _SC_2_PBS_MESSAGE: c_int = 136;
pub const _SC_2_PBS_TRACK: c_int = 137;
pub const _SC_ADVISORY_INFO: c_int = 130;
pub const _SC_BARRIERS: c_int = 138;
pub const _SC_CLOCK_SELECTION: c_int = 139;
pub const _SC_CPUTIME: c_int = 140;
pub const _SC_HOST_NAME_MAX: c_int = 126;
pub const _SC_MONOTONIC_CLOCK: c_int = 141;
pub const _SC_READER_WRITER_LOCKS: c_int = 142;
pub const _SC_REGEXP: c_int = 127;
pub const _SC_SHELL: c_int = 128;
pub const _SC_SPAWN: c_int = 143;
pub const _SC_SPIN_LOCKS: c_int = 144;
pub const _SC_SPORADIC_SERVER: c_int = 145;
pub const _SC_SS_REPL_MAX: c_int = 156;
pub const _SC_SYMLOOP_MAX: c_int = 129;
pub const _SC_THREAD_CPUTIME: c_int = 146;
pub const _SC_THREAD_SPORADIC_SERVER: c_int = 147;
pub const _SC_TIMEOUTS: c_int = 148;
pub const _SC_TRACE: c_int = 149;
pub const _SC_TRACE_EVENT_FILTER: c_int = 150;
pub const _SC_TRACE_EVENT_NAME_MAX: c_int = 157;
pub const _SC_TRACE_INHERIT: c_int = 151;
pub const _SC_TRACE_LOG: c_int = 152;
pub const _SC_TRACE_NAME_MAX: c_int = 158;
pub const _SC_TRACE_SYS_MAX: c_int = 159;
pub const _SC_TRACE_USER_EVENT_MAX: c_int = 160;
pub const _SC_TYPED_MEMORY_OBJECTS: c_int = 153;
pub const _SC_V6_ILP32_OFF32: c_int = 121;
pub const _SC_V6_ILP32_OFFBIG: c_int = 122;
pub const _SC_V6_LP64_OFF64: c_int = 123;
pub const _SC_V6_LPBIG_OFFBIG: c_int = 124;
pub const _SC_XOPEN_STREAMS: c_int = 125;
pub const _SC_IPV6: c_int = 154;
pub const _SC_RAW_SOCKETS: c_int = 155;

// utmp.h
pub const EMPTY: c_short = 0;
pub const RUN_LVL: c_short = 1;
pub const BOOT_TIME: c_short = 2;
pub const OLD_TIME: c_short = 3;
pub const NEW_TIME: c_short = 4;
pub const INIT_PROCESS: c_short = 5;
pub const LOGIN_PROCESS: c_short = 6;
pub const USER_PROCESS: c_short = 7;
pub const DEAD_PROCESS: c_short = 8;
pub const ACCOUNTING: c_short = 9;

f! {
    pub fn CMSG_FIRSTHDR(mhdr: *const msghdr) -> *mut cmsghdr {
        if (*mhdr).msg_controllen as usize >= size_of::<cmsghdr>() {
            (*mhdr).msg_control as *mut cmsghdr
        } else {
            core::ptr::null_mut::<cmsghdr>()
        }
    }

    pub fn CMSG_NXTHDR(mhdr: *const msghdr, cmsg: *const cmsghdr) -> *mut cmsghdr {
        if cmsg.is_null() {
            CMSG_FIRSTHDR(mhdr)
        } else {
            if (cmsg as usize + (*cmsg).cmsg_len as usize + size_of::<cmsghdr>())
                > ((*mhdr).msg_control as usize + (*mhdr).msg_controllen as usize)
            {
                core::ptr::null_mut::<cmsghdr>()
            } else {
                // AIX does not have any alignment/padding for ancillary data, so we don't need _CMSG_ALIGN here.
                (cmsg as usize + (*cmsg).cmsg_len as usize) as *mut cmsghdr
            }
        }
    }

    pub fn CMSG_DATA(cmsg: *const cmsghdr) -> *mut c_uchar {
        (cmsg as *mut c_uchar).offset(size_of::<cmsghdr>() as isize)
    }

    pub {const} fn CMSG_LEN(length: c_uint) -> c_uint {
        size_of::<cmsghdr>() as c_uint + length
    }

    pub {const} fn CMSG_SPACE(length: c_uint) -> c_uint {
        size_of::<cmsghdr>() as c_uint + length
    }

    pub fn FD_ZERO(set: *mut fd_set) -> () {
        for slot in (*set).fds_bits.iter_mut() {
            *slot = 0;
        }
    }

    pub fn FD_SET(fd: c_int, set: *mut fd_set) -> () {
        let bits = size_of::<c_long>() * 8;
        let fd = fd as usize;
        (*set).fds_bits[fd / bits] |= 1 << (fd % bits);
        return;
    }

    pub fn FD_CLR(fd: c_int, set: *mut fd_set) -> () {
        let bits = size_of::<c_long>() * 8;
        let fd = fd as usize;
        (*set).fds_bits[fd / bits] &= !(1 << (fd % bits));
        return;
    }

    pub fn FD_ISSET(fd: c_int, set: *const fd_set) -> bool {
        let bits = size_of::<c_long>() * 8;
        let fd = fd as usize;
        return ((*set).fds_bits[fd / bits] & (1 << (fd % bits))) != 0;
    }
}

safe_f! {
    pub {const} fn WIFSTOPPED(status: c_int) -> bool {
        (status & _W_STOPPED) != 0
    }

    pub {const} fn WSTOPSIG(status: c_int) -> c_int {
        if WIFSTOPPED(status) {
            (((status as c_uint) >> 8) & 0xff) as c_int
        } else {
            -1
        }
    }

    pub {const} fn WIFEXITED(status: c_int) -> bool {
        (status & 0xFF) == 0
    }

    pub {const} fn WEXITSTATUS(status: c_int) -> c_int {
        if WIFEXITED(status) {
            (((status as c_uint) >> 8) & 0xff) as c_int
        } else {
            -1
        }
    }

    pub {const} fn WIFSIGNALED(status: c_int) -> bool {
        !WIFEXITED(status) && !WIFSTOPPED(status)
    }

    pub {const} fn WTERMSIG(status: c_int) -> c_int {
        if WIFSIGNALED(status) {
            (((status as c_uint) >> 16) & 0xff) as c_int
        } else {
            -1
        }
    }

    pub {const} fn WIFCONTINUED(status: c_int) -> bool {
        (status & WCONTINUED) != 0
    }

    // AIX doesn't have native WCOREDUMP.
    pub {const} fn WCOREDUMP(_status: c_int) -> bool {
        false
    }

    pub {const} fn major(dev: crate::dev_t) -> c_uint {
        let x = dev >> 16;
        x as c_uint
    }

    pub {const} fn minor(dev: crate::dev_t) -> c_uint {
        let y = dev & 0xFFFF;
        y as c_uint
    }

    pub {const} fn makedev(major: c_uint, minor: c_uint) -> crate::dev_t {
        let major = major as crate::dev_t;
        let minor = minor as crate::dev_t;
        let mut dev = 0;
        dev |= major << 16;
        dev |= minor;
        dev
    }
}

#[link(name = "thread")]
extern "C" {
    pub fn thr_kill(id: thread_t, sig: c_int) -> c_int;
    pub fn thr_self() -> thread_t;
}

#[link(name = "pthread")]
extern "C" {
    pub fn pthread_atfork(
        prepare: Option<unsafe extern "C" fn()>,
        parent: Option<unsafe extern "C" fn()>,
        child: Option<unsafe extern "C" fn()>,
    ) -> c_int;

    pub fn pthread_attr_getdetachstate(
        attr: *const crate::pthread_attr_t,
        detachstate: *mut c_int,
    ) -> c_int;

    pub fn pthread_attr_getguardsize(
        attr: *const crate::pthread_attr_t,
        guardsize: *mut size_t,
    ) -> c_int;

    pub fn pthread_attr_getinheritsched(
        attr: *const crate::pthread_attr_t,
        inheritsched: *mut c_int,
    ) -> c_int;

    pub fn pthread_attr_getschedparam(
        attr: *const crate::pthread_attr_t,
        param: *mut sched_param,
    ) -> c_int;

    pub fn pthread_attr_getstackaddr(
        attr: *const crate::pthread_attr_t,
        stackaddr: *mut *mut c_void,
    ) -> c_int;

    pub fn pthread_attr_getschedpolicy(
        attr: *const crate::pthread_attr_t,
        policy: *mut c_int,
    ) -> c_int;

    pub fn pthread_attr_getscope(
        attr: *const crate::pthread_attr_t,
        contentionscope: *mut c_int,
    ) -> c_int;

    pub fn pthread_attr_getstack(
        attr: *const crate::pthread_attr_t,
        stackaddr: *mut *mut c_void,
        stacksize: *mut size_t,
    ) -> c_int;

    pub fn pthread_attr_setguardsize(attr: *mut crate::pthread_attr_t, guardsize: size_t) -> c_int;

    pub fn pthread_attr_setinheritsched(
        attr: *mut crate::pthread_attr_t,
        inheritsched: c_int,
    ) -> c_int;

    pub fn pthread_attr_setschedparam(
        attr: *mut crate::pthread_attr_t,
        param: *const sched_param,
    ) -> c_int;

    pub fn pthread_attr_setschedpolicy(attr: *mut crate::pthread_attr_t, policy: c_int) -> c_int;

    pub fn pthread_attr_setscope(attr: *mut crate::pthread_attr_t, contentionscope: c_int)
        -> c_int;

    pub fn pthread_attr_setstack(
        attr: *mut crate::pthread_attr_t,
        stackaddr: *mut c_void,
        stacksize: size_t,
    ) -> c_int;

    pub fn pthread_attr_setstackaddr(
        attr: *mut crate::pthread_attr_t,
        stackaddr: *mut c_void,
    ) -> c_int;

    pub fn pthread_barrierattr_destroy(attr: *mut crate::pthread_barrierattr_t) -> c_int;

    pub fn pthread_barrierattr_getpshared(
        attr: *const crate::pthread_barrierattr_t,
        pshared: *mut c_int,
    ) -> c_int;

    pub fn pthread_barrierattr_init(attr: *mut crate::pthread_barrierattr_t) -> c_int;

    pub fn pthread_barrierattr_setpshared(
        attr: *mut crate::pthread_barrierattr_t,
        pshared: c_int,
    ) -> c_int;

    pub fn pthread_barrier_destroy(barrier: *mut pthread_barrier_t) -> c_int;

    pub fn pthread_barrier_init(
        barrier: *mut pthread_barrier_t,
        attr: *const crate::pthread_barrierattr_t,
        count: c_uint,
    ) -> c_int;

    pub fn pthread_barrier_wait(barrier: *mut pthread_barrier_t) -> c_int;

    pub fn pthread_cancel(thread: crate::pthread_t) -> c_int;

    pub fn pthread_cleanup_pop(execute: c_int) -> c_void;

    pub fn pthread_cleanup_push(
        routine: Option<unsafe extern "C" fn(*mut c_void)>,
        arg: *mut c_void,
    ) -> c_void;

    pub fn pthread_condattr_getclock(
        attr: *const pthread_condattr_t,
        clock_id: *mut clockid_t,
    ) -> c_int;

    pub fn pthread_condattr_getpshared(
        attr: *const pthread_condattr_t,
        pshared: *mut c_int,
    ) -> c_int;

    pub fn pthread_condattr_setclock(
        attr: *mut pthread_condattr_t,
        clock_id: crate::clockid_t,
    ) -> c_int;

    pub fn pthread_condattr_setpshared(attr: *mut pthread_condattr_t, pshared: c_int) -> c_int;

    pub fn pthread_create(
        thread: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        start_routine: extern "C" fn(*mut c_void) -> *mut c_void,
        arg: *mut c_void,
    ) -> c_int;

    pub fn pthread_getconcurrency() -> c_int;

    pub fn pthread_getcpuclockid(
        thread_id: crate::pthread_t,
        clock_id: *mut crate::clockid_t,
    ) -> c_int;

    pub fn pthread_getschedparam(
        thread: crate::pthread_t,
        policy: *mut c_int,
        param: *mut sched_param,
    ) -> c_int;

    pub fn pthread_kill(thread: crate::pthread_t, sig: c_int) -> c_int;

    pub fn pthread_mutexattr_getprioceiling(
        attr: *const crate::pthread_mutexattr_t,
        prioceiling: *mut c_int,
    ) -> c_int;

    pub fn pthread_mutexattr_getprotocol(
        attr: *const pthread_mutexattr_t,
        protocol: *mut c_int,
    ) -> c_int;

    pub fn pthread_mutexattr_getpshared(
        attr: *const pthread_mutexattr_t,
        pshared: *mut c_int,
    ) -> c_int;

    pub fn pthread_mutexattr_getrobust(
        attr: *const crate::pthread_mutexattr_t,
        robust: *mut c_int,
    ) -> c_int;

    pub fn pthread_mutexattr_gettype(
        attr: *const crate::pthread_mutexattr_t,
        _type: *mut c_int,
    ) -> c_int;

    pub fn pthread_mutexattr_setprioceiling(
        attr: *mut crate::pthread_mutexattr_t,
        prioceiling: c_int,
    ) -> c_int;

    pub fn pthread_mutexattr_setprotocol(attr: *mut pthread_mutexattr_t, protocol: c_int) -> c_int;

    pub fn pthread_mutexattr_setpshared(attr: *mut pthread_mutexattr_t, pshared: c_int) -> c_int;

    pub fn pthread_mutexattr_setrobust(
        attr: *mut crate::pthread_mutexattr_t,
        robust: c_int,
    ) -> c_int;

    pub fn pthread_mutex_consistent(mutex: *mut crate::pthread_mutex_t) -> c_int;

    pub fn pthread_mutex_getprioceiling(
        mutex: *const crate::pthread_mutex_t,
        prioceiling: *mut c_int,
    ) -> c_int;

    pub fn pthread_mutex_setprioceiling(
        mutex: *mut crate::pthread_mutex_t,
        prioceiling: c_int,
        old_ceiling: *mut c_int,
    ) -> c_int;

    pub fn pthread_mutex_timedlock(
        mutex: *mut pthread_mutex_t,
        abstime: *const crate::timespec,
    ) -> c_int;

    pub fn pthread_once(
        once_control: *mut crate::pthread_once_t,
        init_routine: Option<unsafe extern "C" fn()>,
    ) -> c_int;

    pub fn pthread_rwlockattr_getpshared(
        attr: *const pthread_rwlockattr_t,
        pshared: *mut c_int,
    ) -> c_int;

    pub fn pthread_rwlockattr_setpshared(attr: *mut pthread_rwlockattr_t, pshared: c_int) -> c_int;

    pub fn pthread_rwlock_timedrdlock(
        rwlock: *mut crate::pthread_rwlock_t,
        abstime: *const crate::timespec,
    ) -> c_int;

    pub fn pthread_rwlock_timedwrlock(
        rwlock: *mut crate::pthread_rwlock_t,
        abstime: *const crate::timespec,
    ) -> c_int;

    pub fn pthread_setcancelstate(state: c_int, oldstate: *mut c_int) -> c_int;
    pub fn pthread_setcanceltype(_type: c_int, oldtype: *mut c_int) -> c_int;

    pub fn pthread_setconcurrency(new_level: c_int) -> c_int;

    pub fn pthread_setschedparam(
        thread: crate::pthread_t,
        policy: c_int,
        param: *const sched_param,
    ) -> c_int;

    pub fn pthread_setschedprio(thread: crate::pthread_t, prio: c_int) -> c_int;

    pub fn pthread_sigmask(how: c_int, set: *const sigset_t, oset: *mut sigset_t) -> c_int;

    pub fn pthread_spin_destroy(lock: *mut pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_init(lock: *mut pthread_spinlock_t, pshared: c_int) -> c_int;
    pub fn pthread_spin_lock(lock: *mut pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_trylock(lock: *mut pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_unlock(lock: *mut pthread_spinlock_t) -> c_int;

    pub fn pthread_testcancel() -> c_void;
}

#[link(name = "iconv")]
extern "C" {
    pub fn iconv(
        cd: iconv_t,
        inbuf: *mut *mut c_char,
        inbytesleft: *mut size_t,
        outbuf: *mut *mut c_char,
        outbytesleft: *mut size_t,
    ) -> size_t;
    pub fn iconv_close(cd: iconv_t) -> c_int;
    pub fn iconv_open(tocode: *const c_char, fromcode: *const c_char) -> iconv_t;
}

extern "C" {
    pub fn acct(filename: *mut c_char) -> c_int;
    #[link_name = "_posix_aio_cancel"]
    pub fn aio_cancel(fildes: c_int, aiocbp: *mut crate::aiocb) -> c_int;
    #[link_name = "_posix_aio_error"]
    pub fn aio_error(aiocbp: *const crate::aiocb) -> c_int;
    #[link_name = "_posix_aio_fsync"]
    pub fn aio_fsync(op: c_int, aiocbp: *mut crate::aiocb) -> c_int;
    #[link_name = "_posix_aio_read"]
    pub fn aio_read(aiocbp: *mut crate::aiocb) -> c_int;
    #[link_name = "_posix_aio_return"]
    pub fn aio_return(aiocbp: *mut crate::aiocb) -> ssize_t;
    #[link_name = "_posix_aio_suspend"]
    pub fn aio_suspend(
        list: *const *const crate::aiocb,
        nent: c_int,
        timeout: *const crate::timespec,
    ) -> c_int;
    #[link_name = "_posix_aio_write"]
    pub fn aio_write(aiocbp: *mut crate::aiocb) -> c_int;
    pub fn basename(path: *mut c_char) -> *mut c_char;
    pub fn bind(
        socket: c_int,
        address: *const crate::sockaddr,
        address_len: crate::socklen_t,
    ) -> c_int;
    pub fn brk(addr: *mut c_void) -> c_int;
    pub fn clearenv() -> c_int;
    pub fn clock_getcpuclockid(pid: crate::pid_t, clk_id: *mut crate::clockid_t) -> c_int;
    pub fn clock_getres(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn clock_gettime(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn clock_nanosleep(
        clk_id: crate::clockid_t,
        flags: c_int,
        rqtp: *const crate::timespec,
        rmtp: *mut crate::timespec,
    ) -> c_int;
    pub fn clock_settime(clock_id: crate::clockid_t, tp: *const crate::timespec) -> c_int;
    pub fn creat64(path: *const c_char, mode: mode_t) -> c_int;
    pub fn ctermid(s: *mut c_char) -> *mut c_char;
    pub fn dirfd(dirp: *mut crate::DIR) -> c_int;
    pub fn dirname(path: *mut c_char) -> *mut c_char;
    pub fn drand48() -> c_double;
    pub fn duplocale(arg1: crate::locale_t) -> crate::locale_t;
    pub fn endgrent();
    pub fn endmntent(streamp: *mut crate::FILE) -> c_int;
    pub fn endpwent();
    pub fn endutent();
    pub fn endutxent();
    pub fn erand48(xseed: *mut c_ushort) -> c_double;
    pub fn faccessat(dirfd: c_int, pathname: *const c_char, mode: c_int, flags: c_int) -> c_int;
    pub fn fattach(fildes: c_int, path: *const c_char) -> c_int;
    pub fn fdatasync(fd: c_int) -> c_int;

    // DIFF(main): changed to `*const *mut` in e77f551de9
    pub fn fexecve(fd: c_int, argv: *const *const c_char, envp: *const *const c_char) -> c_int;

    pub fn ffs(value: c_int) -> c_int;
    pub fn ffsl(value: c_long) -> c_int;
    pub fn ffsll(value: c_longlong) -> c_int;
    pub fn fgetgrent(file: *mut crate::FILE) -> *mut crate::group;
    pub fn fgetpos64(stream: *mut crate::FILE, ptr: *mut fpos64_t) -> c_int;
    pub fn fgetpwent(file: *mut crate::FILE) -> *mut crate::passwd;
    pub fn fopen64(filename: *const c_char, mode: *const c_char) -> *mut crate::FILE;
    pub fn freelocale(loc: crate::locale_t);
    pub fn freopen64(
        filename: *const c_char,
        mode: *const c_char,
        file: *mut crate::FILE,
    ) -> *mut crate::FILE;
    pub fn fseeko64(stream: *mut crate::FILE, offset: off64_t, whence: c_int) -> c_int;
    pub fn fsetpos64(stream: *mut crate::FILE, ptr: *const fpos64_t) -> c_int;
    pub fn fstat64(fildes: c_int, buf: *mut stat64) -> c_int;
    pub fn fstatfs(fd: c_int, buf: *mut statfs) -> c_int;
    pub fn fstatfs64(fd: c_int, buf: *mut statfs64) -> c_int;
    pub fn fstatvfs64(fd: c_int, buf: *mut statvfs64) -> c_int;
    pub fn ftello64(stream: *mut crate::FILE) -> off64_t;
    pub fn ftok(path: *const c_char, id: c_int) -> crate::key_t;
    pub fn ftruncate64(fd: c_int, length: off64_t) -> c_int;
    pub fn futimens(fd: c_int, times: *const crate::timespec) -> c_int;
    pub fn getcontext(ucp: *mut ucontext_t) -> c_int;
    pub fn getdomainname(name: *mut c_char, len: c_int) -> c_int;
    pub fn getdtablesize() -> c_int;
    pub fn getgrent() -> *mut crate::group;
    pub fn getgrgid(gid: crate::gid_t) -> *mut crate::group;
    #[link_name = "_posix_getgrgid_r"]
    pub fn getgrgid_r(
        gid: crate::gid_t,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn getgrnam(name: *const c_char) -> *mut crate::group;
    #[link_name = "_posix_getgrnam_r"]
    pub fn getgrnam_r(
        name: *const c_char,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: c_int,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn getgrset(user: *const c_char) -> *mut c_char;
    pub fn gethostid() -> c_long;
    pub fn getmntent(stream: *mut crate::FILE) -> *mut crate::mntent;
    pub fn getnameinfo(
        sa: *const crate::sockaddr,
        salen: size_t,
        host: *mut c_char,
        hostlen: size_t,
        serv: *mut c_char,
        servlen: size_t,
        flags: c_int,
    ) -> c_int;
    pub fn getpagesize() -> c_int;
    pub fn getpeereid(socket: c_int, euid: *mut crate::uid_t, egid: *mut crate::gid_t) -> c_int;
    pub fn getpriority(which: c_int, who: crate::id_t) -> c_int;
    pub fn getpwent() -> *mut crate::passwd;
    #[link_name = "_posix_getpwnam_r"]
    pub fn getpwnam_r(
        name: *const c_char,
        pwd: *mut passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut passwd,
    ) -> c_int;
    #[link_name = "_posix_getpwuid_r"]
    pub fn getpwuid_r(
        uid: crate::uid_t,
        pwd: *mut passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut passwd,
    ) -> c_int;
    pub fn getrlimit(resource: c_int, rlim: *mut crate::rlimit) -> c_int;
    pub fn getrlimit64(resource: c_int, rlim: *mut rlimit64) -> c_int;
    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut c_void) -> c_int;
    pub fn getitimer(which: c_int, curr_value: *mut crate::itimerval) -> c_int;
    pub fn getutent() -> *mut utmp;
    pub fn getutid(u: *const utmp) -> *mut utmp;
    pub fn getutline(u: *const utmp) -> *mut utmp;
    pub fn getutxent() -> *mut utmpx;
    pub fn getutxid(ut: *const utmpx) -> *mut utmpx;
    pub fn getutxline(ut: *const utmpx) -> *mut utmpx;
    pub fn glob(
        pattern: *const c_char,
        flags: c_int,
        errfunc: Option<extern "C" fn(epath: *const c_char, errno: c_int) -> c_int>,
        pglob: *mut crate::glob_t,
    ) -> c_int;
    pub fn globfree(pglob: *mut crate::glob_t);
    pub fn hasmntopt(mnt: *const crate::mntent, opt: *const c_char) -> *mut c_char;
    pub fn hcreate(nelt: size_t) -> c_int;
    pub fn hdestroy();
    pub fn hsearch(entry: entry, action: ACTION) -> *mut entry;
    pub fn if_freenameindex(ptr: *mut if_nameindex);
    pub fn if_nameindex() -> *mut if_nameindex;
    pub fn initgroups(name: *const c_char, basegid: crate::gid_t) -> c_int;
    pub fn ioctl(fildes: c_int, request: c_int, ...) -> c_int;
    pub fn jrand48(xseed: *mut c_ushort) -> c_long;
    pub fn lcong48(p: *mut c_ushort);
    pub fn lfind(
        key: *const c_void,
        base: *const c_void,
        nelp: *mut size_t,
        width: size_t,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>,
    ) -> *mut c_void;
    #[link_name = "_posix_lio_listio"]
    pub fn lio_listio(
        mode: c_int,
        aiocb_list: *const *mut aiocb,
        nent: c_int,
        sevp: *mut sigevent,
    ) -> c_int;
    pub fn loadquery(flags: c_int, buf: *mut c_void, buflen: c_uint, ...) -> c_int;
    pub fn lpar_get_info(command: c_int, buf: *mut c_void, bufsize: size_t) -> c_int;
    pub fn lpar_set_resources(id: c_int, resource: *mut c_void) -> c_int;
    pub fn lrand48() -> c_long;
    pub fn lsearch(
        key: *const c_void,
        base: *mut c_void,
        nelp: *mut size_t,
        width: size_t,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>,
    ) -> *mut c_void;
    pub fn lseek64(fd: c_int, offset: off64_t, whence: c_int) -> off64_t;
    pub fn lstat64(path: *const c_char, buf: *mut stat64) -> c_int;
    pub fn madvise(addr: caddr_t, len: size_t, advice: c_int) -> c_int;
    pub fn makecontext(ucp: *mut crate::ucontext_t, func: extern "C" fn(), argc: c_int, ...);
    pub fn mallinfo() -> crate::mallinfo;
    pub fn mallopt(param: c_int, value: c_int) -> c_int;
    pub fn memmem(
        haystack: *const c_void,
        haystacklen: size_t,
        needle: *const c_void,
        needlelen: size_t,
    ) -> *mut c_void;
    pub fn memset_s(s: *mut c_void, smax: size_t, c: c_int, n: size_t) -> c_int;
    pub fn mincore(addr: caddr_t, len: size_t, vec: *mut c_char) -> c_int;
    pub fn mkfifoat(dirfd: c_int, pathname: *const c_char, mode: mode_t) -> c_int;
    pub fn mknodat(dirfd: c_int, pathname: *const c_char, mode: mode_t, dev: dev_t) -> c_int;
    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;
    pub fn mq_close(mqd: crate::mqd_t) -> c_int;
    pub fn mq_getattr(mqd: crate::mqd_t, attr: *mut crate::mq_attr) -> c_int;
    pub fn mq_notify(mqd: crate::mqd_t, notification: *const crate::sigevent) -> c_int;
    pub fn mq_open(name: *const c_char, oflag: c_int, ...) -> crate::mqd_t;
    pub fn mq_receive(
        mqd: crate::mqd_t,
        msg_ptr: *mut c_char,
        msg_len: size_t,
        msg_prio: *mut c_uint,
    ) -> ssize_t;
    pub fn mq_send(
        mqd: crate::mqd_t,
        msg_ptr: *const c_char,
        msg_len: size_t,
        msg_prio: c_uint,
    ) -> c_int;
    pub fn mq_setattr(
        mqd: crate::mqd_t,
        newattr: *const crate::mq_attr,
        oldattr: *mut crate::mq_attr,
    ) -> c_int;
    pub fn mq_timedreceive(
        mqd: crate::mqd_t,
        msg_ptr: *mut c_char,
        msg_len: size_t,
        msg_prio: *mut c_uint,
        abs_timeout: *const crate::timespec,
    ) -> ssize_t;
    pub fn mq_timedsend(
        mqd: crate::mqd_t,
        msg_ptr: *const c_char,
        msg_len: size_t,
        msg_prio: c_uint,
        abs_timeout: *const crate::timespec,
    ) -> c_int;
    pub fn mq_unlink(name: *const c_char) -> c_int;
    pub fn mrand48() -> c_long;
    pub fn msgctl(msqid: c_int, cmd: c_int, buf: *mut msqid_ds) -> c_int;
    pub fn msgget(key: crate::key_t, msgflg: c_int) -> c_int;
    pub fn msgrcv(
        msqid: c_int,
        msgp: *mut c_void,
        msgsz: size_t,
        msgtyp: c_long,
        msgflg: c_int,
    ) -> ssize_t;
    pub fn msgsnd(msqid: c_int, msgp: *const c_void, msgsz: size_t, msgflg: c_int) -> c_int;
    pub fn msync(addr: *mut c_void, len: size_t, flags: c_int) -> c_int;
    pub fn newlocale(mask: c_int, locale: *const c_char, base: crate::locale_t) -> crate::locale_t;
    pub fn nl_langinfo(item: crate::nl_item) -> *mut c_char;
    pub fn nl_langinfo_l(item: crate::nl_item, loc: crate::locale_t) -> *mut c_char;
    pub fn nrand48(xseed: *mut c_ushort) -> c_long;
    pub fn open64(path: *const c_char, oflag: c_int, ...) -> c_int;
    pub fn pollset_create(maxfd: c_int) -> pollset_t;
    pub fn pollset_ctl(ps: pollset_t, pollctl_array: *mut poll_ctl, array_length: c_int) -> c_int;
    pub fn pollset_destroy(ps: pollset_t) -> c_int;
    pub fn pollset_poll(
        ps: pollset_t,
        polldata_array: *mut crate::pollfd,
        array_length: c_int,
        timeout: c_int,
    ) -> c_int;
    pub fn pollset_query(ps: pollset_t, pollfd_query: *mut crate::pollfd) -> c_int;
    pub fn popen(command: *const c_char, mode: *const c_char) -> *mut crate::FILE;
    pub fn posix_fadvise(fd: c_int, offset: off_t, len: off_t, advise: c_int) -> c_int;
    pub fn posix_fadvise64(fd: c_int, offset: off64_t, len: off64_t, advise: c_int) -> c_int;
    pub fn posix_fallocate(fd: c_int, offset: off_t, len: off_t) -> c_int;
    pub fn posix_fallocate64(fd: c_int, offset: off64_t, len: off64_t) -> c_int;
    pub fn posix_madvise(addr: *mut c_void, len: size_t, advice: c_int) -> c_int;
    pub fn posix_spawn(
        pid: *mut crate::pid_t,
        path: *const c_char,
        file_actions: *const crate::posix_spawn_file_actions_t,
        attrp: *const crate::posix_spawnattr_t,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
    ) -> c_int;
    pub fn posix_spawn_file_actions_addclose(
        actions: *mut posix_spawn_file_actions_t,
        fd: c_int,
    ) -> c_int;
    pub fn posix_spawn_file_actions_adddup2(
        actions: *mut posix_spawn_file_actions_t,
        fd: c_int,
        newfd: c_int,
    ) -> c_int;
    pub fn posix_spawn_file_actions_addopen(
        actions: *mut posix_spawn_file_actions_t,
        fd: c_int,
        path: *const c_char,
        oflag: c_int,
        mode: mode_t,
    ) -> c_int;
    pub fn posix_spawn_file_actions_destroy(actions: *mut posix_spawn_file_actions_t) -> c_int;
    pub fn posix_spawn_file_actions_init(actions: *mut posix_spawn_file_actions_t) -> c_int;
    pub fn posix_spawnattr_destroy(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_getflags(attr: *const posix_spawnattr_t, flags: *mut c_short) -> c_int;
    pub fn posix_spawnattr_getpgroup(
        attr: *const posix_spawnattr_t,
        flags: *mut crate::pid_t,
    ) -> c_int;
    pub fn posix_spawnattr_getschedparam(
        attr: *const posix_spawnattr_t,
        param: *mut crate::sched_param,
    ) -> c_int;
    pub fn posix_spawnattr_getschedpolicy(
        attr: *const posix_spawnattr_t,
        flags: *mut c_int,
    ) -> c_int;
    pub fn posix_spawnattr_getsigdefault(
        attr: *const posix_spawnattr_t,
        default: *mut sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_getsigmask(
        attr: *const posix_spawnattr_t,
        default: *mut sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_init(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_setflags(attr: *mut posix_spawnattr_t, flags: c_short) -> c_int;
    pub fn posix_spawnattr_setpgroup(attr: *mut posix_spawnattr_t, flags: crate::pid_t) -> c_int;
    pub fn posix_spawnattr_setschedparam(
        attr: *mut posix_spawnattr_t,
        param: *const crate::sched_param,
    ) -> c_int;
    pub fn posix_spawnattr_setschedpolicy(attr: *mut posix_spawnattr_t, flags: c_int) -> c_int;
    pub fn posix_spawnattr_setsigdefault(
        attr: *mut posix_spawnattr_t,
        default: *const crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_setsigmask(
        attr: *mut posix_spawnattr_t,
        default: *const crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnp(
        pid: *mut crate::pid_t,
        file: *const c_char,
        file_actions: *const crate::posix_spawn_file_actions_t,
        attrp: *const crate::posix_spawnattr_t,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
    ) -> c_int;
    pub fn pread64(fd: c_int, buf: *mut c_void, count: size_t, offset: off64_t) -> ssize_t;
    pub fn preadv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: offset_t) -> ssize_t;
    pub fn ptrace64(
        request: c_int,
        id: c_longlong,
        addr: c_longlong,
        data: c_int,
        buff: *mut c_int,
    ) -> c_int;
    pub fn pututline(u: *const utmp) -> *mut utmp;
    pub fn pututxline(ut: *const utmpx) -> *mut utmpx;
    pub fn pwrite64(fd: c_int, buf: *const c_void, count: size_t, offset: off64_t) -> ssize_t;
    pub fn pwritev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: offset_t)
        -> ssize_t;
    pub fn quotactl(cmd: *mut c_char, special: c_int, id: c_int, data: caddr_t) -> c_int;
    pub fn rand() -> c_int;
    pub fn readv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;
    // AIX header socket.h maps recvfrom() to nrecvfrom()
    #[link_name = "nrecvfrom"]
    pub fn recvfrom(
        socket: c_int,
        buf: *mut c_void,
        len: size_t,
        flags: c_int,
        addr: *mut crate::sockaddr,
        addrlen: *mut crate::socklen_t,
    ) -> ssize_t;
    // AIX header socket.h maps recvmsg() to nrecvmsg().
    #[link_name = "nrecvmsg"]
    pub fn recvmsg(sockfd: c_int, msg: *mut msghdr, flags: c_int) -> ssize_t;
    pub fn regcomp(preg: *mut regex_t, pattern: *const c_char, cflags: c_int) -> c_int;
    pub fn regerror(
        errcode: c_int,
        preg: *const crate::regex_t,
        errbuf: *mut c_char,
        errbuf_size: size_t,
    ) -> size_t;
    pub fn regexec(
        preg: *const regex_t,
        input: *const c_char,
        nmatch: size_t,
        pmatch: *mut regmatch_t,
        eflags: c_int,
    ) -> c_int;
    pub fn regfree(preg: *mut regex_t);
    pub fn sbrk(increment: intptr_t) -> *mut c_void;
    pub fn sched_getparam(pid: crate::pid_t, param: *mut sched_param) -> c_int;
    pub fn sched_getscheduler(pid: crate::pid_t) -> c_int;
    pub fn sched_get_priority_max(policy: c_int) -> c_int;
    pub fn sched_get_priority_min(policy: c_int) -> c_int;
    pub fn sched_rr_get_interval(pid: crate::pid_t, tp: *mut crate::timespec) -> c_int;
    pub fn sched_setparam(pid: crate::pid_t, param: *const crate::sched_param) -> c_int;
    pub fn sched_setscheduler(
        pid: crate::pid_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;
    pub fn seed48(xseed: *mut c_ushort) -> *mut c_ushort;
    pub fn seekdir(dirp: *mut crate::DIR, loc: c_long);
    pub fn sem_close(sem: *mut sem_t) -> c_int;
    pub fn sem_destroy(sem: *mut sem_t) -> c_int;
    pub fn sem_getvalue(sem: *mut sem_t, sval: *mut c_int) -> c_int;
    pub fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;
    pub fn sem_open(name: *const c_char, oflag: c_int, ...) -> *mut sem_t;
    pub fn sem_timedwait(sem: *mut sem_t, abstime: *const crate::timespec) -> c_int;
    pub fn sem_unlink(name: *const c_char) -> c_int;
    pub fn semctl(semid: c_int, semnum: c_int, cmd: c_int, ...) -> c_int;
    pub fn semget(key: crate::key_t, nsems: c_int, semflag: c_int) -> c_int;
    pub fn semop(semid: c_int, sops: *mut sembuf, nsops: size_t) -> c_int;
    pub fn send_file(socket: *mut c_int, iobuf: *mut sf_parms, flags: c_uint) -> ssize_t;
    // AIX header socket.h maps sendmsg() to nsendmsg().
    #[link_name = "nsendmsg"]
    pub fn sendmsg(sockfd: c_int, msg: *const msghdr, flags: c_int) -> ssize_t;
    pub fn setcontext(ucp: *const ucontext_t) -> c_int;
    pub fn setdomainname(name: *const c_char, len: c_int) -> c_int;
    pub fn setgroups(ngroups: c_int, ptr: *const crate::gid_t) -> c_int;
    pub fn setgrent();
    pub fn setmntent(filename: *const c_char, ty: *const c_char) -> *mut crate::FILE;
    pub fn setpriority(which: c_int, who: id_t, priority: c_int) -> c_int;
    pub fn setpwent();
    pub fn setrlimit(resource: c_int, rlim: *const crate::rlimit) -> c_int;
    pub fn setrlimit64(resource: c_int, rlim: *const rlimit64) -> c_int;
    pub fn settimeofday(tv: *const crate::timeval, tz: *const crate::timezone) -> c_int;
    pub fn setitimer(
        which: c_int,
        new_value: *const crate::itimerval,
        old_value: *mut crate::itimerval,
    ) -> c_int;
    pub fn setutent();
    pub fn setutxent();
    pub fn sigaltstack(ss: *const stack_t, oss: *mut stack_t) -> c_int;
    pub fn sigsuspend(mask: *const crate::sigset_t) -> c_int;
    pub fn sigtimedwait(
        set: *const sigset_t,
        info: *mut siginfo_t,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn sigwait(set: *const sigset_t, sig: *mut c_int) -> c_int;
    pub fn sigwaitinfo(set: *const sigset_t, info: *mut siginfo_t) -> c_int;
    pub fn shmat(shmid: c_int, shmaddr: *const c_void, shmflg: c_int) -> *mut c_void;
    pub fn shmdt(shmaddr: *const c_void) -> c_int;
    pub fn shmctl(shmid: c_int, cmd: c_int, buf: *mut crate::shmid_ds) -> c_int;
    pub fn shmget(key: key_t, size: size_t, shmflg: c_int) -> c_int;
    pub fn shm_open(name: *const c_char, oflag: c_int, mode: mode_t) -> c_int;
    pub fn shm_unlink(name: *const c_char) -> c_int;
    pub fn srand(seed: c_uint);
    pub fn srand48(seed: c_long);
    pub fn stat64(path: *const c_char, buf: *mut stat64) -> c_int;
    pub fn stat64at(dirfd: c_int, path: *const c_char, buf: *mut stat64, flags: c_int) -> c_int;
    pub fn statfs(path: *const c_char, buf: *mut statfs) -> c_int;
    pub fn statfs64(path: *const c_char, buf: *mut statfs64) -> c_int;
    pub fn statvfs64(path: *const c_char, buf: *mut statvfs64) -> c_int;
    pub fn statx(path: *const c_char, buf: *mut stat, length: c_int, command: c_int) -> c_int;
    pub fn strcasecmp_l(
        string1: *const c_char,
        string2: *const c_char,
        locale: crate::locale_t,
    ) -> c_int;
    pub fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;
    pub fn strftime(
        arg1: *mut c_char,
        arg2: size_t,
        arg3: *const c_char,
        arg4: *const tm,
    ) -> size_t;
    pub fn strncasecmp_l(
        string1: *const c_char,
        string2: *const c_char,
        length: size_t,
        locale: crate::locale_t,
    ) -> c_int;
    pub fn strptime(s: *const c_char, format: *const c_char, tm: *mut crate::tm) -> *mut c_char;
    pub fn strsep(string: *mut *mut c_char, delim: *const c_char) -> *mut c_char;
    pub fn swapcontext(uocp: *mut ucontext_t, ucp: *const ucontext_t) -> c_int;
    pub fn swapoff(path: *const c_char) -> c_int;
    pub fn swapon(path: *const c_char) -> c_int;
    pub fn sync();
    pub fn telldir(dirp: *mut crate::DIR) -> c_long;
    pub fn timer_create(
        clockid: crate::clockid_t,
        sevp: *mut crate::sigevent,
        timerid: *mut crate::timer_t,
    ) -> c_int;
    pub fn timer_delete(timerid: timer_t) -> c_int;
    pub fn timer_getoverrun(timerid: timer_t) -> c_int;
    pub fn timer_gettime(timerid: timer_t, value: *mut itimerspec) -> c_int;
    pub fn timer_settime(
        timerid: crate::timer_t,
        flags: c_int,
        new_value: *const crate::itimerspec,
        old_value: *mut crate::itimerspec,
    ) -> c_int;
    pub fn truncate64(path: *const c_char, length: off64_t) -> c_int;
    pub fn uname(buf: *mut crate::utsname) -> c_int;
    pub fn updwtmp(file: *const c_char, u: *const utmp);
    pub fn uselocale(loc: crate::locale_t) -> crate::locale_t;
    pub fn utmpname(file: *const c_char) -> c_int;
    pub fn utimensat(
        dirfd: c_int,
        path: *const c_char,
        times: *const crate::timespec,
        flag: c_int,
    ) -> c_int;
    pub fn wait4(
        pid: crate::pid_t,
        status: *mut c_int,
        options: c_int,
        rusage: *mut crate::rusage,
    ) -> crate::pid_t;
    pub fn waitid(
        idtype: idtype_t,
        id: id_t,
        infop: *mut crate::siginfo_t,
        options: c_int,
    ) -> c_int;
    pub fn writev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;

    // Use AIX thread-safe version errno.
    pub fn _Errno() -> *mut c_int;
}

cfg_if! {
    if #[cfg(target_arch = "powerpc64")] {
        mod powerpc64;
        pub use self::powerpc64::*;
    }
}
