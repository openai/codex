use crate::prelude::*;

pub type wchar_t = u32;
pub type stat64 = crate::stat;

s! {
    pub struct stat {
        pub st_dev: crate::dev_t,
        pub st_ino: c_ulonglong,
        pub st_mode: c_uint,
        pub st_nlink: c_uint,
        pub st_uid: c_uint,
        pub st_gid: c_uint,
        pub st_rdev: c_ulonglong,
        __st_rdev_padding: c_ulong,
        pub st_size: c_longlong,
        pub st_blksize: crate::blksize_t,
        __st_blksize_padding: c_int,
        pub st_blocks: crate::blkcnt_t,
        pub st_atime: crate::time_t,
        pub st_atime_nsec: c_long,
        pub st_mtime: crate::time_t,
        pub st_mtime_nsec: c_long,
        pub st_ctime: crate::time_t,
        pub st_ctime_nsec: c_long,

        __unused: [c_int; 2],
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_flags: c_int,
        pub ss_size: size_t,
    }

    pub struct ipc_perm {
        #[cfg(musl_v1_2_3)]
        pub __key: crate::key_t,
        #[cfg(not(musl_v1_2_3))]
        #[deprecated(
            since = "0.2.173",
            note = "This field is incorrectly named and will be changed
                to __key in a future release"
        )]
        pub __ipc_perm_key: crate::key_t,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub cuid: crate::uid_t,
        pub cgid: crate::gid_t,
        pub mode: crate::mode_t,
        pub __seq: c_ushort,
    }

    pub struct shmid_ds {
        pub shm_perm: crate::ipc_perm,
        pub shm_segsz: size_t,
        pub shm_atime: crate::time_t,
        __unused1: c_int,
        pub shm_dtime: crate::time_t,
        __unused2: c_int,
        pub shm_ctime: crate::time_t,
        __unused3: c_int,
        pub shm_cpid: crate::pid_t,
        pub shm_lpid: crate::pid_t,
        pub shm_nattch: c_ulong,
        __pad1: c_ulong,
        __pad2: c_ulong,
    }

    pub struct msqid_ds {
        pub msg_perm: crate::ipc_perm,
        pub msg_stime: crate::time_t,
        __unused1: c_int,
        pub msg_rtime: crate::time_t,
        __unused2: c_int,
        pub msg_ctime: crate::time_t,
        __unused3: c_int,
        pub __msg_cbytes: c_ulong,
        pub msg_qnum: crate::msgqnum_t,
        pub msg_qbytes: crate::msglen_t,
        pub msg_lspid: crate::pid_t,
        pub msg_lrpid: crate::pid_t,
        __pad1: c_ulong,
        __pad2: c_ulong,
    }
}

pub const AF_FILE: c_int = 1;
pub const AF_KCM: c_int = 41;
pub const AF_MAX: c_int = 43;
pub const AF_QIPCRTR: c_int = 42;
pub const EADDRINUSE: c_int = 98;
pub const EADDRNOTAVAIL: c_int = 99;
pub const EAFNOSUPPORT: c_int = 97;
pub const EALREADY: c_int = 114;
pub const EBADE: c_int = 52;
pub const EBADMSG: c_int = 74;
pub const EBADR: c_int = 53;
pub const EBADRQC: c_int = 56;
pub const EBADSLT: c_int = 57;
pub const ECANCELED: c_int = 125;
pub const ECHRNG: c_int = 44;
pub const ECONNABORTED: c_int = 103;
pub const ECONNREFUSED: c_int = 111;
pub const ECONNRESET: c_int = 104;
pub const EDEADLK: c_int = 35;
pub const EDEADLOCK: c_int = 35;
pub const EDESTADDRREQ: c_int = 89;
pub const EDQUOT: c_int = 122;
pub const EHOSTDOWN: c_int = 112;
pub const EHOSTUNREACH: c_int = 113;
pub const EHWPOISON: c_int = 133;
pub const EIDRM: c_int = 43;
pub const EILSEQ: c_int = 84;
pub const EINPROGRESS: c_int = 115;
pub const EISCONN: c_int = 106;
pub const EISNAM: c_int = 120;
pub const EKEYEXPIRED: c_int = 127;
pub const EKEYREJECTED: c_int = 129;
pub const EKEYREVOKED: c_int = 128;
pub const EL2HLT: c_int = 51;
pub const EL2NSYNC: c_int = 45;
pub const EL3HLT: c_int = 46;
pub const EL3RST: c_int = 47;
pub const ELIBACC: c_int = 79;
pub const ELIBBAD: c_int = 80;
pub const ELIBEXEC: c_int = 83;
pub const ELIBMAX: c_int = 82;
pub const ELIBSCN: c_int = 81;
pub const ELNRNG: c_int = 48;
pub const ELOOP: c_int = 40;
pub const EMEDIUMTYPE: c_int = 124;
pub const EMSGSIZE: c_int = 90;
pub const EMULTIHOP: c_int = 72;
pub const ENAMETOOLONG: c_int = 36;
pub const ENAVAIL: c_int = 119;
pub const ENETDOWN: c_int = 100;
pub const ENETRESET: c_int = 102;
pub const ENETUNREACH: c_int = 101;
pub const ENOANO: c_int = 55;
pub const ENOBUFS: c_int = 105;
pub const ENOCSI: c_int = 50;
pub const ENOKEY: c_int = 126;
pub const ENOLCK: c_int = 37;
pub const ENOMEDIUM: c_int = 123;
pub const ENOMSG: c_int = 42;
pub const ENOPROTOOPT: c_int = 92;
pub const ENOSYS: c_int = 38;
pub const ENOTCONN: c_int = 107;
pub const ENOTEMPTY: c_int = 39;
pub const ENOTNAM: c_int = 118;
pub const ENOTRECOVERABLE: c_int = 131;
pub const ENOTSOCK: c_int = 88;
pub const ENOTSUP: c_int = 95;
pub const ENOTUNIQ: c_int = 76;
pub const EOPNOTSUPP: c_int = 95;
pub const EOVERFLOW: c_int = 75;
pub const EOWNERDEAD: c_int = 130;
pub const EPFNOSUPPORT: c_int = 96;
pub const EREMCHG: c_int = 78;
pub const ERESTART: c_int = 85;
pub const ERFKILL: c_int = 132;
pub const ESHUTDOWN: c_int = 108;
pub const ESOCKTNOSUPPORT: c_int = 94;
pub const ESTALE: c_int = 116;
pub const ESTRPIPE: c_int = 86;
pub const ETOOMANYREFS: c_int = 109;
pub const ETIMEDOUT: c_int = 110;
pub const EUCLEAN: c_int = 117;
pub const EUNATCH: c_int = 49;
pub const EUSERS: c_int = 87;
pub const EXFULL: c_int = 54;
pub const EXTPROC: c_int = 65536;
pub const F_EXLCK: c_int = 4;
pub const F_GETLK: c_int = 12;
pub const F_GETOWN: c_int = 9;
pub const F_GETOWNER_UIDS: c_int = 17;
pub const F_GETOWN_EX: c_int = 16;
pub const F_GETSIG: c_int = 11;
pub const F_LINUX_SPECIFIC_BASE: c_int = 1024;
pub const FLUSHO: c_int = 4096;
pub const F_OWNER_PGRP: c_int = 2;
pub const F_OWNER_PID: c_int = 1;
pub const F_OWNER_TID: c_int = 0;
pub const F_SETLK: c_int = 13;
pub const F_SETLKW: c_int = 14;
pub const F_SETOWN: c_int = 8;
pub const F_SETOWN_EX: c_int = 15;
pub const F_SETSIG: c_int = 10;
pub const F_SHLCK: c_int = 8;
pub const IEXTEN: c_int = 32768;
pub const MAP_ANON: c_int = 32;
pub const MAP_DENYWRITE: c_int = 2048;
pub const MAP_EXECUTABLE: c_int = 4096;
pub const MAP_GROWSDOWN: c_int = 256;
pub const MAP_HUGETLB: c_int = 262144;
pub const MAP_LOCKED: c_int = 8192;
pub const MAP_NONBLOCK: c_int = 65536;
pub const MAP_NORESERVE: c_int = 16384;
pub const MAP_POPULATE: c_int = 32768;
pub const MAP_STACK: c_int = 131072;
pub const MAP_UNINITIALIZED: c_int = 0;
pub const O_APPEND: c_int = 1024;
pub const O_ASYNC: c_int = 8192;
pub const O_CREAT: c_int = 64;
pub const O_DIRECT: c_int = 16384;
pub const O_DIRECTORY: c_int = 65536;
pub const O_DSYNC: c_int = 4096;
pub const O_EXCL: c_int = 128;
pub const O_LARGEFILE: c_int = 32768;
pub const O_NOCTTY: c_int = 256;
pub const O_NOFOLLOW: c_int = 131072;
pub const O_NONBLOCK: c_int = 2048;
pub const O_SYNC: c_int = 1052672;
pub const PF_FILE: c_int = 1;
pub const PF_KCM: c_int = 41;
pub const PF_MAX: c_int = 43;
pub const PF_QIPCRTR: c_int = 42;
pub const SA_ONSTACK: c_int = 0x08000000;
pub const SA_SIGINFO: c_int = 0x00000004;
pub const SA_NOCLDWAIT: c_int = 0x00000002;
pub const SIGBUS: c_int = 7;
pub const SIGCHLD: c_int = 17;
pub const SIGCONT: c_int = 18;
pub const SIGIO: c_int = 29;
pub const SIGPOLL: c_int = 29;
pub const SIGPROF: c_int = 27;
pub const SIGPWR: c_int = 30;
pub const SIGSTKFLT: c_int = 16;
pub const SIGSTKSZ: size_t = 8192;
pub const MINSIGSTKSZ: size_t = 2048;
pub const SIGSTOP: c_int = 19;
pub const SIGSYS: c_int = 31;
pub const SIGTSTP: c_int = 20;
pub const SIGTTIN: c_int = 21;
pub const SIGTTOU: c_int = 22;
pub const SIGURG: c_int = 23;
pub const SIGUSR1: c_int = 10;
pub const SIGUSR2: c_int = 12;
pub const SIGVTALRM: c_int = 26;
pub const SIGWINCH: c_int = 28;
pub const SIGXCPU: c_int = 24;
pub const SIGXFSZ: c_int = 25;
pub const SIG_SETMASK: c_int = 2; // FIXME(musl) check these
pub const SIG_BLOCK: c_int = 0x000000;
pub const SIG_UNBLOCK: c_int = 0x01;
pub const SOCK_DGRAM: c_int = 2;
pub const SOCK_STREAM: c_int = 1;
pub const SOL_CAIF: c_int = 278;
pub const SOL_IUCV: c_int = 277;
pub const SOL_KCM: c_int = 281;
pub const SOL_NFC: c_int = 280;
pub const SOL_PNPIPE: c_int = 275;
pub const SOL_PPPOL2TP: c_int = 273;
pub const SOL_RDS: c_int = 276;
pub const SOL_RXRPC: c_int = 272;

pub const SYS3264_fadvise64: c_int = 223;
pub const SYS3264_fcntl: c_int = 25;
pub const SYS3264_fstatat: c_int = 79;
pub const SYS3264_fstat: c_int = 80;
pub const SYS3264_fstatfs: c_int = 44;
pub const SYS3264_ftruncate: c_int = 46;
pub const SYS3264_lseek: c_int = 62;
pub const SYS3264_lstat: c_int = 1039;
pub const SYS3264_mmap: c_int = 222;
pub const SYS3264_sendfile: c_int = 71;
pub const SYS3264_stat: c_int = 1038;
pub const SYS3264_statfs: c_int = 43;
pub const SYS3264_truncate: c_int = 45;
pub const SYS_accept4: c_int = 242;
pub const SYS_accept: c_int = 202;
pub const SYS_access: c_int = 1033;
pub const SYS_acct: c_int = 89;
pub const SYS_add_key: c_int = 217;
pub const SYS_adjtimex: c_int = 171;
pub const SYS_alarm: c_int = 1059;
pub const SYS_arch_specific_syscall: c_int = 244;
pub const SYS_bdflush: c_int = 1075;
pub const SYS_bind: c_int = 200;
pub const SYS_bpf: c_int = 280;
pub const SYS_brk: c_int = 214;
pub const SYS_capget: c_int = 90;
pub const SYS_capset: c_int = 91;
pub const SYS_chdir: c_int = 49;
pub const SYS_chmod: c_int = 1028;
pub const SYS_chown: c_int = 1029;
pub const SYS_chroot: c_int = 51;
pub const SYS_clock_adjtime: c_int = 266;
pub const SYS_clock_getres: c_int = 114;
pub const SYS_clock_gettime: c_int = 113;
pub const SYS_clock_nanosleep: c_int = 115;
pub const SYS_clock_settime: c_int = 112;
pub const SYS_clone: c_int = 220;
pub const SYS_close: c_int = 57;
pub const SYS_connect: c_int = 203;
pub const SYS_copy_file_range: c_int = -1; // FIXME(hexagon)
pub const SYS_creat: c_int = 1064;
pub const SYS_delete_module: c_int = 106;
pub const SYS_dup2: c_int = 1041;
pub const SYS_dup3: c_int = 24;
pub const SYS_dup: c_int = 23;
pub const SYS_epoll_create1: c_int = 20;
pub const SYS_epoll_create: c_int = 1042;
pub const SYS_epoll_ctl: c_int = 21;
pub const SYS_epoll_pwait: c_int = 22;
pub const SYS_epoll_wait: c_int = 1069;
pub const SYS_eventfd2: c_int = 19;
pub const SYS_eventfd: c_int = 1044;
pub const SYS_execveat: c_int = 281;
pub const SYS_execve: c_int = 221;
pub const SYS_exit: c_int = 93;
pub const SYS_exit_group: c_int = 94;
pub const SYS_faccessat: c_int = 48;
pub const SYS_fadvise64_64: c_int = 223;
pub const SYS_fallocate: c_int = 47;
pub const SYS_fanotify_init: c_int = 262;
pub const SYS_fanotify_mark: c_int = 263;
pub const SYS_fchdir: c_int = 50;
pub const SYS_fchmodat: c_int = 53;
pub const SYS_fchmod: c_int = 52;
pub const SYS_fchownat: c_int = 54;
pub const SYS_fchown: c_int = 55;
pub const SYS_fcntl64: c_int = 25;
pub const SYS_fcntl: c_int = 25;
pub const SYS_fdatasync: c_int = 83;
pub const SYS_fgetxattr: c_int = 10;
pub const SYS_finit_module: c_int = 273;
pub const SYS_flistxattr: c_int = 13;
pub const SYS_flock: c_int = 32;
pub const SYS_fork: c_int = 1079;
pub const SYS_fremovexattr: c_int = 16;
pub const SYS_fsetxattr: c_int = 7;
pub const SYS_fstat64: c_int = 80;
pub const SYS_fstatat64: c_int = 79;
pub const SYS_fstatfs64: c_int = 44;
pub const SYS_fstatfs: c_int = 44;
pub const SYS_fsync: c_int = 82;
pub const SYS_ftruncate64: c_int = 46;
pub const SYS_ftruncate: c_int = 46;
pub const SYS_futex: c_int = 98;
pub const SYS_futimesat: c_int = 1066;
pub const SYS_getcpu: c_int = 168;
pub const SYS_getcwd: c_int = 17;
pub const SYS_getdents64: c_int = 61;
pub const SYS_getdents: c_int = 1065;
pub const SYS_getegid: c_int = 177;
pub const SYS_geteuid: c_int = 175;
pub const SYS_getgid: c_int = 176;
pub const SYS_getgroups: c_int = 158;
pub const SYS_getitimer: c_int = 102;
pub const SYS_get_mempolicy: c_int = 236;
pub const SYS_getpeername: c_int = 205;
pub const SYS_getpgid: c_int = 155;
pub const SYS_getpgrp: c_int = 1060;
pub const SYS_getpid: c_int = 172;
pub const SYS_getppid: c_int = 173;
pub const SYS_getpriority: c_int = 141;
pub const SYS_getrandom: c_int = 278;
pub const SYS_getresgid: c_int = 150;
pub const SYS_getresuid: c_int = 148;
pub const SYS_getrlimit: c_int = 163;
pub const SYS_get_robust_list: c_int = 100;
pub const SYS_getrusage: c_int = 165;
pub const SYS_getsid: c_int = 156;
pub const SYS_getsockname: c_int = 204;
pub const SYS_getsockopt: c_int = 209;
pub const SYS_gettid: c_int = 178;
pub const SYS_gettimeofday: c_int = 169;
pub const SYS_getuid: c_int = 174;
pub const SYS_getxattr: c_int = 8;
pub const SYS_init_module: c_int = 105;
pub const SYS_inotify_add_watch: c_int = 27;
pub const SYS_inotify_init1: c_int = 26;
pub const SYS_inotify_init: c_int = 1043;
pub const SYS_inotify_rm_watch: c_int = 28;
pub const SYS_io_cancel: c_int = 3;
pub const SYS_ioctl: c_int = 29;
pub const SYS_io_destroy: c_int = 1;
pub const SYS_io_getevents: c_int = 4;
pub const SYS_ioprio_get: c_int = 31;
pub const SYS_ioprio_set: c_int = 30;
pub const SYS_io_setup: c_int = 0;
pub const SYS_io_submit: c_int = 2;
pub const SYS_kcmp: c_int = 272;
pub const SYS_kexec_load: c_int = 104;
pub const SYS_keyctl: c_int = 219;
pub const SYS_kill: c_int = 129;
pub const SYS_lchown: c_int = 1032;
pub const SYS_lgetxattr: c_int = 9;
pub const SYS_linkat: c_int = 37;
pub const SYS_link: c_int = 1025;
pub const SYS_listen: c_int = 201;
pub const SYS_listxattr: c_int = 11;
pub const SYS_llistxattr: c_int = 12;
pub const SYS__llseek: c_int = 62;
pub const SYS_lookup_dcookie: c_int = 18;
pub const SYS_lremovexattr: c_int = 15;
pub const SYS_lseek: c_int = 62;
pub const SYS_lsetxattr: c_int = 6;
pub const SYS_lstat64: c_int = 1039;
pub const SYS_lstat: c_int = 1039;
pub const SYS_madvise: c_int = 233;
pub const SYS_mbind: c_int = 235;
pub const SYS_memfd_create: c_int = 279;
pub const SYS_migrate_pages: c_int = 238;
pub const SYS_mincore: c_int = 232;
pub const SYS_mkdirat: c_int = 34;
pub const SYS_mkdir: c_int = 1030;
pub const SYS_mknodat: c_int = 33;
pub const SYS_mknod: c_int = 1027;
pub const SYS_mlockall: c_int = 230;
pub const SYS_mlock: c_int = 228;
pub const SYS_mmap2: c_int = 222;
pub const SYS_mount: c_int = 40;
pub const SYS_move_pages: c_int = 239;
pub const SYS_mprotect: c_int = 226;
pub const SYS_mq_getsetattr: c_int = 185;
pub const SYS_mq_notify: c_int = 184;
pub const SYS_mq_open: c_int = 180;
pub const SYS_mq_timedreceive: c_int = 183;
pub const SYS_mq_timedsend: c_int = 182;
pub const SYS_mq_unlink: c_int = 181;
pub const SYS_mremap: c_int = 216;
pub const SYS_msgctl: c_int = 187;
pub const SYS_msgget: c_int = 186;
pub const SYS_msgrcv: c_int = 188;
pub const SYS_msgsnd: c_int = 189;
pub const SYS_msync: c_int = 227;
pub const SYS_munlockall: c_int = 231;
pub const SYS_munlock: c_int = 229;
pub const SYS_munmap: c_int = 215;
pub const SYS_name_to_handle_at: c_int = 264;
pub const SYS_nanosleep: c_int = 101;
pub const SYS_newfstatat: c_int = 79;
pub const SYS_nfsservctl: c_int = 42;
pub const SYS_oldwait4: c_int = 1072;
pub const SYS_openat: c_int = 56;
pub const SYS_open_by_handle_at: c_int = 265;
pub const SYS_open: c_int = 1024;
pub const SYS_pause: c_int = 1061;
pub const SYS_perf_event_open: c_int = 241;
pub const SYS_personality: c_int = 92;
pub const SYS_pipe2: c_int = 59;
pub const SYS_pipe: c_int = 1040;
pub const SYS_pivot_root: c_int = 41;
pub const SYS_poll: c_int = 1068;
pub const SYS_ppoll: c_int = 73;
pub const SYS_prctl: c_int = 167;
pub const SYS_pread64: c_int = 67;
pub const SYS_preadv: c_int = 69;
pub const SYS_prlimit64: c_int = 261;
pub const SYS_process_vm_readv: c_int = 270;
pub const SYS_process_vm_writev: c_int = 271;
pub const SYS_pselect6: c_int = 72;
pub const SYS_ptrace: c_int = 117;
pub const SYS_pwrite64: c_int = 68;
pub const SYS_pwritev: c_int = 70;
pub const SYS_quotactl: c_int = 60;
pub const SYS_readahead: c_int = 213;
pub const SYS_read: c_int = 63;
pub const SYS_readlinkat: c_int = 78;
pub const SYS_readlink: c_int = 1035;
pub const SYS_readv: c_int = 65;
pub const SYS_reboot: c_int = 142;
pub const SYS_recv: c_int = 1073;
pub const SYS_recvfrom: c_int = 207;
pub const SYS_recvmmsg: c_int = 243;
pub const SYS_recvmsg: c_int = 212;
pub const SYS_remap_file_pages: c_int = 234;
pub const SYS_removexattr: c_int = 14;
pub const SYS_renameat2: c_int = 276;
pub const SYS_renameat: c_int = 38;
pub const SYS_rename: c_int = 1034;
pub const SYS_request_key: c_int = 218;
pub const SYS_restart_syscall: c_int = 128;
pub const SYS_rmdir: c_int = 1031;
pub const SYS_rt_sigaction: c_int = 134;
pub const SYS_rt_sigpending: c_int = 136;
pub const SYS_rt_sigprocmask: c_int = 135;
pub const SYS_rt_sigqueueinfo: c_int = 138;
pub const SYS_rt_sigreturn: c_int = 139;
pub const SYS_rt_sigsuspend: c_int = 133;
pub const SYS_rt_sigtimedwait: c_int = 137;
pub const SYS_rt_tgsigqueueinfo: c_int = 240;
pub const SYS_sched_getaffinity: c_int = 123;
pub const SYS_sched_getattr: c_int = 275;
pub const SYS_sched_getparam: c_int = 121;
pub const SYS_sched_get_priority_max: c_int = 125;
pub const SYS_sched_get_priority_min: c_int = 126;
pub const SYS_sched_getscheduler: c_int = 120;
pub const SYS_sched_rr_get_interval: c_int = 127;
pub const SYS_sched_setaffinity: c_int = 122;
pub const SYS_sched_setattr: c_int = 274;
pub const SYS_sched_setparam: c_int = 118;
pub const SYS_sched_setscheduler: c_int = 119;
pub const SYS_sched_yield: c_int = 124;
pub const SYS_seccomp: c_int = 277;
pub const SYS_select: c_int = 1067;
pub const SYS_semctl: c_int = 191;
pub const SYS_semget: c_int = 190;
pub const SYS_semop: c_int = 193;
pub const SYS_semtimedop: c_int = 192;
pub const SYS_send: c_int = 1074;
pub const SYS_sendfile64: c_int = 71;
pub const SYS_sendfile: c_int = 71;
pub const SYS_sendmmsg: c_int = 269;
pub const SYS_sendmsg: c_int = 211;
pub const SYS_sendto: c_int = 206;
pub const SYS_setdomainname: c_int = 162;
pub const SYS_setfsgid: c_int = 152;
pub const SYS_setfsuid: c_int = 151;
pub const SYS_setgid: c_int = 144;
pub const SYS_setgroups: c_int = 159;
pub const SYS_sethostname: c_int = 161;
pub const SYS_setitimer: c_int = 103;
pub const SYS_set_mempolicy: c_int = 237;
pub const SYS_setns: c_int = 268;
pub const SYS_setpgid: c_int = 154;
pub const SYS_setpriority: c_int = 140;
pub const SYS_setregid: c_int = 143;
pub const SYS_setresgid: c_int = 149;
pub const SYS_setresuid: c_int = 147;
pub const SYS_setreuid: c_int = 145;
pub const SYS_setrlimit: c_int = 164;
pub const SYS_set_robust_list: c_int = 99;
pub const SYS_setsid: c_int = 157;
pub const SYS_setsockopt: c_int = 208;
pub const SYS_set_tid_address: c_int = 96;
pub const SYS_settimeofday: c_int = 170;
pub const SYS_setuid: c_int = 146;
pub const SYS_setxattr: c_int = 5;
pub const SYS_shmat: c_int = 196;
pub const SYS_shmctl: c_int = 195;
pub const SYS_shmdt: c_int = 197;
pub const SYS_shmget: c_int = 194;
pub const SYS_shutdown: c_int = 210;
pub const SYS_sigaltstack: c_int = 132;
pub const SYS_signalfd4: c_int = 74;
pub const SYS_signalfd: c_int = 1045;
pub const SYS_socket: c_int = 198;
pub const SYS_socketpair: c_int = 199;
pub const SYS_splice: c_int = 76;
pub const SYS_stat64: c_int = 1038;
pub const SYS_stat: c_int = 1038;
pub const SYS_statfs64: c_int = 43;
pub const SYS_swapoff: c_int = 225;
pub const SYS_swapon: c_int = 224;
pub const SYS_symlinkat: c_int = 36;
pub const SYS_symlink: c_int = 1036;
pub const SYS_sync: c_int = 81;
pub const SYS_sync_file_range2: c_int = 84;
pub const SYS_sync_file_range: c_int = 84;
pub const SYS_syncfs: c_int = 267;
pub const SYS_syscalls: c_int = 1080;
pub const SYS__sysctl: c_int = 1078;
pub const SYS_sysinfo: c_int = 179;
pub const SYS_syslog: c_int = 116;
pub const SYS_tee: c_int = 77;
pub const SYS_tgkill: c_int = 131;
pub const SYS_time: c_int = 1062;
pub const SYS_timer_create: c_int = 107;
pub const SYS_timer_delete: c_int = 111;
pub const SYS_timerfd_create: c_int = 85;
pub const SYS_timerfd_gettime: c_int = 87;
pub const SYS_timerfd_settime: c_int = 86;
pub const SYS_timer_getoverrun: c_int = 109;
pub const SYS_timer_gettime: c_int = 108;
pub const SYS_timer_settime: c_int = 110;
pub const SYS_times: c_int = 153;
pub const SYS_tkill: c_int = 130;
pub const SYS_truncate64: c_int = 45;
pub const SYS_truncate: c_int = 45;
pub const SYS_umask: c_int = 166;
pub const SYS_umount2: c_int = 39;
pub const SYS_umount: c_int = 1076;
pub const SYS_uname: c_int = 160;
pub const SYS_unlinkat: c_int = 35;
pub const SYS_unlink: c_int = 1026;
pub const SYS_unshare: c_int = 97;
pub const SYS_uselib: c_int = 1077;
pub const SYS_ustat: c_int = 1070;
pub const SYS_utime: c_int = 1063;
pub const SYS_utimensat: c_int = 88;
pub const SYS_utimes: c_int = 1037;
pub const SYS_vfork: c_int = 1071;
pub const SYS_vhangup: c_int = 58;
pub const SYS_vmsplice: c_int = 75;
pub const SYS_wait4: c_int = 260;
pub const SYS_waitid: c_int = 95;
pub const SYS_write: c_int = 64;
pub const SYS_writev: c_int = 66;
pub const SYS_statx: c_int = 291;
pub const SYS_pidfd_send_signal: c_long = 424;
pub const SYS_io_uring_setup: c_long = 425;
pub const SYS_io_uring_enter: c_long = 426;
pub const SYS_io_uring_register: c_long = 427;
pub const SYS_open_tree: c_long = 428;
pub const SYS_move_mount: c_long = 429;
pub const SYS_fsopen: c_long = 430;
pub const SYS_fsconfig: c_long = 431;
pub const SYS_fsmount: c_long = 432;
pub const SYS_fspick: c_long = 433;
pub const SYS_pidfd_open: c_long = 434;
pub const SYS_clone3: c_long = 435;
pub const SYS_close_range: c_long = 436;
pub const SYS_openat2: c_long = 437;
pub const SYS_pidfd_getfd: c_long = 438;
pub const SYS_faccessat2: c_long = 439;
pub const SYS_process_madvise: c_long = 440;
pub const SYS_epoll_pwait2: c_long = 441;
pub const SYS_mount_setattr: c_long = 442;
pub const TIOCM_LOOP: c_int = 32768;
pub const TIOCM_OUT1: c_int = 8192;
pub const TIOCM_OUT2: c_int = 16384;
pub const TIOCSER_TEMT: c_int = 1;
pub const TOSTOP: c_int = 256;
pub const VEOF: c_int = 4;
pub const VEOL2: c_int = 16;
pub const VEOL: c_int = 11;
pub const VMIN: c_int = 6;
