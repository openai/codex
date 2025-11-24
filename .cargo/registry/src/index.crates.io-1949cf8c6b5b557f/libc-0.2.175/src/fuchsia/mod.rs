//! Definitions found commonly among almost all Unix derivatives
//!
//! More functions and definitions can be found in the more specific modules
//! according to the platform in question.

use crate::prelude::*;

// PUB_TYPE

pub type intmax_t = i64;
pub type uintmax_t = u64;

pub type locale_t = *mut c_void;

pub type size_t = usize;
pub type ptrdiff_t = isize;
pub type intptr_t = isize;
pub type uintptr_t = usize;
pub type ssize_t = isize;

pub type pid_t = i32;
pub type uid_t = u32;
pub type gid_t = u32;
pub type in_addr_t = u32;
pub type in_port_t = u16;
pub type sighandler_t = size_t;
pub type cc_t = c_uchar;
pub type sa_family_t = u16;
pub type pthread_key_t = c_uint;
pub type speed_t = c_uint;
pub type tcflag_t = c_uint;
pub type clockid_t = c_int;
pub type key_t = c_int;
pub type id_t = c_uint;
pub type useconds_t = u32;
pub type dev_t = u64;
pub type socklen_t = u32;
pub type pthread_t = c_ulong;
pub type mode_t = u32;
pub type ino64_t = u64;
pub type off64_t = i64;
pub type blkcnt64_t = i64;
pub type rlim64_t = u64;
pub type mqd_t = c_int;
pub type nfds_t = c_ulong;
pub type nl_item = c_int;
pub type idtype_t = c_uint;
pub type loff_t = c_longlong;

pub type __u8 = c_uchar;
pub type __u16 = c_ushort;
pub type __s16 = c_short;
pub type __u32 = c_uint;
pub type __s32 = c_int;

pub type Elf32_Half = u16;
pub type Elf32_Word = u32;
pub type Elf32_Off = u32;
pub type Elf32_Addr = u32;

pub type Elf64_Half = u16;
pub type Elf64_Word = u32;
pub type Elf64_Off = u64;
pub type Elf64_Addr = u64;
pub type Elf64_Xword = u64;

pub type clock_t = c_long;
pub type time_t = c_long;
pub type suseconds_t = c_long;
pub type ino_t = u64;
pub type off_t = i64;
pub type blkcnt_t = i64;

pub type shmatt_t = c_ulong;
pub type msgqnum_t = c_ulong;
pub type msglen_t = c_ulong;
pub type fsblkcnt_t = c_ulonglong;
pub type fsfilcnt_t = c_ulonglong;
pub type rlim_t = c_ulonglong;

// FIXME(fuchsia): why are these uninhabited types? that seems... wrong?
// Presumably these should be `()` or an `extern type` (when that stabilizes).
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum timezone {}
impl Copy for timezone {}
impl Clone for timezone {
    fn clone(&self) -> timezone {
        *self
    }
}
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum DIR {}
impl Copy for DIR {}
impl Clone for DIR {
    fn clone(&self) -> DIR {
        *self
    }
}

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum fpos64_t {} // FIXME(fuchsia): fill this out with a struct
impl Copy for fpos64_t {}
impl Clone for fpos64_t {
    fn clone(&self) -> fpos64_t {
        *self
    }
}

// PUB_STRUCT

s! {
    pub struct group {
        pub gr_name: *mut c_char,
        pub gr_passwd: *mut c_char,
        pub gr_gid: crate::gid_t,
        pub gr_mem: *mut *mut c_char,
    }

    pub struct utimbuf {
        pub actime: time_t,
        pub modtime: time_t,
    }

    pub struct timeval {
        pub tv_sec: time_t,
        pub tv_usec: suseconds_t,
    }

    pub struct timespec {
        pub tv_sec: time_t,
        pub tv_nsec: c_long,
    }

    // FIXME(fuchsia): the rlimit and rusage related functions and types don't exist
    // within zircon. Are there reasons for keeping them around?
    pub struct rlimit {
        pub rlim_cur: rlim_t,
        pub rlim_max: rlim_t,
    }

    pub struct rusage {
        pub ru_utime: timeval,
        pub ru_stime: timeval,
        pub ru_maxrss: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad1: u32,
        pub ru_ixrss: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad2: u32,
        pub ru_idrss: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad3: u32,
        pub ru_isrss: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad4: u32,
        pub ru_minflt: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad5: u32,
        pub ru_majflt: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad6: u32,
        pub ru_nswap: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad7: u32,
        pub ru_inblock: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad8: u32,
        pub ru_oublock: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad9: u32,
        pub ru_msgsnd: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad10: u32,
        pub ru_msgrcv: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad11: u32,
        pub ru_nsignals: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad12: u32,
        pub ru_nvcsw: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad13: u32,
        pub ru_nivcsw: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        __pad14: u32,
    }

    pub struct in_addr {
        pub s_addr: in_addr_t,
    }

    pub struct in6_addr {
        pub s6_addr: [u8; 16],
    }

    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct ip_mreqn {
        pub imr_multiaddr: in_addr,
        pub imr_address: in_addr,
        pub imr_ifindex: c_int,
    }

    pub struct ipv6_mreq {
        pub ipv6mr_multiaddr: in6_addr,
        pub ipv6mr_interface: c_uint,
    }

    pub struct hostent {
        pub h_name: *mut c_char,
        pub h_aliases: *mut *mut c_char,
        pub h_addrtype: c_int,
        pub h_length: c_int,
        pub h_addr_list: *mut *mut c_char,
    }

    pub struct iovec {
        pub iov_base: *mut c_void,
        pub iov_len: size_t,
    }

    pub struct pollfd {
        pub fd: c_int,
        pub events: c_short,
        pub revents: c_short,
    }

    pub struct winsize {
        pub ws_row: c_ushort,
        pub ws_col: c_ushort,
        pub ws_xpixel: c_ushort,
        pub ws_ypixel: c_ushort,
    }

    pub struct linger {
        pub l_onoff: c_int,
        pub l_linger: c_int,
    }

    pub struct sigval {
        // Actually a union of an int and a void*
        pub sival_ptr: *mut c_void,
    }

    // <sys/time.h>
    pub struct itimerval {
        pub it_interval: crate::timeval,
        pub it_value: crate::timeval,
    }

    // <sys/times.h>
    pub struct tms {
        pub tms_utime: crate::clock_t,
        pub tms_stime: crate::clock_t,
        pub tms_cutime: crate::clock_t,
        pub tms_cstime: crate::clock_t,
    }

    pub struct servent {
        pub s_name: *mut c_char,
        pub s_aliases: *mut *mut c_char,
        pub s_port: c_int,
        pub s_proto: *mut c_char,
    }

    pub struct protoent {
        pub p_name: *mut c_char,
        pub p_aliases: *mut *mut c_char,
        pub p_proto: c_int,
    }

    pub struct aiocb {
        pub aio_fildes: c_int,
        pub aio_lio_opcode: c_int,
        pub aio_reqprio: c_int,
        pub aio_buf: *mut c_void,
        pub aio_nbytes: size_t,
        pub aio_sigevent: crate::sigevent,
        __td: *mut c_void,
        __lock: [c_int; 2],
        __err: c_int,
        __ret: ssize_t,
        pub aio_offset: off_t,
        __next: *mut c_void,
        __prev: *mut c_void,
        #[cfg(target_pointer_width = "32")]
        __dummy4: [c_char; 24],
        #[cfg(target_pointer_width = "64")]
        __dummy4: [c_char; 16],
    }

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct sigaction {
        pub sa_sigaction: crate::sighandler_t,
        pub sa_mask: crate::sigset_t,
        pub sa_flags: c_int,
        pub sa_restorer: Option<extern "C" fn()>,
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

    pub struct ucred {
        pub pid: crate::pid_t,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
    }

    pub struct sockaddr {
        pub sa_family: sa_family_t,
        pub sa_data: [c_char; 14],
    }

    pub struct sockaddr_in {
        pub sin_family: sa_family_t,
        pub sin_port: crate::in_port_t,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [u8; 8],
    }

    pub struct sockaddr_in6 {
        pub sin6_family: sa_family_t,
        pub sin6_port: crate::in_port_t,
        pub sin6_flowinfo: u32,
        pub sin6_addr: crate::in6_addr,
        pub sin6_scope_id: u32,
    }

    pub struct sockaddr_vm {
        pub svm_family: sa_family_t,
        pub svm_reserved1: c_ushort,
        pub svm_port: crate::in_port_t,
        pub svm_cid: c_uint,
        pub svm_zero: [u8; 4],
    }

    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        pub ai_addrlen: socklen_t,

        pub ai_addr: *mut crate::sockaddr,

        pub ai_canonname: *mut c_char,

        pub ai_next: *mut addrinfo,
    }

    pub struct sockaddr_ll {
        pub sll_family: c_ushort,
        pub sll_protocol: c_ushort,
        pub sll_ifindex: c_int,
        pub sll_hatype: c_ushort,
        pub sll_pkttype: c_uchar,
        pub sll_halen: c_uchar,
        pub sll_addr: [c_uchar; 8],
    }

    pub struct fd_set {
        fds_bits: [c_ulong; FD_SETSIZE as usize / ULONG_SIZE],
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

    pub struct sched_param {
        pub sched_priority: c_int,
        pub sched_ss_low_priority: c_int,
        pub sched_ss_repl_period: crate::timespec,
        pub sched_ss_init_budget: crate::timespec,
        pub sched_ss_max_repl: c_int,
    }

    pub struct Dl_info {
        pub dli_fname: *const c_char,
        pub dli_fbase: *mut c_void,
        pub dli_sname: *const c_char,
        pub dli_saddr: *mut c_void,
    }

    pub struct epoll_event {
        pub events: u32,
        pub u64: u64,
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

    pub struct rlimit64 {
        pub rlim_cur: rlim64_t,
        pub rlim_max: rlim64_t,
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

    pub struct ifaddrs {
        pub ifa_next: *mut ifaddrs,
        pub ifa_name: *mut c_char,
        pub ifa_flags: c_uint,
        pub ifa_addr: *mut crate::sockaddr,
        pub ifa_netmask: *mut crate::sockaddr,
        pub ifa_ifu: *mut crate::sockaddr, // FIXME(union) This should be a union
        pub ifa_data: *mut c_void,
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
        #[cfg(target_endian = "little")]
        pub f_fsid: c_ulong,
        #[cfg(all(target_pointer_width = "32", not(target_arch = "x86_64")))]
        __f_unused: c_int,
        #[cfg(target_endian = "big")]
        pub f_fsid: c_ulong,
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
        __f_spare: [c_int; 6],
    }

    pub struct dqblk {
        pub dqb_bhardlimit: u64,
        pub dqb_bsoftlimit: u64,
        pub dqb_curspace: u64,
        pub dqb_ihardlimit: u64,
        pub dqb_isoftlimit: u64,
        pub dqb_curinodes: u64,
        pub dqb_btime: u64,
        pub dqb_itime: u64,
        pub dqb_valid: u32,
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

    pub struct itimerspec {
        pub it_interval: crate::timespec,
        pub it_value: crate::timespec,
    }

    pub struct fsid_t {
        __val: [c_int; 2],
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

    pub struct mmsghdr {
        pub msg_hdr: crate::msghdr,
        pub msg_len: c_uint,
    }

    pub struct sembuf {
        pub sem_num: c_ushort,
        pub sem_op: c_short,
        pub sem_flg: c_short,
    }

    pub struct input_event {
        pub time: crate::timeval,
        pub type_: crate::__u16,
        pub code: crate::__u16,
        pub value: crate::__s32,
    }

    pub struct input_id {
        pub bustype: crate::__u16,
        pub vendor: crate::__u16,
        pub product: crate::__u16,
        pub version: crate::__u16,
    }

    pub struct input_absinfo {
        pub value: crate::__s32,
        pub minimum: crate::__s32,
        pub maximum: crate::__s32,
        pub fuzz: crate::__s32,
        pub flat: crate::__s32,
        pub resolution: crate::__s32,
    }

    pub struct input_keymap_entry {
        pub flags: crate::__u8,
        pub len: crate::__u8,
        pub index: crate::__u16,
        pub keycode: crate::__u32,
        pub scancode: [crate::__u8; 32],
    }

    pub struct input_mask {
        pub type_: crate::__u32,
        pub codes_size: crate::__u32,
        pub codes_ptr: crate::__u64,
    }

    pub struct ff_replay {
        pub length: crate::__u16,
        pub delay: crate::__u16,
    }

    pub struct ff_trigger {
        pub button: crate::__u16,
        pub interval: crate::__u16,
    }

    pub struct ff_envelope {
        pub attack_length: crate::__u16,
        pub attack_level: crate::__u16,
        pub fade_length: crate::__u16,
        pub fade_level: crate::__u16,
    }

    pub struct ff_constant_effect {
        pub level: crate::__s16,
        pub envelope: ff_envelope,
    }

    pub struct ff_ramp_effect {
        pub start_level: crate::__s16,
        pub end_level: crate::__s16,
        pub envelope: ff_envelope,
    }

    pub struct ff_condition_effect {
        pub right_saturation: crate::__u16,
        pub left_saturation: crate::__u16,

        pub right_coeff: crate::__s16,
        pub left_coeff: crate::__s16,

        pub deadband: crate::__u16,
        pub center: crate::__s16,
    }

    pub struct ff_periodic_effect {
        pub waveform: crate::__u16,
        pub period: crate::__u16,
        pub magnitude: crate::__s16,
        pub offset: crate::__s16,
        pub phase: crate::__u16,

        pub envelope: ff_envelope,

        pub custom_len: crate::__u32,
        pub custom_data: *mut crate::__s16,
    }

    pub struct ff_rumble_effect {
        pub strong_magnitude: crate::__u16,
        pub weak_magnitude: crate::__u16,
    }

    pub struct ff_effect {
        pub type_: crate::__u16,
        pub id: crate::__s16,
        pub direction: crate::__u16,
        pub trigger: ff_trigger,
        pub replay: ff_replay,
        // FIXME(1.0): this is actually a union
        #[cfg(target_pointer_width = "64")]
        pub u: [u64; 4],
        #[cfg(target_pointer_width = "32")]
        pub u: [u32; 7],
    }

    pub struct dl_phdr_info {
        #[cfg(target_pointer_width = "64")]
        pub dlpi_addr: Elf64_Addr,
        #[cfg(target_pointer_width = "32")]
        pub dlpi_addr: Elf32_Addr,

        pub dlpi_name: *const c_char,

        #[cfg(target_pointer_width = "64")]
        pub dlpi_phdr: *const Elf64_Phdr,
        #[cfg(target_pointer_width = "32")]
        pub dlpi_phdr: *const Elf32_Phdr,

        #[cfg(target_pointer_width = "64")]
        pub dlpi_phnum: Elf64_Half,
        #[cfg(target_pointer_width = "32")]
        pub dlpi_phnum: Elf32_Half,

        pub dlpi_adds: c_ulonglong,
        pub dlpi_subs: c_ulonglong,
        pub dlpi_tls_modid: size_t,
        pub dlpi_tls_data: *mut c_void,
    }

    pub struct Elf32_Phdr {
        pub p_type: Elf32_Word,
        pub p_offset: Elf32_Off,
        pub p_vaddr: Elf32_Addr,
        pub p_paddr: Elf32_Addr,
        pub p_filesz: Elf32_Word,
        pub p_memsz: Elf32_Word,
        pub p_flags: Elf32_Word,
        pub p_align: Elf32_Word,
    }

    pub struct Elf64_Phdr {
        pub p_type: Elf64_Word,
        pub p_flags: Elf64_Word,
        pub p_offset: Elf64_Off,
        pub p_vaddr: Elf64_Addr,
        pub p_paddr: Elf64_Addr,
        pub p_filesz: Elf64_Xword,
        pub p_memsz: Elf64_Xword,
        pub p_align: Elf64_Xword,
    }

    pub struct statfs64 {
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

    pub struct statvfs64 {
        pub f_bsize: c_ulong,
        pub f_frsize: c_ulong,
        pub f_blocks: u64,
        pub f_bfree: u64,
        pub f_bavail: u64,
        pub f_files: u64,
        pub f_ffree: u64,
        pub f_favail: u64,
        pub f_fsid: c_ulong,
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
        __f_spare: [c_int; 6],
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_flags: c_int,
        pub ss_size: size_t,
    }

    pub struct pthread_attr_t {
        __size: [u64; 7],
    }

    pub struct sigset_t {
        __val: [c_ulong; 16],
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

    pub struct msghdr {
        pub msg_name: *mut c_void,
        pub msg_namelen: crate::socklen_t,
        pub msg_iov: *mut crate::iovec,
        pub msg_iovlen: c_int,
        __pad1: c_int,
        pub msg_control: *mut c_void,
        pub msg_controllen: crate::socklen_t,
        __pad2: crate::socklen_t,
        pub msg_flags: c_int,
    }

    pub struct cmsghdr {
        pub cmsg_len: crate::socklen_t,
        pub __pad1: c_int,
        pub cmsg_level: c_int,
        pub cmsg_type: c_int,
    }

    pub struct sem_t {
        __val: [c_int; 8],
    }

    pub struct siginfo_t {
        pub si_signo: c_int,
        pub si_errno: c_int,
        pub si_code: c_int,
        pub _pad: [c_int; 29],
        _align: [usize; 0],
    }

    pub struct termios2 {
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_line: crate::cc_t,
        pub c_cc: [crate::cc_t; 19],
        pub c_ispeed: crate::speed_t,
        pub c_ospeed: crate::speed_t,
    }

    pub struct in6_pktinfo {
        pub ipi6_addr: crate::in6_addr,
        pub ipi6_ifindex: c_uint,
    }

    #[cfg_attr(
        any(target_pointer_width = "32", target_arch = "x86_64"),
        repr(align(4))
    )]
    #[cfg_attr(
        not(any(target_pointer_width = "32", target_arch = "x86_64")),
        repr(align(8))
    )]
    pub struct pthread_mutexattr_t {
        size: [u8; crate::__SIZEOF_PTHREAD_MUTEXATTR_T],
    }

    #[cfg_attr(target_pointer_width = "32", repr(align(4)))]
    #[cfg_attr(target_pointer_width = "64", repr(align(8)))]
    pub struct pthread_rwlockattr_t {
        size: [u8; crate::__SIZEOF_PTHREAD_RWLOCKATTR_T],
    }

    #[repr(align(4))]
    pub struct pthread_condattr_t {
        size: [u8; crate::__SIZEOF_PTHREAD_CONDATTR_T],
    }
}

s_no_extra_traits! {
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

    pub struct sockaddr_un {
        pub sun_family: sa_family_t,
        pub sun_path: [c_char; 108],
    }

    pub struct sockaddr_storage {
        pub ss_family: sa_family_t,
        __ss_pad2: [u8; 128 - 2 - 8],
        __ss_align: size_t,
    }

    pub struct utsname {
        pub sysname: [c_char; 65],
        pub nodename: [c_char; 65],
        pub release: [c_char; 65],
        pub version: [c_char; 65],
        pub machine: [c_char; 65],
        pub domainname: [c_char; 65],
    }

    pub struct dirent {
        pub d_ino: crate::ino_t,
        pub d_off: off_t,
        pub d_reclen: c_ushort,
        pub d_type: c_uchar,
        pub d_name: [c_char; 256],
    }

    pub struct dirent64 {
        pub d_ino: crate::ino64_t,
        pub d_off: off64_t,
        pub d_reclen: c_ushort,
        pub d_type: c_uchar,
        pub d_name: [c_char; 256],
    }

    // x32 compatibility
    // See https://sourceware.org/bugzilla/show_bug.cgi?id=21279
    pub struct mq_attr {
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub mq_flags: i64,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub mq_maxmsg: i64,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub mq_msgsize: i64,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub mq_curmsgs: i64,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pad: [i64; 4],

        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub mq_flags: c_long,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub mq_maxmsg: c_long,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub mq_msgsize: c_long,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub mq_curmsgs: c_long,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pad: [c_long; 4],
    }

    pub struct sockaddr_nl {
        pub nl_family: crate::sa_family_t,
        nl_pad: c_ushort,
        pub nl_pid: u32,
        pub nl_groups: u32,
    }

    pub struct sigevent {
        pub sigev_value: crate::sigval,
        pub sigev_signo: c_int,
        pub sigev_notify: c_int,
        pub sigev_notify_function: fn(crate::sigval),
        pub sigev_notify_attributes: *mut pthread_attr_t,
        pub __pad: [c_char; 56 - 3 * 8],
    }

    #[cfg_attr(
        all(
            target_pointer_width = "32",
            any(target_arch = "arm", target_arch = "x86_64")
        ),
        repr(align(4))
    )]
    #[cfg_attr(
        any(
            target_pointer_width = "64",
            not(any(target_arch = "arm", target_arch = "x86_64"))
        ),
        repr(align(8))
    )]
    pub struct pthread_mutex_t {
        size: [u8; crate::__SIZEOF_PTHREAD_MUTEX_T],
    }

    #[cfg_attr(
        all(
            target_pointer_width = "32",
            any(target_arch = "arm", target_arch = "x86_64")
        ),
        repr(align(4))
    )]
    #[cfg_attr(
        any(
            target_pointer_width = "64",
            not(any(target_arch = "arm", target_arch = "x86_64"))
        ),
        repr(align(8))
    )]
    pub struct pthread_rwlock_t {
        size: [u8; crate::__SIZEOF_PTHREAD_RWLOCK_T],
    }

    #[cfg_attr(target_pointer_width = "32", repr(align(4)))]
    #[cfg_attr(target_pointer_width = "64", repr(align(8)))]
    #[cfg_attr(target_arch = "x86", repr(align(4)))]
    #[cfg_attr(not(target_arch = "x86"), repr(align(8)))]
    pub struct pthread_cond_t {
        size: [u8; crate::__SIZEOF_PTHREAD_COND_T],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
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

        impl PartialEq for sockaddr_un {
            fn eq(&self, other: &sockaddr_un) -> bool {
                self.sun_family == other.sun_family
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
                self.sun_family.hash(state);
                self.sun_path.hash(state);
            }
        }

        impl PartialEq for sockaddr_storage {
            fn eq(&self, other: &sockaddr_storage) -> bool {
                self.ss_family == other.ss_family
                    && self.__ss_align == other.__ss_align
                    && self
                        .__ss_pad2
                        .iter()
                        .zip(other.__ss_pad2.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for sockaddr_storage {}
        impl hash::Hash for sockaddr_storage {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ss_family.hash(state);
                self.__ss_align.hash(state);
                self.__ss_pad2.hash(state);
            }
        }

        impl PartialEq for utsname {
            fn eq(&self, other: &utsname) -> bool {
                self.sysname
                    .iter()
                    .zip(other.sysname.iter())
                    .all(|(a, b)| a == b)
                    && self
                        .nodename
                        .iter()
                        .zip(other.nodename.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .release
                        .iter()
                        .zip(other.release.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .version
                        .iter()
                        .zip(other.version.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .machine
                        .iter()
                        .zip(other.machine.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for utsname {}
        impl hash::Hash for utsname {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sysname.hash(state);
                self.nodename.hash(state);
                self.release.hash(state);
                self.version.hash(state);
                self.machine.hash(state);
            }
        }

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

        impl PartialEq for dirent64 {
            fn eq(&self, other: &dirent64) -> bool {
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
        impl Eq for dirent64 {}
        impl hash::Hash for dirent64 {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.d_ino.hash(state);
                self.d_off.hash(state);
                self.d_reclen.hash(state);
                self.d_type.hash(state);
                self.d_name.hash(state);
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

        impl PartialEq for sockaddr_nl {
            fn eq(&self, other: &sockaddr_nl) -> bool {
                self.nl_family == other.nl_family
                    && self.nl_pid == other.nl_pid
                    && self.nl_groups == other.nl_groups
            }
        }
        impl Eq for sockaddr_nl {}
        impl hash::Hash for sockaddr_nl {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.nl_family.hash(state);
                self.nl_pid.hash(state);
                self.nl_groups.hash(state);
            }
        }

        // FIXME(msrv): suggested method was added in 1.85
        #[allow(unpredictable_function_pointer_comparisons)]
        impl PartialEq for sigevent {
            fn eq(&self, other: &sigevent) -> bool {
                self.sigev_value == other.sigev_value
                    && self.sigev_signo == other.sigev_signo
                    && self.sigev_notify == other.sigev_notify
                    && self.sigev_notify_function == other.sigev_notify_function
                    && self.sigev_notify_attributes == other.sigev_notify_attributes
            }
        }
        impl Eq for sigevent {}
        impl hash::Hash for sigevent {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sigev_value.hash(state);
                self.sigev_signo.hash(state);
                self.sigev_notify.hash(state);
                self.sigev_notify_function.hash(state);
                self.sigev_notify_attributes.hash(state);
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

        impl PartialEq for pthread_mutex_t {
            fn eq(&self, other: &pthread_mutex_t) -> bool {
                self.size.iter().zip(other.size.iter()).all(|(a, b)| a == b)
            }
        }
        impl Eq for pthread_mutex_t {}
        impl hash::Hash for pthread_mutex_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.size.hash(state);
            }
        }

        impl PartialEq for pthread_rwlock_t {
            fn eq(&self, other: &pthread_rwlock_t) -> bool {
                self.size.iter().zip(other.size.iter()).all(|(a, b)| a == b)
            }
        }
        impl Eq for pthread_rwlock_t {}
        impl hash::Hash for pthread_rwlock_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.size.hash(state);
            }
        }
    }
}

// PUB_CONST

pub const INT_MIN: c_int = -2147483648;
pub const INT_MAX: c_int = 2147483647;

pub const SIG_DFL: sighandler_t = 0 as sighandler_t;
pub const SIG_IGN: sighandler_t = 1 as sighandler_t;
pub const SIG_ERR: sighandler_t = !0 as sighandler_t;

pub const DT_UNKNOWN: u8 = 0;
pub const DT_FIFO: u8 = 1;
pub const DT_CHR: u8 = 2;
pub const DT_DIR: u8 = 4;
pub const DT_BLK: u8 = 6;
pub const DT_REG: u8 = 8;
pub const DT_LNK: u8 = 10;
pub const DT_SOCK: u8 = 12;

pub const FD_CLOEXEC: c_int = 0x1;

pub const USRQUOTA: c_int = 0;
pub const GRPQUOTA: c_int = 1;

pub const SIGIOT: c_int = 6;

pub const S_ISUID: mode_t = 0o4000;
pub const S_ISGID: mode_t = 0o2000;
pub const S_ISVTX: mode_t = 0o1000;

pub const IF_NAMESIZE: size_t = 16;
pub const IFNAMSIZ: size_t = IF_NAMESIZE;

pub const LOG_EMERG: c_int = 0;
pub const LOG_ALERT: c_int = 1;
pub const LOG_CRIT: c_int = 2;
pub const LOG_ERR: c_int = 3;
pub const LOG_WARNING: c_int = 4;
pub const LOG_NOTICE: c_int = 5;
pub const LOG_INFO: c_int = 6;
pub const LOG_DEBUG: c_int = 7;

pub const LOG_KERN: c_int = 0;
pub const LOG_USER: c_int = 1 << 3;
pub const LOG_MAIL: c_int = 2 << 3;
pub const LOG_DAEMON: c_int = 3 << 3;
pub const LOG_AUTH: c_int = 4 << 3;
pub const LOG_SYSLOG: c_int = 5 << 3;
pub const LOG_LPR: c_int = 6 << 3;
pub const LOG_NEWS: c_int = 7 << 3;
pub const LOG_UUCP: c_int = 8 << 3;
pub const LOG_LOCAL0: c_int = 16 << 3;
pub const LOG_LOCAL1: c_int = 17 << 3;
pub const LOG_LOCAL2: c_int = 18 << 3;
pub const LOG_LOCAL3: c_int = 19 << 3;
pub const LOG_LOCAL4: c_int = 20 << 3;
pub const LOG_LOCAL5: c_int = 21 << 3;
pub const LOG_LOCAL6: c_int = 22 << 3;
pub const LOG_LOCAL7: c_int = 23 << 3;

pub const LOG_PID: c_int = 0x01;
pub const LOG_CONS: c_int = 0x02;
pub const LOG_ODELAY: c_int = 0x04;
pub const LOG_NDELAY: c_int = 0x08;
pub const LOG_NOWAIT: c_int = 0x10;

pub const LOG_PRIMASK: c_int = 7;
pub const LOG_FACMASK: c_int = 0x3f8;

pub const PRIO_PROCESS: c_int = 0;
pub const PRIO_PGRP: c_int = 1;
pub const PRIO_USER: c_int = 2;

pub const PRIO_MIN: c_int = -20;
pub const PRIO_MAX: c_int = 20;

pub const IPPROTO_ICMP: c_int = 1;
pub const IPPROTO_ICMPV6: c_int = 58;
pub const IPPROTO_TCP: c_int = 6;
pub const IPPROTO_UDP: c_int = 17;
pub const IPPROTO_IP: c_int = 0;
pub const IPPROTO_IPV6: c_int = 41;

pub const INADDR_LOOPBACK: in_addr_t = 2130706433;
pub const INADDR_ANY: in_addr_t = 0;
pub const INADDR_BROADCAST: in_addr_t = 4294967295;
pub const INADDR_NONE: in_addr_t = 4294967295;

pub const EXIT_FAILURE: c_int = 1;
pub const EXIT_SUCCESS: c_int = 0;
pub const RAND_MAX: c_int = 2147483647;
pub const EOF: c_int = -1;
pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;
pub const _IOFBF: c_int = 0;
pub const _IONBF: c_int = 2;
pub const _IOLBF: c_int = 1;

pub const F_DUPFD: c_int = 0;
pub const F_GETFD: c_int = 1;
pub const F_SETFD: c_int = 2;
pub const F_GETFL: c_int = 3;
pub const F_SETFL: c_int = 4;

// Linux-specific fcntls
pub const F_SETLEASE: c_int = 1024;
pub const F_GETLEASE: c_int = 1025;
pub const F_NOTIFY: c_int = 1026;
pub const F_CANCELLK: c_int = 1029;
pub const F_DUPFD_CLOEXEC: c_int = 1030;
pub const F_SETPIPE_SZ: c_int = 1031;
pub const F_GETPIPE_SZ: c_int = 1032;
pub const F_ADD_SEALS: c_int = 1033;
pub const F_GET_SEALS: c_int = 1034;

pub const F_SEAL_SEAL: c_int = 0x0001;
pub const F_SEAL_SHRINK: c_int = 0x0002;
pub const F_SEAL_GROW: c_int = 0x0004;
pub const F_SEAL_WRITE: c_int = 0x0008;

// FIXME(#235): Include file sealing fcntls once we have a way to verify them.

pub const SIGTRAP: c_int = 5;

pub const PTHREAD_CREATE_JOINABLE: c_int = 0;
pub const PTHREAD_CREATE_DETACHED: c_int = 1;

pub const CLOCK_REALTIME: crate::clockid_t = 0;
pub const CLOCK_MONOTONIC: crate::clockid_t = 1;
pub const CLOCK_PROCESS_CPUTIME_ID: crate::clockid_t = 2;
pub const CLOCK_THREAD_CPUTIME_ID: crate::clockid_t = 3;
pub const CLOCK_MONOTONIC_RAW: crate::clockid_t = 4;
pub const CLOCK_REALTIME_COARSE: crate::clockid_t = 5;
pub const CLOCK_MONOTONIC_COARSE: crate::clockid_t = 6;
pub const CLOCK_BOOTTIME: crate::clockid_t = 7;
pub const CLOCK_REALTIME_ALARM: crate::clockid_t = 8;
pub const CLOCK_BOOTTIME_ALARM: crate::clockid_t = 9;
pub const CLOCK_SGI_CYCLE: crate::clockid_t = 10;
pub const CLOCK_TAI: crate::clockid_t = 11;
pub const TIMER_ABSTIME: c_int = 1;

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

pub const RUSAGE_SELF: c_int = 0;

pub const O_RDONLY: c_int = 0;
pub const O_WRONLY: c_int = 1;
pub const O_RDWR: c_int = 2;

pub const S_IFIFO: mode_t = 0o1_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_IFMT: mode_t = 0o17_0000;
pub const S_IRWXU: mode_t = 0o0700;
pub const S_IXUSR: mode_t = 0o0100;
pub const S_IWUSR: mode_t = 0o0200;
pub const S_IRUSR: mode_t = 0o0400;
pub const S_IRWXG: mode_t = 0o0070;
pub const S_IXGRP: mode_t = 0o0010;
pub const S_IWGRP: mode_t = 0o0020;
pub const S_IRGRP: mode_t = 0o0040;
pub const S_IRWXO: mode_t = 0o0007;
pub const S_IXOTH: mode_t = 0o0001;
pub const S_IWOTH: mode_t = 0o0002;
pub const S_IROTH: mode_t = 0o0004;
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
pub const SIGABRT: c_int = 6;
pub const SIGFPE: c_int = 8;
pub const SIGKILL: c_int = 9;
pub const SIGSEGV: c_int = 11;
pub const SIGPIPE: c_int = 13;
pub const SIGALRM: c_int = 14;
pub const SIGTERM: c_int = 15;

pub const PROT_NONE: c_int = 0;
pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const PROT_EXEC: c_int = 4;

pub const LC_CTYPE: c_int = 0;
pub const LC_NUMERIC: c_int = 1;
pub const LC_TIME: c_int = 2;
pub const LC_COLLATE: c_int = 3;
pub const LC_MONETARY: c_int = 4;
pub const LC_MESSAGES: c_int = 5;
pub const LC_ALL: c_int = 6;
pub const LC_CTYPE_MASK: c_int = 1 << LC_CTYPE;
pub const LC_NUMERIC_MASK: c_int = 1 << LC_NUMERIC;
pub const LC_TIME_MASK: c_int = 1 << LC_TIME;
pub const LC_COLLATE_MASK: c_int = 1 << LC_COLLATE;
pub const LC_MONETARY_MASK: c_int = 1 << LC_MONETARY;
pub const LC_MESSAGES_MASK: c_int = 1 << LC_MESSAGES;
// LC_ALL_MASK defined per platform

pub const MAP_FILE: c_int = 0x0000;
pub const MAP_SHARED: c_int = 0x0001;
pub const MAP_PRIVATE: c_int = 0x0002;
pub const MAP_FIXED: c_int = 0x0010;

pub const MAP_FAILED: *mut c_void = !0 as *mut c_void;

// MS_ flags for msync(2)
pub const MS_ASYNC: c_int = 0x0001;
pub const MS_INVALIDATE: c_int = 0x0002;
pub const MS_SYNC: c_int = 0x0004;

// MS_ flags for mount(2)
pub const MS_RDONLY: c_ulong = 0x01;
pub const MS_NOSUID: c_ulong = 0x02;
pub const MS_NODEV: c_ulong = 0x04;
pub const MS_NOEXEC: c_ulong = 0x08;
pub const MS_SYNCHRONOUS: c_ulong = 0x10;
pub const MS_REMOUNT: c_ulong = 0x20;
pub const MS_MANDLOCK: c_ulong = 0x40;
pub const MS_DIRSYNC: c_ulong = 0x80;
pub const MS_NOATIME: c_ulong = 0x0400;
pub const MS_NODIRATIME: c_ulong = 0x0800;
pub const MS_BIND: c_ulong = 0x1000;
pub const MS_MOVE: c_ulong = 0x2000;
pub const MS_REC: c_ulong = 0x4000;
pub const MS_SILENT: c_ulong = 0x8000;
pub const MS_POSIXACL: c_ulong = 0x010000;
pub const MS_UNBINDABLE: c_ulong = 0x020000;
pub const MS_PRIVATE: c_ulong = 0x040000;
pub const MS_SLAVE: c_ulong = 0x080000;
pub const MS_SHARED: c_ulong = 0x100000;
pub const MS_RELATIME: c_ulong = 0x200000;
pub const MS_KERNMOUNT: c_ulong = 0x400000;
pub const MS_I_VERSION: c_ulong = 0x800000;
pub const MS_STRICTATIME: c_ulong = 0x1000000;
pub const MS_ACTIVE: c_ulong = 0x40000000;
pub const MS_NOUSER: c_ulong = 0x80000000;
pub const MS_MGC_VAL: c_ulong = 0xc0ed0000;
pub const MS_MGC_MSK: c_ulong = 0xffff0000;
pub const MS_RMT_MASK: c_ulong = 0x800051;

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
pub const EWOULDBLOCK: c_int = EAGAIN;

pub const SCM_RIGHTS: c_int = 0x01;
pub const SCM_CREDENTIALS: c_int = 0x02;

pub const PROT_GROWSDOWN: c_int = 0x1000000;
pub const PROT_GROWSUP: c_int = 0x2000000;

pub const MAP_TYPE: c_int = 0x000f;

pub const MADV_NORMAL: c_int = 0;
pub const MADV_RANDOM: c_int = 1;
pub const MADV_SEQUENTIAL: c_int = 2;
pub const MADV_WILLNEED: c_int = 3;
pub const MADV_DONTNEED: c_int = 4;
pub const MADV_FREE: c_int = 8;
pub const MADV_REMOVE: c_int = 9;
pub const MADV_DONTFORK: c_int = 10;
pub const MADV_DOFORK: c_int = 11;
pub const MADV_MERGEABLE: c_int = 12;
pub const MADV_UNMERGEABLE: c_int = 13;
pub const MADV_HUGEPAGE: c_int = 14;
pub const MADV_NOHUGEPAGE: c_int = 15;
pub const MADV_DONTDUMP: c_int = 16;
pub const MADV_DODUMP: c_int = 17;
pub const MADV_HWPOISON: c_int = 100;
pub const MADV_SOFT_OFFLINE: c_int = 101;

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
pub const IFF_TUN: c_int = 0x0001;
pub const IFF_TAP: c_int = 0x0002;
pub const IFF_NO_PI: c_int = 0x1000;

pub const SOL_IP: c_int = 0;
pub const SOL_TCP: c_int = 6;
pub const SOL_UDP: c_int = 17;
pub const SOL_IPV6: c_int = 41;
pub const SOL_ICMPV6: c_int = 58;
pub const SOL_RAW: c_int = 255;
pub const SOL_DECNET: c_int = 261;
pub const SOL_X25: c_int = 262;
pub const SOL_PACKET: c_int = 263;
pub const SOL_ATM: c_int = 264;
pub const SOL_AAL: c_int = 265;
pub const SOL_IRDA: c_int = 266;
pub const SOL_NETBEUI: c_int = 267;
pub const SOL_LLC: c_int = 268;
pub const SOL_DCCP: c_int = 269;
pub const SOL_NETLINK: c_int = 270;
pub const SOL_TIPC: c_int = 271;

pub const AF_UNSPEC: c_int = 0;
pub const AF_UNIX: c_int = 1;
pub const AF_LOCAL: c_int = 1;
pub const AF_INET: c_int = 2;
pub const AF_AX25: c_int = 3;
pub const AF_IPX: c_int = 4;
pub const AF_APPLETALK: c_int = 5;
pub const AF_NETROM: c_int = 6;
pub const AF_BRIDGE: c_int = 7;
pub const AF_ATMPVC: c_int = 8;
pub const AF_X25: c_int = 9;
pub const AF_INET6: c_int = 10;
pub const AF_ROSE: c_int = 11;
pub const AF_DECnet: c_int = 12;
pub const AF_NETBEUI: c_int = 13;
pub const AF_SECURITY: c_int = 14;
pub const AF_KEY: c_int = 15;
pub const AF_NETLINK: c_int = 16;
pub const AF_ROUTE: c_int = AF_NETLINK;
pub const AF_PACKET: c_int = 17;
pub const AF_ASH: c_int = 18;
pub const AF_ECONET: c_int = 19;
pub const AF_ATMSVC: c_int = 20;
pub const AF_RDS: c_int = 21;
pub const AF_SNA: c_int = 22;
pub const AF_IRDA: c_int = 23;
pub const AF_PPPOX: c_int = 24;
pub const AF_WANPIPE: c_int = 25;
pub const AF_LLC: c_int = 26;
pub const AF_CAN: c_int = 29;
pub const AF_TIPC: c_int = 30;
pub const AF_BLUETOOTH: c_int = 31;
pub const AF_IUCV: c_int = 32;
pub const AF_RXRPC: c_int = 33;
pub const AF_ISDN: c_int = 34;
pub const AF_PHONET: c_int = 35;
pub const AF_IEEE802154: c_int = 36;
pub const AF_CAIF: c_int = 37;
pub const AF_ALG: c_int = 38;

pub const PF_UNSPEC: c_int = AF_UNSPEC;
pub const PF_UNIX: c_int = AF_UNIX;
pub const PF_LOCAL: c_int = AF_LOCAL;
pub const PF_INET: c_int = AF_INET;
pub const PF_AX25: c_int = AF_AX25;
pub const PF_IPX: c_int = AF_IPX;
pub const PF_APPLETALK: c_int = AF_APPLETALK;
pub const PF_NETROM: c_int = AF_NETROM;
pub const PF_BRIDGE: c_int = AF_BRIDGE;
pub const PF_ATMPVC: c_int = AF_ATMPVC;
pub const PF_X25: c_int = AF_X25;
pub const PF_INET6: c_int = AF_INET6;
pub const PF_ROSE: c_int = AF_ROSE;
pub const PF_DECnet: c_int = AF_DECnet;
pub const PF_NETBEUI: c_int = AF_NETBEUI;
pub const PF_SECURITY: c_int = AF_SECURITY;
pub const PF_KEY: c_int = AF_KEY;
pub const PF_NETLINK: c_int = AF_NETLINK;
pub const PF_ROUTE: c_int = AF_ROUTE;
pub const PF_PACKET: c_int = AF_PACKET;
pub const PF_ASH: c_int = AF_ASH;
pub const PF_ECONET: c_int = AF_ECONET;
pub const PF_ATMSVC: c_int = AF_ATMSVC;
pub const PF_RDS: c_int = AF_RDS;
pub const PF_SNA: c_int = AF_SNA;
pub const PF_IRDA: c_int = AF_IRDA;
pub const PF_PPPOX: c_int = AF_PPPOX;
pub const PF_WANPIPE: c_int = AF_WANPIPE;
pub const PF_LLC: c_int = AF_LLC;
pub const PF_CAN: c_int = AF_CAN;
pub const PF_TIPC: c_int = AF_TIPC;
pub const PF_BLUETOOTH: c_int = AF_BLUETOOTH;
pub const PF_IUCV: c_int = AF_IUCV;
pub const PF_RXRPC: c_int = AF_RXRPC;
pub const PF_ISDN: c_int = AF_ISDN;
pub const PF_PHONET: c_int = AF_PHONET;
pub const PF_IEEE802154: c_int = AF_IEEE802154;
pub const PF_CAIF: c_int = AF_CAIF;
pub const PF_ALG: c_int = AF_ALG;

pub const SOMAXCONN: c_int = 128;

pub const MSG_OOB: c_int = 1;
pub const MSG_PEEK: c_int = 2;
pub const MSG_DONTROUTE: c_int = 4;
pub const MSG_CTRUNC: c_int = 8;
pub const MSG_TRUNC: c_int = 0x20;
pub const MSG_DONTWAIT: c_int = 0x40;
pub const MSG_EOR: c_int = 0x80;
pub const MSG_WAITALL: c_int = 0x100;
pub const MSG_FIN: c_int = 0x200;
pub const MSG_SYN: c_int = 0x400;
pub const MSG_CONFIRM: c_int = 0x800;
pub const MSG_RST: c_int = 0x1000;
pub const MSG_ERRQUEUE: c_int = 0x2000;
pub const MSG_NOSIGNAL: c_int = 0x4000;
pub const MSG_MORE: c_int = 0x8000;
pub const MSG_WAITFORONE: c_int = 0x10000;
pub const MSG_FASTOPEN: c_int = 0x20000000;
pub const MSG_CMSG_CLOEXEC: c_int = 0x40000000;

pub const SCM_TIMESTAMP: c_int = SO_TIMESTAMP;

pub const SOCK_RAW: c_int = 3;
pub const SOCK_RDM: c_int = 4;

pub const IP_TOS: c_int = 1;
pub const IP_TTL: c_int = 2;
pub const IP_HDRINCL: c_int = 3;
pub const IP_RECVTOS: c_int = 13;
pub const IP_FREEBIND: c_int = 15;
pub const IP_TRANSPARENT: c_int = 19;
pub const IP_MULTICAST_IF: c_int = 32;
pub const IP_MULTICAST_TTL: c_int = 33;
pub const IP_MULTICAST_LOOP: c_int = 34;
pub const IP_ADD_MEMBERSHIP: c_int = 35;
pub const IP_DROP_MEMBERSHIP: c_int = 36;

pub const IPV6_UNICAST_HOPS: c_int = 16;
pub const IPV6_MULTICAST_IF: c_int = 17;
pub const IPV6_MULTICAST_HOPS: c_int = 18;
pub const IPV6_MULTICAST_LOOP: c_int = 19;
pub const IPV6_ADD_MEMBERSHIP: c_int = 20;
pub const IPV6_DROP_MEMBERSHIP: c_int = 21;
pub const IPV6_V6ONLY: c_int = 26;
pub const IPV6_RECVPKTINFO: c_int = 49;
pub const IPV6_RECVTCLASS: c_int = 66;
pub const IPV6_TCLASS: c_int = 67;

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

pub const SO_DEBUG: c_int = 1;

pub const SHUT_RD: c_int = 0;
pub const SHUT_WR: c_int = 1;
pub const SHUT_RDWR: c_int = 2;

pub const LOCK_SH: c_int = 1;
pub const LOCK_EX: c_int = 2;
pub const LOCK_NB: c_int = 4;
pub const LOCK_UN: c_int = 8;

pub const SS_ONSTACK: c_int = 1;
pub const SS_DISABLE: c_int = 2;

pub const PATH_MAX: c_int = 4096;

pub const FD_SETSIZE: usize = 1024;

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

pub const EPOLL_CTL_ADD: c_int = 1;
pub const EPOLL_CTL_MOD: c_int = 3;
pub const EPOLL_CTL_DEL: c_int = 2;

pub const MNT_DETACH: c_int = 0x2;
pub const MNT_EXPIRE: c_int = 0x4;

pub const Q_GETFMT: c_int = 0x800004;
pub const Q_GETINFO: c_int = 0x800005;
pub const Q_SETINFO: c_int = 0x800006;
pub const QIF_BLIMITS: u32 = 1;
pub const QIF_SPACE: u32 = 2;
pub const QIF_ILIMITS: u32 = 4;
pub const QIF_INODES: u32 = 8;
pub const QIF_BTIME: u32 = 16;
pub const QIF_ITIME: u32 = 32;
pub const QIF_LIMITS: u32 = 5;
pub const QIF_USAGE: u32 = 10;
pub const QIF_TIMES: u32 = 48;
pub const QIF_ALL: u32 = 63;

pub const MNT_FORCE: c_int = 0x1;

pub const Q_SYNC: c_int = 0x800001;
pub const Q_QUOTAON: c_int = 0x800002;
pub const Q_QUOTAOFF: c_int = 0x800003;
pub const Q_GETQUOTA: c_int = 0x800007;
pub const Q_SETQUOTA: c_int = 0x800008;

pub const TCIOFF: c_int = 2;
pub const TCION: c_int = 3;
pub const TCOOFF: c_int = 0;
pub const TCOON: c_int = 1;
pub const TCIFLUSH: c_int = 0;
pub const TCOFLUSH: c_int = 1;
pub const TCIOFLUSH: c_int = 2;
pub const NL0: c_int = 0x00000000;
pub const NL1: c_int = 0x00000100;
pub const TAB0: c_int = 0x00000000;
pub const CR0: c_int = 0x00000000;
pub const FF0: c_int = 0x00000000;
pub const BS0: c_int = 0x00000000;
pub const VT0: c_int = 0x00000000;
pub const VERASE: usize = 2;
pub const VKILL: usize = 3;
pub const VINTR: usize = 0;
pub const VQUIT: usize = 1;
pub const VLNEXT: usize = 15;
pub const IGNBRK: crate::tcflag_t = 0x00000001;
pub const BRKINT: crate::tcflag_t = 0x00000002;
pub const IGNPAR: crate::tcflag_t = 0x00000004;
pub const PARMRK: crate::tcflag_t = 0x00000008;
pub const INPCK: crate::tcflag_t = 0x00000010;
pub const ISTRIP: crate::tcflag_t = 0x00000020;
pub const INLCR: crate::tcflag_t = 0x00000040;
pub const IGNCR: crate::tcflag_t = 0x00000080;
pub const ICRNL: crate::tcflag_t = 0x00000100;
pub const IXANY: crate::tcflag_t = 0x00000800;
pub const IMAXBEL: crate::tcflag_t = 0x00002000;
pub const OPOST: crate::tcflag_t = 0x1;
pub const CS5: crate::tcflag_t = 0x00000000;
pub const CRTSCTS: crate::tcflag_t = 0x80000000;
pub const ECHO: crate::tcflag_t = 0x00000008;
pub const OCRNL: crate::tcflag_t = 0o000010;
pub const ONOCR: crate::tcflag_t = 0o000020;
pub const ONLRET: crate::tcflag_t = 0o000040;
pub const OFILL: crate::tcflag_t = 0o000100;
pub const OFDEL: crate::tcflag_t = 0o000200;

pub const CLONE_VM: c_int = 0x100;
pub const CLONE_FS: c_int = 0x200;
pub const CLONE_FILES: c_int = 0x400;
pub const CLONE_SIGHAND: c_int = 0x800;
pub const CLONE_PTRACE: c_int = 0x2000;
pub const CLONE_VFORK: c_int = 0x4000;
pub const CLONE_PARENT: c_int = 0x8000;
pub const CLONE_THREAD: c_int = 0x10000;
pub const CLONE_NEWNS: c_int = 0x20000;
pub const CLONE_SYSVSEM: c_int = 0x40000;
pub const CLONE_SETTLS: c_int = 0x80000;
pub const CLONE_PARENT_SETTID: c_int = 0x100000;
pub const CLONE_CHILD_CLEARTID: c_int = 0x200000;
pub const CLONE_DETACHED: c_int = 0x400000;
pub const CLONE_UNTRACED: c_int = 0x800000;
pub const CLONE_CHILD_SETTID: c_int = 0x01000000;
pub const CLONE_NEWUTS: c_int = 0x04000000;
pub const CLONE_NEWIPC: c_int = 0x08000000;
pub const CLONE_NEWUSER: c_int = 0x10000000;
pub const CLONE_NEWPID: c_int = 0x20000000;
pub const CLONE_NEWNET: c_int = 0x40000000;
pub const CLONE_IO: c_int = 0x80000000;
pub const CLONE_NEWCGROUP: c_int = 0x02000000;

pub const WNOHANG: c_int = 0x00000001;
pub const WUNTRACED: c_int = 0x00000002;
pub const WSTOPPED: c_int = WUNTRACED;
pub const WEXITED: c_int = 0x00000004;
pub const WCONTINUED: c_int = 0x00000008;
pub const WNOWAIT: c_int = 0x01000000;

// Options set using PTRACE_SETOPTIONS.
pub const PTRACE_O_TRACESYSGOOD: c_int = 0x00000001;
pub const PTRACE_O_TRACEFORK: c_int = 0x00000002;
pub const PTRACE_O_TRACEVFORK: c_int = 0x00000004;
pub const PTRACE_O_TRACECLONE: c_int = 0x00000008;
pub const PTRACE_O_TRACEEXEC: c_int = 0x00000010;
pub const PTRACE_O_TRACEVFORKDONE: c_int = 0x00000020;
pub const PTRACE_O_TRACEEXIT: c_int = 0x00000040;
pub const PTRACE_O_TRACESECCOMP: c_int = 0x00000080;
pub const PTRACE_O_EXITKILL: c_int = 0x00100000;
pub const PTRACE_O_SUSPEND_SECCOMP: c_int = 0x00200000;
pub const PTRACE_O_MASK: c_int = 0x003000ff;

// Wait extended result codes for the above trace options.
pub const PTRACE_EVENT_FORK: c_int = 1;
pub const PTRACE_EVENT_VFORK: c_int = 2;
pub const PTRACE_EVENT_CLONE: c_int = 3;
pub const PTRACE_EVENT_EXEC: c_int = 4;
pub const PTRACE_EVENT_VFORK_DONE: c_int = 5;
pub const PTRACE_EVENT_EXIT: c_int = 6;
pub const PTRACE_EVENT_SECCOMP: c_int = 7;
// PTRACE_EVENT_STOP was added to glibc in 2.26
// pub const PTRACE_EVENT_STOP: c_int = 128;

pub const __WNOTHREAD: c_int = 0x20000000;
pub const __WALL: c_int = 0x40000000;
pub const __WCLONE: c_int = 0x80000000;

pub const SPLICE_F_MOVE: c_uint = 0x01;
pub const SPLICE_F_NONBLOCK: c_uint = 0x02;
pub const SPLICE_F_MORE: c_uint = 0x04;
pub const SPLICE_F_GIFT: c_uint = 0x08;

pub const RTLD_LOCAL: c_int = 0;
pub const RTLD_LAZY: c_int = 1;

pub const POSIX_FADV_NORMAL: c_int = 0;
pub const POSIX_FADV_RANDOM: c_int = 1;
pub const POSIX_FADV_SEQUENTIAL: c_int = 2;
pub const POSIX_FADV_WILLNEED: c_int = 3;

pub const AT_FDCWD: c_int = -100;
pub const AT_SYMLINK_NOFOLLOW: c_int = 0x100;
pub const AT_REMOVEDIR: c_int = 0x200;
pub const AT_EACCESS: c_int = 0x200;
pub const AT_SYMLINK_FOLLOW: c_int = 0x400;
pub const AT_NO_AUTOMOUNT: c_int = 0x800;
pub const AT_EMPTY_PATH: c_int = 0x1000;

pub const LOG_CRON: c_int = 9 << 3;
pub const LOG_AUTHPRIV: c_int = 10 << 3;
pub const LOG_FTP: c_int = 11 << 3;
pub const LOG_PERROR: c_int = 0x20;

pub const PIPE_BUF: usize = 4096;

pub const SI_LOAD_SHIFT: c_uint = 16;

pub const CLD_EXITED: c_int = 1;
pub const CLD_KILLED: c_int = 2;
pub const CLD_DUMPED: c_int = 3;
pub const CLD_TRAPPED: c_int = 4;
pub const CLD_STOPPED: c_int = 5;
pub const CLD_CONTINUED: c_int = 6;

pub const SIGEV_SIGNAL: c_int = 0;
pub const SIGEV_NONE: c_int = 1;
pub const SIGEV_THREAD: c_int = 2;

pub const P_ALL: idtype_t = 0;
pub const P_PID: idtype_t = 1;
pub const P_PGID: idtype_t = 2;

pub const UTIME_OMIT: c_long = 1073741822;
pub const UTIME_NOW: c_long = 1073741823;

pub const POLLIN: c_short = 0x1;
pub const POLLPRI: c_short = 0x2;
pub const POLLOUT: c_short = 0x4;
pub const POLLERR: c_short = 0x8;
pub const POLLHUP: c_short = 0x10;
pub const POLLNVAL: c_short = 0x20;
pub const POLLRDNORM: c_short = 0x040;
pub const POLLRDBAND: c_short = 0x080;

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

pub const S_IEXEC: mode_t = 0o0100;
pub const S_IWRITE: mode_t = 0o0200;
pub const S_IREAD: mode_t = 0o0400;

pub const F_LOCK: c_int = 1;
pub const F_TEST: c_int = 3;
pub const F_TLOCK: c_int = 2;
pub const F_ULOCK: c_int = 0;

pub const IFF_LOWER_UP: c_int = 0x10000;
pub const IFF_DORMANT: c_int = 0x20000;
pub const IFF_ECHO: c_int = 0x40000;

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

pub const TCP_MD5SIG: c_int = 14;

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

pub const RENAME_NOREPLACE: c_int = 1;
pub const RENAME_EXCHANGE: c_int = 2;
pub const RENAME_WHITEOUT: c_int = 4;

pub const SCHED_OTHER: c_int = 0;
pub const SCHED_FIFO: c_int = 1;
pub const SCHED_RR: c_int = 2;
pub const SCHED_BATCH: c_int = 3;
pub const SCHED_IDLE: c_int = 5;

// netinet/in.h
// NOTE: These are in addition to the constants defined in src/unix/mod.rs

// IPPROTO_IP defined in src/unix/mod.rs
/// Hop-by-hop option header
pub const IPPROTO_HOPOPTS: c_int = 0;
// IPPROTO_ICMP defined in src/unix/mod.rs
/// group mgmt protocol
pub const IPPROTO_IGMP: c_int = 2;
/// for compatibility
pub const IPPROTO_IPIP: c_int = 4;
// IPPROTO_TCP defined in src/unix/mod.rs
/// exterior gateway protocol
pub const IPPROTO_EGP: c_int = 8;
/// pup
pub const IPPROTO_PUP: c_int = 12;
// IPPROTO_UDP defined in src/unix/mod.rs
/// xns idp
pub const IPPROTO_IDP: c_int = 22;
/// tp-4 w/ class negotiation
pub const IPPROTO_TP: c_int = 29;
/// DCCP
pub const IPPROTO_DCCP: c_int = 33;
// IPPROTO_IPV6 defined in src/unix/mod.rs
/// IP6 routing header
pub const IPPROTO_ROUTING: c_int = 43;
/// IP6 fragmentation header
pub const IPPROTO_FRAGMENT: c_int = 44;
/// resource reservation
pub const IPPROTO_RSVP: c_int = 46;
/// General Routing Encap.
pub const IPPROTO_GRE: c_int = 47;
/// IP6 Encap Sec. Payload
pub const IPPROTO_ESP: c_int = 50;
/// IP6 Auth Header
pub const IPPROTO_AH: c_int = 51;
// IPPROTO_ICMPV6 defined in src/unix/mod.rs
/// IP6 no next header
pub const IPPROTO_NONE: c_int = 59;
/// IP6 destination option
pub const IPPROTO_DSTOPTS: c_int = 60;
pub const IPPROTO_MTP: c_int = 92;
pub const IPPROTO_BEETPH: c_int = 94;
/// encapsulation header
pub const IPPROTO_ENCAP: c_int = 98;
/// Protocol indep. multicast
pub const IPPROTO_PIM: c_int = 103;
/// IP Payload Comp. Protocol
pub const IPPROTO_COMP: c_int = 108;
/// SCTP
pub const IPPROTO_SCTP: c_int = 132;
pub const IPPROTO_MH: c_int = 135;
pub const IPPROTO_UDPLITE: c_int = 136;
pub const IPPROTO_MPLS: c_int = 137;
/// raw IP packet
pub const IPPROTO_RAW: c_int = 255;
pub const IPPROTO_MAX: c_int = 256;

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
pub const MSG_COPY: c_int = 0o40000;

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

pub const EPOLLRDHUP: c_int = 0x2000;
pub const EPOLLEXCLUSIVE: c_int = 0x10000000;
pub const EPOLLONESHOT: c_int = 0x40000000;

pub const QFMT_VFS_OLD: c_int = 1;
pub const QFMT_VFS_V0: c_int = 2;
pub const QFMT_VFS_V1: c_int = 4;

pub const EFD_SEMAPHORE: c_int = 0x1;

pub const LOG_NFACILITIES: c_int = 24;

pub const SEM_FAILED: *mut crate::sem_t = 0 as *mut sem_t;

pub const RB_AUTOBOOT: c_int = 0x01234567u32 as i32;
pub const RB_HALT_SYSTEM: c_int = 0xcdef0123u32 as i32;
pub const RB_ENABLE_CAD: c_int = 0x89abcdefu32 as i32;
pub const RB_DISABLE_CAD: c_int = 0x00000000u32 as i32;
pub const RB_POWER_OFF: c_int = 0x4321fedcu32 as i32;
pub const RB_SW_SUSPEND: c_int = 0xd000fce2u32 as i32;
pub const RB_KEXEC: c_int = 0x45584543u32 as i32;

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

pub const AIO_CANCELED: c_int = 0;
pub const AIO_NOTCANCELED: c_int = 1;
pub const AIO_ALLDONE: c_int = 2;
pub const LIO_READ: c_int = 0;
pub const LIO_WRITE: c_int = 1;
pub const LIO_NOP: c_int = 2;
pub const LIO_WAIT: c_int = 0;
pub const LIO_NOWAIT: c_int = 1;

pub const MREMAP_MAYMOVE: c_int = 1;
pub const MREMAP_FIXED: c_int = 2;

pub const PR_SET_PDEATHSIG: c_int = 1;
pub const PR_GET_PDEATHSIG: c_int = 2;

pub const PR_GET_DUMPABLE: c_int = 3;
pub const PR_SET_DUMPABLE: c_int = 4;

pub const PR_GET_UNALIGN: c_int = 5;
pub const PR_SET_UNALIGN: c_int = 6;
pub const PR_UNALIGN_NOPRINT: c_int = 1;
pub const PR_UNALIGN_SIGBUS: c_int = 2;

pub const PR_GET_KEEPCAPS: c_int = 7;
pub const PR_SET_KEEPCAPS: c_int = 8;

pub const PR_GET_FPEMU: c_int = 9;
pub const PR_SET_FPEMU: c_int = 10;
pub const PR_FPEMU_NOPRINT: c_int = 1;
pub const PR_FPEMU_SIGFPE: c_int = 2;

pub const PR_GET_FPEXC: c_int = 11;
pub const PR_SET_FPEXC: c_int = 12;
pub const PR_FP_EXC_SW_ENABLE: c_int = 0x80;
pub const PR_FP_EXC_DIV: c_int = 0x010000;
pub const PR_FP_EXC_OVF: c_int = 0x020000;
pub const PR_FP_EXC_UND: c_int = 0x040000;
pub const PR_FP_EXC_RES: c_int = 0x080000;
pub const PR_FP_EXC_INV: c_int = 0x100000;
pub const PR_FP_EXC_DISABLED: c_int = 0;
pub const PR_FP_EXC_NONRECOV: c_int = 1;
pub const PR_FP_EXC_ASYNC: c_int = 2;
pub const PR_FP_EXC_PRECISE: c_int = 3;

pub const PR_GET_TIMING: c_int = 13;
pub const PR_SET_TIMING: c_int = 14;
pub const PR_TIMING_STATISTICAL: c_int = 0;
pub const PR_TIMING_TIMESTAMP: c_int = 1;

pub const PR_SET_NAME: c_int = 15;
pub const PR_GET_NAME: c_int = 16;

pub const PR_GET_ENDIAN: c_int = 19;
pub const PR_SET_ENDIAN: c_int = 20;
pub const PR_ENDIAN_BIG: c_int = 0;
pub const PR_ENDIAN_LITTLE: c_int = 1;
pub const PR_ENDIAN_PPC_LITTLE: c_int = 2;

pub const PR_GET_SECCOMP: c_int = 21;
pub const PR_SET_SECCOMP: c_int = 22;

pub const PR_CAPBSET_READ: c_int = 23;
pub const PR_CAPBSET_DROP: c_int = 24;

pub const PR_GET_TSC: c_int = 25;
pub const PR_SET_TSC: c_int = 26;
pub const PR_TSC_ENABLE: c_int = 1;
pub const PR_TSC_SIGSEGV: c_int = 2;

pub const PR_GET_SECUREBITS: c_int = 27;
pub const PR_SET_SECUREBITS: c_int = 28;

pub const PR_SET_TIMERSLACK: c_int = 29;
pub const PR_GET_TIMERSLACK: c_int = 30;

pub const PR_TASK_PERF_EVENTS_DISABLE: c_int = 31;
pub const PR_TASK_PERF_EVENTS_ENABLE: c_int = 32;

pub const PR_MCE_KILL: c_int = 33;
pub const PR_MCE_KILL_CLEAR: c_int = 0;
pub const PR_MCE_KILL_SET: c_int = 1;

pub const PR_MCE_KILL_LATE: c_int = 0;
pub const PR_MCE_KILL_EARLY: c_int = 1;
pub const PR_MCE_KILL_DEFAULT: c_int = 2;

pub const PR_MCE_KILL_GET: c_int = 34;

pub const PR_SET_MM: c_int = 35;
pub const PR_SET_MM_START_CODE: c_int = 1;
pub const PR_SET_MM_END_CODE: c_int = 2;
pub const PR_SET_MM_START_DATA: c_int = 3;
pub const PR_SET_MM_END_DATA: c_int = 4;
pub const PR_SET_MM_START_STACK: c_int = 5;
pub const PR_SET_MM_START_BRK: c_int = 6;
pub const PR_SET_MM_BRK: c_int = 7;
pub const PR_SET_MM_ARG_START: c_int = 8;
pub const PR_SET_MM_ARG_END: c_int = 9;
pub const PR_SET_MM_ENV_START: c_int = 10;
pub const PR_SET_MM_ENV_END: c_int = 11;
pub const PR_SET_MM_AUXV: c_int = 12;
pub const PR_SET_MM_EXE_FILE: c_int = 13;
pub const PR_SET_MM_MAP: c_int = 14;
pub const PR_SET_MM_MAP_SIZE: c_int = 15;

pub const PR_SET_PTRACER: c_int = 0x59616d61;
pub const PR_SET_PTRACER_ANY: c_ulong = 0xffffffffffffffff;

pub const PR_SET_CHILD_SUBREAPER: c_int = 36;
pub const PR_GET_CHILD_SUBREAPER: c_int = 37;

pub const PR_SET_NO_NEW_PRIVS: c_int = 38;
pub const PR_GET_NO_NEW_PRIVS: c_int = 39;

pub const PR_GET_TID_ADDRESS: c_int = 40;

pub const PR_SET_THP_DISABLE: c_int = 41;
pub const PR_GET_THP_DISABLE: c_int = 42;

pub const PR_MPX_ENABLE_MANAGEMENT: c_int = 43;
pub const PR_MPX_DISABLE_MANAGEMENT: c_int = 44;

pub const PR_SET_FP_MODE: c_int = 45;
pub const PR_GET_FP_MODE: c_int = 46;
pub const PR_FP_MODE_FR: c_int = 1 << 0;
pub const PR_FP_MODE_FRE: c_int = 1 << 1;

pub const PR_CAP_AMBIENT: c_int = 47;
pub const PR_CAP_AMBIENT_IS_SET: c_int = 1;
pub const PR_CAP_AMBIENT_RAISE: c_int = 2;
pub const PR_CAP_AMBIENT_LOWER: c_int = 3;
pub const PR_CAP_AMBIENT_CLEAR_ALL: c_int = 4;

pub const ITIMER_REAL: c_int = 0;
pub const ITIMER_VIRTUAL: c_int = 1;
pub const ITIMER_PROF: c_int = 2;

pub const TFD_CLOEXEC: c_int = O_CLOEXEC;
pub const TFD_NONBLOCK: c_int = O_NONBLOCK;
pub const TFD_TIMER_ABSTIME: c_int = 1;

pub const XATTR_CREATE: c_int = 0x1;
pub const XATTR_REPLACE: c_int = 0x2;

pub const _POSIX_VDISABLE: crate::cc_t = 0;

pub const FALLOC_FL_KEEP_SIZE: c_int = 0x01;
pub const FALLOC_FL_PUNCH_HOLE: c_int = 0x02;
pub const FALLOC_FL_COLLAPSE_RANGE: c_int = 0x08;
pub const FALLOC_FL_ZERO_RANGE: c_int = 0x10;
pub const FALLOC_FL_INSERT_RANGE: c_int = 0x20;
pub const FALLOC_FL_UNSHARE_RANGE: c_int = 0x40;

// On Linux, libc doesn't define this constant, libattr does instead.
// We still define it for Linux as it's defined by libc on other platforms,
// and it's mentioned in the man pages for getxattr and setxattr.
pub const ENOATTR: c_int = crate::ENODATA;

pub const SO_ORIGINAL_DST: c_int = 80;
pub const IUTF8: crate::tcflag_t = 0x00004000;
pub const CMSPAR: crate::tcflag_t = 0o10000000000;

pub const MFD_CLOEXEC: c_uint = 0x0001;
pub const MFD_ALLOW_SEALING: c_uint = 0x0002;

// these are used in the p_type field of Elf32_Phdr and Elf64_Phdr, which has
// the type Elf32Word and Elf64Word respectively. Luckily, both of those are u32
// so we can use that type here to avoid having to cast.
pub const PT_NULL: u32 = 0;
pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;
pub const PT_NOTE: u32 = 4;
pub const PT_SHLIB: u32 = 5;
pub const PT_PHDR: u32 = 6;
pub const PT_TLS: u32 = 7;
pub const PT_NUM: u32 = 8;
pub const PT_LOOS: u32 = 0x60000000;
pub const PT_GNU_EH_FRAME: u32 = 0x6474e550;
pub const PT_GNU_STACK: u32 = 0x6474e551;
pub const PT_GNU_RELRO: u32 = 0x6474e552;

// Ethernet protocol IDs.
pub const ETH_P_LOOP: c_int = 0x0060;
pub const ETH_P_PUP: c_int = 0x0200;
pub const ETH_P_PUPAT: c_int = 0x0201;
pub const ETH_P_IP: c_int = 0x0800;
pub const ETH_P_X25: c_int = 0x0805;
pub const ETH_P_ARP: c_int = 0x0806;
pub const ETH_P_BPQ: c_int = 0x08FF;
pub const ETH_P_IEEEPUP: c_int = 0x0a00;
pub const ETH_P_IEEEPUPAT: c_int = 0x0a01;
pub const ETH_P_BATMAN: c_int = 0x4305;
pub const ETH_P_DEC: c_int = 0x6000;
pub const ETH_P_DNA_DL: c_int = 0x6001;
pub const ETH_P_DNA_RC: c_int = 0x6002;
pub const ETH_P_DNA_RT: c_int = 0x6003;
pub const ETH_P_LAT: c_int = 0x6004;
pub const ETH_P_DIAG: c_int = 0x6005;
pub const ETH_P_CUST: c_int = 0x6006;
pub const ETH_P_SCA: c_int = 0x6007;
pub const ETH_P_TEB: c_int = 0x6558;
pub const ETH_P_RARP: c_int = 0x8035;
pub const ETH_P_ATALK: c_int = 0x809B;
pub const ETH_P_AARP: c_int = 0x80F3;
pub const ETH_P_8021Q: c_int = 0x8100;
pub const ETH_P_IPX: c_int = 0x8137;
pub const ETH_P_IPV6: c_int = 0x86DD;
pub const ETH_P_PAUSE: c_int = 0x8808;
pub const ETH_P_SLOW: c_int = 0x8809;
pub const ETH_P_WCCP: c_int = 0x883E;
pub const ETH_P_MPLS_UC: c_int = 0x8847;
pub const ETH_P_MPLS_MC: c_int = 0x8848;
pub const ETH_P_ATMMPOA: c_int = 0x884c;
pub const ETH_P_PPP_DISC: c_int = 0x8863;
pub const ETH_P_PPP_SES: c_int = 0x8864;
pub const ETH_P_LINK_CTL: c_int = 0x886c;
pub const ETH_P_ATMFATE: c_int = 0x8884;
pub const ETH_P_PAE: c_int = 0x888E;
pub const ETH_P_AOE: c_int = 0x88A2;
pub const ETH_P_8021AD: c_int = 0x88A8;
pub const ETH_P_802_EX1: c_int = 0x88B5;
pub const ETH_P_TIPC: c_int = 0x88CA;
pub const ETH_P_8021AH: c_int = 0x88E7;
pub const ETH_P_MVRP: c_int = 0x88F5;
pub const ETH_P_1588: c_int = 0x88F7;
pub const ETH_P_PRP: c_int = 0x88FB;
pub const ETH_P_FCOE: c_int = 0x8906;
pub const ETH_P_TDLS: c_int = 0x890D;
pub const ETH_P_FIP: c_int = 0x8914;
pub const ETH_P_80221: c_int = 0x8917;
pub const ETH_P_LOOPBACK: c_int = 0x9000;
pub const ETH_P_QINQ1: c_int = 0x9100;
pub const ETH_P_QINQ2: c_int = 0x9200;
pub const ETH_P_QINQ3: c_int = 0x9300;
pub const ETH_P_EDSA: c_int = 0xDADA;
pub const ETH_P_AF_IUCV: c_int = 0xFBFB;

pub const ETH_P_802_3_MIN: c_int = 0x0600;

pub const ETH_P_802_3: c_int = 0x0001;
pub const ETH_P_AX25: c_int = 0x0002;
pub const ETH_P_ALL: c_int = 0x0003;
pub const ETH_P_802_2: c_int = 0x0004;
pub const ETH_P_SNAP: c_int = 0x0005;
pub const ETH_P_DDCMP: c_int = 0x0006;
pub const ETH_P_WAN_PPP: c_int = 0x0007;
pub const ETH_P_PPP_MP: c_int = 0x0008;
pub const ETH_P_LOCALTALK: c_int = 0x0009;
pub const ETH_P_CAN: c_int = 0x000C;
pub const ETH_P_CANFD: c_int = 0x000D;
pub const ETH_P_PPPTALK: c_int = 0x0010;
pub const ETH_P_TR_802_2: c_int = 0x0011;
pub const ETH_P_MOBITEX: c_int = 0x0015;
pub const ETH_P_CONTROL: c_int = 0x0016;
pub const ETH_P_IRDA: c_int = 0x0017;
pub const ETH_P_ECONET: c_int = 0x0018;
pub const ETH_P_HDLC: c_int = 0x0019;
pub const ETH_P_ARCNET: c_int = 0x001A;
pub const ETH_P_DSA: c_int = 0x001B;
pub const ETH_P_TRAILER: c_int = 0x001C;
pub const ETH_P_PHONET: c_int = 0x00F5;
pub const ETH_P_IEEE802154: c_int = 0x00F6;
pub const ETH_P_CAIF: c_int = 0x00F7;

pub const SFD_CLOEXEC: c_int = 0x080000;

pub const NCCS: usize = 32;

pub const O_TRUNC: c_int = 0x00040000;
pub const O_NOATIME: c_int = 0x00002000;
pub const O_CLOEXEC: c_int = 0x00000100;
pub const O_TMPFILE: c_int = 0x00004000;

pub const EBFONT: c_int = 59;
pub const ENOSTR: c_int = 60;
pub const ENODATA: c_int = 61;
pub const ETIME: c_int = 62;
pub const ENOSR: c_int = 63;
pub const ENONET: c_int = 64;
pub const ENOPKG: c_int = 65;
pub const EREMOTE: c_int = 66;
pub const ENOLINK: c_int = 67;
pub const EADV: c_int = 68;
pub const ESRMNT: c_int = 69;
pub const ECOMM: c_int = 70;
pub const EPROTO: c_int = 71;
pub const EDOTDOT: c_int = 73;

pub const SA_NODEFER: c_int = 0x40000000;
pub const SA_RESETHAND: c_int = 0x80000000;
pub const SA_RESTART: c_int = 0x10000000;
pub const SA_NOCLDSTOP: c_int = 0x00000001;

pub const EPOLL_CLOEXEC: c_int = 0x80000;

pub const EFD_CLOEXEC: c_int = 0x80000;

pub const BUFSIZ: c_uint = 1024;
pub const TMP_MAX: c_uint = 10000;
pub const FOPEN_MAX: c_uint = 1000;
pub const O_PATH: c_int = 0x00400000;
pub const O_EXEC: c_int = O_PATH;
pub const O_SEARCH: c_int = O_PATH;
pub const O_ACCMODE: c_int = 03 | O_SEARCH;
pub const O_NDELAY: c_int = O_NONBLOCK;
pub const NI_MAXHOST: crate::socklen_t = 255;
pub const PTHREAD_STACK_MIN: size_t = 2048;
pub const POSIX_FADV_DONTNEED: c_int = 4;
pub const POSIX_FADV_NOREUSE: c_int = 5;

pub const POSIX_MADV_DONTNEED: c_int = 4;

pub const RLIM_INFINITY: crate::rlim_t = !0;
pub const RLIMIT_RTTIME: c_int = 15;
#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIMIT_NLIMITS: c_int = 16;
#[allow(deprecated)]
#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIM_NLIMITS: c_int = RLIMIT_NLIMITS;

pub const MAP_ANONYMOUS: c_int = MAP_ANON;

pub const SOCK_DCCP: c_int = 6;
pub const SOCK_PACKET: c_int = 10;

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

pub const SIGUNUSED: c_int = crate::SIGSYS;

pub const __SIZEOF_PTHREAD_CONDATTR_T: usize = 4;
pub const __SIZEOF_PTHREAD_MUTEXATTR_T: usize = 4;
pub const __SIZEOF_PTHREAD_RWLOCKATTR_T: usize = 8;

pub const CPU_SETSIZE: c_int = 128;

pub const PTRACE_TRACEME: c_int = 0;
pub const PTRACE_PEEKTEXT: c_int = 1;
pub const PTRACE_PEEKDATA: c_int = 2;
pub const PTRACE_PEEKUSER: c_int = 3;
pub const PTRACE_POKETEXT: c_int = 4;
pub const PTRACE_POKEDATA: c_int = 5;
pub const PTRACE_POKEUSER: c_int = 6;
pub const PTRACE_CONT: c_int = 7;
pub const PTRACE_KILL: c_int = 8;
pub const PTRACE_SINGLESTEP: c_int = 9;
pub const PTRACE_GETREGS: c_int = 12;
pub const PTRACE_SETREGS: c_int = 13;
pub const PTRACE_GETFPREGS: c_int = 14;
pub const PTRACE_SETFPREGS: c_int = 15;
pub const PTRACE_ATTACH: c_int = 16;
pub const PTRACE_DETACH: c_int = 17;
pub const PTRACE_GETFPXREGS: c_int = 18;
pub const PTRACE_SETFPXREGS: c_int = 19;
pub const PTRACE_SYSCALL: c_int = 24;
pub const PTRACE_SETOPTIONS: c_int = 0x4200;
pub const PTRACE_GETEVENTMSG: c_int = 0x4201;
pub const PTRACE_GETSIGINFO: c_int = 0x4202;
pub const PTRACE_SETSIGINFO: c_int = 0x4203;
pub const PTRACE_GETREGSET: c_int = 0x4204;
pub const PTRACE_SETREGSET: c_int = 0x4205;
pub const PTRACE_SEIZE: c_int = 0x4206;
pub const PTRACE_INTERRUPT: c_int = 0x4207;
pub const PTRACE_LISTEN: c_int = 0x4208;
pub const PTRACE_PEEKSIGINFO: c_int = 0x4209;

pub const EPOLLWAKEUP: c_int = 0x20000000;

pub const EFD_NONBLOCK: c_int = crate::O_NONBLOCK;

pub const SFD_NONBLOCK: c_int = crate::O_NONBLOCK;

pub const TCSANOW: c_int = 0;
pub const TCSADRAIN: c_int = 1;
pub const TCSAFLUSH: c_int = 2;

pub const TIOCINQ: c_int = crate::FIONREAD;

pub const RTLD_GLOBAL: c_int = 0x100;
pub const RTLD_NOLOAD: c_int = 0x4;

pub const MCL_CURRENT: c_int = 0x0001;
pub const MCL_FUTURE: c_int = 0x0002;

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
pub const CIBAUD: crate::tcflag_t = 0o02003600000;
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
pub const EXTA: crate::speed_t = B19200;
pub const EXTB: crate::speed_t = B38400;
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
pub const SO_TIMESTAMP: c_int = 29;
pub const SO_MARK: c_int = 36;
pub const SO_RXQ_OVFL: c_int = 40;
pub const SO_PEEK_OFF: c_int = 42;
pub const SO_BUSY_POLL: c_int = 46;
pub const SO_BINDTOIFINDEX: c_int = 62;

pub const __SIZEOF_PTHREAD_RWLOCK_T: usize = 56;
pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 40;

pub const O_ASYNC: c_int = 0x00000400;

pub const FIOCLEX: c_int = 0x5451;
pub const FIONBIO: c_int = 0x5421;

pub const RLIMIT_RSS: c_int = 5;
pub const RLIMIT_NOFILE: c_int = 7;
pub const RLIMIT_AS: c_int = 9;
pub const RLIMIT_NPROC: c_int = 6;
pub const RLIMIT_MEMLOCK: c_int = 8;

pub const O_APPEND: c_int = 0x00100000;
pub const O_CREAT: c_int = 0x00010000;
pub const O_EXCL: c_int = 0x00020000;
pub const O_NOCTTY: c_int = 0x00000200;
pub const O_NONBLOCK: c_int = 0x00000010;
pub const O_SYNC: c_int = 0x00000040 | O_DSYNC;
pub const O_RSYNC: c_int = O_SYNC;
pub const O_DSYNC: c_int = 0x00000020;

pub const SOCK_CLOEXEC: c_int = 0o2000000;
pub const SOCK_NONBLOCK: c_int = 0o4000;

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

pub const SOL_SOCKET: c_int = 1;

pub const EDEADLK: c_int = 35;
pub const ENAMETOOLONG: c_int = 36;
pub const ENOLCK: c_int = 37;
pub const ENOSYS: c_int = 38;
pub const ENOTEMPTY: c_int = 39;
pub const ELOOP: c_int = 40;
pub const ENOMSG: c_int = 42;
pub const EIDRM: c_int = 43;
pub const ECHRNG: c_int = 44;
pub const EL2NSYNC: c_int = 45;
pub const EL3HLT: c_int = 46;
pub const EL3RST: c_int = 47;
pub const ELNRNG: c_int = 48;
pub const EUNATCH: c_int = 49;
pub const ENOCSI: c_int = 50;
pub const EL2HLT: c_int = 51;
pub const EBADE: c_int = 52;
pub const EBADR: c_int = 53;
pub const EXFULL: c_int = 54;
pub const ENOANO: c_int = 55;
pub const EBADRQC: c_int = 56;
pub const EBADSLT: c_int = 57;
pub const EDEADLOCK: c_int = EDEADLK;
pub const EMULTIHOP: c_int = 72;
pub const EBADMSG: c_int = 74;
pub const EOVERFLOW: c_int = 75;
pub const ENOTUNIQ: c_int = 76;
pub const EBADFD: c_int = 77;
pub const EREMCHG: c_int = 78;
pub const ELIBACC: c_int = 79;
pub const ELIBBAD: c_int = 80;
pub const ELIBSCN: c_int = 81;
pub const ELIBMAX: c_int = 82;
pub const ELIBEXEC: c_int = 83;
pub const EILSEQ: c_int = 84;
pub const ERESTART: c_int = 85;
pub const ESTRPIPE: c_int = 86;
pub const EUSERS: c_int = 87;
pub const ENOTSOCK: c_int = 88;
pub const EDESTADDRREQ: c_int = 89;
pub const EMSGSIZE: c_int = 90;
pub const EPROTOTYPE: c_int = 91;
pub const ENOPROTOOPT: c_int = 92;
pub const EPROTONOSUPPORT: c_int = 93;
pub const ESOCKTNOSUPPORT: c_int = 94;
pub const EOPNOTSUPP: c_int = 95;
pub const ENOTSUP: c_int = EOPNOTSUPP;
pub const EPFNOSUPPORT: c_int = 96;
pub const EAFNOSUPPORT: c_int = 97;
pub const EADDRINUSE: c_int = 98;
pub const EADDRNOTAVAIL: c_int = 99;
pub const ENETDOWN: c_int = 100;
pub const ENETUNREACH: c_int = 101;
pub const ENETRESET: c_int = 102;
pub const ECONNABORTED: c_int = 103;
pub const ECONNRESET: c_int = 104;
pub const ENOBUFS: c_int = 105;
pub const EISCONN: c_int = 106;
pub const ENOTCONN: c_int = 107;
pub const ESHUTDOWN: c_int = 108;
pub const ETOOMANYREFS: c_int = 109;
pub const ETIMEDOUT: c_int = 110;
pub const ECONNREFUSED: c_int = 111;
pub const EHOSTDOWN: c_int = 112;
pub const EHOSTUNREACH: c_int = 113;
pub const EALREADY: c_int = 114;
pub const EINPROGRESS: c_int = 115;
pub const ESTALE: c_int = 116;
pub const EUCLEAN: c_int = 117;
pub const ENOTNAM: c_int = 118;
pub const ENAVAIL: c_int = 119;
pub const EISNAM: c_int = 120;
pub const EREMOTEIO: c_int = 121;
pub const EDQUOT: c_int = 122;
pub const ENOMEDIUM: c_int = 123;
pub const EMEDIUMTYPE: c_int = 124;
pub const ECANCELED: c_int = 125;
pub const ENOKEY: c_int = 126;
pub const EKEYEXPIRED: c_int = 127;
pub const EKEYREVOKED: c_int = 128;
pub const EKEYREJECTED: c_int = 129;
pub const EOWNERDEAD: c_int = 130;
pub const ENOTRECOVERABLE: c_int = 131;
pub const ERFKILL: c_int = 132;
pub const EHWPOISON: c_int = 133;

pub const SO_REUSEADDR: c_int = 2;
pub const SO_TYPE: c_int = 3;
pub const SO_ERROR: c_int = 4;
pub const SO_DONTROUTE: c_int = 5;
pub const SO_BROADCAST: c_int = 6;
pub const SO_SNDBUF: c_int = 7;
pub const SO_RCVBUF: c_int = 8;
pub const SO_KEEPALIVE: c_int = 9;
pub const SO_OOBINLINE: c_int = 10;
pub const SO_NO_CHECK: c_int = 11;
pub const SO_PRIORITY: c_int = 12;
pub const SO_LINGER: c_int = 13;
pub const SO_BSDCOMPAT: c_int = 14;
pub const SO_REUSEPORT: c_int = 15;
pub const SO_PASSCRED: c_int = 16;
pub const SO_PEERCRED: c_int = 17;
pub const SO_RCVLOWAT: c_int = 18;
pub const SO_SNDLOWAT: c_int = 19;
pub const SO_RCVTIMEO: c_int = 20;
pub const SO_SNDTIMEO: c_int = 21;
pub const SO_ACCEPTCONN: c_int = 30;
pub const SO_SNDBUFFORCE: c_int = 32;
pub const SO_RCVBUFFORCE: c_int = 33;
pub const SO_PROTOCOL: c_int = 38;
pub const SO_DOMAIN: c_int = 39;

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

pub const F_GETLK: c_int = 5;
pub const F_GETOWN: c_int = 9;
pub const F_SETLK: c_int = 6;
pub const F_SETLKW: c_int = 7;
pub const F_SETOWN: c_int = 8;

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

pub const O_DIRECTORY: c_int = 0x00080000;
pub const O_DIRECT: c_int = 0x00000800;
pub const O_LARGEFILE: c_int = 0x00001000;
pub const O_NOFOLLOW: c_int = 0x00000080;

pub const HUGETLB_FLAG_ENCODE_SHIFT: u32 = 26;
pub const MAP_HUGE_SHIFT: u32 = 26;

// intentionally not public, only used for fd_set
cfg_if! {
    if #[cfg(target_pointer_width = "32")] {
        const ULONG_SIZE: usize = 32;
    } else if #[cfg(target_pointer_width = "64")] {
        const ULONG_SIZE: usize = 64;
    } else {
        // Unknown target_pointer_width
    }
}

// END_PUB_CONST

f! {
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

    pub fn CMSG_DATA(cmsg: *const cmsghdr) -> *mut c_uchar {
        cmsg.offset(1) as *mut c_uchar
    }

    pub fn CMSG_NXTHDR(mhdr: *const msghdr, cmsg: *const cmsghdr) -> *mut cmsghdr {
        if ((*cmsg).cmsg_len as size_t) < size_of::<cmsghdr>() {
            core::ptr::null_mut::<cmsghdr>()
        } else if __CMSG_NEXT(cmsg).add(size_of::<cmsghdr>()) >= __MHDR_END(mhdr) {
            core::ptr::null_mut::<cmsghdr>()
        } else {
            __CMSG_NEXT(cmsg).cast()
        }
    }

    pub fn CMSG_FIRSTHDR(mhdr: *const msghdr) -> *mut cmsghdr {
        if (*mhdr).msg_controllen as size_t >= size_of::<cmsghdr>() {
            (*mhdr).msg_control.cast()
        } else {
            core::ptr::null_mut::<cmsghdr>()
        }
    }

    pub {const} fn CMSG_ALIGN(len: size_t) -> size_t {
        (len + size_of::<size_t>() - 1) & !(size_of::<size_t>() - 1)
    }

    pub {const} fn CMSG_SPACE(len: c_uint) -> c_uint {
        (CMSG_ALIGN(len as size_t) + CMSG_ALIGN(size_of::<cmsghdr>())) as c_uint
    }

    pub {const} fn CMSG_LEN(len: c_uint) -> c_uint {
        (CMSG_ALIGN(size_of::<cmsghdr>()) + len as size_t) as c_uint
    }
}

safe_f! {
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

    pub {const} fn QCMD(cmd: c_int, type_: c_int) -> c_int {
        (cmd << 8) | (type_ & 0x00ff)
    }

    pub {const} fn makedev(major: c_uint, minor: c_uint) -> crate::dev_t {
        let major = major as crate::dev_t;
        let minor = minor as crate::dev_t;
        let mut dev = 0;
        dev |= (major & 0x00000fff) << 8;
        dev |= (major & 0xfffff000) << 32;
        dev |= (minor & 0x000000ff) << 0;
        dev |= (minor & 0xffffff00) << 12;
        dev
    }

    pub {const} fn major(dev: crate::dev_t) -> c_uint {
        let mut major = 0;
        major |= (dev & 0x00000000000fff00) >> 8;
        major |= (dev & 0xfffff00000000000) >> 32;
        major as c_uint
    }

    pub {const} fn minor(dev: crate::dev_t) -> c_uint {
        let mut minor = 0;
        minor |= (dev & 0x00000000000000ff) >> 0;
        minor |= (dev & 0x00000ffffff00000) >> 12;
        minor as c_uint
    }
}

fn __CMSG_LEN(cmsg: *const cmsghdr) -> ssize_t {
    ((unsafe { (*cmsg).cmsg_len as size_t } + size_of::<c_long>() - 1) & !(size_of::<c_long>() - 1))
        as ssize_t
}

fn __CMSG_NEXT(cmsg: *const cmsghdr) -> *mut c_uchar {
    (unsafe { cmsg.offset(__CMSG_LEN(cmsg)) }) as *mut c_uchar
}

fn __MHDR_END(mhdr: *const msghdr) -> *mut c_uchar {
    unsafe { (*mhdr).msg_control.offset((*mhdr).msg_controllen as isize) }.cast()
}

// EXTERN_FN

#[link(name = "c")]
#[link(name = "fdio")]
extern "C" {}

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum FILE {}
impl Copy for FILE {}
impl Clone for FILE {
    fn clone(&self) -> FILE {
        *self
    }
}
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum fpos_t {} // FIXME(fuchsia): fill this out with a struct
impl Copy for fpos_t {}
impl Clone for fpos_t {
    fn clone(&self) -> fpos_t {
        *self
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
    pub fn _exit(status: c_int) -> !;
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
    pub fn strlen(cs: *const c_char) -> size_t;
    pub fn strnlen(cs: *const c_char, maxlen: size_t) -> size_t;
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

    pub fn abs(i: c_int) -> c_int;
    pub fn labs(i: c_long) -> c_long;
    pub fn rand() -> c_int;
    pub fn srand(seed: c_uint);

    pub fn getpwnam(name: *const c_char) -> *mut passwd;
    pub fn getpwuid(uid: crate::uid_t) -> *mut passwd;

    pub fn fprintf(stream: *mut crate::FILE, format: *const c_char, ...) -> c_int;
    pub fn printf(format: *const c_char, ...) -> c_int;
    pub fn snprintf(s: *mut c_char, n: size_t, format: *const c_char, ...) -> c_int;
    pub fn sprintf(s: *mut c_char, format: *const c_char, ...) -> c_int;
    pub fn fscanf(stream: *mut crate::FILE, format: *const c_char, ...) -> c_int;
    pub fn scanf(format: *const c_char, ...) -> c_int;
    pub fn sscanf(s: *const c_char, format: *const c_char, ...) -> c_int;
    pub fn getchar_unlocked() -> c_int;
    pub fn putchar_unlocked(c: c_int) -> c_int;

    pub fn socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int;
    pub fn connect(socket: c_int, address: *const sockaddr, len: socklen_t) -> c_int;
    pub fn listen(socket: c_int, backlog: c_int) -> c_int;
    pub fn accept(socket: c_int, address: *mut sockaddr, address_len: *mut socklen_t) -> c_int;
    pub fn getpeername(socket: c_int, address: *mut sockaddr, address_len: *mut socklen_t)
        -> c_int;
    pub fn getsockname(socket: c_int, address: *mut sockaddr, address_len: *mut socklen_t)
        -> c_int;
    pub fn setsockopt(
        socket: c_int,
        level: c_int,
        name: c_int,
        value: *const c_void,
        option_len: socklen_t,
    ) -> c_int;
    pub fn socketpair(
        domain: c_int,
        type_: c_int,
        protocol: c_int,
        socket_vector: *mut c_int,
    ) -> c_int;
    pub fn sendto(
        socket: c_int,
        buf: *const c_void,
        len: size_t,
        flags: c_int,
        addr: *const sockaddr,
        addrlen: socklen_t,
    ) -> ssize_t;
    pub fn shutdown(socket: c_int, how: c_int) -> c_int;

    pub fn chmod(path: *const c_char, mode: mode_t) -> c_int;
    pub fn fchmod(fd: c_int, mode: mode_t) -> c_int;

    pub fn fstat(fildes: c_int, buf: *mut stat) -> c_int;

    pub fn mkdir(path: *const c_char, mode: mode_t) -> c_int;

    pub fn stat(path: *const c_char, buf: *mut stat) -> c_int;

    pub fn pclose(stream: *mut crate::FILE) -> c_int;
    pub fn fdopen(fd: c_int, mode: *const c_char) -> *mut crate::FILE;
    pub fn fileno(stream: *mut crate::FILE) -> c_int;

    pub fn open(path: *const c_char, oflag: c_int, ...) -> c_int;
    pub fn creat(path: *const c_char, mode: mode_t) -> c_int;
    pub fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;

    pub fn opendir(dirname: *const c_char) -> *mut crate::DIR;
    pub fn readdir(dirp: *mut crate::DIR) -> *mut crate::dirent;
    pub fn readdir_r(
        dirp: *mut crate::DIR,
        entry: *mut crate::dirent,
        result: *mut *mut crate::dirent,
    ) -> c_int;
    pub fn closedir(dirp: *mut crate::DIR) -> c_int;
    pub fn rewinddir(dirp: *mut crate::DIR);

    pub fn openat(dirfd: c_int, pathname: *const c_char, flags: c_int, ...) -> c_int;
    pub fn fchmodat(dirfd: c_int, pathname: *const c_char, mode: mode_t, flags: c_int) -> c_int;
    pub fn fchown(fd: c_int, owner: crate::uid_t, group: crate::gid_t) -> c_int;
    pub fn fchownat(
        dirfd: c_int,
        pathname: *const c_char,
        owner: crate::uid_t,
        group: crate::gid_t,
        flags: c_int,
    ) -> c_int;
    pub fn fstatat(dirfd: c_int, pathname: *const c_char, buf: *mut stat, flags: c_int) -> c_int;
    pub fn linkat(
        olddirfd: c_int,
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
        flags: c_int,
    ) -> c_int;
    pub fn mkdirat(dirfd: c_int, pathname: *const c_char, mode: mode_t) -> c_int;
    pub fn readlinkat(
        dirfd: c_int,
        pathname: *const c_char,
        buf: *mut c_char,
        bufsiz: size_t,
    ) -> ssize_t;
    pub fn renameat(
        olddirfd: c_int,
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
    ) -> c_int;
    pub fn symlinkat(target: *const c_char, newdirfd: c_int, linkpath: *const c_char) -> c_int;
    pub fn unlinkat(dirfd: c_int, pathname: *const c_char, flags: c_int) -> c_int;

    pub fn access(path: *const c_char, amode: c_int) -> c_int;
    pub fn alarm(seconds: c_uint) -> c_uint;
    pub fn chdir(dir: *const c_char) -> c_int;
    pub fn chown(path: *const c_char, uid: uid_t, gid: gid_t) -> c_int;
    pub fn lchown(path: *const c_char, uid: uid_t, gid: gid_t) -> c_int;
    pub fn close(fd: c_int) -> c_int;
    pub fn dup(fd: c_int) -> c_int;
    pub fn dup2(src: c_int, dst: c_int) -> c_int;

    pub fn execl(path: *const c_char, arg0: *const c_char, ...) -> c_int;
    pub fn execle(path: *const c_char, arg0: *const c_char, ...) -> c_int;
    pub fn execlp(file: *const c_char, arg0: *const c_char, ...) -> c_int;

    // DIFF(main): changed to `*const *mut` in e77f551de9
    pub fn execv(prog: *const c_char, argv: *const *const c_char) -> c_int;
    pub fn execve(
        prog: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int;
    pub fn execvp(c: *const c_char, argv: *const *const c_char) -> c_int;

    pub fn fork() -> pid_t;
    pub fn fpathconf(filedes: c_int, name: c_int) -> c_long;
    pub fn getcwd(buf: *mut c_char, size: size_t) -> *mut c_char;
    pub fn getegid() -> gid_t;
    pub fn geteuid() -> uid_t;
    pub fn getgid() -> gid_t;
    pub fn getgroups(ngroups_max: c_int, groups: *mut gid_t) -> c_int;
    pub fn getlogin() -> *mut c_char;
    pub fn getopt(argc: c_int, argv: *const *mut c_char, optstr: *const c_char) -> c_int;
    pub fn getpgid(pid: pid_t) -> pid_t;
    pub fn getpgrp() -> pid_t;
    pub fn getpid() -> pid_t;
    pub fn getppid() -> pid_t;
    pub fn getuid() -> uid_t;
    pub fn isatty(fd: c_int) -> c_int;
    pub fn link(src: *const c_char, dst: *const c_char) -> c_int;
    pub fn lseek(fd: c_int, offset: off_t, whence: c_int) -> off_t;
    pub fn pathconf(path: *const c_char, name: c_int) -> c_long;
    pub fn pause() -> c_int;
    pub fn pipe(fds: *mut c_int) -> c_int;
    pub fn posix_memalign(memptr: *mut *mut c_void, align: size_t, size: size_t) -> c_int;
    pub fn read(fd: c_int, buf: *mut c_void, count: size_t) -> ssize_t;
    pub fn rmdir(path: *const c_char) -> c_int;
    pub fn seteuid(uid: uid_t) -> c_int;
    pub fn setegid(gid: gid_t) -> c_int;
    pub fn setgid(gid: gid_t) -> c_int;
    pub fn setpgid(pid: pid_t, pgid: pid_t) -> c_int;
    pub fn setsid() -> pid_t;
    pub fn setuid(uid: uid_t) -> c_int;
    pub fn sleep(secs: c_uint) -> c_uint;
    pub fn nanosleep(rqtp: *const timespec, rmtp: *mut timespec) -> c_int;
    pub fn tcgetpgrp(fd: c_int) -> pid_t;
    pub fn tcsetpgrp(fd: c_int, pgrp: crate::pid_t) -> c_int;
    pub fn ttyname(fd: c_int) -> *mut c_char;
    pub fn unlink(c: *const c_char) -> c_int;
    pub fn wait(status: *mut c_int) -> pid_t;
    pub fn waitpid(pid: pid_t, status: *mut c_int, options: c_int) -> pid_t;
    pub fn write(fd: c_int, buf: *const c_void, count: size_t) -> ssize_t;
    pub fn pread(fd: c_int, buf: *mut c_void, count: size_t, offset: off_t) -> ssize_t;
    pub fn pwrite(fd: c_int, buf: *const c_void, count: size_t, offset: off_t) -> ssize_t;
    pub fn umask(mask: mode_t) -> mode_t;

    pub fn utime(file: *const c_char, buf: *const utimbuf) -> c_int;

    pub fn kill(pid: pid_t, sig: c_int) -> c_int;

    pub fn mlock(addr: *const c_void, len: size_t) -> c_int;
    pub fn munlock(addr: *const c_void, len: size_t) -> c_int;
    pub fn mlockall(flags: c_int) -> c_int;
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

    pub fn if_nametoindex(ifname: *const c_char) -> c_uint;
    pub fn if_indextoname(ifindex: c_uint, ifname: *mut c_char) -> *mut c_char;

    pub fn lstat(path: *const c_char, buf: *mut stat) -> c_int;

    pub fn fsync(fd: c_int) -> c_int;

    pub fn setenv(name: *const c_char, val: *const c_char, overwrite: c_int) -> c_int;
    pub fn unsetenv(name: *const c_char) -> c_int;

    pub fn symlink(path1: *const c_char, path2: *const c_char) -> c_int;

    pub fn ftruncate(fd: c_int, length: off_t) -> c_int;

    pub fn signal(signum: c_int, handler: sighandler_t) -> sighandler_t;

    pub fn realpath(pathname: *const c_char, resolved: *mut c_char) -> *mut c_char;

    pub fn flock(fd: c_int, operation: c_int) -> c_int;

    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut c_void) -> c_int;
    pub fn times(buf: *mut crate::tms) -> crate::clock_t;

    pub fn pthread_self() -> crate::pthread_t;
    pub fn pthread_join(native: crate::pthread_t, value: *mut *mut c_void) -> c_int;
    pub fn pthread_exit(value: *mut c_void) -> !;
    pub fn pthread_attr_init(attr: *mut crate::pthread_attr_t) -> c_int;
    pub fn pthread_attr_destroy(attr: *mut crate::pthread_attr_t) -> c_int;
    pub fn pthread_attr_getstacksize(
        attr: *const crate::pthread_attr_t,
        stacksize: *mut size_t,
    ) -> c_int;
    pub fn pthread_attr_setstacksize(attr: *mut crate::pthread_attr_t, stack_size: size_t)
        -> c_int;
    pub fn pthread_attr_setdetachstate(attr: *mut crate::pthread_attr_t, state: c_int) -> c_int;
    pub fn pthread_detach(thread: crate::pthread_t) -> c_int;
    pub fn sched_yield() -> c_int;
    pub fn pthread_key_create(
        key: *mut pthread_key_t,
        dtor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;
    pub fn pthread_key_delete(key: pthread_key_t) -> c_int;
    pub fn pthread_getspecific(key: pthread_key_t) -> *mut c_void;
    pub fn pthread_setspecific(key: pthread_key_t, value: *const c_void) -> c_int;
    pub fn pthread_mutex_init(
        lock: *mut pthread_mutex_t,
        attr: *const pthread_mutexattr_t,
    ) -> c_int;
    pub fn pthread_mutex_destroy(lock: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_mutex_lock(lock: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_mutex_trylock(lock: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_mutex_unlock(lock: *mut pthread_mutex_t) -> c_int;

    pub fn pthread_mutexattr_init(attr: *mut pthread_mutexattr_t) -> c_int;
    pub fn pthread_mutexattr_destroy(attr: *mut pthread_mutexattr_t) -> c_int;
    pub fn pthread_mutexattr_settype(attr: *mut pthread_mutexattr_t, _type: c_int) -> c_int;

    pub fn pthread_cond_init(cond: *mut pthread_cond_t, attr: *const pthread_condattr_t) -> c_int;
    pub fn pthread_cond_wait(cond: *mut pthread_cond_t, lock: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_cond_timedwait(
        cond: *mut pthread_cond_t,
        lock: *mut pthread_mutex_t,
        abstime: *const crate::timespec,
    ) -> c_int;
    pub fn pthread_cond_signal(cond: *mut pthread_cond_t) -> c_int;
    pub fn pthread_cond_broadcast(cond: *mut pthread_cond_t) -> c_int;
    pub fn pthread_cond_destroy(cond: *mut pthread_cond_t) -> c_int;
    pub fn pthread_condattr_init(attr: *mut pthread_condattr_t) -> c_int;
    pub fn pthread_condattr_destroy(attr: *mut pthread_condattr_t) -> c_int;
    pub fn pthread_rwlock_init(
        lock: *mut pthread_rwlock_t,
        attr: *const pthread_rwlockattr_t,
    ) -> c_int;
    pub fn pthread_rwlock_destroy(lock: *mut pthread_rwlock_t) -> c_int;
    pub fn pthread_rwlock_rdlock(lock: *mut pthread_rwlock_t) -> c_int;
    pub fn pthread_rwlock_tryrdlock(lock: *mut pthread_rwlock_t) -> c_int;
    pub fn pthread_rwlock_wrlock(lock: *mut pthread_rwlock_t) -> c_int;
    pub fn pthread_rwlock_trywrlock(lock: *mut pthread_rwlock_t) -> c_int;
    pub fn pthread_rwlock_unlock(lock: *mut pthread_rwlock_t) -> c_int;
    pub fn pthread_rwlockattr_init(attr: *mut pthread_rwlockattr_t) -> c_int;
    pub fn pthread_rwlockattr_destroy(attr: *mut pthread_rwlockattr_t) -> c_int;
    pub fn pthread_getname_np(thread: crate::pthread_t, name: *mut c_char, len: size_t) -> c_int;
    pub fn pthread_setname_np(thread: crate::pthread_t, name: *const c_char) -> c_int;
    pub fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;

    pub fn getsockopt(
        sockfd: c_int,
        level: c_int,
        optname: c_int,
        optval: *mut c_void,
        optlen: *mut crate::socklen_t,
    ) -> c_int;
    pub fn raise(signum: c_int) -> c_int;
    pub fn sigaction(signum: c_int, act: *const sigaction, oldact: *mut sigaction) -> c_int;

    pub fn utimes(filename: *const c_char, times: *const crate::timeval) -> c_int;
    pub fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    pub fn dlerror() -> *mut c_char;
    pub fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    pub fn dlclose(handle: *mut c_void) -> c_int;
    pub fn dladdr(addr: *const c_void, info: *mut Dl_info) -> c_int;

    pub fn getaddrinfo(
        node: *const c_char,
        service: *const c_char,
        hints: *const addrinfo,
        res: *mut *mut addrinfo,
    ) -> c_int;
    pub fn freeaddrinfo(res: *mut addrinfo);
    pub fn gai_strerror(errcode: c_int) -> *const c_char;
    pub fn res_init() -> c_int;

    pub fn gmtime_r(time_p: *const time_t, result: *mut tm) -> *mut tm;
    pub fn localtime_r(time_p: *const time_t, result: *mut tm) -> *mut tm;
    pub fn mktime(tm: *mut tm) -> time_t;
    pub fn time(time: *mut time_t) -> time_t;
    pub fn gmtime(time_p: *const time_t) -> *mut tm;
    pub fn localtime(time_p: *const time_t) -> *mut tm;

    pub fn mknod(pathname: *const c_char, mode: mode_t, dev: crate::dev_t) -> c_int;
    pub fn uname(buf: *mut crate::utsname) -> c_int;
    pub fn gethostname(name: *mut c_char, len: size_t) -> c_int;
    pub fn getservbyname(name: *const c_char, proto: *const c_char) -> *mut servent;
    pub fn getprotobyname(name: *const c_char) -> *mut protoent;
    pub fn getprotobynumber(proto: c_int) -> *mut protoent;
    pub fn usleep(secs: c_uint) -> c_int;
    pub fn send(socket: c_int, buf: *const c_void, len: size_t, flags: c_int) -> ssize_t;
    pub fn recv(socket: c_int, buf: *mut c_void, len: size_t, flags: c_int) -> ssize_t;
    pub fn putenv(string: *mut c_char) -> c_int;
    pub fn poll(fds: *mut pollfd, nfds: nfds_t, timeout: c_int) -> c_int;
    pub fn select(
        nfds: c_int,
        readfds: *mut fd_set,
        writefds: *mut fd_set,
        errorfds: *mut fd_set,
        timeout: *mut timeval,
    ) -> c_int;
    pub fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
    pub fn localeconv() -> *mut lconv;

    pub fn sem_destroy(sem: *mut sem_t) -> c_int;
    pub fn sem_wait(sem: *mut sem_t) -> c_int;
    pub fn sem_trywait(sem: *mut sem_t) -> c_int;
    pub fn sem_post(sem: *mut sem_t) -> c_int;
    pub fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;
    pub fn statvfs(path: *const c_char, buf: *mut statvfs) -> c_int;
    pub fn fstatvfs(fd: c_int, buf: *mut statvfs) -> c_int;

    pub fn readlink(path: *const c_char, buf: *mut c_char, bufsz: size_t) -> ssize_t;

    pub fn sigemptyset(set: *mut sigset_t) -> c_int;
    pub fn sigaddset(set: *mut sigset_t, signum: c_int) -> c_int;
    pub fn sigfillset(set: *mut sigset_t) -> c_int;
    pub fn sigdelset(set: *mut sigset_t, signum: c_int) -> c_int;
    pub fn sigismember(set: *const sigset_t, signum: c_int) -> c_int;

    pub fn sigprocmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t) -> c_int;
    pub fn sigpending(set: *mut sigset_t) -> c_int;

    pub fn timegm(tm: *mut crate::tm) -> time_t;

    pub fn getsid(pid: pid_t) -> pid_t;

    pub fn sysconf(name: c_int) -> c_long;

    pub fn mkfifo(path: *const c_char, mode: mode_t) -> c_int;

    pub fn pselect(
        nfds: c_int,
        readfds: *mut fd_set,
        writefds: *mut fd_set,
        errorfds: *mut fd_set,
        timeout: *const timespec,
        sigmask: *const sigset_t,
    ) -> c_int;
    pub fn fseeko(stream: *mut crate::FILE, offset: off_t, whence: c_int) -> c_int;
    pub fn ftello(stream: *mut crate::FILE) -> off_t;
    pub fn tcdrain(fd: c_int) -> c_int;
    pub fn cfgetispeed(termios: *const crate::termios) -> crate::speed_t;
    pub fn cfgetospeed(termios: *const crate::termios) -> crate::speed_t;
    pub fn cfmakeraw(termios: *mut crate::termios);
    pub fn cfsetispeed(termios: *mut crate::termios, speed: crate::speed_t) -> c_int;
    pub fn cfsetospeed(termios: *mut crate::termios, speed: crate::speed_t) -> c_int;
    pub fn cfsetspeed(termios: *mut crate::termios, speed: crate::speed_t) -> c_int;
    pub fn tcgetattr(fd: c_int, termios: *mut crate::termios) -> c_int;
    pub fn tcsetattr(fd: c_int, optional_actions: c_int, termios: *const crate::termios) -> c_int;
    pub fn tcflow(fd: c_int, action: c_int) -> c_int;
    pub fn tcflush(fd: c_int, action: c_int) -> c_int;
    pub fn tcgetsid(fd: c_int) -> crate::pid_t;
    pub fn tcsendbreak(fd: c_int, duration: c_int) -> c_int;
    pub fn mkstemp(template: *mut c_char) -> c_int;
    pub fn mkdtemp(template: *mut c_char) -> *mut c_char;

    pub fn tmpnam(ptr: *mut c_char) -> *mut c_char;

    pub fn openlog(ident: *const c_char, logopt: c_int, facility: c_int);
    pub fn closelog();
    pub fn setlogmask(maskpri: c_int) -> c_int;
    pub fn syslog(priority: c_int, message: *const c_char, ...);

    pub fn grantpt(fd: c_int) -> c_int;
    pub fn posix_openpt(flags: c_int) -> c_int;
    pub fn ptsname(fd: c_int) -> *mut c_char;
    pub fn unlockpt(fd: c_int) -> c_int;

    pub fn fdatasync(fd: c_int) -> c_int;
    pub fn clock_getres(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn clock_gettime(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn clock_settime(clk_id: crate::clockid_t, tp: *const crate::timespec) -> c_int;
    pub fn dirfd(dirp: *mut crate::DIR) -> c_int;

    pub fn pthread_getattr_np(native: crate::pthread_t, attr: *mut crate::pthread_attr_t) -> c_int;
    pub fn pthread_attr_getstack(
        attr: *const crate::pthread_attr_t,
        stackaddr: *mut *mut c_void,
        stacksize: *mut size_t,
    ) -> c_int;
    pub fn memalign(align: size_t, size: size_t) -> *mut c_void;
    pub fn setgroups(ngroups: size_t, ptr: *const crate::gid_t) -> c_int;
    pub fn pipe2(fds: *mut c_int, flags: c_int) -> c_int;
    pub fn statfs(path: *const c_char, buf: *mut statfs) -> c_int;
    pub fn fstatfs(fd: c_int, buf: *mut statfs) -> c_int;
    pub fn memrchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;

    pub fn posix_fadvise(fd: c_int, offset: off_t, len: off_t, advise: c_int) -> c_int;
    pub fn futimens(fd: c_int, times: *const crate::timespec) -> c_int;
    pub fn utimensat(
        dirfd: c_int,
        path: *const c_char,
        times: *const crate::timespec,
        flag: c_int,
    ) -> c_int;
    pub fn duplocale(base: crate::locale_t) -> crate::locale_t;
    pub fn freelocale(loc: crate::locale_t);
    pub fn newlocale(mask: c_int, locale: *const c_char, base: crate::locale_t) -> crate::locale_t;
    pub fn uselocale(loc: crate::locale_t) -> crate::locale_t;

    pub fn fdopendir(fd: c_int) -> *mut crate::DIR;

    pub fn mknodat(dirfd: c_int, pathname: *const c_char, mode: mode_t, dev: dev_t) -> c_int;
    pub fn pthread_condattr_getclock(
        attr: *const pthread_condattr_t,
        clock_id: *mut clockid_t,
    ) -> c_int;
    pub fn pthread_condattr_setclock(
        attr: *mut pthread_condattr_t,
        clock_id: crate::clockid_t,
    ) -> c_int;
    pub fn accept4(
        fd: c_int,
        addr: *mut crate::sockaddr,
        len: *mut crate::socklen_t,
        flg: c_int,
    ) -> c_int;
    pub fn ptsname_r(fd: c_int, buf: *mut c_char, buflen: size_t) -> c_int;
    pub fn clearenv() -> c_int;
    pub fn waitid(
        idtype: idtype_t,
        id: id_t,
        infop: *mut crate::siginfo_t,
        options: c_int,
    ) -> c_int;
    pub fn setreuid(ruid: crate::uid_t, euid: crate::uid_t) -> c_int;
    pub fn setregid(rgid: crate::gid_t, egid: crate::gid_t) -> c_int;
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
    pub fn acct(filename: *const c_char) -> c_int;
    pub fn brk(addr: *mut c_void) -> c_int;
    pub fn setresgid(rgid: crate::gid_t, egid: crate::gid_t, sgid: crate::gid_t) -> c_int;
    pub fn setresuid(ruid: crate::uid_t, euid: crate::uid_t, suid: crate::uid_t) -> c_int;
    pub fn openpty(
        amaster: *mut c_int,
        aslave: *mut c_int,
        name: *mut c_char,
        termp: *const termios,
        winp: *const crate::winsize,
    ) -> c_int;

    // DIFF(main): changed to `*const *mut` in e77f551de9
    pub fn execvpe(
        file: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int;
    pub fn fexecve(fd: c_int, argv: *const *const c_char, envp: *const *const c_char) -> c_int;

    pub fn ioctl(fd: c_int, request: c_int, ...) -> c_int;

    pub fn lutimes(file: *const c_char, times: *const crate::timeval) -> c_int;

    pub fn setpwent();
    pub fn endpwent();
    pub fn getpwent() -> *mut passwd;

    pub fn shm_open(name: *const c_char, oflag: c_int, mode: mode_t) -> c_int;

    // System V IPC
    pub fn shmget(key: crate::key_t, size: size_t, shmflg: c_int) -> c_int;
    pub fn shmat(shmid: c_int, shmaddr: *const c_void, shmflg: c_int) -> *mut c_void;
    pub fn shmdt(shmaddr: *const c_void) -> c_int;
    pub fn shmctl(shmid: c_int, cmd: c_int, buf: *mut crate::shmid_ds) -> c_int;
    pub fn ftok(pathname: *const c_char, proj_id: c_int) -> crate::key_t;
    pub fn semget(key: crate::key_t, nsems: c_int, semflag: c_int) -> c_int;
    pub fn semop(semid: c_int, sops: *mut crate::sembuf, nsops: size_t) -> c_int;
    pub fn semctl(semid: c_int, semnum: c_int, cmd: c_int, ...) -> c_int;
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

    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;
    pub fn __errno_location() -> *mut c_int;

    pub fn fallocate(fd: c_int, mode: c_int, offset: off_t, len: off_t) -> c_int;
    pub fn posix_fallocate(fd: c_int, offset: off_t, len: off_t) -> c_int;
    pub fn readahead(fd: c_int, offset: off64_t, count: size_t) -> ssize_t;
    pub fn signalfd(fd: c_int, mask: *const crate::sigset_t, flags: c_int) -> c_int;
    pub fn timerfd_create(clockid: c_int, flags: c_int) -> c_int;
    pub fn timerfd_gettime(fd: c_int, curr_value: *mut itimerspec) -> c_int;
    pub fn timerfd_settime(
        fd: c_int,
        flags: c_int,
        new_value: *const itimerspec,
        old_value: *mut itimerspec,
    ) -> c_int;
    pub fn pwritev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off_t) -> ssize_t;
    pub fn preadv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int, offset: off_t) -> ssize_t;
    pub fn quotactl(cmd: c_int, special: *const c_char, id: c_int, data: *mut c_char) -> c_int;
    pub fn dup3(oldfd: c_int, newfd: c_int, flags: c_int) -> c_int;
    pub fn mkostemp(template: *mut c_char, flags: c_int) -> c_int;
    pub fn mkostemps(template: *mut c_char, suffixlen: c_int, flags: c_int) -> c_int;
    pub fn sigtimedwait(
        set: *const sigset_t,
        info: *mut siginfo_t,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn sigwaitinfo(set: *const sigset_t, info: *mut siginfo_t) -> c_int;
    pub fn nl_langinfo_l(item: crate::nl_item, locale: crate::locale_t) -> *mut c_char;
    pub fn getnameinfo(
        sa: *const crate::sockaddr,
        salen: crate::socklen_t,
        host: *mut c_char,
        hostlen: crate::socklen_t,
        serv: *mut c_char,
        servlen: crate::socklen_t,
        flags: c_int,
    ) -> c_int;
    pub fn reboot(how_to: c_int) -> c_int;
    pub fn setfsgid(gid: crate::gid_t) -> c_int;
    pub fn setfsuid(uid: crate::uid_t) -> c_int;

    // Not available now on Android
    pub fn mkfifoat(dirfd: c_int, pathname: *const c_char, mode: mode_t) -> c_int;
    pub fn if_nameindex() -> *mut if_nameindex;
    pub fn if_freenameindex(ptr: *mut if_nameindex);
    pub fn sync_file_range(fd: c_int, offset: off64_t, nbytes: off64_t, flags: c_uint) -> c_int;
    pub fn getifaddrs(ifap: *mut *mut crate::ifaddrs) -> c_int;
    pub fn freeifaddrs(ifa: *mut crate::ifaddrs);

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
    pub fn futimes(fd: c_int, times: *const crate::timeval) -> c_int;
    pub fn nl_langinfo(item: crate::nl_item) -> *mut c_char;

    pub fn bind(
        socket: c_int,
        address: *const crate::sockaddr,
        address_len: crate::socklen_t,
    ) -> c_int;

    pub fn writev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;
    pub fn readv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;

    pub fn sendmsg(fd: c_int, msg: *const crate::msghdr, flags: c_int) -> ssize_t;
    pub fn recvmsg(fd: c_int, msg: *mut crate::msghdr, flags: c_int) -> ssize_t;
    pub fn getdomainname(name: *mut c_char, len: size_t) -> c_int;
    pub fn setdomainname(name: *const c_char, len: size_t) -> c_int;
    pub fn vhangup() -> c_int;
    pub fn sendmmsg(sockfd: c_int, msgvec: *mut mmsghdr, vlen: c_uint, flags: c_int) -> c_int;
    pub fn recvmmsg(
        sockfd: c_int,
        msgvec: *mut mmsghdr,
        vlen: c_uint,
        flags: c_int,
        timeout: *mut crate::timespec,
    ) -> c_int;
    pub fn sync();
    pub fn syscall(num: c_long, ...) -> c_long;
    pub fn sched_getaffinity(
        pid: crate::pid_t,
        cpusetsize: size_t,
        cpuset: *mut cpu_set_t,
    ) -> c_int;
    pub fn sched_setaffinity(
        pid: crate::pid_t,
        cpusetsize: size_t,
        cpuset: *const cpu_set_t,
    ) -> c_int;
    pub fn umount(target: *const c_char) -> c_int;
    pub fn sched_get_priority_max(policy: c_int) -> c_int;
    pub fn tee(fd_in: c_int, fd_out: c_int, len: size_t, flags: c_uint) -> ssize_t;
    pub fn settimeofday(tv: *const crate::timeval, tz: *const crate::timezone) -> c_int;
    pub fn splice(
        fd_in: c_int,
        off_in: *mut crate::loff_t,
        fd_out: c_int,
        off_out: *mut crate::loff_t,
        len: size_t,
        flags: c_uint,
    ) -> ssize_t;
    pub fn eventfd(init: c_uint, flags: c_int) -> c_int;
    pub fn sched_rr_get_interval(pid: crate::pid_t, tp: *mut crate::timespec) -> c_int;
    pub fn sem_timedwait(sem: *mut sem_t, abstime: *const crate::timespec) -> c_int;
    pub fn sem_getvalue(sem: *mut sem_t, sval: *mut c_int) -> c_int;
    pub fn sched_setparam(pid: crate::pid_t, param: *const crate::sched_param) -> c_int;
    pub fn swapoff(puath: *const c_char) -> c_int;
    pub fn vmsplice(fd: c_int, iov: *const crate::iovec, nr_segs: size_t, flags: c_uint)
        -> ssize_t;
    pub fn mount(
        src: *const c_char,
        target: *const c_char,
        fstype: *const c_char,
        flags: c_ulong,
        data: *const c_void,
    ) -> c_int;
    pub fn personality(persona: c_ulong) -> c_int;
    pub fn sched_getparam(pid: crate::pid_t, param: *mut crate::sched_param) -> c_int;
    pub fn ppoll(
        fds: *mut crate::pollfd,
        nfds: nfds_t,
        timeout: *const crate::timespec,
        sigmask: *const sigset_t,
    ) -> c_int;
    pub fn pthread_mutex_timedlock(
        lock: *mut pthread_mutex_t,
        abstime: *const crate::timespec,
    ) -> c_int;
    pub fn clone(
        cb: extern "C" fn(*mut c_void) -> c_int,
        child_stack: *mut c_void,
        flags: c_int,
        arg: *mut c_void,
        ...
    ) -> c_int;
    pub fn sched_getscheduler(pid: crate::pid_t) -> c_int;
    pub fn clock_nanosleep(
        clk_id: crate::clockid_t,
        flags: c_int,
        rqtp: *const crate::timespec,
        rmtp: *mut crate::timespec,
    ) -> c_int;
    pub fn pthread_attr_getguardsize(
        attr: *const crate::pthread_attr_t,
        guardsize: *mut size_t,
    ) -> c_int;
    pub fn pthread_attr_setguardsize(attr: *mut crate::pthread_attr_t, guardsize: size_t) -> c_int;
    pub fn sethostname(name: *const c_char, len: size_t) -> c_int;
    pub fn sched_get_priority_min(policy: c_int) -> c_int;
    pub fn umount2(target: *const c_char, flags: c_int) -> c_int;
    pub fn swapon(path: *const c_char, swapflags: c_int) -> c_int;
    pub fn sched_setscheduler(
        pid: crate::pid_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;
    pub fn sigsuspend(mask: *const crate::sigset_t) -> c_int;
    pub fn getgrgid_r(
        gid: crate::gid_t,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn sigaltstack(ss: *const stack_t, oss: *mut stack_t) -> c_int;
    pub fn sem_close(sem: *mut sem_t) -> c_int;
    pub fn getdtablesize() -> c_int;
    pub fn getgrnam_r(
        name: *const c_char,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn initgroups(user: *const c_char, group: crate::gid_t) -> c_int;
    pub fn pthread_sigmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t) -> c_int;
    pub fn sem_open(name: *const c_char, oflag: c_int, ...) -> *mut sem_t;
    pub fn getgrnam(name: *const c_char) -> *mut crate::group;
    pub fn pthread_cancel(thread: crate::pthread_t) -> c_int;
    pub fn pthread_kill(thread: crate::pthread_t, sig: c_int) -> c_int;
    pub fn sem_unlink(name: *const c_char) -> c_int;
    pub fn daemon(nochdir: c_int, noclose: c_int) -> c_int;
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
    pub fn sigwait(set: *const sigset_t, sig: *mut c_int) -> c_int;
    pub fn pthread_atfork(
        prepare: Option<unsafe extern "C" fn()>,
        parent: Option<unsafe extern "C" fn()>,
        child: Option<unsafe extern "C" fn()>,
    ) -> c_int;
    pub fn getgrgid(gid: crate::gid_t) -> *mut crate::group;

    pub fn setgrent();
    pub fn endgrent();
    pub fn getgrent() -> *mut crate::group;

    pub fn getgrouplist(
        user: *const c_char,
        group: crate::gid_t,
        groups: *mut crate::gid_t,
        ngroups: *mut c_int,
    ) -> c_int;
    pub fn popen(command: *const c_char, mode: *const c_char) -> *mut crate::FILE;
    pub fn faccessat(dirfd: c_int, pathname: *const c_char, mode: c_int, flags: c_int) -> c_int;
    pub fn pthread_create(
        native: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        f: extern "C" fn(*mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;
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
}

cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(any(target_arch = "x86_64"))] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(any(target_arch = "riscv64"))] {
        mod riscv64;
        pub use self::riscv64::*;
    } else {
        // Unknown target_arch
    }
}
