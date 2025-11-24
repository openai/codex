use crate::prelude::*;

pub type clock_t = u32;

pub type sa_family_t = u8;
pub type speed_t = c_uint;
pub type tcflag_t = c_uint;
pub type clockid_t = c_int;
pub type timer_t = c_int;
pub type key_t = c_uint;
pub type id_t = c_int;

pub type useconds_t = u32;
pub type dev_t = u32;
pub type socklen_t = u32;
pub type mode_t = u32;
pub type rlim64_t = u64;
pub type mqd_t = c_int;
pub type nfds_t = c_uint;
pub type idtype_t = c_uint;
pub type errno_t = c_int;
pub type rsize_t = c_ulong;

pub type Elf32_Half = u16;
pub type Elf32_Word = u32;
pub type Elf32_Off = u32;
pub type Elf32_Addr = u32;
pub type Elf32_Lword = u64;
pub type Elf32_Sword = i32;

pub type Elf64_Half = u16;
pub type Elf64_Word = u32;
pub type Elf64_Off = u64;
pub type Elf64_Addr = u64;
pub type Elf64_Xword = u64;
pub type Elf64_Sxword = i64;
pub type Elf64_Lword = u64;
pub type Elf64_Sword = i32;

pub type Elf32_Section = u16;
pub type Elf64_Section = u16;

pub type _Time32t = u32;

pub type pthread_t = c_int;
pub type regoff_t = ssize_t;

pub type nlink_t = u32;
pub type blksize_t = u32;
pub type suseconds_t = i32;

pub type ino_t = u64;
pub type off_t = i64;
pub type blkcnt_t = u64;
pub type msgqnum_t = u64;
pub type msglen_t = u64;
pub type fsblkcnt_t = u64;
pub type fsfilcnt_t = u64;
pub type rlim_t = u64;
pub type posix_spawn_file_actions_t = *mut c_void;
pub type posix_spawnattr_t = crate::uintptr_t;

pub type pthread_mutex_t = crate::sync_t;
pub type pthread_mutexattr_t = crate::_sync_attr;
pub type pthread_cond_t = crate::sync_t;
pub type pthread_condattr_t = crate::_sync_attr;
pub type pthread_rwlockattr_t = crate::_sync_attr;
pub type pthread_key_t = c_int;
pub type pthread_spinlock_t = sync_t;
pub type pthread_barrierattr_t = _sync_attr;
pub type sem_t = sync_t;

pub type nl_item = c_int;

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum timezone {}
impl Copy for timezone {}
impl Clone for timezone {
    fn clone(&self) -> timezone {
        *self
    }
}

s! {
    pub struct dirent_extra {
        pub d_datalen: u16,
        pub d_type: u16,
        pub d_reserved: u32,
    }

    pub struct stat {
        pub st_ino: crate::ino_t,
        pub st_size: off_t,
        pub st_dev: crate::dev_t,
        pub st_rdev: crate::dev_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub __old_st_mtime: crate::_Time32t,
        pub __old_st_atime: crate::_Time32t,
        pub __old_st_ctime: crate::_Time32t,
        pub st_mode: mode_t,
        pub st_nlink: crate::nlink_t,
        pub st_blocksize: crate::blksize_t,
        pub st_nblocks: i32,
        pub st_blksize: crate::blksize_t,
        pub st_blocks: crate::blkcnt_t,
        pub st_mtim: crate::timespec,
        pub st_atim: crate::timespec,
        pub st_ctim: crate::timespec,
    }

    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    #[cfg_attr(any(target_env = "nto71", target_env = "nto70"), repr(packed))]
    pub struct in_addr {
        pub s_addr: crate::in_addr_t,
    }

    pub struct sockaddr {
        pub sa_len: u8,
        pub sa_family: sa_family_t,
        pub sa_data: [c_char; 14],
    }

    #[cfg(not(target_env = "nto71_iosock"))]
    pub struct sockaddr_in {
        pub sin_len: u8,
        pub sin_family: sa_family_t,
        pub sin_port: crate::in_port_t,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [i8; 8],
    }

    #[cfg(target_env = "nto71_iosock")]
    pub struct sockaddr_in {
        pub sin_len: u8,
        pub sin_family: sa_family_t,
        pub sin_port: crate::in_port_t,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [c_char; 8],
    }

    pub struct sockaddr_in6 {
        pub sin6_len: u8,
        pub sin6_family: sa_family_t,
        pub sin6_port: crate::in_port_t,
        pub sin6_flowinfo: u32,
        pub sin6_addr: crate::in6_addr,
        pub sin6_scope_id: u32,
    }

    // The order of the `ai_addr` field in this struct is crucial
    // for converting between the Rust and C types.
    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        pub ai_addrlen: socklen_t,
        pub ai_canonname: *mut c_char,
        pub ai_addr: *mut crate::sockaddr,
        pub ai_next: *mut addrinfo,
    }

    pub struct fd_set {
        fds_bits: [c_uint; 2 * FD_SETSIZE as usize / ULONG_SIZE],
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

    #[repr(align(8))]
    pub struct sched_param {
        pub sched_priority: c_int,
        pub sched_curpriority: c_int,
        pub reserved: [c_int; 10],
    }

    #[repr(align(8))]
    pub struct __sched_param {
        pub __sched_priority: c_int,
        pub __sched_curpriority: c_int,
        pub reserved: [c_int; 10],
    }

    pub struct Dl_info {
        pub dli_fname: *const c_char,
        pub dli_fbase: *mut c_void,
        pub dli_sname: *const c_char,
        pub dli_saddr: *mut c_void,
    }

    pub struct lconv {
        pub currency_symbol: *mut c_char,
        pub int_curr_symbol: *mut c_char,
        pub mon_decimal_point: *mut c_char,
        pub mon_grouping: *mut c_char,
        pub mon_thousands_sep: *mut c_char,
        pub negative_sign: *mut c_char,
        pub positive_sign: *mut c_char,
        pub frac_digits: c_char,
        pub int_frac_digits: c_char,
        pub n_cs_precedes: c_char,
        pub n_sep_by_space: c_char,
        pub n_sign_posn: c_char,
        pub p_cs_precedes: c_char,
        pub p_sep_by_space: c_char,
        pub p_sign_posn: c_char,

        pub int_n_cs_precedes: c_char,
        pub int_n_sep_by_space: c_char,
        pub int_n_sign_posn: c_char,
        pub int_p_cs_precedes: c_char,
        pub int_p_sep_by_space: c_char,
        pub int_p_sign_posn: c_char,

        pub decimal_point: *mut c_char,
        pub grouping: *mut c_char,
        pub thousands_sep: *mut c_char,

        pub _Frac_grouping: *mut c_char,
        pub _Frac_sep: *mut c_char,
        pub _False: *mut c_char,
        pub _True: *mut c_char,

        pub _No: *mut c_char,
        pub _Yes: *mut c_char,
        pub _Nostr: *mut c_char,
        pub _Yesstr: *mut c_char,
        pub _Reserved: [*mut c_char; 8],
    }

    // Does not exist in io-sock
    #[cfg(not(target_env = "nto71_iosock"))]
    pub struct in_pktinfo {
        pub ipi_addr: crate::in_addr,
        pub ipi_ifindex: c_uint,
    }

    pub struct ifaddrs {
        pub ifa_next: *mut ifaddrs,
        pub ifa_name: *mut c_char,
        pub ifa_flags: c_uint,
        pub ifa_addr: *mut crate::sockaddr,
        pub ifa_netmask: *mut crate::sockaddr,
        pub ifa_dstaddr: *mut crate::sockaddr,
        pub ifa_data: *mut c_void,
    }

    pub struct arpreq {
        pub arp_pa: crate::sockaddr,
        pub arp_ha: crate::sockaddr,
        pub arp_flags: c_int,
    }

    #[cfg_attr(any(target_env = "nto71", target_env = "nto70"), repr(packed))]
    pub struct arphdr {
        pub ar_hrd: u16,
        pub ar_pro: u16,
        pub ar_hln: u8,
        pub ar_pln: u8,
        pub ar_op: u16,
    }

    #[cfg(not(target_env = "nto71_iosock"))]
    pub struct mmsghdr {
        pub msg_hdr: crate::msghdr,
        pub msg_len: c_uint,
    }

    #[cfg(target_env = "nto71_iosock")]
    pub struct mmsghdr {
        pub msg_hdr: crate::msghdr,
        pub msg_len: ssize_t,
    }

    #[repr(align(8))]
    pub struct siginfo_t {
        pub si_signo: c_int,
        pub si_code: c_int,
        pub si_errno: c_int,
        __data: [u8; 36], // union
    }

    pub struct sigaction {
        pub sa_sigaction: crate::sighandler_t,
        pub sa_flags: c_int,
        pub sa_mask: crate::sigset_t,
    }

    pub struct _sync {
        _union: c_uint,
        __owner: c_uint,
    }
    pub struct rlimit64 {
        pub rlim_cur: rlim64_t,
        pub rlim_max: rlim64_t,
    }

    pub struct glob_t {
        pub gl_pathc: size_t,
        pub gl_matchc: c_int,
        pub gl_pathv: *mut *mut c_char,
        pub gl_offs: size_t,
        pub gl_flags: c_int,
        pub gl_errfunc: extern "C" fn(*const c_char, c_int) -> c_int,

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
        pub pw_age: *mut c_char,
        pub pw_comment: *mut c_char,
        pub pw_gecos: *mut c_char,
        pub pw_dir: *mut c_char,
        pub pw_shell: *mut c_char,
    }

    pub struct if_nameindex {
        pub if_index: c_uint,
        pub if_name: *mut c_char,
    }

    pub struct sembuf {
        pub sem_num: c_ushort,
        pub sem_op: c_short,
        pub sem_flg: c_short,
    }

    pub struct Elf32_Ehdr {
        pub e_ident: [c_uchar; 16],
        pub e_type: Elf32_Half,
        pub e_machine: Elf32_Half,
        pub e_version: Elf32_Word,
        pub e_entry: Elf32_Addr,
        pub e_phoff: Elf32_Off,
        pub e_shoff: Elf32_Off,
        pub e_flags: Elf32_Word,
        pub e_ehsize: Elf32_Half,
        pub e_phentsize: Elf32_Half,
        pub e_phnum: Elf32_Half,
        pub e_shentsize: Elf32_Half,
        pub e_shnum: Elf32_Half,
        pub e_shstrndx: Elf32_Half,
    }

    pub struct Elf64_Ehdr {
        pub e_ident: [c_uchar; 16],
        pub e_type: Elf64_Half,
        pub e_machine: Elf64_Half,
        pub e_version: Elf64_Word,
        pub e_entry: Elf64_Addr,
        pub e_phoff: Elf64_Off,
        pub e_shoff: Elf64_Off,
        pub e_flags: Elf64_Word,
        pub e_ehsize: Elf64_Half,
        pub e_phentsize: Elf64_Half,
        pub e_phnum: Elf64_Half,
        pub e_shentsize: Elf64_Half,
        pub e_shnum: Elf64_Half,
        pub e_shstrndx: Elf64_Half,
    }

    pub struct Elf32_Sym {
        pub st_name: Elf32_Word,
        pub st_value: Elf32_Addr,
        pub st_size: Elf32_Word,
        pub st_info: c_uchar,
        pub st_other: c_uchar,
        pub st_shndx: Elf32_Section,
    }

    pub struct Elf64_Sym {
        pub st_name: Elf64_Word,
        pub st_info: c_uchar,
        pub st_other: c_uchar,
        pub st_shndx: Elf64_Section,
        pub st_value: Elf64_Addr,
        pub st_size: Elf64_Xword,
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

    pub struct Elf32_Shdr {
        pub sh_name: Elf32_Word,
        pub sh_type: Elf32_Word,
        pub sh_flags: Elf32_Word,
        pub sh_addr: Elf32_Addr,
        pub sh_offset: Elf32_Off,
        pub sh_size: Elf32_Word,
        pub sh_link: Elf32_Word,
        pub sh_info: Elf32_Word,
        pub sh_addralign: Elf32_Word,
        pub sh_entsize: Elf32_Word,
    }

    pub struct Elf64_Shdr {
        pub sh_name: Elf64_Word,
        pub sh_type: Elf64_Word,
        pub sh_flags: Elf64_Xword,
        pub sh_addr: Elf64_Addr,
        pub sh_offset: Elf64_Off,
        pub sh_size: Elf64_Xword,
        pub sh_link: Elf64_Word,
        pub sh_info: Elf64_Word,
        pub sh_addralign: Elf64_Xword,
        pub sh_entsize: Elf64_Xword,
    }

    pub struct in6_pktinfo {
        pub ipi6_addr: crate::in6_addr,
        pub ipi6_ifindex: c_uint,
    }

    pub struct inotify_event {
        pub wd: c_int,
        pub mask: u32,
        pub cookie: u32,
        pub len: u32,
    }

    pub struct regmatch_t {
        pub rm_so: regoff_t,
        pub rm_eo: regoff_t,
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

    pub struct termios {
        pub c_iflag: crate::tcflag_t,
        pub c_oflag: crate::tcflag_t,
        pub c_cflag: crate::tcflag_t,
        pub c_lflag: crate::tcflag_t,
        pub c_cc: [crate::cc_t; crate::NCCS],
        __reserved: [c_uint; 3],
        pub c_ispeed: crate::speed_t,
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

    pub struct flock {
        pub l_type: i16,
        pub l_whence: i16,
        pub l_zero1: i32,
        pub l_start: off_t,
        pub l_len: off_t,
        pub l_pid: crate::pid_t,
        pub l_sysid: u32,
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
        f_filler: [c_uint; 21],
    }

    pub struct aiocb {
        pub aio_fildes: c_int,
        pub aio_reqprio: c_int,
        pub aio_offset: off_t,
        pub aio_buf: *mut c_void,
        pub aio_nbytes: size_t,
        pub aio_sigevent: crate::sigevent,
        pub aio_lio_opcode: c_int,
        pub _aio_lio_state: *mut c_void,
        _aio_pad: [c_int; 3],
        pub _aio_next: *mut crate::aiocb,
        pub _aio_flag: c_uint,
        pub _aio_iotype: c_uint,
        pub _aio_result: ssize_t,
        pub _aio_error: c_uint,
        pub _aio_suspend: *mut c_void,
        pub _aio_plist: *mut c_void,
        pub _aio_policy: c_int,
        pub _aio_param: crate::__sched_param,
    }

    pub struct pthread_attr_t {
        __data1: c_long,
        __data2: [u8; 96],
    }

    pub struct ipc_perm {
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub cuid: crate::uid_t,
        pub cgid: crate::gid_t,
        pub mode: mode_t,
        pub seq: c_uint,
        pub key: crate::key_t,
        _reserved: [c_int; 4],
    }

    pub struct regex_t {
        re_magic: c_int,
        re_nsub: size_t,
        re_endp: *const c_char,
        re_g: *mut c_void,
    }

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct _thread_attr {
        pub __flags: c_int,
        pub __stacksize: size_t,
        pub __stackaddr: *mut c_void,
        pub __exitfunc: Option<unsafe extern "C" fn(_fake: *mut c_void)>,
        pub __policy: c_int,
        pub __param: crate::__sched_param,
        pub __guardsize: c_uint,
        pub __prealloc: c_uint,
        __spare: [c_int; 2],
    }

    pub struct _sync_attr {
        pub __protocol: c_int,
        pub __flags: c_int,
        pub __prioceiling: c_int,
        pub __clockid: c_int,
        pub __count: c_int,
        __reserved: [c_int; 3],
    }

    pub struct sockcred {
        pub sc_uid: crate::uid_t,
        pub sc_euid: crate::uid_t,
        pub sc_gid: crate::gid_t,
        pub sc_egid: crate::gid_t,
        pub sc_ngroups: c_int,
        pub sc_groups: [crate::gid_t; 1],
    }

    pub struct bpf_program {
        pub bf_len: c_uint,
        pub bf_insns: *mut crate::bpf_insn,
    }

    #[cfg(not(target_env = "nto71_iosock"))]
    pub struct bpf_stat {
        pub bs_recv: u64,
        pub bs_drop: u64,
        pub bs_capt: u64,
        bs_padding: [u64; 13],
    }

    #[cfg(target_env = "nto71_iosock")]
    pub struct bpf_stat {
        pub bs_recv: c_uint,
        pub bs_drop: c_uint,
    }

    pub struct bpf_version {
        pub bv_major: c_ushort,
        pub bv_minor: c_ushort,
    }

    pub struct bpf_hdr {
        pub bh_tstamp: crate::timeval,
        pub bh_caplen: u32,
        pub bh_datalen: u32,
        pub bh_hdrlen: u16,
    }

    pub struct bpf_insn {
        pub code: u16,
        pub jt: c_uchar,
        pub jf: c_uchar,
        pub k: u32,
    }

    pub struct bpf_dltlist {
        pub bfl_len: c_uint,
        pub bfl_list: *mut c_uint,
    }

    // Does not exist in io-sock
    #[cfg(not(target_env = "nto71_iosock"))]
    pub struct unpcbid {
        pub unp_pid: crate::pid_t,
        pub unp_euid: crate::uid_t,
        pub unp_egid: crate::gid_t,
    }

    pub struct dl_phdr_info {
        pub dlpi_addr: crate::Elf64_Addr,
        pub dlpi_name: *const c_char,
        pub dlpi_phdr: *const crate::Elf64_Phdr,
        pub dlpi_phnum: crate::Elf64_Half,
    }

    #[repr(align(8))]
    pub struct ucontext_t {
        pub uc_link: *mut ucontext_t,
        pub uc_sigmask: crate::sigset_t,
        pub uc_stack: stack_t,
        pub uc_mcontext: mcontext_t,
    }
}

s_no_extra_traits! {
    pub struct sockaddr_un {
        pub sun_len: u8,
        pub sun_family: sa_family_t,
        pub sun_path: [c_char; 104],
    }

    pub struct sockaddr_storage {
        pub ss_len: u8,
        pub ss_family: sa_family_t,
        __ss_pad1: [c_char; 6],
        __ss_align: i64,
        __ss_pad2: [c_char; 112],
    }

    pub struct utsname {
        pub sysname: [c_char; _SYSNAME_SIZE],
        pub nodename: [c_char; _SYSNAME_SIZE],
        pub release: [c_char; _SYSNAME_SIZE],
        pub version: [c_char; _SYSNAME_SIZE],
        pub machine: [c_char; _SYSNAME_SIZE],
    }

    pub struct sigevent {
        pub sigev_notify: c_int,
        pub __padding1: c_int,
        pub sigev_signo: c_int, // union
        pub __padding2: c_int,
        pub sigev_value: crate::sigval,
        __sigev_un2: usize, // union
    }
    pub struct dirent {
        pub d_ino: crate::ino_t,
        pub d_offset: off_t,
        pub d_reclen: c_short,
        pub d_namelen: c_short,
        pub d_name: [c_char; 1], // flex array
    }

    pub struct sigset_t {
        __val: [u32; 2],
    }

    pub struct mq_attr {
        pub mq_maxmsg: c_long,
        pub mq_msgsize: c_long,
        pub mq_flags: c_long,
        pub mq_curmsgs: c_long,
        pub mq_sendwait: c_long,
        pub mq_recvwait: c_long,
    }

    pub struct msg {
        pub msg_next: *mut crate::msg,
        pub msg_type: c_long,
        pub msg_ts: c_ushort,
        pub msg_spot: c_short,
        _pad: [u8; 4],
    }

    pub struct msqid_ds {
        pub msg_perm: crate::ipc_perm,
        pub msg_first: *mut crate::msg,
        pub msg_last: *mut crate::msg,
        pub msg_cbytes: crate::msglen_t,
        pub msg_qnum: crate::msgqnum_t,
        pub msg_qbytes: crate::msglen_t,
        pub msg_lspid: crate::pid_t,
        pub msg_lrpid: crate::pid_t,
        pub msg_stime: crate::time_t,
        msg_pad1: c_long,
        pub msg_rtime: crate::time_t,
        msg_pad2: c_long,
        pub msg_ctime: crate::time_t,
        msg_pad3: c_long,
        msg_pad4: [c_long; 4],
    }

    #[cfg(not(target_env = "nto71_iosock"))]
    pub struct sockaddr_dl {
        pub sdl_len: c_uchar,
        pub sdl_family: crate::sa_family_t,
        pub sdl_index: u16,
        pub sdl_type: c_uchar,
        pub sdl_nlen: c_uchar,
        pub sdl_alen: c_uchar,
        pub sdl_slen: c_uchar,
        pub sdl_data: [c_char; 12],
    }

    #[cfg(target_env = "nto71_iosock")]
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

    pub struct sync_t {
        __u: c_uint, // union
        pub __owner: c_uint,
    }

    #[repr(align(4))]
    pub struct pthread_barrier_t {
        // union
        __pad: [u8; 28], // union
    }

    pub struct pthread_rwlock_t {
        pub __active: c_int,
        pub __blockedwriters: c_int,
        pub __blockedreaders: c_int,
        pub __heavy: c_int,
        pub __lock: crate::pthread_mutex_t, // union
        pub __rcond: crate::pthread_cond_t, // union
        pub __wcond: crate::pthread_cond_t, // union
        pub __owner: c_uint,
        pub __spare: c_uint,
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        // sigevent
        impl PartialEq for sigevent {
            fn eq(&self, other: &sigevent) -> bool {
                self.sigev_notify == other.sigev_notify
                    && self.sigev_signo == other.sigev_signo
                    && self.sigev_value == other.sigev_value
                    && self.__sigev_un2 == other.__sigev_un2
            }
        }
        impl Eq for sigevent {}
        impl hash::Hash for sigevent {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sigev_notify.hash(state);
                self.sigev_signo.hash(state);
                self.sigev_value.hash(state);
                self.__sigev_un2.hash(state);
            }
        }

        impl PartialEq for sockaddr_un {
            fn eq(&self, other: &sockaddr_un) -> bool {
                self.sun_len == other.sun_len
                    && self.sun_family == other.sun_family
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
                self.sun_len.hash(state);
                self.sun_family.hash(state);
                self.sun_path.hash(state);
            }
        }

        // sigset_t
        impl PartialEq for sigset_t {
            fn eq(&self, other: &sigset_t) -> bool {
                self.__val == other.__val
            }
        }
        impl Eq for sigset_t {}
        impl hash::Hash for sigset_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.__val.hash(state);
            }
        }

        // msg

        // msqid_ds

        // sockaddr_dl
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

        impl PartialEq for mq_attr {
            fn eq(&self, other: &mq_attr) -> bool {
                self.mq_maxmsg == other.mq_maxmsg
                    && self.mq_msgsize == other.mq_msgsize
                    && self.mq_flags == other.mq_flags
                    && self.mq_curmsgs == other.mq_curmsgs
                    && self.mq_msgsize == other.mq_msgsize
                    && self.mq_sendwait == other.mq_sendwait
                    && self.mq_recvwait == other.mq_recvwait
            }
        }

        impl Eq for mq_attr {}

        impl hash::Hash for mq_attr {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.mq_maxmsg.hash(state);
                self.mq_msgsize.hash(state);
                self.mq_flags.hash(state);
                self.mq_curmsgs.hash(state);
                self.mq_sendwait.hash(state);
                self.mq_recvwait.hash(state);
            }
        }

        impl PartialEq for sockaddr_storage {
            fn eq(&self, other: &sockaddr_storage) -> bool {
                self.ss_len == other.ss_len
                    && self.ss_family == other.ss_family
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
                self.ss_len.hash(state);
                self.ss_family.hash(state);
                self.__ss_pad1.hash(state);
                self.__ss_align.hash(state);
                self.__ss_pad2.hash(state);
            }
        }

        impl PartialEq for dirent {
            fn eq(&self, other: &dirent) -> bool {
                self.d_ino == other.d_ino
                    && self.d_offset == other.d_offset
                    && self.d_reclen == other.d_reclen
                    && self.d_namelen == other.d_namelen
                    && self.d_name[..self.d_namelen as _]
                        .iter()
                        .zip(other.d_name.iter())
                        .all(|(a, b)| a == b)
            }
        }

        impl Eq for dirent {}

        impl hash::Hash for dirent {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.d_ino.hash(state);
                self.d_offset.hash(state);
                self.d_reclen.hash(state);
                self.d_namelen.hash(state);
                self.d_name[..self.d_namelen as _].hash(state);
            }
        }
    }
}

pub const _SYSNAME_SIZE: usize = 256 + 1;
pub const RLIM_INFINITY: crate::rlim_t = 0xfffffffffffffffd;
pub const O_LARGEFILE: c_int = 0o0100000;

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

pub const EXIT_FAILURE: c_int = 1;
pub const EXIT_SUCCESS: c_int = 0;
pub const RAND_MAX: c_int = 32767;
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

pub const F_DUPFD_CLOEXEC: c_int = 5;

pub const SIGTRAP: c_int = 5;

pub const CLOCK_REALTIME: crate::clockid_t = 0;
pub const CLOCK_MONOTONIC: crate::clockid_t = 2;
pub const CLOCK_PROCESS_CPUTIME_ID: crate::clockid_t = 3;
pub const CLOCK_THREAD_CPUTIME_ID: crate::clockid_t = 4;
pub const TIMER_ABSTIME: c_uint = 0x80000000;

pub const RUSAGE_SELF: c_int = 0;

pub const F_OK: c_int = 0;
pub const X_OK: c_int = 1;
pub const W_OK: c_int = 2;
pub const R_OK: c_int = 4;

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

pub const PROT_NONE: c_int = 0x00000000;
pub const PROT_READ: c_int = 0x00000100;
pub const PROT_WRITE: c_int = 0x00000200;
pub const PROT_EXEC: c_int = 0x00000400;

pub const MAP_FILE: c_int = 0;
pub const MAP_SHARED: c_int = 1;
pub const MAP_PRIVATE: c_int = 2;
pub const MAP_FIXED: c_int = 0x10;

pub const MAP_FAILED: *mut c_void = !0 as *mut c_void;

pub const MS_ASYNC: c_int = 1;
pub const MS_INVALIDATE: c_int = 4;
pub const MS_SYNC: c_int = 2;

pub const SCM_RIGHTS: c_int = 0x01;
pub const SCM_TIMESTAMP: c_int = 0x02;
cfg_if! {
    if #[cfg(not(target_env = "nto71_iosock"))] {
        pub const SCM_CREDS: c_int = 0x04;
        pub const IFF_NOTRAILERS: c_int = 0x00000020;
        pub const AF_INET6: c_int = 24;
        pub const AF_BLUETOOTH: c_int = 31;
        pub const pseudo_AF_KEY: c_int = 29;
        pub const MSG_NOSIGNAL: c_int = 0x0800;
        pub const MSG_WAITFORONE: c_int = 0x2000;
        pub const IP_IPSEC_POLICY_COMPAT: c_int = 22;
        pub const IP_PKTINFO: c_int = 25;
        pub const IPPROTO_DIVERT: c_int = 259;
        pub const IPV6_IPSEC_POLICY_COMPAT: c_int = 28;
        pub const TCP_KEEPALIVE: c_int = 0x04;
        pub const ARPHRD_ARCNET: u16 = 7;
        pub const SO_BINDTODEVICE: c_int = 0x0800;
        pub const EAI_NODATA: c_int = 7;
        pub const IPTOS_ECN_NOT_ECT: u8 = 0x00;
        pub const RTF_BROADCAST: u32 = 0x80000;
        pub const UDP_ENCAP: c_int = 100;
        pub const HW_IOSTATS: c_int = 9;
        pub const HW_MACHINE_ARCH: c_int = 10;
        pub const HW_ALIGNBYTES: c_int = 11;
        pub const HW_CNMAGIC: c_int = 12;
        pub const HW_PHYSMEM64: c_int = 13;
        pub const HW_USERMEM64: c_int = 14;
        pub const HW_IOSTATNAMES: c_int = 15;
        pub const HW_MAXID: c_int = 15;
        pub const CTL_UNSPEC: c_int = 0;
        pub const CTL_QNX: c_int = 9;
        pub const CTL_PROC: c_int = 10;
        pub const CTL_VENDOR: c_int = 11;
        pub const CTL_EMUL: c_int = 12;
        pub const CTL_SECURITY: c_int = 13;
        pub const CTL_MAXID: c_int = 14;
        pub const AF_ARP: c_int = 28;
        pub const AF_IEEE80211: c_int = 32;
        pub const AF_NATM: c_int = 27;
        pub const AF_NS: c_int = 6;
        pub const BIOCGDLTLIST: c_int = -1072676233;
        pub const BIOCGETIF: c_int = 1083196011;
        pub const BIOCGSEESENT: c_int = 1074020984;
        pub const BIOCGSTATS: c_int = 1082147439;
        pub const BIOCSDLT: c_int = -2147204490;
        pub const BIOCSETIF: c_int = -2138029460;
        pub const BIOCSSEESENT: c_int = -2147204487;
        pub const FIONSPACE: c_int = 1074030200;
        pub const FIONWRITE: c_int = 1074030201;
        pub const IFF_ACCEPTRTADV: c_int = 0x40000000;
        pub const IFF_IP6FORWARDING: c_int = 0x20000000;
        pub const IFF_SHIM: c_int = 0x80000000;
        pub const KERN_ARND: c_int = 81;
        pub const KERN_IOV_MAX: c_int = 38;
        pub const KERN_LOGSIGEXIT: c_int = 46;
        pub const KERN_MAXID: c_int = 83;
        pub const KERN_PROC_ARGS: c_int = 48;
        pub const KERN_PROC_ENV: c_int = 3;
        pub const KERN_PROC_GID: c_int = 7;
        pub const KERN_PROC_RGID: c_int = 8;
        pub const LOCAL_CONNWAIT: c_int = 0x0002;
        pub const LOCAL_CREDS: c_int = 0x0001;
        pub const LOCAL_PEEREID: c_int = 0x0003;
        pub const MSG_NOTIFICATION: c_int = 0x0400;
        pub const NET_RT_IFLIST: c_int = 4;
        pub const NI_NUMERICSCOPE: c_int = 0x00000040;
        pub const PF_ARP: c_int = 28;
        pub const PF_NATM: c_int = 27;
        pub const pseudo_AF_HDRCMPLT: c_int = 30;
        pub const SIOCGIFADDR: c_int = -1064277727;
        pub const SO_FIB: c_int = 0x100a;
        pub const SO_TXPRIO: c_int = 0x100b;
        pub const SO_SETFIB: c_int = 0x100a;
        pub const SO_VLANPRIO: c_int = 0x100c;
        pub const USER_ATEXIT_MAX: c_int = 21;
        pub const USER_MAXID: c_int = 22;
        pub const SO_OVERFLOWED: c_int = 0x1009;
    } else {
        pub const SCM_CREDS: c_int = 0x03;
        pub const AF_INET6: c_int = 28;
        pub const AF_BLUETOOTH: c_int = 36;
        pub const pseudo_AF_KEY: c_int = 27;
        pub const MSG_NOSIGNAL: c_int = 0x20000;
        pub const MSG_WAITFORONE: c_int = 0x00080000;
        pub const IPPROTO_DIVERT: c_int = 258;
        pub const RTF_BROADCAST: u32 = 0x400000;
        pub const UDP_ENCAP: c_int = 1;
        pub const HW_MACHINE_ARCH: c_int = 11;
        pub const AF_ARP: c_int = 35;
        pub const AF_IEEE80211: c_int = 37;
        pub const AF_NATM: c_int = 29;
        pub const BIOCGDLTLIST: c_ulong = 0xffffffffc0104279;
        pub const BIOCGETIF: c_int = 0x4020426b;
        pub const BIOCGSEESENT: c_int = 0x40044276;
        pub const BIOCGSTATS: c_int = 0x4008426f;
        pub const BIOCSDLT: c_int = 0x80044278;
        pub const BIOCSETIF: c_int = 0x8020426c;
        pub const BIOCSSEESENT: c_int = 0x80044277;
        pub const KERN_ARND: c_int = 37;
        pub const KERN_IOV_MAX: c_int = 35;
        pub const KERN_LOGSIGEXIT: c_int = 34;
        pub const KERN_PROC_ARGS: c_int = 7;
        pub const KERN_PROC_ENV: c_int = 35;
        pub const KERN_PROC_GID: c_int = 11;
        pub const KERN_PROC_RGID: c_int = 10;
        pub const LOCAL_CONNWAIT: c_int = 4;
        pub const LOCAL_CREDS: c_int = 2;
        pub const MSG_NOTIFICATION: c_int = 0x00002000;
        pub const NET_RT_IFLIST: c_int = 3;
        pub const NI_NUMERICSCOPE: c_int = 0x00000020;
        pub const PF_ARP: c_int = AF_ARP;
        pub const PF_NATM: c_int = AF_NATM;
        pub const pseudo_AF_HDRCMPLT: c_int = 31;
        pub const SIOCGIFADDR: c_int = 0xc0206921;
        pub const SO_SETFIB: c_int = 0x1014;
    }
}

pub const MAP_TYPE: c_int = 0x3;

pub const IFF_UP: c_int = 0x00000001;
pub const IFF_BROADCAST: c_int = 0x00000002;
pub const IFF_DEBUG: c_int = 0x00000004;
pub const IFF_LOOPBACK: c_int = 0x00000008;
pub const IFF_POINTOPOINT: c_int = 0x00000010;
pub const IFF_RUNNING: c_int = 0x00000040;
pub const IFF_NOARP: c_int = 0x00000080;
pub const IFF_PROMISC: c_int = 0x00000100;
pub const IFF_ALLMULTI: c_int = 0x00000200;
pub const IFF_MULTICAST: c_int = 0x00008000;

pub const AF_UNSPEC: c_int = 0;
pub const AF_UNIX: c_int = AF_LOCAL;
pub const AF_LOCAL: c_int = 1;
pub const AF_INET: c_int = 2;
pub const AF_IPX: c_int = 23;
pub const AF_APPLETALK: c_int = 16;
pub const AF_ROUTE: c_int = 17;
pub const AF_SNA: c_int = 11;

pub const AF_ISDN: c_int = 26;

pub const PF_UNSPEC: c_int = AF_UNSPEC;
pub const PF_UNIX: c_int = PF_LOCAL;
pub const PF_LOCAL: c_int = AF_LOCAL;
pub const PF_INET: c_int = AF_INET;
pub const PF_IPX: c_int = AF_IPX;
pub const PF_APPLETALK: c_int = AF_APPLETALK;
pub const PF_INET6: c_int = AF_INET6;
pub const PF_KEY: c_int = pseudo_AF_KEY;
pub const PF_ROUTE: c_int = AF_ROUTE;
pub const PF_SNA: c_int = AF_SNA;

pub const PF_BLUETOOTH: c_int = AF_BLUETOOTH;
pub const PF_ISDN: c_int = AF_ISDN;

pub const SOMAXCONN: c_int = 128;

pub const MSG_OOB: c_int = 0x0001;
pub const MSG_PEEK: c_int = 0x0002;
pub const MSG_DONTROUTE: c_int = 0x0004;
pub const MSG_CTRUNC: c_int = 0x0020;
pub const MSG_TRUNC: c_int = 0x0010;
pub const MSG_DONTWAIT: c_int = 0x0080;
pub const MSG_EOR: c_int = 0x0008;
pub const MSG_WAITALL: c_int = 0x0040;

pub const IP_TOS: c_int = 3;
pub const IP_TTL: c_int = 4;
pub const IP_HDRINCL: c_int = 2;
pub const IP_OPTIONS: c_int = 1;
pub const IP_RECVOPTS: c_int = 5;
pub const IP_RETOPTS: c_int = 8;
pub const IP_MULTICAST_IF: c_int = 9;
pub const IP_MULTICAST_TTL: c_int = 10;
pub const IP_MULTICAST_LOOP: c_int = 11;
pub const IP_ADD_MEMBERSHIP: c_int = 12;
pub const IP_DROP_MEMBERSHIP: c_int = 13;
pub const IP_DEFAULT_MULTICAST_TTL: c_int = 1;
pub const IP_DEFAULT_MULTICAST_LOOP: c_int = 1;

pub const IPPROTO_HOPOPTS: c_int = 0;
pub const IPPROTO_IGMP: c_int = 2;
pub const IPPROTO_IPIP: c_int = 4;
pub const IPPROTO_EGP: c_int = 8;
pub const IPPROTO_PUP: c_int = 12;
pub const IPPROTO_IDP: c_int = 22;
pub const IPPROTO_TP: c_int = 29;
pub const IPPROTO_ROUTING: c_int = 43;
pub const IPPROTO_FRAGMENT: c_int = 44;
pub const IPPROTO_RSVP: c_int = 46;
pub const IPPROTO_GRE: c_int = 47;
pub const IPPROTO_ESP: c_int = 50;
pub const IPPROTO_AH: c_int = 51;
pub const IPPROTO_NONE: c_int = 59;
pub const IPPROTO_DSTOPTS: c_int = 60;
pub const IPPROTO_ENCAP: c_int = 98;
pub const IPPROTO_PIM: c_int = 103;
pub const IPPROTO_SCTP: c_int = 132;
pub const IPPROTO_RAW: c_int = 255;
pub const IPPROTO_MAX: c_int = 256;
pub const IPPROTO_CARP: c_int = 112;
pub const IPPROTO_DONE: c_int = 257;
pub const IPPROTO_EON: c_int = 80;
pub const IPPROTO_ETHERIP: c_int = 97;
pub const IPPROTO_GGP: c_int = 3;
pub const IPPROTO_IPCOMP: c_int = 108;
pub const IPPROTO_MOBILE: c_int = 55;

pub const IPV6_RTHDR_LOOSE: c_int = 0;
pub const IPV6_RTHDR_STRICT: c_int = 1;
pub const IPV6_UNICAST_HOPS: c_int = 4;
pub const IPV6_MULTICAST_IF: c_int = 9;
pub const IPV6_MULTICAST_HOPS: c_int = 10;
pub const IPV6_MULTICAST_LOOP: c_int = 11;
pub const IPV6_JOIN_GROUP: c_int = 12;
pub const IPV6_LEAVE_GROUP: c_int = 13;
pub const IPV6_CHECKSUM: c_int = 26;
pub const IPV6_V6ONLY: c_int = 27;
pub const IPV6_RTHDRDSTOPTS: c_int = 35;
pub const IPV6_RECVPKTINFO: c_int = 36;
pub const IPV6_RECVHOPLIMIT: c_int = 37;
pub const IPV6_RECVRTHDR: c_int = 38;
pub const IPV6_RECVHOPOPTS: c_int = 39;
pub const IPV6_RECVDSTOPTS: c_int = 40;
pub const IPV6_RECVPATHMTU: c_int = 43;
pub const IPV6_PATHMTU: c_int = 44;
pub const IPV6_PKTINFO: c_int = 46;
pub const IPV6_HOPLIMIT: c_int = 47;
pub const IPV6_NEXTHOP: c_int = 48;
pub const IPV6_HOPOPTS: c_int = 49;
pub const IPV6_DSTOPTS: c_int = 50;
pub const IPV6_RECVTCLASS: c_int = 57;
pub const IPV6_TCLASS: c_int = 61;
pub const IPV6_DONTFRAG: c_int = 62;

pub const TCP_NODELAY: c_int = 0x01;
pub const TCP_MAXSEG: c_int = 0x02;
pub const TCP_MD5SIG: c_int = 0x10;

pub const SHUT_RD: c_int = 0;
pub const SHUT_WR: c_int = 1;
pub const SHUT_RDWR: c_int = 2;

pub const LOCK_SH: c_int = 0x1;
pub const LOCK_EX: c_int = 0x2;
pub const LOCK_NB: c_int = 0x4;
pub const LOCK_UN: c_int = 0x8;

pub const SS_ONSTACK: c_int = 1;
pub const SS_DISABLE: c_int = 2;

pub const PATH_MAX: c_int = 1024;

pub const UIO_MAXIOV: c_int = 1024;

pub const FD_SETSIZE: usize = 256;

pub const TCIOFF: c_int = 0x0002;
pub const TCION: c_int = 0x0003;
pub const TCOOFF: c_int = 0x0000;
pub const TCOON: c_int = 0x0001;
pub const TCIFLUSH: c_int = 0;
pub const TCOFLUSH: c_int = 1;
pub const TCIOFLUSH: c_int = 2;
pub const NL0: crate::tcflag_t = 0x000;
pub const NL1: crate::tcflag_t = 0x100;
pub const TAB0: crate::tcflag_t = 0x0000;
pub const CR0: crate::tcflag_t = 0x000;
pub const FF0: crate::tcflag_t = 0x0000;
pub const BS0: crate::tcflag_t = 0x0000;
pub const VT0: crate::tcflag_t = 0x0000;
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
pub const OPOST: crate::tcflag_t = 0x00000001;
pub const CS5: crate::tcflag_t = 0x00;
pub const ECHO: crate::tcflag_t = 0x00000008;
pub const OCRNL: crate::tcflag_t = 0x00000008;
pub const ONOCR: crate::tcflag_t = 0x00000010;
pub const ONLRET: crate::tcflag_t = 0x00000020;
pub const OFILL: crate::tcflag_t = 0x00000040;
pub const OFDEL: crate::tcflag_t = 0x00000080;

pub const WNOHANG: c_int = 0x0040;
pub const WUNTRACED: c_int = 0x0004;
pub const WSTOPPED: c_int = WUNTRACED;
pub const WEXITED: c_int = 0x0001;
pub const WCONTINUED: c_int = 0x0008;
pub const WNOWAIT: c_int = 0x0080;
pub const WTRAPPED: c_int = 0x0002;

pub const RTLD_LOCAL: c_int = 0x0200;
pub const RTLD_LAZY: c_int = 0x0001;

pub const POSIX_FADV_NORMAL: c_int = 0;
pub const POSIX_FADV_RANDOM: c_int = 2;
pub const POSIX_FADV_SEQUENTIAL: c_int = 1;
pub const POSIX_FADV_WILLNEED: c_int = 3;

pub const AT_FDCWD: c_int = -100;
pub const AT_EACCESS: c_int = 0x0001;
pub const AT_SYMLINK_NOFOLLOW: c_int = 0x0002;
pub const AT_SYMLINK_FOLLOW: c_int = 0x0004;
pub const AT_REMOVEDIR: c_int = 0x0008;

pub const LOG_CRON: c_int = 9 << 3;
pub const LOG_AUTHPRIV: c_int = 10 << 3;
pub const LOG_FTP: c_int = 11 << 3;
pub const LOG_PERROR: c_int = 0x20;

pub const PIPE_BUF: usize = 5120;

pub const CLD_EXITED: c_int = 1;
pub const CLD_KILLED: c_int = 2;
pub const CLD_DUMPED: c_int = 3;
pub const CLD_TRAPPED: c_int = 4;
pub const CLD_STOPPED: c_int = 5;
pub const CLD_CONTINUED: c_int = 6;

pub const UTIME_OMIT: c_long = 0x40000002;
pub const UTIME_NOW: c_long = 0x40000001;

pub const POLLIN: c_short = POLLRDNORM | POLLRDBAND;
pub const POLLPRI: c_short = 0x0008;
pub const POLLOUT: c_short = 0x0002;
pub const POLLERR: c_short = 0x0020;
pub const POLLHUP: c_short = 0x0040;
pub const POLLNVAL: c_short = 0x1000;
pub const POLLRDNORM: c_short = 0x0001;
pub const POLLRDBAND: c_short = 0x0004;

pub const IPTOS_LOWDELAY: u8 = 0x10;
pub const IPTOS_THROUGHPUT: u8 = 0x08;
pub const IPTOS_RELIABILITY: u8 = 0x04;
pub const IPTOS_MINCOST: u8 = 0x02;

pub const IPTOS_PREC_NETCONTROL: u8 = 0xe0;
pub const IPTOS_PREC_INTERNETCONTROL: u8 = 0xc0;
pub const IPTOS_PREC_CRITIC_ECP: u8 = 0xa0;
pub const IPTOS_PREC_FLASHOVERRIDE: u8 = 0x80;
pub const IPTOS_PREC_FLASH: u8 = 0x60;
pub const IPTOS_PREC_IMMEDIATE: u8 = 0x40;
pub const IPTOS_PREC_PRIORITY: u8 = 0x20;
pub const IPTOS_PREC_ROUTINE: u8 = 0x00;

pub const IPTOS_ECN_MASK: u8 = 0x03;
pub const IPTOS_ECN_ECT1: u8 = 0x01;
pub const IPTOS_ECN_ECT0: u8 = 0x02;
pub const IPTOS_ECN_CE: u8 = 0x03;

pub const IPOPT_CONTROL: u8 = 0x00;
pub const IPOPT_RESERVED1: u8 = 0x20;
pub const IPOPT_RESERVED2: u8 = 0x60;
pub const IPOPT_LSRR: u8 = 131;
pub const IPOPT_RR: u8 = 7;
pub const IPOPT_SSRR: u8 = 137;
pub const IPDEFTTL: u8 = 64;
pub const IPOPT_OPTVAL: u8 = 0;
pub const IPOPT_OLEN: u8 = 1;
pub const IPOPT_OFFSET: u8 = 2;
pub const IPOPT_MINOFF: u8 = 4;
pub const IPOPT_NOP: u8 = 1;
pub const IPOPT_EOL: u8 = 0;
pub const IPOPT_TS: u8 = 68;
pub const IPOPT_TS_TSONLY: u8 = 0;
pub const IPOPT_TS_TSANDADDR: u8 = 1;
pub const IPOPT_TS_PRESPEC: u8 = 3;

pub const MAX_IPOPTLEN: u8 = 40;
pub const IPVERSION: u8 = 4;
pub const MAXTTL: u8 = 255;

pub const ARPHRD_ETHER: u16 = 1;
pub const ARPHRD_IEEE802: u16 = 6;
pub const ARPHRD_IEEE1394: u16 = 24;

pub const SOL_SOCKET: c_int = 0xffff;

pub const SO_DEBUG: c_int = 0x0001;
pub const SO_REUSEADDR: c_int = 0x0004;
pub const SO_TYPE: c_int = 0x1008;
pub const SO_ERROR: c_int = 0x1007;
pub const SO_DONTROUTE: c_int = 0x0010;
pub const SO_BROADCAST: c_int = 0x0020;
pub const SO_SNDBUF: c_int = 0x1001;
pub const SO_RCVBUF: c_int = 0x1002;
pub const SO_KEEPALIVE: c_int = 0x0008;
pub const SO_OOBINLINE: c_int = 0x0100;
pub const SO_LINGER: c_int = 0x0080;
pub const SO_REUSEPORT: c_int = 0x0200;
pub const SO_RCVLOWAT: c_int = 0x1004;
pub const SO_SNDLOWAT: c_int = 0x1003;
pub const SO_RCVTIMEO: c_int = 0x1006;
pub const SO_SNDTIMEO: c_int = 0x1005;
pub const SO_TIMESTAMP: c_int = 0x0400;
pub const SO_ACCEPTCONN: c_int = 0x0002;

pub const TIOCM_LE: c_int = 0x0100;
pub const TIOCM_DTR: c_int = 0x0001;
pub const TIOCM_RTS: c_int = 0x0002;
pub const TIOCM_ST: c_int = 0x0200;
pub const TIOCM_SR: c_int = 0x0400;
pub const TIOCM_CTS: c_int = 0x1000;
pub const TIOCM_CAR: c_int = TIOCM_CD;
pub const TIOCM_CD: c_int = 0x8000;
pub const TIOCM_RNG: c_int = TIOCM_RI;
pub const TIOCM_RI: c_int = 0x4000;
pub const TIOCM_DSR: c_int = 0x2000;

pub const SCHED_OTHER: c_int = 3;
pub const SCHED_FIFO: c_int = 1;
pub const SCHED_RR: c_int = 2;

pub const IPC_PRIVATE: crate::key_t = 0;

pub const IPC_CREAT: c_int = 0o001000;
pub const IPC_EXCL: c_int = 0o002000;
pub const IPC_NOWAIT: c_int = 0o004000;

pub const IPC_RMID: c_int = 0;
pub const IPC_SET: c_int = 1;
pub const IPC_STAT: c_int = 2;

pub const MSG_NOERROR: c_int = 0o010000;

pub const LOG_NFACILITIES: c_int = 24;

pub const SEM_FAILED: *mut crate::sem_t = 0xFFFFFFFFFFFFFFFF as *mut sem_t;

pub const AI_PASSIVE: c_int = 0x00000001;
pub const AI_CANONNAME: c_int = 0x00000002;
pub const AI_NUMERICHOST: c_int = 0x00000004;

pub const AI_NUMERICSERV: c_int = 0x00000008;

pub const EAI_BADFLAGS: c_int = 3;
pub const EAI_NONAME: c_int = 8;
pub const EAI_AGAIN: c_int = 2;
pub const EAI_FAIL: c_int = 4;
pub const EAI_FAMILY: c_int = 5;
pub const EAI_SOCKTYPE: c_int = 10;
pub const EAI_SERVICE: c_int = 9;
pub const EAI_MEMORY: c_int = 6;
pub const EAI_SYSTEM: c_int = 11;
pub const EAI_OVERFLOW: c_int = 14;

pub const NI_NUMERICHOST: c_int = 0x00000002;
pub const NI_NUMERICSERV: c_int = 0x00000008;
pub const NI_NOFQDN: c_int = 0x00000001;
pub const NI_NAMEREQD: c_int = 0x00000004;
pub const NI_DGRAM: c_int = 0x00000010;

pub const AIO_CANCELED: c_int = 0;
pub const AIO_NOTCANCELED: c_int = 2;
pub const AIO_ALLDONE: c_int = 1;
pub const LIO_READ: c_int = 1;
pub const LIO_WRITE: c_int = 2;
pub const LIO_NOP: c_int = 0;
pub const LIO_WAIT: c_int = 1;
pub const LIO_NOWAIT: c_int = 0;

pub const ITIMER_REAL: c_int = 0;
pub const ITIMER_VIRTUAL: c_int = 1;
pub const ITIMER_PROF: c_int = 2;

// DIFF(main): changed to `c_short` in f62eb023ab
pub const POSIX_SPAWN_RESETIDS: c_int = 0x00000010;
pub const POSIX_SPAWN_SETPGROUP: c_int = 0x00000001;
pub const POSIX_SPAWN_SETSIGDEF: c_int = 0x00000004;
pub const POSIX_SPAWN_SETSIGMASK: c_int = 0x00000002;
pub const POSIX_SPAWN_SETSCHEDPARAM: c_int = 0x00000400;
pub const POSIX_SPAWN_SETSCHEDULER: c_int = 0x00000040;

pub const RTF_UP: c_ushort = 0x0001;
pub const RTF_GATEWAY: c_ushort = 0x0002;

pub const RTF_HOST: c_ushort = 0x0004;
pub const RTF_DYNAMIC: c_ushort = 0x0010;
pub const RTF_MODIFIED: c_ushort = 0x0020;
pub const RTF_REJECT: c_ushort = 0x0008;
pub const RTF_STATIC: c_ushort = 0x0800;
pub const RTF_XRESOLVE: c_ushort = 0x0200;
pub const RTM_NEWADDR: u16 = 0xc;
pub const RTM_DELADDR: u16 = 0xd;
pub const RTA_DST: c_ushort = 0x1;
pub const RTA_GATEWAY: c_ushort = 0x2;

pub const IN_ACCESS: u32 = 0x00000001;
pub const IN_MODIFY: u32 = 0x00000002;
pub const IN_ATTRIB: u32 = 0x00000004;
pub const IN_CLOSE_WRITE: u32 = 0x00000008;
pub const IN_CLOSE_NOWRITE: u32 = 0x00000010;
pub const IN_CLOSE: u32 = IN_CLOSE_WRITE | IN_CLOSE_NOWRITE;
pub const IN_OPEN: u32 = 0x00000020;
pub const IN_MOVED_FROM: u32 = 0x00000040;
pub const IN_MOVED_TO: u32 = 0x00000080;
pub const IN_MOVE: u32 = IN_MOVED_FROM | IN_MOVED_TO;
pub const IN_CREATE: u32 = 0x00000100;
pub const IN_DELETE: u32 = 0x00000200;
pub const IN_DELETE_SELF: u32 = 0x00000400;
pub const IN_MOVE_SELF: u32 = 0x00000800;
pub const IN_UNMOUNT: u32 = 0x00002000;
pub const IN_Q_OVERFLOW: u32 = 0x00004000;
pub const IN_IGNORED: u32 = 0x00008000;
pub const IN_ONLYDIR: u32 = 0x01000000;
pub const IN_DONT_FOLLOW: u32 = 0x02000000;

pub const IN_ISDIR: u32 = 0x40000000;
pub const IN_ONESHOT: u32 = 0x80000000;

pub const REG_EXTENDED: c_int = 0o0001;
pub const REG_ICASE: c_int = 0o0002;
pub const REG_NEWLINE: c_int = 0o0010;
pub const REG_NOSUB: c_int = 0o0004;

pub const REG_NOTBOL: c_int = 0o00001;
pub const REG_NOTEOL: c_int = 0o00002;

pub const REG_ENOSYS: c_int = 17;
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

// errno.h
pub const EOK: c_int = 0;
pub const EWOULDBLOCK: c_int = EAGAIN;
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
pub const EMULTIHOP: c_int = 74;
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
pub const ENOTRECOVERABLE: c_int = 95;
pub const EOPNOTSUPP: c_int = 103;
pub const EFPOS: c_int = 110;
pub const ESTALE: c_int = 122;
pub const EINPROGRESS: c_int = 236;
pub const EALREADY: c_int = 237;
pub const ENOTSOCK: c_int = 238;
pub const EDESTADDRREQ: c_int = 239;
pub const EMSGSIZE: c_int = 240;
pub const EPROTOTYPE: c_int = 241;
pub const ENOPROTOOPT: c_int = 242;
pub const EPROTONOSUPPORT: c_int = 243;
pub const ESOCKTNOSUPPORT: c_int = 244;
pub const EPFNOSUPPORT: c_int = 246;
pub const EAFNOSUPPORT: c_int = 247;
pub const EADDRINUSE: c_int = 248;
pub const EADDRNOTAVAIL: c_int = 249;
pub const ENETDOWN: c_int = 250;
pub const ENETUNREACH: c_int = 251;
pub const ENETRESET: c_int = 252;
pub const ECONNABORTED: c_int = 253;
pub const ECONNRESET: c_int = 254;
pub const ENOBUFS: c_int = 255;
pub const EISCONN: c_int = 256;
pub const ENOTCONN: c_int = 257;
pub const ESHUTDOWN: c_int = 258;
pub const ETOOMANYREFS: c_int = 259;
pub const ETIMEDOUT: c_int = 260;
pub const ECONNREFUSED: c_int = 261;
pub const EHOSTDOWN: c_int = 264;
pub const EHOSTUNREACH: c_int = 265;
pub const EBADRPC: c_int = 272;
pub const ERPCMISMATCH: c_int = 273;
pub const EPROGUNAVAIL: c_int = 274;
pub const EPROGMISMATCH: c_int = 275;
pub const EPROCUNAVAIL: c_int = 276;
pub const ENOREMOTE: c_int = 300;
pub const ENONDP: c_int = 301;
pub const EBADFSYS: c_int = 302;
pub const EMORE: c_int = 309;
pub const ECTRLTERM: c_int = 310;
pub const ENOLIC: c_int = 311;
pub const ESRVRFAULT: c_int = 312;
pub const EENDIAN: c_int = 313;
pub const ESECTYPEINVAL: c_int = 314;

pub const RUSAGE_CHILDREN: c_int = -1;
pub const L_tmpnam: c_uint = 255;

pub const _PC_LINK_MAX: c_int = 1;
pub const _PC_MAX_CANON: c_int = 2;
pub const _PC_MAX_INPUT: c_int = 3;
pub const _PC_NAME_MAX: c_int = 4;
pub const _PC_PATH_MAX: c_int = 5;
pub const _PC_PIPE_BUF: c_int = 6;
pub const _PC_CHOWN_RESTRICTED: c_int = 9;
pub const _PC_NO_TRUNC: c_int = 7;
pub const _PC_VDISABLE: c_int = 8;
pub const _PC_SYNC_IO: c_int = 14;
pub const _PC_ASYNC_IO: c_int = 12;
pub const _PC_PRIO_IO: c_int = 13;
pub const _PC_SOCK_MAXBUF: c_int = 15;
pub const _PC_FILESIZEBITS: c_int = 16;
pub const _PC_REC_INCR_XFER_SIZE: c_int = 22;
pub const _PC_REC_MAX_XFER_SIZE: c_int = 23;
pub const _PC_REC_MIN_XFER_SIZE: c_int = 24;
pub const _PC_REC_XFER_ALIGN: c_int = 25;
pub const _PC_ALLOC_SIZE_MIN: c_int = 21;
pub const _PC_SYMLINK_MAX: c_int = 17;
pub const _PC_2_SYMLINKS: c_int = 20;

pub const _SC_PAGE_SIZE: c_int = _SC_PAGESIZE;
pub const _SC_ARG_MAX: c_int = 1;
pub const _SC_CHILD_MAX: c_int = 2;
pub const _SC_CLK_TCK: c_int = 3;
pub const _SC_NGROUPS_MAX: c_int = 4;
pub const _SC_OPEN_MAX: c_int = 5;
pub const _SC_JOB_CONTROL: c_int = 6;
pub const _SC_SAVED_IDS: c_int = 7;
pub const _SC_VERSION: c_int = 8;
pub const _SC_PASS_MAX: c_int = 9;
pub const _SC_PAGESIZE: c_int = 11;
pub const _SC_XOPEN_VERSION: c_int = 12;
pub const _SC_STREAM_MAX: c_int = 13;
pub const _SC_TZNAME_MAX: c_int = 14;
pub const _SC_AIO_LISTIO_MAX: c_int = 15;
pub const _SC_AIO_MAX: c_int = 16;
pub const _SC_AIO_PRIO_DELTA_MAX: c_int = 17;
pub const _SC_DELAYTIMER_MAX: c_int = 18;
pub const _SC_MQ_OPEN_MAX: c_int = 19;
pub const _SC_MQ_PRIO_MAX: c_int = 20;
pub const _SC_RTSIG_MAX: c_int = 21;
pub const _SC_SEM_NSEMS_MAX: c_int = 22;
pub const _SC_SEM_VALUE_MAX: c_int = 23;
pub const _SC_SIGQUEUE_MAX: c_int = 24;
pub const _SC_TIMER_MAX: c_int = 25;
pub const _SC_ASYNCHRONOUS_IO: c_int = 26;
pub const _SC_FSYNC: c_int = 27;
pub const _SC_MAPPED_FILES: c_int = 28;
pub const _SC_MEMLOCK: c_int = 29;
pub const _SC_MEMLOCK_RANGE: c_int = 30;
pub const _SC_MEMORY_PROTECTION: c_int = 31;
pub const _SC_MESSAGE_PASSING: c_int = 32;
pub const _SC_PRIORITIZED_IO: c_int = 33;
pub const _SC_PRIORITY_SCHEDULING: c_int = 34;
pub const _SC_REALTIME_SIGNALS: c_int = 35;
pub const _SC_SEMAPHORES: c_int = 36;
pub const _SC_SHARED_MEMORY_OBJECTS: c_int = 37;
pub const _SC_SYNCHRONIZED_IO: c_int = 38;
pub const _SC_TIMERS: c_int = 39;
pub const _SC_GETGR_R_SIZE_MAX: c_int = 40;
pub const _SC_GETPW_R_SIZE_MAX: c_int = 41;
pub const _SC_LOGIN_NAME_MAX: c_int = 42;
pub const _SC_THREAD_DESTRUCTOR_ITERATIONS: c_int = 43;
pub const _SC_THREAD_KEYS_MAX: c_int = 44;
pub const _SC_THREAD_STACK_MIN: c_int = 45;
pub const _SC_THREAD_THREADS_MAX: c_int = 46;
pub const _SC_TTY_NAME_MAX: c_int = 47;
pub const _SC_THREADS: c_int = 48;
pub const _SC_THREAD_ATTR_STACKADDR: c_int = 49;
pub const _SC_THREAD_ATTR_STACKSIZE: c_int = 50;
pub const _SC_THREAD_PRIORITY_SCHEDULING: c_int = 51;
pub const _SC_THREAD_PRIO_INHERIT: c_int = 52;
pub const _SC_THREAD_PRIO_PROTECT: c_int = 53;
pub const _SC_THREAD_PROCESS_SHARED: c_int = 54;
pub const _SC_THREAD_SAFE_FUNCTIONS: c_int = 55;
pub const _SC_2_CHAR_TERM: c_int = 56;
pub const _SC_2_C_BIND: c_int = 57;
pub const _SC_2_C_DEV: c_int = 58;
pub const _SC_2_C_VERSION: c_int = 59;
pub const _SC_2_FORT_DEV: c_int = 60;
pub const _SC_2_FORT_RUN: c_int = 61;
pub const _SC_2_LOCALEDEF: c_int = 62;
pub const _SC_2_SW_DEV: c_int = 63;
pub const _SC_2_UPE: c_int = 64;
pub const _SC_2_VERSION: c_int = 65;
pub const _SC_ATEXIT_MAX: c_int = 66;
pub const _SC_AVPHYS_PAGES: c_int = 67;
pub const _SC_BC_BASE_MAX: c_int = 68;
pub const _SC_BC_DIM_MAX: c_int = 69;
pub const _SC_BC_SCALE_MAX: c_int = 70;
pub const _SC_BC_STRING_MAX: c_int = 71;
pub const _SC_CHARCLASS_NAME_MAX: c_int = 72;
pub const _SC_CHAR_BIT: c_int = 73;
pub const _SC_CHAR_MAX: c_int = 74;
pub const _SC_CHAR_MIN: c_int = 75;
pub const _SC_COLL_WEIGHTS_MAX: c_int = 76;
pub const _SC_EQUIV_CLASS_MAX: c_int = 77;
pub const _SC_EXPR_NEST_MAX: c_int = 78;
pub const _SC_INT_MAX: c_int = 79;
pub const _SC_INT_MIN: c_int = 80;
pub const _SC_LINE_MAX: c_int = 81;
pub const _SC_LONG_BIT: c_int = 82;
pub const _SC_MB_LEN_MAX: c_int = 83;
pub const _SC_NL_ARGMAX: c_int = 84;
pub const _SC_NL_LANGMAX: c_int = 85;
pub const _SC_NL_MSGMAX: c_int = 86;
pub const _SC_NL_NMAX: c_int = 87;
pub const _SC_NL_SETMAX: c_int = 88;
pub const _SC_NL_TEXTMAX: c_int = 89;
pub const _SC_NPROCESSORS_CONF: c_int = 90;
pub const _SC_NPROCESSORS_ONLN: c_int = 91;
pub const _SC_NZERO: c_int = 92;
pub const _SC_PHYS_PAGES: c_int = 93;
pub const _SC_PII: c_int = 94;
pub const _SC_PII_INTERNET: c_int = 95;
pub const _SC_PII_INTERNET_DGRAM: c_int = 96;
pub const _SC_PII_INTERNET_STREAM: c_int = 97;
pub const _SC_PII_OSI: c_int = 98;
pub const _SC_PII_OSI_CLTS: c_int = 99;
pub const _SC_PII_OSI_COTS: c_int = 100;
pub const _SC_PII_OSI_M: c_int = 101;
pub const _SC_PII_SOCKET: c_int = 102;
pub const _SC_PII_XTI: c_int = 103;
pub const _SC_POLL: c_int = 104;
pub const _SC_RE_DUP_MAX: c_int = 105;
pub const _SC_SCHAR_MAX: c_int = 106;
pub const _SC_SCHAR_MIN: c_int = 107;
pub const _SC_SELECT: c_int = 108;
pub const _SC_SHRT_MAX: c_int = 109;
pub const _SC_SHRT_MIN: c_int = 110;
pub const _SC_SSIZE_MAX: c_int = 111;
pub const _SC_T_IOV_MAX: c_int = 112;
pub const _SC_UCHAR_MAX: c_int = 113;
pub const _SC_UINT_MAX: c_int = 114;
pub const _SC_UIO_MAXIOV: c_int = 115;
pub const _SC_ULONG_MAX: c_int = 116;
pub const _SC_USHRT_MAX: c_int = 117;
pub const _SC_WORD_BIT: c_int = 118;
pub const _SC_XOPEN_CRYPT: c_int = 119;
pub const _SC_XOPEN_ENH_I18N: c_int = 120;
pub const _SC_XOPEN_SHM: c_int = 121;
pub const _SC_XOPEN_UNIX: c_int = 122;
pub const _SC_XOPEN_XCU_VERSION: c_int = 123;
pub const _SC_XOPEN_XPG2: c_int = 124;
pub const _SC_XOPEN_XPG3: c_int = 125;
pub const _SC_XOPEN_XPG4: c_int = 126;
pub const _SC_XBS5_ILP32_OFF32: c_int = 127;
pub const _SC_XBS5_ILP32_OFFBIG: c_int = 128;
pub const _SC_XBS5_LP64_OFF64: c_int = 129;
pub const _SC_XBS5_LPBIG_OFFBIG: c_int = 130;
pub const _SC_ADVISORY_INFO: c_int = 131;
pub const _SC_CPUTIME: c_int = 132;
pub const _SC_SPAWN: c_int = 133;
pub const _SC_SPORADIC_SERVER: c_int = 134;
pub const _SC_THREAD_CPUTIME: c_int = 135;
pub const _SC_THREAD_SPORADIC_SERVER: c_int = 136;
pub const _SC_TIMEOUTS: c_int = 137;
pub const _SC_BARRIERS: c_int = 138;
pub const _SC_CLOCK_SELECTION: c_int = 139;
pub const _SC_MONOTONIC_CLOCK: c_int = 140;
pub const _SC_READER_WRITER_LOCKS: c_int = 141;
pub const _SC_SPIN_LOCKS: c_int = 142;
pub const _SC_TYPED_MEMORY_OBJECTS: c_int = 143;
pub const _SC_TRACE_EVENT_FILTER: c_int = 144;
pub const _SC_TRACE: c_int = 145;
pub const _SC_TRACE_INHERIT: c_int = 146;
pub const _SC_TRACE_LOG: c_int = 147;
pub const _SC_2_PBS: c_int = 148;
pub const _SC_2_PBS_ACCOUNTING: c_int = 149;
pub const _SC_2_PBS_CHECKPOINT: c_int = 150;
pub const _SC_2_PBS_LOCATE: c_int = 151;
pub const _SC_2_PBS_MESSAGE: c_int = 152;
pub const _SC_2_PBS_TRACK: c_int = 153;
pub const _SC_HOST_NAME_MAX: c_int = 154;
pub const _SC_IOV_MAX: c_int = 155;
pub const _SC_IPV6: c_int = 156;
pub const _SC_RAW_SOCKETS: c_int = 157;
pub const _SC_REGEXP: c_int = 158;
pub const _SC_SHELL: c_int = 159;
pub const _SC_SS_REPL_MAX: c_int = 160;
pub const _SC_SYMLOOP_MAX: c_int = 161;
pub const _SC_TRACE_EVENT_NAME_MAX: c_int = 162;
pub const _SC_TRACE_NAME_MAX: c_int = 163;
pub const _SC_TRACE_SYS_MAX: c_int = 164;
pub const _SC_TRACE_USER_EVENT_MAX: c_int = 165;
pub const _SC_V6_ILP32_OFF32: c_int = 166;
pub const _SC_V6_ILP32_OFFBIG: c_int = 167;
pub const _SC_V6_LP64_OFF64: c_int = 168;
pub const _SC_V6_LPBIG_OFFBIG: c_int = 169;
pub const _SC_XOPEN_REALTIME: c_int = 170;
pub const _SC_XOPEN_REALTIME_THREADS: c_int = 171;
pub const _SC_XOPEN_LEGACY: c_int = 172;
pub const _SC_XOPEN_STREAMS: c_int = 173;
pub const _SC_V7_ILP32_OFF32: c_int = 176;
pub const _SC_V7_ILP32_OFFBIG: c_int = 177;
pub const _SC_V7_LP64_OFF64: c_int = 178;
pub const _SC_V7_LPBIG_OFFBIG: c_int = 179;

pub const GLOB_ERR: c_int = 0x0001;
pub const GLOB_MARK: c_int = 0x0002;
pub const GLOB_NOSORT: c_int = 0x0004;
pub const GLOB_DOOFFS: c_int = 0x0008;
pub const GLOB_NOCHECK: c_int = 0x0010;
pub const GLOB_APPEND: c_int = 0x0020;
pub const GLOB_NOESCAPE: c_int = 0x0040;

pub const GLOB_NOSPACE: c_int = 1;
pub const GLOB_ABORTED: c_int = 2;
pub const GLOB_NOMATCH: c_int = 3;

pub const S_IEXEC: mode_t = crate::S_IXUSR;
pub const S_IWRITE: mode_t = crate::S_IWUSR;
pub const S_IREAD: mode_t = crate::S_IRUSR;

pub const S_IFIFO: mode_t = 0o1_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_IFMT: mode_t = 0o17_0000;

pub const S_IXOTH: mode_t = 0o0001;
pub const S_IWOTH: mode_t = 0o0002;
pub const S_IROTH: mode_t = 0o0004;
pub const S_IRWXO: mode_t = 0o0007;
pub const S_IXGRP: mode_t = 0o0010;
pub const S_IWGRP: mode_t = 0o0020;
pub const S_IRGRP: mode_t = 0o0040;
pub const S_IRWXG: mode_t = 0o0070;
pub const S_IXUSR: mode_t = 0o0100;
pub const S_IWUSR: mode_t = 0o0200;
pub const S_IRUSR: mode_t = 0o0400;
pub const S_IRWXU: mode_t = 0o0700;

pub const F_LOCK: c_int = 1;
pub const F_TEST: c_int = 3;
pub const F_TLOCK: c_int = 2;
pub const F_ULOCK: c_int = 0;

pub const ST_RDONLY: c_ulong = 0x01;
pub const ST_NOSUID: c_ulong = 0x04;
pub const ST_NOEXEC: c_ulong = 0x02;
pub const ST_NOATIME: c_ulong = 0x20;

pub const RTLD_NEXT: *mut c_void = -3i64 as *mut c_void;
pub const RTLD_DEFAULT: *mut c_void = -2i64 as *mut c_void;
pub const RTLD_NODELETE: c_int = 0x1000;
pub const RTLD_NOW: c_int = 0x0002;

pub const EMPTY: c_short = 0;
pub const RUN_LVL: c_short = 1;
pub const BOOT_TIME: c_short = 2;
pub const NEW_TIME: c_short = 4;
pub const OLD_TIME: c_short = 3;
pub const INIT_PROCESS: c_short = 5;
pub const LOGIN_PROCESS: c_short = 6;
pub const USER_PROCESS: c_short = 7;
pub const DEAD_PROCESS: c_short = 8;
pub const ACCOUNTING: c_short = 9;

pub const ENOTSUP: c_int = 48;

pub const BUFSIZ: c_uint = 1024;
pub const TMP_MAX: c_uint = 26 * 26 * 26;
pub const FOPEN_MAX: c_uint = 16;
pub const FILENAME_MAX: c_uint = 255;

pub const NI_MAXHOST: crate::socklen_t = 1025;
pub const M_KEEP: c_int = 4;
pub const REG_STARTEND: c_int = 0o00004;
pub const VEOF: usize = 4;

pub const RTLD_GLOBAL: c_int = 0x0100;
pub const RTLD_NOLOAD: c_int = 0x0004;

pub const O_RDONLY: c_int = 0o000000;
pub const O_WRONLY: c_int = 0o000001;
pub const O_RDWR: c_int = 0o000002;

pub const O_EXEC: c_int = 0o00003;
pub const O_ASYNC: c_int = 0o0200000;
pub const O_NDELAY: c_int = O_NONBLOCK;
pub const O_TRUNC: c_int = 0o001000;
pub const O_CLOEXEC: c_int = 0o020000;
pub const O_DIRECTORY: c_int = 0o4000000;
pub const O_ACCMODE: c_int = 0o000007;
pub const O_APPEND: c_int = 0o000010;
pub const O_CREAT: c_int = 0o000400;
pub const O_EXCL: c_int = 0o002000;
pub const O_NOCTTY: c_int = 0o004000;
pub const O_NONBLOCK: c_int = 0o000200;
pub const O_SYNC: c_int = 0o000040;
pub const O_RSYNC: c_int = 0o000100;
pub const O_DSYNC: c_int = 0o000020;
pub const O_NOFOLLOW: c_int = 0o010000;

pub const POSIX_FADV_DONTNEED: c_int = 4;
pub const POSIX_FADV_NOREUSE: c_int = 5;

pub const SOCK_SEQPACKET: c_int = 5;
pub const SOCK_STREAM: c_int = 1;
pub const SOCK_DGRAM: c_int = 2;
pub const SOCK_RAW: c_int = 3;
pub const SOCK_RDM: c_int = 4;
pub const SOCK_CLOEXEC: c_int = 0x10000000;

pub const SA_SIGINFO: c_int = 0x0002;
pub const SA_NOCLDWAIT: c_int = 0x0020;
pub const SA_NODEFER: c_int = 0x0010;
pub const SA_RESETHAND: c_int = 0x0004;
pub const SA_NOCLDSTOP: c_int = 0x0001;

pub const SIGTTIN: c_int = 26;
pub const SIGTTOU: c_int = 27;
pub const SIGXCPU: c_int = 30;
pub const SIGXFSZ: c_int = 31;
pub const SIGVTALRM: c_int = 28;
pub const SIGPROF: c_int = 29;
pub const SIGWINCH: c_int = 20;
pub const SIGCHLD: c_int = 18;
pub const SIGBUS: c_int = 10;
pub const SIGUSR1: c_int = 16;
pub const SIGUSR2: c_int = 17;
pub const SIGCONT: c_int = 25;
pub const SIGSTOP: c_int = 23;
pub const SIGTSTP: c_int = 24;
pub const SIGURG: c_int = 21;
pub const SIGIO: c_int = SIGPOLL;
pub const SIGSYS: c_int = 12;
pub const SIGPOLL: c_int = 22;
pub const SIGPWR: c_int = 19;
pub const SIG_SETMASK: c_int = 2;
pub const SIG_BLOCK: c_int = 0;
pub const SIG_UNBLOCK: c_int = 1;

pub const POLLWRNORM: c_short = crate::POLLOUT;
pub const POLLWRBAND: c_short = 0x0010;

pub const F_SETLK: c_int = 106;
pub const F_SETLKW: c_int = 107;
pub const F_ALLOCSP: c_int = 110;
pub const F_FREESP: c_int = 111;
pub const F_GETLK: c_int = 114;

pub const F_RDLCK: c_int = 1;
pub const F_WRLCK: c_int = 2;
pub const F_UNLCK: c_int = 3;

pub const NCCS: usize = 40;

pub const MAP_ANON: c_int = MAP_ANONYMOUS;
pub const MAP_ANONYMOUS: c_int = 0x00080000;

pub const MCL_CURRENT: c_int = 0x000000001;
pub const MCL_FUTURE: c_int = 0x000000002;

pub const _TIO_CBAUD: crate::tcflag_t = 15;
pub const CBAUD: crate::tcflag_t = _TIO_CBAUD;
pub const TAB1: crate::tcflag_t = 0x0800;
pub const TAB2: crate::tcflag_t = 0x1000;
pub const TAB3: crate::tcflag_t = 0x1800;
pub const CR1: crate::tcflag_t = 0x200;
pub const CR2: crate::tcflag_t = 0x400;
pub const CR3: crate::tcflag_t = 0x600;
pub const FF1: crate::tcflag_t = 0x8000;
pub const BS1: crate::tcflag_t = 0x2000;
pub const VT1: crate::tcflag_t = 0x4000;
pub const VWERASE: usize = 14;
pub const VREPRINT: usize = 12;
pub const VSUSP: usize = 10;
pub const VSTART: usize = 8;
pub const VSTOP: usize = 9;
pub const VDISCARD: usize = 13;
pub const VTIME: usize = 17;
pub const IXON: crate::tcflag_t = 0x00000400;
pub const IXOFF: crate::tcflag_t = 0x00001000;
pub const ONLCR: crate::tcflag_t = 0x00000004;
pub const CSIZE: crate::tcflag_t = 0x00000030;
pub const CS6: crate::tcflag_t = 0x10;
pub const CS7: crate::tcflag_t = 0x20;
pub const CS8: crate::tcflag_t = 0x30;
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
pub const ECHOCTL: crate::tcflag_t = 0x00000200;
pub const ISIG: crate::tcflag_t = 0x00000001;
pub const ICANON: crate::tcflag_t = 0x00000002;
pub const NOFLSH: crate::tcflag_t = 0x00000080;
pub const OLCUC: crate::tcflag_t = 0x00000002;
pub const NLDLY: crate::tcflag_t = 0x00000100;
pub const CRDLY: crate::tcflag_t = 0x00000600;
pub const TABDLY: crate::tcflag_t = 0x00001800;
pub const BSDLY: crate::tcflag_t = 0x00002000;
pub const FFDLY: crate::tcflag_t = 0x00008000;
pub const VTDLY: crate::tcflag_t = 0x00004000;
pub const XTABS: crate::tcflag_t = 0x1800;

pub const B0: crate::speed_t = 0;
pub const B50: crate::speed_t = 1;
pub const B75: crate::speed_t = 2;
pub const B110: crate::speed_t = 3;
pub const B134: crate::speed_t = 4;
pub const B150: crate::speed_t = 5;
pub const B200: crate::speed_t = 6;
pub const B300: crate::speed_t = 7;
pub const B600: crate::speed_t = 8;
pub const B1200: crate::speed_t = 9;
pub const B1800: crate::speed_t = 10;
pub const B2400: crate::speed_t = 11;
pub const B4800: crate::speed_t = 12;
pub const B9600: crate::speed_t = 13;
pub const B19200: crate::speed_t = 14;
pub const B38400: crate::speed_t = 15;
pub const EXTA: crate::speed_t = 14;
pub const EXTB: crate::speed_t = 15;
pub const B57600: crate::speed_t = 57600;
pub const B115200: crate::speed_t = 115200;

pub const VEOL: usize = 5;
pub const VEOL2: usize = 6;
pub const VMIN: usize = 16;
pub const IEXTEN: crate::tcflag_t = 0x00008000;
pub const TOSTOP: crate::tcflag_t = 0x00000100;

pub const TCSANOW: c_int = 0x0001;
pub const TCSADRAIN: c_int = 0x0002;
pub const TCSAFLUSH: c_int = 0x0004;

pub const HW_MACHINE: c_int = 1;
pub const HW_MODEL: c_int = 2;
pub const HW_NCPU: c_int = 3;
pub const HW_BYTEORDER: c_int = 4;
pub const HW_PHYSMEM: c_int = 5;
pub const HW_USERMEM: c_int = 6;
pub const HW_PAGESIZE: c_int = 7;
pub const HW_DISKNAMES: c_int = 8;
pub const CTL_KERN: c_int = 1;
pub const CTL_VM: c_int = 2;
pub const CTL_VFS: c_int = 3;
pub const CTL_NET: c_int = 4;
pub const CTL_DEBUG: c_int = 5;
pub const CTL_HW: c_int = 6;
pub const CTL_MACHDEP: c_int = 7;
pub const CTL_USER: c_int = 8;

pub const DAY_1: crate::nl_item = 8;
pub const DAY_2: crate::nl_item = 9;
pub const DAY_3: crate::nl_item = 10;
pub const DAY_4: crate::nl_item = 11;
pub const DAY_5: crate::nl_item = 12;
pub const DAY_6: crate::nl_item = 13;
pub const DAY_7: crate::nl_item = 14;

pub const MON_1: crate::nl_item = 22;
pub const MON_2: crate::nl_item = 23;
pub const MON_3: crate::nl_item = 24;
pub const MON_4: crate::nl_item = 25;
pub const MON_5: crate::nl_item = 26;
pub const MON_6: crate::nl_item = 27;
pub const MON_7: crate::nl_item = 28;
pub const MON_8: crate::nl_item = 29;
pub const MON_9: crate::nl_item = 30;
pub const MON_10: crate::nl_item = 31;
pub const MON_11: crate::nl_item = 32;
pub const MON_12: crate::nl_item = 33;

pub const ABDAY_1: crate::nl_item = 15;
pub const ABDAY_2: crate::nl_item = 16;
pub const ABDAY_3: crate::nl_item = 17;
pub const ABDAY_4: crate::nl_item = 18;
pub const ABDAY_5: crate::nl_item = 19;
pub const ABDAY_6: crate::nl_item = 20;
pub const ABDAY_7: crate::nl_item = 21;

pub const ABMON_1: crate::nl_item = 34;
pub const ABMON_2: crate::nl_item = 35;
pub const ABMON_3: crate::nl_item = 36;
pub const ABMON_4: crate::nl_item = 37;
pub const ABMON_5: crate::nl_item = 38;
pub const ABMON_6: crate::nl_item = 39;
pub const ABMON_7: crate::nl_item = 40;
pub const ABMON_8: crate::nl_item = 41;
pub const ABMON_9: crate::nl_item = 42;
pub const ABMON_10: crate::nl_item = 43;
pub const ABMON_11: crate::nl_item = 44;
pub const ABMON_12: crate::nl_item = 45;

pub const AF_CCITT: c_int = 10;
pub const AF_CHAOS: c_int = 5;
pub const AF_CNT: c_int = 21;
pub const AF_COIP: c_int = 20;
pub const AF_DATAKIT: c_int = 9;
pub const AF_DECnet: c_int = 12;
pub const AF_DLI: c_int = 13;
pub const AF_E164: c_int = 26;
pub const AF_ECMA: c_int = 8;
pub const AF_HYLINK: c_int = 15;
pub const AF_IMPLINK: c_int = 3;
pub const AF_ISO: c_int = 7;
pub const AF_LAT: c_int = 14;
pub const AF_LINK: c_int = 18;
pub const AF_OSI: c_int = 7;
pub const AF_PUP: c_int = 4;
pub const ALT_DIGITS: crate::nl_item = 50;
pub const AM_STR: crate::nl_item = 6;
pub const B76800: crate::speed_t = 76800;

pub const BIOCFLUSH: c_int = 17000;
pub const BIOCGBLEN: c_int = 1074020966;
pub const BIOCGDLT: c_int = 1074020970;
pub const BIOCGHDRCMPLT: c_int = 1074020980;
pub const BIOCGRTIMEOUT: c_int = 1074807406;
pub const BIOCIMMEDIATE: c_int = -2147204496;
pub const BIOCPROMISC: c_int = 17001;
pub const BIOCSBLEN: c_int = -1073462682;
pub const BIOCSETF: c_int = -2146418073;
pub const BIOCSHDRCMPLT: c_int = -2147204491;
pub const BIOCSRTIMEOUT: c_int = -2146418067;
pub const BIOCVERSION: c_int = 1074020977;

pub const BPF_ALIGNMENT: usize = size_of::<c_long>();
pub const CHAR_BIT: usize = 8;
pub const CODESET: crate::nl_item = 1;
pub const CRNCYSTR: crate::nl_item = 55;

pub const D_FLAG_FILTER: c_int = 0x00000001;
pub const D_FLAG_STAT: c_int = 0x00000002;
pub const D_FLAG_STAT_FORM_MASK: c_int = 0x000000f0;
pub const D_FLAG_STAT_FORM_T32_2001: c_int = 0x00000010;
pub const D_FLAG_STAT_FORM_T32_2008: c_int = 0x00000020;
pub const D_FLAG_STAT_FORM_T64_2008: c_int = 0x00000030;
pub const D_FLAG_STAT_FORM_UNSET: c_int = 0x00000000;

pub const D_FMT: crate::nl_item = 3;
pub const D_GETFLAG: c_int = 1;
pub const D_SETFLAG: c_int = 2;
pub const D_T_FMT: crate::nl_item = 2;
pub const ERA: crate::nl_item = 46;
pub const ERA_D_FMT: crate::nl_item = 47;
pub const ERA_D_T_FMT: crate::nl_item = 48;
pub const ERA_T_FMT: crate::nl_item = 49;
pub const RADIXCHAR: crate::nl_item = 51;
pub const THOUSEP: crate::nl_item = 52;
pub const YESEXPR: crate::nl_item = 53;
pub const NOEXPR: crate::nl_item = 54;
pub const F_GETOWN: c_int = 35;

pub const FIONBIO: c_int = -2147195266;
pub const FIOASYNC: c_int = -2147195267;
pub const FIOCLEX: c_int = 26113;
pub const FIOGETOWN: c_int = 1074030203;
pub const FIONCLEX: c_int = 26114;
pub const FIONREAD: c_int = 1074030207;
pub const FIOSETOWN: c_int = -2147195268;

pub const F_SETOWN: c_int = 36;
pub const IFF_LINK0: c_int = 0x00001000;
pub const IFF_LINK1: c_int = 0x00002000;
pub const IFF_LINK2: c_int = 0x00004000;
pub const IFF_OACTIVE: c_int = 0x00000400;
pub const IFF_SIMPLEX: c_int = 0x00000800;
pub const IHFLOW: tcflag_t = 0x00000001;
pub const IIDLE: tcflag_t = 0x00000008;
pub const IP_RECVDSTADDR: c_int = 7;
pub const IP_RECVIF: c_int = 20;
pub const IPTOS_ECN_NOTECT: u8 = 0x00;
pub const IUCLC: tcflag_t = 0x00000200;
pub const IUTF8: tcflag_t = 0x0004000;

pub const KERN_ARGMAX: c_int = 8;
pub const KERN_BOOTTIME: c_int = 21;
pub const KERN_CLOCKRATE: c_int = 12;
pub const KERN_FILE: c_int = 15;
pub const KERN_HOSTID: c_int = 11;
pub const KERN_HOSTNAME: c_int = 10;
pub const KERN_JOB_CONTROL: c_int = 19;
pub const KERN_MAXFILES: c_int = 7;
pub const KERN_MAXPROC: c_int = 6;
pub const KERN_MAXVNODES: c_int = 5;
pub const KERN_NGROUPS: c_int = 18;
pub const KERN_OSRELEASE: c_int = 2;
pub const KERN_OSREV: c_int = 3;
pub const KERN_OSTYPE: c_int = 1;
pub const KERN_POSIX1: c_int = 17;
pub const KERN_PROC: c_int = 14;
pub const KERN_PROC_ALL: c_int = 0;
pub const KERN_PROC_PGRP: c_int = 2;
pub const KERN_PROC_PID: c_int = 1;
pub const KERN_PROC_RUID: c_int = 6;
pub const KERN_PROC_SESSION: c_int = 3;
pub const KERN_PROC_TTY: c_int = 4;
pub const KERN_PROC_UID: c_int = 5;
pub const KERN_PROF: c_int = 16;
pub const KERN_SAVED_IDS: c_int = 20;
pub const KERN_SECURELVL: c_int = 9;
pub const KERN_VERSION: c_int = 4;
pub const KERN_VNODE: c_int = 13;

pub const LC_ALL: c_int = 63;
pub const LC_COLLATE: c_int = 1;
pub const LC_CTYPE: c_int = 2;
pub const LC_MESSAGES: c_int = 32;
pub const LC_MONETARY: c_int = 4;
pub const LC_NUMERIC: c_int = 8;
pub const LC_TIME: c_int = 16;

pub const MAP_STACK: c_int = 0x00001000;
pub const MNT_NOEXEC: c_int = 0x02;
pub const MNT_NOSUID: c_int = 0x04;
pub const MNT_RDONLY: c_int = 0x01;

pub const NET_RT_DUMP: c_int = 1;
pub const NET_RT_FLAGS: c_int = 2;
pub const OHFLOW: tcflag_t = 0x00000002;
pub const P_ALL: idtype_t = 0;
pub const PARSTK: tcflag_t = 0x00000004;
pub const PF_CCITT: c_int = 10;
pub const PF_CHAOS: c_int = 5;
pub const PF_CNT: c_int = 21;
pub const PF_COIP: c_int = 20;
pub const PF_DATAKIT: c_int = 9;
pub const PF_DECnet: c_int = 12;
pub const PF_DLI: c_int = 13;
pub const PF_ECMA: c_int = 8;
pub const PF_HYLINK: c_int = 15;
pub const PF_IMPLINK: c_int = 3;
pub const PF_ISO: c_int = 7;
pub const PF_LAT: c_int = 14;
pub const PF_LINK: c_int = 18;
pub const PF_OSI: c_int = 7;
pub const PF_PIP: c_int = 25;
pub const PF_PUP: c_int = 4;
pub const PF_RTIP: c_int = 22;
pub const PF_XTP: c_int = 19;
pub const PM_STR: crate::nl_item = 7;
pub const POSIX_MADV_DONTNEED: c_int = 4;
pub const POSIX_MADV_NORMAL: c_int = 0;
pub const POSIX_MADV_RANDOM: c_int = 2;
pub const POSIX_MADV_SEQUENTIAL: c_int = 1;
pub const POSIX_MADV_WILLNEED: c_int = 3;
pub const _POSIX_VDISABLE: c_int = 0;
pub const P_PGID: idtype_t = 2;
pub const P_PID: idtype_t = 1;
pub const PRIO_PGRP: c_int = 1;
pub const PRIO_PROCESS: c_int = 0;
pub const PRIO_USER: c_int = 2;
pub const pseudo_AF_PIP: c_int = 25;
pub const pseudo_AF_RTIP: c_int = 22;
pub const pseudo_AF_XTP: c_int = 19;
pub const REG_ASSERT: c_int = 15;
pub const REG_ATOI: c_int = 255;
pub const REG_BACKR: c_int = 0x400;
pub const REG_BASIC: c_int = 0x00;
pub const REG_DUMP: c_int = 0x80;
pub const REG_EMPTY: c_int = 14;
pub const REG_INVARG: c_int = 16;
pub const REG_ITOA: c_int = 0o400;
pub const REG_LARGE: c_int = 0x200;
pub const REG_NOSPEC: c_int = 0x10;
pub const REG_OK: c_int = 0;
pub const REG_PEND: c_int = 0x20;
pub const REG_TRACE: c_int = 0x100;

pub const RLIMIT_AS: c_int = 6;
pub const RLIMIT_CORE: c_int = 4;
pub const RLIMIT_CPU: c_int = 0;
pub const RLIMIT_DATA: c_int = 2;
pub const RLIMIT_FSIZE: c_int = 1;
pub const RLIMIT_MEMLOCK: c_int = 7;
pub const RLIMIT_NOFILE: c_int = 5;
pub const RLIMIT_NPROC: c_int = 8;
pub const RLIMIT_RSS: c_int = 6;
pub const RLIMIT_STACK: c_int = 3;
pub const RLIMIT_VMEM: c_int = 6;
#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIM_NLIMITS: c_int = 14;

pub const SCHED_ADJTOHEAD: c_int = 5;
pub const SCHED_ADJTOTAIL: c_int = 6;
pub const SCHED_MAXPOLICY: c_int = 7;
pub const SCHED_SETPRIO: c_int = 7;
pub const SCHED_SPORADIC: c_int = 4;

pub const SHM_ANON: *mut c_char = -1isize as *mut c_char;
pub const SIGCLD: c_int = SIGCHLD;
pub const SIGDEADLK: c_int = 7;
pub const SIGEMT: c_int = 7;
pub const SIGEV_NONE: c_int = 0;
pub const SIGEV_SIGNAL: c_int = 129;
pub const SIGEV_THREAD: c_int = 135;
pub const SO_USELOOPBACK: c_int = 0x0040;
pub const _SS_ALIGNSIZE: usize = size_of::<i64>();
pub const _SS_MAXSIZE: usize = 128;
pub const _SS_PAD1SIZE: usize = _SS_ALIGNSIZE - 2;
pub const _SS_PAD2SIZE: usize = _SS_MAXSIZE - 2 - _SS_PAD1SIZE - _SS_ALIGNSIZE;
pub const TC_CPOSIX: tcflag_t = CLOCAL | CREAD | CSIZE | CSTOPB | HUPCL | PARENB | PARODD;
pub const TCGETS: c_int = 0x404c540d;
pub const TC_IPOSIX: tcflag_t =
    BRKINT | ICRNL | IGNBRK | IGNPAR | INLCR | INPCK | ISTRIP | IXOFF | IXON | PARMRK;
pub const TC_LPOSIX: tcflag_t =
    ECHO | ECHOE | ECHOK | ECHONL | ICANON | IEXTEN | ISIG | NOFLSH | TOSTOP;
pub const TC_OPOSIX: tcflag_t = OPOST;
pub const T_FMT_AMPM: crate::nl_item = 5;

pub const TIOCCBRK: c_int = 29818;
pub const TIOCCDTR: c_int = 29816;
pub const TIOCDRAIN: c_int = 29790;
pub const TIOCEXCL: c_int = 29709;
pub const TIOCFLUSH: c_int = -2147191792;
pub const TIOCGETA: c_int = 1078752275;
pub const TIOCGPGRP: c_int = 1074033783;
pub const TIOCGWINSZ: c_int = 1074295912;
pub const TIOCMBIC: c_int = -2147191701;
pub const TIOCMBIS: c_int = -2147191700;
pub const TIOCMGET: c_int = 1074033770;
pub const TIOCMSET: c_int = -2147191699;
pub const TIOCNOTTY: c_int = 29809;
pub const TIOCNXCL: c_int = 29710;
pub const TIOCOUTQ: c_int = 1074033779;
pub const TIOCPKT: c_int = -2147191696;
pub const TIOCPKT_DATA: c_int = 0x00;
pub const TIOCPKT_DOSTOP: c_int = 0x20;
pub const TIOCPKT_FLUSHREAD: c_int = 0x01;
pub const TIOCPKT_FLUSHWRITE: c_int = 0x02;
pub const TIOCPKT_IOCTL: c_int = 0x40;
pub const TIOCPKT_NOSTOP: c_int = 0x10;
pub const TIOCPKT_START: c_int = 0x08;
pub const TIOCPKT_STOP: c_int = 0x04;
pub const TIOCSBRK: c_int = 29819;
pub const TIOCSCTTY: c_int = 29793;
pub const TIOCSDTR: c_int = 29817;
pub const TIOCSETA: c_int = -2142473196;
pub const TIOCSETAF: c_int = -2142473194;
pub const TIOCSETAW: c_int = -2142473195;
pub const TIOCSPGRP: c_int = -2147191690;
pub const TIOCSTART: c_int = 29806;
pub const TIOCSTI: c_int = -2147388302;
pub const TIOCSTOP: c_int = 29807;
pub const TIOCSWINSZ: c_int = -2146929561;

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

pub const VDOWN: usize = 31;
pub const VINS: usize = 32;
pub const VDEL: usize = 33;
pub const VRUB: usize = 34;
pub const VCAN: usize = 35;
pub const VHOME: usize = 36;
pub const VEND: usize = 37;
pub const VSPARE3: usize = 38;
pub const VSPARE4: usize = 39;
pub const VSWTCH: usize = 7;
pub const VDSUSP: usize = 11;
pub const VFWD: usize = 18;
pub const VLOGIN: usize = 19;
pub const VPREFIX: usize = 20;
pub const VSUFFIX: usize = 24;
pub const VLEFT: usize = 28;
pub const VRIGHT: usize = 29;
pub const VUP: usize = 30;
pub const XCASE: tcflag_t = 0x00000004;

pub const PTHREAD_BARRIER_SERIAL_THREAD: c_int = -1;
pub const PTHREAD_CREATE_JOINABLE: c_int = 0x00;
pub const PTHREAD_CREATE_DETACHED: c_int = 0x01;

pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 1;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 2;
pub const PTHREAD_MUTEX_NORMAL: c_int = 3;
pub const PTHREAD_STACK_MIN: size_t = 256;
pub const PTHREAD_MUTEX_DEFAULT: c_int = 0;
pub const PTHREAD_MUTEX_STALLED: c_int = 0x00;
pub const PTHREAD_MUTEX_ROBUST: c_int = 0x10;
pub const PTHREAD_PROCESS_PRIVATE: c_int = 0x00;
pub const PTHREAD_PROCESS_SHARED: c_int = 0x01;

pub const PTHREAD_KEYS_MAX: usize = 128;

pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
    __u: 0x80000000,
    __owner: 0xffffffff,
};
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    __u: CLOCK_REALTIME as u32,
    __owner: 0xfffffffb,
};
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = pthread_rwlock_t {
    __active: 0,
    __blockedwriters: 0,
    __blockedreaders: 0,
    __heavy: 0,
    __lock: PTHREAD_MUTEX_INITIALIZER,
    __rcond: PTHREAD_COND_INITIALIZER,
    __wcond: PTHREAD_COND_INITIALIZER,
    __owner: -2i32 as c_uint,
    __spare: 0,
};

const_fn! {
    {const} fn _CMSG_ALIGN(len: usize) -> usize {
        len + size_of::<usize>() - 1 & !(size_of::<usize>() - 1)
    }

    {const} fn _ALIGN(p: usize, b: usize) -> usize {
        (p + b - 1) & !(b - 1)
    }
}

f! {
    pub fn CMSG_FIRSTHDR(mhdr: *const msghdr) -> *mut cmsghdr {
        if (*mhdr).msg_controllen as usize >= size_of::<cmsghdr>() {
            (*mhdr).msg_control as *mut cmsghdr
        } else {
            core::ptr::null_mut::<cmsghdr>()
        }
    }

    pub fn CMSG_NXTHDR(mhdr: *const crate::msghdr, cmsg: *const cmsghdr) -> *mut cmsghdr {
        let msg = _CMSG_ALIGN((*cmsg).cmsg_len as usize);
        let next = cmsg as usize + msg + _CMSG_ALIGN(size_of::<cmsghdr>());
        if next > (*mhdr).msg_control as usize + (*mhdr).msg_controllen as usize {
            core::ptr::null_mut::<cmsghdr>()
        } else {
            (cmsg as usize + msg) as *mut cmsghdr
        }
    }

    pub fn CMSG_DATA(cmsg: *const cmsghdr) -> *mut c_uchar {
        (cmsg as *mut c_uchar).offset(_CMSG_ALIGN(size_of::<cmsghdr>()) as isize)
    }

    pub {const} fn CMSG_LEN(length: c_uint) -> c_uint {
        _CMSG_ALIGN(size_of::<cmsghdr>()) as c_uint + length
    }

    pub {const} fn CMSG_SPACE(length: c_uint) -> c_uint {
        (_CMSG_ALIGN(size_of::<cmsghdr>()) + _CMSG_ALIGN(length as usize)) as c_uint
    }

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

    pub fn _DEXTRA_FIRST(_d: *const dirent) -> *mut crate::dirent_extra {
        let _f = &((*(_d)).d_name) as *const _;
        let _s = _d as usize;

        _ALIGN(_s + _f as usize - _s + (*_d).d_namelen as usize + 1, 8) as *mut crate::dirent_extra
    }

    pub fn _DEXTRA_VALID(_x: *const crate::dirent_extra, _d: *const dirent) -> bool {
        let sz = _x as usize - _d as usize + size_of::<crate::dirent_extra>();
        let rsz = (*_d).d_reclen as usize;

        if sz > rsz || sz + (*_x).d_datalen as usize > rsz {
            false
        } else {
            true
        }
    }

    pub fn _DEXTRA_NEXT(_x: *const crate::dirent_extra) -> *mut crate::dirent_extra {
        _ALIGN(
            _x as usize + size_of::<crate::dirent_extra>() + (*_x).d_datalen as usize,
            8,
        ) as *mut crate::dirent_extra
    }

    pub fn SOCKCREDSIZE(ngrps: usize) -> usize {
        let ngrps = if ngrps > 0 { ngrps - 1 } else { 0 };
        size_of::<sockcred>() + size_of::<crate::gid_t>() * ngrps
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

    pub {const} fn IPTOS_ECN(x: u8) -> u8 {
        x & crate::IPTOS_ECN_MASK
    }

    pub {const} fn makedev(major: c_uint, minor: c_uint) -> crate::dev_t {
        ((major << 10) | (minor)) as crate::dev_t
    }

    pub {const} fn major(dev: crate::dev_t) -> c_uint {
        ((dev as c_uint) >> 10) & 0x3f
    }

    pub {const} fn minor(dev: crate::dev_t) -> c_uint {
        (dev as c_uint) & 0x3ff
    }
}

cfg_if! {
    if #[cfg(not(target_env = "nto71_iosock"))] {
        extern "C" {
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
        }
    } else {
        extern "C" {
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
        }
    }
}

// Network related functions are provided by libsocket and regex
// functions are provided by libregex.
// In QNX <=7.0, libregex functions were included in libc itself.
#[link(name = "socket")]
#[cfg_attr(not(target_env = "nto70"), link(name = "regex"))]
extern "C" {
    pub fn sem_destroy(sem: *mut sem_t) -> c_int;
    pub fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;
    pub fn fdatasync(fd: c_int) -> c_int;
    pub fn getpriority(which: c_int, who: crate::id_t) -> c_int;
    pub fn setpriority(which: c_int, who: crate::id_t, prio: c_int) -> c_int;
    pub fn mkfifoat(dirfd: c_int, pathname: *const c_char, mode: mode_t) -> c_int;
    pub fn mknodat(__fd: c_int, pathname: *const c_char, mode: mode_t, dev: crate::dev_t) -> c_int;

    pub fn clock_getres(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn clock_gettime(clk_id: crate::clockid_t, tp: *mut crate::timespec) -> c_int;
    pub fn clock_settime(clk_id: crate::clockid_t, tp: *const crate::timespec) -> c_int;
    pub fn clock_getcpuclockid(pid: crate::pid_t, clk_id: *mut crate::clockid_t) -> c_int;

    pub fn pthread_attr_getstack(
        attr: *const crate::pthread_attr_t,
        stackaddr: *mut *mut c_void,
        stacksize: *mut size_t,
    ) -> c_int;
    pub fn memalign(align: size_t, size: size_t) -> *mut c_void;
    pub fn setgroups(ngroups: c_int, ptr: *const crate::gid_t) -> c_int;

    pub fn posix_fadvise(fd: c_int, offset: off_t, len: off_t, advise: c_int) -> c_int;
    pub fn futimens(fd: c_int, times: *const crate::timespec) -> c_int;
    pub fn nl_langinfo(item: crate::nl_item) -> *mut c_char;

    pub fn utimensat(
        dirfd: c_int,
        path: *const c_char,
        times: *const crate::timespec,
        flag: c_int,
    ) -> c_int;

    pub fn pthread_condattr_getclock(
        attr: *const pthread_condattr_t,
        clock_id: *mut clockid_t,
    ) -> c_int;
    pub fn pthread_condattr_setclock(
        attr: *mut pthread_condattr_t,
        clock_id: crate::clockid_t,
    ) -> c_int;
    pub fn pthread_condattr_setpshared(attr: *mut pthread_condattr_t, pshared: c_int) -> c_int;
    pub fn pthread_mutexattr_setpshared(attr: *mut pthread_mutexattr_t, pshared: c_int) -> c_int;
    pub fn pthread_rwlockattr_getpshared(
        attr: *const pthread_rwlockattr_t,
        val: *mut c_int,
    ) -> c_int;
    pub fn pthread_rwlockattr_setpshared(attr: *mut pthread_rwlockattr_t, val: c_int) -> c_int;
    pub fn ptsname_r(fd: c_int, buf: *mut c_char, buflen: size_t) -> *mut c_char;
    pub fn clearenv() -> c_int;
    pub fn waitid(
        idtype: idtype_t,
        id: id_t,
        infop: *mut crate::siginfo_t,
        options: c_int,
    ) -> c_int;
    pub fn wait4(
        pid: crate::pid_t,
        status: *mut c_int,
        options: c_int,
        rusage: *mut crate::rusage,
    ) -> crate::pid_t;

    // DIFF(main): changed to `*const *mut` in e77f551de9
    pub fn execvpe(
        file: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int;

    pub fn getifaddrs(ifap: *mut *mut crate::ifaddrs) -> c_int;
    pub fn freeifaddrs(ifa: *mut crate::ifaddrs);
    pub fn bind(
        socket: c_int,
        address: *const crate::sockaddr,
        address_len: crate::socklen_t,
    ) -> c_int;

    pub fn writev(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;
    pub fn readv(fd: c_int, iov: *const crate::iovec, iovcnt: c_int) -> ssize_t;

    pub fn sendmsg(fd: c_int, msg: *const crate::msghdr, flags: c_int) -> ssize_t;
    pub fn recvmsg(fd: c_int, msg: *mut crate::msghdr, flags: c_int) -> ssize_t;
    pub fn openpty(
        amaster: *mut c_int,
        aslave: *mut c_int,
        name: *mut c_char,
        termp: *mut termios,
        winp: *mut crate::winsize,
    ) -> c_int;
    pub fn forkpty(
        amaster: *mut c_int,
        name: *mut c_char,
        termp: *mut termios,
        winp: *mut crate::winsize,
    ) -> crate::pid_t;
    pub fn login_tty(fd: c_int) -> c_int;

    pub fn uname(buf: *mut crate::utsname) -> c_int;

    pub fn getpeereid(socket: c_int, euid: *mut crate::uid_t, egid: *mut crate::gid_t) -> c_int;

    pub fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;

    pub fn abs(i: c_int) -> c_int;
    pub fn labs(i: c_long) -> c_long;
    pub fn rand() -> c_int;
    pub fn srand(seed: c_uint);

    pub fn setpwent();
    pub fn endpwent();
    pub fn getpwent() -> *mut passwd;
    pub fn setgrent();
    pub fn endgrent();
    pub fn getgrent() -> *mut crate::group;
    pub fn setspent();
    pub fn endspent();

    pub fn shm_open(name: *const c_char, oflag: c_int, mode: mode_t) -> c_int;

    pub fn ftok(pathname: *const c_char, proj_id: c_int) -> crate::key_t;
    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;

    pub fn posix_fallocate(fd: c_int, offset: off_t, len: off_t) -> c_int;
    pub fn mkostemp(template: *mut c_char, flags: c_int) -> c_int;
    pub fn mkostemps(template: *mut c_char, suffixlen: c_int, flags: c_int) -> c_int;
    pub fn sigtimedwait(
        set: *const sigset_t,
        info: *mut siginfo_t,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn sigwaitinfo(set: *const sigset_t, info: *mut siginfo_t) -> c_int;
    pub fn pthread_setschedprio(native: crate::pthread_t, priority: c_int) -> c_int;

    pub fn if_nameindex() -> *mut if_nameindex;
    pub fn if_freenameindex(ptr: *mut if_nameindex);

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

    pub fn getdomainname(name: *mut c_char, len: size_t) -> c_int;
    pub fn setdomainname(name: *const c_char, len: size_t) -> c_int;
    pub fn sync();
    pub fn pthread_getschedparam(
        native: crate::pthread_t,
        policy: *mut c_int,
        param: *mut crate::sched_param,
    ) -> c_int;
    pub fn umount(target: *const c_char, flags: c_int) -> c_int;
    pub fn sched_get_priority_max(policy: c_int) -> c_int;
    pub fn settimeofday(tv: *const crate::timeval, tz: *const c_void) -> c_int;
    pub fn sched_rr_get_interval(pid: crate::pid_t, tp: *mut crate::timespec) -> c_int;
    pub fn sem_timedwait(sem: *mut sem_t, abstime: *const crate::timespec) -> c_int;
    pub fn sem_getvalue(sem: *mut sem_t, sval: *mut c_int) -> c_int;
    pub fn sched_setparam(pid: crate::pid_t, param: *const crate::sched_param) -> c_int;
    pub fn mount(
        special_device: *const c_char,
        mount_directory: *const c_char,
        flags: c_int,
        mount_type: *const c_char,
        mount_data: *const c_void,
        mount_datalen: c_int,
    ) -> c_int;
    pub fn sched_getparam(pid: crate::pid_t, param: *mut crate::sched_param) -> c_int;
    pub fn pthread_mutex_consistent(mutex: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_mutex_timedlock(
        lock: *mut pthread_mutex_t,
        abstime: *const crate::timespec,
    ) -> c_int;
    pub fn pthread_spin_init(lock: *mut crate::pthread_spinlock_t, pshared: c_int) -> c_int;
    pub fn pthread_spin_destroy(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_lock(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_trylock(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn pthread_spin_unlock(lock: *mut crate::pthread_spinlock_t) -> c_int;
    pub fn pthread_barrierattr_init(__attr: *mut crate::pthread_barrierattr_t) -> c_int;
    pub fn pthread_barrierattr_destroy(__attr: *mut crate::pthread_barrierattr_t) -> c_int;
    pub fn pthread_barrierattr_getpshared(
        __attr: *const crate::pthread_barrierattr_t,
        __pshared: *mut c_int,
    ) -> c_int;
    pub fn pthread_barrierattr_setpshared(
        __attr: *mut crate::pthread_barrierattr_t,
        __pshared: c_int,
    ) -> c_int;
    pub fn pthread_barrier_init(
        __barrier: *mut crate::pthread_barrier_t,
        __attr: *const crate::pthread_barrierattr_t,
        __count: c_uint,
    ) -> c_int;
    pub fn pthread_barrier_destroy(__barrier: *mut crate::pthread_barrier_t) -> c_int;
    pub fn pthread_barrier_wait(__barrier: *mut crate::pthread_barrier_t) -> c_int;

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
    pub fn pthread_condattr_getpshared(
        attr: *const pthread_condattr_t,
        pshared: *mut c_int,
    ) -> c_int;
    pub fn pthread_setschedparam(
        native: crate::pthread_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;
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
    pub fn getgrouplist(
        user: *const c_char,
        group: crate::gid_t,
        groups: *mut crate::gid_t,
        ngroups: *mut c_int,
    ) -> c_int;
    pub fn pthread_mutexattr_getpshared(
        attr: *const pthread_mutexattr_t,
        pshared: *mut c_int,
    ) -> c_int;
    pub fn pthread_mutexattr_getrobust(
        attr: *const pthread_mutexattr_t,
        robustness: *mut c_int,
    ) -> c_int;
    pub fn pthread_mutexattr_setrobust(attr: *mut pthread_mutexattr_t, robustness: c_int) -> c_int;
    pub fn pthread_create(
        native: *mut crate::pthread_t,
        attr: *const crate::pthread_attr_t,
        f: extern "C" fn(*mut c_void) -> *mut c_void,
        value: *mut c_void,
    ) -> c_int;
    pub fn getitimer(which: c_int, curr_value: *mut crate::itimerval) -> c_int;
    pub fn setitimer(
        which: c_int,
        value: *const crate::itimerval,
        ovalue: *mut crate::itimerval,
    ) -> c_int;
    pub fn posix_spawn(
        pid: *mut crate::pid_t,
        path: *const c_char,
        file_actions: *const crate::posix_spawn_file_actions_t,
        attrp: *const crate::posix_spawnattr_t,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
    ) -> c_int;
    pub fn posix_spawnp(
        pid: *mut crate::pid_t,
        file: *const c_char,
        file_actions: *const crate::posix_spawn_file_actions_t,
        attrp: *const crate::posix_spawnattr_t,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
    ) -> c_int;
    pub fn posix_spawnattr_init(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_destroy(attr: *mut posix_spawnattr_t) -> c_int;
    pub fn posix_spawnattr_getsigdefault(
        attr: *const posix_spawnattr_t,
        default: *mut crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_setsigdefault(
        attr: *mut posix_spawnattr_t,
        default: *const crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_getsigmask(
        attr: *const posix_spawnattr_t,
        default: *mut crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_setsigmask(
        attr: *mut posix_spawnattr_t,
        default: *const crate::sigset_t,
    ) -> c_int;
    pub fn posix_spawnattr_getflags(attr: *const posix_spawnattr_t, flags: *mut c_short) -> c_int;
    pub fn posix_spawnattr_setflags(attr: *mut posix_spawnattr_t, flags: c_short) -> c_int;
    pub fn posix_spawnattr_getpgroup(
        attr: *const posix_spawnattr_t,
        flags: *mut crate::pid_t,
    ) -> c_int;
    pub fn posix_spawnattr_setpgroup(attr: *mut posix_spawnattr_t, flags: crate::pid_t) -> c_int;
    pub fn posix_spawnattr_getschedpolicy(
        attr: *const posix_spawnattr_t,
        flags: *mut c_int,
    ) -> c_int;
    pub fn posix_spawnattr_setschedpolicy(attr: *mut posix_spawnattr_t, flags: c_int) -> c_int;
    pub fn posix_spawnattr_getschedparam(
        attr: *const posix_spawnattr_t,
        param: *mut crate::sched_param,
    ) -> c_int;
    pub fn posix_spawnattr_setschedparam(
        attr: *mut posix_spawnattr_t,
        param: *const crate::sched_param,
    ) -> c_int;

    pub fn posix_spawn_file_actions_init(actions: *mut posix_spawn_file_actions_t) -> c_int;
    pub fn posix_spawn_file_actions_destroy(actions: *mut posix_spawn_file_actions_t) -> c_int;
    pub fn posix_spawn_file_actions_addopen(
        actions: *mut posix_spawn_file_actions_t,
        fd: c_int,
        path: *const c_char,
        oflag: c_int,
        mode: mode_t,
    ) -> c_int;
    pub fn posix_spawn_file_actions_addclose(
        actions: *mut posix_spawn_file_actions_t,
        fd: c_int,
    ) -> c_int;
    pub fn posix_spawn_file_actions_adddup2(
        actions: *mut posix_spawn_file_actions_t,
        fd: c_int,
        newfd: c_int,
    ) -> c_int;
    pub fn popen(command: *const c_char, mode: *const c_char) -> *mut crate::FILE;
    pub fn faccessat(dirfd: c_int, pathname: *const c_char, mode: c_int, flags: c_int) -> c_int;
    pub fn inotify_rm_watch(fd: c_int, wd: c_int) -> c_int;
    pub fn inotify_init() -> c_int;
    pub fn inotify_add_watch(fd: c_int, path: *const c_char, mask: u32) -> c_int;

    pub fn gettid() -> crate::pid_t;

    pub fn pthread_getcpuclockid(thread: crate::pthread_t, clk_id: *mut crate::clockid_t) -> c_int;

    pub fn getnameinfo(
        sa: *const crate::sockaddr,
        salen: crate::socklen_t,
        host: *mut c_char,
        hostlen: crate::socklen_t,
        serv: *mut c_char,
        servlen: crate::socklen_t,
        flags: c_int,
    ) -> c_int;

    pub fn mallopt(param: c_int, value: i64) -> c_int;
    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut c_void) -> c_int;

    pub fn ctermid(s: *mut c_char) -> *mut c_char;
    pub fn ioctl(fd: c_int, request: c_int, ...) -> c_int;

    pub fn mallinfo() -> crate::mallinfo;
    pub fn getpwent_r(
        pwd: *mut crate::passwd,
        buf: *mut c_char,
        __bufsize: c_int,
        __result: *mut *mut crate::passwd,
    ) -> c_int;
    pub fn pthread_getname_np(thread: crate::pthread_t, name: *mut c_char, len: c_int) -> c_int;
    pub fn pthread_setname_np(thread: crate::pthread_t, name: *const c_char) -> c_int;

    pub fn sysctl(
        _: *const c_int,
        _: c_uint,
        _: *mut c_void,
        _: *mut size_t,
        _: *const c_void,
        _: size_t,
    ) -> c_int;

    pub fn getrlimit(resource: c_int, rlim: *mut crate::rlimit) -> c_int;
    pub fn setrlimit(resource: c_int, rlp: *const crate::rlimit) -> c_int;

    pub fn lio_listio(
        __mode: c_int,
        __list: *const *mut aiocb,
        __nent: c_int,
        __sig: *mut sigevent,
    ) -> c_int;

    pub fn dl_iterate_phdr(
        callback: Option<
            unsafe extern "C" fn(
                // The original .h file declares this as *const, but for consistency with other platforms,
                // changing this to *mut to make it easier to use.
                // Maybe in v0.3 all platforms should use this as a *const.
                info: *mut dl_phdr_info,
                size: size_t,
                data: *mut c_void,
            ) -> c_int,
        >,
        data: *mut c_void,
    ) -> c_int;

    pub fn memset_s(s: *mut c_void, smax: size_t, c: c_int, n: size_t) -> c_int;

    pub fn regcomp(__preg: *mut crate::regex_t, __pattern: *const c_char, __cflags: c_int)
        -> c_int;
    pub fn regexec(
        __preg: *const crate::regex_t,
        __str: *const c_char,
        __nmatch: size_t,
        __pmatch: *mut crate::regmatch_t,
        __eflags: c_int,
    ) -> c_int;
    pub fn regerror(
        __errcode: c_int,
        __preg: *const crate::regex_t,
        __errbuf: *mut c_char,
        __errbuf_size: size_t,
    ) -> size_t;
    pub fn regfree(__preg: *mut crate::regex_t);
    pub fn dirfd(__dirp: *mut crate::DIR) -> c_int;
    pub fn dircntl(dir: *mut crate::DIR, cmd: c_int, ...) -> c_int;

    pub fn aio_cancel(__fd: c_int, __aiocbp: *mut crate::aiocb) -> c_int;
    pub fn aio_error(__aiocbp: *const crate::aiocb) -> c_int;
    pub fn aio_fsync(__operation: c_int, __aiocbp: *mut crate::aiocb) -> c_int;
    pub fn aio_read(__aiocbp: *mut crate::aiocb) -> c_int;
    pub fn aio_return(__aiocpb: *mut crate::aiocb) -> ssize_t;
    pub fn aio_suspend(
        __list: *const *const crate::aiocb,
        __nent: c_int,
        __timeout: *const crate::timespec,
    ) -> c_int;
    pub fn aio_write(__aiocpb: *mut crate::aiocb) -> c_int;

    pub fn mq_close(__mqdes: crate::mqd_t) -> c_int;
    pub fn mq_getattr(__mqdes: crate::mqd_t, __mqstat: *mut crate::mq_attr) -> c_int;
    pub fn mq_notify(__mqdes: crate::mqd_t, __notification: *const crate::sigevent) -> c_int;
    pub fn mq_open(__name: *const c_char, __oflag: c_int, ...) -> crate::mqd_t;
    pub fn mq_receive(
        __mqdes: crate::mqd_t,
        __msg_ptr: *mut c_char,
        __msg_len: size_t,
        __msg_prio: *mut c_uint,
    ) -> ssize_t;
    pub fn mq_send(
        __mqdes: crate::mqd_t,
        __msg_ptr: *const c_char,
        __msg_len: size_t,
        __msg_prio: c_uint,
    ) -> c_int;
    pub fn mq_setattr(
        __mqdes: crate::mqd_t,
        __mqstat: *const mq_attr,
        __omqstat: *mut mq_attr,
    ) -> c_int;
    pub fn mq_timedreceive(
        __mqdes: crate::mqd_t,
        __msg_ptr: *mut c_char,
        __msg_len: size_t,
        __msg_prio: *mut c_uint,
        __abs_timeout: *const crate::timespec,
    ) -> ssize_t;
    pub fn mq_timedsend(
        __mqdes: crate::mqd_t,
        __msg_ptr: *const c_char,
        __msg_len: size_t,
        __msg_prio: c_uint,
        __abs_timeout: *const crate::timespec,
    ) -> c_int;
    pub fn mq_unlink(__name: *const c_char) -> c_int;
    pub fn __get_errno_ptr() -> *mut c_int;

    // System page, see https://www.qnx.com/developers/docs/7.1#com.qnx.doc.neutrino.building/topic/syspage/syspage_about.html
    pub static mut _syspage_ptr: *mut syspage_entry;

    // Function on the stack after a call to pthread_create().  This is used
    // as a sentinel to work around an infitnite loop in the unwinding code.
    pub fn __my_thread_exit(value_ptr: *mut *const c_void);
}

// Models the implementation in stdlib.h.  Ctest will fail if trying to use the
// default symbol from libc
pub unsafe fn atexit(cb: extern "C" fn()) -> c_int {
    extern "C" {
        static __dso_handle: *mut c_void;
        pub fn __cxa_atexit(cb: extern "C" fn(), __arg: *mut c_void, __dso: *mut c_void) -> c_int;
    }
    __cxa_atexit(cb, 0 as *mut c_void, __dso_handle)
}

impl siginfo_t {
    pub unsafe fn si_addr(&self) -> *mut c_void {
        #[repr(C)]
        struct siginfo_si_addr {
            _pad: [u8; 32],
            si_addr: *mut c_void,
        }
        (*(self as *const siginfo_t as *const siginfo_si_addr)).si_addr
    }

    pub unsafe fn si_value(&self) -> crate::sigval {
        #[repr(C)]
        struct siginfo_si_value {
            _pad: [u8; 32],
            si_value: crate::sigval,
        }
        (*(self as *const siginfo_t as *const siginfo_si_value)).si_value
    }

    pub unsafe fn si_pid(&self) -> crate::pid_t {
        #[repr(C)]
        struct siginfo_si_pid {
            _pad: [u8; 16],
            si_pid: crate::pid_t,
        }
        (*(self as *const siginfo_t as *const siginfo_si_pid)).si_pid
    }

    pub unsafe fn si_uid(&self) -> crate::uid_t {
        #[repr(C)]
        struct siginfo_si_uid {
            _pad: [u8; 24],
            si_uid: crate::uid_t,
        }
        (*(self as *const siginfo_t as *const siginfo_si_uid)).si_uid
    }

    pub unsafe fn si_status(&self) -> c_int {
        #[repr(C)]
        struct siginfo_si_status {
            _pad: [u8; 28],
            si_status: c_int,
        }
        (*(self as *const siginfo_t as *const siginfo_si_status)).si_status
    }
}

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use self::aarch64::*;
    } else {
        panic!("Unsupported arch");
    }
}

mod neutrino;
pub use self::neutrino::*;
