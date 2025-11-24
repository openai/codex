use crate::off64_t;
use crate::prelude::*;

pub type pthread_t = c_ulong;
pub type __priority_which_t = c_uint;
pub type __rlimit_resource_t = c_uint;
pub type Lmid_t = c_long;
pub type regoff_t = c_int;
pub type __kernel_rwf_t = c_int;

cfg_if! {
    if #[cfg(doc)] {
        // Used in `linux::arch` to define ioctl constants.
        pub(crate) type Ioctl = c_ulong;
    } else {
        #[doc(hidden)]
        pub type Ioctl = c_ulong;
    }
}

s! {
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
        #[cfg(all(
            not(gnu_file_offset_bits64),
            not(target_arch = "x86_64"),
            target_pointer_width = "32"
        ))]
        __unused1: [c_char; 4],
        __glibc_reserved: [c_char; 32],
    }

    pub struct __exit_status {
        pub e_termination: c_short,
        pub e_exit: c_short,
    }

    pub struct __timeval {
        pub tv_sec: i32,
        pub tv_usec: i32,
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

    pub struct msghdr {
        pub msg_name: *mut c_void,
        pub msg_namelen: crate::socklen_t,
        pub msg_iov: *mut crate::iovec,
        pub msg_iovlen: size_t,
        pub msg_control: *mut c_void,
        pub msg_controllen: size_t,
        pub msg_flags: c_int,
    }

    pub struct cmsghdr {
        pub cmsg_len: size_t,
        pub cmsg_level: c_int,
        pub cmsg_type: c_int,
    }

    pub struct termios {
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_line: crate::cc_t,
        pub c_cc: [crate::cc_t; crate::NCCS],
        #[cfg(not(any(
            target_arch = "sparc",
            target_arch = "sparc64",
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )))]
        pub c_ispeed: crate::speed_t,
        #[cfg(not(any(
            target_arch = "sparc",
            target_arch = "sparc64",
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )))]
        pub c_ospeed: crate::speed_t,
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

    pub struct nl_pktinfo {
        pub group: u32,
    }

    pub struct nl_mmap_req {
        pub nm_block_size: c_uint,
        pub nm_block_nr: c_uint,
        pub nm_frame_size: c_uint,
        pub nm_frame_nr: c_uint,
    }

    pub struct nl_mmap_hdr {
        pub nm_status: c_uint,
        pub nm_len: c_uint,
        pub nm_group: u32,
        pub nm_pid: u32,
        pub nm_uid: u32,
        pub nm_gid: u32,
    }

    pub struct rtentry {
        pub rt_pad1: c_ulong,
        pub rt_dst: crate::sockaddr,
        pub rt_gateway: crate::sockaddr,
        pub rt_genmask: crate::sockaddr,
        pub rt_flags: c_ushort,
        pub rt_pad2: c_short,
        pub rt_pad3: c_ulong,
        pub rt_tos: c_uchar,
        pub rt_class: c_uchar,
        #[cfg(target_pointer_width = "64")]
        pub rt_pad4: [c_short; 3usize],
        #[cfg(not(target_pointer_width = "64"))]
        pub rt_pad4: c_short,
        pub rt_metric: c_short,
        pub rt_dev: *mut c_char,
        pub rt_mtu: c_ulong,
        pub rt_window: c_ulong,
        pub rt_irtt: c_ushort,
    }

    pub struct ntptimeval {
        pub time: crate::timeval,
        pub maxerror: c_long,
        pub esterror: c_long,
        pub tai: c_long,
        pub __glibc_reserved1: c_long,
        pub __glibc_reserved2: c_long,
        pub __glibc_reserved3: c_long,
        pub __glibc_reserved4: c_long,
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

    pub struct Elf64_Chdr {
        pub ch_type: crate::Elf64_Word,
        pub ch_reserved: crate::Elf64_Word,
        pub ch_size: crate::Elf64_Xword,
        pub ch_addralign: crate::Elf64_Xword,
    }

    pub struct Elf32_Chdr {
        pub ch_type: crate::Elf32_Word,
        pub ch_size: crate::Elf32_Word,
        pub ch_addralign: crate::Elf32_Word,
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

    pub struct ptrace_peeksiginfo_args {
        pub off: crate::__u64,
        pub flags: crate::__u32,
        pub nr: crate::__s32,
    }

    pub struct __c_anonymous_ptrace_syscall_info_entry {
        pub nr: crate::__u64,
        pub args: [crate::__u64; 6],
    }

    pub struct __c_anonymous_ptrace_syscall_info_exit {
        pub sval: crate::__s64,
        pub is_error: crate::__u8,
    }

    pub struct __c_anonymous_ptrace_syscall_info_seccomp {
        pub nr: crate::__u64,
        pub args: [crate::__u64; 6],
        pub ret_data: crate::__u32,
    }

    pub struct ptrace_syscall_info {
        pub op: crate::__u8,
        pub pad: [crate::__u8; 3],
        pub arch: crate::__u32,
        pub instruction_pointer: crate::__u64,
        pub stack_pointer: crate::__u64,
        pub u: __c_anonymous_ptrace_syscall_info_data,
    }

    pub struct ptrace_sud_config {
        pub mode: crate::__u64,
        pub selector: crate::__u64,
        pub offset: crate::__u64,
        pub len: crate::__u64,
    }

    pub struct iocb {
        pub aio_data: crate::__u64,
        #[cfg(target_endian = "little")]
        pub aio_key: crate::__u32,
        #[cfg(target_endian = "little")]
        pub aio_rw_flags: crate::__kernel_rwf_t,
        #[cfg(target_endian = "big")]
        pub aio_rw_flags: crate::__kernel_rwf_t,
        #[cfg(target_endian = "big")]
        pub aio_key: crate::__u32,
        pub aio_lio_opcode: crate::__u16,
        pub aio_reqprio: crate::__s16,
        pub aio_fildes: crate::__u32,
        pub aio_buf: crate::__u64,
        pub aio_nbytes: crate::__u64,
        pub aio_offset: crate::__s64,
        aio_reserved2: crate::__u64,
        pub aio_flags: crate::__u32,
        pub aio_resfd: crate::__u32,
    }

    // netinet/tcp.h

    pub struct tcp_info {
        pub tcpi_state: u8,
        pub tcpi_ca_state: u8,
        pub tcpi_retransmits: u8,
        pub tcpi_probes: u8,
        pub tcpi_backoff: u8,
        pub tcpi_options: u8,
        /// This contains the bitfields `tcpi_snd_wscale` and `tcpi_rcv_wscale`.
        /// Each is 4 bits.
        pub tcpi_snd_rcv_wscale: u8,
        pub tcpi_rto: u32,
        pub tcpi_ato: u32,
        pub tcpi_snd_mss: u32,
        pub tcpi_rcv_mss: u32,
        pub tcpi_unacked: u32,
        pub tcpi_sacked: u32,
        pub tcpi_lost: u32,
        pub tcpi_retrans: u32,
        pub tcpi_fackets: u32,
        pub tcpi_last_data_sent: u32,
        pub tcpi_last_ack_sent: u32,
        pub tcpi_last_data_recv: u32,
        pub tcpi_last_ack_recv: u32,
        pub tcpi_pmtu: u32,
        pub tcpi_rcv_ssthresh: u32,
        pub tcpi_rtt: u32,
        pub tcpi_rttvar: u32,
        pub tcpi_snd_ssthresh: u32,
        pub tcpi_snd_cwnd: u32,
        pub tcpi_advmss: u32,
        pub tcpi_reordering: u32,
        pub tcpi_rcv_rtt: u32,
        pub tcpi_rcv_space: u32,
        pub tcpi_total_retrans: u32,
    }

    pub struct fanotify_event_info_pidfd {
        pub hdr: crate::fanotify_event_info_header,
        pub pidfd: crate::__s32,
    }

    pub struct fanotify_event_info_error {
        pub hdr: crate::fanotify_event_info_header,
        pub error: crate::__s32,
        pub error_count: crate::__u32,
    }

    // FIXME(1.0) this is actually a union
    #[cfg_attr(target_pointer_width = "32", repr(align(4)))]
    #[cfg_attr(target_pointer_width = "64", repr(align(8)))]
    pub struct sem_t {
        #[cfg(target_pointer_width = "32")]
        __size: [c_char; 16],
        #[cfg(target_pointer_width = "64")]
        __size: [c_char; 32],
    }

    pub struct mbstate_t {
        __count: c_int,
        __wchb: [c_char; 4],
    }

    pub struct fpos64_t {
        __pos: off64_t,
        __state: crate::mbstate_t,
    }

    pub struct fpos_t {
        #[cfg(not(gnu_file_offset_bits64))]
        __pos: off_t,
        #[cfg(gnu_file_offset_bits64)]
        __pos: off64_t,
        __state: crate::mbstate_t,
    }

    // linux x32 compatibility
    // See https://sourceware.org/bugzilla/show_bug.cgi?id=16437
    pub struct timespec {
        pub tv_sec: time_t,
        #[cfg(all(gnu_time_bits64, target_endian = "big"))]
        __pad: i32,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub tv_nsec: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub tv_nsec: i64,
        #[cfg(all(gnu_time_bits64, target_endian = "little"))]
        __pad: i32,
    }
}

impl siginfo_t {
    pub unsafe fn si_addr(&self) -> *mut c_void {
        #[repr(C)]
        struct siginfo_sigfault {
            _si_signo: c_int,
            _si_errno: c_int,
            _si_code: c_int,
            si_addr: *mut c_void,
        }
        (*(self as *const siginfo_t).cast::<siginfo_sigfault>()).si_addr
    }

    pub unsafe fn si_value(&self) -> crate::sigval {
        #[repr(C)]
        struct siginfo_timer {
            _si_signo: c_int,
            _si_errno: c_int,
            _si_code: c_int,
            _si_tid: c_int,
            _si_overrun: c_int,
            si_sigval: crate::sigval,
        }
        (*(self as *const siginfo_t).cast::<siginfo_timer>()).si_sigval
    }
}

// Internal, for casts to access union fields
#[repr(C)]
struct sifields_sigchld {
    si_pid: crate::pid_t,
    si_uid: crate::uid_t,
    si_status: c_int,
    si_utime: c_long,
    si_stime: c_long,
}
impl Copy for sifields_sigchld {}
impl Clone for sifields_sigchld {
    fn clone(&self) -> sifields_sigchld {
        *self
    }
}

// Internal, for casts to access union fields
#[repr(C)]
union sifields {
    _align_pointer: *mut c_void,
    sigchld: sifields_sigchld,
}

// Internal, for casts to access union fields. Note that some variants
// of sifields start with a pointer, which makes the alignment of
// sifields vary on 32-bit and 64-bit architectures.
#[repr(C)]
struct siginfo_f {
    _siginfo_base: [c_int; 3],
    sifields: sifields,
}

impl siginfo_t {
    unsafe fn sifields(&self) -> &sifields {
        &(*(self as *const siginfo_t).cast::<siginfo_f>()).sifields
    }

    pub unsafe fn si_pid(&self) -> crate::pid_t {
        self.sifields().sigchld.si_pid
    }

    pub unsafe fn si_uid(&self) -> crate::uid_t {
        self.sifields().sigchld.si_uid
    }

    pub unsafe fn si_status(&self) -> c_int {
        self.sifields().sigchld.si_status
    }

    pub unsafe fn si_utime(&self) -> c_long {
        self.sifields().sigchld.si_utime
    }

    pub unsafe fn si_stime(&self) -> c_long {
        self.sifields().sigchld.si_stime
    }
}

s_no_extra_traits! {
    pub union __c_anonymous_ptrace_syscall_info_data {
        pub entry: __c_anonymous_ptrace_syscall_info_entry,
        pub exit: __c_anonymous_ptrace_syscall_info_exit,
        pub seccomp: __c_anonymous_ptrace_syscall_info_seccomp,
    }

    pub struct utmpx {
        pub ut_type: c_short,
        pub ut_pid: crate::pid_t,
        pub ut_line: [c_char; __UT_LINESIZE],
        pub ut_id: [c_char; 4],

        pub ut_user: [c_char; __UT_NAMESIZE],
        pub ut_host: [c_char; __UT_HOSTSIZE],
        pub ut_exit: __exit_status,

        #[cfg(any(
            target_arch = "aarch64",
            target_arch = "s390x",
            target_arch = "loongarch64",
            all(target_pointer_width = "32", not(target_arch = "x86_64"))
        ))]
        pub ut_session: c_long,
        #[cfg(any(
            target_arch = "aarch64",
            target_arch = "s390x",
            target_arch = "loongarch64",
            all(target_pointer_width = "32", not(target_arch = "x86_64"))
        ))]
        pub ut_tv: crate::timeval,

        #[cfg(not(any(
            target_arch = "aarch64",
            target_arch = "s390x",
            target_arch = "loongarch64",
            all(target_pointer_width = "32", not(target_arch = "x86_64"))
        )))]
        pub ut_session: i32,
        #[cfg(not(any(
            target_arch = "aarch64",
            target_arch = "s390x",
            target_arch = "loongarch64",
            all(target_pointer_width = "32", not(target_arch = "x86_64"))
        )))]
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

        impl PartialEq for __c_anonymous_ptrace_syscall_info_data {
            fn eq(&self, other: &__c_anonymous_ptrace_syscall_info_data) -> bool {
                unsafe {
                    self.entry == other.entry
                        || self.exit == other.exit
                        || self.seccomp == other.seccomp
                }
            }
        }

        impl Eq for __c_anonymous_ptrace_syscall_info_data {}

        impl hash::Hash for __c_anonymous_ptrace_syscall_info_data {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    self.entry.hash(state);
                    self.exit.hash(state);
                    self.seccomp.hash(state);
                }
            }
        }
    }
}

// include/uapi/asm-generic/hugetlb_encode.h
pub const HUGETLB_FLAG_ENCODE_SHIFT: c_int = 26;
pub const HUGETLB_FLAG_ENCODE_MASK: c_int = 0x3f;

pub const HUGETLB_FLAG_ENCODE_64KB: c_int = 16 << HUGETLB_FLAG_ENCODE_SHIFT;
pub const HUGETLB_FLAG_ENCODE_512KB: c_int = 19 << HUGETLB_FLAG_ENCODE_SHIFT;
pub const HUGETLB_FLAG_ENCODE_1MB: c_int = 20 << HUGETLB_FLAG_ENCODE_SHIFT;
pub const HUGETLB_FLAG_ENCODE_2MB: c_int = 21 << HUGETLB_FLAG_ENCODE_SHIFT;
pub const HUGETLB_FLAG_ENCODE_8MB: c_int = 23 << HUGETLB_FLAG_ENCODE_SHIFT;
pub const HUGETLB_FLAG_ENCODE_16MB: c_int = 24 << HUGETLB_FLAG_ENCODE_SHIFT;
pub const HUGETLB_FLAG_ENCODE_32MB: c_int = 25 << HUGETLB_FLAG_ENCODE_SHIFT;
pub const HUGETLB_FLAG_ENCODE_256MB: c_int = 28 << HUGETLB_FLAG_ENCODE_SHIFT;
pub const HUGETLB_FLAG_ENCODE_512MB: c_int = 29 << HUGETLB_FLAG_ENCODE_SHIFT;
pub const HUGETLB_FLAG_ENCODE_1GB: c_int = 30 << HUGETLB_FLAG_ENCODE_SHIFT;
pub const HUGETLB_FLAG_ENCODE_2GB: c_int = 31 << HUGETLB_FLAG_ENCODE_SHIFT;
pub const HUGETLB_FLAG_ENCODE_16GB: c_int = 34 << HUGETLB_FLAG_ENCODE_SHIFT;

// include/uapi/linux/mman.h
/*
 * Huge page size encoding when MAP_HUGETLB is specified, and a huge page
 * size other than the default is desired.  See hugetlb_encode.h.
 * All known huge page size encodings are provided here.  It is the
 * responsibility of the application to know which sizes are supported on
 * the running system.  See mmap(2) man page for details.
 */
pub const MAP_HUGE_SHIFT: c_int = HUGETLB_FLAG_ENCODE_SHIFT;
pub const MAP_HUGE_MASK: c_int = HUGETLB_FLAG_ENCODE_MASK;

pub const MAP_HUGE_64KB: c_int = HUGETLB_FLAG_ENCODE_64KB;
pub const MAP_HUGE_512KB: c_int = HUGETLB_FLAG_ENCODE_512KB;
pub const MAP_HUGE_1MB: c_int = HUGETLB_FLAG_ENCODE_1MB;
pub const MAP_HUGE_2MB: c_int = HUGETLB_FLAG_ENCODE_2MB;
pub const MAP_HUGE_8MB: c_int = HUGETLB_FLAG_ENCODE_8MB;
pub const MAP_HUGE_16MB: c_int = HUGETLB_FLAG_ENCODE_16MB;
pub const MAP_HUGE_32MB: c_int = HUGETLB_FLAG_ENCODE_32MB;
pub const MAP_HUGE_256MB: c_int = HUGETLB_FLAG_ENCODE_256MB;
pub const MAP_HUGE_512MB: c_int = HUGETLB_FLAG_ENCODE_512MB;
pub const MAP_HUGE_1GB: c_int = HUGETLB_FLAG_ENCODE_1GB;
pub const MAP_HUGE_2GB: c_int = HUGETLB_FLAG_ENCODE_2GB;
pub const MAP_HUGE_16GB: c_int = HUGETLB_FLAG_ENCODE_16GB;

pub const PRIO_PROCESS: crate::__priority_which_t = 0;
pub const PRIO_PGRP: crate::__priority_which_t = 1;
pub const PRIO_USER: crate::__priority_which_t = 2;

pub const MS_RMT_MASK: c_ulong = 0x02800051;

pub const __UT_LINESIZE: usize = 32;
pub const __UT_NAMESIZE: usize = 32;
pub const __UT_HOSTSIZE: usize = 256;
pub const EMPTY: c_short = 0;
pub const RUN_LVL: c_short = 1;
pub const BOOT_TIME: c_short = 2;
pub const NEW_TIME: c_short = 3;
pub const OLD_TIME: c_short = 4;
pub const INIT_PROCESS: c_short = 5;
pub const LOGIN_PROCESS: c_short = 6;
pub const USER_PROCESS: c_short = 7;
pub const DEAD_PROCESS: c_short = 8;
pub const ACCOUNTING: c_short = 9;

// dlfcn.h
pub const LM_ID_BASE: c_long = 0;
pub const LM_ID_NEWLM: c_long = -1;

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

pub const SOCK_NONBLOCK: c_int = O_NONBLOCK;

pub const SOL_RXRPC: c_int = 272;
pub const SOL_PPPOL2TP: c_int = 273;
pub const SOL_PNPIPE: c_int = 275;
pub const SOL_RDS: c_int = 276;
pub const SOL_IUCV: c_int = 277;
pub const SOL_CAIF: c_int = 278;
pub const SOL_NFC: c_int = 280;

pub const MSG_TRYHARD: c_int = 4;

pub const LC_PAPER: c_int = 7;
pub const LC_NAME: c_int = 8;
pub const LC_ADDRESS: c_int = 9;
pub const LC_TELEPHONE: c_int = 10;
pub const LC_MEASUREMENT: c_int = 11;
pub const LC_IDENTIFICATION: c_int = 12;
pub const LC_PAPER_MASK: c_int = 1 << LC_PAPER;
pub const LC_NAME_MASK: c_int = 1 << LC_NAME;
pub const LC_ADDRESS_MASK: c_int = 1 << LC_ADDRESS;
pub const LC_TELEPHONE_MASK: c_int = 1 << LC_TELEPHONE;
pub const LC_MEASUREMENT_MASK: c_int = 1 << LC_MEASUREMENT;
pub const LC_IDENTIFICATION_MASK: c_int = 1 << LC_IDENTIFICATION;
pub const LC_ALL_MASK: c_int = crate::LC_CTYPE_MASK
    | crate::LC_NUMERIC_MASK
    | crate::LC_TIME_MASK
    | crate::LC_COLLATE_MASK
    | crate::LC_MONETARY_MASK
    | crate::LC_MESSAGES_MASK
    | LC_PAPER_MASK
    | LC_NAME_MASK
    | LC_ADDRESS_MASK
    | LC_TELEPHONE_MASK
    | LC_MEASUREMENT_MASK
    | LC_IDENTIFICATION_MASK;

pub const ENOTSUP: c_int = EOPNOTSUPP;

pub const SOCK_SEQPACKET: c_int = 5;
pub const SOCK_DCCP: c_int = 6;
#[deprecated(since = "0.2.70", note = "AF_PACKET must be used instead")]
pub const SOCK_PACKET: c_int = 10;

pub const AF_IB: c_int = 27;
pub const AF_MPLS: c_int = 28;
pub const AF_NFC: c_int = 39;
pub const AF_VSOCK: c_int = 40;
pub const AF_XDP: c_int = 44;
pub const PF_IB: c_int = AF_IB;
pub const PF_MPLS: c_int = AF_MPLS;
pub const PF_NFC: c_int = AF_NFC;
pub const PF_VSOCK: c_int = AF_VSOCK;
pub const PF_XDP: c_int = AF_XDP;

pub const SIGEV_THREAD_ID: c_int = 4;

pub const BUFSIZ: c_uint = 8192;
pub const TMP_MAX: c_uint = 238328;
pub const FOPEN_MAX: c_uint = 16;
pub const FILENAME_MAX: c_uint = 4096;
pub const POSIX_MADV_DONTNEED: c_int = 4;
pub const _CS_GNU_LIBC_VERSION: c_int = 2;
pub const _CS_GNU_LIBPTHREAD_VERSION: c_int = 3;
pub const _CS_V6_ENV: c_int = 1148;
pub const _CS_V7_ENV: c_int = 1149;
pub const _SC_EQUIV_CLASS_MAX: c_int = 41;
pub const _SC_CHARCLASS_NAME_MAX: c_int = 45;
pub const _SC_PII: c_int = 53;
pub const _SC_PII_XTI: c_int = 54;
pub const _SC_PII_SOCKET: c_int = 55;
pub const _SC_PII_INTERNET: c_int = 56;
pub const _SC_PII_OSI: c_int = 57;
pub const _SC_POLL: c_int = 58;
pub const _SC_SELECT: c_int = 59;
pub const _SC_PII_INTERNET_STREAM: c_int = 61;
pub const _SC_PII_INTERNET_DGRAM: c_int = 62;
pub const _SC_PII_OSI_COTS: c_int = 63;
pub const _SC_PII_OSI_CLTS: c_int = 64;
pub const _SC_PII_OSI_M: c_int = 65;
pub const _SC_T_IOV_MAX: c_int = 66;
pub const _SC_2_C_VERSION: c_int = 96;
pub const _SC_CHAR_BIT: c_int = 101;
pub const _SC_CHAR_MAX: c_int = 102;
pub const _SC_CHAR_MIN: c_int = 103;
pub const _SC_INT_MAX: c_int = 104;
pub const _SC_INT_MIN: c_int = 105;
pub const _SC_LONG_BIT: c_int = 106;
pub const _SC_WORD_BIT: c_int = 107;
pub const _SC_MB_LEN_MAX: c_int = 108;
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
pub const _SC_BASE: c_int = 134;
pub const _SC_C_LANG_SUPPORT: c_int = 135;
pub const _SC_C_LANG_SUPPORT_R: c_int = 136;
pub const _SC_DEVICE_IO: c_int = 140;
pub const _SC_DEVICE_SPECIFIC: c_int = 141;
pub const _SC_DEVICE_SPECIFIC_R: c_int = 142;
pub const _SC_FD_MGMT: c_int = 143;
pub const _SC_FIFO: c_int = 144;
pub const _SC_PIPE: c_int = 145;
pub const _SC_FILE_ATTRIBUTES: c_int = 146;
pub const _SC_FILE_LOCKING: c_int = 147;
pub const _SC_FILE_SYSTEM: c_int = 148;
pub const _SC_MULTI_PROCESS: c_int = 150;
pub const _SC_SINGLE_PROCESS: c_int = 151;
pub const _SC_NETWORKING: c_int = 152;
pub const _SC_REGEX_VERSION: c_int = 156;
pub const _SC_SIGNALS: c_int = 158;
pub const _SC_SYSTEM_DATABASE: c_int = 162;
pub const _SC_SYSTEM_DATABASE_R: c_int = 163;
pub const _SC_USER_GROUPS: c_int = 166;
pub const _SC_USER_GROUPS_R: c_int = 167;
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
pub const O_ACCMODE: c_int = 3;
pub const ST_RELATIME: c_ulong = 4096;
pub const NI_MAXHOST: crate::socklen_t = 1025;

// Most `*_SUPER_MAGIC` constants are defined at the `linux_like` level; the
// following are only available on newer Linux versions than the versions
// currently used in CI in some configurations, so we define them here.
cfg_if! {
    if #[cfg(not(target_arch = "s390x"))] {
        pub const BINDERFS_SUPER_MAGIC: c_long = 0x6c6f6f70;
        pub const XFS_SUPER_MAGIC: c_long = 0x58465342;
    } else if #[cfg(target_arch = "s390x")] {
        pub const BINDERFS_SUPER_MAGIC: c_uint = 0x6c6f6f70;
        pub const XFS_SUPER_MAGIC: c_uint = 0x58465342;
    }
}

pub const CPU_SETSIZE: c_int = 0x400;

pub const PTRACE_TRACEME: c_uint = 0;
pub const PTRACE_PEEKTEXT: c_uint = 1;
pub const PTRACE_PEEKDATA: c_uint = 2;
pub const PTRACE_PEEKUSER: c_uint = 3;
pub const PTRACE_POKETEXT: c_uint = 4;
pub const PTRACE_POKEDATA: c_uint = 5;
pub const PTRACE_POKEUSER: c_uint = 6;
pub const PTRACE_CONT: c_uint = 7;
pub const PTRACE_KILL: c_uint = 8;
pub const PTRACE_SINGLESTEP: c_uint = 9;
pub const PTRACE_ATTACH: c_uint = 16;
pub const PTRACE_SYSCALL: c_uint = 24;
pub const PTRACE_SETOPTIONS: c_uint = 0x4200;
pub const PTRACE_GETEVENTMSG: c_uint = 0x4201;
pub const PTRACE_GETSIGINFO: c_uint = 0x4202;
pub const PTRACE_SETSIGINFO: c_uint = 0x4203;
pub const PTRACE_GETREGSET: c_uint = 0x4204;
pub const PTRACE_SETREGSET: c_uint = 0x4205;
pub const PTRACE_SEIZE: c_uint = 0x4206;
pub const PTRACE_INTERRUPT: c_uint = 0x4207;
pub const PTRACE_LISTEN: c_uint = 0x4208;
pub const PTRACE_PEEKSIGINFO: c_uint = 0x4209;
pub const PTRACE_GETSIGMASK: c_uint = 0x420a;
pub const PTRACE_SETSIGMASK: c_uint = 0x420b;
pub const PTRACE_GET_SYSCALL_INFO: c_uint = 0x420e;
pub const PTRACE_SYSCALL_INFO_NONE: crate::__u8 = 0;
pub const PTRACE_SYSCALL_INFO_ENTRY: crate::__u8 = 1;
pub const PTRACE_SYSCALL_INFO_EXIT: crate::__u8 = 2;
pub const PTRACE_SYSCALL_INFO_SECCOMP: crate::__u8 = 3;
pub const PTRACE_SET_SYSCALL_USER_DISPATCH_CONFIG: crate::__u8 = 0x4210;
pub const PTRACE_GET_SYSCALL_USER_DISPATCH_CONFIG: crate::__u8 = 0x4211;

// linux/rtnetlink.h
pub const TCA_PAD: c_ushort = 9;
pub const TCA_DUMP_INVISIBLE: c_ushort = 10;
pub const TCA_CHAIN: c_ushort = 11;
pub const TCA_HW_OFFLOAD: c_ushort = 12;

pub const RTM_DELNETCONF: u16 = 81;
pub const RTM_NEWSTATS: u16 = 92;
pub const RTM_GETSTATS: u16 = 94;
pub const RTM_NEWCACHEREPORT: u16 = 96;

pub const RTM_F_LOOKUP_TABLE: c_uint = 0x1000;
pub const RTM_F_FIB_MATCH: c_uint = 0x2000;

pub const RTA_VIA: c_ushort = 18;
pub const RTA_NEWDST: c_ushort = 19;
pub const RTA_PREF: c_ushort = 20;
pub const RTA_ENCAP_TYPE: c_ushort = 21;
pub const RTA_ENCAP: c_ushort = 22;
pub const RTA_EXPIRES: c_ushort = 23;
pub const RTA_PAD: c_ushort = 24;
pub const RTA_UID: c_ushort = 25;
pub const RTA_TTL_PROPAGATE: c_ushort = 26;

// linux/neighbor.h
pub const NTF_EXT_LEARNED: u8 = 0x10;
pub const NTF_OFFLOADED: u8 = 0x20;

pub const NDA_MASTER: c_ushort = 9;
pub const NDA_LINK_NETNSID: c_ushort = 10;
pub const NDA_SRC_VNI: c_ushort = 11;

// linux/personality.h
pub const UNAME26: c_int = 0x0020000;
pub const FDPIC_FUNCPTRS: c_int = 0x0080000;

pub const MAX_LINKS: c_int = 32;

pub const GENL_UNS_ADMIN_PERM: c_int = 0x10;

pub const GENL_ID_VFS_DQUOT: c_int = crate::NLMSG_MIN_TYPE + 1;
pub const GENL_ID_PMCRAID: c_int = crate::NLMSG_MIN_TYPE + 2;

pub const ELFOSABI_ARM_AEABI: u8 = 64;

// linux/sched.h
pub const CLONE_NEWTIME: c_int = 0x80;
// DIFF(main): changed to `c_ulonglong` in e9abac9ac2
pub const CLONE_CLEAR_SIGHAND: c_int = 0x100000000;
pub const CLONE_INTO_CGROUP: c_int = 0x200000000;

// linux/keyctl.h
pub const KEYCTL_DH_COMPUTE: u32 = 23;
pub const KEYCTL_PKEY_QUERY: u32 = 24;
pub const KEYCTL_PKEY_ENCRYPT: u32 = 25;
pub const KEYCTL_PKEY_DECRYPT: u32 = 26;
pub const KEYCTL_PKEY_SIGN: u32 = 27;
pub const KEYCTL_PKEY_VERIFY: u32 = 28;
pub const KEYCTL_RESTRICT_KEYRING: u32 = 29;

pub const KEYCTL_SUPPORTS_ENCRYPT: u32 = 0x01;
pub const KEYCTL_SUPPORTS_DECRYPT: u32 = 0x02;
pub const KEYCTL_SUPPORTS_SIGN: u32 = 0x04;
pub const KEYCTL_SUPPORTS_VERIFY: u32 = 0x08;
cfg_if! {
    if #[cfg(not(any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6"
    )))] {
        pub const KEYCTL_MOVE: u32 = 30;
        pub const KEYCTL_CAPABILITIES: u32 = 31;

        pub const KEYCTL_CAPS0_CAPABILITIES: u32 = 0x01;
        pub const KEYCTL_CAPS0_PERSISTENT_KEYRINGS: u32 = 0x02;
        pub const KEYCTL_CAPS0_DIFFIE_HELLMAN: u32 = 0x04;
        pub const KEYCTL_CAPS0_PUBLIC_KEY: u32 = 0x08;
        pub const KEYCTL_CAPS0_BIG_KEY: u32 = 0x10;
        pub const KEYCTL_CAPS0_INVALIDATE: u32 = 0x20;
        pub const KEYCTL_CAPS0_RESTRICT_KEYRING: u32 = 0x40;
        pub const KEYCTL_CAPS0_MOVE: u32 = 0x80;
        pub const KEYCTL_CAPS1_NS_KEYRING_NAME: u32 = 0x01;
        pub const KEYCTL_CAPS1_NS_KEY_TAG: u32 = 0x02;
    }
}

pub const M_MXFAST: c_int = 1;
pub const M_NLBLKS: c_int = 2;
pub const M_GRAIN: c_int = 3;
pub const M_KEEP: c_int = 4;
pub const M_TRIM_THRESHOLD: c_int = -1;
pub const M_TOP_PAD: c_int = -2;
pub const M_MMAP_THRESHOLD: c_int = -3;
pub const M_MMAP_MAX: c_int = -4;
pub const M_CHECK_ACTION: c_int = -5;
pub const M_PERTURB: c_int = -6;
pub const M_ARENA_TEST: c_int = -7;
pub const M_ARENA_MAX: c_int = -8;

pub const SOMAXCONN: c_int = 4096;

// linux/mount.h
pub const MOVE_MOUNT_F_SYMLINKS: c_uint = 0x00000001;
pub const MOVE_MOUNT_F_AUTOMOUNTS: c_uint = 0x00000002;
pub const MOVE_MOUNT_F_EMPTY_PATH: c_uint = 0x00000004;
pub const MOVE_MOUNT_T_SYMLINKS: c_uint = 0x00000010;
pub const MOVE_MOUNT_T_AUTOMOUNTS: c_uint = 0x00000020;
pub const MOVE_MOUNT_T_EMPTY_PATH: c_uint = 0x00000040;
pub const MOVE_MOUNT_SET_GROUP: c_uint = 0x00000100;
pub const MOVE_MOUNT_BENEATH: c_uint = 0x00000200;

// sys/timex.h
pub const ADJ_OFFSET: c_uint = 0x0001;
pub const ADJ_FREQUENCY: c_uint = 0x0002;
pub const ADJ_MAXERROR: c_uint = 0x0004;
pub const ADJ_ESTERROR: c_uint = 0x0008;
pub const ADJ_STATUS: c_uint = 0x0010;
pub const ADJ_TIMECONST: c_uint = 0x0020;
pub const ADJ_TAI: c_uint = 0x0080;
pub const ADJ_SETOFFSET: c_uint = 0x0100;
pub const ADJ_MICRO: c_uint = 0x1000;
pub const ADJ_NANO: c_uint = 0x2000;
pub const ADJ_TICK: c_uint = 0x4000;
pub const ADJ_OFFSET_SINGLESHOT: c_uint = 0x8001;
pub const ADJ_OFFSET_SS_READ: c_uint = 0xa001;
pub const MOD_OFFSET: c_uint = ADJ_OFFSET;
pub const MOD_FREQUENCY: c_uint = ADJ_FREQUENCY;
pub const MOD_MAXERROR: c_uint = ADJ_MAXERROR;
pub const MOD_ESTERROR: c_uint = ADJ_ESTERROR;
pub const MOD_STATUS: c_uint = ADJ_STATUS;
pub const MOD_TIMECONST: c_uint = ADJ_TIMECONST;
pub const MOD_CLKB: c_uint = ADJ_TICK;
pub const MOD_CLKA: c_uint = ADJ_OFFSET_SINGLESHOT;
pub const MOD_TAI: c_uint = ADJ_TAI;
pub const MOD_MICRO: c_uint = ADJ_MICRO;
pub const MOD_NANO: c_uint = ADJ_NANO;
pub const STA_PLL: c_int = 0x0001;
pub const STA_PPSFREQ: c_int = 0x0002;
pub const STA_PPSTIME: c_int = 0x0004;
pub const STA_FLL: c_int = 0x0008;
pub const STA_INS: c_int = 0x0010;
pub const STA_DEL: c_int = 0x0020;
pub const STA_UNSYNC: c_int = 0x0040;
pub const STA_FREQHOLD: c_int = 0x0080;
pub const STA_PPSSIGNAL: c_int = 0x0100;
pub const STA_PPSJITTER: c_int = 0x0200;
pub const STA_PPSWANDER: c_int = 0x0400;
pub const STA_PPSERROR: c_int = 0x0800;
pub const STA_CLOCKERR: c_int = 0x1000;
pub const STA_NANO: c_int = 0x2000;
pub const STA_MODE: c_int = 0x4000;
pub const STA_CLK: c_int = 0x8000;
pub const STA_RONLY: c_int = STA_PPSSIGNAL
    | STA_PPSJITTER
    | STA_PPSWANDER
    | STA_PPSERROR
    | STA_CLOCKERR
    | STA_NANO
    | STA_MODE
    | STA_CLK;
pub const NTP_API: c_int = 4;
pub const TIME_OK: c_int = 0;
pub const TIME_INS: c_int = 1;
pub const TIME_DEL: c_int = 2;
pub const TIME_OOP: c_int = 3;
pub const TIME_WAIT: c_int = 4;
pub const TIME_ERROR: c_int = 5;
pub const TIME_BAD: c_int = TIME_ERROR;
pub const MAXTC: c_long = 6;

// Portable GLOB_* flags are defined at the `linux_like` level.
// The following are GNU extensions.
pub const GLOB_PERIOD: c_int = 1 << 7;
pub const GLOB_ALTDIRFUNC: c_int = 1 << 9;
pub const GLOB_BRACE: c_int = 1 << 10;
pub const GLOB_NOMAGIC: c_int = 1 << 11;
pub const GLOB_TILDE: c_int = 1 << 12;
pub const GLOB_ONLYDIR: c_int = 1 << 13;
pub const GLOB_TILDE_CHECK: c_int = 1 << 14;

pub const MADV_COLLAPSE: c_int = 25;

cfg_if! {
    if #[cfg(any(
        target_arch = "arm",
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "s390x",
        target_arch = "riscv64",
        target_arch = "riscv32"
    ))] {
        pub const PTHREAD_STACK_MIN: size_t = 16384;
    } else if #[cfg(any(target_arch = "sparc", target_arch = "sparc64"))] {
        pub const PTHREAD_STACK_MIN: size_t = 0x6000;
    } else {
        pub const PTHREAD_STACK_MIN: size_t = 131072;
    }
}
pub const PTHREAD_MUTEX_ADAPTIVE_NP: c_int = 3;

pub const REG_STARTEND: c_int = 4;

pub const REG_EEND: c_int = 14;
pub const REG_ESIZE: c_int = 15;
pub const REG_ERPAREN: c_int = 16;

extern "C" {
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
    pub fn qsort_r(
        base: *mut c_void,
        num: size_t,
        size: size_t,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void, *mut c_void) -> c_int>,
        arg: *mut c_void,
    );
    #[cfg_attr(gnu_time_bits64, link_name = "__sendmmsg64")]
    pub fn sendmmsg(
        sockfd: c_int,
        msgvec: *mut crate::mmsghdr,
        vlen: c_uint,
        flags: c_int,
    ) -> c_int;
    #[cfg_attr(gnu_time_bits64, link_name = "__recvmmsg64")]
    pub fn recvmmsg(
        sockfd: c_int,
        msgvec: *mut crate::mmsghdr,
        vlen: c_uint,
        flags: c_int,
        timeout: *mut crate::timespec,
    ) -> c_int;

    pub fn getrlimit64(resource: crate::__rlimit_resource_t, rlim: *mut crate::rlimit64) -> c_int;
    pub fn setrlimit64(resource: crate::__rlimit_resource_t, rlim: *const crate::rlimit64)
        -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "getrlimit64")]
    pub fn getrlimit(resource: crate::__rlimit_resource_t, rlim: *mut crate::rlimit) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "setrlimit64")]
    pub fn setrlimit(resource: crate::__rlimit_resource_t, rlim: *const crate::rlimit) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "prlimit64")]
    pub fn prlimit(
        pid: crate::pid_t,
        resource: crate::__rlimit_resource_t,
        new_limit: *const crate::rlimit,
        old_limit: *mut crate::rlimit,
    ) -> c_int;
    pub fn prlimit64(
        pid: crate::pid_t,
        resource: crate::__rlimit_resource_t,
        new_limit: *const crate::rlimit64,
        old_limit: *mut crate::rlimit64,
    ) -> c_int;
    pub fn utmpname(file: *const c_char) -> c_int;
    pub fn utmpxname(file: *const c_char) -> c_int;
    pub fn getutxent() -> *mut utmpx;
    pub fn getutxid(ut: *const utmpx) -> *mut utmpx;
    pub fn getutxline(ut: *const utmpx) -> *mut utmpx;
    pub fn pututxline(ut: *const utmpx) -> *mut utmpx;
    pub fn setutxent();
    pub fn endutxent();
    pub fn getpt() -> c_int;
    pub fn mallopt(param: c_int, value: c_int) -> c_int;
    #[cfg_attr(gnu_time_bits64, link_name = "__gettimeofday64")]
    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut crate::timezone) -> c_int;
    pub fn getentropy(buf: *mut c_void, buflen: size_t) -> c_int;
    pub fn getrandom(buf: *mut c_void, buflen: size_t, flags: c_uint) -> ssize_t;
    pub fn getauxval(type_: c_ulong) -> c_ulong;

    #[cfg_attr(gnu_time_bits64, link_name = "___adjtimex64")]
    pub fn adjtimex(buf: *mut timex) -> c_int;
    #[cfg_attr(gnu_time_bits64, link_name = "___adjtimex64")]
    pub fn ntp_adjtime(buf: *mut timex) -> c_int;
    #[cfg_attr(not(gnu_time_bits64), link_name = "ntp_gettimex")]
    #[cfg_attr(gnu_time_bits64, link_name = "__ntp_gettime64")]
    pub fn ntp_gettime(buf: *mut ntptimeval) -> c_int;
    #[cfg_attr(gnu_time_bits64, link_name = "__clock_adjtime64")]
    pub fn clock_adjtime(clk_id: crate::clockid_t, buf: *mut crate::timex) -> c_int;

    pub fn fanotify_mark(
        fd: c_int,
        flags: c_uint,
        mask: u64,
        dirfd: c_int,
        path: *const c_char,
    ) -> c_int;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "preadv64v2")]
    pub fn preadv2(
        fd: c_int,
        iov: *const crate::iovec,
        iovcnt: c_int,
        offset: off_t,
        flags: c_int,
    ) -> ssize_t;
    #[cfg_attr(gnu_file_offset_bits64, link_name = "pwritev64v2")]
    pub fn pwritev2(
        fd: c_int,
        iov: *const crate::iovec,
        iovcnt: c_int,
        offset: off_t,
        flags: c_int,
    ) -> ssize_t;
    pub fn preadv64v2(
        fd: c_int,
        iov: *const crate::iovec,
        iovcnt: c_int,
        offset: off64_t,
        flags: c_int,
    ) -> ssize_t;
    pub fn pwritev64v2(
        fd: c_int,
        iov: *const crate::iovec,
        iovcnt: c_int,
        offset: off64_t,
        flags: c_int,
    ) -> ssize_t;
    pub fn renameat2(
        olddirfd: c_int,
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
        flags: c_uint,
    ) -> c_int;

    // Added in `glibc` 2.25
    pub fn explicit_bzero(s: *mut c_void, len: size_t);
    // Added in `glibc` 2.29
    pub fn reallocarray(ptr: *mut c_void, nmemb: size_t, size: size_t) -> *mut c_void;

    pub fn ctermid(s: *mut c_char) -> *mut c_char;
    pub fn backtrace(buf: *mut *mut c_void, sz: c_int) -> c_int;
    #[cfg_attr(gnu_time_bits64, link_name = "__glob64_time64")]
    pub fn glob64(
        pattern: *const c_char,
        flags: c_int,
        errfunc: Option<extern "C" fn(epath: *const c_char, errno: c_int) -> c_int>,
        pglob: *mut glob64_t,
    ) -> c_int;
    #[cfg_attr(gnu_time_bits64, link_name = "__globfree64_time64")]
    pub fn globfree64(pglob: *mut glob64_t);
    pub fn ptrace(request: c_uint, ...) -> c_long;
    pub fn pthread_attr_getaffinity_np(
        attr: *const crate::pthread_attr_t,
        cpusetsize: size_t,
        cpuset: *mut crate::cpu_set_t,
    ) -> c_int;
    pub fn pthread_attr_setaffinity_np(
        attr: *mut crate::pthread_attr_t,
        cpusetsize: size_t,
        cpuset: *const crate::cpu_set_t,
    ) -> c_int;
    pub fn getpriority(which: crate::__priority_which_t, who: crate::id_t) -> c_int;
    pub fn setpriority(which: crate::__priority_which_t, who: crate::id_t, prio: c_int) -> c_int;
    pub fn pthread_rwlockattr_getkind_np(
        attr: *const crate::pthread_rwlockattr_t,
        val: *mut c_int,
    ) -> c_int;
    pub fn pthread_rwlockattr_setkind_np(
        attr: *mut crate::pthread_rwlockattr_t,
        val: c_int,
    ) -> c_int;
    pub fn pthread_sigqueue(thread: crate::pthread_t, sig: c_int, value: crate::sigval) -> c_int;
    pub fn mallinfo() -> crate::mallinfo;
    pub fn mallinfo2() -> crate::mallinfo2;
    pub fn malloc_stats();
    pub fn malloc_info(options: c_int, stream: *mut crate::FILE) -> c_int;
    pub fn malloc_usable_size(ptr: *mut c_void) -> size_t;
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

    pub fn sethostid(hostid: c_long) -> c_int;

    pub fn memfd_create(name: *const c_char, flags: c_uint) -> c_int;
    pub fn mlock2(addr: *const c_void, len: size_t, flags: c_uint) -> c_int;

    pub fn euidaccess(pathname: *const c_char, mode: c_int) -> c_int;
    pub fn eaccess(pathname: *const c_char, mode: c_int) -> c_int;

    pub fn asctime_r(tm: *const crate::tm, buf: *mut c_char) -> *mut c_char;
    #[cfg_attr(gnu_time_bits64, link_name = "__ctime64_r")]
    pub fn ctime_r(timep: *const time_t, buf: *mut c_char) -> *mut c_char;

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
    pub fn malloc_trim(__pad: size_t) -> c_int;
    pub fn gnu_get_libc_release() -> *const c_char;
    pub fn gnu_get_libc_version() -> *const c_char;

    // posix/spawn.h
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

    // mntent.h
    pub fn getmntent_r(
        stream: *mut crate::FILE,
        mntbuf: *mut crate::mntent,
        buf: *mut c_char,
        buflen: c_int,
    ) -> *mut crate::mntent;

    pub fn execveat(
        dirfd: c_int,
        pathname: *const c_char,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
        flags: c_int,
    ) -> c_int;

    // Added in `glibc` 2.34
    pub fn close_range(first: c_uint, last: c_uint, flags: c_int) -> c_int;

    pub fn mq_notify(mqdes: crate::mqd_t, sevp: *const crate::sigevent) -> c_int;

    #[cfg_attr(gnu_time_bits64, link_name = "__epoll_pwait2_time64")]
    pub fn epoll_pwait2(
        epfd: c_int,
        events: *mut crate::epoll_event,
        maxevents: c_int,
        timeout: *const crate::timespec,
        sigmask: *const crate::sigset_t,
    ) -> c_int;

    pub fn mempcpy(dest: *mut c_void, src: *const c_void, n: size_t) -> *mut c_void;
}

cfg_if! {
    if #[cfg(any(
        target_arch = "x86",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "csky",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "powerpc",
        target_arch = "sparc",
        target_arch = "riscv32"
    ))] {
        mod b32;
        pub use self::b32::*;
    } else if #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "s390x",
        target_arch = "sparc64",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    ))] {
        mod b64;
        pub use self::b64::*;
    } else {
        // Unknown target_arch
    }
}
