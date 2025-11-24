use crate::prelude::*;

pub type caddr_t = *mut c_char;

pub type clockid_t = c_int;
pub type blkcnt_t = c_long;
pub type clock_t = c_long;
pub type daddr_t = c_long;
pub type dev_t = c_ulong;
pub type fsblkcnt_t = c_ulong;
pub type fsfilcnt_t = c_ulong;
pub type ino_t = c_ulong;
pub type key_t = c_int;
pub type major_t = c_uint;
pub type minor_t = c_uint;
pub type mode_t = c_uint;
pub type nlink_t = c_uint;
pub type rlim_t = c_ulong;
pub type speed_t = c_uint;
pub type tcflag_t = c_uint;
pub type time_t = c_long;
pub type timer_t = c_int;
pub type wchar_t = c_int;
pub type nfds_t = c_ulong;
pub type projid_t = c_int;
pub type zoneid_t = c_int;
pub type psetid_t = c_int;
pub type processorid_t = c_int;
pub type chipid_t = c_int;
pub type ctid_t = crate::id_t;

pub type suseconds_t = c_long;
pub type off_t = c_long;
pub type useconds_t = c_uint;
pub type socklen_t = c_uint;
pub type sa_family_t = u16;
pub type pthread_t = c_uint;
pub type pthread_key_t = c_uint;
pub type thread_t = c_uint;
pub type blksize_t = c_int;
pub type nl_item = c_int;
pub type mqd_t = *mut c_void;
pub type id_t = c_int;
pub type idtype_t = c_uint;
pub type shmatt_t = c_ulong;

pub type lgrp_id_t = crate::id_t;
pub type lgrp_mem_size_t = c_longlong;
pub type lgrp_cookie_t = crate::uintptr_t;
pub type lgrp_content_t = c_uint;
pub type lgrp_lat_between_t = c_uint;
pub type lgrp_mem_size_flag_t = c_uint;
pub type lgrp_view_t = c_uint;

pub type posix_spawnattr_t = *mut c_void;
pub type posix_spawn_file_actions_t = *mut c_void;

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum timezone {}
impl Copy for timezone {}
impl Clone for timezone {
    fn clone(&self) -> timezone {
        *self
    }
}

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum ucred_t {}
impl Copy for ucred_t {}
impl Clone for ucred_t {
    fn clone(&self) -> ucred_t {
        *self
    }
}

s! {
    pub struct in_addr {
        pub s_addr: crate::in_addr_t,
    }

    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct ip_mreq_source {
        pub imr_multiaddr: in_addr,
        pub imr_sourceaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct ipc_perm {
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub cuid: crate::uid_t,
        pub cgid: crate::gid_t,
        pub mode: mode_t,
        pub seq: c_uint,
        pub key: crate::key_t,
    }

    pub struct sockaddr {
        pub sa_family: sa_family_t,
        pub sa_data: [c_char; 14],
    }

    pub struct sockaddr_in {
        pub sin_family: sa_family_t,
        pub sin_port: crate::in_port_t,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [c_char; 8],
    }

    pub struct sockaddr_in6 {
        pub sin6_family: sa_family_t,
        pub sin6_port: crate::in_port_t,
        pub sin6_flowinfo: u32,
        pub sin6_addr: crate::in6_addr,
        pub sin6_scope_id: u32,
        pub __sin6_src_id: u32,
    }

    pub struct in_pktinfo {
        pub ipi_ifindex: c_uint,
        pub ipi_spec_dst: crate::in_addr,
        pub ipi_addr: crate::in_addr,
    }

    pub struct in6_pktinfo {
        pub ipi6_addr: crate::in6_addr,
        pub ipi6_ifindex: c_uint,
    }

    pub struct passwd {
        pub pw_name: *mut c_char,
        pub pw_passwd: *mut c_char,
        pub pw_uid: crate::uid_t,
        pub pw_gid: crate::gid_t,
        pub pw_age: *mut c_char,
        pub pw_comment: *mut c_char,
        pub pw_gecos: *mut c_char,
        pub pw_dir: *mut c_char,
        pub pw_shell: *mut c_char,
    }

    pub struct ifaddrs {
        pub ifa_next: *mut ifaddrs,
        pub ifa_name: *mut c_char,
        pub ifa_flags: u64,
        pub ifa_addr: *mut crate::sockaddr,
        pub ifa_netmask: *mut crate::sockaddr,
        pub ifa_dstaddr: *mut crate::sockaddr,
        pub ifa_data: *mut c_void,
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

    pub struct pthread_attr_t {
        __pthread_attrp: *mut c_void,
    }

    pub struct pthread_mutex_t {
        __pthread_mutex_flag1: u16,
        __pthread_mutex_flag2: u8,
        __pthread_mutex_ceiling: u8,
        __pthread_mutex_type: u16,
        __pthread_mutex_magic: u16,
        __pthread_mutex_lock: u64,
        __pthread_mutex_data: u64,
    }

    pub struct pthread_mutexattr_t {
        __pthread_mutexattrp: *mut c_void,
    }

    pub struct pthread_cond_t {
        __pthread_cond_flag: [u8; 4],
        __pthread_cond_type: u16,
        __pthread_cond_magic: u16,
        __pthread_cond_data: u64,
    }

    pub struct pthread_condattr_t {
        __pthread_condattrp: *mut c_void,
    }

    pub struct pthread_rwlock_t {
        __pthread_rwlock_readers: i32,
        __pthread_rwlock_type: u16,
        __pthread_rwlock_magic: u16,
        __pthread_rwlock_mutex: crate::pthread_mutex_t,
        __pthread_rwlock_readercv: crate::pthread_cond_t,
        __pthread_rwlock_writercv: crate::pthread_cond_t,
    }

    pub struct pthread_rwlockattr_t {
        __pthread_rwlockattrp: *mut c_void,
    }

    pub struct dirent {
        pub d_ino: crate::ino_t,
        pub d_off: off_t,
        pub d_reclen: u16,
        pub d_name: [c_char; 3],
    }

    pub struct glob_t {
        pub gl_pathc: size_t,
        pub gl_pathv: *mut *mut c_char,
        pub gl_offs: size_t,
        __unused1: *mut c_void,
        __unused2: c_int,
        #[cfg(target_os = "illumos")]
        __unused3: c_int,
        #[cfg(target_os = "illumos")]
        __unused4: c_int,
        #[cfg(target_os = "illumos")]
        __unused5: *mut c_void,
        #[cfg(target_os = "illumos")]
        __unused6: *mut c_void,
        #[cfg(target_os = "illumos")]
        __unused7: *mut c_void,
        #[cfg(target_os = "illumos")]
        __unused8: *mut c_void,
        #[cfg(target_os = "illumos")]
        __unused9: *mut c_void,
        #[cfg(target_os = "illumos")]
        __unused10: *mut c_void,
    }

    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        #[cfg(target_arch = "sparc64")]
        __sparcv9_pad: c_int,
        pub ai_addrlen: crate::socklen_t,
        pub ai_canonname: *mut c_char,
        pub ai_addr: *mut crate::sockaddr,
        pub ai_next: *mut addrinfo,
    }

    pub struct sigset_t {
        bits: [u32; 4],
    }

    pub struct sigaction {
        pub sa_flags: c_int,
        pub sa_sigaction: crate::sighandler_t,
        pub sa_mask: sigset_t,
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_size: size_t,
        pub ss_flags: c_int,
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
    }

    pub struct sendfilevec_t {
        pub sfv_fd: c_int,
        pub sfv_flag: c_uint,
        pub sfv_off: off_t,
        pub sfv_len: size_t,
    }

    pub struct sched_param {
        pub sched_priority: c_int,
        sched_pad: [c_int; 8],
    }

    pub struct Dl_info {
        pub dli_fname: *const c_char,
        pub dli_fbase: *mut c_void,
        pub dli_sname: *const c_char,
        pub dli_saddr: *mut c_void,
    }

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
        pub st_atime_nsec: c_long,
        pub st_mtime: crate::time_t,
        pub st_mtime_nsec: c_long,
        pub st_ctime: crate::time_t,
        pub st_ctime_nsec: c_long,
        pub st_blksize: crate::blksize_t,
        pub st_blocks: crate::blkcnt_t,
        pub st_fstype: [c_char; _ST_FSTYPSZ as usize],
    }

    pub struct termios {
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_cc: [crate::cc_t; crate::NCCS],
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

    pub struct sem_t {
        pub sem_count: u32,
        pub sem_type: u16,
        pub sem_magic: u16,
        pub sem_pad1: [u64; 3],
        pub sem_pad2: [u64; 2],
    }

    pub struct flock {
        pub l_type: c_short,
        pub l_whence: c_short,
        pub l_start: off_t,
        pub l_len: off_t,
        pub l_sysid: c_int,
        pub l_pid: crate::pid_t,
        pub l_pad: [c_long; 4],
    }

    pub struct if_nameindex {
        pub if_index: c_uint,
        pub if_name: *mut c_char,
    }

    pub struct mq_attr {
        pub mq_flags: c_long,
        pub mq_maxmsg: c_long,
        pub mq_msgsize: c_long,
        pub mq_curmsgs: c_long,
        _pad: [c_int; 12],
    }

    pub struct port_event {
        pub portev_events: c_int,
        pub portev_source: c_ushort,
        pub portev_pad: c_ushort,
        pub portev_object: crate::uintptr_t,
        pub portev_user: *mut c_void,
    }

    pub struct port_notify {
        pub portnfy_port: c_int,
        pub portnfy_user: *mut c_void,
    }

    pub struct aio_result_t {
        pub aio_return: ssize_t,
        pub aio_errno: c_int,
    }

    pub struct exit_status {
        e_termination: c_short,
        e_exit: c_short,
    }

    pub struct utmp {
        pub ut_user: [c_char; 8],
        pub ut_id: [c_char; 4],
        pub ut_line: [c_char; 12],
        pub ut_pid: c_short,
        pub ut_type: c_short,
        pub ut_exit: exit_status,
        pub ut_time: crate::time_t,
    }

    pub struct timex {
        pub modes: u32,
        pub offset: i32,
        pub freq: i32,
        pub maxerror: i32,
        pub esterror: i32,
        pub status: i32,
        pub constant: i32,
        pub precision: i32,
        pub tolerance: i32,
        pub ppsfreq: i32,
        pub jitter: i32,
        pub shift: i32,
        pub stabil: i32,
        pub jitcnt: i32,
        pub calcnt: i32,
        pub errcnt: i32,
        pub stbcnt: i32,
    }

    pub struct ntptimeval {
        pub time: crate::timeval,
        pub maxerror: i32,
        pub esterror: i32,
    }

    pub struct mmapobj_result_t {
        pub mr_addr: crate::caddr_t,
        pub mr_msize: size_t,
        pub mr_fsize: size_t,
        pub mr_offset: size_t,
        pub mr_prot: c_uint,
        pub mr_flags: c_uint,
    }

    pub struct lgrp_affinity_args_t {
        pub idtype: crate::idtype_t,
        pub id: crate::id_t,
        pub lgrp: crate::lgrp_id_t,
        pub aff: crate::lgrp_affinity_t,
    }

    pub struct processor_info_t {
        pub pi_state: c_int,
        pub pi_processor_type: [c_char; PI_TYPELEN as usize],
        pub pi_fputypes: [c_char; PI_FPUTYPE as usize],
        pub pi_clock: c_int,
    }

    pub struct option {
        pub name: *const c_char,
        pub has_arg: c_int,
        pub flag: *mut c_int,
        pub val: c_int,
    }
}

s_no_extra_traits! {
    pub struct sockaddr_un {
        pub sun_family: sa_family_t,
        pub sun_path: [c_char; 108],
    }

    pub struct utsname {
        pub sysname: [c_char; 257],
        pub nodename: [c_char; 257],
        pub release: [c_char; 257],
        pub version: [c_char; 257],
        pub machine: [c_char; 257],
    }

    pub struct fd_set {
        #[cfg(target_pointer_width = "64")]
        fds_bits: [i64; FD_SETSIZE as usize / 64],
        #[cfg(target_pointer_width = "32")]
        fds_bits: [i32; FD_SETSIZE as usize / 32],
    }

    pub struct sockaddr_storage {
        pub ss_family: crate::sa_family_t,
        __ss_pad1: [u8; 6],
        __ss_align: i64,
        __ss_pad2: [u8; 240],
    }

    #[cfg_attr(target_pointer_width = "64", repr(align(8)))]
    pub struct siginfo_t {
        pub si_signo: c_int,
        pub si_code: c_int,
        pub si_errno: c_int,
        #[cfg(target_pointer_width = "64")]
        pub si_pad: c_int,

        __data_pad: [c_int; SIGINFO_DATA_SIZE],
    }

    pub struct sockaddr_dl {
        pub sdl_family: c_ushort,
        pub sdl_index: c_ushort,
        pub sdl_type: c_uchar,
        pub sdl_nlen: c_uchar,
        pub sdl_alen: c_uchar,
        pub sdl_slen: c_uchar,
        pub sdl_data: [c_char; 244],
    }

    pub struct sigevent {
        pub sigev_notify: c_int,
        pub sigev_signo: c_int,
        pub sigev_value: crate::sigval,
        pub ss_sp: *mut c_void,
        pub sigev_notify_attributes: *const crate::pthread_attr_t,
        __sigev_pad2: c_int,
    }

    #[repr(align(16))]
    pub union pad128_t {
        // pub _q in this structure would be a "long double", of 16 bytes
        pub _l: [i32; 4],
    }

    #[repr(align(16))]
    pub union upad128_t {
        // pub _q in this structure would be a "long double", of 16 bytes
        pub _l: [u32; 4],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
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

        impl PartialEq for fd_set {
            fn eq(&self, other: &fd_set) -> bool {
                self.fds_bits
                    .iter()
                    .zip(other.fds_bits.iter())
                    .all(|(a, b)| a == b)
            }
        }
        impl Eq for fd_set {}
        impl hash::Hash for fd_set {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.fds_bits.hash(state);
            }
        }

        impl PartialEq for sockaddr_storage {
            fn eq(&self, other: &sockaddr_storage) -> bool {
                self.ss_family == other.ss_family
                    && self.__ss_pad1 == other.__ss_pad1
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
                self.__ss_pad1.hash(state);
                self.__ss_align.hash(state);
                self.__ss_pad2.hash(state);
            }
        }

        impl siginfo_t {
            /// The siginfo_t will have differing contents based on the delivered signal.  Based on
            /// `si_signo`, this determines how many of the `c_int` pad fields contain valid data
            /// exposed by the C unions.
            ///
            /// It is not yet exhausitive for the OS-defined types, and defaults to assuming the
            /// entire data pad area is "valid" for otherwise unrecognized signal numbers.
            fn data_field_count(&self) -> usize {
                match self.si_signo {
                    SIGSEGV | SIGBUS | SIGILL | SIGTRAP | SIGFPE => {
                        size_of::<siginfo_fault>() / size_of::<c_int>()
                    }
                    SIGCLD => size_of::<siginfo_sigcld>() / size_of::<c_int>(),
                    SIGHUP
                    | SIGINT
                    | SIGQUIT
                    | SIGABRT
                    | SIGSYS
                    | SIGPIPE
                    | SIGALRM
                    | SIGTERM
                    | crate::SIGUSR1
                    | crate::SIGUSR2
                    | SIGPWR
                    | SIGWINCH
                    | SIGURG => size_of::<siginfo_kill>() / size_of::<c_int>(),
                    _ => SIGINFO_DATA_SIZE,
                }
            }
        }
        impl PartialEq for siginfo_t {
            fn eq(&self, other: &siginfo_t) -> bool {
                if self.si_signo == other.si_signo
                    && self.si_code == other.si_code
                    && self.si_errno == other.si_errno
                {
                    // FIXME(solarish): The `si_pad` field in the 64-bit version of the struct is ignored
                    // (for now) when doing comparisons.

                    let field_count = self.data_field_count();
                    self.__data_pad[..field_count]
                        .iter()
                        .zip(other.__data_pad[..field_count].iter())
                        .all(|(a, b)| a == b)
                } else {
                    false
                }
            }
        }
        impl Eq for siginfo_t {}
        impl hash::Hash for siginfo_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.si_signo.hash(state);
                self.si_code.hash(state);
                self.si_errno.hash(state);

                // FIXME(solarish): The `si_pad` field in the 64-bit version of the struct is ignored
                // (for now) when doing hashing.

                let field_count = self.data_field_count();
                self.__data_pad[..field_count].hash(state)
            }
        }

        impl PartialEq for sockaddr_dl {
            fn eq(&self, other: &sockaddr_dl) -> bool {
                self.sdl_family == other.sdl_family
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
                self.sdl_family.hash(state);
                self.sdl_index.hash(state);
                self.sdl_type.hash(state);
                self.sdl_nlen.hash(state);
                self.sdl_alen.hash(state);
                self.sdl_slen.hash(state);
                self.sdl_data.hash(state);
            }
        }

        impl PartialEq for sigevent {
            fn eq(&self, other: &sigevent) -> bool {
                self.sigev_notify == other.sigev_notify
                    && self.sigev_signo == other.sigev_signo
                    && self.sigev_value == other.sigev_value
                    && self.ss_sp == other.ss_sp
                    && self.sigev_notify_attributes == other.sigev_notify_attributes
            }
        }
        impl Eq for sigevent {}
        impl hash::Hash for sigevent {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sigev_notify.hash(state);
                self.sigev_signo.hash(state);
                self.sigev_value.hash(state);
                self.ss_sp.hash(state);
                self.sigev_notify_attributes.hash(state);
            }
        }

        impl PartialEq for pad128_t {
            fn eq(&self, other: &pad128_t) -> bool {
                unsafe {
                    // FIXME(solarish): self._q == other._q ||
                    self._l == other._l
                }
            }
        }
        impl Eq for pad128_t {}
        impl hash::Hash for pad128_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    // FIXME(solarish): state.write_i64(self._q as i64);
                    self._l.hash(state);
                }
            }
        }
        impl PartialEq for upad128_t {
            fn eq(&self, other: &upad128_t) -> bool {
                unsafe {
                    // FIXME(solarish): self._q == other._q ||
                    self._l == other._l
                }
            }
        }
        impl Eq for upad128_t {}
        impl hash::Hash for upad128_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    // FIXME(solarish): state.write_i64(self._q as i64);
                    self._l.hash(state);
                }
            }
        }
    }
}

cfg_if! {
    if #[cfg(target_pointer_width = "64")] {
        const SIGINFO_DATA_SIZE: usize = 60;
    } else {
        const SIGINFO_DATA_SIZE: usize = 29;
    }
}

#[repr(C)]
struct siginfo_fault {
    addr: *mut c_void,
    trapno: c_int,
    pc: *mut crate::caddr_t,
}
impl Copy for siginfo_fault {}
impl Clone for siginfo_fault {
    fn clone(&self) -> Self {
        *self
    }
}

#[repr(C)]
struct siginfo_cldval {
    utime: crate::clock_t,
    status: c_int,
    stime: crate::clock_t,
}
impl Copy for siginfo_cldval {}
impl Clone for siginfo_cldval {
    fn clone(&self) -> Self {
        *self
    }
}

#[repr(C)]
struct siginfo_killval {
    uid: crate::uid_t,
    value: crate::sigval,
    // Pad out to match the SIGCLD value size
    _pad: *mut c_void,
}
impl Copy for siginfo_killval {}
impl Clone for siginfo_killval {
    fn clone(&self) -> Self {
        *self
    }
}

#[repr(C)]
struct siginfo_sigcld {
    pid: crate::pid_t,
    val: siginfo_cldval,
    ctid: crate::ctid_t,
    zoneid: crate::zoneid_t,
}
impl Copy for siginfo_sigcld {}
impl Clone for siginfo_sigcld {
    fn clone(&self) -> Self {
        *self
    }
}

#[repr(C)]
struct siginfo_kill {
    pid: crate::pid_t,
    val: siginfo_killval,
    ctid: crate::ctid_t,
    zoneid: crate::zoneid_t,
}
impl Copy for siginfo_kill {}
impl Clone for siginfo_kill {
    fn clone(&self) -> Self {
        *self
    }
}

impl siginfo_t {
    unsafe fn sidata<T: Copy>(&self) -> T {
        *((&self.__data_pad) as *const c_int as *const T)
    }
    pub unsafe fn si_addr(&self) -> *mut c_void {
        let sifault: siginfo_fault = self.sidata();
        sifault.addr
    }
    pub unsafe fn si_uid(&self) -> crate::uid_t {
        let kill: siginfo_kill = self.sidata();
        kill.val.uid
    }
    pub unsafe fn si_value(&self) -> crate::sigval {
        let kill: siginfo_kill = self.sidata();
        kill.val.value
    }
    pub unsafe fn si_pid(&self) -> crate::pid_t {
        let sigcld: siginfo_sigcld = self.sidata();
        sigcld.pid
    }
    pub unsafe fn si_status(&self) -> c_int {
        let sigcld: siginfo_sigcld = self.sidata();
        sigcld.val.status
    }
    pub unsafe fn si_utime(&self) -> c_long {
        let sigcld: siginfo_sigcld = self.sidata();
        sigcld.val.utime
    }
    pub unsafe fn si_stime(&self) -> c_long {
        let sigcld: siginfo_sigcld = self.sidata();
        sigcld.val.stime
    }
}

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
pub const LC_ALL_MASK: c_int = LC_CTYPE_MASK
    | LC_NUMERIC_MASK
    | LC_TIME_MASK
    | LC_COLLATE_MASK
    | LC_MONETARY_MASK
    | LC_MESSAGES_MASK;

pub const DAY_1: crate::nl_item = 1;
pub const DAY_2: crate::nl_item = 2;
pub const DAY_3: crate::nl_item = 3;
pub const DAY_4: crate::nl_item = 4;
pub const DAY_5: crate::nl_item = 5;
pub const DAY_6: crate::nl_item = 6;
pub const DAY_7: crate::nl_item = 7;

pub const ABDAY_1: crate::nl_item = 8;
pub const ABDAY_2: crate::nl_item = 9;
pub const ABDAY_3: crate::nl_item = 10;
pub const ABDAY_4: crate::nl_item = 11;
pub const ABDAY_5: crate::nl_item = 12;
pub const ABDAY_6: crate::nl_item = 13;
pub const ABDAY_7: crate::nl_item = 14;

pub const MON_1: crate::nl_item = 15;
pub const MON_2: crate::nl_item = 16;
pub const MON_3: crate::nl_item = 17;
pub const MON_4: crate::nl_item = 18;
pub const MON_5: crate::nl_item = 19;
pub const MON_6: crate::nl_item = 20;
pub const MON_7: crate::nl_item = 21;
pub const MON_8: crate::nl_item = 22;
pub const MON_9: crate::nl_item = 23;
pub const MON_10: crate::nl_item = 24;
pub const MON_11: crate::nl_item = 25;
pub const MON_12: crate::nl_item = 26;

pub const ABMON_1: crate::nl_item = 27;
pub const ABMON_2: crate::nl_item = 28;
pub const ABMON_3: crate::nl_item = 29;
pub const ABMON_4: crate::nl_item = 30;
pub const ABMON_5: crate::nl_item = 31;
pub const ABMON_6: crate::nl_item = 32;
pub const ABMON_7: crate::nl_item = 33;
pub const ABMON_8: crate::nl_item = 34;
pub const ABMON_9: crate::nl_item = 35;
pub const ABMON_10: crate::nl_item = 36;
pub const ABMON_11: crate::nl_item = 37;
pub const ABMON_12: crate::nl_item = 38;

pub const RADIXCHAR: crate::nl_item = 39;
pub const THOUSEP: crate::nl_item = 40;
pub const YESSTR: crate::nl_item = 41;
pub const NOSTR: crate::nl_item = 42;
pub const CRNCYSTR: crate::nl_item = 43;

pub const D_T_FMT: crate::nl_item = 44;
pub const D_FMT: crate::nl_item = 45;
pub const T_FMT: crate::nl_item = 46;
pub const AM_STR: crate::nl_item = 47;
pub const PM_STR: crate::nl_item = 48;

pub const CODESET: crate::nl_item = 49;
pub const T_FMT_AMPM: crate::nl_item = 50;
pub const ERA: crate::nl_item = 51;
pub const ERA_D_FMT: crate::nl_item = 52;
pub const ERA_D_T_FMT: crate::nl_item = 53;
pub const ERA_T_FMT: crate::nl_item = 54;
pub const ALT_DIGITS: crate::nl_item = 55;
pub const YESEXPR: crate::nl_item = 56;
pub const NOEXPR: crate::nl_item = 57;
pub const _DATE_FMT: crate::nl_item = 58;
pub const MAXSTRMSG: crate::nl_item = 58;

pub const PATH_MAX: c_int = 1024;

pub const SA_ONSTACK: c_int = 0x00000001;
pub const SA_RESETHAND: c_int = 0x00000002;
pub const SA_RESTART: c_int = 0x00000004;
pub const SA_SIGINFO: c_int = 0x00000008;
pub const SA_NODEFER: c_int = 0x00000010;
pub const SA_NOCLDWAIT: c_int = 0x00010000;
pub const SA_NOCLDSTOP: c_int = 0x00020000;

pub const SS_ONSTACK: c_int = 1;
pub const SS_DISABLE: c_int = 2;

pub const FIOCLEX: c_int = 0x20006601;
pub const FIONCLEX: c_int = 0x20006602;
pub const FIONREAD: c_int = 0x4004667f;
pub const FIONBIO: c_int = 0x8004667e;
pub const FIOASYNC: c_int = 0x8004667d;
pub const FIOSETOWN: c_int = 0x8004667c;
pub const FIOGETOWN: c_int = 0x4004667b;

pub const SIGCHLD: c_int = 18;
pub const SIGCLD: c_int = SIGCHLD;
pub const SIGBUS: c_int = 10;
pub const SIG_BLOCK: c_int = 1;
pub const SIG_UNBLOCK: c_int = 2;
pub const SIG_SETMASK: c_int = 3;

pub const AIO_CANCELED: c_int = 0;
pub const AIO_ALLDONE: c_int = 1;
pub const AIO_NOTCANCELED: c_int = 2;
pub const LIO_NOP: c_int = 0;
pub const LIO_READ: c_int = 1;
pub const LIO_WRITE: c_int = 2;
pub const LIO_NOWAIT: c_int = 0;
pub const LIO_WAIT: c_int = 1;

pub const SIGEV_NONE: c_int = 1;
pub const SIGEV_SIGNAL: c_int = 2;
pub const SIGEV_THREAD: c_int = 3;
pub const SIGEV_PORT: c_int = 4;

pub const CLD_EXITED: c_int = 1;
pub const CLD_KILLED: c_int = 2;
pub const CLD_DUMPED: c_int = 3;
pub const CLD_TRAPPED: c_int = 4;
pub const CLD_STOPPED: c_int = 5;
pub const CLD_CONTINUED: c_int = 6;

pub const IP_RECVDSTADDR: c_int = 0x7;
pub const IP_PKTINFO: c_int = 0x1a;
pub const IP_DONTFRAG: c_int = 0x1b;
pub const IP_SEC_OPT: c_int = 0x22;

pub const IPV6_UNICAST_HOPS: c_int = 0x5;
pub const IPV6_MULTICAST_IF: c_int = 0x6;
pub const IPV6_MULTICAST_HOPS: c_int = 0x7;
pub const IPV6_MULTICAST_LOOP: c_int = 0x8;
pub const IPV6_PKTINFO: c_int = 0xb;
pub const IPV6_RECVPKTINFO: c_int = 0x12;
pub const IPV6_RECVTCLASS: c_int = 0x19;
pub const IPV6_DONTFRAG: c_int = 0x21;
pub const IPV6_SEC_OPT: c_int = 0x22;
pub const IPV6_TCLASS: c_int = 0x26;
pub const IPV6_V6ONLY: c_int = 0x27;
pub const IPV6_BOUND_IF: c_int = 0x41;

cfg_if! {
    if #[cfg(target_pointer_width = "64")] {
        pub const FD_SETSIZE: usize = 65536;
    } else {
        pub const FD_SETSIZE: usize = 1024;
    }
}

pub const ST_RDONLY: c_ulong = 1;
pub const ST_NOSUID: c_ulong = 2;

pub const NI_MAXHOST: crate::socklen_t = 1025;
pub const NI_MAXSERV: crate::socklen_t = 32;

pub const EXIT_FAILURE: c_int = 1;
pub const EXIT_SUCCESS: c_int = 0;
pub const RAND_MAX: c_int = 32767;
pub const EOF: c_int = -1;
pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;
pub const SEEK_DATA: c_int = 3;
pub const SEEK_HOLE: c_int = 4;
pub const _IOFBF: c_int = 0;
pub const _IONBF: c_int = 4;
pub const _IOLBF: c_int = 64;
pub const BUFSIZ: c_uint = 1024;
pub const FOPEN_MAX: c_uint = 20;
pub const FILENAME_MAX: c_uint = 1024;
pub const L_tmpnam: c_uint = 25;
pub const TMP_MAX: c_uint = 17576;
pub const PIPE_BUF: c_int = 5120;

pub const GRND_NONBLOCK: c_uint = 0x0001;
pub const GRND_RANDOM: c_uint = 0x0002;

pub const O_RDONLY: c_int = 0;
pub const O_WRONLY: c_int = 1;
pub const O_RDWR: c_int = 2;
pub const O_NDELAY: c_int = 0x04;
pub const O_APPEND: c_int = 8;
pub const O_DSYNC: c_int = 0x40;
pub const O_RSYNC: c_int = 0x8000;
pub const O_CREAT: c_int = 256;
pub const O_EXCL: c_int = 1024;
pub const O_NOCTTY: c_int = 2048;
pub const O_TRUNC: c_int = 512;
pub const O_NOFOLLOW: c_int = 0x20000;
pub const O_SEARCH: c_int = 0x200000;
pub const O_EXEC: c_int = 0x400000;
pub const O_CLOEXEC: c_int = 0x800000;
pub const O_ACCMODE: c_int = 0x600003;
pub const O_XATTR: c_int = 0x4000;
pub const O_DIRECTORY: c_int = 0x1000000;
pub const S_IFIFO: mode_t = 0o1_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_IFMT: mode_t = 0o17_0000;
pub const S_IEXEC: mode_t = 0o0100;
pub const S_IWRITE: mode_t = 0o0200;
pub const S_IREAD: mode_t = 0o0400;
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
pub const F_LOCK: c_int = 1;
pub const F_TEST: c_int = 3;
pub const F_TLOCK: c_int = 2;
pub const F_ULOCK: c_int = 0;
pub const F_SETLK: c_int = 6;
pub const F_SETLKW: c_int = 7;
pub const F_GETLK: c_int = 14;
pub const F_ALLOCSP: c_int = 10;
pub const F_FREESP: c_int = 11;
pub const F_BLOCKS: c_int = 18;
pub const F_BLKSIZE: c_int = 19;
pub const F_SHARE: c_int = 40;
pub const F_UNSHARE: c_int = 41;
pub const F_ISSTREAM: c_int = 13;
pub const F_PRIV: c_int = 15;
pub const F_NPRIV: c_int = 16;
pub const F_QUOTACTL: c_int = 17;
pub const F_GETOWN: c_int = 23;
pub const F_SETOWN: c_int = 24;
pub const F_REVOKE: c_int = 25;
pub const F_HASREMOTELOCKS: c_int = 26;
pub const SIGHUP: c_int = 1;
pub const SIGINT: c_int = 2;
pub const SIGQUIT: c_int = 3;
pub const SIGILL: c_int = 4;
pub const SIGABRT: c_int = 6;
pub const SIGEMT: c_int = 7;
pub const SIGFPE: c_int = 8;
pub const SIGKILL: c_int = 9;
pub const SIGSEGV: c_int = 11;
pub const SIGSYS: c_int = 12;
pub const SIGPIPE: c_int = 13;
pub const SIGALRM: c_int = 14;
pub const SIGTERM: c_int = 15;
pub const SIGUSR1: c_int = 16;
pub const SIGUSR2: c_int = 17;
pub const SIGPWR: c_int = 19;
pub const SIGWINCH: c_int = 20;
pub const SIGURG: c_int = 21;
pub const SIGPOLL: c_int = 22;
pub const SIGIO: c_int = SIGPOLL;
pub const SIGSTOP: c_int = 23;
pub const SIGTSTP: c_int = 24;
pub const SIGCONT: c_int = 25;
pub const SIGTTIN: c_int = 26;
pub const SIGTTOU: c_int = 27;
pub const SIGVTALRM: c_int = 28;
pub const SIGPROF: c_int = 29;
pub const SIGXCPU: c_int = 30;
pub const SIGXFSZ: c_int = 31;

pub const WNOHANG: c_int = 0x40;
pub const WUNTRACED: c_int = 0x04;

pub const WEXITED: c_int = 0x01;
pub const WTRAPPED: c_int = 0x02;
pub const WSTOPPED: c_int = WUNTRACED;
pub const WCONTINUED: c_int = 0x08;
pub const WNOWAIT: c_int = 0x80;

pub const AT_FDCWD: c_int = 0xffd19553;
pub const AT_SYMLINK_NOFOLLOW: c_int = 0x1000;
pub const AT_SYMLINK_FOLLOW: c_int = 0x2000;
pub const AT_REMOVEDIR: c_int = 0x1;
pub const _AT_TRIGGER: c_int = 0x2;
pub const AT_EACCESS: c_int = 0x4;

pub const P_PID: idtype_t = 0;
pub const P_PPID: idtype_t = 1;
pub const P_PGID: idtype_t = 2;
pub const P_SID: idtype_t = 3;
pub const P_CID: idtype_t = 4;
pub const P_UID: idtype_t = 5;
pub const P_GID: idtype_t = 6;
pub const P_ALL: idtype_t = 7;
pub const P_LWPID: idtype_t = 8;
pub const P_TASKID: idtype_t = 9;
pub const P_PROJID: idtype_t = 10;
pub const P_POOLID: idtype_t = 11;
pub const P_ZONEID: idtype_t = 12;
pub const P_CTID: idtype_t = 13;
pub const P_CPUID: idtype_t = 14;
pub const P_PSETID: idtype_t = 15;

pub const PBIND_NONE: crate::processorid_t = -1;
pub const PBIND_QUERY: crate::processorid_t = -2;

pub const PS_NONE: c_int = -1;
pub const PS_QUERY: c_int = -2;
pub const PS_MYID: c_int = -3;
pub const PS_SOFT: c_int = -4;
pub const PS_HARD: c_int = -5;
pub const PS_QUERY_TYPE: c_int = -6;
pub const PS_PRIVATE: c_int = 2;

pub const UTIME_OMIT: c_long = -2;
pub const UTIME_NOW: c_long = -1;

pub const PROT_NONE: c_int = 0;
pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const PROT_EXEC: c_int = 4;

pub const MAP_SHARED: c_int = 0x0001;
pub const MAP_PRIVATE: c_int = 0x0002;
pub const MAP_FIXED: c_int = 0x0010;
pub const MAP_NORESERVE: c_int = 0x40;
pub const MAP_ANON: c_int = 0x0100;
pub const MAP_ANONYMOUS: c_int = 0x0100;
pub const MAP_RENAME: c_int = 0x20;
pub const MAP_ALIGN: c_int = 0x200;
pub const MAP_TEXT: c_int = 0x400;
pub const MAP_INITDATA: c_int = 0x800;
pub const MAP_FAILED: *mut c_void = !0 as *mut c_void;

pub const MCL_CURRENT: c_int = 0x0001;
pub const MCL_FUTURE: c_int = 0x0002;

pub const MS_SYNC: c_int = 0x0004;
pub const MS_ASYNC: c_int = 0x0001;
pub const MS_INVALIDATE: c_int = 0x0002;

pub const MMOBJ_PADDING: c_uint = 0x10000;
pub const MMOBJ_INTERPRET: c_uint = 0x20000;
pub const MR_PADDING: c_uint = 0x1;
pub const MR_HDR_ELF: c_uint = 0x2;

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
pub const ENOMSG: c_int = 35;
pub const EIDRM: c_int = 36;
pub const ECHRNG: c_int = 37;
pub const EL2NSYNC: c_int = 38;
pub const EL3HLT: c_int = 39;
pub const EL3RST: c_int = 40;
pub const ELNRNG: c_int = 41;
pub const EUNATCH: c_int = 42;
pub const ENOCSI: c_int = 43;
pub const EL2HLT: c_int = 44;
pub const EDEADLK: c_int = 45;
pub const ENOLCK: c_int = 46;
pub const ECANCELED: c_int = 47;
pub const ENOTSUP: c_int = 48;
pub const EDQUOT: c_int = 49;
pub const EBADE: c_int = 50;
pub const EBADR: c_int = 51;
pub const EXFULL: c_int = 52;
pub const ENOANO: c_int = 53;
pub const EBADRQC: c_int = 54;
pub const EBADSLT: c_int = 55;
pub const EDEADLOCK: c_int = 56;
pub const EBFONT: c_int = 57;
pub const EOWNERDEAD: c_int = 58;
pub const ENOTRECOVERABLE: c_int = 59;
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
pub const ELOCKUNMAPPED: c_int = 72;
pub const ENOTACTIVE: c_int = 73;
pub const EMULTIHOP: c_int = 74;
pub const EADI: c_int = 75;
pub const EBADMSG: c_int = 77;
pub const ENAMETOOLONG: c_int = 78;
pub const EOVERFLOW: c_int = 79;
pub const ENOTUNIQ: c_int = 80;
pub const EBADFD: c_int = 81;
pub const EREMCHG: c_int = 82;
pub const ELIBACC: c_int = 83;
pub const ELIBBAD: c_int = 84;
pub const ELIBSCN: c_int = 85;
pub const ELIBMAX: c_int = 86;
pub const ELIBEXEC: c_int = 87;
pub const EILSEQ: c_int = 88;
pub const ENOSYS: c_int = 89;
pub const ELOOP: c_int = 90;
pub const ERESTART: c_int = 91;
pub const ESTRPIPE: c_int = 92;
pub const ENOTEMPTY: c_int = 93;
pub const EUSERS: c_int = 94;
pub const ENOTSOCK: c_int = 95;
pub const EDESTADDRREQ: c_int = 96;
pub const EMSGSIZE: c_int = 97;
pub const EPROTOTYPE: c_int = 98;
pub const ENOPROTOOPT: c_int = 99;
pub const EPROTONOSUPPORT: c_int = 120;
pub const ESOCKTNOSUPPORT: c_int = 121;
pub const EOPNOTSUPP: c_int = 122;
pub const EPFNOSUPPORT: c_int = 123;
pub const EAFNOSUPPORT: c_int = 124;
pub const EADDRINUSE: c_int = 125;
pub const EADDRNOTAVAIL: c_int = 126;
pub const ENETDOWN: c_int = 127;
pub const ENETUNREACH: c_int = 128;
pub const ENETRESET: c_int = 129;
pub const ECONNABORTED: c_int = 130;
pub const ECONNRESET: c_int = 131;
pub const ENOBUFS: c_int = 132;
pub const EISCONN: c_int = 133;
pub const ENOTCONN: c_int = 134;
pub const ESHUTDOWN: c_int = 143;
pub const ETOOMANYREFS: c_int = 144;
pub const ETIMEDOUT: c_int = 145;
pub const ECONNREFUSED: c_int = 146;
pub const EHOSTDOWN: c_int = 147;
pub const EHOSTUNREACH: c_int = 148;
pub const EWOULDBLOCK: c_int = EAGAIN;
pub const EALREADY: c_int = 149;
pub const EINPROGRESS: c_int = 150;
pub const ESTALE: c_int = 151;

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
pub const EAI_OVERFLOW: c_int = 12;

pub const NI_NOFQDN: c_uint = 0x0001;
pub const NI_NUMERICHOST: c_uint = 0x0002;
pub const NI_NAMEREQD: c_uint = 0x0004;
pub const NI_NUMERICSERV: c_uint = 0x0008;
pub const NI_DGRAM: c_uint = 0x0010;
pub const NI_WITHSCOPEID: c_uint = 0x0020;
pub const NI_NUMERICSCOPE: c_uint = 0x0040;

pub const F_DUPFD: c_int = 0;
pub const F_DUP2FD: c_int = 9;
pub const F_GETFD: c_int = 1;
pub const F_SETFD: c_int = 2;
pub const F_GETFL: c_int = 3;
pub const F_SETFL: c_int = 4;
pub const F_GETXFL: c_int = 45;

pub const SIGTRAP: c_int = 5;

pub const GLOB_APPEND: c_int = 32;
pub const GLOB_DOOFFS: c_int = 16;
pub const GLOB_ERR: c_int = 1;
pub const GLOB_MARK: c_int = 2;
pub const GLOB_NOCHECK: c_int = 8;
pub const GLOB_NOSORT: c_int = 4;
pub const GLOB_NOESCAPE: c_int = 64;

pub const GLOB_NOSPACE: c_int = -2;
pub const GLOB_ABORTED: c_int = -1;
pub const GLOB_NOMATCH: c_int = -3;

pub const POLLIN: c_short = 0x1;
pub const POLLPRI: c_short = 0x2;
pub const POLLOUT: c_short = 0x4;
pub const POLLERR: c_short = 0x8;
pub const POLLHUP: c_short = 0x10;
pub const POLLNVAL: c_short = 0x20;
pub const POLLNORM: c_short = 0x0040;
pub const POLLRDNORM: c_short = 0x0040;
pub const POLLWRNORM: c_short = 0x4; /* POLLOUT */
pub const POLLRDBAND: c_short = 0x0080;
pub const POLLWRBAND: c_short = 0x0100;

pub const POSIX_MADV_NORMAL: c_int = 0;
pub const POSIX_MADV_RANDOM: c_int = 1;
pub const POSIX_MADV_SEQUENTIAL: c_int = 2;
pub const POSIX_MADV_WILLNEED: c_int = 3;
pub const POSIX_MADV_DONTNEED: c_int = 4;

pub const POSIX_SPAWN_RESETIDS: c_short = 0x1;
pub const POSIX_SPAWN_SETPGROUP: c_short = 0x2;
pub const POSIX_SPAWN_SETSIGDEF: c_short = 0x4;
pub const POSIX_SPAWN_SETSIGMASK: c_short = 0x8;
pub const POSIX_SPAWN_SETSCHEDPARAM: c_short = 0x10;
pub const POSIX_SPAWN_SETSCHEDULER: c_short = 0x20;
pub const POSIX_SPAWN_SETSIGIGN_NP: c_short = 0x800;
pub const POSIX_SPAWN_NOSIGCHLD_NP: c_short = 0x1000;
pub const POSIX_SPAWN_WAITPID_NP: c_short = 0x2000;
pub const POSIX_SPAWN_NOEXECERR_NP: c_short = 0x4000;

pub const PTHREAD_CREATE_JOINABLE: c_int = 0;
pub const PTHREAD_CREATE_DETACHED: c_int = 0x40;
pub const PTHREAD_PROCESS_SHARED: c_int = 1;
pub const PTHREAD_PROCESS_PRIVATE: c_ushort = 0;
pub const PTHREAD_STACK_MIN: size_t = 4096;

pub const SIGSTKSZ: size_t = 8192;

// https://illumos.org/man/3c/clock_gettime
// https://github.com/illumos/illumos-gate/
//   blob/HEAD/usr/src/lib/libc/amd64/sys/__clock_gettime.s
// clock_gettime(3c) doesn't seem to accept anything other than CLOCK_REALTIME
// or __CLOCK_REALTIME0
//
// https://github.com/illumos/illumos-gate/
//   blob/HEAD/usr/src/uts/common/sys/time_impl.h
// Confusing! CLOCK_HIGHRES==CLOCK_MONOTONIC==4
// __CLOCK_REALTIME0==0 is an obsoleted version of CLOCK_REALTIME==3
pub const CLOCK_REALTIME: crate::clockid_t = 3;
pub const CLOCK_MONOTONIC: crate::clockid_t = 4;
pub const TIMER_RELTIME: c_int = 0;
pub const TIMER_ABSTIME: c_int = 1;

pub const RLIMIT_CPU: c_int = 0;
pub const RLIMIT_FSIZE: c_int = 1;
pub const RLIMIT_DATA: c_int = 2;
pub const RLIMIT_STACK: c_int = 3;
pub const RLIMIT_CORE: c_int = 4;
pub const RLIMIT_NOFILE: c_int = 5;
pub const RLIMIT_VMEM: c_int = 6;
pub const RLIMIT_AS: c_int = RLIMIT_VMEM;

#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIM_NLIMITS: rlim_t = 7;
pub const RLIM_INFINITY: rlim_t = 0xfffffffffffffffd;

pub const RUSAGE_SELF: c_int = 0;
pub const RUSAGE_CHILDREN: c_int = -1;

pub const MADV_NORMAL: c_int = 0;
pub const MADV_RANDOM: c_int = 1;
pub const MADV_SEQUENTIAL: c_int = 2;
pub const MADV_WILLNEED: c_int = 3;
pub const MADV_DONTNEED: c_int = 4;
pub const MADV_FREE: c_int = 5;
pub const MADV_ACCESS_DEFAULT: c_int = 6;
pub const MADV_ACCESS_LWP: c_int = 7;
pub const MADV_ACCESS_MANY: c_int = 8;

pub const AF_UNSPEC: c_int = 0;
pub const AF_UNIX: c_int = 1;
pub const AF_INET: c_int = 2;
pub const AF_IMPLINK: c_int = 3;
pub const AF_PUP: c_int = 4;
pub const AF_CHAOS: c_int = 5;
pub const AF_NS: c_int = 6;
pub const AF_NBS: c_int = 7;
pub const AF_ECMA: c_int = 8;
pub const AF_DATAKIT: c_int = 9;
pub const AF_CCITT: c_int = 10;
pub const AF_SNA: c_int = 11;
pub const AF_DECnet: c_int = 12;
pub const AF_DLI: c_int = 13;
pub const AF_LAT: c_int = 14;
pub const AF_HYLINK: c_int = 15;
pub const AF_APPLETALK: c_int = 16;
pub const AF_NIT: c_int = 17;
pub const AF_802: c_int = 18;
pub const AF_OSI: c_int = 19;
pub const AF_X25: c_int = 20;
pub const AF_OSINET: c_int = 21;
pub const AF_GOSIP: c_int = 22;
pub const AF_IPX: c_int = 23;
pub const AF_ROUTE: c_int = 24;
pub const AF_LINK: c_int = 25;
pub const AF_INET6: c_int = 26;
pub const AF_KEY: c_int = 27;
pub const AF_POLICY: c_int = 29;
pub const AF_INET_OFFLOAD: c_int = 30;
pub const AF_TRILL: c_int = 31;
pub const AF_PACKET: c_int = 32;

pub const PF_UNSPEC: c_int = AF_UNSPEC;
pub const PF_UNIX: c_int = AF_UNIX;
pub const PF_LOCAL: c_int = PF_UNIX;
pub const PF_FILE: c_int = PF_UNIX;
pub const PF_INET: c_int = AF_INET;
pub const PF_IMPLINK: c_int = AF_IMPLINK;
pub const PF_PUP: c_int = AF_PUP;
pub const PF_CHAOS: c_int = AF_CHAOS;
pub const PF_NS: c_int = AF_NS;
pub const PF_NBS: c_int = AF_NBS;
pub const PF_ECMA: c_int = AF_ECMA;
pub const PF_DATAKIT: c_int = AF_DATAKIT;
pub const PF_CCITT: c_int = AF_CCITT;
pub const PF_SNA: c_int = AF_SNA;
pub const PF_DECnet: c_int = AF_DECnet;
pub const PF_DLI: c_int = AF_DLI;
pub const PF_LAT: c_int = AF_LAT;
pub const PF_HYLINK: c_int = AF_HYLINK;
pub const PF_APPLETALK: c_int = AF_APPLETALK;
pub const PF_NIT: c_int = AF_NIT;
pub const PF_802: c_int = AF_802;
pub const PF_OSI: c_int = AF_OSI;
pub const PF_X25: c_int = AF_X25;
pub const PF_OSINET: c_int = AF_OSINET;
pub const PF_GOSIP: c_int = AF_GOSIP;
pub const PF_IPX: c_int = AF_IPX;
pub const PF_ROUTE: c_int = AF_ROUTE;
pub const PF_LINK: c_int = AF_LINK;
pub const PF_INET6: c_int = AF_INET6;
pub const PF_KEY: c_int = AF_KEY;
pub const PF_POLICY: c_int = AF_POLICY;
pub const PF_INET_OFFLOAD: c_int = AF_INET_OFFLOAD;
pub const PF_TRILL: c_int = AF_TRILL;
pub const PF_PACKET: c_int = AF_PACKET;

pub const SOCK_DGRAM: c_int = 1;
pub const SOCK_STREAM: c_int = 2;
pub const SOCK_RAW: c_int = 4;
pub const SOCK_RDM: c_int = 5;
pub const SOCK_SEQPACKET: c_int = 6;
pub const IP_MULTICAST_IF: c_int = 16;
pub const IP_MULTICAST_TTL: c_int = 17;
pub const IP_MULTICAST_LOOP: c_int = 18;
pub const IP_HDRINCL: c_int = 2;
pub const IP_TOS: c_int = 3;
pub const IP_TTL: c_int = 4;
pub const IP_ADD_MEMBERSHIP: c_int = 19;
pub const IP_DROP_MEMBERSHIP: c_int = 20;
pub const IPV6_JOIN_GROUP: c_int = 9;
pub const IPV6_LEAVE_GROUP: c_int = 10;
pub const IP_ADD_SOURCE_MEMBERSHIP: c_int = 23;
pub const IP_DROP_SOURCE_MEMBERSHIP: c_int = 24;
pub const IP_BLOCK_SOURCE: c_int = 21;
pub const IP_UNBLOCK_SOURCE: c_int = 22;
pub const IP_BOUND_IF: c_int = 0x41;

// These TCP socket options are common between illumos and Solaris, while higher
// numbers have generally diverged:
pub const TCP_NODELAY: c_int = 0x1;
pub const TCP_MAXSEG: c_int = 0x2;
pub const TCP_KEEPALIVE: c_int = 0x8;
pub const TCP_NOTIFY_THRESHOLD: c_int = 0x10;
pub const TCP_ABORT_THRESHOLD: c_int = 0x11;
pub const TCP_CONN_NOTIFY_THRESHOLD: c_int = 0x12;
pub const TCP_CONN_ABORT_THRESHOLD: c_int = 0x13;
pub const TCP_RECVDSTADDR: c_int = 0x14;
pub const TCP_INIT_CWND: c_int = 0x15;
pub const TCP_KEEPALIVE_THRESHOLD: c_int = 0x16;
pub const TCP_KEEPALIVE_ABORT_THRESHOLD: c_int = 0x17;
pub const TCP_CORK: c_int = 0x18;
pub const TCP_RTO_INITIAL: c_int = 0x19;
pub const TCP_RTO_MIN: c_int = 0x1a;
pub const TCP_RTO_MAX: c_int = 0x1b;
pub const TCP_LINGER2: c_int = 0x1c;

pub const UDP_NAT_T_ENDPOINT: c_int = 0x0103;

pub const SOMAXCONN: c_int = 128;

pub const SOL_SOCKET: c_int = 0xffff;
pub const SO_DEBUG: c_int = 0x01;
pub const SO_ACCEPTCONN: c_int = 0x0002;
pub const SO_REUSEADDR: c_int = 0x0004;
pub const SO_KEEPALIVE: c_int = 0x0008;
pub const SO_DONTROUTE: c_int = 0x0010;
pub const SO_BROADCAST: c_int = 0x0020;
pub const SO_USELOOPBACK: c_int = 0x0040;
pub const SO_LINGER: c_int = 0x0080;
pub const SO_OOBINLINE: c_int = 0x0100;
pub const SO_SNDBUF: c_int = 0x1001;
pub const SO_RCVBUF: c_int = 0x1002;
pub const SO_SNDLOWAT: c_int = 0x1003;
pub const SO_RCVLOWAT: c_int = 0x1004;
pub const SO_SNDTIMEO: c_int = 0x1005;
pub const SO_RCVTIMEO: c_int = 0x1006;
pub const SO_ERROR: c_int = 0x1007;
pub const SO_TYPE: c_int = 0x1008;
pub const SO_PROTOTYPE: c_int = 0x1009;
pub const SO_DOMAIN: c_int = 0x100c;
pub const SO_TIMESTAMP: c_int = 0x1013;
pub const SO_EXCLBIND: c_int = 0x1015;

pub const SCM_RIGHTS: c_int = 0x1010;
pub const SCM_UCRED: c_int = 0x1012;
pub const SCM_TIMESTAMP: c_int = SO_TIMESTAMP;

pub const MSG_OOB: c_int = 0x1;
pub const MSG_PEEK: c_int = 0x2;
pub const MSG_DONTROUTE: c_int = 0x4;
pub const MSG_EOR: c_int = 0x8;
pub const MSG_CTRUNC: c_int = 0x10;
pub const MSG_TRUNC: c_int = 0x20;
pub const MSG_WAITALL: c_int = 0x40;
pub const MSG_DONTWAIT: c_int = 0x80;
pub const MSG_NOTIFICATION: c_int = 0x100;
pub const MSG_NOSIGNAL: c_int = 0x200;
pub const MSG_DUPCTRL: c_int = 0x800;
pub const MSG_XPG4_2: c_int = 0x8000;
pub const MSG_MAXIOVLEN: c_int = 16;

pub const IF_NAMESIZE: size_t = 32;
pub const IFNAMSIZ: size_t = 16;

// https://docs.oracle.com/cd/E23824_01/html/821-1475/if-7p.html
pub const IFF_UP: c_int = 0x0000000001; // Address is up
pub const IFF_BROADCAST: c_int = 0x0000000002; // Broadcast address valid
pub const IFF_DEBUG: c_int = 0x0000000004; // Turn on debugging
pub const IFF_LOOPBACK: c_int = 0x0000000008; // Loopback net
pub const IFF_POINTOPOINT: c_int = 0x0000000010; // Interface is p-to-p
pub const IFF_NOTRAILERS: c_int = 0x0000000020; // Avoid use of trailers
pub const IFF_RUNNING: c_int = 0x0000000040; // Resources allocated
pub const IFF_NOARP: c_int = 0x0000000080; // No address res. protocol
pub const IFF_PROMISC: c_int = 0x0000000100; // Receive all packets
pub const IFF_ALLMULTI: c_int = 0x0000000200; // Receive all multicast pkts
pub const IFF_INTELLIGENT: c_int = 0x0000000400; // Protocol code on board
pub const IFF_MULTICAST: c_int = 0x0000000800; // Supports multicast

// Multicast using broadcst. add.
pub const IFF_MULTI_BCAST: c_int = 0x0000001000;
pub const IFF_UNNUMBERED: c_int = 0x0000002000; // Non-unique address
pub const IFF_DHCPRUNNING: c_int = 0x0000004000; // DHCP controls interface
pub const IFF_PRIVATE: c_int = 0x0000008000; // Do not advertise
pub const IFF_NOXMIT: c_int = 0x0000010000; // Do not transmit pkts

// No address - just on-link subnet
pub const IFF_NOLOCAL: c_int = 0x0000020000;
pub const IFF_DEPRECATED: c_int = 0x0000040000; // Address is deprecated
pub const IFF_ADDRCONF: c_int = 0x0000080000; // Addr. from stateless addrconf
pub const IFF_ROUTER: c_int = 0x0000100000; // Router on interface
pub const IFF_NONUD: c_int = 0x0000200000; // No NUD on interface
pub const IFF_ANYCAST: c_int = 0x0000400000; // Anycast address
pub const IFF_NORTEXCH: c_int = 0x0000800000; // Don't xchange rout. info
pub const IFF_IPV4: c_int = 0x0001000000; // IPv4 interface
pub const IFF_IPV6: c_int = 0x0002000000; // IPv6 interface
pub const IFF_NOFAILOVER: c_int = 0x0008000000; // in.mpathd test address
pub const IFF_FAILED: c_int = 0x0010000000; // Interface has failed
pub const IFF_STANDBY: c_int = 0x0020000000; // Interface is a hot-spare
pub const IFF_INACTIVE: c_int = 0x0040000000; // Functioning but not used
pub const IFF_OFFLINE: c_int = 0x0080000000; // Interface is offline
                                             // If CoS marking is supported
pub const IFF_COS_ENABLED: c_longlong = 0x0200000000;
pub const IFF_PREFERRED: c_longlong = 0x0400000000; // Prefer as source addr.
pub const IFF_TEMPORARY: c_longlong = 0x0800000000; // RFC3041
pub const IFF_FIXEDMTU: c_longlong = 0x1000000000; // MTU set with SIOCSLIFMTU
pub const IFF_VIRTUAL: c_longlong = 0x2000000000; // Cannot send/receive pkts
pub const IFF_DUPLICATE: c_longlong = 0x4000000000; // Local address in use
pub const IFF_IPMP: c_longlong = 0x8000000000; // IPMP IP interface

// sys/ipc.h:
pub const IPC_ALLOC: c_int = 0x8000;
pub const IPC_CREAT: c_int = 0x200;
pub const IPC_EXCL: c_int = 0x400;
pub const IPC_NOWAIT: c_int = 0x800;
pub const IPC_PRIVATE: key_t = 0;
pub const IPC_RMID: c_int = 10;
pub const IPC_SET: c_int = 11;
pub const IPC_SEAT: c_int = 12;

// sys/shm.h
pub const SHM_R: c_int = 0o400;
pub const SHM_W: c_int = 0o200;
pub const SHM_RDONLY: c_int = 0o10000;
pub const SHM_RND: c_int = 0o20000;
pub const SHM_SHARE_MMU: c_int = 0o40000;
pub const SHM_PAGEABLE: c_int = 0o100000;

pub const SHUT_RD: c_int = 0;
pub const SHUT_WR: c_int = 1;
pub const SHUT_RDWR: c_int = 2;

pub const F_RDLCK: c_short = 1;
pub const F_WRLCK: c_short = 2;
pub const F_UNLCK: c_short = 3;

pub const O_SYNC: c_int = 16;
pub const O_NONBLOCK: c_int = 128;

pub const IPPROTO_RAW: c_int = 255;

pub const _PC_LINK_MAX: c_int = 1;
pub const _PC_MAX_CANON: c_int = 2;
pub const _PC_MAX_INPUT: c_int = 3;
pub const _PC_NAME_MAX: c_int = 4;
pub const _PC_PATH_MAX: c_int = 5;
pub const _PC_PIPE_BUF: c_int = 6;
pub const _PC_NO_TRUNC: c_int = 7;
pub const _PC_VDISABLE: c_int = 8;
pub const _PC_CHOWN_RESTRICTED: c_int = 9;
pub const _PC_ASYNC_IO: c_int = 10;
pub const _PC_PRIO_IO: c_int = 11;
pub const _PC_SYNC_IO: c_int = 12;
pub const _PC_ALLOC_SIZE_MIN: c_int = 13;
pub const _PC_REC_INCR_XFER_SIZE: c_int = 14;
pub const _PC_REC_MAX_XFER_SIZE: c_int = 15;
pub const _PC_REC_MIN_XFER_SIZE: c_int = 16;
pub const _PC_REC_XFER_ALIGN: c_int = 17;
pub const _PC_SYMLINK_MAX: c_int = 18;
pub const _PC_2_SYMLINKS: c_int = 19;
pub const _PC_ACL_ENABLED: c_int = 20;
pub const _PC_MIN_HOLE_SIZE: c_int = 21;
pub const _PC_CASE_BEHAVIOR: c_int = 22;
pub const _PC_SATTR_ENABLED: c_int = 23;
pub const _PC_SATTR_EXISTS: c_int = 24;
pub const _PC_ACCESS_FILTERING: c_int = 25;
pub const _PC_TIMESTAMP_RESOLUTION: c_int = 26;
pub const _PC_FILESIZEBITS: c_int = 67;
pub const _PC_XATTR_ENABLED: c_int = 100;
pub const _PC_XATTR_EXISTS: c_int = 101;

pub const _POSIX_VDISABLE: crate::cc_t = 0;

pub const _SC_ARG_MAX: c_int = 1;
pub const _SC_CHILD_MAX: c_int = 2;
pub const _SC_CLK_TCK: c_int = 3;
pub const _SC_NGROUPS_MAX: c_int = 4;
pub const _SC_OPEN_MAX: c_int = 5;
pub const _SC_JOB_CONTROL: c_int = 6;
pub const _SC_SAVED_IDS: c_int = 7;
pub const _SC_VERSION: c_int = 8;
pub const _SC_PASS_MAX: c_int = 9;
pub const _SC_LOGNAME_MAX: c_int = 10;
pub const _SC_PAGESIZE: c_int = 11;
pub const _SC_PAGE_SIZE: c_int = _SC_PAGESIZE;
pub const _SC_XOPEN_VERSION: c_int = 12;
pub const _SC_NPROCESSORS_CONF: c_int = 14;
pub const _SC_NPROCESSORS_ONLN: c_int = 15;
pub const _SC_STREAM_MAX: c_int = 16;
pub const _SC_TZNAME_MAX: c_int = 17;
pub const _SC_AIO_LISTIO_MAX: c_int = 18;
pub const _SC_AIO_MAX: c_int = 19;
pub const _SC_AIO_PRIO_DELTA_MAX: c_int = 20;
pub const _SC_ASYNCHRONOUS_IO: c_int = 21;
pub const _SC_DELAYTIMER_MAX: c_int = 22;
pub const _SC_FSYNC: c_int = 23;
pub const _SC_MAPPED_FILES: c_int = 24;
pub const _SC_MEMLOCK: c_int = 25;
pub const _SC_MEMLOCK_RANGE: c_int = 26;
pub const _SC_MEMORY_PROTECTION: c_int = 27;
pub const _SC_MESSAGE_PASSING: c_int = 28;
pub const _SC_MQ_OPEN_MAX: c_int = 29;
pub const _SC_MQ_PRIO_MAX: c_int = 30;
pub const _SC_PRIORITIZED_IO: c_int = 31;
pub const _SC_PRIORITY_SCHEDULING: c_int = 32;
pub const _SC_REALTIME_SIGNALS: c_int = 33;
pub const _SC_RTSIG_MAX: c_int = 34;
pub const _SC_SEMAPHORES: c_int = 35;
pub const _SC_SEM_NSEMS_MAX: c_int = 36;
pub const _SC_SEM_VALUE_MAX: c_int = 37;
pub const _SC_SHARED_MEMORY_OBJECTS: c_int = 38;
pub const _SC_SIGQUEUE_MAX: c_int = 39;
pub const _SC_SIGRT_MIN: c_int = 40;
pub const _SC_SIGRT_MAX: c_int = 41;
pub const _SC_SYNCHRONIZED_IO: c_int = 42;
pub const _SC_TIMERS: c_int = 43;
pub const _SC_TIMER_MAX: c_int = 44;
pub const _SC_2_C_BIND: c_int = 45;
pub const _SC_2_C_DEV: c_int = 46;
pub const _SC_2_C_VERSION: c_int = 47;
pub const _SC_2_FORT_DEV: c_int = 48;
pub const _SC_2_FORT_RUN: c_int = 49;
pub const _SC_2_LOCALEDEF: c_int = 50;
pub const _SC_2_SW_DEV: c_int = 51;
pub const _SC_2_UPE: c_int = 52;
pub const _SC_2_VERSION: c_int = 53;
pub const _SC_BC_BASE_MAX: c_int = 54;
pub const _SC_BC_DIM_MAX: c_int = 55;
pub const _SC_BC_SCALE_MAX: c_int = 56;
pub const _SC_BC_STRING_MAX: c_int = 57;
pub const _SC_COLL_WEIGHTS_MAX: c_int = 58;
pub const _SC_EXPR_NEST_MAX: c_int = 59;
pub const _SC_LINE_MAX: c_int = 60;
pub const _SC_RE_DUP_MAX: c_int = 61;
pub const _SC_XOPEN_CRYPT: c_int = 62;
pub const _SC_XOPEN_ENH_I18N: c_int = 63;
pub const _SC_XOPEN_SHM: c_int = 64;
pub const _SC_2_CHAR_TERM: c_int = 66;
pub const _SC_XOPEN_XCU_VERSION: c_int = 67;
pub const _SC_ATEXIT_MAX: c_int = 76;
pub const _SC_IOV_MAX: c_int = 77;
pub const _SC_XOPEN_UNIX: c_int = 78;
pub const _SC_T_IOV_MAX: c_int = 79;
pub const _SC_PHYS_PAGES: c_int = 500;
pub const _SC_AVPHYS_PAGES: c_int = 501;
pub const _SC_COHER_BLKSZ: c_int = 503;
pub const _SC_SPLIT_CACHE: c_int = 504;
pub const _SC_ICACHE_SZ: c_int = 505;
pub const _SC_DCACHE_SZ: c_int = 506;
pub const _SC_ICACHE_LINESZ: c_int = 507;
pub const _SC_DCACHE_LINESZ: c_int = 508;
pub const _SC_ICACHE_BLKSZ: c_int = 509;
pub const _SC_DCACHE_BLKSZ: c_int = 510;
pub const _SC_DCACHE_TBLKSZ: c_int = 511;
pub const _SC_ICACHE_ASSOC: c_int = 512;
pub const _SC_DCACHE_ASSOC: c_int = 513;
pub const _SC_MAXPID: c_int = 514;
pub const _SC_STACK_PROT: c_int = 515;
pub const _SC_NPROCESSORS_MAX: c_int = 516;
pub const _SC_CPUID_MAX: c_int = 517;
pub const _SC_EPHID_MAX: c_int = 518;
pub const _SC_THREAD_DESTRUCTOR_ITERATIONS: c_int = 568;
pub const _SC_GETGR_R_SIZE_MAX: c_int = 569;
pub const _SC_GETPW_R_SIZE_MAX: c_int = 570;
pub const _SC_LOGIN_NAME_MAX: c_int = 571;
pub const _SC_THREAD_KEYS_MAX: c_int = 572;
pub const _SC_THREAD_STACK_MIN: c_int = 573;
pub const _SC_THREAD_THREADS_MAX: c_int = 574;
pub const _SC_TTY_NAME_MAX: c_int = 575;
pub const _SC_THREADS: c_int = 576;
pub const _SC_THREAD_ATTR_STACKADDR: c_int = 577;
pub const _SC_THREAD_ATTR_STACKSIZE: c_int = 578;
pub const _SC_THREAD_PRIORITY_SCHEDULING: c_int = 579;
pub const _SC_THREAD_PRIO_INHERIT: c_int = 580;
pub const _SC_THREAD_PRIO_PROTECT: c_int = 581;
pub const _SC_THREAD_PROCESS_SHARED: c_int = 582;
pub const _SC_THREAD_SAFE_FUNCTIONS: c_int = 583;
pub const _SC_XOPEN_LEGACY: c_int = 717;
pub const _SC_XOPEN_REALTIME: c_int = 718;
pub const _SC_XOPEN_REALTIME_THREADS: c_int = 719;
pub const _SC_XBS5_ILP32_OFF32: c_int = 720;
pub const _SC_XBS5_ILP32_OFFBIG: c_int = 721;
pub const _SC_XBS5_LP64_OFF64: c_int = 722;
pub const _SC_XBS5_LPBIG_OFFBIG: c_int = 723;
pub const _SC_2_PBS: c_int = 724;
pub const _SC_2_PBS_ACCOUNTING: c_int = 725;
pub const _SC_2_PBS_CHECKPOINT: c_int = 726;
pub const _SC_2_PBS_LOCATE: c_int = 728;
pub const _SC_2_PBS_MESSAGE: c_int = 729;
pub const _SC_2_PBS_TRACK: c_int = 730;
pub const _SC_ADVISORY_INFO: c_int = 731;
pub const _SC_BARRIERS: c_int = 732;
pub const _SC_CLOCK_SELECTION: c_int = 733;
pub const _SC_CPUTIME: c_int = 734;
pub const _SC_HOST_NAME_MAX: c_int = 735;
pub const _SC_MONOTONIC_CLOCK: c_int = 736;
pub const _SC_READER_WRITER_LOCKS: c_int = 737;
pub const _SC_REGEXP: c_int = 738;
pub const _SC_SHELL: c_int = 739;
pub const _SC_SPAWN: c_int = 740;
pub const _SC_SPIN_LOCKS: c_int = 741;
pub const _SC_SPORADIC_SERVER: c_int = 742;
pub const _SC_SS_REPL_MAX: c_int = 743;
pub const _SC_SYMLOOP_MAX: c_int = 744;
pub const _SC_THREAD_CPUTIME: c_int = 745;
pub const _SC_THREAD_SPORADIC_SERVER: c_int = 746;
pub const _SC_TIMEOUTS: c_int = 747;
pub const _SC_TRACE: c_int = 748;
pub const _SC_TRACE_EVENT_FILTER: c_int = 749;
pub const _SC_TRACE_EVENT_NAME_MAX: c_int = 750;
pub const _SC_TRACE_INHERIT: c_int = 751;
pub const _SC_TRACE_LOG: c_int = 752;
pub const _SC_TRACE_NAME_MAX: c_int = 753;
pub const _SC_TRACE_SYS_MAX: c_int = 754;
pub const _SC_TRACE_USER_EVENT_MAX: c_int = 755;
pub const _SC_TYPED_MEMORY_OBJECTS: c_int = 756;
pub const _SC_V6_ILP32_OFF32: c_int = 757;
pub const _SC_V6_ILP32_OFFBIG: c_int = 758;
pub const _SC_V6_LP64_OFF64: c_int = 759;
pub const _SC_V6_LPBIG_OFFBIG: c_int = 760;
pub const _SC_XOPEN_STREAMS: c_int = 761;
pub const _SC_IPV6: c_int = 762;
pub const _SC_RAW_SOCKETS: c_int = 763;

pub const _ST_FSTYPSZ: c_int = 16;

pub const _MUTEX_MAGIC: u16 = 0x4d58; // MX
pub const _COND_MAGIC: u16 = 0x4356; // CV
pub const _RWL_MAGIC: u16 = 0x5257; // RW

pub const NCCS: usize = 19;

pub const LOG_CRON: c_int = 15 << 3;

pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
    __pthread_mutex_flag1: 0,
    __pthread_mutex_flag2: 0,
    __pthread_mutex_ceiling: 0,
    __pthread_mutex_type: PTHREAD_PROCESS_PRIVATE,
    __pthread_mutex_magic: _MUTEX_MAGIC,
    __pthread_mutex_lock: 0,
    __pthread_mutex_data: 0,
};
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    __pthread_cond_flag: [0; 4],
    __pthread_cond_type: PTHREAD_PROCESS_PRIVATE,
    __pthread_cond_magic: _COND_MAGIC,
    __pthread_cond_data: 0,
};
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = pthread_rwlock_t {
    __pthread_rwlock_readers: 0,
    __pthread_rwlock_type: PTHREAD_PROCESS_PRIVATE,
    __pthread_rwlock_magic: _RWL_MAGIC,
    __pthread_rwlock_mutex: PTHREAD_MUTEX_INITIALIZER,
    __pthread_rwlock_readercv: PTHREAD_COND_INITIALIZER,
    __pthread_rwlock_writercv: PTHREAD_COND_INITIALIZER,
};
pub const PTHREAD_MUTEX_NORMAL: c_int = 0;
pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 2;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 4;
pub const PTHREAD_MUTEX_DEFAULT: c_int = crate::PTHREAD_MUTEX_NORMAL;

pub const RTLD_NEXT: *mut c_void = -1isize as *mut c_void;
pub const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
pub const RTLD_SELF: *mut c_void = -3isize as *mut c_void;
pub const RTLD_PROBE: *mut c_void = -4isize as *mut c_void;

pub const RTLD_LAZY: c_int = 0x1;
pub const RTLD_NOW: c_int = 0x2;
pub const RTLD_NOLOAD: c_int = 0x4;
pub const RTLD_GLOBAL: c_int = 0x100;
pub const RTLD_LOCAL: c_int = 0x0;
pub const RTLD_PARENT: c_int = 0x200;
pub const RTLD_GROUP: c_int = 0x400;
pub const RTLD_WORLD: c_int = 0x800;
pub const RTLD_NODELETE: c_int = 0x1000;
pub const RTLD_FIRST: c_int = 0x2000;
pub const RTLD_CONFGEN: c_int = 0x10000;

pub const PORT_SOURCE_AIO: c_int = 1;
pub const PORT_SOURCE_TIMER: c_int = 2;
pub const PORT_SOURCE_USER: c_int = 3;
pub const PORT_SOURCE_FD: c_int = 4;
pub const PORT_SOURCE_ALERT: c_int = 5;
pub const PORT_SOURCE_MQ: c_int = 6;
pub const PORT_SOURCE_FILE: c_int = 7;

pub const NONROOT_USR: c_short = 2;

pub const EMPTY: c_short = 0;
pub const RUN_LVL: c_short = 1;
pub const BOOT_TIME: c_short = 2;
pub const OLD_TIME: c_short = 3;
pub const NEW_TIME: c_short = 4;
pub const INIT_PROCESS: c_short = 5;
pub const LOGIN_PROCESS: c_short = 6;
pub const USER_PROCESS: c_short = 7;
pub const DEAD_PROCESS: c_short = 8;
pub const ACCOUNTING: c_short = 9;
pub const DOWN_TIME: c_short = 10;

const _TIOC: c_int = ('T' as i32) << 8;
const tIOC: c_int = ('t' as i32) << 8;
pub const TCGETA: c_int = _TIOC | 1;
pub const TCSETA: c_int = _TIOC | 2;
pub const TCSETAW: c_int = _TIOC | 3;
pub const TCSETAF: c_int = _TIOC | 4;
pub const TCSBRK: c_int = _TIOC | 5;
pub const TCXONC: c_int = _TIOC | 6;
pub const TCFLSH: c_int = _TIOC | 7;
pub const TCDSET: c_int = _TIOC | 32;
pub const TCGETS: c_int = _TIOC | 13;
pub const TCSETS: c_int = _TIOC | 14;
pub const TCSANOW: c_int = _TIOC | 14;
pub const TCSETSW: c_int = _TIOC | 15;
pub const TCSADRAIN: c_int = _TIOC | 15;
pub const TCSETSF: c_int = _TIOC | 16;
pub const TCSAFLUSH: c_int = _TIOC | 16;
pub const TCIFLUSH: c_int = 0;
pub const TCOFLUSH: c_int = 1;
pub const TCIOFLUSH: c_int = 2;
pub const TCOOFF: c_int = 0;
pub const TCOON: c_int = 1;
pub const TCIOFF: c_int = 2;
pub const TCION: c_int = 3;
pub const TIOC: c_int = _TIOC;
pub const TIOCKBON: c_int = _TIOC | 8;
pub const TIOCKBOF: c_int = _TIOC | 9;
pub const TIOCGWINSZ: c_int = _TIOC | 104;
pub const TIOCSWINSZ: c_int = _TIOC | 103;
pub const TIOCGSOFTCAR: c_int = _TIOC | 105;
pub const TIOCSSOFTCAR: c_int = _TIOC | 106;
pub const TIOCGPPS: c_int = _TIOC | 125;
pub const TIOCSPPS: c_int = _TIOC | 126;
pub const TIOCGPPSEV: c_int = _TIOC | 127;
pub const TIOCGETD: c_int = tIOC | 0;
pub const TIOCSETD: c_int = tIOC | 1;
pub const TIOCHPCL: c_int = tIOC | 2;
pub const TIOCGETP: c_int = tIOC | 8;
pub const TIOCSETP: c_int = tIOC | 9;
pub const TIOCSETN: c_int = tIOC | 10;
pub const TIOCEXCL: c_int = tIOC | 13;
pub const TIOCNXCL: c_int = tIOC | 14;
pub const TIOCFLUSH: c_int = tIOC | 16;
pub const TIOCSETC: c_int = tIOC | 17;
pub const TIOCGETC: c_int = tIOC | 18;
pub const TIOCLBIS: c_int = tIOC | 127;
pub const TIOCLBIC: c_int = tIOC | 126;
pub const TIOCLSET: c_int = tIOC | 125;
pub const TIOCLGET: c_int = tIOC | 124;
pub const TIOCSBRK: c_int = tIOC | 123;
pub const TIOCCBRK: c_int = tIOC | 122;
pub const TIOCSDTR: c_int = tIOC | 121;
pub const TIOCCDTR: c_int = tIOC | 120;
pub const TIOCSLTC: c_int = tIOC | 117;
pub const TIOCGLTC: c_int = tIOC | 116;
pub const TIOCOUTQ: c_int = tIOC | 115;
pub const TIOCNOTTY: c_int = tIOC | 113;
pub const TIOCSCTTY: c_int = tIOC | 132;
pub const TIOCSTOP: c_int = tIOC | 111;
pub const TIOCSTART: c_int = tIOC | 110;
pub const TIOCSILOOP: c_int = tIOC | 109;
pub const TIOCCILOOP: c_int = tIOC | 108;
pub const TIOCGPGRP: c_int = tIOC | 20;
pub const TIOCSPGRP: c_int = tIOC | 21;
pub const TIOCGSID: c_int = tIOC | 22;
pub const TIOCSTI: c_int = tIOC | 23;
pub const TIOCMSET: c_int = tIOC | 26;
pub const TIOCMBIS: c_int = tIOC | 27;
pub const TIOCMBIC: c_int = tIOC | 28;
pub const TIOCMGET: c_int = tIOC | 29;
pub const TIOCREMOTE: c_int = tIOC | 30;
pub const TIOCSIGNAL: c_int = tIOC | 31;

pub const TIOCM_LE: c_int = 0o0001;
pub const TIOCM_DTR: c_int = 0o0002;
pub const TIOCM_RTS: c_int = 0o0004;
pub const TIOCM_ST: c_int = 0o0010;
pub const TIOCM_SR: c_int = 0o0020;
pub const TIOCM_CTS: c_int = 0o0040;
pub const TIOCM_CAR: c_int = 0o0100;
pub const TIOCM_CD: c_int = TIOCM_CAR;
pub const TIOCM_RNG: c_int = 0o0200;
pub const TIOCM_RI: c_int = TIOCM_RNG;
pub const TIOCM_DSR: c_int = 0o0400;

/* termios */
pub const B0: speed_t = 0;
pub const B50: speed_t = 1;
pub const B75: speed_t = 2;
pub const B110: speed_t = 3;
pub const B134: speed_t = 4;
pub const B150: speed_t = 5;
pub const B200: speed_t = 6;
pub const B300: speed_t = 7;
pub const B600: speed_t = 8;
pub const B1200: speed_t = 9;
pub const B1800: speed_t = 10;
pub const B2400: speed_t = 11;
pub const B4800: speed_t = 12;
pub const B9600: speed_t = 13;
pub const B19200: speed_t = 14;
pub const B38400: speed_t = 15;
pub const B57600: speed_t = 16;
pub const B76800: speed_t = 17;
pub const B115200: speed_t = 18;
pub const B153600: speed_t = 19;
pub const B230400: speed_t = 20;
pub const B307200: speed_t = 21;
pub const B460800: speed_t = 22;
pub const B921600: speed_t = 23;
pub const CSTART: crate::tcflag_t = 0o21;
pub const CSTOP: crate::tcflag_t = 0o23;
pub const CSWTCH: crate::tcflag_t = 0o32;
pub const CBAUD: crate::tcflag_t = 0o17;
pub const CIBAUD: crate::tcflag_t = 0o3600000;
pub const CBAUDEXT: crate::tcflag_t = 0o10000000;
pub const CIBAUDEXT: crate::tcflag_t = 0o20000000;
pub const CSIZE: crate::tcflag_t = 0o000060;
pub const CS5: crate::tcflag_t = 0;
pub const CS6: crate::tcflag_t = 0o000020;
pub const CS7: crate::tcflag_t = 0o000040;
pub const CS8: crate::tcflag_t = 0o000060;
pub const CSTOPB: crate::tcflag_t = 0o000100;
pub const ECHO: crate::tcflag_t = 0o000010;
pub const ECHOE: crate::tcflag_t = 0o000020;
pub const ECHOK: crate::tcflag_t = 0o000040;
pub const ECHONL: crate::tcflag_t = 0o000100;
pub const ECHOCTL: crate::tcflag_t = 0o001000;
pub const ECHOPRT: crate::tcflag_t = 0o002000;
pub const ECHOKE: crate::tcflag_t = 0o004000;
pub const EXTPROC: crate::tcflag_t = 0o200000;
pub const IGNBRK: crate::tcflag_t = 0o000001;
pub const BRKINT: crate::tcflag_t = 0o000002;
pub const IGNPAR: crate::tcflag_t = 0o000004;
pub const PARMRK: crate::tcflag_t = 0o000010;
pub const INPCK: crate::tcflag_t = 0o000020;
pub const ISTRIP: crate::tcflag_t = 0o000040;
pub const INLCR: crate::tcflag_t = 0o000100;
pub const IGNCR: crate::tcflag_t = 0o000200;
pub const ICRNL: crate::tcflag_t = 0o000400;
pub const IUCLC: crate::tcflag_t = 0o001000;
pub const IXON: crate::tcflag_t = 0o002000;
pub const IXOFF: crate::tcflag_t = 0o010000;
pub const IXANY: crate::tcflag_t = 0o004000;
pub const IMAXBEL: crate::tcflag_t = 0o020000;
pub const DOSMODE: crate::tcflag_t = 0o100000;
pub const OPOST: crate::tcflag_t = 0o000001;
pub const OLCUC: crate::tcflag_t = 0o000002;
pub const ONLCR: crate::tcflag_t = 0o000004;
pub const OCRNL: crate::tcflag_t = 0o000010;
pub const ONOCR: crate::tcflag_t = 0o000020;
pub const ONLRET: crate::tcflag_t = 0o000040;
pub const OFILL: crate::tcflag_t = 0o0000100;
pub const OFDEL: crate::tcflag_t = 0o0000200;
pub const CREAD: crate::tcflag_t = 0o000200;
pub const PARENB: crate::tcflag_t = 0o000400;
pub const PARODD: crate::tcflag_t = 0o001000;
pub const HUPCL: crate::tcflag_t = 0o002000;
pub const CLOCAL: crate::tcflag_t = 0o004000;
pub const CRTSXOFF: crate::tcflag_t = 0o10000000000;
pub const CRTSCTS: crate::tcflag_t = 0o20000000000;
pub const ISIG: crate::tcflag_t = 0o000001;
pub const ICANON: crate::tcflag_t = 0o000002;
pub const IEXTEN: crate::tcflag_t = 0o100000;
pub const TOSTOP: crate::tcflag_t = 0o000400;
pub const FLUSHO: crate::tcflag_t = 0o020000;
pub const PENDIN: crate::tcflag_t = 0o040000;
pub const NOFLSH: crate::tcflag_t = 0o000200;
pub const VINTR: usize = 0;
pub const VQUIT: usize = 1;
pub const VERASE: usize = 2;
pub const VKILL: usize = 3;
pub const VEOF: usize = 4;
pub const VEOL: usize = 5;
pub const VEOL2: usize = 6;
pub const VMIN: usize = 4;
pub const VTIME: usize = 5;
pub const VSWTCH: usize = 7;
pub const VSTART: usize = 8;
pub const VSTOP: usize = 9;
pub const VSUSP: usize = 10;
pub const VDSUSP: usize = 11;
pub const VREPRINT: usize = 12;
pub const VDISCARD: usize = 13;
pub const VWERASE: usize = 14;
pub const VLNEXT: usize = 15;

// <sys/stropts.h>
const STR: c_int = (b'S' as c_int) << 8;
pub const I_NREAD: c_int = STR | 0o1;
pub const I_PUSH: c_int = STR | 0o2;
pub const I_POP: c_int = STR | 0o3;
pub const I_LOOK: c_int = STR | 0o4;
pub const I_FLUSH: c_int = STR | 0o5;
pub const I_SRDOPT: c_int = STR | 0o6;
pub const I_GRDOPT: c_int = STR | 0o7;
pub const I_STR: c_int = STR | 0o10;
pub const I_SETSIG: c_int = STR | 0o11;
pub const I_GETSIG: c_int = STR | 0o12;
pub const I_FIND: c_int = STR | 0o13;
pub const I_LINK: c_int = STR | 0o14;
pub const I_UNLINK: c_int = STR | 0o15;
pub const I_PEEK: c_int = STR | 0o17;
pub const I_FDINSERT: c_int = STR | 0o20;
pub const I_SENDFD: c_int = STR | 0o21;
pub const I_RECVFD: c_int = STR | 0o16;
pub const I_SWROPT: c_int = STR | 0o23;
pub const I_GWROPT: c_int = STR | 0o24;
pub const I_LIST: c_int = STR | 0o25;
pub const I_PLINK: c_int = STR | 0o26;
pub const I_PUNLINK: c_int = STR | 0o27;
pub const I_ANCHOR: c_int = STR | 0o30;
pub const I_FLUSHBAND: c_int = STR | 0o34;
pub const I_CKBAND: c_int = STR | 0o35;
pub const I_GETBAND: c_int = STR | 0o36;
pub const I_ATMARK: c_int = STR | 0o37;
pub const I_SETCLTIME: c_int = STR | 0o40;
pub const I_GETCLTIME: c_int = STR | 0o41;
pub const I_CANPUT: c_int = STR | 0o42;
pub const I_SERROPT: c_int = STR | 0o43;
pub const I_GERROPT: c_int = STR | 0o44;
pub const I_ESETSIG: c_int = STR | 0o45;
pub const I_EGETSIG: c_int = STR | 0o46;
pub const __I_PUSH_NOCTTY: c_int = STR | 0o47;

// 3SOCKET flags
pub const SOCK_CLOEXEC: c_int = 0x080000;
pub const SOCK_NONBLOCK: c_int = 0x100000;
pub const SOCK_NDELAY: c_int = 0x200000;

//<sys/timex.h>
pub const SCALE_KG: c_int = 1 << 6;
pub const SCALE_KF: c_int = 1 << 16;
pub const SCALE_KH: c_int = 1 << 2;
pub const MAXTC: c_int = 1 << 6;
pub const SCALE_PHASE: c_int = 1 << 22;
pub const SCALE_USEC: c_int = 1 << 16;
pub const SCALE_UPDATE: c_int = SCALE_KG * MAXTC;
pub const FINEUSEC: c_int = 1 << 22;
pub const MAXPHASE: c_int = 512000;
pub const MAXFREQ: c_int = 512 * SCALE_USEC;
pub const MAXTIME: c_int = 200 << PPS_AVG;
pub const MINSEC: c_int = 16;
pub const MAXSEC: c_int = 1200;
pub const PPS_AVG: c_int = 2;
pub const PPS_SHIFT: c_int = 2;
pub const PPS_SHIFTMAX: c_int = 8;
pub const PPS_VALID: c_int = 120;
pub const MAXGLITCH: c_int = 30;
pub const MOD_OFFSET: u32 = 0x0001;
pub const MOD_FREQUENCY: u32 = 0x0002;
pub const MOD_MAXERROR: u32 = 0x0004;
pub const MOD_ESTERROR: u32 = 0x0008;
pub const MOD_STATUS: u32 = 0x0010;
pub const MOD_TIMECONST: u32 = 0x0020;
pub const MOD_CLKB: u32 = 0x4000;
pub const MOD_CLKA: u32 = 0x8000;
pub const STA_PLL: u32 = 0x0001;
pub const STA_PPSFREQ: i32 = 0x0002;
pub const STA_PPSTIME: i32 = 0x0004;
pub const STA_FLL: i32 = 0x0008;
pub const STA_INS: i32 = 0x0010;
pub const STA_DEL: i32 = 0x0020;
pub const STA_UNSYNC: i32 = 0x0040;
pub const STA_FREQHOLD: i32 = 0x0080;
pub const STA_PPSSIGNAL: i32 = 0x0100;
pub const STA_PPSJITTER: i32 = 0x0200;
pub const STA_PPSWANDER: i32 = 0x0400;
pub const STA_PPSERROR: i32 = 0x0800;
pub const STA_CLOCKERR: i32 = 0x1000;
pub const STA_RONLY: i32 =
    STA_PPSSIGNAL | STA_PPSJITTER | STA_PPSWANDER | STA_PPSERROR | STA_CLOCKERR;
pub const TIME_OK: i32 = 0;
pub const TIME_INS: i32 = 1;
pub const TIME_DEL: i32 = 2;
pub const TIME_OOP: i32 = 3;
pub const TIME_WAIT: i32 = 4;
pub const TIME_ERROR: i32 = 5;

pub const PRIO_PROCESS: c_int = 0;
pub const PRIO_PGRP: c_int = 1;
pub const PRIO_USER: c_int = 2;

pub const SCHED_OTHER: c_int = 0;
pub const SCHED_FIFO: c_int = 1;
pub const SCHED_RR: c_int = 2;
pub const SCHED_SYS: c_int = 3;
pub const SCHED_IA: c_int = 4;
pub const SCHED_FSS: c_int = 5;
pub const SCHED_FX: c_int = 6;

// sys/priv.h
pub const PRIV_DEBUG: c_uint = 0x0001;
pub const PRIV_AWARE: c_uint = 0x0002;
pub const PRIV_AWARE_INHERIT: c_uint = 0x0004;
pub const __PROC_PROTECT: c_uint = 0x0008;
pub const NET_MAC_AWARE: c_uint = 0x0010;
pub const NET_MAC_AWARE_INHERIT: c_uint = 0x0020;
pub const PRIV_AWARE_RESET: c_uint = 0x0040;
pub const PRIV_XPOLICY: c_uint = 0x0080;
pub const PRIV_PFEXEC: c_uint = 0x0100;

// sys/systeminfo.h
pub const SI_SYSNAME: c_int = 1;
pub const SI_HOSTNAME: c_int = 2;
pub const SI_RELEASE: c_int = 3;
pub const SI_VERSION: c_int = 4;
pub const SI_MACHINE: c_int = 5;
pub const SI_ARCHITECTURE: c_int = 6;
pub const SI_HW_SERIAL: c_int = 7;
pub const SI_HW_PROVIDER: c_int = 8;
pub const SI_SET_HOSTNAME: c_int = 258;
pub const SI_SET_SRPC_DOMAIN: c_int = 265;
pub const SI_PLATFORM: c_int = 513;
pub const SI_ISALIST: c_int = 514;
pub const SI_DHCP_CACHE: c_int = 515;
pub const SI_ARCHITECTURE_32: c_int = 516;
pub const SI_ARCHITECTURE_64: c_int = 517;
pub const SI_ARCHITECTURE_K: c_int = 518;
pub const SI_ARCHITECTURE_NATIVE: c_int = 519;

// sys/lgrp_user.h
pub const LGRP_COOKIE_NONE: crate::lgrp_cookie_t = 0;
pub const LGRP_AFF_NONE: crate::lgrp_affinity_t = 0x0;
pub const LGRP_AFF_WEAK: crate::lgrp_affinity_t = 0x10;
pub const LGRP_AFF_STRONG: crate::lgrp_affinity_t = 0x100;
pub const LGRP_CONTENT_ALL: crate::lgrp_content_t = 0;
pub const LGRP_CONTENT_HIERARCHY: crate::lgrp_content_t = LGRP_CONTENT_ALL;
pub const LGRP_CONTENT_DIRECT: crate::lgrp_content_t = 1;
pub const LGRP_LAT_CPU_TO_MEM: crate::lgrp_lat_between_t = 0;
pub const LGRP_MEM_SZ_FREE: crate::lgrp_mem_size_flag_t = 0;
pub const LGRP_MEM_SZ_INSTALLED: crate::lgrp_mem_size_flag_t = 1;
pub const LGRP_VIEW_CALLER: crate::lgrp_view_t = 0;
pub const LGRP_VIEW_OS: crate::lgrp_view_t = 1;

// sys/processor.h

pub const P_OFFLINE: c_int = 0x001;
pub const P_ONLINE: c_int = 0x002;
pub const P_STATUS: c_int = 0x003;
pub const P_FAULTED: c_int = 0x004;
pub const P_POWEROFF: c_int = 0x005;
pub const P_NOINTR: c_int = 0x006;
pub const P_SPARE: c_int = 0x007;
pub const P_FORCED: c_int = 0x10000000;
pub const PI_TYPELEN: c_int = 16;
pub const PI_FPUTYPE: c_int = 32;

// sys/auxv.h
pub const AT_SUN_HWCAP: c_uint = 2009;

// As per sys/socket.h, header alignment must be 8 bytes on SPARC
// and 4 bytes everywhere else:
#[cfg(target_arch = "sparc64")]
const _CMSG_HDR_ALIGNMENT: usize = 8;
#[cfg(not(target_arch = "sparc64"))]
const _CMSG_HDR_ALIGNMENT: usize = 4;

const _CMSG_DATA_ALIGNMENT: usize = size_of::<c_int>();

const NEWDEV: c_int = 1;

// sys/sendfile.h
pub const SFV_FD_SELF: c_int = -2;

const_fn! {
    {const} fn _CMSG_HDR_ALIGN(p: usize) -> usize {
        (p + _CMSG_HDR_ALIGNMENT - 1) & !(_CMSG_HDR_ALIGNMENT - 1)
    }

    {const} fn _CMSG_DATA_ALIGN(p: usize) -> usize {
        (p + _CMSG_DATA_ALIGNMENT - 1) & !(_CMSG_DATA_ALIGNMENT - 1)
    }
}

f! {
    pub fn CMSG_DATA(cmsg: *const cmsghdr) -> *mut c_uchar {
        _CMSG_DATA_ALIGN(cmsg.offset(1) as usize) as *mut c_uchar
    }

    pub {const} fn CMSG_LEN(length: c_uint) -> c_uint {
        _CMSG_DATA_ALIGN(size_of::<cmsghdr>()) as c_uint + length
    }

    pub fn CMSG_FIRSTHDR(mhdr: *const crate::msghdr) -> *mut cmsghdr {
        if ((*mhdr).msg_controllen as usize) < size_of::<cmsghdr>() {
            core::ptr::null_mut::<cmsghdr>()
        } else {
            (*mhdr).msg_control as *mut cmsghdr
        }
    }

    pub fn CMSG_NXTHDR(mhdr: *const crate::msghdr, cmsg: *const cmsghdr) -> *mut cmsghdr {
        if cmsg.is_null() {
            return crate::CMSG_FIRSTHDR(mhdr);
        }
        let next =
            _CMSG_HDR_ALIGN(cmsg as usize + (*cmsg).cmsg_len as usize + size_of::<cmsghdr>());
        let max = (*mhdr).msg_control as usize + (*mhdr).msg_controllen as usize;
        if next > max {
            core::ptr::null_mut::<cmsghdr>()
        } else {
            _CMSG_HDR_ALIGN(cmsg as usize + (*cmsg).cmsg_len as usize) as *mut cmsghdr
        }
    }

    pub {const} fn CMSG_SPACE(length: c_uint) -> c_uint {
        _CMSG_HDR_ALIGN(size_of::<cmsghdr>() as usize + length as usize) as c_uint
    }

    pub fn FD_CLR(fd: c_int, set: *mut fd_set) -> () {
        let bits = size_of_val(&(*set).fds_bits[0]) * 8;
        let fd = fd as usize;
        (*set).fds_bits[fd / bits] &= !(1 << (fd % bits));
        return;
    }

    pub fn FD_ISSET(fd: c_int, set: *const fd_set) -> bool {
        let bits = size_of_val(&(*set).fds_bits[0]) * 8;
        let fd = fd as usize;
        return ((*set).fds_bits[fd / bits] & (1 << (fd % bits))) != 0;
    }

    pub fn FD_SET(fd: c_int, set: *mut fd_set) -> () {
        let bits = size_of_val(&(*set).fds_bits[0]) * 8;
        let fd = fd as usize;
        (*set).fds_bits[fd / bits] |= 1 << (fd % bits);
        return;
    }

    pub fn FD_ZERO(set: *mut fd_set) -> () {
        for slot in (*set).fds_bits.iter_mut() {
            *slot = 0;
        }
    }
}

safe_f! {
    pub fn SIGRTMAX() -> c_int {
        unsafe { crate::sysconf(_SC_SIGRT_MAX) as c_int }
    }

    pub fn SIGRTMIN() -> c_int {
        unsafe { crate::sysconf(_SC_SIGRT_MIN) as c_int }
    }

    pub {const} fn WIFEXITED(status: c_int) -> bool {
        (status & 0xFF) == 0
    }

    pub {const} fn WEXITSTATUS(status: c_int) -> c_int {
        (status >> 8) & 0xFF
    }

    pub {const} fn WTERMSIG(status: c_int) -> c_int {
        status & 0x7F
    }

    pub {const} fn WIFCONTINUED(status: c_int) -> bool {
        (status & 0xffff) == 0xffff
    }

    pub {const} fn WSTOPSIG(status: c_int) -> c_int {
        (status & 0xff00) >> 8
    }

    pub {const} fn WIFSIGNALED(status: c_int) -> bool {
        ((status & 0xff) > 0) && (status & 0xff00 == 0)
    }

    pub {const} fn WIFSTOPPED(status: c_int) -> bool {
        ((status & 0xff) == 0x7f) && ((status & 0xff00) != 0)
    }

    pub {const} fn WCOREDUMP(status: c_int) -> bool {
        (status & 0x80) != 0
    }

    pub {const} fn MR_GET_TYPE(flags: c_uint) -> c_uint {
        flags & 0x0000ffff
    }
}

extern "C" {
    pub fn getrlimit(resource: c_int, rlim: *mut crate::rlimit) -> c_int;
    pub fn setrlimit(resource: c_int, rlim: *const crate::rlimit) -> c_int;

    pub fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;

    pub fn sem_destroy(sem: *mut sem_t) -> c_int;
    pub fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;

    pub fn abs(i: c_int) -> c_int;
    pub fn acct(filename: *const c_char) -> c_int;
    pub fn dirfd(dirp: *mut crate::DIR) -> c_int;
    pub fn labs(i: c_long) -> c_long;
    pub fn rand() -> c_int;
    pub fn srand(seed: c_uint);
    pub fn getentropy(buf: *mut c_void, buflen: size_t) -> c_int;
    pub fn getrandom(bbuf: *mut c_void, buflen: size_t, flags: c_uint) -> ssize_t;

    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut c_void) -> c_int;
    pub fn settimeofday(tp: *const crate::timeval, tz: *const c_void) -> c_int;
    pub fn getifaddrs(ifap: *mut *mut crate::ifaddrs) -> c_int;
    pub fn freeifaddrs(ifa: *mut crate::ifaddrs);

    pub fn stack_getbounds(sp: *mut crate::stack_t) -> c_int;
    pub fn getgrouplist(
        name: *const c_char,
        basegid: crate::gid_t,
        groups: *mut crate::gid_t,
        ngroups: *mut c_int,
    ) -> c_int;
    pub fn initgroups(name: *const c_char, basegid: crate::gid_t) -> c_int;
    pub fn setgroups(ngroups: c_int, ptr: *const crate::gid_t) -> c_int;
    pub fn ioctl(fildes: c_int, request: c_int, ...) -> c_int;
    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;
    pub fn ___errno() -> *mut c_int;
    pub fn clock_getres(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn clock_gettime(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn clock_nanosleep(
        clk_id: crate::clockid_t,
        flags: c_int,
        rqtp: *const crate::timespec,
        rmtp: *mut crate::timespec,
    ) -> c_int;
    pub fn clock_settime(clk_id: crate::clockid_t, tp: *const crate::timespec) -> c_int;
    pub fn getnameinfo(
        sa: *const crate::sockaddr,
        salen: crate::socklen_t,
        host: *mut c_char,
        hostlen: crate::socklen_t,
        serv: *mut c_char,
        servlen: crate::socklen_t,
        flags: c_int,
    ) -> c_int;
    pub fn setpwent();
    pub fn endpwent();
    pub fn getpwent() -> *mut passwd;
    pub fn fdatasync(fd: c_int) -> c_int;
    pub fn nl_langinfo_l(item: crate::nl_item, locale: crate::locale_t) -> *mut c_char;
    pub fn duplocale(base: crate::locale_t) -> crate::locale_t;
    pub fn freelocale(loc: crate::locale_t);
    pub fn newlocale(mask: c_int, locale: *const c_char, base: crate::locale_t) -> crate::locale_t;
    pub fn uselocale(loc: crate::locale_t) -> crate::locale_t;
    pub fn getprogname() -> *const c_char;
    pub fn setprogname(name: *const c_char);
    pub fn getloadavg(loadavg: *mut c_double, nelem: c_int) -> c_int;
    pub fn getpriority(which: c_int, who: c_int) -> c_int;
    pub fn setpriority(which: c_int, who: c_int, prio: c_int) -> c_int;

    pub fn mknodat(dirfd: c_int, pathname: *const c_char, mode: mode_t, dev: dev_t) -> c_int;
    pub fn mkfifoat(dirfd: c_int, pathname: *const c_char, mode: mode_t) -> c_int;
    pub fn sethostname(name: *const c_char, len: c_int) -> c_int;
    pub fn if_nameindex() -> *mut if_nameindex;
    pub fn if_freenameindex(ptr: *mut if_nameindex);
    pub fn pthread_create(
        native: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        f: extern "C" fn(*mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;
    pub fn pthread_attr_getstack(
        attr: *const crate::pthread_attr_t,
        stackaddr: *mut *mut c_void,
        stacksize: *mut size_t,
    ) -> c_int;
    pub fn pthread_condattr_getclock(
        attr: *const pthread_condattr_t,
        clock_id: *mut clockid_t,
    ) -> c_int;
    pub fn pthread_condattr_setclock(
        attr: *mut pthread_condattr_t,
        clock_id: crate::clockid_t,
    ) -> c_int;
    pub fn sem_timedwait(sem: *mut sem_t, abstime: *const crate::timespec) -> c_int;
    pub fn sem_getvalue(sem: *mut sem_t, sval: *mut c_int) -> c_int;
    pub fn pthread_mutex_timedlock(
        lock: *mut pthread_mutex_t,
        abstime: *const crate::timespec,
    ) -> c_int;
    pub fn pthread_getname_np(tid: crate::pthread_t, name: *mut c_char, len: size_t) -> c_int;
    pub fn pthread_setname_np(tid: crate::pthread_t, name: *const c_char) -> c_int;
    pub fn waitid(
        idtype: idtype_t,
        id: id_t,
        infop: *mut crate::siginfo_t,
        options: c_int,
    ) -> c_int;

    #[cfg_attr(target_os = "illumos", link_name = "_glob_ext")]
    pub fn glob(
        pattern: *const c_char,
        flags: c_int,
        errfunc: Option<extern "C" fn(epath: *const c_char, errno: c_int) -> c_int>,
        pglob: *mut crate::glob_t,
    ) -> c_int;

    #[cfg_attr(target_os = "illumos", link_name = "_globfree_ext")]
    pub fn globfree(pglob: *mut crate::glob_t);

    pub fn posix_fallocate(fd: c_int, offset: off_t, len: off_t) -> c_int;
    pub fn posix_madvise(addr: *mut c_void, len: size_t, advice: c_int) -> c_int;

    pub fn posix_spawn(
        pid: *mut crate::pid_t,
        path: *const c_char,
        file_actions: *const posix_spawn_file_actions_t,
        attrp: *const posix_spawnattr_t,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
    ) -> c_int;
    pub fn posix_spawnp(
        pid: *mut crate::pid_t,
        file: *const c_char,
        file_actions: *const posix_spawn_file_actions_t,
        attrp: *const posix_spawnattr_t,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
    ) -> c_int;

    pub fn posix_spawn_file_actions_init(file_actions: *mut posix_spawn_file_actions_t) -> c_int;
    pub fn posix_spawn_file_actions_destroy(file_actions: *mut posix_spawn_file_actions_t)
        -> c_int;
    pub fn posix_spawn_file_actions_addopen(
        file_actions: *mut posix_spawn_file_actions_t,
        fildes: c_int,
        path: *const c_char,
        oflag: c_int,
        mode: mode_t,
    ) -> c_int;
    pub fn posix_spawn_file_actions_addclose(
        file_actions: *mut posix_spawn_file_actions_t,
        fildes: c_int,
    ) -> c_int;
    pub fn posix_spawn_file_actions_adddup2(
        file_actions: *mut posix_spawn_file_actions_t,
        fildes: c_int,
        newfildes: c_int,
    ) -> c_int;
    pub fn posix_spawn_file_actions_addclosefrom_np(
        file_actions: *mut posix_spawn_file_actions_t,
        lowfiledes: c_int,
    ) -> c_int;
    pub fn posix_spawn_file_actions_addchdir(
        file_actions: *mut posix_spawn_file_actions_t,
        path: *const c_char,
    ) -> c_int;
    pub fn posix_spawn_file_actions_addchdir_np(
        file_actions: *mut posix_spawn_file_actions_t,
        path: *const c_char,
    ) -> c_int;
    pub fn posix_spawn_file_actions_addfchdir(
        file_actions: *mut posix_spawn_file_actions_t,
        fd: c_int,
    ) -> c_int;

    pub fn posix_spawnattr_init(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_destroy(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_setflags(attr: *mut posix_spawnattr_t, flags: c_short) -> c_int;
    pub fn posix_spawnattr_getflags(attr: *const posix_spawnattr_t, flags: *mut c_short) -> c_int;
    pub fn posix_spawnattr_setpgroup(attr: *mut posix_spawnattr_t, pgroup: crate::pid_t) -> c_int;
    pub fn posix_spawnattr_getpgroup(
        attr: *const posix_spawnattr_t,
        _pgroup: *mut crate::pid_t,
    ) -> c_int;
    pub fn posix_spawnattr_setschedparam(
        attr: *mut posix_spawnattr_t,
        param: *const crate::sched_param,
    ) -> c_int;
    pub fn posix_spawnattr_getschedparam(
        attr: *const posix_spawnattr_t,
        param: *mut crate::sched_param,
    ) -> c_int;
    pub fn posix_spawnattr_setschedpolicy(attr: *mut posix_spawnattr_t, policy: c_int) -> c_int;
    pub fn posix_spawnattr_getschedpolicy(
        attr: *const posix_spawnattr_t,
        _policy: *mut c_int,
    ) -> c_int;
    pub fn posix_spawnattr_setsigdefault(
        attr: *mut posix_spawnattr_t,
        sigdefault: *const sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_getsigdefault(
        attr: *const posix_spawnattr_t,
        sigdefault: *mut sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_setsigignore_np(
        attr: *mut posix_spawnattr_t,
        sigignore: *const sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_getsigignore_np(
        attr: *const posix_spawnattr_t,
        sigignore: *mut sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_setsigmask(
        attr: *mut posix_spawnattr_t,
        sigmask: *const sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_getsigmask(
        attr: *const posix_spawnattr_t,
        sigmask: *mut sigset_t,
    ) -> c_int;

    pub fn shmat(shmid: c_int, shmaddr: *const c_void, shmflg: c_int) -> *mut c_void;

    pub fn shmctl(shmid: c_int, cmd: c_int, buf: *mut crate::shmid_ds) -> c_int;

    pub fn shmdt(shmaddr: *const c_void) -> c_int;

    pub fn shmget(key: key_t, size: size_t, shmflg: c_int) -> c_int;

    pub fn shm_open(name: *const c_char, oflag: c_int, mode: mode_t) -> c_int;
    pub fn shm_unlink(name: *const c_char) -> c_int;

    pub fn seekdir(dirp: *mut crate::DIR, loc: c_long);

    pub fn telldir(dirp: *mut crate::DIR) -> c_long;
    pub fn madvise(addr: *mut c_void, len: size_t, advice: c_int) -> c_int;

    pub fn msync(addr: *mut c_void, len: size_t, flags: c_int) -> c_int;

    pub fn memalign(align: size_t, size: size_t) -> *mut c_void;

    pub fn recvfrom(
        socket: c_int,
        buf: *mut c_void,
        len: size_t,
        flags: c_int,
        addr: *mut crate::sockaddr,
        addrlen: *mut crate::socklen_t,
    ) -> ssize_t;
    pub fn mkstemps(template: *mut c_char, suffixlen: c_int) -> c_int;
    pub fn futimesat(fd: c_int, path: *const c_char, times: *const crate::timeval) -> c_int;
    pub fn futimens(dirfd: c_int, times: *const crate::timespec) -> c_int;
    pub fn utimensat(
        dirfd: c_int,
        path: *const c_char,
        times: *const crate::timespec,
        flag: c_int,
    ) -> c_int;
    pub fn nl_langinfo(item: crate::nl_item) -> *mut c_char;

    #[link_name = "__xnet_bind"]
    pub fn bind(
        socket: c_int,
        address: *const crate::sockaddr,
        address_len: crate::socklen_t,
    ) -> c_int;

    pub fn writev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;
    pub fn readv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;

    #[link_name = "__xnet_sendmsg"]
    pub fn sendmsg(fd: c_int, msg: *const crate::msghdr, flags: c_int) -> ssize_t;
    #[link_name = "__xnet_recvmsg"]
    pub fn recvmsg(fd: c_int, msg: *mut crate::msghdr, flags: c_int) -> ssize_t;
    pub fn accept4(
        fd: c_int,
        address: *mut sockaddr,
        address_len: *mut socklen_t,
        flags: c_int,
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
    pub fn port_create() -> c_int;
    pub fn port_associate(
        port: c_int,
        source: c_int,
        object: crate::uintptr_t,
        events: c_int,
        user: *mut c_void,
    ) -> c_int;
    pub fn port_dissociate(port: c_int, source: c_int, object: crate::uintptr_t) -> c_int;
    pub fn port_get(port: c_int, pe: *mut port_event, timeout: *mut crate::timespec) -> c_int;
    pub fn port_getn(
        port: c_int,
        pe_list: *mut port_event,
        max: c_uint,
        nget: *mut c_uint,
        timeout: *mut crate::timespec,
    ) -> c_int;
    pub fn port_send(port: c_int, events: c_int, user: *mut c_void) -> c_int;
    pub fn port_sendn(
        port_list: *mut c_int,
        error_list: *mut c_int,
        nent: c_uint,
        events: c_int,
        user: *mut c_void,
    ) -> c_int;
    #[cfg_attr(
        any(target_os = "solaris", target_os = "illumos"),
        link_name = "__posix_getgrgid_r"
    )]
    pub fn getgrgid_r(
        gid: crate::gid_t,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn sigaltstack(ss: *const stack_t, oss: *mut stack_t) -> c_int;
    pub fn sigsuspend(mask: *const crate::sigset_t) -> c_int;
    pub fn sem_close(sem: *mut sem_t) -> c_int;
    pub fn getdtablesize() -> c_int;

    #[cfg_attr(
        any(target_os = "solaris", target_os = "illumos"),
        link_name = "__posix_getgrnam_r"
    )]
    pub fn getgrnam_r(
        name: *const c_char,
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut crate::group,
    ) -> c_int;
    pub fn thr_self() -> crate::thread_t;
    pub fn pthread_sigmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t) -> c_int;
    pub fn sem_open(name: *const c_char, oflag: c_int, ...) -> *mut sem_t;
    pub fn getgrnam(name: *const c_char) -> *mut crate::group;
    #[cfg_attr(target_os = "solaris", link_name = "__pthread_kill_xpg7")]
    pub fn pthread_kill(thread: crate::pthread_t, sig: c_int) -> c_int;
    pub fn sched_get_priority_min(policy: c_int) -> c_int;
    pub fn sched_get_priority_max(policy: c_int) -> c_int;
    pub fn sched_getparam(pid: crate::pid_t, param: *mut sched_param) -> c_int;
    pub fn sched_setparam(pid: crate::pid_t, param: *const sched_param) -> c_int;
    pub fn sched_getscheduler(pid: crate::pid_t) -> c_int;
    pub fn sched_setscheduler(
        pid: crate::pid_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;
    pub fn sem_unlink(name: *const c_char) -> c_int;
    pub fn daemon(nochdir: c_int, noclose: c_int) -> c_int;
    #[cfg_attr(
        any(target_os = "solaris", target_os = "illumos"),
        link_name = "__posix_getpwnam_r"
    )]
    pub fn getpwnam_r(
        name: *const c_char,
        pwd: *mut passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut passwd,
    ) -> c_int;
    #[cfg_attr(
        any(target_os = "solaris", target_os = "illumos"),
        link_name = "__posix_getpwuid_r"
    )]
    pub fn getpwuid_r(
        uid: crate::uid_t,
        pwd: *mut passwd,
        buf: *mut c_char,
        buflen: size_t,
        result: *mut *mut passwd,
    ) -> c_int;
    #[cfg_attr(
        any(target_os = "solaris", target_os = "illumos"),
        link_name = "getpwent_r"
    )]
    fn native_getpwent_r(pwd: *mut passwd, buf: *mut c_char, buflen: c_int) -> *mut passwd;
    #[cfg_attr(
        any(target_os = "solaris", target_os = "illumos"),
        link_name = "getgrent_r"
    )]
    fn native_getgrent_r(
        grp: *mut crate::group,
        buf: *mut c_char,
        buflen: c_int,
    ) -> *mut crate::group;
    #[cfg_attr(
        any(target_os = "solaris", target_os = "illumos"),
        link_name = "__posix_sigwait"
    )]
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
    pub fn popen(command: *const c_char, mode: *const c_char) -> *mut crate::FILE;

    pub fn dup3(src: c_int, dst: c_int, flags: c_int) -> c_int;
    pub fn uname(buf: *mut crate::utsname) -> c_int;
    pub fn pipe2(fds: *mut c_int, flags: c_int) -> c_int;

    pub fn makeutx(ux: *const utmpx) -> *mut utmpx;
    pub fn modutx(ux: *const utmpx) -> *mut utmpx;
    pub fn updwtmpx(file: *const c_char, ut: *mut utmpx);
    pub fn utmpxname(file: *const c_char) -> c_int;
    pub fn getutxent() -> *mut utmpx;
    pub fn getutxid(ut: *const utmpx) -> *mut utmpx;
    pub fn getutxline(ut: *const utmpx) -> *mut utmpx;
    pub fn pututxline(ut: *const utmpx) -> *mut utmpx;
    pub fn setutxent();
    pub fn endutxent();

    pub fn endutent();
    pub fn getutent() -> *mut utmp;
    pub fn getutid(u: *const utmp) -> *mut utmp;
    pub fn getutline(u: *const utmp) -> *mut utmp;
    pub fn pututline(u: *const utmp) -> *mut utmp;
    pub fn setutent();
    pub fn utmpname(file: *const c_char) -> c_int;

    pub fn getutmp(ux: *const utmpx, u: *mut utmp);
    pub fn getutmpx(u: *const utmp, ux: *mut utmpx);
    pub fn updwtmp(file: *const c_char, u: *mut utmp);

    pub fn ntp_adjtime(buf: *mut timex) -> c_int;
    pub fn ntp_gettime(buf: *mut ntptimeval) -> c_int;

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

    pub fn ucred_get(pid: crate::pid_t) -> *mut ucred_t;
    pub fn getpeerucred(fd: c_int, ucred: *mut *mut ucred_t) -> c_int;

    pub fn ucred_free(ucred: *mut ucred_t);

    pub fn ucred_geteuid(ucred: *const ucred_t) -> crate::uid_t;
    pub fn ucred_getruid(ucred: *const ucred_t) -> crate::uid_t;
    pub fn ucred_getsuid(ucred: *const ucred_t) -> crate::uid_t;
    pub fn ucred_getegid(ucred: *const ucred_t) -> crate::gid_t;
    pub fn ucred_getrgid(ucred: *const ucred_t) -> crate::gid_t;
    pub fn ucred_getsgid(ucred: *const ucred_t) -> crate::gid_t;
    pub fn ucred_getgroups(ucred: *const ucred_t, groups: *mut *const crate::gid_t) -> c_int;
    pub fn ucred_getpid(ucred: *const ucred_t) -> crate::pid_t;
    pub fn ucred_getprojid(ucred: *const ucred_t) -> projid_t;
    pub fn ucred_getzoneid(ucred: *const ucred_t) -> zoneid_t;
    pub fn ucred_getpflags(ucred: *const ucred_t, flags: c_uint) -> c_uint;

    pub fn ucred_size() -> size_t;

    pub fn pset_create(newpset: *mut crate::psetid_t) -> c_int;
    pub fn pset_destroy(pset: crate::psetid_t) -> c_int;
    pub fn pset_assign(
        pset: crate::psetid_t,
        cpu: crate::processorid_t,
        opset: *mut psetid_t,
    ) -> c_int;
    pub fn pset_info(
        pset: crate::psetid_t,
        tpe: *mut c_int,
        numcpus: *mut c_uint,
        cpulist: *mut processorid_t,
    ) -> c_int;
    pub fn pset_bind(
        pset: crate::psetid_t,
        idtype: crate::idtype_t,
        id: crate::id_t,
        opset: *mut psetid_t,
    ) -> c_int;
    pub fn pset_list(pset: *mut psetid_t, numpsets: *mut c_uint) -> c_int;
    pub fn pset_setattr(pset: psetid_t, attr: c_uint) -> c_int;
    pub fn pset_getattr(pset: psetid_t, attr: *mut c_uint) -> c_int;
    pub fn processor_bind(
        idtype: crate::idtype_t,
        id: crate::id_t,
        new_binding: crate::processorid_t,
        old_binding: *mut processorid_t,
    ) -> c_int;
    pub fn p_online(processorid: crate::processorid_t, flag: c_int) -> c_int;
    pub fn processor_info(processorid: crate::processorid_t, infop: *mut processor_info_t)
        -> c_int;

    pub fn getexecname() -> *const c_char;

    pub fn gethostid() -> c_long;

    pub fn getpflags(flags: c_uint) -> c_uint;
    pub fn setpflags(flags: c_uint, value: c_uint) -> c_int;

    pub fn sysinfo(command: c_int, buf: *mut c_char, count: c_long) -> c_int;

    pub fn faccessat(fd: c_int, path: *const c_char, amode: c_int, flag: c_int) -> c_int;

    // #include <link.h>
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn dl_iterate_phdr(
        callback: Option<
            unsafe extern "C" fn(info: *mut dl_phdr_info, size: usize, data: *mut c_void) -> c_int,
        >,
        data: *mut c_void,
    ) -> c_int;
    pub fn getpagesize() -> c_int;
    pub fn getpagesizes(pagesize: *mut size_t, nelem: c_int) -> c_int;
    pub fn mmapobj(
        fd: c_int,
        flags: c_uint,
        storage: *mut mmapobj_result_t,
        elements: *mut c_uint,
        arg: *mut c_void,
    ) -> c_int;
    pub fn meminfo(
        inaddr: *const u64,
        addr_count: c_int,
        info_req: *const c_uint,
        info_count: c_int,
        outdata: *mut u64,
        validity: *mut c_uint,
    ) -> c_int;

    pub fn strsep(string: *mut *mut c_char, delim: *const c_char) -> *mut c_char;

    pub fn getisax(array: *mut u32, n: c_uint) -> c_uint;

    pub fn backtrace(buffer: *mut *mut c_void, size: c_int) -> c_int;
    pub fn backtrace_symbols(buffer: *const *mut c_void, size: c_int) -> *mut *mut c_char;
    pub fn backtrace_symbols_fd(buffer: *const *mut c_void, size: c_int, fd: c_int);

    pub fn getopt_long(
        argc: c_int,
        argv: *const *mut c_char,
        optstring: *const c_char,
        longopts: *const option,
        longindex: *mut c_int,
    ) -> c_int;

    pub fn sync();

    pub fn aio_cancel(fd: c_int, aiocbp: *mut aiocb) -> c_int;
    pub fn aio_error(aiocbp: *const aiocb) -> c_int;
    pub fn aio_fsync(op: c_int, aiocbp: *mut aiocb) -> c_int;
    pub fn aio_read(aiocbp: *mut aiocb) -> c_int;
    pub fn aio_return(aiocbp: *mut aiocb) -> ssize_t;
    pub fn aio_suspend(
        aiocb_list: *const *const aiocb,
        nitems: c_int,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn aio_waitn(
        aiocb_list: *mut *mut aiocb,
        nent: c_uint,
        nwait: *mut c_uint,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn aio_write(aiocbp: *mut aiocb) -> c_int;
    pub fn lio_listio(
        mode: c_int,
        aiocb_list: *const *mut aiocb,
        nitems: c_int,
        sevp: *mut sigevent,
    ) -> c_int;

    pub fn __major(version: c_int, devnum: crate::dev_t) -> crate::major_t;
    pub fn __minor(version: c_int, devnum: crate::dev_t) -> crate::minor_t;
    pub fn __makedev(
        version: c_int,
        majdev: crate::major_t,
        mindev: crate::minor_t,
    ) -> crate::dev_t;

    pub fn arc4random() -> u32;
    pub fn arc4random_buf(buf: *mut c_void, nbytes: size_t);
    pub fn arc4random_uniform(upper_bound: u32) -> u32;

    pub fn secure_getenv(name: *const c_char) -> *mut c_char;

    #[cfg_attr(target_os = "solaris", link_name = "__strftime_xpg7")]
    pub fn strftime(
        s: *mut c_char,
        maxsize: size_t,
        format: *const c_char,
        timeptr: *const crate::tm,
    ) -> size_t;
    pub fn strftime_l(
        s: *mut c_char,
        maxsize: size_t,
        format: *const c_char,
        timeptr: *const crate::tm,
        loc: crate::locale_t,
    ) -> size_t;
}

#[link(name = "sendfile")]
extern "C" {
    pub fn sendfile(out_fd: c_int, in_fd: c_int, off: *mut off_t, len: size_t) -> ssize_t;
    pub fn sendfilev(
        fildes: c_int,
        vec: *const sendfilevec_t,
        sfvcnt: c_int,
        xferred: *mut size_t,
    ) -> ssize_t;
}

#[link(name = "lgrp")]
extern "C" {
    pub fn lgrp_init(view: lgrp_view_t) -> lgrp_cookie_t;
    pub fn lgrp_fini(cookie: lgrp_cookie_t) -> c_int;
    pub fn lgrp_affinity_get(
        idtype: crate::idtype_t,
        id: crate::id_t,
        lgrp: crate::lgrp_id_t,
    ) -> crate::lgrp_affinity_t;
    pub fn lgrp_affinity_set(
        idtype: crate::idtype_t,
        id: crate::id_t,
        lgrp: crate::lgrp_id_t,
        aff: lgrp_affinity_t,
    ) -> c_int;
    pub fn lgrp_cpus(
        cookie: crate::lgrp_cookie_t,
        lgrp: crate::lgrp_id_t,
        cpuids: *mut crate::processorid_t,
        count: c_uint,
        content: crate::lgrp_content_t,
    ) -> c_int;
    pub fn lgrp_mem_size(
        cookie: crate::lgrp_cookie_t,
        lgrp: crate::lgrp_id_t,
        tpe: crate::lgrp_mem_size_flag_t,
        content: crate::lgrp_content_t,
    ) -> crate::lgrp_mem_size_t;
    pub fn lgrp_nlgrps(cookie: crate::lgrp_cookie_t) -> c_int;
    pub fn lgrp_view(cookie: crate::lgrp_cookie_t) -> crate::lgrp_view_t;
    pub fn lgrp_home(idtype: crate::idtype_t, id: crate::id_t) -> crate::lgrp_id_t;
    pub fn lgrp_version(version: c_int) -> c_int;
    pub fn lgrp_resources(
        cookie: crate::lgrp_cookie_t,
        lgrp: crate::lgrp_id_t,
        lgrps: *mut crate::lgrp_id_t,
        count: c_uint,
        tpe: crate::lgrp_rsrc_t,
    ) -> c_int;
    pub fn lgrp_root(cookie: crate::lgrp_cookie_t) -> crate::lgrp_id_t;
}

pub unsafe fn major(device: crate::dev_t) -> crate::major_t {
    __major(NEWDEV, device)
}

pub unsafe fn minor(device: crate::dev_t) -> crate::minor_t {
    __minor(NEWDEV, device)
}

pub unsafe fn makedev(maj: crate::major_t, min: crate::minor_t) -> crate::dev_t {
    __makedev(NEWDEV, maj, min)
}

mod compat;
pub use self::compat::*;

cfg_if! {
    if #[cfg(target_os = "illumos")] {
        mod illumos;
        pub use self::illumos::*;
    } else if #[cfg(target_os = "solaris")] {
        mod solaris;
        pub use self::solaris::*;
    } else {
        // Unknown target_os
    }
}

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        mod x86_common;
        pub use self::x86_64::*;
        pub use self::x86_common::*;
    } else if #[cfg(target_arch = "x86")] {
        mod x86;
        mod x86_common;
        pub use self::x86::*;
        pub use self::x86_common::*;
    }
}
