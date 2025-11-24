use crate::off64_t;
use crate::prelude::*;

pub type wchar_t = u32;
pub type greg_t = i64;
pub type __u64 = c_ulonglong;
pub type __s64 = c_longlong;

s! {
    pub struct stat {
        pub st_dev: crate::dev_t,
        pub st_ino: crate::ino_t,
        pub st_mode: c_uint,
        pub st_nlink: c_uint,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        __pad1: c_ulong,
        pub st_size: off64_t,
        pub st_blksize: c_int,
        __pad2: c_int,
        pub st_blocks: c_long,
        pub st_atime: crate::time_t,
        pub st_atime_nsec: c_long,
        pub st_mtime: crate::time_t,
        pub st_mtime_nsec: c_long,
        pub st_ctime: crate::time_t,
        pub st_ctime_nsec: c_long,
        __unused4: c_uint,
        __unused5: c_uint,
    }

    pub struct stat64 {
        pub st_dev: crate::dev_t,
        pub st_ino: crate::ino_t,
        pub st_mode: c_uint,
        pub st_nlink: c_uint,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        __pad1: c_ulong,
        pub st_size: off64_t,
        pub st_blksize: c_int,
        __pad2: c_int,
        pub st_blocks: c_long,
        pub st_atime: crate::time_t,
        pub st_atime_nsec: c_long,
        pub st_mtime: crate::time_t,
        pub st_mtime_nsec: c_long,
        pub st_ctime: crate::time_t,
        pub st_ctime_nsec: c_long,
        __unused4: c_uint,
        __unused5: c_uint,
    }
}

s_no_extra_traits! {
    #[repr(align(16))]
    pub struct max_align_t {
        priv_: [f32; 8],
    }
}

pub const O_DIRECT: c_int = 0x40000;
pub const O_DIRECTORY: c_int = 0x200000;
pub const O_NOFOLLOW: c_int = 0x400000;
pub const O_LARGEFILE: c_int = 0x100000;

pub const SIGSTKSZ: size_t = 8192;
pub const MINSIGSTKSZ: size_t = 2048;

// From NDK's asm/hwcap.h
pub const COMPAT_HWCAP_ISA_I: c_ulong = 1 << (b'I' - b'A');
pub const COMPAT_HWCAP_ISA_M: c_ulong = 1 << (b'M' - b'A');
pub const COMPAT_HWCAP_ISA_A: c_ulong = 1 << (b'A' - b'A');
pub const COMPAT_HWCAP_ISA_F: c_ulong = 1 << (b'F' - b'A');
pub const COMPAT_HWCAP_ISA_D: c_ulong = 1 << (b'D' - b'A');
pub const COMPAT_HWCAP_ISA_C: c_ulong = 1 << (b'C' - b'A');

pub const SYS_io_setup: c_long = 0;
pub const SYS_io_destroy: c_long = 1;
pub const SYS_io_submit: c_long = 2;
pub const SYS_io_cancel: c_long = 3;
pub const SYS_io_getevents: c_long = 4;
pub const SYS_setxattr: c_long = 5;
pub const SYS_lsetxattr: c_long = 6;
pub const SYS_fsetxattr: c_long = 7;
pub const SYS_getxattr: c_long = 8;
pub const SYS_lgetxattr: c_long = 9;
pub const SYS_fgetxattr: c_long = 10;
pub const SYS_listxattr: c_long = 11;
pub const SYS_llistxattr: c_long = 12;
pub const SYS_flistxattr: c_long = 13;
pub const SYS_removexattr: c_long = 14;
pub const SYS_lremovexattr: c_long = 15;
pub const SYS_fremovexattr: c_long = 16;
pub const SYS_getcwd: c_long = 17;
pub const SYS_lookup_dcookie: c_long = 18;
pub const SYS_eventfd2: c_long = 19;
pub const SYS_epoll_create1: c_long = 20;
pub const SYS_epoll_ctl: c_long = 21;
pub const SYS_epoll_pwait: c_long = 22;
pub const SYS_dup: c_long = 23;
pub const SYS_dup3: c_long = 24;
pub const SYS_inotify_init1: c_long = 26;
pub const SYS_inotify_add_watch: c_long = 27;
pub const SYS_inotify_rm_watch: c_long = 28;
pub const SYS_ioctl: c_long = 29;
pub const SYS_ioprio_set: c_long = 30;
pub const SYS_ioprio_get: c_long = 31;
pub const SYS_flock: c_long = 32;
pub const SYS_mknodat: c_long = 33;
pub const SYS_mkdirat: c_long = 34;
pub const SYS_unlinkat: c_long = 35;
pub const SYS_symlinkat: c_long = 36;
pub const SYS_linkat: c_long = 37;
pub const SYS_renameat: c_long = 38;
pub const SYS_umount2: c_long = 39;
pub const SYS_mount: c_long = 40;
pub const SYS_pivot_root: c_long = 41;
pub const SYS_nfsservctl: c_long = 42;
pub const SYS_fallocate: c_long = 47;
pub const SYS_faccessat: c_long = 48;
pub const SYS_chdir: c_long = 49;
pub const SYS_fchdir: c_long = 50;
pub const SYS_chroot: c_long = 51;
pub const SYS_fchmod: c_long = 52;
pub const SYS_fchmodat: c_long = 53;
pub const SYS_fchownat: c_long = 54;
pub const SYS_fchown: c_long = 55;
pub const SYS_openat: c_long = 56;
pub const SYS_close: c_long = 57;
pub const SYS_vhangup: c_long = 58;
pub const SYS_pipe2: c_long = 59;
pub const SYS_quotactl: c_long = 60;
pub const SYS_getdents64: c_long = 61;
pub const SYS_read: c_long = 63;
pub const SYS_write: c_long = 64;
pub const SYS_readv: c_long = 65;
pub const SYS_writev: c_long = 66;
pub const SYS_pread64: c_long = 67;
pub const SYS_pwrite64: c_long = 68;
pub const SYS_preadv: c_long = 69;
pub const SYS_pwritev: c_long = 70;
pub const SYS_pselect6: c_long = 72;
pub const SYS_ppoll: c_long = 73;
pub const SYS_signalfd4: c_long = 74;
pub const SYS_vmsplice: c_long = 75;
pub const SYS_splice: c_long = 76;
pub const SYS_tee: c_long = 77;
pub const SYS_readlinkat: c_long = 78;
pub const SYS_sync: c_long = 81;
pub const SYS_fsync: c_long = 82;
pub const SYS_fdatasync: c_long = 83;
pub const SYS_sync_file_range: c_long = 84;
pub const SYS_timerfd_create: c_long = 85;
pub const SYS_timerfd_settime: c_long = 86;
pub const SYS_timerfd_gettime: c_long = 87;
pub const SYS_utimensat: c_long = 88;
pub const SYS_acct: c_long = 89;
pub const SYS_capget: c_long = 90;
pub const SYS_capset: c_long = 91;
pub const SYS_personality: c_long = 92;
pub const SYS_exit: c_long = 93;
pub const SYS_exit_group: c_long = 94;
pub const SYS_waitid: c_long = 95;
pub const SYS_set_tid_address: c_long = 96;
pub const SYS_unshare: c_long = 97;
pub const SYS_futex: c_long = 98;
pub const SYS_set_robust_list: c_long = 99;
pub const SYS_get_robust_list: c_long = 100;
pub const SYS_nanosleep: c_long = 101;
pub const SYS_getitimer: c_long = 102;
pub const SYS_setitimer: c_long = 103;
pub const SYS_kexec_load: c_long = 104;
pub const SYS_init_module: c_long = 105;
pub const SYS_delete_module: c_long = 106;
pub const SYS_timer_create: c_long = 107;
pub const SYS_timer_gettime: c_long = 108;
pub const SYS_timer_getoverrun: c_long = 109;
pub const SYS_timer_settime: c_long = 110;
pub const SYS_timer_delete: c_long = 111;
pub const SYS_clock_settime: c_long = 112;
pub const SYS_clock_gettime: c_long = 113;
pub const SYS_clock_getres: c_long = 114;
pub const SYS_clock_nanosleep: c_long = 115;
pub const SYS_syslog: c_long = 116;
pub const SYS_ptrace: c_long = 117;
pub const SYS_sched_setparam: c_long = 118;
pub const SYS_sched_setscheduler: c_long = 119;
pub const SYS_sched_getscheduler: c_long = 120;
pub const SYS_sched_getparam: c_long = 121;
pub const SYS_sched_setaffinity: c_long = 122;
pub const SYS_sched_getaffinity: c_long = 123;
pub const SYS_sched_yield: c_long = 124;
pub const SYS_sched_get_priority_max: c_long = 125;
pub const SYS_sched_get_priority_min: c_long = 126;
pub const SYS_sched_rr_get_interval: c_long = 127;
pub const SYS_restart_syscall: c_long = 128;
pub const SYS_kill: c_long = 129;
pub const SYS_tkill: c_long = 130;
pub const SYS_tgkill: c_long = 131;
pub const SYS_sigaltstack: c_long = 132;
pub const SYS_rt_sigsuspend: c_long = 133;
pub const SYS_rt_sigaction: c_long = 134;
pub const SYS_rt_sigprocmask: c_long = 135;
pub const SYS_rt_sigpending: c_long = 136;
pub const SYS_rt_sigtimedwait: c_long = 137;
pub const SYS_rt_sigqueueinfo: c_long = 138;
pub const SYS_rt_sigreturn: c_long = 139;
pub const SYS_setpriority: c_long = 140;
pub const SYS_getpriority: c_long = 141;
pub const SYS_reboot: c_long = 142;
pub const SYS_setregid: c_long = 143;
pub const SYS_setgid: c_long = 144;
pub const SYS_setreuid: c_long = 145;
pub const SYS_setuid: c_long = 146;
pub const SYS_setresuid: c_long = 147;
pub const SYS_getresuid: c_long = 148;
pub const SYS_setresgid: c_long = 149;
pub const SYS_getresgid: c_long = 150;
pub const SYS_setfsuid: c_long = 151;
pub const SYS_setfsgid: c_long = 152;
pub const SYS_times: c_long = 153;
pub const SYS_setpgid: c_long = 154;
pub const SYS_getpgid: c_long = 155;
pub const SYS_getsid: c_long = 156;
pub const SYS_setsid: c_long = 157;
pub const SYS_getgroups: c_long = 158;
pub const SYS_setgroups: c_long = 159;
pub const SYS_uname: c_long = 160;
pub const SYS_sethostname: c_long = 161;
pub const SYS_setdomainname: c_long = 162;
pub const SYS_getrlimit: c_long = 163;
pub const SYS_setrlimit: c_long = 164;
pub const SYS_getrusage: c_long = 165;
pub const SYS_umask: c_long = 166;
pub const SYS_prctl: c_long = 167;
pub const SYS_getcpu: c_long = 168;
pub const SYS_gettimeofday: c_long = 169;
pub const SYS_settimeofday: c_long = 170;
pub const SYS_adjtimex: c_long = 171;
pub const SYS_getpid: c_long = 172;
pub const SYS_getppid: c_long = 173;
pub const SYS_getuid: c_long = 174;
pub const SYS_geteuid: c_long = 175;
pub const SYS_getgid: c_long = 176;
pub const SYS_getegid: c_long = 177;
pub const SYS_gettid: c_long = 178;
pub const SYS_sysinfo: c_long = 179;
pub const SYS_mq_open: c_long = 180;
pub const SYS_mq_unlink: c_long = 181;
pub const SYS_mq_timedsend: c_long = 182;
pub const SYS_mq_timedreceive: c_long = 183;
pub const SYS_mq_notify: c_long = 184;
pub const SYS_mq_getsetattr: c_long = 185;
pub const SYS_msgget: c_long = 186;
pub const SYS_msgctl: c_long = 187;
pub const SYS_msgrcv: c_long = 188;
pub const SYS_msgsnd: c_long = 189;
pub const SYS_semget: c_long = 190;
pub const SYS_semctl: c_long = 191;
pub const SYS_semtimedop: c_long = 192;
pub const SYS_semop: c_long = 193;
pub const SYS_shmget: c_long = 194;
pub const SYS_shmctl: c_long = 195;
pub const SYS_shmat: c_long = 196;
pub const SYS_shmdt: c_long = 197;
pub const SYS_socket: c_long = 198;
pub const SYS_socketpair: c_long = 199;
pub const SYS_bind: c_long = 200;
pub const SYS_listen: c_long = 201;
pub const SYS_accept: c_long = 202;
pub const SYS_connect: c_long = 203;
pub const SYS_getsockname: c_long = 204;
pub const SYS_getpeername: c_long = 205;
pub const SYS_sendto: c_long = 206;
pub const SYS_recvfrom: c_long = 207;
pub const SYS_setsockopt: c_long = 208;
pub const SYS_getsockopt: c_long = 209;
pub const SYS_shutdown: c_long = 210;
pub const SYS_sendmsg: c_long = 211;
pub const SYS_recvmsg: c_long = 212;
pub const SYS_readahead: c_long = 213;
pub const SYS_brk: c_long = 214;
pub const SYS_munmap: c_long = 215;
pub const SYS_mremap: c_long = 216;
pub const SYS_add_key: c_long = 217;
pub const SYS_request_key: c_long = 218;
pub const SYS_keyctl: c_long = 219;
pub const SYS_clone: c_long = 220;
pub const SYS_execve: c_long = 221;
pub const SYS_swapon: c_long = 224;
pub const SYS_swapoff: c_long = 225;
pub const SYS_mprotect: c_long = 226;
pub const SYS_msync: c_long = 227;
pub const SYS_mlock: c_long = 228;
pub const SYS_munlock: c_long = 229;
pub const SYS_mlockall: c_long = 230;
pub const SYS_munlockall: c_long = 231;
pub const SYS_mincore: c_long = 232;
pub const SYS_madvise: c_long = 233;
pub const SYS_remap_file_pages: c_long = 234;
pub const SYS_mbind: c_long = 235;
pub const SYS_get_mempolicy: c_long = 236;
pub const SYS_set_mempolicy: c_long = 237;
pub const SYS_migrate_pages: c_long = 238;
pub const SYS_move_pages: c_long = 239;
pub const SYS_rt_tgsigqueueinfo: c_long = 240;
pub const SYS_perf_event_open: c_long = 241;
pub const SYS_accept4: c_long = 242;
pub const SYS_recvmmsg: c_long = 243;
pub const SYS_arch_specific_syscall: c_long = 244;
pub const SYS_wait4: c_long = 260;
pub const SYS_prlimit64: c_long = 261;
pub const SYS_fanotify_init: c_long = 262;
pub const SYS_fanotify_mark: c_long = 263;
pub const SYS_name_to_handle_at: c_long = 264;
pub const SYS_open_by_handle_at: c_long = 265;
pub const SYS_clock_adjtime: c_long = 266;
pub const SYS_syncfs: c_long = 267;
pub const SYS_setns: c_long = 268;
pub const SYS_sendmmsg: c_long = 269;
pub const SYS_process_vm_readv: c_long = 270;
pub const SYS_process_vm_writev: c_long = 271;
pub const SYS_kcmp: c_long = 272;
pub const SYS_finit_module: c_long = 273;
pub const SYS_sched_setattr: c_long = 274;
pub const SYS_sched_getattr: c_long = 275;
pub const SYS_renameat2: c_long = 276;
pub const SYS_seccomp: c_long = 277;
pub const SYS_getrandom: c_long = 278;
pub const SYS_memfd_create: c_long = 279;
pub const SYS_bpf: c_long = 280;
pub const SYS_execveat: c_long = 281;
pub const SYS_userfaultfd: c_long = 282;
pub const SYS_membarrier: c_long = 283;
pub const SYS_mlock2: c_long = 284;
pub const SYS_copy_file_range: c_long = 285;
pub const SYS_preadv2: c_long = 286;
pub const SYS_pwritev2: c_long = 287;
pub const SYS_pkey_mprotect: c_long = 288;
pub const SYS_pkey_alloc: c_long = 289;
pub const SYS_pkey_free: c_long = 290;
pub const SYS_statx: c_long = 291;
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
pub const SYS_quotactl_fd: c_long = 443;
pub const SYS_landlock_create_ruleset: c_long = 444;
pub const SYS_landlock_add_rule: c_long = 445;
pub const SYS_landlock_restrict_self: c_long = 446;
pub const SYS_memfd_secret: c_long = 447;
pub const SYS_process_mrelease: c_long = 448;
pub const SYS_futex_waitv: c_long = 449;
pub const SYS_set_mempolicy_home_node: c_long = 450;

// From NDK's asm/auxvec.h
pub const AT_SYSINFO_EHDR: c_ulong = 33;
pub const AT_L1I_CACHESIZE: c_ulong = 40;
pub const AT_L1I_CACHEGEOMETRY: c_ulong = 41;
pub const AT_L1D_CACHESIZE: c_ulong = 42;
pub const AT_L1D_CACHEGEOMETRY: c_ulong = 43;
pub const AT_L2_CACHESIZE: c_ulong = 44;
pub const AT_L2_CACHEGEOMETRY: c_ulong = 45;
pub const AT_L3_CACHESIZE: c_ulong = 46;
pub const AT_L3_CACHEGEOMETRY: c_ulong = 47;
pub const AT_VECTOR_SIZE_ARCH: c_ulong = 9;
