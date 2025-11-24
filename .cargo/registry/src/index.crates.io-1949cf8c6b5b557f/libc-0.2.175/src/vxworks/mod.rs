//! Interface to VxWorks C library

use core::ptr::null_mut;

use crate::prelude::*;

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum DIR {}
impl Copy for DIR {}
impl Clone for DIR {
    fn clone(&self) -> DIR {
        *self
    }
}

pub type intmax_t = i64;
pub type uintmax_t = u64;

pub type uintptr_t = usize;
pub type intptr_t = isize;
pub type ptrdiff_t = isize;
pub type size_t = crate::uintptr_t;
pub type ssize_t = intptr_t;

pub type pid_t = c_int;
pub type in_addr_t = u32;
pub type sighandler_t = size_t;
pub type cpuset_t = u32;

pub type blkcnt_t = c_long;
pub type blksize_t = c_long;
pub type ino_t = c_ulong;

pub type rlim_t = c_ulong;
pub type suseconds_t = c_long;
pub type time_t = c_longlong;

pub type errno_t = c_int;

pub type useconds_t = c_ulong;

pub type socklen_t = c_uint;

pub type pthread_t = c_ulong;

pub type clockid_t = c_int;

//defined for the structs
pub type dev_t = c_ulong;
pub type mode_t = c_int;
pub type nlink_t = c_ulong;
pub type uid_t = c_ushort;
pub type gid_t = c_ushort;
pub type sigset_t = c_ulonglong;
pub type key_t = c_long;

pub type nfds_t = c_uint;
pub type stat64 = crate::stat;

pub type pthread_key_t = c_ulong;

// From b_off_t.h
pub type off_t = c_longlong;
pub type off64_t = off_t;

// From b_BOOL.h
pub type BOOL = c_int;

// From vxWind.h ..
pub type _Vx_OBJ_HANDLE = c_int;
pub type _Vx_TASK_ID = crate::_Vx_OBJ_HANDLE;
pub type _Vx_MSG_Q_ID = crate::_Vx_OBJ_HANDLE;
pub type _Vx_SEM_ID_KERNEL = crate::_Vx_OBJ_HANDLE;
pub type _Vx_RTP_ID = crate::_Vx_OBJ_HANDLE;
pub type _Vx_SD_ID = crate::_Vx_OBJ_HANDLE;
pub type _Vx_CONDVAR_ID = crate::_Vx_OBJ_HANDLE;
pub type _Vx_SEM_ID = *mut crate::_Vx_semaphore;
pub type OBJ_HANDLE = crate::_Vx_OBJ_HANDLE;
pub type TASK_ID = crate::OBJ_HANDLE;
pub type MSG_Q_ID = crate::OBJ_HANDLE;
pub type SEM_ID_KERNEL = crate::OBJ_HANDLE;
pub type RTP_ID = crate::OBJ_HANDLE;
pub type SD_ID = crate::OBJ_HANDLE;
pub type CONDVAR_ID = crate::OBJ_HANDLE;
pub type STATUS = crate::OBJ_HANDLE;

// From vxTypes.h
pub type _Vx_usr_arg_t = isize;
pub type _Vx_exit_code_t = isize;
pub type _Vx_ticks_t = c_uint;
pub type _Vx_ticks64_t = c_ulonglong;

pub type sa_family_t = c_uchar;

// mqueue.h
pub type mqd_t = c_int;

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum _Vx_semaphore {}
impl Copy for _Vx_semaphore {}
impl Clone for _Vx_semaphore {
    fn clone(&self) -> _Vx_semaphore {
        *self
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

s! {
    // b_pthread_condattr_t.h
    pub struct pthread_condattr_t {
        pub condAttrStatus: c_int,
        pub condAttrPshared: c_int,
        pub condAttrClockId: crate::clockid_t,
    }

    // b_pthread_cond_t.h
    pub struct pthread_cond_t {
        pub condSemId: crate::_Vx_SEM_ID,
        pub condValid: c_int,
        pub condInitted: c_int,
        pub condRefCount: c_int,
        pub condMutex: *mut crate::pthread_mutex_t,
        pub condAttr: crate::pthread_condattr_t,
        pub condSemName: [c_char; _PTHREAD_SHARED_SEM_NAME_MAX],
    }

    // b_pthread_rwlockattr_t.h
    pub struct pthread_rwlockattr_t {
        pub rwlockAttrStatus: c_int,
        pub rwlockAttrPshared: c_int,
        pub rwlockAttrMaxReaders: c_uint,
        pub rwlockAttrConformOpt: c_uint,
    }

    // b_pthread_rwlock_t.h
    pub struct pthread_rwlock_t {
        pub rwlockSemId: crate::_Vx_SEM_ID,
        pub rwlockReadersRefCount: c_uint,
        pub rwlockValid: c_int,
        pub rwlockInitted: c_int,
        pub rwlockAttr: crate::pthread_rwlockattr_t,
        pub rwlockSemName: [c_char; _PTHREAD_SHARED_SEM_NAME_MAX],
    }

    // b_struct_timeval.h
    pub struct timeval {
        pub tv_sec: crate::time_t,
        pub tv_usec: crate::suseconds_t,
    }

    // socket.h
    pub struct linger {
        pub l_onoff: c_int,
        pub l_linger: c_int,
    }

    pub struct sockaddr {
        pub sa_len: c_uchar,
        pub sa_family: sa_family_t,
        pub sa_data: [c_char; 14],
    }

    pub struct iovec {
        pub iov_base: *mut c_void,
        pub iov_len: size_t,
    }

    pub struct msghdr {
        pub msg_name: *mut c_void,
        pub msg_namelen: socklen_t,
        pub msg_iov: *mut iovec,
        pub msg_iovlen: c_int,
        pub msg_control: *mut c_void,
        pub msg_controllen: socklen_t,
        pub msg_flags: c_int,
    }

    pub struct cmsghdr {
        pub cmsg_len: socklen_t,
        pub cmsg_level: c_int,
        pub cmsg_type: c_int,
    }

    // poll.h
    pub struct pollfd {
        pub fd: c_int,
        pub events: c_short,
        pub revents: c_short,
    }

    // resource.h
    pub struct rlimit {
        pub rlim_cur: crate::rlim_t,
        pub rlim_max: crate::rlim_t,
    }

    // stat.h
    pub struct stat {
        pub st_dev: crate::dev_t,
        pub st_ino: crate::ino_t,
        pub st_mode: mode_t,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        pub st_size: off_t,
        pub st_atime: crate::time_t,
        pub st_mtime: crate::time_t,
        pub st_ctime: crate::time_t,
        pub st_blksize: crate::blksize_t,
        pub st_blocks: crate::blkcnt_t,
        pub st_attrib: c_uchar,
        pub st_reserved1: c_int,
        pub st_reserved2: c_int,
        pub st_reserved3: c_int,
        pub st_reserved4: c_int,
    }

    //b_struct__Timespec.h
    pub struct _Timespec {
        pub tv_sec: crate::time_t,
        pub tv_nsec: c_long,
    }

    // b_struct__Sched_param.h
    pub struct sched_param {
        pub sched_priority: c_int,                  /* scheduling priority */
        pub sched_ss_low_priority: c_int,           /* low scheduling priority */
        pub sched_ss_repl_period: crate::_Timespec, /* replenishment period */
        pub sched_ss_init_budget: crate::_Timespec, /* initial budget */
        pub sched_ss_max_repl: c_int,               /* max pending replenishment */
    }

    // b_pthread_attr_t.h
    pub struct pthread_attr_t {
        pub threadAttrStatus: c_int,
        pub threadAttrStacksize: size_t,
        pub threadAttrStackaddr: *mut c_void,
        pub threadAttrGuardsize: size_t,
        pub threadAttrDetachstate: c_int,
        pub threadAttrContentionscope: c_int,
        pub threadAttrInheritsched: c_int,
        pub threadAttrSchedpolicy: c_int,
        pub threadAttrName: *mut c_char,
        pub threadAttrOptions: c_int,
        pub threadAttrSchedparam: crate::sched_param,
    }

    // signal.h

    pub struct sigaction {
        pub sa_u: crate::sa_u_t,
        pub sa_mask: crate::sigset_t,
        pub sa_flags: c_int,
    }

    // b_stack_t.h
    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_size: size_t,
        pub ss_flags: c_int,
    }

    // signal.h
    pub struct siginfo_t {
        pub si_signo: c_int,
        pub si_code: c_int,
        pub si_value: crate::sigval,
        pub si_errno: c_int,
        pub si_status: c_int,
        pub si_addr: *mut c_void,
        pub si_uid: crate::uid_t,
        pub si_pid: crate::pid_t,
    }

    // pthread.h (krnl)
    // b_pthread_mutexattr_t.h (usr)
    pub struct pthread_mutexattr_t {
        mutexAttrStatus: c_int,
        mutexAttrPshared: c_int,
        mutexAttrProtocol: c_int,
        mutexAttrPrioceiling: c_int,
        mutexAttrType: c_int,
    }

    // pthread.h (krnl)
    // b_pthread_mutex_t.h (usr)
    pub struct pthread_mutex_t {
        pub mutexSemId: crate::_Vx_SEM_ID, /*_Vx_SEM_ID ..*/
        pub mutexValid: c_int,
        pub mutexInitted: c_int,
        pub mutexCondRefCount: c_int,
        pub mutexSavPriority: c_int,
        pub mutexAttr: crate::pthread_mutexattr_t,
        pub mutexSemName: [c_char; _PTHREAD_SHARED_SEM_NAME_MAX],
    }

    // b_struct_timespec.h
    pub struct timespec {
        pub tv_sec: crate::time_t,
        pub tv_nsec: c_long,
    }

    // time.h
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

    // in.h
    pub struct in_addr {
        pub s_addr: in_addr_t,
    }

    // in.h
    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    // in6.h
    #[repr(align(4))]
    pub struct in6_addr {
        pub s6_addr: [u8; 16],
    }

    // in6.h
    pub struct ipv6_mreq {
        pub ipv6mr_multiaddr: in6_addr,
        pub ipv6mr_interface: c_uint,
    }

    // netdb.h
    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        pub ai_addrlen: size_t,
        pub ai_canonname: *mut c_char,
        pub ai_addr: *mut crate::sockaddr,
        pub ai_next: *mut crate::addrinfo,
    }

    // in.h
    pub struct sockaddr_in {
        pub sin_len: u8,
        pub sin_family: u8,
        pub sin_port: u16,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [c_char; 8],
    }

    // in6.h
    pub struct sockaddr_in6 {
        pub sin6_len: u8,
        pub sin6_family: u8,
        pub sin6_port: u16,
        pub sin6_flowinfo: u32,
        pub sin6_addr: crate::in6_addr,
        pub sin6_scope_id: u32,
    }

    pub struct Dl_info {
        pub dli_fname: *const c_char,
        pub dli_fbase: *mut c_void,
        pub dli_sname: *const c_char,
        pub dli_saddr: *mut c_void,
    }

    pub struct mq_attr {
        pub mq_maxmsg: c_long,
        pub mq_msgsize: c_long,
        pub mq_flags: c_long,
        pub mq_curmsgs: c_long,
    }
}

s_no_extra_traits! {
    // dirent.h
    pub struct dirent {
        pub d_ino: crate::ino_t,
        pub d_name: [c_char; _PARM_NAME_MAX as usize + 1],
        pub d_type: c_uchar,
    }

    pub struct sockaddr_un {
        pub sun_len: u8,
        pub sun_family: sa_family_t,
        pub sun_path: [c_char; 104],
    }

    // rtpLibCommon.h
    pub struct RTP_DESC {
        pub status: c_int,
        pub options: u32,
        pub entrAddr: *mut c_void,
        pub initTaskId: crate::TASK_ID,
        pub parentId: crate::RTP_ID,
        pub pathName: [c_char; VX_RTP_NAME_LENGTH as usize + 1],
        pub taskCnt: c_int,
        pub textStart: *mut c_void,
        pub textEnd: *mut c_void,
    }
    // socket.h
    pub struct sockaddr_storage {
        pub ss_len: c_uchar,
        pub ss_family: crate::sa_family_t,
        pub __ss_pad1: [c_char; _SS_PAD1SIZE],
        pub __ss_align: i32,
        pub __ss_pad2: [c_char; _SS_PAD2SIZE],
    }

    pub union sa_u_t {
        pub sa_handler: Option<unsafe extern "C" fn(c_int) -> !>,
        pub sa_sigaction:
            Option<unsafe extern "C" fn(c_int, *mut crate::siginfo_t, *mut c_void) -> !>,
    }

    pub union sigval {
        pub sival_int: c_int,
        pub sival_ptr: *mut c_void,
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for sa_u_t {
            fn eq(&self, other: &sa_u_t) -> bool {
                unsafe {
                    let h1 = match self.sa_handler {
                        Some(handler) => handler as usize,
                        None => 0 as usize,
                    };
                    let h2 = match other.sa_handler {
                        Some(handler) => handler as usize,
                        None => 0 as usize,
                    };
                    h1 == h2
                }
            }
        }
        impl Eq for sa_u_t {}
        impl hash::Hash for sa_u_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    let h = match self.sa_handler {
                        Some(handler) => handler as usize,
                        None => 0 as usize,
                    };
                    h.hash(state)
                }
            }
        }

        impl PartialEq for sigval {
            fn eq(&self, other: &sigval) -> bool {
                unsafe { self.sival_ptr as usize == other.sival_ptr as usize }
            }
        }
        impl Eq for sigval {}
        impl hash::Hash for sigval {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe { (self.sival_ptr as usize).hash(state) };
            }
        }
    }
}

pub const STDIN_FILENO: c_int = 0;
pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;

pub const EXIT_SUCCESS: c_int = 0;
pub const EXIT_FAILURE: c_int = 1;

pub const EAI_SERVICE: c_int = 9;
pub const EAI_SOCKTYPE: c_int = 10;
pub const EAI_SYSTEM: c_int = 11;

// FIXME(vxworks): This is not defined in vxWorks, but we have to define it here
// to make the building pass for getrandom and std
pub const RTLD_DEFAULT: *mut c_void = 0i64 as *mut c_void;

//Clock Lib Stuff
pub const CLOCK_REALTIME: c_int = 0x0;
pub const CLOCK_MONOTONIC: c_int = 0x1;
pub const CLOCK_PROCESS_CPUTIME_ID: c_int = 0x2;
pub const CLOCK_THREAD_CPUTIME_ID: c_int = 0x3;
pub const TIMER_ABSTIME: c_int = 0x1;
pub const TIMER_RELTIME: c_int = 0x0;

// PTHREAD STUFF
pub const PTHREAD_INITIALIZED_OBJ: c_int = 0xF70990EF;
pub const PTHREAD_DESTROYED_OBJ: c_int = -1;
pub const PTHREAD_VALID_OBJ: c_int = 0xEC542A37;
pub const PTHREAD_INVALID_OBJ: c_int = -1;
pub const PTHREAD_UNUSED_YET_OBJ: c_int = -1;

pub const PTHREAD_PRIO_NONE: c_int = 0;
pub const PTHREAD_PRIO_INHERIT: c_int = 1;
pub const PTHREAD_PRIO_PROTECT: c_int = 2;

pub const PTHREAD_MUTEX_NORMAL: c_int = 0;
pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 1;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 2;
pub const PTHREAD_MUTEX_DEFAULT: c_int = PTHREAD_MUTEX_NORMAL;
pub const PTHREAD_STACK_MIN: usize = 4096;
pub const _PTHREAD_SHARED_SEM_NAME_MAX: usize = 30;

//sched.h
pub const SCHED_FIFO: c_int = 0x01;
pub const SCHED_RR: c_int = 0x02;
pub const SCHED_OTHER: c_int = 0x04;
pub const SCHED_SPORADIC: c_int = 0x08;
pub const PRIO_PROCESS: c_uint = 0;
pub const SCHED_FIFO_HIGH_PRI: c_int = 255;
pub const SCHED_FIFO_LOW_PRI: c_int = 0;
pub const SCHED_RR_HIGH_PRI: c_int = 255;
pub const SCHED_RR_LOW_PRI: c_int = 0;
pub const SCHED_SPORADIC_HIGH_PRI: c_int = 255;
pub const SCHED_SPORADIC_LOW_PRI: c_int = 0;

// ERRNO STUFF
pub const ERROR: c_int = -1;
pub const OK: c_int = 0;
pub const EPERM: c_int = 1; /* Not owner */
pub const ENOENT: c_int = 2; /* No such file or directory */
pub const ESRCH: c_int = 3; /* No such process */
pub const EINTR: c_int = 4; /* Interrupted system call */
pub const EIO: c_int = 5; /* I/O error */
pub const ENXIO: c_int = 6; /* No such device or address */
pub const E2BIG: c_int = 7; /* Arg list too long */
pub const ENOEXEC: c_int = 8; /* Exec format error */
pub const EBADF: c_int = 9; /* Bad file number */
pub const ECHILD: c_int = 10; /* No children */
pub const EAGAIN: c_int = 11; /* No more processes */
pub const ENOMEM: c_int = 12; /* Not enough core */
pub const EACCES: c_int = 13; /* Permission denied */
pub const EFAULT: c_int = 14;
pub const ENOTEMPTY: c_int = 15;
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
pub const ENAMETOOLONG: c_int = 26;
pub const EFBIG: c_int = 27;
pub const ENOSPC: c_int = 28;
pub const ESPIPE: c_int = 29;
pub const EROFS: c_int = 30;
pub const EMLINK: c_int = 31;
pub const EPIPE: c_int = 32;
pub const EDEADLK: c_int = 33;
pub const ENOLCK: c_int = 34;
pub const ENOTSUP: c_int = 35;
pub const EMSGSIZE: c_int = 36;
pub const EDOM: c_int = 37;
pub const ERANGE: c_int = 38;
pub const EDOOM: c_int = 39;
pub const EDESTADDRREQ: c_int = 40;
pub const EPROTOTYPE: c_int = 41;
pub const ENOPROTOOPT: c_int = 42;
pub const EPROTONOSUPPORT: c_int = 43;
pub const ESOCKTNOSUPPORT: c_int = 44;
pub const EOPNOTSUPP: c_int = 45;
pub const EPFNOSUPPORT: c_int = 46;
pub const EAFNOSUPPORT: c_int = 47;
pub const EADDRINUSE: c_int = 48;
pub const EADDRNOTAVAIL: c_int = 49;
pub const ENOTSOCK: c_int = 50;
pub const ENETUNREACH: c_int = 51;
pub const ENETRESET: c_int = 52;
pub const ECONNABORTED: c_int = 53;
pub const ECONNRESET: c_int = 54;
pub const ENOBUFS: c_int = 55;
pub const EISCONN: c_int = 56;
pub const ENOTCONN: c_int = 57;
pub const ESHUTDOWN: c_int = 58;
pub const ETOOMANYREFS: c_int = 59;
pub const ETIMEDOUT: c_int = 60;
pub const ECONNREFUSED: c_int = 61;
pub const ENETDOWN: c_int = 62;
pub const ETXTBSY: c_int = 63;
pub const ELOOP: c_int = 64;
pub const EHOSTUNREACH: c_int = 65;
pub const ENOTBLK: c_int = 66;
pub const EHOSTDOWN: c_int = 67;
pub const EINPROGRESS: c_int = 68;
pub const EALREADY: c_int = 69;
pub const EWOULDBLOCK: c_int = 70;
pub const ENOSYS: c_int = 71;
pub const ECANCELED: c_int = 72;
pub const ENOSR: c_int = 74;
pub const ENOSTR: c_int = 75;
pub const EPROTO: c_int = 76;
pub const EBADMSG: c_int = 77;
pub const ENODATA: c_int = 78;
pub const ETIME: c_int = 79;
pub const ENOMSG: c_int = 80;
pub const EFPOS: c_int = 81;
pub const EILSEQ: c_int = 82;
pub const EDQUOT: c_int = 83;
pub const EIDRM: c_int = 84;
pub const EOVERFLOW: c_int = 85;
pub const EMULTIHOP: c_int = 86;
pub const ENOLINK: c_int = 87;
pub const ESTALE: c_int = 88;
pub const EOWNERDEAD: c_int = 89;
pub const ENOTRECOVERABLE: c_int = 90;

// NFS errnos: Refer to pkgs_v2/storage/fs/nfs/h/nfs/nfsCommon.h
const M_nfsStat: c_int = 48 << 16;
enum nfsstat {
    NFSERR_REMOTE = 71,
    NFSERR_WFLUSH = 99,
    NFSERR_BADHANDLE = 10001,
    NFSERR_NOT_SYNC = 10002,
    NFSERR_BAD_COOKIE = 10003,
    NFSERR_TOOSMALL = 10005,
    NFSERR_BADTYPE = 10007,
    NFSERR_JUKEBOX = 10008,
}

pub const S_nfsLib_NFS_OK: c_int = OK;
pub const S_nfsLib_NFSERR_PERM: c_int = EPERM;
pub const S_nfsLib_NFSERR_NOENT: c_int = ENOENT;
pub const S_nfsLib_NFSERR_IO: c_int = EIO;
pub const S_nfsLib_NFSERR_NXIO: c_int = ENXIO;
pub const S_nfsLib_NFSERR_ACCESS: c_int = EACCES;
pub const S_nfsLib_NFSERR_EXIST: c_int = EEXIST;
pub const S_nfsLib_NFSERR_ENODEV: c_int = ENODEV;
pub const S_nfsLib_NFSERR_NOTDIR: c_int = ENOTDIR;
pub const S_nfsLib_NFSERR_ISDIR: c_int = EISDIR;
pub const S_nfsLib_NFSERR_INVAL: c_int = EINVAL;
pub const S_nfsLib_NFSERR_FBIG: c_int = EFBIG;
pub const S_nfsLib_NFSERR_NOSPC: c_int = ENOSPC;
pub const S_nfsLib_NFSERR_ROFS: c_int = EROFS;
pub const S_nfsLib_NFSERR_NAMETOOLONG: c_int = ENAMETOOLONG;
pub const S_nfsLib_NFSERR_NOTEMPTY: c_int = ENOTEMPTY;
pub const S_nfsLib_NFSERR_DQUOT: c_int = EDQUOT;
pub const S_nfsLib_NFSERR_STALE: c_int = ESTALE;
pub const S_nfsLib_NFSERR_WFLUSH: c_int = M_nfsStat | nfsstat::NFSERR_WFLUSH as c_int;
pub const S_nfsLib_NFSERR_REMOTE: c_int = M_nfsStat | nfsstat::NFSERR_REMOTE as c_int;
pub const S_nfsLib_NFSERR_BADHANDLE: c_int = M_nfsStat | nfsstat::NFSERR_BADHANDLE as c_int;
pub const S_nfsLib_NFSERR_NOT_SYNC: c_int = M_nfsStat | nfsstat::NFSERR_NOT_SYNC as c_int;
pub const S_nfsLib_NFSERR_BAD_COOKIE: c_int = M_nfsStat | nfsstat::NFSERR_BAD_COOKIE as c_int;
pub const S_nfsLib_NFSERR_NOTSUPP: c_int = EOPNOTSUPP;
pub const S_nfsLib_NFSERR_TOOSMALL: c_int = M_nfsStat | nfsstat::NFSERR_TOOSMALL as c_int;
pub const S_nfsLib_NFSERR_SERVERFAULT: c_int = EIO;
pub const S_nfsLib_NFSERR_BADTYPE: c_int = M_nfsStat | nfsstat::NFSERR_BADTYPE as c_int;
pub const S_nfsLib_NFSERR_JUKEBOX: c_int = M_nfsStat | nfsstat::NFSERR_JUKEBOX as c_int;

// internal offset values for below constants
const taskErrorBase: c_int = 0x00030000;
const semErrorBase: c_int = 0x00160000;
const objErrorBase: c_int = 0x003d0000;

// taskLibCommon.h
pub const S_taskLib_NAME_NOT_FOUND: c_int = taskErrorBase + 0x0065;
pub const S_taskLib_TASK_HOOK_TABLE_FULL: c_int = taskErrorBase + 0x0066;
pub const S_taskLib_TASK_HOOK_NOT_FOUND: c_int = taskErrorBase + 0x0067;
pub const S_taskLib_ILLEGAL_PRIORITY: c_int = taskErrorBase + 0x0068;

// FIXME(vxworks): could also be useful for TASK_DESC type
pub const VX_TASK_NAME_LENGTH: c_int = 31;
pub const VX_TASK_RENAME_LENGTH: c_int = 16;

// semLibCommon.h
pub const S_semLib_INVALID_STATE: c_int = semErrorBase + 0x0065;
pub const S_semLib_INVALID_OPTION: c_int = semErrorBase + 0x0066;
pub const S_semLib_INVALID_QUEUE_TYPE: c_int = semErrorBase + 0x0067;
pub const S_semLib_INVALID_OPERATION: c_int = semErrorBase + 0x0068;

// objLibCommon.h
pub const S_objLib_OBJ_ID_ERROR: c_int = objErrorBase + 0x0001;
pub const S_objLib_OBJ_UNAVAILABLE: c_int = objErrorBase + 0x0002;
pub const S_objLib_OBJ_DELETED: c_int = objErrorBase + 0x0003;
pub const S_objLib_OBJ_TIMEOUT: c_int = objErrorBase + 0x0004;
pub const S_objLib_OBJ_NO_METHOD: c_int = objErrorBase + 0x0005;

// in.h
pub const IPPROTO_IP: c_int = 0;
pub const IPPROTO_IPV6: c_int = 41;

pub const IP_TTL: c_int = 4;
pub const IP_MULTICAST_IF: c_int = 9;
pub const IP_MULTICAST_TTL: c_int = 10;
pub const IP_MULTICAST_LOOP: c_int = 11;
pub const IP_ADD_MEMBERSHIP: c_int = 12;
pub const IP_DROP_MEMBERSHIP: c_int = 13;

// in6.h
pub const IPV6_V6ONLY: c_int = 1;
pub const IPV6_UNICAST_HOPS: c_int = 4;
pub const IPV6_MULTICAST_IF: c_int = 9;
pub const IPV6_MULTICAST_HOPS: c_int = 10;
pub const IPV6_MULTICAST_LOOP: c_int = 11;
pub const IPV6_ADD_MEMBERSHIP: c_int = 12;
pub const IPV6_DROP_MEMBERSHIP: c_int = 13;

// STAT Stuff
pub const S_IFMT: c_int = 0o17_0000;
pub const S_IFIFO: c_int = 0o1_0000;
pub const S_IFCHR: c_int = 0o2_0000;
pub const S_IFDIR: c_int = 0o4_0000;
pub const S_IFBLK: c_int = 0o6_0000;
pub const S_IFREG: c_int = 0o10_0000;
pub const S_IFLNK: c_int = 0o12_0000;
pub const S_IFSHM: c_int = 0o13_0000;
pub const S_IFSOCK: c_int = 0o14_0000;
pub const S_ISUID: c_int = 0o4000;
pub const S_ISGID: c_int = 0o2000;
pub const S_ISTXT: c_int = 0o1000;
pub const S_ISVTX: c_int = 0o1000;
pub const S_IRUSR: c_int = 0o0400;
pub const S_IWUSR: c_int = 0o0200;
pub const S_IXUSR: c_int = 0o0100;
pub const S_IRWXU: c_int = 0o0700;
pub const S_IRGRP: c_int = 0o0040;
pub const S_IWGRP: c_int = 0o0020;
pub const S_IXGRP: c_int = 0o0010;
pub const S_IRWXG: c_int = 0o0070;
pub const S_IROTH: c_int = 0o0004;
pub const S_IWOTH: c_int = 0o0002;
pub const S_IXOTH: c_int = 0o0001;
pub const S_IRWXO: c_int = 0o0007;

pub const UTIME_OMIT: c_long = 0x3ffffffe;
pub const UTIME_NOW: c_long = 0x3fffffff;

// socket.h
pub const SOL_SOCKET: c_int = 0xffff;
pub const SOMAXCONN: c_int = 128;

pub const SO_DEBUG: c_int = 0x0001;
pub const SO_REUSEADDR: c_int = 0x0004;
pub const SO_KEEPALIVE: c_int = 0x0008;
pub const SO_DONTROUTE: c_int = 0x0010;
pub const SO_RCVLOWAT: c_int = 0x0012;
pub const SO_SNDLOWAT: c_int = 0x0013;
pub const SO_SNDTIMEO: c_int = 0x1005;
pub const SO_ACCEPTCONN: c_int = 0x001e;
pub const SO_BROADCAST: c_int = 0x0020;
pub const SO_USELOOPBACK: c_int = 0x0040;
pub const SO_LINGER: c_int = 0x0080;
pub const SO_REUSEPORT: c_int = 0x0200;

pub const SO_VLAN: c_int = 0x8000;

pub const SO_SNDBUF: c_int = 0x1001;
pub const SO_RCVBUF: c_int = 0x1002;
pub const SO_RCVTIMEO: c_int = 0x1006;
pub const SO_ERROR: c_int = 0x1007;
pub const SO_TYPE: c_int = 0x1008;
pub const SO_BINDTODEVICE: c_int = 0x1010;
pub const SO_OOBINLINE: c_int = 0x1011;
pub const SO_CONNTIMEO: c_int = 0x100a;

pub const SOCK_STREAM: c_int = 1;
pub const SOCK_DGRAM: c_int = 2;
pub const SOCK_RAW: c_int = 3;
pub const SOCK_RDM: c_int = 4;
pub const SOCK_SEQPACKET: c_int = 5;
pub const SOCK_PACKET: c_int = 10;

pub const _SS_MAXSIZE: usize = 128;
pub const _SS_ALIGNSIZE: usize = size_of::<u32>();
pub const _SS_PAD1SIZE: usize =
    _SS_ALIGNSIZE - size_of::<c_uchar>() - size_of::<crate::sa_family_t>();
pub const _SS_PAD2SIZE: usize = _SS_MAXSIZE
    - size_of::<c_uchar>()
    - size_of::<crate::sa_family_t>()
    - _SS_PAD1SIZE
    - _SS_ALIGNSIZE;

pub const MSG_OOB: c_int = 0x0001;
pub const MSG_PEEK: c_int = 0x0002;
pub const MSG_DONTROUTE: c_int = 0x0004;
pub const MSG_EOR: c_int = 0x0008;
pub const MSG_TRUNC: c_int = 0x0010;
pub const MSG_CTRUNC: c_int = 0x0020;
pub const MSG_WAITALL: c_int = 0x0040;
pub const MSG_DONTWAIT: c_int = 0x0080;
pub const MSG_EOF: c_int = 0x0100;
pub const MSG_EXP: c_int = 0x0200;
pub const MSG_MBUF: c_int = 0x0400;
pub const MSG_NOTIFICATION: c_int = 0x0800;
pub const MSG_COMPAT: c_int = 0x8000;

pub const AF_UNSPEC: c_int = 0;
pub const AF_LOCAL: c_int = 1;
pub const AF_UNIX: c_int = AF_LOCAL;
pub const AF_INET: c_int = 2;
pub const AF_NETLINK: c_int = 16;
pub const AF_ROUTE: c_int = 17;
pub const AF_LINK: c_int = 18;
pub const AF_PACKET: c_int = 19;
pub const pseudo_AF_KEY: c_int = 27;
pub const AF_KEY: c_int = pseudo_AF_KEY;
pub const AF_INET6: c_int = 28;
pub const AF_SOCKDEV: c_int = 31;
pub const AF_TIPC: c_int = 33;
pub const AF_MIPC: c_int = 34;
pub const AF_MIPC_SAFE: c_int = 35;
pub const AF_MAX: c_int = 37;

pub const SHUT_RD: c_int = 0;
pub const SHUT_WR: c_int = 1;
pub const SHUT_RDWR: c_int = 2;

pub const IPPROTO_TCP: c_int = 6;
pub const TCP_NODELAY: c_int = 1;
pub const TCP_MAXSEG: c_int = 2;
pub const TCP_NOPUSH: c_int = 3;
pub const TCP_KEEPIDLE: c_int = 4;
pub const TCP_KEEPINTVL: c_int = 5;
pub const TCP_KEEPCNT: c_int = 6;

// ioLib.h
pub const FIONREAD: c_int = 0x40040001;
pub const FIOFLUSH: c_int = 2;
pub const FIOOPTIONS: c_int = 3;
pub const FIOBAUDRATE: c_int = 4;
pub const FIODISKFORMAT: c_int = 5;
pub const FIODISKINIT: c_int = 6;
pub const FIOSEEK: c_int = 7;
pub const FIOWHERE: c_int = 8;
pub const FIODIRENTRY: c_int = 9;
pub const FIORENAME: c_int = 10;
pub const FIOREADYCHANGE: c_int = 11;
pub const FIODISKCHANGE: c_int = 13;
pub const FIOCANCEL: c_int = 14;
pub const FIOSQUEEZE: c_int = 15;
pub const FIOGETNAME: c_int = 18;
pub const FIONBIO: c_int = 0x90040010;

// limits.h
pub const PATH_MAX: c_int = _PARM_PATH_MAX;
pub const _POSIX_PATH_MAX: c_int = 256;

// Some poll stuff
pub const POLLIN: c_short = 0x0001;
pub const POLLPRI: c_short = 0x0002;
pub const POLLOUT: c_short = 0x0004;
pub const POLLRDNORM: c_short = 0x0040;
pub const POLLWRNORM: c_short = POLLOUT;
pub const POLLRDBAND: c_short = 0x0080;
pub const POLLWRBAND: c_short = 0x0100;
pub const POLLERR: c_short = 0x0008;
pub const POLLHUP: c_short = 0x0010;
pub const POLLNVAL: c_short = 0x0020;

// fnctlcom.h
pub const FD_CLOEXEC: c_int = 1;
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
pub const F_DUPFD_CLOEXEC: c_int = 14;

// signal.h
pub const SIG_DFL: sighandler_t = 0 as sighandler_t;
pub const SIG_IGN: sighandler_t = 1 as sighandler_t;
pub const SIG_ERR: sighandler_t = -1 as isize as sighandler_t;

pub const SIGHUP: c_int = 1;
pub const SIGINT: c_int = 2;
pub const SIGQUIT: c_int = 3;
pub const SIGILL: c_int = 4;
pub const SIGTRAP: c_int = 5;
pub const SIGABRT: c_int = 6;
pub const SIGEMT: c_int = 7;
pub const SIGFPE: c_int = 8;
pub const SIGKILL: c_int = 9;
pub const SIGBUS: c_int = 10;
pub const SIGSEGV: c_int = 11;
pub const SIGFMT: c_int = 12;
pub const SIGPIPE: c_int = 13;
pub const SIGALRM: c_int = 14;
pub const SIGTERM: c_int = 15;
pub const SIGCNCL: c_int = 16;
pub const SIGSTOP: c_int = 17;
pub const SIGTSTP: c_int = 18;
pub const SIGCONT: c_int = 19;
pub const SIGCHLD: c_int = 20;
pub const SIGTTIN: c_int = 21;
pub const SIGTTOU: c_int = 22;
pub const SIGUSR1: c_int = 30;
pub const SIGUSR2: c_int = 31;
pub const SIGPOLL: c_int = 32;
pub const SIGPROF: c_int = 33;
pub const SIGSYS: c_int = 34;
pub const SIGURG: c_int = 35;
pub const SIGVTALRM: c_int = 36;
pub const SIGXCPU: c_int = 37;
pub const SIGXFSZ: c_int = 38;
pub const SIGRTMIN: c_int = 48;

pub const SIGIO: c_int = SIGRTMIN;
pub const SIGWINCH: c_int = SIGRTMIN + 5;
pub const SIGLOST: c_int = SIGRTMIN + 6;

pub const SIG_BLOCK: c_int = 1;
pub const SIG_UNBLOCK: c_int = 2;
pub const SIG_SETMASK: c_int = 3;

pub const SA_NOCLDSTOP: c_int = 0x0001;
pub const SA_SIGINFO: c_int = 0x0002;
pub const SA_ONSTACK: c_int = 0x0004;
pub const SA_INTERRUPT: c_int = 0x0008;
pub const SA_RESETHAND: c_int = 0x0010;
pub const SA_RESTART: c_int = 0x0020;
pub const SA_NODEFER: c_int = 0x0040;
pub const SA_NOCLDWAIT: c_int = 0x0080;

pub const SI_SYNC: c_int = 0;
pub const SI_USER: c_int = -1;
pub const SI_QUEUE: c_int = -2;
pub const SI_TIMER: c_int = -3;
pub const SI_ASYNCIO: c_int = -4;
pub const SI_MESGQ: c_int = -5;
pub const SI_CHILD: c_int = -6;
pub const SI_KILL: c_int = SI_USER;

// vxParams.h definitions
pub const _PARM_NAME_MAX: c_int = 255;
pub const _PARM_PATH_MAX: c_int = 1024;

// WAIT STUFF
pub const WNOHANG: c_int = 0x01;
pub const WUNTRACED: c_int = 0x02;

const PTHREAD_MUTEXATTR_INITIALIZER: pthread_mutexattr_t = pthread_mutexattr_t {
    mutexAttrStatus: PTHREAD_INITIALIZED_OBJ,
    mutexAttrProtocol: PTHREAD_PRIO_NONE,
    mutexAttrPrioceiling: 0,
    mutexAttrType: PTHREAD_MUTEX_DEFAULT,
    mutexAttrPshared: 1,
};
pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
    mutexSemId: null_mut(),
    mutexValid: PTHREAD_VALID_OBJ,
    mutexInitted: PTHREAD_UNUSED_YET_OBJ,
    mutexCondRefCount: 0,
    mutexSavPriority: -1,
    mutexAttr: PTHREAD_MUTEXATTR_INITIALIZER,
    mutexSemName: [0; _PTHREAD_SHARED_SEM_NAME_MAX],
};

const PTHREAD_CONDATTR_INITIALIZER: pthread_condattr_t = pthread_condattr_t {
    condAttrStatus: 0xf70990ef,
    condAttrPshared: 1,
    condAttrClockId: CLOCK_REALTIME,
};
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    condSemId: null_mut(),
    condValid: PTHREAD_VALID_OBJ,
    condInitted: PTHREAD_UNUSED_YET_OBJ,
    condRefCount: 0,
    condMutex: null_mut(),
    condAttr: PTHREAD_CONDATTR_INITIALIZER,
    condSemName: [0; _PTHREAD_SHARED_SEM_NAME_MAX],
};

const PTHREAD_RWLOCKATTR_INITIALIZER: pthread_rwlockattr_t = pthread_rwlockattr_t {
    rwlockAttrStatus: PTHREAD_INITIALIZED_OBJ,
    rwlockAttrPshared: 1,
    rwlockAttrMaxReaders: 0,
    rwlockAttrConformOpt: 1,
};
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = pthread_rwlock_t {
    rwlockSemId: null_mut(),
    rwlockReadersRefCount: 0,
    rwlockValid: PTHREAD_VALID_OBJ,
    rwlockInitted: PTHREAD_UNUSED_YET_OBJ,
    rwlockAttr: PTHREAD_RWLOCKATTR_INITIALIZER,
    rwlockSemName: [0; _PTHREAD_SHARED_SEM_NAME_MAX],
};

pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;

// rtpLibCommon.h
pub const VX_RTP_NAME_LENGTH: c_int = 255;
pub const RTP_ID_ERROR: crate::RTP_ID = -1;

// h/public/unistd.h
pub const _SC_GETPW_R_SIZE_MAX: c_int = 21; // Via unistd.h
pub const _SC_PAGESIZE: c_int = 39;
pub const O_ACCMODE: c_int = 3;
pub const O_CLOEXEC: c_int = 0x100000; // fcntlcom
pub const O_EXCL: c_int = 0x0800;
pub const O_CREAT: c_int = 0x0200;
pub const O_TRUNC: c_int = 0x0400;
pub const O_APPEND: c_int = 0x0008;
pub const O_RDWR: c_int = 0x0002;
pub const O_WRONLY: c_int = 0x0001;
pub const O_RDONLY: c_int = 0;
pub const O_NONBLOCK: c_int = 0x4000;

// mman.h
pub const PROT_NONE: c_int = 0x0000;
pub const PROT_READ: c_int = 0x0001;
pub const PROT_WRITE: c_int = 0x0002;
pub const PROT_EXEC: c_int = 0x0004;

pub const MAP_SHARED: c_int = 0x0001;
pub const MAP_PRIVATE: c_int = 0x0002;
pub const MAP_ANON: c_int = 0x0004;
pub const MAP_ANONYMOUS: c_int = MAP_ANON;
pub const MAP_FIXED: c_int = 0x0010;
pub const MAP_CONTIG: c_int = 0x0020;

pub const MAP_FAILED: *mut c_void = !0 as *mut c_void;

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum FILE {}
impl Copy for FILE {}
impl Clone for FILE {
    fn clone(&self) -> FILE {
        *self
    }
}
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum fpos_t {} // FIXME(vxworks): fill this out with a struct
impl Copy for fpos_t {}
impl Clone for fpos_t {
    fn clone(&self) -> fpos_t {
        *self
    }
}

f! {
    pub {const} fn CMSG_ALIGN(len: usize) -> usize {
        len + size_of::<usize>() - 1 & !(size_of::<usize>() - 1)
    }

    pub fn CMSG_NXTHDR(mhdr: *const msghdr, cmsg: *const cmsghdr) -> *mut cmsghdr {
        let next = cmsg as usize
            + CMSG_ALIGN((*cmsg).cmsg_len as usize)
            + CMSG_ALIGN(size_of::<cmsghdr>());
        let max = (*mhdr).msg_control as usize + (*mhdr).msg_controllen as usize;
        if next <= max {
            (cmsg as usize + CMSG_ALIGN((*cmsg).cmsg_len as usize)) as *mut cmsghdr
        } else {
            core::ptr::null_mut::<cmsghdr>()
        }
    }

    pub fn CMSG_FIRSTHDR(mhdr: *const msghdr) -> *mut cmsghdr {
        if (*mhdr).msg_controllen as usize > 0 {
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
}

extern "C" {
    pub fn isalnum(c: c_int) -> c_int;
    pub fn isalpha(c: c_int) -> c_int;
    pub fn iscntrl(c: c_int) -> c_int;
    pub fn isdigit(c: c_int) -> c_int;
    pub fn isgraph(c: c_int) -> c_int;
    pub fn islower(c: c_int) -> c_int;
    pub fn isprint(c: c_int) -> c_int;
    pub fn ispunct(c: c_int) -> c_int;
    pub fn isspace(c: c_int) -> c_int;
    pub fn isupper(c: c_int) -> c_int;
    pub fn isxdigit(c: c_int) -> c_int;
    pub fn isblank(c: c_int) -> c_int;
    pub fn tolower(c: c_int) -> c_int;
    pub fn toupper(c: c_int) -> c_int;
    pub fn fopen(filename: *const c_char, mode: *const c_char) -> *mut FILE;
    pub fn freopen(filename: *const c_char, mode: *const c_char, file: *mut FILE) -> *mut FILE;
    pub fn fflush(file: *mut FILE) -> c_int;
    pub fn fclose(file: *mut FILE) -> c_int;
    pub fn remove(filename: *const c_char) -> c_int;
    pub fn rename(oldname: *const c_char, newname: *const c_char) -> c_int;
    pub fn tmpfile() -> *mut FILE;
    pub fn setvbuf(stream: *mut FILE, buffer: *mut c_char, mode: c_int, size: size_t) -> c_int;
    pub fn setbuf(stream: *mut FILE, buf: *mut c_char);
    pub fn getchar() -> c_int;
    pub fn putchar(c: c_int) -> c_int;
    pub fn fgetc(stream: *mut FILE) -> c_int;
    pub fn fgets(buf: *mut c_char, n: c_int, stream: *mut FILE) -> *mut c_char;
    pub fn fputc(c: c_int, stream: *mut FILE) -> c_int;
    pub fn fputs(s: *const c_char, stream: *mut FILE) -> c_int;
    pub fn puts(s: *const c_char) -> c_int;
    pub fn ungetc(c: c_int, stream: *mut FILE) -> c_int;
    pub fn fread(ptr: *mut c_void, size: size_t, nobj: size_t, stream: *mut FILE) -> size_t;
    pub fn fwrite(ptr: *const c_void, size: size_t, nobj: size_t, stream: *mut FILE) -> size_t;
    pub fn fseek(stream: *mut FILE, offset: c_long, whence: c_int) -> c_int;
    pub fn ftell(stream: *mut FILE) -> c_long;
    pub fn rewind(stream: *mut FILE);
    pub fn fgetpos(stream: *mut FILE, ptr: *mut fpos_t) -> c_int;
    pub fn fsetpos(stream: *mut FILE, ptr: *const fpos_t) -> c_int;
    pub fn feof(stream: *mut FILE) -> c_int;
    pub fn ferror(stream: *mut FILE) -> c_int;
    pub fn perror(s: *const c_char);
    pub fn atof(s: *const c_char) -> c_double;
    pub fn atoi(s: *const c_char) -> c_int;
    pub fn atol(s: *const c_char) -> c_long;
    pub fn atoll(s: *const c_char) -> c_longlong;
    pub fn strtod(s: *const c_char, endp: *mut *mut c_char) -> c_double;
    pub fn strtof(s: *const c_char, endp: *mut *mut c_char) -> c_float;
    pub fn strtol(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_long;
    pub fn strtoll(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_longlong;
    pub fn strtoul(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_ulong;
    pub fn strtoull(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_ulonglong;
    pub fn calloc(nobj: size_t, size: size_t) -> *mut c_void;
    pub fn malloc(size: size_t) -> *mut c_void;
    pub fn realloc(p: *mut c_void, size: size_t) -> *mut c_void;
    pub fn free(p: *mut c_void);
    pub fn abort() -> !;
    pub fn exit(status: c_int) -> !;
    pub fn atexit(cb: extern "C" fn()) -> c_int;
    pub fn system(s: *const c_char) -> c_int;
    pub fn getenv(s: *const c_char) -> *mut c_char;

    pub fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    pub fn strncpy(dst: *mut c_char, src: *const c_char, n: size_t) -> *mut c_char;
    pub fn strcat(s: *mut c_char, ct: *const c_char) -> *mut c_char;
    pub fn strncat(s: *mut c_char, ct: *const c_char, n: size_t) -> *mut c_char;
    pub fn strcmp(cs: *const c_char, ct: *const c_char) -> c_int;
    pub fn strncmp(cs: *const c_char, ct: *const c_char, n: size_t) -> c_int;
    pub fn strcoll(cs: *const c_char, ct: *const c_char) -> c_int;
    pub fn strchr(cs: *const c_char, c: c_int) -> *mut c_char;
    pub fn strrchr(cs: *const c_char, c: c_int) -> *mut c_char;
    pub fn strspn(cs: *const c_char, ct: *const c_char) -> size_t;
    pub fn strcspn(cs: *const c_char, ct: *const c_char) -> size_t;
    pub fn strdup(cs: *const c_char) -> *mut c_char;
    pub fn strpbrk(cs: *const c_char, ct: *const c_char) -> *mut c_char;
    pub fn strstr(cs: *const c_char, ct: *const c_char) -> *mut c_char;
    pub fn strcasecmp(s1: *const c_char, s2: *const c_char) -> c_int;
    pub fn strncasecmp(s1: *const c_char, s2: *const c_char, n: size_t) -> c_int;
    pub fn strlen(cs: *const c_char) -> size_t;
    pub fn strerror(n: c_int) -> *mut c_char;
    pub fn strtok(s: *mut c_char, t: *const c_char) -> *mut c_char;
    pub fn strxfrm(s: *mut c_char, ct: *const c_char, n: size_t) -> size_t;
    pub fn wcslen(buf: *const wchar_t) -> size_t;
    pub fn wcstombs(dest: *mut c_char, src: *const wchar_t, n: size_t) -> size_t;

    pub fn memchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;
    pub fn wmemchr(cx: *const wchar_t, c: wchar_t, n: size_t) -> *mut wchar_t;
    pub fn memcmp(cx: *const c_void, ct: *const c_void, n: size_t) -> c_int;
    pub fn memcpy(dest: *mut c_void, src: *const c_void, n: size_t) -> *mut c_void;
    pub fn memmove(dest: *mut c_void, src: *const c_void, n: size_t) -> *mut c_void;
    pub fn memset(dest: *mut c_void, c: c_int, n: size_t) -> *mut c_void;
}

extern "C" {
    pub fn fprintf(stream: *mut crate::FILE, format: *const c_char, ...) -> c_int;
    pub fn printf(format: *const c_char, ...) -> c_int;
    pub fn snprintf(s: *mut c_char, n: size_t, format: *const c_char, ...) -> c_int;
    pub fn sprintf(s: *mut c_char, format: *const c_char, ...) -> c_int;
    pub fn fscanf(stream: *mut crate::FILE, format: *const c_char, ...) -> c_int;
    pub fn scanf(format: *const c_char, ...) -> c_int;
    pub fn sscanf(s: *const c_char, format: *const c_char, ...) -> c_int;
    pub fn getchar_unlocked() -> c_int;
    pub fn putchar_unlocked(c: c_int) -> c_int;
    pub fn stat(path: *const c_char, buf: *mut stat) -> c_int;
    pub fn fdopen(fd: c_int, mode: *const c_char) -> *mut crate::FILE;
    pub fn fileno(stream: *mut crate::FILE) -> c_int;
    pub fn creat(path: *const c_char, mode: mode_t) -> c_int;
    pub fn rewinddir(dirp: *mut crate::DIR);
    pub fn fchown(fd: c_int, owner: crate::uid_t, group: crate::gid_t) -> c_int;
    pub fn access(path: *const c_char, amode: c_int) -> c_int;
    pub fn alarm(seconds: c_uint) -> c_uint;
    pub fn fchdir(dirfd: c_int) -> c_int;
    pub fn chown(path: *const c_char, uid: uid_t, gid: gid_t) -> c_int;
    pub fn fpathconf(filedes: c_int, name: c_int) -> c_long;
    pub fn getegid() -> gid_t;
    pub fn geteuid() -> uid_t;
    pub fn getgroups(ngroups_max: c_int, groups: *mut gid_t) -> c_int;
    pub fn getlogin() -> *mut c_char;
    pub fn getopt(argc: c_int, argv: *const *mut c_char, optstr: *const c_char) -> c_int;
    pub fn pathconf(path: *const c_char, name: c_int) -> c_long;
    pub fn pause() -> c_int;
    pub fn seteuid(uid: uid_t) -> c_int;
    pub fn setegid(gid: gid_t) -> c_int;
    pub fn sleep(secs: c_uint) -> c_uint;
    pub fn ttyname(fd: c_int) -> *mut c_char;
    pub fn wait(status: *mut c_int) -> pid_t;
    pub fn umask(mask: mode_t) -> mode_t;
    pub fn mlock(addr: *const c_void, len: size_t) -> c_int;
    pub fn mlockall(flags: c_int) -> c_int;
    pub fn munlock(addr: *const c_void, len: size_t) -> c_int;
    pub fn munlockall() -> c_int;

    pub fn mmap(
        addr: *mut c_void,
        len: size_t,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: off_t,
    ) -> *mut c_void;
    pub fn munmap(addr: *mut c_void, len: size_t) -> c_int;

    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;
    pub fn msync(addr: *mut c_void, len: size_t, flags: c_int) -> c_int;

    pub fn truncate(path: *const c_char, length: off_t) -> c_int;
    pub fn shm_open(name: *const c_char, oflag: c_int, mode: mode_t) -> c_int;
    pub fn shm_unlink(name: *const c_char) -> c_int;

    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut c_void) -> c_int;
    pub fn pthread_exit(value: *mut c_void) -> !;
    pub fn pthread_attr_setdetachstate(attr: *mut crate::pthread_attr_t, state: c_int) -> c_int;

    pub fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;

    pub fn sigaddset(set: *mut sigset_t, signum: c_int) -> c_int;

    pub fn sigaction(signum: c_int, act: *const sigaction, oldact: *mut sigaction) -> c_int;

    pub fn utimes(filename: *const c_char, times: *const crate::timeval) -> c_int;

    pub fn futimens(fd: c_int, times: *const crate::timespec) -> c_int;

    #[link_name = "_rtld_dlopen"]
    pub fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;

    #[link_name = "_rtld_dlerror"]
    pub fn dlerror() -> *mut c_char;

    #[link_name = "_rtld_dlsym"]
    pub fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;

    #[link_name = "_rtld_dlclose"]
    pub fn dlclose(handle: *mut c_void) -> c_int;

    #[link_name = "_rtld_dladdr"]
    pub fn dladdr(addr: *mut c_void, info: *mut Dl_info) -> c_int;

    // time.h
    pub fn gmtime_r(time_p: *const time_t, result: *mut tm) -> *mut tm;
    pub fn localtime_r(time_p: *const time_t, result: *mut tm) -> *mut tm;
    pub fn mktime(tm: *mut tm) -> time_t;
    pub fn time(time: *mut time_t) -> time_t;
    pub fn gmtime(time_p: *const time_t) -> *mut tm;
    pub fn localtime(time_p: *const time_t) -> *mut tm;
    pub fn timegm(tm: *mut tm) -> time_t;
    pub fn difftime(time1: time_t, time0: time_t) -> c_double;
    pub fn gethostname(name: *mut c_char, len: size_t) -> c_int;
    pub fn usleep(secs: crate::useconds_t) -> c_int;
    pub fn putenv(string: *mut c_char) -> c_int;
    pub fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;

    pub fn sigprocmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t) -> c_int;
    pub fn sigpending(set: *mut sigset_t) -> c_int;

    pub fn mkfifo(path: *const c_char, mode: mode_t) -> c_int;

    pub fn fseeko(stream: *mut crate::FILE, offset: off_t, whence: c_int) -> c_int;
    pub fn ftello(stream: *mut crate::FILE) -> off_t;
    pub fn mkstemp(template: *mut c_char) -> c_int;

    pub fn tmpnam(ptr: *mut c_char) -> *mut c_char;

    pub fn openlog(ident: *const c_char, logopt: c_int, facility: c_int);
    pub fn closelog();
    pub fn setlogmask(maskpri: c_int) -> c_int;
    pub fn syslog(priority: c_int, message: *const c_char, ...);
    pub fn getline(lineptr: *mut *mut c_char, n: *mut size_t, stream: *mut FILE) -> ssize_t;

}

extern "C" {
    // stdlib.h
    pub fn memalign(block_size: size_t, size_arg: size_t) -> *mut c_void;

    // ioLib.h
    pub fn getcwd(buf: *mut c_char, size: size_t) -> *mut c_char;

    // ioLib.h
    pub fn chdir(attr: *const c_char) -> c_int;

    // pthread.h
    pub fn pthread_mutexattr_init(attr: *mut pthread_mutexattr_t) -> c_int;

    // pthread.h
    pub fn pthread_mutexattr_destroy(attr: *mut pthread_mutexattr_t) -> c_int;

    // pthread.h
    pub fn pthread_mutexattr_settype(pAttr: *mut crate::pthread_mutexattr_t, pType: c_int)
        -> c_int;

    // pthread.h
    pub fn pthread_mutex_init(
        mutex: *mut pthread_mutex_t,
        attr: *const pthread_mutexattr_t,
    ) -> c_int;

    // pthread.h
    pub fn pthread_mutex_destroy(mutex: *mut pthread_mutex_t) -> c_int;

    // pthread.h
    pub fn pthread_mutex_lock(mutex: *mut pthread_mutex_t) -> c_int;

    // pthread.h
    pub fn pthread_mutex_trylock(mutex: *mut pthread_mutex_t) -> c_int;

    // pthread.h
    pub fn pthread_mutex_timedlock(attr: *mut pthread_mutex_t, spec: *const timespec) -> c_int;

    // pthread.h
    pub fn pthread_mutex_unlock(mutex: *mut pthread_mutex_t) -> c_int;

    // pthread.h
    pub fn pthread_attr_setname(pAttr: *mut crate::pthread_attr_t, name: *mut c_char) -> c_int;

    // pthread.h
    pub fn pthread_attr_setstacksize(attr: *mut crate::pthread_attr_t, stacksize: size_t) -> c_int;

    // pthread.h
    pub fn pthread_attr_getstacksize(
        attr: *const crate::pthread_attr_t,
        size: *mut size_t,
    ) -> c_int;

    // pthread.h
    pub fn pthread_attr_init(attr: *mut crate::pthread_attr_t) -> c_int;

    // pthread.h
    pub fn pthread_create(
        pThread: *mut crate::pthread_t,
        pAttr: *const crate::pthread_attr_t,
        start_routine: extern "C" fn(*mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;

    //pthread.h
    pub fn pthread_setschedparam(
        native: crate::pthread_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;

    //pthread.h
    pub fn pthread_getschedparam(
        native: crate::pthread_t,
        policy: *mut c_int,
        param: *mut crate::sched_param,
    ) -> c_int;

    //pthread.h
    pub fn pthread_attr_setinheritsched(
        attr: *mut crate::pthread_attr_t,
        inheritsched: c_int,
    ) -> c_int;

    //pthread.h
    pub fn pthread_attr_setschedpolicy(attr: *mut crate::pthread_attr_t, policy: c_int) -> c_int;

    // pthread.h
    pub fn pthread_attr_destroy(thread: *mut crate::pthread_attr_t) -> c_int;

    // pthread.h
    pub fn pthread_detach(thread: crate::pthread_t) -> c_int;

    // int pthread_atfork (void (*)(void), void (*)(void), void (*)(void));
    pub fn pthread_atfork(
        prepare: Option<unsafe extern "C" fn()>,
        parent: Option<unsafe extern "C" fn()>,
        child: Option<unsafe extern "C" fn()>,
    ) -> c_int;

    // stat.h
    pub fn fstat(fildes: c_int, buf: *mut stat) -> c_int;

    // stat.h
    pub fn lstat(path: *const c_char, buf: *mut stat) -> c_int;

    // unistd.h
    pub fn ftruncate(fd: c_int, length: off_t) -> c_int;

    // dirent.h
    pub fn readdir_r(
        pDir: *mut crate::DIR,
        entry: *mut crate::dirent,
        result: *mut *mut crate::dirent,
    ) -> c_int;

    // dirent.h
    pub fn readdir(pDir: *mut crate::DIR) -> *mut crate::dirent;

    // fcntl.h or
    // ioLib.h
    pub fn open(path: *const c_char, oflag: c_int, ...) -> c_int;

    // poll.h
    pub fn poll(fds: *mut pollfd, nfds: nfds_t, timeout: c_int) -> c_int;

    // pthread.h
    pub fn pthread_condattr_init(attr: *mut crate::pthread_condattr_t) -> c_int;

    // pthread.h
    pub fn pthread_condattr_destroy(attr: *mut crate::pthread_condattr_t) -> c_int;

    // pthread.h
    pub fn pthread_condattr_getclock(
        pAttr: *const crate::pthread_condattr_t,
        pClockId: *mut crate::clockid_t,
    ) -> c_int;

    // pthread.h
    pub fn pthread_condattr_setclock(
        pAttr: *mut crate::pthread_condattr_t,
        clockId: crate::clockid_t,
    ) -> c_int;

    // pthread.h
    pub fn pthread_cond_init(
        cond: *mut crate::pthread_cond_t,
        attr: *const crate::pthread_condattr_t,
    ) -> c_int;

    // pthread.h
    pub fn pthread_cond_destroy(cond: *mut pthread_cond_t) -> c_int;

    // pthread.h
    pub fn pthread_cond_signal(cond: *mut crate::pthread_cond_t) -> c_int;

    // pthread.h
    pub fn pthread_cond_broadcast(cond: *mut crate::pthread_cond_t) -> c_int;

    // pthread.h
    pub fn pthread_cond_wait(
        cond: *mut crate::pthread_cond_t,
        mutex: *mut crate::pthread_mutex_t,
    ) -> c_int;

    // pthread.h
    pub fn pthread_rwlockattr_init(attr: *mut crate::pthread_rwlockattr_t) -> c_int;

    // pthread.h
    pub fn pthread_rwlockattr_destroy(attr: *mut crate::pthread_rwlockattr_t) -> c_int;

    // pthread.h
    pub fn pthread_rwlockattr_setmaxreaders(
        attr: *mut crate::pthread_rwlockattr_t,
        attr2: c_uint,
    ) -> c_int;

    // pthread.h
    pub fn pthread_rwlock_init(
        attr: *mut crate::pthread_rwlock_t,
        host: *const crate::pthread_rwlockattr_t,
    ) -> c_int;

    // pthread.h
    pub fn pthread_rwlock_destroy(attr: *mut crate::pthread_rwlock_t) -> c_int;

    // pthread.h
    pub fn pthread_rwlock_rdlock(attr: *mut crate::pthread_rwlock_t) -> c_int;

    // pthread.h
    pub fn pthread_rwlock_tryrdlock(attr: *mut crate::pthread_rwlock_t) -> c_int;

    // pthread.h
    pub fn pthread_rwlock_timedrdlock(
        attr: *mut crate::pthread_rwlock_t,
        host: *const crate::timespec,
    ) -> c_int;

    // pthread.h
    pub fn pthread_rwlock_wrlock(attr: *mut crate::pthread_rwlock_t) -> c_int;

    // pthread.h
    pub fn pthread_rwlock_trywrlock(attr: *mut crate::pthread_rwlock_t) -> c_int;

    // pthread.h
    pub fn pthread_rwlock_timedwrlock(
        attr: *mut crate::pthread_rwlock_t,
        host: *const crate::timespec,
    ) -> c_int;

    // pthread.h
    pub fn pthread_rwlock_unlock(attr: *mut crate::pthread_rwlock_t) -> c_int;

    // pthread.h
    pub fn pthread_key_create(
        key: *mut crate::pthread_key_t,
        dtor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;

    // pthread.h
    pub fn pthread_key_delete(key: crate::pthread_key_t) -> c_int;

    // pthread.h
    pub fn pthread_setspecific(key: crate::pthread_key_t, value: *const c_void) -> c_int;

    // pthread.h
    pub fn pthread_getspecific(key: crate::pthread_key_t) -> *mut c_void;

    // pthread.h
    pub fn pthread_cond_timedwait(
        cond: *mut crate::pthread_cond_t,
        mutex: *mut crate::pthread_mutex_t,
        abstime: *const crate::timespec,
    ) -> c_int;

    // pthread.h
    pub fn pthread_attr_getname(attr: *mut crate::pthread_attr_t, name: *mut *mut c_char) -> c_int;

    // pthread.h
    pub fn pthread_join(thread: crate::pthread_t, status: *mut *mut c_void) -> c_int;

    // pthread.h
    pub fn pthread_self() -> crate::pthread_t;

    // clockLib.h
    pub fn clock_gettime(clock_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;

    // clockLib.h
    pub fn clock_settime(clock_id: crate::clockid_t, tp: *const crate::timespec) -> c_int;

    // clockLib.h
    pub fn clock_getres(clock_id: crate::clockid_t, res: *mut crate::timespec) -> c_int;

    // clockLib.h
    pub fn clock_nanosleep(
        clock_id: crate::clockid_t,
        flags: c_int,
        rqtp: *const crate::timespec,
        rmtp: *mut crate::timespec,
    ) -> c_int;

    // timerLib.h
    pub fn nanosleep(rqtp: *const crate::timespec, rmtp: *mut crate::timespec) -> c_int;

    // socket.h
    pub fn accept(s: c_int, addr: *mut crate::sockaddr, addrlen: *mut crate::socklen_t) -> c_int;

    // socket.h
    pub fn bind(fd: c_int, addr: *const sockaddr, len: socklen_t) -> c_int;

    // socket.h
    pub fn connect(s: c_int, name: *const crate::sockaddr, namelen: crate::socklen_t) -> c_int;

    // socket.h
    pub fn getpeername(
        s: c_int,
        name: *mut crate::sockaddr,
        namelen: *mut crate::socklen_t,
    ) -> c_int;

    // socket.h
    pub fn getsockname(socket: c_int, address: *mut sockaddr, address_len: *mut socklen_t)
        -> c_int;

    // socket.h
    pub fn getsockopt(
        sockfd: c_int,
        level: c_int,
        optname: c_int,
        optval: *mut c_void,
        optlen: *mut crate::socklen_t,
    ) -> c_int;

    // socket.h
    pub fn listen(socket: c_int, backlog: c_int) -> c_int;

    // socket.h
    pub fn recv(s: c_int, buf: *mut c_void, bufLen: size_t, flags: c_int) -> ssize_t;

    // socket.h
    pub fn recvfrom(
        s: c_int,
        buf: *mut c_void,
        bufLen: size_t,
        flags: c_int,
        from: *mut crate::sockaddr,
        pFromLen: *mut crate::socklen_t,
    ) -> ssize_t;

    pub fn recvmsg(socket: c_int, mp: *mut crate::msghdr, flags: c_int) -> ssize_t;

    // socket.h
    pub fn send(socket: c_int, buf: *const c_void, len: size_t, flags: c_int) -> ssize_t;

    pub fn sendmsg(socket: c_int, mp: *const crate::msghdr, flags: c_int) -> ssize_t;

    // socket.h
    pub fn sendto(
        socket: c_int,
        buf: *const c_void,
        len: size_t,
        flags: c_int,
        addr: *const sockaddr,
        addrlen: socklen_t,
    ) -> ssize_t;

    // socket.h
    pub fn setsockopt(
        socket: c_int,
        level: c_int,
        name: c_int,
        value: *const c_void,
        option_len: socklen_t,
    ) -> c_int;

    // socket.h
    pub fn shutdown(s: c_int, how: c_int) -> c_int;

    // socket.h
    pub fn socket(domain: c_int, _type: c_int, protocol: c_int) -> c_int;

    // icotl.h
    pub fn ioctl(fd: c_int, request: c_int, ...) -> c_int;

    // fcntl.h
    pub fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;

    // ntp_rfc2553.h for kernel
    // netdb.h for user
    pub fn gai_strerror(errcode: c_int) -> *mut c_char;

    // ioLib.h or
    // unistd.h
    pub fn close(fd: c_int) -> c_int;

    // ioLib.h or
    // unistd.h
    pub fn read(fd: c_int, buf: *mut c_void, count: size_t) -> ssize_t;

    // ioLib.h or
    // unistd.h
    pub fn write(fd: c_int, buf: *const c_void, count: size_t) -> ssize_t;

    // ioLib.h or
    // unistd.h
    pub fn isatty(fd: c_int) -> c_int;

    // ioLib.h or
    // unistd.h
    pub fn dup(src: c_int) -> c_int;

    // ioLib.h or
    // unistd.h
    pub fn dup2(src: c_int, dst: c_int) -> c_int;

    // ioLib.h or
    // unistd.h
    pub fn pipe(fds: *mut c_int) -> c_int;

    // ioLib.h or
    // unistd.h
    pub fn unlink(pathname: *const c_char) -> c_int;

    // unistd.h and
    // ioLib.h
    pub fn lseek(fd: c_int, offset: off_t, whence: c_int) -> off_t;

    // netdb.h
    pub fn getaddrinfo(
        node: *const c_char,
        service: *const c_char,
        hints: *const addrinfo,
        res: *mut *mut addrinfo,
    ) -> c_int;

    // netdb.h
    pub fn freeaddrinfo(res: *mut addrinfo);

    // signal.h
    pub fn signal(signum: c_int, handler: sighandler_t) -> sighandler_t;

    // unistd.h
    pub fn getpid() -> pid_t;

    // unistd.h
    pub fn getppid() -> pid_t;

    // wait.h
    pub fn waitpid(pid: pid_t, status: *mut c_int, options: c_int) -> pid_t;

    // unistd.h
    pub fn sysconf(attr: c_int) -> c_long;

    // stdlib.h
    pub fn setenv(
        // setenv.c
        envVarName: *const c_char,
        envVarValue: *const c_char,
        overwrite: c_int,
    ) -> c_int;

    // stdlib.h
    pub fn unsetenv(
        // setenv.c
        envVarName: *const c_char,
    ) -> c_int;

    // stdlib.h
    pub fn realpath(fileName: *const c_char, resolvedName: *mut c_char) -> *mut c_char;

    // unistd.h
    pub fn link(src: *const c_char, dst: *const c_char) -> c_int;

    // unistd.h
    pub fn readlink(path: *const c_char, buf: *mut c_char, bufsize: size_t) -> ssize_t;

    // unistd.h
    pub fn symlink(path1: *const c_char, path2: *const c_char) -> c_int;

    // dirent.h
    pub fn opendir(name: *const c_char) -> *mut crate::DIR;

    // unistd.h
    pub fn rmdir(path: *const c_char) -> c_int;

    // stat.h
    pub fn mkdir(dirName: *const c_char, mode: mode_t) -> c_int;

    // stat.h
    pub fn chmod(path: *const c_char, mode: mode_t) -> c_int;

    // stat.h
    pub fn fchmod(attr1: c_int, attr2: mode_t) -> c_int;

    // unistd.h
    pub fn fsync(fd: c_int) -> c_int;

    // dirent.h
    pub fn closedir(ptr: *mut crate::DIR) -> c_int;

    //sched.h
    pub fn sched_get_priority_max(policy: c_int) -> c_int;

    //sched.h
    pub fn sched_get_priority_min(policy: c_int) -> c_int;

    //sched.h
    pub fn sched_setparam(pid: crate::pid_t, param: *const crate::sched_param) -> c_int;

    //sched.h
    pub fn sched_getparam(pid: crate::pid_t, param: *mut crate::sched_param) -> c_int;

    //sched.h
    pub fn sched_setscheduler(
        pid: crate::pid_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;

    //sched.h
    pub fn sched_getscheduler(pid: crate::pid_t) -> c_int;

    //sched.h
    pub fn sched_rr_get_interval(pid: crate::pid_t, tp: *mut crate::timespec) -> c_int;

    // sched.h
    pub fn sched_yield() -> c_int;

    // errnoLib.h
    pub fn errnoSet(err: c_int) -> c_int;

    // errnoLib.h
    pub fn errnoGet() -> c_int;

    // unistd.h
    pub fn _exit(status: c_int) -> !;

    // unistd.h
    pub fn setgid(gid: crate::gid_t) -> c_int;

    // unistd.h
    pub fn getgid() -> crate::gid_t;

    // unistd.h
    pub fn setuid(uid: crate::uid_t) -> c_int;

    // unistd.h
    pub fn getuid() -> crate::uid_t;

    // signal.h
    pub fn sigemptyset(__set: *mut sigset_t) -> c_int;

    // pthread.h for kernel
    // signal.h for user
    pub fn pthread_sigmask(__how: c_int, __set: *const sigset_t, __oset: *mut sigset_t) -> c_int;

    // signal.h for user
    pub fn kill(__pid: pid_t, __signo: c_int) -> c_int;

    // signal.h for user
    pub fn sigqueue(__pid: pid_t, __signo: c_int, __value: crate::sigval) -> c_int;

    // signal.h for user
    pub fn _sigqueue(
        rtpId: crate::RTP_ID,
        signo: c_int,
        pValue: *const crate::sigval,
        sigCode: c_int,
    ) -> c_int;

    // signal.h
    pub fn taskKill(taskId: crate::TASK_ID, signo: c_int) -> c_int;

    // signal.h
    pub fn raise(__signo: c_int) -> c_int;

    // taskLibCommon.h
    pub fn taskIdSelf() -> crate::TASK_ID;
    pub fn taskDelay(ticks: crate::_Vx_ticks_t) -> c_int;

    // taskLib.h
    pub fn taskNameSet(task_id: crate::TASK_ID, task_name: *mut c_char) -> c_int;
    pub fn taskNameGet(task_id: crate::TASK_ID, buf_name: *mut c_char, bufsize: size_t) -> c_int;

    // rtpLibCommon.h
    pub fn rtpInfoGet(rtpId: crate::RTP_ID, rtpStruct: *mut crate::RTP_DESC) -> c_int;
    pub fn rtpSpawn(
        pubrtpFileName: *const c_char,
        argv: *mut *const c_char,
        envp: *mut *const c_char,
        priority: c_int,
        uStackSize: size_t,
        options: c_int,
        taskOptions: c_int,
    ) -> RTP_ID;

    // ioLib.h
    pub fn _realpath(fileName: *const c_char, resolvedName: *mut c_char) -> *mut c_char;

    // pathLib.h
    pub fn _pathIsAbsolute(filepath: *const c_char, pNameTail: *mut *const c_char) -> BOOL;

    pub fn writev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;
    pub fn readv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;

    // randomNumGen.h
    pub fn randBytes(buf: *mut c_uchar, length: c_int) -> c_int;
    pub fn randABytes(buf: *mut c_uchar, length: c_int) -> c_int;
    pub fn randUBytes(buf: *mut c_uchar, length: c_int) -> c_int;
    pub fn randSecure() -> c_int;

    // mqueue.h
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

    // vxCpuLib.h
    pub fn vxCpuEnabledGet() -> crate::cpuset_t; // Get set of running CPU's in the system
    pub fn vxCpuConfiguredGet() -> crate::cpuset_t; // Get set of Configured CPU's in the system
}

//Dummy functions, these don't really exist in VxWorks.

// wait.h macros
safe_f! {
    pub {const} fn WIFEXITED(status: c_int) -> bool {
        (status & 0xFF00) == 0
    }
    pub {const} fn WIFSIGNALED(status: c_int) -> bool {
        (status & 0xFF00) != 0
    }
    pub {const} fn WIFSTOPPED(status: c_int) -> bool {
        (status & 0xFF0000) != 0
    }
    pub {const} fn WEXITSTATUS(status: c_int) -> c_int {
        status & 0xFF
    }
    pub {const} fn WTERMSIG(status: c_int) -> c_int {
        (status >> 8) & 0xFF
    }
    pub {const} fn WSTOPSIG(status: c_int) -> c_int {
        (status >> 16) & 0xFF
    }
}

pub fn pread(_fd: c_int, _buf: *mut c_void, _count: size_t, _offset: off64_t) -> ssize_t {
    -1
}

pub fn pwrite(_fd: c_int, _buf: *const c_void, _count: size_t, _offset: off64_t) -> ssize_t {
    -1
}
pub fn posix_memalign(memptr: *mut *mut c_void, align: size_t, size: size_t) -> c_int {
    // check to see if align is a power of 2 and if align is a multiple
    //  of sizeof(void *)
    if (align & align - 1 != 0) || (align as usize % size_of::<size_t>() != 0) {
        return crate::EINVAL;
    }

    unsafe {
        // posix_memalign should not set errno
        let e = crate::errnoGet();

        let temp = memalign(align, size);
        crate::errnoSet(e as c_int);

        if temp.is_null() {
            crate::ENOMEM
        } else {
            *memptr = temp;
            0
        }
    }
}

cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(target_arch = "arm")] {
        mod arm;
        pub use self::arm::*;
    } else if #[cfg(target_arch = "x86")] {
        mod x86;
        pub use self::x86::*;
    } else if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(target_arch = "powerpc")] {
        mod powerpc;
        pub use self::powerpc::*;
    } else if #[cfg(target_arch = "powerpc64")] {
        mod powerpc64;
        pub use self::powerpc64::*;
    } else if #[cfg(target_arch = "riscv32")] {
        mod riscv32;
        pub use self::riscv32::*;
    } else if #[cfg(target_arch = "riscv64")] {
        mod riscv64;
        pub use self::riscv64::*;
    } else {
        // Unknown target_arch
    }
}
