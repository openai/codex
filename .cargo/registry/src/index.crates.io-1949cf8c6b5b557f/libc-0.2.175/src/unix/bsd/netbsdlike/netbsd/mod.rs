use crate::prelude::*;
use crate::{cmsghdr, off_t};

pub type clock_t = c_uint;
pub type suseconds_t = c_int;
pub type dev_t = u64;
pub type blksize_t = i32;
pub type fsblkcnt_t = u64;
pub type fsfilcnt_t = u64;
pub type idtype_t = c_int;
pub type mqd_t = c_int;
type __pthread_spin_t = __cpu_simple_lock_nv_t;
pub type vm_size_t = crate::uintptr_t; // FIXME(deprecated): deprecated since long time
pub type lwpid_t = c_uint;
pub type shmatt_t = c_uint;
pub type cpuid_t = c_ulong;
pub type cpuset_t = _cpuset;
pub type pthread_spin_t = c_uchar;
pub type timer_t = c_int;

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

pub type iconv_t = *mut c_void;

e! {
    #[repr(C)]
    pub enum fae_action {
        FAE_OPEN,
        FAE_DUP2,
        FAE_CLOSE,
    }
}

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

impl siginfo_t {
    pub unsafe fn si_addr(&self) -> *mut c_void {
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
            _si_errno: c_int,
            _si_code: c_int,
            __pad1: c_int,
            _pid: crate::pid_t,
        }
        (*(self as *const siginfo_t as *const siginfo_timer))._pid
    }

    pub unsafe fn si_uid(&self) -> crate::uid_t {
        #[repr(C)]
        struct siginfo_timer {
            _si_signo: c_int,
            _si_errno: c_int,
            _si_code: c_int,
            __pad1: c_int,
            _pid: crate::pid_t,
            _uid: crate::uid_t,
        }
        (*(self as *const siginfo_t as *const siginfo_timer))._uid
    }

    pub unsafe fn si_value(&self) -> crate::sigval {
        #[repr(C)]
        struct siginfo_timer {
            _si_signo: c_int,
            _si_errno: c_int,
            _si_code: c_int,
            __pad1: c_int,
            _pid: crate::pid_t,
            _uid: crate::uid_t,
            value: crate::sigval,
        }
        (*(self as *const siginfo_t as *const siginfo_timer)).value
    }

    pub unsafe fn si_status(&self) -> c_int {
        #[repr(C)]
        struct siginfo_timer {
            _si_signo: c_int,
            _si_errno: c_int,
            _si_code: c_int,
            __pad1: c_int,
            _pid: crate::pid_t,
            _uid: crate::uid_t,
            _value: crate::sigval,
            _cpid: crate::pid_t,
            _cuid: crate::uid_t,
            status: c_int,
        }
        (*(self as *const siginfo_t as *const siginfo_timer)).status
    }
}

s! {
    pub struct aiocb {
        pub aio_offset: off_t,
        pub aio_buf: *mut c_void,
        pub aio_nbytes: size_t,
        pub aio_fildes: c_int,
        pub aio_lio_opcode: c_int,
        pub aio_reqprio: c_int,
        pub aio_sigevent: crate::sigevent,
        _state: c_int,
        _errno: c_int,
        _retval: ssize_t,
    }

    pub struct glob_t {
        pub gl_pathc: size_t,
        pub gl_matchc: size_t,
        pub gl_offs: size_t,
        pub gl_flags: c_int,
        pub gl_pathv: *mut *mut c_char,

        __unused3: *mut c_void,

        __unused4: *mut c_void,
        __unused5: *mut c_void,
        __unused6: *mut c_void,
        __unused7: *mut c_void,
        __unused8: *mut c_void,
    }

    pub struct mq_attr {
        pub mq_flags: c_long,
        pub mq_maxmsg: c_long,
        pub mq_msgsize: c_long,
        pub mq_curmsgs: c_long,
    }

    pub struct itimerspec {
        pub it_interval: crate::timespec,
        pub it_value: crate::timespec,
    }

    pub struct sigset_t {
        __bits: [u32; 4],
    }

    pub struct stat {
        pub st_dev: crate::dev_t,
        pub st_mode: crate::mode_t,
        pub st_ino: crate::ino_t,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        pub st_atime: crate::time_t,
        pub st_atimensec: c_long,
        pub st_mtime: crate::time_t,
        pub st_mtimensec: c_long,
        pub st_ctime: crate::time_t,
        pub st_ctimensec: c_long,
        pub st_birthtime: crate::time_t,
        pub st_birthtimensec: c_long,
        pub st_size: off_t,
        pub st_blocks: crate::blkcnt_t,
        pub st_blksize: crate::blksize_t,
        pub st_flags: u32,
        pub st_gen: u32,
        pub st_spare: [u32; 2],
    }

    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        pub ai_addrlen: crate::socklen_t,
        pub ai_canonname: *mut c_char,
        pub ai_addr: *mut crate::sockaddr,
        pub ai_next: *mut crate::addrinfo,
    }

    pub struct siginfo_t {
        pub si_signo: c_int,
        pub si_code: c_int,
        pub si_errno: c_int,
        __pad1: c_int,
        pub si_addr: *mut c_void,
        __pad2: [u64; 13],
    }

    pub struct pthread_attr_t {
        pta_magic: c_uint,
        pta_flags: c_int,
        pta_private: *mut c_void,
    }

    pub struct pthread_mutex_t {
        ptm_magic: c_uint,
        ptm_errorcheck: __pthread_spin_t,
        #[cfg(any(
            target_arch = "sparc",
            target_arch = "sparc64",
            target_arch = "x86",
            target_arch = "x86_64"
        ))]
        ptm_pad1: [u8; 3],
        // actually a union with a non-unused, 0-initialized field
        ptm_unused: __pthread_spin_t,
        #[cfg(any(
            target_arch = "sparc",
            target_arch = "sparc64",
            target_arch = "x86",
            target_arch = "x86_64"
        ))]
        ptm_pad2: [u8; 3],
        ptm_owner: crate::pthread_t,
        ptm_waiters: *mut u8,
        ptm_recursed: c_uint,
        ptm_spare2: *mut c_void,
    }

    pub struct pthread_mutexattr_t {
        ptma_magic: c_uint,
        ptma_private: *mut c_void,
    }

    pub struct pthread_rwlockattr_t {
        ptra_magic: c_uint,
        ptra_private: *mut c_void,
    }

    pub struct pthread_cond_t {
        ptc_magic: c_uint,
        ptc_lock: __pthread_spin_t,
        ptc_waiters_first: *mut u8,
        ptc_waiters_last: *mut u8,
        ptc_mutex: *mut crate::pthread_mutex_t,
        ptc_private: *mut c_void,
    }

    pub struct pthread_condattr_t {
        ptca_magic: c_uint,
        ptca_private: *mut c_void,
    }

    pub struct pthread_rwlock_t {
        ptr_magic: c_uint,
        ptr_interlock: __pthread_spin_t,
        ptr_rblocked_first: *mut u8,
        ptr_rblocked_last: *mut u8,
        ptr_wblocked_first: *mut u8,
        ptr_wblocked_last: *mut u8,
        ptr_nreaders: c_uint,
        ptr_owner: crate::pthread_t,
        ptr_private: *mut c_void,
    }

    pub struct pthread_spinlock_t {
        pts_magic: c_uint,
        pts_spin: crate::pthread_spin_t,
        pts_flags: c_int,
    }

    pub struct kevent {
        pub ident: crate::uintptr_t,
        pub filter: u32,
        pub flags: u32,
        pub fflags: u32,
        pub data: i64,
        // FIXME(netbsd): NetBSD 10.0 will finally have same layout as other BSD
        pub udata: intptr_t,
    }

    pub struct dqblk {
        pub dqb_bhardlimit: u32,
        pub dqb_bsoftlimit: u32,
        pub dqb_curblocks: u32,
        pub dqb_ihardlimit: u32,
        pub dqb_isoftlimit: u32,
        pub dqb_curinodes: u32,
        pub dqb_btime: i32,
        pub dqb_itime: i32,
    }

    pub struct Dl_info {
        pub dli_fname: *const c_char,
        pub dli_fbase: *mut c_void,
        pub dli_sname: *const c_char,
        pub dli_saddr: *const c_void,
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
        pub int_n_cs_precedes: c_char,
        pub int_p_sep_by_space: c_char,
        pub int_n_sep_by_space: c_char,
        pub int_p_sign_posn: c_char,
        pub int_n_sign_posn: c_char,
    }

    pub struct if_data {
        pub ifi_type: c_uchar,
        pub ifi_addrlen: c_uchar,
        pub ifi_hdrlen: c_uchar,
        pub ifi_link_state: c_int,
        pub ifi_mtu: u64,
        pub ifi_metric: u64,
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
        pub ifi_noproto: u64,
        pub ifi_lastchange: crate::timespec,
    }

    pub struct if_msghdr {
        pub ifm_msglen: c_ushort,
        pub ifm_version: c_uchar,
        pub ifm_type: c_uchar,
        pub ifm_addrs: c_int,
        pub ifm_flags: c_int,
        pub ifm_index: c_ushort,
        pub ifm_data: if_data,
    }

    pub struct sockcred {
        pub sc_pid: crate::pid_t,
        pub sc_uid: crate::uid_t,
        pub sc_euid: crate::uid_t,
        pub sc_gid: crate::gid_t,
        pub sc_egid: crate::gid_t,
        pub sc_ngroups: c_int,
        pub sc_groups: [crate::gid_t; 1],
    }

    pub struct uucred {
        pub cr_unused: c_ushort,
        pub cr_uid: crate::uid_t,
        pub cr_gid: crate::gid_t,
        pub cr_ngroups: c_int,
        pub cr_groups: [crate::gid_t; NGROUPS_MAX as usize],
    }

    pub struct unpcbid {
        pub unp_pid: crate::pid_t,
        pub unp_euid: crate::uid_t,
        pub unp_egid: crate::gid_t,
    }

    pub struct sockaddr_dl {
        pub sdl_len: c_uchar,
        pub sdl_family: c_uchar,
        pub sdl_index: c_ushort,
        pub sdl_type: u8,
        pub sdl_nlen: u8,
        pub sdl_alen: u8,
        pub sdl_slen: u8,
        pub sdl_data: [c_char; 12],
    }

    pub struct __exit_status {
        pub e_termination: u16,
        pub e_exit: u16,
    }

    pub struct shmid_ds {
        pub shm_perm: crate::ipc_perm,
        pub shm_segsz: size_t,
        pub shm_lpid: crate::pid_t,
        pub shm_cpid: crate::pid_t,
        pub shm_nattch: crate::shmatt_t,
        pub shm_atime: crate::time_t,
        pub shm_dtime: crate::time_t,
        pub shm_ctime: crate::time_t,
        _shm_internal: *mut c_void,
    }

    pub struct utmp {
        pub ut_line: [c_char; UT_LINESIZE],
        pub ut_name: [c_char; UT_NAMESIZE],
        pub ut_host: [c_char; UT_HOSTSIZE],
        pub ut_time: crate::time_t,
    }

    pub struct lastlog {
        pub ll_line: [c_char; UT_LINESIZE],
        pub ll_host: [c_char; UT_HOSTSIZE],
        pub ll_time: crate::time_t,
    }

    pub struct timex {
        pub modes: c_uint,
        pub offset: c_long,
        pub freq: c_long,
        pub maxerror: c_long,
        pub esterror: c_long,
        pub status: c_int,
        pub constant: c_long,
        pub precision: c_long,
        pub tolerance: c_long,
        pub ppsfreq: c_long,
        pub jitter: c_long,
        pub shift: c_int,
        pub stabil: c_long,
        pub jitcnt: c_long,
        pub calcnt: c_long,
        pub errcnt: c_long,
        pub stbcnt: c_long,
    }

    pub struct ntptimeval {
        pub time: crate::timespec,
        pub maxerror: c_long,
        pub esterror: c_long,
        pub tai: c_long,
        pub time_state: c_int,
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

    pub struct Aux32Info {
        pub a_type: Elf32_Word,
        pub a_v: Elf32_Word,
    }

    pub struct Aux64Info {
        pub a_type: Elf64_Word,
        pub a_v: Elf64_Xword,
    }

    // link.h

    pub struct dl_phdr_info {
        pub dlpi_addr: Elf_Addr,
        pub dlpi_name: *const c_char,
        pub dlpi_phdr: *const Elf_Phdr,
        pub dlpi_phnum: Elf_Half,
        pub dlpi_adds: c_ulonglong,
        pub dlpi_subs: c_ulonglong,
        pub dlpi_tls_modid: usize,
        pub dlpi_tls_data: *mut c_void,
    }

    pub struct _cpuset {
        bits: [u32; 0],
    }

    pub struct accept_filter_arg {
        pub af_name: [c_char; 16],
        af_arg: [c_char; 256 - 16],
    }

    pub struct ki_sigset_t {
        pub __bits: [u32; 4],
    }

    pub struct kinfo_proc2 {
        pub p_forw: u64,
        pub p_back: u64,
        pub p_paddr: u64,
        pub p_addr: u64,
        pub p_fd: u64,
        pub p_cwdi: u64,
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
        pub p_siglist: ki_sigset_t,
        pub p_sigmask: ki_sigset_t,
        pub p_sigignore: ki_sigset_t,
        pub p_sigcatch: ki_sigset_t,
        pub p_stat: i8,
        pub p_priority: u8,
        pub p_usrpri: u8,
        pub p_nice: u8,
        pub p_xstat: u16,
        pub p_acflag: u16,
        pub p_comm: [c_char; KI_MAXCOMLEN as usize],
        pub p_wmesg: [c_char; KI_WMESGLEN as usize],
        pub p_wchan: u64,
        pub p_login: [c_char; KI_MAXLOGNAME as usize],
        pub p_vm_rssize: i32,
        pub p_vm_tsize: i32,
        pub p_vm_dsize: i32,
        pub p_vm_ssize: i32,
        pub p_uvalid: i64,
        pub p_ustart_sec: u32,
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
        pub p_cpuid: u64,
        pub p_realflag: u64,
        pub p_nlwps: u64,
        pub p_nrlwps: u64,
        pub p_realstat: u64,
        pub p_svuid: u32,
        pub p_svgid: u32,
        pub p_ename: [c_char; KI_MAXEMULLEN as usize],
        pub p_vm_vsize: i64,
        pub p_vm_msize: i64,
    }

    pub struct kinfo_lwp {
        pub l_forw: u64,
        pub l_back: u64,
        pub l_laddr: u64,
        pub l_addr: u64,
        pub l_lid: i32,
        pub l_flag: i32,
        pub l_swtime: u32,
        pub l_slptime: u32,
        pub l_schedflags: i32,
        pub l_holdcnt: i32,
        pub l_priority: u8,
        pub l_usrpri: u8,
        pub l_stat: i8,
        l_pad1: i8,
        l_pad2: i32,
        pub l_wmesg: [c_char; KI_WMESGLEN as usize],
        pub l_wchan: u64,
        pub l_cpuid: u64,
        pub l_rtime_sec: u32,
        pub l_rtime_usec: u32,
        pub l_cpticks: u32,
        pub l_pctcpu: u32,
        pub l_pid: u32,
        pub l_name: [c_char; KI_LNAMELEN as usize],
    }

    pub struct kinfo_vmentry {
        pub kve_start: u64,
        pub kve_end: u64,
        pub kve_offset: u64,
        pub kve_type: u32,
        pub kve_flags: u32,
        pub kve_count: u32,
        pub kve_wired_count: u32,
        pub kve_advice: u32,
        pub kve_attributes: u32,
        pub kve_protection: u32,
        pub kve_max_protection: u32,
        pub kve_ref_count: u32,
        pub kve_inheritance: u32,
        pub kve_vn_fileid: u64,
        pub kve_vn_size: u64,
        pub kve_vn_fsid: u64,
        pub kve_vn_rdev: u64,
        pub kve_vn_type: u32,
        pub kve_vn_mode: u32,
        pub kve_path: [[c_char; 32]; 32],
    }

    pub struct __c_anonymous_posix_spawn_fae_open {
        pub path: *mut c_char,
        pub oflag: c_int,
        pub mode: crate::mode_t,
    }

    pub struct __c_anonymous_posix_spawn_fae_dup2 {
        pub newfildes: c_int,
    }

    pub struct posix_spawnattr_t {
        pub sa_flags: c_short,
        pub sa_pgroup: crate::pid_t,
        pub sa_schedparam: crate::sched_param,
        pub sa_schedpolicy: c_int,
        pub sa_sigdefault: sigset_t,
        pub sa_sigmask: sigset_t,
    }

    pub struct posix_spawn_file_actions_entry_t {
        pub fae_action: fae_action,
        pub fae_fildes: c_int,
        pub fae_data: __c_anonymous_posix_spawn_fae,
    }

    pub struct posix_spawn_file_actions_t {
        pub size: c_uint,
        pub len: c_uint,
        pub fae: *mut posix_spawn_file_actions_entry_t,
    }

    pub struct ptrace_lwpinfo {
        pub pl_lwpid: lwpid_t,
        pub pl_event: c_int,
    }

    pub struct ptrace_lwpstatus {
        pub pl_lwpid: lwpid_t,
        pub pl_sigpend: sigset_t,
        pub pl_sigmask: sigset_t,
        pub pl_name: [c_char; 20],
        pub pl_private: *mut c_void,
    }

    pub struct ptrace_siginfo {
        pub psi_siginfo: siginfo_t,
        pub psi_lwpid: lwpid_t,
    }

    pub struct ptrace_event {
        pub pe_set_event: c_int,
    }

    pub struct sysctldesc {
        pub descr_num: i32,
        pub descr_ver: u32,
        pub descr_len: u32,
        pub descr_str: [c_char; 1],
    }

    pub struct ifreq {
        pub _priv: [[c_char; 6]; 24],
    }

    pub struct ifconf {
        pub ifc_len: c_int,
        pub ifc_ifcu: __c_anonymous_ifc_ifcu,
    }

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
        pub __tcpi_pad: [u32; 26],
    }
}

s_no_extra_traits! {
    pub struct utmpx {
        pub ut_name: [c_char; _UTX_USERSIZE],
        pub ut_id: [c_char; _UTX_IDSIZE],
        pub ut_line: [c_char; _UTX_LINESIZE],
        pub ut_host: [c_char; _UTX_HOSTSIZE],
        pub ut_session: u16,
        pub ut_type: u16,
        pub ut_pid: crate::pid_t,
        pub ut_exit: __exit_status, // FIXME(netbsd): when anonymous struct are supported
        pub ut_ss: sockaddr_storage,
        pub ut_tv: crate::timeval,
        pub ut_pad: [u8; _UTX_PADSIZE],
    }

    pub struct lastlogx {
        pub ll_tv: crate::timeval,
        pub ll_line: [c_char; _UTX_LINESIZE],
        pub ll_host: [c_char; _UTX_HOSTSIZE],
        pub ll_ss: sockaddr_storage,
    }

    pub struct in_pktinfo {
        pub ipi_addr: crate::in_addr,
        pub ipi_ifindex: c_uint,
    }

    pub struct arphdr {
        pub ar_hrd: u16,
        pub ar_pro: u16,
        pub ar_hln: u8,
        pub ar_pln: u8,
        pub ar_op: u16,
    }

    pub struct in_addr {
        pub s_addr: crate::in_addr_t,
    }

    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub struct sockaddr_in {
        pub sin_len: u8,
        pub sin_family: crate::sa_family_t,
        pub sin_port: crate::in_port_t,
        pub sin_addr: crate::in_addr,
        pub sin_zero: [i8; 8],
    }

    pub struct dirent {
        pub d_fileno: crate::ino_t,
        pub d_reclen: u16,
        pub d_namlen: u16,
        pub d_type: u8,
        pub d_name: [c_char; 512],
    }

    pub struct statvfs {
        pub f_flag: c_ulong,
        pub f_bsize: c_ulong,
        pub f_frsize: c_ulong,
        pub f_iosize: c_ulong,

        pub f_blocks: crate::fsblkcnt_t,
        pub f_bfree: crate::fsblkcnt_t,
        pub f_bavail: crate::fsblkcnt_t,
        pub f_bresvd: crate::fsblkcnt_t,

        pub f_files: crate::fsfilcnt_t,
        pub f_ffree: crate::fsfilcnt_t,
        pub f_favail: crate::fsfilcnt_t,
        pub f_fresvd: crate::fsfilcnt_t,

        pub f_syncreads: u64,
        pub f_syncwrites: u64,

        pub f_asyncreads: u64,
        pub f_asyncwrites: u64,

        pub f_fsidx: crate::fsid_t,
        pub f_fsid: c_ulong,
        pub f_namemax: c_ulong,
        pub f_owner: crate::uid_t,

        pub f_spare: [u32; 4],

        pub f_fstypename: [c_char; 32],
        pub f_mntonname: [c_char; 1024],
        pub f_mntfromname: [c_char; 1024],
    }

    pub struct sockaddr_storage {
        pub ss_len: u8,
        pub ss_family: crate::sa_family_t,
        __ss_pad1: [u8; 6],
        __ss_pad2: i64,
        __ss_pad3: [u8; 112],
    }

    pub struct sigevent {
        pub sigev_notify: c_int,
        pub sigev_signo: c_int,
        pub sigev_value: crate::sigval,
        __unused1: *mut c_void, //actually a function pointer
        pub sigev_notify_attributes: *mut c_void,
    }

    pub union __c_anonymous_posix_spawn_fae {
        pub open: __c_anonymous_posix_spawn_fae_open,
        pub dup2: __c_anonymous_posix_spawn_fae_dup2,
    }

    pub union __c_anonymous_ifc_ifcu {
        pub ifcu_buf: *mut c_void,
        pub ifcu_req: *mut ifreq,
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for utmpx {
            fn eq(&self, other: &utmpx) -> bool {
                self.ut_type == other.ut_type
                    && self.ut_pid == other.ut_pid
                    && self.ut_name == other.ut_name
                    && self.ut_line == other.ut_line
                    && self.ut_id == other.ut_id
                    && self.ut_exit == other.ut_exit
                    && self.ut_session == other.ut_session
                    && self.ut_tv == other.ut_tv
                    && self.ut_ss == other.ut_ss
                    && self
                        .ut_pad
                        .iter()
                        .zip(other.ut_pad.iter())
                        .all(|(a, b)| a == b)
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
                self.ut_name.hash(state);
                self.ut_type.hash(state);
                self.ut_pid.hash(state);
                self.ut_line.hash(state);
                self.ut_id.hash(state);
                self.ut_host.hash(state);
                self.ut_exit.hash(state);
                self.ut_session.hash(state);
                self.ut_tv.hash(state);
                self.ut_ss.hash(state);
                self.ut_pad.hash(state);
            }
        }

        impl PartialEq for lastlogx {
            fn eq(&self, other: &lastlogx) -> bool {
                self.ll_tv == other.ll_tv
                    && self.ll_line == other.ll_line
                    && self.ll_ss == other.ll_ss
                    && self
                        .ll_host
                        .iter()
                        .zip(other.ll_host.iter())
                        .all(|(a, b)| a == b)
            }
        }

        impl Eq for lastlogx {}

        impl hash::Hash for lastlogx {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ll_tv.hash(state);
                self.ll_line.hash(state);
                self.ll_host.hash(state);
                self.ll_ss.hash(state);
            }
        }

        impl PartialEq for in_pktinfo {
            fn eq(&self, other: &in_pktinfo) -> bool {
                self.ipi_addr == other.ipi_addr && self.ipi_ifindex == other.ipi_ifindex
            }
        }
        impl Eq for in_pktinfo {}
        impl hash::Hash for in_pktinfo {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ipi_addr.hash(state);
                self.ipi_ifindex.hash(state);
            }
        }

        impl PartialEq for arphdr {
            fn eq(&self, other: &arphdr) -> bool {
                self.ar_hrd == other.ar_hrd
                    && self.ar_pro == other.ar_pro
                    && self.ar_hln == other.ar_hln
                    && self.ar_pln == other.ar_pln
                    && self.ar_op == other.ar_op
            }
        }
        impl Eq for arphdr {}
        impl hash::Hash for arphdr {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                let ar_hrd = self.ar_hrd;
                let ar_pro = self.ar_pro;
                let ar_op = self.ar_op;
                ar_hrd.hash(state);
                ar_pro.hash(state);
                self.ar_hln.hash(state);
                self.ar_pln.hash(state);
                ar_op.hash(state);
            }
        }

        impl PartialEq for in_addr {
            fn eq(&self, other: &in_addr) -> bool {
                self.s_addr == other.s_addr
            }
        }
        impl Eq for in_addr {}
        impl hash::Hash for in_addr {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                let s_addr = self.s_addr;
                s_addr.hash(state);
            }
        }

        impl PartialEq for ip_mreq {
            fn eq(&self, other: &ip_mreq) -> bool {
                self.imr_multiaddr == other.imr_multiaddr
                    && self.imr_interface == other.imr_interface
            }
        }
        impl Eq for ip_mreq {}
        impl hash::Hash for ip_mreq {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.imr_multiaddr.hash(state);
                self.imr_interface.hash(state);
            }
        }

        impl PartialEq for sockaddr_in {
            fn eq(&self, other: &sockaddr_in) -> bool {
                self.sin_len == other.sin_len
                    && self.sin_family == other.sin_family
                    && self.sin_port == other.sin_port
                    && self.sin_addr == other.sin_addr
                    && self.sin_zero == other.sin_zero
            }
        }
        impl Eq for sockaddr_in {}
        impl hash::Hash for sockaddr_in {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sin_len.hash(state);
                self.sin_family.hash(state);
                self.sin_port.hash(state);
                self.sin_addr.hash(state);
                self.sin_zero.hash(state);
            }
        }

        impl PartialEq for dirent {
            fn eq(&self, other: &dirent) -> bool {
                self.d_fileno == other.d_fileno
                    && self.d_reclen == other.d_reclen
                    && self.d_namlen == other.d_namlen
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
                self.d_fileno.hash(state);
                self.d_reclen.hash(state);
                self.d_namlen.hash(state);
                self.d_type.hash(state);
                self.d_name.hash(state);
            }
        }

        impl PartialEq for statvfs {
            fn eq(&self, other: &statvfs) -> bool {
                self.f_flag == other.f_flag
                    && self.f_bsize == other.f_bsize
                    && self.f_frsize == other.f_frsize
                    && self.f_iosize == other.f_iosize
                    && self.f_blocks == other.f_blocks
                    && self.f_bfree == other.f_bfree
                    && self.f_bavail == other.f_bavail
                    && self.f_bresvd == other.f_bresvd
                    && self.f_files == other.f_files
                    && self.f_ffree == other.f_ffree
                    && self.f_favail == other.f_favail
                    && self.f_fresvd == other.f_fresvd
                    && self.f_syncreads == other.f_syncreads
                    && self.f_syncwrites == other.f_syncwrites
                    && self.f_asyncreads == other.f_asyncreads
                    && self.f_asyncwrites == other.f_asyncwrites
                    && self.f_fsidx == other.f_fsidx
                    && self.f_fsid == other.f_fsid
                    && self.f_namemax == other.f_namemax
                    && self.f_owner == other.f_owner
                    && self.f_spare == other.f_spare
                    && self.f_fstypename == other.f_fstypename
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
            }
        }
        impl Eq for statvfs {}
        impl hash::Hash for statvfs {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.f_flag.hash(state);
                self.f_bsize.hash(state);
                self.f_frsize.hash(state);
                self.f_iosize.hash(state);
                self.f_blocks.hash(state);
                self.f_bfree.hash(state);
                self.f_bavail.hash(state);
                self.f_bresvd.hash(state);
                self.f_files.hash(state);
                self.f_ffree.hash(state);
                self.f_favail.hash(state);
                self.f_fresvd.hash(state);
                self.f_syncreads.hash(state);
                self.f_syncwrites.hash(state);
                self.f_asyncreads.hash(state);
                self.f_asyncwrites.hash(state);
                self.f_fsidx.hash(state);
                self.f_fsid.hash(state);
                self.f_namemax.hash(state);
                self.f_owner.hash(state);
                self.f_spare.hash(state);
                self.f_fstypename.hash(state);
                self.f_mntonname.hash(state);
                self.f_mntfromname.hash(state);
            }
        }

        impl PartialEq for sockaddr_storage {
            fn eq(&self, other: &sockaddr_storage) -> bool {
                self.ss_len == other.ss_len
                    && self.ss_family == other.ss_family
                    && self.__ss_pad1 == other.__ss_pad1
                    && self.__ss_pad2 == other.__ss_pad2
                    && self
                        .__ss_pad3
                        .iter()
                        .zip(other.__ss_pad3.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for sockaddr_storage {}
        impl hash::Hash for sockaddr_storage {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.ss_len.hash(state);
                self.ss_family.hash(state);
                self.__ss_pad1.hash(state);
                self.__ss_pad2.hash(state);
                self.__ss_pad3.hash(state);
            }
        }

        impl PartialEq for sigevent {
            fn eq(&self, other: &sigevent) -> bool {
                self.sigev_notify == other.sigev_notify
                    && self.sigev_signo == other.sigev_signo
                    && self.sigev_value == other.sigev_value
                    && self.sigev_notify_attributes == other.sigev_notify_attributes
            }
        }
        impl Eq for sigevent {}
        impl hash::Hash for sigevent {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.sigev_notify.hash(state);
                self.sigev_signo.hash(state);
                self.sigev_value.hash(state);
                self.sigev_notify_attributes.hash(state);
            }
        }

        impl Eq for __c_anonymous_posix_spawn_fae {}

        impl PartialEq for __c_anonymous_posix_spawn_fae {
            fn eq(&self, other: &__c_anonymous_posix_spawn_fae) -> bool {
                unsafe { self.open == other.open || self.dup2 == other.dup2 }
            }
        }

        impl hash::Hash for __c_anonymous_posix_spawn_fae {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    self.open.hash(state);
                    self.dup2.hash(state);
                }
            }
        }

        impl Eq for __c_anonymous_ifc_ifcu {}

        impl PartialEq for __c_anonymous_ifc_ifcu {
            fn eq(&self, other: &__c_anonymous_ifc_ifcu) -> bool {
                unsafe { self.ifcu_buf == other.ifcu_buf || self.ifcu_req == other.ifcu_req }
            }
        }

        impl hash::Hash for __c_anonymous_ifc_ifcu {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                unsafe {
                    self.ifcu_buf.hash(state);
                    self.ifcu_req.hash(state);
                }
            }
        }
    }
}

pub const AT_FDCWD: c_int = -100;
pub const AT_EACCESS: c_int = 0x100;
pub const AT_SYMLINK_NOFOLLOW: c_int = 0x200;
pub const AT_SYMLINK_FOLLOW: c_int = 0x400;
pub const AT_REMOVEDIR: c_int = 0x800;

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
pub const AT_DCACHEBSIZE: c_int = 10;
pub const AT_ICACHEBSIZE: c_int = 11;
pub const AT_UCACHEBSIZE: c_int = 12;
pub const AT_STACKBASE: c_int = 13;
pub const AT_EUID: c_int = 2000;
pub const AT_RUID: c_int = 2001;
pub const AT_EGID: c_int = 2002;
pub const AT_RGID: c_int = 2003;
pub const AT_SUN_LDELF: c_int = 2004;
pub const AT_SUN_LDSHDR: c_int = 2005;
pub const AT_SUN_LDNAME: c_int = 2006;
pub const AT_SUN_LDPGSIZE: c_int = 2007;
pub const AT_SUN_PLATFORM: c_int = 2008;
pub const AT_SUN_HWCAP: c_int = 2009;
pub const AT_SUN_IFLUSH: c_int = 2010;
pub const AT_SUN_CPU: c_int = 2011;
pub const AT_SUN_EMUL_ENTRY: c_int = 2012;
pub const AT_SUN_EMUL_EXECFD: c_int = 2013;
pub const AT_SUN_EXECNAME: c_int = 2014;

pub const EXTATTR_NAMESPACE_USER: c_int = 1;
pub const EXTATTR_NAMESPACE_SYSTEM: c_int = 2;

pub const LC_COLLATE_MASK: c_int = 1 << crate::LC_COLLATE;
pub const LC_CTYPE_MASK: c_int = 1 << crate::LC_CTYPE;
pub const LC_MONETARY_MASK: c_int = 1 << crate::LC_MONETARY;
pub const LC_NUMERIC_MASK: c_int = 1 << crate::LC_NUMERIC;
pub const LC_TIME_MASK: c_int = 1 << crate::LC_TIME;
pub const LC_MESSAGES_MASK: c_int = 1 << crate::LC_MESSAGES;
pub const LC_ALL_MASK: c_int = !0;

pub const ERA: crate::nl_item = 52;
pub const ERA_D_FMT: crate::nl_item = 53;
pub const ERA_D_T_FMT: crate::nl_item = 54;
pub const ERA_T_FMT: crate::nl_item = 55;
pub const ALT_DIGITS: crate::nl_item = 56;

pub const O_CLOEXEC: c_int = 0x400000;
pub const O_ALT_IO: c_int = 0x40000;
pub const O_NOSIGPIPE: c_int = 0x1000000;
pub const O_SEARCH: c_int = 0x800000;
pub const O_DIRECTORY: c_int = 0x200000;
pub const O_DIRECT: c_int = 0x00080000;
pub const O_RSYNC: c_int = 0x00020000;

pub const MS_SYNC: c_int = 0x4;
pub const MS_INVALIDATE: c_int = 0x2;

// Here because they are not present on OpenBSD
// (https://github.com/openbsd/src/blob/HEAD/sys/sys/resource.h)
pub const RLIMIT_SBSIZE: c_int = 9;
pub const RLIMIT_AS: c_int = 10;
pub const RLIMIT_NTHR: c_int = 11;

#[deprecated(since = "0.2.64", note = "Not stable across OS versions")]
pub const RLIM_NLIMITS: c_int = 12;

pub const EIDRM: c_int = 82;
pub const ENOMSG: c_int = 83;
pub const EOVERFLOW: c_int = 84;
pub const EILSEQ: c_int = 85;
pub const ENOTSUP: c_int = 86;
pub const ECANCELED: c_int = 87;
pub const EBADMSG: c_int = 88;
pub const ENODATA: c_int = 89;
pub const ENOSR: c_int = 90;
pub const ENOSTR: c_int = 91;
pub const ETIME: c_int = 92;
pub const ENOATTR: c_int = 93;
pub const EMULTIHOP: c_int = 94;
pub const ENOLINK: c_int = 95;
pub const EPROTO: c_int = 96;
pub const EOWNERDEAD: c_int = 97;
pub const ENOTRECOVERABLE: c_int = 98;
#[deprecated(
    since = "0.2.143",
    note = "This value will always match the highest defined error number \
            and thus is not stable. \
            See #3040 for more info."
)]
pub const ELAST: c_int = 98;

pub const F_DUPFD_CLOEXEC: c_int = 12;
pub const F_CLOSEM: c_int = 10;
pub const F_GETNOSIGPIPE: c_int = 13;
pub const F_SETNOSIGPIPE: c_int = 14;
pub const F_MAXFD: c_int = 11;
pub const F_GETPATH: c_int = 15;

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
pub const FUTEX_PRIVATE_FLAG: c_int = 1 << 7;
pub const FUTEX_CLOCK_REALTIME: c_int = 1 << 8;
pub const FUTEX_CMD_MASK: c_int = !(FUTEX_PRIVATE_FLAG | FUTEX_CLOCK_REALTIME);
pub const FUTEX_WAITERS: u32 = 1 << 31;
pub const FUTEX_OWNER_DIED: u32 = 1 << 30;
pub const FUTEX_SYNCOBJ_1: u32 = 1 << 29;
pub const FUTEX_SYNCOBJ_0: u32 = 1 << 28;
pub const FUTEX_TID_MASK: u32 = (1 << 28) - 1;
pub const FUTEX_BITSET_MATCH_ANY: u32 = !0;

pub const IP_RECVDSTADDR: c_int = 7;
pub const IP_SENDSRCADDR: c_int = IP_RECVDSTADDR;
pub const IP_RECVIF: c_int = 20;
pub const IP_PKTINFO: c_int = 25;
pub const IP_RECVPKTINFO: c_int = 26;
pub const IPV6_JOIN_GROUP: c_int = 12;
pub const IPV6_LEAVE_GROUP: c_int = 13;

pub const TCP_KEEPIDLE: c_int = 3;
pub const TCP_KEEPINTVL: c_int = 5;
pub const TCP_KEEPCNT: c_int = 6;
pub const TCP_KEEPINIT: c_int = 7;
pub const TCP_MD5SIG: c_int = 0x10;
pub const TCP_CONGCTL: c_int = 0x20;

pub const SOCK_CONN_DGRAM: c_int = 6;
pub const SOCK_DCCP: c_int = SOCK_CONN_DGRAM;
pub const SOCK_NOSIGPIPE: c_int = 0x40000000;
pub const SOCK_FLAGS_MASK: c_int = 0xf0000000;

pub const SO_SNDTIMEO: c_int = 0x100b;
pub const SO_RCVTIMEO: c_int = 0x100c;
pub const SO_NOSIGPIPE: c_int = 0x0800;
pub const SO_ACCEPTFILTER: c_int = 0x1000;
pub const SO_TIMESTAMP: c_int = 0x2000;
pub const SO_OVERFLOWED: c_int = 0x1009;
pub const SO_NOHEADER: c_int = 0x100a;

// http://cvsweb.netbsd.org/bsdweb.cgi/src/sys/sys/un.h?annotate
pub const LOCAL_OCREDS: c_int = 0x0001; // pass credentials to receiver
pub const LOCAL_CONNWAIT: c_int = 0x0002; // connects block until accepted
pub const LOCAL_PEEREID: c_int = 0x0003; // get peer identification
pub const LOCAL_CREDS: c_int = 0x0004; // pass credentials to receiver

// https://github.com/NetBSD/src/blob/trunk/sys/net/if.h#L373
pub const IFF_UP: c_int = 0x0001; // interface is up
pub const IFF_BROADCAST: c_int = 0x0002; // broadcast address valid
pub const IFF_DEBUG: c_int = 0x0004; // turn on debugging
pub const IFF_LOOPBACK: c_int = 0x0008; // is a loopback net
pub const IFF_POINTOPOINT: c_int = 0x0010; // interface is point-to-point link
pub const IFF_NOTRAILERS: c_int = 0x0020; // avoid use of trailers
pub const IFF_RUNNING: c_int = 0x0040; // resources allocated
pub const IFF_NOARP: c_int = 0x0080; // no address resolution protocol
pub const IFF_PROMISC: c_int = 0x0100; // receive all packets
pub const IFF_ALLMULTI: c_int = 0x0200; // receive all multicast packets
pub const IFF_OACTIVE: c_int = 0x0400; // transmission in progress
pub const IFF_SIMPLEX: c_int = 0x0800; // can't hear own transmissions
pub const IFF_LINK0: c_int = 0x1000; // per link layer defined bit
pub const IFF_LINK1: c_int = 0x2000; // per link layer defined bit
pub const IFF_LINK2: c_int = 0x4000; // per link layer defined bit
pub const IFF_MULTICAST: c_int = 0x8000; // supports multicast

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
/// IP Mobility RFC 2004
pub const IPPROTO_MOBILE: c_int = 55;
/// IPv6 ICMP
pub const IPPROTO_IPV6_ICMP: c_int = 58;
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
/// VRRP RFC 2338
pub const IPPROTO_VRRP: c_int = 112;
/// Common Address Resolution Protocol
pub const IPPROTO_CARP: c_int = 112;
/// L2TPv3
pub const IPPROTO_L2TP: c_int = 115;
/// SCTP
pub const IPPROTO_SCTP: c_int = 132;
/// PFSYNC
pub const IPPROTO_PFSYNC: c_int = 240;
pub const IPPROTO_MAX: c_int = 256;

/// last return value of *_input(), meaning "all job for this pkt is done".
pub const IPPROTO_DONE: c_int = 257;

/// sysctl placeholder for (FAST_)IPSEC
pub const CTL_IPPROTO_IPSEC: c_int = 258;

pub const AF_OROUTE: c_int = 17;
pub const AF_ARP: c_int = 28;
pub const pseudo_AF_KEY: c_int = 29;
pub const pseudo_AF_HDRCMPLT: c_int = 30;
pub const AF_BLUETOOTH: c_int = 31;
pub const AF_IEEE80211: c_int = 32;
pub const AF_MPLS: c_int = 33;
pub const AF_ROUTE: c_int = 34;
pub const NET_RT_DUMP: c_int = 1;
pub const NET_RT_FLAGS: c_int = 2;
pub const NET_RT_OOOIFLIST: c_int = 3;
pub const NET_RT_OOIFLIST: c_int = 4;
pub const NET_RT_OIFLIST: c_int = 5;
pub const NET_RT_IFLIST: c_int = 6;
pub const NET_RT_MAXID: c_int = 7;

pub const PF_OROUTE: c_int = AF_OROUTE;
pub const PF_ARP: c_int = AF_ARP;
pub const PF_KEY: c_int = pseudo_AF_KEY;
pub const PF_BLUETOOTH: c_int = AF_BLUETOOTH;
pub const PF_MPLS: c_int = AF_MPLS;
pub const PF_ROUTE: c_int = AF_ROUTE;

pub const MSG_NBIO: c_int = 0x1000;
pub const MSG_WAITFORONE: c_int = 0x2000;
pub const MSG_NOTIFICATION: c_int = 0x4000;

pub const SCM_TIMESTAMP: c_int = 0x08;
pub const SCM_CREDS: c_int = 0x10;

pub const O_DSYNC: c_int = 0x10000;

pub const MAP_RENAME: c_int = 0x20;
pub const MAP_NORESERVE: c_int = 0x40;
pub const MAP_HASSEMAPHORE: c_int = 0x200;
pub const MAP_TRYFIXED: c_int = 0x400;
pub const MAP_WIRED: c_int = 0x800;
pub const MAP_STACK: c_int = 0x2000;
// map alignment aliases for MAP_ALIGNED
pub const MAP_ALIGNMENT_SHIFT: c_int = 24;
pub const MAP_ALIGNMENT_MASK: c_int = 0xff << MAP_ALIGNMENT_SHIFT;
pub const MAP_ALIGNMENT_64KB: c_int = 16 << MAP_ALIGNMENT_SHIFT;
pub const MAP_ALIGNMENT_16MB: c_int = 24 << MAP_ALIGNMENT_SHIFT;
pub const MAP_ALIGNMENT_4GB: c_int = 32 << MAP_ALIGNMENT_SHIFT;
pub const MAP_ALIGNMENT_1TB: c_int = 40 << MAP_ALIGNMENT_SHIFT;
pub const MAP_ALIGNMENT_256TB: c_int = 48 << MAP_ALIGNMENT_SHIFT;
pub const MAP_ALIGNMENT_64PB: c_int = 56 << MAP_ALIGNMENT_SHIFT;
// mremap flag
pub const MAP_REMAPDUP: c_int = 0x004;

pub const DCCP_TYPE_REQUEST: c_int = 0;
pub const DCCP_TYPE_RESPONSE: c_int = 1;
pub const DCCP_TYPE_DATA: c_int = 2;
pub const DCCP_TYPE_ACK: c_int = 3;
pub const DCCP_TYPE_DATAACK: c_int = 4;
pub const DCCP_TYPE_CLOSEREQ: c_int = 5;
pub const DCCP_TYPE_CLOSE: c_int = 6;
pub const DCCP_TYPE_RESET: c_int = 7;
pub const DCCP_TYPE_MOVE: c_int = 8;

pub const DCCP_FEATURE_CC: c_int = 1;
pub const DCCP_FEATURE_ECN: c_int = 2;
pub const DCCP_FEATURE_ACKRATIO: c_int = 3;
pub const DCCP_FEATURE_ACKVECTOR: c_int = 4;
pub const DCCP_FEATURE_MOBILITY: c_int = 5;
pub const DCCP_FEATURE_LOSSWINDOW: c_int = 6;
pub const DCCP_FEATURE_CONN_NONCE: c_int = 8;
pub const DCCP_FEATURE_IDENTREG: c_int = 7;

pub const DCCP_OPT_PADDING: c_int = 0;
pub const DCCP_OPT_DATA_DISCARD: c_int = 1;
pub const DCCP_OPT_SLOW_RECV: c_int = 2;
pub const DCCP_OPT_BUF_CLOSED: c_int = 3;
pub const DCCP_OPT_CHANGE_L: c_int = 32;
pub const DCCP_OPT_CONFIRM_L: c_int = 33;
pub const DCCP_OPT_CHANGE_R: c_int = 34;
pub const DCCP_OPT_CONFIRM_R: c_int = 35;
pub const DCCP_OPT_INIT_COOKIE: c_int = 36;
pub const DCCP_OPT_NDP_COUNT: c_int = 37;
pub const DCCP_OPT_ACK_VECTOR0: c_int = 38;
pub const DCCP_OPT_ACK_VECTOR1: c_int = 39;
pub const DCCP_OPT_RECV_BUF_DROPS: c_int = 40;
pub const DCCP_OPT_TIMESTAMP: c_int = 41;
pub const DCCP_OPT_TIMESTAMP_ECHO: c_int = 42;
pub const DCCP_OPT_ELAPSEDTIME: c_int = 43;
pub const DCCP_OPT_DATACHECKSUM: c_int = 44;

pub const DCCP_REASON_UNSPEC: c_int = 0;
pub const DCCP_REASON_CLOSED: c_int = 1;
pub const DCCP_REASON_INVALID: c_int = 2;
pub const DCCP_REASON_OPTION_ERR: c_int = 3;
pub const DCCP_REASON_FEA_ERR: c_int = 4;
pub const DCCP_REASON_CONN_REF: c_int = 5;
pub const DCCP_REASON_BAD_SNAME: c_int = 6;
pub const DCCP_REASON_BAD_COOKIE: c_int = 7;
pub const DCCP_REASON_INV_MOVE: c_int = 8;
pub const DCCP_REASON_UNANSW_CH: c_int = 10;
pub const DCCP_REASON_FRUITLESS_NEG: c_int = 11;

pub const DCCP_CCID: c_int = 1;
pub const DCCP_CSLEN: c_int = 2;
pub const DCCP_MAXSEG: c_int = 4;
pub const DCCP_SERVICE: c_int = 8;

pub const DCCP_NDP_LIMIT: c_int = 16;
pub const DCCP_SEQ_NUM_LIMIT: c_int = 16777216;
pub const DCCP_MAX_OPTIONS: c_int = 32;
pub const DCCP_MAX_PKTS: c_int = 100;

pub const _PC_LINK_MAX: c_int = 1;
pub const _PC_MAX_CANON: c_int = 2;
pub const _PC_MAX_INPUT: c_int = 3;
pub const _PC_NAME_MAX: c_int = 4;
pub const _PC_PATH_MAX: c_int = 5;
pub const _PC_PIPE_BUF: c_int = 6;
pub const _PC_CHOWN_RESTRICTED: c_int = 7;
pub const _PC_NO_TRUNC: c_int = 8;
pub const _PC_VDISABLE: c_int = 9;
pub const _PC_SYNC_IO: c_int = 10;
pub const _PC_FILESIZEBITS: c_int = 11;
pub const _PC_SYMLINK_MAX: c_int = 12;
pub const _PC_2_SYMLINKS: c_int = 13;
pub const _PC_ACL_EXTENDED: c_int = 14;
pub const _PC_MIN_HOLE_SIZE: c_int = 15;

pub const _SC_SYNCHRONIZED_IO: c_int = 31;
pub const _SC_IOV_MAX: c_int = 32;
pub const _SC_MAPPED_FILES: c_int = 33;
pub const _SC_MEMLOCK: c_int = 34;
pub const _SC_MEMLOCK_RANGE: c_int = 35;
pub const _SC_MEMORY_PROTECTION: c_int = 36;
pub const _SC_LOGIN_NAME_MAX: c_int = 37;
pub const _SC_MONOTONIC_CLOCK: c_int = 38;
pub const _SC_CLK_TCK: c_int = 39;
pub const _SC_ATEXIT_MAX: c_int = 40;
pub const _SC_THREADS: c_int = 41;
pub const _SC_SEMAPHORES: c_int = 42;
pub const _SC_BARRIERS: c_int = 43;
pub const _SC_TIMERS: c_int = 44;
pub const _SC_SPIN_LOCKS: c_int = 45;
pub const _SC_READER_WRITER_LOCKS: c_int = 46;
pub const _SC_GETGR_R_SIZE_MAX: c_int = 47;
pub const _SC_GETPW_R_SIZE_MAX: c_int = 48;
pub const _SC_CLOCK_SELECTION: c_int = 49;
pub const _SC_ASYNCHRONOUS_IO: c_int = 50;
pub const _SC_AIO_LISTIO_MAX: c_int = 51;
pub const _SC_AIO_MAX: c_int = 52;
pub const _SC_MESSAGE_PASSING: c_int = 53;
pub const _SC_MQ_OPEN_MAX: c_int = 54;
pub const _SC_MQ_PRIO_MAX: c_int = 55;
pub const _SC_PRIORITY_SCHEDULING: c_int = 56;
pub const _SC_THREAD_DESTRUCTOR_ITERATIONS: c_int = 57;
pub const _SC_THREAD_KEYS_MAX: c_int = 58;
pub const _SC_THREAD_STACK_MIN: c_int = 59;
pub const _SC_THREAD_THREADS_MAX: c_int = 60;
pub const _SC_THREAD_ATTR_STACKADDR: c_int = 61;
pub const _SC_THREAD_ATTR_STACKSIZE: c_int = 62;
pub const _SC_THREAD_PRIORITY_SCHEDULING: c_int = 63;
pub const _SC_THREAD_PRIO_INHERIT: c_int = 64;
pub const _SC_THREAD_PRIO_PROTECT: c_int = 65;
pub const _SC_THREAD_PROCESS_SHARED: c_int = 66;
pub const _SC_THREAD_SAFE_FUNCTIONS: c_int = 67;
pub const _SC_TTY_NAME_MAX: c_int = 68;
pub const _SC_HOST_NAME_MAX: c_int = 69;
pub const _SC_PASS_MAX: c_int = 70;
pub const _SC_REGEXP: c_int = 71;
pub const _SC_SHELL: c_int = 72;
pub const _SC_SYMLOOP_MAX: c_int = 73;
pub const _SC_V6_ILP32_OFF32: c_int = 74;
pub const _SC_V6_ILP32_OFFBIG: c_int = 75;
pub const _SC_V6_LP64_OFF64: c_int = 76;
pub const _SC_V6_LPBIG_OFFBIG: c_int = 77;
pub const _SC_2_PBS: c_int = 80;
pub const _SC_2_PBS_ACCOUNTING: c_int = 81;
pub const _SC_2_PBS_CHECKPOINT: c_int = 82;
pub const _SC_2_PBS_LOCATE: c_int = 83;
pub const _SC_2_PBS_MESSAGE: c_int = 84;
pub const _SC_2_PBS_TRACK: c_int = 85;
pub const _SC_SPAWN: c_int = 86;
pub const _SC_SHARED_MEMORY_OBJECTS: c_int = 87;
pub const _SC_TIMER_MAX: c_int = 88;
pub const _SC_SEM_NSEMS_MAX: c_int = 89;
pub const _SC_CPUTIME: c_int = 90;
pub const _SC_THREAD_CPUTIME: c_int = 91;
pub const _SC_DELAYTIMER_MAX: c_int = 92;
// These two variables will be supported in NetBSD 8.0
// pub const _SC_SIGQUEUE_MAX : c_int = 93;
// pub const _SC_REALTIME_SIGNALS : c_int = 94;
pub const _SC_PHYS_PAGES: c_int = 121;
pub const _SC_NPROCESSORS_CONF: c_int = 1001;
pub const _SC_NPROCESSORS_ONLN: c_int = 1002;
pub const _SC_SCHED_RT_TS: c_int = 2001;
pub const _SC_SCHED_PRI_MIN: c_int = 2002;
pub const _SC_SCHED_PRI_MAX: c_int = 2003;

pub const FD_SETSIZE: usize = 0x100;

pub const ST_NOSUID: c_ulong = 8;

pub const BIOCGRSIG: c_ulong = 0x40044272;
pub const BIOCSRSIG: c_ulong = 0x80044273;
pub const BIOCSDLT: c_ulong = 0x80044278;
pub const BIOCGSEESENT: c_ulong = 0x40044276;
pub const BIOCSSEESENT: c_ulong = 0x80044277;

// <sys/fstypes.h>
pub const MNT_UNION: c_int = 0x00000020;
pub const MNT_NOCOREDUMP: c_int = 0x00008000;
pub const MNT_RELATIME: c_int = 0x00020000;
pub const MNT_IGNORE: c_int = 0x00100000;
pub const MNT_NFS4ACLS: c_int = 0x00200000;
pub const MNT_DISCARD: c_int = 0x00800000;
pub const MNT_EXTATTR: c_int = 0x01000000;
pub const MNT_LOG: c_int = 0x02000000;
pub const MNT_NOATIME: c_int = 0x04000000;
pub const MNT_AUTOMOUNTED: c_int = 0x10000000;
pub const MNT_SYMPERM: c_int = 0x20000000;
pub const MNT_NODEVMTIME: c_int = 0x40000000;
pub const MNT_SOFTDEP: c_int = 0x80000000;
pub const MNT_POSIX1EACLS: c_int = 0x00000800;
pub const MNT_ACLS: c_int = MNT_POSIX1EACLS;
pub const MNT_WAIT: c_int = 1;
pub const MNT_NOWAIT: c_int = 2;
pub const MNT_LAZY: c_int = 3;

// sys/ioccom.h
pub const IOCPARM_SHIFT: u32 = 16;
pub const IOCGROUP_SHIFT: u32 = 8;

pub const fn IOCPARM_LEN(x: u32) -> u32 {
    (x >> IOCPARM_SHIFT) & crate::IOCPARM_MASK
}

pub const fn IOCBASECMD(x: u32) -> u32 {
    x & (!(crate::IOCPARM_MASK << IOCPARM_SHIFT))
}

pub const fn IOCGROUP(x: u32) -> u32 {
    (x >> IOCGROUP_SHIFT) & 0xff
}

pub const fn _IOC(inout: c_ulong, group: c_ulong, num: c_ulong, len: c_ulong) -> c_ulong {
    (inout)
        | (((len) & crate::IOCPARM_MASK as c_ulong) << IOCPARM_SHIFT)
        | ((group) << IOCGROUP_SHIFT)
        | (num)
}

//<sys/timex.h>
pub const CLOCK_PROCESS_CPUTIME_ID: crate::clockid_t = 2;
pub const CLOCK_THREAD_CPUTIME_ID: crate::clockid_t = 4;
pub const NTP_API: c_int = 4;
pub const MAXPHASE: c_long = 500000000;
pub const MAXFREQ: c_long = 500000;
pub const MINSEC: c_int = 256;
pub const MAXSEC: c_int = 2048;
pub const NANOSECOND: c_long = 1000000000;
pub const SCALE_PPM: c_int = 65;
pub const MAXTC: c_int = 10;
pub const MOD_OFFSET: c_uint = 0x0001;
pub const MOD_FREQUENCY: c_uint = 0x0002;
pub const MOD_MAXERROR: c_uint = 0x0004;
pub const MOD_ESTERROR: c_uint = 0x0008;
pub const MOD_STATUS: c_uint = 0x0010;
pub const MOD_TIMECONST: c_uint = 0x0020;
pub const MOD_PPSMAX: c_uint = 0x0040;
pub const MOD_TAI: c_uint = 0x0080;
pub const MOD_MICRO: c_uint = 0x1000;
pub const MOD_NANO: c_uint = 0x2000;
pub const MOD_CLKB: c_uint = 0x4000;
pub const MOD_CLKA: c_uint = 0x8000;
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
pub const TIME_OK: c_int = 0;
pub const TIME_INS: c_int = 1;
pub const TIME_DEL: c_int = 2;
pub const TIME_OOP: c_int = 3;
pub const TIME_WAIT: c_int = 4;
pub const TIME_ERROR: c_int = 5;

pub const LITTLE_ENDIAN: c_int = 1234;
pub const BIG_ENDIAN: c_int = 4321;

pub const PL_EVENT_NONE: c_int = 0;
pub const PL_EVENT_SIGNAL: c_int = 1;
pub const PL_EVENT_SUSPENDED: c_int = 2;

cfg_if! {
    if #[cfg(any(
        target_arch = "sparc",
        target_arch = "sparc64",
        target_arch = "x86",
        target_arch = "x86_64"
    ))] {
        pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
            ptm_magic: 0x33330003,
            ptm_errorcheck: 0,
            ptm_pad1: [0; 3],
            ptm_unused: 0,
            ptm_pad2: [0; 3],
            ptm_waiters: 0 as *mut _,
            ptm_owner: 0,
            ptm_recursed: 0,
            ptm_spare2: 0 as *mut _,
        };
    } else {
        pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
            ptm_magic: 0x33330003,
            ptm_errorcheck: 0,
            ptm_unused: 0,
            ptm_waiters: 0 as *mut _,
            ptm_owner: 0,
            ptm_recursed: 0,
            ptm_spare2: 0 as *mut _,
        };
    }
}

pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    ptc_magic: 0x55550005,
    ptc_lock: 0,
    ptc_waiters_first: 0 as *mut _,
    ptc_waiters_last: 0 as *mut _,
    ptc_mutex: 0 as *mut _,
    ptc_private: 0 as *mut _,
};
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = pthread_rwlock_t {
    ptr_magic: 0x99990009,
    ptr_interlock: 0,
    ptr_rblocked_first: 0 as *mut _,
    ptr_rblocked_last: 0 as *mut _,
    ptr_wblocked_first: 0 as *mut _,
    ptr_wblocked_last: 0 as *mut _,
    ptr_nreaders: 0,
    ptr_owner: 0,
    ptr_private: 0 as *mut _,
};
pub const PTHREAD_MUTEX_NORMAL: c_int = 0;
pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 1;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 2;
pub const PTHREAD_MUTEX_DEFAULT: c_int = PTHREAD_MUTEX_NORMAL;

pub const SCHED_NONE: c_int = -1;
pub const SCHED_OTHER: c_int = 0;
pub const SCHED_FIFO: c_int = 1;
pub const SCHED_RR: c_int = 2;

pub const EVFILT_AIO: u32 = 2;
pub const EVFILT_PROC: u32 = 4;
pub const EVFILT_READ: u32 = 0;
pub const EVFILT_SIGNAL: u32 = 5;
pub const EVFILT_TIMER: u32 = 6;
pub const EVFILT_VNODE: u32 = 3;
pub const EVFILT_WRITE: u32 = 1;
pub const EVFILT_FS: u32 = 7;
pub const EVFILT_USER: u32 = 8;
pub const EVFILT_EMPTY: u32 = 9;

pub const EV_ADD: u32 = 0x1;
pub const EV_DELETE: u32 = 0x2;
pub const EV_ENABLE: u32 = 0x4;
pub const EV_DISABLE: u32 = 0x8;
pub const EV_ONESHOT: u32 = 0x10;
pub const EV_CLEAR: u32 = 0x20;
pub const EV_RECEIPT: u32 = 0x40;
pub const EV_DISPATCH: u32 = 0x80;
pub const EV_FLAG1: u32 = 0x2000;
pub const EV_ERROR: u32 = 0x4000;
pub const EV_EOF: u32 = 0x8000;
pub const EV_SYSFLAGS: u32 = 0xf000;

pub const NOTE_TRIGGER: u32 = 0x01000000;
pub const NOTE_FFNOP: u32 = 0x00000000;
pub const NOTE_FFAND: u32 = 0x40000000;
pub const NOTE_FFOR: u32 = 0x80000000;
pub const NOTE_FFCOPY: u32 = 0xc0000000;
pub const NOTE_FFCTRLMASK: u32 = 0xc0000000;
pub const NOTE_FFLAGSMASK: u32 = 0x00ffffff;
pub const NOTE_LOWAT: u32 = 0x00000001;
pub const NOTE_DELETE: u32 = 0x00000001;
pub const NOTE_WRITE: u32 = 0x00000002;
pub const NOTE_EXTEND: u32 = 0x00000004;
pub const NOTE_ATTRIB: u32 = 0x00000008;
pub const NOTE_LINK: u32 = 0x00000010;
pub const NOTE_RENAME: u32 = 0x00000020;
pub const NOTE_REVOKE: u32 = 0x00000040;
pub const NOTE_EXIT: u32 = 0x80000000;
pub const NOTE_FORK: u32 = 0x40000000;
pub const NOTE_EXEC: u32 = 0x20000000;
pub const NOTE_PDATAMASK: u32 = 0x000fffff;
pub const NOTE_PCTRLMASK: u32 = 0xf0000000;
pub const NOTE_TRACK: u32 = 0x00000001;
pub const NOTE_TRACKERR: u32 = 0x00000002;
pub const NOTE_CHILD: u32 = 0x00000004;
pub const NOTE_MSECONDS: u32 = 0x00000000;
pub const NOTE_SECONDS: u32 = 0x00000001;
pub const NOTE_USECONDS: u32 = 0x00000002;
pub const NOTE_NSECONDS: u32 = 0x00000003;
pub const NOTE_ABSTIME: u32 = 0x000000010;

pub const TMP_MAX: c_uint = 308915776;

pub const AI_PASSIVE: c_int = 0x00000001;
pub const AI_CANONNAME: c_int = 0x00000002;
pub const AI_NUMERICHOST: c_int = 0x00000004;
pub const AI_NUMERICSERV: c_int = 0x00000008;
pub const AI_ADDRCONFIG: c_int = 0x00000400;
pub const AI_SRV: c_int = 0x00000800;

pub const NI_MAXHOST: crate::socklen_t = 1025;
pub const NI_MAXSERV: crate::socklen_t = 32;

pub const NI_NOFQDN: c_int = 0x00000001;
pub const NI_NUMERICHOST: c_int = 0x000000002;
pub const NI_NAMEREQD: c_int = 0x000000004;
pub const NI_NUMERICSERV: c_int = 0x000000008;
pub const NI_DGRAM: c_int = 0x00000010;
pub const NI_WITHSCOPEID: c_int = 0x00000020;
pub const NI_NUMERICSCOPE: c_int = 0x00000040;

pub const RTLD_NOLOAD: c_int = 0x2000;
pub const RTLD_LOCAL: c_int = 0x200;

pub const CTL_MAXNAME: c_int = 12;
pub const SYSCTL_NAMELEN: c_int = 32;
pub const SYSCTL_DEFSIZE: c_int = 8;
pub const CTLTYPE_NODE: c_int = 1;
pub const CTLTYPE_INT: c_int = 2;
pub const CTLTYPE_STRING: c_int = 3;
pub const CTLTYPE_QUAD: c_int = 4;
pub const CTLTYPE_STRUCT: c_int = 5;
pub const CTLTYPE_BOOL: c_int = 6;
pub const CTLFLAG_READONLY: c_int = 0x00000000;
pub const CTLFLAG_READWRITE: c_int = 0x00000070;
pub const CTLFLAG_ANYWRITE: c_int = 0x00000080;
pub const CTLFLAG_PRIVATE: c_int = 0x00000100;
pub const CTLFLAG_PERMANENT: c_int = 0x00000200;
pub const CTLFLAG_OWNDATA: c_int = 0x00000400;
pub const CTLFLAG_IMMEDIATE: c_int = 0x00000800;
pub const CTLFLAG_HEX: c_int = 0x00001000;
pub const CTLFLAG_ROOT: c_int = 0x00002000;
pub const CTLFLAG_ANYNUMBER: c_int = 0x00004000;
pub const CTLFLAG_HIDDEN: c_int = 0x00008000;
pub const CTLFLAG_ALIAS: c_int = 0x00010000;
pub const CTLFLAG_MMAP: c_int = 0x00020000;
pub const CTLFLAG_OWNDESC: c_int = 0x00040000;
pub const CTLFLAG_UNSIGNED: c_int = 0x00080000;
pub const SYSCTL_VERS_MASK: c_int = 0xff000000;
pub const SYSCTL_VERS_0: c_int = 0x00000000;
pub const SYSCTL_VERS_1: c_int = 0x01000000;
pub const SYSCTL_VERSION: c_int = SYSCTL_VERS_1;
pub const CTL_EOL: c_int = -1;
pub const CTL_QUERY: c_int = -2;
pub const CTL_CREATE: c_int = -3;
pub const CTL_CREATESYM: c_int = -4;
pub const CTL_DESTROY: c_int = -5;
pub const CTL_MMAP: c_int = -6;
pub const CTL_DESCRIBE: c_int = -7;
pub const CTL_UNSPEC: c_int = 0;
pub const CTL_KERN: c_int = 1;
pub const CTL_VM: c_int = 2;
pub const CTL_VFS: c_int = 3;
pub const CTL_NET: c_int = 4;
pub const CTL_DEBUG: c_int = 5;
pub const CTL_HW: c_int = 6;
pub const CTL_MACHDEP: c_int = 7;
pub const CTL_USER: c_int = 8;
pub const CTL_DDB: c_int = 9;
pub const CTL_PROC: c_int = 10;
pub const CTL_VENDOR: c_int = 11;
pub const CTL_EMUL: c_int = 12;
pub const CTL_SECURITY: c_int = 13;
pub const CTL_MAXID: c_int = 14;
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
pub const KERN_OBOOTTIME: c_int = 21;
pub const KERN_DOMAINNAME: c_int = 22;
pub const KERN_MAXPARTITIONS: c_int = 23;
pub const KERN_RAWPARTITION: c_int = 24;
pub const KERN_NTPTIME: c_int = 25;
pub const KERN_TIMEX: c_int = 26;
pub const KERN_AUTONICETIME: c_int = 27;
pub const KERN_AUTONICEVAL: c_int = 28;
pub const KERN_RTC_OFFSET: c_int = 29;
pub const KERN_ROOT_DEVICE: c_int = 30;
pub const KERN_MSGBUFSIZE: c_int = 31;
pub const KERN_FSYNC: c_int = 32;
pub const KERN_OLDSYSVMSG: c_int = 33;
pub const KERN_OLDSYSVSEM: c_int = 34;
pub const KERN_OLDSYSVSHM: c_int = 35;
pub const KERN_OLDSHORTCORENAME: c_int = 36;
pub const KERN_SYNCHRONIZED_IO: c_int = 37;
pub const KERN_IOV_MAX: c_int = 38;
pub const KERN_MBUF: c_int = 39;
pub const KERN_MAPPED_FILES: c_int = 40;
pub const KERN_MEMLOCK: c_int = 41;
pub const KERN_MEMLOCK_RANGE: c_int = 42;
pub const KERN_MEMORY_PROTECTION: c_int = 43;
pub const KERN_LOGIN_NAME_MAX: c_int = 44;
pub const KERN_DEFCORENAME: c_int = 45;
pub const KERN_LOGSIGEXIT: c_int = 46;
pub const KERN_PROC2: c_int = 47;
pub const KERN_PROC_ARGS: c_int = 48;
pub const KERN_FSCALE: c_int = 49;
pub const KERN_CCPU: c_int = 50;
pub const KERN_CP_TIME: c_int = 51;
pub const KERN_OLDSYSVIPC_INFO: c_int = 52;
pub const KERN_MSGBUF: c_int = 53;
pub const KERN_CONSDEV: c_int = 54;
pub const KERN_MAXPTYS: c_int = 55;
pub const KERN_PIPE: c_int = 56;
pub const KERN_MAXPHYS: c_int = 57;
pub const KERN_SBMAX: c_int = 58;
pub const KERN_TKSTAT: c_int = 59;
pub const KERN_MONOTONIC_CLOCK: c_int = 60;
pub const KERN_URND: c_int = 61;
pub const KERN_LABELSECTOR: c_int = 62;
pub const KERN_LABELOFFSET: c_int = 63;
pub const KERN_LWP: c_int = 64;
pub const KERN_FORKFSLEEP: c_int = 65;
pub const KERN_POSIX_THREADS: c_int = 66;
pub const KERN_POSIX_SEMAPHORES: c_int = 67;
pub const KERN_POSIX_BARRIERS: c_int = 68;
pub const KERN_POSIX_TIMERS: c_int = 69;
pub const KERN_POSIX_SPIN_LOCKS: c_int = 70;
pub const KERN_POSIX_READER_WRITER_LOCKS: c_int = 71;
pub const KERN_DUMP_ON_PANIC: c_int = 72;
pub const KERN_SOMAXKVA: c_int = 73;
pub const KERN_ROOT_PARTITION: c_int = 74;
pub const KERN_DRIVERS: c_int = 75;
pub const KERN_BUF: c_int = 76;
pub const KERN_FILE2: c_int = 77;
pub const KERN_VERIEXEC: c_int = 78;
pub const KERN_CP_ID: c_int = 79;
pub const KERN_HARDCLOCK_TICKS: c_int = 80;
pub const KERN_ARND: c_int = 81;
pub const KERN_SYSVIPC: c_int = 82;
pub const KERN_BOOTTIME: c_int = 83;
pub const KERN_EVCNT: c_int = 84;
pub const KERN_MAXID: c_int = 85;
pub const KERN_PROC_ALL: c_int = 0;
pub const KERN_PROC_PID: c_int = 1;
pub const KERN_PROC_PGRP: c_int = 2;
pub const KERN_PROC_SESSION: c_int = 3;
pub const KERN_PROC_TTY: c_int = 4;
pub const KERN_PROC_UID: c_int = 5;
pub const KERN_PROC_RUID: c_int = 6;
pub const KERN_PROC_GID: c_int = 7;
pub const KERN_PROC_RGID: c_int = 8;
pub const KERN_PROC_ARGV: c_int = 1;
pub const KERN_PROC_NARGV: c_int = 2;
pub const KERN_PROC_ENV: c_int = 3;
pub const KERN_PROC_NENV: c_int = 4;
pub const KERN_PROC_PATHNAME: c_int = 5;
pub const VM_PROC: c_int = 16;
pub const VM_PROC_MAP: c_int = 1;

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

pub const AIO_CANCELED: c_int = 1;
pub const AIO_NOTCANCELED: c_int = 2;
pub const AIO_ALLDONE: c_int = 3;
pub const LIO_NOP: c_int = 0;
pub const LIO_WRITE: c_int = 1;
pub const LIO_READ: c_int = 2;
pub const LIO_WAIT: c_int = 1;
pub const LIO_NOWAIT: c_int = 0;

pub const SIGEV_NONE: c_int = 0;
pub const SIGEV_SIGNAL: c_int = 1;
pub const SIGEV_THREAD: c_int = 2;

pub const WSTOPPED: c_int = 0x00000002; // same as WUNTRACED
pub const WCONTINUED: c_int = 0x00000010;
pub const WEXITED: c_int = 0x000000020;
pub const WNOWAIT: c_int = 0x00010000;

pub const WALTSIG: c_int = 0x00000004;
pub const WALLSIG: c_int = 0x00000008;
pub const WTRAPPED: c_int = 0x00000040;
pub const WNOZOMBIE: c_int = 0x00020000;

pub const P_ALL: idtype_t = 0;
pub const P_PID: idtype_t = 1;
pub const P_PGID: idtype_t = 4;

pub const UTIME_OMIT: c_long = 1073741822;
pub const UTIME_NOW: c_long = 1073741823;

pub const B460800: crate::speed_t = 460800;
pub const B921600: crate::speed_t = 921600;

pub const ONOCR: crate::tcflag_t = 0x20;
pub const ONLRET: crate::tcflag_t = 0x40;
pub const CDTRCTS: crate::tcflag_t = 0x00020000;
pub const CHWFLOW: crate::tcflag_t = crate::MDMBUF | crate::CRTSCTS | crate::CDTRCTS;

// pub const _PATH_UTMPX: &[c_char; 14] = b"/var/run/utmpx";
// pub const _PATH_WTMPX: &[c_char; 14] = b"/var/log/wtmpx";
// pub const _PATH_LASTLOGX: &[c_char; 17] = b"/var/log/lastlogx";
// pub const _PATH_UTMP_UPDATE: &[c_char; 24] = b"/usr/libexec/utmp_update";
pub const UT_NAMESIZE: usize = 8;
pub const UT_LINESIZE: usize = 8;
pub const UT_HOSTSIZE: usize = 16;
pub const _UTX_USERSIZE: usize = 32;
pub const _UTX_LINESIZE: usize = 32;
pub const _UTX_PADSIZE: usize = 40;
pub const _UTX_IDSIZE: usize = 4;
pub const _UTX_HOSTSIZE: usize = 256;
pub const EMPTY: u16 = 0;
pub const RUN_LVL: u16 = 1;
pub const BOOT_TIME: u16 = 2;
pub const OLD_TIME: u16 = 3;
pub const NEW_TIME: u16 = 4;
pub const INIT_PROCESS: u16 = 5;
pub const LOGIN_PROCESS: u16 = 6;
pub const USER_PROCESS: u16 = 7;
pub const DEAD_PROCESS: u16 = 8;
pub const ACCOUNTING: u16 = 9;
pub const SIGNATURE: u16 = 10;
pub const DOWN_TIME: u16 = 11;

pub const SOCK_CLOEXEC: c_int = 0x10000000;
pub const SOCK_NONBLOCK: c_int = 0x20000000;

// Uncomment on next NetBSD release
// pub const FIOSEEKDATA: c_ulong = 0xc0086661;
// pub const FIOSEEKHOLE: c_ulong = 0xc0086662;
pub const OFIOGETBMAP: c_ulong = 0xc004667a;
pub const FIOGETBMAP: c_ulong = 0xc008667a;
pub const FIONWRITE: c_ulong = 0x40046679;
pub const FIONSPACE: c_ulong = 0x40046678;
pub const FIBMAP: c_ulong = 0xc008667a;

pub const SIGSTKSZ: size_t = 40960;

pub const REG_ENOSYS: c_int = 17;

pub const PT_DUMPCORE: c_int = 12;
pub const PT_LWPINFO: c_int = 13;
pub const PT_SYSCALL: c_int = 14;
pub const PT_SYSCALLEMU: c_int = 15;
pub const PT_SET_EVENT_MASK: c_int = 16;
pub const PT_GET_EVENT_MASK: c_int = 17;
pub const PT_GET_PROCESS_STATE: c_int = 18;
pub const PT_SET_SIGINFO: c_int = 19;
pub const PT_GET_SIGINFO: c_int = 20;
pub const PT_RESUME: c_int = 21;
pub const PT_SUSPEND: c_int = 23;
pub const PT_STOP: c_int = 23;
pub const PT_LWPSTATUS: c_int = 24;
pub const PT_LWPNEXT: c_int = 25;
pub const PT_SET_SIGPASS: c_int = 26;
pub const PT_GET_SIGPASS: c_int = 27;
pub const PT_FIRSTMACH: c_int = 32;
pub const POSIX_SPAWN_RETURNERROR: c_int = 0x40;

// Flags for chflags(2)
pub const SF_APPEND: c_ulong = 0x00040000;
pub const SF_ARCHIVED: c_ulong = 0x00010000;
pub const SF_IMMUTABLE: c_ulong = 0x00020000;
pub const SF_LOG: c_ulong = 0x00400000;
pub const SF_SETTABLE: c_ulong = 0xffff0000;
pub const SF_SNAPINVAL: c_ulong = 0x00800000;
pub const SF_SNAPSHOT: c_ulong = 0x00200000;
pub const UF_APPEND: c_ulong = 0x00000004;
pub const UF_IMMUTABLE: c_ulong = 0x00000002;
pub const UF_NODUMP: c_ulong = 0x00000001;
pub const UF_OPAQUE: c_ulong = 0x00000008;
pub const UF_SETTABLE: c_ulong = 0x0000ffff;

// sys/sysctl.h
pub const KVME_PROT_READ: c_int = 0x00000001;
pub const KVME_PROT_WRITE: c_int = 0x00000002;
pub const KVME_PROT_EXEC: c_int = 0x00000004;

pub const KVME_FLAG_COW: c_int = 0x00000001;
pub const KVME_FLAG_NEEDS_COPY: c_int = 0x00000002;
pub const KVME_FLAG_NOCOREDUMP: c_int = 0x000000004;
pub const KVME_FLAG_PAGEABLE: c_int = 0x000000008;
pub const KVME_FLAG_GROWS_UP: c_int = 0x000000010;
pub const KVME_FLAG_GROWS_DOWN: c_int = 0x000000020;

pub const NGROUPS_MAX: c_int = 16;

pub const KI_NGROUPS: c_int = 16;
pub const KI_MAXCOMLEN: c_int = 24;
pub const KI_WMESGLEN: c_int = 8;
pub const KI_MAXLOGNAME: c_int = 24;
pub const KI_MAXEMULLEN: c_int = 16;
pub const KI_LNAMELEN: c_int = 20;

// sys/lwp.h
pub const LSIDL: c_int = 1;
pub const LSRUN: c_int = 2;
pub const LSSLEEP: c_int = 3;
pub const LSSTOP: c_int = 4;
pub const LSZOMB: c_int = 5;
pub const LSONPROC: c_int = 7;
pub const LSSUSPENDED: c_int = 8;

// sys/xattr.h
pub const XATTR_CREATE: c_int = 0x01;
pub const XATTR_REPLACE: c_int = 0x02;
// sys/extattr.h
pub const EXTATTR_NAMESPACE_EMPTY: c_int = 0;

// For getrandom()
pub const GRND_NONBLOCK: c_uint = 0x1;
pub const GRND_RANDOM: c_uint = 0x2;
pub const GRND_INSECURE: c_uint = 0x4;

// sys/reboot.h
pub const RB_ASKNAME: c_int = 0x000000001;
pub const RB_SINGLE: c_int = 0x000000002;
pub const RB_NOSYNC: c_int = 0x000000004;
pub const RB_HALT: c_int = 0x000000008;
pub const RB_INITNAME: c_int = 0x000000010;
pub const RB_KDB: c_int = 0x000000040;
pub const RB_RDONLY: c_int = 0x000000080;
pub const RB_DUMP: c_int = 0x000000100;
pub const RB_MINIROOT: c_int = 0x000000200;
pub const RB_STRING: c_int = 0x000000400;
pub const RB_POWERDOWN: c_int = RB_HALT | 0x000000800;
pub const RB_USERCONF: c_int = 0x000001000;

pub const fn MAP_ALIGNED(alignment: c_int) -> c_int {
    alignment << MAP_ALIGNMENT_SHIFT
}

// net/route.h
pub const RTF_MASK: c_int = 0x80;
pub const RTF_CONNECTED: c_int = 0x100;
pub const RTF_ANNOUNCE: c_int = 0x20000;
pub const RTF_SRC: c_int = 0x10000;
pub const RTF_LOCAL: c_int = 0x40000;
pub const RTF_BROADCAST: c_int = 0x80000;
pub const RTF_UPDATING: c_int = 0x100000;
pub const RTF_DONTCHANGEIFA: c_int = 0x200000;

pub const RTM_VERSION: c_int = 4;
pub const RTM_LOCK: c_int = 0x8;
pub const RTM_IFANNOUNCE: c_int = 0x10;
pub const RTM_IEEE80211: c_int = 0x11;
pub const RTM_SETGATE: c_int = 0x12;
pub const RTM_LLINFO_UPD: c_int = 0x13;
pub const RTM_IFINFO: c_int = 0x14;
pub const RTM_OCHGADDR: c_int = 0x15;
pub const RTM_NEWADDR: c_int = 0x16;
pub const RTM_DELADDR: c_int = 0x17;
pub const RTM_CHGADDR: c_int = 0x18;

pub const RTA_TAG: c_int = 0x100;

pub const RTAX_TAG: c_int = 8;
pub const RTAX_MAX: c_int = 9;

// sys/timerfd.h
pub const TFD_CLOEXEC: i32 = crate::O_CLOEXEC;
pub const TFD_NONBLOCK: i32 = crate::O_NONBLOCK;
pub const TFD_TIMER_ABSTIME: i32 = crate::O_WRONLY;
pub const TFD_TIMER_CANCEL_ON_SET: i32 = crate::O_RDWR;

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

    // dirfd() is a macro on netbsd to access
    // the first field of the struct where dirp points to:
    // http://cvsweb.netbsd.org/bsdweb.cgi/src/include/dirent.h?rev=1.36
    pub fn dirfd(dirp: *mut crate::DIR) -> c_int {
        *(dirp as *const c_int)
    }

    pub fn SOCKCREDSIZE(ngrps: usize) -> usize {
        let ngrps = if ngrps > 0 { ngrps - 1 } else { 0 };
        size_of::<sockcred>() + size_of::<crate::gid_t>() * ngrps
    }

    pub fn PROT_MPROTECT(x: c_int) -> c_int {
        x << 3
    }

    pub fn PROT_MPROTECT_EXTRACT(x: c_int) -> c_int {
        (x >> 3) & 0x7
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
        (status & 0o177) == 0o177
    }

    pub {const} fn WIFCONTINUED(status: c_int) -> bool {
        status == 0xffff
    }

    pub {const} fn makedev(major: c_uint, minor: c_uint) -> crate::dev_t {
        let major = major as crate::dev_t;
        let minor = minor as crate::dev_t;
        let mut dev = 0;
        dev |= (major << 8) & 0x000ff00;
        dev |= (minor << 12) & 0xfff00000;
        dev |= minor & 0xff;
        dev
    }

    pub {const} fn major(dev: crate::dev_t) -> c_int {
        (((dev as u32) & 0x000fff00) >> 8) as c_int
    }

    pub {const} fn minor(dev: crate::dev_t) -> c_int {
        let mut res = 0;
        res |= ((dev as u32) & 0xfff00000) >> 12;
        res |= (dev as u32) & 0x000000ff;
        res as c_int
    }
}

extern "C" {
    pub fn ntp_adjtime(buf: *mut timex) -> c_int;
    pub fn ntp_gettime(buf: *mut ntptimeval) -> c_int;
    pub fn clock_nanosleep(
        clk_id: crate::clockid_t,
        flags: c_int,
        rqtp: *const crate::timespec,
        rmtp: *mut crate::timespec,
    ) -> c_int;

    pub fn reallocarr(ptr: *mut c_void, number: size_t, size: size_t) -> c_int;

    pub fn chflags(path: *const c_char, flags: c_ulong) -> c_int;
    pub fn fchflags(fd: c_int, flags: c_ulong) -> c_int;
    pub fn lchflags(path: *const c_char, flags: c_ulong) -> c_int;

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
    pub fn extattr_namespace_to_string(attrnamespace: c_int, string: *mut *mut c_char) -> c_int;
    pub fn extattr_set_fd(
        fd: c_int,
        attrnamespace: c_int,
        attrname: *const c_char,
        data: *const c_void,
        nbytes: size_t,
    ) -> c_int;
    pub fn extattr_set_file(
        path: *const c_char,
        attrnamespace: c_int,
        attrname: *const c_char,
        data: *const c_void,
        nbytes: size_t,
    ) -> c_int;
    pub fn extattr_set_link(
        path: *const c_char,
        attrnamespace: c_int,
        attrname: *const c_char,
        data: *const c_void,
        nbytes: size_t,
    ) -> c_int;
    pub fn extattr_string_to_namespace(string: *const c_char, attrnamespace: *mut c_int) -> c_int;

    pub fn openpty(
        amaster: *mut c_int,
        aslave: *mut c_int,
        name: *mut c_char,
        termp: *mut crate::termios,
        winp: *mut crate::winsize,
    ) -> c_int;
    pub fn forkpty(
        amaster: *mut c_int,
        name: *mut c_char,
        termp: *mut crate::termios,
        winp: *mut crate::winsize,
    ) -> crate::pid_t;

    pub fn ptsname_r(fd: c_int, buf: *mut c_char, buflen: size_t) -> c_int;

    #[link_name = "__lutimes50"]
    pub fn lutimes(file: *const c_char, times: *const crate::timeval) -> c_int;
    #[link_name = "__gettimeofday50"]
    pub fn gettimeofday(tp: *mut crate::timeval, tz: *mut c_void) -> c_int;
    pub fn getnameinfo(
        sa: *const crate::sockaddr,
        salen: crate::socklen_t,
        host: *mut c_char,
        hostlen: crate::socklen_t,
        serv: *mut c_char,
        servlen: crate::socklen_t,
        flags: c_int,
    ) -> c_int;
    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;
    pub fn sysctl(
        name: *const c_int,
        namelen: c_uint,
        oldp: *mut c_void,
        oldlenp: *mut size_t,
        newp: *const c_void,
        newlen: size_t,
    ) -> c_int;
    pub fn sysctlbyname(
        name: *const c_char,
        oldp: *mut c_void,
        oldlenp: *mut size_t,
        newp: *const c_void,
        newlen: size_t,
    ) -> c_int;
    pub fn sysctlnametomib(sname: *const c_char, name: *mut c_int, namelenp: *mut size_t) -> c_int;
    #[link_name = "__kevent50"]
    pub fn kevent(
        kq: c_int,
        changelist: *const crate::kevent,
        nchanges: size_t,
        eventlist: *mut crate::kevent,
        nevents: size_t,
        timeout: *const crate::timespec,
    ) -> c_int;
    #[link_name = "__mount50"]
    pub fn mount(
        src: *const c_char,
        target: *const c_char,
        flags: c_int,
        data: *mut c_void,
        size: size_t,
    ) -> c_int;
    pub fn mq_open(name: *const c_char, oflag: c_int, ...) -> crate::mqd_t;
    pub fn mq_close(mqd: crate::mqd_t) -> c_int;
    pub fn mq_getattr(mqd: crate::mqd_t, attr: *mut crate::mq_attr) -> c_int;
    pub fn mq_notify(mqd: crate::mqd_t, notification: *const crate::sigevent) -> c_int;
    pub fn mq_receive(
        mqd: crate::mqd_t,
        msg_ptr: *mut c_char,
        msg_len: size_t,
        msg_prio: *mut c_uint,
    ) -> ssize_t;
    pub fn mq_send(
        mqd: crate::mqd_t,
        msg_ptr: *const c_char,
        msg_len: size_t,
        msg_prio: c_uint,
    ) -> c_int;
    pub fn mq_setattr(
        mqd: crate::mqd_t,
        newattr: *const crate::mq_attr,
        oldattr: *mut crate::mq_attr,
    ) -> c_int;
    #[link_name = "__mq_timedreceive50"]
    pub fn mq_timedreceive(
        mqd: crate::mqd_t,
        msg_ptr: *mut c_char,
        msg_len: size_t,
        msg_prio: *mut c_uint,
        abs_timeout: *const crate::timespec,
    ) -> ssize_t;
    #[link_name = "__mq_timedsend50"]
    pub fn mq_timedsend(
        mqd: crate::mqd_t,
        msg_ptr: *const c_char,
        msg_len: size_t,
        msg_prio: c_uint,
        abs_timeout: *const crate::timespec,
    ) -> c_int;
    pub fn mq_unlink(name: *const c_char) -> c_int;
    pub fn ptrace(request: c_int, pid: crate::pid_t, addr: *mut c_void, data: c_int) -> c_int;
    pub fn utrace(label: *const c_char, addr: *mut c_void, len: size_t) -> c_int;
    pub fn pthread_getname_np(t: crate::pthread_t, name: *mut c_char, len: size_t) -> c_int;
    pub fn pthread_setname_np(
        t: crate::pthread_t,
        name: *const c_char,
        arg: *const c_void,
    ) -> c_int;
    pub fn pthread_attr_get_np(thread: crate::pthread_t, attr: *mut crate::pthread_attr_t)
        -> c_int;
    pub fn pthread_getattr_np(native: crate::pthread_t, attr: *mut crate::pthread_attr_t) -> c_int;
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
    pub fn pthread_getaffinity_np(
        thread: crate::pthread_t,
        size: size_t,
        set: *mut cpuset_t,
    ) -> c_int;
    pub fn pthread_setaffinity_np(
        thread: crate::pthread_t,
        size: size_t,
        set: *mut cpuset_t,
    ) -> c_int;

    pub fn _cpuset_create() -> *mut cpuset_t;
    pub fn _cpuset_destroy(set: *mut cpuset_t);
    pub fn _cpuset_clr(cpu: cpuid_t, set: *mut cpuset_t) -> c_int;
    pub fn _cpuset_set(cpu: cpuid_t, set: *mut cpuset_t) -> c_int;
    pub fn _cpuset_isset(cpu: cpuid_t, set: *const cpuset_t) -> c_int;
    pub fn _cpuset_size(set: *const cpuset_t) -> size_t;
    pub fn _cpuset_zero(set: *mut cpuset_t);
    #[link_name = "__sigtimedwait50"]
    pub fn sigtimedwait(
        set: *const sigset_t,
        info: *mut siginfo_t,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn sigwaitinfo(set: *const sigset_t, info: *mut siginfo_t) -> c_int;

    pub fn duplocale(base: crate::locale_t) -> crate::locale_t;
    pub fn freelocale(loc: crate::locale_t);
    pub fn localeconv_l(loc: crate::locale_t) -> *mut lconv;
    pub fn newlocale(mask: c_int, locale: *const c_char, base: crate::locale_t) -> crate::locale_t;
    #[link_name = "__settimeofday50"]
    pub fn settimeofday(tv: *const crate::timeval, tz: *const c_void) -> c_int;

    pub fn dup3(src: c_int, dst: c_int, flags: c_int) -> c_int;

    pub fn kqueue1(flags: c_int) -> c_int;

    pub fn _lwp_self() -> lwpid_t;
    pub fn memmem(
        haystack: *const c_void,
        haystacklen: size_t,
        needle: *const c_void,
        needlelen: size_t,
    ) -> *mut c_void;

    // link.h

    pub fn dl_iterate_phdr(
        callback: Option<
            unsafe extern "C" fn(info: *mut dl_phdr_info, size: usize, data: *mut c_void) -> c_int,
        >,
        data: *mut c_void,
    ) -> c_int;

    // dlfcn.h

    pub fn _dlauxinfo() -> *mut c_void;

    pub fn iconv_open(tocode: *const c_char, fromcode: *const c_char) -> iconv_t;
    pub fn iconv(
        cd: iconv_t,
        inbuf: *mut *mut c_char,
        inbytesleft: *mut size_t,
        outbuf: *mut *mut c_char,
        outbytesleft: *mut size_t,
    ) -> size_t;
    pub fn iconv_close(cd: iconv_t) -> c_int;

    pub fn timer_create(
        clockid: crate::clockid_t,
        sevp: *mut crate::sigevent,
        timerid: *mut crate::timer_t,
    ) -> c_int;
    pub fn timer_delete(timerid: crate::timer_t) -> c_int;
    pub fn timer_getoverrun(timerid: crate::timer_t) -> c_int;
    pub fn timer_gettime(timerid: crate::timer_t, curr_value: *mut crate::itimerspec) -> c_int;
    pub fn timer_settime(
        timerid: crate::timer_t,
        flags: c_int,
        new_value: *const crate::itimerspec,
        old_value: *mut crate::itimerspec,
    ) -> c_int;

    // Added in `NetBSD` 7.0
    pub fn explicit_memset(b: *mut c_void, c: c_int, len: size_t);
    pub fn consttime_memequal(a: *const c_void, b: *const c_void, len: size_t) -> c_int;

    pub fn setproctitle(fmt: *const c_char, ...);
    pub fn mremap(
        oldp: *mut c_void,
        oldsize: size_t,
        newp: *mut c_void,
        newsize: size_t,
        flags: c_int,
    ) -> *mut c_void;

    pub fn sched_rr_get_interval(pid: crate::pid_t, t: *mut crate::timespec) -> c_int;
    pub fn sched_setparam(pid: crate::pid_t, param: *const crate::sched_param) -> c_int;
    pub fn sched_getparam(pid: crate::pid_t, param: *mut crate::sched_param) -> c_int;
    pub fn sched_getscheduler(pid: crate::pid_t) -> c_int;
    pub fn sched_setscheduler(
        pid: crate::pid_t,
        policy: c_int,
        param: *const crate::sched_param,
    ) -> c_int;

    #[link_name = "__pollts50"]
    pub fn pollts(
        fds: *mut crate::pollfd,
        nfds: crate::nfds_t,
        ts: *const crate::timespec,
        sigmask: *const crate::sigset_t,
    ) -> c_int;
    pub fn ppoll(
        fds: *mut crate::pollfd,
        nfds: crate::nfds_t,
        ts: *const crate::timespec,
        sigmask: *const crate::sigset_t,
    ) -> c_int;
    pub fn getrandom(buf: *mut c_void, buflen: size_t, flags: c_uint) -> ssize_t;

    pub fn reboot(mode: c_int, bootstr: *mut c_char) -> c_int;

    #[link_name = "___lwp_park60"]
    pub fn _lwp_park(
        clock: crate::clockid_t,
        flags: c_int,
        ts: *const crate::timespec,
        unpark: crate::lwpid_t,
        hint: *const c_void,
        unparkhint: *mut c_void,
    ) -> c_int;
    pub fn _lwp_unpark(lwp: crate::lwpid_t, hint: *const c_void) -> c_int;
    pub fn _lwp_unpark_all(
        targets: *const crate::lwpid_t,
        ntargets: size_t,
        hint: *const c_void,
    ) -> c_int;
    #[link_name = "__getmntinfo13"]
    pub fn getmntinfo(mntbufp: *mut *mut crate::statvfs, flags: c_int) -> c_int;
    pub fn getvfsstat(buf: *mut statvfs, bufsize: size_t, flags: c_int) -> c_int;

    // Added in `NetBSD` 10.0
    pub fn timerfd_create(clockid: crate::clockid_t, flags: c_int) -> c_int;
    pub fn timerfd_gettime(fd: c_int, curr_value: *mut itimerspec) -> c_int;
    pub fn timerfd_settime(
        fd: c_int,
        flags: c_int,
        new_value: *const itimerspec,
        old_value: *mut itimerspec,
    ) -> c_int;
}

#[link(name = "rt")]
extern "C" {
    pub fn aio_read(aiocbp: *mut aiocb) -> c_int;
    pub fn aio_write(aiocbp: *mut aiocb) -> c_int;
    pub fn aio_fsync(op: c_int, aiocbp: *mut aiocb) -> c_int;
    pub fn aio_error(aiocbp: *const aiocb) -> c_int;
    pub fn aio_return(aiocbp: *mut aiocb) -> ssize_t;
    #[link_name = "__aio_suspend50"]
    pub fn aio_suspend(
        aiocb_list: *const *const aiocb,
        nitems: c_int,
        timeout: *const crate::timespec,
    ) -> c_int;
    pub fn aio_cancel(fd: c_int, aiocbp: *mut aiocb) -> c_int;
    pub fn lio_listio(
        mode: c_int,
        aiocb_list: *const *mut aiocb,
        nitems: c_int,
        sevp: *mut sigevent,
    ) -> c_int;
}

#[link(name = "util")]
extern "C" {
    #[cfg_attr(target_os = "netbsd", link_name = "__getpwent_r50")]
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

    pub fn updwtmpx(file: *const c_char, ut: *const utmpx) -> c_int;
    pub fn getlastlogx(fname: *const c_char, uid: crate::uid_t, ll: *mut lastlogx)
        -> *mut lastlogx;
    pub fn updlastlogx(fname: *const c_char, uid: crate::uid_t, ll: *mut lastlogx) -> c_int;
    pub fn utmpxname(file: *const c_char) -> c_int;
    pub fn getutxent() -> *mut utmpx;
    pub fn getutxid(ut: *const utmpx) -> *mut utmpx;
    pub fn getutxline(ut: *const utmpx) -> *mut utmpx;
    pub fn pututxline(ut: *const utmpx) -> *mut utmpx;
    pub fn setutxent();
    pub fn endutxent();

    pub fn getutmp(ux: *const utmpx, u: *mut utmp);
    pub fn getutmpx(u: *const utmp, ux: *mut utmpx);

    pub fn utpname(file: *const c_char) -> c_int;
    pub fn setutent();
    pub fn endutent();
    pub fn getutent() -> *mut utmp;

    pub fn efopen(p: *const c_char, m: *const c_char) -> crate::FILE;
    pub fn emalloc(n: size_t) -> *mut c_void;
    pub fn ecalloc(n: size_t, c: size_t) -> *mut c_void;
    pub fn erealloc(p: *mut c_void, n: size_t) -> *mut c_void;
    pub fn ereallocarr(p: *mut c_void, n: size_t, s: size_t);
    pub fn estrdup(s: *const c_char) -> *mut c_char;
    pub fn estrndup(s: *const c_char, len: size_t) -> *mut c_char;
    pub fn estrlcpy(dst: *mut c_char, src: *const c_char, len: size_t) -> size_t;
    pub fn estrlcat(dst: *mut c_char, src: *const c_char, len: size_t) -> size_t;
    pub fn estrtoi(
        nptr: *const c_char,
        base: c_int,
        lo: crate::intmax_t,
        hi: crate::intmax_t,
    ) -> crate::intmax_t;
    pub fn estrtou(
        nptr: *const c_char,
        base: c_int,
        lo: crate::uintmax_t,
        hi: crate::uintmax_t,
    ) -> crate::uintmax_t;
    pub fn easprintf(string: *mut *mut c_char, fmt: *const c_char, ...) -> c_int;
    pub fn evasprintf(string: *mut *mut c_char, fmt: *const c_char, ...) -> c_int;
    pub fn esetfunc(
        cb: Option<unsafe extern "C" fn(c_int, *const c_char, ...)>,
    ) -> Option<unsafe extern "C" fn(c_int, *const c_char, ...)>;
    pub fn secure_path(path: *const c_char) -> c_int;
    pub fn snprintb(buf: *mut c_char, buflen: size_t, fmt: *const c_char, val: u64) -> c_int;
    pub fn snprintb_m(
        buf: *mut c_char,
        buflen: size_t,
        fmt: *const c_char,
        val: u64,
        max: size_t,
    ) -> c_int;

    pub fn getbootfile() -> *const c_char;
    pub fn getbyteorder() -> c_int;
    pub fn getdiskrawname(buf: *mut c_char, buflen: size_t, name: *const c_char) -> *const c_char;
    pub fn getdiskcookedname(
        buf: *mut c_char,
        buflen: size_t,
        name: *const c_char,
    ) -> *const c_char;
    pub fn getfsspecname(buf: *mut c_char, buflen: size_t, spec: *const c_char) -> *const c_char;

    pub fn strpct(
        buf: *mut c_char,
        bufsiz: size_t,
        numerator: crate::uintmax_t,
        denominator: crate::uintmax_t,
        precision: size_t,
    ) -> *mut c_char;
    pub fn strspct(
        buf: *mut c_char,
        bufsiz: size_t,
        numerator: crate::intmax_t,
        denominator: crate::intmax_t,
        precision: size_t,
    ) -> *mut c_char;
    #[link_name = "__login50"]
    pub fn login(ut: *const utmp);
    #[link_name = "__loginx50"]
    pub fn loginx(ut: *const utmpx);
    pub fn logout(line: *const c_char);
    pub fn logoutx(line: *const c_char, status: c_int, tpe: c_int);
    pub fn logwtmp(line: *const c_char, name: *const c_char, host: *const c_char);
    pub fn logwtmpx(
        line: *const c_char,
        name: *const c_char,
        host: *const c_char,
        status: c_int,
        tpe: c_int,
    );

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
    ) -> c_int;
    pub fn lsetxattr(
        path: *const c_char,
        name: *const c_char,
        value: *const c_void,
        size: size_t,
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
    pub fn fremovexattr(fd: c_int, path: *const c_char, name: *const c_char) -> c_int;

    pub fn string_to_flags(
        string_p: *mut *mut c_char,
        setp: *mut c_ulong,
        clrp: *mut c_ulong,
    ) -> c_int;
    pub fn flags_to_string(flags: c_ulong, def: *const c_char) -> c_int;

    pub fn kinfo_getvmmap(pid: crate::pid_t, cntp: *mut size_t) -> *mut kinfo_vmentry;
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
    pub fn backtrace_symbols_fd_fmt(
        addrlist: *const *mut c_void,
        len: size_t,
        fd: c_int,
        fmt: *const c_char,
    ) -> c_int;
}

cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(target_arch = "arm")] {
        mod arm;
        pub use self::arm::*;
    } else if #[cfg(target_arch = "powerpc")] {
        mod powerpc;
        pub use self::powerpc::*;
    } else if #[cfg(target_arch = "sparc64")] {
        mod sparc64;
        pub use self::sparc64::*;
    } else if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(target_arch = "x86")] {
        mod x86;
        pub use self::x86::*;
    } else if #[cfg(target_arch = "mips")] {
        mod mips;
        pub use self::mips::*;
    } else if #[cfg(target_arch = "riscv64")] {
        mod riscv64;
        pub use self::riscv64::*;
    } else {
        // Unknown target_arch
    }
}
