use crate::prelude::*;
use crate::{
    exit_status, off_t, termios, NET_MAC_AWARE, NET_MAC_AWARE_INHERIT, PRIV_AWARE_RESET,
    PRIV_DEBUG, PRIV_PFEXEC, PRIV_XPOLICY,
};

pub type door_attr_t = c_uint;
pub type door_id_t = c_ulonglong;
pub type lgrp_affinity_t = c_uint;

e! {
    #[repr(u32)]
    pub enum lgrp_rsrc_t {
        LGRP_RSRC_CPU = 0,
        LGRP_RSRC_MEM = 1,
        LGRP_RSRC_TYPES = 2,
    }
}

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
        pub aio_state: c_char,
        pub aio_returned: c_char,
        pub aio__pad1: [c_char; 2],
        pub aio_flags: c_int,
    }

    pub struct shmid_ds {
        pub shm_perm: crate::ipc_perm,
        pub shm_segsz: size_t,
        pub shm_flags: crate::uintptr_t,
        pub shm_lkcnt: c_ushort,
        pub shm_lpid: crate::pid_t,
        pub shm_cpid: crate::pid_t,
        pub shm_nattch: crate::shmatt_t,
        pub shm_cnattch: c_ulong,
        pub shm_atime: crate::time_t,
        pub shm_dtime: crate::time_t,
        pub shm_ctime: crate::time_t,
        pub shm_amp: *mut c_void,
        pub shm_gransize: u64,
        pub shm_allocated: u64,
        pub shm_pad4: [i64; 1],
    }

    pub struct xrs_t {
        pub xrs_id: c_ulong,
        pub xrs_ptr: *mut c_char,
    }
}

s_no_extra_traits! {
    #[repr(packed)]
    pub struct door_desc_t__d_data__d_desc {
        pub d_descriptor: c_int,
        pub d_id: crate::door_id_t,
    }

    pub union door_desc_t__d_data {
        pub d_desc: door_desc_t__d_data__d_desc,
        d_resv: [c_int; 5], /* Check out /usr/include/sys/door.h */
    }

    pub struct door_desc_t {
        pub d_attributes: door_attr_t,
        pub d_data: door_desc_t__d_data,
    }

    pub struct door_arg_t {
        pub data_ptr: *const c_char,
        pub data_size: size_t,
        pub desc_ptr: *const door_desc_t,
        pub dec_num: c_uint,
        pub rbuf: *const c_char,
        pub rsize: size_t,
    }

    pub struct utmpx {
        pub ut_user: [c_char; _UTMP_USER_LEN],
        pub ut_id: [c_char; _UTMP_ID_LEN],
        pub ut_line: [c_char; _UTMP_LINE_LEN],
        pub ut_pid: crate::pid_t,
        pub ut_type: c_short,
        pub ut_exit: exit_status,
        pub ut_tv: crate::timeval,
        pub ut_session: c_int,
        pub pad: [c_int; 5],
        pub ut_syslen: c_short,
        pub ut_host: [c_char; 257],
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
                    && self.pad == other.pad
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
                self.pad.hash(state);
            }
        }
    }
}

// FIXME(solaris): O_DIRECT and SIGINFO are NOT available on Solaris.
// But in past they were defined here and thus other crates expected them.
// Latest version v0.29.0 of Nix crate still expects this. Since last
// version of Nix crate is almost one year ago let's define these two
// temporarily before new Nix version is released.
pub const O_DIRECT: c_int = 0x2000000;
pub const SIGINFO: c_int = 41;

pub const _UTMP_USER_LEN: usize = 32;
pub const _UTMP_LINE_LEN: usize = 32;
pub const _UTMP_ID_LEN: usize = 4;

pub const PORT_SOURCE_POSTWAIT: c_int = 8;
pub const PORT_SOURCE_SIGNAL: c_int = 9;

pub const AF_LOCAL: c_int = 1; // AF_UNIX
pub const AF_FILE: c_int = 1; // AF_UNIX

pub const TCP_KEEPIDLE: c_int = 0x1d;
pub const TCP_KEEPINTVL: c_int = 0x1e;
pub const TCP_KEEPCNT: c_int = 0x1f;

pub const F_DUPFD_CLOEXEC: c_int = 47;
pub const F_DUPFD_CLOFORK: c_int = 49;
pub const F_DUP2FD_CLOEXEC: c_int = 48;
pub const F_DUP2FD_CLOFORK: c_int = 50;

pub const _PC_LAST: c_int = 102;

pub const PRIV_PROC_SENSITIVE: c_uint = 0x0008;
pub const PRIV_PFEXEC_AUTH: c_uint = 0x0200;
pub const PRIV_PROC_TPD: c_uint = 0x0400;
pub const PRIV_TPD_UNSAFE: c_uint = 0x0800;
pub const PRIV_PROC_TPD_RESET: c_uint = 0x1000;
pub const PRIV_TPD_KILLABLE: c_uint = 0x2000;

pub const POSIX_SPAWN_SETSID: c_short = 0x400;

pub const PRIV_USER: c_uint = PRIV_DEBUG
    | PRIV_PROC_SENSITIVE
    | NET_MAC_AWARE
    | NET_MAC_AWARE_INHERIT
    | PRIV_XPOLICY
    | PRIV_AWARE_RESET
    | PRIV_PFEXEC
    | PRIV_PFEXEC_AUTH
    | PRIV_PROC_TPD
    | PRIV_TPD_UNSAFE
    | PRIV_TPD_KILLABLE
    | PRIV_PROC_TPD_RESET;

extern "C" {
    // DIFF(main): changed to `*const *mut` in e77f551de9
    pub fn fexecve(fd: c_int, argv: *const *const c_char, envp: *const *const c_char) -> c_int;

    pub fn mincore(addr: *mut c_void, len: size_t, vec: *mut c_char) -> c_int;

    pub fn door_call(d: c_int, params: *mut door_arg_t) -> c_int;
    pub fn door_return(
        data_ptr: *mut c_char,
        data_size: size_t,
        desc_ptr: *mut door_desc_t,
        num_desc: c_uint,
    ) -> c_int;
    pub fn door_create(
        server_procedure: extern "C" fn(
            cookie: *mut c_void,
            argp: *mut c_char,
            arg_size: size_t,
            dp: *mut door_desc_t,
            n_desc: c_uint,
        ),
        cookie: *mut c_void,
        attributes: door_attr_t,
    ) -> c_int;

    pub fn fattach(fildes: c_int, path: *const c_char) -> c_int;

    pub fn pthread_getattr_np(thread: crate::pthread_t, attr: *mut crate::pthread_attr_t) -> c_int;

    pub fn euidaccess(path: *const c_char, amode: c_int) -> c_int;

    pub fn openpty(
        amain: *mut c_int,
        asubord: *mut c_int,
        name: *mut c_char,
        termp: *mut termios,
        winp: *mut crate::winsize,
    ) -> c_int;

    pub fn forkpty(
        amain: *mut c_int,
        name: *mut c_char,
        termp: *mut termios,
        winp: *mut crate::winsize,
    ) -> crate::pid_t;
}
