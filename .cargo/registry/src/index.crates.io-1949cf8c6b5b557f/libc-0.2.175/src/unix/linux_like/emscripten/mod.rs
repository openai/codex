use crate::prelude::*;

pub type wchar_t = i32;
pub type useconds_t = u32;
pub type dev_t = u32;
pub type socklen_t = u32;
pub type pthread_t = c_ulong;
pub type mode_t = u32;
pub type shmatt_t = c_ulong;
pub type mqd_t = c_int;
pub type msgqnum_t = c_ulong;
pub type msglen_t = c_ulong;
pub type nfds_t = c_ulong;
pub type nl_item = c_int;
pub type idtype_t = c_uint;
pub type loff_t = i64;
pub type pthread_key_t = c_uint;

pub type clock_t = c_long;
pub type time_t = i64;
pub type suseconds_t = c_long;
pub type ino_t = u64;
pub type off_t = i64;
pub type blkcnt_t = i32;

pub type blksize_t = c_long;
pub type fsblkcnt_t = u32;
pub type fsfilcnt_t = u32;
pub type rlim_t = u64;
pub type nlink_t = u32;

pub type ino64_t = crate::ino_t;
pub type off64_t = off_t;
pub type blkcnt64_t = crate::blkcnt_t;
pub type rlim64_t = crate::rlim_t;

pub type rlimit64 = crate::rlimit;
pub type flock64 = crate::flock;
pub type stat64 = crate::stat;
pub type statfs64 = crate::statfs;
pub type statvfs64 = crate::statvfs;
pub type dirent64 = crate::dirent;

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum fpos64_t {} // FIXME(emscripten): fill this out with a struct
impl Copy for fpos64_t {}
impl Clone for fpos64_t {
    fn clone(&self) -> fpos64_t {
        *self
    }
}

s! {
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

    pub struct passwd {
        pub pw_name: *mut c_char,
        pub pw_passwd: *mut c_char,
        pub pw_uid: crate::uid_t,
        pub pw_gid: crate::gid_t,
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
        __f_unused: c_int,
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
        __f_spare: [c_int; 6],
    }

    pub struct signalfd_siginfo {
        pub ssi_signo: u32,
        pub ssi_errno: i32,
        pub ssi_code: i32,
        pub ssi_pid: u32,
        pub ssi_uid: u32,
        pub ssi_fd: i32,
        pub ssi_tid: u32,
        pub ssi_band: u32,
        pub ssi_overrun: u32,
        pub ssi_trapno: u32,
        pub ssi_status: i32,
        pub ssi_int: i32,
        pub ssi_ptr: u64,
        pub ssi_utime: u64,
        pub ssi_stime: u64,
        pub ssi_addr: u64,
        pub ssi_addr_lsb: u16,
        _pad2: u16,
        pub ssi_syscall: i32,
        pub ssi_call_addr: u64,
        pub ssi_arch: u32,
        _pad: [u8; 28],
    }

    pub struct fsid_t {
        __val: [c_int; 2],
    }

    pub struct cpu_set_t {
        bits: [u32; 32],
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

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct sigaction {
        pub sa_sigaction: crate::sighandler_t,
        pub sa_mask: crate::sigset_t,
        pub sa_flags: c_int,
        pub sa_restorer: Option<extern "C" fn()>,
    }

    pub struct ipc_perm {
        pub __ipc_perm_key: crate::key_t,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub cuid: crate::uid_t,
        pub cgid: crate::gid_t,
        pub mode: mode_t,
        pub __seq: c_int,
        __unused1: c_long,
        __unused2: c_long,
    }

    pub struct termios {
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_line: crate::cc_t,
        pub c_cc: [crate::cc_t; crate::NCCS],
        pub __c_ispeed: crate::speed_t,
        pub __c_ospeed: crate::speed_t,
    }

    pub struct flock {
        pub l_type: c_short,
        pub l_whence: c_short,
        pub l_start: off_t,
        pub l_len: off_t,
        pub l_pid: crate::pid_t,
    }

    pub struct pthread_attr_t {
        __size: [u32; 11],
    }

    pub struct sigset_t {
        __val: [c_ulong; 32],
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

    pub struct sem_t {
        __val: [c_int; 4],
    }
    pub struct stat {
        pub st_dev: crate::dev_t,
        #[cfg(emscripten_old_stat_abi)]
        __st_dev_padding: c_int,
        #[cfg(emscripten_old_stat_abi)]
        __st_ino_truncated: c_long,
        pub st_mode: mode_t,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        #[cfg(emscripten_old_stat_abi)]
        __st_rdev_padding: c_int,
        pub st_size: off_t,
        pub st_blksize: crate::blksize_t,
        pub st_blocks: crate::blkcnt_t,
        pub st_atime: crate::time_t,
        pub st_atime_nsec: c_long,
        pub st_mtime: crate::time_t,
        pub st_mtime_nsec: c_long,
        pub st_ctime: crate::time_t,
        pub st_ctime_nsec: c_long,
        pub st_ino: crate::ino_t,
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_flags: c_int,
        pub ss_size: size_t,
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
        pub msg_stime: crate::time_t,
        pub msg_rtime: crate::time_t,
        pub msg_ctime: crate::time_t,
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
        pub f_blocks: crate::fsblkcnt_t,
        pub f_bfree: crate::fsblkcnt_t,
        pub f_bavail: crate::fsblkcnt_t,
        pub f_files: crate::fsfilcnt_t,
        pub f_ffree: crate::fsfilcnt_t,
        pub f_fsid: crate::fsid_t,
        pub f_namelen: c_ulong,
        pub f_frsize: c_ulong,
        pub f_flags: c_ulong,
        pub f_spare: [c_ulong; 4],
    }

    pub struct siginfo_t {
        pub si_signo: c_int,
        pub si_errno: c_int,
        pub si_code: c_int,
        pub _pad: [c_int; 29],
        _align: [usize; 0],
    }

    pub struct arpd_request {
        pub req: c_ushort,
        pub ip: u32,
        pub dev: c_ulong,
        pub stamp: c_ulong,
        pub updated: c_ulong,
        pub ha: [c_uchar; crate::MAX_ADDR_LEN],
    }

    #[repr(align(4))]
    pub struct pthread_mutex_t {
        size: [u8; crate::__SIZEOF_PTHREAD_MUTEX_T],
    }

    #[repr(align(4))]
    pub struct pthread_rwlock_t {
        size: [u8; crate::__SIZEOF_PTHREAD_RWLOCK_T],
    }

    #[repr(align(4))]
    pub struct pthread_mutexattr_t {
        size: [u8; crate::__SIZEOF_PTHREAD_MUTEXATTR_T],
    }

    #[repr(align(4))]
    pub struct pthread_rwlockattr_t {
        size: [u8; crate::__SIZEOF_PTHREAD_RWLOCKATTR_T],
    }

    #[repr(align(4))]
    pub struct pthread_condattr_t {
        size: [u8; crate::__SIZEOF_PTHREAD_CONDATTR_T],
    }
}

s_no_extra_traits! {
    pub struct dirent {
        pub d_ino: crate::ino_t,
        pub d_off: off_t,
        pub d_reclen: c_ushort,
        pub d_type: c_uchar,
        pub d_name: [c_char; 256],
    }

    pub struct sysinfo {
        pub uptime: c_ulong,
        pub loads: [c_ulong; 3],
        pub totalram: c_ulong,
        pub freeram: c_ulong,
        pub sharedram: c_ulong,
        pub bufferram: c_ulong,
        pub totalswap: c_ulong,
        pub freeswap: c_ulong,
        pub procs: c_ushort,
        pub pad: c_ushort,
        pub totalhigh: c_ulong,
        pub freehigh: c_ulong,
        pub mem_unit: c_uint,
        pub __reserved: [c_char; 256],
    }

    pub struct mq_attr {
        pub mq_flags: c_long,
        pub mq_maxmsg: c_long,
        pub mq_msgsize: c_long,
        pub mq_curmsgs: c_long,
        pad: [c_long; 4],
    }

    #[cfg_attr(target_pointer_width = "32", repr(align(4)))]
    #[cfg_attr(target_pointer_width = "64", repr(align(8)))]
    pub struct pthread_cond_t {
        size: [u8; crate::__SIZEOF_PTHREAD_COND_T],
    }

    #[repr(align(8))]
    pub struct max_align_t {
        priv_: [f64; 3],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for dirent {
            fn eq(&self, other: &dirent) -> bool {
                self.d_ino == other.d_ino
                    && self.d_off == other.d_off
                    && self.d_reclen == other.d_reclen
                    && self.d_type == other.d_type
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
                self.d_ino.hash(state);
                self.d_off.hash(state);
                self.d_reclen.hash(state);
                self.d_type.hash(state);
                self.d_name.hash(state);
            }
        }

        impl PartialEq for sysinfo {
            fn eq(&self, other: &sysinfo) -> bool {
                self.uptime == other.uptime
                    && self.loads == other.loads
                    && self.totalram == other.totalram
                    && self.freeram == other.freeram
                    && self.sharedram == other.sharedram
                    && self.bufferram == other.bufferram
                    && self.totalswap == other.totalswap
                    && self.freeswap == other.freeswap
                    && self.procs == other.procs
                    && self.pad == other.pad
                    && self.totalhigh == other.totalhigh
                    && self.freehigh == other.freehigh
                    && self.mem_unit == other.mem_unit
                    && self
                        .__reserved
                        .iter()
                        .zip(other.__reserved.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for sysinfo {}
        impl hash::Hash for sysinfo {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.uptime.hash(state);
                self.loads.hash(state);
                self.totalram.hash(state);
                self.freeram.hash(state);
                self.sharedram.hash(state);
                self.bufferram.hash(state);
                self.totalswap.hash(state);
                self.freeswap.hash(state);
                self.procs.hash(state);
                self.pad.hash(state);
                self.totalhigh.hash(state);
                self.freehigh.hash(state);
                self.mem_unit.hash(state);
                self.__reserved.hash(state);
            }
        }

        impl PartialEq for mq_attr {
            fn eq(&self, other: &mq_attr) -> bool {
                self.mq_flags == other.mq_flags
                    && self.mq_maxmsg == other.mq_maxmsg
                    && self.mq_msgsize == other.mq_msgsize
                    && self.mq_curmsgs == other.mq_curmsgs
            }
        }
        impl Eq for mq_attr {}
        impl hash::Hash for mq_attr {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.mq_flags.hash(state);
                self.mq_maxmsg.hash(state);
                self.mq_msgsize.hash(state);
                self.mq_curmsgs.hash(state);
            }
        }

        impl PartialEq for pthread_cond_t {
            fn eq(&self, other: &pthread_cond_t) -> bool {
                self.size.iter().zip(other.size.iter()).all(|(a, b)| a == b)
            }
        }
        impl Eq for pthread_cond_t {}
        impl hash::Hash for pthread_cond_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.size.hash(state);
            }
        }
    }
}

pub const MADV_SOFT_OFFLINE: c_int = 101;
pub const MS_NOUSER: c_ulong = 0x80000000;
pub const MS_RMT_MASK: c_ulong = 0x02800051;

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

pub const RUSAGE_THREAD: c_int = 1;
pub const RUSAGE_CHILDREN: c_int = -1;

pub const RADIXCHAR: crate::nl_item = 0x10000;
pub const THOUSEP: crate::nl_item = 0x10001;

pub const YESEXPR: crate::nl_item = 0x50000;
pub const NOEXPR: crate::nl_item = 0x50001;
pub const YESSTR: crate::nl_item = 0x50002;
pub const NOSTR: crate::nl_item = 0x50003;

pub const FILENAME_MAX: c_uint = 4096;
pub const L_tmpnam: c_uint = 20;
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
pub const _SC_PAGE_SIZE: c_int = _SC_PAGESIZE;
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
pub const _SC_EXPR_NEST_MAX: c_int = 42;
pub const _SC_LINE_MAX: c_int = 43;
pub const _SC_RE_DUP_MAX: c_int = 44;
pub const _SC_2_VERSION: c_int = 46;
pub const _SC_2_C_BIND: c_int = 47;
pub const _SC_2_C_DEV: c_int = 48;
pub const _SC_2_FORT_DEV: c_int = 49;
pub const _SC_2_FORT_RUN: c_int = 50;
pub const _SC_2_SW_DEV: c_int = 51;
pub const _SC_2_LOCALEDEF: c_int = 52;
pub const _SC_UIO_MAXIOV: c_int = 60;
pub const _SC_IOV_MAX: c_int = 60;
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
pub const _SC_2_UPE: c_int = 97;
pub const _SC_XOPEN_XPG2: c_int = 98;
pub const _SC_XOPEN_XPG3: c_int = 99;
pub const _SC_XOPEN_XPG4: c_int = 100;
pub const _SC_NZERO: c_int = 109;
pub const _SC_XBS5_ILP32_OFF32: c_int = 125;
pub const _SC_XBS5_ILP32_OFFBIG: c_int = 126;
pub const _SC_XBS5_LP64_OFF64: c_int = 127;
pub const _SC_XBS5_LPBIG_OFFBIG: c_int = 128;
pub const _SC_XOPEN_LEGACY: c_int = 129;
pub const _SC_XOPEN_REALTIME: c_int = 130;
pub const _SC_XOPEN_REALTIME_THREADS: c_int = 131;
pub const _SC_ADVISORY_INFO: c_int = 132;
pub const _SC_BARRIERS: c_int = 133;
pub const _SC_CLOCK_SELECTION: c_int = 137;
pub const _SC_CPUTIME: c_int = 138;
pub const _SC_THREAD_CPUTIME: c_int = 139;
pub const _SC_MONOTONIC_CLOCK: c_int = 149;
pub const _SC_READER_WRITER_LOCKS: c_int = 153;
pub const _SC_SPIN_LOCKS: c_int = 154;
pub const _SC_REGEXP: c_int = 155;
pub const _SC_SHELL: c_int = 157;
pub const _SC_SPAWN: c_int = 159;
pub const _SC_SPORADIC_SERVER: c_int = 160;
pub const _SC_THREAD_SPORADIC_SERVER: c_int = 161;
pub const _SC_TIMEOUTS: c_int = 164;
pub const _SC_TYPED_MEMORY_OBJECTS: c_int = 165;
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

pub const RLIM_SAVED_MAX: crate::rlim_t = RLIM_INFINITY;
pub const RLIM_SAVED_CUR: crate::rlim_t = RLIM_INFINITY;

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

pub const POSIX_MADV_NORMAL: c_int = 0;
pub const POSIX_MADV_RANDOM: c_int = 1;
pub const POSIX_MADV_SEQUENTIAL: c_int = 2;
pub const POSIX_MADV_WILLNEED: c_int = 3;

pub const AT_EACCESS: c_int = 0x200;

pub const S_IEXEC: mode_t = 0o0100;
pub const S_IWRITE: mode_t = 0o0200;
pub const S_IREAD: mode_t = 0o0400;

pub const F_LOCK: c_int = 1;
pub const F_TEST: c_int = 3;
pub const F_TLOCK: c_int = 2;
pub const F_ULOCK: c_int = 0;

pub const ST_RDONLY: c_ulong = 1;
pub const ST_NOSUID: c_ulong = 2;
pub const ST_NODEV: c_ulong = 4;
pub const ST_NOEXEC: c_ulong = 8;
pub const ST_SYNCHRONOUS: c_ulong = 16;
pub const ST_MANDLOCK: c_ulong = 64;
pub const ST_WRITE: c_ulong = 128;
pub const ST_APPEND: c_ulong = 256;
pub const ST_IMMUTABLE: c_ulong = 512;
pub const ST_NOATIME: c_ulong = 1024;
pub const ST_NODIRATIME: c_ulong = 2048;

pub const RTLD_NEXT: *mut c_void = -1i64 as *mut c_void;
pub const RTLD_DEFAULT: *mut c_void = 0i64 as *mut c_void;
pub const RTLD_NODELETE: c_int = 0x1000;
pub const RTLD_NOW: c_int = 0x2;

pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
    size: [0; __SIZEOF_PTHREAD_MUTEX_T],
};
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    size: [0; __SIZEOF_PTHREAD_COND_T],
};
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = pthread_rwlock_t {
    size: [0; __SIZEOF_PTHREAD_RWLOCK_T],
};

pub const PTHREAD_MUTEX_NORMAL: c_int = 0;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 1;
pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 2;
pub const PTHREAD_MUTEX_DEFAULT: c_int = PTHREAD_MUTEX_NORMAL;
pub const PTHREAD_PROCESS_PRIVATE: c_int = 0;
pub const PTHREAD_PROCESS_SHARED: c_int = 1;
pub const __SIZEOF_PTHREAD_COND_T: usize = 48;

pub const SCHED_OTHER: c_int = 0;
pub const SCHED_FIFO: c_int = 1;
pub const SCHED_RR: c_int = 2;
pub const SCHED_BATCH: c_int = 3;
pub const SCHED_IDLE: c_int = 5;

pub const AF_IB: c_int = 27;
pub const AF_MPLS: c_int = 28;
pub const AF_NFC: c_int = 39;
pub const AF_VSOCK: c_int = 40;
pub const PF_IB: c_int = AF_IB;
pub const PF_MPLS: c_int = AF_MPLS;
pub const PF_NFC: c_int = AF_NFC;
pub const PF_VSOCK: c_int = AF_VSOCK;

// System V IPC
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

pub const SHM_R: c_int = 0o400;
pub const SHM_W: c_int = 0o200;

pub const SHM_RDONLY: c_int = 0o10000;
pub const SHM_RND: c_int = 0o20000;
pub const SHM_REMAP: c_int = 0o40000;
pub const SHM_EXEC: c_int = 0o100000;

pub const SHM_LOCK: c_int = 11;
pub const SHM_UNLOCK: c_int = 12;

pub const SHM_HUGETLB: c_int = 0o4000;
pub const SHM_NORESERVE: c_int = 0o10000;

pub const LOG_NFACILITIES: c_int = 24;

pub const SEM_FAILED: *mut crate::sem_t = 0 as *mut sem_t;

pub const AI_PASSIVE: c_int = 0x0001;
pub const AI_CANONNAME: c_int = 0x0002;
pub const AI_NUMERICHOST: c_int = 0x0004;
pub const AI_V4MAPPED: c_int = 0x0008;
pub const AI_ALL: c_int = 0x0010;
pub const AI_ADDRCONFIG: c_int = 0x0020;

pub const AI_NUMERICSERV: c_int = 0x0400;

pub const EAI_BADFLAGS: c_int = -1;
pub const EAI_NONAME: c_int = -2;
pub const EAI_AGAIN: c_int = -3;
pub const EAI_FAIL: c_int = -4;
pub const EAI_FAMILY: c_int = -6;
pub const EAI_SOCKTYPE: c_int = -7;
pub const EAI_SERVICE: c_int = -8;
pub const EAI_MEMORY: c_int = -10;
pub const EAI_OVERFLOW: c_int = -12;

pub const NI_NUMERICHOST: c_int = 1;
pub const NI_NUMERICSERV: c_int = 2;
pub const NI_NOFQDN: c_int = 4;
pub const NI_NAMEREQD: c_int = 8;
pub const NI_DGRAM: c_int = 16;

pub const SYNC_FILE_RANGE_WAIT_BEFORE: c_uint = 1;
pub const SYNC_FILE_RANGE_WRITE: c_uint = 2;
pub const SYNC_FILE_RANGE_WAIT_AFTER: c_uint = 4;

pub const EAI_SYSTEM: c_int = -11;

pub const MREMAP_MAYMOVE: c_int = 1;
pub const MREMAP_FIXED: c_int = 2;

pub const ITIMER_REAL: c_int = 0;
pub const ITIMER_VIRTUAL: c_int = 1;
pub const ITIMER_PROF: c_int = 2;

pub const _POSIX_VDISABLE: crate::cc_t = 0;

pub const FALLOC_FL_KEEP_SIZE: c_int = 0x01;
pub const FALLOC_FL_PUNCH_HOLE: c_int = 0x02;

pub const NCCS: usize = 32;

pub const O_TRUNC: c_int = 512;
pub const O_NOATIME: c_int = 0o1000000;
pub const O_CLOEXEC: c_int = 0x80000;

// Defined as wasi value.
pub const EPERM: c_int = 63;
pub const ENOENT: c_int = 44;
pub const ESRCH: c_int = 71;
pub const EINTR: c_int = 27;
pub const EIO: c_int = 29;
pub const ENXIO: c_int = 60;
pub const E2BIG: c_int = 1;
pub const ENOEXEC: c_int = 45;
pub const EBADF: c_int = 8;
pub const ECHILD: c_int = 12;
pub const EAGAIN: c_int = 6;
pub const ENOMEM: c_int = 48;
pub const EACCES: c_int = 2;
pub const EFAULT: c_int = 21;
pub const ENOTBLK: c_int = 105;
pub const EBUSY: c_int = 10;
pub const EEXIST: c_int = 20;
pub const EXDEV: c_int = 75;
pub const ENODEV: c_int = 43;
pub const ENOTDIR: c_int = 54;
pub const EISDIR: c_int = 31;
pub const EINVAL: c_int = 28;
pub const ENFILE: c_int = 41;
pub const EMFILE: c_int = 33;
pub const ENOTTY: c_int = 59;
pub const ETXTBSY: c_int = 74;
pub const EFBIG: c_int = 22;
pub const ENOSPC: c_int = 51;
pub const ESPIPE: c_int = 70;
pub const EROFS: c_int = 69;
pub const EMLINK: c_int = 34;
pub const EPIPE: c_int = 64;
pub const EDOM: c_int = 18;
pub const ERANGE: c_int = 68;
pub const EWOULDBLOCK: c_int = EAGAIN;
pub const ENOLINK: c_int = 47;
pub const EPROTO: c_int = 65;
pub const EDEADLK: c_int = 16;
pub const EDEADLOCK: c_int = EDEADLK;
pub const ENAMETOOLONG: c_int = 37;
pub const ENOLCK: c_int = 46;
pub const ENOSYS: c_int = 52;
pub const ENOTEMPTY: c_int = 55;
pub const ELOOP: c_int = 32;
pub const ENOMSG: c_int = 49;
pub const EIDRM: c_int = 24;
pub const EMULTIHOP: c_int = 36;
pub const EBADMSG: c_int = 9;
pub const EOVERFLOW: c_int = 61;
pub const EILSEQ: c_int = 25;
pub const ENOTSOCK: c_int = 57;
pub const EDESTADDRREQ: c_int = 17;
pub const EMSGSIZE: c_int = 35;
pub const EPROTOTYPE: c_int = 67;
pub const ENOPROTOOPT: c_int = 50;
pub const EPROTONOSUPPORT: c_int = 66;
pub const EAFNOSUPPORT: c_int = 5;
pub const EADDRINUSE: c_int = 3;
pub const EADDRNOTAVAIL: c_int = 4;
pub const ENETDOWN: c_int = 38;
pub const ENETUNREACH: c_int = 40;
pub const ENETRESET: c_int = 39;
pub const ECONNABORTED: c_int = 13;
pub const ECONNRESET: c_int = 15;
pub const ENOBUFS: c_int = 42;
pub const EISCONN: c_int = 30;
pub const ENOTCONN: c_int = 53;
pub const ETIMEDOUT: c_int = 73;
pub const ECONNREFUSED: c_int = 14;
pub const EHOSTUNREACH: c_int = 23;
pub const EALREADY: c_int = 7;
pub const EINPROGRESS: c_int = 26;
pub const ESTALE: c_int = 72;
pub const EDQUOT: c_int = 19;
pub const ECANCELED: c_int = 11;
pub const EOWNERDEAD: c_int = 62;
pub const ENOTRECOVERABLE: c_int = 56;

pub const ENOSTR: c_int = 100;
pub const EBFONT: c_int = 101;
pub const EBADSLT: c_int = 102;
pub const EBADRQC: c_int = 103;
pub const ENOANO: c_int = 104;
pub const ECHRNG: c_int = 106;
pub const EL3HLT: c_int = 107;
pub const EL3RST: c_int = 108;
pub const ELNRNG: c_int = 109;
pub const EUNATCH: c_int = 110;
pub const ENOCSI: c_int = 111;
pub const EL2HLT: c_int = 112;
pub const EBADE: c_int = 113;
pub const EBADR: c_int = 114;
pub const EXFULL: c_int = 115;
pub const ENODATA: c_int = 116;
pub const ETIME: c_int = 117;
pub const ENOSR: c_int = 118;
pub const ENONET: c_int = 119;
pub const ENOPKG: c_int = 120;
pub const EREMOTE: c_int = 121;
pub const EADV: c_int = 122;
pub const ESRMNT: c_int = 123;
pub const ECOMM: c_int = 124;
pub const EDOTDOT: c_int = 125;
pub const ENOTUNIQ: c_int = 126;
pub const EBADFD: c_int = 127;
pub const EREMCHG: c_int = 128;
pub const ELIBACC: c_int = 129;
pub const ELIBBAD: c_int = 130;
pub const ELIBSCN: c_int = 131;
pub const ELIBMAX: c_int = 132;
pub const ELIBEXEC: c_int = 133;
pub const ERESTART: c_int = 134;
pub const ESTRPIPE: c_int = 135;
pub const EUSERS: c_int = 136;
pub const ESOCKTNOSUPPORT: c_int = 137;
pub const EOPNOTSUPP: c_int = 138;
pub const ENOTSUP: c_int = EOPNOTSUPP;
pub const EPFNOSUPPORT: c_int = 139;
pub const ESHUTDOWN: c_int = 140;
pub const ETOOMANYREFS: c_int = 141;
pub const EHOSTDOWN: c_int = 142;
pub const EUCLEAN: c_int = 143;
pub const ENOTNAM: c_int = 144;
pub const ENAVAIL: c_int = 145;
pub const EISNAM: c_int = 146;
pub const EREMOTEIO: c_int = 147;
pub const ENOMEDIUM: c_int = 148;
pub const EMEDIUMTYPE: c_int = 149;
pub const ENOKEY: c_int = 150;
pub const EKEYEXPIRED: c_int = 151;
pub const EKEYREVOKED: c_int = 152;
pub const EKEYREJECTED: c_int = 153;
pub const ERFKILL: c_int = 154;
pub const EHWPOISON: c_int = 155;
pub const EL2NSYNC: c_int = 156;

pub const SA_NODEFER: c_int = 0x40000000;
pub const SA_RESETHAND: c_int = 0x80000000;
pub const SA_RESTART: c_int = 0x10000000;
pub const SA_NOCLDSTOP: c_int = 0x00000001;

pub const BUFSIZ: c_uint = 1024;
pub const TMP_MAX: c_uint = 10000;
pub const FOPEN_MAX: c_uint = 1000;
pub const O_PATH: c_int = 0o10000000;
pub const O_EXEC: c_int = 0o10000000;
pub const O_SEARCH: c_int = 0o10000000;
pub const O_ACCMODE: c_int = 0o10000003;
pub const O_NDELAY: c_int = O_NONBLOCK;
pub const NI_MAXHOST: crate::socklen_t = 255;
pub const PTHREAD_STACK_MIN: size_t = 2048;
pub const POSIX_FADV_DONTNEED: c_int = 4;
pub const POSIX_FADV_NOREUSE: c_int = 5;

pub const POSIX_MADV_DONTNEED: c_int = 4;

pub const RLIM_INFINITY: crate::rlim_t = !0;
#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIMIT_NLIMITS: c_int = 16;
#[allow(deprecated)]
#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIM_NLIMITS: c_int = RLIMIT_NLIMITS;

pub const MAP_ANONYMOUS: c_int = MAP_ANON;

#[doc(hidden)]
#[deprecated(since = "0.2.55", note = "Use SIGSYS instead")]
pub const SIGUNUSED: c_int = crate::SIGSYS;

pub const __SIZEOF_PTHREAD_CONDATTR_T: usize = 4;
pub const __SIZEOF_PTHREAD_MUTEXATTR_T: usize = 4;
pub const __SIZEOF_PTHREAD_RWLOCKATTR_T: usize = 8;

pub const CPU_SETSIZE: c_int = 1024;

pub const TCSANOW: c_int = 0;
pub const TCSADRAIN: c_int = 1;
pub const TCSAFLUSH: c_int = 2;

pub const TIOCINQ: c_int = crate::FIONREAD;

pub const RTLD_GLOBAL: c_int = 0x100;
pub const RTLD_NOLOAD: c_int = 0x4;

pub const CLOCK_SGI_CYCLE: crate::clockid_t = 10;

pub const MCL_CURRENT: c_int = 0x0001;
pub const MCL_FUTURE: c_int = 0x0002;

pub const SIGSTKSZ: size_t = 8192;
pub const MINSIGSTKSZ: size_t = 2048;
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

pub const SO_BINDTODEVICE: c_int = 25;
pub const SO_TIMESTAMP: c_int = 63;
pub const SO_MARK: c_int = 36;
pub const SO_RXQ_OVFL: c_int = 40;
pub const SO_PEEK_OFF: c_int = 42;
pub const SO_BUSY_POLL: c_int = 46;

pub const __SIZEOF_PTHREAD_RWLOCK_T: usize = 32;
pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 24;

pub const O_DIRECT: c_int = 0x4000;
pub const O_DIRECTORY: c_int = 0x10000;
pub const O_NOFOLLOW: c_int = 0x20000;
pub const O_ASYNC: c_int = 0x2000;

pub const FIOCLEX: c_int = 0x5451;
pub const FIONBIO: c_int = 0x5421;

pub const RLIMIT_RSS: c_int = 5;
pub const RLIMIT_NOFILE: c_int = 7;
pub const RLIMIT_AS: c_int = 9;
pub const RLIMIT_NPROC: c_int = 6;
pub const RLIMIT_MEMLOCK: c_int = 8;
pub const RLIMIT_CPU: c_int = 0;
pub const RLIMIT_FSIZE: c_int = 1;
pub const RLIMIT_DATA: c_int = 2;
pub const RLIMIT_STACK: c_int = 3;
pub const RLIMIT_CORE: c_int = 4;
pub const RLIMIT_LOCKS: c_int = 10;
pub const RLIMIT_SIGPENDING: c_int = 11;
pub const RLIMIT_MSGQUEUE: c_int = 12;
pub const RLIMIT_NICE: c_int = 13;
pub const RLIMIT_RTPRIO: c_int = 14;

pub const O_APPEND: c_int = 1024;
pub const O_CREAT: c_int = 64;
pub const O_EXCL: c_int = 128;
pub const O_NOCTTY: c_int = 256;
pub const O_NONBLOCK: c_int = 2048;
pub const O_SYNC: c_int = 1052672;
pub const O_RSYNC: c_int = 1052672;
pub const O_DSYNC: c_int = 4096;

pub const SOCK_NONBLOCK: c_int = 2048;

pub const MAP_ANON: c_int = 0x0020;
pub const MAP_GROWSDOWN: c_int = 0x0100;
pub const MAP_DENYWRITE: c_int = 0x0800;
pub const MAP_EXECUTABLE: c_int = 0x01000;
pub const MAP_LOCKED: c_int = 0x02000;
pub const MAP_NORESERVE: c_int = 0x04000;
pub const MAP_POPULATE: c_int = 0x08000;
pub const MAP_NONBLOCK: c_int = 0x010000;
pub const MAP_STACK: c_int = 0x020000;

pub const SOCK_STREAM: c_int = 1;
pub const SOCK_DGRAM: c_int = 2;
pub const SOCK_SEQPACKET: c_int = 5;

pub const IPPROTO_MAX: c_int = 263;

pub const SOL_SOCKET: c_int = 1;

pub const SO_REUSEADDR: c_int = 2;
pub const SO_TYPE: c_int = 3;
pub const SO_ERROR: c_int = 4;
pub const SO_DONTROUTE: c_int = 5;
pub const SO_BROADCAST: c_int = 6;
pub const SO_SNDBUF: c_int = 7;
pub const SO_RCVBUF: c_int = 8;
pub const SO_KEEPALIVE: c_int = 9;
pub const SO_OOBINLINE: c_int = 10;
pub const SO_LINGER: c_int = 13;
pub const SO_REUSEPORT: c_int = 15;
pub const SO_RCVLOWAT: c_int = 18;
pub const SO_SNDLOWAT: c_int = 19;
pub const SO_RCVTIMEO: c_int = 66;
pub const SO_SNDTIMEO: c_int = 67;
pub const SO_ACCEPTCONN: c_int = 30;

pub const IPV6_RTHDR_LOOSE: c_int = 0;
pub const IPV6_RTHDR_STRICT: c_int = 1;

pub const SA_ONSTACK: c_int = 0x08000000;
pub const SA_SIGINFO: c_int = 0x00000004;
pub const SA_NOCLDWAIT: c_int = 0x00000002;

pub const SIGCHLD: c_int = 17;
pub const SIGBUS: c_int = 7;
pub const SIGTTIN: c_int = 21;
pub const SIGTTOU: c_int = 22;
pub const SIGXCPU: c_int = 24;
pub const SIGXFSZ: c_int = 25;
pub const SIGVTALRM: c_int = 26;
pub const SIGPROF: c_int = 27;
pub const SIGWINCH: c_int = 28;
pub const SIGUSR1: c_int = 10;
pub const SIGUSR2: c_int = 12;
pub const SIGCONT: c_int = 18;
pub const SIGSTOP: c_int = 19;
pub const SIGTSTP: c_int = 20;
pub const SIGURG: c_int = 23;
pub const SIGIO: c_int = 29;
pub const SIGSYS: c_int = 31;
pub const SIGSTKFLT: c_int = 16;
pub const SIGPOLL: c_int = 29;
pub const SIGPWR: c_int = 30;
pub const SIG_SETMASK: c_int = 2;
pub const SIG_BLOCK: c_int = 0x000000;
pub const SIG_UNBLOCK: c_int = 0x01;

pub const EXTPROC: crate::tcflag_t = 0x00010000;

pub const MAP_HUGETLB: c_int = 0x040000;

pub const F_GETLK: c_int = 12;
pub const F_GETOWN: c_int = 9;
pub const F_SETLK: c_int = 13;
pub const F_SETLKW: c_int = 14;
pub const F_SETOWN: c_int = 8;
pub const F_OFD_GETLK: c_int = 36;
pub const F_OFD_SETLK: c_int = 37;
pub const F_OFD_SETLKW: c_int = 38;

pub const VEOF: usize = 4;
pub const VEOL: usize = 11;
pub const VEOL2: usize = 16;
pub const VMIN: usize = 6;
pub const IEXTEN: crate::tcflag_t = 0x00008000;
pub const TOSTOP: crate::tcflag_t = 0x00000100;
pub const FLUSHO: crate::tcflag_t = 0x00001000;

pub const TCGETS: c_int = 0x5401;
pub const TCSETS: c_int = 0x5402;
pub const TCSETSW: c_int = 0x5403;
pub const TCSETSF: c_int = 0x5404;
pub const TCGETA: c_int = 0x5405;
pub const TCSETA: c_int = 0x5406;
pub const TCSETAW: c_int = 0x5407;
pub const TCSETAF: c_int = 0x5408;
pub const TCSBRK: c_int = 0x5409;
pub const TCXONC: c_int = 0x540A;
pub const TCFLSH: c_int = 0x540B;
pub const TIOCGSOFTCAR: c_int = 0x5419;
pub const TIOCSSOFTCAR: c_int = 0x541A;
pub const TIOCLINUX: c_int = 0x541C;
pub const TIOCGSERIAL: c_int = 0x541E;
pub const TIOCEXCL: c_int = 0x540C;
pub const TIOCNXCL: c_int = 0x540D;
pub const TIOCSCTTY: c_int = 0x540E;
pub const TIOCGPGRP: c_int = 0x540F;
pub const TIOCSPGRP: c_int = 0x5410;
pub const TIOCOUTQ: c_int = 0x5411;
pub const TIOCSTI: c_int = 0x5412;
pub const TIOCGWINSZ: c_int = 0x5413;
pub const TIOCSWINSZ: c_int = 0x5414;
pub const TIOCMGET: c_int = 0x5415;
pub const TIOCMBIS: c_int = 0x5416;
pub const TIOCMBIC: c_int = 0x5417;
pub const TIOCMSET: c_int = 0x5418;
pub const FIONREAD: c_int = 0x541B;
pub const TIOCCONS: c_int = 0x541D;

pub const SYS_gettid: c_long = 224; // Valid for arm (32-bit) and x86 (32-bit)

pub const POLLWRNORM: c_short = 0x100;
pub const POLLWRBAND: c_short = 0x200;

pub const TIOCM_LE: c_int = 0x001;
pub const TIOCM_DTR: c_int = 0x002;
pub const TIOCM_RTS: c_int = 0x004;
pub const TIOCM_ST: c_int = 0x008;
pub const TIOCM_SR: c_int = 0x010;
pub const TIOCM_CTS: c_int = 0x020;
pub const TIOCM_CAR: c_int = 0x040;
pub const TIOCM_RNG: c_int = 0x080;
pub const TIOCM_DSR: c_int = 0x100;
pub const TIOCM_CD: c_int = TIOCM_CAR;
pub const TIOCM_RI: c_int = TIOCM_RNG;
pub const O_TMPFILE: c_int = 0x410000;

pub const MAX_ADDR_LEN: usize = 7;
pub const ARPD_UPDATE: c_ushort = 0x01;
pub const ARPD_LOOKUP: c_ushort = 0x02;
pub const ARPD_FLUSH: c_ushort = 0x03;
pub const ATF_MAGIC: c_int = 0x80;

pub const PRIO_PROCESS: c_int = 0;
pub const PRIO_PGRP: c_int = 1;
pub const PRIO_USER: c_int = 2;

pub const SOMAXCONN: c_int = 128;

f! {
    pub fn CMSG_NXTHDR(mhdr: *const msghdr, cmsg: *const cmsghdr) -> *mut cmsghdr {
        if ((*cmsg).cmsg_len as usize) < size_of::<cmsghdr>() {
            return core::ptr::null_mut::<cmsghdr>();
        }
        let next = (cmsg as usize + super::CMSG_ALIGN((*cmsg).cmsg_len as usize)) as *mut cmsghdr;
        let max = (*mhdr).msg_control as usize + (*mhdr).msg_controllen as usize;
        if (next.offset(1)) as usize > max {
            core::ptr::null_mut::<cmsghdr>()
        } else {
            next as *mut cmsghdr
        }
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
        ()
    }

    pub fn CPU_CLR(cpu: usize, cpuset: &mut cpu_set_t) -> () {
        let size_in_bits = 8 * size_of_val(&cpuset.bits[0]); // 32, 64 etc
        let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
        cpuset.bits[idx] &= !(1 << offset);
        ()
    }

    pub fn CPU_ISSET(cpu: usize, cpuset: &cpu_set_t) -> bool {
        let size_in_bits = 8 * size_of_val(&cpuset.bits[0]);
        let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
        0 != (cpuset.bits[idx] & (1 << offset))
    }

    pub fn CPU_EQUAL(set1: &cpu_set_t, set2: &cpu_set_t) -> bool {
        set1.bits == set2.bits
    }
}

safe_f! {
    pub {const} fn makedev(major: c_uint, minor: c_uint) -> crate::dev_t {
        let major = major as crate::dev_t;
        let minor = minor as crate::dev_t;
        let mut dev = 0;
        dev |= (major & 0xfffff000) << 31 << 1;
        dev |= (major & 0x00000fff) << 8;
        dev |= (minor & 0xffffff00) << 12;
        dev |= minor & 0x000000ff;
        dev
    }

    pub {const} fn major(dev: crate::dev_t) -> c_uint {
        // see
        // https://github.com/emscripten-core/emscripten/blob/
        // main/system/lib/libc/musl/include/sys/sysmacros.h
        let mut major = 0;
        major |= (dev >> 31 >> 1) & 0xfffff000;
        major |= (dev >> 8) & 0x00000fff;
        major as c_uint
    }

    pub {const} fn minor(dev: crate::dev_t) -> c_uint {
        // see
        // https://github.com/emscripten-core/emscripten/blob/
        // main/system/lib/libc/musl/include/sys/sysmacros.h
        let mut minor = 0;
        minor |= (dev >> 12) & 0xffffff00;
        minor |= dev & 0x000000ff;
        minor as c_uint
    }
}

extern "C" {
    pub fn getrlimit(resource: c_int, rlim: *mut crate::rlimit) -> c_int;
    pub fn setrlimit(resource: c_int, rlim: *const crate::rlimit) -> c_int;
    pub fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;

    pub fn abs(i: c_int) -> c_int;
    pub fn labs(i: c_long) -> c_long;
    pub fn rand() -> c_int;
    pub fn srand(seed: c_uint);

    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut c_void) -> c_int;

    pub fn setpwent();
    pub fn endpwent();
    pub fn getpwent() -> *mut passwd;

    pub fn shm_open(name: *const c_char, oflag: c_int, mode: mode_t) -> c_int;

    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;
    pub fn __errno_location() -> *mut c_int;

    pub fn posix_fallocate(fd: c_int, offset: off_t, len: off_t) -> c_int;
    pub fn pwritev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off_t) -> ssize_t;
    pub fn preadv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off_t) -> ssize_t;
    pub fn dup3(oldfd: c_int, newfd: c_int, flags: c_int) -> c_int;
    pub fn nl_langinfo_l(item: crate::nl_item, locale: crate::locale_t) -> *mut c_char;
    pub fn accept4(
        fd: c_int,
        addr: *mut crate::sockaddr,
        len: *mut crate::socklen_t,
        flg: c_int,
    ) -> c_int;
    pub fn getnameinfo(
        sa: *const crate::sockaddr,
        salen: crate::socklen_t,
        host: *mut c_char,
        hostlen: crate::socklen_t,
        serv: *mut c_char,
        servlen: crate::socklen_t,
        flags: c_int,
    ) -> c_int;
    pub fn getloadavg(loadavg: *mut c_double, nelem: c_int) -> c_int;

    pub fn mkfifoat(dirfd: c_int, pathname: *const c_char, mode: mode_t) -> c_int;
    pub fn if_nameindex() -> *mut if_nameindex;
    pub fn if_freenameindex(ptr: *mut if_nameindex);

    pub fn mremap(
        addr: *mut c_void,
        len: size_t,
        new_len: size_t,
        flags: c_int,
        ...
    ) -> *mut c_void;

    pub fn glob(
        pattern: *const c_char,
        flags: c_int,
        errfunc: Option<extern "C" fn(epath: *const c_char, errno: c_int) -> c_int>,
        pglob: *mut crate::glob_t,
    ) -> c_int;
    pub fn globfree(pglob: *mut crate::glob_t);

    pub fn posix_madvise(addr: *mut c_void, len: size_t, advice: c_int) -> c_int;

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
    pub fn mkstemps(template: *mut c_char, suffixlen: c_int) -> c_int;
    pub fn nl_langinfo(item: crate::nl_item) -> *mut c_char;

    pub fn sendmmsg(
        sockfd: c_int,
        msgvec: *mut crate::mmsghdr,
        vlen: c_uint,
        flags: c_uint,
    ) -> c_int;
    pub fn recvmmsg(
        sockfd: c_int,
        msgvec: *mut crate::mmsghdr,
        vlen: c_uint,
        flags: c_uint,
        timeout: *mut crate::timespec,
    ) -> c_int;
    pub fn sync();
    pub fn ioctl(fd: c_int, request: c_int, ...) -> c_int;
    pub fn getpriority(which: c_int, who: crate::id_t) -> c_int;
    pub fn setpriority(which: c_int, who: crate::id_t, prio: c_int) -> c_int;
    pub fn pthread_create(
        native: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        f: extern "C" fn(*mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;

    pub fn getentropy(buf: *mut c_void, buflen: size_t) -> c_int;

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

    // grp.h
    pub fn getgrgid(gid: crate::gid_t) -> *mut crate::group;
    pub fn getgrnam(name: *const c_char) -> *mut crate::group;
    pub fn getgrnam_r(
        name: *const c_char,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn getgrgid_r(
        gid: crate::gid_t,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
}

// Alias <foo> to <foo>64 to mimic glibc's LFS64 support
mod lfs64;
pub use self::lfs64::*;
