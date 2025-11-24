use crate::off_t;
use crate::prelude::*;

// APIs in FreeBSD 12 that have changed since 11.

pub type nlink_t = u64;
pub type dev_t = u64;
pub type ino_t = u64;
pub type shmatt_t = c_uint;

s! {
    pub struct shmid_ds {
        pub shm_perm: crate::ipc_perm,
        pub shm_segsz: size_t,
        pub shm_lpid: crate::pid_t,
        pub shm_cpid: crate::pid_t,
        pub shm_nattch: crate::shmatt_t,
        pub shm_atime: crate::time_t,
        pub shm_dtime: crate::time_t,
        pub shm_ctime: crate::time_t,
    }

    pub struct kevent {
        pub ident: crate::uintptr_t,
        pub filter: c_short,
        pub flags: c_ushort,
        pub fflags: c_uint,
        pub data: i64,
        pub udata: *mut c_void,
        pub ext: [u64; 4],
    }

    pub struct kvm_page {
        pub version: c_uint,
        pub paddr: c_ulong,
        pub kmap_vaddr: c_ulong,
        pub dmap_vaddr: c_ulong,
        pub prot: crate::vm_prot_t,
        pub offset: crate::u_long,
        pub len: size_t,
    }

    pub struct kinfo_proc {
        /// Size of this structure.
        pub ki_structsize: c_int,
        /// Reserved: layout identifier.
        pub ki_layout: c_int,
        /// Address of command arguments.
        pub ki_args: *mut crate::pargs,
        // This is normally "struct proc".
        /// Address of proc.
        pub ki_paddr: *mut c_void,
        // This is normally "struct user".
        /// Kernel virtual address of u-area.
        pub ki_addr: *mut c_void,
        // This is normally "struct vnode".
        /// Pointer to trace file.
        pub ki_tracep: *mut c_void,
        // This is normally "struct vnode".
        /// Pointer to executable file.
        pub ki_textvp: *mut c_void,
        /// Pointer to open file info.
        pub ki_fd: *mut crate::filedesc,
        // This is normally "struct vmspace".
        /// Pointer to kernel vmspace struct.
        pub ki_vmspace: *mut c_void,
        /// Sleep address.
        pub ki_wchan: *mut c_void,
        /// Process identifier.
        pub ki_pid: crate::pid_t,
        /// Parent process ID.
        pub ki_ppid: crate::pid_t,
        /// Process group ID.
        pub ki_pgid: crate::pid_t,
        /// tty process group ID.
        pub ki_tpgid: crate::pid_t,
        /// Process session ID.
        pub ki_sid: crate::pid_t,
        /// Terminal session ID.
        pub ki_tsid: crate::pid_t,
        /// Job control counter.
        pub ki_jobc: c_short,
        /// Unused (just here for alignment).
        pub ki_spare_short1: c_short,
        /// Controlling tty dev.
        pub ki_tdev_freebsd11: u32,
        /// Signals arrived but not delivered.
        pub ki_siglist: crate::sigset_t,
        /// Current signal mask.
        pub ki_sigmask: crate::sigset_t,
        /// Signals being ignored.
        pub ki_sigignore: crate::sigset_t,
        /// Signals being caught by user.
        pub ki_sigcatch: crate::sigset_t,
        /// Effective user ID.
        pub ki_uid: crate::uid_t,
        /// Real user ID.
        pub ki_ruid: crate::uid_t,
        /// Saved effective user ID.
        pub ki_svuid: crate::uid_t,
        /// Real group ID.
        pub ki_rgid: crate::gid_t,
        /// Saved effective group ID.
        pub ki_svgid: crate::gid_t,
        /// Number of groups.
        pub ki_ngroups: c_short,
        /// Unused (just here for alignment).
        pub ki_spare_short2: c_short,
        /// Groups.
        pub ki_groups: [crate::gid_t; crate::KI_NGROUPS],
        /// Virtual size.
        pub ki_size: crate::vm_size_t,
        /// Current resident set size in pages.
        pub ki_rssize: crate::segsz_t,
        /// Resident set size before last swap.
        pub ki_swrss: crate::segsz_t,
        /// Text size (pages) XXX.
        pub ki_tsize: crate::segsz_t,
        /// Data size (pages) XXX.
        pub ki_dsize: crate::segsz_t,
        /// Stack size (pages).
        pub ki_ssize: crate::segsz_t,
        /// Exit status for wait & stop signal.
        pub ki_xstat: crate::u_short,
        /// Accounting flags.
        pub ki_acflag: crate::u_short,
        /// %cpu for process during `ki_swtime`.
        pub ki_pctcpu: crate::fixpt_t,
        /// Time averaged value of `ki_cpticks`.
        pub ki_estcpu: crate::u_int,
        /// Time since last blocked.
        pub ki_slptime: crate::u_int,
        /// Time swapped in or out.
        pub ki_swtime: crate::u_int,
        /// Number of copy-on-write faults.
        pub ki_cow: crate::u_int,
        /// Real time in microsec.
        pub ki_runtime: u64,
        /// Starting time.
        pub ki_start: crate::timeval,
        /// Time used by process children.
        pub ki_childtime: crate::timeval,
        /// P_* flags.
        pub ki_flag: c_long,
        /// KI_* flags (below).
        pub ki_kiflag: c_long,
        /// Kernel trace points.
        pub ki_traceflag: c_int,
        /// S* process status.
        pub ki_stat: c_char,
        /// Process "nice" value.
        pub ki_nice: i8, // signed char
        /// Process lock (prevent swap) count.
        pub ki_lock: c_char,
        /// Run queue index.
        pub ki_rqindex: c_char,
        /// Which cpu we are on.
        pub ki_oncpu_old: c_uchar,
        /// Last cpu we were on.
        pub ki_lastcpu_old: c_uchar,
        /// Thread name.
        pub ki_tdname: [c_char; crate::TDNAMLEN + 1],
        /// Wchan message.
        pub ki_wmesg: [c_char; crate::WMESGLEN + 1],
        /// Setlogin name.
        pub ki_login: [c_char; crate::LOGNAMELEN + 1],
        /// Lock name.
        pub ki_lockname: [c_char; crate::LOCKNAMELEN + 1],
        /// Command name.
        pub ki_comm: [c_char; crate::COMMLEN + 1],
        /// Emulation name.
        pub ki_emul: [c_char; crate::KI_EMULNAMELEN + 1],
        /// Login class.
        pub ki_loginclass: [c_char; crate::LOGINCLASSLEN + 1],
        /// More thread name.
        pub ki_moretdname: [c_char; crate::MAXCOMLEN - crate::TDNAMLEN + 1],
        /// Spare string space.
        pub ki_sparestrings: [[c_char; 23]; 2], // little hack to allow PartialEq
        /// Spare room for growth.
        pub ki_spareints: [c_int; crate::KI_NSPARE_INT],
        /// Controlling tty dev.
        pub ki_tdev: crate::dev_t,
        /// Which cpu we are on.
        pub ki_oncpu: c_int,
        /// Last cpu we were on.
        pub ki_lastcpu: c_int,
        /// PID of tracing process.
        pub ki_tracer: c_int,
        /// P2_* flags.
        pub ki_flag2: c_int,
        /// Default FIB number.
        pub ki_fibnum: c_int,
        /// Credential flags.
        pub ki_cr_flags: crate::u_int,
        /// Process jail ID.
        pub ki_jid: c_int,
        /// Number of threads in total.
        pub ki_numthreads: c_int,
        /// Thread ID.
        pub ki_tid: crate::lwpid_t,
        /// Process priority.
        pub ki_pri: crate::priority,
        /// Process rusage statistics.
        pub ki_rusage: crate::rusage,
        /// rusage of children processes.
        pub ki_rusage_ch: crate::rusage,
        // This is normally "struct pcb".
        /// Kernel virtual addr of pcb.
        pub ki_pcb: *mut c_void,
        /// Kernel virtual addr of stack.
        pub ki_kstack: *mut c_void,
        /// User convenience pointer.
        pub ki_udata: *mut c_void,
        // This is normally "struct thread".
        pub ki_tdaddr: *mut c_void,
        pub ki_spareptrs: [*mut c_void; crate::KI_NSPARE_PTR],
        pub ki_sparelongs: [c_long; crate::KI_NSPARE_LONG],
        /// PS_* flags.
        pub ki_sflag: c_long,
        /// kthread flag.
        pub ki_tdflags: c_long,
    }

    pub struct stat {
        pub st_dev: crate::dev_t,
        pub st_ino: crate::ino_t,
        pub st_nlink: crate::nlink_t,
        pub st_mode: crate::mode_t,
        st_padding0: i16,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        st_padding1: i32,
        pub st_rdev: crate::dev_t,
        #[cfg(target_arch = "x86")]
        st_atim_ext: i32,
        pub st_atime: crate::time_t,
        pub st_atime_nsec: c_long,
        #[cfg(target_arch = "x86")]
        st_mtim_ext: i32,
        pub st_mtime: crate::time_t,
        pub st_mtime_nsec: c_long,
        #[cfg(target_arch = "x86")]
        st_ctim_ext: i32,
        pub st_ctime: crate::time_t,
        pub st_ctime_nsec: c_long,
        #[cfg(target_arch = "x86")]
        st_btim_ext: i32,
        pub st_birthtime: crate::time_t,
        pub st_birthtime_nsec: c_long,
        pub st_size: off_t,
        pub st_blocks: crate::blkcnt_t,
        pub st_blksize: crate::blksize_t,
        pub st_flags: crate::fflags_t,
        pub st_gen: u64,
        pub st_spare: [u64; 10],
    }
}

s_no_extra_traits! {
    pub struct dirent {
        pub d_fileno: crate::ino_t,
        pub d_off: off_t,
        pub d_reclen: u16,
        pub d_type: u8,
        d_pad0: u8,
        pub d_namlen: u16,
        d_pad1: u16,
        pub d_name: [c_char; 256],
    }

    pub struct statfs {
        pub f_version: u32,
        pub f_type: u32,
        pub f_flags: u64,
        pub f_bsize: u64,
        pub f_iosize: u64,
        pub f_blocks: u64,
        pub f_bfree: u64,
        pub f_bavail: i64,
        pub f_files: u64,
        pub f_ffree: i64,
        pub f_syncwrites: u64,
        pub f_asyncwrites: u64,
        pub f_syncreads: u64,
        pub f_asyncreads: u64,
        f_spare: [u64; 10],
        pub f_namemax: u32,
        pub f_owner: crate::uid_t,
        pub f_fsid: crate::fsid_t,
        f_charspare: [c_char; 80],
        pub f_fstypename: [c_char; 16],
        pub f_mntfromname: [c_char; 1024],
        pub f_mntonname: [c_char; 1024],
    }

    pub struct vnstat {
        pub vn_fileid: u64,
        pub vn_size: u64,
        pub vn_dev: u64,
        pub vn_fsid: u64,
        pub vn_mntdir: *mut c_char,
        pub vn_type: c_int,
        pub vn_mode: u16,
        pub vn_devname: [c_char; crate::SPECNAMELEN as usize + 1],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for statfs {
            fn eq(&self, other: &statfs) -> bool {
                self.f_version == other.f_version
                    && self.f_type == other.f_type
                    && self.f_flags == other.f_flags
                    && self.f_bsize == other.f_bsize
                    && self.f_iosize == other.f_iosize
                    && self.f_blocks == other.f_blocks
                    && self.f_bfree == other.f_bfree
                    && self.f_bavail == other.f_bavail
                    && self.f_files == other.f_files
                    && self.f_ffree == other.f_ffree
                    && self.f_syncwrites == other.f_syncwrites
                    && self.f_asyncwrites == other.f_asyncwrites
                    && self.f_syncreads == other.f_syncreads
                    && self.f_asyncreads == other.f_asyncreads
                    && self.f_namemax == other.f_namemax
                    && self.f_owner == other.f_owner
                    && self.f_fsid == other.f_fsid
                    && self.f_fstypename == other.f_fstypename
                    && self
                        .f_mntfromname
                        .iter()
                        .zip(other.f_mntfromname.iter())
                        .all(|(a, b)| a == b)
                    && self
                        .f_mntonname
                        .iter()
                        .zip(other.f_mntonname.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for statfs {}
        impl hash::Hash for statfs {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.f_version.hash(state);
                self.f_type.hash(state);
                self.f_flags.hash(state);
                self.f_bsize.hash(state);
                self.f_iosize.hash(state);
                self.f_blocks.hash(state);
                self.f_bfree.hash(state);
                self.f_bavail.hash(state);
                self.f_files.hash(state);
                self.f_ffree.hash(state);
                self.f_syncwrites.hash(state);
                self.f_asyncwrites.hash(state);
                self.f_syncreads.hash(state);
                self.f_asyncreads.hash(state);
                self.f_namemax.hash(state);
                self.f_owner.hash(state);
                self.f_fsid.hash(state);
                self.f_charspare.hash(state);
                self.f_fstypename.hash(state);
                self.f_mntfromname.hash(state);
                self.f_mntonname.hash(state);
            }
        }

        impl PartialEq for dirent {
            fn eq(&self, other: &dirent) -> bool {
                self.d_fileno == other.d_fileno
                    && self.d_off == other.d_off
                    && self.d_reclen == other.d_reclen
                    && self.d_type == other.d_type
                    && self.d_namlen == other.d_namlen
                    && self.d_name[..self.d_namlen as _]
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
                self.d_name[..self.d_namlen as _].hash(state);
            }
        }

        impl PartialEq for vnstat {
            fn eq(&self, other: &vnstat) -> bool {
                let self_vn_devname: &[c_char] = &self.vn_devname;
                let other_vn_devname: &[c_char] = &other.vn_devname;

                self.vn_fileid == other.vn_fileid
                    && self.vn_size == other.vn_size
                    && self.vn_dev == other.vn_dev
                    && self.vn_fsid == other.vn_fsid
                    && self.vn_mntdir == other.vn_mntdir
                    && self.vn_type == other.vn_type
                    && self.vn_mode == other.vn_mode
                    && self_vn_devname == other_vn_devname
            }
        }
        impl Eq for vnstat {}
        impl hash::Hash for vnstat {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                let self_vn_devname: &[c_char] = &self.vn_devname;

                self.vn_fileid.hash(state);
                self.vn_size.hash(state);
                self.vn_dev.hash(state);
                self.vn_fsid.hash(state);
                self.vn_mntdir.hash(state);
                self.vn_type.hash(state);
                self.vn_mode.hash(state);
                self_vn_devname.hash(state);
            }
        }
    }
}

pub const RAND_MAX: c_int = 0x7fff_fffd;
pub const ELAST: c_int = 97;

/// max length of devicename
pub const SPECNAMELEN: c_int = 63;
pub const KI_NSPARE_PTR: usize = 6;

pub const MINCORE_SUPER: c_int = 0x20;

safe_f! {
    pub {const} fn makedev(major: c_uint, minor: c_uint) -> crate::dev_t {
        let major = major as crate::dev_t;
        let minor = minor as crate::dev_t;
        let mut dev = 0;
        dev |= ((major & 0xffffff00) as dev_t) << 32;
        dev |= ((major & 0x000000ff) as dev_t) << 8;
        dev |= ((minor & 0x0000ff00) as dev_t) << 24;
        dev |= ((minor & 0xffff00ff) as dev_t) << 0;
        dev
    }

    pub {const} fn major(dev: crate::dev_t) -> c_int {
        (((dev >> 32) & 0xffffff00) | ((dev >> 8) & 0xff)) as c_int
    }

    pub {const} fn minor(dev: crate::dev_t) -> c_int {
        (((dev >> 24) & 0xff00) | (dev & 0xffff00ff)) as c_int
    }
}

extern "C" {
    pub fn setgrent();
    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;
    pub fn freelocale(loc: crate::locale_t);
    pub fn msgrcv(
        msqid: c_int,
        msgp: *mut c_void,
        msgsz: size_t,
        msgtyp: c_long,
        msgflg: c_int,
    ) -> ssize_t;

    pub fn dirname(path: *mut c_char) -> *mut c_char;
    pub fn basename(path: *mut c_char) -> *mut c_char;
}

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    }
}
