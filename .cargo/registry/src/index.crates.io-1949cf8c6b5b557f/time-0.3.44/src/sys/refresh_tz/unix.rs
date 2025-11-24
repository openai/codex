/// Whether the operating system has a thread-safe environment. This allows bypassing the check for
/// if the process is multi-threaded.
// This is the same value as `cfg!(target_os = "x")`.
// Use byte-strings to work around current limitations of const eval.
const OS_HAS_THREAD_SAFE_ENVIRONMENT: bool = match std::env::consts::OS.as_bytes() {
    // https://github.com/illumos/illumos-gate/blob/0fb96ba1f1ce26ff8b286f8f928769a6afcb00a6/usr/src/lib/libc/port/gen/getenv.c
    b"illumos"
    // https://github.com/NetBSD/src/blob/f45028636a44111bc4af44d460924958a4460844/lib/libc/stdlib/getenv.c
    // https://github.com/NetBSD/src/blob/f45028636a44111bc4af44d460924958a4460844/lib/libc/stdlib/setenv.c
    | b"netbsd"
    // https://github.com/apple-oss-distributions/Libc/blob/63976b830a836a22649b806fe62e8614fe3e5555/stdlib/FreeBSD/getenv.c#L118
    // https://github.com/apple-oss-distributions/Libc/blob/63976b830a836a22649b806fe62e8614fe3e5555/stdlib/FreeBSD/setenv.c#L446
    // https://blog.rust-lang.org/2023/09/25/Increasing-Apple-Version-Requirements/
    | b"macos"
    => true,
    _ => false,
};

/// Update time zone information from the system.
///
/// For safety documentation, see [`time::util::refresh_tz`].
#[inline]
pub(super) unsafe fn refresh_tz_unchecked() {
    extern "C" {
        #[cfg_attr(target_os = "netbsd", link_name = "__tzset50")]
        fn tzset();
    }

    // Safety: The caller must uphold the safety requirements.
    unsafe { tzset() };
}

/// Attempt to update time zone information from the system. Returns `None` if the call is not known
/// to be sound.
#[inline]
pub(super) fn refresh_tz() -> Option<()> {
    // Refresh $TZ if and only if the call is known to be sound.
    //
    // Soundness can be guaranteed either by knowledge of the operating system or knowledge that the
    // process is single-threaded. If the process is single-threaded, then the environment cannot
    // be mutated by a different thread in the process while execution of this function is taking
    // place, which can cause a segmentation fault by dereferencing a dangling pointer.
    //
    // If the `num_threads` crate is incapable of determining the number of running threads, then
    // we conservatively return `None` to avoid a soundness bug.

    if OS_HAS_THREAD_SAFE_ENVIRONMENT || num_threads::is_single_threaded() == Some(true) {
        // Safety: The caller must uphold the safety requirements.
        unsafe { refresh_tz_unchecked() };
        Some(())
    } else {
        None
    }
}
