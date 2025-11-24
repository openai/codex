#![allow(dead_code)]

use crate::c_schar;
use crate::prelude::*;

// types
pub type __s16_type = c_short;
pub type __u16_type = c_ushort;
pub type __s32_type = c_int;
pub type __u32_type = c_uint;
pub type __slongword_type = c_long;
pub type __ulongword_type = c_ulong;

pub type __u_char = c_uchar;
pub type __u_short = c_ushort;
pub type __u_int = c_uint;
pub type __u_long = c_ulong;
pub type __int8_t = c_schar;
pub type __uint8_t = c_uchar;
pub type __int16_t = c_short;
pub type __uint16_t = c_ushort;
pub type __int32_t = c_int;
pub type __uint32_t = c_uint;
pub type __int_least8_t = __int8_t;
pub type __uint_least8_t = __uint8_t;
pub type __int_least16_t = __int16_t;
pub type __uint_least16_t = __uint16_t;
pub type __int_least32_t = __int32_t;
pub type __uint_least32_t = __uint32_t;
pub type __int_least64_t = __int64_t;
pub type __uint_least64_t = __uint64_t;

pub type __dev_t = __uword_type;
pub type __uid_t = __u32_type;
pub type __gid_t = __u32_type;
pub type __ino_t = __ulongword_type;
pub type __ino64_t = __uquad_type;
pub type __mode_t = __u32_type;
pub type __nlink_t = __uword_type;
pub type __off_t = __slongword_type;
pub type __off64_t = __squad_type;
pub type __pid_t = __s32_type;
pub type __rlim_t = __ulongword_type;
pub type __rlim64_t = __uquad_type;
pub type __blkcnt_t = __slongword_type;
pub type __blkcnt64_t = __squad_type;
pub type __fsblkcnt_t = __ulongword_type;
pub type __fsblkcnt64_t = __uquad_type;
pub type __fsfilcnt_t = __ulongword_type;
pub type __fsfilcnt64_t = __uquad_type;
pub type __fsword_t = __sword_type;
pub type __id_t = __u32_type;
pub type __clock_t = __slongword_type;
pub type __time_t = __slongword_type;
pub type __useconds_t = __u32_type;
pub type __suseconds_t = __slongword_type;
pub type __suseconds64_t = __squad_type;
pub type __daddr_t = __s32_type;
pub type __key_t = __s32_type;
pub type __clockid_t = __s32_type;
pub type __timer_t = __uword_type;
pub type __blksize_t = __slongword_type;
pub type __fsid_t = __uquad_type;
pub type __ssize_t = __sword_type;
pub type __syscall_slong_t = __slongword_type;
pub type __syscall_ulong_t = __ulongword_type;
pub type __cpu_mask = __ulongword_type;

pub type __loff_t = __off64_t;
pub type __caddr_t = *mut c_char;
pub type __intptr_t = __sword_type;
pub type __ptrdiff_t = __sword_type;
pub type __socklen_t = __u32_type;
pub type __sig_atomic_t = c_int;
pub type __time64_t = __int64_t;
pub type wchar_t = c_int;
pub type wint_t = c_uint;
pub type gid_t = __gid_t;
pub type uid_t = __uid_t;
pub type off_t = __off_t;
pub type off64_t = __off64_t;
pub type useconds_t = __useconds_t;
pub type pid_t = __pid_t;
pub type socklen_t = __socklen_t;

pub type in_addr_t = u32;

pub type _Float32 = f32;
pub type _Float64 = f64;
pub type _Float32x = f64;
pub type _Float64x = f64;

pub type __locale_t = *mut __locale_struct;
pub type locale_t = __locale_t;

pub type u_char = __u_char;
pub type u_short = __u_short;
pub type u_int = __u_int;
pub type u_long = __u_long;
pub type quad_t = __quad_t;
pub type u_quad_t = __u_quad_t;
pub type fsid_t = __fsid_t;
pub type loff_t = __loff_t;
pub type ino_t = __ino_t;
pub type ino64_t = __ino64_t;
pub type dev_t = __dev_t;
pub type mode_t = __mode_t;
pub type nlink_t = __nlink_t;
pub type id_t = __id_t;
pub type daddr_t = __daddr_t;
pub type caddr_t = __caddr_t;
pub type key_t = __key_t;
pub type clock_t = __clock_t;
pub type clockid_t = __clockid_t;
pub type time_t = __time_t;
pub type timer_t = __timer_t;
pub type suseconds_t = __suseconds_t;
pub type ulong = c_ulong;
pub type ushort = c_ushort;
pub type uint = c_uint;
pub type u_int8_t = __uint8_t;
pub type u_int16_t = __uint16_t;
pub type u_int32_t = __uint32_t;
pub type u_int64_t = __uint64_t;
pub type register_t = c_int;
pub type __sigset_t = c_ulong;
pub type sigset_t = __sigset_t;

pub type __fd_mask = c_long;
pub type fd_mask = __fd_mask;
pub type blksize_t = __blksize_t;
pub type blkcnt_t = __blkcnt_t;
pub type fsblkcnt_t = __fsblkcnt_t;
pub type fsfilcnt_t = __fsfilcnt_t;
pub type blkcnt64_t = __blkcnt64_t;
pub type fsblkcnt64_t = __fsblkcnt64_t;
pub type fsfilcnt64_t = __fsfilcnt64_t;

pub type __pthread_spinlock_t = c_int;
pub type __tss_t = c_int;
pub type __thrd_t = c_long;
pub type __pthread_t = c_long;
pub type pthread_t = __pthread_t;
pub type __pthread_process_shared = c_uint;
pub type __pthread_inheritsched = c_uint;
pub type __pthread_contentionscope = c_uint;
pub type __pthread_detachstate = c_uint;
pub type pthread_attr_t = __pthread_attr;
pub type __pthread_mutex_protocol = c_uint;
pub type __pthread_mutex_type = c_uint;
pub type __pthread_mutex_robustness = c_uint;
pub type pthread_mutexattr_t = __pthread_mutexattr;
pub type pthread_mutex_t = __pthread_mutex;
pub type pthread_condattr_t = __pthread_condattr;
pub type pthread_cond_t = __pthread_cond;
pub type pthread_spinlock_t = __pthread_spinlock_t;
pub type pthread_rwlockattr_t = __pthread_rwlockattr;
pub type pthread_rwlock_t = __pthread_rwlock;
pub type pthread_barrierattr_t = __pthread_barrierattr;
pub type pthread_barrier_t = __pthread_barrier;
pub type __pthread_key = c_int;
pub type pthread_key_t = __pthread_key;
pub type pthread_once_t = __pthread_once;

pub type __rlimit_resource = c_uint;
pub type __rlimit_resource_t = __rlimit_resource;
pub type rlim_t = __rlim_t;
pub type rlim64_t = __rlim64_t;

pub type __rusage_who = c_int;

pub type __priority_which = c_uint;

pub type sa_family_t = c_uchar;

pub type in_port_t = u16;

pub type __sigval_t = crate::sigval;

pub type sigevent_t = sigevent;

pub type nfds_t = c_ulong;

pub type tcflag_t = c_uint;
pub type cc_t = c_uchar;
pub type speed_t = c_int;

pub type sigval_t = crate::sigval;

pub type greg_t = c_int;
pub type gregset_t = [greg_t; 19usize];

pub type __ioctl_dir = c_uint;

pub type __ioctl_datum = c_uint;

pub type __error_t_codes = c_int;

pub type int_least8_t = __int_least8_t;
pub type int_least16_t = __int_least16_t;
pub type int_least32_t = __int_least32_t;
pub type int_least64_t = __int_least64_t;
pub type uint_least8_t = __uint_least8_t;
pub type uint_least16_t = __uint_least16_t;
pub type uint_least32_t = __uint_least32_t;
pub type uint_least64_t = __uint_least64_t;
pub type int_fast8_t = c_schar;
pub type uint_fast8_t = c_uchar;
pub type intmax_t = __intmax_t;
pub type uintmax_t = __uintmax_t;

pub type tcp_seq = u32;

pub type tcp_ca_state = c_uint;

pub type idtype_t = c_uint;

pub type mqd_t = c_int;

pub type Lmid_t = c_long;

pub type regoff_t = c_int;

pub type nl_item = c_int;

pub type iconv_t = *mut c_void;

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum fpos64_t {} // FIXME(hurd): fill this out with a struct
impl Copy for fpos64_t {}
impl Clone for fpos64_t {
    fn clone(&self) -> fpos64_t {
        *self
    }
}

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum timezone {}
impl Copy for timezone {}
impl Clone for timezone {
    fn clone(&self) -> timezone {
        *self
    }
}

// structs
s! {
    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct ip_mreqn {
        pub imr_multiaddr: in_addr,
        pub imr_address: in_addr,
        pub imr_ifindex: c_int,
    }

    pub struct ip_mreq_source {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
        pub imr_sourceaddr: in_addr,
    }

    pub struct sockaddr {
        pub sa_len: c_uchar,
        pub sa_family: sa_family_t,
        pub sa_data: [c_char; 14usize],
    }

    pub struct in_addr {
        pub s_addr: in_addr_t,
    }

    pub struct sockaddr_in {
        pub sin_len: c_uchar,
        pub sin_family: sa_family_t,
        pub sin_port: in_port_t,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [c_uchar; 8usize],
    }

    pub struct sockaddr_in6 {
        pub sin6_len: c_uchar,
        pub sin6_family: sa_family_t,
        pub sin6_port: in_port_t,
        pub sin6_flowinfo: u32,
        pub sin6_addr: crate::in6_addr,
        pub sin6_scope_id: u32,
    }

    pub struct sockaddr_un {
        pub sun_len: c_uchar,
        pub sun_family: sa_family_t,
        pub sun_path: [c_char; 108usize],
    }

    pub struct sockaddr_storage {
        pub ss_len: c_uchar,
        pub ss_family: sa_family_t,
        pub __ss_padding: [c_char; 122usize],
        pub __ss_align: __uint32_t,
    }

    pub struct sockaddr_at {
        pub _address: u8,
    }

    pub struct sockaddr_ax25 {
        pub _address: u8,
    }

    pub struct sockaddr_x25 {
        pub _address: u8,
    }

    pub struct sockaddr_dl {
        pub _address: u8,
    }
    pub struct sockaddr_eon {
        pub _address: u8,
    }
    pub struct sockaddr_inarp {
        pub _address: u8,
    }

    pub struct sockaddr_ipx {
        pub _address: u8,
    }
    pub struct sockaddr_iso {
        pub _address: u8,
    }

    pub struct sockaddr_ns {
        pub _address: u8,
    }

    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        pub ai_addrlen: crate::socklen_t,
        pub ai_addr: *mut sockaddr,
        pub ai_canonname: *mut c_char,
        pub ai_next: *mut addrinfo,
    }

    pub struct msghdr {
        pub msg_name: *mut c_void,
        pub msg_namelen: crate::socklen_t,
        pub msg_iov: *mut crate::iovec,
        pub msg_iovlen: c_int,
        pub msg_control: *mut c_void,
        pub msg_controllen: crate::socklen_t,
        pub msg_flags: c_int,
    }

    pub struct cmsghdr {
        pub cmsg_len: crate::socklen_t,
        pub cmsg_level: c_int,
        pub cmsg_type: c_int,
    }

    pub struct dirent {
        pub d_ino: __ino_t,
        pub d_reclen: c_ushort,
        pub d_type: c_uchar,
        pub d_namlen: c_uchar,
        pub d_name: [c_char; 1usize],
    }

    pub struct dirent64 {
        pub d_ino: __ino64_t,
        pub d_reclen: c_ushort,
        pub d_type: c_uchar,
        pub d_namlen: c_uchar,
        pub d_name: [c_char; 1usize],
    }

    pub struct fd_set {
        pub fds_bits: [__fd_mask; 8usize],
    }

    pub struct termios {
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_cc: [crate::cc_t; 20usize],
        pub __ispeed: crate::speed_t,
        pub __ospeed: crate::speed_t,
    }

    pub struct mallinfo {
        pub arena: c_int,
        pub ordblks: c_int,
        pub smblks: c_int,
        pub hblks: c_int,
        pub hblkhd: c_int,
        pub usmblks: c_int,
        pub fsmblks: c_int,
        pub uordblks: c_int,
        pub fordblks: c_int,
        pub keepcost: c_int,
    }

    pub struct mallinfo2 {
        pub arena: size_t,
        pub ordblks: size_t,
        pub smblks: size_t,
        pub hblks: size_t,
        pub hblkhd: size_t,
        pub usmblks: size_t,
        pub fsmblks: size_t,
        pub uordblks: size_t,
        pub fordblks: size_t,
        pub keepcost: size_t,
    }

    pub struct sigaction {
        pub sa_sigaction: crate::sighandler_t,
        pub sa_mask: __sigset_t,
        pub sa_flags: c_int,
    }

    pub struct sigevent {
        pub sigev_value: crate::sigval,
        pub sigev_signo: c_int,
        pub sigev_notify: c_int,
        __unused1: *mut c_void, //actually a function pointer
        pub sigev_notify_attributes: *mut pthread_attr_t,
    }

    pub struct siginfo_t {
        pub si_signo: c_int,
        pub si_errno: c_int,
        pub si_code: c_int,
        pub si_pid: __pid_t,
        pub si_uid: __uid_t,
        pub si_addr: *mut c_void,
        pub si_status: c_int,
        pub si_band: c_long,
        pub si_value: crate::sigval,
    }

    pub struct timespec {
        pub tv_sec: __time_t,
        pub tv_nsec: __syscall_slong_t,
    }

    pub struct __timeval {
        pub tv_sec: i32,
        pub tv_usec: i32,
    }

    pub struct __locale_data {
        pub _address: u8,
    }

    pub struct stat {
        pub st_fstype: c_int,
        pub st_dev: __fsid_t, /* Actually st_fsid */
        pub st_ino: __ino_t,
        pub st_gen: c_uint,
        pub st_rdev: __dev_t,
        pub st_mode: __mode_t,
        pub st_nlink: __nlink_t,
        pub st_uid: __uid_t,
        pub st_gid: __gid_t,
        pub st_size: __off_t,
        pub st_atim: crate::timespec,
        pub st_mtim: crate::timespec,
        pub st_ctim: crate::timespec,
        pub st_blksize: __blksize_t,
        pub st_blocks: __blkcnt_t,
        pub st_author: __uid_t,
        pub st_flags: c_uint,
        pub st_spare: [c_int; 11usize],
    }

    pub struct stat64 {
        pub st_fstype: c_int,
        pub st_dev: __fsid_t, /* Actually st_fsid */
        pub st_ino: __ino64_t,
        pub st_gen: c_uint,
        pub st_rdev: __dev_t,
        pub st_mode: __mode_t,
        pub st_nlink: __nlink_t,
        pub st_uid: __uid_t,
        pub st_gid: __gid_t,
        pub st_size: __off64_t,
        pub st_atim: crate::timespec,
        pub st_mtim: crate::timespec,
        pub st_ctim: crate::timespec,
        pub st_blksize: __blksize_t,
        pub st_blocks: __blkcnt64_t,
        pub st_author: __uid_t,
        pub st_flags: c_uint,
        pub st_spare: [c_int; 8usize],
    }

    pub struct statx {
        pub stx_mask: u32,
        pub stx_blksize: u32,
        pub stx_attributes: u64,
        pub stx_nlink: u32,
        pub stx_uid: u32,
        pub stx_gid: u32,
        pub stx_mode: u16,
        __statx_pad1: [u16; 1],
        pub stx_ino: u64,
        pub stx_size: u64,
        pub stx_blocks: u64,
        pub stx_attributes_mask: u64,
        pub stx_atime: crate::statx_timestamp,
        pub stx_btime: crate::statx_timestamp,
        pub stx_ctime: crate::statx_timestamp,
        pub stx_mtime: crate::statx_timestamp,
        pub stx_rdev_major: u32,
        pub stx_rdev_minor: u32,
        pub stx_dev_major: u32,
        pub stx_dev_minor: u32,
        __statx_pad2: [u64; 14],
    }

    pub struct statx_timestamp {
        pub tv_sec: i64,
        pub tv_nsec: u32,
        pub __statx_timestamp_pad1: [i32; 1],
    }

    pub struct statfs {
        pub f_type: c_uint,
        pub f_bsize: c_ulong,
        pub f_blocks: __fsblkcnt_t,
        pub f_bfree: __fsblkcnt_t,
        pub f_bavail: __fsblkcnt_t,
        pub f_files: __fsblkcnt_t,
        pub f_ffree: __fsblkcnt_t,
        pub f_fsid: __fsid_t,
        pub f_namelen: c_ulong,
        pub f_favail: __fsfilcnt_t,
        pub f_frsize: c_ulong,
        pub f_flag: c_ulong,
        pub f_spare: [c_uint; 3usize],
    }

    pub struct statfs64 {
        pub f_type: c_uint,
        pub f_bsize: c_ulong,
        pub f_blocks: __fsblkcnt64_t,
        pub f_bfree: __fsblkcnt64_t,
        pub f_bavail: __fsblkcnt64_t,
        pub f_files: __fsblkcnt64_t,
        pub f_ffree: __fsblkcnt64_t,
        pub f_fsid: __fsid_t,
        pub f_namelen: c_ulong,
        pub f_favail: __fsfilcnt64_t,
        pub f_frsize: c_ulong,
        pub f_flag: c_ulong,
        pub f_spare: [c_uint; 3usize],
    }

    pub struct statvfs {
        pub __f_type: c_uint,
        pub f_bsize: c_ulong,
        pub f_blocks: __fsblkcnt_t,
        pub f_bfree: __fsblkcnt_t,
        pub f_bavail: __fsblkcnt_t,
        pub f_files: __fsfilcnt_t,
        pub f_ffree: __fsfilcnt_t,
        pub f_fsid: __fsid_t,
        pub f_namemax: c_ulong,
        pub f_favail: __fsfilcnt_t,
        pub f_frsize: c_ulong,
        pub f_flag: c_ulong,
        pub f_spare: [c_uint; 3usize],
    }

    pub struct statvfs64 {
        pub __f_type: c_uint,
        pub f_bsize: c_ulong,
        pub f_blocks: __fsblkcnt64_t,
        pub f_bfree: __fsblkcnt64_t,
        pub f_bavail: __fsblkcnt64_t,
        pub f_files: __fsfilcnt64_t,
        pub f_ffree: __fsfilcnt64_t,
        pub f_fsid: __fsid_t,
        pub f_namemax: c_ulong,
        pub f_favail: __fsfilcnt64_t,
        pub f_frsize: c_ulong,
        pub f_flag: c_ulong,
        pub f_spare: [c_uint; 3usize],
    }

    pub struct aiocb {
        pub aio_fildes: c_int,
        pub aio_lio_opcode: c_int,
        pub aio_reqprio: c_int,
        pub aio_buf: *mut c_void,
        pub aio_nbytes: size_t,
        pub aio_sigevent: crate::sigevent,
        __next_prio: *mut aiocb,
        __abs_prio: c_int,
        __policy: c_int,
        __error_code: c_int,
        __return_value: ssize_t,
        pub aio_offset: off_t,
        #[cfg(all(not(target_arch = "x86_64"), target_pointer_width = "32"))]
        __unused1: [c_char; 4],
        __glibc_reserved: [c_char; 32],
    }

    pub struct mq_attr {
        pub mq_flags: c_long,
        pub mq_maxmsg: c_long,
        pub mq_msgsize: c_long,
        pub mq_curmsgs: c_long,
    }

    pub struct __exit_status {
        pub e_termination: c_short,
        pub e_exit: c_short,
    }

    #[cfg_attr(target_pointer_width = "32", repr(align(4)))]
    #[cfg_attr(target_pointer_width = "64", repr(align(8)))]
    pub struct sem_t {
        __size: [c_char; 20usize],
    }

    pub struct __pthread {
        pub _address: u8,
    }

    pub struct __pthread_mutexattr {
        pub __prioceiling: c_int,
        pub __protocol: __pthread_mutex_protocol,
        pub __pshared: __pthread_process_shared,
        pub __mutex_type: __pthread_mutex_type,
    }
    pub struct __pthread_mutex {
        pub __lock: c_uint,
        pub __owner_id: c_uint,
        pub __cnt: c_uint,
        pub __shpid: c_int,
        pub __type: c_int,
        pub __flags: c_int,
        pub __reserved1: c_uint,
        pub __reserved2: c_uint,
    }

    pub struct __pthread_condattr {
        pub __pshared: __pthread_process_shared,
        pub __clock: __clockid_t,
    }

    pub struct __pthread_rwlockattr {
        pub __pshared: __pthread_process_shared,
    }

    pub struct __pthread_barrierattr {
        pub __pshared: __pthread_process_shared,
    }

    pub struct __pthread_once {
        pub __run: c_int,
        pub __lock: __pthread_spinlock_t,
    }

    pub struct __pthread_cond {
        pub __lock: __pthread_spinlock_t,
        pub __queue: *mut __pthread,
        pub __attr: *mut __pthread_condattr,
        pub __wrefs: c_uint,
        pub __data: *mut c_void,
    }

    pub struct __pthread_attr {
        pub __schedparam: sched_param,
        pub __stackaddr: *mut c_void,
        pub __stacksize: size_t,
        pub __guardsize: size_t,
        pub __detachstate: __pthread_detachstate,
        pub __inheritsched: __pthread_inheritsched,
        pub __contentionscope: __pthread_contentionscope,
        pub __schedpolicy: c_int,
    }

    pub struct __pthread_rwlock {
        pub __held: __pthread_spinlock_t,
        pub __lock: __pthread_spinlock_t,
        pub __readers: c_int,
        pub __readerqueue: *mut __pthread,
        pub __writerqueue: *mut __pthread,
        pub __attr: *mut __pthread_rwlockattr,
        pub __data: *mut c_void,
    }

    pub struct __pthread_barrier {
        pub __lock: __pthread_spinlock_t,
        pub __queue: *mut __pthread,
        pub __pending: c_uint,
        pub __count: c_uint,
        pub __attr: *mut __pthread_barrierattr,
        pub __data: *mut c_void,
    }

    pub struct seminfo {
        pub semmap: c_int,
        pub semmni: c_int,
        pub semmns: c_int,
        pub semmnu: c_int,
        pub semmsl: c_int,
        pub semopm: c_int,
        pub semume: c_int,
        pub semusz: c_int,
        pub semvmx: c_int,
        pub semaem: c_int,
    }

    pub struct _IO_FILE {
        _unused: [u8; 0],
    }

    pub struct sched_param {
        pub sched_priority: c_int,
    }

    pub struct iovec {
        pub iov_base: *mut c_void,
        pub iov_len: size_t,
    }

    pub struct passwd {
        pub pw_name: *mut c_char,
        pub pw_passwd: *mut c_char,
        pub pw_uid: __uid_t,
        pub pw_gid: __gid_t,
        pub pw_gecos: *mut c_char,
        pub pw_dir: *mut c_char,
        pub pw_shell: *mut c_char,
    }

    pub struct spwd {
        pub sp_namp: *mut c_char,
        pub sp_pwdp: *mut c_char,
        pub sp_lstchg: c_long,
        pub sp_min: c_long,
        pub sp_max: c_long,
        pub sp_warn: c_long,
        pub sp_inact: c_long,
        pub sp_expire: c_long,
        pub sp_flag: c_ulong,
    }

    pub struct itimerspec {
        pub it_interval: crate::timespec,
        pub it_value: crate::timespec,
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
        pub tm_gmtoff: c_long,
        pub tm_zone: *const c_char,
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

    pub struct Dl_info {
        pub dli_fname: *const c_char,
        pub dli_fbase: *mut c_void,
        pub dli_sname: *const c_char,
        pub dli_saddr: *mut c_void,
    }

    pub struct ifaddrs {
        pub ifa_next: *mut ifaddrs,
        pub ifa_name: *mut c_char,
        pub ifa_flags: c_uint,
        pub ifa_addr: *mut crate::sockaddr,
        pub ifa_netmask: *mut crate::sockaddr,
        pub ifa_ifu: *mut crate::sockaddr, // FIXME(union) This should be a union
        pub ifa_data: *mut c_void,
    }

    pub struct arpreq {
        pub arp_pa: crate::sockaddr,
        pub arp_ha: crate::sockaddr,
        pub arp_flags: c_int,
        pub arp_netmask: crate::sockaddr,
        pub arp_dev: [c_char; 16],
    }

    pub struct arpreq_old {
        pub arp_pa: crate::sockaddr,
        pub arp_ha: crate::sockaddr,
        pub arp_flags: c_int,
        pub arp_netmask: crate::sockaddr,
    }

    pub struct arphdr {
        pub ar_hrd: u16,
        pub ar_pro: u16,
        pub ar_hln: u8,
        pub ar_pln: u8,
        pub ar_op: u16,
    }

    pub struct arpd_request {
        pub req: c_ushort,
        pub ip: u32,
        pub dev: c_ulong,
        pub stamp: c_ulong,
        pub updated: c_ulong,
        pub ha: [c_uchar; crate::MAX_ADDR_LEN],
    }

    pub struct mmsghdr {
        pub msg_hdr: crate::msghdr,
        pub msg_len: c_uint,
    }

    pub struct ifreq {
        /// interface name, e.g. "en0"
        pub ifr_name: [c_char; crate::IFNAMSIZ],
        pub ifr_ifru: crate::sockaddr,
    }

    pub struct __locale_struct {
        pub __locales: [*mut __locale_data; 13usize],
        pub __ctype_b: *const c_ushort,
        pub __ctype_tolower: *const c_int,
        pub __ctype_toupper: *const c_int,
        pub __names: [*const c_char; 13usize],
    }

    pub struct utsname {
        pub sysname: [c_char; _UTSNAME_LENGTH],
        pub nodename: [c_char; _UTSNAME_LENGTH],
        pub release: [c_char; _UTSNAME_LENGTH],
        pub version: [c_char; _UTSNAME_LENGTH],
        pub machine: [c_char; _UTSNAME_LENGTH],
        pub domainname: [c_char; _UTSNAME_LENGTH],
    }

    pub struct rlimit64 {
        pub rlim_cur: rlim64_t,
        pub rlim_max: rlim64_t,
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_size: size_t,
        pub ss_flags: c_int,
    }

    pub struct dl_phdr_info {
        pub dlpi_addr: Elf_Addr,
        pub dlpi_name: *const c_char,
        pub dlpi_phdr: *const Elf_Phdr,
        pub dlpi_phnum: Elf_Half,
        pub dlpi_adds: c_ulonglong,
        pub dlpi_subs: c_ulonglong,
        pub dlpi_tls_modid: size_t,
        pub dlpi_tls_data: *mut c_void,
    }

    pub struct flock {
        #[cfg(target_pointer_width = "32")]
        pub l_type: c_int,
        #[cfg(target_pointer_width = "32")]
        pub l_whence: c_int,
        #[cfg(target_pointer_width = "64")]
        pub l_type: c_short,
        #[cfg(target_pointer_width = "64")]
        pub l_whence: c_short,
        pub l_start: __off_t,
        pub l_len: __off_t,
        pub l_pid: __pid_t,
    }

    pub struct flock64 {
        #[cfg(target_pointer_width = "32")]
        pub l_type: c_int,
        #[cfg(target_pointer_width = "32")]
        pub l_whence: c_int,
        #[cfg(target_pointer_width = "64")]
        pub l_type: c_short,
        #[cfg(target_pointer_width = "64")]
        pub l_whence: c_short,
        pub l_start: __off_t,
        pub l_len: __off64_t,
        pub l_pid: __pid_t,
    }

    pub struct glob_t {
        pub gl_pathc: size_t,
        pub gl_pathv: *mut *mut c_char,
        pub gl_offs: size_t,
        pub gl_flags: c_int,

        __unused1: *mut c_void,
        __unused2: *mut c_void,
        __unused3: *mut c_void,
        __unused4: *mut c_void,
        __unused5: *mut c_void,
    }

    pub struct glob64_t {
        pub gl_pathc: size_t,
        pub gl_pathv: *mut *mut c_char,
        pub gl_offs: size_t,
        pub gl_flags: c_int,

        __unused1: *mut c_void,
        __unused2: *mut c_void,
        __unused3: *mut c_void,
        __unused4: *mut c_void,
        __unused5: *mut c_void,
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

    pub struct cpu_set_t {
        #[cfg(all(target_pointer_width = "32", not(target_arch = "x86_64")))]
        bits: [u32; 32],
        #[cfg(not(all(target_pointer_width = "32", not(target_arch = "x86_64"))))]
        bits: [u64; 16],
    }

    pub struct if_nameindex {
        pub if_index: c_uint,
        pub if_name: *mut c_char,
    }

    // System V IPC
    pub struct msginfo {
        pub msgpool: c_int,
        pub msgmap: c_int,
        pub msgmax: c_int,
        pub msgmnb: c_int,
        pub msgmni: c_int,
        pub msgssz: c_int,
        pub msgtql: c_int,
        pub msgseg: c_ushort,
    }

    pub struct sembuf {
        pub sem_num: c_ushort,
        pub sem_op: c_short,
        pub sem_flg: c_short,
    }

    pub struct mntent {
        pub mnt_fsname: *mut c_char,
        pub mnt_dir: *mut c_char,
        pub mnt_type: *mut c_char,
        pub mnt_opts: *mut c_char,
        pub mnt_freq: c_int,
        pub mnt_passno: c_int,
    }

    pub struct posix_spawn_file_actions_t {
        __allocated: c_int,
        __used: c_int,
        __actions: *mut c_int,
        __pad: [c_int; 16],
    }

    pub struct posix_spawnattr_t {
        __flags: c_short,
        __pgrp: crate::pid_t,
        __sd: crate::sigset_t,
        __ss: crate::sigset_t,
        __sp: crate::sched_param,
        __policy: c_int,
        __pad: [c_int; 16],
    }

    pub struct regmatch_t {
        pub rm_so: regoff_t,
        pub rm_eo: regoff_t,
    }

    pub struct option {
        pub name: *const c_char,
        pub has_arg: c_int,
        pub flag: *mut c_int,
        pub val: c_int,
    }
}

s_no_extra_traits! {
    pub struct utmpx {
        pub ut_type: c_short,
        pub ut_pid: crate::pid_t,
        pub ut_line: [c_char; __UT_LINESIZE],
        pub ut_id: [c_char; 4],

        pub ut_user: [c_char; __UT_NAMESIZE],
        pub ut_host: [c_char; __UT_HOSTSIZE],
        pub ut_exit: __exit_status,

        #[cfg(any(all(target_pointer_width = "32", not(target_arch = "x86_64"))))]
        pub ut_session: c_long,
        #[cfg(any(all(target_pointer_width = "32", not(target_arch = "x86_64"))))]
        pub ut_tv: crate::timeval,

        #[cfg(not(any(all(target_pointer_width = "32", not(target_arch = "x86_64")))))]
        pub ut_session: i32,
        #[cfg(not(any(all(target_pointer_width = "32", not(target_arch = "x86_64")))))]
        pub ut_tv: __timeval,

        pub ut_addr_v6: [i32; 4],
        __glibc_reserved: [c_char; 20],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for utmpx {
            fn eq(&self, other: &utmpx) -> bool {
                self.ut_type == other.ut_type
                    && self.ut_pid == other.ut_pid
                    && self.ut_line == other.ut_line
                    && self.ut_id == other.ut_id
                    && self.ut_user == other.ut_user
                    && self
                        .ut_host
                        .iter()
                        .zip(other.ut_host.iter())
                        .all(|(a, b)| a == b)
                    && self.ut_exit == other.ut_exit
                    && self.ut_session == other.ut_session
                    && self.ut_tv == other.ut_tv
                    && self.ut_addr_v6 == other.ut_addr_v6
                    && self.__glibc_reserved == other.__glibc_reserved
            }
        }

        impl Eq for utmpx {}

        impl hash::Hash for utmpx {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ut_type.hash(state);
                self.ut_pid.hash(state);
                self.ut_line.hash(state);
                self.ut_id.hash(state);
                self.ut_user.hash(state);
                self.ut_host.hash(state);
                self.ut_exit.hash(state);
                self.ut_session.hash(state);
                self.ut_tv.hash(state);
                self.ut_addr_v6.hash(state);
                self.__glibc_reserved.hash(state);
            }
        }
    }
}

impl siginfo_t {
    pub unsafe fn si_addr(&self) -> *mut c_void {
        self.si_addr
    }

    pub unsafe fn si_value(&self) -> crate::sigval {
        self.si_value
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

// const

// aio.h
pub const AIO_CANCELED: c_int = 0;
pub const AIO_NOTCANCELED: c_int = 1;
pub const AIO_ALLDONE: c_int = 2;
pub const LIO_READ: c_int = 0;
pub const LIO_WRITE: c_int = 1;
pub const LIO_NOP: c_int = 2;
pub const LIO_WAIT: c_int = 0;
pub const LIO_NOWAIT: c_int = 1;

// glob.h
pub const GLOB_ERR: c_int = 1 << 0;
pub const GLOB_MARK: c_int = 1 << 1;
pub const GLOB_NOSORT: c_int = 1 << 2;
pub const GLOB_DOOFFS: c_int = 1 << 3;
pub const GLOB_NOCHECK: c_int = 1 << 4;
pub const GLOB_APPEND: c_int = 1 << 5;
pub const GLOB_NOESCAPE: c_int = 1 << 6;

pub const GLOB_NOSPACE: c_int = 1;
pub const GLOB_ABORTED: c_int = 2;
pub const GLOB_NOMATCH: c_int = 3;

pub const GLOB_PERIOD: c_int = 1 << 7;
pub const GLOB_ALTDIRFUNC: c_int = 1 << 9;
pub const GLOB_BRACE: c_int = 1 << 10;
pub const GLOB_NOMAGIC: c_int = 1 << 11;
pub const GLOB_TILDE: c_int = 1 << 12;
pub const GLOB_ONLYDIR: c_int = 1 << 13;
pub const GLOB_TILDE_CHECK: c_int = 1 << 14;

// ipc.h
pub const IPC_PRIVATE: crate::key_t = 0;

pub const IPC_CREAT: c_int = 0o1000;
pub const IPC_EXCL: c_int = 0o2000;
pub const IPC_NOWAIT: c_int = 0o4000;

pub const IPC_RMID: c_int = 0;
pub const IPC_SET: c_int = 1;
pub const IPC_STAT: c_int = 2;
pub const IPC_INFO: c_int = 3;
pub const MSG_STAT: c_int = 11;
pub const MSG_INFO: c_int = 12;

pub const MSG_NOERROR: c_int = 0o10000;
pub const MSG_EXCEPT: c_int = 0o20000;

// shm.h
pub const SHM_R: c_int = 0o400;
pub const SHM_W: c_int = 0o200;

pub const SHM_RDONLY: c_int = 0o10000;
pub const SHM_RND: c_int = 0o20000;
pub const SHM_REMAP: c_int = 0o40000;

pub const SHM_LOCK: c_int = 11;
pub const SHM_UNLOCK: c_int = 12;
// unistd.h
pub const STDIN_FILENO: c_int = 0;
pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;
pub const __FD_SETSIZE: usize = 256;
pub const R_OK: c_int = 4;
pub const W_OK: c_int = 2;
pub const X_OK: c_int = 1;
pub const F_OK: c_int = 0;
pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;
pub const SEEK_DATA: c_int = 3;
pub const SEEK_HOLE: c_int = 4;
pub const L_SET: c_int = 0;
pub const L_INCR: c_int = 1;
pub const L_XTND: c_int = 2;
pub const F_ULOCK: c_int = 0;
pub const F_LOCK: c_int = 1;
pub const F_TLOCK: c_int = 2;
pub const F_TEST: c_int = 3;
pub const CLOSE_RANGE_CLOEXEC: c_int = 4;

// stdio.h
pub const EOF: c_int = -1;

// stdlib.h
pub const WNOHANG: c_int = 1;
pub const WUNTRACED: c_int = 2;
pub const WSTOPPED: c_int = 2;
pub const WCONTINUED: c_int = 4;
pub const WNOWAIT: c_int = 8;
pub const WEXITED: c_int = 16;
pub const __W_CONTINUED: c_int = 65535;
pub const __WCOREFLAG: c_int = 128;
pub const RAND_MAX: c_int = 2147483647;
pub const EXIT_FAILURE: c_int = 1;
pub const EXIT_SUCCESS: c_int = 0;
pub const __LITTLE_ENDIAN: usize = 1234;
pub const __BIG_ENDIAN: usize = 4321;
pub const __PDP_ENDIAN: usize = 3412;
pub const __BYTE_ORDER: usize = 1234;
pub const __FLOAT_WORD_ORDER: usize = 1234;
pub const LITTLE_ENDIAN: usize = 1234;
pub const BIG_ENDIAN: usize = 4321;
pub const PDP_ENDIAN: usize = 3412;
pub const BYTE_ORDER: usize = 1234;

// sys/select.h
pub const FD_SETSIZE: usize = 256;
pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 32;
pub const __SIZEOF_PTHREAD_ATTR_T: usize = 32;
pub const __SIZEOF_PTHREAD_RWLOCK_T: usize = 28;
pub const __SIZEOF_PTHREAD_BARRIER_T: usize = 24;
pub const __SIZEOF_PTHREAD_MUTEXATTR_T: usize = 16;
pub const __SIZEOF_PTHREAD_COND_T: usize = 20;
pub const __SIZEOF_PTHREAD_CONDATTR_T: usize = 8;
pub const __SIZEOF_PTHREAD_RWLOCKATTR_T: usize = 4;
pub const __SIZEOF_PTHREAD_BARRIERATTR_T: usize = 4;
pub const __SIZEOF_PTHREAD_ONCE_T: usize = 8;
pub const __PTHREAD_SPIN_LOCK_INITIALIZER: c_int = 0;
pub const PTHREAD_MUTEX_NORMAL: c_int = 0;

// sys/resource.h
pub const RLIM_INFINITY: crate::rlim_t = 2147483647;
pub const RLIM64_INFINITY: crate::rlim64_t = 9223372036854775807;
pub const RLIM_SAVED_MAX: crate::rlim_t = RLIM_INFINITY;
pub const RLIM_SAVED_CUR: crate::rlim_t = RLIM_INFINITY;
pub const PRIO_MIN: c_int = -20;
pub const PRIO_MAX: c_int = 20;

// pwd.h
pub const NSS_BUFLEN_PASSWD: usize = 1024;

// sys/socket.h
pub const SOCK_TYPE_MASK: usize = 15;
pub const PF_UNSPEC: c_int = 0;
pub const PF_LOCAL: c_int = 1;
pub const PF_UNIX: c_int = 1;
pub const PF_FILE: c_int = 1;
pub const PF_INET: c_int = 2;
pub const PF_IMPLINK: c_int = 3;
pub const PF_PUP: c_int = 4;
pub const PF_CHAOS: c_int = 5;
pub const PF_NS: c_int = 6;
pub const PF_ISO: c_int = 7;
pub const PF_OSI: c_int = 7;
pub const PF_ECMA: c_int = 8;
pub const PF_DATAKIT: c_int = 9;
pub const PF_CCITT: c_int = 10;
pub const PF_SNA: c_int = 11;
pub const PF_DECnet: c_int = 12;
pub const PF_DLI: c_int = 13;
pub const PF_LAT: c_int = 14;
pub const PF_HYLINK: c_int = 15;
pub const PF_APPLETALK: c_int = 16;
pub const PF_ROUTE: c_int = 17;
pub const PF_XTP: c_int = 19;
pub const PF_COIP: c_int = 20;
pub const PF_CNT: c_int = 21;
pub const PF_RTIP: c_int = 22;
pub const PF_IPX: c_int = 23;
pub const PF_SIP: c_int = 24;
pub const PF_PIP: c_int = 25;
pub const PF_INET6: c_int = 26;
pub const PF_MAX: c_int = 27;
pub const AF_UNSPEC: c_int = 0;
pub const AF_LOCAL: c_int = 1;
pub const AF_UNIX: c_int = 1;
pub const AF_FILE: c_int = 1;
pub const AF_INET: c_int = 2;
pub const AF_IMPLINK: c_int = 3;
pub const AF_PUP: c_int = 4;
pub const AF_CHAOS: c_int = 5;
pub const AF_NS: c_int = 6;
pub const AF_ISO: c_int = 7;
pub const AF_OSI: c_int = 7;
pub const AF_ECMA: c_int = 8;
pub const AF_DATAKIT: c_int = 9;
pub const AF_CCITT: c_int = 10;
pub const AF_SNA: c_int = 11;
pub const AF_DECnet: c_int = 12;
pub const AF_DLI: c_int = 13;
pub const AF_LAT: c_int = 14;
pub const AF_HYLINK: c_int = 15;
pub const AF_APPLETALK: c_int = 16;
pub const AF_ROUTE: c_int = 17;
pub const pseudo_AF_XTP: c_int = 19;
pub const AF_COIP: c_int = 20;
pub const AF_CNT: c_int = 21;
pub const pseudo_AF_RTIP: c_int = 22;
pub const AF_IPX: c_int = 23;
pub const AF_SIP: c_int = 24;
pub const pseudo_AF_PIP: c_int = 25;
pub const AF_INET6: c_int = 26;
pub const AF_MAX: c_int = 27;
pub const SOMAXCONN: c_int = 4096;
pub const _SS_SIZE: usize = 128;
pub const CMGROUP_MAX: usize = 16;
pub const SOL_SOCKET: c_int = 65535;

// sys/time.h
pub const ITIMER_REAL: c_int = 0;
pub const ITIMER_VIRTUAL: c_int = 1;
pub const ITIMER_PROF: c_int = 2;

// netinet/in.h
pub const SOL_IP: c_int = 0;
pub const SOL_TCP: c_int = 6;
pub const SOL_UDP: c_int = 17;
pub const SOL_IPV6: c_int = 41;
pub const SOL_ICMPV6: c_int = 58;
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
pub const IPV6_ADDRFORM: c_int = 1;
pub const IPV6_2292PKTINFO: c_int = 2;
pub const IPV6_2292HOPOPTS: c_int = 3;
pub const IPV6_2292DSTOPTS: c_int = 4;
pub const IPV6_2292RTHDR: c_int = 5;
pub const IPV6_2292PKTOPTIONS: c_int = 6;
pub const IPV6_CHECKSUM: c_int = 7;
pub const IPV6_2292HOPLIMIT: c_int = 8;
pub const IPV6_RXINFO: c_int = 2;
pub const IPV6_TXINFO: c_int = 2;
pub const SCM_SRCINFO: c_int = 2;
pub const IPV6_UNICAST_HOPS: c_int = 16;
pub const IPV6_MULTICAST_IF: c_int = 17;
pub const IPV6_MULTICAST_HOPS: c_int = 18;
pub const IPV6_MULTICAST_LOOP: c_int = 19;
pub const IPV6_JOIN_GROUP: c_int = 20;
pub const IPV6_LEAVE_GROUP: c_int = 21;
pub const IPV6_ROUTER_ALERT: c_int = 22;
pub const IPV6_MTU_DISCOVER: c_int = 23;
pub const IPV6_MTU: c_int = 24;
pub const IPV6_RECVERR: c_int = 25;
pub const IPV6_V6ONLY: c_int = 26;
pub const IPV6_JOIN_ANYCAST: c_int = 27;
pub const IPV6_LEAVE_ANYCAST: c_int = 28;
pub const IPV6_RECVPKTINFO: c_int = 49;
pub const IPV6_PKTINFO: c_int = 50;
pub const IPV6_RECVHOPLIMIT: c_int = 51;
pub const IPV6_HOPLIMIT: c_int = 52;
pub const IPV6_RECVHOPOPTS: c_int = 53;
pub const IPV6_HOPOPTS: c_int = 54;
pub const IPV6_RTHDRDSTOPTS: c_int = 55;
pub const IPV6_RECVRTHDR: c_int = 56;
pub const IPV6_RTHDR: c_int = 57;
pub const IPV6_RECVDSTOPTS: c_int = 58;
pub const IPV6_DSTOPTS: c_int = 59;
pub const IPV6_RECVPATHMTU: c_int = 60;
pub const IPV6_PATHMTU: c_int = 61;
pub const IPV6_DONTFRAG: c_int = 62;
pub const IPV6_RECVTCLASS: c_int = 66;
pub const IPV6_TCLASS: c_int = 67;
pub const IPV6_ADDR_PREFERENCES: c_int = 72;
pub const IPV6_MINHOPCOUNT: c_int = 73;
pub const IPV6_ADD_MEMBERSHIP: c_int = 20;
pub const IPV6_DROP_MEMBERSHIP: c_int = 21;
pub const IPV6_RXHOPOPTS: c_int = 3;
pub const IPV6_RXDSTOPTS: c_int = 4;
pub const IPV6_RTHDR_LOOSE: c_int = 0;
pub const IPV6_RTHDR_STRICT: c_int = 1;
pub const IPV6_RTHDR_TYPE_0: c_int = 0;
pub const IN_CLASSA_NET: u32 = 4278190080;
pub const IN_CLASSA_NSHIFT: usize = 24;
pub const IN_CLASSA_HOST: u32 = 16777215;
pub const IN_CLASSA_MAX: u32 = 128;
pub const IN_CLASSB_NET: u32 = 4294901760;
pub const IN_CLASSB_NSHIFT: usize = 16;
pub const IN_CLASSB_HOST: u32 = 65535;
pub const IN_CLASSB_MAX: u32 = 65536;
pub const IN_CLASSC_NET: u32 = 4294967040;
pub const IN_CLASSC_NSHIFT: usize = 8;
pub const IN_CLASSC_HOST: u32 = 255;
pub const IN_LOOPBACKNET: u32 = 127;
pub const INET_ADDRSTRLEN: usize = 16;
pub const INET6_ADDRSTRLEN: usize = 46;

// netinet/ip.h
pub const IPTOS_TOS_MASK: u8 = 0x1E;
pub const IPTOS_PREC_MASK: u8 = 0xE0;

pub const IPTOS_ECN_NOT_ECT: u8 = 0x00;

pub const IPTOS_LOWDELAY: u8 = 0x10;
pub const IPTOS_THROUGHPUT: u8 = 0x08;
pub const IPTOS_RELIABILITY: u8 = 0x04;
pub const IPTOS_MINCOST: u8 = 0x02;

pub const IPTOS_PREC_NETCONTROL: u8 = 0xe0;
pub const IPTOS_PREC_INTERNETCONTROL: u8 = 0xc0;
pub const IPTOS_PREC_CRITIC_ECP: u8 = 0xa0;
pub const IPTOS_PREC_FLASHOVERRIDE: u8 = 0x80;
pub const IPTOS_PREC_FLASH: u8 = 0x60;
pub const IPTOS_PREC_IMMEDIATE: u8 = 0x40;
pub const IPTOS_PREC_PRIORITY: u8 = 0x20;
pub const IPTOS_PREC_ROUTINE: u8 = 0x00;

pub const IPTOS_ECN_MASK: u8 = 0x03;
pub const IPTOS_ECN_ECT1: u8 = 0x01;
pub const IPTOS_ECN_ECT0: u8 = 0x02;
pub const IPTOS_ECN_CE: u8 = 0x03;

pub const IPOPT_COPY: u8 = 0x80;
pub const IPOPT_CLASS_MASK: u8 = 0x60;
pub const IPOPT_NUMBER_MASK: u8 = 0x1f;

pub const IPOPT_CONTROL: u8 = 0x00;
pub const IPOPT_RESERVED1: u8 = 0x20;
pub const IPOPT_MEASUREMENT: u8 = 0x40;
pub const IPOPT_RESERVED2: u8 = 0x60;
pub const IPOPT_END: u8 = 0 | IPOPT_CONTROL;
pub const IPOPT_NOOP: u8 = 1 | IPOPT_CONTROL;
pub const IPOPT_SEC: u8 = 2 | IPOPT_CONTROL | IPOPT_COPY;
pub const IPOPT_LSRR: u8 = 3 | IPOPT_CONTROL | IPOPT_COPY;
pub const IPOPT_TIMESTAMP: u8 = 4 | IPOPT_MEASUREMENT;
pub const IPOPT_RR: u8 = 7 | IPOPT_CONTROL;
pub const IPOPT_SID: u8 = 8 | IPOPT_CONTROL | IPOPT_COPY;
pub const IPOPT_SSRR: u8 = 9 | IPOPT_CONTROL | IPOPT_COPY;
pub const IPOPT_RA: u8 = 20 | IPOPT_CONTROL | IPOPT_COPY;
pub const IPVERSION: u8 = 4;
pub const MAXTTL: u8 = 255;
pub const IPDEFTTL: u8 = 64;
pub const IPOPT_OPTVAL: u8 = 0;
pub const IPOPT_OLEN: u8 = 1;
pub const IPOPT_OFFSET: u8 = 2;
pub const IPOPT_MINOFF: u8 = 4;
pub const MAX_IPOPTLEN: u8 = 40;
pub const IPOPT_NOP: u8 = IPOPT_NOOP;
pub const IPOPT_EOL: u8 = IPOPT_END;
pub const IPOPT_TS: u8 = IPOPT_TIMESTAMP;
pub const IPOPT_TS_TSONLY: u8 = 0;
pub const IPOPT_TS_TSANDADDR: u8 = 1;
pub const IPOPT_TS_PRESPEC: u8 = 3;

// net/if_arp.h
pub const ARPOP_REQUEST: u16 = 1;
pub const ARPOP_REPLY: u16 = 2;
pub const ARPOP_RREQUEST: u16 = 3;
pub const ARPOP_RREPLY: u16 = 4;
pub const ARPOP_InREQUEST: u16 = 8;
pub const ARPOP_InREPLY: u16 = 9;
pub const ARPOP_NAK: u16 = 10;

pub const MAX_ADDR_LEN: usize = 7;
pub const ARPD_UPDATE: c_ushort = 0x01;
pub const ARPD_LOOKUP: c_ushort = 0x02;
pub const ARPD_FLUSH: c_ushort = 0x03;
pub const ATF_MAGIC: c_int = 0x80;

pub const ATF_NETMASK: c_int = 0x20;
pub const ATF_DONTPUB: c_int = 0x40;

pub const ARPHRD_NETROM: u16 = 0;
pub const ARPHRD_ETHER: u16 = 1;
pub const ARPHRD_EETHER: u16 = 2;
pub const ARPHRD_AX25: u16 = 3;
pub const ARPHRD_PRONET: u16 = 4;
pub const ARPHRD_CHAOS: u16 = 5;
pub const ARPHRD_IEEE802: u16 = 6;
pub const ARPHRD_ARCNET: u16 = 7;
pub const ARPHRD_APPLETLK: u16 = 8;
pub const ARPHRD_DLCI: u16 = 15;
pub const ARPHRD_ATM: u16 = 19;
pub const ARPHRD_METRICOM: u16 = 23;
pub const ARPHRD_IEEE1394: u16 = 24;
pub const ARPHRD_EUI64: u16 = 27;
pub const ARPHRD_INFINIBAND: u16 = 32;

pub const ARPHRD_SLIP: u16 = 256;
pub const ARPHRD_CSLIP: u16 = 257;
pub const ARPHRD_SLIP6: u16 = 258;
pub const ARPHRD_CSLIP6: u16 = 259;
pub const ARPHRD_RSRVD: u16 = 260;
pub const ARPHRD_ADAPT: u16 = 264;
pub const ARPHRD_ROSE: u16 = 270;
pub const ARPHRD_X25: u16 = 271;
pub const ARPHRD_HWX25: u16 = 272;
pub const ARPHRD_CAN: u16 = 280;
pub const ARPHRD_PPP: u16 = 512;
pub const ARPHRD_CISCO: u16 = 513;
pub const ARPHRD_HDLC: u16 = ARPHRD_CISCO;
pub const ARPHRD_LAPB: u16 = 516;
pub const ARPHRD_DDCMP: u16 = 517;
pub const ARPHRD_RAWHDLC: u16 = 518;

pub const ARPHRD_TUNNEL: u16 = 768;
pub const ARPHRD_TUNNEL6: u16 = 769;
pub const ARPHRD_FRAD: u16 = 770;
pub const ARPHRD_SKIP: u16 = 771;
pub const ARPHRD_LOOPBACK: u16 = 772;
pub const ARPHRD_LOCALTLK: u16 = 773;
pub const ARPHRD_FDDI: u16 = 774;
pub const ARPHRD_BIF: u16 = 775;
pub const ARPHRD_SIT: u16 = 776;
pub const ARPHRD_IPDDP: u16 = 777;
pub const ARPHRD_IPGRE: u16 = 778;
pub const ARPHRD_PIMREG: u16 = 779;
pub const ARPHRD_HIPPI: u16 = 780;
pub const ARPHRD_ASH: u16 = 781;
pub const ARPHRD_ECONET: u16 = 782;
pub const ARPHRD_IRDA: u16 = 783;
pub const ARPHRD_FCPP: u16 = 784;
pub const ARPHRD_FCAL: u16 = 785;
pub const ARPHRD_FCPL: u16 = 786;
pub const ARPHRD_FCFABRIC: u16 = 787;
pub const ARPHRD_IEEE802_TR: u16 = 800;
pub const ARPHRD_IEEE80211: u16 = 801;
pub const ARPHRD_IEEE80211_PRISM: u16 = 802;
pub const ARPHRD_IEEE80211_RADIOTAP: u16 = 803;
pub const ARPHRD_IEEE802154: u16 = 804;

pub const ARPHRD_VOID: u16 = 0xFFFF;
pub const ARPHRD_NONE: u16 = 0xFFFE;

// bits/posix1_lim.h
pub const _POSIX_AIO_LISTIO_MAX: usize = 2;
pub const _POSIX_AIO_MAX: usize = 1;
pub const _POSIX_ARG_MAX: usize = 4096;
pub const _POSIX_CHILD_MAX: usize = 25;
pub const _POSIX_DELAYTIMER_MAX: usize = 32;
pub const _POSIX_HOST_NAME_MAX: usize = 255;
pub const _POSIX_LINK_MAX: usize = 8;
pub const _POSIX_LOGIN_NAME_MAX: usize = 9;
pub const _POSIX_MAX_CANON: usize = 255;
pub const _POSIX_MAX_INPUT: usize = 255;
pub const _POSIX_MQ_OPEN_MAX: usize = 8;
pub const _POSIX_MQ_PRIO_MAX: usize = 32;
pub const _POSIX_NAME_MAX: usize = 14;
pub const _POSIX_NGROUPS_MAX: usize = 8;
pub const _POSIX_OPEN_MAX: usize = 20;
pub const _POSIX_FD_SETSIZE: usize = 20;
pub const _POSIX_PATH_MAX: usize = 256;
pub const _POSIX_PIPE_BUF: usize = 512;
pub const _POSIX_RE_DUP_MAX: usize = 255;
pub const _POSIX_RTSIG_MAX: usize = 8;
pub const _POSIX_SEM_NSEMS_MAX: usize = 256;
pub const _POSIX_SEM_VALUE_MAX: usize = 32767;
pub const _POSIX_SIGQUEUE_MAX: usize = 32;
pub const _POSIX_SSIZE_MAX: usize = 32767;
pub const _POSIX_STREAM_MAX: usize = 8;
pub const _POSIX_SYMLINK_MAX: usize = 255;
pub const _POSIX_SYMLOOP_MAX: usize = 8;
pub const _POSIX_TIMER_MAX: usize = 32;
pub const _POSIX_TTY_NAME_MAX: usize = 9;
pub const _POSIX_TZNAME_MAX: usize = 6;
pub const _POSIX_QLIMIT: usize = 1;
pub const _POSIX_HIWAT: usize = 512;
pub const _POSIX_UIO_MAXIOV: usize = 16;
pub const _POSIX_CLOCKRES_MIN: usize = 20000000;
pub const NAME_MAX: usize = 255;
pub const NGROUPS_MAX: usize = 256;
pub const _POSIX_THREAD_KEYS_MAX: usize = 128;
pub const _POSIX_THREAD_DESTRUCTOR_ITERATIONS: usize = 4;
pub const _POSIX_THREAD_THREADS_MAX: usize = 64;
pub const SEM_VALUE_MAX: c_int = 2147483647;
pub const MAXNAMLEN: usize = 255;

// netdb.h
pub const _PATH_HEQUIV: &[u8; 17usize] = b"/etc/hosts.equiv\0";
pub const _PATH_HOSTS: &[u8; 11usize] = b"/etc/hosts\0";
pub const _PATH_NETWORKS: &[u8; 14usize] = b"/etc/networks\0";
pub const _PATH_NSSWITCH_CONF: &[u8; 19usize] = b"/etc/nsswitch.conf\0";
pub const _PATH_PROTOCOLS: &[u8; 15usize] = b"/etc/protocols\0";
pub const _PATH_SERVICES: &[u8; 14usize] = b"/etc/services\0";
pub const HOST_NOT_FOUND: c_int = 1;
pub const TRY_AGAIN: c_int = 2;
pub const NO_RECOVERY: c_int = 3;
pub const NO_DATA: c_int = 4;
pub const NETDB_INTERNAL: c_int = -1;
pub const NETDB_SUCCESS: c_int = 0;
pub const NO_ADDRESS: c_int = 4;
pub const IPPORT_RESERVED: c_int = 1024;
pub const SCOPE_DELIMITER: u8 = 37u8;
pub const GAI_WAIT: c_int = 0;
pub const GAI_NOWAIT: c_int = 1;
pub const AI_PASSIVE: c_int = 1;
pub const AI_CANONNAME: c_int = 2;
pub const AI_NUMERICHOST: c_int = 4;
pub const AI_V4MAPPED: c_int = 8;
pub const AI_ALL: c_int = 16;
pub const AI_ADDRCONFIG: c_int = 32;
pub const AI_IDN: c_int = 64;
pub const AI_CANONIDN: c_int = 128;
pub const AI_NUMERICSERV: c_int = 1024;
pub const EAI_BADFLAGS: c_int = -1;
pub const EAI_NONAME: c_int = -2;
pub const EAI_AGAIN: c_int = -3;
pub const EAI_FAIL: c_int = -4;
pub const EAI_FAMILY: c_int = -6;
pub const EAI_SOCKTYPE: c_int = -7;
pub const EAI_SERVICE: c_int = -8;
pub const EAI_MEMORY: c_int = -10;
pub const EAI_SYSTEM: c_int = -11;
pub const EAI_OVERFLOW: c_int = -12;
pub const EAI_NODATA: c_int = -5;
pub const EAI_ADDRFAMILY: c_int = -9;
pub const EAI_INPROGRESS: c_int = -100;
pub const EAI_CANCELED: c_int = -101;
pub const EAI_NOTCANCELED: c_int = -102;
pub const EAI_ALLDONE: c_int = -103;
pub const EAI_INTR: c_int = -104;
pub const EAI_IDN_ENCODE: c_int = -105;
pub const NI_MAXHOST: usize = 1025;
pub const NI_MAXSERV: usize = 32;
pub const NI_NUMERICHOST: c_int = 1;
pub const NI_NUMERICSERV: c_int = 2;
pub const NI_NOFQDN: c_int = 4;
pub const NI_NAMEREQD: c_int = 8;
pub const NI_DGRAM: c_int = 16;
pub const NI_IDN: c_int = 32;

// time.h
pub const CLOCK_REALTIME: crate::clockid_t = 0;
pub const CLOCK_MONOTONIC: crate::clockid_t = 1;
pub const CLOCK_PROCESS_CPUTIME_ID: crate::clockid_t = 2;
pub const CLOCK_THREAD_CPUTIME_ID: crate::clockid_t = 3;
pub const CLOCK_MONOTONIC_RAW: crate::clockid_t = 4;
pub const CLOCK_REALTIME_COARSE: crate::clockid_t = 5;
pub const CLOCK_MONOTONIC_COARSE: crate::clockid_t = 6;
pub const TIMER_ABSTIME: c_int = 1;
pub const TIME_UTC: c_int = 1;

// sys/poll.h
pub const POLLIN: i16 = 1;
pub const POLLPRI: i16 = 2;
pub const POLLOUT: i16 = 4;
pub const POLLRDNORM: i16 = 1;
pub const POLLRDBAND: i16 = 2;
pub const POLLWRNORM: i16 = 4;
pub const POLLWRBAND: i16 = 4;
pub const POLLERR: i16 = 8;
pub const POLLHUP: i16 = 16;
pub const POLLNVAL: i16 = 32;

// locale.h
pub const __LC_CTYPE: usize = 0;
pub const __LC_NUMERIC: usize = 1;
pub const __LC_TIME: usize = 2;
pub const __LC_COLLATE: usize = 3;
pub const __LC_MONETARY: usize = 4;
pub const __LC_MESSAGES: usize = 5;
pub const __LC_ALL: usize = 6;
pub const __LC_PAPER: usize = 7;
pub const __LC_NAME: usize = 8;
pub const __LC_ADDRESS: usize = 9;
pub const __LC_TELEPHONE: usize = 10;
pub const __LC_MEASUREMENT: usize = 11;
pub const __LC_IDENTIFICATION: usize = 12;
pub const LC_CTYPE: c_int = 0;
pub const LC_NUMERIC: c_int = 1;
pub const LC_TIME: c_int = 2;
pub const LC_COLLATE: c_int = 3;
pub const LC_MONETARY: c_int = 4;
pub const LC_MESSAGES: c_int = 5;
pub const LC_ALL: c_int = 6;
pub const LC_PAPER: c_int = 7;
pub const LC_NAME: c_int = 8;
pub const LC_ADDRESS: c_int = 9;
pub const LC_TELEPHONE: c_int = 10;
pub const LC_MEASUREMENT: c_int = 11;
pub const LC_IDENTIFICATION: c_int = 12;
pub const LC_CTYPE_MASK: c_int = 1;
pub const LC_NUMERIC_MASK: c_int = 2;
pub const LC_TIME_MASK: c_int = 4;
pub const LC_COLLATE_MASK: c_int = 8;
pub const LC_MONETARY_MASK: c_int = 16;
pub const LC_MESSAGES_MASK: c_int = 32;
pub const LC_PAPER_MASK: c_int = 128;
pub const LC_NAME_MASK: c_int = 256;
pub const LC_ADDRESS_MASK: c_int = 512;
pub const LC_TELEPHONE_MASK: c_int = 1024;
pub const LC_MEASUREMENT_MASK: c_int = 2048;
pub const LC_IDENTIFICATION_MASK: c_int = 4096;
pub const LC_ALL_MASK: c_int = 8127;

pub const ABDAY_1: crate::nl_item = 0x20000;
pub const ABDAY_2: crate::nl_item = 0x20001;
pub const ABDAY_3: crate::nl_item = 0x20002;
pub const ABDAY_4: crate::nl_item = 0x20003;
pub const ABDAY_5: crate::nl_item = 0x20004;
pub const ABDAY_6: crate::nl_item = 0x20005;
pub const ABDAY_7: crate::nl_item = 0x20006;

pub const DAY_1: crate::nl_item = 0x20007;
pub const DAY_2: crate::nl_item = 0x20008;
pub const DAY_3: crate::nl_item = 0x20009;
pub const DAY_4: crate::nl_item = 0x2000A;
pub const DAY_5: crate::nl_item = 0x2000B;
pub const DAY_6: crate::nl_item = 0x2000C;
pub const DAY_7: crate::nl_item = 0x2000D;

pub const ABMON_1: crate::nl_item = 0x2000E;
pub const ABMON_2: crate::nl_item = 0x2000F;
pub const ABMON_3: crate::nl_item = 0x20010;
pub const ABMON_4: crate::nl_item = 0x20011;
pub const ABMON_5: crate::nl_item = 0x20012;
pub const ABMON_6: crate::nl_item = 0x20013;
pub const ABMON_7: crate::nl_item = 0x20014;
pub const ABMON_8: crate::nl_item = 0x20015;
pub const ABMON_9: crate::nl_item = 0x20016;
pub const ABMON_10: crate::nl_item = 0x20017;
pub const ABMON_11: crate::nl_item = 0x20018;
pub const ABMON_12: crate::nl_item = 0x20019;

pub const MON_1: crate::nl_item = 0x2001A;
pub const MON_2: crate::nl_item = 0x2001B;
pub const MON_3: crate::nl_item = 0x2001C;
pub const MON_4: crate::nl_item = 0x2001D;
pub const MON_5: crate::nl_item = 0x2001E;
pub const MON_6: crate::nl_item = 0x2001F;
pub const MON_7: crate::nl_item = 0x20020;
pub const MON_8: crate::nl_item = 0x20021;
pub const MON_9: crate::nl_item = 0x20022;
pub const MON_10: crate::nl_item = 0x20023;
pub const MON_11: crate::nl_item = 0x20024;
pub const MON_12: crate::nl_item = 0x20025;

pub const AM_STR: crate::nl_item = 0x20026;
pub const PM_STR: crate::nl_item = 0x20027;

pub const D_T_FMT: crate::nl_item = 0x20028;
pub const D_FMT: crate::nl_item = 0x20029;
pub const T_FMT: crate::nl_item = 0x2002A;
pub const T_FMT_AMPM: crate::nl_item = 0x2002B;

pub const ERA: crate::nl_item = 0x2002C;
pub const ERA_D_FMT: crate::nl_item = 0x2002E;
pub const ALT_DIGITS: crate::nl_item = 0x2002F;
pub const ERA_D_T_FMT: crate::nl_item = 0x20030;
pub const ERA_T_FMT: crate::nl_item = 0x20031;

pub const CODESET: crate::nl_item = 14;
pub const CRNCYSTR: crate::nl_item = 0x4000F;
pub const RADIXCHAR: crate::nl_item = 0x10000;
pub const THOUSEP: crate::nl_item = 0x10001;
pub const YESEXPR: crate::nl_item = 0x50000;
pub const NOEXPR: crate::nl_item = 0x50001;
pub const YESSTR: crate::nl_item = 0x50002;
pub const NOSTR: crate::nl_item = 0x50003;

// reboot.h
pub const RB_AUTOBOOT: c_int = 0x0;
pub const RB_ASKNAME: c_int = 0x1;
pub const RB_SINGLE: c_int = 0x2;
pub const RB_KBD: c_int = 0x4;
pub const RB_HALT: c_int = 0x8;
pub const RB_INITNAME: c_int = 0x10;
pub const RB_DFLTROOT: c_int = 0x20;
pub const RB_NOBOOTRC: c_int = 0x20;
pub const RB_ALTBOOT: c_int = 0x40;
pub const RB_UNIPROC: c_int = 0x80;
pub const RB_DEBUGGER: c_int = 0x1000;

// semaphore.h
pub const __SIZEOF_SEM_T: usize = 20;
pub const SEM_FAILED: *mut crate::sem_t = 0 as *mut sem_t;

// termios.h
pub const IGNBRK: crate::tcflag_t = 1;
pub const BRKINT: crate::tcflag_t = 2;
pub const IGNPAR: crate::tcflag_t = 4;
pub const PARMRK: crate::tcflag_t = 8;
pub const INPCK: crate::tcflag_t = 16;
pub const ISTRIP: crate::tcflag_t = 32;
pub const INLCR: crate::tcflag_t = 64;
pub const IGNCR: crate::tcflag_t = 128;
pub const ICRNL: crate::tcflag_t = 256;
pub const IXON: crate::tcflag_t = 512;
pub const IXOFF: crate::tcflag_t = 1024;
pub const IXANY: crate::tcflag_t = 2048;
pub const IMAXBEL: crate::tcflag_t = 8192;
pub const IUCLC: crate::tcflag_t = 16384;
pub const OPOST: crate::tcflag_t = 1;
pub const ONLCR: crate::tcflag_t = 2;
pub const ONOEOT: crate::tcflag_t = 8;
pub const OCRNL: crate::tcflag_t = 16;
pub const ONOCR: crate::tcflag_t = 32;
pub const ONLRET: crate::tcflag_t = 64;
pub const NLDLY: crate::tcflag_t = 768;
pub const NL0: crate::tcflag_t = 0;
pub const NL1: crate::tcflag_t = 256;
pub const TABDLY: crate::tcflag_t = 3076;
pub const TAB0: crate::tcflag_t = 0;
pub const TAB1: crate::tcflag_t = 1024;
pub const TAB2: crate::tcflag_t = 2048;
pub const TAB3: crate::tcflag_t = 4;
pub const CRDLY: crate::tcflag_t = 12288;
pub const CR0: crate::tcflag_t = 0;
pub const CR1: crate::tcflag_t = 4096;
pub const CR2: crate::tcflag_t = 8192;
pub const CR3: crate::tcflag_t = 12288;
pub const FFDLY: crate::tcflag_t = 16384;
pub const FF0: crate::tcflag_t = 0;
pub const FF1: crate::tcflag_t = 16384;
pub const BSDLY: crate::tcflag_t = 32768;
pub const BS0: crate::tcflag_t = 0;
pub const BS1: crate::tcflag_t = 32768;
pub const VTDLY: crate::tcflag_t = 65536;
pub const VT0: crate::tcflag_t = 0;
pub const VT1: crate::tcflag_t = 65536;
pub const OLCUC: crate::tcflag_t = 131072;
pub const OFILL: crate::tcflag_t = 262144;
pub const OFDEL: crate::tcflag_t = 524288;
pub const CIGNORE: crate::tcflag_t = 1;
pub const CSIZE: crate::tcflag_t = 768;
pub const CS5: crate::tcflag_t = 0;
pub const CS6: crate::tcflag_t = 256;
pub const CS7: crate::tcflag_t = 512;
pub const CS8: crate::tcflag_t = 768;
pub const CSTOPB: crate::tcflag_t = 1024;
pub const CREAD: crate::tcflag_t = 2048;
pub const PARENB: crate::tcflag_t = 4096;
pub const PARODD: crate::tcflag_t = 8192;
pub const HUPCL: crate::tcflag_t = 16384;
pub const CLOCAL: crate::tcflag_t = 32768;
pub const CRTSCTS: crate::tcflag_t = 65536;
pub const CRTS_IFLOW: crate::tcflag_t = 65536;
pub const CCTS_OFLOW: crate::tcflag_t = 65536;
pub const CDTRCTS: crate::tcflag_t = 131072;
pub const MDMBUF: crate::tcflag_t = 1048576;
pub const CHWFLOW: crate::tcflag_t = 1245184;
pub const ECHOKE: crate::tcflag_t = 1;
pub const _ECHOE: crate::tcflag_t = 2;
pub const ECHOE: crate::tcflag_t = 2;
pub const _ECHOK: crate::tcflag_t = 4;
pub const ECHOK: crate::tcflag_t = 4;
pub const _ECHO: crate::tcflag_t = 8;
pub const ECHO: crate::tcflag_t = 8;
pub const _ECHONL: crate::tcflag_t = 16;
pub const ECHONL: crate::tcflag_t = 16;
pub const ECHOPRT: crate::tcflag_t = 32;
pub const ECHOCTL: crate::tcflag_t = 64;
pub const _ISIG: crate::tcflag_t = 128;
pub const ISIG: crate::tcflag_t = 128;
pub const _ICANON: crate::tcflag_t = 256;
pub const ICANON: crate::tcflag_t = 256;
pub const ALTWERASE: crate::tcflag_t = 512;
pub const _IEXTEN: crate::tcflag_t = 1024;
pub const IEXTEN: crate::tcflag_t = 1024;
pub const EXTPROC: crate::tcflag_t = 2048;
pub const _TOSTOP: crate::tcflag_t = 4194304;
pub const TOSTOP: crate::tcflag_t = 4194304;
pub const FLUSHO: crate::tcflag_t = 8388608;
pub const NOKERNINFO: crate::tcflag_t = 33554432;
pub const PENDIN: crate::tcflag_t = 536870912;
pub const _NOFLSH: crate::tcflag_t = 2147483648;
pub const NOFLSH: crate::tcflag_t = 2147483648;
pub const VEOF: usize = 0;
pub const VEOL: usize = 1;
pub const VEOL2: usize = 2;
pub const VERASE: usize = 3;
pub const VWERASE: usize = 4;
pub const VKILL: usize = 5;
pub const VREPRINT: usize = 6;
pub const VINTR: usize = 8;
pub const VQUIT: usize = 9;
pub const VSUSP: usize = 10;
pub const VDSUSP: usize = 11;
pub const VSTART: usize = 12;
pub const VSTOP: usize = 13;
pub const VLNEXT: usize = 14;
pub const VDISCARD: usize = 15;
pub const VMIN: usize = 16;
pub const VTIME: usize = 17;
pub const VSTATUS: usize = 18;
pub const NCCS: usize = 20;
pub const B0: crate::speed_t = 0;
pub const B50: crate::speed_t = 50;
pub const B75: crate::speed_t = 75;
pub const B110: crate::speed_t = 110;
pub const B134: crate::speed_t = 134;
pub const B150: crate::speed_t = 150;
pub const B200: crate::speed_t = 200;
pub const B300: crate::speed_t = 300;
pub const B600: crate::speed_t = 600;
pub const B1200: crate::speed_t = 1200;
pub const B1800: crate::speed_t = 1800;
pub const B2400: crate::speed_t = 2400;
pub const B4800: crate::speed_t = 4800;
pub const B9600: crate::speed_t = 9600;
pub const B7200: crate::speed_t = 7200;
pub const B14400: crate::speed_t = 14400;
pub const B19200: crate::speed_t = 19200;
pub const B28800: crate::speed_t = 28800;
pub const B38400: crate::speed_t = 38400;
pub const EXTA: crate::speed_t = B19200;
pub const EXTB: crate::speed_t = B38400;
pub const B57600: crate::speed_t = 57600;
pub const B76800: crate::speed_t = 76800;
pub const B115200: crate::speed_t = 115200;
pub const B230400: crate::speed_t = 230400;
pub const B460800: crate::speed_t = 460800;
pub const B500000: crate::speed_t = 500000;
pub const B576000: crate::speed_t = 576000;
pub const B921600: crate::speed_t = 921600;
pub const B1000000: crate::speed_t = 1000000;
pub const B1152000: crate::speed_t = 1152000;
pub const B1500000: crate::speed_t = 1500000;
pub const B2000000: crate::speed_t = 2000000;
pub const B2500000: crate::speed_t = 2500000;
pub const B3000000: crate::speed_t = 3000000;
pub const B3500000: crate::speed_t = 3500000;
pub const B4000000: crate::speed_t = 4000000;
pub const TCSANOW: c_int = 0;
pub const TCSADRAIN: c_int = 1;
pub const TCSAFLUSH: c_int = 2;
pub const TCSASOFT: c_int = 16;
pub const TCIFLUSH: c_int = 1;
pub const TCOFLUSH: c_int = 2;
pub const TCIOFLUSH: c_int = 3;
pub const TCOOFF: c_int = 1;
pub const TCOON: c_int = 2;
pub const TCIOFF: c_int = 3;
pub const TCION: c_int = 4;
pub const TTYDEF_IFLAG: crate::tcflag_t = 11042;
pub const TTYDEF_LFLAG: crate::tcflag_t = 1483;
pub const TTYDEF_CFLAG: crate::tcflag_t = 23040;
pub const TTYDEF_SPEED: crate::tcflag_t = 9600;
pub const CEOL: u8 = 0u8;
pub const CERASE: u8 = 127;
pub const CMIN: u8 = 1;
pub const CQUIT: u8 = 28;
pub const CTIME: u8 = 0;
pub const CBRK: u8 = 0u8;

// dlfcn.h
pub const RTLD_DEFAULT: *mut c_void = 0i64 as *mut c_void;
pub const RTLD_NEXT: *mut c_void = -1i64 as *mut c_void;
pub const RTLD_LAZY: c_int = 1;
pub const RTLD_NOW: c_int = 2;
pub const RTLD_BINDING_MASK: c_int = 3;
pub const RTLD_NOLOAD: c_int = 4;
pub const RTLD_DEEPBIND: c_int = 8;
pub const RTLD_GLOBAL: c_int = 256;
pub const RTLD_LOCAL: c_int = 0;
pub const RTLD_NODELETE: c_int = 4096;
pub const DLFO_STRUCT_HAS_EH_DBASE: usize = 1;
pub const DLFO_STRUCT_HAS_EH_COUNT: usize = 0;
pub const LM_ID_BASE: c_long = 0;
pub const LM_ID_NEWLM: c_long = -1;

// bits/signum_generic.h
pub const SIGINT: c_int = 2;
pub const SIGILL: c_int = 4;
pub const SIGABRT: c_int = 6;
pub const SIGFPE: c_int = 8;
pub const SIGSEGV: c_int = 11;
pub const SIGTERM: c_int = 15;
pub const SIGHUP: c_int = 1;
pub const SIGQUIT: c_int = 3;
pub const SIGTRAP: c_int = 5;
pub const SIGKILL: c_int = 9;
pub const SIGPIPE: c_int = 13;
pub const SIGALRM: c_int = 14;
pub const SIGIOT: c_int = 6;
pub const SIGBUS: c_int = 10;
pub const SIGSYS: c_int = 12;
pub const SIGEMT: c_int = 7;
pub const SIGINFO: c_int = 29;
pub const SIGLOST: c_int = 32;
pub const SIGURG: c_int = 16;
pub const SIGSTOP: c_int = 17;
pub const SIGTSTP: c_int = 18;
pub const SIGCONT: c_int = 19;
pub const SIGCHLD: c_int = 20;
pub const SIGTTIN: c_int = 21;
pub const SIGTTOU: c_int = 22;
pub const SIGPOLL: c_int = 23;
pub const SIGXCPU: c_int = 24;
pub const SIGVTALRM: c_int = 26;
pub const SIGPROF: c_int = 27;
pub const SIGXFSZ: c_int = 25;
pub const SIGUSR1: c_int = 30;
pub const SIGUSR2: c_int = 31;
pub const SIGWINCH: c_int = 28;
pub const SIGIO: c_int = 23;
pub const SIGCLD: c_int = 20;
pub const __SIGRTMIN: usize = 32;
pub const __SIGRTMAX: usize = 32;
pub const _NSIG: usize = 33;
pub const NSIG: usize = 33;

// bits/sigaction.h
pub const SA_ONSTACK: c_int = 1;
pub const SA_RESTART: c_int = 2;
pub const SA_NODEFER: c_int = 16;
pub const SA_RESETHAND: c_int = 4;
pub const SA_NOCLDSTOP: c_int = 8;
pub const SA_SIGINFO: c_int = 64;
pub const SA_INTERRUPT: c_int = 0;
pub const SA_NOMASK: c_int = 16;
pub const SA_ONESHOT: c_int = 4;
pub const SA_STACK: c_int = 1;
pub const SIG_BLOCK: c_int = 1;
pub const SIG_UNBLOCK: c_int = 2;
pub const SIG_SETMASK: c_int = 3;

// bits/sigcontext.h
pub const FPC_IE: u16 = 1;
pub const FPC_IM: u16 = 1;
pub const FPC_DE: u16 = 2;
pub const FPC_DM: u16 = 2;
pub const FPC_ZE: u16 = 4;
pub const FPC_ZM: u16 = 4;
pub const FPC_OE: u16 = 8;
pub const FPC_OM: u16 = 8;
pub const FPC_UE: u16 = 16;
pub const FPC_PE: u16 = 32;
pub const FPC_PC: u16 = 768;
pub const FPC_PC_24: u16 = 0;
pub const FPC_PC_53: u16 = 512;
pub const FPC_PC_64: u16 = 768;
pub const FPC_RC: u16 = 3072;
pub const FPC_RC_RN: u16 = 0;
pub const FPC_RC_RD: u16 = 1024;
pub const FPC_RC_RU: u16 = 2048;
pub const FPC_RC_CHOP: u16 = 3072;
pub const FPC_IC: u16 = 4096;
pub const FPC_IC_PROJ: u16 = 0;
pub const FPC_IC_AFF: u16 = 4096;
pub const FPS_IE: u16 = 1;
pub const FPS_DE: u16 = 2;
pub const FPS_ZE: u16 = 4;
pub const FPS_OE: u16 = 8;
pub const FPS_UE: u16 = 16;
pub const FPS_PE: u16 = 32;
pub const FPS_SF: u16 = 64;
pub const FPS_ES: u16 = 128;
pub const FPS_C0: u16 = 256;
pub const FPS_C1: u16 = 512;
pub const FPS_C2: u16 = 1024;
pub const FPS_TOS: u16 = 14336;
pub const FPS_TOS_SHIFT: u16 = 11;
pub const FPS_C3: u16 = 16384;
pub const FPS_BUSY: u16 = 32768;
pub const FPE_INTOVF_TRAP: c_int = 1;
pub const FPE_INTDIV_FAULT: c_int = 2;
pub const FPE_FLTOVF_FAULT: c_int = 3;
pub const FPE_FLTDIV_FAULT: c_int = 4;
pub const FPE_FLTUND_FAULT: c_int = 5;
pub const FPE_SUBRNG_FAULT: c_int = 7;
pub const FPE_FLTDNR_FAULT: c_int = 8;
pub const FPE_FLTINX_FAULT: c_int = 9;
pub const FPE_EMERR_FAULT: c_int = 10;
pub const FPE_EMBND_FAULT: c_int = 11;
pub const ILL_INVOPR_FAULT: c_int = 1;
pub const ILL_STACK_FAULT: c_int = 2;
pub const ILL_FPEOPR_FAULT: c_int = 3;
pub const DBG_SINGLE_TRAP: c_int = 1;
pub const DBG_BRKPNT_FAULT: c_int = 2;
pub const __NGREG: usize = 19;
pub const NGREG: usize = 19;

// bits/sigstack.h
pub const MINSIGSTKSZ: usize = 8192;
pub const SIGSTKSZ: usize = 40960;

// sys/stat.h
pub const __S_IFMT: mode_t = 0o17_0000;
pub const __S_IFDIR: mode_t = 0o4_0000;
pub const __S_IFCHR: mode_t = 0o2_0000;
pub const __S_IFBLK: mode_t = 0o6_0000;
pub const __S_IFREG: mode_t = 0o10_0000;
pub const __S_IFLNK: mode_t = 0o12_0000;
pub const __S_IFSOCK: mode_t = 0o14_0000;
pub const __S_IFIFO: mode_t = 0o1_0000;
pub const __S_ISUID: mode_t = 0o4000;
pub const __S_ISGID: mode_t = 0o2000;
pub const __S_ISVTX: mode_t = 0o1000;
pub const __S_IREAD: mode_t = 0o0400;
pub const __S_IWRITE: mode_t = 0o0200;
pub const __S_IEXEC: mode_t = 0o0100;
pub const S_INOCACHE: mode_t = 0o20_0000;
pub const S_IUSEUNK: mode_t = 0o40_0000;
pub const S_IUNKNOWN: mode_t = 0o700_0000;
pub const S_IUNKSHIFT: mode_t = 0o0014;
pub const S_IPTRANS: mode_t = 0o1000_0000;
pub const S_IATRANS: mode_t = 0o2000_0000;
pub const S_IROOT: mode_t = 0o4000_0000;
pub const S_ITRANS: mode_t = 0o7000_0000;
pub const S_IMMAP0: mode_t = 0o10000_0000;
pub const CMASK: mode_t = 18;
pub const UF_SETTABLE: c_uint = 65535;
pub const UF_NODUMP: c_uint = 1;
pub const UF_IMMUTABLE: c_uint = 2;
pub const UF_APPEND: c_uint = 4;
pub const UF_OPAQUE: c_uint = 8;
pub const UF_NOUNLINK: c_uint = 16;
pub const SF_SETTABLE: c_uint = 4294901760;
pub const SF_ARCHIVED: c_uint = 65536;
pub const SF_IMMUTABLE: c_uint = 131072;
pub const SF_APPEND: c_uint = 262144;
pub const SF_NOUNLINK: c_uint = 1048576;
pub const SF_SNAPSHOT: c_uint = 2097152;
pub const UTIME_NOW: c_long = -1;
pub const UTIME_OMIT: c_long = -2;
pub const S_IFMT: mode_t = 0o17_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFIFO: mode_t = 0o1_0000;
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_ISUID: mode_t = 0o4000;
pub const S_ISGID: mode_t = 0o2000;
pub const S_ISVTX: mode_t = 0o1000;
pub const S_IRUSR: mode_t = 0o0400;
pub const S_IWUSR: mode_t = 0o0200;
pub const S_IXUSR: mode_t = 0o0100;
pub const S_IRWXU: mode_t = 0o0700;
pub const S_IREAD: mode_t = 0o0400;
pub const S_IWRITE: mode_t = 0o0200;
pub const S_IEXEC: mode_t = 0o0100;
pub const S_IRGRP: mode_t = 0o0040;
pub const S_IWGRP: mode_t = 0o0020;
pub const S_IXGRP: mode_t = 0o0010;
pub const S_IRWXG: mode_t = 0o0070;
pub const S_IROTH: mode_t = 0o0004;
pub const S_IWOTH: mode_t = 0o0002;
pub const S_IXOTH: mode_t = 0o0001;
pub const S_IRWXO: mode_t = 0o0007;
pub const ACCESSPERMS: mode_t = 511;
pub const ALLPERMS: mode_t = 4095;
pub const DEFFILEMODE: mode_t = 438;
pub const S_BLKSIZE: usize = 512;
pub const STATX_TYPE: c_uint = 1;
pub const STATX_MODE: c_uint = 2;
pub const STATX_NLINK: c_uint = 4;
pub const STATX_UID: c_uint = 8;
pub const STATX_GID: c_uint = 16;
pub const STATX_ATIME: c_uint = 32;
pub const STATX_MTIME: c_uint = 64;
pub const STATX_CTIME: c_uint = 128;
pub const STATX_INO: c_uint = 256;
pub const STATX_SIZE: c_uint = 512;
pub const STATX_BLOCKS: c_uint = 1024;
pub const STATX_BASIC_STATS: c_uint = 2047;
pub const STATX_ALL: c_uint = 4095;
pub const STATX_BTIME: c_uint = 2048;
pub const STATX_MNT_ID: c_uint = 4096;
pub const STATX_DIOALIGN: c_uint = 8192;
pub const STATX__RESERVED: c_uint = 2147483648;
pub const STATX_ATTR_COMPRESSED: c_uint = 4;
pub const STATX_ATTR_IMMUTABLE: c_uint = 16;
pub const STATX_ATTR_APPEND: c_uint = 32;
pub const STATX_ATTR_NODUMP: c_uint = 64;
pub const STATX_ATTR_ENCRYPTED: c_uint = 2048;
pub const STATX_ATTR_AUTOMOUNT: c_uint = 4096;
pub const STATX_ATTR_MOUNT_ROOT: c_uint = 8192;
pub const STATX_ATTR_VERITY: c_uint = 1048576;
pub const STATX_ATTR_DAX: c_uint = 2097152;

// sys/ioctl.h
pub const TIOCM_LE: c_int = 1;
pub const TIOCM_DTR: c_int = 2;
pub const TIOCM_RTS: c_int = 4;
pub const TIOCM_ST: c_int = 8;
pub const TIOCM_SR: c_int = 16;
pub const TIOCM_CTS: c_int = 32;
pub const TIOCM_CAR: c_int = 64;
pub const TIOCM_CD: c_int = 64;
pub const TIOCM_RNG: c_int = 128;
pub const TIOCM_RI: c_int = 128;
pub const TIOCM_DSR: c_int = 256;
pub const TIOCPKT_DATA: c_int = 0;
pub const TIOCPKT_FLUSHREAD: c_int = 1;
pub const TIOCPKT_FLUSHWRITE: c_int = 2;
pub const TIOCPKT_STOP: c_int = 4;
pub const TIOCPKT_START: c_int = 8;
pub const TIOCPKT_NOSTOP: c_int = 16;
pub const TIOCPKT_DOSTOP: c_int = 32;
pub const TIOCPKT_IOCTL: c_int = 64;
pub const TTYDISC: c_int = 0;
pub const TABLDISC: c_int = 3;
pub const SLIPDISC: c_int = 4;
pub const TANDEM: crate::tcflag_t = 1;
pub const CBREAK: crate::tcflag_t = 2;
pub const LCASE: crate::tcflag_t = 4;
pub const CRMOD: crate::tcflag_t = 16;
pub const RAW: crate::tcflag_t = 32;
pub const ODDP: crate::tcflag_t = 64;
pub const EVENP: crate::tcflag_t = 128;
pub const ANYP: crate::tcflag_t = 192;
pub const NLDELAY: crate::tcflag_t = 768;
pub const NL2: crate::tcflag_t = 512;
pub const NL3: crate::tcflag_t = 768;
pub const TBDELAY: crate::tcflag_t = 3072;
pub const XTABS: crate::tcflag_t = 3072;
pub const CRDELAY: crate::tcflag_t = 12288;
pub const VTDELAY: crate::tcflag_t = 16384;
pub const BSDELAY: crate::tcflag_t = 32768;
pub const ALLDELAY: crate::tcflag_t = 65280;
pub const CRTBS: crate::tcflag_t = 65536;
pub const PRTERA: crate::tcflag_t = 131072;
pub const CRTERA: crate::tcflag_t = 262144;
pub const TILDE: crate::tcflag_t = 524288;
pub const LITOUT: crate::tcflag_t = 2097152;
pub const NOHANG: crate::tcflag_t = 16777216;
pub const L001000: crate::tcflag_t = 33554432;
pub const CRTKIL: crate::tcflag_t = 67108864;
pub const PASS8: crate::tcflag_t = 134217728;
pub const CTLECH: crate::tcflag_t = 268435456;
pub const DECCTQ: crate::tcflag_t = 1073741824;

pub const FIONBIO: c_ulong = 0xa008007e;
pub const FIONREAD: c_ulong = 0x6008007f;
pub const TIOCSWINSZ: c_ulong = 0x90200767;
pub const TIOCGWINSZ: c_ulong = 0x50200768;
pub const TIOCEXCL: c_ulong = 0x70d;
pub const TIOCNXCL: c_ulong = 0x70e;
pub const TIOCSCTTY: c_ulong = 0x761;

pub const FIOCLEX: c_ulong = 1;

// fcntl.h
pub const O_EXEC: c_int = 4;
pub const O_NORW: c_int = 0;
pub const O_RDONLY: c_int = 1;
pub const O_WRONLY: c_int = 2;
pub const O_RDWR: c_int = 3;
pub const O_ACCMODE: c_int = 3;
pub const O_LARGEFILE: c_int = 0;
pub const O_CREAT: c_int = 16;
pub const O_EXCL: c_int = 32;
pub const O_NOLINK: c_int = 64;
pub const O_NOTRANS: c_int = 128;
pub const O_NOFOLLOW: c_int = 1048576;
pub const O_DIRECTORY: c_int = 2097152;
pub const O_APPEND: c_int = 256;
pub const O_ASYNC: c_int = 512;
pub const O_FSYNC: c_int = 1024;
pub const O_SYNC: c_int = 1024;
pub const O_NOATIME: c_int = 2048;
pub const O_SHLOCK: c_int = 131072;
pub const O_EXLOCK: c_int = 262144;
pub const O_DSYNC: c_int = 1024;
pub const O_RSYNC: c_int = 1024;
pub const O_NONBLOCK: c_int = 8;
pub const O_NDELAY: c_int = 8;
pub const O_HURD: c_int = 458751;
pub const O_TRUNC: c_int = 65536;
pub const O_CLOEXEC: c_int = 4194304;
pub const O_IGNORE_CTTY: c_int = 524288;
pub const O_TMPFILE: c_int = 8388608;
pub const O_NOCTTY: c_int = 0;
pub const FREAD: c_int = 1;
pub const FWRITE: c_int = 2;
pub const FASYNC: c_int = 512;
pub const FCREAT: c_int = 16;
pub const FEXCL: c_int = 32;
pub const FTRUNC: c_int = 65536;
pub const FNOCTTY: c_int = 0;
pub const FFSYNC: c_int = 1024;
pub const FSYNC: c_int = 1024;
pub const FAPPEND: c_int = 256;
pub const FNONBLOCK: c_int = 8;
pub const FNDELAY: c_int = 8;
pub const F_DUPFD: c_int = 0;
pub const F_GETFD: c_int = 1;
pub const F_SETFD: c_int = 2;
pub const F_GETFL: c_int = 3;
pub const F_SETFL: c_int = 4;
pub const F_GETOWN: c_int = 5;
pub const F_SETOWN: c_int = 6;
pub const F_GETLK: c_int = 7;
pub const F_SETLK: c_int = 8;
pub const F_SETLKW: c_int = 9;
pub const F_GETLK64: c_int = 10;
pub const F_SETLK64: c_int = 11;
pub const F_SETLKW64: c_int = 12;
pub const F_DUPFD_CLOEXEC: c_int = 1030;
pub const FD_CLOEXEC: c_int = 1;
pub const F_RDLCK: c_int = 1;
pub const F_WRLCK: c_int = 2;
pub const F_UNLCK: c_int = 3;
pub const POSIX_FADV_NORMAL: c_int = 0;
pub const POSIX_FADV_RANDOM: c_int = 1;
pub const POSIX_FADV_SEQUENTIAL: c_int = 2;
pub const POSIX_FADV_WILLNEED: c_int = 3;
pub const POSIX_FADV_DONTNEED: c_int = 4;
pub const POSIX_FADV_NOREUSE: c_int = 5;
pub const AT_FDCWD: c_int = -100;
pub const AT_SYMLINK_NOFOLLOW: c_int = 256;
pub const AT_REMOVEDIR: c_int = 512;
pub const AT_SYMLINK_FOLLOW: c_int = 1024;
pub const AT_NO_AUTOMOUNT: c_int = 2048;
pub const AT_EMPTY_PATH: c_int = 4096;
pub const AT_STATX_SYNC_TYPE: c_int = 24576;
pub const AT_STATX_SYNC_AS_STAT: c_int = 0;
pub const AT_STATX_FORCE_SYNC: c_int = 8192;
pub const AT_STATX_DONT_SYNC: c_int = 16384;
pub const AT_RECURSIVE: c_int = 32768;
pub const AT_EACCESS: c_int = 512;

// sys/uio.h
pub const RWF_HIPRI: c_int = 1;
pub const RWF_DSYNC: c_int = 2;
pub const RWF_SYNC: c_int = 4;
pub const RWF_NOWAIT: c_int = 8;
pub const RWF_APPEND: c_int = 16;

// errno.h
pub const EPERM: c_int = 1073741825;
pub const ENOENT: c_int = 1073741826;
pub const ESRCH: c_int = 1073741827;
pub const EINTR: c_int = 1073741828;
pub const EIO: c_int = 1073741829;
pub const ENXIO: c_int = 1073741830;
pub const E2BIG: c_int = 1073741831;
pub const ENOEXEC: c_int = 1073741832;
pub const EBADF: c_int = 1073741833;
pub const ECHILD: c_int = 1073741834;
pub const EDEADLK: c_int = 1073741835;
pub const ENOMEM: c_int = 1073741836;
pub const EACCES: c_int = 1073741837;
pub const EFAULT: c_int = 1073741838;
pub const ENOTBLK: c_int = 1073741839;
pub const EBUSY: c_int = 1073741840;
pub const EEXIST: c_int = 1073741841;
pub const EXDEV: c_int = 1073741842;
pub const ENODEV: c_int = 1073741843;
pub const ENOTDIR: c_int = 1073741844;
pub const EISDIR: c_int = 1073741845;
pub const EINVAL: c_int = 1073741846;
pub const EMFILE: c_int = 1073741848;
pub const ENFILE: c_int = 1073741847;
pub const ENOTTY: c_int = 1073741849;
pub const ETXTBSY: c_int = 1073741850;
pub const EFBIG: c_int = 1073741851;
pub const ENOSPC: c_int = 1073741852;
pub const ESPIPE: c_int = 1073741853;
pub const EROFS: c_int = 1073741854;
pub const EMLINK: c_int = 1073741855;
pub const EPIPE: c_int = 1073741856;
pub const EDOM: c_int = 1073741857;
pub const ERANGE: c_int = 1073741858;
pub const EAGAIN: c_int = 1073741859;
pub const EWOULDBLOCK: c_int = 1073741859;
pub const EINPROGRESS: c_int = 1073741860;
pub const EALREADY: c_int = 1073741861;
pub const ENOTSOCK: c_int = 1073741862;
pub const EMSGSIZE: c_int = 1073741864;
pub const EPROTOTYPE: c_int = 1073741865;
pub const ENOPROTOOPT: c_int = 1073741866;
pub const EPROTONOSUPPORT: c_int = 1073741867;
pub const ESOCKTNOSUPPORT: c_int = 1073741868;
pub const EOPNOTSUPP: c_int = 1073741869;
pub const EPFNOSUPPORT: c_int = 1073741870;
pub const EAFNOSUPPORT: c_int = 1073741871;
pub const EADDRINUSE: c_int = 1073741872;
pub const EADDRNOTAVAIL: c_int = 1073741873;
pub const ENETDOWN: c_int = 1073741874;
pub const ENETUNREACH: c_int = 1073741875;
pub const ENETRESET: c_int = 1073741876;
pub const ECONNABORTED: c_int = 1073741877;
pub const ECONNRESET: c_int = 1073741878;
pub const ENOBUFS: c_int = 1073741879;
pub const EISCONN: c_int = 1073741880;
pub const ENOTCONN: c_int = 1073741881;
pub const EDESTADDRREQ: c_int = 1073741863;
pub const ESHUTDOWN: c_int = 1073741882;
pub const ETOOMANYREFS: c_int = 1073741883;
pub const ETIMEDOUT: c_int = 1073741884;
pub const ECONNREFUSED: c_int = 1073741885;
pub const ELOOP: c_int = 1073741886;
pub const ENAMETOOLONG: c_int = 1073741887;
pub const EHOSTDOWN: c_int = 1073741888;
pub const EHOSTUNREACH: c_int = 1073741889;
pub const ENOTEMPTY: c_int = 1073741890;
pub const EPROCLIM: c_int = 1073741891;
pub const EUSERS: c_int = 1073741892;
pub const EDQUOT: c_int = 1073741893;
pub const ESTALE: c_int = 1073741894;
pub const EREMOTE: c_int = 1073741895;
pub const EBADRPC: c_int = 1073741896;
pub const ERPCMISMATCH: c_int = 1073741897;
pub const EPROGUNAVAIL: c_int = 1073741898;
pub const EPROGMISMATCH: c_int = 1073741899;
pub const EPROCUNAVAIL: c_int = 1073741900;
pub const ENOLCK: c_int = 1073741901;
pub const EFTYPE: c_int = 1073741903;
pub const EAUTH: c_int = 1073741904;
pub const ENEEDAUTH: c_int = 1073741905;
pub const ENOSYS: c_int = 1073741902;
pub const ELIBEXEC: c_int = 1073741907;
pub const ENOTSUP: c_int = 1073741942;
pub const EILSEQ: c_int = 1073741930;
pub const EBACKGROUND: c_int = 1073741924;
pub const EDIED: c_int = 1073741925;
pub const EGREGIOUS: c_int = 1073741927;
pub const EIEIO: c_int = 1073741928;
pub const EGRATUITOUS: c_int = 1073741929;
pub const EBADMSG: c_int = 1073741931;
pub const EIDRM: c_int = 1073741932;
pub const EMULTIHOP: c_int = 1073741933;
pub const ENODATA: c_int = 1073741934;
pub const ENOLINK: c_int = 1073741935;
pub const ENOMSG: c_int = 1073741936;
pub const ENOSR: c_int = 1073741937;
pub const ENOSTR: c_int = 1073741938;
pub const EOVERFLOW: c_int = 1073741939;
pub const EPROTO: c_int = 1073741940;
pub const ETIME: c_int = 1073741941;
pub const ECANCELED: c_int = 1073741943;
pub const EOWNERDEAD: c_int = 1073741944;
pub const ENOTRECOVERABLE: c_int = 1073741945;
pub const EMACH_SEND_IN_PROGRESS: c_int = 268435457;
pub const EMACH_SEND_INVALID_DATA: c_int = 268435458;
pub const EMACH_SEND_INVALID_DEST: c_int = 268435459;
pub const EMACH_SEND_TIMED_OUT: c_int = 268435460;
pub const EMACH_SEND_WILL_NOTIFY: c_int = 268435461;
pub const EMACH_SEND_NOTIFY_IN_PROGRESS: c_int = 268435462;
pub const EMACH_SEND_INTERRUPTED: c_int = 268435463;
pub const EMACH_SEND_MSG_TOO_SMALL: c_int = 268435464;
pub const EMACH_SEND_INVALID_REPLY: c_int = 268435465;
pub const EMACH_SEND_INVALID_RIGHT: c_int = 268435466;
pub const EMACH_SEND_INVALID_NOTIFY: c_int = 268435467;
pub const EMACH_SEND_INVALID_MEMORY: c_int = 268435468;
pub const EMACH_SEND_NO_BUFFER: c_int = 268435469;
pub const EMACH_SEND_NO_NOTIFY: c_int = 268435470;
pub const EMACH_SEND_INVALID_TYPE: c_int = 268435471;
pub const EMACH_SEND_INVALID_HEADER: c_int = 268435472;
pub const EMACH_RCV_IN_PROGRESS: c_int = 268451841;
pub const EMACH_RCV_INVALID_NAME: c_int = 268451842;
pub const EMACH_RCV_TIMED_OUT: c_int = 268451843;
pub const EMACH_RCV_TOO_LARGE: c_int = 268451844;
pub const EMACH_RCV_INTERRUPTED: c_int = 268451845;
pub const EMACH_RCV_PORT_CHANGED: c_int = 268451846;
pub const EMACH_RCV_INVALID_NOTIFY: c_int = 268451847;
pub const EMACH_RCV_INVALID_DATA: c_int = 268451848;
pub const EMACH_RCV_PORT_DIED: c_int = 268451849;
pub const EMACH_RCV_IN_SET: c_int = 268451850;
pub const EMACH_RCV_HEADER_ERROR: c_int = 268451851;
pub const EMACH_RCV_BODY_ERROR: c_int = 268451852;
pub const EKERN_INVALID_ADDRESS: c_int = 1;
pub const EKERN_PROTECTION_FAILURE: c_int = 2;
pub const EKERN_NO_SPACE: c_int = 3;
pub const EKERN_INVALID_ARGUMENT: c_int = 4;
pub const EKERN_FAILURE: c_int = 5;
pub const EKERN_RESOURCE_SHORTAGE: c_int = 6;
pub const EKERN_NOT_RECEIVER: c_int = 7;
pub const EKERN_NO_ACCESS: c_int = 8;
pub const EKERN_MEMORY_FAILURE: c_int = 9;
pub const EKERN_MEMORY_ERROR: c_int = 10;
pub const EKERN_NOT_IN_SET: c_int = 12;
pub const EKERN_NAME_EXISTS: c_int = 13;
pub const EKERN_ABORTED: c_int = 14;
pub const EKERN_INVALID_NAME: c_int = 15;
pub const EKERN_INVALID_TASK: c_int = 16;
pub const EKERN_INVALID_RIGHT: c_int = 17;
pub const EKERN_INVALID_VALUE: c_int = 18;
pub const EKERN_UREFS_OVERFLOW: c_int = 19;
pub const EKERN_INVALID_CAPABILITY: c_int = 20;
pub const EKERN_RIGHT_EXISTS: c_int = 21;
pub const EKERN_INVALID_HOST: c_int = 22;
pub const EKERN_MEMORY_PRESENT: c_int = 23;
pub const EKERN_WRITE_PROTECTION_FAILURE: c_int = 24;
pub const EKERN_TERMINATED: c_int = 26;
pub const EKERN_TIMEDOUT: c_int = 27;
pub const EKERN_INTERRUPTED: c_int = 28;
pub const EMIG_TYPE_ERROR: c_int = -300;
pub const EMIG_REPLY_MISMATCH: c_int = -301;
pub const EMIG_REMOTE_ERROR: c_int = -302;
pub const EMIG_BAD_ID: c_int = -303;
pub const EMIG_BAD_ARGUMENTS: c_int = -304;
pub const EMIG_NO_REPLY: c_int = -305;
pub const EMIG_EXCEPTION: c_int = -306;
pub const EMIG_ARRAY_TOO_LARGE: c_int = -307;
pub const EMIG_SERVER_DIED: c_int = -308;
pub const EMIG_DESTROY_REQUEST: c_int = -309;
pub const ED_IO_ERROR: c_int = 2500;
pub const ED_WOULD_BLOCK: c_int = 2501;
pub const ED_NO_SUCH_DEVICE: c_int = 2502;
pub const ED_ALREADY_OPEN: c_int = 2503;
pub const ED_DEVICE_DOWN: c_int = 2504;
pub const ED_INVALID_OPERATION: c_int = 2505;
pub const ED_INVALID_RECNUM: c_int = 2506;
pub const ED_INVALID_SIZE: c_int = 2507;
pub const ED_NO_MEMORY: c_int = 2508;
pub const ED_READ_ONLY: c_int = 2509;
pub const _HURD_ERRNOS: usize = 122;

// sched.h
pub const SCHED_OTHER: c_int = 0;
pub const SCHED_FIFO: c_int = 1;
pub const SCHED_RR: c_int = 2;
pub const _BITS_TYPES_STRUCT_SCHED_PARAM: usize = 1;
pub const __CPU_SETSIZE: usize = 1024;
pub const CPU_SETSIZE: usize = 1024;

// pthread.h
pub const PTHREAD_SPINLOCK_INITIALIZER: c_int = 0;
pub const PTHREAD_CANCEL_DISABLE: c_int = 0;
pub const PTHREAD_CANCEL_ENABLE: c_int = 1;
pub const PTHREAD_CANCEL_DEFERRED: c_int = 0;
pub const PTHREAD_CANCEL_ASYNCHRONOUS: c_int = 1;
pub const PTHREAD_BARRIER_SERIAL_THREAD: c_int = -1;

// netinet/tcp.h
pub const TCP_NODELAY: c_int = 1;
pub const TCP_MAXSEG: c_int = 2;
pub const TCP_CORK: c_int = 3;
pub const TCP_KEEPIDLE: c_int = 4;
pub const TCP_KEEPINTVL: c_int = 5;
pub const TCP_KEEPCNT: c_int = 6;
pub const TCP_SYNCNT: c_int = 7;
pub const TCP_LINGER2: c_int = 8;
pub const TCP_DEFER_ACCEPT: c_int = 9;
pub const TCP_WINDOW_CLAMP: c_int = 10;
pub const TCP_INFO: c_int = 11;
pub const TCP_QUICKACK: c_int = 12;
pub const TCP_CONGESTION: c_int = 13;
pub const TCP_MD5SIG: c_int = 14;
pub const TCP_COOKIE_TRANSACTIONS: c_int = 15;
pub const TCP_THIN_LINEAR_TIMEOUTS: c_int = 16;
pub const TCP_THIN_DUPACK: c_int = 17;
pub const TCP_USER_TIMEOUT: c_int = 18;
pub const TCP_REPAIR: c_int = 19;
pub const TCP_REPAIR_QUEUE: c_int = 20;
pub const TCP_QUEUE_SEQ: c_int = 21;
pub const TCP_REPAIR_OPTIONS: c_int = 22;
pub const TCP_FASTOPEN: c_int = 23;
pub const TCP_TIMESTAMP: c_int = 24;
pub const TCP_NOTSENT_LOWAT: c_int = 25;
pub const TCP_CC_INFO: c_int = 26;
pub const TCP_SAVE_SYN: c_int = 27;
pub const TCP_SAVED_SYN: c_int = 28;
pub const TCP_REPAIR_WINDOW: c_int = 29;
pub const TCP_FASTOPEN_CONNECT: c_int = 30;
pub const TCP_ULP: c_int = 31;
pub const TCP_MD5SIG_EXT: c_int = 32;
pub const TCP_FASTOPEN_KEY: c_int = 33;
pub const TCP_FASTOPEN_NO_COOKIE: c_int = 34;
pub const TCP_ZEROCOPY_RECEIVE: c_int = 35;
pub const TCP_INQ: c_int = 36;
pub const TCP_CM_INQ: c_int = 36;
pub const TCP_TX_DELAY: c_int = 37;
pub const TCP_REPAIR_ON: c_int = 1;
pub const TCP_REPAIR_OFF: c_int = 0;
pub const TCP_REPAIR_OFF_NO_WP: c_int = -1;

// stdint.h
pub const INT8_MIN: i8 = -128;
pub const INT16_MIN: i16 = -32768;
pub const INT32_MIN: i32 = -2147483648;
pub const INT8_MAX: i8 = 127;
pub const INT16_MAX: i16 = 32767;
pub const INT32_MAX: i32 = 2147483647;
pub const UINT8_MAX: u8 = 255;
pub const UINT16_MAX: u16 = 65535;
pub const UINT32_MAX: u32 = 4294967295;
pub const INT_LEAST8_MIN: int_least8_t = -128;
pub const INT_LEAST16_MIN: int_least16_t = -32768;
pub const INT_LEAST32_MIN: int_least32_t = -2147483648;
pub const INT_LEAST8_MAX: int_least8_t = 127;
pub const INT_LEAST16_MAX: int_least16_t = 32767;
pub const INT_LEAST32_MAX: int_least32_t = 2147483647;
pub const UINT_LEAST8_MAX: uint_least8_t = 255;
pub const UINT_LEAST16_MAX: uint_least16_t = 65535;
pub const UINT_LEAST32_MAX: uint_least32_t = 4294967295;
pub const INT_FAST8_MIN: int_fast8_t = -128;
pub const INT_FAST16_MIN: int_fast16_t = -2147483648;
pub const INT_FAST32_MIN: int_fast32_t = -2147483648;
pub const INT_FAST8_MAX: int_fast8_t = 127;
pub const INT_FAST16_MAX: int_fast16_t = 2147483647;
pub const INT_FAST32_MAX: int_fast32_t = 2147483647;
pub const UINT_FAST8_MAX: uint_fast8_t = 255;
pub const UINT_FAST16_MAX: uint_fast16_t = 4294967295;
pub const UINT_FAST32_MAX: uint_fast32_t = 4294967295;
pub const INTPTR_MIN: __intptr_t = -2147483648;
pub const INTPTR_MAX: __intptr_t = 2147483647;
pub const UINTPTR_MAX: usize = 4294967295;
pub const PTRDIFF_MIN: __ptrdiff_t = -2147483648;
pub const PTRDIFF_MAX: __ptrdiff_t = 2147483647;
pub const SIG_ATOMIC_MIN: __sig_atomic_t = -2147483648;
pub const SIG_ATOMIC_MAX: __sig_atomic_t = 2147483647;
pub const SIZE_MAX: usize = 4294967295;
pub const WINT_MIN: wint_t = 0;
pub const WINT_MAX: wint_t = 4294967295;
pub const INT8_WIDTH: usize = 8;
pub const UINT8_WIDTH: usize = 8;
pub const INT16_WIDTH: usize = 16;
pub const UINT16_WIDTH: usize = 16;
pub const INT32_WIDTH: usize = 32;
pub const UINT32_WIDTH: usize = 32;
pub const INT64_WIDTH: usize = 64;
pub const UINT64_WIDTH: usize = 64;
pub const INT_LEAST8_WIDTH: usize = 8;
pub const UINT_LEAST8_WIDTH: usize = 8;
pub const INT_LEAST16_WIDTH: usize = 16;
pub const UINT_LEAST16_WIDTH: usize = 16;
pub const INT_LEAST32_WIDTH: usize = 32;
pub const UINT_LEAST32_WIDTH: usize = 32;
pub const INT_LEAST64_WIDTH: usize = 64;
pub const UINT_LEAST64_WIDTH: usize = 64;
pub const INT_FAST8_WIDTH: usize = 8;
pub const UINT_FAST8_WIDTH: usize = 8;
pub const INT_FAST16_WIDTH: usize = 32;
pub const UINT_FAST16_WIDTH: usize = 32;
pub const INT_FAST32_WIDTH: usize = 32;
pub const UINT_FAST32_WIDTH: usize = 32;
pub const INT_FAST64_WIDTH: usize = 64;
pub const UINT_FAST64_WIDTH: usize = 64;
pub const INTPTR_WIDTH: usize = 32;
pub const UINTPTR_WIDTH: usize = 32;
pub const INTMAX_WIDTH: usize = 64;
pub const UINTMAX_WIDTH: usize = 64;
pub const PTRDIFF_WIDTH: usize = 32;
pub const SIG_ATOMIC_WIDTH: usize = 32;
pub const SIZE_WIDTH: usize = 32;
pub const WCHAR_WIDTH: usize = 32;
pub const WINT_WIDTH: usize = 32;

pub const TH_FIN: u8 = 1;
pub const TH_SYN: u8 = 2;
pub const TH_RST: u8 = 4;
pub const TH_PUSH: u8 = 8;
pub const TH_ACK: u8 = 16;
pub const TH_URG: u8 = 32;
pub const TCPOPT_EOL: u8 = 0;
pub const TCPOPT_NOP: u8 = 1;
pub const TCPOPT_MAXSEG: u8 = 2;
pub const TCPOLEN_MAXSEG: u8 = 4;
pub const TCPOPT_WINDOW: u8 = 3;
pub const TCPOLEN_WINDOW: u8 = 3;
pub const TCPOPT_SACK_PERMITTED: u8 = 4;
pub const TCPOLEN_SACK_PERMITTED: u8 = 2;
pub const TCPOPT_SACK: u8 = 5;
pub const TCPOPT_TIMESTAMP: u8 = 8;
pub const TCPOLEN_TIMESTAMP: u8 = 10;
pub const TCPOLEN_TSTAMP_APPA: u8 = 12;
pub const TCPOPT_TSTAMP_HDR: u32 = 16844810;
pub const TCP_MSS: usize = 512;
pub const TCP_MAXWIN: usize = 65535;
pub const TCP_MAX_WINSHIFT: usize = 14;
pub const TCPI_OPT_TIMESTAMPS: u8 = 1;
pub const TCPI_OPT_SACK: u8 = 2;
pub const TCPI_OPT_WSCALE: u8 = 4;
pub const TCPI_OPT_ECN: u8 = 8;
pub const TCPI_OPT_ECN_SEEN: u8 = 16;
pub const TCPI_OPT_SYN_DATA: u8 = 32;
pub const TCP_MD5SIG_MAXKEYLEN: usize = 80;
pub const TCP_MD5SIG_FLAG_PREFIX: usize = 1;
pub const TCP_COOKIE_MIN: usize = 8;
pub const TCP_COOKIE_MAX: usize = 16;
pub const TCP_COOKIE_PAIR_SIZE: usize = 32;
pub const TCP_COOKIE_IN_ALWAYS: c_int = 1;
pub const TCP_COOKIE_OUT_NEVER: c_int = 2;
pub const TCP_S_DATA_IN: c_int = 4;
pub const TCP_S_DATA_OUT: c_int = 8;
pub const TCP_MSS_DEFAULT: usize = 536;
pub const TCP_MSS_DESIRED: usize = 1220;

// sys/wait.h
pub const WCOREFLAG: c_int = 128;
pub const WAIT_ANY: pid_t = -1;
pub const WAIT_MYPGRP: pid_t = 0;

// sys/file.h
pub const LOCK_SH: c_int = 1;
pub const LOCK_EX: c_int = 2;
pub const LOCK_UN: c_int = 8;
pub const LOCK_NB: c_int = 4;

// sys/mman.h
pub const PROT_NONE: c_int = 0;
pub const PROT_READ: c_int = 4;
pub const PROT_WRITE: c_int = 2;
pub const PROT_EXEC: c_int = 1;
pub const MAP_FILE: c_int = 1;
pub const MAP_ANON: c_int = 2;
pub const MAP_ANONYMOUS: c_int = MAP_ANON;
pub const MAP_TYPE: c_int = 15;
pub const MAP_COPY: c_int = 32;
pub const MAP_SHARED: c_int = 16;
pub const MAP_PRIVATE: c_int = 0;
pub const MAP_FIXED: c_int = 256;
pub const MAP_NOEXTEND: c_int = 512;
pub const MAP_HASSEMAPHORE: c_int = 1024;
pub const MAP_INHERIT: c_int = 2048;
pub const MAP_32BIT: c_int = 4096;
pub const MAP_EXCL: c_int = 16384;
pub const MAP_FAILED: *mut c_void = !0 as *mut c_void;
pub const MADV_NORMAL: c_int = 0;
pub const MADV_RANDOM: c_int = 1;
pub const MADV_SEQUENTIAL: c_int = 2;
pub const MADV_WILLNEED: c_int = 3;
pub const MADV_DONTNEED: c_int = 4;
pub const POSIX_MADV_NORMAL: c_int = 0;
pub const POSIX_MADV_RANDOM: c_int = 1;
pub const POSIX_MADV_SEQUENTIAL: c_int = 2;
pub const POSIX_MADV_WILLNEED: c_int = 3;
pub const POSIX_MADV_WONTNEED: c_int = 4;

pub const MS_ASYNC: c_int = 1;
pub const MS_SYNC: c_int = 0;
pub const MS_INVALIDATE: c_int = 2;
pub const MREMAP_MAYMOVE: c_int = 1;
pub const MREMAP_FIXED: c_int = 2;
pub const MCL_CURRENT: c_int = 0x0001;
pub const MCL_FUTURE: c_int = 0x0002;

// sys/xattr.h
pub const XATTR_CREATE: c_int = 0x1;
pub const XATTR_REPLACE: c_int = 0x2;

// spawn.h
pub const POSIX_SPAWN_USEVFORK: c_int = 64;
pub const POSIX_SPAWN_SETSID: c_int = 128;

// sys/syslog.h
pub const LOG_CRON: c_int = 9 << 3;
pub const LOG_AUTHPRIV: c_int = 10 << 3;
pub const LOG_FTP: c_int = 11 << 3;
pub const LOG_PERROR: c_int = 0x20;

// net/if.h
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
pub const IFF_MASTER: c_int = 0x400;
pub const IFF_SLAVE: c_int = 0x800;
pub const IFF_MULTICAST: c_int = 0x1000;
pub const IFF_PORTSEL: c_int = 0x2000;
pub const IFF_AUTOMEDIA: c_int = 0x4000;
pub const IFF_DYNAMIC: c_int = 0x8000;

// random.h
pub const GRND_NONBLOCK: c_uint = 1;
pub const GRND_RANDOM: c_uint = 2;
pub const GRND_INSECURE: c_uint = 4;

pub const _PC_LINK_MAX: c_int = 0;
pub const _PC_MAX_CANON: c_int = 1;
pub const _PC_MAX_INPUT: c_int = 2;
pub const _PC_NAME_MAX: c_int = 3;
pub const _PC_PATH_MAX: c_int = 4;
pub const _PC_PIPE_BUF: c_int = 5;
pub const _PC_CHOWN_RESTRICTED: c_int = 6;
pub const _PC_NO_TRUNC: c_int = 7;
pub const _PC_VDISABLE: c_int = 8;
pub const _PC_SYNC_IO: c_int = 9;
pub const _PC_ASYNC_IO: c_int = 10;
pub const _PC_PRIO_IO: c_int = 11;
pub const _PC_SOCK_MAXBUF: c_int = 12;
pub const _PC_FILESIZEBITS: c_int = 13;
pub const _PC_REC_INCR_XFER_SIZE: c_int = 14;
pub const _PC_REC_MAX_XFER_SIZE: c_int = 15;
pub const _PC_REC_MIN_XFER_SIZE: c_int = 16;
pub const _PC_REC_XFER_ALIGN: c_int = 17;
pub const _PC_ALLOC_SIZE_MIN: c_int = 18;
pub const _PC_SYMLINK_MAX: c_int = 19;
pub const _PC_2_SYMLINKS: c_int = 20;
pub const _SC_ARG_MAX: c_int = 0;
pub const _SC_CHILD_MAX: c_int = 1;
pub const _SC_CLK_TCK: c_int = 2;
pub const _SC_NGROUPS_MAX: c_int = 3;
pub const _SC_OPEN_MAX: c_int = 4;
pub const _SC_STREAM_MAX: c_int = 5;
pub const _SC_TZNAME_MAX: c_int = 6;
pub const _SC_JOB_CONTROL: c_int = 7;
pub const _SC_SAVED_IDS: c_int = 8;
pub const _SC_REALTIME_SIGNALS: c_int = 9;
pub const _SC_PRIORITY_SCHEDULING: c_int = 10;
pub const _SC_TIMERS: c_int = 11;
pub const _SC_ASYNCHRONOUS_IO: c_int = 12;
pub const _SC_PRIORITIZED_IO: c_int = 13;
pub const _SC_SYNCHRONIZED_IO: c_int = 14;
pub const _SC_FSYNC: c_int = 15;
pub const _SC_MAPPED_FILES: c_int = 16;
pub const _SC_MEMLOCK: c_int = 17;
pub const _SC_MEMLOCK_RANGE: c_int = 18;
pub const _SC_MEMORY_PROTECTION: c_int = 19;
pub const _SC_MESSAGE_PASSING: c_int = 20;
pub const _SC_SEMAPHORES: c_int = 21;
pub const _SC_SHARED_MEMORY_OBJECTS: c_int = 22;
pub const _SC_AIO_LISTIO_MAX: c_int = 23;
pub const _SC_AIO_MAX: c_int = 24;
pub const _SC_AIO_PRIO_DELTA_MAX: c_int = 25;
pub const _SC_DELAYTIMER_MAX: c_int = 26;
pub const _SC_MQ_OPEN_MAX: c_int = 27;
pub const _SC_MQ_PRIO_MAX: c_int = 28;
pub const _SC_VERSION: c_int = 29;
pub const _SC_PAGESIZE: c_int = 30;
pub const _SC_PAGE_SIZE: c_int = 30;
pub const _SC_RTSIG_MAX: c_int = 31;
pub const _SC_SEM_NSEMS_MAX: c_int = 32;
pub const _SC_SEM_VALUE_MAX: c_int = 33;
pub const _SC_SIGQUEUE_MAX: c_int = 34;
pub const _SC_TIMER_MAX: c_int = 35;
pub const _SC_BC_BASE_MAX: c_int = 36;
pub const _SC_BC_DIM_MAX: c_int = 37;
pub const _SC_BC_SCALE_MAX: c_int = 38;
pub const _SC_BC_STRING_MAX: c_int = 39;
pub const _SC_COLL_WEIGHTS_MAX: c_int = 40;
pub const _SC_EQUIV_CLASS_MAX: c_int = 41;
pub const _SC_EXPR_NEST_MAX: c_int = 42;
pub const _SC_LINE_MAX: c_int = 43;
pub const _SC_RE_DUP_MAX: c_int = 44;
pub const _SC_CHARCLASS_NAME_MAX: c_int = 45;
pub const _SC_2_VERSION: c_int = 46;
pub const _SC_2_C_BIND: c_int = 47;
pub const _SC_2_C_DEV: c_int = 48;
pub const _SC_2_FORT_DEV: c_int = 49;
pub const _SC_2_FORT_RUN: c_int = 50;
pub const _SC_2_SW_DEV: c_int = 51;
pub const _SC_2_LOCALEDEF: c_int = 52;
pub const _SC_PII: c_int = 53;
pub const _SC_PII_XTI: c_int = 54;
pub const _SC_PII_SOCKET: c_int = 55;
pub const _SC_PII_INTERNET: c_int = 56;
pub const _SC_PII_OSI: c_int = 57;
pub const _SC_POLL: c_int = 58;
pub const _SC_SELECT: c_int = 59;
pub const _SC_UIO_MAXIOV: c_int = 60;
pub const _SC_IOV_MAX: c_int = 60;
pub const _SC_PII_INTERNET_STREAM: c_int = 61;
pub const _SC_PII_INTERNET_DGRAM: c_int = 62;
pub const _SC_PII_OSI_COTS: c_int = 63;
pub const _SC_PII_OSI_CLTS: c_int = 64;
pub const _SC_PII_OSI_M: c_int = 65;
pub const _SC_T_IOV_MAX: c_int = 66;
pub const _SC_THREADS: c_int = 67;
pub const _SC_THREAD_SAFE_FUNCTIONS: c_int = 68;
pub const _SC_GETGR_R_SIZE_MAX: c_int = 69;
pub const _SC_GETPW_R_SIZE_MAX: c_int = 70;
pub const _SC_LOGIN_NAME_MAX: c_int = 71;
pub const _SC_TTY_NAME_MAX: c_int = 72;
pub const _SC_THREAD_DESTRUCTOR_ITERATIONS: c_int = 73;
pub const _SC_THREAD_KEYS_MAX: c_int = 74;
pub const _SC_THREAD_STACK_MIN: c_int = 75;
pub const _SC_THREAD_THREADS_MAX: c_int = 76;
pub const _SC_THREAD_ATTR_STACKADDR: c_int = 77;
pub const _SC_THREAD_ATTR_STACKSIZE: c_int = 78;
pub const _SC_THREAD_PRIORITY_SCHEDULING: c_int = 79;
pub const _SC_THREAD_PRIO_INHERIT: c_int = 80;
pub const _SC_THREAD_PRIO_PROTECT: c_int = 81;
pub const _SC_THREAD_PROCESS_SHARED: c_int = 82;
pub const _SC_NPROCESSORS_CONF: c_int = 83;
pub const _SC_NPROCESSORS_ONLN: c_int = 84;
pub const _SC_PHYS_PAGES: c_int = 85;
pub const _SC_AVPHYS_PAGES: c_int = 86;
pub const _SC_ATEXIT_MAX: c_int = 87;
pub const _SC_PASS_MAX: c_int = 88;
pub const _SC_XOPEN_VERSION: c_int = 89;
pub const _SC_XOPEN_XCU_VERSION: c_int = 90;
pub const _SC_XOPEN_UNIX: c_int = 91;
pub const _SC_XOPEN_CRYPT: c_int = 92;
pub const _SC_XOPEN_ENH_I18N: c_int = 93;
pub const _SC_XOPEN_SHM: c_int = 94;
pub const _SC_2_CHAR_TERM: c_int = 95;
pub const _SC_2_C_VERSION: c_int = 96;
pub const _SC_2_UPE: c_int = 97;
pub const _SC_XOPEN_XPG2: c_int = 98;
pub const _SC_XOPEN_XPG3: c_int = 99;
pub const _SC_XOPEN_XPG4: c_int = 100;
pub const _SC_CHAR_BIT: c_int = 101;
pub const _SC_CHAR_MAX: c_int = 102;
pub const _SC_CHAR_MIN: c_int = 103;
pub const _SC_INT_MAX: c_int = 104;
pub const _SC_INT_MIN: c_int = 105;
pub const _SC_LONG_BIT: c_int = 106;
pub const _SC_WORD_BIT: c_int = 107;
pub const _SC_MB_LEN_MAX: c_int = 108;
pub const _SC_NZERO: c_int = 109;
pub const _SC_SSIZE_MAX: c_int = 110;
pub const _SC_SCHAR_MAX: c_int = 111;
pub const _SC_SCHAR_MIN: c_int = 112;
pub const _SC_SHRT_MAX: c_int = 113;
pub const _SC_SHRT_MIN: c_int = 114;
pub const _SC_UCHAR_MAX: c_int = 115;
pub const _SC_UINT_MAX: c_int = 116;
pub const _SC_ULONG_MAX: c_int = 117;
pub const _SC_USHRT_MAX: c_int = 118;
pub const _SC_NL_ARGMAX: c_int = 119;
pub const _SC_NL_LANGMAX: c_int = 120;
pub const _SC_NL_MSGMAX: c_int = 121;
pub const _SC_NL_NMAX: c_int = 122;
pub const _SC_NL_SETMAX: c_int = 123;
pub const _SC_NL_TEXTMAX: c_int = 124;
pub const _SC_XBS5_ILP32_OFF32: c_int = 125;
pub const _SC_XBS5_ILP32_OFFBIG: c_int = 126;
pub const _SC_XBS5_LP64_OFF64: c_int = 127;
pub const _SC_XBS5_LPBIG_OFFBIG: c_int = 128;
pub const _SC_XOPEN_LEGACY: c_int = 129;
pub const _SC_XOPEN_REALTIME: c_int = 130;
pub const _SC_XOPEN_REALTIME_THREADS: c_int = 131;
pub const _SC_ADVISORY_INFO: c_int = 132;
pub const _SC_BARRIERS: c_int = 133;
pub const _SC_BASE: c_int = 134;
pub const _SC_C_LANG_SUPPORT: c_int = 135;
pub const _SC_C_LANG_SUPPORT_R: c_int = 136;
pub const _SC_CLOCK_SELECTION: c_int = 137;
pub const _SC_CPUTIME: c_int = 138;
pub const _SC_THREAD_CPUTIME: c_int = 139;
pub const _SC_DEVICE_IO: c_int = 140;
pub const _SC_DEVICE_SPECIFIC: c_int = 141;
pub const _SC_DEVICE_SPECIFIC_R: c_int = 142;
pub const _SC_FD_MGMT: c_int = 143;
pub const _SC_FIFO: c_int = 144;
pub const _SC_PIPE: c_int = 145;
pub const _SC_FILE_ATTRIBUTES: c_int = 146;
pub const _SC_FILE_LOCKING: c_int = 147;
pub const _SC_FILE_SYSTEM: c_int = 148;
pub const _SC_MONOTONIC_CLOCK: c_int = 149;
pub const _SC_MULTI_PROCESS: c_int = 150;
pub const _SC_SINGLE_PROCESS: c_int = 151;
pub const _SC_NETWORKING: c_int = 152;
pub const _SC_READER_WRITER_LOCKS: c_int = 153;
pub const _SC_SPIN_LOCKS: c_int = 154;
pub const _SC_REGEXP: c_int = 155;
pub const _SC_REGEX_VERSION: c_int = 156;
pub const _SC_SHELL: c_int = 157;
pub const _SC_SIGNALS: c_int = 158;
pub const _SC_SPAWN: c_int = 159;
pub const _SC_SPORADIC_SERVER: c_int = 160;
pub const _SC_THREAD_SPORADIC_SERVER: c_int = 161;
pub const _SC_SYSTEM_DATABASE: c_int = 162;
pub const _SC_SYSTEM_DATABASE_R: c_int = 163;
pub const _SC_TIMEOUTS: c_int = 164;
pub const _SC_TYPED_MEMORY_OBJECTS: c_int = 165;
pub const _SC_USER_GROUPS: c_int = 166;
pub const _SC_USER_GROUPS_R: c_int = 167;
pub const _SC_2_PBS: c_int = 168;
pub const _SC_2_PBS_ACCOUNTING: c_int = 169;
pub const _SC_2_PBS_LOCATE: c_int = 170;
pub const _SC_2_PBS_MESSAGE: c_int = 171;
pub const _SC_2_PBS_TRACK: c_int = 172;
pub const _SC_SYMLOOP_MAX: c_int = 173;
pub const _SC_STREAMS: c_int = 174;
pub const _SC_2_PBS_CHECKPOINT: c_int = 175;
pub const _SC_V6_ILP32_OFF32: c_int = 176;
pub const _SC_V6_ILP32_OFFBIG: c_int = 177;
pub const _SC_V6_LP64_OFF64: c_int = 178;
pub const _SC_V6_LPBIG_OFFBIG: c_int = 179;
pub const _SC_HOST_NAME_MAX: c_int = 180;
pub const _SC_TRACE: c_int = 181;
pub const _SC_TRACE_EVENT_FILTER: c_int = 182;
pub const _SC_TRACE_INHERIT: c_int = 183;
pub const _SC_TRACE_LOG: c_int = 184;
pub const _SC_LEVEL1_ICACHE_SIZE: c_int = 185;
pub const _SC_LEVEL1_ICACHE_ASSOC: c_int = 186;
pub const _SC_LEVEL1_ICACHE_LINESIZE: c_int = 187;
pub const _SC_LEVEL1_DCACHE_SIZE: c_int = 188;
pub const _SC_LEVEL1_DCACHE_ASSOC: c_int = 189;
pub const _SC_LEVEL1_DCACHE_LINESIZE: c_int = 190;
pub const _SC_LEVEL2_CACHE_SIZE: c_int = 191;
pub const _SC_LEVEL2_CACHE_ASSOC: c_int = 192;
pub const _SC_LEVEL2_CACHE_LINESIZE: c_int = 193;
pub const _SC_LEVEL3_CACHE_SIZE: c_int = 194;
pub const _SC_LEVEL3_CACHE_ASSOC: c_int = 195;
pub const _SC_LEVEL3_CACHE_LINESIZE: c_int = 196;
pub const _SC_LEVEL4_CACHE_SIZE: c_int = 197;
pub const _SC_LEVEL4_CACHE_ASSOC: c_int = 198;
pub const _SC_LEVEL4_CACHE_LINESIZE: c_int = 199;
pub const _SC_IPV6: c_int = 235;
pub const _SC_RAW_SOCKETS: c_int = 236;
pub const _SC_V7_ILP32_OFF32: c_int = 237;
pub const _SC_V7_ILP32_OFFBIG: c_int = 238;
pub const _SC_V7_LP64_OFF64: c_int = 239;
pub const _SC_V7_LPBIG_OFFBIG: c_int = 240;
pub const _SC_SS_REPL_MAX: c_int = 241;
pub const _SC_TRACE_EVENT_NAME_MAX: c_int = 242;
pub const _SC_TRACE_NAME_MAX: c_int = 243;
pub const _SC_TRACE_SYS_MAX: c_int = 244;
pub const _SC_TRACE_USER_EVENT_MAX: c_int = 245;
pub const _SC_XOPEN_STREAMS: c_int = 246;
pub const _SC_THREAD_ROBUST_PRIO_INHERIT: c_int = 247;
pub const _SC_THREAD_ROBUST_PRIO_PROTECT: c_int = 248;
pub const _SC_MINSIGSTKSZ: c_int = 249;
pub const _SC_SIGSTKSZ: c_int = 250;

pub const _CS_PATH: c_int = 0;
pub const _CS_V6_WIDTH_RESTRICTED_ENVS: c_int = 1;
pub const _CS_GNU_LIBC_VERSION: c_int = 2;
pub const _CS_GNU_LIBPTHREAD_VERSION: c_int = 3;
pub const _CS_V5_WIDTH_RESTRICTED_ENVS: c_int = 4;
pub const _CS_V7_WIDTH_RESTRICTED_ENVS: c_int = 5;
pub const _CS_LFS_CFLAGS: c_int = 1000;
pub const _CS_LFS_LDFLAGS: c_int = 1001;
pub const _CS_LFS_LIBS: c_int = 1002;
pub const _CS_LFS_LINTFLAGS: c_int = 1003;
pub const _CS_LFS64_CFLAGS: c_int = 1004;
pub const _CS_LFS64_LDFLAGS: c_int = 1005;
pub const _CS_LFS64_LIBS: c_int = 1006;
pub const _CS_LFS64_LINTFLAGS: c_int = 1007;
pub const _CS_XBS5_ILP32_OFF32_CFLAGS: c_int = 1100;
pub const _CS_XBS5_ILP32_OFF32_LDFLAGS: c_int = 1101;
pub const _CS_XBS5_ILP32_OFF32_LIBS: c_int = 1102;
pub const _CS_XBS5_ILP32_OFF32_LINTFLAGS: c_int = 1103;
pub const _CS_XBS5_ILP32_OFFBIG_CFLAGS: c_int = 1104;
pub const _CS_XBS5_ILP32_OFFBIG_LDFLAGS: c_int = 1105;
pub const _CS_XBS5_ILP32_OFFBIG_LIBS: c_int = 1106;
pub const _CS_XBS5_ILP32_OFFBIG_LINTFLAGS: c_int = 1107;
pub const _CS_XBS5_LP64_OFF64_CFLAGS: c_int = 1108;
pub const _CS_XBS5_LP64_OFF64_LDFLAGS: c_int = 1109;
pub const _CS_XBS5_LP64_OFF64_LIBS: c_int = 1110;
pub const _CS_XBS5_LP64_OFF64_LINTFLAGS: c_int = 1111;
pub const _CS_XBS5_LPBIG_OFFBIG_CFLAGS: c_int = 1112;
pub const _CS_XBS5_LPBIG_OFFBIG_LDFLAGS: c_int = 1113;
pub const _CS_XBS5_LPBIG_OFFBIG_LIBS: c_int = 1114;
pub const _CS_XBS5_LPBIG_OFFBIG_LINTFLAGS: c_int = 1115;
pub const _CS_POSIX_V6_ILP32_OFF32_CFLAGS: c_int = 1116;
pub const _CS_POSIX_V6_ILP32_OFF32_LDFLAGS: c_int = 1117;
pub const _CS_POSIX_V6_ILP32_OFF32_LIBS: c_int = 1118;
pub const _CS_POSIX_V6_ILP32_OFF32_LINTFLAGS: c_int = 1119;
pub const _CS_POSIX_V6_ILP32_OFFBIG_CFLAGS: c_int = 1120;
pub const _CS_POSIX_V6_ILP32_OFFBIG_LDFLAGS: c_int = 1121;
pub const _CS_POSIX_V6_ILP32_OFFBIG_LIBS: c_int = 1122;
pub const _CS_POSIX_V6_ILP32_OFFBIG_LINTFLAGS: c_int = 1123;
pub const _CS_POSIX_V6_LP64_OFF64_CFLAGS: c_int = 1124;
pub const _CS_POSIX_V6_LP64_OFF64_LDFLAGS: c_int = 1125;
pub const _CS_POSIX_V6_LP64_OFF64_LIBS: c_int = 1126;
pub const _CS_POSIX_V6_LP64_OFF64_LINTFLAGS: c_int = 1127;
pub const _CS_POSIX_V6_LPBIG_OFFBIG_CFLAGS: c_int = 1128;
pub const _CS_POSIX_V6_LPBIG_OFFBIG_LDFLAGS: c_int = 1129;
pub const _CS_POSIX_V6_LPBIG_OFFBIG_LIBS: c_int = 1130;
pub const _CS_POSIX_V6_LPBIG_OFFBIG_LINTFLAGS: c_int = 1131;
pub const _CS_POSIX_V7_ILP32_OFF32_CFLAGS: c_int = 1132;
pub const _CS_POSIX_V7_ILP32_OFF32_LDFLAGS: c_int = 1133;
pub const _CS_POSIX_V7_ILP32_OFF32_LIBS: c_int = 1134;
pub const _CS_POSIX_V7_ILP32_OFF32_LINTFLAGS: c_int = 1135;
pub const _CS_POSIX_V7_ILP32_OFFBIG_CFLAGS: c_int = 1136;
pub const _CS_POSIX_V7_ILP32_OFFBIG_LDFLAGS: c_int = 1137;
pub const _CS_POSIX_V7_ILP32_OFFBIG_LIBS: c_int = 1138;
pub const _CS_POSIX_V7_ILP32_OFFBIG_LINTFLAGS: c_int = 1139;
pub const _CS_POSIX_V7_LP64_OFF64_CFLAGS: c_int = 1140;
pub const _CS_POSIX_V7_LP64_OFF64_LDFLAGS: c_int = 1141;
pub const _CS_POSIX_V7_LP64_OFF64_LIBS: c_int = 1142;
pub const _CS_POSIX_V7_LP64_OFF64_LINTFLAGS: c_int = 1143;
pub const _CS_POSIX_V7_LPBIG_OFFBIG_CFLAGS: c_int = 1144;
pub const _CS_POSIX_V7_LPBIG_OFFBIG_LDFLAGS: c_int = 1145;
pub const _CS_POSIX_V7_LPBIG_OFFBIG_LIBS: c_int = 1146;
pub const _CS_POSIX_V7_LPBIG_OFFBIG_LINTFLAGS: c_int = 1147;
pub const _CS_V6_ENV: c_int = 1148;
pub const _CS_V7_ENV: c_int = 1149;

pub const PTHREAD_PROCESS_PRIVATE: __pthread_process_shared = 0;
pub const PTHREAD_PROCESS_SHARED: __pthread_process_shared = 1;

pub const PTHREAD_EXPLICIT_SCHED: __pthread_inheritsched = 0;
pub const PTHREAD_INHERIT_SCHED: __pthread_inheritsched = 1;

pub const PTHREAD_SCOPE_SYSTEM: __pthread_contentionscope = 0;
pub const PTHREAD_SCOPE_PROCESS: __pthread_contentionscope = 1;

pub const PTHREAD_CREATE_JOINABLE: __pthread_detachstate = 0;
pub const PTHREAD_CREATE_DETACHED: __pthread_detachstate = 1;

pub const PTHREAD_PRIO_NONE: __pthread_mutex_protocol = 0;
pub const PTHREAD_PRIO_INHERIT: __pthread_mutex_protocol = 1;
pub const PTHREAD_PRIO_PROTECT: __pthread_mutex_protocol = 2;

pub const PTHREAD_MUTEX_TIMED: __pthread_mutex_type = 0;
pub const PTHREAD_MUTEX_ERRORCHECK: __pthread_mutex_type = 1;
pub const PTHREAD_MUTEX_RECURSIVE: __pthread_mutex_type = 2;

pub const PTHREAD_MUTEX_STALLED: __pthread_mutex_robustness = 0;
pub const PTHREAD_MUTEX_ROBUST: __pthread_mutex_robustness = 256;

pub const RLIMIT_CPU: crate::__rlimit_resource_t = 0;
pub const RLIMIT_FSIZE: crate::__rlimit_resource_t = 1;
pub const RLIMIT_DATA: crate::__rlimit_resource_t = 2;
pub const RLIMIT_STACK: crate::__rlimit_resource_t = 3;
pub const RLIMIT_CORE: crate::__rlimit_resource_t = 4;
pub const RLIMIT_RSS: crate::__rlimit_resource_t = 5;
pub const RLIMIT_MEMLOCK: crate::__rlimit_resource_t = 6;
pub const RLIMIT_NPROC: crate::__rlimit_resource_t = 7;
pub const RLIMIT_OFILE: crate::__rlimit_resource_t = 8;
pub const RLIMIT_NOFILE: crate::__rlimit_resource_t = 8;
pub const RLIMIT_SBSIZE: crate::__rlimit_resource_t = 9;
pub const RLIMIT_AS: crate::__rlimit_resource_t = 10;
pub const RLIMIT_VMEM: crate::__rlimit_resource_t = 10;
pub const RLIMIT_NLIMITS: crate::__rlimit_resource_t = 11;
pub const RLIM_NLIMITS: crate::__rlimit_resource_t = 11;

pub const RUSAGE_SELF: __rusage_who = 0;
pub const RUSAGE_CHILDREN: __rusage_who = -1;

pub const PRIO_PROCESS: __priority_which = 0;
pub const PRIO_PGRP: __priority_which = 1;
pub const PRIO_USER: __priority_which = 2;

pub const __UT_LINESIZE: usize = 32;
pub const __UT_NAMESIZE: usize = 32;
pub const __UT_HOSTSIZE: usize = 256;

pub const SOCK_STREAM: c_int = 1;
pub const SOCK_DGRAM: c_int = 2;
pub const SOCK_RAW: c_int = 3;
pub const SOCK_RDM: c_int = 4;
pub const SOCK_SEQPACKET: c_int = 5;
pub const SOCK_CLOEXEC: c_int = 4194304;
pub const SOCK_NONBLOCK: c_int = 2048;

pub const MSG_OOB: c_int = 1;
pub const MSG_PEEK: c_int = 2;
pub const MSG_DONTROUTE: c_int = 4;
pub const MSG_EOR: c_int = 8;
pub const MSG_TRUNC: c_int = 16;
pub const MSG_CTRUNC: c_int = 32;
pub const MSG_WAITALL: c_int = 64;
pub const MSG_DONTWAIT: c_int = 128;
pub const MSG_NOSIGNAL: c_int = 1024;
pub const MSG_CMSG_CLOEXEC: c_int = 0x40000000;

pub const SCM_RIGHTS: c_int = 1;
pub const SCM_TIMESTAMP: c_int = 2;
pub const SCM_CREDS: c_int = 3;

pub const SO_DEBUG: c_int = 1;
pub const SO_ACCEPTCONN: c_int = 2;
pub const SO_REUSEADDR: c_int = 4;
pub const SO_KEEPALIVE: c_int = 8;
pub const SO_DONTROUTE: c_int = 16;
pub const SO_BROADCAST: c_int = 32;
pub const SO_USELOOPBACK: c_int = 64;
pub const SO_LINGER: c_int = 128;
pub const SO_OOBINLINE: c_int = 256;
pub const SO_REUSEPORT: c_int = 512;
pub const SO_SNDBUF: c_int = 4097;
pub const SO_RCVBUF: c_int = 4098;
pub const SO_SNDLOWAT: c_int = 4099;
pub const SO_RCVLOWAT: c_int = 4100;
pub const SO_SNDTIMEO: c_int = 4101;
pub const SO_RCVTIMEO: c_int = 4102;
pub const SO_ERROR: c_int = 4103;
pub const SO_STYLE: c_int = 4104;
pub const SO_TYPE: c_int = 4104;

pub const IPPROTO_IP: c_int = 0;
pub const IPPROTO_ICMP: c_int = 1;
pub const IPPROTO_IGMP: c_int = 2;
pub const IPPROTO_IPIP: c_int = 4;
pub const IPPROTO_TCP: c_int = 6;
pub const IPPROTO_EGP: c_int = 8;
pub const IPPROTO_PUP: c_int = 12;
pub const IPPROTO_UDP: c_int = 17;
pub const IPPROTO_IDP: c_int = 22;
pub const IPPROTO_TP: c_int = 29;
pub const IPPROTO_DCCP: c_int = 33;
pub const IPPROTO_IPV6: c_int = 41;
pub const IPPROTO_RSVP: c_int = 46;
pub const IPPROTO_GRE: c_int = 47;
pub const IPPROTO_ESP: c_int = 50;
pub const IPPROTO_AH: c_int = 51;
pub const IPPROTO_MTP: c_int = 92;
pub const IPPROTO_BEETPH: c_int = 94;
pub const IPPROTO_ENCAP: c_int = 98;
pub const IPPROTO_PIM: c_int = 103;
pub const IPPROTO_COMP: c_int = 108;
pub const IPPROTO_L2TP: c_int = 115;
pub const IPPROTO_SCTP: c_int = 132;
pub const IPPROTO_UDPLITE: c_int = 136;
pub const IPPROTO_MPLS: c_int = 137;
pub const IPPROTO_ETHERNET: c_int = 143;
pub const IPPROTO_RAW: c_int = 255;
pub const IPPROTO_MPTCP: c_int = 262;
pub const IPPROTO_MAX: c_int = 263;

pub const IPPROTO_HOPOPTS: c_int = 0;
pub const IPPROTO_ROUTING: c_int = 43;
pub const IPPROTO_FRAGMENT: c_int = 44;
pub const IPPROTO_ICMPV6: c_int = 58;
pub const IPPROTO_NONE: c_int = 59;
pub const IPPROTO_DSTOPTS: c_int = 60;
pub const IPPROTO_MH: c_int = 135;

pub const IPPORT_ECHO: in_port_t = 7;
pub const IPPORT_DISCARD: in_port_t = 9;
pub const IPPORT_SYSTAT: in_port_t = 11;
pub const IPPORT_DAYTIME: in_port_t = 13;
pub const IPPORT_NETSTAT: in_port_t = 15;
pub const IPPORT_FTP: in_port_t = 21;
pub const IPPORT_TELNET: in_port_t = 23;
pub const IPPORT_SMTP: in_port_t = 25;
pub const IPPORT_TIMESERVER: in_port_t = 37;
pub const IPPORT_NAMESERVER: in_port_t = 42;
pub const IPPORT_WHOIS: in_port_t = 43;
pub const IPPORT_MTP: in_port_t = 57;
pub const IPPORT_TFTP: in_port_t = 69;
pub const IPPORT_RJE: in_port_t = 77;
pub const IPPORT_FINGER: in_port_t = 79;
pub const IPPORT_TTYLINK: in_port_t = 87;
pub const IPPORT_SUPDUP: in_port_t = 95;
pub const IPPORT_EXECSERVER: in_port_t = 512;
pub const IPPORT_LOGINSERVER: in_port_t = 513;
pub const IPPORT_CMDSERVER: in_port_t = 514;
pub const IPPORT_EFSSERVER: in_port_t = 520;
pub const IPPORT_BIFFUDP: in_port_t = 512;
pub const IPPORT_WHOSERVER: in_port_t = 513;
pub const IPPORT_ROUTESERVER: in_port_t = 520;
pub const IPPORT_USERRESERVED: in_port_t = 5000;

pub const DT_UNKNOWN: c_uchar = 0;
pub const DT_FIFO: c_uchar = 1;
pub const DT_CHR: c_uchar = 2;
pub const DT_DIR: c_uchar = 4;
pub const DT_BLK: c_uchar = 6;
pub const DT_REG: c_uchar = 8;
pub const DT_LNK: c_uchar = 10;
pub const DT_SOCK: c_uchar = 12;
pub const DT_WHT: c_uchar = 14;

pub const ST_RDONLY: c_ulong = 1;
pub const ST_NOSUID: c_ulong = 2;
pub const ST_NOEXEC: c_ulong = 8;
pub const ST_SYNCHRONOUS: c_ulong = 16;
pub const ST_NOATIME: c_ulong = 32;
pub const ST_RELATIME: c_ulong = 64;

pub const RTLD_DI_LMID: c_int = 1;
pub const RTLD_DI_LINKMAP: c_int = 2;
pub const RTLD_DI_CONFIGADDR: c_int = 3;
pub const RTLD_DI_SERINFO: c_int = 4;
pub const RTLD_DI_SERINFOSIZE: c_int = 5;
pub const RTLD_DI_ORIGIN: c_int = 6;
pub const RTLD_DI_PROFILENAME: c_int = 7;
pub const RTLD_DI_PROFILEOUT: c_int = 8;
pub const RTLD_DI_TLS_MODID: c_int = 9;
pub const RTLD_DI_TLS_DATA: c_int = 10;
pub const RTLD_DI_PHDR: c_int = 11;
pub const RTLD_DI_MAX: c_int = 11;

pub const SI_ASYNCIO: c_int = -4;
pub const SI_MESGQ: c_int = -3;
pub const SI_TIMER: c_int = -2;
pub const SI_QUEUE: c_int = -1;
pub const SI_USER: c_int = 0;

pub const ILL_ILLOPC: c_int = 1;
pub const ILL_ILLOPN: c_int = 2;
pub const ILL_ILLADR: c_int = 3;
pub const ILL_ILLTRP: c_int = 4;
pub const ILL_PRVOPC: c_int = 5;
pub const ILL_PRVREG: c_int = 6;
pub const ILL_COPROC: c_int = 7;
pub const ILL_BADSTK: c_int = 8;

pub const FPE_INTDIV: c_int = 1;
pub const FPE_INTOVF: c_int = 2;
pub const FPE_FLTDIV: c_int = 3;
pub const FPE_FLTOVF: c_int = 4;
pub const FPE_FLTUND: c_int = 5;
pub const FPE_FLTRES: c_int = 6;
pub const FPE_FLTINV: c_int = 7;
pub const FPE_FLTSUB: c_int = 8;

pub const SEGV_MAPERR: c_int = 1;
pub const SEGV_ACCERR: c_int = 2;

pub const BUS_ADRALN: c_int = 1;
pub const BUS_ADRERR: c_int = 2;
pub const BUS_OBJERR: c_int = 3;

pub const TRAP_BRKPT: c_int = 1;
pub const TRAP_TRACE: c_int = 2;

pub const CLD_EXITED: c_int = 1;
pub const CLD_KILLED: c_int = 2;
pub const CLD_DUMPED: c_int = 3;
pub const CLD_TRAPPED: c_int = 4;
pub const CLD_STOPPED: c_int = 5;
pub const CLD_CONTINUED: c_int = 6;

pub const POLL_IN: c_int = 1;
pub const POLL_OUT: c_int = 2;
pub const POLL_MSG: c_int = 3;
pub const POLL_ERR: c_int = 4;
pub const POLL_PRI: c_int = 5;
pub const POLL_HUP: c_int = 6;

pub const SIGEV_SIGNAL: c_int = 0;
pub const SIGEV_NONE: c_int = 1;
pub const SIGEV_THREAD: c_int = 2;

pub const REG_GS: c_uint = 0;
pub const REG_FS: c_uint = 1;
pub const REG_ES: c_uint = 2;
pub const REG_DS: c_uint = 3;
pub const REG_EDI: c_uint = 4;
pub const REG_ESI: c_uint = 5;
pub const REG_EBP: c_uint = 6;
pub const REG_ESP: c_uint = 7;
pub const REG_EBX: c_uint = 8;
pub const REG_EDX: c_uint = 9;
pub const REG_ECX: c_uint = 10;
pub const REG_EAX: c_uint = 11;
pub const REG_TRAPNO: c_uint = 12;
pub const REG_ERR: c_uint = 13;
pub const REG_EIP: c_uint = 14;
pub const REG_CS: c_uint = 15;
pub const REG_EFL: c_uint = 16;
pub const REG_UESP: c_uint = 17;
pub const REG_SS: c_uint = 18;

pub const IOC_VOID: __ioctl_dir = 0;
pub const IOC_OUT: __ioctl_dir = 1;
pub const IOC_IN: __ioctl_dir = 2;
pub const IOC_INOUT: __ioctl_dir = 3;

pub const IOC_8: __ioctl_datum = 0;
pub const IOC_16: __ioctl_datum = 1;
pub const IOC_32: __ioctl_datum = 2;
pub const IOC_64: __ioctl_datum = 3;

pub const TCP_ESTABLISHED: c_uint = 1;
pub const TCP_SYN_SENT: c_uint = 2;
pub const TCP_SYN_RECV: c_uint = 3;
pub const TCP_FIN_WAIT1: c_uint = 4;
pub const TCP_FIN_WAIT2: c_uint = 5;
pub const TCP_TIME_WAIT: c_uint = 6;
pub const TCP_CLOSE: c_uint = 7;
pub const TCP_CLOSE_WAIT: c_uint = 8;
pub const TCP_LAST_ACK: c_uint = 9;
pub const TCP_LISTEN: c_uint = 10;
pub const TCP_CLOSING: c_uint = 11;

pub const TCP_CA_Open: tcp_ca_state = 0;
pub const TCP_CA_Disorder: tcp_ca_state = 1;
pub const TCP_CA_CWR: tcp_ca_state = 2;
pub const TCP_CA_Recovery: tcp_ca_state = 3;
pub const TCP_CA_Loss: tcp_ca_state = 4;

pub const TCP_NO_QUEUE: c_uint = 0;
pub const TCP_RECV_QUEUE: c_uint = 1;
pub const TCP_SEND_QUEUE: c_uint = 2;
pub const TCP_QUEUES_NR: c_uint = 3;

pub const P_ALL: idtype_t = 0;
pub const P_PID: idtype_t = 1;
pub const P_PGID: idtype_t = 2;

pub const SS_ONSTACK: c_int = 1;
pub const SS_DISABLE: c_int = 4;

pub const SHUT_RD: c_int = 0;
pub const SHUT_WR: c_int = 1;
pub const SHUT_RDWR: c_int = 2;
pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
    __lock: 0,
    __owner_id: 0,
    __cnt: 0,
    __shpid: 0,
    __type: PTHREAD_MUTEX_TIMED as c_int,
    __flags: 0,
    __reserved1: 0,
    __reserved2: 0,
};
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    __lock: __PTHREAD_SPIN_LOCK_INITIALIZER,
    __queue: 0i64 as *mut __pthread,
    __attr: 0i64 as *mut __pthread_condattr,
    __wrefs: 0,
    __data: 0i64 as *mut c_void,
};
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = pthread_rwlock_t {
    __held: __PTHREAD_SPIN_LOCK_INITIALIZER,
    __lock: __PTHREAD_SPIN_LOCK_INITIALIZER,
    __readers: 0,
    __readerqueue: 0i64 as *mut __pthread,
    __writerqueue: 0i64 as *mut __pthread,
    __attr: 0i64 as *mut __pthread_rwlockattr,
    __data: 0i64 as *mut c_void,
};
pub const PTHREAD_STACK_MIN: size_t = 0;

// Non-public helper constants
const _UTSNAME_LENGTH: usize = 1024;

const_fn! {
    {const} fn CMSG_ALIGN(len: usize) -> usize {
        (len + size_of::<usize>() - 1) & !(size_of::<usize>() - 1)
    }
}

// functions
f! {
    pub fn CMSG_FIRSTHDR(mhdr: *const msghdr) -> *mut cmsghdr {
        if (*mhdr).msg_controllen as usize >= size_of::<cmsghdr>() {
            (*mhdr).msg_control.cast::<cmsghdr>()
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
        if ((*cmsg).cmsg_len as usize) < size_of::<cmsghdr>() {
            return core::ptr::null_mut::<cmsghdr>();
        }
        let next = (cmsg as usize + CMSG_ALIGN((*cmsg).cmsg_len as usize)) as *mut cmsghdr;
        let max = (*mhdr).msg_control as usize + (*mhdr).msg_controllen as usize;
        if (next.offset(1)) as usize > max
            || next as usize + CMSG_ALIGN((*next).cmsg_len as usize) > max
        {
            core::ptr::null_mut::<cmsghdr>()
        } else {
            next.cast::<cmsghdr>()
        }
    }

    pub fn CPU_ALLOC_SIZE(count: c_int) -> size_t {
        let _dummy: cpu_set_t = mem::zeroed();
        let size_in_bits = 8 * size_of_val(&_dummy.bits[0]);
        ((count as size_t + size_in_bits - 1) / 8) as size_t
    }

    pub fn CPU_ZERO(cpuset: &mut cpu_set_t) -> () {
        for slot in cpuset.bits.iter_mut() {
            *slot = 0;
        }
    }

    pub fn CPU_SET(cpu: usize, cpuset: &mut cpu_set_t) -> () {
        let size_in_bits = 8 * size_of_val(&cpuset.bits[0]); // 32, 64 etc
        let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
        cpuset.bits[idx] |= 1 << offset;
    }

    pub fn CPU_CLR(cpu: usize, cpuset: &mut cpu_set_t) -> () {
        let size_in_bits = 8 * size_of_val(&cpuset.bits[0]); // 32, 64 etc
        let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
        cpuset.bits[idx] &= !(1 << offset);
    }

    pub fn CPU_ISSET(cpu: usize, cpuset: &cpu_set_t) -> bool {
        let size_in_bits = 8 * size_of_val(&cpuset.bits[0]);
        let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
        0 != (cpuset.bits[idx] & (1 << offset))
    }

    pub fn CPU_COUNT_S(size: usize, cpuset: &cpu_set_t) -> c_int {
        let mut s: u32 = 0;
        let size_of_mask = size_of_val(&cpuset.bits[0]);
        for i in cpuset.bits[..(size / size_of_mask)].iter() {
            s += i.count_ones();
        }
        s as c_int
    }

    pub fn CPU_COUNT(cpuset: &cpu_set_t) -> c_int {
        CPU_COUNT_S(size_of::<cpu_set_t>(), cpuset)
    }

    pub fn CPU_EQUAL(set1: &cpu_set_t, set2: &cpu_set_t) -> bool {
        set1.bits == set2.bits
    }

    pub fn IPTOS_TOS(tos: u8) -> u8 {
        tos & IPTOS_TOS_MASK
    }

    pub fn IPTOS_PREC(tos: u8) -> u8 {
        tos & IPTOS_PREC_MASK
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

extern "C" {
    pub fn lutimes(file: *const c_char, times: *const crate::timeval) -> c_int;

    pub fn futimes(fd: c_int, times: *const crate::timeval) -> c_int;
    pub fn futimens(__fd: c_int, __times: *const crate::timespec) -> c_int;

    pub fn utimensat(
        dirfd: c_int,
        path: *const c_char,
        times: *const crate::timespec,
        flag: c_int,
    ) -> c_int;

    pub fn mkfifoat(__fd: c_int, __path: *const c_char, __mode: __mode_t) -> c_int;

    pub fn mknodat(dirfd: c_int, pathname: *const c_char, mode: mode_t, dev: dev_t) -> c_int;

    pub fn __libc_current_sigrtmin() -> c_int;

    pub fn __libc_current_sigrtmax() -> c_int;

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

    pub fn sigwait(__set: *const sigset_t, __sig: *mut c_int) -> c_int;

    pub fn sigsuspend(mask: *const crate::sigset_t) -> c_int;
    pub fn sigtimedwait(
        set: *const sigset_t,
        info: *mut siginfo_t,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn sigwaitinfo(set: *const sigset_t, info: *mut siginfo_t) -> c_int;

    pub fn sigaltstack(ss: *const stack_t, oss: *mut stack_t) -> c_int;

    pub fn ioctl(__fd: c_int, __request: c_ulong, ...) -> c_int;

    pub fn pipe2(fds: *mut c_int, flags: c_int) -> c_int;

    pub fn dup3(oldfd: c_int, newfd: c_int, flags: c_int) -> c_int;

    pub fn pread64(fd: c_int, buf: *mut c_void, count: size_t, offset: off64_t) -> ssize_t;
    pub fn pwrite64(fd: c_int, buf: *const c_void, count: size_t, offset: off64_t) -> ssize_t;

    pub fn readv(__fd: c_int, __iovec: *const crate::iovec, __count: c_int) -> ssize_t;
    pub fn writev(__fd: c_int, __iovec: *const crate::iovec, __count: c_int) -> ssize_t;

    pub fn preadv(
        __fd: c_int,
        __iovec: *const crate::iovec,
        __count: c_int,
        __offset: __off_t,
    ) -> ssize_t;
    pub fn pwritev(
        __fd: c_int,
        __iovec: *const crate::iovec,
        __count: c_int,
        __offset: __off_t,
    ) -> ssize_t;

    pub fn preadv64(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off64_t)
        -> ssize_t;
    pub fn pwritev64(
        fd: c_int,
        iov: *const crate::iovec,
        iovcnt: c_int,
        offset: off64_t,
    ) -> ssize_t;

    pub fn fread_unlocked(
        buf: *mut c_void,
        size: size_t,
        nobj: size_t,
        stream: *mut crate::FILE,
    ) -> size_t;

    pub fn aio_read(aiocbp: *mut aiocb) -> c_int;
    pub fn aio_write(aiocbp: *mut aiocb) -> c_int;
    pub fn aio_fsync(op: c_int, aiocbp: *mut aiocb) -> c_int;
    pub fn aio_error(aiocbp: *const aiocb) -> c_int;
    pub fn aio_return(aiocbp: *mut aiocb) -> ssize_t;
    pub fn aio_suspend(
        aiocb_list: *const *const aiocb,
        nitems: c_int,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn aio_cancel(fd: c_int, aiocbp: *mut aiocb) -> c_int;
    pub fn lio_listio(
        mode: c_int,
        aiocb_list: *const *mut aiocb,
        nitems: c_int,
        sevp: *mut crate::sigevent,
    ) -> c_int;

    pub fn mq_open(name: *const c_char, oflag: c_int, ...) -> crate::mqd_t;
    pub fn mq_close(mqd: crate::mqd_t) -> c_int;
    pub fn mq_unlink(name: *const c_char) -> c_int;
    pub fn mq_receive(
        mqd: crate::mqd_t,
        msg_ptr: *mut c_char,
        msg_len: size_t,
        msg_prio: *mut c_uint,
    ) -> ssize_t;
    pub fn mq_timedreceive(
        mqd: crate::mqd_t,
        msg_ptr: *mut c_char,
        msg_len: size_t,
        msg_prio: *mut c_uint,
        abs_timeout: *const crate::timespec,
    ) -> ssize_t;
    pub fn mq_send(
        mqd: crate::mqd_t,
        msg_ptr: *const c_char,
        msg_len: size_t,
        msg_prio: c_uint,
    ) -> c_int;
    pub fn mq_timedsend(
        mqd: crate::mqd_t,
        msg_ptr: *const c_char,
        msg_len: size_t,
        msg_prio: c_uint,
        abs_timeout: *const crate::timespec,
    ) -> c_int;
    pub fn mq_getattr(mqd: crate::mqd_t, attr: *mut crate::mq_attr) -> c_int;
    pub fn mq_setattr(
        mqd: crate::mqd_t,
        newattr: *const crate::mq_attr,
        oldattr: *mut crate::mq_attr,
    ) -> c_int;

    pub fn lseek64(__fd: c_int, __offset: __off64_t, __whence: c_int) -> __off64_t;

    pub fn lseek(__fd: c_int, __offset: __off_t, __whence: c_int) -> __off_t;

    pub fn fgetpos64(stream: *mut crate::FILE, ptr: *mut fpos64_t) -> c_int;
    pub fn fseeko64(stream: *mut crate::FILE, offset: off64_t, whence: c_int) -> c_int;
    pub fn fsetpos64(stream: *mut crate::FILE, ptr: *const fpos64_t) -> c_int;
    pub fn ftello64(stream: *mut crate::FILE) -> off64_t;

    pub fn bind(__fd: c_int, __addr: *const sockaddr, __len: crate::socklen_t) -> c_int;

    pub fn accept4(
        fd: c_int,
        addr: *mut crate::sockaddr,
        len: *mut crate::socklen_t,
        flg: c_int,
    ) -> c_int;

    pub fn ppoll(
        fds: *mut crate::pollfd,
        nfds: nfds_t,
        timeout: *const crate::timespec,
        sigmask: *const sigset_t,
    ) -> c_int;

    pub fn recvmsg(__fd: c_int, __message: *mut msghdr, __flags: c_int) -> ssize_t;

    pub fn sendmsg(__fd: c_int, __message: *const msghdr, __flags: c_int) -> ssize_t;

    pub fn recvfrom(
        socket: c_int,
        buf: *mut c_void,
        len: size_t,
        flags: c_int,
        addr: *mut crate::sockaddr,
        addrlen: *mut crate::socklen_t,
    ) -> ssize_t;

    pub fn sendfile(out_fd: c_int, in_fd: c_int, offset: *mut off_t, count: size_t) -> ssize_t;
    pub fn sendfile64(out_fd: c_int, in_fd: c_int, offset: *mut off64_t, count: size_t) -> ssize_t;

    pub fn shutdown(__fd: c_int, __how: c_int) -> c_int;

    pub fn sethostname(name: *const c_char, len: size_t) -> c_int;
    pub fn getdomainname(name: *mut c_char, len: size_t) -> c_int;
    pub fn setdomainname(name: *const c_char, len: size_t) -> c_int;
    pub fn if_nameindex() -> *mut if_nameindex;
    pub fn if_freenameindex(ptr: *mut if_nameindex);

    pub fn getnameinfo(
        sa: *const crate::sockaddr,
        salen: crate::socklen_t,
        host: *mut c_char,
        hostlen: crate::socklen_t,
        serv: *mut c_char,
        servlen: crate::socklen_t,
        flags: c_int,
    ) -> c_int;

    pub fn getifaddrs(ifap: *mut *mut crate::ifaddrs) -> c_int;
    pub fn freeifaddrs(ifa: *mut crate::ifaddrs);

    pub fn uname(buf: *mut crate::utsname) -> c_int;

    pub fn gethostid() -> c_long;
    pub fn sethostid(hostid: c_long) -> c_int;

    pub fn setpwent();
    pub fn endpwent();
    pub fn getpwent() -> *mut passwd;
    pub fn setgrent();
    pub fn endgrent();
    pub fn getgrent() -> *mut crate::group;
    pub fn setspent();
    pub fn endspent();
    pub fn getspent() -> *mut spwd;

    pub fn getspnam(name: *const c_char) -> *mut spwd;

    pub fn getpwent_r(
        pwd: *mut crate::passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::passwd,
    ) -> c_int;
    pub fn getgrent_r(
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn fgetpwent_r(
        stream: *mut crate::FILE,
        pwd: *mut crate::passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::passwd,
    ) -> c_int;
    pub fn fgetgrent_r(
        stream: *mut crate::FILE,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;

    pub fn putpwent(p: *const crate::passwd, stream: *mut crate::FILE) -> c_int;
    pub fn putgrent(grp: *const crate::group, stream: *mut crate::FILE) -> c_int;

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

    pub fn fgetspent_r(
        fp: *mut crate::FILE,
        spbuf: *mut crate::spwd,
        buf: *mut c_char,
        buflen: size_t,
        spbufp: *mut *mut crate::spwd,
    ) -> c_int;
    pub fn sgetspent_r(
        s: *const c_char,
        spbuf: *mut crate::spwd,
        buf: *mut c_char,
        buflen: size_t,
        spbufp: *mut *mut crate::spwd,
    ) -> c_int;
    pub fn getspent_r(
        spbuf: *mut crate::spwd,
        buf: *mut c_char,
        buflen: size_t,
        spbufp: *mut *mut crate::spwd,
    ) -> c_int;

    pub fn getspnam_r(
        name: *const c_char,
        spbuf: *mut spwd,
        buf: *mut c_char,
        buflen: size_t,
        spbufp: *mut *mut spwd,
    ) -> c_int;

    // mntent.h
    pub fn getmntent_r(
        stream: *mut crate::FILE,
        mntbuf: *mut crate::mntent,
        buf: *mut c_char,
        buflen: c_int,
    ) -> *mut crate::mntent;

    pub fn utmpname(file: *const c_char) -> c_int;
    pub fn utmpxname(file: *const c_char) -> c_int;
    pub fn getutxent() -> *mut utmpx;
    pub fn getutxid(ut: *const utmpx) -> *mut utmpx;
    pub fn getutxline(ut: *const utmpx) -> *mut utmpx;
    pub fn pututxline(ut: *const utmpx) -> *mut utmpx;
    pub fn setutxent();
    pub fn endutxent();

    pub fn getresuid(
        ruid: *mut crate::uid_t,
        euid: *mut crate::uid_t,
        suid: *mut crate::uid_t,
    ) -> c_int;
    pub fn getresgid(
        rgid: *mut crate::gid_t,
        egid: *mut crate::gid_t,
        sgid: *mut crate::gid_t,
    ) -> c_int;
    pub fn setresuid(ruid: crate::uid_t, euid: crate::uid_t, suid: crate::uid_t) -> c_int;
    pub fn setresgid(rgid: crate::gid_t, egid: crate::gid_t, sgid: crate::gid_t) -> c_int;

    pub fn initgroups(user: *const c_char, group: crate::gid_t) -> c_int;

    pub fn getgrgid(gid: crate::gid_t) -> *mut crate::group;
    pub fn getgrgid_r(
        gid: crate::gid_t,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;

    pub fn getgrnam(name: *const c_char) -> *mut crate::group;
    pub fn getgrnam_r(
        name: *const c_char,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;

    pub fn getgrouplist(
        user: *const c_char,
        group: crate::gid_t,
        groups: *mut crate::gid_t,
        ngroups: *mut c_int,
    ) -> c_int;

    pub fn setgroups(ngroups: size_t, ptr: *const crate::gid_t) -> c_int;

    pub fn acct(filename: *const c_char) -> c_int;

    pub fn setmntent(filename: *const c_char, ty: *const c_char) -> *mut crate::FILE;
    pub fn getmntent(stream: *mut crate::FILE) -> *mut crate::mntent;
    pub fn addmntent(stream: *mut crate::FILE, mnt: *const crate::mntent) -> c_int;
    pub fn endmntent(streamp: *mut crate::FILE) -> c_int;
    pub fn hasmntopt(mnt: *const crate::mntent, opt: *const c_char) -> *mut c_char;

    pub fn pthread_create(
        native: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        f: extern "C" fn(*mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;
    pub fn pthread_kill(__threadid: crate::pthread_t, __signo: c_int) -> c_int;
    pub fn pthread_cancel(thread: crate::pthread_t) -> c_int;
    pub fn __pthread_equal(__t1: __pthread_t, __t2: __pthread_t) -> c_int;

    pub fn pthread_getattr_np(__thr: crate::pthread_t, __attr: *mut pthread_attr_t) -> c_int;

    pub fn pthread_attr_getguardsize(
        __attr: *const pthread_attr_t,
        __guardsize: *mut size_t,
    ) -> c_int;
    pub fn pthread_attr_setguardsize(attr: *mut crate::pthread_attr_t, guardsize: size_t) -> c_int;

    pub fn pthread_attr_getstack(
        __attr: *const pthread_attr_t,
        __stackaddr: *mut *mut c_void,
        __stacksize: *mut size_t,
    ) -> c_int;

    pub fn pthread_mutexattr_getpshared(
        attr: *const pthread_mutexattr_t,
        pshared: *mut c_int,
    ) -> c_int;
    pub fn pthread_mutexattr_setpshared(attr: *mut pthread_mutexattr_t, pshared: c_int) -> c_int;

    pub fn pthread_mutex_timedlock(
        lock: *mut pthread_mutex_t,
        abstime: *const crate::timespec,
    ) -> c_int;

    pub fn pthread_rwlockattr_getpshared(
        attr: *const pthread_rwlockattr_t,
        val: *mut c_int,
    ) -> c_int;
    pub fn pthread_rwlockattr_setpshared(attr: *mut pthread_rwlockattr_t, val: c_int) -> c_int;

    pub fn pthread_condattr_getclock(
        attr: *const pthread_condattr_t,
        clock_id: *mut clockid_t,
    ) -> c_int;
    pub fn pthread_condattr_setclock(
        __attr: *mut pthread_condattr_t,
        __clock_id: __clockid_t,
    ) -> c_int;
    pub fn pthread_condattr_getpshared(
        attr: *const pthread_condattr_t,
        pshared: *mut c_int,
    ) -> c_int;
    pub fn pthread_condattr_setpshared(attr: *mut pthread_condattr_t, pshared: c_int) -> c_int;

    pub fn pthread_once(control: *mut pthread_once_t, routine: extern "C" fn()) -> c_int;

    pub fn pthread_barrierattr_init(attr: *mut crate::pthread_barrierattr_t) -> c_int;
    pub fn pthread_barrierattr_destroy(attr: *mut crate::pthread_barrierattr_t) -> c_int;
    pub fn pthread_barrierattr_getpshared(
        attr: *const crate::pthread_barrierattr_t,
        shared: *mut c_int,
    ) -> c_int;
    pub fn pthread_barrierattr_setpshared(
        attr: *mut crate::pthread_barrierattr_t,
        shared: c_int,
    ) -> c_int;
    pub fn pthread_barrier_init(
        barrier: *mut pthread_barrier_t,
        attr: *const crate::pthread_barrierattr_t,
        count: c_uint,
    ) -> c_int;
    pub fn pthread_barrier_destroy(barrier: *mut pthread_barrier_t) -> c_int;
    pub fn pthread_barrier_wait(barrier: *mut pthread_barrier_t) -> c_int;
    pub fn pthread_spin_init(lock: *mut crate::pthread_spinlock_t, pshared: c_int) -> c_int;
    pub fn pthread_spin_destroy(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_lock(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_trylock(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_unlock(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn pthread_atfork(
        prepare: Option<unsafe extern "C" fn()>,
        parent: Option<unsafe extern "C" fn()>,
        child: Option<unsafe extern "C" fn()>,
    ) -> c_int;

    pub fn pthread_sigmask(
        __how: c_int,
        __newmask: *const __sigset_t,
        __oldmask: *mut __sigset_t,
    ) -> c_int;

    pub fn sched_getparam(pid: crate::pid_t, param: *mut crate::sched_param) -> c_int;
    pub fn sched_setparam(pid: crate::pid_t, param: *const crate::sched_param) -> c_int;
    pub fn sched_getscheduler(pid: crate::pid_t) -> c_int;
    pub fn sched_setscheduler(
        pid: crate::pid_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;
    pub fn pthread_getschedparam(
        native: crate::pthread_t,
        policy: *mut c_int,
        param: *mut crate::sched_param,
    ) -> c_int;
    pub fn pthread_setschedparam(
        native: crate::pthread_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;

    pub fn pthread_getcpuclockid(thread: crate::pthread_t, clk_id: *mut crate::clockid_t) -> c_int;

    pub fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;
    pub fn sem_destroy(sem: *mut sem_t) -> c_int;
    pub fn sem_timedwait(sem: *mut sem_t, abstime: *const crate::timespec) -> c_int;
    pub fn sem_getvalue(sem: *mut sem_t, sval: *mut c_int) -> c_int;

    pub fn clock_getres(__clock_id: clockid_t, __res: *mut crate::timespec) -> c_int;
    pub fn clock_gettime(__clock_id: clockid_t, __tp: *mut crate::timespec) -> c_int;
    pub fn clock_settime(__clock_id: clockid_t, __tp: *const crate::timespec) -> c_int;
    pub fn clock_getcpuclockid(pid: crate::pid_t, clk_id: *mut crate::clockid_t) -> c_int;

    pub fn clock_nanosleep(
        clk_id: crate::clockid_t,
        flags: c_int,
        rqtp: *const crate::timespec,
        rmtp: *mut crate::timespec,
    ) -> c_int;

    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut crate::timezone) -> c_int;
    pub fn settimeofday(tv: *const crate::timeval, tz: *const crate::timezone) -> c_int;

    pub fn asctime_r(tm: *const crate::tm, buf: *mut c_char) -> *mut c_char;
    pub fn ctime_r(timep: *const time_t, buf: *mut c_char) -> *mut c_char;

    pub fn strftime(
        s: *mut c_char,
        max: size_t,
        format: *const c_char,
        tm: *const crate::tm,
    ) -> size_t;
    pub fn strptime(s: *const c_char, format: *const c_char, tm: *mut crate::tm) -> *mut c_char;

    pub fn timer_create(
        clockid: crate::clockid_t,
        sevp: *mut crate::sigevent,
        timerid: *mut crate::timer_t,
    ) -> c_int;
    pub fn timer_delete(timerid: crate::timer_t) -> c_int;
    pub fn timer_getoverrun(timerid: crate::timer_t) -> c_int;
    pub fn timer_gettime(timerid: crate::timer_t, curr_value: *mut crate::itimerspec) -> c_int;
    pub fn timer_settime(
        timerid: crate::timer_t,
        flags: c_int,
        new_value: *const crate::itimerspec,
        old_value: *mut crate::itimerspec,
    ) -> c_int;

    pub fn fstat(__fd: c_int, __buf: *mut stat) -> c_int;
    pub fn fstat64(__fd: c_int, __buf: *mut stat64) -> c_int;

    pub fn fstatat(__fd: c_int, __file: *const c_char, __buf: *mut stat, __flag: c_int) -> c_int;
    pub fn fstatat64(
        __fd: c_int,
        __file: *const c_char,
        __buf: *mut stat64,
        __flag: c_int,
    ) -> c_int;

    pub fn statx(
        dirfd: c_int,
        pathname: *const c_char,
        flags: c_int,
        mask: c_uint,
        statxbuf: *mut statx,
    ) -> c_int;

    pub fn ftruncate(__fd: c_int, __length: __off_t) -> c_int;
    pub fn ftruncate64(__fd: c_int, __length: __off64_t) -> c_int;
    pub fn truncate64(__file: *const c_char, __length: __off64_t) -> c_int;

    pub fn lstat(__file: *const c_char, __buf: *mut stat) -> c_int;
    pub fn lstat64(__file: *const c_char, __buf: *mut stat64) -> c_int;

    pub fn statfs(path: *const c_char, buf: *mut statfs) -> c_int;
    pub fn statfs64(__file: *const c_char, __buf: *mut statfs64) -> c_int;
    pub fn fstatfs(fd: c_int, buf: *mut statfs) -> c_int;
    pub fn fstatfs64(__fildes: c_int, __buf: *mut statfs64) -> c_int;

    pub fn statvfs(__file: *const c_char, __buf: *mut statvfs) -> c_int;
    pub fn statvfs64(__file: *const c_char, __buf: *mut statvfs64) -> c_int;
    pub fn fstatvfs(__fildes: c_int, __buf: *mut statvfs) -> c_int;
    pub fn fstatvfs64(__fildes: c_int, __buf: *mut statvfs64) -> c_int;

    pub fn open(__file: *const c_char, __oflag: c_int, ...) -> c_int;
    pub fn open64(__file: *const c_char, __oflag: c_int, ...) -> c_int;

    pub fn openat(__fd: c_int, __file: *const c_char, __oflag: c_int, ...) -> c_int;
    pub fn openat64(__fd: c_int, __file: *const c_char, __oflag: c_int, ...) -> c_int;

    pub fn fopen64(filename: *const c_char, mode: *const c_char) -> *mut crate::FILE;
    pub fn freopen64(
        filename: *const c_char,
        mode: *const c_char,
        file: *mut crate::FILE,
    ) -> *mut crate::FILE;

    pub fn creat64(path: *const c_char, mode: mode_t) -> c_int;

    pub fn mkostemp(template: *mut c_char, flags: c_int) -> c_int;
    pub fn mkostemps(template: *mut c_char, suffixlen: c_int, flags: c_int) -> c_int;
    pub fn mkstemps(template: *mut c_char, suffixlen: c_int) -> c_int;
    pub fn tmpfile64() -> *mut crate::FILE;

    pub fn popen(command: *const c_char, mode: *const c_char) -> *mut crate::FILE;

    pub fn getdtablesize() -> c_int;

    // Added in `glibc` 2.34
    pub fn close_range(first: c_uint, last: c_uint, flags: c_int) -> c_int;

    pub fn openpty(
        __amaster: *mut c_int,
        __aslave: *mut c_int,
        __name: *mut c_char,
        __termp: *const termios,
        __winp: *const crate::winsize,
    ) -> c_int;

    pub fn forkpty(
        __amaster: *mut c_int,
        __name: *mut c_char,
        __termp: *const termios,
        __winp: *const crate::winsize,
    ) -> crate::pid_t;

    pub fn getpt() -> c_int;
    pub fn ptsname_r(fd: c_int, buf: *mut c_char, buflen: size_t) -> c_int;
    pub fn login_tty(fd: c_int) -> c_int;

    pub fn ctermid(s: *mut c_char) -> *mut c_char;

    pub fn clearenv() -> c_int;

    pub fn execveat(
        dirfd: c_int,
        pathname: *const c_char,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
        flags: c_int,
    ) -> c_int;

    // DIFF(main): changed to `*const *mut` in e77f551de9
    pub fn execvpe(
        file: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int;
    pub fn fexecve(fd: c_int, argv: *const *const c_char, envp: *const *const c_char) -> c_int;

    pub fn daemon(nochdir: c_int, noclose: c_int) -> c_int;

    // posix/spawn.h
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
    pub fn posix_spawnattr_init(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_destroy(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_getsigdefault(
        attr: *const posix_spawnattr_t,
        default: *mut crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_setsigdefault(
        attr: *mut posix_spawnattr_t,
        default: *const crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_getsigmask(
        attr: *const posix_spawnattr_t,
        default: *mut crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_setsigmask(
        attr: *mut posix_spawnattr_t,
        default: *const crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_getflags(attr: *const posix_spawnattr_t, flags: *mut c_short) -> c_int;
    pub fn posix_spawnattr_setflags(attr: *mut posix_spawnattr_t, flags: c_short) -> c_int;
    pub fn posix_spawnattr_getpgroup(
        attr: *const posix_spawnattr_t,
        flags: *mut crate::pid_t,
    ) -> c_int;
    pub fn posix_spawnattr_setpgroup(attr: *mut posix_spawnattr_t, flags: crate::pid_t) -> c_int;
    pub fn posix_spawnattr_getschedpolicy(
        attr: *const posix_spawnattr_t,
        flags: *mut c_int,
    ) -> c_int;
    pub fn posix_spawnattr_setschedpolicy(attr: *mut posix_spawnattr_t, flags: c_int) -> c_int;
    pub fn posix_spawnattr_getschedparam(
        attr: *const posix_spawnattr_t,
        param: *mut crate::sched_param,
    ) -> c_int;
    pub fn posix_spawnattr_setschedparam(
        attr: *mut posix_spawnattr_t,
        param: *const crate::sched_param,
    ) -> c_int;

    pub fn posix_spawn_file_actions_init(actions: *mut posix_spawn_file_actions_t) -> c_int;
    pub fn posix_spawn_file_actions_destroy(actions: *mut posix_spawn_file_actions_t) -> c_int;
    pub fn posix_spawn_file_actions_addopen(
        actions: *mut posix_spawn_file_actions_t,
        fd: c_int,
        path: *const c_char,
        oflag: c_int,
        mode: mode_t,
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

    // Added in `glibc` 2.29
    pub fn posix_spawn_file_actions_addchdir_np(
        actions: *mut crate::posix_spawn_file_actions_t,
        path: *const c_char,
    ) -> c_int;
    // Added in `glibc` 2.29
    pub fn posix_spawn_file_actions_addfchdir_np(
        actions: *mut crate::posix_spawn_file_actions_t,
        fd: c_int,
    ) -> c_int;
    // Added in `glibc` 2.34
    pub fn posix_spawn_file_actions_addclosefrom_np(
        actions: *mut crate::posix_spawn_file_actions_t,
        from: c_int,
    ) -> c_int;
    // Added in `glibc` 2.35
    pub fn posix_spawn_file_actions_addtcsetpgrp_np(
        actions: *mut crate::posix_spawn_file_actions_t,
        tcfd: c_int,
    ) -> c_int;

    pub fn shm_open(name: *const c_char, oflag: c_int, mode: mode_t) -> c_int;
    pub fn shm_unlink(name: *const c_char) -> c_int;

    pub fn euidaccess(pathname: *const c_char, mode: c_int) -> c_int;
    pub fn eaccess(pathname: *const c_char, mode: c_int) -> c_int;

    pub fn faccessat(dirfd: c_int, pathname: *const c_char, mode: c_int, flags: c_int) -> c_int;

    pub fn stat(__file: *const c_char, __buf: *mut stat) -> c_int;
    pub fn stat64(__file: *const c_char, __buf: *mut stat64) -> c_int;

    pub fn readdir(dirp: *mut crate::DIR) -> *mut crate::dirent;
    pub fn readdir64(dirp: *mut crate::DIR) -> *mut crate::dirent64;
    pub fn readdir_r(
        dirp: *mut crate::DIR,
        entry: *mut crate::dirent,
        result: *mut *mut crate::dirent,
    ) -> c_int;
    pub fn readdir64_r(
        dirp: *mut crate::DIR,
        entry: *mut crate::dirent64,
        result: *mut *mut crate::dirent64,
    ) -> c_int;
    pub fn seekdir(dirp: *mut crate::DIR, loc: c_long);
    pub fn telldir(dirp: *mut crate::DIR) -> c_long;

    pub fn dirfd(dirp: *mut crate::DIR) -> c_int;

    #[link_name = "__xpg_strerror_r"]
    pub fn strerror_r(__errnum: c_int, __buf: *mut c_char, __buflen: size_t) -> c_int;

    pub fn __errno_location() -> *mut c_int;

    pub fn mmap64(
        __addr: *mut c_void,
        __len: size_t,
        __prot: c_int,
        __flags: c_int,
        __fd: c_int,
        __offset: __off64_t,
    ) -> *mut c_void;

    pub fn mremap(
        addr: *mut c_void,
        len: size_t,
        new_len: size_t,
        flags: c_int,
        ...
    ) -> *mut c_void;

    pub fn mprotect(__addr: *mut c_void, __len: size_t, __prot: c_int) -> c_int;

    pub fn msync(__addr: *mut c_void, __len: size_t, __flags: c_int) -> c_int;
    pub fn sync();
    pub fn syncfs(fd: c_int) -> c_int;
    pub fn fdatasync(fd: c_int) -> c_int;

    pub fn fallocate64(fd: c_int, mode: c_int, offset: off64_t, len: off64_t) -> c_int;
    pub fn posix_fallocate(fd: c_int, offset: off_t, len: off_t) -> c_int;
    pub fn posix_fallocate64(fd: c_int, offset: off64_t, len: off64_t) -> c_int;

    pub fn posix_fadvise(fd: c_int, offset: off_t, len: off_t, advise: c_int) -> c_int;

    pub fn posix_fadvise64(fd: c_int, offset: off64_t, len: off64_t, advise: c_int) -> c_int;

    pub fn madvise(__addr: *mut c_void, __len: size_t, __advice: c_int) -> c_int;

    pub fn posix_madvise(addr: *mut c_void, len: size_t, advice: c_int) -> c_int;

    pub fn getrlimit(resource: crate::__rlimit_resource_t, rlim: *mut crate::rlimit) -> c_int;
    pub fn getrlimit64(resource: crate::__rlimit_resource_t, rlim: *mut crate::rlimit64) -> c_int;
    pub fn setrlimit(resource: crate::__rlimit_resource_t, rlim: *const crate::rlimit) -> c_int;
    pub fn setrlimit64(resource: crate::__rlimit_resource_t, rlim: *const crate::rlimit64)
        -> c_int;

    pub fn getpriority(which: crate::__priority_which, who: crate::id_t) -> c_int;
    pub fn setpriority(which: crate::__priority_which, who: crate::id_t, prio: c_int) -> c_int;

    pub fn getrandom(__buffer: *mut c_void, __length: size_t, __flags: c_uint) -> ssize_t;
    pub fn getentropy(__buffer: *mut c_void, __length: size_t) -> c_int;

    pub fn memrchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;
    pub fn memmem(
        haystack: *const c_void,
        haystacklen: size_t,
        needle: *const c_void,
        needlelen: size_t,
    ) -> *mut c_void;
    pub fn strchrnul(s: *const c_char, c: c_int) -> *mut c_char;

    pub fn abs(i: c_int) -> c_int;
    pub fn labs(i: c_long) -> c_long;
    pub fn rand() -> c_int;
    pub fn srand(seed: c_uint);

    pub fn drand48() -> c_double;
    pub fn erand48(xseed: *mut c_ushort) -> c_double;
    pub fn lrand48() -> c_long;
    pub fn nrand48(xseed: *mut c_ushort) -> c_long;
    pub fn mrand48() -> c_long;
    pub fn jrand48(xseed: *mut c_ushort) -> c_long;
    pub fn srand48(seed: c_long);
    pub fn seed48(xseed: *mut c_ushort) -> *mut c_ushort;
    pub fn lcong48(p: *mut c_ushort);

    pub fn qsort_r(
        base: *mut c_void,
        num: size_t,
        size: size_t,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void, *mut c_void) -> c_int>,
        arg: *mut c_void,
    );

    pub fn brk(addr: *mut c_void) -> c_int;
    pub fn sbrk(increment: intptr_t) -> *mut c_void;

    pub fn memalign(align: size_t, size: size_t) -> *mut c_void;
    pub fn mallopt(param: c_int, value: c_int) -> c_int;

    pub fn mallinfo() -> crate::mallinfo;
    pub fn mallinfo2() -> crate::mallinfo2;
    pub fn malloc_info(options: c_int, stream: *mut crate::FILE) -> c_int;
    pub fn malloc_usable_size(ptr: *mut c_void) -> size_t;
    pub fn malloc_trim(__pad: size_t) -> c_int;

    pub fn iconv_open(tocode: *const c_char, fromcode: *const c_char) -> iconv_t;
    pub fn iconv(
        cd: iconv_t,
        inbuf: *mut *mut c_char,
        inbytesleft: *mut size_t,
        outbuf: *mut *mut c_char,
        outbytesleft: *mut size_t,
    ) -> size_t;
    pub fn iconv_close(cd: iconv_t) -> c_int;

    pub fn getopt_long(
        argc: c_int,
        argv: *const *mut c_char,
        optstring: *const c_char,
        longopts: *const option,
        longindex: *mut c_int,
    ) -> c_int;

    pub fn backtrace(buf: *mut *mut c_void, sz: c_int) -> c_int;

    pub fn reboot(how_to: c_int) -> c_int;

    pub fn getloadavg(loadavg: *mut c_double, nelem: c_int) -> c_int;

    pub fn regexec(
        preg: *const crate::regex_t,
        input: *const c_char,
        nmatch: size_t,
        pmatch: *mut regmatch_t,
        eflags: c_int,
    ) -> c_int;

    pub fn regerror(
        errcode: c_int,
        preg: *const crate::regex_t,
        errbuf: *mut c_char,
        errbuf_size: size_t,
    ) -> size_t;

    pub fn regfree(preg: *mut crate::regex_t);

    pub fn glob(
        pattern: *const c_char,
        flags: c_int,
        errfunc: Option<extern "C" fn(epath: *const c_char, errno: c_int) -> c_int>,
        pglob: *mut crate::glob_t,
    ) -> c_int;
    pub fn globfree(pglob: *mut crate::glob_t);

    pub fn glob64(
        pattern: *const c_char,
        flags: c_int,
        errfunc: Option<extern "C" fn(epath: *const c_char, errno: c_int) -> c_int>,
        pglob: *mut glob64_t,
    ) -> c_int;
    pub fn globfree64(pglob: *mut glob64_t);

    pub fn getxattr(
        path: *const c_char,
        name: *const c_char,
        value: *mut c_void,
        size: size_t,
    ) -> ssize_t;
    pub fn lgetxattr(
        path: *const c_char,
        name: *const c_char,
        value: *mut c_void,
        size: size_t,
    ) -> ssize_t;
    pub fn fgetxattr(
        filedes: c_int,
        name: *const c_char,
        value: *mut c_void,
        size: size_t,
    ) -> ssize_t;
    pub fn setxattr(
        path: *const c_char,
        name: *const c_char,
        value: *const c_void,
        size: size_t,
        flags: c_int,
    ) -> c_int;
    pub fn lsetxattr(
        path: *const c_char,
        name: *const c_char,
        value: *const c_void,
        size: size_t,
        flags: c_int,
    ) -> c_int;
    pub fn fsetxattr(
        filedes: c_int,
        name: *const c_char,
        value: *const c_void,
        size: size_t,
        flags: c_int,
    ) -> c_int;
    pub fn listxattr(path: *const c_char, list: *mut c_char, size: size_t) -> ssize_t;
    pub fn llistxattr(path: *const c_char, list: *mut c_char, size: size_t) -> ssize_t;
    pub fn flistxattr(filedes: c_int, list: *mut c_char, size: size_t) -> ssize_t;
    pub fn removexattr(path: *const c_char, name: *const c_char) -> c_int;
    pub fn lremovexattr(path: *const c_char, name: *const c_char) -> c_int;
    pub fn fremovexattr(filedes: c_int, name: *const c_char) -> c_int;

    pub fn dirname(path: *mut c_char) -> *mut c_char;
    /// POSIX version of `basename(3)`, defined in `libgen.h`.
    #[link_name = "__xpg_basename"]
    pub fn posix_basename(path: *mut c_char) -> *mut c_char;
    /// GNU version of `basename(3)`, defined in `string.h`.
    #[link_name = "basename"]
    pub fn gnu_basename(path: *const c_char) -> *mut c_char;

    pub fn dlmopen(lmid: Lmid_t, filename: *const c_char, flag: c_int) -> *mut c_void;
    pub fn dlinfo(handle: *mut c_void, request: c_int, info: *mut c_void) -> c_int;
    pub fn dladdr1(
        addr: *const c_void,
        info: *mut crate::Dl_info,
        extra_info: *mut *mut c_void,
        flags: c_int,
    ) -> c_int;

    pub fn duplocale(base: crate::locale_t) -> crate::locale_t;
    pub fn freelocale(loc: crate::locale_t);
    pub fn newlocale(mask: c_int, locale: *const c_char, base: crate::locale_t) -> crate::locale_t;
    pub fn uselocale(loc: crate::locale_t) -> crate::locale_t;
    pub fn nl_langinfo(item: crate::nl_item) -> *mut c_char;
    pub fn nl_langinfo_l(item: crate::nl_item, locale: crate::locale_t) -> *mut c_char;

    pub fn dl_iterate_phdr(
        callback: Option<
            unsafe extern "C" fn(
                info: *mut crate::dl_phdr_info,
                size: size_t,
                data: *mut c_void,
            ) -> c_int,
        >,
        data: *mut c_void,
    ) -> c_int;

    pub fn gnu_get_libc_release() -> *const c_char;
    pub fn gnu_get_libc_version() -> *const c_char;
}

safe_f! {
    pub {const} fn makedev(major: c_uint, minor: c_uint) -> crate::dev_t {
        let major = major as crate::dev_t;
        let minor = minor as crate::dev_t;
        let mut dev = 0;
        dev |= major << 8;
        dev |= minor;
        dev
    }

    pub {const} fn major(dev: crate::dev_t) -> c_uint {
        ((dev >> 8) & 0xff) as c_uint
    }

    pub {const} fn minor(dev: crate::dev_t) -> c_uint {
        (dev & 0xffff00ff) as c_uint
    }

    pub fn SIGRTMAX() -> c_int {
        unsafe { __libc_current_sigrtmax() }
    }

    pub fn SIGRTMIN() -> c_int {
        unsafe { __libc_current_sigrtmin() }
    }

    pub {const} fn WIFSTOPPED(status: c_int) -> bool {
        (status & 0xff) == 0x7f
    }

    pub {const} fn WSTOPSIG(status: c_int) -> c_int {
        (status >> 8) & 0xff
    }

    pub {const} fn WIFCONTINUED(status: c_int) -> bool {
        status == 0xffff
    }

    pub {const} fn WIFSIGNALED(status: c_int) -> bool {
        ((status & 0x7f) + 1) as i8 >= 2
    }

    pub {const} fn WTERMSIG(status: c_int) -> c_int {
        status & 0x7f
    }

    pub {const} fn WIFEXITED(status: c_int) -> bool {
        (status & 0x7f) == 0
    }

    pub {const} fn WEXITSTATUS(status: c_int) -> c_int {
        (status >> 8) & 0xff
    }

    pub {const} fn WCOREDUMP(status: c_int) -> bool {
        (status & 0x80) != 0
    }

    pub {const} fn W_EXITCODE(ret: c_int, sig: c_int) -> c_int {
        (ret << 8) | sig
    }

    pub {const} fn W_STOPCODE(sig: c_int) -> c_int {
        (sig << 8) | 0x7f
    }

    pub {const} fn QCMD(cmd: c_int, type_: c_int) -> c_int {
        (cmd << 8) | (type_ & 0x00ff)
    }

    pub {const} fn IPOPT_COPIED(o: u8) -> u8 {
        o & IPOPT_COPY
    }

    pub {const} fn IPOPT_CLASS(o: u8) -> u8 {
        o & IPOPT_CLASS_MASK
    }

    pub {const} fn IPOPT_NUMBER(o: u8) -> u8 {
        o & IPOPT_NUMBER_MASK
    }

    pub {const} fn IPTOS_ECN(x: u8) -> u8 {
        x & crate::IPTOS_ECN_MASK
    }
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
