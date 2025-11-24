//! Documentation about unimplemented functions.
//!
//! This module contains documentation for several functions that rustix does
//! not implement, either because they are out of scope, or because they are
//! could probably be implemented but are not yet.

macro_rules! not_implemented {
    ($func:ident) => {
        /// See the [module comment](self).
        pub fn $func() {
            unimplemented!()
        }
    };
}

/// Memory-allocation functions are out of scope for rustix.
///
/// It is possible to implement `malloc`, `free`, and similar functions in
/// Rust, however rustix itself is focused on syscall-like functions. This
/// module contains an incomplete list of such functions.
///
/// There are several allocator implementations for Rust; one of them is
/// [dlmalloc]. For a rustix-based implementation, see [rustix-dlmalloc].
/// Another allocator implementation is [talc].
///
/// [dlmalloc]: https://crates.io/crates/dlmalloc
/// [talc]: https://crates.io/crates/talc
/// [rustix-dlmalloc]: https://crates.io/crates/rustix-dlmalloc
pub mod memory_allocation {
    not_implemented!(malloc);
    not_implemented!(realloc);
    not_implemented!(calloc);
    not_implemented!(free);
    not_implemented!(posix_memalign);
    not_implemented!(aligned_alloc);
    not_implemented!(malloc_usable_size);
}

/// Functions which need access to libc internals are out of scope for rustix.
///
/// Most Rust programs have a libc present, and when a libc is present, it
/// expects to be the only thing in the process that can do certain operations.
/// For example, there can be only one `atexit` list in a process, only one
/// `pthread_atfork` list in a process, only one implementation of pthreads in
/// a process, and so on, and libc expects to own the one of each of those
/// things. And libc implementations may expect to be involved in signal
/// handling. So, these functions are believed to be out of scope for rustix.
/// This module contains an incomplete list of such functions.
///
/// It would be possible to make a rust library which provides safe or
/// ergonomic wrappers around these libc functions, however that is out of
/// scope for rustix itself.
///
/// If you would like to write a Rust program which does not use a libc, and
/// which does provide APIs for some of these functions, [Eyra] and [origin]
/// are two libraries which may be useful, and which provide public interfaces
/// for some of this functionality.
///
/// If you are otherwise writing Rust code which you know will not share a
/// process with a libc, perhaps because you are writing a libc or similar
/// yourself, rustix's codebase does include experimental implementations of
/// the primitives needed to implement most of these functions.
///
/// [Eyra]: https://github.com/sunfishcode/eyra?tab=readme-ov-file#eyra
/// [origin]: https://github.com/sunfishcode/origin?tab=readme-ov-file#origin
pub mod libc_internals {
    not_implemented!(exit);
    not_implemented!(fork);
    not_implemented!(clone);
    not_implemented!(clone3);
    not_implemented!(brk);
    not_implemented!(sigaction);
    not_implemented!(sigaltstack);
    not_implemented!(sigprocmask);
    not_implemented!(sigwait);
    not_implemented!(sigwaitinfo);
    not_implemented!(sigtimedwait);
    not_implemented!(set_thread_area);
    not_implemented!(set_tid_address);
    not_implemented!(tkill);
    not_implemented!(sched_setscheduler);
    not_implemented!(rseq);
    not_implemented!(setuid);
    not_implemented!(setgid);
    not_implemented!(seteuid);
    not_implemented!(setegid);
    not_implemented!(setreuid);
    not_implemented!(setregid);
    not_implemented!(setresuid);
    not_implemented!(setresgid);
    not_implemented!(setgroups);

    not_implemented!(pthread_atfork);
    not_implemented!(pthread_attr_destroy);
    not_implemented!(pthread_attr_getaffinity_np);
    not_implemented!(pthread_attr_getdetachstate);
    not_implemented!(pthread_attr_getguardsize);
    not_implemented!(pthread_attr_getinheritsched);
    not_implemented!(pthread_attr_getschedparam);
    not_implemented!(pthread_attr_getschedpolicy);
    not_implemented!(pthread_attr_getscope);
    not_implemented!(pthread_attr_getsigmask_np);
    not_implemented!(pthread_attr_getstack);
    not_implemented!(pthread_attr_getstackaddr);
    not_implemented!(pthread_attr_getstacksize);
    not_implemented!(pthread_attr_init);
    not_implemented!(pthread_attr_setaffinity_np);
    not_implemented!(pthread_attr_setdetachstate);
    not_implemented!(pthread_attr_setguardsize);
    not_implemented!(pthread_attr_setinheritsched);
    not_implemented!(pthread_attr_setschedparam);
    not_implemented!(pthread_attr_setschedpolicy);
    not_implemented!(pthread_attr_setscope);
    not_implemented!(pthread_attr_setsigmask_np);
    not_implemented!(pthread_attr_setstack);
    not_implemented!(pthread_attr_setstackaddr);
    not_implemented!(pthread_attr_setstacksize);
    not_implemented!(pthread_barrierattr_destroy);
    not_implemented!(pthread_barrierattr_getpshared);
    not_implemented!(pthread_barrierattr_init);
    not_implemented!(pthread_barrierattr_setpshared);
    not_implemented!(pthread_barrier_destroy);
    not_implemented!(pthread_barrier_wait);
    not_implemented!(pthread_cancel);
    not_implemented!(pthread_cleanup_pop);
    not_implemented!(pthread_cleanup_pop_restore_np);
    not_implemented!(pthread_cleanup_push);
    not_implemented!(pthread_cleanup_push_defer_np);
    not_implemented!(pthread_condattr_destroy);
    not_implemented!(pthread_condattr_getclock);
    not_implemented!(pthread_condattr_getpshared);
    not_implemented!(pthread_condattr_init);
    not_implemented!(pthread_condattr_setclock);
    not_implemented!(pthread_condattr_setpshared);
    not_implemented!(pthread_cond_broadcast);
    not_implemented!(pthread_cond_destroy);
    not_implemented!(pthread_cond_signal);
    not_implemented!(pthread_cond_timedwait);
    not_implemented!(pthread_create);
    not_implemented!(pthread_detach);
    not_implemented!(pthread_equal);
    not_implemented!(pthread_exit);
    not_implemented!(pthread_getaffinity_np);
    not_implemented!(pthread_getattr_default_np);
    not_implemented!(pthread_getattr_np);
    not_implemented!(pthread_getconcurrency);
    not_implemented!(pthread_getcpuclockid);
    not_implemented!(pthread_getname_np);
    not_implemented!(pthread_getschedparam);
    not_implemented!(pthread_getspecific);
    not_implemented!(pthread_join);
    not_implemented!(pthread_key_create);
    not_implemented!(pthread_key_delete);
    not_implemented!(pthread_kill);
    not_implemented!(pthread_kill_other_threads_np);
    not_implemented!(pthread_mutexattr_destroy);
    not_implemented!(pthread_mutexattr_getprioceiling);
    not_implemented!(pthread_mutexattr_getprotocol);
    not_implemented!(pthread_mutexattr_getpshared);
    not_implemented!(pthread_mutexattr_getrobust);
    not_implemented!(pthread_mutexattr_getrobust_np);
    not_implemented!(pthread_mutexattr_gettype);
    not_implemented!(pthread_mutexattr_init);
    not_implemented!(pthread_mutexattr_setprioceiling);
    not_implemented!(pthread_mutexattr_setprotocol);
    not_implemented!(pthread_mutexattr_setpshared);
    not_implemented!(pthread_mutexattr_setrobust);
    not_implemented!(pthread_mutexattr_setrobust_np);
    not_implemented!(pthread_mutexattr_settype);
    not_implemented!(pthread_mutex_consistent);
    not_implemented!(pthread_mutex_consistent_np);
    not_implemented!(pthread_mutex_destroy);
    not_implemented!(pthread_mutex_getprioceiling);
    not_implemented!(pthread_mutex_init);
    not_implemented!(pthread_mutex_lock);
    not_implemented!(pthread_mutex_setprioceiling);
    not_implemented!(pthread_mutex_timedlock);
    not_implemented!(pthread_mutex_trylock);
    not_implemented!(pthread_once);
    not_implemented!(pthread_rwlockattr_destroy);
    not_implemented!(pthread_rwlockattr_getkind_np);
    not_implemented!(pthread_rwlockattr_getpshared);
    not_implemented!(pthread_rwlockattr_init);
    not_implemented!(pthread_rwlockattr_setkind_np);
    not_implemented!(pthread_rwlockattr_setpshared);
    not_implemented!(pthread_rwlock_destroy);
    not_implemented!(pthread_rwlock_rdlock);
    not_implemented!(pthread_rwlock_timedrdlock);
    not_implemented!(pthread_rwlock_timedwrlock);
    not_implemented!(pthread_rwlock_tryrdlock);
    not_implemented!(pthread_rwlock_trywrlock);
    not_implemented!(pthread_rwlock_unlock);
    not_implemented!(pthread_rwlock_wrlock);
    not_implemented!(pthread_self);
    not_implemented!(pthread_setaffinity_np);
    not_implemented!(pthread_setattr_default_np);
    not_implemented!(pthread_setcancelstate);
    not_implemented!(pthread_setcanceltype);
    not_implemented!(pthread_setconcurrency);
    not_implemented!(pthread_setname_np);
    not_implemented!(pthread_setschedparam);
    not_implemented!(pthread_setschedprio);
    not_implemented!(pthread_setspecific);
    not_implemented!(pthread_sigmask);
    not_implemented!(pthread_sigqueue);
    not_implemented!(pthread_spin_destroy);
    not_implemented!(pthread_spin_init);
    not_implemented!(pthread_spin_lock);
    not_implemented!(pthread_spin_trylock);
    not_implemented!(pthread_spin_unlock);
    not_implemented!(pthread_testcancel);
    not_implemented!(pthread_timedjoin_np);
    not_implemented!(pthread_tryjoin_np);
    not_implemented!(pthread_yield);
}

/// Functions which provide higher-level functionality are out of scope for
/// rustix.
///
/// These functions are provided by typical libc implementations, but are
/// higher-level than the simple syscall-like functions that rustix focuses on.
/// They could be implemented as a separate library built on top of rustix,
/// rather than being part of rustix itself. This module contains an incomplete
/// list of such functions.
pub mod higher_level {
    not_implemented!(getpwent);
    not_implemented!(getpwuid);
    not_implemented!(getpwnam);
    not_implemented!(getpwuid_r);
    not_implemented!(getpwnam_r);
    not_implemented!(gethostbyname);
    not_implemented!(execv);
    not_implemented!(execvp);
    not_implemented!(execvpe);
    not_implemented!(wordexp);
    not_implemented!(localtime);
    not_implemented!(localtime_r);
    not_implemented!(gmtime);
    not_implemented!(gmtime_r);
    not_implemented!(ctime);
    not_implemented!(ctime_r);
    not_implemented!(asctime);
    not_implemented!(asctime_r);
    not_implemented!(mktime);
    not_implemented!(getifaddrs);

    /// See [rustix-openpty](https://crates.io/crates/rustix-openpty).
    pub fn closefrom() {
        unimplemented!()
    }
    /// See [rustix-openpty](https://crates.io/crates/rustix-openpty).
    pub fn login_tty() {
        unimplemented!()
    }
    /// See [rustix-openpty](https://crates.io/crates/rustix-openpty).
    pub fn openpty() {
        unimplemented!()
    }

    /// See [`std::io::IsTerminal`].
    ///
    /// For Rust < 1.70, see [is-terminal]. For a rustix-based implementation,
    /// see [rustix-is-terminal].
    ///
    /// [`std::io::IsTerminal`]: std::io::IsTerminal
    /// [is-terminal]: https://crates.io/crates/is-terminal
    /// [rustix-is-terminal]: https://crates.io/crates/rustix-is-terminal
    pub fn isatty() {
        unimplemented!()
    }
}

/// Functions which don't seem possible to even call from Rust with current
/// language features, even with `unsafe`.
pub mod impossible {
    not_implemented!(vfork);
    not_implemented!(sigreturn);
    not_implemented!(setjmp);
    not_implemented!(longjmp);
    not_implemented!(sigsetjmp);
    not_implemented!(siglongjmp);
}

/// These functions are not yet implemented in rustix, but probably could be.
///
/// These are functions that users have asked about, and which probably are in
/// scope for rustix, but are not yet implemented. This module contains an
/// incomplete list of such functions.
pub mod yet {
    not_implemented!(tgkill);
    not_implemented!(raise);
    not_implemented!(sysctl);
    not_implemented!(mq_open);
    not_implemented!(mq_send);
    not_implemented!(mq_unlink);
    not_implemented!(recvmmsg);
    not_implemented!(cachestat);
    not_implemented!(fanotify_init);
    not_implemented!(fanotify_mark);
    not_implemented!(getifaddrs);
    not_implemented!(signalfd);
    not_implemented!(mount_setattr);
    not_implemented!(extattr_delete_fd);
    not_implemented!(extattr_delete_link);
    not_implemented!(extattr_get_fd);
    not_implemented!(extattr_get_link);
    not_implemented!(extattr_list_fd);
    not_implemented!(extattr_list_link);
    not_implemented!(extattr_set_fd);
    not_implemented!(extattr_set_link);
    not_implemented!(get_mempolicy);
    not_implemented!(mbind);
    not_implemented!(set_mempolicy);
    not_implemented!(migrate_pages);
    not_implemented!(move_pages);
    not_implemented!(fchmodat2);
    not_implemented!(shmat);
    not_implemented!(shmdt);
    not_implemented!(shmget);
    not_implemented!(shmctl);
}

/// These functions are not quite yet finished in rustix.
///
/// Rustix's codebase includes experimental implementations of these functions,
/// however they are not yet publicly exposed because their API might need more
/// work and/or they don't yet have a libc backend implementation yet.
///
/// See [#1314] for more information, and please leave comments if there are
/// specific functions you're interested in.
///
/// [#1314]: https://github.com/bytecodealliance/rustix/issues/1314
pub mod quite_yet {
    not_implemented!(_exit);
    not_implemented!(_Exit);
    not_implemented!(exit_group);
    not_implemented!(sigpending);
    not_implemented!(sigsuspend);
    not_implemented!(execveat);
    not_implemented!(execve);

    /// For now, use `rustix::process::uname().nodename()` instead.
    ///
    /// See also the [module comment](self).
    pub fn gethostname() {
        unimplemented!()
    }
}
