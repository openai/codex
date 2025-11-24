use crate::off_t;
use crate::prelude::*;

// Define lock_data_instrumented as an empty enum
missing! {
    #[cfg_attr(feature = "extra_traits", derive(Debug))]
    pub enum lock_data_instrumented {}
}

s! {
    pub struct sigset_t {
        pub ss_set: [c_ulong; 4],
    }

    pub struct fd_set {
        pub fds_bits: [c_long; 1024],
    }

    pub struct flock {
        pub l_type: c_short,
        pub l_whence: c_short,
        pub l_sysid: c_uint,
        pub l_pid: crate::pid_t,
        pub l_vfs: c_int,
        pub l_start: off_t,
        pub l_len: off_t,
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
        pub f_basetype: [c_char; 16],
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
        pub f_fstr: [c_char; 32],
        pub f_filler: [c_ulong; 16],
    }

    pub struct pthread_rwlock_t {
        __rw_word: [c_long; 10],
    }

    pub struct pthread_cond_t {
        __cv_word: [c_long; 6],
    }

    pub struct pthread_mutex_t {
        __mt_word: [c_long; 8],
    }

    pub struct pthread_once_t {
        __on_word: [c_long; 9],
    }

    pub struct stat {
        pub st_dev: crate::dev_t,
        pub st_ino: crate::ino_t,
        pub st_mode: crate::mode_t,
        pub st_nlink: crate::nlink_t,
        pub st_flag: c_ushort,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        pub st_ssize: c_int,
        pub st_atim: crate::timespec,
        pub st_mtim: crate::timespec,
        pub st_ctim: crate::timespec,
        pub st_blksize: crate::blksize_t,
        pub st_blocks: crate::blkcnt_t,
        pub st_vfstype: c_int,
        pub st_vfs: c_uint,
        pub st_type: c_uint,
        pub st_gen: c_uint,
        pub st_reserved: [c_uint; 9],
        pub st_padto_ll: c_uint,
        pub st_size: off_t,
    }

    pub struct statfs {
        pub f_version: c_int,
        pub f_type: c_int,
        pub f_bsize: c_ulong,
        pub f_blocks: crate::fsblkcnt_t,
        pub f_bfree: crate::fsblkcnt_t,
        pub f_bavail: crate::fsblkcnt_t,
        pub f_files: crate::fsblkcnt_t,
        pub f_ffree: crate::fsblkcnt_t,
        pub f_fsid: crate::fsid64_t,
        pub f_vfstype: c_int,
        pub f_fsize: c_ulong,
        pub f_vfsnumber: c_int,
        pub f_vfsoff: c_int,
        pub f_vfslen: c_int,
        pub f_vfsvers: c_int,
        pub f_fname: [c_char; 32],
        pub f_fpack: [c_char; 32],
        pub f_name_max: c_int,
    }

    pub struct aiocb {
        pub aio_lio_opcode: c_int,
        pub aio_fildes: c_int,
        pub aio_word1: c_int,
        pub aio_offset: off_t,
        pub aio_buf: *mut c_void,
        pub aio_return: ssize_t,
        pub aio_errno: c_int,
        pub aio_nbytes: size_t,
        pub aio_reqprio: c_int,
        pub aio_sigevent: crate::sigevent,
        pub aio_word2: c_int,
        pub aio_fp: c_int,
        pub aio_handle: *mut aiocb,
        pub aio_reserved: [c_uint; 2],
        pub aio_sigev_tid: c_long,
    }

    pub struct __vmxreg_t {
        __v: [c_uint; 4],
    }

    pub struct __vmx_context_t {
        pub __vr: [crate::__vmxreg_t; 32],
        pub __pad1: [c_uint; 3],
        pub __vscr: c_uint,
        pub __vrsave: c_uint,
        pub __pad2: [c_uint; 3],
    }

    pub struct __vsx_context_t {
        pub __vsr_dw1: [c_ulonglong; 32],
    }

    pub struct __tm_context_t {
        pub vmx: crate::__vmx_context_t,
        pub vsx: crate::__vsx_context_t,
        pub gpr: [c_ulonglong; 32],
        pub lr: c_ulonglong,
        pub ctr: c_ulonglong,
        pub cr: c_uint,
        pub xer: c_uint,
        pub amr: c_ulonglong,
        pub texasr: c_ulonglong,
        pub tfiar: c_ulonglong,
        pub tfhar: c_ulonglong,
        pub ppr: c_ulonglong,
        pub dscr: c_ulonglong,
        pub tar: c_ulonglong,
        pub fpscr: c_uint,
        pub fpscrx: c_uint,
        pub fpr: [fpreg_t; 32],
        pub tmcontext: c_char,
        pub tmstate: c_char,
        pub prevowner: c_char,
        pub pad: [c_char; 5],
    }

    pub struct __context64 {
        pub gpr: [c_ulonglong; 32],
        pub msr: c_ulonglong,
        pub iar: c_ulonglong,
        pub lr: c_ulonglong,
        pub ctr: c_ulonglong,
        pub cr: c_uint,
        pub xer: c_uint,
        pub fpscr: c_uint,
        pub fpscrx: c_uint,
        pub except: [c_ulonglong; 1],
        pub fpr: [fpreg_t; 32],
        pub fpeu: c_char,
        pub fpinfo: c_char,
        pub fpscr24_31: c_char,
        pub pad: [c_char; 1],
        pub excp_type: c_int,
    }

    pub struct mcontext_t {
        pub jmp_context: __context64,
    }

    pub struct __extctx_t {
        pub __flags: c_uint,
        pub __rsvd1: [c_uint; 3],
        pub __vmx: crate::__vmx_context_t,
        pub __ukeys: [c_uint; 2],
        pub __vsx: crate::__vsx_context_t,
        pub __tm: crate::__tm_context_t,
        pub __reserved: [c_char; 1860],
        pub __extctx_magic: c_int,
    }

    pub struct ucontext_t {
        pub __sc_onstack: c_int,
        pub uc_sigmask: crate::sigset_t,
        pub __sc_uerror: c_int,
        pub uc_mcontext: crate::mcontext_t,
        pub uc_link: *mut ucontext_t,
        pub uc_stack: crate::stack_t,
        pub __extctx: *mut crate::__extctx_t,
        pub __extctx_magic: c_int,
        pub __pad: [c_int; 1],
    }

    pub struct utmpx {
        pub ut_user: [c_char; 256],
        pub ut_id: [c_char; 14],
        pub ut_line: [c_char; 64],
        pub ut_pid: crate::pid_t,
        pub ut_type: c_short,
        pub ut_tv: crate::timeval,
        pub ut_host: [c_char; 256],
        pub __dbl_word_pad: c_int,
        pub __reservedA: [c_int; 2],
        pub __reservedV: [c_int; 6],
    }

    pub struct pthread_spinlock_t {
        pub __sp_word: [c_long; 3],
    }

    pub struct pthread_barrier_t {
        pub __br_word: [c_long; 5],
    }

    pub struct msqid_ds {
        pub msg_perm: crate::ipc_perm,
        pub msg_first: c_uint,
        pub msg_last: c_uint,
        pub msg_cbytes: c_uint,
        pub msg_qnum: c_uint,
        pub msg_qbytes: c_ulong,
        pub msg_lspid: crate::pid_t,
        pub msg_lrpid: crate::pid_t,
        pub msg_stime: crate::time_t,
        pub msg_rtime: crate::time_t,
        pub msg_ctime: crate::time_t,
        pub msg_rwait: c_int,
        pub msg_wwait: c_int,
        pub msg_reqevents: c_ushort,
    }
}

s_no_extra_traits! {
    pub struct siginfo_t {
        pub si_signo: c_int,
        pub si_errno: c_int,
        pub si_code: c_int,
        pub si_pid: crate::pid_t,
        pub si_uid: crate::uid_t,
        pub si_status: c_int,
        pub si_addr: *mut c_void,
        pub si_band: c_long,
        pub si_value: crate::sigval,
        pub __si_flags: c_int,
        pub __pad: [c_int; 3],
    }

    pub union _kernel_simple_lock {
        pub _slock: c_long,
        pub _slockp: *mut lock_data_instrumented,
    }

    pub struct fileops_t {
        pub fo_rw: Option<
            extern "C" fn(
                file: *mut file,
                rw: crate::uio_rw,
                io: *mut c_void,
                ext: c_long,
                secattr: *mut c_void,
            ) -> c_int,
        >,
        pub fo_ioctl: Option<
            extern "C" fn(
                file: *mut file,
                a: c_long,
                b: crate::caddr_t,
                c: c_long,
                d: c_long,
            ) -> c_int,
        >,
        pub fo_select: Option<
            extern "C" fn(file: *mut file, a: c_int, b: *mut c_ushort, c: extern "C" fn()) -> c_int,
        >,
        pub fo_close: Option<extern "C" fn(file: *mut file) -> c_int>,
        pub fo_fstat: Option<extern "C" fn(file: *mut file, sstat: *mut crate::stat) -> c_int>,
    }

    pub struct file {
        pub f_flag: c_long,
        pub f_count: c_int,
        pub f_options: c_short,
        pub f_type: c_short,
        // Should be pointer to 'vnode'
        pub f_data: *mut c_void,
        pub f_offset: c_longlong,
        pub f_dir_off: c_long,
        // Should be pointer to 'cred'
        pub f_cred: *mut c_void,
        pub f_lock: _kernel_simple_lock,
        pub f_offset_lock: _kernel_simple_lock,
        pub f_vinfo: crate::caddr_t,
        pub f_ops: *mut fileops_t,
        pub f_parentp: crate::caddr_t,
        pub f_fnamep: crate::caddr_t,
        pub f_fdata: [c_char; 160],
    }

    pub union __ld_info_file {
        pub _ldinfo_fd: c_int,
        pub _ldinfo_fp: *mut file,
        pub _core_offset: c_long,
    }

    pub struct ld_info {
        pub ldinfo_next: c_uint,
        pub ldinfo_flags: c_uint,
        pub _file: __ld_info_file,
        pub ldinfo_textorg: *mut c_void,
        pub ldinfo_textsize: c_ulong,
        pub ldinfo_dataorg: *mut c_void,
        pub ldinfo_datasize: c_ulong,
        pub ldinfo_filename: [c_char; 2],
    }

    pub union __pollfd_ext_u {
        pub addr: *mut c_void,
        pub data32: u32,
        pub data: u64,
    }

    pub struct pollfd_ext {
        pub fd: c_int,
        pub events: c_short,
        pub revents: c_short,
        pub data: __pollfd_ext_u,
    }

    pub struct fpreg_t {
        pub d: c_double,
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

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for siginfo_t {
            fn eq(&self, other: &siginfo_t) -> bool {
                self.si_signo == other.si_signo
                    && self.si_errno == other.si_errno
                    && self.si_code == other.si_code
                    && self.si_pid == other.si_pid
                    && self.si_uid == other.si_uid
                    && self.si_status == other.si_status
                    && self.si_addr == other.si_addr
                    && self.si_band == other.si_band
                    && self.__si_flags == other.__si_flags
                    && self.si_value == other.si_value
            }
        }
        impl Eq for siginfo_t {}
        impl hash::Hash for siginfo_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.si_signo.hash(state);
                self.si_errno.hash(state);
                self.si_code.hash(state);
                self.si_pid.hash(state);
                self.si_uid.hash(state);
                self.si_status.hash(state);
                self.si_addr.hash(state);
                self.si_band.hash(state);
                self.si_value.hash(state);
                self.__si_flags.hash(state);
            }
        }

        impl PartialEq for __pollfd_ext_u {
            fn eq(&self, other: &__pollfd_ext_u) -> bool {
                unsafe {
                    self.addr == other.addr
                        && self.data32 == other.data32
                        && self.data == other.data
                }
            }
        }
        impl Eq for __pollfd_ext_u {}
        impl hash::Hash for __pollfd_ext_u {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    self.addr.hash(state);
                    self.data.hash(state);
                    self.data32.hash(state);
                }
            }
        }

        impl PartialEq for pollfd_ext {
            fn eq(&self, other: &pollfd_ext) -> bool {
                self.fd == other.fd
                    && self.events == other.events
                    && self.revents == other.revents
                    && self.data == other.data
            }
        }
        impl Eq for pollfd_ext {}
        impl hash::Hash for pollfd_ext {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.fd.hash(state);
                self.events.hash(state);
                self.revents.hash(state);
                self.data.hash(state);
            }
        }
        impl PartialEq for fpreg_t {
            fn eq(&self, other: &fpreg_t) -> bool {
                self.d == other.d
            }
        }

        impl Eq for fpreg_t {}

        impl hash::Hash for fpreg_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                let d: u64 = unsafe { mem::transmute(self.d) };
                d.hash(state);
            }
        }
    }
}

pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
    __mt_word: [0, 2, 0, 0, 0, 0, 0, 0],
};
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    __cv_word: [0, 0, 0, 0, 2, 0],
};
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = pthread_rwlock_t {
    __rw_word: [2, 0, 0, 0, 0, 0, 0, 0, 0, 0],
};

pub const PTHREAD_ONCE_INIT: pthread_once_t = pthread_once_t {
    __on_word: [0, 0, 0, 0, 0, 2, 0, 0, 0],
};

pub const RLIM_INFINITY: c_ulong = 0x7fffffffffffffff;

extern "C" {
    pub fn getsystemcfg(label: c_int) -> c_ulong;
}
