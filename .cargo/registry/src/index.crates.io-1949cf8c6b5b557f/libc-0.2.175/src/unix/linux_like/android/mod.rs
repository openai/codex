//! Android-specific definitions for linux-like values

use crate::prelude::*;
use crate::{cmsghdr, msghdr};

cfg_if! {
    if #[cfg(doc)] {
        pub(crate) type Ioctl = c_int;
    } else {
        #[doc(hidden)]
        pub type Ioctl = c_int;
    }
}

pub type clock_t = c_long;
pub type time_t = c_long;
pub type suseconds_t = c_long;
pub type off_t = c_long;
pub type blkcnt_t = c_ulong;
pub type blksize_t = c_ulong;
pub type nlink_t = u32;
pub type useconds_t = u32;
pub type pthread_t = c_long;
pub type pthread_mutexattr_t = c_long;
pub type pthread_rwlockattr_t = c_long;
pub type pthread_barrierattr_t = c_int;
pub type pthread_condattr_t = c_long;
pub type pthread_key_t = c_int;
pub type fsfilcnt_t = c_ulong;
pub type fsblkcnt_t = c_ulong;
pub type nfds_t = c_uint;
pub type rlim_t = c_ulong;
pub type dev_t = c_ulong;
pub type ino_t = c_ulong;
pub type ino64_t = u64;
pub type __CPU_BITTYPE = c_ulong;
pub type idtype_t = c_int;
pub type loff_t = c_longlong;
pub type __kernel_loff_t = c_longlong;
pub type __kernel_pid_t = c_int;

pub type __u8 = c_uchar;
pub type __u16 = c_ushort;
pub type __s16 = c_short;
pub type __u32 = c_uint;
pub type __s32 = c_int;

// linux/elf.h

pub type Elf32_Addr = u32;
pub type Elf32_Half = u16;
pub type Elf32_Off = u32;
pub type Elf32_Word = u32;

pub type Elf64_Addr = u64;
pub type Elf64_Half = u16;
pub type Elf64_Off = u64;
pub type Elf64_Word = u32;
pub type Elf64_Xword = u64;

pub type eventfd_t = u64;

// these structs sit behind a heap allocation on Android
pub type posix_spawn_file_actions_t = *mut c_void;
pub type posix_spawnattr_t = *mut c_void;

s! {
    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_flags: c_int,
        pub ss_size: size_t,
    }

    pub struct __fsid_t {
        __val: [c_int; 2],
    }

    pub struct termios {
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_line: crate::cc_t,
        pub c_cc: [crate::cc_t; crate::NCCS],
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

    pub struct mallinfo {
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

    pub struct flock {
        pub l_type: c_short,
        pub l_whence: c_short,
        pub l_start: off_t,
        pub l_len: off_t,
        pub l_pid: crate::pid_t,
    }

    pub struct flock64 {
        pub l_type: c_short,
        pub l_whence: c_short,
        pub l_start: crate::__kernel_loff_t,
        pub l_len: crate::__kernel_loff_t,
        pub l_pid: crate::__kernel_pid_t,
    }

    pub struct cpu_set_t {
        #[cfg(target_pointer_width = "64")]
        __bits: [__CPU_BITTYPE; 16],
        #[cfg(target_pointer_width = "32")]
        __bits: [__CPU_BITTYPE; 1],
    }

    pub struct sem_t {
        count: c_uint,
        #[cfg(target_pointer_width = "64")]
        __reserved: [c_int; 3],
    }

    pub struct exit_status {
        pub e_termination: c_short,
        pub e_exit: c_short,
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
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
        #[cfg(target_pointer_width = "64")]
        __f_reserved: [u32; 6],
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
        pub ssi_ptr: c_ulonglong,
        pub ssi_utime: c_ulonglong,
        pub ssi_stime: c_ulonglong,
        pub ssi_addr: c_ulonglong,
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

    pub struct genlmsghdr {
        pub cmd: u8,
        pub version: u8,
        pub reserved: u16,
    }

    pub struct nlmsghdr {
        pub nlmsg_len: u32,
        pub nlmsg_type: u16,
        pub nlmsg_flags: u16,
        pub nlmsg_seq: u32,
        pub nlmsg_pid: u32,
    }

    pub struct nlmsgerr {
        pub error: c_int,
        pub msg: nlmsghdr,
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

    pub struct nlattr {
        pub nla_len: u16,
        pub nla_type: u16,
    }

    pub struct in6_pktinfo {
        pub ipi6_addr: crate::in6_addr,
        pub ipi6_ifindex: c_int,
    }

    pub struct inotify_event {
        pub wd: c_int,
        pub mask: u32,
        pub cookie: u32,
        pub len: u32,
    }

    pub struct sock_extended_err {
        pub ee_errno: u32,
        pub ee_origin: u8,
        pub ee_type: u8,
        pub ee_code: u8,
        pub ee_pad: u8,
        pub ee_info: u32,
        pub ee_data: u32,
    }

    pub struct regex_t {
        re_magic: c_int,
        re_nsub: size_t,
        re_endp: *const c_char,
        re_guts: *mut c_void,
    }

    pub struct regmatch_t {
        pub rm_so: ssize_t,
        pub rm_eo: ssize_t,
    }

    pub struct sockaddr_vm {
        pub svm_family: crate::sa_family_t,
        pub svm_reserved1: c_ushort,
        pub svm_port: c_uint,
        pub svm_cid: c_uint,
        pub svm_zero: [u8; 4],
    }

    // linux/elf.h

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

    // link.h

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

        // These fields were added in Android R
        pub dlpi_adds: c_ulonglong,
        pub dlpi_subs: c_ulonglong,
        pub dlpi_tls_modid: size_t,
        pub dlpi_tls_data: *mut c_void,
    }

    // linux/seccomp.h
    pub struct seccomp_data {
        pub nr: c_int,
        pub arch: crate::__u32,
        pub instruction_pointer: crate::__u64,
        pub args: [crate::__u64; 6],
    }

    pub struct seccomp_metadata {
        pub filter_off: crate::__u64,
        pub flags: crate::__u64,
    }

    pub struct ptrace_peeksiginfo_args {
        pub off: crate::__u64,
        pub flags: crate::__u32,
        pub nr: crate::__s32,
    }

    // linux/input.h
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

    // linux/uinput.h
    pub struct uinput_ff_upload {
        pub request_id: crate::__u32,
        pub retval: crate::__s32,
        pub effect: ff_effect,
        pub old: ff_effect,
    }

    pub struct uinput_ff_erase {
        pub request_id: crate::__u32,
        pub retval: crate::__s32,
        pub effect_id: crate::__u32,
    }

    pub struct uinput_abs_setup {
        pub code: crate::__u16,
        pub absinfo: input_absinfo,
    }

    pub struct option {
        pub name: *const c_char,
        pub has_arg: c_int,
        pub flag: *mut c_int,
        pub val: c_int,
    }

    pub struct __c_anonymous_ifru_map {
        pub mem_start: c_ulong,
        pub mem_end: c_ulong,
        pub base_addr: c_ushort,
        pub irq: c_uchar,
        pub dma: c_uchar,
        pub port: c_uchar,
    }

    pub struct in6_ifreq {
        pub ifr6_addr: crate::in6_addr,
        pub ifr6_prefixlen: u32,
        pub ifr6_ifindex: c_int,
    }

    pub struct if_nameindex {
        pub if_index: c_uint,
        pub if_name: *mut c_char,
    }
}

s_no_extra_traits! {
    pub struct sockaddr_nl {
        pub nl_family: crate::sa_family_t,
        nl_pad: c_ushort,
        pub nl_pid: u32,
        pub nl_groups: u32,
    }

    pub struct dirent {
        pub d_ino: u64,
        pub d_off: i64,
        pub d_reclen: c_ushort,
        pub d_type: c_uchar,
        pub d_name: [c_char; 256],
    }

    pub struct dirent64 {
        pub d_ino: u64,
        pub d_off: i64,
        pub d_reclen: c_ushort,
        pub d_type: c_uchar,
        pub d_name: [c_char; 256],
    }

    pub struct siginfo_t {
        pub si_signo: c_int,
        pub si_errno: c_int,
        pub si_code: c_int,
        pub _pad: [c_int; 29],
        _align: [usize; 0],
    }

    pub struct lastlog {
        ll_time: crate::time_t,
        ll_line: [c_char; UT_LINESIZE],
        ll_host: [c_char; UT_HOSTSIZE],
    }

    pub struct utmp {
        pub ut_type: c_short,
        pub ut_pid: crate::pid_t,
        pub ut_line: [c_char; UT_LINESIZE],
        pub ut_id: [c_char; 4],
        pub ut_user: [c_char; UT_NAMESIZE],
        pub ut_host: [c_char; UT_HOSTSIZE],
        pub ut_exit: exit_status,
        pub ut_session: c_long,
        pub ut_tv: crate::timeval,
        pub ut_addr_v6: [i32; 4],
        unused: [c_char; 20],
    }

    pub struct sockaddr_alg {
        pub salg_family: crate::sa_family_t,
        pub salg_type: [c_uchar; 14],
        pub salg_feat: u32,
        pub salg_mask: u32,
        pub salg_name: [c_uchar; 64],
    }

    pub struct uinput_setup {
        pub id: input_id,
        pub name: [c_char; UINPUT_MAX_NAME_SIZE],
        pub ff_effects_max: crate::__u32,
    }

    pub struct uinput_user_dev {
        pub name: [c_char; UINPUT_MAX_NAME_SIZE],
        pub id: input_id,
        pub ff_effects_max: crate::__u32,
        pub absmax: [crate::__s32; ABS_CNT],
        pub absmin: [crate::__s32; ABS_CNT],
        pub absfuzz: [crate::__s32; ABS_CNT],
        pub absflat: [crate::__s32; ABS_CNT],
    }

    /// WARNING: The `PartialEq`, `Eq` and `Hash` implementations of this
    /// type are unsound and will be removed in the future.
    #[deprecated(
        note = "this struct has unsafe trait implementations that will be \
                removed in the future",
        since = "0.2.80"
    )]
    pub struct af_alg_iv {
        pub ivlen: u32,
        pub iv: [c_uchar; 0],
    }

    pub struct prop_info {
        __name: [c_char; 32],
        __serial: c_uint,
        __value: [c_char; 92],
    }

    pub union __c_anonymous_ifr_ifru {
        pub ifru_addr: crate::sockaddr,
        pub ifru_dstaddr: crate::sockaddr,
        pub ifru_broadaddr: crate::sockaddr,
        pub ifru_netmask: crate::sockaddr,
        pub ifru_hwaddr: crate::sockaddr,
        pub ifru_flags: c_short,
        pub ifru_ifindex: c_int,
        pub ifru_metric: c_int,
        pub ifru_mtu: c_int,
        pub ifru_map: __c_anonymous_ifru_map,
        pub ifru_slave: [c_char; crate::IFNAMSIZ],
        pub ifru_newname: [c_char; crate::IFNAMSIZ],
        pub ifru_data: *mut c_char,
    }

    pub struct ifreq {
        /// interface name, e.g. "en0"
        pub ifr_name: [c_char; crate::IFNAMSIZ],
        pub ifr_ifru: __c_anonymous_ifr_ifru,
    }

    pub union __c_anonymous_ifc_ifcu {
        pub ifcu_buf: *mut c_char,
        pub ifcu_req: *mut crate::ifreq,
    }

    /*  Structure used in SIOCGIFCONF request.  Used to retrieve interface
    configuration for machine (useful for programs which must know all
    networks accessible).  */
    pub struct ifconf {
        pub ifc_len: c_int, /* Size of buffer.  */
        pub ifc_ifcu: __c_anonymous_ifc_ifcu,
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
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

        impl PartialEq for siginfo_t {
            fn eq(&self, other: &siginfo_t) -> bool {
                self.si_signo == other.si_signo
                    && self.si_errno == other.si_errno
                    && self.si_code == other.si_code
                // Ignore _pad
                // Ignore _align
            }
        }

        impl Eq for siginfo_t {}

        impl hash::Hash for siginfo_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.si_signo.hash(state);
                self.si_errno.hash(state);
                self.si_code.hash(state);
                // Ignore _pad
                // Ignore _align
            }
        }

        impl PartialEq for lastlog {
            fn eq(&self, other: &lastlog) -> bool {
                self.ll_time == other.ll_time
                    && self
                        .ll_line
                        .iter()
                        .zip(other.ll_line.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .ll_host
                        .iter()
                        .zip(other.ll_host.iter())
                        .all(|(a, b)| a == b)
            }
        }

        impl Eq for lastlog {}

        impl hash::Hash for lastlog {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ll_time.hash(state);
                self.ll_line.hash(state);
                self.ll_host.hash(state);
            }
        }

        impl PartialEq for utmp {
            fn eq(&self, other: &utmp) -> bool {
                self.ut_type == other.ut_type
                    && self.ut_pid == other.ut_pid
                    && self
                        .ut_line
                        .iter()
                        .zip(other.ut_line.iter())
                        .all(|(a, b)| a == b)
                    && self.ut_id == other.ut_id
                    && self
                        .ut_user
                        .iter()
                        .zip(other.ut_user.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .ut_host
                        .iter()
                        .zip(other.ut_host.iter())
                        .all(|(a, b)| a == b)
                    && self.ut_exit == other.ut_exit
                    && self.ut_session == other.ut_session
                    && self.ut_tv == other.ut_tv
                    && self.ut_addr_v6 == other.ut_addr_v6
                    && self.unused == other.unused
            }
        }

        impl Eq for utmp {}

        impl hash::Hash for utmp {
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
                self.unused.hash(state);
            }
        }

        impl PartialEq for sockaddr_alg {
            fn eq(&self, other: &sockaddr_alg) -> bool {
                self.salg_family == other.salg_family
                    && self
                        .salg_type
                        .iter()
                        .zip(other.salg_type.iter())
                        .all(|(a, b)| a == b)
                    && self.salg_feat == other.salg_feat
                    && self.salg_mask == other.salg_mask
                    && self
                        .salg_name
                        .iter()
                        .zip(other.salg_name.iter())
                        .all(|(a, b)| a == b)
            }
        }

        impl Eq for sockaddr_alg {}

        impl hash::Hash for sockaddr_alg {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.salg_family.hash(state);
                self.salg_type.hash(state);
                self.salg_feat.hash(state);
                self.salg_mask.hash(state);
                self.salg_name.hash(state);
            }
        }

        impl PartialEq for uinput_setup {
            fn eq(&self, other: &uinput_setup) -> bool {
                self.id == other.id
                    && self.name[..] == other.name[..]
                    && self.ff_effects_max == other.ff_effects_max
            }
        }
        impl Eq for uinput_setup {}

        impl hash::Hash for uinput_setup {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.id.hash(state);
                self.name.hash(state);
                self.ff_effects_max.hash(state);
            }
        }

        impl PartialEq for uinput_user_dev {
            fn eq(&self, other: &uinput_user_dev) -> bool {
                self.name[..] == other.name[..]
                    && self.id == other.id
                    && self.ff_effects_max == other.ff_effects_max
                    && self.absmax[..] == other.absmax[..]
                    && self.absmin[..] == other.absmin[..]
                    && self.absfuzz[..] == other.absfuzz[..]
                    && self.absflat[..] == other.absflat[..]
            }
        }
        impl Eq for uinput_user_dev {}

        impl hash::Hash for uinput_user_dev {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.name.hash(state);
                self.id.hash(state);
                self.ff_effects_max.hash(state);
                self.absmax.hash(state);
                self.absmin.hash(state);
                self.absfuzz.hash(state);
                self.absflat.hash(state);
            }
        }

        #[allow(deprecated)]
        impl af_alg_iv {
            fn as_slice(&self) -> &[u8] {
                unsafe { ::core::slice::from_raw_parts(self.iv.as_ptr(), self.ivlen as usize) }
            }
        }

        #[allow(deprecated)]
        impl PartialEq for af_alg_iv {
            fn eq(&self, other: &af_alg_iv) -> bool {
                *self.as_slice() == *other.as_slice()
            }
        }

        #[allow(deprecated)]
        impl Eq for af_alg_iv {}

        #[allow(deprecated)]
        impl hash::Hash for af_alg_iv {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.as_slice().hash(state);
            }
        }

        impl PartialEq for prop_info {
            fn eq(&self, other: &prop_info) -> bool {
                self.__name == other.__name
                    && self.__serial == other.__serial
                    && self.__value == other.__value
            }
        }
        impl Eq for prop_info {}
    }
}

pub const MADV_SOFT_OFFLINE: c_int = 101;
pub const MS_NOUSER: c_ulong = 0xffffffff80000000;
pub const MS_RMT_MASK: c_ulong = 0x02800051;

pub const O_TRUNC: c_int = 512;
pub const O_CLOEXEC: c_int = 0x80000;
pub const O_PATH: c_int = 0o10000000;
pub const O_NOATIME: c_int = 0o1000000;

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

pub const EPOLL_CLOEXEC: c_int = 0x80000;

// sys/eventfd.h
pub const EFD_SEMAPHORE: c_int = 0x1;
pub const EFD_CLOEXEC: c_int = O_CLOEXEC;
pub const EFD_NONBLOCK: c_int = O_NONBLOCK;

// sys/timerfd.h
pub const TFD_CLOEXEC: c_int = O_CLOEXEC;
pub const TFD_NONBLOCK: c_int = O_NONBLOCK;
pub const TFD_TIMER_ABSTIME: c_int = 1;
pub const TFD_TIMER_CANCEL_ON_SET: c_int = 2;

pub const USER_PROCESS: c_short = 7;

pub const _POSIX_VDISABLE: crate::cc_t = 0;

// linux/falloc.h
pub const FALLOC_FL_KEEP_SIZE: c_int = 0x01;
pub const FALLOC_FL_PUNCH_HOLE: c_int = 0x02;
pub const FALLOC_FL_NO_HIDE_STALE: c_int = 0x04;
pub const FALLOC_FL_COLLAPSE_RANGE: c_int = 0x08;
pub const FALLOC_FL_ZERO_RANGE: c_int = 0x10;
pub const FALLOC_FL_INSERT_RANGE: c_int = 0x20;
pub const FALLOC_FL_UNSHARE_RANGE: c_int = 0x40;

pub const BUFSIZ: c_uint = 1024;
pub const FILENAME_MAX: c_uint = 4096;
pub const FOPEN_MAX: c_uint = 20;
pub const POSIX_FADV_DONTNEED: c_int = 4;
pub const POSIX_FADV_NOREUSE: c_int = 5;
pub const L_tmpnam: c_uint = 4096;
pub const TMP_MAX: c_uint = 308915776;
pub const _PC_LINK_MAX: c_int = 1;
pub const _PC_MAX_CANON: c_int = 2;
pub const _PC_MAX_INPUT: c_int = 3;
pub const _PC_NAME_MAX: c_int = 4;
pub const _PC_PATH_MAX: c_int = 5;
pub const _PC_PIPE_BUF: c_int = 6;
pub const _PC_2_SYMLINKS: c_int = 7;
pub const _PC_ALLOC_SIZE_MIN: c_int = 8;
pub const _PC_REC_INCR_XFER_SIZE: c_int = 9;
pub const _PC_REC_MAX_XFER_SIZE: c_int = 10;
pub const _PC_REC_MIN_XFER_SIZE: c_int = 11;
pub const _PC_REC_XFER_ALIGN: c_int = 12;
pub const _PC_SYMLINK_MAX: c_int = 13;
pub const _PC_CHOWN_RESTRICTED: c_int = 14;
pub const _PC_NO_TRUNC: c_int = 15;
pub const _PC_VDISABLE: c_int = 16;
pub const _PC_ASYNC_IO: c_int = 17;
pub const _PC_PRIO_IO: c_int = 18;
pub const _PC_SYNC_IO: c_int = 19;

pub const FIONBIO: c_int = 0x5421;

pub const _SC_ARG_MAX: c_int = 0x0000;
pub const _SC_BC_BASE_MAX: c_int = 0x0001;
pub const _SC_BC_DIM_MAX: c_int = 0x0002;
pub const _SC_BC_SCALE_MAX: c_int = 0x0003;
pub const _SC_BC_STRING_MAX: c_int = 0x0004;
pub const _SC_CHILD_MAX: c_int = 0x0005;
pub const _SC_CLK_TCK: c_int = 0x0006;
pub const _SC_COLL_WEIGHTS_MAX: c_int = 0x0007;
pub const _SC_EXPR_NEST_MAX: c_int = 0x0008;
pub const _SC_LINE_MAX: c_int = 0x0009;
pub const _SC_NGROUPS_MAX: c_int = 0x000a;
pub const _SC_OPEN_MAX: c_int = 0x000b;
pub const _SC_PASS_MAX: c_int = 0x000c;
pub const _SC_2_C_BIND: c_int = 0x000d;
pub const _SC_2_C_DEV: c_int = 0x000e;
pub const _SC_2_C_VERSION: c_int = 0x000f;
pub const _SC_2_CHAR_TERM: c_int = 0x0010;
pub const _SC_2_FORT_DEV: c_int = 0x0011;
pub const _SC_2_FORT_RUN: c_int = 0x0012;
pub const _SC_2_LOCALEDEF: c_int = 0x0013;
pub const _SC_2_SW_DEV: c_int = 0x0014;
pub const _SC_2_UPE: c_int = 0x0015;
pub const _SC_2_VERSION: c_int = 0x0016;
pub const _SC_JOB_CONTROL: c_int = 0x0017;
pub const _SC_SAVED_IDS: c_int = 0x0018;
pub const _SC_VERSION: c_int = 0x0019;
pub const _SC_RE_DUP_MAX: c_int = 0x001a;
pub const _SC_STREAM_MAX: c_int = 0x001b;
pub const _SC_TZNAME_MAX: c_int = 0x001c;
pub const _SC_XOPEN_CRYPT: c_int = 0x001d;
pub const _SC_XOPEN_ENH_I18N: c_int = 0x001e;
pub const _SC_XOPEN_SHM: c_int = 0x001f;
pub const _SC_XOPEN_VERSION: c_int = 0x0020;
pub const _SC_XOPEN_XCU_VERSION: c_int = 0x0021;
pub const _SC_XOPEN_REALTIME: c_int = 0x0022;
pub const _SC_XOPEN_REALTIME_THREADS: c_int = 0x0023;
pub const _SC_XOPEN_LEGACY: c_int = 0x0024;
pub const _SC_ATEXIT_MAX: c_int = 0x0025;
pub const _SC_IOV_MAX: c_int = 0x0026;
pub const _SC_UIO_MAXIOV: c_int = _SC_IOV_MAX;
pub const _SC_PAGESIZE: c_int = 0x0027;
pub const _SC_PAGE_SIZE: c_int = 0x0028;
pub const _SC_XOPEN_UNIX: c_int = 0x0029;
pub const _SC_XBS5_ILP32_OFF32: c_int = 0x002a;
pub const _SC_XBS5_ILP32_OFFBIG: c_int = 0x002b;
pub const _SC_XBS5_LP64_OFF64: c_int = 0x002c;
pub const _SC_XBS5_LPBIG_OFFBIG: c_int = 0x002d;
pub const _SC_AIO_LISTIO_MAX: c_int = 0x002e;
pub const _SC_AIO_MAX: c_int = 0x002f;
pub const _SC_AIO_PRIO_DELTA_MAX: c_int = 0x0030;
pub const _SC_DELAYTIMER_MAX: c_int = 0x0031;
pub const _SC_MQ_OPEN_MAX: c_int = 0x0032;
pub const _SC_MQ_PRIO_MAX: c_int = 0x0033;
pub const _SC_RTSIG_MAX: c_int = 0x0034;
pub const _SC_SEM_NSEMS_MAX: c_int = 0x0035;
pub const _SC_SEM_VALUE_MAX: c_int = 0x0036;
pub const _SC_SIGQUEUE_MAX: c_int = 0x0037;
pub const _SC_TIMER_MAX: c_int = 0x0038;
pub const _SC_ASYNCHRONOUS_IO: c_int = 0x0039;
pub const _SC_FSYNC: c_int = 0x003a;
pub const _SC_MAPPED_FILES: c_int = 0x003b;
pub const _SC_MEMLOCK: c_int = 0x003c;
pub const _SC_MEMLOCK_RANGE: c_int = 0x003d;
pub const _SC_MEMORY_PROTECTION: c_int = 0x003e;
pub const _SC_MESSAGE_PASSING: c_int = 0x003f;
pub const _SC_PRIORITIZED_IO: c_int = 0x0040;
pub const _SC_PRIORITY_SCHEDULING: c_int = 0x0041;
pub const _SC_REALTIME_SIGNALS: c_int = 0x0042;
pub const _SC_SEMAPHORES: c_int = 0x0043;
pub const _SC_SHARED_MEMORY_OBJECTS: c_int = 0x0044;
pub const _SC_SYNCHRONIZED_IO: c_int = 0x0045;
pub const _SC_TIMERS: c_int = 0x0046;
pub const _SC_GETGR_R_SIZE_MAX: c_int = 0x0047;
pub const _SC_GETPW_R_SIZE_MAX: c_int = 0x0048;
pub const _SC_LOGIN_NAME_MAX: c_int = 0x0049;
pub const _SC_THREAD_DESTRUCTOR_ITERATIONS: c_int = 0x004a;
pub const _SC_THREAD_KEYS_MAX: c_int = 0x004b;
pub const _SC_THREAD_STACK_MIN: c_int = 0x004c;
pub const _SC_THREAD_THREADS_MAX: c_int = 0x004d;
pub const _SC_TTY_NAME_MAX: c_int = 0x004e;
pub const _SC_THREADS: c_int = 0x004f;
pub const _SC_THREAD_ATTR_STACKADDR: c_int = 0x0050;
pub const _SC_THREAD_ATTR_STACKSIZE: c_int = 0x0051;
pub const _SC_THREAD_PRIORITY_SCHEDULING: c_int = 0x0052;
pub const _SC_THREAD_PRIO_INHERIT: c_int = 0x0053;
pub const _SC_THREAD_PRIO_PROTECT: c_int = 0x0054;
pub const _SC_THREAD_SAFE_FUNCTIONS: c_int = 0x0055;
pub const _SC_NPROCESSORS_CONF: c_int = 0x0060;
pub const _SC_NPROCESSORS_ONLN: c_int = 0x0061;
pub const _SC_PHYS_PAGES: c_int = 0x0062;
pub const _SC_AVPHYS_PAGES: c_int = 0x0063;
pub const _SC_MONOTONIC_CLOCK: c_int = 0x0064;
pub const _SC_2_PBS: c_int = 0x0065;
pub const _SC_2_PBS_ACCOUNTING: c_int = 0x0066;
pub const _SC_2_PBS_CHECKPOINT: c_int = 0x0067;
pub const _SC_2_PBS_LOCATE: c_int = 0x0068;
pub const _SC_2_PBS_MESSAGE: c_int = 0x0069;
pub const _SC_2_PBS_TRACK: c_int = 0x006a;
pub const _SC_ADVISORY_INFO: c_int = 0x006b;
pub const _SC_BARRIERS: c_int = 0x006c;
pub const _SC_CLOCK_SELECTION: c_int = 0x006d;
pub const _SC_CPUTIME: c_int = 0x006e;
pub const _SC_HOST_NAME_MAX: c_int = 0x006f;
pub const _SC_IPV6: c_int = 0x0070;
pub const _SC_RAW_SOCKETS: c_int = 0x0071;
pub const _SC_READER_WRITER_LOCKS: c_int = 0x0072;
pub const _SC_REGEXP: c_int = 0x0073;
pub const _SC_SHELL: c_int = 0x0074;
pub const _SC_SPAWN: c_int = 0x0075;
pub const _SC_SPIN_LOCKS: c_int = 0x0076;
pub const _SC_SPORADIC_SERVER: c_int = 0x0077;
pub const _SC_SS_REPL_MAX: c_int = 0x0078;
pub const _SC_SYMLOOP_MAX: c_int = 0x0079;
pub const _SC_THREAD_CPUTIME: c_int = 0x007a;
pub const _SC_THREAD_PROCESS_SHARED: c_int = 0x007b;
pub const _SC_THREAD_ROBUST_PRIO_INHERIT: c_int = 0x007c;
pub const _SC_THREAD_ROBUST_PRIO_PROTECT: c_int = 0x007d;
pub const _SC_THREAD_SPORADIC_SERVER: c_int = 0x007e;
pub const _SC_TIMEOUTS: c_int = 0x007f;
pub const _SC_TRACE: c_int = 0x0080;
pub const _SC_TRACE_EVENT_FILTER: c_int = 0x0081;
pub const _SC_TRACE_EVENT_NAME_MAX: c_int = 0x0082;
pub const _SC_TRACE_INHERIT: c_int = 0x0083;
pub const _SC_TRACE_LOG: c_int = 0x0084;
pub const _SC_TRACE_NAME_MAX: c_int = 0x0085;
pub const _SC_TRACE_SYS_MAX: c_int = 0x0086;
pub const _SC_TRACE_USER_EVENT_MAX: c_int = 0x0087;
pub const _SC_TYPED_MEMORY_OBJECTS: c_int = 0x0088;
pub const _SC_V7_ILP32_OFF32: c_int = 0x0089;
pub const _SC_V7_ILP32_OFFBIG: c_int = 0x008a;
pub const _SC_V7_LP64_OFF64: c_int = 0x008b;
pub const _SC_V7_LPBIG_OFFBIG: c_int = 0x008c;
pub const _SC_XOPEN_STREAMS: c_int = 0x008d;
pub const _SC_XOPEN_UUCP: c_int = 0x008e;
pub const _SC_LEVEL1_ICACHE_SIZE: c_int = 0x008f;
pub const _SC_LEVEL1_ICACHE_ASSOC: c_int = 0x0090;
pub const _SC_LEVEL1_ICACHE_LINESIZE: c_int = 0x0091;
pub const _SC_LEVEL1_DCACHE_SIZE: c_int = 0x0092;
pub const _SC_LEVEL1_DCACHE_ASSOC: c_int = 0x0093;
pub const _SC_LEVEL1_DCACHE_LINESIZE: c_int = 0x0094;
pub const _SC_LEVEL2_CACHE_SIZE: c_int = 0x0095;
pub const _SC_LEVEL2_CACHE_ASSOC: c_int = 0x0096;
pub const _SC_LEVEL2_CACHE_LINESIZE: c_int = 0x0097;
pub const _SC_LEVEL3_CACHE_SIZE: c_int = 0x0098;
pub const _SC_LEVEL3_CACHE_ASSOC: c_int = 0x0099;
pub const _SC_LEVEL3_CACHE_LINESIZE: c_int = 0x009a;
pub const _SC_LEVEL4_CACHE_SIZE: c_int = 0x009b;
pub const _SC_LEVEL4_CACHE_ASSOC: c_int = 0x009c;
pub const _SC_LEVEL4_CACHE_LINESIZE: c_int = 0x009d;

pub const F_LOCK: c_int = 1;
pub const F_TEST: c_int = 3;
pub const F_TLOCK: c_int = 2;
pub const F_ULOCK: c_int = 0;

pub const F_SEAL_FUTURE_WRITE: c_int = 0x0010;
pub const F_SEAL_EXEC: c_int = 0x0020;

pub const IFF_LOWER_UP: c_int = 0x10000;
pub const IFF_DORMANT: c_int = 0x20000;
pub const IFF_ECHO: c_int = 0x40000;

pub const PTHREAD_BARRIER_SERIAL_THREAD: c_int = -1;
pub const PTHREAD_MUTEX_NORMAL: c_int = 0;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 1;
pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 2;
pub const PTHREAD_MUTEX_DEFAULT: c_int = PTHREAD_MUTEX_NORMAL;

pub const PTHREAD_EXPLICIT_SCHED: c_int = 0;
pub const PTHREAD_INHERIT_SCHED: c_int = 1;

// stdio.h
pub const RENAME_NOREPLACE: c_int = 1;
pub const RENAME_EXCHANGE: c_int = 2;
pub const RENAME_WHITEOUT: c_int = 4;

pub const FIOCLEX: c_int = 0x5451;
pub const FIONCLEX: c_int = 0x5450;

pub const SIGCHLD: c_int = 17;
pub const SIGBUS: c_int = 7;
pub const SIGUSR1: c_int = 10;
pub const SIGUSR2: c_int = 12;
pub const SIGCONT: c_int = 18;
pub const SIGSTOP: c_int = 19;
pub const SIGTSTP: c_int = 20;
pub const SIGURG: c_int = 23;
pub const SIGIO: c_int = 29;
pub const SIGSYS: c_int = 31;
pub const SIGSTKFLT: c_int = 16;
#[deprecated(since = "0.2.55", note = "Use SIGSYS instead")]
pub const SIGUNUSED: c_int = 31;
pub const SIGTTIN: c_int = 21;
pub const SIGTTOU: c_int = 22;
pub const SIGXCPU: c_int = 24;
pub const SIGXFSZ: c_int = 25;
pub const SIGVTALRM: c_int = 26;
pub const SIGPROF: c_int = 27;
pub const SIGWINCH: c_int = 28;
pub const SIGPOLL: c_int = 29;
pub const SIGPWR: c_int = 30;
pub const SIG_SETMASK: c_int = 2;
pub const SIG_BLOCK: c_int = 0x000000;
pub const SIG_UNBLOCK: c_int = 0x01;

pub const RUSAGE_CHILDREN: c_int = -1;

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

pub const MAP_ANON: c_int = 0x0020;
pub const MAP_ANONYMOUS: c_int = 0x0020;
pub const MAP_GROWSDOWN: c_int = 0x0100;
pub const MAP_DENYWRITE: c_int = 0x0800;
pub const MAP_EXECUTABLE: c_int = 0x01000;
pub const MAP_LOCKED: c_int = 0x02000;
pub const MAP_NORESERVE: c_int = 0x04000;
pub const MAP_POPULATE: c_int = 0x08000;
pub const MAP_NONBLOCK: c_int = 0x010000;
pub const MAP_STACK: c_int = 0x020000;

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

pub const SOCK_STREAM: c_int = 1;
pub const SOCK_DGRAM: c_int = 2;
pub const SOCK_SEQPACKET: c_int = 5;
pub const SOCK_DCCP: c_int = 6;
#[deprecated(since = "0.2.70", note = "AF_PACKET must be used instead")]
pub const SOCK_PACKET: c_int = 10;

pub const IPPROTO_MAX: c_int = 256;

pub const SOL_SOCKET: c_int = 1;
pub const SOL_SCTP: c_int = 132;
pub const SOL_IPX: c_int = 256;
pub const SOL_AX25: c_int = 257;
pub const SOL_ATALK: c_int = 258;
pub const SOL_NETROM: c_int = 259;
pub const SOL_ROSE: c_int = 260;

/* UDP socket options */
// include/uapi/linux/udp.h
pub const UDP_CORK: c_int = 1;
pub const UDP_ENCAP: c_int = 100;
pub const UDP_NO_CHECK6_TX: c_int = 101;
pub const UDP_NO_CHECK6_RX: c_int = 102;
pub const UDP_SEGMENT: c_int = 103;
pub const UDP_GRO: c_int = 104;

/* DCCP socket options */
pub const DCCP_SOCKOPT_PACKET_SIZE: c_int = 1;
pub const DCCP_SOCKOPT_SERVICE: c_int = 2;
pub const DCCP_SOCKOPT_CHANGE_L: c_int = 3;
pub const DCCP_SOCKOPT_CHANGE_R: c_int = 4;
pub const DCCP_SOCKOPT_GET_CUR_MPS: c_int = 5;
pub const DCCP_SOCKOPT_SERVER_TIMEWAIT: c_int = 6;
pub const DCCP_SOCKOPT_SEND_CSCOV: c_int = 10;
pub const DCCP_SOCKOPT_RECV_CSCOV: c_int = 11;
pub const DCCP_SOCKOPT_AVAILABLE_CCIDS: c_int = 12;
pub const DCCP_SOCKOPT_CCID: c_int = 13;
pub const DCCP_SOCKOPT_TX_CCID: c_int = 14;
pub const DCCP_SOCKOPT_RX_CCID: c_int = 15;
pub const DCCP_SOCKOPT_QPOLICY_ID: c_int = 16;
pub const DCCP_SOCKOPT_QPOLICY_TXQLEN: c_int = 17;
pub const DCCP_SOCKOPT_CCID_RX_INFO: c_int = 128;
pub const DCCP_SOCKOPT_CCID_TX_INFO: c_int = 192;

/// maximum number of services provided on the same listening port
pub const DCCP_SERVICE_LIST_MAX_LEN: c_int = 32;

pub const SO_REUSEADDR: c_int = 2;
pub const SO_TYPE: c_int = 3;
pub const SO_ERROR: c_int = 4;
pub const SO_DONTROUTE: c_int = 5;
pub const SO_BROADCAST: c_int = 6;
pub const SO_SNDBUF: c_int = 7;
pub const SO_RCVBUF: c_int = 8;
pub const SO_KEEPALIVE: c_int = 9;
pub const SO_OOBINLINE: c_int = 10;
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
pub const SO_BINDTODEVICE: c_int = 25;
pub const SO_ATTACH_FILTER: c_int = 26;
pub const SO_DETACH_FILTER: c_int = 27;
pub const SO_GET_FILTER: c_int = SO_ATTACH_FILTER;
pub const SO_TIMESTAMP: c_int = 29;
pub const SO_ACCEPTCONN: c_int = 30;
pub const SO_PEERSEC: c_int = 31;
pub const SO_SNDBUFFORCE: c_int = 32;
pub const SO_RCVBUFFORCE: c_int = 33;
pub const SO_PASSSEC: c_int = 34;
pub const SO_TIMESTAMPNS: c_int = 35;
// pub const SO_TIMESTAMPNS_OLD: c_int = 35;
pub const SO_MARK: c_int = 36;
pub const SO_TIMESTAMPING: c_int = 37;
// pub const SO_TIMESTAMPING_OLD: c_int = 37;
pub const SO_PROTOCOL: c_int = 38;
pub const SO_DOMAIN: c_int = 39;
pub const SO_RXQ_OVFL: c_int = 40;
pub const SO_PEEK_OFF: c_int = 42;
pub const SO_BUSY_POLL: c_int = 46;
pub const SCM_TIMESTAMPING_OPT_STATS: c_int = 54;
pub const SCM_TIMESTAMPING_PKTINFO: c_int = 58;
pub const SO_BINDTOIFINDEX: c_int = 62;
pub const SO_TIMESTAMP_NEW: c_int = 63;
pub const SO_TIMESTAMPNS_NEW: c_int = 64;
pub const SO_TIMESTAMPING_NEW: c_int = 65;

// Defined in unix/linux_like/mod.rs
// pub const SCM_TIMESTAMP: c_int = SO_TIMESTAMP;
pub const SCM_TIMESTAMPNS: c_int = SO_TIMESTAMPNS;
pub const SCM_TIMESTAMPING: c_int = SO_TIMESTAMPING;

pub const IPTOS_ECN_NOTECT: u8 = 0x00;

pub const O_ACCMODE: c_int = 3;
pub const O_APPEND: c_int = 1024;
pub const O_CREAT: c_int = 64;
pub const O_EXCL: c_int = 128;
pub const O_NOCTTY: c_int = 256;
pub const O_NONBLOCK: c_int = 2048;
pub const O_SYNC: c_int = 0x101000;
pub const O_ASYNC: c_int = 0x2000;
pub const O_NDELAY: c_int = 0x800;
pub const O_DSYNC: c_int = 4096;
pub const O_RSYNC: c_int = O_SYNC;

pub const NI_MAXHOST: size_t = 1025;
pub const NI_MAXSERV: size_t = 32;

pub const NI_NOFQDN: c_int = 0x00000001;
pub const NI_NUMERICHOST: c_int = 0x00000002;
pub const NI_NAMEREQD: c_int = 0x00000004;
pub const NI_NUMERICSERV: c_int = 0x00000008;
pub const NI_DGRAM: c_int = 0x00000010;

pub const NCCS: usize = 19;
pub const TCSBRKP: c_int = 0x5425;
pub const TCSANOW: c_int = 0;
pub const TCSADRAIN: c_int = 0x1;
pub const TCSAFLUSH: c_int = 0x2;
pub const VEOF: usize = 4;
pub const VEOL: usize = 11;
pub const VEOL2: usize = 16;
pub const VMIN: usize = 6;
pub const IEXTEN: crate::tcflag_t = 0x00008000;
pub const TOSTOP: crate::tcflag_t = 0x00000100;
pub const FLUSHO: crate::tcflag_t = 0x00001000;
pub const EXTPROC: crate::tcflag_t = 0o200000;

pub const MAP_HUGETLB: c_int = 0x040000;

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
pub const PTRACE_ATTACH: c_int = 16;
pub const PTRACE_DETACH: c_int = 17;
pub const PTRACE_SYSCALL: c_int = 24;
pub const PTRACE_SETOPTIONS: c_int = 0x4200;
pub const PTRACE_GETEVENTMSG: c_int = 0x4201;
pub const PTRACE_GETSIGINFO: c_int = 0x4202;
pub const PTRACE_SETSIGINFO: c_int = 0x4203;
pub const PTRACE_GETREGSET: c_int = 0x4204;
pub const PTRACE_SETREGSET: c_int = 0x4205;
pub const PTRACE_SECCOMP_GET_METADATA: c_int = 0x420d;

pub const PTRACE_EVENT_STOP: c_int = 128;

pub const F_GETLK: c_int = 5;
pub const F_GETOWN: c_int = 9;
pub const F_SETOWN: c_int = 8;
pub const F_SETLK: c_int = 6;
pub const F_SETLKW: c_int = 7;
pub const F_RDLCK: c_int = 0;
pub const F_WRLCK: c_int = 1;
pub const F_UNLCK: c_int = 2;
pub const F_OFD_GETLK: c_int = 36;
pub const F_OFD_SETLK: c_int = 37;
pub const F_OFD_SETLKW: c_int = 38;

pub const RLIMIT_CPU: c_int = 0;
pub const RLIMIT_FSIZE: c_int = 1;
pub const RLIMIT_DATA: c_int = 2;
pub const RLIMIT_STACK: c_int = 3;
pub const RLIMIT_CORE: c_int = 4;
pub const RLIMIT_RSS: c_int = 5;
pub const RLIMIT_NPROC: c_int = 6;
pub const RLIMIT_NOFILE: c_int = 7;
pub const RLIMIT_MEMLOCK: c_int = 8;
pub const RLIMIT_AS: c_int = 9;
pub const RLIMIT_LOCKS: c_int = 10;
pub const RLIMIT_SIGPENDING: c_int = 11;
pub const RLIMIT_MSGQUEUE: c_int = 12;
pub const RLIMIT_NICE: c_int = 13;
pub const RLIMIT_RTPRIO: c_int = 14;

#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIM_NLIMITS: c_int = 16;
pub const RLIM_INFINITY: crate::rlim_t = !0;

pub const TCGETS: c_int = 0x5401;
pub const TCSETS: c_int = 0x5402;
pub const TCSETSW: c_int = 0x5403;
pub const TCSETSF: c_int = 0x5404;
pub const TCGETS2: c_int = 0x802c542a;
pub const TCSETS2: c_int = 0x402c542b;
pub const TCSETSW2: c_int = 0x402c542c;
pub const TCSETSF2: c_int = 0x402c542d;
pub const TCGETA: c_int = 0x5405;
pub const TCSETA: c_int = 0x5406;
pub const TCSETAW: c_int = 0x5407;
pub const TCSETAF: c_int = 0x5408;
pub const TCSBRK: c_int = 0x5409;
pub const TCXONC: c_int = 0x540A;
pub const TCFLSH: c_int = 0x540B;
pub const TIOCGSOFTCAR: c_int = 0x5419;
pub const TIOCSSOFTCAR: c_int = 0x541A;
pub const TIOCINQ: c_int = 0x541B;
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
pub const TIOCSBRK: c_int = 0x5427;
pub const TIOCCBRK: c_int = 0x5428;

pub const ST_RDONLY: c_ulong = 1;
pub const ST_NOSUID: c_ulong = 2;
pub const ST_NODEV: c_ulong = 4;
pub const ST_NOEXEC: c_ulong = 8;
pub const ST_SYNCHRONOUS: c_ulong = 16;
pub const ST_MANDLOCK: c_ulong = 64;
pub const ST_NOATIME: c_ulong = 1024;
pub const ST_NODIRATIME: c_ulong = 2048;
pub const ST_RELATIME: c_ulong = 4096;

pub const RTLD_NOLOAD: c_int = 0x4;
pub const RTLD_NODELETE: c_int = 0x1000;

pub const SEM_FAILED: *mut sem_t = 0 as *mut sem_t;

pub const AI_PASSIVE: c_int = 0x00000001;
pub const AI_CANONNAME: c_int = 0x00000002;
pub const AI_NUMERICHOST: c_int = 0x00000004;
pub const AI_NUMERICSERV: c_int = 0x00000008;
pub const AI_MASK: c_int =
    AI_PASSIVE | AI_CANONNAME | AI_NUMERICHOST | AI_NUMERICSERV | AI_ADDRCONFIG;
pub const AI_ALL: c_int = 0x00000100;
pub const AI_V4MAPPED_CFG: c_int = 0x00000200;
pub const AI_ADDRCONFIG: c_int = 0x00000400;
pub const AI_V4MAPPED: c_int = 0x00000800;
pub const AI_DEFAULT: c_int = AI_V4MAPPED_CFG | AI_ADDRCONFIG;

// linux/kexec.h
pub const KEXEC_ON_CRASH: c_int = 0x00000001;
pub const KEXEC_PRESERVE_CONTEXT: c_int = 0x00000002;
pub const KEXEC_ARCH_MASK: c_int = 0xffff0000;
pub const KEXEC_FILE_UNLOAD: c_int = 0x00000001;
pub const KEXEC_FILE_ON_CRASH: c_int = 0x00000002;
pub const KEXEC_FILE_NO_INITRAMFS: c_int = 0x00000004;

pub const LINUX_REBOOT_MAGIC1: c_int = 0xfee1dead;
pub const LINUX_REBOOT_MAGIC2: c_int = 672274793;
pub const LINUX_REBOOT_MAGIC2A: c_int = 85072278;
pub const LINUX_REBOOT_MAGIC2B: c_int = 369367448;
pub const LINUX_REBOOT_MAGIC2C: c_int = 537993216;

pub const LINUX_REBOOT_CMD_RESTART: c_int = 0x01234567;
pub const LINUX_REBOOT_CMD_HALT: c_int = 0xCDEF0123;
pub const LINUX_REBOOT_CMD_CAD_ON: c_int = 0x89ABCDEF;
pub const LINUX_REBOOT_CMD_CAD_OFF: c_int = 0x00000000;
pub const LINUX_REBOOT_CMD_POWER_OFF: c_int = 0x4321FEDC;
pub const LINUX_REBOOT_CMD_RESTART2: c_int = 0xA1B2C3D4;
pub const LINUX_REBOOT_CMD_SW_SUSPEND: c_int = 0xD000FCE2;
pub const LINUX_REBOOT_CMD_KEXEC: c_int = 0x45584543;

pub const REG_BASIC: c_int = 0;
pub const REG_EXTENDED: c_int = 1;
pub const REG_ICASE: c_int = 2;
pub const REG_NOSUB: c_int = 4;
pub const REG_NEWLINE: c_int = 8;
pub const REG_NOSPEC: c_int = 16;
pub const REG_PEND: c_int = 32;
pub const REG_DUMP: c_int = 128;

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
pub const REG_EMPTY: c_int = 14;
pub const REG_ASSERT: c_int = 15;
pub const REG_INVARG: c_int = 16;
pub const REG_ATOI: c_int = 255;
pub const REG_ITOA: c_int = 256;

pub const REG_NOTBOL: c_int = 1;
pub const REG_NOTEOL: c_int = 2;
pub const REG_STARTEND: c_int = 4;
pub const REG_TRACE: c_int = 256;
pub const REG_LARGE: c_int = 512;
pub const REG_BACKR: c_int = 1024;

pub const MCL_CURRENT: c_int = 0x0001;
pub const MCL_FUTURE: c_int = 0x0002;
pub const MCL_ONFAULT: c_int = 0x0004;

pub const CBAUD: crate::tcflag_t = 0o0010017;
pub const TAB1: crate::tcflag_t = 0x00000800;
pub const TAB2: crate::tcflag_t = 0x00001000;
pub const TAB3: crate::tcflag_t = 0x00001800;
pub const CR1: crate::tcflag_t = 0x00000200;
pub const CR2: crate::tcflag_t = 0x00000400;
pub const CR3: crate::tcflag_t = 0x00000600;
pub const FF1: crate::tcflag_t = 0x00008000;
pub const BS1: crate::tcflag_t = 0x00002000;
pub const VT1: crate::tcflag_t = 0x00004000;
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
pub const BOTHER: crate::speed_t = 0o010000;
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
pub const IBSHIFT: crate::tcflag_t = 16;

pub const BLKIOMIN: c_int = 0x1278;
pub const BLKIOOPT: c_int = 0x1279;
pub const BLKSSZGET: c_int = 0x1268;
pub const BLKPBSZGET: c_int = 0x127B;

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
pub const EAI_OVERFLOW: c_int = 14;

pub const NETLINK_ROUTE: c_int = 0;
pub const NETLINK_UNUSED: c_int = 1;
pub const NETLINK_USERSOCK: c_int = 2;
pub const NETLINK_FIREWALL: c_int = 3;
pub const NETLINK_SOCK_DIAG: c_int = 4;
pub const NETLINK_NFLOG: c_int = 5;
pub const NETLINK_XFRM: c_int = 6;
pub const NETLINK_SELINUX: c_int = 7;
pub const NETLINK_ISCSI: c_int = 8;
pub const NETLINK_AUDIT: c_int = 9;
pub const NETLINK_FIB_LOOKUP: c_int = 10;
pub const NETLINK_CONNECTOR: c_int = 11;
pub const NETLINK_NETFILTER: c_int = 12;
pub const NETLINK_IP6_FW: c_int = 13;
pub const NETLINK_DNRTMSG: c_int = 14;
pub const NETLINK_KOBJECT_UEVENT: c_int = 15;
pub const NETLINK_GENERIC: c_int = 16;
pub const NETLINK_SCSITRANSPORT: c_int = 18;
pub const NETLINK_ECRYPTFS: c_int = 19;
pub const NETLINK_RDMA: c_int = 20;
pub const NETLINK_CRYPTO: c_int = 21;
pub const NETLINK_INET_DIAG: c_int = NETLINK_SOCK_DIAG;

pub const MAX_LINKS: c_int = 32;

pub const NLM_F_REQUEST: c_int = 1;
pub const NLM_F_MULTI: c_int = 2;
pub const NLM_F_ACK: c_int = 4;
pub const NLM_F_ECHO: c_int = 8;
pub const NLM_F_DUMP_INTR: c_int = 16;
pub const NLM_F_DUMP_FILTERED: c_int = 32;

pub const NLM_F_ROOT: c_int = 0x100;
pub const NLM_F_MATCH: c_int = 0x200;
pub const NLM_F_ATOMIC: c_int = 0x400;
pub const NLM_F_DUMP: c_int = NLM_F_ROOT | NLM_F_MATCH;

pub const NLM_F_REPLACE: c_int = 0x100;
pub const NLM_F_EXCL: c_int = 0x200;
pub const NLM_F_CREATE: c_int = 0x400;
pub const NLM_F_APPEND: c_int = 0x800;

pub const NLM_F_NONREC: c_int = 0x100;
pub const NLM_F_BULK: c_int = 0x200;

pub const NLM_F_CAPPED: c_int = 0x100;
pub const NLM_F_ACK_TLVS: c_int = 0x200;

pub const NLMSG_NOOP: c_int = 0x1;
pub const NLMSG_ERROR: c_int = 0x2;
pub const NLMSG_DONE: c_int = 0x3;
pub const NLMSG_OVERRUN: c_int = 0x4;
pub const NLMSG_MIN_TYPE: c_int = 0x10;

// linux/netfilter/nfnetlink.h
pub const NFNLGRP_NONE: c_int = 0;
pub const NFNLGRP_CONNTRACK_NEW: c_int = 1;
pub const NFNLGRP_CONNTRACK_UPDATE: c_int = 2;
pub const NFNLGRP_CONNTRACK_DESTROY: c_int = 3;
pub const NFNLGRP_CONNTRACK_EXP_NEW: c_int = 4;
pub const NFNLGRP_CONNTRACK_EXP_UPDATE: c_int = 5;
pub const NFNLGRP_CONNTRACK_EXP_DESTROY: c_int = 6;
pub const NFNLGRP_NFTABLES: c_int = 7;
pub const NFNLGRP_ACCT_QUOTA: c_int = 8;

pub const NFNETLINK_V0: c_int = 0;

pub const NFNL_SUBSYS_NONE: c_int = 0;
pub const NFNL_SUBSYS_CTNETLINK: c_int = 1;
pub const NFNL_SUBSYS_CTNETLINK_EXP: c_int = 2;
pub const NFNL_SUBSYS_QUEUE: c_int = 3;
pub const NFNL_SUBSYS_ULOG: c_int = 4;
pub const NFNL_SUBSYS_OSF: c_int = 5;
pub const NFNL_SUBSYS_IPSET: c_int = 6;
pub const NFNL_SUBSYS_ACCT: c_int = 7;
pub const NFNL_SUBSYS_CTNETLINK_TIMEOUT: c_int = 8;
pub const NFNL_SUBSYS_CTHELPER: c_int = 9;
pub const NFNL_SUBSYS_NFTABLES: c_int = 10;
pub const NFNL_SUBSYS_NFT_COMPAT: c_int = 11;
pub const NFNL_SUBSYS_COUNT: c_int = 12;

pub const NFNL_MSG_BATCH_BEGIN: c_int = NLMSG_MIN_TYPE;
pub const NFNL_MSG_BATCH_END: c_int = NLMSG_MIN_TYPE + 1;

// linux/netfilter/nfnetlink_log.h
pub const NFULNL_MSG_PACKET: c_int = 0;
pub const NFULNL_MSG_CONFIG: c_int = 1;

pub const NFULA_UNSPEC: c_int = 0;
pub const NFULA_PACKET_HDR: c_int = 1;
pub const NFULA_MARK: c_int = 2;
pub const NFULA_TIMESTAMP: c_int = 3;
pub const NFULA_IFINDEX_INDEV: c_int = 4;
pub const NFULA_IFINDEX_OUTDEV: c_int = 5;
pub const NFULA_IFINDEX_PHYSINDEV: c_int = 6;
pub const NFULA_IFINDEX_PHYSOUTDEV: c_int = 7;
pub const NFULA_HWADDR: c_int = 8;
pub const NFULA_PAYLOAD: c_int = 9;
pub const NFULA_PREFIX: c_int = 10;
pub const NFULA_UID: c_int = 11;
pub const NFULA_SEQ: c_int = 12;
pub const NFULA_SEQ_GLOBAL: c_int = 13;
pub const NFULA_GID: c_int = 14;
pub const NFULA_HWTYPE: c_int = 15;
pub const NFULA_HWHEADER: c_int = 16;
pub const NFULA_HWLEN: c_int = 17;
pub const NFULA_CT: c_int = 18;
pub const NFULA_CT_INFO: c_int = 19;

pub const NFULNL_CFG_CMD_NONE: c_int = 0;
pub const NFULNL_CFG_CMD_BIND: c_int = 1;
pub const NFULNL_CFG_CMD_UNBIND: c_int = 2;
pub const NFULNL_CFG_CMD_PF_BIND: c_int = 3;
pub const NFULNL_CFG_CMD_PF_UNBIND: c_int = 4;

pub const NFULA_CFG_UNSPEC: c_int = 0;
pub const NFULA_CFG_CMD: c_int = 1;
pub const NFULA_CFG_MODE: c_int = 2;
pub const NFULA_CFG_NLBUFSIZ: c_int = 3;
pub const NFULA_CFG_TIMEOUT: c_int = 4;
pub const NFULA_CFG_QTHRESH: c_int = 5;
pub const NFULA_CFG_FLAGS: c_int = 6;

pub const NFULNL_COPY_NONE: c_int = 0x00;
pub const NFULNL_COPY_META: c_int = 0x01;
pub const NFULNL_COPY_PACKET: c_int = 0x02;

pub const NFULNL_CFG_F_SEQ: c_int = 0x0001;
pub const NFULNL_CFG_F_SEQ_GLOBAL: c_int = 0x0002;
pub const NFULNL_CFG_F_CONNTRACK: c_int = 0x0004;

// linux/netfilter/nfnetlink_log.h
pub const NFQNL_MSG_PACKET: c_int = 0;
pub const NFQNL_MSG_VERDICT: c_int = 1;
pub const NFQNL_MSG_CONFIG: c_int = 2;
pub const NFQNL_MSG_VERDICT_BATCH: c_int = 3;

pub const NFQA_UNSPEC: c_int = 0;
pub const NFQA_PACKET_HDR: c_int = 1;
pub const NFQA_VERDICT_HDR: c_int = 2;
pub const NFQA_MARK: c_int = 3;
pub const NFQA_TIMESTAMP: c_int = 4;
pub const NFQA_IFINDEX_INDEV: c_int = 5;
pub const NFQA_IFINDEX_OUTDEV: c_int = 6;
pub const NFQA_IFINDEX_PHYSINDEV: c_int = 7;
pub const NFQA_IFINDEX_PHYSOUTDEV: c_int = 8;
pub const NFQA_HWADDR: c_int = 9;
pub const NFQA_PAYLOAD: c_int = 10;
pub const NFQA_CT: c_int = 11;
pub const NFQA_CT_INFO: c_int = 12;
pub const NFQA_CAP_LEN: c_int = 13;
pub const NFQA_SKB_INFO: c_int = 14;
pub const NFQA_EXP: c_int = 15;
pub const NFQA_UID: c_int = 16;
pub const NFQA_GID: c_int = 17;
pub const NFQA_SECCTX: c_int = 18;
/*
 FIXME: These are not yet available in musl sanitized kernel headers and
 make the tests fail. Enable them once musl has them.

 See https://github.com/rust-lang/libc/pull/1628 for more details.
pub const NFQA_VLAN: c_int = 19;
pub const NFQA_L2HDR: c_int = 20;

pub const NFQA_VLAN_UNSPEC: c_int = 0;
pub const NFQA_VLAN_PROTO: c_int = 1;
pub const NFQA_VLAN_TCI: c_int = 2;
*/

pub const NFQNL_CFG_CMD_NONE: c_int = 0;
pub const NFQNL_CFG_CMD_BIND: c_int = 1;
pub const NFQNL_CFG_CMD_UNBIND: c_int = 2;
pub const NFQNL_CFG_CMD_PF_BIND: c_int = 3;
pub const NFQNL_CFG_CMD_PF_UNBIND: c_int = 4;

pub const NFQNL_COPY_NONE: c_int = 0;
pub const NFQNL_COPY_META: c_int = 1;
pub const NFQNL_COPY_PACKET: c_int = 2;

pub const NFQA_CFG_UNSPEC: c_int = 0;
pub const NFQA_CFG_CMD: c_int = 1;
pub const NFQA_CFG_PARAMS: c_int = 2;
pub const NFQA_CFG_QUEUE_MAXLEN: c_int = 3;
pub const NFQA_CFG_MASK: c_int = 4;
pub const NFQA_CFG_FLAGS: c_int = 5;

pub const NFQA_CFG_F_FAIL_OPEN: c_int = 0x0001;
pub const NFQA_CFG_F_CONNTRACK: c_int = 0x0002;
pub const NFQA_CFG_F_GSO: c_int = 0x0004;
pub const NFQA_CFG_F_UID_GID: c_int = 0x0008;
pub const NFQA_CFG_F_SECCTX: c_int = 0x0010;
pub const NFQA_CFG_F_MAX: c_int = 0x0020;

pub const NFQA_SKB_CSUMNOTREADY: c_int = 0x0001;
pub const NFQA_SKB_GSO: c_int = 0x0002;
pub const NFQA_SKB_CSUM_NOTVERIFIED: c_int = 0x0004;

pub const GENL_NAMSIZ: c_int = 16;

pub const GENL_MIN_ID: c_int = NLMSG_MIN_TYPE;
pub const GENL_MAX_ID: c_int = 1023;

pub const GENL_ADMIN_PERM: c_int = 0x01;
pub const GENL_CMD_CAP_DO: c_int = 0x02;
pub const GENL_CMD_CAP_DUMP: c_int = 0x04;
pub const GENL_CMD_CAP_HASPOL: c_int = 0x08;
pub const GENL_UNS_ADMIN_PERM: c_int = 0x10;

pub const GENL_ID_CTRL: c_int = NLMSG_MIN_TYPE;
pub const GENL_ID_VFS_DQUOT: c_int = NLMSG_MIN_TYPE + 1;
pub const GENL_ID_PMCRAID: c_int = NLMSG_MIN_TYPE + 2;

pub const CTRL_CMD_UNSPEC: c_int = 0;
pub const CTRL_CMD_NEWFAMILY: c_int = 1;
pub const CTRL_CMD_DELFAMILY: c_int = 2;
pub const CTRL_CMD_GETFAMILY: c_int = 3;
pub const CTRL_CMD_NEWOPS: c_int = 4;
pub const CTRL_CMD_DELOPS: c_int = 5;
pub const CTRL_CMD_GETOPS: c_int = 6;
pub const CTRL_CMD_NEWMCAST_GRP: c_int = 7;
pub const CTRL_CMD_DELMCAST_GRP: c_int = 8;
pub const CTRL_CMD_GETMCAST_GRP: c_int = 9;

pub const CTRL_ATTR_UNSPEC: c_int = 0;
pub const CTRL_ATTR_FAMILY_ID: c_int = 1;
pub const CTRL_ATTR_FAMILY_NAME: c_int = 2;
pub const CTRL_ATTR_VERSION: c_int = 3;
pub const CTRL_ATTR_HDRSIZE: c_int = 4;
pub const CTRL_ATTR_MAXATTR: c_int = 5;
pub const CTRL_ATTR_OPS: c_int = 6;
pub const CTRL_ATTR_MCAST_GROUPS: c_int = 7;

pub const CTRL_ATTR_OP_UNSPEC: c_int = 0;
pub const CTRL_ATTR_OP_ID: c_int = 1;
pub const CTRL_ATTR_OP_FLAGS: c_int = 2;

pub const CTRL_ATTR_MCAST_GRP_UNSPEC: c_int = 0;
pub const CTRL_ATTR_MCAST_GRP_NAME: c_int = 1;
pub const CTRL_ATTR_MCAST_GRP_ID: c_int = 2;

pub const NETLINK_ADD_MEMBERSHIP: c_int = 1;
pub const NETLINK_DROP_MEMBERSHIP: c_int = 2;
pub const NETLINK_PKTINFO: c_int = 3;
pub const NETLINK_BROADCAST_ERROR: c_int = 4;
pub const NETLINK_NO_ENOBUFS: c_int = 5;
pub const NETLINK_RX_RING: c_int = 6;
pub const NETLINK_TX_RING: c_int = 7;
pub const NETLINK_LISTEN_ALL_NSID: c_int = 8;
pub const NETLINK_LIST_MEMBERSHIPS: c_int = 9;
pub const NETLINK_CAP_ACK: c_int = 10;
pub const NETLINK_EXT_ACK: c_int = 11;
pub const NETLINK_GET_STRICT_CHK: c_int = 12;

pub const GRND_NONBLOCK: c_uint = 0x0001;
pub const GRND_RANDOM: c_uint = 0x0002;
pub const GRND_INSECURE: c_uint = 0x0004;

// <linux/seccomp.h>
pub const SECCOMP_MODE_DISABLED: c_uint = 0;
pub const SECCOMP_MODE_STRICT: c_uint = 1;
pub const SECCOMP_MODE_FILTER: c_uint = 2;

pub const SECCOMP_SET_MODE_STRICT: c_uint = 0;
pub const SECCOMP_SET_MODE_FILTER: c_uint = 1;
pub const SECCOMP_GET_ACTION_AVAIL: c_uint = 2;
pub const SECCOMP_GET_NOTIF_SIZES: c_uint = 3;

pub const SECCOMP_FILTER_FLAG_TSYNC: c_ulong = 1 << 0;
pub const SECCOMP_FILTER_FLAG_LOG: c_ulong = 1 << 1;
pub const SECCOMP_FILTER_FLAG_SPEC_ALLOW: c_ulong = 1 << 2;
pub const SECCOMP_FILTER_FLAG_NEW_LISTENER: c_ulong = 1 << 3;
pub const SECCOMP_FILTER_FLAG_TSYNC_ESRCH: c_ulong = 1 << 4;
pub const SECCOMP_FILTER_FLAG_WAIT_KILLABLE_RECV: c_ulong = 1 << 5;

pub const SECCOMP_RET_KILL_PROCESS: c_uint = 0x80000000;
pub const SECCOMP_RET_KILL_THREAD: c_uint = 0x00000000;
pub const SECCOMP_RET_KILL: c_uint = SECCOMP_RET_KILL_THREAD;
pub const SECCOMP_RET_TRAP: c_uint = 0x00030000;
pub const SECCOMP_RET_ERRNO: c_uint = 0x00050000;
pub const SECCOMP_RET_USER_NOTIF: c_uint = 0x7fc00000;
pub const SECCOMP_RET_TRACE: c_uint = 0x7ff00000;
pub const SECCOMP_RET_LOG: c_uint = 0x7ffc0000;
pub const SECCOMP_RET_ALLOW: c_uint = 0x7fff0000;

pub const SECCOMP_RET_ACTION_FULL: c_uint = 0xffff0000;
pub const SECCOMP_RET_ACTION: c_uint = 0x7fff0000;
pub const SECCOMP_RET_DATA: c_uint = 0x0000ffff;

pub const SECCOMP_USER_NOTIF_FLAG_CONTINUE: c_ulong = 1;

pub const SECCOMP_ADDFD_FLAG_SETFD: c_ulong = 1;
pub const SECCOMP_ADDFD_FLAG_SEND: c_ulong = 2;

pub const NLA_F_NESTED: c_int = 1 << 15;
pub const NLA_F_NET_BYTEORDER: c_int = 1 << 14;
pub const NLA_TYPE_MASK: c_int = !(NLA_F_NESTED | NLA_F_NET_BYTEORDER);

pub const NLA_ALIGNTO: c_int = 4;

pub const SIGEV_THREAD_ID: c_int = 4;

pub const CIBAUD: crate::tcflag_t = 0o02003600000;
pub const CBAUDEX: crate::tcflag_t = 0o010000;

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

pub const POLLWRNORM: c_short = 0x100;
pub const POLLWRBAND: c_short = 0x200;

pub const SFD_CLOEXEC: c_int = O_CLOEXEC;
pub const SFD_NONBLOCK: c_int = O_NONBLOCK;

pub const SOCK_NONBLOCK: c_int = O_NONBLOCK;

pub const SO_ORIGINAL_DST: c_int = 80;

pub const IP_RECVFRAGSIZE: c_int = 25;

pub const IPV6_FLOWINFO: c_int = 11;
pub const IPV6_MULTICAST_ALL: c_int = 29;
pub const IPV6_ROUTER_ALERT_ISOLATE: c_int = 30;
pub const IPV6_FLOWLABEL_MGR: c_int = 32;
pub const IPV6_FLOWINFO_SEND: c_int = 33;
pub const IPV6_RECVFRAGSIZE: c_int = 77;
pub const IPV6_FREEBIND: c_int = 78;
pub const IPV6_FLOWINFO_FLOWLABEL: c_int = 0x000fffff;
pub const IPV6_FLOWINFO_PRIORITY: c_int = 0x0ff00000;

pub const IUTF8: crate::tcflag_t = 0x00004000;
pub const CMSPAR: crate::tcflag_t = 0o10000000000;
pub const O_TMPFILE: c_int = 0o20000000 | O_DIRECTORY;

pub const MFD_CLOEXEC: c_uint = 0x0001;
pub const MFD_ALLOW_SEALING: c_uint = 0x0002;
pub const MFD_HUGETLB: c_uint = 0x0004;
pub const MFD_NOEXEC_SEAL: c_uint = 0x0008;
pub const MFD_EXEC: c_uint = 0x0010;
pub const MFD_HUGE_64KB: c_uint = 0x40000000;
pub const MFD_HUGE_512KB: c_uint = 0x4c000000;
pub const MFD_HUGE_1MB: c_uint = 0x50000000;
pub const MFD_HUGE_2MB: c_uint = 0x54000000;
pub const MFD_HUGE_8MB: c_uint = 0x5c000000;
pub const MFD_HUGE_16MB: c_uint = 0x60000000;
pub const MFD_HUGE_32MB: c_uint = 0x64000000;
pub const MFD_HUGE_256MB: c_uint = 0x70000000;
pub const MFD_HUGE_512MB: c_uint = 0x74000000;
pub const MFD_HUGE_1GB: c_uint = 0x78000000;
pub const MFD_HUGE_2GB: c_uint = 0x7c000000;
pub const MFD_HUGE_16GB: c_uint = 0x88000000;
pub const MFD_HUGE_MASK: c_uint = 63;
pub const MFD_HUGE_SHIFT: c_uint = 26;

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
pub const PT_LOOS: u32 = 0x60000000;
pub const PT_GNU_EH_FRAME: u32 = 0x6474e550;
pub const PT_GNU_STACK: u32 = 0x6474e551;
pub const PT_GNU_RELRO: u32 = 0x6474e552;
pub const PT_HIOS: u32 = 0x6fffffff;
pub const PT_LOPROC: u32 = 0x70000000;
pub const PT_HIPROC: u32 = 0x7fffffff;

// uapi/linux/mount.h
pub const OPEN_TREE_CLONE: c_uint = 0x01;
pub const OPEN_TREE_CLOEXEC: c_uint = O_CLOEXEC as c_uint;

// linux/netfilter.h
pub const NF_DROP: c_int = 0;
pub const NF_ACCEPT: c_int = 1;
pub const NF_STOLEN: c_int = 2;
pub const NF_QUEUE: c_int = 3;
pub const NF_REPEAT: c_int = 4;
pub const NF_STOP: c_int = 5;
pub const NF_MAX_VERDICT: c_int = NF_STOP;

pub const NF_VERDICT_MASK: c_int = 0x000000ff;
pub const NF_VERDICT_FLAG_QUEUE_BYPASS: c_int = 0x00008000;

pub const NF_VERDICT_QMASK: c_int = 0xffff0000;
pub const NF_VERDICT_QBITS: c_int = 16;

pub const NF_VERDICT_BITS: c_int = 16;

pub const NF_INET_PRE_ROUTING: c_int = 0;
pub const NF_INET_LOCAL_IN: c_int = 1;
pub const NF_INET_FORWARD: c_int = 2;
pub const NF_INET_LOCAL_OUT: c_int = 3;
pub const NF_INET_POST_ROUTING: c_int = 4;
pub const NF_INET_NUMHOOKS: c_int = 5;
pub const NF_INET_INGRESS: c_int = NF_INET_NUMHOOKS;

pub const NF_NETDEV_INGRESS: c_int = 0;
pub const NF_NETDEV_EGRESS: c_int = 1;
pub const NF_NETDEV_NUMHOOKS: c_int = 2;

pub const NFPROTO_UNSPEC: c_int = 0;
pub const NFPROTO_INET: c_int = 1;
pub const NFPROTO_IPV4: c_int = 2;
pub const NFPROTO_ARP: c_int = 3;
pub const NFPROTO_NETDEV: c_int = 5;
pub const NFPROTO_BRIDGE: c_int = 7;
pub const NFPROTO_IPV6: c_int = 10;
pub const NFPROTO_DECNET: c_int = 12;
pub const NFPROTO_NUMPROTO: c_int = 13;

// linux/netfilter_arp.h
pub const NF_ARP: c_int = 0;
pub const NF_ARP_IN: c_int = 0;
pub const NF_ARP_OUT: c_int = 1;
pub const NF_ARP_FORWARD: c_int = 2;
pub const NF_ARP_NUMHOOKS: c_int = 3;

// linux/netfilter_bridge.h
pub const NF_BR_PRE_ROUTING: c_int = 0;
pub const NF_BR_LOCAL_IN: c_int = 1;
pub const NF_BR_FORWARD: c_int = 2;
pub const NF_BR_LOCAL_OUT: c_int = 3;
pub const NF_BR_POST_ROUTING: c_int = 4;
pub const NF_BR_BROUTING: c_int = 5;
pub const NF_BR_NUMHOOKS: c_int = 6;

pub const NF_BR_PRI_FIRST: c_int = crate::INT_MIN;
pub const NF_BR_PRI_NAT_DST_BRIDGED: c_int = -300;
pub const NF_BR_PRI_FILTER_BRIDGED: c_int = -200;
pub const NF_BR_PRI_BRNF: c_int = 0;
pub const NF_BR_PRI_NAT_DST_OTHER: c_int = 100;
pub const NF_BR_PRI_FILTER_OTHER: c_int = 200;
pub const NF_BR_PRI_NAT_SRC: c_int = 300;
pub const NF_BR_PRI_LAST: c_int = crate::INT_MAX;

// linux/netfilter_ipv4.h
pub const NF_IP_PRE_ROUTING: c_int = 0;
pub const NF_IP_LOCAL_IN: c_int = 1;
pub const NF_IP_FORWARD: c_int = 2;
pub const NF_IP_LOCAL_OUT: c_int = 3;
pub const NF_IP_POST_ROUTING: c_int = 4;
pub const NF_IP_NUMHOOKS: c_int = 5;

pub const NF_IP_PRI_FIRST: c_int = crate::INT_MIN;
pub const NF_IP_PRI_RAW_BEFORE_DEFRAG: c_int = -450;
pub const NF_IP_PRI_CONNTRACK_DEFRAG: c_int = -400;
pub const NF_IP_PRI_RAW: c_int = -300;
pub const NF_IP_PRI_SELINUX_FIRST: c_int = -225;
pub const NF_IP_PRI_CONNTRACK: c_int = -200;
pub const NF_IP_PRI_MANGLE: c_int = -150;
pub const NF_IP_PRI_NAT_DST: c_int = -100;
pub const NF_IP_PRI_FILTER: c_int = 0;
pub const NF_IP_PRI_SECURITY: c_int = 50;
pub const NF_IP_PRI_NAT_SRC: c_int = 100;
pub const NF_IP_PRI_SELINUX_LAST: c_int = 225;
pub const NF_IP_PRI_CONNTRACK_HELPER: c_int = 300;
pub const NF_IP_PRI_CONNTRACK_CONFIRM: c_int = crate::INT_MAX;
pub const NF_IP_PRI_LAST: c_int = crate::INT_MAX;

// linux/netfilter_ipv6.h
pub const NF_IP6_PRE_ROUTING: c_int = 0;
pub const NF_IP6_LOCAL_IN: c_int = 1;
pub const NF_IP6_FORWARD: c_int = 2;
pub const NF_IP6_LOCAL_OUT: c_int = 3;
pub const NF_IP6_POST_ROUTING: c_int = 4;
pub const NF_IP6_NUMHOOKS: c_int = 5;

pub const NF_IP6_PRI_FIRST: c_int = crate::INT_MIN;
pub const NF_IP6_PRI_RAW_BEFORE_DEFRAG: c_int = -450;
pub const NF_IP6_PRI_CONNTRACK_DEFRAG: c_int = -400;
pub const NF_IP6_PRI_RAW: c_int = -300;
pub const NF_IP6_PRI_SELINUX_FIRST: c_int = -225;
pub const NF_IP6_PRI_CONNTRACK: c_int = -200;
pub const NF_IP6_PRI_MANGLE: c_int = -150;
pub const NF_IP6_PRI_NAT_DST: c_int = -100;
pub const NF_IP6_PRI_FILTER: c_int = 0;
pub const NF_IP6_PRI_SECURITY: c_int = 50;
pub const NF_IP6_PRI_NAT_SRC: c_int = 100;
pub const NF_IP6_PRI_SELINUX_LAST: c_int = 225;
pub const NF_IP6_PRI_CONNTRACK_HELPER: c_int = 300;
pub const NF_IP6_PRI_LAST: c_int = crate::INT_MAX;

// linux/netfilter_ipv6/ip6_tables.h
pub const IP6T_SO_ORIGINAL_DST: c_int = 80;

// linux/netfilter/nf_tables.h
pub const NFT_TABLE_MAXNAMELEN: c_int = 256;
pub const NFT_CHAIN_MAXNAMELEN: c_int = 256;
pub const NFT_SET_MAXNAMELEN: c_int = 256;
pub const NFT_OBJ_MAXNAMELEN: c_int = 256;
pub const NFT_USERDATA_MAXLEN: c_int = 256;

pub const NFT_REG_VERDICT: c_int = 0;
pub const NFT_REG_1: c_int = 1;
pub const NFT_REG_2: c_int = 2;
pub const NFT_REG_3: c_int = 3;
pub const NFT_REG_4: c_int = 4;
pub const __NFT_REG_MAX: c_int = 5;
pub const NFT_REG32_00: c_int = 8;
pub const NFT_REG32_01: c_int = 9;
pub const NFT_REG32_02: c_int = 10;
pub const NFT_REG32_03: c_int = 11;
pub const NFT_REG32_04: c_int = 12;
pub const NFT_REG32_05: c_int = 13;
pub const NFT_REG32_06: c_int = 14;
pub const NFT_REG32_07: c_int = 15;
pub const NFT_REG32_08: c_int = 16;
pub const NFT_REG32_09: c_int = 17;
pub const NFT_REG32_10: c_int = 18;
pub const NFT_REG32_11: c_int = 19;
pub const NFT_REG32_12: c_int = 20;
pub const NFT_REG32_13: c_int = 21;
pub const NFT_REG32_14: c_int = 22;
pub const NFT_REG32_15: c_int = 23;

pub const NFT_REG_SIZE: c_int = 16;
pub const NFT_REG32_SIZE: c_int = 4;

pub const NFT_CONTINUE: c_int = -1;
pub const NFT_BREAK: c_int = -2;
pub const NFT_JUMP: c_int = -3;
pub const NFT_GOTO: c_int = -4;
pub const NFT_RETURN: c_int = -5;

pub const NFT_MSG_NEWTABLE: c_int = 0;
pub const NFT_MSG_GETTABLE: c_int = 1;
pub const NFT_MSG_DELTABLE: c_int = 2;
pub const NFT_MSG_NEWCHAIN: c_int = 3;
pub const NFT_MSG_GETCHAIN: c_int = 4;
pub const NFT_MSG_DELCHAIN: c_int = 5;
pub const NFT_MSG_NEWRULE: c_int = 6;
pub const NFT_MSG_GETRULE: c_int = 7;
pub const NFT_MSG_DELRULE: c_int = 8;
pub const NFT_MSG_NEWSET: c_int = 9;
pub const NFT_MSG_GETSET: c_int = 10;
pub const NFT_MSG_DELSET: c_int = 11;
pub const NFT_MSG_NEWSETELEM: c_int = 12;
pub const NFT_MSG_GETSETELEM: c_int = 13;
pub const NFT_MSG_DELSETELEM: c_int = 14;
pub const NFT_MSG_NEWGEN: c_int = 15;
pub const NFT_MSG_GETGEN: c_int = 16;
pub const NFT_MSG_TRACE: c_int = 17;
pub const NFT_MSG_NEWOBJ: c_int = 18;
pub const NFT_MSG_GETOBJ: c_int = 19;
pub const NFT_MSG_DELOBJ: c_int = 20;
pub const NFT_MSG_GETOBJ_RESET: c_int = 21;
pub const NFT_MSG_MAX: c_int = 25;

pub const NFT_SET_ANONYMOUS: c_int = 0x1;
pub const NFT_SET_CONSTANT: c_int = 0x2;
pub const NFT_SET_INTERVAL: c_int = 0x4;
pub const NFT_SET_MAP: c_int = 0x8;
pub const NFT_SET_TIMEOUT: c_int = 0x10;
pub const NFT_SET_EVAL: c_int = 0x20;

pub const NFT_SET_POL_PERFORMANCE: c_int = 0;
pub const NFT_SET_POL_MEMORY: c_int = 1;

pub const NFT_SET_ELEM_INTERVAL_END: c_int = 0x1;

pub const NFT_DATA_VALUE: c_uint = 0;
pub const NFT_DATA_VERDICT: c_uint = 0xffffff00;

pub const NFT_DATA_RESERVED_MASK: c_uint = 0xffffff00;

pub const NFT_DATA_VALUE_MAXLEN: c_int = 64;

pub const NFT_BYTEORDER_NTOH: c_int = 0;
pub const NFT_BYTEORDER_HTON: c_int = 1;

pub const NFT_CMP_EQ: c_int = 0;
pub const NFT_CMP_NEQ: c_int = 1;
pub const NFT_CMP_LT: c_int = 2;
pub const NFT_CMP_LTE: c_int = 3;
pub const NFT_CMP_GT: c_int = 4;
pub const NFT_CMP_GTE: c_int = 5;

pub const NFT_RANGE_EQ: c_int = 0;
pub const NFT_RANGE_NEQ: c_int = 1;

pub const NFT_LOOKUP_F_INV: c_int = 1 << 0;

pub const NFT_DYNSET_OP_ADD: c_int = 0;
pub const NFT_DYNSET_OP_UPDATE: c_int = 1;

pub const NFT_DYNSET_F_INV: c_int = 1 << 0;

pub const NFT_PAYLOAD_LL_HEADER: c_int = 0;
pub const NFT_PAYLOAD_NETWORK_HEADER: c_int = 1;
pub const NFT_PAYLOAD_TRANSPORT_HEADER: c_int = 2;

pub const NFT_PAYLOAD_CSUM_NONE: c_int = 0;
pub const NFT_PAYLOAD_CSUM_INET: c_int = 1;

pub const NFT_META_LEN: c_int = 0;
pub const NFT_META_PROTOCOL: c_int = 1;
pub const NFT_META_PRIORITY: c_int = 2;
pub const NFT_META_MARK: c_int = 3;
pub const NFT_META_IIF: c_int = 4;
pub const NFT_META_OIF: c_int = 5;
pub const NFT_META_IIFNAME: c_int = 6;
pub const NFT_META_OIFNAME: c_int = 7;
pub const NFT_META_IIFTYPE: c_int = 8;
pub const NFT_META_OIFTYPE: c_int = 9;
pub const NFT_META_SKUID: c_int = 10;
pub const NFT_META_SKGID: c_int = 11;
pub const NFT_META_NFTRACE: c_int = 12;
pub const NFT_META_RTCLASSID: c_int = 13;
pub const NFT_META_SECMARK: c_int = 14;
pub const NFT_META_NFPROTO: c_int = 15;
pub const NFT_META_L4PROTO: c_int = 16;
pub const NFT_META_BRI_IIFNAME: c_int = 17;
pub const NFT_META_BRI_OIFNAME: c_int = 18;
pub const NFT_META_PKTTYPE: c_int = 19;
pub const NFT_META_CPU: c_int = 20;
pub const NFT_META_IIFGROUP: c_int = 21;
pub const NFT_META_OIFGROUP: c_int = 22;
pub const NFT_META_CGROUP: c_int = 23;
pub const NFT_META_PRANDOM: c_int = 24;

pub const NFT_CT_STATE: c_int = 0;
pub const NFT_CT_DIRECTION: c_int = 1;
pub const NFT_CT_STATUS: c_int = 2;
pub const NFT_CT_MARK: c_int = 3;
pub const NFT_CT_SECMARK: c_int = 4;
pub const NFT_CT_EXPIRATION: c_int = 5;
pub const NFT_CT_HELPER: c_int = 6;
pub const NFT_CT_L3PROTOCOL: c_int = 7;
pub const NFT_CT_SRC: c_int = 8;
pub const NFT_CT_DST: c_int = 9;
pub const NFT_CT_PROTOCOL: c_int = 10;
pub const NFT_CT_PROTO_SRC: c_int = 11;
pub const NFT_CT_PROTO_DST: c_int = 12;
pub const NFT_CT_LABELS: c_int = 13;
pub const NFT_CT_PKTS: c_int = 14;
pub const NFT_CT_BYTES: c_int = 15;
pub const NFT_CT_AVGPKT: c_int = 16;
pub const NFT_CT_ZONE: c_int = 17;
pub const NFT_CT_EVENTMASK: c_int = 18;
pub const NFT_CT_SRC_IP: c_int = 19;
pub const NFT_CT_DST_IP: c_int = 20;
pub const NFT_CT_SRC_IP6: c_int = 21;
pub const NFT_CT_DST_IP6: c_int = 22;
pub const NFT_CT_ID: c_int = 23;

pub const NFT_LIMIT_PKTS: c_int = 0;
pub const NFT_LIMIT_PKT_BYTES: c_int = 1;

pub const NFT_LIMIT_F_INV: c_int = 1 << 0;

pub const NFT_QUEUE_FLAG_BYPASS: c_int = 0x01;
pub const NFT_QUEUE_FLAG_CPU_FANOUT: c_int = 0x02;
pub const NFT_QUEUE_FLAG_MASK: c_int = 0x03;

pub const NFT_QUOTA_F_INV: c_int = 1 << 0;

pub const NFT_REJECT_ICMP_UNREACH: c_int = 0;
pub const NFT_REJECT_TCP_RST: c_int = 1;
pub const NFT_REJECT_ICMPX_UNREACH: c_int = 2;

pub const NFT_REJECT_ICMPX_NO_ROUTE: c_int = 0;
pub const NFT_REJECT_ICMPX_PORT_UNREACH: c_int = 1;
pub const NFT_REJECT_ICMPX_HOST_UNREACH: c_int = 2;
pub const NFT_REJECT_ICMPX_ADMIN_PROHIBITED: c_int = 3;

pub const NFT_NAT_SNAT: c_int = 0;
pub const NFT_NAT_DNAT: c_int = 1;

pub const NFT_TRACETYPE_UNSPEC: c_int = 0;
pub const NFT_TRACETYPE_POLICY: c_int = 1;
pub const NFT_TRACETYPE_RETURN: c_int = 2;
pub const NFT_TRACETYPE_RULE: c_int = 3;

pub const NFT_NG_INCREMENTAL: c_int = 0;
pub const NFT_NG_RANDOM: c_int = 1;

// linux/input.h
pub const FF_MAX: crate::__u16 = 0x7f;
pub const FF_CNT: usize = FF_MAX as usize + 1;

// linux/input-event-codes.h
pub const INPUT_PROP_MAX: crate::__u16 = 0x1f;
pub const INPUT_PROP_CNT: usize = INPUT_PROP_MAX as usize + 1;
pub const EV_MAX: crate::__u16 = 0x1f;
pub const EV_CNT: usize = EV_MAX as usize + 1;
pub const SYN_MAX: crate::__u16 = 0xf;
pub const SYN_CNT: usize = SYN_MAX as usize + 1;
pub const KEY_MAX: crate::__u16 = 0x2ff;
pub const KEY_CNT: usize = KEY_MAX as usize + 1;
pub const REL_MAX: crate::__u16 = 0x0f;
pub const REL_CNT: usize = REL_MAX as usize + 1;
pub const ABS_MAX: crate::__u16 = 0x3f;
pub const ABS_CNT: usize = ABS_MAX as usize + 1;
pub const SW_MAX: crate::__u16 = 0x0f;
pub const SW_CNT: usize = SW_MAX as usize + 1;
pub const MSC_MAX: crate::__u16 = 0x07;
pub const MSC_CNT: usize = MSC_MAX as usize + 1;
pub const LED_MAX: crate::__u16 = 0x0f;
pub const LED_CNT: usize = LED_MAX as usize + 1;
pub const REP_MAX: crate::__u16 = 0x01;
pub const REP_CNT: usize = REP_MAX as usize + 1;
pub const SND_MAX: crate::__u16 = 0x07;
pub const SND_CNT: usize = SND_MAX as usize + 1;

// linux/uinput.h
pub const UINPUT_VERSION: c_uint = 5;
pub const UINPUT_MAX_NAME_SIZE: usize = 80;

// start android/platform/bionic/libc/kernel/uapi/linux/if_ether.h
// from https://android.googlesource.com/platform/bionic/+/HEAD/libc/kernel/uapi/linux/if_ether.h
pub const ETH_ALEN: c_int = 6;
pub const ETH_HLEN: c_int = 14;
pub const ETH_ZLEN: c_int = 60;
pub const ETH_DATA_LEN: c_int = 1500;
pub const ETH_FRAME_LEN: c_int = 1514;
pub const ETH_FCS_LEN: c_int = 4;
pub const ETH_MIN_MTU: c_int = 68;
pub const ETH_MAX_MTU: c_int = 0xFFFF;
pub const ETH_P_LOOP: c_int = 0x0060;
pub const ETH_P_PUP: c_int = 0x0200;
pub const ETH_P_PUPAT: c_int = 0x0201;
pub const ETH_P_TSN: c_int = 0x22F0;
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
/* see rust-lang/libc#924 pub const ETH_P_ERSPAN: c_int = 0x88BE;*/
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
pub const ETH_P_MACSEC: c_int = 0x88E5;
pub const ETH_P_8021AH: c_int = 0x88E7;
pub const ETH_P_MVRP: c_int = 0x88F5;
pub const ETH_P_1588: c_int = 0x88F7;
pub const ETH_P_NCSI: c_int = 0x88F8;
pub const ETH_P_PRP: c_int = 0x88FB;
pub const ETH_P_FCOE: c_int = 0x8906;
/* see rust-lang/libc#924 pub const ETH_P_IBOE: c_int = 0x8915;*/
pub const ETH_P_TDLS: c_int = 0x890D;
pub const ETH_P_FIP: c_int = 0x8914;
pub const ETH_P_80221: c_int = 0x8917;
pub const ETH_P_HSR: c_int = 0x892F;
/* see rust-lang/libc#924 pub const ETH_P_NSH: c_int = 0x894F;*/
pub const ETH_P_LOOPBACK: c_int = 0x9000;
pub const ETH_P_QINQ1: c_int = 0x9100;
pub const ETH_P_QINQ2: c_int = 0x9200;
pub const ETH_P_QINQ3: c_int = 0x9300;
pub const ETH_P_EDSA: c_int = 0xDADA;
/* see rust-lang/libc#924 pub const ETH_P_IFE: c_int = 0xED3E;*/
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
pub const ETH_P_XDSA: c_int = 0x00F8;
/* see rust-lang/libc#924 pub const ETH_P_MAP: c_int = 0x00F9;*/
// end android/platform/bionic/libc/kernel/uapi/linux/if_ether.h

// start android/platform/bionic/libc/kernel/uapi/linux/neighbour.h
pub const NDA_UNSPEC: c_ushort = 0;
pub const NDA_DST: c_ushort = 1;
pub const NDA_LLADDR: c_ushort = 2;
pub const NDA_CACHEINFO: c_ushort = 3;
pub const NDA_PROBES: c_ushort = 4;
pub const NDA_VLAN: c_ushort = 5;
pub const NDA_PORT: c_ushort = 6;
pub const NDA_VNI: c_ushort = 7;
pub const NDA_IFINDEX: c_ushort = 8;
pub const NDA_MASTER: c_ushort = 9;
pub const NDA_LINK_NETNSID: c_ushort = 10;
pub const NDA_SRC_VNI: c_ushort = 11;
pub const NDA_PROTOCOL: c_ushort = 12;
pub const NDA_NH_ID: c_ushort = 13;
pub const NDA_FDB_EXT_ATTRS: c_ushort = 14;
pub const NDA_FLAGS_EXT: c_ushort = 15;
pub const NDA_NDM_STATE_MASK: c_ushort = 16;
pub const NDA_NDM_FLAGS_MASK: c_ushort = 17;

pub const NTF_USE: u8 = 0x01;
pub const NTF_SELF: u8 = 0x02;
pub const NTF_MASTER: u8 = 0x04;
pub const NTF_PROXY: u8 = 0x08;
pub const NTF_EXT_LEARNED: u8 = 0x10;
pub const NTF_OFFLOADED: u8 = 0x20;
pub const NTF_STICKY: u8 = 0x40;
pub const NTF_ROUTER: u8 = 0x80;

pub const NTF_EXT_MANAGED: u8 = 0x01;
pub const NTF_EXT_LOCKED: u8 = 0x02;

pub const NUD_NONE: u16 = 0x00;
pub const NUD_INCOMPLETE: u16 = 0x01;
pub const NUD_REACHABLE: u16 = 0x02;
pub const NUD_STALE: u16 = 0x04;
pub const NUD_DELAY: u16 = 0x08;
pub const NUD_PROBE: u16 = 0x10;
pub const NUD_FAILED: u16 = 0x20;
pub const NUD_NOARP: u16 = 0x40;
pub const NUD_PERMANENT: u16 = 0x80;

pub const NDTPA_UNSPEC: c_ushort = 0;
pub const NDTPA_IFINDEX: c_ushort = 1;
pub const NDTPA_REFCNT: c_ushort = 2;
pub const NDTPA_REACHABLE_TIME: c_ushort = 3;
pub const NDTPA_BASE_REACHABLE_TIME: c_ushort = 4;
pub const NDTPA_RETRANS_TIME: c_ushort = 5;
pub const NDTPA_GC_STALETIME: c_ushort = 6;
pub const NDTPA_DELAY_PROBE_TIME: c_ushort = 7;
pub const NDTPA_QUEUE_LEN: c_ushort = 8;
pub const NDTPA_APP_PROBES: c_ushort = 9;
pub const NDTPA_UCAST_PROBES: c_ushort = 10;
pub const NDTPA_MCAST_PROBES: c_ushort = 11;
pub const NDTPA_ANYCAST_DELAY: c_ushort = 12;
pub const NDTPA_PROXY_DELAY: c_ushort = 13;
pub const NDTPA_PROXY_QLEN: c_ushort = 14;
pub const NDTPA_LOCKTIME: c_ushort = 15;
pub const NDTPA_QUEUE_LENBYTES: c_ushort = 16;
pub const NDTPA_MCAST_REPROBES: c_ushort = 17;
pub const NDTPA_PAD: c_ushort = 18;
pub const NDTPA_INTERVAL_PROBE_TIME_MS: c_ushort = 19;

pub const NDTA_UNSPEC: c_ushort = 0;
pub const NDTA_NAME: c_ushort = 1;
pub const NDTA_THRESH1: c_ushort = 2;
pub const NDTA_THRESH2: c_ushort = 3;
pub const NDTA_THRESH3: c_ushort = 4;
pub const NDTA_CONFIG: c_ushort = 5;
pub const NDTA_PARMS: c_ushort = 6;
pub const NDTA_STATS: c_ushort = 7;
pub const NDTA_GC_INTERVAL: c_ushort = 8;
pub const NDTA_PAD: c_ushort = 9;

pub const FDB_NOTIFY_BIT: u16 = 0x01;
pub const FDB_NOTIFY_INACTIVE_BIT: u16 = 0x02;

pub const NFEA_UNSPEC: c_ushort = 0;
pub const NFEA_ACTIVITY_NOTIFY: c_ushort = 1;
pub const NFEA_DONT_REFRESH: c_ushort = 2;
// end android/platform/bionic/libc/kernel/uapi/linux/neighbour.h

pub const SIOCADDRT: c_ulong = 0x0000890B;
pub const SIOCDELRT: c_ulong = 0x0000890C;
pub const SIOCRTMSG: c_ulong = 0x0000890D;
pub const SIOCGIFNAME: c_ulong = 0x00008910;
pub const SIOCSIFLINK: c_ulong = 0x00008911;
pub const SIOCGIFCONF: c_ulong = 0x00008912;
pub const SIOCGIFFLAGS: c_ulong = 0x00008913;
pub const SIOCSIFFLAGS: c_ulong = 0x00008914;
pub const SIOCGIFADDR: c_ulong = 0x00008915;
pub const SIOCSIFADDR: c_ulong = 0x00008916;
pub const SIOCGIFDSTADDR: c_ulong = 0x00008917;
pub const SIOCSIFDSTADDR: c_ulong = 0x00008918;
pub const SIOCGIFBRDADDR: c_ulong = 0x00008919;
pub const SIOCSIFBRDADDR: c_ulong = 0x0000891A;
pub const SIOCGIFNETMASK: c_ulong = 0x0000891B;
pub const SIOCSIFNETMASK: c_ulong = 0x0000891C;
pub const SIOCGIFMETRIC: c_ulong = 0x0000891D;
pub const SIOCSIFMETRIC: c_ulong = 0x0000891E;
pub const SIOCGIFMEM: c_ulong = 0x0000891F;
pub const SIOCSIFMEM: c_ulong = 0x00008920;
pub const SIOCGIFMTU: c_ulong = 0x00008921;
pub const SIOCSIFMTU: c_ulong = 0x00008922;
pub const SIOCSIFNAME: c_ulong = 0x00008923;
pub const SIOCSIFHWADDR: c_ulong = 0x00008924;
pub const SIOCGIFENCAP: c_ulong = 0x00008925;
pub const SIOCSIFENCAP: c_ulong = 0x00008926;
pub const SIOCGIFHWADDR: c_ulong = 0x00008927;
pub const SIOCGIFSLAVE: c_ulong = 0x00008929;
pub const SIOCSIFSLAVE: c_ulong = 0x00008930;
pub const SIOCADDMULTI: c_ulong = 0x00008931;
pub const SIOCDELMULTI: c_ulong = 0x00008932;
pub const SIOCGIFINDEX: c_ulong = 0x00008933;
pub const SIOGIFINDEX: c_ulong = SIOCGIFINDEX;
pub const SIOCSIFPFLAGS: c_ulong = 0x00008934;
pub const SIOCGIFPFLAGS: c_ulong = 0x00008935;
pub const SIOCDIFADDR: c_ulong = 0x00008936;
pub const SIOCSIFHWBROADCAST: c_ulong = 0x00008937;
pub const SIOCGIFCOUNT: c_ulong = 0x00008938;
pub const SIOCGIFBR: c_ulong = 0x00008940;
pub const SIOCSIFBR: c_ulong = 0x00008941;
pub const SIOCGIFTXQLEN: c_ulong = 0x00008942;
pub const SIOCSIFTXQLEN: c_ulong = 0x00008943;
pub const SIOCETHTOOL: c_ulong = 0x00008946;
pub const SIOCGMIIPHY: c_ulong = 0x00008947;
pub const SIOCGMIIREG: c_ulong = 0x00008948;
pub const SIOCSMIIREG: c_ulong = 0x00008949;
pub const SIOCWANDEV: c_ulong = 0x0000894A;
pub const SIOCOUTQNSD: c_ulong = 0x0000894B;
pub const SIOCGSKNS: c_ulong = 0x0000894C;
pub const SIOCDARP: c_ulong = 0x00008953;
pub const SIOCGARP: c_ulong = 0x00008954;
pub const SIOCSARP: c_ulong = 0x00008955;
pub const SIOCDRARP: c_ulong = 0x00008960;
pub const SIOCGRARP: c_ulong = 0x00008961;
pub const SIOCSRARP: c_ulong = 0x00008962;
pub const SIOCGIFMAP: c_ulong = 0x00008970;
pub const SIOCSIFMAP: c_ulong = 0x00008971;
pub const SIOCADDDLCI: c_ulong = 0x00008980;
pub const SIOCDELDLCI: c_ulong = 0x00008981;
pub const SIOCGIFVLAN: c_ulong = 0x00008982;
pub const SIOCSIFVLAN: c_ulong = 0x00008983;
pub const SIOCBONDENSLAVE: c_ulong = 0x00008990;
pub const SIOCBONDRELEASE: c_ulong = 0x00008991;
pub const SIOCBONDSETHWADDR: c_ulong = 0x00008992;
pub const SIOCBONDSLAVEINFOQUERY: c_ulong = 0x00008993;
pub const SIOCBONDINFOQUERY: c_ulong = 0x00008994;
pub const SIOCBONDCHANGEACTIVE: c_ulong = 0x00008995;
pub const SIOCBRADDBR: c_ulong = 0x000089a0;
pub const SIOCBRDELBR: c_ulong = 0x000089a1;
pub const SIOCBRADDIF: c_ulong = 0x000089a2;
pub const SIOCBRDELIF: c_ulong = 0x000089a3;
pub const SIOCSHWTSTAMP: c_ulong = 0x000089b0;
pub const SIOCGHWTSTAMP: c_ulong = 0x000089b1;
pub const SIOCDEVPRIVATE: c_ulong = 0x000089F0;
pub const SIOCPROTOPRIVATE: c_ulong = 0x000089E0;

// linux/module.h
pub const MODULE_INIT_IGNORE_MODVERSIONS: c_uint = 0x0001;
pub const MODULE_INIT_IGNORE_VERMAGIC: c_uint = 0x0002;

// linux/net_tstamp.h
pub const SOF_TIMESTAMPING_TX_HARDWARE: c_uint = 1 << 0;
pub const SOF_TIMESTAMPING_TX_SOFTWARE: c_uint = 1 << 1;
pub const SOF_TIMESTAMPING_RX_HARDWARE: c_uint = 1 << 2;
pub const SOF_TIMESTAMPING_RX_SOFTWARE: c_uint = 1 << 3;
pub const SOF_TIMESTAMPING_SOFTWARE: c_uint = 1 << 4;
pub const SOF_TIMESTAMPING_SYS_HARDWARE: c_uint = 1 << 5;
pub const SOF_TIMESTAMPING_RAW_HARDWARE: c_uint = 1 << 6;
pub const SOF_TIMESTAMPING_OPT_ID: c_uint = 1 << 7;
pub const SOF_TIMESTAMPING_TX_SCHED: c_uint = 1 << 8;
pub const SOF_TIMESTAMPING_TX_ACK: c_uint = 1 << 9;
pub const SOF_TIMESTAMPING_OPT_CMSG: c_uint = 1 << 10;
pub const SOF_TIMESTAMPING_OPT_TSONLY: c_uint = 1 << 11;
pub const SOF_TIMESTAMPING_OPT_STATS: c_uint = 1 << 12;
pub const SOF_TIMESTAMPING_OPT_PKTINFO: c_uint = 1 << 13;
pub const SOF_TIMESTAMPING_OPT_TX_SWHW: c_uint = 1 << 14;
pub const SOF_TIMESTAMPING_BIND_PHC: c_uint = 1 << 15;
pub const SOF_TIMESTAMPING_OPT_ID_TCP: c_uint = 1 << 16;
pub const SOF_TIMESTAMPING_OPT_RX_FILTER: c_uint = 1 << 17;

#[deprecated(
    since = "0.2.55",
    note = "ENOATTR is not available on Android; use ENODATA instead"
)]
pub const ENOATTR: c_int = crate::ENODATA;

// linux/if_alg.h
pub const ALG_SET_KEY: c_int = 1;
pub const ALG_SET_IV: c_int = 2;
pub const ALG_SET_OP: c_int = 3;
pub const ALG_SET_AEAD_ASSOCLEN: c_int = 4;
pub const ALG_SET_AEAD_AUTHSIZE: c_int = 5;
pub const ALG_SET_DRBG_ENTROPY: c_int = 6;

pub const ALG_OP_DECRYPT: c_int = 0;
pub const ALG_OP_ENCRYPT: c_int = 1;

// sys/mman.h
pub const MLOCK_ONFAULT: c_int = 0x01;

// uapi/linux/vm_sockets.h
pub const VMADDR_CID_ANY: c_uint = 0xFFFFFFFF;
pub const VMADDR_CID_HYPERVISOR: c_uint = 0;
pub const VMADDR_CID_LOCAL: c_uint = 1;
pub const VMADDR_CID_HOST: c_uint = 2;
pub const VMADDR_PORT_ANY: c_uint = 0xFFFFFFFF;

// uapi/linux/inotify.h
pub const IN_ACCESS: u32 = 0x0000_0001;
pub const IN_MODIFY: u32 = 0x0000_0002;
pub const IN_ATTRIB: u32 = 0x0000_0004;
pub const IN_CLOSE_WRITE: u32 = 0x0000_0008;
pub const IN_CLOSE_NOWRITE: u32 = 0x0000_0010;
pub const IN_CLOSE: u32 = IN_CLOSE_WRITE | IN_CLOSE_NOWRITE;
pub const IN_OPEN: u32 = 0x0000_0020;
pub const IN_MOVED_FROM: u32 = 0x0000_0040;
pub const IN_MOVED_TO: u32 = 0x0000_0080;
pub const IN_MOVE: u32 = IN_MOVED_FROM | IN_MOVED_TO;
pub const IN_CREATE: u32 = 0x0000_0100;
pub const IN_DELETE: u32 = 0x0000_0200;
pub const IN_DELETE_SELF: u32 = 0x0000_0400;
pub const IN_MOVE_SELF: u32 = 0x0000_0800;
pub const IN_UNMOUNT: u32 = 0x0000_2000;
pub const IN_Q_OVERFLOW: u32 = 0x0000_4000;
pub const IN_IGNORED: u32 = 0x0000_8000;
pub const IN_ONLYDIR: u32 = 0x0100_0000;
pub const IN_DONT_FOLLOW: u32 = 0x0200_0000;
pub const IN_EXCL_UNLINK: u32 = 0x0400_0000;

pub const IN_MASK_CREATE: u32 = 0x1000_0000;
pub const IN_MASK_ADD: u32 = 0x2000_0000;
pub const IN_ISDIR: u32 = 0x4000_0000;
pub const IN_ONESHOT: u32 = 0x8000_0000;

pub const IN_ALL_EVENTS: u32 = IN_ACCESS
    | IN_MODIFY
    | IN_ATTRIB
    | IN_CLOSE_WRITE
    | IN_CLOSE_NOWRITE
    | IN_OPEN
    | IN_MOVED_FROM
    | IN_MOVED_TO
    | IN_DELETE
    | IN_CREATE
    | IN_DELETE_SELF
    | IN_MOVE_SELF;

pub const IN_CLOEXEC: c_int = O_CLOEXEC;
pub const IN_NONBLOCK: c_int = O_NONBLOCK;

pub const FUTEX_WAIT: c_int = 0;
pub const FUTEX_WAKE: c_int = 1;
pub const FUTEX_FD: c_int = 2;
pub const FUTEX_REQUEUE: c_int = 3;
pub const FUTEX_CMP_REQUEUE: c_int = 4;
pub const FUTEX_WAKE_OP: c_int = 5;
pub const FUTEX_LOCK_PI: c_int = 6;
pub const FUTEX_UNLOCK_PI: c_int = 7;
pub const FUTEX_TRYLOCK_PI: c_int = 8;
pub const FUTEX_WAIT_BITSET: c_int = 9;
pub const FUTEX_WAKE_BITSET: c_int = 10;
pub const FUTEX_WAIT_REQUEUE_PI: c_int = 11;
pub const FUTEX_CMP_REQUEUE_PI: c_int = 12;
pub const FUTEX_LOCK_PI2: c_int = 13;

pub const FUTEX_PRIVATE_FLAG: c_int = 128;
pub const FUTEX_CLOCK_REALTIME: c_int = 256;
pub const FUTEX_CMD_MASK: c_int = !(FUTEX_PRIVATE_FLAG | FUTEX_CLOCK_REALTIME);

// linux/errqueue.h
pub const SO_EE_ORIGIN_NONE: u8 = 0;
pub const SO_EE_ORIGIN_LOCAL: u8 = 1;
pub const SO_EE_ORIGIN_ICMP: u8 = 2;
pub const SO_EE_ORIGIN_ICMP6: u8 = 3;
pub const SO_EE_ORIGIN_TXSTATUS: u8 = 4;
pub const SO_EE_ORIGIN_TIMESTAMPING: u8 = SO_EE_ORIGIN_TXSTATUS;

// errno.h
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

pub const PRIO_PROCESS: c_int = 0;
pub const PRIO_PGRP: c_int = 1;
pub const PRIO_USER: c_int = 2;

// linux/sched.h
pub const SCHED_NORMAL: c_int = 0;
pub const SCHED_FIFO: c_int = 1;
pub const SCHED_RR: c_int = 2;
pub const SCHED_BATCH: c_int = 3;
pub const SCHED_IDLE: c_int = 5;
pub const SCHED_DEADLINE: c_int = 6;

pub const SCHED_RESET_ON_FORK: c_int = 0x40000000;

pub const CLONE_PIDFD: c_int = 0x1000;
pub const CLONE_CLEAR_SIGHAND: c_ulonglong = 0x100000000;
pub const CLONE_INTO_CGROUP: c_ulonglong = 0x200000000;

// linux/membarrier.h
pub const MEMBARRIER_CMD_QUERY: c_int = 0;
pub const MEMBARRIER_CMD_GLOBAL: c_int = 1 << 0;
pub const MEMBARRIER_CMD_GLOBAL_EXPEDITED: c_int = 1 << 1;
pub const MEMBARRIER_CMD_REGISTER_GLOBAL_EXPEDITED: c_int = 1 << 2;
pub const MEMBARRIER_CMD_PRIVATE_EXPEDITED: c_int = 1 << 3;
pub const MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED: c_int = 1 << 4;
pub const MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE: c_int = 1 << 5;
pub const MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE: c_int = 1 << 6;
pub const MEMBARRIER_CMD_PRIVATE_EXPEDITED_RSEQ: c_int = 1 << 7;
pub const MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_RSEQ: c_int = 1 << 8;

// linux/mempolicy.h
pub const MPOL_DEFAULT: c_int = 0;
pub const MPOL_PREFERRED: c_int = 1;
pub const MPOL_BIND: c_int = 2;
pub const MPOL_INTERLEAVE: c_int = 3;
pub const MPOL_LOCAL: c_int = 4;
pub const MPOL_F_NUMA_BALANCING: c_int = 1 << 13;
pub const MPOL_F_RELATIVE_NODES: c_int = 1 << 14;
pub const MPOL_F_STATIC_NODES: c_int = 1 << 15;

// bits/seek_constants.h
pub const SEEK_DATA: c_int = 3;
pub const SEEK_HOLE: c_int = 4;

// sys/socket.h
pub const AF_NFC: c_int = 39;
pub const AF_VSOCK: c_int = 40;
pub const PF_NFC: c_int = AF_NFC;
pub const PF_VSOCK: c_int = AF_VSOCK;

pub const SOMAXCONN: c_int = 128;

// sys/system_properties.h
pub const PROP_VALUE_MAX: c_int = 92;
pub const PROP_NAME_MAX: c_int = 32;

// sys/prctl.h
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
pub const PR_SVE_SET_VL: c_int = 50;
pub const PR_SVE_SET_VL_ONEXEC: c_int = 1 << 18;
pub const PR_SVE_GET_VL: c_int = 51;
pub const PR_SVE_VL_LEN_MASK: c_int = 0xffff;
pub const PR_SVE_VL_INHERIT: c_int = 1 << 17;
pub const PR_GET_SPECULATION_CTRL: c_int = 52;
pub const PR_SET_SPECULATION_CTRL: c_int = 53;
pub const PR_SPEC_STORE_BYPASS: c_int = 0;
pub const PR_SPEC_INDIRECT_BRANCH: c_int = 1;
pub const PR_SPEC_L1D_FLUSH: c_int = 2;
pub const PR_SPEC_NOT_AFFECTED: c_int = 0;
pub const PR_SPEC_PRCTL: c_ulong = 1 << 0;
pub const PR_SPEC_ENABLE: c_ulong = 1 << 1;
pub const PR_SPEC_DISABLE: c_ulong = 1 << 2;
pub const PR_SPEC_FORCE_DISABLE: c_ulong = 1 << 3;
pub const PR_SPEC_DISABLE_NOEXEC: c_ulong = 1 << 4;
pub const PR_PAC_RESET_KEYS: c_int = 54;
pub const PR_PAC_APIAKEY: c_ulong = 1 << 0;
pub const PR_PAC_APIBKEY: c_ulong = 1 << 1;
pub const PR_PAC_APDAKEY: c_ulong = 1 << 2;
pub const PR_PAC_APDBKEY: c_ulong = 1 << 3;
pub const PR_PAC_APGAKEY: c_ulong = 1 << 4;
pub const PR_SET_TAGGED_ADDR_CTRL: c_int = 55;
pub const PR_GET_TAGGED_ADDR_CTRL: c_int = 56;
pub const PR_TAGGED_ADDR_ENABLE: c_ulong = 1 << 0;
pub const PR_MTE_TCF_NONE: c_ulong = 0;
pub const PR_MTE_TCF_SYNC: c_ulong = 1 << 1;
pub const PR_MTE_TCF_ASYNC: c_ulong = 1 << 2;
pub const PR_MTE_TCF_MASK: c_ulong = PR_MTE_TCF_SYNC | PR_MTE_TCF_ASYNC;
pub const PR_MTE_TAG_SHIFT: c_ulong = 3;
pub const PR_MTE_TAG_MASK: c_ulong = 0xffff << PR_MTE_TAG_SHIFT;
pub const PR_MTE_TCF_SHIFT: c_ulong = 1;
pub const PR_SET_IO_FLUSHER: c_int = 57;
pub const PR_GET_IO_FLUSHER: c_int = 58;
pub const PR_SET_SYSCALL_USER_DISPATCH: c_int = 59;
pub const PR_SYS_DISPATCH_OFF: c_int = 0;
pub const PR_SYS_DISPATCH_ON: c_int = 1;
pub const SYSCALL_DISPATCH_FILTER_ALLOW: c_int = 0;
pub const SYSCALL_DISPATCH_FILTER_BLOCK: c_int = 1;
pub const PR_PAC_SET_ENABLED_KEYS: c_int = 60;
pub const PR_PAC_GET_ENABLED_KEYS: c_int = 61;
pub const PR_SCHED_CORE: c_int = 62;
pub const PR_SCHED_CORE_GET: c_int = 0;
pub const PR_SCHED_CORE_CREATE: c_int = 1;
pub const PR_SCHED_CORE_SHARE_TO: c_int = 2;
pub const PR_SCHED_CORE_SHARE_FROM: c_int = 3;
pub const PR_SCHED_CORE_MAX: c_int = 4;
pub const PR_SCHED_CORE_SCOPE_THREAD: c_int = 0;
pub const PR_SCHED_CORE_SCOPE_THREAD_GROUP: c_int = 1;
pub const PR_SCHED_CORE_SCOPE_PROCESS_GROUP: c_int = 2;
pub const PR_SME_SET_VL: c_int = 63;
pub const PR_SME_SET_VL_ONEXEC: c_int = 1 << 18;
pub const PR_SME_GET_VL: c_int = 64;
pub const PR_SME_VL_LEN_MASK: c_int = 0xffff;
pub const PR_SME_VL_INHERIT: c_int = 1 << 17;
pub const PR_SET_MDWE: c_int = 65;
pub const PR_MDWE_REFUSE_EXEC_GAIN: c_ulong = 1 << 0;
pub const PR_MDWE_NO_INHERIT: c_ulong = 1 << 1;
pub const PR_GET_MDWE: c_int = 66;
pub const PR_SET_VMA: c_int = 0x53564d41;
pub const PR_SET_VMA_ANON_NAME: c_int = 0;
pub const PR_GET_AUXV: c_int = 0x41555856;
pub const PR_SET_MEMORY_MERGE: c_int = 67;
pub const PR_GET_MEMORY_MERGE: c_int = 68;
pub const PR_RISCV_V_SET_CONTROL: c_int = 69;
pub const PR_RISCV_V_GET_CONTROL: c_int = 70;
pub const PR_RISCV_V_VSTATE_CTRL_DEFAULT: c_int = 0;
pub const PR_RISCV_V_VSTATE_CTRL_OFF: c_int = 1;
pub const PR_RISCV_V_VSTATE_CTRL_ON: c_int = 2;
pub const PR_RISCV_V_VSTATE_CTRL_INHERIT: c_int = 1 << 4;
pub const PR_RISCV_V_VSTATE_CTRL_CUR_MASK: c_int = 0x3;
pub const PR_RISCV_V_VSTATE_CTRL_NEXT_MASK: c_int = 0xc;
pub const PR_RISCV_V_VSTATE_CTRL_MASK: c_int = 0x1f;

// linux/if_addr.h
pub const IFA_UNSPEC: c_ushort = 0;
pub const IFA_ADDRESS: c_ushort = 1;
pub const IFA_LOCAL: c_ushort = 2;
pub const IFA_LABEL: c_ushort = 3;
pub const IFA_BROADCAST: c_ushort = 4;
pub const IFA_ANYCAST: c_ushort = 5;
pub const IFA_CACHEINFO: c_ushort = 6;
pub const IFA_MULTICAST: c_ushort = 7;

pub const IFA_F_SECONDARY: u32 = 0x01;
pub const IFA_F_TEMPORARY: u32 = 0x01;
pub const IFA_F_NODAD: u32 = 0x02;
pub const IFA_F_OPTIMISTIC: u32 = 0x04;
pub const IFA_F_DADFAILED: u32 = 0x08;
pub const IFA_F_HOMEADDRESS: u32 = 0x10;
pub const IFA_F_DEPRECATED: u32 = 0x20;
pub const IFA_F_TENTATIVE: u32 = 0x40;
pub const IFA_F_PERMANENT: u32 = 0x80;

// linux/if_link.h
pub const IFLA_UNSPEC: c_ushort = 0;
pub const IFLA_ADDRESS: c_ushort = 1;
pub const IFLA_BROADCAST: c_ushort = 2;
pub const IFLA_IFNAME: c_ushort = 3;
pub const IFLA_MTU: c_ushort = 4;
pub const IFLA_LINK: c_ushort = 5;
pub const IFLA_QDISC: c_ushort = 6;
pub const IFLA_STATS: c_ushort = 7;
pub const IFLA_COST: c_ushort = 8;
pub const IFLA_PRIORITY: c_ushort = 9;
pub const IFLA_MASTER: c_ushort = 10;
pub const IFLA_WIRELESS: c_ushort = 11;
pub const IFLA_PROTINFO: c_ushort = 12;
pub const IFLA_TXQLEN: c_ushort = 13;
pub const IFLA_MAP: c_ushort = 14;
pub const IFLA_WEIGHT: c_ushort = 15;
pub const IFLA_OPERSTATE: c_ushort = 16;
pub const IFLA_LINKMODE: c_ushort = 17;
pub const IFLA_LINKINFO: c_ushort = 18;
pub const IFLA_NET_NS_PID: c_ushort = 19;
pub const IFLA_IFALIAS: c_ushort = 20;
pub const IFLA_NUM_VF: c_ushort = 21;
pub const IFLA_VFINFO_LIST: c_ushort = 22;
pub const IFLA_STATS64: c_ushort = 23;
pub const IFLA_VF_PORTS: c_ushort = 24;
pub const IFLA_PORT_SELF: c_ushort = 25;
pub const IFLA_AF_SPEC: c_ushort = 26;
pub const IFLA_GROUP: c_ushort = 27;
pub const IFLA_NET_NS_FD: c_ushort = 28;
pub const IFLA_EXT_MASK: c_ushort = 29;
pub const IFLA_PROMISCUITY: c_ushort = 30;
pub const IFLA_NUM_TX_QUEUES: c_ushort = 31;
pub const IFLA_NUM_RX_QUEUES: c_ushort = 32;
pub const IFLA_CARRIER: c_ushort = 33;
pub const IFLA_PHYS_PORT_ID: c_ushort = 34;
pub const IFLA_CARRIER_CHANGES: c_ushort = 35;
pub const IFLA_PHYS_SWITCH_ID: c_ushort = 36;
pub const IFLA_LINK_NETNSID: c_ushort = 37;
pub const IFLA_PHYS_PORT_NAME: c_ushort = 38;
pub const IFLA_PROTO_DOWN: c_ushort = 39;
pub const IFLA_GSO_MAX_SEGS: c_ushort = 40;
pub const IFLA_GSO_MAX_SIZE: c_ushort = 41;
pub const IFLA_PAD: c_ushort = 42;
pub const IFLA_XDP: c_ushort = 43;
pub const IFLA_EVENT: c_ushort = 44;
pub const IFLA_NEW_NETNSID: c_ushort = 45;
pub const IFLA_IF_NETNSID: c_ushort = 46;
pub const IFLA_TARGET_NETNSID: c_ushort = IFLA_IF_NETNSID;
pub const IFLA_CARRIER_UP_COUNT: c_ushort = 47;
pub const IFLA_CARRIER_DOWN_COUNT: c_ushort = 48;
pub const IFLA_NEW_IFINDEX: c_ushort = 49;
pub const IFLA_MIN_MTU: c_ushort = 50;
pub const IFLA_MAX_MTU: c_ushort = 51;
pub const IFLA_PROP_LIST: c_ushort = 52;
pub const IFLA_ALT_IFNAME: c_ushort = 53;
pub const IFLA_PERM_ADDRESS: c_ushort = 54;
pub const IFLA_PROTO_DOWN_REASON: c_ushort = 55;
pub const IFLA_PARENT_DEV_NAME: c_ushort = 56;
pub const IFLA_PARENT_DEV_BUS_NAME: c_ushort = 57;
pub const IFLA_GRO_MAX_SIZE: c_ushort = 58;
pub const IFLA_TSO_MAX_SIZE: c_ushort = 59;
pub const IFLA_TSO_MAX_SEGS: c_ushort = 60;
pub const IFLA_ALLMULTI: c_ushort = 61;
pub const IFLA_DEVLINK_PORT: c_ushort = 62;
pub const IFLA_GSO_IPV4_MAX_SIZE: c_ushort = 63;
pub const IFLA_GRO_IPV4_MAX_SIZE: c_ushort = 64;

pub const IFLA_INFO_UNSPEC: c_ushort = 0;
pub const IFLA_INFO_KIND: c_ushort = 1;
pub const IFLA_INFO_DATA: c_ushort = 2;
pub const IFLA_INFO_XSTATS: c_ushort = 3;
pub const IFLA_INFO_SLAVE_KIND: c_ushort = 4;
pub const IFLA_INFO_SLAVE_DATA: c_ushort = 5;

// linux/rtnetlink.h
pub const TCA_UNSPEC: c_ushort = 0;
pub const TCA_KIND: c_ushort = 1;
pub const TCA_OPTIONS: c_ushort = 2;
pub const TCA_STATS: c_ushort = 3;
pub const TCA_XSTATS: c_ushort = 4;
pub const TCA_RATE: c_ushort = 5;
pub const TCA_FCNT: c_ushort = 6;
pub const TCA_STATS2: c_ushort = 7;
pub const TCA_STAB: c_ushort = 8;

pub const RTM_NEWLINK: u16 = 16;
pub const RTM_DELLINK: u16 = 17;
pub const RTM_GETLINK: u16 = 18;
pub const RTM_SETLINK: u16 = 19;
pub const RTM_NEWADDR: u16 = 20;
pub const RTM_DELADDR: u16 = 21;
pub const RTM_GETADDR: u16 = 22;
pub const RTM_NEWROUTE: u16 = 24;
pub const RTM_DELROUTE: u16 = 25;
pub const RTM_GETROUTE: u16 = 26;
pub const RTM_NEWNEIGH: u16 = 28;
pub const RTM_DELNEIGH: u16 = 29;
pub const RTM_GETNEIGH: u16 = 30;
pub const RTM_NEWRULE: u16 = 32;
pub const RTM_DELRULE: u16 = 33;
pub const RTM_GETRULE: u16 = 34;
pub const RTM_NEWQDISC: u16 = 36;
pub const RTM_DELQDISC: u16 = 37;
pub const RTM_GETQDISC: u16 = 38;
pub const RTM_NEWTCLASS: u16 = 40;
pub const RTM_DELTCLASS: u16 = 41;
pub const RTM_GETTCLASS: u16 = 42;
pub const RTM_NEWTFILTER: u16 = 44;
pub const RTM_DELTFILTER: u16 = 45;
pub const RTM_GETTFILTER: u16 = 46;
pub const RTM_NEWACTION: u16 = 48;
pub const RTM_DELACTION: u16 = 49;
pub const RTM_GETACTION: u16 = 50;
pub const RTM_NEWPREFIX: u16 = 52;
pub const RTM_GETMULTICAST: u16 = 58;
pub const RTM_GETANYCAST: u16 = 62;
pub const RTM_NEWNEIGHTBL: u16 = 64;
pub const RTM_GETNEIGHTBL: u16 = 66;
pub const RTM_SETNEIGHTBL: u16 = 67;
pub const RTM_NEWNDUSEROPT: u16 = 68;
pub const RTM_NEWADDRLABEL: u16 = 72;
pub const RTM_DELADDRLABEL: u16 = 73;
pub const RTM_GETADDRLABEL: u16 = 74;
pub const RTM_GETDCB: u16 = 78;
pub const RTM_SETDCB: u16 = 79;
pub const RTM_NEWNETCONF: u16 = 80;
pub const RTM_GETNETCONF: u16 = 82;
pub const RTM_NEWMDB: u16 = 84;
pub const RTM_DELMDB: u16 = 85;
pub const RTM_GETMDB: u16 = 86;
pub const RTM_NEWNSID: u16 = 88;
pub const RTM_DELNSID: u16 = 89;
pub const RTM_GETNSID: u16 = 90;

pub const RTM_F_NOTIFY: c_uint = 0x100;
pub const RTM_F_CLONED: c_uint = 0x200;
pub const RTM_F_EQUALIZE: c_uint = 0x400;
pub const RTM_F_PREFIX: c_uint = 0x800;

pub const RTA_UNSPEC: c_ushort = 0;
pub const RTA_DST: c_ushort = 1;
pub const RTA_SRC: c_ushort = 2;
pub const RTA_IIF: c_ushort = 3;
pub const RTA_OIF: c_ushort = 4;
pub const RTA_GATEWAY: c_ushort = 5;
pub const RTA_PRIORITY: c_ushort = 6;
pub const RTA_PREFSRC: c_ushort = 7;
pub const RTA_METRICS: c_ushort = 8;
pub const RTA_MULTIPATH: c_ushort = 9;
pub const RTA_PROTOINFO: c_ushort = 10; // No longer used
pub const RTA_FLOW: c_ushort = 11;
pub const RTA_CACHEINFO: c_ushort = 12;
pub const RTA_SESSION: c_ushort = 13; // No longer used
pub const RTA_MP_ALGO: c_ushort = 14; // No longer used
pub const RTA_TABLE: c_ushort = 15;
pub const RTA_MARK: c_ushort = 16;
pub const RTA_MFC_STATS: c_ushort = 17;

pub const RTN_UNSPEC: c_uchar = 0;
pub const RTN_UNICAST: c_uchar = 1;
pub const RTN_LOCAL: c_uchar = 2;
pub const RTN_BROADCAST: c_uchar = 3;
pub const RTN_ANYCAST: c_uchar = 4;
pub const RTN_MULTICAST: c_uchar = 5;
pub const RTN_BLACKHOLE: c_uchar = 6;
pub const RTN_UNREACHABLE: c_uchar = 7;
pub const RTN_PROHIBIT: c_uchar = 8;
pub const RTN_THROW: c_uchar = 9;
pub const RTN_NAT: c_uchar = 10;
pub const RTN_XRESOLVE: c_uchar = 11;

pub const RTPROT_UNSPEC: c_uchar = 0;
pub const RTPROT_REDIRECT: c_uchar = 1;
pub const RTPROT_KERNEL: c_uchar = 2;
pub const RTPROT_BOOT: c_uchar = 3;
pub const RTPROT_STATIC: c_uchar = 4;

pub const RT_SCOPE_UNIVERSE: c_uchar = 0;
pub const RT_SCOPE_SITE: c_uchar = 200;
pub const RT_SCOPE_LINK: c_uchar = 253;
pub const RT_SCOPE_HOST: c_uchar = 254;
pub const RT_SCOPE_NOWHERE: c_uchar = 255;

pub const RT_TABLE_UNSPEC: c_uchar = 0;
pub const RT_TABLE_COMPAT: c_uchar = 252;
pub const RT_TABLE_DEFAULT: c_uchar = 253;
pub const RT_TABLE_MAIN: c_uchar = 254;
pub const RT_TABLE_LOCAL: c_uchar = 255;

pub const RTMSG_NEWDEVICE: u32 = 0x11;
pub const RTMSG_DELDEVICE: u32 = 0x12;
pub const RTMSG_NEWROUTE: u32 = 0x21;
pub const RTMSG_DELROUTE: u32 = 0x22;

pub const CTL_KERN: c_int = 1;
pub const CTL_VM: c_int = 2;
pub const CTL_NET: c_int = 3;
pub const CTL_FS: c_int = 5;
pub const CTL_DEBUG: c_int = 6;
pub const CTL_DEV: c_int = 7;
pub const CTL_BUS: c_int = 8;
pub const CTL_ABI: c_int = 9;
pub const CTL_CPU: c_int = 10;

pub const CTL_BUS_ISA: c_int = 1;

pub const INOTIFY_MAX_USER_INSTANCES: c_int = 1;
pub const INOTIFY_MAX_USER_WATCHES: c_int = 2;
pub const INOTIFY_MAX_QUEUED_EVENTS: c_int = 3;

pub const KERN_OSTYPE: c_int = 1;
pub const KERN_OSRELEASE: c_int = 2;
pub const KERN_OSREV: c_int = 3;
pub const KERN_VERSION: c_int = 4;
pub const KERN_SECUREMASK: c_int = 5;
pub const KERN_PROF: c_int = 6;
pub const KERN_NODENAME: c_int = 7;
pub const KERN_DOMAINNAME: c_int = 8;
pub const KERN_PANIC: c_int = 15;
pub const KERN_REALROOTDEV: c_int = 16;
pub const KERN_SPARC_REBOOT: c_int = 21;
pub const KERN_CTLALTDEL: c_int = 22;
pub const KERN_PRINTK: c_int = 23;
pub const KERN_NAMETRANS: c_int = 24;
pub const KERN_PPC_HTABRECLAIM: c_int = 25;
pub const KERN_PPC_ZEROPAGED: c_int = 26;
pub const KERN_PPC_POWERSAVE_NAP: c_int = 27;
pub const KERN_MODPROBE: c_int = 28;
pub const KERN_SG_BIG_BUFF: c_int = 29;
pub const KERN_ACCT: c_int = 30;
pub const KERN_PPC_L2CR: c_int = 31;
pub const KERN_RTSIGNR: c_int = 32;
pub const KERN_RTSIGMAX: c_int = 33;
pub const KERN_SHMMAX: c_int = 34;
pub const KERN_MSGMAX: c_int = 35;
pub const KERN_MSGMNB: c_int = 36;
pub const KERN_MSGPOOL: c_int = 37;
pub const KERN_SYSRQ: c_int = 38;
pub const KERN_MAX_THREADS: c_int = 39;
pub const KERN_RANDOM: c_int = 40;
pub const KERN_SHMALL: c_int = 41;
pub const KERN_MSGMNI: c_int = 42;
pub const KERN_SEM: c_int = 43;
pub const KERN_SPARC_STOP_A: c_int = 44;
pub const KERN_SHMMNI: c_int = 45;
pub const KERN_OVERFLOWUID: c_int = 46;
pub const KERN_OVERFLOWGID: c_int = 47;
pub const KERN_SHMPATH: c_int = 48;
pub const KERN_HOTPLUG: c_int = 49;
pub const KERN_IEEE_EMULATION_WARNINGS: c_int = 50;
pub const KERN_S390_USER_DEBUG_LOGGING: c_int = 51;
pub const KERN_CORE_USES_PID: c_int = 52;
pub const KERN_TAINTED: c_int = 53;
pub const KERN_CADPID: c_int = 54;
pub const KERN_PIDMAX: c_int = 55;
pub const KERN_CORE_PATTERN: c_int = 56;
pub const KERN_PANIC_ON_OOPS: c_int = 57;
pub const KERN_HPPA_PWRSW: c_int = 58;
pub const KERN_HPPA_UNALIGNED: c_int = 59;
pub const KERN_PRINTK_RATELIMIT: c_int = 60;
pub const KERN_PRINTK_RATELIMIT_BURST: c_int = 61;
pub const KERN_PTY: c_int = 62;
pub const KERN_NGROUPS_MAX: c_int = 63;
pub const KERN_SPARC_SCONS_PWROFF: c_int = 64;
pub const KERN_HZ_TIMER: c_int = 65;
pub const KERN_UNKNOWN_NMI_PANIC: c_int = 66;
pub const KERN_BOOTLOADER_TYPE: c_int = 67;
pub const KERN_RANDOMIZE: c_int = 68;
pub const KERN_SETUID_DUMPABLE: c_int = 69;
pub const KERN_SPIN_RETRY: c_int = 70;
pub const KERN_ACPI_VIDEO_FLAGS: c_int = 71;
pub const KERN_IA64_UNALIGNED: c_int = 72;
pub const KERN_COMPAT_LOG: c_int = 73;
pub const KERN_MAX_LOCK_DEPTH: c_int = 74;

pub const VM_OVERCOMMIT_MEMORY: c_int = 5;
pub const VM_PAGE_CLUSTER: c_int = 10;
pub const VM_DIRTY_BACKGROUND: c_int = 11;
pub const VM_DIRTY_RATIO: c_int = 12;
pub const VM_DIRTY_WB_CS: c_int = 13;
pub const VM_DIRTY_EXPIRE_CS: c_int = 14;
pub const VM_NR_PDFLUSH_THREADS: c_int = 15;
pub const VM_OVERCOMMIT_RATIO: c_int = 16;
pub const VM_PAGEBUF: c_int = 17;
pub const VM_HUGETLB_PAGES: c_int = 18;
pub const VM_SWAPPINESS: c_int = 19;
pub const VM_LOWMEM_RESERVE_RATIO: c_int = 20;
pub const VM_MIN_FREE_KBYTES: c_int = 21;
pub const VM_MAX_MAP_COUNT: c_int = 22;
pub const VM_LAPTOP_MODE: c_int = 23;
pub const VM_BLOCK_DUMP: c_int = 24;
pub const VM_HUGETLB_GROUP: c_int = 25;
pub const VM_VFS_CACHE_PRESSURE: c_int = 26;
pub const VM_LEGACY_VA_LAYOUT: c_int = 27;
pub const VM_SWAP_TOKEN_TIMEOUT: c_int = 28;
pub const VM_DROP_PAGECACHE: c_int = 29;
pub const VM_PERCPU_PAGELIST_FRACTION: c_int = 30;
pub const VM_ZONE_RECLAIM_MODE: c_int = 31;
pub const VM_MIN_UNMAPPED: c_int = 32;
pub const VM_PANIC_ON_OOM: c_int = 33;
pub const VM_VDSO_ENABLED: c_int = 34;

pub const NET_CORE: c_int = 1;
pub const NET_ETHER: c_int = 2;
pub const NET_802: c_int = 3;
pub const NET_UNIX: c_int = 4;
pub const NET_IPV4: c_int = 5;
pub const NET_IPX: c_int = 6;
pub const NET_ATALK: c_int = 7;
pub const NET_NETROM: c_int = 8;
pub const NET_AX25: c_int = 9;
pub const NET_BRIDGE: c_int = 10;
pub const NET_ROSE: c_int = 11;
pub const NET_IPV6: c_int = 12;
pub const NET_X25: c_int = 13;
pub const NET_TR: c_int = 14;
pub const NET_DECNET: c_int = 15;
pub const NET_ECONET: c_int = 16;
pub const NET_SCTP: c_int = 17;
pub const NET_LLC: c_int = 18;
pub const NET_NETFILTER: c_int = 19;
pub const NET_DCCP: c_int = 20;
pub const HUGETLB_FLAG_ENCODE_SHIFT: c_int = 26;
pub const MAP_HUGE_SHIFT: c_int = HUGETLB_FLAG_ENCODE_SHIFT;

// include/linux/sched.h
pub const PF_VCPU: c_int = 0x00000001;
pub const PF_IDLE: c_int = 0x00000002;
pub const PF_EXITING: c_int = 0x00000004;
pub const PF_POSTCOREDUMP: c_int = 0x00000008;
pub const PF_IO_WORKER: c_int = 0x00000010;
pub const PF_WQ_WORKER: c_int = 0x00000020;
pub const PF_FORKNOEXEC: c_int = 0x00000040;
pub const PF_MCE_PROCESS: c_int = 0x00000080;
pub const PF_SUPERPRIV: c_int = 0x00000100;
pub const PF_DUMPCORE: c_int = 0x00000200;
pub const PF_SIGNALED: c_int = 0x00000400;
pub const PF_MEMALLOC: c_int = 0x00000800;
pub const PF_NPROC_EXCEEDED: c_int = 0x00001000;
pub const PF_USED_MATH: c_int = 0x00002000;
pub const PF_USER_WORKER: c_int = 0x00004000;
pub const PF_NOFREEZE: c_int = 0x00008000;

pub const PF_KSWAPD: c_int = 0x00020000;
pub const PF_MEMALLOC_NOFS: c_int = 0x00040000;
pub const PF_MEMALLOC_NOIO: c_int = 0x00080000;
pub const PF_LOCAL_THROTTLE: c_int = 0x00100000;
pub const PF_KTHREAD: c_int = 0x00200000;
pub const PF_RANDOMIZE: c_int = 0x00400000;

pub const PF_NO_SETAFFINITY: c_int = 0x04000000;
pub const PF_MCE_EARLY: c_int = 0x08000000;
pub const PF_MEMALLOC_PIN: c_int = 0x10000000;

pub const PF_SUSPEND_TASK: c_int = 0x80000000;

pub const KLOG_CLOSE: c_int = 0;
pub const KLOG_OPEN: c_int = 1;
pub const KLOG_READ: c_int = 2;
pub const KLOG_READ_ALL: c_int = 3;
pub const KLOG_READ_CLEAR: c_int = 4;
pub const KLOG_CLEAR: c_int = 5;
pub const KLOG_CONSOLE_OFF: c_int = 6;
pub const KLOG_CONSOLE_ON: c_int = 7;
pub const KLOG_CONSOLE_LEVEL: c_int = 8;
pub const KLOG_SIZE_UNREAD: c_int = 9;
pub const KLOG_SIZE_BUFFER: c_int = 10;

// From NDK's linux/auxvec.h
pub const AT_NULL: c_ulong = 0;
pub const AT_IGNORE: c_ulong = 1;
pub const AT_EXECFD: c_ulong = 2;
pub const AT_PHDR: c_ulong = 3;
pub const AT_PHENT: c_ulong = 4;
pub const AT_PHNUM: c_ulong = 5;
pub const AT_PAGESZ: c_ulong = 6;
pub const AT_BASE: c_ulong = 7;
pub const AT_FLAGS: c_ulong = 8;
pub const AT_ENTRY: c_ulong = 9;
pub const AT_NOTELF: c_ulong = 10;
pub const AT_UID: c_ulong = 11;
pub const AT_EUID: c_ulong = 12;
pub const AT_GID: c_ulong = 13;
pub const AT_EGID: c_ulong = 14;
pub const AT_PLATFORM: c_ulong = 15;
pub const AT_HWCAP: c_ulong = 16;
pub const AT_CLKTCK: c_ulong = 17;
pub const AT_SECURE: c_ulong = 23;
pub const AT_BASE_PLATFORM: c_ulong = 24;
pub const AT_RANDOM: c_ulong = 25;
pub const AT_HWCAP2: c_ulong = 26;
pub const AT_RSEQ_FEATURE_SIZE: c_ulong = 27;
pub const AT_RSEQ_ALIGN: c_ulong = 28;
pub const AT_EXECFN: c_ulong = 31;
pub const AT_MINSIGSTKSZ: c_ulong = 51;

// siginfo.h
pub const SI_DETHREAD: c_int = -7;
pub const TRAP_PERF: c_int = 6;

// Most `*_SUPER_MAGIC` constants are defined at the `linux_like` level; the
// following are only available on newer Linux versions than the versions
// currently used in CI in some configurations, so we define them here.
cfg_if! {
    if #[cfg(not(target_arch = "s390x"))] {
        pub const XFS_SUPER_MAGIC: c_long = 0x58465342;
    } else if #[cfg(target_arch = "s390x")] {
        pub const XFS_SUPER_MAGIC: c_uint = 0x58465342;
    }
}

f! {
    pub fn CMSG_NXTHDR(mhdr: *const msghdr, cmsg: *const cmsghdr) -> *mut cmsghdr {
        let next = (cmsg as usize + super::CMSG_ALIGN((*cmsg).cmsg_len as usize)) as *mut cmsghdr;
        let max = (*mhdr).msg_control as usize + (*mhdr).msg_controllen as usize;
        if (next.offset(1)) as usize > max {
            core::ptr::null_mut::<cmsghdr>()
        } else {
            next as *mut cmsghdr
        }
    }

    pub fn CPU_ALLOC_SIZE(count: c_int) -> size_t {
        let _dummy: cpu_set_t = mem::zeroed();
        let size_in_bits = 8 * size_of_val(&_dummy.__bits[0]);
        ((count as size_t + size_in_bits - 1) / 8) as size_t
    }

    pub fn CPU_ZERO(cpuset: &mut cpu_set_t) -> () {
        for slot in cpuset.__bits.iter_mut() {
            *slot = 0;
        }
    }

    pub fn CPU_SET(cpu: usize, cpuset: &mut cpu_set_t) -> () {
        let size_in_bits = 8 * size_of_val(&cpuset.__bits[0]); // 32, 64 etc
        let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
        cpuset.__bits[idx] |= 1 << offset;
        ()
    }

    pub fn CPU_CLR(cpu: usize, cpuset: &mut cpu_set_t) -> () {
        let size_in_bits = 8 * size_of_val(&cpuset.__bits[0]); // 32, 64 etc
        let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
        cpuset.__bits[idx] &= !(1 << offset);
        ()
    }

    pub fn CPU_ISSET(cpu: usize, cpuset: &cpu_set_t) -> bool {
        let size_in_bits = 8 * size_of_val(&cpuset.__bits[0]);
        let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
        0 != (cpuset.__bits[idx] & (1 << offset))
    }

    pub fn CPU_COUNT_S(size: usize, cpuset: &cpu_set_t) -> c_int {
        let mut s: u32 = 0;
        let size_of_mask = size_of_val(&cpuset.__bits[0]);
        for i in cpuset.__bits[..(size / size_of_mask)].iter() {
            s += i.count_ones();
        }
        s as c_int
    }

    pub fn CPU_COUNT(cpuset: &cpu_set_t) -> c_int {
        CPU_COUNT_S(size_of::<cpu_set_t>(), cpuset)
    }

    pub fn CPU_EQUAL(set1: &cpu_set_t, set2: &cpu_set_t) -> bool {
        set1.__bits == set2.__bits
    }

    pub fn NLA_ALIGN(len: c_int) -> c_int {
        return ((len) + NLA_ALIGNTO - 1) & !(NLA_ALIGNTO - 1);
    }

    pub fn SO_EE_OFFENDER(ee: *const crate::sock_extended_err) -> *mut crate::sockaddr {
        ee.offset(1) as *mut crate::sockaddr
    }
}

safe_f! {
    pub {const} fn makedev(ma: c_uint, mi: c_uint) -> crate::dev_t {
        let ma = ma as crate::dev_t;
        let mi = mi as crate::dev_t;
        ((ma & 0xfff) << 8) | (mi & 0xff) | ((mi & 0xfff00) << 12)
    }

    pub {const} fn major(dev: crate::dev_t) -> c_int {
        ((dev >> 8) & 0xfff) as c_int
    }

    pub {const} fn minor(dev: crate::dev_t) -> c_int {
        ((dev & 0xff) | ((dev >> 12) & 0xfff00)) as c_int
    }
}

extern "C" {
    pub fn setgrent();
    pub fn endgrent();
    pub fn getgrent() -> *mut crate::group;
    pub fn getrlimit64(resource: c_int, rlim: *mut rlimit64) -> c_int;
    pub fn setrlimit64(resource: c_int, rlim: *const rlimit64) -> c_int;
    pub fn getrlimit(resource: c_int, rlim: *mut crate::rlimit) -> c_int;
    pub fn setrlimit(resource: c_int, rlim: *const crate::rlimit) -> c_int;
    pub fn prlimit(
        pid: crate::pid_t,
        resource: c_int,
        new_limit: *const crate::rlimit,
        old_limit: *mut crate::rlimit,
    ) -> c_int;
    pub fn prlimit64(
        pid: crate::pid_t,
        resource: c_int,
        new_limit: *const crate::rlimit64,
        old_limit: *mut crate::rlimit64,
    ) -> c_int;
    pub fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;

    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut crate::timezone) -> c_int;
    pub fn mlock2(addr: *const c_void, len: size_t, flags: c_int) -> c_int;
    pub fn madvise(addr: *mut c_void, len: size_t, advice: c_int) -> c_int;
    pub fn msync(addr: *mut c_void, len: size_t, flags: c_int) -> c_int;
    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;
    pub fn getnameinfo(
        sa: *const crate::sockaddr,
        salen: crate::socklen_t,
        host: *mut c_char,
        hostlen: size_t,
        serv: *mut c_char,
        servlen: size_t,
        flags: c_int,
    ) -> c_int;
    pub fn preadv(fd: c_int, iov: *const crate::iovec, count: c_int, offset: off_t) -> ssize_t;
    pub fn pwritev(fd: c_int, iov: *const crate::iovec, count: c_int, offset: off_t) -> ssize_t;
    pub fn process_vm_readv(
        pid: crate::pid_t,
        local_iov: *const crate::iovec,
        local_iov_count: c_ulong,
        remote_iov: *const crate::iovec,
        remote_iov_count: c_ulong,
        flags: c_ulong,
    ) -> ssize_t;
    pub fn process_vm_writev(
        pid: crate::pid_t,
        local_iov: *const crate::iovec,
        local_iov_count: c_ulong,
        remote_iov: *const crate::iovec,
        remote_iov_count: c_ulong,
        flags: c_ulong,
    ) -> ssize_t;
    pub fn ptrace(request: c_int, ...) -> c_long;
    pub fn getpriority(which: c_int, who: crate::id_t) -> c_int;
    pub fn setpriority(which: c_int, who: crate::id_t, prio: c_int) -> c_int;
    pub fn __sched_cpualloc(count: size_t) -> *mut crate::cpu_set_t;
    pub fn __sched_cpufree(set: *mut crate::cpu_set_t);
    pub fn __sched_cpucount(setsize: size_t, set: *const cpu_set_t) -> c_int;
    pub fn sched_getcpu() -> c_int;
    pub fn mallinfo() -> crate::mallinfo;
    // available from API 23
    pub fn malloc_info(options: c_int, stream: *mut crate::FILE) -> c_int;

    pub fn malloc_usable_size(ptr: *const c_void) -> size_t;

    pub fn utmpname(name: *const c_char) -> c_int;
    pub fn setutent();
    pub fn getutent() -> *mut utmp;

    pub fn seekdir(dirp: *mut crate::DIR, loc: c_long);
    pub fn telldir(dirp: *mut crate::DIR) -> c_long;
    pub fn fallocate(fd: c_int, mode: c_int, offset: off_t, len: off_t) -> c_int;
    pub fn fallocate64(fd: c_int, mode: c_int, offset: off64_t, len: off64_t) -> c_int;
    pub fn posix_fallocate(fd: c_int, offset: off_t, len: off_t) -> c_int;
    pub fn posix_fallocate64(fd: c_int, offset: off64_t, len: off64_t) -> c_int;
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
    pub fn signalfd(fd: c_int, mask: *const crate::sigset_t, flags: c_int) -> c_int;
    pub fn timerfd_create(clock: crate::clockid_t, flags: c_int) -> c_int;
    pub fn timerfd_gettime(fd: c_int, current_value: *mut itimerspec) -> c_int;
    pub fn timerfd_settime(
        fd: c_int,
        flags: c_int,
        new_value: *const itimerspec,
        old_value: *mut itimerspec,
    ) -> c_int;
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
    pub fn epoll_create(size: c_int) -> c_int;
    pub fn epoll_create1(flags: c_int) -> c_int;
    pub fn epoll_wait(
        epfd: c_int,
        events: *mut crate::epoll_event,
        maxevents: c_int,
        timeout: c_int,
    ) -> c_int;
    pub fn epoll_ctl(epfd: c_int, op: c_int, fd: c_int, event: *mut crate::epoll_event) -> c_int;
    pub fn pthread_getschedparam(
        native: crate::pthread_t,
        policy: *mut c_int,
        param: *mut crate::sched_param,
    ) -> c_int;
    pub fn unshare(flags: c_int) -> c_int;
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
    pub fn eventfd_read(fd: c_int, value: *mut eventfd_t) -> c_int;
    pub fn eventfd_write(fd: c_int, value: eventfd_t) -> c_int;
    pub fn sched_rr_get_interval(pid: crate::pid_t, tp: *mut crate::timespec) -> c_int;
    pub fn sem_timedwait(sem: *mut sem_t, abstime: *const crate::timespec) -> c_int;
    pub fn sem_getvalue(sem: *mut sem_t, sval: *mut c_int) -> c_int;
    pub fn sched_setparam(pid: crate::pid_t, param: *const crate::sched_param) -> c_int;
    pub fn setns(fd: c_int, nstype: c_int) -> c_int;
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
    pub fn personality(persona: c_uint) -> c_int;
    pub fn prctl(option: c_int, ...) -> c_int;
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
    pub fn pthread_attr_getinheritsched(
        attr: *const crate::pthread_attr_t,
        flag: *mut c_int,
    ) -> c_int;
    pub fn pthread_attr_setinheritsched(attr: *mut crate::pthread_attr_t, flag: c_int) -> c_int;
    pub fn sethostname(name: *const c_char, len: size_t) -> c_int;
    pub fn sched_get_priority_min(policy: c_int) -> c_int;
    pub fn pthread_condattr_getpshared(
        attr: *const pthread_condattr_t,
        pshared: *mut c_int,
    ) -> c_int;
    pub fn sysinfo(info: *mut crate::sysinfo) -> c_int;
    pub fn umount2(target: *const c_char, flags: c_int) -> c_int;
    pub fn pthread_setschedparam(
        native: crate::pthread_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;
    pub fn swapon(path: *const c_char, swapflags: c_int) -> c_int;
    pub fn sched_setscheduler(
        pid: crate::pid_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;
    pub fn sendfile(out_fd: c_int, in_fd: c_int, offset: *mut off_t, count: size_t) -> ssize_t;
    pub fn sendfile64(out_fd: c_int, in_fd: c_int, offset: *mut off64_t, count: size_t) -> ssize_t;
    pub fn setfsgid(gid: crate::gid_t) -> c_int;
    pub fn setfsuid(uid: crate::uid_t) -> c_int;
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
    pub fn getgrnam_r(
        name: *const c_char,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn pthread_sigmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t) -> c_int;
    pub fn sem_open(name: *const c_char, oflag: c_int, ...) -> *mut sem_t;
    pub fn getgrnam(name: *const c_char) -> *mut crate::group;
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
    pub fn sigtimedwait(
        set: *const sigset_t,
        info: *mut siginfo_t,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn sigwait(set: *const sigset_t, sig: *mut c_int) -> c_int;
    pub fn pthread_atfork(
        prepare: Option<unsafe extern "C" fn()>,
        parent: Option<unsafe extern "C" fn()>,
        child: Option<unsafe extern "C" fn()>,
    ) -> c_int;
    pub fn getgrgid(gid: crate::gid_t) -> *mut crate::group;
    pub fn getgrouplist(
        user: *const c_char,
        group: crate::gid_t,
        groups: *mut crate::gid_t,
        ngroups: *mut c_int,
    ) -> c_int;
    pub fn initgroups(user: *const c_char, group: crate::gid_t) -> c_int;
    pub fn pthread_mutexattr_getpshared(
        attr: *const pthread_mutexattr_t,
        pshared: *mut c_int,
    ) -> c_int;
    pub fn popen(command: *const c_char, mode: *const c_char) -> *mut crate::FILE;
    pub fn faccessat(dirfd: c_int, pathname: *const c_char, mode: c_int, flags: c_int) -> c_int;
    pub fn pthread_create(
        native: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        f: extern "C" fn(*mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;
    pub fn __errno() -> *mut c_int;
    pub fn inotify_rm_watch(fd: c_int, wd: u32) -> c_int;
    pub fn inotify_init() -> c_int;
    pub fn inotify_init1(flags: c_int) -> c_int;
    pub fn inotify_add_watch(fd: c_int, path: *const c_char, mask: u32) -> c_int;

    pub fn regcomp(preg: *mut crate::regex_t, pattern: *const c_char, cflags: c_int) -> c_int;

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

    pub fn android_set_abort_message(msg: *const c_char);

    pub fn gettid() -> crate::pid_t;

    pub fn getauxval(type_: c_ulong) -> c_ulong;

    /// Only available in API Version 28+
    pub fn getrandom(buf: *mut c_void, buflen: size_t, flags: c_uint) -> ssize_t;
    pub fn getentropy(buf: *mut c_void, buflen: size_t) -> c_int;

    pub fn pthread_setname_np(thread: crate::pthread_t, name: *const c_char) -> c_int;

    pub fn __system_property_set(__name: *const c_char, __value: *const c_char) -> c_int;
    pub fn __system_property_get(__name: *const c_char, __value: *mut c_char) -> c_int;
    pub fn __system_property_find(__name: *const c_char) -> *const prop_info;
    pub fn __system_property_find_nth(__n: c_uint) -> *const prop_info;
    pub fn __system_property_foreach(
        __callback: unsafe extern "C" fn(__pi: *const prop_info, __cookie: *mut c_void),
        __cookie: *mut c_void,
    ) -> c_int;

    // #include <link.h>
    /// Only available in API Version 21+
    pub fn dl_iterate_phdr(
        callback: Option<
            unsafe extern "C" fn(info: *mut dl_phdr_info, size: usize, data: *mut c_void) -> c_int,
        >,
        data: *mut c_void,
    ) -> c_int;

    pub fn arc4random() -> u32;
    pub fn arc4random_uniform(__upper_bound: u32) -> u32;
    pub fn arc4random_buf(__buf: *mut c_void, __n: size_t);

    pub fn reallocarray(ptr: *mut c_void, nmemb: size_t, size: size_t) -> *mut c_void;

    pub fn pthread_getcpuclockid(thread: crate::pthread_t, clk_id: *mut crate::clockid_t) -> c_int;

    pub fn dirname(path: *const c_char) -> *mut c_char;
    pub fn basename(path: *const c_char) -> *mut c_char;
    pub fn getopt_long(
        argc: c_int,
        argv: *const *mut c_char,
        optstring: *const c_char,
        longopts: *const option,
        longindex: *mut c_int,
    ) -> c_int;

    pub fn sync();
    pub fn syncfs(fd: c_int) -> c_int;

    pub fn memmem(
        haystack: *const c_void,
        haystacklen: size_t,
        needle: *const c_void,
        needlelen: size_t,
    ) -> *mut c_void;
    pub fn fread_unlocked(
        buf: *mut c_void,
        size: size_t,
        nobj: size_t,
        stream: *mut crate::FILE,
    ) -> size_t;
    pub fn fwrite_unlocked(
        buf: *const c_void,
        size: size_t,
        nobj: size_t,
        stream: *mut crate::FILE,
    ) -> size_t;
    pub fn fflush_unlocked(stream: *mut crate::FILE) -> c_int;
    pub fn fgets_unlocked(buf: *mut c_char, size: c_int, stream: *mut crate::FILE) -> *mut c_char;

    pub fn klogctl(syslog_type: c_int, bufp: *mut c_char, len: c_int) -> c_int;

    pub fn memfd_create(name: *const c_char, flags: c_uint) -> c_int;
    pub fn renameat2(
        olddirfd: c_int,
        oldpath: *const c_char,
        newdirfd: c_int,
        newpath: *const c_char,
        flags: c_uint,
    ) -> c_int;

    pub fn if_nameindex() -> *mut if_nameindex;
    pub fn if_freenameindex(ptr: *mut if_nameindex);
}

cfg_if! {
    if #[cfg(target_pointer_width = "32")] {
        mod b32;
        pub use self::b32::*;
    } else if #[cfg(target_pointer_width = "64")] {
        mod b64;
        pub use self::b64::*;
    } else {
        // Unknown target_pointer_width
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
        (*(self as *const siginfo_t as *const siginfo_sigfault)).si_addr
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
        (*(self as *const siginfo_t as *const siginfo_timer)).si_sigval
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
        &(*(self as *const siginfo_t as *const siginfo_f)).sifields
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
