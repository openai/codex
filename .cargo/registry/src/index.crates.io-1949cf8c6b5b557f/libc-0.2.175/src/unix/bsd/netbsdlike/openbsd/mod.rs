use crate::prelude::*;
use crate::unix::bsd::O_SYNC;
use crate::{cmsghdr, off_t};

pub type clock_t = i64;
pub type suseconds_t = c_long;
pub type dev_t = i32;
pub type sigset_t = c_uint;
pub type blksize_t = i32;
pub type fsblkcnt_t = u64;
pub type fsfilcnt_t = u64;
pub type idtype_t = c_uint;
pub type pthread_attr_t = *mut c_void;
pub type pthread_mutex_t = *mut c_void;
pub type pthread_mutexattr_t = *mut c_void;
pub type pthread_cond_t = *mut c_void;
pub type pthread_condattr_t = *mut c_void;
pub type pthread_rwlock_t = *mut c_void;
pub type pthread_rwlockattr_t = *mut c_void;
pub type pthread_spinlock_t = crate::uintptr_t;
pub type caddr_t = *mut c_char;

// elf.h

pub type Elf32_Addr = u32;
pub type Elf32_Half = u16;
pub type Elf32_Lword = u64;
pub type Elf32_Off = u32;
pub type Elf32_Sword = i32;
pub type Elf32_Word = u32;

pub type Elf64_Addr = u64;
pub type Elf64_Half = u16;
pub type Elf64_Lword = u64;
pub type Elf64_Off = u64;
pub type Elf64_Sword = i32;
pub type Elf64_Sxword = i64;
pub type Elf64_Word = u32;
pub type Elf64_Xword = u64;

// search.h

pub type ENTRY = entry;
pub type ACTION = c_uint;

// spawn.h
pub type posix_spawnattr_t = *mut c_void;
pub type posix_spawn_file_actions_t = *mut c_void;

cfg_if! {
    if #[cfg(target_pointer_width = "64")] {
        type Elf_Addr = Elf64_Addr;
        type Elf_Half = Elf64_Half;
        type Elf_Phdr = Elf64_Phdr;
    } else if #[cfg(target_pointer_width = "32")] {
        type Elf_Addr = Elf32_Addr;
        type Elf_Half = Elf32_Half;
        type Elf_Phdr = Elf32_Phdr;
    }
}

s! {
    pub struct ip_mreqn {
        pub imr_multiaddr: in_addr,
        pub imr_address: in_addr,
        pub imr_ifindex: c_int,
    }

    pub struct glob_t {
        pub gl_pathc: size_t,
        pub gl_matchc: size_t,
        pub gl_offs: size_t,
        pub gl_flags: c_int,
        pub gl_pathv: *mut *mut c_char,
        __unused1: *mut c_void,
        __unused2: *mut c_void,
        __unused3: *mut c_void,
        __unused4: *mut c_void,
        __unused5: *mut c_void,
        __unused6: *mut c_void,
        __unused7: *mut c_void,
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

    pub struct ufs_args {
        pub fspec: *mut c_char,
        pub export_info: export_args,
    }

    pub struct mfs_args {
        pub fspec: *mut c_char,
        pub export_info: export_args,
        // https://github.com/openbsd/src/blob/HEAD/sys/sys/types.h#L134
        pub base: *mut c_char,
        pub size: c_ulong,
    }

    pub struct iso_args {
        pub fspec: *mut c_char,
        pub export_info: export_args,
        pub flags: c_int,
        pub sess: c_int,
    }

    pub struct nfs_args {
        pub version: c_int,
        pub addr: *mut crate::sockaddr,
        pub addrlen: c_int,
        pub sotype: c_int,
        pub proto: c_int,
        pub fh: *mut c_uchar,
        pub fhsize: c_int,
        pub flags: c_int,
        pub wsize: c_int,
        pub rsize: c_int,
        pub readdirsize: c_int,
        pub timeo: c_int,
        pub retrans: c_int,
        pub maxgrouplist: c_int,
        pub readahead: c_int,
        pub leaseterm: c_int,
        pub deadthresh: c_int,
        pub hostname: *mut c_char,
        pub acregmin: c_int,
        pub acregmax: c_int,
        pub acdirmin: c_int,
        pub acdirmax: c_int,
    }

    pub struct msdosfs_args {
        pub fspec: *mut c_char,
        pub export_info: export_args,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub mask: crate::mode_t,
        pub flags: c_int,
    }

    pub struct ntfs_args {
        pub fspec: *mut c_char,
        pub export_info: export_args,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub mode: crate::mode_t,
        pub flag: c_ulong,
    }

    pub struct udf_args {
        pub fspec: *mut c_char,
        pub lastblock: u32,
    }

    pub struct tmpfs_args {
        pub ta_version: c_int,
        pub ta_nodes_max: crate::ino_t,
        pub ta_size_max: off_t,
        pub ta_root_uid: crate::uid_t,
        pub ta_root_gid: crate::gid_t,
        pub ta_root_mode: crate::mode_t,
    }

    pub struct fusefs_args {
        pub name: *mut c_char,
        pub fd: c_int,
        pub max_read: c_int,
        pub allow_other: c_int,
    }

    pub struct xucred {
        pub cr_uid: crate::uid_t,
        pub cr_gid: crate::gid_t,
        pub cr_ngroups: c_short,
        //https://github.com/openbsd/src/blob/HEAD/sys/sys/syslimits.h#L44
        pub cr_groups: [crate::gid_t; 16],
    }

    pub struct export_args {
        pub ex_flags: c_int,
        pub ex_root: crate::uid_t,
        pub ex_anon: xucred,
        pub ex_addr: *mut crate::sockaddr,
        pub ex_addrlen: c_int,
        pub ex_mask: *mut crate::sockaddr,
        pub ex_masklen: c_int,
    }

    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct in_addr {
        pub s_addr: crate::in_addr_t,
    }

    pub struct sockaddr_in {
        pub sin_len: u8,
        pub sin_family: crate::sa_family_t,
        pub sin_port: crate::in_port_t,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [i8; 8],
    }

    pub struct splice {
        pub sp_fd: c_int,
        pub sp_max: off_t,
        pub sp_idle: crate::timeval,
    }

    pub struct kevent {
        pub ident: crate::uintptr_t,
        pub filter: c_short,
        pub flags: c_ushort,
        pub fflags: c_uint,
        pub data: i64,
        pub udata: *mut c_void,
    }

    pub struct stat {
        pub st_mode: crate::mode_t,
        pub st_dev: crate::dev_t,
        pub st_ino: crate::ino_t,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        pub st_atime: crate::time_t,
        pub st_atime_nsec: c_long,
        pub st_mtime: crate::time_t,
        pub st_mtime_nsec: c_long,
        pub st_ctime: crate::time_t,
        pub st_ctime_nsec: c_long,
        pub st_size: off_t,
        pub st_blocks: crate::blkcnt_t,
        pub st_blksize: crate::blksize_t,
        pub st_flags: u32,
        pub st_gen: u32,
        pub st_birthtime: crate::time_t,
        pub st_birthtime_nsec: c_long,
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
    }

    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        pub ai_addrlen: crate::socklen_t,
        pub ai_addr: *mut crate::sockaddr,
        pub ai_canonname: *mut c_char,
        pub ai_next: *mut crate::addrinfo,
    }

    pub struct Dl_info {
        pub dli_fname: *const c_char,
        pub dli_fbase: *mut c_void,
        pub dli_sname: *const c_char,
        pub dli_saddr: *mut c_void,
    }

    pub struct if_data {
        pub ifi_type: c_uchar,
        pub ifi_addrlen: c_uchar,
        pub ifi_hdrlen: c_uchar,
        pub ifi_link_state: c_uchar,
        pub ifi_mtu: u32,
        pub ifi_metric: u32,
        pub ifi_rdomain: u32,
        pub ifi_baudrate: u64,
        pub ifi_ipackets: u64,
        pub ifi_ierrors: u64,
        pub ifi_opackets: u64,
        pub ifi_oerrors: u64,
        pub ifi_collisions: u64,
        pub ifi_ibytes: u64,
        pub ifi_obytes: u64,
        pub ifi_imcasts: u64,
        pub ifi_omcasts: u64,
        pub ifi_iqdrops: u64,
        pub ifi_oqdrops: u64,
        pub ifi_noproto: u64,
        pub ifi_capabilities: u32,
        pub ifi_lastchange: crate::timeval,
    }

    pub struct if_msghdr {
        pub ifm_msglen: c_ushort,
        pub ifm_version: c_uchar,
        pub ifm_type: c_uchar,
        pub ifm_hdrlen: c_ushort,
        pub ifm_index: c_ushort,
        pub ifm_tableid: c_ushort,
        pub ifm_pad1: c_uchar,
        pub ifm_pad2: c_uchar,
        pub ifm_addrs: c_int,
        pub ifm_flags: c_int,
        pub ifm_xflags: c_int,
        pub ifm_data: if_data,
    }

    pub struct sockaddr_dl {
        pub sdl_len: c_uchar,
        pub sdl_family: c_uchar,
        pub sdl_index: c_ushort,
        pub sdl_type: c_uchar,
        pub sdl_nlen: c_uchar,
        pub sdl_alen: c_uchar,
        pub sdl_slen: c_uchar,
        pub sdl_data: [c_char; 24],
    }

    pub struct sockpeercred {
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub pid: crate::pid_t,
    }

    pub struct arphdr {
        pub ar_hrd: u16,
        pub ar_pro: u16,
        pub ar_hln: u8,
        pub ar_pln: u8,
        pub ar_op: u16,
    }

    pub struct shmid_ds {
        pub shm_perm: crate::ipc_perm,
        pub shm_segsz: c_int,
        pub shm_lpid: crate::pid_t,
        pub shm_cpid: crate::pid_t,
        pub shm_nattch: c_short,
        pub shm_atime: crate::time_t,
        __shm_atimensec: c_long,
        pub shm_dtime: crate::time_t,
        __shm_dtimensec: c_long,
        pub shm_ctime: crate::time_t,
        __shm_ctimensec: c_long,
        pub shm_internal: *mut c_void,
    }

    // elf.h
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
        pub dlpi_addr: Elf_Addr,
        pub dlpi_name: *const c_char,
        pub dlpi_phdr: *const Elf_Phdr,
        pub dlpi_phnum: Elf_Half,
    }

    // sys/sysctl.h
    pub struct kinfo_proc {
        pub p_forw: u64,
        pub p_back: u64,
        pub p_paddr: u64,
        pub p_addr: u64,
        pub p_fd: u64,
        pub p_stats: u64,
        pub p_limit: u64,
        pub p_vmspace: u64,
        pub p_sigacts: u64,
        pub p_sess: u64,
        pub p_tsess: u64,
        pub p_ru: u64,
        pub p_eflag: i32,
        pub p_exitsig: i32,
        pub p_flag: i32,
        pub p_pid: i32,
        pub p_ppid: i32,
        pub p_sid: i32,
        pub p__pgid: i32,
        pub p_tpgid: i32,
        pub p_uid: u32,
        pub p_ruid: u32,
        pub p_gid: u32,
        pub p_rgid: u32,
        pub p_groups: [u32; KI_NGROUPS as usize],
        pub p_ngroups: i16,
        pub p_jobc: i16,
        pub p_tdev: u32,
        pub p_estcpu: u32,
        pub p_rtime_sec: u32,
        pub p_rtime_usec: u32,
        pub p_cpticks: i32,
        pub p_pctcpu: u32,
        pub p_swtime: u32,
        pub p_slptime: u32,
        pub p_schedflags: i32,
        pub p_uticks: u64,
        pub p_sticks: u64,
        pub p_iticks: u64,
        pub p_tracep: u64,
        pub p_traceflag: i32,
        pub p_holdcnt: i32,
        pub p_siglist: i32,
        pub p_sigmask: u32,
        pub p_sigignore: u32,
        pub p_sigcatch: u32,
        pub p_stat: i8,
        pub p_priority: u8,
        pub p_usrpri: u8,
        pub p_nice: u8,
        pub p_xstat: u16,
        pub p_spare: u16,
        pub p_comm: [c_char; KI_MAXCOMLEN as usize],
        pub p_wmesg: [c_char; KI_WMESGLEN as usize],
        pub p_wchan: u64,
        pub p_login: [c_char; KI_MAXLOGNAME as usize],
        pub p_vm_rssize: i32,
        pub p_vm_tsize: i32,
        pub p_vm_dsize: i32,
        pub p_vm_ssize: i32,
        pub p_uvalid: i64,
        pub p_ustart_sec: u64,
        pub p_ustart_usec: u32,
        pub p_uutime_sec: u32,
        pub p_uutime_usec: u32,
        pub p_ustime_sec: u32,
        pub p_ustime_usec: u32,
        pub p_uru_maxrss: u64,
        pub p_uru_ixrss: u64,
        pub p_uru_idrss: u64,
        pub p_uru_isrss: u64,
        pub p_uru_minflt: u64,
        pub p_uru_majflt: u64,
        pub p_uru_nswap: u64,
        pub p_uru_inblock: u64,
        pub p_uru_oublock: u64,
        pub p_uru_msgsnd: u64,
        pub p_uru_msgrcv: u64,
        pub p_uru_nsignals: u64,
        pub p_uru_nvcsw: u64,
        pub p_uru_nivcsw: u64,
        pub p_uctime_sec: u32,
        pub p_uctime_usec: u32,
        pub p_psflags: u32,
        pub p_acflag: u32,
        pub p_svuid: u32,
        pub p_svgid: u32,
        pub p_emul: [c_char; KI_EMULNAMELEN as usize],
        pub p_rlim_rss_cur: u64,
        pub p_cpuid: u64,
        pub p_vm_map_size: u64,
        pub p_tid: i32,
        pub p_rtableid: u32,
        pub p_pledge: u64,
        pub p_name: [c_char; KI_MAXCOMLEN as usize],
    }

    pub struct kinfo_vmentry {
        pub kve_start: c_ulong,
        pub kve_end: c_ulong,
        pub kve_guard: c_ulong,
        pub kve_fspace: c_ulong,
        pub kve_fspace_augment: c_ulong,
        pub kve_offset: u64,
        pub kve_wired_count: c_int,
        pub kve_etype: c_int,
        pub kve_protection: c_int,
        pub kve_max_protection: c_int,
        pub kve_advice: c_int,
        pub kve_inheritance: c_int,
        pub kve_flags: u8,
    }

    pub struct ptrace_state {
        pub pe_report_event: c_int,
        pub pe_other_pid: crate::pid_t,
        pub pe_tid: crate::pid_t,
    }

    pub struct ptrace_thread_state {
        pub pts_tid: crate::pid_t,
    }

    // search.h
    pub struct entry {
        pub key: *mut c_char,
        pub data: *mut c_void,
    }

    pub struct ifreq {
        pub ifr_name: [c_char; crate::IFNAMSIZ],
        pub ifr_ifru: __c_anonymous_ifr_ifru,
    }

    pub struct tcp_info {
        pub tcpi_state: u8,
        pub __tcpi_ca_state: u8,
        pub __tcpi_retransmits: u8,
        pub __tcpi_probes: u8,
        pub __tcpi_backoff: u8,
        pub tcpi_options: u8,
        pub tcpi_snd_wscale: u8,
        pub tcpi_rcv_wscale: u8,
        pub tcpi_rto: u32,
        pub __tcpi_ato: u32,
        pub tcpi_snd_mss: u32,
        pub tcpi_rcv_mss: u32,
        pub __tcpi_unacked: u32,
        pub __tcpi_sacked: u32,
        pub __tcpi_lost: u32,
        pub __tcpi_retrans: u32,
        pub __tcpi_fackets: u32,
        pub tcpi_last_data_sent: u32,
        pub tcpi_last_ack_sent: u32,
        pub tcpi_last_data_recv: u32,
        pub tcpi_last_ack_recv: u32,
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
        pub tcpi_snd_nxt: u32,
        pub tcpi_rcv_nxt: u32,
        pub tcpi_toe_tid: u32,
        pub tcpi_snd_rexmitpack: u32,
        pub tcpi_rcv_ooopack: u32,
        pub tcpi_snd_zerowin: u32,
        pub tcpi_rttmin: u32,
        pub tcpi_max_sndwnd: u32,
        pub tcpi_rcv_adv: u32,
        pub tcpi_rcv_up: u32,
        pub tcpi_snd_una: u32,
        pub tcpi_snd_up: u32,
        pub tcpi_snd_wl1: u32,
        pub tcpi_snd_wl2: u32,
        pub tcpi_snd_max: u32,
        pub tcpi_ts_recent: u32,
        pub tcpi_ts_recent_age: u32,
        pub tcpi_rfbuf_cnt: u32,
        pub tcpi_rfbuf_ts: u32,
        pub tcpi_so_rcv_sb_cc: u32,
        pub tcpi_so_rcv_sb_hiwat: u32,
        pub tcpi_so_rcv_sb_lowat: u32,
        pub tcpi_so_rcv_sb_wat: u32,
        pub tcpi_so_snd_sb_cc: u32,
        pub tcpi_so_snd_sb_hiwat: u32,
        pub tcpi_so_snd_sb_lowat: u32,
        pub tcpi_so_snd_sb_wat: u32,
    }
}

impl siginfo_t {
    pub unsafe fn si_addr(&self) -> *mut c_char {
        self.si_addr
    }

    pub unsafe fn si_code(&self) -> c_int {
        self.si_code
    }

    pub unsafe fn si_errno(&self) -> c_int {
        self.si_errno
    }

    pub unsafe fn si_pid(&self) -> crate::pid_t {
        #[repr(C)]
        struct siginfo_timer {
            _si_signo: c_int,
            _si_code: c_int,
            _si_errno: c_int,
            _pad: [c_int; SI_PAD],
            _pid: crate::pid_t,
        }
        (*(self as *const siginfo_t).cast::<siginfo_timer>())._pid
    }

    pub unsafe fn si_uid(&self) -> crate::uid_t {
        #[repr(C)]
        struct siginfo_timer {
            _si_signo: c_int,
            _si_code: c_int,
            _si_errno: c_int,
            _pad: [c_int; SI_PAD],
            _pid: crate::pid_t,
            _uid: crate::uid_t,
        }
        (*(self as *const siginfo_t).cast::<siginfo_timer>())._uid
    }

    pub unsafe fn si_value(&self) -> crate::sigval {
        #[repr(C)]
        struct siginfo_timer {
            _si_signo: c_int,
            _si_code: c_int,
            _si_errno: c_int,
            _pad: [c_int; SI_PAD],
            _pid: crate::pid_t,
            _uid: crate::uid_t,
            value: crate::sigval,
        }
        (*(self as *const siginfo_t).cast::<siginfo_timer>()).value
    }
}

s_no_extra_traits! {
    pub struct dirent {
        pub d_fileno: crate::ino_t,
        pub d_off: off_t,
        pub d_reclen: u16,
        pub d_type: u8,
        pub d_namlen: u8,
        __d_padding: [u8; 4],
        pub d_name: [c_char; 256],
    }

    pub struct sockaddr_storage {
        pub ss_len: u8,
        pub ss_family: crate::sa_family_t,
        __ss_pad1: [u8; 6],
        __ss_pad2: i64,
        __ss_pad3: [u8; 240],
    }

    pub struct siginfo_t {
        pub si_signo: c_int,
        pub si_code: c_int,
        pub si_errno: c_int,
        pub si_addr: *mut c_char,
        #[cfg(target_pointer_width = "32")]
        __pad: [u8; 112],
        #[cfg(target_pointer_width = "64")]
        __pad: [u8; 108],
    }

    pub struct lastlog {
        ll_time: crate::time_t,
        ll_line: [c_char; UT_LINESIZE],
        ll_host: [c_char; UT_HOSTSIZE],
    }

    pub struct utmp {
        pub ut_line: [c_char; UT_LINESIZE],
        pub ut_name: [c_char; UT_NAMESIZE],
        pub ut_host: [c_char; UT_HOSTSIZE],
        pub ut_time: crate::time_t,
    }

    pub union mount_info {
        pub ufs_args: ufs_args,
        pub mfs_args: mfs_args,
        pub nfs_args: nfs_args,
        pub iso_args: iso_args,
        pub msdosfs_args: msdosfs_args,
        pub ntfs_args: ntfs_args,
        pub tmpfs_args: tmpfs_args,
        align: [c_char; 160],
    }

    pub union __c_anonymous_ifr_ifru {
        pub ifru_addr: crate::sockaddr,
        pub ifru_dstaddr: crate::sockaddr,
        pub ifru_broadaddr: crate::sockaddr,
        pub ifru_flags: c_short,
        pub ifru_metric: c_int,
        pub ifru_vnetid: i64,
        pub ifru_media: u64,
        pub ifru_data: crate::caddr_t,
        pub ifru_index: c_uint,
    }

    // This type uses the union mount_info:
    pub struct statfs {
        pub f_flags: u32,
        pub f_bsize: u32,
        pub f_iosize: u32,
        pub f_blocks: u64,
        pub f_bfree: u64,
        pub f_bavail: i64,
        pub f_files: u64,
        pub f_ffree: u64,
        pub f_favail: i64,
        pub f_syncwrites: u64,
        pub f_syncreads: u64,
        pub f_asyncwrites: u64,
        pub f_asyncreads: u64,
        pub f_fsid: crate::fsid_t,
        pub f_namemax: u32,
        pub f_owner: crate::uid_t,
        pub f_ctime: u64,
        pub f_fstypename: [c_char; 16],
        pub f_mntonname: [c_char; 90],
        pub f_mntfromname: [c_char; 90],
        pub f_mntfromspec: [c_char; 90],
        pub mount_info: mount_info,
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for dirent {
            fn eq(&self, other: &dirent) -> bool {
                self.d_fileno == other.d_fileno
                    && self.d_off == other.d_off
                    && self.d_reclen == other.d_reclen
                    && self.d_type == other.d_type
                    && self.d_namlen == other.d_namlen
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
                self.d_fileno.hash(state);
                self.d_off.hash(state);
                self.d_reclen.hash(state);
                self.d_type.hash(state);
                self.d_namlen.hash(state);
                self.d_name.hash(state);
            }
        }

        impl PartialEq for sockaddr_storage {
            fn eq(&self, other: &sockaddr_storage) -> bool {
                self.ss_len == other.ss_len && self.ss_family == other.ss_family
            }
        }

        impl Eq for sockaddr_storage {}

        impl hash::Hash for sockaddr_storage {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ss_len.hash(state);
                self.ss_family.hash(state);
            }
        }

        impl PartialEq for siginfo_t {
            fn eq(&self, other: &siginfo_t) -> bool {
                self.si_signo == other.si_signo
                    && self.si_code == other.si_code
                    && self.si_errno == other.si_errno
                    && self.si_addr == other.si_addr
            }
        }

        impl Eq for siginfo_t {}

        impl hash::Hash for siginfo_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.si_signo.hash(state);
                self.si_code.hash(state);
                self.si_errno.hash(state);
                self.si_addr.hash(state);
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
                self.ut_time == other.ut_time
                    && self
                        .ut_line
                        .iter()
                        .zip(other.ut_line.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .ut_name
                        .iter()
                        .zip(other.ut_name.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .ut_host
                        .iter()
                        .zip(other.ut_host.iter())
                        .all(|(a, b)| a == b)
            }
        }

        impl Eq for utmp {}

        impl hash::Hash for utmp {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ut_line.hash(state);
                self.ut_name.hash(state);
                self.ut_host.hash(state);
                self.ut_time.hash(state);
            }
        }

        impl PartialEq for mount_info {
            fn eq(&self, other: &mount_info) -> bool {
                unsafe {
                    self.align
                        .iter()
                        .zip(other.align.iter())
                        .all(|(a, b)| a == b)
                }
            }
        }

        impl Eq for mount_info {}

        impl hash::Hash for mount_info {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe { self.align.hash(state) };
            }
        }

        impl PartialEq for __c_anonymous_ifr_ifru {
            fn eq(&self, other: &__c_anonymous_ifr_ifru) -> bool {
                unsafe {
                    self.ifru_addr == other.ifru_addr
                        && self.ifru_dstaddr == other.ifru_dstaddr
                        && self.ifru_broadaddr == other.ifru_broadaddr
                        && self.ifru_flags == other.ifru_flags
                        && self.ifru_metric == other.ifru_metric
                        && self.ifru_vnetid == other.ifru_vnetid
                        && self.ifru_media == other.ifru_media
                        && self.ifru_data == other.ifru_data
                        && self.ifru_index == other.ifru_index
                }
            }
        }

        impl Eq for __c_anonymous_ifr_ifru {}

        impl hash::Hash for __c_anonymous_ifr_ifru {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    self.ifru_addr.hash(state);
                    self.ifru_dstaddr.hash(state);
                    self.ifru_broadaddr.hash(state);
                    self.ifru_flags.hash(state);
                    self.ifru_metric.hash(state);
                    self.ifru_vnetid.hash(state);
                    self.ifru_media.hash(state);
                    self.ifru_data.hash(state);
                    self.ifru_index.hash(state);
                }
            }
        }

        impl PartialEq for statfs {
            fn eq(&self, other: &statfs) -> bool {
                self.f_flags == other.f_flags
                    && self.f_bsize == other.f_bsize
                    && self.f_iosize == other.f_iosize
                    && self.f_blocks == other.f_blocks
                    && self.f_bfree == other.f_bfree
                    && self.f_bavail == other.f_bavail
                    && self.f_files == other.f_files
                    && self.f_ffree == other.f_ffree
                    && self.f_favail == other.f_favail
                    && self.f_syncwrites == other.f_syncwrites
                    && self.f_syncreads == other.f_syncreads
                    && self.f_asyncwrites == other.f_asyncwrites
                    && self.f_asyncreads == other.f_asyncreads
                    && self.f_fsid == other.f_fsid
                    && self.f_namemax == other.f_namemax
                    && self.f_owner == other.f_owner
                    && self.f_ctime == other.f_ctime
                    && self
                        .f_fstypename
                        .iter()
                        .zip(other.f_fstypename.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .f_mntonname
                        .iter()
                        .zip(other.f_mntonname.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .f_mntfromname
                        .iter()
                        .zip(other.f_mntfromname.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .f_mntfromspec
                        .iter()
                        .zip(other.f_mntfromspec.iter())
                        .all(|(a, b)| a == b)
                    && self.mount_info == other.mount_info
            }
        }

        impl Eq for statfs {}

        impl hash::Hash for statfs {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.f_flags.hash(state);
                self.f_bsize.hash(state);
                self.f_iosize.hash(state);
                self.f_blocks.hash(state);
                self.f_bfree.hash(state);
                self.f_bavail.hash(state);
                self.f_files.hash(state);
                self.f_ffree.hash(state);
                self.f_favail.hash(state);
                self.f_syncwrites.hash(state);
                self.f_syncreads.hash(state);
                self.f_asyncwrites.hash(state);
                self.f_asyncreads.hash(state);
                self.f_fsid.hash(state);
                self.f_namemax.hash(state);
                self.f_owner.hash(state);
                self.f_ctime.hash(state);
                self.f_fstypename.hash(state);
                self.f_mntonname.hash(state);
                self.f_mntfromname.hash(state);
                self.f_mntfromspec.hash(state);
                self.mount_info.hash(state);
            }
        }
    }
}

pub const UT_NAMESIZE: usize = 32;
pub const UT_LINESIZE: usize = 8;
pub const UT_HOSTSIZE: usize = 256;

pub const O_CLOEXEC: c_int = 0x10000;
pub const O_DIRECTORY: c_int = 0x20000;
pub const O_RSYNC: c_int = O_SYNC;

pub const MS_SYNC: c_int = 0x0002;
pub const MS_INVALIDATE: c_int = 0x0004;

pub const POLLNORM: c_short = crate::POLLRDNORM;

pub const ENOATTR: c_int = 83;
pub const EILSEQ: c_int = 84;
pub const EOVERFLOW: c_int = 87;
pub const ECANCELED: c_int = 88;
pub const EIDRM: c_int = 89;
pub const ENOMSG: c_int = 90;
pub const ENOTSUP: c_int = 91;
pub const EBADMSG: c_int = 92;
pub const ENOTRECOVERABLE: c_int = 93;
pub const EOWNERDEAD: c_int = 94;
pub const EPROTO: c_int = 95;
pub const ELAST: c_int = 95;

pub const F_DUPFD_CLOEXEC: c_int = 10;

pub const UTIME_OMIT: c_long = -1;
pub const UTIME_NOW: c_long = -2;

pub const AT_FDCWD: c_int = -100;
pub const AT_EACCESS: c_int = 0x01;
pub const AT_SYMLINK_NOFOLLOW: c_int = 0x02;
pub const AT_SYMLINK_FOLLOW: c_int = 0x04;
pub const AT_REMOVEDIR: c_int = 0x08;

#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIM_NLIMITS: c_int = 9;

pub const SO_TIMESTAMP: c_int = 0x0800;
pub const SO_SNDTIMEO: c_int = 0x1005;
pub const SO_RCVTIMEO: c_int = 0x1006;
pub const SO_BINDANY: c_int = 0x1000;
pub const SO_NETPROC: c_int = 0x1020;
pub const SO_RTABLE: c_int = 0x1021;
pub const SO_PEERCRED: c_int = 0x1022;
pub const SO_SPLICE: c_int = 0x1023;
pub const SO_DOMAIN: c_int = 0x1024;
pub const SO_PROTOCOL: c_int = 0x1025;

// sys/netinet/in.h
// Protocols (RFC 1700)
// NOTE: These are in addition to the constants defined in src/unix/mod.rs

// IPPROTO_IP defined in src/unix/mod.rs
/// Hop-by-hop option header
pub const IPPROTO_HOPOPTS: c_int = 0;
// IPPROTO_ICMP defined in src/unix/mod.rs
/// group mgmt protocol
pub const IPPROTO_IGMP: c_int = 2;
/// gateway^2 (deprecated)
pub const IPPROTO_GGP: c_int = 3;
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
/// IP Mobility RFC 2004
pub const IPPROTO_MOBILE: c_int = 55;
// IPPROTO_ICMPV6 defined in src/unix/mod.rs
/// IP6 no next header
pub const IPPROTO_NONE: c_int = 59;
/// IP6 destination option
pub const IPPROTO_DSTOPTS: c_int = 60;
/// ISO cnlp
pub const IPPROTO_EON: c_int = 80;
/// Ethernet-in-IP
pub const IPPROTO_ETHERIP: c_int = 97;
/// encapsulation header
pub const IPPROTO_ENCAP: c_int = 98;
/// Protocol indep. multicast
pub const IPPROTO_PIM: c_int = 103;
/// IP Payload Comp. Protocol
pub const IPPROTO_IPCOMP: c_int = 108;
/// CARP
pub const IPPROTO_CARP: c_int = 112;
/// unicast MPLS packet
pub const IPPROTO_MPLS: c_int = 137;
/// PFSYNC
pub const IPPROTO_PFSYNC: c_int = 240;
pub const IPPROTO_MAX: c_int = 256;

// Only used internally, so it can be outside the range of valid IP protocols
pub const IPPROTO_DIVERT: c_int = 258;

pub const IP_RECVDSTADDR: c_int = 7;
pub const IP_SENDSRCADDR: c_int = IP_RECVDSTADDR;
pub const IP_RECVIF: c_int = 30;

// sys/netinet/in.h
pub const TCP_MD5SIG: c_int = 0x04;
pub const TCP_NOPUSH: c_int = 0x10;

pub const MSG_WAITFORONE: c_int = 0x1000;

pub const AF_ECMA: c_int = 8;
pub const AF_ROUTE: c_int = 17;
pub const AF_ENCAP: c_int = 28;
pub const AF_SIP: c_int = 29;
pub const AF_KEY: c_int = 30;
pub const pseudo_AF_HDRCMPLT: c_int = 31;
pub const AF_BLUETOOTH: c_int = 32;
pub const AF_MPLS: c_int = 33;
pub const pseudo_AF_PFLOW: c_int = 34;
pub const pseudo_AF_PIPEX: c_int = 35;
pub const NET_RT_DUMP: c_int = 1;
pub const NET_RT_FLAGS: c_int = 2;
pub const NET_RT_IFLIST: c_int = 3;
pub const NET_RT_STATS: c_int = 4;
pub const NET_RT_TABLE: c_int = 5;
pub const NET_RT_IFNAMES: c_int = 6;
#[doc(hidden)]
#[deprecated(
    since = "0.2.95",
    note = "Possibly increasing over the releases and might not be so used in the field"
)]
pub const NET_RT_MAXID: c_int = 7;

pub const IPV6_JOIN_GROUP: c_int = 12;
pub const IPV6_LEAVE_GROUP: c_int = 13;

pub const PF_ROUTE: c_int = AF_ROUTE;
pub const PF_ECMA: c_int = AF_ECMA;
pub const PF_ENCAP: c_int = AF_ENCAP;
pub const PF_SIP: c_int = AF_SIP;
pub const PF_KEY: c_int = AF_KEY;
pub const PF_BPF: c_int = pseudo_AF_HDRCMPLT;
pub const PF_BLUETOOTH: c_int = AF_BLUETOOTH;
pub const PF_MPLS: c_int = AF_MPLS;
pub const PF_PFLOW: c_int = pseudo_AF_PFLOW;
pub const PF_PIPEX: c_int = pseudo_AF_PIPEX;

pub const SCM_TIMESTAMP: c_int = 0x04;

pub const O_DSYNC: c_int = 128;

pub const MAP_RENAME: c_int = 0x0000;
pub const MAP_NORESERVE: c_int = 0x0000;
pub const MAP_HASSEMAPHORE: c_int = 0x0000;
pub const MAP_TRYFIXED: c_int = 0;

pub const EIPSEC: c_int = 82;
pub const ENOMEDIUM: c_int = 85;
pub const EMEDIUMTYPE: c_int = 86;

pub const EAI_BADFLAGS: c_int = -1;
pub const EAI_NONAME: c_int = -2;
pub const EAI_AGAIN: c_int = -3;
pub const EAI_FAIL: c_int = -4;
pub const EAI_NODATA: c_int = -5;
pub const EAI_FAMILY: c_int = -6;
pub const EAI_SOCKTYPE: c_int = -7;
pub const EAI_SERVICE: c_int = -8;
pub const EAI_MEMORY: c_int = -10;
pub const EAI_SYSTEM: c_int = -11;
pub const EAI_OVERFLOW: c_int = -14;

pub const RUSAGE_THREAD: c_int = 1;

pub const MAP_COPY: c_int = 0x0002;
pub const MAP_NOEXTEND: c_int = 0x0000;

pub const _PC_LINK_MAX: c_int = 1;
pub const _PC_MAX_CANON: c_int = 2;
pub const _PC_MAX_INPUT: c_int = 3;
pub const _PC_NAME_MAX: c_int = 4;
pub const _PC_PATH_MAX: c_int = 5;
pub const _PC_PIPE_BUF: c_int = 6;
pub const _PC_CHOWN_RESTRICTED: c_int = 7;
pub const _PC_NO_TRUNC: c_int = 8;
pub const _PC_VDISABLE: c_int = 9;
pub const _PC_2_SYMLINKS: c_int = 10;
pub const _PC_ALLOC_SIZE_MIN: c_int = 11;
pub const _PC_ASYNC_IO: c_int = 12;
pub const _PC_FILESIZEBITS: c_int = 13;
pub const _PC_PRIO_IO: c_int = 14;
pub const _PC_REC_INCR_XFER_SIZE: c_int = 15;
pub const _PC_REC_MAX_XFER_SIZE: c_int = 16;
pub const _PC_REC_MIN_XFER_SIZE: c_int = 17;
pub const _PC_REC_XFER_ALIGN: c_int = 18;
pub const _PC_SYMLINK_MAX: c_int = 19;
pub const _PC_SYNC_IO: c_int = 20;
pub const _PC_TIMESTAMP_RESOLUTION: c_int = 21;

pub const _SC_CLK_TCK: c_int = 3;
pub const _SC_SEM_NSEMS_MAX: c_int = 31;
pub const _SC_SEM_VALUE_MAX: c_int = 32;
pub const _SC_HOST_NAME_MAX: c_int = 33;
pub const _SC_MONOTONIC_CLOCK: c_int = 34;
pub const _SC_2_PBS: c_int = 35;
pub const _SC_2_PBS_ACCOUNTING: c_int = 36;
pub const _SC_2_PBS_CHECKPOINT: c_int = 37;
pub const _SC_2_PBS_LOCATE: c_int = 38;
pub const _SC_2_PBS_MESSAGE: c_int = 39;
pub const _SC_2_PBS_TRACK: c_int = 40;
pub const _SC_ADVISORY_INFO: c_int = 41;
pub const _SC_AIO_LISTIO_MAX: c_int = 42;
pub const _SC_AIO_MAX: c_int = 43;
pub const _SC_AIO_PRIO_DELTA_MAX: c_int = 44;
pub const _SC_ASYNCHRONOUS_IO: c_int = 45;
pub const _SC_ATEXIT_MAX: c_int = 46;
pub const _SC_BARRIERS: c_int = 47;
pub const _SC_CLOCK_SELECTION: c_int = 48;
pub const _SC_CPUTIME: c_int = 49;
pub const _SC_DELAYTIMER_MAX: c_int = 50;
pub const _SC_IOV_MAX: c_int = 51;
pub const _SC_IPV6: c_int = 52;
pub const _SC_MAPPED_FILES: c_int = 53;
pub const _SC_MEMLOCK: c_int = 54;
pub const _SC_MEMLOCK_RANGE: c_int = 55;
pub const _SC_MEMORY_PROTECTION: c_int = 56;
pub const _SC_MESSAGE_PASSING: c_int = 57;
pub const _SC_MQ_OPEN_MAX: c_int = 58;
pub const _SC_MQ_PRIO_MAX: c_int = 59;
pub const _SC_PRIORITIZED_IO: c_int = 60;
pub const _SC_PRIORITY_SCHEDULING: c_int = 61;
pub const _SC_RAW_SOCKETS: c_int = 62;
pub const _SC_READER_WRITER_LOCKS: c_int = 63;
pub const _SC_REALTIME_SIGNALS: c_int = 64;
pub const _SC_REGEXP: c_int = 65;
pub const _SC_RTSIG_MAX: c_int = 66;
pub const _SC_SEMAPHORES: c_int = 67;
pub const _SC_SHARED_MEMORY_OBJECTS: c_int = 68;
pub const _SC_SHELL: c_int = 69;
pub const _SC_SIGQUEUE_MAX: c_int = 70;
pub const _SC_SPAWN: c_int = 71;
pub const _SC_SPIN_LOCKS: c_int = 72;
pub const _SC_SPORADIC_SERVER: c_int = 73;
pub const _SC_SS_REPL_MAX: c_int = 74;
pub const _SC_SYNCHRONIZED_IO: c_int = 75;
pub const _SC_SYMLOOP_MAX: c_int = 76;
pub const _SC_THREAD_ATTR_STACKADDR: c_int = 77;
pub const _SC_THREAD_ATTR_STACKSIZE: c_int = 78;
pub const _SC_THREAD_CPUTIME: c_int = 79;
pub const _SC_THREAD_DESTRUCTOR_ITERATIONS: c_int = 80;
pub const _SC_THREAD_KEYS_MAX: c_int = 81;
pub const _SC_THREAD_PRIO_INHERIT: c_int = 82;
pub const _SC_THREAD_PRIO_PROTECT: c_int = 83;
pub const _SC_THREAD_PRIORITY_SCHEDULING: c_int = 84;
pub const _SC_THREAD_PROCESS_SHARED: c_int = 85;
pub const _SC_THREAD_ROBUST_PRIO_INHERIT: c_int = 86;
pub const _SC_THREAD_ROBUST_PRIO_PROTECT: c_int = 87;
pub const _SC_THREAD_SPORADIC_SERVER: c_int = 88;
pub const _SC_THREAD_STACK_MIN: c_int = 89;
pub const _SC_THREAD_THREADS_MAX: c_int = 90;
pub const _SC_THREADS: c_int = 91;
pub const _SC_TIMEOUTS: c_int = 92;
pub const _SC_TIMER_MAX: c_int = 93;
pub const _SC_TIMERS: c_int = 94;
pub const _SC_TRACE: c_int = 95;
pub const _SC_TRACE_EVENT_FILTER: c_int = 96;
pub const _SC_TRACE_EVENT_NAME_MAX: c_int = 97;
pub const _SC_TRACE_INHERIT: c_int = 98;
pub const _SC_TRACE_LOG: c_int = 99;
pub const _SC_GETGR_R_SIZE_MAX: c_int = 100;
pub const _SC_GETPW_R_SIZE_MAX: c_int = 101;
pub const _SC_LOGIN_NAME_MAX: c_int = 102;
pub const _SC_THREAD_SAFE_FUNCTIONS: c_int = 103;
pub const _SC_TRACE_NAME_MAX: c_int = 104;
pub const _SC_TRACE_SYS_MAX: c_int = 105;
pub const _SC_TRACE_USER_EVENT_MAX: c_int = 106;
pub const _SC_TTY_NAME_MAX: c_int = 107;
pub const _SC_TYPED_MEMORY_OBJECTS: c_int = 108;
pub const _SC_V6_ILP32_OFF32: c_int = 109;
pub const _SC_V6_ILP32_OFFBIG: c_int = 110;
pub const _SC_V6_LP64_OFF64: c_int = 111;
pub const _SC_V6_LPBIG_OFFBIG: c_int = 112;
pub const _SC_V7_ILP32_OFF32: c_int = 113;
pub const _SC_V7_ILP32_OFFBIG: c_int = 114;
pub const _SC_V7_LP64_OFF64: c_int = 115;
pub const _SC_V7_LPBIG_OFFBIG: c_int = 116;
pub const _SC_XOPEN_CRYPT: c_int = 117;
pub const _SC_XOPEN_ENH_I18N: c_int = 118;
pub const _SC_XOPEN_LEGACY: c_int = 119;
pub const _SC_XOPEN_REALTIME: c_int = 120;
pub const _SC_XOPEN_REALTIME_THREADS: c_int = 121;
pub const _SC_XOPEN_STREAMS: c_int = 122;
pub const _SC_XOPEN_UNIX: c_int = 123;
pub const _SC_XOPEN_UUCP: c_int = 124;
pub const _SC_XOPEN_VERSION: c_int = 125;
pub const _SC_PHYS_PAGES: c_int = 500;
pub const _SC_AVPHYS_PAGES: c_int = 501;
pub const _SC_NPROCESSORS_CONF: c_int = 502;
pub const _SC_NPROCESSORS_ONLN: c_int = 503;

pub const FD_SETSIZE: usize = 1024;

pub const SCHED_FIFO: c_int = 1;
pub const SCHED_OTHER: c_int = 2;
pub const SCHED_RR: c_int = 3;

pub const ST_NOSUID: c_ulong = 2;

pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = 0 as *mut _;
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = 0 as *mut _;
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = 0 as *mut _;

pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 1;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 2;
pub const PTHREAD_MUTEX_NORMAL: c_int = 3;
pub const PTHREAD_MUTEX_STRICT_NP: c_int = 4;
pub const PTHREAD_MUTEX_DEFAULT: c_int = PTHREAD_MUTEX_STRICT_NP;

pub const EVFILT_READ: i16 = -1;
pub const EVFILT_WRITE: i16 = -2;
pub const EVFILT_AIO: i16 = -3;
pub const EVFILT_VNODE: i16 = -4;
pub const EVFILT_PROC: i16 = -5;
pub const EVFILT_SIGNAL: i16 = -6;
pub const EVFILT_TIMER: i16 = -7;
pub const EVFILT_DEVICE: i16 = -8;
pub const EVFILT_EXCEPT: i16 = -9;

pub const EV_ADD: u16 = 0x1;
pub const EV_DELETE: u16 = 0x2;
pub const EV_ENABLE: u16 = 0x4;
pub const EV_DISABLE: u16 = 0x8;
pub const EV_ONESHOT: u16 = 0x10;
pub const EV_CLEAR: u16 = 0x20;
pub const EV_RECEIPT: u16 = 0x40;
pub const EV_DISPATCH: u16 = 0x80;
pub const EV_FLAG1: u16 = 0x2000;
pub const EV_ERROR: u16 = 0x4000;
pub const EV_EOF: u16 = 0x8000;

#[deprecated(since = "0.2.113", note = "Not stable across OS versions")]
pub const EV_SYSFLAGS: u16 = 0xf800;

pub const NOTE_LOWAT: u32 = 0x00000001;
pub const NOTE_EOF: u32 = 0x00000002;
pub const NOTE_OOB: u32 = 0x00000004;
pub const NOTE_DELETE: u32 = 0x00000001;
pub const NOTE_WRITE: u32 = 0x00000002;
pub const NOTE_EXTEND: u32 = 0x00000004;
pub const NOTE_ATTRIB: u32 = 0x00000008;
pub const NOTE_LINK: u32 = 0x00000010;
pub const NOTE_RENAME: u32 = 0x00000020;
pub const NOTE_REVOKE: u32 = 0x00000040;
pub const NOTE_TRUNCATE: u32 = 0x00000080;
pub const NOTE_EXIT: u32 = 0x80000000;
pub const NOTE_FORK: u32 = 0x40000000;
pub const NOTE_EXEC: u32 = 0x20000000;
pub const NOTE_PDATAMASK: u32 = 0x000fffff;
pub const NOTE_PCTRLMASK: u32 = 0xf0000000;
pub const NOTE_TRACK: u32 = 0x00000001;
pub const NOTE_TRACKERR: u32 = 0x00000002;
pub const NOTE_CHILD: u32 = 0x00000004;
pub const NOTE_CHANGE: u32 = 0x00000001;

pub const TMP_MAX: c_uint = 0x7fffffff;

pub const AI_PASSIVE: c_int = 1;
pub const AI_CANONNAME: c_int = 2;
pub const AI_NUMERICHOST: c_int = 4;
pub const AI_EXT: c_int = 8;
pub const AI_NUMERICSERV: c_int = 16;
pub const AI_FQDN: c_int = 32;
pub const AI_ADDRCONFIG: c_int = 64;

pub const NI_NUMERICHOST: c_int = 1;
pub const NI_NUMERICSERV: c_int = 2;
pub const NI_NOFQDN: c_int = 4;
pub const NI_NAMEREQD: c_int = 8;
pub const NI_DGRAM: c_int = 16;

pub const NI_MAXHOST: size_t = 256;

pub const RTLD_LOCAL: c_int = 0;

pub const CTL_MAXNAME: c_int = 12;

pub const CTLTYPE_NODE: c_int = 1;
pub const CTLTYPE_INT: c_int = 2;
pub const CTLTYPE_STRING: c_int = 3;
pub const CTLTYPE_QUAD: c_int = 4;
pub const CTLTYPE_STRUCT: c_int = 5;

pub const CTL_UNSPEC: c_int = 0;
pub const CTL_KERN: c_int = 1;
pub const CTL_VM: c_int = 2;
pub const CTL_FS: c_int = 3;
pub const CTL_NET: c_int = 4;
pub const CTL_DEBUG: c_int = 5;
pub const CTL_HW: c_int = 6;
pub const CTL_MACHDEP: c_int = 7;
pub const CTL_DDB: c_int = 9;
pub const CTL_VFS: c_int = 10;
pub const CTL_MAXID: c_int = 11;

pub const HW_NCPUONLINE: c_int = 25;

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
pub const KERN_PROF: c_int = 16;
pub const KERN_POSIX1: c_int = 17;
pub const KERN_NGROUPS: c_int = 18;
pub const KERN_JOB_CONTROL: c_int = 19;
pub const KERN_SAVED_IDS: c_int = 20;
pub const KERN_BOOTTIME: c_int = 21;
pub const KERN_DOMAINNAME: c_int = 22;
pub const KERN_MAXPARTITIONS: c_int = 23;
pub const KERN_RAWPARTITION: c_int = 24;
pub const KERN_MAXTHREAD: c_int = 25;
pub const KERN_NTHREADS: c_int = 26;
pub const KERN_OSVERSION: c_int = 27;
pub const KERN_SOMAXCONN: c_int = 28;
pub const KERN_SOMINCONN: c_int = 29;
#[deprecated(since = "0.2.71", note = "Removed in OpenBSD 6.0")]
pub const KERN_USERMOUNT: c_int = 30;
pub const KERN_NOSUIDCOREDUMP: c_int = 32;
pub const KERN_FSYNC: c_int = 33;
pub const KERN_SYSVMSG: c_int = 34;
pub const KERN_SYSVSEM: c_int = 35;
pub const KERN_SYSVSHM: c_int = 36;
#[deprecated(since = "0.2.71", note = "Removed in OpenBSD 6.0")]
pub const KERN_ARND: c_int = 37;
pub const KERN_MSGBUFSIZE: c_int = 38;
pub const KERN_MALLOCSTATS: c_int = 39;
pub const KERN_CPTIME: c_int = 40;
pub const KERN_NCHSTATS: c_int = 41;
pub const KERN_FORKSTAT: c_int = 42;
pub const KERN_NSELCOLL: c_int = 43;
pub const KERN_TTY: c_int = 44;
pub const KERN_CCPU: c_int = 45;
pub const KERN_FSCALE: c_int = 46;
pub const KERN_NPROCS: c_int = 47;
pub const KERN_MSGBUF: c_int = 48;
pub const KERN_POOL: c_int = 49;
pub const KERN_STACKGAPRANDOM: c_int = 50;
pub const KERN_SYSVIPC_INFO: c_int = 51;
pub const KERN_SPLASSERT: c_int = 54;
pub const KERN_PROC_ARGS: c_int = 55;
pub const KERN_NFILES: c_int = 56;
pub const KERN_TTYCOUNT: c_int = 57;
pub const KERN_NUMVNODES: c_int = 58;
pub const KERN_MBSTAT: c_int = 59;
pub const KERN_SEMINFO: c_int = 61;
pub const KERN_SHMINFO: c_int = 62;
pub const KERN_INTRCNT: c_int = 63;
pub const KERN_WATCHDOG: c_int = 64;
pub const KERN_PROC: c_int = 66;
pub const KERN_MAXCLUSTERS: c_int = 67;
pub const KERN_EVCOUNT: c_int = 68;
pub const KERN_TIMECOUNTER: c_int = 69;
pub const KERN_MAXLOCKSPERUID: c_int = 70;
pub const KERN_CPTIME2: c_int = 71;
pub const KERN_CACHEPCT: c_int = 72;
pub const KERN_FILE: c_int = 73;
pub const KERN_CONSDEV: c_int = 75;
pub const KERN_NETLIVELOCKS: c_int = 76;
pub const KERN_POOL_DEBUG: c_int = 77;
pub const KERN_PROC_CWD: c_int = 78;
pub const KERN_PROC_NOBROADCASTKILL: c_int = 79;
pub const KERN_PROC_VMMAP: c_int = 80;
pub const KERN_GLOBAL_PTRACE: c_int = 81;
pub const KERN_CONSBUFSIZE: c_int = 82;
pub const KERN_CONSBUF: c_int = 83;
pub const KERN_AUDIO: c_int = 84;
pub const KERN_CPUSTATS: c_int = 85;
pub const KERN_PFSTATUS: c_int = 86;
pub const KERN_TIMEOUT_STATS: c_int = 87;
#[deprecated(
    since = "0.2.95",
    note = "Possibly increasing over the releases and might not be so used in the field"
)]
pub const KERN_MAXID: c_int = 88;

pub const KERN_PROC_ALL: c_int = 0;
pub const KERN_PROC_PID: c_int = 1;
pub const KERN_PROC_PGRP: c_int = 2;
pub const KERN_PROC_SESSION: c_int = 3;
pub const KERN_PROC_TTY: c_int = 4;
pub const KERN_PROC_UID: c_int = 5;
pub const KERN_PROC_RUID: c_int = 6;
pub const KERN_PROC_KTHREAD: c_int = 7;
pub const KERN_PROC_SHOW_THREADS: c_int = 0x40000000;

pub const KERN_SYSVIPC_MSG_INFO: c_int = 1;
pub const KERN_SYSVIPC_SEM_INFO: c_int = 2;
pub const KERN_SYSVIPC_SHM_INFO: c_int = 3;

pub const KERN_PROC_ARGV: c_int = 1;
pub const KERN_PROC_NARGV: c_int = 2;
pub const KERN_PROC_ENV: c_int = 3;
pub const KERN_PROC_NENV: c_int = 4;

pub const KI_NGROUPS: c_int = 16;
pub const KI_MAXCOMLEN: c_int = 24;
pub const KI_WMESGLEN: c_int = 8;
pub const KI_MAXLOGNAME: c_int = 32;
pub const KI_EMULNAMELEN: c_int = 8;

pub const KVE_ET_OBJ: c_int = 0x00000001;
pub const KVE_ET_SUBMAP: c_int = 0x00000002;
pub const KVE_ET_COPYONWRITE: c_int = 0x00000004;
pub const KVE_ET_NEEDSCOPY: c_int = 0x00000008;
pub const KVE_ET_HOLE: c_int = 0x00000010;
pub const KVE_ET_NOFAULT: c_int = 0x00000020;
pub const KVE_ET_STACK: c_int = 0x00000040;
pub const KVE_ET_WC: c_int = 0x000000080;
pub const KVE_ET_CONCEAL: c_int = 0x000000100;
pub const KVE_ET_SYSCALL: c_int = 0x000000200;
pub const KVE_ET_FREEMAPPED: c_int = 0x000000800;

pub const KVE_PROT_NONE: c_int = 0x00000000;
pub const KVE_PROT_READ: c_int = 0x00000001;
pub const KVE_PROT_WRITE: c_int = 0x00000002;
pub const KVE_PROT_EXEC: c_int = 0x00000004;

pub const KVE_ADV_NORMAL: c_int = 0x00000000;
pub const KVE_ADV_RANDOM: c_int = 0x00000001;
pub const KVE_ADV_SEQUENTIAL: c_int = 0x00000002;

pub const KVE_INH_SHARE: c_int = 0x00000000;
pub const KVE_INH_COPY: c_int = 0x00000010;
pub const KVE_INH_NONE: c_int = 0x00000020;
pub const KVE_INH_ZERO: c_int = 0x00000030;

pub const KVE_F_STATIC: c_int = 0x1;
pub const KVE_F_KMEM: c_int = 0x2;

pub const CHWFLOW: crate::tcflag_t = crate::MDMBUF | crate::CRTSCTS;
pub const OLCUC: crate::tcflag_t = 0x20;
pub const ONOCR: crate::tcflag_t = 0x40;
pub const ONLRET: crate::tcflag_t = 0x80;

//https://github.com/openbsd/src/blob/HEAD/sys/sys/mount.h
pub const ISOFSMNT_NORRIP: c_int = 0x1; // disable Rock Ridge Ext
pub const ISOFSMNT_GENS: c_int = 0x2; // enable generation numbers
pub const ISOFSMNT_EXTATT: c_int = 0x4; // enable extended attr
pub const ISOFSMNT_NOJOLIET: c_int = 0x8; // disable Joliet Ext
pub const ISOFSMNT_SESS: c_int = 0x10; // use iso_args.sess

pub const NFS_ARGSVERSION: c_int = 4; // change when nfs_args changes

pub const NFSMNT_RESVPORT: c_int = 0; // always use reserved ports
pub const NFSMNT_SOFT: c_int = 0x1; // soft mount (hard is default)
pub const NFSMNT_WSIZE: c_int = 0x2; // set write size
pub const NFSMNT_RSIZE: c_int = 0x4; // set read size
pub const NFSMNT_TIMEO: c_int = 0x8; // set initial timeout
pub const NFSMNT_RETRANS: c_int = 0x10; // set number of request retries
pub const NFSMNT_MAXGRPS: c_int = 0x20; // set maximum grouplist size
pub const NFSMNT_INT: c_int = 0x40; // allow interrupts on hard mount
pub const NFSMNT_NOCONN: c_int = 0x80; // Don't Connect the socket
pub const NFSMNT_NQNFS: c_int = 0x100; // Use Nqnfs protocol
pub const NFSMNT_NFSV3: c_int = 0x200; // Use NFS Version 3 protocol
pub const NFSMNT_KERB: c_int = 0x400; // Use Kerberos authentication
pub const NFSMNT_DUMBTIMR: c_int = 0x800; // Don't estimate rtt dynamically
pub const NFSMNT_LEASETERM: c_int = 0x1000; // set lease term (nqnfs)
pub const NFSMNT_READAHEAD: c_int = 0x2000; // set read ahead
pub const NFSMNT_DEADTHRESH: c_int = 0x4000; // set dead server retry thresh
pub const NFSMNT_NOAC: c_int = 0x8000; // disable attribute cache
pub const NFSMNT_RDIRPLUS: c_int = 0x10000; // Use Readdirplus for V3
pub const NFSMNT_READDIRSIZE: c_int = 0x20000; // Set readdir size

/* Flags valid only in mount syscall arguments */
pub const NFSMNT_ACREGMIN: c_int = 0x40000; // acregmin field valid
pub const NFSMNT_ACREGMAX: c_int = 0x80000; // acregmax field valid
pub const NFSMNT_ACDIRMIN: c_int = 0x100000; // acdirmin field valid
pub const NFSMNT_ACDIRMAX: c_int = 0x200000; // acdirmax field valid

/* Flags valid only in kernel */
pub const NFSMNT_INTERNAL: c_int = 0xfffc0000; // Bits set internally
pub const NFSMNT_HASWRITEVERF: c_int = 0x40000; // Has write verifier for V3
pub const NFSMNT_GOTPATHCONF: c_int = 0x80000; // Got the V3 pathconf info
pub const NFSMNT_GOTFSINFO: c_int = 0x100000; // Got the V3 fsinfo
pub const NFSMNT_MNTD: c_int = 0x200000; // Mnt server for mnt point
pub const NFSMNT_DISMINPROG: c_int = 0x400000; // Dismount in progress
pub const NFSMNT_DISMNT: c_int = 0x800000; // Dismounted
pub const NFSMNT_SNDLOCK: c_int = 0x1000000; // Send socket lock
pub const NFSMNT_WANTSND: c_int = 0x2000000; // Want above
pub const NFSMNT_RCVLOCK: c_int = 0x4000000; // Rcv socket lock
pub const NFSMNT_WANTRCV: c_int = 0x8000000; // Want above
pub const NFSMNT_WAITAUTH: c_int = 0x10000000; // Wait for authentication
pub const NFSMNT_HASAUTH: c_int = 0x20000000; // Has authenticator
pub const NFSMNT_WANTAUTH: c_int = 0x40000000; // Wants an authenticator
pub const NFSMNT_AUTHERR: c_int = 0x80000000; // Authentication error

pub const MSDOSFSMNT_SHORTNAME: c_int = 0x1; // Force old DOS short names only
pub const MSDOSFSMNT_LONGNAME: c_int = 0x2; // Force Win'95 long names
pub const MSDOSFSMNT_NOWIN95: c_int = 0x4; // Completely ignore Win95 entries

pub const NTFS_MFLAG_CASEINS: c_int = 0x1;
pub const NTFS_MFLAG_ALLNAMES: c_int = 0x2;

pub const TMPFS_ARGS_VERSION: c_int = 1;

const SI_MAXSZ: size_t = 128;
const SI_PAD: size_t = (SI_MAXSZ / size_of::<c_int>()) - 3;

pub const MAP_STACK: c_int = 0x4000;
pub const MAP_CONCEAL: c_int = 0x8000;

// https://github.com/openbsd/src/blob/HEAD/sys/net/if.h#L187
pub const IFF_UP: c_int = 0x1; // interface is up
pub const IFF_BROADCAST: c_int = 0x2; // broadcast address valid
pub const IFF_DEBUG: c_int = 0x4; // turn on debugging
pub const IFF_LOOPBACK: c_int = 0x8; // is a loopback net
pub const IFF_POINTOPOINT: c_int = 0x10; // interface is point-to-point link
pub const IFF_STATICARP: c_int = 0x20; // only static ARP
pub const IFF_RUNNING: c_int = 0x40; // resources allocated
pub const IFF_NOARP: c_int = 0x80; // no address resolution protocol
pub const IFF_PROMISC: c_int = 0x100; // receive all packets
pub const IFF_ALLMULTI: c_int = 0x200; // receive all multicast packets
pub const IFF_OACTIVE: c_int = 0x400; // transmission in progress
pub const IFF_SIMPLEX: c_int = 0x800; // can't hear own transmissions
pub const IFF_LINK0: c_int = 0x1000; // per link layer defined bit
pub const IFF_LINK1: c_int = 0x2000; // per link layer defined bit
pub const IFF_LINK2: c_int = 0x4000; // per link layer defined bit
pub const IFF_MULTICAST: c_int = 0x8000; // supports multicast

pub const PTHREAD_STACK_MIN: size_t = 1_usize << _MAX_PAGE_SHIFT;
pub const MINSIGSTKSZ: size_t = 3_usize << _MAX_PAGE_SHIFT;
pub const SIGSTKSZ: size_t = MINSIGSTKSZ + (1_usize << _MAX_PAGE_SHIFT) * 4;

pub const PT_SET_EVENT_MASK: c_int = 12;
pub const PT_GET_EVENT_MASK: c_int = 13;
pub const PT_GET_PROCESS_STATE: c_int = 14;
pub const PT_GET_THREAD_FIRST: c_int = 15;
pub const PT_GET_THREAD_NEXT: c_int = 16;
pub const PT_FIRSTMACH: c_int = 32;

pub const SOCK_CLOEXEC: c_int = 0x8000;
pub const SOCK_NONBLOCK: c_int = 0x4000;
pub const SOCK_DNS: c_int = 0x1000;

pub const BIOCGRSIG: c_ulong = 0x40044273;
pub const BIOCSRSIG: c_ulong = 0x80044272;
pub const BIOCSDLT: c_ulong = 0x8004427a;

pub const PTRACE_FORK: c_int = 0x0002;

pub const WCONTINUED: c_int = 0x08;
pub const WEXITED: c_int = 0x04;
pub const WSTOPPED: c_int = 0x02; // same as WUNTRACED
pub const WNOWAIT: c_int = 0x10;
pub const WTRAPPED: c_int = 0x20;

pub const P_ALL: crate::idtype_t = 0;
pub const P_PGID: crate::idtype_t = 1;
pub const P_PID: crate::idtype_t = 2;

// search.h
pub const FIND: crate::ACTION = 0;
pub const ENTER: crate::ACTION = 1;

// futex.h
pub const FUTEX_WAIT: c_int = 1;
pub const FUTEX_WAKE: c_int = 2;
pub const FUTEX_REQUEUE: c_int = 3;
pub const FUTEX_PRIVATE_FLAG: c_int = 128;

// sysctl.h, kinfo_proc p_eflag constants
pub const EPROC_CTTY: i32 = 0x01; // controlling tty vnode active
pub const EPROC_SLEADER: i32 = 0x02; // session leader
pub const EPROC_UNVEIL: i32 = 0x04; // has unveil settings
pub const EPROC_LKUNVEIL: i32 = 0x08; // unveil is locked

// Flags for chflags(2)
pub const UF_SETTABLE: c_uint = 0x0000ffff;
pub const UF_NODUMP: c_uint = 0x00000001;
pub const UF_IMMUTABLE: c_uint = 0x00000002;
pub const UF_APPEND: c_uint = 0x00000004;
pub const UF_OPAQUE: c_uint = 0x00000008;
pub const SF_SETTABLE: c_uint = 0xffff0000;
pub const SF_ARCHIVED: c_uint = 0x00010000;
pub const SF_IMMUTABLE: c_uint = 0x00020000;
pub const SF_APPEND: c_uint = 0x00040000;

// sys/exec_elf.h - Legal values for p_type (segment type).
pub const PT_NULL: u32 = 0;
pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;
pub const PT_NOTE: u32 = 4;
pub const PT_SHLIB: u32 = 5;
pub const PT_PHDR: u32 = 6;
pub const PT_TLS: u32 = 7;
pub const PT_LOOS: u32 = 0x60000000;
pub const PT_HIOS: u32 = 0x6fffffff;
pub const PT_LOPROC: u32 = 0x70000000;
pub const PT_HIPROC: u32 = 0x7fffffff;

pub const PT_GNU_EH_FRAME: u32 = 0x6474e550;
pub const PT_GNU_RELRO: u32 = 0x6474e552;

// sys/exec_elf.h - Legal values for p_flags (segment flags).
pub const PF_X: u32 = 0x1;
pub const PF_W: u32 = 0x2;
pub const PF_R: u32 = 0x4;
pub const PF_MASKOS: u32 = 0x0ff00000;
pub const PF_MASKPROC: u32 = 0xf0000000;

// sys/ioccom.h
pub const fn IOCPARM_LEN(x: u32) -> u32 {
    (x >> 16) & crate::IOCPARM_MASK
}

pub const fn IOCBASECMD(x: u32) -> u32 {
    x & (!(crate::IOCPARM_MASK << 16))
}

pub const fn IOCGROUP(x: u32) -> u32 {
    (x >> 8) & 0xff
}

pub const fn _IOC(inout: c_ulong, group: c_ulong, num: c_ulong, len: c_ulong) -> c_ulong {
    (inout) | (((len) & crate::IOCPARM_MASK as c_ulong) << 16) | ((group) << 8) | (num)
}

// sys/mount.h
pub const MNT_NOPERM: c_int = 0x00000020;
pub const MNT_WXALLOWED: c_int = 0x00000800;
pub const MNT_EXRDONLY: c_int = 0x00000080;
pub const MNT_DEFEXPORTED: c_int = 0x00000200;
pub const MNT_EXPORTANON: c_int = 0x00000400;
pub const MNT_ROOTFS: c_int = 0x00004000;
pub const MNT_NOATIME: c_int = 0x00008000;
pub const MNT_DELEXPORT: c_int = 0x00020000;
pub const MNT_STALLED: c_int = 0x00100000;
pub const MNT_SWAPPABLE: c_int = 0x00200000;
pub const MNT_WANTRDWR: c_int = 0x02000000;
pub const MNT_SOFTDEP: c_int = 0x04000000;
pub const MNT_DOOMED: c_int = 0x08000000;

// For use with vfs_fsync and getfsstat
pub const MNT_WAIT: c_int = 1;
pub const MNT_NOWAIT: c_int = 2;
pub const MNT_LAZY: c_int = 3;

// sys/_time.h
pub const CLOCK_PROCESS_CPUTIME_ID: crate::clockid_t = 2;
pub const CLOCK_THREAD_CPUTIME_ID: crate::clockid_t = 4;
pub const CLOCK_UPTIME: crate::clockid_t = 5;
pub const CLOCK_BOOTTIME: crate::clockid_t = 6;

pub const LC_COLLATE_MASK: c_int = 1 << crate::LC_COLLATE;
pub const LC_CTYPE_MASK: c_int = 1 << crate::LC_CTYPE;
pub const LC_MONETARY_MASK: c_int = 1 << crate::LC_MONETARY;
pub const LC_NUMERIC_MASK: c_int = 1 << crate::LC_NUMERIC;
pub const LC_TIME_MASK: c_int = 1 << crate::LC_TIME;
pub const LC_MESSAGES_MASK: c_int = 1 << crate::LC_MESSAGES;

const _LC_LAST: c_int = 7;
pub const LC_ALL_MASK: c_int = (1 << _LC_LAST) - 2;

pub const LC_GLOBAL_LOCALE: crate::locale_t = -1isize as crate::locale_t;

// sys/reboot.h
pub const RB_ASKNAME: c_int = 0x00001;
pub const RB_SINGLE: c_int = 0x00002;
pub const RB_NOSYNC: c_int = 0x00004;
pub const RB_HALT: c_int = 0x00008;
pub const RB_INITNAME: c_int = 0x00010;
pub const RB_KDB: c_int = 0x00040;
pub const RB_RDONLY: c_int = 0x00080;
pub const RB_DUMP: c_int = 0x00100;
pub const RB_MINIROOT: c_int = 0x00200;
pub const RB_CONFIG: c_int = 0x00400;
pub const RB_TIMEBAD: c_int = 0x00800;
pub const RB_POWERDOWN: c_int = 0x01000;
pub const RB_SERCONS: c_int = 0x02000;
pub const RB_USERREQ: c_int = 0x04000;
pub const RB_RESET: c_int = 0x08000;
pub const RB_GOODRANDOM: c_int = 0x10000;
pub const RB_UNHIBERNATE: c_int = 0x20000;

// net/route.h
pub const RTF_CLONING: c_int = 0x100;
pub const RTF_MULTICAST: c_int = 0x200;
pub const RTF_LLINFO: c_int = 0x400;
pub const RTF_PROTO3: c_int = 0x2000;
pub const RTF_ANNOUNCE: c_int = crate::RTF_PROTO2;

pub const RTF_CLONED: c_int = 0x10000;
pub const RTF_CACHED: c_int = 0x20000;
pub const RTF_MPATH: c_int = 0x40000;
pub const RTF_MPLS: c_int = 0x100000;
pub const RTF_LOCAL: c_int = 0x200000;
pub const RTF_BROADCAST: c_int = 0x400000;
pub const RTF_CONNECTED: c_int = 0x800000;
pub const RTF_BFD: c_int = 0x1000000;
pub const RTF_FMASK: c_int = crate::RTF_LLINFO
    | crate::RTF_PROTO1
    | crate::RTF_PROTO2
    | crate::RTF_PROTO3
    | crate::RTF_BLACKHOLE
    | crate::RTF_REJECT
    | crate::RTF_STATIC
    | crate::RTF_MPLS
    | crate::RTF_BFD;

pub const RTM_VERSION: c_int = 5;
pub const RTM_RESOLVE: c_int = 0xb;
pub const RTM_NEWADDR: c_int = 0xc;
pub const RTM_DELADDR: c_int = 0xd;
pub const RTM_IFINFO: c_int = 0xe;
pub const RTM_IFANNOUNCE: c_int = 0xf;
pub const RTM_DESYNC: c_int = 0x10;
pub const RTM_INVALIDATE: c_int = 0x11;
pub const RTM_BFD: c_int = 0x12;
pub const RTM_PROPOSAL: c_int = 0x13;
pub const RTM_CHGADDRATTR: c_int = 0x14;
pub const RTM_80211INFO: c_int = 0x15;
pub const RTM_SOURCE: c_int = 0x16;

pub const RTA_SRC: c_int = 0x100;
pub const RTA_SRCMASK: c_int = 0x200;
pub const RTA_LABEL: c_int = 0x400;
pub const RTA_BFD: c_int = 0x800;
pub const RTA_DNS: c_int = 0x1000;
pub const RTA_STATIC: c_int = 0x2000;
pub const RTA_SEARCH: c_int = 0x4000;

pub const RTAX_SRC: c_int = 8;
pub const RTAX_SRCMASK: c_int = 9;
pub const RTAX_LABEL: c_int = 10;
pub const RTAX_BFD: c_int = 11;
pub const RTAX_DNS: c_int = 12;
pub const RTAX_STATIC: c_int = 13;
pub const RTAX_SEARCH: c_int = 14;
pub const RTAX_MAX: c_int = 15;

const_fn! {
    {const} fn _ALIGN(p: usize) -> usize {
        (p + _ALIGNBYTES) & !_ALIGNBYTES
    }
}

f! {
    pub fn CMSG_DATA(cmsg: *const cmsghdr) -> *mut c_uchar {
        (cmsg as *mut c_uchar).offset(_ALIGN(size_of::<cmsghdr>()) as isize)
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
}

safe_f! {
    pub {const} fn WSTOPSIG(status: c_int) -> c_int {
        status >> 8
    }

    pub {const} fn WIFSIGNALED(status: c_int) -> bool {
        (status & 0o177) != 0o177 && (status & 0o177) != 0
    }

    pub {const} fn WIFSTOPPED(status: c_int) -> bool {
        (status & 0xff) == 0o177
    }

    pub {const} fn WIFCONTINUED(status: c_int) -> bool {
        (status & 0o177777) == 0o177777
    }

    pub {const} fn makedev(major: c_uint, minor: c_uint) -> crate::dev_t {
        let major = major as crate::dev_t;
        let minor = minor as crate::dev_t;
        let mut dev = 0;
        dev |= (major & 0xff) << 8;
        dev |= minor & 0xff;
        dev |= (minor & 0xffff00) << 8;
        dev
    }

    pub {const} fn major(dev: crate::dev_t) -> c_uint {
        ((dev as c_uint) >> 8) & 0xff
    }

    pub {const} fn minor(dev: crate::dev_t) -> c_uint {
        let dev = dev as c_uint;
        let mut res = 0;
        res |= (dev) & 0xff;
        res |= ((dev) & 0xffff0000) >> 8;
        res
    }
}

extern "C" {
    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut crate::timezone) -> c_int;
    pub fn settimeofday(tp: *const crate::timeval, tz: *const crate::timezone) -> c_int;
    pub fn pledge(promises: *const c_char, execpromises: *const c_char) -> c_int;
    pub fn unveil(path: *const c_char, permissions: *const c_char) -> c_int;
    pub fn strtonum(
        nptr: *const c_char,
        minval: c_longlong,
        maxval: c_longlong,
        errstr: *mut *const c_char,
    ) -> c_longlong;
    pub fn dup3(src: c_int, dst: c_int, flags: c_int) -> c_int;
    pub fn chflags(path: *const c_char, flags: c_uint) -> c_int;
    pub fn fchflags(fd: c_int, flags: c_uint) -> c_int;
    pub fn chflagsat(fd: c_int, path: *const c_char, flags: c_uint, atflag: c_int) -> c_int;
    pub fn dirfd(dirp: *mut crate::DIR) -> c_int;
    pub fn getnameinfo(
        sa: *const crate::sockaddr,
        salen: crate::socklen_t,
        host: *mut c_char,
        hostlen: size_t,
        serv: *mut c_char,
        servlen: size_t,
        flags: c_int,
    ) -> c_int;
    pub fn getresgid(
        rgid: *mut crate::gid_t,
        egid: *mut crate::gid_t,
        sgid: *mut crate::gid_t,
    ) -> c_int;
    pub fn getresuid(
        ruid: *mut crate::uid_t,
        euid: *mut crate::uid_t,
        suid: *mut crate::uid_t,
    ) -> c_int;
    pub fn kevent(
        kq: c_int,
        changelist: *const crate::kevent,
        nchanges: c_int,
        eventlist: *mut crate::kevent,
        nevents: c_int,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;
    pub fn getthrid() -> crate::pid_t;
    pub fn pthread_attr_getguardsize(
        attr: *const crate::pthread_attr_t,
        guardsize: *mut size_t,
    ) -> c_int;
    pub fn pthread_attr_setguardsize(attr: *mut crate::pthread_attr_t, guardsize: size_t) -> c_int;
    pub fn pthread_attr_getstack(
        attr: *const crate::pthread_attr_t,
        stackaddr: *mut *mut c_void,
        stacksize: *mut size_t,
    ) -> c_int;
    pub fn pthread_main_np() -> c_int;
    pub fn pthread_get_name_np(tid: crate::pthread_t, name: *mut c_char, len: size_t);
    pub fn pthread_set_name_np(tid: crate::pthread_t, name: *const c_char);
    pub fn pthread_stackseg_np(thread: crate::pthread_t, sinfo: *mut crate::stack_t) -> c_int;

    pub fn openpty(
        amaster: *mut c_int,
        aslave: *mut c_int,
        name: *mut c_char,
        termp: *const crate::termios,
        winp: *const crate::winsize,
    ) -> c_int;
    pub fn forkpty(
        amaster: *mut c_int,
        name: *mut c_char,
        termp: *const crate::termios,
        winp: *const crate::winsize,
    ) -> crate::pid_t;

    pub fn sysctl(
        name: *const c_int,
        namelen: c_uint,
        oldp: *mut c_void,
        oldlenp: *mut size_t,
        newp: *mut c_void,
        newlen: size_t,
    ) -> c_int;
    pub fn setresgid(rgid: crate::gid_t, egid: crate::gid_t, sgid: crate::gid_t) -> c_int;
    pub fn setresuid(ruid: crate::uid_t, euid: crate::uid_t, suid: crate::uid_t) -> c_int;
    pub fn ptrace(request: c_int, pid: crate::pid_t, addr: caddr_t, data: c_int) -> c_int;
    pub fn utrace(label: *const c_char, addr: *const c_void, len: size_t) -> c_int;
    pub fn memmem(
        haystack: *const c_void,
        haystacklen: size_t,
        needle: *const c_void,
        needlelen: size_t,
    ) -> *mut c_void;
    // #include <link.h>
    pub fn dl_iterate_phdr(
        callback: Option<
            unsafe extern "C" fn(info: *mut dl_phdr_info, size: usize, data: *mut c_void) -> c_int,
        >,
        data: *mut c_void,
    ) -> c_int;
    pub fn uselocale(loc: crate::locale_t) -> crate::locale_t;
    pub fn freelocale(loc: crate::locale_t);
    pub fn newlocale(mask: c_int, locale: *const c_char, base: crate::locale_t) -> crate::locale_t;
    pub fn duplocale(base: crate::locale_t) -> crate::locale_t;

    // Added in `OpenBSD` 5.5
    pub fn explicit_bzero(s: *mut c_void, len: size_t);

    pub fn setproctitle(fmt: *const c_char, ...);

    pub fn freezero(ptr: *mut c_void, size: size_t);
    pub fn malloc_conceal(size: size_t) -> *mut c_void;
    pub fn calloc_conceal(nmemb: size_t, size: size_t) -> *mut c_void;

    pub fn srand48_deterministic(seed: c_long);
    pub fn seed48_deterministic(xseed: *mut c_ushort) -> *mut c_ushort;
    pub fn lcong48_deterministic(p: *mut c_ushort);

    pub fn lsearch(
        key: *const c_void,
        base: *mut c_void,
        nelp: *mut size_t,
        width: size_t,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>,
    ) -> *mut c_void;
    pub fn lfind(
        key: *const c_void,
        base: *const c_void,
        nelp: *mut size_t,
        width: size_t,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>,
    ) -> *mut c_void;
    pub fn hcreate(nelt: size_t) -> c_int;
    pub fn hdestroy();
    pub fn hsearch(entry: crate::ENTRY, action: crate::ACTION) -> *mut crate::ENTRY;

    // futex.h
    pub fn futex(
        uaddr: *mut u32,
        op: c_int,
        val: c_int,
        timeout: *const crate::timespec,
        uaddr2: *mut u32,
    ) -> c_int;

    pub fn mimmutable(addr: *mut c_void, len: size_t) -> c_int;

    pub fn reboot(mode: c_int) -> c_int;

    pub fn statfs(path: *const c_char, buf: *mut statfs) -> c_int;
    pub fn fstatfs(fd: c_int, buf: *mut statfs) -> c_int;
    pub fn getmntinfo(mntbufp: *mut *mut crate::statfs, flags: c_int) -> c_int;
    pub fn getfsstat(buf: *mut statfs, bufsize: size_t, flags: c_int) -> c_int;
}

#[link(name = "execinfo")]
extern "C" {
    pub fn backtrace(addrlist: *mut *mut c_void, len: size_t) -> size_t;
    pub fn backtrace_symbols(addrlist: *const *mut c_void, len: size_t) -> *mut *mut c_char;
    pub fn backtrace_symbols_fd(addrlist: *const *mut c_void, len: size_t, fd: c_int) -> c_int;
    pub fn backtrace_symbols_fmt(
        addrlist: *const *mut c_void,
        len: size_t,
        fmt: *const c_char,
    ) -> *mut *mut c_char;
}

cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(target_arch = "arm")] {
        mod arm;
        pub use self::arm::*;
    } else if #[cfg(target_arch = "mips64")] {
        mod mips64;
        pub use self::mips64::*;
    } else if #[cfg(target_arch = "powerpc")] {
        mod powerpc;
        pub use self::powerpc::*;
    } else if #[cfg(target_arch = "powerpc64")] {
        mod powerpc64;
        pub use self::powerpc64::*;
    } else if #[cfg(target_arch = "riscv64")] {
        mod riscv64;
        pub use self::riscv64::*;
    } else if #[cfg(target_arch = "sparc64")] {
        mod sparc64;
        pub use self::sparc64::*;
    } else if #[cfg(target_arch = "x86")] {
        mod x86;
        pub use self::x86::*;
    } else if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    } else {
        // Unknown target_arch
    }
}
