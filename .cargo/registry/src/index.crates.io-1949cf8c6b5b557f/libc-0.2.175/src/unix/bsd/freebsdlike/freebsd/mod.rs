use crate::prelude::*;
use crate::{cmsghdr, off_t};

pub type fflags_t = u32;

pub type vm_prot_t = u_char;
pub type kvaddr_t = u64;
pub type segsz_t = isize;
pub type __fixpt_t = u32;
pub type fixpt_t = __fixpt_t;
pub type __lwpid_t = i32;
pub type lwpid_t = __lwpid_t;
pub type blksize_t = i32;
pub type ksize_t = u64;
pub type inp_gen_t = u64;
pub type so_gen_t = u64;
pub type clockid_t = c_int;
pub type sem_t = _sem;
pub type timer_t = *mut __c_anonymous__timer;

pub type fsblkcnt_t = u64;
pub type fsfilcnt_t = u64;
pub type idtype_t = c_uint;

pub type msglen_t = c_ulong;
pub type msgqnum_t = c_ulong;

pub type cpulevel_t = c_int;
pub type cpuwhich_t = c_int;

pub type mqd_t = *mut c_void;

pub type pthread_spinlock_t = *mut __c_anonymous_pthread_spinlock;
pub type pthread_barrierattr_t = *mut __c_anonymous_pthread_barrierattr;
pub type pthread_barrier_t = *mut __c_anonymous_pthread_barrier;

pub type uuid_t = crate::uuid;
pub type u_int = c_uint;
pub type u_char = c_uchar;
pub type u_long = c_ulong;
pub type u_short = c_ushort;

pub type caddr_t = *mut c_char;

pub type fhandle_t = fhandle;

pub type au_id_t = crate::uid_t;
pub type au_asid_t = crate::pid_t;

pub type cpusetid_t = c_int;

pub type sctp_assoc_t = u32;

pub type eventfd_t = u64;

#[cfg_attr(feature = "extra_traits", derive(Debug, Hash, PartialEq, Eq))]
#[repr(u32)]
pub enum devstat_support_flags {
    DEVSTAT_ALL_SUPPORTED = 0x00,
    DEVSTAT_NO_BLOCKSIZE = 0x01,
    DEVSTAT_NO_ORDERED_TAGS = 0x02,
    DEVSTAT_BS_UNAVAILABLE = 0x04,
}
impl Copy for devstat_support_flags {}
impl Clone for devstat_support_flags {
    fn clone(&self) -> devstat_support_flags {
        *self
    }
}

#[cfg_attr(feature = "extra_traits", derive(Debug, Hash, PartialEq, Eq))]
#[repr(u32)]
pub enum devstat_trans_flags {
    DEVSTAT_NO_DATA = 0x00,
    DEVSTAT_READ = 0x01,
    DEVSTAT_WRITE = 0x02,
    DEVSTAT_FREE = 0x03,
}

impl Copy for devstat_trans_flags {}
impl Clone for devstat_trans_flags {
    fn clone(&self) -> devstat_trans_flags {
        *self
    }
}

#[cfg_attr(feature = "extra_traits", derive(Debug, Hash, PartialEq, Eq))]
#[repr(u32)]
pub enum devstat_tag_type {
    DEVSTAT_TAG_SIMPLE = 0x00,
    DEVSTAT_TAG_HEAD = 0x01,
    DEVSTAT_TAG_ORDERED = 0x02,
    DEVSTAT_TAG_NONE = 0x03,
}
impl Copy for devstat_tag_type {}
impl Clone for devstat_tag_type {
    fn clone(&self) -> devstat_tag_type {
        *self
    }
}

#[cfg_attr(feature = "extra_traits", derive(Debug, Hash, PartialEq, Eq))]
#[repr(u32)]
pub enum devstat_match_flags {
    DEVSTAT_MATCH_NONE = 0x00,
    DEVSTAT_MATCH_TYPE = 0x01,
    DEVSTAT_MATCH_IF = 0x02,
    DEVSTAT_MATCH_PASS = 0x04,
}
impl Copy for devstat_match_flags {}
impl Clone for devstat_match_flags {
    fn clone(&self) -> devstat_match_flags {
        *self
    }
}

#[cfg_attr(feature = "extra_traits", derive(Debug, Hash, PartialEq, Eq))]
#[repr(u32)]
pub enum devstat_priority {
    DEVSTAT_PRIORITY_MIN = 0x000,
    DEVSTAT_PRIORITY_OTHER = 0x020,
    DEVSTAT_PRIORITY_PASS = 0x030,
    DEVSTAT_PRIORITY_FD = 0x040,
    DEVSTAT_PRIORITY_WFD = 0x050,
    DEVSTAT_PRIORITY_TAPE = 0x060,
    DEVSTAT_PRIORITY_CD = 0x090,
    DEVSTAT_PRIORITY_DISK = 0x110,
    DEVSTAT_PRIORITY_ARRAY = 0x120,
    DEVSTAT_PRIORITY_MAX = 0xfff,
}
impl Copy for devstat_priority {}
impl Clone for devstat_priority {
    fn clone(&self) -> devstat_priority {
        *self
    }
}

#[cfg_attr(feature = "extra_traits", derive(Debug, Hash, PartialEq, Eq))]
#[repr(u32)]
pub enum devstat_type_flags {
    DEVSTAT_TYPE_DIRECT = 0x000,
    DEVSTAT_TYPE_SEQUENTIAL = 0x001,
    DEVSTAT_TYPE_PRINTER = 0x002,
    DEVSTAT_TYPE_PROCESSOR = 0x003,
    DEVSTAT_TYPE_WORM = 0x004,
    DEVSTAT_TYPE_CDROM = 0x005,
    DEVSTAT_TYPE_SCANNER = 0x006,
    DEVSTAT_TYPE_OPTICAL = 0x007,
    DEVSTAT_TYPE_CHANGER = 0x008,
    DEVSTAT_TYPE_COMM = 0x009,
    DEVSTAT_TYPE_ASC0 = 0x00a,
    DEVSTAT_TYPE_ASC1 = 0x00b,
    DEVSTAT_TYPE_STORARRAY = 0x00c,
    DEVSTAT_TYPE_ENCLOSURE = 0x00d,
    DEVSTAT_TYPE_FLOPPY = 0x00e,
    DEVSTAT_TYPE_MASK = 0x00f,
    DEVSTAT_TYPE_IF_SCSI = 0x010,
    DEVSTAT_TYPE_IF_IDE = 0x020,
    DEVSTAT_TYPE_IF_OTHER = 0x030,
    DEVSTAT_TYPE_IF_MASK = 0x0f0,
    DEVSTAT_TYPE_PASS = 0x100,
}
impl Copy for devstat_type_flags {}
impl Clone for devstat_type_flags {
    fn clone(&self) -> devstat_type_flags {
        *self
    }
}

#[cfg_attr(feature = "extra_traits", derive(Debug, Hash, PartialEq, Eq))]
#[repr(u32)]
pub enum devstat_metric {
    DSM_NONE,
    DSM_TOTAL_BYTES,
    DSM_TOTAL_BYTES_READ,
    DSM_TOTAL_BYTES_WRITE,
    DSM_TOTAL_TRANSFERS,
    DSM_TOTAL_TRANSFERS_READ,
    DSM_TOTAL_TRANSFERS_WRITE,
    DSM_TOTAL_TRANSFERS_OTHER,
    DSM_TOTAL_BLOCKS,
    DSM_TOTAL_BLOCKS_READ,
    DSM_TOTAL_BLOCKS_WRITE,
    DSM_KB_PER_TRANSFER,
    DSM_KB_PER_TRANSFER_READ,
    DSM_KB_PER_TRANSFER_WRITE,
    DSM_TRANSFERS_PER_SECOND,
    DSM_TRANSFERS_PER_SECOND_READ,
    DSM_TRANSFERS_PER_SECOND_WRITE,
    DSM_TRANSFERS_PER_SECOND_OTHER,
    DSM_MB_PER_SECOND,
    DSM_MB_PER_SECOND_READ,
    DSM_MB_PER_SECOND_WRITE,
    DSM_BLOCKS_PER_SECOND,
    DSM_BLOCKS_PER_SECOND_READ,
    DSM_BLOCKS_PER_SECOND_WRITE,
    DSM_MS_PER_TRANSACTION,
    DSM_MS_PER_TRANSACTION_READ,
    DSM_MS_PER_TRANSACTION_WRITE,
    DSM_SKIP,
    DSM_TOTAL_BYTES_FREE,
    DSM_TOTAL_TRANSFERS_FREE,
    DSM_TOTAL_BLOCKS_FREE,
    DSM_KB_PER_TRANSFER_FREE,
    DSM_MB_PER_SECOND_FREE,
    DSM_TRANSFERS_PER_SECOND_FREE,
    DSM_BLOCKS_PER_SECOND_FREE,
    DSM_MS_PER_TRANSACTION_OTHER,
    DSM_MS_PER_TRANSACTION_FREE,
    DSM_BUSY_PCT,
    DSM_QUEUE_LENGTH,
    DSM_TOTAL_DURATION,
    DSM_TOTAL_DURATION_READ,
    DSM_TOTAL_DURATION_WRITE,
    DSM_TOTAL_DURATION_FREE,
    DSM_TOTAL_DURATION_OTHER,
    DSM_TOTAL_BUSY_TIME,
    DSM_MAX,
}
impl Copy for devstat_metric {}
impl Clone for devstat_metric {
    fn clone(&self) -> devstat_metric {
        *self
    }
}

#[cfg_attr(feature = "extra_traits", derive(Debug, Hash, PartialEq, Eq))]
#[repr(u32)]
pub enum devstat_select_mode {
    DS_SELECT_ADD,
    DS_SELECT_ONLY,
    DS_SELECT_REMOVE,
    DS_SELECT_ADDONLY,
}
impl Copy for devstat_select_mode {}
impl Clone for devstat_select_mode {
    fn clone(&self) -> devstat_select_mode {
        *self
    }
}

s! {
    pub struct aiocb {
        pub aio_fildes: c_int,
        pub aio_offset: off_t,
        pub aio_buf: *mut c_void,
        pub aio_nbytes: size_t,
        __unused1: [c_int; 2],
        __unused2: *mut c_void,
        pub aio_lio_opcode: c_int,
        pub aio_reqprio: c_int,
        // unused 3 through 5 are the __aiocb_private structure
        __unused3: c_long,
        __unused4: c_long,
        __unused5: *mut c_void,
        pub aio_sigevent: sigevent,
    }

    pub struct jail {
        pub version: u32,
        pub path: *mut c_char,
        pub hostname: *mut c_char,
        pub jailname: *mut c_char,
        pub ip4s: c_uint,
        pub ip6s: c_uint,
        pub ip4: *mut crate::in_addr,
        pub ip6: *mut crate::in6_addr,
    }

    pub struct statvfs {
        pub f_bavail: crate::fsblkcnt_t,
        pub f_bfree: crate::fsblkcnt_t,
        pub f_blocks: crate::fsblkcnt_t,
        pub f_favail: crate::fsfilcnt_t,
        pub f_ffree: crate::fsfilcnt_t,
        pub f_files: crate::fsfilcnt_t,
        pub f_bsize: c_ulong,
        pub f_flag: c_ulong,
        pub f_frsize: c_ulong,
        pub f_fsid: c_ulong,
        pub f_namemax: c_ulong,
    }

    // internal structure has changed over time
    pub struct _sem {
        data: [u32; 4],
    }
    pub struct sembuf {
        pub sem_num: c_ushort,
        pub sem_op: c_short,
        pub sem_flg: c_short,
    }

    pub struct input_event {
        pub time: crate::timeval,
        pub type_: crate::u_short,
        pub code: crate::u_short,
        pub value: i32,
    }

    pub struct input_absinfo {
        pub value: i32,
        pub minimum: i32,
        pub maximum: i32,
        pub fuzz: i32,
        pub flat: i32,
        pub resolution: i32,
    }

    pub struct msqid_ds {
        pub msg_perm: crate::ipc_perm,
        __unused1: *mut c_void,
        __unused2: *mut c_void,
        pub msg_cbytes: crate::msglen_t,
        pub msg_qnum: crate::msgqnum_t,
        pub msg_qbytes: crate::msglen_t,
        pub msg_lspid: crate::pid_t,
        pub msg_lrpid: crate::pid_t,
        pub msg_stime: crate::time_t,
        pub msg_rtime: crate::time_t,
        pub msg_ctime: crate::time_t,
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_size: size_t,
        pub ss_flags: c_int,
    }

    pub struct mmsghdr {
        pub msg_hdr: crate::msghdr,
        pub msg_len: ssize_t,
    }

    pub struct sockcred {
        pub sc_uid: crate::uid_t,
        pub sc_euid: crate::uid_t,
        pub sc_gid: crate::gid_t,
        pub sc_egid: crate::gid_t,
        pub sc_ngroups: c_int,
        pub sc_groups: [crate::gid_t; 1],
    }

    pub struct ptrace_vm_entry {
        pub pve_entry: c_int,
        pub pve_timestamp: c_int,
        pub pve_start: c_ulong,
        pub pve_end: c_ulong,
        pub pve_offset: c_ulong,
        pub pve_prot: c_uint,
        pub pve_pathlen: c_uint,
        pub pve_fileid: c_long,
        pub pve_fsid: u32,
        pub pve_path: *mut c_char,
    }

    pub struct ptrace_lwpinfo {
        pub pl_lwpid: lwpid_t,
        pub pl_event: c_int,
        pub pl_flags: c_int,
        pub pl_sigmask: crate::sigset_t,
        pub pl_siglist: crate::sigset_t,
        pub pl_siginfo: crate::siginfo_t,
        pub pl_tdname: [c_char; crate::MAXCOMLEN as usize + 1],
        pub pl_child_pid: crate::pid_t,
        pub pl_syscall_code: c_uint,
        pub pl_syscall_narg: c_uint,
    }

    pub struct ptrace_sc_ret {
        pub sr_retval: [crate::register_t; 2],
        pub sr_error: c_int,
    }

    pub struct ptrace_coredump {
        pub pc_fd: c_int,
        pub pc_flags: u32,
        pub pc_limit: off_t,
    }

    pub struct ptrace_sc_remote {
        pub pscr_ret: ptrace_sc_ret,
        pub pscr_syscall: c_uint,
        pub pscr_nargs: c_uint,
        pub pscr_args: *mut crate::register_t,
    }

    pub struct cpuset_t {
        #[cfg(all(any(freebsd15, freebsd14), target_pointer_width = "64"))]
        __bits: [c_long; 16],
        #[cfg(all(any(freebsd15, freebsd14), target_pointer_width = "32"))]
        __bits: [c_long; 32],
        #[cfg(all(not(any(freebsd15, freebsd14)), target_pointer_width = "64"))]
        __bits: [c_long; 4],
        #[cfg(all(not(any(freebsd15, freebsd14)), target_pointer_width = "32"))]
        __bits: [c_long; 8],
    }

    pub struct cap_rights_t {
        cr_rights: [u64; 2],
    }

    pub struct umutex {
        m_owner: crate::lwpid_t,
        m_flags: u32,
        m_ceilings: [u32; 2],
        m_rb_link: crate::uintptr_t,
        #[cfg(target_pointer_width = "32")]
        m_pad: u32,
        m_spare: [u32; 2],
    }

    pub struct ucond {
        c_has_waiters: u32,
        c_flags: u32,
        c_clockid: u32,
        c_spare: [u32; 1],
    }

    pub struct uuid {
        pub time_low: u32,
        pub time_mid: u16,
        pub time_hi_and_version: u16,
        pub clock_seq_hi_and_reserved: u8,
        pub clock_seq_low: u8,
        pub node: [u8; _UUID_NODE_LEN],
    }

    pub struct __c_anonymous_pthread_spinlock {
        s_clock: umutex,
    }

    pub struct __c_anonymous_pthread_barrierattr {
        pshared: c_int,
    }

    pub struct __c_anonymous_pthread_barrier {
        b_lock: umutex,
        b_cv: ucond,
        b_cycle: i64,
        b_count: c_int,
        b_waiters: c_int,
        b_refcount: c_int,
        b_destroying: c_int,
    }

    pub struct kinfo_vmentry {
        pub kve_structsize: c_int,
        pub kve_type: c_int,
        pub kve_start: u64,
        pub kve_end: u64,
        pub kve_offset: u64,
        pub kve_vn_fileid: u64,
        #[cfg(not(freebsd11))]
        pub kve_vn_fsid_freebsd11: u32,
        #[cfg(freebsd11)]
        pub kve_vn_fsid: u32,
        pub kve_flags: c_int,
        pub kve_resident: c_int,
        pub kve_private_resident: c_int,
        pub kve_protection: c_int,
        pub kve_ref_count: c_int,
        pub kve_shadow_count: c_int,
        pub kve_vn_type: c_int,
        pub kve_vn_size: u64,
        #[cfg(not(freebsd11))]
        pub kve_vn_rdev_freebsd11: u32,
        #[cfg(freebsd11)]
        pub kve_vn_rdev: u32,
        pub kve_vn_mode: u16,
        pub kve_status: u16,
        #[cfg(not(freebsd11))]
        pub kve_vn_fsid: u64,
        #[cfg(not(freebsd11))]
        pub kve_vn_rdev: u64,
        #[cfg(not(freebsd11))]
        _kve_is_spare: [c_int; 8],
        #[cfg(freebsd11)]
        _kve_is_spare: [c_int; 12],
        pub kve_path: [[c_char; 32]; 32],
    }

    pub struct __c_anonymous_filestat {
        pub stqe_next: *mut filestat,
    }

    pub struct filestat {
        pub fs_type: c_int,
        pub fs_flags: c_int,
        pub fs_fflags: c_int,
        pub fs_uflags: c_int,
        pub fs_fd: c_int,
        pub fs_ref_count: c_int,
        pub fs_offset: off_t,
        pub fs_typedep: *mut c_void,
        pub fs_path: *mut c_char,
        pub next: __c_anonymous_filestat,
        pub fs_cap_rights: cap_rights_t,
    }

    pub struct filestat_list {
        pub stqh_first: *mut filestat,
        pub stqh_last: *mut *mut filestat,
    }

    pub struct procstat {
        pub tpe: c_int,
        pub kd: crate::uintptr_t,
        pub vmentries: *mut c_void,
        pub files: *mut c_void,
        pub argv: *mut c_void,
        pub envv: *mut c_void,
        pub core: crate::uintptr_t,
    }

    pub struct itimerspec {
        pub it_interval: crate::timespec,
        pub it_value: crate::timespec,
    }

    pub struct __c_anonymous__timer {
        _priv: [c_int; 3],
    }

    /// Used to hold a copy of the command line, if it had a sane length.
    pub struct pargs {
        /// Reference count.
        pub ar_ref: u_int,
        /// Length.
        pub ar_length: u_int,
        /// Arguments.
        pub ar_args: [c_uchar; 1],
    }

    pub struct priority {
        /// Scheduling class.
        pub pri_class: u_char,
        /// Normal priority level.
        pub pri_level: u_char,
        /// Priority before propagation.
        pub pri_native: u_char,
        /// User priority based on p_cpu and p_nice.
        pub pri_user: u_char,
    }

    pub struct kvm_swap {
        pub ksw_devname: [c_char; 32],
        pub ksw_used: u_int,
        pub ksw_total: u_int,
        pub ksw_flags: c_int,
        pub ksw_reserved1: u_int,
        pub ksw_reserved2: u_int,
    }

    pub struct nlist {
        /// symbol name (in memory)
        pub n_name: *const c_char,
        /// type defines
        pub n_type: c_uchar,
        /// "type" and binding information
        pub n_other: c_char,
        /// used by stab entries
        pub n_desc: c_short,
        pub n_value: c_ulong,
    }

    pub struct kvm_nlist {
        pub n_name: *const c_char,
        pub n_type: c_uchar,
        pub n_value: crate::kvaddr_t,
    }

    pub struct __c_anonymous_sem {
        _priv: crate::uintptr_t,
    }

    pub struct semid_ds {
        pub sem_perm: crate::ipc_perm,
        pub __sem_base: *mut __c_anonymous_sem,
        pub sem_nsems: c_ushort,
        pub sem_otime: crate::time_t,
        pub sem_ctime: crate::time_t,
    }

    pub struct vmtotal {
        pub t_vm: u64,
        pub t_avm: u64,
        pub t_rm: u64,
        pub t_arm: u64,
        pub t_vmshr: u64,
        pub t_avmshr: u64,
        pub t_rmshr: u64,
        pub t_armshr: u64,
        pub t_free: u64,
        pub t_rq: i16,
        pub t_dw: i16,
        pub t_pw: i16,
        pub t_sl: i16,
        pub t_sw: i16,
        pub t_pad: [u16; 3],
    }

    pub struct sockstat {
        pub inp_ppcb: u64,
        pub so_addr: u64,
        pub so_pcb: u64,
        pub unp_conn: u64,
        pub dom_family: c_int,
        pub proto: c_int,
        pub so_rcv_sb_state: c_int,
        pub so_snd_sb_state: c_int,
        /// Socket address.
        pub sa_local: crate::sockaddr_storage,
        /// Peer address.
        pub sa_peer: crate::sockaddr_storage,
        pub type_: c_int,
        pub dname: [c_char; 32],
        #[cfg(any(freebsd12, freebsd13, freebsd14, freebsd15))]
        pub sendq: c_uint,
        #[cfg(any(freebsd12, freebsd13, freebsd14, freebsd15))]
        pub recvq: c_uint,
    }

    pub struct shmstat {
        pub size: u64,
        pub mode: u16,
    }

    pub struct spacectl_range {
        pub r_offset: off_t,
        pub r_len: off_t,
    }

    pub struct rusage_ext {
        pub rux_runtime: u64,
        pub rux_uticks: u64,
        pub rux_sticks: u64,
        pub rux_iticks: u64,
        pub rux_uu: u64,
        pub rux_su: u64,
        pub rux_tu: u64,
    }

    pub struct if_clonereq {
        pub ifcr_total: c_int,
        pub ifcr_count: c_int,
        pub ifcr_buffer: *mut c_char,
    }

    pub struct if_msghdr {
        /// to skip over non-understood messages
        pub ifm_msglen: c_ushort,
        /// future binary compatibility
        pub ifm_version: c_uchar,
        /// message type
        pub ifm_type: c_uchar,
        /// like rtm_addrs
        pub ifm_addrs: c_int,
        /// value of if_flags
        pub ifm_flags: c_int,
        /// index for associated ifp
        pub ifm_index: c_ushort,
        pub _ifm_spare1: c_ushort,
        /// statistics and other data about if
        pub ifm_data: if_data,
    }

    pub struct if_msghdrl {
        /// to skip over non-understood messages
        pub ifm_msglen: c_ushort,
        /// future binary compatibility
        pub ifm_version: c_uchar,
        /// message type
        pub ifm_type: c_uchar,
        /// like rtm_addrs
        pub ifm_addrs: c_int,
        /// value of if_flags
        pub ifm_flags: c_int,
        /// index for associated ifp
        pub ifm_index: c_ushort,
        /// spare space to grow if_index, see if_var.h
        pub _ifm_spare1: c_ushort,
        /// length of if_msghdrl incl. if_data
        pub ifm_len: c_ushort,
        /// offset of if_data from beginning
        pub ifm_data_off: c_ushort,
        pub _ifm_spare2: c_int,
        /// statistics and other data about if
        pub ifm_data: if_data,
    }

    pub struct ifa_msghdr {
        /// to skip over non-understood messages
        pub ifam_msglen: c_ushort,
        /// future binary compatibility
        pub ifam_version: c_uchar,
        /// message type
        pub ifam_type: c_uchar,
        /// like rtm_addrs
        pub ifam_addrs: c_int,
        /// value of ifa_flags
        pub ifam_flags: c_int,
        /// index for associated ifp
        pub ifam_index: c_ushort,
        pub _ifam_spare1: c_ushort,
        /// value of ifa_ifp->if_metric
        pub ifam_metric: c_int,
    }

    pub struct ifa_msghdrl {
        /// to skip over non-understood messages
        pub ifam_msglen: c_ushort,
        /// future binary compatibility
        pub ifam_version: c_uchar,
        /// message type
        pub ifam_type: c_uchar,
        /// like rtm_addrs
        pub ifam_addrs: c_int,
        /// value of ifa_flags
        pub ifam_flags: c_int,
        /// index for associated ifp
        pub ifam_index: c_ushort,
        /// spare space to grow if_index, see if_var.h
        pub _ifam_spare1: c_ushort,
        /// length of ifa_msghdrl incl. if_data
        pub ifam_len: c_ushort,
        /// offset of if_data from beginning
        pub ifam_data_off: c_ushort,
        /// value of ifa_ifp->if_metric
        pub ifam_metric: c_int,
        /// statistics and other data about if or address
        pub ifam_data: if_data,
    }

    pub struct ifma_msghdr {
        /// to skip over non-understood messages
        pub ifmam_msglen: c_ushort,
        /// future binary compatibility
        pub ifmam_version: c_uchar,
        /// message type
        pub ifmam_type: c_uchar,
        /// like rtm_addrs
        pub ifmam_addrs: c_int,
        /// value of ifa_flags
        pub ifmam_flags: c_int,
        /// index for associated ifp
        pub ifmam_index: c_ushort,
        pub _ifmam_spare1: c_ushort,
    }

    pub struct if_announcemsghdr {
        /// to skip over non-understood messages
        pub ifan_msglen: c_ushort,
        /// future binary compatibility
        pub ifan_version: c_uchar,
        /// message type
        pub ifan_type: c_uchar,
        /// index for associated ifp
        pub ifan_index: c_ushort,
        /// if name, e.g. "en0"
        pub ifan_name: [c_char; crate::IFNAMSIZ as usize],
        /// what type of announcement
        pub ifan_what: c_ushort,
    }

    pub struct ifreq_buffer {
        pub length: size_t,
        pub buffer: *mut c_void,
    }

    pub struct ifaliasreq {
        /// if name, e.g. "en0"
        pub ifra_name: [c_char; crate::IFNAMSIZ as usize],
        pub ifra_addr: crate::sockaddr,
        pub ifra_broadaddr: crate::sockaddr,
        pub ifra_mask: crate::sockaddr,
        pub ifra_vhid: c_int,
    }

    /// 9.x compat
    pub struct oifaliasreq {
        /// if name, e.g. "en0"
        pub ifra_name: [c_char; crate::IFNAMSIZ as usize],
        pub ifra_addr: crate::sockaddr,
        pub ifra_broadaddr: crate::sockaddr,
        pub ifra_mask: crate::sockaddr,
    }

    pub struct ifmediareq {
        /// if name, e.g. "en0"
        pub ifm_name: [c_char; crate::IFNAMSIZ as usize],
        /// current media options
        pub ifm_current: c_int,
        /// don't care mask
        pub ifm_mask: c_int,
        /// media status
        pub ifm_status: c_int,
        /// active options
        pub ifm_active: c_int,
        /// # entries in ifm_ulist array
        pub ifm_count: c_int,
        /// media words
        pub ifm_ulist: *mut c_int,
    }

    pub struct ifdrv {
        /// if name, e.g. "en0"
        pub ifd_name: [c_char; crate::IFNAMSIZ as usize],
        pub ifd_cmd: c_ulong,
        pub ifd_len: size_t,
        pub ifd_data: *mut c_void,
    }

    pub struct ifi2creq {
        /// i2c address (0xA0, 0xA2)
        pub dev_addr: u8,
        /// read offset
        pub offset: u8,
        /// read length
        pub len: u8,
        pub spare0: u8,
        pub spare1: u32,
        /// read buffer
        pub data: [u8; 8],
    }

    pub struct ifrsshash {
        /// if name, e.g. "en0"
        pub ifrh_name: [c_char; crate::IFNAMSIZ as usize],
        /// RSS_FUNC_
        pub ifrh_func: u8,
        pub ifrh_spare0: u8,
        pub ifrh_spare1: u16,
        /// RSS_TYPE_
        pub ifrh_types: u32,
    }

    pub struct ifmibdata {
        /// name of interface
        pub ifmd_name: [c_char; crate::IFNAMSIZ as usize],
        /// number of promiscuous listeners
        pub ifmd_pcount: c_int,
        /// interface flags
        pub ifmd_flags: c_int,
        /// instantaneous length of send queue
        pub ifmd_snd_len: c_int,
        /// maximum length of send queue
        pub ifmd_snd_maxlen: c_int,
        /// number of drops in send queue
        pub ifmd_snd_drops: c_int,
        /// for future expansion
        pub ifmd_filler: [c_int; 4],
        /// generic information and statistics
        pub ifmd_data: if_data,
    }

    pub struct ifmib_iso_8802_3 {
        pub dot3StatsAlignmentErrors: u32,
        pub dot3StatsFCSErrors: u32,
        pub dot3StatsSingleCollisionFrames: u32,
        pub dot3StatsMultipleCollisionFrames: u32,
        pub dot3StatsSQETestErrors: u32,
        pub dot3StatsDeferredTransmissions: u32,
        pub dot3StatsLateCollisions: u32,
        pub dot3StatsExcessiveCollisions: u32,
        pub dot3StatsInternalMacTransmitErrors: u32,
        pub dot3StatsCarrierSenseErrors: u32,
        pub dot3StatsFrameTooLongs: u32,
        pub dot3StatsInternalMacReceiveErrors: u32,
        pub dot3StatsEtherChipSet: u32,
        pub dot3StatsMissedFrames: u32,
        pub dot3StatsCollFrequencies: [u32; 16],
        pub dot3Compliance: u32,
    }

    pub struct __c_anonymous_ph {
        pub ph1: u64,
        pub ph2: u64,
    }

    pub struct fid {
        pub fid_len: c_ushort,
        pub fid_data0: c_ushort,
        pub fid_data: [c_char; crate::MAXFIDSZ as usize],
    }

    pub struct fhandle {
        pub fh_fsid: crate::fsid_t,
        pub fh_fid: fid,
    }

    pub struct bintime {
        pub sec: crate::time_t,
        pub frac: u64,
    }

    pub struct clockinfo {
        /// clock frequency
        pub hz: c_int,
        /// micro-seconds per hz tick
        pub tick: c_int,
        pub spare: c_int,
        /// statistics clock frequency
        pub stathz: c_int,
        /// profiling clock frequency
        pub profhz: c_int,
    }

    pub struct __c_anonymous_stailq_entry_devstat {
        pub stqe_next: *mut devstat,
    }

    pub struct devstat {
        /// Update sequence
        pub sequence0: crate::u_int,
        /// Allocated entry
        pub allocated: c_int,
        /// started ops
        pub start_count: crate::u_int,
        /// completed ops
        pub end_count: crate::u_int,
        /// busy time unaccounted for since this time
        pub busy_from: bintime,
        pub dev_links: __c_anonymous_stailq_entry_devstat,
        /// Devstat device number.
        pub device_number: u32,
        pub device_name: [c_char; DEVSTAT_NAME_LEN as usize],
        pub unit_number: c_int,
        pub bytes: [u64; DEVSTAT_N_TRANS_FLAGS as usize],
        pub operations: [u64; DEVSTAT_N_TRANS_FLAGS as usize],
        pub duration: [bintime; DEVSTAT_N_TRANS_FLAGS as usize],
        pub busy_time: bintime,
        /// Time the device was created.
        pub creation_time: bintime,
        /// Block size, bytes
        pub block_size: u32,
        /// The number of simple, ordered, and head of queue tags sent.
        pub tag_types: [u64; 3],
        /// Which statistics are supported by a given device.
        pub flags: devstat_support_flags,
        /// Device type
        pub device_type: devstat_type_flags,
        /// Controls list pos.
        pub priority: devstat_priority,
        /// Identification for GEOM nodes
        pub id: *const c_void,
        /// Update sequence
        pub sequence1: crate::u_int,
    }

    pub struct devstat_match {
        pub match_fields: devstat_match_flags,
        pub device_type: devstat_type_flags,
        pub num_match_categories: c_int,
    }

    pub struct devstat_match_table {
        pub match_str: *const c_char,
        pub type_: devstat_type_flags,
        pub match_field: devstat_match_flags,
    }

    pub struct device_selection {
        pub device_number: u32,
        pub device_name: [c_char; DEVSTAT_NAME_LEN as usize],
        pub unit_number: c_int,
        pub selected: c_int,
        pub bytes: u64,
        pub position: c_int,
    }

    pub struct devinfo {
        pub devices: *mut devstat,
        pub mem_ptr: *mut u8,
        pub generation: c_long,
        pub numdevs: c_int,
    }

    pub struct sockcred2 {
        pub sc_version: c_int,
        pub sc_pid: crate::pid_t,
        pub sc_uid: crate::uid_t,
        pub sc_euid: crate::uid_t,
        pub sc_gid: crate::gid_t,
        pub sc_egid: crate::gid_t,
        pub sc_ngroups: c_int,
        pub sc_groups: [crate::gid_t; 1],
    }

    pub struct ifconf {
        pub ifc_len: c_int,
        pub ifc_ifcu: __c_anonymous_ifc_ifcu,
    }

    pub struct au_mask_t {
        pub am_success: c_uint,
        pub am_failure: c_uint,
    }

    pub struct au_tid_t {
        pub port: u32,
        pub machine: u32,
    }

    pub struct auditinfo_t {
        pub ai_auid: crate::au_id_t,
        pub ai_mask: crate::au_mask_t,
        pub ai_termid: au_tid_t,
        pub ai_asid: crate::au_asid_t,
    }

    pub struct tcp_fastopen {
        pub enable: c_int,
        pub psk: [u8; crate::TCP_FASTOPEN_PSK_LEN as usize],
    }

    pub struct tcp_function_set {
        pub function_set_name: [c_char; crate::TCP_FUNCTION_NAME_LEN_MAX as usize],
        pub pcbcnt: u32,
    }

    // Note: this structure will change in a backwards-incompatible way in
    // FreeBSD 15.
    pub struct tcp_info {
        pub tcpi_state: u8,
        pub __tcpi_ca_state: u8,
        pub __tcpi_retransmits: u8,
        pub __tcpi_probes: u8,
        pub __tcpi_backoff: u8,
        pub tcpi_options: u8,
        pub tcp_snd_wscale: u8,
        pub tcp_rcv_wscale: u8,
        pub tcpi_rto: u32,
        pub __tcpi_ato: u32,
        pub tcpi_snd_mss: u32,
        pub tcpi_rcv_mss: u32,
        pub __tcpi_unacked: u32,
        pub __tcpi_sacked: u32,
        pub __tcpi_lost: u32,
        pub __tcpi_retrans: u32,
        pub __tcpi_fackets: u32,
        pub __tcpi_last_data_sent: u32,
        pub __tcpi_last_ack_sent: u32,
        pub tcpi_last_data_recv: u32,
        pub __tcpi_last_ack_recv: u32,
        pub __tcpi_pmtu: u32,
        pub __tcpi_rcv_ssthresh: u32,
        pub tcpi_rtt: u32,
        pub tcpi_rttvar: u32,
        pub tcpi_snd_ssthresh: u32,
        pub tcpi_snd_cwnd: u32,
        pub __tcpi_advmss: u32,
        pub __tcpi_reordering: u32,
        pub __tcpi_rcv_rtt: u32,
        pub tcpi_rcv_space: u32,
        pub tcpi_snd_wnd: u32,
        pub tcpi_snd_bwnd: u32,
        pub tcpi_snd_nxt: u32,
        pub tcpi_rcv_nxt: u32,
        pub tcpi_toe_tid: u32,
        pub tcpi_snd_rexmitpack: u32,
        pub tcpi_rcv_ooopack: u32,
        pub tcpi_snd_zerowin: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub tcpi_delivered_ce: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub tcpi_received_ce: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub __tcpi_delivered_e1_bytes: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub __tcpi_delivered_e0_bytes: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub __tcpi_delivered_ce_bytes: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub __tcpi_received_e1_bytes: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub __tcpi_received_e0_bytes: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub __tcpi_received_ce_bytes: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub tcpi_total_tlp: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub tcpi_total_tlp_bytes: u64,
        #[cfg(any(freebsd15, freebsd14))]
        pub tcpi_snd_una: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub tcpi_snd_max: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub tcpi_rcv_numsacks: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub tcpi_rcv_adv: u32,
        #[cfg(any(freebsd15, freebsd14))]
        pub tcpi_dupacks: u32,
        #[cfg(freebsd14)]
        pub __tcpi_pad: [u32; 10],
        #[cfg(freebsd15)]
        pub __tcpi_pad: [u32; 14],
        #[cfg(not(any(freebsd15, freebsd14)))]
        pub __tcpi_pad: [u32; 26],
    }

    pub struct _umtx_time {
        pub _timeout: crate::timespec,
        pub _flags: u32,
        pub _clockid: u32,
    }

    pub struct shm_largepage_conf {
        pub psind: c_int,
        pub alloc_policy: c_int,
        __pad: [c_int; 10],
    }

    pub struct memory_type {
        __priva: [crate::uintptr_t; 32],
        __privb: [crate::uintptr_t; 26],
    }

    pub struct memory_type_list {
        __priv: [crate::uintptr_t; 2],
    }

    pub struct pidfh {
        __priva: [[crate::uintptr_t; 32]; 8],
        __privb: [crate::uintptr_t; 2],
    }

    pub struct sctp_event {
        pub se_assoc_id: crate::sctp_assoc_t,
        pub se_type: u16,
        pub se_on: u8,
    }

    pub struct sctp_event_subscribe {
        pub sctp_data_io_event: u8,
        pub sctp_association_event: u8,
        pub sctp_address_event: u8,
        pub sctp_send_failure_event: u8,
        pub sctp_peer_error_event: u8,
        pub sctp_shutdown_event: u8,
        pub sctp_partial_delivery_event: u8,
        pub sctp_adaptation_layer_event: u8,
        pub sctp_authentication_event: u8,
        pub sctp_sender_dry_event: u8,
        pub sctp_stream_reset_event: u8,
    }

    pub struct sctp_initmsg {
        pub sinit_num_ostreams: u16,
        pub sinit_max_instreams: u16,
        pub sinit_max_attempts: u16,
        pub sinit_max_init_timeo: u16,
    }

    pub struct sctp_sndrcvinfo {
        pub sinfo_stream: u16,
        pub sinfo_ssn: u16,
        pub sinfo_flags: u16,
        pub sinfo_ppid: u32,
        pub sinfo_context: u32,
        pub sinfo_timetolive: u32,
        pub sinfo_tsn: u32,
        pub sinfo_cumtsn: u32,
        pub sinfo_assoc_id: crate::sctp_assoc_t,
        pub sinfo_keynumber: u16,
        pub sinfo_keynumber_valid: u16,
        pub __reserve_pad: [[u8; 23]; 4],
    }

    pub struct sctp_extrcvinfo {
        pub sinfo_stream: u16,
        pub sinfo_ssn: u16,
        pub sinfo_flags: u16,
        pub sinfo_ppid: u32,
        pub sinfo_context: u32,
        pub sinfo_timetolive: u32,
        pub sinfo_tsn: u32,
        pub sinfo_cumtsn: u32,
        pub sinfo_assoc_id: crate::sctp_assoc_t,
        pub serinfo_next_flags: u16,
        pub serinfo_next_stream: u16,
        pub serinfo_next_aid: u32,
        pub serinfo_next_length: u32,
        pub serinfo_next_ppid: u32,
        pub sinfo_keynumber: u16,
        pub sinfo_keynumber_valid: u16,
        pub __reserve_pad: [[u8; 19]; 4],
    }

    pub struct sctp_sndinfo {
        pub snd_sid: u16,
        pub snd_flags: u16,
        pub snd_ppid: u32,
        pub snd_context: u32,
        pub snd_assoc_id: crate::sctp_assoc_t,
    }

    pub struct sctp_prinfo {
        pub pr_policy: u16,
        pub pr_value: u32,
    }

    pub struct sctp_default_prinfo {
        pub pr_policy: u16,
        pub pr_value: u32,
        pub pr_assoc_id: crate::sctp_assoc_t,
    }

    pub struct sctp_authinfo {
        pub auth_keynumber: u16,
    }

    pub struct sctp_rcvinfo {
        pub rcv_sid: u16,
        pub rcv_ssn: u16,
        pub rcv_flags: u16,
        pub rcv_ppid: u32,
        pub rcv_tsn: u32,
        pub rcv_cumtsn: u32,
        pub rcv_context: u32,
        pub rcv_assoc_id: crate::sctp_assoc_t,
    }

    pub struct sctp_nxtinfo {
        pub nxt_sid: u16,
        pub nxt_flags: u16,
        pub nxt_ppid: u32,
        pub nxt_length: u32,
        pub nxt_assoc_id: crate::sctp_assoc_t,
    }

    pub struct sctp_recvv_rn {
        pub recvv_rcvinfo: sctp_rcvinfo,
        pub recvv_nxtinfo: sctp_nxtinfo,
    }

    pub struct sctp_sendv_spa {
        pub sendv_flags: u32,
        pub sendv_sndinfo: sctp_sndinfo,
        pub sendv_prinfo: sctp_prinfo,
        pub sendv_authinfo: sctp_authinfo,
    }

    pub struct sctp_snd_all_completes {
        pub sall_stream: u16,
        pub sall_flags: u16,
        pub sall_ppid: u32,
        pub sall_context: u32,
        pub sall_num_sent: u32,
        pub sall_num_failed: u32,
    }

    pub struct sctp_pcbinfo {
        pub ep_count: u32,
        pub asoc_count: u32,
        pub laddr_count: u32,
        pub raddr_count: u32,
        pub chk_count: u32,
        pub readq_count: u32,
        pub free_chunks: u32,
        pub stream_oque: u32,
    }

    pub struct sctp_sockstat {
        pub ss_assoc_id: crate::sctp_assoc_t,
        pub ss_total_sndbuf: u32,
        pub ss_total_recv_buf: u32,
    }

    pub struct sctp_assoc_change {
        pub sac_type: u16,
        pub sac_flags: u16,
        pub sac_length: u32,
        pub sac_state: u16,
        pub sac_error: u16,
        pub sac_outbound_streams: u16,
        pub sac_inbound_streams: u16,
        pub sac_assoc_id: crate::sctp_assoc_t,
        pub sac_info: [u8; 0],
    }

    pub struct sctp_paddr_change {
        pub spc_type: u16,
        pub spc_flags: u16,
        pub spc_length: u32,
        pub spc_aaddr: crate::sockaddr_storage,
        pub spc_state: u32,
        pub spc_error: u32,
        pub spc_assoc_id: crate::sctp_assoc_t,
    }

    pub struct sctp_remote_error {
        pub sre_type: u16,
        pub sre_flags: u16,
        pub sre_length: u32,
        pub sre_error: u16,
        pub sre_assoc_id: crate::sctp_assoc_t,
        pub sre_data: [u8; 0],
    }

    pub struct sctp_send_failed_event {
        pub ssfe_type: u16,
        pub ssfe_flags: u16,
        pub ssfe_length: u32,
        pub ssfe_error: u32,
        pub ssfe_info: sctp_sndinfo,
        pub ssfe_assoc_id: crate::sctp_assoc_t,
        pub ssfe_data: [u8; 0],
    }

    pub struct sctp_shutdown_event {
        pub sse_type: u16,
        pub sse_flags: u16,
        pub sse_length: u32,
        pub sse_assoc_id: crate::sctp_assoc_t,
    }

    pub struct sctp_adaptation_event {
        pub sai_type: u16,
        pub sai_flags: u16,
        pub sai_length: u32,
        pub sai_adaptation_ind: u32,
        pub sai_assoc_id: crate::sctp_assoc_t,
    }

    pub struct sctp_setadaptation {
        pub ssb_adaptation_ind: u32,
    }

    pub struct sctp_pdapi_event {
        pub pdapi_type: u16,
        pub pdapi_flags: u16,
        pub pdapi_length: u32,
        pub pdapi_indication: u32,
        pub pdapi_stream: u16,
        pub pdapi_seq: u16,
        pub pdapi_assoc_id: crate::sctp_assoc_t,
    }

    pub struct sctp_sender_dry_event {
        pub sender_dry_type: u16,
        pub sender_dry_flags: u16,
        pub sender_dry_length: u32,
        pub sender_dry_assoc_id: crate::sctp_assoc_t,
    }

    pub struct sctp_stream_reset_event {
        pub strreset_type: u16,
        pub strreset_flags: u16,
        pub strreset_length: u32,
        pub strreset_assoc_id: crate::sctp_assoc_t,
        pub strreset_stream_list: [u16; 0],
    }

    pub struct sctp_stream_change_event {
        pub strchange_type: u16,
        pub strchange_flags: u16,
        pub strchange_length: u32,
        pub strchange_assoc_id: crate::sctp_assoc_t,
        pub strchange_instrms: u16,
        pub strchange_outstrms: u16,
    }

    pub struct filedesc {
        pub fd_files: *mut fdescenttbl,
        pub fd_map: *mut c_ulong,
        pub fd_freefile: c_int,
        pub fd_refcnt: c_int,
        pub fd_holdcnt: c_int,
        fd_sx: sx,
        fd_kqlist: kqlist,
        pub fd_holdleaderscount: c_int,
        pub fd_holdleaderswakeup: c_int,
    }

    pub struct fdescenttbl {
        pub fdt_nfiles: c_int,
        fdt_ofiles: [*mut c_void; 0],
    }

    // FIXME: Should be private.
    #[doc(hidden)]
    pub struct sx {
        lock_object: lock_object,
        sx_lock: crate::uintptr_t,
    }

    // FIXME: Should be private.
    #[doc(hidden)]
    pub struct lock_object {
        lo_name: *const c_char,
        lo_flags: c_uint,
        lo_data: c_uint,
        // This is normally `struct  witness`.
        lo_witness: *mut c_void,
    }

    // FIXME: Should be private.
    #[doc(hidden)]
    pub struct kqlist {
        tqh_first: *mut c_void,
        tqh_last: *mut *mut c_void,
    }

    pub struct splice {
        pub sp_fd: c_int,
        pub sp_max: off_t,
        pub sp_idle: crate::timeval,
    }
}

s_no_extra_traits! {
    pub struct utmpx {
        pub ut_type: c_short,
        pub ut_tv: crate::timeval,
        pub ut_id: [c_char; 8],
        pub ut_pid: crate::pid_t,
        pub ut_user: [c_char; 32],
        pub ut_line: [c_char; 16],
        pub ut_host: [c_char; 128],
        pub __ut_spare: [c_char; 64],
    }

    pub union __c_anonymous_cr_pid {
        __cr_unused: *mut c_void,
        pub cr_pid: crate::pid_t,
    }

    pub struct xucred {
        pub cr_version: c_uint,
        pub cr_uid: crate::uid_t,
        pub cr_ngroups: c_short,
        pub cr_groups: [crate::gid_t; 16],
        pub cr_pid__c_anonymous_union: __c_anonymous_cr_pid,
    }

    pub struct sockaddr_dl {
        pub sdl_len: c_uchar,
        pub sdl_family: c_uchar,
        pub sdl_index: c_ushort,
        pub sdl_type: c_uchar,
        pub sdl_nlen: c_uchar,
        pub sdl_alen: c_uchar,
        pub sdl_slen: c_uchar,
        pub sdl_data: [c_char; 46],
    }

    pub struct mq_attr {
        pub mq_flags: c_long,
        pub mq_maxmsg: c_long,
        pub mq_msgsize: c_long,
        pub mq_curmsgs: c_long,
        __reserved: [c_long; 4],
    }

    pub struct sigevent {
        pub sigev_notify: c_int,
        pub sigev_signo: c_int,
        pub sigev_value: crate::sigval,
        //The rest of the structure is actually a union.  We expose only
        //sigev_notify_thread_id because it's the most useful union member.
        pub sigev_notify_thread_id: crate::lwpid_t,
        #[cfg(target_pointer_width = "64")]
        __unused1: c_int,
        __unused2: [c_long; 7],
    }

    pub struct ptsstat {
        #[cfg(any(freebsd12, freebsd13, freebsd14, freebsd15))]
        pub dev: u64,
        #[cfg(not(any(freebsd12, freebsd13, freebsd14, freebsd15)))]
        pub dev: u32,
        pub devname: [c_char; SPECNAMELEN as usize + 1],
    }

    pub union __c_anonymous_elf32_auxv_union {
        pub a_val: c_int,
    }

    pub struct Elf32_Auxinfo {
        pub a_type: c_int,
        pub a_un: __c_anonymous_elf32_auxv_union,
    }

    pub union __c_anonymous_ifi_epoch {
        pub tt: crate::time_t,
        pub ph: u64,
    }

    pub union __c_anonymous_ifi_lastchange {
        pub tv: crate::timeval,
        pub ph: __c_anonymous_ph,
    }

    pub struct if_data {
        /// ethernet, tokenring, etc
        pub ifi_type: u8,
        /// e.g., AUI, Thinnet, 10base-T, etc
        pub ifi_physical: u8,
        /// media address length
        pub ifi_addrlen: u8,
        /// media header length
        pub ifi_hdrlen: u8,
        /// current link state
        pub ifi_link_state: u8,
        /// carp vhid
        pub ifi_vhid: u8,
        /// length of this data struct
        pub ifi_datalen: u16,
        /// maximum transmission unit
        pub ifi_mtu: u32,
        /// routing metric (external only)
        pub ifi_metric: u32,
        /// linespeed
        pub ifi_baudrate: u64,
        /// packets received on interface
        pub ifi_ipackets: u64,
        /// input errors on interface
        pub ifi_ierrors: u64,
        /// packets sent on interface
        pub ifi_opackets: u64,
        /// output errors on interface
        pub ifi_oerrors: u64,
        /// collisions on csma interfaces
        pub ifi_collisions: u64,
        /// total number of octets received
        pub ifi_ibytes: u64,
        /// total number of octets sent
        pub ifi_obytes: u64,
        /// packets received via multicast
        pub ifi_imcasts: u64,
        /// packets sent via multicast
        pub ifi_omcasts: u64,
        /// dropped on input
        pub ifi_iqdrops: u64,
        /// dropped on output
        pub ifi_oqdrops: u64,
        /// destined for unsupported protocol
        pub ifi_noproto: u64,
        /// HW offload capabilities, see IFCAP
        pub ifi_hwassist: u64,
        /// uptime at attach or stat reset
        pub __ifi_epoch: __c_anonymous_ifi_epoch,
        /// time of last administrative change
        pub __ifi_lastchange: __c_anonymous_ifi_lastchange,
    }

    pub union __c_anonymous_ifr_ifru {
        pub ifru_addr: crate::sockaddr,
        pub ifru_dstaddr: crate::sockaddr,
        pub ifru_broadaddr: crate::sockaddr,
        pub ifru_buffer: ifreq_buffer,
        pub ifru_flags: [c_short; 2],
        pub ifru_index: c_short,
        pub ifru_jid: c_int,
        pub ifru_metric: c_int,
        pub ifru_mtu: c_int,
        pub ifru_phys: c_int,
        pub ifru_media: c_int,
        pub ifru_data: crate::caddr_t,
        pub ifru_cap: [c_int; 2],
        pub ifru_fib: c_uint,
        pub ifru_vlan_pcp: c_uchar,
    }

    pub struct ifreq {
        /// if name, e.g. "en0"
        pub ifr_name: [c_char; crate::IFNAMSIZ],
        pub ifr_ifru: __c_anonymous_ifr_ifru,
    }

    pub union __c_anonymous_ifc_ifcu {
        pub ifcu_buf: crate::caddr_t,
        pub ifcu_req: *mut ifreq,
    }

    pub struct ifstat {
        /// if name, e.g. "en0"
        pub ifs_name: [c_char; crate::IFNAMSIZ as usize],
        pub ascii: [c_char; crate::IFSTATMAX as usize + 1],
    }

    pub struct ifrsskey {
        /// if name, e.g. "en0"
        pub ifrk_name: [c_char; crate::IFNAMSIZ as usize],
        /// RSS_FUNC_
        pub ifrk_func: u8,
        pub ifrk_spare0: u8,
        pub ifrk_keylen: u16,
        pub ifrk_key: [u8; crate::RSS_KEYLEN as usize],
    }

    pub struct ifdownreason {
        pub ifdr_name: [c_char; crate::IFNAMSIZ as usize],
        pub ifdr_reason: u32,
        pub ifdr_vendor: u32,
        pub ifdr_msg: [c_char; crate::IFDR_MSG_SIZE as usize],
    }

    #[repr(packed)]
    pub struct sctphdr {
        pub src_port: u16,
        pub dest_port: u16,
        pub v_tag: u32,
        pub checksum: u32,
    }

    #[repr(packed)]
    pub struct sctp_chunkhdr {
        pub chunk_type: u8,
        pub chunk_flags: u8,
        pub chunk_length: u16,
    }

    #[repr(packed)]
    pub struct sctp_paramhdr {
        pub param_type: u16,
        pub param_length: u16,
    }

    #[repr(packed)]
    pub struct sctp_gen_error_cause {
        pub code: u16,
        pub length: u16,
        pub info: [u8; 0],
    }

    #[repr(packed)]
    pub struct sctp_error_cause {
        pub code: u16,
        pub length: u16,
    }

    #[repr(packed)]
    pub struct sctp_error_invalid_stream {
        pub cause: sctp_error_cause,
        pub stream_id: u16,
        __reserved: u16,
    }

    #[repr(packed)]
    pub struct sctp_error_missing_param {
        pub cause: sctp_error_cause,
        pub num_missing_params: u32,
        pub tpe: [u8; 0],
    }

    #[repr(packed)]
    pub struct sctp_error_stale_cookie {
        pub cause: sctp_error_cause,
        pub stale_time: u32,
    }

    #[repr(packed)]
    pub struct sctp_error_out_of_resource {
        pub cause: sctp_error_cause,
    }

    #[repr(packed)]
    pub struct sctp_error_unresolv_addr {
        pub cause: sctp_error_cause,
    }

    #[repr(packed)]
    pub struct sctp_error_unrecognized_chunk {
        pub cause: sctp_error_cause,
        pub ch: sctp_chunkhdr,
    }

    #[repr(packed)]
    pub struct sctp_error_no_user_data {
        pub cause: sctp_error_cause,
        pub tsn: u32,
    }

    #[repr(packed)]
    pub struct sctp_error_auth_invalid_hmac {
        pub cause: sctp_error_cause,
        pub hmac_id: u16,
    }

    pub struct kinfo_file {
        pub kf_structsize: c_int,
        pub kf_type: c_int,
        pub kf_fd: c_int,
        pub kf_ref_count: c_int,
        pub kf_flags: c_int,
        _kf_pad0: c_int,
        pub kf_offset: i64,
        _priv: [u8; 304], // FIXME(freebsd): this is really a giant union
        pub kf_status: u16,
        _kf_pad1: u16,
        _kf_ispare0: c_int,
        pub kf_cap_rights: crate::cap_rights_t,
        _kf_cap_spare: u64,
        pub kf_path: [c_char; crate::PATH_MAX as usize],
    }

    pub struct ucontext_t {
        pub uc_sigmask: crate::sigset_t,
        pub uc_mcontext: crate::mcontext_t,
        pub uc_link: *mut crate::ucontext_t,
        pub uc_stack: crate::stack_t,
        pub uc_flags: c_int,
        __spare__: [c_int; 4],
    }

    #[repr(align(8))]
    pub struct xinpgen {
        pub xig_len: ksize_t,
        pub xig_count: u32,
        _xig_spare32: u32,
        pub xig_gen: inp_gen_t,
        pub xig_sogen: so_gen_t,
        _xig_spare64: [u64; 4],
    }

    pub struct in_addr_4in6 {
        _ia46_pad32: [u32; 3],
        pub ia46_addr4: crate::in_addr,
    }

    pub union in_dependaddr {
        pub id46_addr: crate::in_addr_4in6,
        pub id6_addr: crate::in6_addr,
    }

    pub struct in_endpoints {
        pub ie_fport: u16,
        pub ie_lport: u16,
        pub ie_dependfaddr: crate::in_dependaddr,
        pub ie_dependladdr: crate::in_dependaddr,
        pub ie6_zoneid: u32,
    }

    pub struct in_conninfo {
        pub inc_flags: u8,
        pub inc_len: u8,
        pub inc_fibnum: u16,
        pub inc_ie: crate::in_endpoints,
    }

    pub struct xktls_session_onedir {
        // Note: this field is called `gen` in upstream FreeBSD, but `gen` is
        // reserved keyword in Rust since the 2024 Edition, hence `gennum`.
        pub gennum: u64,
        _rsrv1: [u64; 8],
        _rsrv2: [u32; 8],
        pub iv: [u8; 32],
        pub cipher_algorithm: i32,
        pub auth_algorithm: i32,
        pub cipher_key_len: u16,
        pub iv_len: u16,
        pub auth_key_len: u16,
        pub max_frame_len: u16,
        pub tls_vmajor: u8,
        pub tls_vminor: u8,
        pub tls_hlen: u8,
        pub tls_tlen: u8,
        pub tls_bs: u8,
        pub flags: u8,
        pub drv_st_len: u16,
        pub ifnet: [c_char; 16],
    }

    pub struct xktls_session {
        pub tsz: u32,
        pub fsz: u32,
        pub inp_gencnt: u64,
        pub so_pcb: kvaddr_t,
        pub coninf: crate::in_conninfo,
        pub rx_vlan_id: c_ushort,
        pub rcv: crate::xktls_session_onedir,
        pub snd: crate::xktls_session_onedir,
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
                    && self
                        .__ut_spare
                        .iter()
                        .zip(other.__ut_spare.iter())
                        .all(|(a, b)| a == b)
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
                self.__ut_spare.hash(state);
            }
        }

        impl PartialEq for __c_anonymous_cr_pid {
            fn eq(&self, other: &__c_anonymous_cr_pid) -> bool {
                unsafe { self.cr_pid == other.cr_pid }
            }
        }
        impl Eq for __c_anonymous_cr_pid {}
        impl hash::Hash for __c_anonymous_cr_pid {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe { self.cr_pid.hash(state) };
            }
        }

        impl PartialEq for xucred {
            fn eq(&self, other: &xucred) -> bool {
                self.cr_version == other.cr_version
                    && self.cr_uid == other.cr_uid
                    && self.cr_ngroups == other.cr_ngroups
                    && self.cr_groups == other.cr_groups
                    && self.cr_pid__c_anonymous_union == other.cr_pid__c_anonymous_union
            }
        }
        impl Eq for xucred {}
        impl hash::Hash for xucred {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.cr_version.hash(state);
                self.cr_uid.hash(state);
                self.cr_ngroups.hash(state);
                self.cr_groups.hash(state);
                self.cr_pid__c_anonymous_union.hash(state);
            }
        }

        impl PartialEq for sockaddr_dl {
            fn eq(&self, other: &sockaddr_dl) -> bool {
                self.sdl_len == other.sdl_len
                    && self.sdl_family == other.sdl_family
                    && self.sdl_index == other.sdl_index
                    && self.sdl_type == other.sdl_type
                    && self.sdl_nlen == other.sdl_nlen
                    && self.sdl_alen == other.sdl_alen
                    && self.sdl_slen == other.sdl_slen
                    && self
                        .sdl_data
                        .iter()
                        .zip(other.sdl_data.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for sockaddr_dl {}
        impl hash::Hash for sockaddr_dl {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sdl_len.hash(state);
                self.sdl_family.hash(state);
                self.sdl_index.hash(state);
                self.sdl_type.hash(state);
                self.sdl_nlen.hash(state);
                self.sdl_alen.hash(state);
                self.sdl_slen.hash(state);
                self.sdl_data.hash(state);
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

        impl PartialEq for sigevent {
            fn eq(&self, other: &sigevent) -> bool {
                self.sigev_notify == other.sigev_notify
                    && self.sigev_signo == other.sigev_signo
                    && self.sigev_value == other.sigev_value
                    && self.sigev_notify_thread_id == other.sigev_notify_thread_id
            }
        }
        impl Eq for sigevent {}
        impl hash::Hash for sigevent {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sigev_notify.hash(state);
                self.sigev_signo.hash(state);
                self.sigev_value.hash(state);
                self.sigev_notify_thread_id.hash(state);
            }
        }

        impl PartialEq for ptsstat {
            fn eq(&self, other: &ptsstat) -> bool {
                let self_devname: &[c_char] = &self.devname;
                let other_devname: &[c_char] = &other.devname;

                self.dev == other.dev && self_devname == other_devname
            }
        }
        impl Eq for ptsstat {}
        impl hash::Hash for ptsstat {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                let self_devname: &[c_char] = &self.devname;

                self.dev.hash(state);
                self_devname.hash(state);
            }
        }

        impl PartialEq for __c_anonymous_elf32_auxv_union {
            fn eq(&self, other: &__c_anonymous_elf32_auxv_union) -> bool {
                unsafe { self.a_val == other.a_val }
            }
        }
        impl Eq for __c_anonymous_elf32_auxv_union {}
        impl PartialEq for Elf32_Auxinfo {
            fn eq(&self, other: &Elf32_Auxinfo) -> bool {
                self.a_type == other.a_type && self.a_un == other.a_un
            }
        }
        impl Eq for Elf32_Auxinfo {}

        impl PartialEq for __c_anonymous_ifr_ifru {
            fn eq(&self, other: &__c_anonymous_ifr_ifru) -> bool {
                unsafe {
                    self.ifru_addr == other.ifru_addr
                        && self.ifru_dstaddr == other.ifru_dstaddr
                        && self.ifru_broadaddr == other.ifru_broadaddr
                        && self.ifru_buffer == other.ifru_buffer
                        && self.ifru_flags == other.ifru_flags
                        && self.ifru_index == other.ifru_index
                        && self.ifru_jid == other.ifru_jid
                        && self.ifru_metric == other.ifru_metric
                        && self.ifru_mtu == other.ifru_mtu
                        && self.ifru_phys == other.ifru_phys
                        && self.ifru_media == other.ifru_media
                        && self.ifru_data == other.ifru_data
                        && self.ifru_cap == other.ifru_cap
                        && self.ifru_fib == other.ifru_fib
                        && self.ifru_vlan_pcp == other.ifru_vlan_pcp
                }
            }
        }
        impl Eq for __c_anonymous_ifr_ifru {}
        impl hash::Hash for __c_anonymous_ifr_ifru {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe { self.ifru_addr.hash(state) };
                unsafe { self.ifru_dstaddr.hash(state) };
                unsafe { self.ifru_broadaddr.hash(state) };
                unsafe { self.ifru_buffer.hash(state) };
                unsafe { self.ifru_flags.hash(state) };
                unsafe { self.ifru_index.hash(state) };
                unsafe { self.ifru_jid.hash(state) };
                unsafe { self.ifru_metric.hash(state) };
                unsafe { self.ifru_mtu.hash(state) };
                unsafe { self.ifru_phys.hash(state) };
                unsafe { self.ifru_media.hash(state) };
                unsafe { self.ifru_data.hash(state) };
                unsafe { self.ifru_cap.hash(state) };
                unsafe { self.ifru_fib.hash(state) };
                unsafe { self.ifru_vlan_pcp.hash(state) };
            }
        }

        impl PartialEq for ifreq {
            fn eq(&self, other: &ifreq) -> bool {
                self.ifr_name == other.ifr_name && self.ifr_ifru == other.ifr_ifru
            }
        }
        impl Eq for ifreq {}
        impl hash::Hash for ifreq {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ifr_name.hash(state);
                self.ifr_ifru.hash(state);
            }
        }

        impl Eq for __c_anonymous_ifc_ifcu {}

        impl PartialEq for __c_anonymous_ifc_ifcu {
            fn eq(&self, other: &__c_anonymous_ifc_ifcu) -> bool {
                unsafe { self.ifcu_buf == other.ifcu_buf && self.ifcu_req == other.ifcu_req }
            }
        }

        impl hash::Hash for __c_anonymous_ifc_ifcu {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe { self.ifcu_buf.hash(state) };
                unsafe { self.ifcu_req.hash(state) };
            }
        }

        impl PartialEq for ifstat {
            fn eq(&self, other: &ifstat) -> bool {
                let self_ascii: &[c_char] = &self.ascii;
                let other_ascii: &[c_char] = &other.ascii;

                self.ifs_name == other.ifs_name && self_ascii == other_ascii
            }
        }
        impl Eq for ifstat {}
        impl hash::Hash for ifstat {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ifs_name.hash(state);
                self.ascii.hash(state);
            }
        }

        impl PartialEq for ifrsskey {
            fn eq(&self, other: &ifrsskey) -> bool {
                let self_ifrk_key: &[u8] = &self.ifrk_key;
                let other_ifrk_key: &[u8] = &other.ifrk_key;

                self.ifrk_name == other.ifrk_name
                    && self.ifrk_func == other.ifrk_func
                    && self.ifrk_spare0 == other.ifrk_spare0
                    && self.ifrk_keylen == other.ifrk_keylen
                    && self_ifrk_key == other_ifrk_key
            }
        }
        impl Eq for ifrsskey {}
        impl hash::Hash for ifrsskey {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ifrk_name.hash(state);
                self.ifrk_func.hash(state);
                self.ifrk_spare0.hash(state);
                self.ifrk_keylen.hash(state);
                self.ifrk_key.hash(state);
            }
        }

        impl PartialEq for ifdownreason {
            fn eq(&self, other: &ifdownreason) -> bool {
                let self_ifdr_msg: &[c_char] = &self.ifdr_msg;
                let other_ifdr_msg: &[c_char] = &other.ifdr_msg;

                self.ifdr_name == other.ifdr_name
                    && self.ifdr_reason == other.ifdr_reason
                    && self.ifdr_vendor == other.ifdr_vendor
                    && self_ifdr_msg == other_ifdr_msg
            }
        }
        impl Eq for ifdownreason {}
        impl hash::Hash for ifdownreason {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ifdr_name.hash(state);
                self.ifdr_reason.hash(state);
                self.ifdr_vendor.hash(state);
                self.ifdr_msg.hash(state);
            }
        }

        impl PartialEq for __c_anonymous_ifi_epoch {
            fn eq(&self, other: &__c_anonymous_ifi_epoch) -> bool {
                unsafe { self.tt == other.tt && self.ph == other.ph }
            }
        }
        impl Eq for __c_anonymous_ifi_epoch {}
        impl hash::Hash for __c_anonymous_ifi_epoch {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    self.tt.hash(state);
                    self.ph.hash(state);
                }
            }
        }

        impl PartialEq for __c_anonymous_ifi_lastchange {
            fn eq(&self, other: &__c_anonymous_ifi_lastchange) -> bool {
                unsafe { self.tv == other.tv && self.ph == other.ph }
            }
        }
        impl Eq for __c_anonymous_ifi_lastchange {}
        impl hash::Hash for __c_anonymous_ifi_lastchange {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    self.tv.hash(state);
                    self.ph.hash(state);
                }
            }
        }

        impl PartialEq for if_data {
            fn eq(&self, other: &if_data) -> bool {
                self.ifi_type == other.ifi_type
                    && self.ifi_physical == other.ifi_physical
                    && self.ifi_addrlen == other.ifi_addrlen
                    && self.ifi_hdrlen == other.ifi_hdrlen
                    && self.ifi_link_state == other.ifi_link_state
                    && self.ifi_vhid == other.ifi_vhid
                    && self.ifi_datalen == other.ifi_datalen
                    && self.ifi_mtu == other.ifi_mtu
                    && self.ifi_metric == other.ifi_metric
                    && self.ifi_baudrate == other.ifi_baudrate
                    && self.ifi_ipackets == other.ifi_ipackets
                    && self.ifi_ierrors == other.ifi_ierrors
                    && self.ifi_opackets == other.ifi_opackets
                    && self.ifi_oerrors == other.ifi_oerrors
                    && self.ifi_collisions == other.ifi_collisions
                    && self.ifi_ibytes == other.ifi_ibytes
                    && self.ifi_obytes == other.ifi_obytes
                    && self.ifi_imcasts == other.ifi_imcasts
                    && self.ifi_omcasts == other.ifi_omcasts
                    && self.ifi_iqdrops == other.ifi_iqdrops
                    && self.ifi_oqdrops == other.ifi_oqdrops
                    && self.ifi_noproto == other.ifi_noproto
                    && self.ifi_hwassist == other.ifi_hwassist
                    && self.__ifi_epoch == other.__ifi_epoch
                    && self.__ifi_lastchange == other.__ifi_lastchange
            }
        }
        impl Eq for if_data {}
        impl hash::Hash for if_data {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ifi_type.hash(state);
                self.ifi_physical.hash(state);
                self.ifi_addrlen.hash(state);
                self.ifi_hdrlen.hash(state);
                self.ifi_link_state.hash(state);
                self.ifi_vhid.hash(state);
                self.ifi_datalen.hash(state);
                self.ifi_mtu.hash(state);
                self.ifi_metric.hash(state);
                self.ifi_baudrate.hash(state);
                self.ifi_ipackets.hash(state);
                self.ifi_ierrors.hash(state);
                self.ifi_opackets.hash(state);
                self.ifi_oerrors.hash(state);
                self.ifi_collisions.hash(state);
                self.ifi_ibytes.hash(state);
                self.ifi_obytes.hash(state);
                self.ifi_imcasts.hash(state);
                self.ifi_omcasts.hash(state);
                self.ifi_iqdrops.hash(state);
                self.ifi_oqdrops.hash(state);
                self.ifi_noproto.hash(state);
                self.ifi_hwassist.hash(state);
                self.__ifi_epoch.hash(state);
                self.__ifi_lastchange.hash(state);
            }
        }

        impl PartialEq for sctphdr {
            fn eq(&self, other: &sctphdr) -> bool {
                return { self.src_port } == { other.src_port }
                    && { self.dest_port } == { other.dest_port }
                    && { self.v_tag } == { other.v_tag }
                    && { self.checksum } == { other.checksum };
            }
        }
        impl Eq for sctphdr {}
        impl hash::Hash for sctphdr {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.src_port }.hash(state);
                { self.dest_port }.hash(state);
                { self.v_tag }.hash(state);
                { self.checksum }.hash(state);
            }
        }

        impl PartialEq for sctp_chunkhdr {
            fn eq(&self, other: &sctp_chunkhdr) -> bool {
                return { self.chunk_type } == { other.chunk_type }
                    && { self.chunk_flags } == { other.chunk_flags }
                    && { self.chunk_length } == { other.chunk_length };
            }
        }
        impl Eq for sctp_chunkhdr {}
        impl hash::Hash for sctp_chunkhdr {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.chunk_type }.hash(state);
                { self.chunk_flags }.hash(state);
                { self.chunk_length }.hash(state);
            }
        }

        impl PartialEq for sctp_paramhdr {
            fn eq(&self, other: &sctp_paramhdr) -> bool {
                return { self.param_type } == { other.param_type } && { self.param_length } == {
                    other.param_length
                };
            }
        }
        impl Eq for sctp_paramhdr {}
        impl hash::Hash for sctp_paramhdr {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.param_type }.hash(state);
                { self.param_length }.hash(state);
            }
        }

        impl PartialEq for sctp_gen_error_cause {
            fn eq(&self, other: &sctp_gen_error_cause) -> bool {
                return { self.code } == { other.code } && { self.length } == { other.length } && {
                    self.info
                }
                .iter()
                .zip({ other.info }.iter())
                .all(|(a, b)| a == b);
            }
        }
        impl Eq for sctp_gen_error_cause {}
        impl hash::Hash for sctp_gen_error_cause {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.code }.hash(state);
                { self.length }.hash(state);
                { self.info }.hash(state);
            }
        }

        impl PartialEq for sctp_error_cause {
            fn eq(&self, other: &sctp_error_cause) -> bool {
                return { self.code } == { other.code } && { self.length } == { other.length };
            }
        }
        impl Eq for sctp_error_cause {}
        impl hash::Hash for sctp_error_cause {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.code }.hash(state);
                { self.length }.hash(state);
            }
        }

        impl PartialEq for sctp_error_invalid_stream {
            fn eq(&self, other: &sctp_error_invalid_stream) -> bool {
                return { self.cause } == { other.cause } && { self.stream_id } == {
                    other.stream_id
                };
            }
        }
        impl Eq for sctp_error_invalid_stream {}
        impl hash::Hash for sctp_error_invalid_stream {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.cause }.hash(state);
                { self.stream_id }.hash(state);
            }
        }

        impl PartialEq for sctp_error_missing_param {
            fn eq(&self, other: &sctp_error_missing_param) -> bool {
                return { self.cause } == { other.cause }
                    && { self.num_missing_params } == { other.num_missing_params }
                    && { self.tpe }
                        .iter()
                        .zip({ other.tpe }.iter())
                        .all(|(a, b)| a == b);
            }
        }
        impl Eq for sctp_error_missing_param {}
        impl hash::Hash for sctp_error_missing_param {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.cause }.hash(state);
                { self.num_missing_params }.hash(state);
                { self.tpe }.hash(state);
            }
        }

        impl PartialEq for sctp_error_stale_cookie {
            fn eq(&self, other: &sctp_error_stale_cookie) -> bool {
                return { self.cause } == { other.cause } && { self.stale_time } == {
                    other.stale_time
                };
            }
        }
        impl Eq for sctp_error_stale_cookie {}
        impl hash::Hash for sctp_error_stale_cookie {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.cause }.hash(state);
                { self.stale_time }.hash(state);
            }
        }

        impl PartialEq for sctp_error_out_of_resource {
            fn eq(&self, other: &sctp_error_out_of_resource) -> bool {
                return { self.cause } == { other.cause };
            }
        }
        impl Eq for sctp_error_out_of_resource {}
        impl hash::Hash for sctp_error_out_of_resource {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.cause }.hash(state);
            }
        }

        impl PartialEq for sctp_error_unresolv_addr {
            fn eq(&self, other: &sctp_error_unresolv_addr) -> bool {
                return { self.cause } == { other.cause };
            }
        }
        impl Eq for sctp_error_unresolv_addr {}
        impl hash::Hash for sctp_error_unresolv_addr {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.cause }.hash(state);
            }
        }

        impl PartialEq for sctp_error_unrecognized_chunk {
            fn eq(&self, other: &sctp_error_unrecognized_chunk) -> bool {
                return { self.cause } == { other.cause } && { self.ch } == { other.ch };
            }
        }
        impl Eq for sctp_error_unrecognized_chunk {}
        impl hash::Hash for sctp_error_unrecognized_chunk {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.cause }.hash(state);
                { self.ch }.hash(state);
            }
        }

        impl PartialEq for sctp_error_no_user_data {
            fn eq(&self, other: &sctp_error_no_user_data) -> bool {
                return { self.cause } == { other.cause } && { self.tsn } == { other.tsn };
            }
        }
        impl Eq for sctp_error_no_user_data {}
        impl hash::Hash for sctp_error_no_user_data {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.cause }.hash(state);
                { self.tsn }.hash(state);
            }
        }

        impl PartialEq for sctp_error_auth_invalid_hmac {
            fn eq(&self, other: &sctp_error_auth_invalid_hmac) -> bool {
                return { self.cause } == { other.cause } && { self.hmac_id } == { other.hmac_id };
            }
        }
        impl Eq for sctp_error_auth_invalid_hmac {}
        impl hash::Hash for sctp_error_auth_invalid_hmac {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                { self.cause }.hash(state);
                { self.hmac_id }.hash(state);
            }
        }

        impl PartialEq for kinfo_file {
            fn eq(&self, other: &kinfo_file) -> bool {
                self.kf_structsize == other.kf_structsize
                    && self.kf_type == other.kf_type
                    && self.kf_fd == other.kf_fd
                    && self.kf_ref_count == other.kf_ref_count
                    && self.kf_flags == other.kf_flags
                    && self.kf_offset == other.kf_offset
                    && self.kf_status == other.kf_status
                    && self.kf_cap_rights == other.kf_cap_rights
                    && self
                        .kf_path
                        .iter()
                        .zip(other.kf_path.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for kinfo_file {}
        impl hash::Hash for kinfo_file {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.kf_structsize.hash(state);
                self.kf_type.hash(state);
                self.kf_fd.hash(state);
                self.kf_ref_count.hash(state);
                self.kf_flags.hash(state);
                self.kf_offset.hash(state);
                self.kf_status.hash(state);
                self.kf_cap_rights.hash(state);
                self.kf_path.hash(state);
            }
        }
    }
}

#[cfg_attr(feature = "extra_traits", derive(Debug))]
#[repr(u32)]
pub enum dot3Vendors {
    dot3VendorAMD = 1,
    dot3VendorIntel = 2,
    dot3VendorNational = 4,
    dot3VendorFujitsu = 5,
    dot3VendorDigital = 6,
    dot3VendorWesternDigital = 7,
}
impl Copy for dot3Vendors {}
impl Clone for dot3Vendors {
    fn clone(&self) -> dot3Vendors {
        *self
    }
}

// aio.h
pub const LIO_VECTORED: c_int = 4;
pub const LIO_WRITEV: c_int = 5;
pub const LIO_READV: c_int = 6;

// sys/caprights.h
pub const CAP_RIGHTS_VERSION_00: i32 = 0;
pub const CAP_RIGHTS_VERSION: i32 = CAP_RIGHTS_VERSION_00;

// sys/capsicum.h
macro_rules! cap_right {
    ($idx:expr, $bit:expr) => {
        ((1u64 << (57 + ($idx))) | ($bit))
    };
}
pub const CAP_READ: u64 = cap_right!(0, 0x0000000000000001u64);
pub const CAP_WRITE: u64 = cap_right!(0, 0x0000000000000002u64);
pub const CAP_SEEK_TELL: u64 = cap_right!(0, 0x0000000000000004u64);
pub const CAP_SEEK: u64 = CAP_SEEK_TELL | 0x0000000000000008u64;
pub const CAP_PREAD: u64 = CAP_SEEK | CAP_READ;
pub const CAP_PWRITE: u64 = CAP_SEEK | CAP_WRITE;
pub const CAP_MMAP: u64 = cap_right!(0, 0x0000000000000010u64);
pub const CAP_MMAP_R: u64 = CAP_MMAP | CAP_SEEK | CAP_READ;
pub const CAP_MMAP_W: u64 = CAP_MMAP | CAP_SEEK | CAP_WRITE;
pub const CAP_MMAP_X: u64 = CAP_MMAP | CAP_SEEK | 0x0000000000000020u64;
pub const CAP_MMAP_RW: u64 = CAP_MMAP_R | CAP_MMAP_W;
pub const CAP_MMAP_RX: u64 = CAP_MMAP_R | CAP_MMAP_X;
pub const CAP_MMAP_WX: u64 = CAP_MMAP_W | CAP_MMAP_X;
pub const CAP_MMAP_RWX: u64 = CAP_MMAP_R | CAP_MMAP_W | CAP_MMAP_X;
pub const CAP_CREATE: u64 = cap_right!(0, 0x0000000000000040u64);
pub const CAP_FEXECVE: u64 = cap_right!(0, 0x0000000000000080u64);
pub const CAP_FSYNC: u64 = cap_right!(0, 0x0000000000000100u64);
pub const CAP_FTRUNCATE: u64 = cap_right!(0, 0x0000000000000200u64);
pub const CAP_LOOKUP: u64 = cap_right!(0, 0x0000000000000400u64);
pub const CAP_FCHDIR: u64 = cap_right!(0, 0x0000000000000800u64);
pub const CAP_FCHFLAGS: u64 = cap_right!(0, 0x0000000000001000u64);
pub const CAP_CHFLAGSAT: u64 = CAP_FCHFLAGS | CAP_LOOKUP;
pub const CAP_FCHMOD: u64 = cap_right!(0, 0x0000000000002000u64);
pub const CAP_FCHMODAT: u64 = CAP_FCHMOD | CAP_LOOKUP;
pub const CAP_FCHOWN: u64 = cap_right!(0, 0x0000000000004000u64);
pub const CAP_FCHOWNAT: u64 = CAP_FCHOWN | CAP_LOOKUP;
pub const CAP_FCNTL: u64 = cap_right!(0, 0x0000000000008000u64);
pub const CAP_FLOCK: u64 = cap_right!(0, 0x0000000000010000u64);
pub const CAP_FPATHCONF: u64 = cap_right!(0, 0x0000000000020000u64);
pub const CAP_FSCK: u64 = cap_right!(0, 0x0000000000040000u64);
pub const CAP_FSTAT: u64 = cap_right!(0, 0x0000000000080000u64);
pub const CAP_FSTATAT: u64 = CAP_FSTAT | CAP_LOOKUP;
pub const CAP_FSTATFS: u64 = cap_right!(0, 0x0000000000100000u64);
pub const CAP_FUTIMES: u64 = cap_right!(0, 0x0000000000200000u64);
pub const CAP_FUTIMESAT: u64 = CAP_FUTIMES | CAP_LOOKUP;
// Note: this was named CAP_LINKAT prior to FreeBSD 11.0.
pub const CAP_LINKAT_TARGET: u64 = CAP_LOOKUP | 0x0000000000400000u64;
pub const CAP_MKDIRAT: u64 = CAP_LOOKUP | 0x0000000000800000u64;
pub const CAP_MKFIFOAT: u64 = CAP_LOOKUP | 0x0000000001000000u64;
pub const CAP_MKNODAT: u64 = CAP_LOOKUP | 0x0000000002000000u64;
// Note: this was named CAP_RENAMEAT prior to FreeBSD 11.0.
pub const CAP_RENAMEAT_SOURCE: u64 = CAP_LOOKUP | 0x0000000004000000u64;
pub const CAP_SYMLINKAT: u64 = CAP_LOOKUP | 0x0000000008000000u64;
pub const CAP_UNLINKAT: u64 = CAP_LOOKUP | 0x0000000010000000u64;
pub const CAP_ACCEPT: u64 = cap_right!(0, 0x0000000020000000u64);
pub const CAP_BIND: u64 = cap_right!(0, 0x0000000040000000u64);
pub const CAP_CONNECT: u64 = cap_right!(0, 0x0000000080000000u64);
pub const CAP_GETPEERNAME: u64 = cap_right!(0, 0x0000000100000000u64);
pub const CAP_GETSOCKNAME: u64 = cap_right!(0, 0x0000000200000000u64);
pub const CAP_GETSOCKOPT: u64 = cap_right!(0, 0x0000000400000000u64);
pub const CAP_LISTEN: u64 = cap_right!(0, 0x0000000800000000u64);
pub const CAP_PEELOFF: u64 = cap_right!(0, 0x0000001000000000u64);
pub const CAP_RECV: u64 = CAP_READ;
pub const CAP_SEND: u64 = CAP_WRITE;
pub const CAP_SETSOCKOPT: u64 = cap_right!(0, 0x0000002000000000u64);
pub const CAP_SHUTDOWN: u64 = cap_right!(0, 0x0000004000000000u64);
pub const CAP_BINDAT: u64 = CAP_LOOKUP | 0x0000008000000000u64;
pub const CAP_CONNECTAT: u64 = CAP_LOOKUP | 0x0000010000000000u64;
pub const CAP_LINKAT_SOURCE: u64 = CAP_LOOKUP | 0x0000020000000000u64;
pub const CAP_RENAMEAT_TARGET: u64 = CAP_LOOKUP | 0x0000040000000000u64;
pub const CAP_SOCK_CLIENT: u64 = CAP_CONNECT
    | CAP_GETPEERNAME
    | CAP_GETSOCKNAME
    | CAP_GETSOCKOPT
    | CAP_PEELOFF
    | CAP_RECV
    | CAP_SEND
    | CAP_SETSOCKOPT
    | CAP_SHUTDOWN;
pub const CAP_SOCK_SERVER: u64 = CAP_ACCEPT
    | CAP_BIND
    | CAP_GETPEERNAME
    | CAP_GETSOCKNAME
    | CAP_GETSOCKOPT
    | CAP_LISTEN
    | CAP_PEELOFF
    | CAP_RECV
    | CAP_SEND
    | CAP_SETSOCKOPT
    | CAP_SHUTDOWN;
#[deprecated(since = "0.2.165", note = "Not stable across OS versions")]
pub const CAP_ALL0: u64 = cap_right!(0, 0x000007FFFFFFFFFFu64);
#[deprecated(since = "0.2.165", note = "Not stable across OS versions")]
pub const CAP_UNUSED0_44: u64 = cap_right!(0, 0x0000080000000000u64);
#[deprecated(since = "0.2.165", note = "Not stable across OS versions")]
pub const CAP_UNUSED0_57: u64 = cap_right!(0, 0x0100000000000000u64);
pub const CAP_MAC_GET: u64 = cap_right!(1, 0x0000000000000001u64);
pub const CAP_MAC_SET: u64 = cap_right!(1, 0x0000000000000002u64);
pub const CAP_SEM_GETVALUE: u64 = cap_right!(1, 0x0000000000000004u64);
pub const CAP_SEM_POST: u64 = cap_right!(1, 0x0000000000000008u64);
pub const CAP_SEM_WAIT: u64 = cap_right!(1, 0x0000000000000010u64);
pub const CAP_EVENT: u64 = cap_right!(1, 0x0000000000000020u64);
pub const CAP_KQUEUE_EVENT: u64 = cap_right!(1, 0x0000000000000040u64);
pub const CAP_IOCTL: u64 = cap_right!(1, 0x0000000000000080u64);
pub const CAP_TTYHOOK: u64 = cap_right!(1, 0x0000000000000100u64);
pub const CAP_PDGETPID: u64 = cap_right!(1, 0x0000000000000200u64);
pub const CAP_PDWAIT: u64 = cap_right!(1, 0x0000000000000400u64);
pub const CAP_PDKILL: u64 = cap_right!(1, 0x0000000000000800u64);
pub const CAP_EXTATTR_DELETE: u64 = cap_right!(1, 0x0000000000001000u64);
pub const CAP_EXTATTR_GET: u64 = cap_right!(1, 0x0000000000002000u64);
pub const CAP_EXTATTR_LIST: u64 = cap_right!(1, 0x0000000000004000u64);
pub const CAP_EXTATTR_SET: u64 = cap_right!(1, 0x0000000000008000u64);
pub const CAP_ACL_CHECK: u64 = cap_right!(1, 0x0000000000010000u64);
pub const CAP_ACL_DELETE: u64 = cap_right!(1, 0x0000000000020000u64);
pub const CAP_ACL_GET: u64 = cap_right!(1, 0x0000000000040000u64);
pub const CAP_ACL_SET: u64 = cap_right!(1, 0x0000000000080000u64);
pub const CAP_KQUEUE_CHANGE: u64 = cap_right!(1, 0x0000000000100000u64);
pub const CAP_KQUEUE: u64 = CAP_KQUEUE_EVENT | CAP_KQUEUE_CHANGE;
#[deprecated(since = "0.2.165", note = "Not stable across OS versions")]
pub const CAP_ALL1: u64 = cap_right!(1, 0x00000000001FFFFFu64);
#[deprecated(since = "0.2.165", note = "Not stable across OS versions")]
pub const CAP_UNUSED1_22: u64 = cap_right!(1, 0x0000000000200000u64);
#[deprecated(since = "0.2.165", note = "Not stable across OS versions")]
pub const CAP_UNUSED1_57: u64 = cap_right!(1, 0x0100000000000000u64);
pub const CAP_FCNTL_GETFL: u32 = 1 << 3;
pub const CAP_FCNTL_SETFL: u32 = 1 << 4;
pub const CAP_FCNTL_GETOWN: u32 = 1 << 5;
pub const CAP_FCNTL_SETOWN: u32 = 1 << 6;

// sys/devicestat.h
pub const DEVSTAT_N_TRANS_FLAGS: c_int = 4;
pub const DEVSTAT_NAME_LEN: c_int = 16;

// sys/cpuset.h
cfg_if! {
    if #[cfg(any(freebsd15, freebsd14))] {
        pub const CPU_SETSIZE: c_int = 1024;
    } else {
        pub const CPU_SETSIZE: c_int = 256;
    }
}

pub const SIGEV_THREAD_ID: c_int = 4;

pub const EXTATTR_NAMESPACE_EMPTY: c_int = 0;
pub const EXTATTR_NAMESPACE_USER: c_int = 1;
pub const EXTATTR_NAMESPACE_SYSTEM: c_int = 2;

pub const PTHREAD_STACK_MIN: size_t = MINSIGSTKSZ;
pub const PTHREAD_MUTEX_ADAPTIVE_NP: c_int = 4;
pub const PTHREAD_MUTEX_STALLED: c_int = 0;
pub const PTHREAD_MUTEX_ROBUST: c_int = 1;
pub const SIGSTKSZ: size_t = MINSIGSTKSZ + 32768;
pub const SF_NODISKIO: c_int = 0x00000001;
pub const SF_MNOWAIT: c_int = 0x00000002;
pub const SF_SYNC: c_int = 0x00000004;
pub const SF_USER_READAHEAD: c_int = 0x00000008;
pub const SF_NOCACHE: c_int = 0x00000010;
pub const O_CLOEXEC: c_int = 0x00100000;
pub const O_DIRECTORY: c_int = 0x00020000;
pub const O_DSYNC: c_int = 0x01000000;
pub const O_EMPTY_PATH: c_int = 0x02000000;
pub const O_EXEC: c_int = 0x00040000;
pub const O_PATH: c_int = 0x00400000;
pub const O_RESOLVE_BENEATH: c_int = 0x00800000;
pub const O_SEARCH: c_int = O_EXEC;
pub const O_TTY_INIT: c_int = 0x00080000;
pub const O_VERIFY: c_int = 0x00200000;
pub const F_GETLK: c_int = 11;
pub const F_SETLK: c_int = 12;
pub const F_SETLKW: c_int = 13;
pub const ENOTCAPABLE: c_int = 93;
pub const ECAPMODE: c_int = 94;
pub const ENOTRECOVERABLE: c_int = 95;
pub const EOWNERDEAD: c_int = 96;
pub const EINTEGRITY: c_int = 97;
pub const RLIMIT_NPTS: c_int = 11;
pub const RLIMIT_SWAP: c_int = 12;
pub const RLIMIT_KQUEUES: c_int = 13;
pub const RLIMIT_UMTXP: c_int = 14;
#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIM_NLIMITS: crate::rlim_t = 15;
pub const RLIM_SAVED_MAX: crate::rlim_t = crate::RLIM_INFINITY;
pub const RLIM_SAVED_CUR: crate::rlim_t = crate::RLIM_INFINITY;

pub const CP_USER: c_int = 0;
pub const CP_NICE: c_int = 1;
pub const CP_SYS: c_int = 2;
pub const CP_INTR: c_int = 3;
pub const CP_IDLE: c_int = 4;
pub const CPUSTATES: c_int = 5;

pub const NI_NOFQDN: c_int = 0x00000001;
pub const NI_NUMERICHOST: c_int = 0x00000002;
pub const NI_NAMEREQD: c_int = 0x00000004;
pub const NI_NUMERICSERV: c_int = 0x00000008;
pub const NI_DGRAM: c_int = 0x00000010;
pub const NI_NUMERICSCOPE: c_int = 0x00000020;

pub const XU_NGROUPS: c_int = 16;

pub const Q_GETQUOTA: c_int = 0x700;
pub const Q_SETQUOTA: c_int = 0x800;

pub const MAP_GUARD: c_int = 0x00002000;
pub const MAP_EXCL: c_int = 0x00004000;
pub const MAP_PREFAULT_READ: c_int = 0x00040000;
pub const MAP_ALIGNMENT_SHIFT: c_int = 24;
pub const MAP_ALIGNMENT_MASK: c_int = 0xff << MAP_ALIGNMENT_SHIFT;
pub const MAP_ALIGNED_SUPER: c_int = 1 << MAP_ALIGNMENT_SHIFT;

pub const POSIX_FADV_NORMAL: c_int = 0;
pub const POSIX_FADV_RANDOM: c_int = 1;
pub const POSIX_FADV_SEQUENTIAL: c_int = 2;
pub const POSIX_FADV_WILLNEED: c_int = 3;
pub const POSIX_FADV_DONTNEED: c_int = 4;
pub const POSIX_FADV_NOREUSE: c_int = 5;

pub const POLLINIGNEOF: c_short = 0x2000;
pub const POLLRDHUP: c_short = 0x4000;

pub const EVFILT_READ: i16 = -1;
pub const EVFILT_WRITE: i16 = -2;
pub const EVFILT_AIO: i16 = -3;
pub const EVFILT_VNODE: i16 = -4;
pub const EVFILT_PROC: i16 = -5;
pub const EVFILT_SIGNAL: i16 = -6;
pub const EVFILT_TIMER: i16 = -7;
pub const EVFILT_PROCDESC: i16 = -8;
pub const EVFILT_FS: i16 = -9;
pub const EVFILT_LIO: i16 = -10;
pub const EVFILT_USER: i16 = -11;
pub const EVFILT_SENDFILE: i16 = -12;
pub const EVFILT_EMPTY: i16 = -13;

pub const EV_ADD: u16 = 0x1;
pub const EV_DELETE: u16 = 0x2;
pub const EV_ENABLE: u16 = 0x4;
pub const EV_DISABLE: u16 = 0x8;
pub const EV_FORCEONESHOT: u16 = 0x100;
pub const EV_KEEPUDATA: u16 = 0x200;

pub const EV_ONESHOT: u16 = 0x10;
pub const EV_CLEAR: u16 = 0x20;
pub const EV_RECEIPT: u16 = 0x40;
pub const EV_DISPATCH: u16 = 0x80;
pub const EV_SYSFLAGS: u16 = 0xf000;
pub const EV_DROP: u16 = 0x1000;
pub const EV_FLAG1: u16 = 0x2000;
pub const EV_FLAG2: u16 = 0x4000;

pub const EV_EOF: u16 = 0x8000;
pub const EV_ERROR: u16 = 0x4000;

pub const NOTE_TRIGGER: u32 = 0x01000000;
pub const NOTE_FFNOP: u32 = 0x00000000;
pub const NOTE_FFAND: u32 = 0x40000000;
pub const NOTE_FFOR: u32 = 0x80000000;
pub const NOTE_FFCOPY: u32 = 0xc0000000;
pub const NOTE_FFCTRLMASK: u32 = 0xc0000000;
pub const NOTE_FFLAGSMASK: u32 = 0x00ffffff;
pub const NOTE_LOWAT: u32 = 0x00000001;
pub const NOTE_FILE_POLL: u32 = 0x00000002;
pub const NOTE_DELETE: u32 = 0x00000001;
pub const NOTE_WRITE: u32 = 0x00000002;
pub const NOTE_EXTEND: u32 = 0x00000004;
pub const NOTE_ATTRIB: u32 = 0x00000008;
pub const NOTE_LINK: u32 = 0x00000010;
pub const NOTE_RENAME: u32 = 0x00000020;
pub const NOTE_REVOKE: u32 = 0x00000040;
pub const NOTE_OPEN: u32 = 0x00000080;
pub const NOTE_CLOSE: u32 = 0x00000100;
pub const NOTE_CLOSE_WRITE: u32 = 0x00000200;
pub const NOTE_READ: u32 = 0x00000400;
pub const NOTE_EXIT: u32 = 0x80000000;
pub const NOTE_FORK: u32 = 0x40000000;
pub const NOTE_EXEC: u32 = 0x20000000;
pub const NOTE_PDATAMASK: u32 = 0x000fffff;
pub const NOTE_PCTRLMASK: u32 = 0xf0000000;
pub const NOTE_TRACK: u32 = 0x00000001;
pub const NOTE_TRACKERR: u32 = 0x00000002;
pub const NOTE_CHILD: u32 = 0x00000004;
pub const NOTE_SECONDS: u32 = 0x00000001;
pub const NOTE_MSECONDS: u32 = 0x00000002;
pub const NOTE_USECONDS: u32 = 0x00000004;
pub const NOTE_NSECONDS: u32 = 0x00000008;
pub const NOTE_ABSTIME: u32 = 0x00000010;

pub const MADV_PROTECT: c_int = 10;

#[doc(hidden)]
#[deprecated(
    since = "0.2.72",
    note = "CTL_UNSPEC is deprecated. Use CTL_SYSCTL instead"
)]
pub const CTL_UNSPEC: c_int = 0;
pub const CTL_SYSCTL: c_int = 0;
pub const CTL_KERN: c_int = 1;
pub const CTL_VM: c_int = 2;
pub const CTL_VFS: c_int = 3;
pub const CTL_NET: c_int = 4;
pub const CTL_DEBUG: c_int = 5;
pub const CTL_HW: c_int = 6;
pub const CTL_MACHDEP: c_int = 7;
pub const CTL_USER: c_int = 8;
pub const CTL_P1003_1B: c_int = 9;

// sys/sysctl.h
pub const CTL_MAXNAME: c_int = 24;

pub const CTLTYPE: c_int = 0xf;
pub const CTLTYPE_NODE: c_int = 1;
pub const CTLTYPE_INT: c_int = 2;
pub const CTLTYPE_STRING: c_int = 3;
pub const CTLTYPE_S64: c_int = 4;
pub const CTLTYPE_OPAQUE: c_int = 5;
pub const CTLTYPE_STRUCT: c_int = CTLTYPE_OPAQUE;
pub const CTLTYPE_UINT: c_int = 6;
pub const CTLTYPE_LONG: c_int = 7;
pub const CTLTYPE_ULONG: c_int = 8;
pub const CTLTYPE_U64: c_int = 9;
pub const CTLTYPE_U8: c_int = 0xa;
pub const CTLTYPE_U16: c_int = 0xb;
pub const CTLTYPE_S8: c_int = 0xc;
pub const CTLTYPE_S16: c_int = 0xd;
pub const CTLTYPE_S32: c_int = 0xe;
pub const CTLTYPE_U32: c_int = 0xf;

pub const CTLFLAG_RD: c_int = 0x80000000;
pub const CTLFLAG_WR: c_int = 0x40000000;
pub const CTLFLAG_RW: c_int = CTLFLAG_RD | CTLFLAG_WR;
pub const CTLFLAG_DORMANT: c_int = 0x20000000;
pub const CTLFLAG_ANYBODY: c_int = 0x10000000;
pub const CTLFLAG_SECURE: c_int = 0x08000000;
pub const CTLFLAG_PRISON: c_int = 0x04000000;
pub const CTLFLAG_DYN: c_int = 0x02000000;
pub const CTLFLAG_SKIP: c_int = 0x01000000;
pub const CTLMASK_SECURE: c_int = 0x00F00000;
pub const CTLFLAG_TUN: c_int = 0x00080000;
pub const CTLFLAG_RDTUN: c_int = CTLFLAG_RD | CTLFLAG_TUN;
pub const CTLFLAG_RWTUN: c_int = CTLFLAG_RW | CTLFLAG_TUN;
pub const CTLFLAG_MPSAFE: c_int = 0x00040000;
pub const CTLFLAG_VNET: c_int = 0x00020000;
pub const CTLFLAG_DYING: c_int = 0x00010000;
pub const CTLFLAG_CAPRD: c_int = 0x00008000;
pub const CTLFLAG_CAPWR: c_int = 0x00004000;
pub const CTLFLAG_STATS: c_int = 0x00002000;
pub const CTLFLAG_NOFETCH: c_int = 0x00001000;
pub const CTLFLAG_CAPRW: c_int = CTLFLAG_CAPRD | CTLFLAG_CAPWR;
pub const CTLFLAG_NEEDGIANT: c_int = 0x00000800;

pub const CTLSHIFT_SECURE: c_int = 20;
pub const CTLFLAG_SECURE1: c_int = CTLFLAG_SECURE | (0 << CTLSHIFT_SECURE);
pub const CTLFLAG_SECURE2: c_int = CTLFLAG_SECURE | (1 << CTLSHIFT_SECURE);
pub const CTLFLAG_SECURE3: c_int = CTLFLAG_SECURE | (2 << CTLSHIFT_SECURE);

pub const OID_AUTO: c_int = -1;

pub const CTL_SYSCTL_DEBUG: c_int = 0;
pub const CTL_SYSCTL_NAME: c_int = 1;
pub const CTL_SYSCTL_NEXT: c_int = 2;
pub const CTL_SYSCTL_NAME2OID: c_int = 3;
pub const CTL_SYSCTL_OIDFMT: c_int = 4;
pub const CTL_SYSCTL_OIDDESCR: c_int = 5;
pub const CTL_SYSCTL_OIDLABEL: c_int = 6;
pub const CTL_SYSCTL_NEXTNOSKIP: c_int = 7;

pub const KERN_OSTYPE: c_int = 1;
pub const KERN_OSRELEASE: c_int = 2;
pub const KERN_OSREV: c_int = 3;
pub const KERN_VERSION: c_int = 4;
pub const KERN_MAXVNODES: c_int = 5;
pub const KERN_MAXPROC: c_int = 6;
pub const KERN_MAXFILES: c_int = 7;
pub const KERN_ARGMAX: c_int = 8;
pub const KERN_SECURELVL: c_int = 9;
pub const KERN_HOSTNAME: c_int = 10;
pub const KERN_HOSTID: c_int = 11;
pub const KERN_CLOCKRATE: c_int = 12;
pub const KERN_VNODE: c_int = 13;
pub const KERN_PROC: c_int = 14;
pub const KERN_FILE: c_int = 15;
pub const KERN_PROF: c_int = 16;
pub const KERN_POSIX1: c_int = 17;
pub const KERN_NGROUPS: c_int = 18;
pub const KERN_JOB_CONTROL: c_int = 19;
pub const KERN_SAVED_IDS: c_int = 20;
pub const KERN_BOOTTIME: c_int = 21;
pub const KERN_NISDOMAINNAME: c_int = 22;
pub const KERN_UPDATEINTERVAL: c_int = 23;
pub const KERN_OSRELDATE: c_int = 24;
pub const KERN_NTP_PLL: c_int = 25;
pub const KERN_BOOTFILE: c_int = 26;
pub const KERN_MAXFILESPERPROC: c_int = 27;
pub const KERN_MAXPROCPERUID: c_int = 28;
pub const KERN_DUMPDEV: c_int = 29;
pub const KERN_IPC: c_int = 30;
pub const KERN_DUMMY: c_int = 31;
pub const KERN_PS_STRINGS: c_int = 32;
pub const KERN_USRSTACK: c_int = 33;
pub const KERN_LOGSIGEXIT: c_int = 34;
pub const KERN_IOV_MAX: c_int = 35;
pub const KERN_HOSTUUID: c_int = 36;
pub const KERN_ARND: c_int = 37;
pub const KERN_MAXPHYS: c_int = 38;

pub const KERN_PROC_ALL: c_int = 0;
pub const KERN_PROC_PID: c_int = 1;
pub const KERN_PROC_PGRP: c_int = 2;
pub const KERN_PROC_SESSION: c_int = 3;
pub const KERN_PROC_TTY: c_int = 4;
pub const KERN_PROC_UID: c_int = 5;
pub const KERN_PROC_RUID: c_int = 6;
pub const KERN_PROC_ARGS: c_int = 7;
pub const KERN_PROC_PROC: c_int = 8;
pub const KERN_PROC_SV_NAME: c_int = 9;
pub const KERN_PROC_RGID: c_int = 10;
pub const KERN_PROC_GID: c_int = 11;
pub const KERN_PROC_PATHNAME: c_int = 12;
pub const KERN_PROC_OVMMAP: c_int = 13;
pub const KERN_PROC_OFILEDESC: c_int = 14;
pub const KERN_PROC_KSTACK: c_int = 15;
pub const KERN_PROC_INC_THREAD: c_int = 0x10;
pub const KERN_PROC_VMMAP: c_int = 32;
pub const KERN_PROC_FILEDESC: c_int = 33;
pub const KERN_PROC_GROUPS: c_int = 34;
pub const KERN_PROC_ENV: c_int = 35;
pub const KERN_PROC_AUXV: c_int = 36;
pub const KERN_PROC_RLIMIT: c_int = 37;
pub const KERN_PROC_PS_STRINGS: c_int = 38;
pub const KERN_PROC_UMASK: c_int = 39;
pub const KERN_PROC_OSREL: c_int = 40;
pub const KERN_PROC_SIGTRAMP: c_int = 41;
pub const KERN_PROC_CWD: c_int = 42;
pub const KERN_PROC_NFDS: c_int = 43;
pub const KERN_PROC_SIGFASTBLK: c_int = 44;

pub const KIPC_MAXSOCKBUF: c_int = 1;
pub const KIPC_SOCKBUF_WASTE: c_int = 2;
pub const KIPC_SOMAXCONN: c_int = 3;
pub const KIPC_MAX_LINKHDR: c_int = 4;
pub const KIPC_MAX_PROTOHDR: c_int = 5;
pub const KIPC_MAX_HDR: c_int = 6;
pub const KIPC_MAX_DATALEN: c_int = 7;

pub const HW_MACHINE: c_int = 1;
pub const HW_MODEL: c_int = 2;
pub const HW_NCPU: c_int = 3;
pub const HW_BYTEORDER: c_int = 4;
pub const HW_PHYSMEM: c_int = 5;
pub const HW_USERMEM: c_int = 6;
pub const HW_PAGESIZE: c_int = 7;
pub const HW_DISKNAMES: c_int = 8;
pub const HW_DISKSTATS: c_int = 9;
pub const HW_FLOATINGPT: c_int = 10;
pub const HW_MACHINE_ARCH: c_int = 11;
pub const HW_REALMEM: c_int = 12;

pub const USER_CS_PATH: c_int = 1;
pub const USER_BC_BASE_MAX: c_int = 2;
pub const USER_BC_DIM_MAX: c_int = 3;
pub const USER_BC_SCALE_MAX: c_int = 4;
pub const USER_BC_STRING_MAX: c_int = 5;
pub const USER_COLL_WEIGHTS_MAX: c_int = 6;
pub const USER_EXPR_NEST_MAX: c_int = 7;
pub const USER_LINE_MAX: c_int = 8;
pub const USER_RE_DUP_MAX: c_int = 9;
pub const USER_POSIX2_VERSION: c_int = 10;
pub const USER_POSIX2_C_BIND: c_int = 11;
pub const USER_POSIX2_C_DEV: c_int = 12;
pub const USER_POSIX2_CHAR_TERM: c_int = 13;
pub const USER_POSIX2_FORT_DEV: c_int = 14;
pub const USER_POSIX2_FORT_RUN: c_int = 15;
pub const USER_POSIX2_LOCALEDEF: c_int = 16;
pub const USER_POSIX2_SW_DEV: c_int = 17;
pub const USER_POSIX2_UPE: c_int = 18;
pub const USER_STREAM_MAX: c_int = 19;
pub const USER_TZNAME_MAX: c_int = 20;
pub const USER_LOCALBASE: c_int = 21;

pub const CTL_P1003_1B_ASYNCHRONOUS_IO: c_int = 1;
pub const CTL_P1003_1B_MAPPED_FILES: c_int = 2;
pub const CTL_P1003_1B_MEMLOCK: c_int = 3;
pub const CTL_P1003_1B_MEMLOCK_RANGE: c_int = 4;
pub const CTL_P1003_1B_MEMORY_PROTECTION: c_int = 5;
pub const CTL_P1003_1B_MESSAGE_PASSING: c_int = 6;
pub const CTL_P1003_1B_PRIORITIZED_IO: c_int = 7;
pub const CTL_P1003_1B_PRIORITY_SCHEDULING: c_int = 8;
pub const CTL_P1003_1B_REALTIME_SIGNALS: c_int = 9;
pub const CTL_P1003_1B_SEMAPHORES: c_int = 10;
pub const CTL_P1003_1B_FSYNC: c_int = 11;
pub const CTL_P1003_1B_SHARED_MEMORY_OBJECTS: c_int = 12;
pub const CTL_P1003_1B_SYNCHRONIZED_IO: c_int = 13;
pub const CTL_P1003_1B_TIMERS: c_int = 14;
pub const CTL_P1003_1B_AIO_LISTIO_MAX: c_int = 15;
pub const CTL_P1003_1B_AIO_MAX: c_int = 16;
pub const CTL_P1003_1B_AIO_PRIO_DELTA_MAX: c_int = 17;
pub const CTL_P1003_1B_DELAYTIMER_MAX: c_int = 18;
pub const CTL_P1003_1B_MQ_OPEN_MAX: c_int = 19;
pub const CTL_P1003_1B_PAGESIZE: c_int = 20;
pub const CTL_P1003_1B_RTSIG_MAX: c_int = 21;
pub const CTL_P1003_1B_SEM_NSEMS_MAX: c_int = 22;
pub const CTL_P1003_1B_SEM_VALUE_MAX: c_int = 23;
pub const CTL_P1003_1B_SIGQUEUE_MAX: c_int = 24;
pub const CTL_P1003_1B_TIMER_MAX: c_int = 25;

pub const TIOCGPTN: c_ulong = 0x4004740f;
pub const TIOCPTMASTER: c_ulong = 0x2000741c;
pub const TIOCSIG: c_ulong = 0x2004745f;
pub const TIOCM_DCD: c_int = 0x40;
pub const H4DISC: c_int = 0x7;

pub const VM_TOTAL: c_int = 1;

cfg_if! {
    if #[cfg(target_pointer_width = "64")] {
        pub const BIOCSETFNR: c_ulong = 0x80104282;
    } else {
        pub const BIOCSETFNR: c_ulong = 0x80084282;
    }
}

cfg_if! {
    if #[cfg(target_pointer_width = "64")] {
        pub const FIODGNAME: c_ulong = 0x80106678;
    } else {
        pub const FIODGNAME: c_ulong = 0x80086678;
    }
}

pub const FIONWRITE: c_ulong = 0x40046677;
pub const FIONSPACE: c_ulong = 0x40046676;
pub const FIOSEEKDATA: c_ulong = 0xc0086661;
pub const FIOSEEKHOLE: c_ulong = 0xc0086662;
pub const FIOSSHMLPGCNF: c_ulong = 0x80306664;

pub const JAIL_API_VERSION: u32 = 2;
pub const JAIL_CREATE: c_int = 0x01;
pub const JAIL_UPDATE: c_int = 0x02;
pub const JAIL_ATTACH: c_int = 0x04;
pub const JAIL_DYING: c_int = 0x08;
pub const JAIL_SET_MASK: c_int = 0x0f;
pub const JAIL_GET_MASK: c_int = 0x08;
pub const JAIL_SYS_DISABLE: c_int = 0;
pub const JAIL_SYS_NEW: c_int = 1;
pub const JAIL_SYS_INHERIT: c_int = 2;

pub const MNT_ACLS: c_int = 0x08000000;
pub const MNT_BYFSID: c_int = 0x08000000;
pub const MNT_GJOURNAL: c_int = 0x02000000;
pub const MNT_MULTILABEL: c_int = 0x04000000;
pub const MNT_NFS4ACLS: c_int = 0x00000010;
pub const MNT_SNAPSHOT: c_int = 0x01000000;
pub const MNT_UNION: c_int = 0x00000020;
pub const MNT_NONBUSY: c_int = 0x04000000;

pub const SCM_BINTIME: c_int = 0x04;
pub const SCM_REALTIME: c_int = 0x05;
pub const SCM_MONOTONIC: c_int = 0x06;
pub const SCM_TIME_INFO: c_int = 0x07;
pub const SCM_CREDS2: c_int = 0x08;

pub const SO_BINTIME: c_int = 0x2000;
pub const SO_NO_OFFLOAD: c_int = 0x4000;
pub const SO_NO_DDP: c_int = 0x8000;
pub const SO_REUSEPORT_LB: c_int = 0x10000;
pub const SO_LABEL: c_int = 0x1009;
pub const SO_PEERLABEL: c_int = 0x1010;
pub const SO_LISTENQLIMIT: c_int = 0x1011;
pub const SO_LISTENQLEN: c_int = 0x1012;
pub const SO_LISTENINCQLEN: c_int = 0x1013;
pub const SO_SETFIB: c_int = 0x1014;
pub const SO_USER_COOKIE: c_int = 0x1015;
pub const SO_PROTOCOL: c_int = 0x1016;
pub const SO_PROTOTYPE: c_int = SO_PROTOCOL;
pub const SO_TS_CLOCK: c_int = 0x1017;
pub const SO_DOMAIN: c_int = 0x1019;
pub const SO_SPLICE: c_int = 0x1023;
pub const SO_VENDOR: c_int = 0x80000000;

pub const SO_TS_REALTIME_MICRO: c_int = 0;
pub const SO_TS_BINTIME: c_int = 1;
pub const SO_TS_REALTIME: c_int = 2;
pub const SO_TS_MONOTONIC: c_int = 3;
pub const SO_TS_DEFAULT: c_int = SO_TS_REALTIME_MICRO;
pub const SO_TS_CLOCK_MAX: c_int = SO_TS_MONOTONIC;

pub const LOCAL_CREDS: c_int = 2;
pub const LOCAL_CREDS_PERSISTENT: c_int = 3;
pub const LOCAL_CONNWAIT: c_int = 4;
pub const LOCAL_VENDOR: c_int = SO_VENDOR;

pub const PL_EVENT_NONE: c_int = 0;
pub const PL_EVENT_SIGNAL: c_int = 1;
pub const PL_FLAG_SA: c_int = 0x01;
pub const PL_FLAG_BOUND: c_int = 0x02;
pub const PL_FLAG_SCE: c_int = 0x04;
pub const PL_FLAG_SCX: c_int = 0x08;
pub const PL_FLAG_EXEC: c_int = 0x10;
pub const PL_FLAG_SI: c_int = 0x20;
pub const PL_FLAG_FORKED: c_int = 0x40;
pub const PL_FLAG_CHILD: c_int = 0x80;
pub const PL_FLAG_BORN: c_int = 0x100;
pub const PL_FLAG_EXITED: c_int = 0x200;
pub const PL_FLAG_VFORKED: c_int = 0x400;
pub const PL_FLAG_VFORK_DONE: c_int = 0x800;

pub const PT_LWPINFO: c_int = 13;
pub const PT_GETNUMLWPS: c_int = 14;
pub const PT_GETLWPLIST: c_int = 15;
pub const PT_CLEARSTEP: c_int = 16;
pub const PT_SETSTEP: c_int = 17;
pub const PT_SUSPEND: c_int = 18;
pub const PT_RESUME: c_int = 19;
pub const PT_TO_SCE: c_int = 20;
pub const PT_TO_SCX: c_int = 21;
pub const PT_SYSCALL: c_int = 22;
pub const PT_FOLLOW_FORK: c_int = 23;
pub const PT_LWP_EVENTS: c_int = 24;
pub const PT_GET_EVENT_MASK: c_int = 25;
pub const PT_SET_EVENT_MASK: c_int = 26;
pub const PT_GET_SC_ARGS: c_int = 27;
pub const PT_GET_SC_RET: c_int = 28;
pub const PT_COREDUMP: c_int = 29;
pub const PT_GETREGS: c_int = 33;
pub const PT_SETREGS: c_int = 34;
pub const PT_GETFPREGS: c_int = 35;
pub const PT_SETFPREGS: c_int = 36;
pub const PT_GETDBREGS: c_int = 37;
pub const PT_SETDBREGS: c_int = 38;
pub const PT_VM_TIMESTAMP: c_int = 40;
pub const PT_VM_ENTRY: c_int = 41;
pub const PT_GETREGSET: c_int = 42;
pub const PT_SETREGSET: c_int = 43;
pub const PT_SC_REMOTE: c_int = 44;
pub const PT_FIRSTMACH: c_int = 64;

pub const PTRACE_EXEC: c_int = 0x0001;
pub const PTRACE_SCE: c_int = 0x0002;
pub const PTRACE_SCX: c_int = 0x0004;
pub const PTRACE_SYSCALL: c_int = PTRACE_SCE | PTRACE_SCX;
pub const PTRACE_FORK: c_int = 0x0008;
pub const PTRACE_LWP: c_int = 0x0010;
pub const PTRACE_VFORK: c_int = 0x0020;
pub const PTRACE_DEFAULT: c_int = PTRACE_EXEC;

pub const PC_COMPRESS: u32 = 0x00000001;
pub const PC_ALL: u32 = 0x00000002;

pub const PROC_SPROTECT: c_int = 1;
pub const PROC_REAP_ACQUIRE: c_int = 2;
pub const PROC_REAP_RELEASE: c_int = 3;
pub const PROC_REAP_STATUS: c_int = 4;
pub const PROC_REAP_GETPIDS: c_int = 5;
pub const PROC_REAP_KILL: c_int = 6;
pub const PROC_TRACE_CTL: c_int = 7;
pub const PROC_TRACE_STATUS: c_int = 8;
pub const PROC_TRAPCAP_CTL: c_int = 9;
pub const PROC_TRAPCAP_STATUS: c_int = 10;
pub const PROC_PDEATHSIG_CTL: c_int = 11;
pub const PROC_PDEATHSIG_STATUS: c_int = 12;
pub const PROC_ASLR_CTL: c_int = 13;
pub const PROC_ASLR_STATUS: c_int = 14;
pub const PROC_PROTMAX_CTL: c_int = 15;
pub const PROC_PROTMAX_STATUS: c_int = 16;
pub const PROC_STACKGAP_CTL: c_int = 17;
pub const PROC_STACKGAP_STATUS: c_int = 18;
pub const PROC_NO_NEW_PRIVS_CTL: c_int = 19;
pub const PROC_NO_NEW_PRIVS_STATUS: c_int = 20;
pub const PROC_WXMAP_CTL: c_int = 21;
pub const PROC_WXMAP_STATUS: c_int = 22;
pub const PROC_PROCCTL_MD_MIN: c_int = 0x10000000;

pub const PPROT_SET: c_int = 1;
pub const PPROT_CLEAR: c_int = 2;
pub const PPROT_DESCEND: c_int = 0x10;
pub const PPROT_INHERIT: c_int = 0x20;

pub const PROC_TRACE_CTL_ENABLE: c_int = 1;
pub const PROC_TRACE_CTL_DISABLE: c_int = 2;
pub const PROC_TRACE_CTL_DISABLE_EXEC: c_int = 3;

pub const PROC_TRAPCAP_CTL_ENABLE: c_int = 1;
pub const PROC_TRAPCAP_CTL_DISABLE: c_int = 2;

pub const PROC_ASLR_FORCE_ENABLE: c_int = 1;
pub const PROC_ASLR_FORCE_DISABLE: c_int = 2;
pub const PROC_ASLR_NOFORCE: c_int = 3;
pub const PROC_ASLR_ACTIVE: c_int = 0x80000000;

pub const PROC_PROTMAX_FORCE_ENABLE: c_int = 1;
pub const PROC_PROTMAX_FORCE_DISABLE: c_int = 2;
pub const PROC_PROTMAX_NOFORCE: c_int = 3;
pub const PROC_PROTMAX_ACTIVE: c_int = 0x80000000;

pub const PROC_STACKGAP_ENABLE: c_int = 0x0001;
pub const PROC_STACKGAP_DISABLE: c_int = 0x0002;
pub const PROC_STACKGAP_ENABLE_EXEC: c_int = 0x0004;
pub const PROC_STACKGAP_DISABLE_EXEC: c_int = 0x0008;

pub const PROC_NO_NEW_PRIVS_ENABLE: c_int = 1;
pub const PROC_NO_NEW_PRIVS_DISABLE: c_int = 2;

pub const PROC_WX_MAPPINGS_PERMIT: c_int = 0x0001;
pub const PROC_WX_MAPPINGS_DISALLOW_EXEC: c_int = 0x0002;
pub const PROC_WXORX_ENFORCE: c_int = 0x80000000;

pub const AF_SLOW: c_int = 33;
pub const AF_SCLUSTER: c_int = 34;
pub const AF_ARP: c_int = 35;
pub const AF_BLUETOOTH: c_int = 36;
pub const AF_IEEE80211: c_int = 37;
pub const AF_INET_SDP: c_int = 40;
pub const AF_INET6_SDP: c_int = 42;

// sys/net/if.h
pub const IF_MAXUNIT: c_int = 0x7fff;
/// (n) interface is up
pub const IFF_UP: c_int = 0x1;
/// (i) broadcast address valid
pub const IFF_BROADCAST: c_int = 0x2;
/// (n) turn on debugging
pub const IFF_DEBUG: c_int = 0x4;
/// (i) is a loopback net
pub const IFF_LOOPBACK: c_int = 0x8;
/// (i) is a point-to-point link
pub const IFF_POINTOPOINT: c_int = 0x10;
/// (i) calls if_input in net epoch
#[deprecated(since = "0.2.149", note = "Removed in FreeBSD 14")]
pub const IFF_KNOWSEPOCH: c_int = 0x20;
/// (d) resources allocated
pub const IFF_RUNNING: c_int = 0x40;
#[doc(hidden)]
#[deprecated(
    since = "0.2.54",
    note = "IFF_DRV_RUNNING is deprecated. Use the portable IFF_RUNNING instead"
)]
/// (d) resources allocate
pub const IFF_DRV_RUNNING: c_int = 0x40;
/// (n) no address resolution protocol
pub const IFF_NOARP: c_int = 0x80;
/// (n) receive all packets
pub const IFF_PROMISC: c_int = 0x100;
/// (n) receive all multicast packets
pub const IFF_ALLMULTI: c_int = 0x200;
/// (d) tx hardware queue is full
pub const IFF_OACTIVE: c_int = 0x400;
#[doc(hidden)]
#[deprecated(since = "0.2.54", note = "Use the portable `IFF_OACTIVE` instead")]
/// (d) tx hardware queue is full
pub const IFF_DRV_OACTIVE: c_int = 0x400;
/// (i) can't hear own transmissions
pub const IFF_SIMPLEX: c_int = 0x800;
/// per link layer defined bit
pub const IFF_LINK0: c_int = 0x1000;
/// per link layer defined bit
pub const IFF_LINK1: c_int = 0x2000;
/// per link layer defined bit
pub const IFF_LINK2: c_int = 0x4000;
/// use alternate physical connection
pub const IFF_ALTPHYS: c_int = IFF_LINK2;
/// (i) supports multicast
pub const IFF_MULTICAST: c_int = 0x8000;
/// (i) unconfigurable using ioctl(2)
pub const IFF_CANTCONFIG: c_int = 0x10000;
/// (n) user-requested promisc mode
pub const IFF_PPROMISC: c_int = 0x20000;
/// (n) user-requested monitor mode
pub const IFF_MONITOR: c_int = 0x40000;
/// (n) static ARP
pub const IFF_STATICARP: c_int = 0x80000;
/// (n) interface is winding down
pub const IFF_DYING: c_int = 0x200000;
/// (n) interface is being renamed
pub const IFF_RENAMING: c_int = 0x400000;
/// interface is not part of any groups
#[deprecated(since = "0.2.149", note = "Removed in FreeBSD 14")]
pub const IFF_NOGROUP: c_int = 0x800000;

/// link invalid/unknown
pub const LINK_STATE_UNKNOWN: c_int = 0;
/// link is down
pub const LINK_STATE_DOWN: c_int = 1;
/// link is up
pub const LINK_STATE_UP: c_int = 2;

/// can offload checksum on RX
pub const IFCAP_RXCSUM: c_int = 0x00001;
/// can offload checksum on TX
pub const IFCAP_TXCSUM: c_int = 0x00002;
/// can be a network console
pub const IFCAP_NETCONS: c_int = 0x00004;
/// VLAN-compatible MTU
pub const IFCAP_VLAN_MTU: c_int = 0x00008;
/// hardware VLAN tag support
pub const IFCAP_VLAN_HWTAGGING: c_int = 0x00010;
/// 9000 byte MTU supported
pub const IFCAP_JUMBO_MTU: c_int = 0x00020;
/// driver supports polling
pub const IFCAP_POLLING: c_int = 0x00040;
/// can do IFCAP_HWCSUM on VLANs
pub const IFCAP_VLAN_HWCSUM: c_int = 0x00080;
/// can do TCP Segmentation Offload
pub const IFCAP_TSO4: c_int = 0x00100;
/// can do TCP6 Segmentation Offload
pub const IFCAP_TSO6: c_int = 0x00200;
/// can do Large Receive Offload
pub const IFCAP_LRO: c_int = 0x00400;
/// wake on any unicast frame
pub const IFCAP_WOL_UCAST: c_int = 0x00800;
/// wake on any multicast frame
pub const IFCAP_WOL_MCAST: c_int = 0x01000;
/// wake on any Magic Packet
pub const IFCAP_WOL_MAGIC: c_int = 0x02000;
/// interface can offload TCP
pub const IFCAP_TOE4: c_int = 0x04000;
/// interface can offload TCP6
pub const IFCAP_TOE6: c_int = 0x08000;
/// interface hw can filter vlan tag
pub const IFCAP_VLAN_HWFILTER: c_int = 0x10000;
/// can do SIOCGIFCAPNV/SIOCSIFCAPNV
pub const IFCAP_NV: c_int = 0x20000;
/// can do IFCAP_TSO on VLANs
pub const IFCAP_VLAN_HWTSO: c_int = 0x40000;
/// the runtime link state is dynamic
pub const IFCAP_LINKSTATE: c_int = 0x80000;
/// netmap mode supported/enabled
pub const IFCAP_NETMAP: c_int = 0x100000;
/// can offload checksum on IPv6 RX
pub const IFCAP_RXCSUM_IPV6: c_int = 0x200000;
/// can offload checksum on IPv6 TX
pub const IFCAP_TXCSUM_IPV6: c_int = 0x400000;
/// manages counters internally
pub const IFCAP_HWSTATS: c_int = 0x800000;
/// hardware supports TX rate limiting
pub const IFCAP_TXRTLMT: c_int = 0x1000000;
/// hardware rx timestamping
pub const IFCAP_HWRXTSTMP: c_int = 0x2000000;
/// understands M_EXTPG mbufs
pub const IFCAP_MEXTPG: c_int = 0x4000000;
/// can do TLS encryption and segmentation for TCP
pub const IFCAP_TXTLS4: c_int = 0x8000000;
/// can do TLS encryption and segmentation for TCP6
pub const IFCAP_TXTLS6: c_int = 0x10000000;
/// can do IFCAN_HWCSUM on VXLANs
pub const IFCAP_VXLAN_HWCSUM: c_int = 0x20000000;
/// can do IFCAP_TSO on VXLANs
pub const IFCAP_VXLAN_HWTSO: c_int = 0x40000000;
/// can do TLS with rate limiting
pub const IFCAP_TXTLS_RTLMT: c_int = 0x80000000;

pub const IFCAP_HWCSUM_IPV6: c_int = IFCAP_RXCSUM_IPV6 | IFCAP_TXCSUM_IPV6;
pub const IFCAP_HWCSUM: c_int = IFCAP_RXCSUM | IFCAP_TXCSUM;
pub const IFCAP_TSO: c_int = IFCAP_TSO4 | IFCAP_TSO6;
pub const IFCAP_WOL: c_int = IFCAP_WOL_UCAST | IFCAP_WOL_MCAST | IFCAP_WOL_MAGIC;
pub const IFCAP_TOE: c_int = IFCAP_TOE4 | IFCAP_TOE6;
pub const IFCAP_TXTLS: c_int = IFCAP_TXTLS4 | IFCAP_TXTLS6;
pub const IFCAP_CANTCHANGE: c_int = IFCAP_NETMAP | IFCAP_NV;

pub const IFQ_MAXLEN: c_int = 50;
pub const IFNET_SLOWHZ: c_int = 1;

pub const IFAN_ARRIVAL: c_int = 0;
pub const IFAN_DEPARTURE: c_int = 1;

pub const IFSTATMAX: c_int = 800;

pub const RSS_FUNC_NONE: c_int = 0;
pub const RSS_FUNC_PRIVATE: c_int = 1;
pub const RSS_FUNC_TOEPLITZ: c_int = 2;

pub const RSS_TYPE_IPV4: c_int = 0x00000001;
pub const RSS_TYPE_TCP_IPV4: c_int = 0x00000002;
pub const RSS_TYPE_IPV6: c_int = 0x00000004;
pub const RSS_TYPE_IPV6_EX: c_int = 0x00000008;
pub const RSS_TYPE_TCP_IPV6: c_int = 0x00000010;
pub const RSS_TYPE_TCP_IPV6_EX: c_int = 0x00000020;
pub const RSS_TYPE_UDP_IPV4: c_int = 0x00000040;
pub const RSS_TYPE_UDP_IPV6: c_int = 0x00000080;
pub const RSS_TYPE_UDP_IPV6_EX: c_int = 0x00000100;
pub const RSS_KEYLEN: c_int = 128;

pub const IFNET_PCP_NONE: c_int = 0xff;
pub const IFDR_MSG_SIZE: c_int = 64;
pub const IFDR_REASON_MSG: c_int = 1;
pub const IFDR_REASON_VENDOR: c_int = 2;

// sys/net/if_mib.h

/// non-interface-specific
pub const IFMIB_SYSTEM: c_int = 1;
/// per-interface data table
pub const IFMIB_IFDATA: c_int = 2;

/// generic stats for all kinds of ifaces
pub const IFDATA_GENERAL: c_int = 1;
/// specific to the type of interface
pub const IFDATA_LINKSPECIFIC: c_int = 2;
/// driver name and unit
pub const IFDATA_DRIVERNAME: c_int = 3;

/// number of interfaces configured
pub const IFMIB_IFCOUNT: c_int = 1;

/// functions not specific to a type of iface
pub const NETLINK_GENERIC: c_int = 0;

pub const DOT3COMPLIANCE_STATS: c_int = 1;
pub const DOT3COMPLIANCE_COLLS: c_int = 2;

pub const dot3ChipSetAMD7990: c_int = 1;
pub const dot3ChipSetAMD79900: c_int = 2;
pub const dot3ChipSetAMD79C940: c_int = 3;

pub const dot3ChipSetIntel82586: c_int = 1;
pub const dot3ChipSetIntel82596: c_int = 2;
pub const dot3ChipSetIntel82557: c_int = 3;

pub const dot3ChipSetNational8390: c_int = 1;
pub const dot3ChipSetNationalSonic: c_int = 2;

pub const dot3ChipSetFujitsu86950: c_int = 1;

pub const dot3ChipSetDigitalDC21040: c_int = 1;
pub const dot3ChipSetDigitalDC21140: c_int = 2;
pub const dot3ChipSetDigitalDC21041: c_int = 3;
pub const dot3ChipSetDigitalDC21140A: c_int = 4;
pub const dot3ChipSetDigitalDC21142: c_int = 5;

pub const dot3ChipSetWesternDigital83C690: c_int = 1;
pub const dot3ChipSetWesternDigital83C790: c_int = 2;

// sys/netinet/in.h
// Protocols (RFC 1700)
// NOTE: These are in addition to the constants defined in src/unix/mod.rs

// IPPROTO_IP defined in src/unix/mod.rs
/// IP6 hop-by-hop options
pub const IPPROTO_HOPOPTS: c_int = 0;
// IPPROTO_ICMP defined in src/unix/mod.rs
/// group mgmt protocol
pub const IPPROTO_IGMP: c_int = 2;
/// gateway^2 (deprecated)
pub const IPPROTO_GGP: c_int = 3;
/// for compatibility
pub const IPPROTO_IPIP: c_int = 4;
// IPPROTO_TCP defined in src/unix/mod.rs
/// Stream protocol II.
pub const IPPROTO_ST: c_int = 7;
/// exterior gateway protocol
pub const IPPROTO_EGP: c_int = 8;
/// private interior gateway
pub const IPPROTO_PIGP: c_int = 9;
/// BBN RCC Monitoring
pub const IPPROTO_RCCMON: c_int = 10;
/// network voice protocol
pub const IPPROTO_NVPII: c_int = 11;
/// pup
pub const IPPROTO_PUP: c_int = 12;
/// Argus
pub const IPPROTO_ARGUS: c_int = 13;
/// EMCON
pub const IPPROTO_EMCON: c_int = 14;
/// Cross Net Debugger
pub const IPPROTO_XNET: c_int = 15;
/// Chaos
pub const IPPROTO_CHAOS: c_int = 16;
// IPPROTO_UDP defined in src/unix/mod.rs
/// Multiplexing
pub const IPPROTO_MUX: c_int = 18;
/// DCN Measurement Subsystems
pub const IPPROTO_MEAS: c_int = 19;
/// Host Monitoring
pub const IPPROTO_HMP: c_int = 20;
/// Packet Radio Measurement
pub const IPPROTO_PRM: c_int = 21;
/// xns idp
pub const IPPROTO_IDP: c_int = 22;
/// Trunk-1
pub const IPPROTO_TRUNK1: c_int = 23;
/// Trunk-2
pub const IPPROTO_TRUNK2: c_int = 24;
/// Leaf-1
pub const IPPROTO_LEAF1: c_int = 25;
/// Leaf-2
pub const IPPROTO_LEAF2: c_int = 26;
/// Reliable Data
pub const IPPROTO_RDP: c_int = 27;
/// Reliable Transaction
pub const IPPROTO_IRTP: c_int = 28;
/// tp-4 w/ class negotiation
pub const IPPROTO_TP: c_int = 29;
/// Bulk Data Transfer
pub const IPPROTO_BLT: c_int = 30;
/// Network Services
pub const IPPROTO_NSP: c_int = 31;
/// Merit Internodal
pub const IPPROTO_INP: c_int = 32;
#[doc(hidden)]
#[deprecated(
    since = "0.2.72",
    note = "IPPROTO_SEP is deprecated. Use IPPROTO_DCCP instead"
)]
pub const IPPROTO_SEP: c_int = 33;
/// Datagram Congestion Control Protocol
pub const IPPROTO_DCCP: c_int = 33;
/// Third Party Connect
pub const IPPROTO_3PC: c_int = 34;
/// InterDomain Policy Routing
pub const IPPROTO_IDPR: c_int = 35;
/// XTP
pub const IPPROTO_XTP: c_int = 36;
/// Datagram Delivery
pub const IPPROTO_DDP: c_int = 37;
/// Control Message Transport
pub const IPPROTO_CMTP: c_int = 38;
/// TP++ Transport
pub const IPPROTO_TPXX: c_int = 39;
/// IL transport protocol
pub const IPPROTO_IL: c_int = 40;
// IPPROTO_IPV6 defined in src/unix/mod.rs
/// Source Demand Routing
pub const IPPROTO_SDRP: c_int = 42;
/// IP6 routing header
pub const IPPROTO_ROUTING: c_int = 43;
/// IP6 fragmentation header
pub const IPPROTO_FRAGMENT: c_int = 44;
/// InterDomain Routing
pub const IPPROTO_IDRP: c_int = 45;
/// resource reservation
pub const IPPROTO_RSVP: c_int = 46;
/// General Routing Encap.
pub const IPPROTO_GRE: c_int = 47;
/// Mobile Host Routing
pub const IPPROTO_MHRP: c_int = 48;
/// BHA
pub const IPPROTO_BHA: c_int = 49;
/// IP6 Encap Sec. Payload
pub const IPPROTO_ESP: c_int = 50;
/// IP6 Auth Header
pub const IPPROTO_AH: c_int = 51;
/// Integ. Net Layer Security
pub const IPPROTO_INLSP: c_int = 52;
/// IP with encryption
pub const IPPROTO_SWIPE: c_int = 53;
/// Next Hop Resolution
pub const IPPROTO_NHRP: c_int = 54;
/// IP Mobility
pub const IPPROTO_MOBILE: c_int = 55;
/// Transport Layer Security
pub const IPPROTO_TLSP: c_int = 56;
/// SKIP
pub const IPPROTO_SKIP: c_int = 57;
// IPPROTO_ICMPV6 defined in src/unix/mod.rs
/// IP6 no next header
pub const IPPROTO_NONE: c_int = 59;
/// IP6 destination option
pub const IPPROTO_DSTOPTS: c_int = 60;
/// any host internal protocol
pub const IPPROTO_AHIP: c_int = 61;
/// CFTP
pub const IPPROTO_CFTP: c_int = 62;
/// "hello" routing protocol
pub const IPPROTO_HELLO: c_int = 63;
/// SATNET/Backroom EXPAK
pub const IPPROTO_SATEXPAK: c_int = 64;
/// Kryptolan
pub const IPPROTO_KRYPTOLAN: c_int = 65;
/// Remote Virtual Disk
pub const IPPROTO_RVD: c_int = 66;
/// Pluribus Packet Core
pub const IPPROTO_IPPC: c_int = 67;
/// Any distributed FS
pub const IPPROTO_ADFS: c_int = 68;
/// Satnet Monitoring
pub const IPPROTO_SATMON: c_int = 69;
/// VISA Protocol
pub const IPPROTO_VISA: c_int = 70;
/// Packet Core Utility
pub const IPPROTO_IPCV: c_int = 71;
/// Comp. Prot. Net. Executive
pub const IPPROTO_CPNX: c_int = 72;
/// Comp. Prot. HeartBeat
pub const IPPROTO_CPHB: c_int = 73;
/// Wang Span Network
pub const IPPROTO_WSN: c_int = 74;
/// Packet Video Protocol
pub const IPPROTO_PVP: c_int = 75;
/// BackRoom SATNET Monitoring
pub const IPPROTO_BRSATMON: c_int = 76;
/// Sun net disk proto (temp.)
pub const IPPROTO_ND: c_int = 77;
/// WIDEBAND Monitoring
pub const IPPROTO_WBMON: c_int = 78;
/// WIDEBAND EXPAK
pub const IPPROTO_WBEXPAK: c_int = 79;
/// ISO cnlp
pub const IPPROTO_EON: c_int = 80;
/// VMTP
pub const IPPROTO_VMTP: c_int = 81;
/// Secure VMTP
pub const IPPROTO_SVMTP: c_int = 82;
/// Banyon VINES
pub const IPPROTO_VINES: c_int = 83;
/// TTP
pub const IPPROTO_TTP: c_int = 84;
/// NSFNET-IGP
pub const IPPROTO_IGP: c_int = 85;
/// dissimilar gateway prot.
pub const IPPROTO_DGP: c_int = 86;
/// TCF
pub const IPPROTO_TCF: c_int = 87;
/// Cisco/GXS IGRP
pub const IPPROTO_IGRP: c_int = 88;
/// OSPFIGP
pub const IPPROTO_OSPFIGP: c_int = 89;
/// Strite RPC protocol
pub const IPPROTO_SRPC: c_int = 90;
/// Locus Address Resoloution
pub const IPPROTO_LARP: c_int = 91;
/// Multicast Transport
pub const IPPROTO_MTP: c_int = 92;
/// AX.25 Frames
pub const IPPROTO_AX25: c_int = 93;
/// IP encapsulated in IP
pub const IPPROTO_IPEIP: c_int = 94;
/// Mobile Int.ing control
pub const IPPROTO_MICP: c_int = 95;
/// Semaphore Comm. security
pub const IPPROTO_SCCSP: c_int = 96;
/// Ethernet IP encapsulation
pub const IPPROTO_ETHERIP: c_int = 97;
/// encapsulation header
pub const IPPROTO_ENCAP: c_int = 98;
/// any private encr. scheme
pub const IPPROTO_APES: c_int = 99;
/// GMTP
pub const IPPROTO_GMTP: c_int = 100;
/// payload compression (IPComp)
pub const IPPROTO_IPCOMP: c_int = 108;
/// SCTP
pub const IPPROTO_SCTP: c_int = 132;
/// IPv6 Mobility Header
pub const IPPROTO_MH: c_int = 135;
/// UDP-Lite
pub const IPPROTO_UDPLITE: c_int = 136;
/// IP6 Host Identity Protocol
pub const IPPROTO_HIP: c_int = 139;
/// IP6 Shim6 Protocol
pub const IPPROTO_SHIM6: c_int = 140;

/* 101-254: Partly Unassigned */
/// Protocol Independent Mcast
pub const IPPROTO_PIM: c_int = 103;
/// CARP
pub const IPPROTO_CARP: c_int = 112;
/// PGM
pub const IPPROTO_PGM: c_int = 113;
/// MPLS-in-IP
pub const IPPROTO_MPLS: c_int = 137;
/// PFSYNC
pub const IPPROTO_PFSYNC: c_int = 240;

/* 255: Reserved */
/* BSD Private, local use, namespace incursion, no longer used */
/// OLD divert pseudo-proto
pub const IPPROTO_OLD_DIVERT: c_int = 254;
pub const IPPROTO_MAX: c_int = 256;
/// last return value of *_input(), meaning "all job for this pkt is done".
pub const IPPROTO_DONE: c_int = 257;

/* Only used internally, so can be outside the range of valid IP protocols. */
/// divert pseudo-protocol
pub const IPPROTO_DIVERT: c_int = 258;
/// SeND pseudo-protocol
pub const IPPROTO_SEND: c_int = 259;

// sys/netinet/TCP.h
pub const TCP_MD5SIG: c_int = 16;
pub const TCP_INFO: c_int = 32;
pub const TCP_CONGESTION: c_int = 64;
pub const TCP_CCALGOOPT: c_int = 65;
pub const TCP_MAXUNACKTIME: c_int = 68;
#[deprecated(since = "0.2.160", note = "Removed in FreeBSD 15")]
pub const TCP_MAXPEAKRATE: c_int = 69;
pub const TCP_IDLE_REDUCE: c_int = 70;
pub const TCP_REMOTE_UDP_ENCAPS_PORT: c_int = 71;
pub const TCP_DELACK: c_int = 72;
pub const TCP_FIN_IS_RST: c_int = 73;
pub const TCP_LOG_LIMIT: c_int = 74;
pub const TCP_SHARED_CWND_ALLOWED: c_int = 75;
pub const TCP_PROC_ACCOUNTING: c_int = 76;
pub const TCP_USE_CMP_ACKS: c_int = 77;
pub const TCP_PERF_INFO: c_int = 78;
pub const TCP_LRD: c_int = 79;
pub const TCP_KEEPINIT: c_int = 128;
pub const TCP_FASTOPEN: c_int = 1025;
#[deprecated(since = "0.2.171", note = "removed in FreeBSD 15")]
pub const TCP_PCAP_OUT: c_int = 2048;
#[deprecated(since = "0.2.171", note = "removed in FreeBSD 15")]
pub const TCP_PCAP_IN: c_int = 4096;
pub const TCP_FUNCTION_BLK: c_int = 8192;
pub const TCP_FUNCTION_ALIAS: c_int = 8193;
pub const TCP_FASTOPEN_PSK_LEN: c_int = 16;
pub const TCP_FUNCTION_NAME_LEN_MAX: c_int = 32;

pub const TCP_REUSPORT_LB_NUMA: c_int = 1026;
pub const TCP_RACK_MBUF_QUEUE: c_int = 1050;
pub const TCP_RACK_TLP_REDUCE: c_int = 1052;
pub const TCP_RACK_PACE_MAX_SEG: c_int = 1054;
pub const TCP_RACK_PACE_ALWAYS: c_int = 1055;
pub const TCP_RACK_PRR_SENDALOT: c_int = 1057;
pub const TCP_RACK_MIN_TO: c_int = 1058;
pub const TCP_RACK_EARLY_SEG: c_int = 1060;
pub const TCP_RACK_REORD_THRESH: c_int = 1061;
pub const TCP_RACK_REORD_FADE: c_int = 1062;
pub const TCP_RACK_TLP_THRESH: c_int = 1063;
pub const TCP_RACK_PKT_DELAY: c_int = 1064;
pub const TCP_BBR_IWINTSO: c_int = 1067;
pub const TCP_BBR_STARTUP_PG: c_int = 1069;
pub const TCP_BBR_DRAIN_PG: c_int = 1070;
pub const TCP_BBR_PROBE_RTT_INT: c_int = 1072;
pub const TCP_BBR_STARTUP_LOSS_EXIT: c_int = 1074;
pub const TCP_BBR_TSLIMITS: c_int = 1076;
pub const TCP_BBR_PACE_OH: c_int = 1077;
pub const TCP_BBR_USEDEL_RATE: c_int = 1079;
pub const TCP_BBR_MIN_RTO: c_int = 1080;
pub const TCP_BBR_MAX_RTO: c_int = 1081;
pub const TCP_BBR_ALGORITHM: c_int = 1083;
pub const TCP_BBR_PACE_PER_SEC: c_int = 1086;
pub const TCP_BBR_PACE_DEL_TAR: c_int = 1087;
pub const TCP_BBR_PACE_SEG_MAX: c_int = 1088;
pub const TCP_BBR_PACE_SEG_MIN: c_int = 1089;
pub const TCP_BBR_PACE_CROSS: c_int = 1090;
pub const TCP_BBR_TMR_PACE_OH: c_int = 1096;
pub const TCP_BBR_RACK_RTT_USE: c_int = 1098;
pub const TCP_BBR_RETRAN_WTSO: c_int = 1099;
pub const TCP_BBR_PROBE_RTT_GAIN: c_int = 1101;
pub const TCP_BBR_PROBE_RTT_LEN: c_int = 1102;
pub const TCP_BBR_SEND_IWND_IN_TSO: c_int = 1103;
pub const TCP_BBR_USE_RACK_RR: c_int = 1104;
pub const TCP_BBR_HDWR_PACE: c_int = 1105;
pub const TCP_BBR_UTTER_MAX_TSO: c_int = 1106;
pub const TCP_BBR_EXTRA_STATE: c_int = 1107;
pub const TCP_BBR_FLOOR_MIN_TSO: c_int = 1108;
pub const TCP_BBR_MIN_TOPACEOUT: c_int = 1109;
pub const TCP_BBR_TSTMP_RAISES: c_int = 1110;
pub const TCP_BBR_POLICER_DETECT: c_int = 1111;
pub const TCP_BBR_RACK_INIT_RATE: c_int = 1112;

pub const IP_BINDANY: c_int = 24;
pub const IP_BINDMULTI: c_int = 25;
pub const IP_RSS_LISTEN_BUCKET: c_int = 26;
pub const IP_ORIGDSTADDR: c_int = 27;
pub const IP_RECVORIGDSTADDR: c_int = IP_ORIGDSTADDR;

pub const IP_DONTFRAG: c_int = 67;
pub const IP_RECVTOS: c_int = 68;

pub const IPV6_BINDANY: c_int = 64;
pub const IPV6_ORIGDSTADDR: c_int = 72;
pub const IPV6_RECVORIGDSTADDR: c_int = IPV6_ORIGDSTADDR;

pub const PF_SLOW: c_int = AF_SLOW;
pub const PF_SCLUSTER: c_int = AF_SCLUSTER;
pub const PF_ARP: c_int = AF_ARP;
pub const PF_BLUETOOTH: c_int = AF_BLUETOOTH;
pub const PF_IEEE80211: c_int = AF_IEEE80211;
pub const PF_INET_SDP: c_int = AF_INET_SDP;
pub const PF_INET6_SDP: c_int = AF_INET6_SDP;

pub const NET_RT_DUMP: c_int = 1;
pub const NET_RT_FLAGS: c_int = 2;
pub const NET_RT_IFLIST: c_int = 3;
pub const NET_RT_IFMALIST: c_int = 4;
pub const NET_RT_IFLISTL: c_int = 5;

// System V IPC
pub const IPC_INFO: c_int = 3;
pub const MSG_NOERROR: c_int = 0o10000;
pub const SHM_LOCK: c_int = 11;
pub const SHM_UNLOCK: c_int = 12;
pub const SHM_STAT: c_int = 13;
pub const SHM_INFO: c_int = 14;
pub const SHM_ANON: *mut c_char = 1 as *mut c_char;

// The *_MAXID constants never should've been used outside of the
// FreeBSD base system.  And with the exception of CTL_P1003_1B_MAXID,
// they were all removed in svn r262489.  They remain here for backwards
// compatibility only, and are scheduled to be removed in libc 1.0.0.
#[doc(hidden)]
#[deprecated(since = "0.2.54", note = "Removed in FreeBSD 11")]
pub const CTL_MAXID: c_int = 10;
#[doc(hidden)]
#[deprecated(since = "0.2.54", note = "Removed in FreeBSD 11")]
pub const KERN_MAXID: c_int = 38;
#[doc(hidden)]
#[deprecated(since = "0.2.54", note = "Removed in FreeBSD 11")]
pub const HW_MAXID: c_int = 13;
#[doc(hidden)]
#[deprecated(since = "0.2.54", note = "Removed in FreeBSD 11")]
pub const USER_MAXID: c_int = 21;
#[doc(hidden)]
#[deprecated(since = "0.2.74", note = "Removed in FreeBSD 13")]
pub const CTL_P1003_1B_MAXID: c_int = 26;

pub const MSG_NOTIFICATION: c_int = 0x00002000;
pub const MSG_NBIO: c_int = 0x00004000;
pub const MSG_COMPAT: c_int = 0x00008000;
pub const MSG_CMSG_CLOEXEC: c_int = 0x00040000;
pub const MSG_NOSIGNAL: c_int = 0x20000;
pub const MSG_WAITFORONE: c_int = 0x00080000;

// utmpx entry types
pub const EMPTY: c_short = 0;
pub const BOOT_TIME: c_short = 1;
pub const OLD_TIME: c_short = 2;
pub const NEW_TIME: c_short = 3;
pub const USER_PROCESS: c_short = 4;
pub const INIT_PROCESS: c_short = 5;
pub const LOGIN_PROCESS: c_short = 6;
pub const DEAD_PROCESS: c_short = 7;
pub const SHUTDOWN_TIME: c_short = 8;
// utmp database types
pub const UTXDB_ACTIVE: c_int = 0;
pub const UTXDB_LASTLOGIN: c_int = 1;
pub const UTXDB_LOG: c_int = 2;

pub const LC_COLLATE_MASK: c_int = 1 << 0;
pub const LC_CTYPE_MASK: c_int = 1 << 1;
pub const LC_MONETARY_MASK: c_int = 1 << 2;
pub const LC_NUMERIC_MASK: c_int = 1 << 3;
pub const LC_TIME_MASK: c_int = 1 << 4;
pub const LC_MESSAGES_MASK: c_int = 1 << 5;
pub const LC_ALL_MASK: c_int = LC_COLLATE_MASK
    | LC_CTYPE_MASK
    | LC_MESSAGES_MASK
    | LC_MONETARY_MASK
    | LC_NUMERIC_MASK
    | LC_TIME_MASK;

pub const WSTOPPED: c_int = 2; // same as WUNTRACED
pub const WCONTINUED: c_int = 4;
pub const WNOWAIT: c_int = 8;
pub const WEXITED: c_int = 16;
pub const WTRAPPED: c_int = 32;

// FreeBSD defines a great many more of these, we only expose the
// standardized ones.
pub const P_PID: idtype_t = 0;
pub const P_PGID: idtype_t = 2;
pub const P_ALL: idtype_t = 7;

pub const UTIME_OMIT: c_long = -2;
pub const UTIME_NOW: c_long = -1;

pub const B460800: crate::speed_t = 460800;
pub const B921600: crate::speed_t = 921600;

pub const AT_FDCWD: c_int = -100;
pub const AT_EACCESS: c_int = 0x100;
pub const AT_SYMLINK_NOFOLLOW: c_int = 0x200;
pub const AT_SYMLINK_FOLLOW: c_int = 0x400;
pub const AT_REMOVEDIR: c_int = 0x800;
pub const AT_RESOLVE_BENEATH: c_int = 0x2000;
pub const AT_EMPTY_PATH: c_int = 0x4000;

pub const AT_NULL: c_int = 0;
pub const AT_IGNORE: c_int = 1;
pub const AT_EXECFD: c_int = 2;
pub const AT_PHDR: c_int = 3;
pub const AT_PHENT: c_int = 4;
pub const AT_PHNUM: c_int = 5;
pub const AT_PAGESZ: c_int = 6;
pub const AT_BASE: c_int = 7;
pub const AT_FLAGS: c_int = 8;
pub const AT_ENTRY: c_int = 9;
pub const AT_NOTELF: c_int = 10;
pub const AT_UID: c_int = 11;
pub const AT_EUID: c_int = 12;
pub const AT_GID: c_int = 13;
pub const AT_EGID: c_int = 14;
pub const AT_EXECPATH: c_int = 15;
pub const AT_CANARY: c_int = 16;
pub const AT_OSRELDATE: c_int = 18;
pub const AT_NCPUS: c_int = 19;
pub const AT_PAGESIZES: c_int = 20;
pub const AT_TIMEKEEP: c_int = 22;
pub const AT_HWCAP: c_int = 25;
pub const AT_HWCAP2: c_int = 26;
pub const AT_USRSTACKBASE: c_int = 35;
pub const AT_USRSTACKLIM: c_int = 36;

pub const TABDLY: crate::tcflag_t = 0x00000004;
pub const TAB0: crate::tcflag_t = 0x00000000;
pub const TAB3: crate::tcflag_t = 0x00000004;

pub const _PC_ACL_NFS4: c_int = 64;

pub const _SC_CPUSET_SIZE: c_int = 122;

pub const _UUID_NODE_LEN: usize = 6;

// Flags which can be passed to pdfork(2)
pub const PD_DAEMON: c_int = 0x00000001;
pub const PD_CLOEXEC: c_int = 0x00000002;
pub const PD_ALLOWED_AT_FORK: c_int = PD_DAEMON | PD_CLOEXEC;

// Values for struct rtprio (type_ field)
pub const RTP_PRIO_REALTIME: c_ushort = 2;
pub const RTP_PRIO_NORMAL: c_ushort = 3;
pub const RTP_PRIO_IDLE: c_ushort = 4;

// Flags for chflags(2)
pub const UF_SYSTEM: c_ulong = 0x00000080;
pub const UF_SPARSE: c_ulong = 0x00000100;
pub const UF_OFFLINE: c_ulong = 0x00000200;
pub const UF_REPARSE: c_ulong = 0x00000400;
pub const UF_ARCHIVE: c_ulong = 0x00000800;
pub const UF_READONLY: c_ulong = 0x00001000;
pub const UF_HIDDEN: c_ulong = 0x00008000;
pub const SF_SNAPSHOT: c_ulong = 0x00200000;

// fcntl commands
pub const F_ADD_SEALS: c_int = 19;
pub const F_GET_SEALS: c_int = 20;
pub const F_OGETLK: c_int = 7;
pub const F_OSETLK: c_int = 8;
pub const F_OSETLKW: c_int = 9;
pub const F_RDAHEAD: c_int = 16;
pub const F_READAHEAD: c_int = 15;
pub const F_SETLK_REMOTE: c_int = 14;
pub const F_KINFO: c_int = 22;

// for use with F_ADD_SEALS
pub const F_SEAL_GROW: c_int = 4;
pub const F_SEAL_SEAL: c_int = 1;
pub const F_SEAL_SHRINK: c_int = 2;
pub const F_SEAL_WRITE: c_int = 8;

// for use with fspacectl
pub const SPACECTL_DEALLOC: c_int = 1;

// For realhostname* api
pub const HOSTNAME_FOUND: c_int = 0;
pub const HOSTNAME_INCORRECTNAME: c_int = 1;
pub const HOSTNAME_INVALIDADDR: c_int = 2;
pub const HOSTNAME_INVALIDNAME: c_int = 3;

// For rfork
pub const RFFDG: c_int = 4;
pub const RFPROC: c_int = 16;
pub const RFMEM: c_int = 32;
pub const RFNOWAIT: c_int = 64;
pub const RFCFDG: c_int = 4096;
pub const RFTHREAD: c_int = 8192;
pub const RFSIGSHARE: c_int = 16384;
pub const RFLINUXTHPN: c_int = 65536;
pub const RFTSIGZMB: c_int = 524288;
pub const RFSPAWN: c_int = 2147483648;

// For eventfd
pub const EFD_SEMAPHORE: c_int = 0x1;
pub const EFD_NONBLOCK: c_int = 0x4;
pub const EFD_CLOEXEC: c_int = 0x100000;

pub const MALLOCX_ZERO: c_int = 0x40;

/// size of returned wchan message
pub const WMESGLEN: usize = 8;
/// size of returned lock name
pub const LOCKNAMELEN: usize = 8;
/// size of returned thread name
pub const TDNAMLEN: usize = 16;
/// size of returned ki_comm name
pub const COMMLEN: usize = 19;
/// size of returned ki_emul
pub const KI_EMULNAMELEN: usize = 16;
/// number of groups in ki_groups
pub const KI_NGROUPS: usize = 16;
cfg_if! {
    if #[cfg(freebsd11)] {
        pub const KI_NSPARE_INT: usize = 4;
    } else {
        pub const KI_NSPARE_INT: usize = 2;
    }
}
pub const KI_NSPARE_LONG: usize = 12;
/// Flags for the process credential.
pub const KI_CRF_CAPABILITY_MODE: usize = 0x00000001;
/// Steal a bit from ki_cr_flags to indicate that the cred had more than
/// KI_NGROUPS groups.
pub const KI_CRF_GRP_OVERFLOW: usize = 0x80000000;
/// controlling tty vnode active
pub const KI_CTTY: usize = 0x00000001;
/// session leader
pub const KI_SLEADER: usize = 0x00000002;
/// proc blocked on lock ki_lockname
pub const KI_LOCKBLOCK: usize = 0x00000004;
/// size of returned ki_login
pub const LOGNAMELEN: usize = 17;
/// size of returned ki_loginclass
pub const LOGINCLASSLEN: usize = 17;

pub const KF_ATTR_VALID: c_int = 0x0001;
pub const KF_TYPE_NONE: c_int = 0;
pub const KF_TYPE_VNODE: c_int = 1;
pub const KF_TYPE_SOCKET: c_int = 2;
pub const KF_TYPE_PIPE: c_int = 3;
pub const KF_TYPE_FIFO: c_int = 4;
pub const KF_TYPE_KQUEUE: c_int = 5;
pub const KF_TYPE_MQUEUE: c_int = 7;
pub const KF_TYPE_SHM: c_int = 8;
pub const KF_TYPE_SEM: c_int = 9;
pub const KF_TYPE_PTS: c_int = 10;
pub const KF_TYPE_PROCDESC: c_int = 11;
pub const KF_TYPE_DEV: c_int = 12;
pub const KF_TYPE_UNKNOWN: c_int = 255;

pub const KF_VTYPE_VNON: c_int = 0;
pub const KF_VTYPE_VREG: c_int = 1;
pub const KF_VTYPE_VDIR: c_int = 2;
pub const KF_VTYPE_VBLK: c_int = 3;
pub const KF_VTYPE_VCHR: c_int = 4;
pub const KF_VTYPE_VLNK: c_int = 5;
pub const KF_VTYPE_VSOCK: c_int = 6;
pub const KF_VTYPE_VFIFO: c_int = 7;
pub const KF_VTYPE_VBAD: c_int = 8;
pub const KF_VTYPE_UNKNOWN: c_int = 255;

/// Current working directory
pub const KF_FD_TYPE_CWD: c_int = -1;
/// Root directory
pub const KF_FD_TYPE_ROOT: c_int = -2;
/// Jail directory
pub const KF_FD_TYPE_JAIL: c_int = -3;
/// Ktrace vnode
pub const KF_FD_TYPE_TRACE: c_int = -4;
pub const KF_FD_TYPE_TEXT: c_int = -5;
/// Controlling terminal
pub const KF_FD_TYPE_CTTY: c_int = -6;
pub const KF_FLAG_READ: c_int = 0x00000001;
pub const KF_FLAG_WRITE: c_int = 0x00000002;
pub const KF_FLAG_APPEND: c_int = 0x00000004;
pub const KF_FLAG_ASYNC: c_int = 0x00000008;
pub const KF_FLAG_FSYNC: c_int = 0x00000010;
pub const KF_FLAG_NONBLOCK: c_int = 0x00000020;
pub const KF_FLAG_DIRECT: c_int = 0x00000040;
pub const KF_FLAG_HASLOCK: c_int = 0x00000080;
pub const KF_FLAG_SHLOCK: c_int = 0x00000100;
pub const KF_FLAG_EXLOCK: c_int = 0x00000200;
pub const KF_FLAG_NOFOLLOW: c_int = 0x00000400;
pub const KF_FLAG_CREAT: c_int = 0x00000800;
pub const KF_FLAG_TRUNC: c_int = 0x00001000;
pub const KF_FLAG_EXCL: c_int = 0x00002000;
pub const KF_FLAG_EXEC: c_int = 0x00004000;

pub const KVME_TYPE_NONE: c_int = 0;
pub const KVME_TYPE_DEFAULT: c_int = 1;
pub const KVME_TYPE_VNODE: c_int = 2;
pub const KVME_TYPE_SWAP: c_int = 3;
pub const KVME_TYPE_DEVICE: c_int = 4;
pub const KVME_TYPE_PHYS: c_int = 5;
pub const KVME_TYPE_DEAD: c_int = 6;
pub const KVME_TYPE_SG: c_int = 7;
pub const KVME_TYPE_MGTDEVICE: c_int = 8;
// Present in `sys/user.h` but is undefined for whatever reason...
// pub const KVME_TYPE_GUARD: c_int = 9;
pub const KVME_TYPE_UNKNOWN: c_int = 255;
pub const KVME_PROT_READ: c_int = 0x00000001;
pub const KVME_PROT_WRITE: c_int = 0x00000002;
pub const KVME_PROT_EXEC: c_int = 0x00000004;
pub const KVME_FLAG_COW: c_int = 0x00000001;
pub const KVME_FLAG_NEEDS_COPY: c_int = 0x00000002;
pub const KVME_FLAG_NOCOREDUMP: c_int = 0x00000004;
pub const KVME_FLAG_SUPER: c_int = 0x00000008;
pub const KVME_FLAG_GROWS_UP: c_int = 0x00000010;
pub const KVME_FLAG_GROWS_DOWN: c_int = 0x00000020;
pub const KVME_FLAG_USER_WIRED: c_int = 0x00000040;

pub const KKST_MAXLEN: c_int = 1024;
/// Stack is valid.
pub const KKST_STATE_STACKOK: c_int = 0;
/// Stack swapped out.
pub const KKST_STATE_SWAPPED: c_int = 1;
pub const KKST_STATE_RUNNING: c_int = 2;

// Constants about priority.
pub const PRI_MIN: c_int = 0;
pub const PRI_MAX: c_int = 255;
pub const PRI_MIN_ITHD: c_int = PRI_MIN;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PRI_MAX_ITHD: c_int = PRI_MIN_REALTIME - 1;
pub const PI_REALTIME: c_int = PRI_MIN_ITHD + 0;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const PI_AV: c_int = PRI_MIN_ITHD + 4;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const PI_NET: c_int = PRI_MIN_ITHD + 8;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const PI_DISK: c_int = PRI_MIN_ITHD + 12;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const PI_TTY: c_int = PRI_MIN_ITHD + 16;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const PI_DULL: c_int = PRI_MIN_ITHD + 20;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const PI_SOFT: c_int = PRI_MIN_ITHD + 24;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const PRI_MIN_REALTIME: c_int = 48;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PRI_MAX_REALTIME: c_int = PRI_MIN_KERN - 1;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const PRI_MIN_KERN: c_int = 80;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PRI_MAX_KERN: c_int = PRI_MIN_TIMESHARE - 1;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PSWP: c_int = PRI_MIN_KERN + 0;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PVM: c_int = PRI_MIN_KERN + 4;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PINOD: c_int = PRI_MIN_KERN + 8;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PRIBIO: c_int = PRI_MIN_KERN + 12;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PVFS: c_int = PRI_MIN_KERN + 16;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PZERO: c_int = PRI_MIN_KERN + 20;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PSOCK: c_int = PRI_MIN_KERN + 24;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PWAIT: c_int = PRI_MIN_KERN + 28;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PLOCK: c_int = PRI_MIN_KERN + 32;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PPAUSE: c_int = PRI_MIN_KERN + 36;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const PRI_MIN_TIMESHARE: c_int = 120;
pub const PRI_MAX_TIMESHARE: c_int = PRI_MIN_IDLE - 1;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
#[allow(deprecated)]
pub const PUSER: c_int = PRI_MIN_TIMESHARE;
pub const PRI_MIN_IDLE: c_int = 224;
pub const PRI_MAX_IDLE: c_int = PRI_MAX;

pub const NZERO: c_int = 0;

// Resource utilization information.
pub const RUSAGE_THREAD: c_int = 1;

cfg_if! {
    if #[cfg(any(freebsd11, target_pointer_width = "32"))] {
        pub const ARG_MAX: c_int = 256 * 1024;
    } else {
        pub const ARG_MAX: c_int = 2 * 256 * 1024;
    }
}
pub const CHILD_MAX: c_int = 40;
/// max command name remembered
pub const MAXCOMLEN: usize = 19;
/// max interpreter file name length
pub const MAXINTERP: c_int = crate::PATH_MAX;
/// max login name length (incl. NUL)
pub const MAXLOGNAME: c_int = 33;
/// max simultaneous processes
pub const MAXUPRC: c_int = CHILD_MAX;
/// max bytes for an exec function
pub const NCARGS: c_int = ARG_MAX;
///  /* max number groups
pub const NGROUPS: c_int = NGROUPS_MAX + 1;
/// max open files per process
pub const NOFILE: c_int = OPEN_MAX;
/// marker for empty group set member
pub const NOGROUP: c_int = 65535;
/// max hostname size
pub const MAXHOSTNAMELEN: c_int = 256;
/// max bytes in term canon input line
pub const MAX_CANON: c_int = 255;
/// max bytes in terminal input
pub const MAX_INPUT: c_int = 255;
/// max bytes in a file name
pub const NAME_MAX: c_int = 255;
pub const MAXSYMLINKS: c_int = 32;
/// max supplemental group id's
pub const NGROUPS_MAX: c_int = 1023;
/// max open files per process
pub const OPEN_MAX: c_int = 64;

pub const _POSIX_ARG_MAX: c_int = 4096;
pub const _POSIX_LINK_MAX: c_int = 8;
pub const _POSIX_MAX_CANON: c_int = 255;
pub const _POSIX_MAX_INPUT: c_int = 255;
pub const _POSIX_NAME_MAX: c_int = 14;
pub const _POSIX_PIPE_BUF: c_int = 512;
pub const _POSIX_SSIZE_MAX: c_int = 32767;
pub const _POSIX_STREAM_MAX: c_int = 8;

/// max ibase/obase values in bc(1)
pub const BC_BASE_MAX: c_int = 99;
/// max array elements in bc(1)
pub const BC_DIM_MAX: c_int = 2048;
/// max scale value in bc(1)
pub const BC_SCALE_MAX: c_int = 99;
/// max const string length in bc(1)
pub const BC_STRING_MAX: c_int = 1000;
/// max character class name size
pub const CHARCLASS_NAME_MAX: c_int = 14;
/// max weights for order keyword
pub const COLL_WEIGHTS_MAX: c_int = 10;
/// max expressions nested in expr(1)
pub const EXPR_NEST_MAX: c_int = 32;
/// max bytes in an input line
pub const LINE_MAX: c_int = 2048;
/// max RE's in interval notation
pub const RE_DUP_MAX: c_int = 255;

pub const _POSIX2_BC_BASE_MAX: c_int = 99;
pub const _POSIX2_BC_DIM_MAX: c_int = 2048;
pub const _POSIX2_BC_SCALE_MAX: c_int = 99;
pub const _POSIX2_BC_STRING_MAX: c_int = 1000;
pub const _POSIX2_CHARCLASS_NAME_MAX: c_int = 14;
pub const _POSIX2_COLL_WEIGHTS_MAX: c_int = 2;
pub const _POSIX2_EQUIV_CLASS_MAX: c_int = 2;
pub const _POSIX2_EXPR_NEST_MAX: c_int = 32;
pub const _POSIX2_LINE_MAX: c_int = 2048;
pub const _POSIX2_RE_DUP_MAX: c_int = 255;

// sys/proc.h
pub const TDF_BORROWING: c_int = 0x00000001;
pub const TDF_INPANIC: c_int = 0x00000002;
pub const TDF_INMEM: c_int = 0x00000004;
pub const TDF_SINTR: c_int = 0x00000008;
pub const TDF_TIMEOUT: c_int = 0x00000010;
pub const TDF_IDLETD: c_int = 0x00000020;
pub const TDF_CANSWAP: c_int = 0x00000040;
pub const TDF_KTH_SUSP: c_int = 0x00000100;
pub const TDF_ALLPROCSUSP: c_int = 0x00000200;
pub const TDF_BOUNDARY: c_int = 0x00000400;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const TDF_ASTPENDING: c_int = 0x00000800;
pub const TDF_SBDRY: c_int = 0x00002000;
pub const TDF_UPIBLOCKED: c_int = 0x00004000;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const TDF_NEEDSUSPCHK: c_int = 0x00008000;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const TDF_NEEDRESCHED: c_int = 0x00010000;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const TDF_NEEDSIGCHK: c_int = 0x00020000;
pub const TDF_NOLOAD: c_int = 0x00040000;
pub const TDF_SERESTART: c_int = 0x00080000;
pub const TDF_THRWAKEUP: c_int = 0x00100000;
pub const TDF_SEINTR: c_int = 0x00200000;
pub const TDF_SWAPINREQ: c_int = 0x00400000;
#[deprecated(since = "0.2.133", note = "Removed in FreeBSD 14")]
pub const TDF_UNUSED23: c_int = 0x00800000;
pub const TDF_SCHED0: c_int = 0x01000000;
pub const TDF_SCHED1: c_int = 0x02000000;
pub const TDF_SCHED2: c_int = 0x04000000;
pub const TDF_SCHED3: c_int = 0x08000000;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const TDF_ALRMPEND: c_int = 0x10000000;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const TDF_PROFPEND: c_int = 0x20000000;
#[deprecated(since = "0.2.133", note = "Not stable across OS versions")]
pub const TDF_MACPEND: c_int = 0x40000000;

pub const TDB_SUSPEND: c_int = 0x00000001;
pub const TDB_XSIG: c_int = 0x00000002;
pub const TDB_USERWR: c_int = 0x00000004;
pub const TDB_SCE: c_int = 0x00000008;
pub const TDB_SCX: c_int = 0x00000010;
pub const TDB_EXEC: c_int = 0x00000020;
pub const TDB_FORK: c_int = 0x00000040;
pub const TDB_STOPATFORK: c_int = 0x00000080;
pub const TDB_CHILD: c_int = 0x00000100;
pub const TDB_BORN: c_int = 0x00000200;
pub const TDB_EXIT: c_int = 0x00000400;
pub const TDB_VFORK: c_int = 0x00000800;
pub const TDB_FSTP: c_int = 0x00001000;
pub const TDB_STEP: c_int = 0x00002000;

pub const TDP_OLDMASK: c_int = 0x00000001;
pub const TDP_INKTR: c_int = 0x00000002;
pub const TDP_INKTRACE: c_int = 0x00000004;
pub const TDP_BUFNEED: c_int = 0x00000008;
pub const TDP_COWINPROGRESS: c_int = 0x00000010;
pub const TDP_ALTSTACK: c_int = 0x00000020;
pub const TDP_DEADLKTREAT: c_int = 0x00000040;
pub const TDP_NOFAULTING: c_int = 0x00000080;
pub const TDP_OWEUPC: c_int = 0x00000200;
pub const TDP_ITHREAD: c_int = 0x00000400;
pub const TDP_SYNCIO: c_int = 0x00000800;
pub const TDP_SCHED1: c_int = 0x00001000;
pub const TDP_SCHED2: c_int = 0x00002000;
pub const TDP_SCHED3: c_int = 0x00004000;
pub const TDP_SCHED4: c_int = 0x00008000;
pub const TDP_GEOM: c_int = 0x00010000;
pub const TDP_SOFTDEP: c_int = 0x00020000;
pub const TDP_NORUNNINGBUF: c_int = 0x00040000;
pub const TDP_WAKEUP: c_int = 0x00080000;
pub const TDP_INBDFLUSH: c_int = 0x00100000;
pub const TDP_KTHREAD: c_int = 0x00200000;
pub const TDP_CALLCHAIN: c_int = 0x00400000;
pub const TDP_IGNSUSP: c_int = 0x00800000;
pub const TDP_AUDITREC: c_int = 0x01000000;
pub const TDP_RFPPWAIT: c_int = 0x02000000;
pub const TDP_RESETSPUR: c_int = 0x04000000;
pub const TDP_NERRNO: c_int = 0x08000000;
pub const TDP_EXECVMSPC: c_int = 0x40000000;

pub const TDI_SUSPENDED: c_int = 0x0001;
pub const TDI_SLEEPING: c_int = 0x0002;
pub const TDI_SWAPPED: c_int = 0x0004;
pub const TDI_LOCK: c_int = 0x0008;
pub const TDI_IWAIT: c_int = 0x0010;

pub const P_ADVLOCK: c_int = 0x00000001;
pub const P_CONTROLT: c_int = 0x00000002;
pub const P_KPROC: c_int = 0x00000004;
#[deprecated(since = "1.0", note = "Replaced in FreeBSD 15 by P_IDLEPROC")]
pub const P_UNUSED3: c_int = 0x00000008;
pub const P_IDLEPROC: c_int = 0x00000008;
pub const P_PPWAIT: c_int = 0x00000010;
pub const P_PROFIL: c_int = 0x00000020;
pub const P_STOPPROF: c_int = 0x00000040;
pub const P_HADTHREADS: c_int = 0x00000080;
pub const P_SUGID: c_int = 0x00000100;
pub const P_SYSTEM: c_int = 0x00000200;
pub const P_SINGLE_EXIT: c_int = 0x00000400;
pub const P_TRACED: c_int = 0x00000800;
pub const P_WAITED: c_int = 0x00001000;
pub const P_WEXIT: c_int = 0x00002000;
pub const P_EXEC: c_int = 0x00004000;
pub const P_WKILLED: c_int = 0x00008000;
pub const P_CONTINUED: c_int = 0x00010000;
pub const P_STOPPED_SIG: c_int = 0x00020000;
pub const P_STOPPED_TRACE: c_int = 0x00040000;
pub const P_STOPPED_SINGLE: c_int = 0x00080000;
pub const P_PROTECTED: c_int = 0x00100000;
pub const P_SIGEVENT: c_int = 0x00200000;
pub const P_SINGLE_BOUNDARY: c_int = 0x00400000;
pub const P_HWPMC: c_int = 0x00800000;
pub const P_JAILED: c_int = 0x01000000;
pub const P_TOTAL_STOP: c_int = 0x02000000;
pub const P_INEXEC: c_int = 0x04000000;
pub const P_STATCHILD: c_int = 0x08000000;
pub const P_INMEM: c_int = 0x10000000;
pub const P_SWAPPINGOUT: c_int = 0x20000000;
pub const P_SWAPPINGIN: c_int = 0x40000000;
pub const P_PPTRACE: c_int = 0x80000000;
pub const P_STOPPED: c_int = P_STOPPED_SIG | P_STOPPED_SINGLE | P_STOPPED_TRACE;

pub const P2_INHERIT_PROTECTED: c_int = 0x00000001;
pub const P2_NOTRACE: c_int = 0x00000002;
pub const P2_NOTRACE_EXEC: c_int = 0x00000004;
pub const P2_AST_SU: c_int = 0x00000008;
pub const P2_PTRACE_FSTP: c_int = 0x00000010;
pub const P2_TRAPCAP: c_int = 0x00000020;
pub const P2_STKGAP_DISABLE: c_int = 0x00000800;
pub const P2_STKGAP_DISABLE_EXEC: c_int = 0x00001000;

pub const P_TREE_ORPHANED: c_int = 0x00000001;
pub const P_TREE_FIRST_ORPHAN: c_int = 0x00000002;
pub const P_TREE_REAPER: c_int = 0x00000004;

pub const SIDL: c_char = 1;
pub const SRUN: c_char = 2;
pub const SSLEEP: c_char = 3;
pub const SSTOP: c_char = 4;
pub const SZOMB: c_char = 5;
pub const SWAIT: c_char = 6;
pub const SLOCK: c_char = 7;

pub const P_MAGIC: c_int = 0xbeefface;

pub const TDP_SIGFASTBLOCK: c_int = 0x00000100;
pub const TDP_UIOHELD: c_int = 0x10000000;
pub const TDP_SIGFASTPENDING: c_int = 0x80000000;
pub const TDP2_COMPAT32RB: c_int = 0x00000002;
pub const P2_PROTMAX_ENABLE: c_int = 0x00000200;
pub const P2_PROTMAX_DISABLE: c_int = 0x00000400;
pub const TDP2_SBPAGES: c_int = 0x00000001;
pub const P2_ASLR_ENABLE: c_int = 0x00000040;
pub const P2_ASLR_DISABLE: c_int = 0x00000080;
pub const P2_ASLR_IGNSTART: c_int = 0x00000100;
pub const P_TREE_GRPEXITED: c_int = 0x00000008;

// libprocstat.h
pub const PS_FST_VTYPE_VNON: c_int = 1;
pub const PS_FST_VTYPE_VREG: c_int = 2;
pub const PS_FST_VTYPE_VDIR: c_int = 3;
pub const PS_FST_VTYPE_VBLK: c_int = 4;
pub const PS_FST_VTYPE_VCHR: c_int = 5;
pub const PS_FST_VTYPE_VLNK: c_int = 6;
pub const PS_FST_VTYPE_VSOCK: c_int = 7;
pub const PS_FST_VTYPE_VFIFO: c_int = 8;
pub const PS_FST_VTYPE_VBAD: c_int = 9;
pub const PS_FST_VTYPE_UNKNOWN: c_int = 255;

pub const PS_FST_TYPE_VNODE: c_int = 1;
pub const PS_FST_TYPE_FIFO: c_int = 2;
pub const PS_FST_TYPE_SOCKET: c_int = 3;
pub const PS_FST_TYPE_PIPE: c_int = 4;
pub const PS_FST_TYPE_PTS: c_int = 5;
pub const PS_FST_TYPE_KQUEUE: c_int = 6;
pub const PS_FST_TYPE_MQUEUE: c_int = 8;
pub const PS_FST_TYPE_SHM: c_int = 9;
pub const PS_FST_TYPE_SEM: c_int = 10;
pub const PS_FST_TYPE_UNKNOWN: c_int = 11;
pub const PS_FST_TYPE_NONE: c_int = 12;
pub const PS_FST_TYPE_PROCDESC: c_int = 13;
pub const PS_FST_TYPE_DEV: c_int = 14;
pub const PS_FST_TYPE_EVENTFD: c_int = 15;

pub const PS_FST_UFLAG_RDIR: c_int = 0x0001;
pub const PS_FST_UFLAG_CDIR: c_int = 0x0002;
pub const PS_FST_UFLAG_JAIL: c_int = 0x0004;
pub const PS_FST_UFLAG_TRACE: c_int = 0x0008;
pub const PS_FST_UFLAG_TEXT: c_int = 0x0010;
pub const PS_FST_UFLAG_MMAP: c_int = 0x0020;
pub const PS_FST_UFLAG_CTTY: c_int = 0x0040;

pub const PS_FST_FFLAG_READ: c_int = 0x0001;
pub const PS_FST_FFLAG_WRITE: c_int = 0x0002;
pub const PS_FST_FFLAG_NONBLOCK: c_int = 0x0004;
pub const PS_FST_FFLAG_APPEND: c_int = 0x0008;
pub const PS_FST_FFLAG_SHLOCK: c_int = 0x0010;
pub const PS_FST_FFLAG_EXLOCK: c_int = 0x0020;
pub const PS_FST_FFLAG_ASYNC: c_int = 0x0040;
pub const PS_FST_FFLAG_SYNC: c_int = 0x0080;
pub const PS_FST_FFLAG_NOFOLLOW: c_int = 0x0100;
pub const PS_FST_FFLAG_CREAT: c_int = 0x0200;
pub const PS_FST_FFLAG_TRUNC: c_int = 0x0400;
pub const PS_FST_FFLAG_EXCL: c_int = 0x0800;
pub const PS_FST_FFLAG_DIRECT: c_int = 0x1000;
pub const PS_FST_FFLAG_EXEC: c_int = 0x2000;
pub const PS_FST_FFLAG_HASLOCK: c_int = 0x4000;

// sys/mount.h

/// File identifier.
/// These are unique per filesystem on a single machine.
///
/// Note that the offset of fid_data is 4 bytes, so care must be taken to avoid
/// undefined behavior accessing unaligned fields within an embedded struct.
pub const MAXFIDSZ: c_int = 16;
/// Length of type name including null.
pub const MFSNAMELEN: c_int = 16;
cfg_if! {
    if #[cfg(any(freebsd10, freebsd11))] {
        /// Size of on/from name bufs.
        pub const MNAMELEN: c_int = 88;
    } else {
        /// Size of on/from name bufs.
        pub const MNAMELEN: c_int = 1024;
    }
}

/// Using journaled soft updates.
pub const MNT_SUJ: u64 = 0x100000000;
/// Mounted by automountd(8).
pub const MNT_AUTOMOUNTED: u64 = 0x200000000;
/// Filesys metadata untrusted.
pub const MNT_UNTRUSTED: u64 = 0x800000000;

/// Require TLS.
pub const MNT_EXTLS: u64 = 0x4000000000;
/// Require TLS with client cert.
pub const MNT_EXTLSCERT: u64 = 0x8000000000;
/// Require TLS with user cert.
pub const MNT_EXTLSCERTUSER: u64 = 0x10000000000;

/// Filesystem is stored locally.
pub const MNT_LOCAL: u64 = 0x000001000;
/// Quotas are enabled on fs.
pub const MNT_QUOTA: u64 = 0x000002000;
/// Identifies the root fs.
pub const MNT_ROOTFS: u64 = 0x000004000;
/// Mounted by a user.
pub const MNT_USER: u64 = 0x000008000;
/// Do not show entry in df.
pub const MNT_IGNORE: u64 = 0x000800000;
/// Filesystem is verified.
pub const MNT_VERIFIED: u64 = 0x400000000;

/// Do not cover a mount point.
pub const MNT_NOCOVER: u64 = 0x001000000000;
/// Only mount on empty dir.
pub const MNT_EMPTYDIR: u64 = 0x002000000000;
/// Recursively unmount uppers.
pub const MNT_RECURSE: u64 = 0x100000000000;
/// Unmount in async context.
pub const MNT_DEFERRED: u64 = 0x200000000000;

/// Get configured filesystems.
pub const VFS_VFSCONF: c_int = 0;
/// Generic filesystem information.
pub const VFS_GENERIC: c_int = 0;

/// int: highest defined filesystem type.
pub const VFS_MAXTYPENUM: c_int = 1;
/// struct: vfsconf for filesystem given as next argument.
pub const VFS_CONF: c_int = 2;

/// Synchronously wait for I/O to complete.
pub const MNT_WAIT: c_int = 1;
/// Start all I/O, but do not wait for it.
pub const MNT_NOWAIT: c_int = 2;
/// Push data not written by filesystem syncer.
pub const MNT_LAZY: c_int = 3;
/// Suspend file system after sync.
pub const MNT_SUSPEND: c_int = 4;

pub const MAXSECFLAVORS: c_int = 5;

/// Statically compiled into kernel.
pub const VFCF_STATIC: c_int = 0x00010000;
/// May get data over the network.
pub const VFCF_NETWORK: c_int = 0x00020000;
/// Writes are not implemented.
pub const VFCF_READONLY: c_int = 0x00040000;
/// Data does not represent real files.
pub const VFCF_SYNTHETIC: c_int = 0x00080000;
/// Aliases some other mounted FS.
pub const VFCF_LOOPBACK: c_int = 0x00100000;
/// Stores file names as Unicode.
pub const VFCF_UNICODE: c_int = 0x00200000;
/// Can be mounted from within a jail.
pub const VFCF_JAIL: c_int = 0x00400000;
/// Supports delegated administration.
pub const VFCF_DELEGADMIN: c_int = 0x00800000;
/// Stop at Boundary: defer stop requests to kernel->user (AST) transition.
pub const VFCF_SBDRY: c_int = 0x01000000;

// time.h

/// not on dst
pub const DST_NONE: c_int = 0;
/// USA style dst
pub const DST_USA: c_int = 1;
/// Australian style dst
pub const DST_AUST: c_int = 2;
/// Western European dst
pub const DST_WET: c_int = 3;
/// Middle European dst
pub const DST_MET: c_int = 4;
/// Eastern European dst
pub const DST_EET: c_int = 5;
/// Canada
pub const DST_CAN: c_int = 6;

pub const CPUCLOCK_WHICH_PID: c_int = 0;
pub const CPUCLOCK_WHICH_TID: c_int = 1;

pub const MFD_CLOEXEC: c_uint = 0x00000001;
pub const MFD_ALLOW_SEALING: c_uint = 0x00000002;
pub const MFD_HUGETLB: c_uint = 0x00000004;
pub const MFD_HUGE_MASK: c_uint = 0xFC000000;
pub const MFD_HUGE_64KB: c_uint = 16 << 26;
pub const MFD_HUGE_512KB: c_uint = 19 << 26;
pub const MFD_HUGE_1MB: c_uint = 20 << 26;
pub const MFD_HUGE_2MB: c_uint = 21 << 26;
pub const MFD_HUGE_8MB: c_uint = 23 << 26;
pub const MFD_HUGE_16MB: c_uint = 24 << 26;
pub const MFD_HUGE_32MB: c_uint = 25 << 26;
pub const MFD_HUGE_256MB: c_uint = 28 << 26;
pub const MFD_HUGE_512MB: c_uint = 29 << 26;
pub const MFD_HUGE_1GB: c_uint = 30 << 26;
pub const MFD_HUGE_2GB: c_uint = 31 << 26;
pub const MFD_HUGE_16GB: c_uint = 34 << 26;

pub const SHM_LARGEPAGE_ALLOC_DEFAULT: c_int = 0;
pub const SHM_LARGEPAGE_ALLOC_NOWAIT: c_int = 1;
pub const SHM_LARGEPAGE_ALLOC_HARD: c_int = 2;
pub const SHM_RENAME_NOREPLACE: c_int = 1 << 0;
pub const SHM_RENAME_EXCHANGE: c_int = 1 << 1;

// sys/umtx.h

pub const UMTX_OP_WAIT: c_int = 2;
pub const UMTX_OP_WAKE: c_int = 3;
pub const UMTX_OP_MUTEX_TRYLOCK: c_int = 4;
pub const UMTX_OP_MUTEX_LOCK: c_int = 5;
pub const UMTX_OP_MUTEX_UNLOCK: c_int = 6;
pub const UMTX_OP_SET_CEILING: c_int = 7;
pub const UMTX_OP_CV_WAIT: c_int = 8;
pub const UMTX_OP_CV_SIGNAL: c_int = 9;
pub const UMTX_OP_CV_BROADCAST: c_int = 10;
pub const UMTX_OP_WAIT_UINT: c_int = 11;
pub const UMTX_OP_RW_RDLOCK: c_int = 12;
pub const UMTX_OP_RW_WRLOCK: c_int = 13;
pub const UMTX_OP_RW_UNLOCK: c_int = 14;
pub const UMTX_OP_WAIT_UINT_PRIVATE: c_int = 15;
pub const UMTX_OP_WAKE_PRIVATE: c_int = 16;
pub const UMTX_OP_MUTEX_WAIT: c_int = 17;
pub const UMTX_OP_NWAKE_PRIVATE: c_int = 21;
pub const UMTX_OP_MUTEX_WAKE2: c_int = 22;
pub const UMTX_OP_SEM2_WAIT: c_int = 23;
pub const UMTX_OP_SEM2_WAKE: c_int = 24;
pub const UMTX_OP_SHM: c_int = 25;
pub const UMTX_OP_ROBUST_LISTS: c_int = 26;

pub const UMTX_ABSTIME: u32 = 1;

pub const CPU_LEVEL_ROOT: c_int = 1;
pub const CPU_LEVEL_CPUSET: c_int = 2;
pub const CPU_LEVEL_WHICH: c_int = 3;

pub const CPU_WHICH_TID: c_int = 1;
pub const CPU_WHICH_PID: c_int = 2;
pub const CPU_WHICH_CPUSET: c_int = 3;
pub const CPU_WHICH_IRQ: c_int = 4;
pub const CPU_WHICH_JAIL: c_int = 5;

// net/route.h
pub const RTF_LLDATA: c_int = 0x400;
pub const RTF_FIXEDMTU: c_int = 0x80000;

pub const RTM_VERSION: c_int = 5;

pub const RTAX_MAX: c_int = 8;

// sys/signal.h
pub const SIGTHR: c_int = 32;
pub const SIGLWP: c_int = SIGTHR;
pub const SIGLIBRT: c_int = 33;

// netinet/sctp.h
pub const SCTP_FUTURE_ASSOC: c_int = 0;
pub const SCTP_CURRENT_ASSOC: c_int = 1;
pub const SCTP_ALL_ASSOC: c_int = 2;

pub const SCTP_NO_NEXT_MSG: c_int = 0x0000;
pub const SCTP_NEXT_MSG_AVAIL: c_int = 0x0001;
pub const SCTP_NEXT_MSG_ISCOMPLETE: c_int = 0x0002;
pub const SCTP_NEXT_MSG_IS_UNORDERED: c_int = 0x0004;
pub const SCTP_NEXT_MSG_IS_NOTIFICATION: c_int = 0x0008;

pub const SCTP_RECVV_NOINFO: c_int = 0;
pub const SCTP_RECVV_RCVINFO: c_int = 1;
pub const SCTP_RECVV_NXTINFO: c_int = 2;
pub const SCTP_RECVV_RN: c_int = 3;

pub const SCTP_SENDV_NOINFO: c_int = 0;
pub const SCTP_SENDV_SNDINFO: c_int = 1;
pub const SCTP_SENDV_PRINFO: c_int = 2;
pub const SCTP_SENDV_AUTHINFO: c_int = 3;
pub const SCTP_SENDV_SPA: c_int = 4;

pub const SCTP_SEND_SNDINFO_VALID: c_int = 0x00000001;
pub const SCTP_SEND_PRINFO_VALID: c_int = 0x00000002;
pub const SCTP_SEND_AUTHINFO_VALID: c_int = 0x00000004;

pub const SCTP_NOTIFICATION: c_int = 0x0010;
pub const SCTP_COMPLETE: c_int = 0x0020;
pub const SCTP_EOF: c_int = 0x0100;
pub const SCTP_ABORT: c_int = 0x0200;
pub const SCTP_UNORDERED: c_int = 0x0400;
pub const SCTP_ADDR_OVER: c_int = 0x0800;
pub const SCTP_SENDALL: c_int = 0x1000;
pub const SCTP_EOR: c_int = 0x2000;
pub const SCTP_SACK_IMMEDIATELY: c_int = 0x4000;
pub const SCTP_PR_SCTP_NONE: c_int = 0x0000;
pub const SCTP_PR_SCTP_TTL: c_int = 0x0001;
pub const SCTP_PR_SCTP_PRIO: c_int = 0x0002;
pub const SCTP_PR_SCTP_BUF: c_int = SCTP_PR_SCTP_PRIO;
pub const SCTP_PR_SCTP_RTX: c_int = 0x0003;
pub const SCTP_PR_SCTP_MAX: c_int = SCTP_PR_SCTP_RTX;
pub const SCTP_PR_SCTP_ALL: c_int = 0x000f;

pub const SCTP_INIT: c_int = 0x0001;
pub const SCTP_SNDRCV: c_int = 0x0002;
pub const SCTP_EXTRCV: c_int = 0x0003;
pub const SCTP_SNDINFO: c_int = 0x0004;
pub const SCTP_RCVINFO: c_int = 0x0005;
pub const SCTP_NXTINFO: c_int = 0x0006;
pub const SCTP_PRINFO: c_int = 0x0007;
pub const SCTP_AUTHINFO: c_int = 0x0008;
pub const SCTP_DSTADDRV4: c_int = 0x0009;
pub const SCTP_DSTADDRV6: c_int = 0x000a;

pub const SCTP_RTOINFO: c_int = 0x00000001;
pub const SCTP_ASSOCINFO: c_int = 0x00000002;
pub const SCTP_INITMSG: c_int = 0x00000003;
pub const SCTP_NODELAY: c_int = 0x00000004;
pub const SCTP_AUTOCLOSE: c_int = 0x00000005;
pub const SCTP_SET_PEER_PRIMARY_ADDR: c_int = 0x00000006;
pub const SCTP_PRIMARY_ADDR: c_int = 0x00000007;
pub const SCTP_ADAPTATION_LAYER: c_int = 0x00000008;
pub const SCTP_ADAPTION_LAYER: c_int = 0x00000008;
pub const SCTP_DISABLE_FRAGMENTS: c_int = 0x00000009;
pub const SCTP_PEER_ADDR_PARAMS: c_int = 0x0000000a;
pub const SCTP_DEFAULT_SEND_PARAM: c_int = 0x0000000b;
pub const SCTP_EVENTS: c_int = 0x0000000c;
pub const SCTP_I_WANT_MAPPED_V4_ADDR: c_int = 0x0000000d;
pub const SCTP_MAXSEG: c_int = 0x0000000e;
pub const SCTP_DELAYED_SACK: c_int = 0x0000000f;
pub const SCTP_FRAGMENT_INTERLEAVE: c_int = 0x00000010;
pub const SCTP_PARTIAL_DELIVERY_POINT: c_int = 0x00000011;
pub const SCTP_AUTH_CHUNK: c_int = 0x00000012;
pub const SCTP_AUTH_KEY: c_int = 0x00000013;
pub const SCTP_HMAC_IDENT: c_int = 0x00000014;
pub const SCTP_AUTH_ACTIVE_KEY: c_int = 0x00000015;
pub const SCTP_AUTH_DELETE_KEY: c_int = 0x00000016;
pub const SCTP_USE_EXT_RCVINFO: c_int = 0x00000017;
pub const SCTP_AUTO_ASCONF: c_int = 0x00000018;
pub const SCTP_MAXBURST: c_int = 0x00000019;
pub const SCTP_MAX_BURST: c_int = 0x00000019;
pub const SCTP_CONTEXT: c_int = 0x0000001a;
pub const SCTP_EXPLICIT_EOR: c_int = 0x00000001b;
pub const SCTP_REUSE_PORT: c_int = 0x00000001c;
pub const SCTP_AUTH_DEACTIVATE_KEY: c_int = 0x00000001d;
pub const SCTP_EVENT: c_int = 0x0000001e;
pub const SCTP_RECVRCVINFO: c_int = 0x0000001f;
pub const SCTP_RECVNXTINFO: c_int = 0x00000020;
pub const SCTP_DEFAULT_SNDINFO: c_int = 0x00000021;
pub const SCTP_DEFAULT_PRINFO: c_int = 0x00000022;
pub const SCTP_PEER_ADDR_THLDS: c_int = 0x00000023;
pub const SCTP_REMOTE_UDP_ENCAPS_PORT: c_int = 0x00000024;
pub const SCTP_ECN_SUPPORTED: c_int = 0x00000025;
pub const SCTP_AUTH_SUPPORTED: c_int = 0x00000027;
pub const SCTP_ASCONF_SUPPORTED: c_int = 0x00000028;
pub const SCTP_RECONFIG_SUPPORTED: c_int = 0x00000029;
pub const SCTP_NRSACK_SUPPORTED: c_int = 0x00000030;
pub const SCTP_PKTDROP_SUPPORTED: c_int = 0x00000031;
pub const SCTP_MAX_CWND: c_int = 0x00000032;

pub const SCTP_STATUS: c_int = 0x00000100;
pub const SCTP_GET_PEER_ADDR_INFO: c_int = 0x00000101;
pub const SCTP_PEER_AUTH_CHUNKS: c_int = 0x00000102;
pub const SCTP_LOCAL_AUTH_CHUNKS: c_int = 0x00000103;
pub const SCTP_GET_ASSOC_NUMBER: c_int = 0x00000104;
pub const SCTP_GET_ASSOC_ID_LIST: c_int = 0x00000105;
pub const SCTP_TIMEOUTS: c_int = 0x00000106;
pub const SCTP_PR_STREAM_STATUS: c_int = 0x00000107;
pub const SCTP_PR_ASSOC_STATUS: c_int = 0x00000108;

pub const SCTP_COMM_UP: c_int = 0x0001;
pub const SCTP_COMM_LOST: c_int = 0x0002;
pub const SCTP_RESTART: c_int = 0x0003;
pub const SCTP_SHUTDOWN_COMP: c_int = 0x0004;
pub const SCTP_CANT_STR_ASSOC: c_int = 0x0005;

pub const SCTP_ASSOC_SUPPORTS_PR: c_int = 0x01;
pub const SCTP_ASSOC_SUPPORTS_AUTH: c_int = 0x02;
pub const SCTP_ASSOC_SUPPORTS_ASCONF: c_int = 0x03;
pub const SCTP_ASSOC_SUPPORTS_MULTIBUF: c_int = 0x04;
pub const SCTP_ASSOC_SUPPORTS_RE_CONFIG: c_int = 0x05;
pub const SCTP_ASSOC_SUPPORTS_INTERLEAVING: c_int = 0x06;
pub const SCTP_ASSOC_SUPPORTS_MAX: c_int = 0x06;

pub const SCTP_ADDR_AVAILABLE: c_int = 0x0001;
pub const SCTP_ADDR_UNREACHABLE: c_int = 0x0002;
pub const SCTP_ADDR_REMOVED: c_int = 0x0003;
pub const SCTP_ADDR_ADDED: c_int = 0x0004;
pub const SCTP_ADDR_MADE_PRIM: c_int = 0x0005;
pub const SCTP_ADDR_CONFIRMED: c_int = 0x0006;

pub const SCTP_ACTIVE: c_int = 0x0001;
pub const SCTP_INACTIVE: c_int = 0x0002;
pub const SCTP_UNCONFIRMED: c_int = 0x0200;

pub const SCTP_DATA_UNSENT: c_int = 0x0001;
pub const SCTP_DATA_SENT: c_int = 0x0002;

pub const SCTP_PARTIAL_DELIVERY_ABORTED: c_int = 0x0001;

pub const SCTP_AUTH_NEW_KEY: c_int = 0x0001;
pub const SCTP_AUTH_NEWKEY: c_int = SCTP_AUTH_NEW_KEY;
pub const SCTP_AUTH_NO_AUTH: c_int = 0x0002;
pub const SCTP_AUTH_FREE_KEY: c_int = 0x0003;

pub const SCTP_STREAM_RESET_INCOMING_SSN: c_int = 0x0001;
pub const SCTP_STREAM_RESET_OUTGOING_SSN: c_int = 0x0002;
pub const SCTP_STREAM_RESET_DENIED: c_int = 0x0004;
pub const SCTP_STREAM_RESET_FAILED: c_int = 0x0008;

pub const SCTP_ASSOC_RESET_DENIED: c_int = 0x0004;
pub const SCTP_ASSOC_RESET_FAILED: c_int = 0x0008;

pub const SCTP_STREAM_CHANGE_DENIED: c_int = 0x0004;
pub const SCTP_STREAM_CHANGE_FAILED: c_int = 0x0008;

pub const KENV_DUMP_LOADER: c_int = 4;
pub const KENV_DUMP_STATIC: c_int = 5;

pub const RB_PAUSE: c_int = 0x100000;
pub const RB_REROOT: c_int = 0x200000;
pub const RB_POWERCYCLE: c_int = 0x400000;
pub const RB_PROBE: c_int = 0x10000000;
pub const RB_MULTIPLE: c_int = 0x20000000;

// netinet/in_pcb.h
pub const INC_ISIPV6: c_uchar = 0x01;
pub const INC_IPV6MINMTU: c_uchar = 0x02;

// sys/time.h
pub const CLOCK_BOOTTIME: crate::clockid_t = crate::CLOCK_UPTIME;
pub const CLOCK_REALTIME_COARSE: crate::clockid_t = crate::CLOCK_REALTIME_FAST;
pub const CLOCK_MONOTONIC_COARSE: crate::clockid_t = crate::CLOCK_MONOTONIC_FAST;

// sys/timerfd.h

pub const TFD_NONBLOCK: c_int = crate::O_NONBLOCK;
pub const TFD_CLOEXEC: c_int = O_CLOEXEC;
pub const TFD_TIMER_ABSTIME: c_int = 0x01;
pub const TFD_TIMER_CANCEL_ON_SET: c_int = 0x02;

// sys/unistd.h

pub const CLOSE_RANGE_CLOEXEC: c_uint = 1 << 2;

pub const KCMP_FILE: c_int = 100;
pub const KCMP_FILEOBJ: c_int = 101;
pub const KCMP_FILES: c_int = 102;
pub const KCMP_SIGHAND: c_int = 103;
pub const KCMP_VM: c_int = 104;

pub const fn MAP_ALIGNED(a: c_int) -> c_int {
    a << 24
}

const_fn! {
    {const} fn _ALIGN(p: usize) -> usize {
        (p + _ALIGNBYTES) & !_ALIGNBYTES
    }
}

f! {
    pub fn CMSG_DATA(cmsg: *const cmsghdr) -> *mut c_uchar {
        (cmsg as *mut c_uchar).add(_ALIGN(size_of::<cmsghdr>()))
    }

    pub {const} fn CMSG_LEN(length: c_uint) -> c_uint {
        _ALIGN(size_of::<cmsghdr>()) as c_uint + length
    }

    pub fn CMSG_NXTHDR(mhdr: *const crate::msghdr, cmsg: *const cmsghdr) -> *mut cmsghdr {
        if cmsg.is_null() {
            return crate::CMSG_FIRSTHDR(mhdr);
        }
        let next = cmsg as usize + _ALIGN((*cmsg).cmsg_len as usize) + _ALIGN(size_of::<cmsghdr>());
        let max = (*mhdr).msg_control as usize + (*mhdr).msg_controllen as usize;
        if next > max {
            core::ptr::null_mut::<cmsghdr>()
        } else {
            (cmsg as usize + _ALIGN((*cmsg).cmsg_len as usize)) as *mut cmsghdr
        }
    }

    pub {const} fn CMSG_SPACE(length: c_uint) -> c_uint {
        (_ALIGN(size_of::<cmsghdr>()) + _ALIGN(length as usize)) as c_uint
    }

    pub fn MALLOCX_ALIGN(lg: c_uint) -> c_int {
        ffsl(lg as c_long - 1)
    }

    pub {const} fn MALLOCX_TCACHE(tc: c_int) -> c_int {
        (tc + 2) << 8 as c_int
    }

    pub {const} fn MALLOCX_ARENA(a: c_int) -> c_int {
        (a + 1) << 20 as c_int
    }

    pub fn SOCKCREDSIZE(ngrps: usize) -> usize {
        let ngrps = if ngrps > 0 { ngrps - 1 } else { 0 };
        size_of::<sockcred>() + size_of::<crate::gid_t>() * ngrps
    }

    pub fn uname(buf: *mut crate::utsname) -> c_int {
        __xuname(256, buf as *mut c_void)
    }

    pub fn CPU_ZERO(cpuset: &mut cpuset_t) -> () {
        for slot in cpuset.__bits.iter_mut() {
            *slot = 0;
        }
    }

    pub fn CPU_FILL(cpuset: &mut cpuset_t) -> () {
        for slot in cpuset.__bits.iter_mut() {
            *slot = !0;
        }
    }

    pub fn CPU_SET(cpu: usize, cpuset: &mut cpuset_t) -> () {
        let bitset_bits = 8 * size_of::<c_long>();
        let (idx, offset) = (cpu / bitset_bits, cpu % bitset_bits);
        cpuset.__bits[idx] |= 1 << offset;
    }

    pub fn CPU_CLR(cpu: usize, cpuset: &mut cpuset_t) -> () {
        let bitset_bits = 8 * size_of::<c_long>();
        let (idx, offset) = (cpu / bitset_bits, cpu % bitset_bits);
        cpuset.__bits[idx] &= !(1 << offset);
    }

    pub fn CPU_ISSET(cpu: usize, cpuset: &cpuset_t) -> bool {
        let bitset_bits = 8 * size_of::<c_long>();
        let (idx, offset) = (cpu / bitset_bits, cpu % bitset_bits);
        0 != cpuset.__bits[idx] & (1 << offset)
    }

    pub fn CPU_COUNT(cpuset: &cpuset_t) -> c_int {
        let mut s: u32 = 0;
        let cpuset_size = size_of::<cpuset_t>();
        let bitset_size = size_of::<c_long>();

        for i in cpuset.__bits[..(cpuset_size / bitset_size)].iter() {
            s += i.count_ones();
        }
        s as c_int
    }

    pub fn SOCKCRED2SIZE(ngrps: usize) -> usize {
        let ngrps = if ngrps > 0 { ngrps - 1 } else { 0 };
        size_of::<sockcred2>() + size_of::<crate::gid_t>() * ngrps
    }

    pub fn PROT_MAX(x: c_int) -> c_int {
        x << 16
    }

    pub fn PROT_MAX_EXTRACT(x: c_int) -> c_int {
        (x >> 16) & (crate::PROT_READ | crate::PROT_WRITE | crate::PROT_EXEC)
    }
}

safe_f! {
    pub {const} fn WIFSIGNALED(status: c_int) -> bool {
        (status & 0o177) != 0o177 && (status & 0o177) != 0 && status != 0x13
    }

    pub {const} fn INVALID_SINFO_FLAG(x: c_int) -> bool {
        (x) & 0xfffffff0
            & !(SCTP_EOF
                | SCTP_ABORT
                | SCTP_UNORDERED
                | SCTP_ADDR_OVER
                | SCTP_SENDALL
                | SCTP_EOR
                | SCTP_SACK_IMMEDIATELY)
            != 0
    }

    pub {const} fn PR_SCTP_POLICY(x: c_int) -> c_int {
        x & 0x0f
    }

    pub {const} fn PR_SCTP_ENABLED(x: c_int) -> bool {
        PR_SCTP_POLICY(x) != SCTP_PR_SCTP_NONE && PR_SCTP_POLICY(x) != SCTP_PR_SCTP_ALL
    }

    pub {const} fn PR_SCTP_TTL_ENABLED(x: c_int) -> bool {
        PR_SCTP_POLICY(x) == SCTP_PR_SCTP_TTL
    }

    pub {const} fn PR_SCTP_BUF_ENABLED(x: c_int) -> bool {
        PR_SCTP_POLICY(x) == SCTP_PR_SCTP_BUF
    }

    pub {const} fn PR_SCTP_RTX_ENABLED(x: c_int) -> bool {
        PR_SCTP_POLICY(x) == SCTP_PR_SCTP_RTX
    }

    pub {const} fn PR_SCTP_INVALID_POLICY(x: c_int) -> bool {
        PR_SCTP_POLICY(x) > SCTP_PR_SCTP_MAX
    }

    pub {const} fn PR_SCTP_VALID_POLICY(x: c_int) -> bool {
        PR_SCTP_POLICY(x) <= SCTP_PR_SCTP_MAX
    }
}

cfg_if! {
    if #[cfg(not(any(freebsd10, freebsd11)))] {
        extern "C" {
            pub fn fhlink(fhp: *mut fhandle_t, to: *const c_char) -> c_int;
            pub fn fhlinkat(fhp: *mut fhandle_t, tofd: c_int, to: *const c_char) -> c_int;
            pub fn fhreadlink(fhp: *mut fhandle_t, buf: *mut c_char, bufsize: size_t) -> c_int;
            pub fn getfhat(fd: c_int, path: *mut c_char, fhp: *mut fhandle, flag: c_int) -> c_int;
        }
    }
}

extern "C" {
    #[cfg_attr(doc, doc(alias = "__errno_location"))]
    #[cfg_attr(doc, doc(alias = "errno"))]
    pub fn __error() -> *mut c_int;

    pub fn aio_cancel(fd: c_int, aiocbp: *mut aiocb) -> c_int;
    pub fn aio_error(aiocbp: *const aiocb) -> c_int;
    pub fn aio_fsync(op: c_int, aiocbp: *mut aiocb) -> c_int;
    pub fn aio_read(aiocbp: *mut aiocb) -> c_int;
    pub fn aio_readv(aiocbp: *mut crate::aiocb) -> c_int;
    pub fn aio_return(aiocbp: *mut aiocb) -> ssize_t;
    pub fn aio_suspend(
        aiocb_list: *const *const aiocb,
        nitems: c_int,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn aio_write(aiocbp: *mut aiocb) -> c_int;
    pub fn aio_writev(aiocbp: *mut crate::aiocb) -> c_int;

    pub fn copy_file_range(
        infd: c_int,
        inoffp: *mut off_t,
        outfd: c_int,
        outoffp: *mut off_t,
        len: size_t,
        flags: c_uint,
    ) -> ssize_t;

    pub fn devname_r(
        dev: crate::dev_t,
        mode: crate::mode_t,
        buf: *mut c_char,
        len: c_int,
    ) -> *mut c_char;

    pub fn extattr_delete_fd(fd: c_int, attrnamespace: c_int, attrname: *const c_char) -> c_int;
    pub fn extattr_delete_file(
        path: *const c_char,
        attrnamespace: c_int,
        attrname: *const c_char,
    ) -> c_int;
    pub fn extattr_delete_link(
        path: *const c_char,
        attrnamespace: c_int,
        attrname: *const c_char,
    ) -> c_int;
    pub fn extattr_get_fd(
        fd: c_int,
        attrnamespace: c_int,
        attrname: *const c_char,
        data: *mut c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_get_file(
        path: *const c_char,
        attrnamespace: c_int,
        attrname: *const c_char,
        data: *mut c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_get_link(
        path: *const c_char,
        attrnamespace: c_int,
        attrname: *const c_char,
        data: *mut c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_list_fd(
        fd: c_int,
        attrnamespace: c_int,
        data: *mut c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_list_file(
        path: *const c_char,
        attrnamespace: c_int,
        data: *mut c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_list_link(
        path: *const c_char,
        attrnamespace: c_int,
        data: *mut c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_set_fd(
        fd: c_int,
        attrnamespace: c_int,
        attrname: *const c_char,
        data: *const c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_set_file(
        path: *const c_char,
        attrnamespace: c_int,
        attrname: *const c_char,
        data: *const c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_set_link(
        path: *const c_char,
        attrnamespace: c_int,
        attrname: *const c_char,
        data: *const c_void,
        nbytes: size_t,
    ) -> ssize_t;

    pub fn fspacectl(
        fd: c_int,
        cmd: c_int,
        rqsr: *const spacectl_range,
        flags: c_int,
        rmsr: *mut spacectl_range,
    ) -> c_int;

    pub fn jail(jail: *mut crate::jail) -> c_int;
    pub fn jail_attach(jid: c_int) -> c_int;
    pub fn jail_remove(jid: c_int) -> c_int;
    pub fn jail_get(iov: *mut crate::iovec, niov: c_uint, flags: c_int) -> c_int;
    pub fn jail_set(iov: *mut crate::iovec, niov: c_uint, flags: c_int) -> c_int;

    pub fn lio_listio(
        mode: c_int,
        aiocb_list: *const *mut aiocb,
        nitems: c_int,
        sevp: *mut sigevent,
    ) -> c_int;

    pub fn getutxuser(user: *const c_char) -> *mut utmpx;
    pub fn setutxdb(_type: c_int, file: *const c_char) -> c_int;

    pub fn aio_waitcomplete(iocbp: *mut *mut aiocb, timeout: *mut crate::timespec) -> ssize_t;
    pub fn mq_getfd_np(mqd: crate::mqd_t) -> c_int;

    pub fn waitid(
        idtype: idtype_t,
        id: crate::id_t,
        infop: *mut crate::siginfo_t,
        options: c_int,
    ) -> c_int;
    pub fn ptsname_r(fd: c_int, buf: *mut c_char, buflen: size_t) -> c_int;

    pub fn ftok(pathname: *const c_char, proj_id: c_int) -> crate::key_t;
    pub fn shmget(key: crate::key_t, size: size_t, shmflg: c_int) -> c_int;
    pub fn shmat(shmid: c_int, shmaddr: *const c_void, shmflg: c_int) -> *mut c_void;
    pub fn shmdt(shmaddr: *const c_void) -> c_int;
    pub fn shmctl(shmid: c_int, cmd: c_int, buf: *mut crate::shmid_ds) -> c_int;
    pub fn semget(key: crate::key_t, nsems: c_int, semflg: c_int) -> c_int;
    pub fn semctl(semid: c_int, semnum: c_int, cmd: c_int, ...) -> c_int;
    pub fn semop(semid: c_int, sops: *mut sembuf, nsops: size_t) -> c_int;
    pub fn msgctl(msqid: c_int, cmd: c_int, buf: *mut crate::msqid_ds) -> c_int;
    pub fn msgget(key: crate::key_t, msgflg: c_int) -> c_int;
    pub fn msgsnd(msqid: c_int, msgp: *const c_void, msgsz: size_t, msgflg: c_int) -> c_int;
    pub fn cfmakesane(termios: *mut crate::termios);

    pub fn pdfork(fdp: *mut c_int, flags: c_int) -> crate::pid_t;
    pub fn pdgetpid(fd: c_int, pidp: *mut crate::pid_t) -> c_int;
    pub fn pdkill(fd: c_int, signum: c_int) -> c_int;

    pub fn rtprio_thread(function: c_int, lwpid: crate::lwpid_t, rtp: *mut super::rtprio) -> c_int;

    pub fn uuidgen(store: *mut uuid, count: c_int) -> c_int;

    pub fn thr_kill(id: c_long, sig: c_int) -> c_int;
    pub fn thr_kill2(pid: crate::pid_t, id: c_long, sig: c_int) -> c_int;
    pub fn thr_self(tid: *mut c_long) -> c_int;
    pub fn pthread_getthreadid_np() -> c_int;
    pub fn pthread_getaffinity_np(
        td: crate::pthread_t,
        cpusetsize: size_t,
        cpusetp: *mut cpuset_t,
    ) -> c_int;
    pub fn pthread_setaffinity_np(
        td: crate::pthread_t,
        cpusetsize: size_t,
        cpusetp: *const cpuset_t,
    ) -> c_int;

    // sched.h linux compatibility api
    pub fn sched_getaffinity(
        pid: crate::pid_t,
        cpusetsz: size_t,
        cpuset: *mut crate::cpuset_t,
    ) -> c_int;
    pub fn sched_setaffinity(
        pid: crate::pid_t,
        cpusetsz: size_t,
        cpuset: *const crate::cpuset_t,
    ) -> c_int;
    pub fn sched_getcpu() -> c_int;

    pub fn pthread_mutex_consistent(mutex: *mut crate::pthread_mutex_t) -> c_int;

    pub fn pthread_mutexattr_getrobust(
        attr: *mut crate::pthread_mutexattr_t,
        robust: *mut c_int,
    ) -> c_int;
    pub fn pthread_mutexattr_setrobust(
        attr: *mut crate::pthread_mutexattr_t,
        robust: c_int,
    ) -> c_int;

    pub fn pthread_spin_init(lock: *mut pthread_spinlock_t, pshared: c_int) -> c_int;
    pub fn pthread_spin_destroy(lock: *mut pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_lock(lock: *mut pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_trylock(lock: *mut pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_unlock(lock: *mut pthread_spinlock_t) -> c_int;

    #[cfg_attr(all(target_os = "freebsd", freebsd11), link_name = "statfs@FBSD_1.0")]
    pub fn statfs(path: *const c_char, buf: *mut statfs) -> c_int;
    #[cfg_attr(all(target_os = "freebsd", freebsd11), link_name = "fstatfs@FBSD_1.0")]
    pub fn fstatfs(fd: c_int, buf: *mut statfs) -> c_int;

    pub fn dup3(src: c_int, dst: c_int, flags: c_int) -> c_int;
    pub fn __xuname(nmln: c_int, buf: *mut c_void) -> c_int;

    pub fn sendmmsg(
        sockfd: c_int,
        msgvec: *mut crate::mmsghdr,
        vlen: size_t,
        flags: c_int,
    ) -> ssize_t;
    pub fn recvmmsg(
        sockfd: c_int,
        msgvec: *mut crate::mmsghdr,
        vlen: size_t,
        flags: c_int,
        timeout: *const crate::timespec,
    ) -> ssize_t;
    pub fn memmem(
        haystack: *const c_void,
        haystacklen: size_t,
        needle: *const c_void,
        needlelen: size_t,
    ) -> *mut c_void;

    pub fn fhopen(fhp: *const fhandle_t, flags: c_int) -> c_int;
    pub fn fhstat(fhp: *const fhandle, buf: *mut crate::stat) -> c_int;
    pub fn fhstatfs(fhp: *const fhandle_t, buf: *mut crate::statfs) -> c_int;
    pub fn getfh(path: *const c_char, fhp: *mut fhandle_t) -> c_int;
    pub fn lgetfh(path: *const c_char, fhp: *mut fhandle_t) -> c_int;
    pub fn getfsstat(buf: *mut crate::statfs, bufsize: c_long, mode: c_int) -> c_int;
    #[cfg_attr(
        all(target_os = "freebsd", freebsd11),
        link_name = "getmntinfo@FBSD_1.0"
    )]
    pub fn getmntinfo(mntbufp: *mut *mut crate::statfs, mode: c_int) -> c_int;
    pub fn mount(
        type_: *const c_char,
        dir: *const c_char,
        flags: c_int,
        data: *mut c_void,
    ) -> c_int;
    pub fn nmount(iov: *mut crate::iovec, niov: c_uint, flags: c_int) -> c_int;

    pub fn setproctitle(fmt: *const c_char, ...);
    pub fn rfork(flags: c_int) -> c_int;
    pub fn cpuset_getaffinity(
        level: cpulevel_t,
        which: cpuwhich_t,
        id: crate::id_t,
        setsize: size_t,
        mask: *mut cpuset_t,
    ) -> c_int;
    pub fn cpuset_setaffinity(
        level: cpulevel_t,
        which: cpuwhich_t,
        id: crate::id_t,
        setsize: size_t,
        mask: *const cpuset_t,
    ) -> c_int;
    pub fn cpuset(setid: *mut crate::cpusetid_t) -> c_int;
    pub fn cpuset_getid(
        level: cpulevel_t,
        which: cpuwhich_t,
        id: crate::id_t,
        setid: *mut crate::cpusetid_t,
    ) -> c_int;
    pub fn cpuset_setid(which: cpuwhich_t, id: crate::id_t, setid: crate::cpusetid_t) -> c_int;
    pub fn cap_enter() -> c_int;
    pub fn cap_getmode(modep: *mut c_uint) -> c_int;
    pub fn cap_fcntls_get(fd: c_int, fcntlrightsp: *mut u32) -> c_int;
    pub fn cap_fcntls_limit(fd: c_int, fcntlrights: u32) -> c_int;
    pub fn cap_ioctls_get(fd: c_int, cmds: *mut u_long, maxcmds: usize) -> isize;
    pub fn cap_ioctls_limit(fd: c_int, cmds: *const u_long, ncmds: usize) -> c_int;
    pub fn __cap_rights_init(version: c_int, rights: *mut cap_rights_t, ...) -> *mut cap_rights_t;
    pub fn __cap_rights_get(version: c_int, fd: c_int, rightsp: *mut cap_rights_t) -> c_int;
    pub fn __cap_rights_set(rights: *mut cap_rights_t, ...) -> *mut cap_rights_t;
    pub fn __cap_rights_clear(rights: *mut cap_rights_t, ...) -> *mut cap_rights_t;
    pub fn __cap_rights_is_set(rights: *const cap_rights_t, ...) -> bool;
    pub fn cap_rights_is_valid(rights: *const cap_rights_t) -> bool;
    pub fn cap_rights_limit(fd: c_int, rights: *const cap_rights_t) -> c_int;
    pub fn cap_rights_merge(dst: *mut cap_rights_t, src: *const cap_rights_t) -> *mut cap_rights_t;
    pub fn cap_rights_remove(dst: *mut cap_rights_t, src: *const cap_rights_t)
        -> *mut cap_rights_t;
    pub fn cap_rights_contains(big: *const cap_rights_t, little: *const cap_rights_t) -> bool;
    pub fn cap_sandboxed() -> bool;

    pub fn reallocarray(ptr: *mut c_void, nmemb: size_t, size: size_t) -> *mut c_void;

    pub fn ffs(value: c_int) -> c_int;
    pub fn ffsl(value: c_long) -> c_int;
    pub fn ffsll(value: c_longlong) -> c_int;
    pub fn fls(value: c_int) -> c_int;
    pub fn flsl(value: c_long) -> c_int;
    pub fn flsll(value: c_longlong) -> c_int;
    pub fn malloc_stats_print(
        write_cb: unsafe extern "C" fn(*mut c_void, *const c_char),
        cbopaque: *mut c_void,
        opt: *const c_char,
    );
    pub fn mallctl(
        name: *const c_char,
        oldp: *mut c_void,
        oldlenp: *mut size_t,
        newp: *mut c_void,
        newlen: size_t,
    ) -> c_int;
    pub fn mallctlnametomib(name: *const c_char, mibp: *mut size_t, miplen: *mut size_t) -> c_int;
    pub fn mallctlbymib(
        mib: *const size_t,
        mible: size_t,
        oldp: *mut c_void,
        oldlenp: *mut size_t,
        newp: *mut c_void,
        newlen: size_t,
    ) -> c_int;
    pub fn mallocx(size: size_t, flags: c_int) -> *mut c_void;
    pub fn rallocx(ptr: *mut c_void, size: size_t, flags: c_int) -> *mut c_void;
    pub fn xallocx(ptr: *mut c_void, size: size_t, extra: size_t, flags: c_int) -> size_t;
    pub fn sallocx(ptr: *const c_void, flags: c_int) -> size_t;
    pub fn dallocx(ptr: *mut c_void, flags: c_int);
    pub fn sdallocx(ptr: *mut c_void, size: size_t, flags: c_int);
    pub fn nallocx(size: size_t, flags: c_int) -> size_t;

    pub fn procctl(
        idtype: crate::idtype_t,
        id: crate::id_t,
        cmd: c_int,
        data: *mut c_void,
    ) -> c_int;

    pub fn getpagesize() -> c_int;
    pub fn getpagesizes(pagesize: *mut size_t, nelem: c_int) -> c_int;

    pub fn clock_getcpuclockid2(arg1: crate::id_t, arg2: c_int, arg3: *mut clockid_t) -> c_int;
    pub fn strchrnul(s: *const c_char, c: c_int) -> *mut c_char;

    pub fn shm_create_largepage(
        path: *const c_char,
        flags: c_int,
        psind: c_int,
        alloc_policy: c_int,
        mode: crate::mode_t,
    ) -> c_int;
    pub fn shm_rename(path_from: *const c_char, path_to: *const c_char, flags: c_int) -> c_int;
    pub fn memfd_create(name: *const c_char, flags: c_uint) -> c_int;
    pub fn setaudit(auditinfo: *const auditinfo_t) -> c_int;

    pub fn eventfd(init: c_uint, flags: c_int) -> c_int;
    pub fn eventfd_read(fd: c_int, value: *mut eventfd_t) -> c_int;
    pub fn eventfd_write(fd: c_int, value: eventfd_t) -> c_int;

    pub fn fdatasync(fd: c_int) -> c_int;

    pub fn elf_aux_info(aux: c_int, buf: *mut c_void, buflen: c_int) -> c_int;
    pub fn setproctitle_fast(fmt: *const c_char, ...);
    pub fn timingsafe_bcmp(a: *const c_void, b: *const c_void, len: size_t) -> c_int;
    pub fn timingsafe_memcmp(a: *const c_void, b: *const c_void, len: size_t) -> c_int;

    pub fn _umtx_op(
        obj: *mut c_void,
        op: c_int,
        val: c_ulong,
        uaddr: *mut c_void,
        uaddr2: *mut c_void,
    ) -> c_int;

    pub fn sctp_peeloff(s: c_int, id: crate::sctp_assoc_t) -> c_int;
    pub fn sctp_bindx(s: c_int, addrs: *mut crate::sockaddr, num: c_int, tpe: c_int) -> c_int;
    pub fn sctp_connectx(
        s: c_int,
        addrs: *const crate::sockaddr,
        addrcnt: c_int,
        id: *mut crate::sctp_assoc_t,
    ) -> c_int;
    pub fn sctp_getaddrlen(family: crate::sa_family_t) -> c_int;
    pub fn sctp_getpaddrs(
        s: c_int,
        asocid: crate::sctp_assoc_t,
        addrs: *mut *mut crate::sockaddr,
    ) -> c_int;
    pub fn sctp_freepaddrs(addrs: *mut crate::sockaddr);
    pub fn sctp_getladdrs(
        s: c_int,
        asocid: crate::sctp_assoc_t,
        addrs: *mut *mut crate::sockaddr,
    ) -> c_int;
    pub fn sctp_freeladdrs(addrs: *mut crate::sockaddr);
    pub fn sctp_opt_info(
        s: c_int,
        id: crate::sctp_assoc_t,
        opt: c_int,
        arg: *mut c_void,
        size: *mut crate::socklen_t,
    ) -> c_int;
    pub fn sctp_sendv(
        sd: c_int,
        iov: *const crate::iovec,
        iovcnt: c_int,
        addrs: *mut crate::sockaddr,
        addrcnt: c_int,
        info: *mut c_void,
        infolen: crate::socklen_t,
        infotype: c_uint,
        flags: c_int,
    ) -> ssize_t;
    pub fn sctp_recvv(
        sd: c_int,
        iov: *const crate::iovec,
        iovcnt: c_int,
        from: *mut crate::sockaddr,
        fromlen: *mut crate::socklen_t,
        info: *mut c_void,
        infolen: *mut crate::socklen_t,
        infotype: *mut c_uint,
        flags: *mut c_int,
    ) -> ssize_t;

    pub fn timerfd_create(clockid: c_int, flags: c_int) -> c_int;
    pub fn timerfd_gettime(fd: c_int, curr_value: *mut itimerspec) -> c_int;
    pub fn timerfd_settime(
        fd: c_int,
        flags: c_int,
        new_value: *const itimerspec,
        old_value: *mut itimerspec,
    ) -> c_int;
    pub fn closefrom(lowfd: c_int);
    pub fn close_range(lowfd: c_uint, highfd: c_uint, flags: c_int) -> c_int;

    pub fn execvpe(
        file: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int;

    pub fn kcmp(
        pid1: crate::pid_t,
        pid2: crate::pid_t,
        type_: c_int,
        idx1: c_ulong,
        idx2: c_ulong,
    ) -> c_int;
}

#[link(name = "memstat")]
extern "C" {
    pub fn memstat_strerror(error: c_int) -> *const c_char;
    pub fn memstat_mtl_alloc() -> *mut memory_type_list;
    pub fn memstat_mtl_first(list: *mut memory_type_list) -> *mut memory_type;
    pub fn memstat_mtl_next(mtp: *mut memory_type) -> *mut memory_type;
    pub fn memstat_mtl_find(
        list: *mut memory_type_list,
        allocator: c_int,
        name: *const c_char,
    ) -> *mut memory_type;
    pub fn memstat_mtl_free(list: *mut memory_type_list);
    pub fn memstat_mtl_geterror(list: *mut memory_type_list) -> c_int;
    pub fn memstat_get_name(mtp: *const memory_type) -> *const c_char;
}

#[link(name = "kvm")]
extern "C" {
    pub fn kvm_dpcpu_setcpu(kd: *mut crate::kvm_t, cpu: c_uint) -> c_int;
    pub fn kvm_getargv(
        kd: *mut crate::kvm_t,
        p: *const kinfo_proc,
        nchr: c_int,
    ) -> *mut *mut c_char;
    pub fn kvm_getcptime(kd: *mut crate::kvm_t, cp_time: *mut c_long) -> c_int;
    pub fn kvm_getenvv(
        kd: *mut crate::kvm_t,
        p: *const kinfo_proc,
        nchr: c_int,
    ) -> *mut *mut c_char;
    pub fn kvm_geterr(kd: *mut crate::kvm_t) -> *mut c_char;
    pub fn kvm_getmaxcpu(kd: *mut crate::kvm_t) -> c_int;
    pub fn kvm_getncpus(kd: *mut crate::kvm_t) -> c_int;
    pub fn kvm_getpcpu(kd: *mut crate::kvm_t, cpu: c_int) -> *mut c_void;
    pub fn kvm_counter_u64_fetch(kd: *mut crate::kvm_t, base: c_ulong) -> u64;
    pub fn kvm_getswapinfo(
        kd: *mut crate::kvm_t,
        info: *mut kvm_swap,
        maxswap: c_int,
        flags: c_int,
    ) -> c_int;
    pub fn kvm_native(kd: *mut crate::kvm_t) -> c_int;
    pub fn kvm_nlist(kd: *mut crate::kvm_t, nl: *mut nlist) -> c_int;
    pub fn kvm_nlist2(kd: *mut crate::kvm_t, nl: *mut kvm_nlist) -> c_int;
    pub fn kvm_read_zpcpu(
        kd: *mut crate::kvm_t,
        base: c_ulong,
        buf: *mut c_void,
        size: size_t,
        cpu: c_int,
    ) -> ssize_t;
    pub fn kvm_read2(
        kd: *mut crate::kvm_t,
        addr: kvaddr_t,
        buf: *mut c_void,
        nbytes: size_t,
    ) -> ssize_t;
}

#[link(name = "util")]
extern "C" {
    pub fn extattr_namespace_to_string(attrnamespace: c_int, string: *mut *mut c_char) -> c_int;
    pub fn extattr_string_to_namespace(string: *const c_char, attrnamespace: *mut c_int) -> c_int;
    pub fn realhostname(host: *mut c_char, hsize: size_t, ip: *const crate::in_addr) -> c_int;
    pub fn realhostname_sa(
        host: *mut c_char,
        hsize: size_t,
        addr: *mut crate::sockaddr,
        addrlen: c_int,
    ) -> c_int;

    pub fn kld_isloaded(name: *const c_char) -> c_int;
    pub fn kld_load(name: *const c_char) -> c_int;

    pub fn kinfo_getvmmap(pid: crate::pid_t, cntp: *mut c_int) -> *mut kinfo_vmentry;

    pub fn hexdump(ptr: *const c_void, length: c_int, hdr: *const c_char, flags: c_int);
    pub fn humanize_number(
        buf: *mut c_char,
        len: size_t,
        number: i64,
        suffix: *const c_char,
        scale: c_int,
        flags: c_int,
    ) -> c_int;

    pub fn flopen(path: *const c_char, flags: c_int, ...) -> c_int;
    pub fn flopenat(fd: c_int, path: *const c_char, flags: c_int, ...) -> c_int;

    pub fn getlocalbase() -> *const c_char;

    pub fn pidfile_open(
        path: *const c_char,
        mode: crate::mode_t,
        pidptr: *mut crate::pid_t,
    ) -> *mut crate::pidfh;
    pub fn pidfile_write(path: *mut crate::pidfh) -> c_int;
    pub fn pidfile_close(path: *mut crate::pidfh) -> c_int;
    pub fn pidfile_remove(path: *mut crate::pidfh) -> c_int;
    pub fn pidfile_fileno(path: *const crate::pidfh) -> c_int;
    // FIXME(freebsd): pidfile_signal in due time (both manpage present and updated image snapshot)
}

#[link(name = "procstat")]
extern "C" {
    pub fn procstat_open_sysctl() -> *mut procstat;
    pub fn procstat_getfiles(
        procstat: *mut procstat,
        kp: *mut kinfo_proc,
        mmapped: c_int,
    ) -> *mut filestat_list;
    pub fn procstat_freefiles(procstat: *mut procstat, head: *mut filestat_list);
    pub fn procstat_getprocs(
        procstat: *mut procstat,
        what: c_int,
        arg: c_int,
        count: *mut c_uint,
    ) -> *mut kinfo_proc;
    pub fn procstat_freeprocs(procstat: *mut procstat, p: *mut kinfo_proc);
    pub fn procstat_getvmmap(
        procstat: *mut procstat,
        kp: *mut kinfo_proc,
        count: *mut c_uint,
    ) -> *mut kinfo_vmentry;
    pub fn procstat_freevmmap(procstat: *mut procstat, vmmap: *mut kinfo_vmentry);
    pub fn procstat_close(procstat: *mut procstat);
    pub fn procstat_freeargv(procstat: *mut procstat);
    pub fn procstat_freeenvv(procstat: *mut procstat);
    pub fn procstat_freegroups(procstat: *mut procstat, groups: *mut crate::gid_t);
    pub fn procstat_freeptlwpinfo(procstat: *mut procstat, pl: *mut ptrace_lwpinfo);
    pub fn procstat_getargv(
        procstat: *mut procstat,
        kp: *mut kinfo_proc,
        nchr: size_t,
    ) -> *mut *mut c_char;
    pub fn procstat_getenvv(
        procstat: *mut procstat,
        kp: *mut kinfo_proc,
        nchr: size_t,
    ) -> *mut *mut c_char;
    pub fn procstat_getgroups(
        procstat: *mut procstat,
        kp: *mut kinfo_proc,
        count: *mut c_uint,
    ) -> *mut crate::gid_t;
    pub fn procstat_getosrel(
        procstat: *mut procstat,
        kp: *mut kinfo_proc,
        osrelp: *mut c_int,
    ) -> c_int;
    pub fn procstat_getpathname(
        procstat: *mut procstat,
        kp: *mut kinfo_proc,
        pathname: *mut c_char,
        maxlen: size_t,
    ) -> c_int;
    pub fn procstat_getrlimit(
        procstat: *mut procstat,
        kp: *mut kinfo_proc,
        which: c_int,
        rlimit: *mut crate::rlimit,
    ) -> c_int;
    pub fn procstat_getumask(
        procstat: *mut procstat,
        kp: *mut kinfo_proc,
        maskp: *mut c_ushort,
    ) -> c_int;
    pub fn procstat_open_core(filename: *const c_char) -> *mut procstat;
    pub fn procstat_open_kvm(nlistf: *const c_char, memf: *const c_char) -> *mut procstat;
    pub fn procstat_get_socket_info(
        proc_: *mut procstat,
        fst: *mut filestat,
        sock: *mut sockstat,
        errbuf: *mut c_char,
    ) -> c_int;
    pub fn procstat_get_vnode_info(
        proc_: *mut procstat,
        fst: *mut filestat,
        vn: *mut vnstat,
        errbuf: *mut c_char,
    ) -> c_int;
    pub fn procstat_get_pts_info(
        proc_: *mut procstat,
        fst: *mut filestat,
        pts: *mut ptsstat,
        errbuf: *mut c_char,
    ) -> c_int;
    pub fn procstat_get_shm_info(
        proc_: *mut procstat,
        fst: *mut filestat,
        shm: *mut shmstat,
        errbuf: *mut c_char,
    ) -> c_int;
}

#[link(name = "rt")]
extern "C" {
    pub fn timer_create(clock_id: clockid_t, evp: *mut sigevent, timerid: *mut timer_t) -> c_int;
    pub fn timer_delete(timerid: timer_t) -> c_int;
    pub fn timer_getoverrun(timerid: timer_t) -> c_int;
    pub fn timer_gettime(timerid: timer_t, value: *mut itimerspec) -> c_int;
    pub fn timer_settime(
        timerid: timer_t,
        flags: c_int,
        value: *const itimerspec,
        ovalue: *mut itimerspec,
    ) -> c_int;
}

#[link(name = "devstat")]
extern "C" {
    pub fn devstat_getnumdevs(kd: *mut crate::kvm_t) -> c_int;
    pub fn devstat_getgeneration(kd: *mut crate::kvm_t) -> c_long;
    pub fn devstat_getversion(kd: *mut crate::kvm_t) -> c_int;
    pub fn devstat_checkversion(kd: *mut crate::kvm_t) -> c_int;
    pub fn devstat_selectdevs(
        dev_select: *mut *mut device_selection,
        num_selected: *mut c_int,
        num_selections: *mut c_int,
        select_generation: *mut c_long,
        current_generation: c_long,
        devices: *mut devstat,
        numdevs: c_int,
        matches: *mut devstat_match,
        num_matches: c_int,
        dev_selections: *mut *mut c_char,
        num_dev_selections: c_int,
        select_mode: devstat_select_mode,
        maxshowdevs: c_int,
        perf_select: c_int,
    ) -> c_int;
    pub fn devstat_buildmatch(
        match_str: *mut c_char,
        matches: *mut *mut devstat_match,
        num_matches: *mut c_int,
    ) -> c_int;
}

cfg_if! {
    if #[cfg(freebsd15)] {
        mod freebsd15;
        pub use self::freebsd15::*;
    } else if #[cfg(freebsd14)] {
        mod freebsd14;
        pub use self::freebsd14::*;
    } else if #[cfg(freebsd13)] {
        mod freebsd13;
        pub use self::freebsd13::*;
    } else if #[cfg(freebsd12)] {
        mod freebsd12;
        pub use self::freebsd12::*;
    } else if #[cfg(any(freebsd10, freebsd11))] {
        mod freebsd11;
        pub use self::freebsd11::*;
    } else {
        // Unknown freebsd version
    }
}

cfg_if! {
    if #[cfg(target_arch = "x86")] {
        mod x86;
        pub use self::x86::*;
    } else if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(target_arch = "arm")] {
        mod arm;
        pub use self::arm::*;
    } else if #[cfg(target_arch = "powerpc64")] {
        mod powerpc64;
        pub use self::powerpc64::*;
    } else if #[cfg(target_arch = "powerpc")] {
        mod powerpc;
        pub use self::powerpc::*;
    } else if #[cfg(target_arch = "riscv64")] {
        mod riscv64;
        pub use self::riscv64::*;
    } else {
        // Unknown target_arch
    }
}
