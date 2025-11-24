use crate::prelude::*;
use crate::{
    exit_status, off_t, NET_MAC_AWARE, NET_MAC_AWARE_INHERIT, PRIV_AWARE_RESET, PRIV_DEBUG,
    PRIV_PFEXEC, PRIV_XPOLICY,
};

pub type lgrp_rsrc_t = c_int;
pub type lgrp_affinity_t = c_int;

s! {
    pub struct aiocb {
        pub aio_fildes: c_int,
        pub aio_buf: *mut c_void,
        pub aio_nbytes: size_t,
        pub aio_offset: off_t,
        pub aio_reqprio: c_int,
        pub aio_sigevent: crate::sigevent,
        pub aio_lio_opcode: c_int,
        pub aio_resultp: crate::aio_result_t,
        pub aio_state: c_int,
        pub aio__pad: [c_int; 1],
    }

    pub struct shmid_ds {
        pub shm_perm: crate::ipc_perm,
        pub shm_segsz: size_t,
        pub shm_amp: *mut c_void,
        pub shm_lkcnt: c_ushort,
        pub shm_lpid: crate::pid_t,
        pub shm_cpid: crate::pid_t,
        pub shm_nattch: crate::shmatt_t,
        pub shm_cnattch: c_ulong,
        pub shm_atime: crate::time_t,
        pub shm_dtime: crate::time_t,
        pub shm_ctime: crate::time_t,
        pub shm_pad4: [i64; 4],
    }

    pub struct fil_info {
        pub fi_flags: c_int,
        pub fi_pos: c_int,
        pub fi_name: [c_char; crate::FILNAME_MAX as usize],
    }
}

s_no_extra_traits! {
    #[cfg_attr(any(target_arch = "x86", target_arch = "x86_64"), repr(packed(4)))]
    pub struct epoll_event {
        pub events: u32,
        pub u64: u64,
    }

    pub struct utmpx {
        pub ut_user: [c_char; _UTX_USERSIZE],
        pub ut_id: [c_char; _UTX_IDSIZE],
        pub ut_line: [c_char; _UTX_LINESIZE],
        pub ut_pid: crate::pid_t,
        pub ut_type: c_short,
        pub ut_exit: exit_status,
        pub ut_tv: crate::timeval,
        pub ut_session: c_int,
        pub ut_pad: [c_int; _UTX_PADSIZE],
        pub ut_syslen: c_short,
        pub ut_host: [c_char; _UTX_HOSTSIZE],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for utmpx {
            fn eq(&self, other: &utmpx) -> bool {
                self.ut_type == other.ut_type
                    && self.ut_pid == other.ut_pid
                    && self.ut_user == other.ut_user
                    && self.ut_line == other.ut_line
                    && self.ut_id == other.ut_id
                    && self.ut_exit == other.ut_exit
                    && self.ut_session == other.ut_session
                    && self.ut_tv == other.ut_tv
                    && self.ut_syslen == other.ut_syslen
                    && self.ut_pad == other.ut_pad
                    && self
                        .ut_host
                        .iter()
                        .zip(other.ut_host.iter())
                        .all(|(a, b)| a == b)
            }
        }

        impl Eq for utmpx {}

        impl hash::Hash for utmpx {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ut_user.hash(state);
                self.ut_type.hash(state);
                self.ut_pid.hash(state);
                self.ut_line.hash(state);
                self.ut_id.hash(state);
                self.ut_host.hash(state);
                self.ut_exit.hash(state);
                self.ut_session.hash(state);
                self.ut_tv.hash(state);
                self.ut_syslen.hash(state);
                self.ut_pad.hash(state);
            }
        }

        impl PartialEq for epoll_event {
            fn eq(&self, other: &epoll_event) -> bool {
                self.events == other.events && self.u64 == other.u64
            }
        }
        impl Eq for epoll_event {}
        impl hash::Hash for epoll_event {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                let events = self.events;
                let u64 = self.u64;
                events.hash(state);
                u64.hash(state);
            }
        }
    }
}

pub const _UTX_USERSIZE: usize = 32;
pub const _UTX_LINESIZE: usize = 32;
pub const _UTX_PADSIZE: usize = 5;
pub const _UTX_IDSIZE: usize = 4;
pub const _UTX_HOSTSIZE: usize = 257;

pub const AF_LOCAL: c_int = 1; // AF_UNIX
pub const AF_FILE: c_int = 1; // AF_UNIX

pub const EFD_SEMAPHORE: c_int = 0x1;
pub const EFD_NONBLOCK: c_int = 0x800;
pub const EFD_CLOEXEC: c_int = 0x80000;

pub const POLLRDHUP: c_short = 0x4000;

pub const TCP_KEEPIDLE: c_int = 34;
pub const TCP_KEEPCNT: c_int = 35;
pub const TCP_KEEPINTVL: c_int = 36;
pub const TCP_CONGESTION: c_int = 37;

// These constants are correct for 64-bit programs or 32-bit programs that are
// not using large-file mode.  If Rust ever supports anything other than 64-bit
// compilation on illumos, this may require adjustment:
pub const F_OFD_GETLK: c_int = 47;
pub const F_OFD_SETLK: c_int = 48;
pub const F_OFD_SETLKW: c_int = 49;
pub const F_FLOCK: c_int = 53;
pub const F_FLOCKW: c_int = 54;

pub const F_DUPFD_CLOEXEC: c_int = 37;
pub const F_DUPFD_CLOFORK: c_int = 58;
pub const F_DUP2FD_CLOEXEC: c_int = 36;
pub const F_DUP2FD_CLOFORK: c_int = 57;
pub const F_DUP3FD: c_int = 59;

pub const FD_CLOFORK: c_int = 2;

pub const FIL_ATTACH: c_int = 0x1;
pub const FIL_DETACH: c_int = 0x2;
pub const FIL_LIST: c_int = 0x3;
pub const FILNAME_MAX: c_int = 32;
pub const FILF_PROG: c_int = 0x1;
pub const FILF_AUTO: c_int = 0x2;
pub const FILF_BYPASS: c_int = 0x4;
pub const SOL_FILTER: c_int = 0xfffc;

pub const MADV_PURGE: c_int = 9;

pub const POSIX_FADV_NORMAL: c_int = 0;
pub const POSIX_FADV_RANDOM: c_int = 1;
pub const POSIX_FADV_SEQUENTIAL: c_int = 2;
pub const POSIX_FADV_WILLNEED: c_int = 3;
pub const POSIX_FADV_DONTNEED: c_int = 4;
pub const POSIX_FADV_NOREUSE: c_int = 5;

pub const POSIX_SPAWN_SETSID: c_short = 0x40;

pub const SIGINFO: c_int = 41;

pub const O_DIRECT: c_int = 0x2000000;
pub const O_CLOFORK: c_int = 0x4000000;

pub const MSG_CMSG_CLOEXEC: c_int = 0x1000;
pub const MSG_CMSG_CLOFORK: c_int = 0x2000;

pub const PBIND_HARD: crate::processorid_t = -3;
pub const PBIND_SOFT: crate::processorid_t = -4;

pub const PS_SYSTEM: c_int = 1;

pub const MAP_FILE: c_int = 0;

pub const MAP_32BIT: c_int = 0x80;

pub const AF_NCA: c_int = 28;

pub const PF_NCA: c_int = AF_NCA;

pub const LOCK_SH: c_int = 1;
pub const LOCK_EX: c_int = 2;
pub const LOCK_NB: c_int = 4;
pub const LOCK_UN: c_int = 8;

pub const _PC_LAST: c_int = 101;

pub const VSTATUS: usize = 16;
pub const VERASE2: usize = 17;

pub const EPOLLIN: c_int = 0x1;
pub const EPOLLPRI: c_int = 0x2;
pub const EPOLLOUT: c_int = 0x4;
pub const EPOLLRDNORM: c_int = 0x40;
pub const EPOLLRDBAND: c_int = 0x80;
pub const EPOLLWRNORM: c_int = 0x100;
pub const EPOLLWRBAND: c_int = 0x200;
pub const EPOLLMSG: c_int = 0x400;
pub const EPOLLERR: c_int = 0x8;
pub const EPOLLHUP: c_int = 0x10;
pub const EPOLLET: c_int = 0x80000000;
pub const EPOLLRDHUP: c_int = 0x2000;
pub const EPOLLONESHOT: c_int = 0x40000000;
pub const EPOLLWAKEUP: c_int = 0x20000000;
pub const EPOLLEXCLUSIVE: c_int = 0x10000000;
pub const EPOLL_CLOEXEC: c_int = 0x80000;
pub const EPOLL_CTL_ADD: c_int = 1;
pub const EPOLL_CTL_MOD: c_int = 3;
pub const EPOLL_CTL_DEL: c_int = 2;

pub const PRIV_USER: c_uint = PRIV_DEBUG
    | NET_MAC_AWARE
    | NET_MAC_AWARE_INHERIT
    | PRIV_XPOLICY
    | PRIV_AWARE_RESET
    | PRIV_PFEXEC;

pub const LGRP_RSRC_COUNT: crate::lgrp_rsrc_t = 2;
pub const LGRP_RSRC_CPU: crate::lgrp_rsrc_t = 0;
pub const LGRP_RSRC_MEM: crate::lgrp_rsrc_t = 1;

pub const P_DISABLED: c_int = 0x008;

pub const AT_SUN_HWCAP2: c_uint = 2023;
pub const AT_SUN_FPTYPE: c_uint = 2027;

pub const B1000000: crate::speed_t = 24;
pub const B1152000: crate::speed_t = 25;
pub const B1500000: crate::speed_t = 26;
pub const B2000000: crate::speed_t = 27;
pub const B2500000: crate::speed_t = 28;
pub const B3000000: crate::speed_t = 29;
pub const B3500000: crate::speed_t = 30;
pub const B4000000: crate::speed_t = 31;

// sys/systeminfo.h
pub const SI_ADDRESS_WIDTH: c_int = 520;

// sys/timerfd.h
pub const TFD_CLOEXEC: i32 = 0o2000000;
pub const TFD_NONBLOCK: i32 = 0o4000;
pub const TFD_TIMER_ABSTIME: i32 = 1 << 0;
pub const TFD_TIMER_CANCEL_ON_SET: i32 = 1 << 1;

extern "C" {
    pub fn eventfd(init: c_uint, flags: c_int) -> c_int;

    pub fn epoll_pwait(
        epfd: c_int,
        events: *mut crate::epoll_event,
        maxevents: c_int,
        timeout: c_int,
        sigmask: *const crate::sigset_t,
    ) -> c_int;
    pub fn epoll_create(size: c_int) -> c_int;
    pub fn epoll_create1(flags: c_int) -> c_int;
    pub fn epoll_wait(
        epfd: c_int,
        events: *mut crate::epoll_event,
        maxevents: c_int,
        timeout: c_int,
    ) -> c_int;
    pub fn epoll_ctl(epfd: c_int, op: c_int, fd: c_int, event: *mut crate::epoll_event) -> c_int;

    pub fn mincore(addr: crate::caddr_t, len: size_t, vec: *mut c_char) -> c_int;

    pub fn pset_bind_lwp(
        pset: crate::psetid_t,
        id: crate::id_t,
        pid: crate::pid_t,
        opset: *mut crate::psetid_t,
    ) -> c_int;
    pub fn pset_getloadavg(pset: crate::psetid_t, load: *mut c_double, num: c_int) -> c_int;

    pub fn pthread_attr_get_np(thread: crate::pthread_t, attr: *mut crate::pthread_attr_t)
        -> c_int;
    pub fn pthread_attr_getstackaddr(
        attr: *const crate::pthread_attr_t,
        stackaddr: *mut *mut c_void,
    ) -> c_int;
    pub fn pthread_attr_setstack(
        attr: *mut crate::pthread_attr_t,
        stackaddr: *mut c_void,
        stacksize: size_t,
    ) -> c_int;
    pub fn pthread_attr_setstackaddr(
        attr: *mut crate::pthread_attr_t,
        stackaddr: *mut c_void,
    ) -> c_int;

    pub fn posix_fadvise(fd: c_int, offset: off_t, len: off_t, advice: c_int) -> c_int;
    pub fn preadv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off_t) -> ssize_t;
    pub fn pwritev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off_t) -> ssize_t;
    pub fn getpagesizes2(pagesize: *mut size_t, nelem: c_int) -> c_int;

    pub fn posix_spawn_file_actions_addfchdir_np(
        file_actions: *mut crate::posix_spawn_file_actions_t,
        fd: c_int,
    ) -> c_int;

    pub fn ptsname_r(fildes: c_int, name: *mut c_char, namelen: size_t) -> c_int;

    pub fn syncfs(fd: c_int) -> c_int;

    pub fn strcasecmp_l(s1: *const c_char, s2: *const c_char, loc: crate::locale_t) -> c_int;
    pub fn strncasecmp_l(
        s1: *const c_char,
        s2: *const c_char,
        n: size_t,
        loc: crate::locale_t,
    ) -> c_int;

    pub fn timerfd_create(clockid: c_int, flags: c_int) -> c_int;
    pub fn timerfd_gettime(fd: c_int, curr_value: *mut crate::itimerspec) -> c_int;
    pub fn timerfd_settime(
        fd: c_int,
        flags: c_int,
        new_value: *const crate::itimerspec,
        old_value: *mut crate::itimerspec,
    ) -> c_int;
}
